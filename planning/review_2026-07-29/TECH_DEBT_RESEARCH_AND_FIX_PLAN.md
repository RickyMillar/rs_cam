# Radius-audit technical debt: research and fix plan

Date: 2026-07-29  
Basis: `planning/review_2026-07-29/RADIUS_AUDIT.md` and current HEAD `63d5e8b`  
Purpose: orchestration-ready plan for the technical debt exposed by the audit, ordered by risk from high to low.

## 0. Executive decision

Do this as a sequence of measured, independently reversible PRs. Do **not** perform a global `radius()` → `cusp_radius()` replacement and do **not** change the shared finish-surface resolution or scallop ring algorithm in the same PR.

The dependency order is:

```text
measurement contract + baseline sentries
        ├── tool-scale/reach semantics ──> pencil/rest routing fixes
        └── finish-surface experiment ──> generation resolution decision
                                              ├── classification optimisation
                                              └── scallop algorithm repair
                                                        └── max-rings retirement
polygon-offset root fix (parallel research, late merge)
documentation reconciliation (continuous, then final sweep)
```

Priority is not identical to execution order. Measurement naming and sentries are medium/low debt, but they must land early because every high-risk algorithm decision depends on an honest oracle.

## 1. Current state: do not redo work already landed

Commit `63d5e8b` closed these audit findings:

- corrected `cusp_radius()` documentation to distinguish cusp scale from reach/fit;
- fixed tapered-ball pencil bisector contact radius;
- added chunk-level cancellation polling to the fine classification grid;
- corrected the stale classification-vs-generation alignment documentation;
- added four fast tapered-cusp sentries;
- split the Wanaka diagnostic into combined and fixed-grid experiments;
- printed true-surface and projected-area figures side by side;
- retracted the invalid “313 / 482 = 65% recovered” and “residual is conditioning” claims.

Remaining gaps from that commit:

- no end-to-end tapered UnifiedFinish sentry;
- seven reach/routing sites still use the envelope radius or have an unresolved contract;
- generation surface remains shaft-resolution;
- classification remains expensive despite responsive cancellation;
- scallop still uses one sparse-sampled minimum stepover per ring and a compensating `max_rings` cap;
- `offset_polygon` still inflates vertices under repeated concave offsets;
- radius and area semantics remain represented largely as unnamed `f64` values;
- historical docs and test descriptions still need a systematic consistency pass.

## 2. Non-negotiable rules

1. **Instrument before changing behavior.** Every behavioral PR needs a before/after fixture that can falsify its hypothesis.
2. **One variable per experiment.** Grid resolution, planner dials, tool geometry, and strategy parameters must not move in one table row.
3. **Never compare unlike domains.** XY-projected area, 3D surface area, dexel-top area, residual column count, and removed volume are distinct metrics.
4. **Keep physical envelope conservative.** Collision, bbox padding, spatial queries, swept volume, and coverage margins continue to use the full cutter envelope.
5. **Reach is profile-dependent.** Do not substitute `cusp_radius()` at routing/fit sites without proving the resulting tool-clearance model.
6. **Wanaka is read-only.** Never save over `planning/airrun_2026-06-01/wanaka.toml`.
7. **Synthetic tests gate; Wanaka characterises.** Default CI must not depend on a minute-long project fixture.
8. **External algorithms require lineage.** Add primary sources and update `CREDITS.md` when adopting a published rasterisation, offset, fast-marching, or constant-scallop algorithm.
9. **Zero warnings remain mandatory.** Run workspace clippy with `-D warnings` before merge.
10. **Do not bundle broad consumers.** `offset_polygon`, `MillingCutter`, and finish-surface changes each have a large blast radius and require isolated commits/worktrees.

---

# HIGH PRIORITY

## H1. Establish explicit tool-scale and reach semantics

### Problem

`radius()`, `cusp_radius()`, and `engagement_radius(depth)` answer different questions but all return bare `f64`. Reach/fit is more complex than either tip or envelope radius because the full tapered profile can foul a wall above the tip.

### Research questions

1. For each affected decision, what geometric question is actually being asked?
   - maximum swept envelope;
   - tip-sphere cusp scale;
   - cutter width at axial engagement;
   - vertical clearance required at a lateral radius;
   - two-wall/tool-profile fit in a valley;
   - heuristic path scale only.
2. Is `engagement_radius(depth)` sufficient for each routing site, or is a profile query such as `height_at_radius(r)` / a two-wall clearance solve required?
3. What depth is available at each routing point?
   - branch median/peak rest depth;
   - centerline-local rest depth;
   - commanded stock-to-leave;
   - no depth at all.
4. Should a routing decision be conservative at the deepest point, representative at the median, or evaluated locally along the centerline?
5. Which current APIs should remain backward compatible, and which internal scalar parameters can be removed?

### Research deliverables

Create `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md` containing:

- a complete production `radius()` census for finishing, rest analysis, diagnostics, and their adapters;
- one row per site: current meaning, required meaning, available depth/profile inputs, intended API, and test oracle;
- diagrams for envelope, tip sphere, width-at-height, and valley fit;
- an explicit decision for the ambiguous large-arc narration threshold;
- an ADR recommendation: additive named methods versus typed millimetre newtypes.

Review existing precedents before inventing APIs:

- `MillingCutter::{radius,cusp_radius,engagement_radius,height_at_radius,width_at_height}`;
- `Adaptive3dParams::{tool_radius,envelope_radius}`;
- `ToolGeometryHint::engaged_diameter_at_doc`;
- `feeds/geometry.rs::tapered_ball_effective_diameter`.

### Preferred fix shape

Use a staged additive migration rather than changing 260 radius-related references at once:

1. Add clearly named semantic accessors or a small immutable query object, for example:
   - `envelope_radius_mm()`;
   - `cusp_radius_mm()` for curved-tip operations;
   - `engagement_radius_mm(depth)`;
   - `profile_height_mm(radius)` for clearance solves.
2. Keep `radius()` as a compatibility alias for envelope radius initially.
3. Migrate only the audited finishing/rest paths first.
4. Remove caller-supplied radius scalars where the callee already owns `&dyn MillingCutter`.
5. Consider strong newtypes only after the targeted migration quantifies ergonomics and churn.

Do not define a generic `feature_radius`; that recreates the ambiguity under a new name.

### Acceptance gates

- Unit matrix over Flat, Ball, Bull, TaperedBall, and V-bit.
- `ToolDefinition` delegation parity for every new accessor.
- Property: envelope radius is never smaller than any sampled profile width.
- Property: tapered engagement radius is monotonic with depth and bounded by the envelope.
- Existing tool assembly/collision tests remain byte-identical.
- No production behavior changes in the semantic-API PR.

### Scope and risk

- Primary files: `tool/mod.rs`, tool shape modules, `feeds/geometry.rs`.
- Size: M for research, M for additive API.
- Risk: high if behavior is changed; low if additive and parity-tested.
- Dependency: must precede H2 behavior changes.

---

## H2. Fix Pencil, UnifiedFinish, and generic rest routing semantics

### Problem

The remaining routing path passes shaft radius into width routing and offset fitting:

- `pencil.rs` rest-field routing yardstick and offset-pass count;
- `unified_finish.rs` rest routing, offset-pass count, derived stepover, and crease-own-region threshold;
- `compute/execute.rs` generic rest routing;
- `narrate.rs` large-arc threshold contract.

`RestFieldParams::pencil_radius` is especially misleading: it is a caller-supplied routing yardstick, while actual grid margin, trust erosion, and region dilation derive separately from the cutter profile/envelope.

### Research experiments

Run experiments before selecting formulas:

1. **Analytic valley matrix**
   - symmetric and asymmetric V valleys;
   - wall angles 30°, 45°, 60°, 75°, 85°;
   - valley half-widths spanning below-tip, tip-scale, cone-scale, and shaft-scale;
   - rest depths from 0.05 mm through the ball/cone transition;
   - Ball and Ø1-tip/Ø6-shaft TaperedBall controls.
2. **Routing outcomes**
   - pencil versus clearing classification;
   - number of offset passes that physically fit;
   - whether emitted offsets gouge or air-cut;
   - coverage of rest material;
   - bisector position error.
3. **Real fixture characterisation**
   - Wanaka centerline counts, clearing-region counts, offset-pass count, cutting length, rapid length, and residual columns;
   - hold detector grid, threshold, and source stock fixed.
4. **Candidate models**
   - envelope radius baseline;
   - cusp radius baseline, expected to overstate reach;
   - engagement radius at local/median/peak rest depth;
   - profile-clearance solve against valley width/depth.

The winning model must be conservative against gouging without suppressing reachable detail.

### Fix slices

Do not land all routing changes together.

#### H2.1 Rest-field routing API

- Rename/split `RestFieldParams::pencil_radius` to express routing only.
- Prefer deriving routing geometry from the cutter and branch depth data inside `detect_rest_valleys`.
- If local depth must survive extraction, extend `RestCenterline` with explicit statistics such as median/peak rest depth; audit project/debug serialization implications.
- Keep envelope-derived grid padding, erosion, and polygon reach-back unchanged.

#### H2.2 Width-capped offset passes

- Remove the redundant `cutter_radius: f64` argument from `centerline_cut_paths` if the cutter plus centerline depth can answer the fit question.
- Replace the current scalar fit equation with the selected reach model.
- Keep user-configured standalone Pencil `offset_stepover` user-owned.

#### H2.3 UnifiedFinish-derived pencil stepover

- Replace `cutter.radius() * 0.5` only after the reach experiment establishes whether tip, local engaged width, or another bounded value is appropriate.
- Add telemetry showing the derived stepover and why it was selected.

#### H2.4 Crease-own-region threshold

- Remove or rename the `tool_radius` scalar accepted by `finish_planner::decompose` for crease routing.
- This path is dormant in the current UnifiedFinish call; add a direct sentry that makes it live before changing it.

#### H2.5 Generic rest analysis

- Route through the same canonical reach policy as Pencil; no parallel formula in `compute/execute.rs`.

#### H2.6 Narration contract

- Decide whether “large arc” means relative to envelope or cutting feature scale.
- Keep `radius()` if envelope-relative, but document and test it.
- Change only if the user-facing diagnostic is explicitly feature-relative.

### Acceptance gates

- Differential synthetic tests where envelope, cusp, and reach models produce different answers.
- No new gouges/collisions on the analytic valley matrix.
- Reachable-detail coverage increases over the envelope baseline.
- Offset passes never exceed the count physically supported by the selected model.
- Standalone Pencil and UnifiedFinish use one policy implementation.
- Existing Ball behavior remains unchanged unless a separately justified correction is found.
- Wanaka before/after table reports routing counts and residual material; no unsupported strategy-value conclusion.

### Scope and risk

- Primary files: `rest_field.rs`, `pencil.rs`, `crease_paths.rs`, `finish_planner.rs`, `unified_finish.rs`, `compute/execute.rs`, `narrate.rs`.
- Size: L, split into 4–6 PRs.
- Risk: high; directly changes where the tool cuts.
- Dependencies: H1 and M1 measurement contract.

---

## H3. Decide and repair generation-surface resolution

### Problem

`build_finish_surface_with_cancel` still selects `(envelope_radius / 4).max(tolerance)`. For tapered tools that coarse field drives slope/curvature sampling, ring decimation, and chord probe spacing. Chord refinement fixes Z only along chosen chords, not XY placement or stepover selection.

The helper is shared by Scallop, RampFinish, and SteepShallow, so a global formula change is not an acceptable first experiment.

### Research harness

Add a resolution-explicit A/B harness before changing production defaults:

- use `build_finish_surface_with_cell_size_and_cancel` to pin only generation resolution;
- hold tool, classification, planner dials, boundaries, feeds, and all operation parameters fixed;
- compare envelope/4, cusp/4, tolerance-driven, and one intermediate cell size;
- run on:
  - synthetic narrow ridge;
  - synthetic narrow valley;
  - mixed-slope ribbon;
  - disconnected patches/holes;
  - Wanaka read-only characterisation.

Per operation:

- Scallop;
- RampFinish;
- SteepShallow;
- UnifiedFinish MidSteep scallop band;
- UnifiedFinish VerySteep waterline and Shallow raster as controls.

### Required metrics

- generation wall time and peak memory;
- move/segment count and minimum segment length;
- max/P95/P99 surface residual in the correct metric domain;
- commanded versus measured cusp-height distribution;
- standing material and deep over-cut columns;
- rapid collisions;
- ring count and `uncut_core_mm2`;
- slope/curvature disagreement against a pinned high-resolution reference;
- path fingerprint by region/band.

### Candidate architectures

Evaluate in this order:

1. **Explicit per-consumer resolution policy** — smallest safe refactor; remove hidden derivation from the shared helper.
2. **Split coverage and differential fields** — coarse envelope-aware coverage plus finer slope/curvature data.
3. **Local/adaptive slope sampling along rings/contours** — avoid a globally fine tapered drop-cutter grid.
4. **Globally cusp-scaled generation grid** — correctness-first fallback if it is the only model meeting quality gates.

### Fix sequence

1. Add `FinishSurfaceSpec` / `FinishResolutionPolicy` with explicit mode and cell size; preserve existing output.
2. Make Scallop, RampFinish, and SteepShallow select policy independently.
3. Land the experiment report and decision record.
4. Change one consumer at a time, starting with Scallop because it has the strongest fidelity instrument.
5. Re-run UnifiedFinish only after standalone strategy behavior is understood.

### Acceptance gates

- No hidden global formula change.
- Selected policy meets the operation's tolerance/residual target.
- No collision regression.
- Runtime/memory regression is reported, not hidden; >20% requires documented justification under the repository performance policy.
- Tapered-tool sentries fail if generation silently returns to shaft-scale where the chosen policy forbids it.
- Ball fixtures remain unchanged where the policy resolves to the legacy cell size.

### Scope and risk

- Primary files: `finish_setup.rs`, `slope.rs`, `scallop.rs`, `ramp_finish.rs`, `steep_shallow.rs`, `unified_finish.rs`.
- Size: M research harness, L implementation if fields split.
- Risk: high.
- Dependencies: M1 metrics; can research in parallel with H1.

---

# MEDIUM PRIORITY

## M1. Make measurement domains and provenance explicit

### Problem

The audit confused 3D face area with post-hysteresis XY polygon area. Similar ambiguity exists in names such as `area_mm2`, “coverage,” “standing material,” and region area. Without domain and resolution provenance, future A/B tables can be numerically correct and conceptually false.

### Research/inventory

Inventory every area/coverage value used by:

- `finish_planner` and `UnifiedFinishReport`;
- rest-field reports;
- scallop standing-material findings;
- simulation/deviation harnesses;
- MCP narration and GUI diagnostics;
- planning A/B scripts.

For each value record:

- domain: 3D surface, XY projection, dexel-top cells, stock volume, path length;
- stage: raw threshold, hysteresis, close, absorption, polygon extraction, simulation;
- resolution and cell area;
- units;
- whether holes/overlap can double-count;
- whether it is suitable for a ratio or percentage.

### Fix shape

1. Rename internal fields and table headers explicitly, for example:
   - `projected_xy_area_mm2`;
   - `surface_area_mm2`;
   - `residual_xy_cell_area_mm2`;
   - `removed_volume_mm3`.
2. Add a small provenance struct to diagnostic-only reports where useful:
   - source/grid mode;
   - cell size;
   - threshold/hysteresis stage.
3. Add conversion helpers only where mathematically valid; never infer 3D area from projected area at vertical faces.
4. Preserve serialized compatibility with serde aliases/defaults if public report schemas change; bump schema versions where required.
5. Ban bare “% recovered” unless numerator and denominator share domain, mask, stage, and resolution.

Strong area newtypes are optional. Start with precise names and typed provenance; only introduce newtypes if the inventory finds repeated cross-domain misuse.

### Acceptance gates

- Compile-time or unit-test proof that the Wanaka diagnostic cannot reconstruct the invalid 313/482 ratio from two generically named fields.
- Golden diagnostic output includes domain and stage.
- Existing UI/MCP consumers migrated or compatibility-preserved.
- One review checklist item added: “Are compared metrics in the same domain and stage?”

### Scope and risk

- Primary files: `finish_planner.rs`, `unified_finish.rs`, `rest_field.rs`, simulation reports, diagnostics/tests.
- Size: M.
- Risk: medium if serialized fields change; low if additive aliases are used.
- Dependency: start first because H2/H3/H5 need its oracle.

---

## M2. Complete tapered and shape-differential regression coverage

### Current coverage

`tapered_cusp_radius_sentry.rs` now covers radius/cusp separation, `ToolDefinition`, classification cell/padding, ribbon decomposition, and Ball equality. It does not yet cover end-to-end UnifiedFinish or the open routing/generation behavior.

### Fix plan

#### M2.1 End-to-end UnifiedFinish sentry

- Build a small synthetic mixed-slope mesh.
- Dispatch through `execute_operation()` or `ProjectSession`, not the pure decomposition helper.
- Assert at least one non-shallow region produces cutting moves.
- Assert spans/annotations remain valid.
- Use a tapered tool and a Ball control.

#### M2.2 Tapered operation matrix

Add focused tests for:

- Scallop;
- RampFinish;
- SteepShallow;
- Pencil RestDepth;
- UnifiedFinish claims on/off;
- generic rest-analysis attachment.

Each fixture must make envelope, cusp, and reach scales materially different.

#### M2.3 Parameter-sweep coverage

Add at least one tapered variant to the affected sweep families. Do not duplicate all 56 rows; select parameters whose outputs must differ under the intended scale.

#### M2.4 Differential controls

- same tip, different shaft;
- same shaft, different tip;
- same tool at shallow versus deep engagement;
- Ball behavior unchanged.

These controls are stronger than one expected move count because they isolate which dimension drives behavior.

### Acceptance gates

- Default synthetic sentries complete in seconds.
- Wanaka remains ignored/characterisation-only.
- Every bug-fix PR first demonstrates a red sentry against its parent commit.
- No test that can pass vacuously on empty output.

### Scope and risk

- Primary files: new tests plus `param_sweep.rs`.
- Size: M.
- Risk: low.
- Dependency: incremental; initial end-to-end sentry should precede H2/H3 behavior changes.

---

## M3. Optimise classification without changing its answer

### Problem

The fine true-surface classifier performs a tiny-ball drop-cutter query at every regular-grid cell. The 849² production row measured 19–47 seconds across machines. Cancellation is now responsive, but the computation remains expensive.

### Research candidates

Benchmark candidates against the current classifier as the oracle:

1. direct top-surface triangle rasterisation into regular cells;
2. vertical ray/topmost-face sampling using the spatial index without cutter contact math;
3. tile-based rasterisation parallelised by disjoint output tiles;
4. cached immutable classification fields keyed by mesh revision, bbox, cell size, and stencil mode;
5. adaptive refinement near thresholds, only after a regular-grid equivalent is proven.

Important edge cases:

- overlapping/stacked triangles: choose the topmost upward-facing surface consistently;
- vertical and near-vertical walls;
- holes and uncovered cells;
- non-manifold/open meshes;
- deterministic ties;
- max-gradient stencil behavior;
- cancellation and partial-result discard.

### Research deliverable

`planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md` with:

- algorithm descriptions and source lineage;
- equivalence results on analytic fixtures and Wanaka;
- 143²/425²/849² timing, CPU time, and peak memory;
- label-grid and planned-region diffs, not only height RMS;
- recommendation and rejected options.

### Fix sequence

1. Add criterion benches for current classifier at small synthetic sizes and a release-mode ignored real fixture.
2. Implement candidate behind an internal strategy enum/test hook.
3. Require exact or explicitly bounded classification-label equivalence.
4. Switch production only after deterministic parity.
5. Add cache only after mesh invalidation/revision semantics are proven.
6. Consider adaptive refinement only if direct rasterisation and caching are insufficient.

### Acceptance gates

- No loss of narrow steep regions relative to the current fine classifier.
- Same coverage semantics and no fabricated cliffs.
- Deterministic across thread counts.
- Cancellation latency remains bounded by a tile/chunk.
- Material speedup at 849²; target at least 2× before accepting architecture complexity.
- Memory budget documented; no unbounded cache retention.

### Scope and risk

- Primary files: `slope.rs`, `finish_setup.rs`, `mesh`/spatial helpers, benches.
- Size: L research + implementation.
- Risk: medium/high because classifier changes region ownership.
- Dependencies: M1 and M2; can run after H3's explicit policy refactor.

---

## M4. Repair the scallop algorithm instead of stacking compensations

### Problem

Current scallop behavior combines several compensations:

- one scalar stepover per ring;
- sparse sampling of roughly 20 points regardless of ring length;
- minimum sample controls the entire ring;
- flat-ground-derived `max_rings` truncates smaller-step cascades;
- raising the cap produced +92% time and 34× deep over-cut;
- chord refinement repairs Z along emitted chords, not ring placement;
- decimation contains repeated-offset vertex explosion.

### Research phase A: improve the oracle

Before changing the algorithm, extend existing fidelity instruments to report:

- local requested versus achieved cusp by ring segment;
- stepover distribution along each ring;
- sample coverage density and missed extrema;
- ring count by region and slope band;
- cap consumption and uncut core;
- minimum segment length/junction-cost distribution;
- deep over-cut and standing-material maps.

Use synthetic mixed-slope rings where one ring crosses flat, convex, and steep segments, plus the Wanaka characterisation.

### Research phase B: compare algorithms

Evaluate, in order:

1. **Segmented/local stepover rings** — split a ring into homogeneous spans, apply bounded local advances, and reconnect safely.
2. **Variable-distance polygon offset** — per-vertex/per-segment offset with self-intersection repair.
3. **Level-set / fast-marching iso-scallop field** — larger algorithm replacement; consider only with literature-backed implementation and a clear win.
4. **Median-ratio clamp** — benchmark only as a fallback; it knowingly violates cusp locally and cannot be accepted without quantified quality bounds.

The research must decide whether scallop remains an offset-ring algorithm or should become an iso-field contour extractor.

### Fix sequence

1. Replace fixed-20 sparse sampling with distance-bounded sampling or local field evaluation; behavior gate first.
2. Land the chosen local/variable stepover model.
3. Prove rings converge without relying on the legacy flat-ground cap.
4. Replace `max_rings` with a defensible termination invariant and a large emergency guard.
5. Keep `ScallopReport::uncut_core_mm2` until end-to-end sentries show zero on supported geometry; do not delete observability with the first green run.
6. Re-evaluate chord refinement and decimation after the new placement algorithm; remove only compensations proven redundant.

### Acceptance gates

- No truncation on supported synthetic and real fixtures.
- Achieved cusp stays within tolerance on every slope class, not only average.
- No increase in deep over-cut or collisions.
- Flat segments are not forced to the steep segment's minimum stepover.
- Runtime improves materially against uncapped-min behavior and does not regress shipped behavior without explicit sign-off.
- Minimum segment-length distribution remains compatible with machine acceleration/junction limits.
- Continuous and discrete modes both covered.

### Scope and risk

- Primary files: `scallop.rs`, `scallop_math.rs`, possibly new field/offset module.
- Size: XL algorithm project.
- Risk: high despite medium debt classification; isolated worktree required.
- Dependencies: H3 resolution decision, M1 metrics, and M2 tests.

---

## M5. Fix `offset_polygon` vertex inflation at the source

### Problem

Repeated concave offsets can grow vertices by 15–25% per ring. Scallop currently decimates after each offset, but the shared primitive is used by pocket, adaptive, rest, boundary, profile, trace, zigzag, inlay, and project-curve paths.

### Research

1. Reproduce vertex growth on:
   - captured Wanaka mid-steep polygon;
   - synthetic comb/dendrite polygons;
   - holes and multiple disjoint outputs;
   - near-degenerate short edges.
2. Determine whether growth comes from:
   - upstream CavalierContours arc tessellation settings;
   - repeated line/arc flattening;
   - duplicate/near-collinear vertices;
   - lack of topology-preserving simplification between offsets.
3. Compare:
   - exact arc preservation where available;
   - tolerance-bounded simplification;
   - collinear/near-duplicate cleanup;
   - alternate offset backend only if the existing backend cannot be repaired.

### Fix shape

- Add an explicit offset tolerance/policy rather than silently simplifying every consumer.
- Preserve winding, holes, containment, and offset direction.
- Make simplification error bounded in world units and lower than the calling operation's tolerance.
- Keep the current `offset_polygon` behavior as a compatibility policy until all consumers are audited.
- Migrate scallop first, then high-frequency clearing consumers.

### Acceptance gates

- Vertex count grows linearly or remains bounded across 200 repeated offsets on the stress fixture.
- Hausdorff/area error stays inside the selected tolerance.
- Existing degenerate-input and property tests pass.
- Parameter sweeps for Pocket, Adaptive, Rest, Profile, Trace, and Scallop show no unexplained topology changes.
- Criterion benchmark includes square, concave, holed, and repeated-cascade cases; the existing square-only benchmark is insufficient.

### Scope and risk

- Primary files: `polygon.rs`, consumers listed by `rg 'offset_polygon('`, tests/benches.
- Size: L.
- Risk: very high blast radius; separate worktree and late merge.
- Dependency: research can run in parallel; implementation should not overlap the scallop algorithm PR.

---

# LOW PRIORITY

## L1. Reconcile documentation and comments with executable behavior

### Problem

The repository has high-value historical records, but comments and planning prose can become stale as follow-up commits change experiments. Line-number references drift, diagnostics retain obsolete claims, and “fixed” can mean code fixed while the evidence narrative remains invalid.

### Plan

1. Create two concise ADRs:
   - `architecture/tool_geometry_semantics.md`;
   - `architecture/finish_surface_fields.md`.
2. Keep historical planning sections, but prepend explicit retraction/supersession notices rather than rewriting history silently.
3. Update stale terms:
   - “tool radius” → envelope/cusp/engagement/profile clearance as applicable;
   - “area” → projected/surface/residual domain;
   - classification/generation grid alignment;
   - “variable stepover” where implementation remains one scalar per ring.
4. Replace fragile line-number references with symbol/path references where practical.
5. Give each diagnostic test a contract header:
   - gate versus characterisation;
   - variables held fixed;
   - metric domains;
   - fixture mutability/read-only status.
6. Add a “superseded conclusions” index for §14 strategy claims invalidated by the radius and routing findings.
7. Update `FEATURE_CATALOG.md` only when user-visible behavior changes.
8. Update `CREDITS.md` whenever external algorithms/formulas are adopted.

### Acceptance gates

- `rg` audit finds no unqualified feature-scale `tool_radius` comments in the touched finishing path.
- Every ignored Wanaka diagnostic says what it does and does not prove.
- ADRs point to executable sentries.
- No product claim relies solely on a historical planning table.

### Scope and risk

- Size: M spread across the project.
- Risk: low, but stale docs can drive high-risk wrong fixes.
- Dependency: continuous updates per PR plus one final reconciliation pass.

---

# 3. Orchestration plan

## 3.1 Recommended work packages

| Package | Owner lane | Scope | Can run in parallel with | Must not overlap |
|---|---|---|---|---|
| WP0 | quality/measurement | M1 inventory, baseline fixtures, end-to-end sentry | WP1 research, WP2 harness | behavioral fixes |
| WP1 | tool geometry | H1 semantics research + additive API | WP0, WP2 | H2 implementation |
| WP2 | finish surfaces | H3 resolution A/B harness + explicit policy refactor | WP0, WP1 | M3 production switch |
| WP3 | routing | H2 experiments and sliced fixes | M3 research | WP1 until API lands; overlapping edits to Pencil/Unified |
| WP4 | classifier performance | M3 candidate implementations/benchmarks | WP3 | WP2 policy files during refactor |
| WP5 | scallop algorithm | M4 instrumentation/research | WP3, polygon research | H3 production changes; polygon implementation |
| WP6 | polygon offset | M5 isolated research/worktree | WP1–WP4 | WP5 merge window |
| WP7 | docs | ADRs and per-PR reconciliation | all | editing the same planning section concurrently |

## 3.2 Merge order

1. **PR-0: Measurement contract and fixture inventory**
   - M1 naming/provenance design;
   - no schema-breaking rename yet;
   - baseline synthetic fixture helpers.
2. **PR-1: Missing tapered end-to-end sentries**
   - M2.1 plus Ball controls;
   - prove red against relevant pre-fix commits where possible.
3. **PR-2: Tool-scale semantic API, no behavior change**
   - H1 additive methods/query object;
   - shape/delegation properties.
4. **PR-3: Explicit finish-surface policy, no behavior change**
   - H3 refactor preserving current cell sizes exactly.
5. **PR-4..PR-7: Routing fixes, one behavioral decision per PR**
   - rest routing;
   - offset fit/pass count;
   - Unified derived stepover/crease threshold;
   - generic rest + narration contract.
6. **PR-8: Generation-resolution decision and first consumer**
   - attach the A/B report;
   - change Scallop only.
7. **PR-9: Remaining finish-surface consumers**
   - RampFinish, then SteepShallow, then Unified orchestration.
8. **PR-10: Classifier optimisation**
   - production switch only after parity/quality gates.
9. **PR-11..PR-13: Scallop algorithm repair**
   - instrumentation/sampling;
   - local/variable stepover;
   - termination/max-rings retirement.
10. **PR-14: Offset-polygon policy/root fix**
    - broad sweep gate; merge late.
11. **PR-15: Final docs/schema cleanup**
    - complete names, ADRs, retractions, feature catalog/CREDITS as needed.

PR numbering is sequencing guidance, not a demand to force unrelated work into exactly 16 changes.

## 3.3 Checkpoint decisions

The orchestrator must stop for human review at these points:

### Checkpoint A — reach model selection

Required evidence:

- analytic valley matrix;
- gouge/coverage comparison of envelope, cusp, engagement, and profile-clearance candidates;
- proposed API and migration table.

No H2 behavioral change before approval.

### Checkpoint B — generation resolution

Required evidence:

- per-operation A/B with only cell size changed;
- quality, time, memory, and collision results;
- recommendation: global fine grid, per-consumer policy, split fields, or local sampling.

No shared-helper default change before approval.

### Checkpoint C — scallop algorithm

Required evidence:

- local cusp/stepover instrument;
- candidate comparison;
- termination proof/empirical bound;
- machine-time and segment-length effects.

No `max_rings` increase/removal before approval.

### Checkpoint D — offset primitive

Required evidence:

- consumer census;
- topology/geometry error bounds;
- repeated-offset complexity;
- full affected sweep results.

No default `offset_polygon` behavior change before approval.

---

# 4. Verification matrix

## Every PR

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Run the smallest affected tests first, then the relevant crate test set. Avoid workspace-wide `cargo test` on this repository.

## Fast targeted gates

```bash
cargo test -p rs_cam_core --test tapered_cusp_radius_sentry
cargo test -p rs_cam_core --lib tool::tests
cargo test -p rs_cam_core --lib scallop::tests
cargo test -p rs_cam_core --lib pencil::tests
cargo test -p rs_cam_core --test capability_link_moves_safety
```

Add named filters as new sentries land rather than running unrelated suites during inner loops.

## Behavioral sweep gates

```bash
cargo test -p rs_cam_core --release --test param_sweep -- --ignored
```

For broad `offset_polygon` changes, explicitly inspect affected operation-family fingerprints, not just the aggregate pass count.

## Performance gates

Extend `crates/rs_cam_core/benches/perf_suite.rs` with:

- classification at multiple grid sizes;
- direct-raster candidate if pursued;
- finish-surface generation for Ball and TaperedBall;
- scallop mixed-slope cascade;
- repeated concave/holed polygon offsets.

Record CPU, wall time, peak memory, move count, and output-quality metrics. A faster wrong label/path is a failed benchmark.

## Real-project characterisation

Use ignored release tests and/or live MCP only after synthetic gates pass:

- load Wanaka read-only;
- capture exact project/toolpath state and simulation resolution;
- regenerate after any simulation/tool change;
- report collisions, residual distributions, routing counts, cutting/rapid length, and runtime;
- do not establish general strategy value from Wanaka alone.

## Final merge gate

Run the repository `/verify` workflow and update:

- `planning/PROGRESS.md` for significant shipped work;
- `FEATURE_CATALOG.md` for visible capability/behavior changes;
- `CREDITS.md` for adopted external algorithms or formulas.

---

# 5. Definition of done

This programme is complete when:

1. every finishing/rest radius use has an explicit semantic contract;
2. reach/fit decisions use a validated profile/depth-aware model rather than shaft or tip by accident;
3. tapered tools have fast end-to-end sentries across affected operations;
4. finish-surface resolution is explicit per consumer and quality-gated;
5. classification is acceptably fast without changing region ownership or narrow-feature detection;
6. scallop achieves local cusp targets without ring-wide minimum collapse or routine cap truncation;
7. repeated polygon offsets have bounded vertex growth and geometry error;
8. diagnostics name metric domain, stage, and resolution, preventing cross-domain percentages;
9. historical docs clearly mark superseded conclusions and point to executable evidence;
10. all changes pass zero-warning clippy, focused tests, sweeps, and read-only real-project characterisation.

## Explicitly out of scope

Do not absorb unrelated open items from `planning/finishing_stack_review_2026-07.md` such as stock-to-leave additions, MCP export issues, project invalidation, or general operation consolidation unless a selected fix directly requires them. Track newly discovered adjacent defects separately rather than expanding this programme mid-PR.

---

# Addendum A — campaign findings not covered by the base plan

Added 2026-07-29 after `63d5e8b`. The base plan was derived from
`RADIUS_AUDIT.md`, which scoped itself to the radius defect class. This
addendum adds the work the wider §14 campaign
(`planning/unified_v3_design.md`) leaves open and that no section above
claims. Verified absent from the base plan before adding: intra-node
retract round trips, `claims_reference` default, UnifiedFinish semantic
annotation, standing-material channel, descent/resolution coupling, and
re-measurement of the invalidated conclusions.

## A.0 Correction to §1 "current state"

One finding from reproducing the audit is **not** in the audit and changes
what H2/H3 should expect (design doc §14t, item 4).

The base plan reads the radius defect as "the dials erased steep terrain."
That is wrong. Measured on ONE pinned fine grid:

| | VerySteep |
|---|---|
| coarse grid + shaft dials (the real "before") | 0 regions / 0 mm² |
| fine grid + shaft dials | 1 region / **423 mm²** |
| fine grid + tip dials (the "after") | 10 regions / 313 mm² |

At shaft scale on a fine grid the decomposition produces **more** steep
area, not less — the 1.5 mm close MERGED the ribbons into one blob and the
144 mm² floor then kept it. Therefore:

- **cell size drove the existence recovery; the dials bought STRUCTURE**
  (1 blob → 10 regions), not area;
- **area is not a safe invariant for these fixes** — its direction flips
  between fixtures (synthetic ribbon: the floor swallows it; wanaka: the
  close merges it). Region count / topology is the invariant that holds in
  both. Any H2/H3 acceptance gate phrased as "steep area must increase" is
  unsound; phrase gates on region count, coverage and residual instead;
- H3's A/B must report label-grid topology, not areas alone.

This also means §14q's "15% recovered by the dial fix alone" credited the
dials with the grid's effect. No experiment crossed the two until now.

---

# HIGH PRIORITY (addendum)

## H4. Re-measure every conclusion produced through the broken instruments

### Problem

This is the largest outstanding item in the programme and the base plan
does not schedule it. Four instrument defects were stacked underneath the
entire §14 strategy campaign:

1. classification grid 6× too coarse (fixed, `32c5e48`);
2. planner dials 6×/36× too large (fixed, `5732f57`);
3. `RestFieldParams::pencil_radius` = shaft radius — the valley detector
   was asked where a **3 mm** tool cannot reach, for work the **0.5 mm**
   tip would do (**still open — H2.1**);
4. `claims_reference: self_probe` — analytic rest, never consults stock
   (identified §14p; **still the shipped default** — A/M6 below).

Every strategy verdict measured on top of those is **void, not
falsified**: the comparison could not have come out any other way.
Specifically NOT established, despite being asserted in shipped planning
prose:

- "contour and pencil have nothing to do on this part";
- "scallop wins every time";
- the band-mix tables in §14, §14a and §14m–§14p;
- the mm²/s efficiency comparison (Op B 0.476 vs D 0.938) — measured with
  defect 3 live, so the rest pass's territory was mis-drawn;
- the v3 process-proof closure ("cascade +25% slower at shipped dials").

The v3 closure had **independent** reasons that still stand — the fixture
is a coarse TIN (1.8% of triangles carry 40.8% of area), and the ±10 µm
gate bin is below repeatability and aliased by the 0.25 mm grid. Fixing
the instruments does not resurrect the campaign. It means a negative
result must be **re-earned rather than assumed**.

### Required sequencing

Re-measurement is worthless until the instruments it depends on are fixed.
This order is mandatory:

1. H2.1 (rest routing radius) — without it any pencil verdict is
   meaningless;
2. A/M6 (`claims_reference` default) — without it cascade measurements
   re-cut the whole part;
3. H3 Checkpoint B decision — the generation grid decides achieved cusp,
   the quality half of every speed/quality trade;
4. only then re-run the strategy comparison.

### Deliverable

`planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`: one row per
invalidated claim — origin section, which instrument defect invalidates
it, whether it has been re-measured, and the new result or `NOT RE-RUN`.
L1.6 asks for this index; this section defines its contents and makes
producing it a gated work item rather than a documentation chore.

### Acceptance gates

- No planning document asserts a strategy verdict that is not either
  re-measured post-fix or explicitly marked superseded.
- Re-runs use a fixture chosen for the question. **Do not re-run on wanaka
  alone** — it defeated the v3 gate for reasons unrelated to strategy
  choice. Add at least one fixture with genuinely separable steep/shallow
  territory.
- Each re-run states measure, threshold, stage and resolution for both
  sides of any ratio (M1's rule).
- **Render the surface.** The v3 closure's standing rule: *never gate on
  an aggregate without rendering the surface* — that gate ranked an
  operation above one that left a 28 mm uncut block.

### Scope and risk

- Size: M per re-run, L overall.
- Risk: low technically, high organisationally — stale verdicts steer
  roadmap decisions until they are replaced.
- Dependencies: H2.1, A/M6, H3 Checkpoint B.

---

# MEDIUM PRIORITY (addendum)

## A/M6. `claims_reference: self_probe` is a shipped default footgun

### Problem

`self_probe` derives rest **analytically** — "where can my own cutter not
reach the model" — and never consults simulated stock. In a same-tool
cascade it names precisely what the tool cannot fix, so the rest op
re-cuts the whole part. Measured live on wanaka, single variable, 0.1 mm
sim, after a full same-tool finish:

| `claims_reference` | cutting | rapid | moves |
|---|---|---|---|
| `self_probe` (**the default**) | 46 366 mm | 182 843 mm | 140 477 |
| `machined_stock` | **5 259 mm** | 25 231 mm | 26 220 |

**−88.7% cutting from one dial.** R2 had already measured the analytic
reference overestimating ~14× in this configuration. It is still the
default and nothing warns.

### Fix shape

1. Decide whether the default should flip. `self_probe` is defensible for
   a *first* finish op with no prior stock and indefensible for a rest op
   in a cascade — the choice is contextual, so prefer **deriving** it: if
   the operation has a machined-stock prior in scope, use it.
2. If the default cannot flip safely, emit a **loud** finding when a rest
   op runs `self_probe` while a machined prior exists.
3. Surface the prerequisite in the rest-analysis config UI and MCP.

Note the F.4 ladder interaction: a disabled op has no prior-stock
snapshot, so `enable → simulate → generate` is required before
`machined_stock` is even available. The silent fallback is what makes the
current behaviour invisible.

### Acceptance gates

- Cascade sentry: same tool, finish then rest; the rest op's cutting
  length must be a small fraction of the finish op's.
- No silent fallback — selecting or falling back to `self_probe` when a
  machined prior exists must produce a user-visible finding.
- Ball and tapered controls.

### Scope and risk

- Primary files: `rest_field.rs`, `unified_finish.rs` (`ClaimsConfig`),
  `compute/execute.rs`, MCP `set_rest_analysis_config`.
- Size: S–M. Risk: medium — flipping a default changes existing projects.
- Dependency: **blocks H4**.

## A/M7. Intra-region retract round trips — the real efficiency lever

### Problem

The finishing efficiency work mis-sized its own prize. Region-node
reordering was built to cut rapid distance; it delivered −67.6% hop
distance for only **−4.6%** time, because the cost is **count-bound**, not
distance-bound:

- 15 311 of 15 363 retract round trips are **inside a single region node**
  — node reordering cannot touch them;
- 0.79 trips/mm² for the rest pass vs 0.028 for the all-over pass;
- area-normalised throughput 0.476 mm²/s vs 0.938 — the rest pass is
  **half as efficient per unit area finished**;
- estimated headroom ~**−30%**.

Use **mm²/s (area finished per second)** as the metric: it is invariant to
tool and stepover, unlike cutting length or wall time.

### Research

1. Classify the intra-node trips — band crossings, ring-to-ring
   transitions, sliver-driven lifts, or clearance-plane defaults.
2. Determine which can become **surface links** (feed moves at or near the
   surface) rather than retract/rapid/plunge cycles — the same fix class
   the span defect exposed.
3. Check the interaction with descent planning (A/M10): a link that stays
   low is safe only if the descent model is resolution-honest.

### Acceptance gates

- Trip **count** reported per toolpath, not only rapid distance.
- mm²/s improves on both a rest pass and an all-over pass.
- No new collisions at the **finest** sim resolution, not the default.
- Link moves preserve material state — extend
  `capability_link_moves_safety`.

### Scope and risk

- Primary files: `unified_finish.rs`, `tsp.rs`, link/lead generation,
  `ToolpathStats`.
- Size: M–L. Risk: medium/high — changes where the tool travels between
  cuts.
- Dependency: A/M10 for the safety half; M1 for the metric.

## A/M8. UnifiedFinish emits no semantic annotation

### Problem

`narrate_toolpath` reports `regions 0` for UnifiedFinish. The structural
span system (region nodes and `move_range` tiling — fixed in `77f2b7a`,
coverage 12.6% → 100%) and the **semantic** trace consumed by narration
are independent systems, and only the first was implemented. The
agent-facing diagnostic therefore cannot see the band/strategy structure
of the one operation whose entire premise is mixing strategies.

This directly blocks H4: the mix table that would adjudicate "which
strategy earned its time" is exactly what is missing.

### Fix shape

Emit semantic region annotations from `unified_finish` alongside the
structural spans, matching what `steep_shallow` already produces
(`steep_shallow_semantic_trace_splits_steep_and_shallow_regions` is the
working precedent).

### Acceptance gates

- `narrate_toolpath` reports non-zero regions with band and strategy
  labels, on a tapered and a ball fixture.
- Labels reconcile with the structural spans — same count, same ranges.
- A sentry asserts the two systems agree so they cannot drift apart again.

### Scope and risk

- Primary files: `unified_finish.rs`, `narrate.rs`, viz worker trace.
- Size: S–M. Risk: low.
- Dependency: none. **Do this early** — it is cheap and H4 needs it.

## A/M9. Standing material has no diagnostic channel

### Problem

`uncut_core_mm2` exists on `ScallopReport` / `UnifiedFinishReport` but is
tracing-only: it never reaches `ToolpathStats`, so there is no gate, no
GUI surface and no MCP field. The v3 campaign's standing rule came from
exactly this gap — *a warning nobody sees is not a warning* — and its
closure found a gate ranking an operation above one that left a 28 mm
uncut block.

M4 lists standing-material maps as scallop instrumentation; this item is
the **plumbing** that makes any such measure visible, which M4 assumes and
does not schedule.

### Fix shape

Add a standing-material slot to `ToolpathStats` and wire the ~5
construction sites; surface it in narration, the GUI diagnostics panel and
`get_toolpath_diagnostics`. Prefer one honest measure with declared domain
and resolution (M1) over several approximations.

### Acceptance gates

- A deliberately-truncated cascade produces a non-zero, user-visible
  standing-material figure.
- The figure declares domain, stage and resolution.
- Gate wiring is opt-in initially — report before enforcing.

### Scope and risk

- Primary files: `ToolpathStats` and its construction sites, `scallop.rs`,
  `unified_finish.rs`, `narrate.rs`, diagnostics adapters, MCP.
- Size: M. Risk: low. Dependency: M1 naming.

## A/M10. Descent planning is resolution-dependent

### Problem

Collision counts depend on **simulation cell size**, not on the toolpath:
the same path sweeps 0 / 15 / 20 rapid collisions at 0.5 / 0.25 / 0.1 mm.
Mechanism: `optimize_entry_descents` takes its ceiling from the coarse
generation-time stock snapshot, while a fine grid resolves thin ridge tops
and uncut slivers between rough passes that the coarse grid averages away.
The residual cases are descents through slivers that **only exist on fine
grids**, so padding the descent target cannot bound them.

This matters more after the radius fixes, not less: the classification
grid is now 6× finer, so the gap between what planning sees and what a
fine simulation sees has moved.

Standing rule already in force: **never clear collisions across mismatched
resolutions.**

### Research

1. Does the descent ceiling need the snapshot at the *simulation's*
   resolution, or a conservative sliver-aware model that is
   resolution-independent?
2. Can descent targets be validated against the same field the collision
   check uses rather than a generation-time copy?
3. What does it cost to raise snapshot resolution only along descent
   candidates rather than globally?

### Acceptance gates

- Collision count is **stable across sim resolutions** on a fixed
  toolpath. This is the real gate and no current test asserts it.
- No regression in entry time from the serpentine/descent work.
- A sentry pins the resolution sweep so the coupling cannot return.

### Scope and risk

- Primary files: descent/entry optimisation, dexel snapshot plumbing,
  simulation collision check.
- Size: M–L. Risk: medium.
- Dependency: interacts with A/M7 — diagnose before fixing either.

---

# LOW PRIORITY (addendum)

## A/L2. Unexplained: the Rivers 6.07 mm axial-DOC spike

A finish pass reports a 6.07 mm peak axial DOC where the commanded value
is a small fraction of that. Arc fitting was the leading hypothesis and is
**exonerated** — identical value and position with `arc_fitting` off. That
is the fourth time on this subsystem that a confirmed mechanism turned out
not to be the cause.

Treat this as an open diagnosis, not a fix: instrument to localise *where*
(which move, which region, which Z level) before proposing *why*. The
engagement-histogram precedent applies — the instrument that localises
beats the argument that explains.

Size: S to instrument, unknown to fix. Risk: unknown — a 6 mm axial
excursion on a finishing pass is a crash-class symptom if real, so do not
close it as noise without evidence.

---

# Addendum C — structural safety work promoted from session observations

Added 2026-07-30 at the operator's direction. Raw observations:
`ANTIPATTERNS_BACKLOG.md` (same dir). These are now scheduled work items.
Theme: **use the type system to make the defect classes unrepresentable**,
not to patch instances. Rulings: the operator explicitly wants safety and
reliability built into the structure; prefer compile-time enforcement over
convention wherever Rust allows it.

## C1. Transform provenance contract (HIGH — blocks A/M7)

Problem: post-generation transforms (dressups, clip, descent optimizer) each
build and consume their `MoveRemap` privately; index-carrying channels
(spans, semantic trace) are remapped only because ae10cb2's
`SemanticLinkCarrier` piggybacks — the next channel or transform silently
breaks again.

Fix shape (typestate): transforms return `Transformed<Unreconciled>`
(`#[must_use]`); the only way to the inner `Toolpath` is
`.reconcile(channels: &mut ReconcileSet) -> Transformed<Reconciled>` where
`ReconcileSet` borrows every registered index-carrying channel (spans,
semantic trace, future ones implement a sealed `RemapConsumer` trait).
Forgetting a channel or skipping reconcile = compile error. Retire
`SemanticLinkCarrier` once migrated; keep its sentries green as the oracle.
Per-dressup semantic items claiming `0..len` and the debug-only viz recorder
path are cleaned up in passing.

Gates: ae10cb2's sentries unchanged; a doc-test or trybuild compile_fail
proving an unreconciled toolpath cannot be extracted; all transforms
migrated in one wave (small blast radius per transform, ~5 call sites known).
**Must land BEFORE A/M7** (motion economy adds/changes link transforms — it
must be born under the contract).

## C2. Retire silent sentinels (HIGH — blocks M3, protects steep_shallow NOW)

Problem: `0.0`/bbox-floor meaning "not measured / not covered" keeps causing
real defects — `min_z()` returning the padded grid's mesh-bbox floor on
uncovered cells is why PR-8b's clamp fires on a flat plane, and every other
`min_z()` consumer is unaudited (steep_shallow ladder bottom included).
`axial_doc_fraction == 0.0` = unknown; footprint fraction 0.0 = silence.

Fix shape (sum types): grid reads return `enum GridZ { Covered(f64),
Uncovered }` (or `Option<f64>` where the enum is overkill) so consumers must
decide uncovered-ness at the call site; sweep every f64 field where zero is
semantically "missing" → Option or a documented, tested contract. Follow the
A/M9 precedent (standing material) which already proved the pattern.

Gates: red-first on a steep_shallow ladder-bottom uncovered-cell fixture;
clamp behavior on covered cells byte-identical; the sweep list itself
recorded in the commit body (which fields converted, which documented).

## C3. One implementation per concept (parity sentries first)

- Tapered width models: unify on `MillingCutter::width_at_height`;
  `feeds/geometry.rs::tapered_ball_effective_diameter` (straight cone, ~5%
  off) gets a parity sentry FIRST, then either fixes or documents its
  deliberate approximation. Must precede M4 touching width math.
- CLI's parallel `ToolpathDiagnostic`: derive from the core struct (serde
  view), keep wire keys stable.
- `waterline.rs` / `compute/execute.rs` private copies of finish_setup
  pieces (Z-ladder, slope-window sentinel): extract to the shared module;
  waterline inherits PR-8d's segment floor in the same move.

## C4. Typed vocabularies (feeds H4)

Third `RegionSpanRole` for drill hole-vs-peck (kill label parsing); migrate
the two harness `"Pencil claims"` label keys to role queries; give
`ToolpathSemanticParams` a typed key layer (`enum SemanticKey` with
`as_str()`, constructors take the enum — JSON wire unchanged). H4's mix
tables must be built on roles, never labels.

## C5. Box `ComputeMessage::Toolpath` (mechanical, do first)

Three waves each paid the 280-byte `large_enum_variant` tax. One commit,
~16 sites, ends it.

## C6. Shared test-fixture library (blocks M4 comfortably, helps M3)

`tests/common/`: mixed-slope/groove/plateau mesh generators, tool builders,
ProjectSession + 17-field ToolpathConfig builders, FNV fingerprint helper —
extracted from `standing_material_channel_am9.rs` / M2.1 / the checkpoint
harnesses, which all copied each other. New tests import; existing tests
migrate opportunistically, never in bulk.

## C7. Mega-harness policy

`v3_cascade_ab.rs` / `p2c_headless_ab_wanaka.rs`: split reusable wanaka
loaders into a support module; the campaign-analysis bodies either get a
scheduled characterisation cadence or are archived as docs with their prose
claims marked historical. Also: §A.0-style topology gates need a
minimum-component/annulus-aware reading (Checkpoint B found a 45° annulus
fragmenting into 39 components at fine reference resolution).

## C8. Findings representation

`GenerationFindings` single slots → collections (two derivations must not
first-writer-win); partial height clipping reported (not just total
collapse); `regions 0` closed for Trace/Scallop/SpiralFinish (A/M8 pattern);
RampFinish standing-material area channel.

## C9. Model debt (evidence-gated, post-live-validation)

- Sampled-cross-section reach: erode against the real `surface_z`
  cross-section (no wall-angle model) — strictly generalises CLR+θ; recorded
  in `reach.rs` module doc. Needs the same matrix gates as Checkpoint A.
- Per-point claims fan (`centerline_cut_paths` scalar retirement).
- Interval index for `tsp::remap_spans` (O(spans×moves) today).
- Rest-grid resolution vs Ø1 tip (H3's question applied to the rest field).

## C10. Hygiene one-offs (mechanical, do first)

One deliberate whole-repo `cargo fmt` commit (nothing else in it);
`.gitignore` unanchored-entry audit (the `diagnostics/` incident class).

## C-sequencing (integrated with the remaining base plan)

After the wanaka live validation report lands:

1. **C5 + C10** — one mechanical wave, zero risk, ends two recurring taxes.
   ✅ DONE 2026-07-30: 288b19b (fmt) / 88ea12f (C5 box) / 9e43b78
   (.gitignore — found and fixed a SECOND live instance: unanchored
   `.claude/` had eaten `sim-diagnostics.md` + `sim-analysis/SKILL.md`).
2. **C1 provenance contract** — before any motion/link work.
   ✅ DONE 2026-07-30: 61bd97c (invariance harness) / 77267ce (typestate
   contract, SemanticLinkCarrier retired, 15/15 fingerprints identical) /
   60e2c0f (true-range attribution + unconditional reconcile).
3. **C2 sentinel sweep** — before M3 (classifier reads grids) and to close
   the live steep_shallow exposure.
   ✅ DONE 2026-07-30: ce426d6 (GridZ + private grid storage + 17-row
   consumer audit — steep_shallow VERDICT: NOT defective, bbox floor is
   intended for the waterline wall ladder; red-then-green proof in the
   commit body) / 96bc300 (sentinel sweep: 4 converted, 2 documented+tested,
   8 ruled out).
3a. **A/M12 → A/M11** (Addendum D, WP8 lane) — MCP non-blocking
   cancel/status first, then the `generate_all` fixpoint (D's own ordering
   rule: never ship an unobservable, unabortable loop). Slotted here
   because every later wave that needs live wanaka validation pays their
   tax until they land; no dependency on C1/C2.
4. **C6 fixture library** — before M4's fixture-heavy work.
5. **A/M6** `claims_reference` (base plan, unchanged position).
6. **M3 classifier** (uses C2+C6).
7. **C3 + C4 + C8** — consolidation/diagnostics wave (feeds H4's oracles).
8. **M4 scallop** (Checkpoint C; uses C3 width parity + C6).
9. **C9** reach generalisation (own evidence pack, matrix-gated).
10. **A/M7 + A/M10** motion economy (after C1, per its gate).
11. **M5 offset_polygon** (Checkpoint D). 12. **H4** (Checkpoint E, last).
13. **L1** final docs sweep (absorbs the backlog doc).

---

# Addendum B — orchestration updates

## B.1 Additional work packages

| Package | Owner lane | Scope | Can run in parallel with | Must not overlap |
|---|---|---|---|---|
| WP8 | diagnostics plumbing | A/M8 semantic annotation, A/M9 standing-material channel | WP0–WP2, WP6 | WP3 edits to `unified_finish.rs` |
| WP9 | rest defaults | A/M6 `claims_reference` derivation + warning | WP1, WP2 | WP3 rest-routing PRs |
| WP10 | motion economy | A/M7 intra-node trips, A/M10 descent/resolution | WP4–WP6 | WP3 link/lead edits |
| WP11 | re-measurement | H4 ledger and re-runs | docs | any lane still landing behavioral change |

## B.2 Merge-order insertions

Slot into the base plan's sequence:

- **PR-1a (with PR-1): A/M8 semantic annotation.** Cheap, no behavioral
  change, unblocks the strategy-mix reporting H4 needs.
- **PR-1b: A/M9 standing-material channel.** Same rationale, report-only.
- **PR-3a (after PR-3): A/M6 `claims_reference`.** Before any routing PR,
  so routing experiments are not measured through the analytic reference.
- **PR-7a (after routing): A/M10 descent/resolution diagnosis**, then
  **PR-7b: A/M7 intra-node trips.** Both need routing settled first.
- **PR-15a (last): H4 re-runs and the superseded-conclusions ledger.**
  Must be last — it measures the finished stack.

## B.3 Checkpoint E — re-measurement readiness

Before any H4 re-run the orchestrator stops for human review with:

- confirmation that H2.1, A/M6 and Checkpoint B have landed;
- the fixture choice, and why wanaka alone is insufficient;
- the metric contract per M1 — domain, stage and resolution for both sides
  of every ratio;
- rendered surfaces accompanying every aggregate.

No strategy verdict may be published from a run that skipped this.

## B.4 Live-validation prerequisites

Any MCP/GUI characterisation run must first:

1. **Rebuild** — `cargo build --release -p rs_cam_viz --bin rs_cam_gui`,
   confirmed complete BEFORE connecting. `.mcp.json` launches via
   `cargo run --release`, so a cold build blows the 30 s connect timeout
   with no error that points at the build. Do not trust the binary's
   mtime; run the build.
2. **Restore project state** — the GUI holds session-only parameter
   overrides that are not on disk (design doc §14s). Only
   `claims_reference: machined_stock` is a keeper; the rest were
   experiment values, and `min_rest_depth_mm` / `waterline_threshold_deg`
   were specifically shown NOT to be the lever.
3. **Match sim resolution to the tool** — cell size must be well below the
   finishing tool's TIP radius (0.1 mm for a Ø1 tip). A 0.5 mm cell is the
   size of the whole cutting tip and cannot hold ~20 µm cusps.
4. **Regenerate rest ops after re-simulating** — a new simulation does NOT
   mark toolpaths stale (§14o), so a rest op will happily report
   `status: Done` against stock that no longer exists.
5. **Never save over** `planning/airrun_2026-06-01/wanaka.toml`.

## B.5 Additions to the definition of done

11. every strategy conclusion in the §14 body is either re-measured
    post-fix or explicitly marked superseded, with the ledger published;
12. `claims_reference` cannot silently select the analytic reference when
    a machined prior exists;
13. UnifiedFinish reports its band/strategy mix through narration, and the
    semantic and structural region systems are asserted to agree;
14. standing material is visible to the user and to gates, not
    tracing-only;
15. collision count is stable across simulation resolutions on a fixed
    toolpath.

---

# Addendum D — live-validation workflow defects (added 2026-07-30)

(Originally written by the live-validation session under a colliding
"Addendum C" heading; renamed D by the orchestrator — the structural-safety
addendum above keeps the C name and its item numbering.)

Found while running the live validation of the behavioral waves. This is a
workflow/API defect, not a toolpath defect: no cut is wrong, but getting a
project with rest-machining ops into a generated state costs an
undiscoverable number of manual rounds, and nothing tells the user how many
or why.

## A/M11. `generate_all` is not a fixpoint over the rest-stock chain

### Problem

An op with `FromRemainingStock` needs the **simulated** stock of the ops
before it. That snapshot is recorded **during a simulation**. So within a
single `generate_all` pass, a rest op can only ever see the stock from the
LAST simulation — never from an op generated earlier in the *same* pass.

Measured on wanaka this session (9 ops, 4 of them rest-dependent):

| round | action | result |
|---|---|---|
| 1 | `run_simulation` (only ops 0,1,2 generated) | ok |
| 2 | `generate_all` | 5 generated, **2 failed** (`Lakes` id 6, `Unified Finish` id 15) |
| 3 | `run_simulation` | ok, 247 semantic summaries |
| 4 | `generate_all` | (needed purely to clear round 2's failures) |

`Rivers` (index 4) generated fine in round 2 while `Lakes` (index 5)
failed — even though Lakes' predecessor is Rivers and Rivers had just
succeeded **in that same pass**. That is the defect in one line: a chain of
`k` dependent rest ops needs `k` sim→generate rounds, and the tool offers
no way to know `k` in advance.

### Secondary defects observed in the same run

1. **The error text under-specifies the remedy.** *"run a simulation of the
   preceding operations first, then regenerate"* is true but incomplete: it
   does not say the cycle may need repeating, and it does not name **which**
   upstream op the snapshot is missing for. The user cannot tell a
   one-round wait from a four-round one.
2. **Disabled ops keep stale error text.** `Rivers (back) (copy)` and
   `3D Finish 6` are `enabled: false` and still report the rest-stock error
   in `list_toolpaths` and in `run_simulation`'s `runtime_errors`. They read
   as broken when they are merely off, and they inflate the error list an
   agent or user has to triage. A disabled op should report *disabled*, not
   the last error it had while enabled.
3. **No staleness signal after a simulation.** Re-simulating does not mark
   toolpaths stale (§14o), so after the sim that finally makes stock
   available there is no indication that regeneration is now *possible*.
   The user must know to try again. This is the same root as the §14o trap
   but its user-facing half.
4. **`runtime_errors` mixes "cannot yet" with "cannot ever".** A missing
   upstream snapshot is a *sequencing* state; a genuine generation failure
   is an *error*. They are the same shape in the API today.

### Fix shape

Ordered cheapest-first; 1 and 2 are worth doing even if 3 is declined.

1. **Make `generate_all` iterate to a fixpoint.** Loop
   `generate → simulate → generate` while (a) at least one op is blocked
   *only* on missing prior stock and (b) the previous round generated at
   least one new op. Bound it by the number of rest ops (the chain cannot be
   longer) and report the round count. Simulation resolution must be an
   explicit parameter — silently choosing one would re-create the
   resolution-mismatch trap (A/M10), so the caller states it or the call
   refuses.
2. **Distinguish blocked-on-sequencing from failed.** Add a distinct status
   (e.g. `AwaitingPriorStock { blocking_toolpath_id }`) separate from
   `Error`, and exclude disabled ops from both. `list_toolpaths`,
   `run_simulation.runtime_errors`, and the GUI badge all read from it.
3. **Name the blocker in the message.** "waiting on simulated stock after
   *`3D Rough 6`* (index 6)" is actionable; the current text is not.
4. **Clear stale error text when an op is disabled**, so `enabled: false`
   never carries a live-looking error.

### Acceptance gates

- A synthetic project with a 3-deep rest chain reaches fully-generated from
  cold in ONE `generate_all` call, and the call reports how many internal
  rounds it took.
- The fixpoint loop terminates on a genuinely-failing op instead of
  spinning — assert a bounded round count and a clear final error.
- A disabled rest op reports disabled, never a rest-stock error.
- Blocked ops name their blocking upstream op; a sentry pins the message.
- Simulation resolution inside the loop is caller-specified; no default is
  silently applied (A/M10's rule).
- MCP and GUI both consume the new status — no parallel error taxonomy.

### Scope and risk

- Primary files: `generate_all` / compute orchestration in `rs_cam_viz`
  worker + `rs_cam_mcp`, `ProjectSession` toolpath status model,
  `session/compute.rs` prior-stock resolution.
- Size: M. Risk: low-to-medium — it changes an API status enum consumed by
  GUI, MCP and CLI, so it wants one commit and a consumer sweep.
- Dependency: none. Independent of the radius programme; it blocks nothing
  but taxes every live validation, including this one.

### Orchestration

Add to the WP8 (diagnostics plumbing) lane — same consumers, same kind of
status/telemetry surface. Slot as **PR-1c**, alongside A/M8 and A/M9: it is
report/plumbing-shaped, has no toolpath-geometry risk, and every subsequent
live characterisation run gets cheaper once it lands.

Definition of done, item 16: bringing a cold project with rest-machining
ops to fully-generated is a single call, and any op that cannot generate
says whether it is disabled, waiting on an upstream op (named), or failed.

## A/M12. MCP serializes every call behind toolpath generation — including its own escape hatches

### Problem

Measured live on wanaka, 2026-07-30, during the behavioral-wave validation.
A `generate_all` on this project ran **>40 minutes**. While it was in
flight, every other MCP call blocked behind it:

| call | documented behaviour | observed |
|---|---|---|
| `list_toolpaths` | read-only status query | blocked; **aborted after 1800 s** of silence by the client idle timeout |
| `cancel_generation` | *"Instant response reporting whether a job was actually cancelled"* | blocked >120 s, moved to background; **serviced only after the generation had already ended, returning `was_busy: false`** |
| `generate_all` (on timeout) | *"Check progress with `list_toolpaths`, or abort with `cancel_generation`"* | **both suggested remedies are themselves blocked** |

So the two documented escape hatches from a long generation are
unreachable *for exactly as long as you need them*. An agent driving the
GUI over MCP has, in this state, no way to observe progress, no way to
attribute the cost to an operation, and no way to abort. That last row is the defect in its purest form: the abort arrived too late
to abort anything. Note also what this means for diagnosis — a `was_busy:
false` from a queued cancel is NOT evidence that the lane was idle when you
asked; it only reports the state at the moment it was finally serviced. In
this run the CPU drop from ~720% to ~136% was the generation completing
naturally, and reading it as "the cancel worked" was wrong.

The only
diagnosis available was out-of-band: `/proc` thread accounting showed all
rayon workers burning ~440 s CPU each evenly, which is how the run was
established to be a genuine parallel grind rather than a deadlock. That
should not require reading `/proc`.

This is independent of A/M11. A/M11 is "the ladder needs N rounds";
this is "you cannot see or stop any one round".

### Research questions

1. Where is the serialization? Candidates: a single mutex over
   `ProjectSession` held for the whole generation; the MCP request handler
   being single-threaded; or the compute lane and the request lane sharing
   one lock.
2. Which calls are genuinely read-only and can be served from a snapshot
   or a read guard while a generation holds the write side
   (`list_toolpaths`, `project_summary`, `get_toolpath_params`,
   `inspect_*`)?
3. Can `cancel_generation` be routed to the cancel flag directly —
   it only needs to set an `AtomicBool` the compute loop already polls —
   without acquiring whatever lock the generation holds?
4. Is there a progress channel already (the GUI shows *something* during
   generation) that MCP could expose as `generation_status`?

### Fix shape

1. **`cancel_generation` must never block.** It sets the existing cancel
   flag; give it a path that takes no lock the generation can hold. This
   is the highest-value fix and probably the smallest.
2. **Serve read-only calls concurrently** — `RwLock` (or a cheap status
   snapshot updated by the compute loop) so status/params/inspection
   answer during generation.
3. **Add a `generation_status` call** reporting which toolpath index is
   in flight, which stage, and elapsed time. Without it, per-op cost
   attribution is impossible and every long generation is unfalsifiable.
   The `#[cfg]`-gated per-stage timings the compute path already logs are
   the natural source.
4. **Correct the two docstrings** either way: `cancel_generation` should
   not promise instant response, and `generate_all`'s timeout message
   should not recommend calls that cannot answer, until 1 and 2 land.

### Acceptance gates

- With a long generation in flight: `list_toolpaths` returns in < 1 s;
  `cancel_generation` returns in < 1 s and actually stops the job.
- `generation_status` names the in-flight toolpath index and stage, and a
  sentry asserts it changes as generation advances.
- No regression in generation throughput from added locking/snapshotting
  (measure; a status snapshot per op is not a hot path, but prove it).
- An integration test drives generate → status → cancel over the MCP
  surface, not just the library API.

### Scope and risk

- Primary files: `rs_cam_mcp` request dispatch, `rs_cam_viz` compute
  worker + session locking, `ProjectSession` accessors.
- Size: M. Risk: medium — touches the locking model; worth doing as its
  own commit with the concurrency test above.
- Dependency: none. **Do it before A/M11's fixpoint loop** — a loop that
  can run for tens of minutes with no observability and no abort is worse
  than the manual ladder it replaces.

Definition of done, item 17: with a generation in flight, status calls
answer and `cancel_generation` stops it, both within a second.
