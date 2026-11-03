from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import re
import sys
import ssl
import tomllib
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "legal/desktop-dependencies"
FETCH = "--fetch" in sys.argv[1:]


def download(url, limit=256 * 1024):
    request = urllib.request.Request(url, headers={"User-Agent": "Garrido-Studio-license-inventory"})
    certificates=Path("/etc/ssl/cert.pem")
    context=ssl.create_default_context(cafile=str(certificates)) if certificates.is_file() else ssl.create_default_context()
    with urllib.request.urlopen(request, timeout=20, context=context) as response:
        if not response.url.startswith("https://"):
            raise ValueError("Origen de licencia no seguro")
        data = response.read(limit + 1)
        if len(data) > limit:
            raise ValueError("Licencia fuera de límites")
        data.decode("utf-8")
        return data


def upstream(group):
    repository, commit = group
    folder = OUTPUT / "upstream" / repository.replace("/", "_") / commit
    if FETCH and not (folder / "provenance.json").exists():
        try:
            tree = json.loads(download(f"https://api.github.com/repos/{repository}/git/trees/{commit}"))
        except urllib.error.HTTPError as error:
            if error.code==404:
                print(f"Revisión no accesible: {repository} {commit}",flush=True)
                return None
            raise
        if not re.fullmatch(r"[0-9a-f]{40}", tree.get("sha", "")):
            raise ValueError("Árbol de licencias no válido")
        files = []
        for item in tree["tree"]:
            name = item["path"]
            if item["type"] != "blob" or not re.fullmatch(r"(?i)(?:licen[cs]e|copying|copyright|notice)[A-Za-z0-9_.-]*", name):
                continue
            url = f"https://raw.githubusercontent.com/{repository}/{commit}/{name}"
            data = download(url)
            blob=hashlib.sha1(b"blob "+str(len(data)).encode()+b"\0"+data).hexdigest()
            if blob!=item["sha"]:
                raise ValueError("La licencia no coincide con el árbol de Git")
            files.append({"name": name, "url": url, "sha256": hashlib.sha256(data).hexdigest()})
            folder.mkdir(parents=True, exist_ok=True)
            (folder / name).write_bytes(data)
        if files:
            (folder / "provenance.json").write_text(json.dumps(files, indent=2) + "\n")
    metadata = folder / "provenance.json"
    if metadata.is_file():
        files = json.loads(metadata.read_text())
        for item in files:
            if not re.fullmatch(r"[A-Za-z0-9_.-]+", item["name"]):
                raise ValueError("Nombre de licencia no válido")
            if hashlib.sha256((folder / item["name"]).read_bytes()).hexdigest() != item["sha256"]:
                raise ValueError("La licencia ha cambiado")
        return str(metadata.relative_to(ROOT))
    return None


def main():
    if OUTPUT.is_symlink():
        raise SystemExit("La carpeta de licencias no puede ser un enlace")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    lock = ROOT / "apps/desktop/Cargo.lock"
    packages = tomllib.loads(lock.read_text())["package"]
    sources = Path.home() / ".cargo/registry/src"
    records, groups = [], set()
    for package in packages:
        if "source" not in package:
            continue
        if package["source"] != "registry+https://github.com/rust-lang/crates.io-index":
            raise ValueError("Origen no reconocido")
        name, version = package["name"], package["version"]
        if not re.fullmatch(r"[A-Za-z0-9_.+-]+", name + version):
            raise ValueError("Nombre de paquete no válido")
        roots = list(sources.glob(f"*/{name}-{version}"))
        if len(roots) != 1:
            raise ValueError(f"Fuente no disponible: {name} {version}")
        root = roots[0]
        manifest = tomllib.loads((root / "Cargo.toml").read_text())["package"]
        folder = OUTPUT / "registry" / f"{name}-{version}"
        files = []
        for source in root.iterdir():
            if not source.is_file() or source.is_symlink() or not re.match(r"(?i)^(licen[cs]e|copying|copyright|notice|authors)", source.name):
                continue
            data = source.read_bytes()
            if len(data) > 256 * 1024:
                raise ValueError("Texto de licencia demasiado grande")
            data.decode("utf-8")
            folder.mkdir(parents=True, exist_ok=True)
            destination = folder / source.name
            destination.write_bytes(data)
            files.append({"file": str(destination.relative_to(ROOT)), "sha256": hashlib.sha256(data).hexdigest()})
        vcs = root / ".cargo_vcs_info.json"
        commit = json.loads(vcs.read_text()).get("git", {}).get("sha1") if vcs.is_file() else None
        repository = manifest.get("repository", "")
        match = re.match(r"https?://github.com/([^/]+/[^/]+)", repository)
        group = (match[1].removesuffix(".git"), commit) if match and commit and re.fullmatch(r"[0-9a-f]{40}", commit) else None
        if not files and group:
            if group==("emilk/egui","7c0b40402271e8d40cda69f87cf3c2617c2e4382"):
                group=("emilk/egui","841a328ba2dd8f6996beb7189ebb7fc00e668a2a")
            groups.add(group)
        records.append({"name": name, "version": version, "registry_checksum": package["checksum"], "license": manifest.get("license"), "repository": repository, "commit": commit, "files": files, "upstream_group": group})
    with ThreadPoolExecutor(max_workers=4) as pool:
        results = dict(zip(sorted(groups), pool.map(upstream, sorted(groups))))
    missing = []
    for record in records:
        group = record.pop("upstream_group")
        record["upstream_notice"] = results.get(group) if group else None
        if not record["files"] and not record["upstream_notice"]:
            missing.append(f"{record['name']}@{record['version']}")
    report = {"lock_sha256": hashlib.sha256(lock.read_bytes()).hexdigest(), "packages": records, "missing_notices": missing}
    (OUTPUT / "inventory.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{len(records)} paquetes; {len(missing)} sin texto de licencia localizado.")
    if missing:
        print("\n".join(missing))
        raise SystemExit(1)


if __name__ == "__main__":
    main()
