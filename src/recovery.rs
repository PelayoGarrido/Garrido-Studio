use crate::{Error, Result, codec, model::Project, storage};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const LIMIT_BYTES: u64 = 128 * 1024 * 1024;
const LIMIT_COPIES: usize = 8;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct Store {
    folder: PathBuf,
}

pub(crate) fn directory(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err(Error::Invalid("Ruta de recuperación no válida"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        #[cfg(windows)]
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Error::Invalid(
                    "La recuperación necesita carpetas sin enlaces",
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let builder = fs::DirBuilder::new();
                #[cfg(unix)]
                let mut builder = builder;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(&current)?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

impl Store {
    pub fn open(folder: &Path) -> Result<Self> {
        directory(folder)?;
        Ok(Self {
            folder: folder.to_path_buf(),
        })
    }
    fn inventory(&self) -> Result<(Vec<PathBuf>, u64)> {
        directory(&self.folder)?;
        let mut paths = Vec::new();
        let mut bytes = 0u64;
        for (index, entry) in fs::read_dir(&self.folder)?.enumerate() {
            if index >= 256 {
                return Err(Error::Limit("Demasiados archivos de recuperación"));
            }
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(Error::Invalid("Archivo de recuperación no válido"));
            }
            bytes = bytes
                .checked_add(meta.len())
                .ok_or(Error::Limit("Recuperación demasiado grande"))?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("recovery-") && n.ends_with(".gstudio"))
            {
                paths.push(entry.path());
            }
        }
        paths.sort();
        paths.reverse();
        Ok((paths, bytes))
    }
    pub fn list(&self) -> Result<Vec<PathBuf>> {
        Ok(self.inventory()?.0)
    }
    pub fn save(&self, project: &Project) -> Result<PathBuf> {
        let data = codec::encode(project)?;
        let (paths, bytes) = self.inventory()?;
        if paths.len() >= LIMIT_COPIES || bytes + data.len() as u64 > LIMIT_BYTES {
            return Err(Error::Limit(
                "Recuperación llena: guarda una copia de la sesión. Se conservan las copias anteriores",
            ));
        }
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Invalid("Reloj no válido"))?
            .as_nanos();
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = self.folder.join(format!(
            "recovery-{time:032}-{}-{id}.gstudio",
            std::process::id()
        ));
        storage::write_new(&path, &data)?;
        Ok(path)
    }
}
