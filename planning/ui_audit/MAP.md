# rs_cam_viz IA Audit — Layer A: Surface Map

> **Legend** — Health flag: 🟢 green = single clear concern, earns its place · 🟡 yellow = works but mixes concerns / has a confusable twin / leans on prose · 🔴 red = duplicate editing home, dead control, or actively-misleading state.
> **Kind**: panel / tab / menu / modal / inline-widget. One row per surface; "Job" is the one-sentence reason the surface exists. Skim this before drilling into `DIAGNOSIS.md`.

## Cluster: Toolpath Queue & Tool Library

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Operations Queue Panel | `crates/rs_cam_viz/src/ui/toolpath_panel.rs` | panel | List the per-setup operation queue with status/visibility; add, reorder, generate, select. | 🟡 |
| Toolpath Card Context Menu | `crates/rs_cam_viz/src/ui/toolpath_panel.rs` | menu | Full per-toolpath action set on right-click. | 🟡 |
| Add Toolpath Menu | `crates/rs_cam_viz/src/ui/toolpath_panel.rs` | menu | Pick an op type to add, grouped 2.5D vs 3D, geometry-gated. | 🟢 |
| Tool Library (collapsible) | `crates/rs_cam_viz/src/ui/toolpath_panel.rs` | panel | List defined tools; add new tools by type. | 🟢 |

## Cluster: Toolpath Properties — Shell & Tabs

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Toolpath Properties Header | `crates/rs_cam_viz/src/ui/properties/mod.rs` | panel | Always-on identity / IO / generate / diagnostics ribbon above the tab bar. | 🟡 |
| Toolpath Tab Bar | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Switch Params/Feeds/Heights/Dressups with per-tab warning badges. | 🟢 |
| Params Tab | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Edit geometry/strategy params with inline LUT Suggest pills + diagram. | 🔴 |
| Feeds Tab | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Computed feeds recipe: per-row Apply, formula, engagement diagram, vendor LUT. | 🔴 |
| Heights Tab | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Edit five Z planes as reference+offset rows plus a draggable side-view. | 🟡 |
| Dressups Tab | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Toggle/configure post-processing dressups + per-toolpath boundary. | 🟡 |

## Cluster: Feeds & Speeds (editing homes — the duplication epicentre)

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| feeds-tab | `crates/rs_cam_viz/src/ui/properties/mod.rs` | tab | Properties-panel home for feeds: launch modal, host legacy card, formula, diagram, vendor viewer. | 🔴 |
| feeds-card | `crates/rs_cam_viz/src/ui/properties/mod.rs` | panel | Cached LUT result with per-field ⚡ Suggest + Suggest-all (admitted transitional legacy). | 🔴 |
| Feeds & Speeds Modal (launcher) | `crates/rs_cam_viz/src/ui/properties/mod.rs` | modal | Open the full-screen modal — a third editor of the same feed/rpm/doc/woc values. | 🔴 |
| toolpath-tab-feed-params | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` | inline-widget | Authoritative editable feed/plunge/RPM/stepover/DOC inputs with ⚡ pills. | 🟡 |
| toolpath-tab-suggest-all | `crates/rs_cam_viz/src/ui/properties/mod.rs` | inline-widget | Third "apply all LUT feeds" button (mirrors Feeds-tab button). | 🔴 |
| feeds-modal-toolpath | `crates/rs_cam_viz/src/ui/feeds_modal.rs` | modal | One toolpath's current-vs-recommended feeds: provenance, derate, 3 charts, apply. | 🟡 |
| feeds-modal-project | `crates/rs_cam_viz/src/ui/feeds_modal.rs` | modal | Project-wide current-vs-recommended rollup; batch-apply LUT recs. | 🟡 |
| feeds-modal-nomogram-explore | `crates/rs_cam_viz/src/ui/feeds_modal.rs` | inline-widget | Drag feed-vs-RPM nomogram what-if, verdict + power, then apply. | 🟡 |
| Params Pattern Diagrams | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` | inline-widget | Value-reactive minimap of the active op's cut pattern. | 🟢 |
| Engagement Diagram | `crates/rs_cam_viz/src/ui/properties/mod.rs` | inline-widget | Visualize recommended radial WOC / axial DOC vs tool cross-section. | 🟢 |
| Vendor LUT Viewer | `crates/rs_cam_viz/src/ui/properties/mod.rs` | inline-widget | Embedded vendor rows filtered to tool family/diameter as evidence. | 🟢 |
| vendor-lut-viewer | `crates/rs_cam_viz/src/ui/properties/mod.rs` | panel | Raw embedded vendor cutting-data rows, read-only reference table. | 🟢 |

## Cluster: Optimizer (sim-based recommendations)

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| optimize-modal | `crates/rs_cam_viz/src/ui/optimize_modal.rs` | modal | Ranked candidate param sets for one toolpath, per-gate verdicts, apply safe one. | 🟡 |
| optimize-project | `crates/rs_cam_viz/src/ui/optimize_project.rs` | modal | Sim optimizer across all toolpaths: bottleneck, per-row saving, batch-apply, reconcile. | 🟡 |
| sim-diagnostics-optimize-entry | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | inline-widget | Contextual Optimize entry points from sim findings. | 🟢 |

## Cluster: Setup, Stock, Fixtures, Tools

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| setup-list-panel | `crates/rs_cam_viz/src/ui/setup_panel.rs` | panel | Browse setups, stock summary, readiness, project diagnostics, Models list. | 🟡 |
| setup-properties-panel | `crates/rs_cam_viz/src/ui/properties/setup.rs` | panel | Edit setup orientation, datum, model scoping, fixtures, keep-out zones. | 🟡 |
| fixture-properties-panel | `crates/rs_cam_viz/src/ui/properties/setup.rs` | panel | Edit one fixture's name/kind/enabled/position/size/clearance. | 🟡 |
| keepout-properties-panel | `crates/rs_cam_viz/src/ui/properties/setup.rs` | panel | Edit one keep-out zone's name/enabled/position/size. | 🟡 |
| stock-properties-panel | `crates/rs_cam_viz/src/ui/properties/stock.rs` | panel | Edit stock material, dims, origin, auto-from-model, alignment pins. | 🟡 |
| material-picker-menu | `crates/rs_cam_viz/src/ui/properties/stock.rs` | menu | Hierarchical Category→Wood→species material drilldown with filter. | 🟢 |
| alignment-pins-section | `crates/rs_cam_viz/src/ui/properties/stock.rs` | inline-widget | Two-sided flip axis + alignment pin positions/diameter (shared across setups). | 🟡 |
| tool-properties-panel | `crates/rs_cam_viz/src/ui/properties/tool.rs` | panel | Edit project tool geometry/material/direction/holder, preview, save to catalog. | 🟡 |
| post-processor-panel | `crates/rs_cam_viz/src/ui/properties/post.rs` | panel | G-code format, spindle speed, safe-Z clearance, safe-rapids substitution. | 🔴 |
| tool-library-modal | `crates/rs_cam_viz/src/ui/tool_library_modal.rs` | modal | Browse/preview/edit/organise/import per-user tool catalogs. | 🟢 |

## Cluster: Simulation — Run & Op List

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Setup & run | `crates/rs_cam_viz/src/ui/sim_op_list.rs` | panel | Choose what the next sim run records and launch the run. | 🟡 |
| Verification empty-state card | `crates/rs_cam_viz/src/ui/sim_op_list.rs` | inline-widget | Tell the user there are no sim results; route to run/toolpaths. | 🟢 |
| Verification staleness card | `crates/rs_cam_viz/src/ui/sim_op_list.rs` | inline-widget | Warn results are stale after edits; one-click re-run. | 🟡 |
| Operation list | `crates/rs_cam_viz/src/ui/sim_op_list.rs` | panel | List toolpaths by setup: include/visibility/jump + per-op status. | 🟡 |

## Cluster: Simulation — Inspector (diagnostics)

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Inspector › View | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Control how the sim looks in the 3D viewport. | 🔴 |
| Inspector › Selection details | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Active semantic item's label/kind/bbox/params. | 🟡 |
| Inspector › Generation Metrics | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Generator phase timings + item counts (build-time). | 🟢 |
| Inspector › Focused hotspot card | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | inline-widget | Clicked hotspot metrics with jump/optimize/clear. | 🟡 |
| Inspector › Focused issue card | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | inline-widget | Air-cut/low-engagement issue at current move; prev/next/jump/optimize. | 🟡 |
| Inspector › Project overview | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Whole-cut summary: totals, pass/warn banner, findings counts. | 🟡 |
| Inspector › Top hotspots list | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Project-wide hotspot triage sorted by wasted runtime. | 🟡 |
| Inspector › Now-playing strip | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | inline-widget | Playing toolpath name + tool-load/drill-gate badges; optimize/jump. | 🟡 |
| Inspector › Selected span section | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | panel | Sample aggregates + in-scope findings for the locked/playhead span. | 🟡 |

## Cluster: Simulation — Timeline

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Transport & scrubber | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | panel | Drive playback: step/play/pause, time, speed multiplier. | 🟢 |
| Verdict HUD | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | inline-widget | Glanceable project rollup of load/collisions/issues/traces as pills. | 🔴 |
| Boundary timeline | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | inline-widget | Painted op-segment bar with safety markers, playhead, click/drag seek. | 🟡 |
| Span ribbon | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | inline-widget | Subdivide scope toolpath into DepthPass/Region blocks; scope + scrub. | 🟡 |
| Semantic timeline band | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | inline-widget | Per-TP semantic segments/annotations/issue dots; click to pin/jump. | 🟡 |
| Signal spine | `crates/rs_cam_viz/src/ui/sim_timeline.rs` | panel | Per-sample chipload/engagement/DOC/MRR/feed tracks for focused TP. | 🟡 |

## Cluster: Diagnostics (cross-cutting)

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Diagnostics Ribbon | `crates/rs_cam_viz/src/ui/properties/mod.rs` | inline-widget | Per-toolpath diagnostics in 3 tiers with optional one-click fixes. | 🟡 |

## Cluster: Viewport, Export, Chrome

| Surface | File | Kind | Job | Health |
|---|---|---|---|---|
| Viewport overlay toolbar | `crates/rs_cam_viz/src/ui/viewport_overlay.rs` | menu | Floating toolbar: camera, render/projection, visibility, isolation, compute, workspace. | 🔴 |
| Export Readiness modal | `crates/rs_cam_viz/src/ui/preflight.rs` | modal | Pre-export checklist gating g-code behind tool-load overrides. | 🟡 |
| menu-bar | `crates/rs_cam_viz/src/ui/menu_bar.rs` | menu | Top-level command menus + global keyboard shortcut registration. | 🟡 |
| menu-file-direct-export | `crates/rs_cam_viz/src/ui/menu_bar.rs` | menu | Bypass wizard, emit g-code straight to file dialog (all/combined/per-setup). | 🔴 |
| workspace-bar | `crates/rs_cam_viz/src/ui/workspace_bar.rs` | tab | Switch Setup/Toolpaths/Simulation workspaces; per-workspace readiness badge. | 🟡 |
| status-bar | `crates/rs_cam_viz/src/ui/status_bar.rs` | inline-widget | Read-only bottom strip: counts, progress, compute lanes, sim, collisions, dirty. | 🟡 |
| project-tree | `crates/rs_cam_viz/src/ui/project_tree.rs` | panel | Primary selection/navigation tree with per-item context actions. | 🟡 |
| export-wizard | `crates/rs_cam_viz/src/ui/export_wizard.rs` | modal | Gated multi-step g-code export (post/layout/coord/tool-change/pauses/preview/save). | 🟡 |
| shortcuts-window | `crates/rs_cam_viz/src/ui/shortcuts_window.rs` | modal | Static read-only keyboard-shortcut reference (drifted from real bindings). | 🔴 |
| automation-snapshot | `crates/rs_cam_viz/src/ui/automation.rs` | inline-widget | Non-visible infra: record widget label/rect/enabled for external automation/tests. | 🟢 |

---

**Tallies** — 60 surfaces. By health: 🔴 11 · 🟡 35 · 🟢 14. The red concentration is the Feeds & Speeds cluster (Params/Feeds tabs, feeds-card, modal launcher, suggest-all) plus the viewport/inspector visibility duplication and a few orphaned/drifted chrome controls.
