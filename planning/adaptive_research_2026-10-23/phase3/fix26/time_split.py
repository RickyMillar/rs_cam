"""D35: the time split of each op of a run, from simulation.json (the trace
samples' segment_time_s), by the sample labels:
  cut          : clearing_cut samples that are not plunges (linear, arc, helix)
  plunge/ramp  : entry_plunge samples, and linking samples of plunge kinematics
  link at feed : linking samples at feed that are not plunges (linear, helix, arc)
  rapid        : rapid kinematics (linking and retract)
  other        : anything else (printed by label when non-zero)
The helix part of 'cut' and its feed are printed apart: the copy op cuts
mostly on helices. Descents come from the label-free audit (fix25/audit_d34.py
JSON, descent_events) when its file is given.
Usage: time_split.py <run dir under phase3> <tp ids> [--audit tp=path ...]"""
import collections, json, sys
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent
run = sys.argv[1]; tps = [int(x) for x in sys.argv[2].split(",")]
aud = dict(a.split("=", 1) for a in sys.argv[3:])
t = json.load(open(P3 / run / "simulation.json"))["trace"]
acc = {tp: collections.Counter() for tp in tps}; hf = {tp: collections.Counter() for tp in tps}
for s in t["samples"]:
    tp = s["toolpath_id"]
    if tp not in acc:
        continue
    si = s["source_intent"] if isinstance(s["source_intent"], str) else json.dumps(s["source_intent"])
    k = s["cut_kinematics"]; dt = s["segment_time_s"]
    if k == "rapid":
        acc[tp]["rapid"] += dt
    elif si == "entry_plunge" or (si == "linking" and k == "plunge"):
        acc[tp]["plunge/ramp"] += dt
    elif si == "clearing_cut":
        acc[tp]["cut"] += dt
        if k == "helix":
            acc[tp]["cut: helix"] += dt; hf[tp][round(s["feed_rate_mm_min"])] += dt
    elif si == "linking":
        acc[tp]["link at feed"] += dt
    else:
        acc[tp][f"other {si}/{k}"] += dt
summ = {x["toolpath_id"]: x for x in t["toolpath_summaries"]}
out = {}
for tp in tps:
    r = {k: round(v, 1) for k, v in acc[tp].items()}
    r["sum"] = round(sum(v for k, v in acc[tp].items() if not k.startswith("cut: ")), 1)
    r["summary_runtime_s"] = round(summ[tp]["total_runtime_s"], 1)
    r["helix_cut_time_by_feed"] = {f: round(v, 1) for f, v in hf[tp].most_common(4)}
    if str(tp) in aud:
        a = json.load(open(aud[str(tp)])); r["descents (audit)"] = a["descent_events"]; r["audit_split_s"] = a["time_split_s"]
    out[tp] = r
    print(run, "tp", tp, json.dumps(r))
json.dump(out, open(P3 / "fix26" / f"time_split_{run.replace('/', '_')}.json", "w"), indent=1)
