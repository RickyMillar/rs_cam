# P33: a depth per pass inside Project Curve (2026-11-15)

Operator ruling 2026-11-15 (`P31_OPERATOR_RULINGS_2026-11-15.md` section E, ruling 3): "For
multiple passes that should be built into the toolpath, not multiple tool paths!". The Project Curve
op gets a depth per pass.

This file holds the rules and the predictions first (sections 1 to 4, written before the code),
then the results (section 5 on).

## 1. The pass convention, from history

- `git log -p -S'pass 1 of 7' -- planning/rivmap_block_job/` gives no commit, on every ref. No
  stored file holds a pass 1 to 6. The operator file holds only "River channel 6 mm, pass 7 of 7"
  (`depth = -4.0`, from above, cutter projection, `top_z` manual 16.5 = the cavity floor).
- The convention is the loop of `planning/rivmap_block_job/derive_wanaka21.py` lines 646 to 664
  (commit `a73803f70`): `for k in range(n_ch - 1, -1, -1)`, pass `n_ch - k` of `n_ch` has
  `depth = -(web + k * ch_step)`.
  - Each pass is the SAME projected curve, `k` steps away from the full depth. It is not a flat level.
  - A pass covers every curve of the layer. The deepest pass (`k = 0`) is the last pass.
  - The count comes from the deepest channel below the op top Z
    (`channel_depth = cav_floor - (lowest sheet Z + web)`), and `heights.top_z = cav_floor`.
  - The script comment: "where a level is above the cavity floor it cuts air".

## 2. The rules

1. New key `depth_per_pass` on Project Curve, optional. No key: one pass, the path of today,
   byte for byte. No file gains the key on a save.
2. The ladder is the crate's one ladder, `ops::depth::z_ladder` (`LadderMode::EvenRedistributed`),
   from the op top Z (`heights.top_z`) down to the lowest tip Z of the full-depth path of every
   curve of the op: `n = ceil(total / depth_per_pass)` equal steps. No second ladder.
3. Pass `i` is the full-depth path moved up by `level_i - lowest tip Z`. The last pass is the
   full-depth path itself.
4. A sample of an earlier pass at or above the op top Z is left out, and the chain splits there (as
   it does at a gap). So an earlier pass does not feed above the op top, and each pass cuts at most
   one step below the op top or below the pass before it.
5. Order: pass-major, as the history loop. Pass 1 covers every curve, then pass 2.
6. Entry of each later pass: the entry of the first pass. A rapid at the retract plane to the chain
   start, a straight plunge at the plunge rate (`EntryPlunge`), the cut, a rapid retract. The plunge
   goes through the slot of the pass before and then at most one step into material. The dressup
   stage sees each chain as it sees a chain of today.
7. The chain link stage (`chain_distance_mm`) runs on each pass alone. No link joins two passes.
   The rapid-order dressup gets a barrier before each later pass.
8. The stated cut: with the key, `Axial::PerPass(depth_per_pass)`, so the declared body depth is
   `Stated(depth_per_pass)`. With no key nothing moves.
9. No feeds door starts a ladder. The feeds setter of the op writes the key only when the op
   already states it; the operator states it through the parameter door or the project file.

Departures from the history convention:

- History used a constant step from the full depth up (`k * ch_step`). The one ladder gives equal
  steps `total / n`, each at or below the stated depth per pass.
- History fed the passes in air above the cavity floor. Rule 4 leaves those samples out.

## 3. What the cut object states today (found in the code, before the change)

- `compute/catalog.rs` (the `ProjectCurve` arm of `cut_spec`): `axial: Axial::SurfaceOffset(depth)`,
  `total: None(DEPTH_AFTER_SIMULATION)`.
- `compute/catalog/cuts.rs:1206-1214` (`declared_depth`): `DeclaredDepth::AtLeast(TipDepth)`.
- `compute/catalog/cuts.rs:156-168` (`TipDepth::lower_bound_mm`): a depth at or below 0 from above
  gives `None`.
- `feeds/planned.rs:315-317`: then `BodyDepth::None(DepthAfterSimulation)`. No row before the
  simulation holds a depth.
- The river pass has `depth = -4.0` from above. So `ProjectSession::stated_cut_over_a_limit`
  (`session/limit_rows.rs`) finds no stated row with a depth, and `export.stated_cut_over_limit`
  does not come for the 14.3 mm pass. The op states NO axial depth for a tip at or above the model
  surface, and a lower bound for a tip below it.

## 4. Predictions (before the code)

| # | Prediction |
|---|---|
| P-A1 | No key: the op golden does not move (the known 46 red rows, the same rows). |
| P-A2 | Flat plate at Z 0 = the op top, depth 3.0, depth per pass 1.0, 8 chains: 3 passes, 24 chains in the order 8 at Z -1, 8 at Z -2, 8 at Z -3; 24 plunges; every rapid at the retract plane; the last pass equals the no-key path. |
| P-A3 | Surface at Z 0, depth -4.0 (tip at Z 4), op top Z 10, depth per pass 2.5: `ceil(6 / 2.5) = 3` steps of 2.0, passes at Z 8, 6, 4. |
| P-A4 | A V valley: each earlier pass has fewer fed points than the last pass, no fed point of an earlier pass is at or above the op top, and each fed point is within one step of the op top or of the pass before it. |
| P-A5 | The stated cut: `Axial::PerPass` with the key, `Axial::SurfaceOffset` with no key. |
| P-A6 | The MCP wire snapshot moves by the one parameter `depth_per_pass` of `project_curve`. |
| P-A7 | FM1 does not move (no FM1 cell holds the key). |
| P-A8 | The red test: before the code a config with the key cuts 8 chains at Z -3 (one pass). |
