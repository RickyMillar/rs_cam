# D56 pre-registration (option b of D55), written before the change and before any run

Base: `f2724a5bd` (D55 committed).

## A correction to option (b) as worded in D55

"The boundary clear runs per region, and the sliver-safe rule stays" cannot lower the wanaka rapid floor:
- With the sliver-safe rule, a per-region clear keeps every cell whose square can touch a disc centred in the consumer's regions. Those are the cells that the rapid-floor read (`max_conservative_top_z_in_disc`, reach R + cs/2) looks at.
- So the frame cell that holds the floor up stays, and the floor stays.
- The plain per-region clear (centre inside a region, as the single-polygon clear does) also clears real stock under the disc edge, in the band between a region and the disc reach. The rapid floor would then under-read that band. That breaks the sliver-safe rule.

The clear that is correct and does what (b) intends uses the SOURCE op's machining reach:
- The source op never cut outside its reach. After D48 (the published regions lie inside the source's centre containment) and D53 (the consumer's tool is no larger than the source's, unless the containment is `Inside`), the consumer's tool disc lies inside that reach.
- So stock outside the source's reach (the holding frame) can never be under the consumer's disc. A planner cell whose CENTRE is outside the source reach holds no stock that the consumer can touch.
- The dexel model gives the whole cell square that height. The real frame starts at the source reach line, and the consumer's disc ends at or before that line. So the "sliver" the conservative read guards against cannot be there.
- The sliver-safe read itself does not change.

## The change

1. Generation (`session/compute/generation.rs`) resolves the source op's reach for a `DerivedRestRegions` consumer. It does so only when the source's boundary is enabled, is a single polygon (`Stock`, `ModelSilhouette`), has `Inside` or `Center` containment, and the source is on the same setup and model as the consumer.
   - The reach is the raw source polygon (`resolve_containment_polygon`, with no keep-outs, so it errs large) for `Inside`, and that polygon grown by the source's tool radius for `Center`.
   - In every other case: no reach, no clear (as today).
2. The reach goes to the 3D Rough planner (`Adaptive3dGeometry::source_reach`; empty = none). After the boundary clear, the planner clears every planner-stock cell whose centre is outside the reach, with the clear of the existing boundary clear (down to the surface, or to the stock bottom when uncovered).

## Expected effect

- With no `DerivedRestRegions` consumer, `source_reach` is empty everywhere, so the output is byte-identical. Parity fixtures and goldens: no change (asserted by running them).
- Wanaka op 7 (`Inside` source boundary): the frame leaves op 7's planner stock. At the edge sites the rapid floor drops from the frame top (G-code Z 0.0) to the region's own stock. The straight air feed of D55 then shrinks to the clearance, and most of op 7's entry time goes. The anchor (G-LADDERANCHOR) can also drop, because the frame no longer counts as material.
- Corne op 5 (`Center` source with offset +6): the reach is the silhouette grown by 6 mm and then by R, past which no frame stands near op 5. Expect little or no change.
- Sentry: a rest op beside an uncut frame has its rapid floor at the region's own stock, not at the frame top. Non-vacuity: an entry lies within one tool radius plus one cell of the frame. It is red on the base.
