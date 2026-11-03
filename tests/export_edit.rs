use garrido_studio::{
    edit,
    export::{self, Scope, Settings},
    model::*,
    routing::Route,
    wav,
};
use std::sync::Arc;
#[test]
fn mono_pcm24_padding_and_float_output_are_readable() {
    for depth in [wav::Depth::Pcm16, wav::Depth::Pcm24, wav::Depth::Float32] {
        let settings = Settings {
            rate: 44100,
            channels: 1,
            depth,
            tail: 0.,
            scope: Scope::Mix,
        };
        let bytes = export::encode(&[[0.5, -0.25]; 3], settings).unwrap();
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
            bytes.len()
        );
        let sample = wav::decode(&bytes).unwrap();
        assert_eq!(sample.rate, 44100);
        assert_eq!(sample.frames.len(), 3);
        assert!((sample.frames[0][0] - 0.125).abs() < 0.0001);
        assert_eq!(sample.frames[0][0], sample.frames[0][1]);
    }
}
#[test]
fn export_scope_preserves_original_and_bus_routes() {
    let mut p = Project::demo();
    p.routes.push(Route {
        from: 0,
        to: Some(1),
        gain: 0.5,
        invert: false,
        swap: false,
    });
    let original = p.clone();
    let settings = Settings {
        rate: 44100,
        channels: 2,
        depth: wav::Depth::Pcm24,
        tail: 1.,
        scope: Scope::Pattern {
            track: 0,
            pattern: 0,
        },
    };
    let out = export::prepare(&p, settings).unwrap();
    assert_eq!(out.clips.len(), 1);
    assert_eq!(out.clips[0].pattern, 0);
    assert_eq!(out.bars, 1);
    assert_eq!(out.routes, p.routes);
    assert!(out.tracks[0].solo);
    assert!(!out.tracks[1].solo);
    assert_eq!(p, original);
    assert!(
        export::prepare(
            &p,
            Settings {
                scope: Scope::Track(99),
                ..settings
            }
        )
        .is_err()
    );
    assert!(
        export::prepare(
            &p,
            Settings {
                tail: f64::NAN,
                ..settings
            }
        )
        .is_err()
    );
}
#[test]
fn sample_clipboard_splices_without_touching_original() {
    let sample = Sample {
        rate: 8000,
        frames: Arc::new(vec![[0.1; 2], [0.2; 2], [0.3; 2], [0.4; 2]]),
    };
    let copied = edit::copy(&sample, 1, 3).unwrap();
    let cut = edit::splice(&sample, 1, 3, None).unwrap();
    assert_eq!(cut.frames.as_ref(), &vec![[0.1; 2], [0.4; 2]]);
    let restored = edit::splice(&cut, 1, 1, Some(&copied)).unwrap();
    assert_eq!(restored, sample);
    assert_eq!(sample.frames.len(), 4);
    assert!(edit::copy(&sample, 3, 2).is_err());
    assert!(edit::splice(&sample, 0, 5, None).is_err());
}
#[test]
fn sample_paste_converts_rate_and_refuses_nonfinite_audio() {
    let sample = Sample {
        rate: 8000,
        frames: Arc::new(Vec::new()),
    };
    let replacement = Sample {
        rate: 16000,
        frames: Arc::new(vec![[0.2; 2]; 100]),
    };
    let out = edit::splice(&sample, 0, 0, Some(&replacement)).unwrap();
    assert_eq!(out.frames.len(), 50);
    assert_eq!(out.rate, 8000);
    assert!(
        out.frames
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 0.3)
    );
    let invalid = Sample {
        rate: 8000,
        frames: Arc::new(vec![[f32::NAN; 2]]),
    };
    assert!(edit::splice(&sample, 0, 0, Some(&invalid)).is_err());
}
