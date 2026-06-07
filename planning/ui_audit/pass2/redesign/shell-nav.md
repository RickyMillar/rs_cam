# Pass-2 Redesign — Shell, navigation chrome, ribbon & menus

Scope: app shell chrome that frames every workspace — the always-visible
workspace bar + status bar, the two left navigation rails (Setup list,
Toolpath queue), the per-toolpath property **header** (the always-on block
*above* the tab bar), and the dead `project_tree` module.

Out of scope (pass-1 territory): the per-toolpath property TABS and any
feeds/speeds editing surface; the toolpath-tab restructure; the fixture
collision fix, viewport-visibility dedup, dead Edit>Delete, post-config sync.
This spec deals only with problems *intrinsic* to the chrome itself.

Findings addressed: SHE-001..SHE-007.

---

## Guiding principle for the chrome layer

The shell has exactly **one collision/readiness truth** and one place each
fact lives. Today the same fact (collision count, "is this ready") is
computed three different ways across three always-visible surfaces, and a
whole navigation panel (`project_tree`) ships unreachable. The redesign
collapses the duplicated computations to a single source and gives each
chrome surface one concern.

---

## 1. Retire the dead project-tree panel — SHE-001

`ui/project_tree.rs` (445 lines, `pub fn draw` at line 12) has **zero call
sites**. The left rails are `setup_panel::draw` and `toolpath_panel::draw`;
`project_tree` is never drawn. It only compiles because the module is `pub`,
which suppresses the dead-code lint.

Decision: **delete the module.** Remove `pub mod project_tree;` from
`ui/mod.rs:10` and delete `ui/project_tree.rs`.

Rationale, not "keep it just in case":
- It is the phantom referenced by every "this duplicates the tree" note
  elsewhere in the audit — those comparisons are against a panel users have
  never seen. Removing it removes the phantom baseline.
- Its job (job name, stock, post-processor, per-setup tree, toolpath tree)
  is already covered by `setup_panel` (stock card, setup cards, Models
  collapsing header) and `toolpath_panel` (the operation queue). There is
  no missing capability to preserve.
- Anything genuinely worth salvaging (e.g. the Post-Processor selectable, if
  it is *only* in project_tree) must be re-homed explicitly, not kept alive
  by an unreachable module. Audit before delete: `rg "Selection::PostProcessor"`
  — if the post-processor selectable has no other surface, move it into the
  Setup rail's stock/summary card; otherwise drop it with the module.

Retired: entire `ui/project_tree.rs`.

---

## 2. One collision/readiness truth for the chrome — SHE-002, SHE-003

Two always-co-visible surfaces print "collisions" with two different
tallies, and two adjacent badges in the *same* bar disagree on
collision-vs-stale precedence.

- `status_bar.rs:83` prints `"{collision_count} collisions"` where
  `collision_count = controller.collision_positions().len()` — **holder
  clearance only** (compute.rs:633). Rapid collisions are silently omitted.
- `workspace_bar.rs:140` (Simulation badge) and `:170` (Setup readiness
  badge) both use `holder_collision_count + rapid_collisions.len()` — the
  full tally.
- Within the workspace bar, `simulation_badge` returns ` stale` and exits
  (lines 136-138) **before** checking collisions (140-143), while
  `readiness_badge` checks collisions **first** (175) then stale (179). A
  stale-and-colliding project shows red `N collision(s)` on Setup but only
  yellow ` stale` on Simulation.

### Fix: a single `ShellHealth` summary, computed once

Introduce one helper (e.g. `state::shell_health()` or a small struct built
in `app.rs` next to where `col_count`/`lane_snapshots` are already gathered)
that produces the canonical chrome health snapshot exactly once per frame:

```
struct ShellHealth {
    collisions: usize,   // holder_collision_count + rapid_collisions.len()  (one definition)
    uncomputed: usize,   // enabled ops with no result
    sim_stale: bool,     // has_results && is_stale(edit_counter)
    has_results: bool,
}
```

Both the status bar and both workspace-bar badges consume this struct — no
surface recomputes the tally. Result:

- **SHE-002 fixed:** there is one `collisions` number. The status bar prints
  `ShellHealth.collisions` (holder **+** rapid), matching the workspace bar.
- **SHE-003 fixed:** the badge severity ordering is computed *once* in a
  single precedence ladder and reused by both badges. Collisions are an
  ERROR and always outrank a stale WARNING, on every tab.

### Canonical severity ladder (one definition, used by every badge)

Highest severity wins, evaluated top-down:

| Order | Condition                         | Tier    | Color   |
|-------|-----------------------------------|---------|---------|
| 1     | `collisions > 0`                  | ERROR   | red     |
| 2     | `uncomputed > 0`                  | WARNING | yellow  |
| 3     | `sim_stale`                       | WARNING | yellow  |
| 4     | `has_results && collisions == 0`  | OK      | green   |
| 5     | otherwise                         | (none)  | —       |

The Setup readiness badge already follows essentially this ladder; the
Simulation badge is brought into line by deleting its early ` stale` return
and routing it through the shared ladder. Per-tab badges may still *scope*
which conditions are relevant (e.g. the Toolpaths "pending" badge), but they
never reorder severity: collisions never hide behind stale anywhere.

### Status-bar collision chip earns provenance

Since the redesign principles require freshness/provenance on diagnostic
readouts, the status-bar collision chip becomes legible rather than a bare
count: hover reveals the split (`"3 collisions — 1 holder, 2 rapid"`) and the
chip is greyed when `sim_stale` (the count is from a stale run). UI wins: the
chip color is the ladder tier, the breakdown is on hover, not inline prose.

---

## 3. Toolpath-property header — group five concerns, dig the diagnostics up — SHE-004, SHE-005

`draw_toolpath_panel` (properties/mod.rs:2501-2744) stacks five unrelated
concern groups flat above the tab bar, with the richest signal (the
three-tier diagnostics ribbon) buried last under IO fields:

1. **Identity** — Name (2507-2510)
2. **Geometry IO** — Tool combo (2513-2527), Input/model combo (2530-2544),
   BREP-missing warning (2550-2559), Face Selection picker (2562-2592)
3. **Stock linking** — Use-remaining-stock checkbox (2595-2613)
4. **Generate** — Generate button + status + move count (2619-2654),
   validation errors (2655-2663)
5. **Diagnostics** — three-tier ribbon (2679-2743)

### Fix: summary-first header, detail behind disclosure

Re-order and group so the header reads **status → action → detail**, not
**IO → action → status-buried-at-bottom**:

```
[ Name (inline edit) ]                         ← identity, one line
──────────────────────────────────────────────
[ Status line: ● Done · 412 moves   |  Generate ]   ← summary + primary action, top
[ actionable diagnostics rows ]                ← the rich signal, promoted above the fold
[ stateful diagnostic rows ]
▸ Hints (n)                                    ← collapsed
──────────────────────────────────────────────
▸ Geometry  (Tool · Input model · Faces)       ← group 2+3 behind one disclosure
   [ Tool combo ]
   [ Input model combo ]
   [ Face Selection ... ]   (only if STEP w/ enriched mesh)
   [ ☐ Use remaining stock ]
```

Concrete moves:

- **Promote the Generate+status line and the diagnostics ribbon** to sit
  directly under the Name, *above* the geometry/IO fields. The most
  actionable info (is it generated? are there warnings?) is now summary-first
  instead of buried under combos the user rarely touches after setup.
- **Group Tool + Input model + Face Selection + Use-remaining-stock** under a
  single collapsing **"Geometry"** disclosure (default-open on a freshly
  added op, default-collapsed once the op has a result — i.e. once these are
  settled they get out of the way). This is the "dig deeper" tier:
  rarely-changed wiring lives behind one header instead of four flat rows.
- The BREP-missing warning stays inside the Geometry group, adjacent to the
  Input model combo it concerns (it is currently orphaned between Input and
  Face Selection).
- Validation errors move next to the Generate button (they gate it), where
  they read as "why is Generate disabled" rather than as loose red text.

Distinct roles look distinct: identity (text), summary/status+action (one
status line), diagnostics (tiered colored rows), wiring (collapsed group).
Five flat lookalike rows become four visually-distinct zones.

### SHE-005: collapse the doubled face-picker prose

In the Face Selection block, the zero-selected state currently shows **two**
stacked sentences:
- italic `"Click faces in viewport to select"` (2582-2585)
- `.small()` `"Tip: click faces in the 3D view while this toolpath is
  selected"` (2587-2591) — and this Tip renders even when faces *are*
  selected.

UI wins, not prose: keep **one** affordance.
- Zero selected: a single muted placeholder line `Pick faces in viewport ↗`
  styled as a placeholder, not a sentence.
- ≥1 selected: `{n} faces selected   [Clear]` — no tip line at all.
- Delete the unconditional Tip line entirely. The action ("click faces") is
  conveyed by the placeholder + the live selection count updating as the user
  clicks; it does not need a second explanatory sentence.

---

## 4. Toolpath card — surface the hidden queue actions — SHE-006

Per-card **Enable/Disable** and **Duplicate** exist *only* in the right-click
context menu (toolpath_panel.rs:463-471) with no visible inline affordance.
Inline rows expose Sim (373), Generate (383), and the eye/C/R/isolate strip
(424-431). (Reorder is **not** part of this gap — a drag grip already exists
inline at 301-305, and enabled/disabled state already shows passively via the
dim/`TEXT_FAINT` name coloring at 346-351. So Move-Up/Move-Down menu items
are redundant with the grip and need no inline promotion.)

### Fix: give Enable + Duplicate a visible inline cue

Add to the existing Row-4 controls strip (`toolpath_row_controls::draw`,
already the home of eye/C/R/isolate) two small icon toggles:

- **Enable/Disable** — a checkbox/power glyph reflecting `tc.enabled`. This
  makes the existing passive dim-name cue *actionable* in place, instead of
  forcing a right-click to toggle. The passive dim coloring stays as
  reinforcement.
- **Duplicate** — a small duplicate glyph.

Keep the context menu as the full superset (it is fine for a right-click to
*also* offer everything), but every core queue-management action now has a
visible affordance: Enable, Generate, Sim, Isolate, Hide, Duplicate inline;
Reorder via grip; Delete + Move-Up/Down remain menu-only (Delete is
destructive, Move-Up/Down are redundant with the grip).

One concern / one home is preserved: the row-controls strip is *the* home for
per-card actions; the menu is a discoverability fallback, not the sole entry.

---

## 5. Setup rail — separate navigation from project rollups — SHE-007

`setup_panel::draw` stacks five concerns in one left dock:

1. Stock summary + edit card (22-42)
2. Project ops/tools/time **rollup** summary (draw_project_summary, 47/341)
3. Project-wide **diagnostics** card (draw_project_diagnostics_card, 53/272)
4. Setup cards list (58-61) + Add Setup (67)
5. Models collapsing list (75-103)

Mitigations already present (and why severity is med not high): stock card,
setup cards, and Models are legitimately setup-context; the genuinely
out-of-place items are only #2 (project KPI rollup) and #3 (project
diagnostics). Models is default-collapsed; the diagnostics card self-hides
when there are no findings.

### Fix: navigation on top, rollups behind disclosure

The rail's job is "pick a setup / edit stock / see models." The two
project-level rollups are *whole-project* readouts that belong with project
chrome, not interleaved into the per-setup navigation.

Target structure for the Setup rail (top to bottom):

```
[ Stock card  — dims + Edit ]          ← setup-context, stays
──────────────────────────────────────
[ Setup cards (the actual nav job) ]   ← promoted directly under stock
[ + Add Setup ]
▸ Models (n)                            ← stays collapsed
──────────────────────────────────────
▸ Project summary   (n ops · n tools · ~h:mm)   ← rollup behind disclosure
   [ ops / tools / time table ]
[ ⚠ Project diagnostics card ]          ← only when findings exist (self-hiding, unchanged)
```

Concrete moves:

- **Setup cards move up**, directly under the Stock card — the primary
  navigation job is no longer pushed below two rollup readouts.
- **Project summary becomes a collapsing disclosure** with a one-line
  summary (op count · tool count · est. time) as the header. Default
  collapsed. This is the "summary first, detail behind disclosure" tier:
  the headline numbers are visible in the header text; the full table is one
  click away.
- **Project diagnostics card stays self-hiding** but moves to the **bottom**
  of the rail (after summary), so when it *does* appear it's a distinct
  project-level alert zone, not wedged between stock and setups. Its existing
  hide-when-empty behavior (comment at 49-53) is preserved — no change to
  when it shows, only where.

This keeps every setup-context item (stock, setups, models) as the visible
navigation spine, demotes the two project-rollup concerns to a bottom
disclosure zone, and gives the rail a clear summary-to-detail hierarchy.

Note: the project diagnostics card and the chrome ShellHealth (§2) overlap in
intent — both report project findings/collisions. They are *not* merged here
(the card is the detailed per-finding list; ShellHealth is the one-glance
badge), but both must read collisions from the **same** `ShellHealth.collisions`
definition so the rail card and the workspace badge never disagree.

---

## Summary of new homes

| Concern                         | Old home(s)                              | New home                                          |
|---------------------------------|------------------------------------------|---------------------------------------------------|
| Collision/readiness truth       | 3 surfaces, 3 computations               | one `ShellHealth`, consumed by all (§2)           |
| Badge severity ordering         | 2 divergent ladders                      | one canonical ladder (§2)                         |
| Dead project tree               | `project_tree.rs` (unreachable)          | deleted (§1)                                      |
| Toolpath header status+action   | buried below IO fields                   | promoted to top, summary-first (§3)               |
| Toolpath geometry/IO wiring     | 4 flat rows                              | one "Geometry" disclosure (§3)                    |
| Face-picker guidance            | 2 stacked sentences                      | 1 placeholder + live count (§3, SHE-005)          |
| Enable / Duplicate (per card)   | right-click menu only                    | inline row-controls toggles (§4)                  |
| Setup-rail navigation           | interleaved with project rollups         | nav spine on top (§5)                             |
| Project summary / diagnostics   | stacked into setup nav                   | bottom disclosure zone (§5)                       |
