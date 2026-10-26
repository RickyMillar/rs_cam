"""C43 (fix24): re-score op 1 of each run with the load L of area_oracle.py
(the per-strip sum of mrr_cross_section_mm2 over mrr_cross_section_mm2(d, D);
the box L_box is an info column). The old count is C22 §16.9's: the XY step length of clearing_cut
cutting samples with sim radial > 0.471 (fix21-23 load_split.py). Its length
is split into three classes, in this order:
  sim-only   : L <= 0.471 and the exact width w <= 0.471 (the exact radial
               reading clears it already);
  thin layer : w > 0.471 but the area load L <= 0.471 (the material is thinner
               than Depth/Pass over part or all of the width);
  still over : L > 0.471 (whatever w reads).
'new over' is the length that the old count does not hold (sim radial <=
0.471) but the area load reads over 0.471 (a cut deeper than Depth/Pass).
Writes rescore.json."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
B = 0.471
RUNS = [("corne", "contour_parallel", "fix6/corne/contour_parallel"), ("corne", "D26", "fix16/corne_pick"), ("corne", "C35", "fix12/corne"), ("corne", "D33", "fix23/default/corne"),
        ("corne", "D33 stock_depth", "fix23/stock_depth/corne"),
        ("wanaka200", "contour_parallel", "fix6/wanaka200/contour_parallel"), ("wanaka200", "D26", "fix16/wanaka200_pick"), ("wanaka200", "C35", "fix12/wanaka200"), ("wanaka200", "D33", "fix23/default/wanaka200"),
        ("wanaka200", "D33 stock_depth", "fix23/stock_depth/wanaka200")]
HBINS = [0.0, 0.5, 1.5, 3.0, 1e9]   # hmax classes of the thin-layer length (mm)


def score(rel):
    z = np.load(H / f"area_{rel.replace('/', '_')}.npz"); a = z["data"]; c = list(z["cols"]); C = lambda k: a[:, c.index(k)]
    step, rad, w, L, hm = C("step_mm"), C("sim_radial"), C("exact_w"), C("L"), C("hmax")
    old = rad > B
    so = old & (w <= B) & (L <= B); th = old & (w > B) & (L <= B); st = old & (L > B); new = ~old & (L > B)
    r = {"dpp": float(z["dpp"]), "read": int(len(a)),
         "old_over_mm": round(float(step[old].sum()), 1),
         "sim_only_mm": round(float(step[so].sum()), 1),
         "thin_layer_mm": round(float(step[th].sum()), 1),
         "still_over_mm": round(float(step[st].sum()), 1),
         "new_over_mm": round(float(step[new].sum()), 1),
         "area_over_mm": round(float(step[(L > B)].sum()), 1),
         "thin_by_hmax_mm": [round(float(step[th & (hm > lo) & (hm <= hi)].sum()), 1) for lo, hi in zip(HBINS[:-1], HBINS[1:])],
         "thin_mean_height_mm": round(float(np.average(C("area_mm2")[th] / np.maximum(w[th] * 6.0, 1e-9), weights=step[th])), 2) if th.any() else None,
         "still_over_peak_L": round(float(L[st].max()), 3) if st.any() else None,
         "area_peak_L": round(float(L.max()), 3)}
    # L over 0.471 but w <= 0.471 can only come from a cut deeper than Depth/Pass
    # still over at the base feed (the planner did not lower it: not a C38 slot, not a capped turn)
    fd = C("feed"); base = float(z["base_feed"]); r["base_feed"] = base
    r["still_over_reduced_feed_mm"] = round(float(step[st & (fd < base - 0.5)].sum()), 1)
    r["still_over_base_feed_mm"] = round(float(step[st & (fd >= base - 0.5)].sum()), 1)
    r["still_over_full_depth_mm"] = round(float(step[st & (hm >= r["dpp"] - 0.01)].sum()), 1)
    # the sim's own area form: removed volume / XY step / (D d) (noisy per sample; a cross-check only)
    Ls = C("removed_mm3") / np.maximum(step, 1e-9) / (6.0 * r["dpp"])
    r["thin_simvol_agree_frac"] = round(float(step[th & (Ls <= B)].sum() / step[th].sum()), 3) if th.any() else None
    # info: the box (deepest point x full width), not the definition
    Lb = C("L_box"); r["box_over_mm"] = round(float(step[Lb > B].sum()), 1)
    r["box_only_mm"] = round(float(step[(Lb > B) & (L <= B)].sum()), 1)
    r["deep_narrow_mm"] = round(float(step[(L > B) & (w <= B)].sum()), 1)
    return r


if __name__ == "__main__":
    out = {}
    for fx, arm, rel in RUNS:
        out[f"{fx} {arm}"] = {"run": rel, **score(rel)}
        print(fx, arm, json.dumps(out[f"{fx} {arm}"]))
    json.dump(out, open(H / "rescore.json", "w"), indent=1)
