use crate::{Error, Result, model::*};
use std::sync::Arc;

pub struct Reader<'a> {
    data: &'a [u8],
    pub(crate) position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
    pub fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(size)
            .filter(|end| *end <= self.data.len())
            .ok_or(Error::Invalid("Archivo truncado"))?;
        let data = &self.data[self.position..end];
        self.position = end;
        Ok(data)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| Error::Invalid("Entero truncado"))?,
        ))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| Error::Invalid("Entero truncado"))?,
        ))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| Error::Invalid("Número truncado"))?,
        ))
    }
    pub fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n > max {
            Err(Error::Limit("Cantidad fuera del límite"))
        } else {
            Ok(n)
        }
    }
    pub fn string(&mut self) -> Result<String> {
        let len = self.count(256)?;
        let text = std::str::from_utf8(self.take(len)?)
            .map_err(|_| Error::Invalid("Texto UTF-8 no válido"))?;
        crate::model::text(text)?;
        Ok(text.into())
    }
    pub fn boolean(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Invalid("Booleano no válido")),
        }
    }
}

struct Writer(Vec<u8>);
impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend(v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend(v.to_le_bytes());
    }
    fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend(s.as_bytes());
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub fn encode(p: &Project) -> Result<Vec<u8>> {
    p.validate()?;
    let mut w = Writer(Vec::new());
    w.string(&p.title);
    w.f64(p.bpm);
    w.u32(p.rate);
    w.u16(p.bars);
    w.f32(p.master);
    w.f32(p.swing);
    w.u32(p.tracks.len() as u32);
    for t in &p.tracks {
        w.string(&t.name);
        w.u8(t.instrument as u8);
        w.f32(t.gain);
        w.f32(t.pan);
        w.u8(t.mute as u8);
        w.u8(t.solo as u8);
        w.u32(t.effects.len() as u32);
        for fx in &t.effects {
            match *fx {
                Effect::Eq { hz, db, q } => {
                    w.u8(0);
                    w.f32(hz);
                    w.f32(db);
                    w.f32(q);
                }
                Effect::Delay {
                    seconds,
                    feedback,
                    mix,
                } => {
                    w.u8(1);
                    w.f32(seconds);
                    w.f32(feedback);
                    w.f32(mix);
                }
                Effect::Reverb { decay, mix } => {
                    w.u8(2);
                    w.f32(decay);
                    w.f32(mix);
                }
                Effect::Chorus { rate, depth, mix } => {
                    w.u8(3);
                    w.f32(rate);
                    w.f32(depth);
                    w.f32(mix);
                }
                Effect::Drive { drive, mix } => {
                    w.u8(4);
                    w.f32(drive);
                    w.f32(mix);
                }
                Effect::Width(v) => {
                    w.u8(5);
                    w.f32(v);
                }
                Effect::Compressor { threshold, ratio } => {
                    w.u8(6);
                    w.f32(threshold);
                    w.f32(ratio);
                }
            }
        }
        w.u8(t.sample.is_some() as u8);
        if let Some(s) = &t.sample {
            w.u32(s.rate);
            w.u32(s.frames.len() as u32);
            for f in s.frames.iter() {
                w.f32(f[0]);
                w.f32(f[1]);
            }
        }
    }
    w.u32(p.patterns.len() as u32);
    for pattern in &p.patterns {
        w.string(&pattern.name);
        w.u32(pattern.length);
        w.u32(pattern.notes.len() as u32);
        for n in &pattern.notes {
            w.u32(n.tick);
            w.u32(n.length);
            w.u8(n.key);
            w.f32(n.velocity);
            w.f32(n.pan);
            w.u8(n.channel.map_or(0, ChannelId::encoded));
            w.u16(n.source_id.map_or(0, SourceNoteId::encoded));
        }
    }
    w.u32(p.clips.len() as u32);
    for c in &p.clips {
        w.u32(c.track as u32);
        w.u32(c.pattern as u32);
        w.u32(c.tick);
        w.u32(c.length);
        w.u32(c.offset);
        w.u8(c.muted as u8);
        w.u16(c.group);
    }
    w.u32(p.automation.len() as u32);
    for a in &p.automation {
        match a.target {
            Target::Master => w.u8(0),
            Target::Gain(i) => {
                w.u8(1);
                w.u32(i as u32);
            }
            Target::Pan(i) => {
                w.u8(2);
                w.u32(i as u32);
            }
        }
        w.u8(a.curve as u8);
        w.u32(a.points.len() as u32);
        for &(tick, value) in &a.points {
            w.u32(tick);
            w.f32(value);
        }
    }
    w.u32(p.tempos.len() as u32);
    for &(tick, bpm) in &p.tempos {
        w.u32(tick);
        w.f64(bpm);
    }
    w.u32(p.markers.len() as u32);
    for (tick, name) in &p.markers {
        w.u32(*tick);
        w.string(name);
    }
    if w.0.len() + 16 > MAX_FILE {
        return Err(Error::Limit("Archivo demasiado grande"));
    }
    w.u8(p.source_flp.is_some() as u8);
    if let Some(source) = &p.source_flp {
        w.u32(source.len() as u32);
        w.0.extend(source.iter());
    }
    w.u32(p.routes.len() as u32);
    for r in &p.routes {
        w.u32(r.from as u32);
        w.u32(r.to.map_or(u32::MAX, |i| i as u32));
        w.f32(r.gain);
        w.u8(r.invert as u8);
        w.u8(r.swap as u8);
    }
    w.u32(p.arrangement_mutes);
    w.u32(p.audio_clips.len() as u32);
    for c in &p.audio_clips {
        w.u32(c.track as u32);
        w.u8(c.channel.encoded());
        w.u32(c.tick);
        w.u32(c.length);
        w.u32(c.origin as u32);
        w.f64(c.offset);
        w.f32(c.gain);
        w.u8(c.muted as u8);
        w.u16(c.group);
    }
    if w.0.len() + 16 > MAX_FILE {
        return Err(Error::Limit("Proyecto demasiado grande"));
    }
    let mut result = b"GARRIDO\x05".to_vec();
    result.extend((w.0.len() as u32).to_le_bytes());
    result.extend(crc32(&w.0).to_le_bytes());
    result.extend(w.0);
    Ok(result)
}

pub fn decode(data: &[u8]) -> Result<Project> {
    if data.len() > MAX_FILE {
        return Err(Error::Limit("Archivo demasiado grande"));
    }
    let mut r = Reader::new(data);
    let magic = r.take(8)?;
    let audio = magic == b"GARRIDO\x05";
    let multichannel = magic == b"GARRIDO\x04" || audio;
    let routed = magic == b"GARRIDO\x03" || multichannel;
    let extended = magic == b"GARRIDO\x02" || routed;
    if magic != b"GARRIDO\x01" && !extended {
        return Err(Error::Unsupported(
            "Formato o versión de proyecto no compatible",
        ));
    }
    let size = r.count(MAX_FILE)?;
    let crc = r.u32()?;
    if size != r.remaining() || crc32(r.take(size)?) != crc {
        return Err(Error::Invalid("Proyecto incompleto o checksum incorrecto"));
    }
    let mut r = Reader::new(&data[16..]);
    let mut p = Project::empty();
    p.title = r.string()?;
    p.bpm = r.f64()?;
    p.rate = r.u32()?;
    p.bars = r.u16()?;
    p.master = r.f32()?;
    p.swing = r.f32()?;
    let mut sample_frames = 0;
    let tracks = r.count(MAX_TRACKS)?;
    for _ in 0..tracks {
        let name = r.string()?;
        let instrument = Instrument::from_byte(r.u8()?)?;
        let mut t = Track::new(&name, instrument);
        t.gain = r.f32()?;
        t.pan = r.f32()?;
        t.mute = r.boolean()?;
        t.solo = r.boolean()?;
        let effects = r.count(8)?;
        for _ in 0..effects {
            let effect = match r.u8()? {
                0 => Effect::Eq {
                    hz: r.f32()?,
                    db: r.f32()?,
                    q: r.f32()?,
                },
                1 => Effect::Delay {
                    seconds: r.f32()?,
                    feedback: r.f32()?,
                    mix: r.f32()?,
                },
                2 => Effect::Reverb {
                    decay: r.f32()?,
                    mix: r.f32()?,
                },
                3 => Effect::Chorus {
                    rate: r.f32()?,
                    depth: r.f32()?,
                    mix: r.f32()?,
                },
                4 => Effect::Drive {
                    drive: r.f32()?,
                    mix: r.f32()?,
                },
                5 => Effect::Width(r.f32()?),
                6 => Effect::Compressor {
                    threshold: r.f32()?,
                    ratio: r.f32()?,
                },
                _ => return Err(Error::Invalid("Efecto desconocido")),
            };
            effect.validate()?;
            t.effects.push(effect);
        }
        if r.boolean()? {
            let rate = r.u32()?;
            let count = r.count(4_000_000 - sample_frames)?;
            sample_frames += count;
            if count > r.remaining() / 8 {
                return Err(Error::Invalid("Muestra truncada"));
            }
            let mut frames = Vec::with_capacity(count);
            for _ in 0..count {
                frames.push([r.f32()?, r.f32()?]);
            }
            t.sample = Some(Sample {
                rate,
                frames: Arc::new(frames),
            });
        }
        p.tracks.push(t);
    }
    let patterns = r.count(256)?;
    let mut note_count = 0;
    for _ in 0..patterns {
        let name = r.string()?;
        let length = r.u32()?;
        let count = r.count(MAX_NOTES - note_count)?;
        note_count += count;
        if count > r.remaining() / if multichannel { 20 } else { 17 } {
            return Err(Error::Invalid("Notas truncadas"));
        }
        let mut notes = Vec::with_capacity(count);
        for _ in 0..count {
            notes.push(Note {
                tick: r.u32()?,
                length: r.u32()?,
                key: r.u8()?,
                velocity: r.f32()?,
                pan: r.f32()?,
                channel: if multichannel {
                    match r.u8()? {
                        0 => None,
                        channel => Some(ChannelId::new(channel as usize - 1)?),
                    }
                } else {
                    None
                },
                source_id: if multichannel {
                    match r.u16()? {
                        0 => None,
                        id => Some(SourceNoteId::new(id as usize - 1)?),
                    }
                } else {
                    None
                },
            });
        }
        p.patterns.push(Pattern {
            name,
            length,
            notes,
        });
    }
    let clips = r.count(4096)?;
    for _ in 0..clips {
        p.clips.push(Clip {
            track: r.u32()? as usize,
            pattern: r.u32()? as usize,
            tick: r.u32()?,
            length: r.u32()?,
            offset: if audio { r.u32()? } else { 0 },
            muted: audio && r.boolean()?,
            group: if audio { r.u16()? } else { 0 },
        });
    }
    let lanes = r.count(64)?;
    for _ in 0..lanes {
        let target = match r.u8()? {
            0 => Target::Master,
            1 => Target::Gain(r.u32()? as usize),
            2 => Target::Pan(r.u32()? as usize),
            _ => return Err(Error::Invalid("Destino desconocido")),
        };
        let curve = match r.u8()? {
            0 => Curve::Hold,
            1 => Curve::Linear,
            2 => Curve::Smooth,
            3 => Curve::Exponential,
            _ => return Err(Error::Invalid("Curva desconocida")),
        };
        let count = r.count(4096)?;
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            points.push((r.u32()?, r.f32()?));
        }
        p.automation.push(Automation {
            target,
            curve,
            points,
        });
    }
    let tempos = r.count(1024)?;
    for _ in 0..tempos {
        p.tempos.push((r.u32()?, r.f64()?));
    }
    let markers = r.count(1024)?;
    for _ in 0..markers {
        p.markers.push((r.u32()?, r.string()?));
    }
    if extended && r.boolean()? {
        let count = r.count(32 * 1024 * 1024)?;
        p.source_flp = Some(Arc::new(r.take(count)?.to_vec()));
    }
    if routed {
        let count = r.count(256)?;
        for _ in 0..count {
            let from = r.u32()? as usize;
            let to = r.u32()?;
            p.routes.push(crate::routing::Route {
                from,
                to: if to == u32::MAX {
                    None
                } else {
                    Some(to as usize)
                },
                gain: r.f32()?,
                invert: r.boolean()?,
                swap: r.boolean()?,
            });
        }
    }
    if multichannel {
        p.arrangement_mutes = r.u32()?;
    }
    if audio {
        let count = r.count(4096usize.saturating_sub(p.clips.len()))?;
        for _ in 0..count {
            p.audio_clips.push(AudioClip {
                track: r.u32()? as usize,
                channel: ChannelId::new(
                    r.u8()?
                        .checked_sub(1)
                        .ok_or(Error::Invalid("Canal de audio nulo"))? as usize,
                )?,
                tick: r.u32()?,
                length: r.u32()?,
                origin: r.u32()? as i32,
                offset: r.f64()?,
                gain: r.f32()?,
                muted: r.boolean()?,
                group: r.u16()?,
            });
        }
    }
    if r.remaining() != 0 {
        return Err(Error::Invalid("Datos extra al final del proyecto"));
    }
    p.validate()?;
    Ok(p)
}
