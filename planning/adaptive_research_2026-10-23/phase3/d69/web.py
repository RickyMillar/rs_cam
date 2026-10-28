"""D69: does the job cut the part free? Per op (cumulative stock of stockmap.py): the bottleneck Z of the web, the
lowest stock top on the best path of standing stock (top > h) from the part rim to the stock border (binary search on h,
4-connected cells outside the model silhouette), and the stock volume left outside the silhouette above the stock bottom
+ the job's lowest floor. Usage: web.py <model npz> <stock npz> <tps>"""
import sys
import numpy as np
from scipy import ndimage
M = np.load(sys.argv[1]); S = np.load(sys.argv[2]); tps = [int(t) for t in sys.argv[3].split(",")]; g = float(S["g"])
part = ~np.isnan(M["ztop"]); out = ~part; edge = ndimage.binary_dilation(part) & out
holes = M["holes"] > 0; outside = out & ~holes
for tp in tps:
    t = S[f"top_after_{tp}"]
    def connected(h):
        lab, _ = ndimage.label((t > h) & out)
        b = set(np.unique(np.r_[lab[0], lab[-1], lab[:, 0], lab[:, -1]])) - {0}; e = set(np.unique(lab[edge])) - {0}
        return bool(b & e)
    if not connected(0.0): print(tp, "part free"); continue
    lo, hi = 0.0, float(t.max())
    for _ in range(30):
        mid = (lo + hi) / 2; lo, hi = (mid, hi) if connected(mid) else (lo, mid)
    print(tp, "web bottleneck Z", round(lo, 3), "| stock outside the silhouette (no holes) above 0.5:", round(float(np.clip(t[outside] - 0.5, 0, None).sum() * g * g)), "mm3")
