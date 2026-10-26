"""D23: where the depth-of-cut peaks and the "deeper than commanded" samples
of the two Corne D23 runs are. Reads simulation.json only.
- depth of cut: the sample's axial_doc_mm (the gate's 'depth of cut, peak').
- "crosses standing material deeper than this pass was planned to cut": a
  cutting sample whose axial_doc_mm is over the op's depth per pass plus one
  sim cell (op 1: 6.0 + 0.3; copy op: 3.0 + 0.3, the project file). The
  share of cutting samples is printed so it can be checked against the CLI
  line (3.5 % headon, 3.6 % pick).
- place: the hole-filled silhouette of fix9/corne_part.npz, the distance
  outside it (the outer strip is 0-9 mm), or beyond the mesh bbox.
- kind: the sample's source_intent and cut_kinematics (the trace has no slot
  tag)."""
import collections, json, sys
from pathlib import Path
import numpy as np
from scipy import ndimage

H = Path(__file__).resolve().parent; P3 = H.parent
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
dout = ndimage.distance_transform_edt(~sil) * g


def place(x, y):
    c, r = int(round((x - x0) / g)), int(round((y - y0) / g))
    if not (0 <= r < sil.shape[0] and 0 <= c < sil.shape[1]):
        return "beyond mesh bbox"
    if sil[r, c]:
        return "inside silhouette"
    d = float(dout[r, c])
    return "outer strip 0-9" if d < 9 else "outside >= 9"


DPP = {1: 6.0, 3: 3.0}
out = {}
for run in sys.argv[1:]:
    S = json.load(open(P3 / run / "simulation.json"))["trace"]["samples"]
    cell = 0.3
    r = {}
    for tp in (1, 3):
        cut = [s for s in S if s["toolpath_id"] == tp and s["is_cutting"]]
        peak = max(s["axial_doc_mm"] for s in cut)
        at_peak = [s for s in cut if s["axial_doc_mm"] >= peak - 1e-9]
        deep = [s for s in cut if s["axial_doc_mm"] > DPP[tp] + cell]
        key = lambda s: (s["source_intent"], s["cut_kinematics"], place(*s["position"][:2]))
        r[f"tp{tp}"] = {
            "cutting_samples": len(cut), "peak_doc_mm": peak, "peak_samples": len(at_peak),
            "peak_by_kind_place": {" / ".join(k): v for k, v in collections.Counter(map(key, at_peak)).most_common()},
            "peak_moves": sorted({s["move_index"] for s in at_peak})[:40],
            "peak_z": sorted({round(s["position"][2], 2) for s in at_peak}),
            "peak_xy_examples": [[round(v, 2) for v in s["position"]] for s in at_peak[:: max(1, len(at_peak) // 8)]][:8],
            "deep_share_pct": round(100 * len(deep) / len(cut), 2), "deep_samples": len(deep),
            "deep_by_kind_place": {" / ".join(k): v for k, v in collections.Counter(map(key, deep)).most_common()},
            "deep_z": dict(sorted(collections.Counter(round(s["position"][2], 2) for s in deep).items())),
        }
    out[run] = r
    print("==", run)
    for tp, v in r.items():
        print(f"  {tp}: peak doc {v['peak_doc_mm']} mm on {v['peak_samples']} samples; {v['peak_by_kind_place']}; z {v['peak_z']}")
        print(f"     peak xyz examples {v['peak_xy_examples']}; moves {v['peak_moves'][:12]}")
        print(f"     deeper than dpp+cell: {v['deep_samples']} = {v['deep_share_pct']} % of cutting samples; {v['deep_by_kind_place']}")
        print(f"     by z {v['deep_z']}")
json.dump(out, open(H / "peaks_d23.json", "w"), indent=1)
