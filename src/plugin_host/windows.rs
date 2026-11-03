use super::{Error, Result};
use std::{
    ffi::{CStr, c_char, c_void},
    os::windows::ffi::OsStrExt,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    active_processes: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct ExtendedLimits {
    basic: BasicLimits,
    io: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "LoadLibraryExW"]
    fn load_library_ex_w(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    #[link_name = "GetProcAddress"]
    fn get_proc_address(module: *mut c_void, name: *const c_char) -> *mut c_void;
    #[link_name = "FreeLibrary"]
    fn free_library(module: *mut c_void) -> i32;
    #[link_name = "CreateJobObjectW"]
    fn create_job_object_w(security: *const c_void, name: *const u16) -> *mut c_void;
    #[link_name = "SetInformationJobObject"]
    fn set_information_job_object(
        job: *mut c_void,
        kind: i32,
        information: *const c_void,
        length: u32,
    ) -> i32;
    #[link_name = "AssignProcessToJobObject"]
    fn assign_process_to_job_object(job: *mut c_void, process: *mut c_void) -> i32;
    #[link_name = "GetCurrentProcess"]
    fn get_current_process() -> *mut c_void;
    #[link_name = "CloseHandle"]
    fn close_handle(handle: *mut c_void) -> i32;
}
static JOB: AtomicUsize = AtomicUsize::new(0);
pub(super) fn jail() -> Result<()> {
    if JOB.load(Ordering::Acquire) != 0 {
        return Ok(());
    }
    let limits = ExtendedLimits {
        basic: BasicLimits {
            process_time: 100_000_000,
            flags: 0x2 | 0x8 | 0x100 | 0x400 | 0x2000,
            active_processes: 1,
            ..Default::default()
        },
        process_memory: 384 * 1024 * 1024,
        ..Default::default()
    };
    // SAFETY: estructuras Win64 propias; el job contiene exclusivamente este worker.
    unsafe {
        let job = create_job_object_w(ptr::null(), ptr::null());
        if job.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        if set_information_job_object(
            job,
            9,
            (&limits as *const ExtendedLimits).cast(),
            std::mem::size_of::<ExtendedLimits>() as u32,
        ) == 0
            || assign_process_to_job_object(job, get_current_process()) == 0
        {
            let error = std::io::Error::last_os_error();
            close_handle(job);
            return Err(Error::Io(error));
        }
        JOB.store(job as usize, Ordering::Release);
    }
    Ok(())
}
pub(super) unsafe fn dlopen(path: *const c_char, _: i32) -> *mut c_void {
    // SAFETY: el llamador aporta un CString UTF-8 vivo durante toda la conversión.
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
    let Ok(path) = std::str::from_utf8(bytes) else {
        return ptr::null_mut();
    };
    let wide: Vec<_> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: ruta absoluta terminada en cero; las dependencias se buscan solo en System32.
    unsafe { load_library_ex_w(wide.as_ptr(), ptr::null_mut(), 0x800) }
}
pub(super) unsafe fn dlsym(module: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: módulo abierto y nombre C estático aportados por el cargador CLAP.
    unsafe { get_proc_address(module, name) }
}
pub(super) unsafe fn dlclose(module: *mut c_void) -> i32 {
    // SAFETY: todas las instancias se han cerrado antes de liberar este módulo.
    unsafe { free_library(module) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_job_layout_matches_the_native_x64_abi() {
        assert_eq!(std::mem::size_of::<BasicLimits>(), 64);
        assert_eq!(std::mem::size_of::<ExtendedLimits>(), 144);
        assert_eq!(std::mem::offset_of!(ExtendedLimits, process_memory), 112);
        assert_eq!(std::mem::offset_of!(BasicLimits, affinity), 48);
    }
}
