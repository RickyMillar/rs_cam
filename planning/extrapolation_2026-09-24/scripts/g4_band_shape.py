#!/usr/bin/env python3
"""G4 (derived, read-only): the band shape of every LUT wood row that prints
both limits. Prints min/max ratio and the absolute width (in inches, the
charts' own unit) per source and tool family. Also counts one-value rows by
their encoding (min absent vs min == max).

Usage: python3 g4_band_shape.py > ../fetch/G4/band_shape_derived.txt
"""
import glob
import json
import os
import statistics as st
from collections import defaultdict

ROOT = os.path.join(os.path.dirname(__file__), "../../../crates/rs_cam_core/data/vendor_lut/observations")
WOOD = {"softwood", "hardwood", "mdf", "hdf", "particleboard", "plywood_hardwood", "plywood_softwood"}
IN = 25.4

groups = defaultdict(list)
encodings = defaultdict(lambda: defaultdict(int))
for path in sorted(glob.glob(os.path.join(ROOT, "*.json"))):
    for o in json.load(open(path))["observations"]:
        if o.get("material_family") not in WOOD:
            continue
        mn, mx = o.get("chipload_min_mm_tooth"), o.get("chipload_max_mm_tooth")
        if mx is None:
            continue
        key = (o["tool_family"], o["source_id"])
        if mn is None:
            encodings[key]["min absent"] += 1
        elif mn >= mx:
            encodings[key]["min == max"] += 1
        else:
            encodings[key]["band"] += 1
            groups[key].append((mn, mx, o.get("evidence_grade"), o.get("row_kind")))

print("# G4 band shape, DERIVED from the LUT (not printed by any vendor)")
print("# columns: tool_family source n grades | min/max ratio median [lo-hi] | width in in median [lo-hi]")
for key in sorted(groups):
    rows = groups[key]
    ratios = [a / b for a, b, _, _ in rows]
    widths = [(b - a) / IN for a, b, _, _ in rows]
    grades = "".join(sorted({g or "?" for _, _, g, _ in rows}))
    print(f"{key[0]:18s} {key[1]:48s} n={len(rows):3d} g={grades:3s} | "
          f"ratio {st.median(ratios):.2f} [{min(ratios):.2f}-{max(ratios):.2f}] | "
          f"width {st.median(widths):.4f} [{min(widths):.4f}-{max(widths):.4f}]")
print()
print("# one-value encodings per (tool_family, source)")
for key in sorted(encodings):
    e = encodings[key]
    if e.get("min absent") or e.get("min == max"):
        print(f"{key[0]:18s} {key[1]:48s} " + ", ".join(f"{k}={v}" for k, v in sorted(e.items())))
