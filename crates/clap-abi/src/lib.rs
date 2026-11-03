use std::ffi::{c_char, c_void};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub revision: u32,
}
pub const VERSION: Version = Version {
    major: 1,
    minor: 2,
    revision: 0,
};
#[repr(C)]
pub struct Descriptor {
    pub version: Version,
    pub id: *const c_char,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub url: *const c_char,
    pub manual_url: *const c_char,
    pub support_url: *const c_char,
    pub plugin_version: *const c_char,
    pub description: *const c_char,
    pub features: *const *const c_char,
}
// SAFETY: compartir una descripción no desreferencia sus punteros.
unsafe impl Sync for Descriptor {}
#[repr(C)]
pub struct Entry {
    pub version: Version,
    pub init: Option<unsafe extern "C" fn(*const c_char) -> bool>,
    pub deinit: Option<unsafe extern "C" fn()>,
    pub factory: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}
#[repr(C)]
pub struct Factory {
    pub count: Option<unsafe extern "C" fn(*const Self) -> u32>,
    pub descriptor: Option<unsafe extern "C" fn(*const Self, u32) -> *const Descriptor>,
    pub create:
        Option<unsafe extern "C" fn(*const Self, *const Host, *const c_char) -> *const Plugin>,
}
#[repr(C)]
pub struct Host {
    pub version: Version,
    pub data: *mut c_void,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub url: *const c_char,
    pub host_version: *const c_char,
    pub extension: Option<unsafe extern "C" fn(*const Self, *const c_char) -> *const c_void>,
    pub restart: Option<unsafe extern "C" fn(*const Self)>,
    pub request_process: Option<unsafe extern "C" fn(*const Self)>,
    pub callback: Option<unsafe extern "C" fn(*const Self)>,
}
#[repr(C)]
pub struct Plugin {
    pub descriptor: *const Descriptor,
    pub data: *mut c_void,
    pub init: Option<unsafe extern "C" fn(*const Self) -> bool>,
    pub destroy: Option<unsafe extern "C" fn(*const Self)>,
    pub activate: Option<unsafe extern "C" fn(*const Self, f64, u32, u32) -> bool>,
    pub deactivate: Option<unsafe extern "C" fn(*const Self)>,
    pub start: Option<unsafe extern "C" fn(*const Self) -> bool>,
    pub stop: Option<unsafe extern "C" fn(*const Self)>,
    pub reset: Option<unsafe extern "C" fn(*const Self)>,
    pub process: Option<unsafe extern "C" fn(*const Self, *const Process) -> i32>,
    pub extension: Option<unsafe extern "C" fn(*const Self, *const c_char) -> *const c_void>,
    pub on_main_thread: Option<unsafe extern "C" fn(*const Self)>,
}
#[repr(C)]
pub struct AudioBuffer {
    pub data32: *mut *mut f32,
    pub data64: *mut *mut f64,
    pub channels: u32,
    pub latency: u32,
    pub constant_mask: u64,
}
#[repr(C)]
pub struct Process {
    pub time: i64,
    pub frames: u32,
    pub transport: *const c_void,
    pub inputs: *const AudioBuffer,
    pub outputs: *mut AudioBuffer,
    pub inputs_count: u32,
    pub outputs_count: u32,
    pub input_events: *const InputEvents,
    pub output_events: *const OutputEvents,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EventHeader {
    pub size: u32,
    pub time: u32,
    pub space: u16,
    pub kind: u16,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NoteEvent {
    pub header: EventHeader,
    pub note_id: i32,
    pub port: i16,
    pub channel: i16,
    pub key: i16,
    pub velocity: f64,
}
#[repr(C)]
pub struct ParamEvent {
    pub header: EventHeader,
    pub id: u32,
    pub cookie: *mut c_void,
    pub note_id: i32,
    pub port: i16,
    pub channel: i16,
    pub key: i16,
    pub value: f64,
}
#[repr(C)]
pub struct InputEvents {
    pub ctx: *mut c_void,
    pub size: Option<unsafe extern "C" fn(*const Self) -> u32>,
    pub get: Option<unsafe extern "C" fn(*const Self, u32) -> *const EventHeader>,
}
#[repr(C)]
pub struct OutputEvents {
    pub ctx: *mut c_void,
    pub push: Option<unsafe extern "C" fn(*const Self, *const EventHeader) -> bool>,
}
#[repr(C)]
pub struct AudioPorts {
    pub count: Option<unsafe extern "C" fn(*const Plugin, bool) -> u32>,
    pub get: Option<unsafe extern "C" fn(*const Plugin, u32, bool, *mut PortInfo) -> bool>,
}
#[repr(C)]
pub struct PortInfo {
    pub id: u32,
    pub name: [c_char; 256],
    pub flags: u32,
    pub channels: u32,
    pub kind: *const c_char,
    pub pair: u32,
}
#[repr(C)]
pub struct NotePorts {
    pub count: Option<unsafe extern "C" fn(*const Plugin, bool) -> u32>,
    pub get: Option<unsafe extern "C" fn(*const Plugin, u32, bool, *mut NotePortInfo) -> bool>,
}
#[repr(C)]
pub struct NotePortInfo {
    pub id: u32,
    pub supported: u32,
    pub preferred: u32,
    pub name: [c_char; 256],
}
#[repr(C)]
pub struct ParamInfo {
    pub id: u32,
    pub flags: u32,
    pub cookie: *mut c_void,
    pub name: [c_char; 256],
    pub module: [c_char; 1024],
    pub min: f64,
    pub max: f64,
    pub default: f64,
}
#[repr(C)]
pub struct Params {
    pub count: Option<unsafe extern "C" fn(*const Plugin) -> u32>,
    pub info: Option<unsafe extern "C" fn(*const Plugin, u32, *mut ParamInfo) -> bool>,
    pub value: Option<unsafe extern "C" fn(*const Plugin, u32, *mut f64) -> bool>,
    pub to_text: Option<unsafe extern "C" fn(*const Plugin, u32, f64, *mut c_char, u32) -> bool>,
    pub from_text:
        Option<unsafe extern "C" fn(*const Plugin, u32, *const c_char, *mut f64) -> bool>,
    pub flush: Option<unsafe extern "C" fn(*const Plugin, *const InputEvents, *const OutputEvents)>,
}
#[repr(C)]
pub struct State {
    pub save: Option<unsafe extern "C" fn(*const Plugin, *const OStream) -> bool>,
    pub load: Option<unsafe extern "C" fn(*const Plugin, *const IStream) -> bool>,
}
#[repr(C)]
pub struct OStream {
    pub ctx: *mut c_void,
    pub write: Option<unsafe extern "C" fn(*const Self, *const c_void, u64) -> i64>,
}
#[repr(C)]
pub struct IStream {
    pub ctx: *mut c_void,
    pub read: Option<unsafe extern "C" fn(*const Self, *mut c_void, u64) -> i64>,
}
