use garrido_studio::{codec, flp::Document, model::PPQ};
use std::collections::BTreeMap;

fn event(id: u8, bytes: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    if id >= 192 {
        let mut n = bytes.len();
        loop {
            out.push((n & 127) as u8 | if n > 127 { 128 } else { 0 });
            n >>= 7;
            if n == 0 {
                break;
            }
        }
    }
    out.extend(bytes);
}
fn fixture() -> Vec<u8> {
    let mut data = Vec::new();
    event(199, b"24.2.0\0", &mut data);
    event(156, &120000u32.to_le_bytes(), &mut data);
    event(28, &[0], &mut data);
    event(200, b"unchanged-license-data\0", &mut data);
    event(65, &1u16.to_le_bytes(), &mut data);
    let mut n = [0u8; 24];
    n[..4].copy_from_slice(&96u32.to_le_bytes());
    n[8..12].copy_from_slice(&24u32.to_le_bytes());
    n[12..14].copy_from_slice(&60u16.to_le_bytes());
    n[16] = 120;
    n[18] = 64;
    n[20] = 64;
    n[21] = 100;
    event(224, &n, &mut data);
    event(251, &[0xff, 0x80, 0x0a, 0], &mut data);
    let mut bytes = b"FLhd".to_vec();
    bytes.extend(6u32.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(96u16.to_le_bytes());
    bytes.extend(b"FLdt");
    bytes.extend((data.len() as u32).to_le_bytes());
    bytes.extend(data);
    bytes.extend(b"TEST");
    bytes.extend(3u32.to_le_bytes());
    bytes.extend([1, 2, 3]);
    bytes
}
#[test]
fn opaque_roundtrip_is_byte_exact() {
    let bytes = fixture();
    let d = Document::parse(bytes.clone()).unwrap();
    assert_eq!(d.rewrite(&BTreeMap::new()).unwrap(), bytes);
}
#[test]
fn every_truncated_prefix_is_rejected_except_valid_chunk_boundary() {
    let bytes = fixture();
    for end in 0..bytes.len() {
        let result = Document::parse(bytes[..end].to_vec());
        if end == bytes.len() - 11 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "{end}");
        }
    }
}
#[test]
fn notes_and_ppq_are_mapped() {
    let imported = Document::parse(fixture()).unwrap().import().unwrap();
    let n = imported.project.patterns[0].notes[0];
    assert_eq!(n.tick, PPQ);
    assert_eq!(n.length, PPQ / 4);
    assert_eq!(n.key, 60);
    assert!((n.velocity - 100. / 128.).abs() < 1e-6);
}
#[test]
fn untouched_model_export_retains_license_and_unknown_bytes() {
    let source = fixture();
    let d = Document::parse(source.clone()).unwrap();
    let p = d.import().unwrap().project;
    assert_eq!(d.update_notes_and_tempo(&p).unwrap(), source);
}
#[test]
fn tempo_change_preserves_every_other_event_and_outer_chunks() {
    let d = Document::parse(fixture()).unwrap();
    let next = Document::parse(d.with_tempo(142.5).unwrap()).unwrap();
    for (a, b) in d.events.iter().zip(&next.events) {
        if a.id != 156 {
            assert_eq!(d.value(a), next.value(b));
        }
    }
    assert_eq!(next.import().unwrap().project.bpm, 142.5);
    assert!(
        next.original()
            .ends_with(&[b'T', b'E', b'S', b'T', 3, 0, 0, 0, 1, 2, 3])
    );
}
#[test]
fn edited_notes_roundtrip_with_opaque_events() {
    let d = Document::parse(fixture()).unwrap();
    let mut p = d.import().unwrap().project;
    p.patterns[0].notes[0].key = 67;
    let next = Document::parse(d.update_notes_and_tempo(&p).unwrap()).unwrap();
    assert_eq!(next.import().unwrap().project.patterns[0].notes[0].key, 67);
    for (a, b) in d.events.iter().zip(&next.events) {
        if a.id != 224 {
            assert_eq!(d.value(a), next.value(b));
        }
    }
}
#[test]
fn unsupported_arrangement_edits_fail_instead_of_being_lost() {
    let d = Document::parse(fixture()).unwrap();
    let mut p = d.import().unwrap().project;
    p.clips.clear();
    assert!(d.update_notes_and_tempo(&p).is_err());
}
#[test]
fn pattern_metadata_changes_are_not_silently_lost() {
    let d = Document::parse(fixture()).unwrap();
    let original = d.import().unwrap().project;
    let mut p = original.clone();
    p.patterns[0].name = "New name".into();
    assert!(d.update_notes_and_tempo(&p).is_err());
    let mut p = original.clone();
    p.patterns[0].length *= 2;
    assert!(d.update_notes_and_tempo(&p).is_err());
    let mut p = original;
    p.rate = 44100;
    assert!(d.update_notes_and_tempo(&p).is_err());
}
#[test]
fn native_session_embeds_original_flp() {
    let d = Document::parse(fixture()).unwrap();
    let p = d.import().unwrap().project;
    let decoded = codec::decode(&codec::encode(&p).unwrap()).unwrap();
    assert_eq!(decoded.source_flp.unwrap().as_slice(), d.original());
}
#[test]
fn malformed_lengths_and_varints_fail_closed() {
    let mut b = fixture();
    b[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Document::parse(b).is_err());
    let d = Document::parse(fixture()).unwrap();
    let mut changes = BTreeMap::new();
    changes.insert(1, vec![0; 3]);
    assert!(d.rewrite(&changes).is_err());
}

fn multichannel_fixture() -> Vec<u8> {
    let d = Document::parse(fixture()).unwrap();
    let index = d.events.iter().position(|e| e.id == 224).unwrap();
    let mut first = d.value(&d.events[index]).to_vec();
    first[6..8].copy_from_slice(&0u16.to_le_bytes());
    first[17] = 11;
    let mut second = first.clone();
    second[6..8].copy_from_slice(&3u16.to_le_bytes());
    second[12..14].copy_from_slice(&72u16.to_le_bytes());
    second[17] = 22;
    first.extend(second);
    let mut replacements = BTreeMap::new();
    replacements.insert(index, first);
    let mut bytes = d.rewrite(&replacements).unwrap();
    bytes[10..12].copy_from_slice(&4u16.to_le_bytes());
    bytes
}
#[test]
fn multiple_flp_channels_share_one_pattern_and_one_preview_clip() {
    let bytes = multichannel_fixture();
    let d = Document::parse(bytes.clone()).unwrap();
    let p = d.import().unwrap().project;
    assert_eq!(p.tracks.len(), 2);
    assert_eq!(p.patterns.len(), 1);
    assert_eq!(p.clips.len(), 1);
    assert_eq!(p.patterns[0].notes[0].channel.unwrap().index(), 0);
    assert_eq!(p.patterns[0].notes[1].channel.unwrap().index(), 1);
    assert_eq!(d.update_notes_and_tempo(&p).unwrap(), bytes);
}
#[test]
fn reordered_flp_notes_keep_their_own_opaque_record_fields() {
    let d = Document::parse(multichannel_fixture()).unwrap();
    let mut p = d.import().unwrap().project;
    p.patterns[0].notes.reverse();
    p.patterns[0].notes[0].key = 74;
    let next = Document::parse(d.update_notes_and_tempo(&p).unwrap()).unwrap();
    let event = next.events.iter().find(|e| e.id == 224).unwrap();
    let data = next.value(event);
    assert_eq!(data.len(), 48);
    for note in data.as_chunks::<24>().0 {
        let channel = u16::from_le_bytes([note[6], note[7]]);
        assert_eq!(note[17], if channel == 0 { 11 } else { 22 });
    }
    let imported = next.import().unwrap();
    assert_eq!(imported.project.patterns[0].notes.len(), 2);
    assert!(
        imported.project.patterns[0]
            .notes
            .iter()
            .any(|n| n.key == 74)
    );
}
#[test]
fn invalid_source_note_reference_is_refused_without_rewriting() {
    let d = Document::parse(multichannel_fixture()).unwrap();
    let mut p = d.import().unwrap().project;
    p.patterns[0].notes[0].source_id = Some(garrido_studio::model::SourceNoteId::new(100).unwrap());
    assert!(d.update_notes_and_tempo(&p).is_err());
}

fn playlist_fixture(records: &[u8], version: &[u8], selected: u16) -> Vec<u8> {
    let source = fixture();
    let doc = Document::parse(source).unwrap();
    let mut data = Vec::new();
    for e in &doc.events {
        if e.id == 199 {
            event(199, version, &mut data);
        } else {
            event(e.id, doc.value(e), &mut data);
        }
    }
    event(65, &3u16.to_le_bytes(), &mut data);
    event(193, b"Empty pattern\0", &mut data);
    event(99, &0u16.to_le_bytes(), &mut data);
    event(233, records, &mut data);
    let mut track = [0u8; 13];
    track[12] = 1;
    for i in 0..4 {
        track[12] = if i == 2 { 0 } else { 1 };
        event(238, &track, &mut data);
    }
    event(99, &1u16.to_le_bytes(), &mut data);
    event(233, &[], &mut data);
    event(100, &selected.to_le_bytes(), &mut data);
    let mut bytes = b"FLhd".to_vec();
    bytes.extend(6u32.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(96u16.to_le_bytes());
    bytes.extend(b"FLdt");
    bytes.extend((data.len() as u32).to_le_bytes());
    bytes.extend(data);
    bytes
}
fn playlist_item(tick: u32, length: u32, pattern: u16, lane: u16, group: u16) -> [u8; 32] {
    let mut r = [0u8; 32];
    r[..4].copy_from_slice(&tick.to_le_bytes());
    r[4..6].copy_from_slice(&20480u16.to_le_bytes());
    r[6..8].copy_from_slice(&(20480 + pattern).to_le_bytes());
    r[8..12].copy_from_slice(&length.to_le_bytes());
    r[12..14].copy_from_slice(&(499 - lane).to_le_bytes());
    r[14..16].copy_from_slice(&group.to_le_bytes());
    r[16] = 120;
    r[18] = 64;
    r[20..24].copy_from_slice(&[64, 100, 128, 128]);
    r[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    r[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    r
}
#[test]
fn fl20_playlist_maps_positions_groups_empty_patterns_and_track_mutes() {
    let mut records = playlist_item(384, 768, 1, 2, 9).to_vec();
    records.extend(playlist_item(1536, 384, 3, 5, 9));
    let bytes = playlist_fixture(&records, b"20.8.4.2576\0", 0);
    let d = Document::parse(bytes.clone()).unwrap();
    let p = d.import().unwrap().project;
    assert_eq!(p.clips.len(), 2);
    let c = &p.clips[0];
    assert_eq!((c.tick, c.length, c.track, c.group), (3840, 7680, 2, 9));
    assert!(p.patterns[p.clips[1].pattern].notes.is_empty());
    assert_eq!(p.arrangement_mutes, 1 << 2);
    assert_eq!(d.update_notes_and_tempo(&p).unwrap(), bytes);
    assert_eq!(codec::decode(&codec::encode(&p).unwrap()).unwrap(), p);
}
#[test]
fn selecting_an_empty_flp_arrangement_does_not_place_other_patterns() {
    let records = playlist_item(384, 768, 1, 2, 0);
    let d = Document::parse(playlist_fixture(&records, b"20.8.4.2576\0", 1)).unwrap();
    let p = d.import().unwrap().project;
    assert!(p.clips.is_empty());
    assert_eq!(p.arrangement_mutes, 0);
    assert!(!p.patterns[0].notes.is_empty());
}
#[test]
fn unverified_playlist_flags_crops_and_versions_remain_opaque() {
    let mut records = playlist_item(0, 384, 1, 2, 0);
    records[18] = 65;
    let d = Document::parse(playlist_fixture(&records, b"20.8.4.2576\0", 0)).unwrap();
    let imported = d.import().unwrap();
    assert!(imported.project.clips.is_empty());
    assert!(imported.warnings.iter().any(|w| w.contains("1 clips")));
    assert_eq!(
        d.update_notes_and_tempo(&imported.project).unwrap(),
        d.original()
    );
    records[18] = 64;
    records[24..28].copy_from_slice(&96u32.to_le_bytes());
    assert!(
        Document::parse(playlist_fixture(&records, b"20.8.4.2576\0", 0))
            .unwrap()
            .import()
            .unwrap()
            .project
            .clips
            .is_empty()
    );
    let imported = Document::parse(playlist_fixture(&records, b"24.2.0\0", 0))
        .unwrap()
        .import()
        .unwrap();
    assert!(imported.warnings.iter().any(|w| w.contains("sin validar")));
    assert!(!imported.project.clips.is_empty());
}
#[test]
fn malformed_playlist_lengths_fail_without_rewriting_the_original() {
    let bytes = playlist_fixture(&[0; 31], b"20.8.4.2576\0", 0);
    let d = Document::parse(bytes.clone()).unwrap();
    assert!(d.import().is_err());
    assert_eq!(d.rewrite(&BTreeMap::new()).unwrap(), bytes);
}
