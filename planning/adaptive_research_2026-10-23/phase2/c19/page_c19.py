# C19: write farside_ideas_v5.html and P2_C19_march_arms.md from
# results_c19.json, paths_c19.npz, C17's sweep.json / sweep_paths.npz and the
# D12 harness output. Every quoted number goes through q(); the list goes to
# quotes_c19.json, and assert_c19.py recomputes each one.
# Run: python3 -I page_c19.py
import html
import json
import math
import os
import re
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
PH2 = os.path.dirname(HERE)
ROOT = os.path.dirname(PH2)
sys.path.insert(0, HERE)
import clip as cl  # noqa: E402
from clip import R, S, D, CD, C_FRAC, TOL, CELL, TURN_BOUND, QUANTUM  # noqa: E402

PAGE = os.path.join(ROOT, "farside_ideas_v5.html")
NOTE = os.path.join(ROOT, "P2_C19_march_arms.md")

QUOTES = {}


def q(key, value, fmt):
    """Format a quoted number and record it for assert_c19.py."""
    if isinstance(value, str):
        s = value
    elif fmt == "%":
        s = f"{100 * value:.0f}%"
    else:
        s = format(value, fmt)
    QUOTES[key] = dict(text=s, fmt=fmt, value=value if not isinstance(value, (np.floating, np.integer)) else value.item())
    return s


def load():
    res = json.load(open(os.path.join(HERE, "results_c19.json")))
    sw = json.load(open(os.path.join(PH2, "c17", "sweep.json")))
    return res, sw


def d12():
    """(a) + one-way in Rust: D12 harness, column geodesic_local_oneway."""
    txt = open(os.path.join(PH2, "d12_harness.txt")).read().split("\n")
    out = {}
    for nm in ("star", "pocket"):
        k = txt.index(f"{nm} / geodesic_local_oneway")
        blk = []
        for ln in txt[k + 1:]:
            if re.match(r"^(star|pocket) / ", ln):
                break
            blk.append(ln)
        b = "\n".join(blk)
        m = {}
        m["cut_mm"] = float(re.search(r"^cut length: ([\d.]+) mm", b, re.M).group(1))
        m["keepdown_mm"] = float(re.search(r"^keep-down links: \d+ \(([\d.]+) mm\)", b, re.M).group(1))
        m["entries"] = int(re.search(r"^entries \(plunges\): (\d+)", b, re.M).group(1))
        e = re.search(r"wrap end \(join\) (\d+),.*residue mop (\d+)", b)
        m["join_entries"], m["mop_entries"] = int(e.group(1)), int(e.group(2))
        a = re.search(r"clearing turns (\d+), over the bound [\d.]+ mm (\d+)", b)
        m["turns"], m["over_bound"] = int(a.group(1)), int(a.group(2))
        m["return_mm"] = float(re.search(r"return links \d+ \(([\d.]+) mm\)", b).group(1))
        m["factor_med"] = float(re.search(r"turn feed factor: min [\d.]+, quartiles [\d.]+ / ([\d.]+)", b).group(1))
        m["no_turn_max"] = float(re.search(r"steps on no turn \(emitted\): max ([\d.]+)", b).group(1))
        m["time_cut_feed_min"] = (m["cut_mm"] + m["keepdown_mm"]) / 1500.0
        m["time_link_feed_min"] = m["cut_mm"] / 1500.0 + m["keepdown_mm"] / 500.0
        out[nm] = m
    return out


def paths(nm, src, mode):
    d = np.load(os.path.join(HERE, "paths_c19.npz"))
    moves = {}
    for k in d.files:
        if k.split("|")[2] == "piece":
            continue
        n_, s_, m_, i, kind, part = k.split("|")
        if (n_, s_, m_) == (nm, src, mode):
            moves.setdefault(int(i), dict(kind=kind))[part] = d[k]
    return [moves[i] for i in sorted(moves)]


def c17_paths(nm):
    d = np.load(os.path.join(PH2, "c17", "sweep_paths.npz"))
    moves = {}
    for k in d.files:
        n_, key, i, kind, part = k.split("|")
        if (n_, key) == (nm, "h_step"):
            moves.setdefault(int(i), dict(kind=kind))[part] = d[k]
    return [moves[i] for i in sorted(moves)]


# ── drawing ────────────────────────────────────────────────────────────

FRAME = {"star": (3.0, 97.0, 3.0, 93.0), "pocket": (3.0, 77.0, 3.0, 57.0)}
COL = dict(cut="#1f5fbf", air="#b9b6ae", keep="#e08a00", turn="#d0201a", ring="#111111", wall="#3a3a3a")


def svg(nm, layers, width=430):
    x0, x1, y0, y1 = FRAME[nm]
    sc = width / (x1 - x0)
    hgt = (y1 - y0) * sc

    def pts(P):
        return " ".join(f"{(x - x0) * sc:.1f},{(y1 - y) * sc:.1f}" for x, y in P)

    out = [f'<svg class="pane" viewBox="0 0 {width} {hgt:.0f}" role="img" aria-label="{html.escape(nm)} tool path">']
    if nm == "star":
        out.append(f'<polygon points="{pts(cl.st.STAR)}" fill="#f4f2ec" stroke="{COL["wall"]}" stroke-width="1"/>')
    else:
        from geom import RECT, RX, ISL_C, ISL_R
        out.append(f'<rect x="{(RECT[0] - x0) * sc:.1f}" y="{(y1 - RECT[3]) * sc:.1f}" width="{(RECT[1] - RECT[0]) * sc:.1f}" '
                   f'height="{(RECT[3] - RECT[2]) * sc:.1f}" rx="{RX * sc:.1f}" fill="#f4f2ec" stroke="{COL["wall"]}" stroke-width="1"/>')
        out.append(f'<circle cx="{(ISL_C[0] - x0) * sc:.1f}" cy="{(y1 - ISL_C[1]) * sc:.1f}" r="{ISL_R * sc:.1f}" fill="#ffffff" '
                   f'stroke="{COL["wall"]}" stroke-width="1"/>')
    for color, wdt, P in layers.get("lines", []):
        if len(P) >= 2:
            out.append(f'<polyline points="{pts(P)}" fill="none" stroke="{color}" stroke-width="{wdt}" '
                       f'stroke-linejoin="round" stroke-linecap="round"/>')
    for x, y in layers.get("rings", []):
        out.append(f'<circle cx="{(x - x0) * sc:.1f}" cy="{(y1 - y) * sc:.1f}" r="4.2" fill="none" stroke="{COL["ring"]}" stroke-width="1.3"/>')
    out.append("</svg>")
    return "\n".join(out)


def thin(P, step=3):
    if len(P) <= 2:
        return P
    idx = np.unique(np.r_[np.arange(0, len(P), step), len(P) - 1])
    return P[idx]


def runs_of(P, e):
    """Split a polyline into (engaged, part) runs; the joint point is shared."""
    out = []
    k = 0
    n = len(P)
    while k < n - 1:
        j = k
        while j + 1 < n - 1 and e[j + 1] == e[k]:
            j += 1
        out.append((bool(e[k]), P[k:j + 2]))
        k = j + 1
    return out


def layers_full(nm):
    L = []
    for m in c17_paths(nm):
        if m["kind"] == "retract":
            continue
        for eng, P in runs_of(m["P"].astype(float), m["e"]):
            L.append((COL["cut"], 0.9, P) if eng else (COL["air"], 0.6, P))
    # air first, cuts on top
    L.sort(key=lambda t: t[0] == COL["cut"])
    return dict(lines=L)


def layers_plan(nm, src, mode):
    L, rings = [], []
    for m in paths(nm, src, mode):
        k = m["kind"]
        P = m["P"].astype(float)
        if k == "retract":
            rings.append(P[-1])
        elif k == "cut":
            L.append((COL["cut"], 0.9, thin(P)))
        elif k in ("link", "return"):
            L.append((COL["keep"], 0.6, thin(P)))
        elif k == "turn":
            L.append((COL["turn"], 1.8, P))
    order = {COL["keep"]: 0, COL["cut"]: 1, COL["turn"]: 2}
    L.sort(key=lambda t: order[t[0]])
    return dict(lines=L, rings=rings)


# ── text ───────────────────────────────────────────────────────────────

MODES = [
    ("oneway", "march, one-way, C14 turns (the brief as written)"),
    ("oneway_head", "march, one-way, head as the turn, C14 bound"),
    ("oneway_head_slanted", "march, one-way, head as the turn, C16 slanted bound"),
    ("zigzag", "march, zigzag, C14 turns"),
]
A_MODES = [
    ("oneway", "C15 (a), one-way, C14 turns"),
    ("oneway_head_slanted", "C15 (a), one-way, C16 slanted bound"),
    ("zigzag", "C15 (a), zigzag, C14 turns"),
]
E_MODES = [
    ("oneway", "C15 (e) harmonic, one-way, C14 turns"),
    ("oneway_head_slanted", "C15 (e) harmonic, one-way, head as the turn, C16 slanted bound"),
    ("zigzag", "C15 (e) harmonic, zigzag, C14 turns"),
]
CARD = "oneway_head_slanted"  # the plan the cards draw (march, (e) and (a))


def main():
    res, sw = load()
    rs = d12()
    inp = res["inputs"]
    H = []  # html body parts
    N = []  # note lines

    cD = q("const|cD", CD, ".6f")
    cc = q("const|c", C_FRAC, ".4f")
    tb = q("const|turn_bound", TURN_BOUND, ".3f")
    qc = q("const|c_plus_cell", C_FRAC + QUANTUM, ".4f")

    def P(nm, src, mode, key, fmt):
        return q(f"{nm}|{src}|{mode}|{key}", res[nm][src][mode][key], fmt)

    def BP(nm, src, mode, place, key, fmt):
        return q(f"{nm}|{src}|{mode}|place.{place}.{key}", res[nm][src][mode]["by_place"]["acc"][place][key], fmt)

    def BX(nm, src, mode, key, fmt):
        return q(f"{nm}|{src}|{mode}|bp.{key}", res[nm][src][mode]["by_place"][key], fmt)

    def C17(nm, key, fmt):
        return q(f"{nm}|c17|{key}", sw[nm]["h_step"][key], fmt)

    def C17E(nm, key, fmt):
        return q(f"{nm}|c17e|{key}", sw[nm]["c15_e"][key], fmt)

    def RS(nm, key, fmt):
        return q(f"{nm}|d12|{key}", rs[nm][key], fmt)

    def SP(nm, src, key, fmt):
        return q(f"{nm}|{src}|split|{key}", res[nm][src]["split"][key], fmt)

    # ── cards ──
    cards = {}
    for nm in ("star", "pocket"):
        m = res[nm]["march"][CARD]
        a = res[nm]["a"][CARD]
        full_air_share = sw[nm]["h_step"]["air_mm"] / (sw[nm]["h_step"]["air_mm"] + sw[nm]["h_step"]["engaged_mm"])
        s_full = (f"The march cuts every loop whole, so the tool runs again along all the wall that earlier loops cut: "
                  f"{q(f'{nm}|c17|air_share', full_air_share, '%')} of its path cuts nothing.")
        s_clip = (f"The same loops cut only where they remove stock, one {'arm' if nm == 'star' else 'lobe'} at a time, with a "
                  f"keep-down return and a short turn between pieces: the shape stays, and the time falls from "
                  f"{C17(nm, 'time_cut_feed_min', '.2f')} to {P(nm, 'march', CARD, 'time_cut_feed_min', '.2f')} min.")
        s_a = (f"The exact-distance arcs of C15 (a) with the same links: {P(nm, 'a', CARD, 'engaged_mm', '.0f')} mm of cut "
               f"against {P(nm, 'march', CARD, 'engaged_mm', '.0f')} mm for the march, because the march makes more and "
               f"thinner passes{' into the tips' if nm == 'star' else ''}.")
        s_e = (f"C15's harmonic level lines with the same clip, split and links: the time falls from "
               f"{C17E(nm, 'time_cut_feed_min', '.2f')} to {P(nm, 'e', CARD, 'time_cut_feed_min', '.2f')} min, but the cut is "
               f"thin (median swept width {P(nm, 'e', CARD, 'eng_w_med', '.3f')} D), so it engages "
               f"{q(f'{nm}|ratio|e_eng_vs_a', res[nm]['e'][CARD]['engaged_mm'] / res[nm]['a'][CARD]['engaged_mm'], '.2f')} "
               f"times the length of (a).")
        cards[nm] = [
            ("Full march (C17)", s_full, svg(nm, layers_full(nm)), [
                ("Engaged / air mm", f"{C17(nm, 'engaged_mm', '.0f')} / {C17(nm, 'air_mm', '.0f')}"),
                ("Retracts (FreeCAD rule)", C17(nm, "retracts", "d")),
                ("Time, air at cut feed", C17(nm, "time_cut_feed_min", ".2f") + " min"),
            ]),
            ("March, clipped and split, one-way", s_clip, svg(nm, layers_plan(nm, "march", CARD)), [
                ("Engaged / keep-down mm", f"{P(nm, 'march', CARD, 'engaged_mm', '.0f')} / {P(nm, 'march', CARD, 'keepdown_mm', '.0f')}"),
                ("Plunges / turns", f"{P(nm, 'march', CARD, 'plunges', 'd')} / {P(nm, 'march', CARD, 'turns', 'd')}"),
                ("Time, air at cut feed", P(nm, "march", CARD, "time_cut_feed_min", ".2f") + " min"),
            ]),
            ("Harmonic (e), clipped and split, one-way", s_e, svg(nm, layers_plan(nm, "e", CARD)), [
                ("Engaged / keep-down mm", f"{P(nm, 'e', CARD, 'engaged_mm', '.0f')} / {P(nm, 'e', CARD, 'keepdown_mm', '.0f')}"),
                ("Plunges / turns", f"{P(nm, 'e', CARD, 'plunges', 'd')} / {P(nm, 'e', CARD, 'turns', 'd')}"),
                ("Time, air at cut feed", P(nm, "e", CARD, "time_cut_feed_min", ".2f") + " min"),
            ]),
            ("C15 (a), same linking", s_a, svg(nm, layers_plan(nm, "a", CARD)), [
                ("Engaged / keep-down mm", f"{P(nm, 'a', CARD, 'engaged_mm', '.0f')} / {P(nm, 'a', CARD, 'keepdown_mm', '.0f')}"),
                ("Plunges / turns", f"{P(nm, 'a', CARD, 'plunges', 'd')} / {P(nm, 'a', CARD, 'turns', 'd')}"),
                ("Time, air at cut feed", P(nm, "a", CARD, "time_cut_feed_min", ".2f") + " min"),
            ]),
        ]

    # ── tables ──
    def row_plan(nm, src, mode, title):
        x = res[nm][src][mode]
        b = x["by_place"]["acc"]
        k = f"{nm}|{src}|{mode}"
        return [title,
                P(nm, src, mode, "entries", "d"),
                f"{P(nm, src, mode, 'plunges', 'd')} = {P(nm, src, mode, 'plunge_seam', 'd')} / {P(nm, src, mode, 'plunge_arc', 'd')} / "
                f"{P(nm, src, mode, 'plunge_lobe', 'd')} / {P(nm, src, mode, 'plunge_jump', 'd')}",
                P(nm, src, mode, "refusals", "d"),
                f"{P(nm, src, mode, 'turns', 'd')} ({P(nm, src, mode, 'turn_over', 'd')})",
                q(f"{k}|turn_factor_med", x["turn_factor_q"][2], ".2f"),
                P(nm, src, mode, "keepdown_mm", ".0f"),
                P(nm, src, mode, "engaged_mm", ".0f"),
                P(nm, src, mode, "air_mm", ".0f"),
                P(nm, src, mode, "time_cut_feed_min", ".2f"),
                P(nm, src, mode, "time_link_feed_min", ".2f"),
                BP(nm, src, mode, "head", "max", ".3f"),
                BP(nm, src, mode, "departure", "max", ".3f"),
                f"{BP(nm, src, mode, 'interior', 'max', '.3f')} ({BP(nm, src, mode, 'interior', 'over', 'd')})",
                f"{P(nm, src, mode, 'residue_mm2', '.1f')} ({P(nm, src, mode, 'residue_unreachable_mm2', '.1f')})",
                P(nm, src, mode, "eng_w_med", ".3f"),
                f"{P(nm, src, mode, 'piece_w_med', '.3f')} / {P(nm, src, mode, 'piece_w_min', '.3f')}",
                P(nm, src, mode, "mean_width", ".3f")]

    HEAD = ["Plan", "Entries", "Plunges = seam / turn over bound / lobe / in-lobe", "Refusals", "Turns (over c)",
            "Turn feed factor, median", "Keep-down mm", "Engaged mm", "Air mm", "Time, air at cut feed, min",
            "Time, air at link feed, min", "Head max D", "Departure max D", "Interior max D (over)", "Coverage left mm² (unreachable)",
            "Swept width of engaged samples, median D", "Piece mean width, median / min D", "Mean width D"]
    tables = {}
    for nm in ("star", "pocket"):
        rows = []
        h = sw[nm]["h_step"]
        rows.append(["C17 full march (sweep.json)", "—", f"{C17(nm, 'retracts', 'd')} (FreeCAD rule)", "—", "—", "—",
                     C17(nm, "link_mm", ".0f"), C17(nm, "engaged_mm", ".0f"), C17(nm, "air_mm", ".0f"),
                     C17(nm, "time_cut_feed_min", ".2f"), C17(nm, "time_link_feed_min", ".2f"),
                     "—", "—", f"{C17(nm, 'cut_width_max', '.4f')} ({C17(nm, 'cut_width_over', 'd')})",
                     f"{C17(nm, 'residue_mm2', '.1f')} ({C17(nm, 'residue_unreachable_mm2', '.1f')})", "—", "—", "—"])
        rows.append(["C17 full harmonic (e) (sweep.json c15_e)", "—", f"{C17E(nm, 'retracts', 'd')} (FreeCAD rule)", "—", "—", "—",
                     C17E(nm, "link_mm", ".0f"), C17E(nm, "engaged_mm", ".0f"), C17E(nm, "air_mm", ".0f"),
                     C17E(nm, "time_cut_feed_min", ".2f"), C17E(nm, "time_link_feed_min", ".2f"),
                     "—", "—", f"{C17E(nm, 'cut_width_max', '.4f')} ({C17E(nm, 'cut_width_over', 'd')})",
                     f"{C17E(nm, 'residue_mm2', '.1f')} ({C17E(nm, 'residue_unreachable_mm2', '.1f')})", "—", "—", "—"])
        for mode, t in MODES:
            rows.append(row_plan(nm, "march", mode, t))
        for mode, t in E_MODES:
            rows.append(row_plan(nm, "e", mode, t))
        for mode, t in A_MODES:
            rows.append(row_plan(nm, "a", mode, t))
        rows.append(["(a) + one-way in Rust (D12, geodesic_local_oneway)",
                     RS(nm, "entries", "d"), f"{RS(nm, 'join_entries', 'd')} join + {RS(nm, 'mop_entries', 'd')} mop", "—",
                     f"{RS(nm, 'turns', 'd')} ({RS(nm, 'over_bound', 'd')} over the bound)", RS(nm, "factor_med", ".2f"),
                     RS(nm, "keepdown_mm", ".0f"), f"{RS(nm, 'cut_mm', '.0f')} (cut length)", "—",
                     RS(nm, "time_cut_feed_min", ".2f"), RS(nm, "time_link_feed_min", ".2f"), "—", "—",
                     f"{RS(nm, 'no_turn_max', '.4f')} (any non-turn step)", "mop", "—", "—", "—"])
        tables[nm] = rows

    def table_html(rows):
        out = ['<div class="wide"><table class="all"><thead><tr>' + "".join(f"<th>{html.escape(h)}</th>" for h in HEAD) + "</tr></thead><tbody>"]
        for r in rows:
            out.append("<tr>" + "".join(f"<td>{html.escape(str(c))}</td>" for c in r) + "</tr>")
        out.append("</tbody></table></div>")
        return "\n".join(out)

    def table_md(rows):
        out = ["| " + " | ".join(HEAD) + " |", "|" + "---|" * len(HEAD)]
        for r in rows:
            out.append("| " + " | ".join(str(c) for c in r) + " |")
        return "\n".join(out)

    # ── facts for the prose ──
    F = {}
    for nm in ("star", "pocket"):
        sp = res[nm]["march"]["split"]
        F[nm] = dict(
            first=SP(nm, "march", "first_multi_level", "d"),
            splits=q(f"{nm}|march|split|split_levels", ", ".join(str(v) for v in sp["split_levels"]), "s"),
            lobes=q(f"{nm}|march|split|lobes_n", len(sp["lobes"]), "d"),
            pieces=q(f"{nm}|march|split|pieces_n", sum(L["pieces"] for L in sp["lobes"]), "d"),
            share=SP(nm, "march", "share_min", ".2f"),
            merged=q(f"{nm}|march|split|merged_n", len(sp["merged"]), "d"),
            dropped=q(f"{nm}|march|split|dropped_n", len(sp["dropped"]), "d"),
            pieces_mm=SP(nm, "march", "pieces_mm", ".0f"),
            curve_mm=SP(nm, "march", "curve_mm", ".0f"),
            clip_share=q(f"{nm}|march|split|clip_share", sp["pieces_mm"] / sp["curve_mm"], "%"),
        )

    FE = {}
    for nm in ("star", "pocket"):
        sp = res[nm]["e"]["split"]
        FE[nm] = dict(
            first=SP(nm, "e", "first_multi_level", "d"),
            splits=q(f"{nm}|e|split|split_levels", ", ".join(str(v) for v in sp["split_levels"]), "s"),
            lobes=q(f"{nm}|e|split|lobes_n", len(sp["lobes"]), "d"),
            pieces=q(f"{nm}|e|split|pieces_n", sum(L["pieces"] for L in sp["lobes"]), "d"),
            merged=q(f"{nm}|e|split|merged_n", len(sp["merged"]), "d"),
            dropped=q(f"{nm}|e|split|dropped_n", len(sp["dropped"]), "d"),
            clip_share=q(f"{nm}|e|split|clip_share", sp["pieces_mm"] / sp["curve_mm"], "%"),
        )

    def be(nm, src):
        """Break-even time per entry (s): (time of the plan - time of (a)) /
        (entries of (a) - entries of the plan), both from this replay."""
        x, a = res[nm][src][CARD], res[nm]["a"][CARD]
        return q(f"{nm}|ratio|be_{src}", 60 * (x["time_cut_feed_min"] - a["time_cut_feed_min"]) / (a["entries"] - x["entries"]), ".1f")

    sm, pm = res["star"]["march"], res["pocket"]["march"]

    # ── html ──
    H.append(f"""<h1>Clipped march arms</h1>
<p class="sub">Agent C19, 2026-10-24. Page v5 after C17's <code>farside_ideas_v4.html</code>. Star and pocket fixtures in tool-centre space. Python prototype on C15's geometry and 0.1 mm replay and C17's march loops; no Rust. Tool R 3 mm, stepover 2 mm, c·D = {cD} mm (c = {cc}). Scripts: <code>phase2/c19/</code>; <code>assert_c19.py</code> checks every number on this page.</p>
<p><b>The question (operator, 2026-10-24).</b> "The paths look great, is the issue that down both arms is one move? So the tool traverses both arms at the same time? If we could somehow clip the overlap and detect the split and traverse the paths differently the shape is perfect."</p>
<p><b>The answer in one line.</b> Yes: when the loops are cut only where they remove stock and one arm at a time, the march shape stays exactly and its air falls from {C17('star', 'air_mm', '.0f')} mm to {P('star', 'march', CARD, 'air_mm', '.0f')} mm (star). But the march still engages {q('star|ratio|eng_vs_a', sm[CARD]['engaged_mm'] / res['star']['a'][CARD]['engaged_mm'], '.2f')} times the length of (a), and its pieces start with a head-on run along the wall. The same clip on C15's harmonic loops (e) (operator's addition) gives the fewest entries of all plans here ({P('star', 'e', CARD, 'entries', 'd')} star, {P('pocket', 'e', CARD, 'entries', 'd')} pocket), but its cut is thin: it engages {q('star|ratio|e_eng_vs_a', res['star']['e'][CARD]['engaged_mm'] / res['star']['a'][CARD]['engaged_mm'], '.2f')} (star) and {q('pocket|ratio|e_eng_vs_a', res['pocket']['e'][CARD]['engaged_mm'] / res['pocket']['a'][CARD]['engaged_mm'], '.2f')} (pocket) times the length of (a).</p>
<h2>What C19 did</h2>
<p>The same five steps run on three families of curves: C17's march loops, C15's harmonic level lines (e) (closed loops, one or two per level) and C15's exact-distance wraps (a). A closed loop is cut counter-clockwise when it holds the entry, else clockwise, so the stock is always on the right.</p>
<ol>
<li><b>Clip.</b> C17's replay of each march loop in order; a sample is engaged when its area per mm over one pass step is more than one cell width (0.1 mm). Each engaged run is a cut piece. The pieces lie on the C17 loops: the largest distance of a cut point from its loop is {P('star', 'march', CARD, 'overlay_max_mm', '.1e')} mm (star) and {P('pocket', 'march', CARD, 'overlay_max_mm', '.1e')} mm (pocket). Three rules, each from the raster or the tool:
<ul><li>a closed loop whose air lies only within R of arc before its seam is a ring (that air is the loop's own start);</li>
<li>a run of one sample, or one that removes no cell, is not a piece ({F['star']['dropped']} star, {F['pocket']['dropped']} pocket);</li>
<li>two pieces of one loop and one stock part with an air gap shorter than one pass step are one piece, because the split reads over one pass step ({F['star']['merged']} star, {F['pocket']['merged']} pocket).</li></ul></li>
<li><b>Detect the split.</b> Before each level the standing stock is labelled by edge connectivity. A piece belongs to the part from which it removes the most cells. A lobe is a chain of parts with one child each; it ends where its part splits. No tuned number.</li>
<li><b>Order.</b> Rings in the core with C17's seam join. Then lobes depth first, each to its end. In a lobe a piece may be cut when every lower-level piece of the same lobe whose tool discs overlap it (centre paths within 2R) is cut; among those, the nearest entry next. A piece of the same loop next along the loop is cut next through the loop's own air run when that is the nearest step.</li>
<li><b>Link.</b> C14 one-way: each piece in climb (stock on the right; check: {P('star', 'march', CARD, 'climb_right', 'd')} samples right, {P('star', 'march', CARD, 'climb_left', 'd')} left, star); a keep-down return through what was just cut (the shorter of the way back along the piece and C17's string-pulled route); then the turn to the next piece. A turn longer than the bound is a plunge (the brief). A link that reads over c is a plunge too: FreeCAD's test leaves 2·s·R at each end out, and the rs_cam replay admits no step over c that is not a turn. Zigzag as a second variant.</li>
<li><b>Count.</b> Entries = 1 (the starter) + plunges + refusals. A refusal is a run of cut samples over c + one cell ({qc}) that is not a turn, where the rs_cam replay would refuse the step; the run that leaves a plunge is exempt, as in the replay.</li>
</ol>
""")
    for nm in ("star", "pocket"):
        H.append(f"<h2>{nm.capitalize()}</h2>")
        H.append('<p class="key"><span class="sw" style="background:#1f5fbf"></span>cut <span class="sw" style="background:#b9b6ae"></span>air (full march) '
                 '<span class="sw" style="background:#e08a00"></span>keep-down link or return <span class="sw" style="background:#d0201a;height:4px"></span>turn '
                 '<span style="display:inline-block;width:10px;height:10px;border:1.4px solid #111;border-radius:50%;margin:0 4px 0 10px;vertical-align:middle"></span>plunge</p>')
        H.append('<div class="grid">')
        for title, sent, sv, mini in cards[nm]:
            H.append(f'<div class="card"><h3>{html.escape(title)}</h3>{sv}<p>{html.escape(sent)}</p><table class="mini">'
                     + "".join(f"<tr><th>{html.escape(a)}</th><td>{html.escape(b)}</td></tr>" for a, b in mini) + "</table></div>")
        H.append("</div>")
        H.append(table_html(tables[nm]))

    def lobes_html(nm):
        L = res[nm]["march"]["split"]["lobes"]
        out = ['<table class="all"><thead><tr><th>Lobe</th><th>Parent</th><th>Levels</th><th>Children</th><th>Pieces</th><th>Levels with 2+ pieces</th></tr></thead><tbody>']
        for x in L:
            out.append(f"<tr><td>{x['id']}</td><td>{'—' if x['parent'] is None else x['parent']}</td><td>{x['first']}–{x['last']}</td>"
                       f"<td>{', '.join(map(str, x['children'])) or '—'}</td><td>{x['pieces']}</td><td>{x['multi_levels']}</td></tr>")
        out.append("</tbody></table>")
        return "\n".join(out)

    st_, po = "star", "pocket"
    H.append(f"""<h2>The split</h2>
<ul>
<li>Star: {F[st_]['pieces']} cut pieces in {F[st_]['lobes']} lobes. Loop {F[st_]['first']} is the first loop with more than one cut piece. At that level the stock splits into the five arms (lobes 1 to 5). The pieces are {F[st_]['clip_share']} of the loop length ({F[st_]['pieces_mm']} of {F[st_]['curve_mm']} mm). Each piece removes at least {F[st_]['share']} of its cells from one part.</li>
<li>Pocket: {F[po]['pieces']} cut pieces in {F[po]['lobes']} lobes. Loop {F[po]['first']} is the first loop with more than one cut piece (it reaches the bottom and left walls), but the stock stays one part round the island. It splits at levels {F[po]['splits']}, behind the island where the two fronts meet. Each piece removes at least {F[po]['share']} of its cells from one part. The pieces are {F[po]['clip_share']} of the loop length ({F[po]['pieces_mm']} of {F[po]['curve_mm']} mm).</li>
<li>The pocket's core lobe holds two pieces per level (above and below the island) for {q('pocket|march|split|lobe0_multi', res['pocket']['march']['split']['lobes'][0]['multi_levels'], 'd')} levels. Connectivity cannot tell the two sides apart; the overlap rule of the order lets the tool do one side ahead of the other.</li>
</ul>
<div class="grid"><div class="card"><h3>Star lobes</h3>{lobes_html('star')}</div><div class="card"><h3>Pocket lobes</h3>{lobes_html('pocket')}</div></div>
""")

    def dep_items(nm):
        x = res[nm]["march"]
        k = f"{nm}|march"
        return f"""<li>{nm.capitalize()}, C14 turns: the head runs on the wall for {q(f'{k}|oneway|bp.head_mm_med', x['oneway']['by_place']['head_mm_med'], '.1f')} mm (median) and reads up to {BP(nm, 'march', 'oneway', 'head', 'max', '.3f')} D in {BP(nm, 'march', 'oneway', 'head', 'pieces', 'd')} pieces. With the head as the turn and the C14 bound {tb} mm, {P(nm, 'march', 'oneway_head', 'over_bound', 'd')} turns are over the bound and become plunges; C16's slanted bound s / cos θ + (1 + √2)·cell holds {P(nm, 'march', 'oneway_head', 'over_bound_fit', 'd')} of them. The angle θ between those turns and the normal of the piece is {q(f'{k}|oneway_head|theta_med', float(np.median([t['theta'] for t in x['oneway_head']['over_bound_list']])), '.0f')}° at the median ({q(f'{k}|oneway_head|theta_min', min(t['theta'] for t in x['oneway_head']['over_bound_list']), '.0f')}° to {q(f'{k}|oneway_head|theta_max', max(t['theta'] for t in x['oneway_head']['over_bound_list']), '.0f')}°). With the slanted bound: {P(nm, 'march', 'oneway_head_slanted', 'turns', 'd')} turns, {P(nm, 'march', 'oneway_head_slanted', 'over_bound', 'd')} over it, feed factor median {q(f'{k}|oneway_head_slanted|turn_factor_med', x['oneway_head_slanted']['turn_factor_q'][2], '.2f')}.</li>
<li>{nm.capitalize()}, departure: within R of arc after the piece leaves the wall the cut reads up to {BP(nm, 'march', CARD, 'departure', 'max', '.3f')} D ({BP(nm, 'march', CARD, 'departure', 'over', 'd')} samples over c in {BP(nm, 'march', CARD, 'departure', 'pieces', 'd')} pieces). The gap to the loop before is {BX(nm, 'march', CARD, 'dep_gap_med', '.2f')} mm (median). Past the departure, the interior reads at most {BP(nm, 'march', CARD, 'interior', 'max', '.3f')} D ({BP(nm, 'march', CARD, 'interior', 'over', 'd')} over).</li>"""

    H.append(f"""<h2>The wall departure overload</h2>
<p>C17 read 0.50 D (star) and 0.44 D (pocket) where a loop leaves a wall. The clip does not remove it. The cause is now clear: each piece starts with a <b>head</b>, a run along the wall into new stock, before it leaves the wall. That run is the step-over at the arc end, the same step that C14 makes a counted turn. Under C14's straight turn it stays inside the piece at full feed.</p>
<ul>
{dep_items('star')}
{dep_items('pocket')}
</ul>
<p><b>Two derived fixes</b> (the second is not built):</p>
<ol>
<li>The head is the turn: the turn runs from the previous departure along the head to this departure, its feed is cut by c / max(w, a) (C14), and its bound is C16's slanted bound, because the march meets the walls at an angle. Built as the plan of the cards.</li>
<li>The departure corner: extend the counted turn to the last sample over c within R after the departure. That adds {BX('star', 'march', CARD, 'ext_med', '.2f')} mm (star) and {BX('pocket', 'march', CARD, 'ext_med', '.2f')} mm (pocket) at the median; the longer turn stays within its bound in {BX('star', 'march', CARD, 'ext_fit', 'd')} of {BX('star', 'march', CARD, 'ext_n', 'd')} (star) and {BX('pocket', 'march', CARD, 'ext_fit', 'd')} of {BX('pocket', 'march', CARD, 'ext_n', 'd')} (pocket). The other way is a fillet. C15 (c)'s bound is for a corner where the cleared side is concave; at the departure the reading is over the gap, so the cleared side is convex and the C5 law gives a lower bound instead: ρ ≥ ρ(g) with s'(ρ(g)) = g. At the median gap that is {BX('star', 'march', CARD, 'dep_rho_med', '.1f')} mm (star) and {BX('pocket', 'march', CARD, 'dep_rho_med', '.1f')} mm (pocket), at most {BX('star', 'march', CARD, 'dep_rho_max', '.1f')} and {BX('pocket', 'march', CARD, 'dep_rho_max', '.1f')} mm. A fillet changes the march shape, so this note prefers fix 2 by the turn.</li>
</ol>
""")

    def e_items(nm):
        return f"""<li>{nm.capitalize()}: loop {FE[nm]['first']} is the first loop with more than one cut piece; the stock splits at level {FE[nm]['splits']}; {FE[nm]['pieces']} pieces in {FE[nm]['lobes']} lobes. The pieces are {FE[nm]['clip_share']} of the loop length. {FE[nm]['dropped']} runs are too small to be pieces and {FE[nm]['merged']} gaps are merged (the march: {F[nm]['dropped']} and {F[nm]['merged']}): the thin cut breaks into many short runs.</li>
<li>{nm.capitalize()}, time: {C17E(nm, 'time_cut_feed_min', '.2f')} min for the full (e) and {P(nm, 'e', CARD, 'time_cut_feed_min', '.2f')} min clipped (air at the cut feed); {P(nm, 'a', CARD, 'time_cut_feed_min', '.2f')} min for (a) with the same links. Entries {P(nm, 'e', CARD, 'entries', 'd')} = 1 + {P(nm, 'e', CARD, 'plunges', 'd')} plunges ({P(nm, 'e', CARD, 'plunge_seam', 'd')} core seams) + {P(nm, 'e', CARD, 'refusals', 'd')} refusals. Turns {P(nm, 'e', CARD, 'turns', 'd')}, of them {P(nm, 'e', CARD, 'turn_over', 'd')} over c: a thin cut gives a light turn.</li>
<li>{nm.capitalize()}, engagement (one-way, slanted bound): engaged length {P(nm, 'e', CARD, 'engaged_mm', '.0f')} mm, against {P(nm, 'march', CARD, 'engaged_mm', '.0f')} for the march and {P(nm, 'a', CARD, 'engaged_mm', '.0f')} for (a). Median swept width of the engaged samples {P(nm, 'e', CARD, 'eng_w_med', '.3f')} D (march {P(nm, 'march', CARD, 'eng_w_med', '.3f')}, (a) {P(nm, 'a', CARD, 'eng_w_med', '.3f')}); mean width per piece, median {P(nm, 'e', CARD, 'piece_w_med', '.3f')} and smallest {P(nm, 'e', CARD, 'piece_w_min', '.3f')} D (march {P(nm, 'march', CARD, 'piece_w_med', '.3f')} / {P(nm, 'march', CARD, 'piece_w_min', '.3f')}, (a) {P(nm, 'a', CARD, 'piece_w_med', '.3f')} / {P(nm, 'a', CARD, 'piece_w_min', '.3f')}).</li>
<li>{nm.capitalize()}, load: head up to {BP(nm, 'e', CARD, 'head', 'max', '.3f')} D, departure up to {BP(nm, 'e', CARD, 'departure', 'max', '.3f')} D, interior at most {BP(nm, 'e', CARD, 'interior', 'max', '.3f')} D ({BP(nm, 'e', CARD, 'interior', 'over', 'd')} over). Coverage left {P(nm, 'e', CARD, 'residue_mm2', '.1f')} mm² ({P(nm, 'e', CARD, 'residue_unreachable_mm2', '.1f')} unreachable); the full (e) leaves {C17E(nm, 'residue_mm2', '.1f')} mm².</li>"""

    H.append(f"""<h2>The harmonic (e), clipped and split</h2>
<p>The operator: "I think the same thing could be done with harmonic too." It can. The clip, the split, the order and the links run on the (e) loops with no change. What the clip cannot fix is the uneven gap of (e): a level line of the harmonic field lies close to the one before where the field is steep, so most of each loop cuts a thin strip. The clip removes the air, not the thin cut.</p>
<ul>
{e_items('star')}
{e_items('pocket')}
</ul>
<p>On the pocket the clipped (e) leaves stock that the full (e) cuts: the runs too small to be pieces (one sample, or no cell) hold it.</p>
<p><b>Which is faster depends on the cost of an entry.</b> With the counts of this replay, clipped (e) is faster than (a) with the same links only when one entry costs more than {be('star', 'e')} s (star) or {be('pocket', 'e')} s (pocket); the clipped march only when it costs more than {be('star', 'march')} s (star) or {be('pocket', 'march')} s (pocket). The cost of an entry (lift, travel, re-entry at the plunge feed) is not measured here.</p>
""")

    star_t = res["star"]["march"][CARD]["time_cut_feed_min"]
    H.append(f"""<h2>Verdict</h2>
<div class="verdict"><ul>
<li><b>The operator's reading is right.</b> The full march cuts every loop whole, so it runs again along all the wall that earlier loops cut. Clipped, split per lobe and linked one-way, the same loops (to {P('star', 'march', CARD, 'overlay_max_mm', '.1e')} mm) take {P('star', 'march', CARD, 'time_cut_feed_min', '.2f')} min against {C17('star', 'time_cut_feed_min', '.2f')} (star) and {P('pocket', 'march', CARD, 'time_cut_feed_min', '.2f')} min against {C17('pocket', 'time_cut_feed_min', '.2f')} (pocket), air at the cut feed.</li>
<li><b>It is still slower than (a) with the same links:</b> {P('star', 'a', CARD, 'time_cut_feed_min', '.2f')} min (star) and {P('pocket', 'a', CARD, 'time_cut_feed_min', '.2f')} min (pocket). The march engages {P('star', 'march', CARD, 'engaged_mm', '.0f')} mm against {P('star', 'a', CARD, 'engaged_mm', '.0f')} mm (star) and {P('pocket', 'march', CARD, 'engaged_mm', '.0f')} against {P('pocket', 'a', CARD, 'engaged_mm', '.0f')} mm (pocket). No link removes that: it is the march's thinner passes.</li>
<li><b>Plunges:</b> {P('star', 'march', CARD, 'plunges', 'd')} (star) and {P('pocket', 'march', CARD, 'plunges', 'd')} (pocket) against {P('star', 'a', CARD, 'plunges', 'd')} and {P('pocket', 'a', CARD, 'plunges', 'd')} for (a). The core rings give {P('star', 'march', CARD, 'plunge_seam', 'd')} and {P('pocket', 'march', CARD, 'plunge_seam', 'd')} of them (the seam step reads over c).</li>
<li><b>Load:</b> the march needs C16's slanted bound to take its heads as turns (with the C14 bound: {P('star', 'march', 'oneway_head', 'plunges', 'd')} and {P('pocket', 'march', 'oneway_head', 'plunges', 'd')} plunges). Its departure corners still read up to {BP('star', 'march', CARD, 'departure', 'max', '.3f')} D (star) and {BP('pocket', 'march', CARD, 'departure', 'max', '.3f')} D (pocket). In the same replay the refusals are {P('star', 'march', CARD, 'refusals', 'd')} and {P('pocket', 'march', CARD, 'refusals', 'd')} for the march, {P('star', 'a', CARD, 'refusals', 'd')} and {P('pocket', 'a', CARD, 'refusals', 'd')} for (a); the entries are {P('star', 'march', CARD, 'entries', 'd')} and {P('pocket', 'march', CARD, 'entries', 'd')} against {P('star', 'a', CARD, 'entries', 'd')} and {P('pocket', 'a', CARD, 'entries', 'd')}. The Python replay over-reads the Rust one: for (a) + one-way, Rust has {RS('star', 'join_entries', 'd')} and {RS('pocket', 'join_entries', 'd')} join entries. So on entries the clipped march is not worse than (a); on time it is.</li>
<li><b>Harmonic (e), clipped:</b> the fewest entries ({P('star', 'e', CARD, 'entries', 'd')} star, {P('pocket', 'e', CARD, 'entries', 'd')} pocket, against {P('star', 'a', CARD, 'entries', 'd')} and {P('pocket', 'a', CARD, 'entries', 'd')} for (a)) and the lightest load (pocket: {P('pocket', 'e', CARD, 'refusals', 'd')} refusals). But it takes {P('star', 'e', CARD, 'time_cut_feed_min', '.2f')} and {P('pocket', 'e', CARD, 'time_cut_feed_min', '.2f')} min against {P('star', 'a', CARD, 'time_cut_feed_min', '.2f')} and {P('pocket', 'a', CARD, 'time_cut_feed_min', '.2f')}: the clip cannot make a thin cut thick.</li>
<li><b>Not a candidate for Rust as it stands.</b> If the operator wants the march shape, the next step is fix 2 (the departure as part of the turn) and a ruling on the slanted bound; it would still engage more length than (a).</li>
</ul></div>
<h2>Limits</h2>
<ul>
<li>The entry counts of this replay are higher than Rust's for the same family, on both sides of each break-even; the break-even values compare like with like, not with D12.</li>
<li>Two shapes, one tool, one stepover. The replay is C15's 0.1 mm raster, not the planner, and it reads the (a) ends harder than the Rust replay does.</li>
<li>Times are length over feed (cut feed {q('const|F_cut', inp['F_CUT'], '.0f')} mm/min, link feed {q('const|F_link', inp['F_LINK'], '.0f')} mm/min, turns at their reduced feed), with no acceleration; plunges are counted, not timed.</li>
<li>The core rings keep C17's seam join; a spiral morph of the rings is not tried.</li>
<li>No mop: stock that the plan leaves is reported as coverage left.</li>
</ul>
""")

    css = """:root { --bg:#fbfaf7; --ink:#1b1b1b; --muted:#5b5b5b; --line:#d9d6cf; --card:#ffffff; --accent:#1f5fbf; }
body { background:var(--bg); color:var(--ink); font:15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; margin:0; }
main { max-width:1220px; margin:0 auto; padding:20px 16px 60px; }
h1 { font-size:26px; margin:6px 0 4px; } h2 { font-size:19px; margin:30px 0 8px; border-bottom:1px solid var(--line); padding-bottom:4px; }
h3 { font-size:16px; margin:0 0 6px; } p, li { max-width:84ch; } p { margin:6px 0; } .sub { color:var(--muted); }
.grid { display:grid; grid-template-columns:repeat(auto-fit, minmax(270px, 1fr)); gap:12px; margin-bottom:14px; }
.card { background:var(--card); border:1px solid var(--line); border-radius:8px; padding:12px; }
.pane { width:100%; height:auto; display:block; margin:6px 0; background:#fff; }
table { border-collapse:collapse; font-size:13px; } table.mini { width:100%; }
table.mini th { text-align:left; font-weight:500; color:var(--muted); padding:2px 6px 2px 0; }
table.mini td { padding:2px 10px 2px 0; font-variant-numeric:tabular-nums; }
.wide { overflow-x:auto; margin:8px 0 18px; }
table.all th, table.all td { border-bottom:1px solid var(--line); padding:4px 8px; text-align:right; font-variant-numeric:tabular-nums; vertical-align:top; }
table.all th:first-child, table.all td:first-child { text-align:left; }
.key span.sw { display:inline-block; width:22px; height:3px; vertical-align:middle; margin:0 4px 0 10px; }
.verdict { background:#eef3fb; border-left:4px solid var(--accent); padding:8px 12px; border-radius:4px; }
code { font-size:13px; }"""
    page = ("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n"
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Clipped march arms</title>\n"
            f"<style>\n{css}\n</style></head>\n<body><main>\n" + "\n".join(H) + "\n</main></body></html>\n")
    with open(PAGE, "w") as f:
        f.write(page)

    # ── note ──
    def strip(s):
        s = re.sub(r"<[^>]+>", "", s)
        return html.unescape(s)

    N.append(f"""# P2 C19 — the streamline march and the harmonic (e), clipped and split per lobe (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Page: `farside_ideas_v5.html`. Scripts: `phase2/c19/` (`clip.py`,
`build.py`, `run_c19.py`, `page_c19.py`, `assert_c19.py`). Run `run_c19.py`
(about a minute), then `page_c19.py`, then `assert_c19.py`. `page_c19.py`
writes this note and the page; `assert_c19.py` recomputes every quoted
number. C15's and C17's scripts are imported and not edited.

## 1. The question

The operator, on C17's march renders: "The paths look great, is the issue
that down both arms is one move? So the tool traverses both arms at the same
time? If we could somehow clip the overlap and detect the split and traverse
the paths differently the shape is perfect."

## 2. Method

Inputs: C17's march loops (`curves_c17.npz`, run `h_step`), C15 (a)
(`curves_whole.npz`), C15's replay, c·D = {cD} mm (c = {cc}), the C14
turn bound s + (1 + √2)·cell = {tb} mm (cell 0.5 mm), c plus one cell
{qc}.

""")
    for item in re.findall(r"<li><b>(.*?)</b>(.*?)</li>\n(?=<li><b>|</ol>)", H[0], re.S):
        N.append(f"- **{strip(item[0])}**{strip(item[1])}".replace("\n", " ") + "\n")
    ans = re.search(r"<p><b>The answer in one line.</b>(.*?)</p>", H[0], re.S).group(1)
    N.append("\n**The answer in one line.**" + strip(ans) + "\n")
    N.append("\n## Cards (one sentence each, as on the page)\n")
    for nm in ("star", "pocket"):
        for title, sent, _, mini in cards[nm]:
            N.append(f"- {nm.capitalize()}, {title}: {sent} " + "; ".join(f"{a} {b}" for a, b in mini) + ".\n")
    N.append("\n## 3. Tables\n\nColumns: entries = 1 + plunges + refusals. Plunges by kind: seam (core ring "
             "joins), turn over the bound, lobe change with no keep-down route, in-lobe link with none. Head: the "
             "piece's leading run on the wall. Departure: within R of arc after the piece leaves the wall. Interior: "
             "the rest more than R from the piece ends. Readings are swept width / D.\n")
    for nm in ("star", "pocket"):
        N.append(f"\n{nm.capitalize()}:\n\n<!-- {nm}:begin -->\n" + table_md(tables[nm]) + f"\n<!-- {nm}:end -->\n")
    for sec in H[1:]:
        if sec.startswith("<h2>The split") or sec.startswith("<h2>The wall") or sec.startswith("<h2>The harmonic") or sec.startswith("<h2>Verdict"):
            body = re.sub(r'<div class="grid">.*?</div></div>\n', "", sec, flags=re.S)
            t = re.sub(r"<h2>(.*?)</h2>", lambda m: f"\n## {m.group(1)}\n", body)
            t = re.sub(r"<li>", "- ", t)
            t = re.sub(r"</li>", "", t)
            t = strip(t)
            t = re.sub(r"\n\s*\n+", "\n\n", t)
            N.append(t)
    for nm in ("star", "pocket"):
        N.append(f"\n## Lobes, {nm}\n\n| Lobe | Parent | Levels | Children | Pieces | Levels with 2+ pieces |\n|---|---|---|---|---|---|\n")
        for x in res[nm]["march"]["split"]["lobes"]:
            N.append(f"| {x['id']} | {'—' if x['parent'] is None else x['parent']} | {x['first']}–{x['last']} | "
                     f"{', '.join(map(str, x['children'])) or '—'} | {x['pieces']} | {x['multi_levels']} |\n")
    with open(NOTE, "w") as f:
        f.write("".join(s if s.endswith("\n") else s + "\n" for s in N))
    with open(os.path.join(HERE, "quotes_c19.json"), "w") as f:
        json.dump(QUOTES, f, indent=1)
    print("page", PAGE, os.path.getsize(PAGE), "note", NOTE, "quotes", len(QUOTES))


if __name__ == "__main__":
    main()
