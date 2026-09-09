# R03 — Choosing tools and authoring the intended cut

Date: 2026-09-09. Reviewer: Claude (Fable 5.1, Claude Code session `rs-cam-b9`)
on its OWN GUI instance. **Expert walkthrough, MCP-assisted. Not a novice test.**
No HUMAN or DESKTOP evidence exists for this package. Menu discovery, the Add
menu, viewport face picking, drag, typing and toasts were NOT clicked.
No application source, tests, `.mcp.json`, agent config, user projects or
tool libraries were changed. No builds, cargo tests, commits or process kills.

## 1. Scope, build, fixtures, limits

- Instance: `target/release/rs_cam_gui --mcp` pid 3967327, a child of this
  Claude Code process (`.mcp.json` stdio launch). Binary SHA-256
  `9f1cb522…1311`, the same file W00 recorded. `project_summary.build` =
  `8a4df241-dirty`, 2026-09-09T08:55:51+12:00. Checkout HEAD ef91cb03 with
  other developers' uncommitted edits (`dressup.rs`, `execute.rs`, `app/mcp.rs`,
  `multitool_planner.rs`, …). CODE citations are working-tree lines; the binary
  predates some of them. The other review session's instance was not running
  and its terrain job was never loaded here.
- Window 1400×900 logical, never resized. Simulation, where run, 0.5 mm cell
  with metrics capture switched on by the MCP call.
- Seeds and hashes: see [trace.md](trace.md). Four spines were authored:
  F1 pocket+profile (SVG), F3 rough+scallop (0.2-scale terrain STL), F2
  v-carve+trace+drill (star SVG), F4 pocket on STEP without a face pick. Saved
  editable states are in `scratch/` and named REVIEW ONLY. One scratch NC export
  exists, named NOT FOR MACHINING, never sent anywhere.
- Evidence labels: MCP-VIEW (full-window PNG after `set_ui_view`), MCP-STATE
  (tool replies), CODE (working tree). Nothing here is HUMAN or DESKTOP.
- Two task cards are only partly covered: the **GUI Add menu's first-tool
  default** (CODE only, needs a click), and **face picking on STEP** (no MCP
  route; NOT TESTED). Project Curve (mixed curves on a mesh) was not
  exercised live; its availability rule is CODE only.

## 2. Five-line summary

- All four spines were authored, generated and saved through MCP; every
  inspector carries a purpose line, Tool/Input selectors and sensible Suggest
  defaults, and the static validator disables Generate with a readable reason.
- The largest obstruction is **the wrong-tool experience**: the GUI Add path
  binds the FIRST project tool (CODE), refuses ball-required ops with a toast
  when that tool is flat, and handles V-bit-required ops the opposite way.
- The largest confidence risk is a **generated-path defect the UI cannot see**:
  pocket ramp legs leave the pocket and cut the surrounding stock (NC + sim
  confirmed), while the simulation says OK / no collisions.
- Two silent defaults would surprise a CAM-literate user: a Drill op with no
  circles drills the outline's centroid, and a Profile defaults to a full-depth
  through cut with zero tabs; depth deeper than the board is accepted silently.
- The Feeds card labels recommendation values ("Commanded advance/tooth",
  "DOC") as if they were the stored op values, and the per-field ⚡ pill beside
  Depth/Pass writes that uncapped value (R04 owns both fixes).
- Source verification added two S1 states the screen never shows: a stale
  result on a 3D op keeps its green OK, and the "Inherit from stock" boundary
  checkbox is a dead control.

## 3. Task outcome ledger

| Card | Outcome | Active / wait | Wrong turns | Notes |
|---|---|---|---|---|
| 1 First 2D cut (pocket + outline) | Completed via MCP | ~35 min incl. captures; generation < 5 s | 0 | "Keep the part held" was NOT satisfiable from defaults: tabs 0, depth = stock |
| 2 First 3D cut (rough + detail) | Completed via MCP | generation ~10 s | 0 | Reach map shown before generation (69.8 % unreachable at Ø3 ball) |
| 3 Tool order / ball-required | Refusal reproduced (MCP); first-tool default CODE only | — | — | GUI toast wording/visibility NOT TESTED |
| 4 Detail input: v-carve, trace, drill, STEP face | Completed except face pick | v-carve 214 k moves in < 60 s | 1 (end mill on v-carve, deliberate) | Drill drilled one unrequested hole |
| 5 Control and recovery | Completed: geometry (depth, stepover), heights (read only), entry style (read only), validation block, restore | — | — | Undo NOT TESTED (no MCP route) |
| 6 Tool reuse | Library import + tool inspector captured; catalog not written (timestamps) | — | — | Apply/Revert/navigation-away CODE only |

## 4. Findings

```text
ID: UX-R03-001
Task / starting state: F1 pocket, defaults (Entry Style Ramp 3°, DPP 1.2), generated + simulated at 0.5 mm
Type: behaviour defect (generated path) — reported separately from UX per PROTOCOL §5
Impact: S0 misleading confidence: the stock outside the pocket is cut while every operator surface reads OK
Evidence: MCP-STATE + MCP-VIEW, confirmed. Known ledgered follow-on, previously unmeasured
Observed: pocket six-view shows entry legs outside the outline (evidence/10_pocket_6view.png). Sim
  checkpoint 0 (pocket only) shows trenches outside the pocket wall, the longest ≈ 11 mm past the wall
  at the lower-left (12_sim_after_pocket_6view.png). NC: leg (22.485,18.03,Z1) → (3.622,20.909,Z0)
  → back to (22.485,18.03,Z−1); the pocket wall is at X 15 (scratch/…f1_pocket_profile.nc lines 9–13).
  run_simulation: verdict OK, collision_count 0, header "No collisions, air cutting under threshold".
  planning/entry_moves_2026-09-03/FINDINGS.md:263 lists "2.5D ramp containment: a 19 mm leg can leave
  a small pocket and side-cut a wall" as a not-measured follow-on; CLAUDE.md says prism ops keep blind
  legs by design (that note is about Z, not XY containment).
Expected: a pocket never cuts outside its closed region; if it must, the evidence surface says so.
Consequence: on a part where the pocket is not cut out afterwards (the normal case), the part is
  damaged; here the trenches fall in the waste ring only by luck. The rapid checker cannot see fed cuts.
Scope: 2.5D pocket with ramp entry, small pockets relative to the ramp leg (leg ≈ DPP/tan 3° ≈ 19–23 mm
  here); reproducible on the F1 seed every time (725 moves, deterministic).
Known issue: entry_moves FINDINGS follow-on. Owner: engineering (generator); R06 for the evidence gap.
Proposal: clip ramp legs to the pocket region (fold the leg, or degrade to plunge/helix when it does not
  fit) and surface a "fed move outside the operation region" finding. Alternative: default Entry Style
  to helix or plunge for pockets whose inscribed width is below the ramp leg. Preserve the ramp option.
Acceptance: on this seed, sim checkpoint 0 shows no material removed outside the 70×50 outline, or the
  triage carries a safety row naming the move.
Effort: cross-layer (generator + diagnostics)
```

```text
ID: UX-R03-002
Task / starting state: F3 terrain with tools [Ø6 flat, Ø3 ball]; user wants a scallop finish
Type: interaction/IA + behaviour
Impact: S1 blocks a supported task on the GUI route until the user reorders tools or learns the rule
Evidence: CODE confirmed; MCP-STATE confirmed for the refusal text; GUI toast NOT TESTED (needs live)
Observed: controller/events/toolpath.rs:43 `tools().first()` — the Add menu always binds the FIRST
  project tool, not the selected or the only suitable one; :112 `models().first()` likewise. When
  Suggest refuses the tool×op pair the toolpath is NOT added and a Warning notification is pushed
  (:92-101). Live MCP text: "Cannot add toolpath: scallop requires curved tip (need ball|bull|
  tapered_ball; got Flat on Scallop)". In the ball-first seed the same click would succeed, and a
  3D Rough click would silently bind the Ø3 ball.
  The refusal is a Warning toast: bottom-right, max width 400 px, 6-second lifetime, no close
  button, no click action, no log (controller.rs:62-79, app.rs:892-933 — CODE, source track).
Expected: adding an op picks a compatible tool (or the selected one) and lets me change it in place.
Consequence: a project with the right tools still cannot add the op by the menu; the fix (reorder or
  delete/re-add tools) is not stated by the message, and the message is gone after 6 s. Reverse
  order silently roughs with the ball.
Scope: every op with a tool precondition (Scallop, and any Suggest refusal); any project with ≥ 2 tools.
Known issue: PLAN.md reconnaissance row 4 predicted this. Owner: R03.
Proposal: bind the first COMPATIBLE tool (fall back to first) and, when none is compatible, still add
  the op in the same blocked state V-carve uses (UX-R03-006) so the Tool dropdown is the fix.
Acceptance: flat-first seed, click Scallop Finish → op exists, tool = Ø3 ball or a blocked row with
  the Tool dropdown highlighted; no toast-only refusal.
Effort: interaction/wiring
```

```text
ID: UX-R03-003
Task / starting state: any MCP add_toolpath that is refused (F3, scallop with flat tool)
Type: behaviour defect (notification)
Impact: S0 misleading confidence for an agent-assisted operator: the GUI announces success on a failure
Evidence: MCP-VIEW + CODE, confirmed
Observed: evidence/19_f3_loaded.png toast "MCP: Added toolpath 'Detail finish (flat tool attempt)'"
  while the reply was ok:false and list_toolpaths shows no such op. app/mcp.rs:571-577 pushes the
  "Added" notification BEFORE calling mcp_add_toolpath.
Expected: the toast reports the outcome.
Consequence: a person watching the screen while an agent authors believes an op exists.
Scope: every refused MCP add (no tools, no geometry, tool precondition) — and the same pattern
  covers all 16 pre-announced MCP handlers (R08 source track, CODE): nine use the past tense before
  the handler runs and never correct on failure ("Loaded", "Saved", "Set … on", "Added toolpath",
  "Removed toolpath", "Added tool", "Imported tool from library"; app/mcp.rs:368-609).
Owner: R03 (MCP surface), R08 notifications.
Proposal: push the notification after the result, with the refusal text on failure.
Acceptance: repeat step 23 of trace.md; the toast reads "Cannot add toolpath: …".
Effort: copy/local
```

```text
ID: UX-R03-004
Task / starting state: F2 star (one closed polygon, no circles/points); add Drill with Ø3 end mill
Type: behaviour defect + guidance
Impact: S1 (an unrequested hole is generated and exports as Within) — safety-adjacent, not a collision
Evidence: MCP-STATE + MCP-VIEW + CODE, confirmed
Observed: inspector purpose "Drill holes from SVG circle positions"; no target list, no warning,
  Generate enabled (23_drill_no_targets_inspector.png). Generate → 10 moves, one hole at (50, 51.1)
  = the star polygon's vertex centroid (25_drill_6view.png). execute.rs:914-950: selected_holes None →
  "fall back to the centroid of every closed polygon"; drill.rs:20-31 hides the target selector when
  the model has no DrillTarget. All three drill gates read Within.
Expected: with no circles or points, the op says "no drill targets" and does not generate.
Consequence: a hole through the middle of the part, invisible at authoring time, green at export.
Scope: any SVG/DXF whose closed shapes are not holes; Drill and (CODE, untested) Pin Drill differ.
Known issue: none found. Owner: R03; R06 for the gate population note.
Proposal: treat "no picked targets" as an error (the code already does this for an EMPTY pick), show
  the target count (0) in the inspector, and make the purpose line match the behaviour.
Acceptance: on the star seed, Generate is disabled with "No drill targets — pick points/circles".
Effort: interaction/wiring
```

```text
ID: UX-R03-005
Task / starting state: F1 pocket, Feeds & Speeds tab, fresh op (feed 750, RPM 15000, 2 flutes, DPP 1.2)
Type: guidance (provenance)
Impact: S2 uncertainty about what is stored vs recommended
Evidence: MCP-VIEW + MCP-STATE + CODE, confirmed
Observed: 05_pocket_feeds_tab.png "Commanded advance/tooth: 0.0435 mm/tooth" and "DOC: 4.20 mm";
  stored op = 750/(15000×2) = 0.025 mm/tooth and DPP 1.2. properties/mod.rs:2047 prints
  `result.chip_load_mm`, :2105 `result.axial_depth_mm` — the live recommendation, hover text says
  "at the recommended feed". get_suggest_rationale: DPP capped 4.2 → 1.2 by the rigidity factor.
  The same panel prints the stored-value warning "Commanded advance/tooth below rubbing floor:
  0.024 → 0.025", so two different numbers carry one label. Source track (W03/support/
  r04_source_track.md, CODE): 0.0435 is `FeedsResult::chip_load_mm`, the pre-derate TARGET
  chipload (feeds/mod.rs:1989), not feed ÷ (rpm × flutes); the rigidity cap (0.20 × D = 1.2 on the
  Generic Wood Router) runs only inside the apply funnel; "DOC" is the Pocket default 0.70 × D.
  The Feeds modal's DOC row shows the same 4.2.
Expected: "Commanded" and "DOC" describe the op as configured; recommendations are labelled as such.
Consequence: an operator reading 4.20 mm believes the op will cut 4.2 mm per pass; it cuts 1.2.
Scope: every op with a feeds card. Owner: R04 (primary), R03 discovered.
Proposal: label the rows "Recommended advance/tooth" / "Recommended DOC" or show stored → recommended
  pairs; the r04-source track was asked to check whether Apply cut geometry writes 4.2 or 1.2.
Acceptance: the card shows 0.025 / 1.2 as the op's values and 0.0435 / 4.2 as recommendations.
Effort: copy/local
```

```text
ID: UX-R03-006
Task / starting state: F1, add Profile to "cut the outline while keeping the part held"
Type: guidance + capability default
Impact: S2 (substantial detour; the default releases the part)
Evidence: MCP-STATE + MCP-VIEW + CODE, confirmed
Observed: Suggest wrote depth 12.0 (= stock 12), tab_count 0; inspector shows Depth 12.0 mm and a
  collapsed "Tabs" section (07_profile_geometry_tab.png; boundary_2d.rs:178). No caution mentions a
  through cut or workholding. The validator (properties/operations/mod.rs:1726-1838) has no depth-vs-
  stock and no through-cut rule. Side offers Outside/Inside only; the schema also has `on`.
Expected: a full-depth outside profile prompts for tabs or at least says "cuts through the board".
Consequence: the part is freed on the last pass with zero tabs unless the user opens Tabs unprompted.
Scope: Profile on 2D input; every project where depth ≥ stock thickness.
Owner: R03; R02 for workholding intent.
Proposal: when depth ≥ stock thickness, show a one-line "Through cut — no tabs" hint beside Depth and
  expand Tabs by default; keep zero tabs available for vacuum/double-sided-tape users.
Acceptance: on this seed a user can state before Generate that the part will be released.
Effort: copy/local
```

```text
ID: UX-R03-007
Task / starting state: F1 pocket, stock 12 mm; set Depth 15 mm
Type: guidance (missing guard)
Impact: S2 uncertainty; physically it cuts 3 mm into the bed
Evidence: MCP-STATE + MCP-VIEW + CODE, confirmed
Observed: Generate → Done, 1885 moves, row OK, no warning (14_pocket_depth15_over_stock.png).
  Heights tab would read Bottom −15.0 "below Stock Top". dv() range for Depth is 0.1..=100 with no
  stock bound (boundary_2d.rs). Validator has no rule (see 006).
Expected: a depth below the stock bottom is at least flagged.
Consequence: spoilboard/bed cut with a green row and a Within load verdict.
Scope: all 2.5D depth fields. Owner: R03; R06 for the sim (the dexel cannot see below the stock).
Proposal: a static "Depth exceeds stock thickness by X mm" caution on the row and header; do not
  clamp silently.
Acceptance: Depth 15 on this seed shows the caution before Generate.
Effort: copy/local
```

```text
ID: UX-R03-008
Task / starting state: F2 star, add VCarve with an end mill (tool 0)
Type: interaction/IA (inconsistent refusal designs)
Impact: S2
Evidence: MCP-STATE + MCP-VIEW confirmed for both arms; GUI toast arm NOT TESTED
Observed: VCarve with an end mill is ADDED, then Generate is disabled with red "Error: VCarve requires
  a V-Bit tool" and the Tool dropdown is right there (22_vcarve_endmill_validation_error.png). Scallop
  with a flat tool is REFUSED at add time and never appears (UX-R03-002). Same class of mistake, two
  behaviours, two vocabularies ("requires a V-Bit tool" vs "requires curved tip (need ball|bull|
  tapered_ball…)").
Expected: one model: the op appears, the row says why it cannot generate, the fix is in place.
Consequence: users learn the wrong lesson from whichever case they meet first.
Scope: VCarve/Inlay/Chamfer (validator) vs Scallop/UnifiedFinish (Suggest refusal); Rest has a
  third message. Three membership defects ride on the second design (source track, verified by
  the orchestrator against feeds/mod.rs:824-841 and diagnostics/adapters/from_static_checks.rs:
  120-144): (i) a BULL nose passes the add gate but the registry excludes it, so it refuses only
  at Generate; (ii) the Critical ribbon row fires only for ToolType::EndMill, so a V-bit or bull
  on Scallop shows a clean ribbon; (iii) that row also names Pencil, which has no tool constraint
  at all, so a flat end mill on Pencil generates normally under a red Critical row. The static
  validator has no ball-tip arm, so Generate is NOT disabled for a Scallop that reached the queue
  with a flat tool (load, duplicate, MCP, tool swap) — it fails in the engine instead.
Owner: R03.
Proposal: adopt the validator model for all preconditions (add, then block with the fix visible),
  driven by the one registry `ToolConstraints` table so the four texts collapse to one.
Acceptance: flat-first seed, Scallop appears as a blocked row naming the tool requirement; a
  bull-nose Scallop is blocked before Generate; Pencil shows no false Critical row.
Effort: interaction/wiring
```

```text
ID: UX-R03-014
Task / starting state: F1 pocket, Geometry tab, Depth/Pass 1.2 with a ⚡ pill beside it
Type: behaviour defect (recommendation route bypasses the engine's guarantees)
Impact: S0 misleading confidence: the pill offers a value the engine itself capped for rigidity
Evidence: CODE confirmed (source track lead, verified); live click NOT TESTED (no MCP route)
Observed: the per-field ⚡ pill (`dv_pill`, properties/mod.rs:4879-4900 → `ValueRow::suggest`,
  components/value_row.rs:112-135) writes `round_suggestion_value(recommended)` straight into the
  field. Its `recommended` is `feeds_result.axial_depth_mm` (operations/boundary_2d.rs:128), the
  calculator output BEFORE `enforce_invariants` — 4.2 mm here — while add-time Suggest and the
  "Apply cut geometry" button both pass through `apply_feeds_subset` (feeds/suggest.rs:856-932)
  and land on the rigidity-capped 1.2. The hover reads "Suggest Depth/Pass = 4.200 mm (source: …).
  Click to overwrite this field only." 24 such pills ship (Stepover, Depth/Pass, Max depth, Feed,
  Plunge across the op forms), per the R04 source track (W03/support).
Expected: every ⚡ offers the same value the engine would apply.
Consequence: one click sets a 3.5× deeper pass than the rigidity model allows, labelled as the
  recommendation.
Scope: all ops with a ⚡ pill. Owner: R04 (primary), R03 discovered.
Proposal: route the pill through the same funnel (`apply_feeds_subset` scoped to one field) or
  show the clamped value.
Acceptance: on this seed the Depth/Pass pill offers 1.2, not 4.2.
Effort: interaction/wiring
```

```text
ID: UX-R03-009
Task / starting state: F3 scallop selected, Geometry tab
Type: behaviour defect (dead control) + guidance
Impact: S1 — the visible boundary control describes a mechanism that does not exist
Evidence: MCP-VIEW + MCP-STATE + CODE, confirmed (source track lead X1, verified by grep)
Observed: "Enable boundary ✓ / Inherit from stock ✓" (20_scallop_geometry_tab.png) while
  get_toolpath_params says boundary.source = model_silhouette (controller/MCP add for 3D ops on a
  mesh, toolpath.rs:130-153 sets source AND boundary_inherit = true). properties/mod.rs:4252 hover:
  "Use the stock-level default boundary. Uncheck to configure a custom boundary". `boundary_inherit`
  has NO reader outside serde, the entry mirror and this checkbox (rg over crates/, excluding tests):
  generation clones `tc.boundary` unconditionally (session/compute.rs:1107). There is no stock-level
  default boundary. Unticking only reveals Source/Containment/Offset; it changes nothing generated.
Expected: the label names the boundary that will actually be used.
Consequence: the operator reads "inherited from stock"; the finish is clipped to the model
  silhouette. On a part smaller than its stock that is the difference between sweeping the board
  and sweeping the part.
Scope: every 3D op added on a mesh model; the planner sets the flag false deliberately.
Owner: R05 (boundary ownership), R03 discovered.
Proposal: remove the dead checkbox and print the resolved source ("Boundary: model silhouette
  (auto)") with the Source/Containment/Offset controls always visible.
Acceptance: the Geometry tab names the effective source and it matches get_toolpath_params.
Effort: copy/local
```

```text
ID: UX-R03-010
Task / starting state: F3 scallop and F2 v-carve inspectors
Type: guidance (diagram/schema mismatch)
Impact: S3
Evidence: MCP-VIEW + MCP-STATE, confirmed
Observed: Scallop shows the generic 2D "Contour" rings diagram (step 0.30); the end-mill VCarve showed
  a "Zigzag" raster diagram; Scallop Direction combo reads "Outside In" while get_operation_schema says
  `enum:x|y` with default "outside_in" (the default is not in its own enum).
Expected: the diagram illustrates the chosen strategy; the schema matches the UI.
Scope: 3D finish forms and the MCP schema. Owner: R03; MCP schema to R04/R09 shared.
Proposal: per-strategy diagram or none; fix the enum text.
Effort: copy/local
```

```text
ID: UX-R03-011
Task / starting state: F1 generated; stepover edited 2.1 → 6 → 2.1 without regenerating
Type: evidence presentation (missing state)
Impact: S1 for every 3D operation (S3 for auto-regenerating 2.5D ops, where the state lasts 500 ms)
Evidence: CODE confirmed by grep (source track lead, verified); the live capture is NOT clean
  evidence — see below
Observed: 18_pocket_stale_after_param_roundtrip.png — row "OK · 725 moves · 7:54 · 9.8 m", header
  "Done 725 moves", no row/header stale mark; only the workspace-bar chips "stale" / "sim stale"
  changed (those read the project-wide SimulationState::is_stale, a different quantity). CAVEAT:
  a Pocket auto-regenerates 500 ms after an edit (`process_auto_regen`, controller.rs:302-333),
  and the final value equalled the original, so by capture time the card was probably CURRENT
  again (R08 source track, hypothesis 1); the capture does not by itself show a stale row.
  The CODE case stands regardless: `RuntimeSnapshot` (toolpath_panel.rs:28-34) carries no
  `stale_since`, so the card cannot draw it.
  `ToolpathRuntime::stale_since` is set at ~20 sites and READ by exactly one GUI line,
  toolpath_panel.rs:746, inside the Rest dependency badge. No card chip, header or viewport
  treatment renders it. Every 3D family has default_auto_regen = false (catalog.rs:2142-2384),
  so for a 3D op the flag persists indefinitely while the card says OK / Done. The MCP wire does
  publish `stale` (app/mcp.rs:951, 1251): an agent can see it, the operator cannot.
Expected: the row whose result no longer matches its parameters says so.
Consequence: a scallop whose stepover was just changed shows OK beside the old result; the MAN chip
  says "needs manual generation", not "this result is out of date". The same gap lets a
  reorder/disable/tool edit leave a green card (see W03/support r05/r08 leads, R08 owner).
Scope: all operations; acute for 3D. Owner: R08 (freshness model), R03 discovered.
Proposal: render stale_since as a card chip ("STALE") and in the header ("Done · edited since"),
  and dim the drawn path.
Acceptance: edit a scallop stepover without regenerating; the row and header change visibly.
Effort: copy/local
```

```text
ID: UX-R03-012
Task / starting state: F1 pocket before Generate; predict what will be removed
Type: capability gap
Impact: S3 (needs a human to rate)
Evidence: MCP-VIEW, confirmed absence
Observed: nothing in the viewport distinguishes the pocket region from the island before Generate;
  the inspector diagram is generic. After Generate the paths show it, and the six-view is clear.
Proposal: hatch the target region (and islands) for the selected 2D op before generation.
Effort: interaction/wiring
```

```text
ID: UX-R03-013
Task / starting state: load a project through MCP after another project
Type: interaction/IA
Impact: S3
Evidence: MCP-VIEW, confirmed for the MCP load route only
Observed: the camera is not refitted on load (star partly off-screen, 22_…png); the empty inspector
  says "Select an item in the project tree" but no panel is called that (01_f1_loaded_toolpaths.png);
  "Triangles: 0" is the only size cue for a 2D model. Same class as UX-R01-002.
Owner: R01/R09.
```

## 5. Strengths and expert capabilities to preserve

- Purpose line under every tab strip ("Clear material inside a closed region",
  "Variable stepover for constant scallop height") and Tool / Input dropdowns
  in the header of every op; the wrong-tool fix is one dropdown away.
- Heights tab: five planes, each with a reference ("above Stock Top") and an
  "(auto)" annotation, plus a diagram. Clear and controllable.
- Static validator disables Generate and prints the reason inline; the MCP
  reply carries the same text.
- Reach map shown BEFORE generation on a finish op (69.8 % unreachable at Ø3).
- `add_tool` refuses a V-bit without an angle and explains why; `add_tool_from_
  library` imports a snapshot and did not touch `~/.config/rs_cam/tools/*`
  (timestamps unchanged).
- Suggest at add time gives stock-aware defaults (depth 5 on a 12 mm board for
  the pocket; DPP capped by rigidity with a rationale on request).
- MCP mutation replies carry `diagnostic_delta` and `gui_banners`, so an agent
  sees the same cautions the panel prints.
- Expert routes to keep: ⚡ per-field suggest pills; "Reset to recommended" on
  Dressup; Linking tab with entry style, links, feed optimization, rapid order
  and retract strategy in one place; "MAN" manual-generation badge with the
  "Press G" hint; the Tool Library modal's per-catalog search and Save-to-library.

## 6. Decision tree (what the user must choose)

1. **Input**: which model (Input dropdown) and, for STEP, which faces (viewport
   pick — NOT TESTED). 2D ops need polygons, 3D ops need a mesh, Project Curve
   needs both (menu greys the rest, `add_op_menu_item`).
2. **Goal → operation**: the menu is grouped "2.5D (from SVG)" / "3D (from
   STL)" by strategy name with a hover description; no goal-first grouping.
3. **Tool**: bound to the first project tool on add (CODE); change in the
   header. Preconditions: Scallop → ball/bull/tapered ball (refused at add);
   VCarve/Inlay/Chamfer → V-bit (blocked after add); Rest → previous tool
   larger and an earlier op with it.
4. **Cut geometry**: depth / depth-per-pass / stepover (⚡ suggest), pattern,
   side, tabs (collapsed), stock to leave, scallop height.
5. **Heights**: auto from stock; override per plane.
6. **Entry and linking**: ramp/plunge/helix, lead-in/out, links, feed
   optimization, rapid order, retract strategy.
7. **Boundary / remaining stock / rest analysis**: checkboxes on Geometry.
8. **Generate**, read the row (PEND / OK / ERR / MAN badges, moves · time ·
   distance), then Sim.

## 7. Operation census

See [census.md](census.md) (all 23 menu operations + Pin Drill = 24 variants;
labels and descriptions from `catalog.rs:1916-2404`; preconditions, forms and
menu availability cross-checked against the read-only source track, which
found no correction to the table and adds: the Add menu tests geometry over
ANY model while the validator tests THE assigned model, so an SVG-first
project offers every 3D op and then blocks it; no op form hides advanced
fields except Profile's collapsed Tabs — Adaptive3d's 26 and UnifiedFinish's
21 parameters render flat). Interaction patterns exercised
deeply: 2D closed-region clearing (Pocket), 2D contour with side/tabs (Profile),
3D rough with auto boundary (3D Rough), 3D ball finish with reach map
(Scallop), variable-depth engraving (VCarve), open-path following (Trace),
point targets (Drill), STEP face gate (Pocket on STEP). Not exercised: Face,
Adaptive, Rest, Inlay, Zigzag, Chamfer, 3D Finish, Waterline, Pencil, Unified,
Steep/Shallow, Ramp/Spiral/Radial/Horizontal Finish, Project Curve, Pin Drill.

## 8. Handoffs, known issues, historical claims

Track B source reports (read-only agents, saved verbatim) are in
`../W03/support/`: `r03_census_source_track_v1.md`, `r04_source_track_v1_partial.md`,
`r05_source_track_v1.md`, `r08_source_track_v1.md`, plus second-run files named
`*_source_track.md` / `r03_census_verification.md` when present. Each file's header
lists which claims the orchestrator spot-checked against source (✔). Everything
else in them is a lead for the owning package, not an accepted finding.

- UX-R03-001 → engineering (generator) + R06: the ledgered "2.5D ramp
  containment" follow-on is now measured on a checked-in seed.
- UX-R03-005 → R04 (owner). Verified by the source track: Apply cut geometry
  writes the capped 1.2 (the button is honest, the label is not); the per-field
  ⚡ pill is the route that writes 4.2 (UX-R03-014).
- UX-R03-014 → R04 (owner): 24 per-field ⚡ pills bypass `enforce_invariants`.
- UX-R03-009 → R05: `boundary_inherit` is a dead dial (verified). R05 leads to
  verify next: GUI runtime cache not invalidated on reorder/disable/move; Rest
  badge ignores order/enabled/model (verified); whole-project export omits an
  enabled op with no result; cross-setup drag appends; Generate All submits
  disabled ops when no rest chain exists.
- UX-R03-011 → R08: stale never rendered (verified). R08 leads to verify next:
  tool edit leaves an old-tool result exportable (fallback verified at
  io/export.rs:129-131); setup face/rotation invalidates nothing in the GUI;
  toggling enabled does not dirty (verified) or flag; dressup edits silent;
  stock drags push one undo entry per frame; no browse-for-path on a missing model;
  and (second run, needs live confirmation) merely inspecting a toolpath, post or
  machine panel then leaving pushes an undo entry and sets dirty with no equality
  check (properties/mod.rs:143-202), while a simulation that finishes after an edit
  stamps the edit counter at drain time and reads fresh (events/compute.rs:1176-1185).
- UX-R03-009 → R05: boundary inherit vs model-silhouette precedence.
- UX-R03-011 → R08 freshness model; UX-R03-003 → R08 notifications.
- UX-R03-013 → R01/R09.
- The W01 plunge-class critical finding reproduces here unchanged (70/70 and
  10/10 moves over the op plunge rate); nothing new added.
- Historical: PLAN.md's "first tool / first model" lead is confirmed in code
  and by the MCP refusal path; not yet by a click.

## 9. Untested questions

- Does the GUI Add menu toast for a refused Scallop read clearly, and how long
  does it stay? (needs DESKTOP)
- Face picking on STEP: reach, feedback, and whether a picked planar face makes
  Pocket generate. (no MCP route)
- Undo after a parameter edit and after remove_toolpath. (no MCP route)
- Tool Apply / Revert / navigation-away commit on the tool inspector: CODE says
  Apply/Revert appear only when modified and navigation auto-commits
  (`tool.rs:19-52`); the MCP edit committed directly ("✓ saved").
- Whether "Inherit from stock ✓" or the stored model-silhouette source wins at
  generation (UX-R03-009).
- Whether ramp legs also leave the region on Profile (the NC shows the same leg
  shape; not measured against the outline).
- Bull Nose on Scallop: does the user reach Generate before anything flags the
  tool (CODE says yes: add door accepts Bull, validator has no arm, engine
  refuses with an ERR chip).
- Task card 6 live run: set `RS_CAM_TOOL_DIR` to a scratch directory BEFORE
  launching the GUI. "Save to library", the modal's Save / Delete / move /
  Dedupe / Delete catalog all write the real catalog immediately, with no
  project undo, and "Save to library" writes the uncommitted draft and
  overwrites by geometry signature (tool.rs:83-101, tool_library.rs:181-202,
  359-370 — CODE, source track).
