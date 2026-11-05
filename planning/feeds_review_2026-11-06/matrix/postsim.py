"""Post-sim gate cells: the engine's own shipped recipe, simulated through the CLI project door.

Usage: python3 -I postsim.py <rs_cam_cli binary>
For each cell: write the project (run_cells.project), run `suggest --json`, write the shipped
feed, plunge and rpm into the op params, run `project <toml> --output-dir`, and keep the
load fields of the tp_*.json record. Writes postsim.json.
"""
import glob
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_cells as rc  # noqa: E402

IN = 25.4
CELLS = [
    # R37: the engine's free pick on 3.175 mm hardwood (cells.json A_flat_3.175_2f_hardwood proposal)
    dict(id="P_r37_pick", kind="end_mill", d=0.125 * IN, z=2, mat="hardwood", ap=6.985, ae=2.32, flute=12.0, stick=20.0),
    # the same tool at the Sienci printed pocket (5.6 x 1.45)
    dict(id="P_r37_sienci", kind="end_mill", d=0.125 * IN, z=2, mat="hardwood", ap=5.6, ae=1.45, flute=12.0, stick=20.0),
    # white oak 6.35 mm full slot at the Sienci hardwood depth (Freud row)
    dict(id="P_oak_slot", kind="end_mill", d=0.25 * IN, z=2, mat="whiteoak", ap=2.5, ae=0.25 * IN, flute=19.05, stick=27.05),
    # generic hardwood 6.35 mm full slot at the same depth (Spektra row)
    dict(id="P_hw_slot", kind="end_mill", d=0.25 * IN, z=2, mat="hardwood", ap=2.5, ae=0.25 * IN, flute=19.05, stick=27.05),
    # R42: a 0.05 D width at 1 D depth
    dict(id="P_thin_005", kind="end_mill", d=6.0, z=2, mat="hardwood", ap=6.0, ae=0.3, flute=20.0, stick=28.0),
]


def with_feeds(text, sug):
    add = (f"feed_rate = {sug['feed_rate']}\nplunge_rate = {sug['plunge_rate']}\n"
           f"spindle_rpm = {int(sug['spindle_rpm'])}\n")
    return text.replace("finishing_passes = 0\n", "finishing_passes = 0\n" + add)


def main():
    cli = sys.argv[1]
    base = os.path.join(HERE, "postsim")
    os.makedirs(base, exist_ok=True)
    with open(os.path.join(rc.FIX, "square.svg")) as f:
        svg = f.read()
    with open(os.path.join(base, "square.svg"), "w") as f:
        f.write(svg)
    machine = rc.rivmap_machine()
    out = {}
    for c in CELLS:
        c = dict(c, extra="", machine="shapeoko", group="P")
        p = os.path.join(base, c["id"] + ".toml")
        text = rc.project(c, machine)
        with open(p, "w") as f:
            f.write(text)
        s = subprocess.run(["timeout", "120", cli, "suggest", "--json", p], capture_output=True, text=True, check=True)
        rec = json.loads(s.stdout.split("\n")[0])
        sug = rec["suggested"]
        with open(p, "w") as f:
            f.write(with_feeds(text, sug))
        od = os.path.join(base, c["id"] + "_out")
        r = subprocess.run(["timeout", "600", cli, "project", p, "--output-dir", od], capture_output=True, text=True)
        tps = sorted(glob.glob(os.path.join(od, "tp_*.json")))
        tp = json.load(open(tps[0])) if tps else None
        summ = json.load(open(os.path.join(od, "summary.json"))) if os.path.exists(os.path.join(od, "summary.json")) else None
        out[c["id"]] = dict(cell=c, suggested=sug, calc=rec["profile"]["calculator"],
                            envelope_operator=(rec["profile"]["envelope"] or {}).get("operator"),
                            project_exit=r.returncode, project_stderr=r.stderr[-600:], tp=tp, summary=summ)
        print(c["id"], "exit", r.returncode, "tp", bool(tp))
    with open(os.path.join(HERE, "postsim.json"), "w") as f:
        json.dump(out, f)


if __name__ == "__main__":
    main()
