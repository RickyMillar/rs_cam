"""C18: assert each number the note P3_C18_3d_oracle.md derives.

Run: python3 -I assert_c18.py   (numpy; about one minute)

Inputs are the C12 fixture values (tests/adaptive3d_geodesic_arm_c12.rs) and
source constants: ASSUMED_RAPID_MM_MIN 5000 and RAPID_DESCENT_BUFFER_MM 0.5
(adaptive3d/clearing.rs, path.rs), STARTER_STEPS_PER_TURN 64 and
starter_spiral_points (adaptive/path.rs), the ceiling 0.3626 at s 2, R 3
(adaptive/CLAUDE.md). The lattice reading copies exact_width_at of
a_clearing_cut_holds_the_pass_load_g_adaptpassload (pitch 0.05 mm).
"""
import math

import numpy as np

R, S, TOL, SIM_CELL = 3.0, 2.0, 0.1, 0.5
D = 2 * R
CELL = max(R / 6, TOL)
PITCH = SIM_CELL / 10
CEILING = 0.3626  # adaptive/CLAUDE.md

# 1. The bounds.
sim_bound = CEILING + (CELL + SIM_CELL + TOL) / D
exact_bound = CEILING + (CELL + TOL + PITCH) / D
assert abs(sim_bound - 0.5459) < 1e-3, sim_bound
assert abs(exact_bound - 0.4709) < 1e-3, exact_bound

# 2. The air-run crossover of the 3D lifter (air_run_crossover_mm).
def crossover(feed, plunge, depth, rapid=5000.0, buf=0.5):
    f, r, p = feed / 60, rapid / 60, plunge / 60
    over = depth / r + max(depth - buf, 0) / r + min(buf, depth) / p
    return over / (1 / f - 1 / r)

x7 = crossover(1000, 500, 15 - 7)
x45 = crossover(1000, 500, 15 - 4.5)
assert abs(x7 - 5.125) < 1e-9 and abs(x45 - 6.375) < 1e-9, (x7, x45)

# 3. The 2D starter path (starter_spiral_points) at pitch p.
def spiral(p):
    steps = 64
    dth = 2 * math.pi / steps
    th_end = 2 * math.pi * R / p
    pts = [(0.0, 0.0)]
    i = 1
    while i * dth < th_end:
        th = i * dth
        r = p * th / (2 * math.pi)
        pts.append((r * math.cos(th), r * math.sin(th)))
        i += 1
    lap_start = len(pts)
    for k in range(steps + 1):
        th = th_end + k * dth
        pts.append((R * math.cos(th), R * math.sin(th)))
    return pts, lap_start

pts, lap_start = spiral(S)
# The air run: the spiral points whose centre lies in the plunge hole.
# The plunge hole of the 3D stamp ends near r = 2.67 (D11: the re-plunge,
# move 57, lies 2.67 mm from the medial point).
air = [q for q in pts if math.hypot(*q) < 2.67]
air_len = sum(math.dist(a, b) for a, b in zip(air, air[1:]))
assert air_len > x45 > x7, air_len
assert 10.0 < air_len < 12.5, air_len

# 4. The historical starter: its air run is the arc within R of its plunge.
assert math.pi * R / 3 < x7

# 5. Analytic: a cutter on the circle of radius R round a cleared disc of
#    radius R meets uncut points from -R/2 to +R across its path.
assert abs((R + R / 2) / D - 0.75) < 1e-12

# 6. The lattice readings.
def dense(path):
    out = []
    for a, b in zip(path, path[1:]):
        n = max(1, math.ceil(math.dist(a, b) * 2 / PITCH))
        out += [(a[0] + t / n * (b[0] - a[0]), a[1] + t / n * (b[1] - a[1])) for t in range(n)]
    out.append(path[-1])
    return np.array(out)

def exact(prev, frm, to):
    half = 2 * R
    n = int(2 * half / PITCH) + 1
    xs = to[0] - half + np.arange(n) * PITCH
    ys = to[1] - half + np.arange(n) * PITCH
    X, Y = np.meshgrid(xs, ys)
    cut = np.zeros_like(X, bool)
    near = prev[(np.abs(prev[:, 0] - to[0]) <= half + R) & (np.abs(prev[:, 1] - to[1]) <= half + R)]
    for px, py in near:
        cut |= (X - px) ** 2 + (Y - py) ** 2 <= R * R
    d = np.subtract(to, frm)
    l = max(np.hypot(*d), 1e-9)
    sx, sy = -d[1] / l, d[0] / l
    mid = (np.add(frm, to)) / 2
    m = (~cut) & ((X - mid[0]) ** 2 + (Y - mid[1]) ** 2 <= R * R)
    if not m.any():
        return 0.0
    u = (X[m] - mid[0]) * sx + (Y[m] - mid[1]) * sy
    return (u.max() - u.min()) / D

def read_lap(before, lap, hole, step=SIM_CELL):
    pre, lp = dense(before), dense(lap)
    idx, acc = [0], 0.0
    for i in range(1, len(lp)):
        acc += np.hypot(*(lp[i] - lp[i - 1]))
        if acc >= step:
            idx.append(i)
            acc = 0.0
    out = []
    for a, b in zip(idx, idx[1:]):
        prev = np.vstack([np.array([hole]), pre, lp[: a + 1]])[::2]
        out.append(exact(prev, tuple(lp[a]), tuple(lp[b])))
    return out

# 6a. The demoted starter (the 3D emission now): the plunge hole at the
#     medial point, the spiral from r 2.67 on, then the lap.
cut = [q for q in pts if math.hypot(*q) >= 2.67]
k = next(i for i, q in enumerate(cut) if abs(math.hypot(*q) - R) < 1e-9)
v = read_lap(cut[: k + 1], cut[k:], (0.0, 0.0))
assert 0.70 < np.median(v) < 0.76 and 0.70 < max(v) < 0.80, (np.median(v), max(v))

# 6b. The planned starter (the fix): the whole spiral, then the lap.
for p, lo, hi in ((2.0, 0.47, 0.48), (1.0, 0.25, 0.28)):
    pp, ls = spiral(p)
    # The lap from its first point at r = R: the step into it starts at
    # r < R, inside the plunge hole, so the 2D rule exempts it.
    v = read_lap(pp[: ls + 1], pp[ls:], (0.0, 0.0))
    assert lo < max(v) < hi, (p, max(v))
    if p == 2.0:
        # Only the first lap sample is over the exact bound.
        assert sum(w > exact_bound for w in v) == 1 and v[0] > exact_bound, v[:3]
        assert max(v[1:]) < exact_bound, max(v[1:])

# 6c. The historical starter of the shipped AgentSearch slice: plunge at
#     the rim (R, 0), one lap of radius R round the medial point.
circ = [(R * math.cos(2 * math.pi * i / 64), R * math.sin(2 * math.pi * i / 64)) for i in range(65)]
v = read_lap(circ[:1], circ, (R, 0.0))
assert max(v) > 0.99 and np.median(v) > 0.98, (max(v), np.median(v))

print("C18: all claims hold")
