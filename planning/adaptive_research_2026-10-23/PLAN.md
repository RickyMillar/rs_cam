# Adaptive roughing and spiral research — plan (2026-10-23)

**Status:** plan only. No research or code yet. A Fable session executes it
(`PROMPT_NEXT.md`).

**End goal:** a 3D Rough whose path makes sense to the eye, and a decision
on a spiral finish for the many-island case.

## Operator rulings (2026-10-23)

- The gate is the LOOK, not a statistic. The operator asks three
  questions per fixture:
  1. Do the links make sense?
  2. Does the path make sense?
  3. Does the finished part look good?
- Earlier campaigns over-optimised efficiency, load and air numbers into
  statistical edge cases. Do not rank candidates by an aggregate score.
- Do not measure against Fusion and do not use Fusion output. A target
  that copies one implementation is hard to replicate.
- Real, complex fixtures are the gold standard. Synthetic shapes are for
  development only.
- Baselines: `ContourParallel` (the default) and `AgentSearch` for the
  rough; iso-scallop (`iso_field`) for the finish.
- The geodesic-wavefront idea is a lead, not a decision.

## Hard correctness bars (not scores)

Every candidate must pass these before the operator sees it. They are
pass/fail only. Do not tune toward them.

- 0 rapid collisions and 0 holder collisions.
- Coverage audit passes (no uncut island bigger than the stock to leave).
- No gouge: emitted G-code against the mesh, not the planner's Z ladder.
- Generation completes on every fixture with no refusal.

## Evidence from the earlier work (read before you start)

- `git show planning-pre-purge-2026-09-17:planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`
  — review F1–F6, staging, and the June spiral results.
- `research/02_algorithms.md` §5, `research/04_open_source_reference.md` §2.
- `crates/rs_cam_core/src/adaptive/CLAUDE.md` and `adaptive3d/CLAUDE.md`
  — current invariants (pass load, keep-down links, planner stock).
- `planning/spiral_finish_2026-09-01/` — compact spiral track (bar met).
- `planning/conformal_finish_2026-08-28/` — conformal slit map (shelved).
- `planning/multitool_2026-08-23/` — multitool split on wanaka200.

### Q0 — why is the June spiral not the default?

In June, `ContourSpiral` at trochoid cap 1.6 beat the agent on wanaka200
for time, travel and air. `ContourParallel` is still the default. The
memory notes record what the operator saw on screen:

- **V1 corner mop loops.** The round spiral cannot reach square corners.
  The residue mop cuts each corner with small concentric loops. Each loop
  is a separate plunge, cut and retract (`OFFSET_OVERLAP=0.85`,
  `contour_parallel_segments`). Stay-down links are correctly rejected
  because uncut residue separates the loops.
- **V2 "full circles, half air".** These were the corner-blend G2/G3 arcs
  (`blend_corners_to_moves`), not trochoid loops.
- **V3 does not grow into spaces.** Wraps follow the walls. Side lobes and
  branches fill late or through the mop.

V1–V3 are the first named defects. Each candidate page shows whether it
fixes them.

### The gap the June review left open

Review §3 item 1 recommended a spiral that MORPHS between consecutive
wavefronts (Held–Spielberger). Stage 1 shipped plain EDT iso-contour wraps
joined by seams. The EDT measures distance to the region boundary, so its
level sets follow the walls and split at the medial axis. No attempt in
the repo morphs a spiral or grows wraps from the cleared area as a whole.

## The codebase changed after the old reviews

The June review and the August/September spiral work describe a codebase
that no longer exists. Between 2026-06-15 and 2026-10-23 the repository
took 2 720 commits (1 677 files changed under `crates/`). The programmes
that changed structure:

- architecture consolidation (2026-09-09..13) and duplicate sweep
  (2026-09-16);
- structure programme 3 (2026-09-17): the core root split into folders;
  the planning purge deleted the old review files;
- op architecture (2026-10-14..16): shape runners, one territory
  resolver, `#[derive(OpParams)]` as the schema source;
- tech-debt review waves (2026-10-19..22): typed ids, enums for dressups,
  dead code deleted, lint rule W16;
- adaptive-specific (2026-09-24..10-15): step ladder removed, stock-aware
  entries, `ramp_feed_rate`, By Area, pass-load and link-load rules,
  planner stamps, every inset piece planned.

Rules that follow:

1. Treat every `file:line` in an old document as stale. Find the symbol
   again with `codebase_search` / `rg` before you cite it. Example:
   `blend_corners_to_moves` is now in `ops/adaptive_shared.rs`, and
   `apply_adaptive_feed_modulation` is in
   `session/compute/simulation.rs`.
2. Treat each old finding (review F1–F6, V1–V3) as a hypothesis. Phase 0
   confirms on HEAD whether it still holds.
3. Read the current folder `CLAUDE.md` files for the invariants. They
   override the old reviews.
4. Operator ruling 2026-10-23: a default with no derivation is removed.
   A new candidate parameter needs a derivation (math or a source in the
   repo), or it stays empty and the op refuses. The tuned
   `TROCHOID_CAP_MULT_3D = 1.6` is such a value; check its state.

## Phase 0 — re-baseline on HEAD

The 2D kernel and the link rules changed after every June number
(2026-09-25..27: G-ADAPTPASSLOAD, G-ADAPTLINKLOAD, G-PLANSIMGAP,
G-PHANTOMSTAMP, entry stock awareness). Do not cite June numbers or June
images.

1. Build the release GUI from HEAD. Confirm the running binary is HEAD
   (`gui_status`) before you trust one image.
2. Run `ContourParallel`, `AgentSearch` and `ContourSpiral` on each
   fixture below. Make the fixture page (protocol below) for each.
3. Mark V1–V3 and any new visual defect on the images.
4. The operator reviews the baseline pages before Phase 2 code starts.

## Fixtures

| Fixture | Project file | Model | Why |
|---|---|---|---|
| rivmap100 | `planning/fixtures/rivmap100/rivmap100.toml` | `planning/fixtures/rivmap100/rivmap_export/terrain.stl` | roughing benchmark, terrain with banks |
| wanaka200 | `planning/multitool_2026-08-23/wanaka200_mt1.toml` (and `_iso_scallop`) | `/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl` (machine-local) | large terrain, holes, many islands |
| Corne case | `planning/corne_case_analysis_2026-09-18/corne_job_corrected.toml` | `/home/ricky/Downloads/Corne Case Hex.stl` (machine-local) | prismatic part: walls, square corners, pockets |
| 2D star / pocket | `test_data/ux_2d_star.toml`, `test_data/ux_2d_pocket.toml` | in-file | development only: lobes, corners |

Work on the fixtures in place. Do not measure on a copy in a scratchpad.
Before a session writes a project file, it saves to a new file name.

## Look-review protocol (the gate)

One HTML page per fixture. Each strategy is one column. Each column has:

1. cutting moves only (`screenshot_toolpath include_rapids:false`);
2. rapids and links only, so the link logic is visible;
3. entries marked (helix, ramp, plunge);
4. the final stock after simulation (`screenshot_simulation`);
5. for a finish: the finished surface at a raking light.

Show the hard bars as a pass/fail row under each column. Show cycle time
as information only, below the images. Do not show a ranking.

Standing rules: never gate on an aggregate without the surface render;
measure emitted motion, not the plan; the GUI must be built from HEAD.

## Phase 1 — reading (questions, not summaries)

Each read writes one note with answers to its questions and the source
line for each answer. Use ideas only. Do not copy GPL code into rs_cam
(MIT OR Apache-2.0). Credit each used source in `CREDITS.md`.

### R1 — FreeCAD Adaptive (`Adaptive.cpp`, CAM module libarea)

1. Who wrote it, and when? Read the file header. Do not assume the
   Freesteel lineage that `research/02_algorithms.md` states.
2. How does it grow a pass? From the cleared area, from the boundary, or
   step by step?
3. How does it measure engagement, and against what target?
4. How does it decide a link: keep down, lift, or retract? What does it
   test along the link?
5. What does it do in a concave corner and in a slot narrower than 2R?
6. What residue does it leave, and how does it clean it up?
7. Which of its failure modes match V1–V3?

### R2 — Freesteel blog (the authors of the adaptive that Fusion uses)

1. Find the source first. Try the direct address (freesteel.co.uk), then
   the Wayback Machine. A web search on 2026-10-23 found nothing.
2. If it is not found, record that and continue. No phase depends on R2.
3. If it is found: what did they try that failed? How do passes grow? How
   do they link? How do they handle corners and the end of a region?

### R3 — papers for the candidate fields

1. Held & Spielberger, smooth spiral for 2D pockets (CAD 2009/2011): the
   morph between offsets and the island case.
2. Bieterman & Sandström 2003: the PDE (Laplace) field from boundary to
   seed point.
3. One source on geodesic distance or fast marching inside a polygon
   region, for candidate (a).
4. libactp (`Heeks/libactp-old`): how it relates to R1 and R2.

Each paper note states what the method needs that rs_cam does not have.

## Phase 2 — candidate fields for the rough (2D first)

Three candidates, side by side. Each gives a scalar field over one slice
region. Wraps are its level sets, joined into one continuous path.

| Id | Field | Expected strength | Expected risk |
|---|---|---|---|
| (a) | geodesic distance from the entry (wavefront from the cleared area) | grows into every lobe; matches "grow into spaces" | wraps can be uneven where the front meets itself |
| (b) | Bieterman–Sandström PDE morph, boundary to seed | smooth; one spiral by construction | islands and narrow necks; needs a solver |
| (c) | Held medial-axis spiral | proven for pockets with islands | medial axis is noisy on terrain slices |

Steps:

1. Build each candidate in a research module behind a flag. Reuse the
   existing EDT, marching squares, pass-load measure and link rules.
2. Render all three plus the baselines on `ux_2d_star` and
   `ux_2d_pocket`. Operator picks by look.
3. Run the chosen field (or two) on the 3D slice set of rivmap100, then
   wanaka200 and the Corne case. Make the fixture pages.
4. Check V1–V3 on every page.

Engagement stays bounded by the existing pass-load rule. Spacing comes
from the field, not from a feedback search.

## Phase 3 — links and entries (in every page, not a separate study)

Entries took 36–40 % of rough time on rivmap100 (2026-09-25). "Do the
links make sense" is a gate question. For each candidate, the page shows:

- each retract and the reason for it (uncut residue, height, distance);
- each entry type and where it is;
- the order of regions and levels.

## Phase 4 — spiral finish for the many-island case

"Many small islands" here means the small-tool regions of the multitool
split: many separate compact patches.

The conformal slit map stays shelved: it measured 3.4× slower than raster
on its friendliest region (`conformal_finish_2026-08-28/FINDINGS_F2.md`).
Do not reopen it.

Use the compact spiral first. It costs no new code:
`crates/rs_cam_core/src/finish/spiral_finish_compact.rs` met its bar on
the sphere and the dish (1.113× and 1.223× of the floor, 0 retracts). Its
next gate, the operator appearance review, never happened
(`planning/finishing_status_2026-09-01.md:418`).

1. Run the module on the wanaka200 multitool small-tool regions. Count
   the refusals; the module refuses non-compact shapes.
2. If most regions refuse, record it and stop this arm.
3. If not, make the fixture page: compact spiral beside iso-scallop on
   the same regions. Operator reviews the finished surface.

## Out of scope

- Fusion output, Fusion comparisons, decompiling any CAM binary.
- Aggregate scores, parameter sweeps for a statistic, new optimiser work.
- Trochoid loop variants (engaged-arc tried and rejected 2026-06-23).

## Decisions the operator makes

| Gate | Input | Decision |
|---|---|---|
| G0 | Phase 0 baseline pages | confirm V1–V3, add defects |
| G1 | 2D candidate renders | which field goes to 3D |
| G2 | 3D fixture pages | promote, iterate or drop |
| G3 | compact spiral refusal count, then pages | productise or stop |
