use crate::audio_macos::Playback;
use garrido_studio::{
    Error, Result, codec,
    edit::{self, Edit},
    engine::{Engine, Render},
    history::{self, History},
    midi,
    model::*,
    storage, stretch, wav,
};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, CString, c_char, c_void},
    path::PathBuf,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

type Id = *mut c_void;
type Sel = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        origin: Point { x, y },
        size: Size {
            width: w,
            height: h,
        },
    }
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
    fn class_addMethod(
        class: Id,
        name: Sel,
        implementation: *const c_void,
        encoding: *const c_char,
    ) -> i8;
    fn objc_registerClassPair(class: Id);
}
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSRectFill(rect: Rect);
    static NSFontAttributeName: Id;
    static NSForegroundColorAttributeName: Id;
}
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    fn NSHomeDirectory() -> Id;
}

fn class(name: &CStr) -> Id {
    // SAFETY: el nombre termina en cero y vive durante la llamada.
    unsafe { objc_getClass(name.as_ptr()) }
}
fn selector(name: *const c_char) -> Sel {
    // SAFETY: solo recibe nombres estáticos de los mensajes del adaptador.
    unsafe { sel_registerName(name) }
}

trait Arguments<R> {
    unsafe fn call(self, object: Id, selector: Sel) -> R;
}
macro_rules! arguments {
    ($($name:ident:$index:tt),*)=>{
        impl<R $(,$name)*> Arguments<R> for ($($name,)*) {
            unsafe fn call(self,object:Id,selector:Sel)->R {
                // SAFETY: la tupla fija la firma de objc_msgSend en arm64.
                let method:unsafe extern "C" fn(Id,Sel,$($name),*)->R=unsafe{std::mem::transmute(objc_msgSend as *const ())};
                // SAFETY: receptor, selector y tipos se fijan en cada mensaje.
                unsafe{method(object,selector,$(self.$index),*)}
            }
        }
    };
}
arguments!();
arguments!(A:0);
arguments!(A:0,B:1);
arguments!(A:0,B:1,C:2);
arguments!(A:0,B:1,C:2,D:3);
arguments!(A:0,B:1,C:2,D:3,E:4);
arguments!(A:0,B:1,C:2,D:3,E:4,F:5);

macro_rules! send {
    ($object:expr,$selector:literal,$ret:ty $(; $value:expr,$ty:ty)*) => {{
        let receiver=$object;
        let values:($($ty,)*)=($($value,)*);
        let selector=selector(concat!($selector,"\0").as_ptr().cast());
        // SAFETY: los argumentos se evalúan fuera del bloque FFI.
        let result:$ret=unsafe {values.call(receiver,selector)};
        result
    }};
}

fn string(value: &str) -> Id {
    let cleaned = value.replace('\0', "");
    let bytes = CString::new(cleaned).unwrap_or_default();
    send!(class(c"NSString"),"stringWithUTF8String:",Id;bytes.as_ptr(),*const c_char)
}

fn rust_string(object: Id) -> Option<String> {
    if object.is_null() {
        return None;
    }
    let bytes = send!(object, "UTF8String", *const c_char);
    if bytes.is_null() {
        return None;
    }
    // SAFETY: NSString conserva este UTF-8 durante la conversión.
    Some(
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn color(rgb: [f64; 3]) -> Id {
    send!(class(c"NSColor"),"colorWithSRGBRed:green:blue:alpha:",Id;rgb[0],f64;rgb[1],f64;rgb[2],f64;1.,f64)
}
fn fill(area: Rect, rgb: [f64; 3]) {
    send!(color(rgb), "setFill", ());
    // SAFETY: rectángulo por valor dentro del contexto de dibujo.
    unsafe {
        NSRectFill(area);
    }
}
fn label(value: &str, x: f64, y: f64, size: f64, rgb: [f64; 3], bold: bool) {
    let attributes = send!(class(c"NSMutableDictionary"), "dictionary", Id);
    let font = theme::font(size, bold);
    // SAFETY: constante exportada por AppKit durante toda la aplicación.
    send!(attributes,"setObject:forKey:",();font,Id;unsafe {NSFontAttributeName},Id);
    // SAFETY: constante exportada por AppKit durante toda la aplicación.
    send!(attributes,"setObject:forKey:",();color(rgb),Id;unsafe {NSForegroundColorAttributeName},Id);
    send!(string(value),"drawAtPoint:withAttributes:",();Point{x,y:y-1.5},Point;attributes,Id);
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Steps,
    Piano,
    Arrange,
    Mixer,
    Audio,
    Automation,
}
enum Goal {
    Play,
    Export(PathBuf, garrido_studio::export::Settings),
    Plugin(usize),
    Instrument(usize),
}
struct Job {
    rx: mpsc::Receiver<Result<Render>>,
    cancel: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    goal: Goal,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
struct BackupJob {
    rx: mpsc::Receiver<Result<PathBuf>>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Drop for BackupJob {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
struct State {
    history: History,
    selected: usize,
    page: Page,
    status: String,
    dirty: bool,
    saved: Option<Project>,
    job: Option<Job>,
    playback: Option<Playback>,
    view: Id,
    peak: f32,
    rendered: bool,
    rms: f32,
    voices: usize,
    width: f64,
    height: f64,
    selection: (f64, f64),
    anchor: Option<f64>,
    workspace: workspace::Workspace,
    capture: Option<crate::capture_macos::Capture>,
    pending_take: Option<Sample>,
    plugin_path: Option<PathBuf>,
    plugin_instruments: Vec<bool>,
    selected_pattern: Option<usize>,
    scale: f64,
    song_mode: bool,
    metronome: bool,
    window_restore: Option<Rect>,
    recovery: Option<garrido_studio::recovery::Store>,
    recovery_paths: Vec<PathBuf>,
    backup: Option<BackupJob>,
    backup_at: Instant,
    backup_paused: bool,
    revision: u64,
    backup_revision: u64,
    audio_clipboard: Option<Sample>,
}
thread_local! {static STATE:RefCell<Option<State>>=const {RefCell::new(None)};static ACTION:Cell<Option<isize>>=const{Cell::new(None)};}

fn with_state(action: impl FnOnce(&mut State)) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut()
                && let Some(state) = state.as_mut()
            {
                action(state);
            }
        })
    }));
}

impl State {
    fn redraw(&self) {
        let window = send!(self.view, "window", Id);
        send!(window,"setDocumentEdited:",();self.dirty as i8,i8);
        send!(self.view,"setNeedsDisplay:",();1i8,i8);
    }
    fn stop(&mut self) {
        self.job = None;
        self.playback = None;
    }
    fn changed(&mut self) {
        self.workspace.selected_notes.clear();
        self.revision = self.revision.wrapping_add(1);
        self.stop();
        self.dirty = self.saved.as_ref() != Some(&self.history.project);
        self.status = "Cambios listos. Pulsa Reproducir.".into();
        self.redraw();
    }
    fn result(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.status = error.to_string();
        }
        self.redraw();
    }
    fn begin(&mut self, goal: Goal) {
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
        let metronome = matches!(goal, Goal::Play) && self.metronome;
        match thread::Builder::new()
            .name("garrido-render".into())
            .spawn(move || {
                let result = Engine::new(&p, hq, tail)
                    .and_then(|e| {
                        if metronome { e.with_metronome() } else { e }.render(&worker_cancel)
                    })
                    .and_then(|mut render| {
                        if let Some((path, settings)) = export {
                            let data = if path.extension().is_some_and(|s| s == "flac") {
                                let depth = match settings.depth {
                                    wav::Depth::Pcm16 => 16,
                                    wav::Depth::Pcm24 => 24,
                                    _ => return Err(Error::Unsupported("FLAC usa PCM16 o PCM24")),
                                };
                                garrido_studio::flac::encode_cancel(
                                    &render.audio,
                                    settings.rate,
                                    depth,
                                    settings.channels,
                                    &worker_cancel,
                                )?
                            } else {
                                garrido_studio::export::encode(&render.audio, settings)?
                            };
                            if worker_cancel.load(Ordering::Relaxed) {
                                return Err(Error::Invalid("Exportación cancelada"));
                            }
                            storage::write_new(&path, &data)?;
                            render.audio.clear();
                        }
                        Ok(render)
                    });
                let _ = tx.send(result);
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
    fn start_backup(&mut self) {
        if self.backup.is_some() || self.backup_paused || !self.dirty {
            return;
        }
        let Some(store) = self.recovery.clone() else {
            return;
        };
        // SAFETY: Foundation devuelve la carpeta del usuario como NSString autoretenida.
        let home = rust_string(unsafe { NSHomeDirectory() }).unwrap_or_default();
        let attributes = send!(class(c"NSFileManager"), "defaultManager", Id);
        let values = send!(attributes,"attributesOfFileSystemForPath:error:",Id;string(&home),Id;ptr::null_mut(),*mut Id);
        let free = send!(
            send!(values,"objectForKey:",Id;string("NSFileSystemFreeSize"),Id),
            "unsignedLongLongValue",
            u64
        );
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
    fn poll_backup(&mut self) {
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
    fn poll(&mut self) {
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
                    Ok(render) => {
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
                                    pattern.notes = vec![Note::new(0, ticks, 60, 1.)];
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
                            Goal::Play => match Playback::start(render.audio, render.rate) {
                                Ok(playback) => {
                                    self.playback = Some(playback);
                                    self.status =
                                        "Reproduciendo en bucle · detener antes de editar".into();
                                }
                                Err(e) => self.status = e.to_string(),
                            },
                            Goal::Export(path, _) => {
                                self.status = if path.extension().is_some_and(|s| s == "flac") {
                                    "FLAC exportado"
                                } else {
                                    "WAV exportado"
                                }
                                .into();
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
    fn select_pattern(&mut self, index: usize) {
        if index >= self.history.project.patterns.len() {
            return;
        }
        self.selected_pattern = Some(index);
        self.workspace.reset_selection();
        if let Some(clip) = self
            .history
            .project
            .clips
            .iter()
            .find(|c| c.pattern == index)
        {
            self.selected = clip.track;
        }
        self.redraw();
    }
    fn pattern(&self) -> Option<usize> {
        if let Some(index) = self
            .selected_pattern
            .filter(|i| *i < self.history.project.patterns.len())
        {
            return Some(index);
        }
        self.history
            .project
            .clips
            .iter()
            .find(|c| c.track == self.selected)
            .map(|c| c.pattern)
            .or_else(|| {
                (self.selected < self.history.project.patterns.len()).then_some(self.selected)
            })
    }
    fn apply_clap(&mut self, index: usize) {
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
        let project = self.history.project.clone();
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
                let _ = tx.send(result);
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
    fn finish_capture(&mut self) {
        if let Some(capture) = self.capture.take() {
            match capture.finish() {
                Ok((sample, interrupted)) => {
                    self.pending_take = Some(sample.clone());
                    let result = self.history.edit(|p| {
                        let length = (p
                            .tick_at(sample.frames.len() as f64 / sample.rate as f64)
                            .ceil() as u32)
                            .max(1);
                        let i = p.add_track(Instrument::Sampler)?;
                        p.tracks[i].name = "Recorded take".into();
                        p.tracks[i].sample = Some(sample);
                        let pattern = p.patterns.len() - 1;
                        p.patterns[pattern].length = length.div_ceil(BAR) * BAR;
                        p.patterns[pattern].notes = vec![Note::new(0, length, 60, 1.)];
                        p.bars = p.bars.max(length.div_ceil(BAR) as u16);
                        p.clips
                            .last_mut()
                            .ok_or(Error::Invalid("Clip no disponible"))?
                            .length = length;
                        Ok(())
                    });
                    if result.is_ok() {
                        self.pending_take = None;
                        self.selected = self.history.project.tracks.len() - 1;
                        self.changed();
                        self.selection = (0., 1.);
                        self.anchor = None;
                        self.page = Page::Audio;
                        self.workspace.show(Page::Audio);
                        self.sync_tools();
                        self.status = if interrupted {"Entrada interrumpida. Se ha conservado el audio recibido; guarda una copia."} else {"Toma añadida. Guarda una copia para conservarla."}.into();
                    } else {
                        self.status="La toma no cabe en esta sesión. FILE > Export pending take la conserva.".into();
                    }
                }
                Err(e) => self.status = e.to_string(),
            }
        }
        self.redraw();
    }
    fn sync_tools(&self) {
        let specs = theme::controls();
        let views = send!(self.view, "subviews", Id);
        let count = send!(views, "count", usize);
        for i in 0..count {
            let v = send!(views,"objectAtIndex:",Id;i,usize);
            let tag = send!(v, "tag", isize);
            let Some(spec) = specs.iter().find(|s| s.tag == tag) else {
                continue;
            };
            let mut r = spec.r;
            if (50..=56).contains(&tag) {
                let panel = self.workspace.audio_rect();
                r = rect(
                    panel.origin.x + 12. + (tag - 50) as f64 * 88.,
                    panel.origin.y + 27.,
                    84.,
                    26.,
                );
            }
            let actual = rect(
                r.origin.x * self.scale,
                r.origin.y * self.scale,
                r.size.width * self.scale,
                r.size.height * self.scale,
            );
            let old = send!(v, "frame", Rect);
            if old.origin.x != actual.origin.x
                || old.origin.y != actual.origin.y
                || old.size.width != actual.size.width
                || old.size.height != actual.size.height
            {
                send!(v,"setFrame:",();actual,Rect);
            }
            if (50..=56).contains(&tag) {
                send!(v,"setHidden:",();i8::from(self.page!=Page::Audio),i8);
            }
            let active = match tag {
                100 | 9301 => Some(self.workspace.visible(0)),
                101 | 9302 => Some(self.workspace.visible(1)),
                102 => Some(self.page == Page::Arrange),
                103 | 9303 => Some(self.workspace.visible(2)),
                104 => Some(self.workspace.visible(3)),
                105 => Some(self.workspace.visible(4)),
                9003 => Some(!self.song_mode),
                9004 => Some(self.song_mode),
                9005 => Some(self.metronome),
                _ => None,
            };
            if let Some(active) = active {
                send!(v,"setState:",();isize::from(active),isize);
            }
        }
    }
}

fn effect_name(fx: &Effect) -> &'static str {
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

mod actions;
mod files;
mod icon_codes;
mod menus;
mod theme;
mod workspace;

unsafe extern "C" fn draw(_object: Id, _selector: Sel, _rect: Rect) {
    with_state(|s| s.draw());
}
unsafe extern "C" fn mouse(_object: Id, _selector: Sel, event: Id) {
    with_state(|s| s.mouse(event, false));
}
unsafe extern "C" fn dragged(_object: Id, _selector: Sel, event: Id) {
    with_state(|s| s.drag(event));
}
unsafe extern "C" fn released(_object: Id, _selector: Sel, _event: Id) {
    with_state(|s| s.release());
}
unsafe extern "C" fn right_mouse(_object: Id, _selector: Sel, event: Id) {
    with_state(|s| s.mouse(event, true));
}
unsafe extern "C" fn clicked(object: Id, _selector: Sel, sender: Id) {
    let window = send!(object, "window", Id);
    send!(window,"makeFirstResponder:",i8;object,Id);
    let tag = send!(sender, "tag", isize);
    ACTION.with(|action| action.set(Some(tag)));
}
unsafe extern "C" fn tick(_object: Id, _selector: Sel, _timer: Id) {
    let action = ACTION.with(|a| a.take());
    if let Some(tag @ 9000..=9002) = action {
        let window = send!(_object, "window", Id);
        match tag {
            9000 => {
                send!(window,"miniaturize:",();ptr::null_mut(),Id);
            }
            9001 => {
                let mut target = None;
                with_state(|s| {
                    target = s.window_restore.take().or_else(|| {
                        let screen = send!(window, "screen", Id);
                        if screen.is_null() {
                            return None;
                        }
                        s.window_restore = Some(send!(window, "frame", Rect));
                        Some(send!(screen, "visibleFrame", Rect))
                    });
                });
                if let Some(frame) = target {
                    send!(window,"setFrame:display:",();frame,Rect;1i8,i8);
                }
            }
            _ => {
                let app = send!(class(c"NSApplication"), "sharedApplication", Id);
                send!(app,"terminate:",();ptr::null_mut(),Id);
            }
        }
    }
    with_state(|s| {
        if let Some(tag) = action.filter(|t| !(9000..=9002).contains(t)) {
            s.action(tag);
        }
        s.poll();
    });
}
unsafe extern "C" fn yes(_object: Id, _selector: Sel) -> i8 {
    1
}
unsafe extern "C" fn yes_arg(_object: Id, _selector: Sel, _sender: Id) -> i8 {
    1
}
unsafe extern "C" fn close(_object: Id, _selector: Sel, _window: Id) -> i8 {
    let mut result = 0;
    with_state(|s| {
        if s.may_leave() {
            s.stop();
            s.dirty = false;
            result = 1;
        }
    });
    result
}
unsafe extern "C" fn terminate(_object: Id, _selector: Sel, _app: Id) -> usize {
    let mut result = 0;
    with_state(|s| {
        if s.may_leave() {
            s.stop();
            s.dirty = false;
            result = 1;
        }
    });
    result
}
unsafe extern "C" fn scroll(_object: Id, _selector: Sel, event: Id) {
    with_state(|s| s.scroll(event));
}
unsafe extern "C" fn key(_object: Id, _selector: Sel, event: Id) {
    let code = send!(event, "keyCode", u16);
    let flags = send!(event, "modifierFlags", usize);
    let command = flags & (1 << 20) != 0;
    let shift = flags & (1 << 17) != 0;
    let text = rust_string(send!(event, "charactersIgnoringModifiers", Id))
        .unwrap_or_default()
        .to_lowercase();
    with_state(|s| {
        let action = if command {
            match text.as_str() {
                "n" => Some(9010),
                "s" => Some(3),
                "o" => Some(4),
                "z" => Some(if shift { 7 } else { 6 }),
                "a" if s.page == Page::Audio => {
                    s.selection = (0., 1.);
                    s.redraw();
                    None
                }
                "c" if s.page == Page::Audio => Some(530),
                "x" if s.page == Page::Audio => Some(531),
                "v" if s.page == Page::Audio => Some(532),
                "a" if s.page == Page::Piano => Some(440),
                "c" if s.page == Page::Piano => Some(441),
                "x" if s.page == Page::Piano => Some(442),
                "v" if s.page == Page::Piano => Some(443),
                _ => None,
            }
        } else {
            match text.as_str() {
                " " => Some(1),
                "q" => Some(34),
                "r" => Some(37),
                "1" => Some(100),
                "2" => Some(101),
                "3" => Some(102),
                "4" => Some(103),
                _ => match code {
                    126 => Some(35),
                    125 => Some(36),
                    53 => Some(2),
                    51 | 117 if s.page == Page::Piano => Some(444),
                    _ => None,
                },
            }
        };
        if let Some(action) = action {
            s.action(action);
        }
    });
}

fn method(class: Id, name: &CStr, imp: *const c_void, encoding: &CStr) -> Result<()> {
    // SAFETY: selector, función C y codificación son estáticos y coinciden.
    if unsafe { class_addMethod(class, selector(name.as_ptr()), imp, encoding.as_ptr()) } == 0 {
        return Err(Error::Unsupported("No se pudo registrar un control nativo"));
    }
    Ok(())
}

fn button(view: Id, title: &str, tag: isize, x: f64, y: f64, w: f64) {
    let button = send!(class(c"NSButton"),"buttonWithTitle:target:action:",Id;string(title),Id;view,Id;selector(c"clicked:".as_ptr()),Sel);
    send!(button,"setFrame:",();rect(x,y,w,30.),Rect);
    send!(button,"setTag:",();tag,isize);
    send!(button,"setBezelStyle:",();1usize,usize);
    if (100..=105).contains(&tag) || (9003..=9005).contains(&tag) || (9301..=9303).contains(&tag) {
        send!(button,"setButtonType:",();1usize,usize);
    }
    let font = send!(class(c"NSFont"),"systemFontOfSize:",Id;12.,f64);
    send!(button,"setFont:",();font,Id);
    if (30..=40).contains(&tag) || (50..=56).contains(&tag) {
        send!(button,"setAutoresizingMask:",();8usize,usize);
    }
    send!(button,"setToolTip:",();string(title),Id);
    send!(view,"addSubview:",();button,Id);
}

pub fn run() -> Result<()> {
    let pool = send!(send!(class(c"NSAutoreleasePool"), "alloc", Id), "init", Id);
    let app = send!(class(c"NSApplication"), "sharedApplication", Id);
    send!(app,"setActivationPolicy:",i8;0isize,isize);
    theme::fonts()?;
    // SAFETY: clase base de AppKit y nombre propio estático.
    let view_class =
        unsafe { objc_allocateClassPair(class(c"NSView"), c"GarridoStudioView".as_ptr(), 0) };
    // SAFETY: NSObject y nombre propio estático.
    let delegate_class =
        unsafe { objc_allocateClassPair(class(c"NSObject"), c"GarridoStudioDelegate".as_ptr(), 0) };
    // SAFETY: ventana propia para chrome dibujado, sin barra de título nativa.
    let window_class =
        unsafe { objc_allocateClassPair(class(c"NSWindow"), c"GarridoStudioWindow".as_ptr(), 0) };
    if view_class.is_null() || delegate_class.is_null() || window_class.is_null() {
        return Err(Error::Unsupported("No se pudo crear la ventana nativa"));
    }
    for (name, imp, encoding) in [
        (
            c"drawRect:",
            draw as *const () as *const c_void,
            c"v@:{CGRect={CGPoint=dd}{CGSize=dd}}",
        ),
        (c"mouseDown:", mouse as *const () as *const c_void, c"v@:@"),
        (
            c"rightMouseDown:",
            right_mouse as *const () as *const c_void,
            c"v@:@",
        ),
        (c"keyDown:", key as *const () as *const c_void, c"v@:@"),
        (c"clicked:", clicked as *const () as *const c_void, c"v@:@"),
        (c"tick:", tick as *const () as *const c_void, c"v@:@"),
        (c"isFlipped", yes as *const () as *const c_void, c"c@:"),
        (
            c"acceptsFirstResponder",
            yes as *const () as *const c_void,
            c"c@:",
        ),
    ] {
        method(view_class, name, imp, encoding)?;
    }
    method(
        view_class,
        c"mouseDragged:",
        dragged as *const () as *const c_void,
        c"v@:@",
    )?;
    method(
        view_class,
        c"mouseUp:",
        released as *const () as *const c_void,
        c"v@:@",
    )?;
    method(
        delegate_class,
        c"windowShouldClose:",
        close as *const () as *const c_void,
        c"c@:@",
    )?;
    method(
        delegate_class,
        c"applicationShouldTerminate:",
        terminate as *const () as *const c_void,
        c"Q@:@",
    )?;
    method(
        delegate_class,
        c"applicationShouldTerminateAfterLastWindowClosed:",
        yes_arg as *const () as *const c_void,
        c"c@:@",
    )?;
    method(
        view_class,
        c"scrollWheel:",
        scroll as *const c_void,
        c"v@:@",
    )?;
    method(
        window_class,
        c"canBecomeKeyWindow",
        yes as *const () as *const c_void,
        c"c@:",
    )?;
    method(
        window_class,
        c"canBecomeMainWindow",
        yes as *const () as *const c_void,
        c"c@:",
    )?;
    // SAFETY: ambas clases están creadas y tienen sus métodos registrados.
    unsafe {
        objc_registerClassPair(view_class);
        objc_registerClassPair(delegate_class);
        objc_registerClassPair(window_class);
    }
    let delegate = send!(send!(delegate_class, "alloc", Id), "init", Id);
    send!(app,"setDelegate:",();delegate,Id);
    let screen = send!(class(c"NSScreen"), "mainScreen", Id);
    if screen.is_null() {
        return Err(Error::Unsupported("No hay una pantalla disponible"));
    }
    let visible = send!(screen, "visibleFrame", Rect);
    let width = (visible.size.width - 40.)
        .min((visible.size.height - 40.) * 16. / 9.)
        .clamp(1024., 1280.);
    let height = width * 9. / 16.;
    let view =
        send!(send!(view_class,"alloc",Id),"initWithFrame:",Id;rect(0.,0.,width,height),Rect);
    let window = send!(send!(window_class,"alloc",Id),"initWithContentRect:styleMask:backing:defer:",Id;rect(0.,0.,width,height),Rect;14usize,usize;2usize,usize;0i8,i8);
    if window.is_null() || view.is_null() {
        return Err(Error::Unsupported("No se pudo abrir la ventana"));
    }
    send!(window,"setTitle:",();string("Garrido Studio · prealfa"),Id);
    send!(window,"setContentMinSize:",();Size{width:1024.,height:576.},Size);
    send!(window,"setContentView:",();view,Id);
    send!(window,"setHasShadow:",();1i8,i8);
    send!(view,"setAutoresizingMask:",();18usize,usize);
    send!(window,"setDelegate:",();delegate,Id);
    let appearance =
        send!(class(c"NSAppearance"),"appearanceNamed:",Id;string("NSAppearanceNameDarkAqua"),Id);
    if !appearance.is_null() {
        send!(window,"setAppearance:",();appearance,Id);
    }
    let menu = send!(send!(class(c"NSMenu"), "alloc", Id), "init", Id);
    let item = send!(send!(class(c"NSMenuItem"), "alloc", Id), "init", Id);
    let submenu =
        send!(send!(class(c"NSMenu"),"alloc",Id),"initWithTitle:",Id;string("Garrido Studio"),Id);
    let quit = send!(send!(class(c"NSMenuItem"),"alloc",Id),"initWithTitle:action:keyEquivalent:",Id;string("Salir de Garrido Studio"),Id;selector(c"terminate:".as_ptr()),Sel;string("q"),Id);
    send!(submenu,"addItem:",();quit,Id);
    send!(item,"setSubmenu:",();submenu,Id);
    send!(menu,"addItem:",();item,Id);
    send!(app,"setMainMenu:",();menu,Id);
    workspace::controls(view);
    let project = Project::starter();
    let saved = Some(project.clone());
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            history: History::new(project).expect("demo validada"),
            selected: 0,
            page: Page::Steps,
            status: "Listo · el audio solo empieza al pulsar Reproducir".into(),
            dirty: false,
            saved,
            job: None,
            playback: None,
            view,
            peak: 0.,
            rendered: false,
            rms: 0.,
            voices: 0,
            width: 1280.,
            height: 840.,
            selection: (0., 1.),
            anchor: None,
            workspace: workspace::Workspace::default(),
            capture: None,
            pending_take: None,
            plugin_path: None,
            plugin_instruments: Vec::new(),
            selected_pattern: None,
            scale: width / 1920.,
            song_mode: false,
            metronome: true,
            window_restore: None,
            recovery: {
                // SAFETY: ruta local proporcionada por Foundation.
                rust_string(unsafe { NSHomeDirectory() }).and_then(|home| {
                    garrido_studio::recovery::Store::open(
                        &PathBuf::from(home)
                            .join("Library/Application Support/Garrido Studio/Recovery"),
                    )
                    .ok()
                })
            },
            recovery_paths: Vec::new(),
            backup: None,
            backup_at: Instant::now(),
            backup_paused: false,
            revision: 0,
            backup_revision: 0,
            audio_clipboard: None,
        })
    });
    with_state(|s| s.sync_tools());
    let timer = send!(class(c"NSTimer"),"scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:",Id;1./30.,f64;view,Id;selector(c"tick:".as_ptr()),Sel;ptr::null_mut(),Id;1i8,i8);
    send!(window, "center", ());
    send!(window,"makeKeyAndOrderFront:",();ptr::null_mut(),Id);
    send!(window,"makeFirstResponder:",i8;view,Id);
    send!(app,"activateIgnoringOtherApps:",();1i8,i8);
    send!(app, "run", ());
    send!(timer, "invalidate", ());
    STATE.with(|cell| *cell.borrow_mut() = None);
    send!(pool, "drain", ());
    Ok(())
}
