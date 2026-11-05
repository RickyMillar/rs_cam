#!/usr/bin/env python3
"""Check every reference in the P12 hand-off files.

The script reads the four hand-off files. It checks:

- each `path:line` and `path:line-line` (repo-root relative): the file
  exists and has that line (both ends of a range);
- each commit hash (a 9- or 40-character hex word with a digit and a
  letter): `git cat-file -e <hash>^{commit}` passes;
- each bare repo path in backticks that names a file or a folder: it exists.

Run from anywhere: `python3 -I planning/feeds_contract_2026-11-03/handoff/check_refs.py`.
Exit 0 when every reference resolves; exit 1 and print each failure else.
"""

import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
)
FILES = ["INDEX.md", "WHAT_IS_DONE.md", "MODELS.md", "VERIFICATION_BRIEF.md"]

# A repo path: starts with a known top folder or a root file, then a path.
TOP = r"(?:crates|planning|scripts|fixtures|tools|CREDITS\.md|FEATURE_CATALOG\.md|CLAUDE\.md|Cargo\.toml)"
PATH_LINE = re.compile(r"(?<![\w/.-])(" + TOP + r"[\w./-]*):(\d+)(?:-(\d+))?")
BARE_PATH = re.compile(r"`(" + TOP + r"(?:/[\w.-]+)*/?)`")
HASH = re.compile(r"(?<![\w/.-])([0-9a-f]{9}|[0-9a-f]{40})(?![\w/-])")


def line_count(path: pathlib.Path) -> int:
    with path.open("rb") as f:
        return sum(1 for _ in f)


def main() -> int:
    failures = []
    counts = {"path_line": 0, "hash": 0, "bare_path": 0}
    seen_hash = {}
    for name in FILES:
        doc = HERE / name
        if not doc.exists():
            failures.append(f"{name}: file missing")
            continue
        text = doc.read_text(encoding="utf-8")
        for m in PATH_LINE.finditer(text):
            rel, a, b = m.group(1).rstrip("."), int(m.group(2)), m.group(3)
            counts["path_line"] += 1
            p = ROOT / rel
            if not p.is_file():
                failures.append(f"{name}: {m.group(0)}: no such file")
                continue
            n = line_count(p)
            for ln in [a] + ([int(b)] if b else []):
                if ln < 1 or ln > n:
                    failures.append(f"{name}: {m.group(0)}: line {ln} > {n} lines")
        for m in BARE_PATH.finditer(text):
            rel = m.group(1)
            counts["bare_path"] += 1
            if not (ROOT / rel).exists():
                failures.append(f"{name}: `{rel}`: no such path")
        for m in HASH.finditer(text):
            h = m.group(1)
            if not (re.search(r"\d", h) and re.search(r"[a-f]", h)):
                continue
            counts["hash"] += 1
            if h not in seen_hash:
                r = subprocess.run(
                    ["git", "-C", str(ROOT), "cat-file", "-e", f"{h}^{{commit}}"],
                    capture_output=True,
                )
                seen_hash[h] = r.returncode == 0
            if not seen_hash[h]:
                failures.append(f"{name}: {h}: not a commit")
    for f in failures:
        print("FAIL", f)
    total = sum(counts.values())
    print(
        f"checked {total} references: {counts['path_line']} path:line, "
        f"{counts['hash']} commit hashes ({len(seen_hash)} distinct), "
        f"{counts['bare_path']} bare paths; {len(failures)} failures"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
