# One shared surface-link stage for finishing fragments

Date: 2026-09-09. Status: SPEC. Code reads only, no crate edit, no cargo
run. Branch `isoclip-rapid`.

Problem, measured today: a finishing pass confined to the multi-tool
planner's islands is RETRACT-COUNT-BOUND (study doc
`planning/deep_doc_modulation_2026-09-08.md` §2.7a; island analysis
`planning/island_clip_2026-09-09/SPEC.md`). Whole-board R1.0 raster (T1)
= 76 retracts, 4 317 s fed. The same raster on the islands (T5) = 954
row fragments, 954 retracts, 4 910 s. Contour scallop on the islands
(T3b, `continuous:false`, hookup 3) = 600 rings, 613 retracts; hookup 6
(T3c) joined 46 more. The lever is a link between two cutting fragments.
The repo already has a good kernel. It is wired to three ops, and on the
op that needs it most it is configured wrong.

## 1. Inventory — every place a link is decided today

| # | Site | file:line | Knows surface? | Knows stock? | Knows boundary? | Costed? | Reorders? | Rotates a loop? | Used by |
|---|---|---|---|---|---|---|---|---|---|
| A | `relink_fragments` (the shared kernel) | `surface_link.rs:354` | yes, drop-cutter sampled | optional `LinkCeiling` | optional `RegionSet` | optional F-034 | optional | no | B, C, D |
| B | Scallop intra-pass relink | `scallop.rs:2543-2578` | yes | **no** (`link_ceiling: None`) | yes, no airborne waiver | yes | **no** | no | scallop, iso-scallop |
| C | Unified-finish intra-region relink | `unified_finish.rs:2306-2352` | yes | yes, from `ctx.initial_stock` (`execute.rs:2395-2404`) | yes, airborne waiver on | yes | yes | no | unified finish |
| D | Project-curve chaining | `execute.rs:1973-2016` | yes | yes | yes, no waiver | yes | yes | no | project_curve |
| E | Unified-finish region router `choose_link` | `unified_finish.rs:3039-3115` | yes | no | yes | yes | route only | no | unified finish |
| F | Pencil emit-time link | `pencil.rs:1631-1700` | yes | yes, `plan_link_lift` | no | yes | no | no | pencil |
| G | Scallop `continuous` spiral connector | `scallop.rs:2331-2372` | ring points only | no | keep predicate only | no, a distance bar | no | yes, `closest_kept_point_idx` | scallop, iso-scallop |
| H | Raster serpentine hookup | `toolpath.rs:636-700` | grid points only | no | run split only | no, one cell diagonal | no | no | drop_cutter, all raster bands |
| I | adaptive3d stay-down link | `operation_configs.rs:660-675` | mesh max Z + clearance | no | no | no, 8×diameter bar | no | no | adaptive3d |
| J | Dressup `apply_link_moves` | `dressup.rs:2051`, corridor test `dressup.rs:1948-1995` | no mesh; swept-corridor test on emitted moves | no | no | no, `link_max_distance` | no | no | any op with the dressup on |
| K | Post-generation boundary clip | `boundary.rs:332-410` | no | no | yes, it IS the boundary | no | no | no | any op with `boundary.enabled` |
| L | TSP rapid reorder | `tsp.rs:116-139` | no | no | no | distance only | yes, whole segments | no | any op with `optimize_rapid_order` |

Three readings.

- Row A is already the shared stage this spec asks for. The gap is the
  call sites, not the kernel.
- Rows G, H and I are per-generator linkers with a distance bar and no
  gouge test. Keep them. They must not be an op's only linker.
- Row H is the raster's whole answer today. `link_max` is
  `hypot(x_step, y_step) * 1.05` (`toolpath.rs:636`). Two runs split by
  an excluded cell are two steps apart, so nothing inside a row links.
  That is why T5 reads 954 retracts for 953 fragments.

## 2. Why the scallop relink joins so few rings

The refusal is NOT the distance dial, NOT the boundary veto and NOT the
kinematics cost. It is the CANDIDATE SET. Two causes, both about order
and start point.

**Cause 1 — the ring list is breadth-first by offset level.**
`generate_scallop_rings_with_cancel` offsets every current polygon and
pushes every resulting polygon before it advances the cascade
(`scallop.rs:1500-1535`). The wanaka tier-1 island carries 402 holes
after the 1.25 mm dilation, so one offset level holds many disjoint
loops. Two neighbouring entries in `rings` are then usually DIFFERENT
loops at the same level, tens of mm apart — not radially adjacent rings
1.03 mm apart. The scallop passes `reorder: false`
(`scallop.rs:2557-2560`), so `relink_fragments` only tries
`prev_exit → next entry` in that order. The gap exceeds the cap, and the
counter that rises is `too_far`.

**Cause 2 — the discrete branch never rotates a ring.** Each emitted run
starts at `points.first()` (`scallop.rs:2481-2497`), the offset
library's own start vertex. The rotation toward the tool
(`closest_kept_point_idx` + `rotate_ring`, `scallop.rs:2379-2382`) runs
in the `continuous` branch ONLY. So even two radially adjacent rings
meet at unrelated points on their circumference.

A cap of 3 → 6 mm samples a slightly larger radius of a distribution
whose mass sits at ring-circumference scale. 46 links out of 600 is the
expected yield, and it is evidence FOR this diagnosis.

The code rules out the other three candidates.

- The kinematics test is biased IN FAVOUR of the link.
  `retract_link_time` runs with `descend_rapid_to: None`
  (`machine_kinematics.rs:660-684`), so it prices the whole descent from
  `safe_z` at the plunge rate. `slower_than_retract` must be small.
- The `from.z >= safe_z` arm (`surface_link.rs:496-500`) cannot fire.
  Every discrete run ends on a fed move at cut depth.
- `outside_boundary` is real but secondary. A ring that splits into
  several runs splits AT the excluded gap, so that chord leaves the
  region by construction. That is a run-to-run effect, not the
  ring-to-ring majority.

Settle one contradiction when the fix lands. The scallop's
`reorder: false` comment cites climb/conventional churn
(`scallop.rs:2555-2559`); `RelinkParams::reorder`'s own doc says the
pass is forward-only and preserves cut direction
(`surface_link.rs:207-211`). UNVERIFIED: whether `offset_polygon` and
`cleanup.apply` preserve start-vertex correspondence between two
offsets. It stops mattering once the stage rotates loops.

**Zero-code reproduction.** `scallop.rs:2580-2594` already logs the
whole `RelinkReport` at `info!` with the fields `too_far`,
`off_surface`, `slower_than_retract`, `outside_boundary` and
`surface_links`. The GUI installs an `INFO`-default subscriber on stderr
(`rs_cam_viz/src/bin/main.rs:36, 41-45`, filter at `:58-66`), so a T3b
or T3c generate ALREADY prints the counter breakdown. `.mcp.json`
launches the GUI, so that stderr goes to the MCP host: read the host's
`rs-cam` server log, or launch the GUI by hand with `2> log` for this
run. UNVERIFIED: that the hand-built layer admits
`rs_cam_core::scallop` at INFO on the `--mcp` path. Do not run it here.

## 3. Design — one shared surface-link stage

Keep `surface_link::relink_fragments` as the kernel. Add three things.

**3.1 One params builder in `execute.rs`.** The unified-finish
configuration (`unified_finish.rs:2306-2343`) is the correct finishing
one: `reorder: true`, `link_ceiling` from `ctx.initial_stock`,
`flush_ride: true`, `airborne_links_may_leave_territory: true`,
`boundary: ctx.boundary_regions`, `link_kinematics: ctx.link_kinematics`.
Hoist that construction into one helper beside `chain_project_curve`
(`execute.rs:1941`). Every finishing adapter then calls the helper with
its own hookup dial and its own fragment kinds. The scallop's
`link_ceiling: None` is the defect this removes: on a
`FromRemainingStock` island pass the scallop can never take a lifted
finger-to-finger hop, which is exactly the 17 083-retract case the field
doc records (`surface_link.rs:262-273`).

`scallop.rs` holds no `ctx.initial_stock`, so the ceiling has to be
threaded. The precedent is in the tree:
`unified_finish_toolpath_with_cancel_and_ceiling` takes `link_ceiling`
as a parameter from its adapter (`execute.rs:2405-2419`). DECIDE and
record which of the two shapes the scallop takes. Run the stage inside
the generator, and thread the ceiling into `ScallopParams` — that keeps
the annotation reconcile where it is (`scallop.rs:2596-2600`). Or run
the stage in the adapter, and expose the ring annotations as a
provenance channel. Do not leave both open.

**3.2 A fragment kind.** `relink_fragments` treats a fragment as an
opaque move run. Give each fragment a kind:

- `OpenRun` — ends are fixed, never reversed. Raster rows, pencil
  traces, split ring arcs.
- `ClosedLoop` — the stage may ROTATE the loop to start at the point
  nearest the previous exit, then close it there. Scallop rings,
  waterline loops, iso-field level sets.

Rotation is what makes `reorder: true` pay on rings, and the
`continuous` branch already performs it without a gouge test. Build the
kind in each generator (the ring annotations carry the ring index) and
pass it in `RelinkParams`. Note one C1 consequence: a rotation is a
permutation WITHIN a fragment, and `Transformed` carries two provenance
flavours only — an index-preserving remap and a fragment permutation
(`surface_link.rs:342-350`). Add a third flavour, or re-anchor the ring
annotation to the new start.

**3.3 A Minimum-retract fallback.** When the stage refuses a link, it
retracts to `max(clear_z)` over the hop instead of `safe_z`, where
`clear_z` is the value the lifted arm already computes per sample
(`surface_link.rs:634`). Three consequences.

- `retract_link_time` must price THAT retract, with
  `descend_rapid_to: Some(ceiling)` (`machine_kinematics.rs:674-678`).
  Otherwise the link-versus-retract comparison is biased.
- **The TSP will undo it unless the ceiling is threaded.**
  `internal_link_ceiling_z` returns `None` for every non-drill family
  (`execute.rs:3500-3502`), and with `None` every rapid frames a segment
  (`tsp.rs:122-126`); the reorder then discards those rapids and
  re-plants them at `safe_z`, which is the documented drill defect. The
  stage must make this function return `Some(ceiling)` for a toolpath
  that carries stage-emitted low retracts. A surface LINK is a feed and
  never frames a segment, so linking alone needs no change here.
- `retract_strategy` (`compute/config.rs:1874-1880`) is the natural
  selector. It is defined, defaulted `Full`, read only by the GUI panel
  (`ui/properties/mod.rs:5025, 5260-5267`) and the MCP description, and
  consumed by NO generator or dressup — the orchestrator measured a
  `minimum` rerun of T5 as byte-identical. Give the dial its meaning
  here: `Full` = today, `Minimum` = the ceiling-height retract. Do not
  retire it.

**3.4 Where it plugs in.** The stage runs at the END of generation, in
the generator or its `execute.rs` adapter, on the whole emitted path —
where rows B, C and D already run — and BEFORE the post-generation
boundary clip (`session/compute.rs:1673`), so the clip keeps the last
word on territory.

**3.5 Agreement with the boundary clip.** The two doors must see the
same polygons. The pre-generation door applies `processed()` only
(`session/compute.rs:1318-1322`); the post-clip additionally applies
`effective_boundary_reported(region, containment, tool_radius)`
(`session/compute.rs:2338`). For `Center` that call is the identity
(`boundary.rs:73`) and the fixtures use `containment = "center"`
(`T5_r10_raster_on_planner_islands.toml:1734-1736`), so the two agree
today. Under `Inside` the clip insets by the tool radius and chops both
ring ends and stage links the generator approved. Hand the stage the
SAME set the clip will apply, or refuse to link when containment is not
`Center`.

The polygon SET agrees under `Center`; the emitted POINTS can still
cross. T3b reads 613 retracts on 600 rings, and if the relink joined
almost nothing at 3 mm then some of those 13 came from the post-clip.
The island spec §2 names the candidate: arc fitting runs in the
dressups, and the dressups run before the clip
(`session/compute.rs:1666-1673`), so a ring point nudged by up to
`arc_tolerance` lands outside and pays retract + rapid + `EntryPlunge`.
The stage's links travel the same dressup-then-clip path and carry the
same exposure. Do not solve it here. Read `boundary_clip_dropped` and
the clip crossing count on T3b first (L6).

**3.6 What still has work to do.** `optimize_entry_descents`
(`dressup.rs:375`) stays as the safety net for every junction the stage
does not touch. The G-ISOCLIPENTRY ramp ladder (`dressup.rs:343-360`)
stays for the entries that remain: the stage removes junctions, it does
not soften the ones it keeps. Where the stage converts a retract into a
link, both passes see one fewer entry and do nothing.

**3.7 Byte-identity for ops that do not opt in.** Follow the
`chain_distance_mm` pattern: a `0.0` dial returns the input untouched
before the relinker is entered (`execute.rs:1950-1952`,
`operation_configs.rs:1586-1588`). Add `hookup_mm: 0.0` to
`DropCutterConfig`, `WaterlineConfig` and the iso-field arm. Leave the
scallop and unified-finish defaults at 3.0 and 6.0
(`operation_configs.rs:938-940, 1345-1347`).

**3.8 The T5 hole case.** A raster row is split by a hole that belongs
to the R2.0 tier. The mesh exists under the hole, so
`build_surface_link` succeeds. The surface-riding form is vetoed, because
the hole is outside the region set. With a ceiling from
`ctx.initial_stock`, the flush test compares the standing material
against the surface with `FLUSH_EPS_MM = 0.15`
(`surface_link.rs:607-611`), and the R2.0 cusp on this board is 0.146 mm
— borderline. If the hop reads flush, the veto re-runs and refuses the
link. If it does not, the lifted shape applies, the link is airborne,
the waiver applies and the cost decides. So the stage bridges an island
hole through the LIFTED arm only. The flush-epsilon versus cusp
collision is a risk row.

## 4. Cost model, and the island planner's hole rule

Take a junction cost `t_j` and a lateral speed `v`. At 604 mm/min,
`v = 10.07 mm/s`; the measured retract is about 2 s, so a retract buys
about 20 mm of cutting.

**The link stage's rule stays what the kernel already does**: keep the
link when `surface_link_time <= retract_link_time`
(`surface_link.rs:649-676`). Do not add a second bar.

**The island planner's hole rule is the same arithmetic.** Let a hole
have area `A` and extent `h` measured NORMAL to the pass direction. It
creates about `h / s` junctions and it saves about `(A / s) / v` seconds
of cutting. Fill the hole (machine through it) when `A < h · v · t_j` —
that is, when the hole's MEAN CHORD along the pass direction, `A / h`,
is under `v · t_j` (about 20 mm at 604 mm/min and a 2 s junction). For a
round hole the break-even diameter is `4·v·t_j/π ≈ 25 mm`, so about
500 mm², not 40 mm². The operator's ~40 mm² is the PER-JUNCTION form of
the same rule (`A* = t_j · v · s ≈ 20 mm²` at `s = 1.0`), correct for a
hole that breaks a single pass. Keep both; the planner needs the
per-hole form. UNVERIFIED: which `t_j` produced the operator's 40.

The two rules agree on one line: the planner must take
`t_j = min(link_time, retract_time)` from the SAME
`machine_kinematics::surface_link_time` / `retract_link_time` pair the
stage uses. A hole the stage can bridge with a lifted link costs about
0.3 s, so the threshold falls to a ~3 mm mean chord and hole filling is
almost never worth it. That coupling is the point.

The slot is `tier_islands.rs`. It carries `min_region_area_mm2`
(`tier_islands.rs:140-143`, `:331-334`) for "is this island worth a
tier", and NO hole rule at all. Add the hole rule beside it, with the
same derived/explicit dial shape.

## 5. Experiments the operator can run with no code

The orchestrator has `retract_strategy = "minimum"` reruns of T5 and
T3c in flight. Do not duplicate them. Expect them to be byte-identical,
because the dial is inert (§3.3).

| # | Run | Dial | Decides |
|---|---|---|---|
| L1 | T3b and T3c, capture the GUI stderr | none | §2 directly. Reads `too_far` vs `outside_boundary` vs `slower_than_retract` off the existing `info!` line. Do this FIRST. |
| L2 | T3c with `intra_pass_hookup_mm` 20 | one | The yield should stay flat as the cap grows. A flat curve confirms the candidate set is the fault, not the cap. |
| L3 | T3b with `direction = "inside_out"` | one | FALSIFIER only. `InsideOut` calls `rings.reverse()` (`scallop.rs:2296-2299`); a reversed breadth-first list is still breadth-first, so Cause 1 predicts ~no change. A large change falsifies Cause 1. |
| L4 | T5 with `stepover` 1.5 | one | Halves the row count on the same islands. It separates fragment COUNT from cut LENGTH in the pair time. |
| L5 | T3c at `islands.overlap_mm` 0.5 | one | Shrinks the machining set. It tests the G-OVERLAPFILL dial against the retract-count law. |
| L6 | T3b, read `boundary_clip_dropped` and the clip crossing count | none | §3.5. Says how many of T3b's 613 retracts the post-clip made, not the ring junctions. |

Read on every run: `retract_trips`, second-pass fed seconds, pair
total, `entry_load` severity and peak, `rapid_collision_count` at
0.2 mm, and `air_cut_time_s` in seconds, not percent.

## 6. Risks and sentries

Existing sentries. Do not break them.

| Test | Pins |
|---|---|
| `scallop_intra_pass_relink_am7.rs:252, 311` | the relink loses no cut position and the labels are honest |
| `island_stay_down_links_o3.rs:148, 219` | ceiling links clear standing stock; `link_ceiling: None` is byte-identical |
| `retract_trip_channel_am7.rs` | the hookup must never ADD retract trips |
| `pencil_surface_link_g_linkload.rs` | a link never takes a full-diameter bite |
| `project_curve_chaining.rs` | a link may not leave the machining boundary |
| `capability_link_moves_safety.rs` | the dressup bridge only crosses swept ground |
| `profile_link_ceiling.rs:246, 286, 351, 421` | the profile-aware ceiling, and flat-endmill byte-identity |
| `boundary_reentry_plunge_rate_g_boundaryplunge.rs` | a clip re-entry descends at the operation plunge rate |
| `isoclip_link_rapid_g_isocliprapid.rs` | a lead-in rapid repositions at the retract plane |

New sentries a shared stage needs.

1. Byte-identity at `hookup_mm = 0.0` for every newly opted-in adapter.
2. A hole-crossing link on a rest-driven raster is LIFTED, never
   surface-riding. That is the P0.4 gouge class.
3. The retract count is non-increasing when the stage is switched on, on
   a dendritic island fixture. Extend the `retract_trip_channel_am7`
   pattern to the raster and the scallop.
4. A rotated `ClosedLoop` fragment keeps every cut position and its cut
   direction.
5. `retract_strategy = Full` stays byte-identical after the dial gains
   meaning.
6. A stage-emitted low retract survives `optimize_rapid_order`, i.e.
   `internal_link_ceiling_z` reports it.

Risks.

- Row J (`apply_link_moves`) is a SECOND linker. It can fire after the
  stage on the same path, with a swept-corridor test and no mesh. Say
  whether it stays off for an opted-in op.
- The flush epsilon (0.15 mm) sits on top of the R2.0 cusp (0.146 mm).
  See §3.8.
- A containment other than `Center` splits the two boundary doors. See
  §3.5.
- Two of the three offset panic classes are `debug_assert!`s in a
  dependency. A rotation or offset change is not comparable across a
  debug and a release build.

## 7. L1 result (2026-09-09, headless CLI `project`, binary 09-06, resolution 0.5 — the relink counters do not depend on the stock cell)

`RUST_LOG=info rs_cam_cli project <T3b|T3c>.toml --setup "Setup 2 — front"`;
the counters go to STDOUT (the CLI's `tracing_subscriber::fmt()` default
writer), not stderr. One line per scallop tier:

| run | hookup mm | fragments | surface_links | retract_links | too_far | off_surface | slower_than_retract | outside_boundary |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| T3b | 3 | 600 | 89 | 510 | 493 | 0 | 0 | 17 |
| T3c | 6 | 600 | 124 | 475 | 451 | 0 | 0 | 24 |

Verdict on §2: CONFIRMED. `too_far` is 82 % of the junctions at 3 mm
and 75 % at 6 mm; the kinematics test refused nothing (`slower_than_retract`
0); the surface test refused nothing (`off_surface` 0); the boundary
refused 17 and 24. Doubling the cap moved 42 junctions out of `too_far`
and 35 of them became links. The candidate set (breadth-first ring
order, no reorder, no loop rotation) is the cause. The GUI's 613 and
567 retract trips are these 510 / 475 ring junctions plus the region
and clip transitions.

## 8. Pencil baseline (2026-09-09 evening, binary 15b407b7, 0.3 mm)

Fixture: `planning/deep_doc_modulation_2026-09-08/PENCIL_baseline_r10_rest.toml`
(a durable copy of the 09-04 throwaway, which lived only in a working
tree). Two arms, because the saved fixture is not a realistic pass.

> **READ THIS BEFORE RE-RUNNING (2026-09-10, G-FRESHLINK).** The saved
> fixture carries `stock_source = "fresh"` on the pencil, so it reproduces
> the **P0** arm and NOTHING ELSE. A fresh op awaits no prior stock, so it
> never blocks a fixpoint round and the reply still says `simulations: 1` —
> the G-STALESTOCK check passes and the arm is still wrong. It also gets
> `entry_stock: None`, so the clearance-hop tier CANNOT run and the link
> report prints `linked_via_hop: 0` as if it were a measured zero.
> For the P1 / correct chain use
> `planning/deep_doc_modulation_2026-09-08/PENCIL_chain_r10_rest.toml`,
> which is this file with that one line set to `from_remaining_stock`.
> Verified 2026-09-10: as saved → 9 275 junctions, 8 348 at depth, 0 hops,
> 30 841.9 s fed; correct chain → 1 969 / 735 / 993 and 4 415.2 s fed.

| arm | stock the pencil sees | total s | entry s | cutting s | linking s | rapid s | fragments | retracts | rapid collisions |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| P0 | FRESH block (the 09-04 setup as saved) | 32 426 | 29 114 (89.8 %) | 1 418 | 723 | 1 170 | 9 282 | 987 | **32** |
| P1 ⚠ CONTAMINATED | after an R1.5 iso-scallop on fresh stock | 2 736 | 2 307 (84.3 %) | 50 | 120 | 259 | 319 | 313 | 0 |

> ⚠ **The P1 row is NOT a valid baseline (2026-09-09, G-STALESTOCK).** Its
> `generate_all` reported `rounds: 1, simulations: 0` — no simulation ran
> during generation, so the pencil generated against the stale stock left
> by the P0 run, i.e. the pencil cutting its own earlier grooves. The
> correctly-simulated chain on the same project gives **1 969 junctions,
> 96 % tip float (5 257 of 5 498), 2 294 retracts**, against the
> contaminated 319 / 39 % / 313. The upstream iso-scallop is identical in
> both (171 334 moves, 100 556.6 mm), which is what proves the difference
> is the pencil's INPUT STOCK and not the chain. Read the `simulations`
> count in a `generate_all` reply before trusting any rest-driven
> measurement. The P0 row stands.

P0 reproduces the 09-04 reading (entry 29 114 against 28 986, cutting
1 418 against 1 418) on today's binary, so the emitter work of
2026-09-09 does not touch this cost. But P0 runs the pencil on a raw
block: every ramp descends about 9.5 mm from the block top, every rapid
crosses the block (32 collisions), and the tool floats over 96 % of its
own centreline. It is not a pass anyone would run.

P1 is the honest baseline. The absolute cost falls 11.9×, the float
falls to 39 %, and the rapid collisions go to zero — but **the shape
does not change: 84 % entry, 2 % cutting.** Per fragment: 7.2 s of
entry against 0.16 s of cutting. That is the number the link stage has
to move, and it is not an artifact of a bad fixture.

Prize at P1: removing the entry on four fifths of the fragments takes
the pass from 2 736 s to about 890 s, a little over 3×. The pass also
carries a `plunge_class_load` **critical** of its own — 10 of 322
vertical-dominant moves run up to 6.6× the op's plunge rate, the
untagged-descent class — which is a separate defect from linking and
should be ledgered on its own.
