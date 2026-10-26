"""C36: run audit.py on the 8 G2 columns; write audit_results.json and audit_output.txt.

Nominal MRR = feed_rate * stepover * depth_per_pass of the first rough op
(project files): Corne corne_job_corrected_c12_*.toml '3D Rough 1':
1518.75 mm/min * 1.2 mm * 6.0 mm / 60 = 182.25 mm3/s; wanaka200
wanaka200_mt1_c12_rough_only*.toml '2 Back Rough': 1000 mm/min * 2.0 mm *
5.46 mm / 60 = 182.0 mm3/s."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audit
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
NOM = {"corne": 1518.75 * 1.2 * 6.0 / 60, "wanaka200": 1000.0 * 2.0 * 5.46 / 60}
OP = {"corne": "3D Rough 1", "wanaka200": "2 Back Rough"}
COLS = {"corne": [("contour_parallel", "fix6/corne/contour_parallel", "fix6/corne/contour_parallel"),
                  ("zigzag_pick D18", "fix5/corne/geodesic_local_zigzag_pick", "fix5/corne/geodesic_local_zigzag_pick"),
                  ("zigzag_pick D20", "fix7/corne/geodesic_local_zigzag_pick", "fix7/corne/geodesic_local_zigzag_pick"),
                  ("AgentSearch", "fix1/corne/agent", "fix2/simart/corne/agent")],
        "wanaka200": [("contour_parallel", "fix6/wanaka200/contour_parallel", "fix6/wanaka200/contour_parallel"),
                      ("zigzag_pick D18", "fix2/wanaka200/geodesic_local_zigzag", "fix2/simart/wanaka200/geodesic_local_zigzag"),
                      ("zigzag_pick D20", "fix7/wanaka200/geodesic_local_zigzag_pick", "fix7/wanaka200/geodesic_local_zigzag_pick"),
                      ("AgentSearch", "fix2/wanaka200/agent", "fix2/simart/wanaka200/agent")]}
res, txt = {}, []
for fx, cols in COLS.items():
    for label, rd, sd in cols:
        r = audit.audit(os.path.join(P3, rd), os.path.join(P3, sd), OP[fx], "tp_1_*.json", NOM[fx])
        r["run"] = rd; r.pop("descents")
        res[f"{fx} {label}"] = r
        txt.append(f"#### {fx} {label}\n" + audit.text(r))
json.dump(res, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "audit_results.json"), "w"), indent=1)
open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "audit_output.txt"), "w").write("\n\n".join(txt) + "\n")
print("\n\n".join(txt))
