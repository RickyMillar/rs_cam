"""D69: per toolpath of one run: the acceleration clock (trace toolpath_summaries: total and runtime_by_intent), the
entries (runs of non-rapid samples after a rapid), peak axial and removed volume. Usage: opsum.py <run dir> -> opsum_<run>.json"""
import json, sys
run = sys.argv[1]; d = json.load(open(f"{run}/simulation.json")); t = d["trace"]
ent = {}; prev = {}
for s in t["samples"]:
    tp = s["toolpath_id"]; r = s["cut_kinematics"] == "rapid"
    if not r and prev.get(tp, True): ent[tp] = ent.get(tp, 0) + 1
    prev[tp] = r
out = {}
for x in t["toolpath_summaries"]:
    rb = x.get("runtime_by_intent") or {}
    out[x["toolpath_id"]] = {"total_s": round(x["total_runtime_s"], 1), **{k: round(v, 1) for k, v in rb.items() if k != "total_s" and v}, "entries": ent.get(x["toolpath_id"], 0),
                             "peak_axial": round(x["peak_axial_doc_mm"], 2), "removed_mm3": round(x["total_removed_volume_est_mm3"])}
out["job_s"] = round(sum(v["total_s"] for v in out.values()), 1)
json.dump(out, open(f"opsum_{run}.json", "w"), indent=1); print(run, json.dumps(out))
