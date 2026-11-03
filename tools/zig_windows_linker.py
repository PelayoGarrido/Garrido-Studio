#!/usr/bin/env python3
from pathlib import Path
import os
import sys

ROOT = Path(__file__).resolve().parents[1]
executable = ROOT / ".toolchain/zig-aarch64-macos-0.17.0/zig"
arguments = []
search = [Path(sys.argv[i + 1]) for i in range(1, len(sys.argv) - 1) if sys.argv[i] == "-L"]
for argument in sys.argv[1:]:
    if argument == "-lmsvcrt":
        arguments.append("-lc")
    elif argument == "-Wl,-O1":
        continue
    elif argument.startswith("-Wl,") and argument.endswith(".def"):
        arguments.append(argument[4:])
    elif argument.startswith("-lwindows."):
        library = next((folder / ("lib" + argument[2:] + ".a") for folder in search
                        if (folder / ("lib" + argument[2:] + ".a")).is_file()), None)
        if library is None:
            raise SystemExit("Falta la biblioteca de importación de Windows")
        arguments.append(str(library))
    else:
        arguments.append(argument)
runtime = ROOT / "target/windows-runtime/libcompiler_rt.a"
if not runtime.is_file():
    raise SystemExit("Falta el runtime oficial del compilador")
os.execv(str(executable), [str(executable), "cc", "-target", "x86_64-windows-gnu", *arguments, str(runtime)])
