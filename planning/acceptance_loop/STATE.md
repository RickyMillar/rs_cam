# STATE — acceptance loop, current snapshot

**Read this first.** This is the single source of truth. Both auditor
and implementer write here.

> **Autonomous mode**: when the user asks Claude to "keep running the
> loop", the auditor session reads
> `handoff_prompts/autonomous_auditor.md` and orchestrates implementer
> agents in background. The user is only paged when the rs-cam MCP
> needs reconnecting/rebuilding (the auditor cannot do that itself).

## Current round

**round-03** (in progress — F-023 implementer running, F-015 smoke-verified)

- Round-02 closed 2026-05-25; delta:
  `rounds/round-02-2026-05-25/delta.md`. 5-case focused smoke
  (AS001/AS002/AS011/AS013/AS015) vs round-01 baseline.
- F-015 landed at `ba3f84d` during round-02; **smoke-verified late
  in round-02** after user rebuilt MCP. Both rest and project_curve
  preconditions emit blocking `precondition.*` diagnostics through
  `diagnostic_delta` + `gui_banners` + `warnings`.
- F-023 implementer fired 2026-05-25 (background) targeting
  model_id-ref diagnostic asymmetry; pattern mirrors F-015's
  `from_preconditions.rs` adapter shape.

## Last verified baseline

- **round-02 delta** (5-case focused smoke vs round-01):
  `planning/acceptance_loop/rounds/round-02-2026-05-25/delta.md`
  (smoke run dir: `target/acceptance_sweeps/agent_smoke_20260525_0957/`)
- **round-01 baseline** (the agent smoke acceptance):
  `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
- **round-00 baseline** (Tier 0 param sweep, pre-MCP-overhaul):
  `planning/acceptance_loop/rounds/round-00-2026-05-24/baseline.md`

## Open queue — implementer pulls from the top

Severity ordering: high → medium → low. Within same severity, lower effort first.

| Finding | Title | Stage | Sev | Effort | Status |
|---|---|---|---|:-:|---|
| [F-023](findings/F-023-mcp-gui-diagnostic-asymmetry.md) | MCP/GUI diagnostic surface asymmetry: "Selected model missing" + invalid model_id accepted silently | substrate | high | S–M | open — **next implementer pickup**; opened round-02 |
| [F-002](findings/F-002-axial-doc-mm-two-quantities.md) | `peak_axial_doc_mm` linear-kinematics still over-reads (deflection still over-fires) | sim | high | S | reopened — partial fix landed; need linear-class follow-up |
| [F-020](findings/F-020-optimizer-ranked-bs-path.md) | Optimizer Ranked-outcome BS-stepover path untested | optimize | high | M | open (needs test fixture before fix) |
| [F-006](findings/F-006-operation-config-three-default-paths.md) | Default `OperationConfig` produced via three paths | suggest | medium | M | open — partially absorbed by F-003 |
| [F-004](findings/F-004-three-project-loaders.md) | Three project-TOML loaders | substrate | medium | L | open — likely lands with F-005 |
| [F-018](findings/F-018-test-data-templates-mismatch.md) | `test_data/ux_*.toml` templates don't match smoke CSV | infra | medium | S | open |
| [F-009](findings/F-009-diagnostic-views-viz-only.md) | Five diagnostic views, only viz emits them | substrate | medium | M | open — partially lands with F-005 |
| [F-005](findings/F-005-two-mcp-servers.md) | Two MCP server implementations | substrate | medium | XL | deferred — wait for F-001…F-005 of unification to land first |
| [F-011](findings/F-011-operation-feeds-hints-split-crates.md) | `operation_feeds_hints` split across crates | suggest | low | S | open — lands with F-003 |
| [F-017](findings/F-017-rapid-collisions-everywhere.md) | Rapid collisions in nearly every op (1041→844 on adaptive3d post-batch; still high) | sim | medium | L | open — improvement noted round-02, separate path-planning investigation |
| [F-010](findings/F-010-catalog-six-match-blocks.md) | Catalog has six 23-arm match blocks | substrate | low | M | open — re-evaluate after F-003 lands |
| [F-019](findings/F-019-stepover-semantic-cardinality.md) | Stepover semantic cardinality across op families | suggest | low | M | deferred — re-evaluate after F-003 |
| [F-021](findings/F-021-suggest-all-paint-thrash.md) | Suggest All button recomputes LUT every paint | viz | low | S | open |
| [F-022](findings/F-022-catalog-match-arm-collapse.md) | Collapse remaining catalog `match` blocks | substrate | low | M | open — unblocked by F-003 landing |

## In flight (claimed by implementer)

| Finding | Claimed by | PR | Notes |
|---|---|---|---|
_(none — see Implementation log for F-023 landing 2026-05-25)_

## Closed round-02 (2026-05-25)

Verified by round-02 smoke (`rounds/round-02-2026-05-25/delta.md`):

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

**Reopened this round:**
- **F-002** — partial fix landed but smoke acceptance NOT met.
  Engagement-vector kinematics (arc/helix/plunge) correctly report
  axial-DOC vs plunge-descent. Linear kinematics still over-reads
  `peak_axial_doc_mm = 12.0` (full stock height). Deflection gate
  still over-fires on AS001 (374µm), AS002 (408µm), AS013 (573µm),
  AS015 (431µm) — identical to round-01.

**Opened this round:**
- **F-023** — MCP/GUI diagnostic surface asymmetry: "Selected model
  missing" surfaces in GUI banner, project loader warnings, and
  `list_toolpaths.error` / `runtime_errors`, but **not** in
  `get_toolpath_diagnostics` / `get_project_diagnostics` /
  `add_toolpath` envelope. `add_toolpath` accepts invalid `model_id`
  silently.

## Acceptance bars status

Snapshot taken from round-02 delta vs round-01 baseline.

| Bar | Target | Round-01 | Round-02 | Status |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured | unmeasured | needs full sweep |
| Sim chipload calibration (3D ops) | ≥ 95% | 3/3 fired | 1/1 fired (AS013) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 0/7 (all Unmodeled) | 2/2 fired (AS001/AS002) | **F-001 verified** |
| Sim deflection calibration | ≥ 95% | 4/13 Within | 0/4 verdict change | **F-002 reopened** |
| Optimizer honest-improvement | ≥ 95% | 2/2 | 1/1 (AS015) | stable; F-020 still untested |
| Optimizer refusal correctness | 100% | 2/2 | 1/1 (AS015 byte-identical) | stable |
| Export gate | 100% | not tested | not tested | open |

## Blockers / questions for the user

- None active. (Round-02 MCP-rebuild blocker resolved: user rebuilt
  late-round-02 and F-015 was smoke-verified.) Next rebuild trigger
  will be when F-023 implementer returns and round-03 smoke runs
  against the new core-side adapter.

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

## How to update this file

- **Auditor** rewrites the queue at the start of every round, moves
  resolved items into "Closed this round" with the round's baseline
  link, and records the new acceptance-bar snapshot.
- **Implementer** updates only the "In flight" table when claiming /
  releasing a finding, and appends one line to "Implementation log"
  when a PR lands. Never touches the queue ordering or acceptance bars.
