# Dressup transform cleanup plan

## Invariant

Any transform that changes toolpath move order or move count must either preserve semantic spans or explicitly mark them discarded/approximate before downstream consumers use them.

This applies to dressups, boundary clipping, rapid-order optimization, feed optimization, and any future post-generation transform.

## Current guardrail

`OperationTransformCapabilities` in `compute::catalog` records the minimum safety metadata needed before generic topology transforms run:

- `allows_global_rapid_reorder`
- `requires_depth_order`
- `continuous_path_required`

The dressup pipeline uses these flags to prevent unbarriered TSP and stay-down link moves from crossing operations that depend on original order or continuity. Adaptive3d can still use marker-derived barriers for within-level rapid ordering.

## Next steps

1. Replace raw barrier indices with generic `ToolpathSpan` metadata: `start_move`, `end_move`, `kind`, `label`, and optional payload.
2. Convert operation runtime annotations into spans immediately after operation generation.
3. Move GUI tracing and session dressups onto one shared core `DressupPipeline`.
4. Make each transform span-aware:
   - no-op transforms preserve spans;
   - TSP reorders only within compatible spans and remaps ranges;
   - link moves never bridge protected span boundaries;
   - boundary clip and air-cut filter either remap spans or mark them approximate.
5. Audit every operation against depth ordering, continuity, TSP safety, link safety, and span emission.
