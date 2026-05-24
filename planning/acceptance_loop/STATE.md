# STATE — acceptance loop, current snapshot

**Read this first.** This is the single source of truth. Both auditor
and implementer write here.

> **Autonomous mode**: when the user asks Claude to "keep running the
> loop", the auditor session reads
> `handoff_prompts/autonomous_auditor.md` and orchestrates implementer
> agents in background. The user is only paged when the rs-cam MCP
> needs reconnecting/rebuilding (the auditor cannot do that itself).

## Current round

**round-02** (audit partial — test-level verification done 2026-05-25;
live MCP smoke deferred until rs-cam MCP reconnect)

- Auditor for round-02: post-compact Claude session, 2026-05-25
- Implementer(s) active: none — next pickup is F-015
- **Verification status:** F-001/F-002/F-003/F-007/F-008/F-013/F-016
  test-verified via `drill_material_plumbing_f016` (3/3 PASS),
  `chip_welding_threshold_per_material_family` (PASS), and commit
  inspection on 072c11a + 2a287c1. **Smoke verification of the
  acceptance bars (chipload-2D, deflection over-fire) is deferred to
  round-03** — needs live MCP to re-run AS006–AS017 against round-01
  baseline.

## Last verified baseline

- **round-01 baseline** (the agent smoke acceptance):
  `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
  (canonical link; original lived at
   `planning/toolpath_acceptance/baselines/2026-05-24_agent_smoke_acceptance.md`
   before migration)
- **round-00 baseline** (Tier 0 param sweep, pre-MCP-overhaul):
  `planning/acceptance_loop/rounds/round-00-2026-05-24/baseline.md`

## Open queue — implementer pulls from the top

Severity ordering: high → medium → low. Within same severity, lower effort first.

| Finding | Title | Stage | Sev | Effort | Status |
|---|---|---|---|:-:|---|
| [F-015](findings/F-015-op-precondition-static-validation.md) | Op-precondition static validation rules missing (rest, drill, project_curve) | substrate | medium | M | open — next implementer pickup |
| [F-020](findings/F-020-optimizer-ranked-bs-path.md) | Optimizer Ranked-outcome BS-stepover path untested | optimize | high | M | open (needs test fixture before fix) |
| [F-006](findings/F-006-operation-config-three-default-paths.md) | Default `OperationConfig` produced via three paths | suggest | medium | M | open — partially absorbed by F-003 |
| [F-004](findings/F-004-three-project-loaders.md) | Three project-TOML loaders | substrate | medium | L | open — likely lands with F-005 |
| [F-018](findings/F-018-test-data-templates-mismatch.md) | `test_data/ux_*.toml` templates don't match smoke CSV | infra | medium | S | open |
| [F-009](findings/F-009-diagnostic-views-viz-only.md) | Five diagnostic views, only viz emits them | substrate | medium | M | open — partially lands with F-005 |
| [F-005](findings/F-005-two-mcp-servers.md) | Two MCP server implementations | substrate | medium | XL | deferred — wait for F-001…F-005 of unification to land first |
| [F-011](findings/F-011-operation-feeds-hints-split-crates.md) | `operation_feeds_hints` split across crates | suggest | low | S | open — lands with F-003 |
| [F-017](findings/F-017-rapid-collisions-everywhere.md) | Rapid collisions in nearly every op (1041 on adaptive3d) | sim | medium | L | open — separate path-planning investigation |
| [F-010](findings/F-010-catalog-six-match-blocks.md) | Catalog has six 23-arm match blocks | substrate | low | M | open — re-evaluate after F-003 lands |
| [F-019](findings/F-019-stepover-semantic-cardinality.md) | Stepover semantic cardinality across op families | suggest | low | M | deferred — re-evaluate after F-003 |
| [F-021](findings/F-021-suggest-all-paint-thrash.md) | Suggest All button recomputes LUT every paint | viz | low | S | open |
| [F-022](findings/F-022-catalog-match-arm-collapse.md) | Collapse remaining catalog `match` blocks | substrate | low | M | open — unblocked by F-003 landing |
| [F-014](findings/F-014-doc-drift-service-layer.md) | Three audit docs describe dead architecture | docs | low | S | open |

## In flight (claimed by implementer)

| Finding | Claimed by | PR | Notes |
|---|---|---|---|
_(none — see Implementation log for F-015 landing 2026-05-25)_

## Closed this round

- **F-001** — chipload 2D feedopt probe fixed (landed 2026-05-25, unification batch).
- **F-002** — `peak_axial_doc_mm` split into axial-DOC + plunge-descent (landed 2026-05-25).
- **F-003** — VENDOR_LUT singletons collapsed + workholding plumbed through (landed 2026-05-25; subsumes F-012).
- **F-007** — `DrillConfig::set_plunge_rate` honored (landed 2026-05-25).
- **F-008** — `compute_stale_set` introduced as single authority (landed 2026-05-25).
- **F-013** — feeds-result invariants enforced (landed 2026-05-25 with F-003).
- **F-016** — drill `chip_welding` / `peck_adequacy` / `plunge_feed` gates now see the live stock material instead of `Material::default()` (landed 2026-05-25).
- **F-012** — closed as duplicate of F-003 (the singleton collapse covered the workholding-Medium hardcode in `feeds_result_for_toolpath`).

## Acceptance bars status

Snapshot taken from round-01 baseline.

| Bar | Target | Current | Delta needed |
|---|---:|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured (no clean rerun yet) | needs first post-fix baseline |
| Sim chipload calibration (3D ops) | ≥ 95% | 3/3 fired correctly when applicable | already passing for 3D, dead on 2D |
| Sim chipload calibration (2D ops) | ≥ 95% | 0/7 (all Unmodeled) | F-001 unblocks |
| Sim deflection calibration | ≥ 95% | 4/13 (deflection over-fires from F-002) | F-002 unblocks |
| Optimizer honest-improvement | ≥ 95% | 2/2 tested produced honest outcomes | undertested — need Ranked outcome reached, F-020 |
| Optimizer refusal correctness | 100% | 2/2 (drill Skipped, scallop deflection_setup_locked) | passing in tested cases |
| Export gate | 100% | not tested | open |

## Blockers / questions for the user

- None active.

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
- 2026-05-25 — F-015 landed: op-precondition static-validation adapter
  (`from_preconditions`) now surfaces blocking diagnostics on rest /
  drill / alignment_pin_drill / project_curve before generate time.
  New file `crates/rs_cam_core/src/diagnostics/adapters/from_preconditions.rs`
  + wire-up in `session/compute.rs::precondition_context_for_toolpath`.
  Acceptance tests:
  `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs::{rest_op_without_prior_enabled_tool_surfaces_blocking_diagnostic, rest_op_without_prev_tool_id_surfaces_blocking_diagnostic, rest_op_with_correct_prior_is_silent, project_curve_in_single_model_project_surfaces_blocking_diagnostic, project_curve_without_any_surface_mesh_surfaces_blocking_diagnostic, project_curve_with_curve_and_surface_models_is_silent, drill_op_against_mesh_only_model_surfaces_blocking_diagnostic, drill_op_against_polygon_model_is_silent}`
  + adapter unit tests at
  `diagnostics::adapters::from_preconditions::tests::*`.

## How to update this file

- **Auditor** rewrites the queue at the start of every round, moves
  resolved items into "Closed this round" with the round's baseline
  link, and records the new acceptance-bar snapshot.
- **Implementer** updates only the "In flight" table when claiming /
  releasing a finding, and appends one line to "Implementation log"
  when a PR lands. Never touches the queue ordering or acceptance bars.
