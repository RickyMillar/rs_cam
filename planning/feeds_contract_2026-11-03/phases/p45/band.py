"""P45: the band at the bottom edge of the cavity (operator remark on the P44b render).

Window (G-code frame, the frame of the render): X 45..350, Y 105..120, between the cavity
wall and the slit slot. For one run of setup "1 Back" it prints:
(1) the wood where the 6 mm tool fits after "Back rest 6 mm" in the window (mm2, more than
    0.3 mm over the reach surface `places.reach6h`) and its tallest height;
(2) how "Back rest 6 mm" cuts in the window: the count of separate fed runs, of entries, and
    the longest run. A fed run is a chain of fed moves (G1, G2, G3) between two rapids (G0),
    cut to the part inside the window (a chain that leaves the window and comes back counts
    two). An entry is a run that starts inside the window after a rapid. Lengths are XY
    lengths of moves that travel in XY (a plunge has none). A run under 1 mm is not counted
    (a plunge or a touch at the window edge).
    With a baseline G-code (a run with no strip pass, P44b) the same counts are given for
    the moves of the op after the common prefix with the baseline: the strip pass alone.
usage: band.py <out.npz> <out.nc> [baseline.nc]
"""
import sys, os, re, math
H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(H, "..", "p44")); sys.path.insert(0, os.path.join(H, "..", "p37"))
from places import *

WIN = (45.0, 350.0, 105.0, 120.0)
OP = "Back rest"


def op_lines(nc):
    """The motion lines of the op (text), in order."""
    out = []; on = False
    for line in open(nc, errors="replace"):
        line = line.strip()
        if line.startswith("("):
            t = line[1:-1]
            if not t.startswith(SKIP):
                on = t.startswith(OP)
            continue
        if on and line:
            out.append(line)
    return out


def parse(lines, pos0=(0.0, 0.0, 36.0)):
    """(mode, p0, p1) of each move of `lines`; an arc is one move with its chord (the window test reads the ends)."""
    word = re.compile(r"([GXYZIJFTM])(-?\d+\.?\d*)")
    pos = list(pos0); mode = None; out = []
    for line in lines:
        d = {}
        for k, val in word.findall(line):
            if k == "G":
                g = int(float(val))
                if g in (0, 1, 2, 3): mode = g
            elif k in "XYZ":
                d[k] = float(val)
        if not d: continue
        new = [d.get("X", pos[0]), d.get("Y", pos[1]), d.get("Z", pos[2])]
        out.append((mode, tuple(pos), tuple(new))); pos = new
    return out


def inside(p):
    return WIN[0] <= p[0] <= WIN[1] and WIN[2] <= p[1] <= WIN[3]


def runs(mv):
    """The fed runs in the window: (length, started after a rapid inside the window, lowest Z, highest Z)."""
    out = []; cur = None; after_rapid = True
    def close():
        nonlocal cur
        if cur is not None and cur[0] >= 1.0: out.append(tuple(cur))
        cur = None
    for mode, a, b in mv:
        if mode == 0 or mode is None:
            close(); after_rapid = True; continue
        ok = inside(a) and inside(b)
        if not ok:
            close(); after_rapid = False; continue
        L = math.hypot(b[0] - a[0], b[1] - a[1])
        if cur is None:
            cur = [0.0, after_rapid, min(a[2], b[2]), max(a[2], b[2])]
        cur[0] += L; cur[2] = min(cur[2], a[2], b[2]); cur[3] = max(cur[3], b[2] if L > 0 else cur[3])
        after_rapid = False
    close()
    return out


def report(tag, mv):
    r = runs(mv)
    if not r:
        print(f"   {tag}: no fed run in the window"); return
    L = sorted((x[0] for x in r), reverse=True)
    print(f"   {tag}: {len(r)} fed runs, {sum(1 for x in r if x[1])} entries, total {sum(L):.0f} mm, longest {L[0]:.0f} mm, "
          f"median {L[len(L) // 2]:.1f} mm, runs under 15 mm {sum(1 for v in L if v < 15)}, runs of 100 mm or more {sum(1 for v in L if v >= 100)}; "
          f"the 5 longest {[round(v) for v in L[:5]]}")


d = np.load(sys.argv[1]); names = [str(n) for n in d["names"]]
k = [i for i, n in enumerate(names) if n.startswith(OP)][0]
S = d[f"s{k}"].astype(float); T = target(); ok = np.isfinite(T); R6 = reach6h(T)
J, I = np.mgrid[0:NY, 0:NX]; X, Y = I * CELL, J * CELL
m = (X >= WIN[0]) & (X <= WIN[1]) & (Y >= WIN[2]) & (Y <= WIN[3])
g = np.where(ok & m, S - R6, 0.0); fits = g > BAR
print(f"band X {WIN[0]}..{WIN[1]} Y {WIN[2]}..{WIN[3]} ({os.path.basename(sys.argv[1])}):")
if fits.any():
    j, i = np.unravel_index(np.argmax(g), g.shape)
    hh = g[fits]
    print(f"   wood where the 6 mm tool fits after '{OP}': {fits.sum() * CELL * CELL:.1f} mm2; tallest {g[j, i]:.2f} mm at ({i * CELL:.1f}, {j * CELL:.1f}); "
          f"by height: 0.3-1 mm {((hh <= 1)).sum() * CELL * CELL:.1f}, 1-3 mm {((hh > 1) & (hh <= 3)).sum() * CELL * CELL:.1f}, 3-6.5 mm {((hh > 3) & (hh <= 6.5)).sum() * CELL * CELL:.1f}, over 6.5 mm {(hh > 6.5).sum() * CELL * CELL:.1f} mm2")
else:
    print("   wood where the 6 mm tool fits: 0.0 mm2")
lines = op_lines(sys.argv[2]); mv = parse(lines)
report("all moves of the op", mv)
if len(sys.argv) > 3:
    base = op_lines(sys.argv[3]); n = 0
    while n < min(len(base), len(lines)) and base[n] == lines[n]: n += 1
    print(f"   the op has {len(lines)} motion lines; the first {n} are the lines of the baseline ({len(base)} lines)")
    pre = parse(lines[:n]); pos = pre[-1][2] if pre else (0.0, 0.0, 36.0)
    report("the moves after the common prefix (the strip pass)", parse(lines[n:], pos))
