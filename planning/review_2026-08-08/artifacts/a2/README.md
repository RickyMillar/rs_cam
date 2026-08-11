# A-2 — the live before/after heat-map pair (F-HEATMAP, Checkpoint H4.2)

Captured 2026-08-11. **These are GUI captures of a running
`rs_cam_gui --mcp`**, not derived renders — A-1's `artifacts/a1/`
swatch was derived and said so; this is the live evidence its ledger
row demanded.

## Which build each PNG came from

| file | build | commit | colour mode |
|---|---|---|---|
| `heatmap_before_viewport.png` | debug `rs_cam_gui`, md5 `ac9278bb…` | **`825524f`** (pre-fix — the last commit before A-2's first) | `ToolpathColorMode::Chipload` |
| `heatmap_before_simulation.png` | same | same | same |
| `heatmap_after_viewport.png` | debug `rs_cam_gui`, md5 `27b1fe60…` | **`7aa0be0`** (post-fix — A-2's five commits applied) | `ToolpathColorMode::AdvancePerTooth` |
| `heatmap_after_simulation.png` | same | same | same |

**Not the operator's live GUI.** That instance (PID 3837079, build
`d820226-dirty`) was reachable but its frame loop was **not
dispatching** — `project_summary` answered `served_from: "snapshot"`,
`snapshot_age_s` 142.6, `frame_loop.healthy: false`, so every
GUI-dispatched call (`load_project`, `set_ui_view`, `screenshot_gui`)
was unreachable. That is G-LV.1 re-open (b), the identical blocker A-1
recorded, and it is Lane B's charter, not something A-2 could clear.
The brief's sanctioned fallback was taken: capture **both** halves from
my own instances, one built pre-fix and one post-fix.

That fallback is arguably the better evidence. Both runs use the same
project, the same generation, the same simulation resolution and the
same camera, so the only variable between the two images is the code.

## One patch, applied identically to both builds

Neither MCP nor the CLI can set the viewport colour mode — it is a GUI
combo box (`ui/viewport_overlay.rs`) with no automation surface. So each
scratch build has **one line changed**, uncommitted, in
`state/viewport.rs`: the *default* `toolpath_color_mode` is set to the
heat-map instead of `Normal`. Nothing about the measure, the band, the
classification or the colours is touched. The same patch is on both
builds, so it cannot bias the comparison.

## Fixture

`a2_heatmap_fixture.toml` (session scratch, not committed): a **copy**
of `planning/airrun_2026-06-01/wanaka.toml` with every toolpath
disabled except **"Back Rough"** — `adaptive3d`, 6 mm 2-flute flat
endmill, `stock_source = "fresh"`, hard maple. Chosen because it is the
cheapest enabled op that matches a vendor chipload row and does not
depend on prior simulated stock; the first attempt used "3D Rough 6",
which is `from_remaining_stock` and returned `awaiting_prior_stock`
with nothing generated. The operator's `wanaka.toml` was read only and
never written.

`generate_all {fixpoint, simulation_resolution_mm: 0.4}` →
`generated: 1` in ~58 s; `run_simulation {resolution: 0.4}`; 3355
moves, 3:09 cycle time. Driver: `a2_shot.py` (this directory), which
reuses B-1's dependency-free stdio client from `../b1/`.

## What the pair shows

**`*_viewport.png` — V1, the viewport heat-map.** Same camera, same
path. Before: predominantly **red** — `> cl_max`, "breakage risk" —
over an operation the gate rules **Within**. After: predominantly
**orange** — `[0.9·max, max]`, approaching the ceiling — which is what
`Within` with a peak near the band top actually looks like. The colour
and the verdict now agree because they are one classification of one
quantity.

Note the direction. The synthetic two-arc fixture shows the retired
measure reading **low** against the band (arm A painted blue, "rubbing
risk", at 0.0099 against a 0.032 floor). On this real adaptive3d pass
at near-full-slot engagement it reads **high** instead. Both are the
same defect — a quantity carrying an engagement-arc term compared to a
band that does not — and the fixture's 4.497× must not be quoted as a
wanaka figure.

**`*_simulation.png` — V3 and V6 on one screen.** Before: the timeline
track is labelled **`load vs limit`** and its trace sits **above 1.0**
across nearly the whole path — i.e. "over the limit almost everywhere"
— while the Inspector three lines up reads **`✓ 1 within · ✗ 0
exceeding`** and the badge reads **`chipload 100%`**. After: the track
is **`advance/tooth vs band max`**, its trace sits **at ≈1.0**, and the
badge reads **`advance/tooth 100%`**. The before image is the clearest
single frame of F-HEATMAP in the repository: the normalised
"fraction of limit" and the gate verdict contradict each other on one
screen.

## `screenshot_toolpath` is NOT a heat-map surface

`screenshot_toolpath_is_not_a_heatmap_surface.png` is a
`screenshot_toolpath(index: 1, include_rapids: false)` taken on the
**pre-fix** build with the heat-map colour mode active. It is uniformly
green: that tool renders through a separate offscreen path with a fixed
palette and does not read `toolpath_color_mode` at all.

Recorded because A-1's stated resume condition asked for
`screenshot_toolpath(include_rapids:false)` as half of the pair, and it
cannot serve that purpose. Anyone repeating this capture should use
`set_ui_view {workspace: "toolpaths"}` + `screenshot_gui`, as
`a2_shot.py` does. Not a defect this wave fixes — ledger it or leave
it, but do not ask that tool for a colour-mode answer.
