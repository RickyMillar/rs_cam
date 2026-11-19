"""P43: the depth rows of the toolpaths of one run, with the deepest sample and the ratio of
the judged depth to its own cap. usage: rows.py <out dir> [id ...]"""
import json, sys
d = json.load(open(sys.argv[1] + "/summary.json"))
ids = [int(v) for v in sys.argv[2:]]
print("total_runtime_s %.1f collisions %d rapid %d failed %d" % (d["total_runtime_s"], d["collision_count"], d["rapid_collision_count"], d["collision_checks_failed"]))
states = {}
for tp in d["per_toolpath"]:
    for r in tp["limit_rows"]:
        st = r["check"]["state"]["state"]; states[st] = states.get(st, 0) + 1
    if tp["collision_count"]: print("COLLISION", tp["id"], tp["name"], tp["collision_count"])
    if ids and tp["id"] not in ids: continue
    out = []
    for r in tp["limit_rows"]:
        c = r["check"]
        if c["limit"] != "force_budget" or r["subject"]["kind"] != "group" or c["value"] is None: continue
        dp = (r.get("deepest") or {}).get("depth_mm")
        out.append("%s %.3f/%.3f=%.2f (%s) deepest %s" % (r["subject"]["group"], c["value"], c["bound"], c["value"] / c["bound"], c["state"]["state"], "-" if dp is None else "%.3f" % dp))
    print(tp["id"], tp["name"][:34], "moves", tp["move_count"], "| " + "; ".join(out))
print("row states of the setup:", dict(sorted(states.items())))
