# Cutting Calc Data Gaps — Roadmap

Created 2026-05-08, post optimizer-redesign merge. This is the working tracker
for gaps surfaced by the [tool × op capability matrix audit](#audit-summary)
between (a) the operation × tool combinations the optimizer should be able to
handle and (b) what actually works today.

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

**Validation gate.**
- Build a fixture with a Pocket-equivalent toolpath using `OperationType::Profile` on FlatEnd in a wood material that has a matching LUT row.
- MCP `optimize_toolpath` should produce ≥3 attempted candidates (not 1) when feed/RPM is binding at machine cap.
- Existing `tool_load::optimize::stage1_grid_tests::*` should still pass.

**Status.** Not started.

---

### G2: `ScallopConfig` stepover not exposed via `OperationParams::stepover()`

**Symptom.** Ball-nose Scallop has a Stage F retarget path (Scallop/Finish LUT
rows exist for ball-nose) but Stage 1 produces no candidates because the
optimizer thinks Scallop has no stepover knob. Tapered ball is similarly
affected once G5 lands.

**Root cause.** `ScallopConfig` carries a stepover value internally but the
`OperationParams::stepover()` impl returns `None` (or doesn't exist).

**Fix shape.** Wire `ScallopConfig::stepover` through the trait. Likely a
2-line change once the field is identified.

**Validation gate.**
- Construct a Scallop fixture on a ball-nose tool with a wood material that has a matching Scallop/Finish LUT row.
- MCP `optimize_toolpath` should generate Stage 1 candidates with varying stepover; the verdict on each should be sim-measured.
- `build_stepover_variants` unit tests should already cover the math; the new test asserts that the variants reach the apply path for Scallop.

**Status.** Not started.

---

### G3: Stepover accessors missing on RampFinish, RadialFinish, Trace

**Symptom.** Ball-nose finishing ops with LUT matches (Parallel/Finish) can run
Stage F but Stage 1 produces zero candidates because no stepover knob is
exposed to the optimizer. Similarly for V-bit Trace.

**Root cause.** Same shape as G2 — config carries a stepover but the trait
impl is missing or returns None.

**Fix shape.** Audit each op's config for a stepover field, surface via the
trait. Some ops may genuinely lack stepover (true 1D path-following) — those
stay None.

**Validation gate.**
- Per op, build a fixture and run `optimize_toolpath`. Outcome should produce stepover-varying candidates when the op has one.
- Run the param sweep system on the relevant ops: `cargo run -p rs_cam_cli -- sweep <fixture> --param stepover --values "..."` should write valid SVGs at every value (not blank).

**Status.** Not started.

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

## Category B — Routing gaps (pass_role / op_family mismatches)

LUT data exists but the lookup criteria sent by the optimizer don't match it.
Pure routing logic in `crates/rs_cam_core/src/tool_load/chipload.rs` and the
op spec mappings in `crates/rs_cam_core/src/compute/operation_configs.rs`.

### G5: TaperedBall + Scallop sends `Finish`, LUT only has `SemiFinish`

**Symptom.** Wanaka TPs 5 and 7 (tapered-ball 3D finishing) return
`Unmodeled(NoVendorData)` on chipload. Stage F refuses, Stage 1 has no knobs
(see G2). Operator sees `NoImprovementFound` even though calibrated data
exists.

**Root cause.** `data/vendor_lut/observations/*.json` has 2 rows for
`tapered_ball_nose / scallop / semi_finish`. `ScallopConfig`'s `feeds_pass_role`
returns `Finish`. Lookup never matches.

**Fix shape.** Two valid paths:

1. Widen `routed_lookup_family` (or `find_best_row`) to fall back across
   `Finish ↔ SemiFinish` for ops where the operator-facing distinction isn't
   meaningful (everything except true-roughing). Keep diameter / family /
   tool-family as hard matches.
2. Re-classify the existing tapered-ball Scallop LUT rows as `Finish` (data
   change, not code).

(1) is more general; (2) is faster but only helps these specific rows.

**Validation gate.**
- MCP `optimize_toolpath` index 5 (Lakes back inside) on wanaka should produce a Stage F retarget candidate (or `BipolarEngagement` refusal) — not `NoVendorData`.
- `get_tool_load_report` for that TP should show `chipload: Within` or `Exceeds`, not `Unmodeled`.

**Status.** Not started. **Highest priority for wanaka regression.**

---

### G6: TaperedBall + SpiralFinish — same mismatch as G5

**Symptom.** Same shape as G5. Spiral finishing on a tapered ball ends up
`Unmodeled` despite the data existing.

**Root cause.** Identical: `feeds_pass_role` mismatch.

**Fix shape.** Closing G5 in the general (1) form closes G6 automatically.
If we go with the data-only fix in (2), G6 needs its own row reclassification.

**Validation gate.** Build a SpiralFinish fixture on a tapered ball, expect
non-`Unmodeled` chipload verdict.

**Status.** Not started; bundles with G5.

---

### G7: FlatEnd + Profile sends `Roughing`, LUT only has `Contour/Finish`

**Symptom.** Flat-end profile cuts return `Unmodeled(NoVendorData)`. Wanaka
doesn't exercise this directly but it's a common workflow gap.

**Root cause.** `ProfileConfig::feeds_pass_role` returns `Roughing`. The LUT
has `flat_end / contour / finish` rows but no `flat_end / contour / roughing`.

**Fix shape.** Same widening as G5 (Finish ↔ Roughing fallback for Contour
family). Profile cuts a wall, not bulk material — Finish-calibrated chipload
is closer to right than nothing.

**Validation gate.** Build a Profile fixture, expect non-`Unmodeled` chipload.

**Status.** Not started.

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

**Status.** Not started. Largest single piece of work in this doc.

---

## Category E — Cross-cutting

### G14: Validate engaged-diameter usage on every tapered-ball gate path

**Symptom.** Commit A2 fixed the LUT lookup to use engaged diameter at
commanded DOC, but only in `find_matched_lut_row`. Other gate paths (chipload
sample-by-sample, deflection's `tool.diameter()`, power's `Kc` engagement
width) may still use nominal/shaft diameter.

**Root cause.** Audit not done.

**Fix shape.** Audit each `tool.diameter()` call site in `tool_load/`, decide
whether engaged or nominal is correct per call, fix any shaft-where-engaged
should land.

**Validation gate.** A tapered-ball fixture at very shallow DOC (0.1mm) should
match a small-diameter LUT row, not the shank's row. Per-sample chipload
verdicts should be consistent across gates.

**Status.** Not started. Likely small.

---

## Priority order (suggested)

1. **G5 + G6 + G7** — routing widening unlocks 3 cells of LUT data the
   optimizer can already reach. Single change, biggest impact, validates
   directly against wanaka.
2. **G1 + G2 + G3** — code-only knob fixes; once routing is fixed, these
   make Stage 1 sweeps actually run on the unblocked cells.
3. **G13** — deflection model rewrite; needed before serious sub-6mm work.
4. **G14** — engaged-diameter audit; likely small but worth doing before
   we trust any chipload verdict on tapered tools.
5. **G4** — knob-accessor work for V-bit ops (lower volume of users).
6. **G8 / G9** — data gaps that have routing workarounds; punt unless
   operator demand surfaces.
7. **G10 / G11 / G12** — long tail; do as users hit them.

## Tracking

Every gap closure should land as its own commit (or small commit set) with:

- Title naming the gap number (`fix(optimizer): G5 widen pass_role lookup`)
- Body referencing this doc
- Validation gate run + result captured in the commit message
- Status field on the gap above flipped to `Done` in the same commit

When in doubt, log empirical data here. The audit was built from incomplete
data; future audits should layer on top of what we observe end-to-end.
