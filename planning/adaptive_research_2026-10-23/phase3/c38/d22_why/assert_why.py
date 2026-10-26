"""C38 D22 analysis: assert every number of P3_C38 section 9 that why.py,
why2.py and stockedge.py give (run those first)."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
w = json.loads((H / "why.json").read_text())
w2 = json.loads((H / "why2.json").read_text())
a = w["c38a"]
assert w["d20"]["op1_strip_clearing_cut"]["length_mm"] == 23.1
assert a["op1_strip_clearing_cut"]["length_mm"] == 4547.7
assert a["op1_strip_clearing_cut"]["radial_p50"] == 0.0
assert a["op1_strip_clearing_cut"]["radial_over_c"] == 1176
assert a["op1_strip_clearing_cut"]["samples"] == 15159
assert a["op1_strip_entry_plunge"]["samples"] == 0
assert (a["copy_link_samples"], a["copy_link_over_3_30"], a["copy_link_over_moves"]) == (4596, 301, 193)
assert a["copy_link_over_by_wall"]["1.5-3"] == 301
assert w["d20"]["copy_link_over_3_30"] == 0
m = w2["op1_strip_moves"]
assert (m["moves"], m["length_p50_mm"], m["across_share_p50"], m["moves_mostly_across"]) == (3049, 1.2, 0.8, 1908)
assert w2["copy_outside_d20"]["clearing_cut"]["removed_mm3"] == 181.9
assert w2["copy_outside_c38a"]["clearing_cut"]["removed_mm3"] == 3238.7
assert w2["copy_outside_c38a"]["linking"]["removed_mm3"] == 2475.9
assert (w2["over_link_samples"], w2["over_link_with_a_same_level_cut_within_R_before"]) == (301, 14)
assert w2["over_link_levels"] == [0.5, 2.01, 8.01]
print("ASSERTS OK")
