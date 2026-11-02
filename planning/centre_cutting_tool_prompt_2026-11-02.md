# Prompt: non-centre-cutting tools (G-CENTRECUT)

Paste the text below into an agent session in `/home/ricky/personal_repos/rs_cam`.

---

You work in the rs_cam repository. Read `CLAUDE.md` and the folder `CLAUDE.md` of every folder you
change. Write prose in ASD-STE100. Run every cargo command through `scripts/cargo_lane.sh`. Do not
run the core full gate or the whole viz crate; run the folder sentries, the focused crate tests and
clippy. Commit by path (the checkout is shared). Do not push. No legacy shapes: a breaking change is
allowed; say so in the commit.

## Working beside another session

Another session works in the same checkout at the same time (job files under
`planning/rivmap_block_job*`; it drives the GUI through MCP). Do not use a worktree (a worktree
target directory fills the disk; a shared target with another commit breaks the derive crate).

- Change only the files this task needs; commit by path; format only your own files.
- Do not build `target/release/rs_cam_gui` until the end: the other session's GUI runs that binary
  and a restart would load a half-done build. Before the release build, tell the operator.
- Do not run a full nz-wanaka-21 simulation while the operator says the other session simulates
  (two GUIs at 0.2 mm do not fit in memory together).
- For the nz-wanaka-21 check, copy the project to your scratchpad (its model paths are absolute,
  so the copy loads as it is) and never save over `planning/rivmap_block_job/nz_wanaka_21.toml`.

## The problem

Some of the operator's tools do not cut at the centre. Example: the 18 mm 2-flute bulk bit in
`~/.config/rs_cam/tools/my_roughing.toml` ("18 mm 2F") has a gap of about 8 mm between the blades at
the centre, and the blades stand about 1 mm below that centre face (operator values, "around":
treat them as inputs the operator measures). The tool moves in X/Y without a problem. It must never
go down into wood faster than its blades can clear the core:

- a straight plunge into stock: at most the blade protrusion (1 mm);
- any descent into stock (ramp, helix, or a cutting move that descends along a 3D surface): the
  core must not reach uncut wood. A point that enters the core stays under it while the tool moves
  one core diameter, so the descent over that distance must be at most the protrusion:
  max angle = atan(protrusion / core diameter) = atan(1 / 8) = 7.13 deg for this bit.

rs_cam has no property for this. `ToolConfig` has no centre-cutting field, so every op treats every
end mill as centre cutting.

## Evidence (job nz-wanaka-21, 2026-11-02)

Project: `planning/rivmap_block_job/nz_wanaka_21.toml` (models in
`/home/ricky/cnc_jobs/rivmap_block/nz_wanaka_21/`). Op "Back bulk rough 18 mm" (3D Rough,
`entry_style = "ramp"`, `ramp_angle_deg = 3.0`):

- The ramp entries are at 3 deg (safe). But the emitted program has 89 straight `G1` plunges of more
  than 3 mm at the plunge feed, for example `G1 X212.666 Y59.050 Z6.312 F2162` from Z 26.0 (program
  frame, back setup, stock top Z 25.5). The planner labels these "plunge entry": it plunges straight
  where it thinks the column is open.
- The simulation shows wood there: the plunge-class samples of these entry spans reach radial
  engagement 1.0, and single entries remove 1056-2788 mm3 (`get_cut_trace` toolpath_id 1,
  span_kind "entry").
- "Front bulk rough 18 mm": 175 descents of more than 1 mm steeper than 7.13 deg, up to 12 mm.
- `project.entry_load` does not catch this on a bulk pass: its threshold is 2x the pass's own median
  bite, which is large for a deep bulk level.

## What to build

1. The tool model: an optional non-centre-cutting description on the tool (for example
   `centre_core_diameter_mm` and `blade_protrusion_mm`; empty = centre cutting, as now). It goes
   through the TOML tool library, the project file, the GUI tool editor, MCP (`add_tool`,
   `set_tool_param`) and the CLI. Derive the limits in one place (max plunge = protrusion; max
   descent angle = atan(protrusion / core)); no other copy of the formula.
2. The planners never break the limit inside stock:
   - 3D Rough (`adaptive3d`): the stock-aware entries (no straight plunge deeper than the protrusion
     into stock: ramp or helix at most the limit angle); the cut and link paths
     (`adaptive3d/path.rs`, `emitted_cut_path` / `emitted_link_path`, and the drape / chord lift)
     must not descend into stock steeper than the limit.
   - The 2D ops that enter stock (Pocket, Face, 2D Adaptive, Profile) and the entry dressups
     (`dressup/`, `DRESSUP_RAMP_ANGLE_DEG`, `feeds/ramp.rs`): clamp the ramp angle and refuse a
     straight plunge into stock.
   - An op that cannot meet the limit refuses with a clear message; it does not cut silently.
3. A simulation gate: a finding (blocking, so export refuses) for any sample where a
   non-centre-cutting tool descends into stock steeper than its limit or plunges deeper than the
   protrusion. It must work on a bulk pass (absolute limit, not relative to the median bite).
4. Sentries: a test that is red before the fix (a 3D Rough on a synthetic pocket that today emits a
   straight plunge into stock with such a tool), and a gate test. Inject the defect once and see the
   test go red before you trust it.

## Done when

- The nz-wanaka-21 back and front 18 mm bulk ops, with the 18 mm bit set to core 8 / protrusion 1,
  regenerate with no straight plunge into stock deeper than 1 mm and no descent into stock steeper
  than 7.13 deg (check the emitted G-code and the new gate), and still simulate with 0 collisions.
- The folder sentries, `cargo test -p rs_cam_viz --lib`, `cargo test -p rs_cam_mcp`, and the full
  clippy line are green.
- Update `FEATURE_CATALOG.md` and the folder `CLAUDE.md` files you change.

---

## Status 2026-11-02 (night) and the next prompt

Part 1 is committed (aa048853c): `ToolConfig.non_centre_cutting` (`NonCentreCuttingGeometry`, the
one formula `NonCentreCuttingGeometry::limits`) through the TOML library, the project file, MCP, the
CLI and the GUI tool panel. Nothing reads the limits yet. A first part-2 attempt stopped before it
changed code.

Paste this for part 2:

> Continue G-CENTRECUT in `/home/ricky/personal_repos/rs_cam` (read
> `planning/centre_cutting_tool_prompt_2026-11-02.md` first, then the folder `CLAUDE.md` files you
> touch). Part 1 (the tool model) is committed in aa048853c. Build part 2: enforcement.
>
> Operator rule: the TOOL owns the limit and every toolpath accepts it from the tool. No op, planner
> or dressup gets its own formula or its own copy of the check. Use the existing shared patterns:
> the entry builders (`dressup/`, G10 `helix_radius_for` / `entry_feed_rate`, `feeds/ramp.rs`), the
> one session stage after generation (`JobPhases::run_stages`, beside `clip_and_split_entries`), and
> the shared simulation decisions in `compute/simulate.rs` (put a new one there, not in one builder).
>
> 1. Generation: every entry that a shared builder makes (ramp, helix, the 3D Rough stock-aware
>    entries) reads `ToolConfig::non_centre_cutting_limits()` and keeps the descent into stock at or
>    under the limit angle and a straight plunge into stock at or under the protrusion. A planner
>    that cannot meet it refuses with a named `OperationError`; it does not cut silently.
> 2. One gate: a simulation finding (blocking: export refuses) for any sample where a
>    non-centre-cutting tool goes down into stock steeper than its limit or plunges deeper than the
>    protrusion. Absolute limits, not relative to the median bite (the `project.entry_load` gate
>    misses a deep bulk pass for that reason). Descents in air are allowed.
> 3. Sentries: a 3D Rough on a synthetic pocket that today emits a straight plunge into stock with
>    such a tool (red before the fix; inject the defect once), and a gate test.
>
> Done when the nz-wanaka-21 18 mm bulk ops (set the 18 mm bit to core 8, protrusion 1 in a COPY of
> `planning/rivmap_block_job/nz_wanaka_21.toml`) emit no plunge into stock deeper than 1 mm and no
> descent into stock steeper than 7.13 deg, with 0 collisions; folder sentries, `rs_cam_viz --lib`,
> `rs_cam_mcp` and the full clippy line green. Do not build the release GUI until the end (another
> session runs it). Commit by path; agents do not commit if the lead says so.

Two more generic features for later (operator, 2026-11-02):

- Rest region filter on 3D Rough (remaining stock): cut only regions up to a given area, or only
  where the remaining material is up to a given thickness. Then a small finish ball can rest only
  the valleys a larger rough could not reach (a whole-relief rest measured 4266 m on nz-wanaka-21).
- Project Curve depth per pass (and the open-path offset): needed for a 6 mm trace along the rivers
  that replaces the adaptive in the back hollow channels.
