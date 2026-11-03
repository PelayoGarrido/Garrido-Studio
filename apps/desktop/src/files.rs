use crate::app::*;

impl Studio {
    pub(crate) fn open_path(&mut self, path: &std::path::Path) {
        enum Loaded {
            Session(Project),
            Imported(Project, String),
            Audio(Sample),
        }
        let result = (|| {
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .ok_or(Error::Invalid(
                    "El archivo necesita una extensión compatible",
                ))?
                .to_ascii_lowercase();
            if !matches!(
                extension.as_str(),
                "gstudio" | "flp" | "mid" | "midi" | "wav" | "flac" | "aif" | "aiff" | "aifc"
            ) {
                return Err(Error::Unsupported(
                    "Abre sesiones .gstudio, FLP, MIDI, WAV, FLAC o AIFF",
                ));
            }
            let data = storage::read(path)?;
            match extension.as_str() {
                "gstudio" => Ok(Loaded::Session(
                    codec::decode(&data)?.with_bound_channels()?,
                )),
                "flp" => {
                    let imported = garrido_studio::flp::Document::parse(data)?.import()?;
                    Ok(Loaded::Imported(
                        imported.project,
                        imported.warnings.join(" · "),
                    ))
                }
                "mid" | "midi" => {
                    let imported = midi::decode(&data)?;
                    Ok(Loaded::Imported(
                        imported.project.with_bound_channels()?,
                        format!(
                            "MIDI importado · {} eventos no representados",
                            imported.ignored_events
                        ),
                    ))
                }
                "wav" => Ok(Loaded::Audio(wav::decode(&data)?)),
                "flac" => Ok(Loaded::Audio(garrido_studio::flac::decode(&data)?)),
                "aif" | "aiff" | "aifc" => Ok(Loaded::Audio(garrido_studio::aiff::decode(&data)?)),
                _ => unreachable!(),
            }
        })();
        match result {
            Ok(Loaded::Audio(sample)) => {
                let selected = self.selected;
                let arrangement = self.page == Page::Arrange;
                let tick = self.workspace.arrangement_cursor;
                let lane = self.workspace.arrangement_lane;
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Audio")
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(64)
                    .collect::<String>();
                let result = self.history.edit_named("Import audio", |project| {
                    if arrangement {
                        garrido_studio::arrangement::add_audio(project, sample, &name, tick, lane)?;
                        return Ok(());
                    }
                    let track = project
                        .tracks
                        .get_mut(selected)
                        .ok_or(Error::Invalid("Añade un canal antes de importar audio"))?;
                    track.sample = Some(sample);
                    track.instrument = Instrument::Sampler;
                    Ok(())
                });
                if result.is_ok() {
                    self.selection = (0., 1.);
                    self.anchor = None;
                    self.changed();
                    self.status = if arrangement {
                        self.selected = self.history.project.tracks.len() - 1;
                        "Clip de audio añadido al arreglo"
                    } else {
                        "Audio importado en el canal seleccionado"
                    }
                    .into();
                    self.remember_path(path);
                }
                self.result(result);
            }
            Ok(loaded) => {
                if !self.may_leave() {
                    return;
                }
                let (project, status, imported) = match loaded {
                    Loaded::Session(project) => (project, "Sesión abierta".into(), false),
                    Loaded::Imported(project, status) => (project, status, true),
                    Loaded::Audio(_) => unreachable!(),
                };
                match History::new(project) {
                    Ok(history) => {
                        self.stop();
                        self.saved = if imported {
                            None
                        } else {
                            Some(history.project.clone())
                        };
                        self.history = history;
                        self.selected = 0;
                        self.selected_pattern = None;
                        self.workspace = workspace::Workspace::default();
                        self.selection = (0., 1.);
                        self.anchor = None;
                        self.dirty = imported;
                        self.revision = self.revision.wrapping_add(1);
                        self.status = status;
                        self.workspace.ghost_notes = self.preferences.ghost_notes;
                        self.remember_path(path);
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            Err(error) => self.status = error.to_string(),
        }
        self.redraw();
    }
    pub(crate) fn choose_path(&self, save: bool, ext: &str, name: &str) -> Option<PathBuf> {
        let extensions: &[&str] = match ext {
            "aiff" => &["aiff", "aif", "aifc"],
            "mid" => &["mid", "midi"],
            _ => &[ext],
        };
        let dialog = rfd::FileDialog::new().add_filter(ext.to_uppercase(), extensions);
        if save {
            dialog.set_file_name(name).save_file()
        } else {
            dialog.pick_file()
        }
    }
    pub(crate) fn save(&mut self) -> bool {
        let Some(path) = self.choose_path(true, "gstudio", "Session.gstudio") else {
            return false;
        };
        if path.extension().is_none_or(|s| s != "gstudio") {
            self.status = "Usa la extensión .gstudio".into();
            return false;
        }
        match codec::encode(&self.history.project).and_then(|data| storage::write_new(&path, &data))
        {
            Ok(()) => {
                self.saved = Some(self.history.project.clone());
                self.dirty = false;
                self.status = "Copia guardada".into();
                self.remember_path(&path);
                true
            }
            Err(e) => {
                self.status = format!("{e}. Usa un nombre nuevo.");
                false
            }
        }
    }
    pub(crate) fn may_leave(&mut self) -> bool {
        if self.capture.is_some() {
            self.finish_capture();
        }
        if !self.pending_takes.is_empty() {
            self.status = "Exporta la toma pendiente antes de cerrar o cambiar de sesión".into();
            return false;
        }
        if !self.dirty {
            return true;
        }
        match rfd::MessageDialog::new()
            .set_title("Hay cambios sin guardar")
            .set_description(
                "¿Guardar una copia antes de continuar? No descarta ninguna copia anterior.",
            )
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => self.save(),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }
}
