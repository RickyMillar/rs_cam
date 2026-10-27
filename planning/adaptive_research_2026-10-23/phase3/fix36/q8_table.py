"""D52 (Q8): the pick on (C35, geodesic_local_spiral_pick) against the pick off (C37, geodesic_local_spiral_headon = plan B on
every region) on Corne s2.1 B and wanaka s2.0, StockDepth split, tree 453d52571. Per run:
- program.nc md5 (whole file and the op 1 section), for the run-to-run spread;
- op 1 load (area_oracle.py, AREA_SCREEN=0, fix27's percentiles weighted by XY step; own bound ceiling(s) + 0.65/6);
- acceleration clock (summary) of op 1 and the edge op (clocks_*.json);
- plunges: vertical G1 descents in program.nc (op 1 and the edge op), and the audit's descent events into stock (op 1);
- plan time of op 1: the stdout uptime stamp of the last 'generate_toolpath{index=0}' line minus that of the first one;
  and the C29 pick lines (kept plan per region)."""
import glob, hashlib, json, math, re
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
def wq(v, w, q):
    o = np.argsort(v); c = np.cumsum(w[o]); return float(v[o][np.searchsorted(c, q * c[-1])])
def ceiling(s, R=3.0):
    a = math.acos(max(-1, min(1, 1 - min(s, 2 * R) / R))) / (2 * math.pi) * 1.05
    return (1 - math.cos(2 * math.pi * min(a, 0.5))) / 2
def section(path, name):
    out, on = [], False
    for ln in open(path):
        if re.match(r"^\((2 Back Rough|3D Rough 1)", ln): on = ln.strip() == f"({name})"; continue
        if on: out.append(ln)
    return out
def plunges(lines):
    x = y = 0.0; z = 50.0; n = 0
    for ln in lines:
        if not ln.startswith("G"): continue
        g = ln.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", ln):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        if g == "G1" and nx == x and ny == y and nz < z: n += 1
        x, y, z = nx, ny, nz
    return n
rows = {}
for d in sorted(p for p in H.iterdir() if p.is_dir() and (p / "program.nc").exists()):
    s = d.name; fx = s.split("_")[0]; so = 2.1 if fx == "corne" else 2.0
    o1, oe, ke = ("3D Rough 1", "3D Rough 1 [edges, CP rest]", 5) if fx == "corne" else ("2 Back Rough", "2 Back Rough [edges, CP rest]", 7)
    s1, se = section(d / "program.nc", o1), section(d / "program.nc", oe)
    z = np.load(H / f"area_fix36_{s}.npz"); a = z["data"]; c = list(z["cols"]); L = a[:, c.index("L")]; w = a[:, c.index("step_mm")]
    b = ceiling(so) + 0.65 / 6
    c1 = json.load(open(H / f"clocks_{s}_tp1.json")); ce = json.load(open(H / f"clocks_{s}_tp{ke}.json"))
    au = json.load(open(H / f"audit_{s}_tp1.json"))
    ts = [float(m.group(1)) for m in (re.match(r"^\s*([\d.]+)s\s.*generate_toolpath\{index=0\}", ln) for ln in open(d / "stdout_plain.txt")) if m]
    picks = [re.search(r'kept="(\w)"', ln).group(1) for ln in open(d / "stdout_plain.txt") if "C29 pick" in ln and "index=0" in ln]
    rows[s] = {"md5_file": hashlib.md5((d / "program.nc").read_bytes()).hexdigest()[:10], "md5_op1": hashlib.md5("".join(s1).encode()).hexdigest()[:10],
               "L_max": round(float(L.max()), 3), "L_p99": round(wq(L, w, 0.99), 3), "mm_over_own_bound": round(float(w[L > b].sum()), 1),
               "op1_s": c1["summary_total_s"], "edge_s": ce["summary_total_s"], "op1+edge_s": round(c1["summary_total_s"] + ce["summary_total_s"], 1),
               "plunges_op1": plunges(s1), "plunges_edge": plunges(se), "descent_events_op1": au["descent_events"],
               "op1_plan_s": round(ts[-1] - ts[0], 1) if ts else None, "c29_kept": "".join(picks)}
    print(s, json.dumps(rows[s]))
json.dump(rows, open(H / "q8_table.json", "w"), indent=1)
