# Viewport interaction redesign

**Status:** Phases 1 and 2 landed 2026-09-23 night (171ec9c4 inventory + mockups, 2594fec7 dock + catalogue) against mockups the operator has not reviewed, by the operator's instruction to run unattended; phases 3 and 4 in progress  
**Scope:** `crates/rs_cam_viz` viewport controls and their presentation; no core, MCP-wire, or compute-contract change is proposed by this document.

## Decision

Replace the current fragmented viewport control presentation with a stable, bottom-centred floating viewport dock. It has four text-labelled sections:

- **View:** camera, projection, fit, and reset.
- **Scene:** geometry, stock, fixtures, grid, boundaries, and layers.
- **Paths: Selected:** selected/all visibility, global cuts/rapids, and move colour.
- **Inspect: Reach:** model/stock analysis and consistent access to path analysis.

The dock opens one upward popover at a time. A complete, searchable **All viewport options** catalogue is the escape hatch for every registered option, including options currently blocked or unavailable; it is not a permanent second panel. Section positions remain stable while their contents reflect the live target, availability, and relevant analysis. V1 deliberately excludes favourites, presets, and drag-customisation.

```
                    target: Op 12 — Adaptive Pocket
  Legend: Move colour — Engagement (%, Op 12) | Stock deviation (mm)
 ┌──────────────────────────────────────────────────────────────────┐
 │ View                 Scene        Paths: Selected   Inspect: Reach│
 │ Camera / Fit         Layers       Selected / All     Model / Stock│
 └──────────────────────────────────────────────────────────────────┘
                         ↑ one contextual popover
```

The target and a compact legend rail sit above the dock. The proposed dock must remain usable alongside simulation playback and respect the current 320pt viewport-width guard; narrow layouts require measured wrapping and space-limit validation rather than a claim of unlimited room.

## Current evidence and problem

`crates/rs_cam_viz/src/ui/viewport_overlay.rs` provides a top strip. `ui/overlays/panel.rs` provides a large roughly 40-row dock/floating panel, with legends buried at its end. `ui/overlays/registry.rs` is the authoritative source for UI, MCP, defaults, and exclusivity. The redesign is a presentation and interaction plan around those facts, not a claim that a replacement has landed.

Reach defaults to Toolpaths, and a selected supported operation drives background computation. Rest is demand-driven through **Compute rest**; it may regenerate or invalidate simulation, and is not automatically driven by selection. Those distinctions are currently easy to lose among controls and status messages.

## Interaction and availability model

Every analysis or render-affecting choice uses a visible availability state:

- **Ready/off** — available but not presently rendered.
- **Showing** — actually rendered now.
- **Needs compute** — accompanied by its action.
- **Computing** — progress/cancellation remains available through the existing jobs surface.
- **Blocked** — explains reason and remedy.
- **Stale** — identifies that its result is no longer current.
- **Failed** — provides the failure and recovery route.

The UI distinguishes a preferred analysis/mode from the current rendered result. Changing operation selection changes the target, not the user’s chosen mode. If the preferred mode cannot apply, label it as unavailable; do not silently substitute another mode. Plain geometry may remain visible. Ready modes apply immediately. **Compute** and **Compute & show** have distinct outcomes. Completion of asynchronous work must never take over rendering after the user changed selection or intent.

Before Rest work starts, explicitly explain its regeneration/invalidation consequence. Any decision to alter Reach computation policy or defaults is separate future work and must not be silently bundled into this redesign.

Exclusive groups remain explicit: Model Reach and Rest; Stock Solid, Deviation, and Height; and Moves Palette, Engagement, and Advance-per-tooth. Other layers stack. Per-operation eye/C/R controls remain distinct from global Paths controls.

## Legends and semantics

Always show compact legends above the dock for **all actual active colour encodings**, even when their menus are closed. Each gives a name, scale or categories, units where applicable, and target. Audit beyond scalar registry legends: toolpath palette, cut, rapid, collision, and territory encodings are in scope. Multiple surface legends coexist; essential meaning has no `+N` overflow. Detailed caveats may be expandable.

## Architecture constraints

Retain registry IDs, MCP semantics, defaults, triggers, and exclusivity. Use the established UI/controller/worker path and `ProjectSession::apply(Command)` mutation contract. The draw loop must neither mutate core state nor initiate compute; derive live state instead of creating parallel cached truths. Preserve canonical `toolpaths_to_draw` upload and picking semantics. Jobs and cancellation remain separate from view controls but reachable from their Computing state. Keyboard navigation, Escape, outside click, clear labels, and non-interception of viewport input outside controls are required.

## Delivery phases

1. Inventory registry options and state definitions; produce user-reviewed mockups for ready, blocked, computing, multiple legends, narrow layouts, and simulation playback.
2. Implement dock and catalogue presentation over the existing registry.
3. Implement target/selection/compute behaviour and asynchronous intent protection.
4. Complete legend audit, responsive verification, and interaction verification.

## Acceptance and next action

Acceptance requires every registry option be reachable; blocked states explain reason and action; actual rendering agrees with state and legend; all active legends are visible; exclusivity holds; async work cannot cause stale takeover; per-operation versus global controls and draw/pick parity remain correct; and narrow/playback layouts remain usable. UI and MCP behaviour must remain consistent.

**Next action:** coordinate with concurrent work, then obtain user-reviewed mockups before implementation. Future source work will follow repository testing gates and `scripts/cargo_lane.sh`; this documentation-only proposal runs no builds or tests.
