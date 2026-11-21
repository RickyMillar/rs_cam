"""P45 step 1: the trace of the replay pieces of the strip pass.

Input: the probe dumps of `p45/probe1.patch` (env P45PROBE=<dir>): per round of the strip
pass `stock_<n>.bin` (the planner stock before the round: ray top and conservative top of
each cell), `points_<n>.txt` (each ring point: x y, its own Z read, the Z after the
neighbour rule, standing, refused, the strip top), `runs_<n>.txt` (the runs that ship).
And the G-code of the same run.

For each counted node of each replay piece (p43/deepc.py class) it prints:
- the replay top before the piece (a point sample at the node), and the lowest Z of the
  piece over the node;
- the planner cell of the node before the round of the piece: ray top, conservative top;
- each ring point of that round whose disc holds the cell centre: its own Z read and what
  the read gave for this cell (ray top less Depth/Pass);
- the cause class: A the planner top is under the replay top (a blended cell);
  B no ring point of the round holds the cell centre in its disc; C the reads are right
  and the shipped chord is under them.
usage: trace.py <probe dir> <nc> <op prefix> <bar> <depth per pass> [frame dx dy]
"""
import sys, os, math, struct, glob
import numpy as np
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "p37"))
from gapmap import moves, stamp, TOOLS, CELL, NX, NY, STOCK_TOP

d, nc, pref, bar, dpp = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]), float(sys.argv[5])
DX, DY = (float(sys.argv[6]), float(sys.argv[7])) if len(sys.argv) > 7 else (0.0, 0.0)
TOL = 0.1


def load_stock(path):
    b = open(path, "rb").read()
    rows, cols = struct.unpack("<QQ", b[:16]); ou, ov, cs = struct.unpack("<ddd", b[16:40])
    a = np.frombuffer(b, dtype="<f4", offset=40).reshape(2, rows, cols)
    return dict(rows=rows, cols=cols, ou=ou, ov=ov, cs=cs, top=a[0], cons=a[1])


rounds = sorted(glob.glob(os.path.join(d, "points_*.txt")), key=lambda p: int(p.split("_")[-1][:-4]))
pts = [np.loadtxt(p, ndmin=2) for p in rounds]          # round ring i x y own z stand refused top
runs = []
for k in range(len(rounds)):
    rr = []
    for line in open(os.path.join(d, f"runs_{k}.txt")):
        rr.append(np.array([[float(v) for v in t.split(",")] for t in line.split()]))
    runs.append(rr)
st0 = load_stock(os.path.join(d, "stock_0.bin"))
print(f"planner grid: rows {st0['rows']} cols {st0['cols']} origin ({st0['ou']:.4f}, {st0['ov']:.4f}) cell {st0['cs']}; "
      f"G-code frame = planner frame + ({DX}, {DY}); rounds {len(rounds)}; ring points per round {[len(p) for p in pts]}; runs {[len(r) for r in runs]}")

# The round of a G-code move: the run of a round that holds both ends of the move (XY within 2 um).
def round_of(p0, p1):
    for k, rr in enumerate(runs):
        for run in rr:
            if len(run) == 0: continue
            xy = run[:, :2] + (DX, DY)
            d0 = np.hypot(xy[:, 0] - p0[0], xy[:, 1] - p0[1]).min(); d1 = np.hypot(xy[:, 0] - p1[0], xy[:, 1] - p1[1]).min()
            if d0 < 0.25 and d1 < 0.25 and abs(run[:, 2].min() - min(p0[2], p1[2])) < 3.0:
                # the best match is the round whose run Z at the nearest points is nearest
                i0 = np.hypot(xy[:, 0] - p0[0], xy[:, 1] - p0[1]).argmin(); i1 = np.hypot(xy[:, 0] - p1[0], xy[:, 1] - p1[1]).argmin()
                yield k, d0, d1, abs(run[i0, 2] - p0[2]) + abs(run[i1, 2] - p1[2])


top = np.full((NY, NX), STOCK_TOP)
k = 0; started = False
last_cut = {}                       # node -> (op, move index, z) of the last replay cut
near = {}                           # filled per piece
hist = []                           # (op, kidx, tool, p0, p1) of ids before, kept for the graze list
allmoves = []
for op, tool, p0, p1 in moves(nc):
    allmoves.append((op, tool, p0, p1))
idx_in_op = 0
for n, (op, tool, p0, p1) in enumerate(allmoves):
    if op is None or not op.startswith(pref):
        if started: break
        stamp(top, tool, p0, p1); continue
    started = True
    kind, R, half = TOOLS[tool]
    ia = max(0, int(math.ceil((min(p0[0], p1[0]) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(p0[0], p1[0]) + R) / CELL)))
    ja = max(0, int(math.ceil((min(p0[1], p1[1]) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(p0[1], p1[1]) + R) / CELL)))
    before = top[ja:jb + 1, ia:ib + 1].copy()
    stamp(top, tool, p0, p1)
    dd = before - top[ja:jb + 1, ia:ib + 1]
    deep = dd > bar
    if deep.any():
        X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
        t = np.clip(((X - p0[0]) * dx + (Y - p0[1]) * dy) / L2, 0, 1) if L2 > 1e-18 else np.zeros(dd.shape)
        dist = np.hypot(X - (p0[0] + t * dx), Y - (p0[1] + t * dy))
        real = deep & (R - dist >= TOL)
        if real.any():
            cands = sorted(round_of(p0, p1), key=lambda c: c[3])
            if not cands:
                print(f"\npiece {idx_in_op}: {tuple(round(v, 3) for v in p0)} -> {tuple(round(v, 3) for v in p1)}: NOT a move of the strip pass"); idx_in_op += 1; continue
            rk = cands[0][0]
            st = load_stock(os.path.join(d, f"stock_{rk}.bin"))
            P = pts[rk]
            print(f"\npiece {idx_in_op}: {tuple(round(v, 3) for v in p0)} -> {tuple(round(v, 3) for v in p1)}; round {rk} of the strip pass")
            # the ring points near the piece
            pxy = P[:, 3:5] + (DX, DY)
            m = (np.hypot(pxy[:, 0] - p0[0], pxy[:, 1] - p0[1]) < 1.01) | (np.hypot(pxy[:, 0] - p1[0], pxy[:, 1] - p1[1]) < 1.01)
            print("   ring points within 1 mm of the ends (ring, i, x, y, own Z, Z after the neighbour rule, standing, refused):")
            for r in P[m]:
                print(f"      ring {int(r[1])} i {int(r[2])} ({r[3] + DX:.3f}, {r[4] + DY:.3f}) own {r[5]:.3f} final {r[6]:.3f} standing {int(r[7])} refused {int(r[8])}")
            jj, ii = np.nonzero(real)
            for j, i in zip(jj, ii):
                nx, ny = (ia + i) * CELL, (ja + j) * CELL
                col = int(round((nx - DX - st["ou"]) / st["cs"])); row = int(round((ny - DY - st["ov"]) / st["cs"]))
                cx, cy = st["ou"] + col * st["cs"] + DX, st["ov"] + row * st["cs"] + DY
                ptop, pcons = float(st["top"][row, col]), float(st["cons"][row, col])
                rtop = float(before[j, i]); cutz = float(top[ja + j, ia + i])
                dpt = np.hypot(pxy[:, 0] - cx, pxy[:, 1] - cy)
                holds = (dpt <= R) & m
                lowest_ok = rtop - dpp
                cls = "A (planner top under the replay top by %.2f)" % (rtop - ptop) if ptop < rtop - 0.05 else ("B (no ring point near the piece holds the cell centre)" if not holds.any() else "C (the reads hold the cell; the chord is under them)")
                print(f"   node ({nx:.1f},{ny:.1f}) cell centre ({cx:.3f},{cy:.3f}): replay top {rtop:.2f}, cut to {cutz:.2f} (lowered {rtop - cutz:.2f}); planner ray top {ptop:.2f}, conservative top {pcons:.2f}; "
                      f"inside the piece edge {float(R - dist[j, i]):.3f}; ring points that hold the centre: {int(holds.sum())}"
                      + (f", their own Z {P[holds][:, 5].min():.2f}..{P[holds][:, 5].max():.2f} (replay top less Depth/Pass = {lowest_ok:.2f})" if holds.any() else "")
                      + f"; class {cls}")
                # earlier fed moves of the two roughs whose disc edge passed within one cell of the node
                g = []
                for n2 in range(n):
                    o2, t2, a, b = allmoves[n2]
                    if o2 is None or t2 not in TOOLS: continue
                    R2 = TOOLS[t2][1]
                    ex, ey = b[0] - a[0], b[1] - a[1]; l2 = ex * ex + ey * ey
                    tt = min(1, max(0, ((nx - a[0]) * ex + (ny - a[1]) * ey) / l2)) if l2 > 1e-18 else 0
                    dn = math.hypot(nx - (a[0] + tt * ex), ny - (a[1] + tt * ey))
                    zz = a[2] + tt * (b[2] - a[2])
                    if abs(dn - R2) <= CELL and zz < rtop - 0.05 + (rtop - ptop if dn > R2 else 0) and zz < STOCK_TOP:
                        g.append((o2[:22], n2, R2 - dn, zz))
                gz = {}
                for o2, n2, ins, zz in g:
                    key = (o2, round(zz, 2), ins > 0)
                    gz.setdefault(key, [0, ins, ins]); gz[key][0] += 1; gz[key][1] = min(gz[key][1], ins); gz[key][2] = max(gz[key][2], ins)
                for (o2, zz, inn), (cnt, lo, hi) in sorted(gz.items(), key=lambda kv: -kv[0][1])[:8]:
                    print(f"        earlier: '{o2}' {cnt} moves at Z {zz:.2f} with the node {'inside' if inn else 'OUTSIDE'} the disc edge by {lo:+.3f}..{hi:+.3f} mm")
    idx_in_op += 1
