"""C13: assert each number that P2_C13_lobe_margin.md quotes.

Run from any folder:
    python3 -I final.py      (once, about 15 minutes; writes final.json)
    python3 -I assert_c13.py
The cheap numbers (ceiling, spans, law check, closed-form width, one full
band) are recomputed here; the band tables come from final.json, and one
band of each table is rebuilt here and compared with it.
"""
import json
import math
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from geom import (CD, CD_C10, CELL, R, S, SEED, Stock, arc_range, arm_axis,  # noqa: E402
                  exact_width, growing_span, reserve_delta, span_exact, span_min, w_front,
                  width_no_wall)
from band import build, exact_band  # noqa: E402
from final import C10_BANDS, LAW, c10_law_check  # noqa: E402
from rule import corner_touch  # noqa: E402

F = json.load(open(os.path.join(HERE, "final.json")))


def close(a, b, t):
    return abs(a - b) <= t


def ok(r):
    return r["fits"] and r["lead"] <= CD + 1e-6


def by_band(rows):
    out = {}
    for r in rows:
        out.setdefault((r["level"], r["arm"]), []).append(r)
    return out


# ---- the ceiling and the C10 inputs ---------------------------------------
assert close(CD, 2.175868, 5e-7) and close(CD_C10, 2.1756, 1e-12)
assert close(CD - CD_C10, 0.00027, 5e-6) and CELL == 0.5
rho_star = S * (2 * R - S) / (2 * (CD_C10 - S))
assert close(rho_star, 22.78, 0.005)
C10_D = {9: 1.952, 10: 1.957, 11: 1.947, 12: 1.915, 13: 1.848, 14: 1.752, 15: 1.568}
for lvl, rho, L in C10_BANDS:
    assert close(reserve_delta(rho, L), C10_D[lvl], 5e-4)          # C10 reserve table
    assert close(F["deltas"][str(lvl)], C10_D[lvl], 5e-4)

# level 8 is not a lobe band: its parent (level 7, 15.15 mm) is a closed wrap,
# since a circle of radius < 19.09 - R round the seed stays in the region
assert LAW[7] < 19.09 - R and arc_range(LAW[7], arm_axis(0), cap=math.pi) is not None
assert close(arc_range(LAW[7], arm_axis(0), cap=math.pi)[1] - arc_range(LAW[7], arm_axis(0), cap=math.pi)[0],
             2 * math.pi, 1e-6)
assert 7 * 5 == 35

# ---- the closed-form exact width against the numeric one -------------------
st = Stock([(23.05, -math.inf, math.inf)], -math.pi / 2, walls=False)
for b, be in [(0.5, 0.3), (1.0, 0.05), (1.5, 0.8), (1.99, 0.3)]:
    r = 23.05 + b
    th = -1.0
    p = SEED + r * np.array([math.cos(th), math.sin(th)])
    et = np.array([-math.sin(th), math.cos(th)])
    er = np.array([math.cos(th), math.sin(th)])
    num = exact_width(st, p, math.cos(be) * et + math.sin(be) * er)[0]
    assert close(num, width_no_wall(b, be, 25.05, 2.0), 3e-5)
# a plain pass at mid-arc reads the C5 formula (exact model check)
b12 = build(0, 25.05, 1.915, +1, "c10")
e12 = exact_band(b12, True)
assert close(e12["plain_mid"], w_front(25.05, b12["delta"]), 3e-5)

# ---- spans: C10 law and C13 law at s ------------------------------------------
assert close(growing_span(1e9, S), 7.70, 0.01)
for rho, g, x in [(25.05, 14.37, 12.53), (31.05, 11.32, 9.62)]:
    assert close(growing_span(rho, S), g, 0.01)
    assert abs(span_min(rho, S) - g) <= 0.03          # the C13 law: the C10 span
    assert close(span_exact(rho, S), x, 0.01)        # the exact tilt alone: shorter

# ---- law check (closed form, no wall) -------------------------------------------
over = {}
for lvl, rho, _ in C10_BANDS:
    w, bite = c10_law_check(rho, F["deltas"][str(lvl)])
    over[lvl] = w - CD
    assert close(bite, F["deltas"][str(lvl)], 1e-9)     # the excess is at full bite
assert all(over[k] > 0 for k in (9, 10, 11, 12)) and all(over[k] < 0 for k in (13, 14, 15))
assert close(over[9], 0.00010, 2e-5) and close(over[11], 0.00048, 2e-5) and max(over.values()) < 0.0005

# ---- table A: C10 law at C10's spacing -------------------------------------------
A = by_band(F["A"])
assert len(A) == 35
passA = [k for k, rs in A.items() if all(ok(r) for r in rs)]
assert len(passA) == 12 and all(k[0] >= 13 and k[1] != 0 for k in passA)
exA = {k: max(r["lead"] for r in rs) - CD for k, rs in A.items()}
small = [k for k, v in exA.items() if 1e-6 < v <= 0.0006]
corner = {k: v for k, v in exA.items() if v > 0.001}
assert len(small) == 19 and all(k[0] <= 12 for k in small)
assert sorted(corner) == [(12, 0), (13, 0), (14, 0), (15, 0)]
assert close(corner[(12, 0)], 0.020, 0.001) and close(corner[(15, 0)], 0.236, 0.001)
for k in corner:   # the worst point is on the ramp, about 1 mm from the far wall
    w = max(A[k], key=lambda r: r["lead"])
    assert w["piece"] == "ramp" and 0.7 <= w["wall"] <= 1.05 and w["spare"] <= 1.2

# ---- table B: C13 law at C10's spacing --------------------------------------------
B = by_band(F["B"])
passB = [k for k, rs in B.items() if all(ok(r) for r in rs)]
assert len(passB) == 31 and sorted(set(B) - set(passB)) == [(12, 0), (13, 0), (14, 0), (15, 0)]
assert all(r["nowall"] <= CD + 2e-5 for r in F["B"] if r["fits"])

# ---- the accepted spacing and the rule --------------------------------------------
acc = {int(k): v for k, v in F["accepted"].items()}
rul = {int(k): v for k, v in F["rule"].items()}
ACC = {9: 1.9577, 10: 1.9612, 11: 1.9465, 12: 1.9033, 13: 1.8201, 14: 1.6765, 15: 1.4451}
for k, v in ACC.items():
    assert close(acc[k], v, 1e-4)
for k in (9, 10, 11, 12):
    assert abs(rul[k] - acc[k]) <= 0.0013
for k in (13, 14, 15):
    assert 0.003 <= acc[k] - rul[k] <= 0.063
assert acc[9] > C10_D[9] and acc[10] > C10_D[10] and all(acc[k] < C10_D[k] for k in (12, 13, 14, 15))
M = {k: CD - w_front(LAW[k], acc[k]) for k in acc}
assert close(M[9], 0.0106, 1e-4) and close(M[14], 0.3746, 1e-4) and close(M[15], 0.6248, 1e-4)
assert all(M[k] < CELL for k in range(9, 15)) and M[15] > CELL
Rb = by_band(F["R"])
assert len(Rb) == 35 and all(all(ok(r) for r in rs) for rs in Rb.values())
touch = {r["level"]: r["touch"] for r in F["R"] if r["arm"] == 0 and r["sgn"] > 0}
assert close(touch[9], 0.66, 0.01) and close(touch[15], 1.79, 0.01)

# ---- lattice -------------------------------------------------------------------------
def lat(rows, sp):
    plus = [r for r in rows if r["sgn"] > 0 and "lattice" in r]
    leads = [x for r in plus for x in r["lattice"][sp]["lead"]]
    diffs = [x - r["lead"] for r in plus for x in r["lattice"][sp]["lead"]]
    pairs = sum(1 for r in plus for n in r["lattice"][sp]["lead_over"] if n > 0)
    bands = sum(1 for r in plus if any(n > 0 for n in r["lattice"][sp]["lead_over"]))
    med = [x for r in plus for x in r["lattice"][sp]["ramp_med"]]
    return min(leads), max(leads), min(diffs), max(diffs), pairs, bands, min(med), max(med)

la = lat(F["A"], "cell")
assert close(la[2], -0.707, 0.001) and la[3] <= 1e-3 and la[4] == 2 and la[5] == 2
lr = lat(F["R"], "cell")
assert close(lr[2], -0.561, 0.001) and close(lr[3], -0.016, 0.001) and lr[4] == 0
assert close(lr[6], 0.841, 0.001) and close(lr[7], 1.684, 0.001)
ls = lat(F["R"], "step")
assert close(ls[2], -0.475, 0.001) and close(ls[3], 0.281, 0.001) and ls[4] == 36 and ls[5] == 25
lsa = lat(F["A"], "step")
assert lsa[5] == 26
for k in corner:   # the lattice accepts what exact geometry rejects (one cell steps)
    r = [x for x in A[k] if x["sgn"] > 0][0]
    assert sum(1 for n in r["lattice"]["cell"]["lead_over"] if n == 0) >= 3
    assert max(r["lattice"]["cell"]["lead"]) < r["lead"]

# ---- the plain wall arrival -----------------------------------------------------------
pe = [max(r["plain_F"], r["plain_E"]) for r in F["A"] if r["fits"]]
assert close(min(pe), 2.252, 0.001) and close(max(pe), 2.697, 0.001)
lp = [x for r in F["A"] if "lattice" in r for x in r["lattice"]["cell"]["plain"]]
lps = [x for r in F["A"] if "lattice" in r for x in r["lattice"]["step"]["plain"]]
assert close(min(lp), 1.453, 0.001) and close(max(lps), 2.462, 0.001)

# ---- the sequence and the cost ---------------------------------------------------------
seq = F["seq"]
assert len(seq) - 1 == 8 and close(seq[0], 17.10, 1e-9) and close(seq[-1], 31.05, 1e-9)
assert close(seq[-1] - seq[-2], 1.054, 0.001)
K = F["K"]
assert len(K) == 80 and all(ok(r) for r in K)
cur = sum(r["L_b"] for r in F["A"] if r["sgn"] > 0)
arcs = sum(r["L_b"] for r in K if r["sgn"] > 0)
extra = sum(r["ramp_len"] + r["spare"] for r in K if r["sgn"] > 0)
assert close(cur, 353.3, 0.05) and close(arcs, 389.2, 0.05) and close(extra, 442.8, 0.05)
assert close(arcs + extra - cur, 478.7, 0.05) and close((arcs + extra - cur) / 35, 13.68, 0.01)
assert close((arcs - cur) / 35, 1.03, 0.01) and close(extra / 35, 12.65, 0.01)

# ---- one band of each table rebuilt here --------------------------------------------------
for name, law in (("A", "c10"), ("R", "min")):
    r = [x for x in F[name] if x["level"] == 13 and x["arm"] == 0 and x["sgn"] > 0][0]
    b = build(0, r["rho"], r["delta"], +1, law)
    assert close(exact_band(b, True)["lead"], r["lead"], 1e-9)
    assert close(corner_touch(b), r["touch"], 1e-9)

# ---- the further numbers the note quotes ---------------------------------------------
# the star lobe arcs: seed-visible (one branch label), both ends on the inset wall
sys.path.insert(0, os.path.join(HERE, "..", "c10"))
import model as M10  # noqa: E402
F10 = M10.Field(M10.Star())
lv10 = M10.law_levels(F10.dmax)
K10 = M10.core_levels(F10, lv10)
arm_arcs = [c for c in M10.chains_of(F10, lv10) if c.level >= K10 + 1 and not c.closed and c.level <= 18]
assert arm_arcs and all(c.labels == frozenset({0}) for c in arm_arcs)
ends = [float(M10.Star().boundary_dist(np.array([p[0]]), np.array([p[1]]))[0])
        for c in arm_arcs for p in (c.pts[0], c.pts[-1])]
assert 3.0 - 1e-9 <= min(ends) and max(ends) <= 3.095
# arm 0 holds the shortest arc of every level
for lvl in range(9, 16):
    ls = {r["arm"]: r["L_b"] for r in F["A"] if r["level"] == lvl and r["sgn"] > 0}
    assert min(ls.values()) == ls[0]
# the rim pitch and the tilt sensitivity at full bite
from geom import NPHI  # noqa: E402
assert close(math.pi * R / NPHI, 0.0031, 1e-4)
dwdb = math.sqrt(R * R - (R - CD) ** 2)
assert close(dwdb, 2.885, 0.001) and close(dwdb * 0.002, 0.0058, 1e-4)
assert close(e12["plain_mid"], w_front(25.05, b12["delta"]), 2e-5)
# the exact tilt alone leaves the region at the wall it starts from
assert not build(0, 25.05, 1.915, +1, "exact")["ramp_inside"]
assert build(0, 25.05, 1.915, +1, "min")["ramp_inside"]
# the law-check margins
for lvl, m in {9: 0.0167, 10: 0.0311, 11: 0.0582, 12: 0.1045, 13: 0.1859, 14: 0.2958, 15: 0.4962}.items():
    assert close(CD - w_front(LAW[lvl], F["deltas"][str(lvl)]), m, 1e-4)
# the cut back holds the ceiling in every band orientation of every table
for name in ("A", "B", "R", "K"):
    assert all(r["back"] <= CD for r in F[name] if r["fits"])
# the corner failures: worst point and spare
cw = sorted(round(max(A[k], key=lambda r: r["lead"])["wall"], 2) for k in corner)
cs = sorted(round(min(r["spare"] for r in A[k]), 2) for k in corner)
assert cw == [0.73, 0.77, 1.01, 1.02] and cs == [0.8, 0.85, 1.13, 1.17]
exB = {k: max(r["lead"] for r in rs) - CD for k, rs in B.items()}
for k, v in {(12, 0): 0.019, (13, 0): 0.049, (14, 0): 0.167, (15, 0): 0.236}.items():
    assert close(exB[k], v, 0.0006)
assert close(corner[(13, 0)], 0.050, 0.0005) and close(corner[(14, 0)], 0.168, 0.0005)
assert close(lsa[2], -0.488, 0.001) and close(lsa[3], 0.272, 0.001)
assert max(lp) < 2.337 and close(max(lp), 2.336, 0.001)
# R: per level, the lattice reading at cell steps and the bands refused at pass steps
REF = {9: 4, 10: 3, 11: 5, 12: 4, 13: 4, 14: 2, 15: 3}
CELLR = {9: (1.965, 2.160), 10: (1.802, 2.151), 11: (1.857, 2.155), 12: (1.793, 2.103),
         13: (1.654, 2.141), 14: (1.664, 2.116), 15: (1.614, 2.118)}
from collections import Counter  # noqa: E402
at = Counter()
for lvl in range(9, 16):
    rows = [r for r in F["R"] if r["level"] == lvl and r["sgn"] > 0]
    assert sum(1 for r in rows if any(n > 0 for n in r["lattice"]["step"]["lead_over"])) == REF[lvl]
    xs = [x for r in rows for x in r["lattice"]["cell"]["lead"]]
    assert close(min(xs), CELLR[lvl][0], 6e-4) and close(max(xs), CELLR[lvl][1], 6e-4)
    for r in rows:
        at.update(a for a, n in zip(r["lattice"]["step"]["at"], r["lattice"]["step"]["lead_over"]) if n > 0)
assert at == Counter({"fwd": 22, "ramp": 14})
# the cost per arm
PER = {0: (64.2, 151.4), 1: (72.1, 169.3), 2: (72.5, 170.2), 3: (72.5, 170.8), 4: (72.1, 170.2)}
for arm, (c0, c1) in PER.items():
    a = sum(r["L_b"] for r in F["A"] if r["sgn"] > 0 and r["arm"] == arm)
    b = sum(r["L_b"] + r["ramp_len"] + r["spare"] for r in F["K"] if r["sgn"] > 0 and r["arm"] == arm)
    assert close(a, c0, 0.05) and close(b, c1, 0.05)
assert close(F["seq"][1], 19.058, 5e-4) and close(F["seq"][-2], 29.996, 5e-4)
assert 73 - 35 == 38
# the rule spacing at levels 11 and 12: over the ceiling by at most 0.0002 mm
for lvl in (11, 12):
    b = build(0, LAW[lvl], rul[lvl], +1, "min")
    ex = exact_band(b, True)["lead"] - CD
    assert 0 < ex <= 0.0002

print("all C13 claims hold")
