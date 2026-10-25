"""C35: assert every number on fixture3d_v3.html ("Geodesic roughing G2")
from the raw run files.

This script does NOT import fixture3d_v3.py. The G-code, trace, producer
and thick readers are the independent readers of assert_fixture3d_v2.py
(copied). It rebuilds each number from its source file and checks:
  1. the number equals the one in fixture3d_v3_numbers.json, and
  2. the page holds it at its place (the table cell, the list item).
Run: python3 -I assert_fixture3d_v3.py spec_v3.json
"""
import glob
import html
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RES = os.path.dirname(os.path.dirname(HERE))  # planning/adaptive_research_2026-10-23
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


def thick_check(run_dir, simart_dir, tp_glob):
    """Own C32 thick measure: samples of the toolpath with intent clearing_cut
    read by the sim over 0.471; for each, the earlier path (sample path
    densified to 0.1 mm) at Z <= sample Z + 0.5 stamps discs of R; the
    lattice (0.05 mm) points of the disc at the step midpoint that no such
    point covers give the swept width across the step; the step counts when
    that width is over 0.471. Per phase as producers_check."""
    import numpy as np
    from scipy.spatial import cKDTree
    tp = json.load(open(glob.glob(os.path.join(run_dir, tp_glob))[0]))
    tpid = int(re.search(r"tp_(\d+)_", tp_glob).group(1))
    n = tp["move_count"]
    ents = sorted((e for e in tp["semantic_trace"]["items"] if e["kind"] == "entry"), key=lambda e: e["move_start"])
    gen = {}
    for an in tp["debug_trace"]["annotations"]:
        mm = re.match(r"Pass (\d+) — ", an["label"])
        if mm:
            gen[int(mm.group(1))] = an["move_index"]
    pairs = sorted((gen[e["params"]["values"]["pass_index"]], e["move_start"]) for e in ents)
    fin = lambda g: next((f for gi, f in pairs if gi >= g), n)
    lv = [fin(an["move_index"]) for an in tp["debug_trace"]["annotations"] if an["label"].startswith("Adaptive Z")]
    wl = [fin(an["move_index"]) for an in tp["debug_trace"]["annotations"] if an["label"] == "Waterline cleanup"]
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
    smp = [s for s in samples_of(simart_dir) if s["toolpath_id"] == tpid]
    pos = np.array([s["position"] for s in smp])
    dens, owner = [pos[:1]], [np.zeros(1, int)]
    for k in range(1, len(pos)):
        seg = pos[k] - pos[k - 1]
        cnt = max(1, int(math.ceil(math.hypot(seg[0], seg[1]) / 0.1)))
        f = (np.arange(1, cnt + 1) / cnt)[:, None]
        dens.append(pos[k - 1] + f * seg)
        owner.append(np.full(cnt, k))
    dens = np.vstack(dens)
    owner = np.concatenate(owner)
    lat = np.arange(-3.0, 3.0 + 0.025, 0.05)
    lx, ly = np.meshgrid(lat, lat)
    disc = lx ** 2 + ly ** 2 <= 9.0
    lx, ly = lx[disc], ly[disc]
    cellsz = 9.0
    bucket = {}
    for i, (cx, cy) in enumerate(zip(np.floor(dens[:, 0] / cellsz).astype(int), np.floor(dens[:, 1] / cellsz).astype(int))):
        bucket.setdefault((cx, cy), []).append(i)
    bucket = {k: np.array(v) for k, v in bucket.items()}
    acc = {"perimeter": 0.0, "body": 0.0, "waterline": 0.0, "unplaced": 0.0}
    for k in range(1, len(smp)):
        s = smp[k]
        if not s["is_cutting"] or s["source_intent"] != "clearing_cut" or s["engagement"]["radial_woc_fraction"] <= 0.471:
            continue
        a, b = pos[k - 1, :2], pos[k, :2]
        mid = (a + b) / 2
        d = b - a
        dl = max(math.hypot(d[0], d[1]), 1e-9)
        nx, ny = -d[1] / dl, d[0] / dl
        bx, by = int(math.floor(mid[0] / cellsz)), int(math.floor(mid[1] / cellsz))
        idx = [bucket[(i, j)] for i in range(bx - 1, bx + 2) for j in range(by - 1, by + 2) if (i, j) in bucket]
        pts = np.column_stack([mid[0] + lx, mid[1] + ly])
        free = np.ones(len(pts), bool)
        if idx:
            idx = np.concatenate(idx)
            idx = idx[(owner[idx] <= k - 1) & (dens[idx, 2] <= pos[k, 2] + 0.5)]
            if len(idx):
                dd, _ = cKDTree(dens[idx, :2]).query(pts, distance_upper_bound=3.0 + 1e-12)
                free = ~(dd <= 3.0)
        if not free.any():
            continue
        proj = lx[free] * nx + ly[free] * ny
        if (proj.max() - proj.min()) / 6.0 > 0.471:
            m = s["move_index"]
            acc[(ph[m] if m < n else None) or "unplaced"] += math.hypot(d[0], d[1])
    tot = sum(acc.values())
    return {"total_mm": round(tot, 1), "perimeter": round(acc["perimeter"], 1) if sl else None,
            "body": round(acc["body"], 1), "waterline": round(acc["waterline"], 1), "unplaced": round(acc["unplaced"], 1)}




def load_mm(simart_dir, tpid, cell):
    """The D-FIX6 load rule, with the over lengths in mm."""
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
    return {"cut_mm": round(tot, 1), "over_mm": round(o1, 1), "over_pct": round(100 * o1 / tot, 2),
            "screen": round(screen, 4), "over_screen_mm": round(o2, 1), "over_screen_pct": round(100 * o2 / tot, 2)}


def bar_cells(run_dir):
    """Own reading of the four hard bars and their page cells."""
    s = json.load(open(os.path.join(run_dir, "summary.json")))
    err = open(os.path.join(run_dir, "stderr.txt")).read()
    out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(run_dir, "stdout.txt")).read())
    ok = '<b class="pass">PASS</b>'
    bad = '<b class="fail">FAIL</b>'
    col = (s["collision_count"], s["rapid_collision_count"], s["collision_checks_failed"])
    cells = [f'{ok if col == (0, 0, 0) else bad} '
             f'{col[0]} feed / {col[1]} rapid / {col[2]} failed checks']
    isl = []
    for ln in err.splitlines():
        m = re.search(r'"([^"]+)": the rough leaves (\d+) island\(s\) standing.*?(\d+) of them against a wall: (\d+\.\d+) mm2', ln)
        if m:
            isl.append(f"{html.escape(m.group(1))}: {int(m.group(2))} islands ({int(m.group(3))} at a wall), {float(m.group(4)):.1f} mm²")
    cells.append('<b class="fail">FLAGGED</b> ' + "; ".join(isl) if isl else ok)
    n = sum(1 for ln in err.splitlines() if "gouge" in ln.lower() or "deeper than" in ln or "fed link cuts" in ln)
    cells.append(f'{ok if n == 0 else bad} {n} line(s)')
    g = re.search(r"generation plan finished steps=(\d+) simulations=\d+ blocked=(\d+)", out)
    good = int(g.group(2)) == 0 and all(t["move_count"] > 0 for t in s["per_toolpath"])
    cells.append(f'{ok if good else bad} {g.group(1)} steps, {g.group(2)} blocked')
    return cells


def main():
    spec = json.load(open(sys.argv[1]))
    for fx in spec["fixtures"]:
        for arm in fx["arms"]:
            SIMART[arm["dir"]] = arm["simart"]
    got = json.load(open(os.path.join(HERE, "fixture3d_v3_numbers.json")))
    raw = open(os.path.join(RES, "fixture3d_v3.html")).read()
    page = html.unescape(raw)
    checks = 0
    derived = {}   # (fixture, column) -> numbers this script re-derived from the raw files

    def row_in(cells):
        nonlocal checks
        needle = "".join(f"<td>{c}</td>" for c in cells)
        assert needle in raw, needle[:300]
        checks += 1

    assert f"<title>{spec['title']}</title>" in raw and spec["title"] == "Geodesic roughing G2"
    lead = open(os.path.join(HERE, "lead_reading.html")).read()
    assert f'<div id="lead-reading">{lead}</div>' in raw, "the lead's reading is not on the page verbatim"
    checks += 2
    for fx in spec["fixtures"]:
        gf = got["fixtures"][fx["name"]]
        arms = fx["arms"]
        f0 = fx["ops"][0]["comment"]
        tpid = int(re.search(r"tp_(\d+)_", fx["ops"][0]["tp_glob"]).group(1))
        assert list(gf["arms"]) == [a["label"] for a in arms], "column order"
        assert [a["role"] for a in arms][0] == "main control" and arms[-1]["role"] == "secondary control"
        checks += 2
        cols = {k: [] for k in ("bars", "over", "prod", "thick", "screen", "plunge", "kd", "air", "cut", "time", "open")}
        for a in arms:
            ga = gf["arms"][a["label"]]
            s = json.load(open(os.path.join(a["dir"], "summary.json")))
            tot = {"cycle_time_s": round(s["total_runtime_s"]), "cutting_mm": round(s["total_cutting_distance_mm"]),
                   "air_cut_pct": round(s["air_cut_pct_of_total_runtime"], 1)}
            for k, v in tot.items():
                assert ga[k] == v, (fx["name"], a["label"], k)
                checks += 1
            ats = []
            for o in fx["ops"]:
                per, at, _, _ = derive_full(a["dir"], o["tp_glob"], o["comment"])
                ats.append(at)
                assert ga["ops"][o["comment"]] == per, (fx["name"], a["label"], o["comment"], ga["ops"][o["comment"]], per)
                checks += len(per)
                row = (f"<tr><td>{html.escape(o['comment'])}</td><td>{per['entries']}</td><td>{per['keepdown_links']}</td>"
                       f"<td>{per['keepdown_mm']:.0f}</td><td>{per['cut_xy_mm']:.0f}</td></tr>")
                assert row in raw, (fx["name"], a["label"], row)
                checks += 1
            for lab in gf["levels_shown"]:
                im = ats[0](lab)
                assert ga["images"][lab] == im, (fx["name"], a["label"], lab)
                sent = (f"At {lab}: {im['entries']} red ring(s) where the cutter goes down, "
                        f"{im['keepdown_mm']:.0f} mm of orange keep-down travel at depth, {im['cut_mm']:.0f} mm of blue cutting.")
                assert sent in page, sent
                tag = re.sub(r"[^A-Za-z0-9]+", "_", f"{fx['name']}_{a['label']}_{gf['levels_shown'].index(lab)}").strip("_")
                assert os.path.exists(os.path.join(HERE, "img", f"{tag}.png")) and f'src="phase3/fix8/img/{tag}.png"' in raw
                checks += 5
            if a["simart"] != a["dir"]:
                assert open(os.path.join(a["simart"], "program.nc"), "rb").read() == open(os.path.join(a["dir"], "program.nc"), "rb").read(), (a["label"], "rerun plan differs")
                checks += 1
            cell = json.load(open(os.path.join(a["simart"], "summary.json")))["simulation_resolution"]["mm"]
            ld = load_mm(a["simart"], tpid, cell)
            for k, v in ld.items():
                assert ga["load"][k] == v, (fx["name"], a["label"], k, ga["load"][k], v)
            checks += len(ld)
            pr = producers_check(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
            assert ga["producers"] == pr, (fx["name"], a["label"], ga["producers"], pr)
            assert abs(pr["perimeter"] or 0) + pr["body"] + pr["waterline"] + pr["unplaced"] - ld["over_mm"] < 0.35, (a["label"], "split sum")
            checks += 5
            th = thick_check(a["dir"], a["simart"], fx["ops"][0]["tp_glob"])
            th["pct"] = round(100 * th["total_mm"] / ld["cut_mm"], 2)
            assert ga["thick"] == th, (fx["name"], a["label"], ga["thick"], th)
            checks += 6
            err = open(os.path.join(a["dir"], "stderr.txt")).read()
            derived[(fx["name"], a["label"])] = {
                "thick_pct": th["pct"], "thick_body": th["body"], "time": tot["cycle_time_s"],
                "plunges": ga["ops"][f0]["entries"],
                "gouge_lines": sum(1 for ln in err.splitlines() if "gouge" in ln.lower() or "deeper than" in ln or "fed link cuts" in ln),
                "rapid": s["rapid_collision_count"]}
            cols["bars"].append(bar_cells(a["dir"]))
            checks += 4
            cols["over"].append(f"{ld['over_mm']:.1f} mm = {ld['over_pct']:.2f}% of {ld['cut_mm']:.1f} mm cut")
            cols["prod"].append("{} / {:.1f} / {:.1f} mm".format("—" if pr["perimeter"] is None else f"{pr['perimeter']:.1f}", pr["body"], pr["waterline"]))
            cols["thick"].append("{:.1f} mm = {:.2f}% ({} / {:.1f} / {:.1f} mm)".format(th["total_mm"], th["pct"],
                                 "—" if th["perimeter"] is None else f"{th['perimeter']:.1f}", th["body"], th["waterline"]))
            cols["screen"].append(f"{ld['over_screen_mm']:.1f} mm = {ld['over_screen_pct']:.2f}% (screen {ld['screen']:.3f})")
            p0 = ga["ops"][f0]
            cols["plunge"].append(f"{p0['entries']}")
            cols["kd"].append(f"{p0['keepdown_links']}, {p0['keepdown_mm']:.0f} mm")
            cols["air"].append(f"{tot['air_cut_pct']}% of runtime")
            cols["cut"].append(f"{tot['cutting_mm']} mm")
            cols["time"].append(f"{tot['cycle_time_s']} s")
            if a.get("openings"):
                out = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(a["dir"], "stdout.txt")).read())
                ops_ = [(float(z), int(r), d) for z, r, d in re.findall(r'C31 region opening z=([\d.]+) region=(\d+) .*?decision="([^"]+)"', out)]
                src = os.path.join(RES, "phase3", "fix7", f"{'corne' if fx['name'] == 'corne' else 'wanaka'}_openings.txt")
                ref = [(float(z), int(r), d) for z, r, d in re.findall(r'C31 region opening z=([\d.]+) region=(\d+) .*?decision="([^"]+)"', open(src).read())]
                assert ops_ == ref, (a["label"], "openings differ from", src)
                tags = a.get("opening_tags", {})
                grown = sum(1 for _, _, d in ops_ if d == "grown, no sweep")
                items = "<br>".join(f"Z {z:.2f} region {r}: {html.escape(d)}" + (f" ({html.escape(tags[f'{z:.2f}/{r}'])})" if f"{z:.2f}/{r}" in tags else "")
                                    for z, r, d in ops_)
                cols["open"].append(f"{grown} of {len(ops_)} grown<br>{items}")
                checks += 2 + len(ops_)
                for t in tags.values():
                    assert t.split(",")[0] in open(os.path.join(RES, "DEFECTS.md")).read()
                    checks += 1
            else:
                cols["open"].append("—")
        for i in range(4):
            row_in([c[i] for c in cols["bars"]])
        for k in ("over", "prod", "thick", "screen", "open", "plunge", "kd", "air", "cut", "time"):
            row_in(cols[k])
        if fx["name"] == "corne":
            d18 = json.load(open(os.path.join(RES, "phase3/fix7/islands_d18.json")))
            d20 = json.load(open(os.path.join(RES, "phase3/fix7/islands_d20.json")))
            near = lambda i, j: abs(i["x"] - j["x"]) <= 0.35 and abs(i["y"] - j["y"]) <= 0.35 and abs(i["height_mm"] - j["height_mm"]) <= 0.35
            un = lambda A, B: [i for i in A["islands"] if not any(near(i, j) for j in B["islands"])]
            r1, cp = "3D Rough 1", "3D Rough 1 (copy)"
            new = sorted(un(d20[r1], d18[r1]), key=lambda i: -i["area_mm2"])
            assert new[0]["area_mm2"] == 412.1 and new[0]["height_mm"] == 14.5 and new[0]["at_wall"] and new[1]["area_mm2"] < 10
            gone = [i for i in un(d18[r1], d20[r1]) if not i["at_wall"]]
            big = max(gone, key=lambda i: i["area_mm2"])
            cpn = un(d20[cp], d18[cp])
            mv = max(cpn, key=lambda i: i["area_mm2"])
            twin = [j for j in d18[cp]["islands"] if (j["height_mm"], j["width_mm"], j["area_mm2"]) == (mv["height_mm"], mv["width_mm"], mv["area_mm2"])]
            assert len(twin) == 1
            rest = [i for i in cpn if i is not mv]
            for needle in (f"D20 adds one real wall island: {new[0]['height_mm']:.1f} mm high, {new[0]['area_mm2']:.1f} mm² at ({new[0]['x']:.1f}, {new[0]['y']:.1f}).",
                           f"The D18 free island ({big['height_mm']:.1f} mm high, {big['area_mm2']:.1f} mm² at ({big['x']:.1f}, {big['y']:.1f})) is not in D20.",
                           f"the {mv['height_mm']:.2f} mm high, {mv['area_mm2']:.1f} mm² wall island is in both runs",
                           f"from ({twin[0]['x']:.1f}, {twin[0]['y']:.1f}) to ({mv['x']:.1f}, {mv['y']:.1f})",
                           f"The other {len(rest)} islands that the match rule calls new in D20 are each {max(i['area_mm2'] for i in rest):.1f} mm² or less."):
                assert needle in page, needle
                checks += 1
            checks += 4
            txt = open(os.path.join(RES, "phase3/c33/c33_output.txt")).read()
            blk = [ln for ln in txt.split("by cause:")[1].split("\n\n")[0].splitlines()[1:]]
            total = 0.0
            for ln in blk:
                m = re.match(r"\s+(.+?)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+sum\s+([\d.]+)$", ln)
                v = [float(m.group(k)) for k in range(2, 7)]
                assert abs(sum(v[:4]) - v[4]) < 0.15, ln
                total += v[4]
                needle = f"<tr><td>{html.escape(m.group(1))}</td>" + "".join(f"<td>{x:.1f}</td>" for x in v) + "</tr>"
                assert needle in raw, needle
                checks += 2
            th20 = gf["arms"]["zigzag_pick D20"]["thick"]["body"]
            assert abs(total - th20) < 0.15, (total, th20)   # the cause table covers the whole thick body load
            checks += 1
            lay = re.search(r"median ([\d.]+) mm, p10 ([\d.]+), p90 ([\d.]+)", open(os.path.join(RES, "phase3/c33/c33_layer.txt")).read())
            needle = f"median {float(lay.group(1)):.2f} mm (p10 {float(lay.group(2)):.2f}, p90 {float(lay.group(3)):.2f}; phase3/c33/c33_layer.txt)"
            assert needle in page, needle
            checks += 1

    # The lead's reading: every number re-derived from its source.
    C, W = "corne", "wanaka200"
    cp, d18, d20, ag = "contour_parallel", "zigzag_pick D18", "zigzag_pick D20", "AgentSearch"
    v = lambda f, l, k: derived[(f, l)][k]
    c33 = {}
    for ln in open(os.path.join(RES, "phase3/c33/c33_output.txt")).read().split("by cause:")[1].split("\n\n")[0].splitlines()[1:]:
        m = re.match(r"\s+(.+?)\s+[\d.]+\s+[\d.]+\s+[\d.]+\s+[\d.]+\s+sum\s+([\d.]+)$", ln)
        c33[m.group(1)] = float(m.group(2))
    step = c33["draped wrap below a higher same-level pass (shoulder step)"] + c33["wrap at the level below a higher same-level pass (step)"]
    lay = float(re.search(r"median ([\d.]+) mm", open(os.path.join(RES, "phase3/c33/c33_layer.txt")).read()).group(1))
    claims = [
        f"falls from {v(C, d18, 'thick_pct'):.2f}% to {v(C, d20, 'thick_pct'):.2f}% on Corne and from {v(W, d18, 'thick_pct'):.2f}% to {v(W, d20, 'thick_pct'):.2f}% on wanaka200",
        f"contour_parallel reads {v(C, cp, 'thick_pct'):.2f}% and {v(W, cp, 'thick_pct'):.2f}%",
        f"Corne goes from {v(C, d18, 'time')} s to {v(C, d20, 'time')} s; contour_parallel takes {v(C, cp, 'time')} s",
        f"wanaka200 goes from {v(W, d18, 'time')} s to {v(W, d20, 'time')} s; contour_parallel takes {v(W, cp, 'time')} s",
        f"has {v(C, d20, 'plunges')} plunges on Corne and {v(W, d20, 'plunges')} on wanaka200. contour_parallel has {v(C, cp, 'plunges')} and {v(W, cp, 'plunges')}",
        f"fails the gouge row on Corne ({v(C, cp, 'gouge_lines')} lines) and the collision row on wanaka200 ({v(W, cp, 'rapid')} rapid)",
        f"Of its {v(C, d20, 'thick_body'):.1f} mm, {step:.1f} mm is a thin step (median {lay:.2f} mm above the tip)",
        f"is the fastest column on wanaka200 ({v(W, ag, 'time')} s), with a thick over-load of {v(C, ag, 'thick_pct'):.2f}% on Corne and {v(W, ag, 'thick_pct'):.2f}% on wanaka200",
    ]
    # "fastest on wanaka200" holds, and it is not the fastest on Corne
    assert v(W, ag, 'time') == min(v(W, c, 'time') for c in (cp, d18, d20, ag)), "AgentSearch fastest on wanaka200"
    assert v(C, ag, 'time') > min(v(C, c, 'time') for c in (cp, d18, d20, ag)), "AgentSearch not fastest on Corne"
    checks += 2
    for cl in claims:
        assert cl in lead, ("lead reading", cl)
        checks += 1
    # the research arm passes collisions, gouge and generation on both parts
    for f in (C, W):
        cells = bar_cells([a for x in spec["fixtures"] if x["name"] == f for a in x["arms"] if a["label"] == d20][0]["dir"])
        assert all(cells[i].startswith('<b class="pass">PASS</b>') for i in (0, 2, 3)), (f, cells)
        checks += 1
    assert v(C, d20, "gouge_lines") == 0 and v(W, d20, "rapid") == 0
    checks += 1
    defects = open(os.path.join(RES, "DEFECTS.md")).read()
    c22 = open(os.path.join(RES, "P3_C22_3d_turns.md")).read()
    c18 = open(os.path.join(RES, "P3_C18_3d_oracle.md")).read()
    for tag in ("G-ARCENDHOOK", "G-RINGGROW", "G-PICKEXIT"):
        assert f"| {tag} " in defects and f"<b>{tag}</b> (DEFECTS.md)" in raw, tag
        checks += 1
    assert "## C33. Corne after C31" in c18 and "<b>C33 outline-wrap load</b>" in raw
    assert "### 11.6 C31 part 1: built" in c22 and "Part 2 (the held\nwaterline skim) is not built" in c22 and "<b>C31 part 2 (waterline skim)</b>" in raw
    assert "### 11.11 The H1 guard on paper" in c22 and "<b>Flat-floor design (H1 guard)</b>" in raw
    checks += 3
    # No ranking and no aggregate score on the page.
    for word in ("rank", "score:", "winner", "best arm"):
        assert word not in page.lower().replace("no ranking", "").replace("not a ranking", "").replace("the c29 pick", ""), word
    checks += 1
    print(f"fixture3d_v3: all {checks} checks hold")


if __name__ == "__main__":
    main()
