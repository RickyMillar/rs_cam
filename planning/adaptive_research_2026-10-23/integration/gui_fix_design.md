# GUI fix design: the G1 review items #1–#6 and #11, and the Z Blend removal

Date: 2026-10-28. Branch `geodesic-integration`, HEAD `de1696763`.
Author: I1 (implementer). Design only: this step changes no code.
Source: `planning/adaptive_research_2026-10-23/integration/gui_review.md`
(G1, main checkout). Line numbers are at `de1696763`.

The operator's constraint governs every item: fix each problem with no new
always-visible text. A text line shows only when the user must act, and it
is one short line. §7 counts the conditional messages and states one rule for every
message, new or old.

## 0. The panel today, and what goes to make room

### 0.1 How I measured

A temporary copy of the IC1 harness (`the_inspector_text_is_bounded_ic1`:
the real `properties::draw` in the app's 280-point panel, two frames, every
painted galley) rendered the 3D Rough Cut tab in four states. A refusal
went in as `ToolpathRun::Failed(text)`, the state a failed generate leaves.
The probe was deleted after the run. "Lines" is the harness's
`visible_lines`: the row bands of every galley, merged where they overlap.
The fixture's mesh is a flat test plate, so no Suggest warning and no
pathology fire; I add those by their measured size (one line each).

### 0.2 The always-visible lines

Body op, Geodesic Adaptive, boundary `BoundaryConfig::for_3d_op(6)`
(model silhouette, `Center`, offset 6), fresh stock, Advanced closed, not
generated. **23 lines**:

| Lines | Text | Source |
|---|---|---|
| 1 | `3D Rough` (the panel title) | `ui/properties/mod.rs` |
| 1 | `Name:` | `toolpath_panel.rs:85-89` |
| 1 | `Generate` · `Ready` | `:113` |
| 2 | the tab strip (it wraps at 280: `Feeds & Speeds` starts row 2) | `:255` |
| 3 | `Tool:`, `Input:`, `Rest machining: from previous ops` | `tab_badges.rs:669-763` |
| 2 | the diagram's labels `Spiral (Inside → Out)`, `step 2.00 mm` (about 120 points of picture) | `operations/registry.rs:405` |
| 5 | `Stepover:`, `Depth/Pass:`, `Stock to Leave:`, `Entry Style:`, `Strategy:` | `surface_3d.rs:64-189` |
| 1 | `Advanced` (closed) | `:190` |
| 6 | `Machining Boundary`, `Enable boundary`, `Boundary: model silhouette`, `Source:`, `Containment:`, `Offset:` | `toolpath_panel.rs:580-833` |
| 2 | the footer: `Gates: run simulation to evaluate`, `2 hints` | `:376-411` |

With Advanced open: five more (Tolerance, Min Cut Radius, Detect Flat,
Ordering, Z Blend). A 3D Rough draws no reach-map row
(`supports_reach_map` excludes roughing, `catalog.rs:664`).

The rest edge op (Contour Parallel, `Rest Regions` source, rest machining
on): **25 lines** (`Compare with fresh`, `Rest source:`).

### 0.3 What goes to make room

| Row | Change | Why |
|---|---|---|
| `Boundary: model silhouette` | removed from the Cut tab; its text becomes the hover of the `Source:` combo | It repeats the combo. On a rest edge op it reads "Boundary: rest regions of toolpath 0" (a raw id, measured) one row above "Rest source: Pocket". `boundary_summary_line` stays (the G-BOUNDARYINHERIT sentry (b) tests the function, not its placement); its module doc changes from "the tab prints" to "the Source hover prints". **D-1, ruled yes** (UX-R03-009 had put the line there). |
| `Z Blend` (Advanced) | removed with the field (§5) | the operator's ruling |
| The load-refusal toast | replaced by a modal (§4) | 8 s of text in a 400-pixel box |

### 0.4 The count after

| State (Cut tab, Advanced closed) | Before (measured) | After (by the design) |
|---|---|---|
| Body op, clean | 23 | 22 |
| Body op, Advanced open | 28 (23 + the five Advanced rows) | 26 |
| Rest edge op, clean | 25 | 24 |
| Rest edge op, worst state (§7.6): D53, the Suggest chip, a pathology line, a refusal from the last generate | 39 (38 measured + the pathology line) | 26 with G-E; 31 without G-E |
| Rest edge op, a generate refusal only (5–7 rows today) | 28–30 (25 + 3 to 5; measured deltas) | 25 with G-E (the `Refused` word and one face line) |

No fix adds an always-visible line. Every new signal is a chip, a grey
state, a colour, a hover, a word in an existing label, or a modal.

## 1. #1 Feed caps are invisible

**Form.** A line style in the 3D view and data in MCP. No panel text.

| Part | Where | When | Words |
|---|---|---|---|
| a. A capped move draws **dashed** in its own engagement colour | `render/toolpath_render.rs` `ToolpathLines::engagement` (`:570`): a move with `feed_cap.is_some()` goes through the existing `push_dashed_segment` (`:86`), not `push_segment` | Move colour: Engagement only | — |
| b. The Engagement legend gets one tail entry, no swatch | `ui/overlays/legend_rail.rs:1122`: `Key::Ramp { tail: [..] }` (the ramp already has a `tail`) | with the legend | `dashed: fed at a load cap` |
| c. The Generate row's figures hover names the count | `toolpath_panel.rs` `figures_hover` (`:1269`) | a result with capped moves | `N moves at a load cap (planned), lowest X mm/min` |
| d. `get_cut_trace` returns the caps | `app/mcp/simulation.rs` `build_cut_trace_response` (`:509`) | always when the trace has caps; omitted when none | keys below |
| e. The verdict comment | `rs_cam_core/src/tool_load/verdict.rs:80` | — | "Geodesic Adaptive writes it on tight turns and plunge departures (integration step 7.4a)." |

Why the dash: the emitter already writes the capped feed into the move
(`adaptive3d/path.rs:1883-1889`), so the Engagement view already draws a
capped turn as a slow move. The user cannot tell a cap from a modulation
slow-down. The dash names the cause; the colour still gives the feed.
`push_dashed_segment` dashes by fraction of each segment, so a run of
short turn segments reads as a dotted arc.

The figures hover reads the stored toolpath (`moves` with a cap), so it
says "planned" (viz contract: name the evidence).

`get_cut_trace` keys (the `BoundedResponse` pattern of `hotspots`):

- `capped_move_count` (`insert_always`): rows of `ct.capped_moves`, after
  the `toolpath_id` filter;
- `capped_min_feed_mm_min` (`insert_always`, `null` when 0 rows);
- `capped_moves` (`insert_capped`, cap `max_capped_moves`, default
  `rs_cam_mcp::response::DEFAULT_MAX_ISSUES`): `{toolpath_id, move_index,
  feed_cap_mm_min}` in move order.

The new request field `max_capped_moves` goes in the `get_cut_trace` param
type in `rs_cam_mcp`. The wire snapshot gains the field and the three keys.

Not done: a line in the simulation trace modal (rule 4 prefers the view
and MCP), and a per-move feed-source readout (none exists today).

Tests:

- a `render` unit test: a toolpath with one capped move gives dashed
  vertices for that move only;
- an MCP test: a trace with caps returns the three keys, and one with none
  omits `capped_moves`;
- the legend test reads the tail entry.

## 2. #2 D53 is known only after a generate

**Form.** Stop the bad choice: grey the two unsafe Containment options,
with the core sentence as their hover. A refusal line shows only in the
bad state.

| Part | Where | When | Words |
|---|---|---|---|
| a. Core read door | `session/compute/generation.rs`: split `refuse_larger_rest_consumer` (`:522`) into a pure read `larger_rest_consumer(index, source_id) -> Option<LargerRestConsumer { own, theirs, source_name }>`, which ignores the containment; the refusal calls it | — | the refusal text stays the same |
| b. Panel input | `ToolpathPanelInputs::rest_containment_refusal: Option<String>` (the inspector adds a field, not a parameter) | the source is `DerivedRestRegions` and the read returns `Some` | — |
| c. Grey `Center` and `Outside` | `toolpath_panel.rs:796-816`: `add_enabled_ui` + `on_disabled_hover_text`, the pattern of the Tool combo's refused tools (`tab_badges.rs:727-732`) | b is `Some` | hover: `This tool (Ø{2·own}) is larger than the tool of '{source}' (Ø{2·theirs}). It could cut {own−theirs} mm past that op's boundary. Use Inside.` |
| d. The bad state: Generate off, one header line | the validation ladder: `ToolpathValidationContext` gains `rest_consumer_refusals: Vec<(ToolpathId, String)>`, filled from a (as `hole_refusals`, `validate.rs:13-22`); the header draws it as the existing short `OneLine` with the full core sentence as `detail` (`toolpath_panel.rs:151-156`), and a small `Use Inside` button at the right of the same line (rule 3, §7.2; the fit, §7.3) that writes `Inside` to the draft | containment is `Center` or `Outside` and a is `Some` | `⛔ Containment must be Inside` · `[Use Inside]` |
| e. The pick (D-2, ruled yes) | the `Rest Regions` click (`toolpath_panel.rs:678-684`) | the pair is larger and the containment is `Center` or `Outside` | the click also writes `Inside`. The Containment combo shows `Inside` in the same frame, with the `ProvKind::AutoCorrect` badge `⟲` (the existing `ProvenanceBadge`) until the user edits the containment; badge hover: `Set to Inside: this tool (Ø10) is larger than the tool of 'Body rough' (Ø6).` |

Part e stops the bad state at its usual cause: a 3D op starts at
`Center` (`for_3d_op`). The lead ruled it yes, on the condition that it is
visible: the combo changes in the same frame and carries the `⟲` badge.
The badge state lives in the draft entry (a GUI flag, not a project
field), and it clears on the first containment edit. The blocker d stays
for the other path: a larger tool picked after the source.

The inputs read the committed session, so the grey state follows the
draft by one commit. The generate-time refusal stays as the hard check.

Tests:

- core: `larger_rest_consumer` is `Some` for a larger tool whatever the
  containment, and `None` for an equal tool;
- viz (egui harness): with a larger tool, `Center` and `Outside` are
  disabled, Generate is off at `Center`, the header holds one line, and
  `Use Inside` sets the draft to `Inside`;
- viz: the pick writes `Inside` and the combo shows `⟲` in the same frame.

## 3. #3 Suggest's "use Geodesic Adaptive" is hidden in a hover

**Form.** The existing ⚡ field chip (`SuggestButton::field`,
`ui/components/suggest.rs`), beside the Strategy combo. No text row.

| Part | Where | When | Words |
|---|---|---|---|
| a. The chip | `surface_3d.rs` Strategy row (`:158-189`), after the combo, in the same grid cell | `snapshot.suggest_warnings` holds `StrategyRecommendedNotApplied { param: "clearing_strategy", .. }` (Contour Parallel, roughing, mixed terrain: `feeds/suggest/adaptive_entry.rs:62-93`) | hover: `Suggest: Geodesic Adaptive for this terrain (mixed-terrain classifier). Click to switch.` |
| b. The click | writes `cfg.clearing_strategy = GeodesicAdaptive` in the draft; the draft commits as one `Command::ReplaceToolpathConfig` (`ui/properties/mod.rs:1005-1025`) | — | — |
| c. The context | `OpDrawCtx` gains `strategy_suggestion: Option<&'static str>` (the recommended label), built in `draw_cut_tab` from `snapshot.suggest_warnings` | — | — |

The chip colour is `ProvKind::RepoRule` ("a named repo rule with no
source"); the classifier is a heuristic. The chip goes when the strategy
is Geodesic Adaptive, because the warning then does not fire. The feeds
card keeps its hover entry. MCP `get_suggest_rationale` already has the
full entry.

Also (stale wording, §6): the `SuggestWarning::StrategyRecommendedNotApplied`
doc (`feeds/suggest.rs:450-453`) names the `ContourParallel → Adaptive`
promotion. It becomes `ContourParallel → GeodesicAdaptive`.

Tests:

- viz: with the warning, the Strategy row holds one ⚡ chip, and a click
  sets the draft to Geodesic Adaptive; without it, no chip;
- the panel line count does not change (`the_inspector_text_is_bounded_ic1`
  stays green).

## 4. #4 The retired-strategy load refusal is a toast

**Form.** One modal, with the same frame as the load-warnings modal
(`app.rs:1244-1294`: `egui::Modal`, `NoticeStack` in a scroll area, the
`Close` button). One list of hits, one fix, one Copy act. No toast.

| Part | Where | When | Words |
|---|---|---|---|
| a. Controller state | `controller` gains `load_refusal: Option<LoadRefusal { path, error: Box<SessionError> }>` | an interactive open fails with `VizError::ProjectLoad` | — |
| b. The two routes | `app/input.rs` `open_job_from_picked` (`:713-725`) and the second interactive route set a; `push_error` stays for every other `VizError` | — | — |
| c. The modal | `app.rs`, beside the load-warnings modal | a is `Some` | title `Project not opened` |
| d. The file | first row: the file name; hover: the full path | — | `wanaka_back.toml` |
| e. The hits | `SessionError::RetiredKeys(found)`: one `Notice` (`Role::Danger`) per `RetiredKeyFound::path`, hover = its `replacement`. `NoticeStack` bounds the list | — | `setups[0].toolpaths[2].operation.params.clearing_strategy = "agent_search"` |
| f. The fix | one line: the `replacement` when every hit shares it, else `Each line's hover names its fix.` | — | core text, unchanged |
| g. Copy | `Copy` (secondary) beside `Close`; `ctx.copy_text` of the file path, each hit and the fix, one per line | — | `Copy` |
| h. Any other load error | the same modal, the error text as one `Notice` | `ProjectLoad` that is not `RetiredKeys` | — |

`VizError::ProjectLoad` keeps the boxed `SessionError` (`error.rs:18`), so
the modal lists the structured hits; it does not parse text. The
`RS_CAM_JOB` start-up route keeps its log line (no window yet). MCP
`load_project` is unchanged.

Mock:

```
┌───────────────────────────────────────────────────────────────┐
│ Project not opened                                            │
│ wanaka_back.toml                                              │
│ ● setups[0].toolpaths[2].operation.params.clearing_strategy = │
│   "agent_search"                                              │
│ ● setups[0].toolpaths[4].operation.params.clearing_strategy = │
│   "contour_spiral"                                            │
│ Removed (removal step R2, operator ruling 2026-10-28): the 3D │
│ Adaptive clearing strategies are "geodesic_adaptive" and      │
│ "contour_parallel". Set clearing_strategy to one of them      │
│                                         [ Copy ]  [ Close ]   │
└───────────────────────────────────────────────────────────────┘
```

Tests:

- viz (egui harness): an open of a file with two retired values shows the
  modal with two hits and no toast; Copy puts the hits and the fix on the
  clipboard;
- viz: a load of a missing file shows the same modal with one row.

## 5. #5 Z Blend: retire it

### 5.1 Who reads it

`rg z_blend` (54 hits). The one reader is `clear_z_level_contour_parallel`
(`adaptive3d/clearing.rs:1378-1397`). The slice path (Geodesic Adaptive)
does not read it: the field reaches `ctx.z_blend` and stops there.

### 5.2 What it does, measured

With Z Blend on, each inner Contour Parallel ring descends from the level
toward `surface + stock_to_leave`. The clamp (`:1421`) stops it only at
the surface, not at the next level. So an inner ring can cut deeper than
one Depth/Pass.

Measured on the `contour_parallel_flat_blend` golden setup (hemisphere R10,
Ø4 flat, stepover 1.5, Depth/Pass 3, stock to leave 0.5, Detect Flat on),
with the tri-dexel simulator at a 0.25 mm cell. The probe was a temporary
test, deleted after the run:

| Run | Moves | Max axial engagement | Cutting samples deeper than one DPP | Cut time (sim) | Total (sim) |
|---|---|---|---|---|---|
| CP, Z Blend on | 1198 | 4.854 mm | 139 of 14059 | 217.4 s | 270.0 s |
| CP, Z Blend off | 1124 | 3.000 mm | 0 of 17848 | 268.5 s | 321.8 s |
| Geodesic, on | 4192 | 3.000 mm | 0 | 435.5 s | 556.8 s |
| Geodesic, off | identical move list | | | | |

So Z Blend breaks the one-Depth/Pass rule on Contour Parallel (up to 1.62×
the Depth/Pass on this fixture), and it does nothing on Geodesic Adaptive.

**The removal is also a safety fix.** With Z Blend on, Contour Parallel
cuts 4.854 mm deep at a 3 mm Depth/Pass: the tool takes a deeper axial
cut than the operator set and than the feeds were chosen for. After the
removal, no Contour Parallel level cuts deeper than one Depth/Pass
(3.000 mm on the probe).

### 5.3 The exact effect of the removal on a project with Z Blend on

- **Contour Parallel:** every ring cuts at its level, then the emitter's
  drape lifts it to the surface where the surface is above the level. This
  is the output of the same op with `z_blend = false`, byte for byte. Each
  level holds one Depth/Pass. On the fixture above the cycle is 19 % longer
  (270.0 s → 321.8 s), because the material that the blend removed early
  is now cut by the next level.
- **Geodesic Adaptive:** no change.

### 5.4 The retirement

| Surface | Change |
|---|---|
| Config | `Adaptive3dConfig::z_blend` (`operation_configs.rs:1693`) deleted. The schema (ParamEnum / `OpParams` derive) drops it with the field. |
| Engine | `Adaptive3dParams::z_blend` (`adaptive3d/mod.rs:374`), the clearing context field (`clearing.rs:372`), the pass-through (`path.rs:1056`, `rough_3d.rs:160`) and the blend code (`clearing.rs:1374-1397`, the lerp at `:1418-1421`) deleted. The ring Z is `target_z.max(z_level)`, the no-blend value. |
| GUI | the `Z Blend` row (`surface_3d.rs:221-223`) deleted. |
| MCP | `set_toolpath_param z_blend` refuses with the retired text (d below). `get_operation_schema` no longer lists it. The wire snapshot changes. |
| CLI | the `z_blend = true` line of the sweep template (`rs_cam_cli/src/sweep.rs:611`) deleted. |
| Load rule | see below |

**The load rule (recommendation, Decision D-3).** Every 3D Rough that an
older build saved holds `z_blend = false` (the field has no
`skip_serializing`), so a refusal of the key would refuse every such
project. Serde ignores an unknown key at load (`fine_stepdown` in
`tests/fixtures/test_job.toml` loads today).

- `z_blend = false`: loads; the key is ignored. The output does not
  change.
- `z_blend = true`: **refuses**, through a `RETIRED_VALUES` row (option B,
  the precedent of the strategy removal). Fix text: `Removed (GUI fix,
  operator ruling 2026-10-28): Z Blend cut deeper than one Depth/Pass.
  Delete the key; the op then cuts one Depth/Pass per level`. The matcher
  compares `as_str` today (`project_file.rs:395`); it must compare the
  TOML value, so that a boolean row matches.
- The parameter setter refuses `z_blend` with the same text
  (`retired_param_value` reads the same rows).

The alternative is to ignore `true` too: a Contour Parallel op then
changes its output on load with no word. That is against the option B
ruling, so I do not recommend it.

Files that set `true` today: `tests/fixtures/test_job.toml:739` (a
Geodesic op, so no output change; the line is deleted) and the CLI
template. The planning fixtures (`rivmap100*`, `wanaka*`) set `false`.

### 5.5 The goldens and tests

- `adaptive3d_emission_byte_parity`: the case `contour_parallel_flat_blend`
  becomes `contour_parallel_flat` (Detect Flat only). Its fixture is
  re-blessed alone: the new dump equals the Z-Blend-off output (the
  committed dump has 1198 moves; the probe's Z-Blend-off run has 1124). The bless log names the cause and the D-3
  ruling. The other four cases do not move.
- `every_op_keeps_its_toolpath_g_opgolden`: no row sets Z Blend (`rg`), so
  no change is expected; the run confirms it.
- `param_sweep.rs` `sweep_adaptive3d_z_blend` (ignored instrument):
  deleted, with a one-line note as R4 did.
- `end_to_end.rs` `test_adaptive3d_rapids_lift_before_xy_traverse`: it
  guards the lift before an XY rapid, on a Z Blend CP run. It keeps the
  guard on the plain CP run. Its non-vacuity assert (at least one XY
  rapid) must still hold; if it does not, report to the lead.
- New sentry `a_z_blend_true_refuses_at_load` (core): `true` refuses with
  the fix text; `false` loads; MCP `set_toolpath_param z_blend` refuses.
- The adaptive3d folder map and `FEATURE_CATALOG.md` drop Z Blend; the
  catalog states the load rule.

## 6. #6 No "regions left" figure on a Geodesic source

**Form.** Words in two labels that exist today. No new row.

| Part | Where | When | Words |
|---|---|---|---|
| a. The consumer's source combo | `toolpath_panel.rs:715-745`: the item text `"{name} (regions ready)"` | the source result has regions | `{name} · {N} regions` |
| b. Its hover | the combo hover (`:748-753`), first line | the source has `rest_regions_top_z` | `Top of the regions: Z {z:.2}.` then the hover of §8 |
| c. The source's own disclosure title | `toolpath_panel.rs:854-858` `Rest regions → {consumers}` | a consumer exists and the result has regions | `Rest regions → {consumers} · {N} left` |

`BoundaryRestCandidate` already carries the regions (`ui/properties/mod.rs:606-630`),
so N is `regions.len()`. The candidate tuple gains `rest_regions_top_z`
from `produced`. A source with no regions keeps the plain name; the
`(regions ready)` tag goes.

Tests: viz, a consumer with a generated source reads `· N regions` in the
combo; the source's disclosure title reads `· N left`.

## 7. Conditional messages: notes, warnings, refusals

The operator: a conditional note, warning or error often takes a whole
paragraph and a large part of the panel. This section counts them, states
one rule for all of them, applies it to the new items, and proposes the
same treatment for the existing ones (optional step G-E).

### 7.1 The inventory

Measured with the temporary IC1 harness copy of §0.1 (280-point panel,
`ToolpathRun::Failed(text)` for a refusal), or at the 264-point inner
width for a text the fixture cannot reach.

Every note, warning, refusal and pathology line that the 3D Rough panel
can show, with its longest wording, in lines at 280 points. "Measured" is
the probe; "IC1" means the IC1 sentry holds it to one row (only text in
the danger colour may wrap).

| Message | Where | Longest wording | Lines | Form today |
|---|---|---|---|---|
| **Generate refusal `Error: …`** | header, beside Generate (`toolpath_panel.rs:1441-1447`) | D53 larger-tool refusal (`generation.rs:556`) | **15** (measured; the tab strip moves down 257 points) | wrapped danger label, full text |
| Generate refusal: source produced no regions | same | `generation.rs:625` | **7** (measured) | same |
| Generate refusal: the op names itself as source | same | `generation.rs:601` | **7** (measured) | same |
| Generate refusal: source not generated | same | `generation.rs:617` | **5** (measured) | same |
| Generate refusal: memory budget | same | `budget/mod.rs:177` | **3** (measured) | same |
| Validation error (Generate off) | header (`:163`) | "Selected model must provide both 2D geometry and a 3D mesh (use Surface selector for separate mesh)" | 3 (measured at 264 points) | wrapped danger label |
| Selection refusal | header | `Refused · see Geometry` | 1 | `OneLine` + hover |
| No tool | Cut tab (`tab_badges.rs:741`) | "No tool. Set a tool on the Cut tab" | 1 | danger label |
| BREP not loaded | Cut tab (`:769`) | `⚠ BREP topology not loaded` | 1 | `OneLine` + hover |
| Blocked / edited since / waiting / disabled | header state word | `Done · edited since — regenerate` | 1 | word + hover |
| Selection boundary note | Cut tab (`toolpath_panel.rs:692`) | "Pick the boundary items in the Geometry tab, under Boundary." | 1 | `OneLine` (truncates) |
| Rest pathology (consumer and own) | Cut tab (`:779`, `:978`) | `⚠ One rest region covers 87% of the part` | 1 each | `OneLine` + hover |
| `Compare with fresh` result | Cut tab (`tab_badges.rs:816-834`) | `rest 412.0 s · fresh 380.1 s` | 1 | button + figures |
| Footer findings | footer (`:376-395`) | any diagnostic | 1 **each**, no bound on the count | one line + hover (IC1) |
| Hints | footer | `N hints` | 1 (closed) | collapsed |
| Rest disclosure | Cut tab (`:854`) | title + 4 rows when open | 1 closed | disclosure |
| Feeds unavailable | Feeds tab (`feeds_speeds.rs:71`) | `Feeds unavailable: {e}` | not measured; wraps with `e` | danger label |

The worst offenders, in order:

1. **The generate refusal in the header.** It prints the core sentence in
   full, in the narrow column beside Generate: 3 to 15 lines, above the
   tab strip, so the tabs move down by up to 257 points.
2. **The footer findings.** Each is one line, but nothing bounds the
   count; a rough with five findings adds five lines.
3. **The header validation error.** Up to 3 lines.

Everything else already holds one line with the detail on hover.

### 7.2 The rule for every message

1. A message takes **one line at most** in the panel: an icon or a role
   colour, and a short label (`OneLine`, which truncates; or a
   `StatusChip`).
2. The explanation goes behind a hover (`OneLine::detail`) or one
   expander.
3. Where a fix exists, the fix is an action (a button or an icon button),
   not a sentence that names it.
4. A list of messages shows at most three lines, then `+N more` (the
   bounded `NoticeStack`, `ui/components/notice.rs`).
5. Above the tab strip: the title, `Name:`, the Generate row and at most
   one refusal line. The tabs never move.

### 7.3 The fit, measured

`OneLine` paints at `tokens::SIZE_CAPTION`. Widths measured with the
app's fonts (a temporary probe, deleted); the inner panel width is about
264 points:

| Text | Width |
|---|---|
| `Refused · Containment must be Inside` | 197.5 |
| `⛔ Refused · tool larger than source's` | 192.6 |
| `⛔ Refused · 'Body rough' left no regions` | 210.8 |
| `⛔ Refused · generate 'Body rough' first` | 206.2 |
| `⚠ One rest region covers 87% of the part` (today) | 216.9 |
| button `Use Inside` (small, with padding) | 62.6 |
| button `Generate 'Body rough'` | 125.6 |
| item spacing | 4.0 |

So `Refused · Containment must be Inside` fits alone, but with the
`Use Inside` button it needs 264.1 points: no margin. A face plus a text
button that names the source does not fit. The design therefore:

- drops `Refused ·` where the icon `⛔` already says it;
- uses a text button only when it is short (`Use Inside`), and an icon
  button with a hover for an action that names a toolpath (`⟳`, hover
  `Generate 'Body rough'`);
- relies on `OneLine` truncation as the last guard: a long source name
  cuts the face, never wraps it.

The implementer's test proves each face with its action on one row at 280
points (the IC1 `rows == 1` rule and the `excess_width` check).

### 7.4 The rule applied to the new items

| Item | Face (one line) | Hover or expander | Action |
|---|---|---|---|
| §1 feed caps | none in the panel; dashed moves in the view; one legend entry | figures hover: `N moves at a load cap (planned), lowest X mm/min` | — |
| §2 D53, the option | `Center` and `Outside` grey in the combo | the sizes, the overrun, `Use Inside` | pick `Inside` |
| §2 D53, the blocker | `⛔ Containment must be Inside` + `[Use Inside]` (estimate: about 160 + 66 points, from the widths above) | the core sentence | `Use Inside` writes the draft |
| §2 D-2, the pick | the Containment combo shows `Inside` at once, with the `ProvKind::AutoCorrect` badge `⟲` beside it until the user edits the containment | `Set to Inside: this tool (Ø10) is larger than the tool of 'Body rough' (Ø6).` | — |
| §3 Suggest | `⚡` on the Strategy row | `Suggest: Geodesic Adaptive for this terrain …` | click switches |
| §4 load refusal | a modal, not the panel | each hit's fix on its hover | `Copy`, `Close` |
| §6 regions | `· 14 regions` in the combo item; `· 14 left` in the disclosure title | `Top of the regions: Z 12.30.` | — |

### 7.5 Two refusals at once

A rest op can hold a refusal from its last generate (say "no regions")
and, after an edit, the D53 blocker. Both are true, but only the blocker
answers the settings on screen. So the panel shows one line:

- the blocker line (`⛔ Containment must be Inside` + `[Use Inside]`);
- the last run's refusal becomes the state word `Refused` in the Generate
  row, with its text on hover (G-E). Without G-E, it stays the wrapped
  `Error: …` label beside Generate.

### 7.6 The worst state, before and after

A rest edge op: Contour Parallel on mixed terrain (the Suggest chip), a
`Rest Regions` source with a pathology, rest machining on, a Ø10 tool
against the Ø6 tool of the source at `Center` (D53), and a refusal from
its last generate ("no regions"). The figures are illustrative; the line
numbers are `visible_lines` counts, from the measured layout.

Before: **39 lines** with the D53 refusal as the last generate refusal
(38 measured + the pathology line). With "no regions" as the last refusal
it is 31 (7-line error); today the D53 state shows only after a generate,
so the 39-line form is the worst one.

```
 1  3D Rough
 2  Name: [Edges                              ]
 3  [Generate] Error: 'Edges' takes its boundary
 4             from the rest regions of 'Body
 5             rough' with Center containment,
 6             but its tool radius 5.000 mm is
 7             larger than the tool radius of
 8             'Body rough', 3.000 mm. The
 9             regions are tool-centre
10             positions of that tool, so this
11             tool could cut up to 2.000 mm
12             past the machining boundary of
13             'Body rough'. Set the boundary
14             containment to Inside, or use a
15             tool no larger than the tool of
16             'Body rough'.
    (15 rows, measured; the tab strip moves down 257 points)
17  Geometry | Cut | Heights
18  ● Feeds & Speeds | Linking | Dressup
19  Tool:   [End Mill Ø10.00 mm ▾]
20  Input:  [wanaka200 ▾]
21  [x] Rest machining: from previous ops
22  [Compare with fresh]
23  Spiral (Inside → Out)
       ( diagram )
24                       step 2.00 mm
25  Stepover:        [2.0 mm] ⚡
26  Depth/Pass:      [3.0 mm] ⚡
27  Stock to Leave:  [0.50 mm]
28  Entry Style:     [Plunge ▾]
29  Strategy:        [Contour Parallel ▾]
        (the Suggest advice is not on this tab; it is a hover on the
         Feeds card's Advance row)
30  ▸ Advanced
31  Machining Boundary
32  [x] Enable boundary
33  Boundary: rest regions of toolpath 7
34  Source:      [Rest Regions ▾]
35  Rest source: [Body rough (regions ready) ▾]
36  ⚠ One rest region covers 87% of the part
37  Containment: [Center ▾]
38  Offset:      [0.0 mm]
39  Gates: run simulation to evaluate
    ▸ 2 hints
```

After, with G-A to G-D and G-E: **26 lines**.

```
 1  3D Rough
 2  Name: [Edges                              ]
 3  [Generate] Refused          (Generate off; "Refused" hover: the last
                                 run's text, "'Body rough' produced no
                                 rest regions …")
 4  ⛔ Containment must be Inside           [Use Inside]
        (hover: the core D53 sentence)
 5  Geometry | Cut | Heights
 6  ● Feeds & Speeds | Linking | Dressup
 7  Tool:   [End Mill Ø10.00 mm ▾]
 8  Input:  [wanaka200 ▾]
 9  [x] Rest machining: from previous ops
10  [Compare with fresh]
11  Spiral (Inside → Out)
       ( diagram )
12                       step 2.00 mm
13  Stepover:        [2.0 mm] ⚡
14  Depth/Pass:      [3.0 mm] ⚡
15  Stock to Leave:  [0.50 mm]
16  Entry Style:     [Plunge ▾]
17  Strategy:        [Contour Parallel ▾] ⚡
        (⚡ hover: "Suggest: Geodesic Adaptive for this terrain …"; click switches)
18  ▸ Advanced
19  Machining Boundary
20  [x] Enable boundary
21  Source:      [Rest Regions ▾]   (hover: "Boundary: rest regions of 'Body rough'")
22  Rest source: [Body rough · 14 regions ▾]   (hover: "Top of the regions: Z 12.30. …")
23  ⚠ One rest region covers 87% of the part   (hover: the fix)
24  Containment: [Center ▾]   (Center and Outside grey; hover: sizes, overrun)
25  Offset:      [0.0 mm]
26  Gates: run simulation to evaluate
    ▸ 2 hints
```

After, with G-A to G-D only (no G-E): **31 lines**: line 3 is the 7-row
wrapped `Error: 'Body rough' produced no rest regions …` beside Generate
(it adds 5 lines, the measured delta), and line 4 is the D53 blocker line.

With D-2 (ruled yes), the usual path to D53 is gone: the `Rest Regions`
pick writes `Inside` and shows `⟲`. The blocker shows only when the user
picks the larger tool after the source, or sets `Center` by hand.

### 7.7 Step G-E: the existing messages, optional

What it changes, all in this panel, with no change to any toolpath:

| Message | Today | After | Size (estimate) |
|---|---|---|---|
| Generate refusal (`FreshnessState::Error`, `toolpath_panel.rs:1441-1447`) | 3–15 wrapped lines beside Generate | the state word `Refused` (danger) in the Generate row, the full text on its hover; plus one `OneLine` face below the row with its action, unless a validation refusal already holds that line (§7.5) | viz: the `Error` arm, about 25 lines |
| The three rest-boundary refusals (`generation.rs:593-629`) | 5–7 lines | short faces with an action: not generated → `⛔ Generate '{source}' first` + the icon button `⟳` (hover `Generate '{source}'`, `AppEvent::GenerateToolpath(source)`); no regions → `⛔ '{source}' left no regions` + `⟳`; own regions → `Refused · source is this op` (MCP only: the GUI list leaves the op out) | core: a typed `RestBoundaryRefusal` with today's text as its `Display` (the wire and MCP text do not change), about 40 lines; viz: the face and action map, about 30 lines |
| Any other refusal | wrapped | the same `OneLine`, face = the text, truncated | in the first row |
| Header validation errors (`:163`) | up to 3 lines | `OneLine` + `detail` | about 10 lines |
| Footer findings (`:376-395`) | one line each, no bound | the first three, then `+N more` that opens the rest (the bounded `NoticeStack` rule) | about 40 lines |
| `Feeds unavailable: {e}` (`feeds_speeds.rs:71`) | wraps | `OneLine` + `detail` | about 5 lines |
| The IC1 sentry | danger text may wrap | in the toolpath panel no text wraps; above the tab strip: the title, `Name:`, the Generate row and at most one refusal line | about 30 lines of test |

Total: roughly 180 lines of code and test, one core type with no wire
change, and one sentry rule made stricter. The measured gain: a refusal
state drops from 5–15 lines to one, and the tabs stay in place.

Outside this panel, the same rule would reach the Geometry tab's selection
refusals (`selection_section.rs:154, 1354`, wrapped danger labels). They
belong to the 2D ops and are not in G-E.

One more always-visible observation, not proposed here: at 280 points the
tab strip takes two lines (`Feeds & Speeds` wraps), on every op.

## 8. #11 Stale wording

| Where | Today | After |
|---|---|---|
| `mcp_server.rs:1902` (`set_machine_kinematics`) | "the parallel-vs-spiral verdict from `recommend_clearing_strategy`" | "the strategy ranking from `recommend_clearing_strategy`" |
| `rs_cam_mcp/src/server.rs:1680` (doc, in the wire snapshot) | "the parallel-vs-spiral strategy verdict" | "the strategy advisor's ranking" |
| `mcp_server.rs:1361` (`set_boundary_config`), the `derived_rest_regions` clause | "whose REST ANALYSIS supplies the regions" | "whose regions supply the boundary: its rest analysis, or the pieces a Geodesic Adaptive rough left" |
| `mcp_server.rs:1375` (`set_rest_analysis_config`) | "It attaches a heatmap grid and the machining regions …" | add: "On a Geodesic Adaptive rough the regions are the pieces it left." |
| `rs_cam_mcp/src/server.rs:1723-1725` (`source_toolpath_id`) | "whose pencil rest-depth result supplies the boundary" | "whose rest regions supply the boundary (its rest analysis, or the pieces a Geodesic Adaptive rough left)" |
| `toolpath_panel.rs:668-676` (`Rest Regions` hover) | "Boundary = rest regions computed by another toolpath's rest analysis. Pick the source toolpath below." | "The regions another toolpath leaves: the pieces a Geodesic Adaptive rough did not cut, or another op's rest analysis. Pick the source below." |
| `toolpath_panel.rs:748-753` (`Rest source` hover) | 4 lines on the rest analysis | "The toolpath whose regions bound this one. Picking it switches its rest analysis on." (after the top-Z line of §6 b) |
| `feeds/suggest.rs:450-453` | "`ContourParallel` → `Adaptive` promotion" | "`ContourParallel` → `GeodesicAdaptive` promotion" |
| `tool_load/verdict.rs:80` | "No shipped planner writes a cap" | §1 e |
| `mcp_authoring_surface.rs` test doc (R6 fixed it) | — | — |

The two MCP description edits move the wire snapshot rows; that is a
documentation change, as in R6.

## 9. The clean panel, before and after

The mocks follow the measured layout at 280 points (§0.1). The figures in
them (moves, sizes, counts, Z) are illustrative. The number at the left is
the visible line count, as `visible_lines` counts it.

### 9.1 The clean body op

Geodesic Adaptive, Cut tab, Advanced closed. Before: 23 lines. After: 22.
Only line 15 changes:

```
 1  3D Rough
 2  Name: [Body rough                         ]
 3  [Generate]  Done                  1 842 moves  Feed: modulated
        (figures hover gains: "412 moves at a load cap (planned), lowest 1 200 mm/min")
 4  Geometry | Cut | Heights
 5  ● Feeds & Speeds | Linking | Dressup
 6  Tool:   [End Mill Ø6.00 mm ▾]
 7  Input:  [wanaka200 ▾]
 8  [ ] Rest machining: from previous ops
 9  Spiral (Inside → Out)
       ( diagram )
10                       step 2.00 mm
11  Stepover:        [2.0 mm] ⚡
12  Depth/Pass:      [5.46 mm] ⚡
13  Stock to Leave:  [0.50 mm]
14  Entry Style:     [Plunge ▾]
15  Strategy:        [Geodesic Adaptive ▾]
16  ▸ Advanced                     (open: Tolerance, Min Cut Radius,
                                    Detect Flat, Ordering; Z Blend gone)
17  Machining Boundary
18  [x] Enable boundary
    Boundary: model silhouette     ← removed (D-1); now the Source hover
19  Source:      [Model Silhouette ▾]
20  Containment: [Center ▾]
21  Offset:      [6.0 mm]
22  Gates: run simulation to evaluate
    ▸ 2 hints
```

## 10. Order, gates and rulings

Implementation order, each a frozen step with its own report:

1. G-A: §5 Z Blend (core, CLI, GUI row, MCP, load rule, golden).
2. G-B: §1 feed caps (render, legend, figures hover, MCP, comment).
3. G-C: §2 D53 and §6 regions (core read door, panel inputs, combo).
4. G-D: §3 chip, §4 modal, §8 wording, §0.3 summary line.
5. G-E (optional, Decision D-4): the rule of §7.2 on the existing
   messages of this panel. See §7.7 for its size.

Gates per step: fmt, full clippy, the folder sentries of each touched
folder (`ui/properties`, `ui/components`, `render`, `ui/overlays`,
`adaptive3d`, `session`), `mcp_wire_surface_pin`, the byte parity and op
golden (G-A re-blesses one case only), and the viz sentries
`the_inspector_text_is_bounded_ic1`, `tabular_data_reads_one_table_ic2`,
`the_help_key_is_the_param_name_ui04`, `boundary_controls_always_visible_g_boundaryinherit`.
Every write goes through the existing command path:
`Command::ReplaceToolpathConfig` (the chip, the pick), the core loader
(the modal reads its error), the param setter (the MCP refusal).

Rulings (the lead, 2026-10-28):

- **D-1 yes**: `Boundary: …` moves from a row to the `Source:` hover.
- **D-2 yes**: the `Rest Regions` pick writes `Inside` when the tool is
  larger, and the combo shows it at once with the `⟲` badge (§2 e).
- **D-3 yes**: `z_blend = false` loads silently; `true` refuses with the
  fix.

Open:

- **D-4**: do step G-E (the existing long messages), or stop after G-D.
