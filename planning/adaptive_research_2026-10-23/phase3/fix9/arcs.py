"""C18/fix9: why the v3 level image shows a sparse annulus. The v3 builder
draws each G2/G3 move as its chord (fixture3d_v2/v3.py `parse`: no I/J).
This reads '3D Rough 1' of program.nc on the Z 17 plane (G-code Z -6.000)
and, in rings round the seed (G-code frame), the length of G1 moves, the
true length of G2/G3 arcs and the length of their chords."""
import json, math, os, re, sys
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
SEED_G = (55.552, 59.554)   # program.nc: the first plunge of D20 '3D Rough 1'
W = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
out = {}
for col, rd in (("zigzag_pick D18", "fix5/corne/geodesic_local_zigzag_pick"), ("zigzag_pick D20", "fix7/corne/geodesic_local_zigzag_pick"),
                ("contour_parallel", "fix6/corne/contour_parallel")):
    lines = open(os.path.join(P3, rd, "program.nc")).read().splitlines()
    s = lines.index("(3D Rough 1)") + 1
    e = next(i for i in range(s, len(lines)) if lines[i].startswith("(3D Rough 1 [copy])"))
    g, pos = 0, [0.0, 0.0, 0.0]
    acc = {k: [0.0, 0.0, 0.0, 0] for k in ("r<25.45", "25.45-50")}   # G1 mm, arc mm, chord mm, arcs
    for ln in lines[s:e]:
        w = dict((a, float(b)) for a, b in W.findall(ln))
        if "G" in w: g = int(w["G"])
        if not any(k in w for k in "XYZ"): continue
        end = [w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2])]
        if g in (1, 2, 3) and abs(pos[2] + 6.0) < 0.01 and abs(end[2] + 6.0) < 0.01:
            mid = ((pos[0] + end[0]) / 2, (pos[1] + end[1]) / 2)
            r = math.hypot(mid[0] - SEED_G[0], mid[1] - SEED_G[1])
            key = "r<25.45" if r < 25.45 else ("25.45-50" if r < 50 else None)
            if key:
                ch = math.hypot(end[0] - pos[0], end[1] - pos[1])
                if g == 1:
                    acc[key][0] += ch
                else:
                    cx, cy = pos[0] + w.get("I", 0.0), pos[1] + w.get("J", 0.0)
                    rad = math.hypot(pos[0] - cx, pos[1] - cy)
                    a0 = math.atan2(pos[1] - cy, pos[0] - cx); a1 = math.atan2(end[1] - cy, end[0] - cx)
                    sw = (a0 - a1) % (2 * math.pi) if g == 2 else (a1 - a0) % (2 * math.pi)
                    if sw < 1e-9: sw = 2 * math.pi
                    acc[key][1] += rad * sw; acc[key][2] += ch; acc[key][3] += 1
        pos = end
    out[col] = {k: {"g1_mm": round(v[0], 1), "arc_mm": round(v[1], 1), "chord_mm": round(v[2], 1), "arcs": v[3]} for k, v in acc.items()}
    print(col, out[col])
json.dump(out, open(os.path.join(H, "arcs_z17.json"), "w"), indent=1)
