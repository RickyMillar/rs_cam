"""C33: the step layer of the outline wrap at Z 17. For each thick-over body
sample whose same-level reading is <= 0.471 (a step), the lowest earlier
sample Z within R that stands more than 0.5 mm above the tip (and below the
level + 6 mm), minus the tip Z. Prints the length-unweighted median, p10
and p90 per level. Needs c33_rows.pkl (c33.py)."""
import json, os, pickle
import numpy as np
H = os.path.dirname(os.path.abspath(__file__))
D = pickle.load(open(os.path.join(H, "c33_rows.pkl"), "rb")); rows = D["rows"]
S = [x for x in json.load(open(os.path.join(H, "..", "fix7", "corne", "geodesic_local_zigzag_pick", "simulation.json")))["trace"]["samples"] if x["toolpath_id"] == 1]
P = np.array([x["position"] for x in S])
for lz in (17.0,):
    A = [r for r in rows if r["thick"] > 0.471 and r["loose"] <= 0.471 and r["lz"] == lz]
    th = []
    for r in A:
        k = r["k"]; q = P[k]; Q = P[:k]
        sel = (np.hypot(Q[:, 0] - q[0], Q[:, 1] - q[1]) <= 3.0) & (Q[:, 2] > q[2] + 0.5) & (Q[:, 2] < lz + 6)
        if sel.any(): th.append(Q[sel, 2].min() - q[2])
    th = np.array(th)
    print(f"layer Z {lz}: samples {len(A)}, with a higher pass within R {len(th)}; median {np.median(th):.2f} mm, p10 {np.percentile(th, 10):.2f}, p90 {np.percentile(th, 90):.2f}; tip Z median {np.median([r['p'][2] for r in A]):.2f}")
