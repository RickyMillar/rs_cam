# Programme 3 — navigable structure (prompt for a fresh session)

Paste everything below the line into a new Claude Code session in
`/home/ricky/personal_repos/rs_cam`. It is self-contained.

---

Orchestrate a structure clean-up of the rs_cam Rust workspace. Goal: a
navigable repository. Three work packages, in order: **P1 planning purge**,
**P2 core module grouping**, **P3 root tidy**. Use Opus agents for the
implementation; verify the plan yourself before dispatch; orchestrate, do not
implement large moves in your own context.

## Read first (in order)
1. `CLAUDE.md` (root), `planning/CLAUDE.md`, `crates/rs_cam_core/CLAUDE.md`,
   `crates/rs_cam_viz/CLAUDE.md`.
2. `planning/tech_debt_2026-09-16/EXECUTION_SUMMARY.md` — what the two
   previous programmes did and their protocol lessons.
3. `planning/PROGRESS.md` — the current status entry point (33 planning
   packages are referenced from it; those are the "active" candidates).

## Rulings in force (operator, 2026-09-16/17)
- rs_cam is in early development. No legacy support of any kind; breaking
  changes are fine. Delete, do not shim.
- Obsolete planning material is DELETED from the tree, not archived. Before
  the first deletion, create an annotated tag on the pre-purge commit so
  every deleted document stays retrievable:
  `git tag -a planning-pre-purge-2026-09-17 -m "planning tree before the structure purge; git show <tag>:planning/<path> retrieves any file"`.
  Record the tag and the retrieval command in `planning/CLAUDE.md` and in a
  new `planning/DELETED_INDEX.md` (one line per deleted package: name, what
  it decided, why it is obsolete). Decision rationale must remain findable
  through that index.
- The other session (power-calcs) is DOWN during this programme, so
  `crates/rs_cam_core/src/feeds/**` and `tool_load/**` may be moved but
  their internals must not be edited; check `git status` for foreign
  uncommitted files before every wave anyway.
- Every product surface mutates `ProjectSession` through
  `ProjectSession::apply(Command)`. Core stays independent of GUI concerns.
- No large test gates (`--features heavy-tests` full core is forbidden).

## P1 — planning purge (no Rust; one agent; do this first)
Measured 2026-09-17: `planning/` holds 1 414 tracked files, 1.2 GB, 154
markdown files at its top level, 200 package directories, 74 files under
`planning/archive/`; tracked blobs over 5 MB include
`planning/airrun_2026-08-19/p2_a1_lakes_vbit_chk4.html` (905 MB), two 22 MB
tarballs under `planning/data_ingest_2026-05-30/`, and
`fixtures/debug_adaptive/wanaka_diag.json` (220 MB).
1. Produce a keep/delete manifest FIRST (`planning/structure_2026-09-17/P1_MANIFEST.md`)
   and commit it before deleting anything. KEEP: `PROGRESS.md`, `CLAUDE.md`,
   `TECH_DEBT_REGISTER.md`, `FEATURE_CATALOG.md` (root), every package that
   `PROGRESS.md` or a crate `CLAUDE.md` names as active or as a sentry's
   evidence, the two 2026-09-16 programme directories, and any package whose
   status file says open. DELETE: closed / superseded / retired packages,
   `planning/archive/` in full, `probe_artifacts`, `gcode_current_outputs`
   unless a test reads them (`rg -l "planning/" crates/*/tests` decides —
   any path a test reads is KEEP), every blob over 5 MB (the tests that read
   `fixtures/debug_adaptive/wanaka_diag.json`, if any, decide its fate).
   For each deleted package write the one-line rationale into
   `planning/DELETED_INDEX.md`.
2. Tag, then `git rm -r` the delete set, one commit per group (reviews,
   ui packages, airrun/measurement campaigns, data ingest, archive), each
   commit body listing the packages. Fix every dangling link in kept
   documents (`rg -n "planning/<deleted>"`), including the memory-style
   pointers in `PROGRESS.md`.
3. Collapse the 154 top-level markdown files: each either stays (active),
   moves into its package, or is deleted with an index line.
4. History rewrite is NOT part of this programme (the 905 MB blob stays in
   history). Record in the summary that a `git filter-repo` pass is an
   operator decision for a moment when no session and no clone is live.
Gate: `rg -n "planning/" crates/*/tests crates/*/src` shows no path that no
longer exists; `cargo test -p rs_cam_core -q --test <each sentry that reads a
planning path>` green; the root `CLAUDE.md` table still points at existing
files.

## P2 — core module grouping (Rust; run after P1; one or two agents)
Measured: `crates/rs_cam_core/src/lib.rs` declares 123 top-level modules and
115 `.rs` files sit directly in `src/`; existing folders (`compute`,
`session`, `feeds`, `tool_load`, `dexel_stock`, `adaptive3d`, `adaptive`,
`gcode`, `diagnostics`, `tool`, `metrology`, `material`) are fine.
1. Write the target layout first (`planning/structure_2026-09-17/P2_LAYOUT.md`)
   and commit it: every root file assigned to a folder such as `geometry/`
   (geo, mesh, grid2, enriched_mesh, …), `ops/` (scallop, pencil, ramp_finish,
   spiral_finish, unified_finish, project_curve, crease_paths, …),
   `stock/` (dexel*, simulation, simulation_cut, …), `maps/` (tier_map*,
   reach_map*, memo, grid, geom_cache, finish_surface_cache), `io/`
   (io, step_input, svg_input, dxf_input, tool_library, machine_library,
   named_toml_library, artifact_io, …), `trace/` (debug_trace,
   semantic_trace, toolpath_spans, …), `viz/`-free. Leave `feeds/` and
   `tool_load/` in place. Get the layout ratified by the operator before
   moving (one message with the table).
2. Move with `git mv`, one folder per commit; update `mod` declarations and
   every `crate::` path (`rg`, then compiler); NO re-export shims at the old
   paths (ruling). Update `planning/AGENT_CODEMAP.md`, the crate `CLAUDE.md`
   module tables, and the 79 source-scanning sentries listed in
   `planning/tech_debt_2026-09-16/evidence/source_scanning_sentries.txt`
   whose needles are file paths (programme 1's only defect was a sentry
   naming a moved site). `cargo fmt` is expected to reformat moved files;
   that is fine inside the move commit.
3. Gate per commit: `scripts/cargo_lane.sh check --workspace --all-targets`,
   `scripts/cargo_lane.sh test -p rs_cam_core -q --lib`, the path-scanning
   sentries, `scripts/cargo_lane.sh clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all -- --check`. Known pre-existing reds to ignore: viz
   controller `..._ur3`, `simulation_staleness_tracks_edits`,
   `freshness_does_not_outrank_a_collision`; viz dc6
   `off_workspace_run_producers_hold_their_recorded_ruling_ur3`; core f036b
   `modulation_raises_cutting_chipload_toward_band`.
4. Same treatment for viz only if `ui/` (39k lines) has a flat-root problem;
   measure first, propose, do not assume.

## P3 — root tidy (trivial; one commit)
Untracked strays: `EEPROM.DAT`, `profile.json.gz`, five `screenshot_*.png`
(ask before deleting an untracked file the operator may want; otherwise add
to `.gitignore`). Tracked: `G13_PROMPT.md` (no references) and
`AGENT_PROMPT.md` (two references — check them) — delete or move into
`planning/`; `demos/` and `reference/` hold zero tracked files — delete the
directories; `review/` (106 tracked files) sits beside `planning/review_*` —
decide with the P1 manifest; `toolpath_stress_test/`, `research/`,
`architecture/`, `tests/` (4 files) — keep only what a crate or doc
references, else delete with an index line.

## Protocol (from the two previous programmes; follow exactly)
- Cargo ONLY through `scripts/cargo_lane.sh <args>` (serialises cargo on
  this machine; concurrent cargo jobs crash the PC; waits can be minutes —
  use a 600000 ms timeout or run in the background).
- Never `git add -A`, never a bare `git commit` (a peer's staged `git rm`
  gets swept in): `git commit <explicit paths> -m …`. Never
  `git reset --hard`, never workspace-wide `cargo test`.
- Commit each item the moment it gates. Agents have been cut off by the
  usage limit with 1 400 uncommitted lines.
- Parallel agents get disjoint file sets; for P2 one agent per folder group
  is fine because `git mv` sets are disjoint, but only ONE agent may edit
  `lib.rs` and the sentry files — assign that explicitly.
- Prose in Simplified Technical English (root `CLAUDE.md`). Commit messages
  end with the harness attribution line.
- Finish with an Opus read-only completeness review, an
  `EXECUTION_SUMMARY.md` in `planning/structure_2026-09-17/`, and the same
  before/after numbers as above (tracked planning files and bytes, root
  files in core `src/`, top-level modules in `lib.rs`).
