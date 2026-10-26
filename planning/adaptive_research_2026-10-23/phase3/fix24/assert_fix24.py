"""C43 (fix24): checks every number of README.md.
- The bound: the ceiling and the exact bound from the shipped formulas.
- Depth/Pass: read from the two project files at the stated lines.
- The synthetic cases: run again; each within one bin of the closed form.
- The oracle: an own brute-force reader (no incremental height field; the
  stock under each lattice point is the lowest stamp of the earlier steps
  that covers it) on 12 read samples per run of four runs, against the npz.
- rescore.json: every field again from the npz with own code.
- The old counts: equal to P15_D_verify.md, P3_C38_outer_band.md §16.4 and
  fix23's load_split json files.
- The longest sample step per part, from the simulation files.
- README: each table cell and each number of the reading is found; every
  number token of the README prose is a checked value or a declared
  constant."""
import json, math, re, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent; ROOT = P3.parent; REPO = ROOT.parent.parent
sys.path.insert(0, str(H))
README = open(H / "README.md").read()
J = json.load(open(H / "rescore.json")); SYN = json.load(open(H / "synthetic.json"))
N = [0]; SEEN = set()


def ok(c, m):
    N[0] += 1
    if not c:
        raise SystemExit(f"FAIL: {m}")


def on(s, m):
    ok(s in README, f"{m}: {s!r} not in README"); SEEN.add(s)


R, D, PITCH, B = 3.0, 6.0, 0.05, 0.471
# ---- the bound
ceil = (1 - math.cos(2 * math.pi * 1.05 * math.acos(1 - 2.0 / R) / (2 * math.pi))) / 2
ok(abs(ceil - 0.3625) < 1e-3, f"ceiling {ceil}")
ok(round(ceil + (max(R / 6, 0.1) + 0.1 + 0.5 / 10) / D, 3) == B, "exact bound 0.471")
ok(abs(PITCH / D - 0.0083) < 5e-5, "pitch / D"); on("each w within pitch / D = 0.0083 of e/D", "pitch term")
on("0.471 at s 2, R 3", "bound text")
# ---- Depth/Pass from the project files
c_toml = open(REPO / "planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml").read().splitlines()
w_toml = open(REPO / "planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml").read().splitlines()
ok(c_toml[126].strip() == "depth_per_pass = 6.0" and c_toml[113].strip() == 'name = "3D Rough 1"', "Corne d line 127")
ok(w_toml[255].strip() == "depth_per_pass = 5.46" and w_toml[242].strip() == 'name = "2 Back Rough"', "wanaka d line 256")
import area_oracle  # noqa: E402
ok(area_oracle.DPP == {"corne": 6.0, "wanaka200": 5.46} and area_oracle.PITCH == PITCH and area_oracle.R == R, "oracle constants")
on("Corne op 1 \"3D Rough 1\" 6.0 mm\n  (`corne_job_corrected_c12_agent_search.toml` line 127), wanaka200 op 1\n  \"2 Back Rough\" 5.46 mm", "d text")
# ---- synthetic
import synthetic  # noqa: E402
for c in SYN["cases"]:
    again = synthetic.case(c["e"], c["t"], c["ang"]); ok(again == c, f"synthetic rerun {c}")
    ok(abs(c["L"] - c["e"] * c["t"] / (D * 6.0)) <= PITCH * c["t"] / (D * 6.0) + 1e-4, f"L within one bin {c}")
    ok(abs(c["w"] - c["e"] / D) <= PITCH / D + 1e-4, f"w within pitch/D {c}")
    on(f"| {c['e']:.1f} | {c['t']:.1f} | {c['ang']:.1f} | {c['L']:.4f} | {c['L_expect']:.4f} | {c['w']:.4f} | {c['w_expect']:.4f} |", "syn row")
on(f"{SYN['cases'][6]['L']:.4f} against w {SYN['cases'][6]['w']:.4f}: a deep side cut is held", "deep text")
ok(SYN["cases"][6]["L"] > B > SYN["cases"][6]["w"], "deep case over the bound, width under")

RUNS = {"corne contour_parallel": "fix6/corne/contour_parallel", "wanaka200 contour_parallel": "fix6/wanaka200/contour_parallel",
        "corne D26": "fix16/corne_pick", "corne C35": "fix12/corne", "corne D33": "fix23/default/corne", "corne D33 stock_depth": "fix23/stock_depth/corne",
        "wanaka200 D26": "fix16/wanaka200_pick", "wanaka200 C35": "fix12/wanaka200", "wanaka200 D33": "fix23/default/wanaka200",
        "wanaka200 D33 stock_depth": "fix23/stock_depth/wanaka200"}


def load(rel):
    z = np.load(H / f"area_{rel.replace('/', '_')}.npz"); return z, list(z["cols"])


# ---- an own brute-force reader
def brute(rel, fx, rows_pick):
    sim = json.load(open(P3 / rel / "simulation.json")); T = sim["trace"]["samples"]
    lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
    if fx == "wanaka200":
        lo, hi = [0.0, 0.0, 0.0], [hi[k] - lo[k] for k in range(3)]
    P = np.array([s["position"] for s in T]); tpid = np.array([s["toolpath_id"] for s in T])
    # stamp points: sample 0 alone; step j (j-1 -> j, same toolpath): ceil(len / 0.3) points, Z linear
    pts, owner = [P[:1]], [np.array([0])]
    for j in range(1, len(T)):
        if tpid[j] != tpid[j - 1]:
            pts.append(P[j:j + 1]); owner.append(np.array([j])); continue
        L = math.hypot(*(P[j, :2] - P[j - 1, :2])); m = max(1, math.ceil(L / 0.3))
        f = np.arange(1, m + 1)[:, None] / m
        pts.append(P[j - 1] + f * (P[j] - P[j - 1])); owner.append(np.full(m, j))
    pts = np.vstack(pts); owner = np.concatenate(owner)
    out = []
    for row in rows_pick:
        n = int(row[0]); p, q = P[n], P[n - 1]
        mx, my, mz = (p + q) / 2
        ii = np.arange(math.ceil((mx - R - lo[0]) / PITCH), math.floor((mx + R - lo[0]) / PITCH) + 1)
        jj = np.arange(math.ceil((my - R - lo[1]) / PITCH), math.floor((my + R - lo[1]) / PITCH) + 1)
        ii = ii[(ii >= 0) & (lo[0] + ii * PITCH <= hi[0] + 1e-9)]; jj = jj[(jj >= 0) & (lo[1] + jj * PITCH <= hi[1] + 1e-9)]
        X, Y = np.meshgrid(lo[0] + ii * PITCH, lo[1] + jj * PITCH)
        inside = (X - mx) ** 2 + (Y - my) ** 2 <= R * R
        X, Y = X[inside], Y[inside]
        near = (owner < n) & (np.abs(pts[:, 0] - mx) <= 2 * R + 0.1) & (np.abs(pts[:, 1] - my) <= 2 * R + 0.1)
        Q = pts[near]
        top = np.full(X.shape, float(np.float32(hi[2])))
        for a in range(0, len(Q), 2000):
            qq = Q[a:a + 2000]
            cov = (X[:, None] - qq[None, :, 0]) ** 2 + (Y[:, None] - qq[None, :, 1]) ** 2 <= R * R
            zz = np.where(cov, qq[None, :, 2], np.inf).min(1)
            top = np.minimum(top, zz.astype(np.float32).astype(float))   # the oracle keeps H in float32
        ht = top - mz; sel = ht > 0.01
        dx, dy = p[0] - q[0], p[1] - q[1]; L = math.hypot(dx, dy); sx, sy = -dy / L, dx / L
        if not sel.any():
            out.append((0.0, 0.0, 0.0, 0.0, 0.0)); continue
        u = (X[sel] - mx) * sx + (Y[sel] - my) * sy; h = ht[sel]
        bins = {}
        for b, v in zip(np.rint((u + R) / PITCH).astype(int), h):
            bins[b] = max(bins.get(b, 0.0), v)
        # mrr_cross_section_mm2 of the flat end mill is ap * ae (tool/mod.rs:508): per strip h * pitch
        A = sum(v * PITCH for v in bins.values()); d = area_oracle.DPP[fx]; w = (u.max() - u.min()) / D
        out.append((w, h.max(), A, A / (d * D), (h.max() * w * D) / (d * D)))
    return out


rng = np.random.default_rng(43)
for key in ("corne D33", "corne contour_parallel", "wanaka200 D33", "wanaka200 contour_parallel"):
    rel = RUNS[key]; fx = key.split()[0]; z, c = load(rel); a = z["data"]
    pick = a[rng.choice(len(a), 12, replace=False)]
    for row, (w, hm, A, L, Lb) in zip(pick, brute(rel, fx, pick)):
        ci = lambda k: row[c.index(k)]
        ok(abs(w - ci("exact_w")) < 1e-6 and abs(hm - ci("hmax")) < 1e-4 and abs(A - ci("area_mm2")) < 1e-3 and abs(L - ci("L")) < 1e-4
           and abs(Lb - ci("L_box")) < 1e-4, f"brute {key} sample {int(row[0])}: {(w, hm, A, L, Lb)}")


# ---- rescore again
def again(rel):
    z, c = load(rel); a = z["data"]; C = lambda k: a[:, c.index(k)]
    st_, rad, w, L, hm, fd = C("step_mm"), C("sim_radial"), C("exact_w"), C("L"), C("hmax"), C("feed")
    d = float(z["dpp"]); base = float(z["base_feed"]); old = rad > B
    S = lambda m: round(float(st_[m].sum()), 1)
    th = old & (w > B) & (L <= B); stl = old & (L > B)
    Ls = C("removed_mm3") / np.maximum(st_, 1e-9) / (D * d)
    r = {"old": S(old), "so": S(old & (w <= B) & (L <= B)), "th": S(th), "st": S(stl), "new": S(~old & (L > B)), "area": S(L > B),
         "red": S(stl & (fd < base - 0.5)), "base": S(stl & (fd >= base - 0.5)), "full": S(stl & (hm >= d - 0.01)),
         "hb": [S(th & (hm > l) & (hm <= u)) for l, u in ((0, .5), (.5, 1.5), (1.5, 3), (3, 1e9))],
         "mean": round(float(np.average(C("area_mm2")[th] / (w[th] * D), weights=st_[th])), 2), "peak": round(float(L.max()), 3),
         "agree": round(float(st_[th & (Ls <= B)].sum() / st_[th].sum()), 3), "d": d, "bf": base,
         "box": S(C("L_box") > B), "boxonly": S((C("L_box") > B) & (L <= B))}
    # the base feed again from the simulation file
    sim = json.load(open(P3 / rel / "simulation.json"))
    fs = [round(s["feed_rate_mm_min"]) for s in sim["trace"]["samples"] if s["toolpath_id"] == 1 and s["is_cutting"] and s["source_intent"] == "clearing_cut"]
    vals, cnt = np.unique(fs, return_counts=True); r["bf_sim"] = float(vals[cnt.argmax()])
    # the old count again from the simulation file (load_split.py's measure; every op-1 sample, not only the read ones)
    S1 = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]; tot = 0.0
    for s0, s1 in zip(S1[:-1], S1[1:]):
        if s1["is_cutting"] and s1["source_intent"] == "clearing_cut" and s1["engagement"]["radial_woc_fraction"] > B:
            tot += math.hypot(s1["position"][0] - s0["position"][0], s1["position"][1] - s0["position"][1])
    r["old_sim"] = round(tot, 1)
    return r


AG = {}
for key, rel in RUNS.items():
    r = again(rel); j = J[key]; AG[key] = r
    ok([r[k] for k in ("old", "so", "th", "st", "new", "area", "red", "base", "full", "hb", "mean", "peak", "agree", "d", "bf")] ==
       [j["old_over_mm"], j["sim_only_mm"], j["thin_layer_mm"], j["still_over_mm"], j["new_over_mm"], j["area_over_mm"],
        j["still_over_reduced_feed_mm"], j["still_over_base_feed_mm"], j["still_over_full_depth_mm"], j["thin_by_hmax_mm"],
        j["thin_mean_height_mm"], j["area_peak_L"], j["thin_simvol_agree_frac"], j["dpp"], j["base_feed"]], f"rescore {key}: {r}")
    ok(r["box"] == j["box_over_mm"] and r["boxonly"] == j["box_only_mm"], f"{key} box / box-only")
    z_, c_ = load(rel); a_ = z_["data"]; Cc = lambda k: a_[:, c_.index(k)]
    ok(bool(np.all(Cc("L") <= Cc("L_box") + 2 * PITCH * Cc("hmax") / (D * float(z_["dpp"])) + 1e-9)), f"{key} the box is not under L (to two strips: rint bins at both ends), per sample")
    ok(r["bf_sim"] == r["bf"], f"{key} base feed"); ok(r["old_sim"] == r["old"], f"{key} old count from the sim {r['old_sim']}")
    ok(abs(r["so"] + r["th"] + r["st"] - r["old"]) <= 0.25, f"{key} partition")
    ok(abs(r["st"] + r["new"] - r["area"]) <= 0.15, f"{key} area = still + new")
# ---- the old counts against the reports
P15 = open(ROOT / "P15_D_verify.md").read()
row = re.search(r"^\| over 0\.471 \(mm\) \| 780\.8 \| 1065\.9 \| (.*)$", P15, re.M).group(0)
cells = [x.strip() for x in row.split("|")[2:-1]]
ok(cells == [f"{AG[k]['old']:.1f}" if k else c_ for k, c_ in zip(["corne D26", None, "corne D33", "corne D33 stock_depth", "wanaka200 D26", None, "wanaka200 D33", "wanaka200 D33 stock_depth"], cells)], f"P15 old counts {cells}")
C38 = open(ROOT / "P3_C38_outer_band.md").read()
ok(f"| Corne C35 | {AG['corne C35']['old']:.1f} |" in C38 and f"| wanaka C35 | {AG['wanaka200 C35']['old']:.1f} |" in C38, "C38 §16.4 C35 old counts")
for fx in ("corne", "wanaka200"):
    for col in ("default", "stock_depth"):
        ls = json.load(open(P3 / "fix23" / f"load_split_{fx}_{col}.json"))[f"fix23/{col}/{fx}"]
        ok(ls["over_0.471_mm"] == AG[f"{fx} D33{'' if col == 'default' else ' stock_depth'}"]["old"], f"fix23 load_split {fx} {col}")
# ---- the longest step
for fx, val in (("corne", "0.54"), ("wanaka200", "1.83")):
    mx = 0.0
    for key, rel in RUNS.items():
        if key.startswith(fx):
            S1 = [s["position"] for s in json.load(open(P3 / rel / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]
            mx = max(mx, max(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(S1[:-1], S1[1:])))
    ok(f"{mx:.2f}" == val, f"{fx} longest step {mx}")
on("0.54 mm on Corne\n  and 1.83 mm on wanaka200", "longest step text"); on("(wanaka200: up to 1.83 mm)", "longest step limit")
# ---- README tables and reading
ARMS = ["contour_parallel", "D26", "C35", "D33", "D33 stock_depth"]
for fx in ("corne", "wanaka200"):
    g = lambda f: " | ".join(f(AG[f"{fx} {a}"]) for a in ARMS)
    for lab, f in (("op 1 Depth/Pass d (mm)", lambda r: f"{r['d']:.2f}"), ("old count: sim radial > 0.471 (mm)", lambda r: f"{r['old']:.1f}"),
                   ("**load count, per strip: L > 0.471 (mm)**", lambda r: f"**{r['area']:.1f}**"),
                   ("box count, info: L_box > 0.471 (mm)", lambda r: f"{r['box']:.1f}"),
                   ("box-only: L_box > 0.471, L ≤ 0.471 (mm)", lambda r: f"{r['boxonly']:.1f}"),
                   ("old count, of it sim-only: L ≤ 0.471, w ≤ 0.471 (mm)", lambda r: f"{r['so']:.1f}"),
                   ("old count, of it thin: w > 0.471, L ≤ 0.471 (mm)", lambda r: f"{r['th']:.1f}"),
                   ("old count, of it still over: L > 0.471 (mm)", lambda r: f"{r['st']:.1f}"),
                   ("new over: sim radial ≤ 0.471, L > 0.471 (mm)", lambda r: f"{r['new']:.1f}"),
                   ("load count at a lowered feed / at the base feed (mm)", lambda r: f"{r['red']:.1f} / {r['base']:.1f}"),
                   ("base clearing feed (mm/min)", lambda r: f"{r['bf']:.0f}"), ("peak L", lambda r: f"{r['peak']:.3f}")):
        on(f"| {lab} | {g(f)} |", f"{fx} {lab}")
    on(" | ".join(f"{a} (`{RUNS[f'{fx} {a}']}`)" for a in ARMS), f"{fx} head")
A = AG
on(f"Corne D26 {A['corne D26']['area']:.1f} of {A['corne D26']['old']:.1f} mm,\n   C35 {A['corne C35']['area']:.1f} of {A['corne C35']['old']:.1f} mm, D33 {A['corne D33']['area']:.1f} of\n   {A['corne D33']['old']:.1f} mm; wanaka200 D26 {A['wanaka200 D26']['area']:.1f} of\n   {A['wanaka200 D26']['old']:.1f} mm, C35 {A['wanaka200 C35']['area']:.1f} of {A['wanaka200 C35']['old']:.1f} mm,\n   D33 {A['wanaka200 D33']['area']:.1f} of {A['wanaka200 D33']['old']:.1f} mm.", "reading 1")
ok(all(A[k]["area"] < 0.5 * A[k]["old"] for k in A if "contour" not in k and k != "corne D33" and k != "corne D33 stock_depth"), "most of the old count was thin")
on(f"contour_parallel reads {A['corne contour_parallel']['area']:.1f} mm on Corne\n   and {A['wanaka200 contour_parallel']['area']:.1f} mm on wanaka200.", "reading 2")
ok(all(A[k]["area"] < A[f"{k.split()[0]} contour_parallel"]["area"] for k in A if "contour" not in k), "every research column below CP")
on(f"{A['corne D33']['box']:.1f} mm on Corne D33 and {A['wanaka200 D33']['box']:.1f} mm on wanaka200 D33.", "reading 3")
on(f"{A['corne D33']['red']:.1f} mm of the Corne D33\n   load count runs at a lowered feed", "reading 4")
pk_r = max(A[k]["peak"] for k in A if "contour" not in k); pk_c = max(A[k]["peak"] for k in A if "contour" in k)
on(f"The peak L is {pk_r:.3f} in the research columns\n   and {pk_c:.3f} in contour_parallel.", "reading 5a")
on(f"is {max(A[k]['new'] for k in A):.1f} mm or less in every run.", "reading 5b")
ok(A["corne D26"]["bf"] == 3556.0, "D26 Corne base feed"); on("The base feed of D26 Corne is 3556 mm/min, not 4000", "limit base feed")
on("§C32.2 (wanaka200 contour_parallel, \"body over 0.471\")", "C32 cite")
ok("| body over 0.471 (sim) | 1871.1 mm (helix 1839.8, linear+arc 31.4) | " + f"{A['wanaka200 contour_parallel']['old']:.1f} mm" in open(ROOT / "P3_C18_3d_oracle.md").read(), "C32.2 CP old count")
# ---- §4: the C12 run
RUN = open(H / "c12_test_run.txt").read()
ok("test result: FAILED. 7 passed; 2 failed;" in RUN, "C12 run result"); on("7 passed,\n2 failed", "C12 run text")
for t in ("the_geodesic_arm_feeds_its_turns_down_at_the_level", "the_research_arm_leaves_the_channel_to_the_edge_pass"):
    ok(f"test {t} ... FAILED" in RUN, f"{t} failed")
for lab, key in (("control AgentSearch", r"^control AgentSearch:"), ("geodesic_curv", r"^geodesic_curv:"), ("geodesic_curv_split", r"^geodesic_curv_split:"),
                 ("geodesic_local_spiral_pick, slope base", r"slope base: geodesic_local_spiral_pick"), ("geodesic_local_spiral_pick, slope field", r"slope field: geodesic_local_spiral_pick"),
                 ("geodesic_local_spiral_pick, shelf base", r"spiral_pick shelf base:"), ("geodesic_local_spiral_pick, shelf field", r"spiral_pick shelf field:")):
    ln = next(l for l in RUN.splitlines() if re.search(key, l))
    m = re.search(r"(\d+) over the screen 0\.546 \(sim peak [\d.]+\), (\d+) over the exact bound 0\.471 \(exact peak ([\d.]+)\)", ln)
    on(f"| {lab} | {m.group(1)} | {m.group(2)} | {m.group(3)} |", f"judge row {lab}")
on("(line 141) and two\n`type_complexity` errors (lines 1954 and 1980", "clippy text")
# ---- every number token of the README prose (code blocks and paths out)
txt = re.sub(r"```.*?```", " ", README, flags=re.S)
txt = re.sub(r"`[^`]*`", " ", txt)
import subprocess
head = subprocess.run(["git", "-C", str(REPO / ".claude/worktrees/adaptive-head"), "rev-parse", "--short=9", "HEAD"], capture_output=True, text=True).stdout.strip()
ok(head == "3181ae54f", f"adaptive-head HEAD {head}"); on("tree at 3181ae54f", "head text")
txt = txt.replace("3181ae54f", " ")
txt = re.sub(r"\b(?:fix|C|D|P|F|G|W)\d+[a-z]?\b|§\d+(?:\.\d+)?|2026-10-27|ASD-STE100", " ", txt)
CONST = {"0.01", "0.471", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0.05", "0.0083", "0.3", "1.05", "R", "0", "10", "30", "40",
         "240", "250", "25", "0.5", "1.5", "6.0", "5.46", "127", "256", "3.0", "9.0", "0.52", "1.80", "3556", "4000", "0.9"}
allowed = set(CONST)
for s in SEEN:
    allowed |= set(re.findall(r"\d+(?:\.\d+)?", s))
bad = sorted({t for t in re.findall(r"\d+(?:\.\d+)?", txt) if t not in allowed})
ok(not bad, f"README number tokens without a check: {bad}")
print(f"OK: {N[0]} checks")
