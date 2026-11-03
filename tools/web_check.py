from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / "web/public"


def require(condition, message):
    if not condition:
        raise ValueError(message)


class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.references = []
        self.panels = []
        self.csp = False
        self.headings = 0

    def handle_starttag(self, tag, attributes):
        values = dict(attributes)
        if "id" in values:
            require(values["id"] not in self.ids, "Duplicate page ID")
            self.ids.add(values["id"])
        self.headings += tag == "h1"
        if tag == "meta" and values.get("http-equiv", "").lower() == "content-security-policy":
            content = values.get("content", "")
            require("connect-src 'none'" in content and "object-src 'none'" in content, "Missing resource policy")
            require("'unsafe-inline'" not in content and "'unsafe-eval'" not in content, "Unsafe script policy")
            self.csp = True
        for name in ("src", "href"):
            if name in values:
                self.references.append((tag, values[name]))
        if values.get("role") == "tab":
            self.panels.append(values.get("aria-controls"))
        if tag == "audio":
            require("controls" in values and "autoplay" not in values and values.get("preload") == "none", "Audio must be explicit")
        if tag == "img":
            require("alt" in values, "Image needs alternative text")


def main():
    require(PUBLIC.is_dir() and not PUBLIC.is_symlink(), "Invalid website directory")
    files = [path for path in PUBLIC.rglob("*") if path.is_file()]
    require(all(not path.is_symlink() for path in PUBLIC.rglob("*")), "Website cannot contain linked files")
    require(sum(path.stat().st_size for path in files) < 20 * 1024**2, "Website outside its asset budget")
    for path in files:
        if path.suffix in (".html", ".css", ".js"):
            text = path.read_text(encoding="utf-8")
            require(not any(value in text for value in ("/Users/", "Documents/Privado", "PREPARACION-TECNICA", "interview-prep")), "Private content in website")
    page = Page()
    page.feed((PUBLIC / "index.html").read_text(encoding="utf-8"))
    require(page.csp and page.headings == 1, "Invalid document structure")
    require(len(page.panels) == 3 and all(panel in page.ids for panel in page.panels), "Invalid tab targets")
    for tag, reference in page.references:
        parsed = urlsplit(reference)
        if parsed.scheme:
            require(tag == "a" and parsed.scheme == "https" and parsed.hostname in ("github.com", "pelayogarrido.github.io"), "External runtime resource")
        elif parsed.path:
            path = (PUBLIC / unquote(parsed.path)).resolve()
            require(path.is_relative_to(PUBLIC) and path.is_file(), f"Missing local asset: {parsed.path}")
        elif parsed.fragment:
            require(parsed.fragment in page.ids, "Missing section anchor")
    subprocess.run(["node", "--check", str(PUBLIC / "app.js")], check=True)
    print(f"Website checked: {len(files)} files, local assets, keyboard-tab targets, CSP and private-content exclusion")


if __name__ == "__main__":
    main()
