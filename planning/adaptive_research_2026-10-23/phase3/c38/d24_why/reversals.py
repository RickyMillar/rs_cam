"""C38 D24 analysis, step 2 (places from the samples, model frame): where are op 1's planar reversals (fed-to-fed
XY junctions over 90 deg, fix11/audit.py's rule), C35 against D24; their
place (padded silhouette wall distance, the outer strip = centres 3-6.2 mm
outside), level, the length of the moves that meet there, and the sample
time of the moves that reverse (segment_time_s summed over both moves)."""
import importlib.util, json, math, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
P3 = H.parent.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py")
audit = importlib.util.module_from_spec(spec); spec.loader.exec_module(audit)
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
RUNS = {"c35": P3 / "fix12" / "corne", "d24": P3 / "fix14" / "corne"}
out = {}
for run, d in RUNS.items():
    tp = json.load(open(next(d.glob("tp_1_*.json")))); n = tp["move_count"]
    mv = audit.op_moves(str(d / "program.nc"), "3D Rough 1")[1:n + 1]
    S = np.load(H / f"samples_{run}.npz")
    sel = S["tp"] == 1
    tim = np.bincount(S["mv"][sel], weights=S["seg_t"][sel], minlength=n + 1)
    # The program is in the work frame; the samples are in the model frame.
    # A junction's place is the first sample of its second move.
    first = {}
    for i in np.nonzero(sel)[0]:
        first.setdefault(int(S["mv"][i]), i)
    posall = S["pos"].astype(float)
    pts, times, lens, zs = [], [], [], []
    prev = None; prev_xy = False; prev_k = None
    xy = lambda m: math.hypot(m["b"][0] - m["a"][0], m["b"][1] - m["a"][1])
    for k, m in enumerate(mv):
        if m["g"] == 0:
            prev = None; continue
        t0, t1 = audit.tangents(m)
        if t0 is None:
            continue
        if prev is not None and prev_xy and xy(m) >= audit.GCODE_EPS:
            c = max(-1.0, min(1.0, sum(a * b for a, b in zip(prev, t0))))
            if math.degrees(math.acos(c)) > 90.0 + 1e-6:
                j = first.get(k)
                if j is None:
                    continue
                pts.append(posall[j, :2]); zs.append(posall[j, 2])
                times.append(tim[k] + tim[prev_k]); lens.append(min(xy(m), xy(mv[prev_k])))
        prev = t1; prev_xy = xy(m) >= audit.GCODE_EPS; prev_k = k
    P = np.array(pts, float); w = wall(np.c_[P, np.zeros(len(P))]) if len(P) else np.array([])
    strip = (w > 3.0) & (w <= 6.2)
    z = np.array(zs); L = np.array(lens); T = np.array(times)
    total = float(S["seg_t"][sel].sum())
    out[run] = {
        "planar_reversals": int(len(P)),
        "in_outer_strip": int(strip.sum()),
        "inside_silhouette": int((w <= 0).sum()),
        "outside_other": int(((w > 0) & ~strip).sum()),
        "strip_levels": {f"{v:.1f}": int(((np.abs(z - v) < 0.05) & strip).sum()) for v in (17.0, 11.0, 5.0, 0.5)},
        "shorter_move_p50_mm": round(float(np.median(L)), 2) if len(L) else None,
        "strip_shorter_move_p50_mm": round(float(np.median(L[strip])), 2) if strip.any() else None,
        "op1_sample_time_s": round(total, 1),
        "time_of_moves_at_strip_reversals_s": round(float(T[strip].sum()), 1),
    }
    # op 1 time spent in the strip at the strip levels (all fed samples)
    pos = S["pos"].astype(float); ww = wall(pos); zz = pos[:, 2]
    lv = np.zeros(len(zz), bool)
    for v in (11.0, 5.0, 0.5):
        lv |= np.abs(zz - v) < 0.05
    m1 = sel & S["cut"] & (ww > 3.0) & (ww <= 6.2) & lv
    out[run]["op1_strip_levels_time_s"] = round(float(S["seg_t"][m1].sum()), 1)
print(json.dumps(out, indent=1))
(H / "reversals.json").write_text(json.dumps(out, indent=1))
