"""D57: where op 7 (the wanaka edge op) starts its feed descents, and which stock holds that start up.
From the trace of toolpath 7 (simulation.json, setup-0 frame) and the stock after op 1 (stock_<tag>.npz from op7_wanaka.py,
0.5 mm grid, before = after op 1):
- a site = a feed descent run (non-rapid samples that go down at one XY, after a rapid); its rapid floor = the Z of the last
  rapid sample before it (the feed starts there); its bottom = the last sample of the run.
- the column under the disc (R 3, cells within R of the site): the highest op-1 stock cell and its place: 'frame' (outside
  the part outline), 'band 0-3' (inside, within 3 mm of the outline: inside op 1's reach, outside its centre containment),
  'inside > 3'. Also the place of the highest cell within 0.6 mm (one sim cell) of the floor, if the floor is held by stock.
- 'edge site': centre within 6 mm of the outline (the disc reaches the 0-3 mm band).
Usage: entry_floor.py <run dir under phase3> <tag>. Writes entry_floor_<tag>.json."""
import collections, json, math, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
run, tag = sys.argv[1], sys.argv[2]
T = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 7]
S = np.load(H / f"stock_{tag}.npz"); top = S["before"]; g = float(S["g"]); PL = S["PL"]; ny, nx = top.shape
def place(i): return "frame" if i >= 4 else ("band 0-3" if i == 0 else "inside > 3")
sites = []; last_rapid_z = None; cur = None
for s in T:
    p = s["position"]
    if s["cut_kinematics"] == "rapid":
        last_rapid_z = p[2]; cur = None; continue
    if cur is not None and abs(p[0] - cur["x"]) < 1e-6 and abs(p[1] - cur["y"]) < 1e-6 and p[2] < cur["bottom"] - 1e-9:
        cur["bottom"] = p[2]; continue
    if last_rapid_z is not None and cur is None and abs(p[2] - last_rapid_z) > -1:
        # first non-rapid sample after a rapid: a descent starts if the next samples go down at this XY
        cur = {"x": p[0], "y": p[1], "floor": last_rapid_z, "bottom": p[2]}; sites.append(cur); last_rapid_z = None; continue
    cur = None
sites = [s for s in sites if s["floor"] - s["bottom"] > 0.05]
k = int(math.ceil(3.0 / g)); out = collections.Counter(); hold = collections.Counter(); rows = []
din = None
for s in sites:
    ci, cj = int(s["x"] / g), int(s["y"] / g)
    J, I = np.mgrid[max(cj - k, 0):min(cj + k + 1, ny), max(ci - k, 0):min(ci + k + 1, nx)]
    m = ((I + 0.5) * g - s["x"]) ** 2 + ((J + 0.5) * g - s["y"]) ** 2 <= 9.0
    I, J = I[m], J[m]; h = top[J, I]; a = int(np.argmax(h))
    edge = bool((PL[J, I] == 0).any() or (PL[J, I] >= 4).any())
    near = h >= s["floor"] - 0.6
    hp = collections.Counter(place(int(PL[J[i], I[i]])) for i in np.nonzero(near)[0])
    rows.append({"x": round(s["x"], 2), "y": round(s["y"], 2), "floor": round(s["floor"], 2), "bottom": round(s["bottom"], 2),
                 "max_stock": round(float(h.max()), 2), "max_place": place(int(PL[J[a], I[a]])), "edge": edge,
                 "holding_places": dict(hp)})
E = [r for r in rows if r["edge"]]
res = {"run": run, "descent_sites": len(rows), "edge_sites": len(E),
       "edge: floor - bottom median mm": round(float(np.median([r["floor"] - r["bottom"] for r in E])), 2) if E else None,
       "edge: highest stock under the disc, by place": dict(collections.Counter(r["max_place"] for r in E)),
       "edge: floor held by stock within 0.6 mm (sites)": sum(1 for r in E if r["holding_places"]),
       "edge: places of the holding cells (sites with any such cell)": dict(collections.Counter(p for r in E for p in r["holding_places"])),
       "edge: floor median / max stock median": [round(float(np.median([r["floor"] for r in E])), 2), round(float(np.median([r["max_stock"] for r in E])), 2)] if E else None,
       "non-edge: highest by place": dict(collections.Counter(r["max_place"] for r in rows if not r["edge"]))}
print(json.dumps(res, indent=1)); res["rows"] = rows
json.dump(res, open(H / f"entry_floor_{tag}.json", "w"), indent=1)
