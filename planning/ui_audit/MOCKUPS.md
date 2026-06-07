# rs_cam_viz redesign — assembled ASCII walkthrough

One walkthrough of the redesigned UI, in the order a user encounters it: set up
the job (stock / fixtures) → configure a toolpath across the five concern tabs →
simulate and read diagnostics → export. Sourced from the four domain mockup
files; see `IA_TARGET.md` for the rationale and `MIGRATION.md` for the
per-capability map.

## Shared legend (one visual language everywhere)

```
PROVENANCE CHIPS (status badges — click = "why", never apply):
  [▣ vendor LUT]  / ( vendor 1234 )   green   — embedded vendor cutting table (obs-id)
  [◆ sim-optimized]                   blue    — sim optimizer candidate
  [◷ nomogram]    / { what-if }       violet  — nomogram what-if explorer
  [✎ hand edit]                       grey    — typed by the user
  [▲ formula]                         amber    — formula fallback / floor (no vendor row)
  < project 18000 >                   hollow  — inherited project default / read-only mirror

ACTION GLYPHS (role-distinct — fixes the ⚡ overload):
  ⚡  / ‹↑ 1400›    single-field apply (writes ONE field; chip shows the target)
  ⚡⚡ / [ Apply recommended speeds ]   bulk apply (whole recipe, SPEED only)
  →                "edit at its home" link (read-only mirror; navigates, never writes)

SIM PILLS:
  [ within 6/8 ]      verdict, READ-ONLY   (flat, no cursor change)
  ( exceeds 2/8→ )    verdict, ACTIONABLE  (rounded, hover-fill, trailing →)
  { traces 6 }        observation, READ-ONLY (neutral palette, not verdict ramp)
  ( collisions 3→ )   observation, ACTIONABLE (neutral, alarm-red)

SAFETY:
  🛡  field consumed by the holder-clearance check
  ◇ not checked (grey)   ✓ clear (green)   ✗ hit (red, clickable → seeks marker)
  ＋  create / mirror-trigger    ▸ navigates to authoritative home    ▾/▸ disclosure
```

---

# 1. Job setup

## 1.1 Fixture panel — Z is now a real safety input (P6-003, P3, P4)

```
┌─ Fixture ──────────────────────────────────────────────┐
│ Name   [ Front clamp            ]   ☑ Enabled           │
│ Kind   [ Clamp        ▼ ]                               │
│                                                         │
│ Holder clearance:  ✗ hit            [ Run holder clearance ▸ ] │  ← point-of-need
│   └ clicking ✗ seeks viewport to the collision marker   │     mirror of menu cmd
│                                                         │
│ 🛡 Geometry (safety-checked)                         ▾  │  ← disclosure (P3)
│ ┌─────────────────────────────────────────────────────┐ │
│ │ 🛡 Position  X[ 10.0]  Y[  5.0]  Z[  0.0]            │ │  ← shield marks the
│ │ 🛡 Size      X[ 40.0]  Y[ 20.0]  Z[ 25.0]            │ │     fields the check
│ │ 🛡 Clearance [  3.0]   (inflates all 6 faces)        │ │     actually consumes
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
  Editing any 🛡 field flips the pill back to ◇ not checked — never falsely green.
```

Keep-out panel stays a plain XY form — the two near-clones now read as different jobs:
```
┌─ Keep-out zone ────────────────────────────┐
│ Name   [ Vise body          ]  ☑ Enabled   │
│ Position  X[ 60.0]  Y[ 10.0]               │   ← no 🛡, no Z, no clearance pill
│ Size      X[ 30.0]  Y[ 25.0]               │
└────────────────────────────────────────────┘
```

## 1.2 Two-sided trigger — one capability, two mirror triggers (P2-005)

```
  Setup panel:                          Stock panel:
  ┌──────────────────────────────┐      ┌──────────────────────────────┐
  │ [ ＋ Two-sided setup ]        │      │ [ ＋ Two-sided setup ]        │
  │   edit pins in  Stock ▸       │      │   edit pins in  Stock ▸       │ ← navigates to
  └──────────────────────────────┘      └──────────────────────────────┘   the one home
        both push SetupTwoSided                both push SetupTwoSided
```
Authoritative editing of `flip_axis` / `alignment_pins` lives only in
**Stock › Alignment pins**. The `Stock ▸` chip replaces the old caption prose.

---

# 2. Toolpath configuration — the five concern tabs

## 2.0 New tab bar (replaces [Params][Feeds][Heights][Dressups])

```
┌─ Toolpath: "Pocket — slot 6mm" ──────────────────────────────────────────┐
│  [ Geometry ] [ Feeds & Speeds ] [ Linking ] [ Heights ] [ Dressup ]      │
│    one concern per tab — "Params" catch-all is gone (P2)                   │
└───────────────────────────────────────────────────────────────────────────┘
```

## 2.1 GEOMETRY tab — summary + drill (P3-001/002/003, P2-004, P1-007)

```
┌─ Geometry  (what to cut) ─────────────────────────────────────────────────┐
│  Summary  ───────────────────────────────────────────────────────────     │
│   Pattern        [ Zigzag        ▾ ]    Direction  ( ◉ Climb  ○ Conv )     │
│   Stepover       [  3.00 ] mm   ⚡ ( vendor 1234 )                          │
│   Depth/pass     [  2.00 ] mm   ⚡ ( vendor 1234 )                          │
│   Total depth    [ 12.00 ] mm                                              │
│   ┌──────────── live pattern minimap (P5, value-reactive) ───────────┐    │
│   │  ▕▏▕▏▕▏▕▏▕▏▕▏   zigzag @ 3.0mm stepover                          │    │
│   └────────────────────────────────────────────────────────────────┘    │
│  ▸ Advanced  (tolerance · min-radius · finishing passes · stock-to-leave) │
│  ▸ Machining boundary  (enable · inherit · source · containment · offset) │
│  ▸ Rest machining                                                          │
└───────────────────────────────────────────────────────────────────────────┘
```

adaptive3d (was a 20-control flat grid):
```
┌─ Geometry  (adaptive3d) ──────────────────────────────────────────────────┐
│  Summary  Strategy [ Boundary-walk ▾ ]  Stepover [40%]⚡  Depth/pass [3.0]⚡ │
│  ▸ Advanced  (region ordering · fine-stepdown · mill-shallow · sampling)   │
│  ▸ Conditional  (revealed by Strategy)                                     │
└───────────────────────────────────────────────────────────────────────────┘
   ↑ entry style is NOT here — it lives in Linking (one home, P2-003)
```

Inlay fit becomes a labelled section; drill R-plane relocated + relabelled:
```
│  Inlay Fit  ────────────────────────────────────                          │
│   Pocket depth [..] Glue gap [..] Flat depth [..] Bndry offset [..]        │
│  Drill cycle  ──────────────────────────────────                          │
│   Cycle [ Peck ▾ ]   Peck depth [2.0]   Dwell [0.0]                        │
│   Peck retract (R-plane)  [ 1.0 ] mm    ← was "Retract Z" + apology tooltip│
│  ▸ Advanced                                                                │
│     Finish scallop limit (ball-tip) [0.02] mm  ← was "Scallop height"      │
│                                                  (distinct from Scallop op)│
```

## 2.2 FEEDS & SPEEDS tab — the ONE feeds home (P1, P2-002, P3-005, P4, P7)

```
┌─ Toolpath ▸ Feeds & Speeds ──────────────────────────────────────────────┐
│  SPEED · how fast ───────────────────────────────────────────────────     │
│    Feed      [ 1200 ] mm/min   [▣ vendor LUT]            ‹↑ 1400›          │
│    Plunge    [  400 ] mm/min   [✎ hand edit]            ‹↑ 480›           │
│    Spindle   < project 18000 >  ◯ override → [ _____ ] RPM  ⚡             │
│              └ effective (hollow=inherited) ┘ └ off: disabled ┘            │
│              ↑ ONE precedence widget, both numbers visible (P1-005/P2-006) │
│                                                                            │
│  CUT · how deep / wide ──────────────────────────────────  ⚠ geometry     │
│    DOC       [  2.0 ] mm        [✎ hand edit]      ‹↑ 2.4 · changes cut›  │
│    WOC       [  6.0 ] mm        [✎ hand edit]      ‹↑ 5.4 · changes cut›  │
│    Stock to leave [ 0.2 ] mm    [✎ hand edit]                            │
│                                                                            │
│  Derived ─────────────────────────────────────────────────────────────    │
│    Chipload  0.033 mm/tooth     [▣ vendor LUT]    ← no apply affordance    │
│    Power     ▓▓▓▓▓▓░░░░ 62%      [▣ vendor LUT]                            │
│    MRR       11.5 cm³/min        —                                         │
│                                                                            │
│  [ Apply recommended speeds ]   ← SPEED only; never touches DOC/WOC        │
│  Recipe: 18000 rpm · 0.05 chip · 41 cm³/min    [ ⚡⚡ Suggest all ]         │
│  Details ▸                                                                  │
└────────────────────────────────────────────────────────────────────────────┘
```
Spindle widget, override ON (precedence visible, no prose):
```
│  Spindle  <̶ ̶p̶r̶o̶j̶e̶c̶t̶ ̶1̶8̶0̶0̶0̶ ̶>  ◉ override → [ 22000 ] RPM  ⚡            │
│           └ dimmed strikethrough mirror ┘  └ active, grey "hand" pill ┘    │
```

`Details ▸` drawer (replaces the modal + Feeds-tab body; read-mostly, NO Apply rows):
```
┌─ Feeds & Speeds · Details ───────────────────────────────────  [ Close ▴ ] ┐
│  ▾ Current vs recommended                                                  │
│      field  current      recommended    source                            │
│      Feed   1200 mm/min  1400 mm/min   [▣ vendor LUT (obs#A21)]           │
│      RPM    18000        16000         [▲ formula]                        │
│      DOC    2.0 mm       2.4 mm  ⚠geom [▣ vendor LUT (obs#A21)]           │
│      (read-only — apply from the summary above)                            │
│  ▸ Why this value?    ← click any chip above to jump here (P7-005)         │
│  ▸ Charts (engagement · feed-vs-RPM · vendor band)                         │
│  ▸ What-if explorer   (Apply stamps [◷ nomogram]; SPEED only)             │
│  ▸ Raw vendor LUT     (one canonical table, mirrored read-only — P1-006)   │
└────────────────────────────────────────────────────────────────────────────┘
```

Alternate-engine modals (kept, engine-attributed — P4-006):
```
┌─ Sim optimizer · Profile_3 ──────────────────────────────────  [ Close ] ┐
│  cand #2  1500  15000  2.2  5.8  -18%  ✓ within  [◆ Apply sim-optimized]  │
│  cand #5  1650  15000  2.4  6.0   -9%  ⚠ exceeds [  blocked  ]            │
│  Applied values are stamped [◆ sim-optimized] on the toolpath.            │
└────────────────────────────────────────────────────────────────────────────┘
┌─ Feeds · all toolpaths ──────────────────────────────────────  [ Close ] ┐
│  ☑ 1 Pocket   1200→1400  +17%  [▣ vendor LUT]                            │
│  ☑ 2 Profile   900→ 900   —    [▣ vendor LUT]                            │
│  [ ▣ Apply vendor LUT to selected ]   [ ▣ Apply vendor LUT to all ]      │
└────────────────────────────────────────────────────────────────────────────┘
```

## 2.3 LINKING tab — entry style has ONE home (P2-003)

```
┌─ Linking  (how moves connect) ────────────────────────────────────────────┐
│  Entry & Exit                                                              │
│   Entry style  [ Helix ▾ ]   Ramp angle [ 3° ]                            │
│     ▸ Helix    (radius · pitch — revealed when Entry = Helix)             │
│   Lead-in/out  [✓]   radius [ 2.0 ]                                       │
│  Move optimization                                                         │
│   Link moves [✓]   Retract strategy [ Smart ▾ ]   Optimize rapids [✓]     │
│   Arc fitting (G2/G3) [✓]                                                  │
└───────────────────────────────────────────────────────────────────────────┘
  For adaptive3d, Entry style reads/writes Adaptive3dConfig directly.
  The old generic Dressups entry-style combo (live-but-inert) is GONE.
```

## 2.4 DRESSUP tab — cosmetic / post edge work

```
┌─ Dressup  (cosmetic / post edge work) ────────────────────────────────────┐
│  Holding tabs   Count [ 4 ]   Width [ 6.0 ]   Height [ 2.0 ]              │
│  Dogbone overcuts [✓]                                                      │
│  Path quality   Feed-rate optimization [✓]   Tolerance [ 0.01 ]          │
└───────────────────────────────────────────────────────────────────────────┘
```

## 2.5 HEIGHTS tab — gains effective-safe-Z mirror (P2-004)

```
┌─ Heights  (Z reference planes) ───────────────────────────────────────────┐
│   Clearance   [ +5.0 ] from [ Stock top ▾ ]        ┌──────────────┐       │
│   Retract     [ +2.0 ] from [ Stock top ▾ ]        │   ═══ clear  │       │
│   Feed        [ +1.0 ] from [ Stock top ▾ ]        │   ─── retract│       │
│   Top         [  0.0 ] from [ Stock top ▾ ]        │  ▓▓▓▓ stock  │       │
│   Bottom      [-12.0 ] from [ Model bot ▾ ]        │  drag side-view       │
│                                                    └──────────────┘       │
│   ──────────────────────────────────────────────────────────────────     │
│   Effective safe-Z   < project 6.0 >  → edit in Post                       │
│        ↑ read-only mirror, hollow pill + → link (was Post apology tooltip) │
└───────────────────────────────────────────────────────────────────────────┘
```

## 2.6 POST panel — distinct spindle role + override link (P1-005, P2-006, P1-004)

```
┌─ Post / output ───────────────────────────────────────────────────────────┐
│   Format            [ GRBL ▾ ]                                             │
│   Project spindle default   [ 18000 ] RPM   → per-op override on Feeds tab │
│   Safe-Z (project)  [ 6.0 ] mm        (mirrored read-only on Heights tab)  │
│   Safe rapids (G0→G1) [✓]   Rapid-replacement speed [ 5000 ]              │
│        ↑ "high feedrate" relabelled out of feed vocabulary (P2-007)        │
└───────────────────────────────────────────────────────────────────────────┘
  Panel edits route through SetPostConfig → session (canonical); gui.post mirrors.
```

---

# 3. Simulation & diagnostics

## 3.1 Verdict HUD — per-toolpath /T counts, clickable (P4-001/002, P5-001, P6-004)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  ── LOAD (vendor-LUT gate) ──────────  ── SIM RUN ───────────────────────  │
│  [ within 6/8 ]  ( exceeds 2/8→ )      ( collisions 3→ )   ( issues 41→ )   │
│  [ unmodeled 0/8 ]                     { traces 6 }                         │
└──────────────────────────────────────────────────────────────────────────┘
   verdict family (green/red/amber ramp)   observation family (neutral)
  hover ( exceeds 2/8→ ):  "Load-limit exceedances"   ← NAME only, no nav prose
  click ( exceeds 2/8→ ):  jumps to first exceed marker + opens drill (below)
  { traces 6 } flat/read-only;  { collisions 0/safe } when zero (read-only green)
```
within/exceeds/unmodeled read EXACTLY what Project overview reads (both from
`summary()`); the shared `/8` proves the same denominator.

Exceeds-pill drill (progressive disclosure):
```
( exceeds 2/8→ )  ◀ clicked
   ┌──────────────────────────────────────────────┐
   │ 2 exceedances                                 │
   │ • Back Rough   · power      · high     →jump  │  each row clickable
   │ • Finish Pass  · deflection · (L/D)    →jump  │
   └──────────────────────────────────────────────┘
```

## 3.2 Viewport overlay › Show ▼ — the ONE visibility home (P4-005)

```
[Show ▼]
 ┌───────────────────────────┐
 │ [x] Stock                 │   single label set, shared everywhere
 │ [x] Fixtures              │
 │ [x] Curves (DXF/SVG)      │
 │ [x] Cutting moves         │   ("Paths (cutting)" → "Cutting moves")
 │ [x] Rapid moves           │   ("Rapids"          → "Rapid moves")
 │ [x] Collisions            │
 └───────────────────────────┘
```

## 3.3 Inspector › View — appearance only; ByOperation wired (P4-005, P6-002, P5-004)

```
▼ View
  Stock appearance
   Opacity: [======|====] 0.6
   Stock color: [ Solid          ▼ ]
                 ├ Solid  ├ Deviation  ├ By Height  └ By Operation  ← WIRED (P6-002)
   ⓘ Deviation needs fresh sim data:        ← only in Deviation mode w/ no data
      ( ▶ Re-run simulation )               ← point-of-need button (P5-004)
  (raw show/hide for Stock · Cutting · Rapid moves → Viewport "Show ▼" toolbar)
```
The three duplicate visibility checkboxes are gone from here.

## 3.4 Global vs per-toolpath visibility layering (P4-004)

```
Global "Cutting moves" ON:                 Global "Cutting moves" OFF:
  ◉ Back Rough  [👁] [ C ][ R ]              ◉ Back Rough  [👁] [·C·][ R ]
  ◉ Finish Pass [👁] [ C ][ R ]              ◉ Finish Pass [👁] [·C·][ R ]
        C/R live, toggle this TP                  C greyed: global gate off,
                                                  this per-row override is inert
```
`[·C·]` = disabled/greyed; `[ C ]` = active. No "toggled and nothing happened" surprise.

## 3.5 Empty-state card — inline Run (P5-003)

```
┌ Ready to simulate ───────────────────────┐
│        ( ▶ Run Simulation )               │  ← matches the sibling "Go to
└───────────────────────────────────────────┘     Toolpaths" button (was prose)
```

---

# 4. Shell affordances

## 4.1 Edit › Delete Selected wired (P6-001)

```
  Edit ▾  (a toolpath is selected)          Edit ▾  (nothing deletable selected)
  │ Delete Selected          Del  │ ←active  │ Delete Selected          Del  │ ←greyed
  →emits RemoveToolpath(id)                   for a real reason (no selection)
```
The `Del` label is now truthful: menu click and the keyboard do the same thing.

## 4.2 Post sync boundary (P1-004 — invisible, by design)

```
  BEFORE:  panel edit → gui.post →(only at save)→ session   ← MCP sees stale value
  AFTER:   panel edit → SetPostConfig → session →(mirror)→ gui.post   ← always agree
```
