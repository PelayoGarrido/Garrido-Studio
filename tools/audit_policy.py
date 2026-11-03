from datetime import date
import re


def assess(report, policy, today=None):
    today = today or date.today()
    if not isinstance(report, dict) or not isinstance(policy, dict):
        return {"passed": False, "reviewed_warnings": [], "unreviewed_warnings": []}
    vulnerability = report.get("vulnerabilities", {})
    warnings = report.get("warnings")
    if not isinstance(vulnerability, dict) or not isinstance(warnings, dict):
        return {"passed": False, "reviewed_warnings": [], "unreviewed_warnings": []}
    valid = (
        report.get("error") is None
        and isinstance(report.get("warnings"), dict)
        and vulnerability.get("count") == 0
        and vulnerability.get("found") is False
        and vulnerability.get("list") == []
    )
    reviewed = []
    pending = []
    for kind, items in warnings.items():
        if not isinstance(items, list):
            valid = False
            continue
        for item in items:
            if not isinstance(item, dict):
                valid = False
                continue
            advisory = item.get("advisory")
            package = item.get("package")
            advisory = advisory if isinstance(advisory, dict) else {}
            package = package if isinstance(package, dict) else {}
            accepted = False
            exceptions = policy.get("exceptions", [])
            for entry in exceptions if isinstance(exceptions, list) else []:
                if not isinstance(entry, dict):
                    continue
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
            record = {"id": advisory.get("id"), "package": package.get("name"), "kind": kind,
                      "version": package.get("version"), "checksum": package.get("checksum")}
            (reviewed if accepted else pending).append(record)
    return {"passed": valid and not pending, "reviewed_warnings": reviewed, "unreviewed_warnings": pending}
