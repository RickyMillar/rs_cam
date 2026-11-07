# P20 side finding: the chip gate does not grade the capped roughs (2026-11-08)

Read-only investigation. Live GUI build 998ac93a3, project
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`, already simulated. Code read at HEAD
0a6e78116 (`tool_load/` has no local edit). No file under `crates/` changed.

## 1. The defect

The chip gate of these toolpaths reports `load.chipload.unmodeled` with the text "Chipload: not
enough steady cutting to check (the toolpath can be all entry and exit moves)". The verdict is
`Unmodeled(SteadyStateSamplesNotPresent)`. The main roughing passes never get a chip grade.

## 2. The predicate

The verdict does not come from the span predicate `locality::is_steady_state_for_gate`
(`tool_load/locality.rs:156`). That predicate runs later, on the set that is already empty. The
verdict comes from the feed filter before it:

- `crates/rs_cam_core/src/tool_load/chipload.rs:196`: `STEADY_STATE_FEED_FRACTION = 0.95`.
- `chipload.rs:218-241` `steady_state_samples_for_toolpath`: line 223 sets
  `feed_threshold = 0.95 x operation_feed_rate_mm_min`; line 234 keeps a sample only if
  `s.feed_rate_mm_min >= feed_threshold`.
- `chipload.rs:429-432`: an empty kept set with `any_in_cut = true` returns
  `SteadyStateSamplesNotPresent`.
- The reference feed is `tc.operation.feed_rate()`, the stated op feed
  (`session/compute/toolpath_diagnostics.rs:58`).
- The sample feed is the feed of the simulation run. It is at or below the op's
  `feed_max_rate` (G-FEEDMAX: `session/compute/simulation.rs:796` caps the modulator; the
  emitted feed of the roughs is about 3000).

Thus the filter compares a capped sample feed with an uncapped reference. When
`feed_max_rate < 0.95 x feed_rate`, no sample passes, on any op family. The cause is not span
tags, entry or transit labels, a minimum run length or an engagement band.

## 3. Evidence per toolpath

Source: MCP `get_tool_load_report` (live), and the project file for `feed_rate` and
`feed_max_rate` (`feed_optimization = false` on every toolpath). "Cut samples" is the
`cut_force.population.contributing` count (in-cut samples). "Delta" is the
`modulation_summary.median_feed_delta_pct` (emitted feed against the op feed).

| Index (id) | Toolpath | feed_rate | feed_max_rate | ratio | Delta % | Cut samples / offered | Chip verdict |
|---|---|---|---|---|---|---|---|
| 1 (1) | Back bulk rough 18 mm | 8475 | 3000 | 0.354 | -64.60 | 30730 / 120106 | steady_state_samples_not_present |
| 2 (2) | Back rest 6 mm | 5279 | 3000 | 0.568 | -43.17 | 74798 / 280548 | steady_state_samples_not_present |
| 14 (20) | Front bulk rough 18 mm | 8060 | 3000 | 0.372 | -62.78 | 64639 / 212590 | steady_state_samples_not_present |
| 15 (21) | Front 3D rough | 5429 | 3000 | 0.553 | -44.74 | 99749 / 381873 | steady_state_samples_not_present |
| 12 (18) | Face front to T | 6488 | 3000 | 0.462 | -53.76 | 89347 / 108504 | steady_state_samples_not_present |
| 5 (11) | River channel 6 mm | 3165 | 3000 | 0.948 | -5.21 | 23310 / 71956 | steady_state_samples_not_present |
| 17 (24) | Outline profile with tabs | 3165 | 3000 | 0.948 | -5.21 | 120511 / 145289 | steady_state_samples_not_present |
| 9 (15) | V rivers and lakes | 2286 | 3000 | 1.312 | +31.23 | 22295 / 70080 | within |
| 6, 7, 8 (12, 13, 14) | Lake and antenna pockets | 1010-1258 | 3000 | > 2 | -12.7 to +138 | 376-27399 | within |

Checks on the table:

- Back bulk rough: 3000 / 8475 = 0.354, which gives exactly the reported delta of -64.60 %.
  The time-weighted commanded feed is 2984.5 mm/min (`kinematic_utilization`).
- `get_cut_trace` toolpath 1: peak chip 0.08333 mm. This agrees with F3000 at 18000 rpm, 2 flutes
  (3000 / 36000).
- River channel and outline profile: 3000 / 3165 = 0.9479, which is below 0.95 by 0.2 %. These
  two prove the cliff. They are 2D ops, not adaptive ones.
- Every toolpath with ratio < 0.95 that cuts material is `Unmodeled`. Every toolpath with
  ratio >= 0.95 grades. Ids 0 and 17 are `all_samples_air_cut_or_rapid` (a separate verdict).
- Open exception: id 23 "Front scallop finish" (4700 / 3000 = 0.64) grades `within`, but its
  delta is +24.16 % with the cap binding on only 1.9 % of its moves. With a 3000 cap this delta
  cannot occur. The live scallop feeds can differ from the saved file. Read
  `get_toolpath_params` 16 before the fix lands; this agent could not call it.
- The sample field that the filter reads holds the feed of the simulation run, not the
  modulated map (`tests/every_gate_reads_one_feed_p16.rs`, the `ALLOWED` entry for
  `steady_state_samples_for_toolpath`). This investigation did not find which stage writes the
  capped feed into the run. The 0 % pass rate shows that it is below 0.95 x `feed_rate`.

## 4. Root cause

The steady-state filter uses the stated op feed as its reference. Since G-FEEDMAX, the op's
`feed_max_rate` caps every feed stage, so a correct cutting move runs below the stated feed. The
filter then classifies every cutting move as a ramp or plunge transient. The verdict text
("the toolpath can be all entry and exit moves") is false for these toolpaths: the entry
moves are a small part (`runtime_by_intent` toolpath 1: entry 117.9 s of 595.2 s).

The adaptive path shape has no effect. The 2D profile and the face op fail the same way.

## 5. Relation to item 4c (STATUS.md; P18.md lines 310-313, 423)

Item 4c makes the chip and depth gates abstain with the detector reason when
`stock/sim_measurability.rs` reports `NotMeasurable`. That abstain is correct and still open.
This defect is a different path: the trace is measurable, and the gate abstains on a reference
error with a reason that names the wrong cause. Do these items in this order:

1. Fix this defect first. Otherwise item 4c sentries on a capped op read this false abstain and
   pass for the wrong reason.
2. Item 4c then gives a `NotMeasurable` abstain its own `UnmodeledReason` and text, separate from
   `SteadyStateSamplesNotPresent`.

## 6. Proposed fix (5 lines)

1. Add `steady_feed_reference_mm_min` to `ToolpathLoadContext` (`tool_load/mod.rs:438, 494`).
   Its value is `min(feed_rate, feed_max_rate, machine cutting ceiling)`.
2. Set it in `load_context_parts` (`session/compute/toolpath_diagnostics.rs:58`) with the one
   `CuttingCeilingMmMin::capped_at`, the cap that the modulator uses (`simulation.rs:796`).
3. Pass it, not `operation_feed_rate_mm_min`, to `steady_state_samples_for_toolpath` at
   `chipload.rs:411` and `distribution.rs:763-767`.
4. Keep `operation_feed_rate_mm_min` for `CommandedStage` (`chipload.rs:576-581`). That stage
   states the commanded feed.
5. Sentry: an op with `feed_rate` 8475 and `feed_max_rate` 3000 grades its chip (not
   `SteadyStateSamplesNotPresent`). An all-ramp op at `ramp_feed_rate` stays unmodeled.

Blast radius (`rg`):

- `steady_state_samples_for_toolpath`: 2 production callers (`chipload.rs:411`,
  `distribution.rs:763`). Tests: `every_gate_reads_one_feed_p16.rs:73` (the `ALLOWED` text and
  reason change if the read text changes) and `histogram_population_is_the_gate_population_g_cuthist.rs`.
- `operation_feed_rate_mm_min`: 20 lines in `src/`, all in `tool_load/` and one in
  `toolpath_diagnostics.rs`. The fixtures that build the context (`depth.rs:486, 683`,
  `power.rs:578`, `deflection.rs:528`, `mod.rs:806`) need the new field.
- Expected verdict moves on wanaka: 7 toolpaths change from `Unmodeled` to a graded chip.
  Run the FM1 matrix and the `wanaka_e2e_chipload_gate` test after the fix. Neither one is run
  here.

A stronger alternative: drop the feed fraction and classify by move intent (the K4 entry
intents and `is_configured_entry`). The feed fraction is a proxy for "not a ramp or plunge",
and the span ancestry already holds that fact. This changes more sentries; the lead decides.
