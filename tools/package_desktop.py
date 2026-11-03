from pathlib import Path
import argparse
import hashlib
import json
import os
import platform
import plistlib
import re
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
from dev import generated, own_directory


def copy(source, destination):
    if source.is_symlink() or destination.is_symlink():
        raise SystemExit("El empaquetado no admite enlaces")
    own_directory(destination.parent)
    pending = destination.with_name(destination.name + ".new")
    with source.open("rb") as input_stream, pending.open("xb") as output_stream:
        shutil.copyfileobj(input_stream, output_stream)
    shutil.copystat(source, pending)
    os.replace(pending, destination)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", default="")
    parser.add_argument("--check-plugins", action="store_true")
    parser.add_argument("--release", action="store_true")
    cross = parser.add_mutually_exclusive_group()
    cross.add_argument("--from-windows-cross", action="store_true")
    cross.add_argument("--from-linux-cross", action="store_true")
    options = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    revision = subprocess.run(["git", "rev-parse", "--verify", "HEAD"], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=10)
    commit = revision.stdout.strip() if revision.returncode == 0 else None
    if options.release:
        if commit is None or not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise SystemExit("La publicación necesita una revisión Git identificable")
        status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, capture_output=True, timeout=10, check=True)
        if status.stdout:
            raise SystemExit("Guarda los cambios de fuentes en Git antes de empaquetar la publicación")
        if os.environ.get("GITHUB_SHA", commit) != commit:
            raise SystemExit("La revisión del workflow no coincide con las fuentes")
    if options.check_plugins and (options.from_windows_cross or options.from_linux_cross):
        raise SystemExit("El diagnóstico solo se ejecuta en el sistema nativo del paquete")
    if options.variant and not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,39}", options.variant):
        raise SystemExit("Nombre de variante no válido")
    if shutil.disk_usage(ROOT).free < 3 * 1024**3:
        raise SystemExit("Se conservan 3 GiB de reserva")
    from dev import font_assets, brand_assets
    font_assets()
    brand_assets()
    system = platform.system()
    architecture = platform.machine().lower()
    build_folder = ROOT / "target/desktop/release"
    plugin_folder = ROOT / "target/release"
    if options.from_windows_cross:
        system = "Windows"
        architecture = "x86_64"
        build_folder = ROOT / "target/windows-build/x86_64-pc-windows-gnullvm/release"
        plugin_folder = build_folder
        evidence = json.loads((ROOT / "outputs/windows-build-report.json").read_text(encoding="utf-8"))
        if options.release and evidence.get("version") != version:
            raise SystemExit("La compilación Windows no corresponde a la versión de publicación")
        for name in ("garrido-desktop.exe", "garrido_native.dll"):
            if hashlib.sha256((build_folder / name).read_bytes()).hexdigest() != evidence["files"][name]["sha256"]:
                raise SystemExit("La aplicación Windows no coincide con su compilación")
    elif options.from_linux_cross:
        system = "Linux"
        architecture = "x86_64"
        build_folder = ROOT / "target/linux-build/x86_64-unknown-linux-gnu/release"
        plugin_folder = build_folder
        evidence = json.loads((ROOT / "outputs/linux-build-report.json").read_text(encoding="utf-8"))
        if options.release and evidence.get("version") != version:
            raise SystemExit("La compilación Linux no corresponde a la versión de publicación")
        for name in ("garrido-desktop", "libgarrido_native.so"):
            if hashlib.sha256((build_folder / name).read_bytes()).hexdigest() != evidence["files"][name]["sha256"]:
                raise SystemExit("La aplicación Linux no coincide con su compilación")
    key = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}.get(system)
    if key is None:
        raise SystemExit("Sistema no compatible")
    suffix = f"-{options.variant}" if options.variant else ""
    folder = ROOT / "dist/desktop" / f"{key}-{architecture}{suffix}"
    if folder.is_symlink() or (ROOT / "dist").is_symlink() or (ROOT / "dist/desktop").is_symlink():
        raise SystemExit("La carpeta de salida no puede ser un enlace")
    suffix = ".exe" if system == "Windows" else ""
    source = build_folder / ("garrido-desktop" + suffix)
    if not source.is_file():
        raise SystemExit("Falta la compilación: pnpm run desktop:build")
    if options.release and not (options.from_windows_cross or options.from_linux_cross):
        result = subprocess.run([str(source), "--version"], capture_output=True, text=True, encoding="utf-8", timeout=10, check=True)
        if result.stdout.strip() != f"Garrido Studio {version} · Pelayo Garrido Martínez":
            raise SystemExit("El ejecutable no corresponde a la versión de publicación")
    if system == "Darwin":
        app = folder / "Garrido Studio.app"
        binary = app / "Contents/MacOS/garrido-desktop"
        resources = app / "Contents/Resources"
        copy(source, binary)
        info = {
            "CFBundleName": "Garrido Studio", "CFBundleDisplayName": "Garrido Studio",
            "CFBundleExecutable": "garrido-desktop", "CFBundleIdentifier": "studio.garrido.GarridoStudio.Desktop",
            "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version, "CFBundleVersion": "3",
            "CFBundleIconFile": "Garrido.icns", "LSMinimumSystemVersion": "13.0", "NSHighResolutionCapable": True,
            "LSApplicationCategoryType": "public.app-category.music",
            "NSHumanReadableCopyright": "Copyright © 2026 Pelayo Garrido Martínez",
            "NSMicrophoneUsageDescription": "Garrido Studio graba audio local solo cuando pulsas Record.",
        }
        generated(app / "Contents/Info.plist", plistlib.dumps(info))
        from dev import icon
        own_directory(resources)
        icon(resources)
    else:
        app = folder
        binary = app / ("GarridoStudio" + suffix)
        resources = app / "Resources"
        copy(source, binary)
        copy(ROOT / "assets/brand/logo-256.png", resources / "Garrido.png")
        copy(ROOT / "assets/brand/Garrido.ico", resources / "Garrido.ico")
    for name in ["LICENSE", "NOTICE", "COMMERCIAL-MUSIC-PERMISSION.md", "THIRD-PARTY-NOTICES.md", "CONTRIBUTING.md"]:
        copy(ROOT / name, resources / name)
    copy(ROOT / "fixtures/multichannel.gstudio", resources / "Test Session.gstudio")
    copy(ROOT / "fixtures/example.gstudio", folder / "Example.gstudio")
    copy(ROOT / "docs/PLATFORM-ACCEPTANCE.md", resources / "PLATFORM-ACCEPTANCE.md")
    if system == "Windows":
        generated(app / "Check-Plugins.cmd", '@echo off\r\nsetlocal\r\n"%~dp0GarridoStudio.exe" --check-bundled-plugins-report\r\nset "result=%errorlevel%"\r\necho Plugin-Check reports are saved beside the application.\r\necho No audio is played and no microphone is opened.\r\npause\r\nexit /b %result%\r\n')
        generated(app / "LEEME-PRUEBA.txt", "Garrido Studio · Pelayo Garrido Martínez\n\nDescomprime la carpeta completa y abre GarridoStudio.exe.\nEs una compilación de prueba; todavía no es la v1. No tiene firma de distribución.\nLa app empieza en oscuro y no activa el micrófono al abrirse.\nAbre Resources/Test Session.gstudio para comprobar edición, guardar copia y exportar.\nLa guía de pruebas está en Resources/PLATFORM-ACCEPTANCE.md.\nSi Windows bloquea la apertura, conserva el mensaje para revisarlo; no desactives protecciones.\n")
    elif system == "Linux":
        script = app / "Check-Plugins.sh"
        generated(script, '#!/bin/sh\ncd -- "$(dirname -- "$0")" || exit 1\nexec ./GarridoStudio --check-bundled-plugins-report\n')
        script.chmod(0o755)
        generated(app / "LEEME-PRUEBA.txt", "Garrido Studio · Pelayo Garrido Martínez\n\nPaquete de prueba para Linux x86_64, preparado con Ubuntu 24.04/glibc 2.39.\nDescomprime la carpeta completa y abre GarridoStudio.\nNecesita una sesión gráfica X11/Wayland, ALSA y las bibliotecas de escritorio habituales.\nTodavía no es la v1 ni se ha aceptado su ejecución en Linux.\nLa app empieza en oscuro y no activa el micrófono al abrirse.\nLa sesión y la guía de pruebas están en Resources.\nConserva cualquier mensaje de error para revisarlo.\n")
    for directory in ["legal", "assets/fonts"]:
        source_folder = ROOT / directory
        for source_file in sorted(source_folder.rglob("*")):
            if source_file.is_file():
                copy(source_file, resources / directory / source_file.relative_to(source_folder))
    plugin_source=plugin_folder / {"Darwin":"libgarrido_native.dylib","Linux":"libgarrido_native.so","Windows":"garrido_native.dll"}[system]
    if not plugin_source.is_file():
        raise SystemExit("Falta el plugin propio: pnpm run build")
    if system=="Darwin":
        plugin=app / "Contents/PlugIns/GarridoNative.clap"
        plugin_binary=plugin / "Contents/MacOS/GarridoNative"
        copy(plugin_source,plugin_binary)
        generated(plugin / "Contents/Info.plist", plistlib.dumps({"CFBundleExecutable":"GarridoNative","CFBundleIdentifier":"studio.garrido.Native","CFBundlePackageType":"BNDL","CFBundleName":"Garrido Native","CFBundleVersion":"2"}))
        subprocess.run(["/usr/bin/codesign","--force","--sign","-","--timestamp=none",str(plugin)],check=True)
    else:
        plugin_binary=app / "Plugins/GarridoNative.clap"
        copy(plugin_source,plugin_binary)
    generated(resources / "garrido-native.sha256", hashlib.sha256(plugin_binary.read_bytes()).hexdigest()+"\n")
    if options.release:
        generated(folder / "LEEME.txt", f"Garrido Studio {version} · Pelayo Garrido Martínez\n\nExtrae la carpeta completa y abre la aplicación.\nNo requiere herramientas de desarrollo ni descargas de modelos.\nLa app empieza en oscuro; entrada y salida de audio se activan mediante sus controles.\nResources contiene una sesión de ejemplo, licencias y la guía de comprobación.\nConsulta CONTRIBUTING.md para compatibilidad, trabajo pendiente y cómo informar de errores.\nmacOS: firma ad hoc, sin notarización. Windows: ejecutable sin firma de distribución.\nLinux x86_64: glibc 2.39 y bibliotecas de escritorio/audio.\nNo desactives protecciones si el sistema bloquea la apertura; conserva el mensaje.\n")
        old_readme = app / "LEEME-PRUEBA.txt"
        if old_readme.is_file() and not old_readme.is_symlink():
            old_readme.unlink()
        status = f"Garrido Studio {version}. Release scope: sequencing, native sessions, original DSP, bounded recording and audio export. See CONTRIBUTING.md and PLATFORM-ACCEPTANCE.md for compatibility and further work.\n"
    else:
        status = "Internal preview. See CONTRIBUTING.md and PLATFORM-ACCEPTANCE.md.\n"
    generated(resources / "RELEASE-STATUS.txt", status)
    if system == "Darwin":
        subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", str(app)], check=True)
        subprocess.run(["/usr/bin/codesign", "--verify", "--strict", "--deep", str(app)], check=True)
    report = {
        "platform": key, "architecture": architecture, "version": version,
        "internal_preview": not options.release, "publication_gate": "OWNER_AUTHORIZED_V1" if options.release else "PREVIEW",
        "cross_built": options.from_windows_cross or options.from_linux_cross, "runtime_accepted": False,
        "source_commit": commit,
        "application": str(app.relative_to(ROOT)), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "bytes": sum(p.stat().st_size for p in folder.rglob("*") if p.is_file()),
    }
    generated(folder / "build.json", json.dumps(report, indent=2) + "\n")
    if options.check_plugins:
        result = subprocess.run([str(binary), "--check-bundled-plugins"], capture_output=True, timeout=60, check=True)
        if len(result.stdout) > 64 * 1024 or len(result.stderr) > 64 * 1024:
            raise SystemExit("Respuesta de diagnóstico fuera de límites")
        diagnostic = json.loads(result.stdout)
        if (diagnostic.get("descriptors"), diagnostic.get("effects"), diagnostic.get("instruments")) != (14, 8, 6):
            raise SystemExit("El diagnóstico no confirma los catorce plugins")
        if diagnostic.get("audio_played") is not False or diagnostic.get("microphone_opened") is not False:
            raise SystemExit("El diagnóstico no confirma su ejecución sin dispositivos de audio")
        report["included_plugin_check"] = diagnostic
        generated(folder / "build.json", json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
