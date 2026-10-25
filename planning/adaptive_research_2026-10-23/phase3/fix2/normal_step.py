"""C24's spacing measure, for every arm: the step along the normal between
adjacent passes at one level.

- Passes: the cut moves (fed, XY travel, not in a keep-down-link span) of
  the first rough op at the level that the closing simulation reads as
  ENGAGED (some sample of the move has radial width of cut over one sim
  cell / D, the simulation's resolution). A straight
  move across the cut runs through cleared stock and reads 0, so it is not
  a pass (C24: the one-way cross moves).
- The starter helix is left out: moves with an end within R + 0.05 mm of
  the starter centre (C24 used r > 3.05).
- From the midpoint of each pass move, the normal is cast both ways. The
  distance to the nearest crossing of another pass move (not the move
  itself and not its sequence neighbours) is one sample, weighted by the
  move's length. A side with no crossing within one tool diameter is set
  apart (the 3 x 3 grid of one-diameter cells around the midpoint holds
  every segment that can cross within one diameter).
Returns the weighted median and the weighted 10th and 90th percentiles.
"""
import json
import math
import os
from collections import defaultdict

TOOL_D = 6.0
R = 3.0


def engaged_moves(simart_dir, toolpath_id):
    """Moves with a sample over one sim cell / D: a reading at or under one
    sim cell is under the simulation's resolution (a cusp, a sliver)."""
    sim = json.load(open(os.path.join(simart_dir, "simulation.json")))
    floor = sim["resolution_mm"] / TOOL_D
    eng = set()
    for sm in sim["trace"]["samples"]:
        if sm["toolpath_id"] == toolpath_id and sm["is_cutting"] and sm["engagement"]["radial_woc_fraction"] > floor:
            eng.add(sm["move_index"])
    return eng


def wq(pairs, q):
    pairs = sorted(pairs)
    tot = sum(w for _, w in pairs)
    acc = 0.0
    for v, w in pairs:
        acc += w
        if acc >= q * tot:
            return v
    return pairs[-1][0]


def normal_step(d, lab, centre, engaged):
    segs = []
    for i, ((g, a, b), c) in enumerate(zip(d["moves"], d["cls"])):
        if c != "cut" or d["lev"][i] != lab or i not in engaged:
            continue
        if math.hypot(b[0] - a[0], b[1] - a[1]) < 1e-6:
            continue
        if min(math.hypot(a[0] - centre[0], a[1] - centre[1]), math.hypot(b[0] - centre[0], b[1] - centre[1])) <= R + 0.05:
            continue
        segs.append((i, a, b))
    cell = TOOL_D
    grid = defaultdict(list)
    for k, (i, a, b) in enumerate(segs):
        for gx in range(int(math.floor(min(a[0], b[0]) / cell)), int(math.floor(max(a[0], b[0]) / cell)) + 1):
            for gy in range(int(math.floor(min(a[1], b[1]) / cell)), int(math.floor(max(a[1], b[1]) / cell)) + 1):
                grid[(gx, gy)].append(k)
    samples, apart = [], 0
    for k, (i, a, b) in enumerate(segs):
        L = math.hypot(b[0] - a[0], b[1] - a[1])
        tx, ty = (b[0] - a[0]) / L, (b[1] - a[1]) / L
        mx, my = 0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])
        cand = set()
        gx0, gy0 = int(math.floor(mx / cell)), int(math.floor(my / cell))
        for gx in range(gx0 - 1, gx0 + 2):
            for gy in range(gy0 - 1, gy0 + 2):
                cand.update(grid.get((gx, gy), ()))
        for nx, ny in ((-ty, tx), (ty, -tx)):
            best = None
            for kk in cand:
                j, p, q = segs[kk]
                if abs(j - i) <= 1:
                    continue
                ex, ey = q[0] - p[0], q[1] - p[1]
                den = nx * ey - ny * ex
                if abs(den) < 1e-12:
                    continue
                wx, wy = p[0] - mx, p[1] - my
                t = (wx * ey - wy * ex) / den
                u = (wx * ny - wy * nx) / den
                if 0.05 < t <= TOOL_D and 0 <= u <= 1 and (best is None or t < best):
                    best = t
            if best is None:
                apart += 1
            else:
                samples.append((best, L))
    if not samples:
        return {"n": 0, "apart": apart, "median": None, "p10": None, "p90": None}
    return {"n": len(samples), "apart": apart, "median": round(wq(samples, 0.5), 3),
            "p10": round(wq(samples, 0.1), 3), "p90": round(wq(samples, 0.9), 3)}
