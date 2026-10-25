"""D20 numbers for one run dir against its D18 reference: entries and
keep-down links of the first rough op (the page's rule, fixture3d_v2.py),
cycle time, collisions, the producer split and the thick measure of the
over-0.471 load (producer_split.py, thick_measure.py), and the CLI hard
bars (standing islands, gouge and deeper-than-commanded lines).
Usage: d20_numbers.py <run dir> <reference dir> <op comment> <tp glob>"""
import importlib.util as ilu
import json
import os
import sys

H = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fix2")
sys.argv_saved = sys.argv


def mod(name):
    s = ilu.spec_from_file_location(name, os.path.join(H, name + ".py"))
    m = ilu.module_from_spec(s)
    s.loader.exec_module(m)
    return m


fx = mod("fixture3d_v2")
ps = mod("producer_split")
tm = mod("thick_measure")
run, ref, op, glob_ = sys.argv[1:5]


def bars(d):
    err = open(os.path.join(d, "stderr.txt")).read().splitlines()
    return [ln.strip() for ln in err if any(k in ln for k in ("island(s) standing", "gouge", "Gouge", "deeper than", "fed link cuts"))]


for d in (ref, run):
    s = json.load(open(os.path.join(d, "summary.json")))
    o = fx.load_op(d, glob_, op, d)
    n = fx.op_numbers(o)
    print(f"== {d}")
    print(f"  time {s['total_runtime_s']:.1f} s; cutting {s['total_cutting_distance_mm']:.0f} mm; collisions {s['collision_count']}, rapid {s['rapid_collision_count']}, checks failed {s['collision_checks_failed']}; export refused {s['gcode_export']['refused']}; safety errors {s['gcode_export']['machine_safety_errors']}")
    print(f"  {op}: moves {n['moves']}, plunge entries {o['entries_plunge']}, keep-down links {n['keepdown_links']} ({n['keepdown_mm']} mm)")
    print(f"  producer split over 0.471: {ps.split(d, d, glob_)}")
    print(f"  thick (>= 0.5 mm) over 0.471: {tm.thick_over(d, d, glob_)}")
    for b in bars(d):
        print(f"  BAR {b}")
