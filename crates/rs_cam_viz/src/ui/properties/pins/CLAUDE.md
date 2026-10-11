# `ui/properties/pins/` — the stock panel's alignment-pin section (P7)

The operator edits the pins AS A SET. Entry: `mod.rs::draw`, called from
`../stock.rs`.

## Files

- `mod.rs` — the section, `PinSectionInputs` / `PinSectionOut`,
  `PinDrillState`, `KeyedPinInputs`, `keyed_pin_plan` (the placer door).
- `edit.rs` — pure set edits: preset choice, move all, snap, "Move the
  pair off the axis". No state, no drawing.
- `controls.rs` — preset picker, pattern and set controls, Auto-place, the
  part-model chooser, the closed "Edit single pins" fold.
- `status.rs` — the chips, the keying warning and its buttons, the table.

## Invariants

- Every number comes from ONE core report,
  `ProjectSession::pin_set_report_for(draft pins)`. Do not compute a pin
  number here; the MCP `pin_report` and the viewport read the same report.
- An edit writes the stock DRAFT. A drag commits once, on release
  (`PanelEdit::drag`); a button commits on its click. The inspector sends
  `SetAlignmentPins` (or `SetPinPartModels`) through `stock_edit_command`.
- A preset control writes through the `fit_*` inverse in core. Keep its
  range inside the fit (`edit::MIN_PAIR_ACROSS_MM`, `MIN_DIAMETER_STEP_MM`),
  or a drag drops the set out of the preset and the control disappears.
- A distance the panel offers comes from the placer or a printed
  constant. Never type a distance.
- The pin-drill chip reads `controller::pin_drill_blocker`, the door's own
  check.

## Sentries

- `cargo test -p rs_cam_viz -q --test pin_editor_and_viewport_p7`
- `cargo test -p rs_cam_viz -q --test keyed_pin_panel_reads_the_real_strips_g_pinpanel`
- `cargo test -p rs_cam_viz -q --lib ui::properties::pins`
