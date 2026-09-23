# Viewport interaction redesign — phase 1 mockups

**Status line:** Mockups NOT operator-reviewed; phase 2 built against them on
2026-09-23 night by the operator's instruction to run unattended.

**Phase boundary.** Phase 2 can render Mockups A, B (without B2), D, E, F
and the catalogue from today's two `Precondition` arms. Mockups C, G and B2
need state that does not exist today (inventory 1.2): Computing, Stale,
Failed and the Compute versus Compute & show split. That is phase 3 work. In
phase 2 those rows stay in the states they reach today: Ready/off, Showing,
Needs compute and Blocked.

**Date:** 2026-09-23. **Base commit:** `5c04b774`.
**Evidence file:** `PHASE1_INVENTORY.md` in this folder. Section numbers like
"inventory 2.3" point there. Paths are relative to `crates/rs_cam_viz/`.

## 0. Rules for every mockup

### 0.1 String rules

- Every quoted string below is the exact text for phase 2.
- `tests/ui_string_hygiene.rs` fails a single-line literal with a run of 3 or
  more interior spaces (`MIN_RUN`, line 70). No string here has one. The ASCII
  art uses spaces for layout only; do not copy art spacing into a literal.
- Write each long string as ONE literal. Do not use a `\`-continued literal
  for a dock string: `cargo fmt` can collapse it and keep the spaces.
- Use `…` (U+2026) and `—` (U+2014) as the code does today.
- A string that the MCP reply also carries (a precondition reason) keeps its
  current text unless this file says otherwise. Section 11.3 lists the changes.

### 0.2 Colour rules

- UI colours come from `ui/tokens.rs` only. `tests/panels_read_the_token_module_up1.rs`
  allows one `Color32::from_rgb(` outside `ui/tokens.rs` and `render/colors.rs`,
  and `ui/sim_timeline.rs` already uses it.
- Legend swatches are DATA colours. They come from the render colour function
  that paints the 3D view, converted with `tokens::from_linear_rgb`
  (`ui/tokens.rs:739`). This is the rule that `panel.rs:301-305` states today:
  "Every caller passes the overlay's OWN colour function".
- Staleness has one cue: `components::Freshness`
  (`ui/components/CLAUDE.md`, invariant 2). Do not draw a second stale badge.
- A verdict colour carries a glyph (`tokens::GLYPH_*`, `ui/tokens.rs:211-217`).

### 0.3 Token table used by the mockups

| Element | Token |
|---|---|
| Dock frame fill | `SURFACE_OVERLAY` |
| Dock frame shadow | `SHADOW_OVERLAY` |
| Dock and popover corner | `RADIUS_MD` |
| Dock frame border | `HAIRLINE` |
| Section button text | `TEXT_BODY`, font `SIZE_BUTTON` (13) |
| Section button, popover open | fill `ACCENT_QUIET`, text `TEXT_STRONG` |
| Section suffix (`Selected`, `Reach`) | `TEXT_MUTED` |
| Popover fill | `SURFACE_OVERLAY` |
| Row label | `TEXT_BODY` |
| Reason line | `TEXT_MUTED` (today `TEXT_FAINT`, `panel.rs:248`; `ui/tokens.rs:86` gives `TEXT_FAINT` a worst ratio of 3.28:1, under the WCAG AA 4.5:1 floor for body text) |
| Needs compute action | `components::Button` secondary variant |
| Computing | `egui::Spinner` + `TEXT_MUTED`; lane colour `LANE_RUNNING` |
| Blocked glyph | `GLYPH_UNKNOWN` in `UNKNOWN` |
| Stale | `components::Freshness` |
| Failed | `GLYPH_DANGER` in `DANGER`, fill `TINT_DANGER` |
| Target line | `TEXT_MUTED`, `SIZE_CAPTION` (11) |
| Legend rail fill | `SURFACE_RAISED` |
| Legend name | `TEXT_BODY`, `SIZE_CAPTION` |
| Legend scale ends and units | `TEXT_MUTED`, `SIZE_CAPTION` |
| Legend caveat line | `TEXT_FAINT` only when it repeats a fact; `CAUTION` for the reach over-statement note (as `panel.rs:417-434`) |
| Spacing | `SPACE_2` (4) between items, `SPACE_3` (8) frame margin, `ROW_ACTION` (26) row height |

### 0.4 Keyboard and pointer model (all mockups)

**Caution:** in Simulation, `Escape` switches to Toolpaths
(`app/input.rs:645-652`). The guard at `app/input.rs:606-609` returns early
only when a widget has focus. A popover opened with the mouse can have no
focused widget. Phase 3 must add an explicit "popover open" guard before the
Simulation handler reads `Escape`.

1. One popover is open at a time. A click on a second section closes the
   first and opens the second.
2. `Escape` closes the open popover. It is consumed; no workspace handler sees
   it. With no popover open, `Escape` keeps its current meaning.
3. A click outside the popover closes it. Ruling needed: the plan says the
   dock must not intercept viewport input outside the controls. This file
   recommends that the closing click does NOT also select or deselect in the
   viewport. Section 12 lists this as open question Q1.
4. Tab order: `View`, `Scene`, `Paths`, `Inspect`, `All viewport options`.
   With a popover open, Tab moves into its rows top to bottom. A row's action
   button follows its row. Shift+Tab reverses. The last row wraps to the
   section button.
5. `Enter` or `Space` on a focused section button opens its popover. In
   Simulation, `Space` is Play / Pause (`app/input.rs:638-643`); it must not
   fire while a dock control has focus. The focus guard at
   `app/input.rs:606-609` already gives this.
6. The overlay keys keep their meaning: `S`, `P`, `R`, `X`, `,`, `.`
   (`app/input.rs:562-596`). Both key handlers return while any widget has
   focus (`app/input.rs:438`, `app/input.rs:606-609`), so a focused catalogue
   search field receives the letters.
7. `O` opens and closes the catalogue. Section 11.4 gives the reason.
8. The dock and the rails take pointer input only inside their own rects.
   A drag that starts outside them orbits the camera as today.

## 1. Where each registry row goes

The plan names four sections. This table gives each of the 39 rows ONE home.
One home per row keeps the rule "one state, one writer" that
`tests/overlays_registry.rs:122-136` enforces for flags. The catalogue lists
every row as the panel does today.

| Dock section | Registry rows | Count |
|---|---|---|
| View | `orientation_gizmo` | 1 |
| Scene | `grid`, `model`, `stock_box`, `stock_solid`, `origin_axes`, `datum`, `fixtures`, `keep_outs`, `alignment_pins`, `flip_axis`, `curves`, `derived_rest_regions`, `boundary_outline`, `simulated_stock` | 14 |
| Paths | `all_toolpaths`, `cutting_moves`, `rapids`, `entry_markers`, `height_planes`, `tool_profile_ghost`, `span_entry`, `span_lead_out`, `span_link_bridge`, `span_dressup`, `move_colour_palette`, `move_colour_engagement`, `move_colour_advance_per_tooth` | 13 |
| Inspect | `reach_map`, `rest_heatmap`, `tier_map`, `planner_islands`, `stock_colour_solid`, `stock_colour_deviation`, `stock_colour_by_height`, `collisions`, `tool_deflection`, `generator_steps`, `active_step_highlight` | 11 |
| **Total** | | **39** |

Non-registry controls:

| Control | Home |
|---|---|
| Top / Front / Right / Iso, Reset view | View |
| Perspective / Orthographic | View (constructs `UiCommand::ToggleProjection`; inventory 5.4) |
| Fit | View, as `Reset view` (`UiCommand::ResetView` fits the first model, `app/input.rs:319`) |
| Stock opacity slider | Scene, under `Simulated stock` |
| Selected / All scope | Paths header toggle (constructs `UiCommand::ToggleShowAllToolpaths`; inventory 5.4) |
| Per-operation eye / C / R | stays on each operation row; Paths shows the note in section 3 |
| Compute activity + cancel | status bar, beside the lane chips (section 11.1) |
| Simulation `Reset` | the transport bar (section 8 and 11.1) |

A proposal for "consistent access to path analysis" in Inspect: the Inspect
popover shows one line, `Move colour is in Paths.`, with a button `Open Paths`.
The button opens the Paths popover. It does not add a second writer.

**Section suffix rule.** The Paths suffix is `Selected` or `All`, from
`viewport.show_all_toolpaths`. The Inspect suffix names the preferred colour
source of the workspace surface: in Toolpaths, the model surface (`Reach`,
`Rest` or `Off`); in Simulation, the stock surface (`Solid`, `Deviation` or
`Height`). The suffix is the PREFERENCE. When the preference cannot draw, the
suffix gets the Blocked glyph (section 4).

## 2. The frame: target line, legend rail, dock

```
┌─ viewport ───────────────────────────────────────────────────────────────┐
│ (deflection panel, top-left)                     (orientation gizmo)     │
│                                                                          │
│                              3D view                                     │
│                                                                          │
│                  Target: #12 Adaptive Pocket                             │
│  ┌ legend rail ──────────────────────────────────────────────────────┐   │
│  │ Reach · #12  [▓▓▓▓▓▓▓▓] miss 0.050 mm … 2.40 mm  log scale        │   │
│  └───────────────────────────────────────────────────────────────────┘   │
│      ┌ dock ───────────────────────────────────────────────────────┐     │
│      │ View │ Scene │ Paths: Selected │ Inspect: Reach │ All ⌕ (2) │     │
│      └─────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────┘
```

- The dock is an `egui::Area` anchored at the viewport's bottom centre,
  `SPACE_3` above the bottom edge. It is not a panel, so it takes no width
  from the 3D view (inventory 4).
- The legend rail sits directly above the dock. The target line sits above
  the rail.
- Existing floating paints keep their places: the deflection panel at the
  top-left (`app/viewport.rs:807`), the orientation gizmo at the top-right
  (`app/viewport.rs:653-656`), the Simulation loading pill at the top centre
  (`app/viewport.rs:582`). The dock uses the bottom edge, so none of them
  overlaps it.

Exact strings:

| Element | String |
|---|---|
| Section buttons | `View`, `Scene`, `Paths`, `Inspect` |
| Section suffix separator | `: ` (so `Paths: Selected`, `Inspect: Reach`) |
| Catalogue button | `All ⌕` with the count in brackets when `registry::non_default_count` > 0, for example `All ⌕ (2)` |
| Catalogue button hover | `Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O).` |
| Target line, toolpath selected | `Target: #{n} {name}` where `n` is the 1-based config index |
| Target line, nothing selected | `Target: none — select an operation` |
| Target line, Simulation | `Target: #{n} {name} (playback)` from `SimulationState::focused_toolpath` (see `state/viewport.rs:314-316`) |
| Target line, Setup | `Target: setup {n}` |

## 3. Mockup A — ready state (Toolpaths, one op selected, Reach showing)

State: Toolpaths workspace. Toolpath #12 is a finishing operation. The reach
map is `Ready`. Scene popover is open.

```
                  Target: #12 Adaptive Pocket
  ┌──────────────────────────────────────────────────────────────────┐
  │ Reach · #12  [▓▓▓▓▓▓▓▓▓] miss 0.050 mm … 2.40 mm  log scale      │
  │   green ≤ 0.050 mm · grey = unresolved · mid 0.35 mm             │
  └──────────────────────────────────────────────────────────────────┘
        ┌ Scene ───────────────────────────┐
        │ ☑ Grid                           │
        │ ☑ Model                          │
        │ ☑ Stock — box                    │
        │ ☐ Stock — solid                  │
        │ ☑ Origin axes                    │
        │ ☑ Datum crosshair                │
        │ ☐ Fixtures                       │
        │   — no fixture in this setup     │
        │ ☐ Keep-out zones                 │
        │   — no keep-out zone in this     │
        │     setup                        │
        │ ☑ Alignment pins                 │
        │ ☑ Flip axis                      │
        │ ☑ Curves (DXF/SVG)               │
        │ ☐ Derived rest regions           │
        │   — not drawn yet (no renderer)  │
        │ ☐ Boundary outline               │
        │   — not drawn yet (no renderer)  │
        │ ☐ Simulated stock                │
        │   — the simulated stock draws in │
        │     the Simulation workspace     │
        └──────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │[Scene]│ Paths: Selected │ Inspect: Reach │ All ⌕     │
      └─────────────────────────────────────────────────────────────┘
```

Exact strings: every row label is the registry `label` (inventory 1.4-1.7).
Every reason is the registry reason. The reason line prefix changes from
`· ` (`panel.rs:247`) to the Blocked glyph `—` (`GLYPH_UNKNOWN`) and a space.

Popover heights: Scene has 14 rows. At `ROW_ACTION` (26) plus reason lines,
the popover is about 14 × 26 + 5 × 15 ≈ 440 pt high. This is an estimate, not
a measurement. The popover needs a `ScrollArea` with a maximum height of
the viewport height minus the dock, the rail and `SPACE_6`.

Paths popover, same state:

```
        ┌ Paths ────────────────────────────────────┐
        │ Draw  (•) Selected  ( ) All               │
        │ ☑ Cutting moves                           │
        │ ☑ Rapids                                  │
        │ ☑ Entry markers                           │
        │ ☑ Height planes                           │
        │ ☐ Tool-profile ghost                      │
        │ Move colour  (•) Palette ( ) Engagement   │
        │              ( ) Advance / tooth          │
        │   — advance per tooth needs a simulation  │
        │ Spans  ☑ Entry ☑ LeadOut ☑ LinkBridge     │
        │        ☑ DressupArtifact                  │
        │ Per operation: eye, C and R are on each   │
        │ operation row.                            │
        └───────────────────────────────────────────┘
```

| Element | String |
|---|---|
| Scope label | `Draw` |
| Scope choices | `Selected`, `All` (a `components::ChoiceRow`) |
| Move colour label | `Move colour` |
| Move colour choices | `Palette`, `Engagement`, `Advance / tooth` |
| Span label | `Spans` |
| Span choices | `Entry`, `LeadOut`, `LinkBridge`, `DressupArtifact` |
| Per-operation note | `Per operation: eye, C and R are on each operation row.` (replaces the stale "the bullseye for isolation" text, `panel.rs:146-147`) |

The scope choice writes through `UiCommand::ToggleShowAllToolpaths`, so the
dock constructs that command (inventory 5.4).

View popover:

```
        ┌ View ──────────────────────────────┐
        │ Top   Front   Right   Iso          │
        │ Projection (•) Perspective         │
        │            ( ) Orthographic        │
        │ [Reset view]                       │
        │ ☑ Orientation gizmo                │
        └────────────────────────────────────┘
```

| Element | String |
|---|---|
| Presets | `Top`, `Front`, `Right`, `Iso` (hover `Shortcut: 1`, `2`, `3`, `4`) |
| Projection label | `Projection` |
| Projection choices | `Perspective`, `Orthographic` |
| Reset | `Reset view` |

## 4. Mockup B — a blocked option with reason and remedy

State: Toolpaths. The operator prefers Reach. The selected toolpath #4 is a
roughing operation, so no reach map applies. Inspect popover is open.

```
                  Target: #4 Adaptive Rough
      (no legend rail: nothing colours the model)
        ┌ Inspect ──────────────────────────────────────────┐
        │ Model colour                                      │
        │  (•) Reach  —                                     │
        │      — select a finishing operation               │
        │      Your choice is kept. The model draws plain.  │
        │  ( ) Rest                                         │
        │      — this toolpath has no rest grid yet         │
        │      [Compute rest…]                              │
        │  ( ) Off                                          │
        │ ☐ Tier map                                        │
        │   — run a preview from Toolpath ▸ Plan            │
        │     multi-tool finishing…                         │
        │   [Plan…]                                         │
        │ ☐ Planner islands                                 │
        │   — drawn by Inspect ▸ Tier map — no separate     │
        │     island outline renderer yet                   │
        │ Stock colour, collisions, deflection and          │
        │ generator steps draw in the Simulation workspace. │
        │ Move colour is in Paths.  [Open Paths]            │
        └───────────────────────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │ Scene │ Paths: Selected │[Inspect: Reach —]│ All ⌕   │
      └─────────────────────────────────────────────────────────────┘
```

Rules shown:

- The preference stays `Reach`. A selection change moves the target only.
  This matches the code today: no path clears `show_reach_map` on selection
  (inventory 2.2).
- The dock suffix shows the preference and the Blocked glyph: `Inspect: Reach —`.
  The dock does not show `Rest` or `Off` in its place. That is the plan's
  "do not silently substitute" rule.
- `Compute rest…` has an ellipsis because it opens a confirm step
  (Mockup B2). Today the button runs at once (`panel.rs:289-295`).

Exact strings:

| Element | String |
|---|---|
| Group label | `Model colour` |
| Choices | `Reach`, `Rest`, `Off` |
| Kept-preference line | `Your choice is kept. The model draws plain.` |
| Simulation note | `Stock colour, collisions, deflection and generator steps draw in the Simulation workspace.` |
| Path link | `Move colour is in Paths.` and button `Open Paths` |
| Rest action | `Compute rest…` |

**Mockup B2 — the Compute rest confirm.** The plan asks for the
regeneration and invalidation consequence BEFORE the work starts.

```
        ┌ Compute rest for #4 Adaptive Rough ────────────────┐
        │ This switches on rest analysis for this operation  │
        │ and regenerates it.                                │
        │ ! The current simulation will be cleared.          │
        │                                                    │
        │ [Compute]   [Compute & show]   [Cancel]            │
        └────────────────────────────────────────────────────┘
```

| Element | String |
|---|---|
| Title | `Compute rest for #{n} {name}` |
| Body | `This switches on rest analysis for this operation and regenerates it.` |
| Consequence line, only when a simulation exists | `The current simulation will be cleared.` with `GLYPH_CAUTION` in `CAUTION` |
| Buttons | `Compute`, `Compute & show`, `Cancel` |
| `Compute` outcome | applies `AutoEnableRestAnalysis` and generates. The row stays off. |
| `Compute & show` outcome | as `Compute`, and it records the intent to show Rest for THIS toolpath id. Phase 3 applies the intent on arrival only if the selection and the preference are unchanged. |

Honesty note: whether the simulation clears is the core's answer
(`effects.simulation_cleared`, `ui/properties/mod.rs:1338`). The dialog cannot
know it before the apply. The line must read the condition "a simulation
exists" and use the word `will` only if phase 3 confirms that
`AutoEnableRestAnalysis` always clears one. Otherwise use
`The current simulation may be cleared.` Section 12, Q2.

## 5. Mockup C — a computing option with its jobs link

State: Toolpaths. Preference `Reach`. Toolpath #12 was just selected, and the
reach walk runs on `ComputeLane::Reach`.

```
                  Target: #12 Adaptive Pocket
  ┌──────────────────────────────────────────────────────────────────┐
  │ Reach · #12  ◌ computing…  the model draws plain until it is done│
  └──────────────────────────────────────────────────────────────────┘
        ┌ Inspect ──────────────────────────────────────────┐
        │ Model colour                                      │
        │  (•) Reach  ◌ computing…  [Cancel all jobs]       │
        │      Status bar: Reach lane                       │
        │  ( ) Rest                                         │
        │  ( ) Off                                          │
        │ …                                                 │
        └───────────────────────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │ Scene │ Paths: Selected │[Inspect: Reach ◌]│ All ⌕   │
      └─────────────────────────────────────────────────────────────┘
 ─ status bar ──────────────────────────────────────────────────────────
  Models 1 · Triangles 661 k │ Reach running 1.4s │ [Cancel all jobs]
```

- Today the reach row reads `Ready` while it computes (inventory 2.3, item 1).
  The mockup gives it its own state.
- The legend rail keeps a line for the preferred encoding while it computes.
  So the operator sees why the model is plain.
- **The jobs link.** No jobs panel exists today. The existing jobs surface is
  the status bar lane chips (`ui/status_bar.rs:78-106`, automation ids at
  `ui/status_bar.rs:240-248`), which have hover text only, plus the strip's
  `Cancel All` (`ui/viewport_overlay.rs:144-148`). The mockup names the lane
  (`Status bar: Reach lane`) and offers the one cancel that exists.
- `UiCommand::CancelCompute` cancels ALL lanes. The button label must say so:
  `Cancel all jobs`, not `Cancel`.

Exact strings:

| Element | String |
|---|---|
| Computing word | `computing…` (matches the inspector, `ui/properties/toolpath_panel.rs:327`) |
| Rail line while computing | `the model draws plain until it is done` |
| Lane pointer | `Status bar: Reach lane` (for other lanes: `Toolpath`, `Analysis`, `Optimize`, `Job`) |
| Cancel | `Cancel all jobs` |
| Cancel hover | `Cancels every running and queued compute job, not only this one.` |

The Computing state applies to three rows today: `reach_map` (the reach
lane), `rest_heatmap` after `Compute rest` (the toolpath lane), and
`collisions` after `Run collision check` (the analysis lane). For the second
and third, the code has no row-level Computing state today (inventory 2.3,
item 9). Phase 3 derives it from the lane snapshot and the toolpath id; it
must not store a second flag.

## 6. Mockup D — multiple legends active

State: Simulation workspace. Stock colour `Deviation`, move colour
`Engagement`, collisions on, `Paths: All`.

```
                  Target: #3 Back Rough (playback)
  ┌──────────────────────────────────────────────────────────────────────┐
  │ Stock deviation  [▓▓▓▓▓▓▓] −1 mm over-cut … +1 mm remaining          │
  │   green = within ±0.1 mm                                             │
  │ Move colour: Engagement · all drawn  [▓▓▓▓▓▓] 0 % … 150 % of feed    │
  │   red = heavy load, feed reduced                                     │
  │ Toolpaths  ■ #1 Pin Drill  ■ #2 Front Rough  ■ #3 Back Rough  ■ #4 … │
  │ Collisions · 7  [▓▓▓▓] isolated … clustered                          │
  └──────────────────────────────────────────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │ Scene │ Paths: All │ Inspect: Deviation │ All ⌕ (3)  │
      └─────────────────────────────────────────────────────────────┘
```

The mockup cuts the toolpath line at `#4 …` for space only. The rail must not
do that: `+N` or `…` overflow is forbidden for essential meaning. The
toolpath palette line wraps onto a second line instead:

```
  │ Toolpaths  ■ #1 Pin Drill  ■ #2 Front Rough  ■ #3 Back Rough         │
  │            ■ #4 Profile  ■ #5 Finish                                 │
```

Legend strings (one rail line per active encoding):

| Encoding | Name | Scale ends | Caveat line | Colour source |
|---|---|---|---|---|
| Reach | `Reach · #{n}` | `miss {bar:.3} mm` … `{top:.2} mm`, then `log scale` | `green ≤ {tol:.3} mm · grey = unresolved · mid {mid:.2} mm` (today, `panel.rs:377-385`); plus `area_basis_note` and `over_statement_note` (inventory 5.1.11) | `reach_color` |
| Rest | `Rest · #{n}` | `{threshold:.2} mm` … `p95 {p95:.2} mm` | `grey = at or below the threshold` | `rest_ramp_color`. **Needs the p95 from core.** Today the legend uses the maximum (inventory 3, Rest row). |
| Tier map | `Tier map · setup {s}` | one swatch per fine tier: `tier 1`, `tier 2`, … | `light tint = overlap band` | `tier_fill_color`, `tier_overlap_color` |
| Stock deviation | `Stock deviation` | `−1 mm over-cut` … `+1 mm remaining` | `green = within ±0.1 mm` | `deviation_colors` |
| Stock height | `Stock height` | `low` … `high` | `scaled to this stock's Z range` | `height_gradient_colors`. The Z numbers are not available to the legend today. |
| Engagement | `Move colour: Engagement · {target}` | `0 %` … `150 % of feed` | `red = heavy load, feed reduced` | `engagement_color` (input `feed / nominal`, `render/toolpath_render.rs:818-819`) |
| Advance per tooth | `Move colour: Advance / tooth · {target}` | swatches `below`, `within`, `near max`, `above`, `no band` | `against the matched vendor band` | `advance_per_tooth_segment_color` |
| Toolpath palette | `Toolpaths` | one swatch per DRAWN toolpath: `#{n} {name}` | none | `palette_color(config index)` |
| Cut / rapid (Palette mode) | `Moves` | `cut = toolpath colour`, `rapid = darker toolpath colour` | span swatches `entry`, `lead-out`, `link bridge`, `dressup` | `render/toolpath_render.rs:227-235` |
| Collisions | `Collisions · {count}` | `isolated` … `clustered` | none | the density ramp, `app/gpu_upload.rs:959-962`. Phase 4 should move it to a named function in `render/colors.rs`. |

`{target}` is `#{n}` when `Paths: Selected`, and `all drawn` when `Paths: All`.

Rules:

- The rail shows every ACTUAL active colour encoding, with its menu closed.
- "Actual" means the rendered result, not the flag. A data-less `Deviation`
  shows no Deviation legend; it shows a Blocked line
  `Stock deviation — no deviation data — run a simulation` (inventory 2.3,
  item 3). Phase 3 decides whether the renderer keeps painting the Solid
  substitute.
- The toolpath palette line shows only when more than one toolpath is drawn,
  or when move colour is Palette. With one selected toolpath and Palette, the
  single swatch still names the operation.
- The Moves line (cut / rapid / spans) shows only when the move colour is
  Palette and cutting or rapids are on.
- The rail has no `+N`. A long line wraps. Caveat lines may collapse behind
  one `ⓘ` (`GLYPH_DETAIL`) per legend; the name, the scale and the units never
  collapse.

## 7. Mockup E — narrow layout at 320 pt

**These widths are ESTIMATES.** No app ran for this file. The estimate uses
13 pt Inter Medium at about 7.2 pt per character, the egui default
`button_padding` (`ui/tokens.rs` sets only `item_spacing` and
`interact_size.y`, lines 447-449), `SPACE_2` item spacing and a `SPACE_3`
frame margin. Phase 4 must measure with a headless pass, as
`tests/component_contracts_up2.rs:378-400` does for a narrow panel.

| Dock content | Estimated width |
|---|---|
| `View │ Scene │ Paths: Selected │ Inspect: Reach │ All ⌕ (2)` | about 400 pt |
| `View │ Scene │ Paths: Selected │ Inspect: Reach │ All ⌕` (no count) | about 360 pt |
| `View │ Scene │ Paths │ Inspect │ All ⌕ (2)` | about 250 pt |
| Available at a 320 pt viewport with a 16 pt gutter each side | 288 pt |

Width rules, in order, applied until the dock fits:

1. At viewport width ≥ 420 pt: the full dock.
2. Under 420 pt: drop the section suffixes. `Paths: Selected` becomes `Paths`.
   The target line and the rail still carry the target and the encoding.
   A Blocked or Computing glyph stays on the bare section name:
   `Inspect —`, `Inspect ◌`.
3. Under 300 pt available: the catalogue button becomes the glyph `⌕` alone,
   with the hover text of section 2. The count moves into the hover.
4. Nothing else hides. The four section buttons never hide.

```
┌─ viewport, 320 pt ─────────────────┐
│                                    │
│ Target: #12 Adaptive Pock…         │
│ ┌────────────────────────────────┐ │
│ │ Reach · #12 [▓▓▓▓▓]            │ │
│ │  miss 0.050 mm … 2.40 mm       │ │
│ │  log scale                     │ │
│ │ Move colour: Engagement · #12  │ │
│ │  [▓▓▓▓▓] 0 % … 150 % of feed   │ │
│ └────────────────────────────────┘ │
│ ┌────────────────────────────────┐ │
│ │ View │ Scene │ Paths │ Inspect │ ⌕│ │
│ └────────────────────────────────┘ │
└────────────────────────────────────┘
```

What wraps and what hides at 320 pt:

| Element | Behaviour |
|---|---|
| Section suffixes | hide (rule 2) |
| Catalogue label `All` and the count | hide at 288 pt available, because 288 is under the 300 pt limit of rule 3. The button is `⌕` alone; the count is in its hover. |
| Target line | one line; the operation name truncates with `…`; the full text is on hover |
| Legend name, scale, units | wrap onto more lines; never hide |
| Legend caveat lines | collapse behind `ⓘ` |
| Popover | width = min(320 pt, viewport width − 2 × `SPACE_5`); reason lines wrap |

**Caution — height.** The floor at `app/viewport.rs:248-256` guards width
only. No height floor exists. Under Mockup E, two legends take about five
rail lines. With the dock and the target line, the stack is about 150 pt
(estimate). Phase 4 must decide a rule for a short viewport, for example: the
rail collapses caveats first, then the whole rail folds to one line per legend
name with a `ⓘ` for its scale. It must never drop a legend.

The 320 pt guard itself: the docked Overlays column goes away with this
design, so `draw_docked`'s refusal (`panel.rs:50`) has no job left. Keep
`MIN_VIEWPORT_WIDTH` and the floor in `app/viewport.rs:252`. They protect the
3D view from the side panels, not from the dock.

## 8. Mockup F — simulation playback (dock plus the transport bar)

The Simulation layout (HEAD `src/app.rs:539-611`): the timeline is a bottom
panel `Panel::bottom("sim_timeline")` with `default_size(360.0)` and
`max_size(480.0)` (HEAD `src/app.rs:550-554`). The 3D view is the central
panel above it. The first row of the timeline is the DC6 transport frame
(HEAD `src/ui/sim_timeline.rs:75-95`): `◄`, `▶` or `❚❚`, `►`, the clock, the
basis tag, `Speed:` and the slider (HEAD `src/ui/sim_timeline.rs:1116-1244`).

```
┌─ viewport (central panel) ───────────────────────────────────────────────┐
│ ┌ deflection ┐        ◌ Loading next state…               (gizmo)        │
│ └────────────┘                                                           │
│                          simulated stock                                 │
│                  Target: #3 Back Rough (playback)                        │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │ Collisions · 7  [▓▓▓▓] isolated … clustered                      │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│      ┌─────────────────────────────────────────────────────────────┐     │
│      │ View │ Scene │ Paths: Selected │ Inspect: Solid │ All ⌕     │     │
│      └─────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────┘
┌─ sim_timeline (bottom panel) ────────────────────────────────────────────┐
│ ⚡ Mesh quality reduced for playback — pause for full render             │
│ ┌ transport ───────────────────────────────────────────────────────────┐ │
│ │ ◄  ❚❚  ►  │ 12:04 / 48:10 (machine model) │ Speed: [====●==] 4.0×  │ │
│ │ [Reset]   │ verdict HUD …                                            │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
│  boundary timeline …                                                     │
└──────────────────────────────────────────────────────────────────────────┘
```

- The dock sits in the viewport. The transport sits in the bottom panel. The
  two rows do not share a container. A `SPACE_3` gap plus the panel border
  separates them.
- In Simulation, most Paths and Inspect rows are off or Blocked by the
  defaults: `all_toolpaths`, `cutting_moves`, `rapids`, `entry_markers` are
  off (`registry.rs:252-258`, `registry.rs:290-298`); `reach_map` and
  `rest_heatmap` are Blocked (`registry.rs:946-947`, `registry.rs:816-817`);
  `model` is Blocked with "replaced by the simulated stock here"
  (`registry.rs:421-424`).
- The Inspect suffix names the stock surface here: `Solid`.
- `Reset` moves from the strip (`ui/viewport_overlay.rs:166-170`) to the
  transport frame. It is not a run route, so the DC6 "one RunSimulation
  producer" rule (inventory 5.3.3) is unaffected. The transport file is
  peer-modified today; coordinate before the edit.
- During playback, the dock and the rail do not repaint on each frame beyond
  what egui already does. The rail reads derived state; it does not start
  compute (plan: "The draw loop must neither mutate core state nor initiate
  compute").

Keyboard in Simulation: with no popover open and no focus, `Space`, arrows,
`Home`, `End`, `[`, `]` and `Escape` keep their transport meanings
(`app/input.rs:611-667`). With a popover open, `Escape` closes it only
(section 0.4).

| Element | String |
|---|---|
| Inspect suffix choices in Simulation | `Solid`, `Deviation`, `Height` |
| Reset button (moved) | `Reset` with hover `Reset the simulation to the start.` |

## 9. Mockup G — stale and failed states

**G1 — stale.** State: Toolpaths. The rest heatmap shows for #7. The operator
then edits #7, so its toolpath is `EditedSince`. Today the heatmap keeps
drawing the old grid with no mark (inventory 2.3, item 7).

```
                  Target: #7 Pencil Finish
  ┌──────────────────────────────────────────────────────────────────┐
  │ Rest · #7  [Freshness: STALE]  0.20 mm … p95 1.35 mm             │
  │   from the last generation; regenerate #7 to update              │
  └──────────────────────────────────────────────────────────────────┘
        ┌ Inspect ──────────────────────────────────────────┐
        │ Model colour                                      │
        │  ( ) Reach                                        │
        │  (•) Rest  [Freshness: STALE]                     │
        │      from the last generation of #7               │
        │      [Generate #7]                                │
        │  ( ) Off                                          │
        └───────────────────────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │ Scene │ Paths: Selected │[Inspect: Rest !]│ All ⌕    │
      └─────────────────────────────────────────────────────────────┘
```

| Element | String |
|---|---|
| Stale cue | `components::Freshness` for `FreshnessState::EditedSince` (its own words; no new badge) |
| Stale rail line | `from the last generation; regenerate #{n} to update` |
| Stale row line | `from the last generation of #{n}` |
| Stale action | `Generate #{n}` (pushes `AppEvent::GenerateToolpath(id)`, the event that `panel.rs:293` already uses) |
| Dock suffix glyph | `!` (`GLYPH_CAUTION`) |

The stale source is `freshness::freshness_at` (`app/viewport.rs:536-547`
reads it for the line dimming). Phase 3 reads the same function. It must not
store a stale flag (`state/CLAUDE.md`, invariants 1 and 2).

For the simulation surfaces, the stale source is `AppState::simulation_is_stale`
(`state/CLAUDE.md`, invariant 3). The same cue applies to `Simulated stock`,
the stock colours and `Collisions`.

**G2 — failed.** State: Toolpaths. The reach walk for #12 failed. Today the
row says "select a finishing operation" (inventory 2.3, item 2).

```
                  Target: #12 Adaptive Pocket
  ┌──────────────────────────────────────────────────────────────────┐
  │ Reach · #12  ✕ failed — the model draws plain                    │
  └──────────────────────────────────────────────────────────────────┘
        ┌ Inspect ──────────────────────────────────────────┐
        │ Model colour                                      │
        │  (•) Reach  ✕ failed                              │
        │      {message}                                    │
        │      [Retry]                                      │
        │  ( ) Rest                                         │
        │  ( ) Off                                          │
        └───────────────────────────────────────────────────┘
      ┌─────────────────────────────────────────────────────────────┐
      │ View │ Scene │ Paths: Selected │[Inspect: Reach ✕]│ All ⌕   │
      └─────────────────────────────────────────────────────────────┘
```

| Element | String |
|---|---|
| Failed word | `failed` with `GLYPH_DANGER` in `DANGER` |
| Rail line | `failed — the model draws plain` |
| Message | the `ReachStatus::Failed(message)` text, verbatim |
| Recovery | `Retry` |
| Retry hover | `Ask for the reach map of #{n} again.` |

`Retry` clears the reach key so that the scheduler submits again. This is the
same move as the stall recovery (`controller.rs:657-668`,
`ReachOverlayState::clear`). The click is an intent; the scheduler does the
submit. The draw loop starts no compute.

## 10. The catalogue — All viewport options

It replaces the ~40-row Overlays panel. It is a floating `egui::Window`, not a
docked column. It lists all 39 registry rows, grouped by DOCK section, plus
the non-registry controls of section 1.

```
┌ All viewport options ─────────────────────────────── ✕ ┐
│ ⌕ [ filter by name or id…                           ]  │
│ Show  (•) All  ( ) Changed  ( ) Cannot draw            │
│                                                        │
│ VIEW                                                   │
│  ☑ Orientation gizmo                        orientation_gizmo
│ SCENE                                                  │
│  ☑ Grid                                     grid       │
│  ☐ Fixtures                                 fixtures   │
│    — no fixture in this setup                          │
│  …                                                     │
│ PATHS: SELECTED                                        │
│  …                                                     │
│ INSPECT: REACH                                         │
│  (•) Model colour: Reach  ◌ computing…  [Cancel all jobs]
│  …                                                     │
│                                                        │
│ 39 options · 2 changed from this workspace's defaults  │
└────────────────────────────────────────────────────────┘
```

| Element | String |
|---|---|
| Window title | `All viewport options` |
| Search hint | `filter by name or id…` |
| Filter label | `Show` |
| Filter choices | `All`, `Changed`, `Cannot draw` |
| Section headers | `components::SectionHeader`: `VIEW`, `SCENE`, `PATHS: SELECTED` or `PATHS: ALL`, `INSPECT: {suffix}` |
| Row id column | the registry `id`, in `TEXT_MUTED` monospace; it is the MCP name, so an agent and the operator can match them |
| Footer | `{n} options · {m} changed from this workspace's defaults` (`m` from `registry::non_default_count`) |
| Empty filter result | `No option matches "{query}".` |

Rules:

- The catalogue lists every row in every state, as the panel does today
  (`registry.rs:9-13`: "Every overlay appears in the list, always").
- A catalogue row and a dock row call the SAME setter,
  `registry::set_overlay`. The catalogue is a second VIEW of one state, not a
  second writer.
- `Changed` shows rows where `get != default_for(workspace)`. `Cannot draw`
  shows rows whose precondition is not `Ready`.
- The catalogue holds no legend block. The rail holds every legend. The reach notes that `tests/overlays_registry.rs`
  requires in `panel.rs` move with the rail code (inventory 5.1.11, 5.1.12).
- `Escape` closes the catalogue. With the search field focused, the first
  `Escape` clears the query and the second closes the window.

## 11. What changes for the operator

### 11.1 What leaves

| Today | After |
|---|---|
| The top strip above the 3D view (`ui/viewport_overlay.rs`): `View ▼`, `Persp ▼` / `Ortho ▼`, `Overlays (n)`, `Selected only` / `All toolpaths`, the compute label, `Cancel All`, `Reset` | Removed. View and projection go to View. The scope goes to Paths. The count goes to the catalogue button. The compute label and the cancel go to the status bar. `Reset` goes to the transport. |
| The Overlays panel, docked or floating (`ui/overlays/panel.rs`, 4 groups, 39 rows, legend block at the end) | Replaced by the four popovers for daily use and the catalogue for the full list. |
| The pinned (docked) column and the pin button | Removed. The dock takes no column. |
| The legend block at the end of the panel | Replaced by the legend rail, always visible. |
| The group names Geometry, Toolpath, Regions, Analysis | Replaced by View, Scene, Paths, Inspect. |

### 11.2 What stays

- Every registry `id`, and so every MCP overlay name.
- `set_ui_view`'s `overlays` semantics: applied or refused, the refusal
  quotes the row's own reason, radio rows refuse OFF, Readiness refuses all
  (`registry.rs:1334-1380`).
- Every per-workspace default (`registry.rs:224-302`) and the displaced-value
  restore (`registry.rs:1408-1428`).
- Exclusivity on the model, stock and moves surfaces (`registry.rs:1301-1311`).
- The keys `S`, `P`, `R`, `X`, `,`, `.`, `1`-`4`.
- The per-operation eye, C and R on each operation row.
- The automation ids `overlay_cancel_all` and `overlay_collision_check`
  (`src/controller/tests/smoke.rs:33-34`). Keep them on the new controls:
  `overlay_cancel_all` on `Cancel all jobs`, `overlay_collision_check` on the
  Inspect button (the Collisions row lives there).
- The WP27 rule: the selected toolpath is the default draw set.
- `toolpaths_to_draw` stays the one draw and pick rule
  (`tests/viewport_draws_selected_only_wp27.rs:222-235`).

### 11.3 Reason strings that name the old groups

These strings name a group path. The regrouping makes them wrong. Three of
them are precondition reasons, so they also go out in MCP refusals.

| Line | Today | Proposed |
|---|---|---|
| `registry.rs:894-895` (reason, MCP) | `drawn by Regions ▸ Tier map — no separate island outline renderer yet` | `drawn by Inspect ▸ Tier map — no separate island outline renderer yet` |
| `registry.rs:950-951` (reason, MCP) | `the reach map IS the model, re-coloured — switch Geometry ▸ Model on` | `the reach map IS the model, re-coloured — switch Scene ▸ Model on` |
| `registry.rs:836-837` (hover) | `… clears Analysis ▸ Model colour: Reach, which shares the model surface.` | `… clears Inspect ▸ Model colour: Reach, which shares the model surface.` |
| `registry.rs:971-972` (hover) | `Clears Regions ▸ Rest heatmap — both colour the model.` | `Clears Inspect ▸ Rest heatmap — both colour the model.` |
| `registry.rs:1352` (MCP refusal) | `unknown overlay id '{id}' — see the Overlays panel` | `unknown overlay id '{id}' — see All viewport options` |
| `registry.rs:660` (hover) | `Green fed moves, every toolpath. Per-toolpath: each row's C.` | `Fed moves, in each toolpath's colour. Per-toolpath: each row's C.` |
| `registry.rs:683` (hover) | `Orange rapid moves. Per-toolpath: each row's R.` | `Rapid moves, a darker toolpath colour. Per-toolpath: each row's R.` |

The last two fix a colour claim that is wrong today (inventory 3).

### 11.4 The `O` and `Shift+O` keys

- **`O` — keep, and remap to the catalogue.** `O` opens and closes the Overlays
  panel today (`app/input.rs:551-560`). The catalogue is the panel's
  successor: it lists the same 39 rows. An operator who presses `O` gets the
  full list, as before. `S`, `P`, `R` and `X` are taken, so `O` is also the
  only free mnemonic.
- **`Shift+O` — retire.** It pins the panel as a docked column
  (`app/input.rs:554-556`). The design has no docked column, so the key has
  nothing to do. The operator ruled on 2026-09-16 that old shapes go outright
  and the commit states the break. A remap to a new meaning would teach a
  second meaning for a known key. Retirement is the clearer break.
- Update `ui/shortcuts_window.rs:43-52` in the same commit:
  `("O", "Open / close All viewport options")`, and delete the `Shift+O` row.
  The same window lists `("I", "Toggle isolation")` (36). No `Key::I` binding
  exists in `src/` today, so that row names a dead key; delete it too.
- `OverlayPanelState::pinned` and `OverlayPanelState::groups`
  (`state/overlays.rs:40-42`) become dead. `open`, `defaults_applied_for` and
  `displaced` stay (`registry.rs:1391-1428` reads the last two).

## 12. Open questions for the operator

- **Q1.** Does a click outside a popover, on the 3D view, only close the
  popover, or also select there? This file recommends "only close".
- **Q2.** Does `AutoEnableRestAnalysis` always clear the simulation? The
  confirm text says `will` or `may` from the answer (Mockup B2).
- **Q3.** Move `Cancel all jobs` to the status bar, or keep a job line above
  the dock? This file recommends the status bar, where the lane chips are.
- **Q4.** The inspector's `Show reach map` checkbox is a second writer that
  bypasses exclusivity (inventory 2.3, item 6). Delete it, or route it through
  `registry::set_overlay`? The redesign exposes it; the fix is not phase 2.
- **Q5.** Keep painting the Solid substitute for a data-less Deviation, or
  paint the model plain and label it (inventory 2.3, item 3)?
