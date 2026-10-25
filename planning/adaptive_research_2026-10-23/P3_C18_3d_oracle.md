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

## C30. Corne "3D Rough 1": which producer cuts over the bound (2026-10-25)

Measurement only. No code change, no cargo. Data: the D18 run
`phase3/fix5/corne/geodesic_local_zigzag_pick/`, the D17 runs
`phase3/fix4/corne/geodesic_local_{zigzag,oneway}/`, and the AgentSearch
control `phase3/fix2/simart/corne/agent/` (the D-FIX2 rerun with
`--sim-artifact`; `fix1/corne/agent` has no `simulation.json`). Scripts and
output: `phase3/c30/` (`run_c30.sh` runs all four; `c30_output.txt`; 70 s).

### C30.1 The method

- **The length.** The measure of `fixture3d_v2.load_share`: the XY length
  from the previous sample, summed over the cutting samples of toolpath 1
  with `linear` or `arc` kinematics. It gives the lead's number exactly:
  1375.8 mm over 0.471 on `geodesic_local_zigzag_pick`, 4.14 % of
  33250.9 mm.
- **The level.** The `Adaptive Z` annotations (generator move indices),
  mapped to final move indices through the `Pass N` annotations and the
  semantic `entry` items (same `pass_index`).
- **The phase**, in the planner's order per level (C22 §10d):
  1. Perimeter sweep: the level's first entry run. It is one closed loop
     at every level: 456.7 mm, closed to 0.5 mm, at Z 17; 370.6 mm,
     closed to 0.62 mm, at Z 11, 5 and 0.5.
  2. The region body: from the level's second entry (the starter plunge)
     to the first waterline entry.
  3. The waterline cleanup: from the `Waterline cleanup` annotation to the
     next level.
- **The producer, in this order:**
  - vertical (no XY step, or `plunge` kinematics): entries, peck
    retracts, link lifts;
  - perimeter sweep;
  - waterline cleanup;
  - starter: the run of the body's first entry;
  - keep-down link traverse: body samples with `linking` intent;
  - plunge departure: a move that starts within R of the last vertical
    plunge (C18 rule C);
  - draped: a body sample with |dz| > 0.01 mm;
  - the rest of the body: wraps, turns and mop. The trace does not
    separate these three, but section C30.2 shows that the geodesic
    body has nothing over 0.471.
- **The bounds** (`attribute.py` derives them):
  - The lead's bounds: exact 0.471 and screen 0.5126. These are for
    s 2.0, with the 0.3 mm sim cell.
  - Corne's own stepover is 1.2. Its ceiling is
    (1 - cos(1.05 · acos(1 - 1.2/3))) / 2 = 0.2188. So its exact bound is
    0.2188 + 0.65/6 = 0.3272, and its screen is 0.2188 + 0.9/6 = 0.3689.
    The 0.471 bound under-counts on Corne (D-FIX2 says so too).
- **The exact oracle** (`exact.py`): C18's `exact_width_at` in numpy, on
  the sample path. The earlier samples of the toolpath at or below the
  sample's Z + 0.01 are stamped as discs of R. They are 0.3 mm apart, so
  the cusp is 0.004 mm. The lattice pitch is 0.05 mm. `from` is the
  previous sample. The oracle reads every linear or arc sample over
  0.3272.

### C30.2 Per producer: XY length over 0.471 (sim), all four levels

| Producer | zigzag_pick (D18) | zigzag (D17) | oneway (D17) | AgentSearch |
|---|---|---|---|---|
| perimeter sweep | 1026.1 | 1026.1 | 1026.1 | 1146.5 |
| waterline cleanup | 317.1 | 317.1 | 313.2 | 364.2 |
| plunge departure (2D-exempt) | 31.3 | 31.3 | 32.1 | 135.6 |
| starter | 1.3 | 1.3 | 1.3 | 22.4 |
| body: wraps / turns / mop / agent passes | **0.0** | **0.0** | **0.0** | **1111.1** |
| keep-down link traverse, draped body | 0.0 | 0.0 | 0.0 | 0.0 |
| total | 1375.8 (4.14 %) | 1375.8 | 1372.7 | 2779.8 (20.2 % of 13768.6) |

The `zigzag_pick` and D17 `zigzag` numbers are identical, because the two
`program.nc` files are byte-identical (D18). One-way differs only in the
waterline (-3.9 mm) and the departures (+0.8 mm). That explains the
"within 3 mm".

**The geodesic body has no linear or arc sample over 0.471.** The
producer that every geodesic arm shares is the shipped slice:
- the perimeter sweep: 74.6 % of the over length;
- the waterline cleanup: 23.0 %;
- the plunge departures, which the 2D rule exempts: 2.3 %.

The perimeter sweep, the waterline cleanup and the departures carry
1646 mm (59 %) of the AgentSearch over length. The other 1111 mm (40 %) is the agent passes themselves.

### C30.3 Per level, `geodesic_local_zigzag_pick` (mm over 0.471)

| Level | Perimeter sweep | Waterline cleanup | Departure | Starter | Body |
|---|---|---|---|---|---|
| Z 17 | 123.6 | 11.4 | 0 | 0.5 | 0 |
| Z 11 | 307.5 | 240.7 | 0 | 0.3 | 0 |
| Z 5 | 307.5 | 56.0 | 0 | 0.3 | 0 |
| Z 0.5 | 287.5 | 9.0 | 31.3 | 0.3 | 0 |

AgentSearch, the same columns:
- Z 17: 154.2 / 8.0 / 51.8 / 5.2 / 365.4
- Z 11: 336.4 / 274.9 / 14.5 / 5.7 / 272.0
- Z 5: 336.4 / 58.2 / 14.5 / 5.7 / 267.2
- Z 0.5: 319.6 / 23.1 / 54.8 / 5.7 / 206.4

Notes on the three shipped producers:

- **The perimeter sweep** is a full slot by design. It is a loop through
  fresh stock at the full level depth (6 mm at Corne Depth/Pass 6). Its
  exact width is 0.952-1.000, median 1.000. Another 267 mm of it (all
  levels) reads over 0.471 on `helix` kinematics samples (draped, Z
  changing), which the page's measure leaves out.
- **The waterline cleanup**:
  - Most of it is at Z 11, on the first contour (start (20.3, -106.2)):
    169.2 of its 317.3 mm is over, exact > 0.513 on 169.0 mm.
  - At Z 11, Z 5 and Z 0.5, region 1 is skipped by F-038 (C22 §10d). The
    waterline traces every mesh contour at the Z (`path.rs`, the F9
    note), so it can cut a contour whose band no clearing pass planned.
    This is a hypothesis for the Z 11 share; C30 did not map the contour
    to the region.
  - Its exact median is 1.000. Another 276 mm is over on `helix`
    samples.
- **The plunge departures at Z 0.5**: 0.3 mm single samples just after
  the keep-down plunges into the hex holes (exact 0.54-1.00). C18 rule C
  exempts them.

At Corne's own bound (exact 0.3272), the geodesic body reads 2.3 mm (Z 17)
and the starter 13.8 mm (exact 0.435-0.457). The AgentSearch body reads
3620 mm. The perimeter and waterline numbers barely change, because they
are slots.

### C30.4 Which share is a sim-cell artefact

| Run | Sim > 0.471 | Exact also > 0.471 | Sim-only (artefact) | Sim > screen 0.5126 |
|---|---|---|---|---|
| zigzag_pick / zigzag | 1375.8 | 1368.6 | **7.1 mm (0.5 %)**: waterline 5.8, starter 1.3 | 1282.3 |
| oneway | 1372.7 | 1365.5 | 7.1 mm (0.5 %) | 1279.0 |
| AgentSearch | 2779.8 | 2722.3 | **57.5 mm (2.1 %)**: body 38.7, waterline 11.0, departure 7.0, perimeter 0.9 | 2489.6 |

- **The cell-aware screen is not an artefact filter here.** The screen
  drops 93.5 mm of `zigzag_pick` (the band 0.471 < sim ≤ 0.5126). The
  exact oracle reads 92.4 mm of that band over 0.5126 too. The simulation
  under-reads those samples (blended partial dexels), so they are real.
  For AgentSearch the band is 290.2 mm, and the exact reads 201.3 mm of it
  over 0.5126.
- **Over the screen, the sim almost never over-reads.** Of the 1282.3 mm
  over 0.5126, the exact reads only 6.1 mm at 0.471 or less (AgentSearch:
  37.4 of 2489.6).
- So on Corne the artefact share of the 0.471 over length is 0.5 % for
  the geodesic arms and 2.1 % for AgentSearch. The rest is real swept
  width on the exact geometry. Of the geodesic arms' real share, all is
  the shipped perimeter sweep, the waterline cleanup, and the exempt
  departures.

### C30.5 Why the C12 heavy test reads 0 and Corne does not

The C18 oracle exempts the perimeter sweep (by band) and the waterline
cleanup (by tag). It also exempts the plunge departures (rule C). On
Corne those three are 1374.5 of the 1375.8 mm. The 1.3 mm left is the
starter, and the exact oracle reads it at 0.435-0.457, under 0.471. So the
C12 oracle and Corne agree: the geodesic clearing holds 0.471. The
remaining load is in the shipped slice (G-PERIMTAG, G-WATERLOAD).

### C30.6 Limits

- The exact oracle is a 2D column reading at the sample's Z. It counts a
  thin layer left between two drape heights as full-width stock.
- The `helix`-kinematics samples (draped, Z changing) are left out of the
  length, as in the page measure. They add 543 mm over 0.471, all in
  the perimeter sweep (267.0) and the waterline (275.8).
- The body cannot be split into wraps, turns and mop from the trace.
  This matters only for AgentSearch, because the geodesic body has 0 mm
  over 0.471.

## C32. wanaka200 "2 Back Rough": the draped body cuts over 0.471 (2026-10-25)

Measurement only. No code change, no cargo. Data:
- geodesic zigzag: `phase3/fix2/simart/wanaka200/geodesic_local_zigzag/`
  (31935b89);
- contour_parallel: `phase3/fix6/wanaka200/contour_parallel/` (f32401326).

Both runs use a 1.0 mm sim cell. Scripts and output are in `phase3/c32/`
(`run_c32.sh` runs both; `c32_output.txt`; about 1 min).

### C32.1 The method

- **The samples:** the D-FIX6 rule. Cutting samples of toolpath 1 with
  `source_intent == clearing_cut`, any kinematics, in the body phase
  (`fix2/producer_split.py::phases`), read over 0.471 by the simulation.
  It reproduces the lead's numbers exactly: zigzag 1871.1 mm,
  contour_parallel 6201.7 mm.
- **The exact width (strict):** C18's `exact_width_at`. The path is the
  sample path, densified to 0.1 mm, because the 1.0 mm sample step would
  leave 0.04 mm cusps beside the path. A disc of R is stamped at every
  earlier point with Z ≤ the sample's Z + 0.01. The lattice pitch is
  0.05 mm.
- **The exact width (same level):** as strict, but every earlier point of
  the same level counts as cut at any Z.
- **The mechanism of an over sample:**
  - **0, sim-only:** strict ≤ 0.471.
  - **A, step:** strict > 0.471 and same-level ≤ 0.471. The uncut stock in
    the disc is only stock that an earlier pass of the same level left,
    because that pass ran higher (draped).
  - **B, fresh:** same-level > 0.471. No earlier pass of the level cut
    that stock at any height.
- **The layer thickness of A:** for each lattice point that only a higher
  same-level pass covers (every 7th point), the lowest such pass Z within
  R, minus the sample Z. The script reports the median per sample.
- **Slope:** the path slope |dz| / XY step of the sample. **Drape:** the
  sample Z above its level Z.

### C32.2 Is it real? Mostly yes; the sim artefact is small

| | geodesic zigzag | contour_parallel |
|---|---|---|
| body over 0.471 (sim) | 1871.1 mm (helix 1839.8, linear+arc 31.4) | 6201.7 mm (helix 4619.4, linear+arc 1582.2) |
| 0, sim-only (exact ≤ 0.471) | **41.4 mm (2.2 %)** | **186.9 mm (3.0 %)** |
| A, step left by a higher same-level pass | 1828.2 mm | 5362.5 mm |
| B, fresh stock | 1.5 mm | 652.2 mm |
| sim over the screen 0.6293 | 1292.5 mm, of which exact > 0.471: 1281.5 | 5190.0 mm, of which exact > 0.471: 5082.2 |

- Even at the 1.0 mm cell, the simulation over-reads only 2-3 % of the
  over length. The exact geometry confirms the rest.
- The cell-aware screen (0.629) is not an artefact filter. On zigzag it
  removes 578.6 mm, and the exact reads 548.2 mm of that over 0.471.
- Medians, A: zigzag sim 0.687, exact 0.997, same-level 0.337;
  contour_parallel sim 0.667, exact 1.000, same-level 0.333. With the
  higher passes counted, the width falls to s/D = 0.333, the commanded
  stepover. So the 2D plan holds its XY spacing. The over width is the
  height step between neighbouring passes.

### C32.3 The mechanism: lead's candidate (a), a thin step on a slope

- **Candidate (a): a wrap draped down a slope meets the stock above the
  previous pass's floor.** This is mechanism A: 97.7 % of the zigzag
  over length and 86.5 % of contour_parallel.
  - Each 2D pass is lifted point by point to its floor (the surface plus
    the leave). Two neighbours about one stepover apart on a slope stand
    at different Z.
  - The lower pass then meets, across its whole disc, the layer between
    its own floor and the higher neighbour's floor. The swept-width
    reading (2D rule and simulation alike) counts that layer at full
    width, whatever its thickness.
- **Candidate (b): the cutter side meets terrain.** It is ruled out by
  construction. The lifted Z is the drop-cutter height plus the leave, so
  no terrain stands above the cutter floor under its disc. The exact
  reading counts only material above the sample Z, which is stock.
- **Mechanism B (fresh):** it is not a draping effect. For
  contour_parallel it is 182-188 mm per upper level, about the length of
  a first loop through fresh stock: a slot (exact 1.000, same-level
  1.000). For zigzag it is 1.5 mm.

**The step is thin.** Layer thickness of A, as mm of cut per bin:

| Layer | zigzag | contour_parallel |
|---|---|---|
| ≤ 0.25 mm | 1551.0 | 3017.2 |
| 0.25-0.5 mm | 153.1 | 423.8 |
| 0.5-1 mm | 82.5 | 154.3 |
| 1-2 mm | 23.5 | 178.6 |
| > 2 mm | 16.4 | 1585.3 |

- Zigzag: 85 % of the step length is a layer of 0.25 mm or less. It is a
  full-width skim at very small axial depth.
- contour_parallel: 1585 mm is a step over 2 mm thick. Its flat passes
  at the level run beside neighbours that draped more than 2 mm higher
  ("at level": 1194.7 mm in class A).
- The same exact reading with only stock of at least one planner cell
  (0.5 mm) thickness counted: zigzag 263.8 mm (Z 8.6: 255.1; Z 4.0: 8.7),
  contour_parallel 2822.0 mm.

### C32.4 By level, slope and drape (mm over 0.471, sim; 0 / A / B)

| | zigzag | contour_parallel |
|---|---|---|
| Z 19.5 | 0 / 0 / 0 | 0 / 560.0 / 187.8 |
| Z 14.1 | 0 / 0 / 0 | 0 / 566.0 / 181.8 |
| Z 8.6 | 41.4 / 1821.0 / 0 | 186.9 / 4190.0 / 163.5 |
| Z 4.0 | 0 / 7.2 / 1.5 | 0 / 46.4 / 119.2 |
| path slope flat | 0 / 29.0 / 0 | 2.3 / 1184.5 / 386.0 |
| slope ≤ 0.1 | 40.3 / 1616.9 / 1.0 | 161.3 / 3156.7 / 211.1 |
| slope 0.1-0.3 | 0.8 / 146.6 / 0 | 8.7 / 716.7 / 43.0 |
| slope 0.3-1 | 0 / 12.9 / 0.6 | 11.7 / 261.1 / 9.7 |
| slope > 1 | 0.3 / 22.8 / 0 | 2.8 / 43.5 / 2.4 |
| drape: at the level | 0 / 52.3 / 0 | 1.9 / 1194.7 / 369.5 |
| drape ≤ 1 mm | 10.1 / 436.2 / 0 | 36.6 / 1052.4 / 1.8 |
| drape 1-3 mm | 24.0 / 792.5 / 0 | 130.6 / 2289.8 / 62.9 |
| drape > 3 mm | 7.2 / 547.2 / 1.5 | 17.9 / 825.6 / 218.0 |

- The zigzag over length is almost all at Z 8.6, the level that drapes
  over the terrain.
- Most of it is on gentle paths: slope ≤ 0.1 along the path, 1617 mm. A
  path along the contour can be nearly level, while the terrain falls
  across it, between the passes.

### C32.5 What it means for the bound

- The 2D rule (and the oracle) read a radial width at the sample's own
  floor. In 3D a pass also meets the layer that a higher neighbour left
  above that floor, and the reading counts that layer at full width.
  Most such layers on wanaka200 are less than 0.25 mm thick (zigzag).
- So most of the geodesic zigzag's draped over length is real contact,
  but a full-width skim of small depth, not an overload of the 2D kind.
- A load bound that is fair on terrain needs the axial depth of the
  stock it counts: for example, the exact width over stock thicker than
  one planner cell (zigzag 263.8 mm; contour_parallel 2822.0 mm). That is
  a proposal for the lead, not a rule.
- contour_parallel carries real thick steps too: 1585 mm over 2 mm, and
  652 mm of fresh slots.

### C32.6 Limits

- The layer thickness reads every 7th lattice point. It is the lowest
  higher-pass Z within R, minus the sample Z: the top of the layer that
  pass left, not a full 3D stock model.
- The slope is along the path, not the terrain gradient across the
  passes.
- Corne's body rise of 10.6 mm under the D9 rule was not analysed.

## C33. Corne after C31: which body moves carry the thick over-load (2026-10-26)

Read-only analysis. No production code, no cargo. Data:
- D20 `phase3/fix7/corne/geodesic_local_zigzag_pick/` (b37cad1a4, no
  perimeter sweep in the research arm);
- D18 `phase3/fix5/corne/geodesic_local_zigzag_pick/`, for question 3.

Scripts and output are in `phase3/c33/` (`run_c33.sh`, `c33_output.txt`,
about 30 s):
- `partmap.py` rasterises the part top from the project's STL, on a 0.2 mm
  grid with no transform. The STL bbox x 5.31-148.07 sits inside the
  stock bbox x 0.31-153.07 with a 5 mm margin.
- `c33.py` reads each body sample.
- `report33.py` makes the tables.

### C33.1 The method

- **The body:** from the level's first move to its waterline annotation.
  After C31 there is no sweep, so the "perimeter" of `producer_split`
  (1.3 mm) is the starter and is counted here as body.
- **The samples:** clearing_cut samples the simulation reads over 0.471:
  425.0 mm.
- **The readings,** on the densified sample path, with C32's lattice:
  - **strict:** stock above the tip;
  - **thick:** stock at least 0.5 mm (one planner cell) above the tip, as
    `thick_measure.py`. It reproduces the lead's **268.0 mm**;
  - **same-level:** every earlier pass of the level counts as cut, at any
    height (C32).
- **Wall distance:** from the cutter centre to the nearest part cell whose
  top stands above the tip. The tip Z is binned down to 0.5 mm. Island:
  that cell's connected part component is not the largest.
- **D18 cover:** the share of the thick-uncut lattice points in the disc
  that D18's sweep of the same level covered (a disc of R round each
  sweep point with Z ≤ tip + 0.5).
- **Move class** comes from the trace:
  - the run is from the last entry to the next;
  - "run end" means the next non-cut move is a `linking` lift within 2
    moves.

### C33.2 Answers

**4. Sim artefact: none in the 268 mm.**
- Of the 425.0 mm body over 0.471, the strict exact reads 422.5 mm over.
  Sim-only: 2.5 mm (0.6 %), C30's class.
- Every thick-over sample is also strict-over, so the 268.0 mm holds no
  artefact.

**2. Where: every thick-over sample is at the outer part's wall.**
- Length-weighted wall distance: p10 3.23, p50 3.65, p90 4.25, max
  6.05 mm.
- At a wall (≤ R + leave + 0.5 = 4.0 mm): 209.1 mm. Wall band (4.0-6.5):
  57.6 mm. Near an island: 1.3 mm. In the open (> 6.5 mm): 0.
- The median 3.65 mm is the grown outline's own centre locus: the region
  boundary at about R + leave, plus the margin m = 0.25. So **no meeting
  line or open-field wrap carries thick load**.

**1. Which moves.** By cause (mm thick over 0.471):

| Cause | Z 17 | Z 11 | Z 5 | Z 0.5 | Sum | At a wall | D18 sweep covered ≥ half | Draped (helix) | Wall dist. median |
|---|---|---|---|---|---|---|---|---|---|
| Outline wrap on the case-top shoulder, draped below a higher same-level pass | 101.2 | 0 | 0 | 6.9 | **108.1** | 106.2 | 77.1 | 77.2 | 3.37 |
| Outline wrap at the level, below a higher same-level pass | 72.8 | 2.4 | 2.4 | 0 | **77.5** | 50.8 | 41.6 | 22.6 | 3.86 |
| Fresh: first cut of that stock at the level (wrap ends at the outline) | 13.2 | 11.8 | 11.8 | 11.7 | **48.5** | 18.2 | 5.9 | 0 | 4.25 |
| Hex-hole dip: a short run down to Z 0.5 between links at Z 4 | 0 | 0 | 0 | 33.9 | **33.9** | 33.9 | 0.7 | 0.7 | 3.35 |
| Sum | 187.2 | 14.2 | 14.2 | 52.5 | **268.0** | 209.1 | 125.3 | 100.5 | 3.65 |

The brief's classes, measured (mm; Z 17 / 11 / 5 / 0.5):

| Brief class | mm | What the trace shows |
|---|---|---|
| First wrap against the grown outline | 185.6 (step rows above) | Z 17: the outermost wrap rides the case-top shoulder. The wraps inside it ran over the case top, higher. The same-level reading falls to ≤ 0.471 (C32's step). The layer from the tip up to the lowest higher pass within R is 0.59 mm median (p10 0.51, p90 1.00), at tip Z 17.11 median. |
| Arc ends / hooks (short last move, then a lift) | 84.8 (44.6 / 0.5 / 0.5 / 39.2) | These are the same rows. At Z 17 the lift splits the shoulder wrap into short runs (pieces of 3-5 moves between keep-down links). At Z 0.5 they are the hole dips. The trace does not show C31 §11.12's sideways hop. G-ARCENDHOOK is on the curv modes; these are G1 zigzag moves. |
| Turns / first moves after a keep-down link | 46.9 (33.0 / 5.8 / 1.0 / 7.0) | Mostly the start of the same shoulder runs at Z 17. At Z 11, one 0.3 mm sample per wrap where the wrap meets the outline at y -68.26 (fresh, thick 0.70-0.77). |
| Keep-down links | 0 | Their samples are `linking`; the D-FIX6 rule leaves them out. |
| Lobe starts after a plunge | 4.6 (4.3 / 0.3 / 0 / 0) | |
| Meeting line | 0 | No thick-over sample is more than 6.05 mm from a wall. |
| Inside a run | 28.4 (2.0 / 7.5 / 12.6 / 6.2) | |

**3. Did D18's sweep cut that stock?** Measured as the share of each
sample's thick-uncut stock that D18's sweep covered:
- ≥ 0.9: 94.0 mm (Z 17: 93.3);
- 0.5-0.9: 31.3 mm;
- < 0.5: 142.7 mm.

So the sweep took most of the Z 17 shoulder stock (77 of 108 mm covered
by half or more). It did not take the fresh wrap ends at Z 11, 5 and 0.5
(5.9 of 48.5 mm), nor the hex-hole dips (0.7 of 33.9 mm). In D18 the dips
were the "plunge departure" rows (31.3 mm at Z 0.5, C30), exempt then.
After C31 the descent into a hole is a keep-down link, not a vertical
plunge, so the departure exemption no longer applies to them.

### C33.3 Reading

- **There is one dominant place:** the outermost wrap of the grown
  outline, at the outer wall. 185.6 of the 268.0 mm are that wrap
  meeting a step that the inner wraps left above it. Most of it is at
  Z 17 (174.0 mm), where the outline runs on the case-top shoulder. There
  the inner wraps ride over the case top about 0.5-1.0 mm higher than the
  outline wrap.
- The step is just over the thick threshold: median 0.59 mm against
  0.5. A threshold of one planner cell counts it, and a 1.0 mm threshold
  would count about the p90 tail only.
- The other classes:
  - wrap ends at the outline (fresh): 48.5 mm, 12 mm per level;
  - the hex-hole dips: 33.9 mm at Z 0.5. They are not new; D18 exempted
    them as plunge departures.
- **Why the C12 heavy test does not see it:** the C12 fixture has
  vertical walls and a flat floor. No wrap of a level runs higher than
  its neighbour there, and no descent into a hole exists. Corne's Z 17
  level cuts 1 mm below the case top, so the outline sits on a slope
  that the drape follows.
- **No fix proposal.** The cause is one place, but two mechanisms: a
  draping step on the shoulder, and fresh wrap ends.
