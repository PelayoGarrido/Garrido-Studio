use super::*;

impl State {
    pub(super) fn choose_path(&self, save: bool, ext: &str, name: &str) -> Option<PathBuf> {
        let panel = if save {
            send!(class(c"NSSavePanel"), "savePanel", Id)
        } else {
            send!(class(c"NSOpenPanel"), "openPanel", Id)
        };
        let types = send!(class(c"NSArray"),"arrayWithObject:",Id;string(ext),Id);
        send!(panel,"setAllowedFileTypes:",();types,Id);
        send!(panel,"setCanCreateDirectories:",();1i8,i8);
        if save {
            send!(panel,"setNameFieldStringValue:",();string(name),Id);
        } else {
            send!(panel,"setAllowsMultipleSelection:",();0i8,i8);
            send!(panel,"setCanChooseDirectories:",();0i8,i8);
        }
        if send!(panel, "runModal", isize) != 1 {
            return None;
        }
        let url = send!(panel, "URL", Id);
        rust_string(send!(url, "path", Id)).map(PathBuf::from)
    }
    pub(super) fn save(&mut self) -> bool {
        let Some(path) = self.choose_path(true, "gstudio", "Sesión.gstudio") else {
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
                true
            }
            Err(error) => {
                self.status = format!("{error}. Usa un nombre nuevo.");
                false
            }
        }
    }
    pub(super) fn may_leave(&mut self) -> bool {
        if self.capture.is_some() {
            self.finish_capture();
        }
        if self.pending_take.is_some() {
            self.status = "Exporta la toma pendiente antes de cerrar o cambiar de sesión".into();
            return false;
        }
        if !self.dirty {
            return true;
        }
        let alert = send!(send!(class(c"NSAlert"), "alloc", Id), "init", Id);
        send!(alert,"setMessageText:",();string("Hay cambios sin guardar"),Id);
        send!(alert,"setInformativeText:",();string("Guarda una copia de la sesión antes de continuar."),Id);
        for title in ["Guardar copia", "Cancelar", "Descartar"] {
            send!(alert,"addButtonWithTitle:",Id;string(title),Id);
        }
        let answer = send!(alert, "runModal", isize);
        send!(alert, "release", ());
        match answer {
            1000 => self.save(),
            1002 => true,
            _ => false,
        }
    }
}
