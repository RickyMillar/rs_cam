"""C39: the cycle time of one rough op, three ways.

1. summary: simulation.json trace.toolpath_summaries[tp].total_runtime_s
   (the G2 page's cycle time; summary.json total_runtime_s is the sum over
   the ops).
2. samples: the sum of the trace samples' segment_time_s (= the trace's
   per-kinematics sum).
3. program: program.nc read alone. Each fed move at its own F, at its
   length (an arc at r * sweep, a helix at hypot(r * sweep, dz)); each G0 at
   the post's rapid rate, which is the machine's max_feed_mm_min
   (gcode/mod.rs rapid_feed_mm_min with high_feedrate_mode off): Corne
   4000 mm/min, wanaka200 10000 mm/min (the project files). No
   acceleration: a lower bound for the machine's time at the emitted feeds.
It also compares the feed each sample was timed at (feed_rate_mm_min) with
the F of the same move in program.nc."""
import collections, glob, json, math, os, re, sys
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")  # copy of fix11/c39_runtime.py with the fix13 runs
W = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
RUNS = [("corne spiral_pick D25", "fix15/corne_pick", "fix15/corne_pick", "3D Rough 1", 4000.0),
        ("corne spiral_headon D25", "fix15/corne_headon", "fix15/corne_headon", "3D Rough 1", 4000.0),
        ("wanaka200 spiral_pick D25", "fix15/wanaka200_pick", "fix15/wanaka200_pick", "2 Back Rough", 10000.0)]


def op_moves(path, op):
    L = open(path).read().splitlines(); i = L.index(f"({op})") + 1; cmds = []
    for ln in L[i:]:
        t = ln.strip()
        if t.startswith("("):
            if cmds and not t.startswith("(feeds"): break
            continue
        if t and t[0] in "MTS":
            if cmds: break
            continue
        if t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ"): cmds.append(t)
    g, p, f, out = 0, (0.0, 0.0, 0.0), None, []
    for c in cmds:
        w = {k: float(v) for k, v in W.findall(c)}
        g = int(w.get("G", g)); f = w.get("F", f); q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        out.append((g, p, q, w.get("I", 0.0), w.get("J", 0.0), f)); p = q
    return out


def length(m):
    g, a, b, i, j, f = m
    if g in (2, 3):
        cx, cy = a[0] + i, a[1] + j; r = math.hypot(a[0] - cx, a[1] - cy)
        t0, t1 = math.atan2(a[1] - cy, a[0] - cx), math.atan2(b[1] - cy, b[0] - cx)
        sw = ((t0 - t1) if g == 2 else (t1 - t0)) % (2 * math.pi) or 2 * math.pi
        return math.hypot(r * sw, b[2] - a[2])
    return math.dist(a, b)


out = {}
for name, rd, sd, op, rapid in RUNS:
    tp = json.load(open(glob.glob(os.path.join(P3, rd, "tp_1_*.json"))[0])); n = tp["move_count"]
    allm = op_moves(os.path.join(P3, rd, "program.nc"), op); mv = allm[1:n + 1]
    sim = json.load(open(os.path.join(P3, sd, "simulation.json")))
    summ = next(t for t in sim["trace"]["toolpath_summaries"] if t["toolpath_id"] == 1)
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    prog = {"fed": 0.0, "rapid": 0.0}
    for m in allm:   # the whole op section, the surplus G0 included
        L = length(m)
        if m[0] == 0: prog["rapid"] += L / rapid * 60
        else: prog["fed"] += L / m[5] * 60
    # sample feed against program feed, per fed move with samples
    sf = {}
    for s in S:
        sf.setdefault(s["move_index"], s["feed_rate_mm_min"])
    pairs = [(sf[k], m[5]) for k, m in enumerate(mv) if m[0] != 0 and k in sf]
    ratio = sorted(p / s for s, p in pairs if s > 0)
    lower = sum(1 for s, p in pairs if s < p - 1e-6); higher = sum(1 for s, p in pairs if s > p + 1e-6)
    # samples re-timed at the program feed: segment_time * sample feed / program feed
    retimed = 0.0
    for s in S:
        k = s["move_index"]
        if k < n and mv[k][0] != 0 and s["feed_rate_mm_min"] > 0:
            retimed += s["segment_time_s"] * s["feed_rate_mm_min"] / mv[k][5]
        else:
            retimed += s["segment_time_s"]
    # C36's time split, re-timed at the program feed (naive): plunge feed
    # (fed vertical down) against the rest.
    vol = collections.Counter()
    for s_ in S:
        vol[s_["move_index"]] += s_["removed_volume_est_mm3"]
    pl = sum(length(m) / m[5] * 60 for m in mv if m[0] != 0 and math.hypot(m[2][0] - m[1][0], m[2][1] - m[1][1]) < 1e-3 and m[2][2] - m[1][2] < -1e-3)
    r = {"program_plunge_feed_s": round(pl, 1), "summary_runtime_s": round(summ["total_runtime_s"], 1), "summary_cutting_s": round(summ["cutting_runtime_s"], 1),
         "summary_rapid_s": round(summ["rapid_runtime_s"], 1),
         "per_kinematics_sum_s": round(sum(v["cutting_runtime_s"] for v in summ["per_kinematics"].values()), 1),
         "samples_s": round(sum(s["segment_time_s"] for s in S), 1),
         "program_s": round(prog["fed"] + prog["rapid"], 1), "program_fed_s": round(prog["fed"], 1), "program_rapid_s": round(prog["rapid"], 1),
         "samples_retimed_at_program_feed_s": round(retimed, 1),
         "fed_moves_compared": len(pairs), "sample_feed_below_program": lower, "sample_feed_above_program": higher,
         "program_over_sample_feed_median": round(ratio[len(ratio) // 2], 3) if ratio else None,
         "runtime_by_intent_present": summ.get("runtime_by_intent") is not None}
    out[name] = r
    print(name, r)
json.dump(out, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "c39_d25.json"), "w"), indent=1)
