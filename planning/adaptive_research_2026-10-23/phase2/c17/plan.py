# C17 additions (operator, 2026-10-24): a plan is CUT parts joined by LINK
# parts. This module orders the curves of a plan, links them keep-down where
# FreeCAD's rule finds a route through cleared stock (else a retract), and
# replays the whole tool path in order with the C15 raster.
#
# Sources:
#   FreeCAD link rule: R1_freecad_adaptive.md §4 (ideas only, no code):
#     a link is clear when the tool disc grown by s*R (s = stepover / D)
#     sweeps no stock; the test starts 2*s*R inside each end (half the direct
#     length, clearance 0, when 2*s*R is more than half of it); a route is
#     accepted up to 3 x the direct length (keepToolDownDistRatio); a gap
#     shorter than 2*s*R may be cut straight when each step cuts at most 1.5 x
#     the target; otherwise a retract.
#   Feeds: fixture_feeds(Adaptive) = (1500, 500) mm/min (cut, plunge),
#     crates/rs_cam_core/src/compute/fixture_values.rs:34-35 (the op has no
#     feed default, S11). A 2D adaptive keep-down Link is fed at the cut feed:
#     adaptive/path.rs:3163-3168 (params.feed_rate). The link-moves dressup
#     has its own feed, default 500 mm/min: compute/config.rs:1077-1083
#     (LinkDressupParams), field "link_feed_rate" (config.rs:1439).
#   Engaged: a sample is engaged when the stock it removes, read as area per
#     mm over one pass step (the planner's step) centred on it, is more than
#     one cell width (H = 0.1 mm, the raster's resolution: H^2 per H of path).
import math

import numpy as np
from scipy import ndimage
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import dijkstra

from stream import R, S, D, CD, C_FRAC, TOL, PASS_STEP, H, Replay
from march import resample

F_CUT = 1500.0  # mm/min, fixture_feeds(Adaptive)
F_LINK_DRESSUP = 500.0  # mm/min, LinkDressupParams::default
S_FRAC = S / D  # FreeCAD's stepOverFactor
CLEAR = S_FRAC * R  # FreeCAD clearance s*R (mm)
ROUTE_RATIO = 3.0  # keepToolDownDistRatio default (R1 §4)
CUT_THROUGH = 1.5  # IsAllowedToCutTrough factor (R1 §4)


def is_closed(P):
    return len(P) > 2 and math.hypot(*(P[0] - P[-1])) <= TOL


class Planner:
    def __init__(self, sh):
        self.sh = sh
        self.rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
        self.pieces = []  # dict(kind=cut|link|feed|retract, P, w, a, eng, interior)
        self.here = sh.seed + np.array([R, 0.0])  # the starter's last lap ends here
        self.level = 0

    # ── replay a piece ────────────────────────────────────────────────
    def _cut(self, P, kind):
        Q = resample(P, TOL) if len(P) >= 2 else P
        if len(Q) < 2:
            return
        w, a = self.rp.cut(Q)
        s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(Q, axis=0).T))])
        L = s[-1]
        # area per mm over one pass step centred on each sample
        cs = np.concatenate([[0], np.cumsum(a)])
        lo = np.searchsorted(s, s - PASS_STEP / 2, side="left")
        hi = np.searchsorted(s, s + PASS_STEP / 2, side="right")
        span = np.maximum(s[np.minimum(hi, len(s)) - 1] - s[lo], TOL)
        apm = (cs[hi] - cs[lo]) / span
        eng = apm > H
        interior = (s > R) & (s < L - R)
        self.pieces.append(dict(kind=kind, P=Q, w=w, a=a, eng=eng, interior=interior, s=s, apm=apm,
                                level=self.level))
        self.here = Q[-1].copy()

    # ── stock distance at tool-centre nodes, in a window ───────────────
    def _clear_nodes(self, x0, x1, y0, y1, clr):
        sh = self.sh
        m = R + clr + 2 * H
        j0 = max(0, int(math.floor((x0 - m - sh.mx[0]) / H)))
        j1 = min(len(sh.mx), int(math.ceil((x1 + m - sh.mx[0]) / H)) + 1)
        i0 = max(0, int(math.floor((y0 - m - sh.my[0]) / H)))
        i1 = min(len(sh.my), int(math.ceil((y1 + m - sh.my[0]) / H)) + 1)
        stock = self.rp.stock[i0:i1, j0:j1]
        if stock.any():
            edt = ndimage.distance_transform_edt(~stock) * H
        else:
            edt = np.full(stock.shape, np.inf)
        cj0 = max(0, int(math.floor((x0 - sh.xs[0]) / H)))
        cj1 = min(len(sh.xs), int(math.ceil((x1 - sh.xs[0]) / H)) + 1)
        ci0 = max(0, int(math.floor((y0 - sh.ys[0]) / H)))
        ci1 = min(len(sh.ys), int(math.ceil((y1 - sh.ys[0]) / H)) + 1)
        X, Y = sh.GX[ci0:ci1, cj0:cj1], sh.GY[ci0:ci1, cj0:cj1]
        ii = np.clip(np.round((Y - sh.my[0]) / H).astype(int) - i0, 0, edt.shape[0] - 1)
        jj = np.clip(np.round((X - sh.mx[0]) / H).astype(int) - j0, 0, edt.shape[1] - 1)
        inside_m = (Y >= sh.my[0]) & (Y <= sh.my[-1]) & (X >= sh.mx[0]) & (X <= sh.mx[-1])
        dist = np.where(inside_m, edt[ii, jj], np.inf)
        om = sh.omega[ci0:ci1, cj0:cj1]
        return X, Y, om, dist

    def route(self, A, B):
        """FreeCAD's rule (R1 §4): ('link', points) keep-down, ('feed', [A, B])
        a short straight cut, or ('retract', [A, B])."""
        d = float(np.hypot(*(B - A)))
        if d <= TOL:
            return "link", np.array([A, B])
        end = min(2 * CLEAR, d / 2)
        clr = CLEAR if 2 * CLEAR <= d / 2 else 0.0
        half = ROUTE_RATIO * d / 2  # the ellipse |n-A| + |n-B| <= 3d
        c = (A + B) / 2
        X, Y, om, dist = self._clear_nodes(c[0] - half, c[0] + half, c[1] - half, c[1] + half, clr)
        dA = np.hypot(X - A[0], Y - A[1])
        dB = np.hypot(X - B[0], Y - B[1])
        ok = om & ((dist > R + clr) | (dA <= end) | (dB <= end)) & (dA + dB <= ROUTE_RATIO * d + 2 * H)
        # straight segment first
        n = max(2, int(math.ceil(d / H)) + 1)
        t = np.linspace(0, 1, n)
        seg = A[None] + t[:, None] * (B - A)[None]
        si = np.clip(np.round((seg[:, 1] - Y[0, 0]) / H).astype(int), 0, X.shape[0] - 1)
        sj = np.clip(np.round((seg[:, 0] - X[0, 0]) / H).astype(int), 0, X.shape[1] - 1)
        dsA = np.hypot(seg[:, 0] - A[0], seg[:, 1] - A[1])
        dsB = np.hypot(seg[:, 0] - B[0], seg[:, 1] - B[1])
        seg_ok = (dist[si, sj] > R + clr) | (dsA <= end) | (dsB <= end)
        seg_in = self.sh.in_omega(seg[:, 0], seg[:, 1])
        if seg_ok.all() and seg_in.all():
            return "link", np.array([A, B])
        # keep-down route: shortest 8-connected path over the clear nodes
        path = self._dijkstra(X, Y, ok, A, B)
        if path is not None:
            L = float(np.hypot(*np.diff(path, axis=0).T).sum())
            if L <= ROUTE_RATIO * d:
                return "link", path
        if d < 2 * CLEAR and seg_in.all() and self._cut_through_ok(seg):
            return "feed", np.array([A, B])
        return "retract", np.array([A, B])

    def _dijkstra(self, X, Y, ok, A, B):
        ny, nx = ok.shape
        idx = -np.ones(ok.shape, int)
        r, c = np.nonzero(ok)
        if len(r) == 0:
            return None
        idx[r, c] = np.arange(len(r))
        rows, cols, wts = [], [], []
        for dr, dc in ((0, 1), (1, 0), (1, 1), (1, -1)):
            rr, cc = r + dr, c + dc
            m = (rr >= 0) & (rr < ny) & (cc >= 0) & (cc < nx)
            rr, cc, r0, c0 = rr[m], cc[m], r[m], c[m]
            m2 = ok[rr, cc]
            rows.append(idx[r0[m2], c0[m2]])
            cols.append(idx[rr[m2], cc[m2]])
            wts.append(np.full(m2.sum(), H * math.hypot(dr, dc)))
        rows, cols, wts = np.concatenate(rows), np.concatenate(cols), np.concatenate(wts)
        G = coo_matrix((np.concatenate([wts, wts]), (np.concatenate([rows, cols]), np.concatenate([cols, rows]))),
                       shape=(len(r), len(r))).tocsr()
        pa = np.argmin(np.where(ok, np.hypot(X - A[0], Y - A[1]), np.inf))
        pb = np.argmin(np.where(ok, np.hypot(X - B[0], Y - B[1]), np.inf))
        ia, ib = idx.flat[pa], idx.flat[pb]
        if ia < 0 or ib < 0:
            return None
        dist, pred = dijkstra(G, indices=ia, return_predecessors=True)
        if not np.isfinite(dist[ib]):
            return None
        seq = [ib]
        while seq[-1] != ia:
            seq.append(pred[seq[-1]])
        seq = seq[::-1]
        pts = np.stack([X[r[seq], c[seq]], Y[r[seq], c[seq]]], 1)
        return np.vstack([A, pts, B])

    def _cut_through_ok(self, seg):
        """A straight feed through stock is allowed when no step of the
        segment cuts more than 1.5 x the target (area per mm at c*D)."""
        saved = self.rp.stock.copy()
        w, a = self.rp.cut(seg)
        self.rp.stock = saved
        step = float(np.hypot(*(seg[1] - seg[0])))
        return bool((a / step).max() <= CUT_THROUGH * CD) if len(a) and step > 0 else True

    # ── a plan ────────────────────────────────────────────────────────
    def add_curve(self, P):
        """Link to the curve (rotated or reversed to start nearest the
        current place) and cut it."""
        if is_closed(P):
            Q = P[:-1]
            k = int(np.argmin(np.hypot(*(Q - self.here).T)))
            P = np.vstack([Q[k:], Q[:k], Q[k:k + 1]])
        elif np.hypot(*(P[-1] - self.here)) < np.hypot(*(P[0] - self.here)):
            P = P[::-1]
        kind, path = self.route(self.here.copy(), P[0].copy())
        if kind == "retract":
            self.pieces.append(dict(kind="retract", P=path, level=self.level))
            self.here = P[0].copy()
        elif len(path) >= 2 and np.hypot(*(path[-1] - path[0])) > 0:
            self._cut(path, kind)
        self._cut(P, "cut")

    def add_level(self, curves):
        left = list(curves)
        while left:
            def dist(P):
                if is_closed(P):
                    return float(np.hypot(*(P - self.here).T).min())
                return float(min(np.hypot(*(P[0] - self.here)), np.hypot(*(P[-1] - self.here))))
            k = int(np.argmin([dist(P) for P in left]))
            self.add_curve(left.pop(k))


def run_interior(s, e):
    """Samples more than R of arc from both ends of their engaged run (a CUT
    part): the cut part's interior."""
    out = np.zeros(len(s), bool)
    if not e.any():
        return out
    ch = np.diff(e.astype(int))
    starts = list(np.nonzero(ch == 1)[0] + 1)
    ends = list(np.nonzero(ch == -1)[0])
    if e[0]:
        starts = [0] + starts
    if e[-1]:
        ends = ends + [len(s) - 1]
    for a, b in zip(starts, ends):
        k = np.arange(a, b + 1)
        out[k] = (s[k] - s[a] > R) & (s[b] - s[k] > R)
    return out


def summarise(pl, sh):
    """Lengths, times, loads and the residue of a finished plan."""
    eng_len = air_len = 0.0
    eng_cut = eng_link = air_cut = air_link = 0.0
    w_int, w_all = [], []
    retracts = links = feeds = 0
    link_len = 0.0
    for p in pl.pieces:
        if p["kind"] == "retract":
            retracts += 1
            continue
        ds = np.diff(p["s"])
        e = p["eng"]
        seg_e = 0.5 * (e[1:].astype(float) + e[:-1])  # a segment between an engaged and an air sample counts half
        le = float((ds * seg_e).sum())
        la = float(ds.sum()) - le
        if p["kind"] == "cut":
            eng_cut += le
            air_cut += la
            m = run_interior(p["s"], e) & p["interior"]
            if m.any():
                w_int.append(p["w"][m] / D)
            if e.any():
                w_all.append(p["w"][e] / D)
        else:
            links += p["kind"] == "link"
            feeds += p["kind"] == "feed"
            link_len += float(ds.sum())
            eng_link += le
            air_link += la
    eng_len = eng_cut + eng_link
    air_len = air_cut + air_link
    w_int = np.concatenate(w_int) if w_int else np.zeros(1)
    w_all = np.concatenate(w_all) if w_all else np.zeros(1)
    from stream import residue
    res = residue(sh, pl.rp.stock & sh.stock0)
    return dict(
        engaged_mm=eng_len, air_mm=air_len, engaged_cut_mm=eng_cut, air_cut_mm=air_cut,
        engaged_link_mm=eng_link, air_link_mm=air_link, link_mm=link_len,
        links=int(links), feeds=int(feeds), retracts=int(retracts),
        time_cut_feed_min=(eng_len + air_len) / F_CUT,
        time_link_feed_min=eng_len / F_CUT + air_len / F_LINK_DRESSUP,
        cut_width_max=float(w_int.max()), cut_width_over=int((w_int > C_FRAC + TOL / D).sum()),
        cut_width_all_max=float(w_all.max()), cut_width_all_over=int((w_all > C_FRAC + TOL / D).sum()),
        residue_mm2=res["mm2"], residue_unreachable_mm2=res["unreachable_mm2"],
    )
