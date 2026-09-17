# EXECUTION_SUMMARY.md — structure programme 3, 2026-09-17

Orchestrated by Claude Fable 5.1 from `PROMPT.md` (`fa8176fa`). Opus agents
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

Range `fa8176fa..e0125f8b`: 69 commits (5 belong to the peer's load-model
workstream), 543 files under `crates/` changed, +4 503 / −3 979 lines there.

## P1 — planning purge (9 commits, `aede83fc..4ba1b2a2`)

Tag `planning-pre-purge-2026-09-17` sits on `2b1416ab`. Any deleted file
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

## P3 — root tidy (`155f1178`, `eb862de5`)

`review/` (106 files), `G13_PROMPT.md`, `AGENT_PROMPT.md`, five generated
fixture outputs and the excluded `tests/step_validation` crate (with its
`Cargo.toml` exclude line) are gone. Root screenshots are ignored. The
untracked `demos/` (103 MB) and `reference/` (4.9 GB) stay for the operator.

## P2 — core module grouping (17 commits, `602911a6..5614d761`)

Layout `P2_LAYOUT.md` (`677e96b0`, rulings `602911a6`). One sequential move
agent, one folder per commit, each commit gated by
`check --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/step`
and fmt. 3 454 path sites rewritten; no re-export shim; one visibility
change (`artifact_io` → `pub(crate)`, required by the layout). The
`checkpoint_b_resolution_ab` sentry gained the `finish/` prefix on three
literals. Final gate: workspace clippy `-D warnings` clean, fmt clean, core
`--lib` 2527/0, cli 34/0, mcp 29/0, viz `--lib` 382 + the three known reds.

Completeness review (`P2_REVIEW.md`): GAP — no
behaviour, visibility or persisted value changed, nothing left behind; six
documents still named old paths. Fixed in `e0125f8b` (CREDITS.md 15 paths,
FEATURE_CATALOG, AGENT_CODEMAP, the dev skill census, four test docs, one
orphan comment in `lib.rs`). rustdoc: 233 warnings (85 unresolved links,
139 private links), none naming an old root path; the 255 baseline's
command is unrecorded, so the two numbers are not a trend.

`rs_cam_viz` was measured and not moved: `ui/` holds 25 panel files beside
four folders; the two groups that grew are already foldered.

## Feeds wave (23 commits, `FEEDS_WAVE.md` `ae40daee`)

Triage: 23 rows, no tier A. Agent A (`feeds/`, 11 commits `126e8053..68745825`):
`enumerate_matching_rows`, `VendorLut::load_dir`, `render_label`, the
`stock_to_leave_radial` match arms deleted; eight own-file-only items
private; nine inert `dead_code` attributes gone; every allow carries
`SAFETY:`; four `Test door:` lines. Agent B (`tool_load/`, 12 commits
`2340aaed..905bbb80`): `evaluate_project`, `active_axes` deleted; the
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

## Red tests (`RED_TESTS.md` `17740823`; fixes `fc07674f`, `8cac2857`, `a9333598`, `ad944a0a`, `20160147`)

Two hand-off attributions were wrong: test 3 came from UR2 `ec7c6acc`, not
UR3; test 5 came from the ramp fold `d0aeee02`, not `7a5fdad4` (viz-only).
Tests 3 and 4 pinned behaviour the product changed on purpose (badge text
"1 safety"; one `RunSimulation` producer). Tests 1 and 2 were fixture
defects (a seeded result; an unstamped inject). Viz `--lib` is 385/0 for
the first time since `7b4e18f6`. Test 5 was an instrument defect (the
median took every F word, entry feeds included); `4d4d7a9b` measures the
cutting population by `MoveIntent`, the gate's own partition; the median
lands on the band floor (the modulator's clamp) with no band change.

## Round 2

`evidence_round2/` (`2dcb2123`): dead pub 8 → 4, own-file-only 55 → 34,
legacy lines 479 → 468, allows 677 → 665 (449 without `SAFETY:`), test-only
pub 36, functions ≥ 250 lines 89, src near-dup pairs 118 at 0.88 (moved
files re-chunked).

**Residue wave** (5 commits `4a9928e1..bc8c4338`, net −373 lines): the
whole dead `ToolpathSemanticWriter`; `NewDefaultCtx` and
`apply_stock_defaults` (a duplicate of `feeds::suggest::StockContext`);
three demotions; six `Test door:` lines; the dead viz runtime-profile
cluster (454 lines out of `state/simulation.rs`, nine state methods and five
UI functions lose an argument). Own-file-only rows were mostly false
positives: the word-count instrument does not read the `Stays pub` doc
lines W4 wrote. Q1 remainder: six of eight Suggest sites now carry the
model bbox; the two behind `draw_toolpath_panel` need a 25th parameter and
stay with the ui-premium owner.

**Smoke re-baseline** (`22c9310f`): the CLI built at `21f94270^` and at
`b0d9d330` produce byte-identical smoke CSVs, so the populated context
changes no case. `2026-09-17.csv` replaces the 3.5-month-old June baseline;
the notes list the June deltas and flag AS014's power verdict
(`within` → `exceeds`) for the operator.

**P4 big-file splits** (`P4_SPLITS.md` `7d4bec86`): 19 files over 3 000
lines; 60 271 of 83 364 lines can leave their parents; 30 % of those lines
are inline test modules. Rulings: `polygon.rs` stays whole (spine);
tests-only moves in `feeds/` and `tool_load/` are allowed. Wave 1 (six
files, six commits, each a pure move proven by item inventory):

| file | before | parent after | children |
|---|---:|---:|---|
| `adaptive3d/mod.rs` `b0d9d330` | 3 018 | 532 | tests |
| `dressup/mod.rs` `a1b79182` | 4 726 | 2 181 | air_cut, entry_descent, link, tests |
| `stock/simulation_cut.rs` `6db8fe55` | 3 141 | 887 | accumulate, analysis, reporting, tests |
| `session/mutation.rs` `a7f6d501` | 3 272 | 131 | toolpath, entities, config, tests |
| `compute/catalog.rs` `58473752` | 3 287 | 1 412 | schema, registry, tests |
| `finish/pencil.rs` `785b4acb` | 3 634 | 743 | chain_paths, detectors, emission, tests |

Combined HEAD gate after wave 1: workspace clippy `-D warnings` clean, fmt
clean, core `--lib` 2526/0. Lessons: the plan's visibility column was
built from rustdoc links, not call sites (grep call sites, let the compiler
decide); `wildcard_imports` is denied, so re-exports list names; a
`#[cfg(test)]` item cannot be re-exported in a lib build. Three agents were
cut off by the usage limit mid-split; their partial state was coherent and
was finished, not redone. Wave 2 (`unified_finish`, `scallop`, `feeds/mod`,
`tool_load/optimize/mod`, `compute/execute`, `session/compute`) is running.

## Follow-ups (recorded, not scheduled)

- `git filter-repo` for the 948 MB blob; push of the tag.
- `demos/`, `reference/` at the root (untracked).
- 293 doc-comment citations of deleted planning paths (ruled: leave).
- FW-22 / T-5: `feeds::calculate` split is a design decision.
- `preview_field_applies` carries an inert `too_many_arguments` allow (7 args).
- rustdoc: 85 unresolved intra-doc links.
- COLLISION_POINT and SPACE_0 stay held for `ui_premium_2026-09-13`.
