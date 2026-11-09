#!/usr/bin/env python3
"""P22-0b: assert each geometry claim of the core rules (no engine import).

Run: python3 -I planning/feeds_contract_2026-11-03/phases/p22_0b_core_geometry.py

A tool that does not cut at its centre has a core of radius c that removes
no wood and blades that stand p (the protrusion) below the core face. Only
the blade ring c <= r <= R cuts, at the tip Z. A cell q under the core
(|q - axis| < c) keeps the top it had when the ring last passed over it.
The core meets wood when the tip has gone down more than p since then.
So for any piece of path that stays inside one disc of radius c around a
cell, the descent along that piece must be at most p ("the dwell rule").

The probe tool: wanaka tool 6, D 18, core 8, protrusion 1. ENTRY_CLEARANCE
is the 2 mm of `dressup::entry_descent` (the legacy ramp leg rule).
"""

import math

D = 18.0
CORE_D = 8.0
C = CORE_D / 2.0
P = 1.0
R = D / 2.0
ENTRY_CLEARANCE = 2.0  # dressup::ENTRY_CLEARANCE (mm)
STEPS_PER_REV = 36  # dressup::entry_descent::HELIX_STEPS_PER_REV


def close(a, b, tol):
    return abs(a - b) <= tol


# ---------------------------------------------------------------------------
# A brute-force dwell meter: walk a sampled 3D path, track per cell the tip Z
# at which the ring last cut it, and return the worst "top minus tip" over
# the cells under the core. Only cells on a line are needed by symmetry.
# ---------------------------------------------------------------------------
def worst_over(path, cells, stock_top):
    worst = -math.inf
    for q in cells:
        top = stock_top
        for (x, y, z) in path:
            d = math.hypot(q[0] - x, q[1] - y)
            if C <= d <= R:
                top = min(top, z)
            elif d < C:
                worst = max(worst, top - z)
    return worst


def resample(points, step):
    out = [points[0]]
    for a, b in zip(points, points[1:]):
        n = max(1, int(math.ceil(math.dist(a[:2], b[:2]) / step)))
        for k in range(1, n + 1):
            t = k / n
            out.append(tuple(a[i] + (b[i] - a[i]) * t for i in range(3)))
    return out


# ---------------------------------------------------------------------------
# Claim 1: at the straight-line limit angle atan(p / d_core) the legacy ramp
# leg ENTRY_CLEARANCE / tan / 2 is d_core x (1 mm / p): 8 mm = d_core on the
# probe tool. A leg of d_core or less leaves a cell at the leg middle that no
# blade passes over (a full-depth post).
# ---------------------------------------------------------------------------
tan_limit = P / CORE_D
leg_legacy = ENTRY_CLEARANCE / tan_limit / 2.0
assert close(leg_legacy, CORE_D, 1e-12), leg_legacy
assert close(leg_legacy, CORE_D * (1.0 / P), 1e-12)


def zigzag(leg, slope, depth, top):
    """Out along +X and back, `leg` per leg, descending `slope` per mm."""
    pts = [(0.0, 0.0, top)]
    z, out = top, True
    while z > top - depth + 1e-12:
        x = leg if out else 0.0
        z = max(top - depth, z - slope * leg)
        pts.append((x, 0.0, z))
        out = not out
    return pts


# Leg = d_core minus a little: the middle cell is never under the ring.
short = resample(zigzag(CORE_D - 0.4, tan_limit, 5.0, 12.0), 0.02)
mid = [((CORE_D - 0.4) / 2.0, 0.0)]
assert worst_over(short, mid, 12.0) > 4.0, "a short leg leaves a post"

# ---------------------------------------------------------------------------
# Claim 2: on a straight zigzag with legs >= d_core a cell at distance b < c
# behind a turn point stays under the core for 2 (b + c) of travel; the
# supremum is 4 c. At the straight-line limit slope p / (2 c) the drop under
# the core reaches 2 p; at p / (4 c) it stays within p.
# ---------------------------------------------------------------------------
for b in (0.0, 1.0, 2.0, 3.0, 3.9):
    dwell = 2.0 * (b + C)
    assert dwell <= 4.0 * C
assert close(2.0 * (3.999999 + C), 4.0 * C, 1e-5)

leg = 2.0 * CORE_D  # well over d_core
cells = [(x * 0.05, 0.0) for x in range(int((leg + C) / 0.05))]
over_limit = worst_over(resample(zigzag(leg, tan_limit, 4.0, 12.0), 0.02), cells, 12.0)
assert over_limit > 1.9 * P, over_limit  # nearly 2 p at the old limit
zig_slope = P / (4.0 * C)
over_zig = worst_over(resample(zigzag(leg, zig_slope, 4.0, 12.0), 0.02), cells, 12.0)
assert over_zig <= P + 0.02, over_zig  # sampling slack of the meter only
# The rule with the least legs: legs of exactly d_core hold too.
over_dcore = worst_over(
    resample(zigzag(CORE_D, zig_slope, 4.0, 12.0), 0.02),
    [(x * 0.05, 0.0) for x in range(int((CORE_D + C) / 0.05))],
    12.0,
)
assert over_dcore <= P + 0.02, over_dcore

# ---------------------------------------------------------------------------
# Claim 3: on a helix of radius r_h >= c a cell stays under the core for an
# arc phi = 2 asin(c / r_h), so the pitch must be at most p 2 pi / phi:
# 2.0 mm/turn at r 4, 3.77 mm/turn at r 5.4. The chord clamp
# slope x chord x 36 gives 3.138 and 4.236, which drop 1.569 and 1.125 mm.
# ---------------------------------------------------------------------------
def phi(r_h):
    return 2.0 * math.asin(min(1.0, C / r_h))


def exact_pitch(r_h):
    return P * 2.0 * math.pi / phi(r_h)


def chord_pitch(r_h):
    chord = 2.0 * r_h * math.sin(math.pi / STEPS_PER_REV)
    return tan_limit * chord * STEPS_PER_REV


assert close(exact_pitch(4.0), 2.0, 1e-12), exact_pitch(4.0)
assert close(exact_pitch(5.4), 3.77, 0.005), exact_pitch(5.4)
assert close(chord_pitch(4.0), 3.138, 0.001), chord_pitch(4.0)
assert close(chord_pitch(5.4), 4.236, 0.001), chord_pitch(5.4)
assert close(chord_pitch(4.0) * phi(4.0) / (2 * math.pi), 1.569, 0.001)
assert close(chord_pitch(5.4) * phi(5.4) / (2 * math.pi), 1.125, 0.001)


def helix(r_h, pitch, depth, top):
    pts, a, z = [(r_h, 0.0, top)], 0.0, top
    step = 2.0 * math.pi / STEPS_PER_REV
    while z > top - depth + 1e-12:
        a += step
        z = max(top - depth, z - pitch / STEPS_PER_REV)
        pts.append((r_h * math.cos(a), r_h * math.sin(a), z))
    for _ in range(STEPS_PER_REV):
        a += step
        pts.append((r_h * math.cos(a), r_h * math.sin(a), top - depth))
    return pts


for r_h in (4.0, 5.4, 7.0, 9.0):
    # The polygon of 36 chords the emitter writes, at the exact pitch.
    path = resample(helix(r_h, exact_pitch(r_h), 5.0, 12.0), 0.02)
    line = [(r_h - C + k * 0.05, 0.0) for k in range(int(2 * C / 0.05) + 1)]
    over = worst_over(path, line, 12.0)
    assert over <= P + 0.03, (r_h, over)
    old = worst_over(resample(helix(r_h, chord_pitch(r_h), 5.0, 12.0), 0.02), line, 12.0)
    if r_h < 7.0:
        assert old > P + 0.1, (r_h, old)

# ---------------------------------------------------------------------------
# Claim 4: for a large r_h the exact pitch tends to the chord rule. phi tends
# to 2 c / r_h, so p 2 pi / phi tends to (p / 2c) 2 pi r_h, the limit slope
# times the circumference. The chord rule is that times 36 sin(pi/36)/pi
# (0.99873): the ratio tends to pi / (36 sin(pi / 36)).
# ---------------------------------------------------------------------------
ratio_limit = math.pi / (STEPS_PER_REV * math.sin(math.pi / STEPS_PER_REV))
for r_h, tol in ((100.0, 1e-3), (1000.0, 1e-5), (1e5, 1e-9)):
    assert close(exact_pitch(r_h) / chord_pitch(r_h), ratio_limit, tol)
assert close(ratio_limit, 1.00127, 1e-5)

# The emitted bound is min(arc, chord): the arc binds for c <= r up to the
# radius where a chord is 2 c long (r = c / sin(pi / 36) = 45.9 mm); the
# chord bound binds under c (a post stands there at any pitch anyway) and
# beyond that radius, by at most the 0.127 % of the ratio above.
def emitted_pitch(r_h):
    return min(exact_pitch(r_h), chord_pitch(r_h))


assert emitted_pitch(4.0) == exact_pitch(4.0)
assert emitted_pitch(5.4) == exact_pitch(5.4)
assert emitted_pitch(9.0) == exact_pitch(9.0)
assert emitted_pitch(2.0) == chord_pitch(2.0)
assert emitted_pitch(1000.0) == chord_pitch(1000.0)
crossover = C / math.sin(math.pi / STEPS_PER_REV)
assert close(crossover, 45.89, 0.01), crossover
lo, hi = 10.0, 1000.0
for _ in range(80):
    mid_r = (lo + hi) / 2.0
    lo, hi = (mid_r, hi) if exact_pitch(mid_r) < chord_pitch(mid_r) else (lo, mid_r)
assert close(lo, crossover, 1e-9), (lo, crossover)  # the bounds meet where a chord is 2 c

# ---------------------------------------------------------------------------
# Claim 5 (the chosen ramp rule): option (ii), a turn of radius rho >= c at
# each reversal, holds the slope p / (2 rho asin(c / rho)) (pi c at rho = c),
# so its path per mm of descent is pi c / p against 4 c / p for option (i):
# (ii) saves 1 - pi / 4 = 21.5 % of the ramp path but needs 2 rho = d_core of
# side room. Option (i) keeps the straight zigzag.
# ---------------------------------------------------------------------------
assert close((math.pi * C) / (4.0 * C), math.pi / 4.0, 1e-12)
assert close(1.0 - math.pi / 4.0, 0.2146, 1e-4)
assert close(2.0 * C * math.asin(1.0), math.pi * C, 1e-12)

print("p22_0b_core_geometry: all claims hold")
print(f"  legacy leg at the limit = {leg_legacy:.3f} mm = d_core x (1 mm / p)")
print(f"  zigzag at the old limit: worst {over_limit:.3f} mm over the tip (p = {P})")
print(f"  zigzag at p / (4c) = {zig_slope:.5f}: worst {over_zig:.3f} mm")
print(f"  exact pitch r 4 = {exact_pitch(4.0):.3f}, r 5.4 = {exact_pitch(5.4):.3f} mm/turn")
print(f"  exact / chord ratio limit = {ratio_limit:.5f}")
