"""C36: re-derive every number of README.md from the raw files with this
script's own reader (it does not import audit.py), compare with
audit_results.json, and check that README.md states each one."""
import collections, glob, json, math, os, re
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, ".."); RES = os.path.join(P3, "..")
EPS = 1e-3
TOK = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
COLS = {"corne": [("contour_parallel", "fix6/corne/contour_parallel", "fix6/corne/contour_parallel"),
                  ("zigzag_pick D18", "fix5/corne/geodesic_local_zigzag_pick", "fix5/corne/geodesic_local_zigzag_pick"),
                  ("zigzag_pick D20", "fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick"),
                  ("AgentSearch", "fix1/corne/agent", "fix2/simart/corne/agent")],
        "wanaka200": [("contour_parallel", "fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel"),
                      ("zigzag_pick D18", "fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag"),
                      ("zigzag_pick D20", "fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick"),
                      ("AgentSearch", "fix2/wanaka200/agent", "fix2/simart/wanaka200/agent")]}
OP = {"corne": "3D Rough 1", "wanaka200": "2 Back Rough"}
NOM = {"corne": 1518.75 * 1.2 * 6.0 / 60, "wanaka200": 1000.0 * 2.0 * 5.46 / 60}
R = json.load(open(os.path.join(H, "audit_results.json")))
readme = open(os.path.join(H, "README.md")).read()
checks = 0


def moves(path, op, n):
    L = open(path).read().splitlines()
    i = L.index(f"({op})") + 1; out = []; g = 0; p = (0.0, 0.0, 0.0)
    for ln in L[i:]:
        t = ln.strip()
        if t.startswith("(") and out and not t.startswith("(feeds"): break
        if t.startswith("("): continue
        if t and t[0] in "MTS":
            if out: break
            continue
        if not (t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ")): continue
        w = {k: float(v) for k, v in TOK.findall(t)}
        g = int(w.get("G", g)); q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        out.append((g, p, q, w.get("I", 0.0), w.get("J", 0.0))); p = q
    assert len(out) - n in (1, 2)
    return out[1:n + 1]


def dirs(m):
    g, a, b, i, j = m
    if g in (2, 3):
        cx, cy = a[0] + i, a[1] + j; r = math.hypot(a[0] - cx, a[1] - cy)
        t0, t1 = math.atan2(a[1] - cy, a[0] - cx), math.atan2(b[1] - cy, b[0] - cx)
        sw = ((t0 - t1) if g == 2 else (t1 - t0)) % (2 * math.pi) or 2 * math.pi
        sg = 1 if g == 3 else -1; slope = (b[2] - a[2]) / (r * sw)
        f = lambda t: (lambda v: tuple(c / math.sqrt(sum(x * x for x in v)) for c in v))((-sg * math.sin(t), sg * math.cos(t), slope))
        return f(t0), f(t1)
    d = [b[k] - a[k] for k in range(3)]; L = math.sqrt(sum(x * x for x in d))
    if L < EPS: return None, None
    u = tuple(x / L for x in d); return u, u


for fx, cols in COLS.items():
    for name, rd, sd in cols:
        key = f"{fx} {name}"; r = R[key]
        tp = json.load(open(glob.glob(os.path.join(P3, rd, "tp_1_*.json"))[0])); n = tp["move_count"]
        mv = moves(os.path.join(P3, rd, "program.nc"), OP[fx], n)
        sim = json.load(open(os.path.join(P3, sd, "simulation.json")))
        q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
        S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
        vol = collections.Counter(); tim = collections.Counter(); first = {}
        for s in S:
            vol[s["move_index"]] += s["removed_volume_est_mm3"]; tim[s["move_index"]] += s["segment_time_s"]; first.setdefault(s["move_index"], s["source_intent"])
        assert all(k < n for k in tim)   # every sample falls on a move of the op
        sty = {}
        for e in tp["semantic_trace"]["items"]:
            if e["kind"] == "entry":
                for k in range(e["move_start"], e["move_end"] + 1): sty[k] = e["params"]["values"]["style"]
        hxy = lambda m: math.hypot(m[2][0] - m[1][0], m[2][1] - m[1][1])
        dz = lambda m: m[2][2] - m[1][2]
        # events
        ev, lab, drops, k = 0, collections.Counter(), [], 0
        while k < n:
            m = mv[k]
            if m[0] != 0 and hxy(m) < EPS and dz(m) < -EPS:
                j = k
                while j + 1 < n and hxy(mv[j + 1]) < EPS and (mv[j + 1][0] == 0 or dz(mv[j + 1]) < -EPS) and abs(mv[j + 1][1][0] - m[1][0]) < EPS and abs(mv[j + 1][1][1] - m[1][1]) < EPS:
                    j += 1
                dn = [i for i in range(k, j + 1) if mv[i][0] != 0 and dz(mv[i]) < -EPS]
                if any(vol[i] > q for i in dn):
                    ev += 1; lab[f"{first.get(dn[0])} / {sty.get(dn[0], 'no entry span')}"] += 1
                    drops.append(round(mv[dn[0]][1][2] - mv[dn[-1]][2][2], 3))
                k = j + 1
            else:
                k += 1
        assert ev == r["descent_events"] and dict(lab) == r["descent_event_labels"], (key, ev, lab)
        assert sorted(drops)[len(drops) // 2] == r["descent_event_drop_median_mm"] and max(drops) == r["descent_event_drop_max_mm"]
        checks += 4
        # junctions
        turns, planar, zr, prev = [], [], 0, None
        for m in mv:
            if m[0] == 0: prev = None; continue
            a0, a1 = dirs(m)
            if a0 is None: continue
            if prev is not None:
                c = max(-1.0, min(1.0, sum(x * y for x, y in zip(prev[0], a0)))); t = math.degrees(math.acos(c)); turns.append(t)
                if prev[2] and hxy(m) >= EPS: planar.append(t)
                if abs(dz(m)) > EPS and abs(prev[1]) > EPS and dz(m) * prev[1] < 0: zr += 1
            prev = (a1, dz(m), hxy(m) >= EPS)
        rev = sum(1 for t in turns if t > 90 + 1e-6); r90 = sum(1 for t in turns if abs(t - 90) <= 1e-6)
        prev_ = sum(1 for t in planar if t > 90 + 1e-6); big = sum(1 for t in planar if t > 30 + 1e-6)
        assert (len(turns), rev, r90, zr, len(planar), prev_) == (r["junctions_at_feed"], r["reversals_over_90deg"], r["turns_exactly_90deg"], r["z_reversals"], r["planar_junctions"], r["planar_reversals"]), key
        assert big == sum(r["planar_turn_hist"][4:]), key
        checks += 7
        # MRR
        sm = sorted((s["removed_volume_est_mm3"] / s["segment_time_s"], s["segment_time_s"], s["move_index"]) for s in S if s["segment_time_s"] > 0)
        T = sum(x[1] for x in sm)
        def qt(f):
            acc = 0.0
            for v, dt, _ in sm:
                acc += dt
                if acc >= f * T: return v
        assert (round(qt(0.5), 1), round(qt(0.95), 1), round(qt(0.99), 1), round(sm[-1][0], 1)) == (r["mrr_p50"], r["mrr_p95"], r["mrr_p99"], r["mrr_max"]), key
        assert round(sum(dt for v, dt, _ in sm if v > NOM[fx]), 1) == r["time_over_nominal_s"] and round(sum(dt for v, dt, _ in sm if v > 2 * NOM[fx]), 1) == r["time_over_2x_nominal_s"]
        mm = mv[sm[-1][2]]
        assert mm[0] != 0 and hxy(mm) >= EPS, (key, "the max-MRR sample is not on a fed XY move")   # finding 4
        checks += 7
        # time split
        sp = collections.Counter()
        for k, m in enumerate(mv):
            c = "rapid" if m[0] == 0 else ("plunge feed" if hxy(m) < EPS and dz(m) < -EPS else ("lift feed" if hxy(m) < EPS and dz(m) > EPS else ("cut" if vol[k] > q else "fed in air")))
            sp[c] += tim[k]
        assert {k: round(v, 1) for k, v in sp.items() if True} == {k: v for k, v in r["time_split_s"].items()}, key
        assert abs(sum(sp.values()) - sum(s["segment_time_s"] for s in S)) < 1e-6
        rt = next(t["total_runtime_s"] for t in sim["trace"]["toolpath_summaries"] if t["toolpath_id"] == 1)
        assert round(rt, 1) == r["sim_runtime_s"]
        checks += 7
        # README rows
        L = lambda kk: lab.get(kk, 0)
        for needle in (f"| {key} | {ev} | {L('entry_plunge / plunge entry')} / {L('entry_plunge / keep-down link')} / {L('linking / keep-down link')} ",
                       f"| {key} | {len(turns)} | {rev} | {r90} | {zr} | {len(planar)} | {prev_} | {big} ({100 * big / len(planar):.1f} %) |",
                       f"| {key} | {round(qt(0.5), 1)} / {round(qt(0.95), 1)} / {round(qt(0.99), 1)} / {round(sm[-1][0], 1)} |"):
            assert needle in readme, needle
            checks += 1
        tot = round(sum(sp.values()), 1)
        for c in ("cut", "fed in air", "lift feed", "plunge feed", "rapid"):
            assert f"{round(sp.get(c, 0.0), 1)} ({100 * round(sp.get(c, 0.0), 1) / tot:.1f} %)" in readme, (key, c)
            checks += 1
        R[key]["_lab"] = lab; R[key]["_share"] = {c: 100 * round(sp.get(c, 0.0), 1) / tot for c in sp}; R[key]["_big"] = 100 * big / len(planar)

# Findings text.
C, W = "corne", "wanaka200"
need = []
names = [("contour_parallel", "contour_parallel"), ("zigzag_pick D18", "D18"), ("zigzag_pick D20", "D20"), ("AgentSearch", "AgentSearch")]
need.append("Corne contour_parallel {}, D18 {}, D20 {}, AgentSearch {};".format(*[R[f"{C} {k}"]["_lab"].get("linking / keep-down link", 0) for k, _ in names]))
need.append("contour_parallel {}, D18 {}, D20 {}, AgentSearch {}.".format(*[R[f"{W} {k}"]["_lab"].get("linking / keep-down link", 0) for k, _ in names]))
d20, cp = R[f"{C} zigzag_pick D20"], R[f"{C} contour_parallel"]; w20, wcp = R[f"{W} zigzag_pick D20"], R[f"{W} contour_parallel"]
need += [f"Plunge feed is {d20['_share']['plunge feed']:.1f} %", f"({d20['time_split_s']['plunge feed']} s), against {cp['_share']['plunge feed']:.1f} % for contour_parallel ({cp['time_split_s']['plunge feed']} s)",
         f"On wanaka200 D20 it is {w20['_share']['plunge feed']:.1f} % ({w20['time_split_s']['plunge feed']} s)", f"against\n   14.3 % for contour_parallel" if False else f"{wcp['_share']['plunge feed']:.1f} % for contour_parallel",
         f"Corne: contour_parallel {cp['_big']:.1f} %, D20 {d20['_big']:.1f} %", f"wanaka200: contour_parallel {wcp['_big']:.1f} %, D20 {w20['_big']:.1f} %",
         f"Planar reversals: Corne {cp['planar_reversals']} against {d20['planar_reversals']}; wanaka200 {wcp['planar_reversals']} against {w20['planar_reversals']}",
         f"D20 has {d20['turns_exactly_90deg']} turns of exactly 90 deg", f"on Corne (contour_parallel {cp['turns_exactly_90deg']})",
         f"contour_parallel\n   {cp['time_over_2x_nominal_s']} s, D20 {d20['time_over_2x_nominal_s']} s" if False else f"{cp['time_over_2x_nominal_s']} s, D20 {d20['time_over_2x_nominal_s']} s",
         f"{wcp['time_over_2x_nominal_s']} s against {w20['time_over_2x_nominal_s']} s"]
for k, short in names:
    rr = R[f"{W} {k}"]
    need.append(f"{short} {rr['sample_time_s']} against {rr['sim_runtime_s']} s")
    assert rr["sample_time_s"] > rr["sim_runtime_s"]
    cc = R[f"{C} {k}"]; assert cc["sample_time_s"] == cc["sim_runtime_s"]
    checks += 2
need += [f"The vertical descent events of Corne D20 are {d20['descent_events']}", f"On wanaka200 D20 the audit counts {w20['descent_events']}"]
E = json.load(open(os.path.join(P3, "fix9", "entries.json")))
for fxk, ek in ((C, "corne zigzag_pick D20"), (W, "wanaka200 zigzag_pick D20")):
    rows = E[ek]["rows"]
    kd = sum(1 for x in rows if x["style"] == "keep-down link" and x["descent_mm"] >= 1.0 and x["removed_mm3"] > 0.1)
    pe = sum(1 for x in rows if x["style"] == "plunge entry" and x["descent_mm"] >= 1.0 and x["removed_mm3"] > 0.1)
    need.append(f"{kd} + {pe}")
for nd in need:
    nd2 = " ".join(nd.split())
    assert nd2 in " ".join(readme.split()), ("README.md does not state", nd)
    checks += 1
print(f"fix11: all {checks} checks hold")
