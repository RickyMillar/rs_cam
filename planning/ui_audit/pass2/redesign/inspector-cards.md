# Redesign SPEC — Inspector diagnostic-card ecosystem (pass 2)

Area: the simulation **Inspector** right-panel — `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`.
Surfaces in scope: `sim-project-overview`, `sim-now-playing-strip`, `sim-selected-span-section`,
`sim-top-hotspots-list`, `sim-hotspot-card`, `sim-issue-card`, `sim-selection-details`.

Findings addressed: INS-001, INS-002, INS-003, INS-004, INS-005, INS-006, INS-007, INS-008, INS-009.

OUT OF SCOPE (pass-1): per-toolpath property tabs, all feeds/speeds surfaces, the
verdict-HUD/Inspector load-rollup reconciliation, viewport-visibility dedup. The
tool-load badge *content* and `verdict_tooltip` are pass-1 territory; we only place
them, we do not redesign the badge internals.

---

## 1. Root cause

The Inspector tries to answer three different questions in one undivided vertical scroll,
and it answers them at three different scopes with no structural separation:

| Scope | Question | Today's blocks |
|-------|----------|----------------|
| PROJECT | "Is this whole run good?" | Global stats, verdict banner, Findings grid, Optimize-all, Must-address, Informational, Top-hotspots (INS-001, INS-002) |
| TOOLPATH (playing) | "What's the op under the playhead doing?" | Now-playing strip + tool-load badges (INS-002) |
| SPAN (selected) | "What's in the span I picked?" | Selected section + its findings list (INS-002, INS-008) |

Everything is always-expanded (only Top-hotspots collapses, INS-001), the two diagnostic
cards look the same (INS-003), the same hotspot datum is printed three different ways
(INS-004), focused cards lose the staleness signal (INS-005), engagement is shown bare
with no provenance (INS-006), and a second "what's selected" block (Selection details)
floats in the same panel from a different selection source (INS-007).

The fix is **scope-tiered disclosure**: one fixed glanceable header that always answers
"is the run good + is it fresh", then exactly three concern-named, collapsible scope
sections (Project / Toolpath / Span) in a fixed order, each summary-first. Confusables get
distinct shapes; metrics get one canonical format + one provenance home.

---

## 2. Target structure

### 2.1 Fixed status header (always visible, never collapses) — fixes INS-001, INS-005

A single non-scrolling strip at the very top of the Inspector, above everything else.
Two lines only:

1. **Verdict line** — the existing single-line banner (collision / air-cut / OK), kept
   verbatim from `:580-606`. This is the glance.
2. **Freshness chip** — `sim.is_stale(gui.edit_counter)` rendered HERE, not buried in the
   overview body. Because the header draws *before* the card dispatch, the stale signal is
   reachable when a hotspot/issue card is focused (INS-005). Fresh → muted "✓ live"; stale
   → amber "⚠ stale — re-run".

This is the only always-on content. Everything below is a collapsible scope section.

### 2.2 Reactive card overlay (unchanged dispatch, restyled) — fixes INS-003, INS-005

`draw_focused_hotspot_card` / `draw_focused_issue_card` keep their mutually-exclusive
early-return dispatch (`:338-343`) and their position (top of the scroll body, under the
fixed header). Two changes:

- **Distinct shapes so distinct roles look distinct (INS-003).** Today both are a dark
  rounded `Frame` with a colored title and a button row. Differentiate structurally, not
  just by 8-unit fill / title hue:
  - **Hotspot card** = orange **left accent bar** (4px), title `◍ Hotspot` (filled glyph),
    body = "time-waste" framing (wasted runtime leads).
  - **Issue card** = amber **dashed/hollow** treatment, title `△ <kind>` (outline glyph),
    body = "engagement/air" framing, keeps Prev/Next nav.
  The accent-bar vs hollow-outline plus the leading glyph make the two readable at a glance
  even though they share the slot.
- The fixed header (2.1) already shows freshness, so a focused card now sits under a live
  stale indicator — INS-005's "act on stale numbers with no signal" is closed without
  duplicating the banner inside each card.

### 2.3 Scope sections — three CollapsingHeaders, fixed order — fixes INS-001, INS-002

Replace the flat always-expanded body of `draw_project_overview` with three named,
collapsible scope sections. Each is `default_open` per its glance value; each is
summary-first with detail behind the disclosure (DIG DEEPER).

**A. Project** — `CollapsingHeader "Project"`, `default_open(true)`.
- Summary row (in the header line, always visible): `Cycle m:ss · ✓N within · ✗M exceeding`.
- Body (expanded): the Global grid (moves/ops/cut/rapid), the Findings grid
  (within/exceeding/unmodeled/collisions), the `⚡ Optimize all N` button (only when
  `bad > 0`), and the **Must-address** + **Informational** issue-kind partition.
- Top-hotspots stays a NESTED collapsing sub-list inside Project body
  (`default_open(false)`), as today.

**B. Toolpath (now playing)** — `CollapsingHeader`, title = `Now playing: <name>` or
`Now playing: —` when idle, `default_open(true)` when a boundary is active.
- This is the per-TP scope (INS-002). It owns the Now-playing strip + tool-load badges
  (`:830-873`) and the per-TP Optimize / Jump-to-start buttons.
- When no boundary plays, header reads `Now playing: —` and the body shows a one-line
  "Scrub or play to see the active op." The section never silently disappears, so the
  scope is always legibly present.

**C. Span (selected)** — `CollapsingHeader`, title = `Selected: <span label>` +
lock glyph, `default_open(true)`.
- This is the per-span scope. It owns `draw_selected_section` (`:1260-1521`): span facts,
  the metrics grid, and the in-span findings list.
- The lock affordance is fixed here (see 2.5).

Because the three sections are named by scope and the header line of each carries its own
scope label, a number can no longer be mistaken for the wrong scope (INS-002): Project
numbers live under "Project", the playing-op numbers under "Now playing: <name>", the span
numbers under "Selected: <span>".

### 2.4 Retire the standalone "Selection details" header — fixes INS-007

`Selection details` (`:163-228`, driven by `active_semantic_item`) and the Span section
(driven by `span_scope/playhead`) both answer "what am I looking at" from different
sources. They are NOT merged — they describe orthogonal axes (semantic generator item vs
structural span) — but they must stop floating as two peer "selection" blocks.

Resolution: fold the semantic-item facts (label / kind / xy_bbox / z / params grid) into
the **Span (selected)** section as a nested `default_open(false)` sub-disclosure titled
**"Generator item"**. One "what's selected" home, two clearly-labelled sub-tiers inside it:
the structural span (primary) and the generator item that produced it (drill-down). The
`Generation Metrics` header (`:236-308`, generator phases/timings) likewise nests under
Span as a sibling `default_open(false)` sub-disclosure **"Generation trace"**. Net: the
top level of the Inspector no longer has two competing "selection" collapsers.

### 2.5 In-panel span lock — fixes INS-008

The Span section's lock can only be SET from the timeline ribbon today; the panel only
offers a passive `· locked` tag and an unlock button that appears after the fact (`:1314-
1330`). Add a **lock toggle on the Span header** that writes `sim.debug.span_scope.span_id`
to the currently-effective span when engaged and to `None` when released — mirroring the
ribbon's existing write (`sim_timeline.rs:1416`). Glyph toggle: `🔓 follow` ⇄ `🔒 locked`.
This gives the panel its own entry point to its primary mode; the ribbon stays as a second
way in. No new event type needed — it writes the same field the unlock button already
writes.

---

## 3. Canonical metric formatting — fixes INS-004, INS-006

### 3.1 One hotspot row format everywhere (INS-004)

The same hotspot datum is printed three ways (focused card `:386-400`, Top-hotspots row
`:789-792`, Span findings row `:1468-1473`). Define ONE helper and call it from all three:

```
fn hotspot_summary_line(h) -> "m{start} · waste {:.2}s · peak chip {:.4} mm"
```

- Card adds the extra detail lines (move range, sample count, DOC, engagement, XYZ) BELOW
  this canonical first line — the lead line matches the list rows exactly.
- The two list rows (Top-hotspots, Span findings) use the helper verbatim. The Span row
  keeps its `Hotspot ·` prefix only because the Span list is mixed hotspots+issues; in the
  Top-hotspots list (hotspots only) the prefix is dropped — both share the same numeric
  tail and units.

### 3.2 One engagement unit + one provenance home (INS-004, INS-006)

Engagement is printed as `{:.0}%` in the card (`:395`) but `{:.2}` raw fraction in the
Span grid (`:1383`). Pick ONE: render engagement as **percent** everywhere
(`avg engage {:.0}%`), since percent is the operator-facing unit and matches the
diagnostic-threshold table. Update the Span grid row to `avg {:.0}% · peak {:.0}%`.

Provenance (INS-006): engagement carries the documented caveat (cylinder-side radial-WOC
fraction, reads ~10× below target, comparative-only — `CLAUDE.md` adaptive_review note).
Today only the tool-load badges have a provenance tooltip (`verdict_tooltip`, `:1063`).
Attach an `on_hover_text` to EVERY engagement readout (card line, Span grid row) with the
short form:

> Engagement = cylinder-side radial width-of-cut fraction. Reads ~10× below the
> algorithmic target; use it to compare variants, not as an absolute under-engagement bar.

UI WINS, not prose: this is a hover, not body text. The number stays terse; the caveat is
one click-away, consistent with how the badges already disclose their source. Optionally
suffix engagement readouts with a small `ⓘ` so the hover is discoverable.

---

## 4. Optimize-button scoping — INS-009 (low, light touch)

Four `Optimize` affordances fire different scopes. After the restructure each lives under
its scope-named section (hotspot card → its TP; Now-playing → playing TP; Project → all
exceeding), so the section heading already disambiguates target. One small label change for
zero ambiguity: the per-TP buttons read **`Optimize this op`**; the project button keeps
**`⚡ Optimize all N exceeding`**. No behavioral change.

---

## 5. What is retired / moved

| Element | Today | After |
|---------|-------|-------|
| Stale banner | inside overview body `:819`, unreachable when card focused | fixed status header (2.1), always reachable |
| Verdict banner | mid-overview `:602` | fixed status header (2.1) |
| Flat always-expanded overview body | `:518-742` no collapse | nested under **Project** CollapsingHeader (2.3-A) |
| Now-playing strip | glued mid-overview `:830-873` | own **Toolpath (now playing)** section (2.3-B) |
| Selected section | bottom of overview `:888` | own **Span (selected)** section (2.3-C) |
| `Selection details` header | top-level peer `:163` | nested "Generator item" under Span (2.4) |
| `Generation Metrics` header | top-level peer `:236` | nested "Generation trace" under Span (2.4) |
| 3× divergent hotspot formats | `:386,:789,:1468` | one `hotspot_summary_line` helper (3.1) |
| engagement `{:.2}` raw | Span grid `:1383` | percent everywhere (3.2) |
| engagement w/ no provenance | `:395,:1383` | `on_hover_text` caveat on every readout (3.2) |
| span lock set only from ribbon | `:1314-1330` | lock toggle on Span header (2.5) |

Nothing is deleted; every datum keeps a home. The change is structural grouping +
disclosure tiers + format/provenance consolidation.

---

## 6. Why this satisfies the principles

- **One concern / one home**: Project / Toolpath / Span are three named sections; "what's
  selected" has a single home (Span) with sub-tiers, not two peer collapsers.
- **Dig deeper**: fixed verdict+freshness header is the glance; the three sections are
  summary-first (header line = summary) with detail behind disclosure.
- **Distinct roles look distinct**: hotspot card (accent bar, filled glyph) vs issue card
  (hollow outline, outline glyph) are now structurally different, not 8-unit fill apart.
- **UI wins not prose**: provenance is a hover + ⓘ, not body text; lock is a glyph toggle.
- **Every control earns its place**: Optimize buttons relabelled by scope; no dead
  duplicate selection block.
- **Provenance/freshness legible**: freshness in the fixed header for all states (incl.
  focused cards); engagement provenance on every readout.
