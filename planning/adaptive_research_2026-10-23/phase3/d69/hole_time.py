"""D69: per op, the time (sample clock), removed volume and number of visits (runs of consecutive non-rapid samples with
the tool centre inside a hole) with the tool centre inside one of the 83 switch holes (holes of model_map.py with area
> the tool's own disc of the 3 mm tool, so the five 2 mm holes drop out). Usage: hole_time.py <run> <model npz> <stock npz>"""
import json, sys, collections, math
import numpy as np
run, mz, sz = sys.argv[1:4]; ZTOP = float(sys.argv[4]) if len(sys.argv) > 4 else 1e9  # only samples with the tip below ZTOP (the plate top)
M = np.load(mz); S = np.load(sz); lo = S["lo"]; g = float(S["g"]); H = M["holes"]
area = collections.Counter(H[H > 0].ravel()); big = {i for i, n in area.items() if n * g * g > math.pi * 1.5 ** 2}
sim = json.load(open(f"{run}/simulation.json")); T = sim["trace"]["samples"]
acc = collections.defaultdict(collections.Counter); prev = {}
for s in T:
    tp = s["toolpath_id"]; x, y, z = s["position"]; i = int((x - lo[0]) // g); j = int((y - lo[1]) // g)
    h = int(H[j, i]) if 0 <= j < H.shape[0] and 0 <= i < H.shape[1] else 0
    inside = h in big and s["cut_kinematics"] != "rapid" and z < ZTOP
    a = acc[tp]
    if inside:
        a["time_s"] += s["segment_time_s"]; a["vol"] += s["removed_volume_est_mm3"]
        if prev.get(tp) != h: a["visits"] += 1
        a["holes_" + str(h)] = 1
    prev[tp] = h if inside else None
for tp, a in sorted(acc.items()):
    nh = sum(1 for k in a if k.startswith("holes_"))
    print(tp, {"time_s": round(a["time_s"], 1), "vol_mm3": round(a["vol"], 1), "visits": a["visits"], "holes_touched": nh})
