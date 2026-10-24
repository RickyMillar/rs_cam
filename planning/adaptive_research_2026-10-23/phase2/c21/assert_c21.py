# C21: assert every number that P2_C21_entry_cost.md quotes.
# Run: python3 -I assert_c21.py
#
# 1. A fresh run of the model (run_c21.compute, not results_c21.json) gives
#    every quoted value again (note_c21.facts), and each quoted text stands
#    in the note.
# 2. The port agrees with closed forms of the Rust formulas it ports.
# 3. The port over the Phase 0 G-code agrees with the Phase 0 runtimes.
# 4. The C19 break-even of the note equals C19's own quoted value when the
#    plan matches.
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import model as M  # noqa: E402
import note_c21 as N  # noqa: E402
import run_c21 as R  # noqa: E402

FAIL = []


def check(ok, msg):
    if not ok:
        FAIL.append(msg)


def close(a, b, tol, msg):
    check(abs(a - b) <= tol, f"{msg}: {a} vs {b} (tol {tol})")


# ── 1. every quote, from a fresh run ──
fresh = R.compute()
c19 = N.load_c19()
F = N.facts(fresh, c19)
quotes = json.load(open(os.path.join(HERE, "quotes_c21.json")))
note = open(os.path.join(N.RES, "P2_C21_entry_cost.md")).read()
for key, qv in quotes.items():
    check(key in F, f"{key}: no fact")
    if key not in F:
        continue
    close(F[key], qv["value"], 1e-9 * max(1.0, abs(qv["value"])), f"{key} value")
    check(format(F[key], qv["fmt"]) == qv["text"], f"{key} text {format(F[key], qv['fmt'])} != {qv['text']}")
    check(qv["text"] in note, f"{key} text {qv['text']} not in the note")

# ── 2. closed forms ──
riv, cor, wan = R.FIX["rivmap100"], R.FIX["corne"], R.FIX["wanaka200"]
kin = R.kin_of(riv["kin"])
# A Z-only rapid that starts and ends at rest and never reaches 10000 mm/min
# is a triangle at the Z axis accel: t = 2 sqrt(L / a) (K:1601-1608).
for L in (7.0, 6.5, 16.5):
    t, _ = M.cycle_time([M.rapid((0, 0, L), "x")], kin, 10000, 10000, (0, 0, 0))
    close(t, 2 * math.sqrt(L / 270.0), 1e-12, f"Z rapid {L} mm")
# The first rivmap100 entry into an open column at L = 0: lift 7 mm, descent
# 6.5 mm (to the cut + 0.5), a 0.5 mm plunge at 500 mm/min that reaches its
# feed (trapezoid at 270 mm/s²). Cut B leaves at a 90° junction, not at rest,
# so the model is a little below this sum.
v = 500 / 60.0
plunge = 2 * v / 270.0 + (0.5 - v * v / 270.0) / v
hand = 2 * math.sqrt(7 / 270.0) + 2 * math.sqrt(6.5 / 270.0) + plunge
got = N.row(fresh["rivmap100"]["curves"], "2|open", 0.0)["entry_s"]
check(0 < hand - got < 0.03, f"rivmap100 open entry {got} vs hand {hand}")
# The rivmap100 level helix: 2.5 mm at pitch 1 is ceil(2.5 * 36) = 90 steps;
# the first leaves the centre (r), each other is a 10° chord, then back to the
# centre (r). Its time is near path / plunge feed (the 10° junctions run
# at the feed: sqrt(a * R) with R = 0.02 cos5° / (1 - cos5°) is far above it).
r = 1.8
steps = math.ceil(2.5 * 36 - 1e-9)
chord = 2 * r * math.sin(math.radians(5))
path = 2 * r + (steps - 1) * chord
style = fresh["rivmap100"]["cross"]["level_style_s"]
close(style, path / v, 0.02 * path / v, "rivmap100 level helix")
jR = 0.02 * math.cos(math.radians(5)) / (1 - math.cos(math.radians(5)))
check(math.sqrt(500 * jR) > v, "10° helix junction below the plunge feed")
# 90° corner at δ 0.02 and 500 mm/s²: 4.9 mm/s (note section 2).
v90 = M.junction_velocity((1, 0, 0), (0, 1, 0), 40, 40, 500, 0.02, False)
check(format(v90, ".1f") == "4.9", f"90° junction {v90}")
close(v90, math.sqrt(500 * 0.02 * math.sqrt(0.5) / (1 - math.sqrt(0.5))), 1e-12, "90° junction closed form")
# Corne has no kinematics: each move is length / feed. Open column, first
# level (6 mm), L = 5: lift 11, travel 5, descent 10.5 at 4000; 0.5 at 750.
hand = (11 + 5 + 10.5) / 4000 * 60 + 0.5 / 750 * 60
close(N.row(fresh["corne"]["curves"], "6|open", 5.0)["entry_s"], hand, 1e-9, "Corne open entry")
# Corne keep-down 5 mm at 1518.75 mm/min.
close(N.row(fresh["corne"]["curves"], "6|open", 5.0)["keep_s"], 5 / 1518.75 * 60, 1e-9, "Corne keep-down")
# The wanaka200 level column: one peck of 5.46 at 541, a 0.5 rapid up, then
# 1.0 mm fed (P:164-195). Check the emitted moves.
mv = M.peck_plunge((0, 0, -5.46), 0.5, 5.46, 0.0, 541)
zs = [round(m[1][2], 6) for m in mv]
check(zs == [-4.96, -4.46, -5.46] and [m[0] for m in mv] == ["line", "rapid", "line"], f"wanaka200 peck {zs}")

# ── 3. the port against Phase 0 runtimes ──
for n in N.NAMES:
    g = fresh[n]["gcode"]
    close(g["total_s"], g["phase0_runtime_s"], 0.001 * g["phase0_runtime_s"], f"{n} port vs Phase 0")

# ── 4. C19 break-even ──
q19 = json.load(open(os.path.join(N.C19, "quotes_c19.json")))
for nm in ("star", "pocket"):
    for src in ("e", "march"):
        k = f"{nm}|ratio|be_{src}"
        if k in q19:
            close(F[f"c19|{nm}|{src}|be"], q19[k]["value"], 1e-9, f"C19 {k}")

if FAIL:
    print(f"FAIL ({len(FAIL)})")
    for m in FAIL:
        print(" -", m)
    sys.exit(1)
print(f"OK: {len(quotes)} quotes, closed forms, 3 Phase 0 runtimes, C19 break-even")
