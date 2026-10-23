# draw a stored march from " + (sys.argv[5] if len(sys.argv) > 5 else "curves_c17.npz") + ": moving parts blue, stopped parts of the last loop grey,
# over-c and crossing marks optional
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
import stream as st
nm, key, out = sys.argv[1], sys.argv[2], sys.argv[3]
box = [float(v) for v in sys.argv[4].split(",")] if len(sys.argv) > 4 else None
d = np.load("" + (sys.argv[5] if len(sys.argv) > 5 else "curves_c17.npz") + "")
n = len([k for k in d.files if k.startswith(f"{nm}|{key}|") and k.endswith("|P")])
sh = st.Shape(nm)
fig, ax = plt.subplots(figsize=(12, 9))
ax.contour(sh.xs, sh.ys, sh.omega.astype(float), [0.5], colors="k", linewidths=0.5)
for i in range(n):
    P = d[f"{nm}|{key}|{i}|P"].astype(float); was = d[f"{nm}|{key}|{i}|was"]; kd = d[f"{nm}|{key}|{i}|kind"]
    seg = ~(was & np.roll(was, -1))
    for j in np.nonzero(seg)[0]:
        q = P[[j, (j + 1) % len(P)]]
        ax.plot(q[:, 0], q[:, 1], color="b", lw=0.4)
    m = kd == 2
    ax.plot(P[m, 0], P[m, 1], ".", color="orange", ms=1.5)
if box: ax.set_xlim(box[0], box[1]); ax.set_ylim(box[2], box[3])
ax.set_aspect("equal"); fig.savefig(out, dpi=110)
