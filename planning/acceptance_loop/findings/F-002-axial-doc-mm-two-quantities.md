# F-002 — `peak_axial_doc_mm` carries two physical quantities

- **Stage:** sim
- **Severity:** high
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~150 LOC + fixture refresh)
- **Linked PRs:** —
- **Verified in round:** —
- **Source audits:** unification + smoke-run evidence

## Evidence

`crates/rs_cam_core/src/dexel_stock/stamping.rs:374-399`
`stamp_segment_with_metrics`:
- Lateral segments (line 372) return the **max per-cell material
  length removed** at the cutter footprint.
- Pure-vertical segments (line 395) return the **Z descent** of the
  segment.

Both flow into `SimulationCutSample.axial_doc_mm` at
`simulation_cut.rs:146`, and bubble into `peak_axial_doc_mm` on five
summary structs at `simulation_cut.rs:288, 328, 377, 402, 434`. Same
field name, same `f64`, two different physical units depending on
which kinematics class produced the sample. The CLAUDE.md "Metric
caveats" block already warns about this in prose; the code has not
been split.

## Smoke evidence

Cases AS001 / 2 / 3 / 5 / 7 / 13 / 14 / 15 / 17 in round-01 baseline
all reported `peak_axial_doc_mm = 12.0` (the full stock height on
2D pocket templates) or `10.5` (on the 10mm STEP plate) even when
commanded `depth_per_pass` was 2. The deflection gate consumed this
and reported 374–573 µm tip deflection, blowing past its 200 µm
threshold on 9/13 cases. The numbers are *correct* for "material
height above cutter at this point" but **wrong** for "engaged axial
DOC", which is what the deflection model wants.

## Acceptance test

1. **Unit test** (must be in PR): construct a sim trace with one
   lateral sample over 12mm tall uncleared stock at cutter Z = -2mm,
   and one plunge sample descending 3mm. Assert
   `lateral.axial_engagement_mm == 2.0` (engaged depth, not 12) and
   `lateral.plunge_descent_mm == 0`. Assert
   `plunge.plunge_descent_mm == 3.0` and `plunge.axial_engagement_mm == 0`.
2. **Smoke verification** (auditor's next round):
   - AS001 pocket (commanded dpp=2): `deflection.peak_mm` drops from
     374 µm into the < 200 µm Within range.
   - AS011 drill (peck_depth=3): drill_summaries unchanged;
     `plunge_descent_mm` per-peck samples == 3.0; `axial_engagement_mm` == 0.
   - AS013 adaptive3d (dpp=3): `deflection.peak_mm` drops from 573 µm
     to a realistic value.

## Files

- `crates/rs_cam_core/src/dexel_stock/stamping.rs:374-399` — split the
  return of `stamp_segment_with_metrics` into two distinct quantities
- `crates/rs_cam_core/src/simulation_cut.rs:146` —
  `SimulationCutSample` field set
- `crates/rs_cam_core/src/simulation_cut.rs:288, 328, 377, 402, 434` —
  five summary structs propagating `peak_axial_doc_mm`
- `crates/rs_cam_core/src/tool_load/deflection.rs` — must switch to
  consume `axial_engagement_mm` (not the plunge field)
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` — display surfaces
- `crates/rs_cam_core/src/fingerprint.rs` — fingerprint capture; may
  need regen on `target/param_sweeps/` if it captures axial_doc_mm

## Fix shape

- Rename lateral-feed return path → `axial_engagement_mm`
- Add new field `plunge_descent_mm` on `SimulationCutSample` and on
  the five summary structs
- Lateral feed samples write `plunge_descent_mm = 0`; plunges write
  `axial_engagement_mm = 0`
- Deflection gate reads `axial_engagement_mm` only
- Update `CLAUDE.md` "Metric caveats" — delete the
  `peak_axial_doc_mm` ambiguity warning since it's no longer ambiguous

## Risk

Medium. Schema change touches 5 summary structs and may invalidate
on-disk fingerprints from `param_sweep`. Regenerate fixtures in the
same PR with a clear commit note.

## Notes

- **Out of scope:** changing the deflection model's coefficients or
  bounds.
- **Out of scope:** adding a third "commanded DOC" field — the dressup
  layer already knows commanded DOC; we just need to stop conflating
  measured engaged DOC with measured plunge descent at the sample
  level.
