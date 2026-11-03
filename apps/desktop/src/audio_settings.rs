use crate::app::*;
use cpal::{
    DeviceId, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use eframe::egui;

#[derive(Clone, Default)]
pub(crate) struct Preferences {
    pub(crate) output: Option<DeviceId>,
    pub(crate) input: Option<DeviceId>,
    pub(crate) rate: Option<u32>,
    pub(crate) buffer: Option<u32>,
    pub(crate) input_pair: u16,
    pub(crate) all_inputs: bool,
}
impl Preferences {
    pub(crate) fn restore(settings: &garrido_studio::preferences::Settings) -> Result<Self> {
        let parse = |value: &Option<String>| {
            value
                .as_ref()
                .map(|v| {
                    v.parse::<DeviceId>().map_err(|_| {
                        Error::Invalid("Identificador de audio no compatible con este sistema")
                    })
                })
                .transpose()
        };
        let preferences = Self {
            input: parse(&settings.input)?,
            output: parse(&settings.output)?,
            rate: settings.rate,
            buffer: settings.buffer,
            input_pair: settings.input_pair,
            all_inputs: settings.all_inputs,
        };
        preferences.validate()?;
        Ok(preferences)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self
            .rate
            .is_some_and(|rate| !(8000..=192000).contains(&rate))
            || self.buffer.is_some_and(|size| !(32..=4096).contains(&size))
            || self.input_pair >= 16
        {
            return Err(Error::Invalid(
                "Frecuencia o tamaño de buffer fuera de límites",
            ));
        }
        Ok(())
    }
    pub(crate) fn device(&self, input: bool) -> Result<cpal::Device> {
        self.validate()?;
        let host = cpal::default_host();
        let id = if input { &self.input } else { &self.output };
        let device = if let Some(id) = id {
            host.devices().ok().and_then(|mut devices| {
                devices
                    .by_ref()
                    .take(64)
                    .find(|device| device.id().ok().as_ref() == Some(id))
            })
        } else if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        };
        device.ok_or(Error::Unsupported(
            "El dispositivo de audio seleccionado no está disponible",
        ))
    }
    pub(crate) fn config(
        &self,
        device: &cpal::Device,
        input: bool,
    ) -> Result<SupportedStreamConfig> {
        self.validate()?;
        let default = if input {
            device.default_input_config()
        } else {
            device.default_output_config()
        }
        .map_err(error)?;
        if self.rate.is_none_or(|rate| rate == default.sample_rate()) {
            return Ok(default);
        }
        let rate = self
            .rate
            .ok_or(Error::Invalid("Falta la frecuencia de audio"))?;
        let ranges: Vec<_> = if input {
            device
                .supported_input_configs()
                .map_err(error)?
                .take(256)
                .collect()
        } else {
            device
                .supported_output_configs()
                .map_err(error)?
                .take(256)
                .collect()
        };
        ranges
            .into_iter()
            .filter(|range| {
                range.channels() == default.channels()
                    && range.sample_format() == default.sample_format()
            })
            .find_map(|range| range.try_with_sample_rate(rate))
            .ok_or(Error::Unsupported(
                "El dispositivo no admite esa frecuencia con su formato actual",
            ))
    }
    pub(crate) fn stream_config(
        &self,
        supported: &SupportedStreamConfig,
    ) -> Result<cpal::StreamConfig> {
        self.validate()?;
        let mut config = supported.config();
        if let Some(size) = self.buffer {
            if let cpal::SupportedBufferSize::Range { min, max } = supported.buffer_size()
                && (size < *min || size > *max)
            {
                return Err(Error::Unsupported(
                    "El dispositivo no admite ese tamaño de buffer",
                ));
            }
            config.buffer_size = cpal::BufferSize::Fixed(size);
        }
        Ok(config)
    }
}
fn error(error: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(error.to_string()))
}
struct Choice {
    id: DeviceId,
    label: String,
}
pub(crate) struct AudioDialog {
    preferences: Preferences,
    inputs: Vec<Choice>,
    outputs: Vec<Choice>,
    error: String,
}
fn devices(input: bool) -> Vec<Choice> {
    let host = cpal::default_host();
    let available = if input {
        host.input_devices()
    } else {
        host.output_devices()
    };
    let Ok(available) = available else {
        return Vec::new();
    };
    available
        .take(64)
        .filter_map(|device| {
            Some(Choice {
                id: device.id().ok()?,
                label: device
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(96)
                    .collect(),
            })
        })
        .collect()
}
fn device_picker(
    ui: &mut egui::Ui,
    label: &str,
    selected: &mut Option<DeviceId>,
    choices: &[Choice],
) {
    let name = selected.as_ref().map_or("System default", |id| {
        choices
            .iter()
            .find(|choice| &choice.id == id)
            .map_or("Unavailable", |choice| choice.label.as_str())
    });
    egui::ComboBox::from_label(label)
        .selected_text(name)
        .width(300.)
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "System default");
            for choice in choices {
                ui.selectable_value(selected, Some(choice.id.clone()), &choice.label);
            }
        });
}
impl Studio {
    pub(crate) fn audio_settings(&self) {
        *self.dialogs.audio.borrow_mut() = Some(AudioDialog {
            preferences: self.audio_preferences.clone(),
            inputs: devices(true),
            outputs: devices(false),
            error: String::new(),
        });
    }
    pub(crate) fn draw_audio_settings(&mut self) {
        let Some(mut dialog) = self.dialogs.audio.borrow_mut().take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let popup_open = egui::Popup::is_any_open(&self.ctx);
        egui::Window::new("Audio settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(460.)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&self.ctx, |ui| {
                device_picker(
                    ui,
                    "Output",
                    &mut dialog.preferences.output,
                    &dialog.outputs,
                );
                device_picker(ui, "Input", &mut dialog.preferences.input, &dialog.inputs);
                egui::ComboBox::from_label("Input channel pair")
                    .selected_text(format!(
                        "{}–{}",
                        dialog.preferences.input_pair * 2 + 1,
                        dialog.preferences.input_pair * 2 + 2
                    ))
                    .show_ui(ui, |ui| {
                        for pair in 0..16 {
                            ui.selectable_value(
                                &mut dialog.preferences.input_pair,
                                pair,
                                format!("{}–{}", pair * 2 + 1, pair * 2 + 2),
                            );
                        }
                    });
                ui.checkbox(&mut dialog.preferences.all_inputs, "Record all input pairs into separate channels");
                egui::ComboBox::from_label("Device sample rate")
                    .selected_text(
                        dialog
                            .preferences
                            .rate
                            .map_or("System default".into(), |rate| format!("{rate} Hz")),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut dialog.preferences.rate, None, "System default");
                        for rate in [44100, 48000, 88200, 96000, 192000] {
                            ui.selectable_value(
                                &mut dialog.preferences.rate,
                                Some(rate),
                                format!("{rate} Hz"),
                            );
                        }
                    });
                egui::ComboBox::from_label("Buffer frames")
                    .selected_text(
                        dialog
                            .preferences
                            .buffer
                            .map_or("System default".into(), |size| size.to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut dialog.preferences.buffer, None, "System default");
                        for size in [64, 128, 256, 512, 1024, 2048] {
                            ui.selectable_value(
                                &mut dialog.preferences.buffer,
                                Some(size),
                                size.to_string(),
                            );
                        }
                    });
                ui.label(
                    "Input opens only when you press Record. Your choices are saved on this computer.",
                );
                if !dialog.error.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, &dialog.error);
                }
                ui.horizontal(|ui| {
                    if ui.button("Refresh devices").clicked() {
                        dialog.inputs = devices(true);
                        dialog.outputs = devices(false);
                    }
                    apply = ui.button("Apply").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if !popup_open && self.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if apply {
            match dialog.preferences.validate() {
                Ok(()) => {
                    if self.capture.is_some() {
                        self.finish_capture();
                    }
                    self.stop();
                    self.audio_preferences = dialog.preferences;
                    self.status = "Ajustes de audio aplicados".into();
                    self.persist_preferences();
                    return;
                }
                Err(error) => dialog.error = error.to_string(),
            }
        }
        if open && !cancel {
            *self.dialogs.audio.borrow_mut() = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_audio_preferences_and_unsupported_buffers_are_rejected_without_devices() {
        let mut preferences = Preferences {
            rate: Some(1),
            ..Default::default()
        };
        assert!(preferences.validate().is_err());
        preferences.rate = Some(48000);
        preferences.buffer = Some(512);
        let supported = SupportedStreamConfig::new(
            2,
            48000,
            cpal::SupportedBufferSize::Range { min: 64, max: 256 },
            cpal::SampleFormat::F32,
        );
        assert!(preferences.stream_config(&supported).is_err());
        preferences.buffer = Some(128);
        assert_eq!(
            preferences.stream_config(&supported).unwrap().buffer_size,
            cpal::BufferSize::Fixed(128)
        );
        preferences.buffer = None;
        assert_eq!(
            preferences.stream_config(&supported).unwrap().buffer_size,
            cpal::BufferSize::Default
        );
    }
}
