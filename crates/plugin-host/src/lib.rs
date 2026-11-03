#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[path = "../../../src/plugin_host.rs"]
mod hosted;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub use hosted::{check_bundled, included, instrument, render, scan, worker};

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod restricted {
    use garrido_studio::{Error, Result, model::Project};
    use std::{path::Path, sync::atomic::AtomicBool};
    const UNAVAILABLE: Error =
        Error::Unsupported("El aislamiento CLAP todavía no está validado en este sistema");
    pub fn included() -> Result<std::path::PathBuf> {
        Err(UNAVAILABLE)
    }
    pub fn check_bundled() -> Result<()> {
        Err(UNAVAILABLE)
    }
    pub fn worker(_: &Path, _: usize, _: bool) -> Result<()> {
        Err(UNAVAILABLE)
    }
    pub fn scan(_: &Path) -> Result<Vec<String>> {
        Err(UNAVAILABLE)
    }
    pub fn render(
        _: &Path,
        _: usize,
        _: &[[f32; 2]],
        _: u32,
        _: &AtomicBool,
    ) -> Result<Vec<[f32; 2]>> {
        Err(UNAVAILABLE)
    }
    pub fn instrument(
        _: &Path,
        _: usize,
        _: &Project,
        _: usize,
        _: &AtomicBool,
    ) -> Result<Vec<[f32; 2]>> {
        Err(UNAVAILABLE)
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use restricted::{check_bundled, included, instrument, render, scan, worker};
