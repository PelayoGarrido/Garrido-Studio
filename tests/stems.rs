use garrido_studio::{
    Error,
    engine::Engine,
    export::{self, Format, Scope, Settings},
    model::*,
    routing::Route,
    stems, wav,
};
use std::sync::atomic::{AtomicBool, Ordering};

fn session() -> Project {
    let mut p = Project::empty();
    p.rate = 8000;
    p.bars = 1;
    p.add_track(Instrument::Trio).unwrap();
    p.add_track(Instrument::Bass).unwrap();
    p.tracks[0].name = "../CON\\one".into();
    p.tracks[1].name = "../CON\\one".into();
    p.patterns[0].notes = vec![
        Note::new(0, PPQ, 60, 0.1).on_channel(0).unwrap(),
        Note::new(PPQ, PPQ, 48, 0.1).on_channel(1).unwrap(),
    ];
    p.clips.truncate(1);
    p
}
fn settings() -> Settings {
    Settings {
        rate: 8000,
        channels: 2,
        depth: wav::Depth::Pcm24,
        tail: 0.,
        scope: Scope::Mix,
    }
}
#[test]
fn stems_keep_alignment_routes_original_and_independent_channel_sources() {
    let mut p = session();
    p.routes.push(Route {
        from: 0,
        to: Some(1),
        gain: 0.25,
        invert: false,
        swap: false,
    });
    let original = p.clone();
    let mut files = Vec::new();
    assert_eq!(
        stems::render(
            &p,
            settings(),
            Format::Wav,
            &AtomicBool::new(false),
            |name, data| {
                files.push((name.to_string(), data.to_vec()));
                Ok(())
            }
        )
        .unwrap(),
        2
    );
    assert_eq!(p, original);
    assert_ne!(files[0].0, files[1].0);
    assert!(
        files
            .iter()
            .all(|(name, _)| !name.contains(['/', '\\', ':']) && !name.contains(".."))
    );
    for (index, (_, bytes)) in files.iter().enumerate() {
        let isolated = export::prepare(
            &p,
            Settings {
                scope: Scope::Track(index),
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(isolated.routes, p.routes);
        let expected = Engine::new(&isolated, true, 0.)
            .unwrap()
            .render(&AtomicBool::new(false))
            .unwrap();
        assert_eq!(
            *bytes,
            Format::Wav
                .encode(&expected.audio, settings(), &AtomicBool::new(false))
                .unwrap()
        );
        assert_eq!(wav::decode(bytes).unwrap().frames.len(), 16000);
    }
}
#[test]
fn stems_respect_original_mutes_and_solos_and_reject_silent_or_invalid_plans() {
    let mut p = session();
    p.tracks[1].solo = true;
    let plan = stems::plan(&p, settings(), Format::Aiff).unwrap();
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].0, 1);
    p.tracks[1].mute = true;
    assert!(stems::plan(&p, settings(), Format::Wav).is_err());
    assert!(
        stems::plan(
            &session(),
            Settings {
                depth: wav::Depth::Float32,
                ..settings()
            },
            Format::Flac
        )
        .is_err()
    );
    assert!(
        stems::plan(
            &session(),
            Settings {
                scope: Scope::Track(0),
                ..settings()
            },
            Format::Wav
        )
        .is_err()
    );
}
#[test]
fn cancellation_and_sink_errors_stop_before_later_files_are_written() {
    let cancel = AtomicBool::new(false);
    let mut count = 0;
    assert!(
        stems::render(&session(), settings(), Format::Aiff, &cancel, |_, data| {
            assert_eq!(&data[..4], b"FORM");
            count += 1;
            cancel.store(true, Ordering::Relaxed);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(count, 1);
    let mut count = 0;
    assert!(
        stems::render(
            &session(),
            settings(),
            Format::Wav,
            &AtomicBool::new(false),
            |_, _| {
                count += 1;
                Err(Error::Invalid("disk error"))
            }
        )
        .is_err()
    );
    assert_eq!(count, 1);
}
