"""D66: a copy of the fix45/fix41 cap_check_offmap.py (writes into fix46). D60: for each capped move that cap_audit.py joined off the line-k map, the line-k candidate (D35's map): its F, its
largest sample-to-segment distance, and that of the joined line. Usage: cap_check_offmap.py <run dir> <dx,dy> <op name>"""
import json, re, sys
from pathlib import Path
d = Path(sys.argv[1]); DX, DY = (float(v) for v in sys.argv[2].split(",")); name = sys.argv[3]
tr = json.load(open(d / "simulation.json"))["trace"]
cap = {c["move_index"]: c["feed_cap_mm_min"] for c in tr["capped_moves"] if c["toolpath_id"] == 1}
S = {}
for s in tr["samples"]:
    if s["toolpath_id"] == 1 and s["move_index"] in cap: S.setdefault(s["move_index"], []).append(s["position"])
mv, on, F, x, y = [], False, None, 0.0, 0.0
for ln in open(d / "program.nc"):
    ln = ln.strip()
    if ln.startswith("("):
        if re.match(r"^\((3D Rough|2 Back Rough)", ln): on = ln == f"({name})"
        continue
    if not on or not ln.startswith("G"): continue
    m = re.search(r"F(-?[\d.]+)", ln); F = float(m.group(1)) if m else F
    for k, v in re.findall(r"([XY])(-?[\d.]+)", ln):
        if k == "X": x = float(v)
        else: y = float(v)
    mv.append((ln.split()[0], F, x + DX, y + DY, ln))
def segd(px, py, i):
    ax, ay, bx, by = mv[i - 1][2], mv[i - 1][3], mv[i][2], mv[i][3]; dx, dy = bx - ax, by - ay; L2 = dx * dx + dy * dy
    t = 0.0 if L2 < 1e-12 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / L2))
    return ((ax + t * dx - px) ** 2 + (ay + t * dy - py) ** 2) ** 0.5
ok = bad = 0
for k, c in sorted(cap.items()):
    if k not in S: continue
    i = k + 1; dk = max(segd(p[0], p[1], i) for p in S[k])
    best = min(range(max(1, i - 8), min(len(mv), i + 13)), key=lambda j: max(segd(p[0], p[1], j) for p in S[k]) if mv[j][0] == "G1" else 9e9)
    db = max(segd(p[0], p[1], best) for p in S[k])
    if best != i:
        print(f"move {k} cap {c:.0f}: line k F{mv[i][1]} {mv[i][0]} dist {dk:.3f} | nearest line {best - 1} F{mv[best][1]} dist {db:.3f} | {mv[i][4]}")
    if mv[i][1] <= c + 0.5: ok += 1
    else: bad += 1
print(f"line-k map (D35): {ok} capped moves with F <= cap, {bad} with F > cap")
