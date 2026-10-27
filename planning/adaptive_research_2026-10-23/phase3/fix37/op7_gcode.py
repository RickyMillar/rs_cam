"""D54: a copy of fix34/op7_gcode.py (writes into fix37). D49: a copy of fix33/op7_gcode.py (writes into fix34). D47: the edge op (2 Back Rough [edges, CP rest]) from program.nc: per cut level, the G1 cut length, the plunges (vertical
fed descents) and the G0 moves; G0 XY lengths and the share that cross the part (|dx| > 150 mm or |dy| > 150 mm).
A G0 belongs to the level of the next cut move. Usage: op7_gcode.py <program.nc>..."""
import re, sys, collections, math
for path in sys.argv[1:]:
    on = False; x = y = 0.0; z = 5.0; recs = []
    for line in open(path):
        line = line.strip()
        if line.startswith("("):
            if re.match(r"^\(2 Back Rough", line): on = line == "(2 Back Rough [edges, CP rest])"
            continue
        if not on or not line or line[0] != "G": continue
        g = line.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", line):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        recs.append((g, x, y, z, nx, ny, nz)); x, y, z = nx, ny, nz
    lev = collections.defaultdict(lambda: collections.Counter()); pend = []
    g0xy = []; cross = 0; cross_len = 0.0
    for g, x0, y0, z0, x1, y1, z1 in recs:
        d = math.hypot(x1 - x0, y1 - y0)
        if g == "G0":
            pend.append(d); g0xy.append(d)
            if abs(x1 - x0) > 150 or abs(y1 - y0) > 150: cross += 1; cross_len += d
            continue
        if d < 1e-6 and z1 < z0: lev[round(z1, 1)]["plunges"] += 1; continue
        if d > 1e-6:
            L = lev[round(min(z0, z1), 1)]; L["cut_mm"] += d; L["g1"] += 1
            if pend: L["g0"] += len(pend); L["g0_xy_mm"] += sum(pend); pend = []
    g0xy = sorted(g0xy)
    print(path.split("phase3/")[1], f"G0 {len(g0xy)}, XY {sum(g0xy):.0f} mm, median {g0xy[len(g0xy)//2]:.1f} mm, crossing the part {cross} ({cross_len:.0f} mm)")
    zs = sorted(lev, reverse=True)
    top = sorted(zs, key=lambda z: -lev[z]["g1"])[:8]
    for zz in sorted(top, reverse=True):
        L = lev[zz]; print(f"   z {zz:7.1f}: G1 {L['g1']:5d} cut {L['cut_mm']:8.0f} mm  plunges {L['plunges']:4d}  G0 {L['g0']:4d} ({L['g0_xy_mm']:7.0f} mm)")
    print(f"   levels with a cut: {len(zs)}; z of cut moves, rounded 0.1: min {min(zs)} max {max(zs)}")
