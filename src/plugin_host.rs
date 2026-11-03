use garrido_clap_abi::*;
use garrido_studio::{Error, Result};
use std::{
    ffi::{CString, c_char, c_void},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
#[path = "plugin_host/linux.rs"]
mod linux;
#[cfg(target_os = "windows")]
#[path = "plugin_host/windows.rs"]
mod windows;
#[cfg(target_os = "windows")]
use windows::{dlclose, dlopen, dlsym};

#[cfg(unix)]
#[cfg_attr(target_os = "macos", link(name = "System"))]
#[cfg_attr(target_os = "linux", link(name = "dl"))]
unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
    fn setrlimit(resource: i32, limit: *const Limit) -> i32;
    fn getrlimit(resource: i32, limit: *mut Limit) -> i32;
}
#[cfg(target_os = "macos")]
#[link(name = "sandbox")]
unsafe extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, error: *mut *mut c_char) -> i32;
    fn sandbox_free_error(error: *mut c_char);
}
#[repr(C)]
#[cfg(unix)]
struct Limit {
    current: u64,
    max: u64,
}
#[cfg(target_os = "macos")]
fn quoted(path: &Path) -> Result<String> {
    let s = path
        .to_str()
        .ok_or(Error::Invalid("Ruta de plugin no UTF-8"))?;
    if s.chars().any(char::is_control) {
        return Err(Error::Invalid("Ruta de plugin no válida"));
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
fn binary(path: &Path) -> Result<PathBuf> {
    let path = std::fs::canonicalize(path)?;
    if path.is_file() {
        return Ok(path);
    }
    if !path.is_dir() || path.extension().is_none_or(|e| e != "clap") {
        return Err(Error::Invalid("Selecciona un bundle .clap"));
    }
    let plist = String::from_utf8(garrido_studio::storage::read_bounded(
        &path.join("Contents/Info.plist"),
        64 * 1024,
    )?)
    .map_err(|_| Error::Invalid("Info.plist no es XML UTF-8"))?;
    let at = plist
        .find("<key>CFBundleExecutable</key>")
        .ok_or(Error::Invalid("El bundle no declara ejecutable"))?;
    let tail = &plist[at..];
    let start = tail
        .find("<string>")
        .ok_or(Error::Invalid("Info.plist no compatible"))?
        + 8;
    let end = tail[start..]
        .find("</string>")
        .ok_or(Error::Invalid("Info.plist incompleto"))?
        + start;
    let name = &tail[start..end];
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || name == ".."
    {
        return Err(Error::Invalid("Ejecutable del bundle no válido"));
    }
    let executable = std::fs::canonicalize(path.join("Contents/MacOS").join(name))?;
    if !executable.starts_with(&path) || !executable.is_file() {
        return Err(Error::Invalid("Ejecutable fuera del bundle"));
    }
    Ok(executable)
}
fn packaged_path(path: &Path) -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let folder = executable
        .parent()
        .ok_or(Error::Invalid("No se encuentra la aplicación"))?;
    #[cfg(target_os = "macos")]
    let root = folder
        .parent()
        .ok_or(Error::Invalid("Paquete incompleto"))?;
    #[cfg(not(target_os = "macos"))]
    let root = folder;
    if !path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err(Error::Unsupported(
            "Selecciona uno de los plugins incluidos en esta aplicación",
        ));
    }
    let canonical_root = std::fs::canonicalize(root)?;
    if !path.starts_with(root) && !path.starts_with(&canonical_root) {
        return Err(Error::Unsupported(
            "Selecciona uno de los plugins incluidos en esta aplicación",
        ));
    }
    let canonical = std::fs::canonicalize(path)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(Error::Invalid("El plugin sale del paquete"));
    }
    Ok(canonical)
}
#[cfg(target_os = "macos")]
fn jail(plugin: &Path, executable: &Path) -> Result<()> {
    let trusted = first_party(plugin, executable)?;
    if !trusted {
        return Err(Error::Unsupported(
            "Solo se alojan los plugins incluidos hasta validar el aislamiento de terceros",
        ));
    }
    let read_rule = if plugin.is_dir() {
        format!("(subpath {})", quoted(plugin)?)
    } else {
        format!("(literal {})", quoted(plugin)?)
    };
    let policy = format!(
        "(version 1)(deny default)(allow file-read* (subpath \"/System\") (subpath \"/usr/lib\") {}(literal {}))(allow sysctl-read)(allow signal (target self))",
        read_rule,
        quoted(executable)?
    );
    let profile = CString::new(policy).map_err(|_| Error::Invalid("Perfil no válido"))?;
    let mut error = ptr::null_mut();
    // SAFETY: límites del proceso hijo antes de cargar cualquier código de plugin.
    unsafe {
        for (resource, max) in [
            (0, 10),
            (2, 384 * 1024 * 1024),
            (5, 384 * 1024 * 1024),
            (8, 64),
        ] {
            let mut existing = Limit { current: 0, max: 0 };
            if getrlimit(resource, &mut existing) != 0 {
                return Err(Error::Io(std::io::Error::last_os_error()));
            }
            let max = max.min(existing.max);
            let limit = Limit { current: max, max };
            if setrlimit(resource, &limit) != 0 {
                if resource == 2 || resource == 5 {
                    continue;
                }
                return Err(Error::Io(std::io::Error::other(format!(
                    "Límite {resource}: {}",
                    std::io::Error::last_os_error()
                ))));
            }
        }
        let status = sandbox_init(profile.as_ptr(), 0, &mut error);
        if !error.is_null() {
            sandbox_free_error(error);
        }
        if status != 0 {
            return Err(Error::Unsupported(
                "No se pudo aislar el worker; plugin rechazado",
            ));
        }
    }
    Ok(())
}
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn jail(plugin: &Path, executable: &Path) -> Result<()> {
    if !first_party(plugin, executable)? {
        return Err(Error::Unsupported(
            "Solo se alojan los plugins incluidos hasta validar el aislamiento de terceros",
        ));
    }
    #[cfg(target_os = "windows")]
    {
        windows::jail()
    }
    #[cfg(target_os = "linux")]
    {
        linux::jail()
    }
}
fn first_party(plugin: &Path, executable: &Path) -> Result<bool> {
    let binary = binary(plugin)?;
    let mut packaged = Vec::new();
    if let Some(folder) = executable.parent() {
        packaged.push(folder.join("Plugins/GarridoNative.clap"));
        if let Some(contents) = folder.parent() {
            packaged.push(contents.join("PlugIns/GarridoNative.clap/Contents/MacOS/GarridoNative"));
        }
    }
    if !packaged.iter().any(|path| {
        !path.is_symlink() && std::fs::canonicalize(path).is_ok_and(|path| path == binary)
    }) {
        return Ok(false);
    }
    let mut manifests = Vec::new();
    if let Some(folder) = executable.parent() {
        manifests.push(folder.join("garrido-native.sha256"));
        manifests.push(folder.join("Resources/garrido-native.sha256"));
        if let Some(contents) = folder.parent() {
            manifests.push(contents.join("Resources/garrido-native.sha256"));
        }
    }
    let expected = manifests.into_iter().find_map(|p| {
        garrido_studio::storage::read_bounded(&p, 256)
            .ok()
            .and_then(|v| String::from_utf8(v).ok())
    });
    let Some(expected) = expected else {
        return Ok(false);
    };
    let expected = expected.trim();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(false);
    }
    let meta = std::fs::metadata(&binary)?;
    if meta.len() > 16 * 1024 * 1024 {
        return Ok(false);
    }
    let bytes = garrido_studio::storage::read_bounded(&binary, 16 * 1024 * 1024)?;
    let digest = garrido_studio::integrity::sha256(&bytes);
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(actual == expected)
}
pub fn included() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let folder = executable.parent().ok_or(Error::Invalid(
        "No se encuentra la carpeta de la aplicación",
    ))?;
    #[cfg(target_os = "macos")]
    let path = folder
        .parent()
        .ok_or(Error::Invalid("Paquete de aplicación incompleto"))?
        .join("PlugIns/GarridoNative.clap");
    #[cfg(not(target_os = "macos"))]
    let path = folder.join("Plugins/GarridoNative.clap");
    if !path.exists() {
        return Err(Error::Unsupported(
            "Abre la aplicación empaquetada para usar sus plugins incluidos",
        ));
    }
    Ok(path)
}

pub fn check_bundled() -> Result<()> {
    use garrido_studio::model::{Instrument, Note, PPQ, Project};
    let path = included()?;
    let listing = scan(&path)?;
    if listing.len() != 14
        || listing
            .iter()
            .filter(|line| line.ends_with("\tinstrument"))
            .count()
            != 6
    {
        return Err(Error::Invalid(
            "El paquete incluido no contiene sus catorce plugins",
        ));
    }
    let rate = 8000;
    let input: Vec<_> = (0..16000)
        .map(|i| {
            let value = (i as f32 * std::f32::consts::TAU * 220. / rate as f32).sin() * 0.1;
            [value, -value]
        })
        .collect();
    let mut project = Project::empty();
    project.rate = rate;
    project.bars = 1;
    project.add_track(Instrument::Trio)?;
    project.patterns[0].notes = vec![Note::new(0, PPQ, 60, 0.65)];
    let cancel = AtomicBool::new(false);
    let mut gain_error = 0f32;
    for index in 0..14 {
        let output = if index < 8 {
            render(&path, index, &input, rate, &cancel)?
        } else {
            instrument(&path, index, &project, 0, &cancel)?
        };
        if output.is_empty() || !output.iter().flatten().any(|sample| sample.abs() > 0.001) {
            return Err(Error::Invalid(
                "Un plugin incluido devolvió silencio inesperado",
            ));
        }
        if index == 0 {
            gain_error = output
                .iter()
                .zip(&input)
                .map(|(a, b)| (a[0] - b[0] * 0.6).abs())
                .fold(0., f32::max);
            if gain_error > 0.000001 {
                return Err(Error::Invalid(
                    "El plugin Gain no coincide con su referencia",
                ));
            }
        }
    }
    println!(
        "{{\"descriptors\":14,\"effects\":8,\"instruments\":6,\"gain_error\":{gain_error},\"audio_played\":false,\"microphone_opened\":false}}"
    );
    Ok(())
}
unsafe extern "C" fn host_extension(_host: *const Host, _id: *const c_char) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn host_request(_host: *const Host) {}
unsafe extern "C" fn event_count(list: *const InputEvents) -> u32 {
    // SAFETY: ctx apunta a la lista local durante cada llamada process.
    unsafe { (*(*list).ctx.cast::<Vec<NoteEvent>>()).len() as u32 }
}
unsafe extern "C" fn event_at(list: *const InputEvents, index: u32) -> *const EventHeader {
    // SAFETY: lista estable durante process; no se entrega un índice fuera de rango.
    unsafe {
        (&*(*list).ctx.cast::<Vec<NoteEvent>>())
            .get(index as usize)
            .map_or(ptr::null(), |e| &e.header)
    }
}
fn instrument_kind(d: &Descriptor) -> Result<bool> {
    if d.features.is_null() {
        return Ok(false);
    }
    for i in 0..32 {
        // SAFETY: metadatos del plugin solo se inspeccionan en el proceso aislado.
        let feature = unsafe { *d.features.add(i) };
        if feature.is_null() {
            return Ok(false);
        }
        if short(feature)? == "instrument" {
            return Ok(true);
        }
    }
    Err(Error::Limit("Demasiadas categorías CLAP"))
}
unsafe extern "C" fn ignore_event(_list: *const OutputEvents, _event: *const EventHeader) -> bool {
    false
}
struct Module {
    handle: *mut c_void,
    entry: *const Entry,
}
impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: las instancias se destruyen antes del módulo.
        unsafe {
            if let Some(deinit) = (*self.entry).deinit {
                deinit();
            }
            dlclose(self.handle);
        }
    }
}
struct Instance {
    plugin: *const Plugin,
    active: bool,
    started: bool,
}
impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: orden de cierre del ciclo CLAP, siempre dentro del worker.
        unsafe {
            let p = &*self.plugin;
            if self.started
                && let Some(f) = p.stop
            {
                f(self.plugin);
            }
            if self.active
                && let Some(f) = p.deactivate
            {
                f(self.plugin);
            }
            if let Some(f) = p.destroy {
                f(self.plugin);
            }
        }
    }
}
fn short(pointer: *const c_char) -> Result<String> {
    if pointer.is_null() {
        return Err(Error::Invalid("Metadato CLAP nulo"));
    }
    let mut bytes = Vec::new();
    for i in 0..512 {
        // SAFETY: el plugin promete cadenas C; un incumplimiento solo afecta al worker.
        let b = unsafe { *pointer.add(i) } as u8;
        if b == 0 {
            return String::from_utf8(bytes).map_err(|_| Error::Invalid("Metadato no UTF-8"));
        }
        if b.is_ascii_control() {
            return Err(Error::Invalid("Metadato con controles"));
        }
        bytes.push(b);
    }
    Err(Error::Limit("Metadato CLAP demasiado largo"))
}

pub fn worker(path: &Path, index: usize, scan: bool) -> Result<()> {
    if index >= 128 {
        return Err(Error::Limit("Índice CLAP fuera de límites"));
    }
    if std::env::args_os()
        .nth(1)
        .is_none_or(|arg| arg != "--clap-worker" && arg != "--clap-scan-worker")
    {
        return Err(Error::Unsupported(
            "El worker CLAP necesita un proceso separado",
        ));
    }
    let path = packaged_path(path)?;
    let executable = binary(&path)?;
    jail(&path, &std::env::current_exe()?)?;
    let cpath = CString::new(
        executable
            .to_str()
            .ok_or(Error::Invalid("Ruta no válida"))?,
    )
    .map_err(|_| Error::Invalid("Ruta no válida"))?;
    // SAFETY: se carga únicamente en el proceso ya aislado y limitado.
    let module = unsafe {
        let handle = dlopen(cpath.as_ptr(), 2);
        if handle.is_null() {
            return Err(Error::Unsupported(
                "No se pudo cargar CLAP (arquitectura/firma/dependencias)",
            ));
        }
        let entry = dlsym(handle, c"clap_entry".as_ptr()).cast::<Entry>();
        if entry.is_null() || (*entry).version.major != 1 {
            dlclose(handle);
            return Err(Error::Unsupported("Entrada CLAP no compatible"));
        }
        let Some(init) = (*entry).init else {
            dlclose(handle);
            return Err(Error::Invalid("Falta init CLAP"));
        };
        let bundle = CString::new(path.to_str().ok_or(Error::Invalid("Ruta no válida"))?)
            .map_err(|_| Error::Invalid("Ruta no válida"))?;
        if !init(bundle.as_ptr()) {
            dlclose(handle);
            return Err(Error::Unsupported("El plugin rechazó init"));
        }
        Module { handle, entry }
    };
    // SAFETY: funciones de fábrica obtenidas del símbolo CLAP validado.
    let factory = unsafe {
        let get = (*module.entry)
            .factory
            .ok_or(Error::Invalid("Falta factory CLAP"))?;
        get(c"clap.plugin-factory".as_ptr())
            .cast::<Factory>()
            .as_ref()
            .ok_or(Error::Invalid("Fábrica CLAP nula"))?
    };
    // SAFETY: fábrica válida dentro de la vida del módulo.
    let count = unsafe { factory.count.ok_or(Error::Invalid("Falta count"))?(factory) };
    if count > 128 {
        return Err(Error::Limit("Máximo 128 plugins por bundle"));
    }
    let descriptor = factory
        .descriptor
        .ok_or(Error::Invalid("Falta descriptor"))?;
    if scan {
        let mut out = std::io::stdout().lock();
        for n in 0..count {
            // SAFETY: índice dentro de la fábrica; todos los punteros quedan en el worker.
            let d = unsafe {
                descriptor(factory, n)
                    .as_ref()
                    .ok_or(Error::Invalid("Descriptor nulo"))?
            };
            writeln!(
                out,
                "{}\t{}\t{}\t{}",
                n,
                short(d.id)?,
                short(d.name)?,
                if instrument_kind(d)? {
                    "instrument"
                } else {
                    "effect"
                }
            )?;
        }
        return Ok(());
    }
    if index >= count as usize {
        return Err(Error::Invalid("Índice CLAP no válido"));
    }
    let host = Host {
        version: VERSION,
        data: ptr::null_mut(),
        name: c"Garrido Studio worker".as_ptr(),
        vendor: c"Pelayo Garrido Martínez".as_ptr(),
        url: c"".as_ptr(),
        host_version: c"1.0.0".as_ptr(),
        extension: Some(host_extension),
        restart: Some(host_request),
        request_process: Some(host_request),
        callback: Some(host_request),
    };
    // SAFETY: descriptor y host permanecen vivos hasta destroy.
    let mut instance = unsafe {
        let d = descriptor(factory, index as u32)
            .as_ref()
            .ok_or(Error::Invalid("Descriptor nulo"))?;
        let create = factory.create.ok_or(Error::Invalid("Falta create"))?;
        let plugin = create(factory, &host, d.id);
        if plugin.is_null() {
            return Err(Error::Unsupported("No se pudo crear la instancia"));
        }
        Instance {
            plugin,
            active: false,
            started: false,
        }
    };
    let mut input = std::io::stdin().lock();
    let mut header = [0u8; 16];
    input.read_exact(&mut header)?;
    let instrument = &header[..8] == b"GCLAP002";
    if &header[..8] != b"GCLAP001" && !instrument {
        return Err(Error::Invalid("Protocolo de worker incorrecto"));
    }
    let rate = u32::from_le_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| Error::Invalid("Rate"))?,
    );
    let frames = u32::from_le_bytes(
        header[12..16]
            .try_into()
            .map_err(|_| Error::Invalid("Frames"))?,
    ) as usize;
    if !(8_000..=192_000).contains(&rate) || frames > 12_000_000 {
        return Err(Error::Limit("Petición de plugin demasiado grande"));
    }
    let mut raw = vec![0u8; frames * 8];
    input.read_exact(&mut raw)?;
    let audio: Vec<[f32; 2]> = raw
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| {
            [
                f32::from_le_bytes(b[..4].try_into().unwrap_or_default()),
                f32::from_le_bytes(b[4..].try_into().unwrap_or_default()),
            ]
        })
        .collect();
    drop(raw);
    if audio
        .iter()
        .flatten()
        .any(|s| !s.is_finite() || s.abs() > 16.)
    {
        return Err(Error::Invalid("Audio de entrada no válido"));
    }
    let mut scheduled = Vec::new();
    if instrument {
        let mut raw_count = [0u8; 4];
        input.read_exact(&mut raw_count)?;
        let count = u32::from_le_bytes(raw_count) as usize;
        if count > 32768 {
            return Err(Error::Limit("Demasiadas notas CLAP"));
        }
        for id in 0..count {
            let mut note = [0u8; 16];
            input.read_exact(&mut note)?;
            let at = u32::from_le_bytes(note[..4].try_into().unwrap_or_default());
            let end = u32::from_le_bytes(note[4..8].try_into().unwrap_or_default());
            let key = u32::from_le_bytes(note[8..12].try_into().unwrap_or_default());
            let velocity = f32::from_le_bytes(note[12..].try_into().unwrap_or_default());
            if at >= end
                || end as usize >= frames
                || key > 127
                || !velocity.is_finite()
                || !(0. ..=1.).contains(&velocity)
            {
                return Err(Error::Invalid("Nota CLAP fuera de rango"));
            }
            for (time, kind) in [(at, 0), (end, 1)] {
                scheduled.push(NoteEvent {
                    header: EventHeader {
                        size: std::mem::size_of::<NoteEvent>() as u32,
                        time,
                        space: 0,
                        kind,
                        flags: 0,
                    },
                    note_id: id as i32,
                    port: 0,
                    channel: 0,
                    key: key as i16,
                    velocity: velocity as f64,
                });
            }
        }
        scheduled.sort_by_key(|e| (e.header.time, std::cmp::Reverse(e.header.kind)));
    }
    header[..8].copy_from_slice(b"GCLAP001");
    // SAFETY: ciclo de vida y buffers preasignados; los plugins solo ven memoria del worker.
    unsafe {
        let p = &*instance.plugin;
        if !p.init.ok_or(Error::Invalid("Falta init"))?(instance.plugin) {
            return Err(Error::Unsupported("Plugin init falló"));
        }
        let ext = p.extension.ok_or(Error::Invalid("Falta get_extension"))?;
        let ports = ext(instance.plugin, c"clap.audio-ports".as_ptr())
            .cast::<AudioPorts>()
            .as_ref()
            .ok_or(Error::Unsupported("El plugin no ofrece puertos"))?;
        let count = ports.count.ok_or(Error::Invalid("Puertos sin count"))?;
        let get = ports.get.ok_or(Error::Invalid("Puertos sin get"))?;
        if count(instance.plugin, true) != u32::from(!instrument)
            || count(instance.plugin, false) != 1
        {
            return Err(Error::Unsupported(
                "Se requiere un efecto estéreo o instrumento sin entrada de audio",
            ));
        }
        for is_input in [true, false] {
            if instrument && is_input {
                continue;
            }
            let mut info = std::mem::MaybeUninit::<PortInfo>::zeroed();
            if !get(instance.plugin, 0, is_input, info.as_mut_ptr())
                || info.assume_init().channels != 2
            {
                return Err(Error::Unsupported("El plugin debe tener puertos estéreo"));
            }
        }
        if instrument {
            let notes = ext(instance.plugin, c"clap.note-ports".as_ptr())
                .cast::<NotePorts>()
                .as_ref()
                .ok_or(Error::Unsupported("Faltan puertos de notas CLAP"))?;
            let count = notes.count.ok_or(Error::Invalid("Falta count de notas"))?;
            let get = notes.get.ok_or(Error::Invalid("Falta get de notas"))?;
            let mut info = std::mem::MaybeUninit::<NotePortInfo>::zeroed();
            if count(instance.plugin, true) < 1
                || !get(instance.plugin, 0, true, info.as_mut_ptr())
                || info.assume_init().supported & 1 == 0
            {
                return Err(Error::Unsupported(
                    "El instrumento no admite eventos de notas CLAP",
                ));
            }
        }
        if !p.activate.ok_or(Error::Invalid("Falta activate"))?(
            instance.plugin,
            rate as f64,
            1,
            512,
        ) {
            return Err(Error::Unsupported("Plugin activate falló"));
        }
        instance.active = true;
        if !p.start.ok_or(Error::Invalid("Falta start"))?(instance.plugin) {
            return Err(Error::Unsupported("Plugin start falló"));
        }
        instance.started = true;
        let mut block_events = Vec::<NoteEvent>::with_capacity(scheduled.len());
        let events = InputEvents {
            ctx: (&mut block_events as *mut Vec<NoteEvent>).cast(),
            size: Some(event_count),
            get: Some(event_at),
        };
        let mut next_event = 0;
        let output_events = OutputEvents {
            ctx: ptr::null_mut(),
            push: Some(ignore_event),
        };
        let mut left = [0f32; 512];
        let mut right = [0f32; 512];
        let mut out_l = [0f32; 512];
        let mut out_r = [0f32; 512];
        let mut input_pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut output_pointers = [out_l.as_mut_ptr(), out_r.as_mut_ptr()];
        let in_buffer = AudioBuffer {
            data32: input_pointers.as_mut_ptr(),
            data64: ptr::null_mut(),
            channels: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut out_buffer = AudioBuffer {
            data32: output_pointers.as_mut_ptr(),
            data64: ptr::null_mut(),
            channels: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut output = std::io::stdout().lock();
        output.write_all(&header)?;
        let process = p.process.ok_or(Error::Invalid("Falta process"))?;
        let mut encoded = [0u8; 4096];
        for (at, block) in audio.chunks(512).enumerate() {
            for (i, f) in block.iter().enumerate() {
                left[i] = f[0];
                right[i] = f[1];
            }
            out_l.fill(0.);
            out_r.fill(0.);
            block_events.clear();
            while next_event < scheduled.len()
                && scheduled[next_event].header.time < (at * 512 + block.len()) as u32
            {
                let mut e = scheduled[next_event];
                e.header.time -= (at * 512) as u32;
                block_events.push(e);
                next_event += 1;
            }
            let data = Process {
                time: (at * 512) as i64,
                frames: block.len() as u32,
                transport: ptr::null(),
                inputs: if instrument { ptr::null() } else { &in_buffer },
                outputs: &mut out_buffer,
                inputs_count: u32::from(!instrument),
                outputs_count: 1,
                input_events: &events,
                output_events: &output_events,
            };
            if process(instance.plugin, &data) == 0 {
                return Err(Error::Unsupported("El plugin rechazó el audio"));
            }
            for i in 0..block.len() {
                if !out_l[i].is_finite() || !out_r[i].is_finite() {
                    return Err(Error::Invalid("El plugin produjo NaN/Inf"));
                }
                encoded[i * 8..i * 8 + 4].copy_from_slice(&out_l[i].clamp(-0.8, 0.8).to_le_bytes());
                encoded[i * 8 + 4..i * 8 + 8]
                    .copy_from_slice(&out_r[i].clamp(-0.8, 0.8).to_le_bytes());
            }
            output.write_all(&encoded[..block.len() * 8])?;
        }
    }
    Ok(())
}

struct ChildGuard(std::process::Child);
impl std::ops::Deref for ChildGuard {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn execute(
    path: &Path,
    index: usize,
    request: Vec<u8>,
    scan: bool,
    cancel: &AtomicBool,
) -> Result<Vec<u8>> {
    let path = packaged_path(path)?;
    binary(&path)?;
    let executable = std::env::current_exe()?;
    let mut command = Command::new(&executable);
    command.env_clear();
    if let Some(folder) = executable.parent() {
        command.current_dir(folder);
    }
    #[cfg(target_os = "windows")]
    command.env(
        "SystemRoot",
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    );
    let mut child = ChildGuard(
        command
            .arg(if scan {
                "--clap-scan-worker"
            } else {
                "--clap-worker"
            })
            .arg(&path)
            .arg(index.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let mut input = child
        .stdin
        .take()
        .ok_or(Error::Invalid("Sin stdin del worker"))?;
    let output = child
        .stdout
        .take()
        .ok_or(Error::Invalid("Sin stdout del worker"))?;
    let limit = if scan { 1024 * 1024 } else { request.len() + 1 };
    let writer = thread::Builder::new()
        .name("garrido-clap-in".into())
        .spawn(move || input.write_all(&request))?;
    let reader = thread::Builder::new()
        .name("garrido-clap-out".into())
        .spawn(move || {
            let mut data = Vec::new();
            output
                .take(limit as u64 + 1)
                .read_to_end(&mut data)
                .map(|_| data)
        })?;
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed)
            || start.elapsed() > Duration::from_secs(if scan { 4 } else { 12 })
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = writer.join();
            let _ = reader.join();
            return Err(Error::Limit("Worker cancelado o agotado; plugin detenido"));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let sent = writer
        .join()
        .map_err(|_| Error::Invalid("Falló escritura al worker"))?;
    let result = reader
        .join()
        .map_err(|_| Error::Invalid("Falló lectura del worker"))??;
    if !status.success() {
        return Err(Error::Unsupported(
            "El plugin falló o fue rechazado por el aislamiento",
        ));
    }
    sent?;
    if result.len() > limit {
        return Err(Error::Limit("Respuesta del plugin demasiado grande"));
    }
    Ok(result)
}
pub fn scan(path: &Path) -> Result<Vec<String>> {
    let data = execute(path, 0, Vec::new(), true, &AtomicBool::new(false))?;
    let text =
        String::from_utf8(data).map_err(|_| Error::Invalid("Respuesta de scanner no válida"))?;
    let lines: Vec<_> = text.lines().map(str::to_owned).collect();
    if lines.len() > 128 {
        return Err(Error::Limit("Demasiados plugins"));
    }
    Ok(lines)
}
pub fn render(
    path: &Path,
    index: usize,
    audio: &[[f32; 2]],
    rate: u32,
    cancel: &AtomicBool,
) -> Result<Vec<[f32; 2]>> {
    if audio.len() > 12_000_000 {
        return Err(Error::Limit("Audio demasiado grande"));
    }
    let mut request = Vec::with_capacity(audio.len() * 8 + 16);
    request.extend(b"GCLAP001");
    request.extend(rate.to_le_bytes());
    request.extend((audio.len() as u32).to_le_bytes());
    for frame in audio {
        for sample in frame {
            if !sample.is_finite() {
                return Err(Error::Invalid("Audio no finito"));
            }
            request.extend(sample.to_le_bytes());
        }
    }
    response(
        execute(path, index, request, false, cancel)?,
        audio.len(),
        rate,
    )
}
fn response(bytes: Vec<u8>, count: usize, rate: u32) -> Result<Vec<[f32; 2]>> {
    if bytes.len() != count * 8 + 16
        || &bytes[..8] != b"GCLAP001"
        || bytes[8..12] != rate.to_le_bytes()
        || bytes[12..16] != (count as u32).to_le_bytes()
    {
        return Err(Error::Invalid("Respuesta de audio no válida"));
    }
    let frames: Vec<_> = bytes[16..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| {
            [
                f32::from_le_bytes(b[..4].try_into().unwrap_or_default()),
                f32::from_le_bytes(b[4..].try_into().unwrap_or_default()),
            ]
        })
        .collect();
    if frames
        .iter()
        .flatten()
        .any(|f| !f.is_finite() || f.abs() > 0.801)
    {
        return Err(Error::Invalid("Salida CLAP fuera de rango"));
    }
    Ok(frames)
}

pub fn instrument(
    path: &Path,
    index: usize,
    project: &garrido_studio::model::Project,
    pattern: usize,
    cancel: &AtomicBool,
) -> Result<Vec<[f32; 2]>> {
    project.validate()?;
    let pattern = project
        .patterns
        .get(pattern)
        .ok_or(Error::Invalid("Patrón no disponible"))?;
    let duration = project.seconds_at(pattern.length as f64) + 1.;
    let frames = (duration * project.rate as f64).ceil() as usize;
    if duration > 120. || frames > 12_000_000 {
        return Err(Error::Limit("Patrón CLAP demasiado largo"));
    }
    let mut request = vec![0u8; 16 + frames * 8];
    request[..8].copy_from_slice(b"GCLAP002");
    request[8..12].copy_from_slice(&project.rate.to_le_bytes());
    request[12..16].copy_from_slice(&(frames as u32).to_le_bytes());
    request.extend((pattern.notes.len() as u32).to_le_bytes());
    for note in &pattern.notes {
        let at = (project.seconds_at(note.tick as f64) * project.rate as f64).round() as u32;
        let end = (project.seconds_at((note.tick + note.length) as f64) * project.rate as f64)
            .round() as u32;
        if at >= end {
            return Err(Error::Invalid(
                "Nota demasiado breve para la frecuencia de muestreo",
            ));
        }
        request.extend(at.to_le_bytes());
        request.extend(end.to_le_bytes());
        request.extend((note.key as u32).to_le_bytes());
        request.extend(note.velocity.to_le_bytes());
    }
    response(
        execute(path, index, request, false, cancel)?,
        frames,
        project.rate,
    )
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn package_boundary_accepts_the_executable_path_and_its_canonical_form() {
        let executable = std::env::current_exe().unwrap();
        let canonical = std::fs::canonicalize(&executable).unwrap();
        assert_eq!(packaged_path(&executable).unwrap(), canonical);
        assert_eq!(packaged_path(&canonical).unwrap(), canonical);
    }
    #[test]
    fn worker_rejects_an_ordinary_process_before_loading_or_applying_limits() {
        assert!(matches!(
            worker(Path::new("missing.clap"), 0, true),
            Err(Error::Unsupported(_))
        ));
    }
    #[test]
    fn package_boundary_rejects_relative_and_parent_paths_before_file_access() {
        assert!(matches!(
            packaged_path(Path::new("relative.clap")),
            Err(Error::Unsupported(_))
        ));
        let exe = std::env::current_exe().unwrap();
        let escaped = exe.parent().unwrap().join("../outside.clap");
        assert!(matches!(
            packaged_path(&escaped),
            Err(Error::Unsupported(_))
        ));
    }
}
