pub(crate) use crate::{
    audio::{Capture, Playback},
    canvas::Point,
    workspace,
};
pub(crate) use garrido_studio::{
    Error, Result, codec,
    edit::{self, Edit},
    engine::{Engine, Render},
    history::{self, History},
    midi,
    model::*,
    storage, stretch, wav,
};
pub(crate) use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Page {
    Steps,
    Piano,
    Arrange,
    Mixer,
    Audio,
    Automation,
}
pub(crate) enum Goal {
    Play,
    Export(PathBuf, garrido_studio::export::Settings),
    Stems(
        PathBuf,
        garrido_studio::export::Settings,
        garrido_studio::export::Format,
    ),
    Plugin(usize),
    Instrument(usize),
}
pub(crate) enum Prepared {
    Stems {
        folder: PathBuf,
        files: usize,
    },
    Render(Render),
    Playback {
        project: Box<Project>,
        engine: Box<Engine>,
        metronome: bool,
    },
}
pub(crate) struct Job {
    pub(crate) rx: mpsc::Receiver<Result<Prepared>>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
    pub(crate) goal: Goal,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
pub(crate) struct ScanJob {
    pub(crate) rx: mpsc::Receiver<Result<Vec<String>>>,
    pub(crate) path: PathBuf,
    pub(crate) revision: u64,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
}
impl Drop for ScanJob {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
pub(crate) struct BackupJob {
    pub(crate) rx: mpsc::Receiver<Result<PathBuf>>,
    pub(crate) handle: Option<thread::JoinHandle<()>>,
}
impl Drop for BackupJob {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
pub(crate) struct Studio {
    pub(crate) theme: crate::brand::Theme,
    pub(crate) logo: eframe::egui::TextureHandle,
    pub(crate) audio_preferences: crate::audio_settings::Preferences,
    pub(crate) preferences: garrido_studio::preferences::Settings,
    pub(crate) preferences_store: Option<garrido_studio::preferences::Store>,
    pub(crate) scanner: Option<ScanJob>,
    pub(crate) last_press: Option<(Instant, Point)>,
    pub(crate) ctx: eframe::egui::Context,
    pub(crate) dialogs: crate::dialogs::Dialogs,
    pub(crate) active_action: isize,
    pub(crate) window_drag: bool,
    pub(crate) recovery_folder: Option<PathBuf>,
    pub(crate) history: History,
    pub(crate) selected: usize,
    pub(crate) page: Page,
    pub(crate) status: String,
    pub(crate) dirty: bool,
    pub(crate) saved: Option<Project>,
    pub(crate) job: Option<Job>,
    pub(crate) playback: Option<Playback>,
    pub(crate) peak: f32,
    pub(crate) rendered: bool,
    pub(crate) rms: f32,
    pub(crate) voices: usize,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) selection: (f64, f64),
    pub(crate) anchor: Option<f64>,
    pub(crate) workspace: workspace::Workspace,
    pub(crate) capture: Option<Capture>,
    pub(crate) capture_position: Option<(u32, usize)>,
    pub(crate) pending_takes: Vec<Sample>,
    pub(crate) last_recording: Option<PathBuf>,
    pub(crate) plugin_path: Option<PathBuf>,
    pub(crate) plugin_instruments: Vec<bool>,
    pub(crate) selected_pattern: Option<usize>,
    pub(crate) scale: f64,
    pub(crate) song_mode: bool,
    pub(crate) metronome: bool,
    pub(crate) recovery: Option<garrido_studio::recovery::Store>,
    pub(crate) recovery_paths: Vec<PathBuf>,
    pub(crate) backup: Option<BackupJob>,
    pub(crate) backup_at: Instant,
    pub(crate) backup_paused: bool,
    pub(crate) revision: u64,
    pub(crate) backup_revision: u64,
    pub(crate) audio_clipboard: Option<Sample>,
}

impl Studio {
    pub(crate) fn redraw(&self) {
        self.ctx.request_repaint();
    }
    pub(crate) fn stop(&mut self) {
        self.job = None;
        self.playback = None;
    }
    pub(crate) fn changed(&mut self) {
        self.workspace.selected_notes.clear();
        self.revision = self.revision.wrapping_add(1);
        self.stop();
        self.dirty = self.saved.as_ref() != Some(&self.history.project);
        self.status = "Cambios listos. Pulsa Reproducir.".into();
        self.redraw();
    }
    pub(crate) fn result(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.status = error.to_string();
        }
        self.redraw();
    }
    pub(crate) fn begin(&mut self, goal: Goal) {
        self.stop();
        let p = match &goal {
            Goal::Export(_, settings) => {
                match garrido_studio::export::prepare(&self.history.project, *settings) {
                    Ok(p) => p,
                    Err(e) => {
                        self.status = e.to_string();
                        return;
                    }
                }
            }
            Goal::Play if !self.song_mode => {
                let Some(pattern) = self.pattern() else {
                    return;
                };
                match garrido_studio::export::prepare(
                    &self.history.project,
                    garrido_studio::export::Settings {
                        rate: self.history.project.rate,
                        channels: 2,
                        depth: wav::Depth::Float32,
                        tail: 0.,
                        scope: garrido_studio::export::Scope::Pattern {
                            track: self.selected,
                            pattern,
                        },
                    },
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        self.status = e.to_string();
                        return;
                    }
                }
            }
            _ => self.history.project.clone(),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let tail = match &goal {
            Goal::Play => 0.,
            Goal::Export(_, settings) => settings.tail,
            _ => 2.,
        };
        let hq = matches!(goal, Goal::Export(..));
        let export = if let Goal::Export(path, settings) = &goal {
            Some((path.clone(), *settings))
        } else {
            None
        };
        let stems = if let Goal::Stems(parent, settings, format) = &goal {
            Some((parent.clone(), *settings, *format))
        } else {
            None
        };
        let playing = matches!(goal, Goal::Play);
        let metronome = playing && self.metronome;
        match thread::Builder::new()
            .name("garrido-render".into())
            .spawn(move || {
                if let Some((parent, settings, format)) = stems {
                    let _ = tx.send(crate::stems::write(
                        &p,
                        settings,
                        format,
                        &parent,
                        &worker_cancel,
                    ));
                    return;
                }
                if playing {
                    let result = Engine::streaming(&p).and_then(|engine| {
                        if worker_cancel.load(Ordering::Relaxed) {
                            return Err(Error::Invalid("Reproducción cancelada"));
                        }
                        Ok(Prepared::Playback {
                            project: Box::new(p),
                            engine: Box::new(if metronome {
                                engine.with_metronome()
                            } else {
                                engine
                            }),
                            metronome,
                        })
                    });
                    let _ = tx.send(result);
                    return;
                }
                let result = Engine::new(&p, hq, tail)
                    .and_then(|e| {
                        if metronome { e.with_metronome() } else { e }.render(&worker_cancel)
                    })
                    .and_then(|mut render| {
                        if let Some((path, settings)) = export {
                            let format = garrido_studio::export::Format::from_extension(
                                path.extension()
                                    .and_then(|s| s.to_str())
                                    .ok_or(Error::Invalid("Falta la extensión de audio"))?,
                            )?;
                            let data = format.encode(&render.audio, settings, &worker_cancel)?;
                            if worker_cancel.load(Ordering::Relaxed) {
                                return Err(Error::Invalid("Exportación cancelada"));
                            }
                            storage::write_new(&path, &data)?;
                            render.audio.clear();
                        }
                        Ok(render)
                    });
                let _ = tx.send(result.map(Prepared::Render));
            }) {
            Ok(handle) => {
                self.job = Some(Job {
                    rx,
                    cancel,
                    handle: Some(handle),
                    goal,
                });
                self.status = "Preparando audio…".into();
            }
            Err(error) => self.status = error.to_string(),
        }
    }
    pub(crate) fn start_backup(&mut self) {
        if self.backup.is_some() || self.backup_paused || !self.dirty {
            return;
        }
        let Some(store) = self.recovery.clone() else {
            return;
        };
        let free = self
            .recovery_folder
            .as_ref()
            .and_then(|p| fs4::available_space(p).ok())
            .unwrap_or(0);
        if free < 3 * 1024 * 1024 * 1024 {
            self.status = "Recuperación en pausa: se conservan 3 GiB de espacio libre".into();
            self.backup_paused = true;
            return;
        }
        let project = self.history.project.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("garrido-recovery".into())
            .spawn(move || {
                let _ = tx.send(store.save(&project));
            }) {
            Ok(handle) => {
                self.backup = Some(BackupJob {
                    rx,
                    handle: Some(handle),
                });
                self.backup_revision = self.revision;
                self.backup_at = Instant::now();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    pub(crate) fn poll_backup(&mut self) {
        let result = self.backup.as_ref().and_then(|b| match b.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(Error::Invalid("Se interrumpió la recuperación")))
            }
            Err(_) => None,
        });
        if let Some(result) = result {
            self.backup = None;
            if let Err(e) = result {
                self.status = e.to_string();
                self.backup_paused = true;
                self.redraw();
            }
        }
        if self.backup_at.elapsed() >= Duration::from_secs(60)
            && self.revision != self.backup_revision
        {
            self.start_backup();
        }
    }
    pub(crate) fn scan_plugin(&mut self, path: PathBuf) {
        if self.scanner.is_some() {
            self.status = "Ya hay un análisis CLAP en curso".into();
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let source = path.clone();
        match thread::Builder::new()
            .name("garrido-scan".into())
            .spawn(move || {
                let _ = tx.send(crate::plugin_host::scan(&source));
            }) {
            Ok(handle) => {
                self.scanner = Some(ScanJob {
                    rx,
                    path,
                    revision: self.revision,
                    handle: Some(handle),
                });
                self.status = "Analizando CLAP en un proceso aislado…".into();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn poll_scanner(&mut self) {
        let result = self
            .scanner
            .as_ref()
            .and_then(|job| match job.rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(Error::Invalid("El análisis CLAP se ha interrumpido")))
                }
                Err(_) => None,
            });
        if let Some(result) = result
            && let Some(job) = self.scanner.take()
        {
            if job.revision != self.revision {
                self.status =
                    "La sesión ha cambiado durante el análisis. Vuelve a seleccionar el plugin."
                        .into();
                return;
            }
            match result {
                Ok(items) => {
                    self.plugin_path = Some(job.path.clone());
                    self.plugin_instruments = items
                        .iter()
                        .map(|line| line.split('\t').nth(3) == Some("instrument"))
                        .collect();
                    let items = items
                        .iter()
                        .enumerate()
                        .map(|(i, line)| {
                            (
                                line.split('\t').nth(2).unwrap_or("CLAP").to_owned(),
                                3000 + i as isize,
                            )
                        })
                        .collect();
                    self.popup(items, Point { x: 1202., y: 79. });
                    self.status = "Análisis CLAP terminado".into();
                }
                Err(e) => self.status = e.to_string(),
            }
            self.redraw();
        }
    }
    pub(crate) fn poll(&mut self) {
        self.poll_scanner();
        self.poll_backup();
        if self.capture.as_ref().is_some_and(|c| c.done()) {
            self.finish_capture();
        }

        if self.playback.as_ref().is_some_and(|p| !p.healthy()) {
            self.stop();
            self.status =
                "La salida de audio se ha detenido. Vuelve a conectar el dispositivo.".into();
            self.redraw();
            return;
        }
        if let Some((peak, rms, voices, underruns)) =
            self.playback.as_ref().and_then(Playback::meters)
        {
            self.peak = peak;
            self.rms = rms;
            self.voices = voices;
            if underruns > 0 {
                self.status =
                    format!("{underruns} buffers incompletos · aumenta el buffer de salida");
            }
        }
        let result = self.job.as_ref().and_then(|j| match j.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(Error::Invalid("El render se ha detenido")))
            }
            Err(_) => None,
        });
        if let Some(result) = result {
            if let Some(job) = self.job.take() {
                match result {
                    Ok(Prepared::Stems { folder, files }) => {
                        self.status = format!("{files} stems exportados · {}", folder.display());
                    }
                    Ok(Prepared::Playback {
                        project,
                        engine,
                        metronome,
                    }) => {
                        match Playback::stream(
                            *project,
                            *engine,
                            metronome,
                            &self.audio_preferences,
                        ) {
                            Ok(playback) => {
                                self.playback = Some(playback);
                                self.rendered = true;
                                self.status =
                                    "Reproduciendo por bloques · detener antes de editar".into();
                            }
                            Err(error) => self.status = error.to_string(),
                        }
                    }
                    Ok(Prepared::Render(render)) => {
                        self.peak = render.peak;
                        self.rms = render.rms;
                        self.voices = render.peak_voices;
                        self.rendered = true;
                        match &job.goal {
                            Goal::Plugin(index) => {
                                let index = *index;
                                let sample = Sample {
                                    rate: render.rate,
                                    frames: std::sync::Arc::new(render.audio),
                                };
                                let result = self.history.edit(|p| {
                                    let t = p
                                        .tracks
                                        .get_mut(index)
                                        .ok_or(Error::Invalid("Canal no disponible"))?;
                                    t.sample = Some(sample);
                                    t.instrument = Instrument::Sampler;
                                    Ok(())
                                });
                                if result.is_ok() {
                                    self.changed();
                                    self.status="CLAP aplicado a una copia de la muestra. El archivo original se conserva.".into();
                                }
                                self.result(result);
                            }
                            Goal::Instrument(source) => {
                                let source = *source;
                                let sample = Sample {
                                    rate: render.rate,
                                    frames: Arc::new(render.audio),
                                };
                                let result = self.history.edit(|p| {
                                    let ticks =
                                        p.tick_at(sample.frames.len() as f64 / sample.rate as f64)
                                            .ceil() as u32;
                                    let bars = ticks.div_ceil(BAR).max(1);
                                    if bars > 256 {
                                        return Err(Error::Limit(
                                            "Render de instrumento demasiado largo",
                                        ));
                                    }
                                    let i = p.add_track(Instrument::Sampler)?;
                                    p.tracks[i].name = "CLAP render".into();
                                    p.tracks[i].sample = Some(sample);
                                    if let Some(track) = p.tracks.get_mut(source) {
                                        track.mute = true;
                                    }
                                    let pattern = p
                                        .patterns
                                        .last_mut()
                                        .ok_or(Error::Invalid("Falta patrón"))?;
                                    pattern.length = bars * BAR;
                                    pattern.notes =
                                        vec![Note::new(0, ticks, 60, 1.).on_channel(i)?];
                                    p.bars = p.bars.max(bars as u16);
                                    p.clips
                                        .last_mut()
                                        .ok_or(Error::Invalid("Falta clip"))?
                                        .length = ticks;
                                    Ok(())
                                });
                                if result.is_ok() {
                                    self.selected = self.history.project.tracks.len() - 1;
                                    self.selected_pattern =
                                        Some(self.history.project.patterns.len() - 1);
                                    self.selection = (0., 1.);
                                    self.anchor = None;
                                    self.changed();
                                    self.status = "Patrón CLAP convertido en muestra. Canal original silenciado y conservado.".into();
                                }
                                self.result(result);
                            }
                            Goal::Play => match Playback::start(
                                render.audio,
                                render.rate,
                                &self.audio_preferences,
                            ) {
                                Ok(playback) => {
                                    self.playback = Some(playback);
                                    self.status =
                                        "Reproduciendo en bucle · detener antes de editar".into();
                                }
                                Err(e) => self.status = e.to_string(),
                            },
                            Goal::Export(path, _) => {
                                self.status = format!(
                                    "{} exportado",
                                    path.extension()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Audio")
                                        .to_ascii_uppercase()
                                );
                            }
                            Goal::Stems(..) => {
                                self.status = "Exportación de stems terminada".into()
                            }
                        }
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            self.redraw();
        } else if self.playback.is_some() {
            self.redraw();
        }
    }
    pub(crate) fn select_pattern(&mut self, index: usize) {
        if index >= self.history.project.patterns.len() {
            return;
        }
        self.selected_pattern = Some(index);
        self.workspace.reset_selection();
        self.redraw();
    }
    pub(crate) fn pattern(&self) -> Option<usize> {
        if let Some(index) = self
            .selected_pattern
            .filter(|i| *i < self.history.project.patterns.len())
        {
            return Some(index);
        }
        (!self.history.project.patterns.is_empty()).then_some(0)
    }
    pub(crate) fn visible_note(&self, note: &Note) -> bool {
        note.channel_or(self.selected) == self.selected
    }
    pub(crate) fn channel_note_indices(&self, pattern: usize) -> Vec<usize> {
        self.history.project.patterns[pattern]
            .notes
            .iter()
            .enumerate()
            .filter(|(_, note)| self.visible_note(note))
            .map(|(i, _)| i)
            .collect()
    }
    pub(crate) fn apply_clap(&mut self, index: usize) {
        let Some(path) = self.plugin_path.clone() else {
            return;
        };
        let instrument = self.plugin_instruments.get(index).copied().unwrap_or(false);
        let sample = self
            .history
            .project
            .tracks
            .get(self.selected)
            .and_then(|t| t.sample.clone());
        if !instrument && sample.is_none() {
            self.status = "Carga o graba una muestra antes de aplicar el efecto CLAP".into();
            return;
        }
        let Some(pattern) = self.pattern() else {
            return;
        };
        if instrument && self.history.project.tracks.len() >= MAX_TRACKS {
            self.status = "No queda un canal libre para el render CLAP".into();
            return;
        }
        self.stop();
        let mut project = self.history.project.clone();
        project.patterns[pattern]
            .notes
            .retain(|note| note.channel_or(self.selected) == self.selected);
        let track = self.selected;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("garrido-clap".into())
            .spawn(move || {
                let (audio, rate) = if instrument {
                    (
                        crate::plugin_host::instrument(&path, index, &project, pattern, &flag),
                        project.rate,
                    )
                } else if let Some(sample) = sample {
                    (
                        crate::plugin_host::render(
                            &path,
                            index,
                            &sample.frames,
                            sample.rate,
                            &flag,
                        ),
                        sample.rate,
                    )
                } else {
                    return;
                };
                let result = audio.map(|audio| {
                    let peak = audio.iter().flatten().fold(0f32, |a, &b| a.max(b.abs()));
                    let musical_frames = audio.len();
                    Render {
                        audio,
                        rate,
                        peak,
                        rms: 0.,
                        peak_voices: 0,
                        stolen_voices: 0,
                        musical_frames,
                    }
                });
                let _ = tx.send(result.map(Prepared::Render));
            }) {
            Ok(handle) => {
                self.job = Some(Job {
                    rx,
                    cancel,
                    handle: Some(handle),
                    goal: if instrument {
                        Goal::Instrument(track)
                    } else {
                        Goal::Plugin(track)
                    },
                });
                self.status = "Procesando CLAP…".into();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    pub(crate) fn finish_capture(&mut self) {
        if let Some(capture) = self.capture.take() {
            self.last_recording = Some(capture.folder().to_path_buf());
            match capture.finish() {
                Ok((samples, interrupted)) => {
                    let (tick, lane) = self.capture_position.take().unwrap_or((0, 0));
                    self.insert_recorded_samples(samples, tick, lane, interrupted);
                }
                Err(e) => self.status = e.to_string(),
            }
        }
        self.redraw();
    }
    pub(crate) fn insert_recorded_samples(
        &mut self,
        samples: Vec<Sample>,
        tick: u32,
        lane: usize,
        interrupted: bool,
    ) {
        let count = samples.len();
        self.pending_takes = samples.clone();
        let result = self.history.edit_named("Record inputs", |p| {
            for (index, sample) in samples.into_iter().enumerate() {
                garrido_studio::arrangement::add_audio(
                    p,
                    sample,
                    &format!("Recorded take {}", index + 1),
                    tick,
                    lane + index,
                )?;
            }
            Ok(())
        });
        if result.is_ok() {
            self.pending_takes.clear();
            self.selected = self.history.project.tracks.len().saturating_sub(1);
            self.changed();
            self.selection = (0., 1.);
            self.anchor = None;
            self.page = if count > 1 {
                Page::Arrange
            } else {
                Page::Audio
            };
            self.workspace.show(self.page);
            self.status = if interrupted {
                format!("Entrada interrumpida · {count} tomas conservadas en la sesión y en disco")
            } else {
                format!("{count} tomas añadidas · originales conservados en disco")
            };
        } else {
            self.status = "Las tomas no caben en la sesión. FILE > Export pending take conserva cada entrada; los originales siguen en disco.".into();
        }
    }
}
pub(crate) fn effect_name(fx: &Effect) -> &'static str {
    match fx {
        Effect::Eq { .. } => "EQ",
        Effect::Delay { .. } => "Delay",
        Effect::Reverb { .. } => "Reverb",
        Effect::Chorus { .. } => "Chorus",
        Effect::Drive { .. } => "Drive",
        Effect::Width(_) => "Width",
        Effect::Compressor { .. } => "Compressor",
    }
}
