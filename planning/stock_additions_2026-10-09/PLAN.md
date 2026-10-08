# Stock changes per setup (add and remove material): plan (2026-10-09)

Owner: the operator. Lead: the local runner session. Status: ACTIVE.

## The operator's ruling

"Make sure that the epoxy is not part of the program, but a generic option
to add models in different setups."

So rs_cam gets a GENERIC capability: a setup can change the stock before its
first toolpath. It can add material or remove material, from a model or from
outlines, with any material from the library. Epoxy is only one material
that a user picks. No code, name, default or UI text says "epoxy".

Typical uses: a pour into milled channels (an inlay or resin fill), a glued
block or riser, a part that is cut free and taken away, a fixture insert.

## Evidence this plan builds on

- `planning/stock_fill_2026-10-09/DESIGN.md`: how the stock works now, and the
  options. Main finding: each setup's simulation starts from a FRESH stock
  (`compute/simulate.rs:1731`). Only `global_stock` carries, and only the
  scrub reads it. Nothing can be added to a stock that does not carry.
- `planning/sim_render_review_2026-10-09/REVIEW.md`: the render review. F1
  (inverted mesh winding in the software renderer) and F2 (three different
  multi-setup pictures) affect what the user sees of this feature.
- The acceptance case: the two-sided test piece
  (`planning/rivmap_block_job/testpiece.toml`).

## The model (generic)

```
SetupData.stock_changes: Vec<StockChange>      // applied in order, before the setup's first toolpath

StockChange {
    id, name, enabled,
    op: StockChangeOp,          // Add | Remove
    geometry: StockGeometry,
    material: MaterialRef,      // a material from the project's material library; Remove ignores it
}

StockGeometry::
    Model { model_id }                         // a closed mesh, in this setup's frame
    OutlineFill { model_ids, level_z }         // fill every space open to the top (this setup's +Z),
                                               // inside the outlines, up to level_z (setup frame)
    OutlineExtrude { model_ids, z_bottom, z_top }   // a prism from 2D outlines
```

- The setup frame decides "up". `OutlineFill` is what a pour does: it fills
  what was CUT, an overcut included.
- `Add` may extend above the current stock top (an overfill, a glued riser).
  The stock top then rises in those cells.
- `Remove` subtracts the geometry from every material.
- The material is a reference into the existing material library. The
  library gains no special entries. A user who wants "epoxy" adds a material
  named epoxy, with or without force data.

## Packages

Every package: worktree, its own `CARGO_TARGET_DIR` (deleted with the
worktree), cargo only through that worktree's `scripts/cargo_lane.sh`, every
test command under `timeout`, no commits by agents, ASD-STE100, no legacy
shims, no large gates (folder sentries, filtered core `--lib`, viz, cli,
clippy, fmt). The lead reviews each package by path and commits.

| ID | Package | Depends on | Proof |
|---|---|---|---|
| S0 | **Carry the stock across setups.** Setup N+1 starts from setup N's final stock, mapped into its frame (Top/Bottom flip, origin shift, Z rotation; lateral setups stay fresh, as today). Composite mesh: append only the last Z group's mesh. `prior_stocks` of setup N+1 = the carried stock. The scrub resumes from it. DESIGN.md section 2.0 | none | A two-setup test where a cut from setup 1 is present in setup 2's stock, flipped, at the right cell; the composite has no stacked solids; the test-piece checkpoints show the back channels from the front |
| S1 | **Material per dexel segment.** A material id on each segment (the default = the stock material). Union and subtract keep ids. The memory cost only where segments split. Cut samples record the material they cut | S0 | Unit tests of the ray operations with two materials; memory delta on the BASELINES protocol |
| S2 | **The model, the Command and the file.** `stock_changes` on `SetupData`; `AddStockChange`/`RemoveStockChange`/`ReplaceStockChange` through `ProjectSession::apply`; invalidation (the simulation of this setup and later; `FromRemainingStock` ops from this setup on); cache keys hash every change; project file `[[setups.stock_changes]]` | S0 | Command round-trip tests; a cache-key test that an edit stales the right results |
| S3 | **Geometry kernels.** `OutlineFill` and `OutlineExtrude` (the existing outline resolver, transformed to the setup frame; one cell scan, reusing `for_each_covered_cell`), and `Model` (a vertical ray cast with all hits, even-odd intervals, refuse a non-closed mesh by name). `Add` = union with the material; `Remove` = subtract | S1, S2 | Volume tests: a box fill, a fill into a pocket with an overcut, a hole through; the mesh cast on a closed box and on a box with a hole; non-closed refusal |
| S4 | **Surfaces.** GUI: a "Stock changes" list in the setup panel, beside fixtures (add, edit, reorder, enable; a material picker from the library; the added and removed volume shown). MCP: `add_stock_change`, `remove_stock_change`, `list_setups` and `inspect_stock` show the changes and the volumes. CLI: `project` prints one line per change with its volume. `FEATURE_CATALOG.md` | S2, S3 | viz tests; the MCP wire snapshot; GUI/MCP/CLI give the same volume (number parity) |
| S5 | **Show the material.** Per-vertex material colour from the segment ids in every stock view (live, playback, paused, `screenshot_simulation`); a legend row per material. Uses the render review P1 fixes (winding, deviation colours on their own mesh) | S1, R1 | A render test: a two-material stock gives two colours at the right cells |
| S6 | **"Cut as" per stock change** (operator ruling 2026-10-09: "in theory it is just more stock"). Each Add change has `cut_as: StockMaterial` (DEFAULT) or `OwnMaterial`. Default: the added material is cut exactly like the stock material for the gates, the cut metrics and the feed modulation (a mixed cut is a normal stock cut); only the colour (S5) and the "material present" fact differ. `OwnMaterial`: the gates use that material's force data, else "not judged: no force data for <material>". Toolpath generation and Suggest never read the added material | S1, S2 | Gate tests: default = identical verdicts to an all-stock run; OwnMaterial with and without force data |
| R1 | **Render review P1** (independent; can run first): fix the mesh winding, keep deviation colours on their own mesh, fix the per-toolpath overlay trim, per-pixel depth in the software renderer | none | REVIEW.md tests |

Order: **S0 and R1 in parallel** (different files), then S1 and S2, then S3,
then S4, S5 and S6. One agent edits one area at a time; a sequential
verifier runs the gates for each wave.

## Acceptance (the operator's test piece)

1. A material "epoxy" exists in the library as a user entry (no force data).
2. Setup 2 of the test piece has one stock change: Add, `OutlineFill` over
   the four channel outlines (models 3..6), level = the back face + the
   1 mm overfill, material = epoxy.
3. After the simulation:
   - the setup-3 front shows the channels in the epoxy colour, in the scrub,
     at the checkpoints and in the final view;
   - the added volume below the back face matches the process sheet's
     milled volume within one cell;
   - the setup-3 cut metrics name the epoxy samples as "not judged".
4. A project with no stock changes gives the same result as before, except
   the expected S0 change. S0 changes the setup-2+ metrics and the modulated
   F-words of every multi-setup project, because they now see the real stock.
   One live re-measure on a two-sided project before the S0 merge.

## Decisions taken (operator, 2026-10-09)

- Generic, not epoxy-specific (the ruling above).
- An `Add` may rise above the stock top (the overfill case).
- An added material is cut "same as the stock" by default (operator: "in theory it is just more stock"); "own material" is an option per change. With own material and no force data, it is counted and named, not judged.
- A live re-check of a real two-sided project before the S0 merge.
