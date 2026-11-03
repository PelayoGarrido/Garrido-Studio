use crate::{app::*, canvas::*, controls, dialogs::Dialogs, workspace::Workspace};
use eframe::egui::{
    self, FontData, FontDefinitions, FontFamily, FontId, Key, Sense, ViewportCommand,
};

#[derive(Clone, Copy)]
pub(crate) struct Input {
    pub(crate) point: Point,
    pub(crate) modifiers: usize,
    pub(crate) click_count: isize,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
}
fn recovery_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|p| p.join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let path = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "linux")]
    let path = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|p| p.join(".local/state"))
        });
    path.filter(|p| p.is_absolute())
        .map(|p| p.join("Garrido Studio/Recovery"))
}
impl Studio {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        Self::from_context(cc.egui_ctx.clone(), recovery_path())
    }
    fn from_context(ctx: egui::Context, recovery_folder: Option<PathBuf>) -> Result<Self> {
        let (preferences_store, preferences, mut status) =
            crate::preferences_ui::load(recovery_folder.as_deref());
        let audio_preferences = match crate::audio_settings::Preferences::restore(&preferences) {
            Ok(value) => value,
            Err(error) => {
                status = format!("{error}. Selecciona los dispositivos en OPTIONS.");
                Default::default()
            }
        };
        let mut fonts = FontDefinitions::default();
        let cuprum = FontData::from_static(include_bytes!("../../../assets/fonts/Cuprum.ttf"));
        fonts.font_data.insert("Cuprum".into(), Arc::new(cuprum));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "Cuprum".into());
        fonts.font_data.insert(
            "Tabler".into(),
            Arc::new(FontData::from_static(include_bytes!(
                "../../../assets/fonts/Tabler.ttf"
            ))),
        );
        fonts
            .families
            .insert(FontFamily::Name("Tabler".into()), vec!["Tabler".into()]);
        ctx.set_fonts(fonts);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |s| {
                s.animation_time = 0.;
                s.text_styles
                    .insert(egui::TextStyle::Body, FontId::proportional(16.));
                s.text_styles
                    .insert(egui::TextStyle::Button, FontId::proportional(16.));
                s.visuals.selection.bg_fill = color(ACCENT);
                s.visuals.selection.stroke = egui::Stroke::new(1., egui::Color32::BLACK);
            });
        }
        let theme = if preferences.light {
            crate::brand::Theme::Light
        } else {
            crate::brand::Theme::Dark
        };
        theme.apply(&ctx);
        let logo = ctx.load_texture(
            "garrido-official-logo",
            egui::ColorImage::from_rgba_unmultiplied([128, 128], crate::brand::LOGO),
            egui::TextureOptions::LINEAR,
        );
        let history = History::new(Project::starter().with_bound_channels()?)?;
        let recovery = recovery_folder
            .as_ref()
            .and_then(|p| garrido_studio::recovery::Store::open(p).ok());
        Ok(Self {
            theme,
            logo,
            audio_preferences,
            preferences_store,
            preferences: preferences.clone(),
            scanner: None,
            saved: Some(history.project.clone()),
            history,
            ctx,
            dialogs: Dialogs::default(),
            active_action: 0,
            window_drag: false,
            selected: 0,
            page: Page::Steps,
            status,
            dirty: false,
            job: None,
            playback: None,
            peak: 0.,
            rendered: false,
            rms: 0.,
            voices: 0,
            width: 1920.,
            height: 1080.,
            selection: (0., 1.),
            anchor: None,
            workspace: Workspace {
                ghost_notes: preferences.ghost_notes,
                ..Default::default()
            },
            capture: None,
            capture_position: None,
            pending_takes: Vec::new(),
            last_recording: None,
            plugin_path: None,
            plugin_instruments: Vec::new(),
            selected_pattern: None,
            scale: 2. / 3.,
            song_mode: false,
            metronome: true,
            recovery,
            recovery_folder,
            recovery_paths: Vec::new(),
            backup: None,
            backup_at: Instant::now(),
            backup_paused: false,
            revision: 0,
            backup_revision: 0,
            audio_clipboard: None,
            last_press: None,
        })
    }
    fn keyboard(&mut self) {
        let ctx = self.ctx.clone();
        let tag = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                else {
                    return None;
                };
                if modifiers.command {
                    match key {
                        Key::N => Some(9010),
                        Key::S => Some(3),
                        Key::O => Some(4),
                        Key::Z => Some(if modifiers.shift { 7 } else { 6 }),
                        Key::A => Some(440),
                        Key::C => Some(if self.page == Page::Audio { 530 } else { 441 }),
                        Key::X => Some(if self.page == Page::Audio { 531 } else { 442 }),
                        Key::V => Some(if self.page == Page::Audio { 532 } else { 443 }),
                        _ => None,
                    }
                } else if !modifiers.alt && !modifiers.ctrl {
                    match key {
                        Key::Space => Some(1),
                        Key::Delete | Key::Backspace => Some(444),
                        Key::F5 => Some(102),
                        Key::F6 => Some(100),
                        Key::F7 => Some(101),
                        Key::F9 => Some(103),
                        _ => None,
                    }
                } else {
                    None
                }
            })
        });
        if let Some(tag) = tag {
            self.action(tag);
        }
    }
    fn command(&mut self, tag: isize) {
        match tag {
            9000 => self.ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            9001 => {
                let maximized = self.ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                self.ctx
                    .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            9002 => self.ctx.send_viewport_cmd(ViewportCommand::Close),
            _ => self.action(tag),
        }
    }
}
impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.workspace_ui(ui);
    }
}
impl Studio {
    fn workspace_ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            if self.dialogs.active() {
                self.status = "Cierra el diálogo antes de abrir otro archivo".into();
            } else if dropped.len() == 1 && dropped[0].path().is_absolute() {
                self.open_path(dropped[0].path());
            } else {
                self.status = "Arrastra un único archivo local".into();
            }
        }
        if ui.ctx().input(|i| i.viewport().close_requested()) && !self.may_leave() {
            ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
        }
        let area = ui.max_rect();
        self.scale = (area.width() as f64 / 1920.)
            .min(area.height() as f64 / 1080.)
            .max(0.1);
        self.width = area.width() as f64 / self.scale;
        self.height = area.height() as f64 / self.scale;
        self.workspace.layout(self.width, self.height);
        let c = Canvas::new(ui.painter().clone(), area.min, self.scale, self.theme);
        self.draw_workspace(&c);
        let response = ui.interact(
            area,
            egui::Id::unique("studio-workspace"),
            Sense::click_and_drag(),
        );
        let blocked = self.dialogs.active();
        let mut selected = None;
        for mut control in controls::controls() {
            if (50..=56).contains(&control.tag) {
                if !self.workspace.visible(3) {
                    continue;
                }
                let panel = self.workspace.audio_rect();
                control.r = rect(
                    panel.origin.x + 12. + (control.tag - 50) as f64 * 88.,
                    panel.origin.y + 27.,
                    84.,
                    26.,
                );
            }
            let button = ui.interact(
                c.rect(control.r),
                egui::Id::unique(("studio-control", control.tag)),
                Sense::click(),
            );
            button.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, !blocked, control.name)
            });
            if button.has_focus() {
                c.outline(control.r, 3., 1., ACCENT);
            }
            if !blocked && button.clicked() {
                selected = Some(control.tag);
            }
            button.on_hover_text(control.name);
        }
        if !blocked {
            if let Some(tag) = selected {
                self.command(tag);
            }
            let (pointer, pressed, right, down, released, modifiers, scroll) = ui.input(|i| {
                (
                    i.pointer.interact_pos(),
                    i.pointer.primary_pressed(),
                    i.pointer.secondary_pressed(),
                    i.pointer.primary_down(),
                    i.pointer.primary_released(),
                    i.events
                        .iter()
                        .find_map(|event| {
                            if let egui::Event::PointerButton {
                                pressed: true,
                                modifiers,
                                ..
                            } = event
                            {
                                Some(*modifiers)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(i.modifiers),
                    i.smooth_scroll_delta,
                )
            });
            if let Some(pos) = pointer {
                let point = c.design_point(pos);
                let flags = (if modifiers.command { 1 << 20 } else { 0 })
                    | (if modifiers.shift { 1 << 17 } else { 0 })
                    | (if modifiers.alt { 1 << 19 } else { 0 })
                    | (if modifiers.ctrl { 1 << 18 } else { 0 });
                let mut input = Input {
                    point,
                    modifiers: flags,
                    click_count: 1,
                    dx: 0.,
                    dy: 0.,
                };
                if (pressed || right)
                    && (response.is_pointer_button_down_on()
                        || response.clicked_by(egui::PointerButton::Primary)
                        || response.clicked_by(egui::PointerButton::Secondary)
                        || response.drag_stopped())
                {
                    if response.double_clicked()
                        || self.last_press.is_some_and(|(at, p)| {
                            at.elapsed() < Duration::from_millis(400)
                                && (p.x - point.x).abs() < 8.
                                && (p.y - point.y).abs() < 8.
                        })
                    {
                        input.click_count = 2;
                    }
                    let origin = ui.input(|i| {
                        i.events.iter().find_map(|event| {
                            if let egui::Event::PointerButton {
                                pos, pressed: true, ..
                            } = event
                            {
                                Some(*pos)
                            } else {
                                None
                            }
                        })
                    });
                    input.point = origin.map_or(point, |p| c.design_point(p));
                    self.last_press = Some((Instant::now(), input.point));
                    self.mouse(input, right);
                    if self.window_drag {
                        self.ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                        self.window_drag = false;
                    }
                }
                input.point = point;
                if (down || released) && self.workspace.drag.is_some() {
                    self.drag(input);
                }
                if response.hovered() && scroll != egui::Vec2::ZERO {
                    input.dx = scroll.x as f64;
                    input.dy = scroll.y as f64;
                    self.scroll(input);
                }
            }
            if released {
                self.release();
            }
            if selected.is_none() {
                self.keyboard();
            }
        }
        self.draw_dialogs();
        let delay = if self.playback.is_some() || self.capture.is_some() {
            Duration::from_millis(33)
        } else if self.job.is_some() || self.backup.is_some() || self.scanner.is_some() {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(1)
        };
        self.ctx.request_repaint_after(delay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};
    fn studio() -> Studio {
        Studio::from_context(egui::Context::default(), None).expect("sesión inicial válida")
    }
    fn frame(studio: &mut Studio, mut events: Vec<Event>) {
        let modifiers = events
            .iter()
            .find_map(|event| {
                if let Event::Key { modifiers, .. } = event {
                    Some(*modifiers)
                } else {
                    None
                }
            })
            .unwrap_or_default();
        events.insert(0, Event::ModifiersChanged(modifiers));
        let ctx = studio.ctx.clone();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1920., 1080.),
                )),
                events,
                ..Default::default()
            },
            |ui| studio.workspace_ui(ui),
        );
        output.textures_delta.clear();
    }
    fn click(studio: &mut Studio, x: f32, y: f32) {
        let pos = egui::pos2(x, y);
        frame(
            studio,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            studio,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn wide_step_sequencer_hits_the_first_and_last_steps_and_undoes_cleanly() {
        let mut s = studio();
        frame(&mut s, vec![]);
        let panel = s.workspace.panels[0].rect;
        let width = crate::workspace::step_width(panel);
        for step in [0, 15] {
            click(
                &mut s,
                (panel.origin.x + 229. + (step as f64 + 0.5) * width) as f32,
                (panel.origin.y + 60.) as f32,
            );
        }
        let notes = &s.history.project.patterns[0].notes;
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].tick, 0);
        assert_eq!(notes[1].tick, 15 * PPQ / 4);
        s.action(6);
        assert_eq!(s.history.project.patterns[0].notes.len(), 1);
        s.action(6);
        assert!(s.history.project.patterns[0].notes.is_empty());
    }
    #[test]
    fn playlist_shortcuts_edit_clips_without_deleting_pattern_notes() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.history.project.patterns[0].notes =
            vec![Note::new(0, 100, 60, 0.6).on_channel(0).unwrap()];
        click(&mut s, 483., 170.);
        let notes = s.history.project.patterns[0].notes.clone();
        let view = s.workspace.arrangement(s.width, s.height);
        click(&mut s, view.x_at(2 * BAR) as f32 + 2., 139.);
        assert!(s.page == Page::Arrange);
        assert_eq!(s.workspace.arrangement_cursor, 2 * BAR);
        s.action(440);
        s.action(441);
        s.action(443);
        assert_eq!(s.history.project.clips.len(), 2);
        s.action(440);
        s.action(444);
        assert!(s.history.project.clips.is_empty());
        assert_eq!(s.history.project.patterns[0].notes, notes);
        s.action(6);
        assert_eq!(s.history.project.clips.len(), 2);
    }
    #[test]
    fn imported_playlist_audio_is_a_direct_clip_with_undoable_cut_and_join() {
        use garrido_studio::arrangement::ClipId;
        let mut s = studio();
        frame(&mut s, vec![]);
        s.page = Page::Arrange;
        s.workspace.arrangement_cursor = PPQ;
        s.workspace.arrangement_lane = 6;
        let path = std::env::temp_dir().join(format!("garrido-arrange-{}.wav", std::process::id()));
        let bytes = wav::encode(&vec![[0.1, -0.2]; 8000], 8000, wav::Depth::Pcm24, false).unwrap();
        storage::write_new(&path, &bytes).unwrap();
        let before = s.history.project.clone();
        s.open_path(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(s.history.project.patterns, before.patterns);
        assert_eq!(s.history.project.audio_clips.len(), 1);
        let clip = &s.history.project.audio_clips[0];
        assert_eq!((clip.tick, clip.track), (PPQ, 6));
        let sample = s.history.project.tracks[1].sample.clone();
        s.workspace.selected_clips = vec![ClipId::Audio(0)];
        s.workspace.arrangement_cursor = PPQ + PPQ / 2;
        s.action(9060);
        assert_eq!(s.history.project.audio_clips.len(), 2);
        s.action(440);
        s.action(9061);
        assert_eq!(s.history.project.audio_clips.len(), 1);
        assert_eq!(s.history.project.tracks[1].sample, sample);
        s.action(6);
        s.action(6);
        s.action(6);
        assert_eq!(s.history.project, before);
    }
    #[test]
    fn keyboard_activation_does_not_toggle_transport_twice() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.ctx
            .memory_mut(|m| m.request_focus(egui::Id::unique(("studio-control", 1isize))));
        frame(
            &mut s,
            vec![Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(s.job.is_some());
        assert!(s.playback.is_none());
        s.stop();
    }
    #[test]
    fn complete_click_between_frames_edits_the_canvas() {
        let mut s = studio();
        frame(&mut s, vec![]);
        let pos = egui::pos2(1275., 500.);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(s.history.project.patterns[0].notes.len(), 1);
        assert!(s.workspace.drag.is_none());
        s.action(6);
        assert!(s.history.project.patterns[0].notes.is_empty());
    }
    #[test]
    fn canvas_edit_survives_codec_and_undo_redo() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 1275., 500.);
        let p = &s.history.project;
        assert_eq!(p.patterns[0].notes.len(), 1);
        assert_eq!(p.patterns[0].notes[0].tick, PPQ);
        assert!(s.dirty);
        let copy = codec::decode(&codec::encode(p).unwrap()).unwrap();
        assert_eq!(&copy, p);
        s.action(6);
        assert!(s.history.project.patterns[0].notes.is_empty());
        s.action(7);
        assert_eq!(s.history.project.patterns[0].notes.len(), 1);
    }
    #[test]
    fn toolbar_focus_does_not_block_undo_shortcut() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 910., 22.);
        assert_eq!(s.history.project.bpm, 131.);
        frame(
            &mut s,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
            ],
        );
        assert_eq!(s.history.project.bpm, 130.);
    }
    #[test]
    fn note_drag_has_one_undo_transaction() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 1275., 500.);
        let baseline = s.history.project.clone();
        let pos = egui::pos2(1280., 500.);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(&mut s, vec![Event::PointerMoved(egui::pos2(1328., 482.))]);
        frame(
            &mut s,
            vec![Event::PointerButton {
                pos: egui::pos2(1328., 482.),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let moved = &s.history.project.patterns[0].notes[0];
        assert_eq!(moved.tick, baseline.patterns[0].notes[0].tick + PPQ / 2);
        assert_eq!(moved.key, baseline.patterns[0].notes[0].key + 1);
        s.action(6);
        assert_eq!(s.history.project, baseline);
    }
    #[test]
    fn final_drag_position_is_committed_in_the_release_frame() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 1275., 500.);
        let baseline = s.history.project.clone();
        let pos = egui::pos2(1280., 500.);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let end = egui::pos2(1328., 482.);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(end),
                Event::PointerButton {
                    pos: end,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let moved = &s.history.project.patterns[0].notes[0];
        assert_eq!(moved.tick, baseline.patterns[0].notes[0].tick + PPQ / 2);
        assert_eq!(moved.key, baseline.patterns[0].notes[0].key + 1);
        s.action(6);
        assert_eq!(s.history.project, baseline);
    }
    #[test]
    fn released_modifier_still_applies_to_its_pointer_event() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 1275., 500.);
        click(&mut s, 1371., 500.);
        for x in [1280., 1376.] {
            let pos = egui::pos2(x, 500.);
            frame(
                &mut s,
                vec![
                    Event::ModifiersChanged(Modifiers::COMMAND),
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::COMMAND,
                    },
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::COMMAND,
                    },
                    Event::ModifiersChanged(Modifiers::NONE),
                ],
            );
        }
        assert_eq!(s.workspace.selected_notes.len(), 2);
        assert_eq!(s.history.project.patterns[0].notes.len(), 2);
    }
    #[test]
    fn channels_share_a_pattern_and_edit_only_the_selected_notes() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.action(8000);
        click(&mut s, 1275., 500.);
        s.action(8003);
        click(&mut s, 1371., 500.);
        assert_eq!(s.history.project.tracks.len(), 3);
        assert_eq!(s.history.project.patterns.len(), 1);
        let before = s.history.project.patterns[0].clone();
        assert_eq!(before.notes.len(), 2);
        assert_eq!(before.notes[0].channel.unwrap().index(), 1);
        assert_eq!(before.notes[1].channel.unwrap().index(), 2);
        s.selected = 1;
        s.action(35);
        let after = &s.history.project.patterns[0];
        assert_eq!(after.notes[0].key, before.notes[0].key + 1);
        assert_eq!(after.notes[1], before.notes[1]);
        s.action(6);
        assert_eq!(s.history.project.patterns[0], before);
    }
    #[test]
    fn selecting_and_placing_patterns_does_not_select_the_playlist_row_as_an_instrument() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.action(8000);
        click(&mut s, 1275., 500.);
        s.select_pattern(0);
        assert_eq!(s.selected, 1);
        click(&mut s, 483., 170.);
        assert_eq!(s.history.project.clips.len(), 1);
        assert_eq!(s.history.project.clips[0].track, 0);
        assert_eq!(s.selected, 1);
        s.action(84);
        assert_eq!(s.selected, 1);
        click(&mut s, 457., 185.);
        assert_eq!(s.history.project.arrangement_mutes, 1);
        assert_eq!(s.selected, 1);
    }
    #[test]
    fn clipboard_paste_moves_notes_to_the_destination_channel() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.action(8000);
        click(&mut s, 1275., 500.);
        s.action(441);
        s.action(8003);
        s.action(443);
        let p = &s.history.project.patterns[0];
        assert_eq!(p.notes.len(), 2);
        assert!(p.notes.iter().any(|n| n.channel.unwrap().index() == 1));
        assert!(p.notes.iter().any(|n| n.channel.unwrap().index() == 2));
    }
    #[test]
    fn arrangement_navigation_places_on_the_visible_lane_and_time() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.workspace.arrangement_start = 100 * BAR;
        s.workspace.arrangement_span = 4 * BAR;
        s.workspace.arrangement_row = 12.;
        click(&mut s, 483., 170.);
        let clip = &s.history.project.clips[0];
        assert_eq!((clip.tick, clip.track, clip.pattern), (100 * BAR, 12, 0));
        assert_eq!(s.selected, 0);
        assert_eq!(s.history.project.bars, 101);
        s.action(6);
        assert!(s.history.project.clips.is_empty());
    }
    #[test]
    fn resizing_a_pattern_clip_is_a_single_undo_transaction() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 483., 170.);
        let before = s.history.project.clone();
        let view = s.workspace.arrangement(s.width, s.height);
        let from = egui::pos2((view.x_at(BAR) - 3.) as f32, 170.);
        let to = egui::pos2((view.x_at(3 * BAR) - 3.) as f32, 170.);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(from),
                Event::PointerButton {
                    pos: from,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(&mut s, vec![Event::PointerMoved(to)]);
        frame(
            &mut s,
            vec![Event::PointerButton {
                pos: to,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(s.history.project.clips[0].length, 3 * BAR);
        assert_eq!(s.history.project.clips[0].tick, 0);
        s.action(6);
        assert_eq!(s.history.project, before);
        s.action(7);
        assert_eq!(s.history.project.clips[0].length, 3 * BAR);
    }
    #[test]
    fn rectangle_selection_ignores_ghost_channels_and_keeps_the_session_clean() {
        let mut s = studio();
        frame(&mut s, vec![]);
        s.action(8000);
        let (grid, base) = s.piano_geometry(s.workspace.panels[1].rect);
        let key = base + 10;
        s.history.project.patterns[0].notes = vec![
            Note::new(PPQ, PPQ / 4, key, 0.6).on_channel(1).unwrap(),
            Note::new(PPQ * 2, PPQ / 4, key, 0.6).on_channel(0).unwrap(),
            Note::new(PPQ * 3, PPQ / 4, key, 0.6).on_channel(1).unwrap(),
        ];
        s.workspace.piano_view.set(Some((0, base)));
        s.workspace.piano_select = true;
        s.saved = Some(s.history.project.clone());
        s.dirty = false;
        let high = base + (grid.size.height / 18.).ceil() as u8 - 1;
        let y = grid.origin.y + (high - key) as f64 * 18.;
        let from = egui::pos2((grid.origin.x + 90.) as f32, (y - 4.) as f32);
        let to = egui::pos2((grid.origin.x + 250.) as f32, (y + 24.) as f32);
        frame(
            &mut s,
            vec![
                Event::PointerMoved(from),
                Event::PointerButton {
                    pos: from,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut s,
            vec![
                Event::PointerMoved(to),
                Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(s.workspace.selected_notes, vec![0]);
        assert_eq!(s.saved.as_ref().unwrap(), &s.history.project);
        assert!(!s.dirty);
    }
    #[test]
    fn damaged_open_keeps_unsaved_notes_and_never_loads_plugin_files() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 1275., 500.);
        let before = s.history.project.clone();
        s.open_path(std::path::Path::new("untrusted.clap"));
        assert_eq!(s.history.project, before);
        assert!(s.dirty);
        assert!(s.scanner.is_none());
        assert!(s.status.contains("gstudio"));
        s.open_path(std::path::Path::new("missing.gstudio"));
        assert_eq!(s.history.project, before);
        assert!(s.dirty);
    }
    #[test]
    fn light_theme_requires_an_explicit_action_and_never_changes_music() {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Light);
        let mut s = Studio::from_context(ctx, None).unwrap();
        assert!(s.theme == crate::brand::Theme::Dark);
        assert_eq!(s.ctx.theme(), egui::Theme::Dark);
        let project = s.history.project.clone();
        s.action(9043);
        assert!(s.theme == crate::brand::Theme::Light);
        assert_eq!(s.ctx.theme(), egui::Theme::Light);
        assert_eq!(s.history.project, project);
        assert!(!s.dirty);
        s.action(9042);
        assert!(s.theme == crate::brand::Theme::Dark);
        assert_eq!(s.history.project, project);
        assert!(!s.dirty);
    }
    #[test]
    fn explicit_theme_audio_choices_and_recents_survive_a_fresh_session() {
        let folder = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "garrido-ui-prefs-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let recovery = folder.join("Recovery");
        let mut s = Studio::from_context(egui::Context::default(), Some(recovery.clone())).unwrap();
        assert!(s.theme == crate::brand::Theme::Dark);
        let music = s.history.project.clone();
        s.action(9043);
        s.audio_preferences.rate = Some(96000);
        s.audio_preferences.buffer = Some(256);
        s.audio_preferences.input_pair = 1;
        s.workspace.ghost_notes = false;
        s.persist_preferences();
        let session = folder.join("Sesión Ñ.gstudio");
        storage::write_new(&session, &codec::encode(&music).unwrap()).unwrap();
        s.open_path(&session);
        assert_eq!(s.history.project, music);
        assert!(!s.dirty);
        drop(s);
        let mut fresh = Studio::from_context(egui::Context::default(), Some(recovery)).unwrap();
        assert!(fresh.theme == crate::brand::Theme::Light);
        assert_eq!(fresh.audio_preferences.rate, Some(96000));
        assert_eq!(fresh.audio_preferences.buffer, Some(256));
        assert_eq!(fresh.audio_preferences.input_pair, 1);
        assert!(!fresh.workspace.ghost_notes);
        fresh.action(9010);
        assert!(!fresh.workspace.ghost_notes);
        assert_eq!(
            fresh.preferences.recent,
            vec![std::fs::canonicalize(&session).unwrap()]
        );
        fresh.action(9042);
        assert!(
            !fresh
                .preferences_store
                .as_ref()
                .unwrap()
                .load()
                .unwrap()
                .light
        );
        drop(fresh);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn history_navigation_restores_dirty_state_and_preserves_redo() {
        let mut s = studio();
        frame(&mut s, vec![]);
        let initial = s.history.project.clone();
        click(&mut s, 1275., 500.);
        let edited = s.history.project.clone();
        assert!(s.dirty);
        s.action(93);
        assert!(s.dialogs.active());
        s.history_seek(0);
        assert_eq!(s.history.project, initial);
        assert!(!s.dirty);
        s.history_seek(1);
        assert_eq!(s.history.project, edited);
        assert!(s.dirty);
        let before = s.history.project.clone();
        s.history_seek(500);
        assert_eq!(s.history.project, before);
        s.dialogs.history.set(false);
        click(&mut s, 1375., 500.);
        assert!(s.history.labels().len() >= 3);
    }
    #[test]
    fn multitrack_recording_is_one_undo_transaction_and_failed_insertion_keeps_every_take() {
        let mut s = studio();
        let before = s.history.project.clone();
        let samples = vec![
            Sample {
                rate: 8000,
                frames: Arc::new(vec![[0.1, -0.2]; 800]),
            },
            Sample {
                rate: 8000,
                frames: Arc::new(vec![[0.3, -0.1]; 800]),
            },
        ];
        s.insert_recorded_samples(samples.clone(), PPQ, 4, false);
        assert!(s.pending_takes.is_empty());
        assert_eq!(s.history.project.audio_clips.len(), 2);
        assert_eq!(
            (
                s.history.project.audio_clips[0].tick,
                s.history.project.audio_clips[0].track
            ),
            (PPQ, 4)
        );
        assert_eq!(
            (
                s.history.project.audio_clips[1].tick,
                s.history.project.audio_clips[1].track
            ),
            (PPQ, 5)
        );
        s.action(6);
        assert_eq!(s.history.project, before);
        s.insert_recorded_samples(samples.clone(), PPQ, MAX_LANES - 1, true);
        assert_eq!(s.history.project, before);
        assert_eq!(s.pending_takes, samples);
    }
    #[test]
    fn aiff_import_uses_the_audio_workflow_and_preserves_its_original_bytes() {
        let folder = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("garrido-ui-aiff-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("Sample.AIF");
        let bytes = garrido_studio::aiff::encode(
            &[[0.25, -0.125]; 128],
            44100,
            wav::Depth::Pcm24,
            2,
            false,
        )
        .unwrap();
        storage::write_new(&path, &bytes).unwrap();
        let mut s = studio();
        let before = s.history.project.clone();
        s.open_path(&path);
        assert_eq!(s.history.project.tracks[0].instrument, Instrument::Sampler);
        assert_eq!(
            s.history.project.tracks[0].sample.as_ref().unwrap().rate,
            44100
        );
        assert!(s.dirty);
        s.action(6);
        assert_eq!(s.history.project, before);
        assert_eq!(storage::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn local_file_open_migrates_channels_and_imported_audio_can_be_undone() {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let folder = std::env::temp_dir().join(format!(
            "garrido-open-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&folder).unwrap();
        let session = folder.join("session.gstudio");
        let audio = folder.join("sample.WAV");
        let p = Project::demo();
        storage::write_new(&session, &codec::encode(&p).unwrap()).unwrap();
        storage::write_new(
            &audio,
            &wav::encode(&[[0.25, -0.25]; 128], 48000, wav::Depth::Pcm24, false).unwrap(),
        )
        .unwrap();
        let mut s = studio();
        s.open_path(&session);
        assert!(!s.dirty);
        assert!(
            s.history
                .project
                .patterns
                .iter()
                .flat_map(|p| &p.notes)
                .all(|n| n.channel.is_some())
        );
        let before = s.history.project.clone();
        let source = storage::read(&audio).unwrap();
        s.open_path(&audio);
        assert!(s.dirty);
        assert_eq!(
            s.history.project.tracks[0]
                .sample
                .as_ref()
                .unwrap()
                .frames
                .len(),
            128
        );
        s.action(6);
        assert_eq!(s.history.project, before);
        assert_eq!(storage::read(&audio).unwrap(), source);
        std::fs::remove_file(session).unwrap();
        std::fs::remove_file(audio).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn menus_are_semantic_and_do_not_edit_the_canvas_underneath() {
        let mut s = studio();
        frame(&mut s, vec![]);
        click(&mut s, 70., 60.);
        assert!(s.dialogs.active());
        assert_eq!(s.history.project.tracks.len(), 1);
        frame(
            &mut s,
            vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(!s.dialogs.active());
        assert!(s.history.project.patterns[0].notes.is_empty());
    }
}
