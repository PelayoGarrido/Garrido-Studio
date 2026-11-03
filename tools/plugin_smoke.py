from pathlib import Path
import json
import hashlib
import math
import shutil
import struct
import subprocess
import tempfile
import wave

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "dist/Garrido Studio.app/Contents"
CLI = APP / "MacOS/garrido-studio"
PLUGIN = APP / "PlugIns/GarridoNative.clap"


def call(*args, success=True):
    result = subprocess.run([str(CLI), *map(str, args)], capture_output=True, text=True, timeout=20)
    if success and result.returncode:
        raise RuntimeError(result.stderr)
    if not success and result.returncode == 0:
        raise RuntimeError("Se aceptó un plugin cuya huella fue alterada")
    return result.stdout


def read(path):
    with wave.open(str(path), "rb") as stream:
        assert stream.getnchannels() == 2
        assert stream.getsampwidth() == 3
        rate = stream.getframerate()
        frames = stream.getnframes()
        raw = stream.readframes(frames)
    samples = [int.from_bytes(raw[i:i+3], "little", signed=True) / 8388608 for i in range(0, len(raw), 3)]
    assert all(math.isfinite(v) and abs(v) <= 0.800001 for v in samples)
    return rate, frames, samples


def main():
    (ROOT / "outputs").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="clap-smoke-", dir=ROOT / "outputs") as folder:
        folder = Path(folder)
        listing = call("clap-scan", PLUGIN).splitlines()
        assert len(listing) == 14
        assert len({line.split("\t")[1] for line in listing}) == 14
        assert sum(line.endswith("\tinstrument") for line in listing) == 6
        source = folder / "source.wav"
        samples = []
        with wave.open(str(source), "wb") as stream:
            stream.setparams((2, 2, 8000, 0, "NONE", "not compressed"))
            data = bytearray()
            for i in range(4000):
                value = round(math.sin(i * math.tau * 220 / 8000) * 3276)
                samples.append(value / 32768)
                data.extend(struct.pack("<hh", value, -value))
            stream.writeframes(data)
        for index in range(8):
            output = folder / f"effect-{index}.wav"
            call("clap-render", PLUGIN, index, source, output)
            rate, count, audio = read(output)
            assert (rate, count) == (8000, 4000)
            assert max(map(abs, audio)) > 0.001
            if index == 0:
                error = max(abs(audio[i*2] - value*0.6) for i, value in enumerate(samples))
                assert error < 0.000001, error
        session = folder / "demo.gstudio"
        call("demo", session)
        for index in range(8, 14):
            output = folder / f"instrument-{index}.wav"
            call("clap-instrument", PLUGIN, index, session, 4, output)
            rate, count, audio = read(output)
            assert rate == 48000 and count > 48000
            assert max(map(abs, audio)) > 0.001
        copied = folder / "Garrido Studio.app"
        shutil.copytree(APP.parent, copied)
        copied_cli = copied / "Contents/MacOS/garrido-studio"
        altered = copied / "Contents/PlugIns/GarridoNative.clap"
        relocated = subprocess.run([str(copied_cli), "clap-scan", str(altered)], capture_output=True, text=True, timeout=20)
        assert relocated.returncode == 0 and len(relocated.stdout.splitlines()) == 14
        original_hash = hashlib.sha256((PLUGIN / "Contents/MacOS/GarridoNative").read_bytes()).hexdigest()
        with (altered / "Contents/MacOS/GarridoNative").open("ab") as stream:
            stream.write(b"modified-test")
        rejected = subprocess.run([str(copied_cli), "clap-scan", str(altered)], capture_output=True, text=True, timeout=20)
        assert rejected.returncode != 0
        assert hashlib.sha256((PLUGIN / "Contents/MacOS/GarridoNative").read_bytes()).hexdigest() == original_hash
    report = {"descriptors":14, "effects_rendered":8, "instruments_rendered":6,
              "gain_absolute_error_max":error, "modified_bundle_rejected":True,
              "cli_sha256":hashlib.sha256(CLI.read_bytes()).hexdigest(),
              "relocated_package_accepted":True,"source_plugin_preserved":True,
              "audio_played":False, "third_party_code_loaded":False}
    (ROOT / "outputs/plugin-smoke.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
