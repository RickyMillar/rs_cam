"""D66: the table of fix46's runs (op 1, plus the edge op for the 'job' runs). Load: area oracle with fix27 percentiles;
own bound ceiling(s) + 0.65/6 (Corne s2.1 0.489, wanaka s2.0 0.471)."""
import json, math, re
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
def wq(v, w, q):
    o = np.argsort(v); c = np.cumsum(w[o]); return float(v[o][np.searchsorted(c, q * c[-1])])
def ceiling(s, R=3.0):
    a = math.acos(max(-1, min(1, 1 - min(s, 2 * R) / R))) / (2 * math.pi) * 1.05; return (1 - math.cos(2 * math.pi * min(a, 0.5))) / 2
def plunges(nc, name):
    on = False; x = y = 0.0; z = 50.0; n = 0
    for ln in open(nc):
        ln = ln.strip()
        if ln.startswith("("):
            if re.match(r"^\((2 Back Rough|3D Rough)", ln): on = ln == f"({name})"
            continue
        if not on or not ln.startswith("G"): continue
        g = ln.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", ln):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        if g == "G1" and nx == x and ny == y and nz < z: n += 1
        x, y, z = nx, ny, nz
    return n
rows = {}
for t in ("7ee3d3226_corne_op1", "a151149fd_corne_op1", "a151149fd_corne_job", "7ee3d3226_wanaka_op1", "a151149fd_wanaka_op1", "a151149fd_wanaka_job"):
    corne = "corne" in t; s = 2.1 if corne else 2.0; b = ceiling(s) + 0.65 / 6; o1 = "3D Rough 1" if corne else "2 Back Rough"
    r = {}
    z = np.load(H / f"area_fix46_{t}.npz"); a = z["data"]; c = list(z["cols"]); L = a[:, c.index("L")]; w = a[:, c.index("step_mm")]
    r.update(L_max=round(float(L.max()), 3), L_p99=round(wq(L, w, 0.99), 3), L_p95=round(wq(L, w, 0.95), 3), mm_over_bound=round(float(w[L > b].sum()), 1))
    r["op1_clock_s"] = json.load(open(H / f"clocks_{t}_tp1.json"))["summary_total_s"]
    r["plunges_op1"] = plunges(H / t / "program.nc", o1); r["descents_op1"] = json.load(open(H / f"audit_{t}_tp1.json"))["descent_events"]
    tp = open(H / f"tpsplit_{t}.txt").read(); r["op1_removed_mm3"] = int(re.search(r"tp 1 total .* vol (\d+)", tp).group(1))
    r["job_s"] = float(re.search(r"total ([\d.]+)\n", tp).group(1))
    r["entries"] = {k: (v["moves"], v["time_s"], v["air_share"]) for k, v in json.load(open(H / f"entries_{t}.json"))["classes"].items()}
    ca = json.load(open(H / f"cap_audit_{t}.json")); r["capped"] = {k: {kk: v.get(kk) for kk in ("capped", "capped moves that remove stock", "cap min/median/max", "trace samples over cap (moves)", "G-code fed lines joined by position", "G-code F over cap (moves)")} for k, v in ca["per_toolpath"].items() if v.get("capped")}
    dp = json.load(open(H / f"depth_{t}.json")); key = [k for k in dp if k.startswith("tp") and k != "tp1"]
    r["op1_axial"] = (dp["tp1"]["max_axial_mm"], dp["tp1"]["removed_at_axial_over_dpp_mm3"], dp["tp1"]["share_over_dpp"])
    if key: r["edge_axial"] = (dp[key[0]]["max_axial_mm"], dp[key[0]]["removed_at_axial_over_dpp_mm3"], dp[key[0]]["share_over_dpp"])
    rows[t] = r; print(t, json.dumps(r))
json.dump(rows, open(H / "table.json", "w"), indent=1)
