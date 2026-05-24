# STATE — acceptance loop, current snapshot

**Read this first.** This is the single source of truth. Both auditor
and implementer write here.

## Current round

**round-02** (audit pending after round-01's fixes land)

- Auditor for round-02: not yet assigned (next post-compact Claude session)
- Implementer(s) active: none (round-01 unification batch landed 2026-05-25)

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
| [F-012](findings/F-012-feeds-result-toolpath-workholding-medium.md) | `feeds_result_for_toolpath` hardcodes workholding=Medium | suggest | high | S | duplicate of F-003 — close when F-003 lands |
| [F-017](findings/F-017-rapid-collisions-everywhere.md) | Rapid collisions in nearly every op (1041 on adaptive3d) | sim | medium | L | open — separate path-planning investigation |
| [F-010](findings/F-010-catalog-six-match-blocks.md) | Catalog has six 23-arm match blocks | substrate | low | M | open — re-evaluate after F-003 lands |
| [F-019](findings/F-019-stepover-semantic-cardinality.md) | Stepover semantic cardinality across op families | suggest | low | M | deferred — re-evaluate after F-003 |
| [F-021](findings/F-021-suggest-all-paint-thrash.md) | Suggest All button recomputes LUT every paint | viz | low | S | open |
| [F-022](findings/F-022-catalog-match-arm-collapse.md) | Collapse remaining catalog `match` blocks | substrate | low | M | deferred — depends on F-003 |
| [F-014](findings/F-014-doc-drift-service-layer.md) | Three audit docs describe dead architecture | docs | low | S | open |

## In flight (claimed by implementer)

| Finding | Claimed by | PR | Notes |
|---|---|---|---|
_(none — round-01 unification batch landed 2026-05-25; see Implementation log)_

## Closed this round

- **F-001** — chipload 2D feedopt probe fixed (landed 2026-05-25, unification batch).
- **F-002** — `peak_axial_doc_mm` split into axial-DOC + plunge-descent (landed 2026-05-25).
- **F-003** — VENDOR_LUT singletons collapsed + workholding plumbed through (landed 2026-05-25; subsumes F-012).
- **F-007** — `DrillConfig::set_plunge_rate` honored (landed 2026-05-25).
- **F-008** — `compute_stale_set` introduced as single authority (landed 2026-05-25).
- **F-013** — feeds-result invariants enforced (landed 2026-05-25 with F-003).
- **F-016** — drill `chip_welding` / `peck_adequacy` / `plunge_feed` gates now see the live stock material instead of `Material::default()` (landed 2026-05-25).

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

## How to update this file

- **Auditor** rewrites the queue at the start of every round, moves
  resolved items into "Closed this round" with the round's baseline
  link, and records the new acceptance-bar snapshot.
- **Implementer** updates only the "In flight" table when claiming /
  releasing a finding, and appends one line to "Implementation log"
  when a PR lands. Never touches the queue ordering or acceptance bars.
