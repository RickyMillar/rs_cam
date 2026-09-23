#!/usr/bin/env python3
"""Phase 0 inventory: map every matrix cell to a gap group (PLAN section 2).

Reads the FM1 matrix CSV and the vendor LUT observations. Writes
inventory_cells.csv (one row per cell that is not a printed in-range row
or a registry tool refusal) and prints the per-group counts.
"""
import collections, csv, glob, json, math, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
MATRIX = ROOT / "planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"
OBS = ROOT / "crates/rs_cam_core/data/vendor_lut/observations"
OUT = pathlib.Path(__file__).resolve().parents[1] / "inventory_cells.csv"

obs = {}
for f in glob.glob(str(OBS / "*.json")):
    d = json.load(open(f))
    for o in d["observations"] if isinstance(d, dict) else d:
        obs[o["observation_id"]] = o

# The query Janka of the four FM1 materials (GenericSoftwood, GenericHardwood,
# SheetGoodKind::Mdf, PlywoodGrade::BalticBirch) and the row-side default the
# lookup uses when a row carries no hardness (vendor_lookup::family_default_janka).
# The two MDF values differ (1100 vs 700): an MDF row scales on an MDF query.
QUERY_JANKA = {"softwood": 600.0, "hardwood": 1450.0, "mdf": 1100.0, "plywood_hardwood": 1200.0}
ROW_JANKA = {"softwood": 500.0, "hardwood": 1290.0, "plywood_softwood": 550.0,
             "plywood_hardwood": 1100.0, "mdf": 700.0, "hdf": 900.0, "particleboard": 600.0}

REASON_GROUP = [
    ("plunge drill", "G6", "drill"),
    ("MDF or plywood", "G2", "category (V-bit, no MDF/ply row)"),
    ("ball-nose cutter on adaptive, pocket, contour or trace passes in plywood", "G2", "category (ball, no ply row)"),
    ("ball-nose cutter on adaptive, pocket, contour or trace passes in MDF", "G2", "category (ball, MDF formula < 0.5x chart)"),
    ("bull-nose", "G3", "family (bull finish)"),
    ("V-bit on adaptive", "G3", "family (V-bit adaptive)"),
    ("V-bit on waterline", "G3", "family (V-bit 3D finish)"),
    ("V-bit on parallel", "G5", "engaged geometry (V-bit width)"),
    ("tapered ball-nose on waterline", "G3", "family (taper contour finish)"),
    ("tapered ball-nose on adaptive", "G3", "family (taper contour/trace, no row in that family)"),
    ("No agent judgement", "G3", "never judged (family)"),
]

rows = list(csv.DictReader(open(MATRIX)))
out, counts = [], collections.Counter()
for x in rows:
    group = sub = None
    if x["status"] != "ok":
        t = x["refusal_text"]
        if "Suggest has no basis" not in t:
            counts[("-", "registry tool rule")] += 1
            continue
        for key, g, s in REASON_GROUP:
            if key in t:
                group, sub = g, s
                break
        else:
            group, sub = "??", t[:80]
    elif x["support_arm"] == "FormulaOnly":
        counts[("-", "formula judged BACKED")] += 1
        continue
    else:
        o = obs.get(x["lut_observation_id"], {})
        bandless = x["chipload_bounds_min_mm"] in ("", x["chipload_bounds_max_mm"])
        tags = []
        if x["lut_is_extrapolated"] == "true":
            d_row = o.get("diameter_mm") or 0.0
            d_ratio = float(x["diameter_mm"]) / d_row if d_row else 1.0
            h_row = o.get("hardness_value") or ROW_JANKA.get(o.get("material_family"), 0.0)
            h_ratio = h_row / QUERY_JANKA[x["material"]] if h_row else 1.0
            axis = "G1" if abs(math.log(d_ratio)) > abs(math.log(h_ratio)) else "G2h"
            tags.append((axis, f"extrapolated d_ratio={d_ratio:.2f} h_ratio={h_ratio:.2f} row_mat={o.get('material_family')}"))
        if bandless:
            if x["chipload_source"].startswith("FormulaFallback"):
                tags.append(("G5", "RPM-only V-bit anchor, chipload from formula"))
            else:
                tags.append(("G4", "bandless printed row"))
        if not tags:
            counts[("-", "printed row in range")] += 1
            continue
        for g, s in tags:
            counts[(g, s.split(" d_ratio")[0])] += 1
            out.append({**{k: x[k] for k in ("tool_type", "diameter_mm", "operation", "material", "status", "lut_observation_id")}, "group": g, "detail": s})
        continue
    counts[(group, sub)] += 1
    out.append({**{k: x[k] for k in ("tool_type", "diameter_mm", "operation", "material", "status", "lut_observation_id")}, "group": group, "detail": sub})

with open(OUT, "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(out[0].keys()))
    w.writeheader(); w.writerows(out)
for k, v in sorted(counts.items()):
    print(f"{v:4d}  {k[0]:4s} {k[1]}")
print("total", sum(counts.values()) - sum(1 for x in out) + len(rows) * 0, "cells", len(rows))
