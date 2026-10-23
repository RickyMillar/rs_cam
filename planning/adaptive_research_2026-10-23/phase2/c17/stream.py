# C17: the streamline march. A field gives only the DIRECTION of motion;
# loop k+1 = every point of loop k moved along its own streamline.
#
# Every number comes from C15 (phase2/c15, imported, not edited):
#   R = 3, s = 2, tolerance 0.1, pass step 1.5 mm, c*D = 2.175868 mm (geom.py);
#   the C5 law gap min(s, s'(rho)) of the NEW loop's own signed radius
#   (march.law_spacing_v), the radius over +- one pass step of arc length;
#   the harmonic field (e) and the exact entry distance (a) (whole.py).
# Resampling: a segment longer than the tolerance gets points inserted at the
# tolerance; a moving point nearer than half the tolerance to the point kept
# before it is dropped. Streamline sub-step: at most the grid cell (0.1 mm).
# A point stops in two cases only:
#   wall: its streamline leaves the region (bisected onto the wall);
#   air:  the tool disc at its new place holds no standing stock (the C15
#         raster at the tolerance): the move cuts nothing.
import math
import os
import sys

import numpy as np
from scipy import ndimage
from scipy.spatial import cKDTree

HERE = os.path.dirname(os.path.abspath(__file__))
C15 = os.path.join(HERE, "..", "c15")
sys.path.insert(0, C15)

from geom import R, S, D, CD, C_FRAC, TOL, PASS_STEP  # noqa: E402
from shapes import Shape, H, STAR  # noqa: E402
from march import law_spacing_v, Replay, SCALLOP  # noqa: E402
import whole  # noqa: E402


# ── direction fields ───────────────────────────────────────────────────

class Direction:
    """Unit direction of grad F, bilinear on the shape's grid."""

    def __init__(self, sh, F):
        self.sh = sh
        gy, gx = np.gradient(F, H)
        self.gx, self.gy = gx, gy
        self.x0, self.y0 = sh.xs[0], sh.ys[0]

    def __call__(self, P):
        c = np.stack([(P[:, 1] - self.y0) / H, (P[:, 0] - self.x0) / H])
        vx = ndimage.map_coordinates(self.gx, c, order=1, mode="nearest")
        vy = ndimage.map_coordinates(self.gy, c, order=1, mode="nearest")
        n = np.hypot(vx, vy)
        n = np.where(n > 0, n, 1.0)
        return np.stack([vx / n, vy / n], 1)


def harmonic_field(sh):
    return whole.harmonic(sh)


def geodesic_field(sh):
    """The exact entry distance on the whole grid (the control). Outside the
    region the closed form still holds, so the gradient at the wall is the
    path direction."""
    return sh.entry_dist(sh.GX, sh.GY)


# ── closed-loop geometry ───────────────────────────────────────────────

def seglen(P):
    return np.hypot(*(np.roll(P, -1, 0) - P).T)


def loop_length(P):
    return float(seglen(P).sum())


def orient(P):
    """+1 for a counter-clockwise loop."""
    x, y = P[:, 0], P[:, 1]
    return 1.0 if (x * np.roll(y, -1) - np.roll(x, -1) * y).sum() > 0 else -1.0


def radius_closed(P, half=PASS_STEP):
    """Three-point circle over +- half of arc length, periodic. Signed + when
    the centre lies inside the loop (the cleared side: every loop holds the
    entry and the cut; the island stays outside every loop). inf: straight."""
    L = seglen(P)
    s = np.concatenate([[0], np.cumsum(L)])
    T = s[-1]
    Pc = np.vstack([P, P[:1]])

    def at(q):
        q = np.mod(q, T)
        return np.stack([np.interp(q, s, Pc[:, 0]), np.interp(q, s, Pc[:, 1])], 1)

    h = min(half, T / 4)
    a, b, c = at(s[:-1] - h), P, at(s[:-1] + h)
    ab, ac = b - a, c - a
    cr = ab[:, 0] * ac[:, 1] - ab[:, 1] * ac[:, 0]
    la, lb, lc = np.hypot(*(b - c).T), np.hypot(*ac.T), np.hypot(*ab.T)
    with np.errstate(divide="ignore", invalid="ignore"):
        rad = la * lb * lc / (2 * np.abs(cr))
    rho = np.full(len(P), np.inf)
    ok = np.isfinite(rad) & (np.abs(cr) > 1e-12)
    rho[ok] = (np.sign(cr) * orient(P) * rad)[ok]
    return rho


def resample_closed(P, step):
    Pc = np.vstack([P, P[:1]])
    s = np.concatenate([[0], np.cumsum(seglen(P))])
    n = max(8, int(math.ceil(s[-1] / step)))
    u = np.linspace(0, s[-1], n, endpoint=False)
    return np.stack([np.interp(u, s, Pc[:, 0]), np.interp(u, s, Pc[:, 1])], 1)


def refine(sh, P, kind):
    """Insert points at the tolerance on long segments (new points move) and
    drop a point nearer than TOL/2 to the point kept before it. kind: 0
    moving, 1 stopped on a wall, 2 stopped in air. A segment between two
    stopped points gets new points only when both stopped on a wall and its
    midpoint lies more than TOL/2 from the wall (a chord across a corner,
    with stock beyond it)."""
    Q, F = [], []
    n = len(P)
    nxt = np.roll(P, -1, 0)
    kn = np.roll(kind, -1)
    mid = 0.5 * (P + nxt)
    both = (kind > 0) & (kn > 0)
    chord = (kind == 1) & (kn == 1) & (sh.wall_dist(mid[:, 0], mid[:, 1]) > TOL / 2)
    no_ins = both & ~chord
    for i in range(n):
        p, q = P[i], nxt[i]
        Q.append(p)
        F.append(kind[i])
        if no_ins[i]:
            continue
        L = math.hypot(*(q - p))
        k = int(math.ceil(L / TOL))
        for j in range(1, k):
            Q.append(p + (q - p) * j / k)
            F.append(0)
    Q, F = np.array(Q), np.array(F)
    keep = np.ones(len(Q), bool)
    last = Q[0]
    for i in range(1, len(Q)):
        if math.hypot(*(Q[i] - last)) < TOL / 2 and (F[i] == 0 or (F[i - 1] > 0 and F[(i + 1) % len(Q)] > 0)):
            keep[i] = False
        else:
            last = Q[i]
    return Q[keep], F[keep]


# ── streamline sampling ───────────────────────────────────────────────

def trace(sh, dirf, P, need, mode, tree, cap_steps):
    """Sample the streamline of every point in sub-steps of at most H.
    mode 'step': sample to arc length `need` (the largest step, s).
    mode 'normal': sample until the distance to the cut set reaches `need`.
    Return per point arrays (arc length, distance to cut, x, y) of shape
    (n, m)."""
    n = len(P)
    m = cap_steps + 1
    X = np.full((n, m), np.nan)
    Y = np.full((n, m), np.nan)
    A = np.full((n, m), np.nan)
    G = np.full((n, m), np.nan)
    X[:, 0], Y[:, 0], A[:, 0] = P[:, 0], P[:, 1], 0.0
    G[:, 0] = tree.query(P)[0]
    cur = P.copy()
    act = np.ones(n, bool)
    for k in range(1, m):
        idx = np.nonzero(act)[0]
        if len(idx) == 0:
            break
        p = cur[idx]
        if mode == "step":
            h = np.minimum(H, need - A[idx, k - 1])
        else:
            h = np.full(len(idx), H)
        d1 = dirf(p)
        d2 = dirf(p + 0.5 * h[:, None] * d1)
        q = p + h[:, None] * d2
        # a step that leaves the region: bisect to the wall and stop there
        out = ~sh.in_omega(q[:, 0], q[:, 1])
        if out.any():
            lo = np.zeros(out.sum())
            hi = np.ones(out.sum())
            pp, qq = p[out], q[out]
            for _ in range(30):
                t = 0.5 * (lo + hi)
                r = pp + t[:, None] * (qq - pp)
                ins = sh.in_omega(r[:, 0], r[:, 1])
                lo = np.where(ins, t, lo)
                hi = np.where(ins, hi, t)
            q[out] = pp + lo[:, None] * (qq - pp)
            h[out] = h[out] * lo
        cur[idx] = q
        X[idx, k], Y[idx, k] = q[:, 0], q[:, 1]
        A[idx, k] = A[idx, k - 1] + h
        G[idx, k] = tree.query(q)[0]
        done = out.copy()
        if mode == "step":
            done |= A[idx, k] >= need - 1e-12
        else:
            done |= G[idx, k] >= need
        act[idx[done]] = False
    return A, G, X, Y


def pick(A, G, X, Y, target, mode):
    """Position on each sampled streamline where arc length (mode 'step') or
    distance to the cut (mode 'normal') first reaches target (linear
    interpolation); the last sample if it never does (a wall hit)."""
    key = A if mode == "step" else G
    n, m = A.shape
    kk = np.where(np.isnan(key), -np.inf, key)
    reach = kk >= target[:, None] - 1e-12
    any_ = reach.any(1)
    first = np.where(any_, reach.argmax(1), 0)
    last = (~np.isnan(A)).sum(1) - 1
    r = np.arange(n)
    j1 = np.where(any_, np.maximum(first, 1), last)
    j0 = np.where(any_, j1 - 1, last)
    k0, k1 = kk[r, j0], kk[r, j1]
    with np.errstate(divide="ignore", invalid="ignore"):
        t = np.where(any_ & (k1 > k0), (target - k0) / (k1 - k0), 1.0)
    t = np.where(any_ & (first == 0), 0.0, np.clip(t, 0, 1))
    j0 = np.where(any_ & (first == 0), 0, j0)
    j1 = np.where(any_ & (first == 0), 0, j1)
    out = np.stack([X[r, j0] + t * (X[r, j1] - X[r, j0]), Y[r, j0] + t * (Y[r, j1] - Y[r, j0])], 1)
    step = A[r, j0] + t * (A[r, j1] - A[r, j0])
    return out, step, ~any_


def has_stock(rp, p):
    """True when the tool disc at p holds standing stock (the C15 replay
    raster, at the tolerance)."""
    ii, jj, dx, dy, inside = rp._window(p[0], p[1])
    return bool((rp.stock[np.ix_(ii, jj)] & inside).any())


def starter(sh):
    n = int(math.ceil(2 * math.pi * R / TOL))
    ang = np.linspace(0, 2 * math.pi, n, endpoint=False)
    return sh.seed + R * np.stack([np.cos(ang), np.sin(ang)], 1)


def solve_targets(P, mv, A, G, X, Y, mode):
    """The implicit rule: g_i = law gap of the NEW loop's own radius at i,
    g <= s. Damped fixed point from g = s; the damping halves whenever the
    largest residual grows, so the iteration cannot run away. Return g, the
    last residual (mm)."""
    g, om, res0 = np.full(mv.sum(), S), 1.0, np.inf
    Q = P.copy()
    for _ in range(400):
        q, step, st = pick(A, G, X, Y, g, mode)
        Q[mv] = q
        allow = law_spacing_v(radius_closed(Q))[mv]
        res = allow - g
        res = np.where(st & (res > 0), 0.0, res)  # a wall stop has no further room
        rn = float(np.abs(res).max())
        if rn < 1e-4:
            break
        if rn > res0:
            om *= 0.5
        res0 = rn
        g = np.clip(g + om * res, 0.0, S)
    return np.minimum(g, allow), rn


def march(sh, dirf, mode, air_move=False, max_loops=600, log=None):
    """The streamline march. Return the loops in cut order; each holds the
    points P, the stop flags after and before the loop, the kind (0 moving,
    1 wall, 2 air), the per-point step along the streamline, the target g,
    and the rule check: the largest g - law gap of the final loop (mm)."""
    P = starter(sh)
    kind = np.zeros(len(P), int)
    cut = [P.copy()]
    rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    loops = []
    cap = int(math.ceil(max(sh.mat_box[1] - sh.mat_box[0], sh.mat_box[3] - sh.mat_box[2]) / H))
    for it in range(max_loops):
        stop = kind > 0
        if stop.all():
            break
        tree = cKDTree(np.concatenate(cut))
        mv = ~stop
        A, G, X, Y = trace(sh, dirf, P[mv], S, mode, tree, cap if mode == "normal" else int(math.ceil(S / H)) + 1)
        g, rn = solve_targets(P, mv, A, G, X, Y, mode)
        q, step, wall = pick(A, G, X, Y, g, mode)
        # a move that cuts no stock is an air move: the point stops (it
        # stays where it is unless air_move)
        air = np.array([not has_stock(rp, p) for p in q]) & ~wall
        if not air_move:
            q[air] = P[mv][air]
            step = np.where(air, 0.0, step)
        Q = P.copy()
        Q[mv] = q
        newkind = kind.copy()
        newkind[np.nonzero(mv)[0]] = np.where(wall, 1, np.where(air, 2, 0))
        fstep = np.zeros(len(P))
        fstep[mv] = step
        fg = np.full(len(P), np.nan)
        fg[mv] = g
        free = mv.copy()
        free[mv] = ~(wall | air)
        viol = float((fg[free] - law_spacing_v(radius_closed(Q))[free]).max()) if free.any() else 0.0
        loops.append(dict(P=Q, stop=newkind > 0, was_stopped=stop.copy(), step=fstep, target=fg,
                          kind=newkind, residual=rn, violation=viol))
        cut.append(Q)
        if log:
            log(f"loop {it + 1}: {len(Q)} pts, moving {mv.sum()}, wall {wall.sum()}, air {air.sum()}, "
                f"step {step.min():.3f}..{step.max():.3f}, len {loop_length(Q):.1f}, res {rn:.1e}, viol {viol:.1e}")
        P, kind = refine(sh, Q, newkind)
        rp.cut(np.vstack([P, P[:1]]))
    return loops


# ── metrics ────────────────────────────────────────────────────────────

def wall_mask(loop):
    """Segment i (point i to i+1) lies on the wall when both ends were
    stopped BEFORE this loop (zero-load wall following)."""
    w = loop["was_stopped"]
    return w & np.roll(w, -1)


def new_wall_mask(loop):
    s = loop["stop"]
    return s & np.roll(s, -1) & ~wall_mask(loop)


def turn_angles(P, mask_change):
    """Turning angle (degrees) at the points listed, from the chords to the
    neighbours one tolerance away along the loop."""
    out = []
    n = len(P)
    for i in np.nonzero(mask_change)[0]:
        a, b, c = P[(i - 1) % n], P[i], P[(i + 1) % n]
        u, v = b - a, c - b
        nu, nv = np.hypot(*u), np.hypot(*v)
        if nu == 0 or nv == 0:
            continue
        cosang = np.clip((u @ v) / (nu * nv), -1, 1)
        out.append(math.degrees(math.acos(cosang)))
    return np.array(out)


def doubled_length(P):
    """Length of the loop where another part of the same loop, more than D
    of arc length away, lies within the tolerance (a doubled strand)."""
    L = seglen(P)
    s = np.concatenate([[0], np.cumsum(L)])[:-1]
    T = L.sum()
    tree = cKDTree(P)
    pairs = tree.query_pairs(TOL, output_type="ndarray")
    if len(pairs) == 0:
        return 0.0
    ds = np.abs(s[pairs[:, 0]] - s[pairs[:, 1]])
    ds = np.minimum(ds, T - ds)
    far = pairs[ds > D]
    flag = np.zeros(len(P), bool)
    flag[far.ravel()] = True
    return float(L[flag].sum())


def self_crossings(P, where=False):
    """Count proper crossings between non-adjacent segments (grid hashed);
    with where, also return the crossing segment midpoints."""
    n = len(P)
    A, B = P, np.roll(P, -1, 0)
    cell = TOL * 4
    buckets = {}
    lo = np.minimum(A, B)
    hi = np.maximum(A, B)
    for i in range(n):
        for gx in range(int(lo[i, 0] // cell), int(hi[i, 0] // cell) + 1):
            for gy in range(int(lo[i, 1] // cell), int(hi[i, 1] // cell) + 1):
                buckets.setdefault((gx, gy), []).append(i)
    seen = set()
    cnt = 0
    pts = []

    def cross(p, q, r, t):
        d1 = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
        d2 = (q[0] - p[0]) * (t[1] - p[1]) - (q[1] - p[1]) * (t[0] - p[0])
        d3 = (t[0] - r[0]) * (p[1] - r[1]) - (t[1] - r[1]) * (p[0] - r[0])
        d4 = (t[0] - r[0]) * (q[1] - r[1]) - (t[1] - r[1]) * (q[0] - r[0])
        return d1 * d2 < 0 and d3 * d4 < 0

    for lst in buckets.values():
        for a_ in range(len(lst)):
            for b_ in range(a_ + 1, len(lst)):
                i, j = lst[a_], lst[b_]
                if abs(i - j) <= 1 or abs(i - j) == n - 1:
                    continue
                key = (min(i, j), max(i, j))
                if key in seen:
                    continue
                seen.add(key)
                if cross(A[i], B[i], A[j], B[j]):
                    cnt += 1
                    pts.append(0.5 * (A[i] + B[i]))
    return (cnt, pts) if where else cnt


def replay_loops(sh, loops, pts_key="P"):
    """C15 replay: cut each loop in order (resampled at the tolerance for the
    reading). Per sample: swept width, area; a class per sample: 'wall' on
    zero-load wall following, 'junction' within R of arc length of a change
    between wall and moving part, 'seam' within R of the loop start, else
    'interior'."""
    rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    out = dict(w=[], cls=[], apm=[], xy=[], li=[], area=0.0, length=0.0)
    for li_, lp in enumerate(loops):
        P, wm = lp[pts_key], wall_mask(lp)
        # resample at the tolerance with the wall flag carried by segment
        Pc = np.vstack([P, P[:1]])
        L = seglen(P)
        s = np.concatenate([[0], np.cumsum(L)])
        T = s[-1]
        n = max(8, int(math.ceil(T / TOL)))
        u = np.linspace(0, T, n + 1)
        Q = np.stack([np.interp(u, s, Pc[:, 0]), np.interp(u, s, Pc[:, 1])], 1)
        seg = np.clip(np.searchsorted(s, u, side="right") - 1, 0, len(P) - 1)
        wall = wm[seg]
        w, a = rp.cut(Q)
        out["area"] += float(a.sum())
        out["length"] += T
        # distance (arc) to the nearest change of the wall flag, periodic
        ch = np.nonzero(wall != np.roll(wall, 1))[0]
        if len(ch):
            du = np.abs(u[:, None] - u[ch][None, :])
            du = np.minimum(du, T - du).min(1)
        else:
            du = np.full(len(u), np.inf)
        cls = np.where(wall, "wall", np.where(du <= R, "junction",
                       np.where((u <= R) | (u >= T - R), "seam", "interior")))
        out["w"].append(w / D)
        out["cls"].append(cls)
        out["xy"].append(Q)
        out["li"].append(np.full(len(Q), li_))
        # area per mm over one pass step, interior samples only
        k = max(1, int(round(PASS_STEP / (T / n))))
        cs = np.cumsum(a)
        apm = (cs[k:] - cs[:-k]) / (u[k:] - u[:-k])
        mid = np.arange(k, len(u)) - k // 2
        ok = (cls[mid] == "interior") & (cls[np.arange(k, len(u))] == "interior") & (cls[np.arange(0, len(u) - k)] == "interior")
        out["apm"].append(apm[ok] / D)
    out["w"] = np.concatenate(out["w"])
    out["cls"] = np.concatenate(out["cls"])
    out["apm"] = np.concatenate(out["apm"])
    out["xy"] = np.concatenate(out["xy"])
    out["li"] = np.concatenate(out["li"])
    res = rp.stock & sh.stock0
    return out, res


def residue(sh, res):
    """Standing stock farther than the wall scallop plus one cell from the
    boundary: area, the part no centre can reach (farther than R plus half a
    cell from every region node), the largest piece (area, centre, distance
    to the boundary), the largest distance to the boundary of any residue."""
    out = dict(mm2=0.0, unreachable_mm2=0.0, wall_dist_mm=0.0, pieces=0, big_mm2=0.0,
               big_at=[0.0, 0.0], big_wall_mm=0.0)
    if not res.any():
        return out
    bd_all = np.full(res.shape, -1.0)
    bd_all[res] = sh.boundary_dist(sh.MGX[res], sh.MGY[res])
    out["wall_dist_mm"] = float(bd_all[res].max())
    inter = res & (bd_all > SCALLOP + H)
    lab, nlab = ndimage.label(inter)
    if nlab == 0:
        return out
    sizes = ndimage.sum(inter, lab, range(1, nlab + 1))
    big = int(np.argmax(sizes)) + 1
    m = lab == big
    om = cKDTree(np.stack([sh.GX[sh.omega], sh.GY[sh.omega]], 1))
    dr = om.query(np.stack([sh.MGX[inter], sh.MGY[inter]], 1))[0]
    out.update(mm2=float(inter.sum() * H * H), unreachable_mm2=float((dr > R + H / 2).sum() * H * H),
               pieces=int(nlab), big_mm2=float(sizes[big - 1] * H * H),
               big_at=[float(sh.MGX[m].mean()), float(sh.MGY[m].mean())], big_wall_mm=float(bd_all[m].max()))
    return out
