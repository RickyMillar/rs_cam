"""C22 §14.6: assert each number of P3_C22_3d_turns.md §14.6 (wanaka200 turn feeds)."""
import json, os, re, collections
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..", "..")
strip = lambda p: re.sub(r"\x1b\[[0-9;]*m", "", open(p).read())
def summary(d):
    for ln in strip(os.path.join(P3, d, "stdout_plain.txt" if os.path.exists(os.path.join(P3, d, "stdout_plain.txt")) else "stdout.txt")).splitlines():
        if "C22 turn summary" in ln and "index=0" in ln:
            return {k: v.strip('"') for k, v in re.findall(r"(\w+)=(\S+)", ln.split("C22 turn summary", 1)[1])}
s24, s25 = summary("fix14/wanaka200"), summary("fix15/wanaka200_pick")
assert (s24["planned"], s24["cut"], s24["fed"], s24["unfed"], s24["full_feed_unmatched"]) == ("77", "74", "2", "72", "0")
assert (s25["planned"], s25["cut"], s25["fed"], s25["unfed"]) == ("331", "77", "5", "72")
# unfed with factor < 1 for every cut turn: reduced == cut
assert s24["reduced"] == s24["cut"]
# D24 wanaka sim: every clearing move runs at 1000 mm/min but 2 moves (4 samples), the fed
# turns, 34-41 mm inside the toolpath box; no cutting sample lies outside the box.
sim = json.load(open(os.path.join(P3, "fix14", "wanaka200", "simulation.json")))
S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
tp = json.load(open(os.path.join(P3, "fix14", "wanaka200", "tp_1_2_Back_Rough.json")))
b = next(i["xy_bbox"] for i in tp["semantic_trace"]["items"] if i["kind"] == "operation")
edge = lambda x, y: min(x - b["min_x"], b["max_x"] - x, y - b["min_y"], b["max_y"] - y)
feeds = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["source_intent"] == "clearing_cut")
assert feeds.most_common(1)[0][0] == 1000
red = [s for s in S if s["source_intent"] == "clearing_cut" and s["is_cutting"] and s["feed_rate_mm_min"] < 999]
assert len(red) == 4 and len({s["move_index"] for s in red}) == 2
assert all(34 < edge(*s["position"][:2]) < 41 for s in red)
assert all(edge(*s["position"][:2]) >= -1e-6 for s in S if s["is_cutting"])
# the 2D plan reaches beyond the box: the D24 held-link counter points (§14.5)
pts = [tuple(float(v) for v in re.findall(r"x=(\S+) y=(\S+)", ln)[0])
       for ln in strip(os.path.join(P3, "fix14", "wanaka200", "stdout_plain.txt")).splitlines() if "held link over standing stock" in ln]
assert len(pts) == 66 and all(edge(x, y) < 0 for x, y in pts)
print("all §14.6 claims hold")
