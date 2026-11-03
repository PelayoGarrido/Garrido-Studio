use crate::{
    Error, Result,
    dsp::{self, OutputGuard, Processor},
    model::*,
};
use std::{
    f64::consts::TAU,
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_VOICES: usize = 64;
const MAX_VOICE_FRAMES: u64 = 80_000_000;

#[derive(Clone)]
struct Event {
    at: u64,
    end: u64,
    track: usize,
    note: Note,
    audio: Option<AudioPlayback>,
}

#[derive(Clone, Copy)]
struct AudioPlayback {
    origin: i64,
    offset: f64,
}

struct Voice {
    track: usize,
    key: u8,
    velocity: f32,
    panning: [f32; 2],
    age: u64,
    started: u64,
    audio: Option<AudioPlayback>,
    gate: u64,
    phase: [f64; 6],
    frequency: f64,
    noise: u64,
    low: f64,
    fm_envelope: [f64; 6],
    fm_decay: [f64; 6],
}

impl Voice {
    fn new(event: &Event, rate: u32) -> Self {
        Self {
            track: event.track,
            key: event.note.key,
            velocity: event.note.velocity,
            panning: dsp::pan(event.note.pan),
            age: 0,
            started: event.at,
            audio: event.audio,
            gate: event.end - event.at,
            phase: [0.; 6],
            frequency: 440. * 2f64.powf((event.note.key as f64 - 69.) / 12.),
            noise: event
                .at
                .wrapping_mul(0x9e3779b97f4a7c15)
                .wrapping_add(event.note.key as u64 + 1)
                .max(1),
            low: 0.,
            fm_envelope: [1.; 6],
            fm_decay: std::array::from_fn(|i| (-((i + 1) as f64) / rate as f64).exp()),
        }
    }

    fn next(&mut self, track: &Track, rate: u32, hq: bool) -> Option<[f32; 2]> {
        if let Some(audio) = self.audio {
            if self.age >= self.gate {
                return None;
            }
            let sample = track.sample.as_ref()?;
            let ratio = sample.rate as f64 / rate as f64;
            let position = (self.started as i64 + self.age as i64 - audio.origin) as f64 * ratio
                + audio.offset;
            if position >= sample.frames.len() as f64 {
                return None;
            }
            let value = if hq {
                dsp::sinc_sample(&sample.frames, position, ratio)
            } else {
                dsp::lerp_sample(&sample.frames, position)
            };
            self.age += 1;
            return Some(value.map(|v| v * self.velocity));
        }
        let seconds = self.age as f64 / rate as f64;
        let release = (0.08 * rate as f64) as u64;
        if self.age >= self.gate + release {
            return None;
        }
        let env = (seconds / 0.005).min(1.)
            * if self.age < self.gate {
                1.
            } else {
                1. - (self.age - self.gate) as f64 / release as f64
            };
        let step = (self.frequency / rate as f64).min(0.45);
        let phase = self.phase[0];
        let sample = match track.instrument {
            Instrument::Kick => {
                let hz = 46. + 125. * (-seconds * 32.).exp();
                self.phase[0] = (phase + hz / rate as f64).fract();
                let v = (TAU * phase).sin() * (-seconds * 11.).exp()
                    + dsp::noise(&mut self.noise) * (-seconds * 180.).exp() * 0.12;
                [v as f32; 2]
            }
            Instrument::Snare => {
                self.phase[0] = (phase + 180. / rate as f64).fract();
                let noise = dsp::noise(&mut self.noise);
                self.low += 0.18 * (noise - self.low);
                let v =
                    ((noise - self.low) * 0.7 + (TAU * phase).sin() * 0.3) * (-seconds * 18.).exp();
                [v as f32; 2]
            }
            Instrument::Hat => {
                let noise = dsp::noise(&mut self.noise);
                self.low += 0.22 * (noise - self.low);
                let v = (noise - self.low) * (-seconds * 50.).exp();
                [v as f32; 2]
            }
            Instrument::Bass => {
                self.phase[0] = (phase + step).fract();
                let v = dsp::saw(phase, step) * 0.65 + dsp::square(self.phase[1], step * 0.5) * 0.2;
                self.phase[1] = (self.phase[1] + step * 0.5).fract();
                let cutoff = (700. + 2000. * (-seconds * 8.).exp()).min(rate as f64 * 0.4);
                self.low += (1. - (-TAU * cutoff / rate as f64).exp()) * (v - self.low);
                [self.low as f32; 2]
            }
            Instrument::Trio => {
                let mut v = 0.;
                for (i, detune) in [0.997, 1., 1.003].into_iter().enumerate() {
                    let dt = (step * detune).min(0.45);
                    v += dsp::saw(self.phase[i], dt) / 3.;
                    self.phase[i] = (self.phase[i] + dt).fract();
                }
                self.low += 0.18 * (v - self.low);
                [self.low as f32 * 0.6; 2]
            }
            Instrument::Fm => {
                let mut modulation = 0.;
                let ratios = [1., 2., 3., 1., 4., 2.];
                for i in (0..6).rev() {
                    modulation =
                        (TAU * self.phase[i] + modulation * 1.4 * self.fm_envelope[i]).sin();
                    self.fm_envelope[i] *= self.fm_decay[i];
                    self.phase[i] = (self.phase[i] + (step * ratios[i]).min(0.45)).fract();
                }
                [(modulation * (-seconds * 0.7).exp() * 0.6) as f32; 2]
            }
            Instrument::Sampler => {
                if let Some(sample) = &track.sample {
                    let ratio =
                        2f64.powf((self.key as f64 - 60.) / 12.) * sample.rate as f64 / rate as f64;
                    let position = self.age as f64 * ratio;
                    if position >= sample.frames.len() as f64 {
                        return None;
                    }
                    if hq {
                        dsp::sinc_sample(&sample.frames, position, ratio)
                    } else {
                        dsp::lerp_sample(&sample.frames, position)
                    }
                } else {
                    return None;
                }
            }
        };
        self.age += 1;
        Some([
            sample[0] * (env as f32) * self.velocity * self.panning[0],
            sample[1] * (env as f32) * self.velocity * self.panning[1],
        ])
    }
}

pub struct Engine {
    project: Project,
    events: Vec<Event>,
    next_event: usize,
    voices: Vec<Voice>,
    effects: Vec<Vec<Processor>>,
    cursor: u64,
    length: u64,
    musical_length: u64,
    guard: OutputGuard,
    pub stolen_voices: u64,
    pub peak_voices: usize,
    hq: bool,
    streaming: bool,
    graph: crate::routing::Graph,
    clicks: Vec<(u64, bool)>,
    click_index: usize,
    click_age: Option<(u32, bool)>,
}

impl Engine {
    pub fn new(project: &Project, hq: bool, tail_seconds: f64) -> Result<Self> {
        Self::compile(project, hq, tail_seconds, false)
    }
    pub fn streaming(project: &Project) -> Result<Self> {
        Self::compile(project, false, 0., true)
    }
    fn compile(project: &Project, hq: bool, tail_seconds: f64, streaming: bool) -> Result<Self> {
        project.validate()?;
        let graph = crate::routing::Graph::compile(project)?;
        if !tail_seconds.is_finite() || !(0. ..=5.).contains(&tail_seconds) {
            return Err(Error::Invalid("Cola de audio no válida"));
        }
        let duration = project.seconds_at(project.bars as f64 * BAR as f64);
        if !streaming && duration + tail_seconds > 120. {
            return Err(Error::Limit("Máximo 120 segundos por render"));
        }
        let musical_length = (duration * project.rate as f64).ceil() as u64;
        let length = ((duration + tail_seconds) * project.rate as f64).ceil() as u64;
        if !streaming && length * 8 > 96 * 1024 * 1024 {
            return Err(Error::Limit("Render: máximo 96 MB"));
        }
        let mut delay_frames = 0usize;
        let effect_count = project
            .tracks
            .iter()
            .map(|t| t.effects.len())
            .sum::<usize>();
        for effect in project.tracks.iter().flat_map(|t| &t.effects) {
            delay_frames += match effect {
                Effect::Delay { seconds, .. } => (*seconds * project.rate as f32).ceil() as usize,
                Effect::Reverb { .. } => (0.152 * project.rate as f64).ceil() as usize,
                Effect::Chorus { .. } => (0.04 * project.rate as f64).ceil() as usize,
                _ => 0,
            };
        }
        let cost = (effect_count + project.tracks.len() * 2 + project.routes.len()) as u64;
        if delay_frames > 4_000_000
            || (!streaming && length * cost > 160_000_000)
            || (streaming && project.rate as u64 * cost > 16_000_000)
        {
            return Err(Error::Limit("El proyecto supera el presupuesto de efectos"));
        }
        let mut events = Vec::new();
        let mut work = 0u64;
        let solo = project.tracks.iter().any(|t| t.solo);
        let audible = project
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !track.mute && (!solo || track.solo))
            .fold(0u32, |mask, (i, _)| mask | 1u32 << i);
        let channel_masks: Vec<_> = project
            .patterns
            .iter()
            .map(|pattern| {
                pattern
                    .notes
                    .iter()
                    .fold((0u32, false), |(mask, legacy), note| match note.channel {
                        Some(channel) => (mask | 1u32 << channel.index(), legacy),
                        None => (mask, true),
                    })
            })
            .collect();
        let mut scheduled = 0usize;
        for clip in &project.clips {
            if clip.muted || project.arrangement_mutes & (1u32 << clip.track) != 0 {
                continue;
            }
            let pattern = &project.patterns[clip.pattern];
            if pattern.notes.is_empty() {
                continue;
            }
            let (mask, legacy) = channel_masks[clip.pattern];
            if (mask | if legacy { 1u32 << clip.track } else { 0 }) & audible == 0 {
                continue;
            }
            let repeats = (clip.length + clip.offset).div_ceil(pattern.length);
            scheduled = scheduled
                .checked_add(repeats as usize * pattern.notes.len())
                .ok_or(Error::Limit("Demasiados eventos en el arreglo"))?;
            if scheduled > 131_072 {
                return Err(Error::Limit("Demasiados eventos en el arreglo"));
            }
            for repeat in 0..repeats {
                let base = clip.tick as i64 + (repeat * pattern.length) as i64 - clip.offset as i64;
                for note in &pattern.notes {
                    let channel = note.channel_or(clip.track);
                    let track = &project.tracks[channel];
                    if track.mute || (solo && !track.solo) {
                        continue;
                    }
                    let tick = base + note.tick as i64;
                    if tick < clip.tick as i64 || tick >= (clip.tick + clip.length) as i64 {
                        continue;
                    }
                    let tick = tick as u32;
                    let swing = if (note.tick / (PPQ / 4)) % 2 == 1 {
                        project.swing as f64 * (PPQ / 4) as f64
                    } else {
                        0.
                    };
                    let at_tick = (tick as f64 + swing).min((clip.tick + clip.length) as f64);
                    let end_tick = (tick + note.length).min(clip.tick + clip.length) as f64;
                    if end_tick <= at_tick {
                        continue;
                    }
                    let at = (project.seconds_at(at_tick) * project.rate as f64).round() as u64;
                    let end = (project.seconds_at(end_tick) * project.rate as f64).round() as u64;
                    if end <= at {
                        continue;
                    }
                    work += end - at + (project.rate as u64 * 8 / 100);
                    if !streaming && work > MAX_VOICE_FRAMES {
                        return Err(Error::Limit("Proyecto demasiado denso para este render"));
                    }
                    events.push(Event {
                        at,
                        end,
                        track: channel,
                        note: *note,
                        audio: None,
                    });
                }
            }
        }
        for clip in &project.audio_clips {
            let channel = clip.channel.index();
            if clip.muted
                || project.arrangement_mutes & (1u32 << clip.track) != 0
                || audible & (1u32 << channel) == 0
            {
                continue;
            }
            let at = (project.seconds_at(clip.tick as f64) * project.rate as f64).round() as u64;
            let end = (project.seconds_at((clip.tick + clip.length) as f64) * project.rate as f64)
                .round() as u64;
            work += end - at;
            if (!streaming && work > MAX_VOICE_FRAMES) || events.len() >= 131_072 {
                return Err(Error::Limit("Proyecto demasiado denso para este render"));
            }
            if end > at {
                events.push(Event {
                    at,
                    end,
                    track: channel,
                    note: Note::new(0, 1, 60, clip.gain),
                    audio: Some(AudioPlayback {
                        origin: (project.seconds_at(clip.origin as f64) * project.rate as f64)
                            .round() as i64,
                        offset: clip.offset,
                    }),
                });
            }
        }
        events.sort_by_key(|e| (e.at, e.track, e.note.key));
        let effects = project
            .tracks
            .iter()
            .map(|t| {
                t.effects
                    .iter()
                    .map(|fx| Processor::new(fx, project.rate))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            project: project.clone(),
            graph,
            clicks: Vec::new(),
            click_index: 0,
            click_age: None,
            events,
            next_event: 0,
            voices: Vec::with_capacity(MAX_VOICES),
            effects,
            cursor: 0,
            length,
            musical_length,
            guard: OutputGuard::new(project.rate),
            stolen_voices: 0,
            peak_voices: 0,
            hq,
            streaming,
        })
    }

    pub fn frames(&self) -> u64 {
        self.length
    }
    pub fn musical_frames(&self) -> u64 {
        self.musical_length
    }
    pub fn position(&self) -> u64 {
        self.cursor
    }

    pub fn with_metronome(mut self) -> Self {
        self.clicks = (0..self.project.bars as u32 * 4)
            .map(|beat| {
                (
                    (self.project.seconds_at((beat * PPQ) as f64) * self.project.rate as f64)
                        .round() as u64,
                    beat % 4 == 0,
                )
            })
            .collect();
        self
    }

    pub fn process(&mut self, output: &mut [[f32; 2]]) -> usize {
        let count = output
            .len()
            .min(self.length.saturating_sub(self.cursor) as usize);
        let base_pan: [[f32; 2]; MAX_TRACKS] = std::array::from_fn(|i| {
            self.project
                .tracks
                .get(i)
                .map_or([0.; 2], |t| dsp::pan(t.pan))
        });
        for frame in output.iter_mut().take(count) {
            self.voices.retain(|v| v.audio.is_none() || v.age < v.gate);
            while self.next_event < self.events.len()
                && self.events[self.next_event].at <= self.cursor
            {
                if self.voices.len() == MAX_VOICES {
                    self.voices.remove(0);
                    self.stolen_voices += 1;
                }
                self.voices
                    .push(Voice::new(&self.events[self.next_event], self.project.rate));
                self.next_event += 1;
            }
            self.peak_voices = self.peak_voices.max(self.voices.len());
            let mut channels = [[0f32; 2]; MAX_TRACKS];
            self.voices.retain_mut(|voice| {
                if let Some(sample) = voice.next(
                    &self.project.tracks[voice.track],
                    self.project.rate,
                    self.hq,
                ) {
                    channels[voice.track][0] += sample[0];
                    channels[voice.track][1] += sample[1];
                    true
                } else {
                    false
                }
            });
            let mut gains = [1f32; MAX_TRACKS];
            let mut pans = base_pan;
            let mut master = self.project.master;
            if !self.project.automation.is_empty() {
                let tick = self
                    .project
                    .tick_at(self.cursor as f64 / self.project.rate as f64);
                for lane in &self.project.automation {
                    let value = lane.at(tick);
                    match lane.target {
                        Target::Master => master = value,
                        Target::Gain(i) => gains[i] = value,
                        Target::Pan(i) => pans[i] = dsp::pan(value),
                    }
                }
            }
            let mut sum = [0.; 2];
            for &i in &self.graph.order {
                let track = &self.project.tracks[i];
                let mut value = channels[i].map(|v| {
                    if v.is_finite() {
                        v.clamp(-16., 16.)
                    } else {
                        0.
                    }
                });
                for fx in &mut self.effects[i] {
                    value = fx.process(value);
                }
                if track.mute || !self.graph.audible[i] {
                    continue;
                }
                let gain = track.gain * gains[i];
                value = [
                    (value[0] * gain * pans[i][0]).clamp(-16., 16.),
                    (value[1] * gain * pans[i][1]).clamp(-16., 16.),
                ];
                for route in &self.graph.outputs[i] {
                    let sign = if route.invert { -1. } else { 1. };
                    let pair = if route.swap {
                        [value[1], value[0]]
                    } else {
                        value
                    };
                    let destination = if let Some(to) = route.to {
                        &mut channels[to]
                    } else {
                        &mut sum
                    };
                    destination[0] += pair[0] * route.gain * sign;
                    destination[1] += pair[1] * route.gain * sign;
                }
            }
            while self.click_index < self.clicks.len()
                && self.clicks[self.click_index].0 <= self.cursor
            {
                self.click_age = Some((0, self.clicks[self.click_index].1));
                self.click_index += 1;
            }
            if let Some((age, accent)) = self.click_age {
                let seconds = age as f64 / self.project.rate as f64;
                if seconds < 0.02 {
                    let v = ((seconds * TAU * if accent { 1600. } else { 1100. }).sin()
                        * (-seconds / 0.003).exp()
                        * 0.08) as f32;
                    sum[0] += v;
                    sum[1] += v;
                    self.click_age = Some((age + 1, accent));
                } else {
                    self.click_age = None;
                }
            }
            *frame = self.guard.process([sum[0] * master, sum[1] * master]);
            self.cursor += 1;
        }
        output[count..].fill([0.; 2]);
        count
    }

    pub fn render(mut self, cancel: &AtomicBool) -> Result<Render> {
        if self.streaming {
            return Err(Error::Unsupported(
                "El motor por bloques no reserva canciones completas; usa Engine::new para renderizar",
            ));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Render cancelado"));
        }
        let mut audio = vec![[0.; 2]; self.length as usize];
        for block in audio.chunks_mut(512) {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Invalid("Render cancelado"));
            }
            self.process(block);
        }
        let mut peak = 0f32;
        let mut squares = 0f64;
        for frame in &audio {
            for sample in frame {
                peak = peak.max(sample.abs());
                squares += (*sample as f64).powi(2);
            }
        }
        let rms = (squares / (audio.len().max(1) * 2) as f64).sqrt() as f32;
        Ok(Render {
            audio,
            rate: self.project.rate,
            peak,
            rms,
            peak_voices: self.peak_voices,
            stolen_voices: self.stolen_voices,
            musical_frames: self.musical_length as usize,
        })
    }
}

pub struct Render {
    pub audio: Vec<[f32; 2]>,
    pub rate: u32,
    pub peak: f32,
    pub rms: f32,
    pub peak_voices: usize,
    pub stolen_voices: u64,
    pub musical_frames: usize,
}

pub struct InstrumentVoice {
    voice: Voice,
    rate: u32,
}
impl InstrumentVoice {
    pub fn new(key: u8, velocity: f32, rate: u32) -> Result<Self> {
        if key > 127 || !(8_000..=192_000).contains(&rate) {
            return Err(Error::Invalid("Voz no válida"));
        }
        crate::model::finite(velocity, 0., 1.)?;
        let event = Event {
            at: 0,
            end: rate as u64 * 120,
            track: 0,
            note: Note::new(0, 1, key, velocity),
            audio: None,
        };
        Ok(Self {
            voice: Voice::new(&event, rate),
            rate,
        })
    }
    pub fn key(&self) -> u8 {
        self.voice.key
    }
    pub fn release(&mut self) {
        self.voice.gate = self.voice.age;
    }
    pub fn frame(&mut self, track: &Track) -> Option<[f32; 2]> {
        self.voice.next(track, self.rate, false)
    }
}
