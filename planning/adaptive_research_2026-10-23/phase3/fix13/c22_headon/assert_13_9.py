"""C22 §13.9: assert each number of P3_C22_3d_turns.md §13.9. Run headon.py and
time_split.py first; the D23 tables are read from ../d23_log.json and ../entries_d23.json."""
import json, os
H = os.path.dirname(os.path.abspath(__file__)); J = lambda *p: json.load(open(os.path.join(H, *p)))
h, t = J("headon.json"), J("time_split.json")
log, ent = J("..", "d23_log.json"), J("..", "entries_d23.json")
# 2D re-entries fell, the 3D descents did not
assert log["corne"]["reentries_total"] == 120 and log["corne_pick"]["reentries_total"] == 161
assert h["headon"]["removing"] == 1093 and h["pick"]["removing"] == 1010
assert h["headon"]["descents_1mm"] - h["headon"]["removing"] == 180 and h["pick"]["descents_1mm"] - h["pick"]["removing"] == 2
# only 188 of the 1093 lie within R of a 2D re-entry at their level
assert h["headon"]["removing_at_a_2d_reentry"] == 188
# the head-on run has 6.5 m more keep-down link and about 200 more sharp link junctions
assert 6500 < h["headon"]["linking_xy_mm"] - h["pick"]["linking_xy_mm"] < 6600
lk = lambda r: sum(n for k, n in h[r]["turns_over_30"].items() if "linking" in k)
assert lk("headon") == 216 and lk("pick") == 16
assert h["headon"]["turns_over_30"]["clearing_cut -> clearing_cut"] - h["pick"]["turns_over_30"]["clearing_cut -> clearing_cut"] == 103
# time: C35 -> D23 pick +174.6 s (op 1 +16.5, copy op +158.1); D23 pick -> headon +266.9 s, all op 1
op = lambda r, tp: sum(v for k, v in t[r].items() if k.startswith(f"tp{tp} "))
assert abs(t["D23 pick"]["total"] - t["C35 pick"]["total"] - 174.6) < 0.2
assert abs(op("D23 pick", 3) - op("C35 pick", 3) - 158.1) < 0.2 and abs(op("D23 pick", 1) - op("C35 pick", 1) - 16.5) < 0.2
assert abs(t["D23 headon"]["total"] - t["D23 pick"]["total"] - 266.9) < 0.2 and abs(op("D23 headon", 3) - op("D23 pick", 3)) < 1.0 and abs(op("D23 headon", 1) - op("D23 pick", 1) - 267.3) < 0.2
assert abs(t["D23 headon"]["tp1 linking"] - t["D23 pick"]["tp1 linking"] - 127.7) < 0.2
assert abs(t["D23 headon"]["tp1 entry_plunge"] - t["D23 pick"]["tp1 entry_plunge"] - 78.3) < 0.2
# wanaka: 153 of 164 re-entries OverBound; descents unchanged
assert log["wanaka200"]["reentries_by_op_join"]["op0 Some(OverBound)"] == 153 and log["wanaka200"]["reentries_total"] == 164
assert ent["wanaka200 spiral_headon D23"]["descent_1mm_removing_stock"]["keep-down link"] == ent["wanaka200 spiral_pick D23"]["descent_1mm_removing_stock"]["keep-down link"] == 2016
print("all §13.9 claims hold")
