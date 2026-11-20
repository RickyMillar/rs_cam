"""P44b: a node map of the wood that fits in one place after id 2 (digit = mm over the reach surface).
usage (from phases/): place_map.py <scratch p44 dir> <place index 0..6> <run tag>...  (reads out_<tag>.npz)"""
import sys, os
sys.path.insert(0, "p44"); sys.path.insert(0, "p37")
from places import *
T = target(); ok = np.isfinite(T); R6 = reach6h(T)
def load(npz):
    d = np.load(npz); names = [str(n) for n in d["names"]]
    k2 = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
    return d[f"s{k2}"].astype(float), d[f"s{k2-1}"].astype(float)
def show(tag, win, step=1):
    x0, x1, y0, y1 = win
    S2, S1 = load(f"{sys.argv[1]}/out_{tag}.npz")
    i0, i1, j0, j1 = int(round(x0/CELL)), int(round(x1/CELL)), int(round(y0/CELL)), int(round(y1/CELL))
    g = np.where(ok, S2 - R6, 0.0)[j0:j1+1, i0:i1+1]
    s = S2[j0:j1+1, i0:i1+1]; t = T[j0:j1+1, i0:i1+1]; r = R6[j0:j1+1, i0:i1+1]
    fits = g > BAR
    print(tag, win, "fits mm2", fits.sum()*CELL*CELL)
    # histogram of stock z of fits nodes and heights
    zs = np.round(s[fits], 1); u, c = np.unique(zs, return_counts=True)
    print(" stock z of fits nodes (z: mm2):", {float(a): round(b*CELL*CELL,1) for a, b in zip(u, c) if b*CELL*CELL >= 0.5})
    hs = g[fits]; print(" height over reach: min %.2f med %.2f max %.2f" % (hs.min(), np.median(hs), hs.max()))
    # distance of each fits node to the nearest non-reachable... rim test: erode fits by 1 node twice
    from scipy import ndimage as nd
    e1 = nd.binary_erosion(fits, np.ones((3,3))); e2 = nd.binary_erosion(e1, np.ones((3,3)))
    print(" fits nodes: all %.1f, deeper than 1 node from the edge %.1f, deeper than 2 nodes %.1f mm2" % (fits.sum()*.04, e1.sum()*.04, e2.sum()*.04))
    lab, n = nd.label(fits, np.ones((3,3)))
    for k in range(1, n+1):
        m = lab == k
        if m.sum()*0.04 < 1.0: continue
        jj, ii = np.nonzero(m)
        print("  comp %.1f mm2 x %.1f..%.1f y %.1f..%.1f stock z %.2f..%.2f target %.2f..%.2f reach %.2f..%.2f width(nodes, max erosion) %d" % (m.sum()*.04, x0+ii.min()*CELL, x0+ii.max()*CELL, y0+jj.min()*CELL, y0+jj.max()*CELL, s[m].min(), s[m].max(), np.nanmin(t[m]), np.nanmax(t[m]), r[m].min(), r[m].max(), int(nd.distance_transform_cdt(m, 'chessboard').max())))
    if step:
        print(" map (. none, digit = height over reach clipped 9, rows y up->down):")
        for j in range(fits.shape[0]-1, -1, -step):
            print("  %6.1f " % (y0+j*CELL) + "".join(("%d" % min(9, int(g[j,i]))) if fits[j,i] else ("#" if not np.isfinite(t[j,i]) else ("^" if s[j,i]-t[j,i] > BAR else ".")) for i in range(0, fits.shape[1], step)))
for tag in sys.argv[3:]:
    show(tag, PLACES[int(sys.argv[2])][1], 1 if int(sys.argv[2]) == 1 else 0)
