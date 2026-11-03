use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use garrido_studio::{
    Error, Result,
    engine::Engine,
    model::Project,
    stream::{self, Consumer, Producer},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

struct State {
    stop: AtomicBool,
    started: AtomicBool,
    failed: AtomicBool,
    consumed: AtomicU64,
    underruns: AtomicU64,
    peak: AtomicU32,
    rms: AtomicU32,
    voices: AtomicU64,
    length: u64,
    rate: u32,
    source_rate: u32,
}
struct Reader {
    consumer: Consumer,
    state: Arc<State>,
    channels: usize,
}
impl Reader {
    fn fill<T: Sample + FromSample<f32>>(&mut self, data: &mut [T]) {
        data.fill(T::from_sample(0.));
        if self.state.stop.load(Ordering::Relaxed) || !self.state.started.load(Ordering::Acquire) {
            return;
        }
        if self.channels == 0
            || self.channels > 8
            || data.len() > 65536
            || !data.len().is_multiple_of(self.channels)
        {
            self.state.failed.store(true, Ordering::Relaxed);
            return;
        }
        let mut frames = [[0.; 2]; 512];
        let mut consumed = 0;
        let mut gap = false;
        for block in data.chunks_mut(self.channels * 512) {
            let count = block.len() / self.channels;
            let received = self.consumer.pop(&mut frames[..count]);
            consumed += received;
            gap |= received < count;
            for (destination, pair) in block.chunks_exact_mut(self.channels).zip(&frames) {
                if self.channels == 1 {
                    destination[0] = T::from_sample((pair[0] + pair[1]) * 0.25);
                } else {
                    destination[0] = T::from_sample(pair[0] * 0.5);
                    destination[1] = T::from_sample(pair[1] * 0.5);
                }
            }
        }
        self.state
            .consumed
            .fetch_add(consumed as u64, Ordering::Relaxed);
        if gap {
            self.state.underruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}
fn produce(engine: &mut Engine, producer: &mut Producer, state: &State) -> usize {
    let mut block = [[0.; 2]; 512];
    let count = engine.process(&mut block);
    if count > 0 {
        let peak = block[..count]
            .iter()
            .flatten()
            .fold(0f32, |a, &b| a.max(b.abs()));
        let rms = (block[..count]
            .iter()
            .flatten()
            .map(|&v| (v as f64).powi(2))
            .sum::<f64>()
            / (count * 2) as f64)
            .sqrt() as f32;
        state.peak.store(peak.to_bits(), Ordering::Relaxed);
        state.rms.store(rms.to_bits(), Ordering::Relaxed);
        state
            .voices
            .store(engine.peak_voices as u64, Ordering::Relaxed);
        producer.push(&block[..count]);
    }
    count
}
pub(crate) struct Player {
    stream: Stream,
    state: Arc<State>,
    worker: Option<JoinHandle<()>>,
}
impl Player {
    pub(crate) fn new(
        mut project: Project,
        mut engine: Engine,
        metronome: bool,
        preferences: &crate::audio_settings::Preferences,
    ) -> Result<Self> {
        let device = preferences.device(false)?;
        let supported = preferences.config(&device, false)?;
        let config = preferences.stream_config(&supported)?;
        if !(1..=8).contains(&config.channels) || !(8000..=192000).contains(&config.sample_rate) {
            return Err(Error::Unsupported(
                "Salida de 1–8 canales y 8–192 kHz requerida",
            ));
        }
        let source_rate = project.rate;
        if project.rate != config.sample_rate {
            project.rate = config.sample_rate;
            engine = Engine::streaming(&project)?;
            if metronome {
                engine = engine.with_metronome();
            }
        }
        let state = Arc::new(State {
            stop: AtomicBool::new(false),
            started: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            consumed: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            peak: AtomicU32::new(0),
            rms: AtomicU32::new(0),
            voices: AtomicU64::new(0),
            length: engine.frames(),
            rate: project.rate,
            source_rate,
        });
        let (mut producer, consumer) = stream::pipe(8192)?;
        while producer.available() >= 512 && engine.position() < engine.frames() {
            produce(&mut engine, &mut producer, &state);
        }
        let channels = config.channels as usize;
        let reader = Reader {
            consumer,
            state: state.clone(),
            channels,
        };
        let stream = match supported.sample_format() {
            SampleFormat::F32 => output::<f32>(&device, config, reader),
            SampleFormat::F64 => output::<f64>(&device, config, reader),
            SampleFormat::I16 => output::<i16>(&device, config, reader),
            SampleFormat::I32 => output::<i32>(&device, config, reader),
            _ => Err(Error::Unsupported("Formato de salida no admitido")),
        }?;
        let shared = state.clone();
        let worker = thread::Builder::new()
            .name("garrido-audio".into())
            .spawn(move || {
                while !shared.stop.load(Ordering::Acquire) {
                    if producer.available() < 512 {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    if engine.position() == engine.frames() {
                        match Engine::streaming(&project) {
                            Ok(next) => {
                                engine = if metronome {
                                    next.with_metronome()
                                } else {
                                    next
                                }
                            }
                            Err(_) => {
                                shared.failed.store(true, Ordering::Relaxed);
                                return;
                            }
                        }
                    }
                    produce(&mut engine, &mut producer, &shared);
                }
            })
            .map_err(Error::Io)?;
        let mut player = Self {
            stream,
            state,
            worker: Some(worker),
        };
        if let Err(error) = player.stream.play() {
            player.stop();
            return Err(failure(error));
        }
        player.state.started.store(true, Ordering::Release);
        Ok(player)
    }
    fn stop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        let _ = self.stream.pause();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
    pub(crate) fn frame(&self) -> u64 {
        (self.state.consumed.load(Ordering::Relaxed) % self.state.length)
            * self.state.source_rate as u64
            / self.state.rate as u64
    }
    pub(crate) fn healthy(&self) -> bool {
        !self.state.failed.load(Ordering::Relaxed)
            && !self.worker.as_ref().is_some_and(JoinHandle::is_finished)
    }
    pub(crate) fn meters(&self) -> (f32, f32, usize, u64) {
        (
            f32::from_bits(self.state.peak.load(Ordering::Relaxed)),
            f32::from_bits(self.state.rms.load(Ordering::Relaxed)),
            self.state.voices.load(Ordering::Relaxed) as usize,
            self.state.underruns.load(Ordering::Relaxed),
        )
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}
fn failure(error: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(error.to_string()))
}
fn output<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    mut reader: Reader,
) -> Result<Stream> {
    let errors = reader.state.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| reader.fill(data),
            move |_| {
                errors.failed.store(true, Ordering::Relaxed);
            },
            Some(Duration::from_secs(2)),
        )
        .map_err(failure)
}

pub(crate) fn probe() -> Result<()> {
    let mut project = Project::empty();
    project.bars = 1;
    project.bpm = 300.;
    project.master = 0.;
    let engine = Engine::new(&project, false, 0.)?;
    let player = Player::new(
        project,
        engine,
        false,
        &crate::audio_settings::Preferences::default(),
    )?;
    thread::sleep(Duration::from_secs(1));
    let frames = player.state.consumed.load(Ordering::Relaxed);
    let (peak, rms, _, underruns) = player.meters();
    let healthy = player.healthy();
    println!(
        "{{\"silent\":true,\"streamed_frames\":{frames},\"underruns\":{underruns},\"healthy\":{healthy},\"peak\":{peak},\"rms\":{rms}}}"
    );
    if !healthy || frames == 0 || underruns != 0 || peak != 0. || rms != 0. {
        return Err(Error::Invalid(
            "La sonda por bloques ha detectado una interrupción",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reader(channels: usize) -> (Producer, Reader) {
        let (tx, rx) = stream::pipe(64).unwrap();
        let state = Arc::new(State {
            stop: AtomicBool::new(false),
            started: AtomicBool::new(true),
            failed: AtomicBool::new(false),
            consumed: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            peak: AtomicU32::new(0),
            rms: AtomicU32::new(0),
            voices: AtomicU64::new(0),
            length: 64,
            rate: 8000,
            source_rate: 8000,
        });
        (
            tx,
            Reader {
                consumer: rx,
                state,
                channels,
            },
        )
    }
    #[test]
    fn callback_silences_underruns_extra_channels_and_invalid_driver_buffers() {
        let (mut tx, mut reader) = reader(4);
        tx.push(&[[0.2, -0.4]; 8]);
        let mut out = [9f32; 64];
        reader.fill(&mut out);
        assert_eq!(&out[..4], &[0.1, -0.2, 0., 0.]);
        assert!(out[32..].iter().all(|v| *v == 0.));
        assert_eq!(reader.state.consumed.load(Ordering::Relaxed), 8);
        assert_eq!(reader.state.underruns.load(Ordering::Relaxed), 1);
        reader.fill(&mut out[..3]);
        assert!(reader.state.failed.load(Ordering::Relaxed));
        assert_eq!(&out[..3], &[0.; 3]);
    }
    #[test]
    fn callbacks_before_play_are_silent_without_false_underruns() {
        let (_, mut reader) = reader(2);
        reader.state.started.store(false, Ordering::Release);
        let mut out = [9f32; 64];
        reader.fill(&mut out);
        assert_eq!(out, [0.; 64]);
        assert_eq!(reader.state.underruns.load(Ordering::Relaxed), 0);
        reader.state.started.store(true, Ordering::Release);
        reader.fill(&mut out);
        assert_eq!(reader.state.underruns.load(Ordering::Relaxed), 1);
    }
}
