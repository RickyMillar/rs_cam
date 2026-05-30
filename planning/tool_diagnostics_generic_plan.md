# Generic, per-shape tool diagnostics — implementation plan

Goal: make every tool type (flat, ball, bull-nose, V-bit, tapered ball,
facing, drill) carry **first-class** feeds + tool-load diagnostics, with the
shape-specific math implemented per type behind shared traits — so V-bits
(and the rest) stop riding on cylinder-shaped approximations. Authored to be
executed **while datasets are being sourced**; most of it is data-independent.

Companion docs: `planning/feeds_data_coverage_audit_2026-05-29.md` (data
gaps) and the V-bit diagnostics audit (geometry OK, gates not cone-aware).

## Design principle

> **Geometry and shape-physics live behind the tool trait; the gates own the
> material + machine physics and ask the trait for every shape-dependent
> quantity.** One shape = one place.

This is already the codebase's pattern. `MillingCutter`
(`crates/rs_cam_core/src/tool/mod.rs:149`) is implemented per type
(`tool/{flat,ball,bullnose,tapered_ball,vbit}.rs`) and already exposes
cone-aware geometry: `chip_geometry`, `engagement_radius`,
`width_at_height`, `lookup_diameter_at`, `geometry_hint`, `profile_points`.
The gates (`tool_load/{chipload,deflection,power}.rs`) already *take* a cutter.
The work is to (a) add the few missing shape-dependent quantities to the trait,
(b) refactor each gate to consume them instead of baking in a cylinder, and
(c) make the feeds LUT match V-bits on geometry.

### Why trait default methods (not a new parallel trait)

Add the new diagnostic-geometry methods **to `MillingCutter` with default
impls that reproduce today's cylinder behavior**, then override per shape.
- Zero regression: flat/facing inherit the defaults → identical numbers.
- No second dyn-dispatch object to thread through the gates (they already hold
  a `&dyn MillingCutter` / `ToolDefinition`).
- Mirrors the existing `chip_geometry` precedent (a diagnostic method already
  lives on this trait).
Keep pure-geometry methods and these diagnostic-geometry methods grouped with
a `// ── diagnostics geometry ──` banner so the boundary stays legible.

## Trait additions (`tool/mod.rs`)

Each is `fn …(&self, …) -> …` with a **default** = current behavior; V-bit /
ball / tapered-ball / bull-nose override. (Confirm exact current formulas in
the gates during implementation; signatures below are the contract.)

1. `effective_cutting_diameter_at(&self, doc_mm: f64) -> f64`
   - Chipload + LUT matching. Default `diameter()`. V-bit/ball/tapered return
     `2 * engagement_radius(doc)` (already computable from `width_at_height`).
   - Replaces the "fake fixed diameter" the V-bit LUT query uses today
     (`feeds/vendor_normalize.rs:77-80`).

2. `mrr_cross_section_mm2(&self, doc_mm: f64, woc_mm: f64) -> f64`
   - Power gate. Default rectangular `doc*woc`. **V-bit: triangular**
     (`0.5 * width * doc`, width from the included angle). Ball/tapered:
     circular-segment. This is the core power fix.

3. `section_diameter_profile(&self, n: usize) -> Vec<(f64 /*h above tip*/, f64 /*dia*/)>`
   - Deflection gate. Default constant `diameter()` over the flute length.
     V-bit/tapered return the true taper (≈0 at the tip → full at shank). The
     cantilever integrator (`deflection.rs`) integrates this profile, so the
     near-zero-section tip is represented and a pointed tool no longer reports
     a reassuring-but-fake stiffness.

4. `cutting_force_arm(&self, sample) -> f64` (or reuse engaged-contact centroid)
   - Where along the axis the cutting force is applied for the bending moment.
     Default mid-engagement (today's behavior). V-bit: near the tip (worst
     case), which is what makes the deflection verdict meaningful.

5. Extend `chip_geometry` handling for **tip-only engagement**: V-bit currently
   rejects `DOC < 0.05 mm` (`tool/vbit.rs:78-83`), so V-carve cusps near
   boundaries produce no chip metric. Add a V-bit chip model that returns a
   small-but-real chip thickness in the tip region instead of an error.

6. `flat_tip_diameter(&self) -> f64` — default `0.0`; lets flat-tip / engraving
   V-bits be modeled (today `tip_diameter` is hardcoded `0.0`,
   `tool/vbit.rs:96`). Plumb from `ToolConfig` (needs a `tip_diameter` field;
   see Phase 2).

## Gate refactors (`tool_load/`)

- **chipload.rs (`evaluate`, ~237):** use `effective_cutting_diameter_at(doc)`
  for both the chip-thickness denominator and the LUT lookup diameter. For
  V-bits this is the difference between a real chipload and `NoVendorData`.
- **power.rs (`evaluate`, ~39):** compute MRR from `mrr_cross_section_mm2 ×
  feed`, not `doc*woc*feed`. Keep `Kc` from material (see data plan).
- **deflection.rs (`sample_tip_deflection_mm`, ~78):** integrate the cantilever
  over `section_diameter_profile()` with the force applied at
  `cutting_force_arm`. Flat tools unchanged (constant profile = today).
- Verdicts (`verdict.rs`) unchanged in shape — they already carry typed
  Within/Exceeds/Unmodeled; the inputs just get correct.

## Feeds / LUT (`feeds/`)

7. **LUT schema add** (`VendorObservation`): `included_angle_deg: Option<f64>`,
   `tip_diameter_mm: Option<f64>`. Backward-compatible (Option).
8. **Geometry-aware matching** (`vendor_lookup.rs:225-258`): for
   `ToolFamily::ChamferVbit`, match rows by `included_angle_deg` (± tolerance)
   and compare on `effective_cutting_diameter_at(typical_doc)` rather than raw
   diameter. This is the **highest-leverage fix** and the slot the incoming
   V-groove datasets need.
9. Keep the empirical fallback, but parameterize K0/p/q so a V-bit/material can
   override the single global curve (`machine.rs:39-41`) — staged with data.

## V-carve-specific quality diagnostics (new, Phase 4)

New module `tool_load/vcarve_quality.rs` (analysis only, no gate coupling):
- **Line width = f(depth, included angle)** — verify emitted groove width
  matches intent; flag depth that over/under-runs target width.
- **Corner reach / medial-axis under-cut** — V-carve's "can't reach": detect
  concave junctions the bit can't fully enter.
- **Flat-bottom cusp height** on wide grooves; **tip-drag** at zero-width
  regions (minimum engagement).

## Tests

- **Regression guard (do first):** golden tests asserting flat-endmill and
  ball chipload/power/deflection are **byte-identical** before/after the trait
  defaults land (proves the refactor is behavior-preserving for existing shapes).
- **Per-shape goldens:** a 90° V-bit at a known depth → hand-computed effective
  diameter, triangular MRR, and a deflection that is *higher* than the old flat
  model (sign + order-of-magnitude check).
- **Tip-only engagement:** V-carve cusp returns a chip metric, not an error.
- **LUT match:** a V-groove row with `included_angle_deg=90` matches a 90°
  V-bit and is rejected for a 60° one.
- Live MCP smoke: re-run the Wanaka V-carve/chamfer, confirm chipload moves off
  `NoVendorData` once a matching row exists.

## Sequencing (data-independent first)

- **Phase 1 — shape-correct math (NO data needed):** trait additions 1–5 +
  gate refactors + regression/per-shape tests. After this the V-bit chipload,
  power, and deflection numbers are *correctly shaped* even on the current LUT.
- **Phase 2 — schema & matching (data-prep):** trait #6/#7/#8 + `ToolConfig`
  `tip_diameter` field + GUI/serde plumbing. Makes the LUT ready to receive
  V-groove data.
- **Phase 3 — data (when datasets land):** populate V-groove rows, per-material
  Kc, plastics breakdown; calibrate against the new goldens. Sourcing is
  catalogued in `planning/feeds_data_source_acquisition_2026-05-29.md`
  (citeable primary sources + evidence grades + ranked ingestion order);
  gaps it closes are in `planning/feeds_data_coverage_audit_2026-05-29.md`.
- **Phase 4 — polish:** `vcarve_quality.rs`, flat-tip bits, per-tool empirical
  curve overrides.

## Implementation status (2026-05-29)

**Phase 1 — shipped.**
- `MillingCutter::mrr_cross_section_mm2(doc, woc)` added (default rectangular
  `doc·woc`; `VBitEndmill` overrides to triangular `0.5·doc·woc`). Power gate
  (`power.rs`) now asks the cutter for the cross-section instead of baking in a
  rectangular slab — V-bit predicted power halves, flat/ball unchanged.
- `VBitEndmill::chip_geometry` tip-only rejection relaxed: any positive depth
  now yields a real small chip; only literal zero-depth tip contact errors.
- Discovery during impl: plan items #1 (`effective_cutting_diameter_at`) and #3
  (`section_diameter_profile`) were **already served** by the existing
  `lookup_diameter_at` / `engagement_radius` — chipload already queries the
  engaged diameter and the deflection integrator already integrates the per-
  axial taper, so V-bit deflection already reflects the thin tip. No redundant
  alias methods were added. Item #4 (`cutting_force_arm`) deferred — the
  section taper already does the heavy lifting; revisit with data.

**Phase 2 — shipped (matching) / partial (tip plumbing).**
- LUT schema: `included_angle_deg` + `tip_diameter_mm` added to
  `VendorObservation` (serde-default, backward compatible). `amana_vbit.json`
  backfilled (insert-vgroove = 90°, Whiteside 120-series = 120°).
- `find_best_vbit_row` added: angle-tolerance gate (±20°) + proximity bonus;
  chipload gate routes `ChamferVbit` lookups to it. Angle-less rows pass
  through unchanged, so non-V-bit and legacy lookups never regress.
- `flat_tip_diameter()` trait method (default 0.0) + `VBitEndmill.tip_diameter`
  field + `geometry_hint` wiring — the **generic trait-level** flat-tip
  contract is in place.
- **Deferred:** the `ToolConfig.tip_diameter` → project-file → GUI plumbing.
  It triggers a ~20-site struct-literal cascade across both crates and the
  integration suite with no live consumer yet (the consumer is Phase 4's
  V-carve quality / flat-tip geometry). Land it together with that consumer.

## Out of scope / guardrails

- Don't rebuild `MillingCutter` geometry — it's correct and unit-tested.
- Honesty over false precision: any quantity we can't yet model returns
  `Unmodeled` with a typed reason (today's pattern), never a fabricated number.
- Zero-warning clippy; tests adjacent to code; one PR per phase.
