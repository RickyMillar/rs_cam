"""C32's second load measure, for any rough op (phase3/c32/c32.py, generalised).

The samples: every cutting sample of the toolpath whose intent is clearing_cut
(the D-FIX6 rule) that the simulation reads over 0.471, in every phase.

For each such sample, the exact swept width (C18 exact_width_at, as c32.py
`width`) on the sample path densified to 0.1 mm, a lattice of 0.05 mm, where
a disc of R is stamped at every earlier path point whose Z is at most the
sample Z + 0.5 mm (one planner cell, max(R/6, tolerance) = 0.5 here). So only
stock standing at least one planner cell above the tip counts as uncut: a
full-width skim of a thinner layer reads under the bound.

Returns the XY length of the samples whose thick width is over 0.471, per
phase (producer_split.phases) and in total.
"""
import collections
import glob
import json
import math
import os
import re
import sys

import numpy as np
from scipy.spatial import cKDTree

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from producer_split import phases  # noqa: E402

R = 3.0
PITCH = 0.05
STEP = 0.1
BOUND = 0.471
THICK = 0.5   # one planner cell: max(R / 6, tolerance 0.1)
BIN = 9.0


def thick_over(run_dir, simart_dir, tp_glob):
    tpf = sorted(glob.glob(os.path.join(run_dir, tp_glob)))[0]
    tp = json.load(open(tpf))
    tpid = int(re.search(r"tp_(\d+)_", os.path.basename(tpf)).group(1))
    phase, slice_arm = phases(tp)
    S = [x for x in json.load(open(os.path.join(simart_dir, "simulation.json")))["trace"]["samples"] if x["toolpath_id"] == tpid]
    P = np.array([x["position"] for x in S])
    pts = [P[0:1]]
    seq = [np.array([0])]
    for k in range(1, len(P)):
        a, b = P[k - 1], P[k]
        n = max(1, int(math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / STEP)))
        t = (np.arange(1, n + 1) / n)[:, None]
        pts.append(a + t * (b - a))
        seq.append(np.full(n, k))
    DP = np.vstack(pts)
    DS = np.concatenate(seq)
    keys = collections.defaultdict(list)
    cx = np.floor(DP[:, 0] / BIN).astype(int)
    cy = np.floor(DP[:, 1] / BIN).astype(int)
    for i in range(len(DP)):
        keys[(cx[i], cy[i])].append(i)
    keys = {k: np.array(v) for k, v in keys.items()}
    g = np.arange(-R, R + PITCH / 2, PITCH)
    GX, GY = np.meshgrid(g, g)
    ins = GX ** 2 + GY ** 2 <= R * R
    GX, GY = GX[ins], GY[ins]

    def width(k, zmax):
        frm, to = P[k - 1, :2], P[k, :2]
        mid = (frm + to) / 2
        dv = to - frm
        l = max(np.hypot(*dv), 1e-9)
        sx, sy = -dv[1] / l, dv[0] / l
        i0, j0 = int(math.floor(mid[0] / BIN)), int(math.floor(mid[1] / BIN))
        cand = [keys[(i, j)] for i in (i0 - 1, i0, i0 + 1) for j in (j0 - 1, j0, j0 + 1) if (i, j) in keys]
        L = np.column_stack([mid[0] + GX, mid[1] + GY])
        unc = np.ones(len(L), bool)
        if cand:
            c = np.concatenate(cand)
            c = c[(DS[c] <= k - 1) & (DP[c, 2] <= zmax)]
            if len(c):
                dist, _ = cKDTree(DP[c, :2]).query(L, distance_upper_bound=R + 1e-12)
                unc = ~(dist <= R)
        if not unc.any():
            return 0.0
        u = GX[unc] * sx + GY[unc] * sy
        return (u.max() - u.min()) / (2 * R)

    out = collections.Counter()
    prev = None
    for k, x in enumerate(S):
        p = x["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev is not None else 0.0
        prev = p
        if k == 0 or not x["is_cutting"] or x["source_intent"] != "clearing_cut":
            continue
        if x["engagement"]["radial_woc_fraction"] <= BOUND:
            continue
        if width(k, p[2] + THICK) > BOUND:
            m = x["move_index"]
            out[(phase[m] if m < len(phase) else None) or "unplaced"] += ln
    total = sum(out.values())
    return {"total_mm": round(total, 1),
            "perimeter": round(out["perimeter"], 1) if slice_arm else None,
            "body": round(out["body"], 1), "waterline": round(out["waterline"], 1),
            "unplaced": round(out["unplaced"], 1)}


if __name__ == "__main__":
    print(json.dumps(thick_over(sys.argv[1], sys.argv[2], sys.argv[3])))
