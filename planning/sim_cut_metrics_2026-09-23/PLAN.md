# Simulation cut metrics: distributions first, time series on request

Status: LANDED packages A–F on 2026-09-23 night; the operator looked on 2026-09-24 and the follow-up in §8 landed (not yet seen on screen). Date: 2026-09-23. Owner-facing decisions: §7 (assumed answers recorded there). Landed commits and residual items: §8.

## 1. The request

The operator finds the cut metrics hard to read and wants them to look better:

- The right panel shows a **distribution histogram** for each metric.
- The raw time-series plots open in the bottom drawer only when the operator
  asks for them ("see time series").
- Each histogram shows the **limits** as bands.
- Each histogram has an **(i)** hover. It says what "too low" and "too high"
  mean, and what you can see, hear or change to correct it.
- The duplicate "capture cutting metrics" control goes. The bottom one is
  removed.
- The right panel overflows its width in many cases. A generic fix is under
  investigation (§6); this work depends on it.

## 2. What the code does today

| Surface | File | What it draws |
|---|---|---|
| Bottom panel, always | `ui/sim_timeline.rs` `draw` | transport + verdict pills, boundary timeline, then the signal spine |
| Summary track | `sim_timeline.rs` `draw_signal_spine` | "advance/tooth vs band max", normalised, with red/amber zone shading |
| Five raw tracks | same, behind `CollapsingHeader("Signal graphs (5)")` | arc-mean chip thickness, arc engagement, axial DOC, MRR, feed; 90 px each |
| Empty state | `draw_spine_empty_placeholder` | `NotMeasured` + a **"Capture" checkbox** (the duplicate) |
| Capture control | `ui/sim_op_list.rs:97` "Setup & run" | "Capture cutting metrics" checkbox (the survivor) |
| Right panel | `ui/sim_diagnostics.rs` | verdict line; Project / "Now playing" / Span sections; limit rows are text (`chipload 84%`) with a hover |

Problems for the operator:

1. The time-series plots answer "where", but the first question is "how much
   of the cut is outside the limit". A line over 100 000 samples does not
   answer it; the eye reads the spikes (the decimator keeps the max per bucket
   on purpose), so every plot looks worse than the cut.
2. Five co-equal tracks with no limit on four of them. The operator cannot
   tell good from bad.
3. The limit rows state one number (the peak as % of the bound). One bad
   sample and a whole bad pass read the same.
4. The words are engineering words ("arc-mean chip thickness", "advance/tooth
   vs band max") with no statement of what to do.
5. The bottom panel takes 360 px by default even when every gate is green.

## 3. Target design

### 3.1 Right panel: a "Cut metrics" section

A new section in the Inspector, scoped to the **focused toolpath**
(`sim.focused_toolpath()`), below "Now playing". The limits are
per-toolpath (the chipload band depends on the tool, the material and the
op), so a project-wide histogram cannot carry a band. With no toolpath in
focus, the section says "Play or select a toolpath."

One card per metric, top to bottom:

```
Chipload (advance per tooth)                      (i)   ✓ 96 % in band
 ▁▂▅█▇▅▃▂▁ ▁                          
 |floor              ceiling|                     
 0.02            0.08         0.14 mm/tooth       
 3 % below floor (rubbing) · 1 % above ceiling     
```

- **Y axis = share of cutting time**, not sample count. Weight each sample by
  `segment_time_s`. "3 % of the cut time is below the floor" is the sentence
  the operator can act on. Sample count over-weights short dense moves.
- **Limit lines** at the gate's own bounds, and only the bounds that exist
  (floor only when the vendor row publishes one — `burn_floor`). Bars outside
  the band take the verdict colour (`CAUTION` below, `DANGER` above). See §7 Q1
  for the shading question.
- **Headline chip** at the right of the title: the in-band share, coloured by
  the gate verdict, not by the share. The gate verdict stays the authority.
- **Caption**: the out-of-band shares, each with its one-word consequence.
- **Hover a bar**: `0.031–0.036 mm/tooth · 14 % of cut time · 312 samples`.
- **Click a bar**: seek the playhead to the first sample in that bin
  (`UiCommand::SimJumpToMove`). This links the distribution to "where" without
  the time series.
- **Card footer**: `See time series ▸` opens the drawer (§3.3) scrolled to this
  metric's track.

v1 metrics:

| Card | Quantity | Unit | Bound source | Banded |
|---|---|---|---|---|
| Chipload | achieved advance per tooth, `tool_load::display::achieved_advance_per_tooth` | mm/tooth | `CriterionKind::Chipload` floor + ceiling (`BoundSource::VendorChipBand`) | yes |
| Depth of cut | `axial_engagement_mm` | mm | `CriterionKind::DepthOfCut` (rigidity rule of thumb, advisory) | yes, ceiling only, dashed (advisory) |
| Tool deflection | `deflection::sample_tip_deflection_mm` | µm | `CriterionKind::Deflection` | yes, ceiling |
| Spindle power | per-sample predicted power (core addition, §4 B) | kW | `CriterionKind::Power` | yes, ceiling |
| Engagement | `arc_engagement_radians` as degrees | ° | none | no |

MRR, commanded feed and arc-mean chip thickness stay in the time series only.
Arc-mean chip thickness has **no** band on purpose (Checkpoint H3,
`ARC_MEAN_CHIP_HOVER`); a histogram would invite the comparison H3 retired.

A metric whose criterion is `Unmodeled` or vacuous draws the `NotMeasured`
mark with core's reason, never an empty histogram (X-VAC). A criterion that is
`NotApplicableForOp` draws no card (same rule as `limit_rows`).

The existing text limit rows (`draw_limit_rows`) stay for Readiness, which
shares the renderer. In the Inspector the cards replace them for the banded
kinds; power/deflection/DOC/chipload rows move into their card header.

### 3.2 The (i) hover: plain facts

Hover on `(i)` shows four short lines. Proposed copy (STE, to review):

| Metric | What it is | Too low | Too high | Levers |
|---|---|---|---|---|
| Chipload | How far each cutting edge moves into the wood per turn: feed ÷ (RPM × flutes). | The edge rubs instead of cutting. Heat builds; you see burn marks, glazed walls, fine dust instead of chips, and the tool dulls fast. | The edge takes too big a bite. You hear chatter or a labouring spindle; you see torn grain, a rough wall, or the tool breaks. | Raise feed or lower RPM to raise it. Lower feed or raise RPM to lower it. |
| Depth of cut | How deep the tool is in the wood at this moment. | Only a time cost: more passes. | The tool and the gantry flex. You hear a deeper tone, see steps or a tapered wall. This limit is a rule of thumb for this machine. | Lower step-down, or use a stiffer (shorter, wider) tool. |
| Deflection | How far the tool tip bends away under the cutting force. | No risk. | The wall is out of size and the finish shows ridges or chatter marks. Near the limit the tool can snap. | Reduce stickout, reduce step-over or depth, use a larger diameter. |
| Spindle power | The power the cut takes from the spindle. | No risk. | The spindle slows under load. You hear the tone drop; chipload then rises and burns can follow. | Reduce depth or step-over first; keep chipload in band. |
| Engagement | How much of the tool's circle is in the wood. | Most of the time is air-cutting or a light skim. | Full-slot cuts: more heat, poor chip clearing, more deflection. | Adaptive/trochoidal clearing keeps it even. |

The Power and Chipload lever lines follow the feeds R4 ruling: keep chipload in
the band and reduce load through engagement (depth and width), not through feed
alone.

The copy lives in core (§4 D), so the MCP tool-load report and the CLI can
print the same words.

### 3.3 Bottom drawer: time series on request

- The bottom panel keeps transport, verdict pills and the boundary timeline.
  These are playback, not metrics.
- The signal spine (summary track + raw tracks) moves behind one toggle,
  **closed by default**. New UI state: `SimulationState::time_series_open`.
- The toggle is `See time series ▸` in the right-panel section header and on
  each card. There is one state and one route; the card links only scroll to
  a track. (A "Time series ▾/▸" handle on the bottom bar would be a second
  control for the same state; §7 Q3.)
- Drawer open: all tracks in one scroll area, the "Signal graphs (5)"
  disclosure is deleted. Each banded track draws its bound lines in its own
  unit (not normalised), so the drawer and the cards show the same numbers.
  The normalised summary track goes; its purpose moves to the chipload card.
- Drawer closed: the bottom panel shrinks to the transport + timeline height.
  `draw_simulation_layout` uses `default_size(360)` with a `max_size(480)`
  cap against a ScrollArea feedback loop. The closed state needs its own size
  (for example a smaller `max_size` when closed); check that the resize handle
  does not remember the open height.

### 3.4 Remove the duplicate capture control

- Delete the checkbox in `draw_spine_empty_placeholder`
  (`ui/sim_timeline.rs:1103-1110`). With the drawer model the whole bottom
  placeholder goes: the drawer is closed until metrics exist, and the right
  section carries the `NotMeasured` state.
- The survivor is "Capture cutting metrics" in `sim_op_list.rs:97`. Change its
  hover: it names "the bottom-panel signal graphs", which becomes false. New:
  "Records per-sample chipload, engagement, depth and MRR during simulation.
  Required for the Cut metrics section. Re-run simulation to apply."
- Amend the sentry `tests/the_simulation_page_is_summary_first_dc6.rs:352-359`.
  It asserts that the placeholder contains `checkbox(`. Keep the
  `NotMeasured::new()` and the no-`AppEvent::RunSimulation` rules; move them to
  the new right-panel empty state.

## 4. Packages

Each package is one commit or a small series. Order: A, G (§6.1), then B + D in
parallel, then C, then E, then F.

**A. Duplicate capture control** (viz only, small). §3.4. Can land first and
alone.

**B. Distribution producer** (core, new file `tool_load/distribution.rs`).

- One public function per banded metric, or one keyed by `CriterionKind`,
  that returns the gate's own population as `(value, weight_s, move_index)`.
- It MUST call the same filters the gate uses:
  `locality::is_steady_state_for_gate`, the `radial_woc_fraction < 0.02`
  air-cut rule, and the gate's effective-feed resolution for chipload and
  deflection. Do not re-implement them in the GUI. `chipload::
  steady_state_samples_for_toolpath` is `pub(crate)` today; the new file sits
  inside `tool_load` and can call it.
- Power needs a per-sample function. `power::predicted_power_kw` is
  `pub(crate)`; add `pub fn sample_power_kw(...)` beside it with the same
  inputs the gate uses.
- Binning: `Histogram { edges, weights_s, counts, first_move_per_bin,
  below_s, above_s, total_s }`. Range = the population's 0.5–99.5 percentile,
  widened to include every bound, with an overflow bin at each end so a spike
  is never hidden. ~24 bins.
- **Sentry** (core, fast, no heavy sim): on a small fixture, the maximum of
  the chipload population equals the gate's `display_peak`, and its sample
  count equals the criterion's `population.contributing`. This is the check
  that the histogram and the badge measure the same thing.

**D. Metric guide copy** (core, new file `tool_load/metric_guide.rs`).

- `CriterionKind::guide() -> MetricGuide { what, too_low, too_high, levers }`
  plus one for engagement (no criterion). `&'static str` fields. Copy from
  §3.2 after operator review.
- Reuse the existing remedies where they agree (`verdict.rs` ~2017–2124);
  keep the `ExceededDetail::remedy` path unchanged.
- Optional: add the guide to the MCP `get_tool_load_report` payload.

**C. Right-panel cards** (viz).

- New component `ui/components/histogram.rs`: a painter-drawn bar chart that
  takes `ui.available_width()` and a fixed height (~56 px). Do not use
  `egui_plot` here: it persists plot bounds per id (see the
  `ui/feeds/explore.rs` header note) and it brings axes and interaction the
  240 px rail does not need. It must fit the panel width (§6).
- Colours from `tokens` only (sentry `panels_read_the_token_module_up1`).
  Strings pass `ui_string_hygiene`. Watch `component_contracts_up2.rs` counts
  of emphasised labels per file.
- Cache: build the histograms once per (trace `Arc`, toolpath, edit counter),
  the same memo pattern as `sim.cached_load_report`. Never per frame.
- New section in `sim_diagnostics.rs`, between "Now playing" and the Span
  section. Follows DC6: summary-first, the section body opens by default only
  when a criterion exceeds.

**E. Time-series drawer** (viz). §3.3. Delete the "Signal graphs (5)"
disclosure and the normalised summary track; add per-unit bound lines to the
banded tracks; the new toggle state; the bottom panel size when closed.

**F. Docs and status.** `FEATURE_CATALOG.md` (the Simulation surface),
`planning/PROGRESS.md`, and this file's status line.

## 5. Constraints that apply

- The histogram population must be the gate population (B). If they differ,
  the card shows mass outside the band while the badge says Within.
- A band is drawn only in the metric's own unit and only when the gate has
  that bound. No GUI-owned caps (V2, 2026-09-18). No invented width (feeds
  chart-display ruling, 2026-09-23).
- A vacuous or unmodelled criterion never draws as a clean histogram (X-VAC).
- The Simulation workspace keeps ONE run route; no new run button.
- Per-folder rules: `ui/CLAUDE.md`, `ui/components/CLAUDE.md`,
  `tool_load/CLAUDE.md`.
- Cross-agent: the feeds session edits `feeds/*`, `tool_load/chipload.rs`,
  `diagnostics/adapters/*` and `ui/feeds/*`. This plan adds new files in
  `tool_load/` and touches `power.rs` and `deflection.rs` only to expose
  per-sample functions. Check `git status` for foreign hunks before each
  commit and commit with explicit paths.

## 6. Right-panel overflow

The operator reports that the right panel overflows in many cases. An agent
is surveying the panels for a generic fix (one wrapper that constrains every
side-panel child to the panel width, plus a sentry). Package C depends on it:
the cards must be built on that wrapper. Result: §6.1 (to be filled in).

### 6.1 Survey result (2026-09-23)

Version note: the crate is on egui **0.36** and egui_plot **0.37**
(`crates/rs_cam_viz/Cargo.toml`). The line in `ui/CLAUDE.md` that says
0.34.3 / 0.35 is stale; correct it in package G. The UI-FIT spec named in
memory was never written to disk.

**Mechanism.** All six workspace side panels (`app.rs` :343, :355, :479, :491,
:567, :586) use the same wrapper: `.max_size(SIDE_PANEL_MAX_WIDTH)` (420)
around a bare `ScrollArea::vertical()`. The x axis does not scroll and
`auto_shrink` is on, so the scroll area takes the content width. The panel
clamps it to 420 and stores it as next frame's width. One wide row therefore
grows the panel to 420 and the panel clips the rest.

**Why it "started".** UP1 (94afd33a) raised body text to 13 pt and captions
to 11 pt. That plan itself records that the change "overflows a dense panel".

**Past fixes were local**: truncating values in `KeyValueRow`, `ValueRow`,
`PrecedenceField`; the ChoiceRow stack rule (6c72a459); single-string wraps
(a2fee8ed, G-REACHWRAP, G-LEGENDWRAP, G-FEEDSFIT, F2). A global
`Style::wrap_mode = Wrap` was tried and reverted (b21e3294): it breaks grid
labels mid-word. A scoped `wrap_mode` has the same defect. Do not retry it.
`properties/toolpath_panel.rs:69` `set_max_width` alone does not stop the
growth.

**Recurring causes**: `ui.horizontal` around text (72 sites in `properties/`
and the sim panels; e.g. `setup.rs:91`, `machine_panel.rs:200`,
`stock.rs:325`, `sim_diagnostics.rs:319/945/1207`); grid label columns (75
`param_grid` + 14 `Grid::new`); fixed widths (`stock.rs:671` 180 px,
`components/compare.rs:62` 160 px, `machine_panel.rs:69` 140 px, `tool.rs:85`
120 px, default 100 px combos and sliders).

**Generic fix (package G, before C).** One `side_panel` helper in `app.rs`.
All six panels go through it:

```rust
fn side_panel(ui: &mut egui::Ui, panel: egui::Panel, default_w: f32,
              add: impl FnOnce(&mut egui::Ui)) {
    panel.default_size(default_w).max_size(SIDE_PANEL_MAX_WIDTH).resizable(true)
        .show_inside(ui, |ui| {
            egui::ScrollArea::both().auto_shrink([false, true]).show(ui, |ui| {
                let w = ui.available_width();
                ui.set_min_width(w);
                ui.set_max_width(w);
                add(ui);
            });
        });
}
```

`ScrollArea::both()` makes the x axis use the inner size, so the panel width
no longer follows the content. Text that wraps keeps wrapping. The cost: a
horizontal scrollbar appears where a row is still too wide. That scrollbar
also shows the residual sites.

**Residual local work (after G, can be staged):** the sentence rows above to
`horizontal_wrapped` or the ChoiceRow stack rule; fixed widths to
`ui.available_width().min(N)`; grid label columns to `KeyValueRow`. The
`ui/feeds/*` and `properties/feeds_speeds.rs` sites belong to the feeds
session: list them to that session, do not edit them.

**Sentry `every_side_panel_fits_its_width_g_panelfit`.** Behavioural arm
first (the headless `Context` pattern of
`inspector_width_is_tab_independent_up4.rs`): draw each panel entry point at
240 and 280 px, and assert that the stored panel width does not grow over two
frames and that the content `min_rect().width()` is not more than the inner
width. Source arm: `app.rs` has no `Panel::left(`/`Panel::right(` outside
`side_panel`. The b21e3294 lesson: a test that checks that a style is SET does
not check that the layout is RIGHT.

**Consequence for this plan.** Package C's histogram takes
`ui.available_width()` inside the helper, so it cannot widen the rail. Order
becomes: A, G, then B + D, then C, E, F.

## 7. Questions for the operator

Assumed answers, 2026-09-23 night (the operator handed the plan over for
an unattended run; each answer is the plan's own recommendation and stands
until the operator rules otherwise):

1. Shading: limit lines and coloured out-of-band bars, no background
   shading. This follows the feeds chart-display ruling of 2026-09-23.
2. Y axis: share of cutting time.
3. Drawer toggle: one route, in the right panel only.
4. Metric set: chipload, depth of cut, deflection, spindle power,
   engagement.


1. **Shading.** The feeds ruling of 2026-09-23 says "fainter band lines only
   when the row publishes a band, no shading" for the feeds charts. For these
   cards the plan draws limit lines and colours the out-of-band **bars**, with
   no background shading. Is that right, or do you want shaded zones here?
2. **Y axis.** Share of cutting time (recommended) or sample count?
3. **Drawer toggle.** Only in the right panel (one route, recommended), or
   also a handle on the bottom bar?
4. **Metric set.** Chipload, depth, deflection, power, engagement for v1.
   Add or drop any?

## 8. Landed, and what is left

| Package | Commit | Sentry |
|---|---|---|
| A duplicate capture control, G side-panel width helper | f1c62483 | `every_side_panel_fits_its_width_g_panelfit`, DC6 |
| B distribution producer, D metric guide | a1cbff0d | `histogram_population_is_the_gate_population_g_cuthist` |
| C right-panel cards, E time-series drawer | 22c3594c | `cut_metric_cards_read_the_gate_g_cutcards`, `the_limit_rows_read_their_own_bound_g_ownbound` |
| F docs | c61041e8 and this section | — |

Order change: the cards follow `criteria()` order (Chipload, Spindle power,
Tool deflection, Depth of cut, Engagement), not the §3.1 order, so the
Inspector, Readiness and the drawer list the kinds in one order.

Residual items, recorded by the editors and the verifier:

- The two content-width arms of `g_panelfit` are `#[ignore]` instruments:
  the toolpath tree (284 pt) and the setup properties panel (249 pt) hold a
  row wider than the 223 pt inner width at a 240 pt default (§6.1 residual
  local work). Fixed widths seen: `properties/tool.rs` 120, `machine_panel.rs`
  140, `components/compare.rs` 160, `stock.rs` 180.
- The chipload population in `distribution.rs` composes the gate's predicates
  in the gate's order but is not one shared function with `chipload.rs`; a
  mismatch logs a warning and fails `g_cuthist`.
- The depth gate has no `radial_woc_fraction < 0.02` air-cut rule, so its
  population counts air-cut samples; the histogram follows the gate.
- `distribution.rs` bins over the cap with zero tolerance; the depth gate
  (R2) allows 1e-4 mm of f32 noise, so a bin at the cap can read "above"
  while the badge reads Within.
- The drawer rebuilds and thins its track points every frame while open.
- The closed bottom panel never gets smaller than the height it last saved.
- "Now playing" still shows a "Tool load" header for the gantry-push row alone.
- No viz test draws the whole section on a real trace (no viz fixture builds
  a `SimulationResults`; a hand-built trace reads as stale). Arm A of
  `g_cutcards` holds the wiring instead.
- `build_cut_metric_set` copies the context-building loop of
  `project_load_report`; core should offer one public function that builds
  a `ToolpathLoadContext` for one toolpath.
- The guide copy is not on the MCP `get_tool_load_report` payload (optional
  in §4 D).
- Seen on screen on 2026-09-24. The operator ruled Q1 (shading is correct for these cards); the follow-up section below records the changes.

### §8 Follow-up 2026-09-24

The operator looked at the cards on screen (screenshot of 2026-09-24
06:51) and made four complaints. Each maps to one change:

| Complaint | Change |
|---|---|
| The cards touch the panel edge. | `app::side_panel` keeps `SIDE_PANEL_GUTTER` (`SPACE_3`, 8 pt) between the content and the right edge of the scroll area, for every side panel. Cause: the egui 0.36 scroll bar floats over the content. `g_panelfit` asserts the gutter. |
| The cards are too big. | No `Card` frame. One header row per metric (title, ⓘ, status glyph, muted peak as % of limit), a 38 pt bar area, a caption only when a share is out of band, hairlines between metrics. The limit row under the title is gone. The per-card "See time series" link is gone; the title opens the metric's track. A metric core could not measure is one muted line, sorted after the measured ones. |
| The "100 % IN BAND" chip is too much. | A status glyph: ✓ (`OK`), ✕ and the out-of-band share (`DANGER`, or `CAUTION` when advisory), — (`UNKNOWN`) for no cut time, muted "no limit". Its hover holds the limit row: face, setting, population, in-band share, bound clause, confidence reason. The hover is built from `verdict_face`, `row_caption` and `verdict_tooltip`, so G-OWNBOUND holds. |
| The band is hard to see; the limit line looks like a bar. | The band is a zone: a faint `OK` wash in band, faint `CAUTION` below the floor, faint `DANGER` above the ceiling. The limit marker is dashed, lower in contrast, starts above the bars under a filled cap, and ends in the label row. The overflow stubs are outlined. The x axis follows the data: core bins over the data only, and a bound more than 15 % of the data range outside it is an off-scale marker at the edge. |

**Shading ruling.** The operator ruled that shading is correct for these
cards. It replaces the §7 Q1 assumed answer. It is an exception: the feeds
charts keep the "no shading" chart-display ruling of 2026-09-23.

Sentries amended: `g_ownbound` finds a card row as the first line of the
status hover (a hover paints in the tooltip layer, so the card titles now
carry the order); `g_cutcards` asserts the hover parts, the caption rule,
the status glyph and the off-scale rule; `g_panelfit` asserts the gutter.
Core `Histogram::build` no longer widens its range to the bounds.

