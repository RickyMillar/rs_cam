# Density & Progressive-Disclosure Pass — 2026-06-11

> **Charter:** owner's complaint, verbatim: *"right now there is a lot of information at
> once, probably way too much for the standard user."* This pass re-judges every
> surface in `crates/rs_cam_viz/src/ui/` against P3 (dig deeper, don't dump) and
> P5 (UI wins, not prose) from `planning/UI_IA_AUDIT_WORKFLOW.md` §0, plus two
> density lenses:
>
> - **D1 — default-visible load:** how much is on screen before any interaction.
> - **D2 — standard-vs-expert segregation:** is expert-grade data (provenance
>   internals, raw metric fractions, debug spans, sample indices) visible by
>   default to a user who just wants to cut wood?
>
> Findings already tracked in `planning/ui_audit/BACKLOG.md` are NOT re-reported
> unless the merged implementation still fails the density lens — those are
> flagged "BACKLOG W-x landed but still dense because…".
>
> Taste baseline (owner, 2026-06-10): the Setup tab lost its project-summary +
> findings rollups — *per-surface focus; whole-project readouts live elsewhere.*
>
> Read-only pass; no source was edited. Effort: S (hours) · M (a day or two) ·
> L (multi-day). **[QW]** = quick win: S effort, pure-disclosure, no logic change.

---

## One-screen summary

| Surface | Default-visible load (with data) | Verdict | Top action |
|---|---|---|---|
| `sim_diagnostics` (Inspector) | ~40–50 items: 3 sections default-open at once + status header | **WALL** | Only Project stays open; Selected-span + View default-closed |
| `properties/mod` Feeds & Speeds tab | ~25: feeds card + 4 always-on formula lines + diagram + LUT entry | **WALL** | Formula breakdown behind "Show the math" disclosure |
| `sim_timeline` (bottom panel) | transport + 6 pills + ribbon + **5 stacked metric tracks** | **DENSE** | Spine behind disclosure w/ 1-line worst-metric summary; drop `traces` pill when 0 |
| `sim_op_list` (Verification) | Setup&run block + per-op: up to ~12 status flags + 4 row buttons | **DENSE** | Collapse Setup&run after first run; roll per-op flags up to worst-of |
| `properties/mod` toolpath header/ribbon | name×2 + generate row + unbounded diagnostic rows w/ evidence lines | **DENSE** | Evidence lines → hover; cap rows; de-duplicate name |
| `toolpath_panel` (op queue) | ~13 elements/card × N cards (4 rows + 6 glyph buttons) | **DENSE** | Row-4 controls hover/selection-only; trace badge only when traces exist |
| `draw_model_properties` | ~20 read-only rows incl. BREP adjacency, surface histogram | **DENSE** | Mesh/BREP stats behind "Details" disclosure |
| `feeds_modal` | modal (opt-in) but ~35 items: card + open derate chain + 3 charts | DENSE (in-modal) | "Why is the recommendation here?" default-closed |
| `properties/stock` | material + 2 expert rows + dims + origin + pins section open | CLEAN-ish | Pins default-open only when relevant; Kc/hardness → hover |
| `setup_panel` (rail) | card per setup: 2-4 chips + up to 2 italic warnings each | CLEAN-ish | "Starts from uncut stock" → icon + hover |
| `properties/setup` | 7 small flat groups | CLEAN | — |
| `properties/tool` | 8-row grid + preview; holder/catalog collapsed | CLEAN | — |
| `properties/post` | 3 fields + effective-Z line + 1 toggle | CLEAN | — |
| machine panel (`properties/mod`) | preset/library + 4 specs + 2 sliders + captions | CLEAN | — |
| `readiness_panel` | banner + 6 check rows + pills | CLEAN (model citizen) | — |
| `preflight` | 6 check cards + gated overrides | CLEAN | — |
| `export_wizard` | stepper, capped previews/scrolls | CLEAN | — |
| `optimize_modal` / `optimize_project` | narrative headline + ≤5-col tables, rest collapsed | CLEAN | — |
| `status_bar` / `workspace_bar` / `viewport_overlay` / `menu_bar` / `shortcuts_window` | minimal / menu-gated | CLEAN | — |
| heights tab / linking tab / dressup tab / op param forms | 4–9 fields + diagram each | CLEAN | — |
| components/* (`ValueRow`, `CountPill`, `FreshnessGate`, …) | n/a (substrate) | CLEAN | — |

---

## Per-surface findings (worst-first)

### 1. `sim_diagnostics.rs` — Inspector right panel · **WALL**

**Lens: D1, D2, P3.** BACKLOG **W3.3 landed but still dense because** the
"summary-first behind its own disclosure" sections all ship `default_open(true)`
simultaneously — the disclosure exists but is pre-sprung, so the panel renders
all three scopes plus View at once.

Evidence:
- `sim_diagnostics.rs:87` — `CollapsingHeader::new("View").default_open(true)`.
  Display settings (opacity slider, "Toolpaths" pointer prose, stock-color
  combo, generator-overlay toggles) are NEEDED-SOMETIMES at best; they sit
  fully expanded *below the separator on every frame*, even before a sim runs.
- `sim_diagnostics.rs:461` — Project section `default_open(true)`. Reasonable
  to keep (it is the standard user's verdict home), but its body stacks the
  Global 4-row grid, 4 CountPills, an optimize entry, a "Must address" grid,
  an "Informational" grid, *and* the Top-hotspots collapser.
- `sim_diagnostics.rs:741` — Toolpath section `default_open(now.is_some())`:
  fine (conditional).
- `sim_diagnostics.rs:1228` — Selected-span section `default_open(true)`. Body
  = lock toggle + span facts + a 5-row metrics grid (`avg/peak` chipload in mm,
  peak axial DOC, avg/peak MRR in mm³/s, sample counts, `sim_diagnostics.rs:1328-1373`)
  + up to 8 hotspot rows + up to 8 issue rows. **This is D2's poster child:**
  raw per-span aggregates are machinist-debug data, expanded by default for
  everyone, every frame the playhead is inside a span.
- Inside Project: the Global grid (`sim_diagnostics.rs:470-502`) carries
  "Moves: 91 234" and "Operations: 8" — Moves is an EXPERT datum (no standard
  user acts on a move count); Operations duplicates the left panel's op list.

Standard-user story: user opens Simulation to ask "is the cut OK?". The answer
is the banner + the within/exceeds pills (≤6 items). They must visually skip
~35 more: a metrics grid in raw mm/mm³, span facts, a lock toggle, two nested
drill-down headers, and a display-settings block.

Classification: status header + Findings pills = NEEDED-ALWAYS. Global grid =
NEEDED-SOMETIMES (cut/rapid distance) + EXPERT (Moves). Span metrics grid,
Generator item, Generation trace = EXPERT. View = NEEDED-SOMETIMES. "Operations"
row = NOISE (duplicated in left panel).

Fix shape:
- `View` → `default_open(false)`. **[QW]**
- `Selected:` → `default_open(locked_span_id.is_some())` — open only when the
  user explicitly locked a span; follow-the-playhead stays one click away. **[QW]**
- Drop "Moves"/"Operations" rows from the Global grid (Operations is in the
  left rail; Moves moves to the span/expert tier). S.
- Project section header already carries cycle + within/exceeds — good; keep.

Effort: S (the two flips), S for the grid trim. Mostly quick wins.

### 2. `properties/mod.rs` — Feeds & Speeds tab · **WALL**

**Lens: P3, P5, D2.**

Evidence:
- `properties/mod.rs:3299-3346` — when a feeds result exists, **four equation
  lines render unconditionally** ("Feed = RPM × flutes × chipload = …",
  "MRR = DOC × WOC × Feed = …", "Power = MRR × Kc / 60e6 = …", "Plunge = …"),
  annotated in-code as "always visible, the key teaching tool". Teaching prose
  is the definition of P5/D2 expert content shown by default. Below it, the
  engagement diagram (`:3349`) renders unconditionally too.
- The tab simultaneously offers: the modal entry button (`:3270`), the
  "Feeds & Speeds" collapsing card (`:1236`, closed — good) which itself holds
  SPEED + CUT + Derived sections + warnings, then the formula wall, the
  diagram, then the Vendor Cutting Data viewer (`:1545`, closed — good).
- `properties/mod.rs:1587` — the vendor LUT table is **7 columns** and
  unbounded rows (every observation for the tool family). It's behind a
  disclosure (good) but once opened it dumps the whole LUT; the
  current-diameter highlight is the only triage. (>5-column flag, D2.)

Standard-user story: user opens the tab to set feed/plunge. They get three
competing tiers at once (card, math wall, modal button) and must learn that
the collapsed card is where editing happens while the always-visible math is
read-only.

Fix shape:
- Wrap the formula breakdown + engagement diagram in
  `CollapsingHeader::new("Show the math").default_open(false)`. **[QW]**
- The feeds card should open by default instead (it's the actionable tier) —
  one `ui.collapsing` → `CollapsingHeader::default_open(true)` swap, paired
  with the math demotion so net default load *drops*. S.
- Vendor LUT viewer: default-filter rows to the current diameter ±25 % with a
  "show all N" toggle row. M (small logic).

### 3. `sim_timeline.rs` — bottom panel · **DENSE**

**Lens: D1, D2.** BACKLOG **W3.6 partially landed but still dense because** the
strip-disambiguation and click-contract fixes (TIM-005/006/009/010) merged, but
the "metric-resolved summary tier above the spine" never did — the spine is
still five co-equal expert tracks.

Evidence:
- `sim_timeline.rs:386-430` — `tracks: [SignalTrack; 5]` (chipload, arc
  engagement, axial DOC, MRR, feed) all render every frame in a 360 px-default
  panel (`app.rs:323-327`), each ~90 px tall in the scroll area
  (`sim_timeline.rs:470-495`). Arc engagement in radians and MRR in mm³/s are
  D2 expert signals; the standard user's question ("where is it in trouble?")
  is already answered by the HUD pills and the gate-trip dots.
- `sim_timeline.rs:199-203` — the verdict HUD renders a **"traces" pill even
  when the count is 0**. Generator-trace counts are debug-grade (D2); five
  other pills already compete for the same glance.
- `sim_timeline.rs:91-205` — HUD = 6 pills; `issues` pill counts air-cut/
  low-engagement emission noise (the project section now partitions these, the
  HUD doesn't — the same number the CLAUDE.md metric caveats call noise).

Standard-user story: user scrubs the timeline; below the ribbon five stacked
graphs consume a third of the screen even when every gate is green.

Fix shape:
- Wrap the spine in a CollapsingHeader ("Signal graphs (5)") default-closed
  when all load pills are green / no focused TP; auto-open on exceeds-pill
  click. M (interaction nuance), or S for an unconditional default-closed.
- `traces` pill: render only when `trace_count > 0`. **[QW]**
- `issues` pill: either drop (Project section partitions it properly) or count
  only must-address kinds. S.

### 4. `sim_op_list.rs` — Verification left panel · **DENSE**

**Lens: D1, D2, P3.**

Evidence:
- `sim_op_list.rs:39-118` — the "Setup & run" block (2 capture checkboxes,
  resolution slider, auto checkbox, grid warnings, Run button) renders expanded
  on every visit forever. Capture toggles + resolution are set-once recording
  options (NEEDED-SOMETIMES); only the Run button is NEEDED-ALWAYS.
- `sim_op_list.rs:894-983` — per-op status flags: up to **6 issue-kind flags**
  (`⚠ air×24800`-style counts included — air/low-eng are the documented
  emission-noise metrics) **plus up to 6 criterion flags**, including `≈ chip`
  for *approximate-but-within* and `? load` for unmodeled. A healthy-but-
  approximate op row can stack 4+ amber/grey glyphs. D2: per-criterion
  confidence states are expert; the standard user needs worst-of.
- Each op card also re-renders the shared row controls (eye/C/R/isolate,
  `sim_op_list.rs:317-326`) — 4 buttons per row × N ops.

Standard-user story: user opens Simulation to run a check; every op row shouts
a flag soup where one "✓ / ⚠ / ✗" would answer the question, and the recording
settings they configured once sit above the op list permanently.

Fix shape:
- "Setup & run" → CollapsingHeader, `default_open(sim.boundaries().is_empty())`
  (open until first run, Run button stays outside the collapse). **[QW]**
- Per-op flags: show worst-of (one glyph + count), full stack behind hover or
  the Inspector's Toolpath section (which already owns per-op load badges —
  current state is a P1 echo of that surface). M.
- Suppress `air`/`low-eng` count flags at row level (they remain in the
  Inspector's Informational partition). S.

### 5. `properties/mod.rs` — toolpath panel header + diagnostics ribbon · **DENSE**

**Lens: D2, P1, P3.** BACKLOG **W3.7 landed but still dense because** the
promoted ribbon renders every actionable + stateful diagnostic *with evidence
lines* inline, unbounded.

Evidence:
- `properties/mod.rs:2672-2682` — `ui.heading(&entry.name)` immediately
  followed by an editable `Name:` field holding the same string. Same value,
  two homes, four lines of panel height (micro-P1). 
- `properties/mod.rs:2801-2815` — `for d in &actionable` / `for d in &stateful`
  render unbounded rows (hints are collapsed — good). Each non-hint row also
  prints its evidence line (`:2385-2394`): "at samples 1234–5678: observed
  0.123 mm/tooth, threshold 0.1 [steady_state]" plus "(approximate)" confidence
  chips. Sample indices and locality tags are D2 expert payload; the message
  line alone carries the action.
- Geometry disclosure `default_open(entry.result.is_none())`
  (`properties/mod.rs:2823`) — **good** density decision; keep as the pattern
  to copy.

Fix shape:
- Evidence line → `on_hover_text` on the diagnostic row. **[QW]**
- Cap visible actionable rows at ~4 with "+N more" expander (mirror the span
  section's `MAX_ROWS` pattern at `sim_diagnostics.rs:1429`). S.
- Drop the duplicate heading (keep the editable field; or make the heading
  click-to-edit and drop the field). **[QW]**

### 6. `toolpath_panel.rs` — operation queue cards · **DENSE**

**Lens: D1, D2.** BACKLOG **SHE-006/W1.1 landed but still dense because** every
harvested affordance is now *always-on* per card.

Evidence:
- Per card: grip + swatch + status chip + optional `MAN` + trace badge
  (`toolpath_panel.rs:402-405`) + name (row 1); tool summary + rest badge +
  `Sim` + `▶` (row 2); stats line (row 3); **six glyph buttons** — eye, C, R,
  isolate, power, duplicate (row 4, `toolpath_row_controls.rs:18-175`). ≈13
  elements/card; an 8-op job renders ~100 interactive elements in the rail.
- The trace badge is generator-debug provenance (D2) on every card regardless
  of whether the user has ever recorded a trace.
- The C/R buttons gray correctly when the global toggle is off (W4.3 ✓), but
  they still occupy a full row per card even in the common all-defaults state.

Standard-user story: user scans the queue to find the op that failed; each card
offers 10+ touch targets when the scan needs only swatch + name + status.

Fix shape:
- Row-4 controls render only on hover or selection (egui: check
  `ui.rect_contains_pointer`/selected before drawing the row; reserve no space
  otherwise). M (layout shift care), or S to gate on `selected` only.
- Trace badge only when a trace exists or `sim.debug.enabled`. S.
- Stats line (row 3) is fine — it's the glanceable payoff.

### 7. `properties/mod.rs` — `draw_model_properties` · **DENSE**

**Lens: D2, P3.**

Evidence: `properties/mod.rs:559-764` — flat dump: type/path, mesh-info grid
(vertices, triangles), dimensions grid (each axis with `min..max` ranges),
size-hint warnings, winding-report warning, **BREP Topology grid (face count,
"Adjacency pairs", surface-type histogram)**, units/scale selector, custom
scale, polygon listing (first 5 + "and N more").

Classification: units/scale + dimensions = NEEDED-ALWAYS (acted on at import).
Mesh counts, BREP adjacency pairs, surface histogram, per-axis ranges = EXPERT.
Warnings = NEEDED-SOMETIMES (conditional — fine).

Fix shape: keep heading + dimensions (plain, no ranges) + units/scale +
warnings; move "Mesh Info" + "BREP Topology" + per-axis ranges + polygon list
under one `CollapsingHeader::new("Details").default_open(false)`. S. **[QW]**

### 8. `feeds_modal.rs` · DENSE — but it *is* the drill-in

**Lens: P3, D2.** A modal is already disclosure, so judged inside its own frame.

Evidence:
- `feeds_modal.rs:1097-1102` — "Why is the recommendation here?"
  `default_open(true)`: full derate chain (3-col grid of factors) + monospace
  formula `fz = K₀ · D^p · (1/H)^q` expanded on open. The left column then also
  shows provenance disclosure, warnings, and rationale; the right column three
  charts at once.
- `feeds_modal.rs:2768-2806` — project table is **9 columns** (>5-col flag);
  acceptable for a comparison table, but col-merge candidates exist (Δ could
  fold into Rec feed).

Standard-user story: user clicked "Open Feeds & Speeds modal" to compare and
apply. The compare card + Apply is the job; the derate chain answers a question
they haven't asked yet — and it's open.

Fix shape: `default_open(false)` on the breakdown (`:1102`) — the header text
is literally the question the user clicks when they have it. **[QW]** Charts
A/B could collapse behind "More charts" (M, lower priority — modal is opt-in).

### 9. `properties/stock.rs` · CLEAN-ish — two demotions

**Lens: D1, D2.**

- `stock.rs:181` — Alignment Pins `default_open(true)`. Pins matter only for
  two-sided work; for the standard single-sided job the section (flip-axis
  combo, two-sided button, caption) is permanent furniture. Fix:
  `default_open(!stock.alignment_pins.is_empty() || stock.flip_axis.is_some())`.
  **[QW]**
- `stock.rs:28-63` — "Hardness Index" + "Kc" read-only grid under the material
  picker: D2 expert (engine internals). Already muted/small, but two rows ×
  every visit. Fix: fold both into the material button's existing
  `on_hover_text` (`stock.rs:626-630`). **[QW]**

### 10. `setup_panel.rs` · CLEAN-ish — one warning-as-wallpaper

**Lens: P5, D1.** Post-cleanup rail is good (chips, cards, Models collapsed).
One regression-shaped item:

- `setup_panel.rs:218-225` — every non-first setup card prints the italic
  warning "Starts from uncut stock (prior setups not reflected)" *always*. A
  permanent, unconditional sentence is wallpaper, not signal (the same text on
  every Setup-2+ card forever). Fix: an `ⓘ`/`⚠` chip with the sentence on
  hover. **[QW]**
- Flip-instruction italic line (`:208-215`) is conditional on a real state —
  acceptable.

### 11. Cross-surface density notes (P1 survivors under the D-lens)

- **within/exceeds/unmodeled/collisions pills render twice simultaneously in
  the Simulation workspace** — verdict HUD (`sim_timeline.rs:147-198`) and
  Inspector Project section (`sim_diagnostics.rs:515-549`). W0.4 made them
  provably identical (same `summary()` producer) — correctness fixed, but the
  *pixels* are duplicated in one viewport. Candidate: HUD keeps the pills
  (timeline is where you navigate to the bad move); Inspector header line keeps
  its `✓N within · ✗N exceeding` text and drops the pill row from the body. S.
- **Collision count** is visible in up to 4 places at once in Simulation
  (status bar, workspace badge, HUD pill, Inspector pill) — all one source
  (post-W0.3), so this is tolerable redundancy across *chrome* tiers; no action
  beyond the Inspector consolidation above.
- **Engagement provenance hover** (`ENGAGEMENT_PROVENANCE_HOVER`,
  `sim_diagnostics.rs:803`) — good D2 pattern: caveat lives on hover, not
  inline. Cited as the pattern to copy for the diagnostics-ribbon evidence
  lines (finding 5).

---

## Top 10 quick wins

All S-effort, pure-disclosure/demotion, no logic change. File:line cited.

1. **Inspector "View" section default-closed** — `sim_diagnostics.rs:87`
   `default_open(true)` → `false`. Display settings out of the diagnostics glance.
2. **Inspector "Selected:" span section opens only when locked** —
   `sim_diagnostics.rs:1228` `default_open(true)` →
   `default_open(locked_span_id.is_some())`. Raw span metrics become opt-in.
3. **Feeds tab math wall behind "Show the math"** — wrap
   `properties/mod.rs:3299-3349` (4 formula lines + engagement diagram) in a
   default-closed CollapsingHeader.
4. **Feeds-modal derate chain default-closed** — `feeds_modal.rs:1102`
   `default_open(true)` → `false` ("Why is the recommendation here?" is the
   click, not the default).
5. **Alignment Pins collapse when irrelevant** — `stock.rs:181` →
   `default_open(!pins.is_empty() || flip_axis.is_some())`.
6. **HUD "traces" pill only when non-zero** — `sim_timeline.rs:199-203` gate on
   `trace_count > 0`.
7. **"Setup & run" collapses after first run** — `sim_op_list.rs:39-118` into a
   CollapsingHeader `default_open(sim.boundaries().is_empty())`, Run button
   stays outside.
8. **Diagnostic evidence lines → hover** — `properties/mod.rs:2385-2394`: move
   `evidence_line` text to `on_hover_text` on the row (copy the
   `ENGAGEMENT_PROVENANCE_HOVER` pattern).
9. **Model properties "Details" disclosure** — `properties/mod.rs:565-695`:
   Mesh Info + BREP Topology + per-axis ranges + polygon list under one
   default-closed header; dimensions + units stay.
10. **De-duplicate toolpath name** — `properties/mod.rs:2672` heading vs
    `:2678` Name field show the same string; keep one.

Honourable mentions (same class): setup-card "Starts from uncut stock" → hover
chip (`setup_panel.rs:218`); Kc/Hardness grid → material-button hover
(`stock.rs:28-63`); trace badge on queue cards only when a trace exists
(`toolpath_panel.rs:402`).

---

## Already clean — surfaces that pass the density lens

Audited and explicitly passing; no action proposed:

- **`readiness_panel.rs`** — the model citizen: one banner, six rows, pills
  with denominators, one primary action. This is the per-surface-focus taste
  baseline realised.
- **`preflight.rs`** — same grammar as readiness; overrides appear only when
  their refusal class exists; confirm gating is proportionate.
- **`export_wizard.rs`** — stepper is progressive disclosure by construction;
  G-code preview capped (`PREVIEW_LINE_LIMIT`), findings in bounded scroll
  areas, per-step content ≤ ~10 controls.
- **`optimize_modal.rs` / `optimize_project.rs`** — post-W3.5 shape is right:
  narrative headline, baseline card, ≤5-column tables, "Not optimized (N)"
  collapsed (`optimize_project.rs:208-212`), refusals get prose only where a
  refusal *is* the content.
- **`workspace_bar.rs` / `status_bar.rs`** — one-line chrome; badges
  conditional; collision chip greys on stale with provenance on hover.
- **`viewport_overlay.rs`** — everything behind View/Show/Shaded menus; the
  SpanKind filter (expert) is two menus deep, exactly where it belongs.
- **`menu_bar.rs` / `shortcuts_window.rs` / `automation.rs` / `sim_debug.rs`**
  — menus and a help window; debug helpers render nothing by default.
- **`properties/setup.rs`** (setup/fixture/keep-out editors) — flat but small
  groups, one concern each; empty states are single muted lines.
- **`properties/tool.rs`** — main grid ~8 rows, holder + catalog metadata
  collapsed, safety state promoted onto the collapsed header (TOO-006 pattern:
  *header carries the warning, body carries the fields* — worth reusing).
- **`properties/post.rs`** — 3 fields, one conditional sub-grid, effective-Z
  line is a legitimate computed readout.
- **Machine panel** (`properties/mod.rs:1006-1148`) — presets/library, 4 specs,
  2 captioned sliders; the captions are short state descriptions, not crutches.
- **Heights tab** (`operations/mod.rs:125-188`) — 5 reference rows + resolved-Z
  hints + diagram; the dim `= Z` hint is the right way to show derived values.
- **Linking / Dressup tabs** (`properties/mod.rs:3616-3909`) — sub-fields only
  appear when their feature is enabled; incompatible controls grey with reason
  on hover.
- **Per-op param forms** (`operations/boundary_2d.rs`, `surface_3d.rs`,
  `drill.rs`, `finishing.rs`, `engrave.rs`, `project.rs`) — 4–9 fields each in
  2-col grids with ⚡ pills; diagrams below. Right altitude.
- **`tool_library_modal.rs`** — catalog/detail split, edit form gated behind
  Edit, read-only grid bounded.
- **Component layer** (`components/*`) — `ValueRow`, `SuggestButton`,
  `ProvenanceBadge`, `PrecedenceField`, `CountPill`, `FreshnessGate`,
  `Section` — these are the *tools* of density discipline; the failures above
  are surfaces not yet using the disclosure those components afford.

---

## Method note

Counts are code-led (egui immediate-mode: items inside `CollapsingHeader`
closures with `default_open(false)`, menus, modals, and `on_hover_text` were
scored as *behind disclosure*; everything in the always-run path as
*default-visible*). Every `default_open(true)` in the tree was located via
`rg default_open` and individually challenged; the full inventory is:
`stock.rs:181`, `setup_panel.rs:67(false)`, `optimize_project.rs:212/478(false)`,
`properties/mod.rs:1546(false)/2809(false)/2823(conditional ✓)`,
`feeds_modal.rs:1102(true ✗)`, `sim_diagnostics.rs:87/461/1228(true ✗✗△),
682/1510/1585(false)/741(conditional ✓)`, `toolpath_panel.rs:180(false)`.
