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
