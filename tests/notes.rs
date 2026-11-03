use garrido_studio::{
    model::*,
    notes::{self, Tool},
};

fn chord() -> Pattern {
    Pattern {
        name: "Chord".into(),
        length: BAR,
        notes: [60, 64, 67]
            .into_iter()
            .map(|k| Note::new(960, 960, k, 0.8))
            .collect(),
    }
}
#[test]
fn chop_preserves_duration_and_unselected_notes() {
    let p = chord();
    let out = notes::apply(&p, &[0], Tool::Chop(7)).unwrap();
    let pieces: Vec<_> = out.notes.iter().filter(|n| n.key == 60).collect();
    assert_eq!(pieces.len(), 7);
    assert_eq!(pieces.iter().map(|n| n.length).sum::<u32>(), 960);
    assert!(
        pieces
            .windows(2)
            .all(|w| w[0].tick + w[0].length == w[1].tick)
    );
    assert!(out.notes.contains(&p.notes[1]));
    assert!(out.notes.contains(&p.notes[2]));
}
#[test]
fn strum_and_arpeggio_order_chord_by_pitch() {
    let p = chord();
    let strum = notes::apply(&p, &[0, 1, 2], Tool::Strum(30)).unwrap();
    assert_eq!(
        strum.notes.iter().map(|n| n.tick).collect::<Vec<_>>(),
        vec![960, 990, 1020]
    );
    let arp = notes::apply(&p, &[0, 1, 2], Tool::Arpeggio(240)).unwrap();
    assert_eq!(
        arp.notes
            .iter()
            .map(|n| (n.tick, n.length))
            .collect::<Vec<_>>(),
        vec![(960, 240), (1200, 240), (1440, 240)]
    );
}
#[test]
fn clipboard_preserves_intervals_and_extends_without_overflow() {
    let p = chord();
    let copied = notes::copy(&p, &[0, 2]).unwrap();
    let pasted = notes::paste(&p, &copied, BAR).unwrap();
    assert_eq!(pasted.notes.len(), 5);
    assert_eq!(pasted.length, BAR + 960);
    assert_eq!(pasted.notes[3].tick, BAR);
    assert_eq!(pasted.notes[4].key - pasted.notes[3].key, 7);
    assert!(notes::paste(&p, &copied, u32::MAX).is_err());
    assert!(notes::copy(&p, &[0, 0]).is_err());
    assert!(notes::copy(&p, &[3]).is_err());
}
#[test]
fn flam_scale_and_delete_stay_in_bounds() {
    let mut p = chord();
    p.notes[0].key = 61;
    let out = notes::apply(&p, &[0, 1, 2], Tool::Flam(120)).unwrap();
    assert_eq!(out.notes.len(), 6);
    assert_eq!(out.notes[0].velocity, 0.4);
    let out = notes::apply(
        &p,
        &[0],
        Tool::Scale {
            root: 0,
            minor: false,
        },
    )
    .unwrap();
    assert_eq!(out.notes[0].key, 60);
    let out = notes::apply(&p, &[1], Tool::Delete).unwrap();
    assert_eq!(out.notes.len(), 2);
    assert!(notes::apply(&p, &[0], Tool::Chop(0)).is_err());
    for n in &out.notes {
        n.validate(out.length).unwrap();
    }
}
