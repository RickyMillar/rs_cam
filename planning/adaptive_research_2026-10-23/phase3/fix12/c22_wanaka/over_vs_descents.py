"""C22 §12.8 Q2 test: is the C35 over-load at Z 8.62 where D20 lifted over and C35 stays down?
D20-only descents: D20 L3 descents (r < 105) with no C35 L3 descent within 3 mm (R).
Share of each column's L3 over-0.471 length (r < 105) within 3 mm of a D20-only descent."""
import sys, os, math, json, bisect
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import probe as pr
R_TOOL = 3.0; EXACT = 0.471
def level3(d):
    sim = json.load(open(os.path.join(pr.P3, d, "simulation.json")))
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    tp = json.load(open(os.path.join(pr.P3, d, [f for f in os.listdir(os.path.join(pr.P3, d)) if f.startswith("tp_1_")][0])))
    ent = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"])
    pl = []
    for a, b, st in ent:
        if st != "plunge entry": continue
        ss = [s for s in S if a <= s["move_index"] <= b]; pl.append((a, ss[-1]["position"]))
    seeds = [a for a, p in pl if math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1]) < 0.01]
    l4 = next(a for a, p in pl if abs(p[0]-122.0) < 0.01 and abs(p[1]-31.18) < 0.01)
    inner = lambda p: math.hypot(p[0]-pr.SEED[0], p[1]-pr.SEED[1]) < 105
    D = np.array([p[:2] for m, p in pr.descents(S, ent) if seeds[2] <= m < l4 and inner(p)])
    over = []; prev = None; cut = 0.0
    for sm in sim["trace"]["samples"]:
        p = sm["position"]; ln = math.hypot(p[0]-prev[0], p[1]-prev[1]) if prev else 0.0; prev = p
        if sm["toolpath_id"] != 1 or not sm["is_cutting"] or sm["source_intent"] != "clearing_cut": continue
        if not (seeds[2] <= sm["move_index"] < l4) or not inner(p): continue
        if sm["engagement"]["radial_woc_fraction"] > EXACT: over.append((p[0], p[1], ln))
    return D, np.array(over)
D20d, D20o = level3(pr.COLS["D20"]); C35d, C35o = level3(pr.COLS["C35"])
def near(pts, ref, r):
    if len(ref) == 0: return np.zeros(len(pts), bool)
    out = np.zeros(len(pts), bool)
    for i in range(0, len(pts), 2000):
        d = np.hypot(pts[i:i+2000, None, 0] - ref[None, :, 0], pts[i:i+2000, None, 1] - ref[None, :, 1])
        out[i:i+2000] = (d <= r).any(1)
    return out
only = D20d[~near(D20d, C35d, R_TOOL)]
print("L3 r<105 descents: D20", len(D20d), "C35", len(C35d), "D20-only (no C35 descent within R)", len(only))
res = {"descents": {"D20": len(D20d), "C35": len(C35d), "D20_only": len(only)}}
for lab, o in (("D20", D20o), ("C35", C35o)):
    m = near(o[:, :2], only, R_TOOL)
    res[lab] = {"over_mm": round(float(o[:, 2].sum()), 1), "near_d20_only_mm": round(float(o[m, 2].sum()), 1)}
    print(f"{lab}: over 0.471 length {o[:,2].sum():.1f} mm; within R of a D20-only descent {o[m,2].sum():.1f} mm ({100*o[m,2].sum()/o[:,2].sum():.1f} %)")
json.dump(res, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "over_vs_descents.json"), "w"), indent=1)
