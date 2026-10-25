"""C33 report: the thick over-0.471 body length by class, level, place and D18 sweep cover."""
import collections, math, os, pickle
import numpy as np
H = os.path.dirname(os.path.abspath(__file__))
D = pickle.load(open(os.path.join(H, "c33_rows.pkl"), "rb")); rows = D["rows"]; ctx = D["ctx"]
B = 0.471; R = 3.0; LEAVE = 0.5; M = 0.25
T = [r for r in rows if r["thick"] > B]
tot = sum(r["ln"] for r in rows); tT = sum(r["ln"] for r in T)
print(f"body over 0.471 (sim): {tot:.1f} mm; exact strict > 0.471: {sum(r['ln'] for r in rows if r['strict'] > B):.1f}; "
      f"sim-only (strict <= 0.471): {sum(r['ln'] for r in rows if r['strict'] <= B):.1f}; thick > 0.471: {tT:.1f}")

def turn_deg(m):
    c = ctx[m]; a, b = np.array(c["a"][:2]), np.array(c["b"][:2])
    return c, a, b

def cls(r):
    c = ctx[r["m"]]
    end = c["next_intent"] == "linking" and c["next_noncut"] - r["m"] <= 2
    start = c["moves_from_start"] <= 2 if False else False
    if c["run_style"] == "plunge entry" and c["moves_from_start"] <= 6:
        return "after a plunge (lobe start)"
    if end and c["len"] <= 2.0:
        return "run end <= 2 mm, then a lift (hook-like)"
    if end:
        return "run end > 2 mm, then a lift"
    if c["moves_from_start"] <= 6 and c["run_style"] == "keep-down link":
        return "first moves after a keep-down link (turn / lobe)"
    return "inside a run"

def place(r):
    d = r["wall_d"]
    if d <= R + LEAVE + 0.5: w = f"at a wall (<= {R + LEAVE + 0.5:.1f} mm)"
    elif d <= 2 * R + LEAVE: w = f"wall band ({R + LEAVE + 0.5:.1f}-{2 * R + LEAVE:.1f} mm)"
    else: w = f"open (> {2 * R + LEAVE:.1f} mm)"
    return w + (", island" if r["island"] else ", outer part")

def table(keyf, title):
    t = collections.defaultdict(lambda: collections.defaultdict(float))
    for r in T: t[keyf(r)][r["lz"]] += r["ln"]
    lv = sorted({r["lz"] for r in rows}, reverse=True)
    print(f"\n{title}: mm thick-over, levels {lv}")
    for k in sorted(t, key=lambda q: -sum(t[q].values())):
        print(f"  {k:52s} " + " ".join(f"{t[k][z]:7.1f}" for z in lv) + f"   sum {sum(t[k].values()):7.1f}")

table(cls, "by move class")
table(place, "by place")
table(lambda r: "draped (helix)" if r["kin"] == "helix" else r["kin"], "by kinematics")
table(lambda r: ("swept by D18 sweep >= 0.9" if (r["swept_d18"] or 0) >= 0.9 else ("0.5-0.9" if (r["swept_d18"] or 0) >= 0.5 else "< 0.5")), "by the share of the thick uncut stock D18's sweep covered")
z_above = collections.defaultdict(float)
for r in T:
    h = r["p"][2] - r["lz"]
    z_above["at level" if h <= 0.05 else ("<= 1 mm above" if h <= 1 else ("1-3 mm above" if h <= 3 else "> 3 mm above"))] += r["ln"]
print("\nby drape above the level:", {k: round(v, 1) for k, v in z_above.items()})
wd = np.array([r["wall_d"] for r in T]); w = np.array([r["ln"] for r in T])
o = np.argsort(wd); cw = np.cumsum(w[o]) / w.sum()
print("wall distance, length-weighted: p10 %.2f p50 %.2f p90 %.2f max %.2f" % (tuple(wd[o][np.searchsorted(cw, q)] for q in (0.1, 0.5, 0.9)) + (wd.max(),)))
th = np.array([r["thick"] for r in T]); print("thick width median %.3f" % np.median(th))
table(lambda r: "A step: a same-level pass ran higher" if r["loose"] <= B else "B fresh: no same-level pass cut it", "by mechanism (C32 reading)")

def cause(r):
    c = ctx[r["m"]]; h = r["p"][2] - r["lz"]
    run_len = c["run_end"] - c["run_start"]
    if r["lz"] == 0.5 and r["p"][2] <= 1.0 and run_len <= 8:
        return "hex-hole dip: a short run down to Z 0.5 between links at Z 4"
    if r["loose"] <= B and h > 0.05:
        return "draped wrap below a higher same-level pass (shoulder step)"
    if r["loose"] <= B:
        return "wrap at the level below a higher same-level pass (step)"
    return "fresh: first cut of that stock at this level"
table(cause, "by cause")
t = collections.defaultdict(lambda: [0.0, 0.0, 0.0, 0.0, []])
for r in T:
    e = t[cause(r)]; e[0] += r["ln"]; e[1] += r["ln"] * (r["wall_d"] <= R + LEAVE + 0.5); e[2] += r["ln"] * ((r["swept_d18"] or 0) >= 0.5); e[3] += r["ln"] * (r["kin"] == "helix"); e[4].append(r["wall_d"])
print("\ncause: mm; at a wall (<= 4.0); D18 sweep covered >= half; draped; wall distance median")
for k, e in sorted(t.items(), key=lambda q: -q[1][0]):
    print(f"  {k:62s} {e[0]:6.1f} {e[1]:6.1f} {e[2]:6.1f} {e[3]:6.1f} {np.median(e[4]):5.2f}")
