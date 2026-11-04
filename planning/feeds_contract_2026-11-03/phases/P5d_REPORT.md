# P5d report (p5d-impl, 2026-11-04)

Base: HEAD 6b7d81e3c. No commit. No tree-wide git command was run.

## Paths changed

Owned:
- `crates/rs_cam_viz/src/ui/feeds/envelope.rs`
- `crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `crates/rs_cam_viz/src/ui/feeds/explore_tests.rs`
- `crates/rs_cam_viz/src/ui/feeds/CLAUDE.md` (40 lines, 5073 bytes; it was 5078)
- `crates/rs_cam_viz/tests/every_limit_is_drawn_and_named_p5d.rs` (new sentry)
- `crates/rs_cam_viz/tests/the_envelope_plot_shows_only_what_limits_the_cut_p5c.rs`
- `crates/rs_cam_viz/tests/the_envelope_plot_marks_the_operator_and_the_proposal_k2b.rs`
- `planning/feeds_contract_2026-11-03/phases/P5d.md` (the Mock paragraph only: the counts table)

Not owned, touched with a reason:
- `crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`. The test finds the
  suggested line by its colour. D3 makes that line quiet ACCENT, so the `suggested_colour`
  helper now returns `envelope::recommendation_color()`. No assertion changed.
- `crates/rs_cam_viz/tests/a_one_axis_op_draws_a_bar_k2b.rs`. D4 changes the names. The bar end
  check now compares with `limit_name(&env, kind)`, not `short_label(kind)`. It is still an
  exact equality.

Not changed: `shared.rs`, `mod.rs`, `tests/envelope_k2b/`, and the hoverbound test. Each one
still passes.

## RED, then GREEN

RED (the sentry first, at HEAD): the sentry does not compile. A compile error prints no
`test result:` line. The real output is:
```
error[E0432]: unresolved imports `rs_cam_viz::ui::feeds::envelope::RECOMMENDATION_WIDTH`, `rs_cam_viz::ui::feeds::envelope::limit_name`, `rs_cam_viz::ui::feeds::envelope::recommendation_color`
error[E0603]: constant `GHOST_RADIUS` is private
error[E0603]: constant `VIEW_HEADROOM` is private
error[E0609]: no field `top` on type `EnvelopeScene`
error[E0599]: no method named `drawn_edges` found for struct `EnvelopeScene` in the current scope
error: could not compile `rs_cam_viz` (test "every_limit_is_drawn_and_named_p5d") due to 7 previous errors
```
After the API compiled, the first behavioural run was also red: `test result: FAILED. 0 passed;
3 failed`. Slot DOC had no label place, the D tick did not paint, and the review-point fixture
had its proposal at Now.

GREEN:
```
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.02s
```

## What was built (D1–D5)

- D1: `EnvelopeScene::of` keeps every non-frame limit whose edge enters the view. `NEAR_SHARE`,
  `view_distance`, `segment_distance` and the Runtime special case are deleted.
  `EnvelopeScene::drawn_edges(plot size)` drops an edge whose points all lie within
  `MARK_CLEARANCE` px of one side of the frame. On rivmap it drops Runtime at ae 0.043. The hover
  still names that edge. A per-point limit has no edge. An edge above the top is out of view.
- D2: `view_ranges` = `min(hard top, VIEW_HEADROOM x max(flat ApCeiling/ApCeilingAbove of the
  non-frame limits under the hard top, Now, proposal))`. The candidate does not change it. The
  sentry derives 11.7 from the envelope (hard top 20, tallest 9). `scene.top` names CuttingLength
  or FluteGuard when the range reaches it. The frame top then carries "Cut length" or "Flute guard".
  `AXIS_MARGIN` is deleted.
- D3, envelope: the proposal diamond is drawn when the proposal is not at the setpoint. It uses
  `recommendation_color()` = `wash(ACCENT, BAND_LIMIT_ALPHA)` and a 1 pt outline
  (`RECOMMENDATION_WIDTH` = `BAND_LIMIT_WIDTH`). The marks are now painted after the plot, so the
  stroke width is exact (egui_plot strokes an outline marker at radius/5).
- D3, nomogram: the white dot is at `candidate_speeds` during a draft, else at the stored speeds,
  with the label "Now". A hollow `TEXT_MUTED` ghost (envelope `GHOST_RADIUS`, `OUTLINE_WIDTH`)
  marks the stored speeds only during a draft. The Recommended ring stays at
  `explain.recommended`: quiet ACCENT, 1 pt. The suggested chip line stays at the recommendation:
  quiet ACCENT, 1 pt. The painted "Recommended" label is gone. The hover adds a "Recommended" line
  when the pointer is on the ring. `candidate_advance_mm` and `SUGGESTED_LINE_WIDTH` are deleted.
  The "Before derate" target no longer hides during a draft. It belongs to the recommendation, and
  the recommendation no longer moves.
- D4: `short_label` uses the new base names. `limit_name(env, kind)` adds the k×D share for
  VendorAxial. k is trimmed to at most 2 decimals; rivmap gives "Vendor DOC 1×D". The label, the
  bar end, the readout and the ⚡ hover use `limit_name`. The binding line is `TEXT_BODY` 2 pt,
  a crossed line is `CAUTION` 2 pt, and every other line is `TEXT_MUTED` 1.5 pt
  (`MUTED_LINE_WIDTH`, the card's number). Each label takes its line's colour. The line hover
  shows the name, the law, the mm and the basis.
- D5: the D edge, and the top when it is a hard stop, paint as a 1 pt `TEXT_FAINT` line. The
  drawn bounds go `BAR_END_ROOM` pt past that edge (`edge_pad`), and the clamp stays at D. Padding
  alone did not paint "6": 6 mm is on the 0.25 mm minor step, and egui_plot names a mark by its
  step size. `with_edge_mark` therefore gives the edge mark the large step. The nomogram RPM axis
  now reads `nice_steps`. At 240 it names "0" and "20000".
- A step edge (the slot: two segments) puts its label beside either segment
  (`LabelAnchor::Lines`). Its chord ran through the plot, so "Slot DOC" had no place at 340.
- `ChartLabel.text` and `PlacedLabel.text` are now `String`, because the vendor label is computed.

Not built as written: none. One item to check: D5 says "Now" and "Max RPM" must not touch at 240.
The new arm passes with a gap of at least 1 pt. I did not measure the gap before the change, and
I made no layout change for it.

## Old assertions changed, with reasons

| File | Assertion | Change | Reason |
|---|---|---|---|
| p5c | `a_drag_moves...`: "still no proposal mark" after the drag | One quiet diamond, at the proposal, `recommendation_color` | D3: the diamond shows when the proposal is not at the setpoint, and the draft moved the setpoint off it |
| p5c | 1b: diamond == `ACCENT` | == `recommendation_color()` | D3: quiet ACCENT |
| p5c | `Marks::no_blue_circle` | Also rejects `recommendation_color` | Stronger: no circle is blue in either shade |
| p5c | `the_rivmap_rough_plot_draws_only_the_limits_near_the_cut_p5c` (≤2 lines; review point = Vendor only; near-slot cut adds Slot; all lines TEXT_BODY) | Renamed `..._draws_every_limit_in_view_p5c`: exactly Vendor, Rigidity, Slot at both points; binding TEXT_BODY, others TEXT_MUTED; one label per drawn line; no DANGER | D1 reverses the "near a mark" rule; D4 colours |
| k2b marks | All edges TEXT_BODY (inside); non-crossed edges TEXT_BODY (outside) | Binding TEXT_BODY, others TEXT_MUTED; crossed CAUTION | D4 |
| k2b marks | Labels == `short_label` of `scene.edges` | == `drawn_edges` labels plus `top_label()`; non-empty | D1 (a frame-hugging edge has no label) and D2 (a hard top is named) |
| k2b marks | Diamond == `ACCENT` | == `recommendation_color()` | D3 |
| explore_tests | `MUST_PAINT_AT_340` held `LABEL_RECOMMENDED` | Removed | D3: the ring paints no label; the hover names it |
| corridor | Suggested line colour `CHART_SERIES[1]` | `recommendation_color()` | D3: the chip line is quiet ACCENT |
| bar k2b | Bar end == `short_label(kind)` | == `limit_name(&env, kind)` | D4 names |

## Counts (rivmap100 3D Rough at rest, the inspector)

Before is HEAD 6b7d81e3c, measured with a temporary probe that is now deleted. After is from
the sentry `--nocapture` output. A line is an open painted path inside the chart band.

| Chart | Width | Lines before | Labels before | Lines after | Labels after |
|---|---|---|---|---|---|
| Envelope | 340 | 1 | 1 (Vendor max DOC) | 4 (3 limits + D edge) | 3 (Vendor DOC 1×D, Rigidity DOC, Slot DOC) |
| Envelope | 240 | 1 | 1 | 4 | 3 |
| Nomogram | 340 | 5 | 5 (Max feed, Max RPM, Rubbing, Min RPM, Now) | 5 | 5 |
| Nomogram | 240 | 5 | 3 (Max feed, Max RPM, Now) | 5 | 3 |

RPM ticks at 240: before "0", after "0", "20000". At 340: "0", "10000", "20000", before and after.

## Gates

- New sentry plus every sentry in `ui/feeds/CLAUDE.md` (19 targets): all `test result: ok`.
  The `--lib _w3k` filter gives `test result: ok. 5 passed; 0 failed; ... 692 filtered out`.
  The `_w3i` lib tests passed in the full run.
- `cargo test -p rs_cam_viz -q --no-fail-fast -j 2`: `error: 11 targets failed`. Every target is
  on the STATUS.md Known red list: fm5 (3), wp19 (2), f140 (1, `ui/components/glyph_tests.rs`
  not on a map), genalldisabled, wp12, mcp_authoring_surface, r2, ui09 (2), fscontrol, ic1, p4_3.
  `apply_contract_a3` and `mcp_wire_surface_pin` from the list now pass.
  **New failure inside a known-red target:** `the_inspector_text_is_bounded_ic1` reads
  "3D Rough / Passes: 35 visible text rows, cap 34". It also reads "Tool: 18 visible text rows,
  cap 17". The cause, from the inventory: D2 grows the depth axis from 7.8 to 11.7 mm, so the axis
  paints one more tick row ("10"). The three limit labels share rows with existing ticks. I did
  not run ic1 at HEAD, so this cause is inferred from the inventory. I did not raise the cap. That
  decision belongs to the lead (the W6 rule: the cap is the measure).
- Clippy (full command, `-D warnings`): `Finished \`dev\` profile`, exit 0. The new sentry needed
  `#![expect(clippy::print_stderr, ..)]` for the printed counts.
- `rustfmt --edition 2024 --check` on the 8 touched `.rs` files: clean.

## Round 2: the frame edge label (lead review, 2026-11-04)

The defect: when D is off the large step, `with_edge_mark` kept the large mark beside the edge.
The two labels then overlapped ("5" beside the edge). `tick_text` also wrote the edge with one
decimal, so 6.35 mm painted as "6.3".

The fix in `envelope.rs`:
- `with_edge_mark(marks, edge, per_pt, room)` removes every named mark nearer the edge than
  `room` points. A mark is named when its step is wider than `room`. A fine mark keeps its grid
  line, because it has no label.
- For the width, `room` = `X_LABEL_SPACING` lower bound (45 pt). For a hard depth top, `room` =
  `Y_LABEL_SPACING` lower bound (20 pt).
- `Y_LABEL_SPACING` (20..=30) and `GRID_SPACING` (8..=300) are egui_plot 0.37's own defaults,
  cited by file. The plot now sets both explicitly, so the code reads the values in force. No
  visible change comes from that.
- `per_pt` = `GridInput::base_step_size / GRID_SPACING.start()`. egui_plot computes
  `base_step_size` as `dvalue_dpos x grid_spacing.min` (`plot.rs`).
- `tick_text(v, edge)` writes the frame edge with up to two decimals (`trim_decimals`). The depth
  axis is now `custom_y_axes`, with the same title, formatter and min width.

The new sentry arm is `an_edge_off_the_step_names_itself_alone_p5d`. Its fixture is the
testpiece_mini "1/4 in flat end mill" (D read from the fixture: 6.35) on its "Back cavity Z9.00"
Pocket. The arm runs at 360 and 240 pt. It asserts that the D tick paints as `trimmed(D)` and
that no two width tick labels overlap. It also asserts that D is off every half-mm step, so the
fixture still exercises the rule.

RED (the arm alone, before the fix):
```
at 360: the `6.35` tick paints at D: [("0", ..), ("2.5", ..), ("5", ..), ("6.3", ..)]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.09s
```
GREEN (the whole sentry):
```
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.70s
```

Gates after round 2:
- Every sentry in `ui/feeds/CLAUDE.md` plus the P5d sentry: all `test result: ok`.
- `--lib _w3`: `test result: ok. 22 passed; 0 failed; ... 675 filtered out`.
- `the_inspector_text_is_bounded_ic1` (with the lead's cap of 35): the only red arm is the known
  "Tool: 18 visible text rows, cap 17". 3D Rough / Passes is now within its cap.
- Clippy (full command, `-D warnings`): exit 0, `Finished \`dev\` profile`.
- `rustfmt --check` on `envelope.rs` and the P5d sentry: clean.

I did not add a separate test arm for the hard depth top: no fixture at hand reaches it. It uses
the same function with the y room.
