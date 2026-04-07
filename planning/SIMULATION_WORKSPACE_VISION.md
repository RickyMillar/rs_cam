# Simulation Workspace Vision

A dedicated verification environment for rs_cam — not a sidebar overlay, but a distinct mode with its own layout, controls, and diagnostic tools.

---

## Critical UX Problem with Current Design

Today, simulation is a thin overlay on the toolpath editing workspace. The timeline scrubber, progress bars, and tool readout are crammed into the viewport overlay alongside view presets and render toggles. The properties panel switches to a simulation info view, but it's the same panel that normally shows operation parameters.

**This conflates two fundamentally different workflows:**

1. **Toolpath authoring** — creating and tuning operations. The user thinks in terms of parameters, feeds, depths, and strategies. The viewport shows toolpath lines.

2. **Verification** — validating that the program is safe and correct before committing to a physical cut. The user thinks in terms of material removal, collisions, surface quality, and cycle time. The viewport shows the workpiece being machined.

These should be separate modes with different layouts, different controls, and different information hierarchies. A machinist switching from "design" to "verify" is making a cognitive shift — the UI should support that shift, not fight it.

---

## Design Principles

1. **Simulation is a destination, not a decoration.** Enter it deliberately, leave it deliberately. Don't mix editing controls with verification controls.

2. **The workpiece is the hero.** In editing mode, toolpath lines are the primary visual. In simulation mode, the stock material and its progressive removal are the primary visual. Toolpath lines become secondary (toggleable).

3. **Time is the organizing dimension.** Everything in simulation is indexed by time (or move number). The timeline is the master control — all other displays derive from it.

4. **Safety information is always visible.** Collision warnings, rapid-through-stock alerts, and gouge indicators must be persistent, not buried behind clicks.

5. **Diagnostics before commitment.** The simulation workspace should answer: "Is it safe to send this to the machine?" Before exiting, the user should have seen: collision status, cycle time estimate, tool change count, and any warnings.

---

## Workspace Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│  [← Back to Editor]   SIMULATION   [Stock ▾] [Tool ▾] [Paths ▾]   │
├──────────┬──────────────────────────────────────────┬───────────────┤
│          │                                          │               │
│  Op List │          3D Viewport                     │  Diagnostics  │
│          │     (stock removal visualization)        │    Panel      │
│  ☑ Op 1  │                                          │               │
│  ☑ Op 2  │                                          │  ┌─────────┐  │
│  ☐ Op 3  │                                          │  │ Current │  │
│          │                                          │  │  State  │  │
│  ───────>│                                          │  └─────────┘  │
│  Jump-to │                                          │               │
│  buttons │                                          │  ┌─────────┐  │
│          │                                          │  │Warnings │  │
│          │                                          │  │ & Flags │  │
│          │                                          │  └─────────┘  │
│          │                                          │               │
│          │                                          │  ┌─────────┐  │
│          │                                          │  │ Summary │  │
│          │                                          │  │  Stats  │  │
│          │                                          │  └─────────┘  │
│          │                                          │               │
├──────────┴──────────────────────────────────────────┴───────────────┤
│  [|◄] [◄] [▶/❚❚] [►] [►|]   ═══════●═══════════   00:42 / 03:15  │
│  Op1 ████████████  Op2 ████░░░░░░  Op3 ░░░░░░░░░   Speed: [500▾]  │
└─────────────────────────────────────────────────────────────────────┘
```

### Left Panel: Operation List
- Checkboxes to include/exclude each operation from simulation
- Colored bullets matching per-toolpath palette
- Click to jump timeline to that operation's start
- Shows tool name and estimated time per operation
- Current operation highlighted with bold/accent

### Center: 3D Viewport
- Stock material is the primary rendered object (not toolpath lines)
- Tool model rendered at current position during playback
- Toolpath lines toggleable (off by default in sim mode)
- Model overlay toggleable (ghost of target shape for comparison)
- Collision markers visible on the stock surface

### Right Panel: Diagnostics
- Context-sensitive: shows different info depending on playback state
- Sections: Current State, Warnings & Flags, Summary Statistics

### Bottom Bar: Timeline
- Full-width timeline with scrubber
- Per-operation colored segments
- Red markers at collision locations
- Transport controls: jump-to-start, step-back, play/pause, step-forward, jump-to-end
- Elapsed / total time display
- Speed control

---

## User Stories

### Epic 1: Enter and Exit Simulation Mode

#### US-1.1: Enter simulation mode
**As a** user who has generated toolpaths,
**I want to** click "Simulate" and enter a dedicated simulation workspace,
**So that** I can focus entirely on verification without editing distractions.

**Acceptance criteria:**
- The entire UI layout changes (not just a panel swap)
- The left panel becomes the operation checklist (not the project tree)
- The right panel becomes diagnostics (not properties)
- The viewport switches from toolpath-line-primary to stock-primary rendering
- Toolpath editing controls are not visible
- A clear "Back to Editor" button is always visible

#### US-1.2: Exit simulation and return to editor
**As a** user who has finished verifying,
**I want to** click "Back to Editor" and return to exactly where I was,
**So that** I can make changes if needed without losing my place.

#### US-1.3: Re-enter simulation preserving state
**As a** user who made edits after a simulation,
**I want** the simulation to detect stale results and offer to re-run,
**So that** I don't accidentally verify outdated toolpaths.

---

### Epic 2: Playback Controls

#### US-2.1: Play/pause continuous playback
**As a** user verifying a program,
**I want to** watch the machining process play out in real-time (or accelerated),
**So that** I can visually inspect the cutting sequence.

#### US-2.2: Step forward/backward one move
**As a** user investigating a specific moment,
**I want to** step one move at a time in either direction,
**So that** I can examine exactly what happens at a suspicious point.

**Acceptance criteria:**
- Single-move forward: advance current_move by 1, update tool position and stock mesh
- Single-move backward: decrement current_move by 1, load appropriate checkpoint mesh + replay from checkpoint to current_move (or use per-move caching if available)

#### US-2.3: Jump to operation boundary
**As a** user who wants to inspect a specific operation,
**I want to** click an operation name and jump directly to its start or end,
**So that** I don't have to scrub through the entire program.

#### US-2.4: Scrub timeline with mouse
**As a** user who sees something wrong at a certain point,
**I want to** drag the timeline scrubber to any position,
**So that** I can quickly navigate to the moment of interest.

**Acceptance criteria:**
- Smooth scrubbing with heightmap checkpoint loading for backward movement
- Tool position and stock mesh update in near-real-time during scrub
- Playback pauses automatically when scrubbing

#### US-2.5: Adjustable playback speed
**As a** user,
**I want** speed presets (0.25x, 0.5x, 1x, 2x, 5x, 10x, max) plus a custom speed slider,
**So that** I can watch slow sections in detail and skip through simple passes.

#### US-2.6: Keyboard transport controls
**As a** power user,
**I want** Space=play/pause, Left/Right=step, Home/End=jump to start/end, [/]=speed up/down,
**So that** I can control playback without touching the mouse.

---

### Epic 3: Stock Visualization Modes

#### US-3.1: Solid stock with wood-tone coloring
**As a** user watching material removal,
**I want** the stock to look like real wood being cut away,
**So that** I have an intuitive sense of what the physical result will look like.

*Already implemented: heightmap_to_mesh with tan→walnut color gradient.*

#### US-3.2: Transparent stock
**As a** user who needs to see the model inside the stock,
**I want** a transparency slider for the stock mesh,
**So that** I can see both the remaining material and the target shape simultaneously.

#### US-3.3: Model ghost overlay
**As a** user comparing the machined result to the design,
**I want** the target CAD model rendered as a semi-transparent ghost inside/over the stock,
**So that** I can visually spot where material is missing or remaining.

#### US-3.4: Color by operation
**As a** user with multiple operations,
**I want** the stock colored by which operation removed each region,
**So that** I can see the contribution of each toolpath.

#### US-3.5: Color by deviation
**As a** user verifying surface quality,
**I want** the stock colored by deviation from the model (green=on-target, red=overcut, blue=remaining),
**So that** I can immediately spot quality issues.

*Infrastructure exists: deviation_colors() function is implemented. Needs: model mesh in SimulationRequest, per-vertex deviation computation, colored mesh rendering.*

#### US-3.6: Color by height/Z
**As a** user inspecting depth consistency,
**I want** a height-map color gradient showing Z values across the stock surface,
**So that** I can verify uniform depth in pockets and detect unexpected ridges.

#### US-3.7: Section view / cross-section
**As a** user who needs to inspect internal geometry,
**I want** a clipping plane that cuts through the stock at an adjustable Z or XY position,
**So that** I can see wall profiles, pocket depths, and undercut geometry.

---

### Epic 4: Tool Visualization

#### US-4.1: Tool wireframe at current position
**As a** user watching playback,
**I want** a 3D tool model visible at the current tool position,
**So that** I can see where the cutter is and what it's doing.

*Already implemented: ToolModelGpuData renders wireframe cylinder/hemisphere.*

#### US-4.2: Tool with holder and shank
**As a** user checking for collisions,
**I want** the tool holder and shank visible (not just the cutter),
**So that** I can see whether the holder is close to the stock or fixture.

#### US-4.3: Tool trail / recent path
**As a** user following the tool's motion,
**I want** a fading trail behind the tool showing its recent path (last N moves),
**So that** I can see the cutting direction and pattern without full toolpath lines.

#### US-4.4: Tool-tip cursor in viewport
**As a** user inspecting a specific location,
**I want to** hover over the stock and see the Z height at that XY point plus the deviation from the model,
**So that** I can do spot-checks without needing a full deviation map.

---

### Epic 5: Collision Detection & Safety

#### US-5.1: Holder/shank collision detection
**As a** user who needs to verify clearance,
**I want** the simulation to detect when the tool holder or shank would contact the stock or fixture,
**So that** I can avoid damaging the machine or workpiece.

*Already implemented: check_collisions_interpolated() with multi-segment holders.*

#### US-5.2: Rapid collision detection
**As a** user who needs to verify safe rapids,
**I want** the simulation to flag any G0 rapid moves that pass through stock material,
**So that** I can avoid catastrophic crashes.

*Core algorithm implemented: check_rapid_collisions(). Not yet rendered.*

#### US-5.3: Collision markers on timeline
**As a** user reviewing collision results,
**I want** red segments on the timeline at collision locations,
**So that** I can scrub directly to each collision and inspect it.

#### US-5.4: Stop-on-collision mode
**As a** user doing careful verification,
**I want** an option to automatically pause playback at the first collision,
**So that** I can inspect the exact moment and decide how to fix it.

#### US-5.5: Collision detail panel
**As a** user investigating a collision,
**I want** a detailed view showing: collision type (holder/shank/rapid), penetration depth, tool position, which operation, and suggested fix (increase stickout by Xmm),
**So that** I know exactly what to change.

*Data exists: CollisionReport has penetration_depth and min_safe_stickout. Not surfaced.*

#### US-5.6: Gouge detection
**As a** user verifying surface quality,
**I want** the simulation to detect where the tool cuts below the target model surface (overcut/gouge),
**So that** I can fix toolpaths that damage the part.

*Related to deviation coloring (US-3.5). Needs: per-vertex comparison, threshold-based flagging.*

#### US-5.7: Minimum material remaining check
**As a** user who specified stock-to-leave,
**I want** verification that no point on the finished surface has less than the specified remaining material,
**So that** my finishing pass has consistent engagement.

---

### Epic 6: Statistics & Metrics

#### US-6.1: Cycle time estimate
**As a** user planning production,
**I want** an accurate estimated machining time based on feed rates and move distances,
**So that** I can quote jobs and plan schedules.

**Breakdown needed:**
- Cutting time (distance / feed_rate per move)
- Rapid time (distance / machine max rapid rate)
- Tool change time (if applicable)
- Total

#### US-6.2: Per-operation statistics
**As a** user optimizing a multi-operation program,
**I want** per-operation metrics: cutting time, rapid time, cutting distance, rapid distance, number of moves,
**So that** I can identify which operations are slowest and optimize them.

#### US-6.3: Material removal volume
**As a** user tracking efficiency,
**I want** the total volume of material removed (from heightmap cell counting),
**So that** I can verify the program removes the expected amount.

#### US-6.4: Tool utilization summary
**As a** user with multiple tools,
**I want** a summary showing each tool's cutting time, cutting distance, and number of operations,
**So that** I can assess tool wear and plan replacements.

#### US-6.5: Feed rate histogram
**As a** user verifying cutting conditions,
**I want** a histogram showing the distribution of actual feed rates during cutting,
**So that** I can spot unexpected slowdowns or speedups.

#### US-6.6: Maximum engagement/depth chart
**As a** user worried about tool load,
**I want** a visualization of the maximum depth-of-cut or engagement along the toolpath,
**So that** I can verify the tool isn't being overloaded.

---

### Epic 7: Pre-Flight Checklist

#### US-7.1: Verification summary before export
**As a** user about to export G-code,
**I want** a pre-flight checklist showing: simulation run (yes/no), collisions detected (count), rapid collisions (count), estimated time, tool changes, warnings,
**So that** I have confidence the program is safe.

**Acceptance criteria:**
- Shown as a modal/panel before G-code export
- Green checkmarks for passed items, red X for failures, yellow for warnings
- "Export Anyway" button (with warning) if issues exist
- "Fix Issues" button returns to editor with the problem selected

#### US-7.2: Stale simulation warning
**As a** user who edited toolpaths after simulation,
**I want** a clear warning that the simulation results are outdated,
**So that** I don't export G-code based on stale verification.

#### US-7.3: Missing verification warning
**As a** user who never ran simulation,
**I want** a gentle nudge when exporting that simulation hasn't been run,
**So that** I don't accidentally skip verification.

---

### Epic 8: Operation Selection & Filtering

#### US-8.1: Select which operations to simulate
**As a** user who only wants to verify a subset,
**I want** checkboxes to include/exclude each operation from the simulation,
**So that** I can focus on the operations I just changed.

*State exists: selected_toolpaths on SimulationState. Needs UI.*

#### US-8.2: Solo an operation
**As a** user debugging a single operation,
**I want to** solo one operation (simulate only that one, starting from fresh stock or from a checkpoint),
**So that** I can see exactly what it does in isolation.

#### US-8.3: Compare before/after
**As a** user who changed an operation,
**I want** side-by-side or overlay comparison of the stock before and after a specific operation,
**So that** I can see exactly what material it removed.

---

### Epic 9: Advanced Diagnostics (Blue Sky)

#### US-9.1: G-code synchronized view
**As a** machinist reviewing the program,
**I want** a scrolling G-code view synchronized with simulation playback,
**So that** I can correlate visual behavior with specific G-code lines.

#### US-9.2: Cutting force estimation
**As a** user worried about tool deflection or chatter,
**I want** an estimated cutting force chart along the toolpath (based on engagement and material),
**So that** I can identify high-load areas.

#### US-9.3: Surface finish prediction
**As a** user targeting a specific surface quality,
**I want** a predicted scallop height or surface roughness map based on tool geometry and stepover,
**So that** I can verify the finish will meet requirements without a physical test cut.

#### US-9.4: Thermal load estimation
**As a** user cutting heat-sensitive materials,
**I want** an estimate of thermal load along the toolpath,
**So that** I can identify areas where the tool dwells too long.

#### US-9.5: Toolpath replay with real-time audio
**As a** user who wants an immersive preview,
**I want** simulated cutting audio (frequency based on spindle speed, volume based on engagement),
**So that** I can "hear" the cut and detect anomalies by sound pattern.

#### US-9.6: Comparison with physical measurement
**As a** user who has a touch probe,
**I want to** import measured surface data and compare it against the simulation,
**So that** I can calibrate my simulation accuracy.

---

## What We Already Have (and what's missing for each)

| Capability | Current State | Gap |
|-----------|--------------|-----|
| Heightmap stock simulation | Fully working | None — shipped |
| Playback (play/pause) | Working | Missing: step-forward/backward |
| Timeline scrubbing | Working slider | Missing: click-on-timeline, zoom, collision markers |
| Checkpoint rewind | Data stored | Not used during scrub — backward scrubbing replays from last checkpoint |
| Per-op progress bars | Working | Could be richer (time estimate per op) |
| Tool model during playback | Working wireframe | Missing: holder/shank model, tool trail |
| Tool position readout | Working (X/Y/Z) | Missing: move type, feed rate, spindle speed |
| Collision detection (holder) | Fully working | Not integrated into simulation timeline |
| Rapid collision detection | Core algorithm done | Not rendered or integrated |
| Deviation coloring | Color function done | Not connected (needs model mesh in sim) |
| Cycle time estimate | Basic (distance/feed) | Needs per-op breakdown, rapid time |
| Pre-flight checklist | Not started | High priority for safety |
| Dedicated workspace layout | Not started | Currently overlaid on editing workspace |
| Stock transparency/modes | Not started | Currently solid wood-tone only |
| Model ghost overlay | Not started | Would need separate mesh render pass |
| Section view | Not started | Would need clip plane in shader |
| G-code synchronized view | Not started | Blue sky |
| Operation solo/compare | Not started | Checkpoint data supports this |

---

## Implementation Priority

### Phase 1: Workspace Separation (foundation)
- Dedicated simulation mode (separate UI layout)
- "Enter Simulation" / "Back to Editor" transitions
- Operation checklist in left panel
- Diagnostics panel in right panel
- Full-width timeline at bottom

### Phase 2: Enhanced Playback
- Step forward/backward buttons
- Keyboard transport controls
- Click-to-jump on timeline
- Per-operation time estimates in op list
- Move type indicator (Feed/Rapid/Plunge/Lead)

### Phase 3: Safety Integration
- Collision markers on timeline (red segments)
- Rapid collision markers
- Stop-on-collision toggle
- Collision detail in diagnostics panel
- Stale simulation warning

### Phase 4: Stock Visualization Modes
- Transparent stock (alpha slider)
- Model ghost overlay
- Color by deviation (wire up existing infrastructure)
- Color by operation

### Phase 5: Statistics & Pre-Flight
- Cycle time breakdown (cutting/rapid/tool change)
- Per-operation statistics table
- Material removal volume
- Pre-flight checklist modal before export

### Phase 6: Advanced (blue sky)
- Section view / clipping plane
- G-code synchronized scroll
- Tool holder/shank rendering
- Surface finish prediction
- Feed rate histogram
