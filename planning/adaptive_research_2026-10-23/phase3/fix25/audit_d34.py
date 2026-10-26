"""D34: a copy of fix11/audit.py. One change: an op section with exactly
move_count lines (a later op: no move-0 line) maps line k to move k + 1.

C36: a kinematic audit of one rough op that trusts no intent label.

Usage:
  python3 -I audit.py <run_dir> [--sim <dir>] [--op "3D Rough 1"] [--tp tp_1_*.json]
          [--nominal-mrr <mm3/s>] [--json out.json]

Inputs: <run_dir>/program.nc (the motion), <run_dir>/tp_*.json (only its
move_count, to map G-code lines to move indices), and <sim>/simulation.json
(per sample: move_index, segment_time_s, removed_volume_est_mm3). The
labels (source_intent, span styles) are read only to print a cross-table
beside the predicates; no count depends on them.

Every threshold is derived:
- GCODE_EPS = 1e-3 mm: the post writes X, Y and Z with 3 decimals, so two
  coordinates that differ by less than one output step are the same point.
  A move is vertical when its XY travel is under GCODE_EPS; it goes down
  when its Z drops by more than GCODE_EPS.
- VOLUME_QUANTUM = cell^2 * sample_step (simulation.json resolution_mm and
  sample_step_mm): the volume of one dexel column of one cell, swept over
  one sample step. It is the smallest removal the trace resolves; a move
  whose samples remove more is cutting stock. Corne: 0.3^2 * 0.3 =
  0.027 mm3; wanaka200: 1.0^2 * 1.0 = 1.0 mm3.
- Reversal: a junction between two fed moves whose 3D turn is over 90 deg.
  Past 90 deg the velocity component along the incoming direction changes
  sign, so the machine must stop that motion at the junction whatever its
  acceleration or junction deviation. The turn-angle histogram has fixed
  bin edges for reading only; no count uses them. The machine profiles in
  the project files give no acceleration and no junction deviation, so the
  audit predicts no junction speed.
- Z reversal: two fed moves in a row whose Z changes have opposite signs,
  each over GCODE_EPS.
- MRR: removed_volume_est_mm3 / segment_time_s per sample, weighted by the
  sample's time. The nominal MRR (feed_rate * stepover * depth_per_pass of
  the project) is given on the command line and is only a reference line.
- Time split (sample time per move, by motion):
    rapid       G0;
    plunge feed fed, vertical, going down;
    lift feed   fed, vertical, going up;
    cut         fed, not vertical, removes more than VOLUME_QUANTUM;
    fed in air  fed, not vertical, removes VOLUME_QUANTUM or less (links at
                depth and air cuts).
"""
import argparse, collections, glob, json, math, os, re, sys

GCODE_EPS = 1e-3
WORD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")
TURN_BINS = [0, 1, 5, 15, 30, 45, 60, 90, 135, 180]
MRR_BINS = [0, 1e-9, 25, 50, 100, 200, 400, 800, 1600, float("inf")]


def op_moves(path, comment):
    lines = open(path).read().splitlines()
    i = lines.index(f"({comment})") + 1
    cmds = []
    for ln in lines[i:]:
        t = ln.strip()
        if t.startswith("("):
            if cmds and not t.startswith("(feeds"):
                break
            continue
        if t and t[0] in "MTS":
            if cmds:
                break
            continue
        if t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ"):
            cmds.append(t)
    g, pos, f, out = 0, (0.0, 0.0, 0.0), None, []
    for c in cmds:
        w = {k: float(v) for k, v in WORD.findall(c)}
        g = int(w.get("G", g)); f = w.get("F", f)
        q = (w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2]))
        out.append({"g": g, "a": pos, "b": q, "i": w.get("I", 0.0), "j": w.get("J", 0.0), "f": f})
        pos = q
    return out


def tangents(m):
    """Unit 3D direction at the start and at the end of a move (arcs: the
    tangent of the circle; Z changes linearly along the arc)."""
    a, b = m["a"], m["b"]
    if m["g"] in (2, 3):
        cx, cy = a[0] + m["i"], a[1] + m["j"]
        rad = math.hypot(a[0] - cx, a[1] - cy)
        a0 = math.atan2(a[1] - cy, a[0] - cx); a1 = math.atan2(b[1] - cy, b[0] - cx)
        sw = ((a0 - a1) if m["g"] == 2 else (a1 - a0)) % (2 * math.pi) or 2 * math.pi
        s = -1 if m["g"] == 2 else 1
        L = rad * sw; dz = (b[2] - a[2]) / L if L > 0 else 0.0

        def t(ang):
            v = (-s * math.sin(ang), s * math.cos(ang), dz)
            n = math.sqrt(v[0] ** 2 + v[1] ** 2 + v[2] ** 2)
            return tuple(c / n for c in v)
        return t(a0), t(a1)
    d = (b[0] - a[0], b[1] - a[1], b[2] - a[2]); n = math.sqrt(sum(c * c for c in d))
    if n < GCODE_EPS:
        return None, None
    u = tuple(c / n for c in d)
    return u, u


def audit(run_dir, sim_dir, op, tp_glob, nominal_mrr):
    tpf = sorted(glob.glob(os.path.join(run_dir, tp_glob)))[0]
    tp = json.load(open(tpf)); n = tp["move_count"]; tpid = int(re.search(r"tp_(\d+)_", os.path.basename(tpf)).group(1))
    allm = op_moves(os.path.join(run_dir, "program.nc"), op)
    assert len(allm) - n in (0, 1, 2), (len(allm), n)
    if len(allm) == n:   # D34: a later op's section has no move-0 line; its line 0 is move 1 (checked on fix25 body_cp/corne tp 5)
        mv = [allm[0]] + allm[:n - 1]
    else:
        mv = allm[1:n + 1]                      # move i = command line i + 1 of the op section
    sim = json.load(open(os.path.join(sim_dir, "simulation.json")))
    cell, step = sim["resolution_mm"], sim["sample_step_mm"]
    quantum = cell * cell * step
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == tpid]
    vol = collections.defaultdict(float); tim = collections.defaultdict(float); lab = {}
    for s in S:
        vol[s["move_index"]] += s["removed_volume_est_mm3"]; tim[s["move_index"]] += s["segment_time_s"]
        lab.setdefault(s["move_index"], s["source_intent"])
    styles = {}
    for e in tp["semantic_trace"]["items"]:
        if e["kind"] == "entry":
            for k in range(e["move_start"], e["move_end"] + 1):
                styles[k] = e["params"]["values"]["style"]

    def xy(m):
        return math.hypot(m["b"][0] - m["a"][0], m["b"][1] - m["a"][1])

    # 1. Descents into stock.
    desc = []
    for k, m in enumerate(mv):
        dz = m["b"][2] - m["a"][2]
        if m["g"] != 0 and dz < -GCODE_EPS and vol[k] > quantum:
            desc.append({"move": k, "vertical": xy(m) < GCODE_EPS, "drop_mm": round(-dz, 3), "end_z": round(m["b"][2], 3),
                         "removed_mm3": round(vol[k], 3), "time_s": round(tim[k], 4), "feed": m["f"],
                         "label": f"{lab.get(k)} / {styles.get(k, 'no entry span')}"})
    vert = [d for d in desc if d["vertical"]]
    # A descent event: a run of vertical moves at one XY (fed downs and the
    # G0 pecks between them) with at least one fed down that cuts stock.
    events, k = [], 0
    while k < len(mv):
        m = mv[k]
        if m["g"] != 0 and xy(m) < GCODE_EPS and m["b"][2] - m["a"][2] < -GCODE_EPS:
            j = k
            while j + 1 < len(mv) and xy(mv[j + 1]) < GCODE_EPS and (mv[j + 1]["g"] == 0 or mv[j + 1]["b"][2] - mv[j + 1]["a"][2] < -GCODE_EPS) \
                    and abs(mv[j + 1]["a"][0] - m["a"][0]) < GCODE_EPS and abs(mv[j + 1]["a"][1] - m["a"][1]) < GCODE_EPS:
                j += 1
            downs = [i for i in range(k, j + 1) if mv[i]["g"] != 0 and mv[i]["b"][2] - mv[i]["a"][2] < -GCODE_EPS]
            if any(vol[i] > quantum for i in downs):
                events.append({"first_move": k, "drop_mm": round(mv[downs[0]]["a"][2] - mv[downs[-1]]["b"][2], 3),
                               "removed_mm3": round(sum(vol[i] for i in downs), 3), "time_s": round(sum(tim[i] for i in range(k, j + 1)), 4),
                               "label": f"{lab.get(downs[0])} / {styles.get(downs[0], 'no entry span')}"})
            k = j + 1
        else:
            k += 1
    # 2. Junctions at feed.
    turns, zrev, prev = [], 0, None
    planar = []   # junctions whose two moves both travel in XY (no vertical move)
    for k, m in enumerate(mv):
        if m["g"] == 0:
            prev = None
            continue
        t0, t1 = tangents(m)
        if t0 is None:
            continue
        if prev is not None:
            p, pdz = prev
            c = max(-1.0, min(1.0, sum(x * y for x, y in zip(p, t0))))
            turns.append(math.degrees(math.acos(c)))
            if prev_xy and xy(m) >= GCODE_EPS:
                planar.append(turns[-1])
            dz = m["b"][2] - m["a"][2]
            if abs(dz) > GCODE_EPS and abs(pdz) > GCODE_EPS and dz * pdz < 0:
                zrev += 1
        prev = (t1, m["b"][2] - m["a"][2]); prev_xy = xy(m) >= GCODE_EPS
    # bins (lo, hi], the first [0, 1]; a turn within 1e-6 deg of a bin edge
    # (90 deg: horizontal to vertical) counts in the lower bin
    def bin_of(t):
        for i in range(len(TURN_BINS) - 1):
            if t <= TURN_BINS[i + 1] + 1e-6:
                return i
        return len(TURN_BINS) - 2
    hist = [0] * (len(TURN_BINS) - 1)
    for t in turns:
        hist[bin_of(t)] += 1
    rev = sum(1 for t in turns if t > 90.0 + 1e-6)
    right = sum(1 for t in turns if abs(t - 90.0) <= 1e-6)
    planar_rev = sum(1 for t in planar if t > 90.0 + 1e-6)
    planar_hist = [0] * (len(TURN_BINS) - 1)
    for t in planar:
        planar_hist[bin_of(t)] += 1
    # 3. MRR over time.
    mrr_t = [0.0] * (len(MRR_BINS) - 1); tot_t = 0.0; samples_mrr = []
    for s in S:
        dt = s["segment_time_s"]
        if dt <= 0:
            continue
        r = s["removed_volume_est_mm3"] / dt; tot_t += dt; samples_mrr.append((r, dt))
        for i in range(len(MRR_BINS) - 1):
            if MRR_BINS[i] <= r < MRR_BINS[i + 1] or (i == 0 and r == 0):
                mrr_t[i] += dt
                break
    samples_mrr.sort()

    def q(f):
        acc = 0.0; tgt = f * tot_t
        for r, dt in samples_mrr:
            acc += dt
            if acc >= tgt:
                return r
        return samples_mrr[-1][0]
    over_nom = sum(dt for r, dt in samples_mrr if nominal_mrr and r > nominal_mrr)
    over_2nom = sum(dt for r, dt in samples_mrr if nominal_mrr and r > 2 * nominal_mrr)
    # 4. Time split by motion.
    split = collections.defaultdict(float)
    for k, m in enumerate(mv):
        dz = m["b"][2] - m["a"][2]
        if m["g"] == 0:
            c = "rapid"
        elif xy(m) < GCODE_EPS and dz < -GCODE_EPS:
            c = "plunge feed"
        elif xy(m) < GCODE_EPS and dz > GCODE_EPS:
            c = "lift feed"
        elif vol[k] > quantum:
            c = "cut"
        else:
            c = "fed in air"
        split[c] += tim[k]
    runtime = next(t["total_runtime_s"] for t in sim["trace"]["toolpath_summaries"] if t["toolpath_id"] == tpid)
    return {
        "run": run_dir, "op": op, "moves": n, "sim_cell_mm": cell, "sample_step_mm": step, "volume_quantum_mm3": round(quantum, 4),
        "descents_into_stock": len(desc), "vertical_descents_into_stock": len(vert),
        "descent_events": len(events), "descent_event_labels": dict(collections.Counter(e["label"] for e in events)),
        "descent_event_drop_median_mm": round(sorted(e["drop_mm"] for e in events)[len(events) // 2], 3) if events else None,
        "descent_event_drop_max_mm": max((e["drop_mm"] for e in events), default=None),
        "descent_event_time_s": round(sum(e["time_s"] for e in events), 1),
        "vertical_descent_removed_mm3": round(sum(d["removed_mm3"] for d in vert), 1),
        "vertical_descent_drop_median_mm": round(sorted(d["drop_mm"] for d in vert)[len(vert) // 2], 3) if vert else None,
        "vertical_descent_drop_max_mm": max((d["drop_mm"] for d in vert), default=None),
        "descent_labels": dict(collections.Counter(d["label"] for d in vert)),
        "junctions_at_feed": len(turns), "reversals_over_90deg": rev, "turns_exactly_90deg": right,
        "planar_junctions": len(planar), "planar_reversals": planar_rev, "planar_turn_hist": planar_hist, "z_reversals": zrev,
        "turn_hist_edges_deg": TURN_BINS, "turn_hist": hist,
        "mrr_bins_mm3_s": [b if b != float("inf") else "inf" for b in MRR_BINS], "mrr_time_s": [round(x, 1) for x in mrr_t],
        "mrr_p50": round(q(0.5), 1), "mrr_p95": round(q(0.95), 1), "mrr_p99": round(q(0.99), 1), "mrr_max": round(samples_mrr[-1][0], 1),
        "nominal_mrr": nominal_mrr, "time_over_nominal_s": round(over_nom, 1), "time_over_2x_nominal_s": round(over_2nom, 1),
        "time_split_s": {k: round(v, 1) for k, v in sorted(split.items())}, "sample_time_s": round(sum(split.values()), 1),
        "sim_runtime_s": round(runtime, 1), "descents": desc,
    }


def text(r):
    out = [f"== {r['run']} '{r['op']}': {r['moves']} moves; sim cell {r['sim_cell_mm']} mm, sample step {r['sample_step_mm']} mm, volume quantum {r['volume_quantum_mm3']} mm3",
           f"1. descent events into stock (a vertical fed descent at one XY, pecks included, that removes > quantum): {r['descent_events']} "
           f"(drop median {r['descent_event_drop_median_mm']} mm, max {r['descent_event_drop_max_mm']} mm, {r['descent_event_time_s']} s); labels {r['descent_event_labels']}",
           f"   moves: descents into stock (fed, Z down, removes > quantum) {r['descents_into_stock']}; vertical {r['vertical_descents_into_stock']} "
           f"(removed {r['vertical_descent_removed_mm3']} mm3, drop median {r['vertical_descent_drop_median_mm']} mm, max {r['vertical_descent_drop_max_mm']} mm)",
           f"   labels of the vertical descents (intent / span style): {r['descent_labels']}",
           f"   planar junctions (both moves travel in XY) {r['planar_junctions']}; planar reversals {r['planar_reversals']}; planar histogram {r['planar_turn_hist']}",
           f"2. junctions at feed {r['junctions_at_feed']}; reversals (turn > 90 deg) {r['reversals_over_90deg']}; turns of exactly 90 deg {r['turns_exactly_90deg']}; Z reversals {r['z_reversals']}",
           "   turn histogram (deg, bins (lo, hi]): " + ", ".join(f"({a},{b}] {c}" for a, b, c in zip(r["turn_hist_edges_deg"][:-1], r["turn_hist_edges_deg"][1:], r["turn_hist"])),
           "3. MRR time histogram (mm3/s): " + ", ".join(f"[{a},{b}) {t} s" for a, b, t in zip(r["mrr_bins_mm3_s"][:-1], r["mrr_bins_mm3_s"][1:], r["mrr_time_s"]))
           + f"; p50 {r['mrr_p50']}, p95 {r['mrr_p95']}, p99 {r['mrr_p99']}, max {r['mrr_max']}; nominal {r['nominal_mrr']}: {r['time_over_nominal_s']} s over, {r['time_over_2x_nominal_s']} s over 2x",
           f"4. time split (s): {r['time_split_s']}; sum {r['sample_time_s']} of sim runtime {r['sim_runtime_s']}"]
    return "\n".join(out)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("run"); ap.add_argument("--sim"); ap.add_argument("--op", default="3D Rough 1"); ap.add_argument("--tp", default="tp_1_*.json")
    ap.add_argument("--nominal-mrr", type=float, default=None); ap.add_argument("--json"); ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    r = audit(a.run, a.sim or a.run, a.op, a.tp, a.nominal_mrr)
    print(text(r))
    if a.list:
        for d in r["descents"]:
            print("   ", d)
    if a.json:
        json.dump(r, open(a.json, "w"), indent=1)
