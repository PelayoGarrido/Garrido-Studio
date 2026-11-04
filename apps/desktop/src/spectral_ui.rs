use crate::app::*;
use eframe::egui;
use garrido_studio::spectral::{self, Comparison, Settings};

#[derive(Clone)]
enum Source {
    Audio(String, Sample),
    File(PathBuf),
}

impl Source {
    fn name(&self) -> String {
        match self {
            Self::Audio(name, _) => name.clone(),
            Self::File(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(80)
                .collect(),
        }
    }
    fn load(&self) -> Result<Sample> {
        match self {
            Self::Audio(_, sample) => Ok(sample.clone()),
            Self::File(path) => {
                let bytes = storage::read_bounded(path, 24 * 1024 * 1024)?;
                if bytes.starts_with(b"fLaC") {
                    garrido_studio::flac::decode(&bytes)
                } else if bytes.starts_with(b"FORM") {
                    garrido_studio::aiff::decode(&bytes)
                } else {
                    wav::decode(&bytes)
                }
            }
        }
    }
}

struct Worker {
    rx: mpsc::Receiver<Result<Comparison>>,
    cancel: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) struct JourneyDialog {
    sources: [Option<Source>; 2],
    settings: Settings,
    modes: usize,
    revision: u64,
    tick: u32,
    lane: usize,
    worker: Option<Worker>,
    output: Option<Comparison>,
    error: String,
}

impl JourneyDialog {
    fn begin(&mut self) -> Result<()> {
        self.settings.validate()?;
        let [Some(a), Some(b)] = self.sources.clone() else {
            return Err(Error::Invalid("Elige las dos fuentes de audio"));
        };
        let settings = self.settings;
        let modes = self.modes;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.output = None;
        let handle = thread::Builder::new()
            .name("garrido-spectral".into())
            .spawn(move || {
                let result = (|| {
                    let a = spectral::analyze(&a.load()?, modes, &flag)?;
                    let b = spectral::analyze(&b.load()?, modes, &flag)?;
                    spectral::compare(&a, &b, settings, &flag)
                })();
                let _ = tx.send(result);
            })?;
        self.worker = Some(Worker {
            rx,
            cancel,
            handle: Some(handle),
        });
        Ok(())
    }
    fn poll(&mut self) {
        let result = self
            .worker
            .as_ref()
            .and_then(|worker| match worker.rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(Error::Invalid("El proceso espectral se ha detenido")))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.worker = None;
            match result {
                Ok(output) => {
                    self.output = Some(output);
                    self.error.clear();
                }
                Err(error) => self.error = error.to_string(),
            }
        }
    }
}

impl Studio {
    pub(crate) fn open_journey(&mut self) {
        if self.capture.is_some() {
            self.status = "Termina la grabación antes de abrir Spectral Journey".into();
            return;
        }
        self.stop();
        let source = self
            .history
            .project
            .tracks
            .get(self.selected)
            .and_then(|t| t.sample.clone().map(|s| Source::Audio(t.name.clone(), s)));
        *self.dialogs.spectral.borrow_mut() = Some(JourneyDialog {
            sources: [source, None],
            settings: Settings {
                rate: self.history.project.rate,
                ..Default::default()
            },
            modes: 24,
            revision: self.revision,
            tick: self.workspace.arrangement_cursor,
            lane: self.workspace.arrangement_lane,
            worker: None,
            output: None,
            error: String::new(),
        });
    }

    pub(crate) fn insert_journey(
        &mut self,
        sample: Sample,
        revision: u64,
        tick: u32,
        lane: usize,
    ) -> Result<()> {
        if revision != self.revision || self.capture.is_some() {
            return Err(Error::Invalid(
                "La sesión ha cambiado. Abre de nuevo Spectral Journey",
            ));
        }
        self.history.edit_named("Spectral Journey", |project| {
            garrido_studio::arrangement::add_audio(
                project,
                sample,
                "Spectral Journey",
                tick,
                lane,
            )?;
            Ok(())
        })?;
        self.selected = self.history.project.tracks.len() - 1;
        self.selection = (0., 1.);
        self.anchor = None;
        self.page = Page::Arrange;
        self.workspace.show(Page::Arrange);
        self.changed();
        self.status = "Spectral Journey añadido como un clip nuevo · Undo para volver".into();
        Ok(())
    }

    pub(crate) fn draw_journey(&mut self) {
        let Some(mut dialog) = self.dialogs.spectral.borrow_mut().take() else {
            return;
        };
        dialog.poll();
        let mut open = true;
        let mut insert = false;
        egui::Window::new("Spectral Journey")
            .open(&mut open).default_width(610.).default_height(600.).resizable(true).max_height(600.).vscroll(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&self.ctx.clone(), |ui| {
                ui.heading("Move between sounds.");
                ui.label("Turn two spectral fingerprints into a new evolving texture.");
                ui.small("Resynthesis for tonal sound design. Rhythm, speech and original phase are not reconstructed.");
                ui.add_space(10.);
                let mut changed = false;
                ui.add_enabled_ui(dialog.worker.is_none(), |ui| {
                    for index in 0..2 {
                        ui.horizontal(|ui| {
                            ui.strong(if index == 0 { "A" } else { "B" });
                            ui.label(dialog.sources[index].as_ref().map_or("Choose a short source".into(), Source::name));
                            if ui.button(if index == 0 { "Load A…" } else { "Load B…" }).clicked()
                                && let Some(path) = rfd::FileDialog::new().add_filter("Audio · up to 12 seconds", &["wav", "flac", "aif", "aiff", "aifc"]).pick_file()
                            {
                                dialog.sources[index] = Some(Source::File(path)); changed = true;
                            }
                            egui::ComboBox::from_id_salt(("spectral-source", index)).selected_text("From session").show_ui(ui, |ui| {
                                let mut available = false;
                                for (channel, track) in self.history.project.tracks.iter().enumerate() {
                                    if let Some(sample) = &track.sample {
                                        available = true;
                                        if ui.selectable_label(false, format!("{} · {}", channel + 1, track.name)).clicked() {
                                            dialog.sources[index] = Some(Source::Audio(track.name.clone(), sample.clone())); changed = true;
                                        }
                                    }
                                }
                                if !available { ui.label("Import a sample into the session first."); }
                            });
                        });
                    }
                    if ui.button("Load example pair").clicked() {
                        let [a, b] = spectral::example_sources();
                        dialog.sources = [Some(Source::Audio("Harmonic source".into(), a)), Some(Source::Audio("Bell source".into(), b))]; changed = true;
                    }
                    changed |= ui.add(egui::Slider::new(&mut dialog.settings.seconds, 0.25..=12.).text("Seconds")).changed();
                    changed |= ui.add(egui::Slider::new(&mut dialog.modes, 1..=32).text("Spectral modes")).changed();
                    changed |= ui.add(egui::Slider::new(&mut dialog.settings.start, 0. ..=1.).text("Start · A → B")).changed();
                    changed |= ui.add(egui::Slider::new(&mut dialog.settings.end, 0. ..=1.).text("End · A → B")).changed();
                    ui.small("Set Start = End to explore a fixed hybrid. Reverse them for a return journey.");
                });
                if changed { dialog.output = None; self.stop(); }
                if dialog.worker.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner(); ui.label("Analyzing and rendering locally…");
                        if ui.button("Cancel render").clicked() && let Some(worker) = &dialog.worker {
                            worker.cancel.store(true, Ordering::Relaxed);
                        }
                    });
                    self.ctx.request_repaint_after(Duration::from_millis(50));
                } else if ui.button("Render journey + matched crossfade").clicked() {
                    self.stop();
                    if let Err(error) = dialog.begin() { dialog.error = error.to_string(); }
                }
                if let Some(output) = &dialog.output {
                    draw_routes(ui, &output.routes, dialog.settings);
                    ui.label("Both auditions have matched overall RMS. The crossfade uses the same resynthesized endpoints.");
                    ui.horizontal(|ui| {
                        for (label, sample) in [("Listen: Journey", &output.journey), ("Listen: Crossfade", &output.crossfade)] {
                            if ui.button(label).clicked() {
                                self.stop();
                                match Playback::start(sample.frames.as_ref().clone(), sample.rate, &self.audio_preferences) {
                                    Ok(playback) => self.playback = Some(playback),
                                    Err(error) => dialog.error = error.to_string(),
                                }
                            }
                        }
                        if ui.button("Stop audition").clicked() { self.stop(); }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Add journey as a new clip").clicked() { insert = true; }
                        if ui.button("Export journey WAV…").clicked()
                            && let Some(path) = rfd::FileDialog::new().add_filter("WAV", &["wav"]).set_file_name("Spectral-Journey.wav").save_file()
                        {
                            let result = wav::encode(&output.journey.frames, output.journey.rate, wav::Depth::Pcm24, true).and_then(|data| storage::write_new(&path, &data));
                            if let Err(error) = result { dialog.error = error.to_string(); }
                        }
                    });
                }
                if !dialog.error.is_empty() { ui.colored_label(egui::Color32::from_rgb(255, 150, 120), &dialog.error); }
            });
        if insert && let Some(output) = &dialog.output {
            match self.insert_journey(
                output.journey.clone(),
                dialog.revision,
                dialog.tick,
                dialog.lane,
            ) {
                Ok(()) => open = false,
                Err(error) => dialog.error = error.to_string(),
            }
        }
        if open {
            *self.dialogs.spectral.borrow_mut() = Some(dialog);
        } else {
            self.stop();
        }
    }
}

fn draw_routes(ui: &mut egui::Ui, paths: &[spectral::Route], settings: Settings) {
    let (response, painter) =
        ui.allocate_painter(egui::vec2(ui.available_width(), 160.), egui::Sense::hover());
    let area = response.rect.shrink(10.);
    let low = paths
        .iter()
        .map(|r| r.from_hz.min(r.to_hz))
        .fold(f64::INFINITY, f64::min)
        .ln()
        - 0.15;
    let high = paths
        .iter()
        .map(|r| r.from_hz.max(r.to_hz))
        .fold(0., f64::max)
        .ln()
        + 0.15;
    for path in paths {
        let points: Vec<_> = (0..=48)
            .map(|i| {
                let t = i as f64 / 48.;
                let amount =
                    settings.start + (settings.end - settings.start) * t * t * (3. - 2. * t);
                let hz = path.from_hz.ln() + amount * (path.to_hz / path.from_hz).ln();
                egui::pos2(
                    area.left() + t as f32 * area.width(),
                    area.bottom() - ((hz - low) / (high - low)) as f32 * area.height(),
                )
            })
            .collect();
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(
                1. + path.power.sqrt() as f32 * 5.,
                egui::Color32::from_rgb(247, 139, 149),
            ),
        ));
    }
    ui.small("Frequency paths · low → high vertically · time → horizontally");
}
