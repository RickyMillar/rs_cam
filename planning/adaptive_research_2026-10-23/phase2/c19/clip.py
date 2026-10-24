# C19: the streamline march, clipped to its cut pieces and split per lobe.
#
# Every number comes from C15 (phase2/c15) and C17 (phase2/c17), imported and
# not edited:
#   R = 3, s = 2, tolerance 0.1, pass step 1.5 mm, c*D = 2.175868 mm (geom.py);
#   the march loops (C17 curves_c17.npz, run "h_step": the lead's rule);
#   C15 (a) curves (c15/curves_whole.npz, key "a");
#   the replay raster (march.Replay) and the residue rule (stream.residue);
#   the engaged / air split (plan.py: area per mm over one pass step,
#   centred, more than one cell width H = 0.1 mm);
#   FreeCAD's link rule with the string-pulled route (plan.Planner.route).
# C14 (P2_C2_geodesic_fixes.md §C14): one-way = every piece in the climb
# direction (stock on the right of travel), a keep-down return along the
# piece just cut, then a turn to the next piece's start; the turn bound
# s + (1 + sqrt 2) * cell with the planner cell 0.5 mm (3.207 mm); the turn
# feed factor c / max(w, a). C16 §Task 4: theta and s / cos(theta) +
# (1 + sqrt 2) * cell.
import math
import os
import sys

import numpy as np
from scipy import ndimage

HERE = os.path.dirname(os.path.abspath(__file__))
C17 = os.path.join(HERE, "..", "c17")
C15 = os.path.join(HERE, "..", "c15")
sys.path.insert(0, C17)
sys.path.insert(0, C15)

import stream as st  # noqa: E402  (C17)
import plan as pn  # noqa: E402  (C17)
from geom import R, S, D, CD, C_FRAC, TOL, PASS_STEP, CELL  # noqa: E402  (C15)
from march import resample  # noqa: E402  (C15)

H = st.H
TURN_BOUND = S + (1 + math.sqrt(2)) * CELL  # C14 turn_bound(s, cell)
QUANTUM = TOL / D  # one replay cell as a fraction of D (C17's "plus one cell")
FOUR = ndimage.generate_binary_structure(2, 1)  # cells that share an edge


# ── inputs ─────────────────────────────────────────────────────────────

def ccw(P):
    return P if st.orient(P) > 0 else P[::-1].copy()


def climb(sh, P):
    """C14 climb_oriented: read T one cell right and one cell left of each
    segment's middle; turn the arc so that T grows to the right."""
    m = 0.5 * (P[1:] + P[:-1])
    t = np.diff(P, axis=0)
    n = np.hypot(*t.T)
    ok = n > 0
    t = t[ok] / n[ok, None]
    m = m[ok]
    rgt = np.stack([t[:, 1], -t[:, 0]], 1)
    tr = sh.entry_dist(*(m + CELL * rgt).T)
    tl = sh.entry_dist(*(m - CELL * rgt).T)
    return P if (tr > tl).sum() >= (tl > tr).sum() else P[::-1].copy()


def march_levels(nm):
    """C17 march loops (closed, first point not repeated), one per level,
    turned counter-clockwise: the cleared side is inside, so the stock is on
    the right of the travel (climb, C14 §(3))."""
    d = np.load(os.path.join(C17, "curves_c17.npz"))
    n = len({k.split("|")[2] for k in d.files if k.startswith(f"{nm}|h_step|")})
    return [[dict(P=ccw(d[f"{nm}|h_step|{i}|P"].astype(float)), closed=True)] for i in range(n)]


def a_levels(nm, sh, key="a"):
    """C15 curves per level: key "a" (exact-distance wraps: open arcs and
    closed loops) or "e" (harmonic level lines: closed loops). An open arc
    is turned so that the entry distance T grows to its right (C14
    `climb_oriented`: the stock side is where T grows). A closed loop is a
    level line of a field that is 0 at the entry and grows outward, so it is
    turned counter-clockwise when it holds the entry (stock outside, on the
    right), else clockwise (it holds stock)."""
    d = np.load(os.path.join(C15, "curves_whole.npz"))
    cur = {}
    for k_ in d.files:
        n_, k, i, j = k_.split("|")
        if n_ == nm and k == key:
            cur.setdefault(int(i), []).append((int(j), d[k_].astype(float)))
    out = []
    for i in sorted(cur):
        lv = []
        for _, P in sorted(cur[i], key=lambda t: t[0]):
            closed = pn.is_closed(P)
            Q = P[:-1] if closed else P
            if not closed:
                Q = climb(sh, Q)
            else:
                Q = ccw(Q)
                if not st.poly_inside(Q, np.array([sh.seed[0]]), np.array([sh.seed[1]]))[0]:
                    Q = Q[::-1].copy()
            lv.append(dict(P=Q, closed=closed))
        out.append(lv)
    return out


# ── the replay with the label of the stock each sample removes ─────────

def cut_labelled(rp, Q, lab):
    """march.Replay.cut, and per sample the stock labels it removes:
    a list of (labels, cell counts)."""
    n = len(Q)
    tang = np.gradient(Q, axis=0)
    tl = np.hypot(*tang.T)
    tang = tang / np.where(tl > 0, tl, 1)[:, None]
    w = np.zeros(n)
    a = np.zeros(n)
    labs = []
    for k in range(n):
        x, y = Q[k]
        ii, jj, dx, dy, inside = rp._window(x, y)
        sub = rp.stock[np.ix_(ii, jj)]
        mat = sub & inside
        if mat.any():
            nx, ny = -tang[k, 1], tang[k, 0]
            nn = (dx * nx + dy * ny)[mat]
            w[k] = nn.max() - nn.min()
            a[k] = mat.sum() * H * H
            u, c = np.unique(lab[np.ix_(ii, jj)][mat], return_counts=True)
            labs.append((u, c))
        else:
            labs.append((np.zeros(0, int), np.zeros(0, int)))
        sub[inside] = False
        rp.stock[np.ix_(ii, jj)] = sub
    return w, a, labs


def engaged(s, a, closed):
    """plan.py's split: area per mm over one pass step centred on the sample
    is more than one cell width H. Periodic on a closed curve."""
    n = len(s)
    if closed:
        T = s[-1]
        s3 = np.concatenate([s[:-1] - T, s[:-1], s[:-1] + T, [s[-1] + T]])
        a3 = np.concatenate([a[:-1], a[:-1], a[:-1], [a[-1]]])
        cs = np.concatenate([[0], np.cumsum(a3)])
        q = s3[n - 1:2 * (n - 1) + 1]
        lo = np.searchsorted(s3, q - PASS_STEP / 2, side="left")
        hi = np.searchsorted(s3, q + PASS_STEP / 2, side="right")
        span = np.maximum(s3[hi - 1] - s3[lo], TOL)
        return (cs[hi] - cs[lo]) / span > H
    cs = np.concatenate([[0], np.cumsum(a)])
    lo = np.searchsorted(s, s - PASS_STEP / 2, side="left")
    hi = np.searchsorted(s, s + PASS_STEP / 2, side="right")
    span = np.maximum(s[np.minimum(hi, n) - 1] - s[lo], TOL)
    return (cs[hi] - cs[lo]) / span > H


def runs(e, closed):
    """[first, last] index pairs of the engaged runs. On a closed curve
    (sample 0 = sample n-1) a run may wrap; it is returned with last < first.
    None for a closed curve engaged all round (a ring)."""
    n = len(e)
    if closed:
        m = e[:-1]
        if m.all():
            return None
        if not m.any():
            return []
        k0 = int(np.argmin(m))  # an air sample: start the scan there
        rr = []
        i = 0
        N = len(m)
        while i < N:
            j = (k0 + i) % N
            if m[j]:
                st_ = j
                while i + 1 < N and m[(k0 + i + 1) % N]:
                    i += 1
                rr.append((st_, (k0 + i) % N))
            i += 1
        return rr
    k = np.nonzero(np.diff(np.concatenate([[0], e.astype(int), [0]])))[0]
    return [(int(a), int(b) - 1) for a, b in zip(k[0::2], k[1::2])]


def sub_polyline(V, sv, u0, u1, closed):
    """The part of the curve V (vertex arc lengths sv; closed: V[-1] = V[0])
    from arc length u0 to u1, with the curve's own vertices between them, so
    every point of the part lies on the curve. On a closed curve u1 < u0
    wraps through the seam."""
    T = sv[-1]

    def at(u):
        return np.array([np.interp(u, sv, V[:, 0]), np.interp(u, sv, V[:, 1])])

    def inner(a, b):
        k = np.nonzero((sv > a) & (sv < b))[0]
        return V[k]

    if u1 >= u0:
        return np.vstack([at(u0), inner(u0, u1), at(u1)])
    return np.vstack([at(u0), inner(u0, T), V[-1:], inner(0, u1), at(u1)])


def classify(sh, levels):
    """The march order (C17 §7: levels in order, in a level the nearest curve
    next) replayed whole. Before each level the standing stock is labelled
    by edge connectivity. Each engaged run is a cut piece; it belongs to the
    stock component from which it removes the most cells (no tuned number).

    Rules (each from the raster or the tool, no tuned number):
    - ring: a closed curve whose air lies only within R of arc before its
      seam is a ring. That air is the loop's own start: the tool disc at the
      seam already cut R ahead of the end.
    - a run of one sample (no length) or one that removes no cell is not a
      piece; its stock stays and is counted as coverage left.
    Returns the pieces, the labels per level, the components per level, the
    curves and the stock after the last level."""
    rp = st.Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    here = sh.seed + np.array([R, 0.0])
    pieces, labs, ncomp = [], [], []
    curves = []
    dropped = []
    for k, lv in enumerate(levels):
        lab, nl = ndimage.label(rp.stock, structure=FOUR)
        labs.append(lab)
        ncomp.append(nl)
        left = list(range(len(lv)))
        while left:
            def dist(c):
                P = lv[c]["P"]
                if lv[c]["closed"]:
                    return float(np.hypot(*(P - here).T).min())
                return float(min(np.hypot(*(P[0] - here)), np.hypot(*(P[-1] - here))))
            c = min(left, key=dist)
            left.remove(c)
            P, closed = lv[c]["P"], lv[c]["closed"]
            V = np.vstack([P, P[:1]]) if closed else P
            sv = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(V, axis=0).T))])
            T = sv[-1]
            n = max(8, int(math.ceil(T / TOL))) if closed else max(2, int(math.ceil(T / TOL)) + 1)
            u = np.linspace(0, T, n + 1) if closed else np.linspace(0, T, n)
            Q = np.stack([np.interp(u, sv, V[:, 0]), np.interp(u, sv, V[:, 1])], 1)
            w, a, ls = cut_labelled(rp, Q, lab)
            e = engaged(u, a, closed)
            rr = runs(e, closed)
            ring = rr is None
            if closed and rr is not None and len(rr) == 1:
                air = np.nonzero(~e[:-1])[0]
                if (T - u[air]).max() <= R and (T - u[air]).min() > 0:
                    ring = True
            curves.append(dict(level=k, curve=c, V=V, sv=sv, u=u, Q=Q, closed=closed, e=e, w=w, a=a,
                               nruns=-1 if ring else len(rr)))
            here = Q[-1].copy()
            if ring:
                rr = [(0, len(Q) - 1)]
            for i, j in rr:
                idx = np.arange(i, j + 1) if j >= i else np.r_[np.arange(i, len(Q) - 1), np.arange(0, j + 1)]
                tot = {}
                for t in idx:
                    for lb, cnt in zip(*ls[t]):
                        if lb > 0:
                            tot[int(lb)] = tot.get(int(lb), 0) + int(cnt)
                cells = sum(tot.values())
                if len(idx) < 2 or cells == 0:
                    dropped.append(dict(level=k, at=Q[i].copy(), samples=len(idx), cells=cells))
                    continue
                comp = max(tot, key=tot.get)
                Pp = V.copy() if ring else sub_polyline(V, sv, u[i], u[j], closed)
                pieces.append(dict(level=k, curve=c, ring=ring, P=Pp, comp=comp, u0=float(u[i]), u1=float(u[j]),
                                   cells=cells, share=tot[comp] / cells,
                                   length=float(np.hypot(*np.diff(Pp, axis=0).T).sum())))
    out = dict(pieces=pieces, labs=labs, ncomp=ncomp, curves=curves, stock=rp.stock.copy(), dropped=dropped)
    merge_short_gaps(out)
    return out


def merge_short_gaps(cls):
    """Two pieces of one curve and one stock component with an air gap
    shorter than one pass step between them are one piece: the engaged
    split reads over one pass step, so it cannot resolve a shorter gap.
    The gap is cut through on the curve (the shape is kept)."""
    curves = {(c["level"], c["curve"]): c for c in cls["curves"]}
    by = {}
    for p in cls["pieces"]:
        by.setdefault((p["level"], p["curve"]), []).append(p)
    out = []
    merged = []
    for key, ps in by.items():
        c = curves[key]
        T = c["sv"][-1]
        ps = sorted(ps, key=lambda p: p["u0"])
        if len(ps) == 1 or ps[0]["ring"]:
            out += ps
            continue
        changed = True
        while changed and len(ps) > 1:
            changed = False
            n = len(ps)
            for a in range(n if c["closed"] else n - 1):
                p, q = ps[a], ps[(a + 1) % n]
                g = (q["u0"] - p["u1"]) % T if c["closed"] else q["u0"] - p["u1"]
                if g < PASS_STEP and p["comp"] == q["comp"]:
                    m = dict(p)
                    m["u1"] = q["u1"]
                    m["P"] = sub_polyline(c["V"], c["sv"], m["u0"], m["u1"], c["closed"])
                    m["length"] = float(np.hypot(*np.diff(m["P"], axis=0).T).sum())
                    m["cells"] = p["cells"] + q["cells"]
                    m["share"] = min(p["share"], q["share"])
                    merged.append(dict(level=key[0], gap_mm=float(g)))
                    ps = [x for k, x in enumerate(ps) if k not in (a, (a + 1) % n)] + [m]
                    ps = sorted(ps, key=lambda p: p["u0"])
                    changed = True
                    break
        out += ps
    out.sort(key=lambda p: (p["level"], p["curve"], p["u0"]))
    cls["pieces"] = out
    cls["merged"] = merged


# ── lobes ──────────────────────────────────────────────────────────────

def lobe_tree(labs, pieces):
    """Node (k, l): component l of the standing stock before level k. Its
    parent is the component before level k-1 that holds its cells (stock
    only shrinks, so there is exactly one). A lobe is a maximal chain of
    nodes with one child each; it ends where its component splits in two or
    more components that still get pieces, or where it gets no more pieces."""
    used = {(p["level"], p["comp"]) for p in pieces}
    parent = {}
    for k in range(1, len(labs)):
        lab, prev = labs[k], labs[k - 1]
        m = lab > 0
        pairs = np.unique(np.stack([lab[m], prev[m]], 1), axis=0)
        for l, q in pairs:
            parent[(k, int(l))] = (k - 1, int(q))
    # keep only nodes on a path to a node with pieces
    live = set()
    for n in used:
        while n is not None and n not in live:
            live.add(n)
            n = parent.get(n)
    children = {}
    for n in live:
        p = parent.get(n)
        if p is not None and p in live:
            children.setdefault(p, []).append(n)
    roots = sorted(n for n in live if parent.get(n) not in live)
    # chains
    lobe_of = {}
    lobes = []

    def chain(n, up):
        lid = len(lobes)
        nodes = [n]
        while len(children.get(nodes[-1], [])) == 1:
            nodes.append(children[nodes[-1]][0])
        lobes.append(dict(id=lid, nodes=nodes, parent=up, children=[]))
        for x in nodes:
            lobe_of[x] = lid
        for c in sorted(children.get(nodes[-1], [])):
            cid = chain(c, lid)
            lobes[lid]["children"].append(cid)
        return lid

    for r in roots:
        chain(r, None)
    for p in pieces:
        p["lobe"] = lobe_of[(p["level"], p["comp"])]
    return lobes, lobe_of
