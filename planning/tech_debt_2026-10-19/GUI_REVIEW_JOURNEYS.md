# GUI review journeys — tech-debt programme 2026-10-19

Follow each journey in a GUI built from HEAD. Each step names what you
do and what you must see. Tick the box when it holds; write a note when
it does not. Part B holds the geometry-selection journeys (P4–P6).

Not yet walked: both parts come from the editors' reports and the code,
not from a look at a running GUI. Button names, labels and click paths
may differ on screen; note each difference.

Before you start:
- The lead rebuilds the release GUI from HEAD (`gui_status` shows the
  binary is not newer than the process).
- Test files: `fixtures/gui_step/*.step`, `fixtures/demo_pocket.svg`,
  `fixtures/terrain_small.stl`, `planning/fixtures/rivmap100/rivmap100.toml`.
- For a "bad file" step, edit a COPY of a project in `/tmp`, never the
  fixture.

## Part A — tech-debt changes

### A1. Tool editor refuses an impossible tool (W1)
- [ ] Add a V-bit. Type 0 in the included angle. The editor keeps the
      old value or shows a refusal. The GUI does not crash.
- [ ] Drag the diameter field. It moves inside the old GUI range. A
      saved value outside that range stays as it is (it is not clamped).
- [ ] Save the tool to the library. Change only its name. Save again.
      The library keeps one tool: name and stickout do not make a new
      tool. Change the diameter and save: the library holds two tools.

### A2. Parameter ranges at every door (W6a, W6b)
- [ ] On a Pocket, type 0 or a negative value in the feed rate. It is
      refused. Plunge, ramp feed and spindle speed act the same way.
- [ ] On a Drop Cutter (or Scallop), set `slope_from` above `slope_to`.
      It is refused, and the text names both fields.
- [ ] In the stock panel, type 0 in a stock edge. It is refused.
- [ ] Apply Suggest on any op. The values it writes stay inside the
      ranges; a value outside a range shows a refusal, not a write.

### A3. Project file refusals (W4)
- [ ] Copy a project. Change its `z_datum` to `stock_topp`. Open it.
      The load refuses and names the setup and the token.
- [ ] Copy a project. Add an unknown key at the top level. Open it. The
      load refuses and names the key.
- [ ] Copy a project. Delete a toolpath's `tool_id`. Open it. The load
      refuses.

### A4. Export (W5, F005 follow-up)
- [ ] Generate, do not simulate, then Export. The refusal names the
      override in the GUI's words (the pre-flight label).
- [ ] Accept the override and export. The record says "OVERRIDE USED".
- [ ] Use the keyboard export. The toasts show the load outcome, the
      machine-safety findings and the feeds cautions.
- [ ] Set the post to LinuxCNC. Add a snippet with `G81` and export.
      The machine-safety findings name the canned cycle (the LinuxCNC
      post data says the controller does not run canned cycles).
- [ ] Turn on high-feedrate mode on the post. Export from the GUI and
      from the CLI (`rs_cam_cli project --emit-gcode`). The two files
      are byte-equal.

### A5. One verdict, one stepover rule (W3)
- [ ] Run Simulation. The project verdict in the GUI, in MCP
      `get_project_diagnostics` and in the CLI summary is the same.
- [ ] Set a stepover equal to the tool diameter. It is accepted. Set it
      above the diameter. It is refused on every surface.

### A6. Feeds (W10, F216)
- [ ] On a Drill, Apply Suggest with scope "Both". The peck depth
      changes. With scope "Speeds", the peck stays.
- [ ] Select a toolpath and leave the panel open. The GUI stays smooth;
      the feeds numbers and pills update only after an edit.

### A7. Post panel (F072)
- [ ] Change the post format in the Post panel. Undo. The format goes
      back. Redo. It comes back. The project shows as edited.

### A8. Simulation and the apply door (W9a, W9b, W14b)
- [ ] On a new project with no generated toolpath, open the Simulation
      menu. "Run Simulation" is greyed out.
- [ ] Simulate a project with feed modulation on. Switch the viewport to
      the feed colouring. The colours show the modulated feeds (the
      feeds the exported file holds), not the planned feeds.
- [ ] Run the multi-tool planner. The viewport simulation clears, and
      the replaced toolpaths show as stale.
- [ ] Make an edit that core refuses (for example, move a toolpath to a
      setup that does not exist through MCP). The project does not show
      as edited.

### A9. File dialogs do not freeze the GUI (W18)
- [ ] Open File > Import STL and leave the dialog open. The GUI behind
      it keeps redrawing. An MCP call (`project_summary`) answers while
      the dialog is open.
- [ ] Click Import again while the dialog is open. Nothing new opens.

### A10. Legend, span help, dressups (W8, W14b)
- [ ] Open the viewport legend. The entry, lead-out and link-bridge
      swatches match the drawn line colours.
- [ ] Hover a debug span in the simulation debug view. It shows help
      text (17 kinds gained help).
- [ ] On a Roughing op, look at the dressup count "N/8 active". It
      counts segment merge.

### A11. MCP wire (W17)
- [ ] MCP `set_stock_config` with `padding`. The stock grows by it.
- [ ] MCP `set_tool_param` with `helix_deg`. The tool changes.
- [ ] MCP `set_spindle_strategy` with an old alias (`MatchChart`). It
      is refused by the wire, before the GUI.

### A12. Refusal text (W12)
- [ ] Through MCP, set a toolpath's model to an id that does not exist.
      The text reads "Model id N not found; project model ids: [..]",
      and the refused field is `model_id`.

## Part B — geometry selection (P4–P6, since the 2026-10-21 demo)

Fixture A: `crates/rs_cam_core/tests/fixtures/selection_p4_4/states.dxf`
(mm): three nested squares on layer OUTER (0..60, 10..50, 20..40); a
Ø 4 circle at (5,5) and a point at (80,10) on HOLES; an OPEN polyline on
TEXT, (0,70)→(15,80)→(30,70); squares on KEEP and SPARE.
For each journey on A: import A into a new project, size the stock to
cover it (X 0..110, Y 0..85 or larger), and add a 6 mm flat end mill.

Fixture B: `crates/rs_cam_core/tests/fixtures/wanaka100/wanaka_full_tuned.toml`
(open as a project): a Drill on `rivmap_export/holes.dxf`, 12 circles of
Ø 5.859, a 6 mm tool, size tolerance 0.2.

### B1. Finished edges keep their corners; leads go away from the part (P6.0, P6.0b, P6.0c)
- [ ] Add a Profile on OUTER, side Outside, with a lead-in or lead-out
      arc. Generate. The outer corners are true arcs (no chord chamfer).
      Each lead arc curls OUTSIDE the square, into the air.
- [ ] Set side Inside. Generate. The lead arcs curl to the waste side,
      never into the wall that stays.
- [ ] Add a Trace on TEXT, compensation "On the line". No lead arc (by
      design: the line has no side).

### B2. Arcs draw as arcs (R-ARC)
- [ ] On the B1 Profile, zoom in on a corner and on a lead. The arcs are
      smooth, with no zigzag. A click on an arc move selects that move.

### B3. Profile "On the line" (P6.1, P6.3, P6.7a)
- [ ] Add a Profile on TEXT and OUTER. Side offers Outside | Inside |
      On the line.
- [ ] Pick "On the line". In Advanced, Compensation "In Control" is
      greyed, with the hover "side On has no side for G41/G42". A Climb
      hint for open paths shows (the drawn direction). The Tabs header
      reads "Tabs · closed loops only".
- [ ] Generate. The tool centre runs ON the open TEXT line, end to end,
      with no lead. The closed squares are cut on the line too. Tabs go
      on the closed loops only.
- [ ] Set the floor to "Each picked face". "On the line" is greyed with
      the refusal text. Known gap: the Floor row still offers Each
      picked face under On, and that choice refuses with no message.

### B4. Trace on an open path, Left or Right of the drawn direction (P6.2, P6.7a)
- [ ] Add a Trace on TEXT. Compensation offers "On the line", "Left of
      the drawn direction", "Right of the drawn direction". The diagram
      shows an arrow and the tool on its side.
- [ ] With the selection-states overlay on: a direction arrow at the
      middle of the TEXT path (drawn left to right from (0,70)). For
      Left, a tool ring on the left of travel (above the line); for
      Right, below it.
- [ ] Generate Left, then Right. The cut follows the ring side.
- [ ] Optional: a Bottom setup mirrors the ring and the cut together.

### B5. Pocket "Extend to stock" (P6.4, P6.7a)
- [ ] Add a Pocket on TEXT. Open paths: Skip (the default) gives a
      note that the open path is not cut.
- [ ] Pick "Extend to stock", Side "Left of the drawn direction". The
      Selection section reads "1 path extended". The viewport outlines
      the area from the line to the stock edge in the cut colour.
- [ ] Click "↔ Flip". The area moves to the other side of the line. One
      undo brings it back.
- [ ] Generate. The pocket reaches one tool radius past the stock edge,
      so no thin strip stays at the edge.
- [ ] Move the stock so that one end of the line is outside it. The
      refusal reads "the open path cannot extend to the stock: an end of
      the path is outside the stock", with its Fix and Show me.

### B6. Drill hole fit and size tolerance (P6.5, P6.5b, P6.7a)
- [ ] Open B. Select the Drill. The Selection section has one row per
      size: "Ø 5.859 × 12 drilled" (Ok chip). Generate works.
- [ ] Set "Size tolerance" to its minimum (0.01). The rows read
      "smaller than the tool" (Caution), the op refuses (HoleDoesNotFit
      names the hole), and the misfit drill markers draw in the skipped
      colour. Set it back to 0.2.
- [ ] On A: add a Drill on HOLES with the 6 mm tool. "Ø 4.000 × 1
      smaller than the tool" (Caution), and a points row "drilled at
      Ø 6.000" for the point.

### B7. Bore (P6.6, P6.7a)
- [ ] Open B. Add a Bore with a 3 mm end mill on holes.dxf. The rows
      read "Ø 5.859 × 12" with a helix radius of about 1.43 (Ok).
- [ ] Generate and simulate. A helix in each hole, and a clean wall of
      Ø 5.859.
- [ ] Change the tool to 2 mm. "leaves a core" in Danger (the hole is
      more than twice the tool), and the op refuses.
- Not built yet: "Add a Bore for these" on the Drill misfit note
  (P6.7b, after W13b).

### B8. Height order in the Heights tab (F033, F033b)
- [ ] On a Pocket, open the Heights tab. Drag Clearance below Retract.
      The drag stops at the last value in order, and the tab names the
      break in red.
- [ ] Type a value that breaks the order. It refuses with one text.
- [ ] On Waterline or Unified Finish, set Top = Bottom. It is allowed
      (one level). Unified Finish drops its very-steep band with a
      finding, and the other bands cut.
- [ ] Generate and export refuse a broken stack. A Feed Z out of order
      is a yellow caution only.

### Known gaps (in `planning/geometry_selection_2026-10-16/PLAN.md`)
- The Drill shows one HoleFit note per circle beside the size rows.
- Trace Left/Right on a closed loop has no side ring.
- The side ring shows only with the selection-states overlay on.
