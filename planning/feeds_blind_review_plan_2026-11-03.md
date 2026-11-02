# Plan: blind review of feeds, speeds, plunge, stepover and depth (nz-wanaka-21)

Owner: operator. Worker: another account. Repo: `/home/ricky/personal_repos/rs_cam`. Write prose in
ASD-STE100. Commit by path; agents do not commit (the lead session reviews and commits).

## Goal

Independent numbers for every toolpath of the nz-wanaka-21 job, found from first principles and
published data, NOT from rs_cam's Suggest. Then compare with Suggest, flag each disagreement as
safe / risky / dangerous, and publish one report. The operator adjusts values by hand today and
has little intuition for the V-bits and the finishing passes.

## Inputs (read only)

- The project: `planning/rivmap_block_job/nz_wanaka_21_operator.toml` (the operator's current
  values) and `nz_wanaka_21.toml` (the generator's). The generator
  `planning/rivmap_block_job/derive_wanaka21.py` writes a `why` comment above every toolpath:
  what it cuts and why.
- The tools: the `[[tools]]` tables of the project; the operator's bits in
  `~/.config/rs_cam/tools/my_roughing.toml`. Measure-flagged values (stick-outs, the 18 mm centre
  core and protrusion, the DMM 2 mm shank, the R1 taper) are estimates: say so.
- The machine: `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml` (spindle 1.5 kW H100 VFD
  and its power curve, rpm range, max feeds, accelerations, rigidity factors).
- The material: white oak (block.json / project stock material), an epoxy fill in the cavity.
- The job story: `planning/rivmap_block_job.md`, sections from "2026-11-02".

## Phase 1: briefs (one agent, about 30 min)

Write one brief per toolpath (19 ops; group the identical ones) into
`planning/feeds_blind_review_2026-11-03/briefs/<nn>_<name>.md`:

- the tool: type, diameter, flutes, cutting length, stick-out, shank, coating if known, special
  geometry (flat-tip V, tapered ball, non-centre-cutting core);
- the cut: op kind and strategy (adaptive with constant engagement, pocket rings with full-width
  first ring, slot, trace, project curve, scallop, face, drill), radial and axial engagement as
  planned, entry style, what material it meets (fresh oak, rest stock, epoxy, air share);
- the machine limits that bind it (max feed, rpm range, power at the rpm, rigidity);
- the material and what matters for the finish (grain, burning, tear-out, epoxy).

Leave out every number rs_cam suggested and every current feed or speed. A brief describes the
job, not an answer.

## Phase 2: blind reviewers (three agents in parallel, each with its briefs only)

Groups: (A) roughing and slotting: 18 mm faces and bulk roughs, 6 mm rests, the river-channel
slot, pockets, the clamp-pad pocket, pin and light-hole drilling, the outline profile; (B) V-bits
and engraving: 20 deg V 0.3 mm flat tip (river V-grooves by Project Curve), V60 slit, V60 engraving;
(C) finishing and small tools: R1 x 6 x 20 tapered ball scallop and R1 rest, the DMM 2 mm 1-flute
lake and antenna pockets.

Each reviewer:

1. Works from chip load: chip load per tooth for the tool, material and engagement, with chip
   thinning for radial engagement under 50 % and the ball/V effective diameter at the real depth.
   Rule: hold the chip load constant and move rpm and feed together inside the machine's limits
   (rpm range, max feed, power curve, acceleration); show where a limit binds.
2. Finds data and cites it: vendor charts (Amana, Whiteside, Onsrud, DMM, Harvey, Sorotec and the
   like), the repo's sources (`crates/rs_cam_core/tests/literature_matrix/sources.toml`,
   `manufacture/block/feeds_oak.toml` in the monorepo), and handbook formulas. A number without a
   source or a derivation is marked "judgement".
3. Gives options, not one value, where the choice matters: for example depth x stepover pairs
   (deep and narrow against shallow and wide) with the feed for each, and a conservative and a
   productive column. States plunge and ramp rules (non-centre-cutting tools: no plunge deeper than
   the protrusion, ramp at or under atan(protrusion / core)).
4. Writes `planning/feeds_blind_review_2026-11-03/review_<group>.md`: one table per op (field,
   value or range, unit, basis, source, confidence) and the risks it sees.

## Phase 3: compare with rs_cam (one agent, after phase 2)

- For each op, read rs_cam's Suggest WITHOUT changing the project: MCP `get_suggest_rationale`
  (index) on a GUI that has the project loaded (no generation, no simulation needed), and the
  operator's current values from the project file.
- Classify each field: agree (within the reviewer's range), Suggest high, Suggest low; and the risk
  of the shipped value: safe, risky (tool life, finish, burning), dangerous (breakage, stall,
  plunge with a non-centre-cutting tool, power over the curve).
- Where a reviewer value is new, check it against rs_cam's gates on one op at a time
  (`apply_feeds` is NOT used; set the value with `set_toolpath_param` on a scratch copy of the
  project, generate that op, read `get_toolpath_diagnostics`). Do not run the full job; ask the
  operator before any full simulation (it needs about 27 GB).

## Phase 4: report

An HTML page (operator preference: an interactive page plus a script that asserts each claim the
page makes) in `planning/feeds_blind_review_2026-11-03/`: per op the brief summary, the reviewer
options with sources, Suggest and the operator value, the classification, and a recommended value.
A short list on top: the dangerous items first. The script recomputes every chip load and every
limit check on the page from the tables.

## Optional tool (makes phase 3 GUI-free)

A CLI command `rs_cam suggest <project.toml> [--json]` that prints `feeds::suggest_for_operation`
and its rationale for every enabled toolpath without generation. Core has the function
(`crates/rs_cam_core/src/feeds/suggest.rs`); the MCP tool `get_suggest_rationale`
(`crates/rs_cam_viz/src/mcp_server.rs`) shows the output shape. Small; build only if the GUI path is
in the way.

## Known questions to put to the reviewers (as questions, not answers)

- The 18 mm 2-flute bulk bit: is a feed at the machine cap (about 6000 mm/min) right for 9 mm deep,
  3.6 mm wide in oak at the available power? What plunge or ramp for a bit that does not cut at the
  centre?
- The river channel: a 6 mm full-width slot up to about 10 mm deep in oak in one pass.
- The V-bits: chip load at the real cutting diameter (a 0.3 mm flat tip, depths 2-5 mm).
- The R1 tapered ball as a rest roughing tool: is it sane, and at what load?
- The DMM 2 mm single flute at 20 mm stick-out in oak.
