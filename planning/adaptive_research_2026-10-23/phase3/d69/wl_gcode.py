"""D69: the Waterline section of a program.nc as passes, and three re-orderings of it, timed afterwards by
`rs_cam_cli nc-time` (the F-034 integrator). Nothing here plans geometry: every variant re-uses the rings that the shipped
op emitted, so no XY position is new except the short in-hole links of the helical variants (checked: each link chord
lies inside the ring it starts on, see `inside`).

Pass = the moves from one retract to safe Z to the next. Inside a pass: the approach (G0 down), the entry (the moves
above the ring Z) and the ring (the closed run of moves at the ring Z, start = end).
Chain = the passes of one region over the levels: consecutive levels whose rings match one to one. Two rings match
when one lies inside the other (to within the op's sampling) and no third ring of the two levels lies between them;
the match must be the only one both ways. No distance threshold: the rule is topological. Width of a chain step = the Hausdorff distance of the two rings (vertex sampled, arcs linearised).

Variants:
  base    : the shipped order (level by level).
  region  : region first; each chain top to bottom, chains in nearest-neighbour order; each pass keeps its own retract,
            rapid, approach and entry (only the order changes).
  helix   : region first; a chain of n rings is ONE entry: the first ring flat at its level, then for each next level a
            feed link inside the ring to the nearest vertex of the next ring and one lap of that ring descending linearly
            from the upper level to its own, then one flat lap of the last ring.
  helix_holes : helix for the chains whose ring bbox is under the hole bound (passed), region for the rest.
Usage: wl_gcode.py <program.nc> <op comment> <out prefix> [hole bound mm]"""
import math, re, sys, json
W = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
path, op, pre = sys.argv[1], sys.argv[2], sys.argv[3]; HOLE = float(sys.argv[4]) if len(sys.argv) > 4 else 15.0
L = open(path).read().splitlines(); i0 = L.index(f"({op})")
head, body, tail = [], [], []
# the header of the op section up to the first G0, the moves, then the trailer (M5/M30...) after the last move
k = i0
while not L[k].startswith("G"): head.append(L[k]); k += 1
while k < len(L) and (L[k].startswith(("G0", "G1", "G2", "G3"))): body.append(L[k]); k += 1
tail = L[k:]
# moves with full state
mv, f, p = [], None, None
for ln in body:
    w = {a: float(b) for a, b in W.findall(ln)}; g = int(w["G"]); f = w.get("F", f)
    q = (w.get("X", p[0] if p else 0.0), w.get("Y", p[1] if p else 0.0), w.get("Z", p[2] if p else 0.0))
    mv.append({"g": g, "a": p, "b": q, "i": w.get("I"), "j": w.get("J"), "f": f}); p = q
SAFE = max(m["b"][2] for m in mv)
# passes: split before each G0 that rises to SAFE; the rising G0 and the XY G0 at SAFE are the link, dropped here and
# rebuilt by the writer
passes, cur = [], []
for m in mv:
    if m["g"] == 0 and m["b"][2] >= SAFE - 1e-9:
        if cur: passes.append(cur); cur = []
        continue
    cur.append(m)
if cur: passes.append(cur)
def arcpts(m, tol=0.01):
    (ax, ay, az), (bx, by, bz) = m["a"], m["b"]; cx, cy = ax + m["i"], ay + m["j"]; r = math.hypot(ax - cx, ay - cy)
    t0, t1 = math.atan2(ay - cy, ax - cx), math.atan2(by - cy, bx - cx)
    sw = ((t0 - t1) if m["g"] == 2 else (t1 - t0)) % (2 * math.pi) or 2 * math.pi
    n = max(1, int(math.ceil(sw / (2 * math.acos(max(-1, 1 - tol / r))))) if r > tol else 1)
    s = -1 if m["g"] == 2 else 1
    return [(cx + r * math.cos(t0 + s * sw * k / n), cy + r * math.sin(t0 + s * sw * k / n), az + (bz - az) * k / n) for k in range(1, n + 1)]
def split(P):
    zr = min(m["b"][2] for m in P)
    k = next(n for n, m in enumerate(P) if m["g"] != 0 and abs(m["b"][2] - zr) < 1e-6 and abs(m["a"][2] - zr) < 1e-6) if any(
        m["g"] != 0 and abs(m["b"][2] - zr) < 1e-6 and abs(m["a"][2] - zr) < 1e-6 for m in P) else len(P)
    ring = P[k:]; pts = [ring[0]["a"]] if ring else []
    for m in ring: pts += arcpts(m) if m["g"] in (2, 3) else [m["b"]]
    return {"z": zr, "pre": P[:k], "ring": ring, "pts": pts, "closed": bool(ring) and math.dist(pts[0][:2], pts[-1][:2]) < 1e-3}
PS = [split(P) for P in passes]
for q in PS:
    xs = [t[0] for t in q["pts"]] or [0]; ys = [t[1] for t in q["pts"]] or [0]
    q["bb"] = (min(xs), min(ys), max(xs), max(ys)); q["c"] = (sum(xs) / len(xs), sum(ys) / len(ys))
def pip(pt, poly):
    x, y = pt; ins = False
    for (x1, y1, _), (x2, y2, _) in zip(poly, poly[1:] + poly[:1]):
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1: ins = not ins
    return ins
levels = sorted({round(q["z"], 4) for q in PS}, reverse=True)
by = {z: [n for n, q in enumerate(PS) if round(q["z"], 4) == z] for z in levels}
SAMPLING = 0.5  # the op's own `sampling` (the play project: 0.5): the weave places a ring to within it
def samp(n, k=48):
    P = PS[n]["pts"]; return P[::max(1, len(P) // k)]
def near(pt, n):
    return min(math.dist(pt[:2], q[:2]) for q in samp(n, 400)) <= SAMPLING
def inside(a, b):
    """ring a lies inside ring b (or on it, to within the op's sampling)"""
    return all(pip(p[:2], PS[b]["pts"]) or near(p, b) for p in samp(a))
def area(n):
    P = PS[n]["pts"]; return abs(sum(x1 * y2 - x2 * y1 for (x1, y1, _), (x2, y2, _) in zip(P, P[1:] + P[:1]))) / 2
def match(u, v):
    """one encloses the other and no third ring of the two levels lies between them"""
    if not (PS[u]["closed"] and PS[v]["closed"]): return False
    o, i = (u, v) if area(u) >= area(v) else (v, u)
    if not inside(i, o): return False
    bb = lambda n: PS[n]["bb"]
    for w in by[round(PS[u]["z"], 4)] + by[round(PS[v]["z"], 4)]:
        if w in (u, v) or not PS[w]["closed"]: continue
        a, b = bb(w), bb(o)
        if a[0] < b[0] - 1 or a[1] < b[1] - 1 or a[2] > b[2] + 1 or a[3] > b[3] + 1: continue
        if inside(w, o) and inside(i, w) and not (inside(o, w)): return False
    return True
nxt = {}
for a, b in zip(levels, levels[1:]):
    cand = {u: [v for v in by[b] if match(u, v)] for u in by[a]}
    back = {}
    for u, vs in cand.items():
        for v in vs: back.setdefault(v, []).append(u)
    for u, vs in cand.items():
        if len(vs) == 1 and back[vs[0]] == [u]: nxt[u] = vs[0]
heads = [n for n in range(len(PS)) if n not in set(nxt.values())]
chains = []
for h in heads:
    c = [h]
    while c[-1] in nxt: c.append(nxt[c[-1]])
    chains.append(c)
def haus(A, B):
    d = lambda P, Q: max(min(math.dist(p[:2], q[:2]) for q in Q) for p in P)
    return max(d(A, B), d(B, A))
stat = {"passes": len(PS), "chains": len(chains), "closed": sum(q["closed"] for q in PS),
        "chain_len_hist": {}, "max_haus_holes": 0.0, "max_haus_walls": 0.0}
for c in chains:
    stat["chain_len_hist"][len(c)] = stat["chain_len_hist"].get(len(c), 0) + 1
    hole = max(PS[c[0]]["bb"][2] - PS[c[0]]["bb"][0], PS[c[0]]["bb"][3] - PS[c[0]]["bb"][1]) < HOLE
    for u, v in zip(c, c[1:]):
        P = PS[u]["pts"][::max(1, len(PS[u]["pts"]) // 400)]; Q = PS[v]["pts"][::max(1, len(PS[v]["pts"]) // 400)]
        h = haus(P, Q); key = "max_haus_holes" if hole else "max_haus_walls"; stat[key] = max(stat[key], round(h, 3))
# writer
def fmt(g, b, f=None, i=None, j=None):
    s = f"G{g} X{b[0]:.3f} Y{b[1]:.3f} Z{b[2]:.3f}"
    if i is not None: s += f" I{i:.3f} J{j:.3f}"
    if g != 0 and f: s += f" F{f:.0f}"
    return s
def emit_pass(out, q, cur):
    first = (q["pre"] or q["ring"])[0]["a"] if (q["pre"] or q["ring"]) else None
    out.append(f"G0 X{cur[0]:.3f} Y{cur[1]:.3f} Z{SAFE:.3f}"); out.append(fmt(0, (first[0], first[1], SAFE)))
    for m in q["pre"] + q["ring"]: out.append(fmt(m["g"], m["b"], m["f"], m["i"], m["j"]))
    return (q["pre"] + q["ring"])[-1]["b"]
def lap(out, pts, z0, z1, f, start):
    # one lap of the closed ring `pts` (pts[0] == pts[-1]) from vertex `start`, Z linear in arc length from z0 to z1
    ring = pts[:-1]; n = len(ring); seq = [ring[(start + k) % n] for k in range(n + 1)]
    tot = sum(math.dist(a[:2], b[:2]) for a, b in zip(seq, seq[1:])) or 1.0; s = 0.0
    for a, b in zip(seq, seq[1:]):
        s += math.dist(a[:2], b[:2]); out.append(fmt(1, (b[0], b[1], z0 + (z1 - z0) * s / tot), f))
    return seq[-1][:2] + (z1,)
def emit_helix(out, c, cur):
    q0 = PS[c[0]]; end = emit_pass(out, q0, cur); feed = q0["ring"][-1]["f"]; links = []
    for u, v in zip(c, c[1:]):
        pts = PS[v]["pts"]; st = min(range(len(pts) - 1), key=lambda k: math.dist(pts[k][:2], end[:2]))
        links.append(math.dist(pts[st][:2], end[:2]))
        out.append(fmt(1, (pts[st][0], pts[st][1], end[2]), feed)); end = lap(out, pts, PS[u]["z"], PS[v]["z"], feed, st)
    end = lap(out, PS[c[-1]]["pts"], PS[c[-1]]["z"], PS[c[-1]]["z"], feed, min(range(len(PS[c[-1]]["pts"]) - 1), key=lambda k: math.dist(PS[c[-1]]["pts"][k][:2], end[:2])))
    return end, links
def order_chains(cs):
    left = list(cs); pos = (0.0, 0.0); out = []
    while left:
        c = min(left, key=lambda c: math.dist(pos, PS[c[0]]["c"])); left.remove(c); out.append(c); pos = PS[c[-1]]["c"]
    return out
def write(name, mode):
    out = list(head); cur = mv[0]["a"] or (0, 0, SAFE); cur = (mv[0]["b"][0], mv[0]["b"][1], SAFE); maxlink = 0.0; entries = 0
    if mode == "base":
        for q in PS: cur = emit_pass(out, q, cur); entries += 1
    else:
        for c in order_chains(chains):
            hole = max(PS[c[0]]["bb"][2] - PS[c[0]]["bb"][0], PS[c[0]]["bb"][3] - PS[c[0]]["bb"][1]) < HOLE
            if (mode == "helix" or (mode == "helix_holes" and hole)) and len(c) > 1 and all(PS[n]["closed"] for n in c):
                cur, links = emit_helix(out, c, cur); entries += 1; maxlink = max([maxlink] + links)
            else:
                for n in c: cur = emit_pass(out, PS[n], cur); entries += 1
    out.append(f"G0 X{cur[0]:.3f} Y{cur[1]:.3f} Z{SAFE:.3f}"); out += tail
    open(f"{pre}_{name}.nc", "w").write("\n".join(out) + "\n")
    return {"entries": entries, "max_link_mm": round(maxlink, 3)}
for name in ("base", "region", "helix", "helix_holes"): stat[name] = write(name, name)
print(json.dumps(stat))
json.dump(stat, open(f"{pre}_stat.json", "w"), indent=1)
