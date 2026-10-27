"""D62: a copy of fix42/depth_by_level.py (writes into fix43). D61: the axial depth of cut of the edge op (Corne toolpath 5, wanaka toolpath 7), from the trace samples of
simulation.json. Per level (the sample tip Z, bins: the op's planned level Zs from 'Z levels computed', each sample in the
bin of the nearest planned level at or above it; the samples of plunges and links are counted apart): samples, max
axial_engagement_mm, removed volume, and the volume and XY length taken at axial > DPP. 'Removing' = removed_volume_est_mm3
> cell^2 * step. Also the op total and the same for op 1 for comparison. Usage: depth_by_level.py <run dir> <tp> <DPP> [op 1 DPP, default DPP] (D62: op 1 gets its own DPP; D61 used one DPP
for both, right there since op 1 and the edge op had the same DPP)."""
import collections, json, re, sys
from pathlib import Path
d = Path(sys.argv[1]); tp = int(sys.argv[2]); dpp = float(sys.argv[3]); dpp1 = float(sys.argv[4]) if len(sys.argv) > 4 else dpp
sim = json.load(open(d / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]; T = sim["trace"]["samples"]
out = {"run": d.name, "dpp": dpp}
for t in (1, tp):
    D = dpp1 if t == 1 else dpp
    S = [s for s in T if s["toolpath_id"] == t]
    rem = [s for s in S if s["removed_volume_est_mm3"] > Q]
    ax = lambda s: s.get("axial_engagement_mm") or 0.0
    by = collections.defaultdict(lambda: {"samples": 0, "max_axial": 0.0, "vol": 0.0, "vol_over": 0.0, "n_over": 0})
    for s in rem:
        si = s["source_intent"] if isinstance(s["source_intent"], str) else "other"
        key = (si, round(s["position"][2] / 1.0) * 1.0)
        b = by[key]; b["samples"] += 1; b["max_axial"] = max(b["max_axial"], ax(s)); b["vol"] += s["removed_volume_est_mm3"]
        if ax(s) > D + 0.05: b["vol_over"] += s["removed_volume_est_mm3"]; b["n_over"] += 1
    tot = sum(s["removed_volume_est_mm3"] for s in rem); over = sum(s["removed_volume_est_mm3"] for s in rem if ax(s) > D + 0.05)
    out[f"tp{t}"] = {"samples_removing": len(rem), "removed_mm3": round(tot), "max_axial_mm": round(max((ax(s) for s in rem), default=0), 3),
                     "removed_at_axial_over_dpp_mm3": round(over), "share_over_dpp": round(over / tot, 3) if tot else None,
                     "samples_over_dpp": sum(1 for s in rem if ax(s) > D + 0.05),
                     "by_intent_and_z_1mm": {f"{k[0]} z~{k[1]:.0f}": {kk: (round(v, 2) if isinstance(v, float) else v) for kk, v in b.items()} for k, b in sorted(by.items(), key=lambda kv: (kv[0][0], -kv[0][1])) if b["samples"] >= 5}}
print(json.dumps(out, indent=1)); json.dump(out, open(Path(__file__).resolve().parent / f"depth_{d.name}.json", "w"), indent=1)
