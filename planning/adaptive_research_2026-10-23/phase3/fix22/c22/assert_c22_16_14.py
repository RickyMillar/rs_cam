"""Assert each number that P3_C22_3d_turns.md §16.14 claims, from d31_rise.json and nearA_new_wanaka200.json."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
d = json.loads((H / "d31_rise.json").read_text()); n = json.loads((H / "nearA_new_wanaka200.json").read_text())
a, b = d["D26"], d["D31"]
assert (a["over_mm"], b["over_mm"]) == (2222.8, 2812.8)
rise = round(b["over_mm"] - a["over_mm"], 1); assert rise == 590.0
k26, k31 = a["by kind"], b["by kind"]
steps = round(k31["on a descent"] + k31["on a climb"] - k26["on a descent"] - k26["on a climb"], 1); assert steps == 36.1
after = round(k31["after a descent"] + k31["after a climb"] - k26["after a descent"], 1); assert after == 187.4
level = round(k31["level"] - k26["level"], 1); assert level == 366.5
assert round(steps + after + level, 1) == rise
p26, p31 = a["by place"], b["by place"]
at26 = p26["at a D26 plunge foot, own foot"]; at31 = p31["at a D26 plunge foot, no own foot"] + p31["at a D26 plunge foot, own foot"]
assert round(at31 - at26, 1) == 391.3 and p31["at a D26 plunge foot, no own foot"] == 318.1
assert round(at31 - at26, 1) / rise > 0.66  # two thirds of the rise at D26's plunge feet
assert d["plunge feet"] == {"D26": 883, "D31": 712}
# near-A (the +504): new places beyond R of any D26 over sample at that level
assert n["new within 3.0 mm"]["total"] == 246.8 and round(n["D31 total"] - n["D26 total"], 1) == 503.7
t = d["trade"]
assert t["wanaka200"]["D29v4->D31"]["s_per_split"] == 0.35 and t["wanaka200"]["D31->D26"]["s_per_split"] == 0.362
assert t["wanaka200"]["D29v4->D31"]["over_mm_per_split"] == -0.253 and t["wanaka200"]["D31->D26"]["over_mm_per_split"] == -0.279
assert t["corne"]["D30->D31"]["splits"] == 191 and t["corne"]["D30->D31"]["over_mm"] == -3.7
print("§16.14 asserts hold")
