"""C36: write README.md from audit_results.json (the tables) and the fixed text."""
import json, os
H = os.path.dirname(os.path.abspath(__file__))
R = json.load(open(os.path.join(H, "audit_results.json")))
COLS = ["contour_parallel", "zigzag_pick D18", "zigzag_pick D20", "AgentSearch"]


def lab(r, key):
    return r["descent_event_labels"].get(key, 0)


rows1, rows2, rows3, rows4 = [], [], [], []
for fx in ("corne", "wanaka200"):
    for c in COLS:
        r = R[f"{fx} {c}"]; n = f"{fx} {c}"
        rows1.append(f"| {n} | {r['descent_events']} | {lab(r, 'entry_plunge / plunge entry')} / {lab(r, 'entry_plunge / keep-down link')} / {lab(r, 'linking / keep-down link')} "
                     f"| {r['descent_event_drop_median_mm']} / {r['descent_event_drop_max_mm']} | {r['descent_event_time_s']} |")
        big = sum(r["planar_turn_hist"][4:])
        rows2.append(f"| {n} | {r['junctions_at_feed']} | {r['reversals_over_90deg']} | {r['turns_exactly_90deg']} | {r['z_reversals']} | "
                     f"{r['planar_junctions']} | {r['planar_reversals']} | {big} ({100 * big / r['planar_junctions']:.1f} %) | "
                     + " / ".join(str(x) for x in r["planar_turn_hist"]) + " |")
        rows3.append(f"| {n} | {r['mrr_p50']} / {r['mrr_p95']} / {r['mrr_p99']} / {r['mrr_max']} | {r['time_over_nominal_s']} | {r['time_over_2x_nominal_s']} | "
                     + " / ".join(str(x) for x in r["mrr_time_s"]) + " |")
        ts = r["time_split_s"]; tot = r["sample_time_s"]
        cells = " | ".join(f"{ts.get(k, 0.0)} ({100 * ts.get(k, 0.0) / tot:.1f} %)" for k in ("cut", "fed in air", "lift feed", "plunge feed", "rapid"))
        rows4.append(f"| {n} | {cells} | {tot} | {r['sim_runtime_s']} |")
head = open(os.path.join(H, "README_head.md")).read()
body = f"""
## The 8 G2 columns ('3D Rough 1' on Corne, '2 Back Rough' on wanaka200)

`audit_output.txt` holds the full printout; `audit_results.json` the numbers.

### 1. Descent events into stock (motion predicate; labels only for the cross-table)

| Column | Events | Labels: plunge entry / keep-down link (entry_plunge) / keep-down link (linking) | Drop median / max (mm) | Time (s) |
|---|---|---|---|---|
{chr(10).join(rows1)}

### 2. Junctions at feed (turn = the 3D angle between the outgoing and the incoming direction)

Histogram bins (deg): (0,1] / (1,5] / (5,15] / (15,30] / (30,45] / (45,60] / (60,90] / (90,135] / (135,180].

All junctions at feed, then the planar ones (both moves travel in XY: the
cutting path without the vertical lifts and plunges).

| Column | Junctions | Reversals (> 90 deg) | Exactly 90 deg | Z reversals | Planar junctions | Planar reversals | Planar turns over 30 deg | Planar turn histogram |
|---|---|---|---|---|---|---|---|---|
{chr(10).join(rows2)}

### 3. MRR over time (removed volume per second of each sample, weighted by time)

Bins (mm³/s): 0 / (0,25) / [25,50) / [50,100) / [100,200) / [200,400) / [400,800) / [800,1600) / ≥ 1600, in seconds.
Nominal MRR: Corne 182.25 mm³/s, wanaka200 182.0 mm³/s (`run_all.py`).

| Column | p50 / p95 / p99 / max (mm³/s) | Time over nominal (s) | Time over 2x nominal (s) | Time per bin (s) |
|---|---|---|---|---|
{chr(10).join(rows3)}

### 4. Time split by motion (sample time; share of the sample time)

| Column | Cut | Fed in air | Lift feed | Plunge feed | Rapid | Sample time (s) | Toolpath summary runtime (s) |
|---|---|---|---|---|---|---|---|
{chr(10).join(rows4)}
"""
tail = open(os.path.join(H, "README_tail.md")).read()
open(os.path.join(H, "README.md"), "w").write(head + body + tail)
print("README.md written")
