# Pass-2 Redesign — Simulation timeline internals & transport

Area: the bottom panel of the simulation workspace (`crates/rs_cam_viz/src/ui/sim_timeline.rs`)
plus the staleness/capture touch-points it shares with the left op-list
(`sim_op_list.rs`). Surfaces in scope: `sim-verdict-hud`, `sim-boundary-timeline`,
`sim-span-ribbon`, `sim-semantic-band`, `sim-signal-spine`, `sim-op-list`,
`sim-staleness-card`.

Out of scope (pass-1): per-toolpath property tabs, all feeds/speeds surfaces,
fixture-collision fix, Verdict-HUD/Inspector load-rollup reconciliation,
viewport-visibility dedup. Findings here are intrinsic to the timeline/transport
surfaces, not feeds duplication.

---

## 1. Root problem

The bottom panel today is a vertical stack of independently-authored painted
strips and plots, each of which:

- invented its own X-coordinate space (TIM-001),
- attached its own click side-effect (TIM-002),
- attached its own white playhead,
- disclosed its interactivity only through prose tooltips (TIM-004, TIM-005),
- ignored staleness (TIM-009).

Nothing in the stack declares "we are all the same time axis, scrubbed by the
same playhead." So the panel reads as 3-5 look-alike bars that secretly mean and
do different things. The redesign collapses the stack into **one shared time
axis** with **one playhead**, **one click contract**, and a **summary→drill
depth structure** so the five metric graphs stop being a flat dump.

---

## 2. Target structure — one transport spine, three tiers

The panel becomes a single vertically-coherent **Transport spine**. Everything
in it is locked to the same project-global move axis (X = move / total_moves) and
governed by one playhead that paints straight down through every tier. Tiers, top
to bottom:

### Tier 0 — Transport row (unchanged role)
Play/pause/step, elapsed/total, speed multiplier. Keep as-is.

### Tier 1 — Verdict summary (was `sim-verdict-hud`)
The at-a-glance pills (`load / unmodeled / exceeds / approx / collisions /
issues`). Two changes:

- **Pills become the affordance, not a sign pointing at one** (TIM-005). The
  `exceeds` and `collisions` pills get `Sense::click`. Clicking a pill seeks the
  playhead to the *first* offending marker on the timeline below and pulses that
  marker. Retire the tooltip prose "Click the red lines on the boundary timeline
  below to navigate." — the pill now *is* the navigation control. Hover keeps a
  short factual definition only ("Load-limit exceedances"), no instructions.
- **Add a metric-resolved sub-row** (TIM-008). Under the verdict pills, a thin
  row of five tiny per-metric chips (chipload / arc / axial / MRR / feed), each
  coloured by its own worst gate state (green/amber/red). This is the
  "which-of-the-five-is-in-trouble" summary that today forces the user to scroll
  all five plots. Clicking a metric chip scrolls the signal spine to that track
  and flashes it. Summary first; the full plot is the drill.

### Tier 2 — Time axis (the single canonical bar; was `sim-boundary-timeline` + `sim-span-ribbon`)
One painted timeline widget that owns the project-global X axis. It carries:

- **Op segments** (per-toolpath coloured blocks) — top band, as today.
- **Span subdivisions** (DepthPass / Region blocks) — folded *into* the same
  widget as a thin sub-band under the op segments, NOT a separate full-width
  strip with its own playhead (TIM-001). The span ribbon already computes its X
  in global space (`(boundary.start_move + local)/total_moves`), so it merges
  cleanly. Drop its independent white playhead — the single Tier-2 playhead
  serves all bands.
- **Safety markers** (collisions, rapid, tool-load exceedances) — vertical
  ticks, as today, hover-tooltipped.
- **One playhead** painted full-height through op band + span band, with the
  diamond handle.

This widget is the only one in the panel that owns absolute time. The signal
spine (Tier 3) zooms *within* it when a TP is focused but never re-bases the
axis under a different meaning.

#### Click contract on Tier 2 (resolves TIM-002)
A single, mode-aware click contract replaces the two-strips-two-meanings
confusion:

- **Click empty timeline / op band** → seek playhead to that move (primary verb).
- **Click within ~7px of a safety marker** → seek + focus Safety tab (today's
  boundary behaviour, kept, now visibly distinct because markers are the only
  ticks on the bar).
- **Click a span sub-band block** → seek to span start + set chip-row scope
  (today's ribbon behaviour). Because the span band is now a visibly-distinct
  *thinner, indented* sub-band, "click a pass block to scope it" reads as a
  different target than "click the bar to seek" — the roles look distinct, so the
  side-effect is predictable.

Net: same gesture, but the *target sub-region* now telegraphs the side-effect,
instead of two identical-looking adjacent full-width strips doing seek vs scope.

### Tier 3 — Signal spine (was `sim-signal-spine`)
The five metric plots, locked to the Tier-2 X axis (when a TP is focused, zoom to
that TP's range — keep current behaviour, the link-group machinery already does
this). Changes:

- **Empty/absent state gets an on-surface placeholder** (TIM-003). When there is
  no `cut_trace`, do not early-`return` into a void. Paint a single muted
  placeholder row in the spine's slot: "No cutting metrics captured — enable
  *Capture cutting metrics* and re-run." with an inline **Enable & re-run**
  button that flips `sim.metric_options.enabled` and fires `RunSimulation`. The
  capability becomes discoverable from where it's missing, instead of living only
  in a left-panel toggle the user can't see from here.
- **Interactivity becomes visible** (TIM-004). Replace the whole-plot hover-prose
  tooltip with real affordances: a scrub cursor (crosshair / resize-horizontal
  icon) when hovering a track, and a faint painted scrub handle at the playhead
  on the active track. The five-track-readout shared cursor stays. Keep one
  short hover line, but the primary disclosure is the cursor change, not prose.
- **Gate-trip dots become a real drill, not a dead half** (TIM-010). Today
  clicking a gate-trip dot fires `SimJumpToMove` but the `focused_hotspot` half
  is dead because the synthetic index (`10000+i`) never indexes `trace.hotspots`.
  Fix: stop overloading `trace.hotspots.get()`. Carry the gate verdict's own
  toolpath id on `HotspotMarker` and set `sim.debug.focused_hotspot` directly
  from it (or route to the per-toolpath gate detail). A gate-trip dot click now
  both seeks AND focuses the offending gate's detail — the drill-into half works.

---

## 3. Staleness — one freshness contract for the whole spine (TIM-009)

Freshness must be legible *on the readouts*, per redesign principle. Today only
the left card (and right panel) know results are stale; the bottom graphs render
the old trace styled as fresh.

Target: when `sim.is_stale(gui.edit_counter)` is true, the entire Transport
spine adopts a **stale skin** — desaturate the signal-track line colours, overlay
a faint diagonal hatch or a thin "stale — showing last run" ribbon pinned to the
top edge of Tier 1, and disable the gate-trip / metric-chip drill clicks (they'd
point at moves that no longer exist). The single **Re-run** affordance lives once,
visibly, at the top of the spine (mirroring the left card's button) so the user
can refresh from where the stale data is shown. The left-panel card can remain as
the canonical control; the spine's stale skin is the *legibility* fix, not a
second button to maintain.

---

## 4. `sim-op-list` row — separate the four concerns (TIM-006, TIM-007)

Each op row today packs: sim-inclusion checkbox (re-runs sim), viewport
visibility eye/cut/rapid/isolate, whole-card click = jump playback, and (debug
only) a nested span/semantic outline. Four jobs, one card, with a whole-card
click Id overlapping the inner controls.

Target — group by concern within the row, left to right, with the whole-card
empty-space jump retired in favour of an explicit affordance:

```
[☑ run] [name ......................] [👁 ✂ ⟿ ⌖] [▸ jump]
         └ click name = jump playback (one clear target)
```

- **Inclusion checkbox** (`run`) stays leftmost — "what's in the sim run."
- **Name label is the jump target** (not the whole card's empty space). This
  removes the overlapping-Id ambiguity (TIM-006): the jump verb has one explicit
  hit-region (the name), the visibility cluster has its own, and there is no
  invisible card-wide click zone competing with them.
- **Visibility cluster** (eye/cut/rapid/isolate) stays grouped on the right —
  "what's visible in 3D." Already deduped in pass-1, untouched here.
- **Outline disclosure** (debug span/semantic tree) stays behind `sim.debug`
  with its existing kind-specific labels. For TIM-007: the dual-tree behind one
  toggle is already signposted by the differing button label ("Show spans" vs
  "Show semantics") and hover; keep that. The one improvement: prefix the
  expanded tree with a one-line, muted kind banner ("Structural spans" /
  "Legacy semantic trace — spans invalidated") so an expanded tree is
  self-identifying without re-reading the toggle. Low priority; debug-only.

---

## 5. Semantic band retirement / relocation (TIM-001, TIM-002)

The `sim-semantic-band` (lines 1712-1910) is the only strip that genuinely uses a
**toolpath-local** X axis (`move_start / local_total`) while every other strip is
project-global. It is debug-gated, but while visible it is the one strip whose
playhead provably won't align with the others.

Target: **retire it as a third full-width playheaded strip.** Its content
(semantic-kind segments, debug annotations, air-cut / low-engagement issue dots)
moves into the Tier-2 time axis as an additional debug-only sub-band rendered in
the *same project-global X space* (offset each item by `boundary.start_move`
exactly as the span ribbon already does). It then shares the single Tier-2
playhead and the single Tier-2 click contract. Its existing pin-semantic /
route-to-DebugTrace-or-CutQuality-tab behaviour becomes the click side-effect of
*that sub-band's* blocks — distinct target, distinct effect, no fourth divergent
axis.

This kills the last divergent X axis and the last orphan playhead in one move.

---

## 6. What's retired

- The standalone `draw_span_ribbon` full-width strip with its **own white
  playhead** (1390-1398) → merged into Tier-2 as an indented sub-band sharing the
  single playhead.
- The standalone `draw_semantic_band` full-width strip with its **own white
  playhead** (1806-1811) and **toolpath-local X axis** → merged into Tier-2 as a
  project-global debug sub-band (TIM-001).
- Verdict-pill prose "Click the red lines on the boundary timeline below to
  navigate." (1133, 1150) → replaced by clickable pills (TIM-005).
- Signal-spine whole-plot prose tooltip as the *sole* interactivity disclosure
  (804-806) → replaced by cursor change + painted scrub handle (TIM-004).
- Silent early-`return` when no `cut_trace` (217) → replaced by on-surface
  placeholder + enable-&-rerun button (TIM-003).
- Dead `trace.hotspots.get(10000+i)` lookup (459-465) → replaced by direct
  focus from the marker's own toolpath id (TIM-010).
- Whole-card empty-space jump Id on op rows (sim_op_list 368-377) → replaced by
  an explicit name-label / jump-chevron target (TIM-006).

---

## 7. Findings coverage

| Finding | Resolution |
|---------|-----------|
| TIM-001 | One shared global X axis; span + semantic bands merged into Tier-2 as sub-bands; one playhead. |
| TIM-002 | Single mode-aware click contract on Tier-2; side-effect telegraphed by which sub-region is clicked. |
| TIM-003 | On-surface placeholder + enable-&-rerun button where the spine would be empty. |
| TIM-004 | Cursor change + painted scrub handle replace prose-only disclosure. |
| TIM-005 | Verdict pills become clickable navigation; prose instruction retired. |
| TIM-006 | Op row split into run / name-jump / visibility groups; explicit jump target replaces overlapping card-wide Id. |
| TIM-007 | Self-identifying kind banner on expanded outline (debug-only, low). |
| TIM-008 | Metric-resolved chip sub-row in Tier-1 (summary first); click scrolls to that track. |
| TIM-009 | Whole-spine stale skin + single re-run affordance where stale data shows. |
| TIM-010 | Gate-trip dot drill fixed: focus set from marker's own toolpath id. |
