from copy import deepcopy
from datetime import date
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from audit_policy import assess


class AuditPolicyTests(unittest.TestCase):
    def setUp(self):
        self.policy = {"exceptions": [{
            "id": "RUSTSEC-2026-0370", "package": "zvariant", "version": "5.15.1",
            "checksum": "a" * 64, "reviewed": "2026-11-03", "expires": "2026-11-17",
        }]}
        self.report = {
            "vulnerabilities": {"count": 0, "found": False, "list": []},
            "warnings": {"unmaintained": [{
                "kind": "unmaintained",
                "package": {"name": "zvariant", "version": "5.15.1", "checksum": "a" * 64,
                            "source": "registry+https://github.com/rust-lang/crates.io-index"},
                "advisory": {"id": "RUSTSEC-2026-0370", "informational": "unmaintained"},
            }]},
        }

    def decide(self, report=None, policy=None, today=date(2026, 11, 3)):
        return assess(self.report if report is None else report, self.policy if policy is None else policy, today)

    def test_only_exact_reviewed_maintenance_is_accepted_and_remains_visible(self):
        result = self.decide()
        self.assertTrue(result["passed"])
        self.assertEqual(len(result["reviewed_warnings"]), 1)
        self.assertEqual(len(self.report["warnings"]["unmaintained"]), 1)

    def test_a_vulnerability_always_blocks_even_with_a_maintenance_exception(self):
        self.report["vulnerabilities"] = {"count": 1, "found": True, "list": [{"id": "RUSTSEC-2026-0370"}]}
        self.assertFalse(self.decide()["passed"])

    def test_expired_future_and_overlong_reviews_fail_closed(self):
        self.assertFalse(self.decide(today=date(2026, 11, 18))["passed"])
        self.assertFalse(self.decide(today=date(2026, 11, 2))["passed"])
        self.policy["exceptions"][0]["expires"] = "2027-01-01"
        self.assertFalse(self.decide()["passed"])

    def test_modified_version_checksum_or_registry_is_not_covered(self):
        for key, value in (("version", "5.15.2"), ("checksum", "b" * 64), ("source", "git+untrusted")):
            report = deepcopy(self.report)
            report["warnings"]["unmaintained"][0]["package"][key] = value
            self.assertFalse(self.decide(report=report)["passed"])

    def test_new_warning_and_nonmaintenance_kinds_are_not_covered(self):
        report = deepcopy(self.report)
        report["warnings"]["unmaintained"][0]["advisory"]["id"] = "RUSTSEC-2026-9999"
        self.assertFalse(self.decide(report=report)["passed"])
        report = deepcopy(self.report)
        report["warnings"]["unsound"] = report["warnings"].pop("unmaintained")
        self.assertFalse(self.decide(report=report)["passed"])

    def test_missing_metadata_and_explicit_errors_block(self):
        self.assertFalse(self.decide(report={})["passed"])
        self.report["error"] = "database failed"
        self.assertFalse(self.decide()["passed"])

    def test_strict_policy_keeps_all_warnings_blocking(self):
        self.assertFalse(self.decide(policy={})["passed"])
        self.report["warnings"] = {}
        self.assertTrue(self.decide(policy={})["passed"])


if __name__ == "__main__":
    unittest.main()
