"""C22 §12.8: assert each claim of P3_C22_3d_turns.md §12.8 (wanaka200, D20 fix7 zigzag_pick
against C35 fix12 spiral_pick). Run descents_by_core.py, over_by_core.py and
over_vs_descents.py first (they write the JSON files read here); gcore.py is imported."""
import os, re, json, sys
H = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, H)
import gcore
J = lambda f: json.load(open(os.path.join(H, f)))
# Claim 1: at Z 19.54 and 14.08 the C35 path leaves the seed as one spiral of about 58 turns
# out to r 97.4 with no radial step; D20 steps out at the first ring (r 3.00 -> 4.38).
M = gcore.moves(os.path.join(gcore.P3, gcore.G["C35"]))
for k0 in gcore.seed_runs(M)[:2]:
    ra, rb, kind, n = gcore.first_radial_step(M, k0)
    assert kind == "G0" and 97.0 < ra < 97.8 and n > 2000, (ra, kind, n)
M20 = gcore.moves(os.path.join(gcore.P3, gcore.G["D20"]))
for k0 in gcore.seed_runs(M20)[:2]:
    ra, rb, kind, n = gcore.first_radial_step(M20, k0)
    assert kind == "radial step" and abs(ra - 3.0) < 0.05 and abs(rb - 4.38) < 0.05
# Claim 2: the 1562 removed descents: 48 + 48 inside the disc at Z 19.54/14.08 (all of them),
# 1435 inside at Z 8.62, 37 outside at Z 8.62; outside at Z 19.54/14.08 rises by 3 each.
d = J("descents_by_core.json")
assert sum(d["D20"].values()) - sum(d["C35"].values()) == 1562
assert d["D20"]["L1 inside"] == d["D20"]["L2 inside"] == 48 and "L1 inside" not in d["C35"] and "L2 inside" not in d["C35"]
assert d["D20"]["L3 inside"] - d["C35"]["L3 inside"] == 1435
assert d["D20"]["L3 outside"] - d["C35"]["L3 outside"] == 37
assert d["C35"]["L1 outside"] - d["D20"]["L1 outside"] == 3
# Claim 3: no over-0.471 length at Z 19.54 or 14.08 in either column; the rise is at Z 8.62
# inside r 105, and the length beyond r 105 is the same.
o = J("over_by_core.json")
for c in ("D20", "C35"):
    for L in ("L1", "L2"):
        assert all(v[1] == 0 for k, v in o[c].items() if k.startswith(L))
assert o["D20"]["L3 r>=105"] == o["C35"]["L3 r>=105"]
rise = sum(o["C35"][k][1] - o["D20"][k][1] for k in ("L3 r<90", "L3 90-105"))
assert 505 < rise < 520, rise
cut20 = o["D20"]["L3 r<90"][0] + o["D20"]["L3 90-105"][0]; cut35 = o["C35"]["L3 r<90"][0] + o["C35"]["L3 90-105"][0]
assert 0.15 < cut35 / cut20 - 1 < 0.18
# Claim 4: of the rise, about 288 mm lies within R of a place where D20 lifted over and C35 does
# not; the rest (about 224 mm) grows as the cut length does (+16 %).
v = J("over_vs_descents.json")
near_rise = v["C35"]["near_d20_only_mm"] - v["D20"]["near_d20_only_mm"]
rest20 = v["D20"]["over_mm"] - v["D20"]["near_d20_only_mm"]; rest35 = v["C35"]["over_mm"] - v["C35"]["near_d20_only_mm"]
assert 280 < near_rise < 295 and 0.15 < rest35 / rest20 - 1 < 0.22, (near_rise, rest35 / rest20)
# Claim 5: the Z 4.0 regions differ in area between the runs (regions come from the planner
# stock after the upper levels), and C31 takes other branches on them.
def openings(path):
    t = re.sub(r"\x1b\[[0-9;]*m", "", open(path).read())
    return [(m.group(1), float(m.group(2)), m.group(3)) for m in re.finditer(r"C31 region opening z=4\.0 region=(\d+) .*?region_area_mm2=([\d.]+) .*?decision=\"([^\"]+)\"", t)]
a = openings(os.path.join(gcore.P3, "fix7/wanaka200/geodesic_local_zigzag_pick/stdout.txt"))
b = openings(os.path.join(gcore.P3, "fix12/wanaka200/stdout.txt"))
assert [x[0] for x in a] == [x[0] for x in b] == ["1", "3"]
assert a[0][1] == 2580.5 and b[0][1] == 2563.2 and a[1][1] == 159.9 and b[1][1] == 160.4
assert a[0][2].startswith("closing guard") and b[0][2].startswith("the shortfall inset split")
assert a[1][2].startswith("the shortfall inset split") and b[1][2] == "grown, no sweep"
print("all §12.8 claims hold")
