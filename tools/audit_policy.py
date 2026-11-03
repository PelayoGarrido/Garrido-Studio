from datetime import date
import re


def assess(report, policy, today=None):
    today = today or date.today()
    vulnerability = report.get("vulnerabilities", {})
    valid = (
        report.get("error") is None
        and isinstance(report.get("warnings"), dict)
        and vulnerability.get("count") == 0
        and vulnerability.get("found") is False
        and vulnerability.get("list") == []
    )
    reviewed = []
    pending = []
    for kind, items in report.get("warnings", {}).items():
        for item in items:
            advisory = item.get("advisory", {})
            package = item.get("package", {})
            accepted = False
            for entry in policy.get("exceptions", []):
                if not all(isinstance(entry.get(key), str) and entry[key] for key in ("id", "package", "version", "checksum")):
                    continue
                if not re.fullmatch(r"RUSTSEC-[0-9]{4}-[0-9]{4}", entry["id"]) or not re.fullmatch(r"[0-9a-f]{64}", entry["checksum"]):
                    continue
                try:
                    start = date.fromisoformat(entry["reviewed"])
                    end = date.fromisoformat(entry["expires"])
                    dates_valid = start <= today <= end and 0 <= (end - start).days <= 31
                except (ValueError, TypeError, KeyError):
                    dates_valid = False
                if (
                    dates_valid
                    and kind == "unmaintained"
                    and item.get("kind") == "unmaintained"
                    and advisory.get("informational") == "unmaintained"
                    and advisory.get("id") == entry.get("id")
                    and package.get("name") == entry.get("package")
                    and package.get("version") == entry.get("version")
                    and package.get("checksum") == entry.get("checksum")
                    and package.get("source") == "registry+https://github.com/rust-lang/crates.io-index"
                ):
                    accepted = True
                    break
            record = {"id": advisory.get("id"), "package": package.get("name"), "kind": kind}
            (reviewed if accepted else pending).append(record)
    return {"passed": valid and not pending, "reviewed_warnings": reviewed, "unreviewed_warnings": pending}
