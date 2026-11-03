from pathlib import Path
import argparse
import hashlib
import json
import math
import platform
import re
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", default="")
    options = parser.parse_args()
    if options.variant and not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,39}", options.variant):
        raise SystemExit("Nombre de variante no válido")
    if platform.system() != "Darwin":
        raise SystemExit("Este adaptador CLAP solo está validado en macOS")
    suffix = f"-{options.variant}" if options.variant else ""
    app = ROOT / f"dist/desktop/macos-arm64{suffix}/Garrido Studio.app/Contents"
    binary = app / "MacOS/garrido-desktop"
    plugin = app / "PlugIns/GarridoNative.clap"

    def worker(path, index=0, request=None):
        args = [str(binary), "--clap-scan-worker" if request is None else "--clap-worker", str(path), str(index)]
        return subprocess.run(args, input=request or b"", capture_output=True, timeout=20)

    listing = worker(plugin)
    assert listing.returncode == 0, listing.stderr.decode(errors="replace")
    lines = listing.stdout.decode().splitlines()
    assert len(lines) == 14 and len({line.split("\t")[1] for line in lines}) == 14
    assert sum(line.endswith("\tinstrument") for line in lines) == 6
    rate, count = 8000, 16000
    samples = [math.sin(i * math.tau * 220 / rate) * 0.1 for i in range(count)]
    effect = b"GCLAP001" + struct.pack("<II", rate, count) + b"".join(struct.pack("<ff", value, -value) for value in samples)
    instrument = b"GCLAP002" + struct.pack("<II", rate, count) + bytes(count * 8) + struct.pack("<IIII f", 1, 0, 4000, 60, 0.65)
    gain_error = None
    for index in range(14):
        result = worker(plugin, index, effect if index < 8 else instrument)
        assert result.returncode == 0, result.stderr.decode(errors="replace")
        data = result.stdout
        assert len(data) == 16 + count * 8
        assert data[:16] == b"GCLAP001" + struct.pack("<II", rate, count)
        values = [value for frame in struct.iter_unpack("<ff", data[16:]) for value in frame]
        assert all(math.isfinite(v) and abs(v) <= 0.800001 for v in values)
        assert max(map(abs, values)) > 0.001
        if index == 0:
            gain_error = max(abs(values[i * 2] - value * 0.6) for i, value in enumerate(samples))
            assert gain_error < 0.000001
    with tempfile.TemporaryDirectory(prefix="desktop-plugin-", dir=ROOT / "outputs") as folder:
        copied = Path(folder) / "Garrido Studio.app"
        shutil.copytree(app.parent, copied)
        copied_binary = copied / "Contents/MacOS/garrido-desktop"
        altered = copied / "Contents/PlugIns/GarridoNative.clap"
        copied_scan = subprocess.run([str(copied_binary), "--clap-scan-worker", str(altered), "0"], capture_output=True, timeout=20)
        assert copied_scan.returncode == 0
        assert len(copied_scan.stdout.decode().splitlines()) == 14
        diagnostic = subprocess.run([str(copied_binary), "--check-bundled-plugins-report"], capture_output=True, timeout=45)
        assert diagnostic.returncode == 0, diagnostic.stderr.decode(errors="replace")
        reports = list(copied_binary.parent.glob("Plugin-Check-*.txt"))
        assert len(reports) == 1
        original_report = reports[0].read_bytes()
        assert b"PASS: 14 descriptors" in original_report
        assert hashlib.sha256(copied_binary.read_bytes()).hexdigest().encode() in original_report
        before = hashlib.sha256((plugin / "Contents/MacOS/GarridoNative").read_bytes()).hexdigest()
        with (altered / "Contents/MacOS/GarridoNative").open("ab") as stream:
            stream.write(b"modified-test")
        rejected = subprocess.run([str(copied_binary), "--clap-scan-worker", str(altered), "0"], capture_output=True, timeout=20)
        assert rejected.returncode != 0
        failed_diagnostic = subprocess.run([str(copied_binary), "--check-bundled-plugins-report"], capture_output=True, timeout=20)
        assert failed_diagnostic.returncode != 0
        failures = [path for path in copied_binary.parent.glob("Plugin-Check-*.txt") if path not in reports]
        assert len(failures) == 1 and b"FAIL:" in failures[0].read_bytes()
        assert reports[0].read_bytes() == original_report
        assert hashlib.sha256((plugin / "Contents/MacOS/GarridoNative").read_bytes()).hexdigest() == before
    report = {
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "descriptors": 14, "effects": 8, "instruments": 6,
        "gain_absolute_error_max": gain_error, "modified_bundle_rejected": True,
        "relocated_package_accepted": True, "source_plugin_preserved": True,
        "public_diagnostic_pass_and_failure_reports_verified": True,
        "previous_report_preserved": True,
        "audio_played": False, "microphone_opened": False, "third_party_code_loaded": False,
    }
    (ROOT / f"outputs/desktop-plugin-smoke{suffix}.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
