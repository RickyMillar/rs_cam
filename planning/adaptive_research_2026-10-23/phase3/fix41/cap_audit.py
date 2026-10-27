"""D60: the feed-cap audit. v2: the G-code join is by position, not by line number: for each capped move, the end point
of its last trace sample is matched to a G-code line end in the window k-8 .. k+12 (XY within 0.02 mm; the G1 line whose XY segment lies within TOL (default 0.05; 0.35 on Corne,
whose trace samples sit up to the 0.3 mm segment-merge tolerance off the emitted lines) of every sample of the move, the
closest such line; arcs are not joined; Z not used). v1 joined line k to move k and drifted where the program has extra
lines (wanaka s3.5: 3 surplus lines). The feed-cap audit (closes D58 item 3). trace.capped_moves (integration step 7.4c) lists every simulated move with a
feed cap: (toolpath_id, move_index, feed_cap_mm_min). Two checks per capped move:
- trace: every sample joined by (toolpath_id, move_index) has feed_rate_mm_min <= cap (+ 1e-6);
- G-code: the emitted F of that move (program line k of the op = move k, offset by the move count, D35's map) <= cap
  (+ 0.5, the post rounds F to an integer).
Also: capped moves per toolpath, cap range, and how many capped moves cut stock. Usage: cap_audit.py <run dir> <tag> [G-code to sample XY offset dx,dy; Corne 0,-109.3155 (D58 frame check)] [TOL mm]"""
import collections, glob, json, re, sys
from pathlib import Path
d = Path(sys.argv[1]); tag = sys.argv[2]; DX, DY = (float(v) for v in sys.argv[3].split(',')) if len(sys.argv) > 3 else (0.0, 0.0); TOL = float(sys.argv[4]) if len(sys.argv) > 4 else 0.05; H = Path(__file__).resolve().parent
sim = json.load(open(d / "simulation.json")); tr = sim["trace"]; Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
cap = {(c["toolpath_id"], c["move_index"]): c["feed_cap_mm_min"] for c in tr.get("capped_moves", [])}
names = {}
for f in glob.glob(str(d / "tp_*.json")):
    j = json.load(open(f)); names[j["toolpath_id"]] = (j["toolpath_name"], j["move_count"])
def op_moves(name):
    out, on, F, x, y, z = [], False, None, 0.0, 0.0, 50.0
    pat = name.replace("(", "[").replace(")", "]")
    for ln in open(d / "program.nc"):
        ln = ln.strip()
        if ln.startswith("("):
            if re.match(r"^\((3D Rough|Waterline|2 Back Rough)", ln): on = ln == f"({pat})"
            continue
        if not on or not ln or ln[0] != "G": continue
        m = re.search(r"F(-?[\d.]+)", ln)
        if m: F = float(m.group(1))
        for kk, v in re.findall(r"([XYZ])(-?[\d.]+)", ln):
            if kk == "X": x = float(v)
            elif kk == "Y": y = float(v)
            else: z = float(v)
        out.append((ln.split()[0], F, x, y, z))
    return out
res = {"run": str(d), "capped_moves": len(cap), "per_toolpath": {}}
samples = collections.defaultdict(list)
for s in tr["samples"]:
    k = (s["toolpath_id"], s["move_index"])
    if k in cap: samples[k].append(s)
for tp, (name, n) in sorted(names.items()):
    keys = [k for k in cap if k[0] == tp]
    if not keys: res["per_toolpath"][name] = {"capped": 0}; continue
    mv = op_moves(name); off = 0 if len(mv) == n else 1
    over_t = [(k[1], max(s["feed_rate_mm_min"] for s in samples[k]), cap[k]) for k in keys if samples[k] and max(s["feed_rate_mm_min"] for s in samples[k]) > cap[k] + 1e-6]
    zo = sorted(samples[k][-1]["position"][2] - mv[off + k[1]][4] for k in keys if samples[k] and off + k[1] < len(mv)
                and abs(samples[k][-1]["position"][0] - mv[off + k[1]][2]) < 0.02 and abs(samples[k][-1]["position"][1] - mv[off + k[1]][3]) < 0.02)
    zoff = zo[len(zo) // 2] if zo else 0.0
    ZTOL = 1e9  # XY only inside the local window; the Z offset is reported, not used (the median above can come from drifted lines)
    join = {}
    def segd(px, py, i):
        if i == 0: return 9e9
        ax, ay, bx, by = mv[i - 1][2] + DX, mv[i - 1][3] + DY, mv[i][2] + DX, mv[i][3] + DY; dx, dy = bx - ax, by - ay; L2 = dx * dx + dy * dy
        t = 0.0 if L2 < 1e-12 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / L2))
        return ((ax + t * dx - px) ** 2 + (ay + t * dy - py) ** 2) ** 0.5
    for k in keys:
        if not samples[k]: continue
        i0 = off + k[1]; pts = [s["position"] for s in samples[k]]
        c = [i for i in range(max(1, i0 - 8), min(len(mv), i0 + 13)) if mv[i][0] == "G1" and max(segd(p[0], p[1], i) for p in pts) < TOL]
        if c: join[k] = min(c, key=lambda i: (max(segd(p[0], p[1], i) for p in pts), abs(i - i0)))
    over_g = [(k[1], mv[i][1], cap[k], i - off - k[1]) for k, i in join.items() if mv[i][1] > cap[k] + 0.5]
    gfed = len(join)
    caps = sorted(cap[k] for k in keys)
    res["per_toolpath"][name] = {"capped": len(keys), "moves": n, "map_offset": off, "joined_samples": sum(len(samples[k]) for k in keys),
                                 "capped moves with no sample": sum(1 for k in keys if not samples[k]),
                                 "capped moves that remove stock": sum(1 for k in keys if any(s["removed_volume_est_mm3"] > Q for s in samples[k])),
                                 "cap min/median/max": [round(caps[0], 1), round(caps[len(caps) // 2], 1), round(caps[-1], 1)],
                                 "trace samples over cap (moves)": len(over_t), "trace over examples": over_t[:5],
                                 "G-code fed lines joined by position": gfed, "joins off the line-k map": sum(1 for k, i in join.items() if i != off + k[1]), "G-code F over cap (moves)": len(over_g), "G-code over examples": over_g[:5]}
print(json.dumps(res, indent=1)); json.dump(res, open(H / f"cap_audit_{tag}.json", "w"), indent=1)
