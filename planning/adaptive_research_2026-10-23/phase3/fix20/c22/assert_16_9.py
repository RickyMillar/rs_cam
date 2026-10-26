"""C22 §16.9: assert the numbers of P3_C22_3d_turns.md §16.9. Run parta_place.py, turns_xy.py
first (they write parta_place.json, turns_xy.json)."""
import json, os, re
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..", "..")
J = lambda f: json.load(open(os.path.join(H, f)))
p = J("parta_place.json")
c_rise = p["corne D29"]["over_mm"] - p["corne D26"]["over_mm"]; c_near = p["corne D29"]["over_near_steps_mm"] - p["corne D26"]["over_near_steps_mm"]
w_rise = p["wanaka200 D29"]["over_mm"] - p["wanaka200 D26"]["over_mm"]; w_near = p["wanaka200 D29"]["over_near_steps_mm"] - p["wanaka200 D26"]["over_near_steps_mm"]
assert 490 < c_rise < 500 and 420 < c_near < 430 and 1135 < w_rise < 1145 and 1060 < w_near < 1070, (c_rise, c_near, w_rise, w_near)
t = J("turns_xy.json")
assert t["fix16/corne_pick"] == {"3d": 831, "xy": 452} and t["fix20/a_only/corne"] == {"3d": 1767, "xy": 446}
# the collision XY is a C12 corridor Start entry at Z 11, 5, 0.5
lines = [ln.split("C40 entry", 1)[1] for ln in open(os.path.join(P3, "fix20/a_only/corne/stdout_plain.txt")) if "index=0" in ln and "C40 entry kind" in ln]
hits = []
for ln in lines:
    kv = dict(re.findall(r"(\w+)=(\S+)", ln))
    if (float(kv["x"]) - 91.3) ** 2 + (float(kv["y"]) + 1.13) ** 2 < 0.01:
        hits.append((kv["cause"], kv["z"], kv["kind"]))
assert hits == [("Some(Start)", "11.000", "Plunge"), ("Some(Start)", "5.000", "Plunge"), ("Some(Start)", "0.500", "Plunge")], hits
print("all §16.9 claims hold")
