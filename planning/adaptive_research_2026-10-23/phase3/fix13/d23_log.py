"""D23 step 13: read the tracing lines of the four fix13 runs (stdout_plain.txt).

- Region blocks: a block starts at a 'C31 region opening' or 'C38 G-b' line
  for a new (op, z, region); a second line for the same region joins the
  block. In a pick run the block ends at its 'C29 pick (3D score)' line
  (kept="A"|"B"); a G-b region has no pick line.
- Plans in a pick block: each plan's 2D plan writes one 'F-038: dropped short
  cut groups' debug line; a 'C35b hand-off' line just before it belongs to the
  same plan. With two such markers, the lines from the first marker's plan
  are plan A, the rest plan B. Corridor-opening lines with no marker are
  split in two equal halves, and the halves must be equal line for line.
  Only the kept plan's lines are counted. A block that cannot be split is
  reported, not guessed.
- C37 re-entries are counted by join= and by z; their x, y are classified by
  the rules of c38/classify_outer_descents.py (Corne only): the hole-filled
  silhouette of fix9/corne_part.npz, outside distance bands, beyond the mesh
  bbox.
Writes d23_log.json."""
import collections, json, re, sys
from pathlib import Path
import numpy as np
from scipy import ndimage

H = Path(__file__).resolve().parent
P3 = H.parent
KV = re.compile(r"(\w+)=(\"[^\"]*\"|\S+)")
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
dist_out = ndimage.distance_transform_edt(~sil) * g
BANDS = [(0, 1.5), (1.5, 3), (3, 4.5), (4.5, 6), (6, 99)]


def where(x, y):
    c, r = int(round((x - x0) / g)), int(round((y - y0) / g))
    if not (0 <= r < sil.shape[0] and 0 <= c < sil.shape[1]):
        return "beyond mesh bbox"
    if sil[r, c]:
        return "inside silhouette"
    d = float(dist_out[r, c])
    return next(f"outside {lo}-{hi}" for lo, hi in BANDS if lo <= d < hi)


def events(path):
    ev = []
    for ln in open(path):
        m = re.search(r"generate_toolpath\{index=(\d)\}: rs_cam_core::adaptive3d::\w+: (.*)$", ln.rstrip())
        if not m:
            continue
        op, msg = int(m.group(1)), m.group(2)
        kind = next((k for k in ("C31 region opening", "C38 G-b", "C29 pick", "C37 re-entry", "C35b hand-off",
                                 "C38 corridor opening", "F-038: dropped short cut groups") if msg.startswith(k)), None)
        if kind:
            kv = dict((k, v.strip('"')) for k, v in KV.findall(msg))
            if kind == "C35b hand-off":
                kv["z"] = msg.rsplit("z=", 1)[1]
            ev.append({"op": op, "kind": kind, "kv": kv, "msg": msg})
    return ev


def blocks(ev, pick):
    out, cur = [], None
    for e in ev:
        if e["kind"] in ("C31 region opening", "C38 G-b"):
            key = (e["op"], float(e["kv"]["z"]), int(e["kv"]["region"]))
            if cur is not None and not cur["closed"] and (cur["op"], cur["z"], cur["region"]) == key:
                cur["head"].append(e)   # the same region again (plan B's G-b line)
            else:
                cur = {"op": e["op"], "z": key[1], "region": key[2], "head": [e], "body": [], "closed": False, "kept": None}
                out.append(cur)
        elif e["kind"] == "C29 pick":
            cur["kept"] = e["kv"]["kept"]; cur["closed"] = True
        elif cur is not None:
            cur["body"].append(e)
    return out


def kept_lines(b, pick):
    body = b["body"]
    if not pick:
        return body, "single plan"
    mk = [i for i, e in enumerate(body) if e["kind"] == "F-038: dropped short cut groups"]
    starts = []
    for i in mk:
        j = i
        while j > 0 and body[j - 1]["kind"] == "C35b hand-off":
            j -= 1
        starts.append(j)
    if len(mk) == 2:
        a, bb = body[:starts[1]], body[starts[1]:]
        how = "two markers"
    elif len(mk) == 0 and len(body) % 2 == 0 and all(e["kind"] == "C38 corridor opening" for e in body):
        n = len(body) // 2; a, bb = body[:n], body[n:]
        assert [e["msg"] for e in a] == [e["msg"] for e in bb], ("corridor halves differ", b["z"], b["region"])
        how = "corridor halves (equal)"
    elif len(body) == 0:
        return [], "empty"
    else:
        return None, f"UNSPLIT: {len(mk)} markers, {len(body)} lines"
    return (a if b["kept"] == "A" else bb), how


res = {}
for run, pick in (("corne", False), ("corne_pick", True), ("wanaka200", False), ("wanaka200_pick", True)):
    ev = events(H / run / "stdout_plain.txt")
    bl = blocks(ev, pick)
    reent, hand, corr, unsplit, gb, c31 = [], [], [], [], [], []
    for b in bl:
        for h in b["head"]:
            (gb if h["kind"] == "C38 G-b" else c31).append({"op": b["op"], "z": b["z"], "region": b["region"], **{k: h["kv"].get(k) for k in ("decision", "planned_mm", "region_total")}})
        lines, how = kept_lines(b, pick)
        if lines is None:
            unsplit.append({"op": b["op"], "z": b["z"], "region": b["region"], "why": how}); continue
        for e in lines:
            if e["kind"] == "C37 re-entry":
                x, y = float(e["kv"]["x"]), float(e["kv"]["y"])
                reent.append({"op": e["op"], "z": float(e["kv"]["z"]), "region": b["region"], "join": e["kv"]["join"], "producer": e["kv"]["producer"],
                              "x": x, "y": y, "where": where(x, y) if run.startswith("corne") else None})
            elif e["kind"] == "C35b hand-off":
                hand.append({"op": e["op"], "z": b["z"], "region": b["region"], "msg": e["msg"]})
            elif e["kind"] == "C38 corridor opening":
                corr.append({"op": e["op"], "z": b["z"], "region": b["region"], **{k: float(e["kv"][k]) for k in ("loops", "slot_steps", "cut_mm", "centre_area_mm2")}})
    r = {"blocks": len(bl), "unsplit": unsplit, "c31": c31, "gb": gb, "handoffs": hand, "corridor": corr,
         "reentries_total": len(reent),
         "reentries_by_op_join": {f"op{k[0]} {k[1]}": v for k, v in sorted(collections.Counter((e["op"], e["join"]) for e in reent).items())},
         "reentries_by_op_z": {f"op{k[0]} z{k[1]}": v for k, v in sorted(collections.Counter((e["op"], e["z"]) for e in reent).items())},
         "reentries_by_op_where": {f"op{k[0]} {k[1]}": v for k, v in sorted(collections.Counter((e["op"], e["where"]) for e in reent).items())},
         "kept": [(b["op"], b["z"], b["region"], b["kept"]) for b in bl if pick]}
    res[run] = r
    print(f"== {run}: blocks {len(bl)}, unsplit {unsplit}")
    print("  re-entries", len(reent), r["reentries_by_op_join"])
    print("  by z", r["reentries_by_op_z"])
    if run.startswith("corne"):
        print("  where", r["reentries_by_op_where"])
    print("  hand-offs", len(hand), "corridor openings", len(corr), "G-b lines", len(gb))
json.dump(res, open(H / "d23_log.json", "w"), indent=1)
