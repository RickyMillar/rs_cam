# Mockups — Simulation & diagnostics legibility

Legend for visual roles (the "distinct roles look distinct" requirement):

```
[ within 6/8 ]      verdict pill, READ-ONLY   (flat, no cursor change)
( exceeds 2/8→ )    verdict pill, ACTIONABLE  (rounded, hover-fill, trailing → )
{ collisions 0 }    sim-observation pill, READ-ONLY  (neutral, binary green)
( collisions 3→ )   sim-observation pill, ACTIONABLE (neutral, alarm-red)
[x] Label           checkbox (global scope)
 C  / [C]           per-row single-letter button: active / greyed (gated off)
```

`/T` denominator is the visible proof the HUD and overview count the same thing.

---

## A. Verdict HUD — before vs after (P4-001, P4-002, P5-001, P6-004)

### BEFORE (per-criterion counts, prose tooltips, no clicks)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ ✓ load 18   ⚠ unmodeled 3   ✕ exceeds 4   ~ approx 2   collisions 3        │  <- all plain labels
│ issues 41   traces 6                                                        │     hover only:
└──────────────────────────────────────────────────────────────────────────┘     "...click the red
   (18 = per-criterion, 3 gates × TP — NOT a toolpath count, but tooltip            lines on the
    says "Toolpaths within modeled load limits")                                     timeline below"
```

### AFTER (per-toolpath /T counts, clickable, no redirect prose)
```
┌──────────────────────────────────────────────────────────────────────────┐
│  ── LOAD (vendor-LUT gate) ──────────  ── SIM RUN ───────────────────────  │
│  [ within 6/8 ]  ( exceeds 2/8→ )      ( collisions 3→ )   ( issues 41→ )   │
│  [ unmodeled 0/8 ]                     { traces 6 }                         │
└──────────────────────────────────────────────────────────────────────────┘
       verdict family (green/red/amber ramp)   observation family (neutral)

  hover ( exceeds 2/8→ ):  "Load-limit exceedances"   <- NAME only, no nav prose
  click ( exceeds 2/8→ ):  jumps playhead to first exceed marker + opens drill (B)
  { traces 6 } is flat/read-only — nothing to jump to, no cursor change
  { collisions 0 } when zero -> renders as read-only green { collisions 0/safe }
```

Note: `within/exceeds/unmodeled` now read **exactly** what the Project overview
reads (both from `summary()`), and the shared `/8` makes the same-denominator
obvious at a glance. The retired `~ approx` is folded into the verdict (advisory
state shown in the drill, not as a 5th competing top-level count).

---

## B. Exceeds pill drill — progressive disclosure (P3)

Clicking `( exceeds 2/8→ )` reveals the `exceeds_breakdown` list. Summary is the
pill; detail is behind the click.

```
( exceeds 2/8→ )  ◀ clicked
   ┌──────────────────────────────────────────────┐
   │ 2 exceedances                                 │
   │ • Back Rough   · power     · high      →jump  │   each row = clickable
   │ • Finish Pass  · deflection· (L/D)     →jump  │   jump to that marker
   └──────────────────────────────────────────────┘
```

---

## C. Inspector › View — before vs after (P4-005, P6-002)

### BEFORE (duplicate visibility toggles + dead ByOperation)
```
▼ View
  Stock
   [x] Show stock                    <- DUP of overlay "Stock"
   Opacity: [======|====] 0.6
  Toolpaths
   [x] Show cutting moves            <- DUP of overlay "Paths (cutting)"
   [x] Show rapid moves              <- DUP of overlay "Rapids"
  Analysis
   Stock color: [ Solid        ▼ ]   <- ByOperation missing (maps to "Solid")
   No deviation data — re-run simulation to compute     (prose, no button)
```

### AFTER (visibility relocated to overlay; analysis stays; ByOperation wired)
```
▼ View
  Stock appearance
   Opacity: [======|====] 0.6
   Stock color: [ Solid          ▼ ]
                 ├ Solid
                 ├ Deviation
                 ├ By Height
                 └ By Operation        <- WIRED (P6-002), real GPU branch + serde

   ⓘ Deviation needs fresh sim data:   <- shown only in Deviation mode w/ no data
      ( ▶ Re-run simulation )          <- POINT-OF-NEED button (P5-004), not prose

  (raw show/hide for Stock · Cutting · Rapid moves -> Viewport "Show ▼" toolbar)
```

The three duplicated checkboxes are gone from here. A single muted line names
their one home (this is a *pointer to the home*, not the redirect-prose the rules
forbid — it replaces three live duplicate controls with zero controls).

---

## D. Deviation point-of-need re-run (P5-004) — detail

### BEFORE
```
Stock color: [ Deviation ▼ ]
No deviation data — re-run simulation to compute      <- WARNING label, no action.
                                                          Run lives on another panel.
```

### AFTER
```
Stock color: [ Deviation ▼ ]
┌────────────────────────────────────────────┐
│ ⓘ No deviation data captured for this run.  │
│            ( ▶ Re-run simulation )          │   <- emits RunSimulation in place
└────────────────────────────────────────────┘
```

The button is co-located with the selector that created the requirement; the
user never leaves the View panel.

---

## E. Viewport overlay › Show ▼ — the ONE visibility home (P4-005)

```
[Show ▼]
 ┌───────────────────────────┐
 │ [x] Stock                 │   <- single label set, shared everywhere
 │ [x] Fixtures              │
 │ [x] Curves (DXF/SVG)      │
 │ [x] Cutting moves         │      ("Paths (cutting)" -> "Cutting moves")
 │ [x] Rapid moves           │      ("Rapids"          -> "Rapid moves")
 │ [x] Collisions            │
 └───────────────────────────┘
```

---

## F. Global vs per-toolpath visibility layering (P4-004)

Global "Cutting moves" ON — per-row C/R buttons active:
```
Operations Queue
  ◉ Back Rough     [👁] [ C ][ R ]      <- C/R live, toggle this toolpath's moves
  ◉ Finish Pass    [👁] [ C ][ R ]
                          ▲
        global "Cutting moves" = ON, so per-row C is enabled
```

Global "Cutting moves" OFF — per-row C greyed (affordance = explanation, no prose):
```
Viewport Show ▼:  [ ] Cutting moves          <- global gate OFF

Operations Queue
  ◉ Back Rough     [👁] [·C·][ R ]      <- C greyed: "global Cutting is off,
  ◉ Finish Pass    [👁] [·C·][ R ]          this per-row override is inert"
                          ▲
        no "toggled and nothing happened" surprise — the greyed state shows why
```

`[·C·]` = disabled/greyed; `[ C ]` = active. R follows the same rule against the
global Rapid toggle.

---

## G. Empty-state card (P5-003) — inline Run, matching sibling pattern

### BEFORE
```
┌ Ready to simulate ───────────────────────┐
│ Use Run Simulation above to begin.        │   <- prose only
└───────────────────────────────────────────┘
```

### AFTER
```
┌ Ready to simulate ───────────────────────┐
│        ( ▶ Run Simulation )               │   <- matches the sibling
└───────────────────────────────────────────┘      "Go to Toolpaths" button
```

---

## H. Pill role cheat-sheet (the explicit "distinct roles look distinct" ask)

```
ACTIONABLE  ( exceeds 2/8→ )   rounded · hover-fill · trailing →  · jumps on click
READ-ONLY   [ within 6/8 ]     flat    · no cursor change · no →   · glance only
OBSERVATION { traces 6 }       neutral palette (not verdict ramp)  · read-only
ALARM       ( collisions 3→ )  neutral palette · alarm-red · clickable
SAFE        { collisions 0/safe } neutral palette · green · read-only
```

Verdict family and observation family never share a color ramp, so "issues 41"
can never be misread as a load verdict.
```
