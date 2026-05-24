# UX Testing Session Plan — 2026-05-11

## Purpose

Walk a real user (me, via MCP + GUI) through the operations a person actually does
end-to-end and record where the experience falls down. We're hunting for:

- **Too much info** — long lists of warnings, walls of text, every move flagged red.
- **Silent state changes** — values flip without surfacing in the UI; toolpaths
  go stale without a banner; optimizer applies a patch with no diff.
- **Verdict without next step** — "Exceeds power" with no suggestion, no link to
  the offending move, no explanation of how to fix it.
- **Counts without context** — "1247 issues" with no severity breakdown.
- **Lists vs graphs** — point-issue lists where continuous time-data would be
  honest (the F6 reframe; this session validates whether that landed cleanly).
- **Stale data shown as fresh** — toolpath edited but graph still shows old run.
- **Defaults that need editing** — the first thing a user sees on a new toolpath
  must be sensible; if every default has to be changed, the defaults are wrong.
- **Ambiguous affordances** — "is this button going to regenerate or just save?"
- **Missing model-limitation framing** — engagement reads ~0 on 2D ops; do we
  tell the user that, or let them think the toolpath is air-cutting?

## Fixtures

Existing geometry — no downloads needed:

| File | Type | What it exercises |
|------|------|-------------------|
| `fixtures/demo_pocket.svg` | 2D | rounded rectangle with circular hole — pocket+profile+drill defaults |
| `fixtures/demo_star.svg` | 2D | 5-point star — sharp internal corners, v-carve, trace |
| `fixtures/terrain_small.stl` | 3D mesh | adaptive3d + scallop/drop_cutter finishing |
| `fixtures/gui_step/block_40x40x60.step` | STEP | flat-top block — face-selective pocket on top face |
| `fixtures/gui_step/plate_100x60x10.step` | STEP | thin plate — single setup, profile/pocket on top face |
| `fixtures/gui_step/l_bracket.step` | STEP | L-bracket — multi-face geometry, face picker UX |
| `fixtures/gui_step/stepped_block.step` | STEP | multi-level steps — face selection across heights |
| `~/Downloads/wanaka100/...` | mixed (STL + DXF) | mature multi-setup project (already dial-in tested) |

Skeleton project TOMLs (this PR — no toolpaths, just stock/tools/machine/model
so the session exercises "what defaults do I get when I create a toolpath?"):

- `test_data/ux_2d_pocket.toml` — wraps `demo_pocket.svg`
- `test_data/ux_2d_star.toml` — wraps `demo_star.svg`
- `test_data/ux_step_block.toml` — wraps `block_40x40x60.step`
- `test_data/ux_step_lbracket.toml` — wraps `l_bracket.step`
- `test_data/ux_step_plate.toml` — wraps `plate_100x60x10.step`
- `test_data/ux_step_stepped.toml` — wraps `stepped_block.step`
- `test_data/ux_3d_terrain.toml` — wraps `terrain_small.stl`

Wanaka stays the gold-standard reference (it's already a fully built multi-setup
project with known dial-in characteristics, so it covers the "load mature
project, what does the warnings layer look like at scale?" journey).

## Journeys

Each journey runs against one or more fixtures. Each step records:

- what was shown,
- what was confusing or missing,
- what was redundant,
- what would make a non-author user dial it in correctly.

### J1 — First-load impression (every fixture)

Open the project. Before touching anything: what does the viewport show? Does
the stock fit the model sensibly? Do alignment pins land in valid positions?
Are tools sensible defaults? Does machine config make sense for the material?

### J2 — Stock setup

Start fresh, set stock from model bbox, change material, change pin diameter,
move pins, switch workholding rigidity. Does the auto-from-model toggle
behave sensibly? What about the new edge-clearance offset on auto-pin?

### J3 — Tool creation / editing

Add a new end mill, add a new ball nose. Are the LUT-sourced fields obviously
LUT-sourced vs user-set? When you edit a tool that's referenced by toolpaths,
is the staleness propagation visible?

### J4 — Toolpath creation defaults (per operation family)

For each operation family, add a new toolpath and just look at what's there:

- **2D contour** — pocket, profile, drill, v_carve, trace, adaptive (svg/dxf)
- **3D roughing** — adaptive3d (stl/step)
- **3D finish** — drop_cutter, scallop, waterline (stl/step)
- **Curve projection** — project_curve onto a surface model
- **STEP face-selective** — pocket on a selected face (block, plate, stepped)

What feeds/speeds defaults appear? Are they realistic for the material?
What heights are auto vs manual? What does the params panel look like the
moment you arrive on it?

### J5 — Param edit → generate → simulate cycle

Change one parameter (stepover, depth_per_pass, feed_rate). Does the toolpath
go visibly stale? Is the regenerate affordance obvious? After regenerating, is
the previous simulation result invalidated visually?

### J6 — Simulation results — graphs and warnings

Run a simulation. Look at: timeline graph, point markers, hotspot list,
diagnostic counts. Is it clear what's *risky* vs what's *suboptimal*? Does
the F6.1 viewport (no per-bucket pins) feel cleaner? Are the things that
remain on the timeline only the genuinely dangerous moves?

Specifically: load wanaka, simulate everything, count how many "issues" the
GUI shows. If it's still in the thousands, the F6.2-F6.4 reframe (per-criterion
graphs with bands and time-in-bounds %) is still pending and we should note
which surfaces are loudest.

### J7 — Tool-load report — header + per-toolpath verdicts

Open the tool-load report. Does the F5 summary header (within / exceeds /
fully_unmodeled + breakdown) tell you what to look at first? Are individual
toolpath verdicts actionable? Does a "Within" verdict give you confidence,
or do you still feel uncertain?

### J8 — Optimizer journey

Run the optimizer on a toolpath that has Exceeds verdicts. Does the candidate
list explain *why* each variant won? Does the diff between baseline and
proposed make sense? Does the F1 RPM-down compensation visibly fire on
chipload-low + feed-clamped cases?

Apply the patch. Does the toolpath regenerate? Are the changed params
visible in the panel? Does the simulation result update?

### J9 — Error reasoning — can a user decide if a verdict is acceptable?

Pick three different verdicts (a chipload exceeds, a power exceeds, an
air-cut warning). For each: can a user without engine knowledge reason
about whether to accept it, fix it, or ignore it? What information would
they need that isn't shown?

### J10 — STEP-specific journeys

- **Face picker** — load `block_40x40x60.step`, use BREP face selector to
  pick the top face, add a pocket. Is the selector discoverable? Does it
  show face normals / face IDs in a way a user can map to geometry?
- **Multi-feature STEP** — load `stepped_block.step` or `l_bracket.step`,
  try to plan operations that use different faces. Does the UI cope when
  multiple operations reference different faces of the same model?

### J11 — Drill carve-outs

Load a project with drill ops (wanaka has Pin Drill + Holes). Confirm F3's
same-XY descent carve-out kicks in: simulation should now show 0 rapid
collisions on peck cycles where it used to show many. If it doesn't, that's
a regression.

### J12 — Export

Export g-code from a finished project. Does the post tell you what it did?
Does it warn about anything it dropped or simplified?

## Deliverable

`planning/UX_PAIN_POINTS_2026-05-11.md` with bullets organized by journey,
each tagged with severity:

- **🔴 Critical** — blocks a user from getting a correct result, or hides
  information that would change a safety decision.
- **🟡 Important** — adds friction or causes confusion but the user can
  recover with effort.
- **🟢 Polish** — cosmetic or minor flow improvements.

End with a summary table: rows = surface (stock panel, tool panel, sim
graph, viewport, optimizer panel, tool-load report, export dialog),
columns = severity counts, so we can pick the next surface to invest in.

## Common-negatives watchlist

The session must specifically check for these patterns at every step:

1. Silent state mutation (value flips without UI feedback).
2. Verdict shown with no actionable next step.
3. Count shown with no severity breakdown.
4. Same color used for two unrelated severities.
5. List of issues where a graph would be more honest.
6. Stale data shown as fresh after edits.
7. Modal panel that blocks the viewport.
8. Defaults that every user has to change.
9. Missing framing for known model limitations (2D engagement, drill
   air-cut, etc.).
10. Ambiguous button text or icon affordance.

## Confirmed (2026-05-11)

- Deliverable: `planning/UX_PAIN_POINTS_2026-05-11.md`.
- Severity scale: 🔴 / 🟡 / 🟢.
- Fixtures: 7 skeletons + wanaka. No additional pre-stuffed synthetic
  project — wanaka covers warnings-at-scale.
- Logging: findings only (no per-journey timing).
