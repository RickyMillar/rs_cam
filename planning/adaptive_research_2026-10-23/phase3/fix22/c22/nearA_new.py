"""C22 §16.14: wanaka's near-A clearing samples over 0.471 in D31 that D26 does not have.
Near-A set and move kinds as fix21/c22/nearA_kind.py (on a step / after a step / level).
'New' = a D31 over sample with no D26 near-A over sample within `rad` in XY and DPP/2 in Z.
Also the radial width of each over sample in sim cells over the bound (sim cell 1.0 mm on wanaka)."""
import collections, json, math, re, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent
R, D = 3.0, 6.0
fx = sys.argv[1] if len(sys.argv) > 1 else "wanaka200"
ref_run, cell, dpp = {"wanaka200": ("fix16/wanaka200_pick", 1.0, 5.46), "corne": ("fix16/corne_pick", 0.3, 6.0)}[fx]
ref = []
for ln in open(P3 / ref_run / "stdout_plain.txt"):
    if "index=0" in ln and "C40 entry kind" in ln and "LargeZ" in ln:
        kv = dict(re.findall(r"(\w+)=(\S+)", ln.split("C40 entry", 1)[1])); ref.append((float(kv["x"]), float(kv["y"])))
ref = np.array(ref)
thr = max(1.1 * dpp, 1.0)

def load(run):
    S = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]
    last = {}
    for s in S: last[s["move_index"]] = s
    ms = sorted(last); prev_of = {b: a for a, b in zip(ms, ms[1:])}
    def step(m):
        a = prev_of.get(m)
        if a is None: return None
        pa, pb = last[a]["position"], last[m]["position"]
        dz = pb[2] - pa[2]; xy = math.hypot(pb[0] - pa[0], pb[1] - pa[1])
        if last[m]["source_intent"] == "clearing_cut" and (abs(dz) > thr or (xy > 1e-6 and abs(dz) / xy > 0.3) or (xy <= 1e-6 and abs(dz) > 1e-6)):
            return dz
        return None
    base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
    rows = []; prev = None
    for s in S:
        p = s["position"]; ln = math.hypot(p[0]-prev[0], p[1]-prev[1]) if prev else 0.0; prev = p
        w = s["engagement"]["radial_woc_fraction"]
        if not (s["is_cutting"] and s["source_intent"] == "clearing_cut" and w > 0.471): continue
        if s["feed_rate_mm_min"] < base - 0.5: continue
        if not (len(ref) and np.min(np.hypot(ref[:, 0]-p[0], ref[:, 1]-p[1])) <= R + cell): continue
        m = s["move_index"]; st = step(m)
        if st is not None: kind = "on a step (climb)" if st > 0 else "on a step (descent)"
        elif prev_of.get(m) is not None and step(prev_of[m]) is not None:
            kind = "after a climb" if step(prev_of[m]) > 0 else "after a descent"
        else: kind = "level"
        rows.append((p[0], p[1], p[2], ln, w, s["axial_engagement_mm"], kind, m))
    return rows

old = load(ref_run); new = load(f"{H.parent.name}/cut_feed/{fx}")
O = np.array([(r[0], r[1], r[2]) for r in old])
out = {"D26 total": round(sum(r[3] for r in old), 1), "D31 total": round(sum(r[3] for r in new), 1)}
for rad in (cell, R):
    tot = collections.Counter(); cells_over = collections.Counter(); n = 0
    for r in new:
        d = np.hypot(O[:, 0]-r[0], O[:, 1]-r[1]); same = np.any((d <= rad) & (np.abs(O[:, 2]-r[2]) <= dpp/2))
        if same: continue
        tot[r[6]] += r[3]; n += 1
        cells_over[min(int((r[4]-0.471) * D / cell) + 1, 4)] += r[3]
    out[f"new within {rad} mm"] = {"total": round(sum(tot.values()), 1), "samples": n,
        **{k: round(v, 1) for k, v in sorted(tot.items())},
        "by sim cells over the bound (width over 0.471 D, 4 = 4 or more)": {k: round(v, 1) for k, v in sorted(cells_over.items())}}
# all over samples (old and new), by cells over
for tag, rows in (("D26", old), ("D31", new)):
    c = collections.Counter()
    for r in rows: c[min(int((r[4]-0.471) * D / cell) + 1, 4)] += r[3]
    out[f"{tag} near-A over by sim cells over"] = {k: round(v, 1) for k, v in sorted(c.items())}
    k = collections.Counter()
    for r in rows: k[r[6]] += r[3]
    out[f"{tag} near-A over by kind"] = {a: round(b, 1) for a, b in sorted(k.items())}
print(json.dumps(out, indent=1))
json.dump(out, open(H / f"nearA_new_{fx}.json", "w"), indent=1)
