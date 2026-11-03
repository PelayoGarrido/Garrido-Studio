use crate::app::*;
use crate::canvas::*;
use crate::session::Input;
use crate::workspace::{Drag, PIANO_TOOLS};
use garrido_studio::arrangement::{self, ClipId, Tool};
impl Studio {
    fn automation_mouse(&mut self, p: Point, r: Rect, right: bool) {
        if p.y < r.origin.y + 50. {
            if p.x < r.origin.x + 215. {
                self.workspace.automation_target =
                    ((p.x - r.origin.x - 5.) / 68.).clamp(0., 2.) as u8;
            } else {
                let target = self.automation_target();
                let old = self
                    .history
                    .project
                    .automation
                    .iter()
                    .find(|a| a.target == target)
                    .map_or(self.workspace.automation_curve, |a| a.curve);
                let next = [
                    Curve::Hold,
                    Curve::Linear,
                    Curve::Smooth,
                    Curve::Exponential,
                ][(old as usize + 1) % 4];
                self.workspace.automation_curve = next;
                let result = self.history.edit(|p| {
                    if let Some(a) = p.automation.iter_mut().find(|a| a.target == target) {
                        a.curve = next;
                    }
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            self.redraw();
            return;
        }
        let graph = self.automation_graph(r);
        if !inside(graph, p) {
            return;
        }
        let target = self.automation_target();
        if matches!(target,Target::Gain(i)|Target::Pan(i) if i>=self.history.project.tracks.len()) {
            return;
        }
        let end = self.history.project.bars as u32 * BAR;
        let tick = (((p.x - graph.origin.x) / graph.size.width * end as f64 / (PPQ / 4) as f64)
            .round() as u32
            * (PPQ / 4))
            .min(end);
        let (low, high) = automation_range(target);
        let value = low
            + (1. - ((p.y - graph.origin.y) / graph.size.height).clamp(0., 1.) as f32)
                * (high - low);
        let base = self.history.project.clone();
        let mut seed = base.clone();
        let lane = if let Some(i) = seed.automation.iter().position(|a| a.target == target) {
            i
        } else {
            if right || seed.automation.len() >= 64 {
                return;
            }
            seed.automation.push(Automation {
                target,
                curve: self.workspace.automation_curve,
                points: Vec::new(),
            });
            seed.automation.len() - 1
        };
        let points = &mut seed.automation[lane].points;
        let near = points
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.0.abs_diff(tick))
            .filter(|(_, p)| p.0.abs_diff(tick) as f64 / end as f64 * graph.size.width < 7.)
            .map(|(i, _)| i);
        if right {
            let Some(index) = near else {
                return;
            };
            points.remove(index);
            if points.is_empty() {
                seed.automation.remove(lane);
            }
            let result = self.history.edit(|p| {
                *p = seed;
                Ok(())
            });
            if result.is_ok() {
                self.changed();
            }
            self.result(result);
            return;
        }
        let point = if let Some(i) = near {
            i
        } else {
            if points.len() >= 4096 {
                return;
            }
            let i = points.partition_point(|p| p.0 < tick);
            points.insert(i, (tick, value));
            i
        };
        if seed.validate().is_err() {
            return;
        }
        self.stop();
        self.history.project = seed.clone();
        self.workspace.drag = Some(Drag::Automation {
            base: Box::new(base),
            seed: Box::new(seed),
            lane,
            point,
            graph,
        });
        self.redraw();
    }
    pub(crate) fn mouse(&mut self, event: Input, right: bool) {
        let p = event.point;
        if p.y < 40. && ((p.x >= 73. && p.x < 378.) || p.x > 1470.) && !right {
            self.window_drag = true;
            return;
        }
        for i in self.workspace.order.clone().into_iter().rev() {
            let panel = self.workspace.panels[i];
            if !panel.visible || !inside(panel.rect, p) {
                continue;
            }
            self.workspace.order.retain(|v| *v != i);
            self.workspace.order.push(i);
            self.page = match i {
                0 => Page::Steps,
                1 => Page::Piano,
                2 => Page::Mixer,
                3 => Page::Audio,
                _ => Page::Automation,
            };

            let r = panel.rect;
            if p.y < r.origin.y + 26. {
                if p.x > r.origin.x + r.size.width - 25. {
                    self.workspace.panels[i].visible = false;
                } else if i == 1
                    && ((p.x > r.origin.x + r.size.width - 49.
                        && p.x < r.origin.x + r.size.width - 27.)
                        || event.click_count >= 2)
                {
                    self.workspace.maximize_piano();
                } else if i == 1 && p.x < r.origin.x + 355. {
                    let item = PIANO_TOOLS.iter().position(|(_, x)| {
                        p.x >= r.origin.x + x - 1. && p.x < r.origin.x + x + 19.
                    });
                    match item {
                        Some(0 | 1) => self.action(2005),
                        Some(2) => self.action(9030),
                        Some(3) => self.action(2002),
                        Some(5) => self.action(6),
                        Some(6 | 7) => {
                            self.workspace.piano_erase = false;
                            self.workspace.piano_select = false;
                        }
                        Some(9) => {
                            self.workspace.piano_erase = true;
                            self.workspace.piano_select = false;
                        }
                        Some(12) => {
                            self.workspace.piano_select = true;
                            self.workspace.piano_erase = false;
                        }
                        Some(13) => self
                            .workspace
                            .piano_zoom
                            .set((self.workspace.piano_zoom.get() * 1.25).min(384.)),
                        _ => {}
                    }
                } else {
                    self.workspace.drag = Some(Drag::Window {
                        index: i,
                        dx: p.x - r.origin.x,
                        dy: p.y - r.origin.y,
                    });
                }
                self.redraw();
                return;
            }
            let result = match i {
                0 => {
                    if p.y > r.origin.y + r.size.height - 30.
                        && p.x > r.origin.x + 130.
                        && p.x < r.origin.x + 190.
                    {
                        self.action(9);
                        return;
                    }
                    let row = ((p.y - r.origin.y - 48.) / 28.).floor();
                    if row < 0. {
                        return;
                    }
                    let track = (self.selected / 6) * 6 + row as usize;
                    if track >= self.history.project.tracks.len() {
                        return;
                    }
                    if self.selected != track {
                        self.workspace.piano_view.set(None);
                        self.workspace.selected_notes.clear();
                    }
                    self.selected = track;
                    if right {
                        self.history.edit(|p| {
                            p.tracks[track].mute = !p.tracks[track].mute;
                            Ok(())
                        })
                    } else if p.x >= r.origin.x + 229. {
                        let step = ((p.x - r.origin.x - 229.) / 16.).floor().clamp(0., 15.) as u32;
                        let Some(pattern) = self.pattern() else {
                            return;
                        };
                        self.history.edit(|p| {
                            let pat = &mut p.patterns[pattern];
                            let tick = step * PPQ / 4;
                            if tick >= pat.length {
                                return Ok(());
                            }
                            if pat
                                .notes
                                .iter()
                                .any(|n| n.channel_or(track) == track && n.tick / (PPQ / 4) == step)
                            {
                                pat.notes.retain(|n| {
                                    n.channel_or(track) != track || n.tick / (PPQ / 4) != step
                                });
                            } else {
                                let key = if track < 4 {
                                    [36, 38, 42, 36][track]
                                } else {
                                    60
                                };
                                pat.notes.push(
                                    Note::new(tick, (PPQ / 4).min(pat.length - tick), key, 0.65)
                                        .on_channel(track)?,
                                );
                            }
                            Ok(())
                        })
                    } else {
                        self.redraw();
                        return;
                    }
                }
                1 => {
                    let right = right || self.workspace.piano_erase;
                    let (g, base) = self.piano_geometry(r);
                    let Some(pattern) = self.pattern() else {
                        return;
                    };
                    if p.y < g.origin.y && p.y > r.origin.y + 26. && p.x >= g.origin.x {
                        self.workspace.drag = Some(Drag::PanView {
                            x: p.x,
                            start: self.workspace.piano_start.get(),
                            zoom: self.workspace.piano_zoom.get(),
                        });
                        return;
                    }
                    if p.y > g.origin.y + g.size.height + 9.
                        && p.y < r.origin.y + r.size.height - 5.
                        && p.x >= g.origin.x
                        && p.x < g.origin.x + g.size.width
                    {
                        let pat = &self.history.project.patterns[pattern];
                        let tick = self.workspace.piano_start.get()
                            + ((p.x - g.origin.x) / self.workspace.piano_zoom.get() * PPQ as f64)
                                as u32;
                        let threshold =
                            (PPQ as f64 / self.workspace.piano_zoom.get() * 5.).ceil() as u32;
                        if let Some(index) = pat
                            .notes
                            .iter()
                            .enumerate()
                            .filter(|(_, note)| self.visible_note(note))
                            .min_by_key(|(_, n)| n.tick.abs_diff(tick))
                            .filter(|(_, n)| n.tick.abs_diff(tick) <= threshold)
                            .map(|(i, _)| i)
                        {
                            let bottom = r.origin.y + r.size.height - 7.;
                            let velocity = ((bottom - p.y)
                                / (bottom - g.origin.y - g.size.height - 34.))
                                .clamp(0., 1.) as f32;
                            let result = self.history.edit(|p| {
                                p.patterns[pattern].notes[index].velocity = velocity;
                                Ok(())
                            });
                            if result.is_ok() {
                                self.changed();
                            }
                            self.result(result);
                        }
                        return;
                    }
                    if !inside(g, p) {
                        return;
                    }
                    let pat = &self.history.project.patterns[pattern];
                    let grid = self.workspace.note_grid;
                    let tick = ((self.workspace.piano_start.get()
                        + ((p.x - g.origin.x) / self.workspace.piano_zoom.get() * PPQ as f64)
                            as u32)
                        / grid
                        * grid)
                        .min(MAX_TICKS - 1);
                    self.workspace.cursor = tick;
                    let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
                    let key = base + rows - 1 - ((p.y - g.origin.y) / 18.) as u8;
                    if let Some(index) = pat.notes.iter().position(|n| {
                        self.visible_note(n)
                            && n.key == key
                            && n.tick <= tick
                            && n.tick + n.length > tick
                    }) {
                        if right {
                            self.workspace.selected_notes.clear();
                            self.history.edit(|p| {
                                p.patterns[pattern].notes.remove(index);
                                Ok(())
                            })
                        } else {
                            let flags = event.modifiers;
                            if flags & (1 << 20) != 0 {
                                if self.workspace.selected_notes.contains(&index) {
                                    self.workspace.selected_notes.retain(|i| *i != index);
                                } else {
                                    self.workspace.selected_notes.push(index);
                                }
                                self.redraw();
                                return;
                            }
                            if !self.workspace.selected_notes.contains(&index) {
                                self.workspace.selected_notes = vec![index];
                            }
                            let resize = flags & (1 << 17) != 0;
                            self.workspace.drag = Some(Drag::Note {
                                base: Box::new(self.history.project.clone()),
                                pattern,
                                index,
                                indices: self.workspace.selected_notes.clone(),
                                x: p.x,
                                y: p.y,
                                width: self.workspace.piano_zoom.get(),
                                row: 18.,
                                resize,
                            });
                            return;
                        }
                    } else if !right
                        && (self.workspace.piano_select || event.modifiers & (1 << 20) != 0)
                    {
                        let previous = if event.modifiers & (1 << 20) != 0 {
                            self.workspace.selected_notes.clone()
                        } else {
                            Vec::new()
                        };
                        self.workspace.selected_notes = previous.clone();
                        self.workspace.drag = Some(Drag::Selection {
                            pattern,
                            origin: p,
                            current: p,
                            grid: g,
                            base_key: base,
                            previous,
                        });
                        self.redraw();
                        return;
                    } else if !right {
                        self.workspace.selected_notes.clear();
                        let channel = self.selected;
                        self.history.edit(|p| {
                            let length = (PPQ / 4).min(MAX_TICKS - tick);
                            p.patterns[pattern].length = p.patterns[pattern]
                                .length
                                .max((tick + length).div_ceil(BAR) * BAR);
                            p.patterns[pattern]
                                .notes
                                .push(Note::new(tick, length, key, 0.65).on_channel(channel)?);
                            Ok(())
                        })
                    } else {
                        return;
                    }
                }
                2 => {
                    let start = (self.selected / 12) * 12;
                    let count = self
                        .history
                        .project
                        .tracks
                        .len()
                        .saturating_sub(start)
                        .clamp(1, 12);
                    let cw = (r.size.width - 15.) / count as f64;
                    let track = start + ((p.x - r.origin.x - 7.) / cw).floor().max(0.) as usize;
                    if track >= self.history.project.tracks.len() {
                        return;
                    }
                    self.selected = track;
                    if right {
                        self.routing_menu(p);
                        self.redraw();
                        return;
                    }
                    let top = r.origin.y + 52.;
                    let height = r.size.height - 125.;
                    if p.y >= top && p.y < top + height {
                        self.workspace.drag = Some(Drag::Gain {
                            base: Box::new(self.history.project.clone()),
                            track,
                            top,
                            height,
                        });
                        self.drag(event);
                        return;
                    }
                    let point = p;
                    self.history.edit(|p| {
                        if (point.y - top - height - 34.).abs() < 14. {
                            if (point.x - r.origin.x - 7.) % cw < cw * 0.5 {
                                p.tracks[track].mute = !p.tracks[track].mute;
                            } else {
                                p.tracks[track].solo = !p.tracks[track].solo;
                            }
                        }
                        Ok(())
                    })
                }
                4 => {
                    self.automation_mouse(p, r, right);
                    return;
                }
                _ => {
                    let at = ((p.x - r.origin.x - 10.) / (r.size.width - 20.)).clamp(0., 1.);
                    if let Some(anchor) = self.anchor.take() {
                        self.selection = (anchor.min(at), anchor.max(at));
                    } else {
                        self.anchor = Some(at);
                        self.selection = (at, at);
                    }
                    self.redraw();
                    return;
                }
            };
            if result.is_ok() {
                self.changed();
            }
            self.result(result);
            return;
        }
        if p.x >= 209. && p.x < 350. && p.y >= 154. {
            let start = self.pattern().unwrap_or(0).saturating_sub(20);
            self.select_pattern(start + ((p.y - 154.) / 32.) as usize);
            return;
        }
        if p.x < 205. {
            if (142. ..166.).contains(&p.y) {
                self.workspace.browser_expanded = !self.workspace.browser_expanded;
                self.redraw();
                return;
            }
            if self.workspace.browser_expanded && p.y >= 190. {
                let count = self
                    .history
                    .project
                    .patterns
                    .len()
                    .min(((self.height - 330.) / 23.).max(0.) as usize);
                let row = ((p.y - 190.) / 23.) as usize;
                if row < count {
                    self.select_pattern(row);
                    if event.click_count >= 2 {
                        self.workspace.show(Page::Piano);
                    }
                    return;
                }
                if row == count {
                    self.action(40);
                } else if row == count + 1 {
                    self.action(9);
                } else if row == count + 2 {
                    self.action(8);
                } else {
                    self.action(71);
                }
            } else if p.y >= 166. && p.y < 189. {
                self.action(71);
            }
            return;
        }
        let view = self.workspace.arrangement(self.width, self.height);
        if p.x >= view.grid.origin.x && (128. ..148.).contains(&p.y) {
            self.page = Page::Arrange;
            self.workspace.arrangement_cursor =
                view.tick_at(p.x) / self.workspace.note_grid * self.workspace.note_grid;
            self.redraw();
            return;
        }
        if (369. ..481.).contains(&p.x) && p.y >= 148. {
            let lane = view.lane_at(p.y);
            if lane < MAX_LANES && p.x >= 449. {
                let result = self.history.edit(|project| {
                    project.arrangement_mutes ^= 1u32 << lane;
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            return;
        }
        if p.x >= self.width - 22. && p.y >= 148. {
            let travel = (self.height - 148. - 34.).max(1.);
            let max_row = (MAX_LANES as f64 - view.grid.size.height / 48.).max(0.);
            self.workspace.arrangement_row = ((p.y - 148. - 17.) / travel).clamp(0., 1.) * max_row;
            self.redraw();
            return;
        }
        if inside(view.grid, p) {
            let track = view.lane_at(p.y);
            if track >= MAX_LANES {
                return;
            }
            let grid = self.workspace.note_grid;
            let tick = view.tick_at(p.x) / grid * grid;
            self.page = Page::Arrange;
            self.workspace.arrangement_cursor = tick;
            self.workspace.arrangement_lane = track;
            let existing = arrangement::all(&self.history.project)
                .filter(|&id| {
                    arrangement::properties(&self.history.project, id).is_ok_and(|c| {
                        c.track == track
                            && c.tick <= view.tick_at(p.x)
                            && c.tick + c.length > view.tick_at(p.x)
                    })
                })
                .last();
            if let Some(id) = existing {
                if let ClipId::Pattern(index) = id {
                    self.selected_pattern = Some(self.history.project.clips[index].pattern);
                    self.workspace.selected_notes.clear();
                } else if let ClipId::Audio(index) = id {
                    self.selected = self.history.project.audio_clips[index].channel.index();
                    self.selection = (0., 1.);
                }
                self.select_clip(id, event.modifiers & (1 << 20) != 0);
                if right {
                    self.arrangement_action(444);
                } else if event.modifiers & (1 << 19) != 0 {
                    self.arrangement_action(9060);
                } else if let Ok(c) = arrangement::properties(&self.history.project, id) {
                    self.workspace.drag = Some(Drag::Clip {
                        base: Box::new(self.history.project.clone()),
                        ids: self.workspace.selected_clips.clone(),
                        x: p.x,
                        y: p.y,
                        width: view.grid.size.width,
                        span: view.ticks,
                        resize: event.modifiers & (1 << 17) != 0
                            || view.x_at(c.tick + c.length) - p.x < 7.,
                    });
                }
            } else if !right
                && event.modifiers & ((1 << 20) | (1 << 19)) == 0
                && let Some(pattern) = self.pattern()
            {
                let length = self.history.project.patterns[pattern].length;
                if tick + length > MAX_TICKS {
                    return;
                }
                let result = self.history.edit(|p| {
                    p.bars = p.bars.max((tick + length).div_ceil(BAR) as u16);
                    p.clips.push(Clip {
                        offset: 0,
                        muted: false,
                        group: 0,
                        track,
                        pattern,
                        tick,
                        length,
                    });
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
        }
        self.redraw();
    }
    pub(crate) fn drag(&mut self, event: Input) {
        let p = event.point;
        let Some(drag) = self.workspace.drag.as_ref() else {
            return;
        };
        let mut next = match drag {
            Drag::Window { .. } | Drag::PanView { .. } | Drag::Selection { .. } => None,
            Drag::Note { base, .. } | Drag::Clip { base, .. } | Drag::Gain { base, .. } => {
                Some(base.as_ref().clone())
            }
            Drag::Automation { seed, .. } => Some(seed.as_ref().clone()),
        };
        match drag {
            Drag::Window { index, dx, dy } => {
                let r = &mut self.workspace.panels[*index].rect;
                r.origin.x = (p.x - dx).clamp(209., self.width - 120.);
                r.origin.y = (p.y - dy).clamp(82., self.height - 60.);
            }
            Drag::Note {
                base,
                pattern,
                index,
                indices,
                x,
                y,
                width,
                row,
                resize,
            } => {
                if let Some(next) = &mut next {
                    let pat = &mut next.patterns[*pattern];
                    let old = base.patterns[*pattern].notes[*index];
                    let grid = self.workspace.note_grid as f64;
                    let delta = (((p.x - x) / width * PPQ as f64 / grid).round() * grid) as i64;
                    if *resize {
                        for i in indices {
                            let old = base.patterns[*pattern].notes[*i];
                            pat.notes[*i].length = (old.length as i64 + delta)
                                .clamp(1, (pat.length - old.tick) as i64)
                                as u32;
                        }
                    } else {
                        let starts = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].tick)
                            .min()
                            .unwrap_or(old.tick);
                        let ends = indices
                            .iter()
                            .map(|i| {
                                let n = base.patterns[*pattern].notes[*i];
                                n.tick + n.length
                            })
                            .max()
                            .unwrap_or(old.tick + old.length);
                        let min_key = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].key)
                            .min()
                            .unwrap_or(old.key);
                        let max_key = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].key)
                            .max()
                            .unwrap_or(old.key);
                        let delta = delta.clamp(-(starts as i64), (pat.length - ends) as i64);
                        let pitch = (-((p.y - y) / row).round() as i64)
                            .clamp(-(min_key as i64), 127 - max_key as i64);
                        for i in indices {
                            let old = base.patterns[*pattern].notes[*i];
                            pat.notes[*i].tick = (old.tick as i64 + delta) as u32;
                            pat.notes[*i].key = (old.key as i64 + pitch) as u8;
                        }
                    }
                }
            }
            Drag::Clip {
                base,
                ids,
                x,
                y,
                width,
                span,
                resize,
            } => {
                let delta = (((p.x - x) / width * *span as f64 / self.workspace.note_grid as f64)
                    .round() as i64)
                    * self.workspace.note_grid as i64;
                let tool = if *resize {
                    Tool::Resize(delta)
                } else {
                    Tool::Move {
                        ticks: delta,
                        lanes: ((p.y - y) / 48.).round() as i32,
                    }
                };
                match arrangement::apply(base, ids, tool) {
                    Ok(project) => next = Some(project),
                    Err(error) => self.status = error.to_string(),
                }
            }
            Drag::Selection {
                pattern,
                origin,
                grid,
                base_key,
                previous,
                ..
            } => {
                let left = p.x.min(origin.x).max(grid.origin.x);
                let right = p.x.max(origin.x).min(grid.origin.x + grid.size.width);
                let top = p.y.min(origin.y).max(grid.origin.y);
                let bottom = p.y.max(origin.y).min(grid.origin.y + grid.size.height);
                let high = *base_key + (grid.size.height / 18.).ceil().clamp(1., 128.) as u8 - 1;
                let start = self.workspace.piano_start.get() as f64;
                let zoom = self.workspace.piano_zoom.get();
                let mut selected = previous.clone();
                for (index, note) in self.history.project.patterns[*pattern]
                    .notes
                    .iter()
                    .enumerate()
                {
                    if !self.visible_note(note) || note.key < *base_key || note.key > high {
                        continue;
                    }
                    let x = grid.origin.x + (note.tick as f64 - start) / PPQ as f64 * zoom;
                    let end = x + note.length as f64 / PPQ as f64 * zoom;
                    let y = grid.origin.y + (high - note.key) as f64 * 18.;
                    if x < right && end > left && y < bottom && y + 18. > top {
                        selected.push(index);
                    }
                }
                selected.sort_unstable();
                selected.dedup();
                self.workspace.selected_notes = selected;
            }
            Drag::Automation {
                seed,
                lane,
                point,
                graph,
                ..
            } => {
                if let Some(next) = &mut next {
                    let end = seed.bars as u32 * BAR;
                    let points = &seed.automation[*lane].points;
                    let min = if *point == 0 {
                        0
                    } else {
                        points[*point - 1].0 + 1
                    };
                    let max = points.get(*point + 1).map_or(end, |p| p.0 - 1);
                    let tick = ((((p.x - graph.origin.x) / graph.size.width).clamp(0., 1.)
                        * end as f64
                        / (PPQ / 4) as f64)
                        .round() as u32
                        * (PPQ / 4))
                        .clamp(min, max);
                    let (low, high) = automation_range(seed.automation[*lane].target);
                    let value = low
                        + (1. - ((p.y - graph.origin.y) / graph.size.height).clamp(0., 1.) as f32)
                            * (high - low);
                    next.automation[*lane].points[*point] = (tick, value);
                }
            }
            Drag::PanView { x, start, zoom } => self.workspace.piano_start.set(
                (*start as f64 + (p.x - x) / zoom * PPQ as f64).clamp(0., (MAX_TICKS - BAR) as f64)
                    as u32,
            ),
            Drag::Gain {
                track, top, height, ..
            } => {
                if let Some(next) = &mut next {
                    next.tracks[*track].gain =
                        ((1. - (p.y - top) / height) * 2.).clamp(0., 2.) as f32;
                }
            }
        }

        if let Some(next) = next
            && next.validate().is_ok()
        {
            self.stop();
            self.history.project = next;
        }
        if let Some(Drag::Selection { current, .. }) = &mut self.workspace.drag {
            *current = p;
        }
        self.redraw();
    }
    pub(crate) fn scroll(&mut self, event: Input) {
        if self.workspace.drag.is_some() || !event.dx.is_finite() || !event.dy.is_finite() {
            return;
        }
        let point = event.point;
        let panel = self.workspace.panels[1];
        let topmost = self.workspace.order.iter().rev().copied().find(|&index| {
            let panel = self.workspace.panels[index];
            panel.visible && inside(panel.rect, point)
        });
        if topmost == Some(1) {
            let (g, key) = self.piano_geometry(panel.rect);
            let delta = event.dy;
            let dx = event.dx;
            let flags = event.modifiers;
            if !delta.is_finite() || !dx.is_finite() {
                return;
            }
            if flags & (1 << 20) != 0 {
                self.workspace.piano_zoom.set(
                    (self.workspace.piano_zoom.get() * (1. + delta * 0.02).clamp(0.5, 2.))
                        .clamp(24., 384.),
                );
            } else if flags & (1 << 17) != 0 || dx.abs() > delta.abs() {
                let delta = if dx.abs() > delta.abs() { dx } else { delta };
                self.workspace.piano_start.set(
                    (self.workspace.piano_start.get() as f64
                        - delta / self.workspace.piano_zoom.get() * PPQ as f64)
                        .clamp(0., (MAX_TICKS - BAR) as f64) as u32,
                );
            } else {
                let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
                let shift = (delta * 0.5).round().clamp(-12., 12.) as i16;
                self.workspace.piano_view.set(Some((
                    self.pattern().unwrap_or(usize::MAX),
                    (key as i16 + shift).clamp(0, (128 - rows) as i16) as u8,
                )));
            }
            self.redraw();
            return;
        }
        if self
            .workspace
            .panels
            .iter()
            .any(|panel| panel.visible && inside(panel.rect, point))
        {
            return;
        }
        let view = self.workspace.arrangement(self.width, self.height);
        if point.x >= 369. && point.y >= 128. {
            if event.modifiers & (1 << 20) != 0 {
                let anchor = view.tick_at(point.x) as f64;
                let span = (view.ticks as f64 * (-event.dy * 0.01).exp())
                    .clamp(BAR as f64, MAX_TICKS as f64) as u32;
                let fraction =
                    ((point.x - view.grid.origin.x) / view.grid.size.width).clamp(0., 1.);
                self.workspace.arrangement_start =
                    (anchor - fraction * span as f64).clamp(0., (MAX_TICKS - span) as f64) as u32;
                self.workspace.arrangement_span = span;
            } else if event.modifiers & (1 << 17) != 0 || event.dx.abs() > event.dy.abs() {
                let delta = if event.dx.abs() > event.dy.abs() {
                    event.dx
                } else {
                    event.dy
                };
                self.workspace.arrangement_start =
                    (view.start as f64 - delta / view.grid.size.width * view.ticks as f64)
                        .clamp(0., (MAX_TICKS - view.ticks) as f64) as u32;
            } else {
                let max_row = (MAX_LANES as f64 - view.grid.size.height / 48.).max(0.);
                self.workspace.arrangement_row =
                    (view.first_row - event.dy / 48.).clamp(0., max_row);
            }
            self.redraw();
        }
    }
    pub(crate) fn release(&mut self) {
        let Some(drag) = self.workspace.drag.take() else {
            return;
        };
        let (base, label) = match drag {
            Drag::Window { .. } | Drag::PanView { .. } | Drag::Selection { .. } => return,
            Drag::Note { base, resize, .. } => {
                (base, if resize { "Resize notes" } else { "Move notes" })
            }
            Drag::Clip { base, resize, .. } => {
                (base, if resize { "Resize clips" } else { "Move clips" })
            }
            Drag::Gain { base, .. } => (base, "Channel level"),
            Drag::Automation { base, .. } => (base, "Edit automation"),
        };
        let next = self.history.project.clone();
        self.history.project = *base;
        let result = self.history.edit_named(label, |p| {
            *p = next;
            Ok(())
        });
        if result.is_ok() {
            self.changed();
        }
        self.result(result);
    }
}
fn automation_range(target: Target) -> (f32, f32) {
    match target {
        Target::Pan(_) => (-1., 1.),
        Target::Gain(_) => (0., 2.),
        Target::Master => (0., 1.),
    }
}
