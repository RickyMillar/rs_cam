# Workspace UX Redesign Plan

Detailed implementation plan for restructuring the `rs_cam_viz` desktop UX around clearer workspaces, better setup flows, and a more trustworthy simulation/verification experience.

This plan is intended as a handoff document for another agent. It is deliberately prescriptive about sequencing, file targets, and acceptance criteria so execution can start without redoing the design analysis.

## Problem summary

Current UX issues are not just visual polish problems. The main failures come from state architecture and information architecture:

- simulation results, simulation visibility, and simulation workspace are conflated
- setup editing is powerful but too form-heavy and weakly guided
- toolpath authoring, setup definition, and verification are mixed into the same surface
- collision checking is underspecified in both scope and labeling
- the viewport shows useful information but is not yet a primary manipulation surface

These problems should be addressed in layers:

1. fix state and workflow boundaries
2. redesign workspace navigation and panel layout
3. redesign setup and toolpath authoring flows
4. expand viewport interaction
5. harden simulation/collision correctness and stability

Do not start with cosmetic UI cleanup. The architecture currently causes UX leakage no matter how polished the widgets become.

## Product goals

### Primary goals

- make `Setup`, `Toolpaths`, and `Simulation` feel like distinct workspaces
- make it obvious where a user should go for setup definition vs path authoring vs verification
- make multi-setup jobs understandable and trustworthy
- reduce parameter soup in setup editing
- make simulation a first-class verification environment, not a partial overlay on the editor
- move high-value actions into the viewport where direct manipulation is faster than forms

### Non-goals for the first tranche

- full visual redesign / branding / theme overhaul
- generalized CAD-style transform gizmos for every entity type
- complete rewrite of simulation math unless needed for multi-setup correctness
- CLI parity work beyond what is required for test fixtures

## High-level end state

The target end state is:

- top-level workspace switcher with `Setup`, `Toolpaths`, and `Simulation`
- `Setup` workspace focused on stock orientation, datum, workholding, and registration
- `Toolpaths` workspace focused on operation list, tools, parameters, generation, and path preview
- `Simulation` workspace focused on preview, animation, verification, and export readiness
- simulation results cached as artifacts, not treated as the active editor mode
- one clear entry point into simulation
- collision and safety checks expressed as named verification tasks with explicit scope
- viewport used for picking, positioning, and review, not just passive viewing

## Architectural direction

### 1. Separate workspace state from simulation results

Current `AppMode` and `SimulationState.active` are doing overlapping jobs. Replace that with a clearer split:

- `AppState.workspace`: which workspace is currently shown
- `SimulationState.results`: optional cached simulation output
- `SimulationState.playback`: transport/timeline/playhead state
- `SimulationState.checks`: verification outputs such as rapid collisions and holder collisions

Recommended shape:

```rust
pub enum Workspace {
    Setup,
    Toolpaths,
    Simulation,
}

pub struct SimulationState {
    pub results: Option<SimulationResults>,
    pub playback: SimulationPlayback,
    pub checks: SimulationChecks,
    pub last_run: Option<SimulationRunMeta>,
    pub resolution: f64,
    pub auto_resolution: bool,
    pub stock_viz_mode: StockVizMode,
    pub stock_opacity: f32,
}
```

Key rule:

- leaving the `Simulation` workspace must not clear results unless the user explicitly resets them
- having results must not hijack the properties panel or the viewport unless the current workspace chooses to show them

### 2. Treat simulation as a workspace, not a hidden mode

`RunSimulation` and `EnterSimulation` should stop being two loosely-related concepts.

Recommended behavior:

- entering `Simulation` never implicitly mutates toolpaths or job state
- `Simulation` workspace can show:
  - empty state if no results exist
  - stale state if results exist but inputs changed
  - current results if up to date
- simulation execution is triggered explicitly from inside the `Simulation` workspace or from one clear call-to-action in `Toolpaths`

### 3. Separate minimal bug fix from full multi-setup simulation fidelity

The current multi-setup simulation bug has two layers:

- visual/workspace bug: setup 2 does not align with the stock/model presentation
- physical simulation bug: stock removal is simulated as one continuous heightmap without proper setup-to-setup transforms

The plan should handle this in two steps:

- `Phase 1 correctness fix`: make playback and display use per-setup transforms so the second setup is not visibly wrong
- `Phase 2 fidelity fix`: correctly carry stock state between setups using setup-frame transforms and resampling

Do not claim fully correct multi-setup stock simulation until the second step is complete.

## Execution plan

## Phase 1: State split and correctness blockers

Status target: first implementation tranche

### 1. Introduce first-class workspaces

File targets:

- `crates/rs_cam_viz/src/state/mod.rs`
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/ui/menu_bar.rs`
- `crates/rs_cam_viz/src/ui/mod.rs`

Tasks:

- rename or replace `AppMode` with `Workspace`
- add three workspaces: `Setup`, `Toolpaths`, `Simulation`
- stop using simulation result existence to decide which layout or panel is shown
- add explicit workspace-switch events if needed
- keep the menu bar global, but remove duplicated simulation entry semantics

Acceptance criteria:

- the app can show `Setup`, `Toolpaths`, and `Simulation` explicitly
- leaving `Simulation` does not alter the `Setup` or `Toolpaths` side panels
- simulation results can exist without forcing the editor into a simulation view

### 2. Refactor simulation state so results are cached artifacts

File targets:

- `crates/rs_cam_viz/src/state/simulation.rs`
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/controller/events.rs`

Tasks:

- replace `simulation.active` with an explicit `results: Option<...>`
- move timeline/playback fields into a playback sub-struct
- move rapid-collision and holder-collision outputs into a checks sub-struct
- update rendering so sim mesh is shown only when the active workspace requests it
- update preflight logic to inspect explicit results/check state instead of `active`

Acceptance criteria:

- stale/up-to-date/no-results states are distinct and testable
- sim mesh does not appear in `Setup` or `Toolpaths` unless intentionally requested
- reset semantics are explicit: reset results, reset playback, or clear checks

### 3. Fix the current multi-setup simulation alignment bug

File targets:

- `crates/rs_cam_viz/src/controller/events.rs`
- `crates/rs_cam_viz/src/compute/worker.rs`
- `crates/rs_cam_viz/src/compute/worker/execute.rs`
- `crates/rs_cam_viz/src/state/job.rs`
- `crates/rs_cam_viz/src/app.rs`

Tasks:

- extend simulation request data to carry setup membership and transform metadata per toolpath
- ensure playback knows which setup frame each boundary belongs to
- apply the active setup’s display transform when drawing tool position and setup-specific overlays
- ensure setup markers are tied to transform changes, not just labels

Recommended data addition:

- add simulation boundary metadata:
  - `setup_id`
  - `setup_name`
  - possibly `setup_transform`

Acceptance criteria:

- a two-setup top/bottom example no longer shows the second setup running next to the stock
- setup transition markers correlate with visible transform/orientation changes
- unit or controller test covers a multi-setup playback case

### 4. Rename or redefine collision checking before further UI work

File targets:

- `crates/rs_cam_viz/src/ui/viewport_overlay.rs`
- `crates/rs_cam_viz/src/ui/menu_bar.rs`
- `crates/rs_cam_viz/src/ui/preflight.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/controller/events.rs`

Tasks:

- decide whether the near-term feature is:
  - `Check Holder Clearances` for one path, or
  - full-job collision/safety verification
- if the backend remains narrow, rename every button and checklist item to match the real scope
- if expanding scope now, feed all relevant paths/setups into the check pipeline

Recommended near-term decision:

- rename now if a full-scope implementation is not part of this tranche
- fold the current narrow holder check into the `Simulation` workspace instead of leaving it as a floating global action

Acceptance criteria:

- no button label implies broader verification than the code performs
- preflight can explain which checks are up to date, stale, or not run

## Phase 2: Workspace navigation redesign

Status target: ship after Phase 1 state work lands

### 5. Build explicit top-level workspace navigation

File targets:

- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/ui/menu_bar.rs`
- new file if needed: `crates/rs_cam_viz/src/ui/workspace_bar.rs`

Tasks:

- add a prominent workspace switcher near the top of the app
- show stable labels and optional badges:
  - `Setup`
  - `Toolpaths`
  - `Simulation`
- add state badges such as:
  - stale simulation
  - collisions detected
  - operations pending generation

Acceptance criteria:

- a new user can tell which workspace they are in without inferring from panel contents
- the switcher is available in every workspace

### 6. Redefine each workspace layout

File targets:

- `crates/rs_cam_viz/src/app.rs`
- new layout helpers under `crates/rs_cam_viz/src/ui/`

Tasks:

- replace the current `editor` vs `simulation` layout split with three explicit layouts
- recommended layout model:
  - `Setup`: left setup list, right setup details, center viewport
  - `Toolpaths`: left operation queue, right operation/tool parameters, center viewport
  - `Simulation`: left run/check summary, right diagnostics, bottom timeline, center viewport

Acceptance criteria:

- setup editing no longer shares the same left/right panel structure as toolpath authoring by accident
- simulation no longer inherits unrelated authoring controls

## Phase 3: Setup workspace redesign

Status target: first real UX payoff phase

### 7. Replace setup parameter soup with guided sections

File targets:

- `crates/rs_cam_viz/src/ui/properties/setup.rs`
- `crates/rs_cam_viz/src/ui/project_tree.rs`
- possibly new files:
  - `crates/rs_cam_viz/src/ui/setup_workspace.rs`
  - `crates/rs_cam_viz/src/ui/setup_cards.rs`

Tasks:

- stop presenting setup as a long generic property form
- create setup sections/cards:
  - orientation
  - zeroing / datum
  - workholding
  - alignment / registration
  - setup notes / operator instructions
- add a compact setup summary card showing:
  - face up
  - z rotation
  - datum mode
  - fixture count
  - keep-out count
  - pin count

Acceptance criteria:

- a user can understand setup state from summary cards without opening every form section
- orientation and datum are visually elevated above fixture micro-fields

### 8. Make orientation and datum task-based

File targets:

- `crates/rs_cam_viz/src/ui/properties/setup.rs`
- `crates/rs_cam_viz/src/app.rs`
- viewport interaction files added in later phases

Tasks:

- replace tiny rotation toggles with larger orientation choices
- present operator-facing instructions adjacent to the chosen setup orientation
- make datum selection task-based:
  - choose datum strategy
  - then configure the chosen strategy
- stop showing irrelevant controls for inactive datum modes

Recommended UI model:

- `Orientation` card with face presets and rotation presets
- `Work Offset` card with:
  - `Corner probe`
  - `Center of stock`
  - `Alignment pins`
  - `Manual`

Acceptance criteria:

- the setup flow reads as a sequence of decisions, not a parameter dump

### 9. Move workholding into a clearer manager

File targets:

- `crates/rs_cam_viz/src/ui/properties/setup.rs`
- `crates/rs_cam_viz/src/ui/project_tree.rs`
- new file if helpful: `crates/rs_cam_viz/src/ui/workholding_panel.rs`

Tasks:

- group fixtures and keep-outs under a proper workholding section
- show each item as a card or list row with status, dimensions, and action buttons
- add strong labels for fixture type and enabled state
- remove the current weak visual distinction between fixtures and keep-outs

Acceptance criteria:

- workholding reads as its own subdomain within setup authoring
- adding/editing a fixture does not feel like editing a generic hidden child object

## Phase 4: Toolpath workspace redesign

Status target: after setup workspace is stable

### 10. Give toolpaths their own operation queue

File targets:

- `crates/rs_cam_viz/src/ui/project_tree.rs`
- `crates/rs_cam_viz/src/ui/properties/mod.rs`
- new files if needed:
  - `crates/rs_cam_viz/src/ui/toolpath_queue.rs`
  - `crates/rs_cam_viz/src/ui/toolpath_workspace.rs`

Tasks:

- stop making the generic project tree carry the full burden of operation management
- create a proper operation queue panel with:
  - grouped by setup
  - clear status chips
  - enable/disable
  - reorder
  - duplicate
  - generate selected
  - generate all in current setup
- keep tools visible as related context, but not mixed into the same crowded tree

Acceptance criteria:

- operation authoring is possible without constantly traversing a mixed project tree
- setups, tools, and paths feel visually distinct

### 11. Reduce parameter density in toolpath editing

File targets:

- `crates/rs_cam_viz/src/ui/properties/mod.rs`

Tasks:

- keep the current operation-specific parameters, but group them into higher-level sections
- default-collapsed advanced sections:
  - manual g-code
  - dressups / modifications
  - advanced heights
  - advanced boundary controls
- keep the generate/status area fixed and prominent

Acceptance criteria:

- common authoring path is visible without scrolling through every advanced option
- advanced controls remain available without dominating the panel

## Phase 5: Simulation workspace redesign

Status target: after state split, before heavy viewer interactivity

### 12. Make simulation a dedicated verification workspace

File targets:

- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `crates/rs_cam_viz/src/ui/sim_timeline.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/preflight.rs`

Tasks:

- make `Simulation` the only place where simulation transport, stock playback, and verification live
- give it explicit empty/stale/up-to-date states
- move export-readiness messaging into this workspace

Recommended split:

- `Preview` subview:
  - operation list
  - collision summary
  - stock display modes
  - run/update checks
- `Animate` subview:
  - transport
  - timeline
  - playhead-linked tool model
  - setup transition markers

Acceptance criteria:

- a user can understand the difference between “generate toolpaths” and “verify toolpaths”
- export readiness is anchored in verification, not a modal detour from anywhere

### 13. Rebuild preflight around explicit check cards

File targets:

- `crates/rs_cam_viz/src/ui/preflight.rs`
- `crates/rs_cam_viz/src/state/simulation.rs`

Tasks:

- convert the checklist into named checks with explicit status:
  - simulation result state
  - rapid collisions
  - holder clearances
  - cycle time
  - operation readiness
- add links/actions that open the relevant workspace section
- stop using `Fix Issues` as a vague jump target

Acceptance criteria:

- every preflight warning points to a clear remediation path
- the user does not have to guess whether a warning belongs to setup, toolpaths, or simulation

### 14. Remove or finish misleading simulation display controls

File targets:

- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/compute/worker/execute.rs`

Tasks:

- either implement actual deviation and per-operation coloring or hide those options for now
- do not expose placeholder visualizations as if they are final analysis modes

Acceptance criteria:

- no visible simulation display mode is knowingly fake

## Phase 6: Viewport interaction foundation

Status target: separate but related project after core workspace redesign

### 15. Build a generalized picking layer

File targets:

- `crates/rs_cam_viz/src/app.rs`
- render modules as needed
- possibly new files:
  - `crates/rs_cam_viz/src/interaction/picking.rs`
  - `crates/rs_cam_viz/src/interaction/hit_proxy.rs`

Tasks:

- replace toolpath-only click picking with an extensible hit system
- support pick targets for:
  - toolpaths
  - fixtures
  - keep-outs
  - alignment pins
  - stock corners / stock faces
  - simulation collision markers

Acceptance criteria:

- viewport picking is no longer hardcoded to toolpath nearest-point sampling only

### 16. Add direct setup manipulation in the viewport

File targets:

- new setup interaction modules
- `crates/rs_cam_viz/src/ui/properties/setup.rs`
- `crates/rs_cam_viz/src/app.rs`

Tasks:

- click stock corners to set XY datum when appropriate
- click stock face to set or confirm Z datum/orientation
- drag fixture and keep-out rectangles in XY
- place alignment pins with click-to-add
- highlight the active setup frame in the viewport

Acceptance criteria:

- a user can complete common setup tasks without typing every coordinate manually

### 17. Add direct simulation interaction in the viewport

File targets:

- `crates/rs_cam_viz/src/ui/sim_timeline.rs`
- `crates/rs_cam_viz/src/app.rs`

Tasks:

- click collision markers to jump to the offending move
- hover operation segments to preview the operation in the viewport
- click setup transition markers to jump and reframe

Acceptance criteria:

- simulation review is interactive enough to diagnose issues quickly

## Phase 7: Stability and regression coverage

Status target: parallel to all phases, mandatory before shipping

### 18. Add controller-level regression tests for workspace behavior

File targets:

- `crates/rs_cam_viz/src/controller/tests.rs`

Tasks:

- test that simulation results do not replace editor panels outside the `Simulation` workspace
- test workspace switching behavior
- test stale simulation state after edits
- test reset semantics

### 19. Add regression tests for the multi-setup simulation bug

File targets:

- `crates/rs_cam_viz/src/controller/tests.rs`
- compute tests as needed

Tasks:

- add a two-setup fixture job
- verify setup transition metadata
- verify per-setup transform usage in playback/display code

### 20. Add crash instrumentation before large UI work continues

File targets:

- `crates/rs_cam_viz/src/main.rs`
- `crates/rs_cam_viz/src/app.rs`

Tasks:

- add panic/backtrace logging around desktop startup
- replace the most sensitive render-path `unwrap()`s with guarded logging where possible
- log active workspace, sim state, and current operation when simulation/toolpath failures occur

Acceptance criteria:

- future crash reports contain enough context to localize the failure path

## Recommended execution order

1. Phase 1.1 and 1.2: state split and workspace introduction
2. Phase 1.3: multi-setup simulation alignment bug fix
3. Phase 1.4: collision semantics cleanup
4. Phase 2: top-level workspace navigation and layout split
5. Phase 3: setup workspace redesign
6. Phase 4: toolpath workspace redesign
7. Phase 5: simulation workspace redesign and preflight rebuild
8. Phase 6: viewport interaction foundation
9. Phase 7: hardening, logging, and regression coverage

## Suggested branch strategy

- one feature branch for the full initiative is acceptable, but use checkpoint commits after each phase
- do not batch Phase 1 and later UX polish into one giant commit
- recommended checkpoint boundaries:
  - workspace/state split
  - multi-setup simulation fix
  - workspace navigation/layout
  - setup workspace
  - toolpath workspace
  - simulation workspace
  - viewport interaction foundation

## Manual QA expectations

At minimum, manually verify:

- single-setup editing still works in all three workspaces
- a stale simulation result does not hijack non-simulation workspaces
- a top/bottom two-setup job shows the second setup in the correct frame
- preflight status is understandable without prior product knowledge
- collision actions clearly state what they check
- setup editing feels possible without hunting across tiny controls
- toolpath authoring feels separated from setup authoring

## Final note for the implementing agent

Do not treat this as a pure UI refactor. The first deliverable is a cleaner state and workspace model. If that is done correctly, the panel redesign becomes much simpler and the current UX confusion drops immediately even before the full viewer-interaction project lands.
