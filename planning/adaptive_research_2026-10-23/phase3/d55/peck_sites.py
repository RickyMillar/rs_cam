"""Peck sites of one op in a program.nc: per XY, the vertical fed descents, the ladder top Z and the bottom Z."""
import re, sys, collections, math, statistics as st
path, name = sys.argv[1], sys.argv[2]
on = False; x = y = 0.0; z = 0.0; recs = []
for line in open(path):
    line = line.strip()
    if line.startswith("("):
        if re.match(r"^\(\d+ ", line) or re.match(r"^\(2 Back Rough", line) or re.match(r"^\(\d", line):
            on = line == "(" + name + ")"
        continue
    if not on or not line or line[0] != "G": continue
    g = line.split()[0]; nx, ny, nz = x, y, z
    for k, v in re.findall(r"([XYZ])(-?[\d.]+)", line):
        if k == "X": nx = float(v)
        elif k == "Y": ny = float(v)
        else: nz = float(v)
    recs.append((g, x, y, z, nx, ny, nz)); x, y, z = nx, ny, nz
xs = [r[4] for r in recs]; ys = [r[5] for r in recs]
print("moves", len(recs), "x", min(xs), max(xs), "y", min(ys), max(ys))
# a site: consecutive moves at one XY that hold >= 1 fed vertical descent
sites = []; cur = None
for g, x0, y0, z0, x1, y1, z1 in recs:
    vertical = math.hypot(x1 - x0, y1 - y0) < 1e-6
    if vertical and g in ("G1", "G0"):
        key = (round(x1, 3), round(y1, 3))
        if cur is None or cur["xy"] != key:
            cur = {"xy": key, "desc": 0, "top": z0, "bottom": z1, "fed_top": None}; sites.append(cur)
        if g == "G1" and z1 < z0:
            cur["desc"] += 1
            if cur["fed_top"] is None: cur["fed_top"] = z0
        cur["bottom"] = min(cur["bottom"], z1)
    else:
        cur = None
sites = [s for s in sites if s["desc"] > 0]
print("sites", len(sites), "descents", sum(s["desc"] for s in sites), "median per site", st.median(s["desc"] for s in sites))
xmin, xmax, ymin, ymax = (float(v) for v in sys.argv[3:7]) if len(sys.argv) > 6 else (min(xs), max(xs), min(ys), max(ys))
def dist(s):
    x, y = s["xy"]; return min(x - xmin, xmax - x, y - ymin, ymax - y)
hist = collections.Counter()
for s in sites:
    d = dist(s); b = "<=1" if d <= 1 else "1-4" if d <= 4 else "4-10" if d <= 10 else ">10"
    hist[(b, min(s["desc"], 4))] += 1
for k in sorted(hist): print(" dist-to-box", k[0], "descents", k[1] if k[1] < 4 else "4+", hist[k])
tops = collections.Counter(round(s["fed_top"], 1) for s in sites)
print(" fed ladder top Z (most common):", tops.most_common(6))
bots = collections.Counter(round(s["bottom"], 1) for s in sites)
print(" bottom Z (most common):", bots.most_common(6))
