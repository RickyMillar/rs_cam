# P2 completeness review — `eb862de5..5614d761`

**Verdict: GAP.** No behaviour, visibility or persisted value changed, and
no file was left behind; but `CREDITS.md` (13 paths), `lib.rs:41`, four test
doc comments and four documents that carry a bare file name still name the
old positions.

Reviewer scope: read-only. This review ran one cargo job (`doc -p
rs_cam_core --no-deps`). It compiled no test target and no bench target.

## Range composition

The range holds 20 commits, not 17. Three of them belong to another
workstream and touch only `planning/load_model_2026-09-16/`:

- `df9c58f0` — a prototype of the limits surface
- `7ba6beb6` — every limit is one kind of thing
- `f282d626` — distributions after the cut

They change no code. The other 17 commits are the P2 move and its two
documentation commits.

## Checks

| # | Check | Evidence command | Result | Finding |
|---|---|---|---|---|
| 1 | Behaviour change in moved files | `norm3.py`: rewrite every `crate::<old>` / `rs_cam_core::<old>` in the pre-range file, tokenise, then `difflib` against the post-range file | 52 of 106 renamed files are token-identical. The other 54 differ only by (a) `use` order, (b) rustfmt trailing commas and closure braces, (c) the `//!` paragraph plus `pub mod` block that Q2 appends to `dressup/mod.rs`, `io/mod.rs` and `machine/mod.rs`. | none |
| 1b | Behaviour change in modified files | `norm5.py`: the same rewrite, with every `use` statement and bare `mod` line removed first | 75 files differ. Every chunk is a rustfmt reflow (trailing comma, closure brace) or a documentation path rewrite. Example: `compute/execute.rs:370` becomes `\|z\| { Ok(crate::ops::trace_path::trace_polygons_at_z(&rings, z, &params)) }` — the same expression, wrapped. | none |
| 1c | Persisted value | `rg -n 'any::type_name\|module_path!' crates/` | 0 hits. No module path reaches a hash, a fingerprint or a serialised field. The `type_name` hits in `compute/catalog.rs` are a struct field of that name, not `std::any::type_name`. | none |
| 1d | Relative include in a moved file | `rg -n 'include_str!\|include_bytes!\|file!\(\)'` over the 106 renamed files (the deleted `simulation.rs` is the 107th) | 0 hits. No moved file reads a path relative to its own depth. | none |
| 2 | Visibility | A script compares every `mod` line of `git show eb862de5:.../lib.rs` with the same module in the new `lib.rs` or in its `folder/mod.rs` | One change: `artifact_io` goes from `mod` (crate root, so crate-wide) to `pub(crate) mod` in `export/mod.rs` (also crate-wide). Layout §7 requires it. `grid`, `memo` and `named_toml_library` stay private and narrow to their folder, as §7 allows. `nn_order` and `tool_shape_key` stay `pub(crate)`. `machine_kinematics` reports as absent because it renames to `machine/kinematics.rs`. | none |
| 3a | Left-behind root files | `ls crates/rs_cam_core/src/*.rs` | Exactly `lib.rs`, `geo.rs`, `ids.rs`, `interrupt.rs`, `measurement.rs`, `mesh.rs`, `polygon.rs`, `toolpath.rs` — the 8 files ruling Q1 names. | none |
| 3b | Folder file counts | `ls <folder>/*.rs \| wc -l` | `util` 2+mod, `geometry` 14+mod, `surface` 6+mod, `maps` 11+mod, `ops` 16+mod, `finish` 20+mod, `dressup` 6+mod(`dressup.rs`), `stock` 9+mod, `io` 6+mod(`io.rs`), `export` 4+mod, `trace` 5+mod, `machine` 3+mod(`machine.rs`), `material` 1+mod(`material.rs`). Every row matches the Q-ruling table. `lib.rs` declares 31 modules plus one `pub use`. | none |
| 3c | Duplicate file | `find -name '*.rs' \| xargs -n1 basename \| sort \| uniq -d` | 10 repeated stems, each in a different folder (`geometry/boundary.rs` and `tool_load/boundary.rs`, `ops/profile.rs` and `feeds/profile.rs`, and so on). §6 permits this. No path repeats. | none |
| 3d | Re-export shim | `rg -n 'pub use' crates/rs_cam_core/src/{export,finish,geometry,io,machine,maps,ops,stock,surface,trace,util,dressup,material}/mod.rs` | 0 hits. No folder facade re-exports an old crate-root path. | none |
| 4a | Stale `crate::<old>` in core source | `rg -n "crate::($STEMS)\b" crates/rs_cam_core/src`, with the self-mapping stems (`io`, `machine`, `material`, `dressup`) removed | 0 hits. | none |
| 4b | The deleted facades | `rg -n 'crate::simulation::\|rs_cam_core::simulation::' crates/`; `rg -n 'crate::trace::' crates/rs_cam_core/src` filtered to the five `trace/` members | 0 hits each. `crate::trace::` names only the folder. `rs_cam_viz::app::gpu_upload` and the controller tests name `rs_cam_core::stock::stock_mesh::StockMesh`. | none |
| 4c | Stale `rs_cam_core::<old>` outside core | `rg -n "rs_cam_core::($STEMS)\b" crates/ scripts/` | 0 hits. | none |
| 4d | Brace-grouped import | `rg -U -n "use (rs_cam_core\|crate)::\{[^}]*\}" crates/` filtered for an old stem | 0 hits. The 16 brace blocks that layout §5 counted carry no old stem. | none |
| 4e | Stale `crate::<old>` in test prose | `rg -n "crate::($STEMS)\b" crates/rs_cam_core/tests` | 4 hits. See finding **F3**. | **F3** |
| 4f | Stale `src/<moved>.rs` in a live document | `rg -n "src/($STEMS)\.rs" .` over `CLAUDE.md`, `FEATURE_CATALOG.md`, `AI_MACHINIST_ANALYSIS_REFERENCE.md`, `architecture/`, `.claude/`, `planning/AGENT_CODEMAP.md`, `CREDITS.md`, `crates/*/src`, `crates/*/tests`, `scripts/` | `CREDITS.md` holds 13. Every other live document is clean; commits `1a486a47` and `5614d761` repaired them. | **F1** |
| 4g | Orphan comment in `lib.rs` | `sed -n '38,44p' crates/rs_cam_core/src/lib.rs` | Line 41 keeps `// The walk grid \`tier_map\` and \`reach_map\` share; private to the crate.` between `pub mod geo;` and `pub mod geometry;`. The comment belonged to `mod grid;`. `maps/mod.rs` already carries the correct copy. | **F2** |
| 4h | Dated evidence that keeps old paths | the same scan over `planning/` | Reported below, not a defect. | none |
| 4i | Bare file name of a renamed file | `rg -n '\b(trace\|machine_kinematics\|io\|machine\|dressup\|material\|simulation)\.rs\b'` over the live documents, then `find crates -name '<name>.rs'` to see which names still exist | `trace.rs`, `machine_kinematics.rs`, `machine.rs`, `dressup.rs` and `material.rs` exist nowhere in the tree. Four live documents still name them. See finding **F4**. | **F4** |
| 5a | Sentry path literals | A script reads all 79 files in `source_scanning_sentries.txt` and resolves every `"…\.rs"` literal against the crate root, `src/` and the test directory | Every unresolved literal is a `rs_cam_viz` `src/ui/…` path or a format-string fragment. No literal names a moved core file. | none |
| 5b | The two §9 sentry fixes | `rg -n 'ramp_finish.rs\|PLUMBING' crates/rs_cam_core/tests/checkpoint_b_resolution_ab.rs`; `rg -n 'src/ops/pocket.rs' crates/rs_cam_core/tests/common/adversarial2d.rs` | Line 1117 reads `"finish/ramp_finish.rs"` and the two neighbours match. `PLUMBING` keeps the bare names, as §9 requires. `adversarial2d.rs:546` reads `crates/rs_cam_core/src/ops/pocket.rs`. | none |
| 5c | The recursive-walker claim | `rg -c 'is_dir\(\)'` over the 8 files §9 names | All 8 recurse. None passes vacuously after the move. | none |
| 5d | The extra §9 prose fix | `rg -n 'tool_shape_key' crates/rs_cam_viz/src/state/runtime.rs` | Line 127 reads `rs_cam_core::maps::tool_shape_key::ToolShapeKey`. | none |
| 6 | rustdoc | `scripts/cargo_lane.sh doc -p rs_cam_core --no-deps` | Exit 0. 233 warnings: 85 `unresolved link`, 139 `links to private item`, 6 redundant link target, 3 unclosed HTML tag. No unresolved link names an old root path. The five that carry a new path (`crate::finish::pencil::emit_paths` and four more) point at items that do not exist under either name, so they predate the move. The 31 `REPO` links are citation markers. | none |
| 7a | Mutation contract | `rg -o 'fn [a-z_]+_mut\b'` over `git archive eb862de5` and over the working tree, sorted and counted | The two lists are identical. No new `_mut` function. | none |
| 7b | GUI type in core, parallel flow | `git diff eb862de5 5614d761 -- Cargo.toml crates/*/Cargo.toml` | Empty. No dependency changed. The only `build.rs` change is one doc line: `rs_cam_core::build_info` becomes `rs_cam_core::util::build_info`. | none |

## Findings

### F1 — `CREDITS.md` keeps 13 old module paths (live document)

The root `CLAUDE.md` names `CREDITS.md` as a source of truth. The range did
not update it.

| Line | Old path | New path |
|---|---|---|
| 134 | `crates/rs_cam_core/src/conformal_spiral.rs` | `src/finish/conformal_spiral.rs` |
| 179 | `crates/rs_cam_core/src/dexel.rs` | `src/stock/dexel.rs` |
| 181 | `crates/rs_cam_core/src/dexel_mesh.rs` | `src/stock/dexel_mesh.rs` |
| 205 | `crates/rs_cam_core/src/dropcutter.rs` | `src/surface/dropcutter.rs` |
| 206 | `crates/rs_cam_core/src/waterline.rs` | `src/ops/waterline.rs` |
| 208 | `crates/rs_cam_core/src/tsp.rs` | `src/dressup/tsp.rs` |
| 209 | `crates/rs_cam_core/src/scallop_math.rs` | `src/finish/scallop_math.rs` |
| 210 | `crates/rs_cam_core/src/arcfit.rs` | `src/dressup/arcfit.rs` |
| 211 | `crates/rs_cam_core/src/contour_extract.rs` | `src/geometry/contour_extract.rs` |
| 470 | `crates/rs_cam_core/src/material.rs` | `src/material/mod.rs` |
| 525 | `crates/rs_cam_core/src/material.rs` | `src/material/mod.rs` |
| 545 | `crates/rs_cam_core/src/material.rs` | `src/material/mod.rs` |
| 813 | `crates/rs_cam_core/src/viz.rs` | `src/export/viz.rs` |

Line 180 (`src/dexel_stock.rs`) is a separate, older error: the path is
`src/dexel_stock/`, a folder that P2 did not touch. Repair it in the same
pass.

### F2 — orphan comment at `crates/rs_cam_core/src/lib.rs:41`

```
pub mod geo;
// The walk grid `tier_map` and `reach_map` share; private to the crate.
pub mod geometry;
```

The comment documented `mod grid;`. The move deleted the `mod grid;` line
and left the comment, where it now reads as a description of `geometry`.
`maps/mod.rs:8` already carries the correct copy. Delete line 41.

### F3 — four old `crate::<stem>` paths in test doc comments

An integration test's `crate::` names the test binary, so these paths were
already wrong before the move. They are still stale names of moved modules.

| Site | Text | Correct name |
|---|---|---|
| `crates/rs_cam_core/tests/face_stock_top_frame_f028.rs:5` | `crate::face::face_toolpath` | `rs_cam_core::ops::face::face_toolpath` |
| `crates/rs_cam_core/tests/generic_rest_routing_pr7.rs:12` | `crate::reach` | `rs_cam_core::surface::reach` |
| `crates/rs_cam_core/tests/pencil_spine_ab_p1.rs:14` | `crate::flow_accum` | `rs_cam_core::surface::flow_accum` |
| `crates/rs_cam_core/tests/conformal_spiral_synthetic_f2.rs:1893` | `crate::crest_lines` | `rs_cam_core::finish::crest_lines` |

### F4 — four live documents name a file that no longer exists

Commit `1a486a47` rewrote every path that carried a `src/` prefix, and its
message states that it left bare file names alone. That rule is safe for a
name the move kept, such as `scallop.rs`. It is not safe for the five names
the move retired. `trace.rs`, `machine_kinematics.rs`, `machine.rs`,
`dressup.rs` and `material.rs` match no file in the tree today.

| Site | Text | Correct name |
|---|---|---|
| `FEATURE_CATALOG.md:27` | `\| 2.5D \| Trace \| \`trace.rs\` \|` | `ops/trace_path.rs` |
| `planning/AGENT_CODEMAP.md:146` | `\| \`io.rs\` \| Shared IO helpers. \|` | `io/mod.rs` |
| `.claude/skills/dev/SKILL.md:48` | `trace.rs` in the operations list | `ops/trace_path.rs` |
| `.claude/skills/dev/SKILL.md:52` | `simulation.rs` in the simulation list | the facade is deleted; name `stock/stock_mesh.rs` |
| `.claude/skills/dev/SKILL.md:56` | `dressup.rs` | `dressup/mod.rs` |
| `.claude/skills/dev/SKILL.md:66` | `machine.rs`, `material.rs` | `machine/mod.rs`, `material/mod.rs` |

The range updated `.claude/agents/cam-navigator.md`,
`.claude/agents/sim-diagnostics.md` and
`.claude/skills/sim-analysis/SKILL.md`, but not
`.claude/skills/dev/SKILL.md`. That file carries older staleness as well: it
claims 56 modules and names `gcode.rs`, `adaptive.rs`, `dexel_stock.rs` and
`pipeline.rs`, which became folders or vanished before P2. Rewrite the whole
module census, not only the five names P2 retired.

`architecture/orientation_refactor.md` names `trace.rs` at lines 193, 228,
230 and 272, and `simulation.rs` at lines 27, 28, 59 and 88. It is a dated
design note — it also names `operations_2d.rs`, which predates this
programme — so it ranks with the dated evidence below.

### Dated evidence that keeps old paths (no action required)

The brief asks which dated files keep old paths. These are records of what
was true on their date, so they may keep them:

- `planning/PROGRESS.md` — 8 lines: 880, 988, 1150, 1255, 1260, 1301, 1366,
  1505. All sit inside closed phase entries.
- `planning/linking_2026-09-09/PENCIL_2026-09-04.md` — lines 437, 440.
- `planning/island_clip_2026-09-09/G-ISOCLIPRAPID_RAMPFALL_report.md` —
  lines 75, 173.
- `planning/ui_review_2026-09-14/` — clean.
- `planning/structure_2026-09-17/P2_LAYOUT.md` (6) and `P1_MANIFEST.md` (1)
  — the move order itself names the old paths by design.
- `research/feeds_and_speeds_integration_plan.md` — 3 lines, a 2026-05 plan.
- The `planning/tech_debt_2026-09-16/evidence*` and
  `planning/duplicate_sweep_2026-09-15/` trees — thousands of lines of
  measured output. Do not rewrite them; a rewritten instrument output is a
  lie about its own date.

## What this review did not verify

State these to the verifier; they are outside a read-only reviewer's one
cargo job.

1. **Test and bench targets.** `cargo doc --no-deps` compiles the library
   only. It compiles no `#[cfg(test)]` module, no `tests/*.rs` and no
   `benches/*.rs`. About 357 out-of-crate files carry rewritten paths. The
   static scans (checks 4a–4d, all zero) are the only evidence for them.
   The closing gate is `cargo check -p rs_cam_core --all-targets`, then the
   same for `rs_cam_viz` and `rs_cam_cli`.
2. **`io/step_input.rs`.** The `step` feature is not in `default`, so
   nothing in this review compiled that file. Its token diff shows an import
   reorder only. Add `--features step` to one check.
3. **The working tree, not the commit.** The cargo job ran on the working
   tree, which holds the uncommitted `feeds/**` and `tool_load/**` edits of
   two other agents.
4. **The rustdoc baseline.** The recorded 255 belongs to
   `planning/tech_debt_2026-09-16/EXECUTION_SUMMARY.md:151` and its command
   is not recorded. Do not read 233 as a fall of 22. The load-bearing
   statement is the one in check 6: no unresolved link names an old root
   path.

## Fix list

Ranked as the brief asks: behaviour change, then shim, then stale path in a
live document, then stale path in dated evidence, then rustdoc.

1. **(behaviour change)** None found. No item.
2. **(shim)** None found. No item.
3. **(live document)** `CREDITS.md` — rewrite the 13 paths in the F1 table.
   Repair line 180 (`src/dexel_stock.rs` → `src/dexel_stock/`) in the same
   pass.
4. **(live document)** `crates/rs_cam_core/src/lib.rs:41` — delete the
   orphan `grid` comment.
5. **(live document)** `.claude/skills/dev/SKILL.md` — rewrite the module
   census of `rs_cam_core`. The file names five retired files and predates
   P2 in other rows as well.
6. **(live document)** `FEATURE_CATALOG.md:27` — `trace.rs` becomes
   `ops/trace_path.rs`.
7. **(live document)** `planning/AGENT_CODEMAP.md:146` — `io.rs` becomes
   `io/mod.rs`.
8. **(live document)** The four test doc comments in the F3 table — rewrite
   each to its `rs_cam_core::<folder>::<module>` name.
9. **(dated evidence)** No action. Leave `PROGRESS.md`, the two open specs,
   `architecture/orientation_refactor.md` and the evidence trees as they
   stand.
10. **(rustdoc)** No action. The count did not rise from the move, and no
    unresolved link names an old path.

## Instruments

The scripts this review used are in the session scratchpad, not in the
repository:

- `norm3.py` — rewrite, tokenise and diff every renamed file.
- `norm5.py` — the same, with `use` statements and bare `mod` lines removed,
  over renamed and modified files together.
- `sentry.py` — resolve every path literal in the 79 sentry files.
