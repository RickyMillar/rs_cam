# C15 geometry of the pocket fixture, in tool-centre space.
# Source of every value:
#   fixtures/demo_pocket.svg: rect 5..75 x 5..55 rx 8, island circle r 10 at (40, 30)
#     (the harness's View maps world = SVG coordinates; the seed (18.5, 18.5) of
#     pocket_geodesic_curv.txt lies at that place).
#   tool: the fixture's 6 mm end mill -> R = 3.
#   AdaptiveConfig::default(): stepover s = 2.0, tolerance 0.1.
#   planner cell = max(R / 6, tolerance) = 0.5 (path.rs planner_cell_size).
#   pass step = 3 cells (path.rs PASS_STEP_CELLS) = 1.5 mm.
#   ceiling c = radial_woc_fraction_from_leading_arc(1.05 * acos(1 - s/R) / 2pi)
#     (search.rs pass_engagement_ceiling, ops/adaptive_shared.rs).
import math
import numpy as np

R = 3.0
S = 2.0
TOL = 0.1
CELL = max(R / 6.0, TOL)
PASS_STEP = 3.0 * CELL
D = 2.0 * R
_alpha = math.acos(1.0 - min(S, 2 * R) / R) / (2 * math.pi)
_f = min(max(_alpha * 1.05, 0.0), 0.5)
C_FRAC = (1.0 - math.cos(2 * math.pi * _f)) / 2.0
CD = C_FRAC * D  # ceiling width, mm

# Pocket (material space) and the tool-centre region.
RECT = (5.0, 75.0, 5.0, 55.0)
RX = 8.0
ISL_C = np.array([40.0, 30.0])
ISL_R = 10.0
HOLE_R = ISL_R + R  # island grown by R
OM = (RECT[0] + R, RECT[1] - R, RECT[2] + R, RECT[3] - R)  # 8..72 x 8..52
OM_RX = RX - R
SEED = np.array([18.5, 18.5])  # pocket_geodesic_curv.txt "seed Some([18.5, 18.5])"
SEED_RADIUS = R  # "seed radius 3.00 mm": the starter's last lap


def rho_star():
    # C5: s'(rho) reaches s at rho* = s(2R - s) / (2(cD - s))
    return S * (2 * R - S) / (2 * (CD - S))


def spacing_bound(rho):
    # C5 s'(rho) = rho + R - sqrt(R^2 + rho^2 + 2 rho (R - cD)); rho > 0
    return rho + R - math.sqrt(R * R + rho * rho + 2 * rho * (R - CD))


def law_spacing(rho):
    """Allowed gap for a front of signed curvature radius rho (C5 law).
    rho > 0: the front bends round the cleared side (the C5 case).
    rho <= 0 or inf: a straight or concave front; the swept width is at
    most the gap, so the cap s applies."""
    if not np.isfinite(rho) or rho <= 0:
        return S
    return max(0.0, min(S, spacing_bound(rho)))


def front_width(g, rho):
    """C5/C7 circular-front width w = g + g (R - g/2) / rho (signed rho)."""
    if not np.isfinite(rho):
        return g
    return g + g * (R - g / 2.0) / rho


def harness_levels(tmax):
    """The C5 law levels from rho_0 = R (law_next_radius), as T = rho - R."""
    b = R - CD
    rho = R
    out = [0.0]
    while True:
        n = min(-b + math.sqrt(b * b + (rho + R) ** 2 - R * R), rho + S)
        if n - R > tmax + S:
            break
        rho = n
        out.append(rho - R)
    return out


def in_omega(x, y):
    x = np.asarray(x, float)
    y = np.asarray(y, float)
    ok = (x >= OM[0]) & (x <= OM[1]) & (y >= OM[2]) & (y <= OM[3])
    cx = np.clip(x, OM[0] + OM_RX, OM[1] - OM_RX)
    cy = np.clip(y, OM[2] + OM_RX, OM[3] - OM_RX)
    ok &= np.hypot(x - cx, y - cy) <= OM_RX + 1e-12
    ok &= np.hypot(x - ISL_C[0], y - ISL_C[1]) >= HOLE_R - 1e-12
    return ok


def in_stock(x, y):
    """Material-space pocket (stock before any cut)."""
    x = np.asarray(x, float)
    y = np.asarray(y, float)
    ok = (x >= RECT[0]) & (x <= RECT[1]) & (y >= RECT[2]) & (y <= RECT[3])
    cx = np.clip(x, RECT[0] + RX, RECT[1] - RX)
    cy = np.clip(y, RECT[2] + RX, RECT[3] - RX)
    ok &= np.hypot(x - cx, y - cy) <= RX
    ok &= np.hypot(x - ISL_C[0], y - ISL_C[1]) >= ISL_R
    return ok


def seg_hits_disc(px, py, qx, qy, r=HOLE_R, c=ISL_C):
    """True where the open segment p-q passes inside the disc (strictly)."""
    dx = qx - px
    dy = qy - py
    L2 = dx * dx + dy * dy
    t = np.where(L2 > 0, ((c[0] - px) * dx + (c[1] - py) * dy) / np.where(L2 > 0, L2, 1), 0.0)
    t = np.clip(t, 0, 1)
    ex = px + t * dx - c[0]
    ey = py + t * dy - c[1]
    return np.hypot(ex, ey) < r - 1e-9


NEAR_ANG = math.atan2(SEED[1] - ISL_C[1], SEED[0] - ISL_C[0])  # hole point facing the seed


def _wrap(a):
    return np.mod(a, 2 * math.pi)


def geo(px, py, X, Y, far_only=True):
    """Exact shortest path length from point(s) p to X,Y in the plane minus the
    open hole disc (the outer region is convex, so only the hole bends paths).
    far_only: a hugging arc may not contain the hole point facing the seed
    (paths stay on the far side; used for sources on the cut edge)."""
    px = np.asarray(px, float)
    py = np.asarray(py, float)
    X = np.asarray(X, float)
    Y = np.asarray(Y, float)
    c = ISL_C
    r = HOLE_R
    straight = np.hypot(X - px, Y - py)
    blocked = seg_hits_disc(px, py, X, Y)
    dp = np.hypot(px - c[0], py - c[1])
    dx = np.hypot(X - c[0], Y - c[1])
    ap = np.arctan2(py - c[1], px - c[0])
    ax = np.arctan2(Y - c[1], X - c[0])
    bp = np.arccos(np.clip(r / np.maximum(dp, r), -1, 1))
    bx = np.arccos(np.clip(r / np.maximum(dx, r), -1, 1))
    lp = np.sqrt(np.maximum(dp * dp - r * r, 0))
    lx = np.sqrt(np.maximum(dx * dx - r * r, 0))
    best = np.full(np.broadcast(px, X).shape, np.inf)
    # ccw: leave p at ap + bp, travel ccw to ax - bx
    for sgn in (+1, -1):
        t0 = ap + sgn * bp
        t1 = ax - sgn * bx
        arc = _wrap(sgn * (t1 - t0))
        L = lp + r * arc + lx
        if far_only:
            # arc from t0 going sgn-wise by `arc`; contains NEAR_ANG?
            off = _wrap(sgn * (NEAR_ANG - t0))
            L = np.where(off <= arc, np.inf, L)
        best = np.minimum(best, L)
    return np.where(blocked, best, straight)


def geo_side(px, py, X, Y, sgn):
    """Hugging path length on one side only (sgn +1 ccw, -1 cw) or straight."""
    c = ISL_C
    r = HOLE_R
    straight = np.hypot(X - px, Y - py)
    blocked = seg_hits_disc(px, py, X, Y)
    dp = np.hypot(px - c[0], py - c[1])
    dx = np.hypot(X - c[0], Y - c[1])
    ap = np.arctan2(py - c[1], px - c[0])
    ax = np.arctan2(Y - c[1], X - c[0])
    bp = np.arccos(np.clip(r / np.maximum(dp, r), -1, 1))
    bx = np.arccos(np.clip(r / np.maximum(dx, r), -1, 1))
    lp = np.sqrt(np.maximum(dp * dp - r * r, 0))
    lx = np.sqrt(np.maximum(dx * dx - r * r, 0))
    t0 = ap + sgn * bp
    t1 = ax - sgn * bx
    arc = _wrap(sgn * (t1 - t0))
    return np.where(blocked, lp + r * arc + lx, straight)


# Shadow lines: the two tangents from the seed to the grown island.
_d = float(np.hypot(*(ISL_C - SEED)))
_a0 = math.atan2(ISL_C[1] - SEED[1], ISL_C[0] - SEED[0])
_h = math.asin(HOLE_R / _d)
ANG_UP = _a0 + _h
ANG_LO = _a0 - _h
TAN_LEN = math.sqrt(_d * _d - HOLE_R * HOLE_R)
P_UP = SEED + TAN_LEN * np.array([math.cos(ANG_UP), math.sin(ANG_UP)])
P_LO = SEED + TAN_LEN * np.array([math.cos(ANG_LO), math.sin(ANG_LO)])
SEED_TO_ISL = _d
WEDGE = ANG_UP - ANG_LO


def ray_exit(p, ang):
    """Where the ray from p at ang leaves the tool-centre region (bisection)."""
    u = np.array([math.cos(ang), math.sin(ang)])
    lo, hi = 0.0, 200.0
    # step out until outside the outer region (ignore the hole: p is past it)
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        q = p + mid * u
        if _in_outer(q[0], q[1]):
            lo = mid
        else:
            hi = mid
    return p + lo * u


def _in_outer(x, y):
    if not (OM[0] <= x <= OM[1] and OM[2] <= y <= OM[3]):
        return False
    cx = min(max(x, OM[0] + OM_RX), OM[1] - OM_RX)
    cy = min(max(y, OM[2] + OM_RX), OM[3] - OM_RX)
    return math.hypot(x - cx, y - cy) <= OM_RX


A_UP = ray_exit(P_UP, ANG_UP)  # upper shadow line meets the wall
B_LO = ray_exit(P_LO, ANG_LO)


def hidden(X, Y):
    """Zone: points of the region the island hides from the seed."""
    return in_omega(X, Y) & seg_hits_disc(SEED[0], SEED[1], X, Y)


def seg_points(p, q, step):
    n = max(2, int(math.ceil(np.hypot(*(q - p)) / step)) + 1)
    t = np.linspace(0, 1, n)
    return p[None, :] + t[:, None] * (q - p)[None, :]
