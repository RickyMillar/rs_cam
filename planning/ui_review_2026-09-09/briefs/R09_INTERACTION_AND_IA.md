# R09 — Viewport, navigation and information architecture

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`, then the original
`../PROMPT.md` as a **visual checklist**, not a competing whole-product brief.
Execute only R09. Write `../results/R09/{REPORT,trace}.md` and evidence.
Source paths are repository-relative.

## Outcome

Menus, panels, viewport, overlays and keyboard act as one understandable tool.
The user can find a capability, see the right context, operate it accurately
and go deeper without the normal path drowning in advanced controls.

## Task cards

1. **Context tour:** on small F1/F3 and later dense F9, ask the user to identify
   project, setup, model, tool, operation, selected region and current evidence.
   Switch workspaces/selection and return. Does selection, camera, isolation or
   a modal steal context? Can the user recover without resetting the job?
2. **Find by goal:** ask “show what stock remains,” “can this cutter reach that
   valley?”, “find the unsafe move,” “show only this operation,” and “plan a finer
   tool for these details.” Record routes without revealing correct labels.
   Work with R05/R06 to ensure these are different questions, not one red overlay.
3. **Point and manipulate:** actually click faces/holes/toolpaths, hover, orbit,
   pan, zoom, fit and use view presets. Drag/reorder operations and scrub timeline
   markers. Test visible target sizes, occlusion and click/drag ambiguity. Do not
   infer manipulation usability from `set_ui_view` state changes.
4. **Overlays:** use the current registry inventory and original prompt A/B.
   Test discovery, disabled reasons/compute actions, exclusivity, pin/float,
   workspace defaults, legend units and scope, unavailable renderers, visibility
   versus enabled inclusion, and two simultaneously visible models. Inspect
   animated transitions for flicker; one still image cannot establish absence.
5. **Presentation at decision points:** capture critical authoring, warning,
   diagnosis, planner and export states at 1400×900 and 2400×1300 logical points.
   Check real DPI/scaling, scrolling, truncation, tiny numeric fields, dialog
   resizing, focus visibility, contrast and non-colour cues. Cover long names
   and large operation counts. Assess colour-vision robustness, but do not claim
   formal accessibility conformance without the appropriate tests.
6. **Keyboard and errors:** follow a representative route with keyboard input,
   including text editing, Escape/cancel, undo, menu shortcuts and viewport keys.
   Check focus traps and shortcuts firing while typing. Inspect native dialogs
   through desktop capture or a human, not app-only offscreen screenshots.
7. **Power-user repeat:** repeat a familiar task via contextual actions/shortcuts.
   Compare friction with the normal route and identify efficiency worth keeping.

## Specific questions

- Is there one authoritative setting with multiple legitimate access points,
  or multiple controls with conflicting scope/defaults/behaviour?
- Are actions, status badges, warnings, read-only mirrors and inherited values
  visually distinct and named consistently?
- Is the next useful detail easy to discover without an all-controls dump?
- Does the same colour carry incompatible meanings at once? Can a user identify
  model/stock/move field and legend without memorizing a palette?
- Are important controls hidden on right-click, collapsed sections or disabled
  states with no discoverable remedy? Conversely, is irrelevant detail dominant?
- Are advertised overlays/actions actually wired to a visible effect today?

## Source anchors

- `crates/rs_cam_viz/src/ui/{workspace_bar,menu_bar,shortcuts_window,viewport_overlay}.rs`.
- `crates/rs_cam_viz/src/ui/overlays/{registry,panel}.rs`: current inventory.
- `crates/rs_cam_viz/src/app/viewport.rs`, `src/interaction/picking.rs`, `src/render/`.
- `crates/rs_cam_viz/src/ui/components/`, `theme.rs` and `src/app.rs` layout.
- `planning/ui_overlays_ux_2026-09-08.md` and
  `planning/ui_overlays_dead_duplicate_2026-09-08.md`: historical baseline claims
  to verify, not a current bug list.
- `planning/ui_audit/` maps/targets: reuse categories, retire stale assumptions.

## Deliverable additions and boundary

Produce a **goal → capability → authoritative home → alternate route → prerequisite
→ feedback** map, plus a term/colour consistency ledger and interaction coverage.
Link each proposed IA change to observed journey costs and state its expert cost.

R01–R08 own task completion and domain decisions. R09 cross-links their findings
and owns common interaction patterns; it does not re-run their full audits or
start a global redesign solely from aesthetic preferences.
