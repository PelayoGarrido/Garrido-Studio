use crate::app::*;
use crate::brand::{PLUM, ROSE, VIOLET};
use crate::canvas::*;
use crate::{controls, workspace::PIANO_TOOLS};
impl Studio {
    pub(crate) fn draw_workspace(&self, c: &Canvas) {
        c.fill(rect(0., 0., self.width, self.height), [0.145, 0.145, 0.145]);
        self.draw_toolbar(c);
        self.draw_browser(c);
        self.draw_playlist(c);
        for index in self.workspace.order.clone() {
            let panel = self.workspace.panels[index];
            if !panel.visible {
                continue;
            }
            let clipped = c.clipped(panel.rect);
            let c = &clipped;
            c.border(panel.rect);
            match index {
                0 => self.draw_rack(c, panel.rect),
                1 => self.draw_piano(c, panel.rect),
                _ => {
                    let r = panel.rect;
                    c.round(
                        rect(r.origin.x + 1., r.origin.y + 1., r.size.width - 2., 25.),
                        8.,
                        [0.20, 0.20, 0.20],
                    );
                    c.icon("chevron-right", r.origin.x + 8., r.origin.y + 5., 15., ICON);
                    c.text(
                        match index {
                            2 => "Mixer",
                            3 => "Audio editor",
                            _ => "Automation",
                        },
                        r.origin.x + 31.,
                        r.origin.y + 4.,
                        16.,
                    );
                    c.icon(
                        "x",
                        r.origin.x + r.size.width - 23.,
                        r.origin.y + 6.,
                        14.,
                        ICON,
                    );
                    match index {
                        2 => self.workspace_mixer(c, r),
                        3 => {
                            for (i, name) in [
                                "Reverse",
                                "Normalize",
                                "Fade in",
                                "Fade out",
                                "Trim",
                                "Stretch",
                                "Pitch",
                            ]
                            .iter()
                            .enumerate()
                            {
                                c.text(
                                    name,
                                    r.origin.x + 14. + i as f64 * 88.,
                                    r.origin.y + 31.,
                                    14.,
                                );
                            }
                            self.workspace_audio(c, r);
                        }
                        _ => self.workspace_automation(c, r),
                    }
                }
            }
        }
    }
    fn draw_toolbar(&self, c: &Canvas) {
        c.fill(rect(0., 0., self.width, 82.), [0.16; 3]);
        c.fill(rect(0., 48., self.width, 34.), [0.19; 3]);
        c.fill(rect(0., 80., self.width, 2.), EDGE);
        c.image(self.logo.id(), rect(16., 7., 36., 36.));
        c.label("GARRIDO STUDIO", 66., 4., 22., TEXT, true);
        c.label(
            &format!(
                "{}{}",
                self.history
                    .project
                    .title
                    .chars()
                    .take(36)
                    .collect::<String>(),
                if self.dirty { " · edited" } else { "" }
            ),
            67.,
            29.,
            13.,
            DIM,
            false,
        );
        for spec in controls::toolbar().iter() {
            let active = match spec.tag {
                9003 => !self.song_mode,
                9004 => self.song_mode,
                9005 => self.metronome,
                1 => self.playback.is_some() || self.job.is_some(),
                70 => self.capture.is_some(),
                100 => self.page == Page::Steps,
                101 => self.page == Page::Piano,
                102 => self.page == Page::Arrange,
                103 => self.page == Page::Mixer,
                104 => self.page == Page::Audio,
                105 => self.page == Page::Automation,
                _ => false,
            };
            let tint = if active { ACCENT } else { [0.22; 3] };
            let ink = if active { PLUM } else { TEXT };
            c.round(spec.r, 8., tint);
            let x = spec.r.origin.x;
            let y = spec.r.origin.y;
            let icon = match spec.tag {
                1 => Some("player-play"),
                2 => Some("player-stop"),
                70 => Some("microphone"),
                31 => Some("minus"),
                32 => Some("plus"),
                9005 => Some("metronome"),
                9351 => Some("help-circle"),
                83 => Some("chevron-left"),
                84 => Some("chevron-right"),
                _ => None,
            };
            if let Some(icon) = icon {
                c.icon(
                    icon,
                    x + (spec.r.size.width - 18.) / 2.,
                    y + (spec.r.size.height - 18.) / 2.,
                    18.,
                    ink,
                );
            } else {
                let title = match spec.tag {
                    2006 => self
                        .pattern()
                        .map_or("Choose pattern", |i| {
                            self.history.project.patterns[i].name.as_str()
                        })
                        .chars()
                        .take(25)
                        .collect::<String>(),
                    9030 => format!(
                        "Snap · {}",
                        match self.workspace.note_grid {
                            1 => "Off",
                            PPQ => "Beat",
                            BAR => "Bar",
                            _ => "1/4",
                        }
                    ),
                    9003 => "PATTERN".into(),
                    9004 => "SONG".into(),
                    _ => spec.name.into(),
                };
                c.label(&title, x + 10., y + 4., 16., ink, active);
            }
        }
        c.label(
            &format!("{:03} BPM", self.history.project.bpm.round() as u16),
            839.,
            16.,
            17.,
            TEXT,
            true,
        );
        let tick = self.playback.as_ref().map_or(0., |p| {
            self.history
                .project
                .tick_at(p.frame() as f64 / self.history.project.rate as f64)
        });
        let time = format!(
            "{}:{:02}:{:02}",
            (tick / BAR as f64) as u32 + 1,
            (tick / PPQ as f64) as u32 % 4 + 1,
            (tick % PPQ as f64 / 10.) as u32
        );
        c.round(rect(1010., 8., 176., 36.), 10., [0.10; 3]);
        c.segments(&time, 1030., 14., 10., 21.);
        c.label("BAR / BEAT", 1204., 17., 12., DIM, false);
        c.label(
            &self.status.chars().take(25).collect::<String>(),
            1310.,
            18.,
            12.,
            DIM,
            false,
        );
    }
    fn draw_browser(&self, c: &Canvas) {
        c.fill(
            rect(0., 82., 205., self.height - 82.),
            [0.135, 0.135, 0.135],
        );
        c.fill(rect(0., 82., 205., 24.), [0.16, 0.16, 0.16]);
        c.label("LIBRARY", 16., 85., 15., DIM, true);
        for (i, title) in ["Audio", "Session", "Plugins"].iter().enumerate() {
            let x = 10. + i as f64 * 64.;
            let selected = self.workspace.browser_tab as usize == i;
            c.round(
                rect(x, 108., 58., 30.),
                7.,
                if selected { [0.27; 3] } else { [0.17; 3] },
            );
            c.label(
                title,
                x + 7.,
                114.,
                14.,
                if selected { ACCENT } else { TEXT },
                selected,
            );
        }
        c.fill(
            rect(13., 141., 188., self.height - 168.),
            [0.117, 0.117, 0.117],
        );
        c.icon(
            if self.workspace.browser_expanded {
                "chevron-down"
            } else {
                "chevron-right"
            },
            3.,
            147.,
            11.,
            DIM,
        );
        c.icon("file-description", 21., 146., 15., ACCENT);
        c.label("This session", 39., 142., 16., ACCENT, false);
        let mut y = 166.;
        if self.workspace.browser_expanded {
            c.icon("chevron-down", 16., 170., 11., DIM);
            c.icon("music", 33., 168., 16., ACCENT);
            c.label("Patterns", 51., 165., 16., ACCENT, false);
            y = 190.;
            for (i, pattern) in self
                .history
                .project
                .patterns
                .iter()
                .enumerate()
                .take(((self.height - 330.) / 23.).max(0.) as usize)
            {
                c.icon("music", 47., y + 2., 14., ACCENT);
                c.label(
                    &pattern.name.chars().take(23).collect::<String>(),
                    64.,
                    y,
                    16.,
                    ACCENT,
                    false,
                );
                if Some(i) == self.pattern() {
                    c.fill(rect(14., y, 2., 20.), ACCENT);
                }
                y += 23.;
            }
            for (name, title) in [
                ("volume", "Effects"),
                ("tools", "Generators"),
                ("wave-sine", "Samples"),
            ] {
                c.icon(name, 33., y + 2., 16., ACCENT);
                c.label(title, 52., y, 16., ACCENT, false);
                y += 23.;
            }
        }
        c.icon("volume", 20., y + 3., 15., ICON);
        c.label("Plugin database", 39., y, 16., ICON, false);
        c.fill(rect(201., 82., 7., self.height - 82.), [0.19, 0.19, 0.19]);
        c.fill(rect(207., 82., 2., self.height - 82.), [0.12, 0.12, 0.12]);
        c.label(
            &format!(
                "{} channels · {} patterns",
                self.history.project.tracks.len(),
                self.history.project.patterns.len()
            ),
            16.,
            self.height - 24.,
            12.,
            DIM,
            false,
        );
    }
    fn draw_playlist(&self, c: &Canvas) {
        let h = self.height;
        c.fill(rect(209., 82., self.width - 209., 24.), [0.19, 0.19, 0.19]);
        c.fill(rect(209., 105., self.width - 209., 1.), EDGE);
        c.label("ARRANGEMENT", 226., 85., 15., TEXT, true);
        c.label(
            &format!(
                "{} bars · {} clips",
                self.history.project.bars,
                self.history.project.clips.len() + self.history.project.audio_clips.len()
            ),
            490.,
            85.,
            15.,
            DIM,
            false,
        );
        c.fill(rect(209., 106., 159., h - 106.), [0.15, 0.15, 0.15]);
        c.label("PATTERNS", 225., 123., 14., DIM, true);
        c.fill(rect(350., 148., 18., h - 148.), [0.17, 0.17, 0.17]);
        c.fill(rect(367., 107., 2., h - 107.), [0.20, 0.20, 0.20]);
        let start = self.pattern().unwrap_or(0).saturating_sub(20);
        for (i, p) in self
            .history
            .project
            .patterns
            .iter()
            .enumerate()
            .skip(start)
            .take(((h - 180.) / 32.) as usize)
        {
            let y = 154. + (i - start) as f64 * 32.;
            if Some(i) == self.pattern() {
                c.round(rect(212., y, 135., 29.), 5., [0.20, 0.20, 0.20]);
                c.outline(rect(212., y, 135., 29.), 5., 1.2, ACCENT);
            }
            c.label(
                &p.name.chars().take(22).collect::<String>(),
                225.,
                y + 5.,
                14.,
                TEXT,
                false,
            );
            if Some(i) == self.pattern() {
                c.circle(215., y + 14., 1.5, ACCENT);
            }
        }
        c.icon("chevron-left", 355., 152., 10., DIM);
        c.icon("plus", 272., h - 16., 12., DIM);
        c.fill(rect(369., 106., 111., 42.), [0.17, 0.17, 0.17]);
        c.icon("plus", 379., 126., 18., TEXT);
        for (s, x) in [("NOTE", 407.), ("CHAN", 435.), ("PAT", 461.)] {
            c.label(s, x, 133., 8., DIM, true);
        }
        c.thumb(rect(485., 110., 330., 14.), 7.);
        let gx = 481.;
        let gw = self.width - gx - 22.;
        let view = self.workspace.arrangement(self.width, self.height);
        let grid = c.clipped(view.grid);
        c.fill(rect(gx, 128., gw, 20.), [0.13, 0.13, 0.13]);
        let first = view.start / (PPQ / 4);
        let last = (view.start + view.ticks).div_ceil(PPQ / 4);
        for step in first..=last {
            let xx = view.x_at(step * (PPQ / 4));
            if xx < gx || xx > gx + gw {
                continue;
            }
            if step % 16 == 0 {
                let bar = step / 16;
                c.label(&(bar + 1).to_string(), xx + 5., 130., 14., DIM, false);
            }
            if step % 4 == 0 {
                grid.fill(
                    rect(xx, 148., if step % 16 == 0 { 1.7 } else { 0.6 }, h - 148.),
                    if step % 16 == 0 {
                        [0.09, 0.09, 0.09]
                    } else {
                        [0.115, 0.115, 0.115]
                    },
                );
            }
        }
        let headers = c.clipped(rect(369., 148., 112., h - 148.));
        for row in view.first_row.floor() as usize..MAX_LANES {
            let y = view.y_at(row);
            if y >= h {
                break;
            }
            headers.round(rect(371., y + 2., 98., 45.), 5., [0.20, 0.20, 0.20]);
            headers.label(
                &format!("Track {}", row + 1),
                375.,
                y + 3.,
                14.,
                [0.48, 0.48, 0.48],
                false,
            );
            headers.label("...", 375., y + 26., 12., [0.34, 0.34, 0.34], false);
            headers.round(
                rect(453., y + 33., 9., 9.),
                2.,
                if row < MAX_LANES && self.history.project.arrangement_mutes & (1u32 << row) != 0 {
                    DIM
                } else {
                    ACCENT
                },
            );
            grid.fill(rect(gx, y + 46., gw, 2.), [0.095, 0.095, 0.095]);
        }
        let mut preview_budget = 8192usize;
        for (index, clip) in self.history.project.clips.iter().enumerate() {
            let y = view.y_at(clip.track);
            if y + 47. < 148. || y >= h {
                continue;
            }
            let x = view.x_at(clip.tick);
            let width = clip.length as f64 / view.ticks as f64 * gw;
            if x + width < gx || x > gx + gw {
                continue;
            }
            grid.round(
                rect(x + 1., y + 1., (width - 2.).max(2.), 44.),
                2.,
                if clip.muted {
                    [0.18, 0.18, 0.18]
                } else {
                    [0.30, 0.34, 0.30]
                },
            );
            if self
                .workspace
                .selected_clips
                .contains(&garrido_studio::arrangement::ClipId::Pattern(index))
            {
                grid.outline(
                    rect(x + 1., y + 1., (width - 2.).max(2.), 44.),
                    2.,
                    1.5,
                    ACCENT,
                );
            }
            let pattern = &self.history.project.patterns[clip.pattern];
            grid.label(
                &pattern
                    .name
                    .chars()
                    .take((width / 6.).max(1.) as usize)
                    .collect::<String>(),
                (x + 4.).max(gx + 4.),
                y + 3.,
                13.,
                TEXT,
                false,
            );
            if !pattern.notes.is_empty() && preview_budget > 0 {
                let first = (view.start.saturating_sub(clip.tick) + clip.offset) / pattern.length;
                let last = (view.start + view.ticks)
                    .min(clip.tick + clip.length)
                    .saturating_sub(clip.tick)
                    .saturating_add(clip.offset)
                    .div_ceil(pattern.length);
                'previews: for repeat in first..last {
                    for note in pattern.notes.iter().take(128) {
                        if preview_budget == 0 {
                            break 'previews;
                        }
                        preview_budget -= 1;
                        let tick = clip.tick as i64 + (repeat * pattern.length + note.tick) as i64
                            - clip.offset as i64;
                        if tick >= clip.tick as i64 && tick < (clip.tick + clip.length) as i64 {
                            let tick = tick as u32;
                            grid.fill(
                                rect(view.x_at(tick), y + 38. - (note.key % 12) as f64, 3., 1.),
                                [0.65, 0.71, 0.65],
                            );
                        }
                    }
                }
            }
        }
        let mut audio_budget = 8192usize;
        for (index, clip) in self.history.project.audio_clips.iter().enumerate() {
            let y = view.y_at(clip.track);
            let x = view.x_at(clip.tick);
            let right = view.x_at(clip.tick + clip.length);
            if y + 47. < 148. || y >= h || right < gx || x > gx + gw {
                continue;
            }
            grid.round(
                rect(x + 1., y + 1., (right - x - 2.).max(2.), 44.),
                2.,
                if clip.muted {
                    [0.18, 0.18, 0.18]
                } else {
                    [0.31, 0.23, 0.34]
                },
            );
            if self
                .workspace
                .selected_clips
                .contains(&garrido_studio::arrangement::ClipId::Audio(index))
            {
                grid.outline(
                    rect(x + 1., y + 1., (right - x - 2.).max(2.), 44.),
                    2.,
                    1.5,
                    ACCENT,
                );
            }
            let track = &self.history.project.tracks[clip.channel.index()];
            grid.label(
                &track
                    .name
                    .chars()
                    .take(((right - x) / 6.).max(1.) as usize)
                    .collect::<String>(),
                (x + 4.).max(gx + 4.),
                y + 3.,
                13.,
                TEXT,
                false,
            );
            if let Some(sample) = &track.sample {
                let left = x.max(gx);
                let columns = ((right.min(gx + gw) - left) / 3.).ceil().clamp(0., 1024.) as usize;
                let columns = columns.min(audio_budget);
                audio_budget -= columns;
                let origin = self.history.project.seconds_at(clip.origin as f64);
                for column in 0..columns {
                    let xx = left + column as f64 * 3.;
                    let tick = view.tick_at(xx).max(clip.tick);
                    let pos = clip.offset
                        + (self.history.project.seconds_at(tick as f64) - origin)
                            * sample.rate as f64;
                    let end_tick = view.tick_at(xx + 3.).min(clip.tick + clip.length);
                    let end = clip.offset
                        + (self.history.project.seconds_at(end_tick as f64) - origin)
                            * sample.rate as f64;
                    let start = (pos.max(0.) as usize).min(sample.frames.len());
                    let end = (end.ceil() as usize)
                        .max(start + 1)
                        .min(sample.frames.len());
                    let stride = (end.saturating_sub(start) / 16).max(1);
                    let peak = sample.frames[start..end]
                        .iter()
                        .step_by(stride)
                        .take(17)
                        .flatten()
                        .fold(0f32, |a, &v| a.max(v.abs()))
                        .min(1.) as f64;
                    grid.fill(
                        rect(xx, y + 31. - peak * 9., 1., (peak * 18.).max(1.)),
                        if clip.muted { DIM } else { [0.87, 0.60, 0.78] },
                    );
                }
            }
            if clip.group != 0 {
                grid.label("G", (right - 12.).max(x + 4.), y + 29., 10., DIM, true);
            }
        }
        if self.page == Page::Arrange {
            let x = view.x_at(self.workspace.arrangement_cursor);
            grid.fill(rect(x, 148., 1., h - 148.), ACCENT);
        }
        c.fill(
            rect(self.width - 22., 128., 22., h - 128.),
            [0.19, 0.19, 0.19],
        );
        let max_row = (MAX_LANES as f64 - view.grid.size.height / 48.).max(0.);
        let thumb_y = 149. + view.first_row / max_row.max(1.) * (h - 148. - 34.).max(1.);
        c.thumb(rect(self.width - 15., thumb_y, 13., 34.), 6.);
        c.icon("player-play", 477., 126., 12., ACCENT);
        if let Some(play) = &self.playback {
            let tick = self
                .history
                .project
                .tick_at(play.frame() as f64 / self.history.project.rate as f64);
            grid.fill(rect(view.x_at(tick as u32), 148., 1., h - 148.), ACCENT);
        }
    }
    fn draw_rack(&self, c: &Canvas, r: Rect) {
        let x = r.origin.x;
        let y = r.origin.y;
        c.round(
            rect(x + 3., y + 29., r.size.width - 7., r.size.height - 57.),
            7.,
            [0.19, 0.19, 0.19],
        );
        c.outline(
            rect(x + 3., y + 29., r.size.width - 7., r.size.height - 57.),
            7.,
            1.,
            [0.14, 0.14, 0.14],
        );
        c.icon("music", x + 12., y + 7., 16., ACCENT);
        c.label("STEP SEQUENCER", x + 38., y + 4., 17., TEXT, true);
        c.label(
            "16 steps / pattern",
            x + r.size.width - 210.,
            y + 6.,
            14.,
            DIM,
            false,
        );
        c.icon("x", x + r.size.width - 22., y + 7., 14., ICON);
        let start = (self.selected / 6) * 6;
        for (index, track) in self
            .history
            .project
            .tracks
            .iter()
            .enumerate()
            .skip(start)
            .take(6)
        {
            let yy = y + 48. + (index - start) as f64 * 28.;
            c.round(
                rect(x + 14., yy + 8., 7., 7.),
                1.,
                if track.mute { DIM } else { ACCENT },
            );
            c.knob(x + 36., yy + 12., 6., (track.pan as f64 + 1.) * 0.5, true);
            c.knob(x + 52., yy + 12., 6., track.gain as f64 / 2., true);
            c.round(rect(x + 66., yy + 2., 33., 24.), 3., [0.25, 0.25, 0.25]);
            c.label(&(index + 1).to_string(), x + 80., yy + 5., 14., TEXT, false);
            c.icon("chevron-up", x + 91., yy + 4., 7., DIM);
            c.icon("chevron-down", x + 91., yy + 15., 7., DIM);
            c.round(rect(x + 108., yy + 1., 93., 25.), 4., [0.23, 0.23, 0.23]);
            if index == self.selected {
                c.outline(rect(x + 108., yy + 1., 93., 25.), 4., 1., ACCENT);
            }
            c.label(
                &track.name.chars().take(19).collect::<String>(),
                x + 114.,
                yy + 5.,
                13.,
                [0.87, 0.87, 0.87],
                false,
            );
            if index == self.selected {
                c.outline(rect(x + 211., yy, 8., 26.), 4., 1.2, ACCENT);
            }
            let pat = self.pattern();
            for step in 0..16u32 {
                let on = pat.is_some_and(|i| {
                    self.history.project.patterns[i]
                        .notes
                        .iter()
                        .any(|n| n.channel_or(index) == index && n.tick / (PPQ / 4) == step)
                });
                c.round(
                    rect(
                        x + 229. + step as f64 * workspace::step_width(r),
                        yy + 4.,
                        workspace::step_width(r) - 5.,
                        19.,
                    ),
                    3.,
                    if on {
                        ACCENT
                    } else if step / 4 % 2 == 0 {
                        [0.26, 0.26, 0.26]
                    } else {
                        [0.31, 0.28, 0.12]
                    },
                );
            }
        }
        c.icon("plus", x + 150., y + r.size.height - 24., 14., TEXT);
        c.round(
            rect(x + 220., y + r.size.height - 11., 15., 4.),
            2.,
            [0.28, 0.28, 0.28],
        );
    }
    fn draw_piano(&self, c: &Canvas, r: Rect) {
        let x = r.origin.x;
        let y = r.origin.y;
        let maximized = self.workspace.piano_maximized;
        if maximized {
            c.fill(r, [0.18, 0.18, 0.18]);
        }
        for (i, (name, offset)) in PIANO_TOOLS.iter().enumerate() {
            let xx = x + offset;
            if (i == 6 && !self.workspace.piano_erase && !self.workspace.piano_select)
                || (i == 9 && self.workspace.piano_erase)
                || (i == 12 && self.workspace.piano_select)
            {
                c.round(rect(xx - 1., y + 4., 20., 20.), 4., ACCENT);
                c.icon(name, xx + 1., y + 6., 17., PLUM);
            } else {
                c.icon(name, xx, y + 6., 16., ICON);
            }
        }
        let track = self
            .history
            .project
            .tracks
            .get(self.selected)
            .map_or("Garrido Sampler", |t| t.name.as_str());
        c.label(
            &format!("Piano roll - {track}"),
            x + 370.,
            y + 3.,
            16.,
            [0.87, 0.87, 0.87],
            false,
        );
        for (name, xx) in [
            ("minus", x + r.size.width - 65.),
            ("copy", x + r.size.width - 45.),
            ("x", x + r.size.width - 25.),
        ] {
            c.icon(name, xx, y + 7., 13., ICON);
        }
        let (g, base) = self.piano_geometry(r);
        let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
        let top = base + rows - 1;
        let key_x = if maximized { x } else { x + 6. };
        c.fill(
            rect(key_x, g.origin.y, g.origin.x - key_x - 2., g.size.height),
            [0.92, 0.92, 0.92],
        );
        let pattern = self
            .pattern()
            .and_then(|i| self.history.project.patterns.get(i));
        let pixels = self.workspace.piano_zoom.get();
        let start = self.workspace.piano_start.get() as f64;
        let span = g.size.width / pixels * PPQ as f64;
        c.fill(g, [0.145, 0.145, 0.145]);
        for row in 0..rows {
            let key = top - row;
            let yy = g.origin.y + row as f64 * 18.;
            let height = (g.origin.y + g.size.height - yy).min(18.);
            let black = matches!(key % 12, 1 | 3 | 6 | 8 | 10);
            c.fill(
                rect(g.origin.x, yy, g.size.width, height),
                if black {
                    [0.135, 0.135, 0.135]
                } else {
                    [0.16, 0.16, 0.16]
                },
            );
            c.fill(
                rect(g.origin.x, yy, g.size.width, 0.6),
                [0.105, 0.105, 0.105],
            );
            if black {
                c.piano_key(rect(key_x, yy + 1., 45., (height - 1.).max(0.5)));
            } else {
                c.fill(
                    rect(key_x, yy + height - 0.5, g.origin.x - key_x - 2., 0.5),
                    [0.61, 0.61, 0.61],
                );
                if key.is_multiple_of(12) {
                    c.label(&name(key), g.origin.x - 17., yy + 2., 10., PLUM, false);
                }
            }
        }
        let first = (start / (PPQ / 4) as f64).floor() as u32;
        let last = ((start + span) / (PPQ / 4) as f64).ceil() as u32;
        for step in first..=last {
            let xx = g.origin.x + (step as f64 * (PPQ / 4) as f64 - start) / PPQ as f64 * pixels;
            if xx < g.origin.x || xx > g.origin.x + g.size.width {
                continue;
            }
            c.fill(
                rect(
                    xx,
                    g.origin.y,
                    if step % 16 == 0 { 1.2 } else { 0.6 },
                    g.size.height,
                ),
                if step % 16 == 0 {
                    [0.095, 0.095, 0.095]
                } else {
                    [0.10, 0.10, 0.10]
                },
            );
            if step % 16 == 0 {
                c.label(
                    &(step / 16 + 1).to_string(),
                    xx + 4.,
                    g.origin.y - 18.,
                    14.,
                    DIM,
                    false,
                );
            }
        }
        let overview_y = y + if maximized { 26. } else { 29. };
        let extent = pattern
            .map_or(BAR as f64, |p| p.length as f64)
            .max(start + span * 1.5);
        let rail = g.size.width - 26.;
        let thumb = (rail * span / extent).max(18.);
        c.round(rect(key_x, overview_y, 68., 34.), 6., [0.48, 0.43, 0.24]);
        c.icon("chevron-left", g.origin.x + 2., overview_y + 3., 13., DIM);
        c.thumb(
            rect(
                g.origin.x + 9. + start / extent * rail,
                overview_y + 1.,
                thumb,
                17.,
            ),
            8.,
        );
        if let Some(pattern) = pattern {
            let low = pattern
                .notes
                .iter()
                .filter(|n| self.visible_note(n))
                .map(|n| n.key)
                .min()
                .unwrap_or(0);
            let high = pattern
                .notes
                .iter()
                .filter(|n| self.visible_note(n))
                .map(|n| n.key)
                .max()
                .unwrap_or(low);
            for note in pattern
                .notes
                .iter()
                .filter(|n| self.visible_note(n))
                .take(4096)
            {
                let a = g.origin.x + 9. + note.tick as f64 / extent * rail;
                let b = g.origin.x + 9. + (note.tick + note.length) as f64 / extent * rail;
                let yy =
                    overview_y + 14. - (note.key - low) as f64 / (high - low).max(1) as f64 * 10.;
                c.line(
                    Point { x: a, y: yy },
                    Point {
                        x: b.max(a + 1.),
                        y: yy,
                    },
                    1.,
                    ACCENT,
                );
            }
        }
        c.icon(
            "chevron-right",
            g.origin.x + g.size.width - 14.,
            overview_y + 3.,
            13.,
            DIM,
        );
        c.fill(
            rect(g.origin.x, g.origin.y - 19., g.size.width, 19.),
            [0.13, 0.13, 0.13],
        );
        for bar in (start as u32 / BAR)..=((start + span) as u32 / BAR) {
            let xx = g.origin.x + (bar as f64 * BAR as f64 - start) / PPQ as f64 * pixels;
            if xx >= g.origin.x {
                c.label(
                    &(bar + 1).to_string(),
                    xx + 4.,
                    g.origin.y - 18.,
                    14.,
                    DIM,
                    false,
                );
            }
        }
        c.fill(
            rect(g.origin.x - 3., g.origin.y, 2., g.size.height),
            [0.08, 0.08, 0.08],
        );
        let scroll_x = g.origin.x + g.size.width + 5.;
        c.fill(
            rect(scroll_x, g.origin.y - 19., 18., g.size.height + 19.),
            [0.19, 0.19, 0.19],
        );
        c.icon("chevron-up", scroll_x + 1., g.origin.y - 16., 15., DIM);
        c.icon(
            "chevron-down",
            scroll_x + 1.,
            g.origin.y + g.size.height - 16.,
            15.,
            DIM,
        );
        let vh = (g.size.height * rows as f64 / 128.).clamp(28., g.size.height - 40.);
        let vy = g.origin.y
            + 18.
            + (127 - top) as f64 / (128 - rows).max(1) as f64 * (g.size.height - vh - 36.);
        c.thumb(rect(scroll_x + 2., vy, 14., vh), 7.);
        c.fill(
            rect(x + 6., g.origin.y + g.size.height, r.size.width - 12., 8.),
            [0.20, 0.20, 0.20],
        );
        let lane_y = g.origin.y + g.size.height + 9.;
        let bottom = y + r.size.height - 7.;
        c.fill(
            rect(g.origin.x, lane_y, g.size.width, (bottom - lane_y).max(1.)),
            [0.14, 0.14, 0.14],
        );
        c.label("Control ›", x + 16., lane_y + 4., 14., TEXT, false);
        c.label(
            "Velocity",
            g.origin.x + 8.,
            lane_y + 4.,
            14.,
            [0.24, 0.24, 0.24],
            false,
        );
        for step in first..=last {
            let xx = g.origin.x + (step as f64 * (PPQ / 4) as f64 - start) / PPQ as f64 * pixels;
            if xx >= g.origin.x && xx <= g.origin.x + g.size.width {
                c.fill(rect(xx, lane_y, 0.6, bottom - lane_y), [0.09, 0.09, 0.09]);
            }
        }
        for i in 1..8 {
            c.fill(
                rect(
                    g.origin.x,
                    lane_y + (bottom - lane_y) * i as f64 / 8.,
                    g.size.width,
                    0.6,
                ),
                [0.11, 0.11, 0.11],
            );
        }
        if let Some(pattern) = pattern {
            if self.workspace.ghost_notes {
                let notes = c.clipped(g);
                for n in pattern
                    .notes
                    .iter()
                    .filter(|n| !self.visible_note(n))
                    .take(4096)
                {
                    if n.key >= base && n.key <= top {
                        let xx = g.origin.x + (n.tick as f64 - start) / PPQ as f64 * pixels;
                        let width = n.length as f64 / PPQ as f64 * pixels;
                        let yy = g.origin.y + (top - n.key) as f64 * 18.;
                        notes.round(
                            rect(xx + 0.4, yy + 0.6, (width - 0.8).max(1.), 17.),
                            2.5,
                            [0.26, 0.28, 0.26],
                        );
                    }
                }
            }
            for (index, n) in pattern
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| self.visible_note(n))
                .take(4096)
            {
                let a = n.tick as f64;
                let end = (n.tick + n.length) as f64;
                if end < start || a > start + span {
                    continue;
                }
                let xx = g.origin.x + (a - start) / PPQ as f64 * pixels;
                let ww = n.length as f64 / PPQ as f64 * pixels;
                if n.key >= base && n.key <= top {
                    let yy = g.origin.y + (top - n.key) as f64 * 18.;
                    let left = xx.max(g.origin.x);
                    let width = (xx + ww).min(g.origin.x + g.size.width) - left;
                    if width > 0. {
                        c.round(
                            rect(left + 0.4, yy + 0.6, (width - 0.8).max(1.), 17.),
                            2.5,
                            if self.workspace.selected_notes.contains(&index) {
                                ROSE
                            } else {
                                ACCENT
                            },
                        );
                        c.outline(
                            rect(left + 0.4, yy + 0.6, (width - 0.8).max(1.), 17.),
                            2.5,
                            0.6,
                            VIOLET,
                        );
                        if width > 17. {
                            c.label(&name(n.key), left + 3., yy + 2., 10., PLUM, false);
                        }
                    }
                }
                if xx >= g.origin.x && xx <= g.origin.x + g.size.width {
                    let yy = bottom - n.velocity as f64 * (bottom - lane_y - 25.);
                    c.line(
                        Point { x: xx, y: bottom },
                        Point { x: xx, y: yy },
                        1.,
                        ACCENT,
                    );
                    c.line(
                        Point { x: xx, y: yy },
                        Point {
                            x: (xx + ww).min(g.origin.x + g.size.width),
                            y: yy,
                        },
                        0.8,
                        ACCENT,
                    );
                    c.circle(xx, yy, 3., ACCENT);
                    c.circle(xx, yy, 2., [0.17, 0.17, 0.17]);
                }
            }
        }
        if let Some(crate::workspace::Drag::Selection {
            origin, current, ..
        }) = &self.workspace.drag
        {
            let left = origin.x.min(current.x).max(g.origin.x);
            let top = origin.y.min(current.y).max(g.origin.y);
            let right = origin.x.max(current.x).min(g.origin.x + g.size.width);
            let bottom = origin.y.max(current.y).min(g.origin.y + g.size.height);
            c.outline(
                rect(left, top, (right - left).max(0.), (bottom - top).max(0.)),
                1.,
                1.,
                ACCENT,
            );
        }
    }
    fn workspace_mixer(&self, c: &Canvas, r: Rect) {
        let start = (self.selected / 12) * 12;
        let count = self
            .history
            .project
            .tracks
            .len()
            .saturating_sub(start)
            .clamp(1, 12);
        let cw = (r.size.width - 15.) / count as f64;
        let top = r.origin.y + 52.;
        let height = r.size.height - 125.;
        for (i, t) in self
            .history
            .project
            .tracks
            .iter()
            .enumerate()
            .skip(start)
            .take(12)
        {
            let x = r.origin.x + 7. + (i - start) as f64 * cw;
            c.fill(
                rect(x, top - 25., cw - 3., r.size.height - 32.),
                [0.16, 0.17, 0.175],
            );
            c.text(
                &t.name.chars().take(10).collect::<String>(),
                x + 5.,
                top - 18.,
                10.,
            );
            c.fill(rect(x + cw * 0.48, top, 3., height), [0.1, 0.11, 0.12]);
            let yy = top + (1. - t.gain as f64 / 2.) * height;
            c.fill(
                rect(x + cw * 0.25, yy, cw * 0.5, 6.),
                if i == self.selected { ACCENT } else { DIM },
            );
            c.text(
                &format!("{:.1}", 20. * t.gain.max(1e-6).log10()),
                x + 4.,
                top + height + 12.,
                9.,
            );
            c.text(
                if t.mute { "M ON" } else { "M" },
                x + 5.,
                top + height + 34.,
                10.,
            );
            c.text(
                if t.solo { "S ON" } else { "S" },
                x + cw * 0.55,
                top + height + 34.,
                10.,
            );
        }
    }
    fn workspace_audio(&self, c: &Canvas, r: Rect) {
        let Some(s) = self
            .history
            .project
            .tracks
            .get(self.selected)
            .and_then(|t| t.sample.as_ref())
        else {
            c.text(
                "Load a WAV or record audio into the selected channel.",
                r.origin.x + 20.,
                r.origin.y + 80.,
                12.,
            );
            return;
        };
        let g = rect(
            r.origin.x + 10.,
            r.origin.y + 55.,
            r.size.width - 20.,
            r.size.height - 100.,
        );
        c.fill(g, [0.12, 0.13, 0.135]);
        if self.selection.1 > self.selection.0 {
            c.fill(
                rect(
                    g.origin.x + g.size.width * self.selection.0,
                    g.origin.y,
                    g.size.width * (self.selection.1 - self.selection.0),
                    g.size.height,
                ),
                [0.24, 0.25, 0.20],
            );
        }
        for px in 0..g.size.width as usize {
            let a = px * s.frames.len() / g.size.width as usize;
            let b = ((px + 1) * s.frames.len() / g.size.width as usize)
                .max(a + 1)
                .min(s.frames.len());
            if a >= b {
                continue;
            }
            let peak = s.frames[a..b]
                .iter()
                .flatten()
                .fold(0f32, |a, &b| a.max(b.abs()));
            c.fill(
                rect(
                    g.origin.x + px as f64,
                    g.origin.y + g.size.height * (0.5 - peak as f64 * 0.45),
                    1.,
                    (peak as f64 * g.size.height * 0.9).max(1.),
                ),
                ACCENT,
            );
        }
        c.text(
            &format!(
                "{:.2}s / {} Hz / original preserved",
                s.frames.len() as f64 / s.rate as f64,
                s.rate
            ),
            r.origin.x + 12.,
            r.origin.y + r.size.height - 28.,
            10.,
        );
    }
    fn workspace_automation(&self, c: &Canvas, r: Rect) {
        let target = self.automation_target();
        let graph = self.automation_graph(r);
        let end = self.history.project.bars as u32 * BAR;
        let lane = self
            .history
            .project
            .automation
            .iter()
            .find(|a| a.target == target);
        let curve = lane.map_or(self.workspace.automation_curve, |a| a.curve);
        for (i, title) in ["Gain", "Pan", "Master"].iter().enumerate() {
            c.label(
                title,
                r.origin.x + 12. + i as f64 * 68.,
                r.origin.y + 29.,
                11.,
                if self.workspace.automation_target as usize == i {
                    ACCENT
                } else {
                    TEXT
                },
                false,
            );
        }
        c.text(
            &format!(
                "Curve: {}",
                ["Hold", "Linear", "Smooth", "Exponential"][curve as usize]
            ),
            r.origin.x + 235.,
            r.origin.y + 29.,
            10.,
        );
        c.fill(graph, [0.12, 0.13, 0.135]);
        let (low, high) = automation_range(target);
        for i in 0..=4 {
            let y = graph.origin.y + i as f64 * graph.size.height / 4.;
            c.fill(
                rect(graph.origin.x, y, graph.size.width, 0.5),
                [0.20, 0.21, 0.22],
            );
            c.text(
                &format!("{:.2}", high - (high - low) * i as f32 / 4.),
                r.origin.x + 5.,
                y - 5.,
                9.,
            );
        }
        for bar in 0..=self.history.project.bars {
            let x =
                graph.origin.x + bar as f64 / self.history.project.bars as f64 * graph.size.width;
            c.fill(rect(x, graph.origin.y, 0.5, graph.size.height), EDGE);
            if self.history.project.bars <= 32 || bar % 8 == 0 {
                c.text(&(bar + 1).to_string(), x + 2., graph.origin.y - 17., 9.);
            }
        }
        if let Some(lane) = lane {
            let mut previous = None;
            for px in 0..graph.size.width as usize {
                let v = lane.at(px as f64 / graph.size.width * end as f64);
                let y =
                    graph.origin.y + (1. - ((v - low) / (high - low)) as f64) * graph.size.height;
                let old = previous.unwrap_or(y);
                c.fill(
                    rect(
                        graph.origin.x + px as f64,
                        old.min(y),
                        1.,
                        (old - y).abs().max(1.),
                    ),
                    ACCENT,
                );
                previous = Some(y);
            }
            for &(tick, v) in &lane.points {
                let x = graph.origin.x + tick as f64 / end as f64 * graph.size.width;
                let y =
                    graph.origin.y + (1. - ((v - low) / (high - low)) as f64) * graph.size.height;
                c.fill(rect(x - 3., y - 3., 6., 6.), [1., 0.91, 0.59]);
            }
        }
        c.text(
            "Click to add · drag to move · right-click to delete",
            r.origin.x + 12.,
            r.origin.y + r.size.height - 25.,
            10.,
        );
    }
    pub(crate) fn piano_geometry(&self, r: Rect) -> (Rect, u8) {
        let full = self.workspace.piano_maximized;
        let top = if full { 65. } else { 68. };
        let graph = rect(
            r.origin.x + if full { 73. } else { 78. },
            r.origin.y + top,
            r.size.width - if full { 96. } else { 107. },
            r.size.height - top - if full { 185. } else { 193. },
        );
        let rows = (graph.size.height / 18.).ceil().clamp(1., 128.) as u8;
        let pattern = self.pattern().unwrap_or(usize::MAX);
        let base = if let Some((_, key)) = self
            .workspace
            .piano_view
            .get()
            .filter(|(i, _)| *i == pattern)
        {
            key
        } else {
            let key = self
                .history
                .project
                .patterns
                .get(pattern)
                .and_then(|p| {
                    p.notes
                        .iter()
                        .filter(|n| self.visible_note(n))
                        .map(|n| n.key)
                        .min()
                })
                .map_or(59, |key| key.saturating_sub(6));
            self.workspace.piano_view.set(Some((pattern, key)));
            key
        };
        (graph, base.min(128 - rows))
    }
    pub(crate) fn automation_target(&self) -> Target {
        match self.workspace.automation_target {
            0 => Target::Gain(self.selected),
            1 => Target::Pan(self.selected),
            _ => Target::Master,
        }
    }
    pub(crate) fn automation_graph(&self, r: Rect) -> Rect {
        rect(
            r.origin.x + 48.,
            r.origin.y + 62.,
            r.size.width - 65.,
            r.size.height - 102.,
        )
    }
}
fn name(key: u8) -> String {
    format!(
        "{}{}",
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ][(key % 12) as usize],
        key as i16 / 12 - 1
    )
}

fn automation_range(target: Target) -> (f32, f32) {
    match target {
        Target::Pan(_) => (-1., 1.),
        Target::Gain(_) => (0., 2.),
        Target::Master => (0., 1.),
    }
}
