"""Assert each number that P3_C22_3d_turns.md §17 claims, from drape_left.json."""
import json
from pathlib import Path
d = json.loads((Path(__file__).resolve().parent / "drape_left.json").read_text())
a = d["D26"]
assert a["over by level"] == {"terrain level": 2222.8}  # all of it on the draped terrain level
o, u = a["over"], a["under"]
above_o = round(o["above by (0.1, 1]"] + o["above by > 1"], 1); assert above_o == 2144.4
assert round(above_o / 2222.8, 3) == 0.965 and o["none above"] == 78.4
above_u = round(u["above by (0.1, 1]"] + u["above by > 1"], 1); assert above_u == 8580.1
assert round(above_u / (above_u + u["none above"]), 2) == 0.53
assert (a["over rate with an earlier pass above"], a["over rate with none above"]) == (0.2, 0.01)
assert a["over by axial"]["> 3 mm"] == 1283.6 and a["over by axial"]["<= 2 mm"] == 791.5
b = d["D31"]
assert round(b["over"]["above by (0.1, 1]"] + b["over"]["above by > 1"], 1) == 2754.5
print("§17 asserts hold")
