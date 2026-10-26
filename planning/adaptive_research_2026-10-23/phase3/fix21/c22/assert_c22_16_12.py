"""Assert each number that P3_C22_3d_turns.md §16.12 claims, from nearA_kind.json."""
import json
import pathlib

d = json.loads((pathlib.Path(__file__).parent / "nearA_kind.json").read_text())
w26, w30 = d["wanaka200 D26"], d["wanaka200 D30"]
c26, c30 = d["corne D26"], d["corne D30"]


def tot(r):
    return round(sum(r.values()), 1)


assert tot(w26) == 1534.7 and tot(w30) == 2040.7, (tot(w26), tot(w30))
assert tot(c26) == 637.5 and tot(c30) == 562.9, (tot(c26), tot(c30))
# The steps add little; the after-step and level shares carry the rise.
assert round(w30["on a step"] - w26["on a step"]) == 39
assert round(w30["after a step"] - w26["after a step"]) == 179
assert round(w30["level"] - w26["level"]) == 288
# The level share was already most of the D26 near-A load (G-FACESLAB).
assert w26["level"] / tot(w26) > 0.85
print("§16.12 asserts hold")
