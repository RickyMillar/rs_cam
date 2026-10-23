# C15 fields on a fine grid (spacing = tolerance, 0.1 mm), tool-centre space.
# Each field is exact up to the grid's bilinear contouring; no lattice
# pivots: the distances come from closed-form shortest paths round the disc.
import math
import numpy as np
import scipy.sparse as sp
import scipy.sparse.linalg as spla

from geom import (TOL, OM, SEED, R, S, CD, HOLE_R, ISL_C, P_UP, P_LO, A_UP, B_LO,
                  WEDGE, in_omega, hidden, geo, seg_points)

H = TOL
XS = np.arange(OM[0], OM[1] + H / 2, H)
YS = np.arange(OM[2], OM[3] + H / 2, H)
GX, GY = np.meshgrid(XS, YS)  # [row=y, col=x]
OMEGA = in_omega(GX, GY)
ZONE = hidden(GX, GY)
VISIBLE = OMEGA & ~ZONE


def seed_T():
    """(a) exact geodesic distance from the seed, as T = D - R."""
    T = np.full(GX.shape, np.nan)
    T[OMEGA] = geo(SEED[0], SEED[1], GX[OMEGA], GY[OMEGA], far_only=False) - R
    return T


def edge_distance(p, q, X, Y, step=TOL):
    """Exact distance inside the zone from the cut-edge segment p-q (sampled
    at the grid spacing) to X,Y: straight, or round the far side
    of the hole."""
    src = seg_points(p, q, step)
    best = np.full(X.shape, np.inf)
    for k0 in range(0, len(src), 64):
        blk = src[k0:k0 + 64]
        d = geo(blk[:, 0][:, None], blk[:, 1][:, None], X[None, :], Y[None, :], far_only=True)
        best = np.minimum(best, d.min(axis=0))
    return best


_cache = {}


def zone_edges():
    if "edges" not in _cache:
        X, Y = GX[ZONE], GY[ZONE]
        du = np.full(GX.shape, np.nan)
        dl = np.full(GX.shape, np.nan)
        du[ZONE] = edge_distance(P_UP, A_UP, X, Y)
        dl[ZONE] = edge_distance(P_LO, B_LO, X, Y)
        _cache["edges"] = (du, dl)
    return _cache["edges"]


def split_exact():
    """C11 made exact: T = min(d_up, d_lo), distance from the cut edge."""
    du, dl = zone_edges()
    return np.fmin(du, dl)


def round_union(r_blend):
    """(c) the two branches joined by a round union of blend radius r_blend:
    tau = min where |d_up - d_lo| >= r, else (sum + sqrt(2r^2 - diff^2))/2 - r
    (the round union of the strips {d < t}, solved for t)."""
    du, dl = zone_edges()
    m = np.fmin(du, dl)
    diff = du - dl
    sm = du + dl
    inside = np.abs(diff) < r_blend
    tau = m.copy()
    with np.errstate(invalid="ignore"):
        bl = (sm + np.sqrt(np.maximum(2 * r_blend ** 2 - diff ** 2, 0))) / 2 - r_blend
    tau[inside] = bl[inside]
    return tau


def one_side(which):
    du, dl = zone_edges()
    return du if which == "up" else dl


def harmonic():
    """(b) Laplace u = 0 in the zone; u = 0 on the cut edge (the two shadow
    segments, placed exactly by Shortley-Weller fractions), u = 1 on the outer
    wall, du/dn = 0 on the island arc. Five-point stencil on the grid."""
    ny, nx = GX.shape
    idx = -np.ones(GX.shape, int)
    zr, zc = np.nonzero(ZONE)
    idx[zr, zc] = np.arange(len(zr))
    n = len(zr)
    rows, cols, vals = [], [], []
    rhs = np.zeros(n)
    diag = np.zeros(n)
    nbrs = ((0, 1), (0, -1), (1, 0), (-1, 0))

    def classify_cross(x0, y0, x1, y1):
        """Fraction along (x0,y0)->(x1,y1) to the zone boundary and its kind."""
        lo, hi = 0.0, 1.0
        for _ in range(40):
            m = 0.5 * (lo + hi)
            if hidden(np.array(x0 + m * (x1 - x0)), np.array(y0 + m * (y1 - y0))):
                lo = m
            else:
                hi = m
        xb, yb = x0 + hi * (x1 - x0), y0 + hi * (y1 - y0)
        if math.hypot(xb - ISL_C[0], yb - ISL_C[1]) <= HOLE_R + 1e-6:
            kind = "island"
        elif in_omega(np.array(xb + 1e-7 * (x1 - x0)), np.array(yb + 1e-7 * (y1 - y0))):
            kind = "cut"  # leaves the zone into the visible region: the shadow line
        else:
            kind = "wall"
        return max(hi, 1e-3), kind

    # Shortley-Weller: for each axis, a node with arms hp, hm (fractions of H)
    for k in range(n):
        r_, c_ = zr[k], zc[k]
        x0, y0 = GX[r_, c_], GY[r_, c_]
        for axis in (0, 1):
            arms = []
            for sgn in (1, -1):
                dr, dc = ((0, sgn) if axis == 0 else (sgn, 0))
                rr, cc = r_ + dr, c_ + dc
                inb = 0 <= rr < ny and 0 <= cc < nx
                if inb and ZONE[rr, cc]:
                    arms.append((1.0, "node", idx[rr, cc]))
                else:
                    x1 = x0 + dc * H
                    y1 = y0 + dr * H
                    f, kind = classify_cross(x0, y0, x1, y1)
                    arms.append((f, kind, None))
            (hp, kp, jp), (hm, km, jm) = arms
            # Neumann arm: mirror (ghost = centre value) -> drop the arm and
            # use the opposite arm's one-sided second difference.
            if kp == "island" and km == "island":
                continue
            if kp == "island":
                hp, kp, jp = hm, km, jm
            if km == "island":
                hm, km, jm = hp, kp, jp
            cp = 2.0 / (hp * (hp + hm))
            cm = 2.0 / (hm * (hp + hm))
            diag[k] -= cp + cm
            for cf, kind, j in ((cp, kp, jp), (cm, km, jm)):
                if kind == "node":
                    rows.append(k)
                    cols.append(j)
                    vals.append(cf)
                elif kind == "wall":
                    rhs[k] -= cf * 1.0
                # cut: u = 0 adds nothing
    rows += list(range(n))
    cols += list(range(n))
    vals += list(diag)
    A = sp.csr_matrix((vals, (rows, cols)), shape=(n, n))
    u = spla.spsolve(A, rhs)
    U = np.full(GX.shape, np.nan)
    U[zr, zc] = u
    return U


def wedge_half():
    return WEDGE / 2.0
