#!/usr/bin/env python3
"""G8 trend: long-tool and small-tool loads (Phase 2, read-only).

Reads:
  - crates/rs_cam_core/data/vendor_lut/observations/*.json   (the LUT)
  - planning/extrapolation_2026-09-24/fetch/G8/verified_rows.json
  - planning/extrapolation_2026-09-24/fetch/G8/derate_factors.json
  - planning/extrapolation_2026-09-24/fetch/G8/micro_rules.json
  - planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv    (the G8 cells)

Writes nothing. Prints the tables that EXTRAPOLATION_G8.md quotes.

The engine formulas are copied from the Rust source, not invented:
  - ToolDefinition::tip_deflection_mm   (crates/rs_cam_core/src/tool/mod.rs):
    stepped cantilever, 32 shank + 64 cutter mid-point segments,
    I = pi d^4 / 64, cutter section at 0.80 x D (ENDMILL_EQUIVALENT_DIAMETER_FRACTION,
    feeds/predict.rs), load at stickout - ap/2, slope x extension to the tip.
  - E carbide = 600 000 N/mm^2 (compute/tool_config.rs).
  - force::chipload_cap_for_deflection_with_reason (feeds/force.rs):
    fz_cap = (delta / C / ap - F_edge) / (Ks sin theta_peak),
    cos psi = 1 - 2 woc, theta_peak = min(psi, pi/2).
  - force::affine_coefficients_for_kc: Ks = 49.95 Kc/35.1, F_edge = 5.30 Kc/35.1.
  - feeds::long_tool_load_share (feeds/mod.rs): 0.75 above 6 x D, 0.88 above 4 x D.
  - tool_load::deflection EXCEEDS_BOUND_MM 0.200; cutter_constraints finish 50 um.
Everything this script computes is DERIVED. Printed values carry their source id.
"""
import csv
import glob
import json
import math
import pathlib
import statistics
from collections import Counter

REPO = pathlib.Path(__file__).resolve().parents[3]
PLAN = REPO / "planning" / "extrapolation_2026-09-24"
G8 = PLAN / "fetch" / "G8"
LUT_DIR = REPO / "crates" / "rs_cam_core" / "data" / "vendor_lut" / "observations"
MATRIX = REPO / "planning" / "feeds_matrix_2026-09-23" / "matrix_2026-09-23.csv"

E_CARBIDE = 600_000.0
BEND_FRACTION = 0.80
KS_ANCHOR, FEDGE_ANCHOR, KC_ANCHOR = 49.95, 5.30, 35.1
DELTA_ROUGH_MM, DELTA_FINISH_MM = 0.200, 0.050
INCH = 25.4

# Sources the verifiers did not check (the reconciler's task input).
UNVERIFIED = {
    "redline_endmill_tech_info", "fullerton_endmill_speeds", "toolstoday_amana_4649x_specs",
    "precisebits_calibrating_feeds_speeds", "precisebits_faq",
    "harvey_blog_optimize_miniature_end_mills", "harvey_blog_running_parameters_miniature",
    "micromachines_2020_hmin_effective_rake", "materials_2019_mdf_drill_edge_radius",
    "materials_2021_spruce_saw_edge_radius", "materials_2020_hss_planer_knife_edge_radius",
    "materials_2025_wpc_pcd_edge_radius",
}


def repo_share(stickout, d):
    r = stickout / d
    return 0.75 if r > 6.0 else 0.88 if r > 4.0 else 1.0


def compliance_mm_per_n(stickout, cutting_length, d_cut, d_shank, ap, sections=None):
    """Port of ToolDefinition::tip_deflection_mm with force 1 N (flat end mill).

    `sections`, when given, replaces the engine's two-section tool with a list of
    (length_from_tip_mm, bending_diameter_mm) from the tip upward; the rest is shank.
    The engine cannot represent that (it has no neck); it is used only for the
    derived Amana XL comparison.
    """
    l, e = stickout, E_CARBIDE
    load_pos = max(l - ap * 0.5, 0.0)
    if load_pos <= 0.0:
        return 0.0

    def bend_d(x):  # x from the collet
        from_tip = l - x
        if sections is None:
            return max(BEND_FRACTION * d_cut, 0.05) if from_tip <= cutting_length else d_shank
        acc = 0.0
        for length, dd in sections:
            acc += length
            if from_tip <= acc:
                return dd
        return d_shank

    delta = slope = 0.0
    if sections is None:
        shank_top = min(max(l - cutting_length, 0.0), load_pos)
        pieces = [(0.0, shank_top, 32), (shank_top, load_pos, 64)]
    else:
        pieces = [(0.0, load_pos, 2000)]
    for a, b, n in pieces:
        if b <= a:
            continue
        dx = (b - a) / n
        for i in range(n):
            x = a + (i + 0.5) * dx
            if sections is None:
                dd = d_shank if (a == 0.0 and n == 32) else max(BEND_FRACTION * d_cut, 0.05)
            else:
                dd = bend_d(x)
            inv_ei = 1.0 / (e * math.pi * dd ** 4 / 64.0)
            arm = load_pos - x
            delta += inv_ei * arm * arm * dx
            slope += inv_ei * arm * dx
    return delta + slope * (l - load_pos)


def fz_cap(kc, compliance, delta, ap, woc):
    """Port of chipload_cap_for_deflection_with_reason. Returns (value, reason)."""
    ks, fe = KS_ANCHOR * kc / KC_ANCHOR, FEDGE_ANCHOR * kc / KC_ANCHOR
    if ap <= 0 or compliance <= 0:
        return None, "Unmodelled"
    s = math.sin(min(math.acos(max(-1.0, min(1.0, 1 - 2 * woc))), math.pi / 2))
    if s <= 0:
        return None, "NoLateralEngagement"
    budget = delta / compliance
    if ap * fe >= budget:
        return None, "EdgeOverBudget"
    return (budget / ap - fe) / (ks * s), "ok"


def load_lut():
    rows = []
    for f in sorted(glob.glob(str(LUT_DIR / "*.json"))):
        d = json.load(open(f))
        rows += d["observations"] if isinstance(d, dict) else d
    return rows


def hr(title):
    print()
    print("=" * 100)
    print(title)
    print("=" * 100)


def fmt(v, p=3):
    return "-" if v is None else f"{v:.{p}f}"


# ---------------------------------------------------------------------------
def table_a_printed_factors():
    hr("A. Printed length rules per vendor (derate_factors.json). All metal unless stated.")
    ents = json.load(open(G8 / "derate_factors.json"))["entries"]
    print(f"{'source_id':40s} {'axis':34s} {'axis value':28s} {'factor':>6s} {'applies to':32s} verified")
    for e in ents:
        if e.get("kind") in ("chipload_multiplier", "feed_multiplier", "feed_multiplier_ceiling",
                             "speed_multiplier", "chipload_and_speed_multiplier",
                             "doc_rule_not_stickout"):
            v = "NO" if e["source_id"] in UNVERIFIED else "yes"
            print(f"{e['source_id']:40s} {e.get('axis',''):34s} {str(e.get('axis_value',''))[:28]:28s} "
                  f"{fmt(e.get('factor'),2):>6s} {str(e.get('applies_to',''))[:32]:32s} {v}")
    print("\nPhysics statements (no factor):")
    for e in ents:
        if e.get("kind") in ("physics_rule", "limit_statement"):
            v = "NO" if e["source_id"] in UNVERIFIED else "yes"
            print(f"  [{v:3s}] {e['source_id']}: {e['verbatim'][:110]}")


# ---------------------------------------------------------------------------
def table_b_harvey_vs_model():
    hr("B. Harvey SF_23200 Table 1 (printed) beside the cantilever law (derived). Base = 5x.")
    harvey = [(3, 1.20), (5, 1.00), (8, 0.80), (12, 0.65), (15, 0.55)]
    print(f"{'L/d':>4s} {'Harvey chip x':>13s} {'(L/5)^3 compl.':>15s} {'1/(L/5)^3':>10s} "
          f"{'repo share (L/D)':>17s}")
    for r, f in harvey:
        c = (r / 5) ** 3
        print(f"{r:>4d} {f:>13.2f} {c:>15.2f} {1/c:>10.3f} {repo_share(r, 1):>17.2f}")
    xs = [math.log(r) for r, _ in harvey]
    ys = [math.log(f) for _, f in harvey]
    mx, my = statistics.mean(xs), statistics.mean(ys)
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    print(f"\nlog-log slope of the Harvey factor against L/d: {slope:.2f} "
          f"(a constant-deflection chip rule for a pure cantilever would be -3.00)")
    print("Harvey rounds UP to the next row (worked example: ratio 6.5 -> factor .8) and stops at 15x.")


# ---------------------------------------------------------------------------
def table_c_engine_model():
    hr("C. Engine deflection model (port of tip_deflection_mm + chipload_cap_for_deflection).")
    print("Flat end mill, carbide E = 600 GPa, flute section 0.80 D, ap = 1 x D, woc = 0.5 D "
          "(sin theta = 1), Kc = 35.1 (anchor hardwood).")
    cases = [
        ("D 6.00, shank 6.00, flute 25 (default length)", 6.0, 6.0, 25.0),
        ("D 6.00, shank 6.00, flute 3 x D = 18", 6.0, 6.0, 18.0),
        ("D 3.175, shank 3.175, flute 3 x D = 9.5", 3.175, 3.175, 9.525),
        ("D 3.175, shank 6.35, flute 25 (engine default geometry)", 3.175, 6.35, 25.0),
    ]
    for name, d, ds, cl in cases:
        print(f"\n  {name}")
        print(f"  {'stick':>6s} {'L/D':>5s} {'C um/N':>9s} {'C/C(4D)':>8s} {'1/(C/C4D)':>9s} "
              f"{'repo':>5s} {'fz cap 200um':>12s} {'fz cap 50um':>12s} {'cap/cap(4D)':>11s}")
        ap = d
        c4 = compliance_mm_per_n(4 * d, cl, d, ds, ap)
        cap4, _ = fz_cap(KC_ANCHOR, c4, DELTA_ROUGH_MM, ap, 0.5)
        for ratio in (2, 3, 4, 5, 6, 8, 10, 12, 14.17, 15):
            st = ratio * d
            c = compliance_mm_per_n(st, cl, d, ds, ap)
            cr, rr = fz_cap(KC_ANCHOR, c, DELTA_ROUGH_MM, ap, 0.5)
            cf, rf = fz_cap(KC_ANCHOR, c, DELTA_FINISH_MM, ap, 0.5)
            rel = cr / cap4 if (cr and cap4) else None
            print(f"  {st:>6.1f} {ratio:>5.2f} {c*1000:>9.3f} {c/c4:>8.2f} {c4/c:>9.3f} "
                  f"{repo_share(st, d):>5.2f} {(fmt(cr,3) if cr else rr):>12s} "
                  f"{(fmt(cf,3) if cf else rf):>12s} {fmt(rel,3):>11s}")
    print("\n  Stickout 45 mm is ToolConfig::new_default; on a 3.175 mm tool that is L/D 14.17.")

    hr("C2. Stickout sweep at fixed diameter (engine default geometry: flute 25, shank 6.35).")
    print(f"  {'D':>6s} {'stick':>6s} {'L/D':>5s} {'C um/N':>9s} {'C/C(20mm)':>10s} "
          f"{'(L/20)^3':>9s} {'repo':>5s}")
    for d in (3.175, 6.0):
        c20 = compliance_mm_per_n(20.0, 25.0, d, 6.35, d)
        for st in (20, 25, 30, 35, 40, 45, 50, 60):
            c = compliance_mm_per_n(st, 25.0, d, 6.35, d)
            print(f"  {d:>6.3f} {st:>6d} {st/d:>5.2f} {c*1000:>9.3f} {c/c20:>10.2f} "
                  f"{(st/20)**3:>9.2f} {repo_share(st, d):>5.2f}")


# ---------------------------------------------------------------------------
def table_c3_matrix_geometry():
    hr("C3. Absolute caps at the matrix geometry (stickout 45, flute 25, shank 6.35; inferred "
       "from ToolConfig::new_default) against the matrix vendor band of the G8 roughing cells.")
    rows = list(csv.DictReader(open(MATRIX)))
    g8r = [r for r in rows if "LongToolDerate" in (r["feeds_warnings"] or "")
           and r["cap_role"] in ("Roughing", "SemiFinish")]
    for d in (3.175, 6.0):
        bmax = [float(r["chipload_bounds_max_mm"]) for r in g8r
                if abs(float(r["diameter_mm"]) - d) < 1e-6 and r["chipload_bounds_max_mm"]]
        cl = [float(r["r_chip_load_mm"]) for r in g8r
              if abs(float(r["diameter_mm"]) - d) < 1e-6 and r["r_chip_load_mm"]]
        med_b = statistics.median(bmax) if bmax else None
        print(f"\n  D {d}: G8 roughing/semi cells {len([r for r in g8r if abs(float(r['diameter_mm'])-d)<1e-6])}; "
              f"band max median {fmt(med_b,4)} (range {fmt(min(bmax) if bmax else None,4)}-"
              f"{fmt(max(bmax) if bmax else None,4)}); commanded chip median {fmt(statistics.median(cl) if cl else None,4)}")
        print(f"  {'ap/D':>5s} {'woc/D':>5s} {'C um/N':>8s} {'cap 200um':>10s} {'cap 50um':>10s}")
        for apf in (1.0, 0.5, 0.2):
            for woc in (0.5, 0.4, 0.1):
                ap = apf * d
                c = compliance_mm_per_n(45.0, 25.0, d, 6.35, ap)
                cr, rr = fz_cap(KC_ANCHOR, c, DELTA_ROUGH_MM, ap, woc)
                cf, rf = fz_cap(KC_ANCHOR, c, DELTA_FINISH_MM, ap, woc)
                print(f"  {apf:>5.2f} {woc:>5.2f} {c*1000:>8.3f} {(fmt(cr,3) if cr else rr):>10s} "
                      f"{(fmt(cf,3) if cf else rf):>10s}")
        # Largest ap that holds 200 um at the band max chip, woc 0.5 (sin theta = 1).
        if med_b:
            ks, fe = KS_ANCHOR, FEDGE_ANCHOR

            def tip(ap):
                return compliance_mm_per_n(45.0, 25.0, d, 6.35, ap) * ap * (ks * med_b + fe)

            lo, hi = 1e-3, 25.0  # bisection on ap inside the flute length
            if tip(hi) <= DELTA_ROUGH_MM:
                print(f"  ap_max at 200 um, chip = band max median, woc 0.5 D: above the 25 mm flute")
            else:
                for _ in range(60):
                    mid = 0.5 * (lo + hi)
                    lo, hi = (mid, hi) if tip(mid) <= DELTA_ROUGH_MM else (lo, mid)
                print(f"  ap_max at 200 um, chip = band max median, woc 0.5 D: {lo:.2f} mm "
                      f"({lo/d:.2f} x D)")
    print("\n  Self-similar 6 mm tool (flute 18, shank 6): L/D where the 200 um cap (ap 1xD, woc 0.5)"
          " meets 0.20 mm/tooth:")
    for ratio in [x / 2 for x in range(16, 30)]:
        c = compliance_mm_per_n(ratio * 6.0, 18.0, 6.0, 6.0, 6.0)
        cr, _ = fz_cap(KC_ANCHOR, c, DELTA_ROUGH_MM, 6.0, 0.5)
        print(f"    L/D {ratio:>5.1f}: cap {fmt(cr,3)}")


# ---------------------------------------------------------------------------
def table_d_amana_xl(lut):
    hr("D. Amana ZrN v8 / Spektra v6 extra-long rows (verified) against the LUT standard rows.")
    ver = json.load(open(G8 / "verified_rows.json"))["observations"]
    lut_by_id = {r["observation_id"]: r for r in lut}
    std_ids = {9.525: "amana-zrn-ball-softwood-parallel-9525-3f",
               12.7: "amana-zrn-ball-softwood-parallel-12700-3f"}
    print(f"  {'D mm':>6s} {'XL min':>7s} {'XL max':>7s} {'std row':44s} {'std min':>7s} "
          f"{'std max':>7s} {'min ratio':>9s} {'max ratio':>9s}")
    seen = set()
    for r in ver:
        if r["tool_family"] != "ball_nose" or r["material_family"] != "softwood":
            continue
        d = r["diameter_mm"]
        key = (d, r["source_id"])
        if key in seen:
            continue
        seen.add(key)
        sid = std_ids.get(d)
        s = lut_by_id.get(sid) if sid else None
        if s:
            print(f"  {d:>6.3f} {r['chipload_min_mm_tooth']:>7.4f} {r['chipload_max_mm_tooth']:>7.4f} "
                  f"{sid:44s} {s['chipload_min_mm_tooth']:>7.4f} {s['chipload_max_mm_tooth']:>7.4f} "
                  f"{r['chipload_min_mm_tooth']/s['chipload_min_mm_tooth']:>9.3f} "
                  f"{r['chipload_max_mm_tooth']/s['chipload_max_mm_tooth']:>9.3f}")
        else:
            print(f"  {d:>6.3f} {r['chipload_min_mm_tooth']:>7.4f} {r['chipload_max_mm_tooth']:>7.4f} "
                  f"{('no LUT 3-flute ZrN row at this size' if 'zrn' in r['source_id'] else 'no LUT 3-flute Spektra row at this size'):44s}")

    print("\n  D2. Derived compliance of the catalog geometries (toolstoday, UNVERIFIED source).")
    print("  The engine has no neck section; this 3-section cantilever is a derived check only.")
    geo = [
        # name, D in, cutting height in, neck D1 in, reach L1 in, OAL in
        ("46491 XL 3/8", 0.375, 0.75, 0.352, 1.25, 4.0),
        ("46494 std 3/8", 0.375, 2.25, None, None, 4.0),
        ("46496 XL 1/2", 0.5, 1.25, 0.470, 2.0, 7.0),
        ("46495 std 1/2", 0.5, 2.25, None, None, 4.0),
    ]
    print("  Equal stickout 76.2 mm (3 in) for every tool. ap = 1 x D, 1 N.")
    print(f"  {'tool':14s} {'C um/N':>8s}")
    for name, d, ch, d1, l1, oal in geo:
        dm = d * INCH
        secs = [(ch * INCH, BEND_FRACTION * dm)]
        if d1:
            secs.append(((l1 - ch) * INCH, d1 * INCH))
        ce = compliance_mm_per_n(3.0 * INCH, 0, dm, dm, dm, sections=secs)
        print(f"  {name:14s} {ce*1000:>8.3f}")
    print("  Printed max-chip ratio XL/std: 0.875 (3/8), 0.889 (1/2).")


# ---------------------------------------------------------------------------
def table_e_micro(lut):
    hr("E. Micro tools: printed LUT chipload against runout and the minimum chip (derived).")
    micro = json.load(open(G8 / "micro_rules.json"))["entries"]
    ratios = []
    for m in micro:
        if m.get("quantity") == "hmin_over_edge_radius":
            v = m["value"]
            lo, hi = (v, v) if isinstance(v, (int, float)) else v
            ratios.append((lo, hi, m["material_scope"]))
    radii = [(m["value"], m["source_id"]) for m in micro if m.get("quantity") in
             ("edge_radius_um", "edge_radius_um_new", "edge_radius_um_worn")]
    print("  hmin / rn printed (metals and crystals, UNVERIFIED source):")
    for lo, hi, sc in ratios:
        print(f"    {lo:.2f}-{hi:.2f}  {sc}")
    rmin = min(r[0] for r in ratios)
    rmax = max(r[1] for r in ratios)
    print("  Edge radius printed (um, UNVERIFIED sources):")
    for v, s in radii:
        print(f"    {v}  {s}")
    carbide_rn = (5.91, 8.0)
    h_lo, h_hi = rmin * carbide_rn[0], rmax * carbide_rn[1]
    print(f"  Derived hmin band for a sharp carbide wood edge: {rmin:.2f} x {carbide_rn[0]} = {h_lo:.2f} um "
          f"to {rmax:.2f} x {carbide_rn[1]} = {h_hi:.2f} um")
    worn = 13.42 / 2.08
    print(f"  Worn edge (HSS knives grew x{worn:.1f}): upper band x{worn:.1f} = {h_hi*worn:.1f} um (derived)")

    print(f"\n  {'observation_id':46s} {'fam':9s} {'mat':9s} {'D mm':>6s} {'z':>2s} {'fz min':>7s} "
          f"{'fz max':>7s} {'min/D %':>7s} {'TIR2% um':>8s} {'fzmin/TIR':>9s} {'fzmin/hmax':>10s} "
          f"{'D* hi um':>8s}")
    wood = {"softwood", "hardwood", "mdf", "plywood_softwood", "plywood_hardwood"}
    sm = [r for r in lut if r.get("chipload_max_mm_tooth") and r["material_family"] in wood
          and r.get("diameter_mm") and r["diameter_mm"] <= 3.2]
    sm.sort(key=lambda r: (r["diameter_mm"], r["observation_id"]))
    dstars = []
    for r in sm:
        d = r["diameter_mm"]
        fmin = r.get("chipload_min_mm_tooth") or r["chipload_max_mm_tooth"]
        tir = 0.02 * d * 1000
        per_d = fmin / d
        dstar_hi = h_hi / 1000 / per_d  # mm, if fz stays proportional to D
        dstars.append((dstar_hi, r["observation_id"]))
        print(f"  {r['observation_id'][:46]:46s} {r['tool_family'][:9]:9s} {r['material_family'][:9]:9s} "
              f"{d:>6.3f} {r.get('flute_count') or 0:>2d} {fmin:>7.4f} {r['chipload_max_mm_tooth']:>7.4f} "
              f"{per_d*100:>7.2f} {tir:>8.1f} {fmin*1000/tir:>9.2f} {fmin*1000/h_hi:>10.1f} "
              f"{dstar_hi*1000:>8.0f}")
    dstars.sort()
    print(f"\n  D* = diameter at which fz (held at this row's fz/D) meets hmin = {h_hi:.2f} um (upper band).")
    print(f"  Largest D* over these rows: {dstars[-1][0]*1000:.0f} um ({dstars[-1][1]}); "
          f"median {statistics.median(x for x, _ in dstars)*1000:.0f} um.")
    print(f"  With the worn-edge band ({h_hi*worn:.1f} um) the largest D* is "
          f"{dstars[-1][0]*worn*1000:.0f} um.")
    small = [r for r in sm if r["diameter_mm"] <= 1.6]
    below_tir = [r for r in small if (r.get("chipload_min_mm_tooth") or r["chipload_max_mm_tooth"])
                 < 0.02 * r["diameter_mm"]]
    below_rub = [r for r in small if (r.get("chipload_min_mm_tooth") or r["chipload_max_mm_tooth"]) < 0.025]
    print(f"  Rows D <= 1.6 mm: {len(small)}; printed fz min under 2 % D runout: {len(below_tir)}; "
          f"under RUBBING_FLOOR_MM_TOOTH 0.025: {len(below_rub)}.")
    print("  PreciseBits start point 3 % of D (UNVERIFIED): 1.0 mm -> 30 um, 0.5 mm -> 15 um, 0.2 mm -> 6 um.")


# ---------------------------------------------------------------------------
def table_f_matrix_cells():
    hr("F. G8 cells in the FM1 matrix (feeds_warnings contains LongToolDerate).")
    rows = list(csv.DictReader(open(MATRIX)))
    g8 = [r for r in rows if "LongToolDerate" in (r["feeds_warnings"] or "")]
    print(f"  matrix cells: {len(rows)}; ship (status ok): {sum(r['status']=='ok' for r in rows)}; "
          f"G8 (LongToolDerate): {len(g8)}")
    for key in ("tool_type", "status", "material", "operation"):
        c = Counter(r[key] for r in g8)
        print(f"  by {key}: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    c = Counter((r["tool_type"], r["diameter_mm"]) for r in rows if r["status"] == "ok")
    cg = Counter((r["tool_type"], r["diameter_mm"]) for r in g8)
    print("  by tool and diameter (G8 / shipped):")
    for k in sorted(c):
        print(f"    {k[0]:16s} {float(k[1]):>7.3f} mm  {cg.get(k,0):>3d} / {c[k]:>3d}")
    agg = sum("feeds.aggressiveness" in (r["diagnostic_ids"] or "") for r in g8)
    print(f"  G8 cells that also carry a feeds.aggressiveness_* diagnostic: {agg}")
    # Operations that feeds/suggest/axial_envelope.rs gives a deflection envelope.
    env = {"Adaptive3d", "VCarve", "ProjectCurve", "Scallop", "UnifiedFinish", "DropCutter",
           "Waterline", "SteepShallow", "SpiralFinish", "RadialFinish", "HorizontalFinish"}
    ga = [r for r in g8 if "feeds.aggressiveness" in (r["diagnostic_ids"] or "")]
    c = Counter(r["operation"] for r in ga)
    print("  share-active cells by operation: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    print(f"  of those, with an axial deflection envelope: {sum(v for k, v in c.items() if k in env)}; "
          f"without: {sum(v for k, v in c.items() if k not in env)}")


def main():
    lut = load_lut()
    print(f"LUT rows: {len(lut)}; verified G8 rows: "
          f"{len(json.load(open(G8 / 'verified_rows.json'))['observations'])}")
    table_a_printed_factors()
    table_b_harvey_vs_model()
    table_c_engine_model()
    table_c3_matrix_geometry()
    table_d_amana_xl(lut)
    table_e_micro(lut)
    table_f_matrix_cells()


if __name__ == "__main__":
    main()
