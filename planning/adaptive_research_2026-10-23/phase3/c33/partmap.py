"""The Corne part's top surface on a 0.2 mm XY grid (max Z of the STL per
cell centre), from the binary STL the project loads (model units mm, no
transform: setup face top, z rotation 0). Saved as partmap.npz."""
import struct, sys
import numpy as np
STL = "/home/ricky/Downloads/Corne Case Hex.stl"
G = 0.2
raw = open(STL, "rb").read()
n = struct.unpack("<I", raw[80:84])[0]
tri = np.frombuffer(raw[84:84 + 50 * n], dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]))["v"].astype(float)
lo = tri.reshape(-1, 3).min(0); hi = tri.reshape(-1, 3).max(0)
print("tris", n, "bbox", lo.round(3), hi.round(3))
x0, y0 = lo[0] - 1, lo[1] - 1
nx = int((hi[0] + 1 - x0) / G) + 1; ny = int((hi[1] + 1 - y0) / G) + 1
top = np.full((ny, nx), -np.inf)
for t in tri:
    (ax, ay, az), (bx, by, bz), (cx, cy, cz) = t
    d = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
    if abs(d) < 1e-12: continue
    i0 = max(0, int(np.floor((min(ax, bx, cx) - x0) / G))); i1 = min(nx - 1, int(np.ceil((max(ax, bx, cx) - x0) / G)))
    j0 = max(0, int(np.floor((min(ay, by, cy) - y0) / G))); j1 = min(ny - 1, int(np.ceil((max(ay, by, cy) - y0) / G)))
    X, Y = np.meshgrid(x0 + np.arange(i0, i1 + 1) * G, y0 + np.arange(j0, j1 + 1) * G)
    l1 = ((by - cy) * (X - cx) + (cx - bx) * (Y - cy)) / d
    l2 = ((cy - ay) * (X - cx) + (ax - cx) * (Y - cy)) / d
    l3 = 1 - l1 - l2
    ins = (l1 >= -1e-9) & (l2 >= -1e-9) & (l3 >= -1e-9)
    if not ins.any(): continue
    z = l1 * az + l2 * bz + l3 * cz
    sub = top[j0:j1 + 1, i0:i1 + 1]
    sub[ins] = np.maximum(sub[ins], z[ins])
np.savez("partmap.npz", top=top, x0=x0, y0=y0, g=G)
print("grid", top.shape, "cells with surface", np.isfinite(top).mean().round(3), "z range", np.nanmin(top[np.isfinite(top)]).round(3), top[np.isfinite(top)].max().round(3))
