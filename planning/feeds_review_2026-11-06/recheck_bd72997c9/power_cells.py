"""Power-ladder cells with a shank the machine holds (<= 6.35 mm). Usage: python3 -I power_cells.py <cli>
Stated full slots at 1, 1.5, 2, 2.5, 3 D. Writes power_cells.json."""
import json, os, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import run_cells as rc
IN = 25.4
cli = sys.argv[1]; d = os.path.join(HERE, "power"); os.makedirs(d, exist_ok=True)
open(os.path.join(d, "square.svg"), "w").write(open(os.path.join(rc.FIX, "square.svg")).read())
m = rc.rivmap_machine(); out = {}
for tool_d, z, mat in ((6.0, 2, "hardwood"), (6.0, 2, "softwood"), (6.0, 2, "mdf"), (0.25 * IN, 3, "hardwood"), (0.25 * IN, 2, "whiteoak")):
    for k in (1.0, 1.5, 2.0, 2.5, 3.0):
        cid = f"PW_{tool_d:.2f}_{z}f_{mat}_{k}D"
        c = dict(id=cid, kind="end_mill", d=tool_d, z=z, mat=mat, ap=k * tool_d, ae=tool_d, flute=3.2 * tool_d, stick=3.2 * tool_d + 8, extra="", machine="shapeoko", group="PW")
        p = os.path.join(d, cid + ".toml"); open(p, "w").write(rc.project(c, m))
        r = subprocess.run(["timeout", "120", cli, "suggest", "--json", p], capture_output=True, text=True, check=True)
        rec = json.loads(r.stdout.split("\n")[0]); calc = rec["profile"]["calculator"]; s = rec["suggested"]
        warns = [str(w)[:60] for w in calc["warnings"]]
        out[cid] = dict(cell=c, feed=s["feed_rate"], rpm=s["spindle_rpm"], chip=calc["chip_load_mm"], power=calc["power_kw"], avail=calc["available_power_kw"],
                        fz=s["feed_rate"] / (s["spindle_rpm"] * z), warnings=[str(w) for w in calc["warnings"]])
        print(f"{cid:34} feed {s['feed_rate']:8} rpm {s['spindle_rpm']:6} fz {out[cid]['fz']:.4f} P {calc['power_kw']:.3f}/{calc['available_power_kw']:.3f} | {[w.split(' ')[0] for w in warns]}")
json.dump(out, open(os.path.join(HERE, "power_cells.json"), "w"), indent=1)
