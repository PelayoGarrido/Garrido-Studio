import argparse
import hashlib
import json
from pathlib import Path
import re

from desktop import CARGO, ENV, MANIFEST, ROOT
from cross_build import run
from dev import own_directory, generated
from package_desktop import copy


def configure(platform):
    ENV["CARGO_BUILD_JOBS"] = "1"
    ENV["ZIG_GLOBAL_CACHE_DIR"] = str(ROOT / "target/zig-cache")
    ENV["ZIG_LOCAL_CACHE_DIR"] = str(ROOT / "target/zig-local")
    if platform == "windows":
        target = "x86_64-pc-windows-gnullvm"
        ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/windows-build")
        ENV["CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER"] = str(ROOT / "tools/zig_windows_linker.py")
    else:
        target = "x86_64-unknown-linux-gnu"
        ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/linux-build")
        ENV["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"] = str(ROOT / "tools/zig_linux_linker.py")
        ENV["CC_x86_64_unknown_linux_gnu"] = str(ROOT / "tools/zig_linux_linker.py")
        ENV["AR_x86_64_unknown_linux_gnu"] = str(ROOT / "tools/zig_ar.py")
        ENV["PKG_CONFIG"] = str(ROOT / ".toolchain/pkgconf-3.0.10/pkgconf")
        ENV["PKG_CONFIG_SYSROOT_DIR"] = str(ROOT / ".toolchain/linux-sysroot")
        ENV["PKG_CONFIG_LIBDIR"] = str(ROOT / ".toolchain/linux-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig")
        ENV["PKG_CONFIG_ALLOW_CROSS"] = "1"
    return target


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("platform", choices=("windows", "linux"))
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--variant", default="")
    options = parser.parse_args()
    if options.variant and not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,39}", options.variant):
        raise SystemExit("Nombre de variante no válido")
    platform = options.platform
    target = configure(platform)
    if options.check_only:
        for manifest in (ROOT / "Cargo.toml", MANIFEST):
            run([CARGO, "clippy", "--manifest-path", str(manifest), "--workspace", "--target", target,
                 "--release", "--all-targets", "--offline", "--locked", "--", "-D", "warnings"], reserve_gib=3.5)
        print(f"Clippy estricto para núcleo, plugins y GUI: {platform}. No ejecuta binarios del destino.")
        return
    suffix = "-" + options.variant if options.variant else ""
    folder = ROOT / "dist/qa" / (platform + "-x64" + suffix)
    own_directory(folder / "Tests")
    artifacts = set()
    for label, manifest in (("core", ROOT / "Cargo.toml"), ("desktop", MANIFEST)):
        log = ROOT / f"outputs/{platform}-{label}-cross-tests.jsonl"
        with log.open("wb") as stream:
            run([CARGO, "test", "--manifest-path", str(manifest), "--workspace", "--target", target,
                 "--release", "--no-run", "--message-format=json", "--offline", "--locked"], reserve_gib=3.5, output=stream)
        if log.stat().st_size > 32 * 1024**2:
            raise SystemExit("Registro de compilación demasiado grande")
        for line in log.read_text(encoding="utf-8").splitlines():
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test") and item.get("executable"):
                artifacts.add(Path(item["executable"]))
        print(f"Pruebas {label} compiladas para {platform}; todavía no ejecutadas.", flush=True)
    records = []
    for path in sorted(artifacts):
        if not re.fullmatch(r"[a-zA-Z0-9_-]+(?:\.exe)?", path.name):
            raise SystemExit("Nombre de prueba no válido")
        path.resolve().relative_to(ROOT / "target")
        destination = folder / "Tests" / path.name
        copy(path, destination)
        records.append({"file": "Tests/" + path.name, "sha256": hashlib.sha256(destination.read_bytes()).hexdigest(), "bytes": destination.stat().st_size})
    if not records:
        raise SystemExit("No se han generado pruebas ejecutables")
    evidence = json.loads((ROOT / f"outputs/{platform}-build-report.json").read_text(encoding="utf-8"))
    if evidence["target"] != target:
        raise SystemExit("La aplicación y las pruebas tienen destinos distintos")
    application = "garrido-desktop.exe" if platform == "windows" else "garrido-desktop"
    application_sha256 = evidence["files"][application]["sha256"]
    if not re.fullmatch(r"[a-f0-9]{64}", application_sha256):
        raise SystemExit("Huella de aplicación no válida")
    for name in ("LICENSE", "NOTICE", "COMMERCIAL-MUSIC-PERMISSION.md", "THIRD-PARTY-NOTICES.md"):
        copy(ROOT / name, folder / name)
    for directory in ("legal", "assets/fonts"):
        for source in sorted((ROOT / directory).rglob("*")):
            if source.is_file():
                copy(source, folder / "Notices" / directory / source.relative_to(ROOT / directory))
    if platform == "windows":
        programs = " ".join('"%~dp0' + item["file"].replace("/", "\\") + '"' for item in records)
        launcher = r"""@echo off
setlocal
set "report=%~dp0Results.txt"
> "%report%" echo Garrido Studio - offline checks
ver >> "%report%"
>> "%report%" echo Architecture: %PROCESSOR_ARCHITECTURE%
>> "%report%" echo Application SHA256: @APPLICATION_SHA256@
set "failed=0"
for %%T in (@PROGRAMS@) do (
  >> "%report%" echo Executable: %%~nxT
  "%%~fT" --test-threads=2 >> "%report%" 2>&1
  if errorlevel 1 set "failed=1"
)
type "%report%"
echo Results.txt contains the full report.
pause
exit /b %failed%
"""
        launcher = launcher.replace("@PROGRAMS@", programs).replace("@APPLICATION_SHA256@", application_sha256)
        generated(folder / "Run-Checks.cmd", launcher.replace("\n", "\r\n"))
    else:
        programs = " ".join("'./" + item["file"] + "'" for item in records)
        launcher = """#!/bin/sh
cd -- "$(dirname -- "$0")" || exit 1
printf '%s\\n' 'Garrido Studio - offline checks' > Results.txt
uname -srmo >> Results.txt
if [ -r /etc/os-release ]; then cat /etc/os-release >> Results.txt; fi
printf 'Desktop session: %s\\n' "${XDG_SESSION_TYPE:-unknown}" >> Results.txt
printf 'Application SHA256: %s\\n' '@APPLICATION_SHA256@' >> Results.txt
failed=0
for program in @PROGRAMS@; do
    printf 'Executable: %s\\n' "$program" >> Results.txt
    if [ -f "$program" ] && [ -x "$program" ]; then
        "$program" --test-threads=2 >> Results.txt 2>&1 || failed=1
    else
        printf 'Missing executable: %s\\n' "$program" >> Results.txt
        failed=1
    fi
done
cat Results.txt
exit "$failed"
"""
        launcher = launcher.replace("@PROGRAMS@", programs).replace("@APPLICATION_SHA256@", application_sha256)
        script = folder / "Run-Checks.sh"
        generated(script, launcher)
        script.chmod(0o755)
    generated(folder / "LEEME.txt", "Pruebas automáticas de Garrido Studio · Pelayo Garrido Martínez\n\nEsta batería usa audio sintético en memoria y carpetas temporales propias.\nNo abre altavoces, micrófono ni plugins externos.\nEjecuta Run-Checks para generar Results.txt y conserva ese informe.\nQue estas pruebas pasen no sustituye probar la app, los dispositivos ni las funciones pendientes.\n")
    generated(folder / "manifest.json", json.dumps({"target": target, "executed": False,
              "application_sha256": application_sha256,
              "desktop_lock_sha256": hashlib.sha256((ROOT / "apps/desktop/Cargo.lock").read_bytes()).hexdigest(),
              "files": records}, indent=2) + "\n")
    print(json.dumps({"platform": platform, "test_executables": len(records), "folder": str(folder.relative_to(ROOT)), "executed": False}, indent=2))


if __name__ == "__main__":
    main()
