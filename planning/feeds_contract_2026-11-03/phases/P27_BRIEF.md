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
- The cut object (P29, in work from 2026-11-11): `phases/P29_FINDINGS.md` section 4. A limit is
  a function of a `Cut` (engagement, operating point, kind), the tool, the material and the
  machine. It returns a `Check`. It never reads the op type. Each op declares its cuts through
  `OperationConfig::cuts()`. Add a new limit or a new model there, not as a per-op rule. Read
  the P29 change list for what has landed; your product wiring comes after P29-3.
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

## From P29 (2026-11-14)

P29 ended with the guard `no_limit_reads_the_op_type_p29_last` and the lead rulings of
`P29-11_DESIGN.md` section 13. Its end state is section 12 of that note. This section is what P29
hands on. The source of each row is the row of `limits::EXEMPTIONS`
(`crates/rs_cam_core/src/limits/exemptions.rs`); the sentry
`the_exemption_list_only_shrinks_p29_3` pins 37 rows: 8 physical, 3 ruled, 26 that need a ruling.

### The open rows by owner

The 11 physical and ruled rows stay (`P28-04`, `P28-07`, `P28-17`, `P28-20`, `P28-24`, `P28-26`,
`P28-30p`, `M9`; `P28-02`, `P28-18`, `S-NoBound`). The 26 rows that need a ruling:

| Owner | Count | Rows | Why |
|---|---|---|---|
| P27 | 7 | `P28-05`, `P28-06`, `P28-08`, `P28-13`, `P28-14`, `F-OffsetRing` | No printed width rule |
| | | `P28-10` | Drill power as a limit: no end-cut force source |
| P25 / P27 | 10 | `P28-09`, `P28-19`, `P28-25`, `P28-27`, `P28-28`, `P28-31`, `P28-33`, `M6`, `M10`, `F-ThinsChip` | A model or a source does not exist: move model, ball axial thinning, vendor axial bound, tip plunge cap, plunge deflection, default cut, V-bit tip, vendor row by op kind, planner floor, finish thinning |
| The modulation design step | 5 | `P28-16`, `P28-21`, `P28-29`, `P28-30` | The feed modulator's own caps and finding |
| | | `S-NoSampleGate` | No sample gate for the vendor depth table, the rubbing floor and the plunge rate; option D-1 of `P29-11_DESIGN.md` 11.6.4 first |
| A generator change | 2 | `U-PerimeterSweep`, `U-EntryFallback` | Generation facts: the geodesic grow refuses; a helix with no room ramps |
| The clean-up pass | 1 | `U-efficiency` | The specific energy and the deflection chip ceiling of the efficiency card read the nominal tool |
| The operator | 1 | `M4` | CLOSED 2026-11-15 (`4fd1af1e1`, M4-A, operator ruling "Commit it"; `P34_M4A_2026-11-15.md`). Was: two sample depths; M4-A or M4-A1 (`P29-11_DESIGN.md` section 3.5) |

A new limit or model goes into `OperationConfig::cuts()` as a fact of the cut and judges every
cut group, before and after the simulation (`limits/CLAUDE.md`, "How to add a limit"). The guard
fails on a read of the op type, the family or the pass role in a limit file.

### F-OF1: one obstacle frame on every face

`P29-8e_OBSTACLE_FRAME.md` section 4. The value frame of an obstacle is the emission frame, and
the emission frame moves with the face: the world frame on a Top / 0 degree setup, the stock
corner on every other face. The editor numbers of setup 1 match neither the G-code nor the screen.
One rule (the stock-corner frame on every face, as the alignment pins use, `StockRelXy`) removes
the trap. It is a lead decision outside P29. The corrected boxes of setup "3 Front" of the
operator file (section 5 of that note) wait for the operator.

### The fold ramp for a surface-riding clip re-entry

`P29-8e_TAB_REPLUNGE.md`, section 11 (addendum), ruling Q10 and its log. A clip re-entry of a
surface-riding op stays the straight plunge: with a surface probe, `emit_ramp` walks straight
legs and clips them in Z only, so it does not fold. A fold under the probe's Z clip is a new
emitter path with no measurement. Open.

### For the clean-up pass

- L-4 (`P29-11_DESIGN.md` section 1 items 16 and 17, section 10 ruling L4): the old records
  (`CriterionStatus`, `Placement`, `JudgedValue`, `criteria()`) become projections with no
  judgement of their own, and a record with no reader goes; each diagnostic and load-policy
  message has one writer (today `diagnostics/adapters/from_tool_load.rs` and the `gcode`
  advisories format their own numbers).
- The triage classifier (`P29-11_DESIGN.md` section 2, ruling L4): `stock/sim_triage.rs` has its
  own intent predicates beside `CutKind::of_sample`; one classifier.
- The three MRR formulas in two units (section 2): `ui/feeds/compare.rs` (ap x ae x f),
  `calculator/close.rs` (section x feed), the sample `mrr_mm3_s` (mm³/s); one MRR function of
  `CutLoad`. A display number moves on a V-bit and a ball.
- The duplicate static checks (section 2): the vendor-table and cutting-length checks of
  `diagnostics/adapters/from_static_checks.rs` duplicate the `VendorDepthTable` and
  `CuttingLength` rows.
- One chord law and a cusp display for every finisher (section 1 item 13; the clean-up pass and
  the UI work).
- `Capabilities::flat_floor_clearing` leaves out 2D Adaptive (section 1 item 15).
- The new-op Suggest context has no stock slot (section 1 item 10).
- `U-efficiency` (section 13, ruling 2): the card reads the engagement of the kept cut
  (`applied_cut` keeps a stated depth with the recommendation's engaged diameter) and the
  deflection limit of the cut.
- A constructor for `Diagnostic`, so that a new field does not touch about 100 struct literals
  (`P29-10_DESIGN.md` section 14).
- `geometry/boundary.rs` imports `feeds::entry_rule`; `apply_dressups_with_entry` is a second door
  into the dressup pipeline (`P29-8e_TAB_REPLUNGE.md` section 12).
- Logged, not traced (`NEXT_SESSION.md`, the row "P29-10 GUI"): an MCP `set_tool_param` is
  written back by the open tool panel; a second `run_simulation` on the operator file OOM-killed
  the GUI at 15 GB; the S7 Warnings list is flooded by 578 holder findings; the S9 idx 16 force
  card reads "Width 1 to 154 % of D"; a black band under the timeline after a window resize; the
  IC1 counter counts rotated axis titles as lines; `set_ui_view` cannot select a tool or resize
  the inspector; the card depths of idx 14 and 15 (5.73, 6.41 mm) differ from the depth peak of
  the load report (2.99, 3.27 mm).
