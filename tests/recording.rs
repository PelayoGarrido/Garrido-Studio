use garrido_studio::{
    recording::{self, Recording},
    storage, wav,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let folder = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "garrido-disk-recording-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&folder).unwrap();
        Self(folder)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn simultaneous_pairs_are_sample_aligned_finite_and_preserve_full_input_range() {
    let temp = Temp::new();
    let (mut input, recording) = Recording::start(&temp.0, 48000, 2, 4096).unwrap();
    for block in 0..16 {
        let samples: Vec<_> = (0..256)
            .flat_map(|i| {
                let x = (block * 256 + i) as f32 / 8192.;
                [x, -x, 0.95, f32::NAN]
            })
            .collect();
        assert_eq!(input.push(&samples), 256);
    }
    assert!(recording.done());
    assert_eq!(recording.frames(), 4096);
    drop(input);
    let report = recording.finish().unwrap();
    assert!(!report.interrupted);
    assert!(report.warning.is_none());
    assert_eq!(report.takes.len(), 2);
    let left = wav::decode(&storage::read(&report.takes[0].path).unwrap()).unwrap();
    let right = wav::decode(&storage::read(&report.takes[1].path).unwrap()).unwrap();
    assert_eq!(left.frames.len(), 4096);
    assert_eq!(right.frames.len(), 4096);
    for (i, frame) in left.frames.iter().enumerate() {
        assert_eq!(*frame, [i as f32 / 8192., -(i as f32 / 8192.)]);
    }
    assert!(right.frames.iter().all(|frame| *frame == [0.95, 0.]));
    assert!(
        report
            .takes
            .iter()
            .all(|take| take.frames == 4096 && take.rate == 48000)
    );
}
#[test]
fn overflow_keeps_the_aligned_prefix_and_reports_interruption_instead_of_skipping_audio() {
    let temp = Temp::new();
    let (mut input, recording) = Recording::start(&temp.0, 8000, 2, 20000).unwrap();
    assert_eq!(input.push(&[0.25; 65536]), 8192);
    assert_eq!(input.push(&[0.5; 8]), 0);
    drop(input);
    let report = recording.finish().unwrap();
    assert!(report.interrupted);
    for take in report.takes {
        assert_eq!(take.frames, 8192);
        let sample = wav::decode(&storage::read(&take.path).unwrap()).unwrap();
        assert!(sample.frames.iter().all(|f| *f == [0.25; 2]));
    }
}
#[test]
fn early_stop_and_malformed_callbacks_keep_received_frames_on_disk() {
    let temp = Temp::new();
    let (mut input, recording) = Recording::start(&temp.0, 8000, 1, 100).unwrap();
    assert_eq!(input.push(&[0.5, -0.5, 2., -2.]), 2);
    input.control().interrupt();
    drop(input);
    let report = recording.finish().unwrap();
    assert!(report.interrupted);
    let sample = wav::decode(&storage::read(&report.takes[0].path).unwrap()).unwrap();
    assert_eq!(&*sample.frames, &[[0.5, -0.5], [1., -1.]]);
    let (mut input, recording) = Recording::start(&temp.0, 8000, 1, 100).unwrap();
    assert_eq!(input.push(&[0.; 3]), 0);
    drop(input);
    assert!(recording.finish().unwrap().interrupted);
}
#[test]
fn interrupted_header_recovery_never_modifies_the_source() {
    let temp = Temp::new();
    let path = temp.0.join("interrupted.wav");
    let frames = [[0.25, -0.5]; 17];
    let mut bytes = wav::encode(&frames, 48000, wav::Depth::Float32, false).unwrap();
    bytes[4..8].copy_from_slice(&36u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
    bytes.extend([1, 2, 3]);
    storage::write_new(&path, &bytes).unwrap();
    let (sample, repaired) = recording::recover(&path).unwrap();
    assert!(repaired);
    assert_eq!(&*sample.frames, &frames);
    assert_eq!(storage::read(&path).unwrap(), bytes);
    assert!(Recording::start(&temp.0, 48000, 17, 10).is_err());
    assert!(Recording::start(&temp.0, 48000, 2, 4_000_000).is_err());
}
