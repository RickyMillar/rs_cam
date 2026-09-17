# core-cutting — design and feature-debt audit

Group: `crates/rs_cam_core/src/ops/` (17 files), `adaptive/` (5), `adaptive3d/` (5), `dressup/` (11).
Date: 2026-09-17. Read-only audit. Line counts: ops 8 613, adaptive 5 661, adaptive3d 8 280, dressup 10 701 (33 255 total).

### CUT-01 `blend_corners` and `blend_corners_to_moves` are the same 90 lines twice
- kind: design
- pattern: duplicate helpers
- where: `crates/rs_cam_core/src/ops/adaptive_shared.rs:112` (`blend_corners_to_moves`), `crates/rs_cam_core/src/ops/adaptive_shared.rs:196` (`blend_corners`)
- evidence: I read both bodies. The corner test, the `170°` / `half < 0.02` reject, the `setback > ba_len * 0.4` reject, the bisector, `center_dist`, `a1`/`a2` and the sweep wrap are character-identical. They differ only at the tail: one pushes `BlendedMove::Arc { end, center, clockwise }`, the other linearises the same arc into `n_pts` points. The two `// SAFETY:` allow blocks are also duplicated.
- proposal: keep one generic corner walk that yields `BlendedMove`; make `blend_corners` a thin `blend_corners_to_moves(...).flat_map(linearise)`. One arc-geometry definition, so a fix to the reject thresholds cannot land in one arm only.
- breaks: none (both are `pub(crate)`)
- effort: S
- risk: low — the linearised arm must keep emitting the same `n_pts` clamp `(2, 20)`; a byte-compare of one adaptive toolpath before and after is enough.
- sentry: `adaptive_property_harness` covers the 2D arm; add a unit test asserting `blend_corners(p, r)` equals the linearisation of `blend_corners_to_moves(p, r)` on a random corner set.
- owner:

### CUT-02 Every dressup ships a `pub` twin that only tests call
- kind: design
- pattern: pub item used only from tests
- where: `crates/rs_cam_core/src/dressup/mod.rs:163` `apply_entry`, `:980` `apply_lead_in_out`, `:1413` `apply_dogbones`, `:1603` `apply_link_moves`, `:1960` `filter_air_cuts`; `dressup/arcfit.rs:62` `fit_arcs`; `dressup/condition.rs:47` `merge_linear_runs`; `dressup/tsp.rs:191` `optimize_rapid_order`
- evidence: `rg` for each name, excluding the `_with_provenance` twin, returns only `dressup/tests.rs`, `dressup/<own file>` test modules, `benches/` and `crates/rs_cam_core/tests/*.rs`. The one production caller of a plain name is `apply_tabs` (`compute/execute/clearing_2d.rs:287`), which has no provenance twin at all. `compute/execute/dressup_apply.rs:362` imports only the `_with_provenance` forms.
- proposal: delete the eight plain wrappers. Give the tests one helper, `fn plain(t: Transformed) -> AnnotatedToolpath { t.into_parts().0 }`, and have them call the provenance form. The `_with_provenance` suffix then stops meaning anything and can go too, leaving one name per dressup.
- breaks: eight public signatures in `rs_cam_core::dressup` (operator ruling 2026-09-16 allows it); ~40 call sites in `dressup/tests.rs` and six integration tests.
- effort: M
- risk: low — pure rename/delete; the compiler finds every site.
- sentry: the existing `dressup/tests.rs` suite; no new test needed.
- owner:

### CUT-03 `retract_strategy` is a dial with two product surfaces and no reader
- kind: feature-debt
- pattern: dial nothing reads
- where: `crates/rs_cam_core/src/compute/config.rs:2028` (enum), `:2083` (field), `:2125` (default); `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:518-526` (combo box), `:278` (badge); `crates/rs_cam_viz/src/mcp_server.rs:1157` (documented MCP field)
- evidence: `rg -n "RetractStrategy::" crates/rs_cam_core/src` outside `compute/config.rs` returns nothing — the enum is constructed at its own default and read by no core code. The GUI offers Full/Minimum and the MCP `set_dressup_config` doc string advertises it. `dressup/CLAUDE.md` already states "The retract-strategy dial is dead (G-RETRACTDIAL)."
- proposal: delete `RetractStrategy`, the `DressupConfig::retract_strategy` field, the combo box and the MCP doc mention. The retract COUNT is the lever the linking work identified; leaving an inert dial on two surfaces tells an operator a setting exists that changes nothing.
- breaks: the `retract_strategy` project-file key (`session/mod.rs:2418` writes `retract_strategy = "full"`), the MCP wire snapshot `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json:1669`, and `crates/rs_cam_core/tests/adaptive3d_post_tsp_z_monotonicity.rs:66`.
- effort: S
- risk: low — no behaviour depends on it; the snapshot test must be re-blessed.
- sentry: `adaptive3d_post_tsp_z_monotonicity` and the MCP wire-surface snapshot both fail loudly on the removal, which is the proof it is inert.
- owner:

### CUT-04 `Adaptive3dParams` is 30 fields with five boolean modes
- kind: design
- pattern: parameter struct > 8 fields
- where: `crates/rs_cam_core/src/adaptive3d/mod.rs:99-226`
- evidence: 30 `pub` fields (counted with python over the struct body) against 14 in the 2D `AdaptiveParams` (`adaptive/mod.rs:111`). Five encode a mode as a bool or an `Option` that means "off": `detect_flat_areas`, `z_blend`, `mill_shallow_areas`, `fine_stepdown: Option<f64>`, `shallow_stepdown: Option<f64>`, plus two overlapping stay-down dials — `max_stay_down_dist: Option<f64>` (`:138`) and `max_stay_down_distance_mm: Option<f64>` (`:219`) — whose names differ only in the unit suffix.
- proposal: split into three named groups — `Adaptive3dGeometry` (tool, envelope, stepover, tolerance, boundary, stock frame), `Adaptive3dDepth` (depth_per_pass, fine_stepdown, shallow tier, z_floor, stock_top_z, stock_to_leave) and `Adaptive3dLinking` (stay-down, region ordering, min lengths). Fold the shallow-area trio into one `ShallowTier { angle_rad, stepdown }` option so "mill_shallow_areas = true with no angle" stops being representable. Resolve the two stay-down names into one.
- breaks: the `Adaptive3dParams` literal at every construction site (`compute/execute/clearing_2d.rs`, tests).
- effort: L
- risk: medium — 30 fields moved by hand; a mis-threaded field changes a toolpath silently. Do it as one mechanical commit with a byte-compare of a generated adaptive3d toolpath either side.
- sentry: `adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`.
- owner:

### CUT-05 `Adaptive3dParams::max_stay_down_dist` is read but no surface can set it
- kind: feature-debt
- pattern: field read but never set
- where: `crates/rs_cam_core/src/adaptive3d/mod.rs:138` (field), `crates/rs_cam_core/src/adaptive3d/path.rs:646` (reader), `crates/rs_cam_core/src/compute/execute/finish_3d.rs:138` (the only production construction)
- evidence: `rg -n "Adaptive3dParams\s*\{"` over `rs_cam_core/src`, `rs_cam_viz/src` and `rs_cam_cli/src`, excluding tests, returns exactly one site: `finish_3d.rs:103`. That literal writes `max_stay_down_dist: None` (line 138) and threads `max_stay_down_distance_mm: cfg.max_stay_down_distance_mm` (line 184) from the config. So `path.rs:646`'s `params.max_stay_down_dist.unwrap_or_else(|| (tool_radius * 6.0).max(params.stepover * 6.0))` always takes the fallback arm. The CLI job key of the same name (`rs_cam_cli/src/job.rs:273`) is coalesced into the OTHER name at `job.rs:914` (`op.max_stay_down_distance_mm.or(op.max_stay_down_dist)`), so even that alias never reaches this field. `max_stay_down_dist` appears in neither `compute/operation_configs.rs` nor `compute/catalog/registry.rs`; `max_stay_down_distance_mm` appears in both.
- proposal: delete `max_stay_down_dist` and inline its fallback at `path.rs:646` as the named default it already is. Two fields whose names differ only by a unit suffix, one of them unreachable, is the shape that makes an operator think a dial does nothing when they set the other one.
- breaks: the `Adaptive3dParams` literal at `finish_3d.rs:138` plus ~11 test literals; the CLI `max_stay_down_dist` job key (`job.rs:273`, `sweep.rs:421`), which today silently lands in a different field.
- effort: S
- risk: low — the reader keeps the identical fallback value, so emitted motion is byte-identical.
- sentry: `adaptive3d_keep_down_link_f038b` guards the live `max_stay_down_distance_mm` arm; add an assertion in it that the dial the CLI accepts is the dial `path.rs` reads.
- owner:

### CUT-06 The dressup chain is ordered but not uniform: eight hand-written blocks
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:338-798` (`apply_dressups`, 461 lines, brace-balanced); the uniform inner wrapper at `:197` (`apply_dressup_traced`); the eight step signatures it drives live in `dressup/mod.rs:178` (entry), `:1426` (dogbones), `:1055` (leads), `:1616` (links), `dressup/arcfit.rs:75`, `dressup/condition.rs:56`, `dressup/tsp.rs:209`, `dressup/mod.rs:1979` (air-cut)
- evidence: there IS one ordered pipeline and one uniform inner shape — `apply_dressup_traced` (`:197`) takes `impl FnOnce(AnnotatedToolpath) -> Transformed` and every step is reconciled through `ReconcileSet`. What is not uniform is the outside: each of the eight steps is a hand-written ~30-line block that repeats an `if cfg.<bool>` gate, a `DressupTraceInfo { debug_key, debug_label, kind, semantic_label }` literal, a `|scope| scope.set_param(...)` closure and a call closure. `DressupConfig` (`compute/config.rs`) carries 23 fields of which 7 are the enable bools; the entry step alone spends 60 lines duplicating the Ramp and Helix arms because the bool/enum gate is written twice.
- proposal: give each dressup one `DressupStep` value in `dressup/` — `{ trace: DressupTraceInfo, enabled: bool, params: fn(&ToolpathSemanticScope), run: Box<dyn FnOnce(AnnotatedToolpath) -> Transformed> }` — built by a small constructor per module, and have `apply_dressups` fold a `Vec<DressupStep>`. The ORDER then becomes a single readable list instead of 461 lines of interleaved policy, and adding a dressup stops being a copy-paste of the neighbouring block.
- breaks: none externally; `apply_dressups`'s own 17-argument signature is unchanged by this step (see CUT-12).
- effort: M
- risk: medium — the order and the capability gates must be preserved exactly; prove it with a byte-compare of one generated `.nc` per family either side.
- sentry: `capability_link_moves_safety`, `lead_in_out_feed_rates_f040`, `arcfit_gate_population_d4`.
- owner:

### CUT-07 Two dressups run outside the pipeline the folder file says is the only driver
- kind: feature-debt
- pattern: stale invariant / second application site
- where: `crates/rs_cam_core/src/dressup/CLAUDE.md` line 4 ("driven from `compute/execute/dressup_apply.rs`, never called ad hoc"); `crates/rs_cam_core/src/session/compute.rs:738` (`optimize_entry_descents_annotated`), `:1772` (`adaptive_feed_modulate`); `crates/rs_cam_core/src/dressup/mod.rs:755` `apply_tabs`, applied from inside generation at `compute/execute/clearing_2d.rs:287`
- evidence: `rg -n "adaptive_feed_modulate|optimize_entry_descents" --glob '!*tests*'` finds `apply_dressups` referencing neither. `session/compute.rs:1511` imports `adaptive_feed_modulate` directly. `apply_tabs` is the one dressup with no `_with_provenance` twin and a bare `Toolpath -> Toolpath` signature, so it runs inside the per-Z-level closure of the profile adapter, before any span or provenance channel exists.
- proposal: either move entry-descent optimisation, feed modulation and tabs into `apply_dressups` as three more steps, or amend `dressup/CLAUDE.md` to name the three exceptions and say why each must run where it does (tabs need the per-level cut depth; modulation needs the simulated engagement, which only exists after a sim). Writing the exception down is the minimum; today the folder file asserts an invariant the code does not hold, and the next engineer will trust it.
- breaks: none for the doc fix; folding tabs into the pipeline breaks `apply_tabs`'s signature.
- effort: S (doc) / M (fold in)
- risk: low for the doc; medium for the fold — tabs need the final-level Z the closure has.
- sentry: no test asserts "every dressup goes through `apply_dressups`"; write one that greps the crate for direct calls, or make the eight step constructors `pub(crate)` so only the driver can reach them.
- owner:

### CUT-08 `apply_tabs` rewrites tagged cut moves into `MoveIntent::Unknown`, including vertical descents
- kind: feature-debt
- pattern: guard reports the wrong thing
- where: `crates/rs_cam_core/src/dressup/mod.rs:870, 874, 878, 899, 903, 908, 910, 919, 921` (nine untagged `feed_to` calls); call site `crates/rs_cam_core/src/compute/execute/clearing_2d.rs:287`
- evidence: a python scan of every non-test line in `ops/`, `adaptive/`, `adaptive3d/`, `dressup/` and `finish/` for the untagged emitter forms (`rapid_to`, `feed_to`, `arc_*_to`, `emit_path_segment`, `emit_closed_contour`) returns exactly 15 production hits, all in `dressup/mod.rs`, and nine of them are in `apply_tabs`. `Toolpath::feed_to` (`toolpath.rs:141`) forwards to `feed_to_with_intent(..., MoveIntent::Unknown)`. The input moves it replaces carry `ClearingCut` / `FinishingCut` from `profile.rs:189`'s `emit_closed_contour_with_intent`. Only the untouched arm (`result.moves.push(m.clone())`, line 885) preserves the tag. Two of the nine are pure vertical moves — `feed_to(P3::new(split_x, split_y, cut_depth), feed_rate)` at `:899` and `:910` — a fed descent back to depth after a tab, emitted untagged. That is the same class `adaptive3d/CLAUDE.md` warns about ("This engine emits UNTAGGED vertical descents"), on the shipped cutout operation.
- evidence (consumers): `rg` for production readers of `.intent` finds `compute/spans.rs`, `trace/narrate.rs`, `stock/sim_triage.rs`, `machine/kinematics.rs`, `metrology/spacing.rs:99` and `dressup/entry_audit.rs`. `metrology/spacing.rs:99` skips any move whose intent is not `FinishingCut`, so a tabbed finishing profile drops out of the spacing instrument entirely.
- proposal: carry `m.intent` through every rewritten segment in `apply_tabs`, and tag the two vertical re-entries `MoveIntent::EntryPlunge`. Separately, rename `Toolpath::feed_to`/`rapid_to`/`arc_*_to` to `*_untagged` (or delete them and make the `_with_intent` form the plain name) so that emitting `Unknown` is a deliberate act rather than the shortest call.
- breaks: `Toolpath`'s four convenience emitters are `pub` and used ~90 times across test modules; renaming them is a large mechanical churn. The `apply_tabs` intent fix alone breaks nothing.
- effort: S (tabs) / M (rename the untagged door)
- risk: low — intent is metadata; no coordinate changes.
- sentry: `crates/rs_cam_core/tests/retract_intent_move_type_census_w6.rs` already censuses intents; extend it with a tabbed profile and assert zero `Unknown` cutting moves.
- owner:

### CUT-09 `clear_z_level_agent_2d_slice` is 1 032 lines with three named stages
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/adaptive3d/clearing.rs:1465-2497`
- evidence: brace-balanced measurement gives 1 032 lines, the largest function in the group (next: `adaptive3d/path.rs:303` `adaptive_3d_segments` at 756, `adaptive/path.rs:115` `adaptive_segments_with_debug` at 568). Its own comments mark the seams: the micro-region area filter and DPP scaling at +102..+144, "Nearest-neighbor region ordering" at +145, `for (region_idx, region_polygon) in regions.iter().enumerate()` at +235, and "F-038: post-emission coalescing pass" at +984. It carries `#[allow(clippy::too_many_arguments, clippy::indexing_slicing)]` and takes nine parameters, four of them `&mut` accumulators.
- proposal: extract three functions with the boundaries the comments already draw — `detect_and_order_regions(ctx, remaining, z_level) -> Vec<Polygon2>`, `clear_one_region(...)` (the ~750-line body, itself splitting at the `match params_2d.cleanup_strategy` at +552), and `coalesce_level_entries(segments, level_marker)`. The four `&mut` accumulators become one `LevelEmission` struct, which removes the `too_many_arguments` allow.
- breaks: none — `pub(super)`.
- effort: L
- risk: medium — this is the AgentSearch arm and all three `ClearingStrategy3d` variants are live (ruled 2026-06-08). Extraction must be mechanical, proven by a byte-compare of a generated adaptive3d toolpath.
- sentry: `adaptive3d_boundary_clear_parity`, `agent_search_coverage`, `adaptive3d_subtool_channel_gouge`.
- owner:

### CUT-10 Two plunge rates in one function: a magic heuristic beside the real dial
- kind: design
- pattern: unit-less number beside a real one
- where: `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:401-409` (the heuristic), `:345` (`plunge_rate_mm_min: Option<f64>`, the operation's own value); consumed by `dressup::apply_entry_with_provenance` (`dressup/mod.rs:178`) and `apply_link_moves_with_provenance` (`:1616`)
- evidence: two lines apart, `apply_dressups` takes `plunge_rate_mm_min: Option<f64>` — documented as "the OPERATION's own plunge rate (mm/min)" and explicitly "NOT the `plunge_rate` local below" — and then derives a second one by scanning for the first linear move and halving its feed, `unwrap_or(500.0)`. The 0.5 factor and the 500.0 fallback carry no unit, no source and no finding. The entry and link dressups — the two passes that emit descending fed motion — get the guessed value; only the feed optimiser gets the real one.
- proposal: pass the operation's `plunge_rate_mm_min` to the entry and link steps too, and delete the scan. Where the caller has no operation in scope, make the absence explicit (`None` → the dressup abstains or the caller supplies a named default constant), rather than inventing `500.0`.
- breaks: emitted ramp and link feed rates change wherever the operation's plunge rate is not half the first cut feed — a real motion change on shipped files, so it needs a measured before/after, not just a compile.
- effort: S to wire, M to validate
- risk: medium — it changes fed descent rates. `entry_moves_stock_aware_g_rampterrain` and `lead_in_out_feed_rates_f040` bound the geometry; the feed values need a fresh reading.
- sentry: `lead_in_out_feed_rates_f040`, `entry_moves_stock_aware_g_rampterrain`; add an assertion that a ramp's feed equals the op's plunge rate.
- owner:

### CUT-11 `apply_dressups` takes 15 positional arguments
- kind: design
- pattern: parameter list > 8
- where: `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:338-361`
- evidence: 15 parameters, carrying its own `#[allow(clippy::too_many_arguments)]`. Six are geometry/stock (`tool_diameter`, `safe_z`, `stock_top`, `prior_stock`, `feed_opt_stock`, `cutter`), three are feeds (`nominal_feed_rate`, `plunge_rate_mm_min`, and `cfg`'s own feed block), three are tracing (`debug_ctx`, `semantic_ctx`, `channels`). Its doc comment notes it "is called from three crates".
- proposal: group into `DressupContext { tool: DressupTool, heights: DressupHeights, stock: DressupStock, feeds: DressupFeeds }` plus the existing `cfg` and a `DressupTracing` triple. Three call sites change; every future dressup that needs one more input stops widening a 15-argument signature in three crates.
- breaks: `rs_cam_core::compute::execute::apply_dressups` is `pub` and called from `session/compute.rs:600`, `compute/execute/project_curve_chaining.rs:788` and the viz worker.
- effort: M
- risk: low — mechanical; the compiler finds every site.
- sentry: the existing `compute/execute/tests.rs` suite.
- owner:

### CUT-12 `adaptive3d/search.rs` no longer holds what its module doc and folder file say
- kind: feature-debt
- pattern: stale navigation aid
- where: `crates/rs_cam_core/src/adaptive3d/search.rs:1-2` (module doc), `crates/rs_cam_core/src/adaptive3d/CLAUDE.md` ("`search.rs` — direction search, engagement, entry-point finding")
- evidence: the file's whole production surface is `MaterialFloorDiagnostic` (:33), `material_remaining_at_level_diag` (:45), `MaterialRemaining` (:84), `material_remaining_at_level` (:105), `material_remaining_in_region` (:135), `is_clear_path_3d` (:189), `blend_corners_3d` (:237) and `interpolate_z_from_path` (:265). There is no direction search and no entry-point finding in it; those live in `clearing.rs` and `path.rs`. By contrast `adaptive/search.rs` really does hold `search_direction` (:252) and `find_entry_point` (:751), so the two folder files describe the same filename doing two different jobs.
- proposal: rename `adaptive3d/search.rs` to `material.rs` (its contents are the material-remaining query plus two path helpers) and correct both the module doc and the folder `CLAUDE.md` row. The per-folder instruction files are the operator's navigation aid (preference of 2026-09-17); a wrong file map costs more than a missing one.
- breaks: `mod search;` in `adaptive3d/mod.rs` and its `super::search::` call sites.
- effort: S
- risk: low.
- sentry: none needed; `cargo fmt --all -- --check` plus the crate build covers it.
- owner:

### CUT-13 `DressupConfig` pairs seven enable bools with their value fields
- kind: design
- pattern: boolean flags that encode a mode
- where: `crates/rs_cam_core/src/compute/config.rs` (`DressupConfig`, 23 fields); consumed at `compute/execute/dressup_apply.rs:374, 466, 534, 555, 597, 630, 653, 674, 740`
- evidence: python over the struct body counts 23 fields, seven of them `bool`: `dogbone`, `lead_in_out`, `link_moves`, `arc_fitting`, `segment_merge`, `feed_optimization`, `optimize_rapid_order`. Each sits beside its own value field (`dogbone_angle`, `lead_radius`, `link_max_distance` + `link_feed_rate`, `arc_tolerance`, `segment_merge_tolerance`, `feed_max_rate` + `feed_ramp_rate`). The state "arc_fitting = false, arc_tolerance = 0.05" is representable and means nothing; so is its mirror. `entry_style` already shows the better shape — one enum whose variants carry their own parameters (`DressupEntryStyle::{None, Ramp, Helix}` with `ramp_angle` / `helix_radius` / `helix_pitch` still hanging outside it).
- proposal: make each dressup one `Option<Params>` — `arc_fitting: Option<ArcFitParams>`, `link_moves: Option<LinkMoveParams>`, and so on — and fold `ramp_angle`/`helix_radius`/`helix_pitch` into the `DressupEntryStyle` variants they belong to. `apply_dressups`'s eight `if cfg.<bool>` gates become `if let Some(p) = ...`, which also removes the "which value field goes with which bool" reading burden.
- breaks: the project-file dressup block and the MCP `set_dressup_config` / `set_dressup_field` wire keys (`rs_cam_viz/src/mcp_server.rs:1157`, the wire snapshot). Operator ruling 2026-09-16 permits it; state the break in the commit.
- effort: M
- risk: medium — it is a file-format change; every fixture project and the wire snapshot must be re-blessed together.
- sentry: `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json` and the project round-trip tests fail loudly, which is the intent.
- owner:

### CUT-14 Two Z-ladder implementations with different arithmetic, one dial
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/ops/depth.rs:97` (`pass_count`) and `:171` (`roughing_levels`); `crates/rs_cam_core/src/finish/finish_setup.rs:678` (`z_ladder`), reached from `crates/rs_cam_core/src/ops/waterline.rs:98`
- evidence: the 2.5D clearing family builds its ladder through `DepthStepping::all_levels` — `n = ceil(total/step)` with a *relative* epsilon (`PASS_COUNT_EPS = 1e-9`, `depth.rs:121`) and then EVEN redistribution `start_z - (total/n) * i`, which `depth.rs:47-60` documents as a staircase ("Asking for 2.90 cuts 2.40"). `ops/waterline.rs:98` instead delegates to `finish::finish_setup::z_ladder`, a CONSTANT-step loop that accumulates `z -= step` with a caller-supplied *absolute* epsilon and no redistribution. Same operator dial, two realised ladders and two float-noise strategies; `z_ladder`'s repeated subtraction also drifts where `roughing_levels`' multiply does not.
- proposal: make `z_ladder` the one primitive and express `DepthStepping::roughing_levels` as `z_ladder` over the redistributed step, so there is one loop and one epsilon convention. If the two semantics must stay, name them on one enum (`LadderMode::{EvenRedistributed, ConstantStep}`) in `ops/depth.rs` and have `finish/` call in, rather than each family owning a private ladder.
- breaks: none if the emitted levels are preserved; `finish::finish_setup::z_ladder` is `pub`, so moving it changes a path.
- effort: M
- risk: medium — a ladder change moves every Z level. `waterline_shared_finish_setup_c3` already pins waterline against `z_ladder` across the boundary cases; the 2.5D side needs the same pinning before the merge.
- sentry: `waterline_shared_finish_setup_c3`, `the_written_depth_is_one_the_machine_cuts_g_stair`, `depth_beyond_stock_core_g_depthstockcore`.
- owner:

### CUT-15 Half of `DepthStepping` is unreachable: `Constant` and `finish_allowance`
- kind: feature-debt
- pattern: dial nothing sets
- where: `crates/rs_cam_core/src/ops/depth.rs:22` (`DepthDistribution::Constant`), `:39` (`finish_allowance`), `:153` (`roughing_depth`), `:158` (`roughing_floor`), `:163` (`has_finish_pass`), `:215` (`finish_level`), `:189` (the `Constant` arm of `roughing_levels`)
- evidence: `rg -n "DepthDistribution::(Constant|Even)"` across the workspace returns `Constant` only inside `depth.rs`'s own `#[cfg(test)]` module (`:425`, `:444`, `:573` of its test module). Every production construction sets `Even`: `compute/catalog.rs:1335, 1344`, `ops/face.rs:174, 307`, `rs_cam_viz/src/compute/worker/execute/mod.rs:516`, and `DepthStepping::new` (`:130`). `finish_allowance` is the same: every production site writes `0.0` (`catalog.rs:1336, 1345`, `face.rs:175, 308`, `worker/execute/mod.rs:517`, `depth.rs:130`), so `has_finish_pass()` is always false and `finish_level()` always `None`. `depth_stepped_with_finish` (`:293`) is already marked `#[cfg(test)]` "Test door. No production path reads it (S29, tech debt 2026-09-16)" — that audit found the function, not the field that feeds it. By contrast `finishing_passes` IS live (GUI at `rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:117-123`, threaded at `catalog.rs:1337`).
- evidence (why it matters): `depth.rs:47-60` documents the Even staircase as a real operator-facing defect — a proposed 2.90 mm step cuts 2.40 mm, "off by 17 %". `DepthDistribution::Constant` is the arm that would cut what was asked for, and it is implemented at `:189` and reachable from no surface.
- proposal: decide, do not leave both. Either expose the distribution as an operation dial (it is the documented fix for the staircase) or delete `Constant`, `finish_allowance`, `roughing_floor`, `has_finish_pass`, `finish_level` and the `#[cfg(test)]` `depth_stepped_with_finish`, which removes about 80 lines and the two-arm `match` from the hot ladder.
- breaks: deletion breaks the `DepthStepping` literal at five production sites and its test module; exposure breaks the operation config and the project file.
- effort: S
- risk: low — no production behaviour depends on either arm today, which is exactly the finding.
- sentry: `the_written_depth_is_one_the_machine_cuts_g_stair` pins the Even staircase; add an assertion that the distribution an operation config carries is the one `roughing_levels` applies.
- owner:

## Top three

1. **CUT-08** — `apply_tabs` rewrites tagged cut moves into `MoveIntent::Unknown`, including vertical descents. Small fix, and it is the September untagged-plunge class reappearing on the shipped cutout operation; `metrology/spacing.rs:99` already silently drops a tabbed finishing profile.
2. **CUT-03** — `retract_strategy` is a dial with two product surfaces and no reader. One afternoon, removes an operator-visible setting that changes nothing, and closes a named open item (G-RETRACTDIAL).
3. **CUT-15** — half of `DepthStepping` is unreachable, and the unreachable half (`DepthDistribution::Constant`) is the documented fix for a depth error the code itself calls "off by 17 %". Cheap to resolve either way, and the decision is worth more than the lines.

Runners-up by benefit per effort: **CUT-05** (a dial the CLI accepts that lands in a different field) and **CUT-02** (eight public wrappers that only tests call).

## Checked and clear

- **Depth stepping is centralised for the 2.5D clearing family.** `compute/execute/clearing_2d.rs:533` `effective_levels` builds the ladder once and every adapter (rest, zigzag, trace, profile, pocket, face, adaptive) runs it through `ops::depth::toolpath_at_levels_with_cancel`. No 2.5D operation re-implements the loop or the inter-level retract.
- **Cancellation has one cadence.** `toolpath_at_levels_with_cancel` (`ops/depth.rs:349`) is the single choke point; it checks the flag as its first statement and again per level, so an empty ladder still short-circuits.
- **Operation dispatch is one table, not N.** `compute/execute.rs:624-679` is a single `match` over `OperationConfig` with one arm per operation. 56 files name `OperationType::`, but most are behaviour classifications (feeds, tool_load, narration), not dispatch.
- **The 2.5D operations tag their intents.** A python scan of every non-test line in `ops/`, `adaptive/`, `adaptive3d/` for the untagged emitter forms returns zero hits; `pocket.rs:445` and `profile.rs:189` both go through `emit_closed_contour_with_intent`. The 15 production untagged calls are all in `dressup/mod.rs` (see CUT-08).
- **`Toolpath::final_retract` tags correctly.** `toolpath.rs:321` emits `MoveIntent::Retract`, so every inter-level retract in the 2.5D family is tagged.
- **One linking kernel.** `finish::surface_link::relink_fragments` has five production callers (`compute/execute/{project_curve_chaining,finish_raster,curve_engrave}.rs`, `metrology/costing.rs`) and no second implementation, as `dressup/CLAUDE.md` claims.
- **The dressup chain really is one ordered pipeline with a uniform inner shape.** `apply_dressup_traced` (`compute/execute/dressup_apply.rs:197`) gives every step the same `AnnotatedToolpath -> Transformed` contract and one `ReconcileSet` reconcile. The problem is the outside (CUT-06) and the three bypasses (CUT-07), not the contract.
- **`blend_corners_3d` is not a third copy.** `adaptive3d/search.rs:237` projects to 2D and delegates to `ops::adaptive_shared::blend_corners`. Only the 2D pair (CUT-01) is duplicated.
- **`waterline_z_levels` is already consolidated.** `ops/waterline.rs:98` delegates to the shared `z_ladder` and `waterline_shared_finish_setup_c3` pins the two. The remaining split is against `ops/depth.rs`, not within `finish/`.
- **`AgentSearch`, all three `ClearingStrategy3d` variants and `clear_z_level` are live** — confirmed, and CUT-09 proposes no deletion, only an extraction inside `clear_z_level_agent_2d_slice`.
- **`ops/mod.rs` is a pure 24-line facade (15 `pub mod` lines).** No logic, no dispatch; adding a module there costs one line.

## Add-a-thing count

### Add a 2.5D operation

Files an engineer edits today, from the `Waterline` / `Rest` touch points. Core structural (an edit is forced — the build fails or the operation is invisible):

1. `crates/rs_cam_core/src/ops/<new_op>.rs` — the new module
2. `crates/rs_cam_core/src/ops/mod.rs` — one `pub mod` line
3. `crates/rs_cam_core/src/compute/catalog.rs` — the `OperationType` variant, the `ALL` list (`:302`), the registry lookup arm (`:340`), the optimizable arm (`:893`)
4. `crates/rs_cam_core/src/compute/catalog/registry.rs` — the `OpRegistryEntry` static and its `ParamDef` rows
5. `crates/rs_cam_core/src/compute/catalog/schema.rs` — the schema arm
6. `crates/rs_cam_core/src/compute/operation_configs.rs` — the config struct and its `Default`
7. `crates/rs_cam_core/src/compute/config.rs` — the `OperationConfig` variant
8. `crates/rs_cam_core/src/compute/execute.rs` — the dispatch arm (`:624-679`)
9. `crates/rs_cam_core/src/compute/execute/clearing_2d.rs` — the `generate_<op>` adapter
10. `crates/rs_cam_core/src/compute/generated_empty.rs` — the empty-result classification
11. `crates/rs_cam_core/src/compute/spans.rs` — span classification
12. `crates/rs_cam_core/src/session/compute/params.rs` — parameter plumbing
13. `crates/rs_cam_core/src/trace/narrate.rs` — narration arms (19 variants named today)
14. `crates/rs_cam_core/src/stock/simulation_cut.rs` — simulation classification
15. `crates/rs_cam_core/src/feeds/{geometry_class,predict,provenance,vendor_normalize}.rs` — *owner: power session*
16. `crates/rs_cam_core/src/tool_load/{chipload,deflection,power,optimize/candidate}.rs` — *owner: power session*
17. `crates/rs_cam_cli/src/job.rs` — the job-file row
18. `crates/rs_cam_viz/src/state/toolpath/configs.rs`, `ui/properties/operations/mod.rs`, `ui/properties/toolpath_panel.rs` — the GUI panel
19. `crates/rs_cam_mcp/src/server.rs` + `crates/rs_cam_viz/src/mcp_server.rs` — the MCP surface

**Count: 22 files minimum, 26 with the two power-session folders**, across four crates. `rg -l "Waterline" --glob '*.rs'` returns 74 files in total; the list above is the subset where the edit is structural rather than behavioural.

### Add a dressup

1. `crates/rs_cam_core/src/dressup/<new_pass>.rs` — the pass, with a `*_with_provenance` signature and (today) a plain twin nobody but tests calls (CUT-02)
2. `crates/rs_cam_core/src/dressup/mod.rs` — the re-export
3. `crates/rs_cam_core/src/compute/config.rs` — an enable `bool` plus one value field per dial on `DressupConfig` (23 fields today, CUT-13), and the `Default`
4. `crates/rs_cam_core/src/compute/execute/dressup_apply.rs` — a hand-written ~30-line block in `apply_dressups`, placed by hand in the order (CUT-06)
5. `crates/rs_cam_core/src/trace/semantic_trace.rs` — a `SemanticKey` per parameter the step records
6. `crates/rs_cam_core/src/compute/catalog.rs` — `OperationTransformCapabilities`, if the pass reorders or links moves
7. `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs` — the GUI control
8. `crates/rs_cam_viz/src/mcp_server.rs` — the `set_dressup_config` / `set_dressup_field` doc string and the wire snapshot `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json`
9. `crates/rs_cam_cli/src/job.rs` — the job-file dressup key

**Count: 9 files across three crates.** The historical reference is `970e12a9` ("segment-merge conditioning"), which touched 5 production files before the tree was regrouped and before the GUI and MCP surfaces were added — the surfaces are what grew the list.

## Could not verify

- `apply_dressups`'s own doc comment says it "is called from three crates". `rg -n "apply_dressups" crates/rs_cam_viz/src crates/rs_cam_cli/src` returns only comments and a test module (`compute/worker/helpers.rs:10`, `compute/worker/gen_parity_p0_tests.rs:14, :413`); the two production callers I could find are both in core (`session/compute.rs:600`, `compute/execute/project_curve_chaining.rs:788`). Either the doc is stale or the viz worker reaches it through a re-export I did not trace — CUT-11's "three call sites" should be re-counted before that refactor starts.
- The runtime effect of CUT-10 (two plunge rates). I can show the two values exist side by side; I cannot say how far apart they are on a real job without running one, and this audit runs no cargo.
