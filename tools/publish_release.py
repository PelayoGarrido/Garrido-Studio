from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import ssl
import tomllib
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "PelayoGarrido/Garrido-Studio"


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        raise RuntimeError("GitHub API redirect refused")


def inspect_assets(folder, version, commit):
    if folder.is_symlink() or not folder.is_dir():
        raise ValueError("Invalid release directory")
    names = [f"Garrido-Studio-{version}-{target}.{extension}" for target, extension in (
        ("macOS-arm64", "zip"), ("Windows-x64", "zip"), ("Linux-x64", "tar.gz"))]
    records = []
    for name in names:
        path = folder / name
        sidecar = folder / (name + ".json")
        if path.is_symlink() or sidecar.is_symlink() or path.stat().st_size > 128 * 1024**2 or sidecar.stat().st_size > 4 * 1024**2:
            raise ValueError("Release file outside limits")
        report = json.loads(sidecar.read_text(encoding="utf-8"))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        build = report.get("build", {})
        if report.get("sha256") != digest or report.get("bytes") != path.stat().st_size or report.get("readback_verified") is not True:
            raise ValueError("Release archive failed integrity checks")
        if build.get("source_commit") != commit or build.get("version") != version or build.get("internal_preview") is not False:
            raise ValueError("Release archive is from a different source revision")
        diagnostic = build.get("included_plugin_check", {})
        if (diagnostic.get("descriptors"), diagnostic.get("effects"), diagnostic.get("instruments")) != (14, 8, 6):
            raise ValueError("Native included-plugin check missing")
        if diagnostic.get("audio_played") is not False or diagnostic.get("microphone_opened") is not False:
            raise ValueError("Invalid offline diagnostic")
        records.append({"file": name, "sha256": digest, "bytes": path.stat().st_size})
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--folder", type=Path, default=ROOT / "dist/release-assets")
    options = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    commit = os.environ.get("GITHUB_SHA", "")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise SystemExit("Missing source commit")
    records = inspect_assets(options.folder, version, commit)
    checksum = options.folder / "SHA256SUMS.txt"
    data = "".join(f"{item['sha256']}  {item['file']}\n" for item in sorted(records, key=lambda item: item['file'])).encode()
    if checksum.exists():
        if checksum.is_symlink() or checksum.read_bytes() != data:
            raise SystemExit("Checksum file already exists with different contents")
    else:
        with checksum.open("xb") as stream:
            stream.write(data)
    records.append({"file": checksum.name, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
    if options.verify_only:
        print(json.dumps({"version": version, "commit": commit, "files": records}, indent=2))
        return
    if os.environ.get("GITHUB_REPOSITORY") != REPOSITORY or os.environ.get("GITHUB_REF") != f"refs/tags/v{version}":
        raise SystemExit("Publication requires the repository's matching version tag")
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        raise SystemExit("Missing scoped GitHub workflow token")
    certificate = "/etc/ssl/cert.pem" if Path("/etc/ssl/cert.pem").is_file() else None
    opener = urllib.request.build_opener(urllib.request.HTTPSHandler(context=ssl.create_default_context(cafile=certificate)), NoRedirect())

    def api(path, method="GET", body=None, raw=None):
        url = path if path.startswith("https://") else "https://api.github.com" + path
        if urllib.parse.urlsplit(url).hostname not in ("api.github.com", "uploads.github.com"):
            raise ValueError("Unexpected GitHub API host")
        payload = raw if raw is not None else None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(url, data=payload, method=method, headers={
            "Authorization": "Bearer " + token, "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "Garrido-Studio-release",
            "Content-Type": "application/octet-stream" if raw is not None else "application/json",
        })
        with opener.open(request, timeout=180) as response:
            contents = response.read(4 * 1024**2 + 1)
        if len(contents) > 4 * 1024**2:
            raise ValueError("GitHub API response outside limits")
        return json.loads(contents)

    base = "/repos/" + REPOSITORY
    reference = api(base + "/git/ref/tags/v" + version)["object"]
    for _ in range(3):
        if reference["type"] == "commit":
            break
        if reference["type"] != "tag":
            raise ValueError("Unexpected release reference")
        reference = api(base + "/git/tags/" + reference["sha"])["object"]
    if reference["type"] != "commit" or reference["sha"] != commit:
        raise ValueError("Release tag points to another commit")
    try:
        release = api(base + "/releases/tags/v" + version)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        release = api(base + "/releases", "POST", {
            "tag_name": "v" + version, "target_commitish": commit,
            "name": "Garrido Studio " + version,
            "body": (ROOT / f"docs/RELEASE-v{version}.md").read_text(encoding="utf-8"),
            "draft": True, "prerelease": False,
        })
    existing = {item["name"]: item for item in api(base + f"/releases/{release['id']}/assets?per_page=100")}
    for record in records:
        name = record["file"]
        asset = existing.get(name)
        if asset is None:
            url = f"https://uploads.github.com/repos/{REPOSITORY}/releases/{release['id']}/assets?" + urllib.parse.urlencode({"name": name})
            asset = api(url, "POST", raw=(options.folder / name).read_bytes())
        if asset.get("size") != record["bytes"] or asset.get("digest") != "sha256:" + record["sha256"] or asset.get("state") != "uploaded":
            raise ValueError("Uploaded asset failed hash/size verification: " + name)
        print(json.dumps({"asset": name, "sha256": record["sha256"], "verified": True}))
    release = api(base + f"/releases/{release['id']}", "PATCH", {"draft": False, "prerelease": False, "make_latest": "true"})
    if release.get("draft") is not False or release.get("tag_name") != "v" + version:
        raise ValueError("Release publication was not confirmed")
    print(release["html_url"])


if __name__ == "__main__":
    main()
