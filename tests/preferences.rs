use garrido_studio::{
    preferences::{self, Settings, Store},
    storage,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        Self(fs::canonicalize(std::env::temp_dir()).unwrap().join(
            format!("garrido-settings-{}-{}",
            std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()),
        ))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn explicit_light_audio_and_unicode_recents_roundtrip_with_bounded_mru() {
    let mut settings = Settings {
        light: true,
        rate: Some(96000),
        buffer: Some(256),
        input_pair: 3,
        all_inputs: true,
        input: Some("test:input".into()),
        output: Some("test:output".into()),
        ..Default::default()
    };
    let root = fs::canonicalize(std::env::temp_dir()).unwrap();
    for i in 0..20 {
        settings
            .remember(&root.join(format!("Sesión Ñ {i}.gstudio")))
            .unwrap();
    }
    assert_eq!(settings.recent.len(), 16);
    let recent = settings.recent[3].clone();
    settings.remember(&recent).unwrap();
    assert_eq!(settings.recent[0], recent);
    assert_eq!(
        preferences::decode(&preferences::encode(&settings).unwrap()).unwrap(),
        settings
    );
    assert!(!Settings::default().light);
    let before = settings.clone();
    assert!(
        settings
            .remember(std::path::Path::new("relative.gstudio"))
            .is_err()
    );
    assert_eq!(settings, before);
}
#[test]
fn truncated_corrupted_and_unbounded_settings_fail_closed() {
    let bytes = preferences::encode(&Settings::default()).unwrap();
    for end in 0..bytes.len() {
        assert!(preferences::decode(&bytes[..end]).is_err());
    }
    for i in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[i] ^= 1;
        assert!(preferences::decode(&changed).is_err());
    }
    let mut invalid = Settings {
        input: Some("x".repeat(1025)),
        ..Default::default()
    };
    assert!(preferences::encode(&invalid).is_err());
    invalid.input = None;
    invalid.input_pair = 16;
    assert!(preferences::encode(&invalid).is_err());
    invalid.input_pair = 0;
    invalid.buffer = Some(1);
    assert!(preferences::encode(&invalid).is_err());
}
#[test]
fn owned_settings_replace_atomically_and_preserve_corrupted_files() {
    let temp = Temp::new();
    let store = Store::open(&temp.0).unwrap();
    assert_eq!(store.load().unwrap(), Settings::default());
    let settings = Settings {
        light: true,
        ..Default::default()
    };
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap(), settings);
    store.save(&Settings::default()).unwrap();
    assert_eq!(store.load().unwrap(), Settings::default());
    let file = temp.0.join("preferences.gprefs");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    fs::write(&file, b"keep-corrupted-original").unwrap();
    assert!(store.load().is_err());
    assert!(store.save(&settings).is_err());
    assert_eq!(storage::read(&file).unwrap(), b"keep-corrupted-original");
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn preference_links_never_replace_their_target() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    let folder = temp.0.join("settings");
    let store = Store::open(&folder).unwrap();
    let original = temp.0.join("original");
    fs::write(&original, b"music").unwrap();
    symlink(&original, folder.join("preferences.gprefs")).unwrap();
    assert!(store.save(&Settings::default()).is_err());
    assert_eq!(fs::read(&original).unwrap(), b"music");
    symlink(&folder, temp.0.join("linked")).unwrap();
    assert!(Store::open(&temp.0.join("linked")).is_err());
}
