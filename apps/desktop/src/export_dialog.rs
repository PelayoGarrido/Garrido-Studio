use crate::app::*;
use eframe::egui;
use garrido_studio::export::{Format, Scope, Settings};

pub(crate) struct ExportDialog {
    revision: u64,
    format: Format,
    scope: u8,
    rate: u32,
    depth: u8,
    channels: u16,
    tail: f64,
    error: String,
}
impl Studio {
    pub(crate) fn open_export(&self, format: Format) {
        *self.dialogs.export.borrow_mut() = Some(ExportDialog {
            revision: self.revision,
            format,
            scope: 0,
            rate: self.history.project.rate,
            depth: 24,
            channels: 2,
            tail: 2.,
            error: String::new(),
        });
    }
    pub(crate) fn draw_export(&mut self) {
        let Some(mut form) = self.dialogs.export.borrow_mut().take() else {
            return;
        };
        let ctx = self.ctx.clone();
        let mut export = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("audio-export")).show(&ctx, |ui| {
            ui.set_width(390.);
            ui.heading(match form.format {
                Format::Wav => "Export WAV",
                Format::Flac => "Export FLAC",
                Format::Aiff => "Export AIFF",
            });
            ui.add_space(12.);
            egui::Grid::new("export-settings")
                .spacing([25., 10.])
                .show(ui, |ui| {
                    ui.label("Source");
                    egui::ComboBox::from_id_salt("export-scope")
                        .selected_text(
                            ["Song", "Selected channel", "Selected pattern", "Channel stems"][form.scope as usize],
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.scope, 0, "Song");
                            ui.selectable_value(&mut form.scope, 1, "Selected channel");
                            ui.selectable_value(&mut form.scope, 2, "Selected pattern");
                            ui.selectable_value(&mut form.scope, 3, "Channel stems");
                        });
                    ui.end_row();
                    ui.label("Sample rate");
                    egui::ComboBox::from_id_salt("export-rate")
                        .selected_text(format!("{} Hz", form.rate))
                        .show_ui(ui, |ui| {
                            for rate in [22050, 32000, 44100, 48000, 88200, 96000, 192000] {
                                ui.selectable_value(&mut form.rate, rate, format!("{rate} Hz"));
                            }
                        });
                    ui.end_row();
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("export-depth")
                        .selected_text(match form.depth {
                            16 => "16-bit PCM",
                            24 => "24-bit PCM",
                            _ => "32-bit float",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.depth, 16, "16-bit PCM");
                            ui.selectable_value(&mut form.depth, 24, "24-bit PCM");
                            if form.format == Format::Wav {
                                ui.selectable_value(&mut form.depth, 32, "32-bit float");
                            }
                        });
                    ui.end_row();
                    ui.label("Channels");
                    egui::ComboBox::from_id_salt("export-channels")
                        .selected_text(if form.channels == 1 { "Mono" } else { "Stereo" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.channels, 1, "Mono");
                            ui.selectable_value(&mut form.channels, 2, "Stereo");
                        });
                    ui.end_row();
                    ui.label("Effect tail");
                    ui.add(
                        egui::DragValue::new(&mut form.tail)
                            .range(0. ..=5.)
                            .speed(0.1)
                            .suffix(" s"),
                    );
                    ui.end_row();
                });
            ui.add_space(10.);
            ui.label("A new file is created. Existing audio is never replaced.");
            if form.scope == 3 { ui.label("Stems include channel effects and sends. Shared effects may change their combined sound."); }
            if !form.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &form.error);
            }
            ui.add_space(10.);
            ui.horizontal(|ui| {
                export = ui.button("Export…").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        cancel |= ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if cancel {
            return;
        }
        if export {
            let result = (|| -> Result<Settings> {
                if form.revision != self.revision {
                    return Err(Error::Invalid(
                        "La sesión ha cambiado; vuelve a abrir la exportación",
                    ));
                }
                let settings = Settings {
                    scope: match form.scope {
                        0 | 3 => Scope::Mix,
                        1 => Scope::Track(self.selected),
                        _ => Scope::Pattern {
                            track: self.selected,
                            pattern: self
                                .pattern()
                                .ok_or(Error::Invalid("Selecciona un patrón"))?,
                        },
                    },
                    rate: form.rate,
                    channels: form.channels,
                    depth: match form.depth {
                        16 => wav::Depth::Pcm16,
                        24 => wav::Depth::Pcm24,
                        _ => wav::Depth::Float32,
                    },
                    tail: form.tail,
                };
                settings.validate()?;
                Ok(settings)
            })();
            match result {
                Ok(settings) => {
                    if form.scope == 3 {
                        if let Some(parent) = rfd::FileDialog::new()
                            .set_title("Choose a folder for stems")
                            .pick_folder()
                        {
                            self.begin(Goal::Stems(parent, settings, form.format));
                            return;
                        }
                        *self.dialogs.export.borrow_mut() = Some(form);
                        return;
                    }
                    let ext = form.format.extension();
                    if let Some(path) = self.choose_path(true, ext, &format!("Mix.{ext}")) {
                        if path.extension().is_some_and(|s| s == ext) {
                            self.begin(Goal::Export(path, settings));
                            return;
                        }
                        form.error = format!("Usa la extensión .{ext}");
                    }
                }
                Err(e) => form.error = e.to_string(),
            }
        }
        *self.dialogs.export.borrow_mut() = Some(form);
    }
}
