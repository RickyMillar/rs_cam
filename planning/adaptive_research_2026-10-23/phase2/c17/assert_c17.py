# C17: assert every number that farside_ideas_v3.html and
# P2_C17_streamline_march.md quote.
# Run: python3 -I assert_c17.py           (checks the stored results)
#      python3 -I assert_c17.py --rerun   (runs run_c17.py first, about 70 s)
import html as htmlmod
import json
import math
import os
import re
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)
        print("FAIL", what)


if "--rerun" in sys.argv:
    subprocess.run([sys.executable, "-I", os.path.join(HERE, "run_c17.py")], check=True)

import stream as st  # noqa: E402
import page_c17 as pg  # noqa: E402

# ── 1. Inputs from the code formula (independent of the pipeline) ──────
R, s, tol = 3.0, 2.0, 0.1
D = 2 * R
f_arc = min(max(math.acos(1 - s / R) / (2 * math.pi) * 1.05, 0.0), 0.5)
c = (1 - math.cos(2 * math.pi * f_arc)) / 2
check(abs(c * D - 2.175868) < 5e-7, f"c*D {c * D}")
check(abs(c - 0.3626) < 5e-5, f"c {c}")
check(st.CD == c * D and st.TOL == tol and st.PASS_STEP == 1.5, "stream inputs")

res, cur = pg.load()
res = pg.derive(res)
inp = res["inputs"]
check(abs(inp["cD"] - c * D) < 1e-12 and inp["pass_step"] == 1.5 and inp["tol"] == tol, "results inputs")

# ── 2. Recompute march numbers from the stored loops ────────────────────
for nm in ("star", "pocket"):
    for k in pg.RUNS:
        r = res[nm][k]
        loops = cur[(nm, k)]
        check(len(loops) == r["loops"], f"{nm} {k} loops")
        check(bool(loops[-1]["stop"].all()) == r["all_stopped"], f"{nm} {k} all stopped")
        L = W = A = 0.0
        for lp in loops:
            P = lp["P"].astype(float)
            sl = st.seglen(P)
            was = lp["was"]
            both = was & np.roll(was, -1)
            kd = lp["kind"]
            air = both & ((kd == 2) | (np.roll(kd, -1) == 2))
            L += sl.sum()
            W += sl[both & ~air].sum()
            A += sl[air].sum()
        for name, v in (("length_mm", L), ("wall_follow_mm", W), ("air_follow_mm", A)):
            check(abs(v - r[name]) <= 1e-4 * max(1.0, r[name]), f"{nm} {k} {name} {v} vs {r[name]}")
        check(abs(r["moving_mm"] - (r["length_mm"] - r["wall_follow_mm"] - r["air_follow_mm"])) < 1e-6, f"{nm} {k} moving")
        check(abs(r["wall_share"] - (W + A) / L) < 1e-5, f"{nm} {k} wall share")
        check(r["residue_unreachable_mm2"] <= r["residue_mm2"] + 1e-9, f"{nm} {k} unreachable <= residue")
        check(r["rule_violation_max_mm"] < 1e-3 or k != "h_step", f"{nm} {k} rule holds")

# the lead's prediction: the normal gap is at most the streamline step
for nm in ("star", "pocket"):
    check(res[nm]["h_step"]["normal_over_step_median"] <= 1.0 + 1e-9, f"{nm} normal/step median <= 1")
# the load more than R from a wall holds the ceiling within one cell
for nm in ("star", "pocket"):
    r = res[nm]["h_step"]
    check(r["sw_far_max"] <= c + tol / D and r["sw_far_over"] == 0, f"{nm} far load {r['sw_far_max']}")
    check(r["over_within_R_of_wall"] == r["sw_interior_over"], f"{nm} every over-c sample within R of a wall")
# the control reproduces (a) before the walls
check(res["star"]["g_step"]["max_radial_diff_mm"] < tol / 10 and res["star"]["g_step"]["loops_compared"] >= 5,
      "control matches (a)")

# ── 3. The C15 rows equal C15's own results ─────────────────────────────
c15 = json.load(open(os.path.join(st.C15, "results_whole.json")))
for nm in ("star", "pocket"):
    for k in ("a", "e"):
        mine, theirs = res[nm]["c15_" + k], c15[nm][k]
        for f in ("levels", "loops", "open_arcs", "levels_multi"):
            check(mine[f] == theirs[f], f"{nm} {k} {f}")
        for f in ("sw_interior_max", "apm_interior_max", "residue_interior_mm2"):
            check(abs(mine[f] - theirs[f]) < 1e-9, f"{nm} {k} {f}")
        check(mine["length_mm"] == theirs["length_mm"], f"{nm} {k} length")
        check(abs(mine["replay_length_mm"] - theirs["length_mm"]) < 0.01 * theirs["length_mm"], f"{nm} {k} replay length")

# ── 4. Replay one march again from the stored loops ─────────────────────
for nm in ("star", "pocket"):
    sh = st.Shape(nm)
    loops = [dict(P=lp["P"].astype(float), was_stopped=lp["was"], stop=lp["stop"], kind=lp["kind"])
             for lp in cur[(nm, "h_step")]]
    out, rem = st.replay_loops(sh, loops)
    inner = out["w"][out["cls"] == "interior"]
    r = res[nm]["h_step"]
    check(abs(inner.max() - r["sw_interior_max"]) < tol / D, f"{nm} replay interior {inner.max()} vs {r['sw_interior_max']}")
    check(abs(out["area"] / out["length"] / D - r["mean_width_frac"]) < 1e-3, f"{nm} replay mean width")

# ── 5. Every claim on the page; the page and the note are what the
#      results give ───────────────────────────────────────────────────────
page = open(pg.PAGE).read()
claims = re.findall(r'<span data-claim="([^"]+)" data-fmt="([^"]+)">([^<]*)</span>', page)
check(len(claims) > 200, f"claims {len(claims)}")
for key, f, txt in claims:
    want = pg.fmt(pg.get(res, key), f)
    check(htmlmod.unescape(txt) == want, f"page claim {key}: {txt} vs {want}")
_, html = pg.build()
check(html == page, "page equals a fresh build")
note = open(pg.NOTE).read()
check(note == pg.note(res), "note equals a fresh build")
# every number in the note's fixed text (with each claim replaced by a
# marker) is a date, a section or agent number, a code constant (0.1 mm
# tolerance, 0.0001 mm solve tolerance) or a percentile label (90 %, 99 %)
marked = pg.note(res, C=lambda key, f=".2f": "§")
left = sorted(set(re.findall(r"(?<![\w.])\d+(?:\.\d+)?(?![\w])", marked)))
print("numbers in the fixed note text:", left)
allowed = {"2026", "10", "24", "17", "15", "14", "5", "70", "0.0001", "0.1", "90", "99", "1", "2", "3", "4", "6", "0"}
check(set(left) <= allowed, f"note numbers not from a claim: {sorted(set(left) - allowed)}")
pmarked = re.sub(r'<span data-claim[^>]*>[^<]*</span>', "§", page)
pmarked = re.sub(r"<svg.*?</svg>", "", pmarked, flags=re.S)
pmarked = re.sub(r"<style>.*?</style>", "", pmarked, flags=re.S)
pmarked = re.sub(r'<[^>]+>', " ", pmarked)
pleft = sorted(set(re.findall(r"(?<![\w.#])\d+(?:\.\d+)?(?![\w])", pmarked)))
print("numbers in the fixed page text:", pleft)
check(set(pleft) <= allowed, f"page numbers not from a claim: {sorted(set(pleft) - allowed)}")

print("C17 asserts:", "OK" if not FAILS else f"{len(FAILS)} FAILED")
sys.exit(1 if FAILS else 0)
