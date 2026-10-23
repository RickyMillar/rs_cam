# C17: write farside_ideas_v3.html and P2_C17_streamline_march.md from
# results_c17.json and curves_c17.npz (run run_c17.py first). Every number
# on the page is a <span data-claim="key" data-fmt="fmt">; every number in
# the note comes from the same claim() call. assert_c17.py checks both.
# Run: python3 -I page_c17.py
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stream as st  # noqa: E402
from page import panel_shape  # noqa: E402  (C15 drawing, same frame as v2)
import page_links as pl  # noqa: E402

ROOT = os.path.join(HERE, "..", "..")
PAGE = os.path.join(ROOT, "farside_ideas_v3.html")
NOTE = os.path.join(ROOT, "P2_C17_streamline_march.md")

BLUE, GREY, ORANGE = "#1f5fbf", "#9a9a9a", "#e08a00"
RUNS = ("h_step", "h_normal", "g_step")


def load():
    res = json.load(open(os.path.join(HERE, "results_c17.json")))
    d = np.load(os.path.join(HERE, "curves_c17.npz"))
    cur = {}
    for key in d.files:
        nm, run, i, part = key.split("|")
        cur.setdefault((nm, run), {}).setdefault(int(i), {})[part] = d[key]
    cur = {k: [v[i] for i in sorted(v)] for k, v in cur.items()}
    return res, cur


def derive(res):
    """Ratios the page quotes (assert_c17.py recomputes them)."""
    for nm in ("star", "pocket"):
        a, e = res[nm]["c15_a"], res[nm]["c15_e"]
        for k in RUNS:
            r = res[nm][k]
            r["wall_share"] = (r["wall_follow_mm"] + r["air_follow_mm"]) / r["length_mm"]
            r["moving_vs_a"] = r["moving_mm"] / a["length_mm"]
            r["length_vs_a"] = r["length_mm"] / a["length_mm"]
            r["length_vs_e"] = r["length_mm"] / e["length_mm"]
            r["chord_total_mm"] = r["moving_mm"] + r["run_chord_mm"]
            r["chord_total_vs_a"] = r["chord_total_mm"] / a["length_mm"]
            r["residue_reachable_mm2"] = r["residue_mm2"] - r["residue_unreachable_mm2"]
            r["extra_loops_vs_a"] = r["loops"] - a["levels"]
            r["extra_loops_vs_e"] = r["loops"] - e["levels"]
        e["length_vs_a"] = e["length_mm"] / a["length_mm"]
        r = res[nm]["h_step"]
        r["crossing_loops"] = sorted({c[0] for c in r["crossings_at"]})
        r["n_crossing_loops"] = len(r["crossing_loops"])
        if r["crossings_at"]:
            xy = np.array([c[1:] for c in r["crossings_at"]])
            r["crossing_x_min"], r["crossing_x_max"] = float(xy[:, 0].min()), float(xy[:, 0].max())
            r["crossing_y_min"], r["crossing_y_max"] = float(xy[:, 1].min()), float(xy[:, 1].max())
    vals = [res[nm]["h_step"]["length_vs_a"] for nm in ("star", "pocket")]
    vals += [res[nm]["c15_e"]["length_vs_a"] for nm in ("star", "pocket")]
    vals += [res["star"]["g_step"]["length_vs_a"]]  # the pocket control does not cut the zone
    res["closed_min"], res["closed_max"] = min(vals), max(vals)
    res["inputs"]["cD_frac"] = res["inputs"]["cD"] / res["inputs"]["D"]
    res["inputs"]["quantum_frac"] = res["inputs"]["tol"] / res["inputs"]["D"]
    return res


def get(res, key):
    v = res
    for part in key.split("."):
        v = v[int(part)] if isinstance(v, list) else v[part]
    return v


def fmt(v, f):
    if f == "list":
        return ", ".join(str(x) for x in v)
    return format(v, f)


class Claims:
    def __init__(self, res, html=True):
        self.res, self.html = res, html

    def __call__(self, key, f=".2f"):
        v = fmt(get(self.res, key), f)
        return f'<span data-claim="{key}" data-fmt="{f}">{v}</span>' if self.html else v


def march_layers(loops):
    """Moving pieces of every loop (blue), the last loop's stopped parts on
    the walls (grey) and in air (orange)."""
    moving, wall, air = [], [], []
    for lp in loops:
        P, was = lp["P"].astype(float), lp["was"]
        seg = ~(was & np.roll(was, -1))
        moving += pieces(P, seg)
    last = loops[-1]
    P, kd = last["P"].astype(float), last["kind"]
    both = (kd > 0) & (np.roll(kd, -1) > 0)
    isair = both & ((kd == 2) | (np.roll(kd, -1) == 2))
    wall += pieces(P, both & ~isair)
    air += pieces(P, isair)
    return [([wall], GREY, 0.45), ([moving], BLUE, 0.25), ([air], ORANGE, 0.45)]


def pieces(P, seg):
    """Polylines of the runs of flagged segments (segment i: point i to i+1)."""
    n = len(P)
    out = []
    if seg.all():
        return [np.vstack([P, P[:1]])]
    starts = np.nonzero(seg & ~np.roll(seg, 1))[0]
    for a in starts:
        idx = [a]
        b = a
        while seg[b % n]:
            b += 1
            idx.append(b % n)
        out.append(P[idx])
    return out


def c15_curves():
    d = np.load(os.path.join(st.C15, "curves_whole.npz"))
    cur = {}
    for key in d.files:
        nm, k, i, j = key.split("|")
        cur.setdefault((nm, k), {}).setdefault(int(i), []).append((int(j), d[key].astype(float)))
    return {kk: [[P for _, P in sorted(v[i], key=lambda t: t[0])] for i in sorted(v)] for kk, v in cur.items()}


TITLES = {
    "h_step": "streamline march (the lead's rule)",
    "h_normal": "streamline march, normal-gap step",
    "g_step": "control: exact-distance streamlines",
    "c15_e": "C15 (e): harmonic level lines",
    "c15_a": "C15 (a): exact-distance wraps",
}


def sentence(nm, k, C):
    p = f"{nm}.{k}"
    S = {
        ("star", "h_step"): lambda: f"Every loop is one closed curve: the loop moves out along the harmonic streamlines and runs along a wall where it reaches one, but {C(p + '.wall_share', '.0%')} of its length is that wall run.",
        ("star", "h_normal"): lambda: f"The same march with the step set by the distance to the cut: it looks the same and cuts {C(p + '.length_mm', '.0f')} mm, so the normal gap is not where the length goes.",
        ("star", "g_step"): lambda: f"The same march along straight rays from the entry: before a wall is reached it gives (a)'s circles to {C(p + '.max_radial_diff_mm', '.3f')} mm, and after that it is as long as the harmonic march.",
        ("star", "c15_e"): lambda: f"The C15 level lines of the same harmonic field: closed loops that crowd at the inner corners, {C(p + '.length_mm', '.0f')} mm.",
        ("star", "c15_a"): lambda: f"The C15 plan: circles cut off by the walls, {C(p + '.open_arcs', 'd')} open arcs, and only {C(p + '.length_mm', '.0f')} mm of cut.",
        ("pocket", "h_step"): lambda: f"The loop stays one closed curve round the island: the two fronts stop in air at a corridor from the island to the far corner (orange), and {C(p + '.wall_share', '.0%')} of the length is wall and corridor runs.",
        ("pocket", "h_normal"): lambda: f"With the normal-gap step the march fails: where a streamline runs along the cut the step grows to {C(p + '.step_max_mm', '.1f')} mm and the loops cross themselves {C(p + '.self_crossings', 'd')} times.",
        ("pocket", "g_step"): lambda: f"The exact-distance streamlines fold where the two branches meet behind the island, so the march stops there and leaves {C(p + '.residue_mm2', '.0f')} mm² uncut.",
        ("pocket", "c15_e"): lambda: f"The C15 level lines: {C(p + '.levels_multi', 'd')} late levels split into two loops, {C(p + '.length_mm', '.0f')} mm.",
        ("pocket", "c15_a"): lambda: f"The C15 plan: {C(p + '.open_arcs', 'd')} open arcs and {C(p + '.length_mm', '.0f')} mm of cut.",
    }
    return S[(nm, k)]()


def card(sh, nm, k, layers, C):
    p = f"{nm}.{k}"
    if k.startswith("c15"):
        rows = (f"<tr><th>levels / wraps</th><td>{C(p + '.levels', 'd')} / {C(p + '.loops', 'd')}</td>"
                f"<th>open arcs</th><td>{C(p + '.open_arcs', 'd')}</td></tr>"
                f"<tr><th>cut length</th><td>{C(p + '.length_mm', '.0f')} mm</td>"
                f"<th>mean width (area / length)</th><td>{C(p + '.mean_width_frac', '.3f')} D</td></tr>"
                f"<tr><th>interior width, max</th><td>{C(p + '.sw_interior_max', '.4f')} D</td>"
                f"<th>residue</th><td>{C(p + '.residue_interior_mm2', '.1f')} mm²</td></tr>")
    else:
        rows = (f"<tr><th>loops (all closed)</th><td>{C(p + '.loops', 'd')}</td>"
                f"<th>self-crossings</th><td>{C(p + '.self_crossings', 'd')}</td></tr>"
                f"<tr><th>cut length</th><td>{C(p + '.length_mm', '.0f')} mm</td>"
                f"<th>of it on stopped parts</th><td>{C(p + '.wall_follow_mm', '.0f')} wall + {C(p + '.air_follow_mm', '.0f')} air mm</td></tr>"
                f"<tr><th>moving part</th><td>{C(p + '.moving_mm', '.0f')} mm ({C(p + '.moving_vs_a', '.2f')} × (a))</td>"
                f"<th>mean width (area / length)</th><td>{C(p + '.mean_width_frac', '.3f')} D</td></tr>"
                f"<tr><th>interior width, max</th><td>{C(p + '.sw_interior_max', '.4f')} D ({C(p + '.sw_interior_over', 'd')} over c)</td>"
                f"<th>more than R from a wall</th><td>{C(p + '.sw_far_max', '.4f')} D ({C(p + '.sw_far_over', 'd')} over c)</td></tr>"
                f"<tr><th>residue (unreachable)</th><td>{C(p + '.residue_mm2', '.1f')} ({C(p + '.residue_unreachable_mm2', '.1f')}) mm²</td>"
                f"<th>sharpest kink at a stop</th><td>{C(p + '.kink_max_deg', '.0f')}°</td></tr>")
    return f"""<section class="card"><h3>{nm}: {TITLES[k]}</h3>
<p class="what">{sentence(nm, k, C)}</p>
{panel_shape(sh, layers, nm + ' ' + k)}
<table class="mini">{rows}</table></section>"""


def shape_table(nm, C):
    head = ("<tr><th>plan</th><th>loops / levels</th><th>open arcs</th><th>self-crossings</th><th>cut mm</th>"
            "<th>moving mm</th><th>wall + air runs mm</th><th>mean width D</th><th>interior max D (over c)</th>"
            "<th>&gt; R from wall, max D</th><th>interior p99 D</th><th>area/mm max D</th>"
            "<th>residue mm² (unreachable)</th></tr>")
    rows = [head]
    for k in RUNS:
        p = f"{nm}.{k}"
        rows.append(f"<tr><td>{TITLES[k]}</td><td>{C(p + '.loops', 'd')}</td><td>0</td><td>{C(p + '.self_crossings', 'd')}</td>"
                    f"<td>{C(p + '.length_mm', '.0f')}</td><td>{C(p + '.moving_mm', '.0f')}</td>"
                    f"<td>{C(p + '.wall_follow_mm', '.0f')} + {C(p + '.air_follow_mm', '.0f')}</td>"
                    f"<td>{C(p + '.mean_width_frac', '.3f')}</td>"
                    f"<td>{C(p + '.sw_interior_max', '.4f')} ({C(p + '.sw_interior_over', 'd')})</td>"
                    f"<td>{C(p + '.sw_far_max', '.4f')}</td><td>{C(p + '.sw_interior_p99', '.4f')}</td>"
                    f"<td>{C(p + '.apm_interior_max', '.4f')}</td>"
                    f"<td>{C(p + '.residue_mm2', '.1f')} ({C(p + '.residue_unreachable_mm2', '.1f')})</td></tr>")
    for k in ("c15_e", "c15_a"):
        p = f"{nm}.{k}"
        rows.append(f"<tr><td>{TITLES[k]}</td><td>{C(p + '.levels', 'd')}</td><td>{C(p + '.open_arcs', 'd')}</td><td>—</td>"
                    f"<td>{C(p + '.length_mm', '.0f')}</td><td>—</td><td>—</td><td>{C(p + '.mean_width_frac', '.3f')}</td>"
                    f"<td>{C(p + '.sw_interior_max', '.4f')}</td><td>—</td><td>—</td><td>{C(p + '.apm_interior_max', '.4f')}</td>"
                    f"<td>{C(p + '.residue_interior_mm2', '.1f')}</td></tr>")
    return '<div class="wide"><table class="all">' + "".join(rows) + "</table></div>"


CSS = """
:root { --bg:#fbfaf7; --ink:#1b1b1b; --muted:#5b5b5b; --line:#d9d6cf; --card:#ffffff; --accent:#1f5fbf; }
body { background:var(--bg); color:var(--ink); font:15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; margin:0; }
main { max-width:1180px; margin:0 auto; padding:20px 16px 60px; }
h1 { font-size:26px; margin:6px 0 4px; }
h2 { font-size:19px; margin:30px 0 8px; border-bottom:1px solid var(--line); padding-bottom:4px; }
h3 { font-size:16px; margin:0 0 6px; }
p, li { max-width:80ch; }
p { margin:6px 0; }
.sub { color:var(--muted); }
.grid { display:grid; grid-template-columns:repeat(auto-fit, minmax(330px, 1fr)); gap:12px; margin-bottom:14px; }
.card { background:var(--card); border:1px solid var(--line); border-radius:8px; padding:12px; }
.pane { width:100%; height:auto; display:block; margin:6px 0; background:#fff; }
.what { font-weight:600; }
table { border-collapse:collapse; font-size:13px; }
table.mini { width:100%; }
table.mini th { text-align:left; font-weight:500; color:var(--muted); padding:2px 6px 2px 0; }
table.mini td { padding:2px 10px 2px 0; font-variant-numeric:tabular-nums; }
.wide { overflow-x:auto; margin:8px 0 18px; }
table.all th, table.all td { border-bottom:1px solid var(--line); padding:4px 8px; text-align:right; font-variant-numeric:tabular-nums; }
table.all th:first-child, table.all td:first-child { text-align:left; }
.key span.sw { display:inline-block; width:22px; height:3px; vertical-align:middle; margin:0 4px 0 10px; }
.verdict { background:#eef3fb; border-left:4px solid var(--accent); padding:8px 12px; border-radius:4px; }
.pred { color:#7a4b00; }
.chartbox { background:#fff; border:1px solid var(--line); border-radius:8px; padding:8px 10px; }
.chart { width:100%; height:auto; display:block; }
.chart .tick { font-size:11px; fill:#5b5b5b; }
code { font-size:13px; }
"""


def answers(C):
    """The five questions of the brief, shared by the page and the note."""
    s, p = "star.h_step", "pocket.h_step"
    return [
        ("1. One closed loop?",
         f"Yes, on both shapes, for every loop ({C(s + '.loops', 'd')} on the star, {C(p + '.loops', 'd')} on the pocket). "
         f"A streamline that reaches the island stops on it, so the loop runs along the near side of the island as it runs along a wall. "
         f"The loop does not have to split. Behind the island the two fronts come together on the line from the island to the far corner. "
         f"There the rule must stop a point when its move cuts no stock (the air stop, {C(p + '.air_stops', 'd')} points). "
         f"The two fronts then stop on the two sides of a corridor of cleared stock, and every later loop runs up one side and down the other. "
         f"Without the air stop the points on that line never stop (seen in a trial; the march did not end). "
         f"The pocket loops cross themselves {C(p + '.self_crossings', 'd')} times, all in loops {C(p + '.crossing_loops', 'list')} at x {C(p + '.crossing_x_min', '.1f')} to {C(p + '.crossing_x_max', '.1f')}, y {C(p + '.crossing_y_min', '.1f')} to {C(p + '.crossing_y_max', '.1f')}, where the corridor meets the corner wall."),
        ("2. The real gap and load",
         f"The lead's prediction holds: the gap normal to the loop is at most the streamline step. "
         f"Normal gap / step: median {C(s + '.normal_over_step_median', '.3f')} and smallest {C(s + '.normal_over_step_min', '.3f')} on the star; "
         f"median {C(p + '.normal_over_step_median', '.3f')} and smallest {C(p + '.normal_over_step_min', '.3f')} on the pocket. "
         f"More than R from every wall the replay holds the ceiling to within one cell: {C(s + '.sw_far_max', '.4f')} D star, {C(p + '.sw_far_max', '.4f')} D pocket "
         f"(c = {C('inputs.c', '.4f')} D; one cell is {C('inputs.quantum_frac', '.4f')} D). "
         f"The interior samples over c ({C(s + '.sw_interior_over', 'd')} star, {C(p + '.sw_interior_over', 'd')} pocket of {C(s + '.sw_interior_n', 'd')} and {C(p + '.sw_interior_n', 'd')}) all lie within R of a wall. "
         f"On the star they are on loops {C(s + '.over_loops.0', 'd')} to {C(s + '.over_loops.-1', 'd')}, the loops that first reach the walls; the largest is {C(s + '.sw_interior_max', '.4f')} D. "
         f"Distribution in the interior: median {C(s + '.sw_interior_p50', '.3f')}, 90 % {C(s + '.sw_interior_p90', '.3f')}, 99 % {C(s + '.sw_interior_p99', '.3f')} D on the star. "
         f"The normal-gap variant steps farther where the streamline is oblique and reads up to {C('pocket.h_normal.sw_interior_max', '.2f')} D on the pocket."),
        ("3. Coverage",
         f"The star leaves {C(s + '.residue_mm2', '.1f')} mm²; {C(s + '.residue_unreachable_mm2', '.1f')} mm² of it is in the five tips, farther than R from every centre position, so no plan cuts it ((e) leaves {C('star.c15_e.residue_interior_mm2', '.1f')} mm²). "
         f"The pocket leaves {C(p + '.residue_mm2', '.2f')} mm² in one piece at ({C(p + '.residue_big_at.0', '.1f')}, {C(p + '.residue_big_at.1', '.1f')}), {C(p + '.residue_big_wall_mm', '.2f')} mm from the wall, in the corner where the corridor ends. "
         f"Where streamlines diverge the loop gets new points at the tolerance, so no wedge stays uncut."),
        ("4. Loops and length",
         f"Star: {C(s + '.loops', 'd')} loops against {C('star.c15_a.levels', 'd')} levels of (a) and {C('star.c15_e.levels', 'd')} of (e). "
         f"Cut length {C(s + '.length_mm', '.0f')} mm: {C(s + '.length_vs_a', '.1f')} times (a) and {C(s + '.length_vs_e', '.2f')} times (e). "
         f"Pocket: {C(p + '.loops', 'd')} loops, {C(p + '.length_mm', '.0f')} mm, {C(p + '.length_vs_a', '.1f')} times (a) and {C(p + '.length_vs_e', '.2f')} times (e). "
         f"The wall and corridor runs are {C(s + '.wall_share', '.0%')} (star) and {C(p + '.wall_share', '.0%')} (pocket) of the length: each loop runs along all the wall that earlier loops reached. "
         f"The moving part alone is {C(s + '.moving_vs_a', '.2f')} times (a) on the star and {C(p + '.moving_vs_a', '.2f')} times on the pocket. "
         f"A straight link across each stopped run instead of the wall run is a lower bound for a shortcut; it still gives {C(s + '.chord_total_vs_a', '.1f')} times (a) on the star."),
        ("5. Smoothness",
         f"Away from the stops (more than one pass step of arc) the smallest loop radius is {C(s + '.rho_min_free_mm', '.2f')} mm (star) and {C(p + '.rho_min_free_mm', '.2f')} mm (pocket); next to a stop it falls to {C(s + '.rho_min_moving_mm', '.2f')} mm. "
         f"Where a loop leaves a wall it turns by a median {C(s + '.kink_median_deg', '.0f')}° and at most {C(s + '.kink_max_deg', '.0f')}° (star; pocket {C(p + '.kink_max_deg', '.0f')}°) over one pass step: a corner in the loop."),
    ]


def build():
    res, cur = load()
    res = derive(res)
    sw, paths = pl.load_sweep()
    res["sw"] = pl.derive_sweep(sw)
    C = Claims(res, html=True)
    c15 = c15_curves()
    sections = []
    for nm in ("star", "pocket"):
        sh = st.Shape(nm)
        cards = []
        for k in RUNS:
            cards.append(card(sh, nm, k, march_layers(cur[(nm, k)]), C))
        for k, ck in (("c15_e", "e"), ("c15_a", "a")):
            cards.append(card(sh, nm, k, [(c15[(nm, ck)], BLUE, 0.25)], C))
        sections.append(f'<h2>{"1" if nm == "star" else "2"}. {nm.capitalize()}</h2>\n<div class="grid">{"".join(cards)}</div>\n{shape_table(nm, C)}')
    qa = "".join(f"<h3>{t}</h3><p>{body}</p>" for t, body in answers(C))
    s, p = "star.h_step", "pocket.h_step"
    html = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Loops and links</title>
<style>{CSS}</style></head>
<body><main>
<h1>Loops and links</h1>
<p class="sub">Agent C17, 2026-10-24. Star and pocket fixtures in tool-centre space, the same frames as <code>farside_ideas_v2.html</code>. Python prototype on C15's exact geometry, fields and 0.1 mm replay; no Rust. Scripts: <code>phase2/c17/</code>; <code>assert_c17.py</code> checks every number on this page.</p>

<p><b>The question.</b> The harmonic level lines (C15 (e)) look good but cut the same stock many times: one value of the field per loop puts the loops close where the field is steep. The lead's idea: use the field only for its direction, move every point of a loop along its own streamline by its own law gap, and let a point that reaches a wall stop there.</p>
<p><b>The rule, as run.</b></p>
<ul>
<li>Loop 0 is the starter lap (radius R about the entry). Loop k+1 moves every moving point of loop k along the streamline of the field (sub-step at most one 0.1 mm cell).</li>
<li>The step of each point is the C5 law gap min(s, s'(ρ)) of the NEW loop's own radius at that point (the circle through ± one pass step of arc, as in C15). The steps of all points are one implicit system; a damped iteration solves it to 0.0001 mm.</li>
<li>A point stops on a wall or the island where its streamline leaves the region. It also stops when its move cuts no stock (the air stop): this rule is necessary on the pocket.</li>
<li>Where two points move apart by more than the tolerance (0.1 mm) the loop gets new points between them. The march ends when every point has stopped.</li>
</ul>
<p>c·D = {C('inputs.cD', '.6f')} mm (c = {C('inputs.c', '.4f')}; R {C('inputs.R', '.0f')}, s {C('inputs.s', '.0f')}, tolerance {C('inputs.tol', '.1f')} mm, pass step {C('inputs.pass_step', '.1f')} mm). Widths are fractions of D; "interior" leaves out R of arc length at the loop seam and at each change between a moving part and a stopped part. "Mean width" is the stock removed divided by the cut length.</p>
<p class="key">Lines: <span class="sw" style="background:{BLUE}"></span>the new cut of each loop<span class="sw" style="background:{GREY}"></span>the stopped parts on the walls and the island (every later loop runs along them)<span class="sw" style="background:{ORANGE}"></span>the stopped parts in air (the pocket corridor)</p>
{"".join(sections)}
<h2>3. The five questions</h2>
{qa}
<div class="verdict">
<p><b>Verdict on the march by cut length.</b> The streamline march does what the lead predicted for the shape: every loop is one closed curve, the load holds away from the walls, and the stock is cut. But it does not remove the overlap. It moves it: the cut length is {C(s + '.length_vs_e', '.2f')} times (e) on the star and {C(p + '.length_vs_e', '.2f')} times on the pocket, because a closed loop must run along all the wall it has reached. Against (a) it is {C(s + '.length_vs_a', '.1f')} times (star) and {C(p + '.length_vs_a', '.1f')} times (pocket). Sections 4 and 5 replace cut length with engaged length, air length and time.</p>
<p class="pred"><b>Prediction (not measured):</b> a Rust version would need a field solve, a streamline tracer, the implicit step solve, the air stop on a stock model, and loop resampling; about the size of the C15 (a) level rule plus a stock query per point per loop. The wall runs are a property of a closed loop, not of the implementation, so a Rust version would keep them.</p>
</div>
{pl.section(res["sw"], paths, C)}
<div class="verdict">
<p><b>Overall verdict.</b> The march is not a candidate for Rust: with the air counted as keep-down travel it takes the time of (e) ({C('sw.star.h_step.time_vs_e', '.2f')} and {C('sw.pocket.h_step.time_vs_e', '.2f')} times (e) at the cut feed), but its cut parts read over c where a loop leaves a wall. In the family from (a) to (e), only (e) has no retract and holds c in its cut parts, so λ* is (e) on both shapes. The choice between (e) and (a) with its retracts depends on what a retract costs, which this prototype does not measure: (a) is faster unless a retract costs more than {C('sw.star.c15_a.breakeven_s', '.1f')} s (star) or {C('sw.pocket.c15_a.breakeven_s', '.1f')} s (pocket).</p>
</div>
</main></body></html>
"""
    return res, html


def note(res, C=None):
    C = C or Claims(res, html=False)
    if "sw" not in res:
        res["sw"] = pl.derive_sweep(pl.load_sweep()[0])
    s, p = "star.h_step", "pocket.h_step"

    def table(nm):
        lines = ["| Plan | Loops / levels | Open arcs | Self-crossings | Cut mm | Moving mm | Wall + air runs mm "
                 "| Mean width D | Interior max D (over c) | > R from wall, max D | Interior p99 D | Area/mm max D "
                 "| Residue mm² (unreachable) |", "|" + "---|" * 13]
        for k in RUNS:
            q = f"{nm}.{k}"
            lines.append(f"| {TITLES[k]} | {C(q + '.loops', 'd')} | 0 | {C(q + '.self_crossings', 'd')} | {C(q + '.length_mm', '.0f')} "
                         f"| {C(q + '.moving_mm', '.0f')} | {C(q + '.wall_follow_mm', '.0f')} + {C(q + '.air_follow_mm', '.0f')} "
                         f"| {C(q + '.mean_width_frac', '.3f')} | {C(q + '.sw_interior_max', '.4f')} ({C(q + '.sw_interior_over', 'd')}) "
                         f"| {C(q + '.sw_far_max', '.4f')} | {C(q + '.sw_interior_p99', '.4f')} | {C(q + '.apm_interior_max', '.4f')} "
                         f"| {C(q + '.residue_mm2', '.1f')} ({C(q + '.residue_unreachable_mm2', '.1f')}) |")
        for k in ("c15_e", "c15_a"):
            q = f"{nm}.{k}"
            lines.append(f"| {TITLES[k]} | {C(q + '.levels', 'd')} | {C(q + '.open_arcs', 'd')} | — | {C(q + '.length_mm', '.0f')} "
                         f"| — | — | {C(q + '.mean_width_frac', '.3f')} | {C(q + '.sw_interior_max', '.4f')} | — | — "
                         f"| {C(q + '.apm_interior_max', '.4f')} | {C(q + '.residue_interior_mm2', '.1f')} |")
        return "\n".join(lines)

    qa = "\n\n".join(f"### {t}\n\n{body}" for t, body in answers(C))
    return f"""# P2 C17 — the streamline march (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Page: `farside_ideas_v3.html`. Scripts: `phase2/c17/`
(`stream.py`, `run_c17.py`, `page_c17.py`, `assert_c17.py`; `try.py`,
`try2.py`, `look.py` and `look2.py` are the debugging tools). Run
`run_c17.py` (about 70 s), then `page_c17.py`, then `assert_c17.py`.
`page_c17.py` writes this note and the page from `results_c17.json` and
`sweep.json` (`sweep.py` with `plan.py`, §7–§8);
`assert_c17.py` checks every number in both. C15's scripts are imported from
`phase2/c15/` and not edited.

## 1. The question

The operator, on C15 (e): "Harmonic looks amazing, but it overlaps the same
area a lot; I wonder if a post-process could make it more efficient?" A loop
of (e) is a level line of u, so its gap follows 1/|∇u| and the loops crowd
where u is steep. The lead's idea (untested before C17): use ∇u only for the
direction; move each point of a loop along its own streamline by its own law
gap; a point that reaches a wall stops on it.

## 2. Method

- Geometry, fields and replay: C15 (`geom.py`, `shapes.py`, `whole.py`
  `harmonic`, `march.py` `Replay`). c·D = {C('inputs.cD', '.6f')} mm from the code formula
  (c = {C('inputs.c', '.4f')}), R = {C('inputs.R', '.0f')}, s = {C('inputs.s', '.0f')}, tolerance {C('inputs.tol', '.1f')} mm, pass step {C('inputs.pass_step', '.1f')} mm.
- Direction: the unit gradient of the field, bilinear on the 0.1 mm grid;
  the streamline is traced by the midpoint rule in sub-steps of at most one
  cell. A step that leaves the region is bisected onto the wall.
- Step: per point, the C5 law gap min(s, s'(ρ)) with ρ the signed radius of
  the NEW loop at that point (three points at ± one pass step of arc, centre
  inside the loop = cleared side). The steps of all points are one implicit
  system (each radius depends on the neighbours' steps). Two failures came
  first and are part of the result:
  - The explicit rule (radius of the old loop) and a "lower monotonically
    from s" rule both amplify a small wave each loop; in the first trials
    the star loop length grew without bound (those trials are not kept).
  - A damped fixed point (the damping halves whenever the largest residual
    grows) converges to 0.0001 mm; the largest rule violation of a final
    loop is {C(s + '.rule_violation_max_mm', '.4f')} mm (star) and {C(p + '.rule_violation_max_mm', '.4f')} mm (pocket).
- Two step variants: "step" (the lead's rule: the arc length along the
  streamline is the law gap) and "normal" (the distance from the new point
  to the cut is the law gap).
- Stops: a wall (or the island), and the air stop: a point whose move cuts
  no standing stock (the tool disc at its new place holds no stock in the
  replay raster) stops there.
  - The pocket needs it. Behind the island the field has a saddle; the
    fronts from above and below meet along its outgoing line, from the
    island to the far corner. Without a stop there, the points on that line
    move along it and the march does not end (seen in a trial, not kept).
  - A "fronts meet" test on distances was tried first. It needs labels to
    tell its own front from the other one, and it stopped points on lines
    that still had stock beyond them. The air stop replaced it.
- Resampling: a segment longer than the tolerance gets points at the
  tolerance; a moving point nearer than half the tolerance to the last kept
  point is dropped. A segment between two wall stops gets new points only if
  its midpoint lies more than half a cell from the wall (a chord across a
  corner with stock beyond it). No other number enters the march.
- Metrics: the C15 replay cuts every loop in order (each loop includes its
  stopped parts, which cut nothing). "Interior" leaves out R of arc at the
  seam and at each change between moving and stopped parts. "Unreachable"
  residue is farther than R (plus half a cell) from every region node.
- Control: the same march along the gradient of the exact entry distance.

## 3. Tables

Star:

<!-- star:begin -->
{table('star')}
<!-- star:end -->

Pocket:

<!-- pocket:begin -->
{table('pocket')}
<!-- pocket:end -->

## 4. The five questions

{qa}

## 5. Verdict

- The march gives one closed loop per level on both shapes, the load holds
  more than R from the walls, and the stock is cut (only the unreachable
  tips of the star and {C(p + '.residue_mm2', '.2f')} mm² in one pocket corner stay).
- It does not make (e) efficient. Its cut length is {C(s + '.length_vs_e', '.2f')} times (e) on the
  star and {C(p + '.length_vs_e', '.2f')} times on the pocket; the mean width (stock per mm of cut)
  is {C(s + '.mean_width_frac', '.3f')} D against (e) {C('star.c15_e.mean_width_frac', '.3f')} D and (a) {C('star.c15_a.mean_width_frac', '.3f')} D on the star. The overlap of (e)
  is not removed; it moves into the wall runs: {C(s + '.wall_share', '.0%')} of the star length and
  {C(p + '.wall_share', '.0%')} of the pocket length.
- The reason is the closed loop itself, not the field: a closed loop must
  run along every part of the wall that earlier loops have reached. The
  control with straight rays shows the same ({C('star.g_step.length_vs_a', '.1f')} times (a) on the star).
  So no post-process of the loop shape can remove it while the loops stay
  closed.
- The moving part alone is {C(s + '.moving_vs_a', '.2f')} times (a) (star) and {C(p + '.moving_vs_a', '.2f')} times (pocket): the
  loops also take more levels into the star tips ({C(s + '.loops', 'd')} loops against {C('star.c15_a.levels', 'd')}).
- The normal-gap variant is not stable on the pocket (steps to {C('pocket.h_normal.step_max_mm', '.1f')} mm,
  {C('pocket.h_normal.self_crossings', 'd')} self-crossings, {C('pocket.h_normal.sw_interior_max', '.2f')} D).
- **Not a candidate for Rust.** By cut length, every closed-loop plan
  measured so far ((e) and this march with either field, where it ends) cuts
  {C('closed_min', '.1f')} to {C('closed_max', '.1f')} times the length of (a). With the air counted as keep-down
  travel (§7), the march takes {C('sw.star.h_step.time_vs_e', '.2f')} (star) and {C('sw.pocket.h_step.time_vs_e', '.2f')} (pocket) times the time
  of (e) at the cut feed, but its cut parts read over c where a loop leaves a
  wall (§8). So it is still not a candidate.
- In the family from (a) to (e) (§8) only (e) has no retract and holds c in
  its cut parts: λ* is (e) on both shapes. (a) is faster unless a retract
  costs more than {C('sw.star.c15_a.breakeven_s', '.1f')} s (star) or {C('sw.pocket.c15_a.breakeven_s', '.1f')} s (pocket); that cost is not measured.
  The lead decides between (a) with the C14 turns and (e) on that cost.
- What a Rust version would cost (prediction, not measured): a Laplace
  solve on the region, a streamline tracer, the implicit step solve per
  loop (many radius reads over the whole loop per loop), the air
  stop (a stock query per point per loop), and the loop resampling. That is
  more than the C15 (a) level rule, for a plan that cuts {C('closed_min', '.1f')} to {C('closed_max', '.1f')} times as much.

## 6. Limits

- Two shapes, one tool, one stepover. The replay is C15's 0.1 mm raster,
  not the planner.
- The links between loops are not modelled; every loop is cut whole.
- The air stop reads the replay raster; a Rust stock model would read its
  own.

""" + pl.note_section(res["sw"], C)


def main():
    res, html = build()
    with open(PAGE, "w") as f:
        f.write(html)
    with open(NOTE, "w") as f:
        f.write(note(res))
    print("wrote", os.path.abspath(PAGE), len(html), os.path.abspath(NOTE))


if __name__ == "__main__":
    main()
