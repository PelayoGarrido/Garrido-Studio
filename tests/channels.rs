use garrido_studio::{
    codec,
    engine::Engine,
    export::{self, Scope, Settings},
    midi,
    model::*,
    wav,
};
use std::sync::atomic::AtomicBool;

fn project() -> Project {
    let mut p = Project::empty();
    p.rate = 8000;
    p.bars = 1;
    p.add_track(Instrument::Trio).unwrap();
    p.add_track(Instrument::Bass).unwrap();
    p.patterns.truncate(1);
    p.patterns[0].notes = vec![
        Note::new(0, PPQ, 60, 0.6).on_channel(0).unwrap(),
        Note::new(PPQ, PPQ, 72, 0.6).on_channel(1).unwrap(),
    ];
    p.clips = vec![Clip {
        offset: 0,
        muted: false,
        group: 0,
        track: 0,
        pattern: 0,
        tick: 0,
        length: BAR,
    }];
    p.validate().unwrap();
    p
}
fn render(p: &Project) -> Vec<[f32; 2]> {
    Engine::new(p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio
}
#[test]
fn one_pattern_routes_notes_to_separate_instruments() {
    let p = project();
    let both = render(&p);
    let mut first = p.clone();
    first.tracks[1].mute = true;
    let mut second = p.clone();
    second.tracks[0].mute = true;
    let a = render(&first);
    let b = render(&second);
    assert!(a.iter().flatten().any(|v| v.abs() > 0.01));
    assert!(b.iter().flatten().any(|v| v.abs() > 0.01));
    assert_ne!(both, a);
    assert_ne!(both, b);
    assert!(a[5000..].iter().flatten().all(|v| v.abs() < 0.0001));
    assert!(b[..3500].iter().flatten().all(|v| v.abs() < 0.0001));
}
#[test]
fn global_pattern_sound_does_not_depend_on_playlist_row() {
    let p = project();
    let mut moved = p.clone();
    moved.clips[0].track = 1;
    assert_eq!(render(&p), render(&moved));
}
#[test]
fn channel_ids_are_bounded_and_do_not_expand_note_storage() {
    assert!(ChannelId::new(MAX_TRACKS).is_err());
    assert!(ChannelId::new(usize::MAX).is_err());
    assert_eq!(ChannelId::new(31).unwrap().index(), 31);
    assert_eq!(std::mem::size_of::<Option<ChannelId>>(), 1);
    assert!(std::mem::size_of::<Note>() <= 24);
    let mut p = project();
    p.patterns[0].notes[0].channel = Some(ChannelId::new(2).unwrap());
    assert!(p.validate().is_err());
}
#[test]
fn channel_projects_roundtrip_and_export_all_pattern_channels() {
    let p = project();
    let bytes = codec::encode(&p).unwrap();
    assert_eq!(&bytes[..8], b"GARRIDO\x05");
    assert_eq!(codec::decode(&bytes).unwrap(), p);
    let out = export::prepare(
        &p,
        Settings {
            rate: 8000,
            channels: 2,
            depth: wav::Depth::Pcm24,
            tail: 0.,
            scope: Scope::Pattern {
                track: 0,
                pattern: 0,
            },
        },
    )
    .unwrap();
    assert_eq!(render(&out), render(&p));
}
#[test]
fn midi_export_preserves_channels_inside_a_single_pattern() {
    let p = project();
    let imported = midi::decode(&midi::encode(&p).unwrap()).unwrap();
    let notes: Vec<_> = imported
        .project
        .patterns
        .iter()
        .flat_map(|p| p.notes.iter())
        .collect();
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|n| n.key == 60));
    assert!(notes.iter().any(|n| n.key == 72));
    assert!(imported.project.tracks.len() >= 2);
}

#[test]
fn legacy_shared_patterns_migrate_without_changing_their_audio() {
    let mut p = project();
    p.patterns[0].notes = vec![Note::new(0, PPQ, 60, 0.6)];
    p.clips.push(Clip {
        offset: 0,
        muted: false,
        group: 0,
        track: 1,
        pattern: 0,
        tick: 0,
        length: BAR,
    });
    let original = p.clone();
    let audio = render(&p);
    let migrated = p.with_bound_channels().unwrap();
    assert_eq!(p, original);
    assert_eq!(migrated.patterns.len(), 2);
    assert_eq!(migrated.clips[0].pattern, 0);
    assert_eq!(migrated.clips[1].pattern, 1);
    assert_eq!(render(&migrated), audio);
    assert_eq!(migrated.with_bound_channels().unwrap(), migrated);
}
#[test]
fn playlist_rows_are_independent_and_mute_the_whole_pattern() {
    let mut p = project();
    p.clips[0].track = 31;
    p.validate().unwrap();
    assert!(render(&p).iter().flatten().any(|v| v.abs() > 0.01));
    p.arrangement_mutes = 1u32 << 31;
    assert!(render(&p).iter().flatten().all(|v| *v == 0.));
    assert_eq!(codec::decode(&codec::encode(&p).unwrap()).unwrap(), p);
}
#[test]
fn empty_one_tick_patterns_do_not_expand_into_a_busy_loop() {
    let mut p = project();
    p.patterns[0].length = 1;
    p.patterns[0].notes.clear();
    p.clips = vec![
        Clip {
            offset: 0,
            muted: false,
            group: 0,
            track: 0,
            pattern: 0,
            tick: 0,
            length: BAR
        };
        4096
    ];
    assert!(render(&p).iter().flatten().all(|v| *v == 0.));
}
