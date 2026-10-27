"""D60: a copy of fix37/corne_reach.py (writes into fix41). D49: Corne, the edge op (op 5) against the reach of op 1's boundary, from program.nc (the sims are deleted for D45).
Op 1's boundary: model_silhouette, containment center, offset 6.0 (d44/k_body_cp_edges_s2p1_d44_rest_regions.toml), so op 1's
tool centres stay within 6 mm of the silhouette and its cutter reaches 9 mm. Target: op 5 cuts nothing beyond 9 mm.
- Frame: G-code = model - stock origin (origin_y -109.3155, top z 23): x = X, y = Y - 109.3155, z = Z + 23 (checked on op 1's
  extents against the trace's).
- Silhouette: the filled finite top > 0 cells of fix9/corne_part.npz, padded by 40 mm (the part grid ends about 2 mm past
  the part, so fix31/op5_analysis.py's 'outside > 9' row also took every off-grid cell; this script does not).
- Measures: op 5's G1 end points by distance outside the silhouette (share > 6.3 mm = beyond op 1's centre region plus one
  0.25 mm cell, and the max); the stock lowered by op 5 (flat 6 mm disc along op 1 then op 5 G1 moves, 0.25 mm grid, from
  the stock top 23) by place, area > 0.8 mm and volume.
Usage: corne_reach.py <program.nc>..."""
import math, sys, re
import numpy as np
from scipy import ndimage
P3 = __import__("pathlib").Path(__file__).resolve().parent.parent
part = np.load(P3 / "fix9" / "corne_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(40 / pg); fin = np.isfinite(ptop) & (ptop > 1e-6); fill = np.pad(ndimage.binary_fill_holes(fin), pad)
X0, Y0 = px0 - pad * pg, py0 - pad * pg; dout = ndimage.distance_transform_edt(~fill) * pg
def dist(x, y):
    i = np.round((x - X0) / pg).astype(int); j = np.round((y - Y0) / pg).astype(int)
    ok = (i >= 0) & (i < fill.shape[1]) & (j >= 0) & (j < fill.shape[0]); d = np.full(len(x), 99.0); d[ok] = dout[j[ok], i[ok]]; return d
def ops(path):
    out, cur, x, y, z = {}, None, 0.0, 0.0, 50.0
    for line in open(path):
        line = line.strip()
        m = re.match(r"^\((3D Rough[^)]*)\)$", line)
        if m: cur = out.setdefault(m.group(1), []); continue
        if cur is None or not line.startswith("G"): continue
        g = line.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", line):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        if g in ("G1", "G2", "G3"): cur.append((x, y - 109.31554412841797, z + 23.0, nx, ny - 109.31554412841797, nz + 23.0))
        x, y, z = nx, ny, nz
    return {k: np.array(v) for k, v in out.items()}
g = 0.25; R = 3.0; lo = (X0, Y0); nx, ny = int(fill.shape[1] * pg / g), int(fill.shape[0] * pg / g)
def lower(top, M):
    P = np.vstack([M[:, :3], M[:, 3:]]); seg = M[:, 3:] - M[:, :3]; L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
    idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
    D = np.vstack([M[:, :3], M[idx, :3] + f[:, None] * seg[idx]])
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int); rr = int(math.ceil(R / g)) + 1
    for dj in range(-rr, rr + 1):
        for di in range(-rr, rr + 1):
            I, J = ci + di, cj + dj; cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
            m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g; XX, YY = np.meshgrid(xs, ys)
DC = dist(XX.ravel(), YY.ravel()).reshape(XX.shape); INS = dist(XX.ravel(), YY.ravel()).reshape(XX.shape) == 0
PLACES = [("silhouette", INS), ("outside 0-3", ~INS & (DC <= 3)), ("outside 3-6.2", (DC > 3) & (DC <= 6.2)), ("outside 6.2-9", (DC > 6.2) & (DC <= 9.0)),
          ("outside 9-9.5", (DC > 9.0) & (DC <= 9.5)), ("outside > 9.5", DC > 9.5)]
for path in sys.argv[1:]:
    O = ops(path); o1 = O["3D Rough 1"]; o5 = O["3D Rough 1 [edges, CP rest]"]
    e1 = np.vstack([o1[:, 3:5]]); e5 = o5[:, 3:5]
    d1 = dist(e1[:, 0], e1[:, 1]); d5 = dist(e5[:, 0], e5[:, 1])
    before = np.full((ny, nx), 23.0); lower(before, o1); after = before.copy(); lower(after, o5)
    low = before - after
    print(path.split("phase3/")[1])
    print(f"  op 1 G1 ends: max distance outside {d1.max():.2f} mm; op 5 G1 ends {len(d5)}: > 6.3 mm {int((d5 > 6.3).sum())}, max {d5.max():.2f} mm")
    for name, m in PLACES:
        print(f"  op 5 lowers {name:14s}: area > 0.8 mm {float(((low > 0.8) & m).sum() * g * g):7.0f} mm2, volume {float((low * m).sum() * g * g):7.0f} mm3")
