from pathlib import Path
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    if shutil.disk_usage(ROOT).free < 5 * 1024**3:
        raise SystemExit("Se conservan 5 GiB para revisar dependencias")
    env = dict(os.environ)
    env["PATH"] = str(Path.home() / ".cargo/bin") + os.pathsep + env.get("PATH", "")
    env["CARGO_BUILD_JOBS"] = "2"
    env["CARGO_NET_OFFLINE"] = "false"
    executable = ROOT / ".toolchain/bin/cargo-audit"
    if not executable.is_file():
        subprocess.run([
            str(Path.home() / ".cargo/bin/cargo"), "install", "cargo-audit", "--version", "0.22.2",
            "--locked", "--root", str(ROOT / ".toolchain"), "--target-dir", str(ROOT / "target/audit-build"),
        ], cwd=ROOT, env=env, check=True)
    subprocess.run([sys.executable, str(ROOT / "tools/dependency_audit.py")], cwd=ROOT, env=env, check=True)


if __name__ == "__main__":
    main()
