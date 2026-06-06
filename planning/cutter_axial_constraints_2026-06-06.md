# Cutter Axial Constraints — unified envelope for "how deep is too deep"

**Date:** 2026-06-06 (rev. 3 — corrections from the architectural-refactor
investigation wave; see `architectural_refactor_2026-06-06_v2.md` Phase 0)
**Author:** session continuation of combined-Suggest v3 work
**Status:** ready for implementation — reviewer findings incorporated
**Successor to:** the deferred v3.3d / v3.3e / slope-histogram phases in
`planning/combined_suggest_v3_2026-06-04.md` (those are rescoped or dropped
in light of the analysis below)

**Rev 3 changes (2026-06-06, adversarially-verified wave):**
- §2.1 docstring + §8.1 test: fixed the residual internal contradiction —
  both still said "invert `predict_peak_deflection_um`" despite §4/§7
  forbidding it. Both now use the `tip_deflection_from_engagement` /
  `tip_deflection_mm` binary-search path.
- §3.1/§3.3: the `ap_min_factor`/`ap_max_factor` fields are **already
  landing in the working tree** (concurrent agent: `vendor_lut.rs:188,192`,
  `vendor_lookup.rs:55,59`; `LookupResult` initializers in
  bounds.rs/headroom.rs/mod.rs were mid-update during this revision).
  Schema step rescoped from "add fields" to "verify migration coverage."
- §3.2/§8.3: dropped the residual `ap_max_absolute_mm` references that
  contradicted the rev-2 §3.1 resolution (reuse `ap_max_mm`).
- §4.1/§7: fixed stale citations — `predict_peak_deflection_um` is
  `predict.rs:125` (not :455); `doc_derating_scale` is `geometry.rs:138`
  (not :117); `engaged_diameter_at_doc` is a method on `ToolGeometryHint`
  at `feeds/mod.rs:103` (not tool/mod.rs:197, which is
  `lookup_diameter_at`).
- §5.3: NEW task — `op.is_finish_3d()` **does not exist**; the pass
  router must either add it as a new exhaustive `OperationType` match or
  route on `OperationSpec.feeds_family` (catalog.rs:60).
- §5.5: fixed false premise — the axial envelope is NOT the first
  DPP-mutating pass; `clamp_dpp_to_rigidity` /
  `clamp_dpp_to_cutting_length` / `backoff_dpp_for_deflection` already
  mutate DPP (suggest.rs:850-852) before chipload recalibration
  (suggest.rs:865).
- §10: `tip_deflection_from_engagement` extraction risk raised
  low → **medium** — it is not a clean 1:1 extraction (see §4.2 note).

**Rev 2 changes (post-review):**
- §3 schema: dropped `ap_max_absolute_mm`; reuses existing
  `ap_min_mm` / `ap_max_mm` absolute caps, adds only the optional
  factor fields.
- §4 calculator: deflection inversion now uses
  `ToolDefinition::tip_deflection_mm` via monotone binary search
  (was: closed-form inversion of `predict_peak_deflection_um`).
  The closed-form claim was wrong for cutters whose engaged diameter
  varies with DOC (V-bit, tapered-ball); `predict_peak_deflection_um`
  also refuses V-bits and returns 0 for ops without `depth_per_pass()`.
- §5 consumers: scoped Phase 3 to Adaptive3d DPP + VCarve clamp +
  ProjectCurve warning. **Deferred** automatic 3D finish
  `stock_to_leave` mutation until in-process stock at gen time is
  modeled — until then, surface as warning + rationale only.
- §5.3 pass order: added explicit `chipload_bounds` re-derivation
  step after any DPP mutation (otherwise stale bounds poison
  downstream chipload recalibration).
- §5.4 (NEW): explicit pinned-field policy — recommend C
  (clamp-down on user values above safe band, warn-only on values
  below safe band).
- §5 ProjectCurve consumer: read `cfg.depth` directly (the field is
  not named `target_depth`, and `operation_feeds_hints` does not
  thread it into `FeedsInput`).

---

## 1. Problem

The user-facing question — *"how deep can I cut without trashing the surface,
exceeding deflection, breaking the tool, or burning the wood?"* — surfaces in
four operations today, each with its own knob that the user **eyeballs**:

| Op | Knob | Eyeballed because |
|---|---|---|
| V-carve | `max_depth` (default 5 mm) | No physical bound check |
| `ProjectCurve` (engraving) | `target_depth` | No feasibility check |
| 3D Finish (scallop / drop_cutter / horizontal_finish / waterline) | `stock_to_leave` | No coupling to chip thickness or deflection |
| Adaptive3d (rough) | `depth_per_pass` | Partial — Suggest's v3 work picks a reasonable value, but the constraints aren't unified |

All four are asking the **same physical question** with different framings.
Different cutter families (flat-end / ball / bull / V-bit / tapered ball)
shift the math, but the constraint shape is identical:

> What range of axial DOC is mechanically safe AND produces an acceptable
> chip thickness for this (tool, material, machine, engagement) combination?

The tapered-ball case is the user's specific worry. On a Ø6 mm tapered ball
with Ø2 mm tip + 7° taper:

| ap (mm) | Engaged D_eff (mm) |
|---|---|
| 0.5 | ~2.0 (pure ball arc) |
| 2.0 | 2.0 (full ball) |
| 4.0 | 2.5 (ball + cone) |
| 6.0 | 3.0 (ball + cone) |
| 10.0 | 4.5 (mostly cone) |

So `D_eff(ap)` grows nonlinearly, force grows quadratically with `ap` past the
ball radius, and the cutter changes character with depth. No single
constant-`xD` rule fits.

## 2. Architecture

**One unified envelope, per-op consumers.** Each operation queries the same
calculator with op-specific inputs (target finish, engagement geometry), gets
back a constraint envelope, and intersects with its own goal.

### 2.1 The envelope

```rust
// crates/rs_cam_core/src/feeds/cutter_constraints.rs (NEW)

/// Axial-DOC constraint envelope for a (tool, material, machine,
/// engagement) tuple. Every Suggest pass that touches axial depth —
/// V-carve `max_depth`, 3D Finish `stock_to_leave`, Adaptive3d
/// `depth_per_pass`, ProjectCurve `target_depth` — queries this once.
/// Each constraint is computed independently; `binding_constraint`
/// names which one is tightest.
#[derive(Debug, Clone)]
pub struct CutterAxialConstraints {
    /// Max axial DOC before predicted tip deflection exceeds the
    /// target bound (target_finish_um for finish ops, 200 µm
    /// EXCEEDS_BOUND for rough). Computed by monotone binary search
    /// over `tip_deflection_from_engagement` (the extracted
    /// `ToolDefinition::tip_deflection_mm` path, §4.2) — NOT by
    /// inverting `predict_peak_deflection_um` (wrong layer; refuses
    /// V-bits, returns 0 without `depth_per_pass()` — see §7).
    pub max_doc_deflection_mm: f64,

    /// Max axial DOC the matched LUT row permits at this chipload.
    /// `ap_max_factor × query_diameter`. `None` when no LUT row
    /// matches (no chipload band data → no DOC band data either).
    pub max_doc_vendor_mm: Option<f64>,

    /// Max axial DOC for acceptable axial scallop on a ball /
    /// tapered-ball / bull-nose finish. `None` for flat-end (no axial
    /// scallop concept on a cylinder) and for V-bits (the V flank
    /// produces straight walls, not scallops). Uses the rotated form
    /// of `feeds::geometry::scallop_stepover`.
    pub max_doc_scallop_mm: Option<f64>,

    /// Min axial DOC below which arc-mean chip thickness collapses
    /// below the LUT chipload_min → burn risk. The 3D Finish 6
    /// `Exceeds(Low)` case from the Wanaka run. `None` when no LUT
    /// row matches.
    pub min_doc_chipload_floor_mm: Option<f64>,

    /// Which constraint produced `max_doc`. Drives the rationale
    /// entry on `SuggestRationale`.
    pub binding_constraint: AxialBindingConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxialBindingConstraint {
    Deflection,
    VendorAp,
    Scallop,
    /// max < min → "this tool can't finish this surface at this
    /// chipload"; consumer should emit a refusal warning.
    SafeBandEmpty,
}

impl CutterAxialConstraints {
    /// The actual `max_doc` the caller should use — min of all bounds.
    pub fn safe_max_doc_mm(&self) -> f64 { ... }

    /// `Some(true)` when min_floor > max — no DOC value satisfies
    /// both chip-thickness and deflection / scallop. `Some(false)`
    /// when the band exists. `None` when no chip-thickness floor was
    /// derivable (no LUT row).
    pub fn safe_band_is_empty(&self) -> Option<bool> { ... }
}

/// Build the envelope. ~150-250 LOC of glue around existing pieces
/// (predict_peak_deflection_um, doc_derating_scale,
/// tapered_ball_effective_diameter, scallop_stepover, chip_geometry).
#[tracing::instrument(level = "debug", skip_all)]
pub fn cutter_axial_constraints(
    tool: &ToolDefinition,
    material: &Material,
    machine: &MachineProfile,
    lut: &VendorLut,
    feed_rate_mm_min: f64,
    radial_woc_mm: f64,
    /// `None` for rough / V-carve (no surface-quality goal).
    /// `Some(scallop_height_um)` for ball / tapered-ball finishes —
    /// produces `max_doc_scallop_mm`.
    target_finish_um: Option<f64>,
    /// Deflection limit: 200 µm for rough (breakage bound), typically
    /// 25-50 µm for finish (surface-quality bound). Default 50 µm if
    /// `target_finish_um.is_some()`, 200 µm otherwise.
    deflection_limit_um: Option<f64>,
) -> CutterAxialConstraints { ... }
```

### 2.2 Per-op consumers

```rust
// V-carve: clamp max_depth, warn if user pinned it too deep.
//   Lives in feeds::suggest::pick_vcarve_max_depth (NEW pass).
let c = cutter_axial_constraints(tool, material, machine, lut, feed,
    vbit_width_at_max_depth, None, None);
let safe = c.safe_max_doc_mm();
if cfg.max_depth > safe { warn!(VCarveDepthExceedsSafe { ... }) }
if !pinned(cfg.max_depth) { cfg.max_depth = safe; }

// 3D Finish: pick stock_to_leave so per-pass DOC is in [min, max].
//   Lives in feeds::suggest::pick_finish_stock_to_leave (NEW pass).
let c = cutter_axial_constraints(tool, material, machine, lut, feed,
    stepover, Some(25.0), Some(50.0));
if matches!(c.safe_band_is_empty(), Some(true)) {
    warn!(FinishToolMismatched { ... });
} else {
    let target_doc = c.safe_max_doc_mm();
    // stock_to_leave on a finish op is the cutter offset above the
    // surface, so its effect on per-pass DOC depends on what the
    // rough left. Without in-process stock at gen time (deferred),
    // we approximate via the rough's stock_to_leave from the same
    // setup.
    let upstream = ctx.upstream_leftover_stock_mm.unwrap_or(0.5);
    cfg.stock_to_leave = (upstream - target_doc).max(0.0);
}

// Adaptive3d rough: pick depth_per_pass.
//   Replaces eyeballed default in the existing combined-Suggest pass.
let c = cutter_axial_constraints(tool, material, machine, lut, feed,
    stepover, None, None);
cfg.depth_per_pass = c.safe_max_doc_mm();

// Engraving / ProjectCurve: feasibility check on commanded depth.
//   Lives in feeds::suggest::check_engrave_depth_feasible (NEW pass).
let c = cutter_axial_constraints(...);
if cfg.target_depth > c.safe_max_doc_mm() {
    warn!(EngraveDepthInfeasible { ... });
}
```

## 3. Phase 1 — LUT schema migration

### 3.1 Schema change

`VendorObservation` already has `ap_min_mm` / `ap_max_mm` (absolute
caps in mm) — reviewer-verified at `vendor_lut.rs:180` and exposed on
`LookupResult` at `vendor_lookup.rs:49`. The rev-1 plan duplicated
those with a proposed `ap_max_absolute_mm` field; rev 2 reuses them.

Only the proportional-rule fields are new — **and as of rev 3 they are
already landing in the working tree** (`VendorObservation` at
`vendor_lut.rs:188,192`, `LookupResult` at `vendor_lookup.rs:55,59`; the
concurrent agent was mid-update on the `LookupResult` initializers in
`tool_load/optimize/bounds.rs`/`headroom.rs`/`mod.rs` when this revision
was written). Phase 1's schema step is therefore **verify, not add**:
confirm every `LookupResult` constructor populates the factor fields and
the §3.2 migration covers all 252 rows.

```rust
// crates/rs_cam_core/src/feeds/vendor_lut.rs

pub struct VendorObservation {
    // existing fields unchanged...
    pub ap_rule: Option<String>,           // KEEP — audit trail, source citation
    pub ap_min_mm: Option<f64>,            // EXISTING — absolute mm cap (lower)
    pub ap_max_mm: Option<f64>,            // EXISTING — absolute mm cap (upper)

    // NEW — for proportional rules like "0.25xD to 0.7xD".
    /// Minimum axial DOC factor (× diameter). `None` when the source
    /// only specifies a max bound. Optional so existing JSON parses
    /// unchanged; missing factor is conservative.
    pub ap_min_factor: Option<f64>,
    /// Maximum axial DOC factor (× diameter). `None` when the source's
    /// `ap_rule` is label-only ("3d finishing") or absolute-mm only —
    /// calculator falls back to whichever field is populated.
    pub ap_max_factor: Option<f64>,
}
```

### 3.1.1 Lookup combinator

Both fields can coexist on a row that publishes "0.5xD up to 4.2 mm
max" — the calculator takes the tighter limit:

```rust
fn max_doc_vendor_mm(row: &LookupResult, query_diameter_mm: f64) -> Option<f64> {
    let from_factor = row.ap_max_factor.map(|f| f * query_diameter_mm);
    let from_absolute = row.ap_max_mm;
    match (from_factor, from_absolute) {
        (Some(f), Some(a)) => Some(f.min(a)),
        (Some(f), None)    => Some(f),
        (None,    Some(a)) => Some(a),
        (None,    None)    => None,
    }
}
```

Same combinator for `min`. The factor is the diameter-scaling rule;
the absolute is the per-row hard cap. They're complementary, not
duplicative.

### 3.2 Migration: 46 unique strings → structured factors

Below is the full mapping table for the 46 unique `ap_rule` strings
currently in the LUT (252 rows total). The reviewer agent should
confirm these interpretations against the source PDFs / observations
data we ingested. **Any "TBD" row stays as `None` until the source is
re-checked — no fallback heuristic, conservative by default.**

| `ap_rule` string | rows | `ap_min_factor` | `ap_max_factor` | Note |
|---|---:|---:|---:|---|
| `cut depth per pass = cutting edge diameter (1xD); 2xD reduce chip load 25%; 3xD reduce chip load 50%` | 56 | `null` | `1.0` | nominal cap; deeper handled by `feeds::geometry::doc_derating_scale` |
| `1xD use recommended feed rate; 2xD reduce 25%; 3xD reduce 50%` | 35 | `null` | `1.0` | same family |
| `1xD use recommended chip load; 2xD reduce chip load 25%; 3xD reduce 50%` | 22 | `null` | `1.0` | same family |
| `depth of cut equal to bit diameter; if 2xD reduce chipload by at least 25%; if 3xD reduce by at least 50%` | 14 | `null` | `1.0` | same family |
| `1xD use recommended feed rate; 2xD reduce feed rate 25%; 3xD reduce 50%` | 10 | `null` | `1.0` | same family |
| `tip depth dependent` | 8 | `null` | `null` | V-bit rows — depth caps at flute length, not `xD` factor. Skip vendor bound; deflection / scallop bound applies. |
| `3d profiling finish` | 8 | `null` | `null` | label, no band info. TBD if a default applies. |
| `3d finishing` | 8 | `null` | `null` | same |
| `1xD per chart; 3D-finish DOC unspecified by source` | 8 | `null` | `1.0` | explicit "1×D" note |
| `Profiling Axial = 1xD` | 4 | `null` | `1.0` | |
| `max 1mm` | 4 | `null` | `null`*| **absolute mm**, not factor. Populate the EXISTING `ap_max_mm` per-row at ingest time (rev 3: no new field — see asterisk note below). |
| `Axial = .5xD (slotting); Pocket/Slot up to 1xD` | 4 | `null` | `1.0` | take higher (pocket); slotting handled by chip-evacuation derating |
| `max 0.8mm` | 3 | `null` | `null`*| absolute mm |
| `Axial = .5xD (slotting)` | 3 | `null` | `0.5` | slotting-specific |
| `20% of diameter for basic engagement parameters; drop feed ~50% when plunging into solid` | 3 | `null` | `0.2` | plunge-only ops |
| `light finishing` | 2 | `null` | `null` | label |
| `3d profiling semi` | 2 | `null` | `null` | label |
| `0.3xD to 0.7xD` | 2 | `0.3` | `0.7` | |
| `0.25xD to 0.65xD` | 2 | `0.25` | `0.65` | |
| `0.5xD to 1xD` | 1 | `0.5` | `1.0` | |
| `0.5xD to 1.5xD` | 1 | `0.5` | `1.5` | |
| `0.4xD to 1.2xD` | 1 | `0.4` | `1.2` | |
| `0.4xD to 1.25xD` | 1 | `0.4` | `1.25` | |
| `0.35xD to 0.8xD` | 1 | `0.35` | `0.8` | |
| `0.35xD to 0.9xD` | 1 | `0.35` | `0.9` | |
| `0.35xD to 1xD` | 1 | `0.35` | `1.0` | |
| `0.35xD to 1.2xD` | 1 | `0.35` | `1.2` | |
| `0.3xD to 0.75xD` | 1 | `0.3` | `0.75` | |
| `0.3xD to 0.85xD` | 1 | `0.3` | `0.85` | |
| `0.25xD to 0.6xD` | 1 | `0.25` | `0.6` | |
| `0.25xD to 0.7xD` | 1 | `0.25` | `0.7` | |
| `0.25xD to 0.9xD` | 1 | `0.25` | `0.9` | |
| `0.2xD to 0.45xD` | 1 | `0.2` | `0.45` | |
| `0.2xD to 0.5xD` | 1 | `0.2` | `0.5` | |
| `0.2xD to 0.65xD` | 1 | `0.2` | `0.65` | |
| `0.15xD to 0.45xD` | 1 | `0.15` | `0.45` | |
| `0.15xD to 0.55xD` | 1 | `0.15` | `0.55` | |
| `0.12xD to 0.42xD` | 1 | `0.12` | `0.42` | |
| `0.08xD to 0.25xD` | 1 | `0.08` | `0.25` | |
| `Traditional ADOC = 0.500 in = 100% x D` | 1 | `null` | `1.0` | |
| `Profiling Axial = 2xD (HEM); side milling up to 2xD` | 1 | `null` | `2.0` | HEM allowed |
| `HEM ADOC = 1.000 in = 200% x D` | 1 | `null` | `2.0` | HEM allowed |
| `Finishing Axial = Max LOC` | 1 | `null` | `null`*| max-LOC = flute length; needs per-row review |
| `semi-finish` | 1 | `null` | `null` | label |
| `side-entry or ramp entry required (no straight plunge); upcut O-flute for chip evacuation. Phase 5 promotion (2026-06-01) — diameter_mm omitted; article publishes one chipload window across the upcut O-flute line.` | 1 | `null` | `null` | no DOC info; chip-evacuation only |
| `max 1.2mm` | 1 | `null` | `null`*| absolute mm |

**Asterisked rows (`max N mm` style)**: 11 rows carrying absolute mm
caps rather than diameter factors. **Resolution (rev 2):** these
rows already have / should have `ap_max_mm` populated as the
absolute cap; leave `ap_max_factor: null`. The §3.1.1 combinator
returns the absolute when only one field is set. No new field
needed.

**Migration note for the `max N mm` rows**: when re-running the
migration script, the data-ingest pipeline should *also* populate
`ap_max_mm` from the prose if it isn't already set. Reviewer noted
177 of 252 rows currently lack both `ap_min_mm` and `ap_max_mm` — for
those, the migration script should populate `ap_*_mm` where the
prose carries an absolute value, and `ap_*_factor` where it carries
a proportional one.

### 3.3 Migration mechanics

1. ~~Add the two new optional fields.~~ **Rev 3: fields already landed
   (see §3.1) — verify instead.** All existing JSON parses unchanged
   (Serde optional fields default to `None`).
2. Write a one-shot Rust binary `tools/migrate_ap_rule.rs` that reads
   each LUT JSON, looks up the row's `ap_rule` in a hardcoded mapping
   table (copied from §3.2 above), writes the `ap_min_factor` /
   `ap_max_factor` fields back. Idempotent — skip rows that already
   have the structured fields populated. Run once, commit the result.
3. After migration, all 252 rows have structured fields OR explicit
   `None` (the label-only / unparseable cases). Calculator reads
   structured fields only; the `ap_rule` string stays as the source
   citation.

**Future rows** added via the lit-matrix refresh process fill the
structured fields at ingest. The `ap_rule` string stays as the
human-readable provenance.

## 4. Phase 2 — Envelope calculator

### 4.1 Building blocks (all exist)

| Concept | Existing call | File |
|---|---|---|
| Deflection prediction | `predict_peak_deflection_um(tool, material, axial, radial, feed, rpm, flutes)` | `feeds/predict.rs:125` (rev 3 fix; was cited :455+) — reference only, NOT used by the envelope (§7) |
| DOC scale cliff | `doc_derating_scale(ratio)` | `feeds/geometry.rs:138` (rev 3 fix; was cited :117) |
| Engaged D for tapered/ball/bull | `ToolGeometryHint::engaged_diameter_at_doc(self, axial_doc_mm, tool_diameter_mm, shank_diameter_mm)` | `feeds/mod.rs:103` (rev 3 fix; was cited tool/mod.rs:197, which is `lookup_diameter_at`) |
| V-bit engaged width | `vbit_width_at_depth(angle, tip_d, ap)` | `feeds/geometry.rs:175` |
| Scallop stepover (horizontal) | `scallop_stepover(ball_radius, target_scallop)` | `feeds/geometry.rs` |
| Arc-mean chip thickness | `MillingCutter::chip_geometry(tool, ap, radial)` | `tool/mod.rs` |
| LUT row matching | `find_best_row_for_geometry(lut, query, hint)` | `feeds/vendor_lookup.rs:157` |
| `lookup_diameter_at` | `ToolDefinition::lookup_diameter_at(ap)` | `tool/mod.rs:197` |

### 4.2 What's new

```rust
// crates/rs_cam_core/src/feeds/cutter_constraints.rs   (NEW, ~250-350 LOC)

pub struct CutterAxialConstraints { ... }
pub enum AxialBindingConstraint { ... }

pub fn cutter_axial_constraints(...) -> CutterAxialConstraints {
    // 1. Deflection bound — REV 2: monotone binary search using the
    //    cutter-physics primitive, NOT the op-aware
    //    predict_peak_deflection_um (which refuses V-bits and returns
    //    0 for ops without depth_per_pass).
    //
    //    Reuse `ToolDefinition::tip_deflection_mm(force, axial_eng, E)`
    //    (tool/mod.rs:416) — cutter-shape-aware, no op assumptions.
    //    For each trial ap, compute the chip area + cutting force,
    //    feed those into tip_deflection_mm, bracket the limit.
    //
    //    Linear inversion does NOT work because:
    //    - V-bit / tapered-ball: D_eff(ap) grows with ap → force is
    //      ~quadratic in ap, not linear.
    //    - Bull-nose: piecewise (ball below corner_r, cylinder above).
    //
    //    Binary search converges in ~25-30 iterations to micron
    //    precision. Cheap.
    let max_doc_deflection_mm = invert_deflection_via_binsearch(
        tool, material, machine, feed, radial_woc_mm,
        deflection_limit_um);

    // 2. Vendor bound: look up the row, combine ap_max_factor and
    //    ap_max_mm per §3.1.1.
    let max_doc_vendor_mm = lookup_vendor_ap_max(tool, material,
        lut, feed_rate_mm_min, radial_woc_mm);

    // 3. Scallop bound: rotated scallop_stepover formula.
    //    For ball-nose: scallop = R - sqrt(R² - (ap/2)²) → invert
    //    for ap given target_finish_um. For tapered-ball: same formula
    //    using the ball-radius end of the tool (the cone shoulder
    //    doesn't produce a clean scallop, so cap at ap = R_tip).
    let max_doc_scallop_mm = target_finish_um.and_then(|h| {
        axial_scallop_max_doc(tool, h)
    });

    // 4. Chipload floor: arc-mean chip thickness collapses as ap → 0
    //    (engagement arc shrinks on ball / tapered-ball; engaged
    //    fraction shrinks on flat). Solve for min_ap such that
    //    arc_mean_chip(ap, radial_woc, feed_per_tooth) ≥ chipload_min.
    //    Closed-form for flat-end (no DOC dependence on engagement
    //    geometry); binary search for ball / bull / tapered-ball / V-bit.
    let min_doc_chipload_floor_mm = lookup_vendor_chipload_min(...)
        .and_then(|cl_min| solve_min_doc_for_chip(...));

    CutterAxialConstraints { ... }
}

/// Cutter-physics helper extracted from `tool_load::deflection`
/// (currently embedded in `sample_tip_deflection_mm` at
/// `tool_load/deflection.rs:~80`). Reuse here so the gate and the
/// envelope share one deflection model.
///
/// `ToolDefinition::tip_deflection_mm` already exists as the
/// cantilever-bending primitive; what's missing is the "compute
/// force from chip geometry + Kc" step. Extract as
/// `feeds::predict::tip_deflection_from_engagement(cutter, axial,
/// radial, kc, feed_per_tooth, rpm, flutes) -> µm` and have BOTH
/// the post-sim deflection gate and the pre-sim envelope call it.
///
/// REV 3 RISK NOTE — this is NOT a clean 1:1 extraction. The truly
/// shared primitive is only the trailing `force → tip_deflection_mm`
/// step of `sample_tip_deflection_mm` (deflection.rs:101-103). The
/// gate derives force from `SimulationCutSample` arc/radial
/// engagement fields that a pre-sim caller does not have — the
/// envelope must SYNTHESIZE equivalent engagement from commanded
/// (axial, radial, feed) instead. The extraction therefore splits
/// into (a) the shared force→deflection tail (pure refactor) and
/// (b) a NEW pre-sim force-synthesis front-end whose parity with the
/// gate's sim-sample-derived force needs its own test (same inputs
/// via both paths → deflections within tolerance).
pub fn tip_deflection_from_engagement(
    cutter: &dyn MillingCutter,
    axial_mm: f64,
    radial_mm: f64,
    kc: f64,
    feed_per_tooth_mm: f64,
    rpm: f64,
    flute_count: u32,
    tool_material: ToolMaterial,
    stickout_mm: f64,
) -> f64;  // µm
```

### 4.3 Cutter-shape-specific notes

- **Flat-end:** `D_eff` constant. Deflection inversion is trivial.
  Scallop bound `None` (no axial scallop on a cylinder).
- **Ball-nose:** `D_eff` shrinks near tip. Both deflection and scallop
  bounds apply; scallop is usually tightest for finish.
- **Bull-nose:** Cylinder above `corner_r`, ball below. Two-regime
  computation: if `ap < corner_r`, use ball-nose math; else flat.
- **V-bit:** `D_eff(ap) = tip_d + 2 ap tan(α)`. No scallop bound.
  Vendor LUT often label-only — falls back to deflection bound.
- **Tapered ball-nose:** Ball arc up to `R_tip`, cone shoulder
  beyond. Force grows ~quadratically with `ap` past the ball — the
  deflection bound dominates aggressively. Scallop bound only valid
  while `ap ≤ R_tip`; beyond, surface finish is determined by cone
  flank scallop (different formula, optional Phase 3 enhancement).

## 5. Phase 3 — Per-op consumers

Each consumer is a small Suggest pass (~30-80 LOC including warning
emission) that calls `cutter_axial_constraints` and acts on the result.

**Rev 2 scope:** Reviewer flagged that many 3D finish ops have no
`depth_per_pass` accessor by design (single-pass surface-followers
like Scallop / DropCutter / SpiralFinish / RadialFinish), and
DropCutter has no `stock_to_leave` field at all. Automatic 3D-finish
mutation **needs in-process stock** to do honestly — without it,
the consumer can't know how much material the cutter will actually
engage per pass. Phase 3 ships warning-only for finish ops; mutation
deferred.

### 5.1 New Suggest passes (Phase 3 ships these)

| Pass | File | Op kinds | Reads | Writes |
|---|---|---|---|---|
| `pick_adaptive3d_dpp` | `feeds/suggest.rs` (refactor of existing) | `Adaptive3d` | tool, material, machine, LUT, stepover, feed | `cfg.depth_per_pass` |
| `pick_vcarve_max_depth` | `feeds/suggest.rs` | `VCarve` | tool, material, machine, LUT, vbit_width_at_max_depth, feed | `cfg.max_depth` (per §5.4 pinned policy) |
| `check_projectcurve_depth_feasible` | `feeds/suggest.rs` | `ProjectCurve` (V-bit / ball / tapered-ball / flat variants) | tool, material, machine, LUT, `cfg.depth` (read directly — field is `depth`, not `target_depth`, and not threaded through `operation_feeds_hints` today), feed | warning only — no mutation |
| `warn_finish_envelope` | `feeds/suggest.rs` | scallop / drop_cutter / horizontal_finish / waterline / spiral_finish / radial_finish | tool, material, machine, LUT, stepover, feed, target_finish_um, `upstream_leftover_stock_mm` | rationale + warning only — no `stock_to_leave` mutation (deferred) |

### 5.1.1 Deferred (post-in-process-stock)

| Pass | Blocked on |
|---|---|
| `pick_finish_stock_to_leave` (mutating variant) | In-process stock at gen time. Today's finish ops generate paths assuming nominal stock + their `stock_to_leave` offset; without dexel-aware generation, automatic stock_to_leave mutation can't safely coordinate with what the rough actually left. Tracked under `planning/in_process_stock_TBD.md` (not yet drafted). |
| Multi-pass finish scheduling | Same. |

### 5.2 New warning variants (rev 2 — finish-mutation variants removed)

```rust
// crates/rs_cam_core/src/feeds/suggest.rs (SuggestWarning enum)

pub enum SuggestWarning {
    // existing...

    /// User-set V-carve max_depth was above the safe envelope;
    /// clamped down per §5.4 policy C.
    VCarveDepthExceedsSafe {
        commanded_mm: f64,
        safe_mm: f64,
        binding: AxialBindingConstraint,
    },
    /// 3D finish tool / surface / chipload combination has an empty
    /// safe band (min_floor > max_safe). Warning-only in this phase;
    /// auto-pick of stock_to_leave deferred until in-process stock.
    FinishToolMismatched {
        chipload_min_mm_per_tooth: f64,
        max_safe_doc_mm: f64,
        op_kind: OperationType,
    },
    /// User-set ProjectCurve depth is above the safe envelope; warn
    /// only (no mutation — user adjusts manually).
    ProjectCurveDepthInfeasible {
        commanded_mm: f64,
        max_safe_mm: f64,
        binding: AxialBindingConstraint,
    },
    /// Generic "axial value clamped" for the cases that DO mutate
    /// (Adaptive3d DPP, V-carve max_depth via §5.4 policy C).
    AxialDocClampedByEnvelope {
        commanded_mm: f64,
        clamped_mm: f64,
        binding: AxialBindingConstraint,
        op_kind: OperationType,
    },
    /// User-set value is below the chipload-burn floor; warned but
    /// not mutated (policy C respects deliberate conservative pins).
    AxialDocBelowBurnFloor {
        commanded_mm: f64,
        floor_mm: f64,
        op_kind: OperationType,
    },
}
```

Each gets a rationale entry through `feeds::rationale::SuggestRationale::from_warnings`'s exhaustive match (the contract that already pins
warning → rationale coverage).

### 5.3 Wiring into the combined-Suggest orchestrator

These passes slot into `enforce_invariants` (reviewer cited current
flow at `feeds/suggest.rs:833-865`):

```rust
// feeds/suggest.rs::enforce_invariants — extended pass order
//
// Current flow: mutates DPP, then entry strategy/warnings, then
// chipload recalibration. The new axial-envelope pass MUST run
// FIRST so downstream passes see the corrected DPP.
// Critically: chipload_bounds is computed BEFORE
// enforce_invariants (passed in via SuggestContext.chipload_bounds);
// if the axial pass mutates DPP, those bounds become stale relative
// to the new DPP. Re-derive bounds with the post-mutation DPP
// before chipload recalibration consumes them.

// ── NEW: axial constraint envelope passes (FIRST) ──
match op_kind {
    OperationType::Adaptive3d => pick_adaptive3d_dpp(...),
    OperationType::VCarve => pick_vcarve_max_depth(...),
    OperationType::ProjectCurve => check_projectcurve_depth_feasible(...),
    op if op.is_finish_3d() => warn_finish_envelope(...),
    _ => {} // no axial envelope work for 2D ops, drill, etc.
}

// ── NEW: chipload_bounds re-derivation (if DPP mutated above) ──
// Reviewer flagged that `chipload_bounds` is computed pre-orchestrator
// and won't reflect a mutated DPP, so downstream chipload recalibration
// targets the wrong band. Recompute via the same LUT match used to
// derive the original bounds, but with the post-axial-pass DPP.
//
// Cleaner architectural alternative (see §9.9): promote bounds to a
// method on FeedsInput that derives from current state on every call.
if dpp_mutated {
    context.chipload_bounds = recompute_chipload_bounds_with_dpp(
        tool, material, lut, cfg.depth_per_pass, /* etc. */);
}

// ── EXISTING: entry strategy / warnings (unchanged) ──
// ── EXISTING: chipload recalibration (unchanged; now reads fresh bounds) ──
// ── EXISTING: clamp / back-off / rpm-write (unchanged) ──
```

**Rev 3 — NEW prerequisite task:** `op.is_finish_3d()` (used by the
router above and §5.1's `warn_finish_envelope` scope) **does not exist
anywhere in the codebase**. Two options:

- (a) add `OperationType::is_finish_3d()` as a new exhaustive match —
  but that creates exactly the scattered-classifier debt the main
  refactor (`architectural_refactor_2026-06-06_v2.md` §2.1 row 14) is
  eliminating; if chosen, it should be a registry/spec-derived method,
  not a free-floating `matches!`;
- (b) **route on `OperationSpec.feeds_family` (catalog.rs:60)** — the
  classification already exists as data on the spec.

Recommend (b); it needs zero new per-op routing and is automatically
correct for op #24.

### 5.4 Pinned-field policy (NEW)

Reviewer flagged that "pinned" behavior for numeric fields isn't
established in rs_cam today. Strategy fields use a default-equality
heuristic ("B" pinning: `field == Default::default()` → auto-rewriteable),
but numeric Suggest passes today write calculator outputs
unconditionally.

The axial envelope sits at the intersection of "respect the user's
deliberate conservative pin" and "protect against unsafe pins". Three
options:

- **Option A — always clamp:** the envelope sets
  `cfg.depth_per_pass = safe_max` regardless. Loses user pin.
  Simplest behavior, surprises operators who deliberately wanted a
  shallower DPP.
- **Option B — warn-only on existing values:** if the user-set value
  is within `[min_floor, max_safe]`, leave it. If outside, emit
  warning but don't mutate. No protection against unsafe pins.
- **Option C — clamp down, warn up (RECOMMENDED, REV 2):** if user
  value > `max_safe`, clamp down + warn (protect against unsafe).
  If user value < `min_floor`, leave it + warn (respect deliberate
  conservative). If user value is within band, leave it silently.

```rust
fn apply_axial_envelope(cfg_value: f64, c: &CutterAxialConstraints,
                         op_kind: OperationType) -> AppliedAxial {
    let safe_max = c.safe_max_doc_mm();
    if cfg_value > safe_max {
        // Above safe band → clamp down + warn.
        AppliedAxial::Clamped { from: cfg_value, to: safe_max,
            binding: c.binding_constraint }
    } else if let Some(floor) = c.min_doc_chipload_floor_mm
              && cfg_value < floor {
        // Below burn floor → leave value + warn.
        AppliedAxial::WarnedBelow { value: cfg_value, floor }
    } else {
        // In band → leave silently.
        AppliedAxial::Kept(cfg_value)
    }
}
```

Each consumer threads its op-specific field through this policy.
The policy is centralized so all four consumers (Adaptive3d DPP,
VCarve max_depth, ProjectCurve depth, finish envelope warning)
behave consistently.

### 5.5 Pass-order interaction with v3 design

Reviewer's concern about `chipload_bounds` staleness applies to the
existing v3 work too — the entry-style pass in `pick_adaptive3d_entry_style`
mutates `entry_style` without touching DPP, so today's bounds stay
valid for that pass.

**Rev 3 correction — the rev-2 premise here was FALSE:** the axial
envelope is **not** the first pass that mutates DPP inside the
orchestrator. `clamp_dpp_to_rigidity`, `clamp_dpp_to_cutting_length`,
and `backoff_dpp_for_deflection` **already mutate DPP**
(suggest.rs:850-852) before chipload recalibration runs
(suggest.rs:865) — meaning the bounds-staleness hazard §5.3 guards
against **already exists in shipped code today**, unguarded. Two
consequences:

1. The §5.3 `chipload_bounds` re-derivation must run **after the LAST
   DPP-mutating pass** (existing clamps included), not merely "after
   the new axial pass." Place the axial pass relative to the existing
   clamps deliberately: envelope first (it sets the target), existing
   rigidity/cutting-length/deflection clamps after (they can only
   tighten), re-derive bounds once after all of them.
2. The re-derivation step is arguably a latent-bug fix for the
   existing pipeline, independent of this feature — worth landing
   first as its own small PR with a regression test (a case where a
   DPP clamp fires and the recalibrated chipload visibly shifts).

Anyone adding future DPP-mutating passes must follow the same
re-derivation pattern — §9.9 option (b) (bounds as a derived method)
eliminates the hazard class entirely and is the architectural endpoint.

## 6. Architectural boundaries (what this does NOT touch)

- **In-process stock at gen-time:** the finish op generators still
  cut to `surface + stock_to_leave` regardless of what the rough left.
  This proposal coordinates the values via Suggest **with warnings
  only** (rev 2); it does not change generation. The real fix (finish
  ops read dexel) is a separate piece of work
  (`planning/in_process_stock_TBD.md`, not yet drafted), and the
  3D-finish `stock_to_leave` auto-pick is blocked on it.
- **Automatic 3D-finish `stock_to_leave` mutation:** see §5.1.1.
  Surface as warning + rationale only in Phase 3.
- **Multi-pass finish:** same dependency on in-process stock.
- **Slope-aware DOC:** `horizontal_finish` already filters to near-flat
  surfaces; `steep_shallow` already splits by slope. The envelope does
  not duplicate that work — it consumes whatever engagement geometry
  the op-specific generator declares.
- **Rough vs finish vs semi-finish target_finish_um defaults:**
  current proposal hardcodes `None` for rough/V-carve and `25 µm` for
  finish. A `FinishQuality` enum could provide presets, but defer.

## 7. Existing building blocks reference (rev 2)

For the reviewer agent — these are the call sites the envelope leans on.
Verify the inversions land correctly:

```
tool/mod.rs:416            ToolDefinition::tip_deflection_mm   ← cutter-physics primitive (USE THIS, not predict_peak_deflection_um)
tool/mod.rs:197            ToolDefinition::lookup_diameter_at  ← LUT diameter query
tool/mod.rs                MillingCutter::chip_geometry        ← arc-mean chip
compute/cutter.rs:8        build_cutter(tool)                  ← geometry constructor (rev 2: reviewer-flagged as reuse target)
tool_load/deflection.rs:~80  sample_tip_deflection_mm          ← embedded force-from-engagement model; EXTRACT as feeds::predict::tip_deflection_from_engagement and reuse
feeds/geometry.rs:138      doc_derating_scale                  ← apply at lookup (rev 3: was :117)
feeds/mod.rs:103           ToolGeometryHint::engaged_diameter_at_doc ← engaged-D method (rev 3: NOT on tool/mod.rs)
feeds/geometry.rs:175      vbit_width_at_depth                 ← V-carve engagement
feeds/geometry.rs          scallop_stepover                    ← rotate for axial
feeds/vendor_lookup.rs:49+ LookupResult.ap_min_mm/ap_max_mm    ← EXISTING absolute caps (rev 2: reuse, don't duplicate)
feeds/vendor_lookup.rs:157 find_best_row_for_geometry          ← LUT match
feeds/mod.rs:241+          FeedsInput                          ← carries axial/radial DOC, chipload bounds, RPM, feed
feeds/mod.rs:304+          FeedsResult                         ← what Suggest writes
feeds/suggest.rs:833-865   enforce_invariants                  ← rev 2 §5.3 pass-order insertion point
feeds/suggest.rs:710-721   operation_feeds_hints               ← V-carve passes max_depth in; ProjectCurve does NOT pass depth in (rev 2 finding)
tool_load/chipload.rs:140+ steady_state_samples_for_toolpath   ← Finding-1/2/3 work
tool_load/locality.rs:135+ is_steady_state_for_gate / is_phantom_transit / is_configured_entry  ← Finding-3 split
```

**Do NOT use** (rev 2 reviewer finding):
```
feeds/predict.rs:125,154,194  predict_peak_deflection_um   ← op-aware, refuses V-bits, returns 0 for ops without depth_per_pass(). Wrong layer.
```

## 8. Test plan

### 8.1 Unit (in `feeds/cutter_constraints.rs`)

- **Deflection inversion** roundtrip (rev 3 fix — forward model is the
  SAME function the search inverts, not `predict_peak_deflection_um`,
  which is op-aware/refuses V-bits per §7): pick `ap`, run forward
  `tip_deflection_from_engagement`, capture deflection, binary-search
  invert, recover `ap` within 1 µm tolerance. Include a V-bit and a
  tapered-ball case — exactly the cutters the old closed-form claim
  broke on.
- **Vendor bound:** mock LUT row with `ap_max_factor=0.7`, query
  diameter 6 mm → 4.2 mm. Vary diameter, confirm scaling.
- **Scallop bound:** Ø6 mm ball-nose, target 25 µm scallop →
  ap ≈ 0.547 mm (closed-form verifiable).
- **Chipload floor:** flat endmill at radial = 0.1×D, solve for
  min `ap` such that arc-mean chip ≥ chipload_min. Verify monotone
  in radial WOC.
- **Tapered ball quadratic force:** at `ap = R_tip`, deflection ≈ X.
  At `ap = 2×R_tip`, deflection ≈ ~4×X (engaged D doubles, force
  doubles, lever arm constant). Sanity check.
- **`SafeBandEmpty`:** craft a tool/material/feed combo where min_floor
  > max — assert `safe_band_is_empty() == Some(true)`.

### 8.2 Integration (`tests/`)

- **Wanaka 3D Finish 6 regression:** the existing `Exceeds(Low)`
  symptom on the tapered ball should now produce a
  `FinishToolMismatched` warning at Suggest-time (band empty for
  this tool / surface combo) — operator catches it before sim.
- **V-carve safe-depth clamp:** synthetic 60° V-bit at high
  `max_depth`, hardwood — assert clamped down to deflection bound.
- **Wanaka Back Rough DPP recalibration:** the current commanded
  DPP of 3.69 mm should remain accepted (under all bounds at 6 mm
  Ø flat in hardwood with this stepover).

### 8.3 Migration (`tests/lut_migration.rs`)

- Parse every observation file post-migration. Assert no row has a
  populated `ap_rule` string without ALSO having either
  `ap_max_factor` or `ap_max_mm` set (rev 3: the EXISTING absolute
  field — `ap_max_absolute_mm` does not exist and must not be
  introduced) — except for the documented "label-only" exceptions
  (`3d finishing`, etc.) listed in §3.2.

## 9. Open questions (rev 2 — resolved + new)

### Resolved by reviewer (incorporated above)

1. ~~**Absolute-mm vs factor field:**~~ **RESOLVED §3.1:** keep
   existing `ap_*_mm` absolute fields; add only `ap_*_factor`;
   combinator returns the tighter of the two.
2. ~~**Pass order in `enforce_invariants`:**~~ **RESOLVED §5.3:**
   axial pass runs FIRST; if it mutates DPP, `chipload_bounds` is
   re-derived before the existing chipload recalibration pass runs.
3. ~~**`in_process_stock` decoupling:**~~ **RESOLVED §5.1.1:**
   automatic 3D finish `stock_to_leave` mutation is **deferred** until
   in-process stock lands. Phase 3 ships warning + rationale only.

### Still open

4. **Tapered ball-nose scallop beyond ball radius:** Phase 3 punt or
   include? Recommend: punt; warn if `ap > R_tip` on a tapered-ball
   finish (cone-flank scallop is a different formula).
5. **`FinishQuality` enum (Coarse / Standard / Fine / Mirror):**
   replaces ad-hoc `target_finish_um: f64` arg. Recommend defer until
   UX surface is designed.
6. **Per-op `radial_woc_mm` derivation:** for adaptive3d the stepover
   is the radial engagement; for V-carve the engaged width at the
   chosen depth is the proxy; for finish ops it varies. Each consumer
   derives its own value before calling the envelope. Reviewer should
   confirm this is the right boundary or push back.
7. **`AxialBindingConstraint::SafeBandEmpty` UX:** surfaces as
   warning today. Should it instead refuse Suggest entirely for that
   op (return `FeedsError`)? Recommend: warning only — refusal is
   too aggressive when the operator might be deliberately exploring.

### New from rev 2

8. **§5.4 pinned-field policy choice:** recommend policy C (clamp
   down, warn up). Reviewer should confirm before implementation.
9. **§5.3 chipload-bounds re-derivation strategy:** two options:
   (a) inline re-derivation step after axial pass; (b) promote bounds
   to a derived method on `FeedsInput` so it always reflects current
   state. Recommend (b) architecturally; (a) is simpler to implement
   first. Reviewer to pick.
10. **`tip_deflection_from_engagement` extraction site:** §4.2
    proposes extracting from `tool_load::deflection`'s embedded
    force-from-engagement code into `feeds::predict`. **Rev 3 —
    partially resolved:** the extraction shape is now characterized
    (shared tail + new pre-sim force-synthesis front-end, §4.2 risk
    note); what remains for the implementer is the gate-parity test
    proving both paths agree.
11. **Phase 3 `warn_finish_envelope` scope:** does it actually need
    per-finish-op-kind logic, or can a single shared pass that runs
    over `op.is_finish_3d()` handle it? Recommend the latter unless
    finish-op-specific differences surface during implementation.

## 10. Sizing (rev 2)

| Piece | Est. LOC | Risk |
|---|---:|---|
| Schema additions (factor fields only, reuse existing `ap_*_mm`) | ~50 | low |
| Migration binary + mapping table | ~250 | low — one-time |
| `cutter_axial_constraints` calculator (binary-search inversion) | ~350 | medium — binary search + edge cases; reuses `tip_deflection_mm` |
| `tip_deflection_from_engagement` extraction from `tool_load::deflection` | ~120 | **medium (rev 3; was low)** — only the force→deflection tail is shared (deflection.rs:101-103); the pre-sim force-synthesis front-end is new code needing a gate-parity test (§4.2 risk note) |
| Per-op consumers (3 Phase-3 passes + 1 warning-only finish pass) | ~250 | low — mirror existing passes |
| `chipload_bounds` re-derivation helper (or method on FeedsInput) | ~50 | low |
| `apply_axial_envelope` policy-C helper (§5.4) | ~60 | low |
| Tests | ~500 | low — including the policy-C matrix |
| Rationale entries (5 new variants) | ~100 | low |
| **Total** | **~1690 LOC** | — |

Slightly larger than rev 1 (~1350) — mostly from the binary search
inversion (replaces a wrongly-claimed closed-form), the bounds
re-derivation step, and the policy-C centralization. Still
comparable to the v3.0 + v3.3 work in scope.

**Scope reduction from rev 1:** automatic finish `stock_to_leave`
mutation is OUT (~200 LOC saved); replaced by `warn_finish_envelope`
warning-only path. Net rev-2 sizing reflects this swap.

---

## Pre-implementation checklist

- [ ] Reviewer agent: read current architecture, verify the building
      blocks at §7 are all where the doc says they are, and that the
      pass-order claim at §9 doesn't conflict with v3's existing flow.
- [ ] Reviewer: confirm the 46-row mapping in §3.2 with at least
      spot-checks against the source JSON files in
      `crates/rs_cam_core/data/vendor_lut/observations/`.
- [ ] Reviewer: open question §9.1 (absolute-mm field vs factor-only)
      and §9.4 (pass-order cycle check) get explicit answers.
- [ ] User: sign off on the mapping table after reviewer feedback.

Then: implement Phase 1 → Phase 2 → Phase 3 in order. Phase 1 is
independently shippable (schema additions + migration; calculator
keeps using the legacy `ap_max_mm / row.diameter_mm` until Phase 2
lands).
