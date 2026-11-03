from pathlib import Path
import argparse
import hashlib
import json
import tomllib

from desktop import CARGO, ENV, MANIFEST, ROOT
from cross_build import run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reuse", action="store_true")
    options = parser.parse_args()
    folder = ROOT / "target/windows-build/x86_64-pc-windows-gnullvm/release"
    names = ("garrido-desktop.exe", "garrido-studio.exe", "garrido_native.dll")
    if options.reuse and not all((folder / name).is_file() for name in names):
        raise SystemExit("Reutilizar requiere una compilación completa anterior")
    reserve = 3.5 if options.reuse else 5
    provenance = ROOT / "outputs/zig-toolchain-provenance.json"
    toolchain = ROOT / ".toolchain/zig-aarch64-macos-0.17.0/zig"
    if not toolchain.is_file() or not provenance.is_file():
        raise SystemExit("Falta el SDK Zig oficial verificado; consulta docs/PLATFORMS.md")
    record = json.loads(provenance.read_text())
    if record.get("archive_sha256") != "b607e9b9234790a008116ae5bdb71c6243b84b9fb42a53a9e70fde41c06c536a":
        raise SystemExit("SDK distinto del revisado")
    ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/windows-build")
    ENV["CARGO_BUILD_JOBS"] = "1"
    ENV["CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER"] = str(ROOT / "tools/zig_windows_linker.py")
    ENV["ZIG_GLOBAL_CACHE_DIR"] = str(ROOT / "target/zig-cache")
    ENV["ZIG_LOCAL_CACHE_DIR"] = str(ROOT / "target/zig-local")
    runtime = ROOT / "target/windows-runtime/libcompiler_rt.a"
    runtime.parent.mkdir(exist_ok=True)
    run([str(toolchain), "build-lib", str(toolchain.parent / "lib/compiler_rt.zig"),
         "-target", "x86_64-windows-gnu", "-OReleaseFast", "-fno-compiler-rt", "-fno-lto",
         "-femit-bin=" + str(runtime)], reserve_gib=reserve)
    run([str(Path.home() / ".cargo/bin/rustup"), "target", "add", "x86_64-pc-windows-gnullvm", "--toolchain", "1.99.1"], reserve_gib=reserve)
    for manifest in (ROOT / "Cargo.toml", MANIFEST):
        run([CARGO, "build", "--manifest-path", str(manifest), "--workspace", "--target",
             "x86_64-pc-windows-gnullvm", "--release", "--offline", "--locked"], reserve_gib=reserve)
    files = {}
    for name in names:
        binary = folder / name
        files[name] = {"bytes": binary.stat().st_size, "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
    report = {"target": "x86_64-pc-windows-gnullvm", "version": tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"], "linked": True, "executed_on_windows": False,
              "compiler_sdk": record, "compiler_runtime_sha256": hashlib.sha256(runtime.read_bytes()).hexdigest(), "files": files}
    (ROOT / "outputs/windows-build-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
