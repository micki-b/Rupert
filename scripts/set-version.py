#!/usr/bin/env python3
"""Sets Rupert's version everywhere it's written, so they never disagree:

- Cargo.toml (`[workspace.package] version`, which every crate inherits)
- package.json and package-lock.json (the root package)
- src-tauri/tauri.conf.json (desktop) and mobile/tauri.conf.json (iPhone and
  Android, whose versionName comes from it)
- Cargo.lock (refreshed by cargo)

Usage: scripts/set-version.py 0.1.1

Only the version line is changed; the files keep their formatting. Apple's
build numbers and Android's version codes are separate and made by the build
scripts (a UTC timestamp).
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Each file with the pattern of its version line. The first match is the one
# that's replaced (in package-lock.json the first two: the lock file's own
# entry and the root package's).
FILES = [
    ("Cargo.toml", r'(?ms)(^\[workspace\.package\].*?^version = ")[^"]*(")', 1),
    ("package.json", r'(?m)(^  "version": ")[^"]*(")', 1),
    ("package-lock.json", r'(?m)(^  "version": "|^      "version": ")[^"]*(")', 2),
    ("src-tauri/tauri.conf.json", r'(?m)(^  "version": ")[^"]*(")', 1),
    ("mobile/tauri.conf.json", r'(?m)(^  "version": ")[^"]*(")', 1),
]


def main() -> int:
    if len(sys.argv) != 2 or not re.fullmatch(r"\d+\.\d+\.\d+", sys.argv[1]):
        print("Usage: scripts/set-version.py MAJOR.MINOR.PATCH (e.g. 0.1.1)", file=sys.stderr)
        return 2
    version = sys.argv[1]

    for name, pattern, count in FILES:
        path = ROOT / name
        text = path.read_text(encoding="utf-8")
        new, n = re.subn(pattern, rf"\g<1>{version}\g<2>", text, count=count)
        if n != count:
            print(f"{name}: version line not found; nothing changed there", file=sys.stderr)
            return 1
        path.write_text(new, encoding="utf-8")
        print(f"{name}: {version}")

    # Cargo.lock lists the workspace crates' versions too.
    subprocess.run(["cargo", "update", "--workspace", "--offline", "--quiet"], cwd=ROOT, check=True)
    print(f"Cargo.lock: {version}")
    print("\nNext: move the CHANGELOG.md \"Unreleased\" entries under the new version, commit,")
    print(f"then tag and push: git tag v{version} && git push origin main v{version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
