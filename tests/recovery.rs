use garrido_studio::{codec, model::Project, recovery::Store, storage};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(
            std::fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!("garrido-recovery-test-{}-{id}", std::process::id())),
        )
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn copies_are_private_readable_and_never_overwritten_or_pruned() {
    let temp = Temp::new();
    let store = Store::open(&temp.0).unwrap();
    let mut p = Project::demo();
    let first = store.save(&p).unwrap();
    let original = storage::read(&first).unwrap();
    for i in 1..8 {
        p.bpm = 120. + i as f64;
        store.save(&p).unwrap();
    }
    assert_eq!(store.list().unwrap().len(), 8);
    assert!(store.save(&p).is_err());
    assert_eq!(original, storage::read(&first).unwrap());
    assert_eq!(codec::decode(&original).unwrap(), Project::demo());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(first).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[cfg(windows)]
#[test]
fn recovery_accepts_both_regular_and_extended_drive_paths() {
    let temp = Temp::new();
    let store = Store::open(&temp.0).unwrap();
    let copy = store.save(&Project::demo()).unwrap();
    assert_eq!(
        codec::decode(&storage::read(&copy).unwrap()).unwrap(),
        Project::demo()
    );
    let regular = std::env::temp_dir().join(temp.0.file_name().unwrap());
    assert_eq!(Store::open(&regular).unwrap().list().unwrap().len(), 1);
}
#[cfg(unix)]
#[test]
fn recovery_rejects_symlinked_folders_and_files() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    fs::create_dir(&temp.0).unwrap();
    let folder = temp.0.join("real");
    let store = Store::open(&folder).unwrap();
    symlink(&folder, temp.0.join("link")).unwrap();
    assert!(Store::open(&temp.0.join("link")).is_err());
    let source = temp.0.join("source");
    fs::write(&source, b"keep").unwrap();
    symlink(&source, folder.join("recovery-link.gstudio")).unwrap();
    assert!(store.save(&Project::demo()).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"keep");
}
