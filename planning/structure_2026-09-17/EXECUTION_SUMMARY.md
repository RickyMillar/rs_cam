# EXECUTION_SUMMARY.md — structure programme 3, 2026-09-17

Orchestrated by Claude Fable 5.1 from `PROMPT.md` (`540b66d0`). Opus agents
did the work: one P1 agent, one P2 layout agent, one P2 move agent, one P2
reviewer, one feeds triage agent, two feeds fix agents, one red-test
diagnosis agent, two red-test fix agents, one round-2 split-proposal agent,
one round-2 residue agent. Every item is one commit on `master` with an
explicit pathspec. The operator left mid-programme with the standing
instruction to continue unattended.

## Operator rulings

- 2026-09-16: no legacy support; delete, do not shim.
- 2026-09-17: obsolete planning material is deleted behind an annotated tag,
  not archived. Decision rationale stays findable through
  `planning/DELETED_INDEX.md`.
- 2026-09-17 (P2): the spine stays at the root; `folder/mod.rs` carries the
  folder's principal type; the trace operation is `ops/trace_path.rs`; the
  `simulation.rs` facade is deleted.
- 2026-09-17: the freeze on `feeds/**` and `tool_load/**` is lifted (the
  power-calcs session writes only an HTML prototype under
  `planning/load_model_2026-09-16/`).

## Numbers, before → after

| measure | before | after |
|---|---|---|
| tracked files under `planning/` | 1 415 | 418 |
| tracked bytes under `planning/` | 1 199 472 022 | 53 144 805 |
| top-level markdown files in `planning/` | 155 | 25 |
| package directories in `planning/` | 46 | 38 |
| `.rs` files at the root of `crates/rs_cam_core/src/` | 115 | 8 |
| `mod` lines in `crates/rs_cam_core/src/lib.rs` | 125 | 31 |
| folders under `crates/rs_cam_core/src/` | 12 | 24 |
| known red tests handed off by earlier programmes | 5 | 0 |
| production files over 3 000 lines (core + viz) | 19 | 1 (`polygon.rs`, ruled) |
| folder-level `CLAUDE.md` files | 0 | 32 |

Range `540b66d0..e0125f8b` (through P2 and the feeds wave): 69 commits.

## P1 — planning purge (9 commits, `56cd3ab2..4ba1b2a2`)

Tag `planning-pre-purge-2026-09-17` sits on `38291bdf`. Any deleted file
returns with `git show planning-pre-purge-2026-09-17:<path>`. 1 010 tracked
files and 1 146 727 947 bytes left the tree. `DELETED_INDEX.md` holds one
rationale line per deleted package, the root deletions, the four moves and
the retrieval command.

The brief said no crate reads a `planning/` path. That was false: 406 files
under `crates/` cite one. The agent classified every citation by behaviour
when absent. Ten files that plain or ignored tests open at run time are
kept (`toolpath_acceptance/`, `gcode_current_outputs/`, three wanaka200
files, `airrun_2026-06-01/wanaka.toml`, two multitool projects, one
deep-DOC project). The 293 doc-comment citations of deleted paths are
deliberately not rewritten; the three `CLAUDE.md` files say a dead
`planning/…` path in a comment is expected. Both run-time readers pass.

History is not rewritten. The 948 MB blob stays in the object database. A
`git filter-repo` pass is an operator decision for a moment when no session
and no clone is live. The tag is local; pushing it is the operator's call.

## P3 — root tidy (`5509f452`, `6474ff8f`)

`review/` (106 files), `G13_PROMPT.md`, `AGENT_PROMPT.md`, five generated
fixture outputs and the excluded `tests/step_validation` crate (with its
`Cargo.toml` exclude line) are gone. Root screenshots are ignored. The
untracked `demos/` (103 MB) and `reference/` (4.9 GB) stay for the operator.

## P2 — core module grouping (17 commits, `5166244c..5614d761`)

Layout `P2_LAYOUT.md` (`939b69b9`, rulings `5166244c`). One sequential move
agent, one folder per commit, each commit gated by
`check --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/step`
and fmt. 3 454 path sites rewritten; no re-export shim; one visibility
change (`artifact_io` → `pub(crate)`, required by the layout). The
`checkpoint_b_resolution_ab` sentry gained the `finish/` prefix on three
literals. Final gate: workspace clippy `-D warnings` clean, fmt clean, core
`--lib` 2527/0, cli 34/0, mcp 29/0, viz `--lib` 382 + the three known reds.

Completeness review (`P2_REVIEW.md`): GAP — no
behaviour, visibility or persisted value changed, nothing left behind; six
documents still named old paths. Fixed in `1e2c4f61` (CREDITS.md 15 paths,
FEATURE_CATALOG, AGENT_CODEMAP, the dev skill census, four test docs, one
orphan comment in `lib.rs`). rustdoc: 233 warnings (85 unresolved links,
139 private links), none naming an old root path; the 255 baseline's
command is unrecorded, so the two numbers are not a trend.

`rs_cam_viz` was measured and not moved: `ui/` holds 25 panel files beside
four folders; the two groups that grew are already foldered.

## Feeds wave (23 commits, `FEEDS_WAVE.md` `f9e765e7`)

Triage: 23 rows, no tier A. Agent A (`feeds/`, 11 commits `e4b05e40..68745825`):
`enumerate_matching_rows`, `VendorLut::load_dir`, `render_label`, the
`stock_to_leave_radial` match arms deleted; eight own-file-only items
private; nine inert `dead_code` attributes gone; every allow carries
`SAFETY:`; four `Test door:` lines. Agent B (`tool_load/`, 12 commits
`ce6c5a7f..905bbb80`): `evaluate_project`, `active_axes` deleted; the
`verdict.rs` banner that said "no consumers" for types with 181 references
corrected; twelve items to `pub(crate)`; nine stale "run_stage_*" comments
gone. Net −49 lines in `feeds/`, −25 in `tool_load/`. Gates: `--lib`
feeds 223/0, tool_load 363/0, 18 + 19 integration targets green, clippy
clean. The two long simulations (`wanaka_suggest_integration`,
`wanaka_e2e_chipload_gate`) were not run under the no-large-gates ruling.

Instrument lesson: `private_interfaces` fires on nominal visibility, so a
`pub(crate)` demotion cascades to every type in the item's signature (seven
follow-on demotions in `tool_load`). FW-22 (`calculate`, 1 302 lines) has no
mechanical seam: 16 `let mut` locals cross its 19 step banners and
`effective_d` is rebound mid-way; recorded on T-5 in the register. T-1
closed.

## Red tests (`RED_TESTS.md` `17740823`; fixes `56011126`, `b037568d`, `82233556`, `03e13fff`, `20160147`)

Two hand-off attributions were wrong: test 3 came from UR2 `e7901838`, not
UR3; test 5 came from the ramp fold `fd135a04`, not `e2ecf697` (viz-only).
Tests 3 and 4 pinned behaviour the product changed on purpose (badge text
"1 safety"; one `RunSimulation` producer). Tests 1 and 2 were fixture
defects (a seeded result; an unstamped inject). Viz `--lib` is 385/0 for
the first time since `f97327c3`. Test 5 was an instrument defect (the
median took every F word, entry feeds included); `d1efe266` measures the
cutting population by `MoveIntent`, the gate's own partition; the median
lands on the band floor (the modulator's clamp) with no band change.

## Round 2 and P4/P5

`evidence_round2/` (`27fa79db`): dead pub 8 → 4, own-file-only 55 → 34,
legacy lines 479 → 468, allows 677 → 665 (449 without `SAFETY:`), test-only
pub 36, functions ≥ 250 lines 89, src near-dup pairs 118 at 0.88 (moved
files re-chunked).

**Residue wave** (5 commits `4c82e4b4..bc8c4338`, net −373 lines): the
whole dead `ToolpathSemanticWriter`; `NewDefaultCtx` and
`apply_stock_defaults` (a duplicate of `feeds::suggest::StockContext`);
three demotions; six `Test door:` lines; the dead viz runtime-profile
cluster (454 lines out of `state/simulation.rs`, nine state methods and five
UI functions lose an argument). Own-file-only rows were mostly false
positives: the word-count instrument does not read the `Stays pub` doc
lines W4 wrote. Q1 remainder: six of eight Suggest sites now carry the
model bbox; the two behind `draw_toolpath_panel` need a 25th parameter and
stay with the ui-premium owner.

**Smoke re-baseline** (`a815b7e4`): the CLI built at `bb1ccc7b^` and at
`f649bef6` produce byte-identical smoke CSVs, so the populated context
changes no case. `2026-09-17.csv` replaces the 3.5-month-old June baseline;
the notes list the June deltas and flag AS014's power verdict
(`within` → `exceeds`) for the operator.

**P4 big-file splits** (`P4_SPLITS.md` `6800ccb5`): 19 files over 3 000
lines; 60 271 of 83 364 lines can leave their parents; 30 % of those lines
are inline test modules. Rulings: `polygon.rs` stays whole (spine);
tests-only moves in `feeds/` and `tool_load/` are allowed. Wave 1 (six
files, six commits, each a pure move proven by item inventory):

| file | before | parent after | children |
|---|---:|---:|---|
| `adaptive3d/mod.rs` `f649bef6` | 3 018 | 532 | tests |
| `dressup/mod.rs` `d883c077` | 4 726 | 2 181 | air_cut, entry_descent, link, tests |
| `stock/simulation_cut.rs` `0658302c` | 3 141 | 887 | accumulate, analysis, reporting, tests |
| `session/mutation.rs` `90aff4dd` | 3 272 | 131 | toolpath, entities, config, tests |
| `compute/catalog.rs` `58473752` | 3 287 | 1 412 | schema, registry, tests |
| `finish/pencil.rs` `7d7138d6` | 3 634 | 743 | chain_paths, detectors, emission, tests |

Combined HEAD gate after wave 1: workspace clippy `-D warnings` clean, fmt
clean, core `--lib` 2526/0. Lessons: the plan's visibility column was
built from rustdoc links, not call sites (grep call sites, let the compiler
decide); `wildcard_imports` is denied, so re-exports list names; a
`#[cfg(test)]` item cannot be re-exported in a lib build. Three agents were
cut off by the usage limit mid-split; their partial state was coherent and
was finished, not redone.

Wave 2 (six files): `unified_finish` 4 806 → 2 673 (`09884357`), `scallop`
3 580 → 1 158 (`5b20691e`), `feeds/mod` 4 624 → 2 767 tests-only
(`c9551ad3`), `tool_load/optimize/mod` 3 361 → 1 019 five test files
(`a94edf29`), `compute/execute` 6 362 → 752 nine children (`56e383f0`),
`session/compute` 8 032 → 2 252 six children (`451680b3`). The
`checkpoint_b_resolution_ab` sentry is a text census over `src/`: a door
string that moves into a child changes its expected list (`5b20691e`); an
`include_str!` needle moved with its function (`451680b3`). `macro_rules!`
textual scope forces `mod` lines below the macro; a sibling `tests.rs` needs
`pub(super)` on struct fields.

Wave 3 (two files): `feeds/suggest` 5 769 → 879, five children
(`1d808fb3..56b6b9c0`, the inert `too_many_arguments` allow deleted);
`finish/conformal_spiral` 4 401 → 2 981, tests plus `spiral_build.rs`
(`80b0d4a8`, `8e9f5cca`); its `flatten` and `rings` children were declined
at 13 and 16 visibility changes.

Viz wave (operator ruling: "happy to run the file split, high risk"):
`app/mcp.rs` 6 067 → 1 089, six children, ten sentries repointed with their
assertions intact and the negative `stamp_stale` check widened to every
child (`15d4dc00..857709cd`); `ui/properties/mod.rs` 5 965 → 1 152, eight
children, seven sentries now read the folder (`ff55de3f..ee25a1a3`);
`ui/properties/operations/mod.rs` 3 013 → 766 (`a4839e74`);
`state/simulation.rs` 2 908 → 1 014 (`cd35f22a`).

After all waves the largest production file is `polygon.rs` at 3 004 lines
(ruled whole, spine) and `conformal_spiral.rs` at 2 981; every other
production file is under 2 900. Final gate: workspace clippy
`--features rs_cam_core/heavy-tests -D warnings` clean, fmt clean, core
`--lib` 2526/0, viz `--lib` 384/0 and every viz integration binary green,
cli and mcp green.

**P5 per-folder instruction files** (`P5_CLAUDE_MD.md` `010758d9`; files
`a52d5714`, parents `45445869`, de-dup `22d629fa`, evidence `e958f9a7`, maps
`556d646c..f89e4d88`): 32 folder `CLAUDE.md` files, each ≤ 40 lines, with a
file map, invariants, sentries and traps; the core crate file 121 → 55
lines, viz 68 → 40; root gains one paragraph. Durable rules that lived only
in the orchestrator's memory (chipload-bounds mirror, the steady-state gate
predicate, the 2D stock Z frame, rest-measurement prerequisites, the mutation
door) now live in the one folder that owns each. Two rules were dropped
because their symbols no longer exist; 13 unsourced bullets were deleted
rather than kept.

Range `540b66d0..HEAD`: 135 commits.

## Follow-ups (recorded, not scheduled)

- ~~`git filter-repo`~~ DONE (operator ruling): four blobs out of every commit,
  HEAD tree byte-identical, pack 271 → 129 MB, 2 614 commits re-hashed;
  every cited hash in the tree was rewritten from the map at
  `commit_map_2026-09-17.tsv`. **The GitHub remote still holds the old
  history until a force-push (`git push --force --all --tags`); the
  `cutting-calcs-gaps` worktree was rewritten in place.**
- `demos/`, `reference/` at the root (untracked): operator ruling 2026-09-17, keep as test beds.
- 293 doc-comment citations of deleted planning paths (ruled: leave).
- FW-22 / T-5: `feeds::calculate` split is a design decision.
- `preview_field_applies` carries an inert `too_many_arguments` allow (7 args).
- rustdoc: 85 unresolved intra-doc links, plus parent-to-private-child links from the splits (not gated).
- Q1 CLOSED in the GUI (`ed1da4a9`): the panel snapshot carries the model bbox; the pill number now matches apply-all (a latent G-PILLCLAMP mismatch). Residual: the two GUI Suggest sites and the six wired sites pass no `StockContext`, so a drill op's inspector rationale can still differ from MCP's on the peck clamp; fix = one more snapshot field.
- Sentry-teeth review (`SENTRY_TEETH_REVIEW.md`): 14 of 15 edited viz sentries had teeth; the four vacuous arms and the dead wp6b allowance were fixed with red proofs (`3f4d48c8`).
- COLLISION_POINT and SPACE_0 stay held for `ui_premium_2026-09-13`.
