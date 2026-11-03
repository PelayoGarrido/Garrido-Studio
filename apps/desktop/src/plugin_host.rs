pub(crate) use garrido_plugin_host::{check_bundled, included, instrument, render, scan, worker};

pub(crate) fn report() -> garrido_studio::Result<()> {
    use garrido_studio::{Error, integrity, storage};
    use std::time::{SystemTime, UNIX_EPOCH};
    let executable = std::env::current_exe()?;
    let bytes = storage::read_bounded(&executable, 64 * 1024 * 1024)?;
    let digest: String = integrity::sha256(&bytes)
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect();
    drop(bytes);
    let result = check_bundled();
    let status = match &result {
        Ok(()) => {
            "PASS: 14 descriptors, 8 effects and 6 instruments; Gain reference verified".into()
        }
        Err(error) => format!("FAIL: {error}"),
    };
    let report = format!(
        "Garrido Studio · Pelayo Garrido Martínez\nIncluded plugin diagnostic\nPlatform: {} / {}\nApplication SHA256: {digest}\n{status}\nAudio played: false\nMicrophone opened: false\nThird-party code loaded: false\nThis check does not validate devices, third-party plugins or complete application functionality.\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Invalid("Reloj no válido"))?
        .as_nanos();
    let folder = executable
        .parent()
        .ok_or(Error::Invalid("Paquete incompleto"))?;
    let path = folder.join(format!("Plugin-Check-{stamp}.txt"));
    storage::write_new(&path, report.as_bytes())?;
    println!("{}", path.display());
    result
}
