"""C22 §16.9: where Part A's new over-load and turns lie. The D26 op-1 large-Z entries
(C40 lines, cause LargeZ/LargeZHeld) mark the steps Part A now cuts at feed. For D26
(fix16 corne_pick, wanaka200_pick) and D29 a_only (fix20): the op-1 clearing length over
0.471 and the planar move-junction turns over 30 deg, within R + one sim cell of such a step
or not."""
import json, os, re, math, collections
import numpy as np
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..", "..")
R = 3.0
RUNS = {"corne": ("fix16/corne_pick", "fix20/a_only/corne", 0.3), "wanaka200": ("fix16/wanaka200_pick", "fix20/a_only/wanaka200", 1.0)}
def steps(d):
    pts = []
    for ln in open(os.path.join(P3, d, "stdout_plain.txt")):
        if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
            kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1]))
            pts.append((float(kv["x"]), float(kv["y"])))
    return np.array(pts)
def near(xy, ref, r):
    if len(ref) == 0 or len(xy) == 0: return np.zeros(len(xy), bool)
    out = np.zeros(len(xy), bool)
    for i in range(0, len(xy), 4000):
        d = np.hypot(xy[i:i+4000, None, 0] - ref[None, :, 0], xy[i:i+4000, None, 1] - ref[None, :, 1])
        out[i:i+4000] = (d <= r).any(1)
    return out
OUT = {}
for fx, (d26, d29, cell) in RUNS.items():
    ref = steps(d26)
    for col, d in (("D26", d26), ("D29", d29)):
        sim = json.load(open(os.path.join(P3, d, "simulation.json")))
        S = sim["trace"]["samples"]
        prev = None; ov = []; last = {}
        for s in S:
            p = s["position"]; ln = math.hypot(p[0]-prev[0], p[1]-prev[1]) if prev else 0.0; prev = p
            if s["toolpath_id"] != 1: continue
            last[s["move_index"]] = s
            if s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["engagement"]["radial_woc_fraction"] > 0.471:
                ov.append((p[0], p[1], ln))
        ov = np.array(ov) if ov else np.zeros((0, 3))
        m = near(ov[:, :2], ref, R + cell)
        ms = sorted(last); tpts = []; pd = None
        for a, b in zip(ms, ms[1:]):
            pa, pb = last[a]["position"], last[b]["position"]
            dx, dy = pb[0]-pa[0], pb[1]-pa[1]
            if math.hypot(dx, dy) < 1e-6: pd = None; continue
            dd = math.atan2(dy, dx)
            if pd is not None and abs((dd-pd+math.pi) % (2*math.pi) - math.pi) > math.radians(30): tpts.append((pa[0], pa[1]))
            pd = dd
        tpts = np.array(tpts) if tpts else np.zeros((0, 2))
        tm = near(tpts, ref, R + cell)
        OUT[f"{fx} {col}"] = {"steps_ref": len(ref), "over_mm": round(float(ov[:, 2].sum()), 1), "over_near_steps_mm": round(float(ov[m, 2].sum()), 1),
                              "turns30": len(tpts), "turns30_near_steps": int(tm.sum())}
        print(fx, col, OUT[f"{fx} {col}"])
json.dump(OUT, open(os.path.join(H, "parta_place.json"), "w"), indent=1)
