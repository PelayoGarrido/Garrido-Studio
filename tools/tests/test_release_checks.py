from pathlib import Path
import hashlib
import json
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from publish_release import inspect_assets


class ReleaseChecksTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="garrido-release-check-")
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        self.commit = "a" * 40
        self.names = []
        for target, extension in (("macOS-arm64", "zip"), ("Windows-x64", "zip"), ("Linux-x64", "tar.gz")):
            name = f"Garrido-Studio-1.0.0-{target}.{extension}"
            self.names.append(name)
            payload = target.encode()
            (self.folder / name).write_bytes(payload)
            report = {"sha256": hashlib.sha256(payload).hexdigest(), "bytes": len(payload),
                      "readback_verified": True, "build": {"source_commit": self.commit,
                      "version": "1.0.0", "internal_preview": False, "included_plugin_check": {
                      "descriptors": 14, "effects": 8, "instruments": 6,
                      "audio_played": False, "microphone_opened": False}}}
            (self.folder / (name + ".json")).write_text(json.dumps(report), encoding="utf-8")

    def inspect(self):
        return inspect_assets(self.folder, "1.0.0", self.commit)

    def change(self, action):
        sidecar = self.folder / (self.names[0] + ".json")
        report = json.loads(sidecar.read_text(encoding="utf-8"))
        action(report)
        sidecar.write_text(json.dumps(report), encoding="utf-8")

    def test_complete_matching_set_is_required(self):
        self.assertEqual(len(self.inspect()), 3)
        (self.folder / self.names[-1]).unlink()
        with self.assertRaises(FileNotFoundError):
            self.inspect()

    def test_changed_bytes_are_rejected(self):
        (self.folder / self.names[0]).write_bytes(b"changed")
        with self.assertRaises(ValueError):
            self.inspect()

    def test_other_commit_and_version_are_rejected(self):
        self.change(lambda report: report["build"].update(source_commit="b" * 40))
        with self.assertRaises(ValueError):
            self.inspect()
        self.change(lambda report: report["build"].update(source_commit=self.commit, version="1.0.1"))
        with self.assertRaises(ValueError):
            self.inspect()

    def test_preview_cannot_be_promoted_by_the_publisher(self):
        self.change(lambda report: report["build"].update(internal_preview=True))
        with self.assertRaises(ValueError):
            self.inspect()

    def test_native_plugin_evidence_cannot_be_omitted(self):
        self.change(lambda report: report["build"].pop("included_plugin_check"))
        with self.assertRaises(ValueError):
            self.inspect()

    def test_archive_readback_is_mandatory(self):
        self.change(lambda report: report.update(readback_verified=False))
        with self.assertRaises(ValueError):
            self.inspect()


if __name__ == "__main__":
    unittest.main()
