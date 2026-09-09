# Current interface — verified interaction and ownership map

Source-led expert review at HEAD **4af7dd96**, 2026-09-09. This describes the
current source, not a newly exercised GUI build. Earlier screenshots are labelled
as earlier-build evidence. Paths below start at `crates/rs_cam_viz/src/` unless
explicitly prefixed with a different crate. Worker drafts are not authoritative;
see [VERIFICATION.md](VERIFICATION.md) for corrections and evidence boundaries.

## 1. Shell: four workspaces, two different kinds of context

| Workspace | Left | Centre | Right | Other |
|---|---|---|---|---|
| Setup | Stock/machine cards, setup cards, Models disclosure | Viewport | Shared selection-dispatched properties | Bottom status/lane strip |
| Toolpaths | Operation queue, Add, project tools/library section | Viewport | The SAME selection-dispatched properties | Bottom status/lane strip |
| Simulation | Capture/resolution/run controls and simulated operation list | Viewport/playback | Dedicated diagnostics: status, focused card OR project/playing/span sections | Resizable bottom timeline/signals |
| Readiness | — | Centred checks dashboard, maximum content width560 | — | Bottom status/lane strip; no viewport |

Source: `app.rs:253-455`, `ui/workspace_bar.rs:6-54`.
Setup/Toolpaths left panels default to240 and right panels280 logical points;
Simulation left/right default240, timeline360 high (resizable, capped480).
Sidebars use vertical ScrollAreas. These are defaults, not fixed screen sizes.

**Workspace and selection are not the same state.** A tab click emits
`SwitchWorkspace`, which changes workspace and applies/restores that workspace's
overlay defaults. It does NOT clear or select an object. Setup and Toolpaths
then dispatch the same properties renderer by `Selection`. A setup inspector
can consequently remain beside the operation queue, or an operation inspector
beside setup cards. This can preserve expert context; it also needs explanation.

- `app/input.rs:71-76` → `ui/overlays/registry.rs:1359-1395`.
- `ui/properties/mod.rs:204` onward → selection match.
- `controller/events/model.rs:23-49`: selection change refreshes relevant GPU data.

The tab bar names all four workspaces. The Workspace menu currently names only
Setup, Toolpaths, Simulation (`ui/menu_bar.rs:213-229`). Readiness remains available
by tab; this omission is not a claim that Readiness is unreachable.

## 2. Human control → state transition → feedback

| Intent / starting state | Actual entry and condition | State/effect | Visible feedback / next route |
|---|---|---|---|
| Begin an empty job | Default Toolpaths; File import, or Setup→Models | GUI import handler adds/selects model; normal import dispatch can fit view | Model inspector; Getting started list no longer applies after input exists |
| Set board/machine | Setup cards→Edit stock / machine | `Select(Stock/Machine)`; properties edits project/session settings | Right editor; Setup card summaries. Return to Toolpaths is a separate action |
| Select machining setup | Setup/fixture/toolpath selection | `handle_select`; toolpath owning setup supplies context | Setup card/operation selection + overlays; not an explicit global setup selector in every dialog |
| Add an operation | Toolpaths Add menu; geometry-gated strategy entries | Select setup then AddToolpath; configured tool/model defaults and compatibility checks | New selected op or refusal; explicit MCP IDs do not exercise these defaults |
| Configure the cut | Selected op header + Geometry tab | Editable config; selected fields have recommendation actions | Purpose line, value fields, pattern diagram, Generate validation |
| Choose cutting speed | Feeds & Speeds tab | Inline fields, speed-only apply, cut-geometry apply | Values/warnings; modal offers larger compare/explore/evidence workspace |
| Generate | Inspector Generate, queue/menu/shortcut alternatives | Submit selected/all operations | Pending/computing/done/waiting/error; selected inspector includes wrapped error text |
| Inspect an op in simulation | Queue Sim/Inspect route | Switch to Simulation; jump to existing boundary, else simulate selected op when it has a result | Playback at that op, possibly only a subset simulated; not merely a view switch |
| Click a simulated op name | Simulation left-list name | `SimJumpToOpStart`; changes playback focus, NOT editor selection | Now-playing toolpath and signal focus follow the new playhead |
| Locate a hotspot | Top-hotspots row or marker/navigation route | Focused hotspot/issue + jump | Focused card replaces project/playing/span body; offers Jump/Optimize and clear/other navigation |
| Manually correct a diagnosis | Switch to Toolpaths, select the responsible op, edit | Editor selection may differ from what was playing; generation/evidence invalidation | Generate; return to Simulation and re-run. No focused-card Edit-operation+return bookmark found |
| Inspect a locked region while scrubbing | Selected section lock/follow control | Explicit `span_scope` lock independent of advancing playhead | Lock glyph and labelled Selected section; keep this capability |
| Recover from stale simulation | Left framed card, timeline Re-run, viewport Re-run | `RunSimulation` | New evidence replaces old; existing stale warnings already work in these surfaces |
| Cancel active compute | Viewport activity strip→Cancel All | `CancelCompute` | Activity-dependent control, not permanently present; no viewport on Readiness |
| Check/export | Readiness→Export goes to preflight; File→Export opens wizard | Different dispatch routes/settings surfaces | Readiness checks vs seven-stage configuration/preview/save; same wording does not imply same route |

Anchors: `ui/toolpath_panel.rs::add_toolpath_menu`; `controller/events/toolpath.rs::handle_add_toolpath`
and `:426-451` (Inspect); `ui/properties/mod.rs:3610-3690`; `ui/sim_op_list.rs:288-302`;
`ui/sim_diagnostics.rs:18-88,200-330,550-614`; `ui/sim_timeline.rs:474-488`;
`ui/viewport_overlay.rs:139-179`; `app/input.rs:132-163`.

## 3. Three useful current journeys

### First input to a configured operation

```text
Toolpaths empty state
  → File import (normal GUI route; not MCP mutation)
  → model selected, input inspector
  → Setup tab → Stock/Machine/setup selection → configure
  → Toolpaths tab → add/select project tool → Add strategy
  → operation selected → Tool/Input + Geometry
  → Feeds / Heights / Linking as needed → Generate
```

The app exposes the pieces; it does not keep a task-aware route visible through
all intermediate states. Getting started exists only when both models and
operations are absent (`ui/properties/mod.rs:217-242`). Its last step still says
“Generate and export G-code”. The post-import selection-none fallback says
“Select an item in the project tree”, although that is not a visible panel title.
These are source facts; whether users miss the route is a design hypothesis.

### A simulation observation back to its responsible settings

```text
Toolpaths: edit selection A
  → Simulation: play/jump to B → inspect B's hotspot
  → choose manual correction, rather than Optimize
  → Toolpaths: editor selection can still be A
  → explicitly select B → edit → Generate
  → Simulation → Re-run → locate corresponding fresh evidence
```

The missing connection is not a Re-run button: several exist. It is a
**scope-preserving edit-and-return route**. The focused simulation card offers
Optimize; that is not the same task as deciding a target, tool, entry style or
stock boundary is wrong. `Esc` returns to Toolpaths when no widget has focus
(`app/input.rs:567-622`), but does not select B for editing.

Simulation focus is simpler than one worker reported:
`state/simulation.rs:1053-1067` makes `focused_toolpath()` follow the current
playback boundary. It is NOT another independent sticky selection. Span lock
and generator-item pin are additional explicit drill-down mechanisms.

### Expert proposal to project changes

```text
Select an op/setup context → Toolpath menu → Plan multi-tool finishing
  → choose tools/dials → Preview → inspect territory
  → Apply plan → emitted planner-owned operations → Generate All
  → Simulation/evidence → manually refine a tier OR revise the planner
```

Current planner Apply requires a ready preview. On successful Apply, dialog closes
and territory overlay stays. Close without Apply drops the overlay without
changing operations; it retains planner state in memory. Replacement count is
reported after Apply (`controller/events/planner.rs:195-258`). The planner target
currently derives from a selected toolpath's setup, else setup0 (`:318-332`).
This makes named scope and before-Apply consequences important even when all
replacement/invalidation logic is correct.

## 4. Value/action ownership — same shapes, different consequences

| Surface | Current role | Important distinction |
|---|---|---|
| Geometry numeric field | Stored cut setting, direct editing | Depth/stepover is the plan, not measured delivered quality |
| Per-field lightning action | Accept a recommendation for that field | An action, not merely a provenance indicator; existing correctness leads remain separate |
| Feeds SPEED Apply | Changes recommended speed-related settings | Intended to preserve cut geometry |
| Feeds CUT Apply | Changes cut geometry | Must predict changes to depth/pass/stepover, not only speed |
| Feeds compare/explore modal | Analysis and temporary candidate controls plus explicit apply | Compare/preview is not the currently configured operation |
| Spindle inheritance control | Default versus explicit override | Default origin and selected value are different facts |
| Feed provenance | Per-field recorded source exists in core | `rs_cam_core/src/feeds/provenance.rs`; don't propose inventing the entire data model anew |
| Tool editor | Draft with Apply/Revert; navigating away commits changed draft | Different commit boundary from direct op edits; `properties/mod.rs:87-140` |
| Library Add to project | Snapshot copy | Editing project copy is not editing catalog; catalog Save/Delete are separate writes |
| Planner Preview/Apply | Temporary territory versus project replacement | Closing preview is not undoing an already-applied plan |
| Export wizard controls | Write session fields immediately; Close hides dialog | Do not claim Cancel rolls back; `app/input.rs:153-228` |

Wizard Post and setup-pause fields write different owners from wizard overrides.
Current project loading constructs `WizardState::default()`
(`rs_cam_core/src/session/project_file.rs:1104`). Merely writing a session field
and marking dirty does not establish save/reopen persistence. Any proposed
“saved with project” label requires verified serialization for that field.

## 5. Current versus historical architecture

Retain the earlier audit's useful questions, not its inventory as truth:
- There are now FOUR workspaces, not the older three-workspace description.
- Current operation tabs are Geometry, Feeds & Speeds, Linking, Heights, Dressup.
- Display controls are now owned by the Overlays registry/panel; the Simulation
  inspector View paragraph directs users there, rather than editing duplicates.
- Feed provenance types already exist; do not re-propose them as absent.
- Modal compare rows do not imply every old per-field Apply button is still there.

This map leads to [PROPOSED_FLOW.md](PROPOSED_FLOW.md). It does not establish
keyboard hit-testing, hover legibility, native dialog behaviour or unaided task
success. No new live GUI requests were made in this source-led pass.
