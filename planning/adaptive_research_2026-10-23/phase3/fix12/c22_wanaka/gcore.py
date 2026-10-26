"""C22 §12.8: the core spiral per level from the G-code (vertices), C35 against D20."""
import os, re, math
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
G = {"D20": "fix7/wanaka200/geodesic_local_zigzag_pick/program.nc", "C35": "fix12/wanaka200/program.nc"}
SEED = (119.800, 124.874)
LEVELS = [-5.46, -10.92, -16.38]   # G-code Z of model Z 19.54, 14.08, 8.62 (top 25)
def moves(path):
    x = y = z = None; out = []
    for line in open(path):
        m = re.match(r"^(G[0-3])\b(.*)", line.strip())
        if not m: continue
        g, rest = m.groups(); d = dict(re.findall(r"([XYZIJ])(-?[\d.]+)", rest))
        nx, ny, nz = float(d.get("X", x if x is not None else 0)), float(d.get("Y", y if y is not None else 0)), float(d.get("Z", z if z is not None else 0))
        out.append((g, (x, y, z), (nx, ny, nz))); x, y, z = nx, ny, nz
    return out
def r(p): return math.hypot(p[0] - SEED[0], p[1] - SEED[1])
def seed_runs(M):
    """Index of each plunge to a level Z at the seed."""
    idx = []
    for k, (g, a, b) in enumerate(M):
        if g == "G1" and a[0] is not None and abs(b[0]-SEED[0]) < 1e-3 and abs(b[1]-SEED[1]) < 1e-3 and any(abs(b[2]-L) < 1e-3 for L in LEVELS) and b[2] < a[2]:
            idx.append(k)
    return idx
def first_radial_step(M, k0):
    """From the seed plunge: the first cutting move that steps out radially by more than 1 mm
    (|dr| > 1 and the move mostly radial), or a G0. Returns (r before, r after, kind, moves)."""
    rmax = 0.0
    for k in range(k0 + 1, len(M)):
        g, a, b = M[k]
        if g == "G0":
            return rmax, r(b), "G0", k - k0
        dr = r(b) - r(a); L = math.hypot(b[0]-a[0], b[1]-a[1])
        if g == "G1" and dr > 1.0 and dr > 0.7 * L:
            return r(a), r(b), "radial step", k - k0
        rmax = max(rmax, r(b))
    return rmax, None, "end", None
if __name__ == "__main__":
    for col, p in G.items():
        M = moves(os.path.join(P3, p))
        for k0 in seed_runs(M):
            ra, rb, kind, n = first_radial_step(M, k0)
            print(f"{col} seed plunge to Z {M[k0][2][2]:.2f}: first {kind} after {n} moves, r {ra:.2f} -> {rb:.2f}")
