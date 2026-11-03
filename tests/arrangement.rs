use garrido_studio::{
    arrangement::{self, ClipId, Tool},
    codec,
    engine::Engine,
    history::History,
    midi,
    model::*,
};
use std::sync::{Arc, atomic::AtomicBool};

fn sample(rate: u32) -> Sample {
    Sample {
        rate,
        frames: Arc::new(
            (0..rate)
                .map(|i| {
                    let a = ((i as f32 * 0.013).sin() + (i as f32 * 0.037).cos()) * 0.08;
                    [a, -a * 0.7]
                })
                .collect(),
        ),
    }
}
fn audio() -> Project {
    let mut p = Project::empty();
    p.rate = 8000;
    p.bars = 2;
    p.bpm = 137.;
    arrangement::add_audio(&mut p, sample(11025), "Take", 97, 4).unwrap();
    p
}
fn render(p: &Project) -> Vec<[f32; 2]> {
    Engine::new(p, true, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio
}
#[test]
fn audio_splits_are_phase_exact_at_fractional_resampling_positions() {
    let p = audio();
    let before = render(&p);
    let sample = p.tracks[0].sample.as_ref().unwrap().frames.clone();
    for cut in [300, 777, 1291, 1980] {
        let split = arrangement::apply(&p, &[ClipId::Audio(0)], Tool::Split(cut)).unwrap();
        assert_eq!(render(&split), before, "cut {cut}");
        assert!(Arc::ptr_eq(
            &split.tracks[0].sample.as_ref().unwrap().frames,
            &sample
        ));
        let joined =
            arrangement::apply(&split, &[ClipId::Audio(0), ClipId::Audio(1)], Tool::Join).unwrap();
        assert_eq!(joined, p);
        assert_eq!(render(&joined), before);
    }
}
#[test]
fn version_five_roundtrips_audio_and_reads_the_version_four_fixture() {
    let mut p = audio();
    p.audio_clips[0].gain = 0.43;
    p.audio_clips[0].group = 27;
    let bytes = codec::encode(&p).unwrap();
    assert_eq!(&bytes[..8], b"GARRIDO\x05");
    let decoded = codec::decode(&bytes).unwrap();
    assert_eq!(decoded, p);
    assert_eq!(render(&decoded), render(&p));
    let old = codec::decode(include_bytes!("../fixtures/multichannel.gstudio")).unwrap();
    assert_eq!(old.tracks.len(), 3);
    assert!(old.audio_clips.is_empty());
    assert!(
        old.clips
            .iter()
            .all(|c| c.offset == 0 && !c.muted && c.group == 0)
    );
}
#[test]
fn mute_delete_groups_and_history_are_transactional() {
    let p = audio();
    let p = arrangement::apply(&p, &[ClipId::Audio(0)], Tool::Split(777)).unwrap();
    let ids = [ClipId::Audio(0), ClipId::Audio(1)];
    let grouped = arrangement::apply(&p, &ids, Tool::Group).unwrap();
    let muted = arrangement::apply(&grouped, &[ids[0]], Tool::Mute).unwrap();
    assert!(muted.audio_clips.iter().all(|c| c.muted));
    assert!(render(&muted).iter().flatten().all(|v| *v == 0.));
    assert_eq!(
        arrangement::apply(&muted, &[ids[0]], Tool::Mute).unwrap(),
        grouped
    );
    let deleted = arrangement::apply(&grouped, &[ids[1]], Tool::Delete).unwrap();
    assert!(deleted.audio_clips.is_empty());
    assert_eq!(deleted.tracks[0].sample, grouped.tracks[0].sample);
    let mut history = History::new(grouped.clone()).unwrap();
    history
        .edit(|p| {
            *p = deleted;
            Ok(())
        })
        .unwrap();
    assert!(history.undo());
    assert_eq!(history.project, grouped);
    assert!(history.redo());
    assert!(history.project.audio_clips.is_empty());
}
#[test]
fn audio_clips_follow_channel_mutes_solo_and_playlist_mutes() {
    let p = audio();
    assert!(render(&p).iter().flatten().any(|v| v.abs() > 0.01));
    let mut next = p.clone();
    next.tracks[0].mute = true;
    assert!(render(&next).iter().flatten().all(|v| *v == 0.));
    let mut next = p.clone();
    next.arrangement_mutes = 1 << 4;
    assert!(render(&next).iter().flatten().all(|v| *v == 0.));
    let mut next = p.clone();
    next.add_channel(Instrument::Bass).unwrap();
    next.tracks[1].solo = true;
    assert!(render(&next).iter().flatten().all(|v| *v == 0.));
}
#[test]
fn copied_groups_are_independent_and_moves_are_clamped_as_one_unit() {
    let p = audio();
    let split = arrangement::apply(&p, &[ClipId::Audio(0)], Tool::Split(777)).unwrap();
    let grouped =
        arrangement::apply(&split, &[ClipId::Audio(0), ClipId::Audio(1)], Tool::Group).unwrap();
    let duplicated =
        arrangement::apply(&grouped, &[ClipId::Audio(0)], Tool::Duplicate(BAR)).unwrap();
    assert_ne!(
        duplicated.audio_clips[0].group,
        duplicated.audio_clips[2].group
    );
    assert_eq!(
        duplicated.audio_clips[2].group,
        duplicated.audio_clips[3].group
    );
    let moved = arrangement::apply(
        &grouped,
        &[ClipId::Audio(0)],
        Tool::Move {
            ticks: i64::MIN,
            lanes: i32::MAX,
        },
    )
    .unwrap();
    assert_eq!(moved.audio_clips[0].tick, 0);
    assert_eq!(moved.audio_clips[0].track, MAX_LANES - 1);
    assert_eq!(moved.audio_clips[1].tick - moved.audio_clips[0].tick, 680);
    assert!(
        arrangement::paste(
            &grouped,
            &arrangement::copy(&grouped, &[ClipId::Audio(0)]).unwrap(),
            MAX_TICKS
        )
        .is_err()
    );
    assert_eq!(grouped.audio_clips.len(), 2);
}
#[test]
fn pattern_splits_keep_loop_phase_in_audio_and_midi() {
    let mut p = Project::starter();
    p.rate = 8000;
    p.bpm = 120.;
    p.bars = 1;
    p.tracks[0].instrument = Instrument::Trio;
    p.patterns[0].length = BAR;
    p.patterns[0].notes = vec![
        Note::new(0, 100, 60, 0.5).on_channel(0).unwrap(),
        Note::new(1920, 100, 67, 0.5).on_channel(0).unwrap(),
    ];
    p.clips.push(Clip {
        track: 2,
        pattern: 0,
        tick: 0,
        length: BAR,
        offset: 0,
        muted: false,
        group: 0,
    });
    let next = arrangement::apply(&p, &[ClipId::Pattern(0)], Tool::Split(960)).unwrap();
    assert_eq!(render(&next), render(&p));
    assert_eq!(midi::encode(&next).unwrap(), midi::encode(&p).unwrap());
    assert_eq!(next.clips[1].offset, 960);
    assert!(arrangement::apply(&p, &[ClipId::Pattern(0)], Tool::Split(50)).is_err());
}
#[test]
fn malformed_audio_metadata_is_rejected_without_touching_the_project() {
    let p = audio();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 1e30] {
        let mut bad = p.clone();
        bad.audio_clips[0].offset = value;
        assert!(bad.validate().is_err());
        assert!(codec::encode(&bad).is_err());
    }
    let mut bad = p.clone();
    bad.audio_clips[0].channel = ChannelId::new(31).unwrap();
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.audio_clips[0].origin = i32::MIN;
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.audio_clips[0].offset = 1e30;
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.audio_clips[0].gain = f32::INFINITY;
    assert!(bad.validate().is_err());
    let mut next = p.clone();
    assert!(arrangement::add_audio(&mut next, sample(11025), "bad\nname", 0, 0).is_err());
    assert_eq!(next, p);
}

#[test]
fn splitting_a_full_voice_bank_does_not_steal_other_audio() {
    let mut p = audio();
    let c = p.audio_clips[0].clone();
    p.audio_clips = vec![c; 64];
    let ids: Vec<_> = (0..64).map(ClipId::Audio).collect();
    let next = arrangement::apply(&p, &ids, Tool::Split(777)).unwrap();
    let a = Engine::new(&p, true, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap();
    let b = Engine::new(&next, true, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(a.audio, b.audio);
    assert_eq!(b.stolen_voices, 0);
}
#[test]
fn moving_a_cropped_take_across_tempo_changes_keeps_its_source_position() {
    let mut p = audio();
    p.tempos = vec![(960, 93.), (1920, 177.)];
    let split = arrangement::apply(&p, &[ClipId::Audio(0)], Tool::Split(777)).unwrap();
    let moved = arrangement::apply(
        &split,
        &[ClipId::Audio(1)],
        Tool::Move {
            ticks: 1500,
            lanes: -2,
        },
    )
    .unwrap();
    let old = &split.audio_clips[1];
    let new = &moved.audio_clips[1];
    let rate = p.tracks[0].sample.as_ref().unwrap().rate as f64;
    let frame = |tick: f64| (p.seconds_at(tick) * p.rate as f64).round();
    let old_position =
        old.offset + (frame(old.tick as f64) - frame(old.origin as f64)) * rate / p.rate as f64;
    assert!((new.offset - old_position).abs() < 1e-8);
    assert_eq!(new.origin, new.tick as i32);
    assert_eq!(new.track, 2);
    assert_eq!(moved.tracks[0].sample, p.tracks[0].sample);
}
