# C18: the 3D oracle of the C12 arm, and the starter after a straight plunge

Agent C18, 2026-10-24. Worktree `.claude/worktrees/adaptive-head`, branch
`adaptive-instruments`, base HEAD c3d38bf4. No cargo run (the verifier
builds). `rustfmt --edition 2024 --check` is green on both changed Rust
files. Every number below comes from source, from the D11 probe output
(`phase2/d11_c12_probe_output.txt`), or from the script
`phase2/c18/assert_c18.py`, which asserts each derived number
(`python3 -I assert_c18.py`: "C18: all claims hold", 8 s). A prediction is
marked **P**.

## 1. Summary

- The test oracle now follows the 2D rules. Vertical moves are not judged.
  The wall pass is the shipped waterline cleanup, and the test finds it by
  its `WaterlineCleanup` annotation, not by a distance band. The plunge
  exemption is per move, by where the move starts. The judged quantity is
  the exact swept width of the move; the simulation only screens.
- The starter overload (D11 class (iv), moves 57-70 and 1142-1155) is NOT
  the helix radius. The 3D lifter calls the inner turns of the starter
  spiral "air". It reads the stock only at the cutter centre, and that
  centre is inside the plunge hole. The lifter changes those turns to a
  rapid and a second plunge. The material of the missing turns stays, and
  the lap at `R` then cuts 0.75 of D (derived; measured 0.739-0.794).
- The fix is research-gated and implemented: the research arm reads stock
  under the whole cutter disc (`clearing.rs:2735-2755`). The shipped arms
  keep the centre read, so byte parity holds.
- The shipped AgentSearch starter is a different defect: the historical
  starter circle, a full-slot lap (exact 0.999). It is shipped by design
  (the historical rule). Section 5 gives the proposals for the lead.
- The test stays `#[ignore]` with a new `evidence:` reason. **P:** the
  starter class clears, but the other class (iv) wraps (about 29 samples
  on `geodesic_curv`, exact 0.558-0.69) have no isolated cause.

## 2. The filter changes (`tests/adaptive3d_geodesic_arm_c12.rs`)

Line numbers are those of the new file.

| Rule | Before (C12) | After (C18) | Lines |
|---|---|---|---|
| (A) vertical moves | judged; the sim reads the lift after a cut at 1.000 | not judged: a move with no XY travel sweeps width 0; counted as `vertical` | 510-516 |
| (B) the wall pass | a distance band [5.5, 7.46] mm that D11 showed holds no wall pass | the waterline cleanup, found by its tag: from each `WaterlineCleanup` annotation to the next level, region or waterline annotation; counted, its screen count and exact peak printed, not asserted | `waterline_moves` 275-305; use 517-524 |
| (C) the plunge exemption | per sample: a sample within `R` of the hole | per move: a move whose START lies within `R` of the last vertical `EntryPlunge`, no helix or ramp after it (the 2D sentry's `plunge_origin`) | 310-332; use 525-531 |
| judged quantity | the sim radial against 0.546 | the sim radial screens at 0.546; the exact peak of the move judges at 0.471 | `sim_bound` 158, `exact_bound` 165, 545-566 |
| perimeter sweep | the band | the band (kept; no tag exists; section 5, proposal 4) | 179-185 |

**(B), the wall pass, by tag.** The D11 class B cuts (wall distance 2.75
to 3.6 mm, Z 7.5 on the slope) are the waterline cleanup. Each level of
`RegionOrdering::Global` runs `waterline_cleanup` after the clearing
(`adaptive3d/path.rs:1126-1145`). The cleanup pushes a
`Marker(Adaptive3dRuntimeEvent::WaterlineCleanup)` first (`path.rs:1128`),
and `segments_to_toolpath` writes that marker as an annotation at the next
move index (`path.rs:1533-1538`). The evidence that the class B cuts are
this pass:

- The cutter centre is on the mesh contour at the level, offset by `R`.
  At Z 7 the wall slope is at x = 34 + 0.5 · 3 / 6 = 34.25 mm, so the
  centre is at 31.25 mm, wall distance 2.75. The lift adds the 0.5 mm
  leave: Z 7.5. Probe row 998: (-31.25, 19.75, 7.50).
- The AgentSearch control has the same moves at the same coordinates
  (control move 245, arm move 998: the same row). The cleanup does not
  depend on the arm.

The test does not judge the waterline cleanup. It is shipped, it is not a
2D producer, and the arm does not change it. The 2D rule ("every step of
every producer") applies to the 2D planner's producers.

**Why the exact width judges.** The simulation blends partly covered
dexels and lumps a move's removal into one sample (the 2D sentry's doc,
moves 1682 and 4885; `adaptive/CLAUDE.md` "Do not trust one simulated
sample"). The exact reading has no sim cell. So:

- The screen bound keeps the three widths of
  `common::adaptive_islands::discretisation_tolerance`: planner cell 0.5,
  sim cell 0.5, tolerance 0.1. 0.3626 + 1.1 / 6 = 0.546.
- The exact bound drops the sim cell and adds the exact lattice pitch
  (0.05 mm, a tenth of the sim cell, as in the 2D sentry). The planner
  cell stays: the 2D invariant admits a sub-point sliver of one cell. The
  tolerance stays: the emitted chord can sit 0.1 mm from the planned path.
  0.3626 + 0.65 / 6 = 0.4709.
- The bound is not looser. A sample fails when the sim reads over 0.546
  AND its move's exact peak is over 0.471. Before, it failed on the sim
  alone. The 2D sentry judges its exact peak against 0.546 + 0.010 / 6;
  C18 uses the tighter derived 0.471.
- Limit: a sample that the sim reads at 0.546 or less is not read exactly.
  The cost decides this. One exact reading loops over every earlier move.
  The D11 probe took 487 s for about 500 over moves.

The test prints each sample over the screen, with its exact peak, for both
modes (`screened:` lines). The summary line gives the counts of every
class that is not judged.

## 3. The starter after a straight plunge

### 3.1 Where the 3D slice sets the helix radius, and why

- `clearing.rs:3095`, in `clear_z_level_agent_2d_slice`: `params_2d` has
  `entry_helix_radius: 0.0`.
- The field doc (`adaptive/mod.rs:221-228`) says "Ignored by the
  Adaptive3d slices". The 3D emitter makes its own entries
  (`EntryStyle3d`, `segments_to_toolpath`). Under the shipped
  `KeepDownLinks::RetractedByCaller` the 2D planner does not read the
  value (`adaptive/path.rs:156-163`: the helix is used only when `holds &&
  helix > 0`).
- For the research arm (`WithinPassLoad`) the 2D planner reads the value.
  0 tells it that each entry is a straight plunge with a hole of radius
  `R`. The C12 fixture has `EntryStyle3d::Plunge`, so 0 is CORRECT here.
  A helix radius of `HELIX_RADIUS_OVER_D · D` would stamp a helix hole
  that no 3D move cuts. That breaks "Do not stamp stock no emitted move
  cuts" (`adaptive/CLAUDE.md`).
- The 2D harness reads the starter at 0.3114 after a helix entry. That is
  a different entry; it does not show a defect in the 3D value.

### 3.2 What the 3D emission does to the starter

The 2D plan of a piece starts with the spiral starter
(`emit_spiral_starter_pocket`, `adaptive/path.rs:2738`):
`Rapid(medial)`, then one `Cut` from the medial point along an
Archimedean spiral of pitch `min(s, R)` = 2 mm out to `r = R`, then one lap
at `R` (`starter_spiral_points`, `path.rs:2712`). The 2D check exempts the
steps that end inside the plunge hole.

The 3D slice lifts that plan in `clear_one_region`
(`clearing.rs:2700-2920`). The C12 fixture sets
`min_region_cut_length_mm: 0`, so `defer_entries` is false and the sink
stamps each segment when it pushes it.

1. `Rapid(medial)` is pushed and stamped. The plunge hole of radius `R`
   at the medial point is now in the planner stock.
2. The lifter classifies each point of the `Cut` before it stamps it
   (`clearing.rs:2735-2755`, before C18). The shipped read is
   `sample_stock_top_at` (`clearing.rs:1762`): the stock top of the ONE
   cell under the cutter centre. Each spiral point with `r` < about
   2.67 mm has its centre in the plunge hole, so it reads "air".
3. The air run is the spiral from `r` 0 to 2.67: 10.0-12.5 mm long
   (script, 3). The re-promotion keeps an air run as a cut only when it is
   shorter than `air_run_crossover_mm` (`clearing.rs:1865`). At feed
   1000, plunge 500, rapid 5000 (`ASSUMED_RAPID_MM_MIN`) and the descent
   buffer 0.5 mm, the crossover is 5.125 mm at Z 7 (8 mm retract) and
   6.375 mm at Z 4.5 (10.5 mm retract) (script, 2). The run is longer, so
   it stays air.
4. The air run becomes `Rapid(end_pt)` (`clearing.rs:2879`): a second
   entry at `r` = 2.67. That is the `EntryPlunge` of move 57 at
   (-19.71, -11.23), 2.67 mm from the medial point (-18.45, -8.88). The
   lap of moves 58-70 is about that medial point at radius 3 (for example
   move 61 at (-15.45, -8.88)).
5. The cutter disc of the demoted turns would have cut the annulus from
   `R` to about `R + 2.67` round the medial point. That material stays.
   The lap at `R` then meets all uncut material from `-R/2` to `+R` across
   its path. The analytic width is 1.5 R / 2R = 0.75 of D (script, 5). The
   lattice reading of this case gives median 0.70-0.76 and peak 0.70-0.80
   (script, 6a: 0.734 and 0.748). D11 measured 0.739-0.794 on the emitted
   chords.

The level Z 4.5 repeats the same plan (moves 1142-1155).

### 3.3 The fix (research-gated, implemented)

`adaptive3d/clearing.rs:2735-2755`. Under the research arm
(`is_research_arm`, `clearing.rs:2189`) a point is engaged when the stock
stands above `p.z + AIR_THRESHOLD_MM` anywhere under the cutter disc:
`TriDexelStock::max_top_z_in_disc(p.x, p.y, ctx.tool_radius)`
(`dexel_stock/mod.rs:334`, the cells whose centre is inside the disc). The
shipped arms keep `sample_stock_top_at`. `is_research_arm` is false
without the `research` feature, and false for every shipped
`ClearingStrategy3d`. So the shipped emission, and with it
`adaptive3d_emission_byte_parity`, cannot change.

Why the disc read is the right test: the cut point removes material when
its disc meets stock. The centre read is a proxy, and the proxy fails on
any cut whose centre runs over cleared stock while its disc still cuts.
The air demotion stays for a point whose whole disc is cleared. Cost: one
disc scan (13 × 13 cells at R 3, cell 0.5) per cut point, on the research
arm only. Side effect (time only): a run in air whose disc touches stock
at its rim now feeds instead of a rapid.

What the planned starter then reads on the exact lattice (script, 6b):

- Pitch 2 (the first pitch the 2D check tries): the lap from its first
  point at `R` reads peak 0.475 and median 0.251. Only the first 0.5 mm
  sample of the lap is over 0.4709, at 0.475. All later samples are under
  it (0.458 and down).
- Pitch 1 (the first halving): peak 0.263.

This source does not say which pitch the 2D check takes; the verifier
prints the moves.

### 3.4 Is it the shipped AgentSearch defect too?

Not the same mechanism.

- The shipped slice plans the historical starter (the
  `pass_load = None` branch of `emit_helical_starter_pocket`,
  `adaptive/path.rs:2681-2702`): `Rapid` to the rim point `medial + (R,
  0)`, then one circle of radius `R` about the medial point.
- Its air run is the arc within `R` of its own plunge: π R / 3 = 3.14 mm,
  less than the 5.125 mm crossover (script, 4). So the lifter keeps the
  whole circle as a cut.
- The circle is a full slot by design. The lattice reads peak 0.999 and
  median 0.991 (script, 6c). The control rows of moves 55-63 and 390-398
  read 0.910-0.998.
- Also, that branch clears the medial disc in the 2D grid
  (`path.rs:2686`), and no emitted move cuts it. The 3D planner stock
  stays honest, because it stamps only the emitted segments. But the 2D
  slice plans its first agent passes on a grid that is wrong at the
  medial disc.
- `adaptive/CLAUDE.md` says "Adaptive3d keeps the historical rule (byte
  parity)". The 3D AgentSearch makes no claim of the pass load. So this
  is shipped behaviour, not a defect against a current claim.

The other control class (iv) samples (moves 156-226 and 413-471, exact
0.490-0.999) have no isolated cause. The shared component is the
centre-read lifter of step 3.2, which is shipped code. Unmeasured
hypothesis: an agent pass whose centre runs inside an earlier segment's
cleared band is demoted the same way. For a pass at radial width `w` the
centre is `R - w` inside the earlier band (0.8 mm at `w` = 2.18 mm), but
only where the earlier band comes from a segment that was pushed before.
Section 5, proposal 1 names the instrument that decides it.

## 4. Predictions for the verifier run (P, not measured)

The disc read changes the arm path after the first air run. So each count
below is the D11 count with the C18 rules applied; the new path can move
it.

| Class (D11) | `geodesic_curv` before | P after | Why |
|---|---|---|---|
| A vertical lifts | 22 over | 0 judged; in `vertical` | rule A |
| B wall band | 25 over | 0 judged; about 25 in `waterline` screened, exact peak about 0.99 | rule B (tag) |
| C plunge departure | 34 over | most in `departing`; any move that starts more than `R` from the plunge stays judged | rule C |
| (iv) starter, moves 57-70 / 1142-1155 | about 18 over, exact 0.74-0.79 | the inner spiral is a cut; the lap exact peak 0.475 at pitch 2 (one sample at the lap start, 0.004 over 0.4709) or 0.263 at pitch 1 | section 3.3 |
| (iv) other wraps | about 29 over, exact 0.558-0.69 | stay over 0.4709, unless they follow a demoted air run that the disc read removes | not isolated |

- **P1:** `geodesic_curv` and `geodesic_curv_split` still have samples
  over the exact bound. The test stays red when it runs with
  `-- --ignored`. Most likely first failure: a class (iv) wrap, or the
  first lap sample of the starter at 0.475 if the 2D check took pitch 2
  and the emitted move into the lap does not start inside the hole.
- **P2:** both modes still have judged samples at two levels, and the two
  toolpaths differ.
- **P3:** the control's starter laps (moves near 55-63 and 390-398) read
  an exact peak of 0.99 or more. Script, 6c gives 0.999.
- **P4:** every shipped sentry is unchanged (byte parity included). The
  change is behind `is_research_arm`.
- **P5:** run time. The exact reading runs on each screened move, the
  control included. Expect 1-5 min in a debug build. The D11 probe took
  487 s for about 500 over moves; C18 reads far fewer moves.

## 5. Proposals for the lead (DEFECTS.md entries; shipped code, not done)

1. **G-AIRCENTRE.** The 3D slice lifter (`adaptive3d/clearing.rs`,
   `clear_one_region`, the `engaged` read; shipped arms AgentSearch and
   ContourSpiral) reads air at the cutter centre only. A cut whose centre
   runs over cleared stock while its disc still cuts becomes a rapid and a
   re-plunge, and the next cut meets the stock it left. Evidence: the C12
   arm's starter (section 3.2: exact 0.74-0.79, derived 0.75). Impact on
   the shipped arms: unmeasured.
   - Instrument: per level, list the 3D `EntryPlunge` XYs that are not a
     2D `Rapid` or `Link` point of the slice plan. Each one is a
     demotion re-entry. The control fixture is
     `adaptive3d_geodesic_arm_c12` with `ClearingStrategy3d::AgentSearch`.
   - Fix: drop the `disc_read` gate in `clearing.rs:2743`
     (`let disc_read = true;`, then remove the variable). Re-bless
     `adaptive3d_emission_byte_parity` on that measured cause, and run the
     13 `adaptive3d/CLAUDE.md` sentries. Expect less air demotion, so more
     feed time, and more coverage.
2. **G-3DSTARTERSLOT.** The shipped 3D slice plans the historical starter
   circle: a full-slot lap of radius `R` from a plunge at its rim, at full
   level depth (exact 0.999, script 6c; control moves 55-63 and 390-398).
   Its 2D branch also clears the medial disc that no move cuts
   (`adaptive/path.rs:2686`). It is shipped by design (historical rule,
   byte parity). Record it, so that no 3D load claim cites the 3D
   AgentSearch.
3. **G-WATERLOAD.** The waterline cleanup of each level cuts the band
   between the outer clearing pass and the wall at 0.72-0.99 of D (exact,
   D11 class B; the same on both arms). It is shipped and makes no load
   claim. The C18 test prints its exact peak.
4. **A `PerimeterSweep` tag.** The perimeter sweep has no annotation, so
   the test still finds it by a distance band. That band also hides the
   arm's samples at a wall distance of 5.5-7.46 mm. A
   `Marker(Adaptive3dRuntimeEvent::PerimeterSweep)` before each sweep in
   `clear_one_region` would let the test find it by tag. That is a new
   public enum variant (`Adaptive3dRuntimeEvent`, `mod.rs:406`). Every
   exhaustive match of the enum (for example `set_z_level_metrics`,
   `mod.rs:455-465`, the label, `compute::spans`) must take it. Annotation
   consumers see one more event; the emitted moves do not change.
5. **The arm's helix radius (only for a helix fixture).** If a C12 fixture
   uses `EntryStyle3d::Helix`, the arm must tell the 2D planner the helix
   hole that the 3D emitter cuts. Otherwise the 2D planner exempts plunge
   departures that the 3D path does not have. Lines for `clearing.rs`
   near 3095 (research arm only): set `entry_helix_radius` to the radius
   that the 3D helix emitter uses when `is_research_arm(..)` and the entry
   style is `Helix`, else 0. C18 did not find that radius in one source
   line, and the C12 fixture does not need it, so C18 wrote no code.

## 6. Files

| File | Change |
|---|---|
| `crates/rs_cam_core/src/adaptive3d/clearing.rs` | 2735-2755: the research-gated disc read of the air classifier |
| `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs` | the oracle of section 2; module doc; new `#[ignore = "evidence: ..."]` reason |
| `planning/adaptive_research_2026-10-23/phase2/c18/assert_c18.py` | asserts each derived number of this note |
| `planning/adaptive_research_2026-10-23/P3_C18_3d_oracle.md` | this note |

C18 did not edit the `adaptive/` files that C16 owns. The worktree also
shows `adaptive/research_geodesic.rs` and `finish/scallop_isofield.rs` as
modified; those are not C18's changes.

## 7. Verifier commands

From the worktree, through the lane script, each with a timeout. This is
the C12 §7 list. The C12 test is `#[ignore]`, so it runs with
`-- --ignored`. The f152 sentry checks the new ignore reason.

```sh
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head
timeout 120 scripts/cargo_lane.sh fmt --all -- --check
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
# the C12 oracle (ignored; run by name)
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture
# the ignore-reason sentry
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --test every_ignore_states_its_kind_f152
# shipped neutrality: byte parity and the adaptive3d sentries, without research
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support --test adaptive3d_emission_byte_parity
for t in adaptive3d_boundary_clear_parity adaptive3d_keep_down_link_f038b \
  adaptive3d_entry_stock_aware adaptive3d_entry_coalescing_f038 agent_search_coverage \
  adaptive3d_subtool_channel_gouge adaptive3d_global_gate_drapes_below_floor \
  adaptive3d_plan_matches_merged_path_g_plansimgap adaptive3d_planner_never_ahead_of_emitted_path \
  adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom \
  adaptive3d_plans_every_slice_piece_g_adapt3dpieces adaptive3d_by_area_cells_confine_jobs \
  adaptive3d_by_area_matches_global_stock; do
  timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support --test "$t" || echo "FAIL $t"
done
# the same byte parity WITH research (the env var unset): must also be green
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_emission_byte_parity
# 2D lib tests and the research kernel tests (C16 edits them; run for the union)
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --lib adaptive::research_geodesic
# both clippy configs
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
# the derived numbers
python3 -I /home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase2/c18/assert_c18.py
```

Read in the C12 test output:

- the summary line per arm: `vertical`, `departing`, `waterline`, and the
  counts over the screen and over the exact bound;
- each `screened:` line, with its exact peak.

Compare them with section 4.

## 8. What C18 did not do

- C18 ran no build and no test. The clippy and compile state of the two
  Rust files is unverified. The casts and lint expectations copy the
  shipped 2D sentry.
- The cause of the other class (iv) wraps (D11: 73 of 81 at x < 0) is not
  isolated. The first read after this run: whether each wrap follows a 3D
  entry that is not a 2D entry (proposal 1's instrument, on the arm).
- The shipped impact of G-AIRCENTRE is not measured.
