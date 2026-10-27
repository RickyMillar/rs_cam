"""D64: the op-1 table per part and strategy, from the per-run files of run_one.sh. Load: the area oracle (AREA_SCREEN=0,
chip_load_fraction per strip) with fix27's weighted percentiles; own bound = ceiling(stepover) + 0.65/6 (the same rule for
every strategy, at the job's stepover). Plunges: vertical G1 descents in program.nc (op 1). Descents: audit_d34 'descent
events into stock'. Left: stock > 0.8 mm over the target (Corne corne_left.py: inside the part = filled silhouette;
wanaka op7_wanaka.py with no edge op: inside the outline, all places, and the 0-9 mm band)."""
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
for part, s, o1 in (("corne_B_s2p1", 2.1, "3D Rough 1"), ("wanaka_s2p0", 2.0, "2 Back Rough")):
    b = ceiling(s) + 0.65 / 6
    for st in ("geodesic_adaptive", "contour_parallel", "adaptive", "agent_search", "contour_spiral"):
        t = f"{part}_{st}"; r = {"bound": round(b, 3)}
        z = np.load(H / f"area_fix45_{t}.npz"); a = z["data"]; c = list(z["cols"]); L = a[:, c.index("L")]; w = a[:, c.index("step_mm")]
        r.update(L_max=round(float(L.max()), 3), L_p99=round(wq(L, w, 0.99), 3), L_p95=round(wq(L, w, 0.95), 3), mm_over_bound=round(float(w[L > b].sum()), 1))
        r["clock_s"] = json.load(open(H / f"clocks_{t}_tp1.json"))["summary_total_s"]
        r["plunges"] = plunges(H / t / "program.nc", o1); r["descents"] = json.load(open(H / f"audit_{t}_tp1.json"))["descent_events"]
        tp = open(H / f"tpsplit_{t}.txt").read(); r["removed_mm3"] = int(re.search(r"tp 1 total .* vol (\d+)", tp).group(1))
        dp = json.load(open(H / f"depth_{t}.json"))["tp1"]; r["max_axial"] = dp["max_axial_mm"]; r["over_dpp_mm3"] = dp["removed_at_axial_over_dpp_mm3"]; r["over_dpp_share"] = dp["share_over_dpp"]
        if part.startswith("corne"):
            lf = json.load(open(H / f"left_{t}.json")); r["left_inside_mm2"] = lf["inside the part"]["left_mm2"]; r["left_vol_inside_mm3"] = lf["inside the part"]["vol_over_target_mm3"]
            r["left_band_0_9_mm2"] = sum(lf[k]["left_mm2"] for k in ("outside 0-3", "outside 3-6.2", "outside 6.2-9"))
        else:
            lf = json.load(open(H / f"op7_{t}.json")); r["left_inside_mm2"] = sum(lf["left_after_edge_op_mm2"][k] for k in ("inside 0-3", "inside 3-6.2", "inside 6.2-9", "inside > 9"))
            r["left_vol_inside_mm3"] = lf["left_after_vol_above_target_inside_mm3"]; r["left_band_0_9_mm2"] = lf["band_inside_0_9_left_after_mm2"]
        r["refusals"] = open(H / t / "stderr.txt").read().lower().count("refused")
        rows[t] = r; print(t, json.dumps(r))
json.dump(rows, open(H / "table.json", "w"), indent=1)
