from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import tomllib

from desktop import CARGO, ENV, MANIFEST, ROOT
from cross_build import run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reuse", action="store_true")
    options = parser.parse_args()
    folder = ROOT / "target/linux-build/x86_64-unknown-linux-gnu/release"
    names = ("garrido-desktop", "garrido-studio", "libgarrido_native.so")
    if options.reuse and not all((folder / name).is_file() for name in names):
        raise SystemExit("Reutilizar requiere una compilación completa anterior")
    reserve = 3.5 if options.reuse else 5
    sysroot = ROOT / ".toolchain/linux-sysroot"
    pkgconf = ROOT / ".toolchain/pkgconf-3.0.10/pkgconf"
    if not pkgconf.is_file() or not (ROOT / "outputs/linux-sysroot-provenance.json").is_file():
        raise SystemExit("Falta el SDK Linux revisado; consulta docs/PLATFORMS.md")
    ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/linux-build")
    ENV["CARGO_BUILD_JOBS"] = "1"
    ENV["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"] = str(ROOT / "tools/zig_linux_linker.py")
    ENV["CC_x86_64_unknown_linux_gnu"] = str(ROOT / "tools/zig_linux_linker.py")
    ENV["AR_x86_64_unknown_linux_gnu"] = str(ROOT / "tools/zig_ar.py")
    ENV["ZIG_GLOBAL_CACHE_DIR"] = str(ROOT / "target/zig-cache")
    ENV["ZIG_LOCAL_CACHE_DIR"] = str(ROOT / "target/zig-local")
    ENV["PKG_CONFIG"] = str(pkgconf)
    ENV["PKG_CONFIG_SYSROOT_DIR"] = str(sysroot)
    ENV["PKG_CONFIG_LIBDIR"] = str(sysroot / "usr/lib/x86_64-linux-gnu/pkgconfig")
    ENV["PKG_CONFIG_ALLOW_CROSS"] = "1"
    subprocess.run([str(pkgconf), "--modversion", "alsa", "wayland-client", "xkbcommon"], cwd=ROOT, env=ENV, check=True)
    for manifest in (ROOT / "Cargo.toml", MANIFEST):
        run([CARGO, "build", "--manifest-path", str(manifest), "--workspace", "--target",
             "x86_64-unknown-linux-gnu", "--release", "--offline", "--locked"], reserve_gib=reserve)
    files = {}
    for name in names:
        binary = folder / name
        if binary.read_bytes()[:4] != b"\x7fELF":
            raise SystemExit("La salida no es un ejecutable ELF")
        files[name] = {"bytes": binary.stat().st_size, "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
    report = {"target": "x86_64-unknown-linux-gnu", "version": tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"], "minimum_glibc": "2.39", "linked": True,
              "executed_on_linux": False, "files": files}
    (ROOT / "outputs/linux-build-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
