#![cfg_attr(all(not(debug_assertions), not(test)), windows_subsystem = "windows")]

mod actions;
mod app;
mod arrangement_ui;
mod audio;
mod audio_settings;
mod brand;
mod canvas;
mod controls;
mod dialogs;
mod export_dialog;
mod files;
mod gesture;
mod history_ui;
mod icon_codes;
mod layout;
mod menus;
mod plugin_host;
mod preferences_ui;
mod session;
mod stems;
mod streaming;
mod workspace;

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!(
            "Garrido Studio {} · Pelayo Garrido Martínez",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args.len() == 3 && (args[0] == "--clap-worker" || args[0] == "--clap-scan-worker") {
        let result = args[2]
            .to_str()
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or(garrido_studio::Error::Invalid("Índice CLAP no válido"))
            .and_then(|index| {
                plugin_host::worker(
                    std::path::Path::new(&args[1]),
                    index,
                    args[0] == "--clap-scan-worker",
                )
            });
        if result.is_err() {
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--probe-output" {
        return audio::probe().map_err(|e| eframe::Error::AppCreation(Box::new(e)));
    }
    if args.len() == 1 && args[0] == "--probe-stream" {
        return streaming::probe().map_err(|e| eframe::Error::AppCreation(Box::new(e)));
    }
    if args.len() == 1 && args[0] == "--check-bundled-plugins" {
        return plugin_host::check_bundled().map_err(|e| eframe::Error::AppCreation(Box::new(e)));
    }
    if args.len() == 1 && args[0] == "--check-bundled-plugins-report" {
        return plugin_host::report().map_err(|e| eframe::Error::AppCreation(Box::new(e)));
    }
    let initial_path = if args.len() == 1 && !args[0].to_string_lossy().starts_with("--") {
        Some(std::path::PathBuf::from(&args[0]))
    } else {
        None
    };
    if !args.is_empty() && initial_path.is_none() {
        return Err(eframe::Error::AppCreation(Box::new(
            garrido_studio::Error::Invalid("Argumentos no válidos"),
        )));
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280., 720.])
            .with_min_inner_size([1024., 576.])
            .with_decorations(true)
            .with_icon(std::sync::Arc::new(brand::window_icon()))
            .with_app_id("studio.garrido.GarridoStudio"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Garrido Studio",
        options,
        Box::new(move |cc| {
            let mut studio = app::Studio::new(cc)?;
            if let Some(path) = initial_path {
                studio.open_path(&path);
            }
            Ok(Box::new(studio))
        }),
    )
}
