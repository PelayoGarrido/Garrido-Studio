use garrido_studio::{
    Result, arrangement, codec,
    model::{Project, Sample},
    spectral, storage, wav,
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<()> {
    let folder = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("outputs/spectral-journey"));
    std::fs::create_dir_all(&folder)?;
    let cancel = AtomicBool::new(false);
    let [a, b] = spectral::example_sources();
    let started = Instant::now();
    let a = spectral::analyze(&a, 24, &cancel)?;
    let b = spectral::analyze(&b, 24, &cancel)?;
    let analysis = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let settings = spectral::Settings::default();
    let output = spectral::compare(&a, &b, settings, &cancel)?;
    let elapsed = started.elapsed().as_secs_f64();
    let mut project = Project::empty();
    project.rate = settings.rate;
    project.bpm = 120.;
    arrangement::add_audio(
        &mut project,
        output.journey.clone(),
        "Spectral Journey",
        0,
        0,
    )?;
    arrangement::add_audio(
        &mut project,
        output.crossfade.clone(),
        "Matched crossfade",
        0,
        1,
    )?;
    project.tracks[1].mute = true;
    storage::write_new(
        &folder.join("Spectral-Journey.gstudio"),
        &codec::encode(&project)?,
    )?;
    for (name, sample) in [
        ("journey", &output.journey),
        ("crossfade", &output.crossfade),
    ] {
        storage::write_new(
            &folder.join(format!("{name}.wav")),
            &wav::encode(&sample.frames, sample.rate, wav::Depth::Pcm24, false)?,
        )?;
    }
    let paths = output
        .routes
        .iter()
        .map(|p| {
            format!(
                "{{\"from_hz\":{:.6},\"to_hz\":{:.6},\"power\":{:.9}}}",
                p.from_hz, p.to_hz, p.power
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    storage::write_new(
        &folder.join("routes.json"),
        format!("[\n{paths}\n]\n").as_bytes(),
    )?;
    let rms = |s: &Sample| {
        (s.frames
            .iter()
            .flatten()
            .map(|v| (*v as f64).powi(2))
            .sum::<f64>()
            / (s.frames.len() * 2) as f64)
            .sqrt()
    };
    let report = format!(
        "{{\n  \"analysis_seconds\": {analysis:.6},\n  \"comparison_render_seconds\": {elapsed:.6},\n  \"audio_seconds\": {},\n  \"rate\": {},\n  \"routes\": {},\n  \"journey_rms\": {:.9},\n  \"crossfade_rms\": {:.9},\n  \"sources\": \"owned synthetic harmonic fixtures\",\n  \"microphone_opened\": false,\n  \"audio_played\": false\n}}\n",
        settings.seconds,
        settings.rate,
        output.routes.len(),
        rms(&output.journey),
        rms(&output.crossfade)
    );
    storage::write_new(&folder.join("report.json"), report.as_bytes())?;
    println!("{report}");
    Ok(())
}
