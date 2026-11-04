#!/usr/bin/env python3
"""State the operator's required tool facts (feeds contract P7, ruling D12).

Ruling D12 makes `centre_cutting` (every tool) and `v_tip_diameter` (a V-bit)
required inputs: an op whose tool does not state them does not generate. The
operator stated the facts on 2026-11-04 (`planning/feeds_contract_2026-11-03/
phases/P7.md`, "Operator tool facts for D12"). This script writes them into
the tool tables of a project file (`[[tools]]`), a tool catalogue
(`[[tools]]`) and a CLI job file (`[tools.<name>]`):

- a tool that already holds `centre_cutting` keeps it (the 18 mm 2F end mill
  states `{ stated = "no", .. }`);
- a 25 mm 2F end mill with a 19 mm flute, and a 25 mm 3F end mill with a 7 mm
  flute, stay Unknown: the operator has not given their blade protrusion.
  The script lists each one it finds;
- every other tool gets `centre_cutting = { stated = "yes" }`;
- a V-bit with no `v_tip_diameter` gets `{ stated = 0.3 }` when it is the
  3.175 mm 20 deg V-bit (diameter 3.175, included angle 20; each match is
  listed), else `{ stated = 0.0 }` (a sharp point).

It never changes a key that is present. It is idempotent: a second run
changes nothing. It edits text line by line, so comments and the order of
keys stay; each key goes in as one inline table at the end of the main body
of its tool table. After each edit it parses the old and the new text
(tomllib) and checks that the new text only adds those keys. It prints one
line per changed file.

Usage:
    scripts/state_tool_facts_d12.py PATH...          # the named files
    scripts/state_tool_facts_d12.py --repo           # every checked-in *.toml
    scripts/state_tool_facts_d12.py --backup PATH..  # also write PATH.bak first
    scripts/state_tool_facts_d12.py --dry-run ...    # report, write nothing
"""

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

# The operator's statements (P7.md, "Operator tool facts for D12").
TIP_V20 = 0.3
TIP_SHARP = 0.0
V20_DIAMETER = 3.175
V20_ANGLE = 20.0
# (diameter, flute count, flute length) of the bits whose protrusion is not
# given: they stay Unknown.
PROTRUSION_NOT_GIVEN = ((25.0, 2, 19.0), (25.0, 3, 7.0))

# Files that the script does not touch: the K3 test matrix (its shim states
# each fact), as `migrate_tool_facts_k3.py` excludes it.
EXCLUDED = ("crates/rs_cam_core/tests/literature_matrix/cells.toml",)

HEADER = re.compile(r"^\s*(\[\[?)\s*([^\]]+?)\s*(\]\]?)\s*(#.*)?$")
KEY = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=")
EPS = 1e-9


def fact_value(value):
    """The number of a fact table or a bare value; None when absent."""
    if isinstance(value, dict):
        for kind in ("stated", "catalogue", "estimated"):
            if kind in value:
                return value[kind]
        return None
    return value


def close(a, b):
    return a is not None and b is not None and abs(float(a) - float(b)) < EPS


def header_tool(header_text, brackets):
    """('project', None) for [[tools]], ('job', name) for [tools.<name>],
    ('sub', None) for a sub-table of a tool, else (None, None)."""
    parts = [p.strip().strip('"') for p in header_text.split(".")]
    if brackets == "[[" and parts == ["tools"]:
        return "project", None
    if brackets == "[" and parts and parts[0] == "tools":
        if len(parts) == 2:
            # `[tools.stickout]` under a `[[tools]]` table is a sub-table of
            # the current project tool; `[tools.<name>]` in a job file is a
            # tool. The caller tells them apart by the file's tools type.
            return "job_or_sub", parts[1]
        return "sub", None
    return None, None


def decide(tool, label, report):
    """The lines to add for one tool (dict from tomllib)."""
    add = []
    diameter = tool.get("diameter")
    kind = tool.get("type")
    if "centre_cutting" not in tool:
        flutes = fact_value(tool.get("flute_count"))
        flute_len = fact_value(tool.get("cutting_length"))
        if any(
            close(diameter, d) and close(flutes, n) and close(flute_len, f)
            for d, n, f in PROTRUSION_NOT_GIVEN
        ):
            report["unknown"].append(f"{label}: {tool.get('name', '?')!r}")
        else:
            add.append('centre_cutting = { stated = "yes" }')
    if kind == "v_bit" and "v_tip_diameter" not in tool:
        angle = tool.get("included_angle")
        if close(diameter, V20_DIAMETER) and close(angle, V20_ANGLE):
            report["v20"].append(f"{label}: {tool.get('name', '?')!r}")
            add.append(f"v_tip_diameter = {{ stated = {TIP_V20} }}")
        else:
            add.append(f"v_tip_diameter = {{ stated = {TIP_SHARP} }}")
    return add


def state_text(text, path, report):
    """The new text and the count of added keys."""
    data = tomllib.loads(text)
    tools = data.get("tools")
    if isinstance(tools, list):
        mode = "project"
    elif isinstance(tools, dict):
        mode = "job"
    else:
        return text, 0
    lines = text.split("\n")
    # The tool tables in text order: (index of the header line, end of the
    # main body = the line before the next header, the tool dict, label).
    tables = []
    project_index = -1
    current = None
    for i, line in enumerate(lines):
        m = HEADER.match(line)
        if not m or line.lstrip().startswith("#"):
            continue
        if current is not None:
            current[1] = i
            tables.append(current)
            current = None
        kind, name = header_tool(m.group(2), m.group(1))
        if mode == "project" and kind == "project":
            project_index += 1
            current = [i, len(lines), tools[project_index], f"{path} tools[{project_index}]"]
        elif mode == "job" and kind == "job_or_sub" and isinstance(tools.get(name), dict):
            current = [i, len(lines), tools[name], f"{path} tools.{name}"]
    if current is not None:
        tables.append(current)
    if mode == "project" and project_index + 1 != len(tools):
        raise ValueError(f"{path}: {len(tools)} tools but {project_index + 1} [[tools]] headers")
    inserts = []
    for start, end, tool, label in tables:
        add = decide(tool, label, report)
        if not add:
            continue
        # After the last key line of the main body (before trailing blank
        # and comment lines, which often head the next table).
        last = start
        for j in range(start + 1, end):
            if KEY.match(lines[j]):
                last = j
        inserts.append((last + 1, add))
    added = 0
    for at, add in sorted(inserts, reverse=True):
        lines[at:at] = add
        added += len(add)
    new_text = "\n".join(lines)
    check(data, tomllib.loads(new_text), path)
    return new_text, added


def check(old, new, path):
    """The new text holds the old values and only adds the D12 keys."""
    allowed = {"centre_cutting", "v_tip_diameter"}

    def tools_of(d):
        t = d.get("tools")
        return list(t) if isinstance(t, list) else [t[k] for k in sorted(t)] if t else []

    old_rest = {k: v for k, v in old.items() if k != "tools"}
    new_rest = {k: v for k, v in new.items() if k != "tools"}
    if old_rest != new_rest:
        raise ValueError(f"{path}: a value outside the tool tables changed")
    for a, b in zip(tools_of(old), tools_of(new), strict=True):
        for key in set(a) | set(b):
            if a.get(key) != b.get(key) and not (key in allowed and key not in a):
                raise ValueError(f"{path}: tool key {key} changed")


def repo_files():
    out = subprocess.run(
        ["git", "ls-files", "*.toml"], capture_output=True, text=True, check=True
    ).stdout.split()
    return [p for p in out if p not in EXCLUDED]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("paths", nargs="*")
    ap.add_argument("--repo", action="store_true", help="every checked-in *.toml")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--backup", action="store_true")
    args = ap.parse_args()
    paths = repo_files() if args.repo else args.paths
    report = {"unknown": [], "v20": []}
    total = 0
    changed = 0
    for p in paths:
        path = Path(p)
        try:
            text = path.read_text()
            new_text, added = state_text(text, p, report)
        except tomllib.TOMLDecodeError:
            continue
        if added == 0:
            continue
        total += added
        changed += 1
        print(f"{p}: +{added} keys{' (dry run)' if args.dry_run else ''}")
        if args.dry_run:
            continue
        if args.backup:
            path.with_suffix(path.suffix + ".bak").write_text(text)
        path.write_text(new_text)
    print(f"{changed} files, {total} keys added")
    for line in report["unknown"]:
        print(f"left Unknown (protrusion not given): {line}")
    for line in report["v20"]:
        print(f"3.175 mm 20 deg V-bit, tip {TIP_V20}: {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
