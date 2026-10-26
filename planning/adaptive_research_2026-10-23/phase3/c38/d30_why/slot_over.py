"""C38 §15.6, step 2: D30 op 1's "slot moves" class of fix21/load_split.py
(clearing samples over 0.471 whose sample feed is below the base feed 4000
and whose move is not a capped turn move). For each such sample: its
program feed, its sample feed, the ratio to op 1's commanded feed
F_OP = 1518.75 (the job file, line 129), the sim
radial, and where it lies (Z, distance outside the silhouette). Writes
slot_over.json. Read only."""
import collections, json, math, runpy, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; FIX = H.parent.parent / "fix21"
sys.argv = ["feed_check.py", "fix21/a_only/corne", "3D Rough 1", "fix21/a_only/corne", "corne"]
g = runpy.run_path(str(FIX / "feed_check.py"))
S, turns, pf, wall = g["S"], set(g["moves"]), g["pf"], g["wall"]
F = 4000.0   # the base feed of the class rule
F_OP = 1518.75
prev = None; rows = []
for s in S:
    p = s["position"]; ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0; prev = p
    if not (s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["engagement"]["radial_woc_fraction"] > 0.471):
        continue
    k = s["move_index"]
    if k in turns or s["feed_rate_mm_min"] >= F - 0.5:
        continue
    rows.append((k, ln, pf[k], s["feed_rate_mm_min"], s["engagement"]["radial_woc_fraction"], p))
pos = np.array([r[5] for r in rows], float); w = wall(pos)
L = np.array([r[1] for r in rows]); pfa = np.array([r[2] for r in rows]); sf = np.array([r[3] for r in rows]); rad = np.array([r[4] for r in rows])
ratio = sf / F_OP
slot = ~np.array([abs(v - 750.0) < 0.5 for v in sf])   # 750: the plunge rate (descents)
vertical = np.array([abs(sf_ - 750.0) < 0.5 for sf_ in sf])
out = {
    "samples": len(rows), "length_mm": round(float(L.sum()), 1), "moves": len({r[0] for r in rows}),
    "program_feed_equals_sample_feed_mm": round(float(L[np.abs(pfa - sf) <= 0.5].sum()), 1),
    "program_feed_above_sample_feed_mm": round(float(L[pfa > sf + 0.5].sum()), 1),
    "at_plunge_rate_750_mm": round(float(L[vertical].sum()), 1),
    "not_750_mm": round(float(L[slot].sum()), 1),
    "not_750_ratio_to_F_op_p0_p10_p50_p90_p100": [round(float(v), 3) for v in np.quantile(ratio[slot], [0, .1, .5, .9, 1])],
    "not_750_in_slot_factor_range_mm": round(float(L[slot & (ratio >= 0.166) & (ratio < 1.0)].sum()), 1),
    "not_750_c_over_sim_radial_p10_p50_p90": [round(float(v), 3) for v in np.quantile(0.2189 / rad[slot], [.1, .5, .9])],
    "not_750_ratio_minus_c_over_sim_radial_p10_p50_p90": [round(float(v), 3) for v in np.quantile(ratio[slot] - 0.2189 / rad[slot], [.1, .5, .9])],
    "not_750_length_by_wall_band_mm": {b: round(float(L[slot & m].sum()), 1) for b, m in {
        "inside part": w <= 0, "0-3 outside": (w > 0) & (w <= 3), "3-6.2 outside (strip)": (w > 3) & (w <= 6.2), ">6.2 outside": w > 6.2}.items()},
    "not_750_length_at_strip_levels_mm": round(float(L[slot & np.isin(np.round(pos[:, 2], 2), [11.0, 5.0, 0.5])].sum()), 1),
    "wall_mm_p0_p50_p100": [round(float(v), 2) for v in np.quantile(w, [0, .5, 1])],
    "length_by_wall_band_mm": {b: round(float(L[m].sum()), 1) for b, m in {
        "inside part": w <= 0, "0-3 outside": (w > 0) & (w <= 3), "3-6.2 outside (strip)": (w > 3) & (w <= 6.2), ">6.2 outside": w > 6.2}.items()},
}
(H / "slot_over.json").write_text(json.dumps(out, indent=1))
print(json.dumps(out, indent=1))
