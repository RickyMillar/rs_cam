# STATE — acceptance loop, current snapshot

**Read this first.** This is the single source of truth. Both auditor
and implementer write here.

> **Autonomous mode**: when the user asks Claude to "keep running the
> loop", the auditor session reads
> `handoff_prompts/autonomous_auditor.md` and orchestrates implementer
> agents in background. The user is only paged when the rs-cam MCP
> needs reconnecting/rebuilding (the auditor cannot do that itself).

## Current round

**round-05** (audit pending — clean stopping point)

Round-04 closed 2026-05-25. **F-024 fully verified on AS001**:
deflection.peak_mm dropped 0.374 → 0.076 (Within), collisions 36 → 0,
air-cut 77 → 37%, per-pass volumes finally plausible. Five
F-024-related commits across core + viz worker + viz controller
(`d82bd4d` + `06a9a2a` + `0c907a6` + `56e9ec2` + `67de558`).

Delta: `rounds/round-04-2026-05-25/delta.md`.

**AS013 unchanged** because its stock has `origin_z = 0` — the
F-024 frame mismatch doesn't apply. Its 573µm deflection is either
genuine overload or a different latent bug; flagged for round-05.

Loop learning from round-04: three rebuild cycles burned because
each implementer's local test passed without exercising the
production entry point. Worth a contract-doc update next round.

Implementer(s) active: none.

## Last verified baseline

- **round-04 delta** (F-024 three-site fix verified on AS001;
  AS013 unchanged): `planning/acceptance_loop/rounds/round-04-2026-05-25/delta.md`
- **round-03 delta** (probes verifying F-015 + F-023; F-002 reframe):
  `planning/acceptance_loop/rounds/round-03-2026-05-25/delta.md`
- **round-02 delta** (5-case focused smoke vs round-01):
  `planning/acceptance_loop/rounds/round-02-2026-05-25/delta.md`
- **round-01 baseline**:
  `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
- **round-00 baseline** (Tier 0 param sweep, pre-MCP-overhaul):
  `planning/acceptance_loop/rounds/round-00-2026-05-24/baseline.md`

## Open queue — implementer pulls from the top

Severity ordering: high → medium → low. Within same severity, lower effort first.

| Finding | Title | Stage | Sev | Effort | Status |
|---|---|---|---|:-:|---|
| [F-020](findings/F-020-optimizer-ranked-bs-path.md) | Optimizer Ranked-outcome BS-stepover path untested | optimize | high | M | open — **next implementer pickup** (needs test fixture before fix) |
| [F-025](findings/F-025-non-identity-setup-z-frame.md) | Z-frame mismatch on non-identity setups (face_up=Bottom etc.) | substrate | medium | S–M | open — stub; revisit when smoke surfaces it |
| [F-006](findings/F-006-operation-config-three-default-paths.md) | Default `OperationConfig` produced via three paths | suggest | medium | M | open — partially absorbed by F-003 |
| [F-004](findings/F-004-three-project-loaders.md) | Three project-TOML loaders | substrate | medium | L | open — likely lands with F-005 |
| [F-009](findings/F-009-diagnostic-views-viz-only.md) | Five diagnostic views, only viz emits them | substrate | medium | M | open — partially lands with F-005 |
| [F-005](findings/F-005-two-mcp-servers.md) | Two MCP server implementations | substrate | medium | XL | deferred — wait for F-001…F-005 of unification to land first |
| [F-011](findings/F-011-operation-feeds-hints-split-crates.md) | `operation_feeds_hints` split across crates | suggest | low | S | open — lands with F-003 |
| [F-017](findings/F-017-rapid-collisions-everywhere.md) | Rapid collisions — reframed: AS001 36→0 via F-024, residual AS013 (844) is auto_from_model-only | sim | low | S–M | reframed round-04; defer until full round-05 sweep confirms scope reduction |
| [F-010](findings/F-010-catalog-six-match-blocks.md) | Catalog has six 23-arm match blocks | substrate | low | M | open — re-evaluate after F-003 lands |
| [F-019](findings/F-019-stepover-semantic-cardinality.md) | Stepover semantic cardinality across op families | suggest | low | M | deferred — re-evaluate after F-003 |
| [F-021](findings/F-021-suggest-all-paint-thrash.md) | Suggest All button recomputes LUT every paint | viz | low | S | open |
| [F-022](findings/F-022-catalog-match-arm-collapse.md) | Collapse remaining catalog `match` blocks | substrate | low | M | open — unblocked by F-003 landing |

## In flight (claimed by implementer)

| Finding | Claimed by | PR | Notes |
|---|---|---|---|
_(none)_

## Closed round-02 (2026-05-25)

Verified by round-02 smoke (`rounds/round-02-2026-05-25/delta.md`):

- **F-002** — `peak_axial_doc_mm` split into `axial_engagement_mm` +
  `plunge_descent_mm` (landed in `072c11a`; verified at the split
  scope by round-03 implementer probe). The deflection over-fire
  observed in round-02 is **not** caused by an incomplete split —
  it's the F-024 frame-mismatch bug. F-002's split itself is
  complete and correct.
- **F-001** — chipload 2D feedopt probe fixed. AS001/AS002 moved from
  `chipload: Unmodeled` to `Exceeds_LOW` with `validated` confidence
  and vendor-LUT bounds.
- **F-016** — drill `chip_welding` threshold material-aware. AS011
  hardwood threshold moved from `8.0` (hardcoded softwood) to `6.0`.
- **F-003** — VENDOR_LUT singletons collapsed + workholding plumbed
  through (subsumes F-012). No vendor-LUT regressions across 3 op
  kinds in smoke.
- **F-007** — `DrillConfig::set_plunge_rate` honored. (Note: drill
  schema no longer exposes a separate `plunge_rate` param; `feed_rate`
  covers the cycle's plunge.)
- **F-008** — `compute_stale_set` single authority. `stale_toolpaths`
  envelope field present and accurate across all add/set_param calls.
- **F-013** — feeds-result invariants enforced. `feeds.feed_vs_lut.high`
  diagnostic consistent across cases.
- **F-015** — op-precondition static-validation landed (commit
  `ba3f84d` during round-02). **Smoke-verified** late-round-02 after
  user rebuilt MCP: rest + project_curve preconditions both emit
  blocking `precondition.*` diagnostics through `diagnostic_delta` +
  `gui_banners` + `warnings`. Drill precondition covered by F-015's
  existing integration tests.
- **F-012** — closed as duplicate of F-003.

**Reframed in round-03 (the round-02 reopen was wrong):**
- **F-002** — round-02 reopen framing was incorrect (linear-class
  asymmetry was transit-tag filtering, not a code path bug). The
  original split into `axial_engagement_mm` + `plunge_descent_mm`
  DID land correctly in commit `072c11a`. Per-sample
  `axial_engagement_mm` is correctly populated for non-plunge samples
  on all four kinematics classes; per-sample `plunge_descent_mm` is
  correctly populated for plunge samples. F-002 is **landed at the
  split scope**. The deflection over-fire is a different root cause —
  see F-024.

**Opened this round:**
- **F-023** (now landed) — MCP/GUI diagnostic surface asymmetry:
  "Selected model missing" surfaces in GUI banner, project loader
  warnings, and `list_toolpaths.error` / `runtime_errors`, but
  previously **not** in `get_toolpath_diagnostics` /
  `get_project_diagnostics` / `add_toolpath` envelope.
  `add_toolpath` accepted invalid `model_id` silently. Now surfaced
  via `ref.model_missing` adapter.
- **F-024** — Z-frame mismatch in dexel stock grid. Root cause of the
  deflection over-fire that round-02 attributed (wrongly) to F-002's
  linear-class path. For identity setups, no local↔global transform
  is applied; toolpath emits Z stock-top-relative; grid spans
  Z=[0, stock_z]. Cutter is below the entire ray → full ray cleared →
  `axial_engagement_mm` reads full stock height instead of commanded
  DOC. Three candidate fix sites; M–L effort with fingerprint regen.

## Acceptance bars status

Snapshot taken from round-02 delta vs round-01 baseline.

| Bar | Target | Round-01 | Round-02 | Status |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured | unmeasured | needs full sweep |
| Sim chipload calibration (3D ops) | ≥ 95% | 3/3 fired | 1/1 fired (AS013) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 0/7 (all Unmodeled) | 2/2 fired (AS001/AS002) | **F-001 verified** |
| Sim deflection calibration | ≥ 95% | 4/13 Within | 0/4 verdict change | **F-024 opened** (root cause isolated; F-002 split was a red herring) |
| Optimizer honest-improvement | ≥ 95% | 2/2 | 1/1 (AS015) | stable; F-020 still untested |
| Optimizer refusal correctness | 100% | 2/2 | 1/1 (AS015 byte-identical) | stable |
| Export gate | 100% | not tested | not tested | open |

## Blockers / questions for the user

- None active. Round-04 closed cleanly with F-024 smoke-verified on
  AS001 (deflection 0.374 → 0.076, all collateral metrics moved too).
- Round-05 candidates when ready:
  - Re-sweep full AS001-AS018 matrix to update deflection bar
    properly (was 4/13 Within; AS001 now joins).
  - Investigate AS013's residual 573µm (origin_z=0 case, F-024
    doesn't apply) — either open a new finding or close as real overload.
  - F-020 (optimizer Ranked-BS path) needs a fixture spec first.
  - F-017 (rapid collisions) reframe — AS001's 36 collisions
    vanished with the frame fix; only AS013-class remains.
  - Doc PR adding the "test through the production entry point"
    learning to `implementer_contract.md` and `autonomous_auditor.md`.

## Implementation log

(implementers append here when they land a PR; auditor moves entries to round directories when verified)

- 2026-05-24 — MCP param UX overhaul shipped (`integer_param_coercion`,
  `mutation_result_envelope`, `operation_schema`, `param_schema_hints`,
  `param_schema_optional_nulls`, `valid_param_error_hints`). Original
  prompt archived at `archive/MCP_PARAM_UX_OVERHAUL_AGENT_PROMPT.md`.
  Result: agent smoke run completed AS006–AS017 without re-hitting the
  param-discovery wall that AS001–AS005 had to work around.
- 2026-05-25 — F-001/F-002/F-003/F-007/F-008/F-013 unification batch
  landed (see `planning/CODEBASE_UNIFICATION_PLAN.md` for the per-fix
  mapping). Subsumes F-012; partially absorbs F-006 / F-011.
- 2026-05-25 — F-016 landed: drill_op view now carries the live stock
  material (was `Material::default()`). Acceptance tests:
  `crates/rs_cam_core/tests/drill_material_plumbing_f016.rs::{drill_op_carries_hardwood_material_from_stock, drill_op_carries_softwood_material_from_stock, drill_op_carries_plastic_material_from_stock}`
  and `drill_metrics::tests::chip_welding_threshold_per_material_family`.
- 2026-05-25 — F-014 landed: annotated stale service-layer/dispatch-duplication audit docs (`review/results/41_duplication.md`, `review/results/30_compute.md`) with resolved-2026-05-25 markers pointing at `crates/rs_cam_core/src/compute/execute.rs:201 execute_operation`; archived `review/SERVICE_LAYER_OWNERSHIP_AUDIT.md` → `review/archive/` with a top-of-file note. No acceptance test (docs-only).
- 2026-05-25 — F-015 landed: op-precondition static-validation adapter
  (`from_preconditions`) now surfaces blocking diagnostics on rest /
  drill / alignment_pin_drill / project_curve before generate time.
  New file `crates/rs_cam_core/src/diagnostics/adapters/from_preconditions.rs`
  + wire-up in `session/compute.rs::precondition_context_for_toolpath`.
  Acceptance tests:
  `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs::{rest_op_without_prior_enabled_tool_surfaces_blocking_diagnostic, rest_op_without_prev_tool_id_surfaces_blocking_diagnostic, rest_op_with_correct_prior_is_silent, project_curve_in_single_model_project_surfaces_blocking_diagnostic, project_curve_without_any_surface_mesh_surfaces_blocking_diagnostic, project_curve_with_curve_and_surface_models_is_silent, drill_op_against_mesh_only_model_surfaces_blocking_diagnostic, drill_op_against_polygon_model_is_silent}`
  + adapter unit tests at
  `diagnostics::adapters::from_preconditions::tests::*`.
- 2026-05-25 — F-024 landed: dexel stock grid uses world Z frame for
  identity setups. `session/compute.rs` now returns
  `local_stock_bbox = None` (and `local_to_global = None`) when the
  setup is identity, so `run_simulation` falls back to
  `request.stock_bbox` (world frame). Per-setup grid Z range now
  matches the toolpath frame; `axial_engagement_mm` reads the
  commanded DOC instead of the full stock height. Commit `d82bd4d`.
  Acceptance tests:
  `crates/rs_cam_core/tests/dexel_stock_z_frame_f024.rs::{as001_pocket_first_pass_axial_engagement_within_commanded_doc, as001_pocket_deflection_gate_within_safe_band}`.
  CLAUDE.md `Metric caveats` block updated with F-024 follow-up
  paragraph. Param sweep fingerprints unchanged — the harness uses the
  lower-level operation kernels directly and doesn't traverse the
  `ProjectSession` setup-transform path the fix touches; no
  regen needed.
- 2026-05-25 — F-023 landed: core-side `from_model_refs` adapter
  surfaces `ref.model_missing` (Severity::Blocking) when a toolpath's
  `model_id` doesn't resolve against the loaded project. Wired into
  the orchestrator via `ToolpathDiagnoseInputs::model_refs` and
  `session::compute::model_ref_context_for_toolpath`; MCP envelope's
  `diagnostic_delta` / `gui_banners` / `warnings` now carries the
  signal the GUI banner already showed. Also fixed the misleading
  `add_toolpath` `model_id` docstring in
  `crates/rs_cam_mcp/src/server.rs` (no longer says "usually 0 for
  the first model"). New file
  `crates/rs_cam_core/src/diagnostics/adapters/from_model_refs.rs`;
  new ID `ids::REF_MODEL_MISSING`. Acceptance tests:
  `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs::{pocket_op_with_unresolved_model_id_surfaces_blocking_diagnostic, pocket_op_with_resolved_model_id_emits_no_ref_diagnostic, face_op_with_unresolved_model_id_is_silent}`
  + adapter unit tests at
  `diagnostics::adapters::from_model_refs::tests::*`.
- 2026-05-25 — F-024 viz-path follow-up landed: round-04 auditor
  smoke confirmed the original F-024 fix (`d82bd4d`) wasn't taking
  effect through the production MCP / GUI path because the viz
  worker's `compute::worker::execute::build_core_simulation_request`
  was unconditionally wrapping the viz-side zero-rooted
  `local_stock_bbox` in `Some(...)` — so core's "fall back to world
  frame when `local_stock_bbox` is None" branch never fired on the
  viz path (AS001 stayed at `peak_axial_doc_mm = 12.0` /
  `deflection.peak_mm = 0.374` after d82bd4d). Mirrored the
  `session/compute.rs` identity-setup conditional in
  `build_core_simulation_request`: when `local_to_global` is `None`
  (identity), forward `local_stock_bbox = None` so core falls back to
  `request.stock_bbox` (world frame). Commit `0c907a6`. Regression
  test:
  `crates/rs_cam_viz/src/compute/worker/tests.rs::as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`
  (pre-fix peak axial = 9.0 mm; post-fix ≈ 2.0 mm).
- 2026-05-25 — F-024 third-site landed: round-04 auditor smoke after
  the second MCP rebuild still showed AS001 `deflection.peak_mm = 0.374`
  byte-identical to round-02. Root cause: even with the core fix
  (`d82bd4d`) and viz-worker fix (`0c907a6`) in place, the viz
  controller's `build_simulation_groups` constructed the world
  `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
  dropping `stock.origin_{x,y,z}`. For AS001 (`origin_z=-12`) the
  controller passed bbox `(0,0,0)..(100,100,12)` to the worker. The
  viz worker's `local_stock_bbox = None` fallback then used this
  broken `request.stock_bbox` as the dexel grid bounds, so the grid
  spanned the wrong Z range and the cutter at world Z=-2 again sat
  below every ray. Fix: replace the inline construction with
  `ProjectSession::stock_bbox()` (which delegates to
  `StockConfig::bbox()` and applies the origin correctly), extracted
  through a pure free function `controller::events::simulation::
  build_world_stock_bbox` for testability. Bumped
  `compute::worker::execute` mod and `run_simulation_with_phase`
  function visibility to `pub(crate)` so the controller-path test
  can drive the same viz simulation entry point the worker thread
  uses. Acceptance tests:
  `crates/rs_cam_viz/src/controller/tests.rs::{build_world_stock_bbox_respects_stock_origin_f024, controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024}`
  (pre-fix peak axial = 9.0 mm; post-fix ≈ 2.0 mm — identical signal
  to the worker-tests F-024 regression but driven through the
  controller helper rather than a hand-built world bbox).
- 2026-05-25 — loop docs: added "Test through the production entry
  point" rule to `implementer_contract.md`, matching MUST-bullet in
  `handoff_prompts/autonomous_auditor.md`, and audit verification
  note in `audit_runbook.md` Step 1. Encodes the round-04 three-
  rebuild-saga learning (`rounds/round-04-2026-05-25/delta.md`).
  No F-ID — loop-doc work. No acceptance test (docs-only).
- 2026-05-25 — F-018 landed: regenerated `test_data/ux_*.toml`
  templates to match `cases_agent_smoke.csv`. Added `End Mill 3mm` +
  `90deg V-bit 6mm` to `ux_2d_star.toml` (AS007-AS010). Created
  per-material variants `ux_2d_pocket_softwood.toml`,
  `ux_2d_pocket_mdf.toml`, `ux_step_plate_mdf.toml`. Deleted the
  broken `Rivers (back) (copy)` toolpath from `ux_3d_terrain.toml`
  and added the `demo_star.svg` curve model so AS018's
  `project_curve` has a source curve. CSV updated for AS002 →
  pocket_softwood, AS004 → plate_mdf, AS005 → pocket_mdf
  (project_template column only, per auditor restriction).
  Residual material mismatches on AS011/AS012/AS013/AS017 left as
  out-of-scope and tolerated by the test's `KNOWN_MATERIAL_MISMATCHES`
  table for a follow-up finding. Acceptance tests:
  `crates/rs_cam_core/tests/test_data_smoke_csv_alignment.rs::{all_smoke_cases_reference_existing_templates_and_fixtures, all_smoke_cases_load_via_project_session, all_smoke_cases_have_required_tool_in_template, all_smoke_cases_have_matching_material_family}`.
  Pre-fix verified failing on `all_smoke_cases_have_required_tool_in_template`
  (AS007-AS010 missing tools) by stashing `ux_2d_star.toml` and
  re-running.

## How to update this file

- **Auditor** rewrites the queue at the start of every round, moves
  resolved items into "Closed this round" with the round's baseline
  link, and records the new acceptance-bar snapshot.
- **Implementer** updates only the "In flight" table when claiming /
  releasing a finding, and appends one line to "Implementation log"
  when a PR lands. Never touches the queue ordering or acceptance bars.
