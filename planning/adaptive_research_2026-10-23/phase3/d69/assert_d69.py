"""D69: assert each number the README states, from the files in this folder. Run: python3 -I assert_d69.py"""
import json, re
from pathlib import Path
H = Path(__file__).resolve().parent
J = lambda f: json.load(open(H / f))
def near(a, b, tol=0.15): assert abs(a - b) <= tol, (a, b)
o = {r: J(f"opsum_{r}.json") for r in ("base_corne", "a0_corne", "opt2_corne", "opt2m_corne", "narrow_corne", "floor4_corne", "base_wanaka", "a0_wanaka", "opt2_wanaka", "opt2m_wanaka")}
# baseline Corne play job
near(o["base_corne"]["job_s"], 4333.8); near(o["base_corne"]["4"]["total_s"], 1299.9)
w = o["base_corne"]["4"]; near(w["rapid_s"], 410.5); near(w["entry_s"], 518.3); near(w["cutting_s"], 371.0); assert w["entries"] == 282
# waterline passes, holes and air
p = J("passes_base_corne.json"); assert p["passes"] == 282
near(p["air_cut_time_s"] / p["cutting_runtime_s"], 0.913, 0.001)
pl = p["pass_list"]; hole = [q for q in pl if max(q["x1"] - q["x0"], q["y1"] - q["y0"]) < 15]
assert len(hole) == 249 and sorted({round(q["zc"], 2) for q in hole}) == [1.0, 2.0, 3.0]
near(sum(q["air_s"] for q in pl), 56.6); near(sum(q["cut_s"] for q in pl), 395.9)
# helical waterline (nc-time, same integrator, both sides re-written by wl_gcode.py)
s = J("wlg_base_stat.json"); assert s["chain_len_hist"] == {"1": 7, "2": 2, "11": 2, "3": 83}
assert s["max_haus_holes"] == 0.0 and s["helix_holes"]["entries"] == 116 and s["helix"]["entries"] == 94 and s["helix"]["max_link_mm"] <= 0.53
t = {}
for ln in open(H / "nc_time_waterline.txt"):
    m = re.match(r"\s*(\d+)m(\d+)s\s+\d+\s+\d+\s+(\S+)", ln)
    if m: t[m.group(3)] = int(m.group(1)) * 60 + int(m.group(2))
assert t == {"wlg_base_base.nc": 1773, "wlg_base_region.nc": 1720, "wlg_base_helix_holes.nc": 1053, "wlg_base_helix.nc": 969}, t
# option 2 and the merge rule
assert o["opt2_corne"]["5"]["entries"] == 327 and o["opt2m_corne"]["5"]["entries"] == 312
near(o["opt2_corne"]["5"]["total_s"], 767.3); near(o["opt2m_corne"]["5"]["total_s"], 745.2); near(o["opt2m_corne"]["job_s"], 4973.1)
near(o["opt2_corne"]["3"]["total_s"] - o["a0_corne"]["3"]["total_s"], 354.5)
near(o["opt2_wanaka"]["7"]["total_s"], 1756.4); near(o["opt2m_wanaka"]["7"]["total_s"], 1691.7); near(o["base_wanaka"]["7"]["total_s"], 1041.1)
d = {r: J(f"depth_{r}.json")["tp5"] for r in ("base_corne", "a0_corne", "opt2_corne", "opt2m_corne")}
assert [d[r]["removed_at_axial_over_dpp_mm3"] for r in d] == [6750, 14062, 3507, 3170]
# item 5
near(o["floor4_corne"]["job_s"] - o["a0_corne"]["job_s"], 1012.1); near(o["floor4_corne"]["3"]["total_s"] - o["a0_corne"]["3"]["total_s"], 1077.2)
near(o["narrow_corne"]["job_s"] - o["a0_corne"]["job_s"], 7.7)
ht = open(H / "hole_time_base_corne.txt").read(); assert "'vol_mm3': 11120.7, 'visits': 136" in ht and "'time_s': 351.8, 'vol_mm3': 3987.9, 'visits': 1132" in ht
assert "web bottleneck Z 0.5" in open(H / "web_base_corne.txt").read().splitlines()[-1]
print("all D69 assertions hold")
