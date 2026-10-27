"""D37: the table of the op-1 stepover sweep. Per run: the per-strip area load L
of op 1 (area_oracle.py read with AREA_SCREEN=0: every clearing_cut cutting
sample with a positive sim radial), weighted by XY step length: max, p99, p95,
mm over 0.327 (the exact bound at Corne's s 1.2: ceiling 0.2187 + (0.5 + 0.1 +
0.05)/6) and over 0.471 (the C12 test's bound, s 2.0); op 1 and op 5 on the
three clocks (three_clocks.py); descents and plunge share (audit_d34.py)."""
import json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
def wq(v, w, q):
    o = np.argsort(v); c = np.cumsum(w[o]); return float(v[o][np.searchsorted(c, q * c[-1])])
def ceiling(s, R=3.0):
    a = math.acos(max(-1, min(1, 1 - min(s, 2 * R) / R))) / (2 * math.pi) * 1.05
    return (1 - math.cos(2 * math.pi * min(a, 0.5))) / 2
rows = {}
for s, so in (("cp", 1.2), ("1p2", 1.2), ("1p5", 1.5), ("1p8", 1.8), ("2p1", 2.1)):
    f = H / f"area_fix27_s{s}.npz"
    if not f.exists():
        continue
    z = np.load(f); a = z["data"]; c = list(z["cols"]); L = a[:, c.index("L")]; w = a[:, c.index("step_mm")]
    r = {"stepover": so, "read": int(len(a)), "L_max": round(float(L.max()), 3), "L_p99": round(wq(L, w, 0.99), 3), "L_p95": round(wq(L, w, 0.95), 3),
         "mm_over_0.327": round(float(w[L > 0.327].sum()), 1), "mm_over_0.471": round(float(w[L > 0.471].sum()), 1),
         "own_exact_bound": round(ceiling(so) + (0.5 + 0.1 + 0.05) / 6, 3)}
    r["mm_over_own_bound"] = round(float(w[L > r["own_exact_bound"]].sum()), 1)
    for tp in (1, 5):
        g = H / f"clocks_s{s}_tp{tp}.json"
        if g.exists() and g.stat().st_size:
            k = json.load(open(g)); r[f"op{tp}_samples_s"] = k["samples_total_s"]; r[f"op{tp}_program_s"] = k["program_total_s"]; r[f"op{tp}_accel_s"] = k["summary_total_s"]
    au = json.load(open(H / f"audit_s{s}_tp1.json")); ts = au["time_split_s"]
    r["descents"] = au["descent_events"]; r["plunge_share"] = round(100 * ts["plunge feed"] / au["sim_runtime_s"], 1)
    r["op1+op5_accel_s"] = round(r.get("op1_accel_s", 0) + r.get("op5_accel_s", 0), 1)
    rows[s] = r
    print(s, json.dumps(r))
json.dump(rows, open(H / "sweep_table.json", "w"), indent=1)
