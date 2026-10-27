"""D35: the op_moves / length functions of fix25/c39_d34.py, without its run loop."""

import collections, glob, json, math, os, re, sys
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")  # D34: a copy of fix23/c39_d33.py; the runs and a toolpath id per run differ
W = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
RUNS = [(f"{fx} {c} tp{tp}", f"fix25/{c}/{fx}", f"fix25/{c}/{fx}", op, rapid, tp)
        for fx, rapid, ops in (("corne", 4000.0, ((1, "3D Rough 1"), (5, "3D Rough 1 [edges, CP rest]"), (3, "3D Rough 1 [copy]"))),
                               ("wanaka200", 10000.0, ((1, "2 Back Rough"), (7, "2 Back Rough [edges, CP rest]"))))
        for c in (("body_cp", "body_cp_field", "cp_alone") if fx == "corne" else ("body_cp", "body_cp_field", "cp_alone_0.6"))
        for tp, op in ops if not (c.startswith("cp_alone") and tp in (5, 7))]
RUNS += [("corne fix6 CP tp1", "fix6/corne/contour_parallel", "fix6/corne/contour_parallel", "3D Rough 1", 4000.0, 1),
         ("corne fix6 CP tp3", "fix6/corne/contour_parallel", "fix6/corne/contour_parallel", "3D Rough 1 [copy]", 4000.0, 3)]


def op_moves(path, op):
    L = open(path).read().splitlines(); i = L.index(f"({op})") + 1; cmds = []
    for ln in L[i:]:
        t = ln.strip()
        if t.startswith("("):
            if cmds and not t.startswith("(feeds"): break
            continue
        if t and t[0] in "MTS":
            if cmds: break
            continue
        if t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ"): cmds.append(t)
    g, p, f, out = 0, (0.0, 0.0, 0.0), None, []
    for c in cmds:
        w = {k: float(v) for k, v in W.findall(c)}
        g = int(w.get("G", g)); f = w.get("F", f); q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        out.append((g, p, q, w.get("I", 0.0), w.get("J", 0.0), f)); p = q
    return out


def length(m):
    g, a, b, i, j, f = m
    if g in (2, 3):
        cx, cy = a[0] + i, a[1] + j; r = math.hypot(a[0] - cx, a[1] - cy)
        t0, t1 = math.atan2(a[1] - cy, a[0] - cx), math.atan2(b[1] - cy, b[0] - cx)
        sw = ((t0 - t1) if g == 2 else (t1 - t0)) % (2 * math.pi) or 2 * math.pi
        return math.hypot(r * sw, b[2] - a[2])
    return math.dist(a, b)

