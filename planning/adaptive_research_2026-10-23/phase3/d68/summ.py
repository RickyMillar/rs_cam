"""D68: one row per run of the edge op: acceleration-clock time (summary), entry and rapid moves, max axial, volume over
one DPP, removed volume, band left. Usage: summ.py <label> [...]"""
import json, sys
from pathlib import Path
H = Path(__file__).resolve().parent
rows = {}
for lab in sys.argv[1:]:
    for part, tp in (("corne", 5), ("holes", 5), ("wanaka", 7)):
        s = f"{lab}_{part}"
        try:
            c = json.load(open(H / f"clocks_{s}_tp{tp}.json")); d = json.load(open(H / f"depth_{s}.json"))[f"tp{tp}"]
        except (FileNotFoundError, ValueError, KeyError):
            continue
        r = {"edge_s": c["summary_total_s"], "entry_moves": c["classes"].get("entry", {}).get("moves", 0),
             "rapid_moves": c["classes"].get("rapid", {}).get("moves", 0), "max_axial": d["max_axial_mm"],
             "over_dpp_mm3": d["removed_at_axial_over_dpp_mm3"], "share_over": d["share_over_dpp"], "removed_mm3": d["removed_mm3"]}
        c1 = json.load(open(H / f"clocks_{s}_tp1.json")); r["op1_s"] = c1["summary_total_s"]
        if part != "wanaka":
            o = json.load(open(H / f"op5_{s}.json")); L = o["left_after_op5_mm2"]
            r["band_left_mm2_0-3/3-6.2/6.2-9"] = [L["outside 0-3"], L["outside 3-6.2"], L["outside 6.2-9"]]
            r["removed_beyond_9mm_mm3"] = o["op5_removed_mm3_by_place"]["outside > 9"]
        else:
            o = json.load(open(H / f"op7_{s}.json"))
            r["band_inside_0_9_left_mm2"] = o["band_inside_0_9_left_after_mm2"]; r["removed_outside_outline_mm3"] = o["edge_op_removed_outside_outline_mm3"]
        rows[s] = r
print(json.dumps(rows, indent=1))
