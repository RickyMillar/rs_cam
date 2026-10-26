"""Rasterise a binary STL: the top (max Z) and bottom (min Z) surface per
XY cell centre. Usage: raster.py <stl> <cell mm> <out.npz>"""
import struct, sys
import numpy as np


def raster(stl, g):
    raw = open(stl, "rb").read()
    n = struct.unpack("<I", raw[80:84])[0]
    assert len(raw) == 84 + 50 * n, "not a binary STL"
    tri = np.frombuffer(raw[84:84 + 50 * n], dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]))["v"].astype(float)
    lo = tri.reshape(-1, 3).min(0); hi = tri.reshape(-1, 3).max(0)
    x0, y0 = lo[0] - g, lo[1] - g
    nx = int((hi[0] + g - x0) / g) + 1; ny = int((hi[1] + g - y0) / g) + 1
    top = np.full((ny, nx), -np.inf); bot = np.full((ny, nx), np.inf)
    A, B, C = tri[:, 0], tri[:, 1], tri[:, 2]
    d = (B[:, 1] - C[:, 1]) * (A[:, 0] - C[:, 0]) + (C[:, 0] - B[:, 0]) * (A[:, 1] - C[:, 1])
    for k in np.nonzero(np.abs(d) > 1e-12)[0]:
        (ax, ay, az), (bx, by, bz), (cx, cy, cz) = tri[k]
        i0 = max(0, int(np.floor((min(ax, bx, cx) - x0) / g))); i1 = min(nx - 1, int(np.ceil((max(ax, bx, cx) - x0) / g)))
        j0 = max(0, int(np.floor((min(ay, by, cy) - y0) / g))); j1 = min(ny - 1, int(np.ceil((max(ay, by, cy) - y0) / g)))
        X, Y = np.meshgrid(x0 + np.arange(i0, i1 + 1) * g, y0 + np.arange(j0, j1 + 1) * g)
        l1 = ((by - cy) * (X - cx) + (cx - bx) * (Y - cy)) / d[k]
        l2 = ((cy - ay) * (X - cx) + (ax - cx) * (Y - cy)) / d[k]
        l3 = 1 - l1 - l2
        ins = (l1 >= -1e-9) & (l2 >= -1e-9) & (l3 >= -1e-9)
        if not ins.any():
            continue
        z = l1 * az + l2 * bz + l3 * cz
        st = top[j0:j1 + 1, i0:i1 + 1]; st[ins] = np.maximum(st[ins], z[ins])
        sb = bot[j0:j1 + 1, i0:i1 + 1]; sb[ins] = np.minimum(sb[ins], z[ins])
    return top, bot, x0, y0, lo, hi


if __name__ == "__main__":
    top, bot, x0, y0, lo, hi = raster(sys.argv[1], float(sys.argv[2]))
    np.savez(sys.argv[3], top=top, bot=bot, x0=x0, y0=y0, g=float(sys.argv[2]), lo=lo, hi=hi)
    print(sys.argv[1], "grid", top.shape, "bbox", lo.round(3), hi.round(3))
