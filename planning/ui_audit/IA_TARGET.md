# rs_cam_viz — Target Information Architecture (unified)

This is the unified target IA assembled from the four domain deep-dives
(`redesign/feeds-speeds.md`, `redesign/toolpath-props.md`,
`redesign/sim-diagnostics.md`, `redesign/shell-safety.md`). It is a SPEC — no
Rust. ASCII layouts live in `MOCKUPS.md`; the per-capability home map lives in
`MIGRATION.md`.

It satisfies the seven redesign principles: P1 one concern / one home, P2 group
by concern, P3 dig deeper not dump, P4 distinct roles look distinct, P5 UI wins
not prose, P6 every control earns its place, P7 provenance is legible.

---

## New IA at a glance

The product keeps its three-workspace shell (Toolpaths / Simulation / shared
panels) but re-charters the surfaces so each capability has exactly ONE
authoritative home.

**Per-toolpath properties** — the tab bar changes from the flat
`[Params][Feeds][Heights][Dressups]` catch-all to **five concern tabs**, each a
*summary tier + progressive disclosure* surface:

| Tab | Owns the one concern of … |
|-----|---------------------------|
| **Geometry** | *what to cut* — pattern/strategy, cut direction, side, the cut-geometry trio (stepover / depth-per-pass / total depth), every op-specific shape param (inlay fit, drill cycle, scallop, slope/threshold, pencil, chamfer, project-curve), and the **machining boundary** |
| **Feeds & Speeds** | *how fast* — feed, plunge, the spindle-RPM precedence widget; recipe read-out, vendor LUT evidence, what-if/optimizer entry points (all behind disclosure) |
| **Linking** | *how moves connect* — entry style + ramp/helix, lead-in/out, link moves, retract strategy, arc-fit, rapid-order optimize |
| **Heights** | *Z reference planes* — the 5 planes + the effective-safe-Z read-only mirror |
| **Dressup** | *cosmetic / post edge work* — holding tabs, dogbone, feed smoothing, path-quality tolerance |

**Other authoritative homes (unchanged or consolidated):**

- **Operations Queue panel** — toolpath list ops + per-row C/R move-visibility buttons.
- **Viewport overlay › Show ▼** — the *sole* home for 3D visibility toggles (stock / fixtures / curves / cutting / rapids / collisions).
- **Inspector › View** — stock *appearance* (opacity, color-mode) + generator overlays only; the duplicate visibility checkboxes are removed.
- **Verdict HUD** — read-only mirror of the load rollup + actionable sim-run pills; fed by `ToolLoadReport::summary()` (same producer the overview reads).
- **Inspector › Project overview** — authoritative load/issue rollup.
- **Transport / scrubber, Boundary timeline** — playback + marker navigation.
- **setup / stock / fixture / keep-out / tool / tool-library panels** — setup-domain editors (largely unchanged; fixture and two-sided get wiring + relabel fixes).
- **post-processor-panel** — post/output settings; now a mirror of the canonical `session.post_config()`.
- **export-wizard** — staged export, including the per-export safe-Z override (distinct from project Safe-Z).

The four feeds editors collapse to one (the Feeds & Speeds tab); the feeds modal
becomes a read-mostly **Details drawer**; the legacy feeds-card and duplicate
Suggest-all button are retired.

---

## Cross-cutting visual language #1 — provenance (P7)

ONE provenance vocabulary, defined once, rendered identically on every surface
that shows a value-with-origin (feeds summary, Details drawer, optimizer modal,
nomogram explorer, project rollup, geometry Suggest pills, spindle widget, Z
mirrors). The chip is a **status badge — never a button that applies**.

| Source | Glyph | Single canonical color | Used for |
|--------|-------|------------------------|----------|
| Vendor LUT | `▣` | green `(80,180,80)` | value from the embedded vendor cutting table (carries obs-id) |
| Sim optimizer | `◆` | blue `(90,150,220)` | value from the sim optimizer (candidate id) |
| Nomogram what-if | `◷` | violet `(170,120,210)` | value from the what-if explorer |
| Hand edit / override | `✎` | grey `(150,150,150)` | value typed by the user |
| Formula fallback / floor | `▲` | amber `(220,180,60)` | derived with no vendor row |
| Project default (inherited) | hollow `< >` outline | — | inherited project value / read-only mirror |

Rules enforced everywhere:
- **One RGB per source** (kills P7-003's three greens / two blues / two ambers).
- **Per-field origin** — each value carries its *own* stamp; one field's chip
  never stands in for another's (kills P7-002's single-enum mis-attribution).
- **Distinguishable after apply, forever** — a sim-validated value reads
  `◆`, a raw LUT value `▣`, a what-if `◷`, a typed value `✎` (kills P7-004).
- **Shallow cue on summary, full derate one click behind it** — click any chip
  to open "Why this value?" in the Details drawer (kills P7-005's modal-only depth).

> ### ⚠ Requires non-UI change (data model)
> Today `OperationConfig` stores feeds as bare `f64`/`u32` with **no origin
> field**, and the displayed color is recomputed from a fresh LUT lookup, not
> from what produced the stored value. The UI cannot show legible provenance
> until each applied value carries a stamp:
> ```
> ValueProvenance = { source ∈ {VendorLUT, SimOptimizer, Nomogram, HandEdit, Default},
>                     ref (obs-id | candidate-id | ""), when (timestamp/generation) }
> ```
> `ChiploadSource` is overloaded as the provenance label for four independently
> derived fields and cannot be reused as-is. Flagged to core as `fix-wiring` on
> the affected `set-*` capabilities.

---

## Cross-cutting visual language #2 — confusable disambiguation (P4)

Similar-but-distinct controls get OBVIOUSLY separate visual roles; same-state
controls get ONE consistent label/treatment.

**Three roles for the three things that used to all be "⚡":**

| Role | Treatment |
|------|-----------|
| Apply ONE field | value-bearing chip showing the target, e.g. `‹↑ 1400›` or `⚡ ( vendor 1234 )` — single-field |
| Apply WHOLE recipe | doubled-glyph labeled button `⚡⚡ Suggest all` / `[ Apply recommended speeds ]` — never confusable with `⚡` |
| "Where did this come from" | a provenance chip (above) — a status badge, **not** clickable-to-apply |

**Actionable vs read-only count pills (sim):**

| Role | Treatment |
|------|-----------|
| Actionable | rounded, hover-fill, trailing `→` on hover, jumps on click |
| Read-only | flat, no cursor change, no `→`, glance only |
| Verdict family (load) | green/red/amber verdict ramp |
| Observation family (collisions/issues/traces) | neutral palette, never the verdict ramp, so "issues 41" can't read as a load verdict |

**Read-only mirrors everywhere** use the hollow `< project >` pill + `→ edit at its home` link — and nowhere else, so a mirror always reads as a mirror.

**Other resolved collisions:**
- `⚡` single-field Suggest ≠ `⚡⚡` Suggest-all (glyph doubled).
- Post "Project spindle default" (bare field) ≠ Feeds two-state precedence widget.
- "Peck retract (R-plane)" (drill cycle) ≠ "Clearance/Retract" Z planes (Heights).
- "Scallop height" (Scallop op) ≠ "Finish scallop limit (ball-tip)" (DropCutter override).
- Fixture panel (🛡 Z fields + clearance-check pill) ≠ keep-out panel (plain XY form).
- "Delete Selected" enabled-by-selection + wired ≠ a permanently greyed dead item.
- Both two-sided triggers use one identical `＋ Two-sided setup` label + `Stock ▸` chip.

---

## Concern group: Geometry — *what to cut*

Authoritative home: the **Geometry tab**. Absorbs every "what-to-cut" field from
the retired Params tab plus the machining boundary from the old Dressups tab.

Tiering (P3):
- **Summary (always open):** the op's defining shape controls + the cut-geometry
  trio (stepover / depth-per-pass / total depth), each with an inline `⚡` Suggest
  pill and provenance chip; plus the **live pattern minimap** pinned at the
  bottom as the what-you-cut affordance.
- **▸ Advanced (collapsed):** tolerance, min-cut-radius, finishing passes,
  stock-to-leave, stock-offset, slot-clearing, sampling resolution, region
  ordering, fine-stepdown, mill-shallow, chamfer tip-offset, project-side,
  cutter-compensation, inlay flat-tool-radius, pencil bitangency, point-spacing,
  DropCutter "Finish scallop limit (ball-tip)".
- **Named sub-sections (collapsed):** Inlay Fit (the 5 fit params), Drill cycle
  (cycle / peck depth / dwell / chip-break retract / **Peck retract (R-plane)** /
  pin spoilboard penetration), Rest machining, Machining boundary (enable /
  inherit / source / containment / offset).

Key fixes folded in:
- **P2-004** — drill "Retract Z" relocates here as **"Peck retract (R-plane)"**
  inside Drill cycle; its apology tooltip is deleted (label + grouping carry the
  meaning). It is a cutting-cycle param, not a clearance plane.
- **P1-007** — DropCutter's scallop override is relabelled "Finish scallop limit
  (ball-tip)" so it stops colliding at a glance with the Scallop op's geometry.
- **P3-001/002/003** — adaptive3d's 20-control grid and the 24 flat
  `draw_*_params` all adopt the Summary + named-section grammar the codebase
  already uses in `draw_dressup_params`.
- Boundary moves out of "Dressups" — it is what-to-cut, not edge work.

---

## Concern group: Feeds & Speeds — *how fast*

Authoritative home: the **Feeds & Speeds tab** (today `toolpath-tab-feed-params`,
the audit's P1 winner). It is the ONE editor for feed / plunge / RPM. Everything
else is a read-only mirror or drill-down.

**The headline split (P2-002):** group B is visually two sub-blocks —

```
SPEED  feed · plunge · RPM        ← "how fast"
CUT    DOC · WOC · stock-to-leave ← "how deep/wide" (geometry; editable home is the Geometry tab)
```

- `Apply recommended speeds` / `⚡⚡ Suggest all` applies **SPEED only** by
  default — it never silently rewrites geometry (today
  `apply_feeds_result_to_op` bundles `set_stepover`+`set_depth_per_pass`).
- A recommendation that wants to change DOC/WOC surfaces it as an **explicit,
  separately-confirmable, separately-attributed** geometry apply, marked
  `· changes cut`, with a distinct visual role from speed pills.

Tiering (P3):
- **Summary (always open):** feed / plunge editable rows + the two-state spindle
  precedence widget; each with provenance chip and single-field Suggest pill;
  one `Apply recommended speeds` button; one Power micro-bar; a one-line recipe
  summary chip; a `Details ▸` disclosure.
- **Derived sub-group:** chipload / power / MRR render as **plain text + chip,
  no apply affordance** (P4-007 — read-only rows obviously distinct from editable).
- **▸ Details drawer** (replaces the modal + Feeds-tab body, opens in place, no
  Apply rows — kills the second editor): Current-vs-recommended table,
  "Why this value?" derate breakdown, Charts (engagement / feed-vs-RPM /
  vendor band), What-if explorer (Apply stamps `◷ nomogram`, SPEED-only),
  Raw vendor LUT (the one canonical table mirrored read-only).

**Spindle precedence (P1-005/P2-006)** — one two-state widget showing BOTH the
project default (hollow inherited pill) and the per-op override, with which one
wins. The Post panel keeps the editable project default (relabelled "Project
spindle default") plus a read-only `→ per-op override on Feeds tab` link.

Surfaces consolidated:
- `feeds-card` (legacy) — **retired** (functional duplicate).
- `toolpath-tab-suggest-all` — **retired** into the one Apply-all.
- `feeds-modal-toolpath` — **redesigned** into the read-mostly Details drawer.
- `feeds-modal-nomogram-explore`, `optimize-modal`, `feeds-modal-project`,
  `vendor-lut-viewer` — **kept** as distinct-engine drills/rollups, now attributed.

---

## Concern group: Linking — *how moves connect*

Authoritative home: the **Linking tab** (split out of the old Dressups tab).

- **Entry & Exit:** entry style (one home — P2-003), ramp angle, conditional
  Helix radius/pitch (revealed when Entry = Helix), lead-in/out.
- **Move optimization:** link moves, retract strategy, arc-fitting (G2/G3),
  optimize rapid order.

For adaptive3d the entry-style control reads/writes `Adaptive3dConfig` directly;
the generic `DressupConfig` entry-style combo (live-but-inert no-op) is **retired**,
so no op ever shows two entry-style controls.

`reset-dressups-to-recommended` becomes a role-scoped reset spanning Linking +
Dressup (applies `DressupConfig::for_role` defaults across both new tabs).

---

## Concern group: Dressup — *cosmetic / post edge work*

Authoritative home: the **Dressup tab** (the cosmetic remainder of old Dressups).

- **Holding tabs** section: tab count / width / height (relocated from Params —
  they are cosmetic/post edge work, not pure geometry).
- Dogbone overcuts.
- **Path quality:** feed-rate optimization smoothing.

---

## Concern group: Heights — *Z reference planes*

Authoritative home: the **Heights tab** — already well-grouped; keeps the grid +
draggable side-view diagram as co-equal editors of the same 5 planes.

New responsibility (P2-004): a read-only **Effective safe-Z mirror row**
(`< project 6.0 > → edit in Post`) shows the post-clamp value at the point the
user edits clearance planes. The Post Safe-Z apology tooltip is deleted; the
mirror carries the meaning. Every distinct Z concept now has ONE editable home:
the 5 planes (Heights), the drill R-plane (Geometry › Drill cycle), the project
Safe-Z (Post), the per-export override (Export wizard).

---

## Concern group: Simulation & diagnostics

Authoritative producer for the load rollup: **`ToolLoadReport::summary()`**
(per-toolpath). Both the Verdict HUD and the Inspector › Project overview read it,
so within/exceeds/unmodeled are byte-identical across both surfaces (P4-001). The
per-criterion `verdict_counts()` folder is **retired** (sole cause of the
irreconcilable numbers; no other caller).

- **Visible `/T` denominator** on every load pill is the proof both surfaces
  count the same thing; it replaces the mislabeled safety tooltip (P4-002).
  `not_applicable` (drill/pin ops) is split out of `unmodeled`.
- **Count pills become point-of-need actions (P5-001/P6-004):** the dead
  `_events` sink in `draw_verdict_hud` is wired; `exceeds` / `collisions` /
  `issues` pills emit `SimJumpToMove` to the relevant marker (and `exceeds`
  opens the `exceeds_breakdown` drill). The "click the red lines below" redirect
  prose is removed; the pill's clickability is the affordance.
- **Read-only pills stay read-only** (traces) — explicitly NOT given a click, so
  actionable vs observation roles stay distinct (P4).
- **Visibility one home (P4-005):** stock / cutting / rapids collapse to the
  Viewport overlay Show ▼ with one label set ("Stock / Cutting moves / Rapid
  moves"); the duplicate Inspector › View checkboxes are removed. Stock opacity
  + color-mode stay in Inspector › View as *appearance* controls.
- **Global vs per-row layering kept but legible (P4-004):** the per-row C/R
  buttons grey out when the global Cutting/Rapid toggle is off — the disabled
  state explains why a toggle had no effect, no prose.
- **Point-of-need Run (P5-003/P5-004):** the empty-state card and the
  Deviation-no-data prompt get inline Run / Re-run buttons.
- **`StockVizMode::ByOperation` wired (P6-002):** real combobox entry + serde
  derive; the GPU branch already exists, so wiring beats retiring.

---

## Concern group: Shell, safety & cross-cutting

- **Fixture Z reaches collision checking (P6-003) — the highest-risk fix.** The
  Z fields are made real, not amputated. The fixture panel gains a per-fixture
  **clearance-check status pill** (`◇ not checked` / `✓ clear` / `✗ hit`), a 🛡
  shield marker on the Z fields, and a point-of-need `Run holder clearance ▸`
  mirror of the menu command. The pill is read from the *actual last collision
  result keyed by fixture_id* (P7 discipline); editing a 🛡 field resets it to
  `◇ not checked` — never falsely green.

  > ### ⚠ Requires non-UI change (data model + compute)
  > `CollisionCheckRequest` gains an `obstacles: &[CollisionObstacle]` slot built
  > from each enabled fixture's `origin_z` / `size_z` / `clearance` (clearance
  > inflates all six faces). `run_collision_check` tests the holder/shank
  > assembly against those boxes and reports `CollisionKind::Fixture { fixture_id }`.
  > Both call sites (`session/compute.rs`, `worker/helpers.rs`) populate it.
  > Without this the Z fields remain false assurance.

- **Edit › Delete Selected wired (P6-001).**

  > ### ⚠ Requires non-UI change (small)
  > Replace the literal `add_enabled(false)` with selection-derived enablement +
  > a click handler pushing the existing `RemoveToolpath(id)` (mirrors the
  > working keyboard path). No new event/data model.

- **Post config one source of truth (P1-004).**

  > ### ⚠ Requires non-UI change (event routing)
  > `session.post_config()` becomes canonical; `gui.post` demotes to a mirror.
  > Every post-panel edit routes through a `SetPostConfig`-style event that
  > writes the session immediately (as the wizard already does), closing the
  > GUI-vs-MCP staleness window. No visible layout change.

- **Two-sided trigger relabel (P2-005, spec-only):** both triggers (Setup panel
  + Stock panel) adopt one identical `＋ Two-sided setup` label + a `Stock ▸`
  navigation chip to the single editing home (Stock › Alignment pins). The
  explanatory caption is replaced by the chip. The event is already shared.

All other shell / setup / tool / catalog / export / menu / status-bar
capabilities **keep their existing authoritative homes** — they were not
implicated in a finding. See `MIGRATION.md` for the per-capability record.
