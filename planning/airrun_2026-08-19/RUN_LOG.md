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

## Live validation session 2026-08-22 — first GUI run of the TD3 fixes

GUI launched on a release binary built at `6ca2c7f2` (all ten TD3 commits).
Project `wanaka.toml` loaded; corrected state saved to
`wanaka_keyed_2026-08-22.toml` — the original is never written.

### Confirmed live

- **G-SAFEZ-LOCAL.** Setup 2 (identity) emits its retract plane at **Z31.02**
  against a stock top of Z25. Post-fix the floor is `max(safe_z 10, world_top
  5 + 5) = 10` → Z30 in exported coordinates; PRE-fix it would have been
  `max(10, local_top 25 + 5) = 30` → **Z50**. Every one of 555 retracts is
  20 mm shorter, ~22 m of rapid removed from that setup, and fed motion above
  the stock top is **2.6 mm on one level with zero lateral** (the residual
  G-PECKROOT rung).
- **G-EXPORT-DATUM.** Both per-setup files carry an identical
  `X0 Y0 = stock min corner` datum line.
- **Pin holes** land at exactly the configured stock-relative XY. NOTE this
  exercises the NON-identity arm (the pin drill is in Setup 1, `face_up=Bottom`)
  — i.e. the case that was already correct, so it proves the fix's no-op arm
  held, not the identity case it repaired. This project cannot exercise that.

### Found in the project itself

- **Toolpath 6 was unrunnable**: `3D Rough 6` used remaining stock while being
  the first enabled op in its setup, so it could never receive any — blocking
  the whole Setup 2 chain (op 8 waited on it). Set to fresh stock; the chain
  then generated in 2 rounds.
- **Alignment pins could not have registered a flip** — see G-PINAUTO.

### `crosses_standing_material` — and why resolution changed the answer

At cell 0.25 the check fired on ONE toolpath. At **0.1** it fires on three:

| toolpath | tool | % samples over | peak bite |
|---|---|---|---|
| 3D Finish 6 | R0.5 tapered ball | 18.8% | 2.39 mm |
| Lakes | 20° V-bit | 19.9% | **8.10 mm** |
| Rivers | 20° V-bit | 12.3% | **7.42 mm** |

The two V-bit rows are new to the REPORT, not to the toolpaths — at 0.25 the
grid could not see them. Provisional reading: Rivers/Lakes are variable-depth
carves into essentially fresh stock, and the check's bar is 3x *that pass's own
median bite*, which any varying-depth carve trips by construction. The finish's
2.39 mm on a Ø1 tip is the row that matters. NOT concluded — see the method
caveat below.

**Root cause on the finish side**: `3D Rough 6` ran with
`mill_shallow_areas: false` and `detect_flat_areas: false` on a river/lake
relief that is mostly shallow. Its total cutting distance was **4,541 mm**
where a Ø6 tool at 2.2 stepover needs ~4,500 mm *per Z level* to cover
100x100 — i.e. roughly a third of one level. The rough was cutting the steep
incisions and skipping the shallow majority, leaving a Ø1-tip finish to meet
up to 3 mm of untouched material.

### METHOD CAVEAT — three variables changed at once

`mill_shallow_areas` false->true, finish Unified->drop_cutter, and resolution
0.25->0.1 were applied in ONE step. Runtime fell 10,782 s -> 7,304 s, air-cut
12.7% -> 7.9%, engagement 0.209 -> 0.309, finish blind fraction 45.6% -> 21.3%,
collisions 0 throughout. **Those gains are real and none of them is
attributed.** This violates the project's own "ISOLATE THE VARIABLE" rule and
any causal claim from this run should be treated as unproven.

### The pre-sim feed heuristic and the post-sim gate DISAGREE

`set_toolpath_param` returned, unprompted:

- toolpath 15 (tapered ball): `Feed 3000 mm/min is 6.3x recommendation (473) —
  tool breakage risk pre-sim`
- toolpaths 4 and 10 (Ø6 end mill): `Feed 4000 mm/min is 5.3x recommendation
  (750) — tool breakage risk`

After simulation those are **superseded** by `load.chipload.within`. So the
project's every major cutting feed is 5-6x the LUT recommendation, and the
post-sim gate reports it as within limits.

**Do not read that `within` as an exoneration.** The chipload gate compares
advance-per-tooth against a VENDOR BAND, and for this tapered ball a direct
probe of `feeds::calculate` returns **no band at any depth sampled** (the tool
is Ø1.0 tip, well inside the sub-Ø2 provisional regime). A gate with no band
has nothing to exceed. This is the fourth occurrence on this programme of a
gate reading healthy because its population is empty, and the first where it
does so while a heuristic on the same surface says "tool breakage risk".

Severity is also arguably mis-set: "tool breakage risk" ships as `severity:
hint`.

### Export still refused, correctly

`chipload=NoVendorData` on the two 20° V-bit ops. Not an exceedance — an
absence of data. Left unexported: accepting it is an operator decision, not
an agent's.

## G-TIMEEST — the operator-facing cycle time is 7x optimistic

> **CORRECTION, AND THEN ITS RETRACTION — 2026-08-22. Read the final position
> first: the diagnosis below is CORRECT. `total_runtime_s` IS accel-aware.**
>
> Mid-session I claimed the opposite, on this reasoning: `segment_time_s` is
> `(segment_len / params.feed_rate_mm_min) * 60.0`
> (`dexel_stock/simulation.rs:738`, `:1085`), the trace accumulates it
> (`simulation_cut.rs:1166`), and a grep for `runtime_by_intent:` found only
> `None`. I concluded the accel integrator was unwired.
>
> **Both facts were true and the conclusion was wrong**, because F-034 throws
> that total away. `compute/simulate.rs:1233` runs
> `apply_kinematics_cycle_time` whenever `request.kinematics` is `Some`, and it
> OVERWRITES the dexel sum with the integrated value:
>
> ```rust
> tp_summary.total_runtime_s = b.total_s;
> tp_summary.runtime_by_intent = Some(b);
> trace.summary.total_runtime_s = project_total;
> trace.summary.runtime_by_intent = Some(project_breakdown);
> ```
>
> `session/compute.rs:2899` re-runs it after feed modulation.
>
> **The method error, which is the reusable part**: I grepped for the field's
> INITIALISERS and never for its MUTATIONS. `runtime_by_intent: None` in
> fixtures and unrelated constructors is not evidence that nothing writes it.
> When asking "is this field populated", search for assignments to it, not
> struct literals containing it. This is the same shape as the G-AIRLADDER
> error earlier the same day — verifying the mechanism I went looking for and
> stopping.
>
> **The 7x is acceleration, and falls out of arithmetic with no fitting.**
> Finish pass mean segment 0.40 mm, commanded F3000 = 50 mm/s, wanaka's
> `acceleration_mm_s2 = 350.0`: reaching commanded feed needs `v²/2a` = 3.6 mm
> of runway against a 0.40 mm segment. Peak reachable is `√(a·L)` = 11.8 mm/s,
> so a from-rest triangular profile averages 5.9 mm/s — 8.5x under commanded —
> and junction deviation (0.01) lets corners carry a few mm/s, pulling the
> effective ratio to the ~7x measured.
>
> **Load-bearing caveat discovered in the process**: every shipped
> `MachineProfile` preset has `kinematics: None` (`machine.rs:178, 204, 226`).
> A project on a stock preset therefore gets the NAIVE runtime from the
> simulator too, with no signal saying so. Only a hand-authored or $$-imported
> kinematics block gets the integrated number. So "the sim is accel-aware" is
> conditional, and the condition is invisible on every surface.
>
> **Scope is larger than four surfaces**: there are SEVEN live copies of
> `cutting_distance / feed_rate()`. Beyond the four named below, also
> `ui/export_wizard.rs:937` (the final save step's "Estimated cycle time"),
> `io/setup_sheet.rs:88` and `:303` (the printed sheet the operator carries to
> the machine), and `ui/toolpath_panel.rs:480`.

## G-DRILLFLIP — CLOSED 2026-08-22 — analytic drill removal does not survive a flipped setup

Found 2026-08-22 by the OPERATOR watching the simulation: "I see the tool drill
the pins but it's not showing in the sim stock — the other holes do."

**Confirmed by render.** Checkpoint 0 (after Pin Drill) is a completely uncut
blank. Checkpoint 2 (after Holes) shows all twelve holes plainly. Both ops are
drills, in the SAME setup, sharing one removal path.

**Mechanism.** `TriDexelStock::apply_drill_op` (`dexel_stock/mod.rs:399`) ends
with, for a flat profile:

```rust
let z_cut = hole.bottom_z;
self.clear_above_at(row, col, z_cut);   // -> ray_subtract_above
```

It uses **only `bottom_z`**, and `clear_above_at` removes material UPWARD from
it. `top_z` never participates. That encodes "the hole was drilled from above" —
an assumption that is true in the setup-local frame and FALSE after
`group_drill_op_to_global` maps a `FaceUp::Bottom` group, because
`local_to_global` inverts Z (`z -> H - z`).

With H = 25 on the live project:

| op | local bottom_z | global bottom_z | `clear_above_at` result |
|---|---|---|---|
| Holes (`Drill`) | 25 - 12 = 13 | 25 - 13 = **12** | clears 12..25 — a hole appears |
| Pin Drill (`AlignmentPinDrill`) | 0 - 1 = **-1** | 25 - (-1) = **26** | clears 26..inf — ENTIRELY ABOVE a 25 mm stock, so nothing |

The pin drill's whole point is that it goes THROUGH the stock and 1 mm into the
spoilboard. That `-1` is what makes it vanish: inverted, it lands a millimetre
above the top of the blank.

**The regular Drill does not escape — it only looks like it does.** It clears
the COMPLEMENTARY band. A 12 mm hole into the up-facing face of a flipped setup
should occupy z 0..12 in the stock-relative frame; the code clears 12..25.
Holes appear, so the render looks plausible, and the material removed is the
wrong material. That is the more dangerous of the two, because the visible
symptom is absent.

**Not caused by G-PINDRILL-FRAME.** That fix corrected where the TOOLPATH puts
the holes (verified in the exported G-code). This is the SIMULATOR's analytic
removal, and it has been direction-blind for as long as it has existed; the pin
drill simply never had a `bottom_z` below the stock before anyone looked.

**Fix shape.** `apply_drill_op` needs the drilling DIRECTION, not just a floor.
Either carry it on `DrillOp` (it is derivable at build time — `top_z` vs
`bottom_z` ordering in the LOCAL frame, before any transform), or have
`group_drill_op_to_global` re-order the pair and flip a direction flag when the
transform's Z determinant is negative. Do NOT simply `min`/`max` the pair at
removal time: that would make the pin hole appear while still removing from the
wrong side on the regular drill, i.e. trade a visible bug for an invisible one.

**Sentry it against BOTH symptoms**: pin drill on a flipped setup removes
material at all, and regular drill on a flipped setup removes the band nearer
the machined face rather than its complement.

### CLOSED 2026-08-22

**Shape chosen: neither of the two the entry proposed.** The direction is not a
property of the operation — it is a property of the *frame the removal is being
applied in*, and one `DrillOp` is applied to two different frames (the per-setup
local stock and the global stock) within a single simulation. Putting a flag on
`DrillOp` would have made one struct carry a fact that is true of one of its two
consumers, which is how the field would eventually have been read in the wrong
place. Re-ordering the pair in `group_drill_op_to_global` was rejected for the
reason the entry already gives, plus one more: `top_z == bottom_z` is a legal
zero-depth hole, so ordering is a degenerate signal.

`StockCutDirection` already carries exactly this fact, already has
`cuts_from_high_side()` documented in precisely these words ("High-side entry
removes material via `subtract_above`; low-side entry via `subtract_below`"),
and was **already in scope at every call site** — including
`app/simulation.rs`, which destructures `direction` from `playback_data` and
was ignoring it on the drill branch. So `apply_drill_op` takes it as a
parameter. Three call sites, all of which knew the answer already.

Changed:

- `TriDexelStock::clear_below_at` — the mirror of `clear_above_at`. It leaves
  `conservative_top` alone, deliberately: subtracting from below cannot raise a
  ray's top, so the existing sliver-safe bound stays valid, and where the
  subtraction empties a ray outright the bound is left loose — which
  over-reports material and therefore costs work, never safety.
- `apply_drill_op(&drill_op, direction) -> DrillRemovalReport`. The cone sign
  follows the advance too: `bottom_z ± r/tan α`, because the cone opens *back
  toward the tool*.
- Lateral setups (`FaceUp::{Front,Back,Left,Right}`) **abstain** rather than
  carve. `DrillHole` is an XY centre plus two Z values and simply cannot express
  a hole whose axis is global X or Y; `group_drill_op_to_global` discards the
  mapped bottom point's XY, so the axis is gone before the kernel sees it.
  Filed as **G-DRILLLATERAL** below.
- `DrillRemovalReport` exists so a test can assert the footprint was actually
  walked. The whole reason this defect needed a person watching a viewport is
  that "removed nothing" and "removed the wrong thing" are both silent.

**Red-first, verified.** `tests/drill_flip_removal_g_drillflip.rs`, 5 tests.
With `from_high` forced back to `true` (the pre-fix constant), three fail with
exactly the reported symptoms — `a through-hole must clear the full 25 mm ray,
25 mm left` (the pin drill the operator could not see) and `0..12 is the hole in
the global frame and must be empty, found 12 mm` (the complementary band, the
one that looked fine). Restored and green.

**Both symptoms are asserted, and so is the complement.** The blind-hole test
asserts the correct band is empty *and* that the band the old kernel removed is
still standing — "material gone" and "material still there" fail in opposite
directions, and a one-sided version of this test would have passed before the
fix.

**Scope note — CORRECTED 2026-08-22, same day.** This entry first said the
global stock is what `StockSource::FromRemainingStock` reads, and therefore
that a rest pass planned against a flipped setup was planning against the
complement of the real material. **That is wrong.** Rest generation reads
`SimulationResult::prior_stocks`, and those are clones of the **per-setup
local** `group_stock` (`compute/simulate.rs:917`, consumed at
`session/compute.rs:1434`). The local stock's drill removal was always correct,
because setup-local Z is always the tool axis — the bug only ever existed in
the global frame.

So G-DRILLFLIP is a **checkpoint / playback / screenshot** defect: what the
operator sees, not what the next operation plans against. Still real, still
worth the five sentries, and still the reason the operator could not find their
pin holes — but it did not mis-plan anything.

**Where the wrong claim came from, because that is the reusable part.** A
comment on `TriDexelStock::playback_dispatch` (`dexel_stock/mod.rs:75`) had said
for a long time that the playback kernel "builds `global_stock`, which
`StockSource::FromRemainingStock` generation reads". I read it, believed it, and
repeated it into a commit message, this log, `CLAUDE.md` and a test header
without checking. That is precisely the failure mode
[[feedback_instrument_integrity]] names — a docstring that has become a lie you
then cite. Both the source comment and all four repetitions are now fixed.

## G-DRILLLATERAL — a drill on a side-face setup cannot be simulated (OPEN, filed 2026-08-22)

Split out of G-DRILLFLIP rather than folded into it, because the fix is a data
model change and not a sign.

`FaceUp::{Front,Back,Left,Right}` are legitimate on a 3-axis router — stand the
board on edge and machine what was its side — and milling handles them: the
tri-dexel lazily allocates an X or Y grid (`grid_for_direction`) and
`StockCutDirection::decompose` reorients the stamp. Drilling does not, and
cannot as currently typed:

- `DrillHole` is `{ xy: [f64; 2], top_z, bottom_z }`. It names an axis only
  while that axis *is* Z.
- `group_drill_op_to_global` maps the mouth and the tip through the transform
  and then keeps `g_top.xy` and `g_bot.z`. For a lateral transform those two
  points differ in X or Y, and that difference — the hole's actual axis — is
  discarded.

So `apply_drill_op` now reports `unrepresentable_axis` and removes nothing,
rather than carving a fabricated Z-axis hole from coordinates that no longer
mean what they are named. **Nothing else changes**: the toolpath is still
emitted, still exported, still collision-checked. It is the analytic stock
removal, and only that, which abstains.

**Fix shape**: give `DrillHole` two 3-D endpoints (mouth and tip) instead of an
XY pair plus two scalars, and let the kernel walk the footprint on
`grid_for_direction(direction)` with `decompose`. That is a `DrillHole`
signature change touching `drill_metrics`, the three drill gates and
`append_drill_cylinders`, which is why it is filed rather than done here.

**Priority: low, and stated why.** No shipped project in the repo uses a lateral
`FaceUp`, and the operator's two-sided gate is Top/Bottom. It becomes real the
first time someone drills an edge.

## G-DRILLTIME — CLOSED 2026-08-22 — drill ops are now inside the cycle-time model

Found 2026-08-22 in the live validation of the G-TIMEEST consolidation, by
reading the GUI rather than the code.

**Symptom.** After a clean generate + simulate on a project that CARRIES machine
kinematics, the readiness/timeline/diagnostics surfaces all read
`2:02:34 (cutting only, no accel)` — the WEAKEST basis — where `wall clock` was
expected.

**The number is right and the label is honest.** 2:02:34 = 7,354 s against the
simulator's `total_runtime_s` of 7,292 s; a true `distance / feed` total for
this project is about 26 min, so the figure is plainly trace-derived. The gap is
**61.66 s**, and it reconciles exactly:

| op | cutting mm | feed | naive time |
|---|---|---|---|
| Pin Drill | 74.0 | 300 | 14.8 s |
| Holes | 234.0 | 300 | 46.8 s |
| | | | **61.6 s** |

**Mechanism.** Drill toolpaths set `metrics_not_applicable` and produce
`drill_summaries` rather than an entry in `SimulationCutTrace::toolpath_summaries`
(confirmed: the run's `measurability` block lists toolpath ids 4, 5, 6, 10, 11 —
the two drill ids, 14 and 7, are absent). `apply_kinematics_cycle_time` iterates
`toolpath_summaries`, so a drill op is never integrated and never reaches the
project total. `readiness::toolpath_cycle_time` then finds no summary, correctly
falls back to `cutting_distance / feed` with basis `CuttingOnly`, and `worse()`
correctly degrades the whole project.

So every layer behaves as designed and the composite answer is unhelpful:
**0.8% of the runtime being unmodelled drags a 99.2%-modelled estimate to the
weakest label.**

**`drill_summaries` is not a drop-in substitute.** `DrillToolpathSummary` carries
`feed_time_s` and `dwell_time_s` and its own doc says it "excludes rapid …
runtime accounting" — a cutting-only quantity, so sourcing the basis from it
would claim modelling it does not have. That is the vacuous-substitute trap.

**The real fix is to integrate drill toolpaths.** `machine_kinematics::compute_cycle_time`
takes a `Toolpath` and a `MachineKinematics`; a drill cycle is ordinary linear
motion and nothing about it resists integration. The only reason it is excluded
is that the integrator is driven off the ENGAGEMENT summary list, which conflates
"has no engagement metrics" with "was not simulated". Those are different facts
and the code currently has one slot for both.

**Interim options, if the full fix is deferred**: report the DOMINANT basis with
the unmodelled fraction named ("wall clock — 0.8% of runtime not modelled, drill
cycles"), which is more useful than a bare weakest-basis label and does not
overclaim. Do NOT simply exempt drills from the fold — that would silently
restore the overclaim the row exists to prevent.

### CLOSED 2026-08-22 — the full fix, not the interim

The interim was not needed. The entry's own diagnosis pointed straight at the
fix: *"the integrator is driven off the ENGAGEMENT summary list, which conflates
'has no engagement metrics' with 'was not simulated'. Those are different facts
and the code currently has one slot for both."*

`apply_kinematics_cycle_time` was **already computing** a breakdown for every
toolpath in the request, drills included — it built `per_toolpath_runtime` over
`request.groups[].toolpaths` and then folded the project total over
`trace.toolpath_summaries`, which drills have no row in. The drill's answer was
computed and thrown away. So the change is small:

- **`SimulationCutTrace::toolpath_runtimes`** — a new list,
  `Vec<ToolpathKinematicRuntime>`, holding the integrator's answer for every
  toolpath it walked. This is the second fact given its own slot.
- **The project fold** now sums the integrated set, plus any engagement summary
  the integrator did not reach (which keeps its own naive runtime rather than
  being dropped). The two sets do not overlap by construction.
- **`readiness::toolpath_cycle_time`** asks "was this integrated?" *before* "does
  it have engagement metrics?", so a drill reads `MachineModel` instead of
  falling through to `cutting_distance / feed`.

**Neither shortcut was taken.** Drills are not exempted from the fold — that
would restore the overclaim the basis exists to prevent; their time is still
counted, just counted by the integrator. And `drill_summaries` was not used as a
substitute basis: `DrillToolpathSummary` says in its own doc that it "excludes
rapid … runtime accounting", so sourcing a `MachineModel` label from it would
claim modelling it does not have.

**Red-first, verified.** With the publication filtered back to
summary-bearing toolpaths, `the_integrator_publishes_a_runtime_for_the_drill_toolpath`
fails on exactly the original mechanism, and `the_project_total_includes_the_drill`
prints the arithmetic: **14.624 s total against 5.512 s of milling** — the
drill's ~9.1 s computed and discarded, in miniature.

Sentries, split by what they can honestly test:

- `rs_cam_core/tests/drill_cycle_time_integration_g_drilltime.rs` (4) — the
  **simulator** populates the slot for a real drill toolpath and the project
  total includes it. Opens with `the_fixture_really_is_the_defects_shape`,
  which asserts the drill has **no** engagement summary and **does** publish a
  `drill_summaries` row: without that, every later claim would be about a
  fixture that had quietly stopped being the defect.
- `rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` (+3, beside the G-TIMEEST
  sentries) — the **basis arithmetic**: a drill reads `MachineModel`, one drill
  no longer drags a modelled project to `CuttingOnly`, and the control
  (`without_integration_a_drill_still_degrades_the_basis`) confirms an
  un-integrated toolpath still weakens the claim, so the middle test is
  measuring the fix and not the absence of a fold.

**Known limit, recorded rather than left to be discovered.** The integral is
over stored motion, so a peck cycle's R-plane and re-entry moves are in it. G82
**dwell is not motion and is not in it** — it is reported separately as
`DrillToolpathSummary::dwell_time_s`, and no surface currently adds the two. On
the shipped `DrillCycle::Peck` default that is zero; on a `Dwell` cycle it is
not.

## G-CHIPGATE-POPULATION — RE-DIAGNOSED 2026-08-22 — the reported mechanism was wrong

Raised 2026-08-22. **Fourth occurrence on this programme of a gate reading
healthy because its population is empty, and the first where a heuristic on the
same surface says the opposite.**

On the live project, `set_toolpath_param` returned, unprompted:

- toolpath 15 (R0.5 tapered ball): `Feed 3000 mm/min is 6.3x recommendation
  (473) — tool breakage risk pre-sim`
- toolpaths 4 and 10 (Ø6 end mill): `Feed 4000 mm/min is 5.3x recommendation
  (750) — tool breakage risk`

After simulation each of those is **superseded** by `load.chipload.within`.

The `within` is not evidence. The chipload gate compares advance-per-tooth
against a VENDOR BAND, and a direct probe of `feeds::calculate` for that
tapered ball returns **no band at any sampled depth** — it is Ø1.0 at the tip,
inside the sub-Ø2 regime where the LUT has no row and the scaling laws are
repo-derived. A gate with no band has nothing to exceed, so it reports the same
`within` it would report on a measured clean cut.

**What to build**: the same population assertion the forced-dive sentry gives
collisions — a gate must be able to say NOT MEASURABLE and abstain, rather than
return a verdict computed over an empty set. `sim_measurability` already models
exactly this vocabulary (`Measurability::NotMeasurable` with a stated reason)
and the chipload gate should use it: no matched band => abstain with
`NoVendorData`, never `Within`.

Note the export gate ALREADY does the right thing on the same fact — it refuses
with `chipload=NoVendorData` rather than passing. So two consumers of the same
missing data disagree: the exporter treats absence as a refusal, the diagnostic
treats it as a pass. Making the gate abstain would align them.

**Severity note**: "tool breakage risk" currently ships as `severity: hint`.

Credit: the population-assertion framing is `rs-cam-2c`'s, from the wave's
forced-dive sentry work.

### RE-DIAGNOSED 2026-08-22 — the gate already abstains; two things above are wrong

Investigated before building the fix, and the fix turned out not to be needed.
Both errors are mine, and both are worth keeping because each is a mistake a
reader of this log would otherwise repeat.

**1. The gate does not pass on an empty band.**
`tool_load::chipload::evaluate` returns `Unmodeled(NoVendorData)` when
`matched_chip_envelope` finds no row (`chipload.rs:561`), and again when the
matched row's bounds fail `derate_chipload_bounds` (`:592`). Every path that
reaches `Within` has a band in hand. There is no empty-band `Within` to fix.

**2. The probe that "showed no band at any sampled depth" asked the wrong
resolver.** It called `feeds::calculate`, which resolves through
`find_best_row_for_geometry` — the **recipe** resolver, which lets RPM-only
anchors compete and then publishes no band. The gate resolves through
`find_best_chip_envelope_row`, which excludes exactly those rows. The LUT ships
**eight** tapered-ball rows and every one publishes a chipload band, so the
gate almost certainly had one. *This is the same two-resolver asymmetry that P1
fixes on the Suggest side* — which is how the misdiagnosis happened: the symptom
is real and it is the recipe resolver's, not the gate's.

**What was checked, since "the gate is fine" needs its own evidence.** The
mechanism the report described — an abstention deleting the pre-sim caution —
is genuinely one line away from being real. An abstention is published under
the id `LOAD_CHIPLOAD_WITHIN`, the *same id a real pass uses*, and it carries a
populated `supersedes` list naming the `feeds.*_vs_lut.*` heuristics. If
`apply_supersession` keyed on the id it would fire. It keys on
`state == Current`, and abstentions carry `NeedsSimulation` / `StaleEvidence` /
`NotApplicable`. That is a **two-place invariant** — the adapter must not mark
an abstention `Current` and the reducer must not stop checking — so it is now
pinned at both ends by `tests/chipload_abstention_cannot_supersede_g_chipgate.rs`
(3 tests, including the control arm that a *modelled* `Within` still supersedes,
without which the main assertion would pass just as well if supersession were
deleted entirely).

**What is still open, and it is the operator's question, not this one.**
Why does a commanded 3000 mm/min on a Ø1.0 tapered ball produce a genuine
`Within`? The two surfaces measure different quantities:

| surface | quantity |
|---|---|
| pre-sim heuristic | **commanded** feed vs LUT recommendation |
| chipload gate | **achieved** advance per tooth, `effective_feed / (rpm · flutes)`, from the kinematics-predicted feed |

On short finishing moves with a small tool the predicted feed can be a fraction
of the commanded one, in which case both surfaces are right about different
things and neither is a defect. **That is a hypothesis and has not been
measured.** The probe is one call against the live project —
`get_tool_load_report().per_toolpath[].chipload` carries the observed advance
beside the band — and it needs the GUI. Until it is run, do not treat the
`within` as clearance to cut at F3000 on that tool: an abstention was ruled out,
an *agreement* was not established.

**Not closed, re-pointed.** The population-assertion instinct that raised this
was right in general; it was aimed at a gate that already had the property.

## RUBBING-FLOOR P1 — LANDED 2026-08-22, and it does NOT explain the live symptom

Not from the airrun itself but from the measurement it triggered
(`tests/rubbing_floor_diameter_scaling_measurement.rs`, written 2026-08-21/22).
Recorded here because the live symptom is one of this session's: the operator's
Ø1.0 tapered ball had its finishing feed raised by a floor that was reading no
band.

**The rule was right; its input could be wrong.** `effective_rubbing_floor`
subordinates `RUBBING_FLOOR_MM_TOOTH` (0.025) to the matched band's ceiling, so
the clamp can never push a recipe past the window it exists to keep it inside.
But the band it is handed is `chipload_bounds`, from the **recipe** resolver —
the one that lets RPM-only anchors win. When one wins, `chipload_bounds` is
`None`, the floor falls back to the bare constant, and the post-sim gate then
judges the same cut against a *chipload-bearing* row it resolved separately.

### The premise did not survive being probed

The measurement file proposed P1 as the explanation for the live Ø1.0
tapered-ball case, hedged with "plausibly". The hedge was doing real work. Probed
directly, on the operator's exact tool:

```text
Ø1.0 tapered ball (tip r0.5, 7deg), 2F, white oak, ap 0.3
  parallel/finish  recipe row = amana-tapered-hardwood-parallel-3175-2f
                   envelope   = amana-tapered-hardwood-parallel-3175-2f   <- SAME ROW
                   bounds = 0.00484 .. 0.00968     floor applied = 0.00968
                   ChiploadClampedToFloor { requested 0.00581, floor 0.00968,
                                            band_capped_from: Some(0.025) }
  scallop/finish   same shape, band 0.00581 .. 0.01161, floor 0.01161
  contour/finish   recipe = None, envelope = None, bounds = None
                   ChiploadClampedToFloor { requested 0.01205, floor 0.025,
                                            band_capped_from: None }
```

The two resolvers **agree** on that cut, the band was already in hand, and the
floor was **0.0097, never 0.025**. The reported symptom — a ~0.012 request
raised to a flat 0.025 — is the **third** row: a cut where *neither* resolver
matches any row. No resolver fix can reach that; with no row there is no band to
subordinate to. Only a floor that carries a diameter would (P2, still not
adopted).

`band_capped_from` is what tells the two shapes apart on a live surface:
`Some(0.025)` = a band was found and beat the constant; `None` = the bare
constant applied because nothing was found. **Read that field before concluding
anything about a clamped feed.**

### What P1 is worth, without inflation

It aligns two resolvers that had no business disagreeing, introduces no number,
and on the LUT as shipped **changes no recipe** — measured, not assumed: the
cells where the resolvers disagree are the Ø6-and-up flat/bull ones, whose
envelope bands sit above 0.025, so `min` returns the constant unchanged. Kept
because a floor consulting the resolver that cannot see bands is wrong whether
or not it currently costs anything. `the_fallback_does_not_lower_the_floor_on_todays_lut`
is a tripwire that will report the day it starts costing something.

**What changed**: when the recipe row publishes no chipload, `feeds::calculate`
now resolves the **envelope** row — the gate's own resolver, same query, same
DOC derate, same `RequireBoth` policy — and hands *that* band to the floor. The
clamp reason is derived from the same band, so the warning and the explanation
record cannot name a ceiling the floor did not use.

**What deliberately did NOT change**, because it is a different claim needing
different evidence:

- `chipload_bounds` stays `None`. The recipe legitimately rests on the RPM
  anchor; re-pointing Suggest's *target* at another row is not this fix.
- No constant moved and no exponent was introduced. `p1_moved_no_constant`
  pins that. The diameter-scaled floor (P2 in the measurement file) is
  **not** adopted.

**Disclosure**: `FeedsWarning::VendorRowPublishesNoChipload` gained
`floor_band_from: Option<String>`, naming the row, rendered on all three
surfaces (properties panel, feeds modal, diagnostics adapter). The adapter's
message used to end "so its verdict is judged against bounds this recipe never
saw" — after P1 the floor *did* see them, and saying which is what stops the
operator reading the old sentence and assuming nothing did.

Sentry: `tests/rubbing_floor_envelope_band_p1.rs` (5 tests), which
**discovers** the affected inputs by sweeping shipped data rather than
hard-coding a row id a LUT edit could retire, and fails naming the axis to widen
if the population is empty. The first draft of that sweep hand-picked 20
combinations and hit **zero** — A-6 measured this disagreement at 141 of 18 144
queries, so a fixture list was the wrong instrument and the non-vacuity guard is
what said so.

**Still open, and now the sharper question**: the binding cases are the cuts
with no vendor row at all. The quantity that governs them is cutting-edge
radius, and `ChiploadSource::EdgeRadiusFloor` is a declared, rendered, wired-up
variant that **nothing in the workspace ever constructs**. That is the empty
slot for the model that would actually answer this.

## G-PINAUTO — auto pin placement can key the flip, and today cannot

Raised 2026-08-22 at the machine, before the first real cut. Three separate
defects in one feature, found by checking the operator's actual project.

### 1. The margin ignores the pin, and reads the wrong source

`handle_setup_two_sided` (`viz/controller/events/model.rs:290-303`) places two
pins at `(margin, y/2)` and `(x - margin, y/2)` with
`margin = padding/2` (when `padding > 2`).

Two things wrong with that margin:

- **It never consults the pin diameter.** On wanaka (`padding = 5`, Ø6 pins)
  it gives `margin = 2.5`, so a Ø6 hole spans −0.5..5.5 mm relative to the
  stock edge — it **hangs 0.5 mm off the stock**. There is no material for the
  dowel to bite. A 5 mm padding ring cannot hold a 6 mm pin at any offset, so
  the correct behaviour is to REFUSE, not to emit a broken placement.
- **`padding` is the wrong source.** On wanaka the real free area comes from
  the stock being explicitly 140×150 around a 100×100 model — a **20 mm** clear
  strip in X, 25 mm in Y — while `padding` says 5. Deriving the margin from the
  MODEL BBOX gives the true room.

### 2. Nothing validates that pins survive the flip

`FaceUp::Bottom` is `(x, D-y, H-z)`: X preserved, **Y mirrored about y = D/2**.
For a part to re-seat after the flip, the pin multiset must be invariant under
that map. Nothing checks it.

wanaka's stored pins were `(2.5, 2.5)` and `(137.5, 147.5)` — diagonal, i.e.
symmetric under a 180° ROTATION, which is not a flip. Under `y -> 150 - y` they
map to `(2.5, 147.5)` and `(137.5, 2.5)`: neither lands on a dowel. **The part
could not have re-seated.** Note the X values (2.5 / 137.5) are exactly what
auto-placement produces, so those pins began as auto output and had their Y
hand-moved off the centreline; current auto (both pins at `y/2`) would have
been flip-correct.

### 3. Keying is achievable and is not being done

A rectangular stock re-seats four ways: identity, `My` (flip about X,
`y -> D-y`), `Mx` (flip about Y, `x -> W-x`), and `R180 = Mx∘My`. The CAM
models exactly `My`. So the requirement is: **invariant under `My`, and under
nothing else.**

- On the mirror line (`y = D/2`) every pin maps to itself → the flip seats.
- `Mx` and `R180` both send `x -> W - x`, so if the pin x-multiset is NOT
  invariant under that, both wrong orientations are mechanically blocked.

Two pins suffice: put both on the mirror line with `x1 + x2 != W`. The
asymmetry must be much larger than hole slop (a few mm against ~0.1 mm fit),
or the operator can force it.

**Does an odd pin count help?** Not inherently — what keys the part is the
x-multiset failing `x -> W-x` invariance, and a centre pin at `x = W/2` is
self-symmetric, so it neither helps nor hurts. Three pins buy redundancy and
resistance to rocking, not keying. On wanaka a third pin has nowhere to go: the
only clear strips are `x ∈ 0..20` and `x ∈ 120..140`, and the middle is under
the model.

### Applied live on wanaka 2026-08-22

`(2.5, 2.5) + (137.5, 147.5)` → **`(10, 75)` + `(126, 75)`**, Ø6.

| seating | pin images | result |
|---|---|---|
| `My` (the modelled flip) | (10,75), (126,75) | seats |
| `R180` | (130,75), (14,75) | blocked |
| `Mx` | (130,75), (14,75) | blocked |

Clearances: 11 mm to the stock edge, 3 mm to the model boundary, 4 mm of
keying asymmetry. NOT saved over `wanaka.toml` (never-touch); live session only.

### Proposed auto algorithm

1. Read the flip from the setup's `face_up`. Only `Bottom` is the in-plane
   two-sided case; `Front/Back/Left/Right` stand the part on its side and
   change the footprint — refuse rather than guess.
2. Derive the free strip from the **model bbox**, not `padding`.
3. Place both pins ON the flip's mirror line.
4. Offset one along that line so the pair is not centre-symmetric, by
   `max(3 mm, 2 × expected hole slop)`.
5. Require `margin >= pin_radius + wall`; **refuse with a stated reason** when
   no valid placement exists (wanaka's Ø6-in-5 mm case).
6. Warn or refuse on LOAD when a project's stored pins are not invariant under
   its own flip — which is what would have caught this one.
7. **Size the pin from the TOOL, not a constant** (operator instruction,
   2026-08-22). `handle_setup_two_sided` hardcodes `AlignmentPin::new(.., 6.0)`.
   The pin diameter should come from the tool the pin-drill op will actually
   use, so the hole the operator gets matches the dowel the geometry assumed —
   and so step 5's `margin >= pin_radius + wall` is computed against the real
   radius rather than a guess. A hardcoded 6.0 against a Ø3 cutter is a hole
   that never gets drilled at size; against a large cutter it is a margin
   computed for the wrong pin.

### Adjacent: `flip_axis` is decorative

`StockConfig::flip_axis` is saved, loaded, defaulted to `Horizontal` by the
two-sided button, and rendered in the stock panel — and is **read by nothing**
in core. Its own doc contradicts itself ("mirror about the X centerline" flips
Y; "Y stays" flips X). On wanaka it is `null` while a `Bottom` setup exists.
Either wire it to the validation above or remove it; a control that means
nothing is worse than no control.

### Adjacent: a tool's display name can lie about its geometry

wanaka tool id 2 is named "Tapered Ball 2mm tip / 7° / 6mm shank" and has
`diameter: 1.0` — i.e. **R0.5 / Ø1.0 tip**, half the named size. For a tapered
ball `diameter` IS the tip diameter (`TaperedBallEndmill::new(ball_diameter,..)`).
Every feed decision follows the number, every human decision follows the name.
Also present on that tool: `corner_radius_mm: 2.0` and `included_angle_deg: 90`,
neither meaningful for a tapered ball — the type-agnostic `add_tool` defaults
that `b0362626` fixed going forward but which persist in saved projects.

## G-SUGGEST-POWERSTALE — CLOSED 2026-08-21, does not reproduce

Pass 9 (`rescale_feed_to_final_geometry`) re-solves the feed against the
operation's FINAL geometry, and nothing between it and the write re-checks the
Step 6 power ceiling — which was computed against the CALCULATOR's operating
point. Power scales with `ap · ae · feed`, so on the face of it a rescale that
raises the feed can leave the machine commanded beyond its envelope.

**Searched, not sampled. It does not happen, and not by luck.** Pass 9 holds the
operation's *implied chipload* fixed and re-multiplies at the final geometry —
and that chipload came from the feed Step 6 had **already clamped**, so the
power clamp rides through the rescale proportionally. `enforce_invariants`
meanwhile clamps geometry downward, shrinking the cross-section.

Measured on the synthetic under-powered spindle (the only place the power
branch is reachable at all — no shipped preset ever engages it):

| population | peak shipped load vs gate ceiling |
|---|---|
| single heaviest point | **59.9 %** |
| 70 requested depths across every tier boundary, 69 power-limited | **75.0 %** |
| all shipped presets × ten species | **26.6 %** |

The sweep is the load-bearing arm. `depth_tier_multiplier` is **stepped**
(1.0 / 0.75 / 0.50 / 0.45 at ap/D of 1 / 2 / 3), so a clamp crossing a boundary
downward buys up to 1.33× feed for an arbitrarily small loss of cross-section —
the one shape that could outrun a ceiling checked before the rescale. That
regime was searched explicitly and stays inside the envelope.

**Two non-vacuity arms are part of the result, not decoration.** At Ø6 the first
fixture never reached the power branch at all: a full-width slot trips
`SlottingDetected`, which cuts DOC to 1.5 mm *before* Step 6, and the collapsed
cross-section left the load far under the ceiling — the headline arm passed
while proving nothing. The guard caught it; the fixture moved to Ø12. This is
the third time on this programme that a gate handed an empty population read as
healthy, so the arms stay.

Sentried by
`crates/rs_cam_core/tests/suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs`
(5 arms). Kept rather than deleted: they fail if a future change makes pass 9
re-solve from something other than the clamped feed.

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

## G-GEOMCACHE-FLAKE — a unit test asserts on a process-global others write to (OPEN, filed 2026-08-22)

Not a product defect. Found while running the verification gate for the
G-DRILLFLIP / G-DRILLTIME work: `cargo test -p rs_cam_core` failed once with

```text
geom_cache::tests::a_dropped_mesh_releases_its_entry
  panicked at crates/rs_cam_core/src/geom_cache.rs:390
test result: FAILED. 2363 passed; 1 failed; 12 ignored
```

and then passed **three consecutive full-lib runs** (2364/0 each) with no code
change in between. Isolated (`--lib geom_cache`) it passes every time.

**Mechanism.** `geom_cache` is a process-global. The test calls `clear()`,
inserts one mesh, drops it, inserts a second to provoke the sweep, and asserts
`cache_len() == 1`. Its two siblings — `second_lookup_reuses_the_same_allocation`
and `distinct_meshes_get_distinct_indexes` — insert into that same global and
run **concurrently** on other threads, so `cache_len()` observes their entries
too. `clear()` at the top narrows the window; it does not close it.

Not attributable to this session's changes: nothing here touches `geom_cache`,
which came in with `2f94dd48` (`perf(gen-w4)`, the concurrent perf programme).
Left for that programme's owner rather than edited from here.

**Fix shape** (any one of): serialise the three with a test-local mutex; assert
on the specific key's presence rather than the map's length; or give the test
its own cache instance. The length assertion is the part that is unsound under
`cargo test`'s default parallelism — a count over a shared global is not a
statement about this test.

**Why it matters beyond tidiness**: it fails roughly one run in four here, so
it will fail CI intermittently, and an intermittent red gate is the thing that
teaches people to re-run instead of read.

## G-PECKROOT — CLOSED 2026-08-22 (operator go-ahead)

The peck-plunge entry ladder was rooted at `params.safe_z` — the retract plane
— and knew nothing about the stock top, so with `SAFE_Z_CLEARANCE_MM` (5.0)
exceeding `depth_per_pass` the first rung, often the first two, landed entirely
in air at plunge feed, each with its own retract rapid after it.

**The root height was not a new decision.** `dressup::emit_ramp` and
`emit_helix` — the other two entry styles, facing the identical problem —
already share one rule: rapid no lower than `stock_top + ENTRY_CLEARANCE`, then
feed. That 2 mm margin exists because `stock_top_z` is a *nominal* flat value
and real timber is proud of it. Peck entry now uses the same constant rather
than inventing a second stock-top margin that could drift from it.

**Measured, and the two numbers came apart — which is the interesting part.**
On the wanaka-shaped fixture:

| quantity | before | after |
|---|---|---|
| cutting Z levels entirely above the stock top | 1 | **0** |
| fed path length above the stock top | ~27 mm | 26.282 mm |

That is not a disappointing second number, it is two different quantities:

* **Levels** counts rungs whose whole descent is in air — pure waste. Zero now,
  and that is the sharp bar.
* **Fed mm** also counts the upper part of a move that *crosses* the plane. The
  first rung now starts at the 2 mm guard and ends in material, so 2 mm of it
  is above the top; over ~13 entries that is the 26.282 mm. **It is the guard
  band, once per entry, and nothing else.**

Driving the second to zero means feeding from the nominal stock top, deleting
the over-thickness allowance both sibling entry styles keep. That is a
machine-safety trade, not a tidy-up, and it has **not** been made. Both bars are
now pinned in `air_ladder_emitted_z_levels_g_airladder.rs` with that written
out, so nobody "finishes the job" by chasing the wrong one.

Caveat recorded there too: a `depth_per_pass` finer than `ENTRY_CLEARANCE`
(2 mm) would put a whole rung inside the guard band and push levels back to 1.
No shipped default does that (wanaka is 4.2).

## G-UNITSRELOAD — CLOSED 2026-08-22 — a 2D model's units are dropped on reload

**Not lateral, not from the airrun, and more urgent than either**: it hits the
ordinary Top workflow.

A project file stores a model's **path** and its declared `ModelUnits`, not its
geometry — both load doors re-import the same file, so both must apply the same
scale. They did not:

| door | STL | SVG | DXF polygons | DXF drill targets |
|---|---|---|---|---|
| `io::load_model_file` (interactive import) | scaled | scaled | scaled | scaled |
| `project_file::load_model_geometry` (project load) | scaled | **dropped** | **dropped** | **dropped** |

`load_model_geometry` computed `let scale = model.units…scale_factor()` and
handed it only to `TriangleMesh::from_stl_scaled`. `ModelUnits`' own doc still
reads *"Assumed units of the imported **STL**"* — it predates 2D import, and
when the SVG/DXF arms were added they never consumed it. `save.rs:144` writes
`units: m.units`, the original declared units, so the information was on disk
and simply unread.

**Effect**: an inch-authored DXF or SVG reloads **25.4x smaller**, silently,
with the stock still at its saved size — `update_from_bbox` runs on import, not
on load, so nothing re-fits and nothing complains. Measured on the repo's own
fixture: import door 2538.02 mm, project door 99.92 mm, ratio exactly 25.4.
Toolpaths then regenerate cleanly around a part a fortieth of its intended
size.

This is the **third** divergence found in this loader pair. The previous two
closed 2026-06-08 with "nothing left to consolidate" — true of the divergences
then known, and the reason the claim now has a test rather than a note.

Sentry: `model_units_survive_reload_g_unitsreload.rs`, which asserts the two
doors **agree** rather than asserting a size — a test pinning "254 mm wide"
would pass just as well if both doors were wrong together. It also pins that
the units dial is not inert, without which the agreement test could pass
trivially.

**Open for the operator**: whether any saved project already declares non-mm
units on a 2D model, i.e. whether this has bitten a real job.

## G-LATERALSIGN — CLOSED 2026-08-22 — `cut_direction()` inverted on all four lateral faces

Found while researching the lateral-setup question, by an agent asked to
**falsify** a claim of mine rather than confirm it.

`SetupTransformInfo::cut_direction()` mapped `FaceUp::Front -> FromFront`,
`Back -> FromBack`, `Left -> FromLeft`, `Right -> FromRight`. Every one is the
wrong sign, because **the two names describe opposite ends of the same setup**:

* `FaceUp::Front` = *the front face is up*, pointing at the spindle.
* `StockCutDirection::FromFront` = *the tool arrives from the front side*, and
  its own doc pins that as the -Y side.

If the front face is up, the tool arrives from where that face now points.
`inverse_transform_point` sends local `+Z` to global `+Y` for `FaceUp::Front`,
so the tool arrives from **+Y** — which is `FromBack`. Both lateral axes negate.

**Why it survived**: the two Z faces *are* an identity (`Top -> FromTop`,
`Bottom -> FromBottom`), so the mapping reads as obviously right and the only
two cases with fixtures both pass.

**Effect**: on the global stock the kernel called `subtract_below` where it
should call `subtract_above` — deleting everything from the far face up to the
cut plane instead of the shallow layer at the near face. Confined to the
live-scrub viewport; no number an operator reads moved, because every metric,
gate, collision check and checkpoint mesh comes from the per-setup local stock.

Sentry: `cut_direction_matches_transform_g_lateralsign.rs`. It deliberately does
**not** transcribe a table of six expected answers — a hand-written table is the
same kind of artefact as the mapping it checks and would have been written wrong
by the same reasoning. It pushes points through `inverse_transform_point`,
measures which global axis local `+Z` lands on and with what sign, and requires
`cut_direction()` to agree. The two Z faces are the control on the derivation.

## Lateral setups — the earlier framing was wrong, and the news is better

The G-SIDEFACE-POLYCOLLAPSE note above said "milling works — `grid_for_direction`
lazily allocates the X/Y dexel grid". **Retracted.** Lateral milling works, but
not by that mechanism: a lateral setup is simulated **entirely in its own
setup-local frame**, where Z is always the tool axis, with `direction`
hardcoded to `FromTop` at `compute/simulate.rs:893`. Metrics, gates, collision
checks, checkpoint meshes and `prior_stocks` all come from that local stock and
are all correct. The X/Y grid machinery is reached by exactly one object, the
global playback stock, where the sign was inverted (above).

Corrected picture:

| layer | lateral status |
|---|---|
| G-code emission | correct — emitted setup-local, where local Z *is* machine Z |
| metrics / gates / engagement / collisions | correct — `group_stock`, `FromTop` |
| checkpoint + composite mesh | correct — local mesh, then frame-mapped |
| rest machining (`prior_stocks`) | correct — local stock clones |
| global playback stock cut sign | **was inverted — fixed (G-LATERALSIGN)** |
| live-scrub viewport mesh | **broken — G-LATERALSCRUB below** |
| 2D polygon ops | **broken — G-SIDEFACE-POLYCOLLAPSE** |
| analytic drill removal, global stock | abstains — G-DRILLLATERAL |

So it was never "three silent behaviours behind one dial". Full spec, including
the 2D semantics recommendation and the effort/impact argument, in
`planning/lateral_setups_2026-08-22/SPEC.md`.

## G-POLYTRANSFORM-DUP — two copies of the polygon transform, already divergent (OPEN)

`apply_to_polygons` (`compute/transform.rs`) and `transform_polygons`
(`rs_cam_viz/src/state/job.rs:519-557`) are the same algorithm written twice,
both carrying the `Z=0` hardcode. **They already disagree on something else**:
core calls `ensure_winding()` only when `result.closed`, the viz copy calls it
unconditionally — so open paths (rivers, traces) can be reversed by the GUI
path and not by the core path.

Independent of lateral setups, and the reason to close it first: it makes the
2D-semantics change a one-place edit instead of two. Fix by **deleting the viz
copy** and routing it through core, not by patching both.

## G-LATERALSCRUB — the live-scrub mesh cannot show a lateral cut (OPEN)

`dexel_stock_to_mesh` builds a closed marching-cubes solid from the Z grid and
then **appends** the X/Y grids as open per-segment heightmap surfaces. There is
no boolean, and the three grids are never reconciled — `ensure_grid` allocates a
pristine full-material X/Y grid at the first lateral stamp, knowing nothing
about what the Z grid already lost (asserted as intended behaviour by
`multi_grid_simulation_preserves_z_grid`).

So a lateral cut can never remove material from the rendered solid on any
surface fed by the global stock, and the checkpoint mesh and the live-scrub mesh
**disagree** for a lateral setup.

Cheapest honest fix: route the live-scrub path through the same
local-stock-then-frame-map that checkpoints already use, rather than teaching
the side grids to boolean.

Also noted while there: `SimGroupEntry.direction` is **dead code** — written by
three producers and read by nobody (`compute/simulate.rs` uses its own local
`FromTop`), and the producers already collapse all four lateral faces to
`FromTop`, so it would be wrong even if read. Delete it or read it.

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

**DONE 2026-08-22 (second pass, this session):**

- ~~**G-DRILLFLIP**~~ — CLOSED. `apply_drill_op` takes the frame's
  `StockCutDirection`; flipped setups carve the right band and a break-through
  pin hole appears at all. Split out **G-DRILLLATERAL** (below, item 9).
- ~~**G-DRILLTIME**~~ — CLOSED. Drill toolpaths are integrated into the
  cycle-time model via `SimulationCutTrace::toolpath_runtimes`; 0.8 % of
  runtime no longer relabels 100 % of the estimate.
- ~~**G-CHIPGATE-POPULATION**~~ — RE-DIAGNOSED, not a defect. The gate already
  abstains on an empty band; the probe that said otherwise asked the recipe
  resolver instead of the envelope one. The abstention-cannot-supersede
  invariant is now pinned at both ends. **The operator question it raised is
  still open** — why F3000 on the Ø1 tapered ball reads `Within` — and needs
  one live `get_tool_load_report` call to settle.
- ~~**Rubbing-floor P1**~~ — LANDED, and it does **not** explain the live
  symptom; that was a no-vendor-row case, which no resolver fix reaches. P1
  changes no recipe on today's LUT.

**Outstanding, in rough priority order:**

1. ~~Emit corrected G-code~~ — **DONE.** Both files re-exported on the fixed
   binary, datum verified, pre-fix files deleted.
2. ~~**G-SUGGEST-NOCLAMP**~~ — **FIXED 2026-08-19** (`a1bb964b`, Suggest pass 9
   `rescale_feed_to_final_geometry`). This row said "not root-caused, top
   priority" long after the fix landed because only the detail section below
   was updated — corrected 2026-08-21. Still open from that work: the seed
   target (band-max seeding measured 56.3% in-band vs the 45.9% shipped, which
   re-opens the v3.0c midpoint ruling — operator ruled 2026-08-21 to LEAVE IT
   at midpoint), and the fact that 54% of banded points still command below the
   vendor minimum, which is the derate stack, not the multiplier.
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
9. **G-SIDEFACE-POLYCOLLAPSE** (new, 2026-08-21, unverified). `apply_to_polygons`
   projects through `world_to_local(P3::new(p.x, p.y, 0.0))`, so on
   `FaceUp::{Front, Back, Left, Right}` setups the local Y (or X) becomes a
   constant and a 2D polygon collapses to a degenerate line rather than a
   polygon. Found while fixing G-PROFILE-FLIP; that fix is a no-op here
   (shoelace of a degenerate ring is 0). Needs a decision on whether 2D ops on
   side-face setups are meant to work at all before it is worth fixing.

   **2026-08-22 — this is now the second lateral-setup defect, and they share
   one question.** G-DRILLFLIP's fix surfaced **G-DRILLLATERAL**: `DrillHole`
   cannot express a hole whose axis is global X or Y, so a drill on a side-face
   setup abstains rather than carving. Milling, by contrast, *does* work on
   those setups — `grid_for_direction` lazily allocates the X/Y dexel grid and
   `StockCutDirection::decompose` reorients the stamp — so the support is
   genuinely partial: **milling yes, 2D polygon ops no, drilling no.**

   That makes "are lateral setups supported?" one operator decision covering
   both rows, not two independent fixes. If the answer is no, the honest move
   is to refuse the setup at the UI rather than ship three different silent
   behaviours behind one dial. If yes, both are real work:
   `apply_to_polygons` needs a projection plane rather than a hardcoded Z=0,
   and `DrillHole` needs two 3-D endpoints instead of an XY pair plus two
   scalars.
10. **G-PECKROOT** (new, 2026-08-21, measured). `emit_peck_plunge` is rooted at
   `safe_z` and knows nothing about the stock top, so while
   `SAFE_Z_CLEARANCE_MM` (5.0) exceeds `depth_per_pass` the first entry rung
   always lands above the stock — one wasted fed rung per entry, ~2 mm each.
   Rooting at the stock top takes it to zero but changes adaptive3d entry
   motion and moves fingerprints, so it needs an explicit go-ahead. Sentried at
   the current residual by `air_ladder_emitted_z_levels_g_airladder.rs`.
11. Commit decision on the two fixes; the viz half of the sim fix has had a
   partial line-by-line review only.
