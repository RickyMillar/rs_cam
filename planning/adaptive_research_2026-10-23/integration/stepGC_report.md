# Step G-C: D53 is known before a generate; the region count

Implementer: I4. Date: 2026-10-28. Base: `66aecd7a1`. Design:
`gui_fix_design.md` §2 and §6, with the review changes and the lead's
notes:

- the D53 blocker is the `Refused · see Cut` header line, a dot on Cut, and
  `[Use Inside]` on the Containment row;
- the ⟲ flag lives in `state.panels`, scoped to the toolpath;
- one phrase for the count: `· N regions`;
- `[Use Inside]` and the D-2 pick check `boundary_collapse` first, and the
  check is memoised.

Patch: `stepGC_tree_vs_66aecd7a1.patch` (14 files: 4 core, 9 viz, the
catalog). It holds only my hunks.

## Core (`session/compute/generation.rs`; the lead allowed the edit)

- New `pub struct LargerRestConsumer { op_name, source_toolpath_id,
  source_name, tool_radius_mm, source_tool_radius_mm }`.
  - `overrun_mm()` gives the difference of the radii.
  - `refusal(containment)` gives the D53 sentence, word for word as before.
  - It is re-exported as `rs_cam_core::session::LargerRestConsumer`.
- New read door `ProjectSession::larger_rest_consumer(index, source)`. It
  ignores the containment: it compares the radii of the built cutters
  only. A tool or a source that does not resolve gives `None`.
- New `ProjectSession::larger_rest_consumer_refusal(index)`: the D53 text
  for the stored boundary and containment, or `None`.
- `refuse_larger_rest_consumer` calls the same door. The generation refusal
  does not change (sentry below).
- New `ProjectSession::boundary_collapse_with(index, &BoundaryConfig)`.
  This is `boundary_collapse` with a candidate boundary: it resolves the
  inputs from the stored config, puts the candidate boundary in
  `ResolvedGenInputs::boundary_config`, and runs the same collapse test.
  It answers "would Inside collapse?" before anything writes `Inside`.

## Viz

| Part | Where | What |
|---|---|---|
| Inputs | `properties/mod.rs` | The candidate tuple gains `rest_regions_top_z`. `RestContainment { larger, inside: InsideAnswer }` and `rest_containment_inputs` give `ToolpathPanelInputs::rest_containment` (the stored `DerivedRestRegions` source) and `rest_pick` (the first candidate, which the `Rest Regions` pick writes, D-2). |
| Memo | `state/runtime.rs` | `InsideMemo` on `ToolpathRuntime`. Its key is the consumer's `toolpath_revision_at`, the source id, and a `Weak` of the source's result. The answer is `Keeps`, `Collapses(sentence)` or `Unknown(error)`. The input resolution runs once per key, not once per frame. |
| Grey options | `toolpath_panel.rs` Containment row | When the draft source is the refused source, `Center` and `Outside` are disabled buttons in the combo. Their disabled hover gives the sizes, the overrun and the fix. Example: `This tool (Ø10.00) is larger than the tool of 'Body rough' (Ø6.00). It could cut 2.00 mm past that op's boundary. Use Inside.`. When `Inside` collapses, the fix is `use a tool no larger than Ø6.00`. |
| `[Use Inside]` | the same row, at the right (right to left) | It shows while the draft containment is not `Inside`. It is enabled only when the answer is `Keeps`; else it is off, with the hover. A click writes `Inside` to the draft. The draft commits as one `ReplaceToolpathConfig`. |
| Blocker | `operations/validate.rs`, header | `ToolpathValidationContext::rest_consumer_refusals` comes from the core text. `validate_toolpath` adds it when the draft source is `DerivedRestRegions` and the draft containment is not `Inside`, so Generate is off. The header face is `Refused · see Cut` (`REFUSED_SEE_CUT`), with the core sentence on hover, in the one G-E4 line (a Geometry refusal goes first). A danger dot goes on the Cut tab (`TabBadges::raise` is `pub(super)` now). |
| D-2 | `pick_sets_inside` | The `Rest Regions` click writes the source. It then writes `Inside` when `rest_pick` names that source, the answer is `Keeps`, and the containment is not `Inside`. It sets `state.panels.containment_autocorrect` with the hover `Set to Inside: this tool (Ø10.00) is larger than the tool of 'Body rough' (Ø6.00).`. |
| ⟲ badge | `state/panels.rs` `ContainmentAutoCorrect` | The compact `ProvenanceBadge(ProvKind::AutoCorrect)` goes beside the combo, with the hover. The slot is scoped to the toolpath (`scope`, as `HeightsRefusal`), threaded through `draw_toolpath_panel` and `draw_cut_tab`. It clears on the first containment change. |
| Regions | `rest_source_label`, `regions_phrase` | Source combo items and the selected text read `{name} · {N} regions` (`1 region`); `(regions ready)` goes. The combo hover starts `Top of the regions: Z {z:.2}.` when the source has a top Z. The source's own disclosure title reads `Rest regions → {consumers} · {N} regions`. |

Two functions now take 8 arguments (`draw_toolpath_panel`,
`draw_cut_tab`). Each has an
`#[expect(clippy::too_many_arguments, reason = ..)]`, as other GUI threaded
slots do.

## Panel line count

| State (rest edge op, Cut tab) | Before G-C | After G-C |
|---|---|---|
| Clean | unchanged (labels only) | unchanged |
| D53, larger tool at `Center` | known only after a generate (the G-E1 face line) | `Refused · see Cut` header line (one line), `[Use Inside]` on the existing Containment row (no line), dot on Cut |
| D-2 pick | `Center` stays, D53 after a generate | `Inside` at once, `⟲` on the existing row (no line) |

No always-visible line is added. The bad state has the one header line, as
the design's worst state (§7.6).

## Tests

- Core `--test a_larger_rest_consumer_tool_refuses_d53`
  `the_read_doors_answer_as_generation_does_g_c`:
  - `larger_rest_consumer` is `Some` under all three containments, with
    the radii and the overrun, and `None` for an equal tool.
  - `larger_rest_consumer_refusal` equals the `generate_toolpath` error
    text, and is `None` under `Inside`.
  - `boundary_collapse_with(Inside)` is `Some` on the narrow channel and
    `None` on the wide one.
  - The existing D53 test stays green.
- Viz `--lib the_rest_pick_sets_inside_with_a_badge_d2`:
  - `Keeps` writes `Inside` and the badge hover.
  - `Collapses`, `Unknown` and another source write nothing, and `None`
    writes nothing.
  - A clear drops the badge, and a new toolpath scope drops it.
- Viz `--lib the_containment_hover_and_the_region_count_g_c`: the two
  hovers, `· 14 regions`, `· 1 region`, and the plain name.
- Viz `--lib use_inside_sits_on_the_containment_row_g_c`:
  - The setup: the real `draw_cut_tab`, with a larger consumer at
    `Center`.
  - `Use Inside` paints, and a click writes `Inside`.
  - When `Inside` collapses, it paints and a click writes nothing.

Not covered by a render test: the `Refused · see Cut` header line with a
real D53 session. That needs the D53 mesh fixture, which takes 30 s to
generate in core. The header face selection is the G-E4 code path with one
more partition; the core text is pinned by the core test.

## Gates (green)

- `cargo test -p rs_cam_core --test a_larger_rest_consumer_tool_refuses_d53`:
  2 passed.
- `cargo test -p rs_cam_viz --lib`: 605 passed, 2 ignored.
- `cargo test -p rs_cam_viz --test` on 11 sentries. All passed.
  - `the_inspector_text_is_bounded_ic1`, `tabular_data_reads_one_table_ic2`
  - `the_help_key_is_the_param_name_ui04`,
    `boundary_controls_always_visible_g_boundaryinherit`
  - `inspector_header_wraps_g_reachwrap` (its G-E4 anchor follows the new
    `if !owned.is_empty()` order), `the_inspector_nests_once_dc5`
  - `the_geometry_rows_fit_the_inspector_f22`,
    `inspector_width_is_tab_independent_up4`
  - `component_contracts_up2`, `panels_read_the_token_module_up1`,
    `ui_string_hygiene`
- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_viz --lib --tests -- -D warnings`:
  clean.
- `cargo fmt -p rs_cam_viz -p rs_cam_core -- --check`: clean.

## A note on formatting

I formatted `session/mod.rs` with `rustfmt`. rustfmt also formats the
child modules, and I5's uncommitted `session/compute/params.rs` and
`session/project_file.rs` were in the tree at that time. Any change it
made there is whitespace only. I5 should check their diff.

## Line counts by state (added at the lead's request)

These counts are derived, not measured. The IC1 fixture has no rest edge
op. The base is the design's measure (`gui_fix_design.md` §0.2: rest edge
op clean, Cut tab, 25 lines), less the D-1 row (G-D), so 24. GT changes
the tab strip only when the strip fits one line; that depends on the tab
dots of the op, so I do not count it here.

| State (rest edge op, Cut tab, Advanced closed) | Before G-C | After G-C |
|---|---|---|
| Clean | 24 | 24 (`· N regions` replaces `(regions ready)` in the same combo) |
| Larger tool at `Center`, no generate yet | 24: nothing shows; D53 waits for a generate | 25: the `Refused · see Cut` header line; `[Use Inside]` sits on the existing Containment row; the options are grey; there is a dot on Cut |
| The same after a generate | 25: the `Refused` word and the G-E1 face line | 25: the header line takes the one refusal line (§7.5), and the `Refused` word stays in the Generate row |
| The D-2 pick with a larger tool | 24, then 25 after a generate (D53) | 24: `Inside` with `⟲` on the existing row; no refusal |
| Worst state (§7.6: D53, the Suggest chip, a pathology line, a refusal from the last generate) | 26 (24 + the face line + the pathology line) | 26 (24 + the header line + the pathology line). This is the design's 26-line target. |
