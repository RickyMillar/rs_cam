# F-039a — 3D feed-rate heatmap on toolpath visualisation

- **Stage:** GUI / viz
- **Severity:** low (UX polish — visualises what F-039 surfaces in numeric form)
- **Status:** open — design only; lands after F-039 settles
- **First found in:** F-039 design discussion, 2026-05-27
- **Effort:** M (viz pipeline extension + color-mapping logic + UI toggle)
- **Linked PRs:** —
- **Workstream:** Feed Modulation
- **Depends on:** F-039 landed (its `SimulationCutTrace::modulated_feeds` map is the data source)

## Symptom

F-039 lands two diagnostic layers (per-toolpath modulation summary; per-move G-code comments) but the third — the **3D heatmap of emitted feed along the toolpath path** — needs viz pipeline plumbing not on F-039's critical path. Operators benefit most from "see where the modulator is biting in 3D" — the long straights glow hot, corners and constraint-binding moves glow cool. One picture replaces 30 lines of numeric readout.

## Hypothesised root cause

n/a — pure visualisation feature.

## Fix shape

Add a viewport overlay mode "feed heatmap" on the 3D toolpath renderer:

1. **Color-mapping**: virtual gradient blue (slow) → green (mid) → orange (fast). Map per-move emitted feed `f` to color as `(f - min_emitted) / (max_emitted - min_emitted)`. Use a perceptually-uniform colormap (viridis or magma) instead of rainbow.
2. **Toggle in the timeline panel**: "Color toolpath by: [solid | cutting time | feed rate | binding constraint]". The "binding constraint" mode uses the discrete `BindingConstraint` enum from F-039 — distinct color per binding type. Surfaces visually where chipload-max bites vs deflection.
3. **Per-move hover tooltip**: hovering a move shows commanded feed, emitted feed, binding constraint, in the same panel the rest of the move info renders in.

## Acceptance test

GUI-thread tests not feasible in the existing test framework. Lands behind:

1. A `crates/rs_cam_viz/src/ui/heatmap_color_test.rs` unit test for the color-mapping helper: assert `viridis(0.0) == blue_endpoint`, `viridis(1.0) == orange_endpoint`, monotonic. No GUI.
2. Manual visual verification on wanaka Back Rough — record screenshots in the F-039a PR description showing the heatmap rendering correctly.
3. F-037 smoke baseline diff clean (pure viz; no simulator code touched).

## Files

- `crates/rs_cam_viz/src/ui/...` — heatmap overlay rendering + toggle
- `crates/rs_cam_viz/src/state/...` — heatmap mode state
- `crates/rs_cam_core/src/viz/...` (or wherever the toolpath-renderer lives) — color-mapping data path

## Risk

S. Pure additive viz feature. Off-by-default. Default behaviour byte-identical.

## Notes

- Feed heatmap is most useful as **immediate post-generation feedback** ("did my modulation tweak do what I expected?"). Operators on the workshop machine still get the G-code comments from F-039 Layer 2.
- A future F-039b could add a **feed-vs-time line chart** in the timeline panel (Layer 2 from the F-039 design discussion). Heatmap is the higher-bang-for-buck of the two.
- The same color-mapping infrastructure can later be reused for: chipload heatmap (F-035 predicted-feed map), deflection heatmap, power heatmap, axial-DOC heatmap. F-039a is the foundation feature; the others are sister findings if anyone asks for them.
