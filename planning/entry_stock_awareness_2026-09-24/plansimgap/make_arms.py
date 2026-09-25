import os
import re

# The rivmap100 project is in planning/fixtures/rivmap100/ (moved from a
# session scratchpad on 2026-09-25). The arms are written to
# planning/fixtures/rivmap100/plansimgap_arms/, one level below the
# fixture, so their model paths get a "../" prefix.
HERE = os.path.dirname(os.path.abspath(__file__))
FIX = os.path.normpath(os.path.join(HERE, "..", "..", "fixtures", "rivmap100"))
OUT = os.path.join(FIX, "plansimgap_arms")
os.makedirs(OUT, exist_ok=True)
src = open(os.path.join(FIX, "rivmap100_live_0925.toml")).read()
src = src.replace('path = "rivmap_export/', 'path = "../rivmap_export/')
# toolpath 1 block: from '[[setups.toolpaths]]\nid = 1' to the next '[[setups.toolpaths]]'
start = src.index("[[setups.toolpaths]]\nid = 1\n")
end = src.index("[[setups.toolpaths]]", start + 10)
head, block, tail = src[:start], src[start:end], src[end:]


def sub(b, key, val):
    pat = re.compile(r"^" + key + r" = .*$", re.M)
    assert len(pat.findall(b)) == 1, key
    return pat.sub(f"{key} = {val}", b)


def sub_boundary_off(b):
    i = b.index("[setups.toolpaths.boundary]")
    j = b.index("enabled = true", i)
    return b[:j] + "enabled = false" + b[j + len("enabled = true"):]


base = sub(block, "region_ordering", '"global"')
arms = {
    "base": base,
    "merge_off": sub(base, "segment_merge", "false"),
    "arc_off": sub(base, "arc_fitting", "false"),
    "both_off": sub(sub(base, "segment_merge", "false"), "arc_fitting", "false"),
    "link_off": sub(base, "link_moves", "false"),
    "feedopt_off": sub(base, "feed_optimization", "false"),
    "boundary_off": sub_boundary_off(base),
}
all_off = arms["both_off"]
for k in ["link_moves", "feed_optimization", "optimize_rapid_order"]:
    all_off = sub(all_off, k, "false")
arms["all_off"] = all_off
for name, b in arms.items():
    open(os.path.join(OUT, f"{name}.toml"), "w").write(head + b + tail)
print(list(arms))
