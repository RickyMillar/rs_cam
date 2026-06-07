# rs_cam_viz IA — Consolidated Action Backlog

> One ranked list to drive the work, merging **pass 1** (feeds/tabs epicenter,
> `DIAGNOSIS.md`) and **pass 2** (the neglected surfaces, `pass2/DIAGNOSIS_NEGLECTED.md`).
> 76 findings total, folded into 24 workstreams.
>
> **Ranking is by user-harm, not by area.** Order of precedence:
> safety/false-assurance → numerical correctness → unreachable capability →
> foundational data model → structural IA → confusables/provenance →
> discoverability/polish. Feeds is *not* first; a safety gap and two wrong
> numbers outrank it.
>
> **Track legend** — each workstream is tagged:
> - 🟥 **CODE** — engine / data-model / wiring change (logic, not layout)
> - 🟦 **SPEC** — UI/IA reorg only (grouping, affordance, disclosure — no logic)
> - 🟪 **BOTH** — needs a code change *and* a UI change to land
>
> Effort: S (hours) · M (a day or two) · L (multi-day). Finding ids in `[...]`.
> The two single-track views (all code work / all spec work) are at the bottom.

---

## Plan posture (v2 — 2026-06-08)

Decisions that reframe this backlog:
- **Platform: full send to egui 0.34.3.** Its own behaviour-preserving PR before the
  component layer (see *Platform upgrade* below). Widget-API churn is tiny; the real
  cost is wgpu 23→29 in the render layer. Spike `render/mesh_render.rs` first.
- **Rewrite, not patch, on the worst surfaces.** With egui 0.34 + Atoms + the shared
  component layer underneath, the feeds cluster, inspector, tool management, optimizer,
  and timeline are *rewritten on components* rather than edited in place. The simpler
  surfaces are still restructure-in-place. Tier-3 items are tagged **♻ rewrite** or **✎ in-place**.
- **Scope = clean up what exists, not grow features.** No command palette, no
  templates/batch this round. The one "new" surface — the job-readiness dashboard
  (W3.8) — is included only because it *consolidates already-computed signals*; it ships
  no new capability.
- **MCP / unified AI-assistant is deferred to its own workstream.** But the backend
  unification this cleanup already does (`ValueProvenance`, the single
  `ToolLoadReport::summary()` rollup producer, the one freshness predicate, the
  apply-speeds/apply-cut split) is designed **MCP-ready**, so later work sits on it
  without a second refactor. Unify at the backend where it falls out of the cleanup;
  don't build the assistant surface now.

---

## Platform upgrade · gates the spec track

### W-UP · egui 0.30 → 0.34.3 🟥 CODE · L · own PR
Brings Atoms (0.32 — substrate for the component layer), Modal (0.33), font hinting (0.34).
**Spiked 2026-06-08 (`SPIKE_egui034.md`) — verdict: ~3–5 person-day mechanical slog, not
a trap.** 89 compile errors, deps resolve clean, **winit stays 0.30.13**.
- **wgpu render track: 1–2 days.** Easier than feared — `mesh_render.rs` had *zero* wgpu
  errors; the churn is repetitive descriptor-field renames concentrated in `render/mod.rs`
  (`push_constant_ranges`→`immediate_size`, `multiview`→`multiview_mask`, new `depth_slice`,
  `bind_group_layouts: &[Option<&_>]`, `depth_write_enabled`/`depth_compare`→`Option`).
- **egui/eframe shell track: 2–3.5 days.** Wider blast radius (~18 UI files): `App::update`
  →`App::ui`, `SidePanel`/`TopBottomPanel`→`Panel`, painter `rect_stroke`+`StrokeKind`,
  `Margin`/`Rounding` f32→i8/u8 literals, `close_menu`→`close()`, plot rebuild, visual re-verify.
- **⚠ One non-obvious gotcha:** use **`egui_plot = "0.35"`** (there is NO 0.34.x for egui
  0.34). `egui_plot="0.34"` silently pulls a second egui (0.33) → ~31 baffling
  `Color32: From<Color32>` errors in `sim_timeline.rs`. One-line fix once you know it.
- **Note:** wgpu/glow are NOT direct deps — render reaches wgpu via `use egui_wgpu::wgpu;`,
  so bumping the egui family moves wgpu; no separate wgpu pin strictly needed.

**Tier 0 logic fixes do NOT depend on this** (engine-side); the component layer and every
♻ rewrite do.

---

## TIER 0 — Safety & numerical correctness · do first

These mislead the operator about whether something is safe or how long/efficient
a job is. Mostly small code changes, highest harm-per-effort.

### W0.1 · Fixture geometry must reach collision checking 🟪 BOTH · L · **top priority**
`[P6-003]` Fixture Z extent/clearance is editable and persisted but
`CollisionCheckRequest` has no fixture field — a holder crashing a clamp is never
flagged. **A safety check giving false assurance.**
- CODE: add `obstacles: &[CollisionObstacle]` to `CollisionCheckRequest`, built
  from each enabled fixture's `origin_z`/`size_z`/`clearance`; test holder/shank
  against them; report `CollisionKind::Fixture{fixture_id}`. Populate at both call
  sites (`session/compute.rs`, `worker/helpers.rs`).
- SPEC: per-fixture clearance-check status pill (`◇ not checked / ✓ clear / ✗ hit`),
  🛡 marker on the safety-consumed Z fields, point-of-need `Run holder clearance ▸`;
  editing a 🛡 field resets the pill to `◇` (never falsely green).

### W0.2 · Fix the optimizer's wrong headline cycle time 🟥 CODE · S
`[OPT-001]` `compute_optimized_cycle()` zeroes NoSafeImprovement/Skipped rows on a
false "no baseline access" premise; `outcome.rs` confirms `candidates[0]` *is* the
baseline. Result: optimized time understated, savings % inflated on every refused
row. Use the real baseline for refused rows; only genuinely-baseline-less Skipped
rows are excludable (and should be shown as "—", not folded into the total).

### W0.3 · One collision tally, severity-correct 🟥 CODE · S
`[SHE-002, SHE-003]` Status bar prints holder-only collisions; workspace bar prints
holder+rapid — same project, two numbers, one safety-relevant. And
`simulation_badge` returns "stale" *before* checking collisions, hiding a red error
behind a yellow warning. Unify the count (single source), and check
collisions-before-stale in both badges (mirror `readiness_badge`'s order).

### W0.4 · Reconcile the two load rollups + fix the mislabeled tooltip 🟥 CODE · S
`[P4-001, P4-002]` Verdict HUD counts per-gate-criterion (≤3×/toolpath); Inspector
counts per-toolpath — irreconcilable, and the HUD tooltip labels a per-criterion
count as a toolpath count on a safety metric. Make both read
`ToolLoadReport::summary()`; retire `verdict_counts()`; show a visible `/T`
denominator as proof both count the same thing.

### W0.5 · Wire `is_stale` everywhere a concrete number is shown 🟥 CODE · M
`[INS-005, OPT-003, TIM-009]` Systemic: `is_stale` exists and two panels use it, but
focused hotspot/issue cards (stale banner on a dead code path), the optimizer
baseline, and the bottom-panel signal spine all render concrete cut metrics styled
as fresh on stale data. One staleness predicate, consulted at every numeric readout.

---

## TIER 1 — Restore unreachable capability

Real, fully-built functionality the shipping GUI can't reach.

### W1.1 · Revive tool delete/duplicate/import/manage 🟪 BOTH · M · **high harm**
`[TOO-001, SHE-001, TOO-002]` The entire 446-line `project_tree.rs` — which holds
the complete tool CRUD set — has **zero call sites** (compiles only because `pub`,
so dead-code lint stays silent). Net effect: **a user cannot delete or duplicate a
project tool in the shipping GUI**, and has no panel-discoverable path to the
library manager / import. Decision: harvest the CRUD into the *live* tool home
(`toolpath_panel.rs` collapsible) — per-row context menu + `Del` key wiring
`DuplicateTool`/`RemoveTool`, plus `+ From Library ▾` and `Manage Library…` — then
**delete `project_tree.rs`** (don't leave a second dead nav home).

### W1.2 · Wire-or-retire the dead/inert controls 🟥 CODE · S each
- `[P6-001]` Edit › Delete Selected is a hardcoded `add_enabled(false)` advertising
  a "Del" it never fires — wire selection-derived enablement to `RemoveToolpath`.
- `[P6-002]` `StockVizMode::ByOperation` has a live GPU branch but no selector and
  no serde — add the combobox entry + derive (GPU path already exists → wire, don't cut).
- `[P2-003]` adaptive3d's generic Dressups entry-style combo is editable but compute
  coerces it to None — grey it out when `entry == ForceNone` (gate isn't on
  `strip_all_reason`), or remove it (the `Adaptive3dConfig` control is authoritative).
- `[TIM-010]` Gate-trip dot click resolves `hotspots.get(10000+i)` → always None, so
  the drill-into-hotspot half is dead; map the synthetic index to its marker.

---

## TIER 2 — Foundational data model · unblocks the provenance UI

Do before Tier 4's provenance visual language — the UI can't be honest until the
model carries origin.

### W2.1 · Per-value provenance on `OperationConfig` 🟥 CODE · L
`[P7-001, P7-002, P7-004]` Feeds are stored as bare `f64`/`u32`; the displayed color
is recomputed from a fresh LUT lookup, not from what produced the stored value; and
a single `ChiploadSource` enum is overloaded to label four independently-derived
fields (so a vendor RPM reads as amber "formula fallback"). Add
`ValueProvenance { source, ref, when }` per applied value; stop reusing
`ChiploadSource` as the universal provenance label. **Blocks W4.1.**

### W2.2 · One source of truth for post config 🟥 CODE · S
`[P1-004]` `gui.post` and `session.post_config()` can disagree until next save
(GUI-vs-MCP window). Make the session canonical; route post-panel edits through a
`SetPostConfig`-style event that writes immediately (as the wizard already does).

---

## TIER 3 — Structural IA reorg · the big moves (spec-heavy)

The "spread everywhere / dump of fields" problems. These are the largest design
artifacts and where the mockups already exist.

**♻ Rewrite on the component layer:** W3.1 feeds, W3.3 inspector, W3.4 tool editor,
W3.5 optimizer, W3.6 timeline. **✎ Restructure in place:** W3.2 tab scaffold, W3.7
header/rail (the per-op *bodies* inside the tabs are still rebuilt from the new
section/value components).

### W3.1 · Feeds: four editors → one, with a SPEED/CUT split 🟪 BOTH · L
`[P1-001, P1-002, P1-003, P2-001, P2-002]` The duplication epicenter. Collapse the
Params-tab fields, the legacy Feeds-tab card, the modal, and the third "Suggest all"
into ONE authoritative Feeds & Speeds tab; the modal becomes a read-mostly Details
drawer; retire the legacy card + duplicate button.
- CODE: split `apply_feeds_result_to_op` so "Apply recommended speeds" writes
  SPEED only — a DOC/WOC change must be a separate, explicitly-confirmed,
  separately-attributed geometry apply (today it silently rewrites cut geometry).

### W3.2 · Toolpath properties → five concern tabs + dig-deeper 🟦 SPEC · L
`[P2-003, P2-004, P3-001, P3-002, P3-003]` Replace the flat
`[Params][Feeds][Heights][Dressups]` catch-all with **Geometry / Feeds & Speeds /
Linking / Heights / Dressup**, each summary-tier + progressive disclosure (adopt the
named-section grammar the codebase already uses in `draw_dressup_params`). Folds in
the Z-plane scatter and the entry-style split. *(Note: this is the move that echoes
the user's original example — it's earned by the data, but it is NOT the headline.)*

### W3.3 · Inspector: summary-first + three-scope split 🟦 SPEC · M
`[INS-001, INS-002]` `draw_project_overview` is one always-expanded scroll of ~6
grids answering three questions at three scopes (project/toolpath/span). Glanceable
verdict on top, the rest behind disclosure; separate the per-toolpath and per-span
blocks from the project rollup.

### W3.4 · Tool editor: grouped geometry + consistent commit 🟪 BOTH · M
`[TOO-003, TOO-005]` The same `draw_tool_fields` form live-applies in the properties
panel but requires Save in the modal — unpredictable. Pick one commit model
(draft + Apply/Revert, shown). Group geometry; separate co-visible "Shaft Diameter"
vs "Shank Diameter" so it's clear which drives which.

### W3.5 · Optimizer rollup: real affordances + column semantics 🟦 SPEC · M
`[OPT-002, OPT-004]` TradeOff/MarginalSafe rows say (in code comments) "open the
modal" but expose no way to — add a per-row `Open ▸`. The blank col-1 header
conflates selectable/locked/N-A; the verdict column mixes glyphs and free text —
give selection and verdict consistent, headed affordances.

### W3.6 · Timeline: disambiguate strips, add a summary tier, de-overload the op row 🟦 SPEC · M
`[TIM-001, TIM-002, TIM-008, TIM-006]` Stacked look-alike strips with divergent
X-axes and click contracts; five co-equal signal tracks with no
which-metric-is-in-trouble indicator; op rows packing run/visibility/jump/(debug)tree.
Give each strip a distinct visual role + consistent click contract, add a
metric-resolved summary above the spine, and separate the op-row concerns.

### W3.7 · De-overload the toolpath header & setup rail 🟦 SPEC · M
`[SHE-004, SHE-007]` The always-on toolpath header is secretly five groups
(identity / geometry IO / stock linking / generate / diagnostics) with the richest
signal — the diagnostics ribbon — buried last. The Setup rail mixes nav with project
KPIs that belong elsewhere. Group by concern; lift diagnostics; move misplaced KPIs.

### W3.8 · Job-readiness dashboard — consolidation, NOT a new feature 🟦 SPEC · M
One setup→cut readiness view folding signals **already computed but scattered**:
preflight (`ui/preflight.rs`), holder-clearance/collision (post-W0.1), tool-load
verdicts via `ToolLoadReport::summary()` (post-W0.4), and staleness (post-W0.5). Built
entirely from existing components — `CountPill` (verdict family), `StatusPill`,
`FreshnessGate`, the fixture clearance pill — so it adds no capability, just answers
"is this safe to cut?" in one place. Depends on **W0.1 + W0.4 + W0.5**. This is the
surface the user pointed at: *"job-readiness could show that [the cleanup]."*

---

## TIER 4 — Confusables & one visual language

"Things that look similar must have obviously separate roles." Mostly spec; W4.1
depends on W2.1.

### W4.1 · One provenance visual language 🟦 SPEC · M · *(needs W2.1)*
`[P7-003, P7-005]` Vendor-LUT state is painted three different greens across
surfaces; provenance depth lives only in the modal. Define ONE glyph + ONE canonical
RGB per source (`▣` vendor / `◆` sim / `◷` what-if / `✎` hand-edit / `▲` formula /
hollow `< >` inherited), rendered identically everywhere, as a status badge (click =
"why", never apply). A shallow cue on the summary, full derate one click behind it.

### W4.2 · Action-glyph roles: ⚡ vs ⚡⚡ vs status chip 🟦 SPEC · S
`[P4-003]` One `⚡` glyph means "suggest this field", "suggest all", and a bare
provenance pill. Give the three distinct treatments: single-field value chip,
doubled-glyph labeled bulk button, and the (non-clickable) provenance badge.

### W4.3 · Viewport visibility: one home, legible scope 🟪 BOTH · S
`[P4-005, P4-004]` `show_stock/cutting/rapids` are writable from both Inspector › View
and the Viewport overlay with *different labels*; per-row C/R buttons add a second
scope. Make the overlay the sole home (remove the duplicate Inspector checkboxes —
keep stock appearance there); grey per-row buttons when the global toggle is off so
the layering explains itself.

### W4.4 · Inspector card disambiguation + metric consistency 🟦 SPEC · M
`[INS-003, INS-004, INS-006]` Hotspot vs issue cards look near-identical in the same
slot; the same engagement datum renders as `%` in one place and raw fraction in
another; engagement is shown with no "comparative-only" caveat. Distinct card
identities; one unit/format per metric; a provenance cue on engagement.

### W4.5 · Z-plane naming + spindle precedence 🟦 SPEC · S
`[P2-004, P1-005, P2-006]` Drill "Retract Z" / Post "Safe Z" / Heights planes reuse
labels across four panels (two carry apology tooltips). Give each Z concept one
home + distinct label; one two-state spindle widget showing default + override + which
wins (delete the apology prose — the relationship is the affordance).

---

## TIER 5 — Discoverability & prose cleanup

### W5.1 · Surface hidden actions inline 🟦 SPEC · S
`[SHE-006, INS-008, OPT-006]` Enable/Duplicate reachable only by right-click; the
Selected-span lock has no in-panel entry (only an after-the-fact unlock); the project
optimizer hides when nothing exceeds and gives no rationale when greyed. Add visible
inline entry points.

### W5.2 · Signal-spine discoverability 🟦 SPEC · S
`[TIM-003, TIM-004]` The spine vanishes with no placeholder when no trace, and its
click/drag-to-seek + gate-snap interactions hide behind one hover tooltip. Add an
empty-state pointing at the capture toggle; make the interactions visible (cursor,
handle, per-track cue).

### W5.3 · Replace prose crutches with affordances 🟦 SPEC · S
`[TOO-006, OPT-005, TIM-005, SHE-005, P5-001, P5-003, P5-004]` Grey-italic safety
prose, "Try this" sentences the operator must retype despite the modal owning
re-optimize, "click the red lines below" tooltips, doubled face-selection captions,
non-clickable count pills. Convert each to the affordance it describes.

### W5.4 · Low-severity mop-up 🟦 SPEC · S
`[P1-006, P1-007, P2-005, P2-007, INS-007, INS-009, TIM-007]` Conceptual-scatter and
labeling lows; batch with whichever tier touches the same surface.

---

## Out of scope this round (recorded so it isn't silently dropped)

- **Unified AI / MCP assistant surface** — consolidating feeds-suggest + optimizer +
  sim-feedback + MCP machinist into one assistant. Deferred to its own workstream. The
  backend unification here (`ValueProvenance`, single rollup producer, freshness
  predicate, speed/cut split) is built **MCP-ready** so that work has a clean foundation.
- **Command palette / global nav** and **templates / presets / batch** — not this round.
  Scope is cleanup of existing surfaces, not new capability.

---

## Single-track views

### 🟥 Code / data-model / wiring track (execution order)
| # | Workstream | Effort | Harm |
|---|-----------|--------|------|
| 1 | W0.1 fixture→collision (code half) | L | safety |
| 2 | W0.2 optimizer cycle-time math | S | correctness |
| 3 | W0.3 collision tally + severity order | S | safety-adj |
| 4 | W0.4 load-rollup reconcile + tooltip | S | correctness |
| 5 | W0.5 wire `is_stale` everywhere | M | trust |
| 6 | W1.1 tool CRUD revive (code half) | M | reachability |
| 7 | W1.2 wire/retire dead controls (×4) | S×4 | reachability |
| 8 | W2.1 ValueProvenance data model | L | unblocks W4.1 |
| 9 | W2.2 canonical post config | S | consistency |
| 10 | W3.1 split `apply_feeds_result_to_op` (code half) | M | correctness |
| 11 | W4.3 remove duplicate visibility writers (code half) | S | — |

### 🟦 Spec-only UI/IA track
W3.2 five tabs · W3.3 inspector restructure · W3.5 optimizer affordances ·
W3.6 timeline · W3.7 header/rail · W4.1 provenance language *(after W2.1)* ·
W4.2 glyph roles · W4.4 inspector cards · W4.5 Z/spindle · W5.1 hidden actions ·
W5.2 spine discoverability · W5.3 prose cleanup · W5.4 mop-up.
Plus the UI halves of the 🟪 BOTH items (W0.1, W1.1, W3.1, W3.4, W4.3).

### Dependencies
- **W4.1** (provenance visual language) needs **W2.1** (data model) to be honest.
- **W3.1** feeds consolidation pairs with the **W3.1 code split**; don't ship the
  one-editor UI while Apply still bundles geometry.
- **W4.3** / **W4.4** sit naturally inside **W3.3** (inspector reorg) — batch them.
- **W1.1** should land before any "tool management" spec polish (no point styling a
  panel whose actions don't fire).

## Suggested first cut (one sprint, maximal harm reduction, minimal surface area)
W0.2 + W0.3 + W0.4 (three small correctness fixes) → W0.1 (the safety gap) →
W1.1 + W1.2 (restore reachable capability). All 🟥/🟪, no large UI reorg — buys
the biggest honesty/safety gain before the structural redesign begins.
