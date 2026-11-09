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

## 9. What the two settings do (read after the edit, 2026-11-10 ~10:40)

The operator asked for the effect, not the edit alone. Read-only: `get_suggest_rationale` under
`constant_chip` (the engine runs the P21a legacy caps until P21c; the ceiling 24000 is carried,
not read), and one regeneration of idx 1 with `entry_style = "helix"`.

### 9.1 Suggest under constant chip (nothing applied)

| idx | Toolpath | Stored RPM / feed | Suggest RPM / feed under constant chip | The limit that binds (P21a names) |
|---|---|---|---|---|
| 1 | Back bulk rough 18 mm | 18000 / 8475 | **14000 / 5444** (ramp 5015, 0.77 of 0.875 kW at 14000) | `LegacyTier` (`milling_rpm_ceiling_for_diameter` large 14000, unsourced F201 row) |
| 2 | Back rest 6 mm | 18000 / 5279 | 20000 / 5865 (ramp 5079) | `LegacySpeedup` (`MAX_SPINDLE_SPEEDUP` 1.5 x the chart, unsourced) |
| 6 | Lake pocket 2 mm | 18000 / 1010 | 22000 / 1235 | `LegacySpeedup` |
| 16 | Front scallop finish | 18000 / 2317 | 22800 / 2935 | `LegacyHeadroom` (24000 x 0.95) |
| 17 | Outline profile | 10610 / 3165 | 15915 / 4748 (ramp 4748) | `LegacySpeedup` over the unsourced SFM RPM 10610 (Q6) |

- The 18 mm roughs (idx 1, 14) go DOWN from 18000 to 14000 rpm and from 8475 to 5444 mm/min
  under the legacy diameter tier. That is the opposite of the operator's intent (24000 ceiling).
  P21c deletes the three `Legacy*` limits and reads the ceiling: do not run Apply under constant
  chip on wanaka before P21c lands.
- The small tools go up along the constant-chip line (feed scales with RPM), capped by the
  unsourced 1.5 x speedup or the 0.95 headroom: the same P21_DESIGN.md section 3 constants that
  P21c removes.
- idx 17 and 5 (10610 "formula" RPM, the unsourced SFM) move to 15915 (1.5 x): the Q6 case.
- Every rationale lists the unsourced rules it accepts (F201): `MAX_SPINDLE_SPEEDUP`,
  `milling_rpm_ceiling_for_diameter.large_ceiling_rpm`, `material.plunge_rate_base`.

### 9.2 idx 1 with `entry_style = "helix"` (regenerated once; no simulation)

| Reading | ramp (section 2) | helix |
|---|---|---|
| moves | 19725 | **47060** |
| cutting distance | 31363 mm | **59096 mm** |
| rapid distance | 1456 mm | 1456 mm |

The entries alone added 27.7 m of cutting path. Section 9.3 reads the G-code to say what the
helix entries are (radius, pitch, the start height and the length per entry).

### 9.3 What the idx 1 helix entries are (G-code read, Back file, section lines 358-47420)

| Reading | ramp (P22f-2a read) | helix (today's 3D helix entry) |
|---|---|---|
| fed length / fed time (G-code, F words) | 31363 mm / 275.2 s | **59096 mm / 558.4 s** (2.0 x) |
| fed length at the ramp feed F6448 | the entry runs only | **29073 mm, 270.5 s** (half the op) |
| fed length at the cut feed F8475 | most of the op | 1127 mm |
| helix-like descents (xy per mm of drop 12-24, drop >= 1.5 mm) | 0 | 91: radius 5.1-5.7 mm (r 5.4), pitch 1.7-1.95 mm/rev; 80 of them drop 1.5 mm, one 9.5, one 10.5 |
| other fed descents (drop >= 0.5 mm) | 58 entry ramps + cut descents | 992, 19820 mm xy |
| core finding (export header, from the partial simulation) | none | **48 moves push the centre into wood; worst move 28527, 7.17 mm over the tip, descent 0.0 deg** |

Move 28527 is `G1 X106.119 Y88.847 Z15.5 F2162`: a straight vertical plunge of 7.09 mm from
Z 22.588 to the level 15.5 at the plunge feed, right after five 0.02 mm descending micro-moves
(the end of a helix or a spiral that the region refused). The tool is the 18 mm non-centre-cutting
cutter (blade protrusion 1.00 mm). That is the "no helix fits" fallback of today's 3D helix entry:
a plunge, not a ramp, and not clamped by `clamp_descents_to_limit` (K5 covers cuts and links, not
this entry). 48 such plunges. The operator's "let's use helix" therefore cannot ship before P22b:
P22b's Q1 fallback (helix at the floor, then ramp at the limit angle, then a plunge the
G-CENTRECUT rules bind) is exactly this case, and its sentry must cover it.

Second surprise: with helix entries half the op runs at the ramp feed 6448 (29 km), where the ramp
op ran at the modulated cut feed. The entry span (helix plus the starter spiral out of it) is fed
at the ramp feed, and on 91 entries per level it is half the path. P22b's bore (R + r_h, one
spiral at the cut feed) and the per-entry drop of entries where no helix fits should remove most
of it; the read after P22b says.

Lead recommendation to the operator: set idx 1 back to `ramp` in the file until P22b lands, or
do not export the Back setup before then. The export warns (48 critical rows) but does not
refuse (K6).
