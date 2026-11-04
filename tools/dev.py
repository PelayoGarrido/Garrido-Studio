from pathlib import Path
import hashlib
import json
import os
import plistlib
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ENV = dict(os.environ)
ENV["PATH"] = str(Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / "bin") + os.pathsep + ENV.get("PATH", "")
ENV.pop("RUSTFLAGS", None)
ENV["CARGO_ENCODED_RUSTFLAGS"] = "--remap-path-prefix=" + str(ROOT) + "=."
CARGO = shutil.which("cargo", path=ENV["PATH"]) or str(Path.home() / ".cargo/bin/cargo")


def run(*args):
    subprocess.run(args, cwd=ROOT, env=ENV, check=True)


def reserve():
    free = shutil.disk_usage(ROOT).free
    print(f"Espacio libre: {free / 1024**3:.2f} GiB", flush=True)
    if free < 3 * 1024**3:
        raise SystemExit("Se conservan 3 GiB de reserva. No se inicia la operación.")


def own_directory(folder):
    folder = Path(folder).absolute()
    try:
        relative = folder.relative_to(ROOT)
    except ValueError:
        raise SystemExit("La salida debe permanecer dentro del proyecto")
    current = ROOT
    for part in relative.parts:
        if part in (".", ".."):
            raise SystemExit("Ruta de salida no válida")
        current = current / part
        if current.is_symlink() or getattr(current, "is_junction", lambda: False)():
            raise SystemExit("La salida no admite carpetas enlazadas")
        current.mkdir(exist_ok=True)
        if not current.is_dir():
            raise SystemExit("Se necesita una carpeta de salida")


def generated(path, data):
    own_directory(path.parent)
    if path.is_symlink():
        raise SystemExit("La salida no puede ser un enlace")
    pending = path.with_name(path.name + ".new")
    with pending.open("xb") as stream:
        stream.write(data.encode("utf-8") if isinstance(data, str) else data)
    os.replace(pending, path)


def icon(folder):
    brand_assets()
    source = ROOT / "assets/brand/logo-1024.png"
    iconset = folder / "Garrido.iconset"
    own_directory(iconset)
    for base in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            size = base * scale
            destination = iconset / f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
            if destination.is_symlink():
                raise SystemExit("El icono no admite enlaces")
            subprocess.run(["/usr/bin/sips", "--resampleHeightWidth", str(size), str(size),
                            str(source), "--out", str(destination)], cwd=ROOT, check=True, capture_output=True)
    if (folder / "Garrido.icns").is_symlink():
        raise SystemExit("El icono no admite enlaces")
    run("/usr/bin/iconutil", "-c", "icns", str(iconset), "-o", str(folder / "Garrido.icns"))


def brand_assets():
    source = ROOT / "assets/brand"
    expected = {"logo-official.jpeg", "moodboard-approved.png", "logo-128.png", "logo-256.png",
                "logo-1024.png", "logo-128.rgba", "Garrido.ico"}
    seen = set()
    for line in (source / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        if name not in expected or name in seen:
            raise SystemExit("Manifiesto de marca no válido")
        path = source / name
        if path.is_symlink() or path.stat().st_size > 10 * 1024**2:
            raise SystemExit("Recurso de marca no válido")
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise SystemExit("El logo o el moodboard aprobado ha cambiado")
        seen.add(name)
    if seen != expected or (source / "logo-128.rgba").stat().st_size != 65536:
        raise SystemExit("Faltan recursos de marca")


def font_assets():
    source = ROOT / "assets/fonts"
    expected = {"Cuprum.ttf", "Tabler.ttf", "Cuprum-OFL.txt", "Tabler-LICENSE.txt"}
    files = {}
    for line in (source / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        if name not in expected or name in files:
            raise SystemExit("Manifiesto de fuentes no válido")
        path = source / name
        if path.is_symlink() or path.stat().st_size > 3 * 1024**2:
            raise SystemExit("Archivo de fuente no válido")
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise SystemExit("La fuente no coincide con su huella auditada")
        files[name] = data
    if set(files) != expected:
        raise SystemExit("Faltan fuentes o licencias")
    return files


def stage_fonts(destination):
    source = ROOT / "assets/fonts"
    if destination.is_symlink() or not destination.resolve().is_relative_to(ROOT.resolve()):
        raise SystemExit("La carpeta de fuentes debe estar dentro del proyecto")
    own_directory(destination)
    for name, data in font_assets().items():
        pending = destination / (name + ".new")
        with pending.open("xb") as stream:
            stream.write(data)
        os.replace(pending, destination / name)
    shutil.copy2(source / "provenance.json", destination / "provenance.json")
    shutil.copy2(source / "SHA256SUMS", destination / "SHA256SUMS")


def bundle():
    reserve()
    run(CARGO, "build", "--workspace", "--release", "--offline", "--locked")
    dist = ROOT / "dist"
    if dist.is_symlink():
        raise SystemExit("La carpeta dist no puede ser un enlace")
    app = dist / "Garrido Studio.app"
    if app.is_symlink():
        raise SystemExit("El bundle no puede ser un enlace")
    binary = app / "Contents/MacOS"
    resources = app / "Contents/Resources"
    binary.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    stage_fonts(resources / "fonts")
    stage_fonts(ROOT / "target/release/fonts")
    staged_binary = binary / "garrido-studio.new"
    if staged_binary.exists() or staged_binary.is_symlink():
        raise SystemExit("Hay un binario temporal pendiente. Revisarlo antes de continuar.")
    shutil.copy2(ROOT / "target/release/garrido-studio", staged_binary)
    os.replace(staged_binary, binary / "garrido-studio")
    for name in ("LICENSE", "NOTICE", "COMMERCIAL-MUSIC-PERMISSION.md", "THIRD-PARTY-NOTICES.md"):
        if (ROOT / name).is_file():
            shutil.copy2(ROOT / name, resources / name)
    shutil.copytree(ROOT / "legal", resources / "legal", dirs_exist_ok=True)
    clap = app / "Contents/PlugIns/GarridoNative.clap/Contents"
    (clap / "MacOS").mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "target/release/libgarrido_native.dylib", clap / "MacOS/GarridoNative")
    (clap / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable":"GarridoNative","CFBundleIdentifier":"studio.garrido.Native","CFBundlePackageType":"BNDL","CFBundleName":"Garrido Native","CFBundleVersion":"2"},fmt=plistlib.FMT_XML))
    run("/usr/bin/codesign","--force","--sign","-","--timestamp=none",str(clap.parent))
    plugin_hash=hashlib.sha256((clap / "MacOS/GarridoNative").read_bytes()).hexdigest()+"\n"
    (resources / "garrido-native.sha256").write_text(plugin_hash)
    (ROOT / "target/release/garrido-native.sha256").write_text(plugin_hash)
    icon(resources)
    info = {
        "CFBundleName": "Garrido Studio", "CFBundleDisplayName": "Garrido Studio",
        "CFBundleExecutable": "garrido-studio", "CFBundleIdentifier": "studio.garrido.GarridoStudio",
        "CFBundlePackageType": "APPL", "CFBundleShortVersionString": "1.2.0", "CFBundleVersion": "1",
        "CFBundleIconFile": "Garrido.icns", "LSMinimumSystemVersion": "13.0",
        "LSApplicationCategoryType": "public.app-category.music", "NSHighResolutionCapable": True,
        "NSHumanReadableCopyright": "Copyright © 2026 Pelayo Garrido Martínez",
        "NSSupportsAutomaticTermination": False,
        "NSMicrophoneUsageDescription": "Garrido Studio graba audio solo cuando pulsas Record. Las tomas se guardan en tu sesión local.",
    }
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    run("/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", str(app))
    run("/usr/bin/codesign", "--verify", "--deep", "--strict", str(app))
    digest = hashlib.sha256((binary / "garrido-studio").read_bytes()).hexdigest()
    (dist / "SHA256SUMS").write_text(f"{digest}  Garrido Studio.app/Contents/MacOS/garrido-studio\n")
    print(app)


def audit():
    result = subprocess.run([CARGO, "metadata", "--offline", "--locked", "--format-version=1", "--no-deps"], cwd=ROOT, env=ENV, text=True, capture_output=True, check=True)
    metadata = json.loads(result.stdout)
    packages = metadata["packages"]
    for package in packages:
        if package.get("source") is not None or not Path(package["manifest_path"]).resolve().is_relative_to(ROOT):
            raise SystemExit("Dependencia externa sin auditar")
        for dependency in package["dependencies"]:
            path = dependency.get("path")
            if dependency.get("source") is not None or not path or not Path(path).resolve().is_relative_to(ROOT):
                raise SystemExit("Dependencia externa sin auditar")
    print(f"Dependencias externas de Cargo: 0. Paquetes propios: {len(packages)}.")
    print(f"Fuentes y licencias: {len(font_assets())} huellas SHA256 verificadas.")
    brand_assets()
    print("Logo oficial, moodboard aprobado y derivados: 7 huellas verificadas.")
    print("Esto no certifica ausencia de fallos en Rust, macOS ni en el código propio.")
    print("Revisión de amenazas y límites: docs/SECURITY.md")


def main():
    os.chdir(ROOT)
    command = sys.argv[1] if len(sys.argv) == 2 else "check"
    if command == "audit":
        audit()
    elif command == "bundle":
        bundle()
    else:
        reserve()
        if command == "format":
            run(CARGO, "fmt")
        elif command == "check":
            run(CARGO, "fmt", "--", "--check")
            run(CARGO, "clippy", "--offline", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings")
            run(CARGO, "test", "--workspace", "--offline", "--locked", "--", "--test-threads=2")
            audit()
        elif command == "test":
            run(CARGO, "test", "--workspace", "--offline", "--locked", "--", "--test-threads=2")
        elif command == "build":
            run(CARGO, "build", "--workspace", "--release", "--offline", "--locked")
            stage_fonts(ROOT / "target/release/fonts")
        elif command == "start":
            stage_fonts(ROOT / "target/release/fonts")
            run(CARGO, "run", "--offline", "--locked", "--release")
        elif command == "bench":
            run(CARGO, "run", "--offline", "--locked", "--release", "--", "bench")
        else:
            raise SystemExit("Comando no reconocido")


if __name__ == "__main__":
    main()
