# Pass-2 Mockups — Simulation timeline & transport

Conventions: `│` is the single shared playhead (paints through EVERY tier at the
same X). `▮` filled / `▯` empty pill. Time axis is project-global throughout.

---

## A. The unified Transport spine (default, metrics captured, fresh)

```
┌─ TRANSPORT ─────────────────────────────────────────────────────────────────┐
│ ◄  ▶  ►     0:42 / 3:18     Speed [───●──────] 4×                            │  Tier 0
├──────────────────────────────────────────────────────────────────────────────┤
│ ✓ load 18   ⚠ unmodeled 2   ✕ exceeds 1 ►   ~ approx 3   collisions 0   …    │  Tier 1 verdict
│ metric:  ▮chipload  ▯arc  ▮axial  ▯MRR  ▮feed        ← red=in trouble         │  Tier 1 metric chips (TIM-008)
│            click a chip → spine scrolls+flashes that track                    │
├──────────────────────────────────────────────────────────────────────────────┤
│ TIME ─────────────────────────│──────────────────────────────────────────    │  Tier 2 (single axis)
│ ┌Pin Drill─┐┌─Back Rough──────┴───┐┌─Finish────────────────────────────┐    │   op band
│ └──────────┘└────────────────╳────┘└──────────────────────╳────────────┘    │   ╳ = safety marker
│ ┌pass┐┌pass┐┌──pass──┐┌──pass──┐  ┌──pass──┐┌──pass──┐ ← indented span sub-band│   span band (was ribbon)
│      ▲ click a pass block = scope+seek   ╳ click marker = seek+Safety tab     │
└──────────────────────────────────────────────────────────────────────────────┘
   one playhead │ runs straight down through verdict, op band, span band, spine.
   one X axis. one click contract. (TIM-001, TIM-002)

┌─ SIGNAL SPINE (zoomed to focused TP) ─────────────────────────────────────────┐  Tier 3
│ Playing: Back Rough                                          ⤡ scrub cursor   │
│ chipload   ╱╲      ╱╲╱╲    ●gate-trip          [red band]    │               │  TIM-004: cursor + handle
│          ╱    ╲╱╲╱      ╲╱      ╲___                          │◆ scrub handle  │  TIM-010: dot drills in
│ ─────────────────────────────────────────────────────────────│──────────────  │
│ arc        ___╱▔▔▔╲___╱▔▔╲____                                │               │
│ ─────────────────────────────────────────────────────────────│──────────────  │
│ axial DOC  ▔▔▔▔▔▔╲______╱▔▔▔▔▔▔                               │               │
└──────────────────────────────────────────────────────────────────────────────┘
```

Key wins shown:
- ONE `│` playhead, not one per strip.
- Span blocks are an *indented, thinner* sub-band of the same Tier-2 bar — a
  visibly different target from "click the bar to seek" (TIM-002), no separate
  full-width strip with its own playhead (TIM-001).
- `✕ exceeds 1 ►` and metric chips are clickable affordances, not prose (TIM-005,
  TIM-008).
- Signal track shows a scrub cursor + handle (`◆`) instead of a hidden tooltip
  (TIM-004); the gate-trip `●` now drills into the gate detail (TIM-010).

---

## B. Empty signal spine — capture was OFF (TIM-003)

Before (silent void — section just disappears):
```
│ TIME ─────────────────────────│──────────────────────────────────────────    │
│ ┌Pin Drill─┐┌─Back Rough──────┴───┐┌─Finish─────────────────────────────┐    │
└──────────────────────────────────────────────────────────────────────────────┘
                          (nothing here — graphs gone, no hint why)
```

After (on-surface placeholder where the spine would be):
```
┌─ SIGNAL SPINE ────────────────────────────────────────────────────────────────┐
│                                                                                │
│     No cutting metrics captured for this run.                                  │
│     [ ✓ Enable capture & re-run ]   ← flips metric_options.enabled + RunSim    │
│                                                                                │
└──────────────────────────────────────────────────────────────────────────────┘
```
The capability is discoverable from where it's missing; the user no longer has to
know a left-panel toggle exists.

---

## C. Stale skin — params changed since last run (TIM-009)

```
┌─ ⚠ STALE — showing last run ──────────────────────────[ Re-run simulation ]──┐  pinned ribbon
│ ◄  ▶  ►     0:42 / 3:18     Speed [───●──────] 4×                            │
├──────────────────────────────────────────────────────────────────────────────┤
│ ✓ load 18   ⚠ unmodeled 2   ✕ exceeds 1    ~ approx 3   …   (clicks disabled) │
├──────────────────────────────────────────────────────────────────────────────┤
│ TIME ░░░░░░░░░░░░░░░░░░░░░░░░░│░░░░░░░░░░░░░░░░░░░ (hatched / desaturated)     │
│ ░┌Pin Drill┐░┌Back Rough─────┴──┐░┌Finish───────────────────────────┐░       │
└──────────────────────────────────────────────────────────────────────────────┘
   chipload track lines desaturated; gate-trip + metric-chip drills disabled
   (they'd point at moves that no longer exist). Re-run lives where stale data is.
```

The graphs no longer style themselves as fresh; freshness is legible on the
readout itself, not only in the far-left card.

---

## D. Op-list row — four concerns separated (TIM-006)

Before (one card, whole empty space = jump, overlaps inner controls):
```
┌────────────────────────────────────────────────────┐
│ ☑  Back Rough            👁 ✂ ⟿ ⌖   ▸ spans         │  ← click ANYWHERE empty = jump
└────────────────────────────────────────────────────┘     (Id overlaps the controls)
```

After (explicit, grouped targets):
```
┌────────────────────────────────────────────────────┐
│ ☑run  Back Rough ↵          │  👁 ✂ ⟿ ⌖  │  ▸jump  │
│       └ click NAME = jump      └ 3D vis ─┘   chevron │
└────────────────────────────────────────────────────┘
  run-inclusion │ jump-playback │ 3D-visibility — three clear homes,
  no invisible card-wide click zone fighting the inner controls.
```

(debug-only) expanded outline gets a self-identifying banner (TIM-007):
```
│       ▾ Structural spans                              │   ← or "Legacy semantic
│         ├ DepthPass 0 · z=-2.00                       │      trace — spans invalidated"
│         ├ DepthPass 1 · z=-4.00                       │
```
