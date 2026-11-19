"""P43 round 2, trace 3: the wall-line cuts of one fresh rough of build 2 against the model.
The wall-line moves are the fed moves of the FIRST cleanup item of each level in the trace
file, less its entry items. For each sample of them (every 0.5 mm):
- clearance: the least XY distance to a target node that stands more than 0.05 mm above the
  tool tip, less the tool radius (the model wall at the height of the tool);
- lift: the point rides the model where a point `shift` farther from every wall would not:
  full = the highest target within R; reduced = the highest target within R - shift;
  lifted when full > max(level, reduced) + tol and Z >= full - 0.15.
usage: wall_line.py <nc> <tp json> <op prefix> <R> <shift> <tol> [dx dy of the target frame]"""
import re, math, json, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
import numpy as np
from gapmap import target, CELL, NX, NY
nc, tpj, pref = sys.argv[1], sys.argv[2], sys.argv[3]
R, shift, tol = float(sys.argv[4]), float(sys.argv[5]), float(sys.argv[6])
T = target(); T = np.where(np.isfinite(T), T, -1e9)
lines = open(nc).read().split("\n")
start = [i for i, l in enumerate(lines) if l.startswith("(" + pref)][0]
mot = []; pos = [0.0, 0.0, 36.0]
for i in range(start + 1, len(lines)):
    l = lines[i]
    if l.startswith("(") and not l.startswith(("(WARN", "(MSG", "(feeds", "(LOAD", "(INFO", "(NOTE", "(drill")): break
    m = re.match(r"G([0123])\b", l)
    if not m: continue
    p0 = tuple(pos)
    for a, v in re.findall(r"([XYZ])(-?\d+\.?\d*)", l): pos["XYZ".index(a)] = float(v)
    mot.append((int(m.group(1)), p0, tuple(pos)))
t = json.load(open(tpj)); its = t["semantic_trace"]["items"]; off = len(mot) - t["move_count"]
print(f"motion lines {len(mot)}, moves {t['move_count']} (offset {off})")
nr = int(math.ceil((R + 0.4) / CELL)); jj, ii = np.mgrid[-nr:nr + 1, -nr:nr + 1]; dd = np.hypot(ii * CELL, jj * CELL)
tot = lifted = 0; worst_lift = 0.0; least = (1e9, None); lifts = []
for lv in [i for i in its if i["kind"] == "depth_level"]:
    cl = sorted([i for i in its if i["kind"] == "cleanup" and i["parent_id"] == lv["id"]], key=lambda i: i["move_start"])
    if len(cl) < 2: print(lv["label"], "no wall item"); continue
    w = cl[0]; zl = lv["params"]["values"]["z_level"]
    ent = [(i["move_start"], i["move_end"]) for i in its if i["kind"] == "entry" and w["move_start"] <= i["move_start"] <= w["move_end"]]
    n = 0; L = 0.0; lv_lift = 0; lv_least = 1e9
    for k in range(w["move_start"], w["move_end"] + 1):
        if any(a <= k <= b for a, b in ent): continue
        g, p0, p1 = mot[k + off]
        if g == 0: continue
        seg = math.dist(p0, p1); L += math.hypot(p1[0] - p0[0], p1[1] - p0[1]); ns = max(1, int(math.ceil(seg / 0.5)))
        for s in range(ns + 1):
            f = s / ns; x, y, z = (p0[c] + f * (p1[c] - p0[c]) for c in range(3))
            i0, j0 = int(round(x / CELL)), int(round(y / CELL))
            if not (nr <= i0 < NX - nr and nr <= j0 < NY - nr): continue
            win = T[j0 - nr:j0 + nr + 1, i0 - nr:i0 + nr + 1]
            d = np.hypot((i0 + ii) * CELL - x, (j0 + jj) * CELL - y)
            above = win > z + 0.05
            c = float(d[above].min()) - R if above.any() else 99.0
            full = float(win[d <= R - 1e-6].max()); red = float(win[d <= R - shift].max())
            n += 1; tot += 1; lv_least = min(lv_least, c)
            if c < least[0]: least = (c, (round(x, 3), round(y, 3), round(z, 3), k))
            if full > max(zl, red) + tol and z >= full - 0.15:
                lifted += 1; lv_lift += 1; worst_lift = max(worst_lift, z - max(zl, red)); lifts.append((round(x, 2), round(y, 2), round(z, 2), round(full, 2), round(red, 2), k))
    print(f"{lv['label']}: wall item moves {w['move_start']}..{w['move_end']}, {len(ent)} entries, {L/1000:.2f} m of cut, {n} samples, lifted {lv_lift}, least clearance {lv_least:.3f} mm")
print(f"all levels: samples {tot}; lifted onto the model by the nearer line: {lifted}, the largest lift {worst_lift:.2f} mm; least clearance {least[0]:.3f} mm at {least[1]}")
for r in lifts[:12]: print("   lifted", r)
