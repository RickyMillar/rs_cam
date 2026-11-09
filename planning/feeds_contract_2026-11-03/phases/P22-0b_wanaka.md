# P22-0b on wanaka 21: before and after (2026-11-09)

Read-only measurement. HEAD `21bddd593` (P22-0b). The release GUI started 2026-11-09 15:32:50
from a binary built 15:32:09, later than HEAD (15:26:27). Project
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`. No file under `crates/` changed. No
cargo job ran. No `set_*`, `apply_feeds`, `save_project` or `edit_*` call ran.

"Before" is `P20g_wanaka.md` (HEAD `75133f73b`) and its G-code `wanaka_p20g_*.nc`. "After" is
this reading and its G-code `p22b/wanaka_p22_0b_*.nc`. Both sets are in the scratchpad of
session `7a1b80aa-de3a-4f94-9eb3-ffbff208a378`, with the parse scripts in `p22b_scripts/`.

## 1. Method

1. `load_project`, then `generate_all` with `timeout_s: 60`. The call moved to the background.
   `generation_status` went idle after about 6 min with no error; the final reply of the call
   was not returned, so the generated / current counts are not recorded. `list_toolpaths`: all
   18 toolpaths `Done`, none stale.
2. `run_simulation` (0.2 mm cell, the stored value). Verdict "52.0 % air cutting", 0 collisions,
   0 rapid collisions, triage safety list empty.
3. `get_tool_load_report`, `get_project_diagnostics`, `get_toolpath_diagnostics` for idx 0 to 17.
4. `export_gcode` with `split_setups: true` (three files). The export header lists 21 warnings.
5. The G-code parser reads modal X, Y, Z and F. Toolpath move `k` is G-code motion line `k + 1`
   of the section in the Back file (one preamble `G0`), and `k + 2` in the Front file (two
   preamble `G0` around the tool change). The finding positions check this (section 4).
6. An entry ramp is a run of descending G1 moves at the ramp feed (6448). A leg is a run in one
   XY direction; a reversal starts a new leg. The last leg of a run can be partial (the ramp
   ends at the level, or a modulated F splits the run), so the 8 mm rule applies to full legs.

## 2. Envelope (P20i): `project.sim_outside_envelope`

| idx | Toolpath | Before (P20g) | After (P22-0b) |
|---|---|---|---|
| 1 | Back bulk rough 18 mm (tool 6) | 5076 of 30557; spindle power +102 % (move 1697, depth 10.00, width 17.72) | **no finding** |
| 2 | Back rest 6 mm | 221 of 76248; vendor axial bound +145 % (move 14212, depth 14.71) | 221 of 76239; vendor axial bound +145 % (move 14235, depth 14.71) |
| 6 | Lake pocket 2 mm, 11 lakes | 4381 of 30084; vendor depth ceiling +34 % (move 1996, depth 2.67) | the same |
| 7 | Lake pocket 2 mm, 1 lake | 195 of 435; vendor depth ceiling +181 % (move 4, depth 5.61) | the same |
| 8 | ANTENNA pocket 2 mm | 770 of 900; corner coverage +5 % (move 61, width 1.80) | the same |
| 10 | Day strip slit V60 | 1144 of 3492; flute guard +5 % (move 4, depth 4.60) | the same |
| 12 | Face front to T (tool 6) | 211 of 89347; corner coverage +15 % (move 143, width 17.60) | 211 of 90004; the same worst |
| 15 | Front 3D rough | 60 of 60669; vendor axial bound +69 % (move 27163, depth 10.15) | 60 of 61198; vendor axial bound +69 % (move 27034, depth 10.15) |

- idx 1 no longer reads power over the limit at move 1697. The finding is gone, so no other
  limit holds an idx 1 sample outside. This is the P20i prediction.
- The other 7 rows keep their counts and their binding limits (all geometric). Only the
  denominators and move numbers of idx 2, 12 and 15 move. idx 12 is a tool-6 op (ring stamp);
  idx 2 and 15 rest on tool-6 stock and were generated again on the new stock.

## 3. Core (P22-0b)

### 3.1 Findings

| Finding | idx | Before | After |
|---|---|---|---|
| `safety.non_centre_cutting_descent` | 1 | none | 1 move: move 17183, 1.05 mm of wood over the tip (0.05 mm beyond the 1.00 mm protrusion), descent 7.1 deg |
| `safety.non_centre_cutting_descent` | 14 | none | 1 move: move 17734 at X116.434 Y291.638 Z17.983, 1.03 mm over the tip (0.03 mm beyond), descent 7.1 deg |
| `safety.non_centre_cutting_plunge` (static, critical) | 14 | not in the P20g list | entry at X333.450 Y54.277 Z15.878 must plunge 1.79 mm (move 21244: G1 Z17.668 -> Z15.878 at F2162) |
| `safety.helix_inside_core` | any | (id did not exist) | none |
| core findings on idx 0, 11, 12 (tool 6) | | none | none |
| core findings on a non-tool-6 op | | none | none |

### 3.2 Move intent of the two descent findings

Both are cut descents at the old straight-line limit (7.125 deg), not entry ramps. P22-0b
open question 3 names this case: `clamp_descents_to_limit` keeps the straight-line limit, and a
path that turns while it descends at that limit can exceed it.

- idx 1, move 17183 (G-code lines 17537-17541): a rapid to Z 30.5, a feed to Z 26.0, an air
  plunge of 0.5 mm to Z 25.5 (the stock top) at F2162, then a straight descent at 7.125 deg
  (F6449). At X104.278 Y53.5 Z24.814 the path turns 50 deg to +Y and descends 22.5 mm more to
  Z 22.002, still at 7.12 deg. The finding is on the leg after the turn (Z 24.448 is on it).
  The core dwells at the turn.
- idx 14, move 17734 (G-code line 18463): the last move of a curved descent at F2075. Each
  move descends at 7.12 deg while the direction turns; the next move climbs at 33.6 deg.
- idx 1 has 25 single descending moves at 7.1 deg (groups at moves 2103-2340 and 17181-17215).
  Only one gives a finding.

### 3.3 The idx 14 plunge

`safety.non_centre_cutting_plunge` is a static (planner) row, critical. Move 21244 is a
vertical G1 of 1.79 mm at the plunge feed after a feed descent to Z 17.668. The simulation core
check gives no finding at move 21244. Thus the planner stock says wood is under the plunge and
the simulation stock says the plunge is clean (an earlier level or pass cleared it). This read
cannot say which stock is right. Two more vertical fed descents in idx 14 are deeper than the
protrusion and give no finding of either kind: move 19497 (4.665 mm, Z 22.333 -> 17.668, F2162)
and move 18554 (1.212 mm, F1703).

## 4. Entries of the tool-6 roughs

The requested ramp angle is 3.0 deg on both ops (`params.ramp_angle_deg`, `entry_style: ramp`,
`ramp_feed_rate` 6448). The limit is min(3.0, 3.576) = 3.0 deg; plus 1 % is 3.030 deg.

| Reading | idx 1 before | idx 1 after | idx 14 before | idx 14 after |
|---|---|---|---|---|
| entry runs at F6448 with a drop >= 0.5 mm | 48 | 58 | 64 | 60 |
| leg descent angle, max | 7.128 deg | **3.002 deg** | 7.129 deg | **3.016 deg** |
| legs over 3.030 deg | 226 of 226 | **0** of 467 | 132 of 132 | **0** of 256 |
| full-leg length, min | 12.738 mm | 12.738 mm | 9.694 mm | 9.210 mm |
| full legs under 8 mm | 0 | **0** | 0 | **0** |
| typical leg | 18.127 mm at 7.125 deg, 5 legs per 9.5 mm drop | 18.127 mm at 3.000 deg, 10 legs per 9.5 mm drop | 7.8 mm, one leg at 7.13 deg | 14.839 mm at 3.0 deg, 6 legs per 4.67 mm drop |
| all descending G1 at 7.0-7.2 deg | 6892 | 25 (cut descents, 3.2) | 5665 | a few hundred cut descents (terrain), see 4.1 |

Count of all legs (any feed, idx 1): 538 legs, max 3.002 deg; full legs min 15.901 mm.

Both ops pass: every full leg is >= 8 mm, and no leg is steeper than 3.0 deg plus 1 %. The terrain
re-plan now holds the requested 3 deg; before, it ran the 3 deg request at 7.125 deg.

### 4.1 Note on idx 14 descents that are not entries

idx 14 follows the terrain. It has about 1100 single-leg cut descents at angles from 1.5 to
7.14 deg, at the cutting feeds. These are not entry ramps; the 3.0 deg ramp rule does not apply
to them. The cut and link descent clamp keeps the 7.125 deg straight-line limit (P22-0b open
question 3). The idx 14 descent finding (3.2) is one of these.

## 5. Power

| Reading | Before (P20g) | After (P22-0b) |
|---|---|---|
| idx 1 verdict, peak / available | within, 1.125 / 1.125 kW, samples 101087-101088 | within, 1.125 / 1.125 kW, samples 101383-101384 |
| idx 1 contributing samples | 30557 | 31089 of 137375 |
| idx 1 `power_max` binding | 0.00105 (15 of 14257 moves) | 0.00079 (18 of 22892 moves) |
| idx 1 moves / fed time | 14284 / 264.1 s | 22920 / 303.7 s (+39.6 s, +15 %) |
| idx 14 verdict, peak | within, 0.769 kW | within, 0.769 kW (0.76929) |
| idx 14 fed time | 416.3 s | 430.8 s (+14.5 s, +3.5 %) |
| project power rows that exceed | 0 | 0 |

The power verdicts do not change. The idx 1 peak still sits exactly at the available power,
because the P20g modulator lowers the feed to the limit. The ring stamp adds 532 contributing
samples on idx 1 and does not move the idx 14 peak. The fed time rises because the 3 deg ramps
are 2.4 times as long as the 7.125 deg ramps (ramp path per unit drop: 1 / tan).

## 6. Every critical and blocking finding (after)

No finding is blocking. No project-level finding is critical.

| idx | id | One line |
|---|---|---|
| 2 | `project.entry_load` | 69 of 44873 entry samples remove more than 3.45 mm; peak 15.92 mm at (94.1, 89.1, 8.84) |
| 14 | `safety.non_centre_cutting_plunge` | an entry at X333.450 Y54.277 Z15.878 must plunge 1.79 mm into stock (blades 1.00 mm below the centre) |
| 15 | `project.entry_load` | 3074 of 32049 entry samples remove more than 0.50 mm; peak 8.95 mm at (348.3, 37.0, 15.84) |

The two `safety.non_centre_cutting_descent` rows are warnings (`warn(Danger::Tool)`) in the
export header; they are not in the diagnostics list (7.1).

## 7. Surprises

1. **The core descent finding is only in the export header.** `get_toolpath_diagnostics` for
   idx 1 and 14 and `get_project_diagnostics` do not list `safety.non_centre_cutting_descent`;
   the `run_simulation` triage safety list is empty. Only `export_gcode` names it. A user who
   reads the GUI diagnostics does not see it.
2. **The idx 1 finding position is in the model frame.** The export names "X85.778 Y47.935
   Z24.448" for idx 1. No move in the Back file passes within 0.3 mm of that point. The stock
   origin is (-18.5, -8.5); with that shift the point is X104.278 Y56.435, which is on move
   17183 of the G-code. The idx 14 position (Front setup, face Bottom) matches the G-code with no
   shift. So the position frame differs between the two setups.
3. **The descent message prints the old limit.** It says "the limits are a 1.00 mm plunge and
   7.13 deg". The ramp limit is now 3.576 deg; 7.125 deg is the straight-line limit that the cut
   descent clamp still uses.
4. **A new static plunge row on idx 14 that the simulation does not confirm** (3.3).
5. **The envelope finding on idx 1 is gone, not reduced.** P20i predicted a fall; the result
   is zero idx 1 samples outside.
6. **idx 1 grew from 14284 to 22920 moves**, nearly all in the 3 deg ramps (370 or 576 moves per
   entry at 0.49 mm steps).

## 8. Reproduction

1. Load the wanaka 21 project in a GUI built from `21bddd593`.
2. Run `generate_all`, then `run_simulation`.
3. Run `export_gcode` with `split_setups: true`. Read the header warnings.
4. Run `python3 -I p22b_scripts/entries.py p22b_scripts <file> "<section>" <offset> 6448 3.0`
   (offset 1 for the Back file, 2 for the Front file) from the scratchpad.
5. Read `get_tool_load_report` (toolpath ids 1 and 20) and `get_toolpath_diagnostics` 0 to 17.
