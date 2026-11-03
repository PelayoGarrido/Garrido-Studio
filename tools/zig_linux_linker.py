#!/usr/bin/env python3
from pathlib import Path
import os
import sys

ROOT = Path(__file__).resolve().parents[1]
executable = ROOT / ".toolchain/zig-aarch64-macos-0.17.0/zig"
lib = ROOT / ".toolchain/linux-sysroot/usr/lib/x86_64-linux-gnu"
os.execv(str(executable), [str(executable), "cc", "-target", "x86_64-linux-gnu.2.39", "-L" + str(lib), *sys.argv[1:]])
