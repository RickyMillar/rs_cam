# Cutting Calc Data Gaps — Roadmap

Created 2026-05-08, post optimizer-redesign merge. This is the working tracker
for gaps surfaced by the [tool × op capability matrix audit](#audit-summary)
between (a) the operation × tool combinations the optimizer should be able to
handle and (b) what actually works today.

## Current state (2026-05-08, end of session)

**Done this session:** G1, G2, G3, G5+G6+G7, G13, G14 — every blocker on the
wanaka project surface. Live MCP verdicts post-fix:
- End-Mill TPs that previously refused pre-flight on `Exceeds(L/D=7.5)` now
  reach Stage F as `Within(Approximate)` 157–175 µm tip deflection.
- TaperedBall TPs read clean `Within(Validated)` (5–9 µm) instead of the prior
  geometric `Approximate(L/D=5.83)`.
- Stage 1 grid axes now exist for every operation that has any sweep knob
  (DOC × stepover × scallop_height).

**Open gaps remaining**, in priority order:
1. **G15** — investigate why Stage F retarget didn't fire on TaperedBall TPs
   despite `Exceeds(BreakageRisk)` with matchable scaled LUT bounds. Open.
2. **G3a** — RadialFinish angular_step axis. Deferred follow-up to G3.
3. **G4** — V-bit knob accessors (Chamfer, VCarve, Inlay). Lower volume.
4. **G8 / G9** — vendor-data backfills for FlatEnd × Face / Profile roughing.
5. **G10 / G11 / G12** — long-tail data gaps (3D finishing rows, BullNose
   coverage, Waterline rows). Do as users hit them.

The optimizer body is now feature-complete for the wanaka surface. Remaining
work is targeted polish (G15 investigation, G3a/G4 knob coverage) and vendor-
LUT data growth (G8–G12) — all driven by user demand rather than a known
broken behaviour.

Sister docs:

- `planning/archive/optimizer_redesign_2026-05-08.md` — design rationale for
  the pre-flight + Stage F + tier dispatcher (now merged).
- `planning/archive/wanaka_audit_2026-05-08.md` — operator-facing wanaka
  findings that drove the redesign.

## How to use this doc

Each gap below has the same five fields, in this order:

1. **Symptom** — what the operator sees today.
2. **Root cause** — single sentence.
3. **Fix shape** — sketch the change, not the patch.
4. **Validation gate** — concrete, observable, leverages existing infrastructure.
5. **Status** — `Not started` / `Investigating` / `Implementing` / `Validating` / `Done`.

Status moves left-to-right. Mark each gap `Done` only after its validation gate
passes.

## Validation infrastructure we already have

We don't need new test harnesses for most of this work — the redesign and the
spans/sim work shipped enough instrumentation that empirical validation is a
matter of running the right thing and reading the output:

| Surface | Where | What it gives us |
|---|---|---|
| Wanaka project | `planning/archive/wanaka_audit_2026-05-08.md` paths | 8 TPs spanning every interesting tool×op combo, full sim trace, known regression case |
| MCP `optimize_toolpath` | running rs-cam server | End-to-end outcome variant + verdict + gate_deltas + attempted candidates |
| MCP `get_tool_load_report` | running rs-cam server | Per-TP verdict per criterion against current sim, ground-truth for "is the gate firing" |
| MCP `narrate_toolpath` | running rs-cam server | Z-level structure, peak axial DOC, air-cut %, suspicious arcs — agent-readable |
| Param sweep system | `crates/rs_cam_core/tests/param_sweep.rs`, `cargo run -p rs_cam_cli -- sweep` | 54 baseline sweeps + targeted single-param sweeps with JSON fingerprints, SVG, 6-view PNG |
| Span-aware diagnostics | MCP `inspect_spans`, sim viewport | Lift-bridge / dressup / link visualisation, per-span chipload + cycle time |
| LUT inspection | `crates/rs_cam_core/data/vendor_lut/observations/*.json` | The 5 embedded observation files; ground truth for what the LUT actually contains |

A validation gate that just says "tests pass" isn't enough — most of these gaps
will pass the existing tests and still produce wrong behaviour against the
wanaka project. **Prefer gates that name a specific MCP call against a specific
fixture and the specific outcome change expected.**

## Audit summary

From the 2026-05-08 capability matrix (full breakdown lives in conversation
history; key bullets here):

- **5 cells fully ✅:** FlatEnd × {Pocket, Adaptive, Rest, Adaptive3d}; FacingBit × Face; BullNose × {Pocket, Adaptive, Rest, Adaptive3d}; BallNose × {DropCutter, SpiralFinish, HorizontalFinish}; TaperedBall × {DropCutter, HorizontalFinish}.
- **Many cells 🟡:** Stage 1 grid runs but Stage F retarget is dead because the LUT has no rows for that (tool, op_family, pass_role) tuple.
- **Many cells ❌:** No DOC knob, no stepover knob, no LUT match — optimizer can't generate any candidates.
- **The tapered-ball pass_role mismatch** (Scallop, SpiralFinish) is the biggest single hidden gap — the data is there but the routing misses it.

Gaps below are grouped by category (cheap → expensive) and numbered for
reference in commits.

---

## Category A — Code gaps (capability lists / missing accessors)

These are pure code changes inside `rs_cam_core`. No new data, no new model.
Each is small (5–30 LOC) and cheap to validate.

### G1: `has_doc_knob` excludes Profile and Zigzag

**Symptom.** Profile and Zigzag toolpaths skip Stage 1 grid generation even
though their configs carry a `depth_per_pass` field that Stage 1 could sweep.
Optimizer reports `NoImprovementFound: no candidates were produced — operation
has no geometry knobs and feed/RPM are at machine limits` when feed/RPM is
already at the cap.

**Root cause.** `has_doc_knob` in `crates/rs_cam_core/src/tool_load/optimize.rs`
only enumerates 5 op kinds (Adaptive3d, Pocket, Adaptive, Rest, Face) but
Profile and Zigzag both expose `depth_per_pass()` via the `OperationParams`
trait.

**Fix shape.** Add `OperationType::Profile` and `OperationType::Zigzag` to
`has_doc_knob`. Audit `OperationParams::depth_per_pass()` impls to confirm
those return non-zero values for these op types.

**Plan (re-verified 2026-05-08).**

- `ProfileConfig` (`compute/operation_configs.rs:243`): has
  `depth_per_pass: f64` (default 2.0 mm); `OperationParams` impl returns
  `Some(self.depth_per_pass)`. No stepover field — Profile is a contour
  follow.
- `ZigzagConfig` (`compute/operation_configs.rs:393`): has both
  `depth_per_pass: f64` (default 1.5 mm) and `stepover: f64` (default
  2.0 mm); both surfaced via the trait.
- The catch with Profile: today Stage 1's grid runs the `for stepover in
  stepover_variants` inner loop unconditionally. With Profile lacking
  stepover, `OperationParams::stepover()` returns `None` (default trait
  impl) and `apply_stepover_to_op` (which calls `set_stepover`) is a
  no-op. Inner-loop iterations would all produce identical toolpaths,
  burning sim budget on duplicates.
- Fix: in `run_stage_1_grid`, when the anchor op has no stepover knob
  (`anchor_op.stepover().is_none()`), collapse `stepover_variants` to a
  single dummy entry so the inner loop runs once per DOC. Existing
  dedup against the anchor stays correct.

**Validation gate.**
- Build a fixture with a Pocket-equivalent toolpath using `OperationType::Profile` on FlatEnd in a wood material that has a matching LUT row.
- MCP `optimize_toolpath` should produce ≥3 attempted candidates (not 1) when feed/RPM is binding at machine cap.
- New unit test pins `has_doc_knob(Profile) == true` and `has_doc_knob(Zigzag) == true`.
- New unit test pins that `run_stage_1_grid` for a Profile op generates `doc_variants.len()` candidates, not `doc_variants.len() × stepover_variants.len()`.
- Existing `tool_load::optimize::stage1_grid_tests::*` pass unchanged.

**Status.** **Done** 2026-05-08. `has_doc_knob` now includes Profile and
Zigzag; `run_stage_1_grid` collapses the stepover dimension to a single
entry when `anchor_op.stepover().is_none()` so Profile (no-stepover op)
no longer fans out to duplicate sims; `bipolar_prescription` now picks
the family-specific lever for Contour/Trace before the DOC-knob branch
because Profile's bipolar variance is geometry-driven, not DOC-driven.
Three new unit tests in `stage1_grid_tests` pin the behaviour. Wanaka
has no Profile or Zigzag TPs so MCP-level validation defers to a future
fixture; `cargo test -p rs_cam_core --lib` 1213/1213 ✓ and `cargo clippy
-p rs_cam_core --all-targets -- -D warnings` clean.

---

### G2: ScallopConfig spacing knob (`scallop_height`) not swept by Stage 1

**Symptom.** Ball-nose Scallop has a Stage F retarget path (Scallop/Finish
LUT rows exist for ball-nose, hardwood) but Stage 1 produces no candidates
because Scallop fails the `has_doc_knob` gate and the optimizer has no
spacing-axis knob to sweep. Tapered-ball scallop has the same shape.

**Root cause (re-verified 2026-05-08).** The audit was wrong on two counts:

1. **`ScallopConfig` has no `stepover` field.** Its driving knob is
   `scallop_height: f64` (default 0.1 mm) — the maximum ridge height
   between passes. The path planner derives an effective radial step
   from `(scallop_height, ball_radius)` via the chord-height formula
   `step ≈ 2·sqrt(2·r·h − h²)`. So 0.1 mm scallop on a 6 mm ball ≈
   1.55 mm radial step. `scallop_height` and the LUT's `ae_*_mm` bounds
   live in different units; conflating them in
   `build_stepover_variants` would clamp incorrectly.
2. **`has_doc_knob` is the only Stage 1 gate.** Scallop has no
   depth-per-pass (it's surface-following), and no current
   `OperationParams` accessor surfaces `scallop_height`, so Stage 1
   short-circuits to an empty candidate list. Adding a `stepover()`
   accessor that returned `scallop_height` would pass-through the unit
   confusion to every other consumer
   (`session/compute.rs`, `commanded_ae`, etc.).

**Fix shape.**

1. Add `OperationParams::scallop_height()` and `set_scallop_height()`
   to the trait with default `None` / no-op. Implement on
   `ScallopConfig`. Keep `scallop_height` semantics distinct from
   `stepover` so existing consumers aren't misled.
2. Add `build_scallop_height_variants(baseline)` — multiplicative
   envelope only (`[0.7×, 1.0×, 1.3×]`), no LUT clamping (the LUT's
   `ae_*_mm` aren't comparable to scallop_height).
3. Add `apply_scallop_height_to_op(op, value)` symmetric to
   `apply_stepover_to_op`.
4. In `run_stage_1_grid`:
   - Widen the gate from `has_doc_knob(...)` to "has any sweep knob"
     — DOC, stepover, or scallop_height.
   - Collapse each axis to a single anchor entry when the op doesn't
     expose that knob (already done for stepover in G1; mirror for
     DOC and scallop_height).
   - Build the candidate as `apply_scallop_height_to_op(apply_stepover_to_op(apply_doc_to_op(...)))`.
5. `delta_against_baseline` records scallop_height changes.

**Plan.**

The gate widening also affects DropCutter (has stepover, no DOC) and
SpiralFinish (has stepover, no DOC). Those should now also enter
Stage 1, consistent with G3's intent. The G2 commit ships this widening
because the gate change is tightly coupled to the scallop_height work
and the alternative (re-narrowing) would be wrong for Scallop too.
G3 stays scoped to its own per-op accessor work (RampFinish
max_stepdown, RadialFinish angular_step, Trace).

**Validation gate.**
- New unit tests pin: `ScallopConfig::scallop_height()` returns Some,
  `set_scallop_height` writes the field, `has_any_sweep_knob(Scallop)`
  is true, the new variant builder produces a `[0.07, 0.10, 0.13]`-shape
  envelope on a 0.10 mm baseline.
- Wanaka has no Scallop TPs to validate end-to-end, but TPs 4 / 5 / 7
  (TaperedBall ProjectCurve / DropCutter, post G5+G6+G7) should
  remain unaffected by the gate widening — DropCutter has stepover
  but the Stage 1 grid result depends on the gate verdicts already
  reported as Approximate Exceeds. Live MCP `optimize_toolpath` on
  TP 7 (DropCutter / TaperedBall) should newly produce Stage 1
  candidates (currently returns NoImprovementFound after the
  Approximate verdict).
- `cargo test -p rs_cam_core --lib --tests` clean.
- `cargo clippy -p rs_cam_core --all-targets -- -D warnings` clean.

**Status.** **Done** 2026-05-08 (commit `c40795b`). Live MCP validation
against wanaka TP 7 (DropCutter / TaperedBall, hardwood) after GUI
rebuild:

- **Pre-G2:** `optimize_toolpath(7)` → `NoImprovementFound`,
  `attempted.len() == 1` (baseline only). DropCutter was excluded by
  `has_doc_knob` and Scallop's `scallop_height` axis didn't exist.
- **Post-G2:** `optimize_toolpath(7)` → `NoImprovementFound`,
  `attempted.len() == 4` (baseline + 3 Stage 1 stepover variants:
  1.0 mm, 0.39 mm, 0.3 mm). Each candidate carries
  `gate_deltas: { chipload: "worsened", deflection: "same", power: "same" }`.
  The verdict's Approximate detail string carries the per-variant
  diameter scale (×0.38 baseline, ×0.59 / ×0.54 / ×0.49 for Stage 1).

The optimizer correctly refuses because all candidates worsen
chipload — Stage 0's RPM drop (21000 → 17500) raises chipload at fixed
feed and the Stage 1 stepover sweeps can't compensate. The gate
widening behaviour is exactly what the plan specified.

Two side observations worth tracking as follow-up gaps:

1. Stage F retarget didn't appear in `attempted`. With `Exceeds(BreakageRisk)`
   on a row whose scaled `chipload_max` is ~0.0114 mm/tooth and a
   baseline peak of 0.0140 mm/tooth, Stage F should be able to drop
   feed/RPM to bring chipload back inside. Possibly Stage F's
   preflight requires bipolar semantics that don't apply when *all*
   samples exceed the upper bound; verify after compaction.
2. Diameter scale varies per Stage 1 variant (×0.38–×0.59) because
   peak axial DOC depends on stepover. Working as intended — the
   Approximate detail carries this per-candidate so the operator can
   see the spread.

---

### G3: Stage 1 sweep knobs for Trace, RampFinish, Waterline, Pencil (RadialFinish deferred)

**Symptom.** Same as G2 in shape: ball-nose / V-bit finishing ops with LUT
matches can hit the chipload gate but Stage 1 produces no candidates because
none of these ops contributes a sweep axis to the grid.

**Root cause (re-verified 2026-05-08).** Per-op audit:

| Op | Driving knob | Existing accessor | Right axis |
|---|---|---|---|
| Trace | `depth_per_pass: f64` | ✅ `depth_per_pass()` exposed | DOC — but Trace is missing from `has_doc_knob`, so post-G2 the DOC dim collapses to anchor-only |
| RampFinish | `max_stepdown: f64` (Z descent per pass) | ❌ none | DOC-equivalent (Z descent ≈ axial DOC for cone-tooth contact) |
| Waterline | `z_step: f64` (Z spacing between contour passes) | ❌ none | DOC-equivalent (same shape as RampFinish) |
| Pencil | `offset_stepover: f64` (only meaningful when `num_offset_passes > 1`) | ❌ none | stepover (conditional) |
| RadialFinish | `angular_step: f64` **degrees** | ❌ none | New axis — out of scope this commit |
| HorizontalFinish | `stepover: f64` | ✅ `stepover()` exposed | Already swept via the G2 gate widening (no `has_doc_knob` add needed because stepover-only ops now enter Stage 1) |

So G3 splits into two distinct fixes:

1. **DOC-axis ops:** Trace is in `has_doc_knob`'s allowlist (alongside
   Profile/Zigzag from G1). RampFinish and Waterline get
   `depth_per_pass()` accessors that wrap their semantically-equivalent
   `max_stepdown`/`z_step` fields, then join `has_doc_knob`.
2. **Stepover-axis ops:** Pencil gets a conditional `stepover()` that
   returns `Some(self.offset_stepover)` only when `num_offset_passes > 1`,
   `None` otherwise. The G2 gate widening already takes care of letting
   stepover-only ops into Stage 1.

RadialFinish's angular_step is degrees, structurally a different axis
than mm-based DOC/stepover. Deferred to a future gap (G3a) — would need
a new `angular_step_deg()` axis with its own envelope logic. Document
the deferral here so a future agent doesn't re-audit.

**Plan.**

1. `TraceConfig`: add to `has_doc_knob` (depth_per_pass accessor
   already exists).
2. `RampFinishConfig`: implement
   `depth_per_pass()` → `Some(self.max_stepdown)` and
   `set_depth_per_pass(value)` → `self.max_stepdown = value`. Add to
   `has_doc_knob`.
3. `WaterlineConfig`: same shape as RampFinish, mapping to `z_step`.
4. `PencilConfig`: implement conditional `stepover()` and
   `set_stepover` mapping to `offset_stepover`. No `has_doc_knob`
   change — the G2 gate widening already lets stepover-only ops into
   Stage 1.
5. `bipolar_prescription` is already routed by op_family for
   Contour/Trace (G1 reorder). Trace ops with bipolar engagement
   would still get the geometry-driven lever — no change needed.

**Validation gate.**

- New unit tests pin: `has_doc_knob(Trace) == true`,
  `has_doc_knob(RampFinish) == true`, `has_doc_knob(Waterline) == true`,
  `RampFinish.depth_per_pass()` reflects `max_stepdown`,
  `Waterline.depth_per_pass()` reflects `z_step`,
  `Pencil.stepover()` returns Some when `num_offset_passes > 1` and
  None otherwise, set-through writes the correct underlying field.
- `cargo test -p rs_cam_core --lib --tests` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Wanaka has no Trace/RampFinish/Waterline/Pencil TPs to validate
  end-to-end. Live MCP gate deferred to a future fixture build.

**Status.** **Done** 2026-05-08. Trace, RampFinish, Waterline added to
`has_doc_knob`; RampFinish wraps `max_stepdown` and Waterline wraps
`z_step` via `OperationParams::depth_per_pass`. PencilConfig exposes
conditional `stepover()` (only when `num_offset_passes > 1`). Five new
unit tests in `stage1_grid_tests` pin the behaviour. Wanaka has no
matching TPs so MCP-level validation defers to a future fixture;
`cargo test -p rs_cam_core --lib` 1224/1224 ✓ and `cargo clippy
--workspace --all-targets -- -D warnings` clean.

RadialFinish split out as a follow-up gap (G3a, opened 2026-05-08):
its `angular_step` is degrees and needs its own axis treatment that
doesn't fit DOC/stepover/scallop_height envelopes.

---

### G3a: RadialFinish angular_step axis (opened 2026-05-08)

**Symptom.** RadialFinish has no Stage 1 sweep axis. Its `angular_step`
field is in degrees and `point_spacing` is mm path resolution (sub-knob
for sample density, not engagement density).

**Root cause.** The Stage 1 grid's three axes (DOC, stepover,
scallop_height) all live in mm. Adding `angular_step()` to
`OperationParams` would let Stage 1 sweep it, but the grid envelope
generators currently assume mm semantics — clamping against LUT
`ae_*_mm` would be nonsensical.

**Fix shape.** Either (a) add a fourth grid axis with degree-aware
envelope (no LUT clamp, multiplicative envelope around baseline), or
(b) translate `angular_step` to a chord-equivalent radial step at the
local cutting radius and treat that as a stepover-equivalent. (a) is
simpler; (b) reuses the stepover envelope but needs the radius (not
known in the trait).

**Validation gate.** Build a RadialFinish fixture; MCP
`optimize_toolpath` should produce ≥3 stepover-varying candidates.

**Status.** Not started — defer until a project actually uses
RadialFinish (low operator demand today).

---

### G4: DOC accessors missing on Chamfer, VCarve, Inlay

**Symptom.** V-bit ops can land Stage F (LUT match exists for ChamferVbit /
Trace / Finish) but cannot sweep depth-per-pass. Optimization is feed-only.

**Root cause.** These ops carry a depth concept (`depth`, `final_depth`,
`max_depth`) but it's not surfaced via `OperationParams::depth_per_pass()`.

**Fix shape.** Decide whether each op's depth field maps cleanly to "axial
depth-per-pass" semantics. For Chamfer this is straightforward; for VCarve
the depth is geometry-driven (not a sweepable parameter) — likely out of
scope. Inlay may sit between.

**Validation gate.**
- For each op accepted into the knob set, MCP `optimize_toolpath` on a fixture should produce DOC-varying candidates.
- The redesign's tier dispatcher should classify them correctly (Improved / Same / Worsened on chipload).

**Status.** Not started; audit which ops actually want this.

---

## Category B — Lookup-matching gaps (engaged-edge + hardness)

Verification 2026-05-08 against live wanaka MCP showed the original B-category
framing (pass_role routing fallback + per-op `feeds_pass_role` overrides) was
**materially wrong**:

- `pass_role` is not a hard filter in `passes_must_match` (vendor_lookup.rs:125).
  It's only a +45/-25 score nudge. Adding `Finish ↔ SemiFinish` fallback would
  not change which rows are returned.
- The wanaka regression table in `AGENT_PROMPT.md` mislabels TPs 4/5: both are
  `Project Curve` / TaperedBall (not Scallop / SpiralFinish). All three failing
  tapered-ball TPs (4, 5, 7) route to `(parallel, finish)`, which has 8 LUT
  rows including hardwood. Routing isn't the failing layer.
- The actual failure for the 1mm-tip / 7° / 6mm-shank tool on hardwood is the
  diameter-ratio gate. `tool.lookup_diameter_at(peak_steady_axial_doc)` returns
  the engaged ball/cone diameter, which for shallow surface-following cuts
  (peak DOC 1.37–2.04 mm) gives ≈1.2–1.4 mm. LUT's smallest row is 3.175 mm,
  ratio ≈ 0.38, fails the [0.5, 2.0] hard gate in `passes_must_match` → all
  rows rejected → `NoVendorData`.

So the Category B gaps are restated below as a single fused gap covering the
real failure mode.

### G5+G6+G7 (fused): engaged-edge lookup with diameter-scaled chipload bounds

**Symptom.** Live wanaka (`mcp__rs-cam__get_tool_load_report` 2026-05-08):

| Index | TP id | Op | Tool | Chipload verdict |
|---|---|---|---|---|
| 4 | 5 | Project Curve | TaperedBall (1mm tip / 7° / 6mm shank) | `Unmodeled(NoVendorData)` |
| 5 | 6 | Project Curve | same | `Unmodeled(NoVendorData)` |
| 7 | 11 | 3D Finish (DropCutter) | same | `Unmodeled(NoVendorData)` |

Stage F refuses, Stage 1 has no knobs for ProjectCurve / DropCutter (see G2/G3).
Operator sees `NoImprovementFound` despite the LUT containing 8 calibrated
`tapered_ball_nose / parallel / finish` rows across hardwood, softwood, mdf,
acrylic at diameters 3.175 and 6.0 mm.

**Root cause (verified empirically).**

1. `passes_must_match` in `crates/rs_cam_core/src/feeds/vendor_lookup.rs:125`
   has a hard diameter-ratio gate `[0.5, 2.0]`. Engaged tip diameter for the
   1mm-tip tapered ball at peak steady-state DOC is ≈ 1.2 mm; ratio against
   the smallest LUT row (3.175 mm) is ≈ 0.38 — every row rejected.
2. `passes_must_match` also hard-rejects on `material_family` mismatch. Today
   wanaka is hardwood and the affected tuples have hardwood rows so this gate
   doesn't fire — but it would for softwood/MDF projects against hardwood-only
   tuples (e.g. tapered_ball_nose / scallop / semi_finish has 2 rows, both
   hardwood). Per operator guidance 2026-05-08: matching should be largely
   hardness-agnostic; hardness should dial parameters, not reject rows.
3. `pass_role` is already a soft +45/-25 score nudge — not a filter. So the
   originally-audited "Finish ↔ SemiFinish fallback" is a no-op.

**Fix shape.** Re-think `vendor_lookup` row selection so engaged-edge geometry
remains the truth (don't lie about engaged diameter to fit the LUT) and LUT
rows become a derivable calibration source for the actual cutting condition:

1. **Relax the [0.5, 2.0] hard ratio gate.** Replace with a wider envelope or
   no gate, ranking purely by the existing diameter-proximity score (`diam_score`
   in `score_observation`).
2. **Scale chipload bounds by diameter ratio when query diverges from row.**
   The LUT exhibits roughly diameter-linear chipload scaling for ball tools
   (3.175 → 6.0 mm, hardwood parallel/finish: 0.010–0.020 → 0.018–0.032; min
   bound scales ≈ d¹·⁰, max ≈ d⁰·⁷). Apply linear scaling to both bounds for
   the safest extrapolation:
   `scaled_min = row.chipload_min × (query_d / row_d)`
   `scaled_max = row.chipload_max × (query_d / row_d)`
3. **Mark verdict confidence as `Approximate` when scaling kicks in** (e.g.
   |log(query_d / row_d)| > log(1.4) ≈ 0.34). Detail string carries the ratio
   so the operator can see how far the extrapolation reached.
4. **Hardness-agnostic matching.** Convert `material_family` from a hard
   filter to a score-only contributor. After row selection, use the row's
   hardness vs the query's hardness as a chipload-scaling factor (scale
   chipload bounds inversely with hardness ratio — softer wood tolerates
   more chipload). Same `Approximate` confidence treatment.

**Why these are fused.** All three originally-numbered gaps trace to the same
file (`vendor_lookup.rs`) and the same fix surface (relax filters, scale
bounds, mark approximate). G7 (FlatEnd Profile) is also in scope: with the
ratio gate relaxed and pass_role already soft, FlatEnd Profile's
`(contour, roughing)` query will pick up the existing `(contour, finish)`
rows that today exhibit a -25 score nudge but already pass `passes_must_match`
on the existing diameter band. (G7 may already be partially functional; the
diagnosis-pass error in the original audit applied here too. Validation gate
will confirm.)

**Plan.**

1. Add a `MatchedRow` field carrying the diameter ratio between query and the
   selected row.
2. In `passes_must_match`, relax the diameter ratio gate from [0.5, 2.0] to
   either drop it entirely (rely on diameter-proximity score) or widen to
   [0.05, 20.0] as a sanity floor.
3. Convert `material_family` from a hard filter to a score-only contributor
   in `passes_must_match` and `score_observation`. Add a hardness scaling
   factor `(row_hardness / query_hardness)^h` to chipload bounds with `h`
   in [0.5, 1.0] (chipload scales inversely with hardness). Default `h=1.0`
   linear; revisit if calibration data supports otherwise.
4. In `build_result` (and in `chipload::evaluate`'s use of the result),
   scale `chip_load_min/max` by `(query_d / row_d) × (row_hardness/query_hardness)`.
5. In `chipload::evaluate`, downgrade verdict confidence to `Approximate`
   when either scaling factor diverges from 1.0 by more than ±40 %. Detail
   string carries both ratios.
6. Tests:
   - Unit test: tapered_ball_nose / parallel / finish / hardwood with
     query diameter 1.0 mm finds the 3.175 mm row and returns
     `chipload_max ≈ 0.020 × (1.0/3.175) ≈ 0.0063`.
   - Unit test: hardwood query against softwood-only row scales chipload
     bounds by hardwood/softwood janka ratio (1450/600 → ~0.41x).
   - Existing `tool_load::*` and `feeds::*` tests must still pass.

**Validation gate.**

- MCP `mcp__rs-cam__get_tool_load_report` after change: TPs 4, 5, 7
  chipload verdict no longer `Unmodeled(NoVendorData)`. Should be
  `Approximate Within` or `Approximate Exceeds`.
- MCP `mcp__rs-cam__optimize_toolpath` on TP 4 should produce ≥ 1
  attempted candidate with a non-`Unmodeled` chipload delta in
  `gate_deltas` (Improved/Same/Worsened, not Unmodeled).
- TPs 0, 2 stay `Skipped` (drill cycles, no change).
- TPs 1, 6 chipload verdicts stay roughly the same (FlatEnd / hardwood /
  adaptive3d already match well within current band; scaling factor ≈ 1.0
  so no Approximate downgrade).
- `cargo test -p rs_cam_core --lib --tests` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.

**Status.** **Done** 2026-05-08 (commit `d09001e`). Live MCP validation gate
passed against wanaka:

| Idx | TP | Tool | Chipload before | Chipload after |
|---|---|---|---|---|
| 4 | Project Curve | TaperedBall (1 mm tip) | `Unmodeled(NoVendorData)` | `Exceeds 0.00992 BreakageRisk Approximate` (×0.44 / ×1.00) |
| 5 | Project Curve | TaperedBall | `Unmodeled(NoVendorData)` | `Exceeds 0.0149 BreakageRisk Approximate` (×0.42 / ×1.00) |
| 7 | 3D Finish (DropCutter) | TaperedBall | `Unmodeled(NoVendorData)` | `Exceeds 0.0140 BreakageRisk Approximate` (×0.38 / ×1.00) |
| 3 | Project Curve | FlatEnd 6 mm | `Exceeds 0.00667 BurnRisk Validated` | `Exceeds 0.00667 BurnRisk Approximate` (×1.89 / ×0.90) — same peak, confidence demoted to reflect that the 3.175 mm row was previously consumed at face value |

TPs 0 / 2 stay `Skipped`. TPs 1 / 6 chipload verdicts unchanged (6 mm flat
matches its calibrated 6 mm row exactly, no scaling). The
`Exceeds(BreakageRisk)` outcomes on the tapered-ball TPs are real:
per-sample chipload is well above the row's `chipload_max` after linear
scaling for the small engaged tip, indicating actual
chipload-vs-tool-size mismatch the optimizer should now address via
Stage F retargeting once G2 / G3 land stepover knobs.

---

## Category C — Data gaps (vendor LUT rows missing)

These are real holes in the embedded Amana data. Closing them needs vendor
data entry, not code. Document them so we know what to ask Amana for (or
which open datasets to mine).

### G8: FlatEnd × Face/Roughing

**Symptom.** Facing wood with an end mill — extremely common workflow —
returns `Unmodeled` on chipload. The LUT has 8 `facing_bit / face / roughing`
rows but FacingBit isn't always available.

**Root cause.** No vendor rows for FlatEnd in the Face op family.

**Fix shape.** Either (a) source FlatEnd Face rows from Amana / public data,
or (b) interpolate from FlatEnd Pocket/Roughing rows (similar geometry,
different op intent — risk: face cuts are typically wider engagement than
pocket).

**Validation gate.** A Face fixture on a 6mm flat end mill in hardwood produces
a non-`Unmodeled` chipload verdict, and the gate verdict matches operator
intuition (validated against operator running the fixture cut on real
hardware — track in this doc once we have it).

**Status.** Not started; data dependency.

---

### G9: FlatEnd × Profile/Roughing

**Symptom.** Same as G7 but the underlying data is genuinely missing for
roughing-style profiling (deep walls, multi-pass).

**Root cause.** LUT has `flat_end / contour / finish` only.

**Fix shape.** Source roughing-pass-role profiling data, or treat G7's routing
fallback as sufficient (Finish chipload calibrations are conservative).

**Validation gate.** Resolves with G7 or by adding LUT rows.

**Status.** Not started; partial coverage via G7.

---

### G10: FlatEnd × 3D finishing (Parallel, Scallop families)

**Symptom.** Drop-cutter / parallel-finish / scallop on a flat end mill (legit
for low-relief contour work) returns `Unmodeled`.

**Root cause.** All `parallel/finish` and `scallop/*` LUT rows are
ball/tapered-ball only.

**Fix shape.** Vendor data backfill. Lower priority — most users don't 3D
finish with flat tools.

**Validation gate.** Drop-cutter fixture on a flat tool produces non-Unmodeled
chipload.

**Status.** Not started; deferred unless operator demand.

---

### G11: BullNose coverage thin outside Adaptive/Pocket/Roughing

**Symptom.** BullNose on Face, Profile, any 3D finishing → `Unmodeled`.

**Root cause.** LUT only has 4 BullNose rows total (3 adaptive/roughing, 1
pocket/roughing).

**Fix shape.** Vendor data backfill, prioritised by operator workflow audit.

**Validation gate.** Per cell as data lands.

**Status.** Not started.

---

### G12: Waterline almost no rows anywhere

**Symptom.** Waterline finishing returns `Unmodeled` for all tools except
ChamferVbit (1 row). Even with knob fixes, no chipload feedback.

**Root cause.** Single row in entire LUT for any waterline-style op.

**Fix shape.** Vendor data backfill, or re-route Waterline → Contour/Finish
where geometrically equivalent.

**Validation gate.** Resolves after data lands or routing change.

**Status.** Not started.

---

## Category D — Model gaps

Engineering-model issues, not data or routing. Higher LOC and more risk.

### G13: Deflection model — replace geometric L/D with force-aware tip deflection

**Symptom.** Sub-6mm carbide tools in wood that should cut fine get rejected
by the optimizer as `DeflectionSetupLocked` because L/D > 6 fires regardless
of material or carbide modulus. Operator can't dig deeper than 6mm without
overriding the gate.

**Root cause.** `tool_load::deflection::evaluate` is geometric only:
`ratio = stickout / diameter`, threshold at 6.0. The doc-comment in
`crates/rs_cam_core/src/tool_load/deflection.rs` calls this out as a
conservative steel-shop heuristic, not a force model.

**Fix shape.** Replace with a force-aware tip-deflection estimator:
`δ = F·L³/(3·E·I)`, where:

- `F` is the peak transverse cutting force per sample, derived from the
  existing `mrr_mm3_s × Kc / (engaged width)` pipeline the power gate
  already uses.
- `E` is the tool-material modulus: carbide ≈ 600 GPa, HSS ≈ 200 GPa.
  Comes from `ToolConfig.tool_material` (already wired through).
- `I = π·d⁴/64` from engaged diameter at commanded DOC (already correct
  for tapered tools post commit A2).
- Threshold on **microns of tip deflection** (Within < 50 µm; Approximate
  50–200 µm; Exceeds > 200 µm — needs calibration against real-world
  finish data from operator runs).

The `DeflectionSetupLocked` pre-flight refusal stays for the new model's
`Exceeds` outcome; the geometric L/D check disappears.

**Validation gate.**
- Wanaka TP 1 (6mm carbide flat, 45mm stickout) in hardwood at the new model should produce `Within (Approximate)` verdict — finish is degraded but tool is not at risk. Optimizer should reach Stage F retarget instead of `DeflectionSetupLocked`.
- A 1mm carbide engraver at 25mm stickout (L/D 25, geometric Exceeds) in hardwood at low feed should still pass the new model — it's a real workflow.
- A 6mm HSS flat at 60mm stickout in steel (which we don't actually cut, but verifies the model isn't wood-only) should fail the new model — F·L³ / (3·E·I) at HSS modulus puts deflection > 200 µm at typical chipload.
- `tool_load::deflection::tests::*` rewritten to pin all three of the above.

**Status.** **Done** 2026-05-08. Force-aware tip-deflection model live;
calibration confirms the 50 µm / 200 µm defaults work without
adjustment.

*Live wanaka MCP results (post-fix, 2026-05-08).*

| TP    | Tool                            | Pre-fix (geom L/D)            | Post-fix (force-aware)            | Notes |
|-------|---------------------------------|--------------------------------|-----------------------------------|-------|
| 14    | 6 mm flat, 45 mm stickout       | Exceeds(L/D=7.5)              | Unmodeled(ArcEngagementNotCaptured) | Drill — no arc capture, parallel to power gate |
| 4     | 6 mm flat, 45 mm stickout       | Exceeds(L/D=7.5)              | **Within(Approximate)** 158 µm    | Reaches Stage F — slot engagement annotated |
| 7     | 6 mm flat, 45 mm stickout       | Exceeds(L/D=7.5)              | Unmodeled(ArcEngagementNotCaptured) | Drill |
| 12    | 6 mm flat, 45 mm stickout       | Exceeds(L/D=7.5)              | **Within(Approximate)** 175 µm    | Reaches Stage F |
| 5     | 2 mm tip / 6 mm shank, 35 mm    | Within(Approximate, L/D=5.83) | **Within(Validated)** 9.4 µm      | Tapered-ball tip-only engagement |
| 6     | 2 mm tip / 6 mm shank, 35 mm    | Within(Approximate, L/D=5.83) | **Within(Validated)** 7.8 µm      | |
| 10    | 6 mm flat, 45 mm stickout       | Exceeds(L/D=7.5)              | **Within(Approximate)** 157 µm    | Reaches Stage F |
| 11    | 2 mm tip / 6 mm shank, 35 mm    | Within(Approximate, L/D=5.83) | **Within(Validated)** 5.3 µm      | |

The five End-Mill TPs that previously refused pre-flight (TPs 4, 12, 10
plus the two drill TPs) all land in the safe zone with the new model:
the cutting TPs at 157–175 µm get the "finish degradation expected"
Approximate annotation, and the drills become Unmodeled (parallel to
the power gate's behavior on no-arc traces). The three TaperedBall TPs
are now correctly classified as low-deflection (sub-10 µm tip wander)
because the stepped-cantilever model accounts for the stiff 6 mm shank
above the tapered cutting region — the previous geometric L/D=5.83
buried this signal.

No threshold adjustment needed: 50 µm / 200 µm defaults discriminate
the wanaka cases cleanly with margin on both sides. Implementation
matches plan: stepped-cantilever integrator on `ToolDefinition`,
force-aware evaluator in `tool_load/deflection.rs`, prescription string
rewritten to report µm + target stickout, no behavioural fallback when
the trace is absent.

**Plan (2026-05-08, awaiting approval).**

*Wanaka baseline (live MCP, current geometric model).* Tool 3 = 6 mm flat,
45 mm stickout (L/D 7.5). Tool 2 = tapered ball (2 mm tip / 7° / 6 mm
shank), 35 mm stickout (L/D 5.83 against shaft). All End-Mill TPs
(`14, 4, 7, 12, 10`) → `Exceeds(LongToolStiffnessUnsafe)` peak 7.5; all
TaperedBall TPs (`5, 6, 11`) → `Within(Approximate)` peak 5.83. End-Mill
TPs cannot reach Stage F because the pre-flight refusal fires first.

*Force model* — per cutting sample, mirrored from `power::evaluate`:

```
F_i  =  Kc(material)  ×  axial_doc_i_mm  ×  radial_width_i_mm    [N]
```

with:
- `radial_width = (arc_engagement_radians / π) × engagement_radius(axial_doc) × 2`
  — same arc-equivalent slab as power; honest within isotropy bounds.
- `Kc = material.kc_n_per_mm2()` — **raw**, no 2.5× anisotropy
  multiplier. Best-practice for static deflection: sustained tip
  position responds to mean cutting force, not transient grain spikes.
  The 2.5× was scoped to power-safety as an instant-spike bound and
  doesn't transfer here.

*Beam model* — stepped/integrated cantilever, not a single-cylinder
approximation:

The tool is anchored at the collet (`x = 0`) and free at the tip
(`x = stickout`). Cross-section diameter varies along the axis: the
shank region is `shank_diameter`; inside the cutting region it follows
the cutter geometry. Each milling-cutter type already exposes
`lookup_diameter_at(axial_doc_from_tip)` (used by the LUT lookup post
G14 audit). The deflection integral reuses this:

```
d(x)        = shank_diameter,                               for stickout - x > cutting_length
            = cutter.lookup_diameter_at(stickout - x),      otherwise
I(x)        = π × d(x)⁴ / 64

Force is treated as a point load applied at the midpoint of axial
engagement: a = stickout − axial_doc / 2 (from the clamp).

For x ∈ [0, a]:  M(x) = F · (a − x)   (bending region)
For x ∈ [a, L]:  M(x) = 0             (rigid extension; just translates)

θ(a)   = (F / E) × ∫₀ᵃ (a − x) / I(x) dx          (slope at load)
δ(a)   = (F / E) × ∫₀ᵃ (a − x)² / I(x) dx        (deflection at load)
δ_tip  = δ(a) + θ(a) × (L − a)                    (carried out to tip)
```

Numerical integration via 64 mid-point segments (over `[0, a]`); fast,
deterministic, accurate to better than 1 % for monotone-tapered
profiles. Implementation lives on `ToolDefinition` so it composes the
shank and cutter regions cleanly; the cutter trait stays at its
existing `lookup_diameter_at` surface (no new trait method needed).

Modulus from `ToolConfig.tool_material` via a new
`ToolMaterial::youngs_modulus_n_per_mm2()` accessor: Carbide = 600 000,
HSS = 200 000.

*Torsion note.* The tool also experiences a torsional load (tangential
force × engagement radius creates twist about the tool axis). For a
coaxial cutter the resulting tip rotation does not translate the tip
position — it only rotates the cutting edge around the axis. So
torsion does not contribute to the "tip-wandering" metric this gate
predicts, and we model bending only. Documented inline in the module
docstring so this assumption is visible.

Verdict from the **peak `δ` across all cutting samples** of the toolpath:

| Peak tip deflection | Verdict           | Confidence detail string                       |
|---------------------|-------------------|------------------------------------------------|
| `< 50 µm`           | `Within`          | `Validated`                                    |
| `50 – 200 µm`       | `Within`          | `Approximate("…Xµm at sample idx Y…")`         |
| `> 200 µm`          | `Exceeds`         | `LongToolStiffnessUnsafe` (reuses existing tag) |

Slot samples (`arc ≥ π`) annotate `Approximate("slot engagement")` like
power does; isotropy disclaimer reused.

*Threshold derivation (50 µm / 200 µm).* The 50 µm Within bound matches
the chipload finish band typical of wood finishing (≥ 0.05 mm
chip-per-tooth ≫ 50 µm tip wander, so the tool is moving more than the
deflection); below it, tool wander is sub-chip and finish is
unaffected. The 200 µm Exceeds bound is the rule-of-thumb chatter
onset / dimensional-tolerance limit cited in handbook deflection
nomograms (≈ 0.2 mm = 0.008"). These are starting points; **calibration
against wanaka per the next step is the load-bearing part**, and
operator pushback on either bound is expected.

*Calibration data (sanity-check from wanaka MCP report, single-cylinder
proxy).*

Hardwood Kc = 15 N/mm², Carbide E = 600 000 N/mm², engaged-cylinder I.
The integrated stepped-cantilever above will produce **lower** δ than
this proxy because the shank region is stiffer than the cutter
cross-section — but the proxy bounds the answer and confirms the
order-of-magnitude is right:

| TP    | Tool    | d (mm) | L (mm) | I (mm⁴) | peak DOC | peak rad-width | F (N) | δ_proxy (µm) | Likely true verdict |
|-------|---------|--------|--------|---------|----------|----------------|-------|--------------|---------------------|
| 4     | 6 flat  | 6.00   | 45     | 63.6    | 3.0      | 6.00 (slot)    | 270   | 215          | Within(Approximate) once stepped-shank is honest |
| 4 avg | 6 flat  | 6.00   | 45     | 63.6    | 3.0      | 1.95 (avg eng) | 88    | 70           | Within(Approximate)                              |
| 5/6/11| 2 tip   | (TBD)  | 35     | (TBD)   | (TBD)    | (TBD)          | (TBD) | (TBD)        | (compute via sim)                                |

The proxy uses an absurdly conservative I (the engaged cross-section
extended all the way to the clamp). Real wanaka tools have a 6 mm shank
region above the cutting flutes that contributes most of the bending
stiffness — for the 6 mm flat the shank IS the same diameter so the
proxy is exact; for the tapered ball the shank dominates and the proxy
massively overestimates δ. **Implementation will compute peak δ across
actual samples with the stepped model**, then thresholds get a
calibration pass against that data inline in this entry.

*Steel-shop sanity check (for the "model isn't wood-only" gate).*
6 mm HSS at 60 mm stickout, mild steel Kc ≈ 2000, light cut DOC=1
chipload=0.05, half-engagement: F = 2000 × 1 × 3 = 6000 N (per sample,
not per tooth). E = 200 000, I = 63.6 → δ = 6000 × 60³ / (3 × 200 000 ×
63.6) = 33.97 mm. Even an order-of-magnitude smaller force (600 N at
deeper engagement giving avg-radial-width 0.5) gives δ = 3.4 mm =
3400 µm ≫ 200. The model fails steel cuts decisively, as required.

*Code structure.*

| File                                                    | Change |
|---------------------------------------------------------|--------|
| `crates/rs_cam_core/src/tool.rs` / `tool/mod.rs`        | New `ToolDefinition::tip_deflection_under_tip_load(force_n, axial_doc_mm) -> f64` that integrates the stepped cantilever (shank + cutting region) using existing `cutter.lookup_diameter_at`. Pure geometry/mechanics — no Material or sim dependency. Unit-tested with closed-form references (uniform cylinder = `F·L³/3EI`; stepped cylinder against a hand-computed two-segment case). |
| `crates/rs_cam_core/src/compute/tool_config.rs`         | `ToolMaterial::youngs_modulus_n_per_mm2()` accessor with constants `CARBIDE_MODULUS_N_PER_MM2 = 600_000.0`, `HSS_MODULUS_N_PER_MM2 = 200_000.0`. |
| `crates/rs_cam_core/src/tool_load/deflection.rs`        | Replace geometric `evaluate(tool)` with `evaluate(toolpath_id, tool, material, sim_trace)`. Walks samples like `power::evaluate`, computes per-sample `F` then `δ = tool.tip_deflection_under_tip_load(F, axial_doc) / E`. Refusals: `SimulationRequired` (no trace), `ArcEngagementNotCaptured` (no arc on samples), `MaterialUnvalidated` (`Custom` Kc), zero stickout / zero-engagement samples filtered. Module docstring rewrites with the formula, threshold table, and torsion note. Tests rewritten for four calibration scenarios. |
| `crates/rs_cam_core/src/tool_load/mod.rs`               | Update `evaluate_toolpath` to pass `sim_trace` and `material` to `deflection::evaluate`. |
| `crates/rs_cam_core/src/tool_load/optimize.rs`          | `deflection_setup_prescription` rewritten to report "predicted tip deflection X µm at sample idx Y — shorten stickout to ~Z mm, use a stiffer tool/material, or reduce DOC". Pre-flight branch (`preflight_classify`) unchanged structurally — still triggers on `Verdict::Exceeds` from baseline-sim deflection. The 4 sites in `optimize.rs` tests that construct mock `Verdict::Exceeds { peak: ld_ratio, ... }` for deflection get updated to construct it with peak in micrometres. |

No public API changes outside the deflection module signature.
`OptimizeOutcome::DeflectionSetupLocked` tag stays (UI/MCP consumers).
Geometric L/D constants `WITHIN_BOUND` / `EXCEEDS_BOUND` deleted.

*Implementation order.*

1. Add `youngs_modulus_n_per_mm2` on `ToolMaterial` with carbide / HSS
   constants. Unit-test the accessor.
2. Add `ToolDefinition::tip_deflection_under_tip_load(force_n,
   axial_doc_mm) -> f64`. Unit tests:
   (a) uniform cylinder (flat endmill, no shank step) matches closed-
       form `F·L³/(3EI)` to better than 1 %;
   (b) two-segment stepped cylinder (shank + cutter, different d)
       matches a hand-derivation;
   (c) tapered ball — sanity check that the integrated δ is between
       the all-shank and all-tip limits.
3. Rewrite `deflection.rs` with the force-aware evaluator. Walks
   cutting samples, computes per-sample F → δ, takes peak across
   toolpath. Refusals symmetric with `power::evaluate`.
4. Wire `material` and `sim_trace` through `mod.rs::evaluate_toolpath`.
5. Update `deflection_setup_prescription` and fix the 4 mock-verdict
   test sites in `optimize.rs`.
6. Run wanaka MCP `get_tool_load_report`. Capture predicted δ for
   every TP. Compare against the proxy table above and the verdict
   targets (End-Mill TPs → `Within(Approximate)`, TaperedBall TPs →
   `Within(Validated)` or `Within(Approximate)` matching the current
   chipload-side conservative read). Adjust thresholds if needed and
   document the adjustment inline here.
7. Rewrite unit tests for the four calibration scenarios (wanaka End-
   Mill peak / wanaka TaperedBall / 1 mm engraver light cut / 6 mm
   HSS in steel); pin both the verdict variant and the predicted δ
   to a tolerance.
8. Validate end-to-end via `optimize_toolpath`: the End-Mill TPs that
   were stuck on `DeflectionSetupLocked` should pass pre-flight and
   reach Stage F (or refuse with a force-derived reason, e.g. a
   chipload-driven retarget that fails to find a Within point).

*Decisions resolved (operator approved 2026-05-08).*

- **Anisotropy multiplier**: drop. Use raw `Kc(material)`. Static
  deflection is not the place for a worst-case spike bound.
- **Tapered tool stiffness**: stepped/integrated cantilever, not a
  single-cylinder approximation. Honest geometry.
- **No-trace behavior**: `Unmodeled(SimulationRequired)`. No
  geometric L/D fallback. The pre-flight refusal moves into the
  post-baseline classifier (where it already runs structurally).
- **Threshold defaults**: 50 µm Within / 200 µm Exceeds, subject to
  the calibration step (implementation step 6).

---

## Category E — Cross-cutting

### G14: Validate engaged-diameter usage on every tapered-ball gate path

**Symptom.** Commit A2 fixed the LUT lookup to use engaged diameter at
commanded DOC, but only in `find_matched_lut_row`. Other gate paths (chipload
sample-by-sample, deflection's `tool.diameter()`, power's `Kc` engagement
width) may still use nominal/shaft diameter.

**Audit (2026-05-08, cam-navigator subagent).** No code fixes needed.
Every LUT-query path under `crates/rs_cam_core/src/tool_load/` already
uses `lookup_diameter_at(doc)`:

- `chipload.rs:265` — `tool.lookup_diameter_at(lookup_axial_doc_mm)` (the A2 fix)
- `mod.rs:214` — suggest path, correct
- `optimize.rs:643` — Stage 0 / Stage F retarget, correct
- `optimize.rs:976` — `diameter_for_lut_lookup` shared helper, correct

The remaining `tool.diameter()` (shaft) call sites are correctly
shaft-scoped:

- `deflection.rs:32` — cantilever L/D uses shaft because shaft *is* the
  beam structurally; this is exactly what the geometric model intends.
- `optimize.rs:850` — stickout recommendation in shaft-relative terms.
- `power.rs:93` — wraps `engagement_radius(axial_doc)` which is already
  DOC-aware; for tapered ball this resolves to the tip geometry.
- `optimize.rs:645` / `:977` — explicit fallbacks when DOC is 0 or
  unknown, well-documented.

One UX clarity item flagged but not a correctness bug:
`deflection_setup_prescription` says "target L/D=4" without naming which
diameter; reasonable to clarify in a future polish pass alongside G13's
prescription rewrite. Logged as a soft follow-up.

**Status.** **Done** 2026-05-08. Audit-only gap — no code changes
required. Closed via this status flip.

---

### G15: Stage F retarget skips TaperedBall chipload-Exceeds(Approximate)

**Symptom.** On wanaka post-G5+G6+G7 fix, the three TaperedBall TPs
(5/6/11) carry `chipload: Exceeds(ChiploadBreakageRisk)` with peaks
0.010–0.015 mm/tooth — well above the scaled max bound from the
extrapolated `amana-tapered-hardwood-parallel-3175-2f` row (×0.38–0.44
diameter scale, ×1.00 hardness). The chipload band IS defined, so Stage
F has bounds to retarget against, yet the operator has not observed
Stage F producing a Within candidate for these TPs in
`optimize_toolpath`. (Side observation flagged during G2 validation;
not investigated end-to-end before session compact.)

**Root cause.** Unconfirmed. Three plausible candidates:

1. Stage F's `solve_chipload_retarget` has a corner case for very
   small engaged-diameter tapered tools — the RCTF compensation may
   produce a feed that exceeds machine `plunge_base` and the safety
   cap rejects it before evaluating.
2. The matched-LUT row from G5+G6+G7 extrapolation passes the gate
   (returns `Some`) but its `chip_load_min_mm` / `chip_load_max_mm`
   may not be re-scaled by the new `chipload_diameter_scale`. If
   Stage F reads the raw bounds, the retarget aims at a bound that
   doesn't apply to the actual cutter.
3. The Project Curve operation type doesn't expose the right knobs to
   Stage F (verify `OperationParams::feed_rate` / `set_feed_rate`
   are wired for Project Curve).

Hypothesis #2 is the most concerning because it would mean the G5+G6+G7
soft-scaling work isn't actually plumbed all the way through to Stage
F's retarget math.

**Fix shape.** First, run `optimize_toolpath` against wanaka TP 5
(easiest, lowest engagement) and capture the full attempted-candidates
list. Verify whether Stage F was attempted at all and, if so, what
constraints it hit. Then decide between a targeted code fix vs. a wider
Stage F audit.

**Validation gate.** Wanaka TP 5/6/11 should each produce at least one
candidate from Stage F retarget (even if the candidate fails verdict
checks) — the optimizer's `attempted` list should show the Stage F
slot non-empty. If Stage F is firing but producing infeasible
candidates, the gate becomes "Stage F output has chipload Within at
the scaled bounds for the cutter".

**Status.** Not started. Investigation gap — needs an MCP run before a
fix shape lands.

---

## Priority order (suggested)

1. **G15** — investigate Stage F retarget skip on TaperedBall TPs
   with extrapolated LUT rows. Most-recent operator pain point now
   that pre-flight refusals are unblocked.
2. **G3a** — RadialFinish angular_step axis. Small Stage 1 follow-up
   to G3.
3. **G4** — knob-accessor work for V-bit ops (lower volume of users).
4. **G8 / G9** — data gaps that have routing workarounds; punt unless
   operator demand surfaces.
5. **G10 / G11 / G12** — long tail; do as users hit them.

### Closed (this session, 2026-05-08)

- **G5 + G6 + G7** — engaged-edge LUT lookup with diameter + hardness
  chipload scaling (commit `d09001e`).
- **G1** — Profile + Zigzag added to `has_doc_knob`; bipolar
  prescription reordered for Contour/Trace family (commit `11e0f9f`).
- **G2** — `scallop_height` axis added to Stage 1 grid; gate widened
  from "has DOC knob" to "has any sweep knob" (commit `c40795b`).
- **G3** — Trace, RampFinish, Waterline get DOC axis; Pencil gets
  conditional stepover (commit `2926a15`).
- **G14** — engaged-diameter usage audit; no code fixes needed
  (commit `13a469e`).
- **G13** — force-aware tip-deflection model; geometric L/D check
  removed (commit `1fe3292`).

## Tracking

Every gap closure should land as its own commit (or small commit set) with:

- Title naming the gap number (`fix(optimizer): G5 widen pass_role lookup`)
- Body referencing this doc
- Validation gate run + result captured in the commit message
- Status field on the gap above flipped to `Done` in the same commit

When in doubt, log empirical data here. The audit was built from incomplete
data; future audits should layer on top of what we observe end-to-end.
