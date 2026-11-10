# P27 brief: feeds from fundamentals

Written on 2026-11-11 for a dedicated session. You own this project end to end: work out the
problem, choose the approach, build it, and present the verification. The lead of the earlier
sessions wrote this page to give you the goal, the ground you stand on and the few rules that do
not bend. Everything else is your decision.

## The operator's direction (their words)

> "Instead of trying to do it perfectly. Could we just try and make math that gives us similar
> results to the existing lookup tables everywhere. Solely based on geometry, materials of the tool
> and the stock. If it matches the tables, then I think we can assume it will do a good job finding
> the other tables. [...] Because if we have that, all cuts can go through our own pipeline, based
> off fundamentals, and then we just use the vendor data as validation."

> "I'd like it to work out the problem, and make the implementation and present the verification
> as a large project."

## The outcome

Today one input of the feeds pipeline is looked up: the target chip per tooth comes from a vendor
row. Everything after it is calculated. The operator wants that last input calculated too, from
the tool (geometry and material), the stock and the machine, so that every cut goes through one
pipeline built on fundamentals. The vendor tables then become the evidence that the math is
right, and a warning when the math and a printed row disagree.

Done means:

1. A model exists in the product that gives the chip per tooth (and whatever else you find it
   should give) for any tool, stock and cut the app supports, says which physical limit decides
   the value, and says plainly when it has no basis for an answer.
2. Suggest uses it. The operator can see, for any toolpath, the model's value, the limit that
   binds, and how it compares with the vendor row where one exists.
3. The verification is a project in its own right: a reader who does not trust the model can see
   how well it reproduces the tables it was built on AND tables it never saw, where it fails, and
   why. The operator's standing preference is an interactive HTML page with a script that asserts
   every claim the page makes.
4. The operator can decide from your presentation whether to cut wood with it.

How you get there is open: what the limits are, how many there are, what is fitted and what is
derived, whether the model replaces the row lookup or stands beside it, how the work is staged,
what to build first, what to throw away. If the evidence says the idea does not work as stated
(for example the tables encode the vendors' machines more than the tools), say so, and build the
most correct thing the evidence supports.

## What the result is judged on

- It reproduces the vendor tables to about the spread the vendors show between themselves. The
  app's own data puts that spread at roughly x0.8 to x1.56 for the same tool at twice the row
  size; measure it properly yourself. Matching every number is not the goal.
- It is tested on rows that had no part in building it. Decide the split before you fit, and
  record it.
- Its constants are few and mean something. A fit with a constant per chart is a lookup table.
- It is honest at its edges: it names what it cannot judge instead of extrapolating quietly.
- A cut it recommends is one the rest of the pipeline (power, force budget, deflection, the
  simulation gates) also accepts, on the benchmark projects.

## The ground you stand on (starting points, not instructions)

- The hand-off of the earlier work: `planning/feeds_contract_2026-11-03/NEXT_SESSION.md` (read the
  top sections). The acceptance table for the wanaka project stands at 98 pass, 0 fail
  (`acceptance/`); it grades Suggest against sourced values and will need to follow whatever you
  change.
- The pipeline: `crates/rs_cam_core/src/feeds/` (read its `CLAUDE.md`), the calculator steps under
  `feeds/calculator/`, the constant-chip RPM rule in `calculator/spindle.rs`.
- The force model: `material/force_line.rs` (Curti 2021, density law; the stored paper is
  `planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt`).
  An independent check (Woodson 1979, white oak) and what is known about straight bits:
  `research/large_straight_bit_roughing_2026-11-10.md`, `large_straight_router_bits_2026-11-10.md`.
- The data: 567 vendor rows with their source texts in `crates/rs_cam_core/data/vendor_lut/`. The
  size-law study (P13) and the extrapolation programme (`planning/extrapolation_2026-09-24/`) are
  earlier attempts to find structure in them.
- The tool record: `compute/tool_config.rs`. It holds diameter, flute length and count, shank,
  stick-out, body material, flute form (straight or spiral), centre cutting. It does not hold core
  diameter, rake, relief or edge radius. Whether the model needs them, and where they come from,
  is yours to settle.
- The row lookup is mid-redesign as categories (`phases/P25_DESIGN.md`, operator rulings 1 to 5
  in section 8). Ruling 6, what to do when no row exists, is waiting for your model. You may find
  the category design changes once the model exists; say so if it does.
- First thoughts from the earlier lead, to use or discard: `phases/P27_NOTES.md`.
- Benchmarks: `planning/rivmap_block_job/nz_wanaka_21_operator.toml` (the operator's real job;
  never save over it without their say), `planning/fixtures/` (rivmap100).

## The rules that do not bend

- A number is derived by math, printed in a source stored in the repo, or fitted to stored data
  with the fit shown. Nothing typed from memory. No bench rig exists and nothing has been measured
  by the operator.
- One definition of each thing in the codebase. Extend the force model and the limits that exist;
  do not grow a second copy.
- The app never refuses a job: a finding warns. Suggest may give no number; generation and export
  still work.
- The operator's machine: every cargo command through `scripts/cargo_lane.sh` with `-j 2`; never
  workspace-wide `cargo test`; ask before the core full gate; do not run heavy suites while the
  GUI holds a simulated project (memory). The GUI restarts through the MCP proxy (`gui_restart`);
  check `gui_status` before trusting a GUI number.
- Git: several sessions share this tree; scope every commit to your own paths.
- Prose in the repo follows the operator's plain-English style (short active sentences).
- You may use agents to implement; you decide and verify. Review their work by path.

## What the operator asks of you along the way

Use your judgment and keep moving; you do not need permission for decisions inside the project.
Bring the operator in when a choice changes what they will cut with (a number that ships, a
ruling of theirs that the model would replace), and when you have something worth looking at.
They prefer to see a model working, with its evidence, over a plan.
