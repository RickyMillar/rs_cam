"""P45: the wood that stays where the 6 mm tool fits after "Back rest 6 mm": the rim at the walls and the other pieces.
rim  = fits nodes at most <rim> mm (default 0.4) from a step of more than 1 mm in the reach surface (a wall): the
       margin of a contour line (D19: one planner cell plus the cut tolerance).
rest = the other fits nodes, as connected pieces (8-neighbour). Prints the sum per place and the largest pieces:
       box (G-code frame), area, tallest wood over the reach surface, the stock Z there, and the last op that cut it.
usage: stays.py <out.npz> [rim mm] [pieces to list]
"""
import sys, os
H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(H, "..", "p44")); sys.path.insert(0, os.path.join(H, "..", "p37"))
from places import *
from scipy import ndimage as nd
T = target(); ok = np.isfinite(T); R6 = reach6h(T)
d = np.load(sys.argv[1]); names = [str(n) for n in d["names"]]
k2 = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
S2 = d[f"s{k2}"].astype(float); S1 = d[f"s{k2-1}"].astype(float)
rim_mm = float(sys.argv[2]) if len(sys.argv) > 2 else 0.4
nlist = int(sys.argv[3]) if len(sys.argv) > 3 else 25
g = np.where(ok, S2 - R6, 0.0); fits = g > BAR
k = int(round(rim_mm / CELL)); Rf = np.where(ok, R6, np.nanmax(R6[ok]))
rim = fits & (nd.maximum_filter(Rf, size=2 * k + 1) - Rf > 1.0)
rest = fits & ~rim
A = CELL * CELL
print(f"{os.path.basename(sys.argv[1])}: fits {fits.sum()*A:.1f} mm2 = rim (at most {rim_mm} mm from a wall) {rim.sum()*A:.1f} + the rest {rest.sum()*A:.1f}")
print(f"{'place':28s} {'fits':>8s} {'rim':>8s} {'rest':>8s} {'rest never cut by id 2':>24s}")
ms = masks()
for name, m in ms:
    print(f"{name:28s} {(fits&m).sum()*A:8.1f} {(rim&m).sum()*A:8.1f} {(rest&m).sum()*A:8.1f} {(rest&m&(S2>S1-1e-4)).sum()*A:24.1f}")
lab, n = nd.label(rest, structure=np.ones((3, 3)))
idx = np.arange(1, n + 1)
area = nd.sum(rest, lab, idx) * A
order = np.argsort(-area)
print(f"the rest: {n} pieces; the {min(nlist, n)} largest:")
sl = nd.find_objects(lab)
for q in order[:nlist]:
    s = sl[q]; mm = lab[s] == q + 1
    gg = np.where(mm, g[s], -1); j, i = np.unravel_index(np.argmax(gg), gg.shape)
    J, I = s[0].start + j, s[1].start + i
    place = next((nm for nm, m in ms if m[J, I]), "?")
    cut2 = (mm & (S2[s] < S1[s] - 1e-4)).sum() * A
    print(f"   X {s[1].start*CELL:.1f}..{(s[1].stop-1)*CELL:.1f} Y {s[0].start*CELL:.1f}..{(s[0].stop-1)*CELL:.1f}: {area[q]:.1f} mm2, tallest {gg[j,i]:.2f} mm at ({I*CELL:.1f}, {J*CELL:.1f}) stock Z {S2[J,I]:.2f} reach Z {R6[J,I]:.2f}; id 2 cut {cut2:.1f} mm2 of it; {place}")
