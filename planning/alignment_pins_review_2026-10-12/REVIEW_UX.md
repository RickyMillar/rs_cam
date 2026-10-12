# Alignment pins: UX review of the pin section (2026-10-12)

Scope: the stock panel's "Alignment pins" section (P7,
`crates/rs_cam_viz/src/ui/properties/pins/`) after waves A-E. Trigger: the
operator's complaint about the mixed preset refusal. Lead rulings R1-R5 are
in the agent prompt; this file lists what the review finds and the fix.

## Root cause of the complaint

The mixed preset refuses because no drill larger than the pin drill exists.
The refusal sends the operator to another screen ("Add a drill operation").
The deeper defect: one pin-drill operation with ONE tool drills EVERY pin
hole. A mixed set (two diameters) is not machinable: the large hole gets
the small drill. So the refusal hid a missing capability (R1).

## Findings, ranked

1. **One drill for all pin sizes (blocker).** `AlignmentPinDrillConfig`
   has no diameter. The generation pass copies every pin into one op.
   Fix: one pin-drill op per distinct pin diameter (R1). The door creates,
   updates and deletes the ops. Findings, notes and MCP replies read the
   per-size bindings.
2. **Messages that send the operator away (high).** Three texts do this:
   the mixed preset refusal, `PIN_DRILL_NEEDS_TOOL` with its "Add a drill
   operation" button (it adds a generic Drill op that the operator must
   then set up on the operation tab), and the "Add a setup" hover ("set its
   face to Top"). Fix: a per-size row in the pin section with a tool
   chooser and "Add a <d> mm drill" (R2). A source test keeps every pin
   text free of "operation tab" / "then choose the preset again" wording.
3. **No picture of the set (high).** The operator reads X/Y numbers and
   turns to the 3D viewport to see where the pins are. Fix: an overhead
   plan view in the section and a large one in the wizard (R3, R4).
4. **The tool of a pin size is not visible (medium).** The chips name one
   op. The table has no tool column. A pin of a size that no drill makes
   shows only as a finding in a closed fold. Fix: a "Drills" block, one
   row per pin size: size, op, tool chooser, state (R2).
5. **The 3-pin preset diameter is a free number (medium).** A drag to
   7.3 mm gives pins that no drill fits, with no visible cue. Fix: the
   per-size row shows "no drill of this size" at once, with the add
   button. The free field stays (a dowel can have any size).
6. **The mixed preset needs a large size from nowhere (medium).** Fix: the
   mixed choice opens a size picker: the existing larger tool diameters,
   and a start value derived from `PIN_MATCH_TOL_MM` when no larger tool
   exists. "Add a <d> mm drill" sits beside it.
7. **The load-warning modal keeps a stale pin warning (medium, R5a).**
   The loader text stays until Close, after a pin edit keys the set. Fix:
   the pin door drops a load-time pin warning that the live check no
   longer gives, and closes the modal when no warning is left. MCP
   `set_ui_view` with modal `none` also closes it.
8. **The section clips at 1600 px (medium, R5b).** The keying banner and
   the "Use 3-pin preset / Move the pair off the axis" row run past the
   panel edge. Fix: the buttons wrap one per line at narrow width; the
   banner text wraps to the available width.
9. **No guided set-up (low, operator request).** Fix: the "Pin setup..."
   wizard (R4): flip, dowel, placement, summary. It works on its own
   draft; Cancel changes nothing.

## Not fixed, and why

- The wizard Apply is not ONE undo step. `UndoAction` has no variant for
  a tool add or a toolpath add; the door's pin-drill sync is outside undo
  today. The pin edit itself is one `StockChange` undo step, as now.
- The 3D viewport keeps its own pin marks. The plan view does not replace
  them; both read the one report.

## Round 2 (2026-10-12, after a live look)

Operator: "the wizard first page has no options. loads of text. bad ui.
... in 3, can you shade the stock different from the model. and also
review the snap to fit ... remove the summary page completely ... the
stock side bar. its too busy."

Changes (lead rulings 1-6):

1. **Stock panel: one block.** A status line ("2 pins · do not key"), at
   most one warning in the danger colour, and ONE button ("Set up pins…" /
   "Edit pins…") that opens the wizard. The pin editor left the panel:
   presets, set controls, nudges, table, drills, plan view, chips, single
   pins, warning boxes and the part-model chooser.
2. **Wizard: two steps.** Step 1 "Dowels": an animated flip diagram of the
   judged setup (a card turn computed from its `PinSetupMap`; a hover shows
   the part turned over) with a one-line caption, or the refusal and "Add a
   Bottom setup"; then the pattern, the sizes and the drill rows. Step 2
   "Place": the plan view, Auto-place, a move along the axis, the pattern
   values and the part models; the keying and wall chips sit beside Apply.
   The summary step is gone.
3. **Less text.** Every paragraph became a label or a short hover. A test
   holds every painted wizard text to one short line, except a refusal.
4. **Plan view shading.** The stock and the part footprint have two fills
   (`DIAGRAM_MATERIAL`, new `DIAGRAM_PART`), with a "Stock / Part" legend.
   A scan-line fill paints a concave outline correctly.
5. **Snap deleted.** Every pin control keeps the set symmetric: a set move
   (drag or nudge) goes along the flip axis only, the distance off the axis
   is "Pair off axis", and a size changes per pattern role. Single-pin
   editing is gone. Core `pattern::snap_to_flip` had no caller left and is
   deleted with its two tests (a break).
6. **The wizard holds the drill fixes.** A drill add and a tool choice wait
   in the draft until Apply. "Add a Top/Bottom setup" and the part-model
   choice apply at once through the pin door; their hover says so.
