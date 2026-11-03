use crate::{Error, Result, model::*};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub enum Tool {
    Chop(u8),
    Strum(u32),
    Arpeggio(u32),
    Flam(u32),
    Scale { root: u8, minor: bool },
    Delete,
}

#[derive(Clone)]
pub struct Clipboard {
    notes: Vec<Note>,
}

fn selected(pattern: &Pattern, indices: &[usize]) -> Result<BTreeSet<usize>> {
    if pattern.length == 0 || pattern.length > MAX_TICKS || pattern.notes.len() > MAX_NOTES {
        return Err(Error::Invalid("Patrón no válido"));
    }
    for note in &pattern.notes {
        note.validate(pattern.length)?;
    }
    let selected: BTreeSet<_> = indices.iter().copied().collect();
    if selected.len() != indices.len() || selected.last().is_some_and(|i| *i >= pattern.notes.len())
    {
        return Err(Error::Invalid("Selección de notas no válida"));
    }
    Ok(selected)
}

pub fn copy(pattern: &Pattern, indices: &[usize]) -> Result<Clipboard> {
    let indices = selected(pattern, indices)?;
    let start = indices
        .iter()
        .map(|i| pattern.notes[*i].tick)
        .min()
        .unwrap_or(0);
    let notes = indices
        .iter()
        .map(|i| {
            let mut n = pattern.notes[*i];
            n.tick -= start;
            n
        })
        .collect();
    Ok(Clipboard { notes })
}

pub fn paste(pattern: &Pattern, clipboard: &Clipboard, at: u32) -> Result<Pattern> {
    selected(pattern, &[])?;
    if pattern.notes.len() + clipboard.notes.len() > MAX_NOTES {
        return Err(Error::Limit("Demasiadas notas"));
    }
    let mut next = pattern.clone();
    for n in &clipboard.notes {
        let mut n = *n;
        n.tick = n
            .tick
            .checked_add(at)
            .ok_or(Error::Limit("Pegado fuera del patrón"))?;
        let end = n
            .tick
            .checked_add(n.length)
            .ok_or(Error::Limit("Pegado fuera del patrón"))?;
        if end > MAX_TICKS {
            return Err(Error::Limit("Pegado fuera del patrón"));
        }
        next.length = next.length.max(end);
        next.notes.push(n);
    }
    next.notes.sort_by_key(|n| (n.tick, n.key));
    Ok(next)
}

pub fn apply(pattern: &Pattern, indices: &[usize], tool: Tool) -> Result<Pattern> {
    let selected = selected(pattern, indices)?;
    match tool {
        Tool::Chop(n) if !(2..=32).contains(&n) => {
            return Err(Error::Invalid("División no válida"));
        }
        Tool::Strum(t) | Tool::Arpeggio(t) | Tool::Flam(t) if t == 0 || t > BAR => {
            return Err(Error::Invalid("Paso no válido"));
        }
        Tool::Scale { root, .. } if root > 11 => return Err(Error::Invalid("Escala no válida")),
        _ => {}
    }
    let mut next = pattern.clone();
    next.notes.clear();
    let mut chord_positions = std::collections::BTreeMap::<u32, usize>::new();
    let mut ordered: Vec<_> = pattern.notes.iter().copied().enumerate().collect();
    ordered.sort_by_key(|(i, n)| (n.tick, n.key, *i));
    for (i, mut n) in ordered {
        if !selected.contains(&i) {
            next.notes.push(n);
            continue;
        }
        match tool {
            Tool::Delete => continue,
            Tool::Chop(parts) => {
                let parts = (parts as u32).min(n.length);
                for i in 0..parts {
                    let a = n.length as u64 * i as u64 / parts as u64;
                    let b = n.length as u64 * (i + 1) as u64 / parts as u64;
                    next.notes.push(Note {
                        tick: n.tick + a as u32,
                        length: (b - a) as u32,
                        ..n
                    });
                }
                if next.notes.len() > MAX_NOTES {
                    return Err(Error::Limit("Demasiadas notas"));
                }
                continue;
            }
            Tool::Strum(step) | Tool::Arpeggio(step) => {
                let position = chord_positions.entry(n.tick).or_default();
                let offset = (*position as u64 * step as u64).min(pattern.length as u64) as u32;
                *position += 1;
                n.tick = n.tick.saturating_add(offset).min(pattern.length - 1);
                n.length = if matches!(tool, Tool::Arpeggio(_)) {
                    step
                } else {
                    n.length
                };
                n.length = n.length.min(pattern.length - n.tick);
            }
            Tool::Flam(step) => {
                if n.tick > 0 {
                    let length = step.min(n.tick);
                    next.notes.push(Note {
                        tick: n.tick - length,
                        length,
                        velocity: n.velocity * 0.5,
                        ..n
                    });
                }
            }
            Tool::Scale { root, minor } => {
                let allowed: [u16; 7] = if minor {
                    [0, 2, 3, 5, 7, 8, 10]
                } else {
                    [0, 2, 4, 5, 7, 9, 11]
                };
                n.key = (0..=127u8)
                    .filter(|key| allowed.contains(&((*key as u16 + 12 - root as u16) % 12)))
                    .min_by_key(|key| (key.abs_diff(n.key), *key))
                    .unwrap_or(n.key);
            }
        }
        next.notes.push(n);
        if next.notes.len() > MAX_NOTES {
            return Err(Error::Limit("Demasiadas notas"));
        }
    }
    next.notes.sort_by_key(|n| (n.tick, n.key));
    Ok(next)
}

pub fn paste_on_channel(
    pattern: &Pattern,
    clipboard: &Clipboard,
    at: u32,
    channel: usize,
) -> Result<Pattern> {
    let id = ChannelId::new(channel)?;
    let mut clipboard = clipboard.clone();
    for note in &mut clipboard.notes {
        note.channel = Some(id);
    }
    paste(pattern, &clipboard, at)
}

pub fn edit_channel(
    pattern: &Pattern,
    channel: usize,
    change: impl FnOnce(&mut Pattern),
) -> Result<Pattern> {
    ChannelId::new(channel)?;
    selected(pattern, &[])?;
    let mut local = pattern.clone();
    local
        .notes
        .retain(|note| note.channel_or(channel) == channel);
    change(&mut local);
    selected(&local, &[])?;
    if local
        .notes
        .iter()
        .any(|note| note.channel_or(channel) != channel)
    {
        return Err(Error::Invalid("La edición cambió el canal de una nota"));
    }
    let mut next = pattern.clone();
    next.length = next.length.max(local.length);
    next.notes
        .retain(|note| note.channel_or(channel) != channel);
    next.notes.extend(local.notes);
    next.notes.sort_by_key(|note| (note.tick, note.key));
    selected(&next, &[])?;
    Ok(next)
}
