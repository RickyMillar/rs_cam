# Tool Kinematics / Chipload System — Senior-Engineer Audit

Authored 2026-05-31 after the Phase 1+2B+2C ingest landed. Goal: surface
duplicate parallel logic, stale magic numbers, dead code, and pattern
drift before Phase 4 promotion gets a chance to ossify any of it.

The Phase 4 LUT promotion is the next milestone, but landing 135 more
rows on top of an inconsistent plumbing layer would calcify the
inconsistencies. This audit drives a focused consolidation pass first.

## Severity 1 — real correctness / safety surface

### S1-1. Two different power formulas in production

The Suggest path and the Sim-verdict path predict spindle power
**differently** for the same toolpath:

| Path | Source | Formula | Geometry |
|------|--------|---------|----------|
| Suggest | `feeds/mod.rs::calculate` Step 6 | `P = kc · ap · ae · feed / 60e6` | rectangular slab (`ap×ae` direct) |
| Sim verdict | `tool_load/power::evaluate` | `P = GRAIN_ANISOTROPY_FACTOR · kc · cross_section_mm2(doc, woc) · feed / 60e6` | arc-engagement slab; V-bit cuts triangular (½ area) |

**Result:** the Suggest panel can tell you a 6 mm V-bit toolpath is
fine, then the Sim verdict Exceeds the same toolpath, because they
bake fundamentally different physics. The 2.0× grain factor is in one
path and not the other. The V-bit's triangular cross-section is in
one path and not the other.

**Fix:** extract a shared `predicted_power_kw(kc, cross_section, feed,
*, grain_factor)` helper; both Suggest's "is this feed power-safe?"
ramp and the Sim verdict's "did we exceed available?" check call it.
The Suggest path will then correctly downrate when the V-bit's
triangular cross-section halves predicted power, and the grain factor
applies consistently.

### S1-2. V-bit angle-aware LUT lookup is gate-only

`find_best_vbit_row(lut, criteria, query_angle)` was added so the
chipload gate can match a 30° V-bit to a 30° row instead of any
diameter-matched row. **Only `tool_load/chipload.rs` calls it.**

Everyone else — `feeds/mod.rs::calculate`, `feeds/explain.rs`,
`feeds/suggest.rs`, the MCP probes, the GUI feeds modal — uses
`find_best_row` which scores rows ignoring `included_angle_deg`.

**Result:** the Suggest panel for a 30° V-bit will silently match an
unrelated angle row.

**Fix:** make a single `find_best_row_for_geometry(lut, criteria,
&ToolGeometryHint)` entry point that dispatches:
- `ToolGeometryHint::VBit { included_angle, .. }` → `find_best_vbit_row(lut, criteria, Some(included_angle))`
- anything else → `find_best_row(lut, criteria)`

Migrate all callers (chipload.rs already does this manually with a
match; the helper centralizes it).

### S1-3. Plastic hardness — two disagreeing sources of truth

`PlasticFamily::hardness()` was added in Phase 1A as the canonical,
citation-anchored accessor. **Zero non-test callers** reference it.

`vendor_normalize::material_to_lut` (the LUT-query path) has its own
hardcoded plastic hardness table with different values:

| Plastic | `PlasticFamily::hardness()` (canonical) | `vendor_normalize` (lookup) |
|---------|-----------------------------------------|------------------------------|
| HDPE | `ShoreD(64.0)` — Direct Plastics ISO 868 | `ShoreD 65.0` — inline |
| Polycarbonate | `ShoreD(80.0)` — Treatstock ASTM D2240 | `ShoreD 80.0` — inline (matches) |
| Delrin | `ShoreD(86.0)` — Alro ASTM D2240 | `ShoreD 85.0` — inline |
| Acrylic | `RockwellM(93.0)` — MakeItFrom PMMA | `ShoreD 85.0` — inline (different scale!) |

The Acrylic case is the worst: the canonical accessor says Rockwell M
93, the LUT lookup uses Shore D 85. These aren't even on the same
scale.

**Fix:** make `vendor_normalize::material_to_lut` call
`PlasticFamily::hardness()` and translate to the LUT's
`HardnessKind`+value pair from there. Drop the inline table.

### S1-4. Stale anisotropy magic number in tests

`tests/constrained_max_modulation_f039.rs:215` still hardcodes
`kc_eff_n_per_mm2: 2.5 * 30.0` — using the pre-Phase-2B value. The
production constant is `GRAIN_ANISOTROPY_FACTOR = 2.0`. The test
fixture is a self-contained constraint-binding scenario (it doesn't
need to track the physical factor), but the literal `2.5` is now a
stale archeological reference. Either replace with the live constant
import or comment "fixture value, not physics-derived".

## Severity 2 — architecture / maintainability

### S2-5. `vendor_normalize::material_to_lut` duplicates per-family accessors

The function inlines janka values for plywood (600/1200/1000),
sheet goods (Mdf 1100, Hdf 1300, Particleboard 750), and foam (200
fallback) that ALSO live on the per-family enums via
`PlywoodGrade::effective_janka_lbf()` and
`SheetGoodKind::effective_janka_lbf()`. The values currently match,
but as the Phase 3 Wood Database / FPL extract data lands as
per-grade refinement, the inline table would drift. Refactor to call
the enum methods directly.

### S2-6. `GRAIN_ANISOTROPY_FACTOR` is private to `tool_load/power.rs`

Defined as a file-scoped `const`. Other consumers
(`session/compute.rs::compute` populating `PowerLimitInputs::kc_eff_n_per_mm2`,
the constrained-max test fixture) hardcode `2.0 *` and `2.5 *`
respectively. Make it `pub(crate) const` exported from
`tool_load::power` so there's exactly one place to bump.

### S2-7. No `tracing` in `feeds` / `tool_load` / `material`

The crate uses `tracing` throughout (~20 modules with at least one
call). `feeds/mod.rs`, all `tool_load/*.rs`, and `material.rs` have
zero. A refuse/Exceeds verdict firing in production leaves no log
trace. Mid-priority: add `tracing::debug!` at gate-refusal sites and
`tracing::warn!` at Exceeds verdicts so production traces include
the gate decision chain.

### S2-8. `Material::hardness_index()` conflates two concepts

The doc string says "Normalized hardness index. 1.0 = soft wood
baseline (Janka 600 lbf)" but:
- For plastics it's a hardcoded `0.5` (no per-family discrimination).
- For aluminum it derives from Brinell via `(hb/60).powf(0.4)` — a
  placeholder, since Janka and Brinell aren't on the same scale.
- For foam it's hardcoded per density.

The consumers (`feeds/mod.rs::calculate`) treat this as a general
scalar driving feed-rate derates. The accessor's name and doc imply
wood-normalization. Either rename to make the wood-baseline
explicit, or split into a wood-specific `wood_hardness_factor` and
a general `feed_scale_factor` with documented semantics per class.

### S2-9. `feed_modulation::{DeflectionLimitInputs,PowerLimitInputs}` are leaky

Both structs take pre-multiplied values (kc, kc_eff_with_anisotropy,
engagement_diameter, Young's modulus). The caller has to know to
apply `GRAIN_ANISOTROPY_FACTOR` to derive `kc_eff_n_per_mm2`. Better
shape:
```rust
pub struct PowerLimitInputs<'a> {
    pub material: &'a Material,
    pub tool: &'a ToolDefinition,
    pub engagement_diameter_mm: f64,
    pub available_kw: f64,
}
```
and let the solver internally compute `kc_eff` via the canonical
helper. Same for `DeflectionLimitInputs`. Defers the anisotropy
arithmetic to one place.

## Severity 3 — testing / documentation hygiene

### S3-10. `PlasticFamily::hardness()` is dead code

Zero non-test callers. S1-3 fix turns this live.

### S3-11. No literature-parity tests

`planning/data_ingest_2026-05-29/kc.md` /
`planning/data_ingest_2026-05-30/fpl_ch5_extract.md` /
`planning/data_ingest_2026-05-29/hardness.md` exist as
verbatim-quoted reference data. Nothing pins the live `Material::*`
accessors to these tables. A future bump that drifts from
literature won't fire any test.

**Fix:** add `crates/rs_cam_core/tests/literature_parity.rs` that
loads each entry from the markdown reference tables and asserts the
live accessor returns the literature value within tolerance. The
failure message includes the citation.

### S3-12. Custom Material is half-validated

```rust
Material::Custom { name: _, hardness_index: hi, kc } =>
    if kc.is_finite() && *kc > 0.0 { Some(*kc) } else { None }
```
Kc gets `is_finite + >0` validation; `hardness_index` propagates
raw (a user supplying `-1.0` triggers downstream NaN/infinity).
Validate symmetrically or document.

### S3-13. Test-fixture clutter

Multiple files construct `Material::Custom { name: "...", hardness_index: 1.0, kc: 10.0 }` ad hoc (`tool_load/power.rs:470`, `tool_load/deflection.rs:456`, `tool_load/optimize/mod.rs:859`, `compute/validate.rs:538`). A shared `Material::test_fixture_custom()` would centralize.

## Fix priority

Land in this order (each fix is a small surgical commit):

1. **S1-1** — extract shared `predicted_power_kw` helper, both paths call it.
2. **S2-6** — `pub(crate) const GRAIN_ANISOTROPY_FACTOR` accessible from the shared helper.
3. **S1-2** — `find_best_row_for_geometry` dispatcher, migrate callers.
4. **S1-3 + S2-5** — `vendor_normalize::material_to_lut` calls the per-family accessors.
5. **S1-4** — replace the stale `2.5` literal in the F-039 test fixture.
6. **S3-11** — literature-parity test harness.
7. **S2-7** — tracing instrumentation at gate-refusal and Exceeds sites.

S2-8, S2-9, S3-12, S3-13 are lower-priority cleanups deferred to a
follow-up.

## What this audit does NOT cover

- The `tool_load::optimize` module — separate concern; the optimizer
  reads the gate verdicts but doesn't synthesize its own physics.
  Worth a follow-up audit but not blocking.
- The `simulation_cut` sample pipeline — feeds inputs into the gates;
  not part of the kinematics/chipload model itself.
- GUI presentation of verdicts — separate UX concern.
