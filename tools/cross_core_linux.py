from pathlib import Path
import shutil

from desktop import CARGO, ENV, ROOT, run


def main():
    free = shutil.disk_usage(ROOT).free
    print(f"Espacio libre: {free / 1024**3:.2f} GiB", flush=True)
    if free < 6 * 1024**3:
        raise SystemExit("La comprobación cruzada conserva 6 GiB de reserva.")
    target = "x86_64-unknown-linux-gnu"
    run(str(Path.home() / ".cargo/bin/rustup"), "target", "add", target, "--toolchain", "1.99.1")
    ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/linux-check")
    run(CARGO, "clippy", "--manifest-path", str(ROOT / "Cargo.toml"), "--workspace",
        "--target", target, "--all-targets", "--offline", "--locked", "--", "-D", "warnings")
    print("Tipos del núcleo y plugins para Linux comprobados. La GUI y ejecución Linux siguen pendientes.")


if __name__ == "__main__":
    main()
