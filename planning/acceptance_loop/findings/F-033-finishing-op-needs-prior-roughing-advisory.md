# F-033 — Pre-sim advisory: 3D finishing op without prior roughing pass

- **Stage:** suggest / diagnostic (user-experience feature, not a sim bug)
- **Severity:** low (UX win; not blocking any acceptance bar)
- **Status:** open — opened round-10 (2026-05-26) as a follow-up to F-032's reframe; not loop-critical
- **First found in:** round-10 (2026-05-26)
- **Effort:** M
- **Linked PRs:** —
- **Source audits:** F-032's round-10 implementer probe + round-10 audit verification

## Background

F-032 (round-09) originally framed AS015's deflection over-fire (0.434 Exceeds) as a transit-sample contamination bug. The round-10 implementer probe refuted that — the triggering sample was a **steady-state finishing cut at 75% radial WOC** on the scallop's path. AS015 ran the scallop op **solo on `ux_3d_terrain.toml` with no prior roughing**; the ball-nose's shank physically passed through ~38 mm of solid uncut stock above each cut. The 0.434 mm tip deflection was **real**, not a measurement artifact. The audit then ran AS015 with a prior AS013-style adaptive3d roughing pass; deflection dropped to **0.197 mm Within** (round-10 verification).

This means **the system was correctly flagging dangerous tool engagement** that would break a 3 mm ball nose in real machining. The user (or test smoke) ran scallop on unroughed stock, which is malpractice. The acceptance loop closed the bar by changing the test methodology to include a prior rough; the **system did not have a bug**.

## What's missing

The system **detected** the dangerous engagement (deflection gate Exceeds) but did **not warn the user pre-sim** that running a 3D finishing op without a prior roughing pass on a workpiece with significant stock-above-model is unsafe. The user has to:
1. Set up scallop/drop_cutter/waterline/pencil/horizontal_finish/project_curve
2. Generate (no warning)
3. Run a multi-minute simulation
4. Read the deflection Exceeds verdict
5. Realize they need a prior rough
6. Add adaptive3d, regenerate, re-simulate

A pre-sim advisory could short-circuit steps 2-5 with a static check: "This 3D finishing op has no upstream roughing toolpath; consider adding adaptive3d first."

## Detection logic

Static, no sim required. Fires when:

1. The current op kind is in the 3D finishing set: `Scallop, DropCutter, Waterline, Pencil, HorizontalFinish, ProjectCurve, ProjectCurve` (and any future 3D finishing ops).
2. The setup has NO upstream roughing toolpath enabled before this one. Upstream means earlier in the setup's toolpath ordering. Roughing means any of: `Adaptive3d, Pocket (with depth ≥ 0.5 × stock_thickness), Adaptive`.
3. The model has `mesh.bbox.max.z - model.lowest_z_in_region ≥ 0.5 × tool.diameter` — i.e. there's enough stock-above-model that the cutter's path will sweep shank through bulk material.

Output: a `Hint`-severity diagnostic on the toolpath with `category = workflow`, `id = workflow.finishing_op_needs_prior_rough`, and a `fix` payload that suggests adding an adaptive3d before this op (or links to a one-click "Add roughing pass before this" action in the GUI).

## Why this matters beyond the acceptance loop

- The acceptance loop's smoke cases (AS013, AS015) tested independent ops. In practice, users compose multi-toolpath workflows. The system should help them compose safely.
- 3D finishing ops are particularly risky because they're driven by surface geometry, not by stock state. The user can innocently set up scallop without realizing the workpiece needs prior clearance.
- This is **UX, not a sim correctness fix**. The sim correctly identified the problem; the system just needs to surface it earlier in the workflow.

## Files

Starting points:
- `crates/rs_cam_core/src/diagnostics/adapters/` — F-015 added precondition checks like `from_preconditions.rs`; F-033 fits the same shape (a new `from_workflow_composition.rs` adapter).
- `crates/rs_cam_core/src/session/compute.rs::precondition_context_for_toolpath` — the existing wiring for static precondition diagnostics; extend with workflow context.

Mirror sites (per the post-F-030 architecture, should be just the core path now):
- The MCP `add_toolpath` and `set_toolpath_param` envelopes' `diagnostic_delta` payloads already carry hint-severity diagnostics; F-033's hint will surface there.
- The GUI's setup-sheet diagnostic panel already renders Hint-severity items.

## Acceptance test

1. Load `test_data/ux_3d_terrain.toml` via `ProjectSession`.
2. Add scallop op (no prior adaptive3d).
3. Assert: a `Hint`-severity diagnostic with `id = workflow.finishing_op_needs_prior_rough` is in `precondition_diagnostics_for_toolpath(idx).filter(workflow)`.
4. Assert the `fix` payload references adding an upstream adaptive3d.
5. Add an adaptive3d toolpath at index 0.
6. Re-evaluate precondition diagnostics for the scallop.
7. Assert: the workflow hint is **not** in the list (the upstream rough satisfies it).

## Risk

M. The detection logic is simple, but the system has to make a judgement about what counts as "upstream roughing for this scallop's region of work" — a scallop that finishes only a tiny pocket may not need a full-stock rough; one that covers the whole model definitely does. Conservative initial implementation: any upstream `Adaptive3d / Pocket / Adaptive` enabled in the same setup satisfies the precondition. Refine if false-positives surface.

## Notes

- This is the **D option** from F-032's reframe. The auditor chose A (add prior roughing to AS015's smoke methodology) to close the deflection bar; F-033 carries the D scope as a separate UX-feature finding.
- **Cross-link**: F-015 (op-precondition static validation) is the closest prior art. F-033 extends the precondition framework to multi-toolpath workflow composition rather than single-op validity.
- **Not blocking** any acceptance bar. Schedule when the team has bandwidth.
</parameter>
</invoke>