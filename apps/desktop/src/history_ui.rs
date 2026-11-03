use crate::app::*;
use eframe::egui;

impl Studio {
    pub(crate) fn history_seek(&mut self, index: usize) {
        if self.capture.is_some() {
            self.status = "Termina la grabación antes de cambiar de estado".into();
            return;
        }
        match self.history.seek(index) {
            Ok(true) => {
                self.workspace.reset_selection();
                self.workspace.drag = None;
                self.selected = self
                    .selected
                    .min(self.history.project.tracks.len().saturating_sub(1));
                self.selected_pattern = self
                    .selected_pattern
                    .filter(|p| *p < self.history.project.patterns.len());
                self.selection = (0., 1.);
                self.anchor = None;
                self.changed();
                self.status = format!("Historial · {}", self.history.labels()[index]);
            }
            Ok(false) => {}
            Err(error) => self.status = error.to_string(),
        }
    }
    pub(crate) fn draw_history(&mut self) {
        if !self.dialogs.history.get() {
            return;
        }
        let mut open = true;
        let mut selected = None;
        let labels: Vec<_> = self
            .history
            .labels()
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let current = self.history.position();
        egui::Window::new("History")
            .open(&mut open).default_width(360.).default_height(380.)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&self.ctx, |ui| {
                ui.label("Select a state to return to it. Later steps remain available until a new edit.");
                ui.add_space(8.);
                ui.add_enabled_ui(self.capture.is_none(), |ui| {
                    egui::ScrollArea::vertical().max_height(350.).show(ui, |ui| {
                        for (index, label) in labels.iter().enumerate() {
                            let text = format!("{index:02}   {label}");
                            if ui.selectable_label(index == current, text).clicked() { selected = Some(index); }
                        }
                    });
                });
                if self.capture.is_some() { ui.label("Finish recording before changing the session state."); }
            });
        if let Some(index) = selected {
            self.history_seek(index);
        }
        if self.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.dialogs.history.set(open);
    }
}
