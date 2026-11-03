use garrido_studio::{
    codec,
    engine::Engine,
    model::*,
    routing::{Graph, Route},
};
use std::sync::{Arc, atomic::AtomicBool};

fn session() -> Project {
    let mut p = Project::empty();
    p.rate = 8000;
    p.bars = 1;
    p.master = 0.5;
    p.add_track(Instrument::Sampler).unwrap();
    p.add_track(Instrument::Trio).unwrap();
    for t in &mut p.tracks {
        t.gain = 1.;
        t.pan = -1.;
    }
    p.tracks[0].sample = Some(Sample {
        rate: 8000,
        frames: Arc::new(vec![[0.1, 0.03]; 4000]),
    });
    p.patterns[0].notes.push(Note::new(0, PPQ, 60, 1.));
    p
}
fn route(from: usize, to: Option<usize>) -> Route {
    Route {
        from,
        to,
        gain: 1.,
        invert: false,
        swap: false,
    }
}
fn render(p: &Project) -> Vec<[f32; 2]> {
    Engine::new(p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap()
        .audio
}
#[test]
fn cycles_duplicates_and_invalid_destinations_are_rejected() {
    let mut p = session();
    p.routes = vec![route(0, Some(1)), route(1, Some(0))];
    assert!(p.validate().is_err());
    p.routes = vec![route(0, Some(0))];
    assert!(p.validate().is_err());
    p.routes = vec![route(0, Some(2))];
    assert!(p.validate().is_err());
    p.routes = vec![route(0, None), route(0, None)];
    assert!(p.validate().is_err());
    p.routes = vec![Route {
        gain: f32::NAN,
        ..route(0, None)
    }];
    assert!(p.validate().is_err());
}
#[test]
fn bus_receives_audio_before_processing_and_roundtrips() {
    let mut p = session();
    let dry = render(&p);
    p.routes = vec![route(0, Some(1))];
    assert_eq!(Graph::compile(&p).unwrap().order, vec![0, 1]);
    let bus = render(&p);
    assert_eq!(dry, bus);
    assert_eq!(p, codec::decode(&codec::encode(&p).unwrap()).unwrap());
    p.tracks[1].gain = 0.;
    assert!(render(&p).iter().flatten().all(|x| *x == 0.));
}
#[test]
fn post_fader_sends_polarity_and_solo_are_explicit() {
    let mut p = session();
    let dry = render(&p);
    p.routes = vec![Route {
        invert: true,
        ..route(0, None)
    }];
    let inverted = render(&p);
    assert!(
        dry.iter()
            .zip(&inverted)
            .all(|(a, b)| (a[0] + b[0]).abs() < 1e-6)
    );
    p.routes = vec![route(0, Some(1))];
    p.tracks[0].solo = true;
    assert_eq!(dry, render(&p));
    p.tracks[1].mute = true;
    assert!(render(&p).iter().flatten().all(|x| *x == 0.));
}

#[test]
fn muted_sources_do_not_consume_or_steal_solo_voices() {
    let mut p = session();
    p.tracks[0].solo = true;
    p.patterns[1].notes = (0..100).map(|i| Note::new(0, PPQ, 20 + i, 0.1)).collect();
    let out = Engine::new(&p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(out.peak_voices, 1);
    assert_eq!(out.stolen_voices, 0);
    p.tracks[0].solo = false;
    p.tracks[1].mute = true;
    let out = Engine::new(&p, false, 0.)
        .unwrap()
        .render(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(out.peak_voices, 1);
    assert_eq!(out.stolen_voices, 0);
}
