# Investigation — Break Feeds & Speeds out into its own workspace

**Date:** 2026-06-02
**Trigger:** Operator suggestion to promote the Feeds & Speeds modal
to a full top-level workspace (alongside Setup / Toolpaths /
Simulation). Idea: more screen real-estate for richer per-toolpath
data — per-axis chipload, effective chipload along the cutting
length of tapered tools, side-by-side comparison of strategies, etc.

## TL;DR

**Easy.** The GUI's workspace switcher is already designed for this
exact extension. Three workspaces today, all dispatched through one
`match` in `app.rs:503-506`. Adding a fourth is **additive** —
~5-7 touch points, all in `rs_cam_viz` — and no core or domain
changes needed.

The harder question is **what goes IN the workspace**. The
content/layout design is a bigger conversation than the wiring.
That's where the time should be spent.

## Current workspace architecture (read this first)

### Enum
`crates/rs_cam_viz/src/state/mod.rs:17`:
```rust
pub enum Workspace {
    Setup,
    Toolpaths,
    Simulation,
}
```

Default: `Workspace::Toolpaths` (`state/mod.rs:206`).

### Switcher bar
`crates/rs_cam_viz/src/ui/workspace_bar.rs` — clean tab strip below
the menu bar. Each tab calls `workspace_tab(ui, "Setup",
Workspace::Setup, current, ...)`. Adding a fourth is one line.

### Layout dispatch
`crates/rs_cam_viz/src/app.rs:503-506`:
```rust
match self.controller.state().workspace {
    Workspace::Setup => self.draw_setup_layout(ctx),
    Workspace::Toolpaths => self.draw_toolpath_layout(ctx),
    Workspace::Simulation => self.draw_simulation_layout(ctx),
}
```

Each layout method is ~40 lines, sets up `SidePanel::left` +
`SidePanel::right` + `TopBottomPanel::bottom` + `CentralPanel`,
populates them via existing UI module draws.

### Other touch points
- `app.rs:127-129` — workspace name parsing for CLI/URL args
- `app.rs:452-454` — keyboard shortcut routing
- `app.rs:650` — context-specific overlay condition
- `ui/viewport_overlay.rs:238-245` — workspace-specific overlay text
- `ui/sim_op_list.rs:167` — cross-workspace navigation event

All trivial conditionals; the largest is `viewport_overlay.rs`'s
match.

## What "adding a Workspace::FeedsSpeeds" looks like

### Cost (the easy part — half a day at most)

1. **`state/mod.rs`** — add `FeedsSpeeds` to the enum (1 line).
2. **`workspace_bar.rs`** — add a tab (5 lines).
3. **`app.rs:127`** — CLI arg parsing for the new workspace name
   (1 line).
4. **`app.rs:452-454`** — route keyboard shortcuts (1 arm).
5. **`app.rs:503-506`** — main dispatch arm (1 line).
6. **`app.rs:650`** — overlay-condition handling if applicable.
7. **`ui/viewport_overlay.rs:238`** — overlay arm (~5 lines).
8. **New module** `crates/rs_cam_viz/src/app/layout_feeds_speeds.rs`
   or extending `app.rs` — the new `draw_feeds_speeds_layout(ctx)`
   method. Pattern: `SidePanel::left` (toolpath picker tree),
   `SidePanel::right` (feeds derate breakdown + apply/suggest
   controls), `CentralPanel` (the rich visualisation area).

That's the architecture cost: half a day, mostly mechanical.

### Cost (the medium part — content)

The existing modal is ~960 wide × 620 tall by default. Promoting
to a workspace gives ~1600 × 900 typical. That's 2.4× the area;
the question is what fills it.

Decisions to make:

1. **Keep or remove the existing modal?**
   - Promotion path: modal stays as a "quick view" reachable from
     anywhere; full workspace is the deep-dive. Both consume the
     same `FeedsExplain` payload.
   - Replacement path: modal goes away; workspace is the only home
     for feeds. Fewer surfaces to maintain.
   - **Recommendation:** keep both. Modal is the inline-while-
     editing-toolpath workflow; workspace is the dedicated session.

2. **Per-axis breakdown layout.**
   - Left rail: per-toolpath list (with feed/RPM/chipload badges,
     same density as the current `toolpath_panel`).
   - Centre: the chosen toolpath's full breakdown. Vertical real
     estate for the existing derate-chain card PLUS new charts.
   - Right: vendor LUT row inspector / comparison.

3. **Rich visualisations we don't have yet (the actual prize).**
   - **Effective chipload along cutting length** for tapered/V-bit
     tools (the operator's specific request). Plot:
     X axis = engagement depth (mm), Y axis = effective chipload
     (mm/tooth) at the current feed/RPM. Annotate LUT min/max
     bands. Mark where the operation's actual DOC sits.
   - **Constant-chipload curve** in feed-RPM space (already drawn
     in some modal charts I saw at lines 1273+). Promote to a
     full chart with grid, axes, and clickable operating points.
   - **Side-by-side strategy comparison**: Match chart vs Max
     speed at a glance, with cycle-time and engaged-power numbers.
   - **Per-flute chipload diagram** for non-equal-flute tools (if
     we ever support those).
   - **Per-Z-level engagement preview**, hooking into sim data.

   Each of these is its own widget. Time per: 1-3 days. The whole
   thing is a multi-week effort to do all five.

### Cost (the hard part — model breakup)

The existing modal is mostly self-contained: takes
`&AppState + toolpath_id`, emits `Vec<AppEvent>`. Promoting to a
workspace adds:

- **State**: which toolpath is selected in the workspace
  (separate from the modal's transient state). Add
  `state.feeds_workspace_selected_toolpath: Option<ToolpathId>` —
  trivial.
- **Persistence**: does the workspace selection persist across
  launches? Probably yes (load last viewed). One line in
  the session-state save/restore.
- **Cross-workspace nav**: jumping FROM the Toolpaths workspace
  TO the Feeds workspace on a specific toolpath. Need a new
  `AppEvent::FocusFeedsToolpath(ToolpathId)`. The existing
  `OpenFeedsModal(ToolpathId)` is the obvious template; promote
  it to take a `FeedsView::{Modal, Workspace}` discriminant.
- **Modal/workspace shared rendering** — the current modal helpers
  (`draw_comparison_card`, `draw_derate_chain`, etc.) are
  module-private. Promote them to `pub(crate)` so the workspace
  can call them. Trivial visibility change.

No architectural smells. The modal was structured well enough
that promoting it is a refactor-not-rewrite. Estimate: 1-2 days
to do the structural break-out, then content fills in.

## Risk areas

1. **Modal currently runs as a top-level `egui::Window` with
   `anchor(CENTER_CENTER)`.** Workspace will be in a series of
   `SidePanel`s + `CentralPanel`. The existing rendering helpers
   use `ui.label`, `ui.horizontal`, etc — those work fine in
   either container. But the `egui::Frame::group` styling may
   look different in a flat panel vs a windowed modal. Visual
   tuning needed.

2. **The existing modal carries its own state object**
   (`state.feeds_modal: Option<FeedsModalState>`). Workspace state
   is different (persistent, multi-toolpath nav). Two state slots
   coexisting is fine; just don't conflate them.

3. **Sim coupling.** The simulation workspace's bottom panel
   (timeline) wouldn't be in the feeds workspace by default, but
   the feeds workspace might WANT a small "current simulated
   engagement" indicator. Cross-workspace data isn't a problem
   (state is shared) but the UI semantic decision is: do you
   need sim trace to be present for the feeds workspace to be
   useful? Initially no — pre-sim feeds-explain rendering is
   already what the modal does. Sim coupling can come later as
   an enrichment.

4. **The unstaged WIP rule.** Per session memory,
   `crates/rs_cam_viz/src/ui/feeds_modal.rs` is on the "never
   stage" list. If the workspace breakout reuses modal helpers
   they'd need to move to a different file (e.g.
   `ui/feeds_shared.rs`) that's NOT on the no-stage list.
   Otherwise the workspace breakout commits would either need
   to also touch feeds_modal.rs (forbidden) or duplicate the
   helpers (ugly).

   **Resolution path:** before starting the breakout, move the
   shared rendering helpers out of feeds_modal.rs into a new
   `feeds_shared.rs` module. That's a refactor commit on its
   own that the user explicitly approves (since it touches
   feeds_modal.rs). Then the workspace breakout uses
   feeds_shared.rs cleanly.

## Recommended sequencing

1. **First:** land the 4 sub-items from
   `planning/feeds_modal_enhancements_2026-06-02.md` so the modal
   is the right shape (knows about scallop, knows about engaged
   diameter, knows about chipload-min). ~1 day.

2. **Then:** the helpers-extraction refactor — move
   `draw_comparison_card`, `draw_derate_chain`, `derate_row`,
   the chart functions, etc. out of `feeds_modal.rs` into a new
   `ui/feeds_shared.rs`. `feeds_modal.rs` becomes a thin
   adapter calling into the shared module. ~half day.

3. **Then:** the workspace wiring (the 5-7 small files). The
   workspace draws the shared widgets in a different layout.
   ~half day.

4. **Then:** the content. Rich visualisations one at a time.
   Effective-chipload-along-tool-length first (the operator's
   specific ask, and we already have `lookup_diameter_at` to
   build the X axis). ~2-3 days for first widget; subsequent
   widgets faster as the pattern lands.

## Total

- Architectural breakout: ~1.5 days (steps 2-3)
- First rich widget (chipload-along-tool): ~2 days
- Polish and additional widgets: open-ended

**The architecture is friendly to this.** The constraint is
content, not plumbing. When the time comes, start with the
helpers-extraction refactor first so the workspace promotion is
clean.

## Decision needed before starting

- Confirm the modal stays as a "quick view" or is replaced (my
  recommendation: keep).
- Decide which rich widget is the first build target. The user
  flagged "effective chipload along cutting length" as the
  motivating example. That fits because we already have all the
  underlying data (`lookup_diameter_at`, `vendor_normalize::
  lookup_diameter_for_input`, the chipload band from the matched
  LUT row).

No build needed for this investigation — pure planning.
