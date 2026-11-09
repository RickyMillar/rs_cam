# Wanaka 21 read after P22f-2a, P21a and P21b (2026-11-10)

Read-only measurement, then the two operator-approved edits (section 7). HEAD `cf5cec41a`
(code at `f06bde4e2`, P21b). The release GUI started 09:10 from a binary built 06:36, after every
code commit. Project `planning/rivmap_block_job/nz_wanaka_21_operator.toml`. "Before" is the
P22-0b read (`P22-0b_wanaka.md`, HEAD `21bddd593`). Between the two reads: P22-0c-4 to 0c-6b (the
tool body zones on the live stock, one cutting cap), P22a, P22f-1, P22f-2a, P21a, P21b.

## 1. Method

`load_project`; `generate_all` (`timeout_s` 60, then the plan ran about 6 min to step 21 of 21;
18 toolpaths `Done`, none stale); `run_simulation` at the stored 0.2 mm cell; `get_tool_load_report`,
`get_project_diagnostics`, `get_toolpath_diagnostics` 1, 2, 14; `export_gcode` with
`split_setups: true` (three files in the lead's scratchpad `p22f_wanaka/`). The G-code read of one
move: toolpath move `k` is motion line `k` of its section after the section header.

## 2. The simulation verdict

| Reading | Before (P22-0b) | After |
|---|---|---|
| Verdict | "52.0 % air cutting", 0 collisions | **"ERROR: 555 holder/shank or obstacle collisions"** |
| Air cut of total runtime | 52.0 % | 50.5 % |
| Rapid collisions | 0 | 0 |
| idx 1 moves | 22920 | 19725 |
| idx 14 moves | (not recorded) | 22517 |
| Export warnings | 21 | 21 (a different mix, section 5) |

The 555 collisions are the P22-0c zone check (0c-4: the shank, holder and shoe zones measured on
the live stock), new since P22-0b. They are real tool-body contacts of this project, not a
simulation defect (section 4).

## 3. Envelope (P20i): `project.sim_outside_envelope`

| idx | Toolpath | Before (P22-0b) | After |
|---|---|---|---|
| 1 | Back bulk rough 18 mm | no finding | no finding |
| 2 | Back rest 6 mm | 221 of 76239; vendor axial bound +145 % (move 14235, depth 14.71) | 304 of 56277; vendor axial bound **+245 %** (move 11955, depth 20.70, width 1.65) |
| 6 | Lake pocket 2 mm, 11 lakes | 4381 of 30084; depth ceiling +34 % | 4607 of 30364; the same worst |
| 7 | Lake pocket 2 mm, 1 lake | 195 of 435; +181 % | the same |
| 8 | ANTENNA pocket | 770 of 900; corner coverage +5 % | the same |
| 10 | Day strip slit V60 | 1144 of 3492; flute guard +5 % | the same |
| 12 | Face front to T | 211 of 90004; corner coverage +15 % | the same |
| 15 | Front 3D rough | 60 of 61198; vendor axial bound +69 % (depth 10.15) | 44 of 67395; **+7 %** (move 32758, depth 6.40, width 5.60) |

- idx 2 move 11955 is `G1 X153.050 Y43.388 Z4.800 F4978` (Back file line 32045): the cavity floor,
  20.7 mm under the stock top (25.5). The sample is a 1.65 mm wide sliver standing the full cavity
  height. The depth gate reads the same 20.7 mm (`load.depth.exceeds`, limit 12.0 = 2 x D, the
  unsourced rule; the one `exceeds` row of the load report). Before, the same class read 14.71 mm.
  The taller read comes from the P22-0c-6a body spans (the engaged depth is read up the body, not
  capped at the flute). Not diagnosed further here.
- idx 15 fell from +69 % at depth 10.15 to +7 % at 6.40: fewer and shallower outside samples on
  the new tool-6 stock.

## 4. Tool body (P22-0c-4/5/6): new since P22-0b

| idx | id | Finding | Position (G-code frame) |
|---|---|---|---|
| 6 | `project.holder_collision` (critical) | 445 fed moves put the **shank** into the stock; worst 2.62 mm of wood over the shank bottom, 8.00 mm above the tip | move 30109, X89.339 Y421.698 Z5.875 |
| 7 | `project.holder_collision` (critical) | 110 fed moves; worst 4.51 mm over the shank bottom | move 192, X240.351 Y193.396 Z3.994 |
| 3 | `tool.flute_overreach` (caution) | pin drill: 5 moves with 30.00 mm of wood over the tip, flute 25.00 mm | move 91, Z-4.500 |
| 9 | `tool.flute_overreach` (caution) | V rivers: 981 moves, 12.91 mm over the tip, flute 8.15 mm | move 6226, X240.711 Y191.500 Z3.593 |
| 10 | `tool.flute_overreach` (caution) | day slit: 7 moves, 7.44 mm over the tip, flute 5.50 mm | move 15, Z-0.970 |
| 14 | `safety.non_centre_cutting_descent` (critical) | 1 move, 1.03 mm over the tip at 7.1 deg (the same cut descent as before, then move 17734) | move 18235, X116.434 Y291.638 Z17.983 |
| 1 | `safety.non_centre_cutting_descent` | before: 1 move (17183) | **none**: `safety.non_centre_cutting_clear` |
| 14 | `safety.non_centre_cutting_plunge` (static, critical, before) | the P22-0b open row (3.3) | **gone**: the simulation row supersedes it |

What they say about the project (for the operator, not the code):

- The 2 mm lake tool (8 mm flute, 3.175 mm shank "assumed: measure") cuts the lakes 10.6 mm below
  the local surface (8.00 + 2.62). Its shank rubs the lake walls on 445 + 110 moves. Either the
  shank is necked (measure it) or the lakes need a longer-flute tool.
- The pin drill goes 4.5 mm into the spoilboard through 25.5 mm of stock: 30 mm of hole against a
  25 mm flute. The shank enters the hole.
- The V-bit rivers cut with 12.9 mm of wood above an 8.15 mm flute (the V-bit flank above the
  flute meets the river wall on 981 moves).

The core descent message still prints "the limits are a 1.00 mm plunge, and at most 1.00 mm of
descent along any 8.00 mm of path" (P22-0c-2 text; the P22-0b surprise 3 is fixed).

## 5. Power and time

| Reading | Before (P22-0b) | After |
|---|---|---|
| idx 1 power | within, 1.125 / 1.125 kW (the modulator at the limit) | within, 1.125 / 1.125 kW; contributing 46140 (was 31089) |
| idx 1 modulator | `power_max` binding 0.00079 | touched 19651 of 19694 moves; binding: chipload_max 0.44, kinematic_reach 0.02, feed_cap 0.02 |
| idx 1 fed time | 303.7 s | **275.2 s** (-28.5 s, -9 %; the sweep is gone, P22f-1) |
| idx 14 power / fed time | within 0.769 kW / 430.8 s | within 0.769 kW / 462.0 s (+31 s) |
| idx 2 power | (not recorded) | within 0.457 kW; the depth gate exceeds (20.7 of 12.0 mm) |
| idx 15 power | (not recorded) | within 0.301 kW |
| Load report summary | | 17 within, 1 exceeds (idx 2 depth), 0 unmodeled rows beyond the two air-cut faces |
| Project power rows that exceed | 0 | 0 |

The idx 1 peak still sits at the available power because the P20g modulator lowers the feed to
the limit on a rectangle reading (P22f-2d: 0 of 3731 idx 1 wide samples are wide by section).
P22f-2e (the operator's Go, 2026-11-10) is the fix; this read is its "before".

## 6. Every critical finding (after)

| idx | id | One line |
|---|---|---|
| 6, 7 | `project.holder_collision` | the 2 mm shank in the lake walls (section 4) |
| 14 | `safety.non_centre_cutting_descent` | move 18235, the same cut descent as P22-0b |
| 2 | `project.entry_load` | 46 of 44294 entry samples remove more than 2.13 mm; peak 14.78 mm at (138.0, 34.8, 10.51) (before: 69 of 44873, peak 15.92) |
| 15 | `project.entry_load` | 1825 of 28721 entry samples remove more than 0.59 mm; peak 4.19 mm at (348.6, 192.5, 17.04) (before: 3074 of 32049, peak 8.95) |

No finding is blocking; the export wrote three files. The two `entry_load` rows are the P22b/P22c
open items; idx 15 halved its count and peak on the new stock.

## 7. The two operator-approved edits (2026-11-10 morning)

1. `set_spindle_strategy { constant_chip, rpm_ceiling 24000 }` (operator: "yes, try constant chip
   on wanaka"). Under P21b the engine runs the legacy caps; the ceiling binds in P21c.
2. idx 1 `entry_style = "helix"` (operator: "let's use helix"). The P22b bore applies to it once
   P22b lands; until then idx 1 cuts the existing 3D helix entry at the op radius.

The GUI did not return from `set_spindle_strategy` (section 8, surprise 2), so the lead wrote the two
edits into the project file directly: `[job.post] spindle_strategy = "constant_chip"`,
`spindle_rpm_ceiling = 24000`, and `entry_style = "helix"` in the `Back bulk rough 18 mm` op block
(the file parses; the dressup `entry_style = "none"` of that op is untouched). The GUI's unsaved
state is discarded on its restart. The next read loads this file.

## 8. Surprises

1. **The zone check changed the verdict from clean to 555 collisions** on a project the operator
   has cut. The findings are real geometry (section 4). The operator should measure the 2 mm
   tool's shank and the pin-drill depth before the next cut.
2. **`set_spindle_strategy` held the GUI thread.** After the call the frame loop stopped for over
   two minutes at 100 % CPU on the main thread, and `gui_stderr.log` printed the depth-gate
   `Exceeds` line (idx 2, 20.7 mm) every 11 s: the policy change re-runs a tool-load gate over the
   full cut trace, once per op, on the GUI thread. After 7 minutes and 35 gate lines (more than the 18 ops) the call had not returned; the
   lead stopped waiting, edited the file (section 7) and asked the operator to restart the GUI. A
   defect to log and fix: the handler re-runs a gate over the full cut trace per op on the GUI
   thread, and more than once per op (the F216 memo should take the policy as an input).
3. **idx 2 reads 20.7 mm deep** where P22-0b read 14.71 at the same class (section 3).
