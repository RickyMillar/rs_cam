# Verification brief: check the feeds engine against shop practice

Paste this file as the brief of the review session. Repository: `/home/ricky/personal_repos/rs_cam`,
hand-off commit `f0150aeb5`.

## 0. Rules for the review session

- You report. You change nothing under `crates/`. You do not edit `planning/feeds_contract_2026-11-03/STATUS.md`.
- You write only in `planning/feeds_review_<date>/` (for example planning/feeds_review_2026-11-07/).
- Never type a number from memory. Each reference number comes from a printed source in the repo,
  or from a source you add to your folder with a citation (title, URL, date, page, the line you read).
- Run every cargo command from the repo root as `timeout 1800 scripts/cargo_lane.sh <args>`.
  Never run a workspace-wide `cargo test`. Ask the operator before any run over three minutes.
- Before you trust a GUI or MCP number, check that the running `rs_cam_gui` was built from HEAD
  (MCP `gui_status`; rebuild, then `gui_restart`).
- Do not measure on a copy of a benchmark project in a scratchpad. Variant projects for the
  matrix go in your review folder.
- Leave alone: `.mcp.json`, `planning/adaptive_research_2026-10-23/`, `.pi/`, untracked `play/*.toml`.

## 1. Read first

1. `planning/feeds_contract_2026-11-03/handoff/INDEX.md` (the reading order).
2. The parameter guide: the explainer page https://claude.ai/artifact/78xXh5LQ2oPoPsErTZZnQy.
   Its source is `planning/feeds_contract_2026-11-03/explainer_p13c/` (`index.html` and
   `assert_claims.py`, which checks every number on the page).
3. `planning/feeds_contract_2026-11-03/handoff/MODELS.md`: every formula, input, source and constant.
4. `planning/feeds_contract_2026-11-03/handoff/WHAT_IS_DONE.md` section 4: the reviewer items. Judge each one.

## 2. The first command (runs as written)

The lead ran these two commands at `f0150aeb5` on 2026-11-06. Both exited 0.

```
timeout 1800 scripts/cargo_lane.sh run -q --release -p rs_cam_cli -- suggest planning/fixtures/rivmap100/rivmap100.toml --json > planning/feeds_contract_2026-11-03/handoff/runs/first_rivmap100_suggest.jsonl
timeout 1800 scripts/cargo_lane.sh run -q --release -p rs_cam_cli -- suggest planning/feeds_contract_2026-11-03/handoff/runs/row_softwood_rivmap100.toml --json > planning/feeds_contract_2026-11-03/handoff/runs/row_softwood_suggest.jsonl
```

- Outputs: `planning/feeds_contract_2026-11-03/handoff/runs/first_rivmap100_suggest.jsonl` and
  `planning/feeds_contract_2026-11-03/handoff/runs/row_softwood_suggest.jsonl` (stderr beside each).
- The second command is one matrix row: rivmap100 with the stock species changed from
  `WhiteOak` to `GenericSoftwood` (and the model paths made relative to the copy). The file is
  `planning/feeds_contract_2026-11-03/handoff/runs/row_softwood_rivmap100.toml`.
- Each line is one toolpath. Keys: `index`, `toolpath_id`, `toolpath_name`, `current`, `run_state`,
  `lineage`, `tool`, `suggested`, `profile`. `profile` holds `calculator` (the `FeedsResult`),
  `shipped_power`, `point_source`, `machine`, `basis`, `envelope` (the MCP `get_safe_envelope`
  JSON) and `rationale`. The command is documented at `crates/rs_cam_cli/src/suggest.rs:1-35`.
- First observation for you to judge (from the two outputs, toolpath "Face 5"): the white oak cell
  reads row `amana-flat-hardwood-pocket-6000-2f-spektra` at chip 0.1311 mm and ships 5550 mm/min;
  the softwood cell reads `amana-flat-softwood-pocket-6000-2f-spektra` at 0.127 mm and ships
  5375 mm/min. Check the printed Spektra rows: does the chart print a larger chip for hardwood?

## 3. The ready-made matrix (read before any run)

`crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv` is the engine's
answer for 1000 cells: every tool type at two diameters, every op, four wood families, on the
default machine (`crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs:1-35`). The sim subset is
`crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23_sim.csv`. Start the
review from these files; they need no run.

## 4. The matrix to run

| Axis | Values |
|---|---|
| Tool kind | flat (`end_mill`), ball (`ball_nose`), bull (`bull_nose`, `corner_radius`), tapered (`tapered_ball_nose`), V (`v_bit`, `included_angle`, `v_tip_diameter`) |
| Flutes | 1, 2, 3 (`flute_count = { stated = N }`) |
| Diameter | 1 to 25 mm; at least 1, 3.175, 6, 6.35, 12, 12.7, 25 |
| Material | soft wood, hard wood (`[job.stock.material.SolidWood] species = "GenericSoftwood"` / `"GenericHardwood"`), MDF (`[job.stock.material.SheetGood] kind = "Mdf"`), plywood (`[job.stock.material.Plywood] grade = "BalticBirch"`) |
| Op | one op per feeds family of the registry (25 rows in `crates/rs_cam_core/src/compute/catalog/registry.rs`): Pocket, Face, Zigzag, Profile, Adaptive, Adaptive3d, Drill, Bore, Trace, Waterline, Scallop, Drop Cutter, Pencil |
| Cut | stepover: slot, 50 %, 20 %, 5 % of D; depth: 0.5, 1, 2, 3 x D |
| Machine | the Shapeoko profile (rivmap100 `[job.machine]`, `planning/fixtures/rivmap100/rivmap100.toml:28-60`) and one rigid profile (below) |

Facts that the TOML forms need:

- Material forms: `crates/rs_cam_core/src/material/mod.rs:543-582` (`SolidWood`, `Plywood`, `SheetGood`);
  species list `crates/rs_cam_core/src/material/mod.rs:83-94`. Spell a grade or a kind as its
  variant name (`crates/rs_cam_core/src/material/mod.rs:177-181`, `crates/rs_cam_core/src/material/mod.rs:210-214`).
- A tool must state `centre_cutting = { stated = "yes" }` (and a V-bit
  `v_tip_diameter = { stated = 0.0 }`, the form of `planning/fixtures/rivmap100/rivmap100.toml:126`), or the
  op stops (D12, `crates/rs_cam_cli/CLAUDE.md:32`). A bull nose reads `corner_radius`
  (`crates/rs_cam_core/src/compute/tool_config.rs:587-588`).
- A plywood material has no force line, so power, deflection and the force budget are Unmodeled
  there (MODELS.md M4). Record it; do not count it as a pass.

How to build the cells: adapt the sweep driver
`planning/feeds_contract_2026-11-03/research_p13_step0/run_cli.py`. It writes one project per cell
from `crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres/project.toml` (one Pocket op) and
runs `suggest --json` on each. It was written at 36fd50c54: check that each string replace still
hits the fixture. Put your copy and its outputs in your review folder.

The rigid profile: no industrial machine file exists in the repo; every preset is hobby
(`crates/rs_cam_core/src/machine/mod.rs:932-1010`). Write one `[job.machine]` table with
`class = "industrial"` and a `[job.machine.rigidity]` block with `adaptive_doc_factor`,
`adaptive_woc_factor`, `slot_doc_factor` and `force_budget_n`. An industrial table without the block
or without `force_budget_n` refuses to load (`crates/rs_cam_core/src/machine/mod.rs:475-500`). Take
each value from a printed machine spec and cite it; else mark every result of that machine Unsourced.

## 5. The sanity-check pass (STATUS item 4e)

For each toolpath kind (Pocket, Face, Profile, Adaptive, Zigzag, Drill, the 3D roughs and the 3D
finishes), on the fixtures (`planning/fixtures/rivmap100/rivmap100.toml`,
`planning/rivmap_block_job/nz_wanaka_21.toml`) and on the matrix cells:

1. Write your own judgement of the safe cut first: depth, width, feed, rpm. Use printed sources
   only (section 6). Save it before you look at the engine.
2. Read the engine:
   - Suggest: CLI `suggest --json` (section 2), or MCP `get_suggest_rationale`.
   - The envelope: the `envelope` key, or MCP `get_safe_envelope`.
   - The lineage of each number: MCP `get_toolpath_lineage`, or the `lineage` key.
   - The post-sim gate: MCP `generate_toolpath`, `run_simulation`, then `get_tool_load_report`.
     The CLI door is `project <toml> --output-dir <dir>`: it writes `summary.json` and one
     `tp_<id>_<name>.json` per toolpath (`crates/rs_cam_cli/src/project.rs:1015`). Check which load
     fields those files hold before you rely on them.
3. Compare. Each disagreement is a finding: the engine is wrong, or the judgement is. Name the
   missing term that would explain it (D11: find the term; never fit a rule per row).

Cases the lead named for this pass:

- 3.175 mm hardwood Pocket picks 2.2 x D deep (WHAT_IS_DONE R37). Judge chip clearing and breakage.
- The hobby budget is set by the MDF reading; hardwood picks deeper than the printed Sienci
  hardwood slot (R36).
- The bull-nose `d_eng` defect below the corner radius (R30).
- Waterline width and thinning (R41); a light finish pass and the Harvey witness (R38).

## 6. References

- Vendor rows: `crates/rs_cam_core/data/vendor_lut/observations/` (stored chart text in
  `crates/rs_cam_core/data/vendor_lut/sources/`, manifest `crates/rs_cam_core/data/vendor_lut/source_manifest.json`).
  Industrial baselines: `crates/rs_cam_core/data/vendor_lut/industrial_only/`.
- Garr chip thinning: `CREDITS.md:363-364`, rule text `CREDITS.md:384-391`.
- The vendor 1/2/3 x D table (Onsrud, Freud, Amana): `CREDITS.md:369-375`.
- Amana, Onsrud, Freud charts: `CREDITS.md:272-279`, `CREDITS.md:346-354`.
- Hobby charts (Sienci, Carbide 3D): `CREDITS.md:937-962`; stored text in
  `planning/extrapolation_2026-09-24/fetch/G10/sources/`.
- Force lines: `CREDITS.md:450-475`.
- You may add a named industry source. Store its text in your folder and cite the line.

## 7. The pass rule (fixed before the run)

Write this rule into your folder before the first run, as P11 step 0 did
(`planning/feeds_contract_2026-11-03/research_p11_step0/PREREGISTRATION.md`).

- Must fall inside a printed band: the shipped chip (the peak chip on a thinning cut) inside the
  vendor row band of the same tool, size and material; the plunge inside the printed plunge rule.
- Must not exceed a printed limit: depth against the vendor depth table at the shipped chip
  (a point past it must carry the warning); rpm inside the machine range.
- May differ, with a stated reason: feed where the machine cap or the spindle power binds; depth
  and width where the force budget or deflection binds (the model, not a chart, sets them);
  any cell on the formula chip (no printed row).
- A cell with no printed reference is "no reference", not a pass.

## 8. Output

In `planning/feeds_review_<date>/`:

- `REPORT.md`: the matrix result, the 4e comparisons, and the verdict on each WHAT_IS_DONE item.
- `index.html`: the explainer page of the results.
- `assert_claims.py`: one assert per claim of the report and the page (operator preference).
- `DISAGREEMENTS.md`: one row per disagreement: cell, engine value, reference value with its
  citation, the missing term it suggests.

## 9. Machine parts (STATUS item 4e)

Order (lead ruling, from STATUS item 4e):
1. Do part A first, before the section 5 pass. It gives the judgement a cited hobby frame value.
2. Do part B when the operator is at the machine. The section 5 pass does not wait for it.
3. Part C is optional.

A. The Sienci stiffness source. Pull Sienci's printed stiffness results into the repo (the
   `sienci.com` "Stiffness Rating with Commentary" PDF and the LongMill MK3 rigidity test) as a cited
   hobby frame ballpark. Store the text and record it in `CREDITS.md` through the lead.

B. The VFD power ladder on the operator's machine (Scylla).
   CAUTION: the operator runs the machine. Do a no-motion test of every macro and streamer first.
   1. Measure an air-cut baseline per rpm.
   2. Cut 6-9 cuts in one wood around the pick (one full slot).
   3. Compare measured power minus baseline with the model's mean-force power (MODELS.md M6).
   - Read the spindle outside the controller's Modbus link: a logging power plug, the VFD analog
     output, or the display. Check the grblHAL H-100 driver source for a current read.
   - A Python streamer on `/dev/ttyACM0` logs a timestamp per cut.

C. Optional: the step 0 dial-gauge stiffness protocol on Scylla
   (`planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md`).

Source of this section: `planning/feeds_contract_2026-11-03/STATUS.md:132-144`.
