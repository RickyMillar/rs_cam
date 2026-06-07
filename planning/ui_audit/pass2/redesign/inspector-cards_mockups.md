# Inspector diagnostic-card ecosystem — ASCII mockups

Demonstrates the SPEC in `inspector-cards.md`. UI wins, not prose.

---

## M1. Default view (no card focused) — fixed header + three scope sections

Fixes INS-001 (summary-first collapse), INS-002 (scope-named sections),
INS-005 (freshness always visible), INS-007 (one selection home).

```
┌─ Inspector ─────────────────────────────────────────────┐
│ ✓ No collisions, air cutting under threshold             │ ◀ fixed verdict (glance)
│ ✓ live                                                   │ ◀ fixed freshness (always on)
├──────────────────────────────────────────────────────────┤
│ ▼ Project          Cycle 4:12 · ✓6 within · ✗2 exceeding │ ◀ summary in header line
│    Moves        1 842                                     │
│    Operations   8                                         │
│    Cut distance 5 210 mm    Rapid 1 980 mm               │
│    ─────────────────────────────────────────────         │
│    Findings:  ✓6 within  ✗2 exceeding  ⚠1 unmodeled      │
│               ● 0 collisions                             │
│    [ ⚡ Optimize all 2 exceeding ]                        │
│    Must address:  Hotspot 14                             │
│    Informational: Low engagement 31 · Air cut 24 800     │
│    ▸ Top hotspots (14)                                   │ ◀ nested, collapsed
│ ▼ Now playing: Pocket — 6mm flat        🔓 follow        │ ◀ TOOLPATH scope
│    chip 82%  power 41%  defl OK≈                         │
│    [ Optimize this op ]  [ Jump to start ]              │
│ ▼ Selected: Z-1.50 contour pass   🔒 locked             │ ◀ SPAN scope, lock on header
│    contour · moves 220–410                              │
│    Samples    190 (171 cutting)                         │
│    Engagement avg 3% · peak 7%             ⓘ            │ ◀ percent + provenance hover
│    Chipload   avg 0.0412 · peak 0.0890 mm              │
│    Axial DOC  peak 2.00 mm                             │
│    MRR        avg 612 · peak 1 040 mm³/s              │
│    Findings in this span                               │
│      m231 · waste 0.84s · peak chip 0.0890 mm         │ ◀ canonical hotspot line
│      Low engagement · m305 · sweeping uncut           │
│    ▸ Generator item                                    │ ◀ nested (was Selection details)
│    ▸ Generation trace                                  │ ◀ nested (was Generation Metrics)
└──────────────────────────────────────────────────────────┘
```

Header line of each section IS its summary; collapsing any section hides only its detail.
"Now playing: —" and a one-liner show when idle, so the scope never vanishes.

---

## M2. Hotspot card focused — distinct SHAPE vs issue card (INS-003)

Note the fixed header still shows freshness ABOVE the card (INS-005 closed).

```
┌─ Inspector ─────────────────────────────────────────────┐
│ ✓ No collisions, air cutting under threshold             │
│ ⚠ stale — re-run                                         │ ◀ stale signal reaches focused card
├──────────────────────────────────────────────────────────┤
│┃ ◍ Hotspot                                  TP 3         │ ◀ orange LEFT ACCENT BAR + filled glyph
│┃ m231 · waste 0.84s · peak chip 0.0890 mm               │ ◀ canonical lead line (matches lists)
│┃ moves 231–248 · 17 samples                             │
│┃ peak DOC 2.00 mm · avg engage 3%          ⓘ           │ ◀ percent + provenance hover
│┃ X120.4 Y58.1 Z-1.50                                    │
│┃ [ Jump ]  [ Optimize this op ]  [ Clear ]             │
└──────────────────────────────────────────────────────────┘
```

## M3. Issue card focused — visibly DIFFERENT card (INS-003)

```
┌─ Inspector ─────────────────────────────────────────────┐
│ ⚠ High air cutting (27%) — sweeping over uncut stock     │
│ ✓ live                                                   │
├──────────────────────────────────────────────────────────┤
│ ┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐ │ ◀ HOLLOW dashed outline (not accent bar)
│ ╎ △ Air cut: sweeping over uncut stock                 ╎ │ ◀ outline glyph (not filled)
│ ╎ Move 305                                             ╎ │
│ ╎ [ ◀ Prev ] [ Next ▶ ] [ Jump ] [ Optimize this op ] ╎ │ ◀ keeps nav (hotspot card has none)
│ └╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘ │
└──────────────────────────────────────────────────────────┘
```

Hotspot = solid frame + left accent bar + `◍`. Issue = dashed hollow frame + `△` + nav row.
Same slot, unmistakably different role — no longer "8-unit fill shift".

---

## M4. Engagement provenance hover (INS-006) + canonical format (INS-004)

```
   Engagement avg 3% · peak 7%   ⓘ
                                 └──hover──────────────────────────────┐
                                 │ Engagement = cylinder-side radial   │
                                 │ width-of-cut fraction. Reads ~10×   │
                                 │ below the algorithmic target — use  │
                                 │ to compare variants, not as an      │
                                 │ absolute under-engagement bar.      │
                                 └─────────────────────────────────────┘
```

Same hover attaches to the card's `avg engage` line and the Span grid Engagement row.
Engagement is `{:.0}%` everywhere; the raw-fraction `{:.2}` in the Span grid is retired.

Canonical hotspot line — identical numeric tail + units in all three places:

```
 focused card lead : m231 · waste 0.84s · peak chip 0.0890 mm
 Top-hotspots row  : m231 · waste 0.84s · peak chip 0.0890 mm
 Span findings row : Hotspot · m231 · waste 0.84s · peak chip 0.0890 mm
                     └prefix only in mixed list┘
```

---

## M5. Span lock toggle on the panel (INS-008)

Lock can now be SET from the Span header, not only released after the ribbon sets it.

```
 ▼ Selected: Z-1.50 contour pass   🔓 follow     ← click toggle to LOCK
 ▼ Selected: Z-1.50 contour pass   🔒 locked     ← now pinned; click to release (follow playhead)
```

Writes `sim.debug.span_scope.span_id` (same field the ribbon writes) — panel and ribbon
are now two equal entry points to the same lock.
