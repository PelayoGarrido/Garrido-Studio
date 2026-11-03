from pathlib import Path
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import re
import shutil
import subprocess
import tomllib
from audit_policy import assess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--db", type=Path, default=ROOT / "outputs/rustsec-db")
    parser.add_argument("--strict", action="store_true")
    options = parser.parse_args()
    lock = ROOT / "apps/desktop/Cargo.lock"
    packages = tomllib.loads(lock.read_text(encoding="utf-8"))["package"]
    registry = []
    for package in packages:
        if "source" not in package:
            continue
        if package["source"] != "registry+https://github.com/rust-lang/crates.io-index":
            raise SystemExit("Origen de dependencia pendiente de revisión")
        if not re.fullmatch(r"[0-9a-f]{64}", package.get("checksum", "")):
            raise SystemExit("Dependencia sin checksum")
        registry.append(package)
    bundled = ROOT / ".toolchain/bin/cargo-audit"
    executable = str(bundled) if bundled.is_file() else shutil.which("cargo-audit")
    if not executable:
        raise SystemExit("Falta cargo-audit oficial. Consulta docs/DEPENDENCIES.md.")
    args = [executable, "audit", "--file", str(lock), "--deny", "warnings", "--json", "--quiet"]
    database = options.db.resolve()
    if database.is_dir():
        args.extend(["--db", str(database), "--no-fetch"])
    env = dict(os.environ)
    env["PATH"] = str(Path.home() / ".cargo/bin") + os.pathsep + env.get("PATH", "")
    result = subprocess.run(args, cwd=ROOT, env=env, capture_output=True, text=True, timeout=300)
    if result.stderr:
        print(result.stderr, end="")
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        raise SystemExit("RustSec no ha producido un informe válido")
    output = ROOT / "outputs"
    output.mkdir(exist_ok=True)
    (output / "desktop-official-audit.json").write_text(json.dumps(report, indent=2) + "\n")
    (output / "desktop-official-audit-stderr.txt").write_text(result.stderr)
    policy = {} if options.strict else tomllib.loads((ROOT / "docs/security-maintenance.toml").read_text(encoding="utf-8"))
    decision = assess(report, policy, datetime.now(timezone.utc).date())
    passed = decision["passed"] and result.returncode in (0, 1) and not result.stderr.strip()
    summary = {
        "checked_at": datetime.now(timezone.utc).isoformat(),
        "advisory_database": str(database),
        "lock_sha256": hashlib.sha256(lock.read_bytes()).hexdigest(),
        "registry_packages": len(registry),
        "known_vulnerabilities": report["vulnerabilities"]["count"],
        "warnings": {key: len(value) for key, value in report["warnings"].items()},
        "passed": passed,
        "official_strict_passed": result.returncode == 0,
        "scanner_diagnostics_present": bool(result.stderr.strip()),
        "policy": "strict" if options.strict else "reviewed-maintenance-only",
        "reviewed_warnings": decision["reviewed_warnings"],
        "unreviewed_warnings": decision["unreviewed_warnings"],
    }
    (output / "desktop-dependency-audit.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
