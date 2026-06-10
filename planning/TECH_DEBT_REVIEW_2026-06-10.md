# Tech-debt review brief — observations from the defect-class cleanup

**Source:** noticed while implementing F2–F4 (`planning/DEFECT_CLASS_CLEANUP_2026-06-10.md`,
branch `defect-class/cleanup`, 2026-06-10). These are *observations with evidence*,
not yet investigations — each item names where it was seen so a review session can
start from concrete code.

Priority order = expected source of the next live-run incident.

## R1 — Optimizer candidate-evaluation isolation + generator robustness at extremes

- Confirmed crash: `cavalier_contours 0.7.0 pline_view.rs:507` assert via
  `polygon::offset_polygon` ← `adaptive::path::is_narrow_machinable` ←
  `generate_adaptive3d` ← `optimize::run_grid_strategy` (repro in the
  DEFECT_CLASS doc backlog — WANAKA Back Rough, one grid candidate).
- Structural issue: `evaluate_candidate` regenerates toolpaths on the LIVE
  session at parameters generators were never exercised at (search-space hard
  floors: 0.05 mm DOC / 0.05 mm stepover). A panic anywhere in the geometry
  stack kills the optimize run; restore depends on `BaselineRestoreGuard::drop`.
  Related backlog item: `mem::replace` placeholder session (MCP observability).
- Review shape: (a) decide candidate isolation (clone vs guard + catch_unwind at
  the candidate boundary), (b) param-fuzz generators at search-space extremes —
  the `param_sweep` harness sweeps sensible values, not optimizer extremes.

## R2 — Error semantics that lie

- `generate_adaptive3d` (and siblings in `compute/execute.rs`) map ANY generator
  error to `OperationError::Cancelled` via `.map_err(|_e| OperationError::Cancelled)`
  — real failures surface to the user as "operation cancelled". Pattern, not typo.
- `OperationError::Other(String)` stringly-typed errors make this easy to
  reintroduce. `RefuseReason` conflation already in the DEFECT_CLASS backlog.
- Review shape: grep `map_err(|_` in compute/, classify each swallow; consider a
  `Cancelled` check that only fires when the cancel flag is actually set.

## R3 — Toolpath id vs index confusion

- APIs mix `usize` index-into-`toolpath_configs` and `tc.id` freely:
  `generate_toolpath(index)` vs `SimulationCutSample.toolpath_id` /
  `tool_load_report().per_toolpath[].toolpath_id` (ids). Bit me during the F2
  WANAKA probe (Back Rough = index 1, id 4) — silent wrong-lookup class.
- Tools have a `ToolId` newtype; toolpaths don't. Review shape: introduce
  `ToolpathId` + audit every `toolpath_id == idx`-shaped comparison.

## R4 — Golden-number test pins vs spec assertions

- `wanaka_suggest_integration.rs` was repinned twice in one day (TP11 for F3,
  TP4/TP10 for F4): exact-value pins ("feed must land 6500–7500") mean every
  intentional behavior change requires a human to ratify a new number — the
  moment a wrong number gets approved is invisible.
- Review shape: classify each pin as (a) literature-backed (keep exact band, cite
  source like the litmatrix cells do) or (b) behavior pin (rewrite as spec
  assertion: which cap binds, which warning fires, monotonicity).

## R5 — Sim-type test-fixture sprawl

- Adding one field to `ChiploadVerdict::Within` touched 18 construction sites
  (F3.3); `SimulationCutSample`/`SimulationCutTrace` ~25-field fixtures are
  hand-copied in ≥5 test modules, many stamped `schema_version: 1` while the
  real schema is v5 (F1).
- Review shape: test-builder helpers (`SimulationCutSample::test_default()`-style
  or a builder in a shared test-support module) + decide whether schema_version
  in fixtures should track the real version.

## R6 — Name-string identity

- `MachineProfile::to_key()` dispatches on `name.contains("VFD")`/"Makita";
  `LookupResult.source_vendor` is `format!("{:?}")`. Cheap fixes; silent
  breakage on rename.

## Already tracked elsewhere (don't double-count)

- Four deflection-model implementations / 0.7·D closed form +36% bias (A4 +
  DEFECT_CLASS backlog — LUT side unified by F3.4, model side not).
- Magic numbers outside `SearchPolicy`; Face op `feeds_family: Pocket` stranding
  the 9 facing-bit LUT rows; 48 single-point LUT rows behind the shrink-only
  allowlist (per-source disposition pending); A2 minor findings (z_step hint
  asymmetry, plunge_rate provenance, CLI raw defaults).
