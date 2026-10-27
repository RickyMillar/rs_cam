# Integration step 1: the feed cap contract (`Move::feed_cap`)

Date: 2026-10-27. Worktree `geodesic-integration`, base master `ab208f83a`.
Source of the design: research branch `adaptive-instruments` at `bd24814c7`
(INTEGRATION_PLAN §4 step 1, §3 D6). The implementer did not commit.

Frozen patch: `step1_tree_vs_ab208f83a.patch`, md5
`40ea6ab029d53d4fe5546bb3afd5e5f9` (23 crate files, 216 insertions, 13
deletions, plus the new sentry file). `git apply --check -R` passes on the tree.

## 1. What changed

The toolpath IR gets `Move::feed_cap: Option<f64>`. It is the highest feed
(mm/min) that a move may carry. No planner on master writes it, so every
move carries `None`.

Feed stages (the binding):

| File | Change |
|---|---|
| `toolpath.rs` | The field, its doc, `None` in the four builders. A hand-written `Debug` (see §4). The unit test `a_move_with_no_cap_prints_as_before_the_field`. |
| `dressup/feed_modulation.rs` | After every arm, the floor and the plunge guard: `if let Some(cap)` and `new_feed > cap`, then `new_feed = cap` and the binding is `FeedCap`. |
| `dressup/feedopt.rs` | After the smoothing: `feed.min(cap)` for a move with a cap. |
| `dressup/arcfit.rs` | The cap is an equality term of the run key. The arc takes `moves[i].feed_cap`. |
| `dressup/condition.rs` | The cap is an equality term of the segment-merge run key. New unit test `does_not_merge_across_feed_cap_change`. |
| `tool_load/verdict.rs` | New `BindingConstraint::FeedCap`, label `feed-cap`. |
| `session/compute.rs` | A dominant `FeedCap` is `LoadRegime::ToolLimited` (as on the research branch). |
| `dressup/CLAUDE.md` | One invariant clause and the new sentry in the sentry list (still 40 lines). |
| `tests/chipload_boundary_g_chip_ulp.rs` | The exhaustive variant list gets `FeedCap`. |
| `tests/a_feed_cap_binds_every_feed_stage_c38.rs` | The ported sentry. It is byte-identical to the research file (`cmp`): master's APIs needed no change. |

## 2. Each Move construction site

The compiler finds every struct literal, because `Move` has no `..` default.
`rg "Move \{"` over `crates/` gives the same list. A copy by `clone()` or
`*m` carries the cap with no edit; examples are `geometry/boundary.rs`
`clipped_linear_move` and the verbatim pushes in `condition.rs` and
`arcfit.rs`.

Sites that rebuild a move from an input move. These carry the cap:

| Site | Carries |
|---|---|
| `compute/transform.rs` `transform_toolpath` | `m.feed_cap` |
| `compute/simulate.rs` `group_toolpath_to_global` | `m.feed_cap` |
| `compute/simulate.rs` `hash_toolpath` | destructures `feed_cap: _`; not hashed (the feed the move carries is hashed) |
| `finish/surface_link.rs` `rotate_closed_loop` | body: `mv.feed_cap`; close: the input's closing move's cap |
| `dressup/arcfit.rs` (two arc pushes) | `moves[i].feed_cap`; equal over the run by the run key |
| `dressup/mod.rs` `apply_dogbones` overcut pair | the corner move's cap (builder then a set, as on the research branch) |

Sites that make a new move with no source move. These get `None`:

| Site | Move |
|---|---|
| `toolpath.rs` four builders | every builder move |
| `dressup/mod.rs` `optimize_entry_descents_with_provenance` (3) | split rapid, entry plunge, ramp lap |
| `compute/obstacle.rs` `start_at_the_plane` | the lift rapid |
| test literals | `obstacle.rs`, `measurement.rs`, `feedopt.rs`, `tsp.rs`, 3 core test files, 3 viz src test modules, 1 viz test |

The viz render and upload code (`render/toolpath_render.rs`,
`app/gpu_upload.rs`) reads moves and builds no move outside its tests. So
the viz change is only the test literals.

Builder calls (`feed_to_with_intent` and the others) also add moves with
`None`. On master these sites make entries, links and retracts, not copies
of a cut. The one copy of a cut through a builder is the dogbone overcut,
and it now carries the cap. See §6 for the open point.

## 3. Gates

All cargo commands ran through the worktree's own `scripts/cargo_lane.sh`.
(The main tree's copy changes to the main tree; see §6.)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | ok, 0 warnings |
| `a_feed_cap_binds_every_feed_stage_c38` | 4/4 |
| `adaptive3d_emission_byte_parity` | 5/5 |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1 (1 ignored instrument) |
| `constrained_max_modulation_f039` | 6/6 |
| dressup sentries (all four lines of `dressup/CLAUDE.md`) | all ok except `tabs_sit_on_straight_sides_p5` 9/10 (pre-existing, §5) |
| finish / compute / tool_load sentries: `finish_resolution_policy_pr3`, `classification_strategy_m3`, `a_scallop_ring_connector_rides_the_surface_g_tierburial`, `scallop_iso_field_config`, `the_research_arms_are_feature_gated_fin01`, `a_link_between_regions_clears_standing_stock_b16`, `a_lifted_link_lands_as_an_entry_b20`, `a_scallop_iso_field_link_clears_standing_stock_d1`, `retract_intent_move_type_census_w6`, `sim_prefix_memo_s5`, `gate_population_vacuity_xvac`, `predicted_feed_gates_f035`, `histogram_population_is_the_gate_population_g_cuthist`, `chipload_boundary_g_chip_ulp`, `exporter_span_classifier_x1` | ok |
| `Debug`-pinned binaries: `transform_provenance_fingerprints`, `fresh_generation_ignores_simulation_b19`, `frozen_snapshot_regeneration_s4`, `job_three_steps_equal_generate_toolpath`, `layered_framework_owns_the_ladder_w3`, `the_volumetric_wrapper_keeps_its_produced_w6_5`, `pushcutter_band_query_g1`, `perf_golden_depth_level_geometry`, `nn_order_scaling_g5_g6` | ok |
| `crease_own_region_pr6b` | 2/3, pre-existing G-FEEDSFIXTURE (§5) |
| core `--lib` for `dressup::`, `finish::surface_link`, `compute::{transform,simulate,obstacle}`, `measurement`, `tool_load::`, `session::compute`, `toolpath::` | 445/445 |
| `conformal_spiral_synthetic_f2` (research) `classify_move_maps_every_intent` | 1/1 |
| `rs_cam_viz -j 2`: `--test an_arc_move_draws_on_its_arc_r_arc`; `--lib` filtered to `render::toolpath_render`, `app::gpu_upload`, `controller::results_parity_tests` | 2/2, 17/17 |

Not run: the `heavy-tests` binaries `checkpoint_b_resolution_ab`,
`offset_candidates_m5` and `scallop_candidates_m4` (they pin `Debug` hashes).
The unit test in §4 proves their pins without a long run. The full viz suite
and the core full gate did not run.

## 4. The parity proof

1. Only tests write a `Some` cap. `rg "feed_cap\s*[:=]" crates` shows every
   non-test write as `None` or a copy of an input cap.
2. With `None` everywhere, each new branch is the identity:
   - the modulator: `if let Some(cap)` does not fire, so the feed and the
     binding are unchanged;
   - feedopt: `map_or(feed, ..)` returns `feed`;
   - arc fit and segment merge: `None == None`, so the run keys are as before;
   - the copies copy `None`.
3. `Move` has no `Serialize` derive, so no file, JSON or MCP payload changes.
   `BindingConstraint` gains a variant that no shipped run reports.
4. The G-code and toolpath output tests pass: G-OPGOLDEN (every op through
   the session path) and `adaptive3d_emission_byte_parity`.
5. The `Debug` text is unchanged. Many sentries pin an FNV-1a hash of
   `format!("{:?}", tp.moves)` (`tests/common/fingerprint.rs` and three local
   copies). With a derived `Debug`, the new field changed that text, and
   `finish_resolution_policy_pr3` failed 3/10 with equal move counts and
   different hashes. A hand-written `Debug` now prints `feed_cap` only when it
   is `Some`. The unit test `a_move_with_no_cap_prints_as_before_the_field`
   compares it with a derived copy of the old three-field struct, in the `{:?}`
   and `{:#?}` forms, for all four move types. After the change `pr3` passes
   10/10.

## 5. Pre-existing reds (not this step)

Each one fails on an S11 "unset feeds" fixture before any changed code runs.

- `crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`
  (:229): `NotSet { op: "Unified Finish", kind: Feeds, .. }`. DEFECTS.md lists
  it under G-FEEDSFIXTURE.
- `tabs_sit_on_straight_sides_p5::tabs_lifts_match_apply_tabs_for_the_same_points_p5`
  (:444): `cfg.feed_rate.expect("S11: the fixture sets it")` panics; the
  fixture's `feed_rate` is `None`. This diff does not touch that field.
  DEFECTS.md does not list this test. It is the same class as G-FEEDSFIXTURE.

## 6. Questions for the lead

1. **The hand-written `Debug` on `Move`.** It keeps every `Debug` pin valid
   without a re-bless. The alternative is to re-pin each `Debug` hash, which
   breaks the rule that a re-bless names only the changed op. The research
   branch had neither: its DEFECTS entry reads the `crease_own_region_pr6b`
   hash change as "stale pins", but the move counts match there too, so the
   cause is probably this field. Re-check that entry.
2. **Builder rebuilds drop the cap.** A dressup that rebuilds a cut through
   `feed_to_with_intent` and the other builders gets `None`. On master only the
   dogbone overcut does this to a cut, and it now carries the cap. When a
   planner starts to write caps (step 7), a census sentry can check this: a
   capped input cut must leave every dressup with a cap at or below its own.
3. **`scripts/cargo_lane.sh` changes to the root of the repo that holds the
   script.** From a worktree, the main tree's copy builds the main tree. Each
   worktree must call its own copy.
