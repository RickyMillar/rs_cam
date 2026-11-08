# P20g on wanaka 21: before and after (2026-11-09)

Read-only measurement. HEAD `75133f73b` (P20g). The release GUI started 2026-11-09 12:18:32
from a binary built 12:18:14, later than HEAD. Project
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`. No file under `crates/` changed. No
cargo job ran. No `set_*`, `apply_feeds`, `save_project` or `edit_*` call ran.

"Before" is the reading of `P20_power_gap.md` (HEAD `f168ffab9`) and its G-code
`wanaka_p20gap_*.nc`. "After" is this reading and its G-code `wanaka_p20g_*.nc`. Both sets
are in the scratchpad of session `7a1b80aa-de3a-4f94-9eb3-ffbff208a378`.

## 1. Method

1. `load_project`, then `generate_all` (7 generated, 11 already current, 3 simulations, no
   error), then `run_simulation` (0.2 mm cell).
2. `export_gcode`, `split_setups: true`. The idx 1 section of the Back file has one preamble
   line and 14284 motion lines (14259 G1, 25 G0 after the preamble), the same as before. The
   new file has one header warning line fewer (`load.power.exceeds` is gone), so move `i` is
   line `358 + i` (before: `359 + i`). The X, Y and Z words of all 14284 lines are identical
   before and after.
3. The F words are read with the modal rule (a line without F keeps the last F).
4. `get_tool_load_report` (all toolpaths) and `get_toolpath_diagnostics` for idx 0 to 17.

## 2. Before and after

| Reading | Before (f168ffab9) | After (P20g) |
|---|---|---|
| Move 1697 of idx 1, F (mm/min) | 4516 | **2360** |
| Moves 1695 / 1696 of idx 1, F | 6449 / 4848 | 2630 / 2323 |
| idx 1 fed moves: min / median / max F | 851 / 6448 / 15000 | 851 / 6448 / 15000 |
| idx 1 moves with a lower F | - | 15 of 14259 (none higher) |
| idx 1 fed time at the commanded F (sum of length / F) | 255.5 s | 262.0 s (+6.5 s, +2.5 %) |
| idx 1 `kinematic_utilization.fed_time_s` | not in the report | 264.1 s |
| idx 14 `fed_time_s` | 416.3 s (same G-code, see 4.3) | 416.3 s |
| idx 1 power verdict | `exceeds`, 1.757 kW of 1.125 kW (156 %), samples 127138-127139 | `within`, 1.125 kW of 1.125 kW (100 %), samples 101087-101088 |
| idx 1 graded feed (`feed_explanation.commanded`) | 8475 (steady feed) | 8475 (steady feed) |
| idx 1 `binding_constraint_distribution.power_max` | 0.00077 (about 11 moves) | 0.00105 (15 of 14257 moves) |
| idx 1 `moves_touched`, median delta | not in the report | 13665 of 14257, -23.9 % |
| idx 14 power verdict | within | `within`, 0.769 kW of 1.125 kW |
| idx 14 `moves_touched`, median delta, binding | not in the report | 17822 of 18687, -20.0 %; `chipload_max` 0.524, `machine_max_feed` 0.357, `kinematic_reach` 0.071, `feed_cap` 0.047, no `power_max` |
| Power rows that exceed (project) | 1 (idx 1, 156 %) | **0** |
| Load-report summary | power exceeds on idx 1 | 17 within, 1 exceeds (idx 2, depth of cut 14.71 of 12.0 mm) |
| idx 1 `project.sim_outside_envelope` | 5076 of 30557; worst move 1697, power +102 % | **5076 of 30557; worst move 1697, power +102 %** (unchanged) |
| `project.sim_outside_envelope` findings (project) | idx 1, 2, 6, 7, 8, 10 in the Back header; idx 12, 15 in the Front header | the same 8 toolpaths (counts in 3) |

The 15 moves with a lower F are in four groups: 225-228, 1695-1697, 1807-1808 with 2044,
and 9238-9242. Each new F is 0.41 to 0.90 of the old F. The count 15 agrees with
`power_max` 0.00105 x 14257 = 15.0.

## 3. Findings of the requested ids (after)

Ids: `load.power.*` at caution, critical or blocking; `project.sim_outside_envelope`; a note
"load cap below the chip floor".

| idx | id | Severity | Message (short) |
|---|---|---|---|
| 1 | `project.sim_outside_envelope` | caution | 5076 of 30557; spindle power +102 % (move 1697, depth 10.00, width 17.72) |
| 2 | `project.sim_outside_envelope` | caution | 221 of 76248; vendor axial bound +145 % (move 14212, depth 14.71) |
| 6 | `project.sim_outside_envelope` | caution | 4381 of 30084; vendor depth ceiling +34 % (move 1996, depth 2.67) |
| 7 | `project.sim_outside_envelope` | caution | 195 of 435; vendor depth ceiling +181 % (move 4, depth 5.61) |
| 8 | `project.sim_outside_envelope` | caution | 770 of 900; corner coverage +5 % (move 61, width 1.80) |
| 10 | `project.sim_outside_envelope` | caution | 1144 of 3492; flute guard +5 % (move 4, depth 4.60) |
| 12 | `project.sim_outside_envelope` | caution | 211 of 89347; corner coverage +15 % (move 143, width 17.60) |
| 15 | `project.sim_outside_envelope` | caution | 60 of 60669; vendor axial bound +69 % (move 27163, depth 10.15) |

- No `load.power.*` finding is above `info`. Every modelled toolpath has
  `load.power.within`; idx 0 and 11 have `load.power.unmodeled` (info, no material contact).
- No toolpath has a note "load cap below the chip floor", and no finding id contains
  "floor".
- Out of the requested set, for the record: `project.entry_load` is critical on idx 2 and
  idx 15; `load.depth.exceeds` is caution on idx 2.

Compared with the earlier safety read: the power row changes from "only idx 1 exceeds, 156 %"
to "no toolpath exceeds". The idx 1 peak is now exactly the available power (1.125 kW).

## 4. Surprises

### 4.1 Move 1697 drops to 2360, not to about 730

`P20_power_gap.md` 4.3 expected about 730 mm/min for move 1697. That figure came from a fit
to the `get_safe_envelope` grid at depth 10 and width 17.72. The gate now grades all idx 1
samples at or below 1.125 kW with move 1697 at 2360. Thus the gate's own sample engagement
or power model for move 1697 differs from the fit that 733 came from. With the fit, depth 10,
width 17.72 and F 2360 give 0.65 + 0.92 = 1.57 kW; the gate reports at most 1.125 kW. The
MCP surface has no per-sample reader, so this measurement cannot say which input differs.

### 4.2 The envelope finding does not read the new feed

The idx 1 `project.sim_outside_envelope` text is identical to the before text: 5076 of
30557, move 1697, spindle power +102 %. The power gate and the envelope finding now disagree
on move 1697: the gate says `within` (1.125 kW peak over all samples); the envelope says the
power at move 1697 is about 2.0 x the available power. The envelope finding grades power at a
feed that is not the emitted per-move feed (P20g lowered move 1697 by 48 % and the count did
not move by one sample). A user who reads both sees a contradiction.

### 4.3 idx 14 does not change

The Front file is byte-identical to the before file except one comment line ("the current
simulation is run 4" against "run 3"). idx 14 has no `power_max` binding and a 0.769 kW peak,
so the P20g rule has nothing to lower there. Its cycle time is the same before and after.

### 4.4 The idx 1 cycle time cost is small

Only 15 of 14259 fed moves change. The fed time at the commanded F rises by 6.5 s (2.5 %).
The min, median and max F of idx 1 do not change.

## 5. Reproduction

1. Load the wanaka 21 project in a GUI built from `75133f73b`.
2. Run `generate_all`, then `run_simulation`.
3. Run `export_gcode` with `split_setups: true`. Read line `358 + 1697` of the Back file.
4. Read `get_tool_load_report` (toolpath id 1 and 20) and `get_toolpath_diagnostics` 0 to 17.
