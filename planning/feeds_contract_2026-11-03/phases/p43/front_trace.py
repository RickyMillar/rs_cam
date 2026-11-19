"""P43 round 2, trace 2: for a point of setup "3 Front", the least XY distance from the fed
moves of each level of 'Front bulk rough 18 mm' (levels by the move ranges of the trace file),
with the Z of the nearest move. usage: front_trace.py <nc> <tp json> x y"""
import re, math, json, sys
nc, tpj, x, y = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
lines = open(nc).read().split("\n")
start = [i for i, l in enumerate(lines) if l.startswith("(Front bulk")][0]
mot = []; pos = [0.0, 0.0, 36.0]
for i in range(start + 1, len(lines)):
    l = lines[i]
    if l.startswith("(") and not l.startswith(("(WARN", "(MSG", "(feeds", "(LOAD", "(INFO", "(NOTE", "(drill")): break
    m = re.match(r"G([0123])\b", l)
    if not m: continue
    p0 = tuple(pos)
    for a, v in re.findall(r"([XYZ])(-?\d+\.?\d*)", l): pos["XYZ".index(a)] = float(v)
    mot.append((int(m.group(1)), p0, tuple(pos)))
t = json.load(open(tpj)); its = t["semantic_trace"]["items"]
off = len(mot) - t["move_count"]
print("motion lines", len(mot), "moves", t["move_count"], "(index offset", off, ")")
for lv in [i for i in its if i["kind"] == "depth_level"]:
    best = (1e9, None)
    for k in range(lv["move_start"] + off, min(lv["move_end"] + off + 1, len(mot))):
        g, p0, p1 = mot[k]
        if g == 0: continue
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
        tt = 0 if L2 < 1e-18 else max(0, min(1, ((x - p0[0]) * dx + (y - p0[1]) * dy) / L2))
        d = math.hypot(x - p0[0] - tt * dx, y - p0[1] - tt * dy); z = p0[2] + tt * (p1[2] - p0[2])
        if z < 25.5 and d < best[0]: best = (d, (k, tuple(round(v, 3) for v in p0), tuple(round(v, 3) for v in p1), round(z, 3)))
    print(f"{lv['label']}: least distance of a fed move under Z 25.5 from ({x},{y}): {best[0]:.2f} mm; {best[1]}")
