"""D69: the passes of one Waterline op in a simulation artifact.
A pass = a maximal run of non-rapid samples (entry + cut). Per pass: Z of the cut, XY bbox, times by class, removed volume,
air cut time (cut samples with removed_volume_est_mm3 == 0). Regions: union-find over passes whose XY bboxes overlap
(any level). Writes passes_<label>.json. Usage: wl_passes.py <run dir> <tp> <label>"""
import json, sys, collections
run, tp, label = sys.argv[1], int(sys.argv[2]), sys.argv[3]
d = json.load(open(f"{run}/simulation.json"))
S = [s for s in d["trace"]["samples"] if s["toolpath_id"] == tp]
summ = next(x for x in d["trace"]["toolpath_summaries"] if x["toolpath_id"] == tp)
passes, cur, rapid_runs, gap = [], None, 0, {"rapid_s": 0.0}
in_rapid = True
for s in S:
    if s["cut_kinematics"] == "rapid":
        if not in_rapid: rapid_runs += 1
        in_rapid = True; gap["rapid_s"] += s["segment_time_s"]
        if cur: passes.append(cur); cur = None
        continue
    in_rapid = False
    if cur is None:
        cur = {"entry_s": 0.0, "cut_s": 0.0, "air_s": 0.0, "vol": 0.0, "z": [], "x0": 1e9, "x1": -1e9, "y0": 1e9, "y1": -1e9, "len": 0.0, "max_ax": 0.0}
    si = s["source_intent"] if isinstance(s["source_intent"], str) else "?"
    t = s["segment_time_s"]
    if si.startswith("entry"):
        cur["entry_s"] += t
    else:
        cur["cut_s"] += t; x, y, z = s["position"]; cur["z"].append(z)
        cur["x0"] = min(cur["x0"], x); cur["x1"] = max(cur["x1"], x); cur["y0"] = min(cur["y0"], y); cur["y1"] = max(cur["y1"], y)
        cur["len"] += t * s["feed_rate_mm_min"] / 60
        if s["removed_volume_est_mm3"] <= 0: cur["air_s"] += t
    cur["vol"] += s["removed_volume_est_mm3"]; cur["max_ax"] = max(cur["max_ax"], s["axial_doc_mm"])
if cur: passes.append(cur)
for p in passes:
    z = sorted(p.pop("z")) or [float("nan")]; p["zc"] = z[len(z) // 2]
# regions: union-find on bbox overlap
par = list(range(len(passes)))
def f(i):
    while par[i] != i: par[i] = par[par[i]]; i = par[i]
    return i
order = sorted(range(len(passes)), key=lambda i: passes[i]["x0"])
for a_i, a in enumerate(order):
    A = passes[a]
    for b in order[a_i + 1:]:
        B = passes[b]
        if B["x0"] > A["x1"]: break
        if B["y0"] <= A["y1"] and A["y0"] <= B["y1"]: par[f(a)] = f(b)
reg = collections.defaultdict(list)
for i in range(len(passes)): reg[f(i)].append(i)
rid = {r: k for k, r in enumerate(sorted(reg, key=lambda r: min(reg[r])))}
for i, p in enumerate(passes): p["region"] = rid[f(i)]
R = []
for r, ids in reg.items():
    P = [passes[i] for i in ids]
    w = max(p["x1"] for p in P) - min(p["x0"] for p in P); h = max(p["y1"] for p in P) - min(p["y0"] for p in P)
    R.append({"region": rid[r], "passes": len(P), "w": round(w, 2), "h": round(h, 2), "levels": sorted({round(p["zc"], 2) for p in P}),
              "cut_s": round(sum(p["cut_s"] for p in P), 1), "air_s": round(sum(p["air_s"] for p in P), 1), "entry_s": round(sum(p["entry_s"] for p in P), 1),
              "vol": round(sum(p["vol"] for p in P), 1), "len": round(sum(p["len"] for p in P), 1)})
R.sort(key=lambda r: r["region"])
out = {"label": label, "summary_total_s": summ["total_runtime_s"], "runtime_by_intent": summ.get("runtime_by_intent"),
       "air_cut_time_s": summ["air_cut_time_s"], "cutting_runtime_s": summ["cutting_runtime_s"], "passes": len(passes),
       "rapid_runs": rapid_runs, "rapid_s_samples": round(gap["rapid_s"], 1), "regions": R, "pass_list": passes}
json.dump(out, open(f"passes_{label}.json", "w"), indent=0)
lv = collections.Counter(round(p["zc"], 1) for p in passes)
small = [r for r in R if max(r["w"], r["h"]) < 30]
print(f"{label}: total {summ['total_runtime_s']:.1f}s {summ.get('runtime_by_intent')}")
print(f" passes {len(passes)} rapid runs {rapid_runs}; regions {len(R)} (small <30mm: {len(small)})")
print(f" air (trace) {summ['air_cut_time_s']:.1f}/{summ['cutting_runtime_s']:.1f} s; air (cut samples, removed=0) {sum(p['air_s'] for p in passes):.1f}/{sum(p['cut_s'] for p in passes):.1f} s")
print(" passes per level:", sorted(lv.items(), reverse=True))
for r in sorted(R, key=lambda r: -r["passes"])[:6]: print("  big", {k: v for k, v in r.items() if k != "levels"}, "levels", len(r["levels"]), r["levels"][:3], r["levels"][-2:])
