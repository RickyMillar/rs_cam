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
- Ring starts: among the ring points with the lowest stock top, the planner
  takes the one nearest to the previous tool position. Then the existing
  planner `Link` at depth (side entry) fires more often.
- A planner `Link` that descends more than the dexel noise must have an
  air corridor on the whole footprint.

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
