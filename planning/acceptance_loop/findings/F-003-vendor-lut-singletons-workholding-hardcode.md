# F-003 — Three VENDOR_LUT singletons + workholding=Medium hardcode

- **Stage:** suggest
- **Severity:** high
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~300 LOC + tests)
- **Linked PRs:** —
- **Verified in round:** —
- **Source audits:** suggest-pipeline + unification
- **Closes:** F-012 (duplicate)

## Evidence

There are **three independent `VENDOR_LUT` singletons** in the workspace:

- `crates/rs_cam_viz/src/ui/properties/mod.rs:54` — used by
  `compute_feeds_for_op` (the GUI / `add_toolpath` path)
- `crates/rs_cam_core/src/session/compute.rs:1792` — used by
  `feeds_result_for_toolpath` (the post-set warning path)
- `VendorLut::embedded` reads inside various tests

The session/compute one **hardcodes `workholding_rigidity: Medium`**
when constructing its `FeedsInput` (line 1816), while the viz one
reads from `session.stock_config().workholding_rigidity`. So when the
user has the project on Heavy workholding, the Suggest button computes
one value and the post-set `feeds.feed_vs_lut.high` warning evaluates
against a different envelope — they disagree by the proportional
factor of (Heavy / Medium) rigidity.

Combined with `apply_feeds_result_to_op` (`viz/ui/properties/mod.rs:95`)
which never enforces `plunge ≤ feed` and never re-applies machine
clamp on stepover, the result is suggestions that land outside the
LUT envelope despite the LUT being consulted — exactly the pattern
we saw on V-carve (feed 3.8× LUT recommendation) and drop_cutter
(stepover 4× LUT recommendation) in round-01.

## Smoke evidence

- AS008 V-carve: default `feed_rate=800` flagged by post-set warning
  as 3.8× LUT recommendation
- AS013 adaptive3d: default `feed_rate=2500` flagged as 2.7× LUT
  recommendation
- AS014 drop_cutter: default `stepover=0.3` flagged as 4× LUT
- AS017 horizontal_finish: default `stepover=1.2` flagged as 6.7× LUT

All of these are the post-set warning correctly catching that Suggest
did not consult the LUT properly. The fix is to make Suggest consult
the LUT once, through the same code path, with the same workholding.

## Acceptance test

1. **Unit test** (must be in PR): create a project with
   `workholding_rigidity = Heavy` and tool 6mm 2F. Call
   `suggest_params(Pocket, …)` and capture the returned
   `feed_rate`. Then call `feeds_result_for_toolpath` on the same
   toolpath and capture its `recommended_feed_rate`. Assert the two
   are equal.
2. **Unit test**: mutate workholding from Medium to Heavy. Assert
   that the suggested `feed_rate` changes by the expected rigidity
   ratio.
3. **Unit test**: invariant — `suggest_params` must return
   `plunge_rate ≤ feed_rate` in all cases. Add a fuzz across all 22
   op kinds.
4. **Unit test**: invariant — `suggest_params` must return
   `depth_per_pass ≤ tool.cutting_length` for any tool/op pair.
5. **Smoke verification** (auditor's next round): post-set warnings
   from `feeds.feed_vs_lut.high` must agree with the values the
   Suggest All button computed for the same (op, tool, material,
   workholding) — i.e. no warning fires immediately after a fresh
   Suggest on covered LUT rows.

## Files

- **New:** `crates/rs_cam_core/src/feeds/suggest.rs` with
  `pub fn suggest_params(op_kind, tool, machine, material,
  workholding, lut, stock_ctx) -> OperationConfig`
- **Move into core suggest module:**
  `operation_feeds_hints` from `crates/rs_cam_viz/src/ui/properties/mod.rs:1051`
  (this also closes F-011)
- **Move:** `apply_stock_defaults` from
  `crates/rs_cam_core/src/compute/catalog.rs:1440-1474` into the new
  suggest module
- **Delete:** `crates/rs_cam_viz/src/ui/properties/mod.rs:54`
  `VENDOR_LUT` static
- **Delete:** `crates/rs_cam_core/src/session/compute.rs:1792`
  `VENDOR_LUT` static
- **Add:** one `crates/rs_cam_core/src/feeds/lut.rs::EMBEDDED_LUT`
  singleton accessed via a getter
- **Delete:** `compute_feeds_for_op` helper at
  `crates/rs_cam_viz/src/ui/properties/mod.rs:61` (replaced by
  `suggest_params`)
- **Delete:** `feeds_result_for_toolpath` helper at
  `crates/rs_cam_core/src/session/compute.rs:1783` (replaced by
  calling `suggest_params` then comparing)

## Fix shape

- Single `suggest_params()` function in core
- Single `EMBEDDED_LUT` singleton
- Function enforces 4 invariants before returning:
  1. `plunge_rate ≤ feed_rate`
  2. `stepover ≤ tool.diameter`
  3. `depth_per_pass ≤ machine.rigidity.doc_*_factor × tool.diameter`
  4. `depth_per_pass ≤ tool.cutting_length`
- Replace the three call sites:
  - `crates/rs_cam_viz/src/controller/events/toolpath.rs:135`
  - `crates/rs_cam_viz/src/app/mcp.rs:2446`
  - `crates/rs_cam_viz/src/ui/properties/mod.rs:1088` (the wizard /
    Suggest All button)

## Risk

Medium-large. Touches GUI flow. Three call sites must agree.

## Notes

- **Reaches into F-007** (drill `set_plunge_rate` is a silent no-op).
  The invariants in this fix are meaningless on drill until F-007
  lands. If F-007 ships in the same PR, that's fine; otherwise note
  that drill's invariant 1 is unenforceable until F-007.
- **Subsumes:** F-012 (the `feeds_result_for_toolpath` workholding=
  Medium hardcode) — close that finding when this lands.
- **Partially subsumes:** F-006 (three OperationConfig default paths)
  — the new `suggest_params` collapses two of the three; the CLI's
  `parse_job_file` path stays separate until F-005 merges the MCPs.
- **Out of scope:** new LUT rows. Expand LUT coverage later.
