# Design and feature-debt audit — core-finish

Group: `crates/rs_cam_core/src/finish/` (32 files, ~33k lines).
Auditor: read-only pass, 2026-09-17.

## Findings

### FIN-01 The research island ships 7.4k lines in the product crate
- kind: feature-debt
- pattern: research arm in the product path
- where: `crates/rs_cam_core/src/finish/mod.rs:10`, `crates/rs_cam_core/src/finish/mod.rs:13`, `crates/rs_cam_core/src/finish/conformal_spiral.rs:252`, `crates/rs_cam_core/src/finish/spiral_finish_compact.rs:50`
- evidence: `rg '^\s*use .*(conformal_spiral|direction_field|spiral_finish_compact)' crates/*/src` returns 4 hits, all inside `finish/` itself. No file under `crates/*/src` outside `finish/` imports any of the three. `metrology/floor.rs` names `conformal_spiral` only in `//!` doc lines (`floor.rs:4`, `:219`). The three modules plus their child folders are 2981 + 1441 + 2062 + 947 = 7431 lines, which is 22 % of the 33 245-line folder. The only non-doc consumers are 6 files under `crates/rs_cam_core/tests/`.
- proposal: Put `conformal_spiral`, `direction_field` and `spiral_finish_compact` behind a `research` cargo feature, or move them to a `research` crate that only the test targets depend on. The folder `CLAUDE.md:19-21` already calls two of them "research arms, both measured"; the build does not.
- breaks: the 6 test targets that import them (`graded_raster_e1.rs`, `banded_raster_costed_f2.rs`, `valley_branch_falsifier_h1.rs`, `direction_field_wanaka_f1.rs`, `spiral_finish_compact_c1.rs`, `whole_board_spiral_ledger_g1.rs`) need the feature in their `required-features`.
- effort: M
- risk: low — no product surface calls them, so no emitted move changes.
- sentry: a test that asserts no `src/` file outside the research set names `conformal_spiral` or `direction_field`, in the style of the existing arch-guard tests.
- owner:

### FIN-02 Every strategy has a Config struct and a near-identical Params struct
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/compute/execute/finish_3d.rs:219`, `:349`, `:655`, `:699`, `:752`, `:815`, `:848`; `crates/rs_cam_core/src/compute/execute/finish_raster.rs:185`
- evidence: Eight hand-written `…Params { … }` literals copy a `…Config` field by field. The field counts pair up: `ScallopConfig` 12 / `ScallopParams` 12, `SteepShallowConfig` 12 / `SteepShallowParams` 12, `RampFinishConfig` 11 / `RampFinishParams` 11, `PencilConfig` 20 / `PencilParams` 21, `UnifiedFinishConfig` 23 / `UnifiedFinishParams` 12. The pencil block alone runs `finish_3d.rs:219-254`.
- proposal: Give each config an inherent `fn params(&self, …) -> …Params` beside the existing `UnifiedFinishConfig::planner_params` (`finish_3d.rs:368`), and move the copy there. The executor then reads only the fields it truly owns, and the seam becomes visible instead of inlined.
- breaks: none — the translation stays private to the crate.
- effort: M
- risk: low — a mechanical move with a per-field diff.
- sentry: `cargo test -p rs_cam_core -q --test finish_resolution_policy_pr3` plus the existing `compute/execute/tests.rs` config round-trips.
- owner:

### FIN-03 `UnifiedFinishConfig` carries 11 fields the strategy never sees
- kind: design
- pattern: a struct that every layer mutates
- where: `crates/rs_cam_core/src/compute/operation_configs.rs:1027`, `crates/rs_cam_core/src/compute/execute/finish_3d.rs:349-360`
- evidence: `UnifiedFinishConfig` has 23 fields, 3 booleans and 4 `Option`s. `UnifiedFinishParams` takes only 12 of them (`finish_3d.rs:349-361`). The remaining fields split three ways inside the executor: planner dials through `cfg.planner_params(…)` (`:368`), claims dials through `cfg.pencil_claims.then(|| …)` (`:376`), and inert dials that only reach a finding (`:374`).
- proposal: Split `UnifiedFinishConfig` into the three groups the executor already separates — pass params, planner params, claims params — as three named nested structs. The config file key layout can stay flat with `#[serde(flatten)]`.
- breaks: the Rust struct shape; the TOML keys can stay identical.
- effort: M
- risk: medium — the config is a saved-project surface, so the serde shape needs a round-trip test.
- sentry: the project-file round-trip tests under `crates/rs_cam_core/tests/`; add an assertion that every `UnifiedFinishConfig` field reaches exactly one of the three groups.
- owner:

### FIN-04 Two dials are dead and the code says so at run time
- kind: feature-debt
- pattern: a dial nothing reads
- where: `crates/rs_cam_core/src/compute/execute/finish_3d.rs:260`, `:374`; `crates/rs_cam_core/src/compute/execute/findings.rs:99`, `:155`
- evidence: `record_deprecated_dial` fires for `route_width_factor` with the comment "still deserialized so every saved project loads unchanged, but … nothing reads it" (`finish_3d.rs:255-259`). `record_inert_claims_dial` fires for `min_rest_depth_mm` and `claims_reference` when `territory_clip == false` (`findings.rs:155-170`). Both recorders suppress themselves at the default value, so an operator at defaults sees nothing.
- proposal: The 2026-09-16 no-legacy ruling removes the reason these stay. Delete `route_width_factor` from `PencilConfig` and `PencilParams`, and delete `record_deprecated_dial` with it. Keep the inert-claims finding; its two dials are live under `territory_clip`.
- breaks: the `route_width_factor` key in every saved project file, and `finish::pencil::route_width_factor_default`.
- effort: S
- risk: low — the value already steers nothing.
- sentry: the pencil emission tests in `crates/rs_cam_core/src/finish/pencil/tests.rs`; add a load test that an old project with the key still loads.
- owner:

### FIN-05 The `#[ignore]` wanaka harness is three diagnostics and one gate
- kind: feature-debt
- pattern: research harness shipped as a test target
- where: `crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs:89`, `:227`, `:342`, `:497`
- evidence: The file holds four `#[ignore]` tests. `wanaka_decomposes_to_order_ten_regions` (`:90`) asserts a region count, so it is a gate. `wanaka_slope_distribution_diagnostic` (`:228`) is headed "Diagnostic (no assertions beyond sanity)" (`:219`). `wanaka_band_mix_vs_cusp_radius` (`:498`) is headed "Diagnostic, not a gate: it prints band mixes and asserts nothing" (`:495`). `p2e_conditioning_dial_sweep` (`:343`) is a dial sweep whose only hard assertion is determinism (`:467-468`). The folder `CLAUDE.md:37` names the whole file a sentry.
- proposal: Keep the one gate in this file. Move the three diagnostics to a `heavy-tests` binary or to `benches/`, and correct `finish/CLAUDE.md:37` to name the gate function, not the file.
- breaks: none — the diagnostics never ran in a gate.
- effort: S
- risk: low — no product code changes.
- sentry: the gate itself, `wanaka_decomposes_to_order_ten_regions`.
- owner:

### FIN-06 A test target loads a project from `planning/`
- kind: feature-debt
- pattern: a fixture outside the fixture directory
- where: `crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs:34-40`
- evidence: `wanaka_project_path()` builds `…/planning/airrun_2026-06-01/wanaka.toml`. `crates/rs_cam_core/tests/fixtures/` already holds `wanaka_2026-08-16_f530995a.toml`, a pinned copy with a content hash in its name. `rg -n 'join("planning")' crates/rs_cam_core/tests/*.rs` finds 8 such call sites. The 2026-09-17 purge deleted 1010 `planning/` files; this one survived by chance, and `planning/CLAUDE.md` says the directory is evidence, not a fixture store.
- proposal: Point the harness at `tests/fixtures/`. Copy each `planning/` project a test needs into `tests/fixtures/` with a hash in the name, as the existing fixture does.
- breaks: none.
- effort: S
- risk: low — the decompose numbers may shift if the two wanaka projects differ; re-bless once and record the hash.
- sentry: the same gate, after the fixture swap.
- owner:

### FIN-07 A 1150-line orchestrator with six numbered seams
- kind: design
- pattern: a god function with a visible seam
- where: `crates/rs_cam_core/src/finish/unified_finish.rs:1519`
- evidence: `unified_finish_toolpath_with_cancel_and_ceiling` runs from line 1519 to line 2668, which is 1150 lines. The body carries its own section comments: "Step 1: classify" (`:1537`), "Step 2: coverage ∧ machining boundary" (`:1560`), "Step 2.5: in-op rest analysis" (`:1589`), "Step 2.6: S4 territory confinement" (`:1775`), "Step 3: decompose" (`:1872`), "Step 3.5: crease-claims emission" (`:1910`), "Step 4: per-region generation" (`:1948`), "Step 5: route" (`:2484`), "Step 6: stitch" (`:2535`). Step 4 alone is 536 lines.
- proposal: Cut the function along the comments it already carries. Each step becomes a private function that takes a small context struct and returns its own product. Step 4's three band arms become three functions, which also removes the per-arm `height_clip` shadowing.
- breaks: none — the entry point signature stays.
- effort: L
- risk: medium — the steps share about a dozen mutable accumulators (`report`, `uncut_core_mm2`, `untouched_mm2`, `standing_mm2`, `monotone_cell_totals`), so the context struct needs care.
- sentry: `crates/rs_cam_core/src/finish/unified_finish/tests.rs` already pins determinism (`:471`), byte-identity (`:247`, `:253`) and region-table ranges (`:1119`); run those before and after.
- owner:

### FIN-08 Two composition paths band the same surface
- kind: design
- pattern: two composition paths for one concept
- where: `crates/rs_cam_core/src/finish/steep_shallow.rs:1`, `crates/rs_cam_core/src/finish/unified_finish.rs:1948-2114`
- evidence: `unified_finish` bands into three (`FinishBand::VerySteep` → waterline at `:1983`, `MidSteep` → scallop at `:2058`, `Shallow` → raster at `:2114`) through `finish_planner::decompose`, which adds hysteresis, morphological close and small-region absorption. `steep_shallow` bands into two through `surface::slope::classify_steep_shallow` (`steep_shallow.rs:26`), with a `steep_first: bool` for ordering (`:44`) and dilate/erode overlap (`:85`, `:121`). The two paths never share a line: `steep_shallow.rs` does not import `finish_planner`.
- proposal: Make `SteepShallow` a preset of the unified planner: two bands, no hysteresis, `route_greedy` disabled. The 1475-line module then reduces to a config mapping, and the conditioning improvements the planner gained reach it for free.
- breaks: the `SteepShallow` operation's emitted moves change, so its saved projects re-generate differently.
- effort: L
- risk: high — it changes a shipped operation's output. Measure a paired A/B first, as the folder invariant demands.
- sentry: the `steep_shallow` module tests (`steep_shallow.rs:863-1475`) plus `cargo test -p rs_cam_core -q --test classification_strategy_m3`.
- owner:

### FIN-09 The pencil detector is a string, and a typo silently changes the strategy
- kind: design
- pattern: a stringly-typed door
- where: `crates/rs_cam_core/src/compute/operation_configs.rs:839`, `crates/rs_cam_core/src/finish/pencil.rs:86-93`, `crates/rs_cam_core/src/compute/execute/finish_3d.rs:233`
- evidence: `PencilConfig.detector: String`. `PencilDetector::parse` maps six tokens and sends everything else to `Dihedral`: `_ => PencilDetector::Dihedral`. The sibling configs are already typed — `ScallopConfig.direction: ScallopDirection` (`operation_configs.rs:943`), `UnifiedFinishConfig.claims_reference: ClaimsReference` (`:1108`), `classification_sampler: ClassificationSampler` (`:1240`). So a project file that says `detector = "rest-depth"` runs the dihedral detector and reports nothing.
- proposal: Change the field to `detector: PencilDetector` with a serde rename map, and delete `parse` and `detector_string_default`. An unknown token then fails the load with the key name, as every other enum field does.
- breaks: a project file with a non-canonical alias (`crest`, `ridgevalley`, `restdepth`, `rest`) now fails to load instead of running Dihedral. That is the intent.
- effort: S
- risk: low — three canonical tokens keep working.
- sentry: `crates/rs_cam_core/src/finish/pencil/tests.rs`; add a load test that an unknown detector token is refused.
- owner:

### FIN-10 Cancel, stage and resolution are function-name suffixes, not parameters
- kind: design
- pattern: entry-point fan-out
- where: `crates/rs_cam_core/src/finish/scallop.rs:1008`, `:1028`, `:1055`, `:1095`, `:1134`; `crates/rs_cam_core/src/finish/scallop/research.rs:75`, `:114`, `:150`, `:190`; `crates/rs_cam_core/src/finish/steep_shallow.rs:566`, `:599`, `:671`, `:701`; `crates/rs_cam_core/src/finish/finish_setup.rs:391`, `:433`, `:459`, `:532`, `:556`, `:582`
- evidence: Scallop has nine entry points, steep-shallow four, and `finish_setup` six surface builders. The folder holds 120 `pub fn` in total. Caller counts outside the owning module: `scallop_toolpath` 0 src / 11 tests, `scallop_toolpath_structured_annotated_with_cancel_and_stage` 1 src / 0 tests, `scallop_toolpath_research` 0 src / 15 tests, `steep_shallow_toolpath_with_cancel` 0 src / 0 tests, `build_finish_surface_with_cell_size_and_cancel` 3 src / 9 tests.
- proposal: Give each family one entry point that takes an options struct carrying `cancel`, `stage`, `resolution` and `ring_budget`. Delete the wrappers no product file calls.
- breaks: the public function names in `rs_cam_core::finish`, and the test targets that use them.
- effort: M
- risk: low — the wrappers only forward arguments.
- sentry: `cargo test -p rs_cam_core -q --test finish_resolution_policy_pr3` and `--test scallop_iso_field_config`.
- owner:

### FIN-11 Five convenience wrappers are `pub` for tests only
- kind: design
- pattern: a `pub` item used only from tests
- where: `crates/rs_cam_core/src/finish/scallop.rs:1008`, `crates/rs_cam_core/src/finish/steep_shallow.rs:566`, `crates/rs_cam_core/src/finish/horizontal_finish.rs:69`, `crates/rs_cam_core/src/finish/radial_finish.rs:53`, `crates/rs_cam_core/src/finish/spiral_finish.rs:93`
- evidence: Each is the non-cancellable wrapper around its `_with_cancel` sibling. `rg` over `crates/` excluding `crates/rs_cam_core/tests/` finds every call site inside the owning file's own `mod tests`. The product path calls the cancellable form: `finish_3d.rs:823` calls `radial_finish_toolpath_with_cancel`, `finish_3d.rs:856` calls `horizontal_finish_toolpath_with_cancel`.
- proposal: Fold these into FIN-10. If a family keeps its wrapper, mark it `#[cfg(test)]` or make the test pass a `&|| false` cancel check.
- breaks: the five names leave the public API.
- effort: S
- risk: low.
- sentry: the module tests in each file.
- owner:

### FIN-12 Four out-parameters carry a report out of the pencil generator
- kind: design
- pattern: out-parameters instead of a return value
- where: `crates/rs_cam_core/src/finish/pencil.rs:662-675`, `crates/rs_cam_core/src/compute/execute/finish_3d.rs:281-295`
- evidence: `pencil_toolpath_structured_annotated_with_cancel` takes `rest_grid_out: &mut Option<RestGrid>`, `rest_regions_out: &mut Option<Vec<Polygon2>>`, `tip_float_out: &mut Option<TipFloatFinding>` and `link_report_out: &mut Option<PencilLinkReport>`. The caller declares four `let mut … = None` bindings (`finish_3d.rs:281-284`) before the call. `unified_finish` solves the same problem with a returned `UnifiedFinishReport` (`unified_finish.rs:1112`). The folder carries 36 `#[allow(clippy::too_many_arguments)]` lines, and this signature is one of them.
- proposal: Add a `PencilReport` struct with the four fields and return `(Toolpath, Vec<Annotation>, PencilReport)`, matching `unified_finish`.
- breaks: the `pencil_toolpath_structured_annotated_with_cancel` signature.
- effort: S
- risk: low — a mechanical change with one call site in the product path.
- sentry: `crates/rs_cam_core/src/finish/pencil/tests.rs` link-report tests (`:958`, `:984`, `:1005`).
- owner:

### FIN-13 The band-clip finding exists twice, and the enum degrades to a string
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/finish/unified_finish.rs:963`, `:988`, `:1017`; `crates/rs_cam_core/src/compute/config.rs:1175`, `:1242`; `crates/rs_cam_core/src/compute/execute/findings.rs:61`, `:72`
- evidence: `finish/unified_finish.rs` defines `DroppedBand`, `BandHeightClip` and `ClippedBand`. `compute/config.rs` defines `DroppedBandFinding` and `ClippedBandFinding` that restate the same fields. The duplicate sweep scores the pair 0.9513 (`planning/structure_2026-09-17/evidence_round2/dup_sweep_0.88_src.md:74`) and 0.9142 (`:75`). Across the boundary the typed `band: FinishBand` becomes `band_label: &'static str` (`config.rs:1178`) and `clip: HeightClip` becomes `clip_label: &'static str` (`config.rs:1190`).
- proposal: Let the finding types hold `FinishBand` and `HeightClip` directly, and keep the `label()` functions for rendering only. Then merge the two struct pairs into one type each, owned by `compute::config`, and have `unified_finish` build them.
- breaks: any diagnostic consumer that matches on the label strings; check `diagnostics/adapters/from_generation.rs`.
- effort: M
- risk: low — the labels stay available through `FinishBand::label`.
- sentry: the diagnostic adapter tests under `crates/rs_cam_core/src/diagnostics/`; add a round-trip test from band to label.
- owner:

### FIN-14 Two of three bands cannot report a height clip, and the report reads as clean
- kind: feature-debt
- pattern: a guard that reports the wrong thing
- where: `crates/rs_cam_core/src/finish/unified_finish.rs:1981`, `:2019`, `:2403`; `crates/rs_cam_core/src/compute/config.rs:1234-1240`
- evidence: `rg -n 'height_clip' unified_finish.rs` returns three lines: the `None` declaration at `:1981`, one assignment at `:2019` and the read at `:2403`. The single assignment sits inside the `FinishBand::VerySteep` arm (`:1983-2057`). The `MidSteep` arm (`:2058`) and the `Shallow` arm (`:2114`) never assign it. `ClippedBandFinding`'s own doc states the limit: "only the `VerySteep` arm measures its clip today … a partial clip there is still invisible" (`config.rs:1234-1240`).
- proposal: Have the scallop and raster arms report their own requested-versus-delivered Z span, as the waterline arm does. Until they can, carry a `bands_measured` field on the finding so absence reads as "not measured" and never as "measured zero".
- breaks: the `ClippedBandFinding` struct shape, which is a diagnostic wire surface.
- effort: M
- risk: medium — the honest fix needs each arm to expose a Z span it does not currently return.
- sentry: `crates/rs_cam_core/src/finish/unified_finish/tests.rs`; add a case where a `bottom_z` clamp shortens a `MidSteep` region and assert the finding fires.
- owner:

### FIN-15 Two `pub` items have no product reader
- kind: design
- pattern: a `pub` item used only from one file or only from tests
- where: `crates/rs_cam_core/src/finish/unified_finish.rs:265`, `crates/rs_cam_core/src/finish/finish_setup.rs:371`
- evidence: `rg -n 'RoutedLink' crates/` finds three hits, all inside `unified_finish.rs` (`:268`, `:1130`, `:2525`); `dead_pub_surface.md:30` lists it. `reset_surface_build_count` carries its own doc line: "**Test door.** The harness `crates/rs_cam_core/tests/finish_surface_cache.rs` is the only caller. No production path reads it." (`finish_setup.rs:365-367`); `test_only_pub_api.md:20` lists it with 4 test uses. The same evidence file lists three more finish items: `sample_with_probe_diameter` (`classify_probe.rs:287`), `is_shipped` (`scallop.rs:706`) and `uncut_core` (`scallop.rs:1813`).
- proposal: Make `RoutedLink` `pub(crate)`, since it is reachable through the public `UnifiedFinishReport::links`, or keep it `pub` and say so in its doc. Put `reset_surface_build_count` behind `#[cfg(feature = "heavy-tests")]` or a `test-hooks` feature.
- breaks: `RoutedLink` and `reset_surface_build_count` leave the default public API.
- effort: S
- risk: low.
- sentry: `cargo test -p rs_cam_core -q --test finish_surface_cache`.
- owner:

## Top three

1. **FIN-01** — The research island ships 7.4k lines in the product crate. It removes 22 % of the folder from every product build, changes no emitted move, and the folder `CLAUDE.md` already calls the modules research.
2. **FIN-09** — The pencil detector is a string, and a typo silently changes the strategy. One field type change removes a silent wrong-strategy path, and every sibling config is already typed.
3. **FIN-02** — Every strategy has a Config struct and a near-identical Params struct. Eight hand-written copy blocks move into eight `impl` methods; the seam between the config surface and the strategy becomes visible and testable.

FIN-07 is the largest structural win but costs `L` effort and carries the shared-accumulator risk, so it ranks below these three on benefit per effort.

## Checked and clear

- Surface sampling is single-sourced. Outside `finish_setup.rs` and `classify_probe.rs`, every `SurfaceHeightmap::from_mesh` call in `finish/` is in a `mod tests`. The `finish_setup.rs:1-27` claim holds.
- Region routing is not duplicated. `geometry/nn_order.rs:1-23` names the five greedy-nearest copies it consolidated and explains why `unified_finish::route_greedy` is not a sixth: its cost is a link time, not an XY distance.
- The surface-build counter is not an ad-hoc cache. `finish_setup.rs:355` is a measurement instrument for the bounded memo in `maps/finish_surface_cache.rs`, and its doc says why it sits in the builder.
- `dressup/link.rs:78-85` is a deliberate second link check, not a duplicate of `surface_link::build_surface_link`. It samples emitted moves because the dressup stage holds no mesh.
- Debt markers are almost absent. `rg 'TODO|FIXME|HACK|XXX|for now'` over the folder returns three hits, all inside doc prose (`finish_planner.rs:167`, `direction_field.rs:624`, `conformal_spiral.rs:455`). None names a live consequence.
- The `iso_field` dial is live and stays. `scallop/research.rs:75` is its entry point; the folder invariant protects it.
- Areas already use newtypes. `ProjectedXyAreaMm2` and `SurfaceAreaMm2` guard the band-area arithmetic (`finish_planner.rs:256`, `config.rs:1199`), and a `compile_fail` doctest blocks the cross-domain division.
- `record_tip_float`, `record_ramp_reach_clamp` and `record_claims_reference` deliberately record a measurement rather than a defect, and say so (`findings.rs:80`, `:109`, `:122`). They are not silent-zero guards.

## Add-a-thing count

The group's extension point is **one new finishing strategy**. An engineer who adds one edits these files today. The list comes from `rg -l 'SteepShallow|steep_shallow'` across `crates/` and the root docs, with the hits that only reference the existing strategy removed.

New file:

1. `crates/rs_cam_core/src/finish/<strategy>.rs`

Core wiring (11):

2. `crates/rs_cam_core/src/finish/mod.rs`
3. `crates/rs_cam_core/src/compute/operation_configs.rs`
4. `crates/rs_cam_core/src/compute/catalog.rs`
5. `crates/rs_cam_core/src/compute/catalog/registry.rs`
6. `crates/rs_cam_core/src/compute/mod.rs`
7. `crates/rs_cam_core/src/compute/execute.rs`
8. `crates/rs_cam_core/src/compute/execute/finish_3d.rs`
9. `crates/rs_cam_core/src/compute/execute/shared.rs`
10. `crates/rs_cam_core/src/compute/generated_empty.rs`
11. `crates/rs_cam_core/src/toolpath.rs`
12. `crates/rs_cam_core/src/trace/narrate.rs`

Feeds classification (4):

13. `crates/rs_cam_core/src/feeds/geometry_class.rs`
14. `crates/rs_cam_core/src/feeds/predict.rs`
15. `crates/rs_cam_core/src/feeds/suggest/axial_envelope.rs`
16. `crates/rs_cam_core/src/feeds/vendor_normalize.rs`

MCP and GUI (9):

17. `crates/rs_cam_mcp/src/server.rs`
18. `crates/rs_cam_viz/src/mcp_server.rs`
19. `crates/rs_cam_viz/src/app/mcp/view.rs`
20. `crates/rs_cam_viz/src/state/toolpath.rs`
21. `crates/rs_cam_viz/src/state/toolpath/configs.rs`
22. `crates/rs_cam_viz/src/ui/properties/operations/mod.rs`
23. `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`
24. `crates/rs_cam_viz/src/ui/properties/operations/shape_diagrams.rs`
25. `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs`

Docs (2):

26. `FEATURE_CATALOG.md`
27. `crates/rs_cam_core/src/finish/CLAUDE.md`

Tests and benches (4):

28. `crates/rs_cam_core/src/compute/catalog/tests.rs`
29. `crates/rs_cam_core/src/compute/execute/tests.rs`
30. `crates/rs_cam_viz/src/compute/worker/tests.rs`
31. `crates/rs_cam_core/benches/perf_suite.rs`

**Count: 27 non-test files, 31 with tests and benches.** The `feeds/` rows carry `owner: power session`; they are listed as extension cost only, with no proposal attached. The number does not fall from any finding in this file. It falls when the `OperationType` enum stops being matched independently in `catalog.rs`, `registry.rs`, `execute.rs`, `generated_empty.rs`, `narrate.rs`, `toolpath.rs` and the four `feeds/` files — a registry-row refactor that belongs to the `core-compute` group, not here.
