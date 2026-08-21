# wanaka200 from-scratch run — log (2026-08-19)

Executed the plan in `PLAN.md`: authored a new project over MCP against the
regenerated rivmap export, in white oak, on the Shapeoko Pro XXL profile.

Operator rulings taken at session start (§5 of the plan):

1. Holes → **6 mm pilot drills** at circle centroids (13 holes).
2. Board thickness → **25 mm** dressed.
3. Rivers tool → **20° V-bit** (hairline).
4. Lakes depth → **0.3 mm**.

## Outcome

**The toolpaths are geometrically CORRECT. The simulator's picture of them is
not.** 8/8 toolpaths generated, zero collisions, all eight tool-load gates
`Within`, and the export gate refused correctly until the unmodeled criteria
were acknowledged.

The rendered stock showed the centre of the part cut clean through. That was
**a display defect, not a machining defect** — `G-SIM-IDENTITY-FRAME` below.
Measured off the emitted G-code, the back rough leaves **4.000 mm** exactly as
commanded, with zero gouges.

**Retracted:** the first version of this log claimed a part-destroying engine
bug, `G-WANAKA-FLIP`, in which `stock_to_leave_axial` supposedly floored ~5.6 mm
too deep on flipped setups. **That claim was false.** It is retracted in full
below, with the reasoning error kept on the record.

Three real defects stand, all found in this run and all silent:

1. `G-SIM-IDENTITY-FRAME` — simulated/playback stock mis-registers identity
   setups by the stock origin. Part-destroying *if trusted*; nothing gates on it.
2. `G-WANAKA-DRILL-RAMP` — auto dressups put a ramp entry on both drill cycles.
3. `G-WANAKA-PECK` — auto Suggest set `peck_depth` above the hole depth.

Plus a shop-floor hazard in the exported program (`G-EXPORT-DATUM`) and a
wasted-air finding (`G-SAFEZ-LOCAL`), both below.

## Artifacts

| file | what |
|---|---|
| `wanaka200.toml` | authored project (final state) |
| `wanaka200_1_Setup_1.nc` | **Setup 1 G-code — post-fix, datum verified. X 0.000..237.500, Y 0.000..247.500.** |
| `wanaka200_2_Setup_2___front.nc` | **Setup 2 G-code — post-fix, datum verified. X 0.000..234.281, Y 0.000..236.639** (was −11.955..215.123 pre-fix). |
| `final_stock_fixed_dropcutter.png` | final render matching the exported program |
| `fixed_final_stock.png` | **after the fix — the truthful picture. Part intact, centred, shell + holes + rivers in register.** |
| `final_stock.png` | pre-fix, kept as evidence — **shows a false through-cut** (G-SIM-IDENTITY-FRAME) |
| `chk1_after_back_rough.png` | after back rough — correct terrain-following cavity |
| `chk4_end_setup1.png` | end of Setup 1 — shell, 13 pilots, rivers + lakes pierced |
| `chk5_front_rough.png` | first identity-setup op — where the false through-cut appears |

The three PNGs of Setup 1 (a non-identity setup) are correctly framed and can be
trusted. Anything showing Setup 2 content is displaced by the stock origin until
the sim fix lands and the run is re-simulated.

## G-SIM-IDENTITY-FRAME — simulated stock mis-registers identity setups by the stock origin

**Severity: part-destroying if trusted. Silent — no gate reads the affected
object, so every gate said `Within` while the picture showed a ruined part.**

> **FIXED and VERIFIED 2026-08-19.** Re-generated and re-simulated on the fixed
> binary: `fixed_final_stock.png` shows an intact part — solid centre, full
> terrain relief, symmetric border on all four sides, Setup 1's shell and all 13
> pilot holes in register with the front for the first time. Every toolpath's
> cutting distance is byte-identical to the pre-fix run, confirming the change
> touched only the simulation frame and not the cut. Sentry
> `crates/rs_cam_core/tests/sim_identity_setup_playback_frame.rs` (4 tests) goes
> red-then-green; `rs_cam_core` / `viz` / `cli` / `mcp` tests, clippy and fmt all
> clean.

`run_simulation` builds its global / playback stock **zero-rooted**, from a bbox
`(0,0,0)..(stock_dx, stock_dy, stock_dz)`, because `local_to_global` returns
stock-relative coordinates (`compute/simulate.rs:547-558`;
`compute/transform.rs:204-223` states it deliberately does not re-add the
origin). Non-identity setup groups are mapped into that frame correctly via
`transform_toolpath`. **Identity setup groups are stamped in verbatim, still in
world frame**, with no `-stock_bbox.min` translation
(`compute/simulate.rs:730-735`, and the same hole in the composite-mesh append
at `:838-839`).

So whenever `StockConfig::origin != 0` and the project contains an identity
setup, that setup's cuts land in the playback stock displaced by exactly the
stock origin. Here that is **(20, 25, 18) mm**.

The Z component is what produced the through-cut. Setup 2's toolpath spans
world Z +7 down to −1.4 and is stamped into a grid whose stock occupies
Z 0..25, so a `FromTop` cutter at relative Z ≈ 0.5 clears the **entire 25 mm
dexel ray** — the interior is removed to full depth. The XY component leaves
survivors only in `x ∈ (220, 240]` and `y ∈ (225, 250]`: measured off
`final_stock.png`, the surviving terrain band is **≈20 mm on one edge and
≈25 mm on the other** — `origin_x` and `origin_y` exactly. That is the L-shaped
remnant, and it is also the answer to "is the cut centred?" — it is; the
*render* is displaced, by different amounts on the two axes because the origin
is.

### Why it survived: a second bug in the GUI cancelled the first

`app/gpu_upload.rs`'s `transform_mesh_to_local_frame` subtracted `stock.origin`
on its identity arm. Against a *world*-framed checkpoint mesh that shift was
correct — but against the *mis-carved* zero-rooted live stock it happened to
undo the mis-registration. **Two errors cancelling is why the GUI viewport
looked right for years while `screenshot_simulation`, which has no such
compensation, showed the ruined part.** Fixing only the core would have
*regressed* the viewport, so the compensating shift had to come out in the same
change.

The identity hole also had a **third home**: `worker/execute/mod.rs`'s
`build_playback_data` carried a private copy of the frame mapping for live
scrub. The fix routes all three through one shared core helper
(`group_point_to_global` / `group_toolpath_to_global` /
`group_drill_op_to_global`), so they cannot drift apart again. Identity-group
**drill ops** were mis-registered too — a site the original diagnosis missed.

The `all_identity` / `world_shift` special case is **retired** rather than
extended: once every group lands in one frame, `compute_deviations` no longer
needs a frame-map parameter and the "mixed projects documented as unresolved"
case falls out uniformly.

The code already knows about this class of bug. The deviation pass carries both
the fix and the scar tissue (`simulate.rs:1021-1032, 1054-1063`): *"Identity
groups … must frame-map the query by `-stock_min` or every deviation
mis-registers by the stock origin — the first scaled-wanaka cascade A/B read a
uniform ~−4 mm 'overcut' through precisely this hole."* And `simulate.rs:900-912`
documents mixed identity/non-identity projects as unresolved. This project
(Setup 1 `bottom` + Setup 2 `top`, non-zero origin) is the first mixed one with a
non-zero origin to reach the stock renderer.

Why every gate passed: per-toolpath metrics, engagement and collision checks all
read `group_stock` (`simulate.rs:584-588, 625, 709`), which is correctly framed
per setup. Only the display/playback stock is wrong — and it is the one thing an
operator actually looks at.

**Corroboration that the G-code is fine.** Rasterising `terrain.stl` to a 1 mm
grid and taking the min over each 6 mm tool disc, then mapping the Setup 1
G-code back through the flip: median measured shell above terrain **4.000 mm**
against a commanded 4.0, **zero** points with negative shell. Setup 2's minimum
measured leave is **0.500 mm** against a commanded 0.5, with **zero** front
points cutting below the back-rough floor. The shell is ≥ 4.5 mm everywhere
after both roughs.

## RETRACTED — `G-WANAKA-FLIP` was not a defect

My first diagnosis this session claimed `stock_to_leave_axial` floored the back
rough ~5.6 mm too deep on flipped setups, destroying the part. **It is wrong and
is withdrawn.** Recording the error because the reasoning failure is the useful
part:

- I read the **nominal Z ladder** out of `narrate_toolpath` (20.8, 16.6, 12.4,
  8.2) and treated it as achieved cut depth. It is not. On a surface-following
  rough the per-cell floor is `(surf_z + stock_to_leave).max(z_level)` with a
  gouge-guard drape (`adaptive3d/search.rs:57-63,117-122,148-153`;
  `adaptive3d/path.rs:1142-1175`), so the deep rungs are almost entirely
  drape-limited and the ladder is not a depth report. It also silently omits
  levels whose region comes out empty.
- The `4.0 → 8.2` / `8.0 → 12.4` experiment I ran as "proof" showed a step of
  **4.2**, which is `depth_per_pass`, not the 4.0 I had changed. That was
  ladder quantisation, and I misread it as a constant error term.
- The "5.6 ≈ 2 × 2.814" signature was **numerology, and it does not even hold**:
  the gap is 13.814 − 8.2 = **5.614** against 2 × 2.8139 = **5.628**. I let a
  near-coincidence supply a mechanism.
- The real tell was available and I ignored it: the emitted G-code. Measuring
  the actual motion against the mesh settles the question in one pass and says
  4.000 mm.

**Answer to the operator's question** ("before I used an offset to stop the full
cut through — are you saying this isn't working?"): No. Nothing about that
approach is broken, and `stock_to_leave_axial = 4.0` with `bottom_z = auto`
produced a correct terrain-following 4 mm shell. Note also that the July
reference's pinned heights were **vacuous no-ops**: on a `face_up = bottom`
setup `world_to_local` is `local_z = 7 − world_z`, so `ModelBottom` resolves to
local 0.0 and `max(4.0, 0.0) = 4.0` — byte-identical to `auto`; and adaptive3d
ignores a pinned `top_z` outright for roughing (`compute/execute.rs:1471-1479`).
Beware the naming trap: on a flipped setup `ModelTop`/`ModelBottom` mean *local*
top/bottom, the reverse of what you see in world.

If you ever want a **flat** backing plane instead of a following shell, the
correct authoring is `bottom_z = from_reference{ ModelTop, +4.0 }` → local
13.814. That is a different part, not a fix.

## G-EXPORT-DATUM — the two setups' G-code use different XY zeros, and only Z is flagged

**Severity: high, shop-floor. This one is in the program that runs the machine.**

> **FIXED and VERIFIED 2026-08-19** (uncommitted). All setups now emit
> **stock-relative XY** — program zero is the stock min corner in every file,
> which is the frame `alignment_pins` are already dimensioned in and the frame
> the setup sheet's default `XYDatum::CornerProbe(FrontLeft)` already tells the
> operator to zero at. Pre-fix, the identity setup's G-code did not match its own
> datum sheet. Sentries `crates/rs_cam_core/tests/export_datum_setup_frame.rs`
> (4 tests) and a viz-path test in `wizard_e2e.rs` go red-then-green; the red run
> measured the delta as **exactly 20.000 = −origin_x**. Core / viz / mcp / cli
> tests, clippy and fmt all clean.
>
> **Z is deliberately NOT shifted.** XY is never re-zeroed between setups so a
> disagreement there is silent and fatal; Z *is* explicitly re-zeroed, so a
> per-file Z datum is an instruction problem, fixed by naming the datum. Shifting
> Z would also move program Z0 to the stock underside for every identity setup,
> breaking the documented 2D convention where `origin_z` is set so the stock top
> sits at Z0 — every 2D job would go from "zero to the top of the stock"
> (self-correcting for actual board thickness) to "zero to the spoilboard"
> (carrying the nominal-vs-actual thickness error into every depth).
>
> The header now names the datum on every split file, e.g.
> `(DATUM: X0 Y0 = stock min corner (SAME in every setup file); Z0 = 7.000mm BELOW top of stock)`
> followed by `FLIP PART BEFORE RUNNING -- re-zero Z to the datum above; KEEP the
> same X/Y zero`. That last clause is only truthful *because* of the fix.
>
> **Blast radius — read before running anything older.** Emitted G-code changes
> for **every project whose stock origin is non-zero in XY**, which is not an edge
> case: `auto_from_model` defaults to true and sets `origin = bbox.min − padding`
> (default 5 mm), so a typical auto-stock project shifts by +5 mm. Single-setup
> identity projects included — the rule is uniform by design, because a frame that
> depends on setup count is the same bug class. Unchanged: any project with
> `origin_x == origin_y == 0` (byte-identical, pinned by a sentry), and every
> non-identity setup everywhere (shift is exactly zero). Three consequences:
> the GUI viewport still shows world frame so it no longer coincides with the
> G-code when the origin is non-zero; `pre_gcode`/`post_gcode` are raw user text
> and are NOT shifted, so a hand-written `G0 X0 Y0` there now means the stock
> corner; and the combined single-file multi-setup export (one `G54`, no chance to
> re-zero mid-program) was silently wrong before and is now correct by
> construction.

| | XY zero | Z zero |
|---|---|---|
| Setup 1 (`face_up = bottom`, non-identity) | **stock min corner** | original top face |
| Setup 2 (`face_up = top`, identity) | **world origin = model corner** | world Z 0, i.e. 7 mm below stock top |

Evidence: the Setup 1 pin drill sits at `X2.500 Y2.500` / `X237.500 Y247.500` —
the stock's own alignment pins — and its back rough spans X 22.75..216.25 (model
at 20..220). Setup 2's front rough spans X 2.82..200.25 (model at 0..200). Both
files emit a plain `G54` with no `G10`/`G92`, and the Setup 2 header says only
`FLIP PART + RE-ZERO Z BEFORE RUNNING`.

**If the operator keeps the same XY zero across the flip, the front side
machines 20 mm / 25 mm off** — reproducing on the real part exactly the picture
the simulator drew for the wrong reason. Either the header must call for an XY
re-zero and name the datum, or the exporter should emit both setups on a common
datum.

## G-PINDRILL-FRAME — the alignment-pin drill mixes two frames in one operation

> **HALF FIXED 2026-08-21.** The `cfg.holes` half is fixed and sentried by
> `crates/rs_cam_core/tests/pindrill_emission_frame_g_pindrill.rs`. The
> `selected_holes` half is measured and split out as **G-DRILLPICK-FRAME**
> below, because it needs a decision about stored frame and it also affects
> the plain `Drill` family.

`generate_alignment_pin_drill` consumed `cfg.holes` — **stock-relative** pin
coordinates snapshotted from `StockConfig::alignment_pins` — verbatim as XY,
while the toolpath emits in the **setup** frame: world for an identity setup,
zero-rooted local otherwise. In a non-identity setup the two agree and the
result was accidentally correct. In an identity setup with a non-zero stock
origin the pins were drilled in the wrong physical place.

**Measured on the wanaka geometry** (240×250 stock at origin (−20,−25), pin at
stock-relative 2.5/2.5):

| stage | XY | should be |
|---|---|---|
| generated toolpath | `2.5, 2.5` | `-17.5, -22.5` (world) |
| after export datum shift `(+20,+25)` | `22.5, 27.5` | `2.5, 2.5` |

**Error is exactly the stock origin.** Registration pins 20/25 mm out is the
whole part 20/25 mm out on the flip — this is the feature whose entire job is
to make the two sides agree.

**The fix needs no new plumbing.** `ctx.stock_bbox` IS the stock expressed in
the emission frame, so its min corner is where stock-relative (0,0) sits. One
translation serves both cases: world for identity, and a provable no-op for
non-identity (`min == (0,0)`) — which is *why* non-identity was accidentally
correct, now pinned by the sentry's second arm so the fix cannot "correct" the
case that was already right. Verified red-first: without the fix the sentry
reports `no move at the pin's world position (-17.5, -22.5)`.

**wanaka200 itself was not affected** — its pin drill lives in Setup 1, which is
non-identity. That is also why the export sentry deliberately does not assert on
pin holes. Core suite 3140 green, zero blast radius.

## G-DRILLPICK-FRAME — picked drill targets are never setup-transformed (OPEN)

Split out of G-PINDRILL-FRAME 2026-08-21. **Measured, not fixed — it needs a
decision that changes what existing project files mean.**

`selected_holes` are raw model/DXF coordinates: the viz picker maps
`LoadedModel::drill_targets` straight through
(`ui/properties/operations/drill.rs:47`). But model polygons **are**
setup-transformed before generation (`session/compute.rs:1090`), and both drill
families then consume `selected_holes` verbatim —
`drill_holes_for_config` returns `selected.clone()` for `Drill`, and
`generate_alignment_pin_drill` appends them unchanged.

So on a non-identity setup the picks are off by the whole setup transform.
Measured on a Bottom flip of 240×250 stock: a target picked at world `(30, 40)`
transforms to setup-local `(50, 185)`, and the op drills it at `(30, 40)`.

**Scope is wider than the pin drill** — this hits the ordinary `Drill` family
too, which is the common case for DXF hole drilling.

**The decision.** Either picks stay stored in world/model frame and are
transformed into the emission frame at generation (a code-only fix, but it
must not double-apply for identity setups), or they are stored stock-relative
like `alignment_pins` (consistent with the export datum's chosen frame, but it
changes the meaning of `selected_holes` in every saved project on load, so it
needs a format migration). Not an agent's call.

## G-PROFILE-FLIP — a `Bottom` flip turns an outside Profile into an inside one

Also found incidentally. A first draft of the export sentry used a Profile and
measured identity X `17..53` against flipped `23..47` — inset by exactly the tool
radius. `apply_to_polygons` mirrors the polygon, which reverses its winding, and
the offset direction follows the winding. The sentry was switched to a Trace
(offset-free, winding-invariant) so it tests one thing. **The Profile behaviour is
a separate, unverified finding** — it means a profile cut on a flipped setup may
be offset to the wrong side. Worth its own investigation before any two-sided job
relies on a profile.

## G-SAFEZ-LOCAL — the retract plane is floored in the wrong frame

> **FIXED 2026-08-21.** One line in `session/eval_context.rs:100`: the floor now
> reads `heights_stock_bbox.max.z` (the emission frame) instead of
> `local_stock_bbox.max.z`. Sentried by
> `crates/rs_cam_core/tests/safe_z_emission_frame_g_safez_local.rs` (3 arms).
> The wanaka200 rapid saving has NOT been re-measured live — that needs the GUI.

`effective_safe_z` floored safe_z at `local_stock_bbox.max.z + clearance` using
the **local zero-rooted** stock top, while identity setups emit in **world** Z.
On wanaka200 (`stock.z = 25`, `origin_z = -20`, `post.safe_z = 10`) that is
`max(10, 25 + 5)` = **30** against a world stock top of **+5** — the correct
value is `max(10, 5 + 5)` = **10**.

**The original write-up understated this in one direction and overstated it in
the other. Both corrections matter more than the row did.**

**1. It was not only waste — it was a rapid through material.** The local top is
the stock *thickness*; the world top is `origin_z + thickness`. So for
`origin_z > SAFE_Z_CLEARANCE_MM` the floor lands **inside** the stock. Measured
by the red sentry: 12 mm stock at `origin_z = +20` has a world top of 32 and was
floored at **17** — a retract plane 15 mm below the surface. `SetupEvalContext`'s
own doc called this "a conservatively-higher floor (never below the world stock
top for identity setups) … always safe". That claim is false above
`origin_z = 5`, and the doc has been rewritten to say so.

**2. It does NOT explain the five air levels, and that half of the claim is
withdrawn.** `HeightsConfig::resolve` (`compute/config.rs:1382-1393`) takes
`top_z` from `ctx.stock_top_z`, not `ctx.safe_z`; only `retract_z`, `feed_z` and
`clearance_z` derive from safe_z. So this fix moves the retract plane (30 → 10),
the feed plane (28 → 8) and the clearance plane (40 → 20) and provably leaves the
depth ladder where it was. The `25.8 / 21.6 / 17.4 / 13.2 / 9.0` ladder and
toolpath 5's 42,667 mm of rapid are therefore **not** attributed here. Those
numbers came from `narrate_toolpath`, whose Z ladder CLAUDE.md flags as
**nominal, not achieved** — the same reading error that produced the retracted
G-WANAKA-FLIP. Re-opened as its own row (**G-AIRLADDER**, below) to be measured
off emitted motion before anything is claimed about it.

**Why this is a leftover rather than a design choice.** The heights/setup-frame
audit of 2026-06-12 (finding 4) already moved `OpContext::stock_bbox` onto
`heights_stock_bbox` for exactly this reason — `session/compute.rs` sets
`emission_stock_bbox = ctx.heights_stock_bbox` and its comment says why.
`safe_z` was not moved with it, so a single `HeightContext` anchored its depth
ladder in the emission frame and floored its retract plane in the local one. The
export path is not a second chance at this: `0bb38a2f` pins
`identity.z.abs() < 1e-9`, so the datum shift is XY-only and the generated
world-frame Z is what ships.

**Blast radius: one test, fully attributed.**
`crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`.
Its fixture is 9 mm stock at `origin_z = -9.0`, world top Z0, and its own
`post.safe_z = 10` already cleared it — the local floor had been overriding that
with `9 + 5 = 14`. Both arms were probed for their distinct-Z multiset before and
after: **identical except the single retract plane, 14.000 → 10.000 mm**, move
counts unchanged (1464 taper, 972 ball). Pins refreshed with that recorded. The
in-loop `assert_eq!` was also converted to accumulate-then-assert, because the
test's own comment records that aborting on the taper arm had left the ball pin
unevaluated for an unknown number of commits.

Non-identity setups cannot move: `heights_stock_bbox == local_stock_bbox` for
them, pinned by the sentry's third arm. The simulator's dexel grid keeps its
local rooting through `SetupEvalContext::sim_local_stock_bbox`, so the actual
F-024 concern is untouched.

## G-AIRLADDER — RESOLVED: it was G-SAFEZ-LOCAL after all

> **My withdrawal of this claim was itself wrong, and is retracted 2026-08-21.**

When G-SAFEZ-LOCAL was fixed I split this out as a separate row, on the grounds
that `HeightsConfig::resolve` takes `top_z` from `ctx.stock_top_z` and only
`retract_z` / `feed_z` / `clearance_z` derive from `safe_z` — so the fix
"provably cannot move the depth ladder". That reasoning is correct about the
**depth** ladder and irrelevant, because the observed rungs were never depth
levels.

**They are the peck-plunge ENTRY ladder.** `adaptive3d/path.rs:1314` calls
`emit_peck_plunge(&mut tp, entry, params.safe_z, params)` — rooted at the
retract plane and stepping down by `depth_per_pass`. `30 − 4.2 = 25.8`, then
21.6, 17.4, 13.2, 9.0. The Z-level plan (`path.rs:448`) starts at
`stock_top_z − depth_per_pass` = 2.8 and never approaches 25.8. So safe_z
drives this ladder after all, just not through the field I checked.

**Measured from the shipped G-code**, not from narration — parsing
`wanaka200_2_Setup_2___front.nc`, toolpath "6 3D Rough (front)", against the
world stock top the file's own datum header names:

| | value |
|---|---|
| distinct **fed** Z above stock top | **5** — 25.800 / 21.600 / 17.400 / 13.200 / 9.000 |
| fed moves at each | 231 → 1,155 total |
| **lateral cutting moves at those levels** | **0** — pure vertical descent |
| fed path length above stock top | **14,591 mm** (≈28 min at F541) |
| rapid in the section | 42,735 mm (the reported 42,667) |

**So the saving G-SAFEZ-LOCAL's commit message declined to claim is real.** On
a wanaka-shaped fixture the fix takes this from five wasted rungs to one, and
from ~14,591 mm of fed air to ~27 mm. `889b1573` was too conservative about
its own effect.

**Why the error happened, since it is the reusable part.** I verified the
mechanism I went looking for (`top_z`) and stopped, rather than asking what
else could put motion at 25.8 — and I reached for code reading when the
repo's own rule ("measure emitted motion, not the plan") prescribes parsing
the G-code, which settles it in one pass. The original reading was NOT a
nominal-vs-achieved narration error; the motion was real.

**Residual, open and deliberately not fixed.** `emit_peck_plunge` is rooted at
`safe_z` and knows nothing about the stock top, so while `SAFE_Z_CLEARANCE_MM`
(5.0) exceeds `depth_per_pass` the first rung still lands above the stock —
10.0 − 4.2 = 5.8 against a top of 5.0. Rooting the peck at the stock top would
take it to zero, but that changes adaptive3d entry motion and moves
fingerprints. Sentried at the residual (`≤ 1 level`, `≤ 40 mm`) by
`crates/rs_cam_core/tests/air_ladder_emitted_z_levels_g_airladder.rs`, so the
tree stays green but a *growth* in wasted air fails immediately.

## Renderer defects the frame bug was sitting behind

Worth fixing on their own, because this project's working rule is "never gate on
an aggregate without rendering the surface" — which requires the render to be
trustworthy.

1. **The 6-view composite drops `StockConfig::origin`** (the defect above). For a
   mixed-setup project the two setups' surfaces are drawn in different frames.
2. **The two middle orthographic panels are mirror-inconsistent.** From
   `fingerprint.rs:542,545`: "Top" is az 0 / el +90 → `sy = −y` so world **+Y
   points down**; "Bottom" is az 0 / el −90 → `sy = +y` so world **+Y points up**.
   Neither flips X. A physically correct top/bottom pair differs by an **X** flip,
   not a Y flip, so the two panels cannot be compared edge-for-edge.
3. **The "vertical striping" on rim walls is aliasing, not standing material.**
   `dexel_stock_to_mesh` emits one wall quad per 0.4 mm dexel column (~600 across
   the stock) and the isometric panels render at ~0.93 mm/px with no
   anti-aliasing. Measured pitch is 2.91 mm in the 1400 px render and 1.62 mm in
   the 1600 px one — it tracks the pixel grid, not the geometry, and at 2.00 px it
   is exactly the Nyquist limit. A real pillar would read as residual in the
   orthographic top view; none does.
4. **No panel labels are drawn**, and the internal view names are themselves
   misleading ("Front-Left" has its eye at +X,+Y).
5. **Each panel auto-fits its own mesh extents** (`fingerprint.rs:630-657`) rather
   than a common stock-bbox camera, so scales are not comparable between renders.

## Rapids are NOT cutting material — checked directly

The operator suspected rapids were gouging. Parsing both `.nc` files for any
`G0` with lateral travel below the stock top:

| Toolpath | lateral rapids | below stock top |
|---|---|---|
| 6 3D Rough (front) | 472 | **0** |
| 7 3D Finish (R1.5) | 1120 | **0** |
| 8 Pencil detail | 3474 | **0** |
| Setup 1, all five ops | 2890 | **0** |

The only combined XY+Z rapids are 529 retracts, every one ending at Z30.000 and
clear of the surface within ~0.1 mm of lateral travel. Note this verdict rests on
the **G-code**, not on `rapid_collision_count` — those counters are vacuous for
Setup 2 for the reason above, since the collision test ran against a misplaced
volume.

## G-WANAKA-DRILL-RAMP — auto dressups gave both drill cycles a ramp entry

**Severity: high (registration-destroying). Found by reading the emitted G-code,
not by any diagnostic.**

`add_toolpath` auto-applied `entry_style = "ramp"` to the alignment-pin drill
and the hole drill. The emitted pin-drill motion was:

```
G0 X15.708 Y16.271 Z30.000
G1 X15.708 Y16.271 Z28.000 F3000
G1 X2.500 Y2.500  Z27.000        <- 19 mm lateral move while descending
```

A ramped alignment-pin hole is an oval slot, which destroys flip registration —
the entire purpose of the op. The July reference used `entry_style = "none"`.
Fixed in-session via `set_dressup_field`; pin-drill cutting distance fell
1094.5 mm → 394.0 mm and move count 128 → 66, and the emitted motion became
clean 3 mm pecks straight down at X2.5 Y2.5.

No gate, warning, or advisory flagged this. A ramp entry on a drill cycle is
never correct and should be refused at the source.

## G-WANAKA-PECK — Suggest set `peck_depth` greater than the hole depth

Auto-applied feeds set `peck_depth = 15.0` on **both** drill ops (schema
default is 3.0). On the holes op, `depth = 12.0` — so the peck depth exceeds
the hole depth and, per the `peck_depth` doc's own warning about degenerate
values, the "peck" cycle silently becomes a single full-depth descent. In
white oak with a 6 mm end mill that is a chip-evacuation problem.

The peck-adequacy gate reported `Within` (observed 0.5 vs threshold 5.0)
because it divides by the envelope radius — the known R-12 blind spot. A gate
reading `Within` is not evidence here. Set to 3.0 manually.

## MCP surface gaps — things the API could not express

These forced a save → hand-edit TOML → reload cycle, twice. For an
"author from scratch over MCP" workflow this is the single biggest friction.

1. **`set_stock_config` sets only x/y/z.** No origin, no material, no
   workholding rigidity. Material is load-bearing — every feed in this run
   depends on White Oak vs the reference's birch ply — and origin decides the
   Z frame the whole job cuts in.
2. **`set_stock_config` does not clear `auto_from_model`.** After setting
   240×250×25 explicitly, `auto_from_model` stayed `true` and the origin was
   silently derived from the *last imported model's* bbox — `holes.dxf`,
   giving origin (2.23, 109.81, −14.81). Setting dimensions explicitly should
   imply "stop auto-fitting", or the call should say it didn't.
3. **No machine-profile write path that carries kinematics.** The library's
   `shapeoko_pro_xxl` has `acceleration_xyz_mm_s2: null` and a flat 350,
   where the real machine (per the reference, with $$ baked in) is 500/500/270
   with junction deviation 0.02. Accel decides parallel-vs-spiral strategy, so
   this is not cosmetic.
4. **`add_tool` takes only name/type/diameter**, and the defaults it fills in
   are type-agnostic and wrong: every tool got `corner_radius 2.0`,
   `taper_half_angle 15°`, `included_angle 90°`, `cutting_length 25`, and
   `tool_number 1`. A 20° V-bit created as `included_angle 90` is silently a
   different tool. Six tools needed ~30 corrections. Note also that identical
   `tool_number` on every tool would break `m6` tool-change export, which
   requires distinct numbers to re-trigger.
5. **`set_tool_param` accepts `included_angle` and `taper_half_angle`** but
   its description doesn't list them. Doc gap, not a code gap.
6. **Array-valued params cannot cross the wire.** `set_toolpath_param`'s
   `value` has no declared type, so an array argument arrives server-side as a
   string: `invalid type: string "[[2.5,2.5],[237.5,247.5]]", expected a
   sequence`. The pin-drill `holes` array had to be hand-written into the TOML.
7. **`add_toolpath`'s description omits `unified_finish` and
   `alignment_pin_drill`.** Both are valid — `parse_operation_type`'s own
   error message lists them. An agent reading only the tool description would
   conclude the newest finishing operation isn't reachable over MCP.
8. **No `z_rotation` setter.** Not needed this run (see below) but there is no
   way to express it.
9. **Alignment-pin drill does not inherit the stock's alignment pins.** Both
   pins existed on the stock; the op still came up with `holes = []` and a
   blocking diagnostic reading "Add alignment pins to the stock before adding
   this op" — advice that was already satisfied. The message should say the op
   needs its own `holes` populated *from* the stock pins.

## Things that worked well

- **The rest-machining chain caught a real modelling error.** Marking the front
  rough `from_remaining_stock` produced a precise refusal: it is the first op
  in its setup, so there is no prior stock — rest does not cross setup
  boundaries. The message named the problem and both fixes. Set to `fresh`.
- **`generate_all` fixpoint** resolved the whole 8-op chain in 3 rounds /
  2 simulations, unattended, first try.
- **The measurability machinery abstained honestly.** At a 0.4 mm cell,
  `radial_engagement` / `air_cut` / `chip_engagement` came back
  `not_measurable` for toolpaths 5, 6 and 7 (blind fraction 0.71–0.80,
  `cell_too_coarse_for_tip_contact`) and `degraded` for toolpath 1. The
  project verdict is still the blunt "WARNING: high air cutting (>20%)", which
  is computed from the very metrics that abstained — read the measurability
  block, not the verdict.
- **The export gate refused** until `accept_unmodeled_tool_load` was set,
  naming toolpaths 3 and 7 (V-bit rivers, R0.5 pencil) and the reason
  (`chipload=NoVendorData`). Their deflection and power gates *were* measured
  on real populations (peak 0.016 mm and 0.013 mm), so only chipload abstained.
- **Drill gates had real populations** (2 and 13 holes contributing) — not the
  vacuous empty-population case.
- **DPP clamp landed exactly where the plan predicted**: Suggest clamped
  depth_per_pass to 4.2 on both roughs, the G-WANAKA-DPP baseline.

## Feeds observed on white oak (for reference)

| op | tool | feed | RPM | notes |
|---|---|---|---|---|
| Pin Drill | 6 mm EM | 2400 | 8000 | clamped into material envelope from 4500 |
| Back Rough | 6 mm EM | 750 | 15000 | DPP 4.2, stepover 1.2 |
| Holes | 6 mm EM | 2400 | 8000 | |
| Rivers | 20° V-bit | 1200 | 24000 | no vendor band at all |
| Lakes | R1.0 TB | 1062 | 19000 | |
| Front Rough | 6 mm EM | 750 | 15000 | DPP 4.2, stepover 1.2 |
| Unified Finish | R1.5 TB | 913 | 18500 | see burnishing note |
| Pencil | R0.5 TB | 1851 | 24000 | sub-Ø2, chipload unmodeled |

**White oak burnishes the R1.5 finish.** The whole derated band sits below the
0.025 mm/tooth chip-formation floor, so the clamp reported
`0.0173 → 0.0247 mm/tooth` against the *band ceiling* rather than the floor —
no feed clears rubbing without exceeding the band. This is the hardwood delta
from the reference's birch ply, and it is a real finish-quality warning, not a
gate failure. Lever: a larger finishing tool.

## Deviations from the plan

- **Setup 2 `z_rotation` left at 0**, where the plan said 90 (copying the July
  reference). A 90° rotation does not map a 240×250 rectangle onto itself, and
  the diagonal pin pair at (2.5, 2.5) / (237.5, 247.5) is a 180° in-plane
  mapping for this stock. Keeping 0 also keeps Setup 2 on the identity frame
  path. Flagged for the operator — if the board is physically rotated on the
  bed, this needs revisiting (and there is no MCP setter for it).
- **Front rough is `fresh`, not `from_remaining_stock`** — forced by the
  setup-boundary rule above. Consequence: Setup 2's ops plan against a full
  block rather than the Setup 1 cavity. Measured after the fact, this cost
  nothing: the front rough's minimum leave above terrain is 0.500 mm as
  commanded, and no front point cuts below the back-rough floor.

## Finish-pass comparison: `unified_finish` vs plain `drop_cutter` (3D Finish)

Operator asked to re-test with a plain 3D finish instead of unified. `drop_cutter`
is the op literally labelled **"3D Finish"**, and it is what the July reference
used, so this also reverts the plan's one finishing deviation. Same R1.5 tapered
ball, same 0.3 stepover, `min_z = -20.0` and `from_remaining_stock` as the
reference. Both variants generated 8/8 with **zero collisions** and produced an
intact, correctly-registered part.

| | `unified_finish` | `drop_cutter` (3D Finish) |
|---|---|---|
| moves | 233,638 | **442,913** (1.9×) |
| cutting distance | 94,164 mm | **148,045 mm** (1.57×) |
| rapid distance | 45,013 mm | 73,749 mm |
| commanded feed | 913 mm/min @ 18,500 | 1,260 mm/min @ 19,000 |
| chipload at apply | clamped to band **ceiling**, burnishing expected | **no clamp at all — see G-SUGGEST-NOCLAMP** |
| project runtime | 50,269 s | 52,897 s (+5.2%) |
| project air-cut (total-runtime) | 57.0% | **52.1%** |
| pencil cutting distance after it | 45,751 mm | **38,561 mm** (−16%) |
| pencil tip-float points after it | 16,538 | **12,631** |
| finish blind fraction @ 0.4 mm cell | 0.797 | 0.931 (worse) |
| `unmachined_band_area_mm2` | 2,681.5 | not measured (op plans no bands) |
| collisions | 0 | 0 |

### Why drop_cutter is slower despite the higher commanded feed

**`drop_cutter`'s higher feed is largely fictional.** From the post-sim
`modulation_summary` and `feed_explanation`:

| | `unified_finish` | `drop_cutter` |
|---|---|---|
| commanded feed | 913 mm/min | 1,260 mm/min |
| **achieved / commanded (median)** | **0.9912** | **0.6137** |
| median feed delta | −0.88% | **−38.63%** |
| moves bound by `kinematic_reach` | 88.3% | **97.0%** |
| moves bound by `machine_max_feed` | 11.7% | 3.0% |
| mean move length | 0.403 mm | **0.334 mm** |

`kinematic_reach` means the move is too short for the machine to accelerate to
the commanded feed before it has to decelerate again. At 0.334 mm average move
length against 500/500/270 mm/s² per-axis accel, **97% of drop_cutter's moves
never reach the number on the label**. Effective feed is roughly 1,260 × 0.614 ≈
**773 mm/min**, against unified's 913 × 0.991 ≈ **905 mm/min**.

So drop_cutter is slower per millimetre *and* travels 1.57× further. The higher
commanded feed buys almost nothing, because this regime is **acceleration-bound,
not feed-bound** — the same lesson as the earlier spiral-vs-parallel work, where
machine accel, not path syntax, decided the winner. Runtime here tracks move
COUNT far more than commanded feed.

- **It leaves less for the pencil** (−16% cutting distance, ~4,000 fewer tip-float
  points), i.e. better coverage into the valleys before the detail pass. That
  partly offsets the finish op's own cost, which is why the project total only
  moves +5.2% while the finish op itself is much slower.
- **It loses two report channels.** `unmachined_band_area_mm2` (2,681.5 mm² on
  unified — a real gap, ~6.7% of the 200×200 face) has no meaning for
  `drop_cutter`, which plans no finish bands, so that measurement simply stops
  existing rather than reading zero. Same for `truncated_core_mm2`. Do not read
  the absence as an improvement.
- **Its engagement is even less measurable at 0.4 mm** (93% blind vs 80%). A
  finer verification sim would be needed to say anything quantitative about
  engagement on either variant.

### G-SUGGEST-NOCLAMP — drop_cutter's commanded feed is 1.63× the chipload band, and the gate passes it only because the machine can't reach it

**Severity: high. The `Within` verdict is contingent on a machine limitation, not
on the feed being correct.** Measured, not inferred:

- Commanded feed-per-tooth is **0.0332 mm/tooth** (narration states it explicitly;
  1,260 ÷ (19,000 × 2)).
- The post-sim gate's band for this op is **0.010174–0.020349 mm/tooth**
  (`vendor_lut`, row `amana-tapered-hardwood-parallel-3175-2f`, diameter_scale
  0.9854, hardness_scale 1.0326).
- Commanded ÷ band max = **1.63×**.
- The gate reads **achieved** advance-per-tooth, not commanded. Achieved median
  ratio is 0.6137, so observed lands at **0.020349 — exactly the band maximum** —
  and the verdict is `Within`.
- `get_suggest_rationale` for this op lists only a stepover raise for runtime
  sanity and a stock_to_leave note. **There is no chipload entry at all.**

By contrast `unified_finish`, same tool and material, *was* clamped at apply time
("Chipload clamped to the matched band ceiling: 0.0173 → 0.0247"), and its
commanded 0.024676 sits just 0.9% over its band max of 0.024459.

**CONFIRMED FROM THE EMITTED G-CODE 2026-08-19.** The exported Setup 2 program
contains **`F1260` on 559 finish lines**, e.g. `G1 X199.700 Y28.300 Z3.804 F1260`.
The full distinct-F set for the file is `{1260, 1851, 750, 541, 270, 68}` —
**every value is a commanded parameter and there are no modulated intermediates**.
So adaptive feed modulation is *not* rewriting the emitted feeds; the
`achieved ÷ commanded = 0.61` figure is the simulator's **prediction** of what the
machine will manage, not something written into the program.

The file therefore genuinely commands 1.63× the band maximum, and the only thing
keeping the tool inside the band is that a Shapeoko cannot accelerate to
1,260 mm/min across 0.33 mm moves. **On a stiffer/faster machine, or after any
change that lengthens moves (coarser stepover, arc fitting, segment merging,
better controller look-ahead), the same file cuts nearer 1.63× the band max.** The
margin is a property of the path geometry and machine dynamics, not of the feed
being correct.

Two questions fall out, neither answered:
1. Should the gate flag *commanded* as well as *achieved*? Achieved is the right
   tool-damage predictor for this machine; commanded is the right portability and
   feed-sanity signal. Nothing currently reports the second.
2. If adaptive feed modulation (default ON per J-3) reports large per-move feed
   deltas but does not change the emitted `F` words, what is it modulating?
   That would also explain the unstable `moves_touched` below. **Unverified.** Two things worth separating:
Suggest not clamping here at all, and the two ops matching **different vendor
rows** (`...-parallel-...` for drop_cutter vs `...-scallop-...`, `row_pass_role
semi_finish`, for unified) which gives them different bands from the same tool
and material.

### ROOT-CAUSED 2026-08-19 — the chip-thinning term is computed at a stepover the op does not run

**The 1.61× is not an unclamped number; it is a deliberately multiplied one,
multiplied by the wrong amount.** Running Suggest over this project:

```
feed 1259.841 @ 19000 rpm, 2F  → commanded 0.033154 mm/tooth
target chip_load = 0.015415  (LUT band MIDPOINT, inside band)
derates: radial_chip_thinning 2.2942 × axial 1.6667 = 3.8236
         × ld_overhang 0.75 × safety 0.75
ae used by the calculator = 0.0900
ae the toolpath actually runs = 0.30375
```

`0.015415 × 3.8236 × 0.75 × 0.75 = 0.033155`. The commanded advance is the band
midpoint times a chip-thinning factor derived from a stepover **3.4× smaller than
the one the operation cuts at**.

**Falsification test** — hypothesis: *if the calculator saw the real stepover, its
existing clamp would contain the feed.* Re-ran `feeds::calculate` on identical
inputs with only `radial_width_mm` overridden:

| `ae` fed in | RCTF | feed | mm/tooth | vs band max | warning |
|---|---|---|---|---|---|
| **0.09** (what it uses) | 2.2942 | **1259.84** | 0.033154 | **1.613×** | *(none)* |
| 0.30 (authored) | 1.3416 | **781.02** | 0.020553 | **1.000×** | `ChiploadClampedToFloor { band_capped_from: Some(0.025) }` |
| 0.30375 (applied) | 1.3350 | **781.02** | 0.020553 | **1.000×** | same |

At the true stepover the op lands **exactly on its band maximum** and emits **the
same warning `unified_finish` already gets**. The hypothesis survived a test
designed to kill it.

**Two independent gaps produce the stale stepover:**

1. `calculate` never sees the authored stepover. `FeedsHints::radial_width_mm` is
   documented "none of the current ops set this" (`compute/catalog.rs:2225`) and
   the exhaustive `feeds_hints()` match confirms it. `ae` therefore comes from
   `operation_default_profile(Parallel, Finish).ae_factor = 0.03` × Ø3.0 = 0.09.
   The project's `stepover = 0.3` is ignored.
2. `enforce_invariants` then mutates the stepover 0.09 → 0.30375
   (`feeds/suggest.rs:1909`) **after** `calculate` froze the feed. There is a
   direct precedent eleven lines earlier: when pass 0 mutates DPP,
   `suggest.rs:1785-1793` explicitly re-derives `chipload_bounds`. **Retired pass 8
   (Checkpoint J-1) was the only stage that reconciled the feed with the final
   geometry, and nothing replaced it.**

**It is generic, not specific to this op.** Mirror-image on toolpath 5: pass 0
clamps DPP 9.0 → 4.2 mm, but the feed still carries `depth_tier = 0.75` computed
at ap = 9.0, so that op is **under**-fed by 1.33× against its own model. The
commanded feed is derived at *pre-clamp* geometry throughout.

**Why `unified_finish` looked safe.** It was **not** clamped down — it was clamped
*up* to the ceiling by the rubbing-floor rule (`band_capped_from: Some(0.025)`).
Its `ae` is equally wrong, just conservatively so: `scallop_height = Some(0.1)`
gives `scallop_stepover(1.5, 0.1) = 1.0770`, ae/D = 0.598 ≥ 0.5, so RCTF = 1.0 and
its commanded landed at 0.70× band max. Both ops actually cut at ~0.3 mm. It
landed safe by accident of a large `ae`, not by a guardrail.

**There is no ceiling clamp anywhere in the Suggest path.** `calculate` has a
power step, a machine cap, a chipload **floor** (Step 9b) and a drill envelope;
`enforce_invariants` has eight passes, none chipload-related since pass 8 retired.
`chipload_bounds.max_mm_per_tooth` is never compared against the commanded feed on
the recipe side.

**Vendor-row divergence is correct by design, not a mapping gap.** `lut_query_for`
is a no-op for both ops (it only reroutes Adaptive3d/ProjectCurve); the rows differ
because the ops declare different families — `drop_cutter` → `Parallel/Finish`,
`unified_finish` → `Scallop/Finish`. `row_pass_role: semi_finish` on the unified
side is honest fallback: the LUT publishes no scallop/finish row for tapered ball
in hardwood. One wart worth a ledger row: `unified_finish` is a **three-band** op
that resolves a single scallop row for all three bands.

**Modulation is a write path, but a no-op here — verified.** `adaptive_feed_modulate`
does write per-move feeds and swap the IR (10/10 existing sentries green, including
one asserting ≥2 distinct emitted F words). But the swap is skipped when
`changed == 0`, and modulated feeds live only in the post-sim IR — any later
generate discards them, and `generate_all`'s fixpoint is literally
generate → simulate → generate. **Falsifier run on the live session:
`run_simulation` → `export_gcode` with no generate between produced a
byte-identical file with the same six commanded F values.** So on this project
modulation changes nothing, and the 1.63× reaches the machine unreduced. The
1.63× is a **Suggest** defect, not a modulation one.

`moves_touched` is not inconsistent after all — it counts changes against *that
move's current feed* (per-invocation) while `median_feed_delta_pct` measures
against the *authored* feed (cumulative). A second pass over an already-modulated
IR gives 0 touched with an unchanged delta. Idempotence, not contradiction — but
two quantities under one heading, and neither answers "is this program modulated?"

### FIXED 2026-08-19 — Suggest pass 9, `rescale_feed_to_final_geometry`

The primary fix below landed. `enforce_invariants` now ends with a pass that
holds the **implied target chipload** fixed — commanded advance per tooth
divided by the geometry terms — and re-multiplies by the chip-thinning ×
depth-tier factor at the operation's final `ae`/`ap`. It works in advance per
tooth rather than feed units so it stays exact when the operation carries a
rounded copy of the calculator's RPM.

It short-circuits in two cases, both load-bearing:

- **No calculator operating point in the context.** `SuggestContext` gained
  `calculator_operating_point`, populated only by `apply_feeds_subset`. A feed a
  human or the optimizer typed was never derived from a chip-thinning term, so
  there is nothing to reconcile — this preserves `resolve_operation_invariants`'
  documented contract ("the safety clamps, not a substitute number").
- **No pass moved the stepover or the DPP.** The entry values are the *rounded*
  ones `apply_feeds_subset` wrote, so re-deriving unconditionally would silently
  un-round every feed in the product for no physical reason. This guard is why
  the blast radius came in at 3 tests rather than hundreds.

**Measured effect.** Every op the pass fires on was previously commanding
**below its vendor band minimum** and now sits inside the band:

| fixture | tier | feed | advance before | after | band (target) |
|---|---|---|---|---|---|
| wanaka Back Rough Ø6 | 0.75 → 1.00 | 750 → 1000 | 0.02500 | 0.03333 | 0.032–0.055 |
| A3D-1 Ø6 HardMaple | 0.75 → 1.00 | 918 → 1223.4 | 0.03060 | 0.04078 | 0.032–0.055 (0.0435) |
| A3D-2 Ø8 WhiteOak | 0.75 → 1.00 | 1694 → 2258.4 | 0.03764 | 0.05019 | 0.0394–0.0677 (0.0535) |

All three are exactly ×4/3 — the same 1×D depth-tier boundary crossing produced
by the axial-envelope DPP clamp. The two DropCutter fixtures in the same table
did **not** move: surface-following ops command no axial step, so their
depth-tier term is identical at both operating points and cancels. That
asymmetry is the check that the pass keys on geometry and not on op family.

**The finish case did not land where the rescale alone put it, and this is
worth knowing.** On the sentry's `3D Finish 6` the stepover back-off
(0.03 → 0.2278) cancels a 3.8236 → 1.7171 chip-thinning lift, and the rescale
produced 293.2 mm/min — advance 0.0077, comfortably mid-band. The existing
Step-9b rubbing-floor rule then lifted it 38% to 404.8. Reason:
`band_capped_from: Some(0.025)` — this row's **entire** derated band sits under
the 0.025 mm/tooth chip-formation floor, so `effective_rubbing_floor` collapses
to the band *maximum*, and the clamp then fires against anything below the band
max, pinning the op to its ceiling. That is the ruled 2026-08-06 behaviour
(FEEDS_CENSUS C-12 / T3.3), not something pass 9 invented — but pass 9 routes
more ops into it, so it is now doing visible work on a shipped surface. Net for
that op: 1.613× over the band max → exactly on it.

**What the sentry actually proved.** Because the floor clamp fired on the finish
case, that arm took the sentry's *licensed exception* branch (advance must sit on
the reported floor) rather than the implied-target-agreement branch. The
mechanism assertion was genuinely exercised only on Back Rough. Do not cite the
finish arm as evidence for the mechanism.

**Deliberately beyond the brief, both documented at the code:**

- `FeedRescaledToFinalGeometry` gained `cap_hit: Option<FeedRecalibrationCap>`
  and the pass enforces the machine cutting-feed ceiling. An up-rescale is
  bounded only by the geometry terms (worst case ~8.9×); emitting a feed the
  machine cannot run would trade one wrong number for another. Same cap
  vocabulary retired pass 8 used for the same ceiling. Neither fixture hits it.
- Pass 1 (`clamp_plunge_to_feed`) re-runs after the rescale. A *downward*
  re-derivation can leave the plunge rate above the feed it was clamped to.

**Not re-checked, stated rather than hidden:** the **power ceiling** (calculator
Step 6). Required power scales with feed *and* with cross-section, and
cross-section moves with `ae`/`ap`, so a rescale can in principle invalidate a
power-limited feed. `power_ceiling_parity_f2.rs` measured that branch never
firing at all across three shipped presets × ten species × Ø3/Ø6/Ø12 — peak
utilisation 23.6 %, rigidity and the machine cutting ceiling bind first — and
the machine ceiling *is* enforced. On a profile where power does bind, pass 9
can over-feed. That wants its own instrument, not an unmeasured clamp bolted on
here. **G-SUGGEST-POWERSTALE**, open.

The **deflection budget** is also not re-verified. Retired pass 8 did verify it
and documented the verify as a no-op: the closed-form predictor is
feed-independent (force = `Kc × axial_doc × radial_woc`). It would be a no-op
here too, and `backoff_dpp_for_deflection` has already settled DPP.

**One defect the fix surfaced in passing.** `with_explored_speeds` drops
`chipload_bounds` deliberately, so that a feed-rewriting pass short-circuits and
a drag-to-explore feed is not silently re-solved. Pass 9 keyed off a different
signal and re-solved a hand-dialled feed —
`explored_speeds_survive_the_funnel_but_still_get_clamped` caught it. Fixed by
adding an explicit `ApplicableRecommendation::speeds_explored` flag rather than
reusing the absent band as the signal: a legitimate calculator result on an
**RPM-only vendor row** publishes no band either, and that feed *does* want
reconciling. Inferring operator intent from a missing band would have silently
disabled the fix for every RPM-only row.

### The original recommendation (for the record)

**Primary:** add a pass at the end of `enforce_invariants`
(`feeds/suggest.rs:1820`, where retired pass 8 sat) that re-runs the chip-thinning
and depth-tier terms against the operation's **final** stepover and DPP, rescales
the feed, and re-applies the Step-9b floor clamp. Direct precedent eleven lines
above. Expected on tp 8: **1260 → 781 mm/min**, commanded lands on the band
maximum, and the same disclosure `unified_finish` already gets appears in
`get_suggest_rationale`.

**Do NOT** instead populate `FeedsHints::radial_width_mm`. That slot is empty
deliberately — `apply_feeds_subset` writes `result.radial_width_mm` back as the
operation's stepover, so feeding the current stepover in would make the calculator
echo its input and stop recommending a stepover at all. The bug is ordering, not
the hint.

**Secondary backstop:** mirror Step 9b with a commanded-side *ceiling* clamp in
`calculate` (`feeds/mod.rs:1735`) plus a new `ChiploadClampedToBandCeiling`
warning. Land it *after* the primary fix and measure the verdict-flip table first —
with the chip-thinning multiplication still in place it would bind on a large
fraction of small-tool finishing ops.

**Tertiary:** `tool_load/chipload.rs:666-682` already computes
`CommandedStage::feed_per_tooth_mm` and **no verdict arm reads it**. Add an
advisory (never a hard export block) when commanded exceeds the band, worded to
name the contingency: *"inside band only because the machine cannot reach the
commanded feed."*

### G-CHIPTHIN-HALFFIX — flagged, needs its own decision

The 2026-08-06 wave established from primary sources that every vendor chipload
column in the LUT is a **linear advance per tooth**, and **deleted** the gate-side
chip-thinning normalisation rather than inverting it.
`CHIPLOAD_LITERATURE_VERDICT.md` §2.3 records that no wood chart in the LUT
publishes a radial-engagement condition for its chipload column at all, and §V-5
warns that applying a thinning factor to such a column "double-thins".

**That correction was applied to the gate and to nothing else.** Two sites still
multiply the vendor band by chip thinning: `feeds/mod.rs:1492-1506` (Suggest,
factor ∈ [1.0, 4.0]) and `feed_modulation.rs:373` (the modulator). Measured across
this project: 1.25× (tp5), 1.67× (tp6), 3.82× (tp8), 4.00× (tp9 — **on the clamp
ceiling**). So Suggest can command up to 4× the vendor's published number by
design, while the gate judges against that number unmultiplied, and the two are
compared to each other on operator-facing surfaces.

**Fact:** the two sides differ by `combined_chip_thinning`. **Judgement (not a new
retrieval):** given the column is an advance per tooth with no published `ae`
condition, the multiplication is the surviving half of the category error the gate
wave deleted. This moves every feed in the product — it needs its own ledger row
and its own decision, not a bundle with G-SUGGEST-NOCLAMP.

#### MEASURED 2026-08-19 — `tests/chipload_thinning_magnitude_survey.rs`

**The per-op magnitudes above are STALE.** They were taken before pass 9
(`a1bb964b`), which re-derives the thinning term at the stepover the operation
actually runs; on the reference finish pass that alone cut the factor from
3.8236 to 1.7171. The survey below is the post-pass-9 measurement and is the one
a decision should be taken against.

3 420 feasible operating points, 24 op types × 5 tool types × 5 (Ø, flute) pairs
× 6 materials. **99.1 % have thinning active.** Multiplier where active: median
**1.809**, p75 3.571, p95 4.000 — and **636 points sit ON the 4.0 clamp
ceiling**, meaning the multiplier there is not the geometric value at all, it is
the cap. 1 134 points matched a vendor row that published a band.

Where the commanded advance lands against that band:

| | inside | over max | under min | median ÷ midpoint |
|---|---|---|---|---|
| as shipped today | 216 (19.0 %) | **427 (37.7 %)** | 491 (43.3 %) | 1.000 |
| thinning deleted, no floor re-applied | 96 (8.5 %) | 1 (0.1 %) | **1 037 (91.4 %)** | 0.563 |
| thinning deleted, Step-9b floor applied | 505 (44.5 %) | 1 (0.1 %) | 628 (55.4 %) | 0.735 |
| thinning deleted, seed at band max + floor | **639 (56.3 %)** | 12 (1.1 %) | 483 (42.6 %) | 0.831 |

**Three findings the original framing did not anticipate.**

1. **81 % of banded operating points already command outside the vendor band** —
   37.7 % over, 43.3 % under, only 19.0 % inside. The defect is not "Suggest
   over-feeds"; it is that the feed stack does not land in the vendor window in
   either direction. The median sits dead centre (1.000) while the distribution
   is far wider than the window, which is exactly the shape a median hides.
2. **A bare deletion makes it worse, not better.** Removing the multiplier with
   nothing else changed drops the median to 0.563 of the band midpoint and puts
   **91.4 %** under the vendor minimum — trading a two-sided error for a
   systematic under-feed, which in wood is the burnishing/heat direction. The
   multiplier is currently doing load-bearing work offsetting the derate stack,
   whatever its justification.
3. **Deletion plus the floor the engine already has is a clear improvement**, and
   deletion plus moving the seed to the band maximum is better still: in-band
   more than doubles (19.0 % → 44.5 % → 56.3 %) while over-band effectively
   vanishes (37.7 % → 0.1 % → 1.1 %). Over-feeding is the breakage direction and
   under-feeding is the finish/heat direction, so this is not a symmetric trade.

Over-feeding is also **bounded at about 2.3×**, not 4× — the worst 20 points run
2.06×–2.34× of band max, led by `RampFinish` on a Ø12 tapered ball in ipe
(2.34×) and `VCarve` with a 60° V-bit in pine (2.31×). The 4.0 figure is the
clamp ceiling on the *multiplier*, not on the resulting band excess.

Caveat carried in the instrument: 553 of the 1 134 banded points had a later
clamp bind (power, machine ceiling, or the rubbing floor), so for those the bare
counterfactual is a lower bound rather than an equality. The floored and
band-max columns model Step 9b explicitly for this reason.

**Not measured, and it matters:** the modulator's site
(`feed_modulation.rs:373`) uses a *different* formula — `target = band_max ÷
sqrt(woc)` — not `radial_chip_thinning_factor`. It was not swept here because it
only runs post-simulation. Whatever is decided for Suggest must be decided for
that site too, or the two halves of the product will disagree again.

#### RULED AND FIXED 2026-08-19 — delete, and let the existing floor catch it

Operator ruling after the survey: **delete the multiplication; do not bundle the
seed-target change with it.** Both sites are gone — `feeds/mod.rs` Step 5 and
`feed_modulation.rs`. The modulator's was the larger of the two: `1/sqrt(woc)`
is unbounded and had no equivalent of Suggest's `[1, 4]` clamp, so at its
engagement floor it reached **31.6×**, applied to the band *maximum*.

The three factors are still **computed and reported** — the geometric condition
is real and an operator should be able to see it — but renamed
`observed_*_chip_thinning`, excluded from `FeedsDerates::combined_factor()`, and
shown in the modal as "chip-thinning (observed, NOT applied)". Leaving a
non-derate inside a struct called `FeedsDerates` whose doc said "the effective
chipload is `target × every_multiplier_here`" is the naming trap this programme
keeps getting caught by, so the doc names the exception explicitly.

**Achieved vs predicted**, on the same 1 134 banded points:

| | in-band | materially over max | under min | median ÷ midpoint |
|---|---|---|---|---|
| predicted (delete + floor) | 44.5 % | 0.1 % | 55.4 % | 0.735 |
| **achieved** | **45.9 %** | **0.1 % (1 point)** | **54.1 %** | **0.750** |

The instrument called it. One wrinkle worth recording: the raw over-band count
reads 9.3 %, but **105 of those 106 points sit within 1 % of the band maximum**
— the rubbing floor clamps them exactly onto the ceiling and
`apply_feeds_subset` then rounds the feed to 1 mm/min, pushing them a hair over.
Only one point is materially over. The survey now separates the two, because
conflating them overstates the defect by two orders of magnitude.

**Blast radius: six tests, all explicable, none re-baselined without evidence.**

| test | what moved | justification |
|---|---|---|
| `arc_fit_disposition_a5` | all four pins: 1223.4→1000, 2258.4→1806.7, 1487→881, 1029→638 | **all four now command INSIDE their derated vendor band** — the two Adaptive3d low in the window, the two DropCutter pinned exactly on the band ceiling by the rubbing floor (their whole band is under 0.025) |
| `vendor_sidebyside_chipload` | the sub-Ø2 probe test **inverted** | it existed to pin the defect: probe D commanded **1.478× the band ceiling**. It now commands 0.034067 against a band of 0.034455–0.056390 — a 1.5 % undershoot in place of a 47.8 % overshoot. Assertion inverted, old magnitude kept in the doc |
| `suggest_feed_matches_final_geometry` | stepover arm **inverted** | its premise (a stale chip-thinning lift) no longer exists. Replaced by the contract the deletion created: **a stepover mutation must not move the feed at all**, which did not previously hold |
| `_litmatrix_rubbing_floor_clamp` | pre-clamp advance 0.0219816 → 0.0209692 | exactly ÷1.0483, this cell's own thinning factor. The cell's conclusion is unchanged — it was below the floor before and is further below now, so the clamp still fires |
| `tapered_width_model_parity_c3` | the pinned feed delta **collapsed to zero** | C3's entire feed effect was mediated by chip thinning; with it gone, effective diameter no longer reaches the feed. All four rows now sit on **1200.00 = the chip-formation floor itself** (0.025 × 24 000 × 2). C3's effective-diameter column is untouched and still asserted |
| `chipload_thinning_magnitude_survey` | promoted from instrument to **instrument + sentry** | now pins the mechanism: commanded advance must equal target × the APPLIED derates and nothing else |

Two of my own instrument bugs surfaced and are worth recording, because both
would have read as engine defects: the mechanism check compared the
**shipped** feed against the **calculator's** published derates, which flags
Suggest pass 9's legitimate depth-tier rescale as "an unexplained multiplier"
(measured exactly 4/3); and it included **drill** ops, whose feed Step 9c
replaces outright with a plunge envelope (65 % disagreement, a different code
path entirely). Both now excluded with the reason stated at the code.

**Still open after this:** the seed target (`SuggestAggressiveness::Default`
aims at the band midpoint; the survey measured band-max seeding at 56.3 %
in-band vs 45.9 %) — deliberately not bundled. And **54 % of banded points
still command below the vendor minimum**, which the deletion did not fix and was
never going to: that is the derate stack, not the multiplier.

### Minor: `modulation_summary.moves_touched` is not stable across reads

Toolpath 1 (back rough) was not regenerated between two `get_tool_load_report`
calls, yet `moves_touched` read **8,789 / 9,496** in the first and **0 / 9,496**
in the second, while `median_feed_delta_pct` stayed identical at 76.67 in both.
A zero touched-count alongside a non-zero delta is self-inconsistent. Report-only
(no gate consumes it), and possibly just an artefact of reading after a
cache-served generation rather than a fresh one — flagged, not verified.

Both project files are kept: `wanaka200.toml` is the drop_cutter version,
`wanaka200_unified_finish.toml` the unified one.

## Lessons from the false diagnosis

Kept because the process failure is more transferable than the bug.

1. **The nominal Z ladder is not a depth report.** `narrate_toolpath`'s levels
   are planned rungs; on a surface-following rough the achieved floor is
   drape-limited per cell, and empty levels are silently omitted. Do not infer
   cut depth from it — this deserves a caveat in CLAUDE.md beside the other
   narration notes.
2. **Measure the emitted motion, not the plan.** Rasterising the STL and
   measuring the G-code against it settled in one pass what three rounds of
   parameter archaeology got backwards. On any "did it cut too deep" question,
   go to the G-code first.
3. **A controlled experiment still needs its result read correctly.** Changing
   `stock_to_leave_axial` by 4.0 moved the floor by 4.2 — that is
   `depth_per_pass`, i.e. quantisation, not a response to my input. I reported
   it as confirmation.
4. **Near-coincidences are not mechanisms.** "5.6 ≈ 2 × 2.814" supplied a
   confident causal story for a number that, checked properly, is 5.614 vs
   5.628 — and would have been meaningless even if it matched.
5. **Render-before-verdict cuts both ways.** Rendering the surface is what
   caught that something was wrong, and it was right to stop on it. But the
   render is itself an instrument, and this one was the broken part. The
   existing rule needs a companion: when the render disagrees with every gate,
   suspect the render too, and go to a third source.

## Next steps

**DONE this session** (all uncommitted, working tree):

1. ~~G-SIM-IDENTITY-FRAME~~ — fixed + verified, part renders intact.
2. ~~Re-simulate and re-render~~ — done, `fixed_final_stock.png`.
3. ~~G-EXPORT-DATUM~~ — fixed + verified, all setups now stock-relative, datum
   named in the header.
4. ~~Drill ramp entry / peck depth~~ — corrected in this project (engine-side
   refusal still outstanding, below).
5. ~~drop_cutter vs unified_finish comparison~~ — done, and it surfaced
   G-SUGGEST-NOCLAMP.

**Outstanding, in rough priority order:**

1. ~~Emit corrected G-code~~ — **DONE.** Both files re-exported on the fixed
   binary, datum verified, pre-fix files deleted.
2. **G-SUGGEST-NOCLAMP** — drop_cutter's commanded feed is 1.63× the chipload
   band and passes only because the machine can't reach it. **Confirmed present in
   the emitted G-code (`F1260` × 559).** Not root-caused. Top priority.
3. ~~**G-PINDRILL-FRAME**~~ — **HALF FIXED 2026-08-21.** Stock-relative pins are
   now translated into the emission frame; exported pin XY was off by exactly
   the stock origin. The `selected_holes` half is **G-DRILLPICK-FRAME**:
   measured (world (30,40) drilled where local (50,185) was meant), affects the
   plain `Drill` family too, and needs a stored-frame decision.
4. **G-PROFILE-FLIP** — a Bottom flip appears to invert outside/inside profile
   offset. Unverified beyond one measurement.
5. ~~**G-SAFEZ-LOCAL**~~ — **FIXED 2026-08-21.** Floor now reads the emission
   frame. Was also a *safety* defect, not just waste: `origin_z > 5` put the
   retract plane inside the stock. **G-AIRLADDER is the same defect** — my
   split of it was wrong and is retracted: the rungs are the peck-plunge entry
   ladder, rooted at `safe_z`, and the fix removes ~14.5 m of fed air on one
   toolpath.
6. Engine-side refusals: ramp/helix entry on drill-family ops
   (G-WANAKA-DRILL-RAMP), and `peck_depth >= depth` (G-WANAKA-PECK).
7. Renderer: origin, panel mirror convention, anti-aliasing, labels.
8. MCP surface: stock material/origin/rigidity setters, typed `value` on
   `set_toolpath_param` so arrays survive, and honest `add_tool` defaults.
9. Commit decision on the two fixes; the viz half of the sim fix has had a
   partial line-by-line review only.
