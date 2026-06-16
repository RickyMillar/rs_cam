# Adaptive3d deep-channel gouge — root cause + engine-fix research

**Status:** root-caused (data-backed) 2026-06-16. Engine fix not yet implemented.
**Repro source:** wanaka100 `terrain.stl` (rivmap DEM, mountains + coastline),
`planning/airrun_2026-06-01/wanaka.toml`, Back Rough op (adaptive3d, 6mm flat
endmill, DPP 3, stepover 2.53, stock_to_leave 4mm, `face_up=Bottom` setup).

## Symptom

Every rough (agent OR spiral) carves the diagonal **coastline channel** much
deeper than the surrounding terrain — a visible gouge. Reported peak axial DOC
6–20 mm vs the 3 mm commanded DPP, always at the same channel spot
(~world XY 26,30), always at the channel floor Z.

## Root cause (CONFIRMED via `wanaka_axial_doc` diagnostic probe)

It is a **real over-cut**, not a metric artifact. The flat 6 mm endmill
surface-follows down to the **deep channel floor (setup-local z≈5.05)** while
the **adjacent terrain wall is uncleared** — the dexel probe shows real
material with `ray_top = 11.18` inside the cutter footprint right before the
peak move; the move cuts to z=5.05 → ~6 mm of real removal. The flat tool's
corner grazes/gouges the steep channel wall because **the channel is narrower
and deeper than the tool can fit**. A 6 mm flat endmill physically cannot rough
a ~5 mm-deep, sub-tool-width coastline notch without gouging its walls.

Key probe line:
`PROBE before move 3158: peak ray_top in footprint at (26.00,32.00): top=11.18`
peak move 3158: Linear z=5.053, axial DOC 6.125 mm.

## Hypotheses RULED OUT (do not re-chase)

- **Mesh defect** — STL is clean: 0 interior cracks, 0 non-manifold, 0
  overhangs/inverted faces (98.7% up-facing). Watertight DEM.
- **Path strategy** — method-independent (agent + spiral both do it). It's
  tool-vs-geometry, not path shape.
- **Stepdown size (DPP)** — halving DPP 3→1.5 made it WORSE (12.5→19.5 mm):
  finer levels → more sub-tool fragmentation.
- **Region dropping (`min_region_cut_length_mm`)** — setting 0 didn't help
  (~20 mm).
- **Stay-down bridging (`max_stay_down_distance_mm`)** — link bridges go OVER
  terrain (`link_z = max(samples).max(from).max(to)+clearance`), not through.
- **F-024 / flipped-setup frame bug (WRONG — I chased this and it's refuted)** —
  the synthetic flipped pocket test `flipped_setup_axial_doc_repro.rs` PASSES
  at 2.0 mm, so non-identity grid rooting is fine for 2.5D. And the probe shows
  real material removed, not a mis-read. The `face_up=Bottom` setup is a red
  herring.

## Repro artifacts (already in tree)

- `tests/flipped_setup_axial_doc_repro.rs` — synthetic flipped pocket, PASSES
  (2.0 mm). Keep as a sentry; it proves flipped 2.5D axial is correct and
  rules the frame theory back out if anyone re-chases it.
- `tests/wanaka_axial_doc.rs` — `#[ignore]` diagnostic that loads the real
  wanaka and probes the dexel at the peak DOC. Run with
  `cargo test --test wanaka_axial_doc -- --ignored --nocapture`. This is the
  ground-truth repro (no fast synthetic exists yet).
- `tests/dexel_stock_z_frame_f024.rs` — the IDENTITY-setup F-024 sentry
  (passes). Template for the synthetic test structure.

## Engine fix — direction

The 3D rough should **not drape the floor cut into a sub-tool-width / deep
channel** where the cutter footprint overlaps material more than ~DPP above the
cut Z. It should stop the cut above the notch and leave it for the finish
(tapered-ball) pass which can physically reach it.

Candidate fix sites:
- `adaptive3d/clearing.rs` — the `lift` closure (`z = (surf_z+stl).max(z_level)`)
  and the per-Z-level marching-squares clear. Investigate how cuts reach
  z≈5.05 BELOW the lowest planned major level (10) — surface-follow / sub-pass.
- `adaptive3d/path.rs` `segments_to_toolpath` — lift bridging + where the
  emitted cut Z is set; this is where a clamp ("don't cut more than DPP below
  the local uncleared material top") would live.
- Z-level planning (`adaptive3d/mod.rs` / `path.rs` z_levels) — does it plan
  levels down to the channel floor, or surface-follow into it on the last pass?

Acceptance test to write: a synthetic deep-narrow-channel mesh (flat terrain
with a sub-tool-width slot dropping ~6 mm), adaptive3d rough with a flat tool,
assert per-sample axial DOC ≤ ~DPP+margin (currently it will gouge the slot
walls). This is the fast repro the prior session lacked.

## PRECISE mechanism + fix (2026-06-16, traced)

- Z-level plan (`adaptive3d/path.rs:447`): `z_bottom = surface_bottom +
  stock_to_leave` (clamped by `z_floor`). `surface_bottom` = the GLOBAL min
  surface = the channel floor, so the rough plans a level right at the channel
  floor (z≈5.05). That level's cut is real.
- The `SurfaceHeightmap` IS cutter-radius-aware (drop-cutter, `slope.rs:89`
  `point_drop_cutter`), so the cutter CENTRE correctly finds the channel floor
  at a cell wide enough to drop into.
- **THE BUG:** the effective floor / lift uses the **point** surface, not the
  **cutter-radius-dilated** surface:
  - `clearing.rs:373` `effective_floor = (surf_z + stock_to_leave).max(z_level)`
  - the lift closure `z = (surf_z + stl).max(z_level)`
  Both read `surf_z` at the single cell. The cutter has radius ~3 mm, so cutting
  a channel cell to z=5.05 removes material across the footprint — including the
  adjacent wall cell whose leave-level is 11.18 → 6 mm wall gouge.

- **FIX:** dilate `(surf_z + stock_to_leave)` by the cutter (engagement) radius
  — i.e. the effective floor at cell A = `max over footprint(A) of
  (surf + stl)`, then `.max(z_level)`. A morphological max-filter of the
  leave-surface by the cutter radius. In a sub-tool channel the dilated floor =
  the wall height, so the rough leaves the notch (correct — flat tool can't fit)
  and the tapered-ball finish carves it. No wall gouge.
  - Apply in `build_material_bool_grid` (clearing.rs:334/373) AND the lift
    (clearing.rs ~1599) — both must use the dilated floor for consistency
    (mask decides WHERE to cut, lift decides the Z).
  - Cheapest impl: precompute a radius-dilated copy of the SurfaceHeightmap
    once per op (max-filter `surf_z` over a disk of the engagement radius in
    grid cells), pass it alongside the raw heightmap; clearing reads the
    dilated one for floor/mask decisions. Keep the raw heightmap for slope/
    flat-area detection where the true surface is wanted.
  - Watch: don't dilate so aggressively that legitimately-wide pockets get
    under-cut. The dilation radius = engagement radius (contact radius at DOC),
    not envelope radius. Validate against the existing F-024/F-029/F-036 sentries
    + the AS001 acceptance (pockets must still bottom out correctly).
  - Risk: this changes EVERY adaptive3d rough's floor slightly (leaves a hair
    more in concave fillets). Gate hard against the sweep + sentry suite.

## Side findings (separate bugs, worth logging)

- **Spiral generation hangs at near-slot stepover (≥~80% tool diameter)** — the
  iso-contour wrap extraction degenerates (multi-hour gen). Needs a hard cap /
  early-bail at ~80% diameter.
- **Full-job sim wedges for 45+ min** on the wanaka job (dominated by the
  112k-move tapered-ball finish op + the deep-channel state). Isolating the
  rough (disable other toolpaths) drops sim time ~13×.
- `run_simulation`'s `per_toolpath` lists disabled toolpaths (reporting noise;
  confirm they're excluded from the aggregate metrics/compute).

## Also from this session (unrelated, already committed on experiment/adaptive-spiral)

- Nibble control built then removed (commit ae6f5f3) — trochoid relief is a
  corner-relief edge case, inert on open roughing; "Optimal load" slider +
  tuned `trochoid_cap_mult=1.6` kept. See `[[project_spiral_wallclock_structural]]`.
