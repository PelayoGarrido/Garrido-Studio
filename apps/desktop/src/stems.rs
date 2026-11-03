use crate::app::*;
use garrido_studio::{
    export::{Format, Settings},
    stems,
};
use std::{
    fs,
    path::Path,
    sync::atomic::AtomicU64,
    time::{SystemTime, UNIX_EPOCH},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const RESERVE: u64 = 3 * 1024 * 1024 * 1024;

pub(crate) fn write(
    project: &Project,
    settings: Settings,
    format: Format,
    parent: &Path,
    cancel: &AtomicBool,
) -> Result<Prepared> {
    let plan = stems::plan(project, settings, format)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Invalid("Exportación cancelada"));
    }
    let parent = fs::canonicalize(parent)?;
    if !parent.is_dir() || fs4::available_space(&parent)? < RESERVE + plan.maximum_bytes {
        return Err(Error::Limit(
            "No hay espacio para los stems y la reserva de 3 GiB",
        ));
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Invalid("Reloj no válido"))?
        .as_nanos();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let folder = parent.join(format!(
        "Garrido-Stems-{stamp}-{}-{sequence}",
        std::process::id()
    ));
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let mut builder = builder;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&folder)?;
    let result = (|| -> Result<usize> {
        storage::write_new(&folder.join("STEMS.txt"), format!("Garrido Studio\n{} Hz · {} channels\nChannel effects and sends included. Files share the same start and duration.\n", settings.rate, settings.channels).as_bytes())?;
        stems::render(project, settings, format, cancel, |name, data| {
            if fs4::available_space(&folder)? < RESERVE + data.len() as u64 {
                return Err(Error::Limit(
                    "Exportación detenida para conservar 3 GiB libres",
                ));
            }
            storage::write_new(&folder.join(name), data)
        })
    })();
    match result {
        Ok(files) => Ok(Prepared::Stems { folder, files }),
        Err(error) => Err(Error::Io(std::io::Error::other(format!(
            "{error}. Se conservan los archivos ya escritos en {}",
            folder.display()
        )))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_uses_a_new_folder_and_never_replaces_existing_audio() {
        let folder = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("garrido-stem-files-{}", std::process::id()));
        fs::create_dir(&folder).unwrap();
        let original = folder.join("01-Trio.wav");
        storage::write_new(&original, b"original-music").unwrap();
        let mut project = Project::empty();
        project.rate = 8000;
        project.bars = 1;
        project.add_track(Instrument::Trio).unwrap();
        project.patterns[0].notes.push(Note::new(0, PPQ, 60, 0.1));
        let settings = Settings {
            rate: 8000,
            channels: 2,
            depth: wav::Depth::Pcm24,
            tail: 0.,
            scope: garrido_studio::export::Scope::Mix,
        };
        let plan = stems::plan(&project, settings, Format::Wav).unwrap();
        let enough = fs4::available_space(&folder).unwrap() >= RESERVE + plan.maximum_bytes;
        let result = write(
            &project,
            settings,
            Format::Wav,
            &folder,
            &AtomicBool::new(false),
        );
        if enough {
            let Prepared::Stems {
                folder: output,
                files,
            } = result.unwrap()
            else {
                panic!("stems result expected")
            };
            assert_eq!(files, 1);
            assert_ne!(output, folder);
            assert_eq!(output.parent(), Some(folder.as_path()));
            let audio = storage::read(&output.join(&plan.files[0].1)).unwrap();
            assert_eq!(wav::decode(&audio).unwrap().frames.len(), 16000);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(storage::read(&original).unwrap(), b"original-music");
        fs::remove_dir_all(folder).unwrap();
    }
}
