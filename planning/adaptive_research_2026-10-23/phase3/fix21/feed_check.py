"""D30: the feed check on the program itself (C12 §15.5 items 1, 4, 5).
- The program feed of op move k is the F of program.nc's op section line k + 1
  (fix11/audit.py op_moves; the map the audit uses).
- A sample's feed_rate_mm_min is the planner's commanded feed of its move,
  before the feed modulation (C39); for a capped move that is already the cap
  (F x factor), since the planner writes the lowered feed and the cap together.
- Capped turn moves: each "C38 capped turn from=(x,y) to=(x,y)" line of op 1 is
  matched to the op-1 move whose first and last sample lie within one sample
  step + 0.05 mm of from and to in XY (the line and the samples share the model/setup frame).
  A sample on such a move whose PROGRAM feed exceeds its sample (capped) feed
  by more than 0.5 mm/min breaks the cap.
- Outer-strip samples (Corne only): op 1 clearing samples at Z 11 / 5 / 0.5
  between 3.0 and 6.2 mm outside the silhouette (C12's strip_feed.py rule),
  over the oracle bound 0.369 and over c = 0.219, at PROGRAM feed >= 3999.
Usage: feed_check.py <run dir> <op name> <caps run dir> [corne]
The capped-turn lines come from <caps run dir> (D30), so a D29 run is read
with D30's turn geometry. The line frame is tried as given and as the
wanaka200 setup flip (x + 20, 225 - y; fix9/stockmap.py); the frame with
more matches is used and printed."""
import collections, importlib.util, json, math, re, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
run, op, capsrun = sys.argv[1], sys.argv[2], sys.argv[3]; corne = len(sys.argv) > 4
d = P3 / run
n = json.load(open(sorted(d.glob("tp_1_*.json"))[0]))["move_count"]
pf = [(m["f"] or 0.0) for m in A.op_moves(str(d / "program.nc"), op)[1:n + 1]]
SIM = json.load(open(d / "simulation.json")); TOL = SIM["sample_step_mm"] + 0.05
S = [s for s in SIM["trace"]["samples"] if s["toolpath_id"] == 1]
first, last = {}, {}
for s in S:
    first.setdefault(s["move_index"], s); last[s["move_index"]] = s
caps = []
for ln in open(P3 / capsrun / "stdout_plain.txt"):
    if "index=0" in ln and "C38 capped turn" in ln:
        m = re.search(r"from=\(([-\d.]+), ([-\d.]+)\) to=\(([-\d.]+), ([-\d.]+)\) level_z=Some\(([-\d.]+)\) factor=([\d.]+)", ln)
        caps.append(tuple(float(v) for v in m.groups()))
def match(tf):
    mv = {}
    for (fx, fy, tx, ty, lz, fac) in caps:
        (fx, fy), (tx, ty) = tf(fx, fy), tf(tx, ty)
        for k in first:
            if math.hypot(first[k]["position"][0] - fx, first[k]["position"][1] - fy) < TOL \
               and math.hypot(last[k]["position"][0] - tx, last[k]["position"][1] - ty) < TOL:
                mv[k] = fac
    return mv
frames = {"as given": lambda x, y: (x, y), "wanaka flip": lambda x, y: (x + 20.0, 225.0 - y)}
tried = {name: match(tf) for name, tf in frames.items()}
frame = max(tried, key=lambda k: len(tried[k])); moves = tried[frame]
over = [s for s in S if s["move_index"] in moves and s["is_cutting"] and pf[s["move_index"]] > s["feed_rate_mm_min"] + 0.5]
out = {"frame": frame, "capped_turn_lines": len(caps), "capped_moves_matched": len(moves),
       "samples_on_capped_moves": sum(1 for s in S if s["move_index"] in moves and s["is_cutting"]),
       "samples_above_cap_at_program_feed": len(over),
       "program_feed_on_capped_moves": dict(collections.Counter(round(pf[k]) for k in moves).most_common(8))}
if corne:
    from scipy import ndimage
    part = np.load(P3 / "fix9" / "corne_part.npz")
    top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
    pad = int(np.ceil(6.0 / g))   # c38/d22_why/why.py wall(): the padded hole-filled silhouette
    sil = np.pad(ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6)), pad)
    X0, Y0 = x0 - pad * g, y0 - pad * g
    dout = ndimage.distance_transform_edt(~sil) * g; din = ndimage.distance_transform_edt(sil) * g
    def wall(pos):
        c = np.clip(np.round((pos[:, 0] - X0) / g).astype(int), 0, sil.shape[1] - 1)
        r = np.clip(np.round((pos[:, 1] - Y0) / g).astype(int), 0, sil.shape[0] - 1)
        return np.where(sil[r, c], -din[r, c], dout[r, c])
    pos = np.array([s["position"] for s in S], float); w = wall(pos)
    z = pos[:, 2]; lv = np.zeros(len(z), bool)
    for L in (11.0, 5.0, 0.5):
        lv |= np.abs(z - L) < 0.05
    cut = np.array([s["is_cutting"] and s["source_intent"] == "clearing_cut" for s in S])
    m = cut & (w > 3.0) & (w <= 6.2) & lv
    r = np.array([s["engagement"]["radial_woc_fraction"] for s in S])
    pfs = np.array([pf[s["move_index"]] if s["move_index"] < n else 0.0 for s in S]); sfs = np.array([s["feed_rate_mm_min"] for s in S])
    out.update({"strip_samples": int(m.sum()),
                "strip_over_0.369_program_F>=3999": int((m & (r > 0.369) & (pfs >= 3999)).sum()),
                "strip_over_0.219_program_F>=3999": int((m & (r > 0.2189) & (pfs >= 3999)).sum()),
                "strip_over_0.369_sample_F>=3999": int((m & (r > 0.369) & (sfs >= 3999)).sum()),
                "strip_over_0.219_sample_F>=3999": int((m & (r > 0.2189) & (sfs >= 3999)).sum())})
print(run, json.dumps(out))
