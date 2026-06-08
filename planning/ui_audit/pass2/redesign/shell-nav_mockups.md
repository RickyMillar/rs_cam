# Pass-2 Mockups — Shell, navigation chrome, ribbon & menus

ASCII mockups for the shell-nav redesign. Findings referenced by id.

---

## A. Always-visible chrome — one collision truth, one severity ladder (SHE-002, SHE-003)

### Before — stale + 2 collisions (1 holder, 1 rapid), Setup tab active

```
┌─ menu bar ──────────────────────────────────────────────────────────────┐
├─ workspace bar ─────────────────────────────────────────────────────────┤
│ [ Setup 2 collision(s) ] [ Toolpaths ] [ Simulation  stale ]            │   ← Setup says RED "2 collision(s)"
│                                          ^^^^^^^^^^^^^^^^^^^^^             │     Simulation says YELLOW " stale"
│                                          same project, hides the collision│     (SHE-003: severity disagrees)
├─ ... viewport ...                                                         │
├─ status bar ────────────────────────────────────────────────────────────┤
│ Models: 1 | Triangles: 8.2k   Toolpaths 3/4    SIM    1 collisions       │   ← "1 collisions" (holder only)
└──────────────────────────────────────────────^^^^^^^^^^^─────────────────┘     disagrees with bar's "2" (SHE-002)
```

Three numbers for one fact: `2 collision(s)`, ` stale`, `1 collisions`.

### After — same project, one ShellHealth feeds every surface

```
┌─ menu bar ──────────────────────────────────────────────────────────────┐
├─ workspace bar ─────────────────────────────────────────────────────────┤
│ [ Setup  ⛔2 ] [ Toolpaths ] [ Simulation  ⛔2 ]                          │   ← both tabs: ERROR, count = 2
│         ^^^^                  ^^^^^^^^^^^^^^^^^                            │     collisions outrank stale on
│         collisions outrank stale on BOTH (one ladder)                     │     every tab (SHE-003 fixed)
├─ ... viewport ...                                                         │
├─ status bar ────────────────────────────────────────────────────────────┤
│ Models: 1 | Triangles: 8.2k   Toolpaths 3/4   SIM   ⛔ 2 collisions      │   ← same "2" (holder+rapid)
│                                                      ╰─ hover ────────────╯     (SHE-002 fixed); greyed if
│                                                      "2 collisions —      │     count is from a stale run.
│                                                       1 holder, 1 rapid"  │     Provenance on hover, not inline.
└──────────────────────────────────────────────────────────────────────────┘
```

Severity ladder (one definition, every badge):
`collisions ⛔ > uncomputed ⚠ > sim_stale ⚠ > clean ✓ > none`

---

## B. Toolpath-property header — summary-first, IO behind disclosure (SHE-004, SHE-005)

### Before — five flat concern groups, diagnostics buried last

```
┌ Properties: Pocket roughing ─────────────────┐
│ Name: [ Pocket roughing            ]          │  identity
│ Tool: [ 6mm flat ▾ ]                          │  ┐
│ Input:[ part.step ▾ ]                         │  │ geometry IO
│ ⚠ BREP topology not loaded — face picker...   │  │ (orphaned warning)
│ Face Selection                                │  │
│   Click faces in viewport to select           │  │  ← SHE-005: two
│   Tip: click faces in the 3D view while this  │  │     stacked sentences
│        toolpath is selected                   │  ┘
│ ☐ Use remaining stock                         │  stock linking
│ [ Generate ]  Done   412 moves                │  ┐ action + status
│ (validation errors here, if any)              │  ┘
│ ⚠ chipload high on pass 3        [Fix]        │  ┐
│ ⓘ needs current simulation                    │  │ diagnostics ribbon
│ ▸ Hints (2)                                   │  ┘ BURIED at bottom
├──────────────────────────────────────────────┤
│ [Params][Feeds][Heights][Dressups]  (tabs)    │
```

### After — status + diagnostics on top, geometry collapsed

```
┌ Properties: Pocket roughing ─────────────────┐
│ Name: [ Pocket roughing            ]          │  identity (1 line)
│ ──────────────────────────────────────────── │
│ ● Done · 412 moves           [ Generate ]     │  SUMMARY + primary action, top
│ ⚠ chipload high on pass 3            [Fix]    │  ┐ diagnostics PROMOTED
│ ⓘ needs current simulation                    │  │ above the fold
│ ▸ Hints (2)                                   │  ┘ (collapsed tier stays collapsed)
│ ──────────────────────────────────────────── │
│ ▾ Geometry   6mm flat · part.step · 3 faces   │  one disclosure for wiring
│    Tool:  [ 6mm flat ▾ ]                       │   (header shows the summary;
│    Input: [ part.step ▾ ]                      │    auto-collapses once op has
│    ⚠ BREP topology not loaded — reload model.  │    a result)
│    Faces: 3 faces selected        [Clear]      │   ← SHE-005: ONE line, live
│    ☐ Use remaining stock                       │      count; Tip line deleted
├──────────────────────────────────────────────┤
│ [Params][Feeds][Heights][Dressups]  (tabs)    │
```

Zero-faces state inside the collapsed Geometry group (SHE-005):
```
│    Faces: Pick faces in viewport ↗            │  ← single placeholder, no Tip sentence
```

---

## C. Toolpath card — Enable + Duplicate get inline affordances (SHE-006)

### Before — Enable/Duplicate hidden in right-click only

```
┌ toolpath queue (left rail) ──────────────────┐
│ ⠿ ▮ OK  Pocket roughing                       │  grip · swatch · status · name
│   6mm flat                       Sim   ▶       │  tool · quick actions
│   412 moves · 3 min · 1.2 m                    │  stats
│   👁  C  R  ⤢                                   │  row controls (eye/C/R/isolate)
└───────────────────────────────────────────────┘
        ↑ Enable/Disable + Duplicate exist ONLY here ↓ (no visible cue)
        ┌─ right-click ────────────┐
        │ Generate                 │
        │ Inspect in Simulation    │
        │ Isolate this toolpath    │
        │ Hide                     │
        │ Disable        ← hidden  │
        │ Duplicate      ← hidden  │
        │ ─────                    │
        │ Move Up / Move Down      │  (redundant w/ grip — stay menu-only)
        │ Delete                   │
        └──────────────────────────┘
```

### After — row-controls strip grows two toggles

```
┌ toolpath queue (left rail) ──────────────────┐
│ ⠿ ▮ OK  Pocket roughing                       │
│   6mm flat                       Sim   ▶       │
│   412 moves · 3 min · 1.2 m                    │
│   ⏻  ⧉  👁  C  R  ⤢                             │  ← ⏻ Enable toggle, ⧉ Duplicate
│   ^^  ^^                                       │     now inline & visible.
│   Enable  Duplicate (was menu-only)           │     Dim-name cue still reinforces
└───────────────────────────────────────────────┘     disabled state.
   Disabled card (⏻ off, name dimmed):
│ ⠿ ▮ OK  Pocket roughing  (dimmed)             │
│   ⏻̶  ⧉  👁  C  R  ⤢                             │  ⏻ shows off; name TEXT_FAINT
```

Context menu remains the full superset (Delete + Move Up/Down stay there).

---

## D. Setup rail — nav spine on top, rollups behind disclosure (SHE-007)

### Before — five concerns interleaved

```
┌ Setups (left rail) ──────────────────────────┐
│ ┌ Stock  120 x 80 x 18 mm ───────────────┐   │  setup-context ✓
│ │  Edit stock dimensions                  │   │
│ └─────────────────────────────────────────┘   │
│ ┌ Project summary ────────────────────────┐   │  ← project rollup, out of place
│ │  4 ops · 3 tools · ~0:42                 │   │
│ │  (full table)                            │   │
│ └─────────────────────────────────────────┘   │
│ ┌ ⚠ Project diagnostics ──────────────────┐   │  ← project alert, wedged mid-rail
│ │  2 collisions · 1 air-cut outlier        │   │
│ └─────────────────────────────────────────┘   │
│ ┌ Setup 1  Top · 0° ──────────────────────┐   │  ← the ACTUAL nav job, pushed
│ │  ...                                      │   │     below two rollups
│ └─────────────────────────────────────────┘   │
│ [ + Add Setup ]                                │
│ ▸ Models                                       │
└───────────────────────────────────────────────┘
```

### After — navigation first, rollups demoted to bottom disclosure

```
┌ Setups (left rail) ──────────────────────────┐
│ ┌ Stock  120 x 80 x 18 mm ───────────────┐   │  setup-context ✓
│ │  Edit stock dimensions                  │   │
│ └─────────────────────────────────────────┘   │
│ ──────────────────────────────────────────── │
│ ┌ Setup 1  Top · 0° ──────────────────────┐   │  ← NAV SPINE promoted to top
│ │  ...                                      │   │
│ └─────────────────────────────────────────┘   │
│ ┌ Setup 2  Bottom · 180° ─────────────────┐   │
│ │  ...                                      │   │
│ └─────────────────────────────────────────┘   │
│ [ + Add Setup ]                                │
│ ▸ Models (2)                                   │  stays collapsed
│ ──────────────────────────────────────────── │
│ ▸ Project summary   4 ops · 3 tools · ~0:42    │  ← rollup behind disclosure;
│                                                │     headline in the header text,
│                                                │     table one click away
│ ┌ ⚠ Project diagnostics ──────────────────┐   │  ← bottom alert zone; self-hides
│ │  ⛔ 2 collisions · 1 air-cut outlier      │   │     when no findings. Reads the
│ └─────────────────────────────────────────┘   │     SAME ShellHealth.collisions
└───────────────────────────────────────────────┘     count as the workspace badge.
```

(Healthy project — diagnostics card absent, summary collapsed:)
```
│ [ + Add Setup ]                                │
│ ▸ Models (2)                                   │
│ ──────────────────────────────────────────── │
│ ▸ Project summary   4 ops · 3 tools · ~0:42    │
└───────────────────────────────────────────────┘
```

---

## E. Retired surface (SHE-001)

```
ui/project_tree.rs  ──►  ✗ DELETED  (445 lines, zero call sites)
ui/mod.rs:10  `pub mod project_tree;`  ──►  ✗ removed
```
No mockup — there is nothing to show; users never saw it.
```
```
