use crate::{Error, Result};
use std::sync::Arc;

pub const PPQ: u32 = 960;
pub const BAR: u32 = PPQ * 4;
pub const MAX_TRACKS: usize = 32;
pub const MAX_LANES: usize = 32;
pub const MAX_NOTES: usize = 32_768;
pub const MAX_FILE: usize = 64 * 1024 * 1024;
pub const MAX_TICKS: u32 = BAR * 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Instrument {
    Kick,
    Snare,
    Hat,
    Bass,
    Trio,
    Fm,
    Sampler,
}

impl Instrument {
    pub const ALL: [Self; 7] = [
        Self::Kick,
        Self::Snare,
        Self::Hat,
        Self::Bass,
        Self::Trio,
        Self::Fm,
        Self::Sampler,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Kick => "Kick",
            Self::Snare => "Snare",
            Self::Hat => "Hi-hat",
            Self::Bass => "Bass",
            Self::Trio => "Trio",
            Self::Fm => "FM six",
            Self::Sampler => "Sampler",
        }
    }
    pub fn from_byte(v: u8) -> Result<Self> {
        Self::ALL
            .get(v as usize)
            .copied()
            .ok_or(Error::Invalid("Instrumento desconocido"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub rate: u32,
    pub frames: Arc<Vec<[f32; 2]>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Eq {
        hz: f32,
        db: f32,
        q: f32,
    },
    Delay {
        seconds: f32,
        feedback: f32,
        mix: f32,
    },
    Reverb {
        decay: f32,
        mix: f32,
    },
    Chorus {
        rate: f32,
        depth: f32,
        mix: f32,
    },
    Drive {
        drive: f32,
        mix: f32,
    },
    Width(f32),
    Compressor {
        threshold: f32,
        ratio: f32,
    },
}

impl Effect {
    pub fn validate(&self) -> Result<()> {
        match *self {
            Self::Eq { hz, db, q } => {
                finite(hz, 20., 20_000.)?;
                finite(db, -24., 24.)?;
                finite(q, 0.1, 12.)?;
            }
            Self::Delay {
                seconds,
                feedback,
                mix,
            } => {
                finite(seconds, 0.001, 2.)?;
                finite(feedback, 0., 0.9)?;
                finite(mix, 0., 1.)?;
            }
            Self::Reverb { decay, mix } => {
                finite(decay, 0., 0.9)?;
                finite(mix, 0., 1.)?;
            }
            Self::Chorus { rate, depth, mix } => {
                finite(rate, 0.05, 8.)?;
                finite(depth, 0., 1.)?;
                finite(mix, 0., 1.)?;
            }
            Self::Drive { drive, mix } => {
                finite(drive, 1., 20.)?;
                finite(mix, 0., 1.)?;
            }
            Self::Width(v) => finite(v, 0., 2.)?,
            Self::Compressor { threshold, ratio } => {
                finite(threshold, 0.01, 1.)?;
                finite(ratio, 1., 20.)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: String,
    pub instrument: Instrument,
    pub gain: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub effects: Vec<Effect>,
    pub sample: Option<Sample>,
}

impl Track {
    pub fn new(name: &str, instrument: Instrument) -> Self {
        Self {
            name: name.into(),
            instrument,
            gain: 0.65,
            pan: 0.,
            mute: false,
            solo: false,
            effects: Vec::new(),
            sample: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ChannelId(std::num::NonZeroU8);
impl ChannelId {
    pub fn new(index: usize) -> Result<Self> {
        if index >= MAX_TRACKS {
            return Err(Error::Invalid("Canal fuera de rango"));
        }
        std::num::NonZeroU8::new((index + 1) as u8)
            .map(Self)
            .ok_or(Error::Invalid("Canal fuera de rango"))
    }
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
    pub(crate) fn encoded(self) -> u8 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SourceNoteId(std::num::NonZeroU16);
impl SourceNoteId {
    pub fn new(index: usize) -> Result<Self> {
        if index >= MAX_NOTES {
            return Err(Error::Invalid("Índice de nota fuente fuera de rango"));
        }
        std::num::NonZeroU16::new((index + 1) as u16)
            .map(Self)
            .ok_or(Error::Invalid("Índice de nota fuente fuera de rango"))
    }
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
    pub(crate) fn encoded(self) -> u16 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub channel: Option<ChannelId>,
    pub source_id: Option<SourceNoteId>,
    pub tick: u32,
    pub length: u32,
    pub key: u8,
    pub velocity: f32,
    pub pan: f32,
}
impl Note {
    pub fn new(tick: u32, length: u32, key: u8, velocity: f32) -> Self {
        Self {
            channel: None,
            source_id: None,
            tick,
            length,
            key,
            velocity,
            pan: 0.,
        }
    }
    pub fn on_channel(mut self, channel: usize) -> Result<Self> {
        self.channel = Some(ChannelId::new(channel)?);
        Ok(self)
    }
    pub fn channel_or(&self, fallback: usize) -> usize {
        self.channel.map_or(fallback, ChannelId::index)
    }
    pub fn validate(&self, end: u32) -> Result<()> {
        if self.key > 127
            || self.length == 0
            || self.tick >= end
            || self.tick.checked_add(self.length).is_none_or(|v| v > end)
        {
            return Err(Error::Invalid("Nota fuera del patrón"));
        }
        finite(self.velocity, 0., 1.)?;
        finite(self.pan, -1., 1.)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub length: u32,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    // Fila del arreglo; cada nota lleva su canal de sonido.
    pub track: usize,
    pub pattern: usize,
    pub tick: u32,
    pub length: u32,
    pub offset: u32,
    pub muted: bool,
    pub group: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClip {
    pub track: usize,
    pub channel: ChannelId,
    pub tick: u32,
    pub length: u32,
    pub origin: i32,
    pub offset: f64,
    pub gain: f32,
    pub muted: bool,
    pub group: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Curve {
    Hold,
    Linear,
    Smooth,
    Exponential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Master,
    Gain(usize),
    Pan(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Automation {
    pub target: Target,
    pub curve: Curve,
    pub points: Vec<(u32, f32)>,
}

impl Automation {
    pub fn at(&self, tick: f64) -> f32 {
        if self.points.is_empty() {
            return 0.;
        }
        let upper = self.points.partition_point(|(t, _)| *t as f64 <= tick);
        if upper == 0 {
            return self.points[0].1;
        }
        if upper == self.points.len() {
            return self.points[upper - 1].1;
        }
        let (a, x) = self.points[upper - 1];
        let (b, y) = self.points[upper];
        let t = ((tick - a as f64) / (b - a) as f64).clamp(0., 1.) as f32;
        let t = match self.curve {
            Curve::Hold => 0.,
            Curve::Linear => t,
            Curve::Smooth => t * t * (3. - 2. * t),
            Curve::Exponential => t * t,
        };
        x + (y - x) * t
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub arrangement_mutes: u32,
    pub routes: Vec<crate::routing::Route>,
    pub source_flp: Option<Arc<Vec<u8>>>,
    pub title: String,
    pub bpm: f64,
    pub rate: u32,
    pub bars: u16,
    pub master: f32,
    pub swing: f32,
    pub tracks: Vec<Track>,
    pub patterns: Vec<Pattern>,
    pub clips: Vec<Clip>,
    pub audio_clips: Vec<AudioClip>,
    pub automation: Vec<Automation>,
    pub tempos: Vec<(u32, f64)>,
    pub markers: Vec<(u32, String)>,
}

pub fn finite(v: f32, min: f32, max: f32) -> Result<()> {
    if !v.is_finite() || !(min..=max).contains(&v) {
        Err(Error::Invalid("Parámetro fuera de rango"))
    } else {
        Ok(())
    }
}

pub fn text(s: &str) -> Result<()> {
    if s.len() > 256 || s.chars().any(char::is_control) {
        Err(Error::Invalid("Texto no válido"))
    } else {
        Ok(())
    }
}

impl Project {
    pub fn empty() -> Self {
        Self {
            arrangement_mutes: 0,
            routes: Vec::new(),
            source_flp: None,
            title: "Sin título".into(),
            bpm: 120.,
            rate: 48_000,
            bars: 4,
            master: 0.65,
            swing: 0.,
            tracks: Vec::new(),
            patterns: Vec::new(),
            clips: Vec::new(),
            audio_clips: Vec::new(),
            automation: Vec::new(),
            tempos: Vec::new(),
            markers: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self
            .source_flp
            .as_ref()
            .is_some_and(|s| s.len() > 32 * 1024 * 1024)
        {
            return Err(Error::Limit("FLP original demasiado grande"));
        }
        crate::routing::Graph::compile(self)?;
        text(&self.title)?;
        if !self.bpm.is_finite()
            || !(20. ..=300.).contains(&self.bpm)
            || !(8_000..=192_000).contains(&self.rate)
            || !(1..=256).contains(&self.bars)
        {
            return Err(Error::Invalid("Tempo o duración no válidos"));
        }
        finite(self.master, 0., 1.)?;
        finite(self.swing, 0., 0.75)?;
        if self.tracks.len() > MAX_TRACKS
            || self.patterns.len() > 256
            || self.clips.len().saturating_add(self.audio_clips.len()) > 4096
            || self.automation.len() > 64
            || self.markers.len() > 1024
            || self.tempos.len() > 1024
        {
            return Err(Error::Limit("Proyecto demasiado grande"));
        }
        let mut frames = 0usize;
        for t in &self.tracks {
            text(&t.name)?;
            finite(t.gain, 0., 2.)?;
            finite(t.pan, -1., 1.)?;
            if t.effects.len() > 8 {
                return Err(Error::Limit("Máximo ocho efectos por canal"));
            }
            for fx in &t.effects {
                fx.validate()?;
            }
            if let Some(s) = &t.sample {
                if !(8_000..=192_000).contains(&s.rate) {
                    return Err(Error::Invalid("Frecuencia de muestra no válida"));
                }
                frames = frames
                    .checked_add(s.frames.len())
                    .ok_or(Error::Limit("Muestras demasiado grandes"))?;
                if frames > 4_000_000 {
                    return Err(Error::Limit("Muestras: máximo 32 MB por proyecto"));
                }
                if s.frames
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 16.)
                {
                    return Err(Error::Invalid("Audio no finito o fuera de rango"));
                }
            }
        }
        let mut count = 0;
        for p in &self.patterns {
            text(&p.name)?;
            if p.length == 0 || p.length > MAX_TICKS {
                return Err(Error::Invalid("Longitud de patrón no válida"));
            }
            count += p.notes.len();
            if count > MAX_NOTES {
                return Err(Error::Limit("Demasiadas notas"));
            }
            for n in &p.notes {
                n.validate(p.length)?;
                if n.channel
                    .is_some_and(|channel| channel.index() >= self.tracks.len())
                {
                    return Err(Error::Invalid("Canal de nota no válido"));
                }
            }
        }
        let end = self.bars as u32 * BAR;
        for c in &self.clips {
            if c.track >= MAX_LANES
                || c.pattern >= self.patterns.len()
                || c.length == 0
                || c.offset >= self.patterns[c.pattern].length
                || c.tick.checked_add(c.length).is_none_or(|v| v > end)
            {
                return Err(Error::Invalid("Clip fuera del arreglo"));
            }
            if c.track >= self.tracks.len()
                && self.patterns[c.pattern]
                    .notes
                    .iter()
                    .any(|note| note.channel.is_none())
            {
                return Err(Error::Invalid("Clip antiguo sin canal de sonido"));
            }
        }
        for c in &self.audio_clips {
            if c.track >= MAX_LANES
                || c.channel.index() >= self.tracks.len()
                || c.length == 0
                || c.tick.checked_add(c.length).is_none_or(|v| v > end)
                || !(-((MAX_TICKS) as i32)..=c.tick as i32).contains(&c.origin)
                || !c.offset.is_finite()
                || c.offset < 0.
            {
                return Err(Error::Invalid("Clip de audio fuera del arreglo"));
            }
            let sample = self.tracks[c.channel.index()]
                .sample
                .as_ref()
                .ok_or(Error::Invalid("Clip de audio sin muestra"))?;
            let position = c.offset
                + (self.seconds_at(c.tick as f64) - self.seconds_at(c.origin as f64))
                    * sample.rate as f64;
            if position >= sample.frames.len() as f64 {
                return Err(Error::Invalid("El clip empieza después de su audio"));
            }
            finite(c.gain, 0., 2.)?;
        }
        let mut targets = Vec::new();
        for a in &self.automation {
            let pan = matches!(a.target, Target::Pan(_));
            if let Target::Gain(i) | Target::Pan(i) = a.target
                && i >= self.tracks.len()
            {
                return Err(Error::Invalid("Canal de automatización no válido"));
            }
            if targets.contains(&a.target) {
                return Err(Error::Invalid("Automatización duplicada"));
            }
            targets.push(a.target);
            if a.points.is_empty() || a.points.len() > 4096 {
                return Err(Error::Invalid("Puntos de automatización no válidos"));
            }
            for (i, &(tick, value)) in a.points.iter().enumerate() {
                finite(
                    value,
                    if pan { -1. } else { 0. },
                    if matches!(a.target, Target::Gain(_)) {
                        2.
                    } else {
                        1.
                    },
                )?;
                if tick > end || (i > 0 && a.points[i - 1].0 >= tick) {
                    return Err(Error::Invalid("Automatización desordenada"));
                }
            }
        }
        for (i, &(tick, bpm)) in self.tempos.iter().enumerate() {
            if tick > end
                || !bpm.is_finite()
                || !(20. ..=300.).contains(&bpm)
                || (i > 0 && self.tempos[i - 1].0 >= tick)
            {
                return Err(Error::Invalid("Mapa de tempo no válido"));
            }
        }
        for (tick, name) in &self.markers {
            if *tick > end {
                return Err(Error::Invalid("Marcador fuera del arreglo"));
            }
            text(name)?;
        }
        Ok(())
    }

    pub fn seconds_at(&self, tick: f64) -> f64 {
        let mut at = 0.;
        let mut seconds = 0.;
        let mut bpm = self.bpm;
        for &(t, next) in &self.tempos {
            let t = t as f64;
            if t >= tick {
                break;
            }
            seconds += (t - at) * 60. / (bpm * PPQ as f64);
            at = t;
            bpm = next;
        }
        seconds + (tick - at) * 60. / (bpm * PPQ as f64)
    }

    pub fn tick_at(&self, seconds: f64) -> f64 {
        let mut at = 0.;
        let mut remaining = seconds.max(0.);
        let mut bpm = self.bpm;
        for &(tick, next) in &self.tempos {
            let duration = (tick as f64 - at) * 60. / (bpm * PPQ as f64);
            if remaining < duration {
                break;
            }
            remaining -= duration;
            at = tick as f64;
            bpm = next;
        }
        at + remaining * bpm * PPQ as f64 / 60.
    }

    pub fn with_bound_channels(&self) -> Result<Self> {
        self.validate()?;
        let mut next = self.clone();
        let mut bindings = std::collections::BTreeMap::new();
        let mut count = self.patterns.iter().map(|p| p.notes.len()).sum::<usize>();
        for (index, pattern) in self.patterns.iter().enumerate() {
            if pattern.notes.iter().all(|note| note.channel.is_some()) {
                continue;
            }
            let fallback = self
                .clips
                .iter()
                .find(|c| c.pattern == index)
                .map_or(index.min(self.tracks.len().saturating_sub(1)), |c| c.track);
            if !pattern.notes.is_empty() && fallback >= self.tracks.len() {
                return Err(Error::Invalid("El patrón antiguo necesita un canal"));
            }
            for note in &mut next.patterns[index].notes {
                if note.channel.is_none() {
                    *note = note.on_channel(fallback)?;
                }
            }
            bindings.insert((index, fallback), index);
        }
        for (index, clip) in self.clips.iter().enumerate() {
            let pattern = &self.patterns[clip.pattern];
            if pattern.notes.iter().all(|note| note.channel.is_some()) {
                continue;
            }
            let key = (clip.pattern, clip.track);
            let binding = if let Some(binding) = bindings.get(&key) {
                *binding
            } else {
                count = count
                    .checked_add(pattern.notes.len())
                    .ok_or(Error::Limit("Demasiadas notas al migrar canales"))?;
                if count > MAX_NOTES || next.patterns.len() >= 256 {
                    return Err(Error::Limit(
                        "La sesión supera el límite al migrar sus canales",
                    ));
                }
                let mut copy = pattern.clone();
                for note in &mut copy.notes {
                    if note.channel.is_none() {
                        *note = note.on_channel(clip.track)?;
                    }
                }
                let binding = next.patterns.len();
                next.patterns.push(copy);
                bindings.insert(key, binding);
                binding
            };
            next.clips[index].pattern = binding;
        }
        next.validate()?;
        Ok(next)
    }
    pub fn add_channel(&mut self, instrument: Instrument) -> Result<usize> {
        if self.tracks.len() >= MAX_TRACKS {
            return Err(Error::Limit("Máximo 32 canales"));
        }
        let index = self.tracks.len();
        self.tracks.push(Track::new(instrument.name(), instrument));
        Ok(index)
    }
    pub fn add_track(&mut self, instrument: Instrument) -> Result<usize> {
        if self.tracks.len() >= MAX_TRACKS
            || self.patterns.len() >= 256
            || self.clips.len() + self.audio_clips.len() >= 4096
        {
            return Err(Error::Limit("Máximo 32 canales"));
        }
        let i = self.tracks.len();
        let pattern = self.patterns.len();
        self.tracks.push(Track::new(instrument.name(), instrument));
        self.patterns.push(Pattern {
            name: format!("{} 1", instrument.name()),
            length: BAR,
            notes: Vec::new(),
        });
        self.clips.push(Clip {
            track: i,
            pattern,
            tick: 0,
            length: self.bars as u32 * BAR,
            offset: 0,
            muted: false,
            group: 0,
        });
        Ok(i)
    }

    pub fn starter() -> Self {
        let mut p = Self::empty();
        p.title = "[untitled]".into();
        p.bpm = 130.;
        p.add_track(Instrument::Sampler)
            .expect("proyecto inicial acotado");
        p.tracks[0].name = "Garrido Sampler".into();
        p.patterns[0].name = "Pattern 1".into();
        p.clips.clear();
        p
    }

    pub fn demo() -> Self {
        let mut p = Self::empty();
        p.title = "Primer pulso".into();
        p.bpm = 118.;
        for kind in [
            Instrument::Kick,
            Instrument::Snare,
            Instrument::Hat,
            Instrument::Bass,
            Instrument::Trio,
            Instrument::Fm,
        ] {
            p.add_track(kind).expect("demo acotada");
        }
        for tick in [0, 960, 1920, 2880] {
            p.patterns[0].notes.push(Note::new(tick, 200, 36, 0.85));
        }
        for tick in [960, 2880] {
            p.patterns[1].notes.push(Note::new(tick, 240, 38, 0.55));
        }
        for step in 0..8 {
            p.patterns[2].notes.push(Note::new(
                step * 480,
                120,
                42,
                if step % 2 == 0 { 0.34 } else { 0.24 },
            ));
        }
        for (i, key) in [36, 36, 43, 39, 36, 46, 43, 39].into_iter().enumerate() {
            p.patterns[3]
                .notes
                .push(Note::new(i as u32 * 480, 360, key, 0.7));
        }
        for key in [60, 63, 67] {
            p.patterns[4].notes.push(Note::new(0, BAR - 240, key, 0.18));
        }
        for (i, key) in [72, 79, 75, 70].into_iter().enumerate() {
            p.patterns[5]
                .notes
                .push(Note::new(i as u32 * 960 + 480, 400, key, 0.28));
        }
        p.tracks[3].effects.push(Effect::Eq {
            hz: 180.,
            db: -3.,
            q: 0.7,
        });
        p.tracks[4].effects.push(Effect::Reverb {
            decay: 0.6,
            mix: 0.2,
        });
        p.tracks[5].effects.push(Effect::Delay {
            seconds: 0.254,
            feedback: 0.32,
            mix: 0.24,
        });
        p.tracks[4].pan = -0.2;
        p.tracks[5].pan = 0.25;
        p.markers.push((0, "Inicio".into()));
        p
    }
}
