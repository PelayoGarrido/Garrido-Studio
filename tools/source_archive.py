from pathlib import Path, PurePosixPath
import argparse
from datetime import datetime
import hashlib
import json
import re
import shutil
import subprocess
import tarfile

from dev import ROOT, generated, own_directory


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", default="")
    parser.add_argument("--date", default=datetime.now().astimezone().strftime("%Y%m%d"))
    options = parser.parse_args()
    if not re.fullmatch(r"[0-9]{8}", options.date):
        raise SystemExit("Fecha de copia no válida")
    try:
        datetime.strptime(options.date, "%Y%m%d")
    except ValueError:
        raise SystemExit("Fecha de copia no válida")
    if options.revision and not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,31}", options.revision):
        raise SystemExit("Nombre de revisión no válido")
    if shutil.disk_usage(ROOT).free < 3 * 1024**3:
        raise SystemExit("Se conservan 3 GiB de reserva")
    listing = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                             cwd=ROOT, capture_output=True, check=True).stdout
    names = sorted(set(name.decode("utf-8") for name in listing.split(b"\0") if name))
    records = []
    total = 0
    for name in names:
        relative = PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit("Ruta fuera del proyecto")
        if relative.name.startswith(".env") or relative.suffix.lower() in (".pem", ".key", ".p12", ".pfx"):
            raise SystemExit("El archivo contiene un fichero sensible")
        path = ROOT
        for part in relative.parts:
            path /= part
            if path.is_symlink():
                raise SystemExit("El archivo de fuentes no admite enlaces")
        if not path.is_file():
            raise SystemExit("Falta un fichero de fuentes")
        size = path.stat().st_size
        total += size
        if size > 32 * 1024**2 or total > 128 * 1024**2:
            raise SystemExit("La copia de fuentes supera su límite de tamaño")
        records.append({"path": name, "bytes": path.stat().st_size,
                        "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    if "fixtures/multichannel.gstudio" not in names:
        raise SystemExit("Falta la sesión necesaria para las pruebas de migración")
    folder = ROOT / "dist/source"
    own_directory(folder)
    suffix = "-" + options.revision if options.revision else ""
    target = folder / f"Garrido-Studio-source-preview-{options.date}{suffix}.tar.gz"
    pending = target.with_name(target.name + ".new")
    if target.exists() or pending.exists():
        raise SystemExit("La copia de fuentes ya existe; se conserva")
    with pending.open("xb") as output, tarfile.open(fileobj=output, mode="w:gz") as archive:
        for record in records:
            path = ROOT / record["path"]
            if hashlib.sha256(path.read_bytes()).hexdigest() != record["sha256"]:
                raise SystemExit("Las fuentes han cambiado durante la copia")
            archive.add(path, arcname="Garrido-Studio/" + record["path"], recursive=False)
    pending.rename(target)
    with tarfile.open(target) as archive:
        for member, record in zip(archive.getmembers(), records, strict=True):
            if not member.isfile():
                raise SystemExit("Entrada inesperada en la copia")
            with archive.extractfile(member) as stream:
                if hashlib.sha256(stream.read()).hexdigest() != record["sha256"]:
                    raise SystemExit("La copia no coincide con las fuentes")
    report = {"internal_preview": True, "publication_gate": "FAIL", "archive": target.name,
              "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "bytes": target.stat().st_size,
              "files": records}
    generated(folder / f"source-manifest-preview-{options.date}{suffix}.json", json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "files"}, indent=2))


if __name__ == "__main__":
    main()
