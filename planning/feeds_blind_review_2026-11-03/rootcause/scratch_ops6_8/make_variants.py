"""Write scratch TOML variants with one dressup field changed in one toolpath block."""
import sys
src = open("wanaka21_ops6_8.toml").read()

def variant(out, op_name, key, value):
    start = src.index(f'name = "{op_name}"')
    nxt = src.find("[[setups.toolpaths]]", start)
    block = src[start:nxt]
    lines = block.split("\n")
    hit = 0
    for i, ln in enumerate(lines):
        if ln.startswith(f"{key} = "):
            lines[i] = f"{key} = {value}"
            hit += 1
    assert hit == 1, (op_name, key, hit)
    open(out, "w").write(src[:start] + "\n".join(lines) + src[nxt:])

variant("v_op8_none.toml", "ANTENNA_D5.00 pocket 2 mm", "entry_style", '"none"')
variant("v_op8_ramp10.toml", "ANTENNA_D5.00 pocket 2 mm", "ramp_angle", "10.0")
variant("v_op8_clear0.toml", "ANTENNA_D5.00 pocket 2 mm", "entry_clearance_mm", "0.0")
variant("v_op8_clear2.toml", "ANTENNA_D5.00 pocket 2 mm", "entry_clearance_mm", "2.0")
variant("v_op6_ramp.toml", "Lake pocket 2 mm, 8 lakes to Z 5.91", "entry_style", '"ramp"')
variant("v_op7_ramp.toml", "Lake pocket 2 mm, 1 lake to Z 3.99", "entry_style", '"ramp"')
variant("v_op8_ramp1.toml", "ANTENNA_D5.00 pocket 2 mm", "ramp_angle", "1.0")
variant("v_op8_nomerge.toml", "ANTENNA_D5.00 pocket 2 mm", "segment_merge", "false")
