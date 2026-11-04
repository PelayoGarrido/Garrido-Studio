from pathlib import Path
import argparse

from cross_build import run
from dev import CARGO, ENV, ROOT, own_directory, reserve


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=ROOT / "outputs/spectral-journey")
    options = parser.parse_args()
    reserve()
    folder = options.out.absolute()
    own_directory(folder)
    if any(folder.iterdir()):
        raise SystemExit("Usa una carpeta vacía para conservar las demos anteriores")
    env = dict(ENV)
    env["CARGO_BUILD_JOBS"] = "2"
    run([CARGO, "run", "--offline", "--locked", "--release", "--example", "spectral_journey", "--", str(folder)], reserve_gib=3, env=env)


if __name__ == "__main__":
    main()
