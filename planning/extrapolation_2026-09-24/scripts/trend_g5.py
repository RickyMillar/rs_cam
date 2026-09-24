#!/usr/bin/env python3
"""G5 trend (Phase 2): engaged geometry (V-bit width, tapered cone, bull corner).

Read-only. Inputs:
- crates/rs_cam_core/data/vendor_lut/observations/*.json (the LUT)
- planning/extrapolation_2026-09-24/fetch/G5/verified_rows.json
- planning/extrapolation_2026-09-24/inventory_cells.csv
- planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv

It prints the tables of EXTRAPOLATION_G5.md section 1. It fits nothing: it
prints values, ratios and endpoint slopes only (Phase 3 fits).

The engine formulas that the script copies (so the reader can check them):
- V-bit engaged width: w = 2 * ap * tan(angle / 2), clamped to D
  (ToolGeometryHint::engaged_diameter_at_doc, feeds/mod.rs).
- The lookup ap is the op's axial hint, or D when the op sends none
  (vendor_normalize::lookup_diameter_for_input).
- Row diameter law: (d_query / d_row) ^ 0.61; a row with no diameter gets 1.0
  (vendor_lookup::build_result, CHIPLOAD_DIAMETER_EXPONENT).
- Formula chipload: k0 * D^0.61 * (1/H)^1.26, so at a fixed material the
  ratio between two diameters is (d1 / d2) ^ 0.61.
- Depth ladder: doc_derating_scale(ap / d_band) (feeds/geometry.rs).

Run: python3 planning/extrapolation_2026-09-24/scripts/trend_g5.py
"""
import csv
import glob
import json
import math
import pathlib
from collections import Counter, defaultdict

REPO = pathlib.Path(__file__).resolve().parents[3]
PLAN = REPO / "planning" / "extrapolation_2026-09-24"
LUT_DIR = REPO / "crates" / "rs_cam_core" / "data" / "vendor_lut" / "observations"
MATRIX = REPO / "planning" / "feeds_matrix_2026-09-23" / "matrix_2026-09-23.csv"
INCH = 25.4
P_LAW = 0.61
WOODS = {"softwood", "hardwood", "mdf", "plywood_hardwood", "plywood_softwood", "hdf", "particleboard"}

# LUT rows that are not on their cited document (fetch/G5/lut_discrepancies.md D1, D2).
# They are listed, and left out of every value, ratio and slope below.
NOT_ON_DOC = {
    "amana-vbit-softwood-trace-6000-2f",
    "amana-vbit-hardwood-trace-6000-2f",
    "amana-vbit-mdf-trace-6000-2f",
    "amana-vbit-plywood-hardwood-trace-6000-2f",
    "amana-vbit-acrylic-trace-6000-2f",
    "amana-vbit-softwood-contour-12000-2f",
    "whiteside-vbit-hardwood-trace-12000-2f",
    "whiteside-vbit-mdf-trace-12000-2f",
}


def load_lut():
    rows = []
    for f in sorted(glob.glob(str(LUT_DIR / "*.json"))):
        d = json.load(open(f))
        for r in d["observations"] if isinstance(d, dict) else d:
            r = dict(r)
            r["_file"] = pathlib.Path(f).name
            r["_origin"] = "LUT"
            rows.append(r)
    return rows


def load_verified():
    d = json.load(open(PLAN / "fetch" / "G5" / "verified_rows.json"))
    out = []
    for r in d["observations"]:
        r = dict(r)
        r["_origin"] = "G5"
        out.append(r)
    return out


def mid(r):
    lo, hi = r.get("chipload_min_mm_tooth"), r.get("chipload_max_mm_tooth")
    if lo is not None and hi is not None:
        return 0.5 * (lo + hi)
    return hi if hi is not None else lo


def band(r):
    lo, hi = r.get("chipload_min_mm_tooth"), r.get("chipload_max_mm_tooth")
    if lo is None and hi is None:
        return "-"
    if lo is None or lo == hi:
        return f"{hi:.4f}"
    return f"{lo:.4f}-{hi:.4f}"


def fmt(x, n=4):
    return "-" if x is None else f"{x:.{n}f}"


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for r in rows:
        print("| " + " | ".join(str(c) for c in r) + " |")
    print()


def doc_derating_scale(ratio):
    if not math.isfinite(ratio) or ratio <= 1.0:
        return 1.0
    if ratio <= 2.0:
        return 1.0 - 0.25 * (ratio - 1.0)
    if ratio <= 3.0:
        return 0.75 - 0.25 * (ratio - 2.0)
    return 0.5


def vbit_width(angle, ap, d):
    return min(2.0 * ap * math.tan(math.radians(angle / 2.0)), d)


def tapered_engaged(tip_d, half_angle_deg, ap, shank):
    """ToolGeometryHint::TaperedBall::engaged_diameter_at_doc, copied."""
    r = tip_d / 2.0
    a = math.radians(half_angle_deg)
    h_contact = r * (1 - math.sin(a))
    r_contact = r * math.cos(a)
    cone_offset = h_contact - r_contact / math.tan(a)
    if ap <= h_contact:
        rad = math.sqrt(max(2 * r * ap - ap * ap, 0.0))
    else:
        rad = (ap - cone_offset) * math.tan(a)
    return min(max(2 * rad, 0.0), shank)


def main():
    lut = load_lut()
    ver = load_verified()
    vb_lut = [r for r in lut if r.get("tool_family") == "chamfer_vbit"]
    vb_ver = [r for r in ver if r.get("tool_family") == "chamfer_vbit"]

    # ------------------------------------------------------------------ T0
    print("## T0. Row counts\n")
    c = Counter()
    for r in vb_lut:
        wood = r.get("material_family") in WOODS
        has_chip = mid(r) is not None
        key = "not on cited document" if r["observation_id"] in NOT_ON_DOC else "on document"
        c[("LUT", key, "wood" if wood else "non-wood", "chipload" if has_chip else "RPM only")] += 1
    for r in vb_ver:
        c[("G5 verified", r["source_id"], r["evidence_grade"], "chipload")] += 1
    table(["origin", "group", "material / grade", "kind", "rows"], [list(k) + [v] for k, v in sorted(c.items())])

    # ------------------------------------------------------------------ T1
    print("## T1. V-bit printed rows (wood only): the diameter each chart keys on\n")
    groups = defaultdict(list)
    for r in vb_lut + vb_ver:
        if r.get("material_family") not in WOODS or r["observation_id"] in NOT_ON_DOC:
            continue
        if mid(r) is None:
            continue
        k = (r["_origin"], r["source_id"], r.get("tool_subfamily"))
        groups[k].append(r)
    out = []
    for (origin, src, sub), rs in sorted(groups.items()):
        angles = sorted({r.get("included_angle_deg") for r in rs if r.get("included_angle_deg") is not None})
        fl = sorted({r.get("flute_count") for r in rs})
        diams = sorted({r.get("diameter_mm") for r in rs if r.get("diameter_mm") is not None})
        tips = sorted({r.get("tip_diameter_mm") for r in rs if r.get("tip_diameter_mm") is not None})
        bands = sorted({band(r) for r in rs})
        grades = "".join(sorted({r["evidence_grade"] for r in rs}))
        ap = sorted({(r.get("ap_rule") or (f"ap_max_factor {r['ap_max_factor']}" if r.get("ap_max_factor") else "-"))[:40] for r in rs})
        out.append([
            origin, src, sub, len(rs),
            ",".join(f"{a:g}" for a in angles) or "-",
            ",".join(str(f) for f in fl),
            ",".join(f"{d:g}" for d in diams) or "none printed",
            ",".join(f"{t:g}" for t in tips) or "-",
            "; ".join(bands), grades, "; ".join(ap),
        ])
    table(["origin", "source", "subfamily", "rows", "angles", "flutes", "diameter_mm key", "tip_mm", "chip mm/tooth", "grade", "depth rule"], out)

    # ------------------------------------------------------------------ T2
    print("## T2. The only V-bit size series with a printed diameter: Onsrud 37-series (all 5 sheets print the same cells)\n")
    ons = [r for r in vb_ver if r["source_id"].startswith("onsrud_")]
    series = defaultdict(dict)
    per_mat = defaultdict(set)
    for r in ons:
        s = r["tool_subfamily"]
        series[s][r["diameter_mm"]] = (r["chipload_min_mm_tooth"], r["chipload_max_mm_tooth"])
        per_mat[(s, r["diameter_mm"])].add((r["chipload_min_mm_tooth"], r["chipload_max_mm_tooth"]))
    same = all(len(v) == 1 for v in per_mat.values())
    print(f"Cells identical on all five material sheets: {same}\n")
    out = []
    for s in sorted(series):
        pts = sorted(series[s].items())
        for i, (d, (lo, hi)) in enumerate(pts):
            m = 0.5 * (lo + hi)
            slope_prev = "-"
            slope_first = "-"
            if i > 0:
                d0, (lo0, hi0) = pts[i - 1]
                m0 = 0.5 * (lo0 + hi0)
                slope_prev = f"{math.log(m / m0) / math.log(d / d0):.2f}"
                dA, (loA, hiA) = pts[0]
                mA = 0.5 * (loA + hiA)
                slope_first = f"{math.log(m / mA) / math.log(d / dA):.2f}"
            out.append([s, f"{d:g}", f"{d / INCH:.4g}", f"{lo:.4f}-{hi:.4f}", f"{m:.4f}", f"{m / d * 100:.2f}", slope_prev, slope_first])
    table(["series", "D mm (printed column)", "D in", "chip mm/tooth", "mid", "mid / D %", "log slope vs previous", "log slope vs first"], out)
    # Same cutting diameter, different series.
    print("Same printed diameter, different series (1 in): 37-60 mid "
          f"{0.5 * sum(series['onsrud_37_60_v_bottom_ct'][25.4]):.4f} mm, 37-80 mid "
          f"{0.5 * sum(series['onsrud_37_80_lettering_ct'][25.4]):.4f} mm, ratio "
          f"{sum(series['onsrud_37_60_v_bottom_ct'][25.4]) / sum(series['onsrud_37_80_lettering_ct'][25.4]):.2f}\n")

    # ------------------------------------------------------------------ T3
    print("## T3. Angle and flute trend on charts with no diameter (Amana), wood rows\n")
    out = []
    ins = [r for r in vb_ver if r["source_id"] == "amana_insert_v_groove_v16"]
    by = defaultdict(dict)
    for r in ins:
        by[(r["included_angle_deg"], r["flute_count"], r["rpm_nominal"])][r["material_family"]] = r["chipload_max_mm_tooth"]
    for (a, f, rpm), m in sorted(by.items()):
        wood = m.get("hardwood")
        out.append([f"{a:g}", f, f"{rpm:g}", fmt(m.get("hardwood")), fmt(m.get("softwood")), fmt(m.get("plywood_hardwood")), fmt(m.get("mdf")), f"{m['mdf'] / wood:.2f}"])
    table(["angle", "flutes", "RPM", "hardwood", "softwood", "plywood (derived b)", "MDF", "MDF / wood"], out)
    hw = {r["chipload_max_mm_tooth"] for r in ins if r["material_family"] in ("hardwood", "softwood")}
    ratios = sorted({round(m["mdf"] / m["hardwood"], 3) for m in by.values()})
    print(f"Insert chart: wood values over all {len(by)} (angle, flutes, RPM) groups = {sorted(hw)}; MDF/wood = {ratios}\n")
    out = []
    for r in vb_lut + vb_ver:
        if r["source_id"] in ("amana_ams159_vgroove_v2", "amana_spektra_engraving_v4", "amana_15_60_90_vgroove_engraving_2f") and r.get("material_family") in WOODS:
            out.append([r["_origin"], r["source_id"], r["observation_id"], f"{r.get('included_angle_deg'):g}", r.get("flute_count"), fmt(r.get("diameter_mm"), 3), fmt(r.get("tip_diameter_mm"), 3), band(r), r["evidence_grade"]])
    table(["origin", "source", "row", "angle", "flutes", "diameter_mm (row)", "tip_mm", "chip mm/tooth", "grade"], out)

    # ------------------------------------------------------------------ T4
    print("## T4. Material ratio per vendor (V-bit): MDF / solid wood on one printed cell\n")
    out = []
    ons_ratio = defaultdict(dict)
    for r in ons:
        ons_ratio[(r["tool_subfamily"], r["diameter_mm"])][r["material_family"]] = mid(r)
    rs = sorted({round(v["mdf"] / v["hardwood"], 3) for v in ons_ratio.values()})
    ps = sorted({round(v["plywood_hardwood"] / v["hardwood"], 3) for v in ons_ratio.values()})
    out.append(["Onsrud 37-series (5 sheets)", len(ons_ratio), ",".join(map(str, rs)), ",".join(map(str, ps))])
    ps2 = sorted({round(m["plywood_hardwood"] / m["hardwood"], 3) for m in by.values()})
    out.append(["Amana insert V-groove v16", len(by), ",".join(map(str, ratios)), ",".join(map(str, ps2)) + " (one shared column)"])
    table(["vendor chart", "cells", "MDF / hardwood", "plywood / hardwood"], out)

    # ------------------------------------------------------------------ T5
    print("## T5. LUT V-bit rows that are NOT on their cited document (left out of T1-T4)\n")
    out = []
    for r in vb_lut:
        if r["observation_id"] in NOT_ON_DOC:
            out.append([r["observation_id"], r["source_id"], f"{r.get('included_angle_deg'):g}", fmt(r.get("diameter_mm"), 2), band(r), r["evidence_grade"] + "/" + r["row_kind"]])
    table(["row", "cited source", "angle", "diameter_mm", "chip mm/tooth", "grade/kind"], out)

    # ------------------------------------------------------------------ T6
    print("## T6. Tapered ball and bull nose: the diameter each printed rule keys on\n")
    out = []
    grp = defaultdict(list)
    for r in lut:
        if r.get("tool_family") in ("tapered_ball_nose", "bull_nose") and r.get("material_family") in WOODS:
            grp[(r["tool_family"], r["source_id"], r["evidence_grade"], r["row_kind"])].append(r)
    for (fam, src, g, k), rs in sorted(grp.items()):
        ds = sorted({r.get("diameter_mm") for r in rs if r.get("diameter_mm")})
        tips = sorted({r.get("tip_diameter_mm") for r in rs if r.get("tip_diameter_mm")})
        ap = sorted({r.get("ap_rule") or "-" for r in rs})
        out.append([fam, src, f"{g}/{k}", len(rs), ",".join(f"{d:g}" for d in ds), ",".join(f"{t:g}" for t in tips) or "-", "; ".join(a[:60] for a in ap)])
    table(["family", "source", "grade/kind", "wood rows", "diameter_mm", "tip_diameter_mm", "depth rule"], out)
    print("Printed text (not LUT rows): PreciseBits CM204/CM304 tapered ball: 'Stepdown ... recommended - 1X tip dia. /"
          " maximum - 2X tip dia.'; no chipload. PreciseBits MM208 bull nose: corner radius only; no chipload.\n")

    # ------------------------------------------------------------------ T7
    print("## T7. V-bit depth ladder: ap / w for a pointed V is set by the angle alone\n")
    out = []
    for a in (15, 18, 30, 40, 45, 53.13, 60, 90, 120, 150):
        ratio = 1.0 / (2.0 * math.tan(math.radians(a / 2.0)))
        out.append([f"{a:g}", f"{ratio:.3f}", f"{doc_derating_scale(ratio):.3f}"])
    table(["included angle deg", "ap / w", "doc_derating_scale"], out)

    # ------------------------------------------------------------------ T8
    print("## T8. Inventory cells in G5\n")
    inv = [r for r in csv.DictReader(open(PLAN / "inventory_cells.csv")) if r["group"] == "G5"]
    cc = Counter((r["tool_type"], r["diameter_mm"], r["status"], r["detail"]) for r in inv)
    table(["tool", "D", "state", "detail", "cells"], [list(k) + [v] for k, v in sorted(cc.items())])
    cc = Counter((r["operation"], r["status"]) for r in inv)
    table(["operation", "state", "cells"], [list(k) + [v] for k, v in sorted(cc.items())])
    cc = Counter((r["material"], r["status"]) for r in inv)
    table(["material", "state", "cells"], [list(k) + [v] for k, v in sorted(cc.items())])

    # ------------------------------------------------------------------ T9
    print("## T9. Engine side: the 24 G5 V-bit cells (60 deg, 2 flutes)\n")
    mx = list(csv.DictReader(open(MATRIX)))
    idx = {(r["tool_type"], r["diameter_mm"], r["operation"], r["material"]): r for r in mx}

    def formula_nominal(d, mat):
        # Pocket sends no axial hint, so its formula chip is keyed at nominal D
        # (the same value the refused no-hint parallel ops had before R1).
        return float(idx[("VBit", d, "Pocket", mat)]["r_chip_load_mm"])

    # Anchors for "a row keyed at the nominal diameter".
    ams60 = {r["material_family"]: r for r in vb_lut if r["source_id"] == "amana_ams159_vgroove_v2" and r.get("included_angle_deg") == 60.0}
    ins60 = [r for r in ins if r["included_angle_deg"] == 60.0 and r["material_family"] == "hardwood"][0]
    ams_d = ams60["softwood"]["diameter_mm"]
    ams_v = ams60["softwood"]["chipload_max_mm_tooth"]
    ins_v = ins60["chipload_max_mm_tooth"]
    print(f"Anchors: AMS-159 60 deg 2F row {ams_v} mm/tooth at diameter_mm {ams_d} (SKU, not printed on the chart);"
          f" insert chart 60 deg 1F {ins_v} mm/tooth, no diameter (engine scale 1.0 at any width).\n")

    cells = [r for r in inv]
    out = []
    for r in cells:
        d_s, op, mat = r["diameter_mm"], r["operation"], r["material"]
        d = float(d_s)
        m = idx[("VBit", d_s, op, mat)]
        refused = r["status"] == "refused"
        # Depth cases, each with its source.
        cases = []
        if refused:
            if op == "RampFinish":
                cases.append(("0.5 RampFinish max_stepdown default (operation_configs.rs)", 0.5, 0.5))
            else:
                cases.append(("0.06 x D, r_axial the same op ships on the ball nose (matrix)", 0.06 * d, d))
                cases.append(("0.2 x D, rigidity cap (EVIDENCE 3.4-6)", 0.2 * d, d))
        else:
            ap_ship = float(m["r_axial_depth_mm"])
            hint = op == "VCarve"
            lookup_ap = ap_ship if hint else d
            cases.append((f"r_axial {ap_ship:g} (matrix){'; hinted' if hint else '; no hint, lookup at D'}", ap_ship, lookup_ap))
        fn = formula_nominal(d_s, mat)
        for label, ap, lookup_ap in cases:
            w = vbit_width(60.0, ap, d)
            w_lookup = vbit_width(60.0, lookup_ap, d)
            f_at_w = fn * (w / d) ** P_LAW
            f_engine = fn * (w_lookup / d) ** P_LAW
            ams_nom = ams_v * (d / ams_d) ** P_LAW
            ams_w = ams_v * (w / ams_d) ** P_LAW
            out.append([
                f"{d:g}", op, mat, r["status"], label, f"{ap:.3f}", f"{w:.3f}", f"{w / d:.3f}",
                f"{w_lookup:.3f}", f"{f_engine:.4f}", f"{f_at_w:.4f}", f"{f_engine / f_at_w:.2f}",
                f"{ams_nom:.4f}", f"{ams_w:.4f}", f"{ins_v:.4f}",
                "yes" if f_at_w < 0.5 * min(ins_v, ams_v) else "no",
            ])
    table([
        "D", "op", "material", "state", "depth case (source)", "ap mm", "w at ap mm", "w / D",
        "w the lookup uses", "formula chip, engine key", "formula chip at w", "engine / at-w",
        "AMS-159 row at nominal D", "AMS-159 row at w", "insert row (no diameter)", "at-w formula < 0.5 x lowest 60 deg figure",
    ], out)

    # ------------------------------------------------------------------ T10
    print("## T10. Tapered ball: tip versus the engaged cone the lookup keys on (the matrix tool, 7 deg half angle)\n")
    out = []
    for tip in (3.175, 6.0):
        shank = max(tip + 3.0, 6.0)
        for ap_label, ap in (("0.5 (RampFinish default)", 0.5), ("1 x tip (Onsrud '1xD')", tip), ("2 x tip (PreciseBits maximum)", 2 * tip)):
            de = tapered_engaged(tip, 7.0, ap, shank)
            out.append([f"{tip:g}", ap_label, f"{ap:.3f}", f"{de:.3f}", f"{de / tip:.3f}", f"{(de / tip) ** P_LAW:.3f}"])
    table(["tip mm", "ap case", "ap mm", "engaged d mm", "engaged / tip", "row scale (d/tip)^0.61"], out)
    tb = [r for r in mx if r["tool_type"] == "TaperedBallNose" and r["status"] == "ok"]
    ex = Counter(r["lut_is_extrapolated"] for r in tb)
    print(f"Tapered ball cells that ship in the matrix: {len(tb)}; lut_is_extrapolated counts {dict(ex)}\n")


if __name__ == "__main__":
    main()
