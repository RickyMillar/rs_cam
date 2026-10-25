"""Assert every number on fixture3d_v2.html from the raw run files.

This script does NOT import fixture3d_v2.py. It parses the G-code, the
semantic trace, summary.json and stderr.txt with its own code, rebuilds each
number, and checks:
  1. that number equals the one in fixture3d_v2_numbers.json, and
  2. the page text holds it at its place (the table row of each op, the
     totals rows, and the sentence under each image).
Run: python3 -I assert_fixture3d_v2.py spec.json
"""
import glob
import html
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RES = os.path.dirname(os.path.dirname(HERE))
TOK = re.compile(r"([GXYZ])(-?\d+(?:\.\d+)?)")


def op_lines(path, comment):
    lines = open(path).read().splitlines()
    i = lines.index(f"({comment})") + 1
    cmds = []
    for ln in lines[i:]:
        t = ln.strip()
        if t.startswith("("):
            if cmds and not t.startswith("(feeds"):
                break
            continue
        if t and t[0] in "MTS":
            if cmds:
                break
            continue
        if t.startswith(("G0", "G1", "G2", "G3")) or (t and t[0] in "XYZ"):
            cmds.append(t)
    return cmds


def moves_of(cmds):
    g, p, out = 0, (0.0, 0.0, 0.0), []
    for c in cmds:
        w = {k: float(v) for k, v in TOK.findall(c)}
        g = int(w.get("G", g))
        q = (w.get("X", p[0]), w.get("Y", p[1]), w.get("Z", p[2]))
        out.append((g, p, q))
        p = q
    return out


def d2(a, b):
    return math.hypot(b[0] - a[0], b[1] - a[1])


SIMART = {}   # run dir -> its simulation dir (from the spec)
_SAMPLES = {}


def samples_of(simart_dir):
    if simart_dir not in _SAMPLES:
        _SAMPLES[simart_dir] = json.load(open(os.path.join(simart_dir, "simulation.json")))["trace"]["samples"]
    return _SAMPLES[simart_dir]


def derive_full(run_dir, tp_glob, comment):
    tp = json.load(open(glob.glob(os.path.join(run_dir, tp_glob))[0]))
    n = tp["move_count"]
    cmds = op_lines(os.path.join(run_dir, "program.nc"), comment)
    assert len(cmds) - n in (1, 2), (run_dir, comment, len(cmds), n)
    mv = moves_of(cmds)[1:n + 1]
    items = tp["semantic_trace"]["items"]
    ent = [it for it in items if it["kind"] == "entry"]
    top = [e for e in ent if not any(
        o is not e and o["move_start"] <= e["move_start"] and e["move_end"] <= o["move_end"]
        and o["move_end"] - o["move_start"] > e["move_end"] - e["move_start"] for o in ent)]
    link = set()
    rings = []
    for e in top:
        g0 = [i for i in range(e["move_start"], min(e["move_end"], n - 1) + 1) if mv[i][0] == 0]
        if "keep-down link" in e["label"]:
            link |= {i for i in range(e["move_start"], min(e["move_end"], n - 1) + 1) if mv[i][0] != 0}
        elif g0:
            rings.append(g0[-1])
    planes = sorted({float(it["label"].split()[-1]) for it in items if it["kind"] == "depth_level"}, reverse=True)
    flat = {}
    for g, a, b in mv:
        if g != 0 and d2(a, b) > 1e-6 and abs(b[2] - a[2]) < 1e-6:
            flat[round(b[2], 2)] = flat.get(round(b[2], 2), 0) + 1
    best = max((sum(flat.get(round(p + o, 2), 0) for p in planes), o)
               for o in {round(z - p, 2) for z in flat for p in planes})[1]
    ncp = [round(p + best, 2) for p in planes]
    lev = [None] * n
    for i, (g, a, b) in enumerate(mv):
        if g != 0:
            k = next((k for k, z in enumerate(ncp) if z <= b[2] + 0.01), len(ncp) - 1)
            lev[i] = f"Z {planes[k]:.2f}"
    nxt = None
    for i in range(n - 1, -1, -1):
        if mv[i][0] == 0:
            lev[i] = nxt
        else:
            nxt = lev[i]
    tpid = int(re.search(r"tp_(\d+)_", tp_glob).group(1))
    first = {}
    for sm in samples_of(SIMART[run_dir]):
        if sm["toolpath_id"] == tpid:
            first.setdefault(sm["move_index"], sm["source_intent"])
    kind = []
    for i, (g, a, b) in enumerate(mv):
        if g == 0:
            kind.append("rapid")
        elif i in first:
            kind.append("link" if first[i] == "linking" else "cut")
        else:
            kind.append("link" if i in link else "cut")
    link = {i for i, k in enumerate(kind) if k == "link"}
    per_op = {
        "moves": n,
        "entries": sum(1 for e in top if "keep-down link" not in e["label"]),
        "keepdown_links": sum(1 for i, k in enumerate(kind) if k == "link" and (i == 0 or kind[i - 1] != "link")),
        "keepdown_mm": round(sum(d2(a, b) for (g, a, b), k in zip(mv, kind) if k == "link"), 1),
        "cut_xy_mm": round(sum(d2(a, b) for (g, a, b), k in zip(mv, kind) if k == "cut"), 1),
    }

    def at(lab):
        return {
            "entries": sum(1 for i in rings if lev[i] == lab),
            "keepdown_mm": round(sum(d2(a, b) for i, ((g, a, b), k) in enumerate(zip(mv, kind))
                                     if k == "link" and lev[i] == lab and d2(a, b) >= 1e-9), 1),
            "cut_mm": round(sum(d2(a, b) for i, ((g, a, b), k) in enumerate(zip(mv, kind))
                                if k == "cut" and lev[i] == lab and d2(a, b) >= 1e-9), 1),
        }
    return per_op, at, lev, link


def derive(run_dir, tp_glob, comment):
    per, at, lev, link = derive_full(run_dir, tp_glob, comment)
    return per, at


def spacing_check(run_dir, tp_glob, comment, lab, centre):
    """Own ray spacing: 4 axis rays from `centre`, the cut moves (fed, XY
    travel, not in a keep-down-link span) at `lab`, crossings within 0.05 mm
    merged, gaps over 6 mm set apart."""
    tp = json.load(open(glob.glob(os.path.join(run_dir, tp_glob))[0]))
    n = tp["move_count"]
    mv = moves_of(op_lines(os.path.join(run_dir, "program.nc"), comment))[1:n + 1]
    per, at, lev, link = derive_full(run_dir, tp_glob, comment)
    kept, apart = [], 0
    for ux, uy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
        ts = []
        for i, (g, a, b) in enumerate(mv):
            if g == 0 or i in link or lev[i] != lab:
                continue
            ex, ey = b[0] - a[0], b[1] - a[1]
            den = ux * ey - uy * ex
            if abs(den) < 1e-12:
                continue
            wx, wy = a[0] - centre[0], a[1] - centre[1]
            t, u = (wx * ey - wy * ex) / den, (wx * uy - wy * ux) / den
            if t > 0 and 0 <= u <= 1:
                ts.append(t)
        ts.sort()
        ps = []
        for t in ts:
            if not ps or t - ps[-1] > 0.05:
                ps.append(t)
        for t0, t1 in zip(ps, ps[1:]):
            if t1 - t0 <= 6.0:
                kept.append(t1 - t0)
            else:
                apart += 1
    kept.sort()
    m = len(kept)
    med = kept[m // 2] if m % 2 else 0.5 * (kept[m // 2 - 1] + kept[m // 2])
    return {"n": m, "apart": apart, "median": round(med, 3), "min": round(kept[0], 3), "max": round(kept[-1], 3)}


def load_check(simart_dir, tpid, cell):
    screen = 0.3626 + (0.5 + cell + 0.1) / 6.0
    prev, tot, o1, o2 = None, 0.0, 0.0, 0.0
    for sm in samples_of(simart_dir):
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] == tpid and sm["is_cutting"] and sm["source_intent"] == "clearing_cut":
            r = sm["engagement"]["radial_woc_fraction"]
            tot += ln
            o1 += ln if r > 0.471 else 0.0
            o2 += ln if r > screen else 0.0
    return {"cut_mm": round(tot, 1), "over_exact_pct": round(100 * o1 / tot, 2),
            "screen": round(screen, 4), "over_screen_pct": round(100 * o2 / tot, 2)}


def normal_step_check(run_dir, tp_glob, comment, lab, centre, simart_dir, tpid):
    """Own normal step: pass moves = cut moves at `lab` (not keep-down link)
    with a sim sample over one sim cell / 6 mm, none within 3.05 mm of the
    centre; from each midpoint the normal both ways, nearest other pass
    move (not itself or a sequence neighbour) beyond 0.05 mm and within 6 mm;
    a side with none is set apart; length-weighted median and 10th / 90th percentiles."""
    sim = json.load(open(os.path.join(simart_dir, "simulation.json")))
    floor = sim["resolution_mm"] / 6.0
    eng = {sm["move_index"] for sm in sim["trace"]["samples"]
           if sm["toolpath_id"] == tpid and sm["is_cutting"] and sm["engagement"]["radial_woc_fraction"] > floor}
    tp = json.load(open(glob.glob(os.path.join(run_dir, tp_glob))[0]))
    n = tp["move_count"]
    mv = moves_of(op_lines(os.path.join(run_dir, "program.nc"), comment))[1:n + 1]
    _, _, lev, link = derive_full(run_dir, tp_glob, comment)
    seg = [(i, a, b) for i, (g, a, b) in enumerate(mv)
           if g != 0 and i not in link and lev[i] == lab and i in eng and d2(a, b) >= 1e-6
           and min(d2(a, centre), d2(b, centre)) > 3.05]
    out, apart = [], 0
    for i, a, b in seg:
        L = d2(a, b)
        tx, ty = (b[0] - a[0]) / L, (b[1] - a[1]) / L
        mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
        for nx, ny in ((-ty, tx), (ty, -tx)):
            best = None
            for j, p, q in seg:
                if abs(j - i) <= 1:
                    continue
                if min(p[0], q[0]) > mx + 6 or max(p[0], q[0]) < mx - 6 or min(p[1], q[1]) > my + 6 or max(p[1], q[1]) < my - 6:
                    continue
                ex, ey = q[0] - p[0], q[1] - p[1]
                den = nx * ey - ny * ex
                if abs(den) < 1e-12:
                    continue
                t = ((p[0] - mx) * ey - (p[1] - my) * ex) / den
                u = ((p[0] - mx) * ny - (p[1] - my) * nx) / den
                if 0.05 < t <= 6.0 and 0 <= u <= 1 and (best is None or t < best):
                    best = t
            if best is None:
                apart += 1
            else:
                out.append((best, L))
    out.sort()
    tot = sum(w for _, w in out)

    def q(f):
        acc = 0.0
        for v, w in out:
            acc += w
            if acc >= f * tot:
                return v
        return out[-1][0]
    return {"n": len(out), "apart": apart, "median": round(q(0.5), 3), "p10": round(q(0.1), 3), "p90": round(q(0.9), 3)}


def producers_check(run_dir, simart_dir, tp_glob):
    """Own producer split: per move, the phase from the trace annotations
    (level start, waterline start, and for agent-slice arms the level's
    second entry as the body start); the over-0.471 XY length of fed linear
    and arc samples per phase."""
    tp = json.load(open(glob.glob(os.path.join(run_dir, tp_glob))[0]))
    tpid = int(re.search(r"tp_(\d+)_", tp_glob).group(1))
    n = tp["move_count"]
    ents = sorted((e for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"), key=lambda e: e["move_start"])
    gen = {}
    for a in tp["debug_trace"]["annotations"]:
        m = re.match(r"Pass (\d+) — ", a["label"])
        if m:
            gen[int(m.group(1))] = a["move_index"]
    pairs = sorted((gen[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    fin = lambda g: next((f for gi, f in pairs if gi >= g), n)
    lv = [fin(a["move_index"]) for a in tp["debug_trace"]["annotations"] if a["label"].startswith("Adaptive Z")]
    wl = [fin(a["move_index"]) for a in tp["debug_trace"]["annotations"] if a["label"] == "Waterline cleanup"]
    sl = any(s.get("kind") == "agent2d_region" for s in tp["debug_trace"]["spans"])
    ph = [None] * n
    for k, st in enumerate(lv):
        en = lv[k + 1] if k + 1 < len(lv) else n
        body = st
        if sl:
            inl = [e["move_start"] for e in ents if st <= e["move_start"] < wl[k]]
            body = inl[1] if len(inl) > 1 else st
        for m in range(st, en):
            ph[m] = "perimeter" if m < body else ("body" if m < wl[k] else "waterline")
    acc = {"perimeter": 0.0, "body": 0.0, "waterline": 0.0, "unplaced": 0.0}
    prev = None
    for sm in samples_of(simart_dir):
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] == tpid and sm["is_cutting"] and sm["source_intent"] == "clearing_cut" \
                and sm["engagement"]["radial_woc_fraction"] > 0.471:
            acc[(ph[sm["move_index"]] if sm["move_index"] < n else None) or "unplaced"] += ln
    return {"perimeter": round(acc["perimeter"], 1) if sl else None, "body": round(acc["body"], 1),
            "waterline": round(acc["waterline"], 1), "unplaced": round(acc["unplaced"], 1)}


def load_old_check(simart_dir, tpid):
    prev, tot, o1 = None, 0.0, 0.0
    for sm in samples_of(simart_dir):
        p = sm["position"]
        ln = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev else 0.0
        prev = p
        if sm["toolpath_id"] == tpid and sm["is_cutting"] and sm["cut_kinematics"] in ("linear", "arc"):
            tot += ln
            o1 += ln if sm["engagement"]["radial_woc_fraction"] > 0.471 else 0.0
    return {"cut_mm": round(tot, 1), "over_exact_pct": round(100 * o1 / tot, 2)}


def main():
    spec = json.load(open(sys.argv[1]))
    for fx in spec["fixtures"]:
        for arm in fx["arms"]:
            SIMART[arm["dir"]] = arm["simart"]
    got = json.load(open(os.path.join(HERE, "fixture3d_v2_numbers.json")))
    page = html.unescape(open(os.path.join(RES, "fixture3d_v2.html")).read())
    checks = 0
    for fx in spec["fixtures"]:
        gf = got["fixtures"][fx["name"]]
        for arm in fx["arms"]:
            if not os.path.exists(os.path.join(arm["dir"], "summary.json")):
                assert arm["label"] not in gf["arms"], arm["label"]
                continue
            ga = gf["arms"][arm["label"]]
            s = json.load(open(os.path.join(arm["dir"], "summary.json")))
            tot = {"cycle_time_s": round(s["total_runtime_s"]), "cutting_mm": round(s["total_cutting_distance_mm"]),
                   "rapid_mm": round(s["total_rapid_distance_mm"]),
                   "air_cut_pct": round(s["air_cut_pct_of_total_runtime"], 1),
                   "collisions": s["collision_count"], "rapid_collisions": s["rapid_collision_count"]}
            for k, v in tot.items():
                assert ga[k] == v, (fx["name"], arm["label"], k, ga[k], v)
                checks += 1
            for needle in (f"{tot['cycle_time_s']} s", f"{tot['cutting_mm']} / {tot['rapid_mm']} mm",
                           f"{tot['air_cut_pct']}% of runtime", f"{tot['collisions']} ({tot['rapid_collisions']})"):
                assert needle in page, (fx["name"], arm["label"], needle)
                checks += 1
            ats = []
            for o in fx["ops"]:
                per, at = derive(arm["dir"], o["tp_glob"], o["comment"])
                ats.append(at)
                assert ga["ops"][o["comment"]] == per, (fx["name"], arm["label"], o["comment"], ga["ops"][o["comment"]], per)
                checks += len(per)
                row = (f"<tr><td>{o['comment']}</td><td>{per['entries']}</td><td>{per['keepdown_links']}</td>"
                       f"<td>{per['keepdown_mm']:.0f}</td><td>{per['cut_xy_mm']:.0f}</td></tr>")
                assert row in page, (fx["name"], arm["label"], row)
                checks += 1
            for lab in gf["levels_shown"]:
                im = ats[0](lab)
                assert ga["images"][lab] == im, (fx["name"], arm["label"], lab, ga["images"][lab], im)
                checks += 3
                sent = (f"At {lab}: {im['entries']} red ring(s) where the cutter goes down, "
                        f"{im['keepdown_mm']:.0f} mm of orange keep-down travel at depth, "
                        f"{im['cut_mm']:.0f} mm of blue cutting.")
                assert sent in page, (fx["name"], arm["label"], sent)
                checks += 1
        if "mid_level" in gf:
            mid = gf["mid_level"]
            arms = {a["label"]: a for a in fx["arms"]}

            def rings_at(a):
                tp = json.load(open(glob.glob(os.path.join(a["dir"], fx["ops"][0]["tp_glob"]))[0]))
                n = tp["move_count"]
                mv = moves_of(op_lines(os.path.join(a["dir"], "program.nc"), fx["ops"][0]["comment"]))[1:n + 1]
                _, _, lev, _ = derive_full(a["dir"], fx["ops"][0]["tp_glob"], fx["ops"][0]["comment"])
                ent = [it for it in tp["semantic_trace"]["items"] if it["kind"] == "entry"]
                top = [e for e in ent if not any(o is not e and o["move_start"] <= e["move_start"] and e["move_end"] <= o["move_end"]
                       and o["move_end"] - o["move_start"] > e["move_end"] - e["move_start"] for o in ent)]
                out = []
                for e in top:
                    if "keep-down link" in e["label"]:
                        continue
                    g0 = [i for i in range(e["move_start"], min(e["move_end"], n - 1) + 1) if mv[i][0] == 0]
                    if g0 and lev[g0[-1]] == mid:
                        out.append((g0[-1], mv[g0[-1]][2]))
                return [p for i, p in sorted(out)]
            a0 = rings_at(arms["AgentSearch"])[0]
            c = min(rings_at(arms["geodesic_local_zigzag"]), key=lambda p: (p[0] - a0[0]) ** 2 + (p[1] - a0[1]) ** 2)
            assert [round(c[0], 3), round(c[1], 3)] == gf["centre"], (fx["name"], c, gf["centre"])
            checks += 1
            tpid = int(re.search(r"tp_(\d+)_", fx["ops"][0]["tp_glob"]).group(1))
            for lab, ga in gf["arms"].items():
                a = arms[lab]
                sp = spacing_check(a["dir"], fx["ops"][0]["tp_glob"], fx["ops"][0]["comment"], mid, c)
                assert ga["spacing"] == sp, (fx["name"], lab, ga["spacing"], sp)
                assert open(os.path.join(a["simart"], "program.nc"), "rb").read() == open(os.path.join(a["dir"], "program.nc"), "rb").read(), (lab, "rerun plan differs")
                cell = json.load(open(os.path.join(a["simart"], "summary.json")))["simulation_resolution"]["mm"]
                ld = load_check(a["simart"], tpid, cell)
                assert ga["load"] == ld, (fx["name"], lab, ga["load"], ld)
                lo = load_old_check(a["simart"], tpid)
                assert ga["load_old"] == lo, (fx["name"], lab, ga["load_old"], lo)
                side = (f"<tr><td>{lab}</td><td>{lo['over_exact_pct']:.2f}% of {lo['cut_mm']:.0f} mm</td>"
                        f"<td>{ld['over_exact_pct']:.2f}% of {ld['cut_mm']:.0f} mm</td>"
                        f"<td>{ld['over_exact_pct'] - lo['over_exact_pct']:+.2f} points</td></tr>")
                assert side in page, (fx["name"], lab, side)
                checks += 3
                checks += len(sp) + len(ld) + 1
                ns = normal_step_check(a["dir"], fx["ops"][0]["tp_glob"], fx["ops"][0]["comment"], mid, c, a["simart"], tpid)
                assert ga["normal_step"] == ns, (fx["name"], lab, ga["normal_step"], ns)
                checks += len(ns)
                row = (f"<tr><td>{lab}</td><td>{ns['median']:.2f} mm</td>"
                       f"<td>{ns['p10']:.2f} to {ns['p90']:.2f} mm (n {ns['n']}; {ns['apart']} over 6 mm set apart)</td>"
                       f"<td>{ld['over_exact_pct']:.2f}%</td><td>{ld['over_screen_pct']:.2f}% (screen {ld['screen']:.3f})</td>")
                pr = producers_check(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
                assert ga["producers"] == pr, (fx["name"], lab, ga["producers"], pr)
                per = "—" if pr["perimeter"] is None else f"{pr['perimeter']:.0f}"
                row += f"<td>{per} / {pr['body']:.0f} / {pr['waterline']:.0f} mm</td></tr>"
                checks += 4
                assert row in page, (fx["name"], lab, row)
                old = (f"<tr><td>{lab}</td><td>{sp['median']:.2f} mm</td>"
                       f"<td>{sp['min']:.2f} to {sp['max']:.2f} mm (n {sp['n']}; {sp['apart']} gaps over 6 mm set apart)</td></tr>")
                assert old in page, (fx["name"], lab, old)
                checks += 2
            ag, gz = gf["arms"]["AgentSearch"], gf["arms"]["geodesic_local_zigzag"]
            cp = gf["arms"]["contour_parallel"]
            for needle in (f"contour_parallel puts its passes {cp['normal_step']['median']:.2f} mm apart and the geodesic zigzag "
                           f"{gz['normal_step']['median']:.2f} mm (stepover {fx['stepover_first_op']:.1f} mm)",
                           f"the geodesic zigzag cuts {gz['load']['cut_mm'] / cp['load']['cut_mm']:.2f} times the contour_parallel length",
                           f"{gz['load']['over_exact_pct']:.2f}% of it reads over 0.471 against {cp['load']['over_exact_pct']:.2f}% for contour_parallel",
                           (f"AgentSearch (secondary) is over the bound: {ag['load']['over_exact_pct']:.2f}% of its cut length"
                            if ag['load']['over_exact_pct'] > 0 else "AgentSearch (secondary) is not over the bound"),
                           f"({ag['load']['over_screen_pct']:.2f}% over the screen {ag['load']['screen']:.3f} for its sim cell)"):
                assert needle in page, (fx["name"], needle)
                checks += 1
    # The C29 pick sentence (spec text): the generation times of '3D Rough 1'
    # (tp debug_trace), the scored plunges of plan A (the "C29 pick" log
    # lines) and the trace's plunges per level, re-derived here.
    for fx in spec["fixtures"]:
        arms = {a["label"]: a for a in fx["arms"]}
        if fx["name"] != "corne" or "geodesic_local_zigzag_pick" not in arms:
            continue
        pk, zz = arms["geodesic_local_zigzag_pick"], arms["geodesic_local_zigzag"]
        gen = lambda d: json.load(open(glob.glob(os.path.join(d, "tp_1_*.json"))[0]))["debug_trace"]["summary"]["total_elapsed_us"] / 1e6
        log = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(pk["dir"], "stdout.txt")).read())
        scored = [int(m) for m in re.findall(r"C29 pick \(3D score\).*?plunges_a=(\d+)", log)]
        kept = re.findall(r"C29 pick \(3D score\).*?kept=\"?([AB])", log)
        _, _, lev, _ = derive_full(pk["dir"], "tp_1_*.json", "3D Rough 1")
        tp = json.load(open(glob.glob(os.path.join(pk["dir"], "tp_1_*.json"))[0]))
        n = tp["move_count"]
        mv = moves_of(op_lines(os.path.join(pk["dir"], "program.nc"), "3D Rough 1"))[1:n + 1]
        ent = [it for it in tp["semantic_trace"]["items"] if it["kind"] == "entry"]
        top = [e for e in ent if not any(o is not e and o["move_start"] <= e["move_start"] and e["move_end"] <= o["move_end"]
               and o["move_end"] - o["move_start"] > e["move_end"] - e["move_start"] for o in ent)]
        per = {}
        for e in top:
            if "keep-down link" in e["label"]:
                continue
            g0 = [i for i in range(e["move_start"], min(e["move_end"], n - 1) + 1) if mv[i][0] == 0]
            if g0:
                per[lev[g0[-1]]] = per.get(lev[g0[-1]], 0) + 1
        trace = [per.get(k, 0) for k in ("Z 17.00", "Z 11.00", "Z 5.00", "Z 0.50")]
        # The trace's plunge-entry items, placed by move order: each level runs
        # from its first cut move to the next level's first cut move.
        names = ("Z 17.00", "Z 11.00", "Z 5.00", "Z 0.50")
        kind_cut = [i for i, (g, a, b) in enumerate(mv) if g != 0]
        firsts = [min(i for i in kind_cut if lev[i] == k) for k in names]
        items_pe = [e for e in tp["semantic_trace"]["items"] if e["kind"] == "entry" and "plunge entry" in e["label"]]
        order = [0, 0, 0, 0]
        for e in items_pe:
            order[sum(1 for x in firsts[1:] if e["move_start"] >= x)] += 1
        assert kept == ["A", "A", "A", "A"], kept
        for needle in (f"generating '3D Rough 1' in {gen(pk['dir']):.2f} s against {gen(zz['dir']):.2f} s for zigzag",
                       f"its {scored[0]}, {scored[1]}, {scored[2]} and {scored[3]} plunges for A equal the body plunges",
                       f"{order[0]}, {order[1]}, {order[2]} and {order[3]} (the trace's plunge-entry items, placed by move order",
                       f"{trace[0]}, {trace[1]}, {trace[2]} and {trace[3]} (this page's red rings",
                       "G-PICKEXIT in DEFECTS.md",
                       f"({sum(trace)} plunges, "):
            assert needle in page, (fx["name"], "pick sentence", needle)
            checks += 1
        assert open(os.path.join(pk["dir"], "program.nc"), "rb").read() == open(os.path.join(zz["dir"], "program.nc"), "rb").read(), "pick != zigzag"
        checks += 2
    print(f"fixture3d_v2: all {checks} checks hold")


if __name__ == "__main__":
    main()
