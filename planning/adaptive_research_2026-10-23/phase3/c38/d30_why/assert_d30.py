"""C38 §15.6: assert each claim the section makes from the D30 files
(strip87.json, departure.json, slot_over.json). Prints ASSERTS OK."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
s87 = json.loads((H / "strip87.json").read_text())
dep = json.loads((H / "departure.json").read_text())
so = json.loads((H / "slot_over.json").read_text())
# 15.6.1: 87 samples on 19 moves; every move at program F 4000.
assert s87["bad_samples"] == 87 and s87["bad_moves"] == 19
assert all(m["program_F"] == 4000.0 for m in s87["moves"])
# 86 of them on 18 straight moves that leave a vertical plunge, start < R = 3 mm from it,
# and end at most 4.091 mm from it; the next move is capped (F < 1518.75).
assert dep["departure_moves"] == 18 and dep["departure_samples"] == 86
assert dep["departure_max_end_from_plunge_mm"] <= 4.1
assert all(r["start_from_plunge_mm"] < 3.0 for r in dep["rows"] if r["g"] == 1)
assert all(any(v < 1518.75 for v in m["next_F"][:2]) for m in s87["moves"] if m["kin"] == "linear")
nxt = {next(v for v in m["next_F"] if v < 1518.75) for m in s87["moves"] if m["kin"] == "linear"}
assert min(nxt) == 312.0 and max(nxt) == 434.0
# 4 places x 3 strip levels.
zs = sorted({m["z"] for m in s87["moves"] if m["kin"] == "linear"})
assert zs == [0.5, 5.0, 11.0]
# The one other sample: a G3 arc of move 5086, radial 0.37 (bound 0.369).
assert len(dep["other"]) == 1 and dep["other"][0]["move"] == 5086 and dep["other"][0]["g"] == 3
assert [m for m in s87["moves"] if m["move"] == 5086][0]["radial_max"] <= 0.371
# 15.6.2: the slot class, 344.7 mm: the program feed equals the sample feed everywhere;
# 340.4 mm at F_op x [0.167, 0.95], 4.3 mm at the plunge rate 750.
assert so["length_mm"] == 344.7 and so["program_feed_equals_sample_feed_mm"] == 344.7
assert so["not_750_mm"] == 340.4 and so["not_750_in_slot_factor_range_mm"] == 340.4
assert so["at_plunge_rate_750_mm"] == 4.3
r = so["not_750_ratio_to_F_op_p0_p10_p50_p90_p100"]
assert r[0] >= 0.166 and r[-1] < 1.0
assert so["not_750_length_by_wall_band_mm"]["3-6.2 outside (strip)"] == 337.5
assert so["not_750_length_at_strip_levels_mm"] == 336.9
d = so["not_750_ratio_minus_c_over_sim_radial_p10_p50_p90"]
assert d[1] < 0.0
print("ASSERTS OK")
