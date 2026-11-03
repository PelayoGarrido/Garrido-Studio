from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch
import json
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import dependency_audit


class DependencyAuditTests(unittest.TestCase):
    def test_incomplete_registry_check_blocks_even_with_clean_advisory_json(self):
        report = {"vulnerabilities": {"count": 0, "found": False, "list": []}, "warnings": {}}
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ["apps/desktop", "docs", ".toolchain/bin"]:
                (root / name).mkdir(parents=True)
            (root / "apps/desktop/Cargo.lock").write_text('[[package]]\nname="example"\nversion="1.0.0"\n')
            (root / "docs/security-maintenance.toml").write_text("exceptions=[]\n")
            (root / ".toolchain/bin/cargo-audit").touch()
            for diagnostic, expected in [("", 0), ("error: could not check registry version", 1)]:
                with self.subTest(diagnostic=diagnostic):
                    result = subprocess.CompletedProcess([], 0, json.dumps(report), diagnostic)
                    with patch.object(dependency_audit, "ROOT", root), \
                         patch.object(sys, "argv", ["dependency_audit", "--db", str(root / "database")]), \
                         patch.object(dependency_audit.subprocess, "run", return_value=result), \
                         patch("builtins.print"), self.assertRaises(SystemExit) as raised:
                        dependency_audit.main()
                    self.assertEqual(raised.exception.code, expected)
                    summary = json.loads((root / "outputs/desktop-dependency-audit.json").read_text())
                    self.assertEqual(summary["passed"], expected == 0)
                    self.assertEqual(summary["scanner_diagnostics_present"], bool(diagnostic))


if __name__ == "__main__":
    unittest.main()
