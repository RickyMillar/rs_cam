"""Find standing spurs (tall thin material) in the stock left by the exported G-code.

Model: a heightmap of the stock, top at G-code Z 0 (the export zeroes Z at
the stock top), on a square grid. Each fed move (G1, G2, G3) of each op is
sampled every 0.25 mm and stamps a flat end mill of the op's diameter: the
height of every cell under the tool becomes min(height, tool tip Z). G0 moves
do not cut. Ball and bull tools are out of scope.

A spur is material narrower than 2 * open_r that stands more than min_h
above the grey opening of the heightmap with a disk of radius open_r (the
opening removes every feature narrower than its disk). The script labels
each connected spur patch and prints: centroid XY (G-code frame), top Z,
height over the opened surface, width (area / length of the patch), and
the move index (within its op) that last cut within 1 mm of the patch, and
whether the spur is FREE-STANDING: the opened surface in a 2 mm ring round
the patch is lower than the spur top by more than min_h (no wide material
beside it). An attached spur leans on a wall or a corner residue; a
free-standing one is the snap risk.

Note: a thin wall of the PART also counts as a spur here. Read each patch
against the part shape before you call it a defect.

Usage:
  python3 spurs.py NC --op "3D Rough 1" 8006 6.0 [--op NAME MOVES DIAM ...]
         [--stop-op NAME --stop-move M] [--res 0.2] [--open-r 1.0] [--min-h 1.5]
         [--png out.png] [--title T]
"""
import argparse
import math
import re

import numpy as np
from scipy import ndimage

from links_entries import section

WORD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")


def path_points(cmds):
    """Yield (move index, g, list of (x, y, z) samples) per command."""
    pos = [0.0, 0.0, 0.0]
    g = 0
    for k, s in enumerate(cmds):
        w = dict((a, float(b)) for a, b in WORD.findall(s))
        g = int(w.get("G", g))
        end = [w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2])]
        pts = []
        if g in (2, 3):
            cx, cy = pos[0] + w.get("I", 0.0), pos[1] + w.get("J", 0.0)
            r = math.hypot(pos[0] - cx, pos[1] - cy)
            a0 = math.atan2(pos[1] - cy, pos[0] - cx)
            a1 = math.atan2(end[1] - cy, end[0] - cx)
            da = (a1 - a0) % (2 * math.pi) if g == 3 else -((a0 - a1) % (2 * math.pi))
            if abs(da) < 1e-9:
                da = 2 * math.pi if g == 3 else -2 * math.pi
            n = max(2, int(abs(da) * r / 0.25) + 1)
            for i in range(n + 1):
                t = i / n
                a = a0 + da * t
                pts.append((cx + r * math.cos(a), cy + r * math.sin(a), pos[2] + (end[2] - pos[2]) * t))
        else:
            n = max(1, int(math.dist(pos, end) / 0.25) + 1)
            for i in range(n + 1):
                t = i / n
                pts.append(tuple(pos[j] + (end[j] - pos[j]) * t for j in range(3)))
        yield k, g, pts
        pos = end


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("nc")
    ap.add_argument("--op", nargs=3, action="append", metavar=("NAME", "MOVES", "DIAM"), required=True)
    ap.add_argument("--stop-op")
    ap.add_argument("--stop-move", type=int)
    ap.add_argument("--res", type=float, default=0.2)
    ap.add_argument("--open-r", type=float, default=1.0)
    ap.add_argument("--min-h", type=float, default=1.5)
    ap.add_argument("--floor", type=float, default=-30.0)
    ap.add_argument("--show", type=int, default=15)
    ap.add_argument("--box", type=float, nargs=4, help="PNG extent X0 X1 Y0 Y1")
    ap.add_argument("--png")
    ap.add_argument("--title", default="")
    a = ap.parse_args()
    lines = open(a.nc).read().splitlines()

    ops = []
    xs, ys = [], []
    for name, moves, diam in a.op:
        cmds = section(lines, name)
        moves = int(moves)
        if len(cmds) - moves not in (1, 2):
            raise SystemExit(f"{name}: mapping not safe")
        segs = list(path_points(cmds))[1:1 + moves]
        ops.append((name, float(diam), segs))
        for _, g, pts in segs:
            if g != 0:
                xs += [p[0] for p in pts]
                ys += [p[1] for p in pts]
    x0, x1 = min(xs) - 5, max(xs) + 5
    y0, y1 = min(ys) - 5, max(ys) + 5
    nx, ny = int((x1 - x0) / a.res) + 1, int((y1 - y0) / a.res) + 1
    h = np.zeros((ny, nx), dtype=np.float32)
    last = np.full((ny, nx), -1, dtype=np.int32)

    done = False
    for name, diam, segs in ops:
        r = diam / 2
        rc = int(math.ceil(r / a.res))
        oy, ox = np.mgrid[-rc:rc + 1, -rc:rc + 1]
        disk = (ox * a.res) ** 2 + (oy * a.res) ** 2 <= r * r
        for k, g, pts in segs:
            if a.stop_op == name and a.stop_move is not None and k > a.stop_move:
                done = True
                break
            if g == 0:
                continue
            for x, y, z in pts:
                cx, cy = int(round((x - x0) / a.res)), int(round((y - y0) / a.res))
                ys_, xs_ = slice(cy - rc, cy + rc + 1), slice(cx - rc, cx + rc + 1)
                sub = h[ys_, xs_]
                m = disk & (sub > z)
                sub[m] = z
                last[ys_, xs_][disk] = k
        if done:
            break

    ro = int(math.ceil(a.open_r / a.res))
    oy, ox = np.mgrid[-ro:ro + 1, -ro:ro + 1]
    fp = (ox ** 2 + oy ** 2) <= ro * ro
    opened = ndimage.grey_opening(h, footprint=fp)
    excess = h - opened
    mask = excess > a.min_h
    lab, n = ndimage.label(mask)
    rows = []
    for i in range(1, n + 1):
        cells = np.argwhere(lab == i)
        if len(cells) < 3:
            continue
        cyx = cells.mean(axis=0)
        ext = cells.max(axis=0) - cells.min(axis=0) + 1
        length = max(ext) * a.res
        area = len(cells) * a.res * a.res
        top = float(h[lab == i].max())
        hgt = float(excess[lab == i].max())
        ring = ndimage.binary_dilation(lab == i, iterations=int(1.0 / a.res)) & ~(lab == i)
        mv = int(last[ring].max()) if ring.any() else -1
        ring2 = ndimage.binary_dilation(lab == i, iterations=int(2.0 / a.res)) & ~(lab == i)
        free = bool(ring2.any() and float(opened[ring2].max()) < top - a.min_h)
        rows.append((hgt, x0 + cyx[1] * a.res, y0 + cyx[0] * a.res, top, area / length, length, mv, free))
    rows.sort(key=lambda r: (not r[7], -r[0]))
    nfree = sum(1 for r in rows if r[7])
    print(f"grid {nx}x{ny} at {a.res} mm; open radius {a.open_r} mm; min height {a.min_h} mm; spurs: {len(rows)}, free-standing: {nfree}")
    for hgt, x, y, top, wid, length, mv, free in rows[:a.show]:
        print(f"  {'FREE' if free else 'att '} XY ({x:7.1f},{y:7.1f})  top Z {top:6.2f}  height {hgt:5.2f}  width {wid:4.2f}  length {length:5.1f}  last cut by move {mv}")
    if a.png:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        fig, ax = plt.subplots(figsize=(10, 10 * ny / nx), dpi=110)
        ax.imshow(h, origin="lower", extent=(x0, x1, y0, y1), cmap="viridis")
        sy, sx = np.nonzero(mask)
        ax.scatter(x0 + sx * a.res, y0 + sy * a.res, s=1, color="red")
        fr = [r for r in rows if r[7]]
        ax.scatter([r[1] for r in fr], [r[2] for r in fr], s=60, facecolors="none", edgecolors="white", linewidths=1.2)
        for k, r in enumerate(fr[:12]):
            ax.annotate(f"S{k + 1}", (r[1], r[2]), color="white", fontsize=8, weight="bold")
        if a.box:
            ax.set_xlim(a.box[0], a.box[1])
            ax.set_ylim(a.box[2], a.box[3])
        ax.set_title(a.title + f"\n(red: material > {a.min_h} mm tall and < {2 * a.open_r} mm wide; white rings S1..: free-standing spurs)", fontsize=9)
        fig.tight_layout()
        fig.savefig(a.png)
    return rows


if __name__ == "__main__":
    main()
