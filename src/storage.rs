use crate::{Error, Result, model::MAX_FILE};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn read(path: &Path) -> Result<Vec<u8>> {
    read_bounded(path, MAX_FILE)
}

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if limit > MAX_FILE {
        return Err(Error::Limit("Límite de lectura demasiado grande"));
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(Error::Invalid("Se necesita un archivo normal, sin enlaces"));
    }
    if meta.len() > limit as u64 {
        return Err(Error::Limit("Archivo supera el límite de lectura"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x100 | 0x4);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000 | 0x800);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000).share_mode(0x0000_0001);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.file_type().is_symlink() {
        return Err(Error::Invalid("Archivo no válido"));
    }
    let mut data = Vec::with_capacity(meta.len() as usize);
    file.take(limit as u64 + 1).read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(Error::Limit("Archivo supera el límite de lectura"));
    }
    Ok(data)
}

pub fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    if data.len() > 128 * 1024 * 1024 {
        return Err(Error::Limit("Salida: máximo 128 MB"));
    }
    if path.file_name().is_none() {
        return Err(Error::Invalid("Nombre de archivo no válido"));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)?;
    let destination = parent.join(path.file_name().ok_or(Error::Invalid("Nombre no válido"))?);
    let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(".garrido-{}-{id}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(data)?;
        file.sync_all()?;
        fs::hard_link(&temp, &destination)?;
        Ok(())
    })();
    drop(file);
    let cleanup = fs::remove_file(&temp);
    result?;
    cleanup?;
    #[cfg(target_os = "linux")]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
