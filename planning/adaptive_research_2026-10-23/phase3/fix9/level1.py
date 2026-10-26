"""C18/fix9: the Corne stock after the FIRST level (Z 17) of '3D Rough 1',
rebuilt as stockmap.py does but from the samples of that level only (the
moves before the level-2 annotation, mapped to final moves by the pass
entries), and read in rings round the D20 seed (the first plunge point)."""
import json, math, os, re, sys
import numpy as np
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
R, g = 3.0, 0.25
COLS = {"contour_parallel": "fix6/corne/contour_parallel", "zigzag_pick D18": "fix5/corne/geodesic_local_zigzag_pick",
        "zigzag_pick D20": "fix7/corne/geodesic_local_zigzag_pick", "AgentSearch": "fix2/simart/corne/agent"}
TP = {"AgentSearch": "fix1/corne/agent"}


def level2_start(tp):
    ents = sorted([i for i in tp["semantic_trace"]["items"] if i["kind"] == "entry"], key=lambda i: i["move_start"])
    gen = {}
    for a in tp["debug_trace"]["annotations"]:
        m = re.match(r"Pass (\d+) — ", a["label"])
        if m: gen[int(m.group(1))] = a["move_index"]
    fin = sorted((gen[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    lv = [a["move_index"] for a in tp["debug_trace"]["annotations"] if a["label"].startswith("Adaptive Z")]
    return next(f for gi, f in fin if gi >= lv[1])


def stock_after(simdir, last_move):
    sim = json.load(open(os.path.join(P3, simdir, "simulation.json")))
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
    top = np.full((ny, nx), float(hi[2]))
    P = np.array([s["position"] for s in sim["trace"]["samples"] if s["toolpath_id"] == 1 and s["move_index"] < last_move])
    seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
    idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
    D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int)
    rr = int(math.ceil(R / g)) + 1
    for dj in range(-rr, rr + 1):
        for di in range(-rr, rr + 1):
            I, J = ci + di, cj + dj
            m = ((lo[0] + (I + 0.5) * g - D[:, 0]) ** 2 + (lo[1] + (J + 0.5) * g - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
    return top, lo[0] + (np.arange(nx) + 0.5) * g, lo[1] + (np.arange(ny) + 0.5) * g


if __name__ == "__main__":
    out = {}
    for col, d in COLS.items():
        tp = json.load(open(os.path.join(P3, TP.get(col, d), "tp_1_3D_Rough_1.json")))
        l2 = level2_start(tp)
        top, xs, ys = stock_after(d, l2)
        full = np.load(os.path.join(H, f"stock_corne_{col.replace(' ', '_')}.npz"))
        floor17 = np.maximum(full["target"], 17.0)          # what level Z 17 may cut down to
        X, Y = np.meshgrid(xs, ys); sil = ndimage.binary_fill_holes(full["surf"] > 0)
        cx, cy = 55.866, -49.762; r = np.hypot(X - cx, Y - cy)
        rows = []
        for r0, r1 in ((0, 25), (25, 50)):
            b = (r >= r0) & (r < r1) & sil
            rows.append({"r": [r0, r1], "cells": int(b.sum()), "share_cut_to_level": round(float(np.mean(top[b] <= floor17[b] + 0.8)), 3),
                         "share_untouched": round(float(np.mean(top[b] >= 23.0 - 1e-9)), 3)})
        out[col] = {"level2_first_move": l2, "rings": rows}
        print(col, l2, rows)
    json.dump(out, open(os.path.join(H, "level1_rings.json"), "w"), indent=1)
