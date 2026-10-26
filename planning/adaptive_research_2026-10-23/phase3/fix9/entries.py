"""C18/fix9: what the red rings of render.py mark. For each column, every
entry span of the trace (semantic_trace 'entry' items) and its vertical
descents (sim samples with intent entry_plunge): the span style, the depth
the tool descends in the span (highest entry_plunge sample Z minus the last
one), the stock that descent removes (removed_volume_est_mm3), and the plane
it ends on. Writes entries.json."""
import collections, json, os
import numpy as np
H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
COLS = {"corne": {"contour_parallel": ("fix6/corne/contour_parallel", "fix6/corne/contour_parallel"),
                  "zigzag_pick D18": ("fix5/corne/geodesic_local_zigzag_pick", "fix5/corne/geodesic_local_zigzag_pick"),
                  "zigzag_pick D20": ("fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick"),
                  "AgentSearch": ("fix1/corne/agent", "fix2/simart/corne/agent")},
        "wanaka200": {"contour_parallel": ("fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel"),
                      "zigzag_pick D18": ("fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag"),
                      "zigzag_pick D20": ("fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick"),
                      "AgentSearch": ("fix2/wanaka200/agent", "fix2/simart/wanaka200/agent")}}
PLANES = {"corne": [17.0, 11.0, 5.0, 0.5], "wanaka200": [19.54, 14.08, 8.62, 4.0]}
out = {}
for fx, cols in COLS.items():
    for col, (rd, sd) in cols.items():
        tp = json.load(open(os.path.join(P3, rd, [f for f in os.listdir(os.path.join(P3, rd)) if f.startswith("tp_1_")][0])))
        spans = sorted([(e["move_start"], e["move_end"], e["params"]["values"]["style"]) for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"])
        S = [s for s in json.load(open(os.path.join(P3, sd, "simulation.json")))["trace"]["samples"] if s["toolpath_id"] == 1]
        byspan = collections.defaultdict(list)
        k = 0
        for s in S:
            if s["source_intent"] != "entry_plunge":
                continue
            m = s["move_index"]
            sp = next((x for x in spans if x[0] <= m <= x[1]), None)
            byspan[sp].append(s)
        rows = []
        for sp, ss in byspan.items():
            z = [s["position"][2] for s in ss]
            end = ss[-1]["position"][2]
            plane = min(PLANES[fx], key=lambda p: abs(p - end))
            rows.append({"style": sp[2] if sp else "none", "descent_mm": max(z) - end,
                         "removed_mm3": sum(s["removed_volume_est_mm3"] for s in ss),
                         "end_plane": plane if abs(plane - end) <= 0.35 else None, "x": ss[-1]["position"][0], "y": ss[-1]["position"][1],
                         "moves": len({s["move_index"] for s in ss})})
        st = collections.Counter(r["style"] for r in rows)
        deep = collections.Counter(r["style"] for r in rows if r["descent_mm"] >= 1.0)
        cut = collections.Counter(r["style"] for r in rows if r["descent_mm"] >= 1.0 and r["removed_mm3"] > 0.1)
        perplane = collections.Counter((r["style"], r["end_plane"]) for r in rows)
        out[f"{fx} {col}"] = {"spans_with_descent": dict(st), "descent_1mm": dict(deep), "descent_1mm_removing_stock": dict(cut),
                              "trace_plunge_entry_spans": sum(1 for s in spans if s[2] == "plunge entry"),
                              "ring_samples": sum(1 for i in range(len(S)) if S[i]["source_intent"] == "entry_plunge" and (i + 1 == len(S) or S[i + 1]["source_intent"] != "entry_plunge")),
                              "per_plane": {f"{a} @ {b}": n for (a, b), n in sorted(perplane.items(), key=str)},
                              "rows": rows}
        o = out[f"{fx} {col}"]
        print(fx, col, "ring samples", o["ring_samples"], "| spans with a descent", o["spans_with_descent"], "| >= 1 mm", o["descent_1mm"],
              "| and removing stock", o["descent_1mm_removing_stock"], "| plunge-entry spans", o["trace_plunge_entry_spans"])
json.dump(out, open(os.path.join(H, "entries.json"), "w"), indent=1)
