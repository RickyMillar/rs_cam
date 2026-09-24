#!/usr/bin/env python3
"""G4 Phase 2 trend (read-only on the LUT). DERIVED numbers only.

Question: when a chart prints both chipload limits, what is the band shape
(min/max ratio and absolute width), per tool family, vendor, material
category and size band? What minimum would each candidate rule give for the
83 bandless G4 cells in inventory_cells.csv?

Inputs (read-only):
  crates/rs_cam_core/data/vendor_lut/observations/*.json
  planning/extrapolation_2026-09-24/fetch/G4/verified_rows.json (0 rows today)
  planning/extrapolation_2026-09-24/inventory_cells.csv

Filter for a "printed band" row:
  - wood material family, min and max present, min < max;
  - evidence_grade a or b;
  - notes / machine_assumption / source_page do not say the band is not
    printed (regex NOT_PRINTED below).
Printed cells are de-duplicated on (source, family, subfamily, diameter,
flutes, material, min, max): one printed chart cell that the LUT copies into
two operation rows counts once.

Usage: python3 trend_g4.py
"""
import csv
import glob
import json
import re
import statistics as st
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROG = HERE.parent
ROOT = PROG.parents[1]
OBS = ROOT / "crates/rs_cam_core/data/vendor_lut/observations"
VERIFIED = PROG / "fetch/G4/verified_rows.json"
CELLS = PROG / "inventory_cells.csv"

IN = 25.4
W002 = 0.002 * IN  # 0.0508 mm, the Onsrud two-tick width (derived constant)
WOOD = {"softwood", "hardwood", "mdf", "hdf", "particleboard",
        "plywood_hardwood", "plywood_softwood"}
CATEGORY = {"softwood": "solid", "hardwood": "solid", "mdf": "sheet",
            "hdf": "sheet", "particleboard": "sheet",
            "plywood_hardwood": "plywood", "plywood_softwood": "plywood"}
NOT_PRINTED = re.compile(
    r"not printed|no printed row|repo-authored|derived \(reduced\)|extrapolated",
    re.I)
R1_LO, R1_HI = 0.5, 2.0  # FORMULA_BACKING_v2 judgement threshold
RUBBING_FLOOR = 0.025  # feeds::RUBBING_FLOOR_MM_TOOTH


def size_band(d):
    if d is None:
        return "n/a"
    if d < 3.0:
        return "a:<3"
    if d < 6.2:
        return "b:3-6"
    if d < 10.0:
        return "c:6.35-9.5"
    return "d:>=12.7"


def load_rows():
    rows = []
    for path in sorted(glob.glob(str(OBS / "*.json"))):
        for o in json.load(open(path))["observations"]:
            o["_file"] = Path(path).name
            rows.append(o)
    try:
        for o in json.load(open(VERIFIED)).get("observations", []):
            o["_file"] = "verified_rows.json"
            rows.append(o)
    except FileNotFoundError:
        pass
    return rows


def q(values):
    v = sorted(values)
    if len(v) == 1:
        return v[0], v[0], v[0]
    q1, med, q3 = st.quantiles(v, n=4, method="inclusive")
    return q1, med, q3


def stat_line(label, values, fmt="{:.2f}"):
    if not values:
        return f"  {label:44s} n=  0"
    q1, med, q3 = q(values)
    f = lambda x: fmt.format(x)
    return (f"  {label:44s} n={len(values):3d}  median {f(med)}  "
            f"Q1-Q3 {f(q1)}-{f(q3)}  range {f(min(values))}-{f(max(values))}")


def table(title, cells, keyfn, metric, fmt="{:.2f}"):
    print(f"\n### {title}")
    groups = defaultdict(list)
    for c in cells:
        groups[keyfn(c)].append(metric(c))
    for k in sorted(groups, key=str):
        print(stat_line(" / ".join(map(str, k)) if isinstance(k, tuple) else str(k),
                        groups[k], fmt))


def slope(xs, ys):
    mx, my = st.mean(xs), st.mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return float("nan")
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx


def main():
    rows = load_rows()
    wood = [o for o in rows if o.get("material_family") in WOOD
            and o.get("chipload_max_mm_tooth") is not None]

    # --- 1. The filter -----------------------------------------------------
    excluded = Counter()
    printed = []
    one_value = Counter()
    for o in wood:
        mn, mx = o.get("chipload_min_mm_tooth"), o["chipload_max_mm_tooth"]
        if mn is None:
            one_value[(o["tool_family"], o["source_id"], "min absent")] += 1
            continue
        if mn >= mx:
            one_value[(o["tool_family"], o["source_id"], "min == max")] += 1
            continue
        text = " ".join(str(o.get(k, "")) for k in ("notes", "machine_assumption", "source_page"))
        if o.get("evidence_grade") not in ("a", "b"):
            excluded[(o["tool_family"], o["source_id"], "grade " + str(o.get("evidence_grade")))] += 1
            continue
        if NOT_PRINTED.search(text):
            excluded[(o["tool_family"], o["source_id"], "notes say not printed")] += 1
            continue
        printed.append(o)

    print("# G4 trend, DERIVED from the LUT. No vendor prints these statistics.")
    print(f"# wood rows with a max: {len(wood)}; printed band rows kept: {len(printed)}")
    print("\n## 1. Band rows excluded from the trend (not a printed band)")
    for k, v in sorted(excluded.items()):
        print(f"  {v:3d}  {k[0]:18s} {k[1]:40s} {k[2]}")
    print("\n## 1b. One-value wood rows (not in the trend; the G4 population)")
    for k, v in sorted(one_value.items()):
        print(f"  {v:3d}  {k[0]:18s} {k[1]:40s} {k[2]}")

    # --- 2. De-duplicate printed cells --------------------------------------
    seen = {}
    for o in printed:
        key = (o["source_id"], o["tool_family"], o.get("tool_subfamily"), o.get("diameter_mm"),
               o.get("flute_count"), o["material_family"],
               round(o["chipload_min_mm_tooth"], 5), round(o["chipload_max_mm_tooth"], 5))
        seen.setdefault(key, o)
    cells = list(seen.values())
    for c in cells:
        c["_ratio"] = c["chipload_min_mm_tooth"] / c["chipload_max_mm_tooth"]
        c["_width_in"] = (c["chipload_max_mm_tooth"] - c["chipload_min_mm_tooth"]) / IN
        c["_cat"] = CATEGORY[c["material_family"]]
        c["_size"] = size_band(c.get("diameter_mm"))
        c["_vendor"] = c.get("source_vendor") or c["source_id"].split("_")[0]
    print(f"\n## 2. Printed band cells after de-duplication: {len(cells)} (from {len(printed)} rows)")
    for k, v in sorted(Counter((c["tool_family"], c["source_id"]) for c in cells).items()):
        print(f"  {v:3d}  {k[0]:18s} {k[1]}")

    ratio = lambda c: c["_ratio"]
    width = lambda c: c["_width_in"]
    print("\n## 3. min/max ratio")
    table("per tool family", cells, lambda c: c["tool_family"], ratio)
    table("per tool family / vendor", cells, lambda c: (c["tool_family"], c["_vendor"]), ratio)
    table("per tool family / source", cells, lambda c: (c["tool_family"], c["source_id"]), ratio)
    table("per tool family / material category", cells, lambda c: (c["tool_family"], c["_cat"]), ratio)
    table("per tool family / size band (mm)", cells, lambda c: (c["tool_family"], c["_size"]), ratio)
    table("flat_end per vendor / size band", [c for c in cells if c["tool_family"] == "flat_end"],
          lambda c: (c["_vendor"], c["_size"]), ratio)

    print("\n## 4. Absolute band width, inches (the charts' unit)")
    table("per tool family", cells, lambda c: c["tool_family"], width, "{:.4f}")
    table("per tool family / vendor", cells, lambda c: (c["tool_family"], c["_vendor"]), width, "{:.4f}")
    table("flat_end per size band", [c for c in cells if c["tool_family"] == "flat_end"],
          lambda c: c["_size"], width, "{:.4f}")
    print("\n### share of cells whose width is 0.002 in (+/- 0.00005 in)")
    g = defaultdict(list)
    for c in cells:
        g[(c["tool_family"], c["_vendor"])].append(abs(c["_width_in"] - 0.002) < 0.00005)
    for k in sorted(g):
        print(f"  {k[0]:18s} {k[1]:10s} {sum(g[k]):3d} of {len(g[k]):3d}")

    print("\n## 5. Ratio against diameter, flat_end (least-squares slope, per mm; derived)")
    for vendor in sorted({c["_vendor"] for c in cells if c["tool_family"] == "flat_end"}):
        sub = [c for c in cells if c["tool_family"] == "flat_end" and c["_vendor"] == vendor
               and c.get("diameter_mm")]
        if len(sub) >= 3:
            xs = [c["diameter_mm"] for c in sub]
            print(f"  {vendor:10s} n={len(sub):3d}  d {min(xs):.2f}-{max(xs):.2f} mm  "
                  f"ratio slope {slope(xs, [c['_ratio'] for c in sub]):+.4f}/mm  "
                  f"width slope {slope(xs, [c['_width_in'] for c in sub]):+.6f} in/mm")

    # --- 6. The G4 cells and the candidate rules ----------------------------
    obs = {o["observation_id"]: o for o in rows}
    g4 = [r for r in csv.DictReader(open(CELLS)) if r["group"] == "G4"]
    by_row = defaultdict(list)
    for r in g4:
        by_row[r["lut_observation_id"]].append(r)
    print(f"\n## 6. The G4 cells: {len(g4)} cells on {len(by_row)} LUT rows")
    print("### counts by tool / operation / material")
    for axis in ("tool_type", "operation", "material"):
        print(f"  {axis}: " + ", ".join(f"{k} {v}" for k, v in sorted(Counter(r[axis] for r in g4).items())))
    enc = Counter("min absent" if obs[i].get("chipload_min_mm_tooth") is None else "min == max"
                  for r in g4 for i in [r["lut_observation_id"]])
    print(f"  encoding of the anchor row (per cell): {dict(enc)}")

    fam_median = {f: q([c["_ratio"] for c in cells if c["tool_family"] == f])[1]
                  for f in {c["tool_family"] for c in cells}}
    fam_min = {f: min(c["_ratio"] for c in cells if c["tool_family"] == f)
               for f in {c["tool_family"] for c in cells}}
    fam_width = {f: q([c["_width_in"] for c in cells if c["tool_family"] == f])[1] * IN
                 for f in {c["tool_family"] for c in cells}}
    print("\n### candidate rules (all DERIVED; v = the printed value)")
    print("  A  top reading, constant width w: band = [v - w, v]"
          + "  w = family median width, in: " + ", ".join(f"{k} {v / IN:.4f}" for k, v in sorted(fam_width.items())))
    print("  A2 top reading, width 0.002 in (shown where the family median width is not 0.002 in)")
    print("  B  top reading, family median ratio: band = [r_med * v, v]"
          + "  r_med: " + ", ".join(f"{k} {v:.2f}" for k, v in sorted(fam_median.items())))
    print("  B- top reading, lowest printed ratio of the family: band = [r_min * v, v]"
          + "  r_min: " + ", ".join(f"{k} {v:.2f}" for k, v in sorted(fam_min.items())))
    print("  C  start reading, constant width w: band = [v, v + w]")
    print("  C2 start reading, width 0.002 in (shown where the family median width is not 0.002 in)")
    print(f"  R1 check: min/v >= {R1_LO} and max/v <= {R1_HI}; floor check: min >= {RUBBING_FLOOR} mm")
    print("\n  row | cells | family d fl mat grade enc | v mm (in) | A | B | B- | C")
    for oid in sorted(by_row):
        o = obs[oid]
        v = o["chipload_max_mm_tooth"]
        fam = o["tool_family"]
        e = "absent" if o.get("chipload_min_mm_tooth") is None else "min=max"
        w = fam_width[fam]
        rules = {"A": (v - w, v)}
        if abs(w - W002) > 1e-6:
            rules["A2"] = (v - W002, v)
        rules["B"] = (fam_median[fam] * v, v)
        rules["B-"] = (fam_min[fam] * v, v)
        rules["C"] = (v, v + w)
        if abs(w - W002) > 1e-6:
            rules["C2"] = (v, v + W002)
        def fmt(lo, hi):
            flags = []
            if lo <= 0:
                return f"{lo:.4f}-{hi:.4f} (min <= 0) REFUSE"
            if lo / v < R1_LO - 1e-9:
                flags.append("min<0.5v")
            if hi / v > R1_HI + 1e-9:
                flags.append("max>2v")
            if lo < RUBBING_FLOOR - 1e-9:
                flags.append("min<floor")
            return f"{lo:.4f}-{hi:.4f} ({lo / v:.2f}-{hi / v:.2f}v){' ' + ','.join(flags) if flags else ''}"
        cl = sorted({f"{r['tool_type']}:{r['operation']}:{r['material']}" for r in by_row[oid]})
        print(f"\n  {oid}  [{len(by_row[oid])} cells]")
        print(f"    {fam} d={o.get('diameter_mm')} f{o.get('flute_count')} {o['material_family']} "
              f"grade {o.get('evidence_grade')} {o.get('row_kind')} enc={e}  v={v:.4f} mm ({v / IN:.4f} in)")
        for k, (lo, hi) in rules.items():
            print(f"    {k:2s} {fmt(lo, hi)}")
        print("    cells: " + ", ".join(cl))

    # --- 7. Rule A along the whole Spektra one-value series ------------------
    print("\n## 7. Rule A along every Spektra one-value printed value (the range edge)")
    vals = defaultdict(set)
    for o in wood:
        if o["source_id"] == "amana_spektra_spiral_plunge_v24" and (
                o.get("chipload_min_mm_tooth") is None
                or o["chipload_min_mm_tooth"] >= o["chipload_max_mm_tooth"]):
            vals[round(o["chipload_max_mm_tooth"] / IN, 4)].add(o.get("diameter_mm"))
    for vin in sorted(vals):
        lo = vin - 0.002
        print(f"  v {vin:.4f} in  d {sorted(vals[vin])}  A min {lo:+.4f} in  min/v {lo / vin:+.2f}"
              f"{'  REFUSE (<0.5v)' if lo / vin < R1_LO else ''}")

    # --- 8. Where a one-value cell sits against other vendors' bands --------
    print("\n## 8. Each G4 anchor value against printed bands of other sources "
          "(same tool family, material family, diameter within 6 %)")
    tally = Counter()
    done = set()
    for oid in sorted(by_row):
        o = obs[oid]
        v, d = o["chipload_max_mm_tooth"], o.get("diameter_mm")
        anchor = (o["tool_family"], o["material_family"], d)
        hits = [c for c in cells if c["tool_family"] == o["tool_family"]
                and c["material_family"] == o["material_family"] and c.get("diameter_mm") and d
                and abs(c["diameter_mm"] / d - 1) <= 0.06 and c["source_id"] != o["source_id"]]
        print(f"  {oid}: v {v / IN:.4f} in")
        if not hits:
            print("      no printed band at this size and material in another source")
        for c in hits:
            lo, hi = c["chipload_min_mm_tooth"], c["chipload_max_mm_tooth"]
            pos = "below min" if v < lo - 1e-9 else ("above max" if v > hi + 1e-9 else
                  ("= min" if abs(v - lo) < 1e-6 else ("= max" if abs(v - hi) < 1e-6 else "inside")))
            if anchor not in done:
                tally[(o["tool_family"], pos)] += 1
            print(f"      {c['source_id']:48s} {c.get('tool_subfamily')!s:24s} d {c['diameter_mm']:.3f} "
                  f"band {lo / IN:.4f}-{hi / IN:.4f} in  v is {pos}  v/min {v / lo:.2f}  v/max {v / hi:.2f}")

        done.add(anchor)
    print("  tally over unique (family, material, diameter) anchors:")
    for k, n in sorted(tally.items()):
        print(f"    {k[0]:14s} v is {k[1]:10s} {n:3d}")

    # --- 9. The same-sheet witness (AMS-159) --------------------------------
    print("\n## 9. AMS-159 same sheet: 1-flute band against 2-flute one value (wood rows)")
    for o in wood:
        if o["source_id"] == "amana_ams159_vgroove_v2":
            mn, mx = o.get("chipload_min_mm_tooth"), o["chipload_max_mm_tooth"]
            print(f"  {o['observation_id']:40s} f{o.get('flute_count')} "
                  f"{(mn or 0) / IN:.4f}-{mx / IN:.4f} in")


if __name__ == "__main__":
    main()
