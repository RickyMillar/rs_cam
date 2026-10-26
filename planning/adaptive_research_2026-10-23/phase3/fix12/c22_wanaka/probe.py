"""C22 §12.8: shared readers of the wanaka200 artefacts (D20 = fix7 zigzag_pick, C35 = fix12)."""
import json, os, collections, bisect
P3 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
COLS = {"D20": "fix7/wanaka200/geodesic_local_zigzag_pick", "C35": "fix12/wanaka200"}
SEED = (119.80, 124.874)
def load(d):
    d = os.path.join(P3, d)
    sim = json.load(open(os.path.join(d, "simulation.json")))
    S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
    tp = json.load(open(os.path.join(d, [f for f in os.listdir(d) if f.startswith("tp_1_")][0])))
    return S, tp
def descents(S, ent):
    """fix9/entries.py's rule: keep-down link spans whose entry_plunge samples drop >= 1 mm
    and remove > 0.1 mm3. Returns (span start move, last sample position)."""
    starts = [a for a, *_ in ent]
    by = collections.defaultdict(list)
    for s in S:
        if s["source_intent"] != "entry_plunge": continue
        m = s["move_index"]; k = bisect.bisect_right(starts, m) - 1
        if k >= 0 and ent[k][0] <= m <= ent[k][1]: by[ent[k]].append(s)
    out = []
    for sp, ss in by.items():
        z = [s["position"][2] for s in ss]
        if sp[2] == "keep-down link" and max(z) - z[-1] >= 1.0 and sum(s["removed_volume_est_mm3"] for s in ss) > 0.1:
            out.append((sp[0], ss[-1]["position"]))
    return out
