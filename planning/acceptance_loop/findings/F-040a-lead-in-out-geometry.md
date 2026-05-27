# F-040a — Lead-in / lead-out geometry: classic shape

- **Stage:** dressup geometry
- **Severity:** medium (operator-visible behaviour; F-040 shipped the feeds, but the geometry was already wrong pre-F-040)
- **Status:** landed 2026-05-27
- **First observed in:** real-machine bench 2026-05-27 — user noted variant 6 "looks like plunge/retract, NOT classic lead in/out" (audible feed change confirmed F-040 itself worked).
- **Effort:** S (one block in `apply_lead_in_out_with_feeds`)
- **Workstream:** Feed Modulation (F-040 follow-up)
- **Depends on:** F-040 landed

## Symptom

Watching the bench machine run variant 6 (small profile with
`lead_in_feed_rate = 500`, `lead_out_feed_rate = 4500`), the lead-in
visibly behaved as a slanted plunge from the rapid endpoint straight to
cutting depth at the lead-in start XY, then arc into the cut. F-040's
feed-rate breakout worked (audible F500), but the **geometry** wasn't
the classic tangent entry an operator expects.

## Hypothesised root cause (confirmed)

Pre-F-040 `apply_lead_in_out` detected a plunge move and replaced it
with:

1. `result.feed_to(lead_start, feed_rate)` — feed from current position
   to `lead_start`. Current position was the preceding rapid's endpoint
   `(cut_start.xy, safe_z)`, so this became a **diagonal feed** to
   `(lead_start.xy, cut_z)` — not a tangent entry.
2. Arc steps from `lead_start` to `plunge_end`.

F-040 inherited this and only changed the feed rates. The user's bench
exposed the geometry bug F-040 didn't touch.

Classic lead-in (Fusion HSM / Mastercam) is:

1. Rapid horizontal at safe-Z to the lead-in start XY.
2. Pure-Z plunge straight down to cut depth at plunge feed.
3. Tangent arc into the cut at lead-in feed.

## Fix shape

In `apply_lead_in_out_with_feeds` (`crates/rs_cam_core/src/dressup.rs`),
read `safe_z` from `moves[i - 1].target.z` (the preceding rapid's Z),
then emit:

1. `rapid_to_with_intent((lead_start.xy, safe_z), MoveIntent::LeadIn)` —
   pre-position above the lead-in start.
2. `feed_to_with_intent(lead_start, plunge_rate, MoveIntent::EntryPlunge)` —
   pure-Z descent at plunge feed.
3. Arc steps at `lead_in_feed_rate.unwrap_or(plunge_rate)` tagged
   `MoveIntent::LeadIn` (unchanged from F-040).

Net effect on move count: +1 per lead-in (the new pre-position rapid).

## Acceptance test

`crates/rs_cam_core/tests/lead_in_out_feed_rates_f040.rs` adds
`lead_in_geometry_is_pre_position_then_pure_z_plunge_then_arc`. Asserts
that the first LeadIn-tagged move is a Rapid, its successor is a Linear
move with the SAME XY (pure-Z descent), and subsequent LeadIn-tagged
moves are Linear arcs at cut Z.

Other F-040 tests updated to reflect that the LeadIn set now contains
both the pre-position Rapid and the arc Linear moves (filter to Linear
when asserting feed rate).

## Files

- `crates/rs_cam_core/src/dressup.rs` — geometry rewrite in
  `apply_lead_in_out_with_feeds`
- `crates/rs_cam_core/tests/lead_in_out_feed_rates_f040.rs` — new
  geometry test; existing 4 tests updated to filter LeadIn → Linear

## Risk

S. Behaviour change for any project with `lead_in_out = true` — one
extra Rapid per lead-in pass. F-037 smoke baseline diff clean (no
smoke case has `lead_in_out = true`). F-040's existing feed-rate test
suite passes (with the filter update). No bench session needed —
verifiable via toolpath trace inspection.

## Validation

- 5/5 tests in `lead_in_out_feed_rates_f040.rs` pass.
- Workspace clippy clean.
- F-037 smoke diff: **no regressions** (18 cases byte-identical).
- Variant 6 regenerated in the bench bundle — G-code header reads:
  ```
  G0 X52.817 Y3.967 Z17.000   ; op's initial rapid to cut-start XY
  G0 X50.250 Y2.778 Z17.000   ; F-040a: pre-position over lead_start
  G1 X50.250 Y2.778 Z-2.000 F400   ; pure-Z plunge at plunge feed
  G1 X51.292 Y4.485 Z-2.000 F500   ; arc step at lead-in feed
  ; ... 7 more arc steps at F500 ...
  G1 X63.095 Y32.000 Z-2.000 F1800 ; first cut
  ```
- CAMotics 1.2.0 GRBL-parse: clean.

## Notes

- F-040a's geometry change is what an operator EXPECTS when they enable
  lead_in_out. Pre-F-040a behaviour was already broken; F-040 just made
  the bug audible by changing the feed.
- One could argue this should have been the F-040 PR. It wasn't — F-040
  scope was strictly feed-rate breakout (per the design doc). The bench
  surfaced the geometry bug as a real-world observation.
