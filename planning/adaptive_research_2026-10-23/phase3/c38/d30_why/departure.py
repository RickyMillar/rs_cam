"""C38 §15.6, step 3: for each move of strip87.json, the plunge it leaves
(the end of the last program move at the plunge rate 750 before it) and
the XY distance of the move's start and end from it. A readout step whose
start lies within R = 3 mm of a straight plunge is exempt
(`PassLoad::departing`, research_geodesic.rs pass_load_readout) and makes
no slot record, so it gets no cap. Writes departure.json. Read only."""
import importlib.util, json, math
from pathlib import Path
H = Path(__file__).resolve().parent; P3 = H.parent.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
ops = A.op_moves(str(P3 / "fix21" / "a_only" / "corne" / "program.nc"), "3D Rough 1")
S = json.load(open(H / "strip87.json"))
rows = []
for m in S["moves"]:
    k = m["move"]
    j = k
    while j > 0 and abs((ops[j + 1]["f"] or 0) - 750.0) > 0.5:
        j -= 1
    plunge = ops[j + 1]["b"]; vertical = ops[j + 1]["a"][:2] == plunge[:2]
    a, b = ops[k + 1]["a"], ops[k + 1]["b"]
    da = math.hypot(a[0] - plunge[0], a[1] - plunge[1]); db = math.hypot(b[0] - plunge[0], b[1] - plunge[1])
    rows.append({"move": k, "over_samples": m["over_samples"], "g": ops[k + 1]["g"], "moves_after_plunge": k - j,
                 "plunge_vertical": vertical, "start_from_plunge_mm": round(da, 3), "end_from_plunge_mm": round(db, 3)})
dep = [r for r in rows if r["g"] == 1 and r["plunge_vertical"] and r["moves_after_plunge"] <= 2 and r["start_from_plunge_mm"] < 3.0]
out = {"moves": len(rows), "samples": sum(r["over_samples"] for r in rows),
       "departure_moves": len(dep), "departure_samples": sum(r["over_samples"] for r in dep),
       "departure_max_end_from_plunge_mm": max(r["end_from_plunge_mm"] for r in dep),
       "other": [r for r in rows if r not in dep], "rows": rows}
(H / "departure.json").write_text(json.dumps(out, indent=1))
print(json.dumps({k: v for k, v in out.items() if k != "rows"}, indent=1))
