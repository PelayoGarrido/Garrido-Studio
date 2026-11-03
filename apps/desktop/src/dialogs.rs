use crate::{app::*, canvas};
use eframe::egui::{self, Color32};
use std::cell::{Cell, RefCell};

struct Menu {
    items: Vec<(String, isize)>,
    at: Point,
    opened: u64,
    selected: usize,
}

#[derive(Default)]
pub(crate) struct Dialogs {
    pub(crate) history: Cell<bool>,
    pub(crate) audio: RefCell<Option<crate::audio_settings::AudioDialog>>,
    form: RefCell<Option<Form>>,
    pub(crate) export: RefCell<Option<crate::export_dialog::ExportDialog>>,
    answer: RefCell<Option<(String, Vec<String>)>>,
    popup: RefCell<Option<Menu>>,
    message: RefCell<Option<(String, String)>>,
}
struct Form {
    title: String,
    fields: Vec<(String, String)>,
    numeric: bool,
    action: isize,
    revision: u64,
    error: String,
}
impl Dialogs {
    pub(crate) fn active(&self) -> bool {
        self.history.get()
            || self.audio.borrow().is_some()
            || self.export.borrow().is_some()
            || self.form.borrow().is_some()
            || self.popup.borrow().is_some()
            || self.message.borrow().is_some()
    }
}
impl Studio {
    pub(crate) fn message(&self, title: &str, body: &str) {
        *self.dialogs.message.borrow_mut() = Some((title.into(), body.into()));
    }
    pub(crate) fn popup(&self, items: Vec<(String, isize)>, at: Point) {
        *self.dialogs.popup.borrow_mut() = Some(Menu {
            items,
            at,
            opened: self.ctx.cumulative_frame_nr(),
            selected: 0,
        });
    }
    fn ask(
        &self,
        title: &str,
        fields: Vec<(String, String)>,
        numeric: bool,
    ) -> Option<Vec<String>> {
        if let Some((key, answer)) = self.dialogs.answer.borrow_mut().take()
            && key == title
        {
            return Some(answer);
        }
        *self.dialogs.form.borrow_mut() = Some(Form {
            title: title.into(),
            fields,
            numeric,
            action: self.active_action,
            revision: self.revision,
            error: String::new(),
        });
        None
    }
    pub(crate) fn ask_text(&self, title: &str, value: &str) -> Option<Result<String>> {
        self.ask(title, vec![("Name".into(), value.into())], false)
            .map(|v| {
                let value = v.into_iter().next().ok_or(Error::Invalid("Falta nombre"))?;
                garrido_studio::model::text(&value)?;
                if value.trim().is_empty() {
                    return Err(Error::Invalid("Escribe un nombre"));
                }
                Ok(value)
            })
    }
    pub(crate) fn numbers(&self, title: &str, values: &[(&str, f32)]) -> Option<Result<Vec<f32>>> {
        self.ask(
            title,
            values
                .iter()
                .map(|(s, n)| (s.to_string(), format!("{n:.4}")))
                .collect(),
            true,
        )
        .map(|values| {
            values
                .iter()
                .map(|s| {
                    s.trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or(Error::Invalid("Número no válido"))
                })
                .collect()
        })
    }
    pub(crate) fn draw_dialogs(&mut self) {
        self.draw_history();
        self.draw_audio_settings();
        self.draw_export();
        let ctx = self.ctx.clone();
        let theme = self.theme;
        let surface = canvas::color(theme.surface(canvas::PANEL));
        let mut command = None;
        let mut close = false;
        {
            let mut popup = self.dialogs.popup.borrow_mut();
            if let Some(menu) = popup.as_mut() {
                if !menu.items.is_empty() {
                    ctx.input(|i| {
                        if i.key_pressed(egui::Key::ArrowDown) {
                            menu.selected = (menu.selected + 1) % menu.items.len();
                        }
                        if i.key_pressed(egui::Key::ArrowUp) {
                            menu.selected =
                                (menu.selected + menu.items.len() - 1) % menu.items.len();
                        }
                        if i.key_pressed(egui::Key::Enter) {
                            command = Some(menu.items[menu.selected].1);
                        }
                    });
                }
                let at = menu.at;
                let items = &menu.items;
                let response = egui::Area::new(egui::Id::unique("studio-menu"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(
                        (at.x * self.scale) as f32,
                        (at.y * self.scale) as f32,
                    ))
                    .show(&ctx, |ui| {
                        ui.style_mut().override_font_id =
                            Some(egui::FontId::proportional((14. * self.scale) as f32));
                        ui.spacing_mut().item_spacing.y = (2. * self.scale) as f32;
                        egui::Frame::popup(ui.style())
                            .fill(surface)
                            .stroke(egui::Stroke::new(
                                1.,
                                canvas::color(theme.stroke(canvas::EDGE)),
                            ))
                            .show(ui, |ui| {
                                ui.set_min_width((220. * self.scale) as f32);
                                egui::ScrollArea::vertical()
                                    .max_height(500.)
                                    .show(ui, |ui| {
                                        for (index, (title, tag)) in items.iter().enumerate() {
                                            if ui
                                                .add_sized(
                                                    [
                                                        ui.available_width(),
                                                        (22. * self.scale) as f32,
                                                    ],
                                                    egui::Button::new("")
                                                        .left_text(
                                                            egui::RichText::new(title).color(
                                                                if index == menu.selected {
                                                                    canvas::color(
                                                                        crate::brand::PLUM,
                                                                    )
                                                                } else {
                                                                    canvas::color(
                                                                        theme.ink(canvas::TEXT),
                                                                    )
                                                                },
                                                            ),
                                                        )
                                                        .frame(true)
                                                        .fill(if index == menu.selected {
                                                            canvas::color(canvas::ACCENT)
                                                        } else {
                                                            surface
                                                        })
                                                        .stroke(egui::Stroke::NONE)
                                                        .selected(index == menu.selected),
                                                )
                                                .clicked()
                                            {
                                                command = Some(*tag);
                                            }
                                        }
                                    });
                            });
                    });
                close = command.is_some()
                    || (ctx.cumulative_frame_nr() > menu.opened
                        && response.response.clicked_elsewhere())
                    || ctx.input(|i| i.key_pressed(egui::Key::Escape));
            }
            if close {
                *popup = None;
            }
        }
        if let Some(tag) = command {
            self.action(tag);
        }
        let mut submit = None;
        let mut cancel = false;
        {
            let mut form = self.dialogs.form.borrow_mut();
            if let Some(f) = form.as_mut() {
                egui::Modal::new(egui::Id::unique("studio-form")).show(&ctx, |ui| {
                    ui.set_width(380.);
                    ui.heading(&f.title);
                    ui.add_space(10.);
                    for (label, value) in &mut f.fields {
                        ui.horizontal(|ui| {
                            ui.add_sized([210., 24.], egui::Label::new(label.as_str()));
                            ui.add_sized(
                                [150., 24.],
                                egui::TextEdit::singleline(value).char_limit(512),
                            );
                        });
                    }
                    if !f.error.is_empty() {
                        ui.colored_label(Color32::LIGHT_RED, &f.error);
                    }
                    ui.add_space(10.);
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            let values: Vec<_> = f.fields.iter().map(|(_, s)| s.clone()).collect();
                            let valid = if f.numeric {
                                values
                                    .iter()
                                    .all(|s| s.trim().parse::<f32>().is_ok_and(|v| v.is_finite()))
                            } else {
                                values.iter().all(|s| {
                                    !s.trim().is_empty() && garrido_studio::model::text(s).is_ok()
                                })
                            };
                            if f.revision != self.revision {
                                f.error = "La sesión ha cambiado; vuelve a abrir el editor".into();
                            } else if valid {
                                submit = Some((f.action, f.title.clone(), values));
                            } else {
                                f.error = "Revisa los valores".into();
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
                cancel |= ctx.input(|i| i.key_pressed(egui::Key::Escape));
            }
            if cancel || submit.is_some() {
                *form = None;
            }
        }
        if let Some((action, title, values)) = submit {
            *self.dialogs.answer.borrow_mut() = Some((title, values));
            self.action(action);
        }
        let mut message = self.dialogs.message.borrow_mut();
        if let Some((title, body)) = message.as_ref() {
            let mut done = false;
            egui::Modal::new(egui::Id::unique("studio-message")).show(&ctx, |ui| {
                ui.set_width(420.);
                ui.heading(title);
                ui.label(body);
                done = ui.button("OK").clicked();
            });
            if done || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                *message = None;
            }
        }
    }
}
