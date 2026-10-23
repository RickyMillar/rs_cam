# Diagnostic preview (not on the page): the (e)/(f) loops of whole.py.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
d = np.load(os.path.join(os.path.dirname(os.path.abspath(__file__)), "curves_whole.npz"))
fig, axs = plt.subplots(2, 2, figsize=(16, 14))
for ax, (nm, k) in zip(axs.flat, [("pocket", "e"), ("pocket", "f"), ("star", "e"), ("star", "f")]):
    for key in d.files:
        a, b, i, j = key.split("|")
        if a == nm and b == k:
            P = d[key]
            ax.plot(P[:, 0], P[:, 1], lw=0.5, color="C%d" % (int(i) % 10))
    ax.set_aspect("equal"); ax.set_title(nm + " " + k)
plt.tight_layout(); plt.savefig(sys.argv[1], dpi=60)
