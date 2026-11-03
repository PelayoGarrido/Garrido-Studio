use crate::{Error, Result, model::*, wav};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Wav,
    Flac,
    Aiff,
}
impl Format {
    pub fn from_extension(extension: &str) -> Result<Self> {
        if extension.eq_ignore_ascii_case("wav") {
            Ok(Self::Wav)
        } else if extension.eq_ignore_ascii_case("flac") {
            Ok(Self::Flac)
        } else if extension.eq_ignore_ascii_case("aif") || extension.eq_ignore_ascii_case("aiff") {
            Ok(Self::Aiff)
        } else {
            Err(Error::Unsupported("Elige WAV, FLAC o AIFF"))
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Aiff => "aiff",
        }
    }
    pub fn encode(
        self,
        audio: &[[f32; 2]],
        settings: Settings,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>> {
        settings.validate()?;
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Exportación cancelada"));
        }
        match self {
            Self::Wav => encode(audio, settings),
            Self::Aiff => crate::aiff::encode_cancel(
                audio,
                settings.rate,
                settings.depth,
                settings.channels,
                true,
                cancel,
            ),
            Self::Flac => {
                let depth = match settings.depth {
                    wav::Depth::Pcm16 => 16,
                    wav::Depth::Pcm24 => 24,
                    _ => return Err(Error::Unsupported("FLAC usa PCM16 o PCM24")),
                };
                crate::flac::encode_cancel(audio, settings.rate, depth, settings.channels, cancel)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Scope {
    Mix,
    Track(usize),
    Pattern { track: usize, pattern: usize },
}
#[derive(Clone, Copy)]
pub struct Settings {
    pub rate: u32,
    pub channels: u16,
    pub depth: wav::Depth,
    pub tail: f64,
    pub scope: Scope,
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(8000..=192000).contains(&self.rate)
            || !(1..=2).contains(&self.channels)
            || !self.tail.is_finite()
            || !(0. ..=5.).contains(&self.tail)
        {
            return Err(Error::Invalid("Ajustes de exportación no válidos"));
        }
        Ok(())
    }
}
pub fn prepare(project: &Project, settings: Settings) -> Result<Project> {
    project.validate()?;
    settings.validate()?;
    let mut p = project.clone();
    p.rate = settings.rate;
    let track = match settings.scope {
        Scope::Mix => None,
        Scope::Track(i) => Some(i),
        Scope::Pattern { track, pattern } => {
            let p = project
                .patterns
                .get(pattern)
                .ok_or(Error::Invalid("Patrón de exportación no válido"))?;
            if p.notes.iter().any(|note| note.channel.is_some()) {
                None
            } else {
                Some(track)
            }
        }
    };
    if let Some(i) = track {
        if i >= p.tracks.len() {
            return Err(Error::Invalid("Canal de exportación no válido"));
        }
        for (index, t) in p.tracks.iter_mut().enumerate() {
            t.solo = index == i;
            if index == i {
                t.mute = false;
            }
        }
    }
    if let Scope::Pattern { track, pattern } = settings.scope {
        p.audio_clips.clear();
        p.arrangement_mutes = 0;
        let length = p
            .patterns
            .get(pattern)
            .ok_or(Error::Invalid("Patrón de exportación no válido"))?
            .length;
        p.bars = length.div_ceil(BAR) as u16;
        p.clips = vec![Clip {
            offset: 0,
            muted: false,
            group: 0,
            track,
            pattern,
            tick: 0,
            length,
        }];
        let end = p.bars as u32 * BAR;
        p.tempos.retain(|(t, _)| *t <= end);
        p.markers.retain(|(t, _)| *t <= end);
        for a in &mut p.automation {
            a.points.retain(|(t, _)| *t <= end);
        }
        p.automation.retain(|a| !a.points.is_empty());
    }
    p.validate()?;
    Ok(p)
}
pub fn encode(audio: &[[f32; 2]], settings: Settings) -> Result<Vec<u8>> {
    settings.validate()?;
    wav::encode_channels(
        audio,
        settings.rate,
        settings.depth,
        true,
        settings.channels,
    )
}
