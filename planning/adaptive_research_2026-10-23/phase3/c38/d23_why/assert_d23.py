"""C38 D23 analysis: assert the numbers of P3_C38 section 10 (run why.py,
slotfeed.py, band_any.py pick and c38a, stockedge.py first)."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
b = json.loads((H / "band_pick.json").read_text()); a = json.loads((H / "band_c38a.json").read_text())
assert (b["band_3_9_mm2"], b["not_reached_mm2"], b["not_reached_within_6mm_of_stock_edge_mm2"]) == (455, 440, 403)
assert (a["band_3_9_mm2"], a["not_reached_mm2"], a["not_reached_within_6mm_of_stock_edge_mm2"]) == (794, 739, 590)
sf = json.loads((H / "slotfeed.json").read_text())
assert (sf["over_c"], sf["over_c_at_full_feed_4000"], sf["moves"], sf["moves_longer_than_s"]) == (2940, 2940, 360, 267)
assert sf["move_length_p50_mm"] == 2.26
assert json.loads((H / "slotfeed_time.json").read_text())["added_s"] == 36.0
w = json.loads((H / "why.json").read_text())
assert w["pick"]["copy_deep_samples"] == 130 and w["pick"]["copy_deep_axial_max"] == 12.0
assert w["pick"]["copy_deep_by_wall"][">3"] == 127
c = json.loads((H / "clipair_base.json").read_text())
assert (c["headon"]["tp1"]["deep_samples"], c["headon"]["tp1"]["at_containment_line_pm_0_5"]) == (654, 158)
assert (c["pick"]["tp1"]["deep_samples"], c["pick"]["tp1"]["at_containment_line_pm_0_5"]) == (627, 165)
assert (c["headon"]["tp3"]["deep_samples"], c["headon"]["tp3"]["at_containment_line_pm_0_5"]) == (129, 126)
assert (c["pick"]["tp3"]["deep_samples"], c["pick"]["tp3"]["at_containment_line_pm_0_5"]) == (130, 127)
t = json.loads((H / "turnfeed.json").read_text())
assert (t["reduced_feed_samples"], t["reduced_feed_samples_over_c"], t["reduced_feed_moves"]) == (6687, 95, 387)
assert (t["over_bound"], t["over_bound_at_full_feed"], t["full_feed_over_bound_moves"], t["full_feed_over_bound_moves_within_turn_bound"]) == (2014, 1943, 568, 519)
pm = json.loads((H / "peak_move.json").read_text())
assert (pm["headon"]["deep_on_line"], pm["headon"]["deep_on_line_on_peak_move"]) == (126, 125)
assert (pm["pick"]["deep_on_line"], pm["pick"]["deep_on_line_on_peak_move"]) == (127, 125)
assert pm["pick"]["peak_move_x_range"] == [151.06, 151.06] and pm["pick"]["peak_move_stock_edge_x"] == 2.02
assert pm["pick"]["peak_move_axial_max"] == 12.0 and pm["headon"]["peak_move_axial_max"] == 9.0
assert pm["pick"]["tp3_cut_levels_within_R"] == [2.01, 5.01, 8.01, 11.01, 14.01, 17.0]
assert min(pm["pick"]["tp1_cut_levels_within_R"]) == 17.0
od = json.loads((H / "op1_deep.json").read_text())
assert od["C38a nibble"]["op1_axial_over_9"] == 89 and od["D23 pick slot"]["op1_axial_over_9"] == 627
assert od["D23 headon slot"]["op1_axial_over_9"] == 654
print("ASSERTS OK")
