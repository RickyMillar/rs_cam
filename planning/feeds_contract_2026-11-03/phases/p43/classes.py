"""P43: for each deep replay piece of one op, the clearance of its worst node from the fed
path of each level (distance to the nearest fed piece of the level, less the tool radius; only the pieces that run under the stock top of the node count).
A clearance of about one planner cell is a strip that the level left; a clearance of microns
is a graze (a node just inside the disc edge).
usage: classes.py <nc> <op prefix> <deep.npz> <bar> <level z ...>"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref, npz, bar = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4])
levels = [float(v) for v in sys.argv[5:]]
segs = {k: [] for k in range(len(levels))}
cur = 0; R = None
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref): continue
    R = TOOLS[tool][1]
    z = min(p0[2], p1[2])
    while cur + 1 < len(levels) and z < levels[cur] - 1e-3: cur += 1  # no level cuts under its own Z
    segs[cur].append((p0[0], p0[1], p1[0], p1[1], p0[2], p1[2]))
S = {k: np.array(v) for k, v in segs.items()}
def clear(k, n, top):
    # only the pieces that run under the stock top of the node: a piece above it cuts nothing there
    a = S[k]; a = a[np.maximum(a[:, 4], a[:, 5]) < top - 0.01]
    if not len(a): return float("nan")
    d = a[:, 2:4] - a[:, 0:2]; L2 = (d ** 2).sum(1)
    t = np.clip(((n[0] - a[:, 0]) * d[:, 0] + (n[1] - a[:, 1]) * d[:, 1]) / np.maximum(L2, 1e-18), 0, 1)
    q = a[:, 0:2] + t[:, None] * d
    return float(np.hypot(n[0] - q[:, 0], n[1] - q[:, 1]).min()) - R
a = np.load(npz)["pieces"]; deep = a[a[:, 7] > bar]
print(f"levels {levels}; pieces per level {[len(S[k]) for k in S]}; deep pieces {len(deep)}")
graze = strip = 0
for r in deep:
    n = (r[9], r[10]); c = [clear(k, n, r[11]) for k in range(len(levels))]
    kind = "graze" if r[8] <= 5 else "strip"
    graze += kind == "graze"; strip += kind == "strip"
    print(f" piece {int(r[0])} z {r[3]:.2f} depth {r[7]:.2f} nodes {int(r[8])} node ({n[0]:.1f},{n[1]:.1f}) clearance per level " + " ".join(f"{v:+.3f}" for v in c) + f"  {kind}")
print(f"strips (more than 5 nodes) {strip}; grazes (1 to 5 nodes) {graze}")
