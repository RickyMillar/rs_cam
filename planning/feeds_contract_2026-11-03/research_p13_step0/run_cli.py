"""Run the release CLI (`suggest --json`, a copy built at 36fd50c54) on Pocket cells.

Usage: python3 -I run_cli.py <path-to-rs_cam_cli> <scratch dir>
Writes cli_points.json next to this file. The project is the G-RESTRES fixture
(crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres) with one Pocket op;
no [job.machine] table unless a cell names one, so the machine is
MachineProfile::generic_wood_router (machine/mod.rs:895-919).
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
FIX = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres")

MATERIAL = {
    "GenericHardwood": '[job.stock.material.SolidWood]\nspecies = "GenericHardwood"',
    "GenericSoftwood": '[job.stock.material.SolidWood]\nspecies = "GenericSoftwood"',
    "mdf": '[job.stock.material.SheetGood]\nkind = "Mdf"',
}

# The Shapeoko Pro XXL table of the wanaka fixture
# (crates/rs_cam_core/tests/fixtures/wanaka_2026-08-16_afc6ecd4.toml:36-48), power from the
# shapeoko_vfd preset (machine/mod.rs:922-947): a repo machine with a 10 000 mm/min travel.
MACHINES = {
    "generic": "",
    "pro_xxl_vfd": """
[job.machine]
class = "hobby"
name = "Shapeoko Pro XXL"
max_feed_mm_min = 10000.0
max_shank_mm = 6.35

[job.machine.spindle.Variable]
min_rpm = 8000.0
max_rpm = 24000.0

[job.machine.power.VfdConstantTorque]
rated_power_kw = 1.5
rated_rpm = 24000.0

[job.machine.chip_load]
k0 = 0.024
p = 0.61
q = 1.26
""",
}

# (id, D, flute length, stickout, material, machine, ap, ae)
CELLS = [
    ("fm1_hw6", 6.0, 22.0, 30.0, "GenericHardwood", "generic", 1.5, 2.1),
    ("hw6_ae05", 6.0, 22.0, 30.0, "GenericHardwood", "generic", 1.5, 0.5),
    ("hw6_slot", 6.0, 22.0, 30.0, "GenericHardwood", "generic", 1.5, 6.0),
    ("hw6_deep", 6.0, 22.0, 30.0, "GenericHardwood", "generic", 6.0, 2.1),
    ("hw6_long", 6.0, 22.0, 60.0, "GenericHardwood", "generic", 1.5, 2.1),
    ("hw3", 3.175, 12.0, 20.0, "GenericHardwood", "generic", 0.79375, 1.11125),
    ("hw3_long", 3.175, 12.0, 40.0, "GenericHardwood", "generic", 0.79375, 1.11125),
    ("hw12", 12.0, 30.0, 38.0, "GenericHardwood", "generic", 3.0, 4.2),
    ("sw6", 6.0, 22.0, 30.0, "GenericSoftwood", "generic", 1.5, 2.1),
    ("mdf6", 6.0, 22.0, 30.0, "mdf", "generic", 1.5, 2.1),
    ("hw6_xxl", 6.0, 22.0, 30.0, "GenericHardwood", "pro_xxl_vfd", 1.5, 2.1),
    ("hw6_xxl_slot", 6.0, 22.0, 30.0, "GenericHardwood", "pro_xxl_vfd", 1.5, 6.0),
]


def project(d, flute, stickout, material, machine, ap, ae):
    src = open(os.path.join(FIX, "project.toml")).read()
    head = src.split("[[setups.toolpaths]]")[0]
    head = head.replace(
        "auto_from_model = false", "auto_from_model = false\n\n" + MATERIAL[material] + "\n" + MACHINES[machine]
    )
    head = head.replace('name = "6 mm End Mill"', f'name = "D{d} End Mill"')
    head = head.replace("diameter = 6.0", f"diameter = {d}")
    head = head.replace(
        'cutting_length = { estimated = 20.0, basis = "bare number before K3 (2026-11)" }',
        f"cutting_length = {{ stated = {flute} }}\nstickout = {{ stated = {stickout} }}\n"
        f"shank_diameter = {{ stated = {d} }}",
    )
    op = f"""[[setups.toolpaths]]
id = 0
name = "Pocket"
enabled = true
tool_id = 0
model_id = 0
stock_source = "fresh"

[setups.toolpaths.operation]
kind = "pocket"

[setups.toolpaths.operation.params]
stepover = {ae}
floor = {{ from = "top", depth = {max(3.0, ap)} }}
depth_per_pass = {ap}
climb = true
pattern = "contour"
angle = 0.0
finishing_passes = 0
"""
    return head + op


def main():
    cli, scratch = sys.argv[1], sys.argv[2]
    os.makedirs(scratch, exist_ok=True)
    with open(os.path.join(FIX, "square.svg")) as f:
        svg = f.read()
    with open(os.path.join(scratch, "square.svg"), "w") as f:
        f.write(svg)
    out = {}
    for cid, d, flute, stick, mat, mach, ap, ae in CELLS:
        p = os.path.join(scratch, cid + ".toml")
        with open(p, "w") as f:
            f.write(project(d, flute, stick, mat, mach, ap, ae))
        res = subprocess.run([cli, "suggest", "--json", p], capture_output=True, text=True, check=True)
        rec = json.loads(res.stdout.splitlines()[0])
        prof = rec["profile"]
        out[cid] = {
            "inputs": {"d": d, "flute": flute, "stickout": stick, "material": mat, "machine": mach, "ap": ap, "ae": ae},
            "calculator": prof["calculator"],
            "shipped_power": prof["shipped_power"],
            "envelope": prof["envelope"],
            "suggested": rec["suggested"],
        }
    with open(os.path.join(HERE, "cli_points.json"), "w") as f:
        json.dump(out, f, indent=1)
    print("cells", len(out))


if __name__ == "__main__":
    main()
