# Feeds & Speeds Workspace — IA + UI Design

**Date:** 2026-06-02
**Predecessor docs:**
- `planning/feeds_workspace_breakout_investigation_2026-06-02.md` (architecture feasibility)
- IA workflow output: `weqnbf14a` (4-agent run, 478k tokens)
- `planning/feeds_modal_enhancements_2026-06-02.md` (S1-S4 — modal changes that
  feed into the workspace's editable form)

This document captures the locked IA decisions, the UI layout, and the
implementation order. It supersedes the modal-as-deep-dive plan: the
workspace **replaces** the modal entirely; the Toolpaths screen loses its
feeds widgets.

## Locked decisions

| # | Decision | Value |
|---|---|---|
| 1 | Modal status | Removed. Workspace is the only home for feeds. |
| 2 | Toolpaths screen | Strip all feeds widgets out. |
| 3 | Compare mode | 2-way (two lanes splitting the centre column). |
| 4 | Apply | Commits the form's current values to the toolpath. |
| 5 | Suggest | Repopulates the form from the feeds pipeline (overwrite). |
| 6 | Auto-regen | On Apply, the toolpath regenerates automatically. |
| 7 | Per-Z engagement | Behind a "show engagement detail" toggle, 3D ops only. |
| 8 | Scallop ↔ Stepover | Both editable with a derive arrow; typing in one updates the other. |
| 9 | Cross-workspace events | Unified discriminant (no separate Modal/Workspace variants — modal is gone). |
| 10 | Out-of-band inputs | Inline red border + hover warning. Operator can still apply. |
| 11 | Header strip — spindle strategy | Interactive (toggle in header). |
| 12 | Header strip — tool/op/material | Read-only with deep-link click-through to owning workspace. What-if exploration via compare mode. |
| 13 | Picker rail grouping | Collapsible-by-setup, all expanded by default. |
| 14 | Picker rail status dot | Green/amber/grey/red + hollow=stale. Folded setups show worst-child dot. |
| 15 | Cross-workspace landing | Opens on the named toolpath. Engagement-detail toggle stays collapsed. |

## Major axes (drive variant displays)

1. **Tool Geometry Class** — Flat / Ball / BullNose / VBit / TaperedBall.
   Drives geometry diagram + presence/shape of effective-chipload chart.
2. **Kinematics Class** — Z-only / Lateral / Contour / 3D-raster / 3D-contour.
   Swaps Per-Kinematics Breakdown ↔ Drill Metrics Panel.
3. **Pass Role** — Rough / SemiFinish / Finish / Drill.
   Re-prioritises which gates/derates take prominence.
4. **Vendor Data Confidence** — Exact / Derived / Extrapolated / Formula-only / Unmodeled.
   Right rail: Vendor LUT card vs Formula-only card.
5. **Simulation State** — Pre-sim vs Post-sim. Post-sim additively reveals
   verdict triad, modulation summary, histograms.
6. **Spindle Strategy** — MatchChart vs MaxSpeed. Annotates constant-chipload
   curve and comparison card.

## Layout (1600×900 typical, ~815 usable vertical)

```
┌──────────┬──────────────────────────────────────────────┬──────────┐
│ Picker   │ Header strip: tool · op · material · spindle │ Vendor   │
│ rail     ├──────────────┬───────────────────────────────┤ LUT      │
│ (240)    │              │ Recommended Values (form)     │ Card     │
│          │ Tool         │  ┌─────┬─────┬─────┬─────┐    │ (320)    │
│ ▾ Setup A│ Geometry     │  │ RPM │Feed │Plng │ DOC │    │          │
│  ● tp 1  │ Diagram      │  ├─────┼─────┼─────┼─────┤    │ ┌──────┐ │
│  ◐ tp 2  │ (240×180)    │  │ WOC │Chip │ Pwr │ MRR │    │ │match │ │
│  ○ tp 3  │              │  └─────┴─────┴─────┴─────┘    │ │ row  │ │
│ ▾ Setup B│  ⌀6.0 mm     │  Scallop ─→ Stepover         │ │ band │ │
│  ◐ tp 4  │  2 flutes    │  (paired, derive arrow)       │ │ evid │ │
│          │  L/D 4.2     │                              │ └──────┘ │
│          ├──────────────┴───────────────────────────────┤ ────     │
│          │ Derate Cascade Waterfall                     │ sibling  │
│          │  target × depth × L/D × safety × strat = eff │ scatters │
│          ├──────────────────────────────────────────────┤ (collap- │
│          │ Constant-Chipload Curve (feed/RPM)           │ sible)   │
│          │  + machine envelope, current/recommended dots│          │
│          │  + speedup vector when MaxSpeed              │ ────     │
│          ├──────────────────────────────────────────────┤ Verdict  │
│          │ Engagement Section (swaps by category)       │ Triad    │
│          │                                              │ (post-   │
│          │ [3D ops post-sim:] ☐ show engagement detail  │  sim)    │
│          │                                              │ ────     │
│          │                                              │ Apply    │
│          │                                              │ Suggest  │
└──────────┴──────────────────────────────────────────────┴──────────┘
```

Vertical reading flow in the centre column: **setup → math → cut**.

### Panel allocation (rough px)

| Region | Width | Height |
|---|---:|---:|
| Picker rail (left) | 240 | full |
| Centre column | 1040 | full |
| Vendor rail (right) | 320 | full |
| Header strip | (1040) | 36 |
| Tool diagram + values | 1040 | 200 |
| Derate cascade | 1040 | 130 |
| Constant-chipload curve | 1040 | 170 |
| Engagement section | 1040 | 240 |

## Compare mode (2-way)

The centre column splits vertically into two lanes; picker rail and vendor
rail stay full-height. Each lane is a complete A-layout stack at ~520px
wide. Charts narrow but stay full-height.

```
┌─────┬───────────────────────┬───────────────────────┬─────┐
│Pick │ Header A · spindle    │ Header B · spindle    │ Δ   │
│     ├───────┬───────────────┼───────┬───────────────┤rail │
│     │ diag  │ values        │ diag  │ values        │     │
│     ├───────┴───────────────┼───────┴───────────────┤Δlut │
│     │ Cascade A             │ Cascade B             │     │
│     │ Curve A               │ Curve B               │Δver │
│     │ Engage A              │ Engage B              │dict │
│     │ [Apply A] [Suggest A] │ [Apply B] [Suggest B] │     │
└─────┴───────────────────────┴───────────────────────┴─────┘
```

- Lane B is loaded by dropdown in lane B's header, or by drag from picker.
- Right rail collapses to a delta strip (Δ-vendor-row, Δ-verdict triad).
- Apply/Suggest are per-lane.
- Compare mode is the **what-if sandbox**: clone a toolpath into lane B,
  swap tool/material on the clone (via lane B's deep-link header), see
  the feeds delta side-by-side. No in-workspace material/tool editing
  needed.

## Editable form behaviour (the new bit)

Each numeric cell in Recommended Values has three visual states:

| State | Visual |
|---|---|
| **Suggest-loaded** (computed, untouched) | italic |
| **Edited** (operator typed over it) | normal + small undo glyph |
| **Applied** (matches toolpath storage) | normal, no glyph |
| **Out-of-band** (chipload outside vendor band, power over envelope, etc.) | + red border, hover tooltip explains which gate it trips |

Operator workflow:
1. Workspace opens with toolpath selected; cells show Suggest-loaded values.
2. Operator types over any cell → that cell flips to Edited. Dependent
   cells recompute (e.g. typing Feed updates Chipload; typing Scallop
   updates Stepover via the paired derive arrow).
3. **Suggest** button → all cells overwritten back to Suggest-loaded.
4. **Apply** button → current cell values committed to the toolpath +
   auto-regen kicked off. Picker dot transitions: stored value mismatched
   → hollow (stale) → green/amber/grey after regen + post-verdict.

### Scallop ↔ Stepover pairing

Both cells live next to each other with a `─→` arrow between them. The
arrow's direction shows which is the active input:

- Operator typed in Scallop → arrow points Scallop → Stepover; Stepover
  shown italic (derived).
- Operator typed in Stepover → arrow flips; Scallop shown italic.
- Suggest sets a default direction per op (Scallop family → scallop active;
  Parallel family → stepover active).

## Engagement section — conditional content

One slot, content swaps wholesale by `(tool_class, kinematics_class, sim_state)`:

| Category | Pre-sim content | Post-sim adds |
|---|---|---|
| **Flat + Lateral rough** | Chipload histogram preview from formula bands | Real-sample histogram + arc distribution |
| **Flat + Contour finish** | Compact derate footnote | Arc distribution dominant |
| **Ball / BullNose / Taper / VBit** | Effective-chipload-vs-engaged-depth chart | + Mean-vs-peak chip thickness scatter |
| **Drill / pin_drill** | Peck-descent bars + chip-evac gradient + welding zone slider | + Drill gates verdict card |
| **3D ops (any tool)** | (per tool class above) | + "show engagement detail" toggle → per-Z profile expands inline |

The detail toggle answers the per-Z question concretely: it's not a default
panel, it's a "do I trust this single chipload number across all this op's
actual engagement?" probe — useful when axial-DOC varies (terrain in
adaptive3d, level set in scallop) and meaningless when it's flat.

## Layout primitives — always-present (9)

1. Picker rail
2. Header strip (read-only tool/op/material + interactive spindle strategy)
3. Tool geometry diagram (annotations swap per tool class)
4. Recommended Values editable form
5. Derate cascade waterfall
6. Constant-chipload curve
7. Vendor LUT card (or Formula-only card if no LUT match)
8. Spindle strategy comparison card (compact when only one strategy active)
9. Apply / Suggest action bar (right rail bottom)

## Layout primitives — conditional (11)

1. Effective-chipload-vs-engaged-depth (Ball/BullNose/Taper/VBit)
2. Per-kinematics breakdown (sim + non-drill)
3. Drill metrics panel (drill ops only — replaces per-kinematics + effective-chipload slots)
4. Per-Z engagement profile (3D ops post-sim, behind detail toggle)
5. Verdict triad (post-sim)
6. Modulation summary panel (post-sim)
7. Chipload histogram (post-sim with cylinder-side engagement)
8. Engagement arc distribution (post-sim, lateral/contour kinematics)
9. Mean-vs-peak chip thickness scatter (post-sim, cylinder-side)
10. Chipload-vs-diameter / hardness sibling scatters (LUT siblings exist)
11. Formula vs LUT comparison bar (both available)
12. Vendor row scoring radar (collapsed by default, "Why this row?" expand)

## Visual treatment

- No card-titles-and-padding everywhere. Each major block separated by
  1px rule + 8px gap. Titles inline with the data.
- Derate cascade: factors at 1.0 collapse to thin ticks; factors that
  deflect ≥2% get full segment width with label + ratio. Spindle_speedup
  uses a distinct colour when active.
- Constant-chipload curve: machine envelope as translucent bounding box;
  current/recommended points as filled/outlined dots; MaxSpeed mode draws
  speedup arrow from MatchChart point to MaxSpeed point.
- Tool diagram: 2D side profile, axis-locked. DOC marker horizontal line
  ties visually to the engaged-depth chart below.
- Vendor row card: source + evidence grade badge + observation id + bands.
  "Why this row?" link expands the scoring radar inline.

## Cross-workspace events

Existing event `OpenFeedsModal(ToolpathId)` is renamed to
`FocusFeedsToolpath(ToolpathId)`. The modal target type is removed.
Workspace selection persists across launches as
`state.feeds_workspace_selected_toolpath: Option<ToolpathId>`.

Deep-links from Setup/Toolpaths workspaces: clicking the tool field in
the Feeds header jumps to Toolpaths workspace with that tool focused;
clicking material jumps to Setup with that material focused.

## Implementation order

Build in phases. Each phase is independently committable + gate-runnable.
Default behaviour preserved on every phase until the modal removal step.

### Phase 0 — Helpers extraction (refactor)

**Why first:** `feeds_modal.rs` is on the never-stage list. Need to move
shared rendering helpers out before we can compose them into the
workspace.

- New `crates/rs_cam_viz/src/ui/feeds_shared.rs`
- Move `draw_comparison_card`, `draw_derate_chain`, derate row helpers,
  chart helpers
- `feeds_modal.rs` becomes a thin adapter calling into `feeds_shared.rs`
- This one commit explicitly touches `feeds_modal.rs` — requires user
  blessing
- Est: half day

### Phase 1 — Workspace shell + editable form

- `Workspace::FeedsSpeeds` enum variant
- Tab in `workspace_bar.rs`
- CLI arg parsing
- Keyboard routing
- Main dispatch arm
- New `draw_feeds_speeds_layout(ctx)` skeleton with empty centre column
- Picker rail (collapsible-by-setup, status dots)
- Header strip (read-only tool/op/material + interactive spindle strategy)
- Tool geometry diagram (always-on, reuse existing draw code if any)
- Recommended Values editable form (Suggest, Apply, dependent recompute,
  out-of-band inline red border + hover warning)
- Scallop ↔ Stepover paired editor with derive arrow
- Apply emits `RegenerateToolpath` event after committing values
- Est: 2 days

### Phase 2 — Math charts (move from modal to workspace)

- Derate cascade waterfall (consume `FeedsExplain.derates`)
- Constant-chipload curve (consume `FeedsResult` + `MachineEnvelope`)
- Vendor LUT card on right rail (consume `LookupResult` +
  `VendorObservation`)
- Sibling scatters (collapsible from Vendor LUT card)
- Spindle strategy comparison card (sits below Vendor LUT card or in
  header — TBD by visual fit)
- Est: 1.5 days

### Phase 3 — Engagement section (conditional)

- Slot infrastructure: `EngagementSectionContent` enum keyed by
  `(tool_class, kinematics_class, sim_state)`
- Chipload histogram (Flat-lateral default)
- Effective-chipload-vs-engaged-depth chart (Ball / BullNose / VBit /
  TaperedBall — curve shape per class)
- Drill metrics panel (peck bars + chip-evac gradient + welding zone)
- Arc distribution (contour ops)
- Mean-vs-peak chip thickness scatter
- Est: 2 days

### Phase 4 — Post-sim layer

- Verdict triad on right rail (post-sim only)
- Modulation summary panel
- Per-Z engagement profile (3D ops, behind "show engagement detail"
  toggle)
- Status dot wiring: gate verdicts → green/amber/grey/red
- Est: 1 day

### Phase 5 — Compare mode

- Centre column splits to 2 lanes
- Lane B loader (dropdown in lane B header)
- Right rail collapses to delta strip
- Per-lane Apply/Suggest
- Est: 1.5 days

### Phase 6 — Strip-out + modal removal

- Remove all feeds widgets from Toolpaths workspace
- Delete `feeds_modal.rs` and the modal launch event
- Rename `OpenFeedsModal` → `FocusFeedsToolpath`
- Update keyboard shortcuts that opened the modal
- Est: half day

**Total: ~9 days of solid work.** Phases 1-3 are the MVP that makes the
workspace usable. Phases 4-5 are additive. Phase 6 is cleanup, last.

## Sentries / regression net

Every phase keeps these green:
- F-024, F-026, F-027, F-028, F-031, F-036b, F-037 smoke baseline diff
- `cargo test -p rs_cam_core --lib`
- `cargo test -p rs_cam_core --tests`
- `cargo clippy --workspace --all-targets -- -D warnings`

The modal removal in phase 6 may shift visual smoke / screenshot tests
if any exist for the modal — audit before that commit.

## Open implementation questions (not blocking design)

These surface during build, not before:

1. **Tool geometry diagram source code** — does any 2D-side-profile-of-tool
   widget exist already (in the tool catalogue UI or elsewhere)? If yes,
   factor it into `feeds_shared.rs`. If no, build it in Phase 1.
2. **Per-Z grouping derivation** — derive in GUI from raw samples
   initially. Promote to core only if another consumer needs it.
3. **Picker rail keyboard nav** — arrow keys to traverse the tree? Enter
   to focus the centre column? Out of scope for Phase 1; nice-to-have for
   Phase 4 polish.
4. **Status dot worst-child rollup** — exact severity ordering for folded
   setups: red > amber > grey > hollow > green. Trivial but needs picking.
5. **Out-of-band hover tooltip wording** — copy needs writing per gate
   verdict reason (chipload-low, chipload-high, power-pegged, deflection-
   over-cap). Phase 1 placeholder, polish in Phase 4.

## Data inventory reference

All visualisations in this design source from existing data surfaces
already in core. Reference (from IA workflow `weqnbf14a`):

- `FeedsExplain` (UNSTAGED WIP, read-only access)
- `FeedsDerates`, `SpindleStrategy`
- `LookupResult`, `VendorObservation`, vendor LUT bands
- `vendor_normalize::lookup_diameter_for_input` (engaged dia at DOC)
- `SimulationCutSample.engagement`, `SimulationToolpathCutSummary.per_kinematics`
- `DrillSample`, `DrillToolpathSummary`, `ToolpathLoadVerdict.drill_gates`
- `ChiploadVerdict`, `PowerVerdict`, `DeflectionVerdict`, `ModulationSummary`
- `MachineEnvelope` from `MachineKinematics`

No new core data is required for the MVP (phases 1-5). Phase 6 is
pure UI cleanup.

## Status

**Design closed.** Ready for build session start. The build session
prompt should reference this doc + the breakout investigation +
the modal enhancements plan (some of S1-S4's modal changes get
absorbed into the workspace's editable form rather than landing as
modal commits).
