# rs_cam_core instructions

`rs_cam_core` owns the CAM engine, shared data model, generation, simulation,
feeds, diagnostics, project/session state and G-code export. Keep it GUI-free.
Read the root `CLAUDE.md` first for workspace-wide architecture and gates.

## Core contracts

- Mutate `ProjectSession` through `ProjectSession::apply(Command)`; commands
  return `Effects`, including the authoritative stale set.
- The toolpath IR separates planning from dressups/export. Do not make render
  or controller state part of the core model.
- A parameter/tool/model/stock/setup edit invalidates the affected cached
  result chain. Do not preserve an old result under new inputs.
- `None` and `0.0` are different for measured findings: `None` means not
  measured; `Some(0.0)` means measured clean. Preserve that distinction on new
  fields and adapters.
- A gate with no population did not prove a clean cut. Check sample count/range
  before treating `Within` as evidence.

## Simulation and diagnostics

- Read `ProjectSession::simulation_triage` before raw issue counts. Triage is
  the bounded, severity-ordered operational answer; raw air-cut issue streams
  are deliberately noisy.
- Air-cut thresholds use `air_cut_pct_of_total_runtime`; do not substitute the
  cutting-time denominator. Prefer absolute air-cut time for cross-arm
  comparisons.
- `average_engagement` is a comparative radial-WOC signal, not an absolute
  pass/fail metric. On non-flat tools it is based on the engaged radius, not
  the shank envelope.
- Chipload gate/heat-map quantity is **advance per tooth**
  (`effective_feed / (rpm * flutes)`), not dexel chip thickness.
- A `NotMeasurable` simulation metric must abstain; collision detection stays
  enabled.
- Rest chains require simulated upstream stock. The GUI and core builders have
  a deliberate phantom-prior-stock rule for the first pending rest op; retain
  its parity when changing admission logic.

## Tooling, feeds and drilling

- For a finish reach claim, read the reach-map grid note and measured area
  first. It is a top-down, rim-eroded surface measure; red at/below the
  discretisation floor is unresolved arithmetic, not automatic geometry.
- `stock_to_leave` is an intentional offset, never a reach-map tolerance.
- Suggest is the validated feeds application path; raw parameter writes are
  deliberate overrides and must stale results.
- A matched vendor band caps the rubbing floor. Where no row matches, any
  sub-Ø2 transfer/scaling conclusion is provisional.
- Drill gates model the R-plane-rooted emitted schedule. Gates read cutting
  geometry, not fed distance. Drill metrics live in `drill_summaries` /
  `drill_gates`, not radial engagement summaries.
- Tapered drill geometry remains an open safety limitation: drill gates use
  the envelope diameter. Do not claim a tapered drill is exonerated solely by
  those gates.

## Frames, heights and boundaries

- Preserve setup-local versus world-frame boundaries. In particular, drill
  removal uses the supplied `StockCutDirection`; never infer direction from
  the ordering of a hole's Z pair.
- A pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and
  `Waterline`; query `OperationType::honors_pinned_bottom_z()` rather than
  assuming a 2.5D operation follows it.
- Explicit-depth operations diagnose cuts below the stock bottom. Other
  semantics abstain, not pass.
- `ToolContainment::Inside` can fall back to unclipped geometry on boundary
  collapse; inspect `boundary_clip_dropped` before asserting containment.

## Tests and evidence

- Tests live close to the code they protect. The heavy core binaries are behind
  `heavy-tests`; the full core gate must run once per phase/commit gate.
- F-024..F-040 and literature-matrix sentries live in `tests/`. Run the named
  sentry when touching a covered invariant.
- Current code and sentries outrank historical descriptions. For incident
  evidence, search `planning/`; do not copy retired measurements into new
  product claims.
