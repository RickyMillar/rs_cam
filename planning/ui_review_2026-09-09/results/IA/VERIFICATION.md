# Verification and evidence limits — IA pass

Astra synthesis; 2026-09-09. Start/end source HEAD **4af7dd96**. SHA-256 snapshots
of all115 `crates/rs_cam_viz/src/**/*.rs` files show **zero changed files during
this pass** (`source-start.json`, `source-end.json`). This is a source-content
check, not a test/build or proof of which executable another session is running.

## Method / constraints actually followed

- Four DeepInfra `zai-org/GLM-5.2` workers, each restricted to read/grep/find/ls;
  no bash/MCP/write tools, extensions, network research or config access.
- Bounded480-second worker processes; all completed normally, exit0, empty stderr.
  Prompts/outputs/process metadata preserved under `tracks/` and `agents.json`.
- Astra read each draft and independently traced central claims through UI,
  event, controller/state and presentation paths. No new live GUI calls.
- Read existing R03/W02 screenshots and created three labelled annotated copies
  under `annotated/`; read all three resulting images. Original images unchanged.
- All writes for this pass under `results/IA/`, plus a dated STATUS.md append.
  No application/config/library/test changes, builds, tests, commits, fix agents
  or user-process cancellation. Source changes from other sessions not modified.

## Worker corrections / decisions

Raw `T*.DRAFT.md` files are leads, NOT approved reports or implementation specs.
Some path/line references in drafts are inaccurate; use the curated documents'
verified anchors instead. Important reconciliations:

| Draft assertion or proposal | Verified result / synthesis decision |
|---|---|
| T1: no stale Re-run affordance on timeline | FALSE. `ui/sim_timeline.rs:474-488` renders Re-run; viewport and left-panel routes also exist. The gap is scope-preserving manual correction, not another run button |
| T1: “observed human interaction” paragraph | NO human interaction was observed. Reclassified as code-informed hypothesis; never used as a completion measurement |
| T1: all four workspaces in tab bar, only three in Workspace menu | Confirmed (`ui/workspace_bar.rs:13-37`, `ui/menu_bar.rs:213-229`). Minor consistency repair, not a blocked workspace |
| T1: workspace switch preserves selection / shared Setup-Toolpaths inspector | Confirmed (`app/input.rs:71-76`, `ui/overlays/registry.rs:1359-1395`, `ui/properties/mod.rs:204+`). Preserve context initially; reject forcing every cross-scope inspection into a navigation detour without task evidence |
| T3: separate sticky simulation focus alongside playhead | Overstated. `state/simulation.rs:1053-1067` derives focus directly from the playback boundary. Authoring selection is separate; span lock and generator-item pin are explicit additional states |
| T3: no shared scope model / add lock toggle | There IS a follow/locked control and lock glyph in Selected (`ui/sim_diagnostics.rs:1325-1362`). Proposal makes the overall inspected scope clearer; does not rebuild an absent control |
| T3: errors only survive as toast/hover | Overstated. Selected operation inspector renders wrapped `Error: …` and validation messages (`ui/properties/mod.rs:3610-3690`). A persistent multi-event history may help, but errors are not toast-only |
| T3: Cancel button untraced | Located: active-lane viewport strip `Cancel All` → `CancelCompute` (`ui/viewport_overlay.rs:139-159`). Do not call it absent. Readiness has no viewport |
| “Fixed” simulation status header interpreted as screen-pinned | Not established. Header precedes focused-card dispatch (`ui/sim_diagnostics.rs:18-88`), but whole inspector is inside vertical ScrollArea (`app.rs:434-445`). Avoid claiming it cannot scroll away |
| T2: Geometry disclosure collapses when generation completes | `default_open(entry.result.is_none())` is a DEFAULT, not proof of a forced transition of egui's remembered open state. Earlier screenshots show both states; exact automatic behaviour requires input observation |
| T2: add Pocket stock-to-leave as an IA fix | Rejected as capability expansion. Present existing capabilities and consequences; do not add an engine feature through a layout proposal |
| T2: Drill targets as secondary; universal finish quality cues | Targets promoted to Primary. Finish target/nominal cusp is not achieved surface quality; no vendor badge or universal Fine guarantee on geometry-derived fields |
| Older audit: per-field stored provenance data model absent | Stale. `rs_cam_core/src/feeds/provenance.rs:1-55` defines per-field recorded/persisted origins. Reuse reliable existing stamps; presentation/route gaps are separate |
| T4: wizard Preview/Save described as fifth/sixth step | Human-visible order is seven steps; Preview & validate is6, Save7 (`ui/export_wizard.rs:26-69`). Zero-based dispatch indices must not leak into UX descriptions |
| T4: all session wizard settings should be called persistent project settings | Not current fact. Setters mutate session/dirty; Close only hides (`app/input.rs:153-228`). Project load constructs default WizardState (`rs_cam_core/src/session/project_file.rs:1104`). Don't label a field saved-with-project without its serialization contract |
| T4: all previews should discard all state on Close | Unnecessary regression. Planner retains dials while hiding overlay (`controller/events/planner.rs:240-258`). Keep useful memory with a freshness/ownership label; preview must not commit project changes |
| T4: planner Apply requires preview and reports replacement after commit | Confirmed (`ui/multitool_planner.rs:807-824`, `controller/events/planner.rs:195-233`). Use a before-Apply named scope/change summary, not another full planner algorithm audit |
| Proposed Return-to-issue by reusing old move index | Rejected. Regeneration can renumber motion. Bookmark op plus spatial/semantic context and disclose failed remapping; disappearance under unmeasurable checks is not a resolved issue |

## Reviewed source anchors beyond draft claims

- `app.rs:253-455`: current four layouts, panel sizes/scrolling.
- `controller/events/model.rs:23-49`: selection and GPU refresh.
- `controller/events/toolpath.rs:426-451`: Inspect is conditional jump OR
  selected-op simulation; it can change evidence scope, not just navigation.
- `ui/sim_op_list.rs:288-302`: name click jumps playback, not editor selection.
- `app/input.rs:567-622`: Escape return and widget-focus guard.
- `ui/sim_diagnostics.rs:18-88,550-614`: focused-card dispatch; plain
  must-address kind/count grid.
- `ui/properties/mod.rs:87-204,3610-3690,3856-3857,5203-5220`: editing/commit,
  feedback, header default and feed-rate optimization home.
- `controller/events/planner.rs:195-258,318-332`: apply/close/scope semantics.
- `app/input.rs:132-228`, `ui/export_wizard.rs:26-69`: export route/setting ownership.
- `rs_cam_core/src/feeds/provenance.rs:1-55`, `session/project_file.rs:26-63,1104`:
  recorded provenance versus session/project persistence distinctions.

## What this pass can and cannot establish

**Established:** current source arrangement, conditional visibility, representative
widget-event effects, retained context and layout/ownership structure. Existing
screenshots demonstrate concrete earlier-build presentation. Synthesized proposals
are expert IA judgments grounded in those facts.

**Not established:** novice choices, task success rates, timing/savings, subjective
confusion, contrast/accessibility conformance, native-dialog UX, pointer hitboxes,
keyboard focus/drag performance, dynamic collapse/scroll behaviour or an exhaustive
current-binary reproduction of the older defect ledger. No old numerical strategy
ranking or physical safety claim has been adopted.

No blanket human-testing prerequisite was applied: this source-led expert design
assessment is complete enough for design discussion. Short targeted human/input
checks are recommended for its uncertain assumptions, not another broad sweep.
