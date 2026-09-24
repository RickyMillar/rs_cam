#!/usr/bin/env python3
"""G6 (drill) Phase 2 trend tables. Read-only on the LUT and the repo source.

Inputs:
  crates/rs_cam_core/data/vendor_lut/observations/*.json   (the LUT, 389 rows)
  planning/extrapolation_2026-09-24/fetch/G6/verified_rows.json
  planning/extrapolation_2026-09-24/inventory_cells.csv
  engine constants, parsed from the Rust source (no cargo):
    machine/mod.rs ChipLoadFormula::default (k0, p, q)
    feeds/mod.rs DRILL_CHIPLOAD_MULTIPLIER, drill_rpm_envelope_for_diameter
    material/mod.rs drill_plunge_feed_envelope_per_mm, janka_to_drill_per_peck_max_dtd

Every number that this script computes is DERIVED. The printed numbers are
the ones in verified_rows.json and the LUT. The script fits nothing beyond a
log-log slope and ratios.

Run: python3 planning/extrapolation_2026-09-24/scripts/trend_g6.py
"""

import csv
import glob
import json
import math
import pathlib
import re
import statistics
from collections import Counter, defaultdict

ROOT = pathlib.Path(__file__).resolve().parents[3]
PLAN = ROOT / "planning" / "extrapolation_2026-09-24"
LUT_DIR = ROOT / "crates" / "rs_cam_core" / "data" / "vendor_lut" / "observations"
VERIFIED = PLAN / "fetch" / "G6" / "verified_rows.json"
CELLS = PLAN / "inventory_cells.csv"
SRC = ROOT / "crates" / "rs_cam_core" / "src"

IN = 25.4

# The engine's Janka proxy per matrix material (material/mod.rs: GenericSoftwood
# 600, GenericHardwood 1450, SheetGoodKind::Mdf 1100, PlywoodGrade::BalticBirch
# 1200, SheetGoodKind::Particleboard 750). The matrix uses Baltic birch for
# plywood_hardwood (INVENTORY.md section 2).
JANKA = {"softwood": 600.0, "hardwood": 1450.0, "mdf": 1100.0,
         "plywood_hardwood": 1200.0, "particleboard": 750.0}
SHEET = {"mdf", "plywood_hardwood", "particleboard"}


# ---------------------------------------------------------------- helpers
def stats(xs):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "n=0"
    return f"n={len(xs)} median {statistics.median(xs):.3f} range {min(xs):.3f}-{max(xs):.3f}"


def slope_loglog(pts):
    """Least-squares slope of log(y) on log(x). Derived, not a fit claim."""
    lx = [math.log(x) for x, _ in pts]
    ly = [math.log(y) for _, y in pts]
    mx, my = statistics.mean(lx), statistics.mean(ly)
    sxx = sum((a - mx) ** 2 for a in lx)
    sxy = sum((a - mx) * (b - my) for a, b in zip(lx, ly))
    return sxy / sxx


def mid(r):
    lo = r.get("chipload_min_mm_tooth")
    hi = r.get("chipload_max_mm_tooth")
    if lo is None and hi is None:
        return None
    if lo is None:
        return hi
    if hi is None:
        return lo
    return 0.5 * (lo + hi)


def rule(title):
    print()
    print("=" * 78)
    print(title)
    print("=" * 78)


# ------------------------------------------------------ engine constants
def engine_constants():
    mach = (SRC / "machine" / "mod.rs").read_text()
    m = re.search(r"impl Default for ChipLoadFormula.*?k0:\s*([\d.]+),\s*p:\s*([\d.]+),\s*q:\s*([\d.]+)",
                  mach, re.S)
    k0, p, q = (float(x) for x in m.groups())
    feeds = (SRC / "feeds" / "mod.rs").read_text()
    mult = float(re.search(r"const DRILL_CHIPLOAD_MULTIPLIER: f64 = ([\d.]+);", feeds).group(1))
    env = re.search(r"fn drill_rpm_envelope_for_diameter.*?\{(.*?)\n\}", feeds, re.S).group(1)
    tiers = [tuple(float(v.replace("_", "")) for v in t)
             for t in re.findall(r"\(([\d_]+\.0),\s*([\d_]+\.0)\)", env)]
    mat = (SRC / "material" / "mod.rs").read_text()
    wood_env = re.search(r"SolidWoodByJanka \{ \.\. \} => \(([\d.]+), ([\d.]+)\)", mat).groups()
    sheet_env = re.search(r"Material::SheetGood \{ \.\. \} => \(([\d.]+), ([\d.]+)\)", mat).groups()
    peck = re.search(r"fn janka_to_drill_per_peck_max_dtd.*?\n\}", mat, re.S).group(0)
    peck_vals = [float(v) for v in re.findall(r"^\s+([\d.]+) //", peck, re.M)]
    pp = re.search(r"fn drill_per_peck_max_dtd.*?\n    \}", mat, re.S).group(0)
    ply_peck = float(re.search(r"Material::Plywood \{ \.\. \} \| Material::SheetGood \{ \.\. \} => ([\d.]+),", pp).group(1))
    fsexp = float(re.search(r"species\.janka_lbf\(\) / 600\.0\)\.powf\(([\d.]+)\)", mat).group(1))
    return dict(k0=k0, p=p, q=q, mult=mult, rpm_tiers=tiers,
                wood_env=tuple(map(float, wood_env)), sheet_env=tuple(map(float, sheet_env)),
                peck_dense=peck_vals[0], peck_soft=peck_vals[1], peck_mid=peck_vals[2],
                peck_sheet=ply_peck, fsexp=fsexp)


C = engine_constants()


def repo_milling_fz(d, material):
    fs = (JANKA[material] / 600.0) ** C["fsexp"]
    return C["k0"] * d ** C["p"] * (1.0 / fs) ** C["q"]


def repo_rpm_tier(d):
    t = C["rpm_tiers"]
    return t[0] if d <= 6.0 else (t[1] if d <= 10.0 else t[2])


# ------------------------------------------------------------ load data
def load_lut():
    rows = []
    for f in sorted(glob.glob(str(LUT_DIR / "*.json"))):
        d = json.load(open(f))
        rows.extend(d["observations"] if isinstance(d, dict) else d)
    return rows


LUT = load_lut()
VER = json.load(open(VERIFIED))
VOBS = VER["observations"]
DEPTH = VER["depth_statements"]

# The printed diameter range of each Leitz / CMT row (from source_page and the
# verifier notes; the rows carry diameter_mm None). Transcribed, not computed.
D_RANGE = {
    "x-g6-leitz-hs-twist-z2-softwood": (3, 12),
    "x-g6-leitz-hw-twist-z2-heel-softwood": (6, 16),
    "x-g6-leitz-hw-marathon-z2-softwood-d-le6": (None, 6),
    "x-g6-leitz-hw-marathon-z2-softwood-d6-12": (6, 12),
    "x-g6-leitz-hw-marathon-z2-softwood-d-gt12": (12, None),
    "x-g6-leitz-hw-vpoint-z2-softwood": (7, 12),
    "x-g6-leitz-hw-vpoint-marathon-z2-softwood-d6-12": (6, 12),
    "x-g6-leitz-hs-levin-z1-solidwood": (5, 12),
    "x-g6-leitz-hw-levin-z1-solidwood": (12, 16),
    "x-g6-leitz-dowel-excellent-z2-chipboard-coated": (3, 10),
    "x-g6-leitz-dowel-excellent-z2-mdf-by-factor": (3, 10),
    "x-g6-leitz-dowel-excellent-z2-softwood-by-factor": (3, 10),
    "x-g6-leitz-dowel-excellent-z2-particleboard-by-factor": (3, 10),
    "x-g6-cmt-311hwm-particleboard": (5, 10),
    "x-g6-cmt-311hwm-mdf": (5, 10),
}


def drange(r):
    if r.get("diameter_mm") is not None:
        return (r["diameter_mm"], r["diameter_mm"])
    return D_RANGE.get(r["observation_id"], (None, None))


def fmt_d(lo, hi):
    if lo == hi:
        return f"{lo:g}"
    return f"{'' if lo is None else f'{lo:g}'}-{'' if hi is None else f'{hi:g}'}"


def fmt(x, n=4):
    return "-" if x is None else f"{x:.{n}f}"


# ================================================================ T0
def t0_gap():
    rule("T0. The gap: G6 cells in inventory_cells.csv")
    cells = [r for r in csv.DictReader(open(CELLS)) if r["group"] == "G6"]
    print(f"G6 cells: {len(cells)}")
    for key in ("tool_type", "operation", "material", "status"):
        print(f"  by {key}: {dict(sorted(Counter(r[key] for r in cells).items()))}")
    by_tool = defaultdict(set)
    for r in cells:
        by_tool[r["tool_type"]].add(float(r["diameter_mm"]))
    print("  diameters per tool kind:", {k: sorted(v) for k, v in sorted(by_tool.items())})
    lut_drill = [r for r in LUT if r.get("operation_family") == "drill"
                 or "drill" in str(r.get("tool_family", ""))]
    print(f"LUT rows with operation_family drill or a drill tool_family: {len(lut_drill)} of {len(LUT)}")


# ================================================================ T1
def t1_coverage():
    rule("T1. What the verified rows cover (tool kind x quantity x material)")
    print(f"verified rows: {len(VOBS)}; depth statements: {len(DEPTH)}")
    print(f"row_kind: {dict(Counter(r['row_kind'] for r in VOBS))}; "
          f"grade: {dict(Counter(r['evidence_grade'] for r in VOBS))}")
    c = Counter()
    for r in VOBS:
        q = "plunge feed (mm/min), no chip load" if mid(r) is None else "chip load"
        c[(r["tool_family"], q, r["material_family"])] += 1
    print(f"{'tool_family':18} {'quantity':36} {'material':17} n")
    for (tf, q, m), n in sorted(c.items()):
        print(f"{tf:18} {q:36} {m:17} {n}")
    print("Matrix tool kinds -> evidence:")
    kinds = {
        "EndMill (flat end mill used as a drill)": [r for r in VOBS if r["tool_family"] == "flat_end"],
        "BullNose": [r for r in VOBS if r["tool_family"] == "bull_nose"],
        "BallNose": [r for r in VOBS if r["tool_family"] == "ball_nose"],
        "TaperedBallNose": [r for r in VOBS if r["tool_family"] == "tapered_ball_nose"],
        "VBit": [r for r in VOBS if r["tool_family"] == "chamfer_vbit"],
        "(a real wood drill: no matrix tool kind)": [
            r for r in VOBS if r["tool_family"] in ("brad_point_drill", "twist_drill", "levin_drill")],
    }
    for k, rs in kinds.items():
        ds = sorted({d for r in rs for d in drange(r) if d is not None})
        ms = sorted({r["material_family"] for r in rs})
        print(f"  {k:44} rows {len(rs):2}  D {ds}  materials {ms}")
    print("Operations: every row is operation_family 'drill'. No row separates Drill from "
          "AlignmentPinDrill; formula_backing maps both to OperationFamily::Drill.")


# ================================================================ T2
def t2_wood_drills():
    rule("T2. Wood drills: chip load per lip and feed per revolution against diameter (derived)")
    print(f"{'vendor':7} {'family':17} {'subfamily':24} {'material':14} {'D mm':7} {'Z':>1} "
          f"{'RPM':>5} {'fz min':>7} {'fz max':>7} {'f/rev min':>9} {'f/rev max':>9}")
    per_mat = defaultdict(list)
    per_mat_rev = defaultdict(list)
    for r in VOBS:
        if r["tool_family"] not in ("brad_point_drill", "twist_drill", "levin_drill"):
            continue
        lo, hi = r["chipload_min_mm_tooth"], r["chipload_max_mm_tooth"]
        z = r["flute_count"]
        dlo, dhi = drange(r)
        print(f"{r['source_vendor']:7} {r['tool_family']:17} {r['tool_subfamily']:24} "
              f"{r['material_family']:14} {fmt_d(dlo, dhi):7} {z:>1} {r['rpm_nominal']:>5.0f} "
              f"{lo:>7.4f} {hi:>7.4f} {lo * z:>9.4f} {hi * z:>9.4f}")
        per_mat[r["material_family"]].append(mid(r))
        per_mat_rev[r["material_family"]].append(mid(r) * z)
    print("Per material (midpoint fz, mm/tooth):")
    for m in sorted(per_mat):
        print(f"  {m:14} fz {stats(per_mat[m])};  f/rev {stats(per_mat_rev[m])}")
    allfz = [x for v in per_mat.values() for x in v]
    allrev = [x for v in per_mat_rev.values() for x in v]
    print(f"  {'all':14} fz {stats(allfz)};  f/rev {stats(allrev)}")

    print("Size trend inside one vendor (derived slopes, midpoints):")
    on = sorted((r["diameter_mm"], mid(r)) for r in VOBS if r["source_id"] == "onsrud_drill_cutting_data")
    lo_pts = sorted((r["diameter_mm"], r["chipload_min_mm_tooth"]) for r in VOBS
                    if r["source_id"] == "onsrud_drill_cutting_data")
    hi_pts = sorted((r["diameter_mm"], r["chipload_max_mm_tooth"]) for r in VOBS
                    if r["source_id"] == "onsrud_drill_cutting_data")
    print(f"  Onsrud 72-000 'Wood', D {[d for d, _ in on]}: fz ~ D^{slope_loglog(on):.3f} (mid), "
          f"D^{slope_loglog(lo_pts):.3f} (min), D^{slope_loglog(hi_pts):.3f} (max)")
    mar = [r for r in VOBS if r["tool_subfamily"] == "leitz_hw_marathon"]
    print("  Leitz HW Marathon (one tool line, three printed D bands): "
          + ", ".join(f"D {fmt_d(*drange(r))} -> {mid(r):.4f}" for r in mar)
          + "  (rises then falls: no monotonic size law)")
    print("  Leitz dowel 'Excellent': one printed vf for D 3-10 mm, so fz is constant over D "
          "(slope 0 by construction).")
    print("  CMT 311 HWM: one printed range 1-4 m/min at 6000 RPM for D 5-10 mm (no size split).")

    print("Printed Leitz material correction factors on vf (same RPM, so they scale fz):")
    print("  dowel p12: base chipboard plastic coated 1.0; 'MDF, solid wood = 0.7'; 'Chipboard, uncoated = 1.3'")
    print("  twist p35 HS: base softwood 1.0; 'Hardwood = 0.7'. twist HW pp37-42: 'Hardwood = 0.8'; LVL 1.1-1.2")
    print("  Levin pp43-44: base 'Solid wood'; 'Drilling depth > 4 x D = 0.8'")


# ================================================================ T3
def lut_flat(source_ids, material):
    out = {}
    for r in LUT:
        if r["source_id"] in source_ids and r["tool_family"] == "flat_end" \
                and r["material_family"] == material and r.get("diameter_mm") and mid(r):
            key = (r.get("tool_subfamily"), r["diameter_mm"], r["pass_role"],
                   r.get("chipload_min_mm_tooth"), r["chipload_max_mm_tooth"])
            out[key] = r
    return list(out.values())


def t3_ratios():
    rule("T3. The ratio the 2.5 multiplier claims: drill chip load / end-mill chip load")
    print("T3a. Wood drill vs the SAME vendor's end mill (Onsrud 72-000 'Wood' vs Onsrud "
          "flat-end LUT rows, nearest printed diameter within 1.25x, midpoints)")
    print(f"  {'drill D':7} {'drill fz':8} {'sheet':9} {'end-mill series':28} {'role':11} "
          f"{'EM D':6} {'size diff':9} {'EM fz':6} {'ratio':5}")
    ratios = defaultdict(list)
    for dr in sorted((r for r in VOBS if r["source_id"] == "onsrud_drill_cutting_data"),
                     key=lambda r: r["diameter_mm"]):
        d = dr["diameter_mm"]
        for mat, src in (("hardwood", "onsrud_hard_wood_cutting_data"),
                         ("softwood", "onsrud_soft_wood_cutting_data")):
            cands = [r for r in lut_flat({src}, mat) if max(r["diameter_mm"], d) / min(r["diameter_mm"], d) <= 1.25]
            if not cands:
                print(f"  {d:<7g} {mid(dr):<8.4f} {mat:9} (no Onsrud flat-end row within 1.25x)")
                continue
            best = min(abs(math.log(r["diameter_mm"] / d)) for r in cands)
            for r in sorted(cands, key=lambda r: (r.get("tool_subfamily"), r["pass_role"])):
                if abs(abs(math.log(r["diameter_mm"] / d)) - best) > 1e-9:
                    continue
                ratio = mid(dr) / mid(r)
                ratios[mat].append(ratio)
                print(f"  {d:<7g} {mid(dr):<8.4f} {mat:9} {r.get('tool_subfamily', ''):28} "
                      f"{r['pass_role']:11} {r['diameter_mm']:<6.3f} {100 * (r['diameter_mm'] / d - 1):+8.1f}% "
                      f"{mid(r):.4f} {ratio:5.2f}")
    for m, v in ratios.items():
        print(f"  ratio vs Onsrud {m} end mills: {stats(v)}")
    print(f"  ratio, all: {stats([x for v in ratios.values() for x in v])}")

    print()
    print("T3b. End-mill plunge vs the SAME tool's side chip load (Amana Spektra v24, same "
          "D, Z, RPM 18000, material column)")
    print(f"  {'Z':1} {'D':6} {'column':17} {'ramp IPM':8} {'axial fz':8} {'side fz (LUT)':13} {'ratio':5}  1/Z")
    col = {"plywood_hardwood": "Wood/Plywood", "mdf": "MDF/Laminate"}
    rs = []
    for r in sorted((r for r in VOBS if r["source_id"] == "amana_spektra_spiral_plunge_v24"),
                    key=lambda r: (r["flute_count"], r["diameter_mm"], r["material_family"])):
        side = [s for s in LUT if s["source_id"] == "amana_spektra_spiral_plunge_v24"
                and s.get("tool_subfamily") == "spektra_spiral_plunge"
                and s["flute_count"] == r["flute_count"] and s.get("diameter_mm") == r["diameter_mm"]
                and s["material_family"] == r["material_family"]]
        sfz = side[0]["chipload_max_mm_tooth"] if side else None
        ratio = r["chipload_max_mm_tooth"] / sfz if sfz else None
        rs.append(ratio)
        print(f"  {r['flute_count']} {r['diameter_mm']:<6g} {col[r['material_family']]:17} "
              f"{r['ramp_down_ipm']:<8g} {r['chipload_max_mm_tooth']:<8.4f} {fmt(sfz):13} "
              f"{fmt(ratio, 3):5}  {1 / r['flute_count']:.3f}")
    print(f"  axial/side: {stats(rs)}  (the printed rule 'Feed Rate IPM / # of flutes' makes it 1/Z;"
          " the vendor rounds the 3F IPM)")

    print()
    print(f"T3c. Vendor figure / the engine's milling formula k0*D^p*(600/J)^({C['fsexp']}*q) "
          f"(k0 {C['k0']}, p {C['p']}, q {C['q']}), i.e. the multiplier each figure implies. "
          f"The engine uses {C['mult']}.")
    print(f"  {'row':52} {'mat':14} {'D eval':7} {'vendor fz':9} {'formula fz':10} {'implied x':9}")
    groups = defaultdict(list)
    for r in VOBS:
        if mid(r) is None:
            continue
        dlo, dhi = drange(r)
        ds = sorted({x for x in (dlo, dhi) if x is not None})
        if r["material_family"] not in JANKA:
            continue
        # The Onsrud sheet prints one 'Wood' row: evaluate it against both woods.
        mats = ["softwood", "hardwood"] if r["source_id"] == "onsrud_drill_cutting_data" \
            else [r["material_family"]]
        kind = "wood drill" if "drill" in r["tool_family"] else "end-mill plunge (Amana ramp)"
        for mat in mats:
            for d in ds:
                f = repo_milling_fz(d, mat)
                for v in sorted({r["chipload_min_mm_tooth"] or mid(r), r["chipload_max_mm_tooth"]}):
                    groups[kind].append(v / f)
                    groups[f"{kind}, {r['source_vendor']}"].append(v / f)
                print(f"  {r['observation_id'][:52]:52} {mat:14} {d:<7g} {mid(r):<9.4f} {f:<10.4f} {mid(r) / f:9.2f}")
    for k, v in groups.items():
        print(f"  implied multiplier, {k} (band ends): {stats(v)}")
    print(f"  engine drill chip load at the matrix sizes (formula x {C['mult']}):")
    for d in (3.175, 6.0):
        print("   ", f"D {d}: " + ", ".join(f"{m} {repo_milling_fz(d, m) * C['mult']:.4f}"
                                           for m in ("softwood", "hardwood", "mdf", "plywood_hardwood")))
    print(f"  engine drill fz (formula x {C['mult']}) / Amana axial fz, same D, Z and column; "
          "softwood and hardwood read through the Wood/Plywood column (R5 shared column):")
    print(f"    {'Z':1} {'D':6} {'matrix material':17} {'column':13} {'engine fz':9} {'Amana fz':8} {'engine/Amana':12}")
    ea = defaultdict(list)
    for r in sorted((r for r in VOBS if r["source_id"] == "amana_spektra_spiral_plunge_v24"),
                    key=lambda r: (r["flute_count"], r["diameter_mm"], r["material_family"])):
        mats = ["mdf"] if r["material_family"] == "mdf" else ["plywood_hardwood", "softwood", "hardwood"]
        for m in mats:
            e = repo_milling_fz(r["diameter_mm"], m) * C["mult"]
            ratio = e / r["chipload_max_mm_tooth"]
            ea["printed column (mdf, plywood_hardwood)" if m in ("mdf", "plywood_hardwood")
               else "shared column (softwood, hardwood)"].append(ratio)
            ea["all"].append(ratio)
            col = "MDF/Laminate" if m == "mdf" else "Wood/Plywood"
            print(f"    {r['flute_count']} {r['diameter_mm']:<6g} {m:17} {col:13} {e:<9.4f} "
                  f"{r['chipload_max_mm_tooth']:<8.4f} {ratio:12.2f}")
    for k, v in ea.items():
        print(f"    engine/Amana, {k}: {stats(v)}")


# ================================================================ T4
def t4_plunge():
    rule("T4. Plunge rate: fractions of the side feed and absolute rates (mm/min)")
    wlo, whi = C["wood_env"]
    slo, shi = C["sheet_env"]
    print(f"Engine envelope drill_plunge_feed_envelope_per_mm: solid wood {wlo:g}-{whi:g}, "
          f"plywood/sheet {slo:g}-{shi:g} mm/min per mm of D (repo-authored).")
    print(f"  {'source':12} {'tool':24} {'D':6} {'Z':1} {'material':17} {'RPM':>5} "
          f"{'plunge mm/min':>13} {'per mm D':>8} {'engine max':>10} {'x max':>5} {'fraction of side feed':>22}")
    per_mm = []
    for r in VOBS:
        if r["source_id"] == "amana_spektra_spiral_plunge_v24":
            mmmin = r["ramp_down_ipm"] * IN
            frac = f"1/{r['flute_count']} (printed rule)"
            tool = f"flat_end {r['tool_subfamily']}"
            rpm = f"{r['rpm_nominal']:.0f}"
        elif r["source_id"] == "precisebits_fret_plane":
            mmmin = r["plunge_feed_mm_min_printed"]
            frac = "not computable (no side feed, no RPM)"
            tool = "bull_nose 3F R0.64"
            rpm = "-"
        else:
            continue
        d = r["diameter_mm"]
        emax = (shi if r["material_family"] in SHEET else whi) * d
        per_mm.append(mmmin / d)
        mat = r["material_family"] + (f" J{r['hardness_value']:.0f}" if r.get("hardness_value") else "")
        if r["verification"]["reconciled_verdict"] == "grade_wrong":
            mat += "*"
        print(f"  {r['source_vendor']:12} {tool:24} {d:<6g} {r['flute_count']} {mat:17} {rpm:>5} "
              f"{mmmin:13.0f} {mmmin / d:8.0f} {emax:10.0f} {mmmin / emax:5.2f} {frac:>22}")
    print("  * grade_wrong row: the page prints 75 in/min only for 'Softwood (Janka < 1,500)'.")
    pb = sorted({r["plunge_feed_mm_min_printed"] for r in VOBS
                 if r["source_id"] == "precisebits_fret_plane"}, reverse=True)
    print("  PreciseBits bands relative to the softwood band: "
          + " : ".join(f"{v / pb[0]:.2f}" for v in pb) + f" ({' / '.join(f'{v:.0f}' for v in pb)} mm/min)")
    print(f"  printed plunge per mm of D: {stats(per_mm)} (engine ceiling {whi:g} wood / {shi:g} sheet)")
    print("Prose fractions (grade c; these sources were NOT verified):")
    print("  CLE Bit Co.: '... at 1/3 of the calculated feed rate' (a 45 deg ramp, not a plunge)")
    print("  ToolsToday: '... reduce ramp or plunge feed to about half the main feed rate' (about 1/2)")
    print("  Adam's Bits: 'You can plunge at 800 mm/min for timber and plastic' (absolute, no size)")
    print("  Onsrud 34-100 (PCT-19 p32, composites, not wood): plunge 40 / feed 80 IPM = 1/2")
    print("Wood drills (absolute feed, derived from the printed vf or IPM):")
    for r in VOBS:
        if "drill" in r["tool_family"] and r.get("vf_m_min_printed"):
            print(f"  {r['observation_id'][:50]:50} vf {r['vf_m_min_printed'] * 1000:6.0f} mm/min at {r['rpm_nominal']:.0f} RPM")
    print("  Onsrud 72-000 footnote: 150 IPM = 3810 mm/min at 4500 RPM (gang drill)")
    print("  CMT 311 HWM: 1000-4000 mm/min at 6000 RPM, D 5-10")


# ================================================================ T5
def t5_rpm_depth():
    rule("T5. RPM and depth: the data against the engine's drill rules")
    print("Engine drill_rpm_envelope_for_diameter:",
          ", ".join(f"{lab} {lo:.0f}-{hi:.0f}" for lab, (lo, hi) in
                    zip(("D<=6", "D<=10", "D>10"), C["rpm_tiers"])))
    for d in (3.175, 6.0):
        print(f"  matrix D {d}: engine tier {repo_rpm_tier(d)}")
    rpm = Counter((r["source_vendor"], r["tool_family"], r.get("rpm_nominal")) for r in VOBS)
    print("Printed RPM in the verified rows:")
    for (v, tf, n), c in sorted(rpm.items(), key=lambda x: str(x)):
        print(f"  {v:12} {tf:17} RPM {n}  rows {c}")
    print("  Leitz diagram ranges: dowel 'n = 3000 - 12000'; HS twist 'n = 1500 - 4000'")
    print()
    print(f"Engine per-peck maximum janka_to_drill_per_peck_max_dtd: softwood (J<=700) "
          f"{C['peck_soft']:g} x D, medium (700<J<=1500) {C['peck_mid']:g} x D, dense (J>1500) "
          f"{C['peck_dense']:g} x D; plywood / sheet {C['peck_sheet']:g} x D. Suggest default = half.")
    print("Printed depth statements (all Leitz, all verified):")
    for s in DEPTH:
        print(f"  {s['depth_statement_id']}: {s['source_page'][:60]} -- {s['meaning'][:110]}")


def main():
    print("G6 trend tables, generated by planning/extrapolation_2026-09-24/scripts/trend_g6.py")
    print(f"LUT rows {len(LUT)}; verified G6 rows {len(VOBS)}; engine constants {C}")
    t0_gap()
    t1_coverage()
    t2_wood_drills()
    t3_ratios()
    t4_plunge()
    t5_rpm_depth()


if __name__ == "__main__":
    main()
