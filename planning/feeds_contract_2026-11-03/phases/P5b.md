# P5b: the envelope plot in the GUI and the toolpath lineage

Status: DONE 2026-11-04 | Contracts: K2b (view), K8 | Rulings: Q4, Q6, Q7, no-refusal (2026-11-03) | Numbers move: no

## Goal

After this phase, the inspector shows the P5 `SafeEnvelope` of the selected op: a small plot of
depth against width with the limit edges named, a dot for the operator's point and a ring for
the proposal (a bar for an op with one free value). A click picks a point; one ⚡ applies it.
Each generated toolpath carries a `Lineage` record. The inspector shows it in ONE closed
`Details`; MCP and the CLI return the same record.

## Preconditions

- P5 committed (`SafeEnvelope`, `EnvelopePick`, `ApplyFeedsArgs.pick`, MCP `get_safe_envelope`).
- P2 (K6 consequence column) and P3 (`EntryContract`, `EntryRecord`) committed: the lineage
  reads them.
- **The operator agrees the sketch below**, and answers D1 to D4 at the end.
- No file of this phase is in the working-tree diff at `40cf411a7`.

## Where Suggest shows today (all `@40cf411a7`)

| Surface | Where |
|---|---|
| Feeds tab host | `crates/rs_cam_viz/src/ui/properties/toolpath_panel/tabs/feeds.rs:12` `draw_feeds_tab` → `ui/properties/feeds_speeds.rs:133` → `ui/feeds/explore.rs:119` `draw_inline` |
| Nomogram (feed x rpm) | `ui/feeds/explore.rs:256` `draw_chart_c`; egui_plot, no navigation (W3k), `CHART_HEIGHT` `:102`, `MAX_LABELS` `:906`, `layout_labels` `:1153`; Recommended = `tokens::ACCENT` ring, Now = neutral dot (R23) |
| Card | `ui/feeds/compare.rs:187` `draw_recommendation`: problem rows, ONE `Details` with `⚡ Apply all` / `⚡ Speeds only` |
| Apply events | `crates/rs_cam_viz/src/controller/events/mod.rs:1134` (Both), `:1161` (Speeds), `:1257` (batch Both), `:1311` (Speeds + explored) |
| Geometry ⚡ pills | `ui/properties/pills.rs` `PillSuggestions` on the 22 `dv_pill` sites; reads `preview_field_applies` |
| Memo | `crates/rs_cam_viz/src/state/runtime.rs:420` `FeedsMemo`; filled by `ui/properties/feeds_speeds.rs:31` `refresh_feeds_memo` on every tab (F216) |
| Toolpath header | `ui/properties/toolpath_panel.rs:35` `draw_toolpath_panel`: status line (move count as meta), then `draw_issue_stack` |

Plot means: `egui_plot = "0.37"` (`crates/rs_cam_viz/Cargo.toml:24`), already used by
`ui/feeds/explore.rs:59`, `ui/feeds/shared.rs:29`, `ui/readiness_panel.rs:11`. No new
dependency. Copy the nomogram pattern: bounds set each frame, every zoom/drag/reset flag off,
labels on the lines through `layout_labels`.

## The sketch (to agree)

Place: the **Passes** tab, above the depth and stepover rows of the op editor. Reason: the
plot changes geometry; the Passes tab owns those rows; the inspector rule is "a diagram comes
before its rows" (`ui/properties/CLAUDE.md`). The Feeds tab keeps the nomogram (speeds). D1.

The mock uses letters, not numbers: the real ticks come from the envelope.

```
 SAFE CUT                                   ⚡        1  section header + apply chip
 ap │- - - - - - - - - - flute - - - - - - - -      ┐
    │━━━━━━━━━━━━━━━━━━━━━━━━┓ power                │
    │                        ┃╲                     │
    │          ●             ┃ ╲ defl.              │  plot, CHART_HEIGHT
    │                ◯       ┃  ╲                   │  (no text rows inside
    │                        ┃slot╲______           │   but the edge labels)
    └────────────────────────┴──────────────── ae   ┘
 ap a · ae b · rpm n · feed f · P p                    1  hover only (else empty)
```

- Text rows added to the tab: **1** (the header; the readout line shows only on hover, as the
  nomogram does). The plot holds at most `MAX_LABELS` edge labels.
- Edges: one line per `Limit`. The edge that binds the proposal is solid; the other edges are
  faint. The label is the short `LimitKind` name. No fill (G-CHARTLINES; D2).
- `●` the operator's point: neutral (R23 "Now"). Outside the envelope it takes
  `tokens::CAUTION`, and the binding edge label takes the same colour. No sentence (Q6: a
  warning, not a block; the K6 row of P5 carries the text to the issue stack).
- `◯` the proposal: `tokens::ACCENT` ring (R23 "Recommended").
- Hover: one line; ap, ae, rpm, feed and power from the `SpeedsMap` at the pointer. Outside the
  envelope the line names the binding limit and shows no speeds (it abstains, as
  `the_nomogram_readout_abstains_g_hoverbound` holds for the nomogram).
- Click inside: the ring moves to the click (a candidate; no write). The header `⚡` applies the
  ring: `ApplyFeeds { pick: Proposal }` when the ring is on the proposal, else
  `pick: Point { ap, ae }`. A click outside the envelope also moves the ring; the ring then
  takes `tokens::CAUTION` and the binding edge label takes the same colour. `⚡` applies it,
  and the op gets one warning that names the limit (ruling 2026-11-03: never a refusal). Esc
  puts the ring back.
- The `⚡` chip is disabled with a hover reason when the op has no envelope (an `Unbacked`
  cell: no numbers to apply; the job still generates). A
  `Partial` recommendation (P5 D4) draws the chip in `tokens::CAUTION`; the hover names the
  Unsourced rows.

One free value (Trace depth, Project Curve depth, Drill peck, scallop width):

```
 SAFE DEPTH                                 ⚡        1  section header + apply chip
 ├────────●──────────◯─────────┤ flute               1  bar, one row high
```

- Text rows added: **1** (plus the bar). The bar ends at the binding limit, named at its end.
- No plot and no section for an op with no free axis (`SurfaceFollow` with `Surface`).

Lineage (header, under the issue stack, only when the toolpath has a result):

```
 ▸ Lineage                                   7         1  closed Details, count
 ── open ──
 Depth / pass      a mm         ✎                       Cut (CutSpec)
 Stepover          b mm         ✎
 Pick              proposal     ◯ power                 K2b pick, binding limit
 Spindle           n rpm        ▣                       speeds: one glyph per basis
 Feed              f mm/min     ▲
 Plunge            p mm/min     □
 Flute length      L mm         ·                       tool fact (state from P6)
 Entry             ramp x12                             K4 contract + records
 Findings          2 warn                               K6 ids, hover lists them
```

- Closed: **1** row. Open: one `DataTable` row per fact, at most the rows above plus one per
  tool fact the op read. No row wraps; the source text is the hover (IC1, IC2).
- The basis glyph comes from `ProvKind` (`ui/components/provenance.rs:32`), the one provenance
  vocabulary. An Estimated or Unknown fact uses the neutral role, not `CAUTION` (Q7: no
  stress on estimated facts).

## Changes

1. **Core `Lineage` record.**
   - Where: new `crates/rs_cam_core/src/trace/lineage.rs`; built in
     `crates/rs_cam_core/src/session/compute.rs:1206` `finish`.
   - How: `pub struct Lineage { cut: CutSpec, pick: Option<PickRecord>, speeds: SpeedBases,
     tool_facts: Vec<FactUse>, entry: EntryLineage, findings: Vec<(DiagId, Consequence)>,
     rules: Vec<RuleOutcome> }`. It reads the persisted `FeedsProvenance` stamps and the
     P5 pick record; it does not run Suggest again (Q5). `FactUse` holds the bare value and
     `state: NotRecorded` until P6; `rules` stays empty until P7. No `Default`.
   - Placement: a required field `lineage: Arc<Lineage>` on `ToolpathComputeResult`
     (`session/mod.rs:1516`), not on `Produced` (D3).
   - Callers: `rg -c 'ToolpathComputeResult \{' crates --type rust` = 89 literal hits in 50
     files; `Produced` (if D3 picks it) has `#[derive(Default)]` and 42 hits in 24 files.
2. **The envelope in the memo.**
   - Where: `crates/rs_cam_viz/src/state/runtime.rs:420` `FeedsMemo`;
     `ui/properties/feeds_speeds.rs:31` `refresh_feeds_memo`.
   - How: `envelope: Option<Arc<SafeEnvelope>>`, filled with the preview, read by the plot. A
     frame never computes an envelope (F216).
   - Callers: `rg -c 'FeedsMemo' crates/rs_cam_viz --type rust` = 23 hits in 7 files.
3. **The plot.**
   - Where: new `crates/rs_cam_viz/src/ui/feeds/envelope.rs` (`draw_envelope`), called once
     from `ui/properties/toolpath_panel/tabs/passes.rs:17` `draw_passes_tab` before the op
     editor.
   - How: the sketch. Reuse `layout_labels` and the axis-gap rule of `explore.rs`; share them
     through `ui/feeds/shared.rs`, do not copy.
4. **Pick and apply events.**
   - Where: `crates/rs_cam_viz/src/ui/mod.rs:84` `AppEvent`; `controller/events/mod.rs:1134-1311`.
   - How: `AppEvent::ApplyFeedsPick { toolpath_id, pick }` → `Command::ApplyFeeds` with
     `pick`. `⚡ Apply all` sends `pick: Proposal`; `⚡ Speeds only` sends no pick. The
     candidate ring is GUI state in `state.panels` keyed by toolpath (not egui memory, ui09).
5. **Geometry ⚡ pills.**
   - Where: `ui/properties/pills.rs` `PillSuggestions`.
   - How: a pill offers the proposal's value for its field; its click sends
     `ApplyFeedsPick { pick: Proposal }` (no per-field apply, Checkpoint I-1).
6. **Lineage view.**
   - Where: `ui/properties/toolpath_panel.rs:35`, after `draw_issue_stack`.
   - How: ONE closed `Details` ("Lineage", with a count) of `DataTable` rows, as the mock.
7. **Lineage on MCP and the CLI.**
   - Where: `crates/rs_cam_mcp/src/server.rs`, `crates/rs_cam_viz/src/mcp_server.rs`,
     `crates/rs_cam_cli/src/project.rs`.
   - How: MCP tool `get_toolpath_lineage { index }` (the `Lineage` JSON; feature token
     `toolpath_lineage`); the CLI `summary.json` gains `lineage` per toolpath.

## Out of scope

- Sim points on the plot (K7, P9). Tool-fact states in the lineage (P6). K5 rule outcomes (P7).
- Any change to a number, a limit or the proposal rule (P5 owns them).
- The nomogram on the Feeds tab stays as it is.

## Tests

- New viz sentries (`crates/rs_cam_viz/tests/`):
  - `the_envelope_plot_marks_the_operator_and_the_proposal_k2b.rs`: rendered (egui harness);
    one dot at the op's point, one ACCENT ring at the proposal, one label per drawn edge.
  - `the_envelope_hover_abstains_outside_k2b.rs`: outside the boundary the readout names the
    limit and has no speeds.
  - `a_plot_click_writes_nothing_until_apply_k2b.rs`: a click moves the ring; the session is
    unchanged; `⚡` sends `ApplyFeeds` with the pick. A click outside the envelope applies too
    and gives one warning that names the limit (no refusal).
  - `a_one_axis_op_draws_a_bar_k2b.rs`: Trace and Drill draw the bar, Drop Cutter draws nothing.
  - `the_lineage_is_one_closed_details_k8.rs`: closed = one row; open rows match the core
    `Lineage` field by field.
- Core: `every_generated_toolpath_has_a_lineage_k8.rs` (walk `OperationType::ALL`; each result
  has a `Lineage` whose `cut` equals the op's `CutSpec`). `compile_fail` doctest: build a
  `ToolpathComputeResult` without `lineage`.
- Must stay green: `feeds_charts_draw_lines_not_shading_g_chartlines` (add
  `src/ui/feeds/envelope.rs` to its `CHART_FILES`), `the_inspector_draws_only_kit_rows_w6`,
  `the_issue_stack_is_capped_w6`, `the_inspector_text_is_bounded_ic1`,
  `tabular_data_reads_one_table_ic2`, `inspector_width_is_tab_independent_up4`,
  `the_panel_feeds_run_once_per_input_f216`, `the_feeds_memo_fills_on_every_tab_w3i`,
  `--lib every_painted_glyph_is_in_the_loaded_fonts` (new glyphs).
- MCP wire snapshot: `RS_CAM_UPDATE_WIRE_SNAPSHOT=1` on `mcp_wire_surface_pin` (one new tool).
- No golden moves: FM1 and the G-code goldens stay identical.

## Gate

All through `scripts/cargo_lane.sh`; viz with `-j 2` (the memory ceiling):

1. `scripts/cargo_lane.sh fmt --all -- --check`
2. `scripts/cargo_lane.sh test -p rs_cam_viz -q -j 2 --test <each sentry above>`
3. `scripts/cargo_lane.sh test -p rs_cam_viz -q -j 2` (focused crate tests; ask first if it
   runs over three minutes on this machine)
4. `scripts/cargo_lane.sh test -p rs_cam_core -q --test every_generated_toolpath_has_a_lineage_k8 --test feeds_matrix_instrument_fm1`
5. `scripts/cargo_lane.sh test -p rs_cam_mcp -q`; `scripts/cargo_lane.sh test -p rs_cam_cli -q`
6. full clippy (root `CLAUDE.md`)

## Docs to update

`crates/rs_cam_viz/src/ui/feeds/CLAUDE.md` (file `envelope.rs`, invariants, sentries);
`crates/rs_cam_viz/src/ui/properties/CLAUDE.md` (Passes tab plot, Lineage `Details`);
`crates/rs_cam_core/src/trace/CLAUDE.md` (`lineage.rs`); `crates/rs_cam_core/CONTRACTS.md` K8;
`crates/rs_cam_mcp/CLAUDE.md`; `FEATURE_CATALOG.md`; `planning/PROGRESS.md`.

## Traps

- G-CHARTLINES is an operator ruling (2026-09-23): no filled region in a feeds chart. A plot
  file outside `CHART_FILES` passes the scan by accident. Add the file to the list.
- The nomogram had a bounds bug from egui_plot plot memory (`explore.rs:33`). Set the bounds
  every frame; keep each plot id unique per toolpath.
- egui_plot paints the rotated axis title outside the plot (`AXIS_LABEL_GAP`, `explore.rs:111`);
  at the inspector edge it clipped. Reuse the indent.
- An input loses focus mid-typing (`ui/feeds/CLAUDE.md`). The ring state must not reset on a
  memo refresh.
- Before you trust a screenshot, check that the running `rs_cam_gui` was built from HEAD.
- Several agents share one working tree: no tree-wide git; read baselines with `git show`.

## Agent brief

> You implement P5b of `planning/feeds_contract_2026-11-03/` in `/home/ricky/personal_repos/rs_cam`.
> Precondition: P5 is committed and the operator agreed the sketch in `phases/P5b.md`. Read
> `PLAN.md` section 0, K2b and K8, then `phases/P5b.md` whole, then
> `crates/rs_cam_viz/CLAUDE.md`, `src/ui/feeds/CLAUDE.md`, `src/ui/properties/CLAUDE.md`,
> `src/ui/components/CLAUDE.md`, `crates/rs_cam_core/src/trace/CLAUDE.md`. Do Changes 1, 2, 3,
> 4, 5, 6, 7 in order. Add no text row beyond the mock's line count. Run every viz command
> through `scripts/cargo_lane.sh` with `-j 2`. Do not commit. Report each file you touched by
> path, the hit counts, and a screenshot path of the plot and of the open Lineage.

## Done when

- [ ] The operator agreed the sketch (date in this card) and D1-D4 are applied.
- [ ] Each Change is in, by path; the text rows added match the mock counts.
- [ ] New sentries green; the listed sentries still green; FM1 and goldens identical.
- [ ] Wire snapshot re-blessed; the token is in the commit message.
- [ ] Screenshots of the plane plot, the bar and the open Lineage checked against the mock.
- [ ] Clippy clean; docs updated.

## Decisions for the operator

- **D1. Place of the plot:** the Passes tab above the depth/stepover rows (recommended), or the
  Feeds tab under the nomogram.
- **D2. Shading:** G-CHARTLINES forbids fill in a feeds chart. (a) lines only, the binding edge
  solid (recommended, as the mock); (b) amend the ruling for this plot and give the sentry a
  named exception.
- **D3. Where `Lineage` lives:** `PLAN.md` K8 says `Produced`. `Produced` is the kernel's side
  channel (`trace/produced.rs:29`, derives `Default`, re-framed with the moves); the lineage is
  a session record. Recommended: a required field on `ToolpathComputeResult`.
- **D4. Glyphs:** map `Basis` onto the existing `ProvKind` kinds (Operator ✎, Printed ▣,
  Claim and Cited rule □, Unsourced rule as `RepoRule`, Derived ▲), or add `ProvKind` kinds
  for `Derived`, `Machine` and the P6 fact states.

## Citation check

Checked at `40cf411a7`.

| Cited | Correct at HEAD |
|---|---|
| B 2.2: "GUI card `compare.rs:110, 146, 866`" | correct as call sites (`:110` the preview, `:146` `suggest_for_operation`, `:860-867` the power fallback); the card entry is `draw_recommendation` `:187` |
| `ui/feeds/CLAUDE.md`: "W3i deleted Explore" | correct: `explore.rs` holds only the inline nomogram |
| `PLAN.md` K8: `Produced` "cannot be built without a `Lineage`" | `Produced` derives `Default` (`trace/produced.rs:28`); see D3 |
| `PLAN.md` K2b: the plot shades the safe region | conflicts with G-CHARTLINES (`crates/rs_cam_viz/tests/feeds_charts_draw_lines_not_shading_g_chartlines.rs:1-12`); see D2 |
| egui_plot version | `0.37`, `crates/rs_cam_viz/Cargo.toml:24` |


## Lead decisions (2026-11-03)

- D1: the Passes tab, as recommended. The operator sees it in the D2 sketch review (STATUS D2).
- D2: lines only. Ruling G-CHARTLINES (2026-09-23) stands; PLAN K2b is corrected. The binding
  edge is solid.
- D3: `Lineage` is a required field on `ToolpathComputeResult`, not on `Produced`. PLAN K8 is
  corrected.
- D4: map each `Basis` onto an existing `ProvKind`. Add a new kind only where no kind fits, and
  name it in the commit.

## Operator ruling on the sketch (2026-11-03)

The operator agreed to the sketch as drawn. D1: the Passes tab, above the depth and stepover
rows. D2-D4 as the lead decided: lines only (binding edge solid), `Lineage` a required field on
`ToolpathComputeResult`, `Basis` mapped onto the existing `ProvKind` kinds.

## Operator ruling: live link (2026-11-03)

- The ring can be dragged, not only clicked. The hover readout and the ring follow the pointer
  on every frame (a lookup in the P5 `SpeedsMap` grid; no new Suggest run per frame).
- The Feeds-tab nomogram follows the candidate ring: when a candidate exists for the toolpath,
  the nomogram draws the speeds of that cut (chip lines, power line, recommended point). No
  write until ⚡; Esc puts the ring back and the nomogram returns to the stored cut.
- No speeds strip under the envelope (no extra text row).
- Test: a GUI-state test that moves the candidate and asserts that the nomogram input reads the
  candidate's (ap, ae), and that the session is unchanged until the apply event.

## Operator ruling: one tab for the cut and its speeds (2026-11-03)

The Feeds tab merges into the Passes tab, so the operator sees each adjustment beside its cut.
`ToolpathTab::FeedsSpeeds` goes; the strip becomes Geometry, Passes, Heights, Linking
(`ui/properties/mod.rs` `ToolpathTab::ALL`; the test `WANT` list in `ui/properties/tests.rs`).
Order in the Passes tab, top to bottom (a diagram comes before its rows):

1. The envelope plot (or the one-value bar), header `SAFE CUT` with the ⚡ chip.
2. The feed × rpm nomogram (`ui/feeds/explore.rs` `draw_chart_c`), which follows the candidate
   ring (live-link ruling above).
3. The depth and stepover rows of the op editor.
4. The feed, rpm and plunge rows, then the Suggest card (`ui/feeds/compare.rs`
   `draw_recommendation`) with its one closed `Details`.

Lead notes for the implementer:
- Every link that opens the Feeds tab (an issue anchor `field_tab`, `placement.rs`; a ⚡ pill; MCP
  `set_ui_view`; F216 memo refresh on every tab) must open Passes. Measure with
  `rg -n "FeedsSpeeds" crates`.
- The two charts must fit the inspector without a scroll at the default window size; if they do
  not, the nomogram sits in a closed `Details` and opens when a candidate exists. Show the lead a
  screenshot before the commit.
- The no-text-clutter rule holds: the merge adds no sentence; it removes one tab.
