from pathlib import Path
import shutil

from desktop import CARGO, ENV, MANIFEST, ROOT, run


def main():
    free = shutil.disk_usage(ROOT).free
    print(f"Espacio libre: {free / 1024**3:.2f} GiB", flush=True)
    if free < 6 * 1024**3:
        raise SystemExit("La comprobación cruzada conserva 6 GiB de reserva.")
    target = "x86_64-pc-windows-gnu"
    run(str(Path.home() / ".cargo/bin/rustup"), "target", "add", target, "--toolchain", "1.99.1")
    ENV["CARGO_TARGET_DIR"] = str(ROOT / "target/windows-check")
    for manifest in (ROOT / "Cargo.toml", MANIFEST):
        run(CARGO, "clippy", "--manifest-path", str(manifest), "--workspace", "--target", target,
            "--all-targets", "--offline", "--locked", "--", "-D", "warnings")
    print("Comprobación de tipos para Windows terminada. No enlaza ni ejecuta una aplicación Windows.")


if __name__ == "__main__":
    main()
