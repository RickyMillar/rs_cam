#!/usr/bin/env python3
"""G2 trend: material category (Phase 2, read-only).

Reads:
  - crates/rs_cam_core/data/vendor_lut/observations/*.json   (the LUT)
  - planning/extrapolation_2026-09-24/fetch/G2/verified_rows.json (91 rows)
  - planning/extrapolation_2026-09-24/inventory_cells.csv       (G2, G2h cells)
  - planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv       (sibling cells)

Writes nothing. Prints the tables that EXTRAPOLATION_G2.md quotes.

Rules of the trend (so no row counts twice and no copy counts as a witness):
  1. A row enters when row_kind is "exact" (grade a or b). Derived grade c rows
     (the Amana v7 x0.16 finish rows, the ZrN tapered seed rows, the Onsrud bull
     seed rows, IDC) and fallback rows (Whiteside Fusion 360) stay out.
  2. The Spektra flat chart prints two columns, "Wood/Plywood" and "MDF/Laminate".
     Every non-MDF Spektra row (exact, or derived grade b) is that one
     "Wood/Plywood" column. They enter as one column "wood_plywood".
     The Amana compression chart prints "Wood", "Plywood" and "MDF/Laminate"; its
     "Wood" rows (labelled hardwood in the LUT) enter as column "wood".
  3. The ZrN charts (amana_zrn_3d_profiling, _v8) print one row "Wood, MDF,
     Sign-Foam". Their exact mdf and softwood rows are one printed cell with two
     labels. They enter as one column "wood_mdf_signfoam" and give no ratio.
  4. The Onsrud laminated tables keep their own columns
     ("particleboard_laminated", "plywood_hardwood_laminated").
  5. A row that the LUT fans out over several operation families is one cell.
     The tool key is (chart set, tool family, subfamily, diameter, flutes, angle).
     The pass role joins the key only where one column prints two values.
  6. mid = (min + max) / 2; a row with one value has mid = max = that value.
Everything this script computes is DERIVED. Printed values carry their row id.
"""
import csv
import glob
import json
import pathlib
import re
import statistics
from collections import Counter, defaultdict

REPO = pathlib.Path(__file__).resolve().parents[3]
PLAN = REPO / "planning" / "extrapolation_2026-09-24"
G2 = PLAN / "fetch" / "G2"
LUT_DIR = REPO / "crates" / "rs_cam_core" / "data" / "vendor_lut" / "observations"
MATRIX = REPO / "planning" / "feeds_matrix_2026-09-23" / "matrix_2026-09-23.csv"
INV = PLAN / "inventory_cells.csv"

WOOD = {"softwood", "hardwood", "mdf", "hdf", "particleboard", "plywood_hardwood", "plywood_softwood"}
BASES = ("hardwood", "softwood", "wood", "wood_plywood")

# The two engine Janka tables (copied from the Rust source).
# material/mod.rs: WoodSpecies::janka_lbf (generic), PlywoodGrade / SheetGoodKind::effective_janka_lbf
QUERY_JANKA = {
    "softwood (GenericSoftwood)": 600.0, "hardwood (GenericHardwood)": 1450.0,
    "plywood_softwood (PlywoodGrade::Softwood)": 600.0,
    "plywood_hardwood (BalticBirch)": 1200.0, "plywood_hardwood (HardwoodFaced)": 1000.0,
    "mdf (SheetGoodKind::Mdf)": 1100.0, "hdf (SheetGoodKind::Hdf)": 1300.0,
    "particleboard (SheetGoodKind::Particleboard)": 750.0,
}
# feeds/vendor_lookup.rs: family_default_janka
ROW_DEFAULT_JANKA = {"softwood": 500.0, "hardwood": 1290.0, "plywood_softwood": 550.0,
                     "plywood_hardwood": 1100.0, "mdf": 700.0, "hdf": 900.0, "particleboard": 600.0}
# The query Janka of the four FM1 materials (INVENTORY, scripts/inventory.py)
FM1_QUERY = {"softwood": 600.0, "hardwood": 1450.0, "mdf": 1100.0, "plywood_hardwood": 1200.0}
HARDNESS_EXP = 0.5  # vendor_lookup.rs CHIPLOAD_HARDNESS_EXPONENT


def load_lut():
    rows = []
    for f in sorted(glob.glob(str(LUT_DIR / "*.json"))):
        d = json.loads(pathlib.Path(f).read_text())
        rows += d["observations"] if isinstance(d, dict) else d
    return rows


def chart_set(src: str) -> str:
    if src.startswith("onsrud_") and src.endswith("_cutting_data") and src != "onsrud_cutting_data_recommendations":
        return "onsrud_wood_sheets"
    return src


def column_of(r: dict):
    """Return (column, why_excluded)."""
    mf = r["material_family"]
    if mf not in WOOD:
        return None, "not wood"
    if r.get("chipload_max_mm_tooth") is None and r.get("chipload_min_mm_tooth") is None:
        return None, "no chipload"
    src = r["source_id"]
    kind, grade = r["row_kind"], r["evidence_grade"]
    if src == "amana_spektra_spiral_plunge_v24" and mf != "mdf" and (kind == "exact" or grade == "b"):
        return "wood_plywood", None
    if kind != "exact":
        return None, f"{kind}/{grade}"
    if src in ("amana_zrn_3d_profiling", "amana_zrn_3d_profiling_v8") and mf in ("mdf", "softwood"):
        return "wood_mdf_signfoam", None
    if src == "amana_compression_spirals_v8" and mf == "hardwood":
        return "wood", None
    if src == "onsrud_laminated_chipboard_cutting_data":
        return "particleboard_laminated", None
    if src == "onsrud_laminated_plywood_cutting_data":
        return "plywood_hardwood_laminated", None
    return mf, None


def mid_max(r):
    lo, hi = r.get("chipload_min_mm_tooth"), r.get("chipload_max_mm_tooth")
    if lo is None:
        return hi, hi
    if hi is None:
        return lo, lo
    return (lo + hi) / 2.0, hi


def fmt(x, n=3):
    return "-" if x is None else f"{x:.{n}f}"


def stats(xs):
    if not xs:
        return (0, None, None, None)
    return (len(xs), statistics.median(xs), min(xs), max(xs))


def build_groups(rows):
    excluded = Counter()
    cells = defaultdict(lambda: defaultdict(set))  # key -> column -> {(mid,max,role,id)}
    for r in rows:
        col, why = column_of(r)
        if col is None:
            if r["material_family"] in WOOD:
                excluded[(r["source_id"], why)] += 1
            continue
        d = r.get("diameter_mm")
        key = (chart_set(r["source_id"]), r["tool_family"], r.get("tool_subfamily", ""),
               None if d is None else round(d, 3), r.get("flute_count"), r.get("included_angle_deg"))
        m, x = mid_max(r)
        cells[key][col].add((round(m, 5), round(x, 5), r["pass_role"], r["observation_id"]))
    # collapse fan-out: one value per column, else split by role
    groups = {}
    for key, cols in cells.items():
        values = {c: {(m, x) for (m, x, _, _) in s} for c, s in cols.items()}
        if all(len(v) == 1 for v in values.values()):
            groups[key + ("any",)] = {c: (next(iter(values[c])), sorted(i for *_, i in cols[c])[0]) for c in cols}
        else:
            by_role = defaultdict(dict)
            for c, s in cols.items():
                for (m, x, role, oid) in sorted(s):
                    by_role[role].setdefault(c, ((m, x), oid))
            for role, cc in by_role.items():
                groups[key + (role,)] = cc
    return groups, excluded


def pair_ratios(groups):
    """One record per (key, column, base): ratio of mid and of max."""
    out = []
    for key, cols in groups.items():
        if len(cols) < 2:
            continue
        for b in BASES:
            if b not in cols:
                continue
            (bm, bx), _ = cols[b]
            for c, ((cm, cx), _) in cols.items():
                if c == b or c in BASES and BASES.index(c) < BASES.index(b):
                    continue
                if c == "wood_mdf_signfoam":
                    continue
                out.append({"key": key, "family": key[1], "chart": key[0], "pair": f"{c}/{b}",
                            "mid": cm / bm, "max": cx / bx})
    return out


def main():
    lut = load_lut()
    ver = json.loads((G2 / "verified_rows.json").read_text())["observations"]
    lut_ids = {r["observation_id"] for r in lut}
    rows = lut + [r for r in ver if r["observation_id"] not in lut_ids]
    print(f"LUT rows {len(lut)}; verified G2 rows {len(ver)}; total {len(rows)}")

    groups, excluded = build_groups(rows)
    print("\n## T0. Wood rows left out of the trend (source, reason): count")
    for (src, why), n in sorted(excluded.items()):
        print(f"  {n:3d}  {src}  {why}")

    multi = {k: v for k, v in groups.items() if len(v) >= 2}
    same_cell = {k: v for k, v in groups.items() if "wood_mdf_signfoam" in v}
    print(f"\n## T1. Tools printed in >= 2 columns: {len(multi)} "
          f"(plus {len(same_cell)} ZrN keys where one cell carries two labels)")
    fam_count = Counter(k[1] for k in multi)
    print("  per family:", dict(sorted(fam_count.items())))
    order = ["softwood", "hardwood", "wood", "wood_plywood", "mdf", "hdf", "particleboard", "particleboard_laminated",
             "plywood_softwood", "plywood_hardwood", "plywood_hardwood_laminated"]
    print("\n  mid chipload (mm/tooth) per column; key = chart | family | subfamily | d | flutes | angle | role")
    print("  " + " | ".join(["chart", "fam", "sub", "d", "z", "ang", "role"] + [c[:12] for c in order]))
    for key in sorted(multi, key=lambda k: (k[1], k[0], k[2], k[3] or 0)):
        cols = multi[key]
        vals = [fmt(cols[c][0][0]) if c in cols else "" for c in order]
        k = [key[0][:24], key[1][:10], (key[2] or "")[:22], fmt(key[3], 2) if key[3] else "none",
             str(key[4]), str(key[5] or ""), key[6]]
        print("  " + " | ".join(k + vals))

    pr = pair_ratios(multi)

    print("\n## T2. Ratio to the same tool's base column, per tool family and pair")
    print("  family | pair | n | mid: median [min, max] | max: median [min, max]")
    by_fp = defaultdict(list)
    for p in pr:
        by_fp[(p["family"], p["pair"])].append(p)
    for (fam, pair), ps in sorted(by_fp.items()):
        n, md, lo, hi = stats([p["mid"] for p in ps])
        _, mx, xlo, xhi = stats([p["max"] for p in ps])
        print(f"  {fam} | {pair} | {n} | {md:.2f} [{lo:.2f}, {hi:.2f}] | {mx:.2f} [{xlo:.2f}, {xhi:.2f}]")

    print("\n## T3. The same, split by chart (flat_end and any family with 2+ charts on one pair)")
    by_fcp = defaultdict(list)
    for p in pr:
        by_fcp[(p["family"], p["pair"], p["chart"])].append(p)
    charts_per_fp = Counter((f, pa) for (f, pa, _c) in by_fcp)
    for (fam, pair, chart), ps in sorted(by_fcp.items()):
        if charts_per_fp[(fam, pair)] < 2:
            continue
        n, md, lo, hi = stats([p["mid"] for p in ps])
        print(f"  {fam} | {pair} | {chart} | n {n} | mid {md:.2f} [{lo:.2f}, {hi:.2f}]")

    print("\n## T4. One ratio per pair across families? (median of mid ratio per family)")
    by_pair = defaultdict(dict)
    for (fam, pair), ps in by_fp.items():
        by_pair[pair][fam] = (len(ps), statistics.median([p["mid"] for p in ps]),
                              min(p["mid"] for p in ps), max(p["mid"] for p in ps))
    for pair in sorted(by_pair):
        fams = by_pair[pair]
        if len(fams) < 2:
            continue
        meds = [v[1] for v in fams.values()]
        allv = [p["mid"] for p in pr if p["pair"] == pair]
        txt = "; ".join(f"{f} {v[1]:.2f} (n {v[0]})" for f, v in sorted(fams.items()))
        print(f"  {pair}: {txt} | family medians span {min(meds):.2f}-{max(meds):.2f}; "
              f"all points {min(allv):.2f}-{max(allv):.2f}")

    print("\n## T5. Ball nose, Amana v7 per size (printed roughing rows, mid mm/tooth)")
    for key in sorted(multi, key=lambda k: k[3] or 0):
        if key[0] != "amana_ball_nose_v7":
            continue
        c = multi[key]
        s, h, m = (c.get(x, ((None, None), ""))[0][0] for x in ("softwood", "hardwood", "mdf"))
        print(f"  d {key[3]:6.3f} | soft {fmt(s)} hard {fmt(h)} mdf {fmt(m)} | "
              f"mdf/hard {m / h:.2f} mdf/soft {m / s:.2f} hard/soft {h / s:.2f}")

    print("\n## T6. Onsrud 37-series V-bit (verified G2 rows): distinct bands per size across the 7 tables")
    vs = defaultdict(lambda: defaultdict(set))
    for r in ver:
        if r["tool_family"] != "chamfer_vbit":
            continue
        d = r.get("diameter_mm")
        vs[(r["tool_subfamily"], d)][(r["chipload_min_mm_tooth"], r["chipload_max_mm_tooth"])].add(
            r["source_id"].replace("onsrud_", "").replace("_cutting_data", ""))
    for (sub, d), bands in sorted(vs.items(), key=lambda kv: (kv[0][0], kv[0][1] or 0)):
        for band, srcs in bands.items():
            print(f"  {sub} | d {fmt(d, 3) if d else 'shank 1/4 in'} | band {band} | tables {len(srcs)}: {', '.join(sorted(srcs))}")

    # ---- G2h: the hardness transfer the engine applies today ----
    lut_by_id = {r["observation_id"]: r for r in lut}
    inv = [x for x in csv.DictReader(INV.open()) if x["group"] == "G2h"]
    print(f"\n## T7. G2h cells: the hardness transfer applied today ({len(inv)} cells)")
    print("  tool | query material | row id | row family | row Janka (source) | query Janka | "
          "raw row/query | scale ^0.5 | cells")
    g = Counter()
    for x in inv:
        oid = x["lut_observation_id"]
        o = lut_by_id.get(oid, {})
        hv = o.get("hardness_value")
        rf = o.get("material_family", "?")
        rj, rsrc = (hv, "per-row") if hv else (ROW_DEFAULT_JANKA.get(rf), "family_default_janka")
        qj = FM1_QUERY[x["material"]]
        raw = rj / qj
        g[(x["tool_type"], x["material"], oid, rf, rj, rsrc, qj, round(raw, 3), round(raw ** HARDNESS_EXP, 3))] += 1
    for k, n in sorted(g.items(), key=lambda kv: (kv[0][1], kv[0][0], kv[0][2])):
        t, m, oid, rf, rj, rsrc, qj, raw, sc = k
        print(f"  {t} | {m} | {oid} | {rf} | {rj:.0f} ({rsrc}) | {qj:.0f} | {raw:.2f} | {sc:.2f} | {n}")
    agg = Counter()
    for k, n in g.items():
        same = "same family" if k[1] == k[3] else "cross family"
        agg[(k[3], k[1], k[4], k[6], k[5], same, k[8])] += n
    print("\n  summary: row family -> query | row J -> query J | row J source | scale | cells")
    for (rf, qm, rj, qj, rsrc, same, sc), n in sorted(agg.items(), key=lambda kv: -kv[1]):
        print(f"  {rf} -> {qm} ({same}) | {rj:.0f} -> {qj:.0f} | {rsrc} | x{sc:.2f} | {n}")

    # compare the Janka law with the printed hardwood:softwood ratio
    hs = [p for p in pr if p["pair"] == "softwood/hardwood"]
    print("\n  printed softwood/hardwood mid ratio (all families): "
          f"n {len(hs)}, median {statistics.median([p['mid'] for p in hs]):.2f}, "
          f"range {min(p['mid'] for p in hs):.2f}-{max(p['mid'] for p in hs):.2f}")
    for fam in sorted({p["family"] for p in hs}):
        v = [p["mid"] for p in hs if p["family"] == fam]
        print(f"    {fam}: n {len(v)} median {statistics.median(v):.2f} [{min(v):.2f}, {max(v):.2f}]")
    print(f"  Janka law (row/query)^0.5 for a 1450 row on a 600 query: x{(1450 / 600) ** 0.5:.2f} "
          f"(compare with the printed softwood/hardwood ratio above); for 1290 -> 600: x{(1290 / 600) ** 0.5:.2f}; "
          f"for 500 -> 600: x{(500 / 600) ** 0.5:.2f}")

    print("\n## T8. The two engine Janka tables side by side (lbf)")
    print("  family | query table (material/mod.rs effective_janka_lbf / janka_lbf) | "
          "row default (vendor_lookup.rs family_default_janka) | sourced value found by the G2 fetch")
    sourced = {"particleboard": "500 minimum (ANSI A208.1-2016 Table B, 2225 N (500 lb); Wood Handbook 1999 Table 10-8, M grades)",
               "mdf": "none found (Wood Handbook Table 10-10 has no hardness column)",
               "hdf": "none searched", "plywood_hardwood": "none (no standard; face species: Birch 1260 in the engine species table)",
               "plywood_softwood": "none (no standard)", "hardwood": "species tables (engine GenericHardwood 1450)",
               "softwood": "species tables (engine GenericSoftwood 600)"}
    qmap = {"softwood": "600", "hardwood": "1450", "plywood_softwood": "600",
            "plywood_hardwood": "1200 BalticBirch / 1000 HardwoodFaced", "mdf": "1100", "hdf": "1300", "particleboard": "750"}
    for fam in ["softwood", "hardwood", "plywood_softwood", "plywood_hardwood", "mdf", "hdf", "particleboard"]:
        print(f"  {fam} | {qmap[fam]} | {ROW_DEFAULT_JANKA[fam]:.0f} | {sourced[fam]}")

    # ---- the G2 cells and their solid-wood siblings in the matrix ----
    mx = list(csv.DictReader(MATRIX.open()))
    idx = {(r["tool_type"], r["diameter_mm"], r["operation"], r["material"]): r for r in mx}
    g2 = [x for x in csv.DictReader(INV.open()) if x["group"] == "G2"]
    print(f"\n## T9. The {len(g2)} G2 cells by tool, material, and what the same cell does in softwood / hardwood")
    t9 = Counter()
    for x in g2:
        sib = []
        for m in ("softwood", "hardwood"):
            r = idx.get((x["tool_type"], x["diameter_mm"], x["operation"], m))
            sib.append("refused" if r is None or r["status"] != "ok" else r["support_arm"])
        t9[(x["tool_type"], x["material"], "/".join(sorted(set(sib))), x["operation"])] += 1
    agg9 = defaultdict(lambda: [0, []])
    for (t, m, s, op), n in t9.items():
        agg9[(t, m, s)][0] += n
        agg9[(t, m, s)][1].append(op)
    for (t, m, s), (n, ops) in sorted(agg9.items()):
        print(f"  {t} | {m} | sw/hw sibling: {s} | {n} cells | {', '.join(sorted(set(ops)))}")

    print("\n## T10. Verified 37-series rows that pass the V-bit angle gate for the matrix 60 deg V-bit")
    print("  (vendor_lookup.rs find_best_vbit_row_where skips a row whose angle differs by > 20 deg;"
          " a row with no angle passes with no bonus)")
    t10 = Counter()
    for r in ver:
        if r["tool_family"] != "chamfer_vbit":
            continue
        a = r.get("included_angle_deg")
        gate = "no angle: passes, 0 bonus" if a is None else ("passes" if abs(a - 60) <= 20 else "skipped")
        t10[(r["tool_subfamily"], r.get("diameter_mm"), a, gate)] += 1
    for (sub, d, a, gate), n in sorted(t10.items(), key=lambda kv: (kv[0][0], kv[0][1] or 0)):
        print(f"  {sub} | d {fmt(d, 3) if d else 'none'} | angle {a} | {gate} | {n} rows")


if __name__ == "__main__":
    main()
