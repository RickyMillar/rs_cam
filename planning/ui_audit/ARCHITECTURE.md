# rs_cam_viz IA — Shared Component Architecture

> How to implement the redesign **without re-creating the duplication the audit
> found.** The audit's root cause is the same one over and over: the same widget
> reimplemented per surface (provenance pill 3×, compare/power/MRR rows trapped in
> `feeds_modal.rs`, 105+ inline section headers, two visibility writers, two load
> rollups). The fix is a **shared `ui::components` layer** that every surface draws
> from, so a concept has one implementation and therefore one behaviour and one look.
>
> Grounded in the real crate: **egui 0.30**, the existing `&mut Vec<AppEvent>` emit
> model, the `OperationConfig` setters, and the undo-snapshot pattern. Components
> live in viz only and depend on core types read-only (keeps the core-independent
> guardrail).
>
> **Target platform: egui 0.34.3** (decision 2026-06-08 — see `BACKLOG.md` › Platform
> upgrade). The signatures below use 0.30 idioms that stay valid through 0.34; on 0.34
> the leaf widgets (`ProvenanceBadge`, `ValueRow`, `CountPill`, `SuggestButton`) are
> implemented as **`AtomLayout`** composites — cleaner than the manual `horizontal()`/
> `allocate` plumbing 0.30 needs — and drawer/modal surfaces use the 0.33 `Modal` API.
> The component *contracts* don't change; their internals get simpler. Surfaces tagged
> **♻ rewrite** in the backlog are rebuilt on these components, not refactored in place.

---

## 0. The duplication this layer kills (evidence)

| Concept | Today (duplicated) | Becomes |
|---|---|---|
| Provenance color | `pill_color_for_source` (`properties/mod.rs:3311`, green/amber) **+** properties narrative (`:1250`, cyan) **+** feeds_modal narrative (`:1223`, no color) — 3 colors for one signal | `ProvenanceBadge` (one glyph+RGB per source) |
| Labeled numeric input | `dv` / `dv_pill` (`properties/mod.rs:3272/3365`) | `ValueRow` (superset) |
| Suggest action | `suggest_pill` (`:3331`) + bulk buttons hand-rolled in 3 places | `SuggestButton{scope}` |
| Current-vs-recommended | `compare_row`/`woc_row`/`format_delta`/`power_color`/`draw_power_bar`/`draw_mrr_row` — all **private to `feeds_modal.rs`** | `compare::*` (shared; optimizer reuses) |
| Section header | raw `RichText::new(..).small().strong().color(..)` inline 105+× | `Ui::named_section` |
| Param grid | `Grid::new(..).num_columns(2).spacing([8.0,4.0])` repeated everywhere | `Ui::param_grid` |
| Count/verdict pill | `info_pill` (read-only, `sim_timeline.rs:193`) + ad-hoc labels; two rollups with different math | `CountPill{family,role}` over `ToolLoadReport::summary()` |
| Staleness | checked in 2 panels, ignored in 3 (`INS-005`/`OPT-003`/`TIM-009`) | `FreshnessGate` wrapper |
| Visibility toggle | written from Inspector **and** overlay, different labels | one `VisibilityToggle`, overlay-owned |

---

## 1. Module layout

```
crates/rs_cam_viz/src/ui/
  components/
    mod.rs          // re-exports + UiExt trait (named_section, disclosure, param_grid)
    provenance.rs   // ProvKind, ProvenanceBadge          [P7-001/002/003/004]
    value_row.rs    // ValueRow, MirrorRow, PrecedenceField [P1-005, P2-004/006, W3.1]
    suggest.rs      // SuggestScope, SuggestButton, Suggestion [P4-003]
    section.rs      // named_section/disclosure/SummaryCard [P3-*, INS-001, TIM-008]
    compare.rs      // CompareRow, DeltaTag, PowerBar, MrrRow (lifted from feeds_modal)
    pill.rs         // PillFamily, PillRole, CountPill, StatusPill [P4-001/002, P5-001, INS-003]
    freshness.rs    // Freshness, FreshnessGate              [W0.5: INS-005/OPT-003/TIM-009]
    visibility.rs   // VisibilityToggle (single home)        [P4-004/005]
    diagram.rs      // InlineDiagram trait + canvas helper    (formalizes existing idiom)
    nav.rs          // NavTarget + mirror/jump link helper    [mirror rows, jump pills]
```

Convention (matches the codebase): leaf widgets impl `egui::Widget` so callers use
`ui.add(Thing::new(..))`; layout helpers are `UiExt` methods or `.show(ui)` builders;
anything that can cause a mutation takes `events: &mut Vec<AppEvent>` and pushes
existing variants — **no component owns state or talks to the controller directly.**

---

## 2. Cross-cutting types

```rust
// components/provenance.rs — THE provenance vocabulary, defined once.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProvKind { VendorLut, SimOptimizer, Nomogram, HandEdit, Formula, Inherited }

impl ProvKind {
    pub fn glyph(self) -> &'static str {            // single source of glyphs
        match self { Self::VendorLut=>"▣", Self::SimOptimizer=>"◆", Self::Nomogram=>"◷",
                     Self::HandEdit=>"✎", Self::Formula=>"▲", Self::Inherited=>"〈〉" }
    }
    pub fn color(self) -> egui::Color32 {           // ONE canonical RGB per source
        match self { Self::VendorLut=>egui::Color32::from_rgb(80,180,80),
                     Self::SimOptimizer=>egui::Color32::from_rgb(90,150,220),
                     Self::Nomogram=>egui::Color32::from_rgb(170,120,210),
                     Self::HandEdit=>theme::TEXT_MUTED,
                     Self::Formula=>egui::Color32::from_rgb(220,180,60),
                     Self::Inherited=>theme::TEXT_FAINT }
    }
    pub fn label(self) -> &'static str { /* "vendor LUT" | "sim-optimized" | … */ }
}

// Maps the CORE provenance (post-W2.1 data model) into the viz vocabulary.
// Until W2.1 lands, a shim maps today's ChiploadSource (VendorLut|FormulaFallback|
// EdgeRadiusFloor) → {VendorLut, Formula}. After W2.1 this reads ValueProvenance.source.
impl From<&rs_cam_core::feeds::ValueProvenance> for ProvKind { /* … */ }
```

> **Dependency on W2.1 (data model).** `ProvenanceBadge` is honest only once each
> stored value carries `ValueProvenance { source, ref, when }`. Build the component
> now against the shim (it already beats today's 3-color mess), and flip the `From`
> impl when W2.1 lands. This is the one hard ordering constraint in the whole plan.

```rust
// components/nav.rs — where a mirror/jump link points (replaces ad-hoc tab/seek code).
#[derive(Clone)]
pub enum NavTarget {
    ToolpathTab(ToolpathId, Tab),     // Tab = Geometry|Feeds|Linking|Heights|Dressup
    PostPanel, StockAlignment, ViewportShowMenu,
    SimMove(usize),                   // seek the scrubber
}
// emits the existing AppEvent (SimJumpToMove, OpenFeedsModal, …) or a new
// AppEvent::Navigate(NavTarget) added once, used everywhere.
```

```rust
// components/freshness.rs
#[derive(Clone, Copy)] pub struct Freshness { pub stale: bool }
impl Freshness { pub fn of(rt_stale_since: Option<std::time::Instant>) -> Self { … } }
```

---

## 3. The components (signatures)

### 3.1 ProvenanceBadge — status badge, never an action `[P7-003]`
```rust
pub struct ProvenanceBadge<'a> { kind: ProvKind, reference: Option<&'a str>, compact: bool }
impl<'a> ProvenanceBadge<'a> {
    pub fn new(kind: ProvKind) -> Self;
    pub fn reference(self, r: &'a str) -> Self;   // obs-id / candidate-id, shown as ▣ (vendor 1234)
    pub fn compact(self) -> Self;                 // glyph only
}
impl egui::Widget for ProvenanceBadge<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response;  // click → opens "why this value"; never applies
}
```
Replaces `pill_color_for_source`, `source_short_label`, and the two narrative re-impls.

### 3.2 ValueRow — the one labeled-input row `[supersedes dv/dv_pill; enables W3.1 split]`
```rust
pub struct ValueRow<'a> {
    label: &'a str, value: &'a mut f64, suffix: &'a str,
    speed: f64, range: std::ops::RangeInclusive<f64>,
    prov: Option<ProvKind>,             // renders a ProvenanceBadge after the input
    suggest: Option<Suggestion<'a>>,    // single-field ⚡ (SuggestScope::Field)
    tooltip: Option<&'a str>,           // falls back to tooltip_for(label)
    changes_geometry: bool,             // appends "· changes cut" marker (DOC/WOC under Feeds)
}
pub struct ValueRowOutcome { pub edited: bool, pub suggested: bool }
impl<'a> ValueRow<'a> {
    pub fn new(label:&'a str, value:&'a mut f64, suffix:&'a str,
               speed:f64, range:std::ops::RangeInclusive<f64>) -> Self;
    pub fn prov(self, k: ProvKind) -> Self;
    pub fn suggest(self, s: Suggestion<'a>) -> Self;
    pub fn changes_geometry(self) -> Self;
    pub fn show(self, ui: &mut egui::Ui) -> ValueRowOutcome;  // calls ui.end_row() (grid-friendly)
}
```
`dv(ui,l,v,s,sp,r)` → `ValueRow::new(l,v,s,sp,r).show(ui)`. `dv_pill(..,sugg)` →
`ValueRow::new(..).suggest(sugg.into()).prov(k).show(ui)`. **The SPEED/CUT split
(W3.1) is just `.changes_geometry()` on DOC/WOC rows + the recipe suggest being
speed-only — enforced by the component, not by discipline.**

### 3.3 SuggestButton — three glyph roles, one type `[P4-003]`
```rust
#[derive(Clone, Copy)] pub enum SuggestScope { Field, Recipe } // ⚡  vs  ⚡⚡ <label>
pub struct Suggestion<'a> { pub recommended: f64, pub source: ProvKind, pub reference: Option<&'a str> }
pub struct SuggestButton<'a> { scope: SuggestScope, label: Option<&'a str>, source: ProvKind, enabled: bool }
impl egui::Widget for SuggestButton<'_> { fn ui(self, ui:&mut egui::Ui) -> egui::Response; }
```
`Field` renders a single ⚡ in `source.color()`; `Recipe` renders a doubled-glyph
labeled button (`⚡⚡ Apply recommended speeds`). The provenance *badge* is a separate
type, so "where from" can never be mistaken for "apply" again.

### 3.4 Sections & summary `[P3-002, INS-001, TIM-008, SHE-004/007]`
```rust
pub trait UiExt {
    /// Always-visible labeled group (the draw_dressup_params idiom, standardized).
    fn named_section(&mut self, title: &str, add: impl FnOnce(&mut egui::Ui));
    /// Collapsible "▸ Advanced" drawer.
    fn disclosure(&mut self, id_salt: &str, title: &str, default_open: bool,
                  add: impl FnOnce(&mut egui::Ui)) -> egui::CollapsingResponse<()>;
    /// 2-column param grid with the canonical [8.0,4.0] spacing.
    fn param_grid(&mut self, id_salt: &str, add: impl FnOnce(&mut egui::Ui));
}
impl UiExt for egui::Ui { /* … */ }

/// Glanceable header + body-behind-disclosure. The "dig deeper" primitive.
pub struct SummaryCard<'a> { headline: egui::WidgetText, badge: Option<StatusPill<'a>>, default_open: bool }
impl<'a> SummaryCard<'a> { pub fn show(self, ui:&mut egui::Ui, body: impl FnOnce(&mut egui::Ui)); }
```

### 3.5 compare.rs — lifted out of feeds_modal, shared with optimizer
```rust
pub struct CompareRow<'a> {
    label:&'a str, current:Option<f64>, recommended:Option<f64>, unit:&'a str, precision:f64,
    apply: Option<(FeedsField, ToolpathId)>, prov: Option<ProvKind>,
}
impl<'a> CompareRow<'a> { pub fn show(self, ui:&mut egui::Ui, events:&mut Vec<AppEvent>); }
pub fn delta_tag(current:Option<f64>, recommended:Option<f64>) -> egui::RichText; // ex format_delta
pub fn power_bar(ui:&mut egui::Ui, frac:f64);                                     // ex draw_power_bar
pub fn mrr_row(ui:&mut egui::Ui, mrr:f64);                                        // ex draw_mrr_row
```

### 3.6 pill.rs — verdict vs observation, read-only vs actionable `[P4-001/002, P5-001, INS-003]`
```rust
#[derive(Clone, Copy)] pub enum PillFamily { Verdict, Observation } // green/red ramp vs neutral
#[derive(Clone, Copy)] pub enum PillRole   { ReadOnly, Actionable } // flat vs rounded+hover+→
pub struct CountPill<'a> {
    label:&'a str, count:usize, denom:Option<usize>,   // denom renders the "/T" proof
    family:PillFamily, role:PillRole, jump: Option<NavTarget>,
}
impl<'a> CountPill<'a> { pub fn show(self, ui:&mut egui::Ui, events:&mut Vec<AppEvent>) -> egui::Response; }
```
Both rollups (Verdict HUD, Inspector overview) build `CountPill`s from
`ToolLoadReport::summary()` with `denom = Some(total_toolpaths)` — same producer,
same denominator, so the numbers **cannot** diverge again (kills P4-001/002). The
HUD's dead `_events` sink (P6-004) becomes the `jump` channel.

### 3.7 visibility.rs — one home `[P4-004/005]`
```rust
pub struct VisibilityToggle<'a> { label:&'a str, on:&'a mut bool, scope: VisScope }
pub enum VisScope { Global, PerToolpath(ToolpathId) }  // per-TP greys when its Global is off
```
Lives only in the Viewport overlay; the Inspector's duplicate checkboxes are deleted.

### 3.8 MirrorRow & PrecedenceField — make "mirror" and "default vs override" legible
```rust
pub struct MirrorRow<'a> { label:&'a str, value_text:&'a str, edit_at: NavTarget } // 〈 value 〉 → edit in X
pub struct PrecedenceField<'a> {           // spindle default vs per-op override [P1-005,P2-006]
    label:&'a str, project_default:u32, override_value:&'a mut Option<u32>,
}
```

### 3.9 diagram.rs — formalize the existing painter idiom
```rust
pub trait InlineDiagram { fn height(&self)->f32 {120.0} fn paint(&self, p:&egui::Painter, r:egui::Rect); }
pub fn diagram(ui:&mut egui::Ui, d:&impl InlineDiagram) -> egui::Response;  // allocate_exact_size + painter_at
```
Engagement diagram, pattern minimap, tool cross-section, timeline strips, feeds
nomogram all become `InlineDiagram` impls — same allocation/hover contract.

---

## 4. Usage map — who shares what (the payoff)

| Component | Surfaces that use it (pass 1 + pass 2) |
|---|---|
| **ProvenanceBadge** | Feeds tab, Feeds Details drawer, Geometry suggest rows, PrecedenceField, Heights mirror, Optimizer rollup, Inspector engagement, Fixture clearance pill |
| **ValueRow** | every op's Geometry & Feeds tab, tool editor, stock/setup, post panel |
| **SuggestButton** | Feeds tab (Field+Recipe), Geometry DOC/WOC, optimizer |
| **named_section / disclosure / param_grid** | all five toolpath tabs, tool editor (W3.4), inspector (W3.3), header/rail (W3.7), setup |
| **CompareRow / power_bar / mrr_row** | Feeds Details drawer **and** Optimizer rollup (was feeds-only) |
| **CountPill** | Verdict HUD, Inspector overview, Boundary timeline, status & workspace bars |
| **FreshnessGate** | Inspector cards, Optimizer baseline, signal spine, any concrete-number readout |
| **VisibilityToggle** | Viewport overlay (sole home) |
| **InlineDiagram** | engagement, pattern minimap, tool xsec, timeline strips, nomogram |

A finding like P7-003 ("three greens") becomes *structurally impossible*: there is
one `ProvKind::color`. P4-001 ("two rollups disagree") can't recur: both build
`CountPill` from one summary with one denom.

---

## 5. State / data-model changes this layer assumes

These are the 🟥 items from `BACKLOG.md` the component layer leans on:

1. **`ValueProvenance` on stored values (W2.1).** `ProvenanceBadge`'s `From` impl
   targets it; until then, the shim from `ChiploadSource`. **Build order: W2.1 → flip shim.**
2. **`apply_feeds_result_to_op` SPEED/CUT split (W3.1 code half).** `ValueRow.
   changes_geometry()` only tells the truth if applying speeds no longer writes
   `set_stepover`/`set_depth_per_pass`. Split the core fn into `apply_speeds_to_op`
   (feed/plunge/rpm) and `apply_cut_geometry_to_op` (stepover/doc), called separately.
3. **`ToolLoadReport::summary()` as the one rollup producer (W0.4).** `CountPill`
   callers read it; retire `verdict_counts()`.
4. **One staleness predicate (W0.5).** `Freshness::of(rt.stale_since)` is the single
   read; `FreshnessGate` is the single renderer.
5. **New `AppEvent` variants (small, additive):** `Navigate(NavTarget)` (mirror/jump
   links), and reuse existing apply/jump/visibility variants otherwise. No event the
   controller can't already route.

No change to the emit model, the undo-snapshot pattern, or the
event→handler→`ProjectSession` mutation route — components plug into all three as-is.

---

## 6. Build order (so components land safely)

0. **Platform:** land the egui 0.30→0.34.3 upgrade as its own visual-parity PR
   (`BACKLOG.md` W-UP) — spike `render/mesh_render.rs` for the wgpu 23→29 cost first.
   The component layer targets Atoms/Modal, so this comes before step 1.
1. **Foundations, no behaviour change:** `theme` already has the colors; add
   `components::{provenance(shim), section, value_row, suggest}`. Refactor `dv`/
   `dv_pill`/`suggest_pill` to delegate to them (pure internal move, surfaces unchanged).
2. **Collapse the 3 provenance re-impls** onto `ProvenanceBadge` (fixes P7-003 immediately).
3. **`pill` + `freshness`**, then retire `verdict_counts` and route both rollups +
   stale gates through them (W0.4, W0.5).
4. **`compare` lifted from feeds_modal**, shared into the optimizer.
5. **W2.1 data model** → flip the provenance `From`; **W3.1 core split** → set
   `changes_geometry` truthfully.
6. Surfaces are then re-laid-out (the 🟦 spec track) using the now-stable components.

Steps 1–4 are mechanical refactors that *reduce* line count and fix several findings
before any redesign is visible — a safe first PR sequence that pays its own way.
