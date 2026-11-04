#!/usr/bin/env python3
"""D9 (2026-11-04): remove the retired key `aggressiveness` from the
tracked project files.

The loader refuses a project file that holds the key (ruling Q9 style), so
every checked-in project must lose it in the same change. The script reads
the tracked .toml list from git, skips planning/adaptive_research_2026-10-23/
(another programme owns it), deletes each line `aggressiveness = <number>`
and prints each path it changed. A second run changes nothing.

Run from the repository root:
    python3 planning/feeds_contract_2026-11-03/scripts/d9_drop_aggressiveness.py
"""

import re
import subprocess
import sys

SKIP = "planning/adaptive_research_2026-10-23/"
LINE = re.compile(r"^[ \t]*aggressiveness[ \t]*=[ \t]*[-+0-9.eE]+[ \t]*\r?\n", re.M)


def main() -> int:
    files = subprocess.run(
        ["git", "ls-files", "--", "*.toml"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split()
    changed = 0
    for path in files:
        if path.startswith(SKIP):
            continue
        with open(path, encoding="utf-8") as f:
            text = f.read()
        new, n = LINE.subn("", text)
        if n == 0:
            continue
        if n != 1:
            print(f"{path}: {n} lines; expected 1", file=sys.stderr)
            return 1
        with open(path, "w", encoding="utf-8") as f:
            f.write(new)
        changed += 1
        print(path)
    print(f"{changed} files changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
