# Pass-2 Diagnosis — The Neglected Surfaces

> Pass 1 focused on the feeds/tabs epicenter. Pass 2 audits everything else in
> `rs_cam_viz`: tool management, the simulation Inspector, the optimizer, the
> simulation timeline, and the shell/navigation chrome.
>
> Findings below are the **surviving** set after adversarial refutation. Each was
> re-verified against source; softened findings carry a `refute_note` explaining
> the mitigation and the severity change.
>
> Lens key: S1 cohesion/altitude · S2 summary-first/overview · S3 consistency ·
> S4 dead/unreachable · S5 affordance-vs-prose · S6 discoverability · S7
> freshness/provenance.

---

## Executive summary

The skipped surfaces were **not** clean. Pass 2 surfaced 33 surviving findings
(5 high, 19 med, 9 low), and several are as serious as anything in the feeds
epicenter. The worst NEW problems found outside feeds/tabs:

1. **A whole navigation panel ships dead (SHE-001 / TOO-001, high).** The entire
   446-line `project_tree.rs` — which hoards the complete tool CRUD set
   (Duplicate, Delete, "From library" import, "Manage Library…") — has **zero
   call sites**. It only compiles because it is `pub`. As a direct consequence a
   user **cannot delete or duplicate a project tool from the shipping GUI at
   all**; the only live triggers are the MCP bridge. This is the single most
   damaging finding in pass 2: real, fully-built capability is invisible and
   unreachable.

2. **The optimizer reports a wrong headline cycle time (OPT-001, high).**
   `compute_optimized_cycle()` zeroes out NoSafeImprovement / Skipped rows on a
   false in-code premise ("no baseline access") — but `outcome.rs` confirms
   candidates[0] *is* the baseline. Every refused row understates the optimized
   time and inflates the savings %, so the operator trusts an incorrect
   project-wide claim.

3. **Stale cut metrics presented as fresh in two places (INS-005 + TIM-009,
   high/low).** When a hotspot/issue card is focused the stale banner is on an
   unreachable code path, so concrete peak-chip/DOC/engagement numbers render
   with no freshness cue. The bottom-panel signal spine never checks staleness at
   all — it renders the old trace styled as fresh.

4. **Three look-alike stacked timeline strips with three different X-axes and
   three different click behaviours (TIM-001 / TIM-002, med after softening).**
   Position reads across them are silently wrong. Softened to med only because
   the divergent third strip is debug-gated.

5. **Two always-visible chrome surfaces disagree on the collision count
   (SHE-002 / SHE-003, med).** Status bar prints holder-only collisions while the
   workspace bar prints holder+rapid, and a stale+colliding project can hide the
   ERROR behind a yellow "stale" on the Simulation tab. Same project, two
   numbers, one of them safety-relevant.

### Was this area actually a problem? — verdict per area

| Area | Verdict | Why |
|------|---------|-----|
| **Tool management** | **Genuinely bad** | A complete CRUD home ships dead; delete/duplicate unreachable in GUI; live-vs-Save commit ambiguity on identical forms; vendor/product-id shown-but-uneditable with silent duplicate pileup. 2 high. |
| **Inspector diagnostic cards** | **Genuinely bad** | One undivided always-expanded scroll answering 3 questions at 3 scopes; stale signal on a dead path under card focus; engagement shown with no provenance and in inconsistent units. 2 high. |
| **Optimizer / sim-recommendation** | **Genuinely bad** | Headline cycle time is numerically wrong on a false premise; can run against a stale baseline with no cue; rows the code says to "open the modal" for have no affordance to do so. 1 high. |
| **Simulation timeline / transport** | **Genuinely bad, but partly debug-gated** | Stacked strips with divergent axes/click-contracts (softened by debug gating); prose-only interactivity; bottom panel never reflects staleness; a dead gate-trip drill path. 0 high after softening, but dense with real defects. |
| **Shell / navigation chrome** | **Genuinely bad** | Dead nav panel (high); two divergent collision tallies; severity-ladder inversion hiding errors; overloaded toolpath header with diagnostics buried last. 1 high. |

No skipped area came back **clean**. Every one warranted a redesign spec.

---

## Tool management — 6 findings (2 high, 3 med, 1 low)

Root cause: two live tool homes never reconciled, plus a fully-built-but-dead
third home (`project_tree.rs`) that holds the complete CRUD set.

### TOO-001 — S4 — HIGH — cannot delete/duplicate a tool in the shipping GUI
- Surfaces: tool-library-collapsible, project-tree (dead)
- Evidence: `ui/mod.rs:10` declares `pub mod project_tree;` but `project_tree::draw`
  (`project_tree.rs:12`) has zero callers. `app.rs:232` wires `toolpath_panel::draw`
  as the live tool home. `AppEvent::DuplicateTool`/`RemoveTool` are emitted from UI
  only at `project_tree.rs:122/126` (dead); handled at `controller/events/mod.rs:50-51`.
  `input.rs:441` Delete/Backspace matches `Selection::Toolpath` only, never
  `Selection::Tool`. Only non-dead trigger is the MCP bridge (`mcp_server.rs:746`,
  `app/mcp.rs:429`). Live collapsible (`toolpath_panel.rs:174-192`) offers only
  Select + "+ Add Tool".
- User impact: In the shipping UI a user cannot delete or duplicate a project tool
  at all — the only affordances live in a never-rendered panel.

### TOO-002 — S6 — HIGH — no panel-discoverable path to library manager / import
- Surfaces: tool-library-collapsible, tool-library-modal
- Evidence: Live collapsible (`toolpath_panel.rs:174-192`) = `selectable_label` +
  "+ Add Tool" only. "Manage library…" and "From library" exist only in dead
  `project_tree.rs` (line 104 `OpenToolLibrary`; 142-159 emitting
  `AddToolFromLibrary` at 154). Only live `OpenToolLibrary` emitter is
  `menu_bar.rs:166-168`. Only live `AddToolFromLibrary` emitter is the modal's
  "➕ Add to project" (`tool_library_modal.rs:378`).
- User impact: From the panel where users manage tools there is no discoverable
  path to the library manager or to importing a saved tool; both require knowing
  to use the top menu bar.

### TOO-004 — S4 — MED — vendor/product-id shown but uneditable; silent dup pileup
- Surfaces: tool-properties-panel, tool-library-modal
- Evidence: Modal readonly grid shows Vendor (`tool_library_modal.rs:485-487`) and
  Product ID (488-490), but `draw_tool_fields` (`tool.rs:54-255`) has no editor for
  `vendor`/`product_id` (rg returns nothing). `tool.rs:31` "Save to library" →
  `append_tool` → `append_to` (`tool_library.rs:160-167`): unconditional
  `catalog.tools.push(tool)` + save, no dedup. Only the modal's `UpdateLibraryTool`
  can replace. A `DedupeToolCatalog` affordance exists (`mod.rs:119`).
- User impact: Vendor/Product ID can never be entered from the GUI, and repeated
  panel saves silently pile up duplicate catalog entries that only a separate
  Dedupe button cleans up.

### TOO-003 — S3 — MED — same form, live-apply in one home vs explicit-Save in other
- Surfaces: tool-properties-panel, tool-library-modal
- Evidence: Both share `draw_tool_fields` (`tool.rs:54`). Properties path
  (`properties/mod.rs:229`) calls it on a live `&mut ToolConfig` from
  `session.tools_mut()` — edits mutate immediately. Modal
  (`tool_library_modal.rs:524`) calls it on a cloned draft, commits only on
  "💾 Save" (`UpdateLibraryTool`, 538), Cancel discards (545).
- Refute note (softened high→med, kept med): the live path captures a
  `tool_snapshot` for undo before editing (`properties/mod.rs:222-227`), so live
  edits ARE Ctrl-Z reversible. "Cannot revert" overstated; the mental-model
  inconsistency (live-apply vs explicit-Save on identical forms) stands.
- User impact: The same-looking editor silently applies live in one home but
  requires Save in the other, so users cannot predict whether a change is committed.

### TOO-005 — S3 — MED — "Shaft" vs "Shank" diameter both visible; ambiguous "Corner Radius"
- Surfaces: tool-properties-panel
- Evidence: For TaperedBallNose the grid renders "Shaft Diameter" →
  `tool.shaft_diameter` (`tool.rs:185-187`) while the always-present Holder/Shank
  collapsible renders "Shank Diameter" → `tool.shank_diameter` (`tool.rs:219-221`),
  both visible. Two distinct "Corner Radius" labels: `corner_radius_mm` for EndMill
  (`tool.rs:121-123`) and `corner_radius` for BullNose (`tool.rs:156-158`).
- Refute note (softened to low): the two "Corner Radius" labels are mutually
  exclusive match arms (never co-visible). The load-bearing part is the Shaft/Shank
  co-visibility for a TaperedBallNose, which is real. Lower-severity labeling nit.
- User impact: Users facing both "Shaft Diameter" and "Shank Diameter" cannot tell
  which dimension drives which geometry.

### TOO-006 — S5 — LOW — "collision check skipped" safety state is grey italic prose
- Surfaces: tool-properties-panel
- Evidence: When `tool.holder_diameter < 0.01` the panel renders italic grey prose
  "Holder not configured — collision check will be skipped" (`tool.rs:247-254`).
  The Holder/Shank section is plain `ui.collapsing` (`tool.rs:205`) with no header
  badge, and the warning sits below a default-collapsed section.
- User impact: A safety-relevant "collision check disabled" state is conveyed only
  by easily-missed grey italic text below a collapsed section.

---

## Inspector diagnostic-card ecosystem — 9 findings (3 high, 4 med, 2 low)

Root cause: one undivided always-expanded scroll (`draw_project_overview`)
answering three questions at three scopes (project / toolpath / span), with
confusable cards and divergent metric formats. File: `sim_diagnostics.rs`.

### INS-001 — S2 — HIGH — no summary-first collapse; 6 grids + span block always expanded
- Surfaces: sim-project-overview, sim-now-playing-strip, sim-selected-span-section, sim-top-hotspots-list
- Evidence: `sim_diagnostics.rs:518-890` stacks Global stats (518), verdict banner
  (602), Findings grid (614), Optimize button (665), Must-address (697),
  Informational (720), Top-hotspots header (778), stale warning (819), Now-playing
  strip (830-873), Selected-span section (888) with no collapse. Only Top hotspots
  (778-779) is behind a `CollapsingHeader`. `rg default_open` returns only 4 headers
  (48, 165, 238, 779), none wrapping the overview body.
- Refute note: verdict banner at 602 is a single glanceable line (slight
  mitigation), but the body below it is always-expanded. Stands high.
- User impact: Default Inspector is one always-expanded vertical scroll of ~6 grids
  plus a span block — no glanceable verdict-then-drill-down.

### INS-002 — S1 — MED — three scopes glued into one function with only separators
- Surfaces: sim-project-overview, sim-now-playing-strip, sim-selected-span-section
- Evidence: `draw_project_overview` (`sim_diagnostics.rs:472-890`) renders (a)
  project rollup (518-742), (b) current-toolpath Now-playing strip scoped to
  `current_boundary` (830-873), (c) selected-span block scoped to
  `span_scope/playhead` (888). Three scopes, only `ui.separator()` between.
- Refute note (softened high→med): each block carries an in-context label
  ("Global" 518, "Now playing:" 838, "Selected" 1302) and comments note scope
  (829, 875). Labels disambiguate; "secretly contains / cannot tell" overstated.
- User impact: A project-overview-labelled panel contains per-toolpath and per-span
  detail at different scopes.

### INS-005 — S7 — HIGH — stale banner unreachable when a card is focused
- Surfaces: sim-hotspot-card, sim-issue-card
- Evidence: The stale warning lives only inside `draw_project_overview`
  (`sim_diagnostics.rs:819-826`). But `draw_reactive_inspector` (338-345)
  early-returns after drawing the focused hotspot card (338-340) or issue card
  (341-343), so the overview never runs and the banner is never reached. Cards
  print peak chip/DOC/engagement (393-400) with no freshness cue.
  `rg is_stale` confirms it appears nowhere else in the file.
- User impact: With a card focused, the user sees concrete cut metrics with no
  signal they may be from a stale simulation.

### INS-003 — S3 — MED — hotspot card vs issue card look near-identical in same slot
- Surfaces: sim-hotspot-card, sim-issue-card
- Evidence: hotspot card (`sim_diagnostics.rs:369-418`) vs issue card (431-462):
  both `egui::Frame`, near-identical dark fills (50,38,28 vs 42,36,28), 6.0 margin,
  4.0 rounding, colored title, trailing Jump+Optimize row. Mutually exclusive
  priority slots (338-343) → same screen position.
- Refute note (softened high→med): title colors differ meaningfully — hotspot
  orange RGB(255,170,90) at 378 vs `theme::WARNING_TEXT` at 439; issue card has
  Prev/Next nav the hotspot card lacks, refuting "same trailing button row". Risk
  real but lower than claimed.
- User impact: Two cards with different meanings look like the same card in the
  same spot, inviting class confusion.

### INS-004 — S3 — MED — same hotspot datum rendered 3 ways, engagement % vs raw fraction
- Surfaces: sim-hotspot-card, sim-top-hotspots-list, sim-selected-span-section
- Evidence: focused card (`sim_diagnostics.rs:386-400`) "Moves a–b · N samples" +
  "avg engage {:.0}%"; Top-hotspots row (789-792) "m{} · waste {:.2}s · peak chip
  {:.4}"; Selected-span row (1468-1473) "Hotspot · m{} · waste {:.2}s · peak chip
  {:.4}". Engagement printed as percent (395) in the card but raw 0–1 fraction
  ("avg {:.2}", 1383) in the Selected-span grid.
- Refute note: 1383 is span-aggregate engagement (different denominator), not
  literally the same datum; unit/label inconsistency for a conceptually-identical
  metric stands.
- User impact: Identical values appear in different units and label forms across
  the three places they surface.

### INS-006 — S7 — MED — engagement shown with no provenance caveat
- Surfaces: sim-hotspot-card, sim-top-hotspots-list, sim-selected-span-section
- Evidence: `sim_diagnostics.rs:395` "avg engage {:.0}%" and 1383 "avg {:.2}"
  carry no caveat that `average_engagement` is the cylinder-side radial-WOC fraction
  reading ~10× below the algorithmic target (CLAUDE.md adaptive_review note).
  `rg on_hover_text` shows hotspot rows only get "Click to focus and jump" (800,
  1475); only tool-load verdict badges carry a source tooltip (`verdict_tooltip`,
  1063).
- User impact: Engagement shown as a bare %/fraction with no "comparative-only"
  provenance, so a user reads a low % as an absolute under-engagement defect.

### INS-007 — S2 — LOW — two separate "what is selected" blocks, different sources
- Surfaces: sim-selection-details, sim-selected-span-section
- Evidence: "Selection details" (`CollapsingHeader`, `sim_diagnostics.rs:164-227`)
  describes the active semantic item (`active_semantic_item`); `draw_selected_section`
  (1260-1521) describes the active structural span
  (`span_scope.span_id.or(playhead_span_id)`). Both answer "what am I looking at",
  no cross-reference.
- Refute note (softened med→low): "Selection details" is `default_open(false)`
  (165) so it doesn't compete by default; the two blocks describe orthogonal axes
  (semantic generator item vs structural span) a power user may want at once;
  comment at 157-162 documents the split.
- User impact: Two "current selection" panels with different sources in one panel.

### INS-008 — S6 — MED — Selected panel's lock has no in-panel entry point
- Surfaces: sim-selected-span-section
- Evidence: The Selected section's scope can be locked, but the lock is set by the
  timeline ribbon (`sim_timeline.rs:1416` writes `span_scope.span_id`). In-panel
  there's only a passive "· locked" tag (`sim_diagnostics.rs:1314-1319`) plus a
  "Follow playhead" unlock button that appears ONLY when already locked (1320-1330).
  No in-panel lock control.
- User impact: The panel's primary mode (pin to a chosen span) has no entry point
  on the panel — only an after-the-fact unlock — so the lock is hidden from anyone
  not already using the ribbon.

### INS-009 — S3 — LOW — four identically-labelled "Optimize" buttons, different scopes
- Surfaces: sim-hotspot-card, sim-issue-card, sim-now-playing-strip, sim-project-overview
- Evidence: hotspot card "Optimize"→`OpenOptimizeModal(toolpath_id)`
  (`sim_diagnostics.rs:411-412`), issue card (457-459), Now-playing (866-867) all
  per-TP; Findings "⚡ Optimize all N exceeding toolpaths"→`OpenOptimizeProject`
  (665-666).
- Refute note: each per-TP button is adjacent to a card/strip naming its toolpath
  ("TP {n}" 381, boundary name 842), and only one is visible per context; project
  button is lexically distinct. "Can't predict target" weak. Stays low.
- User impact: Multiple identically-labelled "Optimize" buttons fire
  different-scoped actions.

---

## Optimizer / sim-recommendation — 6 findings (1 high, 3 med, 2 low)

### OPT-001 — S7 — HIGH — headline optimized cycle time understated, savings inflated
- Surfaces: optimize-project
- Evidence: `optimize_project.rs:192-220` `compute_optimized_cycle()` returns 0.0
  for `NoSafeImprovement` and `Skipped` rows; this total feeds `draw_header`
  (162-186) which renders bold "Optimized" cycle time and a green "(-X, -Y%)" badge.
- Refute note (strengthened): the in-code justification ("no direct access to
  baseline cycle for refused rows") is false for NoSafeImprovement —
  `tool_load/optimize/outcome.rs:77,82-83` documents that NoSafeImprovement carries
  "baseline at index 0 plus every candidate attempted", and the rollup already reads
  `outcome.candidates` (315). So `candidates.first().cycle_time_s` IS the baseline
  and is dropped to 0.0 anyway. Only Skipped (outcome.rs:172) genuinely lacks a
  baseline. NoSafeImprovement is common → understatement/inflation per refused row.
- User impact: Whenever any toolpath is refused/skipped the headline Optimized time
  is understated and savings % inflated; operator trusts a wrong project-wide claim.

### OPT-004 — S3 — MED — blank-header col-1 conflates 3 states; verdict col mixes glyph+text
- Surfaces: optimize-project
- Evidence: `optimize_project.rs:131-136` leaves col-1 header blank; col-1 then
  renders an interactive checkbox for Ranked-with-safe (282), a disabled checkbox
  for Ranked-without-safe (281-282), and empty `ui.label("")` for
  Skipped/NoSafeImprovement/TradeOff/MarginalSafe (309, 334, 348, 369). Verdict
  column mixes ✓/⚠ glyphs (386-396) with free-text reasons/badges (321-326,
  337-341, 356-360).
- User impact: Selection and verdict columns conflate selectable/locked/N-A rows
  with no header or consistent affordance.

### OPT-002 — S6 — MED — TradeOff/MarginalSafe rows have no way to open the per-op modal
- Surfaces: optimize-project
- Evidence: `optimize_project.rs:344-382` renders TradeOff/MarginalSafe rows with
  no checkbox and code comments saying "open the modal to apply", but the rollup
  emits no `OpenOptimizeModal` anywhere (only Close/Cancel/Apply/ToggleRow buttons).
- Refute note (softened high→med): (1) the "open the modal" phrasing is in code
  comments (207-209, 347, 363-367), not user-visible text — the surface gives no
  guidance at all. (2) The per-op modal IS reachable for any toolpath via
  Now-playing (`sim_diagnostics.rs:865-868`, gated only on `current_boundary()`) by
  scrubbing playback into its move range. Core gap (no per-row affordance + a real
  close-and-scrub detour) stands.
- User impact: Rows that need per-op review give no way to open the modal; the user
  must close the rollup and hunt for that TP's inline Optimize button.

### OPT-003 — S7 — MED — optimizer can run off a stale baseline with no cue
- Surfaces: optimize-project, optimize-modal, sim-diagnostics-optimize-entry
- Evidence: Optimize runs off a `baseline_trace` snapshot taken at open
  (`controller/events/mod.rs:858-889` clones `cut_trace`; 420-435 for the modal).
  Menu gate checks only `state.simulation.has_results()` (`menu_bar.rs:155`); the
  handler only checks the trace exists; neither checks staleness. The stale signal
  exists (`sim_diagnostics.rs:819` `sim.is_stale`) but no optimizer surface checks
  or surfaces it.
- User impact: The optimizer can be launched against a stale simulation and present
  candidate cycle times/verdicts as authoritative with no staleness cue.

### OPT-005 — S5 — LOW — recommendations are prose, not actionable affordances
- Surfaces: optimize-modal
- Evidence: `optimize_modal.rs:570-585` `draw_suggestions` renders
  `OperatorSuggestion` items as a "Try this" bullet list of imperative sentences
  ("Cap feed at ~2961 mm/min and re-optimize.", `format_suggestion` 590-618) with a
  comment "each suggestion is a one-liner with no button (operator must manually
  act; we never auto-apply a heuristic)".
- User impact: Actionable recommendations (cap/raise a specific axis to a specific
  value, then re-optimize) are delivered as prose to re-enter by hand, despite the
  modal already owning the re-optimize action.

### OPT-006 — S6 — LOW — project optimizer entry obscure; greyed menu item has no rationale
- Surfaces: sim-diagnostics-optimize-entry
- Evidence: Only always-available entry is `menu_bar.rs:156-162` (Toolpath ▸
  "Optimize project…"); the in-context entry (`sim_diagnostics.rs:658-668`) is gated
  behind `if bad > 0` so it vanishes when nothing exceeds. Menu button uses
  `add_enabled(optimize_enabled, ...)` with no `on_disabled_hover_text`.
- User impact: When nothing is over-budget the project optimizer is reachable only
  via a non-obvious menu item, and when greyed (no sim) the user gets no explanation.

---

## Simulation timeline internals & transport — 10 findings (0 high after softening, 6 med, 4 low)

Root cause: independently-authored painted strips, each with its own X-axis
meaning, its own playhead, and its own click contract. File: `sim_timeline.rs`.

### TIM-001 — S3 — MED — three stacked strips, divergent X-axes, three playheads
- Surfaces: sim-boundary-timeline, sim-span-ribbon, sim-semantic-band
- Evidence: boundary timeline X = `move/total_moves`, project-global
  (`sim_timeline.rs:953-1075`); span ribbon X =
  `(boundary.start_move+local)/total_moves`, project-global (1207-1399); semantic
  band X = `move_start/local_total`, local-to-one-toolpath (1752-1805). All
  full-width, stacked, each with its own white playhead (1068-1074, 1390-1398,
  1806-1811).
- Refute note (softened high→med): boundary timeline and span ribbon BOTH use
  global X (ribbon converts at 1208-1209), so those two playheads DO line up — the
  mismatch is only the semantic band, which is gated behind `sim.debug.enabled`
  (1129). In default state only the two aligned strips render.
- User impact: Three look-alike stacked bars with three playheads whose X-axes mean
  different things; cross-strip position reads are silently wrong (debug mode).

### TIM-002 — S3 — MED — identical strips, three unrelated click side-effects
- Surfaces: sim-boundary-timeline, sim-span-ribbon, sim-semantic-band
- Evidence: boundary click = seek + snap to safety marker + force Safety tab
  (`sim_timeline.rs:1097-1119`); span ribbon click = set chip-row scope (toggle) +
  scrub to span start (1412-1422); semantic band click = pin semantic item + route
  to DebugTrace/CutQuality tab + jump (1814-1910).
- Refute note (softened high→med): semantic band is debug-gated (1129). In default
  UI only two adjacent strips with different click side-effects (boundary
  seek/safety-tab vs span-ribbon scope-toggle+scrub). Still a real unpredictability
  defect, two-way not three-way.
- User impact: Identical-looking adjacent bars respond to the same gesture with
  different side-effects.

### TIM-003 — S6 — MED — signal spine vanishes with no placeholder when no trace
- Surfaces: sim-signal-spine
- Evidence: `draw_signal_spine` returns silently when no `cut_trace`
  (`sim_timeline.rs:212-217`); the only message ("No cutting samples captured.",
  270-274) appears solely when groups exist but are empty. The populating toggle
  ("Capture cutting metrics") lives in a different panel (`sim_op_list.rs:45-51`).
- User impact: When metrics weren't captured the signal graphs just don't exist,
  with no on-surface hint that a left-panel toggle + re-run brings them back.

### TIM-004 — S6 — MED — scrub/seek/gate-snap hidden behind a single hover tooltip
- Surfaces: sim-signal-spine
- Evidence: Each track is click/drag-to-seek + click-to-snap-to-gate-trip
  (`sim_timeline.rs:767-802`); the only disclosure is `on_hover_text` on the whole
  plot ("Hover to read all five tracks… Click or drag to scrub. Scroll to zoom.",
  804-806). No cursor change, no scrub handle, no per-track affordance.
- User impact: Real scrub/seek/gate-snap interactions are hidden behind a hover
  tooltip; users won't know the graphs are interactive.

### TIM-005 — S5 — MED — verdict-HUD pill tooltip tells you to go click a different widget
- Surfaces: sim-verdict-hud
- Evidence: Both the exceeds pill (`sim_timeline.rs:133`) and collisions pill (150)
  carry "Click the red lines on the boundary timeline below to navigate." The red
  lines are in `draw_boundary_timeline` (1006-1030, 1484-1525), a separate widget;
  the pills are plain `ui.label` with no `Sense::click` (`info_pill` 193-196).
- User impact: A count pill explains in words that you must go click something else
  — text doing an affordance's job, on the wrong widget.

### TIM-006 — S1 — MED — op row packs run/visibility/jump/(debug)tree into one card
- Surfaces: sim-op-list
- Evidence: Each row packs a sim-inclusion checkbox that re-runs the sim
  (`sim_op_list.rs:265-274, 384-412`), viewport visibility eye/cut/rapid/isolate
  (306-318), whole-card click = jump playback to TP start (368-377), and a nested
  expandable structural/semantic span tree (320-360). The card-jump Id (371)
  overlaps inner click targets.
- Refute note (softened high→med/low): the span tree (320-360) is gated behind
  `sim.debug.enabled` (320), so default rows pack THREE concerns not four. The "Id
  collides" claim is overstated — egui resolves child clicks first and the discrete
  Id (371) is over `inner.response.rect` with a comment (363-367) noting inner
  widgets consume clicks first. Real issue is a non-obvious overlapping click zone,
  not a functional collision. Multi-purpose-row concern stands.
- User impact: One row mixes "what runs", "what's visible in 3D", "where playback
  jumps", and a span tree, with an overlapping empty-space jump zone.

### TIM-007 — S3 — LOW — one expander silently switches between two different trees
- Surfaces: sim-op-list
- Evidence: `OutlineKind::StructuralSpans` renders SpanKind rows;
  `OutlineKind::SemanticFallback` renders semantic-trace rows
  (`sim_op_list.rs:421-466, 326-358`); selection is automatic via
  `outline_kind_for_toolpath` (446-466) based on span validity.
- Refute note (softened med→low): the toggle button LABEL differs by kind ("Show
  spans"/"Hide spans" vs "Show semantics"/"Hide semantics", 422-434) and hover text
  differs (436-443, fallback says "Spans were invalidated; expand the legacy
  semantic trace fallback"). So the two modes ARE signposted; whole outline is
  debug-gated (320). Disclosed and debug-only.
- User impact: The same expander shows two different data models depending on
  internal validity state.

### TIM-008 — S2 — MED — five co-equal signal tracks, no metric-resolved summary tier
- Surfaces: sim-signal-spine
- Evidence: Five tracks render as five equal-weight 90px stacked plots in a scroll
  area (`sim_timeline.rs:361-405`, 429-453; height 90 at 578) with no
  "which-track-is-in-trouble" indicator above the stack — gate-trip dots are
  per-track inside each plot (745-760).
- Refute note (softened high→med): a problem-summary tier DOES exist above the
  spine — `draw_verdict_hud` (83-166) paints at-a-glance pills and the boundary
  timeline paints project-wide red markers. What's genuinely missing is a
  WHICH-OF-THE-FIVE-METRICS indicator. "No summary-first layer" too strong.
- User impact: The spine is a flat stack of five co-equal graphs with no
  metric-resolved summary; the user must scroll and scan all five.

### TIM-009 — S7 — LOW — bottom-panel spine/strips never reflect staleness
- Surfaces: sim-staleness-card, sim-signal-spine
- Evidence: The staleness card fires on any edit-counter bump
  (`sim_op_list.rs:176-203`) but the signal spine/timeline keep rendering the old
  trace with no stale styling (`draw_signal_spine` reads `cut_trace`
  unconditionally, `sim_timeline.rs:212-217`; tracks 577-803 carry no staleness
  affordance). `is_stale` is consulted in `sim_op_list.rs:176` and
  `sim_diagnostics.rs:819` but NOT in `sim_timeline.rs`.
- User impact: When results go stale only the left/right panels say so; the metric
  graphs keep showing outdated data styled as fresh.

### TIM-010 — S4 — LOW — gate-trip click never focuses its hotspot (dead drill path)
- Surfaces: sim-signal-spine
- Evidence: `clicked_hotspot` is set from gate-trip markers whose synthetic index
  is `10000+i` (`sim_timeline.rs:309-313`), but the click handler resolves it via
  `trace.hotspots.get(hotspot_index)` (459-465). `hotspots` is initialized empty
  (296) and only gate-trip markers (index ≥10000) are pushed, so
  `trace.hotspots.get(10000+)` always returns None; `focused_hotspot` is never set —
  only `SimJumpToMove` (466) fires.
- User impact: Clicking a red gate-trip dot jumps playback but never focuses the
  hotspot it represents; the drill-into half of that action is dead.

---

## Shell, navigation chrome, ribbon & menus — 7 findings (1 high, 5 med, 1 low)

### SHE-001 — S4 — HIGH — entire project-tree navigation panel is dead code
- Surfaces: project-tree
- Evidence: `project_tree.rs:12` defines `pub fn draw` (the entire 446-line panel).
  `rg project_tree:: crates/ --type rust` returns only `ui/mod.rs:10: pub mod
  project_tree;` — zero call sites repo-wide. `app.rs` left panels call
  `setup_panel::draw` (188) and `toolpath_panel::draw` (232); project_tree never
  drawn. Compiles only because `pub`, so dead-code lint doesn't fire.
- User impact: The entire project-tree navigation panel ships unreachable, and
  every "duplicates the tree" note elsewhere compares against a panel users never
  see.

### SHE-002 — S3 — MED — status bar and workspace bar print different collision tallies
- Surfaces: status-bar, workspace-bar
- Evidence: status bar shows `"{} collisions"` (`status_bar.rs:83`) where
  `collision_count = controller.collision_positions().len()` (`app.rs:203`),
  populated ONLY by the holder-clearance check (`controller/events/compute.rs:633`).
  Workspace bar computes `holder_collision_count + rapid_collisions.len()` for both
  its Simulation badge (`workspace_bar.rs:140`) and Setup readiness badge (170). Both
  bars are always visible (workspace bar `app.rs:463-472`; status bar `app.rs:206,250`).
- User impact: Two always-visible surfaces print "collisions" with two different
  tallies (status bar omits rapid), so the same project can read "0 collisions" at
  the bottom and "2!" on the Simulation tab.

### SHE-003 — S3 — MED — severity-ladder inversion hides collisions behind "stale"
- Surfaces: workspace-bar
- Evidence: `simulation_badge` returns " stale" and exits before checking collisions
  (`workspace_bar.rs:136-138` precede the collision check at 140-143), while
  `readiness_badge` checks collisions FIRST (175 before the stale branch at 179).
- User impact: For a stale-and-colliding project the Setup tab shows red "N
  collision(s)" but the visually-identical Simulation tab shows only yellow "stale"
  — the more severe warning is hidden on one of two adjacent identical badges.

### SHE-004 — S1 — MED — toolpath header is five concern groups; diagnostics buried last
- Surfaces: toolpath-properties-header, diagnostics-ribbon
- Evidence: `draw_toolpath_panel` (`properties/mod.rs:2501-2744`) renders, in one
  always-on pre-tab block: rename (2507-2510), tool combo (2513-2527), input-model
  combo (2530-2544), STEP face-selection picker (2562-2592), remaining-stock
  checkbox (2595-2613), Generate+status+move-count (2619-2654), validation errors
  (2655-2663), then the full three-tier diagnostics ribbon (2707-2743). Comment
  labels it "Shared header (always visible above tabs)" (2504).
- User impact: The header is secretly five panels (identity, geometry IO, stock
  linking, generate, diagnostics) stacked with no grouping; the richest signal — the
  tiered diagnostics ribbon — sits buried under unrelated IO fields.

### SHE-007 — S1 — MED — Setup rail stacks five unrelated concerns, nav competes with KPIs
- Surfaces: setup-list-panel
- Evidence: `setup_panel::draw` stacks a Stock summary+edit card
  (`setup_panel.rs:22-42`), a project ops/tools/time summary card
  (`draw_project_summary` 341-419, called 47), a red project-findings diagnostics
  card (`draw_project_diagnostics_card` 272-339, called 53), the setup cards list
  (58-61), and a collapsed Models list (75-103).
- Refute note (softened high→med): Stock/setup/Models are legitimately
  setup-context; the genuinely out-of-place items are only the project KPI summary
  and project diagnostics card (2 of 5). Models list is default-collapsed (75-77);
  the diagnostics card self-hides when no findings (comment 49-53). Overload milder
  than "five panels sharing one wall".
- User impact: The Setup navigation rail is really five panels sharing one wall with
  no summary-to-detail hierarchy, so picking a setup competes with readouts that
  belong elsewhere.

### SHE-006 — S6 — MED — Enable/Duplicate reachable only via right-click context menu
- Surfaces: toolpath-card-context-menu
- Evidence: Per-card Enable/Disable, Duplicate, Move Up/Down exist only in the
  right-click context menu (`toolpath_panel.rs:434-486`: Enable/Disable 463-467,
  Duplicate 468-471, Move Up 473, Move Down 477); inline rows expose only Sim (373),
  Generate (383), and the eye/C/R/isolate strip (424-431).
- Refute note (softened high→med): a drag-to-reorder GRIP exists inline
  (`grip_resp` 301-305), so reorder DOES have a visible inline affordance — only
  Move Up/Down menu items are redundant. Enabled state is reflected inline via
  dim/`TEXT_FAINT` name coloring (346-351). Real gap is narrower: Enable + Duplicate
  hidden.
- User impact: Enable/disable and duplicate — core queue-management actions — are
  discoverable only by right-clicking, with no visible inline cue.

### SHE-005 — S5 — LOW — doubled face-selection prose
- Surfaces: toolpath-properties-header
- Evidence: When zero faces are selected the panel shows italic "Click faces in
  viewport to select" (`properties/mod.rs:2582-2585`) AND immediately below a smaller
  "Tip: click faces in the 3D view while this toolpath is selected" (2587-2591). The
  Tip line is NOT inside the else branch — it renders even when faces ARE selected.
- User impact: Two stacked explanatory sentences do the job a single
  affordance/placeholder should, adding noise to an already overloaded header.

---

## Cross-area themes

- **Staleness/freshness (S7) is systemically unwired** beyond the two panels that
  bother to check it: INS-005 (card path), OPT-003 (optimizer baseline), TIM-009
  (bottom panel). The `is_stale` signal exists; three surfaces that present concrete
  numbers never consult it.
- **Dead/unreachable code (S4)** recurs: SHE-001/TOO-001 (project_tree.rs) and
  TIM-010 (gate-trip drill). Dead `pub` modules dodge the dead-code lint.
- **Prose where an affordance belongs (S5)** recurs: TOO-006, OPT-005, TIM-005,
  SHE-005 — and the "open the modal" guidance in OPT-002 isn't even user-visible.
- **No summary-first tier (S2)** in the two densest panels: INS-001 (Inspector) and
  TIM-008 (signal spine); SHE-004/SHE-007 are the S1 cohesion analogue (richest
  signal buried beneath IO/KPI fields).
