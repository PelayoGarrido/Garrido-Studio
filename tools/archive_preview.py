import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tarfile
import zipfile

from dev import ROOT, generated


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("folder")
    parser.add_argument("--name", required=True)
    parser.add_argument("--format", choices=("zip", "tar.gz"), required=True)
    parser.add_argument("--release", action="store_true")
    options = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]{0,100}", options.name) or ".." in options.name or options.name.endswith("."):
        raise SystemExit("Nombre de paquete no válido")
    relative = Path(options.folder)
    if relative.is_absolute() or ".." in relative.parts or len(relative.parts) < 3:
        raise SystemExit("La carpeta debe estar dentro de dist/desktop o dist/qa")
    if relative.parts[:2] not in (("dist", "desktop"), ("dist", "qa")):
        raise SystemExit("Carpeta fuera de las salidas de prueba")
    folder = ROOT
    for part in relative.parts:
        folder /= part
        if folder.is_symlink() or getattr(folder, "is_junction", lambda: False)():
            raise SystemExit("El paquete no admite enlaces")
    if not folder.is_dir() or shutil.disk_usage(ROOT).free < 3 * 1024**3:
        raise SystemExit("Falta la carpeta o la reserva de 3 GiB")
    records = []
    total = 0
    for path in sorted(folder.rglob("*")):
        if path.is_symlink() or getattr(path, "is_junction", lambda: False)() or not (path.is_dir() or path.is_file()):
            raise SystemExit("Entrada no válida en el paquete")
        if path.is_file():
            size = path.stat().st_size
            total += size
            if size > 64 * 1024**2 or total > 128 * 1024**2 or len(records) >= 4096:
                raise SystemExit("El paquete supera su límite de tamaño")
            records.append({"file": path.relative_to(folder).as_posix(), "bytes": size,
                            "sha256": digest(path), "mode": path.stat().st_mode & 0o777})
    if not records:
        raise SystemExit("La carpeta está vacía")
    if shutil.disk_usage(ROOT).free < 3 * 1024**3 + total + 1024**2:
        raise SystemExit("El archivo debe conservar la reserva de 3 GiB")
    build = {}
    if options.release:
        build_path = folder / "build.json"
        if build_path.stat().st_size > 65536 or build_path.is_symlink():
            raise SystemExit("Metadatos de publicación fuera de límites")
        build = json.loads(build_path.read_text())
        if build.get("internal_preview") is not False or build.get("publication_gate") != "OWNER_AUTHORIZED_V1":
            raise SystemExit("El paquete no corresponde a una publicación autorizada")
    target = folder.parent / (options.name + "." + options.format)
    if target.exists() or target.is_symlink():
        raise SystemExit("El paquete ya existe; se conserva")
    pending = target.with_name(target.name + ".new")
    with pending.open("xb") as stream:
        if options.format == "zip":
            with zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
                for item in records:
                    archive.write(folder / item["file"], folder.name + "/" + item["file"])
        else:
            with tarfile.open(fileobj=stream, mode="w:gz") as archive:
                for item in records:
                    archive.add(folder / item["file"], arcname=folder.name + "/" + item["file"], recursive=False)
    if options.format == "zip":
        with zipfile.ZipFile(pending) as archive:
            if archive.testzip() is not None or len(archive.infolist()) != len(records):
                raise SystemExit("El ZIP no supera la comprobación")
            for entry, item in zip(archive.infolist(), records, strict=True):
                data = archive.read(entry)
                if entry.filename != folder.name + "/" + item["file"] or len(data) != item["bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
                    raise SystemExit("El ZIP no coincide con sus archivos")
    else:
        with tarfile.open(pending) as archive:
            for entry, item in zip(archive.getmembers(), records, strict=True):
                if not entry.isfile() or entry.name != folder.name + "/" + item["file"] or entry.mode & 0o777 != item["mode"]:
                    raise SystemExit("El archivo no conserva sus entradas o permisos")
                with archive.extractfile(entry) as stream:
                    data = stream.read()
                if len(data) != item["bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
                    raise SystemExit("El archivo no coincide con sus archivos")
    pending.rename(target)
    report = {"archive": target.relative_to(ROOT).as_posix(), "bytes": target.stat().st_size,
              "sha256": digest(target), "internal_preview": not options.release,
              "publication_gate": "OWNER_AUTHORIZED_V1" if options.release else "PREVIEW", "build": build,
              "readback_verified": True, "files": records}
    generated(target.with_name(target.name + ".json"), json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "files"}, indent=2))


if __name__ == "__main__":
    main()
