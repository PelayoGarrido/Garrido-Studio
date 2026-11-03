use garrido_studio::{Error, history::History, model::Project};

#[test]
fn named_history_seeks_in_both_directions_and_invalid_targets_are_atomic() {
    let original = Project::starter();
    let mut h = History::new(original.clone()).unwrap();
    for (label, bpm) in [("First", 121.), ("Second", 122.), ("Third", 123.)] {
        h.edit_named(label, |p| {
            p.bpm = bpm;
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(h.labels(), ["Session opened", "First", "Second", "Third"]);
    assert!(h.seek(1).unwrap());
    assert_eq!(h.project.bpm, 121.);
    assert_eq!(h.position(), 1);
    assert!(!h.seek(1).unwrap());
    assert!(h.seek(3).unwrap());
    assert_eq!(h.project.bpm, 123.);
    assert!(h.seek(4).is_err());
    assert_eq!(h.project.bpm, 123.);
    h.seek(0).unwrap();
    assert_eq!(h.project, original);
    assert!(h.redo());
    assert_eq!(h.project.bpm, 121.);
}
#[test]
fn noops_and_failed_edits_preserve_future_but_a_new_edit_branches() {
    let mut h = History::new(Project::starter()).unwrap();
    h.edit_named("Tempo", |p| {
        p.bpm = 130.;
        Ok(())
    })
    .unwrap();
    h.undo();
    let labels: Vec<_> = h.labels().into_iter().map(str::to_owned).collect();
    h.edit_named("No change", |_| Ok(())).unwrap();
    assert!(
        h.edit_named("Invalid", |p| {
            p.bpm = 0.;
            Ok(())
        })
        .is_err()
    );
    assert!(
        h.edit_named("Failure", |_| Err(Error::Invalid("fail")))
            .is_err()
    );
    assert!(h.edit_named("", |_| Ok(())).is_err());
    assert_eq!(h.labels(), labels);
    h.edit_named("Branch", |p| {
        p.bpm = 140.;
        Ok(())
    })
    .unwrap();
    assert_eq!(h.labels(), ["Session opened", "Branch"]);
    assert!(!h.redo());
}
#[test]
fn pruning_keeps_a_bounded_and_honest_navigation_baseline() {
    let mut h = History::new(Project::starter()).unwrap();
    for i in 1..=50 {
        h.edit_named("Tempo", |p| {
            p.bpm = 100. + i as f64;
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(h.labels().len(), 33);
    assert_eq!(h.position(), 32);
    assert_eq!(h.labels()[0], "Earlier retained state");
    h.seek(0).unwrap();
    assert_eq!(h.project.bpm, 118.);
    h.seek(32).unwrap();
    assert_eq!(h.project.bpm, 150.);
}
