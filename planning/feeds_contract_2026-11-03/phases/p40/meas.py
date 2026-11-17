"""P40: the measures of one run of setup "1 Back" (run from the p40 scratch dir).
usage: meas.py <tag>   reads out_<tag>.nc, out_<tag>.npz, out_<tag>/summary.json
"""
import json, sys
from scipy import ndimage
from gapmap import *
tag = sys.argv[1]
T = target(); ok = np.isfinite(T)
# the target lowered over one node on each side (P38 section 5.2)
Tf = np.where(ok, T, np.inf)
T1 = ndimage.minimum_filter(Tf, size=3, mode="nearest")
d = np.load(f"out_{tag}.npz"); names = list(d["names"])
for i, n in enumerate(names[:4]):
    S_i = d[f"s{i}"].astype(float)
    raw = ((S_i - T < -BAR) & ok)
    low = ((S_i - T1 < -BAR) & ok)
    deep = float((T1 - S_i)[low].max()) if low.any() else 0.0
    print(f"below target after '{n[:40]}': raw {raw.sum()*CELL*CELL:.1f} mm2; lowered one node {low.sum()*CELL*CELL:.1f} mm2 ({low.sum()} nodes, to {deep:.2f} mm)")
for op, (L, t, R) in op_times(f"out_{tag}.nc").items():
    if op.startswith(("Back rest", "Back bulk", "Cavity rest")):
        print(f"time of '{op[:40]}': {t:.0f} s, fed {L/1000:.2f} m, rapids {R:.0f} mm")
