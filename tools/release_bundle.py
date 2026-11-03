from pathlib import Path
import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import tomllib

from dev import ROOT, own_directory


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--folder", type=Path)
    options = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    system = platform.system()
    key = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}[system]
    architecture = platform.machine().lower()
    folder = options.folder or ROOT / "dist/desktop" / f"{key}-{architecture}"
    folder = folder.resolve()
    folder.relative_to(ROOT / "dist/desktop")
    build = json.loads((folder / "build.json").read_text(encoding="utf-8"))
    if build.get("version") != version or build.get("internal_preview") is not False:
        raise SystemExit("Se necesita un paquete de la versión de publicación")
    key = build["platform"]
    architecture = build["architecture"]
    if key == "macos":
        if architecture not in ("arm64", "aarch64"):
            raise SystemExit("La publicación macOS actual requiere Apple Silicon")
        label, extension = "macOS-arm64", "zip"
    else:
        if architecture not in ("x86_64", "amd64"):
            raise SystemExit("La publicación Windows/Linux actual requiere x64")
        label = "Windows-x64" if key == "windows" else "Linux-x64"
        extension = "zip" if key == "windows" else "tar.gz"
    name = f"Garrido-Studio-{version}-{label}"
    subprocess.run([sys.executable, str(ROOT / "tools/archive_preview.py"),
                    folder.relative_to(ROOT).as_posix(), "--name", name,
                    "--format", extension, "--release"], cwd=ROOT, check=True)
    archive = folder.parent / f"{name}.{extension}"
    output = ROOT / "dist/release-assets"
    own_directory(output)
    for source in (archive, archive.with_name(archive.name + ".json")):
        destination = output / source.name
        if destination.exists():
            if destination.is_symlink() or hashlib.sha256(destination.read_bytes()).digest() != hashlib.sha256(source.read_bytes()).digest():
                raise SystemExit("Ya existe un archivo de publicación distinto")
            continue
        with source.open("rb") as input_stream, destination.open("xb") as output_stream:
            shutil.copyfileobj(input_stream, output_stream)
    print(archive.name)


if __name__ == "__main__":
    main()
