# P23d survey: wanaka idx 6, the lift on a slot-first ring (2026-11-10)

Read-only. No code changed, no cargo run (the lane was busy). Base `2d0f7849f`. Every statement
is read from the code or from `acceptance/wanaka_21.md`; a statement that I did not check by a run
says so.

## 0. The row

idx 6 "Lake pocket 2 mm" (contour Pocket, 2 mm 1-flute flat, stepover 0.8, constant chip 24000).
Unlifted feed 0.05501 x 1 x 24000 = 1320. Lift at ae 0.8 on D 2: `1 / sqrt(1 - 0.2^2)` = 1.0206.
App Suggest 1347 (= floor(1320.2 x 1.0206)). The table row grades the **Suggest number** ("App
Suggest" column) against 1320 (`wanaka_21.md:346`, check C3).

## 1. How the Pocket feeds its rings, and what the IR can carry

- One feed for the whole op. `ops/pocket.rs::pocket_contours_to_sided_toolpath` emits every ring
  through `emit_closed_pass` with `feed: params.feed_rate` (`pocket.rs:392`). The adapter
  (`compute/execute/clearing_2d.rs`) passes the op's one `feed_rate`.
- Ring order: outside-in. The first contour is the compensated boundary ("the outermost cutting
  contour", `pocket.rs:228`); each next ring is one stepover inward (`rings.offset_reported(
  stepover)`). In fresh stock the first ring is a full slot (width D); each later ring cuts one
  stepover. Each ring is its own closed pass (rapid, plunge, feed, close, retract).
- The IR carries a per-move feed and a per-move cap. Every `Move` has its own feed
  (`MoveType::Linear { feed_rate }`), and `toolpath::Move::feed_cap: Option<f64>` is generic, not
  3D only: "A planner writes it where a load rule lowers the feed of a move". The stages keep it:
  `dressup/arcfit.rs` and `dressup/condition.rs` join only moves with the same cap;
  `dressup/feedopt.rs:262` and the modulator hold the feed at or below it. The post writes each
  move's own feed. Today only the geodesic 3D arm writes the cap (through its
  `FeedCapStart` / `FeedCapEnd` events and `FeedCapSource`, both types of `adaptive3d/`). The 2D
  ops have no cap event; they can write `Move::feed_cap` directly.

## 2. The candidates

### (a) The generator feeds the slot-first ring at the unlifted feed

- The first ring (and each other move that is a slot by construction) is emitted at
  `geometry::UnliftedFeed` (the door the plunge and the ramp already read) with
  `Move::feed_cap` set, so no later stage lifts it. The later rings keep the op feed.
- idx 6: the op feed stays 1347; ring 1 runs at 1320.
- The Suggest number does not move, so the table row as written stays FAIL. It passes only if
  Suggest also prints the slot-ring feed (one more field of `SuggestedSpeeds`) and the grader
  grades that field: an operator-ruled table change.
- Blast: `ops/pocket.rs` (+ Zigzag, + the 2D Adaptive slot lines if included), the adapter in
  `clearing_2d.rs`, the G-OPGOLDEN rows of those ops (the first-ring feed is in the fingerprint;
  the 46 known rows must not be blessed), the `ops/pocket.rs` unit tests that count moves at
  `params.feed_rate` (`:696`, `:831`, `:907`). No Suggest value and no FM1 cell moves.
- Limit: the generator knows only the first ring by construction. The first ring around an
  island and the last ring of a narrow remainder are not known without the stock.

### (b) Suggest gives a slot-first op no lift

- One line in the P23a door: `PowerCases::full_feed_width` returns D when the set has the slot
  case (`FirstPass::Slot`). It is the P23a rule in its own words: the lift reads the widest
  width the plan cuts at the full feed, and a slot-first plan cuts D at the full feed.
- idx 6: Suggest 1347 -> **1320**; the row passes.
- Blast, larger than it looks:
  - every Pocket and Zigzag op with a stepover under D/2 slows on every ring by its lift:
    wanaka idx 2 (Pocket, 6 mm, stepover 1.5, lift 1.1547): 7038 -> 6096 (-13.4 %); the row stays
    PASS only because its band is wide;
  - FM1: the Pocket and Zigzag cells (up to 80) move (feed down, or RPM up on a feed-capped
    cell, as in P23a);
  - **P13c**: the Pocket roughing proposal is "the most removal per minute inside every limit"
    and the feed of each width carries the lift. With no lift the removal rate rises with the
    width alone, so the proposal moves toward the corner-coverage limit. Expect
    `the_pocket_proposal_is_the_removal_rate_maximum_p13c` and
    `the_add_door_stepover_is_the_proposal_p13c` to move, and the default stepover of a new
    Pocket with them (not checked by a run);
  - `radial_chip_thinning_lifts_a_light_stepover_p11b` if its cell is a Pocket (not checked).

### (c) The post-simulation modulator already lowers the ring

- It does not, and it could not close the row. The modulator caps a move at the band MAXIMUM
  times the lift at the measured width (`dressup/feed_modulation.rs`, the lift door at `:1095`).
  On the slot ring the lift is 1, so the cap is `band max x Z x n`. The op feed is 2 % over the
  target chip, which is inside a band unless the row prints one value; so the ring is not
  lowered (not checked by a run on idx 6). It also runs only after a simulation.
- The row grades the Suggest number, not the G-code, so (c) changes no grade.

## 3. Which families have a slot-first move by construction

| Op | `CutSpec` | Slot move | Today |
|---|---|---|---|
| Pocket (contour) | `Radial::Width`, `FirstPass::Slot` | the first (outermost) ring of each level | the lifted op feed on every ring |
| Zigzag | `Radial::Width`, `FirstPass::Slot` | the first raster line of each level | the lifted op feed on every line (generator not read in detail) |
| 2D Adaptive with `slot_clearing` | `Radial::Width`, `FirstPass::Slot` | the opt-in slot lines (exempt from the pass ceiling) | the op feed, lifted at `c D` since P23a |
| 2D Adaptive, 3D Rough | `FirstPass::NotSlot` | none by construction (helix or ramp entry, pass ceiling) | P23a |
| Profile, Trace, Bore, Project Curve | `Radial::Slot` or full width | every move | no lift already (lift 1) |
| Face | `FirstPass::NotSlot` | none (enters from outside the stock) | the lifted feed |

## 4. Recommendation

**(a), with the slot-ring feed printed by Suggest.** Reasons:

1. It is the correct cut: only the slot ring needs the unlifted feed. (b) slows every later ring
   of every pocket by the lift (13 % on idx 2) to protect one ring.
2. It is local: the generator and one printed field. (b) moves the P13c proposal rule and up to
   80 FM1 cells for a 2 % row.
3. The IR door exists (`Move::feed_cap`) and every dressup already keeps it.

Predicted idx 6: op feed 1347 (unchanged), ring 1 at 1320, Suggest prints
`slot_ring_feed_mm_min` 1320. The table row passes when the operator rules that the row grades
the slot-ring feed. Pins that move: the Pocket (and Zigzag, if included) G-OPGOLDEN rows, the
three `ops/pocket.rs` unit tests above; no FM1 cell.

If the lead wants the row closed with no table change, (b) is the one-line rule, at the cost of
section 2 (b). I would then measure the P13c proposals and FM1 before any pin moves.

Open for the ruling: scope of (a). Pocket only (the row), or Pocket, Zigzag and the 2D Adaptive
slot lines (one rule for every slot-first move)?

## 5. Ruling (2026-11-10)

The operator approved candidate (a) ("yes to the lower feed") and that the table row grades the
slot-ring feed. Lead scope: Pocket (contour) only. The generator feeds the slot-first ring of each
level at the unlifted feed through `geometry::UnliftedFeed`, with `Move::feed_cap` set.
`SuggestedSpeeds` gains `slot_ring_feed_mm_min`. Zigzag, the 2D Adaptive slot lines and an
island's first ring are open items (`P23d.md`).
