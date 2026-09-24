# Stock-aware entries for the adaptive3d rough (2026-09-24)

Operator ruling 2026-09-24: build both options. Evidence and cites:
`planning/adaptive3d_step_ladder_roughing_2026-09-24/VALLEY_ENTRY_PROBE.md`.

## What the change reuses

- `Adaptive3dSegment::RapidWithFloor` and its emitter arms in
  `adaptive3d/path.rs`. AgentSearch already uses them. No new segment kind.
- `TriDexelStock::max_conservative_top_z_in_disc`, the sliver-safe and
  radius-aware read that the descent optimiser already uses (A/M10).
- The F-038b keep-down link `try_emit_stay_down_link` and the ONE dial
  `max_stay_down_distance_mm` (unset: 8 x D). No second name.
- The planner `Link` segment and `is_clear_path_3d` for a link at depth.
- `relink_fragments` is NOT used. It reads a post-generation stock
  snapshot, and this decision needs the planner stock at the entry moment.

## Option 1: the rapid stops at the real material

1. One planner helper reads the floor for every entry: the conservative
   stock top over a disc of R (plus the helix radius for helix style), read
   BEFORE the entry is stamped. Every contour-parallel, adaptive, cleanup
   and waterline entry then becomes `RapidWithFloor`. The AgentSearch sites
   change from the single-cell read to the same helper (one mechanism).
2. The `RapidWithFloor` emitter arm gets the `drape_point` gouge guard that
   the plain `Rapid` arm has, and the planner mirror gets it too.
3. `session/compute.rs` `optimize_entry_descents_annotated` is inspected,
   not changed. It splits a rapid only when its own seed-stock target is
   0.5 mm below the rapid end, so it cannot lower a rapid below the planner
   floor. A test holds this (split count 0 on the new output).

## Option 2: keep-down with a stock corridor

The planner attaches a stock proof to the entry: the highest conservative
stock top along the swept disc (R) from the previous tool position to the
entry. The emitter link runs for ALL entry styles, and only with a proof.
A plain `Rapid` (no stock known) never gets a keep-down link.

Corridor rule:

- `link_z = max(surface + leave, from.z, to.z, corridor stock top + clearance)`.
  Thus the traverse cuts no stock above `link_z` on the whole footprint.
- Refuse when `link_z > safe_z`, or above `to.z + cutting length`.
- The descent at the entry goes through air down to the entry floor. Below
  the floor, the entry style emitter (peck, helix, ramp) takes the rest,
  from the link height instead of from safe Z. A link never feeds
  straight down into stock.
- Side entry at depth: the existing planner `Link` (`is_clear_path_3d`)
  now also serves the contour-parallel cleanup runs. A run starts at its end
  nearer to the tool. One gate (`side_link_ok`) serves every link site.
- A planner `Link` reads the whole footprint (`link_footprint_ok`): a level
  link bites at most one Depth/Pass; a link that goes down bites nothing.
  The emitter drapes the link to hold the leave; the mirror does the same.
- Tried and removed (see RESULTS): the ring start nearest to the tool and
  the ring order front by front.

## Known defect, not changed here

`dressup::emit_helix` helixes only the last `ENTRY_CLEARANCE` (2 mm) and
feeds a straight plunge above that. The 2.5D dressup door shares it. The
fix changes every pocket fixture, so it needs an operator ruling. This
package measures the depth and reports it.

## Tests

- New sentry `adaptive3d_entry_stock_aware`: flat plate, flat end mill,
  dpp 8, plunge style with keep-down (the probe's 8 mm case), and helix
  and ramp styles. It replays the emitted path on a dexel stock and asserts:
  no rapid enters stock; no fed vertical descent enters stock deeper than
  the style allows (plunge: one peck = dpp; keep-down link: 0 + noise);
  the removed volume matches the old emission.
- The adaptive3d folder sentries, F-038b, F-038, the dressup entry and
  rapid sentries, `adaptive3d_commanded_ladder`, and
  `adaptive3d_emission_byte_parity` (re-blessed, each case named).

## Measure

`rough-score` on the rivmap100 demo, dpp 2, 4, 8, By Area, helix (and
plunge for dpp 8), before and after.

## RESULTS (2026-09-24)

Instrument: `rough-score demo.toml --toolpath 1 --resolution 0.5`, with the
rivmap100 demo copy (no `coarse_steps`), Contour Parallel, By Area, helix
(radius 0.3 x 6, pitch 1), plunge rate 500. Before is master `5ca7fedf`
(the same numbers as `ee478792`). Times in s. "Vol" is the simulation's
removed volume (mm3).

| Arm | Total | Entry | Link | Rapid | Moves | Entries | Retracts | Vol |
|---|---|---|---|---|---|---|---|---|
| dpp 2 helix, before | 1139.3 | 398.3 | 5.5 | 199.1 | 8905 | 219 | 215 | 49088.5 |
| dpp 2 helix, after | 983.2 | 255.1 | 103.6 | 90.3 | 7991 | 181 | 56 | 49088.9 |
| dpp 4 helix, before | 706.6 | 244.0 | 1.7 | 126.1 | 5780 | 142 | 138 | 47442.0 |
| dpp 4 helix, after | 618.9 | 159.9 | 60.6 | 65.1 | 5196 | 115 | 35 | 47443.6 |
| dpp 8 helix, before | 480.8 | 159.0 | 0.5 | 76.0 | 3922 | 84 | 80 | 47007.4 |
| dpp 8 helix, after | 430.5 | 105.0 | 30.8 | 50.2 | 3577 | 71 | 22 | 47012.7 |
| dpp 8 plunge, before | 361.6 | 29.1 | 54.2 | 33.7 | 2629 | 35 | 35 | 46764.4 |
| dpp 8 plunge, after | 370.1 | 43.7 | 33.4 | 48.5 | 2653 | 71 | 26 | 46755.6 |

- Helix: −13.7 % (dpp 2), −12.4 % (dpp 4), −10.5 % (dpp 8). Peak DOC is
  unchanged (2, 4, 8). Rapid collisions 0 and collisions 0 on every arm.
- Plunge dpp 8 is 2.4 % SLOWER. The old keep-down link fed straight down
  into uncut stock (the probe's 8 mm plunges); the new link stops at the
  stock floor and the peck ladder takes the material. That time is the
  price of the safety fix.
- The probe's 411 s target is not met (430.5 s). The rest of the lever is
  the ring order: see "Tried and removed".
- `entry_load` and `plunge_class_load` are absent on every arm, before and
  after: they need rest context, and this rough has none. The only verdict
  is `air_cut`.
- Finished stock, dpp 8 helix: G-code replay of before and after on one
  flat-end-mill heightmap (0.25 mm) gives a net +12.4 mm3 left (0.03 %).
  71 cells differ by more than 1 mm (max 1.87 mm, near the stock edge):
  the old helix and peck columns cut there by chance. No rapid enters stock.

Tried and removed. Two ring-order changes gave more time but moved the
material that each ring meets. On AS013 (`adaptive3d_interior_cell_parity_f029`,
F-031) the steady-state axial engagement went above Depth/Pass + 1:

| Change | dpp 8 helix total | F-031 max axial (bar 4.0) |
|---|---|---|
| ring start nearest to the tool | – | 4.63 mm |
| plus ring order front by front | 383.8 s | 6.47 mm |

The rings of one threshold alternate between fronts (an outer loop and an
island loop, 11 to 50 mm apart), so each ring needs an entry. A front
order needs a ring drape that bounds the bite on a flank first.

Safety tests (all pass): `adaptive3d_entry_stock_aware` (new; it fails on
master with a `Linking` feed 8.0 mm straight into stock), the five folder
sentries, F-038b (bar 1 now counts retracts), F-038,
`adaptive3d_commanded_ladder`, the seven dressup sentries, the four
`rapid_*` sentries, the two heavy sentries (F-029/F-031, F-027), the core
lib `adaptive3d`, `dressup` and `compute::` tests.

Open for a ruling: helix and ramp entries feed a straight plunge above the
last 2 mm (6.0 mm of 8 on the plate fixture). The sentry holds the peck
bound for them, not the helix pitch or the ramp angle.

`session/compute.rs` (`optimize_entry_descents_annotated`) is inspected and
not changed. Its split target is the seed conservative top + 2, which is at
or above the planner floor + 0.5 where the rapid now stops, so it splits
nothing (`entry_optimiser_leaves_the_planner_entries_alone`).

## RESULTS 2 — full-depth helix and ramp (operator ruling 2026-09-24)

Ruling: a helix or ramp takes the full material depth. The tool rapids to
the clearance above the real local material top, then helixes (at the
pitch) or ramps (at the angle) down to the target. A straight feed goes
only through air.

What changed:

- `dressup/entry_descent.rs`: `emit_helix` and `emit_ramp` share
  `rapid_to_entry_top`. It rapids to `stock_top + ENTRY_CLEARANCE` and the
  helix or ramp takes everything below. With no material above the target,
  it rapids to the clearance and feeds straight down through air.
- The helix descends at most `pitch / 36` per 10-degree step, also after a
  G-RAMPTERRAIN lift. Where the uphill floor holds the circle above the
  target, it spirals in to the centre at the helix slope.
- The ramp zigzag keeps the old leg length (`ENTRY_CLEARANCE / tan / 2`)
  and laps more legs for a deeper descent.
- `dressup/mod.rs` `apply_entry` (the 2.5D door: pocket, adaptive 2D and
  every op with an entry dressup): `EntrySafety::own_stock` replays the
  op's own emitted moves on its prior stock (or a prism at the stock top)
  with the real cutter, and each entry reads its material top there.
- `dressup/condition.rs`: segment merge keeps every point of an entry
  helix or ramp run. RDP moved Z within 0.3 mm and made a folded ramp leg
  2.7 % steeper than the angle.
- adaptive3d: the helix and ramp read the planner floor as the material
  top, or the op's stock top for an entry off the planner grid.

rough-score, rivmap100 demo, By Area, helix (radius 1.8, pitch 1, plunge
rate 500). Before is master `dec02006`.

| Arm | Total s | Entry s | Moves | Entries | Vol mm3 |
|---|---|---|---|---|---|
| dpp 2 helix | 983.2 → 1773.0 (+80 %) | 255.1 → 1032.6 | 7991 → 13001 | 181 → 207 | 49088.9 → 49048.2 |
| dpp 4 helix | 618.9 → 1254.7 (+103 %) | 159.9 → 779.8 | 5196 → 9434 | 115 → 142 | 47443.6 → 47424.6 |
| dpp 8 helix | 430.5 → 1113.6 (+159 %) | 105.0 → 769.0 | 3577 → 7937 | 71 → 98 | 47012.7 → 46975.4 |
| dpp 8 plunge | 370.1 → 370.1 | 43.7 → 43.7 | 2653 → 2653 | 71 → 71 | 46755.6 → 46755.6 |

- The helix arms are much slower. A closed entry now helixes its whole
  material depth at 1 mm per turn of 11.3 mm at 500 mm/min (about 1.4 s per
  mm of depth). Plunge style does not change.
- Rapid collisions 0 and collisions 0 on every arm.
- A new verdict on the helix arms: `measurability_abstained` (60 % of the
  removing samples read zero engagement at the 0.5 mm cell). The air-cut %
  is withheld; it is a statement about the simulation cell, not a defect.
- Finished stock, dpp 8 helix, G-code replay at 0.25 mm: +60.9 mm3 left
  (0.13 %), 36 cells > 1 mm (max 1.40 mm, at the stock edge), 0 rapid hits.

Sentry `adaptive3d_entry_stock_aware` now holds the ruling's bounds on the
plate (all styles) and on a new 2.5D pocket fixture (helix, ramp): helix
slope ≤ pitch / (2 pi r), ramp slope ≤ tan(angle), and no straight feed
into material except a plunge-style peck. OPEN, reported and not asserted:
on the dome, where the surface is steeper than the entry slope, the
G-RAMPTERRAIN clip still makes steep entry drops (helix: 16 moves, deepest
3.08 mm; ramp: 62 moves, deepest 2.81 mm). The ramp clip does not
rate-limit; the helix does, but cannot reach a target that the uphill
floor holds above its circle.

Re-pinned (geometry intentionally moves):
- `transform_provenance_fingerprints`: all pins (three_pass, arc_raster,
  face stages 1-3 and link sites). The face chain loses its six descent
  splits, because its entries at the stock top are straight feeds through
  air now.
- `arcfit_intent_key_cost_f1`: three_pass, arc_raster, face_full. The F1 Q3
  quantity (`unknown_strict`) is unchanged on every fixture.
- `entry_moves_stock_aware_g_rampterrain`: the unclipped ramp keeps its
  planned zigzag, `2 * ceil(depth / ENTRY_CLEARANCE)` legs, not two.

Not re-blessed: `adaptive3d_emission_byte_parity` (its cases use plunge
style) passes unchanged. Pre-existing on master: 
`isoclip_entry_ramp_g_isoclipentry::c_the_ramp_hands_the_tool_back_where_the_plunge_would_have`
fails on master `dec02006` too (not touched here).

## RESULTS 3 — ramp feed, entry clearance, never helix air (rulings 2026-09-25)

What changed:

- `ramp_feed_rate: Option<f64>` (mm/min, `#[serde(default)]`) on all 22
  operation configs that have `plunge_rate` (all but the two drill
  configs); `OperationParams::ramp_feed_rate`; one catalog row per op;
  CLI job key `ramp_feed_rate`. `None` keeps the feed of before. The helix
  and ramp moves through material run at it; a straight feed through air
  and a straight peck keep their own feeds. The feeds session owns the
  value.
- `entry_clearance_mm` (default 0.5) on `Adaptive3dConfig` and on the
  dressup (`DressupConfig`, MCP `set_dressup_config`, GUI row beside the
  helix and ramp fields). The helix or ramp starts at the material top +
  this value. The rapid floor does not move (a measured top: + 0.5; a
  nominal top: + 2); the air between is a straight feed. No helix or ramp
  moves through air.
- The helix and ramp start above the cell-centre stock read. The
  conservative (sliver-safe) read still sets the rapid floor: it holds a
  cut-edge cell at the height above the cut and started entries up to one
  level high. The 2.5D door reads over the tool disc, not the helix circle
  (the circle reaches the part wall beside the entry).
- Sentry: no helix or ramp move starts more than the clearance (+ 0.3 mm
  replay tolerance) above the material, on the plate and the pocket. Unit
  tests: the ramp feed is used when set and the plunge feed when not; the
  helix and ramp start at the material top + the clearance.

What-ifs, rough-score, rivmap100 demo, By Area, helix (radius factor 0.3,
pitch 1, plunge 500, feed 2400). (a) `ramp_feed_rate` None, (b) 2400, (c)
(b) + `helix_pitch` 2, (d) (b) + `helix_radius_factor` 0.45.

| dpp | (a) total / entry s | (b) | (c) | (d) |
|---|---|---|---|---|
| 2 | 1676.0 / 925.3 | 1280.8 / 530.6 | 1125.7 / 383.2 | 1560.7 / 796.7 |
| 4 | 1199.7 / 720.0 | 894.5 / 414.8 | 730.4 / 259.8 | 1088.1 / 603.8 |
| 8 | 1090.3 / 743.9 | 733.7 / 387.3 | 579.9 / 242.4 | 869.2 / 517.2 |

- (a) against the full-depth helix before this change (1773 / 1255 /
  1114 s): the 0.5 mm start saves 97 / 55 / 24 s.
- (c) is the closest to the pre-ruling 431 s at dpp 8 (580 s, +35 %).
- A 2400 mm/min helix is only about twice as fast as 500: its 10-degree
  steps (0.31 mm) are accel-bound in the time integrator.
- A larger helix radius (d) is slower: longer turns at the same pitch.
- Rapid collisions 0 on every arm. `entry_load` is absent on every arm (it
  needs rest context); the only verdict is `measurability_abstained`.

Re-pinned again (the start moved from + 2 to + 0.5 mm):
`transform_provenance_fingerprints` (all; the face chain reads 80 / 110 /
116 moves and six splits again) and `arcfit_intent_key_cost_f1`
(three_pass (27, 4), arc_raster (72, 21), face_full (80, 13) as on
master). `entry_moves_stock_aware_g_rampterrain` reads its depth from the
clearance. The MCP wire snapshot did not change.

OPEN (by geometry, not measured): a 2.5D helix circle at a ring start
reaches the helix radius past the tool into the part wall; the
entry has no containment test for the helix (the ramp has G-RAMPCONTAIN).
