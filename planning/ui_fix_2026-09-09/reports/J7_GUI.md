# J7 GUI half — F1.19 / G-BOTTOMPIN: the Heights tab annotates the Bottom Z row

Date: 2026-09-10. Branch: `ui-fix/lane-j78`. Worktree:
`/home/ricky/personal_repos/rs_cam_wt_p3`. Base: `9292287a`.

The core half is `reports/J7.md`. It measured the fact and shipped the
declaration `OperationType::honors_pinned_bottom_z()`
(`crates/rs_cam_core/src/compute/catalog.rs:618`). This report covers the GUI
half only.

## The decision: ANNOTATE, not disable

The panel prints a sentence beside the Bottom row. The row stays editable.

Three reasons, in order of weight.

1. **A disabled row is a trap on a legacy project.** A stored pin can put the
   resolved bottom ABOVE the resolved top. Two surfaces then report it: the
   Heights tab badge turns red (`compute_tab_badges`,
   `crates/rs_cam_viz/src/ui/properties/mod.rs:3204-3212`, which resolves the
   entry's own heights), and the core check `geom.bottom_above_top_z` prints
   `Bottom Z (…) is above Top Z (…). No material will be cut.` A disabled row
   cannot be set back to Auto, so the operator would read two complaints
   about a field the panel had taken away. The one control that clears the
   pin must stay.
2. **The brief requires the stored value to stay visible.** An annotated row
   shows it and keeps the "= −7.0" resolved hint beside it. A disabled row
   would show it too, so this reason alone does not decide; combined with (1)
   it does.
3. **A note is visible without a hover.** The brief allows a disabled control
   only with a hover reason. The panel's disabled-reason idiom
   (`add_enabled(false, …).on_disabled_hover_text(reason)`, used for Feed
   rate optimization at `mod.rs:5669-5673`) puts the whole explanation behind
   a pointer. The Bottom row's problem is that the operator does not know to
   ask, so the sentence has to be on the screen.

## What I changed

All three edits are in `crates/rs_cam_viz/src/ui/properties/`.

| Site | Change |
|---|---|
| `operations/mod.rs`, new `bottom_z_pin_note` | The one decision. `Option<&'static str>`: `None` when the operation reads the pin, else the sentence naming the dial that does set the floor. |
| `operations/mod.rs`, `draw_heights_params` | Takes the `OperationType`. Prints the note under the grid. Builds the Bottom row tooltip from it. |
| `mod.rs:5164` | Reads `entry.operation.op_type()` and passes it. The one call site. |

The note names a different dial per family, because the operations do:

- `Face` / `Pocket` / `Profile` / `Adaptive` / `Rest` / `Zigzag` / `Trace` /
  `Drill` — "Its Depth field sets the floor."
- `VCarve` — Max Depth. `Inlay` — Pocket Depth. `Chamfer` — Chamfer Width.
- `AlignmentPinDrill` — "Its Depth and Spoilboard set the floor."
- `ProjectCurve` — "The projected surface sets the floor."
- the eight surface-riding finish ops — "The model surface sets the floor."

### The tooltip was the second untrue sentence, and it is gone

The Bottom row shipped this hover text:

```
Deepest cut depth. The tool will not cut below this Z. Left on auto, the
operation decides its own floor — 3D operations drive it from the model, and
pinning this row overrides that.
```

"pinning this row overrides that" is true on three operations and false on
twenty-one. The three honouring operations now read `Deepest cut depth. The
tool stops at this Z.` (`BOTTOM_Z_TOOLTIP`), which is true of them and of them
only. The other twenty-one get the note plus "The row still shows the stored
value."

## No emitted move changes — what I checked, and how

I did not run a generate. The MCP server is down and no cargo command is mine
to run, so this is a read of the change surface, not a measurement.

- The commit touches two files, both under
  `crates/rs_cam_viz/src/ui/properties/`. Nothing under
  `crates/rs_cam_core/src/compute/`, `crates/rs_cam_core/src/session/` or
  `crates/rs_cam_viz/src/compute/` is in the diff. Those are the paths that
  build the move list.
- `bottom_z_pin_note` is pure: it takes a `Copy` enum and returns a
  `&'static str`. It has no caller outside the panel.
- The two places the Heights tab WRITES a `HeightMode` are `commit_height_row`
  (gated on a real user edit) and the diagram's drag handler. Neither is in
  the diff.
- `draw_heights_params` gained a parameter and a label. It still writes only
  through `draw_height_row`, unchanged.
- The generation inputs signature is not touched, so nothing new stales a
  result.

## The sentry

`crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs`, crate
`rs_cam_viz`, test binary `bottom_z_pin_note_g_bottompin`. Four arms.

1. `every_operation_annotates_the_bottom_row_unless_it_honours_the_pin` —
   walks `OperationType::ALL` and asserts
   `bottom_z_pin_note(op).is_some() == !op.honors_pinned_bottom_z()` for all
   twenty-four. Non-vacuity floor: the walk must cover `ALL.len()`, `ALL` must
   still hold at least 24, and BOTH sides must be non-empty.
2. `the_note_names_the_dial_that_sets_the_floor` — every note is a sentence,
   contains "floor", and fits one panel line.
3. `only_the_three_pin_honouring_operations_keep_a_plain_bottom_row` — the
   three are named, so a fourth is a deliberate act.
4. `the_heights_tab_consults_the_note_weaker_source_level_arm` — a
   SOURCE-LEVEL read of `draw_heights_params`.

**Arm 1 is not a tautology.** `bottom_z_pin_note` carries its own exhaustive
`match`, one arm per family, because each family names a different dial. The
arm therefore compares two independent enumerations and fails when they
drift. It would catch the failure the brief warned about — a GUI that
hard-codes the three names.

**Arm 4 is the weaker instrument and its own doc comment says so.** It proves
the draw function mentions the decision function. It proves no pixel. No test
in this crate renders the properties panel; the one headless egui harness in
the repo (`crates/rs_cam_viz/src/controller/tests.rs::render_snapshot`, which
uses `Context::run_ui` plus `ui::automation`) is an in-crate test module and
reaching it would mean building a controller with the Heights tab selected. I
judged that out of reach without a compiler to iterate against, and I say so
rather than claim a rendered check.

**The red at the sentry commit is a COMPILE error, not an assertion.**
`bottom_z_pin_note` does not exist on the parent commit, so the test binary
does not build. That is a real red, and it is a weaker one than an assertion
failure: it demonstrates that the decision is missing, not that the operator
saw the wrong thing.

RED-FIRST OUTPUT: pending verifier run

## Not done, and why

- **The height diagram still draws a BZ line at a pinned bottom.**
  `draw_height_diagram` paints the label at `rect.right() - 42.0` in a 9 pt
  font. "BZ −7.0" fits that column; "BZ (unused) −7.0" would run off the
  canvas. I cannot see the rendering this round, so I left the diagram alone
  rather than ship an overlap. The note is drawn immediately above the
  diagram, which is the mitigation, not a fix. The diagram's drag handler
  also writes `HeightMode::Manual` on the BZ line for every operation,
  including the twenty-one that ignore it.
- **`operations/surface_3d.rs` needs no change.** Its only mention of these
  fields is the comment at line 340 in `draw_waterline_params`: "Z range now
  comes from the Heights tab (top_z / bottom_z)". `Waterline` is one of the
  three operations that DO read the pin, so the comment is true. I read it; I
  did not change it.
- **No generator change.** The core report recommends against making the pin
  drive 2.5D cuts and the operator's ruling is binding. I did not re-open it.
- **No `CLAUDE.md` edit.** The file carries no existing caveat paragraph on
  the Bottom Z field, and the brief allows only a sentence added to an
  existing paragraph.
