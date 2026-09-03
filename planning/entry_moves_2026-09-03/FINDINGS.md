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

## S2 — lead-in/out clip (pre-registered before the instrument ran)

Scope widened by `25d80035` (metrology lane): the buried-chord A/B
found the family's member 3 — with `lead_in_out = true` the entry
plunge lands `lead_radius` away from the ring start and the lead arc
approaches HORIZONTALLY at ring depth through standing terrain. The
emitter confirms it in code: `apply_lead_in_out_with_provenance`
emits the lead plunge to `(lead_start.xy, cut_z)` and eight arc
samples all at `cut_z`, with no surface probe.

Fix design (decided before implementation): the lead-in/out dressup
takes the same optional `EntrySurfaceProbe`.

- With a probe: lift the lead plunge target and every arc sample to
  `max(cut_z, floor)`. The lead still approaches tangentially in XY;
  in Z it follows the surface down into the ring start, whose own
  floor equals `cut_z`.
- If the probe loses contact at any lead sample: SKIP the insertion
  for that pass — the generator's original plunge/retract stays,
  which is the pre-dressup shape and stock-aware by construction.
- Without a probe: behaviour unchanged.
- Zero-churn parity: when no sample lifts, the emitted moves are
  byte-identical to today's.

Bars:

- **S2-red (instrument validity):** ridge fixture, `lead_in_out =
  true`, entry beside the ridge so the lead circle reaches uphill;
  BEFORE the fix at least one `LeadIn`- or `EntryPlunge`-intent
  sample buried > 0.5 mm.
- **S2-green (gate):** AFTER the fix, no entry-intent sample buried
  > 0.2 mm on the same fixture, and the lead still carries `LeadIn`
  moves.

## S3 — sagging refit arcs (pre-registered; mechanism not yet reproduced synthetically)

Family member 2 (`25d80035`): with `arc_fitting = true` (tol 0.05)
the wanaka triage carries an `entry_load` CRITICAL of 2 242 samples,
peak 3.90 mm, which VANISHES entirely with arcs off. That A/B is the
S3-red — measured on wanaka by the metrology lane; this file does not
re-run it.

**Mechanism FOUND by code reading (2026-09-03), before any guard was
built:** `try_fit_arc`'s Z validity check has an endpoint hole. When
`|z_end − z_start| ≤ tolerance` the run is accepted as "constant-Z"
with NO per-point check — so a ring run whose Z rises over a knoll
mid-run and returns to level collapses into a flat arc THROUGH the
knoll. The emitted arc can bury by the full knoll height while both
endpoints sit on the surface. This matches the wanaka signature
exactly (short arcs bridging a small knoll; `entry_load` peak
3.90 mm at `arc_tolerance` 0.05).

Amended fix design: no probe needed. The arc must be faithful to its
OWN SOURCE polyline — the generator already made the source
surface-true. Run the per-point Z check in the constant-Z branch too:
every point's z must sit within `tolerance` of the value GRBL will
interpolate (constant `z_start` for a planar arc). This rejects the
knoll run and changes nothing for genuinely planar or helical runs.

Bars:

- **S3-red (instrument validity):** a synthetic run of on-circle XY
  points with a z bump > 5 × tolerance mid-run and level endpoints is
  ACCEPTED by `try_fit_arc` before the fix.
- **S3-green (gate):** after the fix the same run is rejected (stays
  linear), and a genuinely planar arc run plus a genuine helix run
  still fit (parity — the fix must not kill legitimate arcs).

### S3 results (2026-09-03)

The firing shape is NARROWER than first registered, and the fixture
had to follow it (recorded honestly):

- A WIDE smooth bump does not reproduce the defect. The greedy
  extension fails the helix check before any level-endpoint window
  forms, and the sub-arcs it accepts are z-faithful (measured: 3
  faithful helical arcs pre-fix and post-fix — arc COUNT is the
  wrong assertion).
- A NARROW knoll (one spiked point, level neighbours) reproduces it
  exactly: a small greedy window spans the knoll with level
  endpoints, and the pre-fix code accepted it with no interior
  check. **S3-red measured against the pre-fix fitter (stash run):
  the output passes at z = 0.000 under the 0.5 mm knoll point.**
  This matches the wanaka wording — SHORT buried chords through a
  small knoll.
- Fix: the per-point z-vs-interpolation check runs unconditionally
  in `try_fit_arc` (no probe needed — the arc is held faithful to
  its own source polyline, which the generator made surface-true).
- **S3-green passes**: the knoll point survives the refit; the
  planar and helix parity arms still fit arcs.

The wanaka re-measure (does the `entry_load` CRITICAL stay gone with
`arc_fitting = true` under this fix?) belongs to the lane that owns
the running GUI, alongside the S2/ramp re-measure.

### Post-commit verification probes (2026-09-03, throwaway test, not committed)

Two claims were verified by measurement instead of inference after
`8451e87c`:

1. **Arcfit vs clipped entries.** On the ridge fixture, after full
   dressups: the clipped RAMP legs stay linear (66 linear entry
   moves, 0 arcs — near-collinear XY defeats the circle fit). The
   clipped HELIX turns DO refit into arcs (5 arc moves inside the
   entry intents). The sentry samples arcs along the arc, so its
   green covers them — and the unconditional S3 Z check is what
   holds those refit arcs faithful. The order dependency (arc fit
   runs AFTER entry) is therefore audited, not assumed.
2. **The golden's mechanism.** On a hemisphere + waterline + default
   ramp dressups reconstruction: `EntryRamp = 0`,
   `EntryPlunge = 20` — every entry took the plunge FALLBACK (legs
   poke past the mesh edge), exactly as the re-baseline note says.
   The alternative mechanism (silent clipping changing the
   kinematics class) did not occur.

Also noted: the full core suite's nine other arc-fitting test
binaries stayed green through the S3 change with no re-pinning —
consistent with the hole firing only on narrow knolls.

### Coverage gap, ledgered

The adaptive3d door's clip has NO dedicated test. The pre-existing
`test_helix_entry_no_vertical_plunge` proves its `Unconstrained`
off-mesh policy keeps edge helixes alive; nothing yet proves the
ON-mesh clip fires through that door (its default entry style is
`Plunge`, so no shipped default is exposed). Follow-on: an adaptive3d
ramp-entry-over-ridge arm in the sentry file.

### S2 results (2026-09-03)

- **S2-red is a permanent in-test arm**, not a one-off number: the
  sentry runs the probe-less path on the same fixture and asserts a
  lead sample buried > 0.5 mm, so the instrument's ability to see
  the class is re-proven on every run. (First fixture orientation
  put the lead circle on the downhill flank and read no burial —
  corrected to sweep uphill before any code conclusion was drawn.)
- **S2-green passes**: the probed lead lifts to the surface, still
  carries `LeadIn` moves, and nothing buries > 0.2 mm.

### The golden re-baseline the fix forced (2026-09-03)

`perf_golden_sim_metrics`'s 3D arm went red: the Waterline fixture's
`per_kinematics` lost its `Helix` class (600 samples). Those samples
WERE the terrain-blind ramp legs — Waterline's Finish-role default
carries `entry_style = ramp`, and on the hemisphere fixture the legs
poke past the mesh edge, so the finish door now degrades those
entries to plunges. Re-baselined with `UPDATE_PERF_GOLDENS=1` and
this rationale. The shift also measures a side benefit: the blind
legs were fed air — the Waterline op's `air_cut_time_s` fell from
54.8 s to 15.4 s, and the project air-cut (total-runtime denominator)
from 51.1 % to 39.6 %, on the test dome alone.

## Follow-ons (ledgered, not blocking)
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

### S1-green — the clip holds the bar (2026-09-03, post-fix)

The fix follows the pre-registered design exactly: `emit_ramp` clips
its legs through `clip_polyline_to_floor` (0.5 mm samples, lift-only,
lost contact → plunge fallback); `emit_helix` lifts each 10° turn
sample the same way. Both `#[ignore]` attributes removed. Result:

- Ramp arm: green at 0.2 mm tolerance, and the entry still carries
  `EntryRamp` moves (no silent degrade to plunge).
- Helix arm: green, still carries `EntryHelix`.
- No-intrusion parity arm: green — exactly two legacy legs.

One fixture correction during the green run, recorded honestly: the
parity arm's first fixture put the entry at x = −20, whose legs reach
x = −39 — off the ±30 mesh — so the lost-contact fallback fired and
the arm counted zero `EntryRamp` moves. That run is an accidental
proof of the fallback arm. The fixture moved to x = −8 (legs reach
−27, on the mesh); the parity assertion then passes as pre-registered.

The decision rule ("if the clipped entries still bury > 0.2 mm, stop
and report") was not needed.

### Design amendments after the first full-suite run (2026-09-03)

The core suite (2 482 lib tests) went red on ONE test:
`adaptive3d::tests::test_helix_entry_no_vertical_plunge` — a helix on
a small flat mesh whose circle pokes past the mesh edge. The
pre-registered rule "lost contact at any sample → plunge fallback"
degraded that legitimate roughing helix to a plunge. The failure also
exposed a second, worse latent case by inspection: the session gate
handed the probe to EVERY operation in a mesh-carrying project, so a
2.5D pocket ramp (which legitimately cuts BELOW the mesh top) would
have been silently lifted to the surface and destroyed.

Two amendments, recorded before the code changed:

1. **Probe issuance is per operation type, not per project.** Only
   surface-riding operations receive the probe from the dressup door:
   the mesh finish family plus DropCutter/UnifiedFinish (harmless —
   their dressups are stripped). For these ops "never below
   `CL + stock_to_leave`" is their own contract. Prism operations
   (Pocket, Profile, Adaptive, Face, Rest, Inlay, Zigzag, VCarve,
   Trace, Chamfer, drills, ProjectCurve) get NO probe: their entries
   may descend below the model surface by design, and the 2D-prism
   blind-leg behaviour stays as audited (A0 finding 4).
   `OperationConfig::entry_probe_stock_to_leave` becomes
   `entry_probe_leave() -> Option<f64>`.
2. **Off-mesh policy is per door.** The dressup door (finish family)
   keeps the plunge fallback: beyond the part footprint full-height
   uncut stock can stand, and the plunge at the entry column is the
   only descent known safe. The adaptive3d door sets
   `OffMeshEntry::Unconstrained`: beyond the mesh footprint stands
   prism stock that a 2.5D rough is allowed to cut, its destination
   is already draped, and its `descent_floor` guard covers uncut
   columns — the planned leg z stands there.

With both amendments the failing adaptive3d test is expected green
again with its original meaning intact, and the S1 sentry semantics
do not change.
