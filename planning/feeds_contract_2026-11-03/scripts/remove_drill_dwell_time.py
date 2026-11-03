#!/usr/bin/env python3
"""Remove the retired Drill key `dwell_time` from the tracked project TOML files.

Feeds contract P0, decision D2 (2026-11-03): `DrillConfig::dwell_time` left the
format. The loader refuses the key (`RETIRED_OP_ENTRIES`). This script removes
the key line from each tracked `*.toml` file that holds it, in the same commit
(ruling Q9).

The script changes only the `dwell_time = ...` lines. It then parses the old and
the new text and checks that the only difference is the key in the
`operation.params` table of a Drill toolpath. A file that fails the check stays
as it was, and the script names it.

The script does not edit the operator files in SKIP (uncommitted operator
state). It lists their hits for the lead.

Usage (from the repository root):

    python3 planning/feeds_contract_2026-11-03/scripts/remove_drill_dwell_time.py [--dry-run]
"""

import copy
import re
import subprocess
import sys
import tomllib
from pathlib import Path

KEY_LINE = re.compile(r"^\s*dwell_time\s*=.*\n?", re.MULTILINE)
SKIP = {
    "planning/rivmap_block_job/nz_wanaka_21_operator.toml",
    "planning/rivmap_block_job/testpiece.toml",
}


def without_drill_dwell(doc: dict) -> dict:
    """The parsed file less `dwell_time` in each Drill `operation.params`."""
    out = copy.deepcopy(doc)
    for setup in out.get("setups", []):
        for tp in setup.get("toolpaths", []):
            op = tp.get("operation", {})
            if op.get("kind") == "drill":
                op.get("params", {}).pop("dwell_time", None)
    return out


def main() -> int:
    dry = "--dry-run" in sys.argv
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    files = subprocess.check_output(["git", "ls-files", "*.toml"], cwd=root, text=True).split()
    changed, skipped, failed = [], [], []
    for rel in files:
        path = root / rel
        if not path.is_file():
            continue
        old = path.read_text()
        if not KEY_LINE.search(old):
            continue
        if rel in SKIP:
            skipped.append(rel)
            continue
        new = KEY_LINE.sub("", old)
        try:
            ok = tomllib.loads(new) == without_drill_dwell(tomllib.loads(old))
        except tomllib.TOMLDecodeError:
            ok = False
        if not ok:
            failed.append(rel)
            continue
        changed.append(rel)
        if not dry:
            path.write_text(new)
    print(f"{'would change' if dry else 'changed'}: {len(changed)} files")
    for rel in changed:
        print(f"  {rel}")
    print(f"operator files not edited (hits for the lead): {len(skipped)}")
    for rel in skipped:
        print(f"  {rel}")
    print(f"refused (the key is not only on a Drill): {len(failed)}")
    for rel in failed:
        print(f"  {rel}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
