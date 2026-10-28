"""D68: the removed volume of the edge op by axial engagement band (DPP+0.05..+0.5, +0.5..+1, +1..+3, > DPP+3). Usage:
axial_hist.py <run under phase3> <tp> <DPP>"""
import json, sys, collections
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent
sim = json.load(open(P3 / sys.argv[1] / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]; tp = int(sys.argv[2]); D = float(sys.argv[3])
b = collections.Counter(); n = collections.Counter()
for s in sim["trace"]["samples"]:
    a = s.get("axial_engagement_mm") or 0.0
    if s["toolpath_id"] != tp or s["removed_volume_est_mm3"] <= Q or a <= D + 0.05: continue
    k = "DPP+0.05..0.5" if a <= D + 0.5 else "DPP+0.5..1" if a <= D + 1 else "DPP+1..3" if a <= D + 3 else "> DPP+3"
    b[k] += s["removed_volume_est_mm3"]; n[k] += 1
print(json.dumps({k: {"mm3": round(b[k]), "samples": n[k]} for k in ("DPP+0.05..0.5", "DPP+0.5..1", "DPP+1..3", "> DPP+3")}))
