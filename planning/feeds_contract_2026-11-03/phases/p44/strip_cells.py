"""P44 instrument (design check, before code): the strip cells of the planner stock.

A strip cell is a tool-centre cell c that is not a material cell (the stock at c does not
stand over the floor of c) and whose tool disc covers in full a cell d with
stock top(d) > floor(c) + one planner cell. The part under the disc is at or under floor(c)
(the floor is the drop-cutter height), so that stock is wood, not the part.
Input: the planner stock top and the tool-centre floor at the end of an op (probe1).
Output: the area of the strip cells per place, their band width, and the join with the wood
nodes of the replay (cause_nodes.py: `cause_r3.npz`).
usage: strip_cells.py <probe dir> <radius> <cause npz> [plan bottom z]
"""
import sys, os
import numpy as np
from scipy import ndimage
H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, H)
import probe_read as PR
d, radius = sys.argv[1], float(sys.argv[2])
levels, fin = PR.parse(d, radius)
rows, cols, cell = fin["rows"], fin["cols"], fin["cell"]
top = np.fromfile(os.path.join(d, "r%.1f_stocktop.f32" % radius), np.float32).reshape(rows, cols)
floor = np.fromfile(os.path.join(d, "r%.1f_floor.f32" % radius), np.float32).reshape(rows, cols)
zb = float(sys.argv[4]) if len(sys.argv) > 4 else min(v["z"] for v in levels.values())
fl = np.maximum(floor, zb)
n = int(np.ceil(radius / cell))
yy, xx = np.mgrid[-n:n + 1, -n:n + 1]
full = ((np.abs(xx) + 0.5) * cell) ** 2 + ((np.abs(yy) + 0.5) * cell) ** 2 <= radius * radius
mx = ndimage.maximum_filter(top, footprint=full, mode="nearest")
material = top > fl + 0.01
strip = (~material) & (mx > fl + cell)
X = fin["ox"] + np.arange(cols) * cell + PR.OFF[0]; Y = (fin["oy"] + np.arange(rows) * cell + PR.OFF[1])[:, None]
inside = (X > 20) & (X < 375) & (Y > 12) & (Y < 448)
print(f"tool radius {radius}, cell {cell}, plan bottom {zb}: material cells {material.sum()*cell*cell:.1f} mm2 ; strip cells {strip.sum()*cell*cell:.1f} mm2")
edt = ndimage.distance_transform_edt(strip) * cell
lab, k = ndimage.label(strip, structure=np.ones((3, 3)))
print(f"strip cell bands: {k} components")
w = ndimage.maximum(edt, lab, np.arange(1, k + 1)) * 2 - cell
a = ndimage.sum_labels(np.ones_like(edt), lab, np.arange(1, k + 1)) * cell * cell
for lo, hi in ((0, 0.3), (0.3, 0.6), (0.6, 1.2), (1.2, 3.0), (3.0, 99)):
    m = (w > lo) & (w <= hi)
    print(f"  band width {lo}..{hi} mm: {m.sum()} components, {a[m].sum():.1f} mm2")
h = (mx - fl)[strip]
print("strip height over the cutter, mm2: " + ", ".join("%s: %.0f" % (t, ((h > a_) & (h <= b_)).sum() * cell * cell) for t, a_, b_ in (("0.2..1", 0.2, 1), ("1..3", 1, 3), ("3..6.5", 3, 6.5), ("over 6.5", 6.5, 99))))
used = np.zeros((rows, cols), bool)
for name, (x0, x1, y0, y1) in PR.PLACES:
    m = (X >= x0) & (X <= x1) & (Y >= y0) & (Y <= y1)
    if name.startswith("6"): m = m & ~used
    else: used |= m
    print(f"  {name:20s} strip cells {(strip & m).sum()*cell*cell:8.1f} mm2")
# the join with the wood nodes of the replay: is each strip-wall node within the full disc of a strip cell?
c = np.load(sys.argv[3]); wood = c["wood"]; cls = c["cls"]; near = c["near"]
NY, NX = wood.shape
r_i = np.round((np.arange(NY) * 0.2 - PR.OFF[1] - fin["oy"]) / cell).astype(int); c_i = np.round((np.arange(NX) * 0.2 - PR.OFF[0] - fin["ox"]) / cell).astype(int)
okr = (r_i >= 0) & (r_i < rows); okc = (c_i >= 0) & (c_i < cols)
cover = ndimage.binary_dilation(strip, structure=full)
cov_n = np.zeros((NY, NX), bool); cov_n[np.ix_(okr, okc)] = cover[np.ix_(r_i[okr], c_i[okc])]
sw = wood & (cls == 0) & (near == 0)
print(f"wood nodes of class strip-wall: {sw.sum()*0.04:.1f} mm2 ; of them inside the full disc of a strip cell: {(sw & cov_n).sum()*0.04:.1f} mm2")
sn = wood & (cls == 0) & (near > 0)
print(f"wood nodes of class strip-near: {sn.sum()*0.04:.1f} mm2 ; of them inside the full disc of a strip cell: {(sn & cov_n).sum()*0.04:.1f} mm2")
np.save(os.path.join(os.path.dirname(sys.argv[3]), "strip_r%.0f.npy" % radius), strip)
