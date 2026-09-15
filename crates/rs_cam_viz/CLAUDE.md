# rs_cam_viz instructions

`rs_cam_viz` owns the egui desktop application, controller, compute workers,
rendering and the GUI-embedded MCP server. Read root `CLAUDE.md` first; read
`../rs_cam_core/CLAUDE.md` when a UI change crosses a core contract.

## GUI/controller contract

- GUI state is not an alternate data model. Mutate core through
  `ProjectSession::apply(Command)` and use `Effects.stale`; do not recompute a
  narrower stale answer in the UI or MCP reply.
- When GUI state gains a field, audit setup-sheet, project IO, controller test
  fixtures and MCP/UI initialisers.
- Preserve the controller → worker → result-acceptance path. A UI control that
  changes generation inputs must drop affected results, not merely paint a
  stale label.
- Generated worker IR and post-simulation emitted IR can differ. Surfaces must
  name which evidence they report; do not label a planned reading as emitted.

## Simulation UI

- The Simulation workspace has one visible full-run primary. State surfaces
  may explain stale/missing evidence but must not add a duplicate run route.
- `simulation_request_is_buildable` mirrors the controller's group builder;
  use it for every Run Simulation affordance rather than re-deriving a simpler
  "has generated toolpath" check.
- Metric capture staleness is derived from the accepted run's capture revision.
  Do not reintroduce a mutable stale boolean or clear it on an un-stamped,
  cancelled or failed result.
- Simulation presentation reads bounded triage first. `NotMeasured` is an
  abstention, not a healthy zero.

## Viewport and UI state

- The selected toolpath is the default viewport draw set. Visibility still
  bites in selected-only mode; if a UI exposes the eye, it must remain usable
  whenever a hidden selection would draw nothing.
- Keep overlay registration, MCP overlay handling and the panel on the same
  registry. An unavailable overlay must be refused with a reason, never
  silently accepted.
- UI declutter work removes controls rather than hiding duplicate routes.
  Consult `planning/ui_review_2026-09-14/` before touching the open UR4/UR5/
  UR8 packages.

## Embedded MCP server

The server lives here, not in `rs_cam_mcp`. MCP mutations dispatch through the
same command path as the UI. For live work:

1. Load; inspect model, stock and machine.
2. Review toolpaths/parameters; generate (fixpoint + explicit simulation cell
   for rest chains); simulate; read triage; then inspect traces only to drill
   down.
3. A setter stales results—regenerate before exporting. Tool/model rebinding
   uses its dedicated endpoint, not a generic operation parameter setter.
4. Export refuses missing/stale enabled geometry and unsafe/unmodelled load
   verdicts unless the caller explicitly accepts the relevant override.
5. Use `generation_status` and `cancel_generation` for long jobs. Do not queue
   a second generation behind an unknown long-running first one.

MCP screenshots are visual evidence, not a replacement for core tests. Capture
only after the GUI frame has applied the requested view state.

## Tests

Use focused `rs_cam_viz` integration tests first. Source-scanning tests need a
non-vacuity anchor and must not match comments when asserting removed UI.
Prefer behavioural egui harness coverage when a contract can be rendered.
