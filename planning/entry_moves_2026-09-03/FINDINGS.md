# G-RAMPTERRAIN — findings and pre-registered bars

Charter: `CHARTER.md`. Discovery record:
`planning/metrology_2026-09-02/FINDINGS.md` §G-ISOCHANNEL,
§G-RAMPTERRAIN. Operator ruling: "All entry moves should be stock
aware."

Rules of this file: each bar is written BEFORE its instrument runs.
Results append below the bar. A result does not edit a bar.

## A0 — audit: which entry motions are terrain-blind (measured from code, 2026-09-03)

The dressup layer (`dressup.rs::apply_entry`) rewrites each detected
plunge into ramp or helix moves. `emit_ramp` draws two straight legs.
Each leg is `clearance / tan(angle)` / 2 long in XY. At the default
3° angle and 2.0 mm clearance, each leg is ~19.1 mm and drops ~1.0 mm.
No code reads the mesh or the stock along the leg. This matches the
measured wanaka chords (877 chords, 19–21 mm, 1.0 mm drop).

Default `entry_style` per operation type
(`DressupConfig::for_role` + the registry `dressup_policy`,
pairing verified by script against `compute/catalog.rs`):

| Default | Operation types |
|---------|-----------------|
| `Ramp` (blind legs) | Face, Pocket, Profile, VCarve, Rest, Inlay, Zigzag, Chamfer, Waterline, Pencil, **Scallop**, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish |
| `Helix` (blind circle) | Adaptive (`PREFER_HELIX`) |
| `None` (forced) | Trace, Drill, Adaptive3d (dressup layer), AlignmentPinDrill |
| stripped (all dressups) | DropCutter, UnifiedFinish, ProjectCurve |

Findings from the audit:

1. **The whole mesh-finish family except UnifiedFinish and DropCutter
   ships blind ramp entries by default.** These operations cut ON the
   model surface, so a blind leg can only be neutral or a gouge.
2. **The same family also ships `lead_in_out = true`** (Finish role).
   A lead-in arc is a lateral move at cut Z with radius 2.0 mm,
   also drawn without a surface probe. Same class, smaller reach.
3. **Adaptive3d bypasses the dressup layer** and calls
   `dressup::emit_ramp` / `emit_helix` directly
   (`adaptive3d/path.rs:1388,1551`). The entry DESTINATION is draped
   to `surface + stock_to_leave` (`drape_point`), but the ramp legs
   and the helix circle are not. Its default entry style is `Plunge`,
   so the exposure needs a user opt-in.
4. **2D-only operations (no mesh) have no surface to probe.** For a
   prismatic pocket the ramp legs cut the material between passes —
   that is the purpose of a ramp. The blind-leg defect is a
   mesh-surface defect.

## S1 — the standing sentry (pre-registered before the instrument ran)

Instrument: `buried_entry_chords` (new `pub fn`, core), test
`crates/rs_cam_core/tests/entry_moves_stock_aware_g_rampterrain.rs`.

Check: on the FINAL dressed toolpath (after lead-in, arc fit, and
merge — not on `apply_entry` output), take every fed move whose
intent is `EntryRamp`, `EntryHelix`, `EntryPlunge`, `LeadIn`, or
`LeadOut`. Sample each move every 0.5 mm, arcs along the arc. At each
sample compute the drop-cutter CL height with the operation's cutter.
Burial = `(cl_z + stock_to_leave) − sample_z`.

Fixture: a Gaussian ridge `height_field` mesh. The ridge top rises
≥ 5 mm within 19 mm of the entry point, so a blind 3° ramp leg must
bury by several mm. The ridge is smooth, so generator chord sag stays
far below tolerance.

- **Bar S1-red (instrument validity):** on the fixture, BEFORE the
  fix, the checker must report at least one entry-intent sample
  buried > 1.0 mm. If it does not, the instrument cannot see the
  measured defect class and must not become a sentry.
- **Bar S1-green (the gate):** AFTER the fix, on the same fixture, no
  entry-intent sample may bury more than **0.2 mm** (tolerance for
  facet sag + 0.5 mm sampling). This test is the standing sentry and
  runs in the normal (non-ignored) suite.

## I1 — all-feeds burial census (instrument, report-only)

Same checker, run over ALL fed moves, split by intent, tolerance
0.5 mm (the strict bar the discovery A/B used). This is NOT a gate:
the discovery record already names a residual generator-side class
(short chords through a knoll, candidate arc-fit) that this census
will show and that G-RAMPTERRAIN does not own. Evidence use only.

## Fix design (charter option (a), decided before implementation)

`EntrySurfaceProbe` (mesh + spatial index + cutter + stock_to_leave)
rides as `Option` into `apply_dressups` → `apply_entry` →
`emit_ramp` / `emit_helix`.

- With a probe: sample each ramp leg / helix turn every 0.5 mm. Lift
  each sample to `max(planned z, cl_z + stock_to_leave)`. The entry
  still ramps where the surface allows it and follows the surface
  where it does not. Never lower a point.
- If the probe loses surface contact at any sample (off the mesh):
  fall back to the straight plunge for that entry. The plunge target
  is the first cut point, which the generator already placed on the
  intended surface — stock-aware by construction (charter option (c)).
- Without a probe (2D operations, no mesh): behaviour unchanged.
  This is honest per audit finding 4.
- Adaptive3d's direct `emit_ramp` / `emit_helix` call sites get the
  same probe (mesh, index, cutter, and `stock_to_leave` are in scope
  there).

Decision rule: if the clipped entries still bury > 0.2 mm on the
fixture, stop and report — do not widen the tolerance.

## Follow-ons (ledgered, not blocking)

- **Lead-in/lead-out arcs** are the same class with a 2 mm reach.
  The I1 census measures them. Fix or refusal is a separate decision
  after the measurement.
- **Triage promotion:** deep-biting entry chords currently surface
  only as the `entry_load` load caution. The checker is a `pub fn` so
  a later change can add a safety row. Not in this change.
- **2.5D ramp containment:** a 19 mm leg can leave a small pocket and
  side-cut a wall. Different geometry class (2D containment), not
  measured here.

## Results

### S1-red — the instrument sees the defect class (2026-09-03, pre-fix)

`cargo test -p rs_cam_core --test entry_moves_stock_aware_g_rampterrain`,
run with the probe plumbed through but the clip not yet implemented
(the probe was inert, so this measures shipped behaviour):

- Ramp arm: **2 buried entry moves, worst 5.14 mm** at
  (10.8, 0.0, 1.79), chord 19.1 mm, intent `EntryRamp`. The chord
  length matches the wanaka signature (19–21 mm legs).
- Helix arm: **3 buried entry moves, worst 1.02 mm**, intent
  `EntryHelix`.
- No-intrusion parity arm: green (two legs, no burial) — the legacy
  shape away from terrain is pinned before the fix.

Bar S1-red (> 1.0 mm) PASSES on both arms. The instrument sees the
class. The two red arms carry `#[ignore]` in the instrument commit so
the shared gate stays green; the fix commit removes the attribute and
they become the standing sentry.
