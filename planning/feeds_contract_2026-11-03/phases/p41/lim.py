"""P41: the depth rows (force_budget) of one toolpath of a run. usage: lim.py <out dir> [id ...]"""
import json, sys
d = json.load(open(sys.argv[1] + "/summary.json"))
ids = [int(v) for v in sys.argv[2:]] or [2]
print("total_runtime_s", d["total_runtime_s"], "collisions", d["collision_count"], "rapid", d["rapid_collision_count"], "failed", d["collision_checks_failed"])
for tp in d["per_toolpath"]:
    if tp["id"] in ids:
        rows = {r["subject"]["group"]: r["check"]["value"] for r in tp["limit_rows"] if r["check"]["limit"] == "force_budget" and r["subject"]["kind"] == "group"}
        print(tp["id"], tp["name"][:40], "moves", tp["move_count"], "coll", tp["collision_count"], {k: (None if v is None else round(v, 3)) for k, v in rows.items()})
    if tp["collision_count"]:
        print("COLLISION", tp["id"], tp["name"], tp["collision_count"])
