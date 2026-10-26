"""C38 D24 analysis: assert the numbers of P3_C38 section 12 (run
reversals.py, descents.py, peaks.py and c22a_drops.py first)."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
r = json.loads((H / "reversals.json").read_text())
assert (r["c35"]["planar_reversals"], r["c35"]["in_outer_strip"], r["c35"]["inside_silhouette"]) == (353, 71, 280)
assert (r["d24"]["planar_reversals"], r["d24"]["in_outer_strip"], r["d24"]["inside_silhouette"]) == (1968, 1675, 274)
assert r["d24"]["strip_levels"] == {"17.0": 65, "11.0": 564, "5.0": 518, "0.5": 518}
assert (r["d24"]["op1_sample_time_s"], r["d24"]["time_of_moves_at_strip_reversals_s"], r["d24"]["op1_strip_levels_time_s"]) == (1871.7, 91.9, 47.0)
assert r["c35"]["op1_sample_time_s"] == 1451.7
d = json.loads((H / "descents.json").read_text())
assert (d["c35"]["in_outer_strip"], d["d24"]["in_outer_strip"]) == (165, 403)
assert (d["c35"]["time_s_in_strip"], d["d24"]["time_s_in_strip"], d["c35"]["time_s_all"], d["d24"]["time_s_all"]) == (49.9, 124.0, 537.4, 633.8)
p = json.loads((H / "peaks.json").read_text())
assert len(p["tp1"]) == 8 and all(x["level_cuts_within_R_at"] == [17.0, 11.0] for x in p["tp1"])
assert sorted(x["wall_mm"] for x in p["tp1"]) == [5.0, 5.2, 5.2, 5.2, 5.2, 5.4, 5.4, 5.77]
assert sum(1 for x in p["tp1"] if x["stock_edge_mm"] > 11) == 7
c = p["tp3"][0]
assert (c["xyz"], c["axial"], c["wall_mm"], c["stock_edge_mm"], c["level_cuts_within_R_at"]) == ([2.78, -75.21, 0.5], 15.56, 2.8, 2.47, [11.01, 2.01])
g = json.loads((H / "c22a_drops.json").read_text())
assert (g["op1_strip_lines"], g["op1_strip_dropped_first"], g["op1_strip_dropped_more"], g["op1_strip_max_rounds"]) == (18, 204, 548, 3)
assert (g["c38a_op1_strip_cut_mm"], g["d24_op1_strip_cut_mm"]) == (4547.7, 3041.4)
assert (g["c38a_op1_strip_removed_mm3"], g["d24_op1_strip_removed_mm3"]) == (23671.5, 17340.0)
print("ASSERTS OK")
