"""P44b: split the wood that fits in a place into the rim at the wall and the rest.
usage: rim_split.py <out.npz> <place index 0..6> [rim mm, default 0.4]
rim = fits nodes at most <rim> mm from a step of more than 1 mm in the reach surface (a wall)."""
import sys
sys.path.insert(0, "p44"); sys.path.insert(0, "p37")
from places import *
from scipy import ndimage as nd
T = target(); ok = np.isfinite(T); R6 = reach6h(T)
d = np.load(sys.argv[1]); names = [str(n) for n in d["names"]]
k2 = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
S2 = d[f"s{k2}"].astype(float)
name, m = masks()[int(sys.argv[2])]
rim_mm = float(sys.argv[3]) if len(sys.argv) > 3 else 0.4
fits = (np.where(ok, S2 - R6, 0) > BAR) & m
# a node is at a wall when the reach surface steps up by more than 1 mm within <rim> mm of it
k = int(round(rim_mm / CELL))
Rf = np.where(ok, R6, np.nanmax(R6[ok]))
rim = fits & (nd.maximum_filter(Rf, size=2 * k + 1) - Rf > 1.0)
print("%s: fits %.1f mm2 = rim (at most %.1f mm from the wall) %.1f + the rest %.1f; at the stock top %.1f" % (name, fits.sum()*.04, rim_mm, rim.sum()*.04, (fits & ~rim).sum()*.04, (fits & (S2 > STOCK_TOP - 1e-3)).sum()*.04))
