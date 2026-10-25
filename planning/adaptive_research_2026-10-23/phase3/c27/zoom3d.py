import sys, matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
from perlevel import fx, per_level
lev = sys.argv[1]; box = list(map(float, sys.argv[2:6])); out = sys.argv[6]
fig, axs = plt.subplots(1, 2, figsize=(16, 8 * (box[3]-box[2]) / (box[1]-box[0]) + 0.5))
for ax, arm in zip(axs, ("geodesic_local_zigzag", "geodesic_local_zigzag_v2")):
    d, _ = per_level(arm)
    for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if l != lev or fx.xy(a, b) < 1e-6: continue
        col = {"cut": "#1f5fbf", "link": "#e07000", "rapid": "#888888"}[c]
        ax.plot([a[0], b[0]], [a[1], b[1]], '-', color=col, lw=0.6 if c != "rapid" else 0.4)
    for i, p in d["rings"]:
        if d["lev"][i] == lev: ax.plot(p[0], p[1], 'o', mfc='none', mec='r', ms=8)
    ax.set_xlim(box[0], box[1]); ax.set_ylim(box[2], box[3]); ax.set_aspect('equal'); ax.set_title(f'{arm} {lev}')
fig.savefig(out, dpi=90, bbox_inches='tight')
