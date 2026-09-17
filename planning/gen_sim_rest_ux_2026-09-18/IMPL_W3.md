# W3: the simulating ring, the gutter connector, the folded `dep` badge,
# and Generate All as the progress surface

Status: BRIEF, not implemented. Written 2026-09-18 from a read-only pass over
`rs_cam_viz`. Scope is `PLAN.md` §4.4 and the W3 row of §6.

**This is a handoff document.** `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
and `planning/ui_declutter_2026-09-14/` belong to another account. §9 states
which of their rulings each change touches and how it complies. Nothing here
edits their planning folder.

## 0. What W3 consumes from W0 and W1

Both peer briefs are written, and W3 reads their ACTUAL names. Do not invent
a second spelling.

| From | Item | Shape, as its brief states it |
|---|---|---|
| W0 | `rs_cam_core::session::dependencies::primary_edges(&ProjectSession)` | `Vec<Edge>`, one row per `(consumer, kind)`: the edge the card connector draws |
| W0 | `Edge { from: ToolpathId, on: Option<ToolpathId>, kind: EdgeKind }` | `on: None` means the declaration resolves to no toolpath |
| W0 | `EdgeKind = Stock \| Regions \| PrevTool`, `EdgeState = Ready \| Pending \| Broken` | re-exported from `session/mod.rs:57-61` |
| W0 | `dependencies::state(&Edge, &ProjectSession) -> EdgeState` | argument order is `(edge, session)` |
| W1 | `AppController::generation_plan_progress() -> Option<GenerationPlanProgress>` | `{ step, of, activity: PlanActivity, cancellable }` |
| W1 | `PlanActivity = Generating { name: String } \| Simulating { setup: String }` | names resolve at read time |
| W1 | `AppEvent::CancelGeneration` | W1 adds it; W3 only emits it |

**W3 reads `primary_edges`, not `edges`.** `edges` holds many Stock edges per
consumer and is the walker's input. `primary_edges` keeps the nearest enabled
source, or an `on: None` row when none is enabled, which is exactly the one
line per row §4.6 asks for. The Stock target question is therefore settled in
W0, not here.

`Edge.on` is an `Option`. A `None` source is `Broken` and draws the stub of
§4.7 with no click, because there is no row to select.

---

## 1. Per-row data and the two new fields

### 1.1 Today

`toolpath_panel.rs:18-34` holds two per-row snapshots, both built inside
`draw` and passed by value into `draw_toolpath_card`: `CardInfo` at `:103`
and `RuntimeSnapshot` at `:110`. Both exist for the reason recorded at `:16`
and `:204`: the card body borrows `state.viewport` mutably, so it cannot also
hold `&AppState`. Every session-derived value is read BEFORE the row body.
The new fields obey the same rule.

### 1.2 The fields

Add to `RuntimeSnapshot`, not to `CardInfo`. Both are derived per frame and
neither is project data.

```rust
struct RuntimeSnapshot {
    visible: bool,
    has_result: bool,
    freshness: FreshnessState,
    /// W3: every edge this row declares, read forward, from W0's
    /// `primary_edges`. Never stored; re-read each frame beside
    /// `freshness`. `on` is `None` when the declaration resolves to no row.
    edges_in: Vec<(Option<rs_cam_core::ToolpathId>, EdgeKind, EdgeState)>,
    /// W3: what a worker is doing to this row now. A SEPARATE read from
    /// `freshness`, which keeps its seven arms.
    in_flight: Option<InFlight>,
}

/// Not a `FreshnessState` arm: freshness is derived from the core result
/// cache, and "a worker is chewing on this" is a lane fact the cache
/// cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InFlight { Generating, Simulating }
```

### 1.3 The derivation and the precedence rule

`FreshnessState` gains NO arm. `DESIGN_SPEC.md` §4.3 says the seven-state
pure function stays exactly as it is, and
`the_toolpath_card_is_five_elements_dc1` drives all seven.

**Precedence, one line:** `Generating` spinner > `Simulating` ring > dot.

```rust
/// Pure over its inputs, so the sentry drives it without a `Ui`.
///
/// `Generating` comes from the freshness state, which already folds the
/// toolpath lane's `ComputeStatus::Computing` (freshness.rs:88). The
/// simulating arm cannot: the analysis lane is project-wide and the result
/// cache says nothing about it, so it reads the submit stamp (§2), guarded
/// by the lane being busy.
pub fn in_flight(
    freshness: &FreshnessState,
    submitted_scope: Option<&[ToolpathId]>,
    analysis_active: bool,
    id: ToolpathId,
) -> Option<InFlight> {
    if matches!(freshness, FreshnessState::Regenerating) {
        return Some(InFlight::Generating);
    }
    if analysis_active && submitted_scope.is_some_and(|s| s.contains(&id)) {
        return Some(InFlight::Simulating);
    }
    None
}
```

Call `dependencies::primary_edges(&state.session)` ONCE per `draw`, before
the setup loop, and index it into a `HashMap<ToolpathId, Vec<..>>` by
`Edge::from`. Per card it is O(n^2) and buys nothing.

### 1.4 How the panel learns `analysis_active` and the plan

`toolpath_panel::draw(ui, state, events)` sees only `AppState`, and neither
the compute lanes nor `AppController::generate_all` live there. Do NOT mirror
either onto `AppState`: a mirrored copy is a second store, the defect class
this sweep closes. Pass one context struct, so the signature grows once and
not per feature.

```rust
/// What the operation panel needs that `AppState` does not hold.
pub struct PanelContext<'a> {
    pub analysis_active: bool,
    pub plan: Option<&'a GenerationPlanProgress>,
}
```

Fill it at the one call site, `app.rs:428`. `draw_toolpath_layout` binds
`let lane_snapshots = self.controller.lane_snapshots();` at `app.rs:450`,
AFTER the left panel. Hoist that above `:419`, add
`let plan = self.controller.generation_plan_progress();`, and derive:

```rust
let analysis_active = lane_snapshots.iter()
    .find(|l| l.lane == ComputeLane::Analysis)
    .is_some_and(|l| l.is_active() || l.queue_depth > 0);
```

`LaneSnapshot::is_active` is `compute/mod.rs:100`. `app.rs:1082-1088` already
repaints while any lane is active or queued, so the ring animates with no new
repaint route.

---

## 2. The covered-ids stamp

**The field.** On `SimulationState` (`state/simulation.rs`), beside
`submitted_edit_counter` at `:800`:

```rust
/// Which toolpaths the in-flight simulation COVERS, stamped at submit
/// beside [`Self::submitted_edit_counter`] (W3).
///
/// Stamped at submit rather than read live at render, for the reason the
/// counter is: it describes what the run covers, and the operation list can
/// move under it while the lane works. `None` means nothing is in flight
/// through this controller.
pub submitted_scope: Option<Vec<ToolpathId>>,
```

Initialise to `None` in `SimulationState::new` and in
`state/simulation/playback_state.rs:58`.

**Where it is stamped.** `controller/events/simulation.rs:269`, in
`submit_simulation_for_groups`, the ONE door every simulation goes through.
Both `run_simulation_with_all` and `run_simulation_with_ids` call it.

```rust
self.state.simulation.submitted_edit_counter = Some(self.state.gui.edit_counter);
// W3: the same submit, the same reason. What this run covers.
self.state.simulation.submitted_scope = Some(
    groups.iter().flat_map(|g| g.toolpaths.iter().map(|e| e.id)).collect(),
);
```

`SimGroupEntry.toolpaths` is `Vec<SimToolpathEntry>` and `SimToolpathEntry.id`
is `rs_cam_core::ToolpathId` (`rs_cam_core/src/compute/simulate.rs:33-35,
83-84`). This is the exact set the simulator carves, so it needs no second
derivation.

**A prefix simulation reports its coverage for free.** W1 §2.3 replaces
`run_simulation_with_ids` with `run_simulation_prefix(setup_idx,
memoize_prefix)`, whose filter is "every ENABLED op in setups `0..=setup_idx`"
and which submits through this same `submit_simulation_for_groups`. The
groups it returns ARE the covered set, so the stamp above is correct for a
prefix run with no argument and no plumbing from W1. W1's reason for widening
the filter matters here too: a narrowed id list shifts the phantom-scan index
`k` and takes the snapshot too early.

**Where it is cleared.** The four places `submitted_edit_counter` is cleared:
`controller/events/compute.rs:894` (success), `:931` (`Err(Cancelled)`),
`:945` (`Err(Message)`), and `controller/events/simulation.rs:34`
(`invalidate_simulation`).

With one guard the counter does not have. A superseding submit B overwrites
A's stamp before A's cancellation drains, because `submit_analysis` clears the
queue and cancels the running job. A naive `take()` in the cancelled arm would
clear B's stamp and the ring would go dark while B runs. In the three drain
arms, clear only when the lane has gone quiet:

```rust
// W3: a late CANCELLED result from a superseded run must not clear the
// stamp its superseder wrote. The lane is the tie-break.
let lane = self.compute.lane_snapshot(ComputeLane::Analysis);
if !lane.is_active() && lane.queue_depth == 0 {
    self.state.simulation.submitted_scope = None;
}
```

`invalidate_simulation` clears unconditionally, as it does the counter: it
cancels the lane on the line above (`simulation.rs:28`).

The `analysis_active` guard in §1.3 is the second belt. A stamp that survives
an unforeseen drain path still draws no ring, because the lane is idle. The
residual failure is a MISSING ring, never a stuck one.

---

## 3. The ring glyph

`draw_state_dot` (`:367-411`) gains one argument. The `Regenerating` branch
at `:391-393` is untouched: `egui::Spinner` stays the generating form.

```rust
match in_flight {
    Some(InFlight::Generating) => { ui.put(rect, egui::Spinner::new().size(tokens::SPACE_3)); }
    Some(InFlight::Simulating) => draw_sim_ring(ui, rect),
    None => { ui.painter().circle_filled(rect.center(), tokens::SPACE_2, status_role.text()); }
}
```

Keep `let (status_text, status_role, hover) = status_chip(freshness);`
verbatim: `freshness_surfaces_g_freshrender.rs:130` asserts that literal.

egui 0.34 has no arc primitive. `Shape::line` over sampled points is the
pattern the crate already uses (`ui/properties/tool.rs:431`,
`operations/shape_diagrams.rs:348`).

```rust
/// A hollow ring with a rotating gap: the simulating form (R7). A different
/// SHAPE, not a second spinner colour. §2.6 rule 3: colour is never the only
/// channel, and two spinners in two colours make it the only one.
fn draw_sim_ring(ui: &mut egui::Ui, rect: egui::Rect) {
    const SWEEP: f32 = std::f32::consts::FRAC_PI_2 * 3.0;   // 270°, 90° gap
    const SEGMENTS: usize = 24;
    const TURN_PER_SECOND: f32 = std::f32::consts::TAU;
    let centre = rect.center();
    let radius = tokens::SPACE_2;
    // §5 "Progress spinner: continuous, linear". The clock is the frame time,
    // not an animate_* helper: those interpolate toward a target and a
    // rotation has none.
    let start = ui.input(|i| i.time) as f32 * TURN_PER_SECOND;
    let points: Vec<egui::Pos2> = (0..=SEGMENTS)
        .map(|k| {
            // SAFETY: k <= SEGMENTS, so the cast is exact.
            #[allow(clippy::cast_precision_loss)]
            let a = start + (k as f32 / SEGMENTS as f32) * SWEEP;
            egui::pos2(centre.x + radius * a.cos(), centre.y + radius * a.sin())
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(1.5, tokens::INFO)));
    // The Spinner does this, and app.rs:1082 repaints while a lane is active.
    // Both, so the ring cannot stall if either route changes.
    ui.ctx().request_repaint();
}
```

`tokens::INFO` is the accent. The ring is not a verdict, and §2.6 gives
`INFO` to "informational".

**Hover.** Build it from the row's own Stock edge, so it names the consumer
and not the run. With a Stock edge:
`Simulating \u{00B7} stock for <this row's name>`. A covered row with no Stock
edge: `Simulating \u{00B7} this operation carves the stock`. Append to the
hover built at `:400-406`, before the `Click to generate` line, so one hover
carries state and activity.

---

## 4. The connector

```
 gutter  card
  ┌─┐ ┌────────────────────────────────────────┐
      │ ■  ●  Pin Drill         6.00 mm End Mill │  no edge
      │ ■  ●  Back Rough        6.00 mm End Mill │  source of the next
  ┗━  │ ■  ◌  Holes             6.00 mm End Mill │  Stock, Pending
  ┃   ├────────────────────────────────────────┤
  ┗━  │ ■  ●  Rivers (back)     20° V-Bit        │  Regions on Holes, chain
      ├────────────────────────────────────────┤
      │          Setup 2 (back)                  │
  ↑━  │ ■  ◔  Lakes (back)      20° V-Bit        │  source off list, stub + ring
  └─┘ └────────────────────────────────────────┘
```

Row 3 depends on row 2 (Stock, Pending). Row 4 depends on row 3 (Regions), so
the two lines stack in one gutter column and the chain reads as one thread.
Row 5 sits in another setup whose source is collapsed or filtered, so it draws
a stub with an up glyph, and its dot is the ring because the analysis lane is
simulating its prefix.

### 4.1 The gutter, without touching the card frame

Do NOT wrap `draw_toolpath_card` in a new `ui.horizontal`. Widen the drop
zone's left margin instead, at `toolpath_panel.rs:86`:

```rust
// W3: the gutter is the drop zone's left margin, so every card rect starts
// GUTTER points right of the zone and the connector owns the space to their
// left. The card's own click rect (`:269`) is INSIDE the Frame, so it never
// covers the gutter and a connector click is clean.
let drop_frame = egui::Frame::default()
    .inner_margin(egui::Margin { left: tokens::SPACE_3 as i8, right: 2, top: 2, bottom: 2 });
```

**`PLAN.md` says 6 points. 6 is not on the §2.1 grid** (0, 2, 4, 8, 12, 16,
24, 32). Use `SPACE_3` (8); see §9.2. Do not add a `GUTTER: f32 = 6.0` token:
`ui/components/CLAUDE.md` forbids a literal spacing, and a new off-grid token
is a §2.1 change that is not W3's to make.

`compute_drop_index` (`:582-596`) reads the pointer Y only, so the wider
margin does not disturb drag and drop.

### 4.2 The rect map: one local, one frame, two passes

Record each card's rect during the draw and paint the connectors after the
setup loop, in the SAME frame. Not a one-frame lag, for two reasons:

1. the panel sits inside `egui::ScrollArea::vertical()` (`app.rs:425`), so a
   lagged map is wrong by the scroll delta on every scrolling frame;
2. a cross-setup edge needs a rect recorded inside an EARLIER setup's
   `dnd_drop_zone` closure, which a per-zone map cannot see.

Not egui temporary memory either. `panel_drafts_leave_egui_memory_ui09` holds
a named allowance list that "only ever goes down", so a new `insert_temp`
breaks that sentry and needs the other account's ruling. A local `Vec` needs
neither.

```rust
// W3: where each drawn card landed THIS frame. Local, so it cannot outlive
// the layout it describes.
let mut row_rects: Vec<(ToolpathId, egui::Rect)> = Vec::new();
for (setup_id, setup_name, toolpath_indices) in setups_data { /* push here */ }
draw_connectors(ui, &row_rects, &edges, events);   // before the add menu
```

`draw_toolpath_card` already holds the rect: `inner_response.response.rect`
at `:325`. Return it.

### 4.3 The elbow

Three segments as one `Shape::line`, so the corners join:

```rust
let x = rect_d.left() - tokens::SPACE_3 / 2.0;          // the gutter centre
let points = vec![
    egui::pos2(x, rect_s.center().y),                    // beside the source
    egui::pos2(x, rect_d.center().y),                    // down the gutter
    egui::pos2(rect_d.left(), rect_d.center().y),        // elbow into this row
];
```

### 4.4 Role, stroke and the second channel

| `EdgeState` | Role | Stroke | Why |
|---|---|---|---|
| `Ready` | none | 1.0 solid `tokens::HAIRLINE` | structure, not a verdict (§2.6 rule 1) |
| `Pending` | `Caution` | 1.0 dashed, `Role::Caution.text()` | waiting, a measurement came back |
| `Broken` | `Danger` | 1.5 dashed, `Role::Danger.text()` | source missing or disabled |

**Colour is not the only channel.** §2.6 rule 3 asks for a glyph beside every
colour, and an 8 point gutter cannot hold `!` or `✕` legibly. The dash pattern
is the second channel and stroke width the third. Use
`egui::Shape::dashed_line(&points, stroke, dash, gap)`. The word still reaches
the operator through the hover and through the row dot, so nothing is
colour-only.

### 4.5 Hover and click

Allocate an interaction rect over the VERTICAL segment only, after painting,
with a stable id:

```rust
let hit = egui::Rect::from_x_y_ranges(
    (x - tokens::SPACE_2)..=(x + tokens::SPACE_2), y_top..=y_bottom);
let resp = ui.interact(hit, egui::Id::new("tp_edge").with(d_id).with(s_id),
                       egui::Sense::click())
    .on_hover_text(edge_hover(&source_name, kind, state));
if resp.clicked() {
    events.push(AppEvent::Ui(UiCommand::Select(Selection::Toolpath(s_id))));
}
```

`edge_hover` is pure, so the sentry drives it:

| Kind | `Ready` | `Pending` | `Broken` |
|---|---|---|---|
| `Stock` | `After <S>` | `After <S> · waiting for simulation` | `After <S> · no upstream stock` |
| `Regions` | `Rest regions of <S>` | `Rest regions of <S> · <S> is not current` | `Rest regions of <S> · source missing` |
| `PrevTool` | `Rest after <S>` | `Rest after <S> · <S> is not current` | `No previous operation with that tool` |

Add `\nClick to select it.` when the source is in the list.

### 4.6 Stacking, chains, several dependents

One line per DEPENDENT row, each starting at its OWN source. That one rule
settles the three cases:

- **A chain A→B→C** draws A..B and B..C, which meet end to end in one gutter
  column.
- **Two dependents of one source** overlap on the shared span. Paint in ROW
  order, sources first, so the longer line goes under the shorter one. The
  overlapping span draws once: a `Ready` hairline under a `Pending` amber
  reads amber, which is the honest answer.
- **A row with two edges** (Stock and Regions on one rest op) draws the WORSE
  state only, ranked by `Role::severity_rank` (`components/chip.rs:88`). One
  line per row keeps the gutter one column wide and R32's "one row" honest.

### 4.7 Cross-setup and off-list

Setups are drawn in one `ui.vertical` inside one scroll area, so a source in
an earlier setup HAS a rect and draws a long line straight through the setup
header. That is the picture of a cross-setup dependency, which nothing in the
product draws today.

A source is off-list when it is not in `row_rects`: a collapsed setup, a
filtered list, or a source the session no longer holds. Draw a 6 point stub
upward from the elbow, ending in `\u{2191}`, in the edge's role colour, with
the hover `<kind> <S> \u{00B7} not in this list`. The stub is clickable when
the source merely is not drawn, and not clickable when the source is gone.

`ui.painter()` at `draw` scope clips to the scroll viewport, so a line to a
source scrolled off the top ends at the viewport edge with no extra work.

---

## 5. Folding the `dep` badge (R3, assumed YES)

The `PrevTool` edge carries the same three cases: `Resolved` is `Ready`,
`Stale` is `Pending`, `Missing` is `Broken`. Nothing is lost, and the card
loses two words at rest.

| Item | File:line |
|---|---|
| `pub enum RestBadge` | `toolpath_panel.rs:723-730` |
| `RestBadge::text` | `:734-739` |
| `pub fn rest_badge` | `:753-804` |
| `fn draw_rest_badge` | `:817-831` |
| the `rest` local and its match | `:212-217` |
| the `rest` parameter of `draw_name_and_tool` | `:440-456` |
| `rest_predecessors_in_session` | `state/rest_dependency.rs:90-108`, conditional |

**Keep `rest_predecessors`** (`rest_dependency.rs:68-83`).
`ui/properties/operations/validate.rs:261` calls it on a DRAFT entry whose
model may differ from the stored config, so W0 cannot subsume it. Delete
`rest_predecessors_in_session` only if W0's core `edges()` derives the
PrevTool edge from the same rule; if W0 leaves PrevTool in viz, keep the
wrapper and build the edge from it. Either way there must be ONE rule, which
is G-RESTBADGE's whole point.

`rg "dep|no dep|RestBadge|rest_badge" crates/rs_cam_viz` finds three pins:

| File | What it pins | Action |
|---|---|---|
| `tests/rest_badge_one_predicate_g_restbadge.rs` | badge and validator agree over 8 cases; `:343` asserts `badge.text() == "dep"` | REPLACE, do not delete: the contract moves to §8.2 |
| `tests/the_toolpath_card_is_five_elements_dc1.rs:159` | the card still calls `draw_rest_badge(` | HANDOFF: their sentry, needle becomes `draw_connectors(` |
| `src/state/CLAUDE.md:37` | the sentry row | rename to the new sentry |

`src/state/rest_dependency.rs:3` and `validate.rs:253` carry doc comments
naming `ui::toolpath_panel::rest_badge`. Re-point both.

---

## 6. Generate All as the progress surface

### 6.1 The progress struct (W1 exposes, W3 reads)

W1 §6 defines it. W3 adds nothing:

```rust
pub enum PlanActivity { Generating { name: String }, Simulating { setup: String } }
pub struct GenerationPlanProgress {
    pub step: usize,          // 1-based position of the step in flight
    pub of: usize,
    pub activity: PlanActivity,
    pub cancellable: bool,
}
#[must_use]
pub fn generation_plan_progress(&self) -> Option<GenerationPlanProgress>;
```

Derived each frame, never stored. `None` means no plan runs and the button
draws its normal label. W1 also stops the five-second status line from
carrying plan progress (`controller.rs:405-407`), so the button is the ONE
progress surface and not a second one.

### 6.2 The button

`toolpath_panel.rs:42-48` becomes:

```rust
let label = match ctx.plan {
    None => "Generate All".to_owned(),
    Some(p) => match &p.activity {
        PlanActivity::Generating { .. } => format!("Generating {}/{}", p.step, p.of),
        PlanActivity::Simulating { setup } => format!("Simulating \u{00B7} {setup}"),
    },
};
let mut button = crate::ui::components::Button::primary(label)
    .min_width(ui.available_width());
if let Some(p) = ctx.plan {
    // SAFETY: `of` is clamped to 1, so the ratio is finite and 0..=1.
    button = button.progress(p.step as f32 / p.of.max(1) as f32);
}
if ui.add(button).clicked() {
    events.push(match ctx.plan {
        None => AppEvent::GenerateAll,
        // W1 owns `cancellable`. A step that cannot be cancelled swallows
        // the click rather than raising an event the handler would drop.
        Some(p) if p.cancellable => AppEvent::CancelGeneration,
        Some(_) => return,
    });
}
```

`PlanActivity::Generating` carries the op `name`. The label uses the count,
not the name: `PLAN.md` §4.4 names `Generating 3/8`, and a name of any length
would move the button's one centred label about while a plan runs.

**Keep the literal `"Generate All"` in the file.**
`the_toolpath_card_is_five_elements_dc1.rs` lists it as a non-vacuity anchor,
and a fully dynamic label makes every `!contains` arm pass for the wrong
reason.

### 6.3 The kit addition

`components::Button` has no fill fraction and no secondary label
(`button.rs:83-135`). Add the minimum, a `.progress(f32)` builder:

```rust
/// A fraction of this button's width, painted as a 2 point bar along its
/// bottom edge (W3). `None` paints nothing.
///
/// A progress BAR and not a second label:
/// `component_contracts_up2::a_full_width_button_paints_its_centered_label_once_ur6`
/// asserts the widget impl calls `ui.painter().text(` exactly ONCE. The
/// progress WORDS are the button's own `text`, which the caller composes.
#[must_use]
pub fn progress(mut self, fraction: f32) -> Self {
    self.progress = Some(fraction.clamp(0.0, 1.0));
    self
}
```

Paint it in the `Widget` impl AFTER the hover-and-press repaint block
(`button.rs:177-190`), which covers the whole rect and would erase a bar
painted before it:

```rust
if let Some(fraction) = self.progress {
    // §5: a bar that jumps is a wrong instrument. `animate_value_with_time`
    // is LINEAR in egui 0.34, which is correct here: progress is a
    // measurement, not an arrival.
    let shown = ui.ctx().animate_value_with_time(
        id.with("progress"), fraction, tokens::MOTION_BASE);
    let bar = egui::Rect::from_min_size(
        response.rect.left_bottom() - egui::vec2(0.0, 2.0),
        egui::vec2(response.rect.width() * shown, 2.0));
    // INK_05, not ACCENT: the Primary fill IS ACCENT (button.rs:41), so an
    // ACCENT bar is invisible there. INK_05 on ACCENT reads 6.22
    // (button.rs:15) and needs no new token.
    ui.painter().rect_filled(bar, egui::CornerRadius::ZERO, tokens::INK_05);
}
```

### 6.4 Cancel

**The event does not exist today.** `rg "AppEvent::Cancel|UiCommand::Cancel"`
finds only `UiCommand::CancelCompute` (`ui_command.rs:768`), handled at
`controller/events/mod.rs:517` as `self.compute.cancel_all()`. That cancels
the lanes and leaves `AppController::generate_all` armed, so the ladder would
still resume and still report.

W1 §6 adds `AppEvent::CancelGeneration`, whose handler sets
`plan.cancelled = true`, cancels the toolpath lane, and cancels the analysis
lane as well when the step in flight is a simulation. It deliberately does NOT
call `invalidate_simulation`, which would throw away the snapshots the plan
already earned. W3 only emits the event.

**One W3 consequence of that ruling.** A cancel that does not invalidate the
simulation leaves `submitted_scope` set until the `Err(Cancelled)` drains. The
lane-quiet guard of §2 clears it on that drain, so no ring survives the
cancel.

The button is never disabled while a plan runs. It IS the cancel, and a
disabled control with no route leaves a long plan unstoppable from the panel.
A step W1 marks `cancellable: false` swallows the click instead, and its hover
says why.

---

## 7. Motion

| Element | Helper | Duration | Curve |
|---|---|---|---|
| Progress bar width | `ctx.animate_value_with_time` | `MOTION_BASE`, 0.18 s | linear, the only value form egui 0.34 offers |
| Ring gap rotation | `ui.input(\|i\| i.time)` | continuous | linear, §5 "Progress spinner" |
| Connector colour change | none | n/a | an edge state change is a re-read, not an arrival |
| Button hover fill | `motion::fast` | 0.12 s | `cubic_out`, unchanged |

The ring uses no `animate_*` helper: those interpolate toward a target, and a
rotation has none. §5 rule 2 ("no animation exceeds 200 ms") exempts the
spinner row by naming it "continuous".

---

## 8. Tests

### 8.1 Extend by handoff, do not break

`tests/freshness_surfaces_g_freshrender.rs`. The seven states STAY. Add one
arm to `the_card_reads_the_freshness_state`:

```rust
assert!(PANEL_SRC.contains("in_flight(") && PANEL_SRC.contains("InFlight::Simulating"),
    "the ring is a separate read from freshness, and the card must take it");
assert!(!include_str!("../src/state/freshness.rs").contains("Simulating"),
    "FreshnessState keeps its seven arms: the ring is a lane fact, and the \
     freshness state is derived from the core result cache");
```

Keep `status_chip(freshness)` spelled as it is at `:130`.

### 8.2 New: `connector_reads_edge_state_g_connector.rs`

1. **Role and hover.** For each of the 9 `(EdgeKind, EdgeState)` pairs, the
   role is the §4.4 row, the hover names the source, and the two unready
   states say why. Non-vacuity: 9 pairs, 9 distinct strings.
2. **The G-RESTBADGE contract, carried over.** The PrevTool edge is `Broken`
   if and only if `validate.rs` refuses the same configuration. Port the 8
   cases of `rest_badge_one_predicate_g_restbadge.rs`, which already builds
   the fixtures.
3. **Geometry.** Given a rect map of five rows and a chain, each elbow's three
   points sit on the gutter centre line, the vertical span covers both row
   centres, and an off-list source yields a stub, not a line to nothing.
4. **Non-vacuity.** The panel source contains `draw_connectors(`, and contains
   neither `draw_rest_badge(` nor `"no dep"`.

### 8.3 New: a ring sentry

1. **Pure.** `in_flight(&Current, Some(&[id]), true, id)` is
   `Some(Simulating)`; with `analysis_active = false` it is `None`; with a
   scope excluding `id` it is `None`;
   `in_flight(&Regenerating, Some(&[id]), true, id)` is `Some(Generating)`,
   which pins the precedence.
2. **Rendered.** Drive `draw_state_dot` headless: the in-flight pass yields an
   `egui::Shape::Path`, the idle pass an `egui::Shape::Circle`.

The harness pattern is `tests/the_heights_diagram_fits_f2.rs:82-155`:

```rust
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // epaint panics if a TexturesDelta drops unapplied.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}
// let mut out = ctx.run_ui(RawInput::default(), |ui| draw_state_dot(..));
// let shapes = std::mem::take(&mut out.shapes); out.textures_delta.clear();
```

---

## 9. Handoff to the other account

### 9.1 Their rulings this touches

| Ruling | What it says | How W3 complies |
|---|---|---|
| **R24** one state dot | one dot carries the state, the word on hover, click generates | The ring REPLACES the dot while a lane runs. Still one indicator, one place, one hover, the same click. No element at rest. |
| **R32** the card is ONE row | `swatch · dot · name · tool · eye · …`, fixed height | Unchanged. The gutter is 8 points of SPACE outside the card frame, and the `dep` badge LEAVES the row, so the card loses an element and gains none. |
| **DC1 five elements** | `tests/the_toolpath_card_is_five_elements_dc1.rs` | Two needles change: `draw_rest_badge(` becomes `draw_connectors(`, and `"Generate All"` must survive as the idle label anchor. Both are their file. |
| **Rule D** one height | the card's height never varies with state | The ring and the dot share the `SPACE_4` box. The bar is 2 points INSIDE the existing button rect. |
| **R27 / F2.2 lesson** | a claim about another surface is READ before it is ruled | Every surface this moves work onto is cited with a line number. |

### 9.2 Two decisions that are theirs

1. **The gutter is 8 points, not the 6 that `PLAN.md` §4.4 names**, because 6
   is not on the §2.1 grid. Accept `SPACE_3`, or rule a new token.
2. **R3 itself.** This brief assumes YES. If R3 is NO, §5 does not happen, the
   card keeps the badge, and the connector draws PrevTool edges beside it,
   which is one idea in two places and should be avoided.

### 9.3 Identifiers W3 must not touch

`row_hover_tint` (`components/kv_row.rs`), `draw_trace_badge` (`ui/sim_*`),
`LANE_SCALE`, `SPACE_0`, `INK_00`, `COLLISION_POINT` (`ui/tokens.rs`). None is
read or written by anything above.

### 9.4 The second per-toolpath list: `sim_op_list.rs`

**Recommendation: no ring and no connector there.** Its rows are built from
`sim.boundaries()` (`sim_op_list.rs:256`), which exist only AFTER a simulation
result lands, so no row can be in flight while it is drawn. Its dot means
focused or not focused (`:301-307`), not freshness. A connector would restate
the run order that the list already IS. Leave it alone.

---

## 10. Blast radius and risks

| Risk | Why it bites | Mitigation |
|---|---|---|
| **Drag and drop while lines are drawn** | Connectors paint AFTER the cards, so a line sits over the insertion indicator at `:334-337`. | Skip the connector pass while `egui::DragAndDrop::has_payload_of_type::<ToolpathId>(ui.ctx())`. The rects move during a drag, so a line drawn then is wrong as well as ugly. |
| **30 rows** | `primary_edges()` per card is O(n^2). | Call it once per frame and index it by `Edge::from`. The connector pass is O(edges), at most one per row after §4.6. |
| **Colour-blind operators** | Three states on three hues alone breaks §2.6 rule 3, and an 8 point gutter cannot hold a glyph. | Dash pattern is the second channel, stroke width the third (§4.4). The hover and the dot carry the word. |
| **A stuck ring** | A drain path that does not clear `submitted_scope`. | Two guards: the lane-quiet clear (§2) and `analysis_active` in `in_flight` (§1.3). The residual failure is a missing ring, never a stuck one. |
| **A late cancel clears a newer stamp** | `submit_analysis` cancels the running job, so A's `Err(Cancelled)` can drain after B's submit. | The lane-quiet guard in §2. `submitted_edit_counter` has the same hazard and does not guard; fixing it is W4's area, not W3's. |
| **W1 slips** | `generation_plan_progress` and `CancelGeneration` do not exist yet. | §6 is severable. Ship §1 to §5 first; the button keeps its current form. |
| **The panel signature changes** | `toolpath_panel::draw` grows `ctx: &PanelContext`. | One call site, `app.rs:428`. One struct, so it grows once and not per feature. |

## 11. Order of work

1. §2 the stamp, its four clear sites and the lane guard. No UI.
2. §1 the derivation, `PanelContext`, the `app.rs` hoist.
3. §3 the ring, plus the ring sentry (§8.3).
4. §4 the connector, with §8.2 red first.
5. §5 the fold, plus the three test and doc re-points.
6. §6 the button, after W1 lands `generation_plan_progress` and
   `CancelGeneration`.

Local loop after each step: the `state/` and `ui/` folder sentries,
`cargo test -p rs_cam_viz -q`, then clippy. No full core gate.
