# Redesign SPEC — Simulation & diagnostics legibility

Domain owner findings: **P4-001, P4-002, P4-004, P4-005, P5-001, P5-003, P5-004, P6-002, P6-004.**

This is a SPEC (no Rust edits). It defines target homes, depth tiers, confusable
disambiguation, and retirements for the simulation diagnostic surfaces:

- **Verdict HUD** (`sim_timeline.rs::draw_verdict_hud`)
- **Boundary timeline** (`sim_timeline.rs::draw_boundary_timeline`)
- **Inspector › Project overview** (`sim_diagnostics.rs`, `cut_overview_*`)
- **Inspector › View** (`sim_diagnostics.rs`, lines 50–153)
- **Viewport overlay › Show ▼** (`viewport_overlay.rs`)
- **Operations Queue row controls** (`toolpath_row_controls.rs`, the `C`/`R` buttons)

---

## 1. One concern, one number — the load rollup (P4-001, P4-002)

### Problem recap
Two project rollups count the *same* load concept with different denominators.
The Project overview was migrated to `ToolLoadReport::summary()` (folds
**per-toolpath**); the Verdict HUD still calls `verdict_counts()` which folds
**per-criterion** (up to 3× per TP). So "exceeds 3" (HUD) and "TPs exceeding 1"
(overview) describe the same project and can never be reconciled. The HUD's
`✓ load {ok}` pill even hovers "Toolpaths within modeled load limits" while
`{ok}` is a per-criterion count (P4-002 — a mislabeled safety metric).

### Target: one canonical number, ONE producer
- **Authoritative producer:** `ToolLoadReport::summary(name_resolver)` →
  `{ total_toolpaths, within, exceeds, fully_unmodeled, not_applicable,
  exceeds_breakdown }`. This is already per-toolpath and already carries a
  resolved name per exceed. It becomes the **single** source for every load
  rollup pill or row.
- **Verdict HUD** stops calling `verdict_counts()`. The function is retired
  (it is the per-criterion folder and has no other caller). The HUD reads
  `summary()` — the identical struct the overview reads — so "within / exceeds /
  unmodeled" are byte-identical across both surfaces.
- **Denominator is shown, not implied.** Every load pill renders as
  `within N/T`, `exceeds N/T`, `unmodeled N/T` where `T = total_toolpaths`.
  The shared `/T` is the visible proof both surfaces count the same thing
  (this is the UI-wins replacement for the mislabeled tooltip — P4-002 is fixed
  by making the unit legible in the glyph, not by rewriting prose).
- **`not_applicable`** (drill/pin ops the gate can't model) is split out of
  `unmodeled` so the HUD no longer inflates the amber count with ops that need
  no action — `summary()` already separates it.

Depth tiers (P3):
- **Summary (HUD + overview top):** `within N/T · exceeds N/T · unmodeled N/T`.
- **Drill (click the `exceeds` pill):** opens the per-exceed list from
  `exceeds_breakdown` (toolpath name + gate + low/high) — see §2.

---

## 2. Count pills become point-of-need actions (P5-001, P6-004, P5-003)

### Problem recap
The HUD pills are plain `ui.label` with a hover only (no `Sense::click`, no
events). `draw_verdict_hud` even threads `_events: &mut Vec<AppEvent>` — an
unused sink (P6-004), dead plumbing that *looks* like the HUD could emit
actions. The tooltips redirect the user to "click the red lines on the boundary
timeline below to navigate" — prose pointing at a different widget (P5-001).

### Target: each actionable pill IS the action
- The unused `_events` sink is **wired** — `draw_verdict_hud` takes
  `events: &mut Vec<AppEvent>` and writes to it. (Retires P6-004's dead
  plumbing by giving it the job it was plumbed for.)
- `info_pill` gains a clicked variant (`Sense::click`, button-like hover cursor,
  subtle hover-fill) used for the **actionable** pills; the static
  `info_pill` stays for genuinely read-only counts. The two roles look distinct
  (P4 — see §5).
- **Actionable pills and their action:**
  - `exceeds N/T` → emits `SimJumpToMove(first exceed marker)` via the existing
    `nearest_safety_marker_move` helper, and sets `analytics_tab = Safety`.
    Clicking the *count* jumps to the *first* exceed; the drill list (from
    `exceeds_breakdown`) lets the user pick a specific one.
  - `collisions N` → `SimJumpToMove(first collision marker)`. Zero-count pill is
    inert/static (nothing to jump to).
  - `issues N` → `SimJumpToMove(first issue)` (issue list already sortable in
    overview).
- **Tooltip prose is removed**, not reworded. The pill's affordance (clickable,
  cursor change, "→" trailing affordance on hover) replaces the
  "click the red lines below" sentence (P5 — UI wins). A short hover label
  remains only as the metric's *name* ("Load-limit exceedances"), never as a
  navigation instruction.
- **P5-003** (empty-state card prose "Use Run Simulation above…") is resolved by
  the same principle applied narrowly: the empty-state card gets an inline
  **Run Simulation** affordance (mirrors the sibling "Go to Toolpaths" button)
  rather than a sentence pointing one separator up. Lowest priority in the set;
  one button, not a layout change.

Depth tiers (P3):
- **Summary:** the pill row. Glance = counts; click = jump.
- **Drill:** `exceeds` pill opens the `exceeds_breakdown` list (each row a
  clickable jump to that gate's marker). Read-only counts (traces) never gain a
  click affordance.

---

## 3. Viewport visibility — one home, consistent labels (P4-005)

### Problem recap
`show_stock` / `show_cutting` / `show_rapids` are written from BOTH Inspector ›
View ("Show stock" / "Show cutting moves" / "Show rapid moves") and the
Viewport overlay Show ▼ menu ("Stock" / "Paths (cutting)" / "Rapids"). Same
backing fields, two homes, two label sets — the user can't tell it's the
control they already changed.

### Target: ONE authoritative home + an explicit mirror, ONE label set
- **Authoritative home:** the **Viewport overlay › Show ▼** menu. It sits on the
  viewport these toggles affect (point-of-need), already groups all viewport
  visibility (stock, fixtures, polygons, cutting, rapids, collisions), and is
  reachable from every workspace. Visibility *of the 3D scene* belongs with the
  3D scene.
- **Inspector › View** keeps `Stock` opacity, `Stock color` (Analysis), and the
  generator-step overlay — these are *analysis* controls, not raw visibility,
  and have no second home. The three duplicated visibility checkboxes
  (`show_stock`/`show_cutting`/`show_rapids`) are **removed from Inspector ›
  View**; one concern, one home (P4-005). Opacity stays with stock-color because
  both are stock-*appearance*, distinct from stock-*presence* (a toggle that
  lives in the overlay).
- **Single label set** (used wherever a label for these fields appears,
  including per-toolpath hovers): **"Stock" / "Cutting moves" / "Rapid moves"**.
  "Paths (cutting)" and "Show cutting moves" both collapse to "Cutting moves".

### Global vs per-toolpath layering (P4-004)
`viewport.show_cutting` (global) AND-gates per-toolpath
`entry.show_cutting` (the `C`/`R` row buttons). Toggling one while the other
gates produces "I toggled it and nothing happened". This is a legitimate
hierarchy, not a duplicate, so it is **kept but made legible**:
- Roles already look distinct (labeled checkbox vs single-letter `C`/`R`
  button) — preserved.
- The per-row `C`/`R` buttons render **disabled/greyed when the global toggle is
  off**, with the global toggle as the gate. A greyed `C` says, by affordance,
  "the global Cutting toggle is off — this per-row override has no effect right
  now." No prose; the disabled state is the explanation (P5). This kills the
  "toggled and nothing happened" surprise that P4-004 names.

---

## 4. Dead stock-color mode (P6-002)

### Problem recap
`StockVizMode::ByOperation` has a live GPU render branch
(`gpu_upload.rs:83 → operation_placeholder_colors`) but the combobox maps it to
"Solid" and no `selectable_value` offers it; the enum has no serde derive so it
can't enter via load. Dead code masquerading as a Solid fallback.

### Decision: **WIRE it** (cheap, the render path already exists)
The GPU branch is real and the mode is genuinely useful (color stock faces by
which operation last cut them — a direct legibility win for the diagnostics
domain). Retiring the render code would discard working capability; wiring is a
combobox entry + serde derive.
- Add `ByOperation` as a real `selectable_value` ("By Operation", hover:
  "Color stock by which operation last cut each region.") in the Inspector ›
  View `stock_viz_mode` combo (Analysis group — the one View control that stays).
- Add `#[derive(Serialize, Deserialize)]` coverage so it round-trips through
  project load (matching the other variants).
- Remove the `// placeholder: treated as Solid` arm; `selected_text` returns its
  own name.

*(Note: this is the ONE finding in the domain where wiring beats retiring —
the alternative, deleting `operation_placeholder_colors` + the GPU arm, throws
away a shipped render path for no IA benefit. Recorded here so the reviewer sees
the choice was deliberate.)*

---

## 5. Provenance & distinct-role visual language (P4, P7)

Provenance (P7) is largely a feeds-domain concern; in this domain the only
"where did this come from" signal is the **load verdict source** (vendor-LUT
gate vs sim-feedback collision/issue). One consistent treatment:

- **Load/verdict pills** (within/exceeds/unmodeled) — derived from the vendor-LUT
  tool-load gate — keep the verdict color ramp (green/red/amber). This is the
  domain's one provenance family and reads consistently across HUD + overview
  because both now read `summary()`.
- **Sim-feedback pills** (collisions, issues, traces) are visually a *different
  family* — they are run-observations, not gate verdicts. They get a distinct
  neutral/observation treatment (no green/amber/red verdict ramp; collisions use
  a binary safe-green / alarm-red only). This stops a reader from reading
  "issues 40" as a *load* verdict.

Distinct roles made obviously separate (the explicit user call-out):

| Confusable today | New distinct treatment |
|---|---|
| Actionable count pill vs read-only count pill | Actionable: clickable, hover-fill, trailing `→` on hover. Read-only: flat, no cursor change, no `→`. |
| Same-state two-label visibility (View vs overlay) | Removed from View entirely; ONE home, ONE label set "Stock / Cutting moves / Rapid moves" (§3). |
| Global vs per-toolpath visibility | Checkbox (global) vs single-letter `C`/`R` button (per-row); per-row greys out when global is off (§3). |
| Load verdict pill vs sim-observation pill | Verdict color ramp vs neutral observation treatment (above). |

---

## 6. Retirements (P6)

| Item | Treatment | Reason |
|---|---|---|
| `verdict_counts()` per-criterion folder (`sim_timeline.rs:168-191`) | retire | Sole cause of the irreconcilable HUD vs overview numbers (P4-001/002); no other caller. Replaced by `summary()`. |
| HUD pill prose tooltips ("click the red lines… below") | retire | Replaced by clickable pills (P5-001). Hover keeps the metric *name* only. |
| `_events` unused sink in `draw_verdict_hud` (`:89`) | fix-wiring | Plumbed-but-dead (P6-004); now carries the pill-click jump events. |
| Inspector › View `show_stock`/`show_cutting`/`show_rapids` checkboxes | relocate | Duplicate write path (P4-005); authoritative home is the overlay Show ▼. |
| `StockVizMode::ByOperation` placeholder mapping (`:100`) | redesign-affordance | Wired as a real combo entry + serde (P6-002), not retired. |
| Empty-state card "Use Run Simulation above…" prose | redesign-affordance | Inline Run button (P5-003). |

---

## 7. Capability home map (cross-ref to migration_rows)

- `read-load-findings` → Inspector › Project overview (authoritative) + Verdict
  HUD (mirror), both fed by `summary()`.
- `read-collision-count`, `read-issue-counts` → actionable HUD pills + overview.
- `show-stock`, `show-cutting-moves`, `show-rapid-moves` → Viewport overlay
  (authoritative home); removed from Inspector › View.
- `stock-color-mode` → Inspector › View Analysis group; gains `ByOperation`.
- `toggle-cut-move-visibility`, `toggle-rapid-move-visibility` (per-row C/R) →
  Operations Queue row, kept, greyed when global gate off.
- `run-simulation` → unchanged authoritative homes; empty-state card gets an
  inline mirror affordance.
- `jump-to-safety-marker` / `jump-to-move` → now also reachable from HUD pills.
