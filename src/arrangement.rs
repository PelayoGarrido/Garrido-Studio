use crate::{Error, Result, model::*};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClipId {
    Pattern(usize),
    Audio(usize),
}
#[derive(Clone, Copy, Debug)]
pub struct Properties {
    pub track: usize,
    pub tick: u32,
    pub length: u32,
    pub muted: bool,
    pub group: u16,
}
#[derive(Clone, Copy)]
pub enum Tool {
    Delete,
    Mute,
    Group,
    Ungroup,
    Split(u32),
    TrimStart(u32),
    Join,
    Move { ticks: i64, lanes: i32 },
    Resize(i64),
    Duplicate(u32),
}
#[derive(Clone)]
pub struct Clipboard {
    patterns: Vec<Clip>,
    audio: Vec<AudioClip>,
    start: u32,
}

pub fn all(p: &Project) -> impl Iterator<Item = ClipId> + '_ {
    (0..p.clips.len())
        .map(ClipId::Pattern)
        .chain((0..p.audio_clips.len()).map(ClipId::Audio))
}
pub fn properties(p: &Project, id: ClipId) -> Result<Properties> {
    Ok(match id {
        ClipId::Pattern(i) => {
            let c = p.clips.get(i).ok_or(Error::Invalid("Clip no disponible"))?;
            Properties {
                track: c.track,
                tick: c.tick,
                length: c.length,
                muted: c.muted,
                group: c.group,
            }
        }
        ClipId::Audio(i) => {
            let c = p
                .audio_clips
                .get(i)
                .ok_or(Error::Invalid("Clip de audio no disponible"))?;
            Properties {
                track: c.track,
                tick: c.tick,
                length: c.length,
                muted: c.muted,
                group: c.group,
            }
        }
    })
}
pub fn members(p: &Project, selection: &[ClipId]) -> Result<Vec<ClipId>> {
    let mut groups = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for &id in selection {
        let group = properties(p, id)?.group;
        if group != 0 {
            groups.insert(group);
        }
        ids.insert(id);
    }
    for id in all(p) {
        if groups.contains(&properties(p, id)?.group) {
            ids.insert(id);
        }
    }
    Ok(ids.into_iter().collect())
}
fn next_group(p: &Project) -> Result<u16> {
    let groups = all(p)
        .map(|id| properties(p, id).map(|c| c.group))
        .collect::<Result<BTreeSet<_>>>()?;
    (1..=u16::MAX)
        .find(|n| !groups.contains(n))
        .ok_or(Error::Limit("Demasiados grupos"))
}
fn remove(p: &mut Project, ids: &[ClipId]) {
    for &id in ids.iter().rev() {
        match id {
            ClipId::Pattern(i) => {
                p.clips.remove(i);
            }
            ClipId::Audio(i) => {
                p.audio_clips.remove(i);
            }
        }
    }
}
fn source_position(p: &Project, c: &AudioClip, tick: u32) -> f64 {
    let rate = p.tracks[c.channel.index()]
        .sample
        .as_ref()
        .map_or(0, |s| s.rate);
    let frame = (p.seconds_at(tick as f64) * p.rate as f64).round();
    let origin = (p.seconds_at(c.origin as f64) * p.rate as f64).round();
    c.offset + (frame - origin) * rate as f64 / p.rate as f64
}
fn move_clip(p: &mut Project, id: ClipId, ticks: i64, lanes: i32) {
    match id {
        ClipId::Pattern(i) => {
            let c = &mut p.clips[i];
            c.tick = (c.tick as i64 + ticks) as u32;
            c.track = (c.track as i64 + lanes as i64) as usize;
        }
        ClipId::Audio(i) => {
            let old = &p.audio_clips[i];
            let offset = source_position(p, old, old.tick);
            let c = &mut p.audio_clips[i];
            c.tick = (c.tick as i64 + ticks) as u32;
            if ticks != 0 {
                c.origin = c.tick as i32;
                c.offset = offset;
            }
            c.track = (c.track as i64 + lanes as i64) as usize;
        }
    }
}
pub fn copy(p: &Project, selection: &[ClipId]) -> Result<Clipboard> {
    let ids = members(p, selection)?;
    if ids.is_empty() {
        return Err(Error::Invalid("Selecciona clips del arreglo"));
    }
    let mut clipboard = Clipboard {
        patterns: Vec::new(),
        audio: Vec::new(),
        start: MAX_TICKS,
    };
    for id in ids {
        clipboard.start = clipboard.start.min(properties(p, id)?.tick);
        match id {
            ClipId::Pattern(i) => clipboard.patterns.push(p.clips[i].clone()),
            ClipId::Audio(i) => clipboard.audio.push(p.audio_clips[i].clone()),
        }
    }
    Ok(clipboard)
}
pub fn paste(p: &Project, clipboard: &Clipboard, tick: u32) -> Result<Project> {
    p.validate()?;
    let mut next = p.clone();
    let delta = tick as i64 - clipboard.start as i64;
    let mut groups = BTreeMap::new();
    for mut c in clipboard.patterns.clone() {
        let end = c.tick as i64 + delta + c.length as i64;
        if end > MAX_TICKS as i64 || c.tick as i64 + delta < 0 {
            return Err(Error::Invalid("La copia queda fuera del arreglo"));
        }
        c.tick = (c.tick as i64 + delta) as u32;
        c.group = remap_group(&next, &mut groups, c.group)?;
        next.bars = next.bars.max((end as u32).div_ceil(BAR) as u16);
        next.clips.push(c);
    }
    for mut c in clipboard.audio.clone() {
        let end = c.tick as i64 + delta + c.length as i64;
        if end > MAX_TICKS as i64 || c.tick as i64 + delta < 0 {
            return Err(Error::Invalid("La copia queda fuera del arreglo"));
        }
        let offset = source_position(p, &c, c.tick);
        c.tick = (c.tick as i64 + delta) as u32;
        if delta != 0 {
            c.origin = c.tick as i32;
            c.offset = offset;
        }
        c.group = remap_group(&next, &mut groups, c.group)?;
        next.bars = next.bars.max((end as u32).div_ceil(BAR) as u16);
        next.audio_clips.push(c);
    }
    next.validate()?;
    Ok(next)
}
fn remap_group(p: &Project, groups: &mut BTreeMap<u16, u16>, old: u16) -> Result<u16> {
    if old == 0 {
        return Ok(0);
    }
    if let Some(&group) = groups.get(&old) {
        return Ok(group);
    }
    let group = next_group(p)?;
    groups.insert(old, group);
    Ok(group)
}
pub fn apply(p: &Project, selection: &[ClipId], tool: Tool) -> Result<Project> {
    p.validate()?;
    let ids = members(p, selection)?;
    if ids.is_empty() {
        return Err(Error::Invalid("Selecciona clips del arreglo"));
    }
    if let Tool::Duplicate(tick) = tool {
        return paste(p, &copy(p, &ids)?, tick);
    }
    let mut next = p.clone();
    match tool {
        Tool::Delete => remove(&mut next, &ids),
        Tool::Mute | Tool::Group | Tool::Ungroup => {
            let mute = !ids
                .iter()
                .all(|&id| properties(p, id).is_ok_and(|c| c.muted));
            let group = if matches!(tool, Tool::Group) {
                next_group(p)?
            } else {
                0
            };
            for &id in &ids {
                let (muted, current) = match id {
                    ClipId::Pattern(i) => {
                        let c = &mut next.clips[i];
                        (&mut c.muted, &mut c.group)
                    }
                    ClipId::Audio(i) => {
                        let c = &mut next.audio_clips[i];
                        (&mut c.muted, &mut c.group)
                    }
                };
                if matches!(tool, Tool::Mute) {
                    *muted = mute;
                } else {
                    *current = group;
                }
            }
        }
        Tool::Move { ticks, lanes } => {
            let mut low_tick = i64::MIN;
            let mut high_tick = i64::MAX;
            let mut low_lane = i32::MIN;
            let mut high_lane = i32::MAX;
            for &id in &ids {
                let c = properties(p, id)?;
                low_tick = low_tick.max(-(c.tick as i64));
                high_tick = high_tick.min((MAX_TICKS - c.tick - c.length) as i64);
                if let ClipId::Audio(i) = id {
                    low_tick = low_tick.max(-(MAX_TICKS as i64) - p.audio_clips[i].origin as i64);
                }
                low_lane = low_lane.max(-(c.track as i32));
                high_lane = high_lane.min((MAX_LANES - 1 - c.track) as i32);
            }
            for &id in &ids {
                move_clip(
                    &mut next,
                    id,
                    ticks.clamp(low_tick, high_tick),
                    lanes.clamp(low_lane, high_lane),
                );
            }
        }
        Tool::Resize(delta) => {
            for &id in &ids {
                let c = properties(p, id)?;
                let length = (c.length as i64 + delta).clamp(1, (MAX_TICKS - c.tick) as i64) as u32;
                match id {
                    ClipId::Pattern(i) => next.clips[i].length = length,
                    ClipId::Audio(i) => next.audio_clips[i].length = length,
                }
            }
        }
        Tool::Split(tick) | Tool::TrimStart(tick) => {
            let mut cuts = 0;
            for &id in &ids {
                let c = properties(p, id)?;
                if tick <= c.tick || tick >= c.tick + c.length {
                    continue;
                }
                let left = tick - c.tick;
                match id {
                    ClipId::Pattern(i) => {
                        let mut right = p.clips[i].clone();
                        let pat = &p.patterns[right.pattern];
                        let offset = (right.offset + left) % pat.length;
                        if pat
                            .notes
                            .iter()
                            .any(|n| n.tick < offset && n.tick + n.length > offset)
                        {
                            return Err(Error::Unsupported(
                                "Corta entre notas para conservar su sonido",
                            ));
                        }
                        right.offset = offset;
                        right.tick = tick;
                        right.length -= left;
                        if matches!(tool, Tool::TrimStart(_)) {
                            next.clips[i] = right;
                        } else {
                            next.clips[i].length = left;
                            next.clips.push(right);
                        }
                    }
                    ClipId::Audio(i) => {
                        let mut right = p.audio_clips[i].clone();
                        right.tick = tick;
                        right.length -= left;
                        if matches!(tool, Tool::TrimStart(_)) {
                            next.audio_clips[i] = right;
                        } else {
                            next.audio_clips[i].length = left;
                            next.audio_clips.push(right);
                        }
                    }
                }
                cuts += 1;
            }
            if cuts == 0 {
                return Err(Error::Invalid("El corte debe quedar dentro del clip"));
            }
        }
        Tool::Join => {
            let mut ordered = ids.clone();
            ordered.sort_by_key(|&id| properties(p, id).map_or(MAX_TICKS, |c| c.tick));
            let first = ordered[0];
            if ordered.len() < 2 {
                return Err(Error::Invalid("Selecciona dos clips contiguos"));
            }
            let mut length = properties(p, first)?.length;
            for pair in ordered.windows(2) {
                let a = properties(p, pair[0])?;
                let b = properties(p, pair[1])?;
                if a.track != b.track
                    || a.tick + a.length != b.tick
                    || a.muted != b.muted
                    || a.group != b.group
                {
                    return Err(Error::Invalid(
                        "Solo se unen clips contiguos del mismo material",
                    ));
                }
                let compatible = match (pair[0], pair[1]) {
                    (ClipId::Pattern(i), ClipId::Pattern(j)) => {
                        let x = &p.clips[i];
                        let y = &p.clips[j];
                        x.pattern == y.pattern
                            && (x.offset + x.length) % p.patterns[x.pattern].length == y.offset
                    }
                    (ClipId::Audio(i), ClipId::Audio(j)) => {
                        let x = &p.audio_clips[i];
                        let y = &p.audio_clips[j];
                        x.channel == y.channel
                            && ((x.origin == y.origin && x.offset == y.offset)
                                || (source_position(p, x, y.tick) - source_position(p, y, y.tick))
                                    .abs()
                                    < 1e-7)
                            && x.gain == y.gain
                    }
                    _ => false,
                };
                if !compatible {
                    return Err(Error::Invalid("Los clips usan materiales distintos"));
                }
                length += b.length;
            }
            match first {
                ClipId::Pattern(i) => next.clips[i].length = length,
                ClipId::Audio(i) => next.audio_clips[i].length = length,
            }
            let mut rest = ordered[1..].to_vec();
            rest.sort();
            remove(&mut next, &rest);
        }
        Tool::Duplicate(_) => unreachable!(),
    }
    let end = all(&next)
        .map(|id| properties(&next, id).map(|c| c.tick + c.length))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    next.bars = next.bars.max(end.div_ceil(BAR) as u16);
    next.validate()?;
    Ok(next)
}

pub fn add_audio(
    p: &mut Project,
    sample: Sample,
    name: &str,
    tick: u32,
    lane: usize,
) -> Result<ClipId> {
    p.validate()?;
    if tick >= MAX_TICKS || lane >= MAX_LANES || sample.frames.is_empty() {
        return Err(Error::Invalid("Destino o audio no válido"));
    }
    let mut next = p.clone();
    let end = next
        .tick_at(next.seconds_at(tick as f64) + sample.frames.len() as f64 / sample.rate as f64)
        .ceil();
    if !end.is_finite() || end > MAX_TICKS as f64 {
        return Err(Error::Limit("El audio supera el límite del arreglo"));
    }
    let length = (end as u32).saturating_sub(tick).max(1);
    let channel = next.add_channel(Instrument::Sampler)?;
    next.tracks[channel].name = name.into();
    next.tracks[channel].sample = Some(sample);
    let id = ClipId::Audio(next.audio_clips.len());
    next.audio_clips.push(AudioClip {
        track: lane,
        channel: ChannelId::new(channel)?,
        tick,
        length,
        origin: tick as i32,
        offset: 0.,
        gain: 1.,
        muted: false,
        group: 0,
    });
    next.bars = next.bars.max((tick + length).div_ceil(BAR) as u16);
    next.validate()?;
    *p = next;
    Ok(id)
}
