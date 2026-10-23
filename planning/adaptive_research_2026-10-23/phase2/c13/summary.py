"""C13: the summary numbers of the note (lattice statistics and cost) from
final.json. python3 -I summary.py"""
import json, os, sys
from collections import Counter
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CD
HERE = os.path.dirname(os.path.abspath(__file__))
F = json.load(open(os.path.join(HERE, "final.json")))

def ok(r):
    return r["fits"] and r["lead"] <= CD + 1e-6

def lattice_stats(rows, name):
    plus = [r for r in rows if r["sgn"] > 0 and "lattice" in r]
    out = {}
    for sp in ("cell", "step"):
        leads = [x for r in plus for x in r["lattice"][sp]["lead"]]
        diffs = [x - r["lead"] for r in plus for x in r["lattice"][sp]["lead"]]
        refused_pairs = sum(1 for r in plus for n in r["lattice"][sp]["lead_over"] if n > 0)
        bands_refused = sum(1 for r in plus if any(n > 0 for n in r["lattice"][sp]["lead_over"]))
        at = Counter(a for r in plus for a, n in zip(r["lattice"][sp]["at"], r["lattice"][sp]["lead_over"]) if n > 0)
        ramp_med = [x for r in plus for x in r["lattice"][sp]["ramp_med"]]
        ramp_over = sum(sum(r["lattice"][sp]["ramp_over"]) for r in plus)
        n_ramp = sum(4 * r["lattice"][sp]["n_ramp"] for r in plus)
        out[sp] = dict(lead_min=min(leads), lead_max=max(leads), diff_min=min(diffs), diff_max=max(diffs),
                       refused_pairs=refused_pairs, pairs=4 * len(plus), bands_refused=bands_refused,
                       bands=len(plus), refused_at=dict(at), ramp_med_min=min(ramp_med), ramp_med_max=max(ramp_med),
                       ramp_over=ramp_over, n_ramp=n_ramp)
        print(f"{name} {sp}: lattice lead {min(leads):.3f}-{max(leads):.3f} mm; lattice - exact {min(diffs):+.3f} to {max(diffs):+.3f};"
              f" (band, phase) refused {refused_pairs}/{4*len(plus)}; bands refused at >= 1 phase {bands_refused}/{len(plus)};"
              f" refused step piece {dict(at)}; ramp median reading {min(ramp_med):.3f}-{max(ramp_med):.3f};"
              f" ramp steps over {ramp_over}/{n_ramp}")
    return out

def counts(rows, name):
    by = {}
    for r in rows:
        by.setdefault((r["level"], r["arm"]), []).append(r)
    p = sum(1 for rs in by.values() if all(ok(r) for r in rs))
    worst = {k: max(r["lead"] for r in rs) - CD for k, rs in by.items()}
    print(f"{name}: bands {len(by)} pass {p}; excess over cD by band: " +
          " ".join(f"{k[0]}/{k[1]}:{v:+.4f}" for k, v in sorted(worst.items()) if v > 1e-6))
    return len(by), p, worst

for n in ("A", "B", "R"):
    counts(F[n], n)
SA = lattice_stats(F["A"], "A")
SR = lattice_stats(F["R"], "R")
# corner failures in A accepted by the lattice
for r in F["A"]:
    if r["sgn"] > 0 and r["arm"] == 0 and r["level"] >= 12:
        print(f"A corner fail level {r['level']}: exact {r['lead']:.4f}; lattice cell {[round(x,3) for x in r['lattice']['cell']['lead']]} step {[round(x,3) for x in r['lattice']['step']['lead']]}")
# cost
cur = {}
for r in F["A"]:
    if r["sgn"] > 0:
        cur.setdefault(r["arm"], []).append(r["L_b"])
lead = {}
for r in F["K"]:
    if r["sgn"] > 0:
        lead.setdefault(r["arm"], []).append((r["L_b"], r["ramp_len"], r["spare"], r["level"], r["rho"]))
tot_cur = sum(sum(v) for v in cur.values())
tot_lead = sum(L + rl + sp for v in lead.values() for L, rl, sp, _, _ in v)
tot_lead_arcs = sum(L for v in lead.values() for L, _, _, _, _ in v)
n_plunge = sum(len(v) for v in cur.values())
print(f"cost: current {n_plunge} bands, arcs {tot_cur:.1f} mm; reserved {sum(len(v) for v in lead.values())} bands, arcs {tot_lead_arcs:.1f} mm, ramps+spare {tot_lead - tot_lead_arcs:.1f} mm, total {tot_lead:.1f} mm")
print(f"  extra cut length {tot_lead - tot_cur:.1f} mm for {n_plunge} plunges: {(tot_lead - tot_cur)/n_plunge:.2f} mm per plunge removed")
print(f"  of which extra passes {tot_lead_arcs - tot_cur:.1f} mm, ramps and the forward run {tot_lead - tot_lead_arcs:.1f} mm")
for arm in sorted(cur):
    a = sum(cur[arm]); b = sum(L + rl + sp for L, rl, sp, _, _ in lead[arm])
    print(f"  arm {arm}: current {a:.1f}, reserved {b:.1f} ({len(lead[arm])} bands), +{b - a:.1f} mm, per plunge {(b - a)/len(cur[arm]):.2f}")
K = F["K"]
print("K all pass:", all(ok(r) for r in K), "worst", max(r["lead"] for r in K) - CD)
