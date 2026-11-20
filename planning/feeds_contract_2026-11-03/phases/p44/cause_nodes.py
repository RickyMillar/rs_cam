"""P44 instrument: the cause of each wood node of the replay, from the probe of one op.

A wood node: stock - reach6h > 0.3 mm in the replay after the op (places.py), so a 6 mm flat
tool can cut it. The probe (probe_read.py) gives a class to each tool-centre cell where the
planner stock stands over the plan floor at the end of the op.

A node takes the class of the tool-centre cell under it when that cell has one. Else no
centre cell under the node held material at the end: the node is in a strip beside a wall
(the tool centre cannot stand over it; a centre up to one tool radius away cuts it). Such a
node takes the class "strip":
  strip-near  a centre cell within the tool radius of the node has a class (the plan dropped
              that centre region; the same centres cut the strip), reported under that class
  strip-wall  no centre cell within the tool radius has a class: the level grid reads the
              stock only at the centre cell, so the strip made no region at any level
usage: cause_nodes.py <replay.npz> <probe dir> <radius> <op name prefix>
"""
import sys, os
H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, H); sys.path.insert(0, os.path.join(H, "..", "p37"))
import numpy as np
from scipy import ndimage
import gapmap as G
import places as PL
import probe_read as PR

npz, pdir, radius, pref = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
d = np.load(npz); names = [str(n) for n in d["names"]]
k = [i for i, n in enumerate(names) if n.startswith(pref)][0]
S = d[f"s{k}"].astype(float); Sprev = d[f"s{k-1}"].astype(float)
T = G.target(); ok = np.isfinite(T)
# the reach surface of THIS tool
if abs(radius - 3.0) < 1e-9:
    R6 = PL.reach6h(T)
else:
    lab, n = ndimage.label(~ok)
    edge = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]])))
    hole = (~ok) & ~np.isin(lab, list(edge))
    R6 = G.reach(np.where(hole, -50.0, T), radius)
levels, fin, cls, lastz, top, floor, info = PR.classify(pdir, radius)
zs = sorted({round(v["z"], 3) for v in levels.values()})
zb = zs[0]
fl = np.maximum(floor, zb)
woodc = (top - fl) > PR.BAR
cell = fin["cell"]
# node (i, j) of the replay: G-code x = i * CELL -> setup x = x - OFF -> probe col
NX, NY, C = G.NX, G.NY, G.CELL
cols = np.round((np.arange(NX) * C - G.OFF[0] - fin["ox"]) / cell).astype(int)   # cell centre = origin + index * cell
rows = np.round((np.arange(NY) * C - G.OFF[1] - fin["oy"]) / cell).astype(int)
okc = (cols >= 0) & (cols < fin["cols"]); okr = (rows >= 0) & (rows < fin["rows"])
def to_nodes(a, fill):
    out = np.full((NY, NX), fill, dtype=a.dtype)
    out[np.ix_(okr, okc)] = a[np.ix_(rows[okr], cols[okc])]
    return out
cls_n = to_nodes(np.where(woodc, cls, 0).astype(np.int8), 0)
plan_floor = np.maximum(R6, zb)
wood = ok & ((S - plan_floor) > G.BAR)
print(f"op '{names[k]}', tool radius {radius}, plan bottom Z {zb}: nodes where the stock after the op stands more than {G.BAR} mm over the reach surface of this tool (not under the plan bottom): {wood.sum()*C*C:.1f} mm2")
fp = G.disc(radius)
near = np.zeros((NY, NX), np.int8)
for c in range(len(PR.CLASSES) - 1, 0, -1):          # the lowest class number wins
    m = ndimage.binary_dilation(cls_n == c, structure=fp)
    near[m] = c
names_c = PR.CLASSES
hdr = "%-26s" % "place" + "".join("%9s" % ("c%d" % i) for i in range(1, len(names_c))) + "%10s%10s%10s" % ("strip-wl", "total", "of it:strip-near")
print("classes: " + "; ".join("c%d = %s" % (i, c) for i, c in enumerate(names_c) if i) + "; strip-wl = strip at a wall, no centre cell within the tool radius has a class")
print(hdr)
tot = None
for name, m in PL.masks():
    row = []
    for c in range(1, len(names_c)):
        row.append(((wood & m) & (((cls_n == c)) | ((cls_n == 0) & (near == c)))).sum() * C * C)
    sw = (wood & m & (cls_n == 0) & (near == 0)).sum() * C * C
    sn = (wood & m & (cls_n == 0) & (near > 0)).sum() * C * C
    row += [sw, (wood & m).sum() * C * C, sn]
    tot = np.array(row) if tot is None else tot + np.array(row)
    print("%-26s" % name + "".join("%9.1f" % v for v in row[:-3]) + "%10.1f%10.1f%10.1f" % tuple(row[-3:]))
print("%-26s" % "all" + "".join("%9.1f" % v for v in tot[:-3]) + "%10.1f%10.1f%10.1f" % tuple(tot[-3:]))
# how tall the strip-wall wood stands, and how wide
sw = wood & (cls_n == 0) & (near == 0)
if sw.any():
    h = (S - plan_floor)[sw]
    print("strip at a wall: height over the reach surface, mm2 per band: " + ", ".join("%s: %.0f" % (lab, ((h > a) & (h <= b)).sum() * C * C) for lab, a, b in (("0.3..1", 0.3, 1), ("1..3", 1, 3), ("3..6.5", 3, 6.5), ("over 6.5", 6.5, 99))))
    cutby = (Sprev - S)[sw] > 1e-4
    print("of it, nodes that this op lowered: %.0f mm2; nodes that it did not touch: %.0f mm2" % (cutby.sum() * C * C, (~cutby).sum() * C * C))
np.savez_compressed(os.path.join(os.path.dirname(npz), "cause_r%.0f.npz" % radius), wood=wood, cls=cls_n, near=near)
