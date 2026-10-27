"""D35: the three clocks of one op, split by move class.
  samples : the trace samples' segment_time_s (the commanded, pre-modulation
            feed; no acceleration; simulation_cut/reporting.rs:19-23).
  program : program.nc at its own F (fed) and the post's rapid rate (G0),
            no acceleration (fix25/c39_d34.py's op_moves / length).
  summary : trace.toolpath_summaries[tp].runtime_by_intent, the
            kinematics integrator (machine/kinematics.rs:1177
            compute_cycle_time_breakdown; the modulated feed and the
            machine's acceleration and junction deviation).
Move class: the integrator's buckets (kinematics.rs:1005-1020): rapid (any
G0), cutting (clearing_cut), entry (entry_plunge / helix / ramp), linking,
retract. A program move takes the source_intent of its first trace sample.
Usage: three_clocks.py <run> <tp> <op comment> <rapid mm/min>"""
import collections, glob, importlib.util, json, os, sys
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("c39", P3 / "fix26" / "c39_lib.py"); C = importlib.util.module_from_spec(spec); spec.loader.exec_module(C)
run, tp, op, rapid = sys.argv[1], int(sys.argv[2]), sys.argv[3], float(sys.argv[4])
d = P3 / run
n = json.load(open(glob.glob(str(d / f"tp_{tp}_*.json"))[0]))["move_count"]
allm = C.op_moves(str(d / "program.nc"), op)
mv = allm[:n] if len(allm) == n else allm[1:n + 1]
t = json.load(open(d / "simulation.json"))["trace"]
summ = next(x for x in t["toolpath_summaries"] if x["toolpath_id"] == tp)
BUCKET = {"clearing_cut": "cutting", "finishing_cut": "cutting", "entry_plunge": "entry", "entry_helix": "entry", "entry_ramp": "entry",
          "linking": "linking", "retract": "retract"}
first = {}; samp = collections.Counter()
for s in t["samples"]:
    if s["toolpath_id"] != tp:
        continue
    si = s["source_intent"] if isinstance(s["source_intent"], str) else "unknown"
    cls = "rapid" if s["cut_kinematics"] == "rapid" else BUCKET.get(si, "unknown")
    samp[cls] += s["segment_time_s"]; first.setdefault(s["move_index"], si)
prog = collections.Counter(); nmv = collections.Counter(); short = collections.Counter()
for k, m in enumerate(mv):
    L = C.length(m)
    cls = "rapid" if m[0] == 0 else BUCKET.get(first.get(k, "unknown"), "unknown")
    prog[cls] += (L / rapid * 60) if m[0] == 0 else (L / m[5] * 60 if m[5] else 0.0)
    nmv[cls] += 1; short[cls] += L < 1.0
for m in allm[n:]:   # surplus lines after the mapped moves (the post's closing G0)
    if m[0] == 0: prog["rapid"] += C.length(m) / rapid * 60
rb = summ.get("runtime_by_intent") or {}
sumc = {"rapid": rb.get("rapid_s", 0.0), "cutting": rb.get("cutting_s", 0.0), "entry": rb.get("entry_s", 0.0), "linking": rb.get("linking_s", 0.0),
        "retract": rb.get("retract_s", 0.0), "unknown": rb.get("unknown_s", 0.0)}
out = {"run": run, "tp": tp, "summary_total_s": round(summ["total_runtime_s"], 1), "samples_total_s": round(sum(samp.values()), 1),
       "program_total_s": round(sum(prog.values()), 1), "classes": {}}
for c in ("cutting", "entry", "linking", "rapid", "retract", "unknown"):
    if samp[c] or prog[c] or sumc[c]:
        out["classes"][c] = {"moves": nmv[c], "moves_under_1mm": short[c], "samples_s": round(samp[c], 1), "program_s": round(prog[c], 1), "summary_s": round(sumc[c], 1)}
print(json.dumps(out))
json.dump(out, open(P3 / "fix26" / f"three_clocks_{run.replace('/', '_')}_tp{tp}.json", "w"), indent=1)
