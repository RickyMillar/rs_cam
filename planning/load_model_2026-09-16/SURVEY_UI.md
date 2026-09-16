# SURVEY_UI — what the GUI shows about cutting limits today

Scope: `crates/rs_cam_viz` only. Read-only survey, 2026-09-17.

The survey reports what exists. It proposes no design.

## How to read a citation

Every claim names a **symbol** and a **path:line**. The symbol is the
durable half. Another session is moving modules in this tree, so search for
the symbol first and treat the line as a convenience.

A citation reads: `draw_speed_controls`
(`crates/rs_cam_viz/src/ui/properties/mod.rs:2251` at time of survey).

Paths below drop the `crates/rs_cam_viz/` prefix unless the file sits in
another crate.

### Churn seen during this survey

I checked the modification time of all 30 files I cite. None moved and none
changed while I worked. The newest write under `src/ui/` is
`09:19:28`; my reading ran from about `10:00` to `10:09`. So this survey
describes ONE consistent snapshot.

I corrected three of my own citations during verification. The corrected
values are the ones below:

| Symbol | Wrong line | Correct line |
|---|---|---|
| `headroom_phrase` | 544 | `src/ui/feeds/compare.rs:577` |
| `build_per_depth_pass_summary` | 4917 (doc comment) | `src/app/mcp.rs:4921` |
| `SIGNAL_MAX_POINTS` | 667 | `src/ui/sim_timeline.rs:672` |

One attribution was also wrong. The sentence
`Chipload: run simulation to evaluate` is authored in CORE, by
`from_tool_load` (`crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs:837`),
not in the GUI. The GUI only tiers and paints it.

---

## 1. Every surface that shows a limit or a load metric

Stage key:

- **P** — predicted. The surface draws before any simulation runs.
- **S** — measured. The surface draws only after a simulation run.

| # | Surface | Symbol (path:line at time of survey) | Stage | What it renders |
|---|---|---|---|---|
| 1 | Inspector → Feeds & Speeds → Recommendation card | `draw_inspector_comparison` (`src/ui/feeds/compare.rs:149`) → `draw_comparison_card` (`:259`) | P | Six `current → recommended` text rows built by `rail_row` (`src/ui/feeds/compare.rs:384`) and `woc_row` (`:805`): RPM, Feed, Plunge, DOC, WOC, Advance/tooth. No bar, no chart, no limit. |
| 2 | The chip verdict row (inside surface 1) | `rail_efficiency_row` (`src/ui/feeds/compare.rs:459`) | P | ONE row: the label `Chip`, the advance per tooth, and a coloured verdict phrase from `verdict_face` (`:515`). The phrases are `thin`, `in vendor range`, `heavy`, `no vendor chipload range matched`. The floor, the ceiling and the force headroom live on the hover built by `efficiency_hover` (`:593`). The refusal hover is `unmodelled_hover` (`:675`). |
| 3 | MRR row (inside surface 1) | `rail_mrr_row` (`src/ui/feeds/compare.rs:699`) | P | One number, `mm³/min`. No limit. |
| 4 | Feeds Explore window — the nomogram | `window::draw` (`src/ui/feeds/window.rs:69`) → `draw_modal_body` (`src/ui/feeds/explore.rs:124`) → `draw_chart_c` (`:365`) | P | A feed-versus-RPM chart, plot id `feeds_modal_nomogram` (`src/ui/feeds/explore.rs:421`). It draws the vendor band as three iso-chipload diagonals, the machine envelope walls, and the chipload corridor. |
| 5 | The chipload corridor (inside surface 4) | `Corridor` (`src/ui/feeds/explore.rs:290`), `Ceiling` (`:298`), `CORRIDOR_ALPHA` (`:56`) | P | Two washed wedges: the rubbing floor and the deflection ceiling. `Ceiling` has three variants — `NotModelled`, `OffScale`, `OnChart`. `OffScale` draws NOTHING and the `Sources` row names the number instead. |
| 6 | The headline above the nomogram | `draw_headline` (`src/ui/feeds/explore.rs:204`) | P | One line: `<fz> mm/tooth · N % under vendor`. Its hover names six derate reasons, including the string `spindle power` (`:243`). |
| 7 | Inspector → Feeds & Speeds → Operating point card | `draw_operating_point` (`src/ui/properties/mod.rs:2433`) → `draw_advance_per_tooth_card` (`:2546`) | S | Four text rows: commanded advance per tooth, achieved advance per tooth, the vendor band, and a gate verdict word from `advance_gate_verdict_text` (`:2641`). |
| 8 | Inspector diagnostics ribbon | `collect_diagnostics` (`src/ui/properties/operations/mod.rs:2575`), called at `src/ui/properties/mod.rs:4230`; tiers split at `:4261` | P and S | Sentence rows. Before a simulation the load gates land in the neutral `Stateful` tier. `merge_stateful_gate_rows` (`src/ui/properties/mod.rs:3280`) folds the three identical gate sentences into one row. |
| 9 | Simulation bottom panel — verdict HUD | `draw_verdict_hud` (`src/ui/sim_timeline.rs:113`) | S | Count pills over TOOLPATHS, not over limits: `✓ load`, `✗ exceeds`, `⚠ unmodeled`, plus collisions, hotspots and traces. The exceeds pill seeks the playhead on click. |
| 10 | Simulation bottom panel — summary signal track | `summary_fn` (`src/ui/sim_timeline.rs:546`) and `summary_env` (`:561`), drawn by `draw_signal_track` (`:675`) | S | A time series of the achieved advance per tooth divided by that toolpath's vendor band ceiling, so `1.0` reads `at the limit`. This is the ONLY normalised-to-limit chart in the product. |
| 11 | Simulation bottom panel — five raw tracks | the `tracks: [SignalTrack; 5]` table (`src/ui/sim_timeline.rs:454`), type `SignalTrack` (`:20`) | S | `arc-mean chip thickness`, `arc engagement`, `axial DOC`, `MRR`, `feed`. All five pass `None` for the envelope ON PURPOSE — the comment at `:480` states that a shaded band IS a comparison. They sit behind a `Signal graphs (5)` disclosure (`:600`). |
| 12 | Simulation left panel — per-operation flags | `toolpath_status_flags` (`src/ui/sim_op_list.rs:1023`), `criterion_short_label` (`:1168`), `criterion_detail` (`:1179`) | S | One glyph plus a short label per failing criterion: `advance/t`, `power`, `defl`, `weld`, `peck`, `plunge`. The hover adds ` peak <value> <unit>` and a confidence word. A criterion that is `Within` and `Validated` raises no flag at all. |
| 13 | Simulation Inspector — `Now playing` badges | `draw_toolpath_section` (`src/ui/sim_diagnostics.rs:910`) → `draw_tool_load_badges` (`:1020`) → `verdict_badge` (`:1151`) | S | THREE badges on one row: `advance/tooth NN%`, `power NN%`, `L/D NN%`. `pct_of_cap` (`:1142`) divides the criterion peak by a cap. This is the only place in the product that prints more than one limit as a percent of its own limit. The section starts CLOSED. |
| 14 | Simulation Inspector — selected span metrics | `draw_span_body` (`src/ui/sim_diagnostics.rs:1493`), grid id `selected_metrics_grid` (`:1554`) | S | Text rows of averages and peaks: samples, engagement avg/peak %, achieved advance per tooth avg/peak, arc-mean chip thickness avg/peak, `Axial DOC` peak (`:1604`), MRR avg/peak. NO limit is named on any row. |
| 15 | Simulation Inspector — drill gate badges | `drill_gate_badge` (`src/ui/sim_diagnostics.rs:1091`) | S | `chip weld`, `peck`, `plunge`, each `OK` / `elevated` / `critical`. It REPLACES the three badges of surface 13 for a drill operation — `draw_tool_load_badges` returns early. |
| 16 | Readiness workspace — Tool load check row | the `"Tool load"` `check_row` (`src/ui/readiness_panel.rs:196`) and the three `CountPill::verdict` calls (`:201`, `:207`, `:214`) | S | One check row plus three pills counting TOOLPATHS: `within`, `exceeds`, `unmodeled`. No per-limit split, no number about any one limit. The tier comes from `tool_load_check` (`src/ui/readiness.rs:343`). |
| 17 | Readiness workspace — project feeds rollup | `draw_project_rollup` (`src/ui/readiness_panel.rs:457`), `draw_bottleneck_callout` (`:656`), `project_feeds_status` (`:312`) | P | A table of every toolpath's current-versus-recommended speeds, plus a bottleneck sentence. It is a SPEED rollup, not a limit rollup. |
| 18 | Readiness workspace — feed/RPM scatter | `draw_project_scatter` (`src/ui/readiness_panel.rs:740`), plot id `feeds_modal_project_scatter` (`:763`) | P | Every toolpath as a current point, a recommended point and a joining line, over the shared machine envelope. Behind a checkbox. |
| 19 | Export pre-flight gate | `tool_load_summary_detail` (`src/ui/preflight.rs:362`), `draw_tool_load_overrides` (`:444`), `format_verdict_line` (`:535`) | S | Words only. One line per exceeded toolpath, naming the gate and the reason, for example `power: EXCEEDS (SpindlePowerExceeded)`. Two acceptance checkboxes. |
| 20 | Optimize modal — verdict badges | `draw_verdict_badges` (`src/ui/optimize_modal.rs:926`) → `verdict_badge_state` (`:934`) | S | Three glyph badges: `advance/tooth`, `power`, `L/D`. Glyph and colour only. No number. |
| 21 | Viewport — tool deflection overlay | `draw_sim_deflection_overlay` (`src/app/viewport.rs:798`), `deflection_ui_color` (`:898`), `DEFLECTION_OVERLAY_EXAGGERATION` (`:896`) | S | A bent cutter drawn at 200× exaggeration, labelled `Tool deflection δ=NN µm`, coloured in three bands. Registered as overlay id `tool_deflection` (`src/ui/overlays/registry.rs:1173`). |
| 22 | Viewport — toolpath colour by advance per tooth | overlay id `move_colour_advance_per_tooth` (`src/ui/overlays/registry.rs:1091`), colour from `advance_per_tooth_segment_color` (`src/render/toolpath_render.rs:755`) | S | Each cutting move takes a colour from its advance per tooth against the vendor band. |
| 23 | Status bar — load warnings | `status_bar::draw`, called from `App::draw_setup_layout` and its three siblings (`src/app.rs:346`) | P and S | A count of controller load warnings. It is a project-open concern, not a cutting limit. |

### The machine envelope, shared by surfaces 4 and 18

`draw_machine_envelope` (`src/ui/feeds/shared.rs:223`) paints the forbidden
zones past the spindle cap and the feed cap, plus a caution zone below the
spindle minimum. Those are MACHINE limits. They are the only limits in the
product that already draw as a region rather than as a word.

---

## 2. What each of the five limits shows today

### Chip thickness — the floor

Predicted: one number and one verdict WORD — `rail_efficiency_row`
(`src/ui/feeds/compare.rs:459`) and `verdict_face` (`:515`). The floor
itself is a hover sentence inside `efficiency_hover` (`:593`):

```
"Rubbing floor: {:.3} mm/tooth — below it the edge burnishes instead \
 of cutting.\n",
```

The nomogram draws the floor as a wedge — `Corridor`
(`src/ui/feeds/explore.rs:290`).

Measured: a percent badge `advance/tooth NN%` from `verdict_badge`
(`src/ui/sim_diagnostics.rs:1151`), a normalised time series from
`summary_fn` (`src/ui/sim_timeline.rs:546`), and a peak in a flag hover
from `criterion_detail` (`src/ui/sim_op_list.rs:1179`).

The floor case is already special-cased downstream. When the chipload
verdict is `Exceeds` on the LOW side, the `chipload_bound` binding inside
`draw_tool_load_badges` (`src/ui/sim_diagnostics.rs:1052`) swaps the cap
for the band FLOOR, and `verdict_badge` prints `BURN` instead of a percent.

### Spindle power

Predicted: NOTHING numeric. The only predicted mention is the string
`spindle power` in the derate reason list inside `draw_headline`
(`src/ui/feeds/explore.rs:243`).

A predicted power gauge EXISTED and was deleted. The sentry file
`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs` records the
measurement that condemned it, in its module doc (`:9`):

```
//! power branch never fires, and **peak** utilisation across the whole
//! shipped matrix is 23.6 %. Typical is 1 %. A readout whose maximum
```

The test `no_power_gauge_returns_to_the_card_g_chipverdict` (`:393`) bans
four identifiers — `ProgressBar`, `power_bar`, `power_color`,
`rail_power_row` — from the source it reads (`:406`), and asserts that no
painted text on the Feeds tab contains ` kW` (`:417`).

Measured: a `power NN%` badge from `draw_tool_load_badges`
(`src/ui/sim_diagnostics.rs:1020`), a `power` flag from
`criterion_short_label` (`src/ui/sim_op_list.rs:1168`), and a pre-flight
sentence from `format_verdict_line` (`src/ui/preflight.rs:535`).

### Tool deflection

Predicted: a phrase inside the chip verdict tail — `headroom_phrase`
(`src/ui/feeds/compare.rs:577`) prints `NN % force headroom`,
`>99 % force headroom`, or `NN % past the deflection bound`. The ceiling is
a hover sentence in `efficiency_hover` (`:593`). The nomogram draws the
ceiling as a wedge ONLY when it falls on the chart; the `Ceiling::OffScale`
doc (`src/ui/feeds/explore.rs:298`) records that on the reference fixture
the ceiling sits about eighty times off scale, so the ORDINARY case draws
nothing.

Measured: an `L/D NN%` badge from `draw_tool_load_badges`
(`src/ui/sim_diagnostics.rs:1020`) and the bent-cutter overlay from
`draw_sim_deflection_overlay` (`src/app/viewport.rs:798`).

### Depth of cut

Predicted: a `DOC current → recommended` text row through `rail_row`
(`src/ui/feeds/compare.rs:384`). There is NO DOC limit on any surface.

Measured: the `axial DOC` entry in the `tracks` table
(`src/ui/sim_timeline.rs:454`) and the `Axial DOC` span row inside
`draw_span_body` (`src/ui/sim_diagnostics.rs:1604`). Neither names a
ceiling.

I searched `src/` for `depth_limit`, `max_doc`, `doc_cap` and
`axial_limit`. There are no hits. The nearest thing is the `depth_tier`
derate factor read in `draw_headline` (`src/ui/feeds/explore.rs:240`),
which lowers a FEED recommendation. It is not a depth ceiling.

### Gantry push force

Absent from the GUI. I searched `src/` for `gantry`, `thrust`,
`push_force`, `side_force`, `lateral_force`, `cutting_force` and
`force_n`. There are no hits outside the deflection force-headroom text.

---

## 3. What exists for charting

### The library

`egui_plot`. It is imported in exactly four files:
`src/ui/feeds/explore.rs:30`, `src/ui/feeds/shared.rs:29`,
`src/ui/sim_timeline.rs:9`, `src/ui/readiness_panel.rs:11`.

There are exactly THREE `Plot::new` sites in the crate:

| Plot id | Owning symbol (path:line) | Shape |
|---|---|---|
| `feeds_modal_nomogram` | `draw_chart_c` (`src/ui/feeds/explore.rs:421`) | feed versus RPM, one operation |
| `feeds_modal_project_scatter` | `draw_project_scatter` (`src/ui/readiness_panel.rs:763`) | feed versus RPM, every toolpath |
| `signal_track_{label}` | `draw_signal_track` (`src/ui/sim_timeline.rs:766`) | value versus move index, per track |

### No distribution exists

I searched `src/` for `histogram`, `percentile`, `quantile`,
`distribution`, `heatmap` and `bucket`. Nothing draws a distribution of a
metric. The word `heatmap` refers to the rest-depth surface overlay and to
collision-marker density, both in `src/app/gpu_upload.rs`. Those are 3D
geometry, not statistics.

One MCP function has `histogram` in its doc comment —
`build_per_depth_pass_summary` (`src/app/mcp.rs:4921`). It emits per-span
PEAKS and AVERAGES as JSON. It has no bins and no counts, and the GUI never
draws it.

### The one bar against a limit is dead code

`power_bar` (`src/ui/components/compare.rs:55`) renders
`Power: [bar] X / Y kW (Z %)` on an `egui::ProgressBar`, with a three-step
colour ramp from `power_color` (`:76`).

It has NO production call site. The only references in the whole crate are
its own definition, the re-export in `components::mod`
(`src/ui/components/mod.rs:44`), and the sentry ban list. `CompareRow`
(`src/ui/components/compare.rs:104`) is dead in the same way.

### What could be reused

`draw_signal_track` (`src/ui/sim_timeline.rs:675`) is the most reusable
piece. It already takes a `value_fn`, a colour, and an
`Option<(f64, f64)>` envelope that it paints as a shaded region. It
decimates to `SIGNAL_MAX_POINTS = 1600` (`:672`), links the X axis across
tracks, and handles hover, drag and click-to-seek.

Its axis is MOVE INDEX against VALUE. A histogram is COUNT against VALUE.
The two share the frame, the colour handling and the envelope shading, and
share nothing else. `egui_plot` gives `Polygon` and `Line`, which the
envelope code inside `draw_signal_track` already drives; a bar mark would
be new.

`draw_machine_envelope` (`src/ui/feeds/shared.rs:223`) is the existing
pattern for "draw a limit into a plot": a washed fill, no stroke, no
floating label, no legend row. The `CORRIDOR_ALPHA` doc
(`src/ui/feeds/explore.rs:56`) records that the corridor wedges follow the
same idiom on purpose.

---

## 4. Where a limits surface would naturally live

**The Readiness workspace owns the question by name. The Simulation
Inspector owns the only working implementation. They are different
workspaces, and that is the collision the design hits.**

The argument from the existing structure:

1. `Workspace::Readiness` carries the subtitle `"Is this safe to cut?"`
   in `Workspace::description` (`src/state/mod.rs:85`). That is the
   question a limits surface answers. No other workspace claims it.
2. Readiness is the only workspace with no side panels and no viewport.
   `draw_readiness_layout` (`src/app.rs:373`) draws ONE centred column,
   capped at `set_max_width(560.0)` (`:412`). Compare
   `draw_toolpath_layout` (`:419`) and `draw_simulation_layout` (`:482`),
   which both build a left panel, a right panel and a viewport. Readiness
   is the only workspace whose geometry can hold a chart grid.
3. But what Readiness shows today is the WRONG SHAPE. The `"Tool load"`
   `check_row` plus its three `CountPill::verdict` calls
   (`src/ui/readiness_panel.rs:196`, `:201`, `:207`, `:214`) count
   TOOLPATHS. They answer "how many operations are in trouble", never
   "how close is this cut to each limit".
4. The per-limit question IS already answered, once, by
   `draw_tool_load_badges` (`src/ui/sim_diagnostics.rs:1020`). It is the
   only site that prints several limits on a COMMON SCALE — percent of
   cap, through `pct_of_cap` (`:1142`). It carries three of the five
   limits, it is per toolpath, and its `CollapsingHeader` inside
   `draw_toolpath_section` (`:910`) starts closed.
5. So the extension target is `draw_tool_load_badges`, and the
   replacement candidate is the Readiness `"Tool load"` row plus its three
   pills. Those two symbols already claim the question.

Two structural facts constrain any move:

- **The two stages live in two workspaces.** The predicted half is the
  `ToolpathTab::FeedsSpeeds` tab (`src/ui/properties/mod.rs:3119`), which
  draws in BOTH the Toolpaths and the Simulation right panels. The
  measured half is `sim_diagnostics` and `readiness_panel`. One surface
  covering both stages either moves one half across a workspace boundary
  or draws twice.
- **Width.** `PANEL_WIDTH`
  (`tests/inspector_width_is_tab_independent_up4.rs:53`) pins the
  inspector rail at 240 points, and the test asserts that a tab switch
  never changes the width the inspector asks for. The same constant
  appears in the chip-verdict sentry
  (`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:56`). Five
  stacked histograms at 240 points is the geometric problem. The nomogram
  and the scatter both escape it — one is a window (`window::draw`,
  `src/ui/feeds/window.rs:69`), the other is a 560-point centred column.

Sentries a new or replacing panel must satisfy:

| Sentry symbol or file | What it pins |
|---|---|
| `tests/the_inspector_nests_once_dc5.rs` (module doc) | one nesting mechanism per level; a disclosure never duplicates a tab name |
| `tests/the_simulation_page_is_summary_first_dc6.rs` (module doc) | the Simulation page is summary-first; dense material starts closed |
| `inspector_width_is_tab_independent_up4` `PANEL_WIDTH` (`tests/inspector_width_is_tab_independent_up4.rs:53`) | the inspector asks the same width on every tab |
| `the_feeds_tab_paints_exactly_one_verdict_row_g_chipverdict` (`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:296`) | the Feeds tab paints EXACTLY ONE chip verdict row |
| `no_power_gauge_returns_to_the_card_g_chipverdict` (`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:393`) | no power gauge and no ` kW` on the Feeds tab |
| `tests/the_corridor_bounds_the_band_g_corridor.rs` (module doc) | the nomogram draws the bounds it has and abstains from the one it lacks |
| `tests/the_feeds_window_fits_the_screen_g_feedsfit.rs` | the Explore window fits the screen |

The design rule "a limit that is not set draws no bar" already has a
partner in the tree. The `status.is_vacuous()` branch inside
`verdict_badge` (`src/ui/sim_diagnostics.rs:1168`) paints `∅` and refuses
a colour when a gate measured an empty population. The chip-verdict
sentry's module doc
(`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:20`) states the
general rule: every `None` must reach the screen as a stated abstention,
never a zero, a blank, a dash or a 100 %.

---

## 5. The plumbing a distribution form would need

### The GUI reads a SUMMARY struct, not the raw trace

Every limit verdict in the GUI comes from ONE producer,
`rs_cam_core::gcode::project_load_report(&session, sim_trace)`, which
returns `ToolLoadReport`. Call sites, by symbol:

| Caller symbol | Path:line | Note |
|---|---|---|
| `readiness::load_report` | `src/ui/readiness.rs:355` | Readiness and pre-flight |
| `SimulationState::cached_load_report` | `src/state/simulation.rs:976` | Simulation bottom panel and Inspector |
| the inline build in `properties::draw` | `src/ui/properties/mod.rs:1013` | the inspector, UNMEMOED, once per frame |
| `App::mcp_get_tool_load_report` | `src/app/mcp.rs:2570` | the MCP tool |

Inside that report, one limit reaches the screen as
`rs_cam_core::tool_load::verdict::CriterionStatus`, which carries a SINGLE
scalar: `display_peak` plus `unit`. The two readers are `criterion_detail`
(`src/ui/sim_op_list.rs:1179`) and `verdict_badge`
(`src/ui/sim_diagnostics.rs:1151`). There is no population, no bin, no
percentile and no share-past-the-limit.

### The GUI DOES reach the raw trace, in one place

`draw_signal_spine` (`src/ui/sim_timeline.rs:266`) clones the
`Arc<SimulationCutTrace>` out of `sim.results`, then walks `trace.samples`
through `SpanAggregateCache::cutting_indices_for`
(`src/state/simulation.rs:417` for the cache type). So the sample
population is reachable inside `rs_cam_viz` — from the bottom panel, and
nowhere else.

The GUI's own accumulator is `SpanAggregate`
(`src/state/simulation.rs:298`). It holds sums, peaks and counts:
`sum_eng`, `peak_eng`, `sum_advance`, `peak_advance`, `n_advance`,
`sum_chip`, `peak_chip`, `n_chip`, `peak_doc`, `sum_mrr`. A mean and a
max, never a distribution. Bins mean widening this struct or adding a
second cache beside it.

### Per-limit availability of a per-sample value

| Limit | Per-sample producer the GUI can already call | Status |
|---|---|---|
| Chip thickness | `SimulationCutSample::effective_chip_thickness_mm`, read by the `tracks` table (`src/ui/sim_timeline.rs:454`); `rs_cam_core::tool_load::display::achieved_advance_per_tooth`, called by `summary_fn` (`:546`) | available, already called per sample |
| Depth of cut | `SimulationCutSample::axial_doc_mm`, read by the `tracks` table (`src/ui/sim_timeline.rs:454`) | available; no limit to compare against |
| Tool deflection | `rs_cam_core::tool_load::deflection::sample_tip_deflection_mm`, called by `peak_deflection_for_move` (`src/app/simulation.rs:683`) | available, but today it runs for ONE move at a time, and it needs the `ToolDefinition` and the `Material` beside the sample |
| Spindle power | none found | `crates/rs_cam_core/src/tool_load/power.rs` exports only `evaluate` (`:261`), which returns a whole-toolpath `PowerVerdict`. A power distribution needs a NEW core producer. |
| Gantry push force | none found | no producer anywhere in the GUI-facing surface |

For reference, `rs_cam_core::tool_load::display` exports exactly three
public functions: `achieved_advance_per_tooth` (`:40`),
`arc_mean_chip_thickness` (`:60`) and `advance_per_tooth_per_move`
(`:82`). That module is the GUI's whole per-sample vocabulary today.

### The predicted stage reads a different producer entirely

The predicted half never touches the trace. It reads:

- `rs_cam_core::feeds::suggest::feeds_preview_for_operation`, wrapped by
  `compute_preview_for_operation` (`src/ui/feeds/compare.rs:98`) and
  `compute_preview` (`:117`).
- `rs_cam_core::feeds::efficiency::cut_efficiency`, called inside
  `draw_inspector_comparison` (`src/ui/feeds/compare.rs:149`) and wrapped
  by `read_cut_efficiency` (`src/ui/feeds/explore.rs:164`).

`CutEfficiency` supplies `rubbing_floor_mm`, `deflection_ceiling_mm`
(optional) and `force_headroom` (optional). It carries NO power figure and
NO depth ceiling. A predicted bar for five limits therefore needs three
limits this struct does not hold.

### Two of the three caps come from the GUI, not from core

The `chipload_cap`, `power_cap_kw` and `deflection_cap` bindings inside
`draw_toolpath_section` (`src/ui/sim_diagnostics.rs:947`, `:955`, `:957`)
build the caps in the draw function:

- chipload cap: the end of the cached chipload envelope, from
  `SimulationState::cached_chipload_envelopes`
  (`src/state/simulation.rs:1009`).
- power cap: `max_power_kw * machine.safety_factor`, computed in the GUI
  from `machine.power`.
- deflection cap: `DEFLECTION_SAFE_LD_RATIO`, a GUI constant of `4.0`
  (`src/ui/sim_diagnostics.rs:1137`).

A design that draws every limit on one scale must decide where the limit
value comes from. Today two of the three come from the GUI, and each of
the three arrives by a different route.

### There is no structured provenance for a limit

The "where did this limit come from" hover exists three times, as free
text built inside each draw function:

- `efficiency_hover` (`src/ui/feeds/compare.rs:593`)
- `verdict_tooltip` (`src/ui/sim_diagnostics.rs:1197`)
- `criterion_detail` (`src/ui/sim_op_list.rs:1179`)

`ProvenanceBadge` and `ProvKind` exist and are re-exported from
`components::mod` (`src/ui/components/mod.rs:50`), but they stamp the
source of a RECOMMENDATION — vendor LUT row, formula, edge-radius floor —
not the source of a LIMIT. A hover that names the SETTING that produced a
limit has no component today.

### Caching

Three trace-keyed caches already exist and share one shape — a
`Weak<SimulationCutTrace>` plus `gui.edit_counter`:

- `SimulationState::cached_load_report` (`src/state/simulation.rs:976`)
- `SimulationState::cached_chipload_envelopes` (`:1009`)
- `SimulationState::cached_simulation_triage` (`:1056`)

A distribution cache would follow the same pattern. `cached_load_report`
already logs a warning past 8 ms, and the inline build in
`properties::draw` (`src/ui/properties/mod.rs:1013`) rebuilds a full
project load report on every frame with NO memo at all.

### Surfaces that would have to change

If a limit gained a distribution form:

| Symbol | Path:line | Why |
|---|---|---|
| `rail_efficiency_row` | `src/ui/feeds/compare.rs:459` | the predicted chip surface; a sentry pins it at exactly one row |
| `Corridor` / `draw_chart_c` | `src/ui/feeds/explore.rs:290`, `:365` | already draws the floor and the ceiling in a second idiom |
| `draw_tool_load_badges` | `src/ui/sim_diagnostics.rs:1020` | the only per-limit percent readout; the extension target |
| the cap bindings in `draw_toolpath_section` | `src/ui/sim_diagnostics.rs:947`–`:957` | the caps move if the scale is shared |
| `DEFLECTION_SAFE_LD_RATIO` | `src/ui/sim_diagnostics.rs:1137` | a GUI-owned limit value |
| `draw_span_body` | `src/ui/sim_diagnostics.rs:1493` | peaks and averages of the same quantities, with no limit named |
| `summary_fn` / `summary_env` | `src/ui/sim_timeline.rs:546`, `:561` | the only existing normalised-to-limit view |
| the `tracks` table | `src/ui/sim_timeline.rs:454` | `None` envelopes are a deliberate refusal to compare |
| `draw_verdict_hud` | `src/ui/sim_timeline.rs:113` | counts toolpaths, not limits |
| `toolpath_status_flags` | `src/ui/sim_op_list.rs:1023` | the same verdicts in a third vocabulary |
| the `"Tool load"` row and its pills | `src/ui/readiness_panel.rs:196`–`:214` | the replacement candidate |
| `tool_load_summary_detail` / `format_verdict_line` | `src/ui/preflight.rs:362`, `:535` | read the same report and state the same exceedances |
| `draw_verdict_badges` | `src/ui/optimize_modal.rs:926` | a fourth rendering of the same three verdicts |
| `SpanAggregate` | `src/state/simulation.rs:298` | it must hold bins, or a second cache must exist |
| `mcp_get_tool_load_report` | `src/app/mcp.rs:2570` | the agent-facing twin; `tests/mcp_wire_surface_pin.rs` pins the wire shape |

Note the count. The SAME three verdicts render in six vocabularies today —
`advance/tooth` / `advance/t` / `Chip`, `power`, `L/D` / `defl` /
`deflection`. Any shared scale has to pick one.

---

## 6. Gaps I could not resolve

1. **A per-sample spindle power producer.** I read the public functions of
   `crates/rs_cam_core/src/tool_load/power.rs` and found only `evaluate`
   (`:261`). Core is another agent's scope. I did not search the rest of
   core for a power helper under another name.
2. **Any gantry push force model.** Nothing in `rs_cam_viz` names one. I
   did not look in core.
3. **Any depth-of-cut ceiling.** The GUI has none, and I found no
   candidate to draw a bar against. Whether core holds one is outside my
   scope.
4. **The exact reach of the power-gauge ban.** `compare_source`
   (`tests/the_chipload_verdict_is_one_row_g_chipverdict.rs:288`) reads
   ONLY `src/ui/feeds/compare.rs`. A power bar in another file would not
   trip the source scan. The companion assertion (`:417`) is a render
   check, and it applies only to the text the Feeds tab paints. I did not
   run the test; I read it.
5. **Where the operator's "no more UI" rule is recorded.** I found no file
   in the tree that states it. I treated it as given by the brief.
6. **How often the per-limit badges are read.** `draw_toolpath_section`
   (`src/ui/sim_diagnostics.rs:910`) starts closed, so the only per-limit
   percent readout in the product is two clicks deep. I could not
   determine whether operators open it.

---

## Method note

Everything above is read from source in the working tree on 2026-09-17,
between about `10:00` and `10:09`. I ran no build and no test. I verified
every symbol-to-line pair after writing the first draft, and the three
corrections are listed at the head of this file.

---

# Follow-up by the commissioning session, 2026-09-17

## The ban on a predicted power bar rests on stale evidence

The survey found that `no_power_gauge_returns_to_the_card_g_chipverdict`
(`crates/rs_cam_viz/tests/the_chipload_verdict_is_one_row_g_chipverdict.rs`)
bans `ProgressBar`, `power_bar`, `power_color` and `rail_power_row` from
`ui/feeds/compare.rs`, with this reason:

> The power readout peaked at 23.6 % across the entire shipped matrix; it
> cannot separate a good cut from a bad one.

That was true when it was written. It is not true now, and the order of two
commits is why:

| | |
|---|---|
| `921aa0e3` | the chipload verdict replaces the power gauge |
| `73b84d69` | **R1** — power is built from the force model, not a constant |

The gauge was deleted BEFORE R1 rebuilt the power model. R1 raised the
predicted power about 8.6x at the reference fixture, because the edge term it
added carries most of the load at wood chiploads. So the 23.6 % was measured
against a model that no longer exists.

**Re-measured after R1**, 162 recipes across every machine preset, six species,
three diameters and three operation families:

| | Pre-R1 (the sentry's evidence) | Post-R1 |
|---|---|---|
| Median | about 1 % | **17.8 %** |
| p75 | — | 49.4 % |
| p90 | — | 89.4 % |
| Peak | **23.6 %** | **100.0 %** |
| Over 50 % | — | 25 % of recipes |

A bar that peaks at 23.6 % across the whole matrix genuinely cannot separate a
good cut from a bad one. A bar where a quarter of recipes pass half scale, and
the peak reaches the ceiling exactly, can.

The peak sitting at exactly 100 % with nothing above it is the power ladder
working: it clamps a power-limited recipe onto the ceiling rather than letting
it through.

## What must NOT happen next

The sentry must not be quietly deleted by the session that wrote it. Two
reasons, and the second matters more.

The ruling it encodes was an operator decision about what the Feeds card
shows. Reversing it is the operator's call, not the engine's.

And a sentry deleted by its own author, on the author's own re-measurement, is
the exact shape of re-pinning a test to suit the change. The rule this
programme has held all session is that a red cell goes green by the work, not
by moving the cell. The same rule applies to a green one.

**Recommendation:** put the measurement to the operator. If they reverse the
ruling, the sentry's assertion should be replaced rather than removed — it
should keep watching that a power readout is informative, with the bar being
that utilisation still spreads across the range. A ban whose reason has
expired should become a test of the reason.
