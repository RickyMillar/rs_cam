# G-ISOCLIPRAPID — report (branch isoclip-rapid, NOT committed)

## The move
`scratchpad/repro/t2_before_02.nc` line 86537:
`G0 X124.911 Y40.278 Z-3.687` from `(129.071, 38.102, -3.687)`, dxy 4.695.
Datum: emitted = world + (20, 25, -7). World = (109.1, 13.1, 3.31) -> (104.9, 15.3, 3.31),
the reported collision at move 68538. Identification is by SHAPE plus the datum, not by a
MoveIntent tag read from the .nc.

Three lateral rapids below the stock top exist in the whole 6.1 MB program (lines 18421,
41903, 86537). The reported one carries the full lead-in signature: a lead-out G2 arc ends
at Z, the next G0 travels 4.7 mm laterally at that same Z, then a pure-Z G1 at the lead
feed climbs onto a probe-lifted target, then the lead arc, then the cut. Line 41903 has the
same shape. All three vanish in the after-run.

## The emitter
`dressup::apply_lead_in_out_with_provenance`, the "Move 1" pre-position rapid:
`let safe_z = moves[i - 1].target.z;` — documented as "the preceding rapid's Z", but the
move before a plunge is not always a rapid. A lead-out arc, a stepped pass, or an earlier
dressup puts a CUTTING move there, so the lead-in traversed the work at cutting depth.

## The fix (2 parts, one site)
1. `apply_lead_in_out_with_provenance` takes `retract_z: Option<f64>`.
   `Some(plane) => plane.max(moves[i-1].target.z)` — the operation's retract plane, floored
   at `stock_top + SAFE_Z_CLEARANCE_MM` by `compute::config::effective_safe_z`, and never
   LOWERED below the inherited height. `None` keeps the old behaviour for the two
   convenience wrappers and for tests. The one production caller,
   `compute::execute::apply_dressups`, passes its own `safe_z`.
2. A "Move 0" pure-vertical `Retract` lift before the traverse, emitted only when the tool
   stands below the plane. Without it the stored IR holds a rising DIAGONAL out of the cut:
   `gcode::program_builder::push_rapid` splits that safely at post time, but every reader of
   the IR sees a strike, and `collision::RapidClearanceCheck` exempts pure-vertical climbs,
   not diagonals.

## Falsification
Sentry `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs`, 4 arms, two islands
over a rest stock. Pre-fix: arm `a` failed at Z 0.000 and arm `c` at 2.000 mm of burial.
Post-fix all four pass. Arm `c` runs end to end through `apply_dressups` +
`clip_toolpath_to_boundary_set_with_provenance` and asserts a vacuity precondition (>= 2
lead-in rapids counted on the DRESSED path) so it cannot pass on an empty population.
The fixture uses `entry_style: None` and never touches `emit_ramp`, so it attributes THIS
fix and not G-ISOCLIPENTRY.

## Before / after
### wanaka T2 @ 0.2 mm (`T2_r20_raster_then_r10_iso_islands.toml`, --emit-gcode)
CONFOUNDED PAIR: the before-run predates BOTH G-ISOCLIPENTRY and this fix.

| | before | after |
|---|---|---|
| rapid collisions, #18 tier-1 iso | **1** | **0** |
| verdict | WARNING: 1 rapid-through-stock collisions | WARNING: 63.1% air cutting |
| lateral sub-stock rapids in the .nc (`--below 1.99 --min-xy 0.05`) | 3 | 0 |
| rapids at emitted Z 2.0 | 3292 | 425 |
| tier-1 moves | 157877 | 138917 |
| time | 25580 s | 19691 s |
| wall clock | 1:26 | 1:29:52 |

The coordinator confirmed independently that the T2 collision was ALREADY gone on a rerun
carrying G-ISOCLIPENTRY alone. So the T2 count does not attribute this fix. The sentry does.

### wanaka T3 @ 0.5 mm — cost isolation (both arms carry G-ISOCLIPENTRY)
| | T3 master | T3 + rapid fix | delta |
|---|---|---|---|
| Time | 10016 s | 10017 s | +1 s (+0.01%) |
| Rapid | 47948 mm | 47931 mm | -17 mm |
| Cutting | 67544 mm | 67546 mm | +2 mm |
| tier-1 moves | 64686 | 64697 | +11 |
| collisions | 0 | 0 | - |
| rapids at emitted Z 2.0 | 178 | 114 | -64 |
| rapids at emitted Z 5.0 | 4295 | 4366 | +71 |

The lift fires 64 times and buys back 17 mm of rapid distance. The air cost is +1 s.

## Files
- `crates/rs_cam_core/src/dressup.rs` (+60)
- `crates/rs_cam_core/src/compute/execute.rs` (+5)
- `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs` (new, 4 arms)
- `crates/rs_cam_core/tests/entry_moves_stock_aware_g_rampterrain.rs` (+4, new arg)
- `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs` (re-pin, 73 lines)

The fingerprint re-pin is deliberate and moves counts and sites: exactly 6 `Retract` lifts
are inserted at indices 2, 14, 26, 38, 50, 62 (each `[3.0, y, 2.0] -> [3.0, y, 30.0]`) and
the 6 `LeadIn` traverses move Z 2.0 -> 30.0. Nothing else changes. Verified move by move
with a temporary panic dump. Stage 1 (74, 8357027825945903145) -> (80, 6526175378945769562);
stage 2 (97, 7877196034056840142) -> (103, 10899331192678125387); stage 3
(6, (103, 3086279569100738182)) -> (6, (109, 2717567159789683815)); link sites
(0,32)/(33,74)/(75,102)/(0,102) -> (0,34)/(35,79)/(80,108)/(0,108).

## Gates (all green)
- `cargo fmt --all -- --check` — clean
- `cargo clippy -p rs_cam_core -p rs_cam_cli --all-targets -- -D warnings` — clean
- `cargo clippy -p rs_cam_viz --all-targets -- -D warnings` — clean
- `cargo test -p rs_cam_core --test isoclip_link_rapid_g_isocliprapid` — 4 passed
- `cargo test -p rs_cam_core --test isoclip_entry_ramp_g_isoclipentry` — 6 passed
- `transform_provenance_fingerprints` 3, `entry_moves_stock_aware_g_rampterrain` 5,
  `boundary_reentry_plunge_rate_g_boundaryplunge` 4, `lead_in_out_feed_rates_f040` 5,
  `lead_out_retract_lifts_from_the_arc_f2` 2, `arcfit_intent_boundary_f1` 5,
  `scallop_intra_pass_relink_am7` 5, `retract_intent_move_type_census_w6` 2,
  `capability_link_moves_safety` 17 — all passed
- `cargo test -p rs_cam_core --lib -q` — 2484 passed, 0 failed, 12 ignored

## Ledger text (G-ISOCLIPRAPID)
`apply_lead_in_out` planted its pre-position rapid at `moves[i - 1].target.z`, documented as
"the preceding rapid's Z". The move before a plunge is not always a rapid: a lead-out arc,
a stepped pass, or an earlier dressup leaves a CUTTING move there, so the lead-in traversed
the work at cutting depth. On the wanaka200 tier-1 islands at 0.2 mm that emitted one rapid
through stock, move 68538, 4.7 mm of lateral travel with both ends at the same sub-stock
height. The fix gives the dressup the operation's retract plane and raises the pre-position
rapid to it, never lowering it, and emits a pure-vertical `Retract` lift first so the stored
IR says what `push_rapid` makes the machine do; a caller with no plane to offer keeps the old
height. `isoclip_link_rapid_g_isocliprapid` is the sentry: two islands over a rest stock,
`entry_style: None` so it never touches `emit_ramp`, one arm on the lead-in's own height and
one end to end through the boundary clip asserting that no rapid crosses the REST stock, with
a vacuity precondition on the lead-in rapid count. The wanaka collision count does not
attribute this fix — G-ISOCLIPENTRY had already removed that particular strike — but the
mechanism is real and the T3 measure of the added lift is +11 moves, +1 s and -17 mm of
rapid distance.

## Not a defect
The T3 @0.5 residual `entry_load` Caution peaks at 0.50 mm, which IS
`entry_bite_budget_mm(1.0) = clamp(1.0 x 0.5, 0.10, 0.50)`. The ladder is working to spec;
`pencil.rs:1034-1041` states that a budget too generous for a given pass is reported rather
than hidden. The 0.2 mm CRITICAL (1.39 mm) is a separate mechanism, under investigation.

---

# G-ISOCLIPRAMPFALL — the G-ISOCLIPENTRY residual (same branch, NOT committed)

## Reproduction
Headless, `t2_after_02` (T2 @ 0.2 mm) reproduces the coordinator's residual exactly:
`entry motion cuts far harder than the pass does: 5504 of 259770 entry samples remove more
than 0.23 mm (2x this pass's own 0.12 mm median body bite), peaking at 1.39 mm at
(115.8, 182.4, -1.59)`.

## The site, from the cut trace (no cargo needed)
`--output-dir` writes the full `SimulationCutTrace` as `simulation.json`. An awk pass over it
names the worst sample and the whole move: toolpath 18, **move 2796**, `source_intent =
entry_ramp`, `axial_engagement_mm = 1.394`, `plunge_descent_mm = 0`, at world
(115.938, 182.538, -2.048) — emitted line 20769 of `t2_after_02.nc`,
`G1 X135.940 Y207.540 Z-9.058`. Reconstructing moves 2780..2805 gives the whole manoeuvre:

| move | intent | shape |
|---|---|---|
| 2790 | linking | rapid down to world 2.279 = `stock_top + ENTRY_CLEARANCE` |
| 2791 | entry_plunge | fed 2.146 -> 0.014 = `end.z + ENTRY_CLEARANCE`, max bite 0.020 (air) |
| 2792..2797 | **entry_ramp** | ONE monotone traversal, samples 0.489 mm apart, riding the model floor, then a cliff. Max bite 1.394 and 1.320 |
| 2798 | finishing_cut | the body pass |

0.489 mm is `ENTRY_CLIP_SPACING_MM`. A monotone traversal is not a lap ladder — a ladder
alternates and emits `levels x (window points - 1)` points. So the moves are
`emit_ramp`'s LEGACY two-leg zigzag put through `clip_polyline_to_floor`, and the
G-ISOCLIPENTRY ladder arm above it never fired.

## The mechanism
`dressup::emit_ramp`'s G-ISOCLIPENTRY arm has two abstentions, and BOTH fell through to the
legacy legs:
1. `probe.floor_z(far_xy)` is `None` — the ladder window leaves the mesh footprint.
2. `pencil::plan_entry_ramp` returns `None` — over its own window (1.18 mm on this tip)
   nothing stands that the bite budget does not already cover.

The legs are `ENTRY_CLEARANCE / tan(ramp_angle)` mm long — **38 mm at the shipped 3 degrees**,
against a 1.18 mm ladder window — and they are clipped to the MODEL floor only. On a
rest-driven pass that floor lies BELOW the material, so the return leg walks back across
standing rest stock at full depth. The planner was right that its own window was clear; the
gouge is 5.6 mm further along, outside anything it looked at.

Then the post-clip door stood down: `optimize_entry_descents`' ramp arm skips a plunge whose
`followers[rapid_index + 2].intent == EntryRamp` — the pencil guard, which exists so a
generator that ramps its own entry is not laddered twice. The legacy legs carry exactly that
tag. So one door fired and was gutted, and the other deferred to the result.

## The fix
`crates/rs_cam_core/src/dressup.rs`, `emit_ramp`: where a rest stock is in scope the emitter
is now LADDER OR PLUNGE and never reaches the legacy legs.
- `probe.floor_z(far_xy).unwrap_or(end.z)` — the entry depth is the conservative far floor
  when the probe has no answer. The ladder never descends below `max(level, run z)`, so a
  flat far point can only make the laps shallower.
- `match plan_entry_ramp(..) { Some(plan) => ladder, None => rapid to the conservative
  ceiling + PLUNGE_CLEARANCE_MM, then plunge at the entry column }`, then `return`
  unconditionally. The plunge is the policy `OffMeshEntry::PlungeFallback` already states,
  and the rapid keeps the air part at rapid speed as the legacy pre-descent did.

## Sentry
`crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs`, new arm
`g_an_abstaining_planner_plunges_instead_of_walking_the_legacy_legs`, plus two new fixtures
(`rest_stock_step`, `wide_flat_mesh`) and one new measure.

The fixture leaves the entry column clear and stands the island proud from `STEP_X = 11.5`,
4 mm along the cut direction: the planner reads clear ground over its own window and
abstains, and the legacy legs walk right over the step. Two preconditions assert both halves
of that geometry, so the arm cannot pass on a fixture that says nothing.

The existing `worst_entry_step_mm` reads move TARGETS, which is right for a lap ladder but
blind here: on a flat model nothing lifts the legs, so the emitter writes two endpoints and
the target-only measure reads 0.000 on a leg that gouges. The new
`worst_entry_chord_bite_mm` samples each entry chord every 0.25 mm, buckets the samples by
column in emission order, and drops each column's surface from the stock's own conservative
ceiling — the quantity the simulator reports as `axial_engagement_mm`.

Falsified: pre-fix **1.309 mm** against the 1.000 mm bound (RED); post-fix all 7 arms pass.

## Notes a reviewer will ask about
- **Arms `b` and `f` are permanently RED arms kept green by asserting the DEFECT.** Neither
  moved. Arm `f` drives `dressed_entry(&stock, false)` — `rest_stock: None` — so the new
  ladder-or-plunge block does not fire there at all, and it still reads the full
  `REST_DEPTH_MM`. That is the same reason as before the change, not a new one.
- **The post-clip door is no longer suppressed at these entries.** The pencil guard in
  `optimize_entry_descents` skips a plunge whose `followers[rapid_index + 2].intent ==
  EntryRamp`. Pre-fix the legacy legs carried that tag; post-fix the abstention arm emits
  `Linking` rapid -> `EntryPlunge` -> the body cut, so door 1 sees the plunge and plans its
  own budgeted ladder over it. The entry at such a site is therefore "the budgeted ladder,
  or a plunge inside the budget" — not "a plunge". Arm `g` runs `apply_dressups` alone, so
  it measures the bare plunge; the board runs both doors.
- **An `entry_load` peak at 0.50 mm is the budget, not a defect.**
  `entry_bite_budget_mm(1.0) = clamp(1.0 x 0.5, 0.10, 0.50) = 0.50`, and `pencil.rs:1034-1041`
  says a budget too generous for a given pass is reported rather than hidden.
- **The `None` arm's air rapid is floored at `end.z + PLUNGE_CLEARANCE_MM`.** The
  conservative ceiling can read BELOW the target where the upstream tool already cut that
  column past it. Without the floor the rapid is skipped and the whole descent from safe Z
  is fed through air.
- **The peer's `tier-overlap` work is in the same checkout** (`rest_heatmap_mesh.rs`,
  `session/multitool.rs`, `tier_islands.rs`, `tier_map_cache.rs`, `viz/app/mcp.rs`,
  `viz/ui/multitool_planner.rs`, `tests/tier_band_overlap_g_overlapfill.rs`). Those files
  compiled and linted through every gate above and came up clean. Nothing of theirs was
  edited or staged.

## Measured — wanaka T3 @ 0.5 mm (the fast loop)
All three arms carry G-ISOCLIPENTRY; the third adds G-ISOCLIPRAPID and G-ISOCLIPRAMPFALL.

| | T3 master | + rapid fix | + rampfall fix |
|---|---|---|---|
| Time | 10016 s | 10017 s | **9669 s** (-3.5%) |
| Cutting | 67544 mm | 67546 mm | 67123 mm |
| Rapid | 47948 mm | 47931 mm | **46571 mm** (-1360 mm) |
| tier-1 moves | 64686 | 64697 | 64457 |
| collisions | 0 | 0 | 0 |
| entry_load over-bar | 995 | 995 | 1007 |
| entry_load peak | 0.50 mm | 0.50 mm | 0.50 mm |

The fix is CHEAPER, not dearer: a 38 mm blind zigzag is replaced by a short plunge or a
budgeted ladder. The 0.50 mm peak is the budget itself and does not move — the class this
fix removes only shows at 0.2 mm, where the 0.5 mm grid cannot resolve the standing material
the legs cross.

## Measured — wanaka T2 @ 0.2 mm, the ramp-fall arm (1:22:37 wall, exit 0)

| | T2 before (neither fix) | T2 + rapid fix | T2 + both fixes |
|---|---|---|---|
| rapid collisions, #18 | **1** | 0 | **0** |
| entry_load peak | 1.44 mm CRITICAL | 1.39 mm CRITICAL | **0.50 mm Caution** |
| entry_load over-bar | 17538 / 484325 | 5504 / 259770 | **4498 / 230427** |
| entry samples over 1.0 mm (from the trace) | - | 80 | **0** |
| lateral sub-stock rapids in the .nc | 3 | 0 | 0 |
| rapids at emitted Z 2.0 | 3292 | 425 | 18 |
| Time | 25580 s | 19691 s | **18356 s** |
| Cutting | 120992 mm | 103048 mm | 99451 mm |
| Rapid | 164906 mm | 139129 mm | 134504 mm |
| tier-1 moves | 157877 | 138917 | 136599 |

The residual is closed: the peak falls to 0.50 mm, which IS
`entry_bite_budget_mm(1.0) = clamp(1.0 x 0.5, 0.10, 0.50)`, so the finding now reports the
budget rather than a gouge, and drops from Critical to Caution
(`ENTRY_LOAD_SEVERE_PEAK_MM = 1.0`). The trace confirms it independently: **no entry sample
anywhere in the program exceeds 1.0 mm**, against 80 of them before.

## Ledger text (G-ISOCLIPRAMPFALL)
`emit_ramp`'s rest-driven arm has two abstentions — the ladder window leaves the mesh
footprint, or `plan_entry_ramp` finds nothing over that window the bite budget does not
already cover — and both used to fall through to the legacy two-leg zigzag. Those legs run
`ENTRY_CLEARANCE / tan(ramp_angle)` mm, 38 mm at the shipped 3 degrees against a 1.18 mm
ladder window, and they are clipped to the MODEL floor only, which on a rest-driven pass
lies below the material; the return leg therefore walked back across standing rest stock at
full depth, and `optimize_entry_descents` then declined to re-plan it because the legs carry
the `EntryRamp` tag its pencil guard skips. On the wanaka200 tier-1 islands at 0.2 mm that
was 1.394 mm of axial engagement on move 2796, 5.6 mm from an entry column the planner had
correctly read as clear. The fix makes the emitter ladder-or-plunge wherever a rest stock is
in scope: the far point's floor falls back to the entry depth when the probe has no answer,
and an abstaining planner now rapids to the conservative ceiling and plunges at the entry
column, which is the policy `OffMeshEntry::PlungeFallback` already states. Arm `g` of
`isoclip_entry_ramp_g_isoclipentry` is the sentry — a rest step 4 mm along the cut direction
that the planner's own window cannot see — with a new chord-sampling measure, because the
existing target-only measure reads 0.000 on a straight leg that gouges between its
endpoints. Pre-fix 1.309 mm against a 1.000 mm bound; post-fix the wanaka entry peak falls
1.39 -> 0.50 mm, Critical to Caution, and the whole pass gets 6.8% faster.
