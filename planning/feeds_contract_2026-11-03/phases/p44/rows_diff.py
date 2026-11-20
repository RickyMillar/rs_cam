"""P44: compare the rows of two runs of one setup. usage: rows_diff.py <out dir before> <out dir after>
Prints the setup totals, then each toolpath whose moves or a force-budget depth row changed
(value, bound, state, deepest sample), and each row of any limit whose state changed."""
import json, sys
a = json.load(open(sys.argv[1] + "/summary.json")); b = json.load(open(sys.argv[2] + "/summary.json"))
for tag, d in (("before", a), ("after", b)):
    bad = [(t["id"], t["collision_count"]) for t in d["per_toolpath"] if t["collision_count"]]
    print("%-6s total_runtime_s %.1f collisions %d rapid %d failed checks %d %s" % (tag, d["total_runtime_s"], d["collision_count"], d["rapid_collision_count"], d["collision_checks_failed"], bad))
def rows(tp):
    out = {}
    for r in tp["limit_rows"]:
        c = r["check"]; s = r["subject"]
        key = (c["limit"], s.get("kind"), s.get("group"))
        out[key] = (c["value"], c["bound"], c["state"]["state"], (r.get("deepest") or {}).get("depth_mm"))
    return out
B = {t["id"]: t for t in b["per_toolpath"]}
for t in a["per_toolpath"]:
    u = B.get(t["id"])
    if u is None: print("gone:", t["id"], t["name"]); continue
    ra, rb = rows(t), rows(u)
    lines = []
    for k in sorted(set(ra) | set(rb), key=str):
        x, y = ra.get(k), rb.get(k)
        if x == y: continue
        state = (x and x[2]) != (y and y[2])
        if k[0] == "force_budget" or state:
            lines.append("   %s: %s -> %s%s" % (k, x, y, "  STATE CHANGED" if state else ""))
    if t["move_count"] != u["move_count"] or lines:
        print("%d %s: moves %d -> %d" % (t["id"], t["name"][:44], t["move_count"], u["move_count"]))
        print("\n".join(lines))
for i in set(B) - {t["id"] for t in a["per_toolpath"]}: print("new:", i, B[i]["name"])
