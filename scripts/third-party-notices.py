#!/usr/bin/env python3
"""Writes THIRD_PARTY_NOTICES.md: the open-source components the Rupert apps
ship, with their license texts.

- Rust crates reachable from the desktop app (`rupert`) and the iPhone app
  (`rupert-mobile`) through normal dependencies, on every platform
  (`cargo metadata`); license files come from each crate's own folder.
- JavaScript that ends up in the window: the app's dependencies plus the
  Svelte and SvelteKit runtimes, with theirs (`node_modules`).
- PDFium, from the license files `scripts/fetch-pdfium.sh` downloads.

Identical license texts are printed once, with every component that uses
them. Run from anywhere: `python3 scripts/third-party-notices.py`.
"""

import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_NOTICES.md"
FIRST_PARTY = {"rupert", "rupert-core", "rupert-mobile", "tauri-plugin-rupert-ios"}
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice)([-._].*)?$", re.I)
# Runtime code compiled into the window although it's a dev dependency.
JS_RUNTIMES = ["svelte", "@sveltejs/kit"]


def license_texts(folder: Path) -> list[str]:
    texts = []
    if folder.is_dir():
        for f in sorted(folder.iterdir()):
            if f.is_file() and LICENSE_FILE.match(f.name):
                try:
                    texts.append(f.read_text(encoding="utf-8", errors="replace").strip())
                except OSError:
                    pass
    return texts


# MARK: - Standard texts
# For packages that don't include a license file: the standard text of
# their license (MIT and Zlib with the authors as copyright holders;
# Apache-2.0 and MPL-2.0 from another package's verbatim copy).

MIT = """Copyright (c) {holders}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

ZLIB = """Copyright (c) {holders}

This software is provided 'as-is', without any express or implied
warranty. In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution."""


BSD3 = """Copyright (c) {holders}

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."""


def verbatim(components: list[dict], pattern: str) -> str | None:
    """A full license text (e.g. Apache-2.0) as another package ships it."""
    for c in components:
        for t in c["texts"]:
            if re.search(pattern, t[:400]):
                return t
    return None


def fill_missing(components: list[dict]) -> None:
    apache = verbatim(components, r"Apache License\s+Version 2\.0")
    mpl = verbatim(components, r"Mozilla Public License,? [Vv]ersion 2\.0")
    for c in components:
        if c["texts"]:
            continue
        lic = c["license"]
        holders = ", ".join(c.get("authors") or []) or f"the {c['name']} authors"
        holders = re.sub(r"\s*<[^>]*>", "", holders)  # no email addresses
        if re.search(r"\bMIT\b", lic):
            c["texts"] = [MIT.format(holders=holders)]
        elif "BSD-3-Clause" in lic:
            c["texts"] = [BSD3.format(holders=holders)]
        elif "Zlib" in lic:
            c["texts"] = [ZLIB.format(holders=holders)]
        elif "Apache-2.0" in lic and apache:
            c["texts"] = [apache]
        elif "MPL-2.0" in lic and mpl:
            c["texts"] = [mpl]


# MARK: - Rust


def rust_components() -> list[dict]:
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    roots = [p["id"] for p in meta["packages"] if p["name"] in ("rupert", "rupert-mobile")]
    seen, stack = set(), list(roots)
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            # Normal dependencies only (build scripts and tests don't ship).
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    out = []
    for pid in seen:
        p = packages[pid]
        if p["name"] in FIRST_PARTY:
            continue
        out.append(
            {
                "name": p["name"],
                "version": p["version"],
                "license": p.get("license") or "see license text",
                "source": p.get("repository") or f"https://crates.io/crates/{p['name']}",
                "authors": p.get("authors") or [],
                "texts": license_texts(Path(p["manifest_path"]).parent),
            }
        )
    return out


# MARK: - JavaScript


def js_components() -> list[dict]:
    pkg = json.loads((ROOT / "package.json").read_text())
    todo = list(pkg.get("dependencies", {})) + JS_RUNTIMES
    seen, out = set(), []
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        seen.add(name)
        folder = ROOT / "node_modules" / name
        manifest = folder / "package.json"
        if not manifest.exists():
            continue
        m = json.loads(manifest.read_text())
        lic = m.get("license")
        if isinstance(lic, dict):
            lic = lic.get("type")
        repo = m.get("repository")
        if isinstance(repo, dict):
            repo = repo.get("url")
        author = m.get("author")
        if isinstance(author, dict):
            author = author.get("name")
        out.append(
            {
                "name": name,
                "authors": [author] if author else [],
                "version": m.get("version", ""),
                "license": lic or "see license text",
                "source": (repo or f"https://www.npmjs.com/package/{name}").removeprefix("git+"),
                "texts": license_texts(folder),
            }
        )
        todo.extend(m.get("dependencies", {}))
    return out


# MARK: - PDFium


def pdfium_component() -> dict:
    folder = ROOT / "src-tauri" / "pdfium"
    version = (folder / "VERSION").read_text().strip() if (folder / "VERSION").exists() else "?"
    texts = []
    main = folder / "LICENSE-pdfium.txt"
    if main.exists():
        texts.append(main.read_text(errors="replace").strip())
    lic_dir = folder / "licenses"
    if lic_dir.is_dir():
        for f in sorted(lic_dir.iterdir()):
            if f.is_file():
                texts.append(f"{f.stem}:\n\n{f.read_text(errors='replace').strip()}")
    if not texts:
        raise SystemExit("PDFium's license files are missing: run scripts/fetch-pdfium.sh first")
    return {
        "name": "PDFium (prebuilt by pdfium-binaries)",
        "version": f"chromium/{version}",
        "license": "BSD-3-Clause and others (below)",
        "source": "https://github.com/bblanchon/pdfium-binaries",
        "texts": texts,
    }


# MARK: - Writing


def main() -> None:
    components = [pdfium_component()] + sorted(
        rust_components() + js_components(), key=lambda c: (c["name"].lower(), c["version"])
    )
    fill_missing(components)
    groups: dict[str, dict] = {}
    for c in components:
        body = "\n\n---\n\n".join(c["texts"]) if c["texts"] else ""
        key = hashlib.sha256(body.encode()).hexdigest()[:12] if body else "none"
        groups.setdefault(key, {"text": body, "users": []})["users"].append(c)

    lines = [
        "# Third-party notices",
        "",
        "Rupert (MIT license) includes the open-source components below. Each keeps its own",
        "license; their texts follow the list. Generated by `scripts/third-party-notices.py`.",
        "",
        "| Component | Version | License |",
        "|---|---|---|",
    ]
    for c in components:
        lines.append(f"| [{c['name']}]({c['source']}) | {c['version']} | {c['license']} |")
    lines.append("")
    for g in sorted(groups.values(), key=lambda g: g["users"][0]["name"].lower()):
        names = ", ".join(f"{u['name']} {u['version']}" for u in g["users"])
        lines += ["", "---", "", f"### {names}", ""]
        if g["text"]:
            lines += ["```text", g["text"].replace("```", "'''"), "```"]
        else:
            lines.append(
                "No license file is included in the package; its license is given above "
                "(see the component's source for the text)."
            )
    OUT.write_text("\n".join(lines) + "\n")
    print(f"{OUT.relative_to(ROOT)}: {len(components)} components, {len(groups)} license texts")


if __name__ == "__main__":
    main()
