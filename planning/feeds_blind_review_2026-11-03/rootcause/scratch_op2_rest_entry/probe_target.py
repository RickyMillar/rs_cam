"""Sample the back target surface around the op 2 entry point.

Reads a binary STL, rasterises the top surface on a 0.25 mm grid in a window,
then computes the flat drop-cutter height for the 6 mm and the 18 mm tools.
"""
import sys
import numpy as np

path, cx, cy, half = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])
cell = 0.25
raw = open(path, "rb").read()
n = int.from_bytes(raw[80:84], "little")
dt = np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")])
tri = np.frombuffer(raw, dtype=dt, count=n, offset=84)["v"].astype(np.float64)
pad = 10.0
lo_x, hi_x, lo_y, hi_y = cx - half - pad, cx + half + pad, cy - half - pad, cy + half + pad
mx = tri[:, :, 0]
my = tri[:, :, 1]
sel = (mx.max(1) >= lo_x) & (mx.min(1) <= hi_x) & (my.max(1) >= lo_y) & (my.min(1) <= hi_y)
tri = tri[sel]
xs = np.arange(lo_x, hi_x + 1e-9, cell)
ys = np.arange(lo_y, hi_y + 1e-9, cell)
Z = np.full((len(ys), len(xs)), -np.inf)
for t in tri:
    (x0, y0, z0), (x1, y1, z1), (x2, y2, z2) = t
    d = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)
    if abs(d) < 1e-12:
        continue
    i0 = max(0, int(np.floor((min(x0, x1, x2) - lo_x) / cell)))
    i1 = min(len(xs) - 1, int(np.ceil((max(x0, x1, x2) - lo_x) / cell)))
    j0 = max(0, int(np.floor((min(y0, y1, y2) - lo_y) / cell)))
    j1 = min(len(ys) - 1, int(np.ceil((max(y0, y1, y2) - lo_y) / cell)))
    if i1 < i0 or j1 < j0:
        continue
    X, Y = np.meshgrid(xs[i0:i1 + 1], ys[j0:j1 + 1])
    a = ((y1 - y2) * (X - x2) + (x2 - x1) * (Y - y2)) / d
    b = ((y2 - y0) * (X - x2) + (x0 - x2) * (Y - y2)) / d
    c = 1 - a - b
    inside = (a >= -1e-9) & (b >= -1e-9) & (c >= -1e-9)
    zz = a * z0 + b * z1 + c * z2
    sub = Z[j0:j1 + 1, i0:i1 + 1]
    np.maximum(sub, np.where(inside, zz, -np.inf), out=sub)


def drop(radius):
    k = int(np.ceil(radius / cell))
    out = np.full_like(Z, -np.inf)
    for dj in range(-k, k + 1):
        for di in range(-k, k + 1):
            if (di * cell) ** 2 + (dj * cell) ** 2 > radius ** 2:
                continue
            sh = np.roll(np.roll(Z, dj, 0), di, 1)
            np.maximum(out, sh, out=out)
    return out


D6 = drop(3.0)
D18 = drop(9.0)
step = int(round(1.0 / cell))
ix = [i for i, x in enumerate(xs) if abs(x - cx) <= half + 1e-9][::step]
jy = [j for j, y in enumerate(ys) if abs(y - cy) <= half + 1e-9][::step]
for name, A in (("surface", Z), ("drop6", D6), ("drop18", D18)):
    print(f"== {name} (rows y high->low, cols x {xs[ix[0]]:.1f}..{xs[ix[-1]]:.1f} step 1 mm)")
    for j in reversed(jy):
        print(f"y={ys[j]:6.1f} " + " ".join(f"{A[j, i]:4.1f}" if np.isfinite(A[j, i]) else "  --" for i in ix))
jc = int(round((cy - lo_y) / cell))
ic = int(round((cx - lo_x) / cell))
print(f"at entry: surface {Z[jc, ic]:.2f} drop6 {D6[jc, ic]:.2f} drop18 {D18[jc, ic]:.2f}")
