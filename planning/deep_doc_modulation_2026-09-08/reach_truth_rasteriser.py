#!/usr/bin/env python3
"""Ground truth for the reach question on the wanaka terrain, from the STL
alone — no rs_cam code in the loop.

Step 1 rasterizes the TIN onto a regular grid by exact barycentric plane
evaluation (max Z where triangles overlap), so the height field is the same
surface `surface_z_at` reads.

Step 2 takes the grayscale CLOSING of that field by the ball profile:

    tip_z(p)      = max over q of [ z(q) - h(|p-q|) ]     (drop cutter)
    machined_z(x) = min over p of [ tip_z(p) + h(|p-x|) ]  (min filter)
    gap           = machined_z - z

the same law `reach_map.rs` implements, run at a pitch four times finer than
any cell the map picks. The unreachable share it reports is therefore the
answer the map is TRYING to give.
"""

import sys
import time

import numpy as np

PATH = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl"
CELL = 0.15
SLOT_CAP = 6  # bbox slots handled vectorised; bigger triangles go one by one


def say(*args):
    print(*args, flush=True)


def load_tris(path):
    with open(path, "rb") as fh:
        head = fh.read(84)
        n = int.from_bytes(head[80:84], "little")
        raw = np.frombuffer(fh.read(n * 50), dtype=np.uint8)
    raw = raw.reshape(n, 50)
    floats = raw[:, :48].copy().view(np.float32).reshape(n, 12)
    return floats[:, 3:12].reshape(n, 3, 3).astype(np.float64)


def rasterize(tris, cell):
    x0 = tris[:, :, 0].min()
    y0 = tris[:, :, 1].min()
    x1 = tris[:, :, 0].max()
    y1 = tris[:, :, 1].max()
    nx = int(np.ceil((x1 - x0) / cell)) + 1
    ny = int(np.ceil((y1 - y0) / cell)) + 1
    say(f"raster {ny} x {nx} at {cell} mm  ({x0:.2f}..{x1:.2f}, {y0:.2f}..{y1:.2f})")
    z = np.full((ny, nx), -np.inf)

    ax, ay, az = tris[:, 0, 0], tris[:, 0, 1], tris[:, 0, 2]
    bx, by, bz = tris[:, 1, 0], tris[:, 1, 1], tris[:, 1, 2]
    cx, cy, cz = tris[:, 2, 0], tris[:, 2, 1], tris[:, 2, 2]
    det = (by - ay) * (cx - ax) - (bx - ax) * (cy - ay)
    ok = np.abs(det) > 1e-12

    lo_c = np.floor((np.minimum(np.minimum(ax, bx), cx) - x0) / cell).astype(np.int64)
    hi_c = np.ceil((np.maximum(np.maximum(ax, bx), cx) - x0) / cell).astype(np.int64)
    lo_r = np.floor((np.minimum(np.minimum(ay, by), cy) - y0) / cell).astype(np.int64)
    hi_r = np.ceil((np.maximum(np.maximum(ay, by), cy) - y0) / cell).astype(np.int64)
    w = hi_c - lo_c + 1
    h = hi_r - lo_r + 1
    small = ok & (w <= SLOT_CAP) & (h <= SLOT_CAP)
    say(f"triangles {len(tris)}  small {small.sum()}  large {(ok & ~small).sum()}"
        f"  slots mean {(w * h).mean():.2f} max {(w * h).max()}")

    idx = np.arange(len(tris))[small]
    for dr in range(SLOT_CAP):
        for dc in range(SLOT_CAP):
            r = lo_r[idx] + dr
            c = lo_c[idx] + dc
            live = (r <= hi_r[idx]) & (c <= hi_c[idx])
            live &= (r >= 0) & (c >= 0) & (r < ny) & (c < nx)
            if not live.any():
                continue
            s = idx[live]
            rr, cc = r[live], c[live]
            px = x0 + cc * cell
            py = y0 + rr * cell
            w0 = ((by[s] - ay[s]) * (px - ax[s]) - (bx[s] - ax[s]) * (py - ay[s])) / det[s]
            w1 = ((py - ay[s]) * (cx[s] - ax[s]) - (px - ax[s]) * (cy[s] - ay[s])) / det[s]
            inside = (w0 >= -1e-9) & (w1 >= -1e-9) & (w0 + w1 <= 1.0 + 1e-9)
            if not inside.any():
                continue
            t = s[inside]
            zz = az[t] + w1[inside] * (bz[t] - az[t]) + w0[inside] * (cz[t] - az[t])
            np.maximum.at(z, (rr[inside], cc[inside]), zz)

    for i in np.arange(len(tris))[ok & ~small]:
        for rr in range(max(lo_r[i], 0), min(hi_r[i] + 1, ny)):
            for cc in range(max(lo_c[i], 0), min(hi_c[i] + 1, nx)):
                px = x0 + cc * cell
                py = y0 + rr * cell
                w0 = ((by[i] - ay[i]) * (px - ax[i]) - (bx[i] - ax[i]) * (py - ay[i])) / det[i]
                w1 = ((py - ay[i]) * (cx[i] - ax[i]) - (px - ax[i]) * (cy[i] - ay[i])) / det[i]
                if w0 < -1e-9 or w1 < -1e-9 or w0 + w1 > 1.0 + 1e-9:
                    continue
                zz = az[i] + w1 * (bz[i] - az[i]) + w0 * (cz[i] - az[i])
                if zz > z[rr, cc]:
                    z[rr, cc] = zz

    say(f"filled {np.isfinite(z).mean() * 100:.2f}%")
    return np.where(np.isfinite(z), z, np.nan)


def ball_taps(radius_mm, cell):
    k = int(np.floor(radius_mm / cell))
    taps = []
    for dr in range(-k, k + 1):
        for dc in range(-k, k + 1):
            r = cell * np.hypot(dr, dc)
            if r > radius_mm:
                continue
            taps.append((dr, dc, radius_mm - np.sqrt(max(radius_mm ** 2 - r ** 2, 0.0))))
    return taps


def shift_reduce(field, taps, want_max):
    out = None
    ny, nx = field.shape
    buf = np.empty_like(field)
    for dr, dc, rise in taps:
        buf.fill(np.nan)
        r0, r1 = max(dr, 0), ny + min(dr, 0)
        c0, c1 = max(dc, 0), nx + min(dc, 0)
        buf[r0 - dr:r1 - dr, c0 - dc:c1 - dc] = field[r0:r1, c0:c1]
        if want_max:
            buf -= rise
            out = buf.copy() if out is None else np.fmax(out, buf, out=out)
        else:
            buf += rise
            out = buf.copy() if out is None else np.fmin(out, buf, out=out)
    return out


def main():
    t0 = time.time()
    tris = load_tris(PATH)
    say(f"load {time.time() - t0:.1f}s")
    z = rasterize(tris, CELL)
    say(f"raster done {time.time() - t0:.1f}s")
    np.save("wanaka_dem_015.npy", z)

    d2x = z[1:-1, :-2] - 2 * z[1:-1, 1:-1] + z[1:-1, 2:]
    fin = np.isfinite(d2x)
    with np.errstate(divide="ignore", invalid="ignore"):
        rho = CELL ** 2 / np.abs(d2x[fin])
    say(
        f"x-axis curvature radius mm: median {np.median(rho):.3f}  "
        f"p10 {np.percentile(rho, 10):.3f}  p90 {np.percentile(rho, 90):.3f}  "
        f"share rho < 2.0: {np.mean(rho < 2.0) * 100:.2f}%"
    )
    say(f"|d2z| mm at {CELL} mm: median {np.median(np.abs(d2x[fin])):.4f}  "
        f"p90 {np.percentile(np.abs(d2x[fin]), 90):.4f}")

    for radius in (2.0, 1.5, 1.0, 0.5):
        taps = ball_taps(radius, CELL)
        tip = shift_reduce(z, taps, True)
        machined = shift_reduce(tip, taps, False)
        gap = np.maximum(machined - z, 0.0)
        g = gap[np.isfinite(gap)]
        row = [f"R{radius:.1f} taps {len(taps):5d}  n {len(g)}"]
        for tol in (0.05, 0.146, 0.3):
            row.append(f"tol{tol}: {np.mean(g > tol) * 100:6.2f}%")
        row.append(f"max {g.max():.3f}")
        row.append(f"med {np.median(g):.4f}")
        row.append(f"p90 {np.percentile(g, 90):.4f}")
        say("  ".join(row) + f"   [{time.time() - t0:.1f}s]")


if __name__ == "__main__":
    main()
