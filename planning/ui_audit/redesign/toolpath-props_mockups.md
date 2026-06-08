# ASCII Mockups — Toolpath properties redesign

Demonstrates "UI wins, not prose": every former apology tooltip / explanatory
hint is replaced by a visible affordance. Distinct roles get distinct glyphs.

Legend for provenance pills (one vocabulary, §5 of the SPEC):
```
( vendor 1234 )   green filled   — vendor LUT, obs-id
[ sim ⭐ ]        blue filled    — sim optimizer candidate
{ what-if }       violet filled  — nomogram explore
  hand            grey filled    — hand edit / override
< project >       hollow outline — inherited project default / read-only mirror
```
Action glyphs are now ROLE-DISTINCT (fixes P4 ⚡-overload):
```
⚡   = single-field Suggest (writes ONE field)
⚡⚡  = Suggest all       (writes the whole recipe)   ← visibly doubled, never confusable with ⚡
→    = "edit at its home" link  (read-only mirror, navigates, never writes here)
```

---

## A. New tab bar (replaces [Params][Feeds][Heights][Dressups])

```
┌─ Toolpath: "Pocket — slot 6mm" ──────────────────────────────────────────┐
│  [ Geometry ] [ Feeds & Speeds ] [ Linking ] [ Heights ] [ Dressup ]      │
│    ^^^^^^^^                                                                │
│    one concern per tab — "Params" catch-all is gone (P2 / SPEC §1)        │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## B. GEOMETRY tab — summary + drill (fixes P3-002, P3-001, P3-003)

```
┌─ Geometry  (what to cut) ─────────────────────────────────────────────────┐
│                                                                            │
│  Summary  ───────────────────────────────────────────────────────────     │
│   Pattern        [ Zigzag        ▾ ]    Direction  ( ◉ Climb  ○ Conv )     │
│   Stepover       [  3.00 ] mm   ⚡ ( vendor 1234 )                          │
│   Depth/pass     [  2.00 ] mm   ⚡ ( vendor 1234 )                          │
│   Total depth    [ 12.00 ] mm                                              │
│                                                                            │
│   ┌──────────── live pattern minimap (P5, value-reactive) ───────────┐    │
│   │  ▕▏▕▏▕▏▕▏▕▏▕▏   zigzag @ 3.0mm stepover                          │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                                                                            │
│  ▸ Advanced  (tolerance · min-radius · finishing passes · stock-to-leave) │
│  ▸ Rest machining                                                          │
└───────────────────────────────────────────────────────────────────────────┘
```

Contrast — the OLD Params tab this replaces (one flat grid, mixed concerns):
```
   OLD:  Pattern | Stepover | Depth/pass | Feed Rate | Plunge | Spindle RPM
         | Climb | Total depth | Tolerance | Min radius | Finishing | ...
         (everything always visible, geometry+feeds+strategy interleaved)
```

### B2. adaptive3d Geometry (fixes P3-001 — 20-control grid)
```
┌─ Geometry  (adaptive3d) ──────────────────────────────────────────────────┐
│  Summary                                                                   │
│   Strategy   [ Boundary-walk ▾ ]   Stepover [ 40% ]⚡   Depth/pass [3.0]⚡  │
│  ▸ Advanced  (region ordering · fine-stepdown · mill-shallow · sampling)   │
│  ▸ Conditional  (revealed by Strategy)                                     │
└───────────────────────────────────────────────────────────────────────────┘
        ↑ entry style is NOT here — it lives in Linking (one home, P2-003)
```

### B3. Inlay Geometry (fixes P3-003 — fit params now a labelled section)
```
│  Inlay Fit  ────────────────────────────────────────────                  │
│   Pocket depth [..] Glue gap [..] Flat depth [..] Bndry offset [..]        │
│  ▸ Advanced  (flat-tool radius · tolerance)                                │
```

### B4. Drill Geometry — R-plane relocated here (fixes P2-004)
```
│  Drill cycle  ──────────────────────────────────────────                  │
│   Cycle [ Peck ▾ ]   Peck depth [2.0]   Dwell [0.0]                        │
│   Peck retract (R-plane)  [ 1.0 ] mm        ← was "Retract Z" + apology    │
│                                               tooltip in Params; relabelled│
│                                               + grouped, no prose (P5)     │
```

### B5. DropCutter Geometry — scallop relabel (fixes P1-007)
```
│  ▸ Advanced                                                               │
│     Finish scallop limit (ball-tip)  [ 0.02 ] mm   ← was "Scallop height",│
│                                          now distinct from the Scallop op │
```

---

## C. FEEDS & SPEEDS tab — summary recipe + drill (fixes P3-005, hosts §3)

```
┌─ Feeds & Speeds  (how fast) ──────────────────────────────────────────────┐
│                                                                            │
│  Feed rate    [ 1800 ] mm/min   ⚡ ( vendor 1234 )                          │
│  Plunge rate  [  600 ] mm/min   ⚡ ( vendor 1234 )                          │
│                                                                            │
│  Spindle    < project 18000 >   ◯ override → [ ______ ] RPM   ⚡           │
│             └ effective (hollow=inherited) ┘ └ off: disabled ┘            │
│             ↑ ONE precedence widget, both numbers visible (P1-005/P2-006) │
│                                                                            │
│  Recipe:  18000 rpm · 0.05 chip · 41 cm³/min        [ ⚡⚡ Suggest all ]    │
│           └─ one-line summary chip ─┘                  ↑ doubled glyph =   │
│                                                          whole-recipe scope│
│                                                          (P4, not ⚡)       │
│                                                                            │
│  ▸ How is this calculated?   (formula breakdown — collapsed)  ← P3-005    │
│  ▸ Engagement diagram                                                      │
│  ▸ Vendor cutting data                                                     │
└───────────────────────────────────────────────────────────────────────────┘
```

Spindle widget, override ON (precedence becomes visible, no prose):
```
│  Spindle    <̶ ̶p̶r̶o̶j̶e̶c̶t̶ ̶1̶8̶0̶0̶0̶ ̶>   ◉ override → [ 22000 ] RPM   ⚡           │
│             └ dimmed strikethrough mirror ┘  └ active, grey "hand" pill ┘  │
```

---

## D. LINKING tab — entry style has ONE home (fixes P2-003)

```
┌─ Linking  (how moves connect) ────────────────────────────────────────────┐
│  Entry & Exit                                                              │
│   Entry style  [ Helix ▾ ]   Ramp angle [ 3° ]                            │
│     ▸ Helix    (radius · pitch — revealed when Entry = Helix)             │
│   Lead-in/out  [✓]   radius [ 2.0 ]                                       │
│                                                                            │
│  Move optimization                                                         │
│   Link moves [✓]   Retract strategy [ Smart ▾ ]   Optimize rapids [✓]     │
│   Arc fitting (G2/G3) [✓]                                                  │
└───────────────────────────────────────────────────────────────────────────┘
```
For **adaptive3d**, Entry style here reads/writes `Adaptive3dConfig` directly.
The old generic Dressups entry-style combo (live-but-inert no-op) is GONE — no
second entry-style control exists for any op (P2-003, SPEC §6/§7).

---

## E. HEIGHTS tab — gains effective-safe-Z mirror (fixes P2-004)

```
┌─ Heights  (Z reference planes) ───────────────────────────────────────────┐
│   Clearance   [ +5.0 ] from [ Stock top ▾ ]        ┌──────────────┐       │
│   Retract     [ +2.0 ] from [ Stock top ▾ ]        │   ═══ clear  │       │
│   Feed        [ +1.0 ] from [ Stock top ▾ ]        │   ─── retract│       │
│   Top         [  0.0 ] from [ Stock top ▾ ]        │  ▓▓▓▓ stock  │       │
│   Bottom      [-12.0 ] from [ Model bot ▾ ]        │  drag side-view       │
│                                                    └──────────────┘       │
│   ─────────────────────────────────────────────────────────────────      │
│   Effective safe-Z   < project 6.0 >  → edit in Post                       │
│        ↑ read-only mirror, hollow pill + → link. Post-clamp value shown   │
│          at the point you set clearance. (was: Post apology tooltip, P5)  │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## F. POST panel — distinct spindle role + override link (fixes P1-005, P2-006)

```
┌─ Post / output ───────────────────────────────────────────────────────────┐
│   Format            [ GRBL ▾ ]                                             │
│   Project spindle default   [ 18000 ] RPM   → per-op override on Feeds tab │
│        ↑ relabelled "Project …default" (was "Spindle Speed:") +           │
│          read-only → link to the override — precedence legible both ends   │
│   Safe-Z (project)  [ 6.0 ] mm        (mirrored read-only on Heights tab)  │
│        ↑ apology tooltip removed; the Heights mirror carries the meaning   │
│   Safe rapids (G0→G1) [✓]   High feedrate [ 5000 ]                         │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## G. Distinct-roles cheat-sheet (P4 — the load-bearing visual contract)

```
SAME LOOK, DIFFERENT ROLE — now separated:
  ⚡   single-field Suggest      ≠   ⚡⚡  Suggest-all     (glyph doubled)
  Post "Project spindle default" ≠   Feeds two-state precedence widget
  "Peck retract (R-plane)"       ≠   "Clearance/Retract" Z planes (Heights)
  "Scallop height" (Scallop op)  ≠   "Finish scallop limit" (DropCutter)

SAME ROLE, SAME LOOK — now consistent:
  every provenance pill uses the §5 vocabulary on every tab
  every read-only mirror uses the hollow < > pill + → link, nowhere else
```
