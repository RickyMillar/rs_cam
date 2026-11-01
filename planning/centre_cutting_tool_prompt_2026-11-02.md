# Prompt: non-centre-cutting tools (G-CENTRECUT)

Paste the text below into an agent session in `/home/ricky/personal_repos/rs_cam`.

---

You work in the rs_cam repository. Read `CLAUDE.md` and the folder `CLAUDE.md` of every folder you
change. Write prose in ASD-STE100. Run every cargo command through `scripts/cargo_lane.sh`. Do not
run the core full gate or the whole viz crate; run the folder sentries, the focused crate tests and
clippy. Commit by path (the checkout is shared). Do not push. No legacy shapes: a breaking change is
allowed; say so in the commit.

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
