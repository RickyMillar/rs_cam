# C27: per level, the area swept (disc R 3 along fed cut moves) by each arm,
# and the area one arm sweeps and the other does not; 0.25 mm raster.
import numpy as np, math
from perlevel import fx, per_level
R, H = 3.0, 0.25
def mask(arm):
    d, _ = per_level(arm)
    out = {}
    for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if g == 0 or c != "cut" or l is None: continue
        out.setdefault(l, []).append((a, b))
    return out
A, B = mask("geodesic_local_zigzag"), mask("geodesic_local_zigzag_v2")
allp = [p for m in (A, B) for segs in m.values() for s in segs for p in s]
x0 = min(p[0] for p in allp) - R; x1 = max(p[0] for p in allp) + R
y0 = min(p[1] for p in allp) - R; y1 = max(p[1] for p in allp) + R
X, Y = np.meshgrid(np.arange(x0, x1, H), np.arange(y0, y1, H))
def raster(segs):
    m = np.zeros(X.shape, bool)
    for a, b in segs:
        lo = (min(a[0], b[0]) - R, min(a[1], b[1]) - R); hi = (max(a[0], b[0]) + R, max(a[1], b[1]) + R)
        i0, i1 = int((lo[1]-y0)/H), int((hi[1]-y0)/H)+2; j0, j1 = int((lo[0]-x0)/H), int((hi[0]-x0)/H)+2
        sx, sy = X[i0:i1, j0:j1], Y[i0:i1, j0:j1]
        dx, dy = b[0]-a[0], b[1]-a[1]; L2 = dx*dx+dy*dy
        t = np.clip(((sx-a[0])*dx+(sy-a[1])*dy)/L2, 0, 1) if L2 > 0 else 0
        m[i0:i1, j0:j1] |= np.hypot(a[0]+t*dx-sx, a[1]+t*dy-sy) <= R
    return m
for l in A:
    ma, mb = raster(A[l]), raster(B[l])
    c = H*H
    print(f'{l}: zigzag {ma.sum()*c:.0f} mm2, v2 {mb.sum()*c:.0f} mm2, v2 only {(mb&~ma).sum()*c:.0f}, zigzag only {(ma&~mb).sum()*c:.0f}')
