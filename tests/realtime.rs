use garrido_studio::{engine::Engine, model::Project};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Allocator;
thread_local! {static COUNT:Cell<Option<usize>>=const {Cell::new(None)};}
fn count() {
    let _ = COUNT.try_with(|c| {
        if let Some(n) = c.get() {
            c.set(Some(n + 1));
        }
    });
}

// SAFETY: se conservan el contrato y los punteros del allocador del sistema.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: se delega el mismo Layout sin modificar.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: puntero y Layout proceden de System.
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: puntero, Layout y tamaño mantienen el contrato original.
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn processing_blocks_does_not_allocate() {
    let mut project = Project::demo();
    project.routes.push(garrido_studio::routing::Route {
        from: 0,
        to: Some(1),
        gain: 0.25,
        invert: false,
        swap: false,
    });
    garrido_studio::arrangement::add_audio(
        &mut project,
        garrido_studio::model::Sample {
            rate: 11025,
            frames: std::sync::Arc::new(vec![[0.01; 2]; 11025]),
        },
        "Take",
        0,
        7,
    )
    .unwrap();
    let mut engine = Engine::streaming(&project).unwrap().with_metronome();
    let mut block = [[0.; 2]; 512];
    COUNT.with(|c| c.set(Some(0)));
    while engine.position() < engine.frames() {
        engine.process(&mut block);
    }
    let allocations = COUNT.with(|c| c.replace(None).unwrap_or(usize::MAX));
    assert_eq!(allocations, 0);
}

#[test]
fn audio_pipe_does_not_allocate_while_transferring_blocks() {
    let (mut tx, mut rx) = garrido_studio::stream::pipe(1024).unwrap();
    let input = [[0.125, -0.25]; 512];
    let mut output = [[0.; 2]; 512];
    COUNT.with(|c| c.set(Some(0)));
    for _ in 0..1000 {
        assert_eq!(tx.push(&input), 512);
        assert_eq!(rx.pop(&mut output), 512);
    }
    let allocations = COUNT.with(|c| c.replace(None).unwrap_or(usize::MAX));
    assert_eq!(allocations, 0);
    assert_eq!(output, input);
}

#[test]
fn simultaneous_recording_producers_do_not_allocate_in_the_callback() {
    let folder = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("garrido-record-alloc-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let (mut input, recording) =
        garrido_studio::recording::Recording::start(&folder, 48000, 4, 1024).unwrap();
    let frames = [0.25; 8192];
    COUNT.with(|c| c.set(Some(0)));
    assert_eq!(input.push(&frames), 1024);
    let allocations = COUNT.with(|c| c.replace(None).unwrap_or(usize::MAX));
    assert_eq!(allocations, 0);
    drop(input);
    let report = recording.finish().unwrap();
    assert!(!report.interrupted);
    assert_eq!(report.takes.len(), 4);
    std::fs::remove_dir_all(folder).unwrap();
}
