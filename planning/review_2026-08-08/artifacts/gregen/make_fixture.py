#!/usr/bin/env python3
"""Make the G-REGEN-RACE scratch fixture from a project file, READ-ONLY.

The source is only ever READ. `planning/airrun_2026-06-01/wanaka.toml` is the
operator's play file and is read-only to every wave — B-4b made a scratch
copy the same way for its own rig, and this is that habit kept.

The fixture keeps exactly ONE toolpath enabled, by name (default "Back Rough",
the `adaptive3d` on the 220k-triangle terrain that B-4b's red names). One
enabled op is the minimum that arms the race: `load_project` marks it stale
with `auto_regen`, the 500 ms sweep puts it on the lane, and it runs long
enough that an agent's `generate_all` lands on a busy lane.

Usage:
    python3 make_fixture.py --source planning/airrun_2026-06-01/wanaka.toml \\
        --keep "Back Rough" --out /tmp/.../gregen_back_rough.toml
"""

import argparse
import os
import re


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", required=True)
    ap.add_argument("--keep", default="Back Rough")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    with open(args.source) as fh:
        lines = fh.readlines()

    # `enabled` is not unique to a toolpath: `[setups.toolpaths.boundary]`
    # and `[setups.toolpaths.debug_options]` carry one each, and flipping
    # those would change what the kept operation DOES. Only the flag in the
    # `[[setups.toolpaths]]` table itself is the on/off switch, so the
    # section header is tracked rather than guessed.
    out, section, current_name, changed = [], None, None, 0
    for line in lines:
        if line.startswith("["):
            section = line.strip()
            if section == "[[setups.toolpaths]]":
                current_name = None
        m = re.match(r'^name = "(.*)"\s*$', line)
        if m and section == "[[setups.toolpaths]]" and current_name is None:
            current_name = m.group(1)
        if (
            line.startswith("enabled = ")
            and section == "[[setups.toolpaths]]"
            and current_name is not None
        ):
            want = "true" if current_name == args.keep else "false"
            new = f"enabled = {want}\n"
            if new != line:
                changed += 1
            out.append(new)
            continue
        out.append(line)

    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w") as fh:
        fh.writelines(out)
    kept, section = 0, None
    for line in out:
        if line.startswith("["):
            section = line.strip()
        if section == "[[setups.toolpaths]]" and line.strip() == "enabled = true":
            kept += 1
    print(f"wrote {args.out}: {kept} enabled toolpath(s), {changed} flag(s) flipped")
    assert kept == 1, f"fixture must have exactly one enabled toolpath, got {kept}"


if __name__ == "__main__":
    main()
