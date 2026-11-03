use garrido_studio::{
    codec, dsp,
    edit::{self, Edit},
    engine::Engine,
    history::{self, History},
    midi,
    model::*,
    storage, stretch,
    wav::{self, Depth},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

fn small() -> Project {
    let mut p = Project::empty();
    p.rate = 8_000;
    p.bars = 1;
    p.add_track(Instrument::Trio).unwrap();
    p.patterns[0].notes.push(Note::new(0, PPQ, 60, 0.5));
    p
}
fn render(p: &Project) -> Vec<[f32; 2]> {
    Engine::new(p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio
}
fn sample() -> Sample {
    Sample {
        rate: 8_000,
        frames: Arc::new(
            (0..2048)
                .map(|i| {
                    let x = (i as f32 * std::f32::consts::TAU * 220. / 8000.).sin() * 0.6;
                    [x, x * 0.5]
                })
                .collect(),
        ),
    }
}

#[test]
fn demo_is_valid_and_original_project_roundtrips() {
    let p = Project::demo();
    p.validate().unwrap();
    assert_eq!(codec::decode(&codec::encode(&p).unwrap()).unwrap(), p);
}
#[test]
fn metronome_follows_tempo_and_never_leaks_into_plain_render() {
    let mut p = Project::empty();
    p.rate = 8_000;
    p.bpm = 120.;
    p.bars = 1;
    let plain = render(&p);
    assert!(plain.iter().all(|v| *v == [0.; 2]));
    let clicked = Engine::new(&p, false, 0.)
        .unwrap()
        .with_metronome()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio;
    for beat in 0..4 {
        let at = beat * 4_000;
        assert!(clicked[at..at + 160].iter().any(|v| v[0].abs() > 0.001));
        assert!(
            clicked[at + 600..at + 3_999]
                .iter()
                .all(|v| v[0].abs() < 0.00001)
        );
    }
    assert!(
        clicked
            .iter()
            .all(|v| v[0].is_finite() && v[0] == v[1] && v[0].abs() < 0.1)
    );
}
#[test]
fn crc_matches_standard_vector() {
    assert_eq!(codec::crc32(b"123456789"), 0xcbf43926);
}
#[test]
fn project_rejects_every_truncated_prefix() {
    let bytes = codec::encode(&Project::demo()).unwrap();
    for end in 0..bytes.len() {
        assert!(codec::decode(&bytes[..end]).is_err(), "{end}");
    }
}
#[test]
fn project_rejects_every_single_byte_corruption() {
    let bytes = codec::encode(&Project::demo()).unwrap();
    for i in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[i] ^= 1;
        assert!(codec::decode(&changed).is_err(), "{i}");
    }
}
#[test]
fn project_samples_and_all_effects_roundtrip() {
    let mut p = small();
    p.tracks[0].sample = Some(sample());
    p.tracks[0].effects = vec![
        Effect::Eq {
            hz: 900.,
            db: 4.,
            q: 1.,
        },
        Effect::Delay {
            seconds: 0.12,
            feedback: 0.3,
            mix: 0.2,
        },
        Effect::Reverb {
            decay: 0.4,
            mix: 0.3,
        },
        Effect::Chorus {
            rate: 0.8,
            depth: 0.4,
            mix: 0.3,
        },
        Effect::Drive {
            drive: 3.,
            mix: 0.1,
        },
        Effect::Width(1.2),
        Effect::Compressor {
            threshold: 0.3,
            ratio: 4.,
        },
    ];
    p.automation.push(Automation {
        target: Target::Pan(0),
        curve: Curve::Smooth,
        points: vec![(0, -1.), (BAR, 1.)],
    });
    p.tempos.push((PPQ, 140.));
    p.markers.push((960, "Entrada".into()));
    assert_eq!(codec::decode(&codec::encode(&p).unwrap()).unwrap(), p);
}
#[test]
fn finite_parameter_boundaries() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1., 2.] {
        let mut p = small();
        p.master = value;
        assert!(p.validate().is_err());
    }
    let mut p = small();
    p.bpm = f64::NAN;
    assert!(p.validate().is_err());
    p = small();
    p.tracks[0].pan = 2.;
    assert!(p.validate().is_err());
}
#[test]
fn references_and_time_are_checked() {
    let mut p = small();
    p.clips[0].track = 32;
    assert!(p.validate().is_err());
    p = small();
    p.patterns[0].notes[0].length = u32::MAX;
    assert!(p.validate().is_err());
    p = small();
    p.patterns[0].notes[0].key = 128;
    assert!(p.validate().is_err());
}
#[test]
fn long_dense_renders_are_rejected_before_processing() {
    let mut p = small();
    p.bars = 256;
    assert!(Engine::new(&p, false, 0.).is_err());
    p = small();
    p.patterns[0].notes = vec![Note::new(0, BAR, 60, 0.5); 20_000];
    assert!(Engine::new(&p, false, 0.).is_err());
}
#[test]
fn tempo_map_integrates_and_inverts() {
    let mut p = small();
    p.tempos = vec![(960, 60.), (1920, 180.)];
    assert!((p.seconds_at(960.) - 0.5).abs() < 1e-12);
    assert!((p.seconds_at(1920.) - 1.5).abs() < 1e-12);
    for tick in [0., 1., 500., 960., 1200., 1920., 3840.] {
        assert!((p.tick_at(p.seconds_at(tick)) - tick).abs() < 1e-8);
    }
}
#[test]
fn automation_is_bounded_and_requires_unique_sorted_points() {
    let mut p = small();
    let a = Automation {
        target: Target::Master,
        curve: Curve::Linear,
        points: vec![(0, 0.), (960, 1.)],
    };
    assert_eq!(a.at(480.), 0.5);
    assert_eq!(a.at(-1.), 0.);
    assert_eq!(a.at(2000.), 1.);
    p.automation.push(a.clone());
    p.validate().unwrap();
    p.automation.push(a);
    assert!(p.validate().is_err());
}
#[test]
fn output_is_deterministic_and_non_silent() {
    let p = small();
    let a = render(&p);
    assert_eq!(a, render(&p));
    assert_eq!(a.len(), 16_000);
    assert!(a.iter().flatten().any(|x| x.abs() > 0.01));
    assert!(a.iter().flatten().all(|x| x.is_finite() && x.abs() <= 0.8));
}
#[test]
fn processing_is_independent_of_block_size() {
    let p = small();
    let expected = render(&p);
    for size in [1, 7, 64, 511, 1024] {
        let mut engine = Engine::new(&p, false, 0.).unwrap();
        let mut result = vec![[0.; 2]; expected.len()];
        for block in result.chunks_mut(size) {
            engine.process(block);
        }
        assert_eq!(result, expected, "{size}");
    }
}
#[test]
fn notes_begin_on_the_scheduled_sample() {
    let mut p = small();
    p.patterns[0].notes[0].tick = 960;
    let audio = render(&p);
    assert!(audio[..4000].iter().flatten().all(|x| *x == 0.));
    assert!(audio[4000..4200].iter().flatten().any(|x| x.abs() > 0.001));
}
#[test]
fn mute_and_solo_are_respected() {
    let mut p = small();
    p.tracks[0].mute = true;
    assert!(render(&p).iter().flatten().all(|x| *x == 0.));
    p.tracks[0].mute = false;
    p.add_track(Instrument::Kick).unwrap();
    p.tracks[1].solo = true;
    assert!(render(&p).iter().flatten().all(|x| *x == 0.));
}
#[test]
fn render_obeys_cancellation() {
    assert!(
        Engine::new(&small(), false, 0.)
            .unwrap()
            .render(&AtomicBool::new(true))
            .is_err()
    );
}
#[test]
fn voice_count_is_bounded_and_stealing_is_reported() {
    let mut p = small();
    p.patterns[0].notes = vec![Note::new(0, 240, 60, 0.1); 65];
    let r = Engine::new(&p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(r.peak_voices, 64);
    assert_eq!(r.stolen_voices, 1);
}
#[test]
fn each_instrument_is_finite_and_audible_offline() {
    for instrument in Instrument::ALL {
        let mut p = small();
        p.tracks[0].instrument = instrument;
        p.tracks[0].sample = Some(sample());
        let frames = render(&p);
        assert!(
            frames
                .iter()
                .flatten()
                .all(|x| x.is_finite() && x.abs() <= 0.8)
        );
        assert!(
            frames.iter().flatten().any(|x| x.abs() > 0.0001),
            "{instrument:?}"
        );
    }
}
#[test]
fn every_effect_is_stable_under_impulse_and_silence() {
    let effects = [
        Effect::Eq {
            hz: 3500.,
            db: 24.,
            q: 12.,
        },
        Effect::Delay {
            seconds: 0.01,
            feedback: 0.9,
            mix: 1.,
        },
        Effect::Reverb {
            decay: 0.9,
            mix: 1.,
        },
        Effect::Chorus {
            rate: 8.,
            depth: 1.,
            mix: 1.,
        },
        Effect::Drive {
            drive: 20.,
            mix: 1.,
        },
        Effect::Width(2.),
        Effect::Compressor {
            threshold: 0.01,
            ratio: 20.,
        },
    ];
    for effect in effects {
        let mut processor = dsp::Processor::new(&effect, 8000).unwrap();
        for i in 0..32_000 {
            let value = processor.process(if i == 0 { [1., -1.] } else { [0.; 2] });
            assert!(value.iter().all(|x| x.is_finite() && x.abs() <= 64.));
        }
    }
}
#[test]
fn equal_power_pan_conserves_energy() {
    for i in -100..=100 {
        let pan = dsp::pan(i as f32 / 100.);
        assert!((pan[0] * pan[0] + pan[1] * pan[1] - 1.).abs() < 1e-6);
    }
}
#[test]
fn output_guard_rejects_nonfinite_samples_and_bounds_dc() {
    let mut guard = dsp::OutputGuard::new(8000);
    assert_eq!(guard.process([f32::NAN, f32::INFINITY]), [0.; 2]);
    let mut last = [0.; 2];
    for _ in 0..20_000 {
        last = guard.process([1., -1.]);
    }
    assert!(last.iter().all(|x| x.abs() < 1e-5));
}
#[test]
fn wav_roundtrip_covers_all_export_depths() {
    let s = sample();
    for (depth, tolerance) in [
        (Depth::Pcm16, 0.00002),
        (Depth::Pcm24, 0.0000001),
        (Depth::Float32, 0.),
    ] {
        let bytes = wav::encode(&s.frames, s.rate, depth, false).unwrap();
        let decoded = wav::decode(&bytes).unwrap();
        assert_eq!(decoded.rate, s.rate);
        for (a, b) in s.frames.iter().zip(decoded.frames.iter()) {
            for i in 0..2 {
                assert!((a[i] - b[i]).abs() <= tolerance);
            }
        }
    }
}
#[test]
fn wav_rejects_truncation_nonfinite_and_broken_alignment() {
    let bytes = wav::encode(&sample().frames, 8000, Depth::Float32, false).unwrap();
    for end in 0..bytes.len() {
        assert!(wav::decode(&bytes[..end]).is_err());
    }
    let mut broken = bytes.clone();
    broken[44..48].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(wav::decode(&broken).is_err());
    broken = bytes;
    broken[32] = 255;
    assert!(wav::decode(&broken).is_err());
}
#[test]
fn midi_roundtrip_preserves_notes_and_tempo_changes() {
    let mut p = small();
    p.tempos = vec![(960, 90.), (1920, 150.)];
    p.markers = vec![(960, "Puente".into())];
    let imported = midi::decode(&midi::encode(&p).unwrap()).unwrap().project;
    assert_eq!(imported.patterns[0].notes.len(), 1);
    assert_eq!(imported.patterns[0].notes[0].key, 60);
    assert_eq!(imported.tempos.len(), 2);
    assert!((imported.tempos[0].1 - 90.).abs() < 0.001);
    assert_eq!(imported.markers, p.markers);
}
#[test]
fn midi_rejects_every_truncated_prefix() {
    let bytes = midi::encode(&small()).unwrap();
    for end in 0..bytes.len() {
        assert!(midi::decode(&bytes[..end]).is_err(), "{end}");
    }
}
#[test]
fn parser_mutations_never_panic() {
    let base = codec::encode(&Project::demo()).unwrap();
    let mut state = 0x123456789abcdef;
    for i in 0..1200 {
        let mut bytes = base.clone();
        let position = 16 + (dsp::noise(&mut state).abs() * (bytes.len() - 17) as f64) as usize;
        bytes[position] ^= (i % 255 + 1) as u8;
        let crc = codec::crc32(&bytes[16..]);
        bytes[12..16].copy_from_slice(&crc.to_le_bytes());
        let _ = codec::decode(&bytes);
    }
    for size in 0..512 {
        let bytes: Vec<u8> = (0..size)
            .map(|_| (dsp::noise(&mut state) * 127. + 128.) as u8)
            .collect();
        let _ = codec::decode(&bytes);
        let _ = wav::decode(&bytes);
        let _ = midi::decode(&bytes);
    }
}
#[test]
fn history_rolls_back_invalid_edits_and_preserves_redo() {
    let p = small();
    let mut h = History::new(p.clone()).unwrap();
    assert!(
        h.edit(|p| {
            p.bpm = 0.;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(h.project, p);
    h.edit(|p| {
        p.bpm = 130.;
        Ok(())
    })
    .unwrap();
    assert!(h.undo());
    assert_eq!(h.project, p);
    assert!(h.redo());
    assert_eq!(h.project.bpm, 130.);
}
#[test]
fn note_tools_preserve_model_invariants() {
    let mut p = small();
    p.patterns[0].notes.push(Note::new(1200, 300, 127, 0.8));
    history::quantize(&mut p.patterns[0], 240, 1.);
    history::transpose(&mut p.patterns[0], 12);
    history::reverse(&mut p.patterns[0]);
    history::legato(&mut p.patterns[0]);
    history::humanize(&mut p.patterns[0], 1, 30, 0.1);
    p.validate().unwrap();
}
#[test]
fn sample_edits_preserve_original_and_selection() {
    let s = sample();
    let original = s.frames.clone();
    for effect in [
        Edit::Reverse,
        Edit::Normalize,
        Edit::FadeIn,
        Edit::FadeOut,
        Edit::Silence,
        Edit::Gain(2.),
        Edit::Trim,
    ] {
        let edited = edit::apply(&s, 20, 100, effect).unwrap();
        assert_eq!(s.frames, original);
        assert!(edited.frames.iter().flatten().all(|x| x.is_finite()));
        if matches!(effect, Edit::Trim) {
            assert_eq!(edited.frames.len(), 80);
        } else {
            assert_eq!(edited.frames[0], s.frames[0]);
        }
    }
}
#[test]
fn stretch_is_bounded_and_preserves_identity() {
    let s = sample();
    assert_eq!(stretch::granular(&s, 1., 0.).unwrap(), s);
    for ratio in [0.5, 1.5, 2.] {
        let result = stretch::granular(&s, ratio, 0.).unwrap();
        assert_eq!(
            result.frames.len(),
            (s.frames.len() as f64 * ratio).round() as usize
        );
        assert!(
            result
                .frames
                .iter()
                .flatten()
                .all(|x| x.is_finite() && x.abs() < 1.)
        );
    }
    assert!(stretch::granular(&s, f64::NAN, 0.).is_err());
    assert!(stretch::resample(&s, 10.).is_err());
}

static TEMP: AtomicU64 = AtomicU64::new(0);
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "garrido-tests-{}-{}",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
#[test]
fn saving_never_overwrites_a_destination() {
    let dir = temp();
    let file = dir.join("original.gstudio");
    storage::write_new(&file, b"original").unwrap();
    assert!(storage::write_new(&file, b"replacement").is_err());
    assert_eq!(storage::read(&file).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn output_symlinks_cannot_replace_originals() {
    let dir = temp();
    let file = dir.join("original");
    std::fs::write(&file, b"preserved").unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert!(storage::write_new(&link, b"bad").is_err());
    assert!(storage::read(&link).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"preserved");
    std::fs::remove_dir_all(dir).unwrap();
}
