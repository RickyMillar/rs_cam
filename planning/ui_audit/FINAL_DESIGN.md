# rs_cam_viz — Final Design (cohesive walkthrough)

> The unified target, merging pass 1 (feeds/tabs) and pass 2 (the rest), drawn
> from the shared component layer in `ARCHITECTURE.md`. Read top-to-bottom in the
> order a user meets the app. Every surface is annotated with the **components** it
> is built from — so you can see the same `ProvenanceBadge`, `CountPill`,
> `named_section`, and `ValueRow` recur everywhere instead of being re-invented.
> Finding ids in `[...]` trace each fix back to the audit.

---

## Component legend (one vocabulary, used on every surface below)

```
PROVENANCE  (ProvenanceBadge — status only, click = "why", never applies)
   [▣ vendor 1234]  green    vendor LUT (obs-id)        [◆ sim]   blue   optimizer
   [◷ what-if]      violet   nomogram explorer          [✎ edit]  grey   typed
   [▲ formula]      amber    formula fallback / floor    〈 18000 〉       inherited / mirror

ACTIONS  (SuggestButton — three roles, never confusable)
   ⚡                 apply ONE field (value chip shows target)
   ⚡⚡ Apply …        apply WHOLE recipe (doubled glyph + label)
   → edit in X       mirror link (navigates, never writes)

SIM PILLS  (CountPill — family × role)
   [ within 6/8 ]    verdict · read-only   (flat)        denom /T = same producer proof
   ( exceeds 2/8 → ) verdict · actionable  (rounded, hover-fill, →, jumps)
   { traces 6 }      observation · read-only             ( collisions 3 → ) observation · actionable

SECTIONS  (UiExt)         ▾ open · ▸ collapsed (disclosure) · ── Title ── (named_section)
FRESHNESS (FreshnessGate) ░ dimmed + "⟳ stale" badge when the sim that produced a number is stale
SAFETY                    🛡 field consumed by collision check · ◇ not checked ✓ clear ✗ hit
```

---

## 1. Shell

```
┌ rs_cam ─────────────────────────────────────────────────────────────────────┐
│ File  Edit  Toolpath  View  Help          [ Setup ]  [ Toolpaths ]  [ Simulation ] │ ← workspace tabs
├──────────────┬──────────────────────────────────────────────┬───────────────┤
│ LEFT          │ VIEWPORT                                       │ RIGHT          │
│ context nav   │  (3D model / stock / toolpaths / sim)          │ Properties /   │
│ (queue,setup) │  overlay ▸ [ Show ▾ ]  [ ⟲ ]  [ ⌖ ]            │ Inspector      │
├──────────────┴──────────────────────────────────────────────┴───────────────┤
│ STATUS BAR   ✓ clear · 0 collisions · gen 12/12 · 3.4 min        [⟳ stale]     │
└──────────────────────────────────────────────────────────────────────────────┘
```
- Workspace tabs unchanged. `project_tree.rs` is **deleted** — it was dead nav `[SHE-001]`.
- Status bar + workspace badge read **one** collision count and check
  collisions-before-stale `[SHE-002/003]` (both via `CountPill`).

---

## 2. Job setup

### 2.1 Stock + two-sided
```
── Stock ──────────────────────────────                ── Setup ────────────────
 Material [ Maple ▾ ]  X[300] Y[200] Z[18]              [ ＋ Two-sided setup ]
 [ ＋ Two-sided setup ]   edit pins → Stock              edit pins → Stock
```
One identical `MirrorRow`-style trigger in both panels, both pushing `SetupTwoSided`;
authoritative editing lives only in Stock › Alignment pins `[P2-005]`.

### 2.2 Fixture — Z is a real safety input `[P6-003 — top priority]`
```
── Fixture: Front clamp ──────────────  ☑ Enabled
 Holder clearance:  ✗ hit              [ Run holder clearance → ]   ← point-of-need
 🛡 Geometry (safety-checked)                                    ▾
   🛡 Position  X[10.0] Y[5.0]  Z[0.0]
   🛡 Size      X[40.0] Y[20.0] Z[25.0]
   🛡 Clearance [3.0]  (inflates all 6 faces)
 (editing any 🛡 field → pill resets to ◇ not checked — never falsely green)
```
Keep-out panel stays a plain XY form (no 🛡, no Z) so the two never look alike `[TOO/keepout]`.

---

## 3. Toolpath queue + tool management `[W1.1]`

```
LEFT panel (Toolpaths workspace)
── Operations ─────────────────────────
 ⠿ ☑ Pocket — slot 6mm     ● gen   👁 C R ⌖
 ⠿ ☑ Profile — outline     ⚠ stale 👁 C R ⌖     ← inline: visibility (VisibilityToggle)
 ⠿ ☐ Drill — 8× ⌀5         ◦ off   👁 C R ⌖        + drag grip ⠿ + per-row context menu
        right-click ▸ Enable · Duplicate · Delete(Del)         [SHE-006 inline-discoverable]
 [ ＋ Add Toolpath ▾ ]

── Project Tools ──────────────────────  [ Manage Library… ]   ← was only in dead panel [TOO-002]
 ⬚ 6mm flat endmill  ⌀6.00 4fl
 ◗ 3mm ball nose     ⌀3.00 2fl   ◀ selected
 ▽ 60° V-bit         60°
        right-click ▸ Duplicate · Delete(Del)     ← REVIVED: was unreachable in GUI [TOO-001/SHE-001]
 [ ＋ Add Tool ▾ ]  [ ＋ From Library ▾ ]
```
Glyph differs by tool type (⬚/◗/▽) so rows are distinguishable. CRUD harvested from
the deleted `project_tree.rs` into this live home, wired to existing
`DuplicateTool`/`RemoveTool` + a `Del` key path for `Selection::Tool`.

### 3.1 Tool editor — one commit model, grouped `[TOO-003/005]`
```
┌ 3mm ball nose ───────────────── ● modified ┐
│ Name [ 3mm ball nose      ]  Type [ Tapered ball ▾ ]
│ ── Cutting geometry ──
│   Ø Cutting [3.00]  Flutes [2]  Tip radius [1.50]  Shaft Ø [3.00]
│ ── Holder / Shank ──                              ▸   (collapsed)
│   Shank Ø [6.00]  Holder Ø [20.0]   ◇ collision: not configured 🛡
│ ── Catalog ──
│   Vendor [ Onsrud      ]  Product [ 65-023 ]      ← now EDITABLE [TOO-004]
│                                   [ Revert ]  [ Apply ]   ← explicit commit, both homes [TOO-003]
```
Same `ValueRow` + `named_section` everywhere; "Shaft Ø" (cutting) and "Shank Ø"
(holder) sit under different section headers so they stop colliding.

---

## 4. Toolpath properties — five concern tabs `[W3.2]`

### 4.0 Header (de-overloaded) + tab bar `[SHE-004]`
```
┌ Pocket — slot 6mm ───────────────────────────────────────────┐
│ Tool [6mm flat ▾]   Input [model.step ▾]   [ Generate ▶ ] ● ok │ ← identity+IO+generate, grouped
│ DIAGNOSTICS  ( within ) ( ⚠ 1 hint → )                        │ ← lifted ABOVE the tabs, was buried last
│ [ Geometry ][ Feeds & Speeds ][ Linking ][ Heights ][ Dressup ]│
└───────────────────────────────────────────────────────────────┘
```

### 4.1 Geometry — *what to cut* `[P3-001/002/003, P2-004, P1-007]`
```
┌ Geometry ─────────────────────────────────────────────┐
│ ── Summary ──                                           │
│  Pattern [ Zigzag ▾ ]      Direction ( ◉ Climb ○ Conv ) │
│  Stepover    [3.00] mm  ⚡ [▣ 1234]                      │  ValueRow + ProvenanceBadge
│  Depth/pass  [2.00] mm  ⚡ [▣ 1234]                      │
│  Total depth [12.0] mm                                  │
│  ┌ pattern minimap (InlineDiagram) ▕▏▕▏▕▏ @3.0 ─────┐  │
│  └────────────────────────────────────────────────────┘ │
│ ▸ Advanced (tolerance · finishing · stock-to-leave · …) │  disclosure
│ ▸ Drill cycle (peck · dwell · Peck retract R-plane)     │  [P2-004: R-plane lives here, relabeled]
│ ▸ Machining boundary (enable · inherit · offset)        │  [moved out of Dressups]
└─────────────────────────────────────────────────────────┘
```

### 4.2 Feeds & Speeds — *how fast*, with the SPEED/CUT split `[W3.1, P1-001/002/003, P2-001/002]`
```
┌ Feeds & Speeds ──────────────────────────────────────────┐
│ ── SPEED (how fast) ──                                     │
│  Feed   [1400] mm/min ⚡ [▣ 1234]                          │
│  Plunge [ 450] mm/min ⚡ [▣ 1234]                          │
│  Spindle  override [18000] rpm   default 〈 18000 〉 ▸ wins │  PrecedenceField [P1-005]
│  [ ⚡⚡ Apply recommended speeds ]   ← SPEED ONLY            │  SuggestButton(Recipe)
│ ── CUT (how deep/wide — edited on Geometry) ──             │
│  DOC 〈 2.00 〉  WOC 〈 3.00 〉   [ Apply cut · changes cut ]│  separate, attributed apply [P2-002]
│ ── Derived ── (read-only)                                  │
│  Chipload 0.05  ·  Power ▮▮▮▯▯ 61%  ·  MRR 12.4 cc/min      │  compare::power_bar / mrr_row
│  recipe: vendor LUT, roughing               [ Details ▸ ]  │  → opens drawer (was the modal)
└───────────────────────────────────────────────────────────┘
   Details drawer (read-mostly): CompareRow table · "Why this value?" derate ·
   charts (engagement / feed-vs-rpm / vendor band) · what-if (applies [◷], SPEED-only) ·
   raw vendor LUT (one canonical table). Legacy feeds-card + 3rd "Suggest all" = retired.
```

### 4.3 Linking · 4.4 Heights · 4.5 Dressup
```
┌ Linking — how moves connect ┐  ┌ Heights — Z planes ┐  ┌ Dressup — edge work ┐
│ ── Entry & Exit ──           │  │ Clearance [6.0]    │  │ ── Holding tabs ──   │
│  Entry [ Helix ▾ ]           │  │ Retract  [2.0]     │  │  Count[4] W[8] H[3]  │
│   Helix R[3.0] pitch[1.0]    │  │ Top      [0.0]     │  │ ── Path quality ──   │
│  Lead-in/out [arc 2.0]       │  │ Effective safe-Z   │  │  Feed smoothing ☑    │
│ ── Move optimization ──      │  │  〈 6.0 〉 → Post   │  │  Dogbone ☑           │
│  Retract [ smart ▾ ] Arc-fit │  │ ┌ side-view ⃕ ─┐   │  └──────────────────────┘
└──────────────────────────────┘  │ └──────────────┘   │   (adaptive3d entry combo
   entry split fixed [P2-003]      └────────────────────┘    no longer shown inert [P2-003])
```
Every tab is `named_section` summary + `disclosure` advanced — the same grammar.

---

## 5. Simulation

### 5.1 Verdict HUD + Inspector — one rollup, summary-first `[W0.4, P4-001/002, INS-001/002]`
```
VERDICT HUD (top of viewport)                INSPECTOR › Project overview
 [ within 6/8 ] ( exceeds 2/8 → )             ── Verdict ──  ( exceeds 2/8 → )   ← same CountPill,
 ( collisions 0 → ) { issues 41 }              ▾ Findings (must-address 2)         same /8 producer
                                               ▸ Informational (39)                summary-first
 (pills jump; HUD _events sink now wired)      ▸ Top hotspots
 [P5-001/P6-004]                               ── Now playing: Profile ──  (scope-labeled)
                                               ── Selected span ──  · 🔒 pin [INS-008]
```
Both rollups build `CountPill` from `ToolLoadReport::summary()` with `/8` — they
**cannot** disagree. `verdict_counts()` retired. Verdict pills use the verdict ramp;
`{ issues 41 }` uses the observation palette so it can't read as a load verdict `[INS-003]`.

### 5.2 Focused cards — distinct identities, freshness-gated `[INS-003/005/006]`
```
░ ⟳ stale — re-run to refresh ─────────────   ← FreshnessGate now wraps the card path [INS-005]
┌ 🔥 HOTSPOT · TP3 · m120–138 ───┐   ┌ ⚠ ISSUE · chip-weld · m44 ──┐
│ peak chip 0.0123  DOC 2.0      │   │ < Prev  Next >               │ ← issue card has nav,
│ engage 32% [▲ comparative]     │   │ …                            │   hotspot doesn't:
│ [ Jump → ] [ Optimize → ]      │   │ [ Jump → ] [ Optimize → ]    │   distinct identities
└────────────────────────────────┘   └──────────────────────────────┘
```
Engagement carries a `[▲ comparative]` provenance cue (not an absolute defect) `[INS-006]`;
one unit (`%`) everywhere `[INS-004]`.

### 5.3 Timeline + signal spine `[W3.6, TIM-001…010]`
```
── Boundary ──  ████|███▌|██░░  ● gate-trips (clickable → focus hotspot+jump) [TIM-010 wired]
── Spans ──     [tp1][ tp2 ][tp3]                          (same global X as boundary)
── Signals ──   ⚠ peak chip ↑   (which-metric summary above the stack) [TIM-008]
   ▸ chip  ▁▂▅█▅▂   ▸ engage  ▂▃▃▂   ▸ feed  ▅▅▅   ← per-track disclosure
   ░ ⟳ stale       (spine now reflects staleness) [TIM-009]
   cursor + scrub handle visible (interactions no longer hidden in a tooltip) [TIM-004]
   empty-state: "No metrics — enable Capture in Run, then ▶ re-run"  [TIM-003]
 ⏮ ⏪ ▶ ⏩ ⏭  transport
```
Strips share one X-axis + one click contract; the debug-only semantic band is the
only divergent one and is labeled as such `[TIM-001/002]`.

---

## 6. Optimizer `[W3.5, OPT-001…006]`

```
┌ Optimize project ─────────────────────────────────────────────────┐
│ Baseline 4.10 min  →  Optimized 3.40 min  (−0.70, −17%)  ░⟳ if stale │ ← correct math [OPT-001],
│ ☑ │ Toolpath        │ Δcycle │ Verdict          │                    │   stale-gated [OPT-003]
│ ☑ │ Pocket slot     │ −22%   │ ✓ safe   [◆]     │                    │
│ ☐ │ Profile outline │  —     │ ⚠ trade-off [Open →] │                │ ← per-row open [OPT-002]
│ · │ Drill 8×        │  —     │ skipped (drill)  │                    │
│ Selection col headed; verdict col consistent glyph+badge [OPT-004]  │
│                                            [ Cancel ]  [ Apply ⭐ ]   │
└────────────────────────────────────────────────────────────────────┘
 modal suggestions become actionable: [ Cap feed → 2961 & re-optimize ] (was prose) [OPT-005]
```
Applied candidates carry `[◆ sim]` provenance, distinct from `[▣ vendor]` — a
sim-validated value is forever tellable from a raw LUT one `[P7-004]`.

---

## 7. What this final design guarantees

- **One vocabulary, everywhere.** The same `ProvenanceBadge`, `CountPill`,
  `ValueRow`, `named_section`/`disclosure` appear on every surface above — the
  shared layer makes the consistency structural, not aspirational.
- **Look-alikes have distinct roles.** ⚡ vs ⚡⚡ vs `[badge]`; verdict vs observation
  pills; hotspot vs issue cards; 🛡 fixture vs plain keep-out — each pair is now
  visually separated by a component, not a caption.
- **Dig deeper, no dumps.** Every dense panel is summary (`named_section`) + drill
  (`disclosure` / `Details ▸`) — Geometry, Feeds, Inspector, signal spine, tool editor.
- **No prose crutches.** Apology tooltips, "click the red lines below", "Try this"
  sentences, grey-italic safety text → all replaced by affordances `[P5-*, OPT-005, TIM-005]`.
- **Honesty by construction.** Freshness is a wrapper (can't be forgotten), the load
  rollup has one producer (can't diverge), provenance has one color (can't drift),
  and the fixture safety fields actually feed the collision check.
```
