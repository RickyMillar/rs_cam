# Multi-Setup UX: Implementation Plan

Phased implementation plan for fixing multi-setup workflows in rs_cam_viz.
Each phase is designed to be executable as an independent session with clear
inputs, outputs, and verification criteria.

**Prerequisite completed:** Unified coordinate frame fix (all toolpaths always
generated in local coords, display always in local frame). This is on `master`
but uncommitted — commit before starting any phase.

---

## User Journeys (reference)

Three user archetypes drive the requirements:

**User A — Hobbyist (double-sided terrain relief):** 2 setups (Top + Bottom),
simple toolpaths per side. Needs: visible stock, working multi-setup simulation,
clear flip guidance.

**User B — Maker (wooden jewelry box):** 2 setups with rest machining within
Setup 1 (rough → finish → lid recess). Needs: remaining-stock awareness,
alignment pins for flip, keep-out zones informed by previous cuts.

**User C — Professional (double-sided 3D sculpture):** 2 setups with 3+
toolpaths each, through-cuts, fixtures. Needs: cross-setup material awareness
(safety-critical), multi-setup simulation, deviation verification.

---

## Phase A: Stock Visibility & Display Rules

**Goal:** Users can clearly see the stock block in all workspaces. Each
workspace shows the right combination of elements.

**Can run in parallel with:** Phase B

**Prerequisites:** Coordinate frame fix committed

### Tasks

**A1. Fix solid stock opacity**
- File: `crates/rs_cam_viz/src/render/stock_render.rs`
- Change solid stock alpha from 0.08 to 0.18
- Ensure the wood-tone color is warm and distinct from the model mesh

**A2. Per-workspace display rules**
- File: `crates/rs_cam_viz/src/app.rs` (the render callback flags, ~line 1300+)
- Setup workspace: show model + stock wireframe + solid stock + datum + fixtures + pins
- Toolpaths workspace: show model + stock wireframe + toolpaths + height planes. Hide solid stock (obscures toolpath detail)
- Simulation workspace: show sim heightmap mesh + stock wireframe + tool. Hide model mesh + solid stock

Current flags to audit: `show_mesh`, `show_solid_stock`, `show_origin_axes`,
`show_height_planes`, `show_grid`, `show_fixtures`

**A3. Show effective stock dimensions in setup panel**
- File: `crates/rs_cam_viz/src/ui/setup_panel.rs`
- In the stock summary card, compute effective dims via `setup.effective_stock()`
- Display as: "Stock: {eff_w:.0} × {eff_d:.0} × {eff_h:.0} mm"
- When no setup is active, show raw stock dims

**A4. Fresh-stock warning badge on non-first setups**
- File: `crates/rs_cam_viz/src/ui/setup_panel.rs`
- On Setup 2+ cards, add a small italic label: "Toolpaths use fresh stock"
- Color: muted orange/amber (informational, not error)

### Verification
- Load Wanaka job, confirm solid stock is visible as translucent box
- Switch between all 3 workspaces, confirm correct elements show/hide
- Create 2 setups with different orientations, confirm effective dims update
- Confirm badge appears on Setup 2 but not Setup 1

### Files touched
- `crates/rs_cam_viz/src/render/stock_render.rs`
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/ui/setup_panel.rs`

---

## Phase B: Multi-Setup Simulation (Global Frame)

**Goal:** Simulation correctly combines toolpaths from multiple setups with
different orientations onto a single stock heightmap.

**Can run in parallel with:** Phase A

**Prerequisites:** Coordinate frame fix committed

### Background

Toolpaths are generated in their setup's local frame. The simulation heightmap
is a single 2.5D grid representing the physical stock block. Different setups
cut from different sides. The only way to combine them on one heightmap is to
inverse-transform each setup's toolpaths back to the **global (world) frame**
before simulation.

For display, the heightmap mesh (global frame) is then forward-transformed into
the active setup's local frame so the viewport stays consistent.

### Tasks

**B1. Revert simulation submission to global frame**
- File: `crates/rs_cam_viz/src/controller/events.rs`
- In `run_simulation_with_all` and `run_simulation_with_ids`:
  - Restore inverse-transform of toolpaths before submission
  - Use global `stock.bbox()` for simulation stock
  - Use untransformed model mesh for simulation
- Add back `inverse_transform_toolpath` as a helper (can be a free function
  or method — takes setup + stock + toolpath, returns new Arc<Toolpath>)

```
fn inverse_transform_toolpath(
    setup: &Setup, stock: &StockConfig, tp: &Arc<Toolpath>
) -> Arc<Toolpath>
```

**B2. Forward-transform sim heightmap mesh for display**
- File: `crates/rs_cam_viz/src/app.rs` — sim mesh upload section (~line 932)
- After `heightmap_to_mesh()` produces a global-frame mesh, forward-transform
  all vertex positions into the active setup's local frame:
  ```
  for i in (0..mesh.vertices.len()).step_by(3) {
      let p = P3::new(mesh.vertices[i], mesh.vertices[i+1], mesh.vertices[i+2]);
      let local = setup.transform_point(p.into(), stock);
      mesh.vertices[i] = local.x as f32;
      // ...
  }
  ```
- Apply same transform to checkpoint meshes on load
- Consider adding a helper: `transform_heightmap_mesh(mesh, setup, stock)`
  in `state/job.rs`

**B3. Fix sim tool position for global→local display**
- File: `crates/rs_cam_viz/src/app.rs` — `update_sim_tool_position`
- Tool position is read from the stored toolpath (local frame)
- For display: inverse-transform to global (using the toolpath's own setup),
  then forward-transform to the active display setup's local frame
- For single-setup case (toolpath setup == display setup): these cancel out,
  position is used directly
- For multi-setup: the two transforms compose correctly

**B4. Fix backward-scrub fresh heightmap**
- File: `crates/rs_cam_viz/src/app.rs` — backward scrub section
- When resetting to fresh stock for backward scrubbing, use global `stock.bbox()`
  (simulation is in global frame)
- The `heightmap_to_mesh` output will still be forward-transformed for display
  (same as B2)

**B5. Simulation timeline setup markers**
- File: `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- Use existing `setup_boundaries` in `SimulationResults` to show which setup
  is currently being animated
- Highlight the active setup name/label during playback
- Show a "flip" divider between setup boundaries in the timeline

### Verification
- Load Wanaka job with 2 setups (Top + Bottom)
- Generate toolpaths for both setups
- Run simulation — terrain carve should appear on top, bottom recess on bottom
- Scrub timeline — tool should track correctly for both setups
- Scrub backward past setup boundary — heightmap should reset correctly
- Confirm no visual jump when entering/leaving Simulation workspace

### Files touched
- `crates/rs_cam_viz/src/controller/events.rs`
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/state/job.rs` (optional helper)
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`

---

## Phase C: Display Polish & Edge Cases

**Goal:** Clean up remaining display inconsistencies and edge cases found
during Phase A/B testing.

**Can run in parallel with:** Nothing — runs after A+B merge

**Prerequisites:** Phase A and Phase B both merged

### Tasks

**C1. Audit and fix all `needs_transform()` remnants**
- The function still exists on `Setup` — it's fine to keep it as a utility,
  but no display/generation code should branch on it
- Grep for any remaining call sites and verify they're benign

**C2. Workspace switch camera behavior**
- When switching to Simulation workspace, the scene content changes (heightmap
  replaces model). Consider a "reset view" or "fit to scene" on workspace switch
  so the camera doesn't end up pointed at empty space
- File: `crates/rs_cam_viz/src/app.rs` — SwitchWorkspace event handler

**C3. Entry/exit preview coordinate frame**
- The entry preview reads from `result.toolpath` (local frame) — confirm it
  renders correctly without extra transforms
- File: `crates/rs_cam_viz/src/app.rs` — entry preview section

**C4. Picking in correct frame**
- Viewport click-to-select uses ray casting. Confirm the ray is in the same
  frame as the rendered geometry (local frame)
- File: `crates/rs_cam_viz/src/interaction/picking.rs`

**C5. Collision check frame**
- Collision checking reads toolpath positions — confirm it accounts for the
  local frame or operates in the correct coordinate system
- File: `crates/rs_cam_viz/src/controller/events.rs` — `request_collision_check`

### Verification
- Full manual test: import model → 2 setups → generate → simulate → export
- Verify no visual glitches, no coordinate mismatches
- Run `cargo test -q` — all tests pass
- Run `cargo clippy` — no warnings

### Files touched
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/interaction/picking.rs`
- `crates/rs_cam_viz/src/controller/events.rs`

---

## Phase D: Future Work (not this sprint)

These are documented requirements that should not be attempted until A+B+C
are stable and tested.

**D1. Remaining stock visualization (R4.1)**
- Show a ghost/overlay of simulated remaining stock in Setup workspace
- Requires running a quick simulation of previous setups and rendering the
  result as a transparent overlay

**D2. Expose stock_source in UI (R4.2)**
- Add `FromRemainingStock` option in toolpath properties
- Requires simulating prior toolpaths to generate the starting heightmap
- Feed optimization already disabled for this mode (catalog.rs line 721)

**D3. Cross-setup alignment pin visualization**
- Show alignment pin positions from other setups as ghost markers
- Requires transforming pin positions between setup frames

**D4. Fixture + carved-part preview**
- In Setup 2+, show how the already-carved part sits in the fixture
- Requires combining the sim result mesh with fixture geometry

**D5. Multi-setup deviation map**
- Deviation mode that works across setup boundaries
- Requires transforming the model mesh to match the global-frame heightmap

---

## Parallelism Summary

```
                    ┌─────────────┐
                    │  Commit     │
                    │  coord fix  │
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
     ┌────────────────┐       ┌────────────────┐
     │   Phase A      │       │   Phase B      │
     │  Stock viz +   │       │  Multi-setup   │
     │  display rules │       │  simulation    │
     └───────┬────────┘       └───────┬────────┘
              │                       │
              └────────────┬──────────┘
                           ▼
                  ┌────────────────┐
                  │   Phase C      │
                  │  Polish &      │
                  │  edge cases    │
                  └────────────────┘
                           │
                           ▼
                  ┌────────────────┐
                  │   Phase D      │
                  │  Future work   │
                  │  (deferred)    │
                  └────────────────┘
```

**Phase A** and **Phase B** are fully independent and can run as parallel
sessions on separate worktrees. They touch different sections of the same
files (`app.rs`, `events.rs`) but in non-overlapping regions:

- Phase A touches: render flags, stock_render.rs, setup_panel.rs
- Phase B touches: sim submission, sim mesh upload, sim tool position, sim_op_list.rs

**Phase C** depends on both A and B being merged first.

---

## Session Instructions

Each phase should start a fresh session. Paste the following context:

> Read `planning/MULTI_SETUP_UX_PLAN.md`. Execute Phase [A/B/C].
> The coordinate frame fix is committed on master.
> Read the relevant files listed in the phase before starting.
> Commit when done with a descriptive message.
