use crate::{
    Error, Result,
    engine::Engine,
    export::{self, Format, Scope, Settings},
    model::Project,
};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Plan {
    pub files: Vec<(usize, String)>,
    pub maximum_bytes: u64,
}
pub fn plan(project: &Project, settings: Settings, format: Format) -> Result<Plan> {
    project.validate()?;
    settings.validate()?;
    if !matches!(settings.scope, Scope::Mix) {
        return Err(Error::Invalid("Los stems necesitan la canción completa"));
    }
    if format != Format::Wav && matches!(settings.depth, crate::wav::Depth::Float32) {
        return Err(Error::Unsupported("FLAC y AIFF usan PCM16 o PCM24"));
    }
    let solo = project.tracks.iter().any(|track| track.solo);
    let mut files = Vec::new();
    let mut maximum_bytes = 0u64;
    for (index, track) in project.tracks.iter().enumerate() {
        if track.mute || (solo && !track.solo) {
            continue;
        }
        let prepared = export::prepare(
            project,
            Settings {
                scope: Scope::Track(index),
                ..settings
            },
        )?;
        let engine = Engine::new(&prepared, true, settings.tail)?;
        maximum_bytes = maximum_bytes
            .checked_add(engine.frames() * (settings.channels as u64 * 4 + 1) + 65536)
            .ok_or(Error::Limit("Exportación de stems demasiado grande"))?;
        if maximum_bytes > 512 * 1024 * 1024 {
            return Err(Error::Limit("Stems: máximo 512 MiB por exportación"));
        }
        let name: String = track
            .name
            .chars()
            .take(48)
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        files.push((
            index,
            format!(
                "{:02}-{}.{}",
                index + 1,
                if name.is_empty() { "Channel" } else { &name },
                format.extension()
            ),
        ));
    }
    if files.is_empty() {
        return Err(Error::Invalid("No hay canales activos para exportar"));
    }
    Ok(Plan {
        files,
        maximum_bytes,
    })
}
pub fn render(
    project: &Project,
    settings: Settings,
    format: Format,
    cancel: &AtomicBool,
    mut sink: impl FnMut(&str, &[u8]) -> Result<()>,
) -> Result<usize> {
    let plan = plan(project, settings, format)?;
    let mut written = 0;
    for (index, name) in &plan.files {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Exportación de stems cancelada"));
        }
        let prepared = export::prepare(
            project,
            Settings {
                scope: Scope::Track(*index),
                ..settings
            },
        )?;
        let render = Engine::new(&prepared, true, settings.tail)?.render(cancel)?;
        let data = format.encode(&render.audio, settings, cancel)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Exportación de stems cancelada"));
        }
        sink(name, &data)?;
        written += 1;
    }
    Ok(written)
}
