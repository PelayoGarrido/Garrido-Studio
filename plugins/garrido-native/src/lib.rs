#![deny(unsafe_op_in_unsafe_fn)]
use garrido_clap_abi::*;
use garrido_studio::{
    dsp::Processor,
    engine::InstrumentVoice,
    model::{Effect, Instrument, Track},
};
use std::{
    cell::UnsafeCell,
    ffi::{CStr, c_char, c_void},
    ptr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Features([*const c_char; 3]);
// SAFETY: cadenas estáticas, inmutables y terminadas en cero.
unsafe impl Sync for Features {}
static EFFECT_FEATURES: Features =
    Features([c"audio-effect".as_ptr(), c"stereo".as_ptr(), ptr::null()]);
static INSTRUMENT_FEATURES: Features =
    Features([c"instrument".as_ptr(), c"synthesizer".as_ptr(), ptr::null()]);

macro_rules! descriptor {
    ($id:literal,$name:literal,$features:ident) => {
        Descriptor {
            version: VERSION,
            id: concat!("studio.garrido.", $id, "\0").as_ptr().cast(),
            name: concat!("Garrido ", $name, "\0").as_ptr().cast(),
            vendor: c"Pelayo Garrido Martínez".as_ptr(),
            url: c"https://github.com/PelayoGarrido".as_ptr(),
            manual_url: c"".as_ptr(),
            support_url: c"".as_ptr(),
            plugin_version: c"1.0.0".as_ptr(),
            description: c"Original Rust DSP".as_ptr(),
            features: $features.0.as_ptr(),
        }
    };
}
static DESCRIPTORS: [Descriptor; 14] = [
    descriptor!("gain", "Gain", EFFECT_FEATURES),
    descriptor!("delay", "Delay", EFFECT_FEATURES),
    descriptor!("reverb", "Reverb", EFFECT_FEATURES),
    descriptor!("eq", "EQ", EFFECT_FEATURES),
    descriptor!("chorus", "Chorus", EFFECT_FEATURES),
    descriptor!("drive", "Drive", EFFECT_FEATURES),
    descriptor!("width", "Width", EFFECT_FEATURES),
    descriptor!("compressor", "Compressor", EFFECT_FEATURES),
    descriptor!("trio", "Trio", INSTRUMENT_FEATURES),
    descriptor!("fm", "FM six", INSTRUMENT_FEATURES),
    descriptor!("kick", "Kick", INSTRUMENT_FEATURES),
    descriptor!("snare", "Snare", INSTRUMENT_FEATURES),
    descriptor!("hat", "Hat", INSTRUMENT_FEATURES),
    descriptor!("bass", "Bass", INSTRUMENT_FEATURES),
];
struct Instance {
    plugin: Plugin,
    kind: usize,
    gain: AtomicU64,
    active: AtomicBool,
    rate: AtomicU64,
    voices: UnsafeCell<Vec<InstrumentVoice>>,
    effect: UnsafeCell<Option<Processor>>,
    track: Track,
}

unsafe fn instance<'a>(plugin: *const Plugin) -> Option<&'a Instance> {
    if plugin.is_null() {
        return None;
    }
    // SAFETY: el host devuelve la instancia creada por esta fábrica.
    unsafe { ((*plugin).data as *const Instance).as_ref() }
}
unsafe extern "C" fn entry_init(_path: *const c_char) -> bool {
    true
}
unsafe extern "C" fn entry_deinit() {}
unsafe extern "C" fn factory(id: *const c_char) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    // SAFETY: CLAP exige una cadena C válida para consultar la fábrica.
    if unsafe { CStr::from_ptr(id) } == c"clap.plugin-factory" {
        (&FACTORY as *const Factory).cast()
    } else {
        ptr::null()
    }
}
#[unsafe(no_mangle)]
pub static clap_entry: Entry = Entry {
    version: VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    factory: Some(factory),
};
static FACTORY: Factory = Factory {
    count: Some(count),
    descriptor: Some(describe),
    create: Some(create),
};
unsafe extern "C" fn count(_factory: *const Factory) -> u32 {
    DESCRIPTORS.len() as u32
}
unsafe extern "C" fn describe(_factory: *const Factory, index: u32) -> *const Descriptor {
    DESCRIPTORS
        .get(index as usize)
        .map_or(ptr::null(), |d| d as *const _)
}
unsafe extern "C" fn create(
    _factory: *const Factory,
    host: *const Host,
    id: *const c_char,
) -> *const Plugin {
    if host.is_null() || id.is_null() {
        return ptr::null();
    }
    // SAFETY: host e identificador válidos según la ABI pública.
    let kind = unsafe {
        if (*host).version.major != 1 {
            return ptr::null();
        }
        let id = CStr::from_ptr(id);
        DESCRIPTORS.iter().position(|d| CStr::from_ptr(d.id) == id)
    };
    let Some(kind) = kind else {
        return ptr::null();
    };
    let instrument = match kind {
        9 => Instrument::Fm,
        10 => Instrument::Kick,
        11 => Instrument::Snare,
        12 => Instrument::Hat,
        13 => Instrument::Bass,
        _ => Instrument::Trio,
    };
    let mut boxed = Box::new(Instance {
        plugin: Plugin {
            descriptor: &DESCRIPTORS[kind],
            data: ptr::null_mut(),
            init: Some(init),
            destroy: Some(destroy),
            activate: Some(activate),
            deactivate: Some(deactivate),
            start: Some(start),
            stop: Some(stop),
            reset: Some(reset),
            process: Some(process),
            extension: Some(extension),
            on_main_thread: Some(main_thread),
        },
        kind,
        gain: AtomicU64::new(0.6f64.to_bits()),
        active: AtomicBool::new(false),
        rate: AtomicU64::new(48_000),
        voices: UnsafeCell::new(Vec::with_capacity(64)),
        effect: UnsafeCell::new(None),
        track: Track::new("Plugin", instrument),
    });
    boxed.plugin.data = (&mut *boxed as *mut Instance).cast();
    let result = &boxed.plugin as *const Plugin;
    let _ = Box::into_raw(boxed);
    result
}
unsafe extern "C" fn init(p: *const Plugin) -> bool {
    // SAFETY: solo se consulta una instancia del contrato CLAP.
    unsafe { instance(p).is_some() }
}
unsafe extern "C" fn destroy(p: *const Plugin) {
    // SAFETY: destroy se llama una vez tras desactivar la instancia.
    unsafe {
        if let Some(i) = instance(p) {
            drop(Box::from_raw(i as *const Instance as *mut Instance));
        }
    }
}
unsafe extern "C" fn activate(p: *const Plugin, rate: f64, min: u32, max: u32) -> bool {
    // SAFETY: el host no procesa mientras está activando.
    let Some(i) = (unsafe { instance(p) }) else {
        return false;
    };
    if !rate.is_finite()
        || !(8_000. ..=192_000.).contains(&rate)
        || min == 0
        || max < min
        || max > 4096
    {
        return false;
    }
    let spec = match i.kind {
        1 => Some(Effect::Delay {
            seconds: 0.25,
            feedback: 0.3,
            mix: 0.3,
        }),
        2 => Some(Effect::Reverb {
            decay: 0.6,
            mix: 0.25,
        }),
        3 => Some(Effect::Eq {
            hz: 1000.,
            db: 3.,
            q: 0.7,
        }),
        4 => Some(Effect::Chorus {
            rate: 0.8,
            depth: 0.4,
            mix: 0.3,
        }),
        5 => Some(Effect::Drive {
            drive: 3.,
            mix: 0.4,
        }),
        6 => Some(Effect::Width(1.3)),
        7 => Some(Effect::Compressor {
            threshold: 0.3,
            ratio: 4.,
        }),
        _ => None,
    };
    i.rate.store(rate.round() as u64, Ordering::Relaxed);
    // SAFETY: la activación es exclusiva y anterior al hilo de audio.
    unsafe {
        *i.effect.get() = spec.and_then(|s| Processor::new(&s, rate.round() as u32).ok());
        (*i.voices.get()).clear();
    }
    i.active.store(true, Ordering::Release);
    true
}
unsafe extern "C" fn deactivate(p: *const Plugin) {
    // SAFETY: el host ya ha detenido process.
    unsafe {
        if let Some(i) = instance(p) {
            i.active.store(false, Ordering::Release);
        }
    }
}
unsafe extern "C" fn start(p: *const Plugin) -> bool {
    // SAFETY: instancia entregada por la fábrica.
    unsafe { instance(p).is_some_and(|i| i.active.load(Ordering::Acquire)) }
}
unsafe extern "C" fn stop(p: *const Plugin) {
    // SAFETY: stop no es concurrente con process.
    unsafe {
        if let Some(i) = instance(p) {
            (*i.voices.get()).clear();
        }
    }
}
unsafe extern "C" fn reset(p: *const Plugin) {
    // SAFETY: misma exclusión que stop según CLAP.
    unsafe {
        stop(p);
        if let Some(i) = instance(p)
            && let Some(fx) = &mut *i.effect.get()
        {
            fx.reset();
        }
    }
}

unsafe extern "C" fn main_thread(_p: *const Plugin) {}

unsafe fn event(i: &Instance, event: *const EventHeader) {
    if event.is_null() {
        return;
    }
    // SAFETY: los eventos y su tamaño los entrega el host durante process/flush.
    unsafe {
        let h = &*event;
        if h.space != 0 {
            return;
        }
        if h.kind == 5 && h.size as usize >= size_of::<ParamEvent>() {
            let p = &*event.cast::<ParamEvent>();
            if p.id == 0 && p.value.is_finite() {
                i.gain
                    .store(p.value.clamp(0., 1.).to_bits(), Ordering::Relaxed);
            }
        }
        if i.kind >= 8 && h.size as usize >= size_of::<NoteEvent>() && h.kind <= 2 {
            let n = &*event.cast::<NoteEvent>();
            let voices = &mut *i.voices.get();
            if h.kind == 0 && (0..=127).contains(&n.key) && n.velocity.is_finite() {
                if voices.len() == 64 {
                    voices.remove(0);
                }
                if let Ok(v) = InstrumentVoice::new(
                    n.key as u8,
                    n.velocity.clamp(0., 1.) as f32,
                    i.rate.load(Ordering::Relaxed) as u32,
                ) {
                    voices.push(v);
                }
            } else {
                for v in voices {
                    if n.key < 0 || v.key() == n.key as u8 {
                        v.release();
                    }
                }
            }
        }
    }
}
unsafe extern "C" fn process(p: *const Plugin, data: *const Process) -> i32 {
    if data.is_null() {
        return 0;
    }
    // SAFETY: buffers y listas viven hasta que process retorna; audio exclusivo.
    unsafe {
        let Some(i) = instance(p) else {
            return 0;
        };
        let d = &*data;
        if !i.active.load(Ordering::Acquire)
            || d.frames > 4096
            || d.outputs_count != 1
            || d.outputs.is_null()
        {
            return 0;
        }
        let output = &mut *d.outputs;
        if output.channels != 2 || output.data32.is_null() {
            return 0;
        }
        let left = *output.data32;
        let right = *output.data32.add(1);
        if left.is_null() || right.is_null() {
            return 0;
        }
        let inputs = if i.kind < 8 && d.inputs_count == 1 && !d.inputs.is_null() {
            Some(&*d.inputs)
        } else {
            None
        };
        if inputs.is_some_and(|b| b.data32.is_null() || b.channels != 2) {
            return 0;
        }
        let list = d.input_events.as_ref();
        let total = list
            .and_then(|e| e.size.map(|f| f(e)))
            .unwrap_or(0)
            .min(4096);
        let mut at = 0;
        for frame in 0..d.frames as usize {
            while at < total {
                let Some(events) = list else {
                    break;
                };
                let Some(get) = events.get else {
                    break;
                };
                let e = get(events, at);
                if e.is_null() {
                    at += 1;
                    continue;
                }
                if (*e).time > frame as u32 {
                    break;
                }
                event(i, e);
                at += 1;
            }
            let mut out = [0f32; 2];
            if i.kind >= 8 {
                let voices = &mut *i.voices.get();
                voices.retain_mut(|v| {
                    if let Some(s) = v.frame(&i.track) {
                        out[0] += s[0];
                        out[1] += s[1];
                        true
                    } else {
                        false
                    }
                });
            } else if let Some(input) = inputs {
                let a = *input.data32;
                let b = *input.data32.add(1);
                if a.is_null() || b.is_null() {
                    return 0;
                }
                out = [*a.add(frame), *b.add(frame)];
                if let Some(fx) = &mut *i.effect.get() {
                    out = fx.process(out);
                }
            }
            let gain = f64::from_bits(i.gain.load(Ordering::Relaxed)) as f32;
            *left.add(frame) = if out[0].is_finite() {
                (out[0] * gain).clamp(-0.8, 0.8)
            } else {
                0.
            };
            *right.add(frame) = if out[1].is_finite() {
                (out[1] * gain).clamp(-0.8, 0.8)
            } else {
                0.
            };
        }
        1
    }
}
unsafe extern "C" fn extension(p: *const Plugin, id: *const c_char) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    // SAFETY: consulta de extensión con identificador C válido.
    unsafe {
        match CStr::from_ptr(id).to_bytes() {
            b"clap.audio-ports" => (&AUDIO as *const AudioPorts).cast(),
            b"clap.note-ports" => {
                if instance(p).is_some_and(|i| i.kind >= 8) {
                    (&NOTES as *const NotePorts).cast()
                } else {
                    ptr::null()
                }
            }
            b"clap.params" => (&PARAMS as *const Params).cast(),
            b"clap.state" => (&STATE as *const State).cast(),
            _ => ptr::null(),
        }
    }
}
static AUDIO: AudioPorts = AudioPorts {
    count: Some(audio_count),
    get: Some(audio_info),
};
unsafe extern "C" fn audio_count(p: *const Plugin, input: bool) -> u32 {
    // SAFETY: lectura de metadatos inmutables de la instancia.
    unsafe { instance(p).map_or(0, |i| if input && i.kind >= 8 { 0 } else { 1 }) }
}
unsafe extern "C" fn audio_info(
    p: *const Plugin,
    index: u32,
    input: bool,
    info: *mut PortInfo,
) -> bool {
    if info.is_null() || index != 0 {
        return false;
    }
    // SAFETY: destino válido del host y extensión aún desactivada.
    unsafe {
        if audio_count(p, input) == 0 {
            return false;
        }
        ptr::write(
            info,
            PortInfo {
                id: 0,
                name: [0; 256],
                flags: 1,
                channels: 2,
                kind: c"stereo".as_ptr(),
                pair: u32::MAX,
            },
        );
        put(&mut (*info).name, b"Main");
    }
    true
}
static NOTES: NotePorts = NotePorts {
    count: Some(note_count),
    get: Some(note_info),
};
unsafe extern "C" fn note_count(_p: *const Plugin, input: bool) -> u32 {
    u32::from(input)
}
unsafe extern "C" fn note_info(
    _p: *const Plugin,
    index: u32,
    input: bool,
    info: *mut NotePortInfo,
) -> bool {
    if info.is_null() || !input || index != 0 {
        return false;
    }
    // SAFETY: el host reserva NotePortInfo y solicita el único puerto.
    unsafe {
        ptr::write(
            info,
            NotePortInfo {
                id: 0,
                supported: 1,
                preferred: 1,
                name: [0; 256],
            },
        );
        put(&mut (*info).name, b"Notes");
    }
    true
}
fn put(target: &mut [c_char], text: &[u8]) {
    for (a, &b) in target.iter_mut().take(text.len()).zip(text) {
        *a = b as c_char;
    }
}
static PARAMS: Params = Params {
    count: Some(param_count),
    info: Some(param_info),
    value: Some(param_value),
    to_text: Some(param_text),
    from_text: Some(param_parse),
    flush: Some(flush),
};
unsafe extern "C" fn param_count(_p: *const Plugin) -> u32 {
    1
}
unsafe extern "C" fn param_text(
    _p: *const Plugin,
    id: u32,
    value: f64,
    target: *mut c_char,
    capacity: u32,
) -> bool {
    if id != 0 || !value.is_finite() || target.is_null() || capacity == 0 {
        return false;
    }
    let text = format!("{:.3}", value.clamp(0., 1.));
    if text.len() + 1 > capacity as usize {
        return false;
    }
    // SAFETY: el host reserva capacity bytes; el texto cabe y termina en cero.
    unsafe {
        ptr::copy_nonoverlapping(text.as_ptr().cast(), target, text.len());
        *target.add(text.len()) = 0;
    }
    true
}
unsafe extern "C" fn param_parse(
    _p: *const Plugin,
    id: u32,
    text: *const c_char,
    value: *mut f64,
) -> bool {
    if id != 0 || text.is_null() || value.is_null() {
        return false;
    }
    // SAFETY: CLAP entrega una cadena C y un destino double válidos.
    unsafe {
        let Ok(text) = CStr::from_ptr(text).to_str() else {
            return false;
        };
        let Ok(v) = text.parse::<f64>() else {
            return false;
        };
        if !v.is_finite() || !(0. ..=1.).contains(&v) {
            return false;
        }
        *value = v;
    }
    true
}
unsafe extern "C" fn param_info(_p: *const Plugin, index: u32, info: *mut ParamInfo) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }
    // SAFETY: espacio de ParamInfo aportado por el host.
    unsafe {
        ptr::write(
            info,
            ParamInfo {
                id: 0,
                flags: 1 << 5,
                cookie: ptr::null_mut(),
                name: [0; 256],
                module: [0; 1024],
                min: 0.,
                max: 1.,
                default: 0.6,
            },
        );
        put(&mut (*info).name, b"Gain");
    }
    true
}
unsafe extern "C" fn param_value(p: *const Plugin, id: u32, value: *mut f64) -> bool {
    if id != 0 || value.is_null() {
        return false;
    }
    // SAFETY: instancia válida y destino de un double aportado por el host.
    unsafe {
        let Some(i) = instance(p) else {
            return false;
        };
        *value = f64::from_bits(i.gain.load(Ordering::Relaxed));
    }
    true
}
unsafe extern "C" fn flush(
    p: *const Plugin,
    events: *const InputEvents,
    _out: *const OutputEvents,
) {
    // SAFETY: CLAP serializa flush con process para los eventos de la instancia.
    unsafe {
        let (Some(i), Some(list)) = (instance(p), events.as_ref()) else {
            return;
        };
        let (Some(size), Some(get)) = (list.size, list.get) else {
            return;
        };
        for index in 0..size(events).min(4096) {
            event(i, get(events, index));
        }
    }
}
static STATE: State = State {
    save: Some(save),
    load: Some(load),
};
unsafe extern "C" fn save(p: *const Plugin, stream: *const OStream) -> bool {
    // SAFETY: la instancia y el stream se conservan durante la llamada.
    unsafe {
        let (Some(i), Some(s)) = (instance(p), stream.as_ref()) else {
            return false;
        };
        let Some(write) = s.write else {
            return false;
        };
        let data = i.gain.load(Ordering::Relaxed).to_le_bytes();
        let mut at = 0usize;
        while at < 8 {
            let n = write(stream, data.as_ptr().add(at).cast(), (8 - at) as u64);
            if n <= 0 || n as usize > 8 - at {
                return false;
            }
            at += n as usize;
        }
        true
    }
}
unsafe extern "C" fn load(p: *const Plugin, stream: *const IStream) -> bool {
    // SAFETY: lectura acotada de ocho bytes; no se guarda ningún puntero del host.
    unsafe {
        let (Some(i), Some(s)) = (instance(p), stream.as_ref()) else {
            return false;
        };
        let Some(read) = s.read else {
            return false;
        };
        let mut data = [0u8; 8];
        let mut at = 0;
        while at < 8 {
            let n = read(stream, data.as_mut_ptr().add(at).cast(), (8 - at) as u64);
            if n <= 0 || n as usize > 8 - at {
                return false;
            }
            at += n as usize;
        }
        let v = f64::from_le_bytes(data);
        if !v.is_finite() || !(0. ..=1.).contains(&v) {
            return false;
        }
        i.gain.store(v.to_bits(), Ordering::Relaxed);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Bytes {
        data: Vec<u8>,
        at: usize,
    }
    unsafe extern "C" fn write_piece(
        stream: *const OStream,
        data: *const c_void,
        size: u64,
    ) -> i64 {
        // SAFETY: ctx y origen son buffers propios vivos durante save.
        unsafe {
            let target = &mut *(*stream).ctx.cast::<Bytes>();
            let count = (size as usize).min(3);
            target
                .data
                .extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), count));
            count as i64
        }
    }
    unsafe extern "C" fn read_piece(stream: *const IStream, data: *mut c_void, size: u64) -> i64 {
        // SAFETY: destino acotado por size y contexto exclusivo de este test.
        unsafe {
            let source = &mut *(*stream).ctx.cast::<Bytes>();
            let count = (size as usize).min(2).min(source.data.len() - source.at);
            ptr::copy_nonoverlapping(
                source.data.as_ptr().add(source.at),
                data.cast::<u8>(),
                count,
            );
            source.at += count;
            count as i64
        }
    }
    #[test]
    fn state_supports_partial_streams_and_rejects_corruption() {
        let host = host();
        let mut bytes = Bytes {
            data: Vec::new(),
            at: 0,
        };
        // SAFETY: ciclo propio y streams válidos; ninguna instancia sale del test.
        unsafe {
            let p = create(&FACTORY, &host, DESCRIPTORS[0].id);
            instance(p)
                .unwrap()
                .gain
                .store(0.37f64.to_bits(), Ordering::Relaxed);
            let out = OStream {
                ctx: (&mut bytes as *mut Bytes).cast(),
                write: Some(write_piece),
            };
            assert!(save(p, &out));
            assert_eq!(bytes.data.len(), 8);
            let q = create(&FACTORY, &host, DESCRIPTORS[0].id);
            let input = IStream {
                ctx: (&mut bytes as *mut Bytes).cast(),
                read: Some(read_piece),
            };
            assert!(load(q, &input));
            let mut value = 0.;
            assert!(param_value(q, 0, &mut value));
            assert_eq!(value, 0.37);
            bytes.data = f64::NAN.to_le_bytes().to_vec();
            bytes.at = 0;
            assert!(!load(q, &input));
            assert!(param_value(q, 0, &mut value));
            assert_eq!(value, 0.37);
            bytes.data = vec![0; 7];
            bytes.at = 0;
            assert!(!load(q, &input));
            destroy(p);
            destroy(q);
        }
    }
    unsafe extern "C" fn one(_events: *const InputEvents) -> u32 {
        1
    }
    unsafe extern "C" fn get_event(events: *const InputEvents, _index: u32) -> *const EventHeader {
        // SAFETY: el test conserva el evento dentro de ctx hasta acabar process.
        unsafe { (*events).ctx.cast() }
    }
    fn host() -> Host {
        Host {
            version: VERSION,
            data: ptr::null_mut(),
            name: c"test".as_ptr(),
            vendor: c"test".as_ptr(),
            url: c"".as_ptr(),
            host_version: c"1".as_ptr(),
            extension: None,
            restart: None,
            request_process: None,
            callback: None,
        }
    }
    #[test]
    fn public_c_layout_sizes_match_clap_64_bit() {
        assert_eq!(size_of::<EventHeader>(), 16);
        assert_eq!(size_of::<NoteEvent>(), 40);
        assert_eq!(size_of::<Process>(), 64);
        assert_eq!(size_of::<Descriptor>(), 88);
        assert_eq!(size_of::<Host>(), 88);
        assert_eq!(size_of::<PortInfo>(), 288);
        assert_eq!(size_of::<ParamInfo>(), 1320);
    }
    #[test]
    fn descriptors_are_unique() {
        let mut ids = std::collections::BTreeSet::new();
        for d in &DESCRIPTORS {
            // SAFETY: todos los metadatos del módulo son cadenas estáticas.
            unsafe {
                assert!(ids.insert(CStr::from_ptr(d.id).to_bytes()));
                assert!(!CStr::from_ptr(d.name).is_empty());
            }
        }
    }
    #[test]
    fn every_exported_instrument_responds_to_clap_notes() {
        let host = host();
        for (kind, d) in DESCRIPTORS.iter().enumerate().skip(8) {
            // SAFETY: host, evento y buffers son propios y siguen el ciclo CLAP.
            unsafe {
                let p = create(&FACTORY, &host, d.id);
                assert!(!p.is_null());
                assert!(init(p));
                assert!(activate(p, 8000., 1, 512));
                assert!(start(p));
                let mut event = NoteEvent {
                    header: EventHeader {
                        size: size_of::<NoteEvent>() as u32,
                        time: 0,
                        space: 0,
                        kind: 0,
                        flags: 0,
                    },
                    note_id: 1,
                    port: 0,
                    channel: 0,
                    key: 60,
                    velocity: 0.8,
                };
                let events = InputEvents {
                    ctx: (&mut event as *mut NoteEvent).cast(),
                    size: Some(one),
                    get: Some(get_event),
                };
                let mut left = [0f32; 512];
                let mut right = [0f32; 512];
                let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
                let mut output = AudioBuffer {
                    data32: pointers.as_mut_ptr(),
                    data64: ptr::null_mut(),
                    channels: 2,
                    latency: 0,
                    constant_mask: 0,
                };
                let data = Process {
                    time: 0,
                    frames: 512,
                    transport: ptr::null(),
                    inputs: ptr::null(),
                    outputs: &mut output,
                    inputs_count: 0,
                    outputs_count: 1,
                    input_events: &events,
                    output_events: ptr::null(),
                };
                assert_eq!(process(p, &data), 1);
                assert!(left.iter().any(|v| v.abs() > 0.0001), "instrument {kind}");
                assert!(
                    left.iter()
                        .chain(&right)
                        .all(|v| v.is_finite() && v.abs() <= 0.8)
                );
                stop(p);
                deactivate(p);
                destroy(p);
            }
        }
    }
    #[test]
    fn invalid_activation_and_null_process_fail_closed() {
        let host = host();
        // SAFETY: instancia propia; los argumentos inválidos se rechazan antes de usarse.
        unsafe {
            let p = create(&FACTORY, &host, DESCRIPTORS[0].id);
            assert!(init(p));
            assert!(!activate(p, f64::NAN, 1, 512));
            assert!(!activate(p, 48000., 1, 8192));
            assert_eq!(process(p, ptr::null()), 0);
            destroy(p);
        }
    }
    #[test]
    fn parameter_roundtrip_is_bounded() {
        let host = host();
        // SAFETY: buffers de texto propios y parámetros dentro de la ABI.
        unsafe {
            let p = create(&FACTORY, &host, DESCRIPTORS[0].id);
            let mut text = [0 as c_char; 32];
            assert!(param_text(p, 0, 0.75, text.as_mut_ptr(), 32));
            let mut value = 0.;
            assert!(param_parse(p, 0, text.as_ptr(), &mut value));
            assert_eq!(value, 0.75);
            assert!(!param_parse(p, 0, c"NaN".as_ptr(), &mut value));
            assert!(!param_text(p, 0, 0.75, text.as_mut_ptr(), 1));
            destroy(p);
        }
    }
}
