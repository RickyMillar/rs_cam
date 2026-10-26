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
print("ASSERTS OK")
