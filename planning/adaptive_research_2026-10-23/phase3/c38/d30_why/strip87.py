"""C38 §15.6, step 1: the D30 op 1 outer-strip samples over the oracle
bound 0.369 at program F >= 3999 (fix21/feed_check.py's rule), each with
its move: length, Z, program feed, commanded (sample) feed, the program
feeds of the moves before and after, and the samples of the whole move.
Writes strip87.json. Read only; loads fix21's simulation.json."""
import importlib.util, json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
d = P3 / "fix21" / "a_only" / "corne"
n = json.load(open(sorted(d.glob("tp_1_*.json"))[0]))["move_count"]
ops = A.op_moves(str(d / "program.nc"), "3D Rough 1")
pf = [(m["f"] or 0.0) for m in ops[1:n + 1]]
SIM = json.load(open(d / "simulation.json"))
S = [s for s in SIM["trace"]["samples"] if s["toolpath_id"] == 1]
del SIM
from scipy import ndimage
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(np.ceil(6.0 / g))
sil = np.pad(ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6)), pad)
X0, Y0 = x0 - pad * g, y0 - pad * g
dout = ndimage.distance_transform_edt(~sil) * g; din = ndimage.distance_transform_edt(sil) * g
def wall(pos):
    c = np.clip(np.round((pos[:, 0] - X0) / g).astype(int), 0, sil.shape[1] - 1)
    r = np.clip(np.round((pos[:, 1] - Y0) / g).astype(int), 0, sil.shape[0] - 1)
    return np.where(sil[r, c], -din[r, c], dout[r, c])
pos = np.array([s["position"] for s in S], float); w = wall(pos); z = pos[:, 2]
lv = np.zeros(len(z), bool)
for L in (11.0, 5.0, 0.5):
    lv |= np.abs(z - L) < 0.05
cut = np.array([s["is_cutting"] and s["source_intent"] == "clearing_cut" for s in S])
r = np.array([s["engagement"]["radial_woc_fraction"] for s in S])
mi = np.array([s["move_index"] for s in S])
pfs = np.array([pf[k] if k < n else 0.0 for k in mi]); sfs = np.array([s["feed_rate_mm_min"] for s in S])
strip = cut & (w > 3.0) & (w <= 6.2) & lv
bad = strip & (r > 0.369) & (pfs >= 3999)
by_move = {}
for i in np.nonzero(bad)[0]:
    by_move.setdefault(int(mi[i]), []).append(int(i))
first, last = {}, {}
for i, k in enumerate(mi):
    first.setdefault(int(k), i); last[int(k)] = i
rows = []
for k, idx in sorted(by_move.items()):
    a, b = pos[first[k]], pos[last[k]]
    rows.append({"move": k, "over_samples": len(idx),
                 "move_samples": int(last[k] - first[k] + 1),
                 "from": [round(v, 3) for v in a], "to": [round(v, 3) for v in b],
                 "len_xy": round(math.hypot(b[0] - a[0], b[1] - a[1]), 3),
                 "dz": round(float(b[2] - a[2]), 3),
                 "program_F": pf[k], "commanded_F": float(sfs[idx[0]]),
                 "prev_F": [pf[j] for j in range(max(0, k - 3), k)],
                 "next_F": [pf[j] for j in range(k + 1, min(n, k + 4))],
                 "radial_max": round(float(r[idx].max()), 3),
                 "wall": round(float(w[idx].mean()), 2), "z": round(float(z[idx[0]]), 2),
                 "intent": S[idx[0]]["source_intent"], "kin": S[idx[0]].get("cut_kinematics")})
out = {"strip_samples": int(strip.sum()), "bad_samples": int(bad.sum()), "bad_moves": len(rows), "moves": rows}
(H / "strip87.json").write_text(json.dumps(out, indent=1))
print(json.dumps({k: v for k, v in out.items() if k != "moves"}))
for row in rows:
    print(row["move"], row["over_samples"], row["len_xy"], row["dz"], row["z"], row["program_F"], row["commanded_F"], row["prev_F"], row["next_F"], row["radial_max"], row["wall"], row["kin"])
