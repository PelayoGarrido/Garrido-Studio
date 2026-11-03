use crate::{
    Error, Result,
    codec::{Reader, crc32},
    recovery, storage,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const LIMIT: usize = 65536;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub light: bool,
    pub ghost_notes: bool,
    pub output: Option<String>,
    pub input: Option<String>,
    pub rate: Option<u32>,
    pub buffer: Option<u32>,
    pub input_pair: u16,
    pub all_inputs: bool,
    pub recent: Vec<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            light: false,
            ghost_notes: true,
            output: None,
            input: None,
            rate: None,
            buffer: None,
            input_pair: 0,
            all_inputs: false,
            recent: Vec::new(),
        }
    }
}
fn text(value: &str, maximum: usize) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(Error::Invalid("Texto de preferencias no válido"));
    }
    Ok(())
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.rate.is_some_and(|v| !(8000..=192000).contains(&v))
            || self.buffer.is_some_and(|v| !(32..=4096).contains(&v))
            || self.input_pair >= 16
            || self.recent.len() > 16
        {
            return Err(Error::Invalid("Preferencias fuera de límites"));
        }
        for value in [&self.input, &self.output].into_iter().flatten() {
            text(value, 1024)?;
        }
        for (i, path) in self.recent.iter().enumerate() {
            text(path.to_str().ok_or(Error::Invalid("Ruta sin UTF-8"))?, 2048)?;
            if !path.is_absolute()
                || path
                    .components()
                    .any(|p| matches!(p, std::path::Component::ParentDir))
                || self.recent[..i].contains(path)
            {
                return Err(Error::Invalid("Ruta reciente no válida"));
            }
        }
        Ok(())
    }
    pub fn remember(&mut self, path: &Path) -> Result<()> {
        let mut next = self.clone();
        next.recent.retain(|item| item != path);
        next.recent.insert(0, path.to_path_buf());
        next.recent.truncate(16);
        next.validate()?;
        *self = next;
        Ok(())
    }
}
fn put_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend((value.len() as u32).to_le_bytes());
    bytes.extend(value.as_bytes());
}
pub fn encode(settings: &Settings) -> Result<Vec<u8>> {
    settings.validate()?;
    let mut payload = vec![
        u8::from(settings.light)
            | (u8::from(settings.ghost_notes) << 1)
            | (u8::from(settings.all_inputs) << 2),
    ];
    payload.extend(settings.rate.unwrap_or(0).to_le_bytes());
    payload.extend(settings.buffer.unwrap_or(0).to_le_bytes());
    payload.extend(settings.input_pair.to_le_bytes());
    put_text(&mut payload, settings.output.as_deref().unwrap_or(""));
    put_text(&mut payload, settings.input.as_deref().unwrap_or(""));
    payload.push(settings.recent.len() as u8);
    for path in &settings.recent {
        put_text(
            &mut payload,
            path.to_str().ok_or(Error::Invalid("Ruta sin UTF-8"))?,
        );
    }
    let mut bytes = b"GPREF001".to_vec();
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(crc32(&payload).to_le_bytes());
    bytes.extend(payload);
    if bytes.len() > LIMIT {
        return Err(Error::Limit("Preferencias demasiado grandes"));
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Settings> {
    if bytes.len() > LIMIT {
        return Err(Error::Limit("Preferencias demasiado grandes"));
    }
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != b"GPREF001" {
        return Err(Error::Invalid("Formato de preferencias desconocido"));
    }
    let length = reader.count(LIMIT)?;
    let checksum = reader.u32()?;
    if length != reader.remaining() {
        return Err(Error::Invalid("Preferencias truncadas"));
    }
    let payload = reader.take(length)?;
    if crc32(payload) != checksum {
        return Err(Error::Invalid(
            "Preferencias dañadas; se conserva el archivo",
        ));
    }
    let mut reader = Reader::new(payload);
    let flags = reader.u8()?;
    if flags > 7 {
        return Err(Error::Invalid("Opciones de preferencias desconocidas"));
    }
    let rate = reader.u32()?;
    let buffer = reader.u32()?;
    let input_pair = reader.u16()?;
    fn string(reader: &mut Reader<'_>, maximum: usize) -> Result<String> {
        let count = reader.count(maximum)?;
        Ok(std::str::from_utf8(reader.take(count)?)
            .map_err(|_| Error::Invalid("Preferencias sin UTF-8"))?
            .into())
    }
    let output = string(&mut reader, 1024)?;
    let input = string(&mut reader, 1024)?;
    let count = reader.u8()?;
    if count > 16 {
        return Err(Error::Limit("Demasiados archivos recientes"));
    }
    let mut recent = Vec::with_capacity(count as usize);
    for _ in 0..count {
        recent.push(PathBuf::from(string(&mut reader, 2048)?));
    }
    if reader.remaining() != 0 {
        return Err(Error::Invalid("Datos extra en preferencias"));
    }
    let settings = Settings {
        light: flags & 1 != 0,
        ghost_notes: flags & 2 != 0,
        rate: (rate != 0).then_some(rate),
        buffer: (buffer != 0).then_some(buffer),
        input_pair,
        all_inputs: flags & 4 != 0,
        output: (!output.is_empty()).then_some(output),
        input: (!input.is_empty()).then_some(input),
        recent,
    };
    settings.validate()?;
    Ok(settings)
}

pub struct Store {
    folder: PathBuf,
}
impl Store {
    pub fn open(folder: &Path) -> Result<Self> {
        recovery::directory(folder)?;
        Ok(Self {
            folder: folder.to_path_buf(),
        })
    }
    pub fn load(&self) -> Result<Settings> {
        recovery::directory(&self.folder)?;
        let path = self.folder.join("preferences.gprefs");
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e.into()),
            Ok(_) => decode(&storage::read_bounded(&path, LIMIT)?),
        }
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        let bytes = encode(settings)?;
        self.load()?;
        let target = self.folder.join("preferences.gprefs");
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let pending = self
            .folder
            .join(format!(".preferences-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending)?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&pending, &target)?;
            #[cfg(target_os = "linux")]
            fs::File::open(&self.folder)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&pending);
        }
        result
    }
}
