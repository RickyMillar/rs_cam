# F-012 — `feeds_result_for_toolpath` hardcodes workholding=Medium

- **Stage:** suggest
- **Severity:** high
- **Status:** duplicate of F-003 — close when F-003 lands
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (closes when F-003 deletes this function)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

`crates/rs_cam_core/src/session/compute.rs:1783`
`feeds_result_for_toolpath` builds a `FeedsInput` to drive the
post-set `feeds.feed_vs_lut.high` warning. Line 1816 hardcodes
`workholding_rigidity: WorkholdingRigidity::Medium` regardless of
the project's actual workholding.

`crates/rs_cam_viz/src/ui/properties/mod.rs:61 compute_feeds_for_op`
(the Suggest button path) reads workholding from
`session.stock_config().workholding_rigidity`. So the two disagree by
the rigidity ratio.

## Acceptance test

Covered by F-003's acceptance tests (the Suggest output and the
post-set warning must agree on workholding-dependent values).

## Files

- `crates/rs_cam_core/src/session/compute.rs:1783, 1816`

## Fix shape

`feeds_result_for_toolpath` deleted by F-003. Until then, fixing the
hardcode in isolation would help but is wasted effort because F-003
removes the function entirely.

## Risk

None — closes with F-003.

## Notes

- **Duplicate of F-003.** Keeping as a separate finding only so the
  auditor can confirm the specific hardcode is gone when F-003 lands.
  Close this when F-003 verifies.
