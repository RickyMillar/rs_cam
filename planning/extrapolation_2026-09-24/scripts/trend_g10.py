#!/usr/bin/env python3
"""G10 (entry parameters) Phase 2 trend tables. Read-only on the repo.

Inputs:
  planning/extrapolation_2026-09-24/fetch/G10/statements.json  (243, verified)
  planning/extrapolation_2026-09-24/fetch/G10/sources.json     (60, 43 stored)
  planning/extrapolation_2026-09-24/g10_inventory_cells.csv    (Phase 0)
  planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv       (FM1)
  engine constants, parsed from the Rust source by regex (no cargo):
    compute/operation_configs.rs  Adaptive3d ramp angle, helix factor, pitch
    compute/config.rs             DressupConfig ramp angle, helix radius, pitch
    dressup/entry_descent.rs      ENTRY_CONTACT_CLEARANCE, ENTRY_CLEARANCE
    finish/pencil/emission.rs     ENTRY_RAMP_MAX_ANGLE_DEG
    material/mod.rs               plunge_rate_base (wood / sheet baseline)
    feeds/mod.rs                  ball-tip plunge cap per mm of tip
    rs_cam_cli/src/job.rs         the CLI entry mapping

Every number that this script computes is DERIVED. The printed numbers are
the statement values. The script fits nothing.

Run:    python3 planning/extrapolation_2026-09-24/scripts/trend_g10.py
Output: stdout, and a copy in fetch/G10/trend_g10.out
"""

import csv
import io
import json
import math
import pathlib
import re
import statistics
import sys
from collections import Counter, defaultdict

ROOT = pathlib.Path(__file__).resolve().parents[3]
PLAN = ROOT / "planning" / "extrapolation_2026-09-24"
G10 = PLAN / "fetch" / "G10"
STATEMENTS = G10 / "statements.json"
SOURCES = G10 / "sources.json"
CELLS = PLAN / "g10_inventory_cells.csv"
FM1 = ROOT / "planning" / "feeds_matrix_2026-09-23" / "matrix_2026-09-23.csv"
SRC = ROOT / "crates" / "rs_cam_core" / "src"
CLI = ROOT / "crates" / "rs_cam_cli" / "src" / "job.rs"
OUT = G10 / "trend_g10.out"

IN = 25.4
Z_FM1 = 2  # FLUTES in the FM1 instrument, every tool kind
BULL_CORNER_FRAC = 0.15  # FM1 bull corner radius = 0.15 x D
VBIT_INCLUDED_DEG = 60.0  # FM1 V-bit
SIZES = {"EndMill": [3.175, 6.0], "BullNose": [3.175, 6.0], "BallNose": [3.175, 6.0],
         "TaperedBallNose": [3.175, 6.0], "VBit": [6.35, 12.7]}
KIND_FAMILY = {"EndMill": "flat_end_mill", "BullNose": "bull_nose", "BallNose": "ball_nose",
               "TaperedBallNose": "tapered_ball_nose", "VBit": "v_bit"}
DRILL_OPS = {"Drill", "AlignmentPinDrill"}

_buf = io.StringIO()


def out(s=""):
    print(s)
    _buf.write(s + "\n")


def rule(title):
    out()
    out("=" * 78)
    out(title)
    out("=" * 78)


def rng(xs, fmt="{:.3f}"):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "-"
    lo, hi = min(xs), max(xs)
    return fmt.format(lo) if abs(hi - lo) < 1e-9 else f"{fmt.format(lo)}-{fmt.format(hi)}"


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


# ------------------------------------------------------ engine constants
def line_of(text, pos):
    return text.count("\n", 0, pos) + 1


def grab(path, pattern, flags=0):
    text = path.read_text()
    m = re.search(pattern, text, flags)
    if m is None:
        raise SystemExit(f"regex not found in {path}: {pattern}")
    return float(m.group(1)), f"{path.relative_to(ROOT)}:{line_of(text, m.start(1))}"


def engine_constants():
    oc = SRC / "compute" / "operation_configs.rs"
    cf = SRC / "compute" / "config.rs"
    ed = SRC / "dressup" / "entry_descent.rs"
    pe = SRC / "finish" / "pencil" / "emission.rs"
    mt = SRC / "material" / "mod.rs"
    fd = SRC / "feeds" / "mod.rs"
    c = {}
    c["a3d_ramp_deg"] = grab(oc, r"fn default_adaptive3d_ramp_angle\(\) -> f64 \{\s*([\d.]+)")
    c["a3d_helix_factor"] = grab(oc, r"fn default_adaptive3d_helix_radius_factor\(\) -> f64 \{\s*([\d.]+)")
    c["a3d_helix_pitch"] = grab(oc, r"fn default_adaptive3d_helix_pitch\(\) -> f64 \{\s*([\d.]+)")
    c["dr_ramp_deg"] = grab(cf, r"entry_style: DressupEntryStyle::None,\s*ramp_angle: ([\d.]+),")
    c["dr_helix_r_mm"] = grab(cf, r"entry_style: DressupEntryStyle::None,\s*ramp_angle: [\d.]+,\s*helix_radius: ([\d.]+),")
    c["dr_helix_pitch"] = grab(cf, r"entry_style: DressupEntryStyle::None,\s*ramp_angle: [\d.]+,\s*helix_radius: [\d.]+,\s*helix_pitch: ([\d.]+),")
    c["entry_contact_clearance"] = grab(ed, r"pub const ENTRY_CONTACT_CLEARANCE: f64 = ([\d.]+);")
    c["entry_clearance_nominal"] = grab(ed, r"pub\(crate\) const ENTRY_CLEARANCE: f64 = ([\d.]+);")
    c["pencil_ramp_deg"] = grab(pe, r"const ENTRY_RAMP_MAX_ANGLE_DEG: f64 = ([\d.]+);")
    c["plunge_wood_6mm"] = grab(mt, r"Material::SolidWood \{ \.\. \} \| Material::SolidWoodByJanka \{ \.\. \} => ([\d.]+) / h,")
    c["plunge_sheet_6mm"] = grab(mt, r"Material::Plywood \{ \.\. \} \| Material::SheetGood \{ \.\. \} => ([\d.]+) / h,")
    c["ball_cap_per_mm"] = grab(fd, r"let cap = ([\d.]+) \* tip_d;")
    c["cli_2d_ramp_deg"] = grab(CLI, r'"ramp" => \{\s*dressups\.entry_style = DressupEntryStyle::Ramp;\s*dressups\.ramp_angle = ([\d.]+);')
    c["cli_2d_helix_r_mm"] = grab(CLI, r"dressups\.helix_radius = ([\d.]+);")
    c["cli_2d_helix_pitch"] = grab(CLI, r"dressups\.helix_pitch = ([\d.]+);")
    c["cli_3d_helix_factor"] = grab(CLI, r'p\.push\(\("helix_radius_factor", json!\(([\d.]+)\)\)\);')
    c["cli_3d_helix_pitch"] = grab(CLI, r'p\.push\(\("helix_pitch", json!\(([\d.]+)\)\)\);')
    c["cli_3d_ramp_deg"] = grab(CLI, r'p\.push\(\("ramp_angle_deg", json!\(([\d.]+)\)\)\);')
    return c


C = engine_constants()


def cv(k):
    return C[k][0]


# ------------------------------------------------------------ load data
ST = json.load(open(STATEMENTS))
SRCS = {s["source_id"]: s for s in json.load(open(SOURCES))}
FM = list(csv.DictReader(open(FM1)))
CL = list(csv.DictReader(open(CELLS)))
VALID = [s for s in ST if s["verdict"] in ("confirmed", "grade_wrong", "wrong")]
# "wrong" and "grade_wrong" statements carry the verifier's correction already
# (field corrected_from). Their values stay usable; the label is corrected.


def fnum(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return None


def mat(s):
    m = s["material"]
    return m.get("class") if isinstance(m, dict) else m


def dval(s, key):
    for d in s.get("derived") or []:
        if d.get("quantity") == key:
            return d.get("value")
    return None


def printed_fraction(s):
    """Plunge (or Ramp Down) / side feed for one statement, or None.

    The fraction is derived from the printed operands (the `derived` entry,
    or the statement's own operands). The PreciseBits pages print no feed,
    so they have no fraction."""
    for d in s.get("derived") or []:
        q = d.get("quantity") or ""
        v = d.get("value")
        if isinstance(v, (int, float)) and ("fraction" in q):
            return float(v)
    return None


# ---------------------------------------------------------------- tables
def t0_constants():
    rule("T0. Engine entry constants read from the source (regex, no cargo)")
    for k, (v, where) in C.items():
        out(f"  {k:26s} {v:8.3f}   {where}")
    out("  FM1: Z = 2 on every tool kind; bull corner 0.15 x D; V-bit 60 deg; tapered ball diameter = tip.")


def t1_ramp_angle():
    rule("T1. Ramp angle: every printed statement (max and recommended)")
    keys = ("ramp_angle_max", "ramp_angle_recommended", "no_ramp")
    out(f"  {'statement':34s} {'kind':11s} {'family':13s} {'subfamily':38s} {'Z':>5s} {'cc':>5s} {'deg (printed)':22s} {'grade':5s} material")
    for s in VALID:
        if s["parameter"] not in keys:
            continue
        kind = {"ramp_angle_max": "max", "ramp_angle_recommended": "recommend", "no_ramp": "no ramp"}[s["parameter"]]
        z = s["flutes"]
        z = "-" if z is None else (f"{z[0]}-{z[1]}" if isinstance(z, list) else str(z))
        cc = {True: "yes", False: "no", None: "-"}[s["centre_cutting"]]
        mp = s["material"].get("printed") or s["material"].get("label") or mat(s) if isinstance(s["material"], dict) else s["material"]
        out(f"  {s['statement_id']:34s} {kind:11s} {s['tool_family']:13s} {(s['tool_subfamily'] or '')[:38]:38s} {z:>5s} {cc:>5s} "
            f"{(s['value_printed'] or '')[:22]:22s} {s['grade']:5s} {str(mp)[:30]}")
    out()
    out("  Summary (derived):")
    wood = [s for s in VALID if s["parameter"] in keys and s["part"] != "metal" and s["grade"] in ("a", "b")]
    out(f"  - wood or router vendor statements at grade a/b: {len(wood)}")
    out("  - every ramp-angle number is grade c: metal / non-ferrous vendors (Harvey, Helical, SGS, IMCO),")
    out("    a CAM article (CNCCookbook), or a user post (Carbide Create 20 deg).")
    out("  - SGS centre-cutting 2-3 flute and up-cut router: 90 deg (a plunge is allowed); non-centre-cutting")
    out("    or 4-7 flute: 1-7 deg; compression router 5 deg; down-cut router '-' (no legend).")
    out()
    out("  Repo ramp angles:")
    out(f"    dressup (E1 Face/Pocket/Profile/Rest/Zigzag) {cv('dr_ramp_deg'):.1f} deg   {C['dr_ramp_deg'][1]}")
    out(f"    Adaptive3d (E3, Suggest rewrite to Ramp)     {cv('a3d_ramp_deg'):.1f} deg   {C['a3d_ramp_deg'][1]}")
    out(f"    CLI ramp (2.5D and Adaptive3d)               {cv('cli_2d_ramp_deg'):.1f} / {cv('cli_3d_ramp_deg'):.1f} deg")
    out(f"    Pencil internal ramp cap                      {cv('pencil_ramp_deg'):.1f} deg")
    out("  Position against the metal ranges (context, not a source): 3 deg is the low end and 10 deg the top of")
    out("  Harvey/Helical 'soft / non-ferrous 3-10 deg'. 3 deg is below the SGS compression-router maximum (5 deg);")
    out("  10 deg is above it. No statement prints a ramp angle for a ball, tapered ball or V-bit in wood.")


def path_r_frac_rows():
    """Every helix statement, converted to path radius / D (derived)."""
    rows = []
    for s in VALID:
        sid = s["statement_id"]
        p = s["parameter"]
        if p not in ("helix_diameter_min_frac", "helix_diameter_max_frac") and sid != "g10-hobby-019":
            continue
        rows.append(s)
    return rows


def t2_helix_radius():
    rule("T2. Helix size: printed statements in one frame, path radius / D (derived)")
    out("  Frames: bore diameter B, path (tool-centre) diameter P, path radius r. B = P + D; r = P / 2 = (B - D) / 2.")
    out("  No core for a flat tool: r <= D/2, so P <= D and B <= 2D. With corner radius rc: r <= D/2 - rc (IMCO 2D - 2rc).")
    out()
    conv = {
        # statement id: (frame, text, r/D low, r/D high, bound)
        "g10-metal-harvey-te-helix-dia": ("ambiguous", "'>110-120% of the cutter diameter' read as BORE: r = (1.1..1.2 - 1)/2", 0.05, 0.10, "min"),
        "g10-metal-helical-gb-helix-dia": ("ambiguous", "same text, read as PATH diameter: r = 1.1..1.2 / 2 (leaves a core)", 0.55, 0.60, "min"),
        "g10-metal-imco-bore-2d-2r": ("bore", "B = 2D - 2rc; r = D/2 - rc; flat rc = 0", 0.5, 0.5, "max"),
        "g10-metal-sandvik-max-hole": ("bore", "max hole 2 x D3 (non-centre-cutting inserts): r = D/2", 0.5, 0.5, "max"),
        "g10-metal-sandvik-core": ("bore", "a smaller hole-to-cutter ratio leaves a core (text only)", None, None, "-"),
        "g10-hobby-018": ("path", "helix diameter must not exceed D (boss stays): r = D/2", 0.5, 0.5, "max"),
        "g10-hobby-019": ("path", "figure example 0.8 x Dia (good) vs 1.8 x Dia (boss): r = 0.4 D", 0.4, 0.4, "example"),
        "g10-hobby-020": ("path", "no minimum printed", None, None, "-"),
    }
    out(f"  {'statement':32s} {'frame':10s} {'bound':8s} {'r/D':11s} grade  reading")
    for s in path_r_frac_rows():
        c = conv.get(s["statement_id"])
        if c is None:
            out(f"  {s['statement_id']:32s} (no conversion) {s['value_printed']}")
            continue
        frame, text, lo, hi, bound = c
        r = "-" if lo is None else (f"{lo:.2f}" if lo == hi else f"{lo:.2f}-{hi:.2f}")
        out(f"  {s['statement_id']:32s} {frame:10s} {bound:8s} {r:11s} {s['grade']:5s}  {text}")
    out("  Note: the two Harvey/Helical statements print the SAME words; the table shows each of the two readings once.")
    out("  Bore reading: r/D 0.05-0.10 (a near-plunge). Path reading: r/D 0.55-0.60 (a core of 0.1-0.2 D for a flat tool).")
    out()
    out("  Repo helix radius per matrix size, path radius / D (derived):")
    a3 = cv("a3d_helix_factor")
    drr = cv("dr_helix_r_mm")
    cl3 = cv("cli_3d_helix_factor")
    out(f"  {'kind':16s} {'D':>6s} {'no-core r/D':>11s} | {'A3d '+str(a3)+'D':>9s} {'dressup '+str(drr)+'mm':>13s} {'CLI3d '+str(cl3)+'D':>10s} | centre pip height (mm) at A3d / dressup / CLI3d")
    for kind, ds in SIZES.items():
        for d in ds:
            lim = no_core_limit(kind, d)
            vals = [a3, drr / d, cl3]
            pips = [pip_height(kind, d, f * d) for f in vals]
            out(f"  {kind:16s} {d:6.3f} {lim:11.3f} | {vals[0]:9.3f} {vals[1]:13.3f} {vals[2]:10.3f} | "
                + " / ".join(pip_text(kind, d, f * d, p) for f, p in zip(vals, pips)))
    out("  no-core r/D: flat 0.5; bull 0.5 - 0.15 = 0.35; ball, tapered ball, V-bit 0 (any r > 0 leaves a centre pip).")
    out("  pip height: flat/bull 0 up to the flat bottom radius; past it a core stands ('core' = its diameter, 2 (r - flat));")
    out("  ball / tapered ball R - sqrt(R^2 - r^2) with R = D/2 (core when r > R); V-bit r / tan(30 deg).")
    out("  Caution: Adaptive3d multiplies the factor by the ENVELOPE diameter (INVENTORY_G10 1.3); for the tapered ball")
    out("  the FM1 diameter is the tip, so the real A3d radius on that tool is larger than the table shows.")


def no_core_limit(kind, d):
    if kind == "EndMill":
        return 0.5
    if kind == "BullNose":
        return 0.5 - BULL_CORNER_FRAC
    return 0.0


def pip_height(kind, d, r):
    R = d / 2
    if kind == "EndMill":
        return 0.0 if r <= R + 1e-9 else float("inf")
    if kind == "BullNose":
        rc = BULL_CORNER_FRAC * d
        flat = R - rc
        if r <= flat + 1e-9:
            return 0.0
        x = r - flat
        return rc - math.sqrt(max(rc * rc - x * x, 0.0)) if x < rc else float("inf")
    if kind in ("BallNose", "TaperedBallNose"):
        return R - math.sqrt(R * R - r * r) if r < R else float("inf")
    if kind == "VBit":
        return r / math.tan(math.radians(VBIT_INCLUDED_DEG / 2))
    return float("nan")


def pip_text(kind, d, r, p):
    if p <= 1e-9:
        return "0"
    if math.isinf(p):
        flat = {"EndMill": d / 2, "BullNose": d / 2 - BULL_CORNER_FRAC * d}.get(kind, d / 2)
        return f"core {2 * (r - flat):.2f}"
    return f"{p:.3f}"


def helix_angle(r, p):
    return math.degrees(math.atan(p / (2 * math.pi * r)))


def t3_helix_pitch():
    rule("T3. Helix ramp angle = atan(pitch / (2 pi r_path)), derived, per matrix size")
    combos = [
        ("A3d default", "factor", cv("a3d_helix_factor"), cv("a3d_helix_pitch")),
        ("A3d, pitch 1", "factor", cv("a3d_helix_factor"), 1.0),
        ("dressup default", "abs", cv("dr_helix_r_mm"), cv("dr_helix_pitch")),
        ("dressup, pitch 2", "abs", cv("dr_helix_r_mm"), 2.0),
        ("CLI Adaptive3d", "factor", cv("cli_3d_helix_factor"), cv("cli_3d_helix_pitch")),
        ("CLI Adaptive3d, p 2", "factor", cv("cli_3d_helix_factor"), 2.0),
    ]
    ds = [3.175, 6.0, 6.35, 12.7]
    out(f"  {'arm':22s} {'r rule':10s} {'pitch':>5s} | " + " ".join(f"D={d:<6g}" for d in ds))
    for name, kind, rv, p in combos:
        cells = []
        for d in ds:
            r = rv * d if kind == "factor" else rv
            cells.append(f"{helix_angle(r, p):6.2f}  ")
        rtxt = f"{rv}D" if kind == "factor" else f"{rv} mm"
        out(f"  {name:22s} {rtxt:10s} {p:5.1f} | " + " ".join(cells))
    out()
    out("  Printed helix / ramp angles (all grade c, metal):")
    for s in VALID:
        if s["parameter"] == "helix_ramp_angle":
            out(f"    {s['statement_id']:28s} {s['tool_subfamily'][:40]:40s} {s['value_printed']}")
    out("    Harvey / Helical ramp (linear or circular), soft / non-ferrous 3-10 deg, hard / ferrous 1-3 deg")
    out("    Sandvik: pitch never larger than the maximum ap of the insert (no angle)")
    out("  No wood, router or hobby source prints a pitch or a helix angle. Fusion defines the fields with no value.")


def t4_plunge():
    rule("T4a. Plunge (and Amana 'Ramp Down') as a fraction of the side feed: printed operands, derived fraction")
    rows = defaultdict(list)
    for s in VALID:
        if s["parameter"] not in ("plunge_feed", "plunge_feed_fraction", "ramp_feed", "ramp_feed_fraction"):
            continue
        f = printed_fraction(s)
        if f is None and s["parameter"] in ("plunge_feed_fraction", "ramp_feed_fraction") and isinstance(s["value_si"], (int, float)):
            f = float(s["value_si"])
        if f is None:
            continue
        what = "RampDown" if s["source_id"].startswith("wood_amana") or s["source_id"].startswith("wood_toolstoday_calc") else (
            "ramp" if s["parameter"].startswith("ramp") else "plunge")
        rows[(s["source_id"], s["tool_family"], what, s["grade"])].append((f, s))
    out(f"  {'source':40s} {'family':17s} {'what':8s} {'gr':2s} {'n':>3s} {'fraction':13s} {'median':>6s}  Z / D notes")
    for (sid, fam, what, gr), lst in sorted(rows.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        fs = [f for f, _ in lst]
        zs = sorted({str(s["flutes"]) for _, s in lst})
        ds = sorted({str(s["diameter_mm"]) for _, s in lst})
        out(f"  {sid[:40]:40s} {fam:17s} {what:8s} {gr:2s} {len(fs):3d} {rng(fs):13s} {med(fs):6.3f}  Z {','.join(zs)[:14]}; D {','.join(ds)[:34]}")
    out("  Metal chip-load reductions (fraction of chip load at the same RPM = fraction of feed):")
    out("    Harvey SF_18700 chamfer 2F 0.40-0.50; Harvey SF_25000 pointed engraver 1F 0.50 (plastics and metals); Garr 0.50;")
    out("    SGS plunge 0.25 x slot feed; CNCCookbook plunge = slot feed / Z. All grade c.")
    out("  Caution: an Amana 'Ramp Down' column is not defined as a plunge or a ramp feed (verifier a, 33 files searched).")

    # printed reference per matrix tool kind: grade a/b, wood-class materials, excluding Amana Ramp Down
    woodish = {"wood", "softwood", "hardwood", "mdf", "plywood", "other"}
    ref = {}
    ref_rd = {}
    for kind, fam in KIND_FAMILY.items():
        pl, rd = [], []
        for (sid, f2, what, gr), lst in rows.items():
            if f2 != fam or gr not in ("a", "b"):
                continue
            for f, s in lst:
                if s["diameter_mm"] is not None and not isinstance(s["diameter_mm"], list) and s["diameter_mm"] > 20:
                    continue  # surfacing / bowl / 1 in bits are outside the matrix sizes
                if what == "RampDown":
                    rd.append(f)
                elif mat(s) in woodish or mat(s) == "any":
                    pl.append(f)
        ref[kind] = pl if pl else rd  # BullNose: only the Amana corner-radius Ramp Down rule
        ref_rd[kind] = rd
    out()
    out("  Printed plunge fraction per matrix tool kind (grade a/b, D <= 20 mm, Amana Ramp Down apart;")
    out("  BullNose has no printed plunge column, so its reference is the Amana corner-radius Ramp Down rule):")
    for kind in KIND_FAMILY:
        out(f"    {kind:16s} plunge n={len(ref[kind]):3d} median {med(ref[kind]) or float('nan'):.3f} range {rng(ref[kind])};"
            f"  Amana Ramp Down n={len(ref_rd[kind])} {rng(ref_rd[kind])}")

    rule("T4b. Repo plunge / feed per FM1 cell (ok, non-drill), against the printed median for the tool kind")
    grp = defaultdict(list)
    for r in FM:
        if r["status"] != "ok" or r["operation"] in DRILL_OPS:
            continue
        p, f = fnum(r["plunge_mm_min"]), fnum(r["feed_mm_min"])
        if not p or not f:
            continue
        grp[(r["tool_type"], float(r["diameter_mm"]), r["material"])].append((p / f, p, f, r["operation"]))
    out(f"  {'kind':16s} {'D':>6s} {'material':17s} {'n':>3s} {'repo frac':13s} {'median':>6s} {'printed med':>11s} {'ratio repo/printed':18s} {'plunge mm/min':13s} {'per mm D':9s}")
    for (kind, d, m), lst in sorted(grp.items()):
        fr = [x[0] for x in lst]
        pm = med(ref[kind])
        ratio = [x / pm for x in fr] if pm else []
        pls = [x[1] for x in lst]
        out(f"  {kind:16s} {d:6.3f} {m:17s} {len(lst):3d} {rng(fr):13s} {med(fr):6.3f} {pm:11.3f} {rng(ratio, '{:.2f}'):18s} {rng(pls, '{:.0f}'):13s} {rng([p / d for p in pls], '{:.0f}')}")
    out()
    out("  Summary per tool kind and size (all materials; the Pocket cell is the rough the plunge serves):")
    out(f"  {'kind':16s} {'D':>6s} {'n':>3s} {'repo frac':13s} {'ratio':11s} {'Pocket frac':13s} {'Pocket ratio':12s}")
    by_kd = defaultdict(list)
    for (kind, d, m), lst in grp.items():
        by_kd[(kind, d)].extend(lst)
    for (kind, d), lst in sorted(by_kd.items()):
        pm = med(ref[kind])
        fr = [x[0] for x in lst]
        pk = [x[0] for x in lst if x[3] == "Pocket"]
        out(f"  {kind:16s} {d:6.3f} {len(lst):3d} {rng(fr):13s} {rng([x / pm for x in fr], '{:.2f}'):11s} "
            f"{rng(pk):13s} {rng([x / pm for x in pk], '{:.2f}'):12s}")
    allr = [x[0] / med(ref[k[0]]) for k, lst in grp.items() for x in lst]
    out(f"  all ok non-drill cells: n={len(allr)}, ratio repo/printed median {med(allr):.2f}, range {rng(allr, '{:.2f}')}")
    out("  A ratio < 1: the repo plunges slower, relative to its own feed, than the printed charts do relative to theirs.")

    rule("T4c. Absolute plunge per mm of D: printed (wood) against the repo base")
    for s in VALID:
        if s["source_id"].startswith("precisebits") and s["parameter"] == "plunge_feed" and s["value_si"] is not None:
            d = s["diameter_mm"]
            d = d[-1] if isinstance(d, list) else d
            v = s["value_si"]
            txt = f"{v[0]:.0f}-{v[1]:.0f}" if isinstance(v, list) else f"{v:.0f}"
            per = f"{v[0] / d:.0f}-{v[1] / d:.0f}" if isinstance(v, list) else f"{v / d:.0f}"
            out(f"    {s['statement_id']:14s} {s['tool_family']:10s} D {d:6.3f} ({s['material']['label'][:28]:28s}) plunge {txt:>10s} mm/min = {per:>9s} per mm (D = shank / cutter)")
    fl = [s for s in VALID if s["source_id"] == "sienci_feeds_speeds_metric" and s["tool_family"] == "flat_end_mill"
          and s["diameter_mm"] and s["diameter_mm"] < 20]
    out(f"    Sienci flat rows (1.587-6.35 mm): plunge {rng([s['value_si'] for s in fl], '{:.0f}')} mm/min = "
        f"{rng([s['value_si'] / s['diameter_mm'] for s in fl], '{:.0f}')} per mm")
    base_w, base_s = cv("plunge_wood_6mm"), cv("plunge_sheet_6mm")
    out(f"    Repo base: wood {base_w:.0f}/h, sheet {base_s:.0f}/h at 6 mm, linear in D: {base_w / 6:.0f}/h and {base_s / 6:.0f}/h per mm; h = (Janka/600)^0.4")
    out(f"    Repo ball / tapered-ball cap: {cv('ball_cap_per_mm'):.0f} per mm of tip diameter (FSWizard / GWizard cited in a comment only)")

    rule("T4d. The G6 drill claim against the milling plunge on the same flat end mill (FM1 CSV)")
    idx = {(r["tool_type"], r["diameter_mm"], r["operation"], r["material"]): r for r in FM}
    out(f"  {'D':>6s} {'material':17s} {'Drill feed=plunge':>17s} {'Drill RPM':>9s} {'Pocket plunge':>13s} {'Pocket feed':>11s} {'Pocket F/Z':>10s} {'Drill/Pocket plunge':>19s}")
    for d in ("3.1750", "6.0000"):
        for m in ("softwood", "hardwood", "mdf", "plywood_hardwood"):
            dr = idx[("EndMill", d, "Drill", m)]
            po = idx[("EndMill", d, "Pocket", m)]
            df, pp, pf = fnum(dr["feed_mm_min"]), fnum(po["plunge_mm_min"]), fnum(po["feed_mm_min"])
            out(f"  {float(d):6.3f} {m:17s} {df:17.0f} {fnum(dr['rpm']):9.0f} {pp:13.0f} {pf:11.0f} {pf / Z_FM1:10.0f} {df / pp:19.2f}")
    out("  Pocket F/Z is the Amana 'Ramp Down' form at the pocket RPM (derived). The milling plunge reads the material")
    out("  base; it does not read the G6 row.")


def cell_angle(entry_class):
    """The entry angle each class runs by default in FM1 (derived)."""
    if entry_class.startswith("E1"):
        return cv("dr_ramp_deg"), "dressup ramp"
    if entry_class.startswith("E2"):
        return helix_angle(cv("dr_helix_r_mm"), cv("dr_helix_pitch")), "dressup helix 2.0/1.0"
    if entry_class.startswith("E3"):
        return cv("a3d_ramp_deg"), "A3d ramp (Suggest)"
    return None, None


def t5_ramp_feed():
    rule("T5a. Ramp feed: printed rules")
    for s in VALID:
        if s["parameter"] in ("ramp_feed_fraction", "ramp_feed") and not s["source_id"].startswith("wood_amana") \
                and not s["source_id"].startswith("wood_toolstoday_calc"):
            out(f"    {s['statement_id']:32s} {s['grade']}  {s['tool_family']:14s} {(s['value_printed'] or '')[:70]}")
    out("    Amana (6 charts): 'To find Ramp Down: Feed Rate IPM / # of flutes' (grade b rule, grade a columns); meaning not defined.")
    out("    Harvey: straight and roll-in entries 'reduced by 50%' (metal, grade c).")

    rule("T5b. The approved design, checked: ramp feed = min(F, a_z x n x Z / tan(theta))")
    out("  With the G6 axial chip a_z = f_z,side / Z (Amana), and the side feed F = f_z,side x n x Z:")
    out("    a_z x n x Z = f_z,side x n = F / Z, so the second term = F / (Z tan theta).")
    out("  It binds (is below F) only when tan theta > 1/Z. Z = 2: theta > 26.57 deg; Z = 3: theta > 18.43 deg.")
    out("  Where the feed F is capped below f_z,side x n x Z (the machine cap 4000), the threshold is higher still.")
    out("  Physical check: the vertical rate on a ramp is F_ramp x sin theta (along-path feed). Holding it at the plunge")
    out("  F/Z gives F_ramp = F / (Z sin theta); tan and sin differ by cos theta (0.14 % at 3 deg, 1.5 % at 10 deg).")
    out()
    out(f"  {'theta (deg)':>11s} {'arm':26s} {'1/(Z tan) Z=2':>14s} {'Z=3':>6s} {'SGS (metal)':>12s}")
    arms = [(1.0, "SGS lower"), (2.0, "SGS 'slotting feed'"), (cv("dr_ramp_deg"), "dressup ramp"),
            (helix_angle(cv("dr_helix_r_mm"), cv("dr_helix_pitch")), "dressup helix 2.0 mm / 1.0"),
            (6.0, "SGS '25 %'"), (cv("a3d_ramp_deg"), "A3d ramp"),
            (helix_angle(cv("a3d_helix_factor") * 3.175, cv("a3d_helix_pitch")), "A3d helix, D 3.175"),
            (cv("pencil_ramp_deg"), "pencil cap"), (20.0, "Carbide Create (user post)")]
    for th, name in arms:
        t = math.tan(math.radians(th))
        sgs = "1.00" if th <= 2.0 else ("0.25" if abs(th - 6.0) < 1e-9 else "-")
        out(f"  {th:11.2f} {name:26s} {1 / (2 * t):14.2f} {1 / (3 * t):6.2f} {sgs:>12s}")
    out("  Values > 1 mean the approved form ships the full cutting feed F.")
    out("  SGS vertical rate (derived, frac x sin theta, fraction of slot feed): 1 deg 0.017, 2 deg 0.035, 6 deg 0.026;")
    out("  the SGS plunge is 0.25. So SGS holds its ramp far below its own plunge rate: its rule is not an axial-chip rule.")

    rule("T5c. The approved design per FM1 cell (ok cells of E1-E3), derived")
    idx = {(r["tool_type"], r["diameter_mm"], r["operation"], r["material"]): r for r in FM}
    grp = defaultdict(list)
    for c in CL:
        if c["status"] != "ok":
            continue
        th, arm = cell_angle(c["entry_class"])
        if th is None:
            continue
        r = idx[(c["tool_type"], c["diameter_mm"], c["operation"], c["material"])]
        F, n, P = fnum(r["feed_mm_min"]), fnum(r["rpm"]), fnum(r["plunge_mm_min"])
        t = math.tan(math.radians(th))
        az = None
        if c["tool_type"] == "EndMill":
            dr = idx[("EndMill", c["diameter_mm"], "Drill", c["material"])]
            if dr["status"] == "ok":
                az = fnum(dr["feed_mm_min"]) / (fnum(dr["rpm"]) * Z_FM1)
        if az is not None:
            term = az * n * Z_FM1 / t
            basis = "G6 chip"
        else:
            term = P / t
            basis = "plunge/tan"
        ship = min(F, term)
        grp[(c["tool_type"], float(c["diameter_mm"]), c["entry_class"][:2], basis)].append(
            dict(F=F, term=term, ship=ship, P=P, th=th, arm=arm, lit=P, today=0.5 * P if not c["entry_class"].startswith("E3") else P))
    out(f"  {'kind':16s} {'D':>6s} {'cls':3s} {'basis':10s} {'n':>3s} {'theta':>6s} {'term/F':11s} {'ships F':>7s} {'F mm/min':11s} {'literal plunge':14s} {'today (derived)':15s}")
    tot = Counter()
    for (kind, d, cls, basis), lst in sorted(grp.items()):
        binds = sum(1 for x in lst if x["term"] < x["F"])
        tot[(basis, "cells")] += len(lst)
        tot[(basis, "binds")] += binds
        out(f"  {kind:16s} {d:6.3f} {cls:3s} {basis:10s} {len(lst):3d} {lst[0]['th']:6.2f} {rng([x['term'] / x['F'] for x in lst], '{:.1f}'):11s} "
            f"{len(lst) - binds:3d}/{len(lst):<3d} {rng([x['F'] for x in lst], '{:.0f}'):11s} {rng([x['lit'] for x in lst], '{:.0f}'):14s} {rng([x['today'] for x in lst], '{:.0f}'):15s}")
    for basis in ("G6 chip", "plunge/tan"):
        out(f"  {basis}: {tot[(basis, 'cells')]} cells; the second term binds on {tot[(basis, 'binds')]}; the cutting feed ships on the rest.")
    out("  'literal plunge' = the fallback read as 'ramp feed = plunge rate, as today' (the RULINGS text).")
    out("  'today (derived)' = the dressup entry feed 0.5 x plunge (E1, E2) or the plunge (E3), INVENTORY_G10 section 2.3.")
    out("  Caution: the commanded feed is not the time. The rivmap100 arms (INVENTORY_G10 3.5) show the gain is accel-bound.")


def t6_entry_style():
    rule("T6. Entry style: families a source says must not plunge, or should ramp")
    keys = ("no_plunge", "no_ramp", "entry_general")
    pick = re.compile(r"plunge|ramp|helical|centre|center", re.I)
    for s in VALID:
        if s["parameter"] not in keys:
            continue
        if s["parameter"] == "entry_general" and not pick.search(s["verbatim"] or ""):
            continue
        cc = {True: "cc", False: "non-cc", None: "-"}[s["centre_cutting"]]
        out(f"    {s['parameter']:13s} {s['grade']} {s['tool_family']:14s} {(s['tool_subfamily'] or '')[:30]:30s} {cc:6s} {(s['value_printed'] or '')[:62]}")
    out("  Also (verifier a, from the SGS matrix image): 'Plunging not recommended' in Stainless M1-M3, High Temp S1-S3,")
    out("  Titanium S4, Hardened H1-H4 (metal material classes, not wood).")
    vb = [s for s in VALID if s["tool_family"] in ("v_bit", "tapered_ball_nose", "chamfer_v") and s["parameter"] in ("no_plunge", "no_ramp")]
    out(f"  Statements that forbid a plunge or a ramp for a V-bit, chamfer or tapered tool: {len(vb)}.")
    vbp = [s for s in VALID if s["tool_family"] in ("v_bit", "tapered_ball_nose") and s["parameter"] in ("plunge_feed", "ramp_feed")]
    out(f"  Statements that print a plunge or ramp-down feed for a V-bit or tapered ball: {len(vbp)} "
        f"({', '.join(sorted({s['source_id'] for s in vbp}))}).")


def t7_clearance():
    rule("T7. Helix start clearance above the material")
    hits = [s for s in VALID if re.search(r"clearance|start height|feed height", (s["verbatim"] or "") + (s["notes"] or ""), re.I)]
    out(f"  Statements with a start clearance, start height or feed height: {len(hits)}")
    n = 0
    for sid, s in SRCS.items():
        if not s.get("stored_text"):
            continue
        txt = (G10 / s["stored_text"]).read_text(errors="replace")
        if re.search(r"(start|feed|entry)\s+height|clearance\s+(above|over)\s+the\s+(stock|material|part)", txt, re.I):
            n += 1
            out(f"    text match in {sid}")
    out(f"  Stored texts with a start-height or clearance-above-material phrase: {n} of "
        f"{sum(1 for s in SRCS.values() if s.get('stored_text'))}.")
    out(f"  Result: not published. Repo: entry_clearance_mm {cv('entry_contact_clearance'):.1f} mm (operator ruling 2026-09-25),")
    out(f"  {cv('entry_clearance_nominal'):.1f} mm over a nominal stock top.")


def t8_cells():
    rule("T8. Cells each ruling reaches (g10_inventory_cells.csv, ok / all)")
    cnt = defaultdict(Counter)
    for c in CL:
        cls = c["entry_class"].split()[0]
        cnt[(c["tool_type"], cls)]["all"] += 1
        if c["status"] == "ok":
            cnt[(c["tool_type"], cls)]["ok"] += 1
    classes = ["E1", "E2", "E3", "E4a", "E4b", "E5", "E6", "E7"]
    out(f"  {'kind':16s} " + " ".join(f"{k:>8s}" for k in classes) + "   ok total")
    for kind in SIZES:
        row = []
        tot = 0
        for k in classes:
            v = cnt[(kind, k)]
            row.append(f"{v['ok']:3d}/{v['all']:<4d}")
            tot += v["ok"]
        out(f"  {kind:16s} " + " ".join(f"{x:>8s}" for x in row) + f"   {tot}")
    tot = Counter()
    for (kind, k), v in cnt.items():
        tot[k] += v["ok"]
    out("  ok per class: " + ", ".join(f"{k} {tot[k]}" for k in classes) + f"; all {sum(tot.values())}")


def t9_sources():
    rule("T9. Sources: statements and verdicts per stored source")
    by = defaultdict(list)
    for s in ST:
        by[s["source_id"]].append(s)
    for sid, s in SRCS.items():
        if not s.get("stored_text"):
            continue
        v = Counter(x["verdict"] for x in by[sid])
        g = "".join(sorted({x["grade"] for x in by[sid]})) or "-"
        rh = (s.get("verify") or {}).get("rehash", "-")
        out(f"  {sid:42s} n={len(by[sid]):3d} grade {g:3s} rehash {rh:11s} "
            + " ".join(f"{k} {n}" for k, n in sorted(v.items())) + f"  raw {s['raw_sha256'][:16]}")
    out(f"  stored {sum(1 for s in SRCS.values() if s.get('stored_text'))} of {len(SRCS)}; statements {len(ST)}: "
        + ", ".join(f"{k} {n}" for k, n in Counter(x['verdict'] for x in ST).items()))


def main():
    out("trend_g10.py: G10 entry parameters, Phase 2 tables. Every computed number is derived.")
    t0_constants()
    t1_ramp_angle()
    t2_helix_radius()
    t3_helix_pitch()
    t4_plunge()
    t5_ramp_feed()
    t6_entry_style()
    t7_clearance()
    t8_cells()
    t9_sources()
    OUT.write_text(_buf.getvalue())
    print(f"\nwrote {OUT.relative_to(ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
