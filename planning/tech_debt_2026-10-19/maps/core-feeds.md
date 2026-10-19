# Map: core-feeds (`crates/rs_cam_core/src/{feeds,tool_load,material}/`)

Evidence baseline: HEAD 99b6fb41. Read-only review. No file in scope is in `in_flux_paths.txt`
(the only in-flux paths that touch the area are tests: `crates/rs_cam_core/tests/drill_material_plumbing_f016.rs`
and four `crates/rs_cam_viz/tests/*feeds*` files). Line counts are `wc -l`; "prod" excludes `#[cfg(test)]` code.
Every path below is relative to `crates/rs_cam_core/src/` unless it starts with `crates/` or `planning/`.

## 1. Purpose and file map

Purpose. `feeds/` is the speeds-and-feeds calculator and the Suggest model: it turns (tool, material, machine,
operation) into RPM, feed, plunge, DOC, WOC, with a vendor-LUT chipload band, extrapolation claims and a
provenance record. `tool_load/` is the set of independent gates (chipload, power, deflection, depth, drill) that
judge a simulated cut and build the `ToolLoadReport`. `material/` is the `Material` catalogue and the one
`ForceLine` per material that feeds and tool_load both read (ruling B6). feeds and tool_load are mutually
dependent (section 3). The validated front door is `feeds::suggest`; the gate front door is `tool_load::evaluate_toolpath`.

### feeds/ (28,412 lines incl. tests)
| File | Responsibility | Lines |
|---|---|---|
| feeds/mod.rs | Core types (`FeedsInput`, `FeedsResult`, `FeedsWarning`, `FeedsError`, `OperationFamily`, `PassRole`, `CutterKind`, `ToolGeometryHint`), rubbing floor, `validate_tool_for_operation`, `calculate` (one fn, lines 1494-2822), default profiles | 3091 (tests external) |
| feeds/tests.rs | Unit tests for mod.rs | 1877 |
| feeds/suggest.rs | Suggest model: `SuggestContext`, `SuggestWarning`, `suggest_params`, `default_operation`, `feeds_result_for_operation`, `feeds_explain_for_operation` | 1067 |
| feeds/suggest/apply.rs | Write-back funnel `apply`, `ApplyScope`, previews (`FeedsPreview`, `FieldApplyPreviews`), stock and drill defaults | 1039 |
| feeds/suggest/invariants.rs | Clamps and back-offs (`enforce_invariants`) | 693 |
| feeds/suggest/aggressiveness.rs | Pass 6b, the machine dial | 489 |
| feeds/suggest/axial_envelope.rs | Axial DOC envelope pass; per-`OperationConfig` match | 426 |
| feeds/suggest/adaptive_entry.rs | Adaptive entry pass 10 (power-model based) | 582 |
| feeds/suggest/tests.rs | Suggest tests | 2675 |
| feeds/geometry.rs | Effective diameter, chip thinning, `doc_derating_scale`, `DeratedChiploadBand` (canonical DOC scale) | 746 |
| feeds/geometry_class.rs | `GeometryClass` classifier (op-type only) | 198 |
| feeds/cutter_constraints.rs | `CutterAxialConstraints`, axial binding, default deflection limits | 801 |
| feeds/force.rs | Force at an operating point from `ForceLine`; `DeflectionCapRefusal` | 454 |
| feeds/predict.rs | Deflection predictor, move-count predictor | 1129 |
| feeds/efficiency.rs | `CutEfficiency`, `ChipVerdict` | 389 |
| feeds/operating_point.rs | `PowerFigure`, `power_at_operating_point` | 363 |
| feeds/vendor_lut.rs | `VendorLut`, `VendorObservation`, LUT enums, 26 embedded JSON files, row validation | 1139 |
| feeds/vendor_lookup.rs | `lookup_best`, `LookupQuery`, `LookupResult`, scaling constants | 1877 (tests from 1021) |
| feeds/vendor_normalize.rs | Op/material/tool to LUT mapping, a4 routing (`lut_query_for`) | 569 |
| feeds/support.rs | `FeedsSupport` arm per cell, `formula_backing` judgement table, refusal texts | 653 |
| feeds/extrapolation.rs + extrapolation/{size,hardness,family,drill,plunge}.rs | G1-G10 claims: `SizeLaw`, `hardness_basis`, `FAMILY_RULES`, `DRILL_RULES`, `PLUNGE_RULES` | 391 + 459/331/564/437/816 |
| feeds/ramp.rs | G6 ramp: `RampBasis`, `resolve_ramp_feed`, `EntryNotes` | 1408 (tests from 962) |
| feeds/plunge.rs | G10 `PlungeBasis`, `resolve_plunge` | 457 |
| feeds/profile.rs | `CutterOpProfile` pre-sim bundle | 343 |
| feeds/quantities.rs | Unit newtypes: `CommandedFeedMmMin`, `AchievedFeedMmMin`, `AdvancePerToothMm`, `ArcMeanChipThicknessMm`, `VendorChiploadBand` | 322 |
| feeds/provenance.rs | `FeedsProvenance`, `FeedsField`, `ValueProvenance` | 558 |
| feeds/rationale.rs, feed_explanation.rs, explain_payload.rs | Rationale tree, stage record, UI payload `FeedsExplain` | 1277 / 342 / 336 |
| feeds/INTEGRATION.md | GUI integration note (stale, section 7) | 74 |

### tool_load/ (11,977 lines)
| File | Responsibility | Lines |
|---|---|---|
| tool_load/mod.rs | `evaluate_toolpath` (single verdict assembly), `MetricEvaluator` trait, `ToleranceBands`, `GateEnv`, `ToolpathLoadContext`, session-level chip-target maps, `effective_feed_for_sample` | 805 |
| tool_load/verdict.rs | All verdict/report types (`ChiploadVerdict`, `PowerVerdict`, `DeflectionVerdict`, `DepthVerdict`, `CriterionKind`, `UnmodeledReason`, `Confidence`, `ToolLoadReport`) | 3343 (prod ~2237) |
| tool_load/chipload.rs | Chipload gate | 2289 (prod ~1000) |
| tool_load/power.rs, deflection.rs, depth.rs, plunge_stress.rs | Other milling gates (`PowerTerms`, `PowerModelInputs` live in power.rs) | 1337 / 1114 / 574 / 201 |
| tool_load/drill_gates.rs | Chip welding, peck, plunge-feed gates | 587 |
| tool_load/locality.rs | `is_steady_state_for_gate`, `SpanLookup` | 531 |
| tool_load/boundary.rs | Inclusive-with-epsilon comparisons `exceeds_high`, `below_low` | 171 |
| tool_load/distribution.rs, display.rs, metric_guide.rs | Histograms, advance/tooth display map, hover copy | 724 / 107 / 165 |

### material/ (3,656 lines)
| File | Responsibility | Lines |
|---|---|---|
| material/mod.rs | `Material` + grade enums, `force_line()`, cutting speed, plunge, drill envelopes, picker merge, `to_key`/`from_key` | 2347 (prod ~1843) |
| material/force_line.rs | `ForceLine` (private fields), `ForceLineRefusal`, `WoodDensity`, `ChipRegime`, Curti 2021 / Goli 2018 / FPL constants | 1158 |
| material/wood_species_library.rs | Loader for `data/wood_species.toml` (132 `[[species]]` rows) | 111 |

## 2. Public entry points and external callers

Counts are `rg` matches in `crates/` excluding the defining folder; "tests" are `crates/*/tests/`.

| Entry point | Defined | External prod callers | Tests |
|---|---|---|---|
| `feeds::suggest::suggest_params` | feeds/suggest.rs:865 | 4 (rs_cam_cli/src/smoke.rs:495, rs_cam_viz/src/controller/events/{pins.rs:547,toolpath.rs:105}, rs_cam_viz/src/app/mcp/commands.rs:892) | 18 |
| `feeds::suggest::default_operation` | feeds/suggest.rs:889 | 2 (rs_cam_viz toolpath.rs:136, mcp/commands.rs:919) | 1 |
| `feeds::suggest::feeds_result_for_operation` | feeds/suggest.rs:995 | 6 (session/compute/export.rs:344; viz properties/{pills.rs,toolpath_panel.rs:515,feeds_speeds.rs:35}) | 14 |
| `feeds::suggest::feeds_preview_for_operation` | feeds/suggest/apply.rs:875 | 3 (viz controller/events/mod.rs, ui) | 16 |
| `feeds::suggest::apply` (the funnel) | feeds/suggest/apply.rs:821 | 1 (viz controller/events/mod.rs:759, orchestrated in viz, errors are `String`) | 6 |
| `apply_feeds_result_to_op`, `apply_speeds_to_op`, `apply_cut_geometry_to_op` | feeds/suggest/apply.rs:390,421,452 (all `pub`, re-exported feeds/suggest.rs:21-27) | 0 production callers; tests and `tests/literature_matrix/shim.rs:444` only | many |
| `resolve_operation_invariants` | feeds/suggest/apply.rs:856 | 0 anywhere outside feeds/suggest.rs re-export (zero test callers found) | 0 |
| `feeds::calculate` | feeds/mod.rs:1494 | 1 production (feeds/suggest.rs:1006) | many (feeds/tests.rs) |
| `feeds::embedded_vendor_lut()` / `EMBEDDED_LUT` | feeds/mod.rs:62,66 | 22 | 110 |
| `tool_load::evaluate_toolpath` | tool_load/mod.rs:540 | 6 (gcode/mod.rs:790 is the report path) | 24 |
| `tool_load::{chipload_envelopes,modulation_bands,chip_targets}_for_session` | tool_load/mod.rs:161,244,chip_targets_for_session | 9 (viz render, session/compute) | 6 |
| `tool_load::effective_feed_for_sample` | tool_load/mod.rs:71 | 5 | 2 |
| `tool_load::ToolLoadReport` (type) | tool_load/verdict.rs:512 | 61 mentions (viz UI, MCP, gcode, session) | 34 |
| `feeds::cutter_op_profile` (via `ProjectSession::cutter_op_profile`) | session/mod.rs:1689 | 15 | 14 |
| `feeds::power_at_operating_point`, `cut_efficiency` | feeds/operating_point.rs, efficiency.rs | 5, 7 | 28, 27 |
| `Material::force_line()` | material/mod.rs:935 | viz app/mcp/generation.rs:54, ui/feeds/compare.rs:787, ui/properties/stock.rs:113; in-area feeds x7 | many |
| `feeds::predict::{ENDMILL_EQUIVALENT_DIAMETER_FRACTION, tip_deflection_from_engagement}` | feeds/predict.rs:250,605 | tool/mod.rs:779,1368 (lower layer reads feeds) | |
| `feeds::ToolGeometryHint`, `OperationFamily`, `PassRole`, `CutterKind` | feeds/mod.rs | tool/*.rs, machine/mod.rs:140, compute/catalog/registry.rs:20 | |
| `Material::to_key` / `from_key` | material/mod.rs:1586,1692 | 0 production callers (section 9) | tests only |

Inbound file counts (non-self `src/` files naming the folder): feeds: session 13, tool_load 8, compute 8, tool 6, diagnostics 6, maps 3, dressup 2, material 1, machine 1, gcode 1.
tool_load: feeds 9, session 6, diagnostics 4, stock 3, trace 1, ops 1, machine 1, gcode 1, dressup 1, compute 1.
material: feeds 18, tool_load 6, compute 6, session 4, stock 3, ops 2, trace 1, tool 1, dressup 1.

## 3. Dependencies and layering

Outbound imports (`crate::X` mention counts): feeds uses feeds (self) 163, material 58, geo 27 (`BoundingBox3`, `P3`), compute::operation_configs 27,
compute::catalog 26, machine 18, compute::tool_config 16, tool 14, compute::cutter 11, tool_load 20+ (power, deflection, verdict, plunge_stress), ops::depth 2.
tool_load uses stock::simulation_cut 43, feeds 25+, tool 17, material 16, trace::toolpath_spans 14, compute 23, machine 16, ids 8, dressup::feed_modulation 8,
session 4, ops::{drill, drill_op, drill_metrics} 9, stock::cut_as 3, gcode::refusing_exceedances (doc ref). material uses only serde, std and feeds (1 doc ref).

Layering inversions and cycles:
| Finding | Evidence |
|---|---|
| feeds and tool_load depend on each other (module-level cycle). feeds to tool_load: `PowerTerms`/`PowerModelInputs`, `verdict::UnmodeledReason`, `verdict::ChipBoundsSource`, `deflection::EXCEEDS_BOUND_MM`, `plunge_stress`. tool_load to feeds: force, geometry, vendor_lut, vendor_lookup, vendor_normalize, predict | feeds/mod.rs:1395-1396, feeds/predict.rs:82, feeds/feed_explanation.rs:68, feeds/efficiency.rs:61, feeds/plunge.rs:27, feeds/operating_point.rs:61; tool_load/mod.rs:85, tool_load/chipload.rs:86 |
| The power model (`PowerTerms`) lives in tool_load but is the shared kernel of feeds Suggest; the deflection bound constant `EXCEEDS_BOUND_MM` lives in tool_load but feeds reads it | tool_load/power.rs, tool_load/deflection.rs:64 |
| Tool geometry vocabulary is defined in feeds and consumed by the lower `tool` layer: `MillingCutter::geometry_hint()` returns `feeds::ToolGeometryHint`; `tool/mod.rs` matches on it | tool/flat.rs:63, tool/vbit.rs:102, tool/mod.rs:332,681 |
| `machine` takes `feeds::OperationFamily` and `PassRole` | machine/mod.rs:140-144 |
| `ops/drill_metrics.rs` imports `tool_load::boundary`; `tool_load` imports `ops::drill_metrics`/`drill_op`/`drill` (cycle) | ops/drill_metrics.rs:353,418; tool_load/mod.rs:446,638; tool_load/drill_gates.rs:13-17 |
| feeds writes into `compute::catalog::OperationConfig` and reads `compute::operation_configs` (feeds sits above compute, but `compute/catalog/registry.rs` imports `feeds::support::MILLING_FORMULA_SOURCE`, `CutterKind`, `OperationFamily`, `PassRole`, so the registry depends back on feeds) | feeds/suggest/apply.rs:11-12; compute/catalog/registry.rs:20,50 |
| `CutterKind::tool_type()` returns `compute::ToolType` (feeds names a compute type) | feeds/mod.rs:291 |
| tool_load session functions take `&ProjectSession` (read-only, but gates are tied to the session layer) | tool_load/mod.rs:162,212,245 |
| Apply orchestration (resolve tool, machine, material; draft clone; command door) lives in `rs_cam_viz`, not core; MCP `apply_feeds` is a hand-written holdout parsing a scope string | rs_cam_viz/src/controller/events/mod.rs:687-760; rs_cam_viz/src/app/mcp/generation.rs:862-868; session/command.rs:52-56 |
| `dressup/feed_modulation.rs` re-exports `tool_load::BindingConstraint` and uses `feeds::force`, `tool_load::power` | dressup/feed_modulation.rs:64-69 |

## 4. Traits

| Trait | Defined | Implementors | Notes |
|---|---|---|---|
| `MetricEvaluator` (assoc `Verdict`; `evaluate_gate`, `status`) | tool_load/mod.rs:472 | `ChiploadGate` (:480), `PowerGate` (:492), `DeflectionGate` (:504) | "Internal organizational trait". Not implemented for `DepthGate` or the drill gates; `evaluate_toolpath` names each gate by hand, so the trait does not force a new gate to be registered |
| `Extrapolation` | feeds/extrapolation.rs:386 | `SizeLaw` only (feeds/extrapolation/size.rs:358) | One implementor. `hardness`, `family`, `drill`, `plunge` do not implement it (free functions plus a `*Basis` enum each) |
| `std::error::Error` | std | `FeedsError` (feeds/mod.rs:1100) | `ForceLineRefusal`, `DeflectionCapRefusal`, `PlungeRefusal`, `ForceLineRefusal` implement neither `Error` nor `Display` (they use `card_text`/`variant_id`) |
| `MillingCutter::geometry_hint` | tool/ (other folder) | tool/{flat,vbit,mod}.rs return the feeds type | see section 3 |

Traits from elsewhere: no use of `From`/`TryFrom` for the LUT enum pairs (hand-written `match` instead, section 9). No `Command`/`ProjectSession::apply` trait use inside the area. No `OpParams` derive in the area.

## 5. Recurring-pattern table

| Concern | Pattern used here | Example path:line | Enforcement | Deviations inside the area |
|---|---|---|---|---|
| Errors and refusals | A refusal is a typed value, not a number: `FeedsError{WrongToolForOperation,Unbacked}` (2 variants, `Cow` reason); `ForceLineRefusal` enum; `DeflectionCapRefusal`; `PlungeRefusal`; `FeedsSupport::Refuse`; gates return `Unmodeled(UnmodeledReason)`. `calculate` is infallible; refusal is a second call (`validate_tool_for_operation`). Production code has no `unwrap`/`panic`: the only `expect` is `wood_species_library.rs:72` under `#[allow(clippy::expect_used)]` | feeds/mod.rs:1012; material/force_line.rs:824; tool_load/verdict.rs:142 | TYPES (enums) + LINT (denied clippy `unwrap_used`/`expect_used`/`panic`) + SENTRY (`a_refused_deflection_is_not_a_zero_g_t4`, `suggest_refuses_what_the_registry_refuses_fm4`) | `vendor_lut::validate_observation` returns `Result<(), String>` (feeds/vendor_lut.rs:545); `UnmodeledReason::{CutterModeUnsupported,NotImplemented,NotApplicableForOp}(String)` carry free text (tool_load/verdict.rs:155-173); `Confidence::Approximate(String)` (tool_load/verdict.rs:226); `tapered_tip_floor_refusal` returns `Option<String>` (feeds/support.rs:165); `calculate` can be called without the refusal check (doc-only rule, feeds/mod.rs:1005-1010); apply orchestration returns `Result<Effects, String>` in viz (rs_cam_viz/src/controller/events/mod.rs:692) |
| State mutation | Core never owns session state here. Suggest funnel mutates a caller-supplied `&mut OperationConfig` and `&mut FeedsProvenance`; the GUI applies a DRAFT clone through `ReplaceToolpathConfig` (command door). `ApplicableRecommendation` can only come from a validated `FeedsPreview` | feeds/suggest/apply.rs:821-840; rs_cam_viz/src/controller/events/mod.rs:747-760 | TYPES for `apply` (private-by-construction recommendation); DOC for "the single write" | `apply_feeds_result_to_op`, `apply_speeds_to_op`, `apply_cut_geometry_to_op` are `pub`, take a raw `&FeedsResult`, skip the `ApplicableRecommendation` gate, and contradict the doc "every apply surface ... reaches an OperationConfig through here" (feeds/suggest/apply.rs:389-460, doc :800-812); `resolve_operation_invariants` mutates an op with no recommendation (:856); `apply_stock_defaults`/`apply_drill_defaults` mutate configs directly (:932,:996) |
| Parameters and config | Inputs are plain structs with `Option<f64>` overrides (`FeedsInput`, `SuggestContext`, `SuggestPolicy`); no `OpParams` derive here; ranges/clamps live in `suggest/invariants.rs`; machine dial is `MachineProfile::aggressiveness` | feeds/mod.rs:384; feeds/suggest.rs:94,111 | CONVENTION | `apply_stock_defaults` hard-codes `0.5`, `5.0`, `1.0` (feeds/suggest/apply.rs:1000-1030); `Material` per-family cutting speeds, plunge baselines typed as literals (`200.0`, `180.0`, `170.0`, `250.0`, `100.0`, `300.0`, `120.0`, `1000.0/h`, `900.0/h`, `1500.0`) with "placeholder" comments (material/mod.rs:962-1010); `default_engagement` and `operation_default_profile` ap/ae factors (`0.08`, `0.65`, ...) are bare literals (feeds/mod.rs:2822-2929). Operator rule: never type a number from memory |
| Units and frames | mm, mm/min, mm/tooth, kg/m3, radians in names (`_mm`, `_mm_min`, `_um`, `_deg`). Newtypes exist only for feed and advance-per-tooth quantities. Constructors are `pub const fn new`, so the newtype labels a value but does not validate it. No setup/stock/world frame types here: inputs are scalar depths | feeds/quantities.rs:71,92,114,157 | TYPES (weak: public `new`) | Almost all other APIs use bare `f64` with suffix naming: `FeedsInput` fields (`tool_diameter`, `flute_length`, no `_mm`) (feeds/mod.rs:385-389); `boundary::exceeds_high(observed, bound, tolerance)` takes three bare `f64` and can be transposed (tool_load/boundary.rs:102); `AdvancePerToothMm` adopted in only 5 files (feeds/mod.rs, efficiency.rs, tool_load/display.rs, chipload.rs, quantities.rs); `ADVANCE_PER_TOOTH` text const differs from `ADVANCE_PER_TOOTH_UNIT` (feeds/feed_explanation.rs:110 vs feeds/quantities.rs:67) |
| Identifiers | `ToolpathId` from `ids.rs` on verdicts; `OperationType`/`OperationConfig` from compute; LUT identity by `observation_id: String`; wood species by `(label: String, source_id: String)` pair; materials by serde enum | tool_load/verdict.rs:239; material/mod.rs:579 | TYPES (ToolpathId); CONVENTION (strings) | `SolidWoodByJanka{label,source_id}` is a stringly foreign key resolved at call time with `Option`/`NoDensity` fallback (material/mod.rs:942); `ProvenanceSource::{VendorLut(String),PublishedRule(String),RepoRule(String)}` (feeds/provenance.rs:65-105); stale doc says `toolpath_id` is a `usize` (tool_load/verdict.rs:232) |
| Background work, progress, cancellation | None. The area is synchronous and pure; evaluation is called by session compute/export on worker threads. No `interrupt`, no `rayon`, no threads inside | tool_load/mod.rs:540 | n/a | `evaluate_toolpath` has no cancellation hook; cost scales with sample count (loops over `trace.samples` per gate) |
| Caching, freshness, invalidation | Statics: `EMBEDDED_LUT` (`LazyLock`), `WOOD_SPECIES`, `MATERIALS_BY_CATEGORY`, two `GANTRY_PUSH_*` reasons. Freshness is not owned here: stale sim becomes `UnmodeledReason::StaleSimulation` set by gcode/mod.rs after the verdict is built. Config freshness uses `ToolpathConfig::generation_inputs_signature` (outside) | feeds/mod.rs:62; material/wood_species_library.rs:64; gcode/mod.rs:648-687 | CONVENTION | Verdict fields are rewritten after assembly in gcode/mod.rs (`*reason = StaleSimulation`), so "single assembly site" (tool_load/mod.rs doc :536) is not the only writer; `FeedsResult` has no input hash (staleness tracked by caller in viz `ToolpathEntry`) |
| Diagnostics, warnings, user text | Typed warning enums (`FeedsWarning` ~230 lines, `SuggestWarning`, `DeflectionCaveat`, `PlungeStressWarning`) plus card/hover text methods (`card_text`, `describe`, `card_lines`); refusal prose is `&'static str` consts. Adapters to the diagnostics bus live outside (`diagnostics/adapters/from_feeds.rs`, `from_tool_load.rs`) | feeds/mod.rs:776; feeds/support.rs:85-130; material/force_line.rs:939 | TYPES (enums) | Two overlapping warning enums (`FeedsWarning` mod.rs:776 vs `SuggestWarning` suggest.rs:211); `op_kind: &'static str` labels with a `_ => "finish_3d"` fallback (feeds/suggest/axial_envelope.rs:317-326); stale user-facing string "isotropic Kc, bending only" (tool_load/deflection.rs:298) and `Confidence::Approximate("isotropic Kc only")` examples (tool_load/verdict.rs:225); refusal texts embed numbers and dates in prose (feeds/support.rs:96-130) so they can drift from the rule tables |
| Serialization and wire types | serde derive on verdict/report types with `#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]`; `Material` serde enum is the project-file shape; LUT JSON deserialized with `ObservationFile`; feeds result types are NOT serde (MCP types mirror them in viz) | tool_load/verdict.rs:140-141; feeds/vendor_lut.rs:11-330 | CONVENTION | `Material::to_key`/`from_key` is a second hand-kept string serialization, silent default on unknown key (`_ => Material::default()`, material/mod.rs:1829); `VendorLut::embedded()` skips any file that fails to parse (`if let Ok`) (feeds/vendor_lut.rs:503-510); guarded by `test_embedded_strict_parse` (SENTRY) |
| Numeric tolerances | Named constants with a stated source: `BOUNDARY_EPSILON_REL = 8*EPSILON` (relative, one place); `EXACT_DIAMETER_TOLERANCE`; `HARDNESS_EXACT_TOL`; `SIZE_WINDOW_EDGE_TOLERANCE` | tool_load/boundary.rs:83; feeds/extrapolation/size.rs:52 | SENTRY (`chipload_boundary_g_chip_ulp`) for gates; CONVENTION elsewhere | Inline literals: `1.0e-6` x2 in axial_envelope (feeds/suggest/axial_envelope.rs:284,~296); `TRAVERSE_HELPS_EPS = 0.01` local const (feeds/mod.rs:2273); `UNIT_SHARE_EPS`/`TARGET_REL_EPS` = 1e-9 (feeds/suggest/aggressiveness.rs:47,52); `VBIT_ANGLE_TOLERANCE_DEG = 20.0` (feeds/vendor_lookup.rs:275); `MIN_AP_MM`/`MIN_AE_MM` (feeds/mod.rs:1460,1463); `WITHIN_BOUND_MM`/`EXCEEDS_BOUND_MM` (tool_load/deflection.rs:61-64). No shared epsilon module |
| Geometry primitives | Almost none. `geo::{BoundingBox3,P3}` only in geometry_class, predict, suggest. Tool shapes arrive as `ToolGeometryHint`/`CutterKind` (feeds) or `ToolConfig`/`build_cutter` (compute). Sample geometry arrives as `SimulationCutSample` | feeds/geometry_class.rs:19; tool_load/mod.rs:34 | TYPES | `CutterKind` (5) vs `ToolGeometryHint` (5, with payload) vs `vendor_lut::ToolFamily` (6, incl. `FacingBit`) vs `compute::ToolType` (5): four tool-shape enums with hand-written bijections (feeds/mod.rs:204,276,291), pinned by tests (`tool_type_cutter_kind_round_trips`, `cutter_kind_lut_family_map_is_pinned`) |
| Test style and sentries | Unit tests inline or in `tests.rs`; integration "sentry" tests in `crates/rs_cam_core/tests/` named by sentence plus ruling id (e.g. `the_dial_holds_the_load_and_never_cuts_the_feed_fm7`); golden CSV gate `feeds_matrix_instrument_fm1`; every sentry named in the three CLAUDE.md files exists in `crates/rs_cam_core/tests/` (checked by script) | crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs | SENTRY | `feeds/suggest/tests.rs` has a blanket `#![allow(` (feeds/suggest/tests.rs:4); test modules allow `unwrap_used/expect_used/panic` (11 sites, e.g. material/mod.rs:1844); `OperationType::Pocket` fixtures repeated ~40 times in power.rs/deflection.rs tests (no shared builder) |
| Naming and module layout | `mod.rs` large; flat sibling files named by physics (`force`, `predict`, `efficiency`) plus G-number rule files; `suggest/` has pass files | feeds/mod.rs:18-38 | CONVENTION | Two `plunge.rs` (feeds/plunge.rs and feeds/extrapolation/plunge.rs); `geometry.rs` vs `geometry_class.rs` vs `cutter_constraints.rs` overlap in name; `feeds/mod.rs` hosts the calculator (3091 lines) while `feeds/suggest.rs` is the "model"; `use` statements after code in tool_load/mod.rs:84-88 and feeds/mod.rs:70-71 |
| Registering a new feature | Enum variant plus exhaustive `match` for closed sets; static tables for rules (`FAMILY_RULES`, `DRILL_RULES`, `PLUNGE_RULES`); JSON files for LUT rows listed in `EMBEDDED_FILES`; op identity through the registry row (`feeds_family`, `feeds_formula_source`, `feeds_pass_role`, `tool_constraints`, `capabilities()`) | feeds/vendor_lut.rs:344; compute/catalog/registry.rs:48-63 | TYPES for enums; SENTRY (`every_cell_declares_its_feeds_support_fm0`, `lut_resolver_census_a6`, `test_embedded_strict_parse`) for tables | Wildcard arms hide new ops: `apply_stock_defaults` `_ => {}` (feeds/suggest/apply.rs:1019), `apply_drill_defaults` `_ => {}` (:966), `axial_envelope` `_ => {}` (feeds/suggest/axial_envelope.rs:352), `adaptive_plunge_entry_label` `_ => None` (feeds/suggest/adaptive_entry.rs:581), `support.rs` `_ => Clueless` (feeds/support.rs:350, fail-closed, good) |

## 6. Extension points

| New item | Files touched (what forces it) |
|---|---|
| New operation kind with feeds support | `compute/catalog/registry.rs` row: `feeds_family`, `feeds_pass_role`, `feeds_formula_source`, `tool_constraints`, `capabilities()` incl. `feeds_hints`, axial-envelope class, move-count model (compiler forces the struct fields). Then: `feeds/vendor_normalize.rs::lut_query_for` only if routing differs from the declared family (NOT forced); `feeds/support.rs::formula_backing` (unlisted cell falls to `Clueless` refuse, so safe default; sentry `every_cell_declares_its_feeds_support_fm0`); `feeds/suggest/apply.rs::apply_stock_defaults`/`apply_drill_defaults` (wildcard, NOT forced); `feeds/suggest/axial_envelope.rs:276` match (wildcard, NOT forced); `feeds/suggest/adaptive_entry.rs:575` (wildcard); `feeds/geometry_class.rs::classify` (match on `OperationType`); `feeds/INTEGRATION.md` table (doc only) |
| New `OperationFamily` variant | `feeds/mod.rs` enum; `feeds/vendor_normalize.rs:152 op_family_to_lut` (exhaustive, forced); `feeds/vendor_lut.rs` `LutOperationFamily` + `label()` (forced by the `op_family_to_lut` match); `feeds/mod.rs:2822 operation_default_profile` (check for wildcard); `gcode/mod.rs:745-755` private copy of the same map (exhaustive, forced); `tool_load/mod.rs:308` (uses shared fn). Data: LUT JSON with the new `operation_family` string |
| New tool shape (cutter kind) | `feeds/mod.rs` `ToolGeometryHint`, `CutterKind`, `cutter_kind()`, `lut_family()`, `tool_type()` (forced by exhaustive matches); `vendor_lut::ToolFamily`; `compute::ToolType`; `tool/*.rs` `geometry_hint`; `extrapolation/{family,plunge}.rs` rule keys; `vendor_normalize.rs:108 lut_query_by_cutter` (check wildcard); `feeds/plunge.rs:59 PlungeBasis::for_tool` returns `Option` (silent `None` for unlisted) |
| New vendor LUT source | Add JSON to `crates/rs_cam_core/data/vendor_lut/observations/`; add a tuple to `EMBEDDED_FILES` (feeds/vendor_lut.rs:344; NOT compiler-forced: an unlisted file is silently absent, there are 26 files and 26 entries today); add id to `data/vendor_lut/source_manifest.json`; `CREDITS.md`; run `feeds_matrix_instrument_fm1` with `RS_CAM_UPDATE_FEEDS_MATRIX=1` and re-bless the CSV in the same commit |
| New extrapolation rule (G-series) | New entry in `FAMILY_RULES` (feeds/extrapolation/family.rs:98) / `DRILL_RULES` (drill.rs:86) / `PLUNGE_RULES` (plunge.rs:254); a new rule KIND needs a new `*Basis` enum arm, `FeedsSupport` arm (feeds/support.rs:369, exhaustive matches forced in `card_text`), `ValueProvenance` source, rationale arm (feeds/rationale.rs), UI payload (explain_payload.rs). `Extrapolation` trait is implemented only by `SizeLaw`, so the other kinds do not share a trait |
| New Suggest pass | New file in `feeds/suggest/`; call site in `suggest.rs`/`apply.rs` order is by hand (pass numbers in comments: 0, 1, 2-6, 6b, 9, 10); `ApplyScope::Speeds` skips geometry passes by reading code order (G-SPEEDSAPPLY), NOT type-enforced |
| New `FeedsWarning`/`SuggestWarning` | Enum variant; `Display`/card text; `diagnostics/adapters/from_feeds.rs` match; viz `ui/feeds/why.rs` (27 mentions). Compiler forces exhaustive matches only where there is no wildcard |
| New gate (tool_load criterion) | `CriterionKind` variant (tool_load/verdict.rs:797; forced in `label()` verdict.rs:827, `unit` :838, `metric_guide.rs:109`, `distribution.rs:406,583`, `gcode/mod.rs:957`, `rs_cam_viz/src/ui/sim_op_list.rs:1306`); verdict enum + `as_criterion_status` in verdict.rs; new `tool_load/<gate>.rs`; field on `ToolpathLoadVerdict`; assembly in `evaluate_toolpath` (tool_load/mod.rs:540, by hand); `MetricEvaluator` impl (optional, not forced); must use `locality::is_steady_state_for_gate`, `effective_feed_for_sample`, `boundary::{exceeds_high,below_low}` (convention + sentries, not types) |
| New material or species | Material variant: `material/mod.rs` `Material` plus ~12 exhaustive matches (`force_line`, `label`, `category`, `base_cutting_speed_m_min`, `plunge_rate_base`, drill envelopes, `feed_scale_factor`, `to_key`, `from_key`, catalog); compiler forces these. Library species: row in `data/wood_species.toml` with `source_id` in `source_manifest.json` (sentry `wood_species_library_provenance`). Force line for a new material: new `ForceLine` constructor in `force_line.rs` and a new `ForceBasis` arm |
| New `Material` refusal | `ForceLineRefusal` variant plus `variant_id`, `card_text`, `card_lines` in `material/force_line.rs` (exhaustive, forced) |

## 7. CLAUDE.md drift

| File | Statement | Reality |
|---|---|---|
| feeds/CLAUDE.md | Sentry list (about 40 names), `wanaka_suggest_integration`, `feeds_matrix_instrument_fm1` | All exist under `crates/rs_cam_core/tests/` (verified by script). No missing sentries |
| feeds/CLAUDE.md | "The move-count model, the axial-envelope class and the feeds hints are registry capabilities (W2.4): read `capabilities()`, add no op `match`" | `apply_stock_defaults`, `apply_drill_defaults`, `axial_envelope.rs:276`, `adaptive_entry.rs:575`, `geometry_class::classify` still match on `OperationConfig`/`OperationType` directly (feeds/suggest/apply.rs:943,1000; feeds/suggest/axial_envelope.rs:276) |
| feeds/CLAUDE.md file map | Lists `plunge.rs`, `ramp.rs`, `extrapolation/*`, `suggest/*` | Missing from the map: `feeds/INTEGRATION.md` (74 lines, exists), none else missing. The map names `profile.rs` etc. correctly |
| feeds/INTEGRATION.md | GUI flow calls `rs_cam_core::feeds::calculate`; entry `crates/rs_cam_viz/src/ui/properties/mod.rs`; operation table maps `Face` to `Pocket`; "Current known gaps" list | `calculate` has one production caller (feeds/suggest.rs:1006); the GUI uses `suggest::feeds_preview_for_operation`/`apply`; the table duplicates the registry (which has 24 entries, none with `OperationFamily::Face`) and will drift |
| feeds/suggest/apply.rs doc | "**The single write.** Every apply surface ... reaches an `OperationConfig` through here" | Three more `pub` writers bypass `ApplicableRecommendation` (apply.rs:389-460); `resolve_operation_invariants` is dead |
| tool_load/CLAUDE.md | Invariant: all gates use `locality::is_steady_state_for_gate`; never a second copy | True today: only `locality.rs:212-219` reads `in_transit_span` in non-test code. Held by sentries and convention, not by types (`in_transit_span` is a `pub bool` on `SimulationCutSample`, stock/simulation_cut.rs:207) |
| tool_load/CLAUDE.md | "`evaluate_toolpath`: split populations ONLY here" and doc "the **single** `ToolpathLoadVerdict` assembly site" | `gcode/mod.rs:648-687,809` rewrites `UnmodeledReason` on built verdicts (`StaleSimulation`) |
| tool_load/verdict.rs:232 | "`toolpath_id` is the core `usize` index" | The field is `ToolpathId` (verdict.rs:239) |
| tool_load/CLAUDE.md, material/CLAUDE.md | "No scalar `Kc`" | Live prose and strings still say Kc: tool_load/deflection.rs:298 (user-facing detail text), tool_load/verdict.rs:39,225, feeds/mod.rs:12,2108, feeds/predict.rs:382,816, feeds/suggest.rs:278,643,feeds/rationale.rs:290, feeds/suggest/adaptive_entry.rs:446,483 |
| material/CLAUDE.md | "~130-row Janka library" | 132 rows in `data/wood_species.toml`; `Material::SolidWoodByJanka` doc says "~148 species" (material/mod.rs:556,767) |
| material/CLAUDE.md | Files: mod.rs, force_line.rs, wood_species_library.rs | Matches. mod.rs also owns `to_key`/`from_key` (not described) |
| All three | Reference `planning/...` paths | 17 distinct referenced planning files are deleted (purged 2026-09-17), e.g. `planning/review_2026-08-08/XVAC_CENSUS.md` (4 refs), `LUT_BOUNDARY_EVIDENCE.md` (2), `APPLY_CONTRACT_CENSUS.md`, `FEEDS_CENSUS.md`, `P2_PLUNGE_STRESS_GATE_RCA.md`, `architectural_refactor_2026-06-06_v2.md`. These are in code comments and docs, not in the CLAUDE.md files themselves |
| feeds/CLAUDE.md | "each [folder CLAUDE.md] is 40 lines or fewer" | feeds/CLAUDE.md is exactly 40 lines, but several lines are over 700 characters; the Sentries paragraph is one run-on line |

## 8. Navigability

| Item | Detail |
|---|---|
| `feeds/mod.rs` | 3091 lines in prod. `calculate` is one function of about 1330 lines (feeds/mod.rs:1494-2822) with 18 `let mut` locals. Mixed: core types, warnings, rubbing floor, spindle model, defaults, calculator. Tests are external (`feeds/tests.rs`, 1877) |
| `tool_load/verdict.rs` | 3343 lines (prod ~2237): every verdict type, report types, status projection and bound provenance in one file |
| `tool_load/chipload.rs` | 2289 lines (prod ~1000 plus about 1290 of tests inline) |
| `material/mod.rs` | 2347 lines: enums, picker catalogue (`catalog`, `materials_by_category`), serialization keys, per-material numbers, 500 lines of tests |
| `feeds/suggest/tests.rs` 2675 lines, `feeds/tests.rs` 1877, `feeds/vendor_lookup.rs` 1877 (tests from 1021), `feeds/ramp.rs` 1408, `feeds/rationale.rs` 1277 | Over or near 1500 |
| Misleading names | `feeds::calculate` is a preview and refusal-free (the validated path is `suggest`); `ToolLoadReport` name vs "gates" folder `tool_load`; `feeds/geometry.rs` (DOC scale, chip thinning) vs `tool` geometry; `feeds/profile.rs` (`CutterOpProfile`) vs `OperationFamily` "profile" in `operation_default_profile` (`DefaultProfile`); `EMBEDDED_LUT` and `embedded_vendor_lut()` both public for one instance; `LutOperationFamily` documented as "separate from feeds::OperationFamily for serde" but identical variants |
| Items in the wrong folder | `ToolGeometryHint`/`CutterKind` (tool-shape vocabulary) in feeds, consumed by `tool/` and `compute/`; `PowerTerms`/`PowerModelInputs` (shared physics) in tool_load/power.rs, consumed by feeds, dressup; `EXCEEDS_BOUND_MM` in tool_load/deflection.rs, consumed by feeds/efficiency.rs; `OperationFamily`/`PassRole` consumed by machine/ and compute/catalog; `effective_feed_for_sample` (needs `machine::kinematics`) in tool_load/mod.rs; `drill_rpm_envelope_for_diameter` and `milling_rpm_ceiling_for_diameter` (material-like constants) in feeds/mod.rs:1318,1364 rather than material/ |

## 9. Candidate debt leads (UNVERIFIED; "dead" claims rest on rg over `crates/` excluding tests)

| path:line | Lead |
|---|---|
| material/mod.rs:1586-1830 | `Material::to_key` and `from_key` (about 250 lines) have no production caller; project IO uses serde. Includes a legacy alias `southern_yellow_pine` (:1709) against the "no legacy" ruling and a silent default on unknown key (:1829). Candidate delete with its tests |
| feeds/suggest/apply.rs:856 | `resolve_operation_invariants` has no caller outside its re-export (optimizer culled 2026-10-13). Candidate delete |
| feeds/suggest/apply.rs:389-460 (re-exported feeds/suggest.rs:21-27) | `apply_feeds_result_to_op`, `apply_speeds_to_op`, `apply_cut_geometry_to_op` are `pub` with no production caller; they bypass the `ApplicableRecommendation` type gate. Candidate demote (tests need a `cfg(test)`/`pub(crate)` seam) |
| gcode/mod.rs:745-755, tool_load/mod.rs:310-312, feeds/vendor_normalize.rs:215-217 | `PassRole` to `LutPassRole` `match` repeated 3 times; `OperationFamily` to `LutOperationFamily` is hand-copied in gcode/mod.rs:745 despite `op_family_to_lut`. Add `From<PassRole> for LutPassRole` or merge the enum pairs |
| feeds/mod.rs:304-332 vs feeds/vendor_lut.rs:270-327 | `OperationFamily`/`LutOperationFamily` and `PassRole`/`LutPassRole` are identical variant sets kept in sync by hand |
| feeds/mod.rs:204-290, feeds/vendor_lut.rs:108 | Four tool-shape enums (`ToolGeometryHint`, `CutterKind`, `ToolFamily`, `compute::ToolType`) with hand bijections |
| feeds/mod.rs:304-322, feeds/mod.rs:2903-2911, feeds/vendor_normalize.rs:160 | `OperationFamily::Face` is produced by no registry row (registry has Adaptive 2, Contour 3, Drill 2, Parallel 4, Pocket 3, Scallop 3, Trace 7); only default profiles, maps and tests reference it. `data/vendor_lut/observations/amana_facing.json` rows are reachable only through tests (feeds/vendor_lut.rs:925-940) |
| feeds/geometry_class.rs:27-60 | `ShallowTerrain`, `SteepTerrain` are "not returned today"; the one consumer tests `MixedTerrain` only (feeds/suggest/adaptive_entry.rs:79) |
| feeds/mod.rs:1494-2822 | 1330-line `calculate` with local mutable state; hard to test per step and a source of subtle ordering rules ("Step 7" etc. cited in comments elsewhere) |
| feeds/mod.rs:1005-1010, 1125 | Refusal check is separate from `calculate` (`validate_tool_for_operation`): any new caller of `calculate` can skip it. One production caller today |
| feeds/vendor_lut.rs:503-510 | `embedded()` silently drops a file that fails to parse; protected only by a test. 26 files, 26 entries today |
| feeds/vendor_lut.rs:545 | `validate_observation -> Result<(), String>` (stringly error) |
| tool_load/verdict.rs:155-173, 226 | `UnmodeledReason` free-text variants and `Confidence::Approximate(String)` carry prose into the wire type; consumers cannot match on them |
| tool_load/deflection.rs:298 | User-facing detail still says "isotropic Kc" although no Kc exists (B6) |
| tool_load/verdict.rs:232 | Doc says `toolpath_id` is `usize`; it is `ToolpathId` |
| tool_load/boundary.rs:102,111 | `exceeds_high(observed, bound, tolerance)` three bare `f64`; transposition compiles |
| tool_load/mod.rs:472-510 | `MetricEvaluator` is implemented for 3 of 8 criteria and does not drive assembly; a new gate is not forced to implement it |
| tool_load/mod.rs:540-590 vs gcode/mod.rs:648-809 | Verdict mutated after the "single" assembly site |
| tool_load/mod.rs:71-93 | `use` items after code (mid-file imports at :84-88) |
| feeds/suggest/axial_envelope.rs:317-326 | `op_kind` string labels with `_ => "finish_3d"` fallback; `_ => {}` outer arm comment admits it is unreachable and "kept soft" (:350-352) |
| feeds/suggest/apply.rs:1000-1030 | `apply_stock_defaults` literals (`0.5`, `5.0`, `1.0`) not derived or sourced; wildcard hides new ops |
| material/mod.rs:962-1010, 995-1025 | Hand-typed cutting speeds and plunge baselines (placeholders) against the operator's "no number from memory" rule; compare with the cited vendor LUT |
| feeds/mod.rs:2822-2929 | `operation_default_profile`/`default_engagement` factor tables are bare literals |
| feeds/quantities.rs:71-157 | Newtypes have `pub const fn new` with no validation and are used in 5 files; most feed/chip values stay `f64` |
| feeds/feed_explanation.rs:110 vs feeds/quantities.rs:54-67 | Two spellings of the advance-per-tooth label (`"mm of linear advance per tooth"` vs `"mm/tooth"` and `COMMANDED_ADVANCE_PER_TOOTH`) |
| feeds/vendor_normalize.rs:210-212 | `material_to_lut(input.material)` called three times in one struct literal |
| feeds/support.rs:85-130 | Refusal prose embeds ratios (0.37x, 0.57x, 0.41x-0.48x), dates and ruling ids inside `&'static str`; not derived from the data they describe |
| feeds/suggest.rs:211 and feeds/mod.rs:776 | Two warning enums with overlapping meaning; `FeedsWarning` has 17 in-area uses and 27 in viz `ui/feeds/why.rs` |
| feeds/extrapolation.rs:386 | `Extrapolation` trait has a single implementor |
| feeds/extrapolation/plunge.rs vs feeds/plunge.rs | Same file name in two folders; both own plunge logic (rule table vs resolver) |
| material/wood_species_library.rs:70-72 | `expect` in a `LazyLock` initialiser (build-time data, guarded by a test; one non-test panic site in the whole area) |
| `planning/...` references in code comments | 17 distinct deleted planning files cited as evidence (section 7); retrieve with `git show planning-pre-purge-2026-09-17:<path>` (planning/CLAUDE.md); citations were not rewritten |
| feeds/mod.rs:62-68 | Both a `pub static EMBEDDED_LUT` and a getter; 22 external callers use the getter, so the static can be private |
| tool/mod.rs:779,1362-1368 | Lower layer `tool` calls `feeds::predict::tip_deflection_from_engagement` (layering inversion; moves with the deflection kernel) |
