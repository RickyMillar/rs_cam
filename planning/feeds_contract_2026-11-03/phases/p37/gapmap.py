"""P37 instrument: the stock that the emitted G-code leaves, against the target.

Stock: a height map on the nodes (i * CELL, j * CELL) of the stock box in the
G-code frame (G-code frame = setup frame + (18.5, 8.5); Z is the same). Each
fed move (G1, G2, G3) lowers every node under the tool to the tool surface
(the lowest value along the move, exact for a flat tool and a cone on a
straight move). A rapid (G0) cuts nothing. An arc is cut into chords of at
most 0.05 mm sagitta.

Target: the top of `back_target_s1.stl` at each node (the highest triangle
over the node), moved to the G-code frame.

usage: gapmap.py target | replay <nc> <out.npz> | report <out.npz> <tag>
"""
import math, os, re, struct, sys
import numpy as np
from scipy import ndimage

H = os.path.dirname(os.path.abspath(__file__))
CELL = 0.2
SX, SY, STOCK_TOP = 395.0, 460.0, 25.5
OFF = (18.5, 8.5)
NX, NY = int(round(SX / CELL)) + 1, int(round(SY / CELL)) + 1
STL = "/home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21/back_target_s1.stl"
# tool number -> (kind, radius, half angle in degrees): the [[tools]] of the job
TOOLS = {42: ("flat", 9.0, 0.0), 1: ("flat", 3.0, 0.0), 6: ("flat", 1.0, 0.0),
         2: ("vee", 3.175 / 2, 10.0), 3: ("vee", 6.35 / 2, 30.0)}
SKIP = ("WARNING", "MSG", "feeds", "drill feeds", "Generated", "LOAD", "INFO", "NOTE")


def target():
    out = os.path.join(H, "target.npy")
    if os.path.exists(out):
        return np.load(out)
    b = open(STL, "rb").read()
    n = struct.unpack("<I", b[80:84])[0]
    a = np.frombuffer(b, dtype=np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")]), count=n, offset=84)
    v = a["v"].astype(np.float64)
    v[:, :, 0] += OFF[0]; v[:, :, 1] += OFF[1]
    top = np.full((NY, NX), -np.inf)
    x, y, z = v[:, :, 0], v[:, :, 1], v[:, :, 2]
    i0 = np.ceil(x.min(1) / CELL - 1e-9).astype(int); i1 = np.floor(x.max(1) / CELL + 1e-9).astype(int)
    j0 = np.ceil(y.min(1) / CELL - 1e-9).astype(int); j1 = np.floor(y.max(1) / CELL + 1e-9).astype(int)
    d = (y[:, 1] - y[:, 2]) * (x[:, 0] - x[:, 2]) + (x[:, 2] - x[:, 1]) * (y[:, 0] - y[:, 2])
    ok = (np.abs(d) > 1e-12) & (i1 >= i0) & (j1 >= j0)

    def put(idx, I, J):
        px, py = I * CELL, J * CELL
        w0 = ((y[idx, 1] - y[idx, 2]) * (px - x[idx, 2]) + (x[idx, 2] - x[idx, 1]) * (py - y[idx, 2])) / d[idx]
        w1 = ((y[idx, 2] - y[idx, 0]) * (px - x[idx, 2]) + (x[idx, 0] - x[idx, 2]) * (py - y[idx, 2])) / d[idx]
        w2 = 1 - w0 - w1
        e = -1e-7
        m = (w0 >= e) & (w1 >= e) & (w2 >= e) & (I >= 0) & (I < NX) & (J >= 0) & (J < NY)
        zz = w0 * z[idx, 0] + w1 * z[idx, 1] + w2 * z[idx, 2]
        np.maximum.at(top, (J[m], I[m]), zz[m])

    K = 6
    small = ok & (i1 - i0 < K) & (j1 - j0 < K)
    idx = np.nonzero(small)[0]
    for dj in range(K):
        for di in range(K):
            put(idx, i0[idx] + di, j0[idx] + dj)
    for t in np.nonzero(ok & ~small)[0]:
        I, J = np.meshgrid(np.arange(i0[t], i1[t] + 1), np.arange(j0[t], j1[t] + 1))
        put(np.full(I.size, t), I.ravel(), J.ravel())
    np.save(out, top)
    return top


def moves(nc):
    """Yield (op, tool, kind, p0, p1) for each fed straight piece."""
    pos = [0.0, 0.0, 36.0]; tool = None; op = None; mode = None
    word = re.compile(r"([GXYZIJFTM])(-?\d+\.?\d*)")
    for line in open(nc):
        line = line.strip()
        if line.startswith("("):
            t = line[1:-1]
            if not t.startswith(SKIP):
                op = t
            continue
        w = word.findall(line)
        if not w:
            continue
        d = {}
        for k, val in w:
            if k == "G":
                g = int(float(val))
                if g in (0, 1, 2, 3):
                    mode = g
            elif k == "T":
                tool = int(val)
            elif k != "M":
                d[k] = float(val)
        if not any(k in d for k in "XYZ"):
            continue
        new = [d.get("X", pos[0]), d.get("Y", pos[1]), d.get("Z", pos[2])]
        if mode == 1:
            yield op, tool, tuple(pos), tuple(new)
        elif mode in (2, 3):
            cx, cy = pos[0] + d.get("I", 0.0), pos[1] + d.get("J", 0.0)
            r = math.hypot(pos[0] - cx, pos[1] - cy)
            a0 = math.atan2(pos[1] - cy, pos[0] - cx); a1 = math.atan2(new[1] - cy, new[0] - cx)
            if mode == 2:
                sweep = (a0 - a1) % (2 * math.pi) or 2 * math.pi; sg = -1
            else:
                sweep = (a1 - a0) % (2 * math.pi) or 2 * math.pi; sg = 1
            n = max(2, int(math.ceil(sweep / (2 * math.acos(max(0.0, 1 - 0.05 / max(r, 0.06)))))))
            prev = tuple(pos)
            for k in range(1, n + 1):
                f = k / n; a = a0 + sg * sweep * f
                q = (cx + r * math.cos(a), cy + r * math.sin(a), pos[2] + f * (new[2] - pos[2])) if k < n else tuple(new)
                yield op, tool, prev, q
                prev = q
        pos = new


def stamp(top, tool, p0, p1):
    kind, R, half = TOOLS[tool]
    x0, y0, z0 = p0; x1, y1, z1 = p1
    ia = max(0, int(math.ceil((min(x0, x1) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(x0, x1) + R) / CELL)))
    ja = max(0, int(math.ceil((min(y0, y1) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(y0, y1) + R) / CELL)))
    if ib < ia or jb < ja:
        return
    X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
    dx, dy = x1 - x0, y1 - y0
    L2 = dx * dx + dy * dy
    sub = top[ja:jb + 1, ia:ib + 1]
    if L2 < 1e-18:
        d2 = (X - x0) ** 2 + (Y - y0) ** 2
        zl = min(z0, z1)
        if kind == "flat":
            h = np.where(d2 <= R * R, zl, np.inf)
        else:
            h = np.where(d2 <= R * R, zl + np.sqrt(d2) / math.tan(math.radians(half)), np.inf)
        np.minimum(sub, h, out=sub)
        return
    L = math.sqrt(L2)
    t = ((X - x0) * dx + (Y - y0) * dy) / L2          # projection parameter
    perp2 = ((X - x0) * dy - (Y - y0) * dx) ** 2 / L2  # squared distance to the line
    if kind == "flat":
        ok = perp2 <= R * R
        half_t = np.sqrt(np.where(ok, R * R - perp2, 0.0)) / L
        ta = np.clip(t - half_t, 0.0, 1.0); tb = np.clip(t + half_t, 0.0, 1.0)
        ok &= (t + half_t >= 0.0) & (t - half_t <= 1.0)
        te = tb if z1 < z0 else ta
        h = np.where(ok, z0 + te * (z1 - z0), np.inf)
    else:
        # cone: height over a node = tip z(t) + dist(t) / tan(half); sample the ends and the foot
        k = 1.0 / math.tan(math.radians(half))
        tc = np.clip(t, 0.0, 1.0)
        best = np.full(t.shape, np.inf)
        s = k * (z1 - z0)  # shift of the lowest point along the line for a sloped move
        cands = [np.zeros_like(tc), np.ones_like(tc), tc]
        if abs(z1 - z0) > 1e-12:
            # minimise z0 + t dz + k sqrt(perp2 + (t - t_f)^2 L2): stationary point
            dz = z1 - z0
            den = k * k * L2 - dz * dz
            if den > 1e-12:
                off = -dz * np.sqrt(perp2 / (L2 * den)) * np.sign(1.0)
                cands.append(np.clip(t + off, 0.0, 1.0))
        for tt in cands:
            dist = np.sqrt(perp2 + ((tt - t) ** 2) * L2)
            hh = np.where(dist <= R, z0 + tt * (z1 - z0) + dist * k, np.inf)
            best = np.minimum(best, hh)
        h = best
    np.minimum(sub, h, out=sub)


def replay(nc, out):
    top = np.full((NY, NX), STOCK_TOP)
    snaps = {}; names = []; cur = None; n = 0
    for op, tool, p0, p1 in moves(nc):
        if op != cur:
            if cur is not None:
                snaps[len(names) - 1] = top.astype(np.float32).copy()
            names.append(op); cur = op
        stamp(top, tool, p0, p1); n += 1
    snaps[len(names) - 1] = top.astype(np.float32).copy()
    np.savez_compressed(out, names=np.array(names), **{f"s{k}": v for k, v in snaps.items()})
    print("fed pieces", n, "ops", names)


if __name__ == "__main__":
    if sys.argv[1] == "target":
        t = target(); print(t.shape, np.isfinite(t).sum(), np.nanmin(t[np.isfinite(t)]), t.max())
    elif sys.argv[1] == "replay":
        replay(sys.argv[2], sys.argv[3])


def disc(R):
    r = int(math.floor(R / CELL + 1e-9))
    yy, xx = np.mgrid[-r:r + 1, -r:r + 1]
    return (xx * xx + yy * yy) * CELL * CELL <= R * R + 1e-9


def reach(T, R):
    """The lowest surface a flat tool of radius R makes over the target: at each
    centre the tool sits on the highest target under its disc; a node takes the
    lowest tool that covers it. Outside the model the target is the stock top
    (the tool does not go there in this reading)."""
    Tf = np.where(np.isfinite(T), T, STOCK_TOP)
    fp = disc(R)
    return ndimage.grey_erosion(ndimage.grey_dilation(Tf, footprint=fp, mode="nearest"), footprint=fp, mode="nearest")


def islands(gap, bar, extra=None):
    lab, n = ndimage.label(gap > bar)
    idx = np.arange(1, n + 1)
    area = ndimage.sum_labels(np.ones_like(gap), lab, idx) * CELL * CELL
    mx = ndimage.maximum(gap, lab, idx)
    sl = ndimage.find_objects(lab)
    pos = ndimage.maximum_position(gap, lab, idx)
    rows = []
    for k in range(n):
        s = sl[k]
        rows.append(dict(area=float(area[k]), max=float(mx[k]), peak=(pos[k][1] * CELL, pos[k][0] * CELL),
                         bbox=(s[1].start * CELL, s[0].start * CELL, (s[1].stop - 1) * CELL, (s[0].stop - 1) * CELL)))
    rows.sort(key=lambda r: -r["area"])
    return rows, lab


BAR = 0.3   # one cell (0.2) plus the 0.1 mm tolerance of the ops
# the cavity outline of cavity_d9.00.dxf in the G-code frame (3 mm corner radius ignored here)
CAV = (24.5 + OFF[0], 109.5 + OFF[1], 333.5 + OFF[0], 418.5 + OFF[1])
CLASSES = ["cavity wall band (<= 5 mm inside the outline)", "cavity interior (lakes, hollow, rivers)",
           "step shelf (Y 104..118)", "slot X 243..292 Y 53..64", "mech and slit pockets (Y < 104)", "other"]


def classify():
    J, I = np.mgrid[0:NY, 0:NX]
    X, Y = I * CELL, J * CELL
    x0, y0, x1, y1 = CAV
    inside = (X > x0 - 0.05) & (X < x1 + 0.05) & (Y > y0 - 0.05) & (Y < y1 + 0.05)
    din = np.minimum(np.minimum(X - x0, x1 - X), np.minimum(Y - y0, y1 - Y))
    c = np.full((NY, NX), 5, dtype=np.int8)
    c[Y < 104] = 4
    c[(X > 243) & (X < 292) & (Y > 53) & (Y < 64)] = 3
    c[(Y >= 104) & (Y <= y0 - 0.05)] = 2
    c[inside] = 1
    c[inside & (din <= 5.0)] = 0
    return c


def report(npz, tag, snap=None, png=True):
    d = np.load(npz); names = list(d["names"])
    k = len(names) - 1 if snap is None else snap
    S = d[f"s{k}"].astype(float)
    T = target(); ok = np.isfinite(T)
    rp = os.path.join(H, "reach6.npy")
    R6 = np.load(rp) if os.path.exists(rp) else reach(T, 3.0)
    if not os.path.exists(rp):
        np.save(rp, R6)
    gapT = np.where(ok, S - T, 0.0); gap6 = np.where(ok, S - R6, 0.0)
    c = classify()
    print(f"== {tag}: stock after '{names[k]}' ; cell {CELL} mm ; bar {BAR} mm ; model area {ok.sum()*CELL*CELL:.0f} mm2")
    print(f"above the target by > bar: {(gapT>BAR).sum()*CELL*CELL:.1f} mm2 ; of it by design (a 6 mm flat tool cannot reach: inside corners, narrow floors): {((gapT>BAR)&(gap6<=BAR)).sum()*CELL*CELL:.1f} mm2 ; above the 6 mm reach surface: {(gap6>BAR).sum()*CELL*CELL:.1f} mm2")
    out = []
    for ci, name in enumerate(CLASSES):
        m = (gap6 > BAR) & (c == ci)
        if not m.any():
            print(f"  {name:50s}      0.0 mm2"); out.append((name, 0.0, 0.0, None)); continue
        g = np.where(m, gap6, -1); j, i = np.unravel_index(np.argmax(g), g.shape)
        vol = (gap6[m]).sum() * CELL * CELL
        print(f"  {name:50s} {m.sum()*CELL*CELL:8.1f} mm2  tallest {g[j,i]:5.2f} mm at G-code ({i*CELL:.1f}, {j*CELL:.1f})  volume {vol:8.0f} mm3")
        out.append((name, m.sum() * CELL * CELL, float(g[j, i]), (i * CELL, j * CELL)))
    if png:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        fig, ax = plt.subplots(figsize=(11, 12))
        im = ax.imshow(np.where(ok, np.clip(gap6, -1, 10), np.nan), origin="lower", extent=(0, SX, 0, SY), cmap="turbo", vmin=-1, vmax=10)
        plt.colorbar(im, ax=ax, label="stock above the 6 mm reach surface of the target (mm, clipped -1..10)")
        ax.set_title(f"{tag}: after '{names[k]}' (G-code frame, cell {CELL} mm)")
        plt.tight_layout(); plt.savefig(os.path.join(H, f"gap_{tag}.png"), dpi=80); plt.close(fig)
    return out, S


def holes_top(S, R=3.2):
    """The highest stock under the footprint (radius R) of each light hole named in P36."""
    J, I = np.mgrid[0:NY, 0:NX]
    res = {}
    for (x, y) in [(327.13, 412.17), (327.13, 379.02), (327.13, 345.87), (327.13, 312.72)]:
        gx, gy = x + OFF[0], y + OFF[1]
        res[(x, y)] = float(S[((I * CELL - gx) ** 2 + (J * CELL - gy) ** 2 <= R * R)].max())
    return res


def op_times(nc):
    """Per op: the fed length (mm), the feed time at the emitted F (s, no
    acceleration) and the rapid length (mm)."""
    import collections
    out = collections.OrderedDict()
    pos = [0.0, 0.0, 36.0]; op = None; mode = None; F = None
    word = re.compile(r"([GXYZIJFTM])(-?\d+\.?\d*)")
    for line in open(nc):
        line = line.strip()
        if line.startswith("("):
            t = line[1:-1]
            if not t.startswith(SKIP):
                op = t; out.setdefault(op, [0.0, 0.0, 0.0])
            continue
        d = {}
        for k, val in word.findall(line):
            if k == "G":
                g = int(float(val))
                if g in (0, 1, 2, 3):
                    mode = g
            elif k == "F":
                F = float(val)
            elif k in "XYZIJ":
                d[k] = float(val)
        if not any(k in d for k in "XYZ") or op is None:
            continue
        new = [d.get("X", pos[0]), d.get("Y", pos[1]), d.get("Z", pos[2])]
        L = math.dist(pos, new)
        if mode in (2, 3):
            r = math.hypot(d.get("I", 0.0), d.get("J", 0.0)); c = math.hypot(new[0] - pos[0], new[1] - pos[1])
            if r > 1e-9 and c <= 2 * r:
                a = 2 * math.asin(c / (2 * r)); L = math.hypot(r * a, new[2] - pos[2])
        if mode == 0:
            out[op][2] += L
        elif F:
            out[op][0] += L; out[op][1] += L / F * 60.0
        pos = new
    return out
