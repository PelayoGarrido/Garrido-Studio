use super::*;

impl State {
    pub(super) fn draw_reference(&mut self) {
        let bounds = send!(self.view, "bounds", Rect);
        self.scale = (bounds.size.width / 1920.).max(0.1);
        self.width = 1920.;
        self.height = bounds.size.height / self.scale;
        self.workspace.layout(self.width, self.height);
        self.sync_tools();
        send!(class(c"NSGraphicsContext"), "saveGraphicsState", ());
        let transform = send!(class(c"NSAffineTransform"), "transform", Id);
        send!(transform,"scaleXBy:yBy:",();self.scale,f64;self.scale,f64);
        send!(transform, "concat", ());
        fill(rect(0., 0., self.width, self.height), [0.145, 0.145, 0.145]);
        self.draw_toolbar();
        self.draw_browser();
        self.draw_playlist();
        for index in self.workspace.order.clone() {
            let panel = self.workspace.panels[index];
            if !panel.visible {
                continue;
            }
            border(panel.rect);
            match index {
                0 => self.draw_rack(panel.rect),
                1 => self.draw_piano(panel.rect),
                _ => {
                    let r = panel.rect;
                    round(
                        rect(r.origin.x + 1., r.origin.y + 1., r.size.width - 2., 25.),
                        8.,
                        [0.20, 0.20, 0.20],
                    );
                    icon("chevron-right", r.origin.x + 8., r.origin.y + 5., 15., ICON);
                    text(
                        match index {
                            2 => "Mixer",
                            3 => "Audio editor",
                            _ => "Automation",
                        },
                        r.origin.x + 31.,
                        r.origin.y + 4.,
                        16.,
                    );
                    icon(
                        "x",
                        r.origin.x + r.size.width - 23.,
                        r.origin.y + 6.,
                        14.,
                        ICON,
                    );
                    match index {
                        2 => self.workspace_mixer(r),
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
                                text(
                                    name,
                                    r.origin.x + 14. + i as f64 * 88.,
                                    r.origin.y + 31.,
                                    14.,
                                );
                            }
                            self.workspace_audio(r);
                        }
                        _ => self.workspace_automation(r),
                    }
                }
            }
        }
        let views = send!(self.view, "subviews", Id);
        let count = send!(views, "count", usize);
        let focused = send!(send!(self.view, "window", Id), "firstResponder", Id);
        for i in 0..count {
            let view = send!(views,"objectAtIndex:",Id;i,usize);
            if view == focused && send!(view, "isHidden", i8) == 0 {
                let r = send!(view, "frame", Rect);
                outline(
                    rect(
                        r.origin.x / self.scale,
                        r.origin.y / self.scale,
                        r.size.width / self.scale,
                        r.size.height / self.scale,
                    ),
                    3.,
                    1.,
                    GOLD,
                );
            }
        }
        send!(class(c"NSGraphicsContext"), "restoreGraphicsState", ());
    }
    fn draw_toolbar(&self) {
        fill(rect(0., 0., self.width, 40.), [0.235, 0.235, 0.235]);
        fill(rect(0., 41., self.width, 39.), [0.225, 0.225, 0.225]);
        fill(rect(0., 40., self.width, 1.), [0.15, 0.15, 0.15]);
        fill(rect(0., 80., self.width, 2.), [0.12, 0.12, 0.12]);
        for (i, name) in ["minus", "square", "x"].iter().enumerate() {
            circle(14. + i as f64 * 20., 20., 9., [0.27, 0.28, 0.29]);
            icon(name, 7. + i as f64 * 20., 13., 14., ICON);
        }
        round(rect(73., 4., 304., 32.), 4., [0.17, 0.17, 0.17]);
        outline(rect(73., 4., 304., 32.), 4., 1., [0.13, 0.13, 0.13]);
        label(
            &self
                .history
                .project
                .title
                .chars()
                .take(43)
                .collect::<String>(),
            78.,
            5.,
            14.,
            DIM,
            false,
        );
        let hint = if self.status.starts_with("Listo") {
            ""
        } else {
            &self.status
        };
        label(
            &hint.chars().take(49).collect::<String>(),
            78.,
            21.,
            11.,
            TEXT,
            false,
        );
        icon("adjustments", 360., 18., 14., DIM);
        round(rect(385., 3., 34., 33.), 4., [0.16, 0.16, 0.16]);
        round(
            rect(386., if self.song_mode { 20. } else { 5. }, 32., 15.),
            3.,
            GOLD,
        );
        label(
            "PAT",
            392.,
            3.,
            12.,
            if self.song_mode {
                DIM
            } else {
                [0.25, 0.23, 0.14]
            },
            false,
        );
        label(
            "SONG",
            388.,
            20.,
            12.,
            if self.song_mode {
                [0.25, 0.23, 0.14]
            } else {
                DIM
            },
            false,
        );
        for (name, x) in [("player-play", 425.), ("player-stop", 456.)] {
            icon(name, x, 10., 20., ICON);
        }
        circle(
            500.,
            20.,
            6.,
            if self.capture.is_some() {
                [0.88, 0.25, 0.25]
            } else {
                [0.40, 0.23, 0.23]
            },
        );
        round(rect(516., 3., 54., 32.), 3., [0.12, 0.12, 0.12]);
        label(
            &format!("{:03}", self.history.project.bpm.round() as u16),
            519.,
            7.,
            16.,
            GOLD,
            false,
        );
        label(".000", 542., 14., 8., DIM, false);
        icon("chevron-up", 555., 8., 10., DIM);
        icon("chevron-down", 555., 22., 10., DIM);
        round(
            rect(581., 4., 30., 31.),
            5.,
            if self.metronome {
                GOLD
            } else {
                [0.20, 0.20, 0.20]
            },
        );
        icon("metronome", 585., 8., 22., [0.22, 0.22, 0.18]);
        for (name, x) in [
            ("hourglass", 617.),
            ("clock", 651.),
            ("link", 684.),
            ("repeat", 718.),
        ] {
            icon(name, x, 10., 21., [0.40, 0.44, 0.47]);
        }
        round(rect(746., 4., 128., 31.), 4., [0.12, 0.12, 0.12]);
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
        segments(&time, 785., 10., 10., 21.);
        label("B:S:T", 851., 3., 7., DIM, false);
        round(rect(880., 4., 86., 31.), 3., [0.12, 0.12, 0.12]);
        round(rect(970., 3., 75., 24.), 3., [0.13, 0.13, 0.13]);
        round(rect(970., 29., 75., 9.), 3., [0.14, 0.14, 0.14]);
        round(rect(1054., 3., 147., 34.), 3., [0.13, 0.13, 0.13]);
        let memory = self
            .history
            .project
            .tracks
            .iter()
            .filter_map(|t| t.sample.as_ref())
            .map(|s| s.frames.len() * 8)
            .sum::<usize>()
            / (1024 * 1024);
        label("—%", 1114., 3., 15., TEXT, false);
        label(&format!("{memory} MB"), 1162., 3., 15., TEXT, false);
        label("0", 1185., 20., 13., DIM, false);
        for (name, x) in [
            ("arrow-back-up", 1218.),
            ("scissors", 1250.),
            ("microphone", 1282.),
            ("help-circle", 1314.),
            ("device-floppy", 1346.),
            ("file-export", 1378.),
            ("info-circle", 1410.),
            ("download", 1442.),
        ] {
            round(rect(x - 6., 4., 30., 31.), 4., [0.215, 0.215, 0.215]);
            icon(name, x, 10., 20., ICON);
        }
        for spec in theme::controls()
            .iter()
            .filter(|s| (2000..=2005).contains(&s.tag) || s.tag == 9 || s.tag == 9353)
        {
            label(
                spec.name,
                spec.r.origin.x + 3.,
                54.,
                14.,
                [0.84, 0.84, 0.84],
                false,
            );
        }
        knob(318., 61., 13., self.history.project.master as f64, true);
        let pan = self
            .history
            .project
            .tracks
            .get(self.selected)
            .map_or(0., |t| t.pan);
        knob(358., 61., 13., (pan as f64 + 1.) * 0.5, false);
        fill(rect(390., 55., 179., 17.), [0.15, 0.15, 0.15]);
        fill(rect(391., 58., 2., 11.), GOLD);
        for (tag, name, x) in [
            (100, "layout-list", 581.),
            (101, "layout-sidebar-right", 614.),
        ] {
            let open = self.workspace.panels[if tag == 100 { 0 } else { 1 }].visible;
            round(
                rect(x, 46., 30., 31.),
                5.,
                if open { GOLD } else { [0.18, 0.18, 0.18] },
            );
            icon(
                name,
                x + 4.,
                51.,
                22.,
                if open { [0.23, 0.22, 0.17] } else { ICON },
            );
        }
        for (name, x) in [("grid-dots", 652.), ("link", 685.), ("adjustments", 719.)] {
            icon(name, x, 51., 20., ICON);
        }
        icon("magnet", 750., 52., 18., DIM);
        round(rect(768., 50., 63., 23.), 4., [0.14, 0.14, 0.14]);
        label(
            match self.workspace.note_grid {
                1 => "None",
                PPQ => "Beat",
                BAR => "Bar",
                _ => "Line",
            },
            776.,
            51.,
            16.,
            TEXT,
            false,
        );
        icon("chevron-right", 816., 56., 10., DIM);
        icon("chevron-right", 839., 54., 12., DIM);
        round(rect(853., 47., 96., 30.), 4., [0.13, 0.13, 0.13]);
        if let Some(pattern) = self.pattern() {
            label(
                &self.history.project.patterns[pattern]
                    .name
                    .chars()
                    .take(14)
                    .collect::<String>(),
                862.,
                50.,
                18.,
                [0.86, 0.86, 0.86],
                true,
            );
        }
        icon("plus", 952., 55., 13., DIM);
        for (name, x) in [
            ("music", 978.),
            ("piano", 1011.),
            ("layout-list", 1044.),
            ("adjustments", 1077.),
            ("device-audio-tape", 1110.),
            ("file", 1143.),
            ("tools", 1176.),
            ("volume", 1209.),
            ("hand-move", 1242.),
            ("lock", 1275.),
        ] {
            round(rect(x - 6., 46., 30., 31.), 4., [0.21, 0.21, 0.21]);
            icon(name, x, 52., 20., ICON);
        }
        label(
            "Today  Welcome to Garrido Studio",
            1310.,
            53.,
            13.,
            [0.36, 0.37, 0.37],
            false,
        );
        icon("world", 1500., 54., 14., DIM);
        circle(1880., 38., 24., [0.27, 0.27, 0.27]);
        label("G", 1864., 10., 43., [0.23, 0.23, 0.23], true);
    }
    fn draw_browser(&self) {
        fill(
            rect(0., 82., 205., self.height - 82.),
            [0.135, 0.135, 0.135],
        );
        fill(rect(0., 82., 205., 24.), [0.16, 0.16, 0.16]);
        icon("chevron-right", 4., 87., 11., DIM);
        icon("arrows-horizontal", 17., 87., 12., DIM);
        icon("arrow-left", 34., 87., 11., ICON);
        label("Browser", 51., 85., 14., DIM, false);
        for (i, name) in ["wave-sine", "file-description", "volume", "world", "star"]
            .iter()
            .enumerate()
        {
            let x = 4. + i as f64 * 40.;
            if self.workspace.browser_tab as usize == i {
                round(rect(x, 108., 33., 30.), 4., [0.16, 0.16, 0.16]);
                outline(rect(x, 108., 33., 30.), 4., 1.5, GOLD);
            }
            icon(
                name,
                x + 7.,
                114.,
                20.,
                if self.workspace.browser_tab as usize == i {
                    GOLD
                } else {
                    ICON
                },
            );
        }
        fill(
            rect(13., 141., 188., self.height - 168.),
            [0.117, 0.117, 0.117],
        );
        icon(
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
        icon("file-description", 21., 146., 15., GOLD);
        label("Current project", 39., 142., 16., GOLD, false);
        let mut y = 166.;
        if self.workspace.browser_expanded {
            icon("chevron-down", 16., 170., 11., DIM);
            icon("music", 33., 168., 16., GOLD);
            label("Patterns", 51., 165., 16., GOLD, false);
            y = 190.;
            for (i, pattern) in self
                .history
                .project
                .patterns
                .iter()
                .enumerate()
                .take(((self.height - 330.) / 23.).max(0.) as usize)
            {
                icon("music", 47., y + 2., 14., GOLD);
                label(
                    &pattern.name.chars().take(23).collect::<String>(),
                    64.,
                    y,
                    16.,
                    GOLD,
                    false,
                );
                if Some(i) == self.pattern() {
                    fill(rect(14., y, 2., 20.), GOLD);
                }
                y += 23.;
            }
            for (name, title) in [
                ("volume", "Effects"),
                ("tools", "Generators"),
                ("wave-sine", "Samples"),
            ] {
                icon(name, 33., y + 2., 16., GOLD);
                label(title, 52., y, 16., GOLD, false);
                y += 23.;
            }
        }
        icon("volume", 20., y + 3., 15., ICON);
        label("Plugin database", 39., y, 16., ICON, false);
        fill(rect(201., 82., 7., self.height - 82.), [0.19, 0.19, 0.19]);
        fill(rect(207., 82., 2., self.height - 82.), [0.12, 0.12, 0.12]);
        label(
            "TAGS",
            19.,
            self.height - 22.,
            11.,
            [0.35, 0.35, 0.35],
            true,
        );
        icon("folder", 158., self.height - 21., 13., DIM);
        icon("search", 181., self.height - 22., 15., GOLD);
    }
    fn draw_playlist(&self) {
        let h = self.height;
        fill(rect(209., 82., self.width - 209., 24.), [0.19, 0.19, 0.19]);
        fill(rect(209., 105., self.width - 209., 1.), EDGE);
        for (name, x) in [
            ("chevron-right", 216.),
            ("magnet", 235.),
            ("pencil", 266.),
            ("brush", 289.),
            ("eraser", 312.),
            ("volume-off", 334.),
            ("arrows-horizontal", 357.),
            ("pencil", 378.),
            ("select", 399.),
            ("zoom-in", 420.),
            ("volume", 441.),
            ("arrows-horizontal", 461.),
        ] {
            if name == "pencil" && x == 266. {
                round(rect(x - 1., 84., 21., 20.), 4., GOLD);
                icon(name, x + 1., 86., 17., [0.22, 0.22, 0.18]);
            } else {
                icon(name, x, 86., 17., ICON);
            }
        }
        let pattern = self.pattern().map_or("Pattern 1", |i| {
            self.history.project.patterns[i].name.as_str()
        });
        text(
            &format!("Playlist - Arrangement  ›  {pattern}  ›"),
            489.,
            84.,
            16.,
        );
        icon("minus", 1857., 87., 13., ICON);
        icon("copy", 1877., 87., 13., ICON);
        icon("x", 1898., 87., 13., ICON);
        fill(rect(209., 106., 159., h - 106.), [0.15, 0.15, 0.15]);
        for (name, x) in [("piano", 222.), ("wave-sine", 271.), ("chart-bar", 319.)] {
            icon(name, x, 122., 19., ICON);
        }
        fill(rect(350., 148., 18., h - 148.), [0.17, 0.17, 0.17]);
        fill(rect(367., 107., 2., h - 107.), [0.20, 0.20, 0.20]);
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
                round(rect(212., y, 135., 29.), 5., [0.20, 0.20, 0.20]);
                outline(rect(212., y, 135., 29.), 5., 1.2, GOLD);
            }
            label(
                &p.name.chars().take(22).collect::<String>(),
                225.,
                y + 5.,
                14.,
                TEXT,
                false,
            );
            if Some(i) == self.pattern() {
                circle(215., y + 14., 1.5, GOLD);
            }
        }
        icon("chevron-left", 355., 152., 10., DIM);
        icon("plus", 272., h - 16., 12., DIM);
        fill(rect(369., 106., 111., 42.), [0.17, 0.17, 0.17]);
        icon("plus", 379., 126., 18., TEXT);
        for (s, x) in [("NOTE", 407.), ("CHAN", 435.), ("PAT", 461.)] {
            label(s, x, 133., 8., DIM, true);
        }
        round(rect(485., 110., 330., 14.), 7., [0.33, 0.33, 0.33]);
        let gx = 481.;
        let gw = self.width - gx - 22.;
        let bars = self.history.project.bars.max(24);
        let end = bars as f64 * BAR as f64;
        fill(rect(gx, 128., gw, 20.), [0.13, 0.13, 0.13]);
        for bar in 0..=bars {
            let xx = gx + bar as f64 / bars as f64 * gw;
            if bar < bars {
                label(&(bar + 1).to_string(), xx + 5., 130., 14., DIM, false);
            }
            for part in 0..4 {
                let x = xx + part as f64 * gw / bars as f64 / 4.;
                fill(
                    rect(x, 148., if part == 0 { 1.7 } else { 0.6 }, h - 148.),
                    if part == 0 {
                        [0.09, 0.09, 0.09]
                    } else {
                        [0.115, 0.115, 0.115]
                    },
                );
            }
        }
        for row in 0..((h - 148.) / 48.).ceil() as usize {
            let y = 148. + row as f64 * 48.;
            round(rect(371., y + 2., 98., 45.), 5., [0.20, 0.20, 0.20]);
            label(
                &format!("Track {}", row + 1),
                375.,
                y + 3.,
                14.,
                [0.48, 0.48, 0.48],
                false,
            );
            label("...", 375., y + 26., 12., [0.34, 0.34, 0.34], false);
            round(rect(453., y + 33., 9., 9.), 2., GOLD);
            fill(rect(gx, y + 46., gw, 2.), [0.095, 0.095, 0.095]);
        }
        for clip in &self.history.project.clips {
            let y = 148. + clip.track as f64 * 48.;
            if y + 47. > h {
                continue;
            }
            let x = gx + clip.tick as f64 / end * gw;
            let width = clip.length as f64 / end * gw;
            round(
                rect(x + 1., y + 1., (width - 2.).max(2.), 44.),
                2.,
                [0.30, 0.34, 0.30],
            );
            let pattern = &self.history.project.patterns[clip.pattern];
            label(
                &pattern
                    .name
                    .chars()
                    .take((width / 6.).max(1.) as usize)
                    .collect::<String>(),
                x + 4.,
                y + 3.,
                13.,
                TEXT,
                false,
            );
            for note in pattern.notes.iter().take(128) {
                let xx = x + note.tick as f64 / pattern.length as f64 * width;
                if xx < x + width {
                    fill(
                        rect(xx, y + 26. + (note.key % 12) as f64, 3., 1.),
                        [0.65, 0.71, 0.65],
                    );
                }
            }
        }
        fill(rect(1899., 128., 21., h - 128.), [0.19, 0.19, 0.19]);
        round(rect(1905., 149., 13., 34.), 6., [0.34, 0.34, 0.34]);
        icon("player-play", 477., 126., 12., GOLD);
        if let Some(play) = &self.playback {
            let tick = self
                .history
                .project
                .tick_at(play.frame() as f64 / self.history.project.rate as f64);
            fill(rect(gx + tick / end * gw, 148., 1., h - 148.), GOLD);
        }
    }
    fn draw_rack(&self, r: Rect) {
        let x = r.origin.x;
        let y = r.origin.y;
        round(
            rect(x + 3., y + 29., r.size.width - 7., r.size.height - 57.),
            7.,
            [0.19, 0.19, 0.19],
        );
        outline(
            rect(x + 3., y + 29., r.size.width - 7., r.size.height - 57.),
            7.,
            1.,
            [0.14, 0.14, 0.14],
        );
        for (name, xx) in [
            ("chevron-right", x + 10.),
            ("clock", x + 35.),
            ("volume", x + 226.),
        ] {
            icon(name, xx, y + 7., 15., ICON);
        }
        round(rect(x + 106., y + 4., 95., 23.), 3., [0.17, 0.17, 0.17]);
        label("All", x + 111., y + 5., 16., TEXT, false);
        icon("chevron-right", x + 189., y + 11., 9., DIM);
        label("Channel rack", x + 247., y + 4., 16., TEXT, false);
        knob(x + 363., y + 14., 9., 0.5, false);
        round(rect(x + 380., y + 4., 31., 22.), 4., [0.24, 0.24, 0.24]);
        for (name, xx) in [
            ("repeat", x + 424.),
            ("chart-bar", x + 447.),
            ("piano", x + 470.),
            ("x", x + 490.),
        ] {
            icon(
                name,
                xx,
                y + 7.,
                15.,
                if name == "piano" { GOLD } else { ICON },
            );
        }
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
            round(
                rect(x + 14., yy + 8., 7., 7.),
                1.,
                if track.mute { DIM } else { GOLD },
            );
            knob(x + 36., yy + 12., 6., (track.pan as f64 + 1.) * 0.5, true);
            knob(x + 52., yy + 12., 6., track.gain as f64 / 2., true);
            round(rect(x + 66., yy + 2., 33., 24.), 3., [0.25, 0.25, 0.25]);
            label(&(index + 1).to_string(), x + 80., yy + 5., 14., TEXT, false);
            icon("chevron-up", x + 91., yy + 4., 7., DIM);
            icon("chevron-down", x + 91., yy + 15., 7., DIM);
            round(rect(x + 108., yy + 1., 93., 25.), 4., [0.23, 0.23, 0.23]);
            if index == self.selected {
                outline(rect(x + 108., yy + 1., 93., 25.), 4., 1., GOLD);
            }
            label(
                &track.name.chars().take(19).collect::<String>(),
                x + 114.,
                yy + 5.,
                13.,
                [0.87, 0.87, 0.87],
                false,
            );
            if index == self.selected {
                outline(rect(x + 211., yy, 8., 26.), 4., 1.2, GOLD);
            }
            let pat = if index == self.selected {
                self.pattern()
            } else {
                self.history
                    .project
                    .clips
                    .iter()
                    .find(|c| c.track == index)
                    .map(|c| c.pattern)
                    .or_else(|| (index < self.history.project.patterns.len()).then_some(index))
            };
            for step in 0..16u32 {
                let on = pat.is_some_and(|i| {
                    self.history.project.patterns[i]
                        .notes
                        .iter()
                        .any(|n| n.tick / (PPQ / 4) == step)
                });
                round(
                    rect(x + 229. + step as f64 * 16., yy + 4., 13., 19.),
                    3.,
                    if on {
                        GOLD
                    } else if step / 4 % 2 == 0 {
                        [0.26, 0.26, 0.26]
                    } else {
                        [0.31, 0.28, 0.12]
                    },
                );
            }
        }
        icon("plus", x + 150., y + r.size.height - 24., 14., TEXT);
        round(
            rect(x + 220., y + r.size.height - 11., 15., 4.),
            2.,
            [0.28, 0.28, 0.28],
        );
    }
    fn draw_piano(&self, r: Rect) {
        let x = r.origin.x;
        let y = r.origin.y;
        let maximized = self.workspace.piano_maximized;
        if maximized {
            fill(r, [0.18, 0.18, 0.18]);
        }
        for (i, (name, offset)) in PIANO_TOOLS.iter().enumerate() {
            let xx = x + offset;
            if (i == 6 && !self.workspace.piano_erase) || (i == 9 && self.workspace.piano_erase) {
                round(rect(xx - 1., y + 4., 20., 20.), 4., GOLD);
                icon(name, xx + 1., y + 6., 17., [0.22, 0.22, 0.18]);
            } else {
                icon(name, xx, y + 6., 16., ICON);
            }
        }
        let track = self
            .history
            .project
            .tracks
            .get(self.selected)
            .map_or("Garrido Sampler", |t| t.name.as_str());
        label(
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
            icon(name, xx, y + 7., 13., ICON);
        }
        let (g, base) = self.piano_geometry(r);
        let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
        let top = base + rows - 1;
        let key_x = if maximized { x } else { x + 6. };
        fill(
            rect(key_x, g.origin.y, g.origin.x - key_x - 2., g.size.height),
            [0.92, 0.92, 0.92],
        );
        let pattern = self
            .pattern()
            .and_then(|i| self.history.project.patterns.get(i));
        let pixels = self.workspace.piano_zoom.get();
        let start = self.workspace.piano_start.get() as f64;
        let span = g.size.width / pixels * PPQ as f64;
        fill(g, [0.145, 0.145, 0.145]);
        for row in 0..rows {
            let key = top - row;
            let yy = g.origin.y + row as f64 * 18.;
            let height = (g.origin.y + g.size.height - yy).min(18.);
            let black = matches!(key % 12, 1 | 3 | 6 | 8 | 10);
            fill(
                rect(g.origin.x, yy, g.size.width, height),
                if black {
                    [0.135, 0.135, 0.135]
                } else {
                    [0.16, 0.16, 0.16]
                },
            );
            fill(
                rect(g.origin.x, yy, g.size.width, 0.6),
                [0.105, 0.105, 0.105],
            );
            if black {
                round(
                    rect(key_x, yy + 1., 45., (height - 1.).max(0.5)),
                    3.,
                    [0.23, 0.235, 0.235],
                );
            } else {
                fill(
                    rect(key_x, yy + height - 0.5, g.origin.x - key_x - 2., 0.5),
                    [0.61, 0.61, 0.61],
                );
                if key.is_multiple_of(12) {
                    label(
                        &name(key),
                        g.origin.x - 17.,
                        yy + 2.,
                        10.,
                        [0.58, 0.48, 0.23],
                        false,
                    );
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
            fill(
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
                label(
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
        round(rect(key_x, overview_y, 68., 34.), 6., [0.48, 0.43, 0.24]);
        icon("chevron-left", g.origin.x + 2., overview_y + 3., 13., DIM);
        round(
            rect(
                g.origin.x + 9. + start / extent * rail,
                overview_y + 1.,
                thumb,
                17.,
            ),
            8.,
            [0.34, 0.34, 0.34],
        );
        if let Some(pattern) = pattern {
            let low = pattern.notes.iter().map(|n| n.key).min().unwrap_or(0);
            let high = pattern.notes.iter().map(|n| n.key).max().unwrap_or(low);
            for note in pattern.notes.iter().take(4096) {
                let a = g.origin.x + 9. + note.tick as f64 / extent * rail;
                let b = g.origin.x + 9. + (note.tick + note.length) as f64 / extent * rail;
                let yy =
                    overview_y + 14. - (note.key - low) as f64 / (high - low).max(1) as f64 * 10.;
                line(
                    Point { x: a, y: yy },
                    Point {
                        x: b.max(a + 1.),
                        y: yy,
                    },
                    1.,
                    GOLD,
                );
            }
        }
        icon(
            "chevron-right",
            g.origin.x + g.size.width - 14.,
            overview_y + 3.,
            13.,
            DIM,
        );
        fill(
            rect(g.origin.x, g.origin.y - 19., g.size.width, 19.),
            [0.13, 0.13, 0.13],
        );
        for bar in (start as u32 / BAR)..=((start + span) as u32 / BAR) {
            let xx = g.origin.x + (bar as f64 * BAR as f64 - start) / PPQ as f64 * pixels;
            if xx >= g.origin.x {
                label(
                    &(bar + 1).to_string(),
                    xx + 4.,
                    g.origin.y - 18.,
                    14.,
                    DIM,
                    false,
                );
            }
        }
        fill(
            rect(g.origin.x - 3., g.origin.y, 2., g.size.height),
            [0.08, 0.08, 0.08],
        );
        let scroll_x = g.origin.x + g.size.width + 5.;
        fill(
            rect(scroll_x, g.origin.y - 19., 18., g.size.height + 19.),
            [0.19, 0.19, 0.19],
        );
        icon("chevron-up", scroll_x + 1., g.origin.y - 16., 15., DIM);
        icon(
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
        round(rect(scroll_x + 2., vy, 14., vh), 7., [0.34, 0.34, 0.34]);
        fill(
            rect(x + 6., g.origin.y + g.size.height, r.size.width - 12., 8.),
            [0.20, 0.20, 0.20],
        );
        let lane_y = g.origin.y + g.size.height + 9.;
        let bottom = y + r.size.height - 7.;
        fill(
            rect(g.origin.x, lane_y, g.size.width, (bottom - lane_y).max(1.)),
            [0.14, 0.14, 0.14],
        );
        label("Control ›", x + 16., lane_y + 4., 14., TEXT, false);
        label(
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
                fill(rect(xx, lane_y, 0.6, bottom - lane_y), [0.09, 0.09, 0.09]);
            }
        }
        for i in 1..8 {
            fill(
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
            for (index, n) in pattern.notes.iter().enumerate().take(4096) {
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
                        round(
                            rect(left + 0.4, yy + 0.6, (width - 0.8).max(1.), 17.),
                            2.5,
                            if self.workspace.selected_notes.contains(&index) {
                                [1., 0.92, 0.57]
                            } else {
                                GOLD
                            },
                        );
                        outline(
                            rect(left + 0.4, yy + 0.6, (width - 0.8).max(1.), 17.),
                            2.5,
                            0.6,
                            [0.73, 0.59, 0.24],
                        );
                        if width > 17. {
                            label(
                                &name(n.key),
                                left + 3.,
                                yy + 2.,
                                10.,
                                [0.48, 0.40, 0.15],
                                false,
                            );
                        }
                    }
                }
                if xx >= g.origin.x && xx <= g.origin.x + g.size.width {
                    let yy = bottom - n.velocity as f64 * (bottom - lane_y - 25.);
                    line(Point { x: xx, y: bottom }, Point { x: xx, y: yy }, 1., GOLD);
                    line(
                        Point { x: xx, y: yy },
                        Point {
                            x: (xx + ww).min(g.origin.x + g.size.width),
                            y: yy,
                        },
                        0.8,
                        GOLD,
                    );
                    circle(xx, yy, 3., GOLD);
                    circle(xx, yy, 2., [0.17, 0.17, 0.17]);
                }
            }
        }
    }
}
