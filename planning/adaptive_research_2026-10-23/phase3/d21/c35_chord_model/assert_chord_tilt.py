"""C35 D21b: assert the chord-tilt claims of P3_C22_3d_turns.md section 12.7.
Run: python3 -I assert_chord_tilt.py (about 1 min). Exact circles about the
seed, the replay's swept-width step reading (spiral_replay.py)."""
import sys, os, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import spiral_replay as m
R, D, s, tol, c, cD = m.R, m.D, m.s, m.tol, m.c, m.cD
def law_levels(r0, rK, width):
    out = [r0]
    while out[-1] < rK - 1e-9:
        p = out[-1]
        n = -(R - width) + math.sqrt((R - width) ** 2 + (p + R) ** 2 - R ** 2)
        out.append(min(n, p + s, rK))
    return out
def spiral(lv, per):
    K = len(lv) - 1; d = 2 * math.pi / per; pts = []
    for j in range(per * (K + 1) + 1):
        th = j * d; turns = th / (2 * math.pi)
        if turns >= K: r = lv[K]
        else:
            n = int(turns); r = lv[n] + (turns - n) * (lv[n + 1] - lv[n])
        pts.append((r * math.cos(th), r * math.sin(th)))
    return pts
def run(cell, pts, lv, ox=0.0, oy=0.0):
    g = m.Grid(cell, lv[-1] + R + 1, ox, oy); g.clear_circle(0, 0, lv[0] + R)
    sp = m.subdivide(pts, 1.5); g.clear_circle(*sp[0], R); ov = 0; mx = 0.0
    for a, b in zip(sp, sp[1:]):
        w = g.step(a, b); mx = max(mx, w); ov += w > c; g.clear_segment(a, b, R)
    return ov, mx
PHASES = ((0, 0), (0.17, 0.31), (0.25, 0.25), (0.41, 0.07), (0.33, 0.12))
law = m.levels(3.0, 4); rK = law[-1]
# Claim 1: the law has no margin; the excess is the chord, linear in the step.
ex = {}
for per in (22, 88, 352):
    ex[per] = run(0.05, spiral(law, per), law)[1] - c
    print(f"law levels, {per} per turn: fine-grid max excess {ex[per]:.4f}")
assert ex[22] > 0.05 and ex[88] > 0 and ex[352] > 0
assert 3.5 < ex[22] / ex[88] < 5.5 and 3.5 < ex[88] / ex[352] < 5.5
# Claim 2: today's spiral (chord rule, 22 per turn) reads over on the planner lattice at every phase.
for ox, oy in PHASES:
    ov, mx = run(0.5, spiral(law, 22), law, ox, oy)
    print(f"now, lattice phase ({ox},{oy}): overs {ov} max {mx:.4f}")
    assert ov > 0
# Claim 3: the held spiral reads under the ceiling at every phase and on the fine grid.
h = math.sqrt(R * R - (R - s) ** 2)
beta = math.atan2(R, h) + math.asin((tol - R) / math.hypot(h, R))
per = max(math.ceil(math.pi / math.acos(1 - tol / rK)), math.ceil(math.pi / beta))
st = math.pi / per
width = cD - R * (1 - math.cos(st)) - h * math.sin(st) - rK * (1 - math.cos(st)) - tol
lv = law_levels(3.0, rK, width)
print(f"held: beta {beta:.4f}, {per} per turn, width {width:.4f}, turns {len(lv)-1} (law: {len(law)-1})")
assert per == 91 and len(lv) - 1 == 5
for ox, oy in PHASES:
    ov, mx = run(0.5, spiral(lv, per), lv, ox, oy)
    print(f"held, lattice phase ({ox},{oy}): overs {ov} max {mx:.4f}")
    assert ov == 0
ov, mx = run(0.05, spiral(lv, per), lv)
print(f"held, fine grid: overs {ov} max {mx:.4f}")
assert ov == 0
print("all claims hold")
