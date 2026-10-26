"""C38 §11.1 reconcile with D9's peak (P15 "D23 addendum"): are my copy-op
deep samples (axial over 4.5 mm, centre 3.2 mm outside the silhouette) the
samples of D9's peak move (headon 1421, pick 1420)? And which copy-op and
op-1 levels cut within R of that move before it."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
out = {}
for run, peak in (("headon", 1421), ("pick", 1420)):
    S = np.load(H / f"samples_{run}.npz"); it = list(S["intents"])
    pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
    deep = (S["tp"] == 3) & S["cut"] & (S["axial"] > 4.5)
    on_line = deep & (np.abs(w - 3.0) <= 0.5)
    mv_on = S["mv"][on_line]
    moves, counts = np.unique(mv_on, return_counts=True)
    pk = (S["tp"] == 3) & (S["mv"] == peak) & S["cut"]
    p = pos[pk]
    r = {
        "deep_on_line": int(on_line.sum()),
        "deep_on_line_moves": {int(a): int(b) for a, b in zip(moves, counts)},
        "deep_on_line_on_peak_move": int((on_line & (S["mv"] == peak)).sum()),
        "peak_move_samples": int(pk.sum()),
        "peak_move_x_range": [round(float(p[:, 0].min()), 2), round(float(p[:, 0].max()), 2)],
        "peak_move_y_range": [round(float(p[:, 1].min()), 2), round(float(p[:, 1].max()), 2)],
        "peak_move_wall_distance": [round(float(w[pk].min()), 2), round(float(w[pk].max()), 2)],
        "peak_move_stock_edge_x": round(153.07408142089844 - float(p[:, 0].max()), 2),
        "peak_move_axial_max": round(float(S["axial"][pk].max()), 2),
    }
    # which earlier cuts passed within R of the peak path, per op and level
    first = int(np.nonzero(pk)[0][0])
    before = np.arange(len(z)) < first
    near = lambda R_: np.min(np.hypot(*(pos[:, None, :2] - p[None, ::4, :2]).transpose(2, 0, 1)), axis=1) <= R_
    cand = before & S["cut"] & (S["intent"] == it.index("clearing_cut"))
    idx = np.nonzero(cand)[0]
    d = np.min(np.hypot(pos[idx, None, 0] - p[None, :, 0], pos[idx, None, 1] - p[None, :, 1]), axis=1)
    levels = {}
    for tp, R_ in ((1, 3.0), (3, 1.5)):
        sel = idx[(S["tp"][idx] == tp) & (d[(S["tp"][idx] == tp).nonzero()[0]] <= R_)] if False else idx[(S["tp"][idx] == tp)][d[S["tp"][idx] == tp] <= R_]
        levels[f"tp{tp}_cut_levels_within_R"] = sorted({round(float(v), 2) for v in z[sel]})
    r.update(levels)
    out[run] = r
print(json.dumps(out, indent=1))
(H / "peak_move.json").write_text(json.dumps(out, indent=1))
