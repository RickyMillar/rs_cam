# C15: assert every number that farside_ideas_v1.html and
# P2_C15_farside_ideas.md quote.
# Run: python3 -I assert_c15.py   (re-runs run_c15.py and whole.py first; about two minutes)
import math
import os
import re
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
ROOT = os.path.join(HERE, "..", "..")
PAGE = os.path.join(ROOT, "farside_ideas_v1.html")
NOTE = os.path.join(ROOT, "P2_C15_farside_ideas.md")
RENDERS = os.path.join(HERE, "..", "renders5")

FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)
        print("FAIL", what)


# ── 1. Analytic checks from the formulas, independent of the pipeline ──
R, s, tol = 3.0, 2.0, 0.1
D = 2 * R
f_arc = math.acos(1 - s / R) / (2 * math.pi) * 1.05
c = (1 - math.cos(2 * math.pi * f_arc)) / 2
cD = c * D
rho_star = s * (2 * R - s) / (2 * (cD - s))
check(abs(c - 0.3626) < 5e-5, f"ceiling c {c}")
check(abs(rho_star - 22.74) < 5e-3, f"rho* {rho_star}")
# C5 table: rho_k = -(R - cD) + sqrt((R - cD)^2 + (rho + R)^2 - R^2), capped
b = R - cD
rho, table = R, []
for _ in range(11):
    rho = min(-b + math.sqrt(b * b + (rho + R) ** 2 - R * R), rho + s)
    table.append(rho)
# the C5 table used c rounded to 0.3626; the unrounded c moves a value by < 0.01
c5 = [4.44, 6.03, 7.73, 9.51, 11.35, 13.24, 15.15, 17.10, 19.06, 21.05, 23.05]
check(max(abs(x - y) for x, y in zip(table, c5)) < 0.01, f"C5 table {table}")
# the brief's width form equals the C5 circular-front width
for g, rh in ((1.4, 4.4), (2.0, 22.7), (1.9, 11.0)):
    a = rh - g + R
    w_c5 = R - (a * a - R * R - rh * rh) / (2 * rh)
    check(abs(w_c5 - (g + g * (R - g / 2) / rh)) < 1e-12, "width form")
# shadow lines and tangent points (C11 note: -4.08 deg, 60.36 deg)
seed, O, rh_ = np.array([18.5, 18.5]), np.array([40.0, 30.0]), 13.0
dd = float(np.hypot(*(O - seed)))
a0 = math.atan2(O[1] - seed[1], O[0] - seed[0])
h = math.asin(rh_ / dd)
check(abs(math.degrees(a0 - h) + 4.08) < 5e-3 and abs(math.degrees(a0 + h) - 60.36) < 5e-3, "shadow angles")
# fillet bound and the blend vertex factor
alpha = h  # half the wedge: the shadow lines are symmetric about seed-O
g_a = s / math.sin(alpha)
r_f = g_a * (R - g_a / 2) / (g_a - cD)
check(abs(g_a - 3.75) < 5e-3 and abs(r_f - 2.68) < 5e-3, f"g_axis {g_a} fillet {r_f}")
vf = math.sin(alpha) / (math.sqrt(2) * math.cos(alpha) ** 2)
# numeric: lines through 0 at +-alpha about the x axis; d_up, d_lo distances
rb, t = 5.0, 10.0


def tau(x, y):
    du = x * math.sin(alpha) - y * math.cos(alpha)
    dl = x * math.sin(alpha) + y * math.cos(alpha)
    df = du - dl
    if abs(df) >= rb:
        return min(du, dl)
    return (du + dl + math.sqrt(2 * rb * rb - df * df)) / 2 - rb


def xi_at(eta):
    lo, hi = 0.0, 100.0
    for _ in range(80):
        m = (lo + hi) / 2
        if tau(m, eta) < t:
            lo = m
        else:
            hi = m
    return lo


e = 0.05
p = [np.array([xi_at(v), v]) for v in (-e, 0.0, e)]
A_, B_, C_ = p
la, lb, lc = np.hypot(*(B_ - C_)), np.hypot(*(A_ - C_)), np.hypot(*(A_ - B_))
cr = abs((B_ - A_)[0] * (C_ - A_)[1] - (B_ - A_)[1] * (C_ - A_)[0])
rad = la * lb * lc / (2 * cr)
check(abs(rad / rb - vf) < 1e-3, f"vertex factor numeric {rad / rb} vs {vf}")
check(abs(vf - 0.5268) < 5e-5, f"vertex factor {vf}")

# ── 2. Fresh pipeline ──────────────────────────────────────────────────
import run_c15  # noqa: E402

run_c15.main()
import whole  # noqa: E402

whole.main()
import page  # noqa: E402
import diag  # noqa: E402

res = page.results_with_derived()


def get(key):
    v = res
    for part in key.split("."):
        v = v[part]
    return v


# ── 3. Every page claim against the fresh results ─────────────────────
PAGE2 = os.path.join(ROOT, "farside_ideas_v2.html")
claims = []
for pg in (PAGE, PAGE2):
    html = open(pg).read()
    cl = re.findall(r'<span data-claim="([^"]+)" data-fmt="([^"]+)">([^<]*)</span>', html)
    check(len(cl) > 100, f"page claims found in {os.path.basename(pg)}: {len(cl)}")
    for key, fmt, shown in cl:
        check(format(get(key), fmt) == shown,
              f"{os.path.basename(pg)} claim {key}: shows {shown}, fresh {format(get(key), fmt)}")
    claims += cl
html = open(PAGE).read()
html2 = open(PAGE2).read()

# ── 4. The note's table and its prose numbers ─────────────────────────
note = open(NOTE).read().replace("−", "-")
blk = note.split("<!-- table:begin -->")[1].split("<!-- table:end -->")[0].strip()
check(blk == page.note_table(res).strip(), "note table differs from a fresh build")
inp = res["inputs"]
R_ = res
prose = {
    "c": f"{inp['c']:.4f}", "cD": f"{inp['cD']:.4f}", "rho*": f"{inp['rho_star']:.2f}",
    "seed to island": f"{inp['seed_to_island']:.2f}",
    "angles": f"{inp['ang_lo_deg']:.2f}° and {inp['ang_up_deg']:.2f}°",
    "wedge": f"{inp['wedge_deg']:.2f}°",
    "P_lo": f"({inp['P_lo'][0]:.2f}, {inp['P_lo'][1]:.2f})",
    "P_up": f"({inp['P_up'][0]:.2f}, {inp['P_up'][1]:.2f})",
    "B": f"({inp['B'][0]:.2f}, {inp['B'][1]:.2f})", "A": f"({inp['A'][0]:.2f}, {inp['A'][1]:.2f})",
    "zone": f"{inp['zone_mm2']:.0f} mm²", "scallop": f"{inp['scallop_mm']:.2f} mm",
    "first dev": f"{res['first_region']['harness_levels_dev_mm']:.3f} mm ({res['first_region']['levels']} levels)",
    "near end": f"{res['a']['sw_near_end_max']:.2f} D swept, {res['a']['apm_near_end_max']:.2f} D area",
    "near ref": f"{res['a']['sw_near_interior_max']:.4f} swept and {res['a']['apm_near_interior_max']:.4f} area",
    "a0 over": f"{res['a0']['sw_interior_max']:.4f} in the arc interior on {res['a0']['sw_interior_over']}",
    "a0 over where": f"{res['a0_over_min_mm']:.1f} to {res['a0_over_max_mm']:.1f} mm",
    "a holds": f"({res['a']['sw_interior_max']:.4f}) for {res['a_extra_levels']} more levels",
    "a V": f"({res['a']['v_corners']} V corners)",
    "a convex": f"{res['a']['min_convex_rho_mm']:.2f} mm",
    "b vs c0": f"{res['b']['length_mm']:.0f} mm of cut against {res['c0']['length_mm']:.0f}",
    "b ratio": f"({res['b']['length_mm'] / res['c0']['length_mm']:.1f} times)",
    "b arcs": f"{res['derived']['b_zone_open']} open arcs", "b load": f"({res['b']['sw_interior_max']:.4f})",
    "g_a": f"{inp['g_axis_mm']:.2f} mm", "r_f": f"{inp['rho_fillet_mm']:.2f} mm",
    "vf": f"{inp['vertex_factor']:.4f}",
    "c result": f"smallest radius {res['c']['min_abs_rho_mm']:.2f} mm, the load holds ({res['c']['sw_interior_max']:.4f})",
    "c cost": f"{res['c']['levels'] - res['c0']['levels']} more zone levels and "
              f"{res['c']['length_mm'] - res['c0']['length_mm']:.0f} mm more cut",
    "c_star": f"{res['c_star']['sw_interior_max']:.4f}",
    "d side": f"`d_lo` max {inp['d_lo_max']:.2f} mm against\n`d_up` {inp['d_up_max']:.2f} mm",
    "d result": f"{res['d']['levels']} levels, ONE arc per level",
    "d radius": f"smallest radius {res['d']['min_abs_rho_mm']:.2f} mm, {res['d']['length_mm']:.0f} mm of cut",
    "d pct": f"({(res['d']['length_mm'] / res['c0']['length_mm'] - 1) * 100:.0f} %",
    "d load": f"{res['d']['sw_interior_max']:.4f} swept (within the quantum) and\n  {res['d']['apm_interior_max']:.4f} area per mm",
    "d over": f"{(res['d']['sw_interior_max'] - c) * D:.2f} mm over c·D",
    "a_c": f"{res['a_c']['levels']} levels ({res['a_c_extra_levels']} more",
    "a_c blend": f"{res['a_c']['blend_mm2']:.1f} mm²",
    "a_c load": f"({res['a_c']['sw_interior_max']:.4f} swept, {res['a_c']['apm_interior_max']:.4f} area per mm)",
    "a_c arcs": f"({res['derived']['a_c_plan_arcs']})",
    "h1": f"within {res['h1_curv_vs_exact']['max_abs_mm']:.2f} mm of the exact",
    "h1 rms": f"(RMS {res['h1_curv_vs_exact']['rms_mm']:.2f} mm, on average {res['h1_neg_mean_mm']:.2f} mm farther out",
    "h1 near": f"exact to {res['h1_curv_vs_exact']['near_max_abs_mm']:.3f} mm",
    "h1 sd": f"at most {res['h1_curv_wobble']['max_sd_mm']:.2f} mm",
    "h2": f"up to {res['h2_split_wobble']['max_sd_mm']:.2f} mm (standard deviation;\n   median "
          f"{res['h2_split_wobble']['median_sd_mm']:.2f} mm over {res['h2_split_wobble']['wraps']} wraps)",
    "c0 V": f"({res['c0']['v_corners']}, radius {res['c0']['min_abs_rho_mm']:.2f} mm)",
    "rad lattice": f"{res['radius_lattice']['min_mm']:.2f} to +{res['radius_lattice']['max_mm']:.2f} mm (95 % within "
                   f"{res['radius_lattice']['p95_abs_mm']:.2f} mm)",
    "rad exact": f"{res['radius_exact']['min_mm']:.2f} to +{res['radius_exact']['max_mm']:.2f} mm\n  (95 % within "
                 f"{res['radius_exact']['p95_abs_mm']:.2f} mm); the +{res['radius_exact']['max_mm']:.2f}",
    "harness zone wraps": f"has {res['harness_zone_wraps']} zone wraps",
    "a levels": f"{res['a']['levels']} levels on the\n  pocket",
    "r_b": f"r_b = {inp['rho_fillet_mm']:.2f} / {inp['vertex_factor']:.4f} mm",
}
for k, v in prose.items():
    check(v.replace("−", "-") in note, f"note prose {k}: '{v}' not found")

# quoted sources
status = open(os.path.join(ROOT, "STATUS.md")).read()
check("876 → 202" in status and "curv_local 85 / 89" in status, "STATUS D6 quote")
split_txt = open(os.path.join(RENDERS, "pocket_geodesic_curv_split.txt")).read()
check("(927.2 mm²)" in split_txt and "18 levels" in split_txt, "split txt quote")
curv_txt = open(os.path.join(RENDERS, "pocket_geodesic_curv.txt")).read()
check("(27.50, 35.31)" in curv_txt and "(29.04, 39.00)" in curv_txt, "curv txt quote")
c11 = open(os.path.join(ROOT, "P2_C2_geodesic_fixes.md")).read()
check("3836 nodes, 959 mm²" in c11, "C11 zone quote")
check(res["h2_split_wobble"]["wraps"] > 0 and "18 zone levels" in note, "note split levels")

# ── 5. The qualitative claims ─────────────────────────────────────────
q = tol / D
ideas = ("a", "c0", "b", "c", "d", "a_c")
for k in ideas:
    check(res[k]["sw_interior_max"] <= c + q, f"{k} interior within the ceiling + quantum")
    check(res[k]["apm_interior_max"] <= res["a"]["apm_near_interior_max"] + 1e-12,
          f"{k} area per mm at or under the near-side reference")
    check(res[k]["max_gap_mm"] <= s + 1e-6, f"{k} gap")
    check(res[k]["residue_max_wall_distance_mm"] < 1.5, f"{k} residue near a wall")
check(res["a0"]["sw_interior_max"] > c + q and res["a0"]["sw_interior_over"] > 0, "a0 over the ceiling")
check(res["a"]["residue_interior_mm2"] == 0.0, "a leaves no residue")
check(res["a"]["sw_near_end_max"] > 1.5 * c, "arc ends read high on the near side")
check(res["b"]["length_mm"] > 2 * res["c0"]["length_mm"], "b more than double")
check(res["d"]["arcs"] == res["d"]["levels"], "d one arc per level")
check(diag.ends("d") == {("island", "wall"): res["d"]["levels"]}, "d arcs run island to wall")
check(res["d"]["sw_interior_max"] - c < q / 2, "d over by less than half the quantum")
for k in ("c", "d", "a_c", "b"):
    check(res[k]["v_corners"] == 0, f"{k} has no V")
check(res["c"]["min_abs_rho_mm"] >= inp["rho_fillet_mm"], "c corners not tighter than the fillet")
check(res["c0"]["min_abs_rho_mm"] < 1.0 and res["a"]["min_abs_rho_mm"] < 1.0, "V corners are sharp")
plan = {k: res["derived"][k + "_plan_arcs"] for k in ("a", "c0", "b", "c", "c_star", "d", "a_c")}
check(min(plan, key=plan.get) == "a_c", f"a_c has the fewest open arcs {plan}")
check(res["c_star"]["sw_interior_max"] > 2 * c, "c_star crosses standing stock")
h1 = res["h1_curv_vs_exact"]
check(h1["max_abs_mm"] < inp["cell"] and h1["mean_mm"] < 0, "lattice error under one cell, outward")
check(res["h2_split_wobble"]["max_sd_mm"] > 10 * res["h1_curv_wobble"]["max_sd_mm"], "seed bend >> lattice")
check(abs(res["first_region"]["harness_levels_dev_mm"]) < 0.025 + 1e-9 or
      format(res["first_region"]["harness_levels_dev_mm"], ".3f") == "0.025", "march reproduces C5 levels")
# c_star: the worst interior step lies at the island's far point
far_pt = O + rh_ * (O - seed) / dd
cv = diag.load("c_star")
rp = diag.Replay()
best = (0, None)
for lvl in diag.first:
    for P in lvl:
        rp.cut(P)
for lvl in cv:
    for P in lvl:
        w = rp.cut(P)[0]
        ss = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
        inner = (ss > R) & (ss < ss[-1] - R)
        if inner.any() and w[inner].max() > best[0]:
            best = (w[inner].max(), P[inner][np.argmax(w[inner])])
check(np.hypot(*(best[1] - far_pt)) < R, f"c_star worst step at {best[1]}, far point {far_pt}")

# ── 6. (e)/(f): the whole-region morphs ───────────────────────────────
blk2 = note.split("<!-- whole:begin -->")[1].split("<!-- whole:end -->")[0].strip()
check(blk2 == page.whole_table(res).strip(), "note (e)/(f) table differs from a fresh build")
W = res["whole"]
st, pk = W["star"], W["pocket"]
h4 = res["h4"]
prose2 = {
    "regions": f"({st['region_mm2']:.0f} mm² on the grid; pocket {pk['region_mm2']:.0f} mm²)",
    "visible": f"(fraction {st['visible_fraction']:.2f})",
    "h4 star": f"{h4['star_geodesic_curv']['cut_mm']:.1f} mm, {h4['star_geodesic_curv']['entries']} entries",
    "h4 star cs": f"ContourSpiral {h4['star_contour_spiral']['cut_mm']:.1f} mm, {h4['star_contour_spiral']['entries']} entries",
    "h4 pocket": f"{h4['pocket_geodesic_curv']['cut_mm']:.1f} mm, {h4['pocket_geodesic_curv']['entries']}",
    "h4 pocket cs": f"ContourSpiral {h4['pocket_contour_spiral']['cut_mm']:.1f} mm, {h4['pocket_contour_spiral']['entries']} entries",
    "star e loops": f"({st['e']['loops']} loops in {st['e']['levels']} levels)",
    "pocket e multi": f"({pk['e']['levels_multi']} levels)",
    "star f multi": f"Star (f): {st['f']['levels_multi']} levels split into up to {st['f']['max_loops_per_level']} loops",
    "pocket f stop": f"stops at u = {pk['f']['last_level']:.4f}",
    "pocket f basin": f"(lowest value {pk['f']['zone_min']:.4f})",
    "pocket f reach": f"only at u = {pk['f']['reach_level']:.4f}",
    "pocket f residue": f"{pk['f']['residue_interior_mm2']:.0f} mm² stay uncut",
    "star e corners": f"tip in {st['e']['narrowest_near_inner']} of {st['e']['located_loops']} loops; the widest lies nearer a tip in {st['e']['widest_near_tip']} of them",
    "star e median": f"Median min/max gap per loop {st['e']['median_gap_ratio']:.4f}",
    "pocket e median": f"Pocket (e): median {pk['e']['median_gap_ratio']:.4f}",
    "cost": f"{st['e']['len_vs_curv']:.1f} times the v4 `geodesic_curv` cut length\n    on the star and {pk['e']['len_vs_curv']:.1f} times",
    "pocket e load": f"({pk['e']['sw_interior_max']:.4f})",
    "star e load": f"last {len(st['e']['over_levels'])} loops at {len(st['e']['over_tips'])} of the five tips",
    "star e load2": f"({st['e']['sw_interior_max']:.4f}; every such sample within {st['e']['over_wall_max']:.1f} mm",
    "f load": f"{st['f']['sw_interior_max']:.4f} on the star, {pk['f']['sw_interior_max']:.4f} on the pocket",
}
for k, v in prose2.items():
    check(v in note, f"note (e)/(f) prose {k}: '{v}' not found")
# sources of the method
src = open(os.path.join(ROOT, "..", "..", ".claude", "worktrees", "adaptive-head", "crates", "rs_cam_core", "src",
                        "adaptive", "research_geodesic.rs")).read()
check("GeodesicField::on_lattice(lattice, inside, seed, r, s, options.solver, &walls)" in src
      and "let r = params.tool_radius;" in src, "entry disc radius R from geodesic_segments")
for nm, seed in (("star", "seed Some([49.5, 50.5]), seed radius 3.00 mm"),
                 ("pocket", "seed Some([18.5, 18.5]), seed radius 3.00 mm")):
    check(seed in open(os.path.join(HERE, "..", "renders4", f"{nm}_geodesic_curv.txt")).read(), f"{nm} seed")
# qualitative claims
check(st["visible_fraction"] == 1.0, "star region visible from the seed")
check(st["e"]["all_closed"] == "yes" and st["e"]["levels_multi"] == 0 and st["e"]["loops"] == st["e"]["levels"],
      "star (e): one closed loop per level")
check(pk["e"]["all_closed"] == "yes" and pk["e"]["max_loops_per_level"] == 2, "pocket (e): closed, two per late level")
check(abs(pk["f"]["last_level"] - pk["f"]["zone_min"]) < 1e-6 and pk["f"]["reach_level"] > pk["f"]["zone_min"] + 0.01,
      "pocket (f) stops at its second basin")
ol = st["e"]["over_levels"]
check(ol == list(range(st["e"]["levels"] - len(ol), st["e"]["levels"])), "star (e) overs on the last loops only")
check(st["e"]["over_wall_max"] < 0.5, "star (e) overs next to the wall")
check(pk["e"]["sw_interior_max"] <= c + q and pk["e"]["sw_interior_over"] == 0, "pocket (e) holds")
for nm in ("star", "pocket"):
    check(W[nm]["f"]["sw_interior_max"] > 2 * c, f"{nm} (f) over at the medial axis")
    check(W[nm]["e"]["len_vs_curv"] > 2, f"{nm} (e) cut length many times geodesic_curv")
check(st["e"]["narrowest_near_inner"] == st["e"]["located_loops"], "star (e) narrowest gap at the inner corners")
# the v1 page puts (e)/(f) first; v2 puts (e), (f), (a) first, star beside pocket
check(html.index("0. Whole-region morphs") < html.index("1. Why the far side wobbles"), "(e)/(f) first on v1")
i0 = html2.index("0. Morph fields (e), (f) and the plan (a)")
check(i0 < html2.index("1. Why the far side wobbles"), "(e), (f), (a) first on v2")
check(html2.index("star: (e)") < html2.index("pocket: (e)") < html2.index("star: (f)") < html2.index("star: (a)"),
      "v2: star beside pocket, (e) then (f) then (a)")
check(html2.count('src="data:image/png;base64,') == 4, "v2 embeds the four v4 harness PNGs")
r4 = os.listdir(os.path.join(HERE, "..", "renders4"))
check(not any("parallel" in f for f in r4), "the harness has no ContourParallel render")
harness_src = open(os.path.join(ROOT, "..", "..", ".claude", "worktrees", "adaptive-head", "crates", "rs_cam_core",
                                "tests", "adaptive_geodesic_2d_renders_g1.rs")).read()
check("The 2D op exposes no contour-parallel strategy" in harness_src, "harness doc: no contour-parallel column")
# (a) on both shapes, and the comparisons against it
for nm in ("star", "pocket"):
    A = W[nm]["a"]
    check(A["sw_interior_max"] <= c + q and A["sw_interior_over"] == 0, f"{nm} (a) holds the ceiling")
    check(A["worst_gap_ratio"] > 0.98, f"{nm} (a) even gap")
    check(A["open_arcs"] > 0 and A["one_closed"] == "no", f"{nm} (a) has open arcs")
    check(A["residue_max_wall_distance_mm"] < R, f"{nm} (a) residue next to a wall")
    for k in ("e", "f"):
        check(W[nm][k]["extra_levels_vs_a"] > 0 and W[nm][k]["length_vs_a"] > 2, f"{nm} ({k}) costs more than (a)")
check(W["pocket"]["a"]["levels"] == res["a"]["levels"], "whole.py (a) on the pocket = run_c15 (a)")
check([W[nm][k]["one_closed"] == "yes" for nm in ("star", "pocket") for k in ("e", "f", "a")].count(True) == 1
      and W["star"]["e"]["one_closed"] == "yes", "only star (e) keeps one closed loop per level")
check(W["pocket"]["f"]["one_closed"].startswith("yes, but it stops"), "pocket (f) flagged as stopped")
s8 = note.split("## 8. Page v2")[1]
sa, pa, se, sf, pe, pf = (W["star"]["a"], W["pocket"]["a"], W["star"]["e"], W["star"]["f"], W["pocket"]["e"],
                          W["pocket"]["f"])
prose3 = [
    f"{sa['levels']} levels, {sa['loops']} wraps, {sa['open_arcs']} open arcs",
    f"interior width {sa['sw_interior_max']:.4f} D (0 over c)",
    f"(median min/max per wrap {sa['median_gap_ratio']:.4f}), {sa['length_mm']:.0f} mm of cut",
    f"({sa['residue_interior_mm2']:.1f} mm²) lies within {sa['residue_max_wall_distance_mm']:.2f} mm of a wall",
    f"{pa['levels']} levels (as in §2), {pa['open_arcs']} open arcs",
    f"{pa['sw_interior_max']:.4f} D, {pa['length_mm']:.0f} mm, residue {pa['residue_interior_mm2']:.1f} mm² within "
    f"{pa['residue_max_wall_distance_mm']:.2f} mm",
    f"star (e) needs {se['extra_levels_vs_a']} more levels and {se['length_vs_a']:.1f} times the cut length",
    f"star (f) {sf['extra_levels_vs_a']} more levels and {sf['length_vs_a']:.1f} times",
    f"pocket (e) {pe['extra_levels_vs_a']} more levels and\n  {pe['length_vs_a']:.1f} times",
    f"at {se['length_vs_a']:.1f} times the cut length of (a)",
    f"its last {W['star']['e_tip_levels']} loops read {se['sw_interior_max']:.2f} D at the tips",
]
for v in prose3:
    check(v in s8, f"note §8: '{v}' not found")

print("assert_c15:", "FAILED" if FAILS else "all claims hold",
      f"({len(claims)} page claims over v1 and v2, {len(prose) + len(prose2) + len(prose3)} note claims, 2 note tables)")
sys.exit(1 if FAILS else 0)
