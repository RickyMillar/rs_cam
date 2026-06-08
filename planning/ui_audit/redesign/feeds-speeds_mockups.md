# ASCII mockups — Feeds & speeds redesign

Demonstrates (not describes) the fixes. Legend for provenance chips — ONE
language, used identically on every surface below:

```
  [▣ vendor LUT]   green    — value came from the embedded vendor cutting table
  [◆ sim-optimized] blue    — value came from the sim optimizer
  [◷ nomogram]     violet   — value came from the what-if explorer
  [✎ hand edit]    grey     — value typed by the user
  [▲ formula]      amber    — formula fallback / edge-radius floor (no vendor row)
```

Three DISTINCT roles for the three things that used to all be "⚡":

```
  value-bearing single-field apply ......  ‹↑ 18000›   (shows the target value, applies ONE field)
  bulk apply ............................  [ Apply recommended speeds ]   (labeled button, SPEED only)
  provenance status badge ...............  [▣ vendor LUT]   (a chip; click = open "why", NOT apply)
```

---

## TIER 0 — Toolpath tab, Feeds & Speeds section (the ONE home)

Collapsed-detail summary. SPEED and CUT are visually separated (P2-002). Every
value carries a provenance chip (P7). Single-field applies show their value
(P4-003). One bulk button, speed-scoped (P1-001).

```
┌─ Toolpath ▸ Feeds & Speeds ──────────────────────────────────────────────┐
│                                                                            │
│  SPEED · how fast ───────────────────────────────────────────────────     │
│    Feed      [ 1200  ] mm/min   [▣ vendor LUT]            ‹↑ 1400›         │
│    Plunge    [  400  ] mm/min   [✎ hand edit]             ‹↑ 480›          │
│    RPM       [18000  ]          [▲ formula]               ‹↑ 16000›        │
│                                                                            │
│  CUT · how deep / wide ──────────────────────────────────  ⚠ geometry     │
│    DOC       [  2.0  ] mm        [✎ hand edit]      ‹↑ 2.4 · changes cut›  │
│    WOC       [  6.0  ] mm        [✎ hand edit]      ‹↑ 5.4 · changes cut›  │
│    Stock to leave [ 0.2 ] mm     [✎ hand edit]                            │
│                                                                            │
│  Derived ─────────────────────────────────────────────────────────────    │
│    Chipload  0.033 mm/tooth      [▣ vendor LUT]                            │
│    Power     ▓▓▓▓▓▓░░░░ 62%       [▣ vendor LUT]                           │
│    MRR       11.5 cm³/min         —                                        │
│                                                                            │
│  [ Apply recommended speeds ]   ← SPEED only; never touches DOC/WOC        │
│                                                                            │
│  Details ▸                                                                  │
└────────────────────────────────────────────────────────────────────────────┘
```

Why this beats today:
- **P2-002**: SPEED and CUT are separate blocks. `Apply recommended speeds`
  applies feed/plunge/rpm only. A geometry change is *only* reachable via the
  per-field `‹↑ 2.4 · changes cut›` chip, which says so. No silent geometry
  rewrite.
- **P4-003**: the bulk button is a labeled button; per-field applies are
  value-bearing chips (`‹↑ 1400›`); provenance is a separate badge. Three looks.
- **P4-007**: Derived rows (chipload/power/MRR) have NO apply chip and sit under
  a `Derived` heading — obviously read-only vs the editable rows above.
- **P7-002**: Feed reads `▣ vendor LUT`, RPM reads `▲ formula`, Plunge reads
  `✎ hand edit` — each field's OWN origin, not one shared enum.
- **P5**: zero explanatory paragraphs. The `⚠ geometry` marker and the
  `· changes cut` suffix do the explaining as affordances.

---

## TIER 1 — "Details ▸" drawer (in place; replaces the modal + Feeds-tab body)

Opens inline. Read-mostly. Editing still happens in Tier 0 — there are NO Apply
rows here (kills the second editor, P1-003). Each section is its own collapse.

```
┌─ Feeds & Speeds · Details ───────────────────────────────────  [ Close ▴ ] ┐
│                                                                            │
│  ▾ Current vs recommended                                                  │
│      field    current      recommended     source                         │
│      Feed     1200 mm/min   1400 mm/min    [▣ vendor LUT (obs#A21)]        │
│      RPM      18000         16000          [▲ formula]                     │
│      DOC      2.0 mm        2.4 mm  ⚠geom  [▣ vendor LUT (obs#A21)]        │
│      (read-only — apply from the Feeds & Speeds summary above)             │
│                                                                            │
│  ▸ Why this value?           ← click any chip above to jump here           │
│  ▸ Charts (engagement · feed-vs-RPM · vendor band)                         │
│  ▸ What-if explorer                                                        │
│  ▸ Raw vendor LUT                                                          │
└────────────────────────────────────────────────────────────────────────────┘
```

`▾ Why this value?` expanded (P7-005 depth, now reachable from the summary chip):

```
│  ▾ Why this value?  — Feed                                                  │
│      vendor row obs#A21  ▣   →  chipload 0.033 mm/tooth                     │
│        × flutes 2  × RPM 16000             = 1056 mm/min base               │
│        × rigidity derate 0.92  × wear 1.0  = 971  → rounded 1000... etc.   │
│      [ open Raw vendor LUT ]                                               │
```

- **P7-005**: full derate math is one click from the summary chip, in place — no
  separate window. **P1-006**: the raw LUT is mirrored here read-only, one
  canonical source.

`▾ What-if explorer` expanded (the nomogram drag tool — KEPT, now attributed):

```
│  ▾ What-if explorer                                                         │
│      RPM  ├──────●─────┤   feed ├────────●──┤      ◷ exploring             │
│      [feed-vs-RPM plot, draggable point ●, vendor band shaded]             │
│      chipload 0.041 ✓   power 71% ✓                                        │
│      [ ◷ Apply what-if to speeds ]   ← stamps [◷ nomogram]; SPEED only     │
```

- **P2-002**: even the explorer applies SPEED only; it cannot silently move DOC/WOC.
- **P7-004**: its Apply stamps `◷ nomogram` so the value is later distinguishable.

---

## TIER 2 — alternate-engine modals (kept; engine-attributed, P4-006)

Sim optimizer — note the Apply button is engine-prefixed, never bare "Apply":

```
┌─ Sim optimizer · Profile_3 ──────────────────────────────────  [ Close ] ┐
│  candidate   feed    rpm    DOC    WOC    cycle Δ   verdict                │
│  ▸ cand #2  1500   15000   2.2    5.8    -18%      ✓ within   [◆ Apply sim-optimized] │
│  ▸ cand #5  1650   15000   2.4    6.0     -9%      ⚠ exceeds  [  blocked  ]│
│                                                                            │
│  Applied values will be stamped  [◆ sim-optimized]  on the toolpath.       │
└────────────────────────────────────────────────────────────────────────────┘
```

Feeds project rollup — engine-prefixed batch apply (vs the optimizer's `◆`):

```
┌─ Feeds · all toolpaths ──────────────────────────────────────  [ Close ] ┐
│  ☑ tp  op         feed→rec     speedup   source                           │
│  ☑ 1   Pocket    1200→1400    +17%      [▣ vendor LUT]                    │
│  ☑ 2   Profile    900→ 900     —        [▣ vendor LUT]                    │
│  ☐ 3   Adaptive  1800→2100    +17%      [▣ vendor LUT]                    │
│                                                                            │
│  [ ▣ Apply vendor LUT to selected ]   [ ▣ Apply vendor LUT to all ]       │
└────────────────────────────────────────────────────────────────────────────┘
```

- **P4-006**: `[◆ Apply sim-optimized]` vs `[▣ Apply vendor LUT to all]` — the
  button itself names the engine and carries the same glyph that will mark the
  stored value. No bare "Apply", and the modal that USED to also edit
  (`feeds-modal-toolpath`) no longer has Apply rows at all.

---

## Before → after, the ⚡ overload (P4-003) at a glance

```
  BEFORE                                AFTER
  ⚡  (per field)                        ‹↑ 1400›            value-bearing chip
  ⚡ Suggest all                         [ Apply recommended speeds ]  labeled button
  ⚡ (colored, doubling as status)       [▣ vendor LUT]      status badge (not clickable-to-apply)
```

Distinct roles now read as distinct controls — which is the whole point of P4.
