# Pre-registration: feeds sanity-check review (STATUS item 4e, brief section 5)

Written 2026-11-06, before the first engine run of this review. HEAD `ec5cbe4a9`
(the hand-off commit is `f0150aeb5`; P15 `ec5cbe4a9` landed after it).

## Disclosure: engine numbers seen before this file

The reviewer saw these engine numbers before the judgement was written:

1. The brief, section 2: rivmap100 "Face 5" reads row
   `amana-flat-hardwood-pocket-6000-2f-spektra` at chip 0.1311 mm and ships 5550 mm/min;
   the softwood copy reads `amana-flat-softwood-pocket-6000-2f-spektra` at 0.127 mm and
   ships 5375 mm/min.
2. The first two data rows of `matrix_2026-09-23.csv` (EndMill 3.175 mm 2F Face, softwood
   and hardwood). The hardwood row reads `freud-solid-carbide-eighth-hardwood`, depth
   6.985 mm, width 2.321 mm, chip 0.0889 mm.
3. The numbers that `WHAT_IS_DONE.md` section 4 quotes (R30-R67).

No other engine output was opened before `JUDGEMENT.md` was saved.

## The pass rule (brief section 7, fixed now)

- **Must fall inside a printed band.** The shipped chip (the peak chip on a thinning cut)
  is inside the vendor row band of the same tool, size and material. The plunge is inside
  the printed plunge rule.
- **Must not exceed a printed limit.** The depth is not past the vendor depth table at the
  shipped chip, or the point carries the warning. The rpm is inside the machine range.
- **May differ, with a stated reason.** The feed may differ where the machine feed cap or
  the spindle power binds. The depth and the width may differ where the force budget or
  the deflection binds (the model sets them, not a chart). Any cell on the formula chip
  (no printed row) may differ.
- **No reference.** A cell with no printed reference is "no reference", not a pass.

## Two reference layers (added by the reviewer, fixed now)

The pass rule compares the engine with the vendor row of the same tool. The reviewer also
compares the engine with the printed **hobby** charts (Sienci metric, Carbide 3D S3),
because both fixtures run a hobby machine (Shapeoko Pro XXL). A hobby-chart disagreement is
a finding, not a fail, when the vendor layer passes. The verdict of each cell states the
layer.

- Layer V (vendor): Amana, Onsrud, Freud, Harvey rows. The pass rule above applies.
- Layer H (hobby): Sienci metric chart and Carbide 3D S3 chart. They print a depth, a width
  (Sienci stepover) and a feed at one rpm. A pick deeper than the hobby depth at a
  similar or larger width and chip is a finding.

## Cells (fixed now)

1. The two fixtures, every enabled toolpath: `planning/fixtures/rivmap100/rivmap100.toml`
   (4 toolpaths) and `planning/rivmap_block_job/nz_wanaka_21.toml` (18 toolpaths). Both are
   white oak on a Shapeoko-class hobby table. CLI `suggest --json`, unmodified files.
2. One species swap per fixture where the brief names it (rivmap100 softwood, from
   `planning/feeds_contract_2026-11-03/handoff/runs/row_softwood_rivmap100.toml`).
3. Printed-row cells: one cell per (tool kind, diameter, flutes, material) tuple that has a
   printed row with stored chart text, on the Shapeoko table of rivmap100. Op: Pocket
   (roughing) at the engine's own proposal, plus a stated full slot at the printed hobby
   depth.
4. Probe cells off the printed rows: flutes 1 and 3; diameters 1, 12, 12.7, 25 mm; widths
   slot, 0.5 D, 0.2 D, 0.05 D; depths 0.5, 1, 2, 3 D. These test the size law and the
   thinning lift; they are "no reference" unless a printed row covers them.
5. FM1 (`crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`,
   1000 cells, generic machine) is read as the engine's answer at the generic machine; no
   rerun.
6. The rigid machine: no printed spec in the repo gives `force_budget_n` or the three
   rigidity factors. Every result on that table is marked Unsourced.

## What closes a finding

A finding names the missing term (D11). A finding is never closed by a per-row rule.
