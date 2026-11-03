use super::*;

impl State {
    pub(super) fn message(&self, title: &str, body: &str) {
        let alert = send!(send!(class(c"NSAlert"), "alloc", Id), "init", Id);
        send!(alert,"setMessageText:",();string(title),Id);
        send!(alert,"setInformativeText:",();string(body),Id);
        send!(alert,"addButtonWithTitle:",Id;string("OK"),Id);
        send!(alert, "runModal", isize);
        send!(alert, "release", ());
    }

    pub(super) fn ask_text(&self, title: &str, value: &str) -> Option<Result<String>> {
        let alert = send!(send!(class(c"NSAlert"), "alloc", Id), "init", Id);
        send!(alert,"setMessageText:",();string(title),Id);
        for text in ["Apply", "Cancel"] {
            send!(alert,"addButtonWithTitle:",Id;string(text),Id);
        }
        let field = send!(class(c"NSTextField"),"textFieldWithString:",Id;string(value),Id);
        send!(field,"setFrame:",();rect(0.,0.,320.,26.),Rect);
        send!(alert,"setAccessoryView:",();field,Id);
        let answer = send!(alert, "runModal", isize);
        let text = rust_string(send!(field, "stringValue", Id));
        send!(alert, "release", ());
        if answer != 1000 {
            return None;
        }
        Some(text.ok_or(Error::Invalid("Texto no válido")).and_then(|s| {
            garrido_studio::model::text(&s)?;
            if s.trim().is_empty() {
                return Err(Error::Invalid("Escribe un nombre"));
            }
            Ok(s)
        }))
    }
    pub(super) fn numbers(&self, title: &str, values: &[(&str, f32)]) -> Option<Result<Vec<f32>>> {
        let alert = send!(send!(class(c"NSAlert"), "alloc", Id), "init", Id);
        send!(alert,"setMessageText:",();string(title),Id);
        for title in ["Apply", "Cancel"] {
            send!(alert,"addButtonWithTitle:",Id;string(title),Id);
        }
        let height = values.len() as f64 * 34.;
        let view = send!(send!(class(c"NSView"),"alloc",Id),"initWithFrame:",Id;rect(0.,0.,320.,height),Rect);
        let mut fields = Vec::new();
        for (i, (title, value)) in values.iter().enumerate() {
            let y = height - (i + 1) as f64 * 34.;
            let label = send!(class(c"NSTextField"),"labelWithString:",Id;string(title),Id);
            send!(label,"setFrame:",();rect(0.,y,175.,24.),Rect);
            send!(view,"addSubview:",();label,Id);
            let field = send!(class(c"NSTextField"),"textFieldWithString:",Id;string(&format!("{value:.4}")),Id);
            send!(field,"setFrame:",();rect(180.,y,135.,26.),Rect);
            send!(view,"addSubview:",();field,Id);
            fields.push(field);
        }
        send!(alert,"setAccessoryView:",();view,Id);
        send!(view, "release", ());
        let answer = send!(alert, "runModal", isize);
        let result = if answer == 1000 {
            Some(
                fields
                    .into_iter()
                    .map(|f| {
                        rust_string(send!(f, "stringValue", Id))
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .filter(|v| v.is_finite())
                            .ok_or(Error::Invalid("Número no válido"))
                    })
                    .collect(),
            )
        } else {
            None
        };
        send!(alert, "release", ());
        result
    }
    pub(super) fn popup(&self, items: Vec<(String, isize)>, at: Point) {
        let menu = send!(send!(class(c"NSMenu"), "alloc", Id), "init", Id);
        for (title, tag) in items {
            let item = send!(send!(class(c"NSMenuItem"),"alloc",Id),"initWithTitle:action:keyEquivalent:",Id;string(&title),Id;selector(c"clicked:".as_ptr()),Sel;string(""),Id);
            send!(item,"setTag:",();tag,isize);
            send!(item,"setTarget:",();self.view,Id);
            send!(menu,"addItem:",();item,Id);
            send!(item, "release", ());
        }
        send!(menu,"popUpMenuPositioningItem:atLocation:inView:",();ptr::null_mut(),Id;Point{x:at.x*self.scale,y:at.y*self.scale},Point;self.view,Id);
        send!(menu, "release", ());
    }
    pub(super) fn routing_menu(&self, at: Point) {
        let mut items = vec![
            ("Channel gain / pan…".into(), 4600),
            ("Rename channel…".into(), 86),
        ];
        for (index, r) in self
            .history
            .project
            .routes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.from == self.selected)
        {
            let name =
                r.to.map_or("Master", |i| self.history.project.tracks[i].name.as_str());
            items.push((format!("To {name}: {:.2}…", r.gain), 4700 + index as isize));
        }
        items.extend([
            ("Add send…".into(), 5100),
            ("Route only to…".into(), 5102),
            ("Reset output to Master".into(), 5101),
        ]);
        self.popup(items, at);
    }
    pub(super) fn edit_route(&mut self, tag: isize) {
        use garrido_studio::routing::Route;
        let selected = self.selected;
        if selected >= self.history.project.tracks.len() {
            return;
        }
        let existing = (4700..=4955).contains(&tag);
        let index = (tag - 4700).max(0) as usize;
        let old = if existing {
            let Some(r) = self.history.project.routes.get(index) else {
                return;
            };
            r.clone()
        } else {
            Route {
                from: selected,
                to: None,
                gain: 0.25,
                invert: false,
                swap: false,
            }
        };
        let Some(values) = self.numbers(
            "Routing · 0 = Master, 1…32 = channel",
            &[
                ("Destination", old.to.map_or(0., |i| (i + 1) as f32)),
                ("Gain 0–2", old.gain),
                ("Invert polarity 0/1", old.invert as u8 as f32),
                ("Swap L/R 0/1", old.swap as u8 as f32),
            ],
        ) else {
            return;
        };
        let result = values.and_then(|v| {
            if v[0].fract() != 0.
                || v[0] < 0.
                || v[0] > self.history.project.tracks.len() as f32
                || ![0., 1.].contains(&v[2])
                || ![0., 1.].contains(&v[3])
            {
                return Err(Error::Invalid("Destino o interruptor no válido"));
            }
            let route = Route {
                from: selected,
                to: if v[0] == 0. {
                    None
                } else {
                    Some(v[0] as usize - 1)
                },
                gain: v[1],
                invert: v[2] == 1.,
                swap: v[3] == 1.,
            };
            self.history.edit(|p| {
                if existing {
                    p.routes[index] = route;
                } else {
                    if tag == 5102 {
                        p.routes.retain(|r| r.from != selected);
                    } else if route.to.is_some() && !p.routes.iter().any(|r| r.from == selected) {
                        p.routes.push(Route {
                            from: selected,
                            to: None,
                            gain: 1.,
                            invert: false,
                            swap: false,
                        });
                    }
                    p.routes.push(route);
                }
                Ok(())
            })
        });
        if result.is_ok() {
            self.changed();
        }
        self.result(result);
    }
    pub(super) fn effects_menu(&self) {
        let mut items = Vec::new();
        if let Some(t) = self.history.project.tracks.get(self.selected) {
            for (i, fx) in t.effects.iter().enumerate() {
                items.push((
                    format!("Slot {}: {}…", i + 1, effect_name(fx)),
                    4200 + i as isize,
                ));
            }
        }
        for (i, name) in [
            "EQ",
            "Delay",
            "Reverb",
            "Chorus",
            "Drive",
            "Width",
            "Compressor",
        ]
        .iter()
        .enumerate()
        {
            items.push((format!("Add {name}…"), 4100 + i as isize));
        }
        items.push(("Remove last effect".into(), 4300));
        self.popup(items, Point { x: 713., y: 79. });
    }
    pub(super) fn edit_effect(&mut self, tag: isize) {
        let Some(track) = self.history.project.tracks.get(self.selected) else {
            return;
        };
        let selected = self.selected;
        if tag == 4300 {
            let result = self.history.edit(|p| {
                p.tracks[selected].effects.pop();
                Ok(())
            });
            if result.is_ok() {
                self.changed();
            }
            self.result(result);
            return;
        }
        let existing = tag >= 4200;
        let slot = (if existing { tag - 4200 } else { tag - 4100 }) as usize;
        if !existing && track.effects.len() >= 8 {
            self.status = "Máximo ocho efectos por canal".into();
            return;
        }
        let fx = if existing {
            let Some(fx) = track.effects.get(slot) else {
                return;
            };
            fx.clone()
        } else {
            match slot {
                0 => Effect::Eq {
                    hz: 1200.,
                    db: -3.,
                    q: 0.7,
                },
                1 => Effect::Delay {
                    seconds: 0.25,
                    feedback: 0.3,
                    mix: 0.2,
                },
                2 => Effect::Reverb {
                    decay: 0.6,
                    mix: 0.2,
                },
                3 => Effect::Chorus {
                    rate: 0.4,
                    depth: 0.3,
                    mix: 0.25,
                },
                4 => Effect::Drive {
                    drive: 2.,
                    mix: 0.2,
                },
                5 => Effect::Width(1.),
                6 => Effect::Compressor {
                    threshold: 0.5,
                    ratio: 3.,
                },
                _ => Effect::Compressor {
                    threshold: 0.5,
                    ratio: 3.,
                },
            }
        };
        let values = match fx {
            Effect::Eq { hz, db, q } => vec![
                ("Frequency 20–20000 Hz", hz),
                ("Gain −24–24 dB", db),
                ("Q 0.1–12", q),
            ],
            Effect::Delay {
                seconds,
                feedback,
                mix,
            } => vec![
                ("Time 0.001–2 seconds", seconds),
                ("Feedback 0–0.9", feedback),
                ("Mix 0–1", mix),
            ],
            Effect::Reverb { decay, mix } => vec![("Decay 0–0.9", decay), ("Mix 0–1", mix)],
            Effect::Chorus { rate, depth, mix } => vec![
                ("Rate 0.05–8 Hz", rate),
                ("Depth 0–1", depth),
                ("Mix 0–1", mix),
            ],
            Effect::Drive { drive, mix } => vec![("Drive 1–20", drive), ("Mix 0–1", mix)],
            Effect::Width(v) => vec![("Width 0–2", v)],
            Effect::Compressor { threshold, ratio } => {
                vec![("Threshold 0.01–1", threshold), ("Ratio 1–20", ratio)]
            }
        };
        let Some(values) = self.numbers(effect_name(&fx), &values) else {
            return;
        };
        let result = values.and_then(|v| {
            let fx = match fx {
                Effect::Eq { .. } => Effect::Eq {
                    hz: v[0],
                    db: v[1],
                    q: v[2],
                },
                Effect::Delay { .. } => Effect::Delay {
                    seconds: v[0],
                    feedback: v[1],
                    mix: v[2],
                },
                Effect::Reverb { .. } => Effect::Reverb {
                    decay: v[0],
                    mix: v[1],
                },
                Effect::Chorus { .. } => Effect::Chorus {
                    rate: v[0],
                    depth: v[1],
                    mix: v[2],
                },
                Effect::Drive { .. } => Effect::Drive {
                    drive: v[0],
                    mix: v[1],
                },
                Effect::Width(_) => Effect::Width(v[0]),
                Effect::Compressor { .. } => Effect::Compressor {
                    threshold: v[0],
                    ratio: v[1],
                },
            };
            fx.validate()?;
            self.history.edit(|p| {
                if existing {
                    p.tracks[selected].effects[slot] = fx;
                } else {
                    p.tracks[selected].effects.push(fx);
                }
                Ok(())
            })
        });
        if result.is_ok() {
            self.changed();
        }
        self.result(result);
    }
    pub(super) fn open_menu(&mut self, tag: isize) {
        let items: Vec<(&str, isize)> = match tag {
            2000 => vec![
                ("New session", 9010),
                ("Open session…", 4),
                ("Open FLP…", 60),
                ("Save session copy…", 3),
                ("Rename project…", 87),
                ("Recovery copies…", 90),
                ("Save recovery copy now", 91),
                ("Save FLP copy…", 61),
                ("Export WAV…", 5),
                ("Export FLAC…", 97),
                ("Load FLAC…", 98),
                ("Import MIDI…", 11),
                ("Export MIDI…", 12),
                ("Export pending take…", 75),
                ("Demo", 10),
            ],
            2001 if self.page == Page::Audio => vec![
                ("Undo", 6),
                ("Redo", 7),
                ("Copy audio", 530),
                ("Cut audio", 531),
                ("Paste audio", 532),
                ("Export selection…", 533),
                ("Reverse", 50),
                ("Normalize", 51),
                ("Fade in", 52),
                ("Fade out", 53),
                ("Trim", 54),
            ],
            2001 => vec![
                ("Undo", 6),
                ("Redo", 7),
                ("Quantize", 34),
                ("Reverse notes", 37),
                ("Legato", 38),
                ("Humanize", 39),
                ("Select all notes", 440),
                ("Copy notes", 441),
                ("Cut notes", 442),
                ("Paste notes at cursor", 443),
                ("Delete notes", 444),
                ("Chop into 4", 450),
                ("Strum", 451),
                ("Arpeggio", 452),
                ("Flam", 453),
                ("Snap to C major", 454),
                ("Snap to C minor", 455),
            ],
            2002 => vec![
                ("New pattern", 80),
                ("Clone pattern", 81),
                ("Clear pattern", 82),
                ("Rename pattern…", 85),
                ("Delete pattern", 88),
                ("Previous pattern", 83),
                ("Next pattern", 84),
            ],
            2003 => vec![
                ("Channel rack", 100),
                ("Piano roll", 101),
                ("Playlist", 102),
                ("Mixer", 103),
                ("Audio editor", 104),
                ("Automation", 105),
            ],
            2004 => vec![("Tempo -", 31), ("Tempo +", 32), ("Swing", 33)],
            _ => vec![("Plugins…", 71), ("Record input", 70), ("Load WAV…", 8)],
        };
        let menu = send!(send!(class(c"NSMenu"), "alloc", Id), "init", Id);
        for (title, action) in items {
            let item = send!(send!(class(c"NSMenuItem"),"alloc",Id),"initWithTitle:action:keyEquivalent:",Id;string(title),Id;selector(c"clicked:".as_ptr()),Sel;string(""),Id);
            send!(item,"setTag:",();action,isize);
            send!(item,"setTarget:",();self.view,Id);
            send!(menu,"addItem:",();item,Id);
            send!(item, "release", ());
        }
        let x = match tag {
            2000 => 4.,
            2001 => 32.,
            2002 => 85.,
            2003 => 139.,
            2004 => 171.,
            _ => 222.,
        };
        send!(menu,"popUpMenuPositioningItem:atLocation:inView:",();ptr::null_mut(),Id;Point{x:x*self.scale,y:79.*self.scale},Point;self.view,Id);
        send!(menu, "release", ());
    }
}

pub(super) fn recovery_label(path: &std::path::Path) -> String {
    let Some(seconds) = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
    else {
        return "Recovery copy".into();
    };
    let date = send!(class(c"NSDate"),"dateWithTimeIntervalSince1970:",Id;seconds,f64);
    let formatter = send!(send!(class(c"NSDateFormatter"), "alloc", Id), "init", Id);
    send!(formatter,"setDateFormat:",();string("yyyy-MM-dd HH:mm:ss"),Id);
    let text = rust_string(send!(formatter,"stringFromDate:",Id;date,Id)).unwrap_or_default();
    send!(formatter, "release", ());
    format!("Recovery · {text}")
}
