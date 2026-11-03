use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use garrido_studio::{Error, Result, model::Sample as AudioSample};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

fn failure(error: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(error.to_string()))
}
const MAX_CALLBACK: usize = 65_536;

struct Output {
    frames: Arc<Vec<[f32; 2]>>,
    cursor: AtomicU64,
    consumed: AtomicU64,
    failed: AtomicBool,
}
struct Reader {
    state: Arc<Output>,
    phase: u64,
    source_rate: u32,
    device_rate: u32,
    channels: usize,
}
impl Reader {
    fn fill<T: Sample + FromSample<f32>>(&mut self, data: &mut [T]) {
        if data.len() > MAX_CALLBACK || !data.len().is_multiple_of(self.channels) {
            data.fill(T::from_sample(0.));
            self.state.failed.store(true, Ordering::Relaxed);
            return;
        }
        let modulus = self.state.frames.len() as u64 * self.device_rate as u64;
        for frame in data.chunks_exact_mut(self.channels) {
            let i = (self.phase / self.device_rate as u64) as usize;
            let next = (i + 1) % self.state.frames.len();
            let t = (self.phase % self.device_rate as u64) as f32 / self.device_rate as f32;
            let pair = std::array::from_fn::<_, 2, _>(|ch| {
                let value = self.state.frames[i][ch] * (1. - t) + self.state.frames[next][ch] * t;
                if value.is_finite() {
                    value.clamp(-0.8, 0.8) * 0.5
                } else {
                    0.
                }
            });
            frame.fill(T::from_sample(0.));
            if self.channels == 1 {
                frame[0] = T::from_sample((pair[0] + pair[1]) * 0.5)
            } else {
                frame[0] = T::from_sample(pair[0]);
                frame[1] = T::from_sample(pair[1]);
            }
            self.phase = (self.phase + self.source_rate as u64) % modulus;
        }
        self.state
            .consumed
            .fetch_add((data.len() / self.channels) as u64, Ordering::Relaxed);
        self.state
            .cursor
            .store(self.phase / self.device_rate as u64, Ordering::Relaxed);
    }
}
pub(crate) struct BufferedPlayback {
    _stream: Stream,
    state: Arc<Output>,
}
pub(crate) enum Playback {
    Buffered(BufferedPlayback),
    Streaming(crate::streaming::Player),
}
impl Playback {
    pub fn start(
        frames: Vec<[f32; 2]>,
        rate: u32,
        preferences: &crate::audio_settings::Preferences,
    ) -> Result<Self> {
        if frames.is_empty() || frames.len() > 12_000_000 || !(8000..=192000).contains(&rate) {
            return Err(Error::Limit("Audio de reproducción fuera de límites"));
        }
        let device = preferences.device(false)?;
        let supported = preferences.config(&device, false)?;
        let config = preferences.stream_config(&supported)?;
        if !(1..=8).contains(&config.channels) || !(8000..=192000).contains(&config.sample_rate) {
            return Err(Error::Unsupported(
                "La salida necesita entre 1 y 8 canales y 8–192 kHz",
            ));
        }
        let state = Arc::new(Output {
            frames: Arc::new(frames),
            cursor: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        let stream = match supported.sample_format() {
            SampleFormat::F32 => output::<f32>(&device, config, state.clone(), rate),
            SampleFormat::F64 => output::<f64>(&device, config, state.clone(), rate),
            SampleFormat::I16 => output::<i16>(&device, config, state.clone(), rate),
            SampleFormat::I32 => output::<i32>(&device, config, state.clone(), rate),
            _ => return Err(Error::Unsupported("Formato de salida no admitido")),
        }?;
        stream.play().map_err(failure)?;
        Ok(Self::Buffered(BufferedPlayback {
            _stream: stream,
            state,
        }))
    }
    pub(crate) fn stream(
        project: garrido_studio::model::Project,
        engine: garrido_studio::engine::Engine,
        metronome: bool,
        preferences: &crate::audio_settings::Preferences,
    ) -> Result<Self> {
        crate::streaming::Player::new(project, engine, metronome, preferences).map(Self::Streaming)
    }
    pub fn frame(&self) -> u64 {
        match self {
            Self::Buffered(p) => p.state.cursor.load(Ordering::Relaxed),
            Self::Streaming(p) => p.frame(),
        }
    }
    pub fn healthy(&self) -> bool {
        match self {
            Self::Buffered(p) => !p.state.failed.load(Ordering::Relaxed),
            Self::Streaming(p) => p.healthy(),
        }
    }
    pub(crate) fn meters(&self) -> Option<(f32, f32, usize, u64)> {
        match self {
            Self::Streaming(p) => Some(p.meters()),
            _ => None,
        }
    }
}
fn output<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    state: Arc<Output>,
    rate: u32,
) -> Result<Stream> {
    let errors = state.clone();
    let mut reader = Reader {
        state,
        phase: 0,
        source_rate: rate,
        device_rate: config.sample_rate,
        channels: config.channels as usize,
    };
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

struct Writer {
    input: garrido_studio::recording::Input,
    channels: usize,
    pairs: Vec<u16>,
}
impl Writer {
    fn push<T: Sample>(&mut self, data: &[T])
    where
        f32: FromSample<T>,
    {
        if self.channels == 0
            || self.channels > 32
            || self.pairs.is_empty()
            || self.pairs.len() > 16
            || self
                .pairs
                .iter()
                .any(|pair| *pair as usize * 2 >= self.channels)
            || data.len() > MAX_CALLBACK
            || !data.len().is_multiple_of(self.channels)
        {
            self.input.interrupt();
            return;
        }
        let mut normalized = [0f32; 8192];
        let width = self.pairs.len() * 2;
        for chunk in data.chunks(self.channels * 256) {
            let frames = chunk.len() / self.channels;
            for (index, frame) in chunk.chunks_exact(self.channels).enumerate() {
                for (pair, source) in self.pairs.iter().enumerate() {
                    for ch in 0..2 {
                        let value = f32::from_sample(
                            frame[(*source as usize * 2 + ch).min(self.channels - 1)],
                        );
                        normalized[index * width + pair * 2 + ch] = if value.is_finite() {
                            value.clamp(-1., 1.)
                        } else {
                            0.
                        };
                    }
                }
            }
            if self.input.push(&normalized[..frames * width]) < frames {
                break;
            }
        }
    }
}
pub struct Capture {
    stream: Stream,
    recording: garrido_studio::recording::Recording,
    rate: u32,
    capacity: u64,
    disk_check: std::cell::Cell<std::time::Instant>,
}
impl Capture {
    pub fn start(
        preferences: &crate::audio_settings::Preferences,
        folder: &std::path::Path,
        slots: usize,
        frame_budget: usize,
    ) -> Result<Self> {
        let device = preferences.device(true)?;
        let supported = preferences.config(&device, true)?;
        let config = preferences.stream_config(&supported)?;
        if !(1..=32).contains(&config.channels)
            || !(8000..=192000).contains(&config.sample_rate)
            || (!preferences.all_inputs && preferences.input_pair * 2 >= config.channels)
        {
            return Err(Error::Unsupported(
                "Selecciona un par disponible en una entrada de 1–32 canales y 8–192 kHz",
            ));
        }
        let pairs: Vec<u16> = if preferences.all_inputs {
            (0..config.channels.div_ceil(2)).collect()
        } else {
            vec![preferences.input_pair]
        };
        if pairs.len() > slots || frame_budget < pairs.len() {
            return Err(Error::Limit(
                "No quedan canales, filas o espacio de muestras suficientes para las entradas",
            ));
        }
        let capacity = (config.sample_rate as u64 * 120)
            .min(frame_budget as u64 / pairs.len() as u64)
            .min(4_000_000 / pairs.len() as u64);
        let ancestor = folder
            .ancestors()
            .find(|p| p.is_dir())
            .ok_or(Error::Invalid("Falta la carpeta de grabación"))?;
        if fs4::available_space(ancestor)?
            < 3 * 1024 * 1024 * 1024 + capacity * pairs.len() as u64 * 8 + 65536
        {
            return Err(Error::Limit(
                "La grabación conserva 3 GiB libres además del tamaño de la toma",
            ));
        }
        let (input, recording) = garrido_studio::recording::Recording::start(
            folder,
            config.sample_rate,
            pairs.len(),
            capacity,
        )?;
        let device_name: String = device
            .to_string()
            .chars()
            .filter(|c| !c.is_control())
            .take(128)
            .collect();
        let mut mapping = format!(
            "Garrido Studio · {device_name}\n{} Hz · stereo float32 WAV\n",
            config.sample_rate
        );
        for (index, pair) in pairs.iter().enumerate() {
            let first = pair * 2 + 1;
            mapping.push_str(&format!(
                "Input-{:02}.wav: input channels {}–{}\n",
                index + 1,
                first,
                (first + 1).min(config.channels)
            ));
        }
        garrido_studio::storage::write_new(
            &recording.folder().join("INPUTS.txt"),
            mapping.as_bytes(),
        )?;
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(&device, config, input, pairs),
            SampleFormat::F64 => input_stream::<f64>(&device, config, input, pairs),
            SampleFormat::I16 => input_stream::<i16>(&device, config, input, pairs),
            SampleFormat::I32 => input_stream::<i32>(&device, config, input, pairs),
            _ => return Err(Error::Unsupported("Formato de entrada no admitido")),
        }?;
        stream.play().map_err(failure)?;
        Ok(Self {
            stream,
            recording,
            rate: config.sample_rate,
            capacity,
            disk_check: std::cell::Cell::new(std::time::Instant::now()),
        })
    }
    pub fn folder(&self) -> &std::path::Path {
        self.recording.folder()
    }
    pub fn limit_seconds(&self) -> f64 {
        self.capacity as f64 / self.rate as f64
    }
    pub fn done(&self) -> bool {
        if self.disk_check.get().elapsed() >= Duration::from_secs(1) {
            self.disk_check.set(std::time::Instant::now());
            if fs4::available_space(self.recording.folder()).unwrap_or(0) < 3 * 1024 * 1024 * 1024 {
                self.recording.interrupt();
            }
        }
        self.recording.done()
    }
    pub fn finish(self) -> Result<(Vec<AudioSample>, bool)> {
        if self.stream.pause().is_err() {
            self.recording.interrupt();
        }
        drop(self.stream);
        let report = self.recording.finish()?;
        let mut samples = Vec::with_capacity(report.takes.len());
        let mut interrupted = report.interrupted;
        for take in report.takes {
            if take.frames == 0 {
                continue;
            }
            let (sample, repaired) = garrido_studio::recording::recover(&take.path)?;
            interrupted |= repaired;
            samples.push(sample);
        }
        if samples.is_empty() {
            return Err(Error::Invalid(
                "La entrada no ha recibido audio; los archivos de toma se conservan",
            ));
        }
        Ok((samples, interrupted))
    }
}
fn input_stream<T: SizedSample>(
    device: &cpal::Device,
    config: StreamConfig,
    input: garrido_studio::recording::Input,
    pairs: Vec<u16>,
) -> Result<Stream>
where
    f32: FromSample<T>,
{
    let errors = input.control();
    let mut writer = Writer {
        input,
        channels: config.channels as usize,
        pairs,
    };
    device
        .build_input_stream(
            config,
            move |data: &[T], _| writer.push(data),
            move |_| errors.interrupt(),
            Some(Duration::from_secs(2)),
        )
        .map_err(failure)
}

pub(crate) fn probe() -> Result<()> {
    let playback = Playback::start(
        vec![[0.; 2]; 4800],
        48000,
        &crate::audio_settings::Preferences::default(),
    )?;
    std::thread::sleep(Duration::from_secs(1));
    let frames = match &playback {
        Playback::Buffered(p) => p.state.consumed.load(Ordering::Relaxed),
        Playback::Streaming(_) => {
            return Err(Error::Invalid("La sonda necesita silencio precalculado"));
        }
    };
    if !playback.healthy() || frames == 0 {
        return Err(Error::Invalid(
            "La salida no ha recibido buffers de silencio",
        ));
    }
    println!("{{\"silent\":true,\"callbacks_consumed_frames\":{frames}}}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_resamples_loops_silences_extra_channels_and_clamps() {
        let state = Arc::new(Output {
            frames: Arc::new(vec![[2., f32::NAN], [-2., 0.4]]),
            cursor: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        let mut reader = Reader {
            state,
            phase: 0,
            source_rate: 8000,
            device_rate: 16000,
            channels: 4,
        };
        let mut out = [9f32; 20];
        reader.fill(&mut out);
        assert_eq!(out[0], 0.4);
        assert_eq!(out[8], -0.4);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 0.4));
        assert!(out.as_chunks::<4>().0.iter().all(|f| f[2..] == [0., 0.]));
    }
    #[test]
    fn capture_converts_selected_pairs_without_devices_and_preserves_mono_channels() {
        let folder = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("garrido-cpal-pairs-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let (input, recording) =
            garrido_studio::recording::Recording::start(&folder, 8000, 2, 10).unwrap();
        let mut writer = Writer {
            input,
            channels: 3,
            pairs: vec![0, 1],
        };
        writer.push(&[0.25f32, -0.5, 0.95, f32::NAN, 2., -2.]);
        drop(writer);
        let report = recording.finish().unwrap();
        assert!(!report.interrupted);
        let first = garrido_studio::recording::recover(&report.takes[0].path)
            .unwrap()
            .0;
        let second = garrido_studio::recording::recover(&report.takes[1].path)
            .unwrap()
            .0;
        assert_eq!(&*first.frames, &[[0.25, -0.5], [0., 1.]]);
        assert_eq!(&*second.frames, &[[0.95, 0.95], [-1., -1.]]);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn invalid_capture_channel_layout_fails_closed_without_devices() {
        let folder = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("garrido-cpal-invalid-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let (input, recording) =
            garrido_studio::recording::Recording::start(&folder, 8000, 1, 10).unwrap();
        let mut writer = Writer {
            input,
            channels: 2,
            pairs: vec![1],
        };
        writer.push(&[0f32; 4]);
        drop(writer);
        assert!(recording.finish().unwrap().interrupted);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn all_sixteen_input_pairs_reach_their_own_files_without_index_overlap() {
        let folder = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("garrido-cpal-32-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let (input, recording) =
            garrido_studio::recording::Recording::start(&folder, 8000, 16, 256).unwrap();
        let mut writer = Writer {
            input,
            channels: 32,
            pairs: (0..16).collect(),
        };
        let data: Vec<_> = (0..256 * 32).map(|i| (i % 32) as f32 / 64.).collect();
        writer.push(&data);
        drop(writer);
        let report = recording.finish().unwrap();
        assert!(!report.interrupted);
        assert_eq!(report.takes.len(), 16);
        for (index, take) in report.takes.iter().enumerate() {
            let sample = garrido_studio::recording::recover(&take.path).unwrap().0;
            assert_eq!(sample.frames.len(), 256);
            assert!(
                sample
                    .frames
                    .iter()
                    .all(|frame| *frame == [index as f32 / 32., (index * 2 + 1) as f32 / 64.])
            );
        }
        std::fs::remove_dir_all(folder).unwrap();
    }
}
