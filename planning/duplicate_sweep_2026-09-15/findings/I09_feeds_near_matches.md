# I09 — feeds/inspection near-matches
Verdict: mixed — P1 FALSE_POSITIVE (caller/callee), P2 TRUE_DUP, P3 SIBLING

## Evidence

### P1 (0.9218): feeds/efficiency.rs L309-351 ↔ feeds/force.rs L271-296 — FALSE_POSITIVE
- `feeds/efficiency.rs:320-351` `deflection_chipload_ceiling_mm` (private) is an
  input-assembly adapter: builds the cutter (`compute::cutter::build_cutter`),
  computes stickout/youngs/compliance, then **calls**
  `force::chipload_cap_for_deflection` at `efficiency.rs:343`.
- `feeds/force.rs:279-295` `chipload_cap_for_deflection` is itself a thin
  wrapper: `chipload_cap_for_deflection_with_reason(...).ok()`; the model lives
  in `_with_reason` (`force.rs:225`). The match is doc-comment vocabulary
  ("ceiling … EXCEEDS_BOUND_MM … None = abstain"), not duplicated logic. One
  model, one home (`force.rs`) — the plan's intended pattern.
- Callers: `deflection_chipload_ceiling_mm` ← `efficiency.rs:235` only;
  `chipload_cap_for_deflection` ← efficiency.rs only (rg, production).
- Sentries: force.rs in-module tests (11);
  `tests/efficiency_abstains_without_kc_g_specenergy.rs:216-262` pins the
  ceiling abstention; `tests/cut_efficiency_is_closed_form_g_specenergy.rs`.

### P2 (0.9469): feeds/vendor_normalize.rs L125-141 ↔ tool_load/optimize/context.rs L152-168 — TRUE_DUP
- `vendor_normalize.rs:130` `pub fn op_family_to_lut` vs `context.rs:157`
  `pub(crate) fn lut_op_family_from`: diff shows only doc comment, name and
  visibility differ; the 8-arm `OperationFamily → LutOperationFamily` match
  body is byte-identical. No behavioral drift today.
- `op_family_to_lut` callers: `tool_load/mod.rs:305` (gate),
  `vendor_normalize.rs:190` (`to_lookup_query`), `vendor_lut.rs:818,868`
  (loader reachability census); pinned by sentry
  `tests/lut_resolver_census_a6.rs` (L361, L636).
- `lut_op_family_from` callers: `context.rs:345` (OptimizeContext build) and
  unit test `optimize/mod.rs:3245 lut_op_family_mapping_covers_all_variants` —
  whose own comment says the arms must not drift from "the suggest module's
  mapping". The duplication is acknowledged in-tree; the test is the
  drift tripwire, not a justification.
- Authoritative side: `vendor_normalize::op_family_to_lut` (pub, F3.2
  provenance comment, 4 call sites incl. sentry).

### P3 (0.9271): optimize/narrative.rs L105-116 ↔ optimize/refusal.rs L20-30 — SIBLING
- `narrative.rs:109` `pub DeflectionSetupDetail` (Serialize/Deserialize, 3
  fields) vs `refusal.rs:20` `pub(crate) DeflectionSetupPrescription` (same 3
  fields + `text: String`). Different purposes: serialized MCP/GUI narrative
  payload vs internal refusal builder carrying prose.
- One-directional: Detail is constructed FROM the prescription at
  `preflight.rs:85-91`; both doc comments name the mirror ("minus the prose" /
  "mirrored onto"). Intentional wire-vs-builder parallel, not copy-paste debt.
- Drift guard already exists: F2.3 test
  `optimize/mod.rs:1385 deflection_setup_locked_explanation_carries_target_stickout`
  asserts structured fields agree with the prose-derived numbers.

## Proposed cleanup
- P2: delegate — replace `lut_op_family_from`'s body with
  `crate::feeds::vendor_normalize::op_family_to_lut(family)` (or delete the
  helper and call it at `context.rs:345`). home: `feeds/vendor_normalize.rs`
  risk: low  proof test: `cargo test -p rs_cam_core -q
  lut_op_family_mapping_covers_all_variants` plus sentry
  `lut_resolver_census_a6`.
- P3 (optional, low value): `From<&DeflectionSetupPrescription> for
  DeflectionSetupDetail` in narrative.rs to make the `preflight.rs:85-91` copy
  mechanical. risk: low  proof test: existing F2.3 test.
- P1: none.
