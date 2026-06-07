# Architectural refactor — making Cutter × Operation × Metric explicit (v2)

**Version:** v2
**Date:** 2026-06-06
**Status:** Architecture sketch, revised after an 8-area adversarially-verified investigation wave over the v1 sketch (`architectural_refactor_2026-06-06.md`, original). This is **not** an implementation spec; it is a scaffold for the implementation wave, with every v1 factual claim re-checked against the live working tree.
**Companion:** `planning/cutter_axial_constraints_2026-06-06.md` (Phase 0). That doc carries its own rev-2 review fixes that this v2 folds in (see Phase 0).

> **Working-tree caveat — RESOLVED 2026-06-07.** This plan was synthesized while a concurrent agent was editing the working tree. That flux has landed: commits `14fd789` (planning docs), `d928fed` (viz/CLI/session surface), and `7d01311` (cutter-axial-constraints envelope + Findings 1-3 + combined-Suggest v3 backbone: `feeds/cutter_constraints.rs`, `predict.rs`, `explain.rs`, `rationale.rs`, `geometry_class.rs`, `tests/wanaka_suggest_integration.rs`, LUT `ap_*_factor` migration). **Phase 0 is DONE** (tracker below). Line numbers in this doc predate those commits — **re-verify with `rg` before editing.**

---

## 0. Status tracker

> **Protocol:** the implementation orchestrator updates this table (and ONLY this table — the body sections stay as decided) after every landed PR. States: `TODO` / `IN PROGRESS (who)` / `BLOCKED (on what)` / `DONE (commit)`. A phase is DONE only when its §9-Q9 parity gate passed. Add rows for discovered work; never delete rows — mark them `DROPPED (why)`.

| # | Work item | Phase | Status | Gate / evidence |
|---|---|---|---|---|
| T0 | Axial-constraint envelope (companion doc rev 3, items 1-3: LUT schema+migration, calculator, Suggest consumers) | 0 | **DONE (`7d01311`, 2026-06-07)** | `cutter_constraints.rs` w/ binary-search `invert_deflection` over `tip_deflection_from_engagement` + roundtrip tests; `feeds_family` routing (no `is_finish_3d` invented); `apply_axial_envelope` policy C; 4 warning variants; chipload-bounds re-derivation landed; LUT `ap_*_factor` migrated |
| T1 | PRE-Phase-1 feeds-hints coverage net (guard `suggest.rs` `(None,None,None)` wildcard) | pre-1 | **DONE (`39eec99`, 2026-06-07)** | `operation_feeds_hints_is_an_explicit_per_op_decision` (feeds/suggest.rs tests): exhaustive no-wildcard `OperationConfig` match over all 23 ops — new op fails to compile until hints are a recorded decision. core suite 1766 passed |
| T2 | §7.2 serde/schema parity freeze test set | pre-1 | **DONE (`39eec99`, 2026-06-07)** | All 5 surfaces: `operation_config_serde_shape_is_kind_params` + `operation_type_serde_repr_pinned` (catalog.rs); `tool_type_serde_repr_pinned` (tool_config.rs); `project_tool_section_type_key_shape_frozen` ×2 (core session/project_file.rs lenient-String + viz io/project.rs serde-direct); `build_info_published_capability_flags_frozen` + `mcp_parse_helpers_accept_all_canonical_names` (rs_cam_mcp); partition test strengthened to exact `ALL_2D ∪ ALL_3D ∪ {AlignmentPinDrill} == ALL`. Suites: core 1766 / viz 189 / mcp 2 pass; clippy `-D warnings` + fmt clean. Note: `3de69b3` restored the fmt gate (7d01311 landed unformatted code) |
| T3 | Phase 1 data-only `OpRegistryEntry` (all 23 ops, one PR) + kill `tool_constraints_for_type` wildcard | 1 | **DONE (`18f931f`+`7051171`, 2026-06-07)** | `OpRegistryEntry` table (spec+params+constraints per op) behind exhaustive `registry_entry()` (no discriminant indexing — §12.4 trap avoided); BOTH wildcards killed: `tool_constraints_for_type` `_ => (Vec::new(), true)` → explicit `ToolConstraintsDef::ANY_TOOL` per entry (`tool_constraints_are_an_explicit_per_op_decision` pins 4 restricted + 19 unchanged); `operation_feeds_hints` `_ => (None,None,None)` → exhaustive `OperationConfig::feeds_hints()` (tuple adapter kept, T1 net green). Public fns + serde shapes unchanged (T2 freezes + workspace check green). core 1767 pass, clippy+fmt clean |
| T4 | Phase 1 drill-family predicate consolidation (`is_plunge_only()`, ≥4 open-coded sites) | 1 | **DONE (`e013a38`, 2026-06-07)** | Canonical predicate is the pre-existing `OperationType::is_drill_kinematics`; gates' `is_plunge_only_op` deleted, chipload/power/deflection + optimizer skip + narrate `is_drill_cycle` migrated. optimize/axes.rs NotOptimizable assert list deliberately kept open-coded (it IS the agreement audit). `drill_kinematics_set_is_pinned` pins membership. core 1768 pass + F-036b sentry green |
| T5 | Phase 1 dressup/entry-style policy registry field (replaces `normalize_for_op` 5 predicates + viz dup table) | 1 | **DONE (`6cce2ca`, 2026-06-07)** | `OpRegistryEntry.dressup_policy` (`strip_all(reason)`/`FORCE_NO_ENTRY`/`PREFER_HELIX`/`ANY_DRESSUP`, explicit per entry); `normalize_for_op` + viz panel both read it (UI tooltip string comes from the registry); `dressup_policy_table_is_pinned` + `normalize_for_op_applies_registry_policy` pin table+semantics. "Optimizable axes" descriptor NOT folded in — `OptimizationSurface` (G16) is already an exhaustive no-wildcard classifier; `has_doc_knob`/`four_variant` deferred to T12 scope check. core 1770 + viz 189 pass |
| T6 | Phase 2 `for_each_op!` X-macro (pure lists: enum, ALL, op_type, new_default, as_params, category) | 2 | **DONE (`c0764a0`+`df2aa64`, 2026-06-07)** | One `(Variant, ConfigType, Category)` list generates: `OperationType` decl (serde attrs preserved, T2 repr freezes green), `ALL`, new `const fn category() → OpCategory{Menu2d,Menu3d,SystemOnly}`, and the 4 `OperationConfig` dispatch fns. `ALL_2D`/`ALL_3D` hand-written + order-sensitive sync test vs category tokens. Parity: param_sweep `--ignored` 54/54 pass, 326/329 fingerprints byte-identical; the 3 diffs are pencil@175° nondeterminism on an IDENTICAL build (see T14). NOT generated per plan: spec bodies, ParamDef arrays, OperationConfig, execute arms |
| T14 | (discovered, T6 parity run) Pencil generator is nondeterministic at degenerate bitangency angles — same build, two runs: 7404 vs 7350 moves, 200 vs 177 z-levels at `bitangency_angle=175.0`. Undermines the fingerprint oracle for pencil; suspect rayon order-dependent accumulation or hash iteration in bitangency detection | follow-up | TODO | reproduce with two same-build `sweep_pencil --ignored` runs; fix = deterministic ordering; gate = repeated runs byte-identical |
| T7 | Phase 3 `CutterKind` + dedup `ToolGeometryHint→ToolFamily` maps + fold predict.rs raw-ToolType matches | 3 | **DONE (`5fc0e7b`+`e162203`+`89c810f`, 2026-06-07)** | PR A: `CutterKind` in feeds/mod.rs (fieldless shape class; doc'd vs ToolGeometryHint + GeometryClass), primary derivation `ToolGeometryHint::cutter_kind()`, `ToolType::cutter_kind()` layered convenience; LUT-family map was TRIPLICATED (chipload `tool_family_for` + vendor_normalize inline + lookup_parity test shim) → single `CutterKind::lut_family()`; FacingBit pinned LUT-data-only; ToolType↔CutterKind bijection + serde_token pins. PR B: predict.rs V-bit refusal + `core_diameter_mm` routed on CutterKind. PR C: `ToolConstraintsDef.required_kinds: &[CutterKind]` (schema strings byte-identical via to_schema), execute Scallop refusal reads registry `allows()` — refusal/schema can't drift; VCarve/Inlay/Chamfer deliberately NOT given a new runtime refusal. Gates: core full suite + scallop-refuses-flat litmatrix 7/7 + viz 189 / mcp 2 / cli pass, workspace clippy `-D warnings`, fmt clean |
| T15 | (discovered, T7 gate run) `drill_metrics_pr2::drill_session_oversize_peck_trips_peck_adequacy_gate` stale since `5c052cf` (2026-06-03 Janka-banding raised softwood per-peck D/d 2.0→6.0; in-module tests updated, this integration test missed — Ø4/15mm hole caps per-peck at 3.75, can never trip 6.0). Fails at HEAD independent of refactor. CLAUDE.md drill-threshold table ("softwood 2.0") was stale for the same reason. ALSO unmasked (suite aborted at this binary pre-fix, doctests never ran): 2 suggest.rs doctests failing on `×` in unfenced pseudo-math blocks | follow-up | **DONE (`bab5e87`+`df062a1`, 2026-06-07)** | test re-baselined to Ø2/13mm peck (D/d 6.5 > 6.0, both summary + gate assertions); CLAUDE.md per-peck AND chip-welding rows corrected to banded values (6/5/4, 8/6/5); suggest.rs formula blocks fenced ```text; core suite incl. doctests green |
| T8 | Phase 3 tool-type parser unification (4 parsers → core `parse_lenient`, warn-and-default) | 3 | **DONE (`af99374`, 2026-06-07)** | `ToolType::parse_lenient` = case-insensitive union vocabulary (canonical + core `ballnose`-style + viz-legacy `ball`/`tapered_ball`/`flat` — "flat" verified as the legacy writer's EndMill token, 978dc9c), `None` on unknown. Q4 policy: core loader tracing-warn+default; viz typed loader switched ToolType field→lenient String (kills the unknown-token→whole-file-fails→legacy-cascade failure mode) + viz legacy loader both emit new `ProjectLoadWarning::UnknownToolType`; **MCP deliberate carve-out: keeps explicit `Err` on unknown (live `add_tool` mutations — error beats surprise end mill) but accepts the unified vocabulary**. Pins: `parse_lenient_vocabulary_is_pinned`, session `tool_type_parsing` re-baselined (+aliases), viz type-key freeze re-baselined (lenient+warning shape), MCP helper test +alias acceptance. Gates: core/viz 189/mcp suites, workspace clippy `-D warnings`, fmt |
| T9 | CLI cull: retire job.rs router, migrate sweep onto `OperationConfig`, generic registry-driven subcommand | post-1, pre-5 | **DONE (`b9d64e3`+`921ba68`+`55c61b4`, 2026-06-07)** | Caller check: no script consumers (stress-test agents use `cargo test --test param_sweep`). PR 1: generic `run <op> --set k=v` (registry ParamDef drives `--list-ops`/`--list-params`; params via the GUI/MCP `set_toolpath_param` round-trip; execution via session→`execute_operation_annotated`; core gained `LoadedModel::from_file`); smoke: pocket/SVG + drop_cutter/STL → valid gcode. PR 2: job.rs 6-op router retired — `OperationDef`→`OperationConfig` thin mapping, per-op defaults preserved verbatim (incl. adaptive3d stock-top=model+5, helix 0.8r→factor 0.4, hybrid cleanup), JobFile format + OpResult/JobResult shapes unchanged; **sweep verified live mid-cutover** (baseline+2 variants, fingerprint diffs correct). PR 3: 13 per-op subcommands + dead helpers culled (−3,437 lines); surviving surface version/job/run/sweep/project/smoke/nc-time = ONE execution path; FEATURE_CATALOG + VALIDATION_PLAN updated. Gates: cli suites, core suite (from_file additive), workspace clippy `-D warnings`, fmt |
| T10 | Phase 4 `CutterOpProfile` aggregation + two-caller `SuggestContext` dedup | 4 | TODO | `wanaka_suggest_integration.rs` is THE gate; preserve `enforce_invariants` pass order |
| T11 | Phase 5 behavior adapters (one family at a time; drill-conversion dedup; cancellation regression test first) | 5 | TODO | param_sweep fingerprints + end_to_end + family sentries |
| T12 | Phase 6 (6A): `MetricContext` into gates, collapse two assembly sites, `criteria()` slice, gating derives from criteria() | 6 | TODO | tool_load sentries + MCP serde tests |
| T13 | narrate.rs: label-string routing → typed `feeds_pass_role` (before any Phase-1/2 label change) | consumer | **DONE (`3c0ff1f`, 2026-06-07)** | Routed on a new `ToolpathNarrationContext.operation_kind: Option<OperationType>` (typed op match, not `feeds_pass_role` — preserves the exact pre-existing op groupings incl. curve-family hints that role routing would have changed). Labels demoted to display-only. Strings copied verbatim → output parity by construction; both production constructors populate the kind. core 1770 + viz 189 pass |

**Sequencing constraints:** T1+T2 before T3. T3 before T6 (macro needs frozen registry shape). T3 before T9 (generic CLI needs ParamDef table). T9 before T11 (2 production drivers, not 3, during cutover). T13 before any change to `OperationSpec.label`. T4/T5 ride with T3 or immediately after.

---

## 1. Purpose

`rs_cam` now has **23 operations** (`OperationType::ALL`, asserted `== 23` at `compute/catalog.rs:1592`), **5 cutter families** (`ToolType`: EndMill, BallNose, BullNose, VBit, TaperedBallNose at `compute/tool_config.rs:10-15`), and several tool-load / feeds metrics. The interactions form a real dispatch matrix:

```text
OperationConfig × Tool/Cutter geometry × Material/Machine × Metric/Gate
```

A lot of this matrix is currently represented by scattered enum matches and small per-site helper tables. **The v1 sketch's central correction holds and is reinforced by the investigation: Rust already gives a lot of help here.** Most per-site matches are exhaustive-without-wildcard (compile error on a new op/cutter). The real problem is that one conceptual matrix is split across ~12 parallel 23-arm enumerations in `catalog.rs` plus a small number of genuinely permissive wildcards. The goal is to make the matrix explicit, dedup routing data, and convert the few silent wildcards into named policy — **not** to replace all enums with traits.

This revised sketch is intentionally conservative:

- keep public serde/TOML and MCP shapes stable unless a phase explicitly opts into a schema change;
- avoid pretending `macro_rules!` can append enum variants from separate macro calls (confirmed empirically — see §3.4);
- avoid `enum_dispatch` designs with incompatible associated `Config` types;
- separate pre-sim profiles from post-sim gate verdicts;
- start from the existing architecture (`OperationType::spec`, `OperationConfig`, `OperationParams`, `MillingCutter`, `ToolpathLoadVerdict`).

### 1.1 Endpoint definition — "miss nothing, or don't compile" (added 2026-06-06)

The refactor is done when adding **operation #24** is: one row in `for_each_op!` → the enum variant breaks every exhaustive match (generator dispatch, GUI param editor), and the registry entry **literal** forces an explicit answer for everything that is a silent wildcard today — tool constraints (kills `catalog.rs:1392`), feeds hints (kills `suggest.rs:723`), dressup/entry policy (§2.1 row 20), optimizable axes (row 21), plunge-only/kinematic class (row 14), menu category. Sites the compiler cannot reach (viz menus, MCP error strings, `narrate.rs`, the CLI) either go **generic** (read the registry / serde / typed `feeds_pass_role`) or carry a partition/sync test that fails loudly. The same standard applies to cutter #6 (one `CutterKind` row + closed conversions) and metric #4 (one typed field + inclusion in `criteria()`). Every `_ =>` that survives must be a **named policy** with a test asserting its membership.

---

## 2. Current workflow audit — what is enforced vs what is scattered

### 2.1 Add a new operation today (e.g. hypothetical `Trochoidal`)

| # | Site | File | Today | Coverage signal (verified) |
|---|---|---|---|---|
| 1 | `OperationType` variant + `ALL`/`ALL_2D`/`ALL_3D` | `compute/catalog.rs:108,136,162,176` | manual enum + 3 const lists | Tests assert `ALL.len()==23` (catalog.rs:1592) and partition disjoint/subset (catalog.rs:1601) BUT deliberately allow `system_only>0`, so an op left out of **both** menu lists is silently "system-only" and never appears in GUI menus — no test failure. |
| 2 | `OperationType::spec()` | `compute/catalog.rs:190-446` | exhaustive 23-arm match, **no wildcard** | Compile error on new variant. 9-field `OperationSpec` literal per arm. |
| 3 | `OperationConfig` variant | `compute/catalog.rs:603-629` | serde-tagged enum `{kind, params}` | Compile-time for exhaustive matches. |
| 4 | Config struct + `Default` | `compute/operation_configs.rs` | 23 hand-written structs + Default | Compiler for struct; no semantic coverage. Highly heterogeneous fields. |
| 5 | `OperationParams` impl | `compute/operation_configs.rs:918-1640` | 23 manual trait impls | Optional accessors (stepover/depth_per_pass/scallop_height) silently default to `None` — real semantic-drift risk. |
| 6 | Param schema | `compute/catalog.rs:1092-1384 param_defs_for_type` | match over inline `const FACE: &[ParamDef]` arrays, **no wildcard** | Compile-time. But serde-`skip` runtime fields (e.g. `ProjectCurveConfig.setup_z_flipped`, operation_configs.rs:893) are absent from the schema and not caught by the existing schema-parity test. |
| 7 | Tool constraints | `compute/catalog.rs:1386-1398 tool_constraints_for_type` | **WILDCARD `_ => (Vec::new(), true)` at catalog.rs:1392** | **CORRECTED from v1.** Only VCarve/Inlay/Chamfer/Scallop are explicit; the other **19 of 23 ops silently inherit (no required tool, supports v_bit)**. A new op gets permissive constraints with no compile error. (Rows 2 and 6 use the same "if no wildcard" wording but are genuinely wildcard-free — the contrast is the point.) |
| 8 | Generator dispatch | `compute/execute.rs:242-1275 execute_operation_annotated` | single 23-arm match, **no wildcard** | Strongest existing net: new variant is a compile error. Only **2 production drivers** (session/compute.rs:729, viz worker/execute/mod.rs:132); a third in-core caller is the spans-discarding wrapper `execute_operation` (execute.rs:216) with only test callers. |
| 9 | Drill/aux op data, spans, semantic trace | `compute/execute.rs` | per-arm span helper + annotate fn + `build_drill_op_for_config` (execute.rs:76, own `_ => None`) | Not compiler-enforced that the RIGHT span/annotate helper is used. **DrillCycleType→DrillCycle conversion is TRIPLICATED**: `cfg.cycle.to_core(cfg)` (execute.rs:122), inline (execute.rs:135-148), and the AlignmentPinDrill arm (execute.rs:732-745). |
| 10 | Feeds hints | `feeds/suggest.rs:710-725 operation_feeds_hints` | **WILDCARD `_ => (None, None, None)` at suggest.rs:723** | Only 6 ops handled (Scallop/DropCutter/Waterline/SteepShallow/VCarve/RampFinish); the other **17 silently get no hints**. The single genuinely silent fallback in the feeds layer. Exercised incidentally by literature_matrix/shim.rs but with NO exhaustiveness guard. |
| 11 | Suggest invariant passes | `feeds/suggest.rs:833-869 enforce_invariants` | fixed generic pipeline; per-op behavior gated inside passes by `feeds_family == Adaptive` or `let OperationConfig::Adaptive3d(cfg) = ... else { return }` | A new family-specific need is not compiler-flagged. Pass ORDER is documented as "must mirror the historical monolithic implementation exactly." |
| 12 | Predictions | `feeds/predict.rs` | mixed | **CORRECTED from v1 (undersold).** `arc_fit_ratio_for_op` (predict.rs:680-744) is **fully exhaustive, no wildcard** — the STRONGEST compile-time guarantee in the feeds layer. `predict_move_count` (predict.rs:454-528) by contrast has TWO wildcards (`_ => {}` at :466, `_ => return 0` at :528). `core_diameter_mm` (predict.rs:364-378) and the V-bit special-case (predict.rs:158) are raw `ToolType` matches — Phase 3 absorption targets. |
| 13 | LUT routing exceptions | `tool_load/chipload.rs:716-737 routed_lookup_family` | Adaptive3d→Pocket + ProjectCurve family special-cases; `None` for ProjectCurve+BullNose/ChamferVbit/FacingBit (chipload.rs:735) | The `None`-return is a real silent "no LUT" case. |
| 14 | Gate not-applicable predicate | `tool_load/power.rs:330-335 is_plunge_only_op` | matches Drill\|AlignmentPinDrill; shared by chipload.rs:260 + power.rs:92 | **CORRECTED from v1 ("shared helper") — materially understated.** The identical `Drill \| AlignmentPinDrill` predicate is OPEN-CODED (bypassing the helper) at `tool_load/optimize/mod.rs:147-152`, `tool_load/optimize/axes.rs:324`, and `session/compute.rs:1706-1710` (sets narrate's `is_drill_cycle`); `catalog.rs:503/:542` group the pair with DropCutter for transform/air-cut, and `feeds/geometry_class.rs:79` groups them with VCarve\|ProjectCurve\|Trace\|Inlay. ≥4 independent re-spellings of the same drill-family routing — the highest-value Phase-1 consolidation target. A new plunge-only op silently misroutes through 4+ sites with no compile error. |
| 15a | **GUI param editor** | `rs_cam_viz ui/properties/mod.rs:2901-2945` | **EXHAUSTIVE 23-arm match, no wildcard** | **CORRECTED from v1.** A new `OperationConfig` variant FAILS TO COMPILE here — it IS compiler-enforced, not "manual." |
| 15b | **GUI add-toolpath menus** | viz `project_tree.rs:379`, `toolpath_panel.rs:535` | iterate hand-written `ALL_2D`/`ALL_3D` consts | Silent miss (see row 1). |
| 15c | **GUI diagram / dressup-compat / tool-param** | viz `properties/mod.rs:3005` (`_ => {}`), `:3547` (`_ => None`), `properties/tool.rs:194` (`_ => {}`) | wildcards | New op/cutter silently gets no diagram, no incompatibility warning, no bespoke editor. The dressup-compat table duplicates core `DressupConfig::normalize_for_op` (compute/config.rs:464). |
| 16 | **MCP schema/params** | core round-trip + 2 error strings | **fully generic.** get/set route through core serde round-trip (session/compute.rs:148-388 tagged `{kind,params}`) and core registry methods (catalog.rs:927-980); a new op needs ZERO MCP code | The only manual bits are two hardcoded valid-type error strings (rs_cam_mcp server.rs:448 ops, :454 tools) that drift. |
| 17 | **Project IO** | core + viz loaders | operation block is serde-automatic in both; **but** tool-type = three/four divergent EndMill-fallback parsers (see §2.2 row 2) + viz legacy op-name parser (io/project.rs:1287, `_ => Pocket`) **already missing `alignment_pin_drill`** | Both loaders now **default-with-warn** for a missing operation block (core project_file.rs:900 + `tracing::warn` :904; viz io/project.rs:1114 silently). The v1 memory "core drops / viz defaults" is **STALE** — do not propagate it. |
| 18 | Tests / sweeps / lit matrix | `crates/rs_cam_core/tests/` | manual | Discipline, not type-system. |
| 19 | **Geometry-class classifier (NEW)** | `feeds/geometry_class.rs:71-92 classify()` | **exhaustive OperationType match, no wildcard** | NEW/uncommitted. Classifies TERRAIN (model geometry), NOT cutter. A Phase-2 macro-gen target. **Not the same axis as the proposed `CutterKind`.** |
| 20 | **Dressup/entry-style eligibility (ADDED post-critique)** | `compute/config.rs:421-531 DressupConfig::normalize_for_op` | **FIVE separate per-op `matches!` predicates**: strip_entry = ProjectCurve\|DropCutter (config.rs:464), force_no_entry = Drill\|Trace (:509), prefer_helix = Adaptive (:514), dedicated Adaptive3d entry-strip (:524) | A new op silently inherits the role-default entry style — no compile error, no test forcing a decision. Documented dual-maintenance with the viz op×dressup table (config.rs:539 ↔ properties/mod.rs:3540). The dressup IR side is CLEAN (dressup.rs has zero OperationType coupling), so this is a pure pre-IR routing concern the registry CAN own — ideal Phase-1 field (`dressup_policy`/`compatible_entry_styles`). |
| 21 | **Optimizer axis routing (ADDED post-critique)** | `tool_load/optimize/candidate.rs:96 has_doc_knob`, `bounds.rs:351 four_variant`, `bounds.rs:316 resolve_scallop_height_bounds`, `grid.rs:85` | silent opt-in `matches!` bools (has_doc_knob lists 10 ops; four_variant = Pocket\|Adaptive) | A new op added to `optimization_surface` as Optimizable but forgotten in `has_doc_knob` silently never searches its DOC axis. Contrast: `OperationConfig::optimization_surface` (catalog.rs:657) IS exhaustive no-wildcard (doc comment at :632 notes it). Motivates a registry "optimizable axes" descriptor. |
| 22 | **narrate.rs label-string routing (ADDED post-critique)** | `narrate.rs:942-951, :1074` | routes deflection-context text on `operation_label` STRING literals (`Some("3D Finish")`, `Some("Drill")`…, `_ =>` fallback); carries the duplicated `is_drill_cycle` (set at session/compute.rs:1706) | `narrate_toolpath` is the PRIMARY agent diagnostic surface (per CLAUDE.md) yet routes on display strings: renaming an `OperationSpec.label` in Phase 1/2 silently breaks narrate's DOC-context messaging — no compile error, no test. Prefer routing on typed `feeds_pass_role`/depth-semantics. |
| 23 | **CLI per-op subcommands (ADDED post-decision)** | `rs_cam_cli/src/main.rs:2016-3348` (`Commands::DropCutter`/`Pocket`/`Profile`/`Adaptive`/`Vcarve`/`Rest`/`Adaptive3d`/`Waterline`/`RampFinish`/`SteepShallow`/`Inlay`/`Pencil`/`Scallop`…) + `cli/job.rs:387-797` 6-op router | **~14 hand-rolled per-op clap subcommands** + a parallel legacy execution path | A second full layer of per-op plumbing beyond `job.rs`: op #24 needs a hand-written subcommand or it is silently absent from the CLI. The registry's `&'static [ParamDef]` already holds exactly the schema a generic `run <op> --set key=value` subcommand needs — see §7.1 (DECIDED: cull). |

**Wildcard inventory (the genuinely silent op-routing defaults to convert to named policy):**

1. `tool_constraints_for_type` `_ => (Vec::new(), true)` — `catalog.rs:1392` (19/23 ops).
2. `operation_feeds_hints` `_ => (None,None,None)` — `feeds/suggest.rs:723` (17/23 ops).
3. `predict_move_count` `_ => return 0` — `feeds/predict.rs:528` (silent zero-move prediction).
4. GUI secondary matches (diagram/dressup/tool-param) — viz, UI-only.
5. viz legacy op-name parser `_ => Pocket` — `io/project.rs:1318` (already missing `alignment_pin_drill`).

### 2.2 Add a new cutter geometry today (e.g. future `FacingCutter`)

| # | Site | File | Today | Coverage signal (verified) |
|---|---|---|---|---|
| 1 | `ToolType` variant/defaults | `compute/tool_config.rs:10` | public serde enum (5 variants) | Compile-time for matches; project parsing has fallback. |
| 2 | Tool project IO parse | **THREE/FOUR divergent parsers** | (a) core `project_file.rs:461` (`ball_nose\|ballnose`…, `_ => EndMill`, returns `ToolType`); (b) viz legacy `io/project.rs:882 restore_legacy_tool` (DIVERGENT vocabulary `ball`/`tapered_ball`, `_ => EndMill`); (c) MCP `server.rs:452` (serde snake_case, **returns `Err`** on unknown); (d) viz serde-direct default `io/project.rs:1383` | **CORRECTED from v1** ("parse_tool_type, viz IO"). The vocabularies are mutually inconsistent — `ball` parses to BallNose in viz but EndMill in core. All silent-fallback except MCP. |
| 3 | `MillingCutter` impl | `tool/*.rs` | trait impl | Compiler-enforced methods. |
| 4 | Build `ToolDefinition` | `compute/cutter.rs:9-43 build_cutter` | exhaustive 5-arm match on `ToolType`, no wildcard | Compile-time. **Closed enum — external cutters bypass this.** |
| 5 | `ToolGeometryHint` | `feeds/mod.rs:43-59` | public enum (5 variants), exhaustive where matched | The universal cutter signal: every cutter yields one via `MillingCutter::geometry_hint()` + `ToolDefinition::to_geometry_hint()` (tool/mod.rs:397). **Distinct from `geometry_class.rs::GeometryClass` (terrain).** |
| 6 | LUT `ToolFamily` | `feeds/vendor_lut.rs:80-89` | serde enum, **6 variants** | `FacingBit` is an **orphan** — no `ToolType`/cutter produces it; exists only as LUT row data. |
| 7 | Tool family mapping | `tool_load/chipload.rs:739-747 tool_family_for` + `feeds/vendor_normalize.rs:12-17` | **DUPLICATED identical 5-arm `ToolGeometryHint → ToolFamily` map** | A concrete dedup target for Phase 3 CutterKind. |
| 8 | Feeds geometry math | `ToolGeometryHint::engaged_diameter_at_doc` (**feeds/mod.rs:103-142**, method, sig `(self, axial_doc_mm, tool_diameter_mm, shank_diameter_mm)`) | exhaustive 5-arm match | TaperedBall/VBit are nonlinear in DOC; Flat/Ball/Bull return constant nominal. (Ball near-tip shrink lives in `width_at_height`/`engagement_radius`, not here.) |
| 9 | Cutter chip/deflection profile | `tool/*`, `tool_load/*` | trait methods + gate helpers | Mixed. |
| 10 | GUI tool editor | viz `properties/tool.rs:69` (ToolType::ALL combo), `:154-194` (`_ => {}` type-specific params) | manual | A new cutter with bespoke params renders no editor silently. |
| 11 | Vendor LUT rows | `data/vendor_lut/observations` | data | lit-matrix/tests. |

The cutter side is healthier because `MillingCutter` is a behavior trait and `build_cutter` is exhaustive. The risky bits: the divergent IO parsers, the `FacingBit` orphan family, the duplicated family maps, and the scattered conversion paths (there is NO direct `ToolType → ToolGeometryHint`/`ToolFamily` helper — everything routes through `build_cutter` first).

### 2.3 Add a new tool-load metric today (e.g. `SurfaceFinishRa`)

| # | Site | File | Today | Coverage signal (verified) |
|---|---|---|---|---|
| 1 | Gate module | `tool_load/*.rs` | implementation | local tests |
| 2 | Verdict type(s) | `tool_load/verdict.rs:666-943` | typed enums (ChiploadVerdict/PowerVerdict/DeflectionVerdict) | compiler after field added |
| 3 | `ToolpathLoadVerdict` field | `tool_load/verdict.rs:193-215` | fixed serde shape: chipload/power/deflection always present + `drill_gates`/`modulation_summary` Option + serde-default | additive Option field flows through serde/MCP for free |
| 4 | **Evaluate in report builders — TWO SITES** | `tool_load/mod.rs:366 evaluate_toolpath` AND `gcode/mod.rs:371 project_load_report` | **two independent assemblies**, each calling all three gates inline | **CORRECTED from v1** (one site). They have **already diverged**: `modulation_summary` is wired in gcode (gcode/mod.rs:477) but hardcoded `None` in mod.rs:425. `project_load_report` also does `sim_is_stale` rewriting (gcode/mod.rs:486-491) that `evaluate_toolpath` does not. |
| 5 | Optimizer consumers | `tool_load/optimize/*` (~13 files) + `strategy/` + `retarget/` | read gate-specific typed numeric fields | The dominant 6B cost center (see §6 Phase 6). |
| 6 | MCP serialization | `app/mcp.rs:2471` (`serde_json::to_value(&report)`, pure passthrough) + `rs_cam_viz mcp_server.rs` hardwired typed-shape description string | additive fields free; schema break is hard | no mapping layer to absorb a shape change |
| 7 | GUI display | viz `sim_timeline.rs:1611-1658`, `preflight.rs:486-527`, `sim_diagnostics.rs:929+` | exhaustive typed-verdict matches | not core-enforced |
| 8 | Rationale / Suggest (pre-sim) | `feeds/rationale.rs:136`, `feeds/suggest.rs` | exhaustive `SuggestWarning` match | **VERIFIED: the plan's §4 claim is correct** — adding a `SuggestWarning` variant forces rationale coverage (compile error). |
| 9 | Tests / lit-matrix | tests/data | manual | discipline |

`criteria()`/`modeled_count()`/`any_exceeded()` (verdict.rs:219-258) **hardcode exactly 3 gates**; `criteria()` returns `[CriterionStatus; 3]`. A 4th milling gate changes array arity (`[_;3]→[_;4]`), breaking the consumer `for status in verdict.criteria()` (sim_op_list.rs:938). `drill_gates`/`modulation_summary` are deliberately excluded from `criteria()` (documented verdict.rs:198-204).

Metric refactoring has a hard fork (1 = typed, 2 = dynamic) — **DECIDED 6A**, see §6 Phase 6.

---

## 3. Refined conceptual model

### 3.1 Keep the existing axes, add explicit registries/profiles around them

The current axes are already real:

- `OperationType` + `OperationConfig` + `OperationParams`
- `ToolType` + `ToolConfig` + `ToolDefinition`/`MillingCutter`
- typed verdicts + `ToolpathLoadVerdict`

Make their metadata/routing explicit **before** trying to replace them.

```text
                   ┌─────────────────────────┐
                   │ Operation registry       │
                   │ - OperationType          │
                   │ - OperationSpec          │
                   │ - Param schema (data)    │
                   │ - tool constraints (data)│
                   │ - feeds hints (fn accr)  │
                   └───────────┬─────────────┘
                               │
┌─────────────────┐     ┌──────▼──────┐      ┌────────────────────┐
│ Cutter profile  │     │ Preflight   │      │ Metric evaluators  │
│ - ToolType      │────▶│ profile     │◀────▶│ - chipload         │
│ - CutterKind    │     │ (pre-sim)   │      │ - power            │
│ - geometry math │     │ feeds +     │      │ - deflection       │
│ - compatibility │     │ constraints │      │ - drill_gates      │
└─────────────────┘     └─────────────┘      └────────────────────┘
                               │
                               ▼
                    existing OperationConfig + generators
```

**What the registry CAN feed consumers** (verified): viz `ALL_2D`/`ALL_3D` menu lists → registry 2D/3D classification; MCP/GUI param schema → already core-derived, formalize as registry read; op×dressup-entry compatibility (properties/mod.rs:3540, duplicating compute/config.rs:464) → a registry compatibility field. **What it CANNOT:** the CLI `job.rs` 6-op router (needs a rewrite to the `ProjectSession` path, not a registry read) and bespoke per-op/per-cutter UI widgets (inherently GUI).

### 3.2 `CutterOpProfile` is pre-sim, not a replacement for sim gates

**CORRECTED from v1.** The v1 sketch referenced a `RefusalReason` type that **does not exist anywhere in the codebase**; the real feasibility type is `FeedsError` (feeds/mod.rs:459), produced by `validate_tool_for_operation` (mod.rs:501) and already surfaced through `suggest_for_operation -> Result<SuggestedParams, FeedsError>` (suggest.rs:463). The other sketch fields already have working homes carrying richer payloads than the sketch showed.

> **Naming foot-gun (post-critique):** a near-namesake **already exists and is load-bearing** — `tool_load/mod.rs:100 pub enum RefuseReason`, a serde-tagged public enum whose docstring (mod.rs:96-98) says it is "a superset of the old suggest module's refusal kinds plus optimizer-specific ones." The refusal taxonomy was already unified once and is now split between `RefuseReason` (optimizer, serde-public, ~10 variants incl. SimulationRequired/NoVendorData) and `FeedsError` (feeds). **Never introduce a third type named `RefusalReason`** — one letter off from `RefuseReason`. Pre-sim feasibility reuses `FeedsError`; the profile must not shadow `RefuseReason`.

```rust
pub struct CutterOpProfile<'a> {
    pub tool_cfg: &'a ToolConfig,
    pub tool_def: &'a ToolDefinition,
    pub operation: &'a OperationConfig,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,

    pub cutter_kind: CutterKind,         // Phase 3, net-new (derived from ToolGeometryHint)
    pub op_type: OperationType,
    pub spec: OperationSpec,

    pub feasibility: Result<(), FeedsError>,   // CORRECTED: was RefusalReason (nonexistent)
    pub feeds: Option<FeedsResult>,            // EXISTS: feeds/mod.rs:304
    pub constraints: ConstraintEnvelopes,      // NET-NEW (Phase 0 axial output)
    pub predictions: Predictions,              // NET-NEW aggregate over the 3 predict.rs predictors
    pub warnings: Vec<SuggestWarning>,         // EXISTS: feeds/suggest.rs:167 (11 variants)
}
```

The **only net-new types** are `Predictions` (an aggregate over the existing `DeflectionPrediction` / `ObservedChiploadPrediction` / move-count predictors in `predict.rs`) and `ConstraintEnvelopes` (the Phase-0 axial output). `CutterKind` is net-new (Phase 3). Everything else reuses shipped types.

Boundaries:

- **Can own:** tool/op compatibility, feeds result, pre-sim axial/radial envelopes, move-count estimate, closed-form predictions, Suggest warnings.
- **Must not own:** final `ToolpathLoadVerdict` from simulation trace.
- **Can feed into gates:** shared context (routed LUT family, compatibility result, expected kinematic class). **Leave `doc_derating_scale` (feeds/geometry.rs:138) exactly where it is** — it is already the single canonical home, re-exported into `tool_load::chipload` (chipload.rs:97) and applied in both the calculator (feeds/mod.rs:727) and the post-sim gate (chipload.rs:398), sentried by feeds/mod.rs:2451. The profile **reads** `ChiploadBounds`; it does not re-derive derating.

This makes Phase 4 a **context-aggregation + consumer-dedup pass**, not a behavioral change (see Phase 4 — much of it already shipped).

### 3.3 Avoid the associated-type trap for operations

The v1 sketch's `trait ToolpathOp { type Config; ... }` + `#[enum_dispatch]` does not fit when every variant has a different `Config` associated type (verified: each `OperationConfig` variant wraps a structurally different config; `as_params` (catalog.rs:805) already does `&dyn OperationParams` forwarding).

**DECIDED — Option A/B hybrid (data-only Phase 1):**

```rust
pub struct OpRegistryEntry {
    pub kind: OperationType,
    pub spec: OperationSpec,
    pub params: &'static [ParamDef],
    pub tool_constraints: ToolConstraints,   // explicit per-op — kills the catalog.rs:1392 wildcard
    pub feeds_hints: fn(&OperationConfig) -> FeedsHints,  // fn accessor, NOT static data
    // DEFERRED to Phase 5 behind a parity gate:
    // pub compatibility: fn(CutterKind) -> Result<(), FeedsError>,
    // pub generate: GenerateFn,
}
```

Phase 1 holds **data only** (`spec`, `&'static [ParamDef]`, `ToolConstraints`). `feeds_hints` must stay a `fn(&OperationConfig) -> FeedsHints` accessor because `operation_feeds_hints` projects per-op config fields. `generate`/`compatibility` fn-pointers are **deferred to Phase 5** — a `GenerateFn` would re-match the variant internally (the dispatch it's trying to remove) and must preserve pipeline-set serde-skip fields like `setup_z_flipped` (consumed at execute.rs:1256, mutated at two production sites: session/compute.rs:707 and viz compute.rs:119).

### 3.4 Use an X-macro, not append-style `define_op!`

A `macro_rules!` invocation in one module cannot append a variant to an enum declared elsewhere. **Empirically confirmed** (prototype at `/tmp/rscam_xmacro_proto`, compiling, 4/4 tests): a token-muncher cannot build a filtered `const ALL_2D: &[_]` across list iterations.

The realistic shape is a single authoritative list carrying **inline literals** (NOT per-op functions — `catalog.rs` has no `face_spec`/`face_params`/`face_generate`; `spec()` is an inline struct-literal match and `param_defs` are inline `const FACE: &[ParamDef]` arrays):

```rust
macro_rules! for_each_op {
    ($m:ident) => {
        //     variant,  config_ty,    category,  spec-literal-or-ref, param-array-ref
        $m!(Face,         FaceConfig,   Cat2d,     /* OperationSpec{..} */, FACE);
        $m!(Adaptive3d,   Adaptive3dConfig, Cat3d, /* ... */,              ADAPTIVE3D);
        $m!(Drill,        DrillConfig,  Cat2d,     /* ... */,              DRILL);
        $m!(AlignmentPinDrill, AlignmentPinDrillConfig, CatSys, /*..*/,    ALIGN_PIN);
        // ... all 23 ops ...
    };
}
```

Derive from that one list, **Phase 2, pure-list/metadata only**:

- `OperationType` enum decl
- `OperationType::ALL`
- `op_type()`, `new_default()`, `as_params`/`as_params_mut`
- a generated `const fn category(self) -> OpCategory`

**Do NOT macro-generate** `spec()` bodies, `param_defs` arrays, `OperationConfig`, or `execute.rs` arms in Phase 2.

**Category split (the v1 "hard part") — resolved:** carry a 3-way category token (`Cat2d`/`Cat3d`/`CatSys`) per row, generate `category()`, **KEEP the existing `&'static [OperationType]` `ALL_2D`/`ALL_3D` consts hand-written** (catalog.rs:162,176), and add a sync test asserting `ALL_2D == filter(Cat2d)`, `ALL_3D == filter(Cat3d)`, and the three partitions cover `ALL` exactly once. A naive two-way `ALL_2D ∪ ALL_3D == ALL` assertion would FAIL because `AlignmentPinDrill` is system-only (in `ALL`, in neither sublist).

Scope honesty: the macro's "stay in sync" win is **catalog.rs-local metadata only**. ~16+ cross-file per-op match sites (incl. viz/MCP in other crates: execute.rs, properties/mod.rs, app/mcp.rs, predict.rs, geometry_class.rs, suggest.rs, power.rs, optimize/axes.rs) cannot be driven by a core-private list; they still rely on exhaustive-match compile errors.

Error ergonomics (prototype-verified): missing spec field → E0063, bad category token → E0599, typo'd fn → E0425, duplicate variant → E0428 — all point at the **list-entry line**. A duplicate cascades into secondary E0004/unreachable_patterns warnings, so migrate one surface per PR.

The macro body must use fully-qualified `$crate::...` paths (no glob imports) because `wildcard_imports = deny` is set (Cargo.toml:58).

---

## 4. What the type system can and cannot enforce

### Strong compile-time wins (verified realistic)

| Change | Enforcement mechanism |
|---|---|
| Add op to central list | generated `OperationType`/`ALL`/`op_type`/`new_default` stay in sync (catalog.rs-local) |
| Add op but forget generator | `execute_operation_annotated` exhaustive match (execute.rs:242) fails to compile |
| Add cutter kind | exhaustive `CutterKind` matches fail where no wildcard |
| Add new trait method | compiler breaks every impl |
| Add Suggest warning | **VERIFIED** — `feeds/rationale.rs:136` exhaustive match forces rationale coverage |
| Add typed metric field | report builders/tests fail until field initialized (both assembly sites) |

### Things Rust will not prove

- generator algorithm correctness; schema description semantic meaning; GUI rendering completeness; lit-matrix coverage; dynamic-registry MCP back-compat; that a `_ => default` arm is safe.

### Design rule

Prefer **one central exhaustive match/table** over many small ones. When a fallback is necessary, encode it as a **named policy**, not `_ => default`. Target wildcards: `catalog.rs:1392`, `suggest.rs:723`, `predict.rs:528` (see §2.1 wildcard inventory).

---

## 5. Tooling recommendations

### 5.1 Use now / likely good

- **In-tree X-macros (`macro_rules!`)** — best first tool, no dependency, prototype-proven for the pure-list derivations.
- **Sealed traits only for new internal traits** — do NOT seal public `MillingCutter` (zero external implementors, but sealing is churn). Add internal traits like `MetricEvaluator` only where external extension is unsupported.
- **Existing exhaustive matches + tests** — preserve while moving data sources into one registry.

### 5.2 Dependency verdicts (DECIDED — prototype-backed, see §9 Q7)

- **`strum`** — **REJECT.** The X-macro reproduces `ALL`/`EnumIter`/`EnumCount` with zero deps and gives a free `category()` strum can't.
- **`enum_dispatch`** — **REJECT** for operations (associated-Config trap; fn-pointer registry suffices).
- **`bon`** — **DEFER** to an optional Phase-4 decision; a plain struct/constructor suffices.
- All three confirmed **absent** from every manifest today.

### 5.3 Avoid for now

- proc macros for Phase 1/2; build-script codegen; `#[non_exhaustive]` on internal matrix enums; macro-wrapping generator math.

---

## 6. Phased refactor plan

Each phase produces small PRs and parity tests (per-phase gates in §9 Q9; **NEVER workspace-wide `cargo test` — it loops on this repo**). LOC deltas are rough; do not promise net reductions until a prototype measures it.

### Phase 0 — Axial constraint envelope (before structural refactor)

Ship the companion `cutter_axial_constraints_2026-06-06.md` work first, **with its rev-2 review fixes** (the companion doc has internal contradictions the implementer must resolve):

- **Reuse existing `ap_min_mm`/`ap_max_mm`** (vendor_lut.rs:180-181, vendor_lookup.rs:49-50) — do NOT add `ap_max_absolute_mm`. Drop the residual `ap_max_absolute_mm` references in companion §3.2/§8.3. The `ap_min_factor`/`ap_max_factor` fields are **already in the tree** (vendor_lut.rs:188,192) — verify migration coverage rather than "add new field."
- **Deflection bounds:** build from explicit axial/radial inputs + `ToolDefinition::tip_deflection_mm` (tool/mod.rs:416). **Do NOT invert `predict_peak_deflection_um`** — it refuses V-bits (predict.rs:158→0) and returns 0 without `depth_per_pass` (predict.rs:194). **Fix the companion's internal contradiction:** §2.1 docstring and §8.1 test still say "invert `predict_peak_deflection_um`" — rewrite both to use `tip_deflection_mm` binary search.
- **Binary search for nonlinear cutter profiles** (V-bit/tapered-ball `engaged_diameter_at_doc` is nonlinear; Flat/Ball/Bull constant).
- **`tip_deflection_from_engagement` extraction risk RAISED above "low":** the shared primitive is only the trailing `force → tip_deflection_mm` step of `sample_tip_deflection_mm` (deflection.rs:101-103); the gate derives force from `SimulationCutSample` arc/radial fields the pre-sim caller does not have and must synthesize. Not a clean 1:1 extraction.
- **Finish-op mutation: warning-first** (see §9 Q5). DropCutter has NO `stock_to_leave` field (operation_configs.rs:426-445); Adaptive3d uses `stock_to_leave_radial`/`_axial`. Mutate only Adaptive3d DPP + VCarve `max_depth`; ProjectCurve `depth` warn-only.
- **Add a finish-3D classifier task:** `op.is_finish_3d()` **does not exist** (companion §5.1/§5.3 assume it). Either add `OperationType::is_finish_3d()` as a new exhaustive match (couples Phase 0 to Phase 3's centralize-classifiers goal) or route on `OperationSpec.feeds_family` (catalog.rs:60). The `enforce_invariants` "first pass to mutate DPP" premise (companion §5.5) is **false** — `clamp_dpp_to_rigidity`/`clamp_dpp_to_cutting_length`/`backoff_dpp_for_deflection` already mutate DPP (suggest.rs:850-852) before chipload recalibration (suggest.rs:865); place the bounds re-derivation accordingly.
- **Fix companion citations:** `engaged_diameter_at_doc` → feeds/mod.rs:103 (not tool/mod.rs:197, which is `lookup_diameter_at`); `doc_derating_scale` → geometry.rs:138 (not :117); `predict_peak_deflection_um` → predict.rs:125 (not :455).

**Phase 0 freezes** the `cutter_axial_constraints(&ToolDefinition, &Material, &MachineProfile, &VendorLut, scalars)` input signature and the `CutterAxialConstraints` shape. It does **NOT** define `ConstraintEnvelopes` or `CutterOpProfile::for_combo` (Phase 4). The v1 "pressure-test the profile/constraint API" is therefore **partial** — Phase 0 validates the envelope-builder + warning/rationale plumbing, not the profile aggregation type.

Outcome: real-world pressure test for the envelope/constraint API and the warning-first plumbing.

### Phase 1 — Operation registry from existing data

Goal: one place to ask "what is this op?" without changing generation. **Metadata-only across all 23 ops in one PR** (see §9 Q8).

Tasks:

1. Introduce `OpRegistryEntry` (data-only: `spec` + `&'static [ParamDef]` + `ToolConstraints`) and `FeedsHints` types. `feeds_hints` is a `fn(&OperationConfig)` accessor, NOT static data.
2. Bridge `OperationType::spec`, `param_defs_for_type`, `tool_constraints_for_type`, `operation_feeds_hints` behind registry accessors. **Make tool constraints an explicit per-op field — eliminate the `catalog.rs:1392` wildcard.** Add a test asserting the 19 currently-defaulted ops keep `(empty required, v_bit=true)` to prove no behavior change.
3. Keep existing public functions and serde shape intact.
4. **EXTEND** the existing parity tests (catalog.rs:1591 `operation_catalog_is_exhaustive_and_consistent` — asserts `new_default(op).op_type()==op`; session/compute.rs:2715 `operation_schema_params_match_params_with_nulls_for_every_op` — asserts schema field names == serialized default param keys) to additionally assert: one registry entry per `OperationType::ALL`, no entry uses a placeholder tool-constraint unintentionally. **Do NOT re-implement these two — they already exist.**
5. **(post-critique) Consolidate the drill-family predicate:** make `is_plunge_only()` an `OperationType`/registry method and migrate the ≥4 open-coded re-spellings (power.rs:333, optimize/mod.rs:149, optimize/axes.rs:324, session/compute.rs:1706) onto it; add a test that helper and sites agree. Highest-value single consolidation in the wave.
6. **(post-critique) Add a dressup/entry-style policy field** to the registry entry so `DressupConfig::normalize_for_op`'s five per-op predicates (config.rs:464/509/514/524) and the duplicated viz op×dressup table (properties/mod.rs:3540) read from one source. Consider also an "optimizable axes" descriptor so `has_doc_knob` (candidate.rs:96) / `four_variant` (bounds.rs:351) derive from the registry instead of silent opt-in `matches!`.

**PRE-Phase-1 PREREQUISITE (must land BEFORE consolidation):** the "every-op-has-feeds-hints / no-unintended-`(None,None,None)`-fallback" net **does not exist today** (suggest.rs:723 is unguarded, exercised only incidentally by literature_matrix/shim.rs). A no-behavior-change consolidation needs an independent baseline; build this net first.

Expected risk: **low-medium**. Mostly metadata consolidation. (Note: the `ap_*_factor` fields are already partly landed — coordinate with the concurrent agent; treat `feeds_hints` as the LAST data field migrated, after suggest.rs stabilizes.)

### Phase 2 — Central operation X-macro

Goal: remove duplicated operation lists/counts.

Tasks:

1. Add `for_each_op!` central list (inline literals, per §3.4).
2. Generate `OperationType`, `ALL`, `op_type()`, `new_default()`, `as_params`/`as_params_mut`, and `category()`. **KEEP `ALL_2D`/`ALL_3D` consts hand-written**; add the 3-way partition sync test.
3. Migrate one generated surface per PR; keep diffs reviewable.
4. **Do NOT** generate `OperationConfig` (serde-attribute regression risk), `spec()` bodies, `param_defs` arrays, or `execute.rs` arms.

Expected risk: **medium**. Macro mistakes create noisy diffs and cascading errors — split into mechanical PRs.

### Phase 3 — Cutter classification cleanup

Goal: make cutter kind and compatibility explicit without breaking public cutter APIs.

Tasks:

1. Introduce internal **`CutterKind`** as the canonical cutter-shape classifier. **Name it unambiguously and document that it is NOT `feeds/geometry_class.rs::GeometryClass` (an OperationType-keyed TERRAIN classifier) and NOT `ToolGeometryHint`** — three similarly-named classifiers in `feeds/` will confuse contributors otherwise.
2. **Derive `CutterKind` from `ToolGeometryHint`** (the universal signal every `MillingCutter` yields), with `ToolType → CutterKind` as a convenience layered on top — NOT the primary path — so externally-implemented cutters still classify. **Centralize the duplicated `ToolGeometryHint → ToolFamily` map** (chipload.rs:739 + vendor_normalize.rs:12) onto `CutterKind`.
3. Move `tool_family_for` and compatibility checks toward `CutterKind`. **Reword "V-bit angle routing":** it is LUT-row angle matching in `feeds/vendor_lookup.rs:130` (`angle_bonus`, keyed on `included_angle_deg` defined at vendor_lut.rs:160-167), which STAYS in the LUT lookup; `CutterKind` carries the angle as data but does not absorb the matching. **Reconcile `FacingBit`** (orphan `ToolFamily`, vendor_lut.rs:88, no `ToolType`/cutter): decide whether it is a `CutterKind` at all — any compatibility matrix assuming a `ToolType ↔ family` bijection is wrong.
4. **Subsume the per-op allowed-tool-type string lists** (`tool_constraints_for_type`, catalog.rs:1391, e.g. `Scallop => ['ball_nose','tapered_ball_nose']`) into a `CutterKind` compatibility matrix, reconciled with the runtime refusal at execute.rs:1045. Note `routed_lookup_family` (chipload.rs:716) is an additional op×cutter routing site the hub should feed.
5. **Fold the NEW `feeds/predict.rs` raw-`ToolType` matches** (V-bit special-case predict.rs:158, `core_diameter_mm` predict.rs:364-378) onto `CutterKind`/`ToolGeometryHint` before they ossify into permanent duplication.
6. **Unify the parsers + Q4:** consolidate the three/four divergent tool-type parsers (core project_file.rs:461, viz legacy io/project.rs:882, MCP server.rs:452, viz serde-direct io/project.rs:1383) into ONE core `parse_lenient`; **warn-and-default-to-EndMill** (thread into viz `LoadedProject.warnings`), not silent fallback and not hard-fail. **PARITY TRIPWIRE:** session/mod.rs:1441 asserts `parse_tool_type("unknown")==EndMill` — update deliberately.
7. Do not seal `MillingCutter` yet (see §9 Q3).

Expected risk: **low-medium** (but +parser-unification round-trip tests across all four mechanisms before changing policy).

### Phase 4 — Preflight `CutterOpProfile` (≈70% already shipped — aggregation + dedup, NOT greenfield)

**Reframed from v1.** `predict.rs` (3 predictors + breakdown structs), `explain.rs` (`FeedsExplain`), `rationale.rs` (`SuggestRationale`, exhaustive warning match), and `suggest.rs` (`SuggestContext`/`enforce_invariants`/`SuggestPolicy`/`Scope`/`Aggressiveness`) are NEW-but-present and consumed across the GUI feeds-modal, GUI controller (controller/events/mod.rs:839), MCP rationale surface, and core suggest pipeline. The hard parts (physics, calibration, rationale serialization, consumer wiring) already shipped.

Tasks:

1. Introduce a thin `CutterOpProfile` that **aggregates already-shipped outputs** behind one `for_combo` constructor that internally calls `feeds::suggest::feeds_explain_for_operation` (suggest.rs:564 — NOT explain.rs) + `suggest_for_operation`. The only net-new types are `Predictions` and `ConstraintEnvelopes` (Phase 0).
2. **De-duplicate the structurally-identical `SuggestContext`-assembly** currently in `feeds_modal.rs::compute_suggest_rationale` (460-505) and `app/mcp.rs::mcp_get_suggest_rationale` (979-1039) — the 5-line `SuggestContext{model_bbox, stock, ..default}` block is the genuinely identical part (they differ in path-qualification and error handling). **Preserve the exact context each caller builds** (model_bbox + stock + default policy) — a profile that auto-populates policy/scope differently would silently change live Suggest output.
3. **Leave `doc_derating_scale` untouched** (already canonical, §3.2). **Exclude the diagnostics `from_feeds` adapter** (`diagnostics_from_feeds_result(tp_id, &FeedsResult)` from_feeds.rs:22, `heuristic_hints_from_recommendation` :162) from migration — it consumes `FeedsResult` + raw op params, not the full profile; migrating it would add unused predictions (a behavior change). Drop it from the v1 "diagnostics context builders" target or scope it to the `FeedsResult` slot only.
4. Keep simulation gate evaluation separate.
5. **Preserve `enforce_invariants` pass ORDER** (suggest.rs:841-868, "must mirror the historical monolithic implementation exactly") — any aggregator re-running predictors out of order diverges from GUI/MCP/CLI Suggest. Do NOT "clean up" the calibrated `predict.rs` constants (ENDMILL_CORE_FRACTION 0.7, the +36% safe-side bias) — they are tuned against the post-sim deflection integrator (suggest.rs:746-755).

Expected risk: **medium** (calibration-coupling, not LOC). `wanaka_suggest_integration.rs` (loads real wanaka.toml, exhaustive `SuggestWarning` catch-all at 323-334) is THE parity gate, backed by the in-crate `feeds/suggest.rs` test module + literature_matrix harness.

### Phase 5 — Operation behavior adapters

Goal: reduce `execute_operation_annotated` weight without an all-at-once rewrite. **One family at a time** (see §9 Q8).

Tasks:

1. For one operation family, create a `GenerateFn` adapter (existing `ExecutionContext` + `&OperationConfig` → current generator). The adapter must reproduce the per-arm **span helper** (`generated_with_depth_run_spans` vs `_cut_run_spans` vs `_drill_spans` vs `spans_from_adaptive3d_annotations` vs `spans_from_labeled_events`) AND the per-op annotate fn — getting these wrong produces a toolpath that simulates but has wrong/empty spans (NOT caught by the compiler, only by span-aware sim tests).
2. **Unify the drill family:** `build_drill_op_for_config` (execute.rs:76) + the DrillCycleType→DrillCycle conversion triplicated at execute.rs:122/135-148/732-745.
3. Move adapters into registry entries one family at a time. **Keep the exhaustive match as the fallback** while adapters are proven family-by-family — there are only 2 production drivers (session/compute.rs:729, viz worker/execute/mod.rs:132), so the blast radius is small.
4. Preserve `setup_z_flipped` caller-side pre-dispatch mutation (two sites: session/compute.rs:707, viz compute.rs:119).
5. Retain exhaustive checks so missing adapters fail loudly.
6. **(post-critique) Cancellation asymmetry:** only FOUR of 23 arms cooperatively poll `cancel: &AtomicBool` — Adaptive (execute.rs:428), DropCutter (:784/791), Adaptive3d (:962/967), Waterline (:991/999); the other ~19 fast generators ignore it. A uniform `GenerateFn` carrying cancel in the context is MORE consistent than today, but a naive adapter for those four families that forgets to rebuild the `|| cancel.load(Ordering::SeqCst)` closure silently makes the op uncancellable — no compile error, not caught by fast unit tests. Add a cancellation regression test for at least one long-running family BEFORE the cutover.

Expected risk: **medium-high** (spans, drill aux, semantic trace, cancellation, remaining-stock, serde-skip runtime fields). Primary oracle: `cargo test --test param_sweep -- --ignored` fingerprint-unchanged.

### Phase 6 — Metric refactor (DECIDED: 6A; 6B rejected)

**DECIDED — 6A (typed report preserved) is the only viable option for this refactor.** 6B's true cost center is the optimizer (~13 files under `tool_load/optimize/` + `strategy/`/`retarget/` reading gate-specific typed numeric fields — rank.rs:54, delta.rs:155-185, narrative.rs:595-680, retarget.rs:63-78, headroom.rs:50-52), NOT MCP/GUI. A dynamic `Vec<CriterionStatus>` (verdict.rs:502, 7 fields) cannot serve them without per-metric downcasting, which is the typed report again. The MCP wire is pure serde passthrough (app/mcp.rs:2471) plus a hardwired typed-shape description string in `rs_cam_viz/src/mcp_server.rs` — 6B is a hard schema break with no mapping layer to absorb it.

**6A tasks:**

1. Add a shared `MetricContext` — **reuse/extend the existing `ToolpathLoadContext`** (tool_load/mod.rs:323, currently NOT threaded into the gates) and push it INTO the three gate `evaluate()` fns to collapse their **7-10 positional args** (deflection 7, power 8, chipload 10).
2. **Collapse the two divergent assembly sites:** make `gcode::project_load_report` (gcode/mod.rs:371) call `evaluate_toolpath` (mod.rs:366) instead of re-assembling inline — they have already diverged on `modulation_summary` (a silent correctness gap, since both are valid struct literals). Account for the three `rewrite_sim_required_to_stale_*` helpers (gcode/mod.rs:488-490).
3. Keep `ToolpathLoadVerdict` fields typed; metric modules implement a common internal `MetricEvaluator` trait for tests/organization, report construction stays typed.
4. **Before adding a 4th milling gate to `criteria()`** (verdict.rs:252), change its return from `[CriterionStatus; 3]` to a slice/`Vec` so the consumer `for status in verdict.criteria()` (sim_op_list.rs:938) doesn't break on array arity.
5. **(DECIDED 2026-06-06) Collapse the gating tier to one touch point:** make `exceeded_criteria`/`ExceededCriterion` (verdict.rs:262) and `enforce_load_policy` (gcode/mod.rs:500) **derive from `criteria()`** instead of hand-listing gates. Then "reporting-only vs gating" for any future metric is decided by whether it is included in `criteria()` assembly — one site, and a forgotten gate becomes structurally impossible rather than a silent miss. This removes the need to pre-decide whether SurfaceFinishRa (or any metric) gates g-code export: the decision becomes a one-line change whenever it's made.

**Add-a-metric cost (empirical, from drill_gates commit 47b5864 + modulation_summary commit c1a4932):** additive `Option` + `#[serde(default, skip_serializing_if)]` field = ~2 production initializers + per-test initializers (~20 test struct-literals), MCP/serde-compatible for free. **Two tiers:** "reporting-only" (cheap, above) vs "gating" (also touches `exceeded_criteria`/`ExceededCriterion` verdict.rs:262, `enforce_load_policy` gcode/mod.rs:500, smoke extraction smoke.rs:580 — ~6 extra touch points). **Always add as Option+serde-default** — a non-Option field breaks ~20 test literals and older MCP payload deserialization, falsifying the "low risk" claim.

If 6B is ever reconsidered, it additionally breaks: the `mcp_server.rs` typed-shape description string, the three gcode stale-rewrite helpers, the `diagnostics::all_diagnostic_ids_reachable_in_adapter_source` source-scan (diagnostics/tests.rs:56), and every MCP snapshot.

Expected risk: **low** (6A). Recommendation: 6A. Do not promise dynamic-registry benefits until a schema migration is explicitly approved.

### Phase 7 — Optional config/schema generation

Only after Phases 1-5 stabilize, evaluate generating config structs/defaults/schema from macros, based on the proven registry shape — not designed up front.

---

## 7. Out of scope

- Rewriting algorithm bodies (`adaptive3d`, `scallop`, `waterline`, etc.).
- Changing project TOML format unless a phase explicitly says so.
- Changing MCP schemas before a migration decision.
- Changing GUI layout/theme.
- Reinterpreting vendor LUT rows beyond Phase 0 axial fields.
- Reverting v3 / Findings fixes.
- Turning rs_cam into a plugin system.

### 7.1 Consumer-surface scope note (NEW) — CLI disposition DECIDED 2026-06-06: cull the path, migrate the harness

What the OpRegistryEntry **can** replace for consumers vs cannot — see §3.1.

**DECIDED (user, 2026-06-06): retire the parallel CLI execution path.** The debt is not the `job.toml` *format*; it is that `cli/job.rs:387-797` is a third execution path with its own 6-of-23-op router (`_ => bail!`) bypassing `OperationConfig`/`execute_operation_annotated`. Endpoint:

1. **One execution path.** Everything routes through `OperationConfig` → `execute_operation_annotated` (the strongest compile net in the codebase). `JobFile` becomes a thin deserialize-to-`OperationConfig` mapping, or sweep migrates to project files outright. **Do not attempt a partial registry hookup on the old router** — that would create a fourth divergent path.
2. **Sweep is load-bearing — migrate, don't break.** `sweep.rs:33-93` drives `job::parse_job_file`/`execute_job`, and `cargo run -p rs_cam_cli -- sweep job.toml …` is documented validation infrastructure (CLAUDE.md) AND the parity oracle for Phases 2/5. Sweep must keep working throughout; sequence its migration so the fingerprint oracle is never down during a behavior-cutover phase.
3. **Cull the ~14 per-op subcommands too** (`main.rs:2016-3348`, §2.1 row 23): replace with ONE generic `run <op> --set key=value` subcommand driven by the registry's `&'static [ParamDef]` — the registry already holds the CLI's schema. Converts ~1,400 lines of per-op clap plumbing into a surface that **cannot** miss op #24, and is the cheapest early proof the registry pays for itself. Keep the old subcommands as deprecated aliases for one release if scripts depend on them; check `toolpath_stress_test/agents/` scripts for callers before removal.
4. **Timing:** after Phase 1 (needs the registry's ParamDef table), before Phase 5 (so the behavior cutover has only 2 production drivers to prove, not 3).

### 7.1b Verified-clean surfaces — no registry changes needed (post-critique)

Two candidate areas were checked and came back CLEAN, bounding the consumer-migration surface:

- **Setup-sheet generation** (`rs_cam_viz/src/io/setup_sheet.rs`) routes operation info entirely through generic accessors — `tc.operation.label()` (:250) and `depth_of()` delegating to `OperationParams::depth_semantics()` (:9-13). No per-op match; needs zero registry/X-macro changes.
- **The toolpath-IR boundary holds:** `dressup.rs` (apply_entry/apply_tabs/apply_lead_in_out/apply_link_moves) operates purely on `Toolpath`/`AnnotatedToolpath` + `EntryStyle`/`Tab` value types with zero `OperationType` coupling — confirming the CLAUDE.md guardrail. The OperationType→dressup decision lives entirely upstream in `config.rs::normalize_for_op` (§2.1 row 20).

### 7.2 Serde/schema parity freeze (NEW)

Exact public shapes parity tests must freeze (a careless registry/X-macro change would break these silently):

- The TOML tagged `OperationConfig` `{kind, params}` shape — the MCP `set_toolpath_param` round-trip (session/compute.rs:257-358) depends on `params` being a mutable object.
- The snake_case `OperationType`/`ToolType` serde reprs — the canonical names in MCP error strings (server.rs:448,454) and viz legacy maps.
- **BOTH** `ProjectToolSection` shapes that rename `type`: core project_file.rs:182 (`tool_type: String`, lenient-parsed) and viz io/project.rs:151 (`tool_type: ToolType`, serde-direct).
- MCP `build_info` `features` flags (server.rs:486) that agents probe.
- Add a test that `ALL_2D ∪ ALL_3D ∪ {named system-only set} == ALL` (today catalog.rs:1614 only checks `system_only > 0`).

---

## 8. Value vs cost

### Value

- Fewer duplicated operation/cutter/metric routing tables (concrete dedup targets: catalog.rs:1392 + suggest.rs:723 wildcards; the two `ToolGeometryHint→ToolFamily` maps; the triplicated DrillCycle conversion; the two report-assembly sites).
- Central "what does this operation mean?" registry.
- Easier axial/radial constraint integration.
- Cleaner agent handoff.
- Better compile-time coverage where the code still relies on permissive defaults.

### Cost

- Several weeks of structural work if all phases ship.
- High review burden for macro-generated diffs.
- **Sentry churn (quantified) — deliberate re-baselines per phase:** `rationale.rs:136` exhaustive match + `wanaka_suggest_integration.rs:323-334` catch-all (Phase 4); `session/mod.rs:1441` parse_tool_type pin (Phase 3 if Q4 lands); `diagnostics/tests.rs:56` source-scan + MCP serde snapshots (Phase 6). These "break loudly" = good tripwires; distinguish from silent-pass-wrong (the gcode/mod.rs:425 vs gcode/mod.rs:477 `modulation_summary` divergence is the bad kind — both valid literals, no compile error).
- Possible dependency decisions (all DECIDED reject/defer in §5.2).

### Current recommendation

Do **not** launch a big-bang rewrite. Do:

1. fix/ship Phase 0 axial constraints (with the rev-2 corrections above);
2. land the PRE-Phase-1 feeds-hints coverage net, then Phase 1 registry (metadata-only, all 23 ops);
3. prototype-proven X-macro on metadata only (Phase 2);
4. Phase 3 cutter classification + parser unification;
5. postpone generation adapters (Phase 5) and confirm 6A for metrics.

---

## 9. Decisions (was: open questions)

| # | Question | Decision | Confidence | Evidence anchor |
|---|---|---|---|---|
| 1 | Registry shape | **DECIDED:** Option A/B hybrid, data-only Phase 1 entry (`spec`+`ParamDef`+`ToolConstraints`); `feeds_hints` as `fn` accessor; `generate`/`compatibility` deferred to Phase 5 | high | catalog.rs:190/1092/1392; execute.rs 15-arg arms |
| 2 | X-macro timing | **DECIDED:** hand-written for Phase 1; `for_each_op!` in Phase 2 for pure lists only | high | prototype 4/4 tests; catalog.rs ~12 parallel matches |
| 3 | External `MillingCutter` | **DECIDED:** internal-only, do not seal, document the asymmetry; derive `CutterKind` from `ToolGeometryHint` | high | tool/mod.rs:149/355; cutter.rs:9 closed match |
| 4 | Unknown tool type | **DECIDED:** warn-and-default, unify the 3-4 parsers first | high | project_file.rs:461; io/project.rs:882; server.rs:452; pin session/mod.rs:1441 |
| 5 | Finish-op behavior | **DECIDED:** warning-first; no auto-`stock_to_leave` mutation | high | suggest.rs:1266; companion §20-23; DropCutter has no stock_to_leave |
| 6 | Metrics | **DECIDED:** 6A (typed); reject 6B | high | optimize/* typed-field coupling; app/mcp.rs:2471 |
| 7 | Dependencies | **DECIDED:** reject strum + enum_dispatch, defer bon; no new deps | high | prototype; absent from all manifests |
| 8 | Cutover | **DECIDED:** metadata-only all 23 ops first, then behavior one family at a time | high | parity tests catalog.rs:1592, compute.rs:2715; 2 exec drivers |
| 9 | Parity gates | **DECIDED:** per-phase per-crate table (§9 Q9 in decisions output); fingerprints `--ignored` + geometry-only, gate Phase 2/5 only | high | param_sweep.rs 54 #[ignore]; fingerprint.rs geometry-only |

(Full per-question rationale is in the structured `decisions` block accompanying this document.)

**Formerly-OPEN items — RESOLVED 2026-06-06 (user direction: optimize for the architecturally sound endpoint; the refactor's purpose is compile-time/loud-failure enforcement so new features cannot silently miss sites):**

| Item | Resolution | Endpoint rationale |
|---|---|---|
| CLI `job.rs` + per-op subcommands | **Cull the path, migrate the harness** (§7.1). One execution path through `OperationConfig`→`execute_operation_annotated`; sweep harness migrates (it is the Phase-2/5 parity oracle — never down during a cutover); ~14 per-op subcommands (main.rs:2016-3348) collapse to one registry-driven `run <op> --set k=v`. After Phase 1, before Phase 5. | A third execution path is exactly the debt class this refactor exists to remove; the registry's ParamDef table already IS the generic CLI's schema. |
| Phase 6 metric tier | **Don't pre-decide the metric — fix the structure** (Phase 6 task 5). `criteria()` returns a slice; `exceeded_criteria`/`enforce_load_policy` derive from it. Gating-vs-reporting becomes a one-line inclusion decision per metric, decidable whenever. | The ~6-touch-point gating tier was the problem, not the SurfaceFinishRa answer. |
| `ALL_2D`/`ALL_3D` const vs runtime | **Keep the consts (no API break); make them checked.** Category token in `for_each_op!` is the single source of truth; sync test asserts `ALL_2D == filter(Cat2d)`, `ALL_3D == filter(Cat3d)`, and `2D ∪ 3D ∪ {named system-only set} == ALL` exactly (§7.2). | `macro_rules!` const-filtering is impractical (prototype-proven); CI-loud kills the silent "op in neither menu" failure mode, which is what matters. A runtime API buys nothing more. |
| `MillingCutter` sealing | **Document internal-only in Phase 3; seal opportunistically** whenever the trait is next touched. Not load-bearing: dispatch is on closed `ToolType`, classification derives from `ToolGeometryHint` which every impl must provide. Plugin system is out of scope (§7), so sealed is the honest endpoint — but lowest priority of the four. | Sealing makes the compiler state reality; it does not improve matrix coverage. Don't spend Phase 3 budget on it. |

---

## 10. Suggested agent split (implementation wave)

1. **Operation registry agent** — `compute/catalog.rs`, `operation_configs.rs`, `compute/execute.rs`; Phase 1 data-only registry; EXTEND existing parity tests; land the PRE-Phase-1 feeds-hints net first.
2. **Cutter classification agent** — `ToolType`, `ToolGeometryHint`, `MillingCutter`, vendor LUT mapping, **all four IO parsers**; `CutterKind` (derived from `ToolGeometryHint`), `FacingBit` reconciliation, fold `predict.rs` raw-`ToolType` matches; explicitly NOT `GeometryClass`.
3. **Metric/reporting agent** — `tool_load/verdict.rs`, `tool_load/mod.rs`, `gcode/mod.rs`, `optimize/*`, MCP/GUI consumers; implement 6A `MetricContext`, collapse the two assembly sites.
4. **Suggest/preflight agent** — `feeds/suggest.rs`, `feeds/predict.rs`, `feeds/mod.rs`, `feeds/explain.rs`, `feeds/rationale.rs`; `CutterOpProfile` aggregation around shipped helpers + the two-caller dedup; preserve `enforce_invariants` order; embed Phase-0 axial.
5. **Macro/prototype agent** — productionize the vetted `/tmp/rscam_xmacro_proto` X-macro over the real 9-field `OperationSpec`; report rust-analyzer/test ergonomics.
6. **Consumer-surface migration agent (NEW)** — owns viz `ui/properties` (param editor / menus / dressup-compat / tool editor), `rs_cam_mcp` error strings, **the CLI cull** (§7.1: retire `cli/job.rs` router, migrate sweep onto `OperationConfig`, collapse the ~14 per-op subcommands into the registry-driven generic subcommand), viz `io/project.rs` legacy op-name parser, **and `narrate.rs`** (label-string routing, §2.1 row 22 — migrate to typed `feeds_pass_role` routing before any label rename). None of agents 1-5 fully cover these; the §7.2 serde/schema freeze tests live here.

---

## 11. Pre-implementation checklist

- [x] Phase 0 companion doc updated — **DONE 2026-06-06 (companion rev 3)**: §2.1/§8.1 deflection-inversion contradiction fixed → `tip_deflection_from_engagement` binary search; residual `ap_max_absolute_mm` references dropped (§3.2/§8.3); `ap_*_factor` schema step rescoped add→verify (fields landing via concurrent agent); finish-3D classifier task added (recommend routing on `OperationSpec.feeds_family`, not a new `matches!`); §5.5 first-DPP-pass premise corrected (existing clamps at suggest.rs:850-852 already mutate DPP — bounds re-derivation is a latent-bug fix worth its own PR); stale citations fixed; extraction risk raised to medium with gate-parity test requirement.
- [x] Typed vs dynamic metric reporting — **DECIDED 6A.**
- [x] `MillingCutter` externally implementable — **DECIDED internal-only, not sealed.**
- [x] New dependencies — **DECIDED: none (reject strum/enum_dispatch, defer bon).**
- [x] Phase 1 registry shape — **DECIDED: data-only A/B hybrid.**
- [x] Land the PRE-Phase-1 feeds-hints coverage net — **DONE 2026-06-07 (`39eec99`, tracker T1).**
- [x] Define the §7.2 serde/schema parity freeze test set — **DONE 2026-06-07 (`39eec99`, tracker T2).**
- [x] User decisions — **ALL RESOLVED 2026-06-06** (§9 resolution table): CLI cull-path/migrate-harness; metric tier derived from `criteria()`; ALL_2D/ALL_3D consts kept + partition-checked; MillingCutter documented internal-only, sealed opportunistically.
- [ ] Check `toolpath_stress_test/agents/` scripts for per-op CLI subcommand callers before removal (§7.1 step 3).

---

## 12. Residual unverified items (process gaps from the investigation wave)

Honesty section — what the wave could NOT pin down, to re-check at implementation time:

1. **Line-number drift is expected.** The working tree was in active flux (concurrent agent); verification caught repeated drift during the wave itself (1431→1441, 324→323). Treat every `file:line` here as a locator hint, not gospel — re-`rg` before editing.
2. **The exact `feeds/suggest.rs` in-module test count** could not be statically confirmed; the parity gate description ("in-crate suggest.rs test module") stands but don't cite a number.
3. **The §9 Q5 answer is conditional** on the companion Phase-0 doc being fixed first: all three converging investigators leaned on the proposed (nonexistent) `SuggestWarning::FinishStockToLeaveRecommended` variant and the companion's self-contradicting §2.1/§8.1 (which still mandate inverting `predict_peak_deflection_um` despite its own §7 forbidding it).
4. **Registry-table indexing by enum discriminant** (the Phase 1 `&'static [OpRegistryEntry]` lookup) was never verified against `AlignmentPinDrill`'s system-only status (in `ALL`, in neither menu sublist). The macro prototype handles it via the `CatSys` token; the table-indexing approach must add a test that discriminant order == table order, or index by a generated `const fn` instead.
5. **Optimizer blast-radius count:** the verified figure is **~13 files** under `tool_load/optimize/` (+ `strategy/`/`retarget/` submodules); two investigators kept citing ~8. Use ~13 for sizing.
6. **A wrong citation propagated through multiple findings** during the wave — "Scallop ball-tip gate at validate.rs:254" (actually the wood-adaptive-stepover Flat check; the real enforcement is compute/execute.rs:1045 + catalog.rs:1391). It is corrected everywhere in THIS document, but beware of it resurfacing if the raw investigation corpus is consulted later.

---

## tl;dr

The refactor direction is good; start as metadata/profile consolidation, not a trait/macro big bang. Keep `OperationConfig` and public schemas stable; introduce a **data-only** operation registry, an internal **`CutterKind`** classifier (derived from `ToolGeometryHint`, NOT the existing `GeometryClass` terrain classifier or `ToolGeometryHint`), and a pre-sim `CutterOpProfile` that **aggregates already-shipped feeds/predict/explain/rationale work (~70% done)** rather than building greenfield. Use a single prototype-proven X-macro **only for pure-list metadata after the registry shape is frozen**, keeping `ALL_2D`/`ALL_3D` consts hand-written. **Metrics: 6A typed report — reject 6B** (the optimizer's typed-field coupling and the absent MCP mapping layer make a dynamic vector a rewrite, not a swap). Convert the three genuine silent wildcards (`catalog.rs:1392`, `suggest.rs:723`, `predict.rs:528`) to named policy. Phase 0 axial constraints ship first, with its internal contradiction fixed (invert `tip_deflection_mm`, not `predict_peak_deflection_um`) and a finish-3D classifier added. Parity gates are **per-crate, never workspace-wide**; the fingerprint oracle is `--ignored` and geometry-only (Phase 2/5 only). Add a **6th consumer-surface agent** for the GUI/MCP/IO/CLI surfaces no other agent owns. **All formerly-open decisions are resolved (§9 table):** cull the CLI's parallel execution path and per-op subcommands (migrate the sweep harness — it's the parity oracle), derive gating from `criteria()` so metric tiers are a one-line decision, keep `ALL_2D`/`ALL_3D` consts but partition-check them against the macro list, document `MillingCutter` internal-only and seal opportunistically. Endpoint standard (§1.1): **miss nothing, or don't compile** — every surviving `_ =>` is a named policy with a membership test.
