# Full UX review of the rs_cam GUI — viewport, overlays, toolpaths, visual surfaces

You are a senior product designer and interaction reviewer. You are
reviewing a desktop CAM application for a 3-axis wood router, used by ONE
operator who machines terrain models (mountain maps at 1:200) from plywood
and hardwood. The review is a read-only audit with a written report. Do
not change source code unless the operator asks you to afterwards.

## The product in one paragraph

`rs_cam` (this repo, Rust workspace) has a core CAM engine, a CLI and a
desktop app `rs_cam_gui` (`crates/rs_cam_viz`, egui 0.34 + wgpu 29). The
app has four workspaces: Setup (stock, datum, machine, fixtures), Toolpaths
(operations list, per-op inspector, viewport), Simulation (dexel stock
playback, diagnostics, timeline) and Readiness (export gates). The 3D
viewport draws the model, stock, toolpaths in three colour modes, and a set
of overlays (rest heatmap, tier map, reach map, collisions, deflection,
entry markers, height planes, cutter ghost). The app embeds an MCP server
(`--mcp`) so an agent can drive it and take screenshots — that is how you
will look at it. Read `README.md`, `FEATURE_CATALOG.md` and
`planning/PROGRESS.md` (top entry) first, then `CLAUDE.md` (house rules;
prose is ASD-STE100).

## What changed this week, so you review the CURRENT state

- P5 reach map (2026-09-08): a per-tool "can this ball reach the valleys"
  overlay on the model whenever a finishing op is selected; depth ramp
  (green ≤ bar, grey unresolved, log yellow→red); legend and inspector
  line state the grid, the floor, the bar and the area basis.
- P6 Overlays panel (2026-09-08): ONE registry (`crates/rs_cam_viz/src/ui/
  overlays/registry.rs`, 38 rows in Geometry / Toolpath / Regions /
  Analysis) feeds an `Overlays (n)` toolbar button, the MCP `set_ui_view`
  `overlays` map, and sentries. Every overlay is always listed; one that
  cannot draw is greyed with its reason and a compute button. One colour
  source per surface (model / stock / moves). Per-workspace defaults. The
  old `Show ▼` and `Shaded ▼` menus and the Inspector view items are gone.
- Design + audit that produced P6: `planning/ui_overlays_ux_2026-09-08.md`
  (42-overlay inventory, seven screenshots in the sibling dir) and
  `planning/ui_overlays_dead_duplicate_2026-09-08.md` (what was dead or
  duplicated; §6 lists 17 fixes, all applied). Read both — they are your
  baseline, and you should CHECK them against the live app rather than
  trust them.
- Earlier IA work: `planning/ui_audit/` (IA_TARGET.md, FINAL_DESIGN.md,
  MAP.md, DENSITY_PASS_2026-06-11.md, and `surfaces/*.md` — 64 per-surface
  notes, some stale). `planning/UI_IA_AUDIT_WORKFLOW.md` describes how
  those were produced.

Known open items — do not spend effort re-discovering them, but do say if
you disagree with how they are ledgered:
- derived rest regions and the machining boundary outline have NO renderer
  (listed disabled "not drawn yet");
- the reach overlay blinks on any project edit (coarse cache key, P5-BLINK);
- the live viewport cannot thin lines (wgpu LineList), only dim them;
- G-LEADGATE: an op with entry_style = None and lead-in/out on gets
  stock-blind lead arcs in the GUI worker;
- the Overlays panel's row for "tool-profile ghost" needs a selected
  toolpath.

## How to look at it

1. Build once (the binary is usually already built): `cargo build --release
   -p rs_cam_viz --bin rs_cam_gui`. CARGO LANE RULE: other sessions share
   this machine; before ANY cargo command run `pgrep -af "carg[o]"` (must
   be empty) and `free -g` (≥ 10 Gi available); one cargo job at a time;
   never run the workspace-wide test suite or the heavy-tests feature.
2. The GUI is launched by your MCP config (`.mcp.json` runs
   `target/release/rs_cam_gui --mcp`). If the `rs-cam` MCP tools are not
   connected, ask the operator to run `/mcp` and reconnect. Never edit
   `.mcp.json`.
3. Load real data: `load_project` with
   `planning/deep_doc_modulation_2026-09-08/Q2_r20_s15.toml` (wanaka
   plywood front, one enabled finishing op; `load_project` auto-generates
   enabled ops — wait or `cancel_generation`) or
   `planning/multitool_2026-08-23/wanaka200_iso_scallop.toml` (the full
   multi-setup project: pin drills, back rough, rivers/lakes DXF curves,
   front rough + finish + pencil). For the Simulation workspace run
   `run_simulation` at `resolution: 0.4` (fast) or 0.2 (fine, 3-5 min).
4. Navigate with `set_ui_view` (workspace, toolpath_index, properties_tab,
   select, modal, overlays) and capture with `screenshot_gui { path,
   width, height }` — the FULL window, all panels. Capture EVERY surface
   at TWO sizes: 1400×900 (laptop) and 2400×1300 (desk). The size persists,
   so set it back. `screenshot_toolpath` / `screenshot_simulation` render
   the 3D scene offscreen without the UI (use `include_rapids: false` and
   `reach_overlay: true` where relevant). Read each PNG you take. Save all
   PNGs under `planning/ui_review_2026-09-09/shots/` with names that say
   workspace-surface-size.
5. Also read the code for any control you cannot explain from the screen:
   `crates/rs_cam_viz/src/ui/` (one file per surface), `app/viewport.rs`
   (what the viewport draws per frame), `render/` (pipelines), `state/`
   (flags). Prefer `rg` for exact names.

## What to review (all of it, in this order)

A. **Viewport rendering.** Model shading, stock box/solid, origin/datum,
   fixtures/pins/keep-outs, DXF curves, grid, gizmo. Toolpath drawing:
   palette colours per op, cutting vs rapids, engagement and advance-per-
   tooth heat maps and THEIR legends, entry markers, height planes, cutter
   ghost, isolation. Overlays: rest heatmap, tier map + planner islands,
   reach map (depth ramp, grey unresolved), collisions, deflection,
   simulated stock with deviation / by-height colouring, opacity. Judge:
   can the operator tell what they are looking at without the legend? Do
   colours mean one thing everywhere (green = ?, red = ?), or does the same
   hue mean "reachable", "cutting move" and "within limit" on three
   surfaces at once? Are legends where the eye goes? Does anything z-fight,
   flicker, blink, or vanish on a workspace switch?
B. **The Overlays panel.** Discoverability of the button, the badge count,
   pinned vs floating, group order, row labels (are they the operator's
   words?), disabled reasons (do they say what to DO?), compute buttons,
   per-workspace defaults (are they the right defaults for each task?),
   colour exclusivity behaviour when you switch a colour source, the
   legend strip, shortcut keys (O, Shift+O, S, P, R, X, comma, period —
   are they discoverable, documented, conflicting?).
C. **Toolpaths workspace.** Operations list rows (status pill, tool line,
   time/moves/distance line, eye/C/R/bullseye controls), setup grouping,
   drag/reorder, selection ↔ viewport highlighting, the inspector
   (Geometry / Feeds & Speeds / Linking / Heights / Dressup tabs), hints,
   the "Show reach map" line and its numbers, the Generate/Generate All
   affordances, stale-state signalling after a parameter edit.
D. **Simulation workspace.** Timeline scrubbing, playback controls, the
   diagnostics panel (triage first?), NOT MEASURED strips, hotspots,
   per-op pills (kinematic utilization, plunge class), deviation colouring
   readability, the analytics sections.
E. **Setup and Readiness.** Stock/datum/fixture editing feedback in the
   viewport; readiness gates wording; export wizard.
F. **Cross-cutting.** Information hierarchy and density (see
   DENSITY_PASS), consistency of terms across panels, MCP reply text vs
   GUI text (an agent and a human should read the same words), keyboard
   reach, first-run experience with an empty project, error and refusal
   states, window-size robustness (nothing may collapse or overflow at
   1400×900), theme/contrast of every colour ramp against the dark
   background, and anything that is a CONTROL THAT DOES NOTHING (the
   audit found five of these last week; assume more exist).

## Method

- Walk six operator tasks end to end and screenshot each step: (1) open a
  project and understand what is loaded; (2) set the stock, datum and
  machine; (3) add a finishing op, generate it, judge whether its tool
  reaches (reach map) and whether it is efficient (air %, retracts);
  (4) simulate and find the worst thing (collision, entry load, chipload);
  (5) compare two ops (a raster vs an iso-scallop) on time and finish;
  (6) export. At every step ask: what did I have to already know?
- For every finding give: severity (blocks a task / misleads / slows /
  cosmetic), the screenshot, the exact control or pixel region, why it is
  a problem for THIS operator, the proposed change, and a rough effort
  (one-liner / a day / a programme). Separate DEFECTS (it does not do what
  it says) from DESIGN (it does what it says and that is the wrong thing).
- Check the two 2026-09-08 docs' claims against the live app and list
  every claim that is now false.
- Be concrete and opinionated. Mockups as ASCII or annotated screenshots
  are welcome. Do not propose a redesign of everything; propose the ten
  changes that would most improve the operator's day, ranked, then the
  long tail.

## Deliverables

`planning/ui_review_2026-09-09/REVIEW.md` with: an executive summary (ten
lines), the ranked top-ten, the full findings table, the per-task
walkthroughs with screenshots, the "claims now false" list, and a proposed
programme split into cheap fixes / a P7 design pass / deferred. Screenshots
in `shots/`. Write in short sentences (STE). Commit nothing; tell the
operator what to commit.
