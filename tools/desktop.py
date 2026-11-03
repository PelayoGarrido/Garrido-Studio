from pathlib import Path
import argparse
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps/desktop/Cargo.toml"
ENV = dict(os.environ)
ENV["PATH"] = str(Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / "bin") + os.pathsep + ENV.get("PATH", "")
ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/desktop")
ENV["CARGO_BUILD_JOBS"] = "2"
ENV.pop("RUSTFLAGS", None)
ENV["CARGO_ENCODED_RUSTFLAGS"] = "--remap-path-prefix=" + str(ROOT) + "=."
CARGO = shutil.which("cargo", path=ENV["PATH"]) or str(Path.home() / ".cargo/bin/cargo")
RESERVE = 5


def run(*args):
    from cross_build import run as monitored
    monitored(list(args), reserve_gib=RESERVE, env=ENV)


def main():
    global RESERVE
    parser = argparse.ArgumentParser()
    parser.add_argument("command", nargs="?", default="check")
    parser.add_argument("--reuse", action="store_true")
    parser.add_argument("--release", action="store_true")
    options = parser.parse_args()
    command = options.command
    if options.release and command != "check":
        raise SystemExit("--release solo se aplica a la comprobación de la interfaz")
    free = shutil.disk_usage(ROOT).free
    print(f"Espacio libre: {free / 1024**3:.2f} GiB", flush=True)
    reserve = 3 if command in ("format", "probe", "probe-stream") else 5
    if options.reuse:
        if command not in ("check", "build", "preview"):
            raise SystemExit("Reutilizar solo se aplica a comprobaciones o compilaciones")
        profile = "release" if command == "build" or options.release else "debug"
        dependencies = ROOT / "target/desktop" / profile / "deps"
        if not dependencies.is_dir() or not any(dependencies.glob("*.rlib")):
            raise SystemExit("Reutilizar requiere dependencias ya compiladas")
        reserve = 3.5
    RESERVE = reserve
    if free < reserve * 1024**3:
        raise SystemExit(f"La interfaz común conserva {reserve} GiB de reserva.")
    from dev import font_assets, brand_assets
    font_assets()
    brand_assets()
    base = ("--manifest-path", str(MANIFEST))
    if command == "fetch":
        ENV["CARGO_NET_OFFLINE"] = "false"
        ENV["CARGO_HTTP_MULTIPLEXING"] = "false"
        ENV["CARGO_HTTP_TIMEOUT"] = "30"
        ENV["CARGO_NET_RETRY"] = "1"
        run(CARGO, "fetch", *base, "--locked")
    elif command == "check":
        profile = ("--release",) if options.release else ()
        run(CARGO, "fmt", *base, "--", "--check")
        run(CARGO, "clippy", *base, *profile, "--offline", "--locked", "--all-targets", "--", "-D", "warnings")
        run(CARGO, "test", *base, *profile, "--offline", "--locked")
    elif command == "format":
        run(CARGO, "fmt", *base)
    elif command == "preview":
        run(CARGO, "build", *base, "--offline", "--locked")
    elif command == "build":
        run(CARGO, "build", *base, "--offline", "--locked", "--release")
    elif command == "probe":
        run(str(ROOT / "target/desktop/release/garrido-desktop"), "--probe-output")
    elif command == "probe-stream":
        run(str(ROOT / "target/desktop/release/garrido-desktop"), "--probe-stream")
    elif command == "start":
        run(CARGO, "run", *base, "--offline", "--locked", "--release")
    else:
        raise SystemExit("Comando no reconocido")


if __name__ == "__main__":
    main()
