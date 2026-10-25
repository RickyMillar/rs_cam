"""D20b: parse every standing island of the cap-raised CLI runs (d20b/d18,
d20b/d20) and list the islands of one run with no island of the other
within one cell (0.3 mm) in X and Y (plus the 0.05 mm print rounding) and
with a height within one cell."""
import json
import re
import sys

B = "/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix7"
CELL, ROUND = 0.3, 0.05
ISL = re.compile(r" (\d+\.\d+) mm high, (\d+\.\d+) mm wide, (\d+\.\d+) mm2 at \((-?\d+\.\d+), (-?\d+\.\d+)\)( at a wall)?;")


def parse(tag, op):
    for ln in open(f"{B}/d20b/{tag}/stderr.txt"):
        if f'"{op}": the rough leaves' in ln:
            head = re.search(r"leaves (\d+) island\(s\).*?(\d+) of them against a wall: (\d+\.\d+) mm2", ln)
            isl = [{"height_mm": float(h), "width_mm": float(w), "area_mm2": float(a), "x": float(x), "y": float(y),
                    "at_wall": bool(wall)} for h, w, a, x, y, wall in ISL.findall(ln)]
            n, nw, area = int(head.group(1)), int(head.group(2)), float(head.group(3))
            assert len(isl) == n, (tag, op, len(isl), n)
            assert sum(i["at_wall"] for i in isl) == nw, (tag, op)
            assert "more." not in ln.split("tallest first in each group:")[1].split("stand taller")[0], "list truncated"
            return {"count": n, "at_wall": nw, "area_mm2": area, "islands": isl}
    sys.exit(f"{tag} {op}: no island line")


def unmatched(a, b):
    out = []
    for i in a["islands"]:
        if not any(abs(i["x"] - j["x"]) <= CELL + ROUND and abs(i["y"] - j["y"]) <= CELL + ROUND
                   and abs(i["height_mm"] - j["height_mm"]) <= CELL + ROUND for j in b["islands"]):
            out.append(i)
    return out


res = {}
for op in ("3D Rough 1", "3D Rough 1 (copy)"):
    d18, d20 = parse("d18", op), parse("d20", op)
    res[op] = (d18, d20)
json.dump({op: v[0] for op, v in res.items()}, open(f"{B}/islands_d18.json", "w"), indent=1)
json.dump({op: v[1] for op, v in res.items()}, open(f"{B}/islands_d20.json", "w"), indent=1)
for op, (d18, d20) in res.items():
    new, gone = unmatched(d20, d18), unmatched(d18, d20)
    print(f"== {op}: D18 {d18['count']} ({d18['at_wall']} wall, {d18['area_mm2']} mm2); D20 {d20['count']} ({d20['at_wall']} wall, {d20['area_mm2']} mm2)")
    for name, lst in (("in D20, not in D18", new), ("in D18, not in D20", gone)):
        lst = sorted(lst, key=lambda i: (-i["area_mm2"]))
        print(f"  {name}: {len(lst)} islands, {sum(i['area_mm2'] for i in lst):.1f} mm2, free {sum(not i['at_wall'] for i in lst)}; "
              f"over 1 mm2: {sum(i['area_mm2'] > 1 for i in lst)}; taller than 3.3 mm: {sum(i['height_mm'] > 3.3 for i in lst)}")
        for i in lst:
            if i["area_mm2"] > 1 or i["height_mm"] > 3.3 or not i["at_wall"]:
                print(f"    {i['height_mm']:.2f} mm high, {i['width_mm']:.2f} wide, {i['area_mm2']:.1f} mm2 at ({i['x']:.1f}, {i['y']:.1f}) {'wall' if i['at_wall'] else 'FREE'}")
    q = [i for i in d18["islands"] if abs(i["x"] - 14.1) <= 1 and abs(i["y"] + 109.3) <= 1]
    print(f"  D18 islands within 1 mm of (14.1, -109.3): {q}")
