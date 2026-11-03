use garrido_studio::{engine::Engine, model::Project, stream::pipe};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[test]
fn variable_blocks_and_concurrent_producer_preserve_the_render() {
    let mut project = Project::demo();
    project.rate = 8000;
    project.bars = 1;
    for clip in &mut project.clips {
        clip.length = 3840;
    }
    project.markers.clear();
    let expected = Engine::new(&project, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio;
    let mut engine = Engine::streaming(&project).unwrap();
    let (mut tx, mut rx) = pipe(1024).unwrap();
    let length = expected.len();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let producer = thread::spawn(move || {
        let mut block = [[0.; 2]; 137];
        while engine.position() < engine.frames() && !worker_stop.load(Ordering::Relaxed) {
            if tx.available() < block.len() {
                thread::yield_now();
                continue;
            }
            let count = engine.process(&mut block);
            assert_eq!(tx.push(&block[..count]), count);
        }
    });
    let mut actual = Vec::with_capacity(length);
    let mut block = [[0.; 2]; 199];
    let start = Instant::now();
    while actual.len() < length {
        let count = rx.pop(&mut block);
        actual.extend_from_slice(&block[..count]);
        if count == 0 {
            if start.elapsed() > Duration::from_secs(5) {
                break;
            }
            thread::yield_now();
        }
    }
    stop.store(true, Ordering::Relaxed);
    producer.join().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn long_songs_use_fixed_blocks_and_refuse_a_whole_song_allocation() {
    let mut p = Project::empty();
    p.rate = 192000;
    p.bars = 256;
    p.add_track(garrido_studio::model::Instrument::Trio)
        .unwrap();
    p.patterns[0]
        .notes
        .push(garrido_studio::model::Note::new(0, 960, 60, 0.1));
    assert!(Engine::new(&p, false, 0.).is_err());
    let mut engine = Engine::streaming(&p).unwrap();
    assert_eq!(engine.frames(), 98_304_000);
    let mut block = [[0.; 2]; 512];
    assert_eq!(engine.process(&mut block), 512);
    assert!(block.iter().flatten().all(|value| value.is_finite()));
    assert!(block.iter().flatten().any(|value| value.abs() > 0.00001));
    assert!(engine.render(&AtomicBool::new(false)).is_err());
}
