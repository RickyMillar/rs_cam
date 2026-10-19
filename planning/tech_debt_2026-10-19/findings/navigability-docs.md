# Findings: navigability and documentation debt (`navigability-docs`)

Baseline: evidence 99b6fb41, checkout HEAD cf9b7ff3, branch `geo-select`.
Method: `rg` and file reads only. No cargo run. Each claim cites `path:line`.
"In flux" means the path is in `instruments/in_flux_paths.txt`.

The findings are in tier order. Tier B comes first.

---

## N1 (B). Folder `CLAUDE.md` rules that the code already breaks

**Claim.** Seven folder instruction files state a rule as true. The code
breaks each rule now. An agent that trusts the rule writes wrong code or
makes a wrong review decision. No test or type holds any of these rules.

| Rule (file:line) | Code that breaks it |
|---|---|
| `crates/rs_cam_viz/src/ui/CLAUDE.md:29` "Every Run Simulation affordance uses `simulation_request_is_buildable`" | `crates/rs_cam_viz/src/ui/menu_bar.rs:229-232`: the Simulation menu button pushes `AppEvent::RunSimulation` with no check. Only `readiness_panel.rs:118` and `sim_op_list.rs:50` call the predicate. |
| `crates/rs_cam_viz/src/render/CLAUDE.md:22` "A colour belongs in `colors.rs`. Do not write a literal colour in a pipeline." | `crates/rs_cam_viz/src/render/toolpath_render.rs:228-230,471,628` write literal RGB arrays. |
| `crates/rs_cam_cli/CLAUDE.md:31` "Do not hardcode a preset in a subcommand." | `crates/rs_cam_cli/src/nc_replay.rs:51-60` `builtin_machine()` hardcodes `shapeoko_xxl_ricky_tuned()` and feed caps `4000.0` / `10_000.0`. |
| `crates/rs_cam_core/src/finish/CLAUDE.md:30` "Every finishing link under a ceiling takes `surface_link::shape_under_ceiling` (B16)" | `crates/rs_cam_core/src/finish/pencil/emission.rs:504,792`: pencil links use `plan_link_lift`. `:775` marks the chord refinement "G-TIERBURIAL, open". |
| `crates/rs_cam_core/src/compute/CLAUDE.md:19` "`SelectionInputs` is private to `prologue.rs`, C3" | `crates/rs_cam_core/src/compute/execute/prologue.rs:58` `pub struct`; `crates/rs_cam_core/src/compute/execute.rs:566` `pub use prologue::{.., SelectionInputs}`; `execute.rs:331` `pub selection` on `ExecutionContext`. |
| `crates/rs_cam_core/src/session/CLAUDE.md:19` "`walk_output_dependents` is pure" | `crates/rs_cam_core/src/session/mutation/toolpath.rs:331` takes `&mut self` and calls `drop_result` on the dirty set. |
| `crates/rs_cam_core/src/dressup/CLAUDE.md:8` names `adaptive_feed_modulate` as the dressup that runs in `session/compute.rs` | Production calls `adaptive_feed_modulate_with_materials` (`crates/rs_cam_core/src/session/compute.rs:2080`). `adaptive_feed_modulate` (`dressup/feed_modulation.rs:809`) has only test callers. |
| `crates/rs_cam_cli/src/command.rs:15-16,29` "the three CLI call sites share one entry point"; "all fifteen CLI sites" | `rg -c "apply_command\("` gives 26 call lines in five files. `crates/rs_cam_cli/src/project.rs:309` and `crates/rs_cam_cli/src/smoke.rs:751` call `session.apply` directly. |
| `crates/rs_cam_core/src/geometry/boundary.rs:32-33` "`effective_boundary_reported`, which is what every production call site uses" | `crates/rs_cam_core/src/session/compute.rs:146` calls plain `effective_boundary`. The comment at `:121-122` says the clip refuses on the same failure, so the result is safe. The doc claim is false. |

**Note on selection.** The `SelectionInputs` row is in flux. It is not a
property of the unfinished selection feature. It is a visibility claim
that a contributor trusts today, so it counts as debt.

**Enforcement now.** DOC, and the doc is false.

**Fix.** Make each rule true or delete it. Then give each surviving rule
the strongest guard that fits:

1. `SelectionInputs`: make it `pub(in crate::compute::execute)` and the
   field `pub(crate)`. TYPES.
2. Run Simulation: route every button through one helper that takes the
   predicate result. A source-scan sentry for `AppEvent::RunSimulation`
   outside that helper holds it. SENTRY.
3. `walk_output_dependents`: change the doc to "touches no simulation".
4. Render colours: extend the existing UP1 colour sentry to
   `render/`. SENTRY.
5. CLI preset: take the default from `io::machine_library` or delete the
   rule. CLI doc counts: delete the numbers.
6. Pencil link and `effective_boundary`: correct the text.

Cost M. In flux: yes (`compute/CLAUDE.md`, `execute.rs`, `prologue.rs`,
`session/CLAUDE.md`, `mutation/toolpath.rs`, `session/compute.rs`,
`smoke.rs`).

---

## N2 (B). User-facing text says "Kc" after the Kc model was removed

**Claim.** `crates/rs_cam_core/src/material/CLAUDE.md:24` says "No scalar
`Kc`". The B6 power and deflection model reads `Material::force_line()`
(`tool_load/CLAUDE.md`, B6 line). The operator still sees "isotropic Kc"
in a gate detail string. Doc comments still describe a `Kc` product.

**Sites.**
- `crates/rs_cam_core/src/tool_load/deflection.rs:298` user-facing
  `"peak tip deflection {peak_um:.0} µm (isotropic Kc, bending only)"`.
- `crates/rs_cam_core/src/tool_load/deflection.rs:77` doc "material Kc is
  unavailable".
- `crates/rs_cam_core/src/tool_load/verdict.rs:39` doc "`Kc × DOC × WOC ×
  feed`"; `verdict.rs:225` example text "isotropic Kc only".
- `rg -c "\bKc\b"` in non-test core source gives hits in 17 files, for
  example `feeds/predict.rs` (3), `feeds/suggest/adaptive_entry.rs` (3),
  `session/compute.rs` (3), `material/mod.rs` (5). Some hits are
  history ("the old `Kc·ap·ae` aggregate"). Each hit needs a read.

**Enforcement now.** DOC (the material rule). Nothing checks the text.

**Fix.** Rewrite the detail string and the doc lines in force-line terms.
Add a sentry that refuses `Kc` in a non-comment string literal in
`tool_load/` and `feeds/`. SENTRY. Cost S. In flux: no.

---

## N3 (E). Folder file maps and symbol names point to the wrong place

**Claim.** The folder `CLAUDE.md` files are the entry point for an agent.
Many of them name a symbol that does not exist, name the wrong path, or
omit files. Nothing checks them. A sentry name is correct in every file
that the maps checked, so the drift is in file maps and symbol names.

**Symbols and paths that do not exist as written.**
- `crates/rs_cam_core/src/ops/CLAUDE.md:19` "Query
  `OperationType::honors_pinned_bottom_z()`". No such method. It is a
  field: `crates/rs_cam_core/src/compute/catalog/capabilities.rs:61`.
- `crates/rs_cam_core/src/ops/CLAUDE.md:4` `compute::execute_operation_annotated`.
  The function is `compute::execute::execute_operation_annotated` and is
  `pub(crate)` (`crates/rs_cam_core/src/compute/execute.rs:593`).
- `crates/rs_cam_core/src/tool/CLAUDE.md:10-11` names `CylCutter`,
  `BallCutter`, `BullCutter`, `BallConeCutter`. The types are
  `FlatEndmill`, `BallEndmill`, `BullNoseEndmill`, `TaperedBallEndmill`
  (`tool/flat.rs:15`, `ball.rs:15`, `bullnose.rs:20`,
  `tapered_ball.rs:24`). The old names survive only in the `//!` line 1 of
  the same four files.
- `crates/rs_cam_core/src/export/CLAUDE.md:3-4` "re-exports the four
  surfaces". `export/mod.rs:9-14` declares six modules and re-exports
  nothing.
- `crates/rs_cam_core/src/metrology/CLAUDE.md:4` "re-exports each ruler".
  `metrology/mod.rs:57-62` declares `pub mod` only.
- `crates/rs_cam_core/src/dressup/CLAUDE.md:23` "Every linking arm calls
  the shared `relink_fragments` kernel". No file in `dressup/` names
  `relink_fragments`.
- `crates/rs_cam_viz/src/ui/CLAUDE.md:3` "`ui::mod` runs once per frame".
  `ui/mod.rs` has no draw function; `app.rs` calls each surface.
- `crates/rs_cam_core/src/adaptive/CLAUDE.md:39` names
  `print_exact_width_along_move` like an API. It is a test function at
  `crates/rs_cam_core/tests/a_clearing_cut_holds_the_pass_load_g_adaptpassload.rs:857`.
- `crates/rs_cam_viz/src/ui/properties/pins/CLAUDE.md:9` "`PinDrillState`
  (one `PinDrillRow` per pin size)". It is an enum `NoPins | Sizes(..)`
  (`ui/properties/pins/mod.rs:66-70`).
- `crates/rs_cam_core/src/session/CLAUDE.md:32`
  `project_file::RETIRED_KEYS`. The const is private
  (`session/project_file.rs:85`).
- `FEATURE_CATALOG.md:23,32` cite `adaptive.rs` and `adaptive3d.rs`. Both
  are folders now.

**Files the maps omit.**
- `crates/rs_cam_core/src/compute/CLAUDE.md:5-6` omits
  `execute/clearing_2d.rs`, `curve_engrave.rs`, `dressup_apply.rs`,
  `drilling.rs`, `finish_3d.rs`, `finish_raster.rs`. `execute/shared.rs`
  appears only in a sentry line.
- `crates/rs_cam_core/src/session/CLAUDE.md:7-9` omits `rest_fresh.rs` and
  `compute/params.rs` (`grep -c` = 0 for each).
- `crates/rs_cam_viz/src/app/CLAUDE.md:10` omits `mcp/commands/obstacles.rs`
  (K1, landed 2026-10-16).
- `crates/rs_cam_viz/CLAUDE.md` has no row for the root files: `mcp_bridge.rs`
  (2067 lines), `mcp_server.rs` (1909), `app.rs` (1464), `ui_command.rs`
  (1067), `controller.rs`, `present_mode.rs`, `mcp_lifecycle.rs`,
  `host.rs`, `lib.rs`, `error.rs`: 8817 lines in all.
- `crates/rs_cam_viz/src/ui/properties/pins/CLAUDE.md` is not named by
  its parent `ui/properties/CLAUDE.md` (line 14 names `pins/` only).

**Enforcement now.** NONE. `rg -l "CLAUDE\.md" crates/*/tests` finds no
test that reads the instruction files.

**Fix.** Add one sentry in `crates/rs_cam_core/tests/`. It walks every
`CLAUDE.md` under `crates/`, extracts each backticked token, and:

1. checks that a token ending in `.rs` resolves to a file under the
   folder, the crate `src/`, or the repo root;
2. checks that a token of the form `Type::name` or `name()` matches
   `fn name`, `struct name`, `enum name` or a field `name:` in the crate;
3. checks that every `.rs` file in the folder appears in the folder file
   or in an explicit "not listed" line.

Fix the rows above in the same package. SENTRY. Cost M. In flux: yes
(`ops/CLAUDE.md`, `compute/CLAUDE.md`, `session/CLAUDE.md`).

---

## N4 (E). The 40-line limit is met by line width, not by size

**Claim.** The root `CLAUDE.md` says each folder file "is 40 lines or
fewer". The operator preference is the same. Some files keep 40 lines
and grow sideways. A long line is one run-on paragraph that holds many
rules and ticket codes.

| File | Lines | Longest line | Lines > 200 chars | Bytes |
|---|---|---|---|---|
| `crates/rs_cam_core/src/compute/CLAUDE.md` | 40 | 1592 | 14 | 9573 |
| `crates/rs_cam_core/src/feeds/CLAUDE.md` | 40 | 688 | 13 | 7378 |
| `crates/rs_cam_core/src/tool_load/CLAUDE.md` | 29 | 548 | 1 | 1972 |
| `crates/rs_cam_core/src/session/CLAUDE.md` | 40 | 383 | 4 | 3726 |
| `crates/rs_cam_core/src/ops/CLAUDE.md` | 40 | 285 | 3 | 3135 |

`compute/CLAUDE.md:22` is one bullet that holds the W6.2, W6.3, W6.4 and
W6.5 rules. A reader cannot find one rule in it.

**Enforcement now.** CONVENTION. Nothing measures the limit.

**Fix.** Measure size, not lines. Add a sentry: each folder file has at
most 40 lines, at most 120 characters for each line, and at most about
3 KB. Split `compute/CLAUDE.md` into `compute/CLAUDE.md` and
`compute/execute/CLAUDE.md`. SENTRY. Cost S. In flux: yes
(`compute/CLAUDE.md`, `session/CLAUDE.md`, `ops/CLAUDE.md`).

---

## N5 (E). Broken intra-doc links, and no rustdoc lint in the gate

**Claim.** Doc links name items that do not exist or that moved. The
build does not see them because no rustdoc lint is on and the quality
gates do not run `cargo doc`.

**Sites (read in this sweep).**
- `crates/rs_cam_core/src/session/mod.rs:1804,1815-1816` link
  `Self::edit_toolpath_selection`, `Self::rename_setup`,
  `Self::add_obstacle`, `Self::remove_obstacle`. `rg "pub fn (rename_setup|edit_toolpath_selection|add_obstacle)\b"`
  finds none. The rows are `Command` variants now.
- `crates/rs_cam_core/src/ops/waterline.rs:78` links
  `crate::finish::finish_setup::Z_LADDER_DEFAULT_EPSILON`. The const is
  `crate::ops::depth::Z_LADDER_DEFAULT_EPSILON` (`ops/depth.rs:52`).
- `crates/rs_cam_core/src/finish/finish_setup.rs:15` links `[`z_ladder`]`.
  `finish_setup.rs` does not import it; it is `ops/depth.rs:60`.
- `crates/rs_cam_core/src/diagnostics/mod.rs:33` and `:247` link
  `crate::diagnostics::diagnose_toolpath`. No such function exists in
  `diagnostics/`. The method is `ProjectSession::diagnose_toolpath`
  (`crates/rs_cam_core/src/session/compute/export.rs:101`).

**Known debt.** `planning/tech_debt_2026-09-16/TECH_DEBT_PLAN.md`, W4
residue item 3: `cargo doc --workspace --no-deps` gave 160
`private_intra_doc_links` and 95 `unresolved link` warnings. It was
recorded, not scheduled. What changed: nothing guards it yet.
`rg "broken_intra_doc|rustdoc::" Cargo.toml crates` finds no hit.

**Enforcement now.** NONE.

**Fix.**
1. Add `[workspace.lints.rustdoc] broken_intra_doc_links = "deny"` and
   `private_intra_doc_links = "warn"` to the root `Cargo.toml`.
2. Add `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` to
   the gate table in the root `CLAUDE.md`.
3. Fix the backlog in one package.

LINT. Cost S for the lint, M for the backlog. In flux: yes
(`session/mod.rs`).

---

## N6 (E). Doc comments and module docs describe a past state

**Claim.** Doc text states a type, a count, a field or a version that
the code no longer has. Module docs narrate a past phase as the present.
A reader takes the doc as the truth.

**Wrong type, field, count or version.**
- `crates/rs_cam_core/src/tool_load/verdict.rs:232` "`toolpath_id` is the
  core `usize` index". The field is `ToolpathId` (`:238`).
- `crates/rs_cam_core/src/compute/execute.rs:580` names a field
  `boundary_regions` on `ExecutionContext`. The struct
  (`execute.rs:318-403`) has no such field.
- `crates/rs_cam_core/src/compute/toolpath_stats.rs:4-5` "the nine finding
  types". `rg "^pub struct \w+Finding"` gives 12 in the file.
- `crates/rs_cam_core/src/material/mod.rs:556,767,1485` "~148 species".
  The library has 132 rows (`material/CLAUDE.md` says ~130).
- `crates/rs_cam_core/src/session/command.rs:1109` cites
  `session/mutation.rs:1339`. The file has 164 lines;
  `bump_all_revisions` is at `session/mutation/toolpath.rs:847`.
- `crates/rs_cam_core/src/finish/finish_setup.rs:1-4` says the file holds
  "the Z-ladder". It does not (see N5).
- `crates/rs_cam_viz/src/lib.rs:28` cites `egui-0.34.3`. The crate is on
  egui 0.36 (`crates/rs_cam_viz/Cargo.toml:16`).
- `crates/rs_cam_viz/src/app.rs:772` "the whole 6,146-line `app/mcp.rs`".
  The file is about 1080 lines after the split.
- `crates/rs_cam_viz/src/ui/theme.rs:8-10` says the module "shrinks" as
  call sites migrate to `tokens`. `rg -c "theme::"` gives 221 uses in 24
  files. The operator ruled "no legacy" (2026-09-16).

**Module docs that narrate a past phase.**
- `crates/rs_cam_core/src/session/mod.rs:1-2` "a future MCP server". The
  server exists.
- `crates/rs_cam_core/src/diagnostics/mod.rs:30-34` "The PR-2 landing
  (this commit) is plumbing only".
- `crates/rs_cam_core/src/compute/mod.rs:3-5` "Phase 1 ... Phase 3
  extracted from `rs_cam_viz`".
- `crates/rs_cam_core/src/lib.rs:3-17` lists six layers. It omits
  `session`, `selection`, `settings`, `budget`, `diagnostics` and
  `metrology`.

**Enforcement now.** NONE.

**Fix.** Correct each line. Replace a count or a type in prose with an
intra-doc link that N5's lint checks (for example `[`ToolpathId`]`).
Delete history paragraphs; `git log` holds them. LINT (through N5) for
names; CONVENTION for counts. Cost M. In flux: yes (`execute.rs`,
`toolpath_stats.rs`, `command.rs`, `session/mod.rs`, `lib.rs`, `app.rs`).

---

## N7 (D). `feeds/INTEGRATION.md` is a second, stale description of the feeds flow

**Claim.** `crates/rs_cam_core/src/feeds/INTEGRATION.md` describes the
GUI feeds flow. It is wrong, and it copies a table that the registry
owns.

- `:9` says the GUI entry is `crates/rs_cam_viz/src/ui/properties/mod.rs`.
- `:29` step 4 says the GUI calls `rs_cam_core::feeds::calculate`. `rg`
  finds no call in `crates/rs_cam_viz/src`. The one production caller is
  `crates/rs_cam_core/src/feeds/suggest.rs:1006`; the GUI goes through
  `feeds::suggest`.
- `:31-37` maps each GUI operation to a family and a role by hand. The
  registry (`compute/catalog/registry.rs`) holds that mapping.
- `feeds/CLAUDE.md` does not name `INTEGRATION.md`. `rg "INTEGRATION.md"
  crates` finds no reference.

**Enforcement now.** NONE.

**Fix.** Delete the file. Put a two-line pointer to `feeds::suggest` in
`feeds/CLAUDE.md`. CONVENTION (the N3 sentry then sees one map). Cost S.
In flux: no.

---

## N8 (E). No module-doc discipline and no `missing_docs` lint

**Claim.** 86 of 619 source files have no `//!` module doc. Coverage is
uneven: core 14 of 374, viz 70 of 224. The files without a module doc
include the largest viz facades. No crate has a doc lint.

**`lib.rs` / `mod.rs` / `main.rs` without `//!` (lines).**
`crates/rs_cam_viz/src/ui/properties/mod.rs` (1619),
`crates/rs_cam_viz/src/render/mod.rs` (1610),
`crates/rs_cam_viz/src/controller/events/mod.rs` (1035),
`crates/rs_cam_viz/src/ui/properties/operations/mod.rs` (749),
`crates/rs_cam_cli/src/main.rs` (730),
`crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (641),
`crates/rs_cam_viz/src/compute/mod.rs` (621),
`crates/rs_cam_viz/src/state/mod.rs` (549),
`crates/rs_cam_viz/src/ui/mod.rs` (377),
`crates/rs_cam_viz/src/lib.rs` (228),
`crates/rs_cam_viz/src/io/mod.rs`, `crates/rs_cam_viz/src/interaction/mod.rs`,
`crates/rs_cam_mcp/src/lib.rs`.

`rs_cam_mcp` is the wire crate that other surfaces read. Its `lib.rs` has
two `pub mod` lines and no doc.

**Enforcement now.** NONE. `rg "missing_docs" Cargo.toml crates` finds no
hit.

**Fix.**
1. Add `#![warn(missing_docs)]` to `rs_cam_mcp` first. The clippy gate
   runs with `-D warnings`, so the warning blocks the gate.
2. Add a sentry that each `lib.rs` and `mod.rs` starts with `//!`.
3. Extend `missing_docs` to `rs_cam_core` one folder at a time.

LINT + SENTRY. Cost M. In flux: yes (`ui/properties/mod.rs`).

---

## N9 (E). Live product code sits in a file named `research`

**Claim.** `crates/rs_cam_core/src/finish/CLAUDE.md:17` defines research
code as the `--features research` modules. `finish/scallop/research.rs`
is not gated (`finish/scallop.rs:18` `mod research;`), and it holds the
live iso-field entry `scallop_toolpath_iso_field_with_cancel`
(`research.rs:84`). The product path calls it:
`crates/rs_cam_core/src/compute/execute/finish_raster.rs:119`.
`finish/CLAUDE.md:23` says the iso-field dial is live. A contributor who
gates or deletes "research" code breaks the Scallop operation.

**Enforcement now.** NONE. The name says the opposite of the truth.

**Fix.** Move the live entry points into `scallop/iso_field.rs`. Keep
only the research-only builders in `research.rs` behind
`#[cfg(feature = "research")]`. Then the compiler refuses a product call
into research code. TYPES. Cost S. In flux: yes (`finish_raster.rs`).

---

## N10 (F). `path:line` citations in comments rot

**Claim.** Comments cite source by line number. Lines move with each
edit. `rg -o "[A-Za-z0-9_/]+\.rs:[0-9]+" -g '*.rs' crates` gives 389
citations. At least 34 point past the end of the in-repo file they name,
so they are stale for sure. (One more hit matched an external eframe
path by basename and is not counted.) Others can point to a wrong line
inside the file; this sweep did not check those.

**Past end of file, production source.** One row:
- `crates/rs_cam_core/src/session/command.rs:1109` → `mutation.rs:1339` (164 lines).

**Past end of file, tests (sample).**
- `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs:32` → `core/compute/catalog.rs:2749` (`compute/catalog.rs` has 1480 lines).
- `crates/rs_cam_core/tests/disconnected_finish_retract_structure_p0.rs:122,131` → `compute/execute.rs:2472,2492` (678 lines).
- `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs:31,77,110,549` → `mutation.rs:236..1328` (164 lines).
- `crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs:63,70,81,106,216,724` → `scallop.rs` / `compute/execute.rs` past end.
- `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs:130` → `ui/properties/mod.rs:3666` (1619 lines).
- `crates/rs_cam_core/tests/span_summary_single_pass_c1.rs:103` → `rs_cam_viz/src/app/mcp.rs:4578` (1081 lines).

**Enforcement now.** CONVENTION, none written.

**Fix.** Cite a symbol, not a line: an intra-doc link in `src/`, a
`` `module::item` `` name in tests. N5's lint then checks the `src/`
links. Add a rule to the root `CLAUDE.md`. LINT (for `src/`), CONVENTION
(tests). Cost M. In flux: yes (`session/command.rs`).

---

## N11 (F). Code cites purged `planning/` paths, and some are not in the purge tag

**Claim.** Production source has 165 references to 106 distinct
`planning/…` paths that do not exist. The root `CLAUDE.md` and
`planning/CLAUDE.md` say this is expected: `git show
planning-pre-purge-2026-09-17:<path>` gets the file. That rule fails for
some citations, because those files left the tree before the tag.

**Not in the tag.**
- `planning/adaptive_review_2026-04.md`:
  `crates/rs_cam_core/src/adaptive3d/path.rs:110,653`,
  `adaptive3d/clearing.rs:1194`, `stock/simulation_cut/analysis.rs:132`.
- `planning/acceptance_loop/findings/F-035-predicted-feed-in-gates.md` and
  `F-036b-feed-modulation-flag-plumbing.md`:
  `crates/rs_cam_core/src/session/mod.rs:1077,1133`. Commit 0d559359
  dropped `acceptance_loop` before the purge.
- `planning/P3_TRANSIT_PEAK_DOC_RCA.md`:
  `stock/simulation_cut.rs:205`, `stock/simulation_cut/accumulate.rs:117`,
  `trace/toolpath_spans.rs:575`. Commit 784f902d moved it to
  `planning/archive/`; only `trace/toolpath_spans.rs:176` uses the
  archive name.
- `planning/DELETED_INDEX.md` does not name any of these files.

Most cited (missing): `planning/unified_v3_design.md` (19 sites),
`planning/unified_finish_planner_design.md` (10),
`planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` (9).

**Enforcement now.** DOC (`planning/CLAUDE.md` retrieval rule).

**Fix.** For a citation that holds a reason, copy the reason into the
comment in one sentence and keep the path as provenance. Fix the four
irretrievable paths to a commit hash. Add a sentry: a `planning/` path in
`src/` exists on disk or in the tag. SENTRY. Cost M. In flux: yes
(`session/mod.rs`).

---

## N12 (F). Large files and long functions

**Claim.** 66 source files are over 1500 lines. The longest functions
are hard to read in one pass:

- `crates/rs_cam_core/src/feeds/mod.rs:1494-2821` `calculate`, about 1330 lines.
- `crates/rs_cam_viz/src/app/gpu_upload.rs:155-1496` `upload_gpu_data`, about 1340 lines.
- `crates/rs_cam_viz/src/app/mcp/commands.rs:2031-2932` `describe_core`, about 900 lines.
- Files over 3000 lines: `compute/simulate.rs` 3361, `tool_load/verdict.rs`
  3343, `adaptive/path.rs` 3334, `compute/operation_configs.rs` 3302,
  `rs_cam_mcp/src/server.rs` 3277, `gcode/mod.rs` 3230,
  `rs_cam_viz/src/app/mcp/commands.rs` 3196, `adaptive3d/clearing.rs`
  3186, `feeds/mod.rs` 3091, `dexel_stock/stamping.rs` 3081,
  `polygon.rs` 3010.

**Known debt.** `planning/TECH_DEBT_REGISTER.md` T-5 named
`feeds/mod.rs` (4144 lines) and `feeds/suggest.rs` (5667). What changed:
`suggest.rs` is 1067 lines now, `feeds/mod.rs` is 3091, and `calculate`
is still one function of about 1330 lines. `rs_cam_mcp/src/server.rs`
grew from 2857 (map) to 3277 on this branch.

**Enforcement now.** NONE.

**Fix.** Enable `clippy::too_many_lines` with a high threshold (for
example 400) in `clippy.toml`, and allow the current sites by name with
a `// SAFETY:`-style reason. New long functions then fail the gate.
Split `calculate` into named steps. LINT. Cost L. In flux: yes
(`gpu_upload.rs`, `app/mcp/commands.rs`, `rs_cam_mcp/src/server.rs`).

---

## N13 (F). Two names for one place

**Claim.** Some names make a reader look in the wrong place.

- `crates/rs_cam_core/src/session/mutation.rs` (164 lines of helpers)
  sits beside the folder `session/mutation/` (5 files). Doc citations
  already confuse the two (N6, N10).
- `crates/rs_cam_core/src/dexel_stock/` is the engine;
  `crates/rs_cam_core/src/stock/dexel.rs` holds the dexel data types.
- `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs` (tab host) and
  `crates/rs_cam_viz/src/ui/toolpath_panel.rs` (rail).
- `crates/rs_cam_core/src/feeds/plunge.rs` (resolver) and
  `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs` (rule table).
- `crates/rs_cam_viz/src/ui/theme.rs` and `ui/tokens.rs`: two module names
  for one palette (N6).

**Enforcement now.** NONE.

**Fix.** Rename in one mechanical commit each: `session/mutation.rs` →
`session/mutation/mod.rs`; `feeds/extrapolation/plunge.rs` →
`plunge_rules.rs`; delete `theme.rs` and rewrite its 221 call sites to
`tokens::`. CONVENTION. Cost M. In flux: no for these paths.
