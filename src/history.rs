use crate::{
    Error, Result,
    model::{Note, Pattern, Project},
};
use std::collections::VecDeque;

pub struct History {
    pub project: Project,
    past: VecDeque<Entry>,
    future: Vec<Entry>,
    pruned: bool,
}
struct Entry {
    project: Project,
    label: String,
}
impl History {
    pub fn new(project: Project) -> Result<Self> {
        project.validate()?;
        Ok(Self {
            project,
            past: VecDeque::new(),
            future: Vec::new(),
            pruned: false,
        })
    }
    pub fn edit(&mut self, change: impl FnOnce(&mut Project) -> Result<()>) -> Result<()> {
        self.edit_named("Edit", change)
    }
    pub fn edit_named(
        &mut self,
        label: &str,
        change: impl FnOnce(&mut Project) -> Result<()>,
    ) -> Result<()> {
        crate::model::text(label)?;
        if label.trim().is_empty() || label.len() > 96 {
            return Err(Error::Invalid("Nombre de cambio no válido"));
        }
        let mut next = self.project.clone();
        change(&mut next)?;
        next.validate()?;
        if next == self.project {
            return Ok(());
        }
        self.past.push_back(Entry {
            project: std::mem::replace(&mut self.project, next),
            label: label.into(),
        });
        self.future.clear();
        while self.past.len() > 32
            || retained(self.past.iter().map(|p| &p.project), &self.project) > 32 * 1024 * 1024
        {
            self.past.pop_front();
            self.pruned = true;
        }
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(p) = self.past.pop_back() {
            self.future.push(Entry {
                project: std::mem::replace(&mut self.project, p.project),
                label: p.label,
            });
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(p) = self.future.pop() {
            self.past.push_back(Entry {
                project: std::mem::replace(&mut self.project, p.project),
                label: p.label,
            });
            true
        } else {
            false
        }
    }
    pub fn position(&self) -> usize {
        self.past.len()
    }
    pub fn labels(&self) -> Vec<&str> {
        let mut labels = vec![if self.pruned {
            "Earlier retained state"
        } else {
            "Session opened"
        }];
        labels.extend(self.past.iter().map(|p| p.label.as_str()));
        labels.extend(self.future.iter().rev().map(|p| p.label.as_str()));
        labels
    }
    pub fn seek(&mut self, index: usize) -> Result<bool> {
        if index > self.past.len() + self.future.len() {
            return Err(Error::Invalid("Paso de historial no válido"));
        }
        let changed = index != self.position();
        while self.position() > index {
            self.undo();
        }
        while self.position() < index {
            self.redo();
        }
        Ok(changed)
    }
}

fn retained<'a>(past: impl Iterator<Item = &'a Project>, current: &Project) -> usize {
    let mut shared = std::collections::HashSet::new();
    for sample in current.tracks.iter().filter_map(|t| t.sample.as_ref()) {
        shared.insert(std::sync::Arc::as_ptr(&sample.frames) as usize);
    }
    if let Some(source) = &current.source_flp {
        shared.insert(std::sync::Arc::as_ptr(source) as usize);
    }
    let mut bytes = 0;
    for p in past {
        bytes += p
            .patterns
            .iter()
            .map(|p| p.notes.len() * std::mem::size_of::<Note>() + p.name.len())
            .sum::<usize>()
            + 16_384;
        bytes += p.routes.len() * std::mem::size_of::<crate::routing::Route>();
        bytes += p.clips.len() * std::mem::size_of::<crate::model::Clip>();
        bytes += p.audio_clips.len() * std::mem::size_of::<crate::model::AudioClip>();
        for sample in p.tracks.iter().filter_map(|t| t.sample.as_ref()) {
            if shared.insert(std::sync::Arc::as_ptr(&sample.frames) as usize) {
                bytes += sample.frames.len() * 8;
            }
        }
        if let Some(source) = &p.source_flp
            && shared.insert(std::sync::Arc::as_ptr(source) as usize)
        {
            bytes += source.len();
        }
    }
    bytes
}

pub fn quantize(pattern: &mut Pattern, grid: u32, strength: f32) {
    if grid == 0 || !strength.is_finite() {
        return;
    }
    for note in &mut pattern.notes {
        let target = ((note.tick as f64 / grid as f64).round() * grid as f64) as u32;
        let target = (note.tick as f64
            + (target as f64 - note.tick as f64) * strength.clamp(0., 1.) as f64)
            .round() as u32;
        note.tick = target.min(pattern.length.saturating_sub(note.length));
    }
    pattern.notes.sort_by_key(|n| (n.tick, n.key));
}

pub fn transpose(pattern: &mut Pattern, semitones: i16) {
    for n in &mut pattern.notes {
        n.key = (n.key as i32 + semitones as i32).clamp(0, 127) as u8;
    }
}
pub fn reverse(pattern: &mut Pattern) {
    for n in &mut pattern.notes {
        n.tick = pattern
            .length
            .saturating_sub(n.tick.saturating_add(n.length));
    }
    pattern.notes.sort_by_key(|n| (n.tick, n.key));
}
pub fn legato(pattern: &mut Pattern) {
    pattern.notes.sort_by_key(|n| (n.tick, n.key));
    for i in 0..pattern.notes.len() {
        let start = pattern.notes[i].tick;
        let end = pattern.notes[i + 1..]
            .iter()
            .find(|n| n.tick > start)
            .map_or(pattern.length, |n| n.tick);
        pattern.notes[i].length = end.saturating_sub(start).max(1);
    }
}
pub fn humanize(pattern: &mut Pattern, seed: u64, ticks: u32, amount: f32) {
    if !amount.is_finite() {
        return;
    }
    let mut state = seed.max(1);
    let ticks = ticks.min(240);
    for n in &mut pattern.notes {
        let shift = (crate::dsp::noise(&mut state) * ticks as f64).round() as i64;
        n.tick =
            (n.tick as i64 + shift).clamp(0, pattern.length.saturating_sub(n.length) as i64) as u32;
        n.velocity = (n.velocity + crate::dsp::noise(&mut state) as f32 * amount.clamp(0., 1.))
            .clamp(0.01, 1.);
    }
    pattern.notes.sort_by_key(|n| (n.tick, n.key));
}
