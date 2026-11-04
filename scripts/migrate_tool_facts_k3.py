#!/usr/bin/env python3
"""Migrate tool facts to the K3 file form (feeds contract P6, ruling Q9).

The K3 loader refuses a tool fact as a bare value. This script rewrites, in
the tool tables of a project file, a tool catalogue or a CLI job file:

- each bare fact `key = <number>` as
  `key = { estimated = <number>, basis = "bare number before K3 (2026-11)" }`;
- each `[tools.non_centre_cutting]` table (or inline `non_centre_cutting =
  { .. }`) as `centre_cutting = { stated = "no", centre_core_diameter_mm =
  .., blade_protrusion_mm = .. }`.

It writes no key that was absent. It edits text line by line, so comments and
the order of keys stay. It is idempotent: a second run changes nothing. It
prints one line per changed file. After each edit it parses the old and the
new text (tomllib) and checks that every value is the same.

Usage:
    scripts/migrate_tool_facts_k3.py PATH...         # the named files
    scripts/migrate_tool_facts_k3.py --repo          # every checked-in *.toml
    scripts/migrate_tool_facts_k3.py --backup PATH.. # also write PATH.bak first
    scripts/migrate_tool_facts_k3.py --dry-run ...   # report, write nothing
"""

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

BASIS = "bare number before K3 (2026-11)"

# The fact keys of a project-file or catalogue tool table (`[[tools]]`).
PROJECT_FACT_KEYS = (
    "cutting_length",
    "taper_half_angle",
    "shank_diameter",
    "stickout",
    "flute_count",
    "v_tip_diameter",
)
# The fact keys of a CLI job tool table (`[tools.<name>]`).
JOB_FACT_KEYS = ("flute_count", "taper_angle", "shank_diameter", "stickout")

# Files that the script does not touch (card step 7: a test matrix whose
# shim states each fact).
EXCLUDED = ("crates/rs_cam_core/tests/literature_matrix/cells.toml",)

HEADER = re.compile(r"^\s*(\[\[?)\s*([^\]]+?)\s*(\]\]?)\s*(#.*)?$")
NUMBER = r"[+-]?(?:\d[\d_]*)(?:\.[\d_]*)?(?:[eE][+-]?\d+)?"
BARE = re.compile(r"^(\s*)([A-Za-z_][A-Za-z0-9_]*)(\s*=\s*)(" + NUMBER + r")(\s*)(#.*)?$")
INLINE_NCC = re.compile(r"^(\s*)non_centre_cutting\s*=\s*\{(.*)\}\s*(#.*)?$")
KV = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(" + NUMBER + r")\s*(#.*)?$")


def fact_line(indent, key, value):
    return f'{indent}{key} = {{ estimated = {value}, basis = "{BASIS}" }}'


def centre_line(indent, core, protrusion):
    return (
        f'{indent}centre_cutting = {{ stated = "no", centre_core_diameter_mm = {core}, '
        f"blade_protrusion_mm = {protrusion} }}"
    )


def table_kind(header_text, brackets):
    """'project' for a [[tools]] table, 'job' for [tools.<name>], 'ncc' for a
    non_centre_cutting sub-table, else None."""
    parts = [p.strip().strip('"') for p in header_text.split(".")]
    if brackets == "[[" and parts == ["tools"]:
        return "project"
    if brackets == "[" and parts and parts[0] == "tools":
        if parts[-1] == "non_centre_cutting" and len(parts) in (2, 3):
            return "ncc"
        if len(parts) == 2:
            return "job"
    return None


def migrate_text(text):
    """The migrated text and the count of edits."""
    lines = text.split("\n")
    out = []
    kind = None
    edits = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        m = HEADER.match(line)
        if m and not line.lstrip().startswith("#"):
            kind = table_kind(m.group(2), m.group(1))
            if kind == "ncc":
                # Collect the sub-table body; it ends at the next header.
                body = {}
                j = i + 1
                while j < len(lines) and not HEADER.match(lines[j]):
                    kv = KV.match(lines[j])
                    if kv:
                        body[kv.group(1)] = kv.group(2)
                    elif lines[j].strip() and not lines[j].strip().startswith("#"):
                        raise ValueError(f"unexpected line in non_centre_cutting: {lines[j]!r}")
                    j += 1
                core = body.get("centre_core_diameter_mm")
                prot = body.get("blade_protrusion_mm")
                if core is None or prot is None or len(body) != 2:
                    raise ValueError(f"non_centre_cutting table is not the measurement: {body}")
                # The key goes back into the parent tool table: the header
                # line is gone, so the parent's body continues here.
                out.append(centre_line("", core, prot))
                # Keep every comment and blank line of the old sub-table
                # body, in order (a comment there often heads the next tool).
                out.extend(l for l in lines[i + 1 : j] if not KV.match(l))
                edits += 1
                kind = None
                i = j
                continue
            out.append(line)
            i += 1
            continue
        if kind in ("project", "job"):
            keys = PROJECT_FACT_KEYS if kind == "project" else JOB_FACT_KEYS
            b = BARE.match(line)
            if b and b.group(2) in keys:
                new = fact_line(b.group(1), b.group(2), b.group(4))
                if b.group(6):
                    new += " " + b.group(6)
                out.append(new)
                edits += 1
                i += 1
                continue
            n = INLINE_NCC.match(line)
            if n:
                body = dict(
                    (k.strip(), v.strip())
                    for k, v in (p.split("=", 1) for p in n.group(2).split(",") if p.strip())
                )
                out.append(
                    centre_line(
                        n.group(1),
                        body["centre_core_diameter_mm"],
                        body["blade_protrusion_mm"],
                    )
                )
                edits += 1
                i += 1
                continue
        out.append(line)
        i += 1
    return "\n".join(out), edits


def tool_tables(doc):
    """(path, table) of each tool table of a parsed file."""
    tools = doc.get("tools")
    if isinstance(tools, list):
        return [(f"tools[{i}]", t) for i, t in enumerate(tools)]
    if isinstance(tools, dict):
        return [(f"tools.{k}", t) for k, t in tools.items() if isinstance(t, dict)]
    return []


def check_same(old_text, new_text, path):
    """Every value of the old file is in the new file, in its K3 form."""
    old = tomllib.loads(old_text)
    new = tomllib.loads(new_text)
    old_tools = dict(tool_tables(old))
    new_tools = dict(tool_tables(new))
    if old_tools.keys() != new_tools.keys():
        raise ValueError(f"{path}: the tool tables changed")
    for name, ot in old_tools.items():
        nt = new_tools[name]
        for key, ov in ot.items():
            if key == "non_centre_cutting":
                nv = nt.get("centre_cutting")
                want = {"stated": "no", **ov}
                if nv != want:
                    raise ValueError(f"{path}: {name}.non_centre_cutting {ov} -> {nv}")
                continue
            nv = nt.get(key)
            if isinstance(ov, (int, float)) and isinstance(nv, dict):
                if nv != {"estimated": ov, "basis": BASIS}:
                    raise ValueError(f"{path}: {name}.{key} {ov} -> {nv}")
            elif nv != ov:
                raise ValueError(f"{path}: {name}.{key} {ov} -> {nv}")
        added = set(nt) - set(ot) - {"centre_cutting"}
        if added:
            raise ValueError(f"{path}: {name} gained keys {sorted(added)}")
    old_rest = {k: v for k, v in old.items() if k != "tools"}
    new_rest = {k: v for k, v in new.items() if k != "tools"}
    if old_rest != new_rest:
        raise ValueError(f"{path}: a table other than the tools changed")


def repo_files():
    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
        ).stdout.strip()
    )
    names = subprocess.run(
        ["git", "ls-files", "*.toml"], capture_output=True, text=True, check=True, cwd=root
    ).stdout.split()
    return [root / n for n in names if n not in EXCLUDED]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="*", type=Path)
    ap.add_argument("--repo", action="store_true", help="every checked-in *.toml")
    ap.add_argument("--backup", action="store_true", help="write <file>.bak before a change")
    ap.add_argument("--dry-run", action="store_true", help="report only")
    args = ap.parse_args()
    paths = list(args.paths) + (repo_files() if args.repo else [])
    if not paths:
        ap.error("name a file or pass --repo")
    changed = 0
    for path in paths:
        text = path.read_text()
        try:
            tomllib.loads(text)
        except tomllib.TOMLDecodeError:
            continue  # not TOML this script can read; not a tool file
        new, edits = migrate_text(text)
        if edits == 0:
            continue
        check_same(text, new, path)
        changed += 1
        print(f"{path}: {edits} edit(s)")
        if args.dry_run:
            continue
        if args.backup:
            bak = path.with_name(path.name + ".bak")
            bak.write_text(text)
        path.write_text(new)
    print(f"{changed} file(s) changed", file=sys.stderr)


if __name__ == "__main__":
    main()
