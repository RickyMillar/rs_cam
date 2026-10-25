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


def derive(run_dir, tp_glob, comment):
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
    kind = ["rapid" if g == 0 else ("link" if i in link else "cut") for i, (g, a, b) in enumerate(mv)]
    per_op = {
        "moves": n,
        "entries": sum(1 for e in top if "keep-down link" not in e["label"]),
        "keepdown_links": sum(1 for e in top if "keep-down link" in e["label"]),
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
    return per_op, at


def main():
    spec = json.load(open(sys.argv[1]))
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
    print(f"fixture3d_v2: all {checks} checks hold")


if __name__ == "__main__":
    main()
