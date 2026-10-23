import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stream as st, numpy as np
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
nm, mode, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
sh = st.Shape(nm); d = st.Direction(sh, st.harmonic_field(sh))
loops = st.march(sh, d, mode, max_loops=n)
fig, ax = plt.subplots(figsize=(10,8))
ax.contour(sh.xs, sh.ys, sh.omega.astype(float), [0.5], colors="k")
for i, lp in enumerate(loops):
    P = np.vstack([lp["P"], lp["P"][:1]])
    ax.plot(P[:,0], P[:,1], lw=0.5, color="r" if i == len(loops)-1 else "b")
    m = lp["stop"] & ~lp["was_stopped"]
    if i == len(loops)-1: ax.plot(lp["P"][~lp["was_stopped"],0], lp["P"][~lp["was_stopped"],1], "g.", ms=2)
ax.set_aspect("equal"); fig.savefig(sys.argv[4], dpi=110)
