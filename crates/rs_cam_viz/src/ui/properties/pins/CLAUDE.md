# `ui/properties/pins/` — the alignment pins (P7, R1-R5, round 2)

The stock panel shows ONE block (`mod.rs::draw`, from `../stock.rs`): a
status line, at most one warning and ONE button that opens the pin wizard
(`ui/pin_wizard.rs`). The wizard edits the pins; this folder holds its parts.

## Files

- `mod.rs` — the block, `PinDrillState` (one `PinDrillRow` per pin size),
  `PartModelRow`, `KeyedPinInputs`, `keyed_pin_plan` (the placer door).
- `status.rs` — `summary` (the block's words), `chip_words` (the chips).
- `edit.rs` — pure set edits: presets, sizes per role, a move along the axis.
- `drills.rs` — one row per size: drill, tool chooser, "Add a <d> mm
  drill", "Add a Top setup". Adds and choices wait in `DrillQueue`.
- `plan_view.rs` — the view from above; stock and part fills, a legend.
- `flip_view.rs` — the flip diagram: a card turn from the judged map.

## Invariants

- Every number comes from ONE core report,
  `ProjectSession::pin_set_report_for(draft pins)`: also the outlines
  (`obstacles_local`), the drills (`pin_tools`) and the flip (`map`).
- The set stays symmetric under the flip. A set move goes ALONG the flip
  axis only; the distance off it is the pattern's "Pair off axis"; a size
  changes per role. No snap, no single-pin edit.
- One pin-drill op per pin diameter (R1). Never send the operator to
  another screen: fix a missing drill in the wizard.
- The wizard edits its OWN draft; Apply is one `PinWizardApply` to the pin
  door. Only "Add a Bottom/Top setup" and the part models apply at once.
- A preset control writes through the `fit_*` inverse in core. Keep its
  range inside the fit (`edit::MIN_PAIR_ACROSS_MM`, `MIN_DIAMETER_STEP_MM`).
- A distance comes from the placer or a printed constant. Never type one.

## Sentries

- `cargo test -p rs_cam_viz -q --test pin_editor_and_viewport_p7 --test pin_section_fixes_in_place_r2 --test pin_wizard_apply_cancel_r4`
- `cargo test -p rs_cam_viz -q --test keyed_pin_panel_reads_the_real_strips_g_pinpanel`
- `cargo test -p rs_cam_viz -q --lib ui::properties::pins pin_edit_one_door_p5`
