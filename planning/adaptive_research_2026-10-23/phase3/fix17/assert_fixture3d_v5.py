"""C18/fix17: checks every number of fixture3d_v5.html. Each value is derived
again here from the run artefacts with code of its own (a G-code parser, a
trace counter, a stock-map reader, stderr / stdout regexes), compared with
numbers_v5.json, with the earlier reports (fix11 / fix12 / fix14 / fix15
audit files, P15_D_verify.md), and then found in the page. Last, every
number token in the page text must come from a derived value or from the
declared rule constants. Images: each src exists, each is under 1600 px
wide, and the page plus its images is under 16 MB."""
import glob, html, json, math, os, re, sys
import numpy as np
from scipy import ndimage
from PIL import Image

H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, ".."); ROOT = os.path.abspath(os.path.join(P3, ".."))
PAGE = open(os.path.join(ROOT, "fixture3d_v5.html")).read()
N = json.load(open(os.path.join(H, "numbers_v5.json")))
E = json.load(open(os.path.join(H, "entries_v5.json")))
P15 = open(os.path.join(ROOT, "P15_D_verify.md")).read()
CHECKS = [0]; SEEN = set()


def ok(cond, msg):
    CHECKS[0] += 1
    if not cond:
        raise SystemExit(f"FAIL: {msg}")


def on_page(s, msg):
    ok(s in PAGE, f"{msg}: {s!r} not on the page")
    SEEN.add(s)


COLS = ["contour_parallel", "zigzag_pick D20", "spiral_pick C35", "spiral_pick D24", "spiral_pick D25"]
RUNS = {"corne": ["fix6/corne/contour_parallel", "fix7/corne/geodesic_local_zigzag_pick", "fix12/corne", "fix14/corne", "fix15/corne_pick"],
        "wanaka200": ["fix6/wanaka200/contour_parallel", "fix7/wanaka200/geodesic_local_zigzag_pick", "fix12/wanaka200", "fix14/wanaka200", "fix15/wanaka200_pick"]}
OP = {"corne": "3D Rough 1", "wanaka200": "2 Back Rough"}
RAPID = {"corne": 4000.0, "wanaka200": 10000.0}      # machine max feed (C39)
EPS = 1e-3
TAG = lambda s: re.sub(r"[^A-Za-z0-9]+", "_", s).strip("_")


# ---------- an own G-code reader ----------
def gcode(path, op):
    L = open(path).read().splitlines(); i = L.index(f"({op})") + 1
    g, f, pos, out, started = 0, None, (0.0, 0.0, 0.0), [], False
    for ln in L[i:]:
        t = ln.strip()
        if not t:
            continue
        if t[0] == "(":
            if started and not t.startswith("(feeds"):
                break
            continue
        if t[0] in "MTS":
            if started:
                break
            continue
        if not (t[0] in "GXYZ"):
            continue
        w = dict((k, float(v)) for k, v in re.findall(r"([GXYZIJF])(-?\d+\.?\d*)", t))
        if "G" in w and int(w["G"]) not in (0, 1, 2, 3):
            continue
        started = True
        g = int(w.get("G", g)); f = w.get("F", f)
        q = (w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2]))
        out.append((g, pos, q, w.get("I", 0.0), w.get("J", 0.0), f)); pos = q
    return out


def arc(m):
    g, a, b, i, j, f = m
    cx, cy = a[0] + i, a[1] + j; r = math.hypot(a[0] - cx, a[1] - cy)
    a0, a1 = math.atan2(a[1] - cy, a[0] - cx), math.atan2(b[1] - cy, b[0] - cx)
    sw = ((a0 - a1) if g == 2 else (a1 - a0)) % (2 * math.pi) or 2 * math.pi
    return r, a0, a1, sw


def mlen(m):
    if m[0] in (2, 3):
        r, a0, a1, sw = arc(m); return math.hypot(r * sw, m[2][2] - m[1][2])
    return math.dist(m[1], m[2])


def dirs(m):
    g, a, b = m[0], m[1], m[2]
    if g in (2, 3):
        r, a0, a1, sw = arc(m); s = 1 if g == 3 else -1; dz = (b[2] - a[2]) / (r * sw) if r * sw > 0 else 0.0
        v = lambda t: np.array([-s * math.sin(t), s * math.cos(t), dz])
        return v(a0) / np.linalg.norm(v(a0)), v(a1) / np.linalg.norm(v(a1))
    d = np.subtract(b, a); n = np.linalg.norm(d)
    return (None, None) if n < EPS else (d / n, d / n)


part = np.load(os.path.join(P3, "fix9", "corne_part.npz"))
ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
CSIL = ndimage.binary_fill_holes(np.isfinite(ptop) & (ptop > 1e-6))


def inside(fx, x, y):
    if fx == "wanaka200":
        return 20 <= x <= 220 and 25 <= y <= 225
    i, j = int(round((x - px0) / pg)), int(round((y - py0) / pg))
    return 0 <= j < CSIL.shape[0] and 0 <= i < CSIL.shape[1] and bool(CSIL[j, i])


def derive(fx, k):
    run = os.path.join(P3, RUNS[fx][k]); col = COLS[k]
    tp = json.load(open(sorted(glob.glob(os.path.join(run, "tp_1_*.json")))[0]))
    allm = gcode(os.path.join(run, "program.nc"), OP[fx]); mv = allm[1:tp["move_count"] + 1]
    sim = json.load(open(os.path.join(run, "simulation.json")))
    off = (sim["stock_bbox_min"][0], sim["stock_bbox_min"][1]) if fx == "corne" else (0.0, 0.0)
    xy = lambda m: math.hypot(m[2][0] - m[1][0], m[2][1] - m[1][1])
    t = {"in": [0, 0], "out": [0, 0]}; prev = None
    for m in mv:
        if m[0] == 0:
            prev = None; continue
        u0, u1 = dirs(m)
        if u0 is None:
            continue
        if prev is not None and prev[1] and xy(m) >= EPS:
            ang = math.degrees(math.acos(max(-1.0, min(1.0, float(prev[0] @ u0)))))
            w = "in" if inside(fx, m[1][0] + off[0], m[1][1] + off[1]) else "out"
            t[w][0] += ang > 30 + 1e-6; t[w][1] += ang > 90 + 1e-6
        prev = (u1, xy(m) >= EPS)
    prog = sum(mlen(m) / (RAPID[fx] if m[0] == 0 else m[5]) * 60 for m in allm)
    plunge = sum(mlen(m) / m[5] * 60 for m in mv if m[0] != 0 and xy(m) < EPS and m[2][2] - m[1][2] < -EPS)
    err = open(os.path.join(run, "stderr.txt")).read()
    out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(run, "stdout.txt")).read())
    s = json.load(open(os.path.join(run, "summary.json")))
    pe = sum(1 for it in tp["semantic_trace"]["items"] if it["kind"] == "entry" and it["params"]["values"].get("style") == "plunge entry")
    lo = sum(1 for r in E[f"{fx} {col}"]["rows"] if r["style"] == "keep-down link" and r["descent_mm"] >= 1.0 and r["removed_mm3"] > 0.1)
    fed = re.findall(r'"([^"]+)": a fed link cuts stock[^\n]*?link move (\d+) removes (\d+\.\d+) mm at \(([^)]*)\)', err)
    deep = re.findall(r"deeper than this pass was planned to cut: (\d+\.\d+)% of cutting samples[^\n]*?peaking at (\d+\.\d+) mm", err)
    ts = None
    for ln in out.splitlines():
        if "generate_toolpath{index=0}" in ln and "C22 turn summary" in ln:
            ts = [int(v) for v in re.search(r"planned=(\d+) cut=(\d+) fed=(\d+) unfed=(\d+)", ln).groups()]; break
    isl = None
    for ln in err.splitlines():
        if f'"{OP[fx]}": the rough leaves' in ln:
            m = re.search(r"leaves (\d+) island\(s\).*?(\d+) of them against a wall: (\d+\.\d+) mm2", ln)
            isl = [int(m.group(1)), int(m.group(2)), float(m.group(3))]
    return {"turns_out30": t["out"][0], "turns_in30": t["in"][0], "rev_in": t["in"][1], "rev_out": t["out"][1],
            "plunge_s": plunge, "program_s": prog, "plunge_entries": pe, "lift_over": lo, "cycle_s": round(s["total_runtime_s"]),
            "collisions": [s["collision_count"], s["rapid_collision_count"], s["collision_checks_failed"]],
            "refused": s["gcode_export"]["refused"] is not None, "fed": fed, "deep": deep, "ts": ts, "isl": isl}


# ---------- earlier audit files (descent events) ----------
A11 = json.load(open(os.path.join(P3, "fix11", "audit_results.json")))
def earlier_descents(fx, k):
    if k < 2:
        return A11[f"{fx} {COLS[k]}"]["descent_events"]
    f = {2: f"fix12/audit_{fx}.json", 3: f"fix14/audit_{fx}.json", 4: f"fix15/audit_{'corne' if fx == 'corne' else 'wanaka200'}_pick.json"}[k]
    return json.load(open(os.path.join(P3, f)))["descent_events"]


# ---------- stock maps ----------
def stock(fx, col):
    m = np.load(os.path.join(H, f"stock_{fx}_{TAG(col)}.npz")); a = m["top"] - m["target"]; g = float(m["g"])
    res = {"area": round(float((a > 0.8).sum() * g * g))}
    if fx == "corne":
        X, Y = np.meshgrid(m["xs"], m["ys"])
        i = np.round((X - px0) / pg).astype(int); j = np.round((Y - py0) / pg).astype(int)
        v = (i >= 0) & (i < ptop.shape[1]) & (j >= 0) & (j < ptop.shape[0])
        t = np.zeros(X.shape); t[v] = np.nan_to_num(ptop[j[v], i[v]], nan=0.0, neginf=0.0)
        sil = ndimage.binary_fill_holes(t > 1e-6); d = ndimage.distance_transform_edt(~sil) * g
        res["band"] = round(float(((a > 0.8) & ~sil & (d > 3) & (d <= 9)).sum() * g * g))
        res["disc"] = [round(float(np.median(a[np.hypot(X - cx, Y - cy) <= 2.0])), 2) for cx, cy in N["image_facts"]["holes"]]
    return res


# P15 reported values (cross-check), read from the report text
def p15_row(label):
    m = re.search(r"^\| " + re.escape(label) + r" \|(.*)$", P15, re.M)
    return [c.strip() for c in m.group(1).split("|") if c.strip()]


fmt_pct = lambda p, s, t: f"{p:.1f} % ({s:.1f} of {t:.1f} s)"
D = {}
for fx in ("corne", "wanaka200"):
    for k, col in enumerate(COLS):
        d = derive(fx, k); n = N[f"{fx} {col}"]; st = stock(fx, col); D[(fx, col)] = (d, st)
        # against numbers_v5.json
        ok(n["descents"] == earlier_descents(fx, k), f"{fx} {col} descents vs earlier audit file")
        ok(n["lift_over"] == d["lift_over"], f"{fx} {col} lift-over")
        ok(n["plunge_entries"] == d["plunge_entries"], f"{fx} {col} plunge entries")
        ok([n["turns_in"][1], n["turns_out"][1], n["turns_in"][2], n["turns_out"][2]] == [d["turns_in30"], d["turns_out30"], d["rev_in"], d["rev_out"]], f"{fx} {col} turns")
        ok(abs(n["plunge_s"] - d["plunge_s"]) < 0.05 and abs(n["program_s"] - d["program_s"]) < 0.05, f"{fx} {col} program clock")
        pct = 100 * d["plunge_s"] / d["program_s"]
        ok(abs(n["plunge_pct"] - pct) < 0.05 + 1e-9 and f"{n['plunge_pct']:.1f}" == f"{pct:.1f}", f"{fx} {col} plunge share {pct}")
        ok(n["cycle_s"] == d["cycle_s"], f"{fx} {col} cycle")
        ok(n["collisions"] == d["collisions"] and n["export_refused"] == d["refused"], f"{fx} {col} collisions / export")
        ok(len(n["fed_links"]) == len(d["fed"]) == n["fed_link_lines"], f"{fx} {col} fed links")
        ok(n["turn_summary"] == d["ts"], f"{fx} {col} turn summary")
        if fx == "corne":
            ok(n["band_3_9_mm2"] == st["band"], f"{fx} {col} band")
            ok(n["islands"] == d["isl"], f"{fx} {col} islands")
            ok(N["image_facts"]["disc"][col] == st["disc"], f"{fx} {col} disc")
        # on the page
        e = html.escape
        on_page(f"<td>{n['descents']}</td>", f"{fx} {col} descents")
        on_page(f"<td>{d['lift_over']}</td>", f"{fx} {col} lift-over")
        on_page(f"<td>{d['plunge_entries']}</td>", f"{fx} {col} plunge entries")
        on_page(f"<td>{d['turns_in30']} / {d['turns_out30']} (all {d['turns_in30'] + d['turns_out30']})</td>", f"{fx} {col} turns")
        on_page(f"<td>{d['rev_in']} / {d['rev_out']} (all {d['rev_in'] + d['rev_out']})</td>", f"{fx} {col} reversals")
        on_page(f"<td>{' / '.join(map(str, d['ts'])) if d['ts'] else 'no line'}</td>", f"{fx} {col} turn summary")
        on_page(f"<td>{fmt_pct(round(pct, 1), d['plunge_s'], d['program_s'])}</td>", f"{fx} {col} plunge share")
        on_page(f"<td>{d['cycle_s']} s</td>", f"{fx} {col} cycle")
        on_page(f"<td>{st['area']} mm²</td>", f"{fx} {col} area")
        on_page(f"<td>{' / '.join(map(str, d['collisions']))}</td>", f"{fx} {col} collisions")
        if fx == "corne":
            on_page(f"<td>{st['band']} mm²</td>", f"{fx} {col} band")
            on_page(f"<td>{d['isl'][0]} / {d['isl'][1]} / {d['isl'][2]:.1f} mm²</td>", f"{fx} {col} islands")
        fedtxt = "0" if not d["fed"] else f"{len(d['fed'])}: " + "; ".join(f"{e(o)} move {mv} removes {float(mm):.2f} mm at ({e(p)})" for o, mv, mm, p in d["fed"])
        on_page(f"<td>{fedtxt}</td>", f"{fx} {col} fed links")
        on_page(f"<td>{'; '.join(f'{float(a):.1f} % of samples, peak {float(b):.2f} mm' for a, b in d['deep']) if d['deep'] else '0'}</td>", f"{fx} {col} deeper")
    on_page(f"<td>no</td>", f"{fx} export refused")

# against P15 (D20, C35 from the D21c table; D24 from its table; D25 from the text)
c = lambda fx, col: N[f"{fx} {col}"]
r = p15_row("Corne descents (audit)"); ok(r[:2] == [str(c("corne", "zigzag_pick D20")["descents"]), str(c("corne", "spiral_pick C35")["descents"])], "P15 Corne descents D20 C35")
r = p15_row("Corne 3–9 mm band (mm²)"); ok(r[:2] == [str(c("corne", "zigzag_pick D20")["band_3_9_mm2"]), str(c("corne", "spiral_pick C35")["band_3_9_mm2"])], "P15 band")
r = p15_row("wanaka planar turns > 30°"); ok(r[:2] == ["221", "188"] and [sum(x[1] for x in (c("wanaka200", k)["turns_in"], c("wanaka200", k)["turns_out"])) for k in ("zigzag_pick D20", "spiral_pick C35")] == [221, 188], "P15 wanaka turns")
for pat, val in ((r"Corne descents \(audit\) \| 1062 \| (\d+)", c("corne", "spiral_pick D24")["descents"]),
                 (r"Corne planar turns > 30° \| 1096 \| (\d+)", c("corne", "spiral_pick D24")["turns_in"][1] + c("corne", "spiral_pick D24")["turns_out"][1]),
                 (r"reversals (\d+);", c("corne", "spiral_pick D24")["turns_in"][2] + c("corne", "spiral_pick D24")["turns_out"][2]),
                 (r"descents \(audit\) 1291 → (\d+)", c("corne", "spiral_pick D25")["descents"]),
                 (r"over 30° 3007 → (\d+)", c("corne", "spiral_pick D25")["turns_in"][1] + c("corne", "spiral_pick D25")["turns_out"][1]),
                 (r"cycle 3405 → (\d+) s", c("corne", "spiral_pick D25")["cycle_s"]),
                 (r"wanaka: descents 2031 → (\d+)", c("wanaka200", "spiral_pick D25")["descents"]),
                 (r"turns over 30° 180 → (\d+)", c("wanaka200", "spiral_pick D25")["turns_in"][1]),
                 (r"cycle 3365 → (\d+) s", c("wanaka200", "spiral_pick D25")["cycle_s"])):
    ok(int(re.search(pat, P15).group(1)) == val, f"P15 {pat}")
ok(re.search(r"3–9 mm 455 → \*\*(\d+)\*\*", P15).group(1) == str(c("corne", "spiral_pick D24")["band_3_9_mm2"]), "P15 D24 band")
ok(re.search(r"whole box 4793 → (\d+)", P15).group(1) == str(D[("corne", "spiral_pick D24")][1]["area"]), "P15 D24 whole box")

# ---------- C40 ----------
C40 = json.load(open(os.path.join(P3, "fix15", "c40_join.json")))
ok(N["c40"] == C40, "c40 copy")
for run, ops in (("corne_pick", ["op0", "op1"]), ("wanaka200_pick", ["op0"])):
    lines = [ln for ln in open(os.path.join(P3, "fix15", run, "stdout_plain.txt")).read().splitlines() if "C40 entry summary" in ln]
    ok(len(lines) == len(ops), f"{run} C40 summary lines")
    for ln, op in zip(lines, ops):
        tot = int(re.search(r"entries=(\d+)", ln).group(1))
        kc = re.findall(r"\((\w+), (None|Some\(\w+\))\): (\d+)", ln.split("drop_1mm_by_cause")[0])
        ok(sum(int(v) for _, _, v in kc) == tot, f"{run} {op} split sums")
        oth = sum(int(v) for _, cse, v in kc if cse == "None"); lz = sum(int(v) for _, cse, v in kc if cse.startswith("Some(LargeZ"))
        o = C40[run][op]
        ok(o["entries"] == tot and o["other"] == oth and abs(o["other_share_pct"] - 100 * oth / tot) < 0.005 and abs(o["largez_share_pct"] - 100 * lz / tot) < 0.005, f"{run} {op} shares")
        ok({f"{k} {cse}": int(v) for k, cse, v in kc} == o["by_kind_cause"], f"{run} {op} split")
        on_page(f"<td>{tot}</td><td>{100 * lz / tot:.2f} %</td><td>{oth} ({100 * oth / tot:.2f} %)</td>", f"{run} {op} row")
        on_page(", ".join(f"{html.escape(k)} {v}" for k, v in o["by_kind_cause"].items()), f"{run} {op} split text")
    j = C40[run]["join_op1"]
    ok(sum(j["by_cause"].values()) == j["descents"] == j["matched"] and j["unmatched"] == 0, f"{run} join sums")
    ok(all(abs(j["share_pct"][k] - round(100 * v / j["descents"], 1)) < 1e-9 for k, v in j["by_cause"].items()), f"{run} join shares")
    ok(abs(j["other_share_pct"] - 100 * j["by_cause"].get("None", 0) / j["descents"]) < 0.005, f"{run} join other")
    ok(abs(j["largez_share_pct"] - 100 * sum(v for k, v in j["by_cause"].items() if "LargeZ" in k) / j["descents"]) < 0.05, f"{run} join largez")
    on_page(f"<td>{j['descents']} (matched {j['matched']}, unmatched {j['unmatched']})</td><td>{j['largez_share_pct']:.1f} %</td><td>{j['by_cause'].get('None', 0)} ({j['other_share_pct']:.2f} %)</td>", f"{run} join row")
    on_page(", ".join(f"{html.escape(k)} {v} ({j['share_pct'][k]:.1f} %)" for k, v in j["by_cause"].items()), f"{run} join split")
ok(C40["corne_pick"]["join_op1"]["descents"] - c("corne", "spiral_pick D25")["descents"] in range(-10, 1), "C40 join descents near the audit count")
on_page("The Other share is a known instrument gap: the waterline / clip gap of D25, fixed in D26.", "C40 note")

# ---------- the reading ----------
def slot(fx, col):
    yc, x0, x1, pl = {"corne": (-50.2, 57, 105, (17.0, 5.0)), "wanaka200": (124.9, 122, 220, (14.08,))}[fx]
    return sum(1 for r in E[f"{fx} {col}"]["rows"] if r["style"] == "keep-down link" and r["descent_mm"] >= 1.0 and r["removed_mm3"] > 0.1
               and abs(r["y"] - yc) < 1.5 and x0 <= r["x"] <= x1 and r["end_plane"] in pl)
dz = {col: D[("corne", col)][1]["disc"] for col in COLS}
ok(all(v == 3.5 for col in COLS[2:] for v in dz[col]) and all(v == 0.0 for col in COLS[:2] for v in dz[col]), "discs: C35/D24/D25 3.5, CP/D20 0")
hc = "; ".join(f"({x:.1f}, {y:.1f})" for x, y in N["image_facts"]["holes"])
on_page(f"At {hc} the median stock in a 2 mm disc is {dz['spiral_pick C35'][0]:.2f} mm above the target for C35, D24 and D25 (all three holes). contour_parallel and D20 read {dz['contour_parallel'][0]:.2f} mm.", "reading 1")
# the hole centres: centroids of the > 2.5 mm components of 20 to 100 mm2 on the C35 map that lie at the three holes
m = np.load(os.path.join(H, "stock_corne_spiral_pick_C35.npz")); g = float(m["g"]); lab, nl = ndimage.label((m["top"] - m["target"]) > 2.5)
cents = []
for q in range(1, nl + 1):
    jj, ii = np.nonzero(lab == q)
    if 20 < len(jj) * g * g < 100:
        cents.append((round(float(m["xs"][ii].mean()), 1), round(float(m["ys"][jj].mean()), 1)))
ok(all(tuple(h) in cents for h in N["image_facts"]["holes"]), f"hole centres are component centroids {cents}")
sd = {(fx, col): slot(fx, col) for fx in ("corne", "wanaka200") for col in COLS}
ok(sd[("corne", "zigzag_pick D20")] > 50 and sd[("wanaka200", "zigzag_pick D20")] > 40 and all(sd[(fx, col)] == 0 for fx in ("corne", "wanaka200") for col in COLS[2:]), f"slot {sd}")
on_page(f"D20 has {sd[('corne', 'zigzag_pick D20')]} lift-over descents on the slot row of Corne at Z 17 and Z 5, and {sd[('wanaka200', 'zigzag_pick D20')]} on wanaka200 at Z 14.08. C35, D24 and D25 have 0, 0, 0 on Corne and 0 on wanaka200 (D25).", "reading 2")
# link arcs at Z 17: an own read of the simulation samples
def links_box(rd):
    S = [s for s in json.load(open(os.path.join(P3, rd, "simulation.json")))["trace"]["samples"] if s["toolpath_id"] == 1]
    tot = 0.0
    for s0, s1 in zip(S[:-1], S[1:]):
        p, q = s0["position"], s1["position"]
        if s0["cut_kinematics"] == "rapid" or s1["cut_kinematics"] == "rapid" or math.hypot(q[0] - p[0], q[1] - p[1]) > 0.45 + 1e-9:
            continue
        if s1["source_intent"] == "linking" and abs(p[2] - 17) <= 0.01 and abs(q[2] - 17) <= 0.01 and 95 <= p[0] <= 130 and -70 <= p[1] <= -15:
            tot += math.hypot(q[0] - p[0], q[1] - p[1])
    return round(tot)
LB = {col: links_box(RUNS["corne"][k]) for k, col in enumerate(COLS) if k >= 2}
ok(LB == {k: N["image_facts"]["links_z17_box_mm"][k] for k in LB}, f"links box {LB}")
on_page(f"the linking steps on the plane are {LB['spiral_pick D24']} mm long for D24 and {LB['spiral_pick D25']} mm for D25 (C35: {LB['spiral_pick C35']} mm)", "reading 3")
ar = {col: D[("corne", col)][1]["area"] for col in COLS}
ok(ar["contour_parallel"] < min(ar[c_] for c_ in COLS[1:]) and max(ar["spiral_pick D24"], ar["spiral_pick D25"]) < min(ar["zigzag_pick D20"], ar["spiral_pick C35"]), "outer band order")
on_page(f"({ar['contour_parallel']} mm² over 0.8 mm in the box); D24 and D25 cut short runs in it ({ar['spiral_pick D24']} and {ar['spiral_pick D25']} mm²); D20 and C35 leave most of it ({ar['zigzag_pick D20']} and {ar['spiral_pick C35']} mm²)", "reading 4")
to = {col: D[("corne", col)][0]["turns_out30"] for col in COLS}
ok(to["spiral_pick D24"] > 2000 and to["spiral_pick D25"] > 2000, "more than 2000 outside turns")
ok(all(D[("wanaka200", col)][0]["turns_out30"] == 0 and D[("wanaka200", col)][0]["rev_out"] == 0 for col in COLS), "wanaka no outside junction")
on_page(f"They have more than 2000 turns over 30° outside the silhouette ({to['spiral_pick D24']} and {to['spiral_pick D25']}); C35 has {to['spiral_pick C35']}.", "reading 5")

# ---------- images ----------
srcs = sorted(set(re.findall(r'src="([^"]+)"', PAGE)))
ok(len(srcs) == 2 + 2 * 5 * 4, f"image count {len(srcs)}")
total = len(PAGE.encode())
for s in srcs:
    p = os.path.join(ROOT, s); ok(os.path.exists(p), f"{s} exists")
    w, h = Image.open(p).size; ok(w < 1600, f"{s} width {w}"); total += os.path.getsize(p)
    ok(f'href="{s}"' in PAGE, f"{s} linked")
ok(total < 16 * 1024 * 1024, f"total {total}")
CR = json.load(open(os.path.join(H, "crop_centres.json")))
for fx in CR:
    on_page(f"10 mm crop at ({CR[fx]['x']:.1f}, {CR[fx]['y']:.1f}), cuts from Z {CR[fx]['plane']:.2f} up one depth per pass", f"{fx} crop caption")
RN = json.load(open(os.path.join(H, "render_v4_numbers.json")))
for fx, show in (("corne", [17.0, 5.0, 0.5]), ("wanaka200", [19.54, 14.08, 8.62])):
    for k, z in enumerate(show):
        for col in COLS:
            ok(RN[f"plane_{fx}_{TAG(col)}_{k}"]["z"] == z, f"{fx} {col} plane {k} Z")
            on_page(f'<a href="phase3/fix17/img/plane_{fx}_{TAG(col)}_{k}.png"><img src="phase3/fix17/img/plane_{fx}_{TAG(col)}_{k}.png" alt="{html.escape(col)} strict plane Z {z:.2f}" loading="lazy"></a><figcaption>Z {z:.2f}</figcaption>', f"{fx} {col} plane {k}")
ok(CR == json.load(open(os.path.join(P3, "fix10", "crop_centres.json"))), "crop centres = v4")

# ---------- every number token of the page text ----------
txt = html.unescape(re.sub(r"<style>.*?</style>|<[^>]+>", " ", PAGE, flags=re.S))
txt = re.sub(r"(?:phase3/)?fix\d+|fixture3d_v5|[A-Z]\d+[a-z]?\b|v\d\b|tp_1|bands_d25|entries\.py|audit\.py", " ", txt)
CONST = {"0.01", "1.0", "0.1", "30", "90", "20", "220", "25", "225", "3", "9", "0.8", "2", "95", "130", "−70", "−15", "-70", "-15",
         "17", "5", "14.08", "19.54", "4.0", "10", "2000", "6.00", "5.46", "1600", "16", "1", "0"}
allowed = set(CONST)
for s in SEEN:
    allowed |= set(re.findall(r"[−-]?\d+(?:\.\d+)?", html.unescape(re.sub(r"<[^>]+>", " ", s))))
bad = [t for t in re.findall(r"[−-]?\d+(?:\.\d+)?", txt) if t not in allowed and t.lstrip("−-") not in allowed]
ok(not bad, f"number tokens without a check: {sorted(set(bad))[:40]}")
print(f"OK: {CHECKS[0]} checks; page + images {total / 1e6:.2f} MB")
