"""C22 §14.5: assert each claim of P3_C22_3d_turns.md §14.5 from the D24 artefacts
(fix14/corne, fix14/wanaka200; D23 pick fix13/corne_pick for the reversal base)."""
import json, os, re, math, collections
import numpy as np
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__)); F14 = os.path.join(H, ".."); P3 = os.path.join(F14, "..")
strip = lambda p: re.sub(r"\x1b\[[0-9;]*m", "", open(p).read())
# 1. Rounds: the later rounds drop more groups than the first filter in many plans.
r = []
for ln in strip(os.path.join(F14, "corne", "stdout_plain.txt")).splitlines():
    if "C22a: replayed" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln))
        r.append((int(kv["rounds"]), int(kv["dropped_first"]), int(kv["dropped_more"])))
assert max(x[0] for x in r) == 3 and sum(1 for x in r if x[0] == 3) == 17, collections.Counter(x[0] for x in r)
assert max(x[2] for x in r) == 271 and sum(1 for x in r if x[2] > x[1]) >= 10
# 2. wanaka counter: every flagged point lies outside the emitted toolpath's XY box, and the
# points at the frame edge read the full stock above the level (stock top 25 - Z).
tp = json.load(open(os.path.join(F14, "wanaka200", "tp_1_2_Back_Rough.json")))
box = next(i["xy_bbox"] for i in tp["semantic_trace"]["items"] if i["kind"] == "operation")
assert (box["min_x"], box["max_x"], box["min_y"], box["max_y"]) == (22.76, 217.24, 27.76, 221.74)
pts = []
for ln in strip(os.path.join(F14, "wanaka200", "stdout_plain.txt")).splitlines():
    if "held link over standing stock" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("standing stock", 1)[1]))
        pts.append((float(kv["z"]), float(kv["x"]), float(kv["y"]), float(kv["stock_above_mm"])))
assert len(pts) == 66
outside = lambda x, y: x < box["min_x"] or x > box["max_x"] or y < box["min_y"] or y > box["max_y"]
assert all(outside(x, y) for _, x, y, _ in pts)
full = [(z, a) for z, x, y, a in pts if abs(a - (25.0 - z)) < 1e-3]
assert len(full) == 34, len(full)
# no emitted move beyond the box (the G-code)
xs = [float(v) for v in re.findall(r"X(-?[\d.]+)", open(os.path.join(F14, "wanaka200", "program.nc")).read())]
assert max(xs) == 217.24 and min(xs) == 22.76
# 3. Corne move 6921: a Linking feed at the level (Z 11.00), 2.25 mm, between clearing cuts that
# also remove 12 mm at the same place.
sim = json.load(open(os.path.join(F14, "corne", "simulation.json")))
ms = collections.defaultdict(list)
for s in sim["trace"]["samples"]:
    if s["toolpath_id"] == 1 and 6915 <= s["move_index"] <= 6922: ms[s["move_index"]].append(s)
m = ms[6921]
assert m[-1]["source_intent"] == "linking" and all(abs(s["position"][2] - 11.0) < 1e-6 for s in m)
assert abs(max(s["axial_engagement_mm"] for s in m) - 10.5) < 1e-6
assert max(max(s["axial_engagement_mm"] for s in ms[k]) for k in (6915, 6916, 6917)) == 12.0
assert all(ms[k][-1]["source_intent"] == "clearing_cut" for k in (6915, 6916, 6917, 6918, 6919, 6920, 6922))
# 4. The reversals (> 135 deg at move junctions) of D24 are in the outer strip, where D23's
# corridor opening planned; inside the silhouette they are unchanged.
part = np.load(os.path.join(P3, "fix9", "corne_part.npz"))
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
def inside(x, y):
    c, rr = int(round((x - x0) / g)), int(round((y - y0) / g))
    return 0 <= rr < sil.shape[0] and 0 <= c < sil.shape[1] and bool(sil[rr, c])
def reversals(d):
    last = {}
    for s in json.load(open(os.path.join(P3, d, "simulation.json")))["trace"]["samples"]:
        last[(s["toolpath_id"], s["move_index"])] = s
    out = collections.Counter()
    for tpid in (1, 3):
        mm = sorted(k for (t, k) in last if t == tpid); pd = None
        for a, b in zip(mm, mm[1:]):
            pa, pb = last[(tpid, a)]["position"], last[(tpid, b)]["position"]
            dx, dy = pb[0] - pa[0], pb[1] - pa[1]
            if math.hypot(dx, dy) < 1e-6 or abs(pb[2] - pa[2]) > 1e-3: pd = None; continue
            dd = math.atan2(dy, dx)
            if pd is not None and abs((dd - pd + math.pi) % (2 * math.pi) - math.pi) > math.radians(135):
                out["inside" if inside(pa[0], pa[1]) else "outside"] += 1
            pd = dd
    return out
r23, r24 = reversals("fix13/corne_pick"), reversals("fix14/corne")
assert r23["inside"] == 36 and r24["inside"] == 40 and r23["outside"] == 10 and r24["outside"] == 2217, (r23, r24)
print("all §14.5 claims hold")
