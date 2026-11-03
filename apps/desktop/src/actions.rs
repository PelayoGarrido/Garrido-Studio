use crate::app::*;
use crate::{canvas::*, menus};

impl Studio {
    pub(crate) fn action(&mut self, tag: isize) {
        self.active_action = tag;
        let tag = match tag {
            9301 => 100,
            9302 => 101,
            9303 => 103,
            9304 => 4,
            9305 | 9306 => 2005,
            9350 => 70,
            9351 | 9353 => 9351,
            _ => tag,
        };
        if self.arrangement_action(tag) {
            return;
        }
        match tag {
            9010 => {
                if self.may_leave() {
                    self.stop();
                    if let Ok(history) = Project::starter()
                        .with_bound_channels()
                        .and_then(History::new)
                    {
                        self.saved = Some(history.project.clone());
                        self.history = history;
                        self.workspace = workspace::Workspace::default();
                        self.workspace.ghost_notes = self.preferences.ghost_notes;
                        self.selected = 0;
                        self.selected_pattern = None;
                        self.page = Page::Steps;
                        self.dirty = false;
                        self.revision = self.revision.wrapping_add(1);
                        self.status = "Listo".into();
                    }
                }
            }
            9005 => {
                self.metronome = !self.metronome;
                self.stop();
            }
            9003 | 9004 => {
                self.stop();
                self.song_mode = tag == 9004;
            }
            9360 | 9361 => {
                self.workspace.browser_mode((tag - 9360) as u8);
            }
            9362 => {
                self.workspace.browser_mode(2);
                self.action(71);
            }
            9351 => self.message(
                "Garrido Studio",
                "Space: play/stop · Command S/O: save/open · Command Z: undo.
Piano: Command click selects, Shift drag resizes, scroll changes keys.
Double-click the piano title to maximize.
Record opens input only when pressed.",
            ),
            9352 => self.message(
                "Garrido Studio",
                "Pelayo Garrido Martínez
Independent Rust audio workstation
PolyForm Noncommercial + commercial music permission.
Cuprum: SIL OFL 1.1 · Tabler Icons: MIT.",
            ),
            2006 => {
                let items = self
                    .history
                    .project
                    .patterns
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.name.clone(), 7000 + i as isize))
                    .collect();
                self.popup(items, Point { x: 440., y: 79. });
            }
            7000..=7255 => {
                self.select_pattern((tag - 7000) as usize);
            }
            9030 => self.popup(
                vec![
                    ("None".into(), 9031),
                    ("Line".into(), 9032),
                    ("Beat".into(), 9033),
                    ("Bar".into(), 9034),
                ],
                Point { x: 1380., y: 79. },
            ),
            9031..=9034 => self
                .workspace
                .snap([1, PPQ / 4, PPQ, BAR][(tag - 9031) as usize]),
            9040 => {
                self.workspace.ghost_notes = !self.workspace.ghost_notes;
                self.persist_preferences();
            }
            9041 => self.audio_settings(),
            9042 | 9043 => {
                self.theme = if tag == 9042 {
                    crate::brand::Theme::Dark
                } else {
                    crate::brand::Theme::Light
                };
                self.theme.apply(&self.ctx);
                self.persist_preferences();
            }
            2000..=2005 => self.open_menu(tag),
            60 => {
                if let Some(path) = self.choose_path(false, "flp", "") {
                    self.open_path(&path);
                }
            }
            61 => {
                if let Some(path) = self.choose_path(true, "flp", "Session-copy.flp") {
                    let p = &self.history.project;
                    let result = p
                        .source_flp
                        .as_ref()
                        .ok_or(Error::Invalid(
                            "Abre primero un FLP. Para sesiones nuevas, usa .gstudio",
                        ))
                        .and_then(|b| garrido_studio::flp::Document::parse(b.as_ref().clone()))
                        .and_then(|d| d.update_notes_and_tempo(p))
                        .and_then(|data| storage::write_new(&path, &data));
                    if result.is_ok() {
                        self.status = "Copia FLP guardada".into();
                    }
                    self.result(result);
                }
            }
            71 => {
                if let Some(path) = self.choose_path(false, "clap", "") {
                    self.scan_plugin(path);
                }
            }
            77 => match crate::plugin_host::included() {
                Ok(path) => self.scan_plugin(path),
                Err(error) => self.status = error.to_string(),
            },
            90 => {
                if let Some(store) = &self.recovery {
                    match store.list() {
                        Ok(paths) => {
                            let items = paths
                                .iter()
                                .enumerate()
                                .map(|(i, p)| (menus::recovery_label(p), 6000 + i as isize))
                                .collect();
                            self.recovery_paths = paths;
                            if self.recovery_paths.is_empty() {
                                self.status = "No hay copias de recuperación".into();
                            } else {
                                self.popup(items, Point { x: 4., y: 70. });
                            }
                        }
                        Err(e) => self.status = e.to_string(),
                    }
                } else {
                    self.status = "La carpeta de recuperación no está disponible".into();
                }
            }
            91 => {
                self.backup_paused = false;
                self.start_backup();
            }
            92 => self.recent_files(),
            93 => self.dialogs.history.set(true),
            7400..=7415 => {
                if let Some(path) = self.preferences.recent.get((tag - 7400) as usize).cloned() {
                    self.open_path(&path);
                }
            }
            6000..=6255 => {
                if let Some(path) = self.recovery_paths.get((tag - 6000) as usize).cloned()
                    && self.may_leave()
                {
                    let result = storage::read(&path)
                        .and_then(|data| codec::decode(&data))
                        .and_then(|p| p.with_bound_channels())
                        .and_then(History::new);
                    match result {
                        Ok(history) => {
                            self.stop();
                            self.history = history;
                            self.saved = None;
                            self.selected = 0;
                            self.selected_pattern = None;
                            self.workspace.reset_selection();
                            self.changed();
                            self.status =
                                "Sesión recuperada. Guarda una copia para conservarla.".into();
                        }
                        Err(e) => self.status = e.to_string(),
                    }
                }
            }
            85..=87 => {
                let current = match tag {
                    85 => self
                        .pattern()
                        .map(|i| self.history.project.patterns[i].name.clone()),
                    86 => self
                        .history
                        .project
                        .tracks
                        .get(self.selected)
                        .map(|t| t.name.clone()),
                    _ => Some(self.history.project.title.clone()),
                };
                if let Some(current) = current
                    && let Some(name) = self.ask_text("Rename", &current)
                {
                    let selected = self.selected;
                    let pattern = self.pattern();
                    let result = name.and_then(|name| {
                        self.history.edit_named("Rename", |p| {
                            match tag {
                                85 => {
                                    p.patterns
                                        [pattern.ok_or(Error::Invalid("Selecciona un patrón"))?]
                                    .name = name
                                }
                                86 => p.tracks[selected].name = name,
                                _ => p.title = name,
                            }
                            Ok(())
                        })
                    });
                    if result.is_ok() {
                        self.changed();
                    }
                    self.result(result);
                }
            }
            88 => {
                if let Some(index) = self.pattern() {
                    let result = self.history.edit(|p| {
                        p.patterns.remove(index);
                        p.clips.retain(|c| c.pattern != index);
                        for c in &mut p.clips {
                            if c.pattern > index {
                                c.pattern -= 1;
                            }
                        }
                        Ok(())
                    });
                    if result.is_ok() {
                        self.selected_pattern = None;
                        self.workspace.reset_selection();
                        self.changed();
                    }
                    self.result(result);
                }
            }
            83 | 84 => {
                let count = self.history.project.patterns.len();
                if count > 0 {
                    let current = self.pattern().unwrap_or(0);
                    self.select_pattern(if tag == 83 {
                        (current + count - 1) % count
                    } else {
                        (current + 1) % count
                    });
                }
            }
            440 => {
                if let Some(index) = self.pattern() {
                    self.workspace.selected_notes = self.channel_note_indices(index);
                }
            }
            441..=444 | 450..=455 => {
                if let Some(index) = self.pattern() {
                    let pattern = self.history.project.patterns[index].clone();
                    let indices: Vec<_> = if self.workspace.selected_notes.is_empty() {
                        self.channel_note_indices(index)
                    } else {
                        self.workspace.selected_notes.clone()
                    };
                    let result = (|| -> Result<()> {
                        if tag == 441 || tag == 442 {
                            self.workspace.clipboard =
                                Some(garrido_studio::notes::copy(&pattern, &indices)?);
                        }
                        if tag == 441 {
                            return Ok(());
                        }
                        let next = if tag == 443 {
                            let clipboard = self
                                .workspace
                                .clipboard
                                .as_ref()
                                .ok_or(Error::Invalid("No hay notas copiadas"))?;
                            garrido_studio::notes::paste_on_channel(
                                &pattern,
                                clipboard,
                                self.workspace.cursor,
                                self.selected,
                            )?
                        } else {
                            use garrido_studio::notes::Tool;
                            let tool = match tag {
                                442 | 444 => Tool::Delete,
                                450 => Tool::Chop(4),
                                451 => Tool::Strum(30),
                                452 => Tool::Arpeggio(PPQ / 4),
                                453 => Tool::Flam(60),
                                454 => Tool::Scale {
                                    root: 0,
                                    minor: false,
                                },
                                _ => Tool::Scale {
                                    root: 0,
                                    minor: true,
                                },
                            };
                            garrido_studio::notes::apply(&pattern, &indices, tool)?
                        };
                        self.history.edit(|p| {
                            p.patterns[index] = next;
                            Ok(())
                        })
                    })();
                    if result.is_ok() && tag != 441 {
                        self.changed();
                    }
                    self.result(result);
                }
            }
            4600 => {
                let selected = self.selected;
                if let Some(track) = self.history.project.tracks.get(selected)
                    && let Some(values) = self.numbers(
                        "Channel",
                        &[("Gain 0–2", track.gain), ("Pan −1–1", track.pan)],
                    )
                {
                    let result = values.and_then(|v| {
                        self.history.edit(|p| {
                            p.tracks[selected].gain = v[0];
                            p.tracks[selected].pan = v[1];
                            Ok(())
                        })
                    });
                    if result.is_ok() {
                        self.changed();
                    }
                    self.result(result);
                }
            }
            4700..=4955 | 5100 | 5102 => self.edit_route(tag),
            5101 => {
                let selected = self.selected;
                let result = self.history.edit(|p| {
                    p.routes.retain(|r| r.from != selected);
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            4100..=4106 | 4200..=4207 | 4300 => self.edit_effect(tag),
            3000..=3127 => {
                self.apply_clap((tag - 3000) as usize);
            }
            70 => {
                if self.capture.is_some() {
                    self.finish_capture();
                } else if !self.pending_takes.is_empty() {
                    self.status = "Exporta primero la toma pendiente desde FILE".into();
                } else if self.history.project.tracks.len() >= MAX_TRACKS {
                    self.status = "No queda un canal libre para grabar".into();
                } else {
                    let Some(folder) = self
                        .recovery_folder
                        .as_ref()
                        .and_then(|path| path.parent())
                        .map(|path| path.join("Recordings"))
                    else {
                        self.status =
                            "No hay una carpeta local disponible para conservar las tomas".into();
                        return;
                    };
                    let used: usize = self
                        .history
                        .project
                        .tracks
                        .iter()
                        .filter_map(|track| track.sample.as_ref())
                        .map(|sample| sample.frames.len())
                        .sum();
                    let slots = (MAX_TRACKS - self.history.project.tracks.len())
                        .min(MAX_LANES.saturating_sub(self.workspace.arrangement_lane));
                    match Capture::start(
                        &self.audio_preferences,
                        &folder,
                        slots,
                        4_000_000usize.saturating_sub(used),
                    ) {
                        Ok(c) => {
                            self.status = format!(
                                "Grabando · máximo {:.1} s · Record o Stop para terminar",
                                c.limit_seconds()
                            );
                            self.capture = Some(c);
                            self.capture_position = Some((
                                self.workspace.arrangement_cursor,
                                self.workspace.arrangement_lane,
                            ));
                        }
                        Err(e) => self.status = e.to_string(),
                    }
                }
            }
            75 => {
                if let Some(sample) = self.pending_takes.first().cloned()
                    && let Some(path) = self.choose_path(true, "wav", "Recorded-take.wav")
                {
                    let result = wav::encode(&sample.frames, sample.rate, wav::Depth::Pcm24, true)
                        .and_then(|data| storage::write_new(&path, &data));
                    if result.is_ok() {
                        self.pending_takes.remove(0);
                        self.status =
                            format!("Toma exportada · {} pendientes", self.pending_takes.len());
                    }
                    self.result(result);
                }
            }
            76 => {
                if !self.pending_takes.is_empty() || self.capture.is_some() {
                    self.status = "Termina o exporta primero las tomas pendientes".into();
                    return;
                }
                let directory = self.last_recording.clone().or_else(|| {
                    self.recovery_folder
                        .as_ref()
                        .and_then(|p| p.parent())
                        .map(|p| p.join("Recordings"))
                });
                let mut dialog = rfd::FileDialog::new().add_filter("Recorded WAV", &["wav"]);
                if let Some(directory) = directory {
                    dialog = dialog.set_directory(directory);
                }
                if let Some(path) = dialog.pick_file() {
                    match garrido_studio::recording::recover(&path) {
                        Ok((sample, repaired)) => {
                            self.insert_recorded_samples(
                                vec![sample],
                                self.workspace.arrangement_cursor,
                                self.workspace.arrangement_lane,
                                repaired,
                            );
                        }
                        Err(error) => self.status = error.to_string(),
                    }
                }
            }
            80 => {
                let result = self.history.edit(|p| {
                    if p.patterns.len() >= 256 {
                        return Err(Error::Limit("Máximo 256 patrones"));
                    }
                    let pattern = p.patterns.len();
                    p.patterns.push(Pattern {
                        name: format!("Pattern {}", pattern + 1),
                        length: BAR,
                        notes: Vec::new(),
                    });
                    Ok(())
                });
                if result.is_ok() {
                    self.selected_pattern = Some(self.history.project.patterns.len() - 1);
                    self.changed();
                }
                self.result(result);
            }
            81 => {
                if let Some(index) = self.pattern() {
                    let result = self.history.edit(|p| {
                        if p.patterns.len() >= 256 {
                            return Err(Error::Limit("Máximo 256 patrones"));
                        }
                        let mut cloned = p.patterns[index].clone();
                        cloned.name =
                            format!("{} copy", cloned.name.chars().take(40).collect::<String>());
                        p.patterns.push(cloned);
                        Ok(())
                    });
                    if result.is_ok() {
                        self.selected_pattern = Some(self.history.project.patterns.len() - 1);
                        self.changed();
                    }
                    self.result(result);
                }
            }
            82 => {
                if let Some(index) = self.pattern() {
                    let result = self.history.edit(|p| {
                        p.patterns[index].notes.clear();
                        Ok(())
                    });
                    if result.is_ok() {
                        self.changed();
                    }
                    self.result(result);
                }
            }
            1 => {
                if self.playback.is_some() || self.job.is_some() {
                    self.stop();
                    self.status = "Detenido".into();
                } else {
                    self.begin(Goal::Play);
                }
            }
            2 => {
                if self.capture.is_some() {
                    self.finish_capture();
                }
                self.stop();
                self.status = "Detenido".into();
            }
            3 => {
                self.save();
            }
            4 => {
                if let Some(path) = self.choose_path(false, "gstudio", "") {
                    self.open_path(&path);
                }
            }
            5 | 94 | 97 => self.open_export(match tag {
                94 => garrido_studio::export::Format::Aiff,
                97 => garrido_studio::export::Format::Flac,
                _ => garrido_studio::export::Format::Wav,
            }),
            6 => {
                if self.history.undo() {
                    self.workspace.selected_clips.clear();
                    self.changed();
                    self.selected = self
                        .selected
                        .min(self.history.project.tracks.len().saturating_sub(1));
                }
            }
            7 => {
                if self.history.redo() {
                    self.workspace.selected_clips.clear();
                    self.changed();
                }
            }
            8 | 95 | 98 => {
                let ext = match tag {
                    95 => "aiff",
                    98 => "flac",
                    _ => "wav",
                };
                if let Some(path) = self.choose_path(false, ext, "") {
                    self.open_path(&path);
                }
            }
            9 => {
                self.popup(
                    Instrument::ALL
                        .iter()
                        .enumerate()
                        .map(|(i, k)| (k.name().into(), 8000 + i as isize))
                        .collect(),
                    Point { x: 58., y: 79. },
                );
            }
            8000..=8006 => {
                let index = self.history.project.tracks.len();
                let result = self.history.edit(|p| {
                    p.add_channel(Instrument::ALL[(tag - 8000) as usize])?;
                    if p.patterns.is_empty() {
                        p.patterns.push(Pattern {
                            name: "Pattern 1".into(),
                            length: BAR,
                            notes: Vec::new(),
                        });
                    }
                    Ok(())
                });
                if result.is_ok() {
                    self.selected = index;
                    self.workspace.piano_view.set(None);
                    self.changed();
                }
                self.result(result);
            }
            10 => {
                if self.may_leave() {
                    self.stop();
                    if let Ok(history) =
                        Project::demo().with_bound_channels().and_then(History::new)
                    {
                        self.saved = Some(history.project.clone());
                        self.history = history;
                        self.selected = 0;
                        self.selected_pattern = None;
                        self.workspace.reset_selection();
                        self.dirty = false;
                        self.status = "Primer pulso · sonidos sintetizados aquí".into();
                    }
                }
            }
            11 => {
                if let Some(path) = self.choose_path(false, "mid", "") {
                    self.open_path(&path);
                }
            }
            12 => {
                if let Some(path) = self.choose_path(true, "mid", "Partitura.mid") {
                    let result = midi::encode(&self.history.project)
                        .and_then(|data| storage::write_new(&path, &data));
                    self.status = match result {
                        Ok(()) => "MIDI exportado".into(),
                        Err(e) => e.to_string(),
                    };
                }
            }
            30 => {
                let selected = self.selected;
                let result = self.history.edit(|p| {
                    let t = p
                        .tracks
                        .get_mut(selected)
                        .ok_or(Error::Invalid("Selecciona un canal"))?;
                    t.instrument =
                        Instrument::ALL[(t.instrument as usize + 1) % Instrument::ALL.len()];
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            31 | 32 => {
                let amount = if tag == 31 { -1. } else { 1. };
                let result = self.history.edit(|p| {
                    p.bpm = (p.bpm + amount).clamp(20., 300.);
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            33 => {
                let result = self.history.edit(|p| {
                    p.swing = if p.swing >= 0.45 { 0. } else { p.swing + 0.15 };
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            34..=39 => {
                if let Some(index) = self.pattern() {
                    let channel = self.selected;
                    let label = [
                        "Quantize notes",
                        "Transpose notes",
                        "Transpose notes",
                        "Reverse notes",
                        "Legato",
                        "Humanize",
                    ][(tag - 34) as usize];
                    let result = self.history.edit_named(label, |p| {
                        p.patterns[index] = garrido_studio::notes::edit_channel(
                            &p.patterns[index],
                            channel,
                            |pattern| match tag {
                                34 => history::quantize(pattern, PPQ / 4, 1.),
                                35 => history::transpose(pattern, 1),
                                36 => history::transpose(pattern, -1),
                                37 => history::reverse(pattern),
                                38 => history::legato(pattern),
                                39 => history::humanize(pattern, 0x123456, 20, 0.05),
                                _ => {}
                            },
                        )?;
                        Ok(())
                    });
                    if result.is_ok() {
                        self.changed();
                    }
                    self.result(result);
                }
            }
            40 => self.effects_menu(),
            531 if self.page != Page::Audio => self.action(442),
            530..=533 => {
                let selected = self.selected;
                if let Some(sample) = self
                    .history
                    .project
                    .tracks
                    .get(selected)
                    .and_then(|t| t.sample.clone())
                {
                    let start = (self.selection.0 * sample.frames.len() as f64).floor() as usize;
                    let end = (self.selection.1 * sample.frames.len() as f64).ceil() as usize;
                    let result = (|| -> Result<()> {
                        if tag == 530 || tag == 531 {
                            self.audio_clipboard = Some(edit::copy(&sample, start, end)?);
                        }
                        if tag == 530 {
                            return Ok(());
                        }
                        if tag == 533 {
                            if let Some(path) = self.choose_path(true, "wav", "Selection.wav") {
                                let selection = edit::copy(&sample, start, end)?;
                                storage::write_new(
                                    &path,
                                    &wav::encode(
                                        &selection.frames,
                                        selection.rate,
                                        wav::Depth::Pcm24,
                                        true,
                                    )?,
                                )?;
                            }
                            return Ok(());
                        }
                        let replacement = if tag == 532 {
                            Some(
                                self.audio_clipboard
                                    .as_ref()
                                    .ok_or(Error::Invalid("No hay audio copiado"))?,
                            )
                        } else {
                            None
                        };
                        let edited = edit::splice(&sample, start, end, replacement)?;
                        self.history.edit(|p| {
                            p.tracks[selected].sample = Some(edited);
                            Ok(())
                        })
                    })();
                    if result.is_ok() && (tag == 531 || tag == 532) {
                        self.selection = (0., 1.);
                        self.anchor = None;
                        self.changed();
                    }
                    self.result(result);
                }
            }
            50..=56 => {
                let selected = self.selected;
                let selection = self.selection;
                let result = self.history.edit(|p| {
                    let t = p
                        .tracks
                        .get_mut(selected)
                        .ok_or(Error::Invalid("Selecciona un canal"))?;
                    let sample = t
                        .sample
                        .as_ref()
                        .ok_or(Error::Invalid("Carga primero un WAV en este canal"))?;
                    let start = (selection.0 * sample.frames.len() as f64).floor() as usize;
                    let end = (selection.1 * sample.frames.len() as f64).ceil() as usize;
                    let edited = match tag {
                        50 => edit::apply(sample, start, end, Edit::Reverse)?,
                        51 => edit::apply(sample, start, end, Edit::Normalize)?,
                        52 => edit::apply(sample, start, end, Edit::FadeIn)?,
                        53 => edit::apply(sample, start, end, Edit::FadeOut)?,
                        54 => edit::apply(sample, start, end, Edit::Trim)?,
                        55 => stretch::granular(sample, 1.25, 0.)?,
                        _ => stretch::granular(sample, 1., 1.)?,
                    };
                    t.sample = Some(edited);
                    Ok(())
                });
                if result.is_ok() {
                    self.selection = (0., 1.);
                    self.anchor = None;
                    self.changed();
                }
                self.result(result);
            }
            100..=105 => {
                self.page = match tag {
                    100 => Page::Steps,
                    101 => Page::Piano,
                    102 => Page::Arrange,
                    103 => Page::Mixer,
                    104 => Page::Audio,
                    _ => Page::Automation,
                };
                self.workspace.show(self.page);
            }
            _ => {}
        }
        self.redraw();
    }
}
