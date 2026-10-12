# `ui/properties/pins/` — the stock panel's alignment-pin section (P7, R1-R5)

The operator edits the pins AS A SET. Entry: `mod.rs::draw`, called from
`../stock.rs`. "Pin setup..." opens the wizard (`ui/pin_wizard.rs`).

## Files

- `mod.rs` — the section, `PinSectionInputs` / `PinSectionOut`,
  `PinDrillState` (one `PinDrillRow` per pin size), `KeyedPinInputs`,
  `keyed_pin_plan` (the placer door), `wrapped` (a label that wraps).
- `edit.rs` — pure set edits: preset choice, move all, snap, "Move the
  pair off the axis", `mixed_preset_pins`. No state, no drawing.
- `controls.rs` — preset picker, pattern and set controls, Auto-place, the
  part-model chooser, the closed "Edit single pins" fold.
- `status.rs` — the chips, the keying warning and its buttons, the table.
- `drills.rs` — one row per pin size: the drill, a tool chooser, "Add a
  <d> mm drill", "Add a Top setup" (R2). `plan_view.rs` — the view from
  above; a drag moves the set (R3).

## Invariants

- Every number comes from ONE core report,
  `ProjectSession::pin_set_report_for(draft pins)`: also the plan view's
  outlines (`obstacles_local`) and the drills (`pin_tools`).
- One pin-drill op per pin diameter (R1). Never ask the operator to go to
  another screen: fix a missing drill in place (`AppEvent::PinDrill*`).
- An edit writes the stock DRAFT. A drag commits once, on release
  (`PanelEdit::drag`); a button commits on its click.
- A preset control writes through the `fit_*` inverse in core. Keep its
  range inside the fit (`edit::MIN_PAIR_ACROSS_MM`, `MIN_DIAMETER_STEP_MM`).
- A distance comes from the placer or a printed constant. Never type one.
- The inspector scrolls both ways: a plain label does not wrap. Use
  `wrapped`, and keep a row no wider than the 240 point rail (R5b).

## Sentries

- `cargo test -p rs_cam_viz -q --test pin_editor_and_viewport_p7 --test pin_section_fixes_in_place_r2 --test pin_wizard_apply_cancel_r4`
- `cargo test -p rs_cam_viz -q --test keyed_pin_panel_reads_the_real_strips_g_pinpanel`
- `cargo test -p rs_cam_viz -q --lib ui::properties::pins pin_edit_one_door_p5`
