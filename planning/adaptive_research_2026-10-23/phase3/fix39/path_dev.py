"""D58: the fed path (and the feed at each point) of op 1, R against product, as dense points (lines and G2/G3 arcs in XY with Z linear, a point every
0.05 mm); the deviation each way (nearest point of the other path, cKDTree, 3D). Arc fitting (tolerance 0.05) groups moves
by feed, so a feed change can change the G-code text without moving the path. Usage: path_dev.py <R.nc> <product.nc> <op>"""
import math, re, sys
import numpy as np
from scipy.spatial import cKDTree
def pts(path, name):
    on = False; x = y = 0.0; z = 50.0; P = []; segs = 0; F = []; f = 0.0
    for ln in open(path):
        ln = ln.strip()
        if ln.startswith("("):
            if re.match(r"^\((2 Back Rough|3D Rough 1)", ln): on = ln == f"({name})"
            continue
        if not on or not ln or ln[0] != "G": continue
        m = re.search(r"F(-?[\d.]+)", ln)
        if m: f = float(m.group(1))
        g = ln.split()[0]; v = dict(re.findall(r"([XYZIJ])(-?[\d.]+)", ln)); nx, ny, nz = float(v.get("X", x)), float(v.get("Y", y)), float(v.get("Z", z))
        if g == "G1":
            n = max(1, int(math.dist((x, y, z), (nx, ny, nz)) / 0.05)); t = np.linspace(0, 1, n + 1)[1:]
            P.append(np.c_[x + t * (nx - x), y + t * (ny - y), z + t * (nz - z)]); F.append(np.full(len(t), f)); segs += 1
        elif g in ("G2", "G3"):
            cx, cy = x + float(v["I"]), y + float(v["J"]); r = math.hypot(x - cx, y - cy)
            a0 = math.atan2(y - cy, x - cx); a1 = math.atan2(ny - cy, nx - cx); da = a1 - a0
            if g == "G3" and da <= 0: da += 2 * math.pi
            if g == "G2" and da >= 0: da -= 2 * math.pi
            n = max(1, int(abs(da) * r / 0.05)); t = np.linspace(0, 1, n + 1)[1:]
            P.append(np.c_[cx + r * np.cos(a0 + t * da), cy + r * np.sin(a0 + t * da), z + t * (nz - z)]); F.append(np.full(len(t), f)); segs += 1
        x, y, z = nx, ny, nz
    return np.vstack(P), segs, np.concatenate(F)
a, na, fa = pts(sys.argv[1], sys.argv[3]); b, nb, fb = pts(sys.argv[2], sys.argv[3])
da, _ = cKDTree(b).query(a); db, ib = cKDTree(a).query(b)
print(f"fed segments R {na}, product {nb}; dense points {len(a)} / {len(b)}")
print(f"R -> product: max {da.max():.4f} mm, p99.9 {np.percentile(da, 99.9):.4f}, points > 0.06 mm: {(da > 0.06).sum()}")
print(f"product -> R: max {db.max():.4f} mm, p99.9 {np.percentile(db, 99.9):.4f}, points > 0.06 mm: {(db > 0.06).sum()}")
if da.max() > 0.06: i = int(np.argmax(da)); print("  worst R point", a[i].round(3))
if db.max() > 0.06: i = int(np.argmax(db)); print("  worst product point", b[i].round(3))
fr = fa[ib]; step = 0.05
print(f"fed length by feed, product against R (nearest R point): equal {(abs(fb - fr) <= 0.01 * fr).sum() * step / 1000:.1f} m, "
      f"product slower {(fb < 0.99 * fr).sum() * step:.0f} mm, product faster {(fb > 1.01 * fr).sum() * step:.0f} mm")
