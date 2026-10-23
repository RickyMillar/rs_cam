# C15: write the look page farside_ideas_v1.html from results.json and
# curves.npz (run run_c15.py first). Every number on the page is a
# <span data-claim="key" data-fmt="fmt"> that assert_c15.py checks.
import json
import math
import os
import re
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from geom import (R, S, D, CD, C_FRAC, TOL, SEED, ISL_C, ISL_R, HOLE_R, A_UP, B_LO, P_UP, P_LO,
                  RECT, RX, OM, OM_RX, hidden)
from harness_svg import wraps as harness_wraps

PAGE = os.path.join(HERE, "..", "..", "farside_ideas_v1.html")
PAGE2 = os.path.join(HERE, "..", "..", "farside_ideas_v2.html")


def load_results():
    with open(os.path.join(HERE, "results.json")) as f:
        return json.load(f)


def load_curves():
    d = np.load(os.path.join(HERE, "curves.npz"))
    out = {}
    for key in d.files:
        k, i, j = key.split("|")
        out.setdefault(k, {}).setdefault(int(i), []).append((int(j), d[key].astype(float)))
    return {k: [[P for _, P in sorted(v[i], key=lambda t: t[0])] for i in sorted(v)] for k, v in out.items()}


def derived(res, curves):
    """Numbers the page quotes that are counts over the curves."""
    first = sum(len(l) for l in curves["first"])
    out = {"first_arcs": first}
    for k in ("a", "a0", "a_c"):
        out[k + "_plan_arcs"] = sum(len(l) for l in curves[k]) + res[k]["v_corners"]
    for k in ("c0", "b", "c", "c_star", "d"):
        out[k + "_plan_arcs"] = first + sum(len(l) for l in curves[k]) + res[k]["v_corners"]
        out[k + "_zone_open"] = res[k]["arcs"] + res[k]["v_corners"]
    for k in ("a", "a0", "a_c"):
        out[k + "_zone_open"] = res[k]["arcs"] + res[k]["v_corners"]
    return out


def load_whole():
    with open(os.path.join(HERE, "results_whole.json")) as f:
        w = json.load(f)
    d = np.load(os.path.join(HERE, "curves_whole.npz"))
    cur = {}
    for key in d.files:
        nm, k, i, j = key.split("|")
        cur.setdefault((nm, k), {}).setdefault(int(i), []).append((int(j), d[key].astype(float)))
    cur = {kk: [[P for _, P in sorted(v[i], key=lambda t: t[0])] for i in sorted(v)] for kk, v in cur.items()}
    return w, cur


def harness4():
    """Cut length and entries of the v4 harness renders (renders4/*.txt)."""
    out = {}
    base = os.path.join(HERE, "..", "renders4")
    for nm in ("star", "pocket"):
        for col in ("geodesic_curv", "contour_spiral"):
            txt = open(os.path.join(base, f"{nm}_{col}.txt")).read()
            cut = float(re.search(r"cut length: ([0-9.]+) mm", txt).group(1))
            ent = int(re.search(r"entries \(plunges\): ([0-9]+)", txt).group(1))
            out[f"{nm}_{col}"] = dict(cut_mm=cut, entries=ent)
    return out


def add_whole(res):
    w, cur = load_whole()
    h4 = harness4()
    for nm in ("star", "pocket"):
        ideal = w[nm]["region_mm2"] / S
        w[nm]["ideal_mm"] = ideal
        for k in ("e", "f"):
            w[nm][k]["all_closed"] = "yes" if w[nm][k]["closed"] == w[nm][k]["loops"] else "no"
            w[nm][k]["len_vs_ideal"] = w[nm][k]["length_mm"] / ideal
            w[nm][k]["len_vs_curv"] = w[nm][k]["length_mm"] / h4[f"{nm}_geodesic_curv"]["cut_mm"]
        for k in ("e", "f", "a"):
            w[nm][k]["one_closed"] = "yes" if (w[nm][k]["open_arcs"] == 0 and w[nm][k]["levels_multi"] == 0) else "no"
            if "zone_min" in w[nm][k] and abs(w[nm][k]["last_level"] - w[nm][k]["zone_min"]) < 1e-6:
                w[nm][k]["one_closed"] = "yes, but it stops at the second basin"
    se = w["star"]["e"]
    w["star"]["e_tip_levels"] = len(se["over_levels"])
    w["star"]["e_tip_wall_max"] = se["over_wall_max"]
    w["star"]["e_tip_count"] = len(se["over_tips"])
    res["whole"] = w
    res["h4"] = h4
    return cur


def get(res, key):
    v = res
    for part in key.split("."):
        v = v[part]
    return v


FMTS = {}


def claim(res, key, fmt):
    v = get(res, key)
    FMTS[key] = fmt
    return f'<span data-claim="{key}" data-fmt="{fmt}">{format(v, fmt)}</span>'


# ── SVG ────────────────────────────────────────────────────────────────

def poly(P, every=3):
    Q = P[::every]
    if len(P) and (len(Q) == 0 or not np.allclose(Q[-1], P[-1])):
        Q = np.vstack([Q, P[-1:]])
    return " ".join(f"{x:.2f},{y:.2f}" for x, y in Q)


def zone_outline():
    """The zone boundary for the shading: island far arc, the two shadow
    segments and the outer wall between them (sampled)."""
    pts = []
    # upper segment P_UP -> A_UP
    pts += [P_UP, A_UP]
    # along the wall from A_UP clockwise to B_LO (top wall, corner, right wall)
    xs = np.linspace(A_UP[0], OM[1] - OM_RX, 30)
    pts += [np.array([x, OM[3]]) for x in xs]
    for a in np.linspace(math.pi / 2, 0, 20):
        pts.append(np.array([OM[1] - OM_RX + OM_RX * math.cos(a), OM[3] - OM_RX + OM_RX * math.sin(a)]))
    for y in np.linspace(OM[3] - OM_RX, B_LO[1], 30):
        pts.append(np.array([OM[1], y]))
    pts += [B_LO, P_LO]
    a0 = math.atan2(P_LO[1] - ISL_C[1], P_LO[0] - ISL_C[0])
    a1 = math.atan2(P_UP[1] - ISL_C[1], P_UP[0] - ISL_C[0])
    if a1 < a0:
        a1 += 2 * math.pi
    for a in np.linspace(a0, a1, 60):
        pts.append(ISL_C + HOLE_R * np.array([math.cos(a), math.sin(a)]))
    return " ".join(f"{p[0]:.2f},{p[1]:.2f}" for p in pts)


def panel(layers, label):
    """layers: list of (curves, colour, width). One frame for every panel."""
    out = [f'<svg viewBox="4 4 72 52" role="img" aria-label="{label}" class="pane">',
           '<g transform="translate(0,60) scale(1,-1)">',
           f'<rect x="{RECT[0]}" y="{RECT[2]}" width="{RECT[1]-RECT[0]}" height="{RECT[3]-RECT[2]}" '
           f'rx="{RX}" fill="#ffffff" stroke="#000" stroke-width="0.3"/>',
           f'<polygon points="{zone_outline()}" fill="#fdf3d0" stroke="none"/>',
           f'<circle cx="{ISL_C[0]}" cy="{ISL_C[1]}" r="{ISL_R}" fill="#e6e6e6" stroke="#000" stroke-width="0.3"/>',
           f'<circle cx="{ISL_C[0]}" cy="{ISL_C[1]}" r="{HOLE_R}" fill="none" stroke="#999" stroke-width="0.15" stroke-dasharray="0.6,0.6"/>',
           f'<line x1="{SEED[0]}" y1="{SEED[1]}" x2="{A_UP[0]:.2f}" y2="{A_UP[1]:.2f}" stroke="#2a9d4a" stroke-width="0.2" stroke-dasharray="1,0.7"/>',
           f'<line x1="{SEED[0]}" y1="{SEED[1]}" x2="{B_LO[0]:.2f}" y2="{B_LO[1]:.2f}" stroke="#2a9d4a" stroke-width="0.2" stroke-dasharray="1,0.7"/>',
           f'<circle cx="{SEED[0]}" cy="{SEED[1]}" r="0.6" fill="#d22"/>']
    for curves, col, w in layers:
        for lvl in curves:
            for P in lvl:
                out.append(f'<polyline points="{poly(P)}" fill="none" stroke="{col}" stroke-width="{w}" '
                           'stroke-linejoin="round" stroke-linecap="round"/>')
    out.append("</g></svg>")
    return "\n".join(out)


def panel_shape(sh, layers, label):
    """The same drawing for any shape: its material outline, the entry, and
    the layers (curves, colour, width)."""
    mb = sh.mat_box
    vb = f"{mb[0] - 1} {mb[2] - 1} {mb[1] - mb[0] + 2} {mb[3] - mb[2] + 2}"
    flip = mb[2] + mb[3]
    out = [f'<svg viewBox="{vb}" role="img" aria-label="{label}" class="pane">',
           f'<g transform="translate(0,{flip}) scale(1,-1)">']
    o = sh.outline()
    if "rect" in o:
        (x0, x1, y0, y1), rx = o["rect"]
        out.append(f'<rect x="{x0}" y="{y0}" width="{x1-x0}" height="{y1-y0}" rx="{rx}" fill="#fff" stroke="#000" stroke-width="0.3"/>')
    else:
        pts = " ".join(f"{x},{y}" for x, y in o["poly"])
        out.append(f'<polygon points="{pts}" fill="#fff" stroke="#000" stroke-width="0.3"/>')
    for cc, rr in o["circles"]:
        out.append(f'<circle cx="{cc[0]}" cy="{cc[1]}" r="{rr}" fill="#e6e6e6" stroke="#000" stroke-width="0.3"/>')
    out.append(f'<circle cx="{sh.seed[0]}" cy="{sh.seed[1]}" r="0.6" fill="#d22"/>')
    for curves, col, w in layers:
        for lvl in curves:
            for P in lvl:
                out.append(f'<polyline points="{poly(P)}" fill="none" stroke="{col}" stroke-width="{w}" '
                           'stroke-linejoin="round" stroke-linecap="round"/>')
    out.append("</g></svg>")
    return "\n".join(out)


GREY = "#c4c4c4"
BLUE = "#1f5fbf"
RED = "#d1342f"


def build(v2=False):
    res = load_results()
    cur = load_curves()
    res["derived"] = derived(res, cur)
    res["h1_neg_mean_mm"] = -res["h1_curv_vs_exact"]["mean_mm"]
    wcur = add_whole(res)
    first = cur["first"]
    C = lambda k, f=".2f": claim(res, k, f)

    hwc = [[w for w in harness_wraps("pocket_geodesic_curv") if hidden(w[:, 0], w[:, 1]).sum() >= 2]]
    hws = [[w for w in harness_wraps("pocket_geodesic_curv_split") if hidden(w[:, 0], w[:, 1]).mean() > 0.5]]
    zone_only = lambda cv: [[P for P in lvl if hidden(P[:, 0], P[:, 1]).mean() > 0.0] for lvl in cv]

    def row(k, name):
        r = res[k]
        return (f"<tr><td>{name}</td><td>{C(k + '.levels', 'd')}</td>"
                f"<td>{C('derived.' + k + '_zone_open', 'd')}</td><td>{C(k + '.v_corners', 'd')}</td>"
                f"<td>{C('derived.' + k + '_plan_arcs', 'd')}</td>"
                f"<td>{C(k + '.length_mm', '.0f')}</td><td>{C(k + '.max_gap_mm', '.2f')}</td>"
                f"<td>{C(k + '.max_formula_frac', '.4f')}</td>"
                f"<td>{C(k + '.sw_interior_max', '.4f')}</td><td>{C(k + '.apm_interior_max', '.4f')}</td>"
                f"<td>{C(k + '.sw_end_max', '.2f')}</td>"
                f"<td>{C(k + '.min_abs_rho_mm', '.2f')}</td>"
                f"<td>{C(k + '.residue_interior_mm2', '.2f')} / {C(k + '.residue_max_wall_distance_mm', '.2f')}</td></tr>")

    def card(k, title, what, look, layers):
        r = res[k]
        return f"""
<section class="card">
<h3>{title}</h3>
<p class="what">{what}</p>
<p class="look"><b>Look at:</b> {look}</p>
{panel(layers, title)}
<table class="mini">
<tr><th>levels</th><td>{C(k + '.levels', 'd')}</td><th>wrap pieces in the zone (V corners)</th><td>{C('derived.' + k + '_zone_open', 'd')} ({C(k + '.v_corners', 'd')})</td></tr>
<tr><th>largest gap</th><td>{C(k + '.max_gap_mm', '.2f')} mm</td><th>largest width by the C5 formula</th><td>{C(k + '.max_formula_frac', '.4f')} D</td></tr>
<tr><th>replay, arc interior</th><td>{C(k + '.sw_interior_max', '.4f')} D</td><th>replay area per mm, arc interior</th><td>{C(k + '.apm_interior_max', '.4f')} D</td></tr>
<tr><th>smallest wrap radius</th><td>{C(k + '.min_abs_rho_mm', '.2f')} mm</td><th>cut length in the zone</th><td>{C(k + '.length_mm', '.0f')} mm</td></tr>
</table>
</section>"""

    inp = "inputs"
    html = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{'Morph and far-side wraps' if v2 else 'Far-side wrap ideas'}</title>
<style>
:root {{ --bg:#fbfaf7; --ink:#1b1b1b; --muted:#5b5b5b; --line:#d9d6cf; --card:#ffffff; --accent:#1f5fbf; }}
body {{ background:var(--bg); color:var(--ink); font:15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; margin:0; }}
main {{ max-width:1180px; margin:0 auto; padding:20px 16px 60px; }}
h1 {{ font-size:26px; margin:6px 0 4px; }}
h2 {{ font-size:19px; margin:30px 0 8px; border-bottom:1px solid var(--line); padding-bottom:4px; }}
h3 {{ font-size:16px; margin:0 0 6px; }}
p {{ margin:6px 0; max-width:78ch; }}
.sub {{ color:var(--muted); }}
.grid2 {{ display:grid; grid-template-columns:repeat(auto-fit, minmax(420px, 1fr)); gap:12px; margin-bottom:14px; }}
.grid4 {{ display:grid; grid-template-columns:repeat(auto-fit, minmax(260px, 1fr)); gap:12px; margin-bottom:14px; }}
.grid {{ display:grid; grid-template-columns:repeat(auto-fit, minmax(340px, 1fr)); gap:14px; }}
.card {{ background:var(--card); border:1px solid var(--line); border-radius:8px; padding:12px; }}
.pane {{ width:100%; height:auto; display:block; margin:6px 0; background:#fff; }}
.what {{ font-weight:600; }}
.look {{ color:var(--muted); }}
table {{ border-collapse:collapse; font-size:13px; }}
table.mini {{ width:100%; }}
table.mini th {{ text-align:left; font-weight:500; color:var(--muted); padding:2px 6px 2px 0; }}
table.mini td {{ padding:2px 10px 2px 0; font-variant-numeric:tabular-nums; }}
.wide {{ overflow-x:auto; }}
table.all th, table.all td {{ border-bottom:1px solid var(--line); padding:4px 8px; text-align:right; font-variant-numeric:tabular-nums; }}
table.all th:first-child, table.all td:first-child {{ text-align:left; }}
.key span.sw {{ display:inline-block; width:22px; height:3px; vertical-align:middle; margin:0 4px 0 10px; }}
.verdict {{ background:#eef3fb; border-left:4px solid var(--accent); padding:8px 12px; border-radius:4px; }}
code {{ font-size:13px; }}
</style></head>
<body><main>
<h1>{'Morph and far-side wraps' if v2 else 'Far-side wrap ideas'}</h1>
<p class="sub">Agent C15, 2026-10-24. Pocket fixture <code>ux_2d_pocket.toml</code>, tool-centre space (the pocket inset by R, the island grown by R). Python prototypes with exact geometry; no Rust, no lattice. Scripts: <code>phase2/c15/</code>; every number on this page is checked by <code>assert_c15.py</code>.</p>

<p>Each picture shows the same frame. The black outline is the pocket wall and the grey disc is the island. The dashed circle is the island grown by the tool radius: the cutter centre cannot go inside it. The yellow area is the <b>zone</b>: the area that the island hides from the entry (red dot). The green dashed lines are the two <b>shadow lines</b> from the entry, tangent to the grown island.</p>
<p class="key">Lines: <span class="sw" style="background:{GREY}"></span>near-side wraps (circles about the entry, the same in every idea)<span class="sw" style="background:{BLUE}"></span>the idea's wraps<span class="sw" style="background:{RED}"></span>the harness wraps (Rust, read from the render SVG)</p>

{whole_section(res, wcur, C) if v2 else ''}

<h2>1. Why the far side wobbles</h2>
<div class="grid">
<section class="card">
<h3>Lattice or not? geodesic_curv against the exact field</h3>
<p class="what">Red: the harness wraps of <code>geodesic_curv</code>. Blue: the exact geodesic distance at the same levels.</p>
<p class="look"><b>Look at:</b> the red and blue lines lie on top of each other behind the island. The lattice moves a wrap by at most {C('h1_curv_vs_exact.max_abs_mm', '.2f')} mm. Along one wrap the error changes by at most {C('h1_curv_wobble.max_sd_mm', '.2f')} mm (standard deviation).</p>
{panel([(cur['a0'], BLUE, 0.35), ([harness_wraps('pocket_geodesic_curv')], RED, 0.25)], 'lattice against exact')}
</section>
<section class="card">
<h3>Seed or not? the split column against its exact field</h3>
<p class="what">Red: the harness zone wraps of <code>geodesic_curv_split</code>. Blue: the exact distance from the two shadow lines (idea C11 without the lattice).</p>
<p class="look"><b>Look at:</b> the red wraps bend toward the wall near the top and the right. The blue wraps are straight. Along one red wrap the exact distance changes by up to {C('h2_split_wobble.max_sd_mm', '.2f')} mm (standard deviation; median {C('h2_split_wobble.median_sd_mm', '.2f')} mm).</p>
{panel([(first, GREY, 0.25), (cur['c0'], BLUE, 0.35), (hws, RED, 0.25)], 'split against exact')}
</section>
</div>
<div class="verdict">
<p><b>Hypothesis 1 (lattice pivots): a small part.</b> Behind the island the lattice wraps of <code>geodesic_curv</code> are within {C('h1_curv_vs_exact.max_abs_mm', '.2f')} mm of the exact wraps (RMS {C('h1_curv_vs_exact.rms_mm', '.2f')} mm; on average {C('h1_neg_mean_mm', '.2f')} mm farther out). That is less than one planner cell ({C(inp + '.cell', '.1f')} mm). The visible mess behind the island in <code>geodesic_curv</code> comes from the spacing: the C5 law reads the front radius as R + T about the entry. Behind the island the true radius is the distance to the tangent point on the island, which is smaller. At the harness levels the exact wraps read {C('a0.sw_interior_max', '.4f')} D in the replay (ceiling {C(inp + '.c', '.4f')} D) on {C('a0.sw_interior_over', 'd')} samples, all {C('a0_over_min_mm', '.1f')} to {C('a0_over_max_mm', '.1f')} mm from the grown island. The replay refuses such steps, and each refusal leaves stock for the cuts after it: the cascade of C5.</p>
<p><b>Hypothesis 2 (jagged seed): the main cause in the split column.</b> The zone field starts from the cleared lattice nodes, not from the shadow lines. Near the walls that set reaches into the zone, so the wraps bend toward it. The bend is {C('h2_split_wobble.max_sd_mm', '.2f')} mm against {C('h1_curv_wobble.max_sd_mm', '.2f')} mm for the lattice alone.</p>
<p><b>Hypothesis 3 (the V at the meeting line): a corner, not a wobble.</b> The exact split wraps (blue, right) are straight and meet in a V with a corner radius of {C('c0.min_abs_rho_mm', '.2f')} mm. Each V splits a wrap ({C('c0.v_corners', 'd')} V corners in the exact split). Ideas (c) and (d) remove it.</p>
</div>

<h2>2. The ideas</h2>
<p>Each idea chooses its levels by one rule: every point of a new wrap is at most the C5 law gap from the cut, with the radius read from the wrap's own curvature over one pass step ({C(inp + '.pass_step', '.1f')} mm each side). The replay then cuts the wraps in order on a {C(inp + '.tolerance', '.1f')} mm grid and reads the swept width as the planner does. "Arc interior" means more than R from an arc end. The arc ends read high in every idea and on the clean near side too (near side: {C('a.sw_near_end_max', '.2f')} D), so the ends are the wall-end class that the C14 turns address.</p>
<div class="grid">
{card('a', '(a) Exact geodesic distance, curvature read locally',
      'One front from the entry; behind the island the wraps are circle involutes. The levels use the true local radius, not R + T.',
      'the far-side wraps continue the near-side circles without a break. They meet in a V at the top right.',
      [(cur['a'], BLUE, 0.3)])}
{card('b', '(b) Harmonic fill of the zone',
      'The zone wraps are level lines of u, where u solves the Laplace equation: u = 0 on the two shadow lines, u = 1 on the outer wall, and no flux through the island.',
      'all wraps run into the two corners where a shadow line meets the wall. The cut length is more than double.',
      [(first, GREY, 0.25), (cur['b'], BLUE, 0.3)])}
{card('c', '(c) Two fronts joined by a round union',
      'The zone fronts of the exact split, with the V at the meeting line replaced by a rounded corner of radius ' + C(inp + '.rho_fillet_mm', '.2f') + ' mm.',
      'each wrap turns round the meeting line in one smooth curve instead of a V.',
      [(first, GREY, 0.25), (cur['c'], BLUE, 0.3)])}
{card('d', '(d) One front only, from the lower shadow line',
      'Only the shadow line whose farthest zone point is nearer starts a front. It sweeps the whole zone and wraps round the island to the other shadow line.',
      'one wrap per level, each from the island to the wall. There is no meeting line.',
      [(first, GREY, 0.25), (cur['d'], BLUE, 0.3)])}
{card('a_c', '(a) with the round union of (c)',
      'The involute fronts of (a), with the V behind the island replaced by the same rounded corner.',
      'the wraps go round the island and close behind it in one smooth curve per level.',
      [(cur['a_c'], BLUE, 0.3)])}
</div>

<h2>3. All numbers</h2>
<p>Ceiling: c = {C(inp + '.c', '.4f')} D, c·D = {C(inp + '.cD', '.4f')} mm. Stepover s = {C(inp + '.s', '.1f')} mm, R = {C(inp + '.R', '.1f')} mm. Zone area {C(inp + '.zone_mm2', '.0f')} mm². For (a) the levels are of the whole pocket; for the split ideas they are zone levels after the {C('first_region.levels', 'd')} near-side levels.</p>
<div class="wide"><table class="all">
<tr><th>idea</th><th>levels</th><th>wrap pieces in zone, V splits included</th><th>V corners</th><th>open arcs, whole plan</th><th>zone cut length mm</th><th>largest gap mm</th><th>C5 formula max (D)</th><th>replay interior max (D)</th><th>area/mm interior max (D)</th><th>arc-end max (D)</th><th>smallest radius mm</th><th>interior residue mm² / max distance from a wall mm</th></tr>
{row('a0', '(a) at the harness levels')}
{row('a', '(a) exact, local radius')}
{row('c0', 'exact split (C11 without the lattice)')}
{row('b', '(b) harmonic')}
{row('c', '(c) round union')}
{row('c_star', '(c) with the radius ρ*')}
{row('d', '(d) one front')}
{row('a_c', '(a) + (c)')}
</table></div>
<p>The harness split column has {C('harness_zone_wraps', 'd')} zone wraps for comparison. "(c) with the radius ρ*" uses the fillet radius ρ* = {C(inp + '.rho_star', '.2f')} mm that the brief suggested. That fillet is so wide that the round union no longer stays next to the cut: one wrap crosses standing stock and reads {C('c_star.sw_interior_max', '.2f')} D. The residue is a sliver at the island next to a shadow line, less than 1.5 mm from the wall; the wall pass removes it.</p>

<h2>4. Recommendation</h2>
<div class="verdict">
<p><b>Next Rust step: idea (a), then (c) on top of it.</b> Keep one field from the entry (no split). Space the levels by the C5 law with the radius read from each level's own curvature, and take the smallest allowed gap along the level. Behind the island this holds the ceiling in the replay ({C('a.sw_interior_max', '.4f')} D) where the harness levels did not ({C('a0.sw_interior_max', '.4f')} D). It costs {C('a_extra_levels', 'd')} levels over the whole pocket and no residue is left.</p>
<p>The far-side wraps of (a) are the involutes that continue the near-side circles: the clean pattern of the near side. Idea (c) then rounds the V behind the island, for {C('a_c_extra_levels', 'd')} more levels. Idea (d) is the best split variant: one arc per level and no meeting line. Idea (b) needs more than double the cut length and is not recommended.</p>
</div>
</main></body></html>
"""
    return res, html


def main():
    # v1 is kept as published (the lead's instruction, 2026-10-24); only v2 is
    # written. assert_c15.py still checks every claim of both pages.
    for v2, path in ((True, PAGE2),):
        res, html = build(v2)
        with open(path, "w") as f:
            f.write(html)
        print("wrote", os.path.abspath(path), len(html))



ROWS = [("a0", "(a) at the harness levels"), ("a", "(a) exact, local radius"),
        ("c0", "exact split (C11, no lattice)"), ("b", "(b) harmonic"), ("c", "(c) round union"),
        ("c_star", "(c), fillet radius ρ*"), ("d", "(d) one front"), ("a_c", "(a) + (c)")]


def note_table(res):
    """The per-idea table of P2_C15_farside_ideas.md (assert_c15.py rebuilds
    it from fresh results and compares it with the note)."""
    d = res["derived"]
    head = ("| Idea | Levels | Zone pieces (V incl.) | V | Open arcs, plan | Zone cut mm | Max gap mm "
            "| C5 formula max D | Replay interior D | Area/mm interior D | Arc-end D | Min radius mm "
            "| Residue mm² / wall dist mm |\n|" + "---|" * 13)
    lines = [head]
    for k, name in ROWS:
        r = res[k]
        lines.append(
            f"| {name} | {r['levels']} | {d[k + '_zone_open']} | {r['v_corners']} | {d[k + '_plan_arcs']} "
            f"| {r['length_mm']:.0f} | {r['max_gap_mm']:.2f} | {r['max_formula_frac']:.4f} "
            f"| {r['sw_interior_max']:.4f} | {r['apm_interior_max']:.4f} | {r['sw_end_max']:.2f} "
            f"| {r['min_abs_rho_mm']:.2f} | {r['residue_interior_mm2']:.2f} / {r['residue_max_wall_distance_mm']:.2f} |")
    return "\n".join(lines)


def results_with_derived():
    res = load_results()
    cur = load_curves()
    res["derived"] = derived(res, cur)
    res["h1_neg_mean_mm"] = -res["h1_curv_vs_exact"]["mean_mm"]
    add_whole(res)
    return res


def whole_table(res):
    """The (e)/(f)/(a) table of the note (assert_c15.py rebuilds and compares it)."""
    lines = ["| Shape / idea | Levels / wraps | One closed loop each | Open arcs | Levels with 2+ wraps (max) "
             "| Max gap mm | Min/max gap per loop: median, worst | Interior width D (over) | Arc-end/seam D "
             "| Area/mm D | Cut mm (× (a)) | Extra levels vs (a) | Residue mm² / wall dist mm |",
             "|" + "---|" * 13]
    for nm in ("star", "pocket"):
        for k in ("e", "f", "a"):
            r = res["whole"][nm][k]
            vs = "—" if k == "a" else f"{r['length_vs_a']:.1f}"
            ex = "—" if k == "a" else f"{r['extra_levels_vs_a']}"
            lines.append(
                f"| {nm} ({k}) | {r['levels']} / {r['loops']} | {r['one_closed']} | {r['open_arcs']} "
                f"| {r['levels_multi']} ({r['max_loops_per_level']}) | {r['max_gap_mm']:.2f} "
                f"| {r['median_gap_ratio']:.4f}, {r['worst_gap_ratio']:.6f} | {r['sw_interior_max']:.4f} "
                f"({r['sw_interior_over']}) | {r['sw_seam_max']:.2f} | {r['apm_interior_max']:.4f} "
                f"| {r['length_mm']:.0f} ({vs}) | {ex} "
                f"| {r['residue_interior_mm2']:.1f} / {r['residue_max_wall_distance_mm']:.2f} |")
    return "\n".join(lines)


def png_data(path):
    import base64
    return "data:image/png;base64," + base64.b64encode(open(path, "rb").read()).decode()


def whole_section(res, wcur, C):
    """v2, section 0: (e), (f) and (a) on the star and the pocket, side by side,
    with the v4 harness renders."""
    from shapes import Shape
    base = os.path.join(HERE, "..", "renders4")
    shapes = {nm: Shape(nm) for nm in ("star", "pocket")}
    ideas = (
        ("e", "(e) harmonic morph",
         "The loops are the level lines of a smooth field that is 0 at the entry and 1 at every wall, so they start round and end in the wall shape."),
        ("f", "(f) distance-ratio morph",
         "The loops are the level lines of (distance from the entry) / (distance from the entry + distance to the nearest wall)."),
        ("a", "(a) exact distance from the entry, local radius",
         "The wraps are circles about the entry, cut off by the walls; this is the plan recommended on the far side (section 2)."),
    )

    def card(nm, k, title, what):
        r = f"whole.{nm}.{k}"
        cmp_a = "" if k == "a" else (
            f"<tr><th>levels more than (a)</th><td>{C(r + '.extra_levels_vs_a', 'd')}</td>"
            f"<th>cut length against (a)</th><td>{C(r + '.length_vs_a', '.1f')} times</td></tr>")
        return f"""<section class="card"><h3>{nm}: {title}</h3><p class="what">{what}</p>
{panel_shape(shapes[nm], [(wcur[(nm, k)], BLUE, 0.3)], nm + ' ' + k)}
<table class="mini">
<tr><th>levels / wraps</th><td>{C(r + '.levels', 'd')} / {C(r + '.loops', 'd')}</td><th>every wrap one closed loop</th><td>{C(r + '.one_closed', 's')}</td></tr>
<tr><th>open arcs</th><td>{C(r + '.open_arcs', 'd')}</td><th>levels with 2 or more wraps</th><td>{C(r + '.levels_multi', 'd')}</td></tr>
<tr><th>interior width, max</th><td>{C(r + '.sw_interior_max', '.4f')} D ({C(r + '.sw_interior_over', 'd')} samples over c)</td><th>arc-end / seam width, max</th><td>{C(r + '.sw_seam_max', '.2f')} D</td></tr>
<tr><th>min/max gap per loop: median, worst</th><td>{C(r + '.median_gap_ratio', '.4f')}, {C(r + '.worst_gap_ratio', '.6f')}</td><th>largest gap</th><td>{C(r + '.max_gap_mm', '.2f')} mm</td></tr>
<tr><th>cut length</th><td>{C(r + '.length_mm', '.0f')} mm</td><th>residue / max distance from a wall</th><td>{C(r + '.residue_interior_mm2', '.1f')} mm² / {C(r + '.residue_max_wall_distance_mm', '.2f')} mm</td></tr>
{cmp_a}
</table></section>"""

    rows = []
    for k, title, what in ideas:
        rows.append('<div class="grid2">' + card("star", k, title, what) + card("pocket", k, title, what) + "</div>")
    for col, title, what in (
            ("geodesic_curv", "harness v4: geodesic_curv", "The v4 render of the curvature-spaced wraps (the PNG of the harness)."),
            ("contour_spiral", "harness v4: ContourSpiral", "The v4 render of the shipped ContourSpiral strategy.")):
        cells = []
        for nm in ("star", "pocket"):
            h = f"h4.{nm}_{col}"
            cells.append(f"""<section class="card"><h3>{nm}: {title}</h3><p class="what">{what}</p>
<img class="pane" alt="{nm} {col}" src="{png_data(os.path.join(base, f'{nm}_{col}.png'))}">
<table class="mini"><tr><th>cut length (all passes)</th><td>{C(h + '.cut_mm', '.1f')} mm</td><th>entries</th><td>{C(h + '.entries', 'd')}</td></tr></table></section>""")
        rows.append('<div class="grid2">' + "".join(cells) + "</div>")
    W = "whole"
    return f"""<h2>0. Morph fields (e), (f) and the plan (a), star and pocket</h2>
<p>The operator saw other tools whose wraps start round and turn into the wall shape. (e) and (f) are two fields with no parameter that do this. (a) is the plan recommended for the far side, shown here on the star too. All three use the same level rule (the C5 law gap from each wrap's own curvature) and the same 0.1 mm replay. The harness renders are the v4 PNGs; the harness has no ContourParallel column (the 2D op runs contour-parallel only behind the narrow-region gate, render harness module doc).</p>
<p><b>How to read a card.</b> "Open arcs" are wraps that do not close; each costs a plunge or a turn (problem 2). "Interior width" leaves out R of arc length at each end or seam; the ends and seams are reported apart. "Min/max gap per loop" is the smallest gap of a wrap divided by its largest gap: 1 is even.</p>
{"".join(rows)}
<div class="verdict">
<p><b>Closed loops.</b> Only (e) on the star keeps every wrap one closed loop ({C(W + '.star.e.loops', 'd')} loops, 0 open arcs). On the pocket, (e) loops stay closed but {C(W + '.pocket.e.levels_multi', 'd')} late levels are two loops, one round the island and one along the wall. (f) splits {C(W + '.star.f.levels_multi', 'd')} star levels into up to {C(W + '.star.f.max_loops_per_level', 'd')} loops, and on the pocket it stops at u = {C(W + '.pocket.f.last_level', '.4f')}: behind the island u has a second low basin (lowest {C(W + '.pocket.f.zone_min', '.4f')}) that the loops from the entry reach only at u = {C(W + '.pocket.f.reach_level', '.4f')}. (a) has {C(W + '.star.a.open_arcs', 'd')} open arcs on the star and {C(W + '.pocket.a.open_arcs', 'd')} on the pocket.</p>
<p><b>Spacing and cost.</b> (a) keeps an even gap (median min/max {C(W + '.star.a.median_gap_ratio', '.4f')} on the star). One value of u per loop gives (e) a gap that follows the field's gradient, so the loops crowd at the inner corners (narrowest gap there in {C(W + '.star.e.narrowest_near_inner', 'd')} of {C(W + '.star.e.located_loops', 'd')} star loops; median {C(W + '.star.e.median_gap_ratio', '.4f')}). On the star (e) needs {C(W + '.star.e.extra_levels_vs_a', 'd')} more levels and {C(W + '.star.e.length_vs_a', '.1f')} times the cut length of (a); on the pocket {C(W + '.pocket.e.extra_levels_vs_a', 'd')} more levels and {C(W + '.pocket.e.length_vs_a', '.1f')} times.</p>
<p><b>Load.</b> (a) holds the ceiling in the interior on both shapes ({C(W + '.star.a.sw_interior_max', '.4f')} D star, {C(W + '.pocket.a.sw_interior_max', '.4f')} D pocket; c = {C('inputs.c', '.4f')} D). (e) holds it on the pocket ({C(W + '.pocket.e.sw_interior_max', '.4f')} D); on the star it reads {C(W + '.star.e.sw_interior_max', '.2f')} D on the last {C(W + '.star.e_tip_levels', 'd')} loops at {C(W + '.star.e_tip_count', 'd')} of the five tips. (f) reads up to {C(W + '.star.f.sw_interior_max', '.2f')} D at the corners where the nearest wall changes.</p>
<p><b>Verdict.</b> (e) removes the per-arc plunge only on a part without an island, and at {C(W + '.star.e.length_vs_a', '.1f')} times the cut length of (a). (f) fails on load. (a) stays the recommendation; its open arcs are the C14 turns' work.</p>
</div>
"""


if __name__ == "__main__":
    main()
