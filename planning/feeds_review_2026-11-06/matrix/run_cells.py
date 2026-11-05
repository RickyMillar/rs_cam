"""Matrix cells of the feeds review (pre-registered in ../PREREGISTRATION.md, section "Cells").

Usage: python3 -I run_cells.py <rs_cam_cli binary>
Writes one project per cell under cells/ and cells.json (the engine answer per cell).
Adapted from planning/feeds_contract_2026-11-03/research_p13_step0/run_cli.py: the project is
written whole (no string replace) from the shape of
crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres/project.toml, one Pocket op on the
40 mm square SVG. The machine is the rivmap100 [job.machine] table
(planning/fixtures/rivmap100/rivmap100.toml:28-50) unless a cell names "rigid".

A stated stepover and depth bound the envelope proposal from above ("never above a stated
value", MODELS.md M9). A cell that wants the free proposal states ae = D and ap = 3 D.
"""
import csv
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
FIX = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres")
IN = 25.4

MATERIAL = {
    "softwood": '[job.stock.material.SolidWood]\nspecies = "GenericSoftwood"',
    "hardwood": '[job.stock.material.SolidWood]\nspecies = "GenericHardwood"',
    "whiteoak": '[job.stock.material.SolidWood]\nspecies = "WhiteOak"',
    "mdf": '[job.stock.material.SheetGood]\nkind = "Mdf"',
    "plywood": '[job.stock.material.Plywood]\ngrade = "BalticBirch"',
}


def rivmap_machine():
    """The rivmap100 machine table, copied from the file (lines 28-50), kinematics left out."""
    lines = open(os.path.join(REPO, "planning/fixtures/rivmap100/rivmap100.toml")).read().split("\n")
    block = lines[27:50]
    assert block[0] == "[job.machine]", block[0]
    assert block[-1].startswith("adaptive_woc_factor"), block[-1]
    return "\n".join(block)


def rigid_machine():
    """Unsourced rigid table: the hobby budget scaled by a printed frame-stiffness ratio.

    k ratio = AltMill 4x4 worst axis / LongMill MK2 48x30 worst axis, secant at 50 N, from
    planning/feeds_contract_2026-11-03/research_sienci_stiffness/stiffness_table.csv. The hobby
    budget is the FM1 `cap_factor` column. The proportionality budget ~ k_frame is an
    assumption with no source, so every result on this table is Unsourced.
    """
    rows = list(csv.DictReader(open(os.path.join(
        REPO, "planning/feeds_contract_2026-11-03/research_sienci_stiffness/stiffness_table.csv"))))

    def worst(machine, force):
        ks = [float(r["stiffness_derived_N_per_mm"]) for r in rows
              if r["machine"] == machine and float(r["force"]) == force and r["stiffness_derived_N_per_mm"]
              and "blog" in r["source_file"]]
        return min(ks)

    k_alt = worst("AltMill 4x4", 50.0)
    k_mk2 = worst("LongMill MK2 48x30", 50.0)
    fm1 = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv")
    budgets = {float(r["cap_factor"]) for r in csv.DictReader(open(fm1)) if r["cap_family"] == "Pocket" and r["cap_factor"]}
    hobby = max(budgets)
    budget = hobby * k_alt / k_mk2
    text = rivmap_machine().replace('class = "hobby"', 'class = "industrial"')
    text = text.replace("[job.machine.rigidity]\n", "[job.machine.rigidity]\n" + f"force_budget_n = {budget}\n")
    return text, dict(k_alt=k_alt, k_mk2=k_mk2, hobby_budget=hobby, rigid_budget=budget)


def tool_toml(kind, d, z, flute, stick, extra):
    t = f"""[[tools]]
id = 0
name = "{kind} D{d} Z{z}"
type = "{kind}"
diameter = {d}
cutting_length = {{ stated = {flute} }}
stickout = {{ stated = {stick} }}
shank_diameter = {{ stated = {max(d, 3.175) if kind != 'tapered_ball_nose' else 6.0} }}
flute_count = {{ stated = {z} }}
tool_number = 1
centre_cutting = {{ stated = "yes" }}
"""
    return t + extra


def project(cell, machine_text):
    t = tool_toml(cell["kind"], cell["d"], cell["z"], cell["flute"], cell["stick"], cell.get("extra", ""))
    ap, ae = cell["ap"], cell["ae"]
    return f"""format_version = 3

[job]
name = "feeds review cell {cell['id']}"

[job.simulation]
resolution_mm = 0.5

[job.stock]
x = 40.0
y = 40.0
z = {max(6.0, ap * 2):.4f}
origin_x = 0.0
origin_y = 0.0
origin_z = {-max(6.0, ap * 2):.4f}
auto_from_model = false

{MATERIAL[cell['mat']]}

{machine_text}

{t}
[[models]]
id = 0
path = "square.svg"
name = "Square"
kind = "svg"
units = {{ kind = "millimeters" }}

[[setups]]
name = "Setup 1"

[[setups.toolpaths]]
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
floor = {{ from = "top", depth = {max(ap, 3.0)} }}
depth_per_pass = {ap}
climb = true
pattern = "contour"
angle = 0.0
finishing_passes = 0
"""


def cells():
    C = []

    def add(cid, kind, d, z, mat, ap, ae, flute=None, stick=None, extra="", machine="shapeoko", group=""):
        flute = flute if flute is not None else max(3.0 * d, 3.0)
        stick = stick if stick is not None else flute + 8.0
        C.append(dict(id=cid, kind=kind, d=d, z=z, mat=mat, ap=ap, ae=ae, flute=flute, stick=stick,
                      extra=extra, machine=machine, group=group))

    eighth, quarter, half = 0.125 * IN, 0.25 * IN, 0.5 * IN
    # A. printed-row cells, free proposal (ae = D, ap = 3 D)
    for mat in ("softwood", "hardwood", "whiteoak", "mdf", "plywood"):
        for d, z in ((eighth, 2), (6.0, 2), (quarter, 2), (half, 2), (quarter, 3), (0.0625 * IN, 2)):
            add(f"A_flat_{d:.3f}_{z}f_{mat}", "end_mill", d, z, mat, 3 * d, d, group="A")
        for d in (eighth, quarter):
            add(f"A_ball_{d:.3f}_{mat}", "ball_nose", d, 2, mat, 3 * d, d, group="A")
    # B. hobby slot cells: stated full slot at the Sienci printed depth (6.35 mm, 2F assumed)
    for mat, ap in (("hardwood", 2.5), ("whiteoak", 2.5), ("softwood", 5.4), ("mdf", 5.4)):
        add(f"B_slot_sienci_{mat}", "end_mill", quarter, 2, mat, ap, quarter, group="B")
    # Carbide 3D: 3F .25 in, full slot at the printed DOC (Mahogany .1, Pine .4, MDF .3 in)
    for mat, doc_in in (("hardwood", 0.1), ("softwood", 0.4), ("mdf", 0.3)):
        add(f"B_slot_c3d_{mat}", "end_mill", quarter, 3, mat, doc_in * IN, quarter, group="B")
    # B2. Sienci pocket point: 6.35 mm at the printed pocket depth and stepover 2.85 mm
    for mat, ap in (("hardwood", 5.6), ("softwood", 12.1)):
        add(f"B_pocket_sienci_{mat}", "end_mill", quarter, 2, mat, ap, 2.85, group="B")
    # 1/8 in hardwood pocket at Sienci 5.6 mm x 1.45 mm (R37 reference)
    add("B_pocket_sienci_eighth_hardwood", "end_mill", eighth, 2, "hardwood", 5.6, 1.45, group="B")
    # C. probes, 6 mm 2F hardwood
    for frac in (1.0, 0.5, 0.2, 0.05):
        add(f"C_width_{frac}", "end_mill", 6.0, 2, "hardwood", 6.0, frac * 6.0, group="C")
    for dm in (0.5, 1.0, 2.0, 3.0):
        add(f"C_depth_{dm}", "end_mill", 6.0, 2, "hardwood", dm * 6.0, 3.0, flute=20.0, group="C")
    for z in (1, 3):
        add(f"C_flutes_{z}", "end_mill", 6.0, z, "hardwood", 18.0, 6.0, group="C")
    for d in (1.0, 12.0, 25.0):
        add(f"C_diam_{d}", "end_mill", d, 2, "hardwood", min(3 * d, 50.0), d, group="C")
    # D. bull nose 6 mm r 1 mm (R30): depth below and above the corner radius
    for ap in (0.5, 3.0):
        add(f"D_bull_ap{ap}", "bull_nose", 6.0, 2, "hardwood", ap, 3.0, extra="corner_radius = 1.0\n", group="D")
    # E. rigid table, Unsourced
    for mat in ("hardwood", "softwood"):
        add(f"E_rigid_{mat}", "end_mill", quarter, 2, mat, 3 * quarter, quarter, machine="rigid", group="E")
    return C


def main():
    cli = sys.argv[1]
    cdir = os.path.join(HERE, "cells")
    os.makedirs(cdir, exist_ok=True)
    with open(os.path.join(FIX, "square.svg")) as f:
        svg = f.read()
    with open(os.path.join(cdir, "square.svg"), "w") as f:
        f.write(svg)
    shapeoko = rivmap_machine()
    rigid, rigid_info = rigid_machine()
    out = {"_rigid": rigid_info}
    for c in cells():
        p = os.path.join(cdir, c["id"] + ".toml")
        with open(p, "w") as f:
            f.write(project(c, rigid if c["machine"] == "rigid" else shapeoko))
        res = subprocess.run(["timeout", "120", cli, "suggest", "--json", p], capture_output=True, text=True)
        if res.returncode != 0 or not res.stdout.strip():
            out[c["id"]] = {"cell": c, "error": res.stderr[-800:]}
            print("ERR", c["id"], res.stderr[-200:].replace("\n", " "))
            continue
        rec = json.loads(res.stdout.split("\n")[0])
        out[c["id"]] = {"cell": c, "rec": rec}
    with open(os.path.join(HERE, "cells.json"), "w") as f:
        json.dump(out, f)
    print("cells", len(out) - 1)


if __name__ == "__main__":
    main()
