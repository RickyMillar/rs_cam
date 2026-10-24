# C19: order and link the cut pieces of clip.classify, and replay the plan.
#
# Order (the brief, item 3):
#   - lobes depth first: a lobe is cut to its end, then its child lobes, the
#     nearest first, each to its end;
#   - inside a lobe a piece may be cut when every piece of a lower level of
#     the same lobe whose tool discs overlap it (centre paths within 2R) is
#     cut: only those change the stock it meets. Among those, the nearest
#     entry next.
# Joins (C14 one-way or zigzag, else FreeCAD's route, else a plunge):
#   - ring (core): the seam join of C17 (nearest point of the next ring);
#   - turn: open piece to open piece, straight, at most the C14 bound and
#     inside the tool-centre region;
#   - otherwise plan.Planner.route: a keep-down link (string pulled) or a
#     plunge (a retract and a re-entry). A FreeCAD "feed" (a straight cut
#     through stock) is counted as a plunge: the rs_cam ceiling admits no
#     step over c that is not a turn (C14 §(6)).
import math

import numpy as np
from scipy.spatial import cKDTree

import clip as cl
from clip import R, S, D, CD, C_FRAC, TOL, PASS_STEP, CELL, TURN_BOUND, pn, st
from march import resample  # noqa: E402  (C15)


def inside_seg(sh, A, B):
    """The straight step A-B lies in the tool-centre region (a point within
    one cell outside is the curve's own error, as C17's ClipField reads it)."""
    n = max(2, int(math.ceil(np.hypot(*(B - A)) / TOL)) + 1)
    t = np.linspace(0, 1, n)
    P = A[None] + t[:, None] * (B - A)[None]
    ok = sh.in_omega(P[:, 0], P[:, 1]) | (sh.wall_dist(P[:, 0], P[:, 1]) > -TOL)
    if sh.name == "star":
        ok &= st.STAR_INSIDE(P)
    return bool(ok.all())


def theta_at(P, E_first=True):
    """C16 task 4: the normal of the arc the join goes onto, from its chord
    over one pass step from its entry."""
    Q = P if E_first else P[::-1]
    s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(Q, axis=0).T))])
    L = min(PASS_STEP, s[-1])
    q = np.array([np.interp(L, s, Q[:, 0]), np.interp(L, s, Q[:, 1])])
    t = q - Q[0]
    n = np.hypot(*t)
    return None if n == 0 else np.array([-t[1], t[0]]) / n


def climb_share(rp, P):
    """Share of the samples that meet stock with more stock on the right of
    the travel than on the left (climb, C14 §(3)); read before the cut."""
    Q = resample(P, TOL) if len(P) >= 2 else P
    if len(Q) < 3:
        return 0, 0
    tang = np.gradient(Q, axis=0)
    right = left = 0
    for k in range(0, len(Q), 5):
        ii, jj, dx, dy, inside = rp._window(*Q[k])
        mat = rp.stock[np.ix_(ii, jj)] & inside
        if not mat.any():
            continue
        cr = (tang[k, 0] * dy - tang[k, 1] * dx)[mat]  # > 0: left of travel
        r, l_ = int((cr < 0).sum()), int((cr > 0).sum())
        right += r > l_
        left += l_ > r
    return right, left


class Build:
    def __init__(self, sh, cls, lobes, mode, head=False, slanted=False):
        self.sh = sh
        self.mode = mode
        self.head = head
        self.slanted = slanted  # admit a turn up to C16's slanted bound
        self.pl = pn.Planner(sh)
        self.pieces = cls["pieces"]
        self.curves = {(c["level"], c["curve"]): c for c in cls["curves"]}
        self.lobes = lobes
        self.done = set()
        self.joins = []  # one record per join
        self.turns = []
        self.returns = []
        self.prev = None  # index of the piece cut last
        self.order = []
        self.climb = [0, 0]
        self.dir_flip = 0
        # dependencies inside a lobe
        self.deps = {}
        trees = [cKDTree(resample(p["P"], TOL)) for p in self.pieces]
        for i, q in enumerate(self.pieces):
            Qi = resample(q["P"], TOL)
            self.deps[i] = [j for j, p in enumerate(self.pieces)
                            if p["lobe"] == q["lobe"] and p["level"] < q["level"]
                            and float(trees[j].query(Qi)[0].min()) <= 2 * R]

    # ── the tool's place and a piece's entry ───────────────────────────
    @property
    def here(self):
        return self.pl.here

    def entry(self, i):
        """(entry point, oriented polyline) of piece i from the tool's place."""
        p = self.pieces[i]
        P = p["P"]
        if p["ring"]:
            Q = P[:-1]
            k = int(np.argmin(np.hypot(*(Q - self.here).T)))
            P = np.vstack([Q[k:], Q[:k], Q[k:k + 1]])
            return P[0], P
        if self.mode == "zigzag":
            if np.hypot(*(P[-1] - self.here)) < np.hypot(*(P[0] - self.here)):
                P = P[::-1]
        return P[0], P

    # ── the head of a piece ────────────────────────────────────────────
    def split_head(self, P):
        """(head, body): the head is the leading part of the piece that runs
        on the wall (within half a cell of it: C17 refine's on-wall test), up
        to and with the point where the piece leaves the wall. A piece that
        never leaves the wall has no head."""
        if not self.head:
            return None, P
        wd = self.sh.wall_dist(P[:, 0], P[:, 1])
        on = wd <= TOL / 2
        if not on[0]:
            return None, P
        k = int(np.argmin(on)) if not on.all() else len(P)
        if k >= len(P) - 1:
            return None, P
        # the last on-wall point is where the piece leaves the wall
        return P[:k], P[k - 1:]

    def entry_from(self, i, X):
        """The entry point of piece i for a tool at X."""
        p = self.pieces[i]
        P = p["P"]
        if p["ring"]:
            return P[int(np.argmin(np.hypot(*(P - X).T)))]
        if self.mode == "zigzag" and np.hypot(*(P[-1] - X)) < np.hypot(*(P[0] - X)):
            return P[-1]
        return P[0]

    # ── a join ─────────────────────────────────────────────────────────
    def join(self, i, E, P, Hd):
        """Join the tool to piece i. With a head Hd the turn is the move from
        the tool along the head to the point where the piece leaves the wall
        (C14: the short step-over at the arc end). Returns True when the head
        was cut as the turn."""
        X = self.here.copy()
        q = self.pieces[i]
        prv = self.pieces[self.prev] if self.prev is not None else None
        d = float(np.hypot(*(E - X)))
        tp = np.vstack([X, Hd]) if Hd is not None else np.array([X, E])
        tlen = float(np.hypot(*np.diff(tp, axis=0).T).sum())
        T_end = tp[-1]
        rec = dict(to=i, frm=self.prev, A=X, B=E.copy(), length=d, turn_len=tlen, level=q["level"], lobe=q["lobe"],
                   head=Hd is not None)
        if prv is None or prv["ring"] or q["ring"]:
            rec["cls"] = "seam"
        elif prv["lobe"] != q["lobe"]:
            rec["cls"] = "lobe"
        elif self.prev in self.deps[i]:
            # the next strip of the piece just cut: a turn (C14)
            rec["cls"] = "arc"
        else:
            # a piece of the same lobe that does not border the one just
            # cut: a link inside the lobe, not a turn
            rec["cls"] = "jump"
        if rec["cls"] in ("arc", "lobe", "jump"):
            body = P[len(Hd) - 1:] if Hd is not None else P
            n = theta_at(body)
            dd = float(np.hypot(*(T_end - X)))
            if n is not None and dd > 0:
                c = abs(float(np.dot((T_end - X) / dd, n)))
                rec["theta_deg"] = math.degrees(math.acos(min(1.0, c)))
                rec["slanted_bound"] = S / max(c, 1e-12) + (1 + math.sqrt(2)) * CELL
        inside = all(inside_seg(self.sh, tp[k], tp[k + 1]) for k in range(len(tp) - 1))
        bound = TURN_BOUND
        if self.slanted and "slanted_bound" in rec:
            bound = max(bound, rec["slanted_bound"])
        rec["bound"] = bound
        if d <= TOL and Hd is None:
            rec["kind"] = "none"
        elif rec["cls"] == "arc" and tlen <= bound and inside:
            rec["kind"] = "turn"
            k0 = len(self.pl.pieces)
            self.pl._cut(tp, "turn")
            t = self.pl.pieces[k0]
            wmax = float(t["w"].max()) / D
            amax = float(t["apm"].max()) / D
            m = max(wmax, amax)
            f = C_FRAC / m if m > C_FRAC else 1.0
            t["feed_factor"] = f
            t["turn_len"] = tlen
            self.turns.append(dict(A=X, B=T_end.copy(), length=tlen, swept=wmax, area=amax, factor=f,
                                   over=bool(m > C_FRAC), level=q["level"], lobe=q["lobe"], head=Hd is not None,
                                   head_mm=float(np.hypot(*np.diff(Hd, axis=0).T).sum()) if Hd is not None else 0.0))
            rec["swept"], rec["area"], rec["factor"] = wmax, amax, f
            self.joins.append(rec)
            return Hd is not None
        else:
            rec["outside"] = rec["cls"] == "arc" and tlen <= bound
            rec["over_bound"] = rec["cls"] == "arc" and tlen > bound
            # the brief: a turn over the bound is counted as a plunge
            kind, path = ("over bound", None) if rec["over_bound"] else self.pl.route(X, E.copy())
            rec["route"] = kind
            ok = False
            if kind == "link":
                # FreeCAD's test leaves 2 s R at each end out, so a short
                # "link" can step head-on into stock. The rs_cam replay
                # admits no step over the ceiling that is not a turn
                # (C14 §(6)): such a link is a plunge.
                saved = self.pl.rp.stock.copy()
                k0 = len(self.pl.pieces)
                here0 = self.pl.here.copy()
                if len(path) >= 2 and np.hypot(*(path[-1] - path[0])) > 0:
                    self.pl._cut(path, "link")
                wmax = max((float(p["w"].max()) / D for p in self.pl.pieces[k0:]), default=0.0)
                rec["link_reading"] = wmax
                if wmax <= C_FRAC + cl.QUANTUM:
                    ok = True
                    rec["kind"] = "link"
                    rec["link_mm"] = float(np.hypot(*np.diff(path, axis=0).T).sum())
                else:
                    rec["route"] = "link over c"
                    self.pl.rp.stock = saved
                    del self.pl.pieces[k0:]
                    self.pl.here = here0
            if not ok:
                rec["kind"] = "plunge"
                self.pl.pieces.append(dict(kind="retract", P=np.array([X, E]), level=q["level"]))
                self.pl.here = E.copy()
        self.joins.append(rec)
        return False

    # ── a piece ────────────────────────────────────────────────────────
    def cut(self, i, chained=None):
        """Join to piece i and cut it. chained: None for a normal join, or
        the travel direction (True: along the curve) of a piece reached by a
        keep-down link along its own curve. Returns the polyline cut."""
        q = self.pieces[i]
        if chained is None:
            E, P = self.entry(i)
            Hd, body = (None, P) if q["ring"] else self.split_head(P)
            if self.join(i, E, P, Hd):
                P = body
        else:
            P = q["P"] if chained else q["P"][::-1]
        r, l_ = climb_share(self.pl.rp, P)
        self.climb[0] += r
        self.climb[1] += l_
        k0 = len(self.pl.pieces)
        self.pl._cut(P, "cut")
        self.pl.pieces[k0]["src"] = i
        self.pl.pieces[k0]["lobe"] = q["lobe"]
        self.pl.pieces[k0]["join"] = self.joins[-1]
        self.pl.pieces[k0]["after_plunge"] = self.joins[-1]["kind"] == "plunge"
        self.done.add(i)
        self.order.append(i)
        self.prev = i
        return P

    def forward(self, i, P):
        """True when P runs along piece i's curve direction."""
        Q = self.pieces[i]["P"]
        return bool(np.hypot(*(P[-1] - Q[-1])) <= np.hypot(*(P[-1] - Q[0])))

    def successor(self, i, fwd):
        """The next piece along the same curve in the travel direction, of
        the same lobe, not cut and with its dependencies cut; and the air
        run of the curve between them."""
        p = self.pieces[i]
        if p["ring"]:
            return None, None
        c = self.curves[(p["level"], p["curve"])]
        T = c["sv"][-1]
        best, bd_ = None, math.inf
        for j, q in enumerate(self.pieces):
            if (q["level"], q["curve"]) != (p["level"], p["curve"]) or j in self.done or q["lobe"] != p["lobe"]:
                continue
            if not all(k in self.done for k in self.deps[j]):
                continue
            g = (q["u0"] - p["u1"]) if fwd else (p["u0"] - q["u1"])
            if c["closed"]:
                g %= T
            if 0 <= g < bd_:
                best, bd_ = j, g
        if best is None:
            return None, None
        q = self.pieces[best]
        if fwd:
            air = cl.sub_polyline(c["V"], c["sv"], p["u1"], q["u0"], c["closed"])
        else:
            air = cl.sub_polyline(c["V"], c["sv"], q["u1"], p["u0"], c["closed"])[::-1]
        return best, air

    def keep_down(self, paths, kind):
        """Cut the shortest of the candidate paths whose every sample reads
        at most c (plus one cell) as a keep-down move of this kind. Returns
        its length, or None when none holds."""
        cand = sorted((float(np.hypot(*np.diff(P, axis=0).T).sum()), k, P) for k, P in enumerate(paths)
                      if P is not None and len(P) >= 2)
        for L, _, P in cand:
            saved = self.pl.rp.stock.copy()
            k0 = len(self.pl.pieces)
            here0 = self.pl.here.copy()
            self.pl._cut(P, kind)
            wmax = max((float(p["w"].max()) / D for p in self.pl.pieces[k0:]), default=0.0)
            if wmax <= C_FRAC + cl.QUANTUM:
                return L
            self.pl.rp.stock = saved
            del self.pl.pieces[k0:]
            self.pl.here = here0
        return None

    def chain(self, i, P):
        """After piece i (cut along P), cut the next pieces of the same curve
        through the curve's own air runs (or a shorter keep-down route), then
        in one-way mode return to where the chain began."""
        start = P[0].copy()
        fwd = self.forward(i, P)
        back = [P]
        while True:
            j, air = self.successor(i, fwd)
            if j is None:
                break
            q = self.pieces[j]
            A = (q["P"] if fwd else q["P"][::-1])[0]
            kind, path = self.pl.route(self.here.copy(), A.copy())
            # nearest next: the chain goes on only when its keep-down link
            # is not longer than the step to the nearest other available
            # entry from where the next turn would start
            Lc = min(float(np.hypot(*np.diff(P_, axis=0).T).sum()) for P_ in (air, path) if P_ is not None
                     and (P_ is air or kind == "link"))
            origin = start if self.mode == "oneway" else self.here
            other = [k for k in self.lobe_pieces(q["lobe"]) if k not in self.done and k != j
                     and all(m in self.done for m in self.deps[k])]
            if other and min(float(np.hypot(*(self.entry_from(k, origin) - origin))) for k in other) < Lc:
                break
            L = self.keep_down([air, path if kind == "link" else None], "link")
            if L is None:
                break
            self.joins.append(dict(to=j, frm=i, A=back[-1][-1].copy(), B=A.copy(), length=L, turn_len=L,
                                   level=q["level"], lobe=q["lobe"], cls="chain", kind="link", head=False,
                                   link_mm=L))
            back.append(air)
            back.append(self.cut(j, chained=fwd))
            i = j
        if self.mode == "oneway" and not self.pieces[i]["ring"]:
            # keep-down return to where the chain began: the shorter of the
            # way back along the cut curve (C14) and the string-pulled route
            # through cleared stock (C17)
            along = np.vstack(back)[::-1]
            Lb = float(np.hypot(*np.diff(along, axis=0).T).sum())
            kind, path = self.pl.route(self.here.copy(), start)
            L = self.keep_down([along, path if kind == "link" else None], "return")
            if L is None:  # cannot happen: the way back is through what was just cut
                raise RuntimeError("no return")
            self.returns.append(dict(piece=i, mm=L, along=abs(L - Lb) < 1e-9, piece_mm=Lb))

    # ── the order ──────────────────────────────────────────────────────
    def lobe_pieces(self, lid):
        return [i for i, p in enumerate(self.pieces) if p["lobe"] == lid]

    def run_lobe(self, lid):
        mine = self.lobe_pieces(lid)
        while True:
            left = [i for i in mine if i not in self.done]
            if not left:
                break
            avail = [i for i in left if all(j in self.done for j in self.deps[i])]
            if not avail:  # cannot happen: the lowest level of the lobe has no dependency
                raise RuntimeError("no piece available")
            i = min(avail, key=lambda i: float(np.hypot(*(self.entry(i)[0] - self.here))))
            self.chain(i, self.cut(i))
        kids = list(self.lobes[lid]["children"])
        while kids:
            def first(c):
                ps = [i for i in self.subtree_pieces(c) if not self.deps[i] or
                      all(j in self.done for j in self.deps[i])]
                return min((float(np.hypot(*(self.entry(i)[0] - self.here))) for i in ps), default=math.inf)
            c = min(kids, key=first)
            kids.remove(c)
            self.run_lobe(c)

    def subtree_pieces(self, lid):
        out = self.lobe_pieces(lid)
        for c in self.lobes[lid]["children"]:
            out += self.subtree_pieces(c)
        return out

    def run(self):
        roots = [L["id"] for L in self.lobes if L["parent"] is None]
        while roots:
            c = min(roots, key=lambda c: min((float(np.hypot(*(self.entry(i)[0] - self.here)))
                                              for i in self.subtree_pieces(c)), default=math.inf))
            roots.remove(c)
            self.run_lobe(c)
        return self
