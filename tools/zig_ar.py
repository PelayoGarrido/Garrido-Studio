#!/usr/bin/env python3
from pathlib import Path
import os
import sys

executable = Path(__file__).resolve().parents[1] / ".toolchain/zig-aarch64-macos-0.17.0/zig"
os.execv(str(executable), [str(executable), "ar", *sys.argv[1:]])
