# Migration map — every capability → new home

The miss-nothing table. EVERY capability id from `CAPABILITY_MATRIX.csv` (368
rows) has a new_home + treatment + note. Domain-proposed rows are merged in;
every capability NOT named by a domain row is assigned here — almost all are
**keep** in their existing authoritative home because they were not implicated in
any finding (stated explicitly per row, grouped under "Not implicated — keep").

Treatment vocabulary: `keep` · `consolidate` · `relocate` · `redesign-affordance`
· `fix-wiring` · `retire`.

---

## A. Toolpath list / queue / card (not implicated — keep)

| capability | new_home | treatment | note |
|---|---|---|---|
| generate-all | Operations Queue / Setup & run | keep | Not implicated. |
| add-toolpath | Add Toolpath menu | keep | Not implicated. |
| add-tool | Tool Library | keep | Not implicated. |
| select-tool-from-library | Tool Library | keep | Not implicated. |
| select-toolpath | Operations Queue | keep | Not implicated. |
| reorder-toolpath | Operations Queue | keep | Not implicated. |
| move-toolpath-to-setup | Operations Queue | keep | Not implicated. |
| move-toolpath-up | Toolpath card context menu | keep | Not implicated. |
| move-toolpath-down | Toolpath card context menu | keep | Not implicated. |
| generate-toolpath | Operations Queue + card | keep | Not implicated. |
| inspect-toolpath-in-simulation | Operations Queue | keep | Not implicated. |
| toggle-toolpath-visibility | Operations Queue / card | keep | Not implicated. |
| toggle-toolpath-enabled | Toolpath card context menu | keep | Not implicated. |
| toggle-isolate-toolpath | Operations Queue / Inspector View | keep | Not implicated. |
| duplicate-toolpath | Toolpath card context menu | keep | Not implicated. |
| delete-toolpath | RemoveToolpath event (card + keyboard + now wired Edit>Delete) | keep | Capability unchanged; P6-001 only wires the dead menu mirror of this event. |
| rename-toolpath | Toolpath properties header | keep | Not implicated. |
| select-tool | Toolpath properties header | keep | Not implicated. |
| select-input-model | Toolpath properties header | keep | Not implicated. |
| select-faces | Toolpath properties header | keep | Not implicated. |
| set-stock-source | Toolpath properties header | keep | Not implicated. |
| toggle-cut-move-visibility | Operations Queue per-row 'C' button, greyed when global Cutting off | redesign-affordance | P4-004: greyed state explains why a toggle has no effect, no prose. |
| toggle-rapid-move-visibility | Operations Queue per-row 'R' button, greyed when global Rapid off | redesign-affordance | P4-004: same gating-legibility as C. |
| show-toolpath-diagnostics | Heights tab / diagnostics ribbon (read) | keep | Read-only per-toolpath diagnostics; not implicated. |
| read-toolpath-status-flags | Operation list | keep | Read-only; not implicated. |

## B. Toolpath tab bar

| capability | new_home | treatment | note |
|---|---|---|---|
| switch-toolpath-tab | Toolpath Tab Bar — relabelled [Geometry][Feeds & Speeds][Linking][Heights][Dressup] | redesign-affordance | Tabs re-chartered to the five P2 concern groups; "Params" catch-all retired. |

## C. Feeds & Speeds — SPEED block

| capability | new_home | treatment | note |
|---|---|---|---|
| set-feed-rate | Feeds & Speeds tab (SPEED block) — the ONE editable home | consolidate | feeds-card + modal become read-only mirrors. P1-002. |
| set-plunge-rate | Feeds & Speeds tab (SPEED block) | consolidate | One editor. P1-002. |
| set-spindle-rpm | Feeds & Speeds tab two-state precedence widget (override) + Post panel (project default) | redesign-affordance | P1-005/P2-006: one widget shows both; Post gets a read-only link. |
| apply-all-feeds | Feeds & Speeds tab — one ⚡⚡ Suggest-all / "Apply recommended speeds" | redesign-affordance | Three bulk buttons collapse to one; SPEED-only by default; doubled glyph. P1-001/P2-002/P4. |
| request-feeds-suggestion-feed-rate | Feeds & Speeds tab — inline ⚡ on feed row | consolidate | Single-field Suggest folded into the one feed row. P1-002. |
| request-feeds-suggestion-plunge-rate | Feeds & Speeds tab — inline ⚡ on plunge row | consolidate | Single-field Suggest on the one plunge row. P1-002. |
| request-feeds-suggestion | Feeds & Speeds tab (pills + chips), detail in Details drawer | consolidate | Single recommendation surface; provenance chip on summary, derate behind drill. P7-005. |
| view-feeds-recipe | Feeds & Speeds tab — one-line recipe chip + "Derived" group + collapsible "How is this calculated?" | redesign-affordance | P3-005/P4-007: formula slab demoted; read-out values become a no-apply Derived group. |

## D. Feeds & Speeds — CUT block (geometry, surfaced from feeds, edited on Geometry)

| capability | new_home | treatment | note |
|---|---|---|---|
| set-stepover | Geometry tab (summary tier); CUT sub-block surfaces a "changes cut" apply | relocate | Cut geometry, not feeds. Never written by Apply-speeds. P2-002. |
| set-depth-per-pass | Geometry tab (summary tier); same | relocate | Geometry, separated from speed apply. P2-002. |
| request-feeds-suggestion-doc | Geometry tab — inline ⚡ on depth-per-pass row (marked "changes cut") | relocate | Now an explicit geometry apply, attributed. P2-002. |
| request-feeds-suggestion-stepover | Geometry tab — inline ⚡ on stepover row (marked "changes cut") | relocate | Explicit geometry apply, attributed. P2-002. |
| set-stock-to-leave | Geometry tab → Advanced section (CUT block surfaces it) | relocate | Geometry; behind drill. P2-002. |

## E. Feeds & Speeds — drills, modals, evidence

| capability | new_home | treatment | note |
|---|---|---|---|
| open-feeds-modal | Feeds & Speeds tab "Details ▸" inline drawer (launcher retained on tab) | redesign-affordance | Modal becomes an in-place read-mostly drawer, not a second editor. P1-003/P3. |
| view-vendor-lut | vendor-lut-viewer (canonical) mirrored read-only in Details drawer / "Vendor cutting data" section | consolidate | One canonical raw table. P1-006. |
| view-engagement-diagram | Feeds & Speeds tab → Details drawer › Charts (collapsed) | relocate | Demoted from always-visible to a drill. P3. |
| set-spindle-strategy | Details drawer (per-op) + feeds-modal-project head row | keep | Spindle policy stays with the feeds engine; unchanged scope. |
| toggle-feeds-provenance | Details drawer › "Why this value?" via clicking a provenance chip | redesign-affordance | Provenance depth pulled to one click from the summary chip. P7-005. |
| set-feeds-explore-point | Details drawer › What-if explorer (collapsed) | relocate | Distinct what-if capability kept; nested as a drill. P3. |
| apply-feeds-explore | Details drawer › What-if explorer "Apply what-if to speeds" | redesign-affordance | Now SPEED-only; stamps [◷ nomogram]. P2-002/P7-004. |
| set-feeds-project-sort | feeds-modal-project | keep | Project rollup retained. |
| toggle-feeds-project-row | feeds-modal-project | keep | Retained. |
| toggle-feeds-project-scatter | feeds-modal-project | keep | Retained. |
| open-optimize-modal | optimize-modal | keep | Distinct sim engine; now provenance-attributed. |
| request-optimize-suggestion | optimize-modal | keep | Distinct engine; unchanged. |
| apply-optimize-candidate | optimize-modal | redesign-affordance | Apply prefixed "◆ Apply sim-optimized"; stamps [◆ sim-optimizer]; geometry change explicit. P4-006/P7-004/P2-002. |
| open-optimize-project | optimize-project | keep | Cross-toolpath sim rollup; kept, attributed. |
| toggle-optimize-project-row | optimize-project | keep | Retained. |
| apply-optimize-project | optimize-project | redesign-affordance | Batch apply engine-prefixed + stamps [◆ sim-optimizer]. P4-006/P7-004. |

## F. Geometry tab — what-to-cut (all relocated from the retired Params tab)

| capability | new_home | treatment | note |
|---|---|---|---|
| set-total-depth | Geometry tab (summary tier) | relocate | Moves with Params geometry. |
| set-op-pattern | Geometry tab (summary tier) | relocate | What-to-cut shape control. |
| set-cut-direction-climb | Geometry tab (summary tier) | relocate | Geometry concern. |
| set-zigzag-angle | Geometry tab (summary tier) | relocate | Drives the pattern; feeds the minimap. |
| set-finishing-passes | Geometry tab → Advanced | relocate | Per-tool/rarely-changed → behind drill. P3. |
| set-profile-side | Geometry tab (summary tier) | relocate | Defining shape control for profile ops. |
| set-cutter-compensation | Geometry tab → Advanced | relocate | Set-once; behind drill. |
| set-tolerance | Geometry tab → Advanced | relocate | P3-002/003: behind drill. |
| set-min-cut-radius | Geometry tab → Advanced | relocate | Behind drill. |
| set-adaptive-cleanup-strategy | Geometry tab → Advanced | relocate | Behind drill. |
| set-slot-clearing | Geometry tab → Advanced | relocate | Behind drill. |
| set-stock-offset | Geometry tab → Advanced | relocate | Behind drill. |
| set-rest-prev-tool | Geometry tab → Rest machining | relocate | Own collapsible. P3. |
| set-inlay-pocket-depth | Geometry tab → Inlay Fit | relocate | P3-003. |
| set-inlay-glue-gap | Geometry tab → Inlay Fit | relocate | P3-003. |
| set-inlay-flat-depth | Geometry tab → Inlay Fit | relocate | P3-003. |
| set-inlay-boundary-offset | Geometry tab → Inlay Fit | relocate | P3-003. |
| set-inlay-flat-tool-radius | Geometry tab → Advanced | relocate | Per-tool; not in fit summary. |
| set-drill-cycle | Geometry tab → Drill cycle | relocate | Named Drill cycle group. |
| set-drill-peck-depth | Geometry tab → Drill cycle | relocate | With drill cycle. |
| set-drill-dwell-time | Geometry tab → Drill cycle | relocate | With drill cycle. |
| set-drill-retract-amount | Geometry tab → Drill cycle | relocate | Chip-break retract amount. |
| set-retract-height-op | Geometry tab → Drill cycle, relabelled "Peck retract (R-plane)" | relocate | P2-004: cutting-cycle param, not a clearance plane; apology tooltip deleted. |
| set-pin-spoilboard-penetration | Geometry tab → Drill cycle (alignment-pin) | relocate | Alignment-pin drill geometry. |
| set-chamfer-width | Geometry tab (summary tier) | relocate | Chamfer shape control. |
| set-chamfer-tip-offset | Geometry tab → Advanced | relocate | Behind drill. |
| set-clearing-strategy | Geometry tab (summary tier, adaptive3d) | relocate | Defining strategy control. |
| set-region-ordering | Geometry tab → Advanced | relocate | Behind drill. |
| set-mill-shallow-areas | Geometry tab → Advanced | relocate | Behind drill. |
| set-fine-stepdown | Geometry tab → Advanced | relocate | Behind drill. |
| set-z-step | Geometry tab (summary tier, waterline/3D) | relocate | Per-pass geometry stepdown (not a Z reference plane). |
| set-scallop-height | Geometry tab — Scallop op "Scallop height"; DropCutter "Finish scallop limit (ball-tip)" in Advanced | redesign-affordance | P1-007: two distinct controls relabelled so the term no longer collides. |
| set-slope-range | Geometry tab (summary tier, steep/shallow) | relocate | Defining filter. |
| set-threshold-angle | Geometry tab (summary tier) | relocate | Defining filter. |
| set-pencil-bitangency-angle | Geometry tab → Advanced | relocate | Behind drill. |
| set-pencil-offset-passes | Geometry tab (summary tier, pencil) | relocate | Defining pencil control. |
| set-sampling-resolution | Geometry tab → Advanced | relocate | Behind drill. |
| set-angular-step | Geometry tab (summary tier, radial finish) | relocate | Defining radial-finish control. |
| set-point-spacing | Geometry tab → Advanced | relocate | Behind drill. |
| set-spiral-direction | Geometry tab (summary tier, spiral finish) | relocate | Defining spiral control. |
| set-max-stepdown | Geometry tab (summary tier, ramp finish) | relocate | Defining ramp-finish control. |
| set-project-surface-model | Geometry tab (summary tier, project-curve) | relocate | Defining project-curve input. |
| set-project-direction | Geometry tab (summary tier, project-curve) | relocate | Defining project-curve control. |
| set-project-side | Geometry tab → Advanced | relocate | Compensation side; behind drill. |
| set-tab-count | Dressup tab → Holding tabs | relocate | Holding tabs are cosmetic/post edge work. |
| set-tab-width | Dressup tab → Holding tabs | relocate | With tab-count under Dressup. |
| set-tab-height | Dressup tab → Holding tabs | relocate | With tab-count under Dressup. |

## G. Linking tab (split out of old Dressups)

| capability | new_home | treatment | note |
|---|---|---|---|
| set-adaptive3d-entry-style | Linking tab (reads Adaptive3dConfig) | consolidate | P2-003: one home; generic inert combo retired. |
| set-adaptive3d-ramp-angle | Linking tab → Entry & Exit | relocate | Entry/linking concern. |
| set-adaptive3d-helix-radius | Linking tab → Entry & Exit → Helix (conditional) | relocate | Revealed when entry style is Helix. P3. |
| set-adaptive3d-helix-pitch | Linking tab → Entry & Exit → Helix (conditional) | relocate | With helix radius. |
| set-dressup-entry-style | Linking tab → Entry & Exit | relocate | P2-003: entry style is a linking concern; adaptive3d duplicate retired. |
| toggle-lead-in-out | Linking tab → Entry & Exit | relocate | How the move connects to material. |
| toggle-arc-fitting | Linking tab → Move optimization | relocate | How moves are emitted (G2/G3). |
| toggle-link-moves | Linking tab → Move optimization | relocate | How moves connect. |
| toggle-optimize-rapid-order | Linking tab → Move optimization | relocate | Travel-order linking concern. |
| set-retract-strategy | Linking tab → Move optimization | relocate | How the tool retracts between linked moves. |
| reset-dressups-to-recommended | Linking tab + Dressup tab (role-scoped reset) | relocate | Applies DressupConfig::for_role defaults across both new tabs. |

## H. Dressup tab (cosmetic remainder of old Dressups)

| capability | new_home | treatment | note |
|---|---|---|---|
| toggle-dogbone | Dressup tab | relocate | Cosmetic corner edge work. |
| toggle-feed-optimization | Dressup tab → Path quality | relocate | Feed smoothing is cosmetic path-quality work. |

## I. Geometry tab → Machining boundary (moved out of Dressups)

| capability | new_home | treatment | note |
|---|---|---|---|
| toggle-boundary | Geometry tab → Machining boundary | relocate | Boundary is what-to-cut, not a dressup. |
| set-boundary-inherit | Geometry tab → Machining boundary | relocate | With boundary toggle. |
| set-boundary-source | Geometry tab → Machining boundary | relocate | Boundary geometry. |
| set-boundary-containment | Geometry tab → Machining boundary | relocate | Boundary geometry. |
| set-boundary-offset | Geometry tab → Machining boundary | relocate | Boundary geometry. |

## J. Heights tab

| capability | new_home | treatment | note |
|---|---|---|---|
| set-clearance-height | Heights tab | keep | Single home; well-grouped. |
| set-retract-height | Heights tab | keep | One of the 5 planes. |
| set-feed-height | Heights tab | keep | Stays. |
| set-top-height | Heights tab | keep | Stays. |
| set-bottom-height | Heights tab | keep | Stays. |

## K. Stale-default fix (split by aspect)

| capability | new_home | treatment | note |
|---|---|---|---|
| apply-stale-default-fix | Feeds & Speeds (feed/speed stale via per-field apply chip); Geometry tab inline Fix banner (geometry stale) | redesign-affordance | Feeds-side routes through the one home's apply; geometry-side stays point-of-need with sim-feedback provenance style. |

## L. Spindle-strategy / post feed-vocabulary leak

| capability | new_home | treatment | note |
|---|---|---|---|
| set-high-feedrate | post-processor-panel, relabelled "Rapid-replacement speed" under Rapids/Linking | relocate | Removes "feed" vocabulary leak; stays a post concern. P2-007. |
| toggle-safe-rapids | post-processor-panel (Rapids/Linking heading) | keep | Companion toggle; grouped away from op-feed vocabulary. P2-007. |

## M. Sim & diagnostics — load rollup / pills / visibility

| capability | new_home | treatment | note |
|---|---|---|---|
| read-load-findings | Inspector › Project overview (authoritative) + Verdict HUD (read-only mirror), both fed by ToolLoadReport::summary() | consolidate | P4-001/002: HUD stops calling verdict_counts(); both render identical within/exceeds/unmodeled with visible /T. |
| read-collision-count | Verdict HUD actionable pill (jumps to first collision) + Inspector › Project overview | redesign-affordance | P5-001: pill gains Sense::click → SimJumpToMove; zero-count is read-only safe. |
| read-issue-counts | Verdict HUD actionable pill (jumps to first issue) + Inspector › Project overview | redesign-affordance | P5-001/P6-004: pill jumps via the now-wired events sink. |
| read-trace-count | Verdict HUD (read-only observation pill) | keep | Explicitly NOT given a click — actionable vs read-only roles stay distinct. P4. |
| jump-to-safety-marker | Boundary timeline + Verdict HUD exceeds/collisions pills | relocate | P5-001: pills reuse nearest_safety_marker_move. |
| jump-to-move | Transport/scrubber, Boundary timeline, Issue card + Verdict HUD pills | keep | Underlying SimJumpToMove reused by clickable HUD pills. |
| show-stock | Viewport overlay Show ▼ (sole home), label "Stock" | relocate | P4-005: duplicate Inspector › View checkbox removed. |
| show-cutting-moves | Viewport overlay Show ▼ (sole home), label "Cutting moves" | consolidate | P4-005: two labels for one field collapse to one. |
| show-rapid-moves | Viewport overlay Show ▼ (sole home), label "Rapid moves" | consolidate | P4-005: collapsed to one label/home. |
| stock-opacity | Inspector › View (Stock appearance group) | keep | Appearance, not presence; stays distinct from the relocated visibility toggle. |
| stock-color-mode | Inspector › View Analysis with ByOperation added as a real entry | redesign-affordance | P6-002: ByOperation wired (selectable_value + serde); placeholder Solid arm removed. |
| run-simulation | unchanged authoritative homes + inline mirrors on empty-state card & Deviation-no-data prompt | keep | P5-003/004: point-of-need Run affordances; no new canonical home. |

## N. Setup / fixture / safety wiring

| capability | new_home | treatment | note |
|---|---|---|---|
| set-fixture-position | fixture-properties-panel (under "🛡 Geometry (safety-checked)") | fix-wiring | [CODE] origin_z now feeds CollisionObstacle; Z marked with shield. P6-003. |
| set-fixture-size | fixture-properties-panel (under "🛡 Geometry (safety-checked)") | fix-wiring | [CODE] size_z now defines obstacle box height. P6-003. |
| set-fixture-clearance | fixture-properties-panel (under "🛡 Geometry (safety-checked)") | fix-wiring | [CODE] clearance now inflates all six faces. P6-003. |
| set-fixture-name | fixture-properties-panel (summary tier) | keep | Promoted to always-visible summary alongside clearance-check status pill. P3. |
| set-fixture-kind | fixture-properties-panel (summary tier) | keep | Unchanged role. |
| toggle-fixture-enabled | fixture-properties-panel (summary tier) | keep | Now also gates whether the fixture contributes a CollisionObstacle. P6-003. |
| run-collision-check | menu-bar (authoritative) + fixture-properties-panel point-of-need mirror button | relocate | [CODE] Mirror trigger added; one AppEvent::RunCollisionCheck; now tests fixture boxes. P6-003. |
| setup-two-sided | both triggers kept as mirrors: identical "＋ Two-sided setup" + "Stock ▸" chip; editing home = Stock › Alignment pins | redesign-affordance | [SPEC] P2-005: relabel both triggers, caption → nav chip. |
| set-flip-axis | stock-properties-panel (Alignment pins) | keep | Confirmed single editing home; two-sided triggers only create/navigate. P2-005. |

## O. Menu / post / Z mirrors

| capability | new_home | treatment | note |
|---|---|---|---|
| delete-selected | menu-bar (Edit > Delete Selected, enabled-by-selection, wired) | fix-wiring | [CODE] Replace add_enabled(false) with selection-derived enablement + click → RemoveToolpath. P6-001. |
| set-post-format | session.post_config() authoritative; panel + wizard route through SetPostConfig event; gui.post read-only mirror | consolidate | [CODE] P1-004: panel edit syncs session immediately; closes GUI-vs-MCP staleness. |
| set-safe-z | Post panel (relabelled "Safe-Z (project)") | keep | P2-004: stays editable in Post; apology tooltip removed, meaning carried by the Heights mirror. |
| view-effective-safe-z | Heights tab read-only mirror (hollow pill + → link) + Post panel | redesign-affordance | P2-004/P5: effective post-clamp safe-Z mirrored at the point clearance planes are edited; apology tooltip retired. |

## P. Not implicated — keep in existing home

These capabilities were not named by any finding. Each keeps its current
authoritative home and treatment `keep`. Grouped by panel for legibility.

**Setup / stock / model panels (setup-properties / stock-properties / setup-list):**
select-stock, add-setup, select-setup, select-model, reload-model, delete-model,
view-stock-effective-dims, view-project-readiness, view-estimated-cycle-time,
view-project-diagnostics, rename-setup, set-face-up, view-orientation,
set-z-rotation, set-xy-datum-method, view-xy-datum-method, set-z-datum-method,
set-z-datum-offset, set-datum-notes, scope-setup-models, set-stock-material,
filter-wood-species, view-material-hardness-index, view-material-kc,
set-stock-dimensions, set-stock-origin, toggle-auto-from-model, set-stock-padding.

**Fixture / keep-out add/select/delete:** select-fixture, add-fixture,
delete-fixture, select-keepout, add-keepout, delete-keepout, set-keepout-name,
toggle-keepout-enabled, set-keepout-position, set-keepout-size, remove-fixture,
remove-setup.

**Alignment pins (stock-properties / alignment-pins-section):** set-pin-diameter,
set-pin-position, mirror-pin, remove-pin, add-pin, auto-place-pins,
set-auto-place-pin-count, view-pin-symmetry-warning, view-pin-bounds-warning,
view-pin-count.

**Tool properties (tool-properties-panel):** set-tool-name, set-tool-type,
set-tool-diameter, set-tool-cutting-length, set-tool-flutes, set-tool-helix,
set-tool-corner-radius, set-tool-material, set-cut-direction,
set-vbit-included-angle, set-taper-half-angle, set-shaft-diameter,
set-holder-diameter, set-shank-diameter, set-shank-length, set-stickout,
view-tool-preview, save-tool-to-library, view-holder-not-configured-warning,
duplicate-tool, remove-tool.

**Tool library / catalogs (tool-library-modal):** create-tool-catalog,
select-tool-catalog, rename-tool-catalog, dedupe-tool-catalog,
delete-tool-catalog, search-all-catalogs, filter-tools, select-library-tool,
add-tool-from-library, edit-library-tool, update-library-tool,
delete-library-tool, move-library-tool, view-tool-readonly-detail,
open-tool-library, select-machine, select-post-processor.

**Sim metrics capture / resolution / reset (Setup & run, staleness card):**
capture-cut-metrics, capture-arc-engagement, capture-generator-trace,
sim-resolution, sim-resolution-auto, reset-simulation, read-staleness,
toggle-toolpath-in-sim-selection.

**Workspace nav / playback / scrubber:** go-to-toolpaths, go-to-simulation,
switch-workspace, jump-to-toolpath-start, clear-span-lock, sim-step-backward,
toggle-sim-playback, sim-step-forward, read-elapsed-total-time, playback-speed,
seek-playback.

**Span / semantic-timeline / signal-spine (read-only diagnostics):**
toggle-span-outline, jump-to-span-start, jump-to-span-end, pin-semantic-item,
jump-to-semantic-item-end, show-generator-steps, highlight-active-generator-step,
read-active-semantic-item, read-generator-trace-summary, read-semantic-item-count,
set-span-scope, read-span-subdivision, read-semantic-segments, read-annotations,
read-cut-issue-dots, read-chipload-signal, read-arc-engagement-signal,
read-axial-doc-signal, read-mrr-signal, read-feed-rate-signal,
read-chipload-envelope, read-span-engagement, read-span-chipload,
read-span-axial-doc, read-span-mrr.

**Hotspots / issues (Inspector cards):** read-hotspot-metrics, focus-hotspot,
clear-focused-hotspot, read-issue, focus-prev-issue, focus-next-issue,
read-hotspot-list, read-tool-load-chipload-badge, read-tool-load-power-badge,
read-tool-load-deflection-badge, read-drill-gates.

**Generation metrics (read-only):** read-cycle-time, read-move-count,
read-operation-count, read-cut-distance, read-rapid-distance, read-air-cut-pct.

**Boundary timeline markers (read-only):** read-collision-markers,
read-tool-load-markers.

**Viewport camera / overlay (other toggles, not the relocated visibility set):**
set-view-preset, reset-view, set-render-mode, toggle-projection, show-grid,
show-fixtures, show-polygons, show-collisions, filter-spankind,
show-tool-profile-ghost, toolpath-color-mode, clear-isolation, cancel-compute.

**Verification / export-readiness / collision-check reads:**
read-operations-check, read-holder-clearance-check, accept-unmodeled-load,
accept-exceeded-load, confirm-export-risk, export-gcode, cancel-export.

**Import / file / menu / status-bar:** import-stl, import-svg, import-dxf,
import-step, open-job, save-job, export-gcode-wizard, export-gcode-direct,
export-combined-gcode, export-setup-gcode, export-setup-sheet, export-svg-preview,
quit, undo, redo, optimize-project, show-shortcuts, display-shortcut-reference,
display-model-counts, display-toolpath-progress, display-compute-lanes,
display-sim-available, display-collision-count, display-dirty-flag,
display-readiness-badge, display-sim-stale-badge.

**Export wizard:** wizard-set-step, set-output-layout, set-filename-template,
set-wcs-override, set-units-override, set-safe-z-override (relabel
"Export safe-Z override (this export only)" so it never reads as project Safe-Z —
P2-004 keep), set-dry-run, set-spindle-warmup, set-setup-pause-message,
set-allow-validator-errors, wizard-save, display-post-metadata, set-coolant-mode,
display-tool-change-summary, display-gcode-preview, display-validator-findings,
display-export-summary.

**Automation:** record-widget-state.

---

## Completeness

- **Total capability ids in CAPABILITY_MATRIX.csv:** 368
- **Mapped:** 368 (100%)
- **Unmapped:** 0
- **Retired (capability dropped entirely):** 0

> **Surfaces retired, but no capability lost.** The redesign retires several
> *controls/surfaces* — `feeds-card`, the duplicate `toolpath-tab-suggest-all`
> button, the per-criterion `verdict_counts()` folder, the inert adaptive3d
> generic entry-style combo, the "Params" catch-all tab, two apology tooltips,
> and the HUD redirect-prose tooltips. In every case the underlying *capability*
> survives in a single authoritative home (e.g. the card's per-field Suggest →
> Feeds & Speeds pills; both Suggest-alls → the one Apply-recommended-speeds;
> the inert combo's intent → Linking entry-style reading Adaptive3dConfig). No
> capability id is dropped — only de-duplicated and relocated.

### Requires non-UI change (flagged for the core/wiring owner)

- **ValueProvenance data model** — `set-feed-rate`, `set-plunge-rate`,
  `set-spindle-rpm`, `set-stepover`, `set-depth-per-pass` and the apply machinery
  cannot show legible per-field provenance until each value carries a
  `ValueProvenance` stamp (P7-001/002). The bundled `apply_feeds_result_to_op`
  must also be splittable into SPEED-only vs geometry subsets (P2-002).
- **Fixture-in-collision** — `set-fixture-position/size/clearance`,
  `toggle-fixture-enabled`, `run-collision-check`: `CollisionCheckRequest` gains
  an `obstacles` slot, `run_collision_check` tests fixture boxes, both call sites
  populate them (P6-003).
- **delete-selected** — selection-derived enablement + click handler (P6-001).
- **set-post-format** — `SetPostConfig` event routing so panel edits sync the
  canonical session immediately (P1-004).
