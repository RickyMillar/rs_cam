# A/B instrument flags — cause analysis (2026-09-08)

Source of the flags: `planning/roughing_strategy_ab_results_2026-09-07.md`
("Caveats"). This document reads the code only. It runs nothing and changes
nothing. Branch `machine-kinematics-confidence`, HEAD `86eaa3d7`.

---

## Flag 1 — G-AIRDENOM: the two air-cut percentages use two TIME BASES

**Verdict: defect. The numerator and the two denominators no longer come from
one time model. `CLAUDE.md` and the code docstring both state an invariant that
the code broke.**

### The two figures share one numerator

`AirCutRatios` (`crates/rs_cam_core/src/simulation_cut.rs:606-638`) divides one
field, `air_cut_time_s`, by two fields of the same struct: `total_runtime_s`
(`:618-625`) and `cutting_runtime_s` (`:631-638`). Both readings on the MCP
`run_simulation` reply come from the same project summary object
(`crates/rs_cam_viz/src/controller/events/compute.rs:2078-2090`, emitted at
`:2113-2115`). The numerator and the population are identical. **The order can
only invert when `cutting_runtime_s` exceeds `total_runtime_s`.**

### What each field counts

`SummaryAccumulator::observe` (`simulation_cut.rs:1206-1258`):

- `total_runtime_s += segment_time_s` on every sample (`:1208`);
  `cutting_runtime_s` only when `is_cutting` (`:1235`); `rapid_runtime_s`
  otherwise (`:1256`).
- `air_cut_time_s` only when `is_cutting` AND
  `engagement.radial_woc_fraction < 0.02` (`:1237-1239`). **Rapid time is NOT
  in the numerator.**
- `segment_time_s` is naive: `segment_len / feed_rate_mm_min * 60`
  (`crates/rs_cam_core/src/dexel_stock/simulation.rs:937`). It carries no
  acceleration, and it uses the PRE-modulation commanded feed.

As accumulated, `total_runtime_s == cutting_runtime_s + rapid_runtime_s`. The
invariant holds here.

### Where the invariant breaks

Two later passes overwrite `total_runtime_s` alone. Neither touches
`cutting_runtime_s`, `rapid_runtime_s` or `air_cut_time_s`.

1. F-034 accel integration: `crates/rs_cam_core/src/compute/simulate.rs:1533`
   (per-toolpath at `:1513`). The docstring at `simulate.rs:1429-1438` states
   the mixing as intentional.
2. F-036b re-timing after feed modulation:
   `crates/rs_cam_core/src/session/compute.rs:3184` (per-toolpath at `:3178`).

After step 2 the denominator is a kinematics-integrated wall clock at the
MODULATED feed, while the numerator and `cutting_runtime_s` stay at the naive
commanded feed. When modulation raises the feed by more than acceleration
costs, `total_runtime_s` falls below `cutting_runtime_s` and the order inverts.

### The arithmetic confirms it

Arm C: `92.63 / 52.70 = 1.758`, so implied `cutting_runtime_s` is
`1.758 × 2901.0 = 5099 s`. Naive cutting time is
`63 575 mm / 750 mm/min × 60 = 5086 s`. The two agree to 0.3 %.

Two-operation run A: implied `cutting_runtime_s` is
`0.332 × 14 245.4 / 0.424 = 11 154 s`. Naive prediction is `4013 s` (rough) plus
`150 034 mm / 1260 mm/min × 60 = 7145 s` (finish), so `11 158 s`. The two agree
to 0.03 %.

### Why the order flips on the two-operation runs

This is **not** a per-toolpath average against a project sum. Both figures are
project sums over one accumulator. The flip is a sign change in
`total_runtime_s − cutting_runtime_s`:

- The rough's modulator RAISED the feed (750 commanded, about 1451 achieved).
  Its kinematic total sits about 1435 s BELOW its naive cutting time.
- The finish's modulator LOWERED the feed (1260 commanded, about 784 achieved,
  from `150 034 mm / 11 486.8 s`). Its kinematic total sits about 4340 s ABOVE
  its naive cutting time.

The finish term is the larger one, so the project sum returns to the documented
order. The direction of the inequality is a property of the modulation mix, not
of the air content.

### Scope of the inversion

The inversion appears wherever a summary whose `total_runtime_s` was
overwritten is read: MCP `run_simulation`
(`controller/events/compute.rs:2113-2115`), MCP `get_cut_trace`
`toolpath_summaries` (`crates/rs_cam_viz/src/app/mcp.rs:5970-5971`), the GUI
banner and the CLI report. It does not appear on `inspect_spans`
(`mcp.rs:6162-6163`), which uses a raw `SummaryAccumulator` that no pass
overwrites. Without modulation the invariant is expected to hold: F-034 only
adds acceleration time, and the naive rapid sample already uses the same
`rapid_feed_mm_min` the integrator uses
(`dexel_stock/simulation.rs:518`).

### Which column of the A/B table is comparable

The published `air_cut_pct_of_total_runtime` column is a mixed-base ratio. Its
denominator is the wall-clock column itself, so a faster arm is punished twice.

Recover the absolute air seconds from the two published columns:
`air_cut_time_s = air_cut_pct_of_total_runtime × total_runtime_s / 100`. Every
rough arm ran at the same commanded feed of 750 mm/min, so these seconds are
proportional to fed-air DISTANCE and are comparable across arms.

| Arm | air seconds | vs A |
|---|---|---|
| A | 1457 | — |
| B | 1450 | −0.5 % |
| C | 2687 | +84 % |
| C2 | 1311 | −10 % |
| D | 1662 | +14 % |
| E | 1321 | −9 % |
| BE | 1414 | −3 % |
| C2E | **1241** | **−15 %** |

This reverses the report's Reading 2. B, E, BE and C2E do NOT raise air. They
shorten the denominator. C2E has the least absolute air of every arm, and it
also wins the finish-stock comparison. Arm C remains the genuine air loser.

Caveat: measurability read `degraded` at 0.2 mm cells on every arm. Treat these
figures as relative only.

### Recommended `CLAUDE.md` wording

Replace the sentence "`air_cut_pct_of_cutting_time` (rapids excluded, always ≥
the total-runtime reading)" with:

> `air_cut_pct_of_cutting_time` (rapids excluded) is what the MCP
> `narrate_toolpath` air-cut line reports. **The two percentages no longer
> share a time base, so neither is reliably the larger (G-AIRDENOM,
> 2026-09-07).** `air_cut_time_s` and `cutting_runtime_s` are naive dexel
> seconds at the commanded feed (`dexel_stock/simulation.rs:937`). Two later
> passes overwrite `total_runtime_s` with a kinematics-integrated wall clock at
> the MODULATED feed (`compute/simulate.rs:1533`, `session/compute.rs:3184`).
> Where modulation raises the feed, the total-runtime percentage reads HIGHER
> than the cutting-time one. The observed factor was 1.76× on a wanaka rough.
> **For a cross-arm comparison use absolute `air_cut_time_s`, not either
> percentage.** The shipped 40 % bars and
> `OperationType::air_cut_high_threshold_pct` were recalibrated (W5B-F4,
> 2026-08-21) against the mixed-base quantity. They are empirically tuned, but
> they move with the modulation lift as well as with air.

The same false invariant is in the code, at `simulation_cut.rs:592-593` and
`:628`. Both docstrings need the same correction.

Found in passing on the same instrument: `session/mod.rs:898` still says the
CLI defaults modulation **off**, while `crates/rs_cam_cli/src/project.rs:203`
says the default flipped to `true` at (g1). That docstring is stale.

---

## Flag 2 — `project.entry_load` on the finish runs

**Verdict: method error in the A/B run, not a code defect. The A/B read a
surface that structurally cannot carry the finding.**

1. **The finish operation IS eligible.** `simulation_triage_with_diagnostics`
   builds `rest_driven` from `stock_source == StockSource::FromRemainingStock`
   (`crates/rs_cam_core/src/session/compute.rs:3637-3642`). The filter ignores
   `enabled`. `armA_finish.toml:819` sets
   `stock_source = "from_remaining_stock"` on "7 3D Finish (R1.5
   drop_cutter)". The gate at `crates/rs_cam_core/src/sim_triage.rs:383-387`
   therefore admits it.
2. **The observed surface cannot publish it.** `project.entry_load` is built
   only in `entry_load_finding` (`sim_triage.rs:803-855`) and pushed into
   `SimulationTriage::actions`. MCP `get_toolpath_diagnostics` calls
   `diagnose_toolpath_with_trace` (`crates/rs_cam_viz/src/app/mcp.rs:3213-3229`),
   a disjoint path that builds no triage. The correct surface is
   `triage.actions` on the `run_simulation` reply
   (`controller/events/compute.rs:2148-2158`), or `get_diagnostics`.
3. **A third state exists that the flag did not consider: NOT MEASURED.**
   `entry_load_observation` counts only samples whose `source_intent` is
   `EntryPlunge`, `EntryRamp` or `EntryHelix` (`sim_triage.rs:717-726`).
   `crates/rs_cam_core/src/dropcutter.rs` sets no `MoveIntent`, and the finish
   carries `entry_style = "none"` (`armA_finish.toml:835`). The only tagging
   path found for this operation is the boundary re-entry descent
   (`crates/rs_cam_core/src/boundary.rs:385`), which can run because the finish
   has an enabled `model_silhouette` boundary. If it produced no tagged entry,
   `is_measured()` is false (`sim_triage.rs:680-684`) and the finding is absent
   because it was NOT MEASURED — never because the pass is clean.

**State: eligible, and unrecorded.** The A/B's "absent" is not an observation.
To settle it, read `triage.actions` from `run_simulation` on a finish run. Then
read `entry_load_observation`'s `entry_samples` and `body_samples` to separate
"under the bar" from "no entry population".

---

## Flag 3 — `mill_shallow_areas = true` produced no sub-pass

**Verdict: by design for a one-Z-level plan. The arm cancelled itself. A
separate reporting blind spot means the A/B's evidence could not have shown a
sub-pass even if one had cut.**

### The dial WAS armed

`crates/rs_cam_core/src/compute/execute.rs:1800-1811` supplies the missing
dials: `shallow_angle_deg` becomes 30°, and `shallow_stepdown` becomes
`depth_per_pass × 0.5`, filtered to `> 0.0 && < depth_per_pass`. Arm E sets
`mill_shallow_areas = true` with neither dial
(`armE_dpp546_shallow.toml:740`), so the step is `5.46 × 0.5 = 2.73` and it
passes the filter. The mask build at
`crates/rs_cam_core/src/adaptive3d/path.rs:351-359` therefore succeeds, and the
sub-pass loop at `path.rs:797-818` (and `:941-960`) does run.

### It ran into empty stock

The Z ladder is built at `path.rs:486-497`. `z_bottom = surface_bottom +
stock_to_leave`, the `while z > z_bottom` loop adds intermediate levels, and
`z_bottom` is always appended as the final level. At `depth_per_pass = 5.46`
the `while` loop yields nothing, so `z_levels = [z_bottom]` — the single level
the A/B observed.

The sub-pass loop descends from `z_level − step` and stops at
`next_main_z = z_level − depth_per_pass` (`path.rs:801`), which is the
COMMANDED step and not the next real level. So the only sub-level is
`z_bottom − 2.73`, entirely below the deepest surface.

`build_material_bool_grid` (`crates/rs_cam_core/src/adaptive3d/clearing.rs:334`)
marks a cell as material when stock stands above
`max(surface_z + stock_to_leave, z_level)`. The main pass at `z_bottom` already
cut every cell down to `surface_z + stock_to_leave`. At `z_bottom − 2.73` that
same floor applies, the grid is empty, and the sub-pass emits no segment.

**A `depth_per_pass` that collapses two Z levels into one also removes the only
gap the shallow sub-pass can subdivide.** Arm E paired the two changes, so the
dial could not act. Any `depth_per_pass` that yields two or more main levels
arms it. The baseline 4.2 does.

### The independent reporting blind spot

The sub-pass dispatches through `clear_z_level_dispatch_no_marker`
(`clearing.rs:77-125`, called at `path.rs:804` and `:948`). That entry point
emits no `DepthPass` span marker and no level event. A sub-pass that DID cut
would still leave `per_depth_pass` at one row and the narration Z ladder at one
level. So "one Z level in the narration" is not evidence about the sub-pass
either way. Both facts stand: inert here, and unobservable everywhere.

---

## Summary

| Flag | Verdict | Site |
|---|---|---|
| 1 G-AIRDENOM | Defect — mixed time bases; the docs state a broken invariant | `simulate.rs:1533`, `session/compute.rs:3184`, `simulation_cut.rs:592` |
| 2 entry_load | Method error — eligible, but read on a surface that cannot carry it; a NOT MEASURED state also exists | `sim_triage.rs:383`, `mcp.rs:3213` |
| 3 mill_shallow_areas | By design for one Z level, plus a real reporting blind spot | `path.rs:486-497`, `path.rs:797-818`, `clearing.rs:77` |
