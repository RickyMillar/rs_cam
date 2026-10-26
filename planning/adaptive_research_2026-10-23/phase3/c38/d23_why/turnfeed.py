"""C38 D23 check for the lead: does the merge defect of the slots (P3_C38
10.1) also hit the C22 turns? D23 headon, op 1, clearing-cut samples away
from the outer strip and away from entries: the samples over the oracle
bound c + 0.15 = 0.369, by feed (reduced: under 0.9 x the op's clearing
feed F 4000, or the feed-optimised 3556; full: at or over), and the move
lengths of the reduced-feed moves (a fed turn is one move on one turn
segment, at most turn_bound = s + (1 + sqrt 2) cell = 2.41 mm)."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
BOUND = 0.2189 + (0.5 + 0.3 + 0.1) / 6.0
TURN_BOUND = 1.2 + (1 + 2 ** 0.5) * 0.5
S = np.load(H / "samples_headon.npz"); it = list(S["intents"])
pos = S["pos"].astype(float); w = wall(pos)
op = (S["tp"] == 1) & S["cut"] & (S["intent"] == it.index("clearing_cut"))
strip = (w > 3.0) & (w <= 6.2)
m = op & ~strip
f = S["feed"]; rad = S["radial"]
reduced = f < 0.9 * 3556.0
over = m & (rad > BOUND)
# move lengths of the reduced-feed moves
idx = np.nonzero(m & reduced)[0]; mv = S["mv"][idx]
lens = []
for k in np.unique(mv):
    j = idx[mv == k]
    lens.append(float(np.hypot(*(pos[j[-1], :2] - pos[j[0], :2]))) + 0.3)
lens = np.array(lens)
out = {
    "op1_clearing_samples_off_strip": int(m.sum()),
    "over_bound": int(over.sum()),
    "over_bound_at_reduced_feed": int((over & reduced).sum()),
    "over_bound_at_full_feed": int((over & ~reduced).sum()),
    "reduced_feed_samples": int((m & reduced).sum()),
    "reduced_feed_samples_over_c": int((m & reduced & (rad > 0.2189)).sum()),
    "reduced_feed_moves": int(len(lens)),
    "reduced_move_length_p50": round(float(np.median(lens)), 2) if len(lens) else None,
    "reduced_moves_longer_than_turn_bound": int((lens > TURN_BOUND + 0.31).sum()),
    "reduced_feed_values_p10_p50_p90": [round(float(v)) for v in np.percentile(f[m & reduced], [10, 50, 90])],
}
# the full-feed over-bound samples: on short moves (turn-sized) or long?
idx2 = np.nonzero(over & ~reduced)[0]; mv2 = S["mv"][idx2]
l2 = []
for k in np.unique(mv2):
    j = np.nonzero(op & (S["mv"] == k))[0]
    l2.append(float(np.hypot(*(pos[j[-1], :2] - pos[j[0], :2]))) + 0.3)
l2 = np.array(l2)
out["full_feed_over_bound_moves"] = int(len(l2))
out["full_feed_over_bound_move_length_p50"] = round(float(np.median(l2)), 2) if len(l2) else None
out["full_feed_over_bound_moves_within_turn_bound"] = int((l2 <= TURN_BOUND + 0.31).sum())
print(json.dumps(out, indent=1))
(H / "turnfeed.json").write_text(json.dumps(out, indent=1))
