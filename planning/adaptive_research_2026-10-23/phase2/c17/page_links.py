# C17 additions on page v3 and in the note: cut and link parts, times,
# retracts, and the (a)-(e) family. Reads sweep.json and sweep_paths.npz
# (run sweep.py first). Every number is a claim (page_c17.Claims).
import json
import os

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
import stream as st  # noqa: E402
from page import panel_shape  # noqa: E402  (C15)

CUTC, AIRC, RETC = "#2a78d6", "#eb6834", "#333333"  # validated pair (dataviz validator, light)
PLANS = (("c15_a", "C15 (a): exact-distance wraps"), ("c15_e", "C15 (e): harmonic level lines"),
         ("h_step", "streamline march"), ("g_step", "control: exact-distance streamlines"),
         ("c15_e_trim", "(e), air runs replaced by links"), ("h_step_trim", "march, air runs replaced by links"))


def load_sweep():
    sw = json.load(open(os.path.join(HERE, "sweep.json")))
    d = np.load(os.path.join(HERE, "sweep_paths.npz"))
    paths = {}
    for key in d.files:
        nm, k, i, kind, part = key.split("|")
        paths.setdefault((nm, k), {}).setdefault(int(i), {"kind": kind})[part] = d[key]
    paths = {k: [v[i] for i in sorted(v)] for k, v in paths.items()}
    return sw, paths


def lk(l):
    return f"lam{l:.3f}"


def ck(sw, nm, l):
    """Claim path of the plan at lam l (claim keys hold no dot in a name)."""
    return f"sw.{nm}.by.{sw[nm]['lam_list'].index(l)}"


def derive_sweep(sw):
    import math
    sw["inputs"]["cell"] = st.TOL
    sw["inputs"]["feed_ratio"] = sw["inputs"]["F_cut"] / sw["inputs"]["F_link_dressup"]
    sw["inputs"]["grid_overread"] = 1 / math.cos(math.pi / 8) - 1
    sw["inputs"]["grid_step"] = sw["lams"][1] - sw["lams"][0]
    sw["inputs"]["grid_fine"] = 0.001
    for nm in ("star", "pocket"):
        r = sw[nm]
        e = r["c15_e"]
        for k, m in r.items():
            if not isinstance(m, dict):
                continue
            m["air_share"] = m["air_mm"] / (m["engaged_mm"] + m["air_mm"])
            m["engaged_vs_a"] = m["engaged_mm"] / r["c15_a"]["engaged_mm"]
            m["time_vs_e"] = m["time_cut_feed_min"] / e["time_cut_feed_min"]
            # break-even: the time a retract would have to cost (s) before
            # this plan, with its retracts, is as slow as (e) at the cut feed
            if m["retracts"] > 0:
                m["breakeven_s"] = (e["time_cut_feed_min"] - m["time_cut_feed_min"]) * 60 / m["retracts"]
        lams = r["lam_list"]
        r["by"] = [r[lk(l)] for l in lams]
        below = [l for l in lams if l < 1.0 and r[lk(l)]["retracts"] > 0]
        be = [r[lk(l)]["breakeven_s"] for l in below]
        r["breakeven_min_s"], r["breakeven_max_s"] = min(be), max(be)
        r["breakeven_max_lam"] = below[int(np.argmax(be))]
        r["n_lams"] = len(lams)
        r["n_feasible"] = len(r["feasible_lams"])
        star_ = r["lam_star"]
        r["lam_prev"] = max(l for l in lams if l < star_) if star_ is not None and star_ > 0 else None
        r["retracts_min_below"] = min(r[lk(l)]["retracts"] for l in lams if l < 1.0)
        r["lam_prev_retracts"] = r[lk(r["lam_prev"])]["retracts"] if r["lam_prev"] is not None else 0
        r["star_m"] = r[lk(star_)] if star_ is not None else None
        r["prev_m"] = r[lk(r["lam_prev"])] if r["lam_prev"] is not None else None
        r["zero_m"] = r[lk(0.0)]
        r["c15_e_trim_be_a"] = ((r["c15_e_trim"]["time_cut_feed_min"] - r["c15_a"]["time_cut_feed_min"]) * 60
                                / r["c15_a"]["retracts"])
        r["march_beats"] = "no" if (r["h_step"]["cut_width_max"] > st.C_FRAC + st.TOL / st.D or
                                    r["h_step"]["retracts"] > 0) else "yes"
    return sw


def plan_layers(pieces):
    cut, air, ret = [], [], []
    for p in pieces:
        P = p["P"].astype(float)
        if p["kind"] == "retract":
            ret.append(P)
            continue
        e = p["e"]
        n = len(P)
        if n < 2:
            continue
        seg = e[:-1] & e[1:]
        for flag, dst in ((True, cut), (False, air)):
            m = seg if flag else ~seg
            k = np.nonzero(np.diff(np.concatenate([[0], m.astype(int), [0]])))[0]
            for a, b in zip(k[0::2], k[1::2]):
                dst.append(P[a:b + 1])
    return cut, air, ret


def plan_svg(sh, pieces, label):
    cut, air, ret = plan_layers(pieces)
    # air on top: an air part runs over earlier cut lines, so under them it hides
    svg = panel_shape(sh, [([cut], CUTC, 0.25), ([air], AIRC, 0.3)], label)
    lines = "".join(f'<line x1="{P[0, 0]:.2f}" y1="{P[0, 1]:.2f}" x2="{P[-1, 0]:.2f}" y2="{P[-1, 1]:.2f}" '
                    f'stroke="{RETC}" stroke-width="0.3" stroke-dasharray="1,0.8"/>' for P in ret)
    return svg.replace("</g></svg>", lines + "</g></svg>")


def chart(nm, sw, what):
    """Line chart over lam: what = 'time' (two series) or 'retracts'."""
    r = sw[nm]
    lams = r["lam_list"]
    W, Hh, ml, mr, mt, mb = 520, 230, 52, 14, 26, 36
    if what == "time":
        series = [("time, air at the cut feed", CUTC, [r[lk(l)]["time_cut_feed_min"] for l in lams]),
                  ("time, air at the link-dressup feed", AIRC, [r[lk(l)]["time_link_feed_min"] for l in lams])]
        ylab = "min"
    else:
        series = [("retracts", RETC, [r[lk(l)]["retracts"] for l in lams])]
        ylab = "retracts"
    ymax = max(max(v) for _, _, v in series)
    ystep = 10 ** np.floor(np.log10(ymax / 2))
    ytop = float(np.ceil(ymax / ystep) * ystep)
    X = lambda l: ml + (W - ml - mr) * l
    Y = lambda v: mt + (Hh - mt - mb) * (1 - v / ytop)
    out = [f'<svg viewBox="0 0 {W} {Hh}" class="chart" role="img" aria-label="{nm} {what} against lambda">']
    for t in np.arange(0, ytop + ystep / 2, ystep):
        out.append(f'<line x1="{ml}" x2="{W - mr}" y1="{Y(t):.1f}" y2="{Y(t):.1f}" stroke="#e6e3dc" stroke-width="1"/>'
                   f'<text x="{ml - 6}" y="{Y(t) + 4:.1f}" text-anchor="end" class="tick">{t:g}</text>')
    for t in np.arange(0, 1.01, 0.2):
        out.append(f'<text x="{X(t):.1f}" y="{Hh - mb + 16}" text-anchor="middle" class="tick">{t:.1f}</text>')
    out.append(f'<text x="{(ml + W - mr) / 2}" y="{Hh - 4}" text-anchor="middle" class="tick">λ</text>')
    out.append(f'<text x="{ml}" y="12" class="tick">{ylab}</text>')
    for name, col, v in series:
        pts = " ".join(f"{X(l):.1f},{Y(y):.1f}" for l, y in zip(lams, v))
        out.append(f'<polyline points="{pts}" fill="none" stroke="{col}" stroke-width="2" stroke-linejoin="round"/>')
        for l, y in zip(lams, v):
            feas = r[lk(l)]["feasible"]
            out.append(f'<circle cx="{X(l):.1f}" cy="{Y(y):.1f}" r="{5 if feas else 3}" fill="{col if feas else "#fff"}" '
                       f'stroke="{col}" stroke-width="1.5"><title>λ {l:.3f}: {name} {y:.3f}'
                       f'{" (meets both conditions)" if feas else ""}</title></circle>')
    out.append("</svg>")
    leg = "".join(f'<span class="sw" style="background:{c}"></span>{n}' for n, c, _ in series)
    return f'<div class="chartbox"><p class="key">{nm}: {leg}; a filled marker meets both conditions</p>{"".join(out)}</div>'


def plans_table(nm, C):
    head = ("<tr><th>plan</th><th>engaged mm</th><th>air mm (share)</th><th>keep-down links</th><th>retracts</th>"
            "<th>time, air at cut feed (min)</th><th>time, air at link feed (min)</th>"
            "<th>cut-part interior max D (over c)</th><th>any engaged sample, max D</th></tr>")
    rows = [head]
    for k, title in PLANS:
        p = f"sw.{nm}.{k}"
        rows.append(f"<tr><td>{title}</td><td>{C(p + '.engaged_mm', '.0f')}</td>"
                    f"<td>{C(p + '.air_mm', '.0f')} ({C(p + '.air_share', '.0%')})</td>"
                    f"<td>{C(p + '.links', 'd')}</td><td>{C(p + '.retracts', 'd')}</td>"
                    f"<td>{C(p + '.time_cut_feed_min', '.2f')}</td><td>{C(p + '.time_link_feed_min', '.2f')}</td>"
                    f"<td>{C(p + '.cut_width_max', '.4f')} ({C(p + '.cut_width_over', 'd')})</td>"
                    f"<td>{C(p + '.cut_width_all_max', '.3f')}</td></tr>")
    return '<div class="wide"><table class="all">' + "".join(rows) + "</table></div>"


def lam_table(nm, sw, C):
    head = ("<tr><th>λ</th><th>levels</th><th>curves (open arcs)</th><th>levels with a retract</th><th>retracts</th>"
            "<th>engaged mm</th><th>air mm</th><th>time, cut feed (min)</th><th>time, link feed (min)</th>"
            "<th>cut-part interior max D</th><th>any engaged max D</th><th>both conditions</th></tr>")
    rows = [head]
    for l in sw[nm]["lam_list"]:
        p = ck(sw, nm, l)
        rows.append(f"<tr><td>{C(p + '.lam', '.3f')}</td><td>{C(p + '.levels', 'd')}</td>"
                    f"<td>{C(p + '.curves', 'd')} ({C(p + '.open_arcs', 'd')})</td><td>{C(p + '.levels_with_retract', 'd')}</td>"
                    f"<td>{C(p + '.retracts', 'd')}</td><td>{C(p + '.engaged_mm', '.0f')}</td><td>{C(p + '.air_mm', '.0f')}</td>"
                    f"<td>{C(p + '.time_cut_feed_min', '.2f')}</td><td>{C(p + '.time_link_feed_min', '.2f')}</td>"
                    f"<td>{C(p + '.cut_width_max', '.4f')}</td><td>{C(p + '.cut_width_all_max', '.3f')}</td>"
                    f"<td>{'yes' if sw[nm][lk(l)]['feasible'] else 'no'}</td></tr>")
    return '<div class="wide"><table class="all">' + "".join(rows) + "</table></div>"


def section(sw, paths, C):
    sh = {nm: st.Shape(nm) for nm in ("star", "pocket")}
    i = sw["inputs"]
    cards4 = []
    sent4 = {
        "c15_a": lambda nm: f"Short engaged wraps and almost no air, but {C(f'sw.{nm}.c15_a.retracts', 'd')} links find no keep-down route (dashed): each is a retract.",
        "c15_e": lambda nm: f"No retract; {C(f'sw.{nm}.c15_e.air_share', '.0%')} of the path is air, where a loop runs through cleared stock.",
        "h_step": lambda nm: f"No retract; {C(f'sw.{nm}.h_step.air_share', '.0%')} of the path is air, mostly the wall runs.",
    }
    for nm in ("star", "pocket"):
        for k, title in PLANS[:3]:  # the trimmed plans are drawn in section 6
            p = f"sw.{nm}.{k}"
            cards4.append(f"""<section class="card"><h3>{nm}: {title}</h3><p class="what">{sent4[k](nm)}</p>
{plan_svg(sh[nm], paths[(nm, k)], nm + ' ' + k)}
<table class="mini"><tr><th>engaged / air</th><td>{C(p + '.engaged_mm', '.0f')} / {C(p + '.air_mm', '.0f')} mm</td><th>retracts</th><td>{C(p + '.retracts', 'd')}</td></tr>
<tr><th>time, cut feed</th><td>{C(p + '.time_cut_feed_min', '.2f')} min</td><th>time, link feed</th><td>{C(p + '.time_link_feed_min', '.2f')} min</td></tr></table></section>""")
    s5 = []
    for nm in ("star", "pocket"):
        r = sw[nm]
        cards = []
        trio = [(0.0, "λ = 0 (the (a) end)"), (r["lam_prev"], "the largest λ below λ* that was run"), (r["lam_star"], "λ* (= 1, the (e) end)")]
        for l, title in trio:
            p = ck(sw, nm, l)
            cards.append(f"""<section class="card"><h3>{nm}: {title}, λ = {C(p + '.lam', '.3f')}</h3>
<p class="what">{C(p + '.retracts', 'd')} retracts, {C(p + '.open_arcs', 'd')} open arcs, {C(p + '.time_cut_feed_min', '.2f')} min at the cut feed.</p>
{plan_svg(sh[nm], paths[(nm, lk(l))], nm + ' lam ' + str(l))}</section>""")
        s5.append(f"""<h3>{nm.capitalize()}</h3>
<div class="grid">{"".join(cards)}</div>
<div class="grid">{chart(nm, sw, 'time')}{chart(nm, sw, 'retracts')}</div>
{lam_table(nm, sw, C)}""")
    st_, po = "sw.star", "sw.pocket"
    return f"""<h2>4. Cut parts and link parts (operator addition)</h2>
<p>The operator: the overlap of the harmonic plans runs through cleared stock at depth, so it is keep-down travel, not load. So the cut length is the wrong cost. Each plan here is replayed in order with its links. Every sample is <b>engaged</b> when the stock it removes, read as area per mm over one pass step centred on it, is more than one cell width ({C('sw.inputs.cell', '.1f')} mm: the raster's resolution, one cell per cell of path). Every other sample is <b>air</b>.</p>
<ul>
<li><b>Links.</b> Between two curves the plan takes FreeCAD's rule (R1 §4, ideas only). A link is clear when the tool disc, grown by s·R = {C('sw.inputs.clear', '.2f')} mm, holds no standing stock. The test starts 2·s·R inside each end. A keep-down route may be up to {C('sw.inputs.route_ratio', '.0f')} × the direct length (here, the shortest 8-connected path over the clear nodes, then string pulled to straight segments). A gap shorter than 2·s·R may be cut straight when no step cuts more than {C('sw.inputs.cut_through', '.1f')} × c·D per mm. Otherwise the link is a <b>retract</b>.</li>
<li><b>Order.</b> The levels are cut in order. In a level, the next curve is the nearest one. An open arc starts at its nearer end, and a closed loop starts at its nearest point.</li>
<li><b>Feeds.</b> The cut feed is {C('sw.inputs.F_cut', '.0f')} mm/min (<code>fixture_feeds(Adaptive)</code>, <code>compute/fixture_values.rs:34-35</code>; the op has no feed default). The 2D adaptive feeds its own keep-down <code>Link</code> at that same cut feed (<code>adaptive/path.rs:3163-3168</code>, <code>params.feed_rate</code>). A separate link feed exists only in the link-moves dressup: <code>link_feed_rate</code>, default {C('sw.inputs.F_link_dressup', '.0f')} mm/min (<code>compute/config.rs:1077-1083</code>, <code>1439</code>).</li>
<li><b>Times.</b> The times are for information only: path length over feed, with no acceleration. A retract is counted, not timed.</li>
</ul>
<p class="key">Lines: <span class="sw" style="background:{CUTC}"></span>cut part (engaged)<span class="sw" style="background:{AIRC}"></span>link part (air, keep-down)<span class="sw" style="background:{RETC}"></span>retract (dashed)</p>
<div class="grid">{"".join(cards4)}</div>
<h3>Star</h3>{plans_table('star', C)}
<h3>Pocket</h3>{plans_table('pocket', C)}
<p>With the air at the cut feed, (e) and the march take about the same time: {C(st_ + '.c15_e.time_cut_feed_min', '.2f')} and {C(st_ + '.h_step.time_cut_feed_min', '.2f')} min on the star, {C(po + '.c15_e.time_cut_feed_min', '.2f')} and {C(po + '.h_step.time_cut_feed_min', '.2f')} min on the pocket. (a) takes {C(st_ + '.c15_a.time_cut_feed_min', '.2f')} and {C(po + '.c15_a.time_cut_feed_min', '.2f')} min, plus {C(st_ + '.c15_a.retracts', 'd')} and {C(po + '.c15_a.retracts', 'd')} retracts. (e) engages over {C(st_ + '.c15_e.engaged_vs_a', '.1f')} times the length of (a) on the star, so even its engaged part is a thinner cut. At the link-dressup feed the air is {C('sw.inputs.feed_ratio', '.0f')} times slower than at the cut feed, and (e) takes {C(st_ + '.c15_e.time_link_feed_min', '.2f')} min on the star.</p>

<h2>5. One family from (a) to (e) (operator addition)</h2>
<p><b>The family.</b> u<sub>λ</sub> = (1 − λ)·T/T<sub>max</sub> + λ·u<sub>harm</sub>, where T is the exact entry distance and u<sub>harm</sub> is the harmonic field (C15 (e)). Both fields are 0 on the entry disc, and both are scaled to 1 at their far end. λ = 0 is (a) and λ = 1 is (e).</p>
<ul>
<li><b>Levels.</b> The levels come from the C15 rule: every point of a new level lies within the C5 law gap of its own radius from the cut. Every λ runs until each node lies within one cell of a cut, so every plan clears to the wall. A level line that reaches a wall is cut there into an open arc.</li>
<li><b>The grid.</b> λ steps by {C('sw.inputs.grid_step', '.2f')}. Below the smallest λ that meets both conditions, the grid gets finer, down to {C('sw.inputs.grid_fine', '.3f')}. This gives {C(st_ + '.n_lams', 'd')} values on each shape.</li>
<li><b>λ*.</b> λ* is the λ with the least time at the cut feed that meets (1) no retract and (2) a cut-part interior width of at most c plus one cell. It is computed from the table.</li>
<li><b>Why a linear blend.</b> It is the one-parameter path between the two plans, with no other parameter. A blend that keeps the wall value constant (and so keeps closed loops) is (e) itself. Any λ &lt; 1 gives the wall the value λ + (1 − λ)·T/T<sub>max</sub>, which changes along the wall, so the late levels reach the walls as open arcs.</li>
</ul>
{"".join(s5)}
<div class="verdict">
<p><b>Result.</b> On both shapes only λ = 1 meets both conditions: λ* = {C(st_ + '.lam_star', '.3f')} (star) and {C(po + '.lam_star', '.3f')} (pocket), which is (e). Every λ below 1 has retracts: at least {C(st_ + '.retracts_min_below', 'd')} on the star and {C(po + '.retracts_min_below', 'd')} on the pocket, and {C(st_ + '.lam_prev_retracts', 'd')} and {C(po + '.lam_prev_retracts', 'd')} at λ = {C(st_ + '.lam_prev', '.3f')}. The late levels reach the walls in the lobes and behind the island, and an open arc in a lobe has no keep-down route to the next one. So no λ in the family turns the overlap into only useful links: the plan has either retracts or the air of (e).</p>
<p><b>How to read the time.</b> A plan below λ* is faster than (e) unless a retract costs more than the break-even time: (time of (e) − time of the plan) / retracts. Over the λ &lt; 1 values that break-even time is {C(st_ + '.breakeven_min_s', '.1f')} to {C(st_ + '.breakeven_max_s', '.1f')} s per retract on the star and {C(po + '.breakeven_min_s', '.1f')} to {C(po + '.breakeven_max_s', '.1f')} s on the pocket. For (a) it is {C(st_ + '.c15_a.breakeven_s', '.1f')} s (star) and {C(po + '.c15_a.breakeven_s', '.1f')} s (pocket). These are at the cut feed with no acceleration, and the retract cost is not measured here.</p>
<p><b>The streamline march against the family.</b> It has no retract, but its cut parts read {C(st_ + '.h_step.cut_width_max', '.4f')} D (star) and {C(po + '.h_step.cut_width_max', '.4f')} D (pocket) where a loop leaves a wall, which is above c. So it does not meet condition (2), and it does not beat λ* (march beats the family: star {C(st_ + '.march_beats', 's')}, pocket {C(po + '.march_beats', 's')}).</p>
</div>
"""


def note_section(sw, C):
    st_, po = "sw.star", "sw.pocket"

    def plans_md(nm):
        lines = ["| Plan | Engaged mm | Air mm (share) | Keep-down links | Retracts | Time, air at cut feed min "
                 "| Time, air at link feed min | Cut-part interior max D (over c) | Any engaged max D |",
                 "|" + "---|" * 9]
        for k, title in PLANS:
            p = f"sw.{nm}.{k}"
            lines.append(f"| {title} | {C(p + '.engaged_mm', '.0f')} | {C(p + '.air_mm', '.0f')} ({C(p + '.air_share', '.0%')}) "
                         f"| {C(p + '.links', 'd')} | {C(p + '.retracts', 'd')} | {C(p + '.time_cut_feed_min', '.2f')} "
                         f"| {C(p + '.time_link_feed_min', '.2f')} | {C(p + '.cut_width_max', '.4f')} ({C(p + '.cut_width_over', 'd')}) "
                         f"| {C(p + '.cut_width_all_max', '.3f')} |")
        return "\n".join(lines)

    def lam_md(nm):
        lines = ["| λ | Levels | Curves (open arcs) | Levels with a retract | Retracts | Engaged mm | Air mm "
                 "| Time cut feed min | Time link feed min | Cut-part interior max D | Any engaged max D | Both conditions |",
                 "|" + "---|" * 12]
        for l in sw[nm]["lam_list"]:
            p = ck(sw, nm, l)
            lines.append(f"| {C(p + '.lam', '.3f')} | {C(p + '.levels', 'd')} | {C(p + '.curves', 'd')} ({C(p + '.open_arcs', 'd')}) "
                         f"| {C(p + '.levels_with_retract', 'd')} | {C(p + '.retracts', 'd')} | {C(p + '.engaged_mm', '.0f')} "
                         f"| {C(p + '.air_mm', '.0f')} | {C(p + '.time_cut_feed_min', '.2f')} | {C(p + '.time_link_feed_min', '.2f')} "
                         f"| {C(p + '.cut_width_max', '.4f')} | {C(p + '.cut_width_all_max', '.3f')} "
                         f"| {'yes' if sw[nm][lk(l)]['feasible'] else 'no'} |")
        return "\n".join(lines)

    return f"""## 7. Addition 1 (operator): cut parts and link parts

The operator: the overlap of the harmonic plans is keep-down travel through
cleared stock, not load, so cut length is the wrong cost. Script:
`sweep.py` with `plan.py` (a few minutes on a process pool).

- Engaged / air. Each plan is replayed in order, links included. A sample
  is engaged when its area per mm over one pass step (centred) is more than
  one cell width, {C('sw.inputs.cell', '.1f')} mm (the raster's resolution); else it is air.
- Links: FreeCAD's rule, R1 §4 (ideas only). A link is clear when the tool
  disc grown by s·R = {C('sw.inputs.clear', '.2f')} mm holds no standing stock, tested from
  2·s·R inside each end. A keep-down route may be up to {C('sw.inputs.route_ratio', '.0f')} × the direct
  length (the shortest 8-connected path over clear nodes, then string
  pulled: each kept point goes straight to the farthest later point it sees
  over clear nodes; the grid path alone reads up to {C('sw.inputs.grid_overread', '.1%')} long). A gap
  under 2·s·R may be cut straight at most
  {C('sw.inputs.cut_through', '.1f')} × c·D per mm. Otherwise the link is a retract.
- Order: levels in order; in a level the nearest curve next. An open arc
  starts at its nearer end and a closed loop at its nearest point. The cut
  direction (climb or conventional) is not kept.
- Feeds:
  - Cut feed {C('sw.inputs.F_cut', '.0f')} mm/min: `fixture_feeds(Adaptive)`,
    `crates/rs_cam_core/src/compute/fixture_values.rs:34-35` (S11: the op
    has no feed default).
  - The 2D adaptive emits its keep-down `Link` at `params.feed_rate`, the
    cut feed: `crates/rs_cam_core/src/adaptive/path.rs:3163-3168`.
  - A separate link feed exists only in the link-moves dressup:
    `link_feed_rate`, default {C('sw.inputs.F_link_dressup', '.0f')} mm/min, `compute/config.rs:1077-1083`
    and `1439`.
  - Times are information only: length over feed, no acceleration, and
    retracts are counted, not timed.

Star:

<!-- plans-star:begin -->
{plans_md('star')}
<!-- plans-star:end -->

Pocket:

<!-- plans-pocket:begin -->
{plans_md('pocket')}
<!-- plans-pocket:end -->

- With air at the cut feed, (e) and the march take about the same time:
  {C(st_ + '.c15_e.time_cut_feed_min', '.2f')} and {C(st_ + '.h_step.time_cut_feed_min', '.2f')} min on the star, {C(po + '.c15_e.time_cut_feed_min', '.2f')} and {C(po + '.h_step.time_cut_feed_min', '.2f')} min on the pocket.
- (a) takes {C(st_ + '.c15_a.time_cut_feed_min', '.2f')} and {C(po + '.c15_a.time_cut_feed_min', '.2f')} min, but needs {C(st_ + '.c15_a.retracts', 'd')} and {C(po + '.c15_a.retracts', 'd')} retracts.
- (e) engages over {C(st_ + '.c15_e.engaged_vs_a', '.1f')} times the length of (a) on the star: its
  engaged part is also a thinner cut.
- The cut-part interior of (e) holds c on both shapes. Its tip samples
  (up to {C(st_ + '.c15_e.cut_width_all_max', '.3f')} D on the star) lie within R of where an engaged run begins.

## 8. Addition 2 (operator): one family from (a) to (e)

- Family: u_λ = (1 − λ)·T/T_max + λ·u_harm. T is the exact entry distance,
  u_harm the harmonic field; both are 0 on the entry disc and 1 at their far
  end. Outside the region T keeps its closed form and u_harm is 1, so a level
  line that reaches a wall is cut there (a point within one cell outside is
  the contour's own error and is kept). λ = 0 is (a), λ = 1 is (e).
- Why this blend: it is the one-parameter path between the two plans with no
  other parameter. A blend that keeps the wall value constant (closed loops)
  is (e) itself; for λ < 1 the wall value λ + (1 − λ)·T/T_max varies, so the
  late levels reach the walls as open arcs.
- Levels: the C15 rule, run until every node lies within one cell of a cut
  (the same stop for every λ; it differs from C15's (a), which stopped at s).
- Grid: steps of {C('sw.inputs.grid_step', '.2f')}, then finer below the smallest λ that meets both
  conditions, down to {C('sw.inputs.grid_fine', '.3f')} ({C(st_ + '.n_lams', 'd')} values per shape).
- λ* = the least time at the cut feed among the λ with (1) no retract and
  (2) cut-part interior width at most c plus one cell. It is computed.

Star:

<!-- lam-star:begin -->
{lam_md('star')}
<!-- lam-star:end -->

Pocket:

<!-- lam-pocket:begin -->
{lam_md('pocket')}
<!-- lam-pocket:end -->

Result:

- Only λ = 1 meets both conditions on both shapes: λ* = {C(st_ + '.lam_star', '.3f')} (star) and
  {C(po + '.lam_star', '.3f')} (pocket), which is (e).
- Every λ below 1 has retracts: at least {C(st_ + '.retracts_min_below', 'd')} (star) and {C(po + '.retracts_min_below', 'd')} (pocket). At
  λ = {C(st_ + '.lam_prev', '.3f')} there are still {C(st_ + '.lam_prev_retracts', 'd')} and {C(po + '.lam_prev_retracts', 'd')}. The late levels reach the walls in the
  lobes and behind the island, and an open arc in a lobe has no keep-down
  route to the next one.
- So the family cannot be set so that the overlap is only useful links:
  the plan has either retracts or the air of (e).
- Break-even: a plan below λ* is faster than (e) unless a retract costs more
  than (time of (e) − time of the plan) / retracts. Over λ < 1 that is
  {C(st_ + '.breakeven_min_s', '.1f')} to {C(st_ + '.breakeven_max_s', '.1f')} s per retract (star) and {C(po + '.breakeven_min_s', '.1f')} to {C(po + '.breakeven_max_s', '.1f')} s (pocket); for (a)
  {C(st_ + '.c15_a.breakeven_s', '.1f')} s and {C(po + '.c15_a.breakeven_s', '.1f')} s. These are at the cut feed with no acceleration. The cost of a
  retract (lift, travel, re-entry at the plunge feed) is not measured here,
  so this note does not choose between (a) and (e) on time.
- The streamline march has no retract. But its cut parts read
  {C(st_ + '.h_step.cut_width_max', '.4f')} D (star) and {C(po + '.h_step.cut_width_max', '.4f')} D (pocket) where a loop leaves a wall, which is over c. So it
  fails condition (2) and does not beat λ* (star {C(st_ + '.march_beats', 's')}, pocket {C(po + '.march_beats', 's')}).
"""


def trim_section(sw, paths, C):
    sh = {nm: st.Shape(nm) for nm in ("star", "pocket")}
    cards = []
    for nm in ("star", "pocket"):
        for k, base in (("c15_e_trim", "c15_e"), ("h_step_trim", "h_step")):
            p, b = f"sw.{nm}.{k}", f"sw.{nm}.{base}"
            cards.append(f"""<section class="card"><h3>{nm}: {dict(PLANS)[k]}</h3>
<p class="what">{C(p + '.trims', 'd')} of {C(p + '.air_runs', 'd')} air runs get a shorter keep-down link; the path is {C(p + '.trimmed_mm', '.0f')} mm shorter and takes {C(p + '.time_cut_feed_min', '.2f')} min against {C(b + '.time_cut_feed_min', '.2f')} min.</p>
{plan_svg(sh[nm], paths[(nm, k)], nm + ' ' + k)}
<table class="mini"><tr><th>engaged / air</th><td>{C(p + '.engaged_mm', '.0f')} / {C(p + '.air_mm', '.0f')} mm</td><th>retracts</th><td>{C(p + '.retracts', 'd')}</td></tr>
<tr><th>time, cut feed</th><td>{C(p + '.time_cut_feed_min', '.2f')} min</td><th>time, link feed</th><td>{C(p + '.time_link_feed_min', '.2f')} min</td></tr>
<tr><th>cut-part interior max</th><td>{C(p + '.cut_width_max', '.4f')} D</td><th>residue</th><td>{C(p + '.residue_mm2', '.1f')} mm²</td></tr></table></section>""")
    st_, po = "sw.star", "sw.pocket"
    return f"""<h2>6. Keep the loop shape, replace the air runs by links (lead's variant)</h2>
<p>Each loop of (e), and of the march, keeps its shape. An <b>air run</b> is a part of the loop that cuts nothing, because an earlier loop already cut it. Each air run is replaced by the shortest keep-down route between its two ends (the rule of section 4) when that route is shorter. This is what the C14 turns do for (a). The loops are cut in their order, and the replay classifies each run against the stock as it is at that moment.</p>
<div class="grid">{"".join(cards)}</div>
<div class="verdict">
<p><b>Result.</b></p>
<ul>
<li><b>Star.</b> The air runs of (e) lie along the straight walls of the lobes, so no route is shorter. Only {C(st_ + '.c15_e_trim.trimmed_mm', '.1f')} mm is saved, and (e) stays at {C(st_ + '.c15_e_trim.time_cut_feed_min', '.2f')} min.</li>
<li><b>Pocket.</b> The links cross the cleared corners and the area round the island. They save {C(po + '.c15_e_trim.trimmed_mm', '.0f')} mm, and (e) takes {C(po + '.c15_e_trim.time_cut_feed_min', '.2f')} min instead of {C(po + '.c15_e.time_cut_feed_min', '.2f')} min.</li>
<li><b>The march.</b> Its wall and corridor runs shorten more: {C(st_ + '.h_step_trim.trimmed_mm', '.0f')} mm (star) and {C(po + '.h_step_trim.trimmed_mm', '.0f')} mm (pocket), to {C(st_ + '.h_step_trim.time_cut_feed_min', '.2f')} and {C(po + '.h_step_trim.time_cut_feed_min', '.2f')} min. Its cut parts still read {C(st_ + '.h_step_trim.cut_width_max', '.4f')} D and {C(po + '.h_step_trim.cut_width_max', '.4f')} D, which is over c.</li>
<li><b>Retracts.</b> No retract is added: {C(st_ + '.c15_e_trim.retracts', 'd')} and {C(po + '.c15_e_trim.retracts', 'd')} for the trimmed (e).</li>
<li><b>Against (a).</b> (a) takes {C(st_ + '.c15_a.time_cut_feed_min', '.2f')} and {C(po + '.c15_a.time_cut_feed_min', '.2f')} min, with {C(st_ + '.c15_a.retracts', 'd')} and {C(po + '.c15_a.retracts', 'd')} retracts. The trimmed (e) is faster only if a retract costs more than {C(st_ + '.c15_e_trim_be_a', '.1f')} s (star) or {C(po + '.c15_e_trim_be_a', '.1f')} s (pocket).</li>
</ul>
<p>The rest of the extra time of (e) is engaged path, not air: (e) engages {C(st_ + '.c15_e.engaged_vs_a', '.1f')} and {C(po + '.c15_e.engaged_vs_a', '.1f')} times the length of (a), because its thin loops cut little stock per mm. No change to the links can remove that.</p>
</div>
"""


def trim_note(sw, C):
    st_, po = "sw.star", "sw.pocket"
    return f"""## 9. Lead's variant: keep the loop shape, replace the air runs by links

- Each loop of (e) (and of the march) keeps its shape. An air run (it cuts
  nothing, because an earlier loop already cut it) is replaced by the
  shortest keep-down route between its ends (§7 rule) when that route is
  shorter. This is what the C14 turns do for (a). The run is classified
  against the stock at the time it is cut.
- Star (e): {C(st_ + '.c15_e_trim.trims', 'd')} of {C(st_ + '.c15_e_trim.air_runs', 'd')} air runs get a shorter link, which saves only {C(st_ + '.c15_e_trim.trimmed_mm', '.1f')} mm. The air runs lie
  along the straight lobe walls, where the run is already the shortest
  route. The time is {C(st_ + '.c15_e_trim.time_cut_feed_min', '.2f')} min against {C(st_ + '.c15_e.time_cut_feed_min', '.2f')} min.
- Pocket (e): {C(po + '.c15_e_trim.trims', 'd')} of {C(po + '.c15_e_trim.air_runs', 'd')} runs, which saves {C(po + '.c15_e_trim.trimmed_mm', '.0f')} mm. The time is {C(po + '.c15_e_trim.time_cut_feed_min', '.2f')} min against {C(po + '.c15_e.time_cut_feed_min', '.2f')} min.
- March: it saves {C(st_ + '.h_step_trim.trimmed_mm', '.0f')} mm (star) and {C(po + '.h_step_trim.trimmed_mm', '.0f')} mm (pocket), for {C(st_ + '.h_step_trim.time_cut_feed_min', '.2f')} and {C(po + '.h_step_trim.time_cut_feed_min', '.2f')} min.
  Its cut parts still read {C(st_ + '.h_step_trim.cut_width_max', '.4f')} D and {C(po + '.h_step_trim.cut_width_max', '.4f')} D, which is over c.
- No retract is added ({C(st_ + '.c15_e_trim.retracts', 'd')} and {C(po + '.c15_e_trim.retracts', 'd')}). Against (a) ({C(st_ + '.c15_a.retracts', 'd')} and {C(po + '.c15_a.retracts', 'd')} retracts), the
  trimmed (e) is faster only if a retract costs more than {C(st_ + '.c15_e_trim_be_a', '.1f')} s (star) or
  {C(po + '.c15_e_trim_be_a', '.1f')} s (pocket).
- The rest of the extra time of (e) is engaged path: (e) engages {C(st_ + '.c15_e.engaged_vs_a', '.1f')} and {C(po + '.c15_e.engaged_vs_a', '.1f')}
  times the length of (a). No change to the links removes that.
"""
