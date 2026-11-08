# P20 side finding: the modulator does not hold the power the gate grades (2026-11-09)

Read-only investigation. HEAD f168ffab9; the release GUI ran this HEAD. Project
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`, toolpath idx 1 (id 1) "Back bulk rough
18 mm": 3D Adaptive, 18 mm 2F flat end mill, stepover 5.5, depth 10, `trochoid_cap_mult` 1.6,
feed 8475, 18000 rpm, White Oak, 1.125 kW available. No file under `crates/` changed. No cargo
job ran.

## 1. Question

The power gate says EXCEEDS: peak 1.757 kW at samples 127138-127139. The envelope finding puts
the worst sample at move 1697 (depth 10.00 mm, width 17.72 mm, almost a full slot). The
modulator (ConstrainedMax) reports `power_max` as the binding limit on only 0.077 % of the moves.
Why does the modulator not lower the feed where the gate finds the overload?

## 2. Evidence

### 2.1 Live readings (MCP, after `generate_all` on HEAD; the trace was current)

| Reading | Source | Value |
|---|---|---|
| Power verdict | `get_tool_load_report`, toolpath 1 | `exceeds`, `peak_kw` 1.7574, `available_kw` 1.125, samples 127138-127139, population 30557 of 120105 |
| Deflection peak | `get_toolpath_diagnostics` 1 | the same samples 127138-127139 (8 um) |
| Depth evidence | same | samples 127149-127150, depth 10.000 mm |
| Envelope finding | same | 5076 of 30557 samples outside; worst: move 1697, depth 10.00 mm, width 17.72 mm |
| Binding share | `modulation_summary` | `machine_max_feed` 0.748, `chipload_max` 0.232, `kinematic_reach` 0.011, `feed_cap` 0.009, `power_max` 0.00077 (about 11 of 14257 moves) |
| Machine | `kinematic_utilization` | `max_feed_mm_min` 15000; accel X/Y 1200, Z 550 mm/s^2 |
| Band | `feed_explanation` | point 0.17913 mm/tooth; 0.17913 x 18000 x 2 = 6448.6 mm/min |

### 2.2 The emitted feed of move 1697 (G-code export)

`export_gcode` wrote `wanaka_p20gap_1_1_Back.nc` (scratchpad of this session). The toolpath
section has 14284 motion lines after one preamble line, and 14259 G1 lines. The load report
gives `moves_total` 14284 and `fed_moves` 14259, so line `359 + i` is move `i`.

| Move | Feed (mm/min) | Length (mm) | End Z |
|---|---|---|---|
| 1695 | 6449 | 4.26 | 15.934 |
| 1696 | 4848 | 4.26 | 15.500 |
| **1697** | **4516** | **6.35** | **15.500** |
| 1698 | 2699 | 1.41 | 16.500 |
| 1699 | 4284 | 1.13 | 16.500 |
| 1700 | 6850 | 6.36 | 16.500 |

Move 1697 is a fed cut on the Z 15.5 level (depth 10 below the stock top). The modulator did
not skip it: 4516 is a modulated value, not the op feed 8475. The section header says "feeds:
modulated from simulation run 1"; the run-3 modulation changed no feed (`moves_touched` 0), so
run 3 stamped the same per-move feeds into `predicted_feeds`.

### 2.3 The power model, fitted to the live envelope grid

`get_safe_envelope` 1 prints power at grid points from the same `PowerTerms` law. The rows
at ae 2.589 mm (two feeds) give `Ks` = 93.6 N/mm^2 and `F_edge` = 5.91 N/mm for the
rectangle section `P = Ks ap ae F / 6e7 + F_edge ap (pi D n) (z psi / 2 pi) / 6e7`,
`psi = acos(1 - 2 ae / D)`. The fit reproduces the ae 5.16 and 7.73 rows within 1-2 % and the
ae 10.29 row 5 % low, so the fit is a lower bound near a slot. With the fit:

| Quantity | Value |
|---|---|
| Edge term at ap 10, ae 17.72 | 0.922 kW (feed-free) |
| Power at ap 10, ae 17.72, F 4516 (emitted) | 2.17 kW |
| Feed that gives 1.757 kW at ap 10, ae 17.72 | about 3020 mm/min |
| Power cap of the slot sample (`feed_for_kw(1.125)`) | 733 mm/min |
| Mean ae whose power cap is 4516 mm/min (ap 10) | 8.9 mm |
| Width that gives 1.757 kW at ap 10 and F 8475 | 9.4 mm |

### 2.4 What this shows, and what it cannot show

The MCP surface has no per-sample reader. Thus no call maps sample 127138 to a move index or
prints its feed. Two readings fit 1.757 kW:

- The peak sample is the move-1697 slot sample (ap 10, ae 17.72), graded at the reached feed
  of that move, about 3000 mm/min (P20c: `min(modulated 4516, reached)`; move 1697 ends in a
  1 mm Z rise at 2699 mm/min, Z accel 550 mm/s^2). The deflection peak sits on the same samples.
- The peak sample is a different sample, ap 10 and ae about 9.4, graded at 8475.

The brief says the gate "graded those samples at 8475". The load report carries no
per-sample feed. The 8475 that the cards print is the steady feed (`ctx.steady_feed`, P20e),
not the sample feed that `effective_feed_for_sample` reads. I could not confirm 8475 for
sample 127138. Both readings lead to the same root cause (section 3): in each, the modulator
caps the feed on an engagement smaller than the engagement the gate grades.

## 3. Root cause (candidate 1, confirmed in code)

### 3.1 The modulator caps power on a time-weighted MEAN engagement per move

`crates/rs_cam_core/src/session/compute.rs:2113-2117`:

```rust
    // Aggregate per-move engagement (time-weighted mean over the
    // move's samples). Samples filter on `is_cutting` so air-cut
    // and rapid moves stay at default `(0.0, 0.0)` engagement —
```

`compute.rs:2137` weights each sample by `segment_time_s`; `compute.rs:2167` and `:2171`:

```rust
            let sim_radial = radial_num[i] / w;
            ...
            let axial_mm = axial_mm_num[i] / w;
```

Width and depth are averaged separately. The power cap then builds ONE engagement from the two
means (`dressup/feed_modulation.rs:649`, `:736-759`):

```rust
    let woc_eff = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    ...
                let terms = crate::tool_load::power::PowerTerms::of(
                    pow.cutter.engagement(axial_mm, woc_eff).power_inputs(
    ...
                match terms.feed_for_kw(pow.available_kw) {
```

The deflection cap (`feed_modulation.rs:705`) and the force-budget cap (`:788`) read the same
mean engagement. `band_mid_feed_for_move` (`:1014-1017`) reads the mean width too.

### 3.2 The gate grades each sample at its peak

`tool_load/power.rs:353-357` grades every steady sample at its own feed and keeps the maximum
(`power.rs:390`). The width is the measured arc, `feeds/engagement.rs:218`:
`let ae_mm = r * (1.0 - psi.cos());`.

### 3.3 The sample coalescer widens the gap inside one sample

`dexel_stock/sample_coalesce.rs:29-37` states the split on purpose:

```text
//! * **Time-weighted means** — `engagement.radial_woc_fraction`,
//!   `axial_doc_mm`, `engagement.axial_doc_fraction`: the three readings the
//!   feed modulator averages per move ...
//! * **Maxima** — `axial_engagement_mm` ..., the arc and the chip thicknesses: the
//!   peaks the depth, deflection, chipload and power gates read.
```

On an uncoalesced sample the two widths agree: the stamp writes
`arc = acos(1 - 2 w)` (`dexel_stock/stamping.rs:1502`), so `r (1 - cos arc) = w D`. On a
coalesced sample the gate reads the MAX arc and the modulator reads the MEAN width.

### 3.4 The arithmetic on move 1697

Move 1697 cuts at the Z 15.5 level. Its planned width is the adaptive cap (5.5 x 1.6 = 8.8 mm);
for a short part of the move the cutter is in a near-full slot (17.72 mm). A move with a mean
width of 8.9 mm at depth 10 gets a power cap of 4516 mm/min, the emitted feed. The slot
sample of the same move needs 733 mm/min. One move carries one F word, so the spindle must
hold the worst point of the move. The mean lets the slot through.

The chipload cap does not show the gap: P11b2 already takes the MINIMUM lift over the move's
samples (`compute.rs` thinning loop after `:2213`; `PerMoveEngagement::thinning_lift`,
`feed_modulation.rs:226-236`). The power, deflection and force-budget caps never got the
same worst-sample rule.

### 3.5 The other candidates

| Candidate | Verdict | Reason |
|---|---|---|
| 2. The stamp misses the move, or the gate reads the commanded feed | No | `simulation.rs:1036-1038` writes every `outcome.per_move` entry into `predicted_feeds`; `:1062-1066` lowers it to the reached feed. The modulator writes a per-move entry for every move (`feed_modulation.rs:1376`). The gate reads that map (`tool_load/mod.rs:76-87`). |
| 3. Different power models | No (model); yes (inputs) | Both sides call `PowerTerms::of` on `Engagement` power inputs, the same line, section, rpm and engaged diameter. Only the inputs differ: mean width and depth against the sample's peak arc (3.1-3.3). |
| 4. An entry or transit move that the modulator skips | No | Move 1697 is a fed G1 on the level, emitted at a modulated 4516, not at 8475. The gate counted its samples, so it is not a configured entry or a phantom transit. |
| 5. Other | Side note | The peak 1.757 kW fits a graded feed of about 3000 on the slot sample (2.4), not 8475. The 8475 on the card is the steady feed, which no per-sample grade reads. |

## 4. Fix proposal

### 4.1 Rule

For a cap that the spindle or the tool must hold at every point of a move, the binding value
of the move is the minimum, over the move's cutting samples, of that cap at each sample's own
engagement. Power is affine in feed (`P = shear x F + edge`) with a shared budget, so
`cap_move = min_s feed_for_kw(terms_s)` is exact. The deflection and force-budget caps follow
the same rule (`min_s fz_cap_for_peak_force(engagement_s)`).

A single "worst" pair (max depth, max width) is a safe upper bound, because power, force and
deflection rise in both depth and width. It over-caps a move where the two maxima fall on
different samples. The exact per-sample minimum costs little: idx 1 has about 8 samples per
move.

### 4.2 Shape

1. In `modulate_annotated_against_trace` (`session/compute.rs:2016`), collect per move the
   engagement of each cutting sample through `feeds::engagement::Engagement::from_sample`
   (the gate's builder: depth `axial_doc_mm`, width from the arc). Keep the time-weighted
   means for the fields that stay means (the zero-engagement test, the planner floor).
2. Give the power, deflection and force-budget arms of `max_safe_feed_for_move` the move's
   sample engagements; each arm takes the minimum cap. A side table in
   `ModulationContext` (one slice per move) avoids a new field in `PerMoveEngagement`.
3. Population: use the gate's own filter for the samples it removes as phantom
   (`tool_load::locality::is_phantom_transit`); their dexel depth is inflated. Lead decision:
   whether configured-entry samples (the gate's `entry_spike` advisory) also bind the cap.
4. Update the `sample_coalesce.rs:29-34` contract text: the modulator then reads the maxima
   too.

### 4.3 Expected effect on idx 1

Move 1697 drops from 4516 to about 730 mm/min (power, slot sample). The 5076 samples outside
the envelope sit in moves that slow down in the same way; the cycle time rises. The real cure
for the slot is upstream: the adaptive planner lets the width reach 17.72 against a cap of
8.8 (the open adaptive engagement-control item). The feed rule above still applies, because a
feed stage must hold the power of the path it gets.

### 4.4 Blast radius (`rg` at f168ffab9)

| Item | Count |
|---|---|
| `PerMoveEngagement {` struct literals | 44 (1 with `..`): `feed_modulation.rs` 23 (tests in the file), `session/compute.rs` 1, 13 files under `crates/rs_cam_core/tests/` 20 |
| Callers of `modulate_annotated_against_trace` | 2: `session/compute/simulation.rs:802` (post-sim pass), `session/compute.rs:1952` (strategy advisor) |
| `effective_axial_mm(` call sites | 5, all in `feed_modulation.rs` |
| Arms that read the mean engagement for a load cap | 3 in `max_safe_feed_for_move` (deflection `:705`, power `:736`, force budget `:788`); `band_mid_feed_for_move` `:1014` reads the mean width for the chip target only |

A side table (4.2 step 2) leaves the 44 literals unchanged; a new field in
`PerMoveEngagement` changes 43 of them.

Sentries to read or extend: `constrained_max_modulation_f039`,
`one_engagement_before_and_after_the_simulation_k7`,
`every_machine_limit_runs_through_every_move_p20c`,
`modulation_reads_the_depth_in_mm_g_axialunits`, `the_modulator_keeps_the_lifted_feed_p11b2`.
New sentry: a move with one slot sample among thin samples gets a power cap at or below the
gate's grade of that slot sample (the power twin of P11b2's min-lift sentry).

## 5. Reproduction

1. Load the wanaka 21 project in a GUI built from f168ffab9.
2. Run `generate_all`; it simulates as part of the plan.
3. Read `get_toolpath_diagnostics` 1 and `get_tool_load_report` (toolpath 1).
4. Run `export_gcode`; read line `359 + 1697` of the Back file.
