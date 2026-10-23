# P3 C12 — the geodesic kernel as a 3D clearing arm (research only)

Agent C12, 2026-10-24. Worktree `.claude/worktrees/adaptive-head`, branch
`adaptive-instruments`, base HEAD `ea468814`. Not committed. No cargo command
ran; only `rustfmt --edition 2024 --check` ran (clean on every changed file).

**Shared tree.** A second agent ("C14") edits the same worktree at the same
time. `crates/rs_cam_core/src/adaptive/path.rs` holds about 168 changed
lines that are not C12's (`turn_bound`, `LinkLoad::turns`, `admits_turn`).
C12 did not touch that file. Review each diff by path.

## 1. The route (file:line at ea468814)

The ContourSpiral path, end to end:

1. Config: `compute/operation_configs.rs:148` `ClearingStrategy::ContourSpiral`
   (serde, GUI combo `rs_cam_viz/src/ui/properties/operations/surface_3d.rs:184`,
   MCP schema, `machine/strategy_advisor.rs:110`, `compute/catalog.rs:1025`).
2. Config to planner params: `compute/execute/rough_3d.rs:153`
   `clearing_strategy: cfg.clearing_strategy.into()`, through
   `adaptive3d/mod.rs:104-112` (`From<ClearingStrategy> for ClearingStrategy3d`).
3. The planner enum: `adaptive3d/mod.rs:72` `ClearingStrategy3d`; it reaches
   the level context at `adaptive3d/path.rs:944`.
4. The level dispatch: `adaptive3d/path.rs:469`
   `AgentSearch | ContourSpiral => clear_z_level_agent_2d_slice(..)`.
5. The slice: `adaptive3d/clearing.rs:2922` `clear_z_level_agent_2d_slice`
   builds `params_2d`; `:3006` picks `PathStrategy2d::ContourSpiral` as the
   2D generator; `:3021` sets `KeepDownLinks::RetractedByCaller`.
6. Per region: `clearing.rs:2412` `clear_one_region`: the F-038 forecast, the
   perimeter sweep (`:2561` `PERIMETER_INSET_MARGIN_MM = 0.25`), then
   `:2628` `for outline in planner_outlines(..)` (`:2198`, the D2 piece
   split).
7. Per outline: `clearing.rs:2270` `plan_one_outline`: the 2D producer
   `:2285` `adaptive_segments_with_debug`, the cleanup `:2311`
   `apply_contour_parallel_residue_cleanup` (which is
   `adaptive/path.rs:1798`, the `_with` form at `PassLoadMeasure::SweptWidth`:
   the same replay, residue contours, wall pass and mop as the 2D harness's
   `cleaned`, `research_geodesic.rs:6070`), then the F-038 group filter.
8. Emission: back in `clear_one_region`, each 2D segment is lifted
   (`level_floor_z`) and goes through `LevelSink::emit` (plan entry, centre
   clip, stamp). A 2D `Link` becomes a 3D entry (`clearing.rs:2856`).

The 2D harness runs candidate (a) as `geodesic_segments`
(`research_geodesic.rs:3758`) then `cleaned`, with
`path_strategy = Agent` and `keep_down_links = WithinPassLoad`
(`run_2d_with`, `:6128`).

## 2. The gate and the blast radius

`rg` over `crates/` for `ClearingStrategy3d`:

- One exhaustive match in product code: `adaptive3d/path.rs:443-482`.
- One `matches!`: `clearing.rs:3006` (no change needed).
- The config `From` impl: `adaptive3d/mod.rs:104`.
- Tests name variants in arrays and literals; none matches exhaustively.
- The enum has no serde, no `Hash`, and no GUI or MCP form. Those surfaces
  read the config enum `compute::operation_configs::ClearingStrategy`.

**Choice: a `#[cfg(feature = "research")]` variant
`ClearingStrategy3d::Geodesic(GeodesicOptions)` on the planner enum, and no
change to the config enum.** Why:

- The planner enum has three sites; the config enum has serde, a GUI combo,
  MCP schema, the advisor and the catalog.
- A config variant behind the core feature breaks the viz build under the
  workspace clippy line (`--features rs_cam_core/research`): viz matches the
  config enum exhaustively and has no `research` feature to gate an arm.
- `GeodesicOptions` is `Copy + Eq` and a few bytes, so `Copy`, `Eq`,
  `Default` and `large_enum_variant` stay as they are.

**Selection without a public dial.** A project file cannot name the arm.
In a research build only, the environment variable
`RS_CAM_RESEARCH_ADAPTIVE3D_ARM` (`adaptive3d::RESEARCH_ARM_ENV`) replaces
the `AgentSearch` arm of every config that names `agent_search`:

| value | arm | 2D column |
|---|---|---|
| `geodesic_curv` | `Geodesic(GeodesicOptions::C5_CURV)` | `geodesic_curv` (far side: default) |
| `geodesic_curv_split` | `Geodesic(GeodesicOptions::C11_SPLIT)` | `geodesic_curv_split` (far side: split at the meeting line) |

The variable is read once per process (`OnceLock`, the precedent is
`RS_CAM_STAMP_DISPATCH`, `dexel_stock/whole_path.rs:131`). An unknown value
logs a warning and leaves AgentSearch. The override replaces only
`agent_search`, so a project file picks the ops per toolpath, and the A/B
control is the same file without the variable (AgentSearch, the same slice
dispatch, only the 2D generator and its load rule differ).

## 3. The design

- Dispatch: `path.rs` gets a cfg'd arm `Geodesic(_) =>
  clear_z_level_agent_2d_slice(..)`, the same call as the AgentSearch /
  ContourSpiral arm. No parallel flow.
- `params_2d`: `path_strategy` stays `Agent` (as the 2D harness; no spiral
  corner blend). `keep_down_links` is `WithinPassLoad` for the research arm
  (`is_research_arm`), `RetractedByCaller` for every shipped arm.
- `plan_one_outline`: under `research`, `research_slice_segments` calls
  `research_geodesic::slice_segments` (a `pub(crate)` wrapper of
  `geodesic_segments`). `None` (the field does not apply) falls back to
  the agent with the same `params_2d`. Then the shipped cleanup runs, the
  same replay (`step_within_pass_load`), residue contours, wall pass and
  mop.
- **Links as cuts.** Under `WithinPassLoad` the replay admits a link only
  within the pass load and stamps it on the 2D grid. The 3D lift turns a
  `Link` into an entry, and an entry retracts where the link cuts stock
  ("no link feeds down into stock"), so the 2D grid would hold cut what no
  emitted move cuts ("Do not stamp stock no emitted move cuts",
  `adaptive/CLAUDE.md`). So after the cleanup, and only under
  `WithinPassLoad`, `research_geodesic::links_as_cuts` writes each `Link` as
  a `Cut` from the cutter position, subdivided at the planner cell so that
  each point drapes. The 3D cut lift then handles air runs and Z jumps as it
  does for every cut.
- The far-side mode is the `GeodesicOptions` value in the variant; no new
  public dial. The trochoid insert stays out (the kernel adds none).
- No new constant. Every number in the new code is the planner cell
  `max(R/6, tolerance)` or an existing value. The test cites the shipped
  `PERIMETER_INSET_MARGIN_MM` (0.25) and derives its bands from R, the
  cell, the mesh step and the sim cell.
- What the arm does not get: the F-038 forecast (`clear_one_region`,
  `min_region_cut_length_mm > 0`) is a dry run of the agent, not of the
  field, so it can skip a region the field would cut longer. The Stage 4
  engagement sampler stays empty for the arm (as for AgentSearch). The
  perimeter sweep before each region is the shipped slice behaviour and
  stays.

## 4. Files changed

| File | Change |
|---|---|
| `crates/rs_cam_core/src/adaptive3d/mod.rs` | cfg variant `Geodesic`, `RESEARCH_ARM_ENV`, `research_arm_override`, cfg arms in the `From` impl |
| `crates/rs_cam_core/src/adaptive3d/path.rs` | cfg arm in the level dispatch |
| `crates/rs_cam_core/src/adaptive3d/clearing.rs` | `is_research_arm`, cfg `research_slice_segments`, producer switch and cfg link conversion in `plan_one_outline`, `keep_down_links` choice in `params_2d` |
| `crates/rs_cam_core/src/adaptive/research_geodesic.rs` | `slice_segments`, `links_as_cuts` (one block before `#[cfg(test)]`) |
| `crates/rs_cam_core/src/adaptive3d/CLAUDE.md` | one phrase on the mod.rs line (still 40 lines) |
| `crates/rs_cam_core/Cargo.toml` | `[[test]] adaptive3d_geodesic_arm_c12`, `required-features = ["research", "test-support"]` |
| `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs` | new research test |

Shipped neutrality: without `research`, `is_research_arm` returns false, the
producer switch is `None => adaptive_segments_with_debug(..)` with the same
arguments, and the link conversion is not compiled. The `params_2d` value
is the same expression result (`RetractedByCaller`). So the shipped output
is byte-identical by construction; `adaptive3d_emission_byte_parity` is the
proof the verifier runs.

## 5. The test

`tests/adaptive3d_geodesic_arm_c12.rs`: a height-field pocket
`|x| <= 34, |y| <= 24` (floor Z 4, rim Z 10) with a round island of radius 5
(top Z 10); 6 mm flat, s 2, tolerance 0.1, Depth/Pass 3, leave 0.5: levels
Z 7 and Z 4.5. Oracle: the dexel simulation of the emitted path, bound =
ceiling 0.3626 + (0.5 + 0.5 + 0.1)/6 = 0.546 (the `common::adaptive_islands`
bound). Not judged: entry moves, the samples that leave a straight plunge
within R, and the perimeter-sweep band (wall distance in
`[2R + 0.25 − 1.5·cell, 2R + 0.25 + √2·step + 1.5·cell]` = [5.5, 7.46] mm).
The band also hides the 2D wall pass, so the test says nothing of the wall
pass. Assertions per mode (`geodesic_curv`, `geodesic_curv_split`): judged
cutting samples at two Z levels; no judged sample over the bound (each over
sample is printed first). Across modes: the two toolpaths differ (proof that
the field planned at least one piece). AgentSearch is printed as a control.

## 6. Predictions (not measured; marked P)

- P1: `cargo check` passes with and without `research`; clippy is clean in
  both. Risk: the cfg on the tail blocks of `is_research_arm` and the cfg'd
  `let` shadow in `plan_one_outline`. Both are stable Rust forms.
- P2: `adaptive3d_emission_byte_parity` and the 14 adaptive3d sentries stay
  green without the feature (no shipped expression changed value).
- P3: the 2D harness and `--lib adaptive::research_geodesic` are unchanged
  by C12 (the two new functions are additive; C14's `path.rs` edits are a
  separate question).
- P4: the new test generates at two levels and the modes differ: likely.
- P5: the load assertion is the uncertain one. Known 3D sources of over
  samples that the 2D plan does not see:
  - the 3D RDP (`Adaptive3dGeometry::cut_simplify_tolerance`, here 0.1 mm)
    simplifies each cut before the stamp, while the 2D grid stamped the
    walked line. The 2D emitter under `WithinPassLoad` keeps the walked line
    for that reason (`adaptive/path.rs:3170` comment: 0.65 of D on the
    six-island pocket). Expect a few over samples there;
  - a mop step (the brief's known exception).
  If the test fails, the printed lines name the move, the intent, the
  position and the wall distance. The verifier then classifies them; a
  mop step or an RDP sliver is a measured exemption to pin, not a fix.
- P6: on the fixtures, the arm cuts fewer entries than AgentSearch per level
  only where the 2D run did; the perimeter sweep and the agent fallback are
  shared costs.

## 7. Verifier commands

Run from the worktree. All through the lane script, each with a timeout.

```sh
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head
timeout 120 scripts/cargo_lane.sh fmt --all -- --check
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
# the new research test
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_geodesic_arm_c12 -- --nocapture
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
# 2D lib tests and the research kernel tests
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --lib adaptive
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --lib adaptive::research_geodesic
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --lib adaptive::research_geodesic -- --ignored
# 2D controls unchanged: the 2D render harness (ignored instruments)
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
# both clippy configs
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
```

The `for` loop is written for bash or zsh (the memory note: zsh does not
split a word list, so the list is literal words). The kernel has two
`#[ignore]` tests, so its lib line runs twice (plain, then `-- --ignored`;
`tests/CLAUDE.md` forbids `--include-ignored`). Compare the 2D harness summaries with the C11 run
(`P2_C2_geodesic_fixes.md` §C11) column by column.

## 8. The fixture runs (no project file is edited)

The three projects name `contour_parallel` for their rough. To run the arm,
a session writes a NEW project file in the SAME folder as the original (the
model paths are relative), with `clearing_strategy = "agent_search"` on the
rough op and nothing else changed:

| Original | New file (arm and its AgentSearch control) | Rough op |
|---|---|---|
| `planning/fixtures/rivmap100/rivmap100.toml` | `planning/fixtures/rivmap100/rivmap100_c12_agent_search.toml` | "3D Rough" |
| `planning/corne_case_analysis_2026-09-18/corne_job_corrected.toml` | `planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml` | "3D Rough 1" and "3D Rough 1 (copy)" (both adaptive3d; set both, or skip one with `--skip`) |
| `planning/multitool_2026-08-23/wanaka200_mt1.toml` | `planning/multitool_2026-08-23/wanaka200_mt1_c12_agent_search.toml` | "2 Back Rough" |

The benchmark rule says do not measure on a copy in a scratchpad; these
copies sit next to the originals in the repo. Then, per file, three runs
with the CLI built with the core research feature:

```sh
B="scripts/cargo_lane.sh run --release -p rs_cam_cli --features rs_cam_core/research --"
# control: AgentSearch
timeout 3600 $B project planning/fixtures/rivmap100/rivmap100_c12_agent_search.toml --output-dir <out>/agent --summary
# the arm, default far side
RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_curv timeout 3600 $B project planning/fixtures/rivmap100/rivmap100_c12_agent_search.toml --output-dir <out>/geodesic_curv --summary
# the arm, split at the meeting line
RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_curv_split timeout 3600 $B project planning/fixtures/rivmap100/rivmap100_c12_agent_search.toml --output-dir <out>/geodesic_curv_split --summary
```

`<out>` goes under the planning folder of this research, not `/tmp`. The
variable must be in the environment of the process that plans (the CLI
here; a GUI must be started with it and built with the feature, and then
the HEAD check of the root `CLAUDE.md` applies). The shipped baseline is the
original file (`contour_parallel`), unchanged. The rivmap100 rough sets
`min_region_cut_length_mm = 15.0`, so the F-038 forecast (an agent dry run,
§3) is active there.

## 9. What C12 could not do

- No build, test or clippy run (by the brief). Every outcome in §6 is a
  prediction.
- The test cannot judge the wall pass: the perimeter sweep of the shipped
  slice is untagged in the emitted path, so the test leaves out a band that
  also holds the wall pass. A tag on the perimeter sweep (a runtime marker)
  would let a later test judge the band; that is a shipped change and C12
  did not make it.
- The 3D RDP before the stamp is the ONE rule (`chord_tolerance`); C12 did
  not change it for the arm. If P5's slivers appear, the choice (an arm
  rule that keeps the walked line, as 2D does) is the lead's.
