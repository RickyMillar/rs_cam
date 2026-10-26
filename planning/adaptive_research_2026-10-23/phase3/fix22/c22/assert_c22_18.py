"""Assert each fixture number that P3_C22_3d_turns.md §18.5 claims, from area_metric.json."""
import json
from pathlib import Path
d = json.loads((Path(__file__).resolve().parent / "area_metric.json").read_text())
w26, w31, c26, c31 = d["wanaka200 D26"], d["wanaka200 D31"], d["corne D26"], d["corne D31"]
assert (w26["width over 0.471"], w26["area over 0.471"], w26["area over 0.489"]) == (2222.8, 866.7, 782.5)
assert (w31["width over 0.471"], w31["area over 0.471"], w31["area over 0.489"]) == (2812.8, 976.5, 886.8)
assert (c26["width over 0.471"], c26["area over 0.471"], c26["area over 0.488"]) == (774.2, 453.7, 445.5)
assert (c31["width over 0.471"], c31["area over 0.471"], c31["area over 0.488"]) == (694.6, 370.9, 366.2)
assert w26["width over, ap <= 2"] == 791.5 and "area over, ap <= 2" not in w26  # all thin load drops out
assert w26["area over 0.471"] == w26["area over, ap > 3"]
assert round(2 / 5.46, 3) == 0.366
assert round(100 * (976.5 / 866.7 - 1), 1) == 12.7 and round(100 * (2812.8 / 2222.8 - 1), 1) == 26.5
assert round(100 * (370.9 / 453.7 - 1)) == -18
c = 0.3626  # the planner ceiling at s 2, R 3
assert (round(c * 6 * 3, 2), round(c * 6 * 5.46, 2), round(c * 6 * 6.0, 2)) == (6.53, 11.88, 13.05)
assert (round(0.471 + 0.1 / 3, 3), round(0.471 + 0.1 / 6, 3), round(0.471 + 0.1 / 5.46, 3)) == (0.504, 0.488, 0.489)
assert round(3 / 68 * 6, 2) == 0.26 and round(3 / 68 * 6 / 6, 3) == 0.044  # slope x 2R, over DPP 6
print("§18 asserts hold")
