import re
d = "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/61c87c01-64ec-4b8d-a255-a09cc23316a1/scratchpad/"
src = open(d + "demo/rivmap100_live_0925.toml").read()
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
    open(d + f"plansimgap/{name}.toml", "w").write(head + b + tail)
print(list(arms))
