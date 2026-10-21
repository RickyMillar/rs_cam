# Geometry selection: orchestration plan (2026-10-16)

Owner: the lead session (orchestrator). Agents implement; the lead decides,
reviews by path and commits.

Read first, in this folder:

- `GAP_ANALYSIS.md` — what other CAM tools do, our gaps, defects found.
- `REQUIREMENTS.md` — jobs, user stories S1–S27, requirements R1–R9, the
  per-op table of item kinds, packages G1–G7.

This file says who does what, in which order, and how each step is proved.
The phase names here (P0–P8) replace the package names G1–G7.

## Operator rulings so far (2026-10-16)

- Build generic geometry selection first. Auto detection and auto tool
  select come last, and only as filters that fill a selection the user
  reviews. Never "the widest tool whose centre fits".
- No wizard as the main surface. A thin "job type / template" flow comes
  later and writes ordinary ops with ordinary selections (P7).
- Tabs on straight edges with at least one tab per side, then hand-placed
  tabs (P5).
- Before the core work, a design phase (P0) fits the selection into the
  op-architecture code (shape runners, `Prepared`, territory, capabilities,
  W5 params) for a clean, extensible implementation.
- The Basis fork (`refs/remotes/basis/feature-web-server`) is a source of
  ideas, not code to merge. Its zigzag vertex fix landed as b145de0d.

## Rules for every agent

- Agents edit and test. Agents never commit, push, stash, reset or checkout.
  The lead reviews each change by path and commits with explicit pathspecs.
- Never commit `.mcp.json`, `planning/rivmap_block_job/testpiece.toml` or
  `.pi/` (the operator's own dirty edits).
- One cargo job at a time: `timeout <s> scripts/cargo_lane.sh <args>`.
- No large gates. Use named sentries, `--lib <filter>`, the folder sentries,
  `cargo test -p rs_cam_viz -q --lib`, `cargo test -p rs_cam_mcp -q`, then
  the full clippy line and `cargo fmt --all -- --check`. Ask the operator
  before any test run over 3 minutes.
- One editor agent in the checkout at a time. Read-only agents may run in
  parallel. No worktrees (disk at 89 %).
- Constants are derived from existing constants or a cited source.
- STE prose in comments, docs and reports. No legacy shims: delete old keys
  and state each break in the commit (a retired project key goes into
  `RETIRED_KEYS`, so a load refuses it with a message).
- Each fix gets a sentry. Prove each sentry red once with the defect put
  back, then restore.
- Compiler-enforced over sentry over doc: an exhaustive `match`, a private
  field or a type that cannot hold the wrong state beats a test.
- An agent that cannot finish an item cleanly stops and reports it open.
- Agent report: 500 words or fewer, files changed, gates run with results,
  open points.

## Per-item flow

1. Research agent (read-only), only when the item needs one. It can run in
   parallel with the current editor.
2. Editor agent: code, sentry, red proof, gates.
3. Lead: review the diff by path, run the item's sentry once, commit.
4. Lead: update the status table at the end of this file.
5. Next item.

A change to visible behaviour also needs a live GUI check by the operator
before the phase closes. Build the release GUI from HEAD first.

## Branch

- The lead merges `op-arch` into master locally (it holds b145de0d and
  the planning docs), then makes `geo-select` from master.
- Ask the operator before any push.

---

## P0 — Design: fit the selection into the op architecture

Goal: one design that the operator approves before any core code changes.
Output: `DESIGN.md` in this folder. Read-only agent(s); the lead writes or
edits the final file.

Read before designing: `crates/rs_cam_core/src/compute/CLAUDE.md`,
`compute/execute/prologue.rs`, `compute/execute/layered.rs`,
`compute/execute/territory.rs`, `compute/catalog.rs` and
`catalog/capabilities.rs`, `session/compute/generation.rs` (the face pick,
`face_top_z`), `geometry/enriched_mesh.rs`, `io/dxf_input.rs`,
`io/svg_input.rs`, `session/project_file.rs`, `compute/config.rs`
(`BoundaryConfig`), and `planning/op_architecture_2026-10-14/PLAN.md`.

The design must answer these questions, each with the file and type it
touches:

1. **Items.** One item type for every 2D source: closed loop (with holes
   from containment), open path, point, circle, face (loop + Z + normal).
   Each item carries its source model, its layer, and a stable match key
   for re-import. Where does it live (`geometry/`?), and what replaces
   `Polygon2`-only input?
2. **Sources.** How DXF, SVG (explicit scale), BREP faces, flat regions of
   an STL mesh and the stock produce items. One trait or one function per
   source; adding a source must not touch any op.
3. **Selection.** The stored object on `ToolpathConfig`: rules (layer,
   closed/open, circle diameter ± tolerance, face normal and Z, all) and
   explicit picks (matched by geometry). It replaces `face_selection` and
   Drill `selected_holes` / `selected_layers` (one model for both). Default
   rule = today's behaviour (all closed loops of the model).
4. **Resolution.** Where the selection resolves: a prologue step that fills
   `Prepared` with resolved items, so an adapter never reads the model
   directly. What does `GeometryRequirement` become? (Probably: the catalog
   row states the item kinds it accepts, as a capability; the prologue
   refuses an item kind the op does not accept.)
5. **Refusal.** A selection that resolves to nothing, a missing pick after
   re-import, or an open path given to a closed-only op refuses through
   `ctx.record` / the refusal path, with a message that names the item and
   the fix (R5). No log-only path remains.
6. **Selection and territory.** Selection = what to cut. Territory = where
   the tool may go. How they combine; how a 3D op uses a selection (feeds
   the boundary only); whether `BoundarySource::FaceSelection` and
   `ModelOutline` become selection rules.
7. **Depth references (R4).** How a top and a floor name their reference
   (stock top, model top, picked face, typed), in the setup frame. How a
   per-item floor reaches `run_layered` (it already prepares per region; can
   each region carry its own ladder?). How the height diagram and the GUI
   show the reference. Which ops honour a per-item floor.
8. **Params.** What stays in W5 `OpParams` (side, climb, tabs) and what moves
   to the selection. Whether per-item overrides (depth, side) belong to the
   selection or the op.
9. **Freshness.** A selection change stales only its op. A re-import
   resolves rules again; a lost explicit pick flags the op and keeps its
   tool, feeds and linking (S18).
10. **Wire and file.** Project file shape, MCP wire types (`rs_cam_mcp`),
    the `Command` set (set selection, add/remove pick, set rule), undo.
    Retired keys listed.
11. **Extension test.** Walk through three future changes against the
    design and show each touches one place: a new source (for example
    3MF), a new item kind (for example text), a new op that accepts loops.
12. **Migration.** Per op: today's input, the new input, expected golden
    change (none for the default rule, except where P1 refuses open paths).

Done when: the operator approves `DESIGN.md`. Record rulings in this file.

## P1 — Open-path honesty (safety; may run while P0 is read-only)

Goal: no op closes an open path with a chord without saying so.

1. Research: build a fixture with an open DXF path (a line, an arc, an open
   polyline) and run Trace, Profile, Pocket, Chamfer, VCarve, Zigzag,
   Adaptive 2D, Inlay through the session path. Record what each emits.
2. Editor: every op that is not built for open paths refuses an open input
   with a message ("open path on layer X: use Trace or Project Curve, or
   close it in the drawing"). Project Curve keeps its open-path support.
   Trace: decide (lead) whether it cuts open paths on the line now or
   refuses until P6.
3. Sentry: one test per op family; G-OPGOLDEN must not move (the fixtures
   hold closed loops only; confirm).

Done when: the sentry is green and was proved red.

## P2 — Selection model in core (after P0 approval)

Goal: the P0 design, in core, with no change of output for the default
rule. Split into items by the lead from `DESIGN.md`; expected order:

1. Item type and sources (DXF, SVG, BREP faces, stock). Layer on every loop.
2. Selection type, rules, explicit picks, resolver.
3. Prologue resolution into `Prepared`; every 2.5D adapter reads resolved
   items through one door. Drill moves onto the same selection.
4. Refusals (R5) replace the silent fallbacks in `generation.rs`
   (log-only pick, stock-rectangle fallback, largest-polygon-only union,
   one bad face drops all).
5. Project file, `RETIRED_KEYS`, `Command`s, MCP tools.

Proof: G-OPGOLDEN unchanged; W4 `every_op_conforms_w4` extended with a
selection check; new sentries for each rule and each refusal; re-import
sentry (rule survives, explicit pick flags).

## P3 — Depth references

Goal: R4. A face sets a floor; every depth names its reference.

1. Heights resolution reads references in the setup frame (fixes the
   model-frame `face_top_z`).
2. Per-item floor through `run_layered` for Pocket, Adaptive 2D, Zigzag,
   Profile; one op cuts several floors (S6).
3. Up faces and down faces separate.
4. GUI: each depth field and the height diagram show the reference (S5).

Proof: a multi-floor STEP fixture (golden case), a flipped-setup case, a
sentry that a face floor is in the setup frame.

## P4 — Selection UI and more sources

Goal: R6. The user sees and edits the selection.

1. Viewport highlight of the resolved items; counts ("14 loops, 3 islands,
   2 open"); a lost pick shown as such.
2. Pick loops, faces and holes in the viewport; filters next to the pick:
   layer, open/closed, circle diameter ± tolerance.
3. STL flat-region pick (idea: the fork's `face_region.rs`, rebuilt as a
   core source).
4. SVG scale shown and editable at import. The join tolerance (default
   0.01 mm) is settable per import; the import reports pieces joined and
   open shapes left.
4b. Open paths in their own colour, ends marked, a "select open" filter,
   a "show me" action on the `SelectionNote`; a join/close tool (by
   distance, close with a line, move ends together). See the GAP_ANALYSIS
   addendum 2026-10-19.
5. MCP parity for every UI action (GUI/MCP number parity rule).

Proof: viz controller tests per action, the MCP tests, a live look by the
operator.

## P5 — Tabs

Goal: S12, S13. Ideas from the fork's `tabs_on_straights` and `tab_points`.

1. Tabs on straight runs, clear of corners, at least one per side; a run
   may wrap past the loop start.
2. Hand-placed tabs stored in the op config (W5 `#[param]`), snapped to the
   nearest loop; GUI drag and delete.
3. Hook in `ProfilePass::emit_level` (per contour).

Proof: lift positions match `apply_tabs`; the one-per-side rule on a
D shape; G-OPGOLDEN re-blessed for Profile only, by name.

## P6 — Open paths and holes, done properly

1. Profile side On; Trace cuts open paths on the line (S9).
2. Open path "extend to stock" for Pocket (Fusion's tangent extension), if
   the operator wants it.
3. Holes larger than the drill are helix-milled from the same selection
   (S10); stated size = cut size (S11).

## P7 — Templates and job types (the thin guided flow)

1. A template = ops + selection rules + tool match by property (R8).
2. Missing layers named on apply (S21).
3. Job types (sign, sheet parts, inlay, flatten) propose ops and order for
   a beginner (S25). The flow uses the pin-wizard pattern: own draft, one
   apply event, ordinary ops after (R9).

## P8 — Recognition and tool advice (last)

1. Filters that fill a selection: corner radius, passage width, depth,
   hole diameter. The user reviews the result.
2. Tool advice: the largest tool that fits the tightest corner and passage,
   with flute reach; a rest tool for corners; refuse when no tool fits.
3. Ideas from the fork's `autoplan.rs` (flat levels, the island-or-opening
   rule, the edge overrun), rebuilt in core on the selection model.

---

## Rulings (operator, 2026-10-19: "Do the most complete implementation or best architecture for each. I give you autonomy here.")

- P0: the lead approves `DESIGN.md` (autonomy). Choose the best
  architecture, not the smallest change. Record the choices in this file.
- P1: Trace cuts open paths on the line (most complete), in P1 if the
  design allows, else in P6; every closed-only op refuses an open path.
- P6: yes, "extend an open pocket path to the stock" is in scope.
- P7: all four job types (sign, sheet parts, inlay, flatten).
- Rulings that remain open go to the operator only when they change what
  is machined in a way the plan does not cover; otherwise the lead decides
  and records the reason here.

## Lead rulings on DESIGN.md (2026-10-19)

The lead approves `DESIGN.md`. These rulings override it where they differ.

- **D2 and D13 (one ruling): served kinds by default, refuse on an
  explicit act.** The line "every closed-only op refuses an open path" was
  the lead's wording, not the operator's, so the lead rules it. The test is
  how the item reached the op (`ResolvedItem.by`):
  - The default rule selects every item the op can cut. `KindFilter::Drawing`
    becomes `KindFilter::Served` (NEW): the kinds that the op's `ItemUse`
    serves for its current config.
  - A matched item that the op cannot serve, under the default rule, is
    left out. A `SelectionNote` finding counts it by kind and layer
    ("2 open paths on layer TEXT are not cut"). The note goes through
    `ctx.record` and `SelectionStatus`. It is never log-only.
  - An item that the user named (a pick, or a rule that names its kind)
    and that the op cannot cut refuses with `KindNotCut`.
  - When nothing is left to cut, the op refuses with `Empty`, and the
    message names the skipped items and the fix.
  - Reason: both choices stop the chord cut. A skip only removes material
    from the plan, so it is the safe direction. A refusal on the default
    would block every DXF that holds text strokes and outlines.
  - Trace `Left`/`Right` is the same case: its `ItemUse` is `Regions`, so
    open paths are skipped with a note under the default rule. The open
    offset kernel comes in P6.
- **P1 implements this ruling, not section 13 as written.** In P1 every
  input comes from the default rule (no picks exist yet). So the prologue
  gives a `Regions` op only the closed polygons (an owned list), records the
  note for the skipped open paths, and refuses only when no closed polygon
  is left.
- **D1, D3–D8, D10–D12, D14:** taken as `DESIGN.md` recommends.
- **D9 (pinned bottom reaches the motion for four ops): deferred to P3.**
  When P3 opens, the lead reads the G-BOTTOMPIN sentry. If the pinned
  bottom can cut deeper than the op depth, that changes what is machined,
  and the operator rules it. If it only limits the depth, the lead rules.

## Lead rulings on P2.2 (2026-10-19)

- **Signature changes accepted:** `Selection::apply(edit, op_default)`,
  no `TogglePick` (the P2.6 command resolves, then sends `AddPick` or
  `RemovePick`), `ResolveScope.setup`, fallible `to_setup_frame`,
  `SelectionRefusal::for_op`, `Empty { skipped }`, `NoSuchRule`,
  `Selection::from_rules`.
- **Open paths keep the drawn direction (fix b).** `detect_containment`
  stops reversing open paths. Reason: the signed area of an open path is
  the area of its chord-closed ring, which has no meaning. The drawn
  direction defines the P6 side (Left/Right, as Estlcam does) and the
  start point. With this fix, the setup mirror no longer changes the
  order, so the 14 Bottom-flip reversals close. Effect on cutting: only
  the start and direction of an open Trace path, which P1 introduced
  (no older project depends on it).
- **Tie order on equal areas: accepted as known.** It changes the order
  of equal-area loops only, never their geometry. A tolerance comparator
  is not a strict weak order, so the lead does not take that risk. The
  frozen list in the P2.2 sentry stays; a new entry is a regression.
- **Notes:** a `SelectionNote` finding is recorded only for skipped
  `Path` items (a likely drawing defect: an outline that is not closed).
  Other skipped kinds (loops in a Drill, points in a Pocket) are expected.
  They are counted in `SelectionStatus.skipped_by_kind` and give no
  finding. `Empty` still names every skipped item.
- **`UnknownLayer` refuses** (S21: a missing layer is named, never
  ignored).
- **Served admits no `Face`:** a STEP model under `drawing()` refuses
  `Empty`, as today. A face selection is an explicit act.

## Merge gate for geo-select (lead, 2026-10-19)

`geo-select` does not merge to master until P4 restores the face pick for
the Face op and the 3D ops (lost in P2.6 under D5: those ops hold no
selection, so the auto top from a picked face is gone). Also open:
`production_writes_go_through_apply_wp15a` is red (the CLI builds
`set_dressup_field`; not selection work; check whether master is red too).

## Lead rulings on P4_DESIGN.md (2026-10-19)

The lead approves `P4_DESIGN.md` and takes L-1 to L-12 as it recommends
(merge `geo-select` after P4.3 and an operator look).

- **O-1: (A).** A 3D op face pick goes into the boundary selection and
  sets the auto top. Reason: it restores the behaviour before P2.6. The
  explicit "top from a face" reference (B) belongs to P3 with D9.
- **O-3: face a picked region inside its outline.** Only the stock item
  grows by `stock_offset`. Reason: growing a picked region cuts material
  outside what the user picked; inside is the safe direction.
- **O-4: refuse.** Deleting the last hand tab of a loop is refused in
  P4. A per-loop "no tabs" override can come later. Reason: nothing
  cuts differently; the user is told.
- **O-5: yes.** Open SVG subpaths join at the import tolerance. Reason:
  one join rule for every drawing door (as DXF since P1b).
- **O-2: probe first (P4.0).** If the probe shows that an SVG in mm
  imports 3.78 times too large today, the lead reports it to the operator
  as a live defect before any fix. The fix choice goes to the operator.

## P4.0 SVG probe facts (2026-10-19), for the operator

Every door (GUI, MCP, CLI, project file) reads SVG through one function,
`io::load_geometry` -> `load_svg(path, 0.1, 1.0)`, so the doors agree.

| Input | Size (mm) |
|---|---|
| `width="100mm" viewBox="0 0 100 50"`, 100 x 50 rect | 100 x 50 |
| `width="100"` (no units), no viewBox | 100 x 50 |
| `width="100mm"`, no viewBox | 100 x 50 |
| as row 1, rect in `<g transform="translate(10,5) scale(2)">` | 100 x 50 at (0,0): the transform is LOST |
| `width="100mm"`, viewBox in px (`0 0 377.95 188.98`) | 377.95 x 188.98 (3.78 x too large) |

- Cause: the door reads `path.data()` and never applies `abs_transform()`.
  So it takes raw viewBox units as mm and ignores `width`/`height` and
  every group transform. A modern Inkscape file (mm viewBox, no group
  transforms) imports at 1:1.
- The comment at `io/svg_input.rs:7-15` ("coordinates are px") is wrong.
- Open for the operator: the fix (apply the full transform and honour the
  declared physical size is the lead recommendation). Nothing is changed.

## Status

| Phase | State | Commits | Notes |
|---|---|---|---|
| P0 design | done: DESIGN.md approved with lead rulings | (this commit) | branch geo-select from master d8407731 |
| P1 open paths | done (operator look pending) | bba80b4d, 2bf0132c | G-OPGOLDEN unchanged; lead ruling: an open Trace path gets no lead extension past its ends |
| P2 selection core | done (operator look pending) | P2.5 5d906b48, P2.6 1e2ec0c4, P2.7 89df3283 | 349b3a48, e041d244, 4b6353b9, P2.3b | D5 (Face onto Items) deferred to P4: needs a region-offset adapter and a new is_stock_based rule; selection_dropped reports non-default drops only (lead: a default is no user work). P2.3c open: hole_selection runs per frame (cache in P2.4); viewport pick tests model XY but markers draw in the setup frame (pre-existing defect: fix in P2.6 TogglePick); operator look: a click under "every hole" now excludes that hole | P2.1: bridges byte-equal on 25 drawings; open: hand-built test drawings are not nested (check in P2.2) |
| P3 depth refs | COMPLETE (P3.1-P3.8; D9 one floor field with its reference) | 6026eb37, 86265095, 6d403ac1, 272238d8, 1f814929, 294875be, 5236e6c2, 83df571f | pushed to 0346862f up to P3.6; P3.7-P3.8 local |
| P4 UI + sources | P4.0 probe, P4.1-P4.9 and P4.D done (P4.8 without the SVG scale default); operator demo seen 2026-10-20 | 1ca93935, 229eb719, c223d7c4, 9bdfac8e, 3373d8d2, 099d946b, 22cb38fa, 8f5723e3, a53e0cf5, 30e9a146, 0458a586 (SVG O-2), a83d7236 (P4.10); P4 COMPLETE; pushed to ca18398c | next: P3 (D9) after the tech-debt W4 |
| P5 tabs | done (operator look pending) | e76f82c8, P5b | GUI viewport tab editor -> P4. Lead ruling P5b: tab_width = the material bridge (today bridge = width - D: default leaves none); tab_height holds on every level |
| P6 open paths, holes | blocked on P2 | | |
| P7 templates | blocked on P4 | | |
| P8 recognition | blocked on P4 | | |

## Lead review notes on P4.2 and P4.3 (2026-10-20)

- P4.2: the first draft took the stock ring out of the nesting, so a
  Pocket on the stock with a drawing loop cut the loop. The lead sent it
  back: the stock ring nests as before, and `Regions::is_stock` marks the
  region whose exterior is the stock ring. Sentry arm
  `a_pocket_on_the_stock_clears_round_a_drawing_loop_p4_2`.
- P4.3 deviations accepted: Pick sits in the action row; `+ Rule` is an
  inline card; no "Show me" (P4.4 brings the highlight); notes and lost
  picks are one-line rows (IC1); Pick routes face clicks only (hole clicks
  wait for P4.5); the ic1 row cap is 29 (after-state plus 3).
- Operator look for the merge gate (release GUI built from c223d7c4's
  tree): a Pocket on `layers.dxf` (counts, open-path note, `+ Rule`, ✕,
  Reset); a Drill (Reset, Clear after a hole click, the refusal); a
  STEP-only 3D op (the "Selection" boundary button, face picks, Pick on the
  boundary); the `+ Rule` card at the 280-point inspector width.
- P4.4 deviations accepted: a selected item under a refusal is `Skipped`;
  the note "Show me" is a row click (IC2); path marks are outlines;
  foreign items draw in the toolpath setup frame (where they are cut);
  the boundary outline draws a `Selection` boundary only, in `TEXT_BODY`.
  Operator look: states colours on `layers.dxf` and the P4.4 `states.dxf`
  fixture, a lost-pick ghost and its "Show me", Drill markers with the
  states overlay on and off.
- P4.5 deviations accepted: a shift-click with Circles Ø and other kinds
  gives two rules (a diameter filter admits circles only); MCP `add_rule`
  stays explicit (`replace` reaches the same selection); no overlay row
  for the pick bar. Operator look: the bar and its disabled-chip reasons,
  click and shift-click on `layers.dxf`, a Ø 5 shelf-pin layer pick, the
  hover ring on a Trace at the cut level.
- P4.6 deviations accepted: `plan_profile_tabs(pieces, &ProfileTabRule)`
  (dressup/ imports ops::profile for the cut ring); L-5 also applies to an
  add or a delete on a loop with auto tabs (else an add drops the other
  auto tabs); O-4 and L-5 live in core `apply_tab_edit`, a raw MCP
  `tab_points` write skips them. Operator look: drag, add and delete on a
  tabbed Profile, then generate and compare; the drag bar jumps from the
  outline to the cut ring on release (expected); a Bottom 90° setup.
- P4.7 deviations accepted, with follow-ups:
  - FOLLOW-UP: GUI and MCP generation build cold STL faces in `start`, on
    the frame loop (the one-producer input bundle resolves there). The GUI
    job normally fills the cache first. A Generate during "Computing
    faces…" on a large terrain can stall a frame. Fix: move the face build
    into the worker with the input bundle.
  - FOLLOW-UP: STL faces have no viewport highlight (the foreign lookup
    finds no ring).
  - STL mesh hits pick in pick mode only; a plain terrain click is
    unchanged. `FacesNotReady` is a refusal cause and a status field.
  - The editor deleted about 39 GB of stale `target/debug/deps` files when
    the disk filled during clippy (cache only; it rebuilds).
- P4.8 runs without the SVG scale fix. The dialog shows the declared size
  and the scale, and the default scale stays as today (1.0, raw viewBox
  units) until the operator rules O-2. The O-2 ruling is then a change of
  one default plus the transform fix.
- P4.8 accepted: no new command (the surface reads the file again and
  core adopts, as rescale, reload and relink do); the options are on the
  replaced side of adopt; the join tolerance is in file units before the
  user scale (DXF after `$INSUNITS`), and the dialog hover says so; the
  dialog reads the file on the frame thread, on a committed value only.
  SVG_DEFAULT_UNITS is the one place the O-2 ruling changes.

## Operator rulings, 2026-10-20 morning

- **D9: one floor field that names its reference.** Pocket, Profile,
  Zigzag and Adaptive2D get one floor value with a reference (stock top,
  model top, a face, or absolute Z). The op depth and the pinned bottom
  stop being two fields that can disagree. This unblocks P3. Retired keys
  go in `RETIRED_KEYS`, and the break goes in the commit.
- **O-2 (SVG scale): the full fix.** The SVG door applies every
  transform (`abs_transform`) and honours the declared `width`/`height`
  (mm, in, px; a unitless value is px per the SVG spec). A file imports at
  the size it states. Some SVG imports change size; the commit states it.
  `SVG_DEFAULT_UNITS` (P4.8) changes in the same item. The P4.0 probe rows
  become sentry arms with the corrected sizes.

## Operator demo 2026-10-20 morning: item P4.D (before the push)

The operator saw the GUI (b202808d + P4.9 pass 1). These go in one item
after P4.9, before the push:

1. STEP: 2D ops are greyed out when only a STEP model is open. The Add
   toolpath menu and `validate.rs` gate on `items.has_drawing()`. Fix:
   gate on a drawing item OR a face item (the P4.3 boundary button rule).
   A new 2D op on a STEP model starts "pick a face" (D2: the default rule
   takes no face).
2. STEP: the 3D op face flow is hard to find, and a face click gives the
   L-8 notice. L-8 is revised (operator expectation): a face click on a 3D
   op whose boundary is not a selection switches the boundary to
   `Selection` with that face, in one undo step, with a notice. The 3D op
   Geometry tab shows "Pick faces" for any boundary source.
3. DXF: the drawing is not visible after import until an op reads it (the
   drawing upload walks the toolpaths). Fix: a model's drawing draws on
   import, and each model gets a visibility toggle (GUI state; MCP
   `set_ui_view` parity).
4. Layout: the Selection section moves to the TOP of the op Geometry tab,
   next to the model picker.

Parked by the operator ("we can work on that later"): the kind filter in
pick mode and the `+ Rule` card is hard to understand. A follow-up after
P4.10.
- P4.D (30e9a146) accepted. L-8 is revised as above; P4_DESIGN.md
  section 4 row L-8 ("a notice, no edit") is superseded by this note.
  Accepted: a click keeps the stored `enabled` of the boundary; a
  shift-click on a non-Selection boundary still gives a notice; MCP has
  no click act (`click_edit` is in core for a later click tool); a
  neutral drawing draws in the active setup frame. FOLLOW-UPS: a hidden
  model can still be picked (`pick_model_face` does not read the eye);
  the refused section shows two Pick buttons; the parked kind-filter
  wording.
- SVG O-2 (0458a586): the door maps to mm by the declared size and every
  transform (f64 root map; usvg's f32 root group divided out). Tracked
  fixtures now state their mm size, so none changed size; the P2.1
  baseline held bit for bit. FOLLOW-UP: the dated bench copy
  `planning/feed_modulation_calibration/bench_batch_2026-05-27/demo_star.svg`
  now imports at 52.9 mm.
- P4.10 (a83d7236): P4 is complete. Open follow-ups, all small:
  - tab marks still draw on a hidden model (`upload_tab_lines` does not
    read the eye);
  - STL faces have no highlight; a cold STL face build in `start` can
    stall a frame (P4.7);
  - MCP has no click act (`click_edit` is in core);
  - the kind-filter and `+ Rule` wording (parked by the operator);
  - `session/CLAUDE.md` is over 40 lines with the tech-debt lines; the
    owner of the next session/ edit shortens it.

## Lead rulings on P3_DESIGN.md (2026-10-20)

The lead approves `P3_DESIGN.md` and takes L-1 to L-17 as it recommends.

- **O-1: refuse.** An old project with `depth`, or with a pinned Bottom Z
  on Pocket, Profile, Zigzag or Adaptive 2D, refuses at load with a named
  message (`RETIRED_OP_ENTRIES`). No conversion. Reason: the operator's
  standing ruling of 2026-09-16 (no legacy, breaks OK, do not ask again)
  and D9. A pin conversion would cut deeper with no warning. The lead
  tells the operator which of their own job files need a hand edit
  (`testpiece.toml`, `nz_south.toml`).
- **O-2: (A), a hint.** A face pick under the default floor keeps the
  floor; the floor row shows a hint to choose "Each picked face". Reason:
  nothing cuts differently until the user chooses.
- The operator can overturn O-1 or O-2 at the next check-in; both are
  cheap to change before P3.7.
- Order: P3.4 first (selection only, no overlap with the tech-debt W4
  files), then P3.1 and P3.2 after the W4 commit (they touch
  `session/mod.rs` and `project_file.rs`).
- P3.4 (6026eb37) accepted. The editor found that `stepped_block.step`
  and `l_bracket.step` are triangulated inward (signed volume -48000 and
  -36000 mm^3), so their top faces read as down. P3.4 turns the Face item
  normals round for a closed inward mesh (`io::outward_sign`).
  FOLLOW-UP (lead, structural): reverse the triangles of an inward shell
  once in `step_input.rs`, so every reader of a STEP mesh sees outward
  triangles, and delete `outward_sign`. Run G-OPGOLDEN and the 3D sentries
  on those two files first: a slope or flat-face reader may move.
- P3.1 (86265095) accepted. PENDING SWAP after the tech-debt W5 commit
  (lead, one small commit), from the P3.1 editor:
  1. `session/compute/export.rs` `diagnose_toolpath_with_trace`: replace
     `height_context_for_toolpath(tc)` + `ResolvedHeights::from_heights`
     with `self.toolpath_heights(tc).ok().map(|a|
     ResolvedHeights::from_toolpath_heights(&a, &tc.heights))`, and pass
     `heights: heights.as_ref()`. Optionally emit
     `model_reference_refusal` as a diagnostic there.
  2. Delete `height_context_for_toolpath`, `transform_bbox_world_to_local`,
     the "PR-5 polish" doc line, and `ResolvedHeights::from_heights`.
  3. In `one_heights_resolver_for_every_surface_p3_1`: `mcp_today` uses
     `from_toolpath_heights`; remove `#[ignore]` from
     `the_mcp_diagnostics_read_the_face_top`. Fix the doc comment at
     `tests/depth_beyond_stock_core_g_depthstockcore.rs:351`.
  Accepted deviations: `ToolpathHeights::from_context` is public (two
  ungated core tests use it); L-4 is a value on the answer, and generation
  refuses through `generation_heights`; `TopSource` has three flat arms.
- P3.2 (6d403ac1) accepted, with the P3.1 export.rs swap. Notes:
  - L-16 refuses an empty ladder on EVERY Layered op (Trace too), not
    only the four floor ops. The safe direction; the operator is told.
  - The floor is driver-set, so `get_operation_schema` lists no floor
    and the W6a compile check does not reach its numbers (domains in
    prose in `floor.rs`). P3.3 adds `set_toolpath_floor` and a schema
    entry with domains.
  - GUI until P3.7: the Heights-tab Bottom row of the four ops stays
    editable and snaps back with no message; the diagram still says
    "BZ (not used)". P3.7 owns both.
  - FOR THE OPERATOR: `planning/rivmap_block_job/testpiece.toml` needs a
    hand edit (depth -> floor); `nz_south.toml` still refuses at load
    because of `[boundary.source.model_outline]` (retired in P2.5); its
    generator `derive.py` is stale and a rerun reverts the rewrite.
- P3.3 (272238d8) accepted. Committed from the index: session/mod.rs
  held a tech-debt W6b hunk, which stays in the tree. Deviations
  accepted: Top refuses depth <= 0, other references take a signed depth
  (`Floor::domain` is the one rule; W6b calls `door_refusal`); the heights
  block is on `get_toolpath_params` and the floor reply, not on the 500 ms
  summary replies. Open: `set_toolpath_param depth` on the four ops still
  says "unknown parameter" (its text is in a W6b file); `item_states.floor_z`
  is P3.5.
- P3.5 (1f814929) accepted. Deviations accepted: one ladder function
  (no new ladder hooks); `FloorRef::EachFace { highest, deepest }`; the
  prologue refuses NoFace and AboveTop by item (status = generation);
  Adaptive 2D refuses in its adapter until P3.6. LEAD QUESTION for P3.7:
  under EachFace the auto top is the stock top (L-15). With an auto stock
  from the model box, a pick of the TOP face has floor = top, and the
  whole op refuses FloorAboveTop. Consider: such a region has nothing to
  cut, so skip it with a SelectionNote, not refuse the op. Decide before
  P3.7 (it is a UX rule; nothing cuts differently).
- P3.6 (294875be) accepted. The two adaptive reds in its gate
  (g_adaptpassload, g_adaptrapidlift) are pre-existing (full gate at
  ca18398c) and red with P3.6 reverted.
- Lead ruling for P3.7 (the P3.5 question): under EachFace, a face whose
  floor is at or above the top has nothing to cut. It is skipped with a
  `SelectionNote` that names the face ("at the top: nothing to cut"); the
  op refuses `FloorAboveTop` only when every face is skipped. Reason: the
  user picked the faces to cut; one face at the stock top must not block
  the others, and the note says why it is not cut.
- STEP inward shells (fdea1263): the triangles of an inward closed shell
  are reversed once at import; `outward_sign` is deleted. The 3D ops that
  read normals or slope change on `stepped_block.step` and `l_bracket.step`
  only (the old output was computed on inverted triangles); G-OPGOLDEN and
  the other six STEP fixtures do not move. FOLLOW-UP: `classify_face_surface`
  signs a plane normal from the plane Z, not the face (the BREP edge concave
  flag reads it).
- P3.7 (5236e6c2) accepted, with the three tech-debt W8 follow-ups.
  Deviations accepted: the O-2 hint draws in `toolpath_panel.rs` under the
  section (the section writes only `EditToolpathSelection`, L-1) and sets
  `EachFace { depth: 0 }`; a combo change into Each picked face keeps the
  deepest floor Z; the resolved Z has its own row (rail width). Gaps for
  P3.8: "Show me" on an at-the-top note frames every skipped face of the
  layer; the diagram floor runs have no hover; no MCP test asserts
  `floor_z`.
  Operator look (release GUI built with W15a work in progress; rebuild
  from HEAD first): each reference on a Pocket (the "= Z" row stays);
  the Heights tab Floor line; the red FL run and dashed F1/F2 runs on
  `stepped_block.step`; the hint button; a face at the top skipped with a
  note; Stock bottom 0.5 for a through cut; an absolute Z on a flipped
  setup.
- P3.8 (83df571f): P3 is COMPLETE. Open, small: the diagram hover has no
  pointer test; the floor-depth CLI sweep (L-12, a path syntax); stale
  headers in `bottom_z_pin_note_g_bottompin.rs`,
  `selection_refusals_name_the_item_p2_4.rs` and
  `every_op_keeps_its_toolpath_g_opgolden.rs`; the GUI look at the P3.7
  and P3.8 surfaces (rebuild the release GUI from HEAD first).
- Next: P6 (open paths and holes) needs a design pass (P6_DESIGN.md),
  then P7, P8.
- Tech-debt F033 (a06a3168), owned here: one height-order rule
  (`compute::config::height_order`); generation, export and every edit door
  refuse an inverted stack; the badge and the diagnostics agree.

## P6 design and operator rulings (2026-10-21)

`P6_DESIGN.md` is approved; the lead takes L-1 to L-19 as recommended.
Operator rulings:
- **O-1: the label wins.** Trace Left puts the tool on the left of the
  drawn direction, on closed loops and open paths alike (G41, Estlcam).
  Existing closed Trace Left/Right projects cut on the other side; the
  commit states the break.
- **O-2: extend to stock, opt-in.** Off by default; pocket on the left of
  the drawn direction, a Flip button, a preview; the region ends one tool
  radius past the stock edge.
- **O-3: Drill skips circles that are not its size,** with a note; a
  hand-picked one refuses (S11). A new Bore op helix-mills them.

Defects found by the design:
- **D-C (SAFETY, first):** Profile Outside offsets go through
  `Polygon2::from_pline`, which keeps only the chord of each arc join, so
  the tool centre cuts r(1 - cos 45 deg) = 0.29 r into a 90 deg outside
  corner (`polygon.rs` FlattenPolicy note). Item P6.0, before P6.1.
- D-B: `gcode/mod.rs` maps Outside + climb to G42 (looks swapped; no
  effect without controller compensation). Checked in P6.1.
- D-A is O-1; D-D is O-3.
- P6.0 (e1dfa30e, SAFETY) done: the offset chokepoint flattens arc joins
  on the arc (<= 0.010 mm); 11 golden cases re-blessed by name.
- Lead ruling F033b: for an op whose ladder includes its top
  (Waterline, Ramp Finish, Steep and Shallow: `LadderMode::ConstantStep`),
  top = bottom is one level, not an inverted stack; F033 must not refuse
  it. A `Capabilities` field states it (W2.4: no match on the op type).
  Reason: before F033 such an op cut one contour; F033 made it refuse.
- Next: F033b, then P6.0b (the 0.039 mm corner intrusion that the
  default Profile dressups leave: segment merge / arc fit on a finished
  edge), then P6.1-P6.8.
- Operator demo 2026-10-21 (clean build of 6dd89feb): "logic seems
  there"; a big picking UX pass follows after P6. DEFECT R-ARC (operator):
  the toolpath view draws every move as a straight segment from the last
  target, ArcCW/ArcCCW included (`render/toolpath_render.rs`, the per-move
  loop). An arc-fitted small circle (two half arcs whose ends sit 0.5 mm
  apart) draws as a zigzag; the simulation and the G-code follow the arcs.
  Fix (viz only, after P6.0b): tessellate an arc move with the core arc
  rule that the simulator uses, in every draw and pick site that reads
  moves as lines; a sentry that the drawn polyline of an arc move lies on
  its arc.
- P6.0b (b0a13361, SAFETY) done: the segment merge is capped at 0.010 mm
  on every finished-edge op (`OperationTransformCapabilities::finished_edge`);
  G-OPGOLDEN (4 cases) and perf_golden_sim_metrics (Profile; P6.0 had
  left it unblessed) re-blessed by name.
- P6.0c (SAFETY, next): the lead-in/out curls left of travel always
  (`apply_lead_in_out`); on a closed Trace Left the part is on the left,
  so the lead cuts 1.84 mm in. Profile with climb = false is suspect.
  The lead must go to the side away from the material, from the op's own
  knowledge of the material side per pass. Pinned by
  `trace_left_lead_curls_into_the_part_known_red`.
  Measured on HEAD (lead r 2, tool r 3): wrong side 1.84 mm (Profile
  Outside conventional, Trace Left); concave start corner 2.00 mm (Profile
  Inside, Pocket island ring, Trace Right/None, Chamfer Inside: the lead-in
  goes back along the previous edge). Fix: each op states `RingMaterial`
  per closed pass (a required argument of `emit_closed_contour_with_intent`);
  `dressup/lead_side.rs` turns the arc away from it; a lead point on the
  material side keeps the plunge. All 11 arms within 0.020 mm.
  Lead decisions: Trace None (`BothSides`) gets no lead, not a refusal,
  because Trace has the lead on by default.
- P6.0d (follow-up of P6.0c, not started):
  - An open pass with a known side (Trace Left/Right open path) keeps its
    lead. P6.0c gives every open 2.5D pass no lead (Face, Zigzag, Adaptive
    2D, open Trace).
  - A lead that the operator asked for and the dressup drops gives a note.
  - A closed ring starts in the middle of an edge, not at a concave corner,
    so the lead has room.
  - `RingMaterial::NotStated` (Waterline, Steep and Shallow) states a side.
  - The air check reads one pass ring only. A lead near a second wall (a
    narrow gap between an island and the outer wall) is not checked.
- P6.0c done (5d4e2a6c).
