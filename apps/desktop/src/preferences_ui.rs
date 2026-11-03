use crate::app::*;
use garrido_studio::preferences::{Settings, Store};
use std::path::Path;

pub(crate) fn load(recovery: Option<&Path>) -> (Option<Store>, Settings, String) {
    let Some(folder) = recovery.and_then(Path::parent) else {
        return (None, Settings::default(), "Listo".into());
    };
    match Store::open(folder) {
        Ok(store) => match store.load() {
            Ok(settings) => (Some(store), settings, "Listo".into()),
            Err(error) => (
                Some(store),
                Settings::default(),
                format!("{error}. Se usan los ajustes iniciales."),
            ),
        },
        Err(error) => (
            None,
            Settings::default(),
            format!("No se pueden guardar los ajustes: {error}"),
        ),
    }
}
impl Studio {
    pub(crate) fn persist_preferences(&mut self) {
        self.preferences.light = self.theme == crate::brand::Theme::Light;
        self.preferences.ghost_notes = self.workspace.ghost_notes;
        self.preferences.input = self
            .audio_preferences
            .input
            .as_ref()
            .map(ToString::to_string);
        self.preferences.output = self
            .audio_preferences
            .output
            .as_ref()
            .map(ToString::to_string);
        self.preferences.rate = self.audio_preferences.rate;
        self.preferences.buffer = self.audio_preferences.buffer;
        self.preferences.input_pair = self.audio_preferences.input_pair;
        self.preferences.all_inputs = self.audio_preferences.all_inputs;
        if let Some(store) = &self.preferences_store
            && let Err(error) = store.save(&self.preferences)
        {
            self.status = format!("Ajustes aplicados sin guardar: {error}");
        }
    }
    pub(crate) fn remember_path(&mut self, path: &Path) {
        if self.preferences_store.is_none() {
            return;
        }
        let path = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(_) => return,
        };
        if let Err(error) = self.preferences.remember(&path) {
            self.status = format!("Archivo abierto; no se ha añadido a recientes: {error}");
        } else {
            self.persist_preferences();
        }
    }
    pub(crate) fn recent_files(&self) {
        if self.preferences.recent.is_empty() {
            self.message("Recent files", "No files opened yet.");
            return;
        }
        let items = self
            .preferences
            .recent
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                (format!("{}. {}", index + 1, name), 7400 + index as isize)
            })
            .collect();
        self.popup(items, Point { x: 4., y: 70. });
    }
}
