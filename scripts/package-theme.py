#!/usr/bin/env python3
"""Package a theme source directory into an installable ZIP.

A theme package is a ZIP whose root contains `theme.json` plus the files it
references (`styles.css`, `code.js`, optional preview image).

Usage:
    scripts/package-theme.py                        # Hallows -> examples/themes/hallows.zip
    scripts/package-theme.py <source-dir> <out.zip>
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = ROOT / "examples/themes/hallows"
DEFAULT_OUTPUT = ROOT / "examples/themes/hallows.zip"


def main() -> int:
    source = Path(sys.argv[1]).expanduser().resolve() if len(sys.argv) > 1 else DEFAULT_SOURCE
    output = Path(sys.argv[2]).expanduser().resolve() if len(sys.argv) > 2 else DEFAULT_OUTPUT
    if not (source / "theme.json").is_file():
        print(f"no theme.json in {source}", file=sys.stderr)
        return 1
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(source.rglob("*")):
            if not path.is_file() or path.name.startswith(".") or path.suffix == ".zip":
                continue
            archive.write(path, path.relative_to(source).as_posix())
    print(f"packed {source} -> {output} ({output.stat().st_size / 1024:.0f} KB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
