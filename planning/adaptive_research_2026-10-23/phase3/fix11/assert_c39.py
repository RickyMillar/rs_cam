"""C39: re-derive every number of C39_runtime.md with this script's own
reader (it does not import c39_runtime.py) and check the code cites."""
import glob, json, math, os, re
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, ".."); RS = os.path.abspath(os.path.join(P3, "..", "..", ".."))
md = " ".join(open(os.path.join(H, "C39_runtime.md")).read().split())
A = json.load(open(os.path.join(H, "audit_results.json")))
T = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
checks = 0


def need(s):
    global checks
    assert " ".join(s.split()) in md, ("C39_runtime.md does not state", s)
    checks += 1


# Code cites.
src = lambda p: open(os.path.join(RS, p)).read().splitlines()
rep = src("crates/rs_cam_core/src/stock/simulation_cut/reporting.rs")
assert "segment_len / commanded_feed × 60" in rep[19] and "PRE-modulation commanded" in rep[21] and "naive clock too" in rep[58]
assert "if self.machine.kinematics.is_some() {" in src("crates/rs_cam_core/src/session/compute/simulation.rs")[896]
g = "\n".join(src("crates/rs_cam_core/src/gcode/mod.rs")[1429:1436])
assert "fn rapid_feed_mm_min" in g and "machine.max_feed_mm_min.max(1.0)" in g
wt = open(os.path.join(RS, "planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml")).read()
ct = open(os.path.join(RS, "planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml")).read()
assert "[job.machine.kinematics]" in wt and "[job.machine.kinematics]" not in ct
assert "max_feed_mm_min = 10000.0" in wt and "max_feed_mm_min = 4000.0" in ct
checks += 6

RUNS = [("Corne zigzag_pick D20", "corne zigzag_pick D20", "fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick", "3D Rough 1", 4000.0),
        ("wanaka200 contour_parallel", "wanaka200 contour_parallel", "fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel", "2 Back Rough", 10000.0),
        ("wanaka200 zigzag_pick D18", "wanaka200 zigzag_pick D18", "fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag", "2 Back Rough", 10000.0),
        ("wanaka200 zigzag_pick D20", "wanaka200 zigzag_pick D20", "fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick", "2 Back Rough", 10000.0),
        ("wanaka200 AgentSearch", "wanaka200 AgentSearch", "fix2/wanaka200/agent", "fix2/simart/wanaka200/agent", "2 Back Rough", 10000.0)]
for label, key, rd, sd, op, rapid in RUNS:
    L = open(os.path.join(P3, rd, "program.nc")).read().splitlines()
    i0 = L.index(f"({op})") + 1; cmd = []
    for ln in L[i0:]:
        t = ln.strip()
        if t.startswith("(") and cmd and not t.startswith("(feeds"): break
        if t.startswith("("): continue
        if t and t[0] in "MTS":
            if cmd: break
            continue
        if t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ"): cmd.append(t)
    g_, p, f, mv = 0, (0.0, 0.0, 0.0), None, []
    for c in cmd:
        w = {k: float(v) for k, v in T.findall(c)}
        g_ = int(w.get("G", g_)); f = w.get("F", f); q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        if g_ in (2, 3):
            cx, cy = p[0] + w.get("I", 0), p[1] + w.get("J", 0); r = math.hypot(p[0] - cx, p[1] - cy)
            a0, a1 = math.atan2(p[1] - cy, p[0] - cx), math.atan2(q[1] - cy, q[0] - cx)
            sw = ((a0 - a1) if g_ == 2 else (a1 - a0)) % (2 * math.pi) or 2 * math.pi
            ln_ = math.hypot(r * sw, q[2] - p[2])
        else:
            ln_ = math.dist(p, q)
        mv.append((g_, p, q, ln_, f)); p = q
    fed = sum(m[3] / m[4] * 60 for m in mv if m[0] != 0); rap = sum(m[3] / rapid * 60 for m in mv if m[0] == 0)
    n = json.load(open(glob.glob(os.path.join(P3, rd, "tp_1_*.json"))[0]))["move_count"]; ops = mv[1:n + 1]
    sim = json.load(open(os.path.join(P3, sd, "simulation.json")))
    sm = next(t for t in sim["trace"]["toolpath_summaries"] if t["toolpath_id"] == 1)
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    samp = sum(s["segment_time_s"] for s in S); pk = sum(v["cutting_runtime_s"] for v in sm["per_kinematics"].values())
    ret = sum(s["segment_time_s"] * (s["feed_rate_mm_min"] / ops[s["move_index"]][4] if ops[s["move_index"]][0] != 0 and s["feed_rate_mm_min"] > 0 else 1.0) for s in S)
    first = {}
    for s in S: first.setdefault(s["move_index"], s["feed_rate_mm_min"])
    pr = [(first[k], m[4]) for k, m in enumerate(ops) if m[0] != 0 and k in first]
    rat = sorted(b / a for a, b in pr if a > 0); med = round(rat[len(rat) // 2], 3)
    lo = sum(1 for a, b in pr if a < b - 1e-6); hi = sum(1 for a, b in pr if a > b + 1e-6)
    pl = sum(m[3] / m[4] * 60 for m in ops if m[0] != 0 and math.hypot(m[2][0] - m[1][0], m[2][1] - m[1][1]) < 1e-3 and m[2][2] - m[1][2] < -1e-3)
    tot = round(fed + rap, 1)
    need(f"| {label} | {sm['total_runtime_s']:.1f} | {samp:.1f} | {pk:.1f} | {tot}: {fed:.1f} + {rap:.1f} | {ret:.1f} | {med} | {lo} / {hi} of {len(pr)} |")
    share = 100 * pl / (fed + rap)
    a = A[key]; c36 = 100 * a["time_split_s"].get("plunge feed", 0) / a["sample_time_s"]
    need(f"| {label} | {pl:.1f} | {share:.1f} % | {c36:.1f} % |")
    if label.startswith("wanaka"):
        assert sm["total_runtime_s"] >= fed + rap and abs(ret - (fed + rap)) <= 1.1
        need(f"{label.split(' ', 1)[1].replace('zigzag_pick ', '')} +{100 * (sm['total_runtime_s'] / (fed + rap) - 1):.1f} %")
        checks += 2
    else:
        assert abs(fed - (sm["cutting_runtime_s"])) < 0.05 and round(fed, 1) == 1370.8
        checks += 1
    if label == "wanaka200 contour_parallel":
        need(f"{sm['rapid_runtime_s']:.1f} s against {rap:.1f} s")
    if label == "wanaka200 zigzag_pick D20":
        need(f"D20 spends {share:.1f} % of its time")
print(f"C39: all {checks} checks hold")
