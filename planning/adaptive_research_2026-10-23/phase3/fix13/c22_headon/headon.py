"""C22 §13.9: Corne op 1, D23 spiral_headon against D23 spiral_pick (both the D23 build).
Reads fix13/corne and fix13/corne_pick (simulation.json, tp_1, stdout_plain.txt). Writes headon.json."""
import json, os, math, re, collections, bisect
H = os.path.dirname(os.path.abspath(__file__)); F13 = os.path.join(H, "..")
RUNS = {"headon": "corne", "pick": "corne_pick"}
PLANES = [17.0, 11.0, 5.0, 0.5]
R = 3.0
def load(run):
    d = os.path.join(F13, run)
    sim = json.load(open(os.path.join(d, "simulation.json")))
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    tp = json.load(open(os.path.join(d, [f for f in os.listdir(d) if f.startswith("tp_1_")][0])))
    ent = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"])
    return S, ent
def level_of(z):
    c = [p for p in PLANES if p <= z + 0.35]
    return max(c) if c else None
def reentries(run):
    out = []
    for ln in open(os.path.join(F13, run, "stdout_plain.txt")):
        if "generate_toolpath{index=0}" in ln and "C37 re-entry" in ln:
            kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C37 re-entry", 1)[1]))
            out.append((float(kv["z"]), float(kv["x"]), float(kv["y"]), kv["join"]))
    return out
OUT = {}
for name, run in RUNS.items():
    S, ent = load(run)
    starts = [a for a, *_ in ent]
    by = collections.defaultdict(list)
    for s in S:
        if s["source_intent"] != "entry_plunge": continue
        k = bisect.bisect_right(starts, s["move_index"]) - 1
        if k >= 0 and ent[k][0] <= s["move_index"] <= ent[k][1]: by[ent[k]].append(s)
    mi = [s["move_index"] for s in S]
    D = []
    for sp, ss in by.items():
        z = [s["position"][2] for s in ss]
        if sp[2] != "keep-down link" or max(z) - z[-1] < 1.0: continue
        removing = sum(s["removed_volume_est_mm3"] for s in ss) > 0.1
        i = bisect.bisect_left(mi, sp[0]) - 1
        while i >= 0 and S[i]["source_intent"] in ("entry_plunge", "retract"): i -= 1
        before = S[i]["source_intent"] if i >= 0 else None
        end = ss[-1]["position"]
        D.append({"x": end[0], "y": end[1], "z": end[2], "removing": removing, "before": before,
                  "on_plane": any(abs(end[2] - p) <= 0.35 for p in PLANES), "level": level_of(end[2])})
    # move-junction turns over 30 deg, from the last sample of each move (XY), by the intents of the two moves
    last = {}
    for s in S: last[s["move_index"]] = s
    ms = sorted(last)
    turns = collections.Counter(); prev_dir = None; prev_m = None
    for a, b in zip(ms, ms[1:]):
        pa, pb = last[a]["position"], last[b]["position"]
        dx, dy = pb[0] - pa[0], pb[1] - pa[1]
        if math.hypot(dx, dy) < 1e-6 or abs(pb[2] - pa[2]) > 1e-3: prev_dir = None; continue
        d = math.atan2(dy, dx)
        if prev_dir is not None:
            ang = abs((d - prev_dir + math.pi) % (2 * math.pi) - math.pi)
            if ang > math.radians(30):
                turns[(last[a]["source_intent"], last[b]["source_intent"])] += 1
        prev_dir = d
    rec = {"descents_1mm": len(D), "removing": sum(r["removing"] for r in D),
           "removing_by_before": dict(collections.Counter(r["before"] for r in D if r["removing"])),
           "air_by_before": dict(collections.Counter(r["before"] for r in D if not r["removing"])),
           "removing_on_plane": sum(1 for r in D if r["removing"] and r["on_plane"]),
           "moves": len(ms), "moves_by_intent": dict(collections.Counter(last[m]["source_intent"] for m in ms)),
           "turns_over_30": dict((f"{a} -> {b}", n) for (a, b), n in turns.most_common()),
           "linking_xy_mm": round(sum(math.hypot(last[b]["position"][0]-last[a]["position"][0], last[b]["position"][1]-last[a]["position"][1])
                                      for a, b in zip(ms, ms[1:]) if last[b]["source_intent"] == "linking"), 1)}
    if name == "headon":
        RE = reentries(run)
        def near(r):
            return any(abs(z - r["level"]) < 0.4 and math.hypot(x - r["x"], y - r["y"]) <= R for z, x, y, j in RE) if r["level"] is not None else False
        rec["reentries_2d"] = len(RE)
        rec["removing_at_a_2d_reentry"] = sum(1 for r in D if r["removing"] and near(r))
    OUT[name] = rec
    print(name, json.dumps(rec, indent=1))
json.dump(OUT, open(os.path.join(H, "headon.json"), "w"), indent=1)
