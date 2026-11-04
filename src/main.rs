use garrido_studio::{Error, Result, codec, engine::Engine, midi, model::Project, storage, wav};
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod audio_macos;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod capture_macos;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod macos;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod plugin_host;

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return macos::run();
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            return Err(Error::Unsupported(
                "La interfaz nativa requiere macOS Apple Silicon. El motor funciona por CLI.",
            ));
        }
    }
    match args[0].to_str().unwrap_or("") {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "probe-audio" if args.len() == 1 => audio_macos::probe(),
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "check-bundled-plugins" if args.len() == 1 => plugin_host::check_bundled(),
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "--clap-worker" | "--clap-scan-worker" if args.len() == 3 => {
            let index = args[2]
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or(Error::Invalid("Índice no válido"))?;
            plugin_host::worker(Path::new(&args[1]), index, args[0] == "--clap-scan-worker")
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "clap-scan" if args.len() == 2 => {
            for info in plugin_host::scan(Path::new(&args[1]))? {
                println!("{info}");
            }
            Ok(())
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "clap-render" if args.len() == 5 => {
            let index = args[2]
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or(Error::Invalid("Índice no válido"))?;
            let sample = wav::decode(&storage::read(Path::new(&args[3]))?)?;
            let frames = plugin_host::render(
                Path::new(&args[1]),
                index,
                &sample.frames,
                sample.rate,
                &AtomicBool::new(false),
            )?;
            storage::write_new(
                Path::new(&args[4]),
                &wav::encode(&frames, sample.rate, wav::Depth::Pcm24, true)?,
            )
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        "clap-instrument" if args.len() == 6 => {
            let index = args[2]
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or(Error::Invalid("Índice no válido"))?;
            let pattern = args[4]
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or(Error::Invalid("Patrón no válido"))?;
            let project = codec::decode(&storage::read(Path::new(&args[3]))?)?;
            let audio = plugin_host::instrument(
                Path::new(&args[1]),
                index,
                &project,
                pattern,
                &AtomicBool::new(false),
            )?;
            storage::write_new(
                Path::new(&args[5]),
                &wav::encode(&audio, project.rate, wav::Depth::Pcm24, true)?,
            )
        }
        "--version" => {
            println!(
                "Garrido Studio {} · Pelayo Garrido Martínez",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        "spectral-journey" if args.len() == 5 || args.len() == 7 => {
            let number = |i: usize| {
                args[i]
                    .to_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or(Error::Invalid("Parámetro espectral no válido"))
            };
            let mut settings = garrido_studio::spectral::Settings {
                seconds: number(4)?,
                ..Default::default()
            };
            if args.len() == 7 {
                settings.start = number(5)?;
                settings.end = number(6)?;
            }
            settings.validate()?;
            let load = |path: &Path| {
                let bytes = storage::read_bounded(path, 24 * 1024 * 1024)?;
                if bytes.starts_with(b"fLaC") {
                    garrido_studio::flac::decode(&bytes)
                } else if bytes.starts_with(b"FORM") {
                    garrido_studio::aiff::decode(&bytes)
                } else {
                    wav::decode(&bytes)
                }
            };
            let cancel = AtomicBool::new(false);
            let a = garrido_studio::spectral::analyze(&load(Path::new(&args[1]))?, 24, &cancel)?;
            let b = garrido_studio::spectral::analyze(&load(Path::new(&args[2]))?, 24, &cancel)?;
            let result = garrido_studio::spectral::render(&a, &b, settings, &cancel)?;
            storage::write_new(
                Path::new(&args[3]),
                &wav::encode(&result.frames, result.rate, wav::Depth::Pcm24, false)?,
            )?;
            println!(
                "Spectral Journey · {} modos A · {} modos B · {} frames",
                a.modes().len(),
                b.modes().len(),
                result.frames.len()
            );
            Ok(())
        }
        "convert-audio" if args.len() == 3 => {
            let input = storage::read(Path::new(&args[1]))?;
            let sample = if input.starts_with(b"fLaC") {
                garrido_studio::flac::decode(&input)?
            } else if input.starts_with(b"FORM") {
                garrido_studio::aiff::decode(&input)?
            } else {
                wav::decode(&input)?
            };
            let path = Path::new(&args[2]);
            let data = match path.extension().and_then(|s| s.to_str()) {
                Some("flac") => garrido_studio::flac::encode(&sample.frames, sample.rate, 24, 2)?,
                Some("wav") => wav::encode(&sample.frames, sample.rate, wav::Depth::Pcm24, false)?,
                Some("aif" | "aiff") => garrido_studio::aiff::encode(
                    &sample.frames,
                    sample.rate,
                    wav::Depth::Pcm24,
                    2,
                    false,
                )?,
                _ => return Err(Error::Invalid("Elige .wav, .flac o .aiff")),
            };
            storage::write_new(path, &data)
        }
        "demo" if args.len() == 2 => {
            storage::write_new(Path::new(&args[1]), &codec::encode(&Project::demo())?)
        }
        "render-demo" if args.len() == 2 => render(&Project::demo(), Path::new(&args[1])),
        "render" if args.len() == 3 => render(
            &codec::decode(&storage::read(Path::new(&args[1]))?)?,
            Path::new(&args[2]),
        ),
        "flp-roundtrip" if args.len() == 3 => {
            let d = garrido_studio::flp::Document::parse(storage::read(Path::new(&args[1]))?)?;
            storage::write_new(Path::new(&args[2]), d.original())
        }
        "import-flp" if args.len() == 3 => {
            let d = garrido_studio::flp::Document::parse(storage::read(Path::new(&args[1]))?)?;
            let imported = d.import()?;
            for warning in &imported.warnings {
                eprintln!("{warning}");
            }
            storage::write_new(Path::new(&args[2]), &codec::encode(&imported.project)?)
        }
        "export-flp" if args.len() == 3 => {
            let p = codec::decode(&storage::read(Path::new(&args[1]))?)?;
            let original = p
                .source_flp
                .as_ref()
                .ok_or(Error::Invalid("Esta sesión no conserva un FLP de origen"))?;
            let d = garrido_studio::flp::Document::parse(original.as_ref().clone())?;
            storage::write_new(Path::new(&args[2]), &d.update_notes_and_tempo(&p)?)
        }
        "import-midi" if args.len() == 3 => {
            let imported = midi::decode(&storage::read(Path::new(&args[1]))?)?;
            println!(
                "{} canales; {} eventos sin representación en esta versión",
                imported.project.tracks.len(),
                imported.ignored_events
            );
            storage::write_new(Path::new(&args[2]), &codec::encode(&imported.project)?)
        }
        "export-midi" if args.len() == 3 => storage::write_new(
            Path::new(&args[2]),
            &midi::encode(&codec::decode(&storage::read(Path::new(&args[1]))?)?)?,
        ),
        "inspect" if args.len() == 2 => {
            let p = codec::decode(&storage::read(Path::new(&args[1]))?)?;
            println!(
                "{} · {} BPM · {} Hz · {} canales · {} patrones · {} clips",
                p.title,
                p.bpm,
                p.rate,
                p.tracks.len(),
                p.patterns.len(),
                p.clips.len() + p.audio_clips.len()
            );
            Ok(())
        }
        "bench" if args.len() == 1 => {
            let p = Project::demo();
            let start = Instant::now();
            let rendered = Engine::new(&p, false, 1.)?.render(&AtomicBool::new(false))?;
            let elapsed = start.elapsed().as_secs_f64();
            let duration = rendered.audio.len() as f64 / p.rate as f64;
            println!(
                "audio_s={duration:.3} render_s={elapsed:.6} realtime_factor={:.2} peak_voices={} stolen={} peak={:.6} rms={:.6}",
                duration / elapsed,
                rendered.peak_voices,
                rendered.stolen_voices,
                rendered.peak,
                rendered.rms
            );
            Ok(())
        }
        _ => Err(Error::Invalid(
            "Uso: garrido-studio [demo RUTA | render-demo WAV | render PROYECTO WAV | spectral-journey A B WAV SEGUNDOS [INICIO FIN] | import-midi MIDI PROYECTO | export-midi PROYECTO MIDI | inspect PROYECTO | bench | --version]",
        )),
    }
}

fn render(project: &Project, path: &Path) -> Result<()> {
    let rendered = Engine::new(project, true, 2.)?.render(&AtomicBool::new(false))?;
    let data = match path.extension().and_then(|s| s.to_str()) {
        Some("wav") => wav::encode(&rendered.audio, rendered.rate, wav::Depth::Pcm24, true)?,
        Some("flac") => garrido_studio::flac::encode(&rendered.audio, rendered.rate, 24, 2)?,
        Some("aif" | "aiff") => garrido_studio::aiff::encode(
            &rendered.audio,
            rendered.rate,
            wav::Depth::Pcm24,
            2,
            true,
        )?,
        _ => return Err(Error::Invalid("Elige .wav, .flac o .aiff")),
    };
    storage::write_new(path, &data)?;
    println!(
        "{} frames · pico {:.4} · {} voces robadas",
        rendered.audio.len(),
        rendered.peak,
        rendered.stolen_voices
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
