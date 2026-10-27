"""D59: which emitted feeds exceed feed_max_rate (3000), and do they cut stock. Reads program.nc (the G1/G2/G3 lines with
F over 3000, modal F, of op 1 and the copy op, mapped to the toolpath move index: line k of an op = move k, D35's map,
checked by the per-op move count) and the trace samples of that move (simulation.json): is_cutting, removed volume over the
quantum (cell^2 * step, fix11), radial engagement, axial engagement, MRR. Usage: feedmax.py <run dir> <fmax>"""
import collections, glob, json, re, sys
from pathlib import Path
d = Path(sys.argv[1]); FMAX = float(sys.argv[2])
sim = json.load(open(d / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
T = sim["trace"]["samples"]
def op_moves(name):
    out, on, f, x, y, z = [], False, None, 0.0, 0.0, 50.0
    for ln in open(d / "program.nc"):
        ln = ln.strip()
        if ln.startswith("("):
            if re.match(r"^\((3D Rough|Waterline|2 Back Rough)", ln): on = ln == f"({name})"
            continue
        if not on or not ln or ln[0] != "G": continue
        m = re.search(r"F(-?[\d.]+)", ln)
        if m: f = float(m.group(1))
        g = ln.split()[0]; nx, ny, nz = x, y, z
        for k, v in re.findall(r"([XYZ])(-?[\d.]+)", ln):
            if k == "X": nx = float(v)
            elif k == "Y": ny = float(v)
            else: nz = float(v)
        out.append((g, f, (x, y, z), (nx, ny, nz))); x, y, z = nx, ny, nz
    return out
res = {}
for tp, name in ((1, "3D Rough 1"), (3, "3D Rough 1 [copy]")):
    mv = op_moves(name); n = json.load(open(glob.glob(str(d / f"tp_{tp}_*.json"))[0]))["move_count"]
    off = 0 if len(mv) == n else 1
    S = collections.defaultdict(list)
    for s in T:
        if s["toolpath_id"] == tp: S[s["move_index"]].append(s)
    hi = [k for k, (g, f, a, b) in enumerate(mv[off:off + n]) if g in ("G1", "G2", "G3") and f and f > FMAX + 1e-6]
    c = collections.Counter(); fmax = 0; ex = []; rmax = 0; vol = 0.0; tfeed = collections.Counter()
    for k in hi:
        g, f, a, b = mv[off + k]; fmax = max(fmax, f); ss = S.get(k, [])
        rem = [s for s in ss if s["removed_volume_est_mm3"] > Q]
        cut = [s for s in ss if s["is_cutting"]]
        tfeed[round(max((s["feed_rate_mm_min"] for s in ss), default=0))] += 1
        if not ss: c["no trace sample"] += 1
        elif rem:
            c["removes stock"] += 1; vol += sum(s["removed_volume_est_mm3"] for s in rem)
            r = max((s["engagement"] if isinstance(s["engagement"], (int, float)) else 0) for s in rem); rmax = max(rmax, max(s.get("axial_engagement_mm", 0) for s in rem))
            if len(ex) < 5: ex.append({"move": k, "F": f, "from": [round(v, 3) for v in a], "to": [round(v, 3) for v in b], "removed_mm3": round(sum(s["removed_volume_est_mm3"] for s in rem), 3),
                                      "radial_max": round(max(s["engagement"].get("radial_woc_fraction", 0) if isinstance(s["engagement"], dict) else 0 for s in rem), 3),
                                      "axial_max": round(max(s.get("axial_engagement_mm", 0) for s in rem), 3), "trace_feed": max(s["feed_rate_mm_min"] for s in rem)})
        elif cut: c["is_cutting, removes <= quantum (air)"] += 1
        else: c["not cutting"] += 1
    res[name] = {"moves": n, "map_offset": off, "lines_over": len(hi), "max_F": fmax, "classes": dict(c), "removed_mm3": round(vol, 1), "axial_max_mm": rmax,
                 "trace_feed_of_those_moves (top)": dict(tfeed.most_common(4)), "examples": ex}
    print(name, json.dumps(res[name], indent=1))
print("sample keys of engagement:", type(T[0]["engagement"]).__name__, T[0]["engagement"] if not isinstance(T[0]["engagement"], (int, float)) else "")
json.dump(res, open(Path(__file__).resolve().parent / f"feedmax_{d.name}.json", "w"), indent=1)
