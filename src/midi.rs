use crate::{Error, Result, codec::Reader, model::*};
use std::collections::BTreeMap;

fn be16(r: &mut Reader<'_>) -> Result<u16> {
    Ok(u16::from_be_bytes(
        r.take(2)?
            .try_into()
            .map_err(|_| Error::Invalid("MIDI truncado"))?,
    ))
}
fn be32(r: &mut Reader<'_>) -> Result<u32> {
    Ok(u32::from_be_bytes(
        r.take(4)?
            .try_into()
            .map_err(|_| Error::Invalid("MIDI truncado"))?,
    ))
}
fn vlq(r: &mut Reader<'_>) -> Result<u32> {
    let mut value = 0;
    for _ in 0..4 {
        let b = r.u8()?;
        value = (value << 7) | (b & 127) as u32;
        if b & 128 == 0 {
            return Ok(value);
        }
    }
    Err(Error::Invalid("VLQ MIDI demasiado largo"))
}
fn push_vlq(out: &mut Vec<u8>, value: u32) {
    let mut b = [0u8; 5];
    let mut at = 4;
    b[at] = (value & 127) as u8;
    let mut rest = value >> 7;
    while rest > 0 {
        at -= 1;
        b[at] = (rest & 127) as u8 | 128;
        rest >>= 7;
    }
    out.extend(&b[at..]);
}
fn data_byte(r: &mut Reader<'_>) -> Result<u8> {
    let b = r.u8()?;
    if b > 127 {
        Err(Error::Invalid("Byte MIDI fuera de rango"))
    } else {
        Ok(b)
    }
}

pub struct Import {
    pub project: Project,
    pub ignored_events: usize,
}

pub fn decode(data: &[u8]) -> Result<Import> {
    if data.len() > 8 * 1024 * 1024 {
        return Err(Error::Limit("MIDI: máximo 8 MB"));
    }
    let mut r = Reader::new(data);
    if r.take(4)? != b"MThd" || be32(&mut r)? != 6 {
        return Err(Error::Invalid("Cabecera MIDI incorrecta"));
    }
    let format = be16(&mut r)?;
    let tracks = be16(&mut r)?;
    let division = be16(&mut r)?;
    if format > 1
        || tracks == 0
        || tracks > 64
        || (format == 0 && tracks != 1)
        || division == 0
        || division & 0x8000 != 0
    {
        return Err(Error::Unsupported("MIDI: formato 0/1 con tiempo PPQ"));
    }
    let mut project = Project::empty();
    project.title = "MIDI importado".into();
    let mut channels: BTreeMap<(u16, u8), Vec<Note>> = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut tempos = BTreeMap::new();
    let mut max_tick = BAR;
    let mut event_count = 0;
    let mut note_count = 0;
    let mut ignored = 0;
    let scale = |tick: u32| -> Result<u32> {
        let value = (tick as u64 * PPQ as u64 + division as u64 / 2) / division as u64;
        if value > MAX_TICKS as u64 {
            Err(Error::Limit("MIDI: máximo 256 compases"))
        } else {
            Ok(value as u32)
        }
    };
    for track_index in 0..tracks {
        if r.take(4)? != b"MTrk" {
            return Err(Error::Invalid("Falta pista MIDI"));
        }
        let size = be32(&mut r)? as usize;
        let mut track = Reader::new(r.take(size)?);
        let mut tick = 0u32;
        let mut running = 0u8;
        let mut ended = false;
        let mut active = [None::<(u32, u8, bool)>; 2048];
        let mut sustain = [false; 16];
        while track.remaining() > 0 {
            event_count += 1;
            if event_count > 262_144 {
                return Err(Error::Limit("Demasiados eventos MIDI"));
            }
            tick = tick
                .checked_add(vlq(&mut track)?)
                .ok_or(Error::Limit("Tiempo MIDI desbordado"))?;
            let current = scale(tick)?;
            max_tick = max_tick.max(current);
            let first = track.u8()?;
            let status = if first < 128 {
                if running == 0 {
                    return Err(Error::Invalid("Running status MIDI sin estado"));
                }
                track.position -= 1;
                running
            } else {
                first
            };
            if status == 0xff {
                running = 0;
                let kind = track.u8()?;
                let size = vlq(&mut track)? as usize;
                let bytes = track.take(size)?;
                match kind {
                    0x2f => {
                        if size != 0 || track.remaining() != 0 {
                            return Err(Error::Invalid("Fin de pista MIDI incorrecto"));
                        }
                        ended = true;
                        break;
                    }
                    0x51 => {
                        if size != 3 {
                            return Err(Error::Invalid("Tempo MIDI incorrecto"));
                        }
                        let micros =
                            ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[2] as u32;
                        if micros == 0 {
                            return Err(Error::Invalid("Tempo MIDI nulo"));
                        }
                        let bpm = 60_000_000. / micros as f64;
                        if !(20. ..=300.).contains(&bpm) {
                            return Err(Error::Unsupported("Tempo MIDI fuera de 20–300 BPM"));
                        }
                        tempos.insert(current, bpm);
                    }
                    0x03 => {
                        let name = String::from_utf8_lossy(bytes)
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(64)
                            .collect::<String>();
                        names.insert(track_index, name);
                    }
                    0x06 => {
                        if project.markers.len() >= 1024 {
                            return Err(Error::Limit("Demasiados marcadores MIDI"));
                        }
                        let name = String::from_utf8_lossy(bytes)
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(64)
                            .collect();
                        project.markers.push((current, name));
                    }
                    0x58 => {
                        if bytes.len() != 4 {
                            return Err(Error::Invalid("Compás MIDI incorrecto"));
                        }
                        if bytes[0] != 4 || bytes[1] != 2 {
                            ignored += 1;
                        }
                    }
                    _ => ignored += 1,
                }
            } else if status == 0xf0 || status == 0xf7 {
                running = 0;
                let size = vlq(&mut track)? as usize;
                track.take(size)?;
                ignored += 1;
            } else if (0x80..=0xef).contains(&status) {
                running = status;
                let channel = status & 15;
                let kind = status >> 4;
                let a = data_byte(&mut track)?;
                let b = if kind == 0xc || kind == 0xd {
                    0
                } else {
                    data_byte(&mut track)?
                };
                let index = channel as usize * 128 + a as usize;
                if kind == 0x9 && b > 0 {
                    if let Some((start, velocity, _)) = active[index].take() {
                        push_note(
                            &mut channels,
                            (track_index, channel),
                            start,
                            current,
                            a,
                            velocity,
                            &mut note_count,
                        )?;
                    }
                    active[index] = Some((current, b, false));
                } else if kind == 0x8 || kind == 0x9 {
                    if sustain[channel as usize] {
                        if let Some(v) = &mut active[index] {
                            v.2 = true;
                        }
                    } else if let Some((start, velocity, _)) = active[index].take() {
                        push_note(
                            &mut channels,
                            (track_index, channel),
                            start,
                            current,
                            a,
                            velocity,
                            &mut note_count,
                        )?;
                    }
                } else if kind == 0xb && a == 64 {
                    let held = b >= 64;
                    sustain[channel as usize] = held;
                    if !held {
                        for key in 0..128 {
                            let slot = &mut active[channel as usize * 128 + key];
                            if slot.is_some_and(|v| v.2)
                                && let Some((start, velocity, _)) = slot.take()
                            {
                                push_note(
                                    &mut channels,
                                    (track_index, channel),
                                    start,
                                    current,
                                    key as u8,
                                    velocity,
                                    &mut note_count,
                                )?;
                            }
                        }
                    }
                } else {
                    ignored += 1;
                }
            } else {
                return Err(Error::Invalid("Estado MIDI no válido"));
            }
        }
        if !ended {
            return Err(Error::Invalid("MIDI sin fin de pista"));
        }
        let end = scale(tick)?;
        for (index, slot) in active.iter_mut().enumerate() {
            if let Some((start, velocity, _)) = slot.take() {
                push_note(
                    &mut channels,
                    (track_index, (index / 128) as u8),
                    start,
                    end.max(start + 1),
                    (index % 128) as u8,
                    velocity,
                    &mut note_count,
                )?;
            }
        }
    }
    if r.remaining() != 0 {
        return Err(Error::Invalid("Datos extra al final del MIDI"));
    }
    if channels.len() > MAX_TRACKS {
        return Err(Error::Limit("MIDI: máximo 32 canales con notas"));
    }
    project.bars = max_tick.max(1).div_ceil(BAR).min(256) as u16;
    let length = project.bars as u32 * BAR;
    if let Some(bpm) = tempos.remove(&0) {
        project.bpm = bpm;
    }
    project.tempos = tempos.into_iter().collect();
    for ((source, channel), mut notes) in channels {
        notes.sort_by_key(|n| (n.tick, n.key));
        let i = project.tracks.len();
        let mut t = Track::new(
            names.get(&source).map_or("MIDI", String::as_str),
            if channel == 9 {
                Instrument::Snare
            } else {
                Instrument::Trio
            },
        );
        t.gain = 0.5;
        project.tracks.push(t);
        project.patterns.push(Pattern {
            name: format!("MIDI {}", i + 1),
            length,
            notes,
        });
        project.clips.push(Clip {
            offset: 0,
            muted: false,
            group: 0,
            track: i,
            pattern: i,
            tick: 0,
            length,
        });
    }
    project.validate()?;
    Ok(Import {
        project,
        ignored_events: ignored,
    })
}

fn push_note(
    channels: &mut BTreeMap<(u16, u8), Vec<Note>>,
    key: (u16, u8),
    start: u32,
    end: u32,
    note: u8,
    velocity: u8,
    count: &mut usize,
) -> Result<()> {
    if end <= start {
        return Ok(());
    }
    *count += 1;
    if *count > MAX_NOTES {
        return Err(Error::Limit("Demasiadas notas MIDI"));
    }
    channels.entry(key).or_default().push(Note::new(
        start,
        end - start,
        note,
        velocity as f32 / 127.,
    ));
    Ok(())
}

fn chunk(data: &[u8], out: &mut Vec<u8>) {
    out.extend(b"MTrk");
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
}

pub fn encode(project: &Project) -> Result<Vec<u8>> {
    project.validate()?;
    let mut work = 0usize;
    for clip in &project.clips {
        if clip.muted || project.arrangement_mutes & (1u32 << clip.track) != 0 {
            continue;
        }
        let pattern = &project.patterns[clip.pattern];
        work = work
            .checked_add(
                (clip.length + clip.offset).div_ceil(pattern.length) as usize * pattern.notes.len(),
            )
            .ok_or(Error::Limit("Demasiados eventos MIDI"))?;
        if work > 131_072 {
            return Err(Error::Limit("Demasiados eventos MIDI"));
        }
    }
    let mut out = b"MThd".to_vec();
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend((project.tracks.len() as u16 + 1).to_be_bytes());
    out.extend((PPQ as u16).to_be_bytes());
    let mut timing = Vec::new();
    let mut events = vec![(0, vec![0xff, 0x51, 3])];
    let micros = (60_000_000. / project.bpm).round() as u32;
    events[0].1.extend(&micros.to_be_bytes()[1..]);
    for &(tick, bpm) in &project.tempos {
        let mut e = vec![0xff, 0x51, 3];
        e.extend(&((60_000_000. / bpm).round() as u32).to_be_bytes()[1..]);
        events.push((tick, e));
    }
    for (tick, name) in &project.markers {
        let mut e = vec![0xff, 0x06];
        push_vlq(&mut e, name.len() as u32);
        e.extend(name.as_bytes());
        events.push((*tick, e));
    }
    events.sort_by_key(|e| e.0);
    let mut previous = 0;
    for (tick, event) in events {
        push_vlq(&mut timing, tick - previous);
        timing.extend(event);
        previous = tick;
    }
    push_vlq(&mut timing, project.bars as u32 * BAR - previous);
    timing.extend([0xff, 0x2f, 0]);
    chunk(&timing, &mut out);
    for (i, track) in project.tracks.iter().enumerate() {
        let channel = (i % 16) as u8;
        let mut events = Vec::new();
        for clip in &project.clips {
            if clip.muted || project.arrangement_mutes & (1u32 << clip.track) != 0 {
                continue;
            }
            let pattern = &project.patterns[clip.pattern];
            if pattern.notes.is_empty() {
                continue;
            }
            for repeat in 0..(clip.length + clip.offset).div_ceil(pattern.length) {
                if events.len() + pattern.notes.len() * 2 > 262_144 {
                    return Err(Error::Limit("Demasiados eventos MIDI"));
                }
                for note in &pattern.notes {
                    if note.channel_or(clip.track) != i {
                        continue;
                    }
                    let start = clip.tick as i64 + (repeat * pattern.length + note.tick) as i64
                        - clip.offset as i64;
                    if start < clip.tick as i64 || start >= (clip.tick + clip.length) as i64 {
                        continue;
                    }
                    let start = start as u32;
                    let end = (start + note.length).min(clip.tick + clip.length);
                    if start >= clip.tick + clip.length {
                        continue;
                    }
                    events.push((
                        start,
                        1,
                        [
                            0x90 | channel,
                            note.key,
                            (note.velocity * 127.).round().clamp(1., 127.) as u8,
                        ],
                    ));
                    events.push((end, 0, [0x80 | channel, note.key, 0]));
                }
            }
        }
        events.sort_by_key(|e| (e.0, e.1));
        let mut data = vec![0, 0xff, 0x03];
        push_vlq(&mut data, track.name.len() as u32);
        data.extend(track.name.as_bytes());
        let mut previous = 0;
        for (tick, _, event) in events {
            push_vlq(&mut data, tick - previous);
            data.extend(event);
            previous = tick;
        }
        push_vlq(&mut data, project.bars as u32 * BAR - previous);
        data.extend([0xff, 0x2f, 0]);
        chunk(&data, &mut out);
    }
    Ok(out)
}
