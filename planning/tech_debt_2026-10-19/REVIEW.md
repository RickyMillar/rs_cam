# Tech-debt review 2026-10-19 (rs_cam, baseline 99b6fb41, branch geo-select)

This file is the ranked tech-debt review for the operator. It is read-only
evidence. No agent changed code under `crates/` or `scripts/`, and no agent ran
cargo. The facts come from `rg` and file reads. Each finding cites `path:line`.

The full site lists are in `findings/<category>.md`. This file keeps the claim,
the main sites, the evidence, the fix and the verifier notes for each finding.

## 1. Summary

### 1.1 Counts

The review holds 171 primary findings. The finders filed 194 ids. The
verifiers merged 47 ids into a primary finding (alias index at the end of
section 4), which left 147. The completeness critic (`CRITIC.md`) added 24
findings, F195 to F218, after a check in code on 2026-10-19 (the critic
fold). The verifiers refuted no whole finding. They refuted 20 sub-claims,
and the fold refuted 5 critic sub-claims (section 7).

Tier counts. "As filed" uses the finder tier. "As verified" uses the lower tier
where the two verifiers split. One reader checked each fold finding, so its
two counts are the same.

| Tier | Meaning | As filed | As verified |
|---|---|---|---|
| A | live defect | 5 | 4 |
| B | broken contract, or a pattern a contributor WILL break | 48 | 44 |
| C | dead code that reads as load-bearing | 11 | 11 |
| D | drift-prone duplication | 26 | 27 |
| E | weak or inconsistent pattern, no break yet | 64 | 68 |
| F | size, style, navigation | 17 | 17 |

Contested tiers (the finder tier is kept in the ranked table and marked):
F014 (A or B), F054 (B or E), F070 (B or D), F095 (B or E), F126 (B or C),
F183 (B or E).

Category counts (primary findings, finder tier):

| Category | Count | A | B | C | D | E | F |
|---|---|---|---|---|---|---|---|
| enforcement-gap | 22 | 1 | 13 | 0 | 0 | 8 | 0 |
| duplication | 17 | 0 | 3 | 0 | 11 | 1 | 2 |
| error-handling | 15 | 0 | 5 | 1 | 0 | 8 | 1 |
| architecture-layering | 14 | 1 | 6 | 0 | 1 | 6 | 0 |
| navigability-docs | 14 | 0 | 2 | 0 | 1 | 6 | 5 |
| inconsistency | 14 | 0 | 2 | 0 | 6 | 4 | 2 |
| primitive-newtype | 13 | 0 | 3 | 1 | 3 | 6 | 0 |
| bool-string-params | 12 | 1 | 2 | 2 | 1 | 5 | 1 |
| illegal-states | 12 | 2 | 5 | 0 | 0 | 5 | 0 |
| dead-code | 9 | 0 | 0 | 6 | 0 | 2 | 1 |
| complexity-size | 9 | 0 | 2 | 0 | 1 | 3 | 3 |
| test-debt | 9 | 0 | 4 | 1 | 1 | 2 | 1 |
| encapsulation-visibility | 8 | 0 | 1 | 0 | 1 | 5 | 1 |
| outside the 13 categories (runtime cost) | 3 | 0 | 0 | 0 | 0 | 3 | 0 |

Verdicts: 147 CONFIRMED, 24 PLAUSIBLE. Known debt: 30 findings carry a
`known_id` (register row, 2026-09-16 plan row or a design-audit row). 141 are
new. In flux: 116 findings cite at least one path in
`instruments/in_flux_paths.txt`. Selection programme: 4 findings (section 5).
Cost: 75 S, 79 M, 17 L. The fold added 17 S, 6 M and 1 L to the earlier
figures (58 S, 73 M, 16 L); the earlier figures have no stated rule for a
compound cost such as "S then L".

The fold (F195 to F218) by tier: B F197, F199, F201, F207; C F204; D F198,
F200, F211, F214; E F195, F202, F203, F205, F206, F208, F209, F210, F212,
F215, F216, F217, F218; F F196, F213.

### 1.2 The five most important themes

1. **Fields listed by hand.** Keys, digests, replies and patches name struct
   fields one by one. A new field compiles and is then missing. One live data
   loss follows from this (F176: a library Save deletes a different tool).
   MCP `get_diagnostics` drops `verdicts`, and MCP cannot set
   `StockConfig.padding`. The fix is an exhaustive destructure with no `..`.
2. **Tokens parsed with a silent default.** Post, datum, CLI strategy and
   `--skip` parsers turn an unknown token into a default. Two of these change
   emitted G-code with no message (F028 wrong dialect, F015 wrong Z zero), and
   one emits a toolpath the operator asked to skip (F014). The fix parses once
   at the boundary into the enum, and the loader warns or refuses.
3. **Fail-open dispatch on enums.** Hand `ALL` arrays, `_ =>` arms and a trait
   default (`geometry_hint` gives `Flat`) let a new variant compile and take a
   fallback. `ids::ALL` already misses 8 diagnostic ids. During this review
   commit e76f82c8 added a ninth miss, and 16c1b78b closed it (F162).
   G-BULLCUSP shipped through this shape. The fix generates each list from one row and makes each
   per-variant decision an exhaustive match.
4. **Two surfaces, two answers.** The GUI, MCP and CLI each derive the
   verdict, the export pipeline, the Apply-Suggest funnel and the
   pre-generation refusals. MCP says `OK` over holder collisions (F113). The
   CLI export skips the rapid rewrite and the machine-safety pass (F067). The
   fix moves each answer into one core door that every surface calls.
5. **Unchecked domains at the doors.** Tool geometry, op dials and project-file
   keys enter with no check. A bad V-bit angle panics the GUI thread (F029).
   107 op params publish no domain (F185). A stepover of 0 gives an empty
   toolpath or a panic (F030). The fix checks once at every door, then moves
   to refined types that a kernel cannot receive wrong.

### 1.3 How much the compiler enforces

Two independent instruments agree at about one in three. The ten area maps
grade 153 rows in their section 5 tables. The counting rule: take the first
grade cell of each section 5 row, and exclude a row with no single grade
(`maps/core-feeds.md` "n/a", the mixed "(a) TYPES; (b) ..." row in
`maps/viz-ui.md`). `maps/core-finish.md` has two tables in section 5; both
count. 50 rows (33 %) are TYPES, 6 LINT, 30 SENTRY, 13 DOC, 50 CONVENTION and
4 NONE. The 16 cold-start traces touched 370 sites, and the compiler forced
120 of them (32 %). TYPES is strongest in `maps/core-feeds.md` (7 of 13 rows)
and weakest in `maps/viz-ui.md` (1 of 13 rows). `maps/core-compute.md` has 5
TYPES of 14 rows. The forced share is lowest for tasks that add a field or a value to
an existing shape (tasks 2, 4, 5, 7: 2 to 5 forced sites of 14 to 21). Those
tasks fail because no `match` names fields. Most fixes in this review move a
row from CONVENTION or DOC to TYPES with one of three tools: an exhaustive
destructure, a generated variant list, or a private field with a checked
constructor.

### 1.4 Decisions requested from the operator

1. F015: on an unknown `z_datum`, must export refuse, or warn and default?
2. F033: must an inverted height stack refuse at export?
3. F151: fix `ci.yml` (trigger on master, per-crate commands), or delete it
   and say "no CI" in the two docs?
4. F176: do `name` and `stickout` make a library tool distinct? The safe
   default is yes.
5. F034: may the loader refuse every unknown key (`deny_unknown_fields`) now,
   under the no-legacy ruling?

Next action: start W1 and W3 (without F033) in parallel (section 3). W1
touches almost no in-flux path. W3 is partly in flux (`tab_badges.rs`,
`app/mcp/project.rs`). Start W2 when the selection session stops its edits of
cli `job.rs` (a working-tree edit exists now). The wave conflicts are in
section 3.1.

## 2. Ranked table (above the cut line)

Rule for the cut line. Above the cut: every A, every B (contested tiers
marked), C where a dead door reads as load-bearing, and D where the copies
already disagree. E and F findings appear above the cut only as members of a
theme row. All other findings are in section 6.

Rank order: tier, then how likely a contributor without context breaks it,
then cost. "TH-" rows are themes: one type-level change fixes the members. The
fix column gives the interim grade and the end grade where a verifier split
the fix in two steps.

| Rank | Id | Tier | Cost | Category | Claim | Proposed fix (enforcement) | Verdict | New / known | In flux |
|---|---|---|---|---|---|---|---|---|---|
| 1 | F176 | A | S | enforcement-gap | A library Save overwrites a different tool: `dedupe_key` names 7 of 20 `ToolConfig` fields | Build the key by an exhaustive destructure with no `..`; report the replaced tool name (TYPES) | CONFIRMED | new | no |
| 2 | TH-PARSE: F028, F015, F090, F104, F192, F202 | A | S-M | illegal-states, bool-string | An unknown post token emits GRBL; an unknown Z datum zeroes at the stock top; aliases and hand token lists remain; `$341` is a raw `u32` in core (F202) | Parse once at the boundary into the enum; `from_key -> Option`; load warning or refusal; token lists from `ALL` (TYPES) | CONFIRMED | new; CMP-15 (face_up only), L15 unchanged | yes |
| 3 | F014 | A (contested: B) | S | bool-string | `--skip` drops a bad token and emits that toolpath; a `--modulation-strategy` typo runs ConstrainedMax | clap `ValueEnum` mirrors; `Vec<usize>` with `value_delimiter` (TYPES) | CONFIRMED | new | no |
| 4 | TH-VERDICT: F113, F117, F033, F189 | A | M | architecture | MCP verdict says `OK` over holder collisions; GUI and core stepover rules differ; two height-order rules; Warnings tab reads 2 of 3 buckets | Interim: holder arm in the MCP ladder (S). End: core `ProjectVerdict`, one stepover predicate, one height-order fn, `TriageClass`; surfaces match exhaustively (TYPES) | CONFIRMED (F117 PLAUSIBLE) | new; L11 | F113 no; F117, F033 yes (F033 waits for selection P3, section 3.1) |
| 5 | F029 | A | M then L | illegal-states | MCP `set_tool_param {included_angle: 0}` panics the GUI thread on the next draw; unsaved work is lost | Step 1: one `ToolConfig::validate` at every door, cutter constructors return `Result` (SENTRY). End: `ToolShape` enum, `serde(try_from)` (TYPES) | CONFIRMED | new | yes |
| 6 | TH-DOMAIN: F185, F163, F030, F036, F191, F023, F210, F206 | B | M (+L for refined types) | enforcement-gap, illegal-states | 107 numeric op params publish no domain; feed 0 is accepted; the range is checked only in `set_toolpath_param`; `horizontal_finish` divides by stepover 0; 31 literal GUI ranges are the only domain for tool, stock, head, shoe and machine fields (F210); the derive crate has no test (F206) | Derive emits `compile_error!` for an unbounded numeric field unless `unbounded = "reason"`; one `validate_operation` at AddToolpath, ReplaceToolpathConfig and load; `ParamKind` const (SENTRY then TYPES) | CONFIRMED (F163 PLAUSIBLE) | new (DR-LIVE residue) | yes |
| 7 | TH-FIELDLIST: F181, F118, F166, F187, F065, F101, F048, F197, F209 | B | L | enforcement-gap | MCP replies, wire patches, cache keys and digests list fields by hand; `verdicts` and `padding` never reach MCP; the GUI triage cache can serve a stale answer; four hand copies of the stale-trace rewrite (F197, T-21 class); `project_entry_onto` copies 16 entry fields by hand (F209) | Serialize the core report as the base object; exhaustive destructure for projections and keys; core `StockConfigPatch`; wire enums for fixed tokens (TYPES) | CONFIRMED | new | yes |
| 8 | TH-WILDCARD: F162, F161, F182, F183, F194, F095, F016, F017, F215 | B (F183, F095 contested: E) | M-L | enforcement-gap | Hand `ALL` arrays (`ids::ALL` misses 8 ids); `_ =>` per-variant decisions; `geometry_hint` defaults to Flat; dressup owner tables are wildcard string matches; span kinds are free strings | `ALL` generated by one macro row; required trait method; exhaustive matches on capabilities; `DressupKey` and `DebugSpanKind` enums (TYPES); per-folder wildcard lint (LINT) | CONFIRMED / PLAUSIBLE | new; CUT-13 | yes |
| 9 | TH-APPLY: F096, F040, F115, F126, F031, F056, F120 | B (F126 contested: C) | M-L | inconsistency, architecture | `AppState.session` is pub and 21 viz sites apply directly; MCP arms do not clear the simulation; multitool and modulation mutate outside `apply`; a run record says Done with no result | Private `session` plus `AppState::apply -> Applied` (`#[must_use]`); Commands for multitool and modulation; delete `apply_stale_default_fix`; `ToolpathRun` enum (TYPES) | CONFIRMED (F056 PLAUSIBLE) | new | yes |
| 10 | TH-EXPORT: F067, F116, F051, F070, F061, F121, F072 | B (F070 contested: D) | M-L | duplication, architecture | Only GUI and MCP rewrite rapids and run the machine-safety pass; the CLI job exports with an empty load report and its own cutter builder; the advisory note is dropped | One core export door returns `ExportedGcode { gcode, machine_safety, load_outcome }`; delete `build_tool`; `From<PassRole>` (TYPES) | CONFIRMED (F116 PLAUSIBLE) | new | yes |
| 11 | TH-FEEDS: F114, F002, F042, F025, F006 | B | M | architecture, primitive | Two Apply-Suggest funnels (drill defaults only on the CLI); the advisor modulates against the travel rate; the "one write" doc is false | Core `Command::ApplyFeeds` with one funnel; `CuttingCeilingMmMin` newtype; pass `ApplyScope`; route `suggest_for_operation` through `apply` (TYPES) | CONFIRMED (F042 PLAUSIBLE) | §15 ruling 4; T-18 re-broken; T-11 class | partly |
| 12 | TH-LOAD: F053, F034, F186 | B | M | error-handling, illegal-states | A missing `tool_id` binds to tool 0; an unknown key loads as the default; twin stock and tool structs type defaults twice | Required fields; `deny_unknown_fields` after a flatten/tag audit; persist `StockConfig` directly (TYPES) | CONFIRMED | new | yes |
| 13 | F032 | B | M | illegal-states | The fit-to-model flag rule has four hand copies; the MCP copy differs and recreates the R12 state | Interim: reconcile in core `set_stock_config` (SENTRY). End: `StockSize` enum (TYPES) | CONFIRMED | new | yes |
| 14 | F132 | B | M | complexity-size | `upload_gpu_data` is 1327 lines; each slot builds its key apart from its rebuild inputs; c84ef3be broke this class | Per-slot `Inputs` value: key and build read the same value (TYPES) | CONFIRMED | new | yes |
| 15 | F128 | B | M | complexity-size | The dressup order lives in the statement order of a 470-line fn; `DRESSUP_PIPELINE` is read only by a test | `DressupStep` enum, const `PIPELINE`, exhaustive match per arm (TYPES, keep sentry) | CONFIRMED | new | yes |
| 16 | F188 | B | L | enforcement-gap | "None means not measured" is prose; U2 and X-VAC broke it | `Measured<T>` with a reason and no `unwrap_or` accessor (TYPES) | CONFIRMED | new | no |
| 17 | TH-ERR: F054, F087, F055, F063, F110 | B (F054 contested: E) | L | error-handling | Typed errors become `String` at each layer; viz reads the cancel flag; `VizError` has dead variants; `MissingGeometry` has three meanings | `SessionError::Operation(#[from])`; `collision_check -> Ok(NotApplicable)`; delete dead variants and `From<String>` (TYPES) | CONFIRMED | new | yes |
| 18 | TH-FRAME: F001, F013 | B | S then L | primitive-newtype | One `P3` type for three frames; four frame defects in August 2026; selection holds two frames in one type | Step 1: rename `local_to_global`, typed `AlignmentPin` (DOC). End: `Pt<F>` at the session/compute boundary, `ResolvedSelection<F>` (TYPES) | CONFIRMED | new | F013 yes |
| 19 | TH-ID: F004, F111, F038 | B | L | primitive-newtype | Id newtypes are optional; an index and an id are both `usize` in Args; `get_` means index and id | `ToolId` into `ids.rs`, private fields, typed record and Args fields; `*_at` and `*_by_id` names (TYPES) | CONFIRMED | R3 | yes |
| 20 | TH-TESTGATE: F151, F152, F153, F155, F171, F156, F204 | B | S-M | test-debt | CI triggers on `main`; parked reds; the f026 sentry is vacuous; gate tests load `planning/` and home paths; cross-crate scans run in no viz gate; a viz test module does not compile in the no-`mcp` build (F204) | Fix the trigger or delete `ci.yml`; ignore-reason sentry; panic on a missing fixture; fixture door; viz scans into `rs_cam_viz/tests` (SENTRY) | CONFIRMED (F156 PLAUSIBLE) | T-2, T-3 | partly |
| 21 | TH-DOCS: F138, F173, F144, F140, F142 | B (F173 at the bottom of B) | M | navigability-docs | Seven folder CLAUDE.md rules that the code breaks; literal colours in render pipelines; stale `feeds/INTEGRATION.md` | Make each rule true or delete it; `RunSimulation` buildable token; `Rgb` newtype; CLAUDE.md path sentry; rustdoc lint (TYPES / SENTRY / LINT) | CONFIRMED (F142 PLAUSIBLE) | W4 residue 3 | yes |
| 22 | TH-LINT: F170, F172, F027, F195 | B | M | enforcement-gap | The `// SAFETY:` rule has no enforcer (438 of 693 allows lack it); no `#[expect]`; no design lint; 398 numeric `as` casts and no cast lint (F195) | Deny `allow_attributes_without_reason` and `allow_attributes`; `unsafe_code = "forbid"`; wildcard lint per folder; `cast_possible_truncation` per folder (LINT) | CONFIRMED | Q7-Q12 | no |
| 22a | F199 | B | S | error-handling | Three production callers use the region-extraction forms that hide the 64-region cap; on wanaka the plain form dropped 502 of 566 islands | Keep only `region_polygons_from_mask_reported`; `#[must_use]` on `RegionCapReport` (TYPES) | CONFIRMED | new | partly (`session/multitool.rs`) |
| 22b | F201 | B | M | navigability-docs | Typed-in cutting speeds, plunge baselines and DOC/WOC factors drive the formula feeds; the root and `material/` rules forbid a number with no printed source | Each constant cites its source in the repo, or the formula path refuses; a provenance sentry over the tables (SENTRY) | CONFIRMED | new (T-16 sibling) | no |
| 22c | F207 | B | S | error-handling | An MCP reply that is not JSON is classified as success; a handler that refuses in plain text gets a success toast | Every toast-bearing arm states its `McpOutcome`; no text fallback (TYPES) | PLAUSIBLE | new | yes |
| 23 | TH-TESTDOOR: F085, F086, F088, F059, F091, F047, F005, F020, F093 | C | M | dead-code | Metrology (2663 lines), the adaptive3d wrapper chain, test doors, dead pub items and dead flags read as product API | Delete, or gate behind `test-support` / `research` with `required-features` rows (TYPES for the default build) | CONFIRMED | S1 hold ownerless | partly |
| 24 | TH-DUPSMALL: F074, F080, F103, F075, F078, F082, F200 | D | S each | duplication | Labels, UnmodeledReason text, mirror-enum maps, cutter width, descent floor and deflection inputs each have two or more copies; some already differ; the air / low-engagement bounds are typed at six sites (F200) | One method or fn per fact, in core (single source; TYPES where it is an exhaustive match) | CONFIRMED | T-20 | partly |
| 25 | F102 | D | M | inconsistency | Engine `Params` defaults differ from config defaults (Pencil, Project Curve, Ramp Finish) | Delete the engine `Default`, or derive it from `XConfig::default()` (TYPES) | CONFIRMED | new | yes |
| 26 | F071 | D | S then L | duplication | Two simulation-group builders; the rapid feed is resolved four times and one copy lacks `.max(1.0)` | `PostConfig::rapid_feed_mm_min` now (S); core `simulation_request` later (TYPES) | CONFIRMED | D11 residue | yes |
| 27 | F076 | D | M | duplication | Point-to-segment and quantile copies use different edge rules | Call `geo::point_to_segment_distance`; add a `_sq` twin; stats module returns `Option` (CONVENTION + Option) | PLAUSIBLE | D10 (six copies since) | yes |

Rows 22a, 22b and 22c came from the critic fold. They take a letter so that
the R1 to R27 references in section 4 keep their numbers.

**Cut line.** Rows below this line are in section 6.

## 3. Proposed waves

A wave is one file set. Two parallel editors take waves with disjoint file
sets. "After Wn" means the waves share a file and run in sequence.

In-flux rule. 116 findings touch a path in `instruments/in_flux_paths.txt`.
These paths are all in flux and need coordination with the
geometry-selection session (rs-cam-f6) before an editor starts:
`session/mod.rs`, `session/project_file.rs`, `session/save.rs`,
`session/command.rs`, `session/compute.rs`, `session/compute/simulation.rs`,
`session/compute/export.rs`, `session/mutation/config.rs`,
`session/multitool.rs`, `gcode/mod.rs`, `compute/config.rs`,
`compute/operation_configs.rs`, `compute/validate.rs`,
`compute/execute/dressup_apply.rs`, `diagnostics/ids.rs`, `selection/*`,
`rs_cam_mcp/src/server.rs`, viz `controller/events/mod.rs`,
`app/mcp/commands.rs`, `ui/properties/operations/registry.rs`,
`ui/properties/tab_badges.rs`, `app/gpu_upload.rs`, cli `run.rs`, `job.rs`,
`smoke.rs`. During the review the tabs work landed (e76f82c8, 16c1b78b):
`compute/catalog/schema.rs`, `compute/execute.rs`,
`compute/operation_configs.rs`, `dressup/mod.rs`, `dressup/tabs.rs` and
`diagnostics/ids.rs`. On 2026-10-19 the working tree shows selection P4.1
edits in `session/*`, `session/selection_status.rs`, cli `job.rs`,
`rs_cam_mcp/src/server.rs`, viz `mcp_bridge.rs`, `mcp_server.rs`,
`app/mcp.rs`, `app/gpu_upload.rs`, `app/viewport.rs`,
`controller/events/mod.rs`, `ui/mod.rs` and `ui/properties/tab_badges.rs`
(`git status`).

| Wave | Rows | File set | In flux | Order |
|---|---|---|---|---|
| W1 Tool model | 1, 5, F182 (row 8), F075 (row 24), F210 tool part (row 6) | `io/tool_library.rs`, `compute/tool_config.rs`, `tool/*.rs`, `compute/cutter.rs`, `session/compute/params.rs` (set_tool_param), viz `ui/properties/tool.rs`, `ui/tool_library_modal.rs` | partly: the F029 GUI-thread callers in `app/gpu_upload.rs` and the add_tool check in `rs_cam_mcp/src/server.rs` are in flux; the `helix_entry_diameter_mm` site in `compute/config.rs` is in flux, so leave it to W8 | first |
| W2 CLI boundary | 3, CLI part of 2, F057, F116 step 1 (`build_tool`) | cli `main.rs`, `run.rs`, `job.rs`, `smoke.rs`, `nc_replay.rs` | yes (`run.rs`, `job.rs`, `smoke.rs`); 89df3283 changed `job.rs` (22 lines) and the working tree has one more edit | after the selection session stops its `job.rs` edits; then parallel with W1, W3. The `job.rs` lines in F004, F028, F032, F113, F116 and F118 are at the baseline: add 8 to a line in 233-1157, 7 to a line in 1158-1194, 16 to a line after 1195 (for example F028 `:370` is now `:378`, F116 `:854` is now `:862`) |
| W3 Verdict surfaces | 4 (F113 interim first, F189; F033 moves after selection P3, see 3.1), F181 reply part of 7, F048 (row 7), 16 | viz `controller/events/compute.rs` (build_mcp_diagnostics), cli `project.rs` summary, core `session/diagnostics_types.rs`, `diagnostics/adapters/from_static_checks.rs`, `stock/sim_triage.rs`, viz `ui/sim_diagnostics.rs`, `ui/properties/tab_badges.rs`, `app/mcp/project.rs`, `app/mcp/simulation.rs` | partly (`tab_badges.rs`, `app/mcp/project.rs`; 89df3283 changed `project.rs` by 34 lines) | parallel with W1 |
| W4 Project file and tokens | core part of 2 (with F202), 12, F091 | `session/project_file.rs`, `session/save.rs`, `session/mod.rs` (datum parsers), `gcode/mod.rs` (token fns only), `material/mod.rs`, `machine/mod.rs` from_key | yes: coordinate with rs-cam-f6 | after W2 |
| W5 Export door | 10, F051 CLI flag, F197 (row 7), F214 | `gcode/mod.rs` (export fns), `export/gcode_validator.rs`, viz `io/export.rs`, `io/setup_sheet.rs`, `session/compute/export.rs`, `tool_load/mod.rs`, `feeds/vendor_normalize.rs`, cli `project.rs` export | yes | after W4 and W3 |
| W6 Param domains | 6 (F206 derive tests first; F210 GUI ranges for stock, head, shoe and machine) | `rs_cam_derive/src/lib.rs`, `compute/operation_configs.rs`, `compute/catalog/schema.rs`, `compute/catalog.rs`, `session/compute/params.rs`, `session/mutation/toolpath.rs`, `session/command.rs` (ReplaceToolpathConfig), `finish/horizontal_finish.rs`, viz `ui/properties/linking_dressup.rs` (dv) | yes: `operation_configs.rs` and `schema.rs` changed in e76f82c8 and 16c1b78b | after W1 and after F206 (see 3.1) |
| W7 Enum lists and dispatch | 8 (minus tool and dressup parts), F117 GUI stepover part of 4 | `diagnostics/ids.rs`, `feeds/suggest/apply.rs`, `feeds/suggest/axial_envelope.rs`, viz `ui/properties/operations/shape_diagrams.rs`, `operations/registry.rs`, `operations/validate.rs`, `app/viewport.rs`, `ui/properties/feeds_speeds.rs`, the fm0 and viz `state/job.rs` count tests | yes (`ids.rs` has live edits; `registry.rs`) | after W3 |
| W8 Dressups | 15, F016 and F017 (row 8) | `compute/config.rs`, `compute/execute/dressup_apply.rs`, viz `ui/properties/linking_dressup.rs`, `ui/sim_debug.rs`, `app/mcp/diagnostics.rs`, `rs_cam_mcp/src/server.rs` span examples | yes: wait for the tabs-dressup session to land | after W6 |
| W9 Viz apply door | 9, 13, F209 and F215 (rows 7, 8), F217 | viz `state/mod.rs`, `controller/events/*`, `app/mcp/commands.rs`, `ui/properties/stock.rs`, core `session/multitool.rs`, `session/compute/simulation.rs`, `session/command.rs` (AdoptSimulation), `compute/validate.rs`, `session/mutation/config.rs` | yes: coordinate; blast radius L (see 3.1) | after W3, W6 and after selection P4 |
| W10 Feeds funnel | 11, 22b (F201) | `feeds/suggest.rs`, `feeds/suggest/*`, `feeds/provenance.rs`, `feeds/quantities.rs`, `stock/simulation_cut.rs`, `machine/mod.rs`, `dressup/feed_modulation.rs`, `session/compute.rs`, `session/command.rs`, viz `controller/events/mod.rs`, `ui/feeds/*` | yes | after W7, W9 |
| W11 Caches and GPU slots | 14, F187, F065, F101 (row 7) | `session/compute/diagnostics.rs`, `session/load_report.rs`, `compute/simulate.rs`, `compute/sim_prefix.rs`, `compute/source_stock.rs`, `compute/toolpath_stats.rs`, `machine/head.rs`, viz `state/simulation/issue_triage.rs`, `app/gpu_upload.rs`, `render/upload_cache.rs` | yes (`gpu_upload.rs`) | after W4 |
| W12 Errors | 17, 22a (F199) | `session/mod.rs` (SessionError), `compute/execute.rs` (OperationError), `session/compute/*` flatten sites, viz `error.rs`, `io/import.rs`, `controller/io.rs` | yes | after W4, W10 |
| W13 Frames and ids | 18, 19 | `compute/transform.rs`, `compute/alignment_pins/*`, `ids.rs`, `compute/tool_config.rs` (ToolId), session records and Args, `diagnostics/mod.rs` Scope, `selection/resolve.rs`, viz id users, cli `job.rs` | yes: after the selection programme lands | last but one |
| W14 Tests, CI, docs | 20 (with F204), 21 | `.github/workflows/ci.yml`, `crates/rs_cam_core/tests/CLAUDE.md`, core `Cargo.toml` comments, the cited test files, folder `CLAUDE.md` files, viz `render/*.rs` colours, `ui/menu_bar.rs`, `feeds/INTEGRATION.md` | partly (some fixtures) | parallel with any wave that does not edit the same CLAUDE.md |
| W15 Dead code and small duplicates | 23, 24 (with F200), 25, 26, 27 | the cited files only; each deletion goes in the wave that owns its file if that wave is open | partly | in the owning wave, else after W12 |
| W16 Lint policy | 22 (with F195) | `Cargo.toml` lint table and every file with an `allow(...)` | no | last: it touches every file |
| W17 MCP wire and replies | F118, F166 (row 7), 22c (F207) | `rs_cam_mcp/src/server.rs`, viz `app/mcp/commands.rs`, `app/mcp.rs`, `mcp_bridge.rs`, `compute/stock_config.rs`, `session/command.rs` (patch args) | yes: selection P4 step 5 adds MCP rows to the same files (`geometry_selection_2026-10-16/PLAN.md:206`) | after selection P4 |
| W18 Frame cost | F216, F218 | viz `ui/properties/toolpath_panel.rs`, `ui/menu_bar.rs`, `ui/setup_panel.rs`, `ui/properties/machine_panel.rs`, `ui/properties/model_sim_panels.rs`, `app/input.rs`, `app/export.rs` | partly (`toolpath_panel.rs`) | after selection P4 (the toolpath panel is in flux) |

File-set additions from the critic fold: W4 `machine/mod.rs` (`$341`), W5
`gcode/post.rs`, W6 viz `ui/properties/{stock,head,shoe_row,machine_panel}.rs`
and `rs_cam_derive/tests/`, W9 viz `ui/properties/mod.rs`
(`project_entry_onto`, `write_entry_config_to_session`), W10 `material/mod.rs`
and `feeds/mod.rs` (factor table), W12 `geometry/region_mask.rs`,
`finish/finish_planner.rs` and `session/multitool.rs`, W14 viz `src/lib.rs`,
W15 `stock/sim_triage.rs`, `stock/simulation_cut/*`, `tool_load/power.rs` and
`trace/narrate.rs`.

Below-cut fold findings with no wave: F196, F198, F203, F205, F208, F211,
F212, F213. The wave that owns the file takes each one as a side effect.

### 3.1 Wave conflicts

The critic found seven places where two fixes, or a fix and the geometry
selection programme, edit the same logic. Each entry gives the conflict and
its resolution.

1. **F033 against selection P3.** F033 changes the height-order logic at
   `compute/config.rs:379,398` and `session/compute/export.rs:59`. P3 step 1
   moves heights resolution to the setup frame
   (`geometry_selection_2026-10-16/PLAN.md:179-180`). P4.1 also adds a face
   auto top for an op with no slot (`session/CLAUDE.md`, P4.1 line).
   Resolution: F033 leaves W3. F033 waits for the geometry-selection height
   work (P3, which waits on operator D9, `PLAN.md:384`). P3 may
   own F033. W3 keeps F113, F117, F189 and F181.
2. **TH-APPLY (W9) against selection P4.** A private `AppState.session`
   changes every viz reader. A scan of non-test viz files (with the
   `#[cfg(test)]` tail cut) finds 599 `.session` tokens in 58 files; the
   critic counted 435 in 55. `state/mod.rs:135` holds `pub session`. P4 adds
   viewport picks and MCP parity in the same files. Resolution: W9 runs after
   P4, and its cost is L, not M-L.
3. **TH-DOMAIN (W6) against selection P5 and P5b.** P5 (e76f82c8) and P5b
   (16c1b78b) landed. The P5 scalar params carry ranges
   (`compute/operation_configs.rs`: `tab_count` `0.0..=20.0`, `tab_width`
   `1.0..=50.0`, `tab_height` `0.5..=20.0`), so they do not trip the proposed
   `compile_error!`. The new `tab_points: Vec<[f64; 2]>` is a list with no
   domain; its snap rule refuses a far point at generation. Resolution: the
   W6 rule states how a list param is treated. Add the derive tests (F206)
   first, then W6.
4. **F029 `ToolShape` (W1) against F182 (W7).** The wave table already puts
   F182 in W1, and W7 excludes the tool parts. Resolution: inside W1, land
   F176 first (S, tier A), then F029 step 1 (validate at every door). Land
   the `ToolShape` end state and F182 in one commit, and update the F176
   destructure in that commit.
5. **F015 (W4) against F091 (W15).** F015 proposes `from_key -> Option` for
   `Material::from_key` (`material/mod.rs:1692`) and
   `MachineProfile::from_key` (`machine/mod.rs:678`). F091 deletes
   `Material::from_key`. Only tests call either fn (`material/mod.rs:2260,2334`
   and `tests/literature_matrix/shim.rs:101`). Resolution: F091 moves into W4.
   W4 deletes both fns and moves the material key table into the shim. W4
   retypes only the datum parsers.
6. **TH-LOAD `deny_unknown_fields` (W4) against selection P4 and P6.** P4
   and P6 add project-file keys. Operator ruling 5 (section 9) settles the
   decision: the loaders refuse every unknown key. Resolution: run the
   flatten and tag audit, and switch on `deny_unknown_fields`, after P4 and
   P6 stop adding keys. Until then W4 does the parser half of TH-PARSE only.
7. **TH-LINT `#[expect]` (W16) against TH-TESTDOOR feature gates (W15).** An
   `#[expect(dead_code)]` on an item that one feature set uses gives
   `unfulfilled_lint_expectations` in that set. The full-lint command uses
   three features; the per-crate test commands use the defaults (root
   `CLAUDE.md:85`, Quality gates). Resolution: W16 runs after W15, and checks
   both feature sets. An item that is live in only one set keeps
   `#[allow(.., reason = "..")]` (the F170 verifier note).

## 4. Findings by category

Each entry gives: tier, cost, verdict, known id, in flux, and the ranked row
(R1 to R27 and R22a to R22c, or "below cut"). Then the claim, the main sites, the evidence, the
fix (current grade to proposed grade) and the verifier notes. The verifier
notes keep only the verdict, the corrections, any re-tier or re-cost and the
refined fix. The full site list of each finding is in
`findings/<category>.md`.

The four selection-programme findings (F013, F050, F094, F127) are in
section 5. The critic-fold entries (F195 to F218) hold their full site lists
themselves; their source is `CRITIC.md` sections 1, 2 and 5, and no
`findings/*.md` row exists for them.

### 4.1 primitive-newtype

#### F001 One P3 type for three coordinate frames
B · L · CONFIRMED · new · not in flux · R18
- Claim: World, stock-relative and setup-local points all have the type `P3`.
  `local_to_global` is not the inverse of `world_to_local`. Four frame
  defects shipped in August 2026; one put the retract plane inside the stock.
- Sites: `compute/transform.rs:273,288,301`, `compute/alignment_pins/mod.rs:57`,
  `compute/config.rs:253`.
- Evidence: the doc at `transform.rs:276-280` says `local_to_global` does not
  re-add the stock origin. Both doors are `fn(&self, P3) -> P3`. Commits
  6bcc3c97, 9df20910 (G-SAFEZ-LOCAL), f5afdeca (G-PINDRILL-FRAME), 3b182398.
  `AlignmentPin {x, y}` is a bare stock-relative pair.
- Fix: DOC → TYPES. `Pt<F>` with a `PhantomData` frame marker (World,
  StockRel, SetupLocal) at the session/compute boundary; kernels keep `P3`.
- Verifier: both confirm. One counts 28 production reads of `stock_origin_*`
  outside `transform.rs`, the other 31. `config.rs:253` is weakly related.
  A cheap first step (S): rename `local_to_global` to `local_to_stock_rel`,
  add a real `local_to_world`, and type `AlignmentPin`. The typed point works
  only if the bare-`P3` doors become private.

#### F002 max_feed_mm_min names two quantities; the advisor modulates against the travel rate
B · S · CONFIRMED · T-18 (closed d47a04d8 for four sites; this site is new) · in flux · R11
- Claim: `MachineProfile.max_feed_mm_min` is the travel rate. Other structs
  use the same name for the cutting ceiling. The advisor candidate path gives
  the travel rate to the modulator as its cutting cap.
- Sites: `session/compute.rs:1680,2064`, `session/compute/simulation.rs:520`,
  `dressup/feed_modulation.rs:289,613`, `machine/mod.rs:191,481`,
  `feeds/explain_payload.rs:50`, cli `nc_replay.rs:72`, viz `ui/feeds/why.rs:269`.
- Evidence: `machine/mod.rs:182-191` doc says TRAVEL rate. Production uses
  `cutting_feed_ceiling_mm_min()` at `simulation.rs:520`. `why.rs:269` names
  the wrong field to the operator.
- Fix: DOC → TYPES. `TravelRateMmMin` and `CuttingCeilingMmMin` with private
  fields, made only by `MachineProfile` methods. `ModulationContext` takes the
  ceiling type.
- Verifier: both confirm. The path ranks strategies only, so B, not A. Interim
  one-line fix: `compute.rs:1680` reads `cutting_feed_ceiling_mm_min()`. Give
  `LinkKinematics`, `MachineEnvelope` and `ReplayMachine` the ceiling type. A
  field rename breaks shipped machine TOMLs: update them or use `serde(rename)`.

#### F004 The id newtypes are optional; core records and commands carry raw usize ids
B · L · CONFIRMED · R3 (review 2026-06-10) · in flux · R19 · also F108
- Claim: `LoadedModel.id`, `SetupData.id` and `ToolpathConfig.tool_id/model_id`
  are raw `usize`. `SetToolpathToolArgs` and `SetToolpathModelArgs` hold an
  index and an id as two `usize` values (the R3 class).
- Sites: `session/mod.rs:380,742,808,814,1797,1836`, `session/command.rs:1425,1438`,
  `ids.rs:22-37`, `compute/tool_config.rs:8`, `diagnostics/mod.rs:108`,
  `gcode/mod.rs:124`, `maps/reach_map.rs:327`, viz `ui_command.rs:294`,
  `compute/mod.rs:71`, `ui/mod.rs:151`, cli `job.rs:641`.
- Evidence: `ToolConfig.id` is `ToolId`, so tool identity has two spellings.
  `Scope::Setup/Tool {id: usize}` sits beside `Scope::Toolpath {id: ToolpathId}`.
  Past breaks 7226703d (B7) and 3d7b0d35 (R3).
- Fix: CONVENTION → TYPES. Move `ToolId` into `ids.rs`, make the inner fields
  private, let only allocators and serde mint ids, type the record and Args
  fields, delete `find_*_by_raw_id`.
- Verifier: both confirm. The re-wrap count is unstable (69, 80, 98, 119 by
  method). A newtype serializes as its inner value without
  `serde(transparent)`, so that gap is cosmetic. Order: move `ToolId` and add
  derives (S); then Args and Scope (the R3 swap surface); then records.
- Changed since R3: 3d7b0d35 fixed one swap; the raw `usize` Args that allow
  the swap remain.

#### F005 Feedrate and SafeZ newtypes are dead; the G-code IR is bare f64 / u32
C · S · CONFIRMED · new · not in flux · R23 member
- Claim: `post.rs` defines `Rpm`, `Feedrate` and `SafeZ` to stop unit mixing,
  but `SafeZ` has no use and the `Statement` IR holds bare `u32` and `f64`.
- Sites: `gcode/post.rs:22,34,45`, `gcode/ir.rs:24,53`.
- Evidence: only `PostLimits` uses the types (`post.rs:69-75`). `SafeZ` appears
  only in the re-export `gcode/mod.rs:17`.
- Fix: NONE → TYPES. Use `SafeZ` in `Statement::SafeZRetract`, `Feedrate` in
  the feed fields and `Rpm` in `Preamble`/`SpindleSet`, with private fields.
- Verifier: confirm. Keep `SafeZ` (it fits `SafeZRetract`); a pub tuple field
  closes no gap. The real unit mix is upstream in the toolpath.

#### F006 The feeds quantity newtypes stop at the gate; the source records stay bare
D · M · CONFIRMED · T-11 class (closed 809d28b9) · not in flux · R11 member
- Claim: `feeds/quantities.rs` exists because chip thickness was compared with
  an advance-per-tooth band. The sim sample, a second band type and about 31
  hand `feed/(rpm*flutes)` sites still use bare `f64`.
- Sites: `stock/simulation_cut.rs:181,183`, `dressup/feed_modulation.rs:134`,
  `feeds/quantities.rs:75,127`, `feeds/operating_point.rs:348`,
  `feeds/suggest/aggressiveness.rs:196`, viz `ui/feeds/compare.rs:46`,
  `ui/feeds/explore.rs:1170`.
- Fix: TYPES at the gate only → TYPES at the source. Type the sample fields,
  delete `feed_modulation::ChiploadBand`, add `AdvancePerToothMm::feed_at`.
- Verifier: confirm. The current grade is NONE/DOC at the source, not TYPES.
  A checked `new` cannot stay `const fn` if it returns `Result`. Many sites
  hold rpm as `f64`. Delete the `compare.rs:46` helper first.

#### F007 2D boxes and points as anonymous [f64; 4] / tuples
D · M · PLAUSIBLE · new · in flux · below cut
- Claim: boxes travel as `[f64; 4]` or 4-tuples with the order in a doc line.
- Sites: `polygon.rs:186,1750`, `adaptive/material_grid.rs:655`,
  `surface/rest_field.rs:189`, `adaptive3d/mod.rs:178`,
  `surface/dropcutter.rs:153`, `selection/item.rs:534`.
- Fix: CONVENTION → TYPES. `BoundingBox2 { min: P2, max: P2 }`.
- Verifier: plausible. A 2D box type exists (`trace/debug_trace.rs:18`
  `ToolpathDebugBounds2`). Two production `polygon_bbox` copies are identical;
  `region_mask.rs:344` and `finish_planner.rs:1354` are test copies.
  `ring_aabb` returns `[NAN; 4]` on purpose; the new type must keep that.
  Cheap step: delete the `material_grid.rs:655` copy. Overlaps F133.

#### F008 Angles: degrees in configs, radians in kernels, no suffix on either
E · M · CONFIRMED · new · not in flux · below cut
- Claim: configs take degrees, V-bit kernels and the B-rep mesh take radians;
  neither has a suffix or a type.
- Sites: `ops/vcarve.rs:26`, `ops/inlay.rs:64`, `ops/chamfer.rs:39`,
  `geometry/enriched_mesh.rs:51,156`, `compute/tool_config.rs:175`,
  `compute/execute/curve_engrave.rs:344`, `finish/steep_shallow.rs:42`.
- Fix: DOC → TYPES. `Degrees`, `Radians`, `From<Degrees> for Radians`.
- Verifier: confirm. `vbit_half_angle` is not the only door: the same
  conversion is hand-written at `tool/vbit.rs:41`, `feeds/mod.rs:191`,
  `feeds/geometry.rs:506`, `ops/drill_op.rs:103`, viz `render/sim_render.rs:972`.
  Move it to one `ToolConfig::vbit_half_angle() -> Radians`; do not delete it.

#### F009 cut_summary_metrics returns two air-cut percentages as a bare tuple
E · S · CONFIRMED · T-2 (related) · in flux · below cut
- Claim: the function returns `Option<(f64, f64, f64, f64)>`; a swap compiles.
- Sites: `trace/narrate.rs:2166,2073`.
- Fix: → TYPES. A named struct (S), or two newtypes from `AirCutRatios` (M).
- Verifier: confirm. One private caller. D7 was a choice-of-reading defect, not
  a tuple swap. Current grade is CONVENTION/DOC, not SENTRY.

#### F010 Grid kernels take &[T] + rows, cols beside Grid2
E · M · CONFIRMED · new · not in flux · below cut
- Claim: grid kernels take a slice plus `rows, cols`; callers swap `nx, ny` by
  hand.
- Sites: `geometry/grid_field.rs:79,128,181`, `finish/steep_shallow.rs:91`,
  `finish/finish_planner.rs:764`, `geometry/region_mask.rs:194`,
  `finish/unified_finish.rs:1725`.
- Fix: CONVENTION → TYPES. Take `&Grid2<T>`.
- Verifier: confirm. Four public kernels plus two private, not six public.
  `Grid2` has 91 references in 8 files, not 42 in 4. `Grid2::new_fill(nx, ny)`
  moves the swap to construction; add `Grid2::from_spec`.

#### F011 DiagnosticId(pub String) and other pub-field newtypes accept any value
E · S · CONFIRMED · new · not in flux · below cut
- Claim: a misspelt id in `supersedes` compiles and never matches.
- Sites: `diagnostics/mod.rs:78`, `diagnostics/supersession.rs:45`,
  `diagnostics/adapters/from_tool_load.rs:384`, `stock/material_slot.rs:32`,
  `trace/toolpath_spans.rs:26`, `diagnostics/evidence.rs:17`.
- Fix: CONVENTION → TYPES.
- Verifier: confirm. `&'static str` does not work with `Deserialize`. Generate
  a fieldless `enum DiagnosticId` from the ids list (ties into F162), or keep a
  private `Cow<'static, str>` with `TryFrom` checked against `ids::ALL`.
  `MaterialSlot::new -> Option`.

#### F012 Units live in names only; ad-hoc / 60.0; long runs of f64 parameters
E · L (M for the scoped fix) · PLAUSIBLE · Q10 (2026-09-16, below cut) · not in flux · below cut
- Claim: about 1500 fields carry a unit only in a name suffix; 49 hand `60.0`
  factors; 84 fns take four or more adjacent `f64`.
- Sites: `machine/kinematics.rs:1286,1516`, `machine/kinematic_utilization.rs`,
  `compute/tool_config.rs:171`, `finish/steep_shallow.rs:155`,
  `dexel_stock/simulation.rs:323`.
- Fix: CONVENTION → TYPES. `MmPerMin` and `MmPerSec` in kinematics first.
- Verifier: plausible. Cited sites true; aggregate counts not reproduced.
  Q10 covers the f64 runs, not the units. Name the params struct per
  signature (e.g. `emit_steep_passes` takes the steep-shallow params).

#### F198 The G-code IR carries modal state as text
D · M · CONFIRMED · new · not in flux · below cut
- Claim: coolant (M7/M8/M9), cutter compensation (G40/G41/G42) and other
  modal words enter the IR as `Statement::Raw(String)`. The builder then
  scans its own text to find them again.
- Sites: `gcode/ir.rs:44-47` (the `Raw` doc lists M5, M3, M6, M7/M8/M9,
  G40, G41/G42); 14 production `Statement::Raw` pushes in
  `gcode/program_builder.rs` before the test module at `:850` (for example
  `:178,222,655,845`); the text scanners `gcode/emitter.rs:125`
  `unsupported_mcode_in_line` and `gcode/program_builder.rs:481`
  `dangerous_modal_words`.
- Evidence: the tests find a coolant stop by text
  (`program_builder.rs:1346,1546,1565` `Statement::Raw(t) if t == "M9\n"`).
  The IR already has typed `ToolChange` (`ir.rs:43`), `Dwell` (`ir.rs:80`),
  `Preamble` and `SpindleSet` (`ir.rs:24,28`), so the pattern exists.
- Fix: CONVENTION → TYPES. `Coolant(CoolantMode)` and `CutterComp(..)`
  variants; `Raw` only for operator snippets. Sibling of F005.
- Fold note: the critic counted 20 pushes; 6 of them are in the test
  module. `export/gcode_validator.rs:571` `rule_word_set` also scans words,
  but a validator of emitted text must read text: it is not part of this
  finding. Map: `core-io-output.md` lead 7.

### 4.2 bool-string-params

#### F014 CLI strategy and skip flags accept any string and fall back in silence
A (contested: B) · S · CONFIRMED · new · not in flux · R3
- Claim: `--modulation-strategy` is a free `String`; a typo runs
  ConstrainedMax. `--skip` drops tokens that do not parse, so the toolpath the
  operator asked to skip is emitted.
- Sites: cli `main.rs:250,284,516,523,529`.
- Evidence: `main.rs:523-528` `_ => ConstrainedMax`; `main.rs:516-522`
  `filter_map(|s| s.trim().parse().ok())`.
- Fix: NONE → TYPES. clap `ValueEnum`; `#[arg(value_delimiter = ',')]
  skip: Vec<usize>`; delete alias spellings.
- Verifier: both confirm. One keeps A on `--skip`; one re-grades B (a typo on
  the command line, not a fault for correct input). The `spindle_strategy`
  `_ => None` is unreachable (clap `value_parser`): duplication only.

#### F015 Setup datum parsers fail open; CMP-15 fixed only face_up/z_rotation
B · S · CONFIRMED · CMP-15 (covered face_up/z_rotation only) · in flux · R2 · also F097, F165, F178, F026
- Claim: `ZDatum/XYDatum/Corner::from_key` map an unknown token to the
  default, and `offset:<garbage>` to `FixedOffset(0.0)`, with no load warning.
  A misspelt `table` becomes `StockTop`, which shifts the emitted program Z0.
- Sites: `session/mod.rs:613,662,705`, `session/project_file.rs:1050`,
  `gcode/mod.rs:310,1488,1608`, `session/save.rs:16`,
  `material/mod.rs:1692`, `machine/mod.rs:678`.
- Evidence: `project_file.rs:1050-1066` has no warnings argument;
  `parse_face_up`/`parse_z_rotation` warn. `post_format_or_grbl` turns an
  unknown post token into Grbl. `Material::from_key` and
  `MachineProfile::from_key` have only test callers. `save.rs` `tool_type_to_key`
  copies `ToolType::serde_token`.
- Fix: NONE → TYPES. `from_key -> Option<Self>`; refuse a bad offset; add
  `UnknownXyDatum`/`UnknownZDatum` load warnings; or type the setup section
  fields as serde enums.
- Verifier: both confirm. A Z datum fallback is more dangerous than a face_up
  fallback: the operator should rule whether export refuses on an unknown
  `z_datum`. A serde `deserialize_with` cannot push warnings: deserialize to
  `String` and resolve in the loader. Split the 11-name token-method naming
  into an E follow-up.

#### F016 Dressup field vocabulary is string tables with wildcard arms; CUT-13 fold incomplete
B · M · CONFIRMED · CUT-13 / CMP-17 (feed_optimization holdout reason now stale) · in flux · R8 member · also F035, F168, F179
- Claim: `owner_of_value_field` and `is_dressup_enabled` are `&str` matches
  ending `_ => None` / `_ => false`. A dressup missing from them lets
  `set_dressup_field` accept a value while the dressup is off; the value is
  then dropped.
- Sites: `compute/config.rs:898,920,931,1060,1191,1220`,
  `session/mutation/config.rs:92`, viz `ui/properties/linking_dressup.rs:282`,
  `ui/properties/toolpath_panel.rs:1149`, `mcp_server.rs:1342`.
- Evidence: `linking_dressup.rs:283` says `total = 8` but counts 7 items and
  never counts `segment_merge`; the GUI has no `segment_merge` control. The
  `feed_optimization` holdout comment cites a `tool_load` reader that does not
  exist.
- Fix: CONVENTION → TYPES. A `DressupKey` enum, an `owner` column on
  `DressupFieldDef`, exhaustive matches; fold `feed_optimization` into
  `Option<FeedOptParams>`; derive the GUI total from `DressupKey`.
- Verifier: both confirm. `session/mutation/tests.rs:214-243` tests one
  refusal, so "no test" is too strong; no test checks completeness. The GUI
  total is already wrong.
- Re-check after the tabs work (2026-10-19, working tree): the tabs are a
  Profile emit hook (`compute/execute/clearing_2d.rs`), not a dressup, so no
  dressup key was added. The `compute/config.rs` sites have no change since
  the baseline. The MCP site is the `set_dressup_field` description that
  lists the enable keys by hand: `mcp_server.rs:1301` at the baseline,
  `:1346` now (the cited `:1342` was the wrong line at the baseline too).

#### F017 Debug-span kind vocabulary is free strings in three crates and has drifted
D · M · CONFIRMED · new · in flux · R8 member
- Claim: core emits span kinds as `impl Into<String>`. The viz help table and
  the MCP synonym table re-type the names with wildcard fallbacks.
- Sites: viz `ui/sim_debug.rs:156`, `trace/debug_trace.rs:386`,
  `compute/execute/dressup_apply.rs:163`, viz `app/mcp/diagnostics.rs:707,739`.
- Evidence: four help arms name kinds nothing emits; 17 emitted kinds have no
  help; `rs_cam_mcp/src/server.rs:622` advertises dead kinds as examples.
- Fix: NONE → TYPES. Core `DebugSpanKind` with `as_key()`, `help()`,
  `structural()`.
- Verifier: confirm. The three entry dressups that share `entry_style` may be
  on purpose; keep one variant unless separate kinds are wanted.

#### F018 Arc direction is a positional clockwise: bool at about 20 motion sites
E · M · CONFIRMED · new · in flux · below cut
- Claim: `linearize_arc*` take `clockwise: bool`; a swapped literal mirrors the
  arc in the stock sim and the obstacle clip. The emitter has a private
  `ArcDir`.
- Sites: `geometry/arc_util.rs:9,26`, `dexel_stock/simulation.rs:219`,
  `compute/obstacle.rs:1043`, `dressup/entry_audit.rs:98`, `export/viz.rs:746`,
  `export/ribbon.rs:208`, `gcode/emitter.rs:360`.
- Fix: CONVENTION → TYPES. Public `ArcDir`, or a `MoveType::arc()` helper.
- Verifier: confirm, 20 production calls.

#### F019 OperationTransformCapabilities encodes reorder freedom as four positional bools
E · M · CONFIRMED · new · in flux · below cut
- Claim: ten const rows call `new(bool, bool, bool, bool)`; a swap can turn on
  links that gouged. Only a test keeps out the illegal pairs.
- Sites: `compute/catalog.rs:108,190,214`,
  `compute/catalog/capabilities.rs:483,504`, `compute/catalog/tests.rs:579`.
- Fix: SENTRY → TYPES. `RapidOrder` and `LinkPolicy` enums; store `LeadScope`.
- Verifier: confirm. Ten const rows plus one test call. The Inlay example is
  wrong (its row is all false); `TRANSFORM_DISCRETE_SCALLOP` is a real case.
  Keep `continuous_path_required` inside the enum.

#### F020 SimulationOptions.auto_resolution is a dead flag with a false doc
C · S · CONFIRMED · new · in flux · R23 member
- Claim: `resolution: f64` beside `auto_resolution: bool`. Every production
  literal sets false. The doc says the flag matches the GUI auto rule, but the
  branch omits the rest-resolution rule.
- Sites: `session/mod.rs:1060,1127`, `session/compute/simulation.rs:263`,
  `session/compute.rs:2632`, `session/rest_stock.rs:51,173`, cli `project.rs:405`.
- Fix: NONE → TYPES. `resolution: SimulationResolution` through the one
  session rule; `Option<Modulation { ConstrainedMax { feed_scale }, BandMid }>`.
- Verifier: confirm. `auto_resolution_for_groups` has a second production
  caller (`compute.rs:2257`); keep or migrate it on purpose.

#### F022 Capabilities name config fields by &'static str; the advisor writes through serde JSON by name
E · S · CONFIRMED · new · in flux · below cut
- Claim: `clearing_strategy_dial` is a field name; `with_clearing_strategy`
  round-trips through JSON and drops the candidate with no message on failure.
- Sites: `compute/catalog/capabilities.rs:81,111`, `session/compute.rs:1468`,
  `compute/catalog/registry.rs:684`, `trace/narrate.rs:1426`.
- Fix: SENTRY → TYPES. A typed setter, or a method on `OperationConfig`.
- Verifier: confirm. `relink_dial` is a text label only and writes nothing.

#### F023 Param-schema type is a string; the list kind is a "vec<" text prefix
C (re-tiered: E) · S · CONFIRMED · new · in flux · R6 member
- Claim (at the baseline): both `starts_with("vec<")` branches were dead,
  because no `ParamType` published a `vec<` name.
- Sites: `session/compute/params.rs:166,410`, `compute/catalog.rs:1226`,
  `compute/catalog/schema.rs:144,181`.
- Fix: NONE → TYPES. `const KIND: ParamKind` on `ParamType`; the derive
  forwards it.
- Verifier: confirm.
- Changed by e76f82c8 (selection P5): `Vec<[f64; 2]>` now publishes
  `TYPE_NAME = "vec<[f64;2]>"` (`compute/catalog/schema.rs:181-185`), and
  the Profile param `tab_points` uses it. The branches at
  `compute/catalog.rs:1226` and `session/compute/params.rs:410` are live.
  The dead-code half is closed; do not delete the branches or the impl. The
  string-typed kind remains: a list type with another prefix misses both
  branches. Re-tiered from C to E: the "as verified" count in 1.1 uses E,
  and the category table keeps the finder tier C.

#### F025 ApplyScope unpacked into two positional bools for the provenance stamp
E · S · CONFIRMED · new · not in flux · R11 member
- Claim: `apply_suggested_subset(.., rpm_written, speeds, geometry)`; a swap
  restamps cut-geometry provenance on a speeds-only apply.
- Sites: `feeds/provenance.rs:163,170`, `feeds/suggest/apply.rs:236,360`.
- Fix: CONVENTION → TYPES. Pass `ApplyScope`.
- Verifier: confirm. One production caller.

#### F027 Positional bool parameters elsewhere; clippy excessive-bools lints off
F · S · CONFIRMED · new · not in flux · R22 member
- Claim: APIs take bare bool flags that callers pass as literals;
  `run_project_command` takes 5 bools of 14 args.
- Sites: `finish/pencil/emission.rs:182`, `dressup/entry_descent.rs:541`,
  `finish/unified_finish.rs:221`, `compute/config.rs:756`, cli `project.rs:634`,
  `dexel_stock/simulation.rs:1332`, `Cargo.toml:35`.
- Fix: NONE → LINT plus hand enums.
- Verifier: confirm. `fn_params_excessive_bools` fires at 3 bools, so it
  misses the single-bool sites. cli `run.rs:31` has a stale
  `allow(struct_excessive_bools)` without SAFETY.

#### F208 Overlay registry rows are found by a string id; a renamed id hides the control
E · S · CONFIRMED · new · in flux · below cut
- Claim: `registry::row(id)` takes a `&'static str`. Code that names a row
  by a literal skips the row with no message when the id changes.
- Sites: viz `ui/overlays/registry.rs:198-209` (`id: &'static str`,
  `flag: Option<&'static str>` as a field path); literal lookups at
  `controller/events/mod.rs:502` (`"all_toolpaths"`),
  `ui/viewport_overlay.rs:707,763`, `ui/overlays/panel.rs:553`
  (`"rest_heatmap"`). `rg` finds 16 `registry::row(` calls.
- Evidence: every literal lookup sits behind `if let Some(row)`, so a
  rename compiles and the control is not drawn.
- Fix: CONVENTION → TYPES. An `OverlayId` enum with the wire name as a
  method; `row(OverlayId)` is total. F022 class. Map: `viz-ui.md` lead 8.

### 4.3 illegal-states

#### F028 An unknown post token becomes GRBL with no message
A · S · CONFIRMED · new · in flux · R2
- Claim: `rs_cam_cli run --post <typo>` and the project loader turn an unknown
  post token into `PostFormat::Grbl`; the operator gets GRBL G-code for another
  controller.
- Sites: cli `run.rs:175`, `main.rs:116`, `gcode/mod.rs:1511,1608`, cli
  `job.rs:370,1214`.
- Evidence: `main.rs:116-118` `post: String`; `run.rs:175-176`
  `from_token(..).unwrap_or(Grbl)`. The doc at `gcode/mod.rs:1604-1607` says a
  CLI flag can refuse; `run` does not.
- Fix: NONE → TYPES. `value_parser` that calls `from_token`; `JobSection.post:
  PostFormat`; project load refuses (no-legacy ruling) or warns.
- Verifier: both confirm. A plain `ValueEnum` loses the case-insensitive
  aliases; a `value_parser` keeps them. The job path refuses at `job.rs:370`
  before G-code is written, but `sweep.rs` calls `execute_job` and never
  reaches that check.

#### F029 A tool record can hold geometry that panics the GUI thread
A · L (step 1 M) · CONFIRMED · new · in flux · R5
- Claim: `ToolConfig` is flat; the cutter constructors use `assert!`.
  `set_tool_param` writes any number. `build_cutter` runs on the GUI thread
  with no `catch_unwind`. MCP `set_tool_param {included_angle: 0}` panics the
  GUI on the next tool-panel draw.
- Sites: `session/compute/params.rs:542`, viz `ui/properties/tool.rs:407`,
  `app/gpu_upload.rs:1073`, `tool/vbit.rs:36`, `tool/tapered_ball.rs:46`,
  `tool/bullnose.rs:29`, `compute/tool_config.rs:160,457`,
  `rs_cam_mcp/src/server.rs:2170`, `feeds/geometry.rs:503`, `compute/cutter.rs:8`.
- Evidence: `clippy::panic` does not cover `assert!`. `catch_unwind` exists
  only in worker and MCP lifecycle. The panic hook does not save the project.
  The domain is checked in five places with four rules.
- Fix: CONVENTION → SENTRY (step 1) → TYPES. Step 1: one
  `ToolConfig::validate()` called by `set_tool_param`, AddTool and project
  load; cutter constructors return `Result`. End: `ToolShape` enum with
  checked constructors and `serde(try_from)`.
- Verifier: both confirm A (unsaved work is lost).

#### F030 Positive quantities are clamped at use, not parsed at the door
B · M · CONFIRMED · new · in flux · R6 member
- Claim: the declared `ParamRange` is checked only inside `set_toolpath_param`.
  ReplaceToolpathConfig, AddToolpath and project load write unchecked values.
  `horizontal_finish` divides by an unguarded stepover.
- Sites: `finish/horizontal_finish.rs:220`, `session/compute/params.rs:64`,
  `compute/catalog/schema.rs:198`, `compute/operation_configs.rs:2528`,
  `machine/kinematics.rs:362`, `ops/pocket.rs:137`, `ops/zigzag.rs:104`,
  `finish/spiral_finish.rs:156`, `ops/drill.rs:211`.
- Evidence: with stepover 0, `inf as usize` saturates; `+ 1` panics in dev and
  wraps to 0 lines in release (empty toolpath). `schema.rs:220-224` says the
  generator guard catches the file path; `HorizontalPass` has no guard.
- Fix: SENTRY → SENTRY at three doors → TYPES. One core
  `validate_operation(&OperationConfig) -> Result` over the `ParamDef`
  ranges, called from AddToolpath, ReplaceToolpathConfig and load. End:
  refined field types from the derive (L).
- Verifier: both confirm. The cheapest close that covers all doors is the
  `RadialPass::refuse` pattern applied once in the dispatch.

#### F031 GUI run record can say Done with no result
B · M · CONFIRMED · new · in flux · R9 member
- Claim: `ToolpathRuntime` holds `status` and `result` as two fields that must
  agree. `pins.rs:279` clears the result and leaves `Done`.
- Sites: viz `controller/events/pins.rs:279`, `state/runtime.rs:27`,
  `ui/status_bar.rs:71`, `controller/events/compute.rs:139,498`,
  `compute/config.rs:75`, `state/toolpath/entry.rs:130`,
  `ui/toolpath_panel.rs:1028`.
- Evidence: the status bar counts the op as done and the blocker says "That
  operation is generated". `ComputeStatus::Disabled` is documented "Never
  stored" but storable.
- Fix: CONVENTION → TYPES. GUI `enum ToolpathRun { Pending, Computing,
  Done(ToolpathResult), AwaitingPriorStock(..), Failed(String) }`.
- Verifier: both confirm. Keep core `ComputeStatus` as the derived display
  taxonomy. `FreshnessState` stays. Interim: private fields with
  `set_done`/`clear_result`.

#### F032 Stock fit-to-model flag is reconciled by hand at four doors
B · M · CONFIRMED · new · in flux · R13
- Claim: `StockConfig.auto_from_model` sits beside explicit dimensions. The
  rule "an explicit dimension clears the flag" has four copies.
- Sites: `session/mutation/config.rs:494`, `compute/stock_config.rs:67`,
  viz `ui/properties/stock.rs:28`, `app/mcp/commands.rs:1161`, cli
  `job.rs:1115`, `smoke.rs:1122`.
- Evidence: the MCP copy differs: an explicit `auto_from_model: true` plus
  dimensions keeps the flag, and setting the flag does not refit. That is the
  R12 state again.
- Fix: CONVENTION → SENTRY → TYPES. Interim: run the reconcile inside core
  `set_stock_config`. End: `enum StockSize { FitModel { padding }, Explicit {..} }`.
- Verifier: both confirm. Core owns the refit at import and load, not the clear.

#### F033 Height plane order is a warning, checked twice with two rules
B · M · CONFIRMED · new · in flux · R4 member
- Claim: two checkers test plane order with different anchors and severities.
  The GUI badge applies the raw-top rule that R7 called false.
- Sites: viz `ui/properties/tab_badges.rs:38`,
  `diagnostics/adapters/from_static_checks.rs:58,91,614`,
  `compute/config.rs:379,398`, `session/compute/export.rs:59`.
- Evidence: the badge rates clearance < retract DANGER; diagnostics rate it
  Caution. `depth()` uses `.abs()`. A second `ResolvedHeights` struct has no
  production caller. No export door reads either check.
- Fix: CONVENTION → TYPES. One core fn over op, heights and stock top returns
  `Vec<HeightOrderViolation>`; badge and adapter read it. Operator: should an
  inverted stack refuse at export?
- Verifier: both confirm. Simplest first step: the badge reads the core
  diagnostics. Private fields on core `ResolvedHeights` are L.

#### F034 Project and config files accept unknown keys; renames rely on a denylist
B · M · CONFIRMED · new · in flux · R12 member
- Claim: project, post, machine and tool files deserialize with no
  `deny_unknown_fields`; a misspelt key loads as the default.
- Sites: `session/project_file.rs:85,183,230`.
- Evidence: five production `deny_unknown_fields` uses, none on the persisted
  roots. `material/mod.rs:631-633` says an old `kc` key still loads; no
  `RETIRED_KEYS` row covers it, which breaks the doc at `project_file.rs:80-84`.
- Fix: CONVENTION → TYPES. `deny_unknown_fields` on every persisted struct.
- Verifier: both confirm. Audit `serde(flatten)` and the six internally tagged
  enums first. Exempt the settings file (`settings/tests.rs:163`). Keep the
  `RetiredKey` rows as message text. Add one round-trip sentry per root.

#### F036 Ordered angle pairs are independent fields with no order check
E · M · CONFIRMED · new · in flux · R6 member
- Claim: `slope_from/slope_to` and the UnifiedFinish thresholds have no order
  check; an inverted band admits nothing.
- Sites: `compute/operation_configs.rs:1041,1602,2304`,
  `finish/finish_setup.rs:635`, `compute/catalog.rs:910`,
  `finish/finish_planner.rs:465`.
- Fix: NONE → TYPES. A checked `SlopeBand`.
- Verifier: confirm. `waterline_threshold_deg` has no range at all.
  Cheapest: add a cross-field rule to the F030 validator.

#### F037 Registry adapters re-check config variant and geometry at run time
E · L · CONFIRMED · new · in flux · below cut
- Claim: each adapter re-matches its variant with `config_guard!` and asks for
  geometry through accessors that return `Err` on a row mismatch.
- Sites: `compute/execute.rs:510`, `compute/execute/prologue.rs:114,179`.
- Fix: NONE → TYPES. `for_each_op!` passes the typed config;
  `Prepared<G>` per requirement.
- Verifier: confirm; cost L. Partial step: the macro does the variant match
  once (removes 24 `config_guard!` uses).

#### F038 Toolpath ownership is stored by position in three places
E · L · CONFIRMED · new · in flux · R19 member
- Claim: a flat toolpath Vec, per-setup index lists and an index-keyed result
  map are re-keyed by hand on every structural edit.
- Sites: `session/mod.rs:767,1322`, `session/mutation/toolpath.rs:94,175,568`.
- Fix: CONVENTION → TYPES. Setup owns its list; results keyed by `ToolpathId`.
- Verifier: confirm. The cite `project_file.rs:183` is wrong (file nesting is
  at `:677`). Cheap step: private `toolpath_indices` plus a debug partition
  check after each mutation.

#### F202 $341 tool-change mode is a raw u32 in core and an enum on the wire
E · S · CONFIRMED · new · in flux (`rs_cam_mcp/src/server.rs`) · R2 member
- Claim: `ControllerSettings.tool_change_mode: Option<u32>` holds five
  meanings that only a doc lists. The MCP wire already has an enum for it.
- Sites: `machine/mod.rs:292-296` (the doc lists modes 0 to 4),
  `machine/mod.rs:333` (`TOOL_CHANGE_MODE_IGNORE = 4`), `machine/mod.rs:411`
  (`ignores_m6`); `rs_cam_mcp/src/server.rs:213,245`
  (`ToolChangeModeParam`). `$32` `mode: Option<u32>` at `machine/mod.rs:286`
  has the same shape.
- Evidence: the 2026-10-08 M6 crash came from the tool-change settings
  (`$65`, `$341`; `planning/grblhal_2026-10-08/OPERATOR_NOTE.md:6-10`). A value
  of 5 loads and `ignores_m6` reads it as "M6 runs".
- Fix: NONE → TYPES. A core `ToolChangeMode` enum parsed at the `$$`
  import; the wire enum converts by an exhaustive `From`. Map:
  `core-io-output.md` lead 22.

#### F210 GUI literal ranges are the only domain for tool, stock, head, shoe and machine fields
E · S · CONFIRMED · new · not in flux · R6 member
- Claim: non-operation entities take their numeric domain from a literal
  `lo..=hi` in the draw code. MCP and project load do not see these ranges.
- Sites: 31 `..=` in viz `ui/properties/tool.rs` (for example `:215,242,297,307,360`),
  `stock.rs`, `head.rs`, `shoe_row.rs`, `machine_panel.rs` (`rg -o '\.\.='`).
- Evidence: `tool.rs:297` limits the V-bit included angle to
  `1.0..=179.0`. Core states `0 < included_angle < 180` only in a doc
  (`compute/tool_config.rs:451`). MCP `set_tool_param {included_angle: 0}`
  panics the GUI thread (F029), so the GUI range is today the only check.
- Fix: DOC → TYPES. One core domain per entity field (the F029
  `ToolConfig::validate` step); the GUI reads it, as `dv` reads
  `OperationConfig::param_range_for_type`. This sweeps the section 8
  "GUI literal ranges" gap. Map: `viz-ui.md` section 5, "Parameters and
  config".

### 4.4 encapsulation-visibility

#### F040 The multi-tool planner is a second write door that drops its Effects
B · M · CONFIRMED · new · in flux · R9 member
- Claim: `plan_multitool_finishing` is a pub `&mut` writer outside the
  Command registry. It drops `remove_toolpath`'s Effects. The viz door mirrors
  neither `simulation_cleared` nor `Effects.stale`.
- Sites: `session/multitool.rs:338,363,835`, viz
  `controller/events/planner.rs:56`, `session/mutation/toolpath.rs:64,122`,
  `tests/setters_have_rows_wp15a.rs:101`, viz `controller/events/toolpath.rs:447`.
- Fix: CONVENTION → TYPES. `Command::PlanMultitoolFinishing`, fold the inner
  Effects; or return `(Effects, MultitoolPlanOutcome)`.
- Verifier: both confirm. Update three allowlists (core wp15a, wp15b, viz
  wp15a). `mark_derived_rest_dependents_stale` walks only one edge.

#### F042 Three pub feeds writers bypass the single-write ApplicableRecommendation gate
E · S · PLAUSIBLE · new · not in flux · R11 member
- Claim: `apply_feeds_result_to_op`, `apply_speeds_to_op`,
  `apply_cut_geometry_to_op` and `resolve_operation_invariants` are pub and
  need no `ApplicableRecommendation`.
- Sites: `feeds/suggest/apply.rs:390,421,452,812,856`, `feeds/suggest.rs:21`.
- Fix: DOC → TYPES.
- Verifier: both plausible. `apply_feeds_result_to_op` HAS a production
  caller (`suggest.rs:915`), so a `cfg(test)` gate does not compile. Route
  `suggest_for_operation` through `applicable()` plus `apply`, then demote.

#### F046 Invariant-bearing core types publish their fields
E · L · CONFIRMED · new · in flux · below cut
- Claim: `DexelGrid` (conservative_top), `AnnotatedToolpath` (span bounds) and
  `ExecutionContext` (findings `RefCell`) have pub fields.
- Sites: `stock/dexel.rs:359,620`, `adaptive3d/path.rs:778`,
  `compute/stock_carry.rs:187`, `trace/toolpath_spans.rs:457`,
  `compute/execute.rs:318`, `compute/execute/dressup_apply.rs:127`.
- Fix: DOC → TYPES. Private fields plus invariant-keeping methods.
- Verifier: confirm; cost M-L. 21 pub fields on `ExecutionContext`, not 22.
  Do `ExecutionContext` first (S).

#### F047 detect_rest_valleys is pub though a rule says a product rest field comes only from detect_rest
E · M · CONFIRMED · new · not in flux · R23 member
- Sites: `surface/rest_field.rs:1032`, `finish/rest_detector.rs:127`,
  `tests/reach_policy_pr4.rs:130`.
- Evidence: one production caller; 15 test calls in 8 files;
  `finish/CLAUDE.md:29`.
- Fix: CONVENTION → TYPES. Move the tests onto `detect_rest`, then demote.
- Verifier: confirm.

#### F048 Two pub(crate) not-measured reason strings force viz to copy them as literals
D · S · CONFIRMED · new · not in flux · R7 member
- Sites: `session/diagnostics_types.rs:334,343`, viz
  `controller/events/compute.rs:2469,2471`.
- Fix: SENTRY → TYPES. A pub `CutMetricsNotMeasured` enum with `as_str`.
- Verifier: confirm. A third near-copy in `ui/preflight.rs:600` already
  differs.

#### F049 One sealed trait; core traits with no outside user are pub and unsealed
F · S · CONFIRMED · new · not in flux · below cut
- Sites: `tool_load/mod.rs:472`, `compute/catalog/schema.rs:133,144`,
  `compute/catalog.rs:673`, `trace/transform_provenance.rs:68`.
- Fix: NONE → TYPES. `pub(crate)` where no pub bound needs it; seal
  `OperationParams`.
- Verifier: confirm; hygiene, F not E. The derive already emits `crate::` paths.

#### F212 Library folders come from a process-global PATHS that every resolver reads in secret
E · M · CONFIRMED · new · not in flux · below cut
- Claim: `tool_library_dir` and `machine_library_dir` read a hidden global.
  The first read loads the settings file from disk.
- Sites: `settings/paths.rs:174` (`static PATHS: RwLock<Option<PathSettings>>`),
  `:179` (`install_paths`), `:191-201` (`installed_paths` loads the file on
  first use), `:205-215`; production readers `io/tool_library.rs:79`,
  `io/machine_library.rs:69`.
- Evidence: the pure `*_from` forms exist for tests, so the global is the
  only input that a caller cannot pass. A test that reads a library through
  the plain form depends on the process settings file.
- Fix: CONVENTION → TYPES. Pass `&PathSettings` (or a `LibraryRoots`
  value) from the session or the app; keep the global only at `main`. Map:
  `core-session.md` lead 27.

### 4.5 error-handling

#### F051 Export load gate drops its advisory note; an override erases its own record
B · S · CONFIRMED · new · in flux · R10 member · also F052
- Claim: both production export doors bind `Ok(Some(note))` to `_note` and drop
  it. With `accept_exceeded=true` the refusing lines are discarded. The CLI
  project door sets both overrides to true with no flag.
- Sites: `gcode/mod.rs:853,864,922,929,1201,1331`, `tool_load/verdict.rs:2070`,
  cli `project.rs:1046`, `main.rs:211`, `run.rs:209`.
- Evidence: the doc at `:864` ("no producer until S3") is stale; the arm is
  reachable. `rs_cam_cli/CLAUDE.md:15` requires explicit safety overrides.
- Fix: DOC → TYPES. `LoadPolicyOutcome { advisories, overridden }` inside the
  export result, so each door must destructure it; CLI flags for the overrides.
- Verifier: both confirm. `#[must_use]` alone does not catch `let _note =`.
  `main.rs:211` claims feed parity, not gate parity.

#### F053 Project loader binds a missing tool_id/model_id to id 0
B · S · CONFIRMED · new · in flux · R12 member · also F003
- Claim: a toolpath with no `tool_id` loads bound to the first tool, with no
  warning. This breaks the P2.4/P2.5 "ids are EXACT" rule.
- Sites: `session/project_file.rs:699,856,1030,1125`, `session/mod.rs:1835`.
- Evidence: `project_file.rs:1030-1031` `unwrap_or(0)`; the missing-reference
  warning fires only when id 0 does not exist. `:856` and `:1125`
  `unwrap_or(idx)` can collide with an explicit id.
- Fix: NONE → TYPES. Make both fields required (no serde default).
- Verifier: both confirm. Do the same for `ts.id` and `model_section.id`, or
  refuse duplicates.

#### F054 Typed errors are flattened to String at every layer boundary
B (contested: E) · L · CONFIRMED · new · in flux · R17
- Claim: `OperationError` (except Selection) becomes
  `SessionError::OperationFailed(String)`; import, obstacle and export errors
  become strings. `SessionError` has no `source()`. Viz reads the cancel flag.
- Sites: `session/compute.rs:753`, `io/mod.rs:79`, `compute/simulate.rs:2370`,
  `session/compute/export.rs:71`, `session/mutation/config.rs:603`,
  `session/mutation/entities.rs:401`, `session/compute/generation.rs:129`,
  `session/mod.rs:330`, viz `compute/worker/execute/mod.rs:256`,
  `compute/worker.rs:542`.
- Fix: CONVENTION → TYPES. `SessionError::Operation(#[from] OperationError)`
  first, then Export and obstacle; move to `thiserror`.
- Verifier: one keeps B (G-REGEN-RACE workaround is a past break); one says E
  (Simulation and CollisionCheck already stay typed; the budget-guard read is
  by design). Tests that match message text must move to variant matches.

#### F055 SessionError::MissingGeometry carries three meanings
E · M · CONFIRMED · new · in flux · R17 member
- Sites: `session/compute/diagnostics.rs:285,556`, cli `project.rs:778`,
  `session/compute.rs:2612`, `session/mutation/entities.rs:77`,
  `session/mutation/toolpath.rs:669`, viz `app/mcp/commands.rs:1923`.
- Fix: CONVENTION → TYPES. `collision_check -> Ok(NotApplicable)`;
  `SessionError::ModelNotFound(ModelId)`.
- Verifier: both re-grade B → E: no break today.

#### F056 GUI apply doors drop core refusals with no trace
E · M · PLAUSIBLE · new · in flux · R9 member
- Claim: `apply_quietly` maps `Err(_)` to `None` with no log; 17 callers drop
  that too, some then mark the project edited.
- Sites: viz `controller/events/mod.rs:100`, `undo.rs:153`, `toolpath.rs:397`,
  `model.rs:870`, `ui/properties/panel_apply.rs:239`,
  `ui/properties/mod.rs:834,1609`, `session/command.rs:1100`.
- Fix: CONVENTION → LINT. Return `Result`; deny `let_underscore_must_use` in
  `controller/`.
- Verifier: E, not B. Most callers pre-check the index; `panel_apply.rs` logs.
  `Option` is not `must_use`, so the lint needs the `Result` change. Cheapest:
  `tracing::warn!` in the one door.

#### F057 CLI smoke harness drops the resolution refusal
E · S · PLAUSIBLE · new · in flux · below cut
- Sites: cli `smoke.rs:612,751`, `session/rest_stock.rs:604`, `main.rs:330`,
  `sweep.rs:328,377`.
- Fix: NONE → TYPES. `?` on the apply; a clap `value_parser` for positive mm.
- Verifier: the central claim is refuted: the measurement runs at the raw
  argument, not at the stored cell. The real gap is that the stored cell and
  the sim options can differ. Sweep SVG/PNG write errors are dropped (true).

#### F058 T-4 sentinel shape recurs on the inverse deflection
E · M · PLAUSIBLE · T-4 (forward side closed 2026-09-18) · not in flux · below cut
- Sites: `feeds/cutter_constraints.rs:267`, `feeds/suggest/axial_envelope.rs:153`,
  `feeds/predict.rs:474`.
- Fix: DOC → TYPES. `invert_deflection -> Result<f64, DeflectionUnmodeled>`.
- Verifier: E. Suggest writes the RPM before pass 0 (`apply.rs:176-179`); the
  live path is the profile/rationale view (`profile.rs:178-187`).

#### F059 ProjectSession::generate_all cannot fail and has only test callers
C · S · CONFIRMED · new · in flux · R23 member
- Sites: `session/compute.rs:2587`, `session/mod.rs:9`, seven test sites.
- Fix: NONE → TYPES. Delete it; tests get a helper that walks
  `generation_plan::plan` and returns the failures.
- Verifier: confirm. It also walks index order, not the plan.

#### F061 ExportError is one string for five refusal kinds
E · M · PLAUSIBLE · new · in flux · R10 member
- Sites: `gcode/mod.rs:38,931,1127,1212,1244`, viz `io/export.rs:638`.
- Fix: NONE → TYPES. `enum ExportRefusal` with `override_needed()`.
- Verifier: the spelling claim is wrong but the defect is worse: the text names
  `accept_exceeded=true`, a core field name that no surface accepts (MCP uses
  `accept_exceeded_tool_load`).

#### F062 MCP refusals travel as strings inside success replies, in three shapes
E · M · CONFIRMED · new · in flux · below cut
- Sites: viz `mcp_bridge.rs:1095,1124`, `mcp_server.rs:401`, `app/mcp.rs:832`,
  `app/mcp/commands.rs:1914`.
- Fix: CONVENTION → TYPES. `enum McpReply { Ok(Value), Refused {..} }`.
- Verifier: confirm. Guards hold today. Smaller step: require a stated
  `McpOutcome` on every toast-bearing arm.
- Fold: F207 (R22c) lifts the non-JSON-is-success fallback
  (`mcp_bridge.rs:1124`, `app/mcp.rs:379`) to its own B row.

#### F063 OperationError::Other mixes wiring defects with operator refusals
E · M · CONFIRMED · new · in flux · R17 member
- Sites: `compute/execute.rs:54,263,515`, `compute/execute/prologue.rs:180`,
  `compute/execute/height_field.rs:109`, `compute/execute/curve_engrave.rs:95`,
  `compute/execute/finish_3d.rs:509`.
- Fix: CONVENTION → TYPES. `InvalidParam`, `Internal`; delete `From<String>`.
- Verifier: confirm. Deleting `From<String>` first lets the compiler list the
  sites.

#### F064 Result<_, String> in library APIs
F · M · PLAUSIBLE · new · not in flux · below cut
- Sites: `settings/mod.rs:543,680,960`, `feeds/vendor_lut.rs:545`,
  `rs_cam_mcp_proxy/src/proxy.rs:1064`, cli `memory_budget.rs:35`.
- Fix: limit to core: `SettingsError`, `IoLoadError`, `LutError`.
- Verifier: 50 sites, not 52. `server.rs:1659` is the wrong line. Clap and wire
  helpers may stay `String`.

#### F065 Serialise failure folds to empty inside change-detection signatures
E · S · CONFIRMED · new · in flux · R7 member
- Sites: `session/mod.rs:911`, `compute/simulate.rs:939`.
- Fix: NONE → TYPES. Destructure `ToolpathConfig` with no `..` in
  `generation_inputs_signature`; `.expect` instead of `unwrap_or_default`.
- Verifier: confirm. Only the hand field list misses a new field.

#### F199 The default region-extraction names hide the 64-region cap
B · S · CONFIRMED · new · partly in flux (`session/multitool.rs`) · R22a
- Claim: `region_polygons_from_mask` and `_clamped` return a list that
  cannot say whether `MAX_REST_REGIONS` dropped regions. Three production
  callers use them.
- Sites: `geometry/region_mask.rs:111-119` (plain form), `:149-159`
  (`_clamped`); production callers `finish/finish_planner.rs:537,957`,
  `session/multitool.rs:1291`. The reporting form is used at
  `surface/rest_field.rs:967`, `compute/execute/standing_material.rs:255`,
  `maps/tier_islands.rs:1005,1022`.
- Evidence: the doc at `geometry/region_mask.rs:107-110` says "This return
  value cannot tell you whether the cap fired". `maps/tier_islands.rs:12-20`
  measured 566 raw islands on wanaka; the plain form dropped 502 of them and
  "the caller reads a 64-long list as the whole answer". Only a
  `tracing::warn!` reports the drop.
- Fix: DOC → TYPES. Keep only `region_polygons_from_mask_reported`; put
  `#[must_use]` on `RegionCapReport`; each caller states what it does with
  a truncation (`GEOM_REGION_CAP_TRUNCATED`, `diagnostics/ids.rs:322`,
  already reaches a finding at `diagnostics/adapters/from_generation.rs:123`).
  Map: `core-io-output.md` lead 16.

#### F207 MCP outcome: a reply that is not JSON reads as success
B · S · PLAUSIBLE · new · in flux · R22c
- Claim: when a handler states no outcome, the dispatch classifies the reply
  text. Text that is not JSON is `Succeeded`, so a refusal in plain text
  shows a success toast.
- Sites: viz `mcp_bridge.rs:1125-1127` (`from_json_response` returns
  `Succeeded` when the parse fails), `app/mcp.rs:379` (the fallback when
  `stated` is `None`).
- Evidence: the doc at `mcp_bridge.rs:1100-1115` records G-MCPTOAST, a past
  break of this toast class, and says "A reply that is not JSON at all is
  also a success". No live handler that refuses in plain text was found;
  hence PLAUSIBLE. Both files have working-tree edits from selection P4.
- Fix: DOC → TYPES. Every toast-bearing arm returns an `McpOutcome`; delete
  the text fallback. F062 holds the site `mcp_bridge.rs:1124` and names this
  fix as its smaller step; F207 lifts that mechanism to its own row.

### 4.6 duplication

#### F067 Two G-code export pipelines; only GUI/MCP rewrite rapids and run the machine-safety pass
B · M · CONFIRMED · new · in flux · R10
- Claim: core `export_gcode_checked`, which the CLI uses, applies no
  `replace_rapids_with_feed` under `high_feedrate_mode` and runs no
  `validate_machine_safety`. One project emits G1 traverses from the GUI and
  G0 rapids from the CLI; the CLI file is never safety-checked.
- Sites: viz `io/export.rs:104,597,643`, `gcode/mod.rs:376`, cli `run.rs:216`,
  `project.rs:1051`, `session/compute/export.rs:59`.
- Evidence: both functions have production callers only in viz
  `io/export.rs`. Core simulation already models rapids as feeds when the
  mode is on. `export_parity_core_vs_gui_p0.rs:52-55` avoids the case.
- Fix: DOC → TYPES. Core export returns `ExportedGcode { gcode,
  machine_safety, .. }`; run the safety pass before the rewrite.
- Verifier: both confirm; close to A. The GUI reads `gui.post`, not the session
  post config (see F072): unify the source too. The wizard overlay is a second
  divergence of the same class.

#### F070 Tool-load gate context assembled by hand on export and on GUI cards
B (contested: D) · S · CONFIRMED · new · in flux · R10 member
- Sites: `gcode/mod.rs:740,776`, viz `state/simulation.rs:300,320,464`,
  `tool_load/mod.rs:309`, `feeds/vendor_normalize.rs:152,215`.
- Evidence: `gcode/mod.rs:742-751` copies `op_family_to_lut` inline, whose doc
  says it exists to be shared. PassRole → LutPassRole is matched four times.
- Fix: CONVENTION → TYPES. `From<PassRole> for LutPassRole`; an owned
  `LoadContextParts` with `fn ctx(&self) -> ToolpathLoadContext<'_>`.
- Verifier: one keeps B (cold-start silent miss); one says D (the copies agree
  today). The context borrows `&ToolDefinition`, so an owned context is not
  possible directly.

#### F071 Two simulation-group builders; rapid feed resolved four times
D · L (rapid feed S) · CONFIRMED · D11 residue · in flux · R26
- Sites: `session/compute/simulation.rs:84,521`, viz
  `controller/events/simulation.rs:158,396`, viz `ui/readiness.rs:142`,
  `compute/CLAUDE.md:13`, `session/compute.rs:2184`,
  `session/compute/diagnostics.rs:613`.
- Changed since D11: the `.max(1.0)` clamp is missing at `compute.rs:2184`.
- Fix: DOC → TYPES. `PostConfig::rapid_feed_mm_min(&MachineProfile)` now;
  core `simulation_request(scope)` with WP28.
- Verifier: confirm.

#### F072 GuiState::post is a second copy of the post config
E · M · PLAUSIBLE · new · in flux · R10 member
- Sites: viz `io/export.rs:604`, `controller/events/undo.rs:20,92`,
  `controller/io.rs:378`, `io/setup_sheet.rs:409`, `controller/CLAUDE.md:30`.
- Fix: DOC → TYPES. Delete `GuiState::post`.
- Verifier: the failure scenario is refuted (`SetPostConfig` cannot be
  refused). The MCP export also reads `gui.post`. Consolidation only.

#### F074 Display-label tables re-typed per surface with drift
D · S · CONFIRMED · new · not in flux · R24 member
- Sites: `feeds/extrapolation/plunge.rs:335`, `feeds/vendor_lut.rs:147`, viz
  `ui/feeds/shared.rs:90,101`, `feeds/plunge.rs:248`, viz
  `ui/sim_op_list.rs:792,829`, `ui/sim_diagnostics.rs:2252`,
  `ui/sim_timeline.rs:978`.
- Evidence: "Aluminum" vs "aluminium"; one span shows "Pass 1 @ Z" and
  "DepthPass 0 z=".
- Fix: NONE → single source. `label()` / `display_label()` in core.
- Verifier: confirm. `sim_op_list.rs:1249` is the wrong site.

#### F075 Cutter engaged-diameter geometry implemented twice
D · S · CONFIRMED · new · not in flux · R24 member
- Sites: `feeds/mod.rs:159`, `tool/tapered_ball.rs:176`, `tool/vbit.rs:125`.
- Fix: SENTRY → single source. Free scalar fns in `tool/` called by both.
- Verifier: confirm. `feeds/geometry.rs:350` is a doc line, not a copy.

#### F076 Geometric and statistical primitives copied per module with different edge rules
D · M · PLAUSIBLE · D10 (one pair then; six copies since) · in flux · R27
- Sites: `geo.rs:381`, `compute/execute/standing_material.rs:764`,
  `compute/alignment_pins/walls.rs:181`, `dressup/link.rs:49`,
  `polygon.rs:1946`, `adaptive/search.rs:843`, `metrology/spacing.rs:211`,
  `tool_load/distribution.rs:301`, `maps/reach_map.rs:1375`,
  `metrology/monge.rs:527`, `surface/rest_field.rs:1370`.
- Fix: DOC → CONVENTION + Option.
- Verifier: three nearest-rank quantiles, not four. `conformal_spiral`
  averages the middle pair (claim wrong); `tier_islands.rs:1085` takes the
  lower. Check each median caller before a swap.

#### F077 SweptDispatch is a third band dispatcher; STK-09 merged only two
D · L · CONFIRMED · D14 (STK-09 merged two of three) · not in flux · below cut
- Sites: `dexel_stock/swept.rs:798,920,934,991`, `dexel_stock/band_batch.rs:117`,
  `dexel_stock/whole_path.rs:219`.
- Evidence: Swept is the Auto route. Drift exists: swept `MIN_JOBS_PER_BATCH`
  8 vs whole_path 64.
- Fix: CONVENTION → TYPES. Generalise `BandJob` with `bins()`; paired A/B.
- Verifier: confirm. S step: one `BatchCaps` const.

#### F078 3D Rough planner restates the emitter's descent-floor rule by hand
D · S · CONFIRMED · new · not in flux · R24 member
- Sites: `adaptive3d/clearing.rs:672,694,1823`, `adaptive3d/path.rs:1507`.
- Fix: CONVENTION → single fn. `descent_start_z(..)` and one shared const.
- Verifier: confirm; third copy `CROSSOVER_PLUNGE_BUFFER_MM`.

#### F080 UnmodeledReason has four renderers with different wording
D · S · CONFIRMED · T-20 (open, unchanged since 2026-09-18) · in flux · R24 member
- Sites: `diagnostics/adapters/from_tool_load.rs:998`, viz `ui/preflight.rs:602`,
  `ui/sim_diagnostics.rs:1410`, `gcode/mod.rs:1017`.
- Fix: NONE → TYPES. `operator_text()` on the type.
- Verifier: four full renderers, not two. Nothing new beyond the count.

#### F082 Deflection-cap inputs assembled twice although the doc says one assembly
D · S · CONFIRMED · new · in flux · R24 member
- Sites: `feeds/efficiency.rs:315,344`, `session/compute.rs:2000`.
- Fix: DOC → single fn.
- Verifier: confirm; the guards already differ (finiteness). Home is
  `tool_load::deflection`, not `feeds/force.rs` (layering).

#### F084 Map-cache scaffolding and same-named epsilon constants repeated
F · S · CONFIRMED · D8 residue · not in flux · below cut
- Sites: `maps/tier_map_cache.rs:180`, `maps/reach_map_cache.rs:180`,
  `maps/finish_surface_cache.rs:300`, `maps/geom_cache.rs:252`,
  `toolpath.rs:628`, `dressup/mod.rs:801`.
- Fix: one table trait; one module const per tolerance.
- Verifier: confirm, low value.

#### F186 Project-file twin structs for stock and tool
D · M · CONFIRMED · new · in flux · R12 member · also F081
- Sites: `session/project_file.rs:466,497,521,533,584`,
  `compute/stock_config.rs:68,99`, `session/save.rs:16,142`,
  `compute/tool_config.rs:63,201`.
- Fix: CONVENTION → TYPES. Persist `StockConfig` directly with container
  `serde(default)`.
- Verifier: confirm. For tools, `ToolConfig::new_default` varies by type and
  the loader has a lenient unknown-type path; state the break. First step:
  `tool_type_to_key` → `serde_token` (S).

#### F197 Four hand copies of the stale-trace rewrite; S3 missed one once
B · S · CONFIRMED · T-21 (closed 2026-09-18 for the instance, `TECH_DEBT_REGISTER.md:37`; the pattern stays) · in flux · R7 member
- Claim: each verdict type has its own `rewrite_sim_required_to_stale_*`
  fn, and the call block names each field by hand. A fifth gate needs a
  fifth fn and a fifth call, and nothing forces either.
- Sites: `gcode/mod.rs:641,652,663,680` (one fn per verdict type), the call
  block `gcode/mod.rs:813-822`, the struct `tool_load/verdict.rs:237-250`.
- Evidence: the doc at `gcode/mod.rs:672-679` records that S3 added the
  depth gate with no rewrite arm (T-21), so one toolpath showed three rows
  stale and one row never simulated.
- Fix: CONVENTION → TYPES. One `ToolpathLoadVerdict` method that
  destructures the struct with no `..` and rewrites each `Unmodeled` arm.
  Map: `core-io-output.md` lead 11.

#### F200 The air / low-engagement partition is typed at six sites
D · S · CONFIRMED · new · partly in flux (`trace/narrate.rs`) · R24 member
- Claim: the bounds 0.02 and 0.10 on `radial_woc_fraction` are literals at
  six sites. A named constant exists for one bound and no site reads it.
- Sites: `stock/sim_triage.rs:199-201`, `stock/simulation_cut/analysis.rs:161-163`,
  `stock/simulation_cut/reporting.rs:110-112`,
  `stock/simulation_cut/accumulate.rs:135-138`, `tool_load/power.rs:425`,
  `trace/narrate.rs:730`.
- Evidence: `AIR_CUT_RADIAL_WOC_FRACTION = 0.02` (`stock/simulation_cut.rs:253`)
  is read only by its own predicate (`:262`). The 0.10 bound has no name.
  The comment at `reporting.rs:107-109` says the copies "must stay in step".
- Fix: CONVENTION → TYPES. One `SimulationCutSample` method returns an
  engagement class enum; every site matches it. Map: `core-stock.md` lead 1.

#### F213 Two capitalised helpers in viz; components/format.rs holds only a slug fn
F · S · CONFIRMED · new · not in flux · below cut
- Sites: viz `ui/components/data_table.rs:422`, `ui/sim_diagnostics.rs:2059`
  (same body); `ui/components/format.rs:1-20` (module doc "Shared text
  formatting", one fn `slugify`).
- Fix: CONVENTION → single source. One `capitalised` in `components/`;
  rename `format.rs` by role or move the number formats there. Map:
  `viz-ui.md` leads 19 and 26. Lead 29 (Widget against `show`) stays
  dropped (section 7).

#### F214 Post dialect facts live twice: the validator table and the post data (CS-17)
D · S · CONFIRMED · new (cold-start CS-17) · not in flux · below cut
- Claim: `PostInvariants` restates per post what the post TOML already
  says, in a hand table. An edited post TOML can disagree with the
  validator.
- Sites: `export/gcode_validator.rs:153-203` (`invariants_for`,
  `supports_m6`, `no_canned_cycles`); `gcode/post.rs:299`
  (`canned_drill_cycles`), `gcode/post.rs:434` (`tool_change_commands_m6`);
  `posts/grblhal.toml:100`.
- Evidence: the two sources use different meanings for one fact. For
  LinuxCNC and Mach3 the validator allows canned cycles
  (`gcode_validator.rs:175,186`), and the post data says the controller
  does not run them (no `canned_drill_cycles` key in `posts/linuxcnc.toml`
  or `posts/mach3.toml`, so the serde default `false` applies). Whether
  that is a divergence or a deliberate limit is not settled: D, below cut.
- Fix: CONVENTION → TYPES. Derive `PostInvariants` from the `Post` value
  (`supports_m6 = post.tool_change_commands_m6()`, `no_canned_cycles =
  !post.canned_drill_cycles`). The alias index maps CS-17 here, not to
  F192.

### 4.7 dead-code

#### F085 Test instruments and test-only wrappers compile into the product library with no gate
C · M · CONFIRMED · new · in flux · R23 · also F157, F044
- Claim: `metrology/` (2663 lines), the `adaptive_3d_toolpath*` wrapper chain,
  entry_audit audits, polygon helpers and other self-declared test doors are
  pub with no production caller and no cfg gate.
- Sites: `lib.rs:51`, `adaptive3d/mod.rs:37,488,536,618,634`,
  `adaptive3d/path.rs:560`, `dressup/entry_audit.rs:189`,
  `polygon.rs:166,931,984`, `stock/dexel_mesh.rs:165`,
  `tool_load/mod.rs:267`, `export/fingerprint.rs:856`,
  `surface/flow_accum.rs:131,385`, `maps/tier_map.rs:164`, `mesh.rs:795,842`,
  `session/multitool.rs:721`, `compute/toolpath_stats.rs:1806`,
  `machine/mod.rs:678`, `tests/the_test_doors_are_gated_fld0405.rs`.
- Evidence: only `adaptive_3d_toolpath_output_with_cancel` is production. The
  FLD-04/05 sentry pins 24 items and misses `flow_accum.rs`. 43 "Test door"
  labels vs 29 `test-support` gates.
- Fix: NONE → TYPES (default build). `#[cfg(any(test, feature =
  "test-support"))]`; metrology behind `research` or `test-support` with
  `required-features` rows; collapse the wrapper chain.
- Verifier: confirm. `reach_map.rs:1854` is already gated (wrong site). A
  debt_scan check for an ungated "Test door" doc stops the gap from returning.

#### F086 pub items with zero references anywhere (incl. expired S1 hold)
C · S · CONFIRMED · S1 (TECH_DEBT_PLAN.md:53; hold ownerless since UP8 16df6dcd) · in flux · R23 · also F092, F045
- Claim: nine pub items have only their definition line; COLLISION_POINT and
  SPACE_0 were held for UP8, which landed without the sentry that was to read
  them.
- Sites: `budget/guard.rs:147`, `budget/mod.rs:88`, `machine/head.rs:334`,
  `stock/dexel.rs:285`, viz `state/shoe_plan.rs:204`,
  `ui/components/line.rs:45`, `ui/tokens.rs:36,757`, `render/colors.rs:44`,
  `feeds/suggest/apply.rs:856`.
- Evidence: 141 own-file-only pub items returned after S29 (94 core, 45 viz).
  `dead_code` cannot fire on pub items of a library; viz is a lib with `pub mod`.
- Fix: NONE → TYPES. Delete the items and the `render/CLAUDE.md:14` line.
  Durable: viz modules `pub(crate)` and `unreachable_pub` in core.
- Verifier: confirm. `HELIX_ENTRY`, `with_flag` and `EMBEDDED_LUT` in the site
  list are referenced: remove them.

#### F087 Enum variants that no code constructs (VizError, FindingKind, Scope)
C · S · CONFIRMED · new · not in flux · R17 member · also F060
- Sites: viz `error.rs:8,9,52`, `io/import.rs:14`, `controller/io.rs:411`,
  `export/gcode_validator.rs:58,124,406`, `diagnostics/mod.rs:106`,
  `stock/sim_triage.rs:445`.
- Evidence: `VizError::{Stl,Svg,Dxf,Step}Import` and `ProjectSave` are never
  built; `import.rs` maps errors to `Other` while `From<SessionError>` maps to
  `ProjectLoad`. `requires_percent_brackets` is false on all four posts.
- Fix: NONE → TYPES. Delete; one `SessionError → VizError` mapping; optional:
  delete `From<String>`.
- Verifier: confirm. Update the tests at `gcode_validator.rs:1065,1080`.

#### F088 chipload_envelope_for_toolpath is documented as the advisor door but only sentries call it
C · S · CONFIRMED · new · not in flux · R23 member
- Sites: `tool_load/mod.rs:267`, `tests/every_consumer_reads_one_claimed_band_g1.rs:14,237`,
  `session/compute.rs:1696`.
- Fix: SENTRY → TYPES. Delete; point the sentries at the real entry points.
- Verifier: confirm. Five test files call it, not one.

#### F089 ui/theme.rs alias layer outlived its UP2-UP8 migration
E · M · CONFIRMED · new · not in flux · below cut
- Sites: viz `ui/theme.rs:1,27,32,63`, `tests/panels_read_the_token_module_up1.rs:392`.
- Evidence: 221 `theme::` uses in 24 files; six names have no production
  reader.
- Fix: DOC → TYPES. Rewrite to `tokens::` and delete `theme.rs`.
- Verifier: E, not D: aliases cannot drift.

#### F090 Legacy read paths survive the no-legacy ruling
E · S · CONFIRMED · L15 (deferred 2026-09-16, unchanged; now post.rs:308) · in flux · R2 member
- Sites: `gcode/mod.rs:106,1426,1428,1491`, `gcode/post.rs:308`, viz
  `state/runtime.rs:423`.
- Fix: NONE → parse enforcement. Delete aliases, the `gcode::emit_program`
  re-export, and make `tool_change` required.
- Verifier: confirm. `from_token` also parses CLI input: the change affects
  typed input too.

#### F091 Material::to_key/from_key: 250 lines with no production caller
C · S · CONFIRMED · new · not in flux · R23 member
- Sites: `material/mod.rs:1586,1692,1704,1829`, `tests/literature_matrix/shim.rs:101`.
- Fix: NONE → TYPES. Move the key table into the shim as a fallible match;
  delete both fns.
- Verifier: confirm. Serde cannot parse the shim's flat keys (Material is
  externally tagged).

#### F093 truck-geometry and truck-polymesh are declared but never used
F · S · CONFIRMED · new · not in flux · R23 member
- Sites: core `Cargo.toml:11,61,62`, `io/step_input.rs:13`.
- Fix: remove; check with `cargo check -p rs_cam_core --features step`.
- Verifier: confirm. Grade is NONE unless machete joins the gate.

### 4.8 inconsistency

#### F095 INC-01 The viz re-derives the drill-family predicate
B (contested: E) · S · PLAUSIBLE · new · in flux · R8 member
- Sites: viz `app/viewport.rs:72`, `ui/properties/feeds_speeds.rs:93`,
  `ui/properties/mod.rs:1036`, `compute/catalog.rs:629`.
- Fix: DOC → SENTRY. `op_type().is_drill_kinematics()`; source-scan sentry.
- Verifier: the three drill sites are true; the sets agree today. The
  DropCutter, Pencil and Trace sites are field access, not policy: drop them.
  Check `viewport.rs:72` intent first.

#### F096 INC-02 The post-apply epilogue has 6 named doors and about 15 inline copies
B · M-L · CONFIRMED · new · in flux · R9 · also F164, F041
- Claim: `AppState.session` is pub; 21 viz production sites call
  `session.apply` and choose what to mirror. The MCP core route stamps stale
  on every arm but clears the simulation on three arms only.
- Sites: viz `state/mod.rs:135`, `controller/events/mod.rs:65,100,785`,
  `ui/properties/panel_apply.rs:130,227,339,384`, `controller/io.rs:34,356`,
  `controller/events/pins.rs:203`, `model.rs:258,816`, `undo.rs:266`,
  `simulation.rs:145,848`, `ui/properties/mod.rs:834,1609`,
  `controller/events/compute.rs:184`, `app/input.rs:115,129`,
  `app/mcp/commands.rs:313,2276,2836`, `state/freshness.rs:127`.
- Evidence: WP19 found "BOTH halves of the answer were dropped" at
  `input.rs:129-135`. An MCP `set_toolpath_param` drops the core simulation
  but leaves the viz one.
- Fix: SENTRY → TYPES. Private `session` with a getter; `AppState::apply ->
  Applied` (`#[must_use]`) that stamps stale and raises a pending-invalidate
  flag that the frame loop discharges.
- Verifier: both confirm. `invalidate_simulation` lives on `AppController`,
  so `AppState::apply` raises a flag. The CLI part is refuted (see section 7).

#### F101 INC-07 Five content-digest recipes for one job
D · M · CONFIRMED · new · in flux · R7 member · also F079
- Sites: `compute/simulate.rs:912,921,939,983`, `compute/sim_prefix.rs:119,380,405`,
  `machine/head.rs:399`, `compute/source_stock.rs:149`,
  `compute/toolpath_stats.rs:581`, `session/mod.rs:911`, `compute/obstacle.rs:223`,
  `compute/stock_change.rs:168`, `maps/finish_surface_cache.rs:168`,
  `stock/simulation_cut.rs:131`.
- Evidence: `SimulationProvenance.tool_hashes` has no reader and hashes no
  cutter shape.
- Fix: CONVENTION → TYPES. One `Digest` type and a `Digestible` trait with
  exhaustive destructures.
- Verifier: confirm. Serde and Debug recipes take new fields on their own;
  only the hand lists miss them. First step: delete `tool_hashes`, merge
  `hash_f64_bits` and the two FNV loops.

#### F102 INC-08 Engine Params defaults: three conventions; finish keeps a second copy
D · M · CONFIRMED · new · in flux · R25 · also F073, F174
- Sites: `finish/unified_finish.rs:255,262,275`, `compute/operation_configs.rs:1517,2160,2329,2704`,
  `finish/pencil.rs:227,345`, `ops/project_curve.rs:125`,
  `finish/ramp_finish.rs:228`, cli `nc_replay.rs:51`.
- Evidence: Pencil 0 vs 1 offset passes, feed 1000 vs 800; Project Curve and
  Ramp Finish differ too. Tests on `XParams::default` measure values no
  surface ships.
- Fix: CONVENTION → TYPES. Delete engine `Default`, or implement it as
  `XConfig::default().to_params(test ctx)`.
- Verifier: confirm. `ops/project_curve.rs:125` has a Default (claim "ops have
  none" is wrong). `nc_replay.rs:51` hardcodes a preset (breaks cli
  CLAUDE.md:31): file it apart.

#### F103 INC-09 Mirror enums convert by From, by a free fn, and by inline copies
D · S · CONFIRMED · new · in flux · R24 member
- Sites: `feeds/vendor_normalize.rs:152`, `gcode/mod.rs:743,753`,
  `tool_load/mod.rs:310`, viz `state/simulation.rs:470`, `feeds/mod.rs:304`,
  `feeds/vendor_lut.rs:270`.
- Fix: CONVENTION → TYPES. `From` impls or merge each pair.
- Verifier: confirm. The EntryStyle sub-claim is the normal config/engine
  split: drop it.

#### F104 INC-10 Three op-type token parsers and two workspace-key parsers
D · S · CONFIRMED · new · in flux · R2 member · also F083
- Sites: cli `run.rs:271`, `smoke.rs:1074`, `rs_cam_mcp/src/server.rs:1971`,
  viz `app/mcp.rs:1054`, `app.rs:375`, `geometry/boundary.rs:15`,
  `compute/config.rs:402`, `session/compute.rs:131`,
  `session/compute/generation.rs:889,931`.
- Fix: CONVENTION → TYPES. `OperationType::from_kind_str` in `for_each_op!`;
  delete `ToolContainment`.
- Verifier: confirm. Cosmetic drift; workspace key at SENTRY at best.

#### F105 INC-11 Three mark-above-stock-top helpers with two lifts and a literal
D · S · PLAUSIBLE · new · in flux · below cut
- Sites: viz `interaction/picking.rs:277,290`, `state/pin_view.rs:21,57`,
  `app/gpu_upload.rs:474,529`.
- Fix: use `HOLE_MARK_LIFT_MM` in `gpu_upload.rs`.
- Verifier: all three find the top the same way; two `gpu_upload` sites are
  not stock tops. Tier F fits.

#### F106 INC-12 Two cancellation vocabularies; FlagCancel has no production caller
E · M · CONFIRMED · Q5 (related; covered NeverCancel) · in flux · below cut
- Sites: `interrupt.rs:55`, `session/compute.rs:390`,
  `compute/simulate.rs:1614,1774`, `compute/collision_check.rs:104`,
  `compute/execute.rs:427`.
- Fix: CONVENTION → TYPES. `&dyn CancelCheck` below the lane; `FlagCancel` at
  the door.
- Verifier: confirm; close to F.

#### F109 INC-15 Five Severity vocabularies; the MCP banner re-derives the threshold by string
E · M · CONFIRMED · new · in flux · below cut
- Sites: `diagnostics/mod.rs:143`, `export/gcode_validator.rs:43`, viz
  `controller.rs:72`, `session/diagnostics_types.rs:104`,
  `diagnostics/adapters/from_project_diagnostics.rs:68`,
  `tool_load/drill_gates.rs:80`, viz `app/mcp.rs:749`.
- Fix: CONVENTION → TYPES. Filter on `Severity::is_actionable()` (it exists)
  before serialisation.
- Verifier: confirm. Do not merge all five enums.

#### F110 INC-16 Error-type style differs inside core and per crate
E · M · CONFIRMED · new · in flux · R17 member
- Sites: `session/mod.rs:333`, `compute/execute.rs:238`, `gcode/mod.rs:56`,
  `compute/simulate.rs:895`, `io/mod.rs:79`, viz `error.rs:50,56`,
  `rs_cam_mcp/src/server.rs:1971`, `rs_cam_mcp_proxy/src/proxy.rs:1064`.
- Fix: CONVENTION → TYPES for `VizError::Session(#[from] SessionError)`.
- Verifier: confirm. 18 hand impls in core. Today a non-load refusal shows
  "Load failed:".

#### F111 INC-17 ProjectSession lookup names: get_ means by index and by id
F · M · CONFIRMED · new · in flux · R19 member
- Sites: `session/mod.rs:1522,1542,1547,1552,1614,1619,1745,1797,1836`.
- Fix: `*_at(index)` and `*_by_id(Id)`; TYPES only with F004.
- Verifier: several hundred call sites, not 120.

#### F112 INC-18 Module layout: 44 mod.rs against 19 foo.rs + foo/
F · M · CONFIRMED · new · in flux · below cut
- Sites: `compute/execute.rs`, `compute/catalog.rs`, `session/compute.rs`,
  `session/mutation.rs`, `finish/scallop.rs`, `feeds/suggest.rs`, viz
  `app.rs`, `controller.rs`, `app/mcp.rs`, `compute/worker.rs`, `Cargo.toml:36`.
- Fix: NONE → LINT. Move the 19 after the in-flux sessions land; deny
  `self_named_module_files`.
- Verifier: confirm.

#### F203 One runtime-row creator hard-codes auto_regen = true
E · S · CONFIRMED · new · in flux (`ui/properties/mod.rs`) · below cut
- Claim: `toolpath_rt_or_default` creates a missing row with
  `ToolpathRuntime::new(true)`. The documented rule says every creator reads
  the catalog default.
- Sites: viz `state/runtime.rs:353-356`; callers
  `controller/events/compute.rs:36,56,295,410,497`. The panel snapshot
  `ui/properties/mod.rs:275` uses `new(true)` as a display default only (it
  writes back only to a row that exists, `:862`).
- Rule: `state/stale.rs:35-41` says twelve of the twenty-four operations
  declare `false`, and "a row hardcoded to `true` would queue a 3D finishing
  pass nobody asked for". The creators at `controller/io.rs:470`,
  `controller/events/toolpath.rs:219,279`, `controller/events/planner.rs:91`,
  `app/mcp/commands.rs:2414` and `state/stale.rs:58` obey it.
- Evidence: load and add create a row for every toolpath, so the fallback
  did not fire on the paths read. A new door that skips row creation makes
  `compute.rs:410` create a `true` row.
- Fix: DOC → TYPES. `ToolpathRuntime::new` takes the `OperationConfig`;
  delete the `true` fallback. Map: `viz-rest-and-small-crates.md` lead 2.

#### F211 Two lock-poison policies
D · S · CONFIRMED · new · not in flux · below cut
- Claim: the trace recorders panic on a poisoned lock; the viz worker and
  the settings global recover the value.
- Sites: `trace/debug_trace.rs:221,286,320,354,448,460` and
  `trace/semantic_trace.rs:668,695,728,748` (`lock().expect("... poisoned")`
  under `#[allow(clippy::expect_used)]`); viz `compute/worker.rs:472,485,637,682`
  and `settings/paths.rs:183-185,194` (`into_inner`).
- Evidence: the recorders run on the generation worker. No GUI panic path
  was proven, so this is an inconsistency only.
- Fix: CONVENTION → single source. One rule in `CLAUDE.md` and one helper;
  the recorders take the `into_inner` form.

### 4.9 architecture-layering

#### F113 Three surfaces build the project verdict; MCP says OK over holder collisions
A · S (interim) then M · CONFIRMED · L11 (core verdict key retired; no owner since) · not in flux · R4 · also F066, F184
- Claim: the MCP verdict chain has no holder-collision arm. A project with
  holder collisions and no rapid collision reads `ERROR` in the CLI and `OK`
  in MCP, next to a non-zero `collision_count` in the same reply.
- Sites: viz `controller/events/compute.rs:2467,2491-2494`, cli
  `project.rs:960,964,971,981`, viz `ui/sim_diagnostics.rs:179,341`,
  `session/diagnostics_types.rs:334`, `compute/catalog/capabilities.rs:362`,
  viz `ui/properties/operations/registry.rs:421`,
  `diagnostics/adapters/from_static_checks.rs:246`, cli `job.rs:854`,
  `compute/cutter.rs:8`.
- Evidence: the CMP-14 comment claims the same order as the CLI; the code
  breaks it. 40.0 is typed three times beside `AIR_CUT_BAR_CLEARING_PCT`.
- Fix: NONE → TYPES. Interim (S): add the holder arm. End:
  `ProjectDiagnostics::verdict() -> ProjectVerdict`, rendered by exhaustive
  matches. Core must hold the rapid count and air share first.
- Verifier: both confirm A. The GUI verdict line is built from the triage, so
  two surfaces share a ladder, not three. The stepover and `build_tool`
  sub-items are separate (F117, F116).

#### F114 Two apply-Suggest funnels: GUI/MCP Apply skips the drill defaults the CLI writes
B · M · CONFIRMED · §15 ruling 4 holdout (`session/command.rs:50-55`); Q1 remainder covers only model_bbox · in flux · R11
- Sites: viz `controller/events/mod.rs:687,769`, `app/mcp/generation.rs:861`,
  cli `project.rs:1140`, `session/mod.rs:1689`, `feeds/suggest.rs:925`,
  `feeds/suggest/apply.rs:821`, `session/command.rs:50`.
- Evidence: `apply_drill_defaults` has one production caller; GUI context has
  `stock: None`. Drill and AlignmentPinDrill `peck_depth` differs by surface.
- Fix: CONVENTION → TYPES. `Command::ApplyFeeds { index, scope, explored }`.
- Verifier: both confirm. First step: `suggest::apply` runs
  `apply_drill_defaults`, and a core `suggest_context(tc)` sets the stock.

#### F115 Feed modulation writes session.results outside apply(Command)
B · M (L for rt.result deletion) · CONFIRMED · new · in flux · R9 member
- Sites: `session/compute/simulation.rs:349,411,625`, viz
  `controller/events/compute.rs:612,885,922`, `app/gpu_upload.rs:1040`,
  `render/toolpath_render.rs:502`.
- Evidence: viz `rt.result` keeps the planned toolpath; the feed-coloured
  viewport shows planned feeds while export emits modulated feeds (read, not
  run). `io/export.rs:279-300` says so.
- Fix: DOC → TYPES. Modulate inside `Command::AdoptSimulation`; make
  `modulate_simulation_trace` `pub(crate)`.
- Verifier: both confirm. The epoch guard stops stale runs. Smaller fix:
  report modulated ids in Effects and re-point `rt.result`.

#### F116 The CLI job pipeline is a parallel flow
B · L · PLAUSIBLE · new · in flux · R10 member · also F069
- Sites: cli `job.rs:464,486,854,1008,1053,1129`, `compute/cutter.rs:8`,
  `gcode/mod.rs:376`, `rs_cam_cli/CLAUDE.md:13`, `compute/tool_config.rs:215`.
- Evidence: the job exports with `ToolLoadReport { per_toolpath: vec![] }`;
  `build_tool` omits `helix_deg` and `corner_radius_mm`, hard-codes Carbide,
  and defaults stickout to 4×D while the session uses 45 mm.
- Fix: DOC → TYPES. Step 1 (S): build the phase cutter with core
  `build_cutter(&tool_config_from_def(..))`. End: one job session, export
  through `export_gcode_checked`.
- Verifier: the empty report is sanctioned by the core door doc
  (`gcode/mod.rs:1160-1168`), which contradicts the CLI CLAUDE.md. The datum
  claim is refuted (the job has no datum). Making the phase door `pub(crate)`
  breaks three viz callers; do it last.

#### F117 Pre-generation refusals live in the viz UI folder
B · M · PLAUSIBLE · new · in flux · R4 member · also F068
- Sites: viz `ui/properties/operations/validate.rs:71,85,174`,
  `operations/registry.rs:421`, `controller/events/compute.rs:231,233`,
  `diagnostics/adapters/from_static_checks.rs:245,246`,
  `diagnostics/adapters/from_model_refs.rs:51`.
- Evidence: the GUI refuses `stepover >= diameter` for Pocket and Adaptive;
  core flags `>` on every op. At equality GUI/MCP refuse and the CLI generates.
  The geometry check is written twice in `validate.rs` and the texts differ.
- Fix: CONVENTION → TYPES. One core stepover predicate; the submit gate reads
  core static diagnostics.
- Verifier: "core has no equivalent" is overstated: the resolver refuses a
  missing mesh. The live divergence is the stepover rule.

#### F118 MCP wire patch structs are hand-kept partial copies of core args
B · M · CONFIRMED · new · in flux · R7 member · also F180
- Sites: `rs_cam_mcp/src/server.rs:758`, viz `app/mcp/commands.rs:1107,1118`,
  `compute/stock_config.rs:69`, `session/command.rs:383,439`,
  `session/compute/params.rs:531`, `compute/operation_configs.rs:1171`, cli
  `job.rs:1053`.
- Evidence: MCP cannot set `padding` or `post.format`; the registry says
  `mcp: Reach::Reached`. `set_tool_param` cannot set `helix_deg`,
  `corner_radius_mm`, `tool_material`, `cut_direction`. The
  `trochoid_cap_mult` doc claims a GUI dial that does not exist.
- Fix: NONE → TYPES. Core `StockConfigPatch` with an exhaustive destructure
  in `apply_to`; a `ToolParamKey` enum.
- Verifier: both confirm. The Reach census needs a field-level check.

#### F120 Surfaces hold copies of core domain rules
D · M · CONFIRMED · D12 (merged two mesh-Z readers; the gpu_upload copy is a third) · in flux · R9 member
- Sites: viz `state/simulation.rs:286`, `app/mcp/commands.rs:2775,2933`,
  `session/mutation/config.rs:249`, viz `ui/properties/toolpath_panel.rs:930`,
  `app/gpu_upload.rs:463`.
- Fix: DOC → TYPES. Delete `core_stale_rest_source` and use
  `core_stale(&effects)`; a core `project_curve_preview_z`.
- Verifier: confirm. The MCP stale set is a superset of core Effects. The
  preview ignores `setup_z_flipped`.

#### F121 gcode/mod.rs owns session-level policy
E · M · CONFIRMED · new · in flux · R10 member
- Sites: `gcode/mod.rs:22,583,608`, `export/gcode_validator.rs:39,721`,
  `gcode/emitter.rs:129`, `gcode/program_builder.rs:484`.
- Fix: NONE → CONVENTION. Move `project_load_report` and the freshness rule
  to `session/compute/export.rs`; move `strip_comments` into `gcode/`.
- Verifier: confirm (compute is named 18 times, not 8).

#### F122 Core layering is prose only: 42 folder-level import cycles
E · L · CONFIRMED · new · in flux · below cut
- Sites: `lib.rs:3`, `compute/validate.rs:44`, `compute/simulate.rs:180`,
  `compute/obstacle.rs:326`, `tool/mod.rs:23`, `feeds/mod.rs:75`, and others.
- Fix: DOC → SENTRY. Allowed-edge table and a cycle-count ratchet; move
  `ToolGeometryHint` to `tool/` and `ToolMaterial` out of `compute`.
- Verifier: confirm; 42 is a lower bound (grouped `use` forms are missed).

#### F123 Core has no facade: 34 pub mod and 689 deep paths
E · L · CONFIRMED · S29 (items demoted one by one; no module facade) · in flux · below cut
- Sites: `lib.rs:30`, `compute/mod.rs:30`, `feeds/mod.rs:1`.
- Fix: NONE → TYPES. One folder at a time, starting with `feeds::suggest`
  and `compute::simulate`.
- Verifier: confirm.

#### F124 The toolpath IR file depends on planner types and hosts raster generators
E · S-M · CONFIRMED · new · not in flux · below cut
- Sites: `toolpath.rs:7,8,613,779`, `compute/execute/finish_raster.rs:248`,
  `finish/unified_finish/band_kernels.rs:352`.
- Fix: NONE → SENTRY. Move both generators to `surface/`.
- Verifier: about 20 test files import the old path.

#### F125 viz state/, render/, io/ and controller/ import the ui/ folder
E · M · CONFIRMED · new · in flux · below cut
- Sites: viz `state/mod.rs:198`, `state/panels.rs:176`, `state/runtime.rs:300`,
  `state/app_settings.rs:79`, `state/pin_view.rs:82`, `render/mod.rs:1071`,
  `io/setup_sheet.rs:57`, `controller/events/undo.rs:184`, `model.rs:11`,
  `mod.rs:487`, `ui/mod.rs:80`.
- Fix: NONE → SENTRY. Move `AppEvent` and readiness first.
- Verifier: confirm, 188 lines.

#### F126 compute::validate::apply_stale_default_fix is a public session mutator outside apply
B (contested: C) · S · CONFIRMED · new · in flux · R9 member · also F043, F169
- Sites: `compute/validate.rs:164,182`, `session/mod.rs:1809`,
  `diagnostics/fix.rs:30,31`, `tests/setters_are_crate_private_wp15b.rs:19`.
- Evidence: it writes through `toolpath_configs_mut` with no Command and no
  result drop; only its tests call it; `diagnostics/fix.rs` tells consumers to
  call it. The wp15b scanner reads only `impl ProjectSession` blocks.
- Fix: NONE → TYPES. Delete it and its doc pointers; then delete or gate
  `toolpath_configs_mut` and update the wp7/wp15a allowlists.
- Verifier: one says C (dead in production), one says B (a recommended pub
  door that leaves a stale result).

### 4.10 complexity-size

#### F128 Dressup order lives in the statement order of a 470-line apply_dressups
B · M · CONFIRMED · new · in flux · R15 · also F190
- Sites: `compute/execute/dressup_apply.rs:253,471,538`,
  `tests/dressup_span_invariants.rs:438`, `compute/execute.rs:557`, viz
  `ui/properties/operations/registry.rs:130`.
- Evidence: cold-start task 3 records this silent miss. `RAPID_ORDER` appears
  twice in the list; the entry slot is a three-way choice.
- Fix: SENTRY → TYPES. `enum DressupStep`, const `PIPELINE`, one fn per arm;
  keep the sentry for position. `OP_UI_ROWS::row()` via an exhaustive match.
- Verifier: both confirm. Move the history comment (`:538-573`) out of the body.
- Re-check after the tabs work (2026-10-19): `dressup_apply.rs` has no
  change since the baseline; the tabs are not a dressup stage. The
  `compute/execute.rs:557` re-export is now `:562` (e76f82c8 added 4 lines
  at `:158` and 1 at `:355`). The claim holds.

#### F129 27 stamping/simulation signatures pass a radial LUT and a separate radius
D · M · CONFIRMED · new · partly in flux (`compute/simulate.rs`) · below cut
- Sites: `dexel_stock/stamping.rs:783,809`, `dexel_stock/simulation.rs:66,414`,
  `dexel_stock/swept.rs:332`, `dexel_stock/playback.rs:280`,
  `adaptive3d/clearing.rs:641`, `compute/simulate.rs:1583`,
  `stock/radial_profile.rs:23`.
- Fix: NONE → TYPES. Store `radius` in the LUT; drop the parameter.
- Verifier: confirm; all callers pass the matching radius today.

#### F130 Entry-point ladders: each new knob adds a sibling fn
E · L · CONFIRMED · Q10 · partly in flux · below cut · also F107
- Sites: `dexel_stock/simulation.rs:39,66,82,323,361,414`,
  `compute/simulate.rs:1759`, `finish/scallop.rs:1060,1092`,
  `finish/scallop/research.rs:215`, `adaptive3d/mod.rs:488,499,536`,
  `finish/finish_setup.rs:380`, `finish/ramp_finish.rs:501`,
  `adaptive/mod.rs:348`, `geometry/boundary.rs:164`.
- Fix: CONVENTION → TYPES. One request type per engine; delete test-only rungs.
- Verifier: 147 `too_many_arguments` allows, not 136. Do the safe parts first.

#### F131 feeds::calculate is 1321 lines with 23 mutable locals
F · L · CONFIRMED · T-5 (then 1302 lines, 16 locals) · not in flux · below cut · also F135, F149
- Sites: `feeds/mod.rs:1494,1530`, viz `app/gpu_upload.rs:155`,
  `app/mcp/commands.rs:2031`, `Cargo.toml:35`, and the long-fn list.
- Fix: NONE → LINT for growth. Split `effective_d`; extract the power ladder
  and the plunge/ramp tail; `too_many_lines` with a `clippy.toml` threshold.
- Verifier: 74 src files over 1500 lines (12 over 3000). The allow on
  `ToolpathTab` does nothing.

#### F132 upload_gpu_data is one 1327-line fn with about twelve keyed upload slots
B · M · CONFIRMED · new · in flux · R14
- Sites: viz `app/gpu_upload.rs:155,218,263,662,821,986`, `render/upload_cache.rs:1`.
- Evidence: c84ef3be (caches keyed on `edit_counter`) is a past break of this
  class. `upload_cache.rs:24-26` states the rule as prose.
- Fix: CONVENTION → TYPES. Per-slot `fn inputs(&AppState) -> Option<Inputs>`,
  `impl From<&Inputs> for Key`, build takes `Inputs`.
- Verifier: confirm, tier B by the past break. Keep the SHL-05 no-allocation
  compare in the collision slot.

#### F133 Grids and bounding boxes travel as anonymous tuples
E · S · CONFIRMED · new · in flux · below cut
- Sites: `adaptive3d/clearing.rs:419,1237`, `adaptive3d/path.rs:572`,
  `polygon.rs:186,1750`, `adaptive/material_grid.rs:655`,
  `geometry/grid2.rs:75`.
- Fix: NONE → TYPES. `MaterialMask { cells: Grid2<bool>, origin, cell_size }`.
- Verifier: confirm; keep the NaN semantics of `bbox`. Overlaps F007.

#### F134 The debt instrument mis-measures function and file size
E · S · CONFIRMED · Q11 · in flux (`scripts/debt_scan.py` has edits) · below cut
- Sites: `instruments/inventory.md`, `scripts/debt_scan.py`,
  `planning/tech_debt_2026-09-16/findings/Q_allows_todos_size.md:178`.
- Fix: NONE → SENTRY. Brace-match fn bodies; cut at `#[cfg(test)]`.
- Verifier: confirm (`strip_all` is a 6-line const fn ranked at 1015).

#### F136 app/mcp/commands.rs is 3197 lines; describe_core 899 and core_command_for 505
F · M · CONFIRMED · new · in flux · below cut · also F137
- Sites: viz `app/mcp/commands.rs:335,2031`, `commands/*.rs`,
  `controller/events/compute.rs:675,1547,2345`, `controller/generate_all.rs`.
- Fix: TYPES kept. One-line arms into per-family files.
- Verifier: `core_command_for` matches `CoreRequest`, not `CommandId`.

#### F196 impl ProjectSession is spread over 25 files
F · M · PLAUSIBLE · new · in flux · below cut
- Claim: 29 `impl ProjectSession` blocks sit in 25 files under `session/`
  (`rg -c '^impl ProjectSession' crates/rs_cam_core/src`). A reader finds a
  method only by search.
- Sites: `session/mod.rs`, `session/command.rs`, `session/compute.rs`,
  `session/compute/{generation,simulation,export,diagnostics,params}.rs`,
  `session/mutation/{config,toolpath,entities,stock_change}.rs`,
  `session/multitool.rs`, `session/pins.rs`, `session/selection_status.rs`
  and 11 more.
- Evidence: the split follows the folder map in `session/CLAUDE.md` (Files),
  so it is navigable by file name. PLAUSIBLE as debt: the cost is method
  lookup, not a defect.
- Fix: CONVENTION → CONVENTION. Keep the split; the `session/CLAUDE.md`
  file map names which file owns which method family.

### 4.11 navigability-docs

#### F138 N1 Folder CLAUDE.md rules that the code already breaks
B · M · CONFIRMED · new · in flux · R21
- Claim: seven folder CLAUDE.md files and two doc comments state rules the
  code breaks today.
- Sites: viz `ui/menu_bar.rs:229` vs `ui/CLAUDE.md:29` (unguarded Run
  Simulation); `compute/execute.rs:331,566` vs `compute/CLAUDE.md:19` (pub
  `SelectionInputs`); `session/mutation/toolpath.rs:331` vs `session/CLAUDE.md:19`
  (non-pure walk); `finish/pencil/emission.rs:792` vs `finish/CLAUDE.md:30`;
  viz `render/toolpath_render.rs:228` vs `render/CLAUDE.md:22`; cli
  `nc_replay.rs:51` vs `rs_cam_cli/CLAUDE.md:31`; `dressup/CLAUDE.md:8`;
  cli `command.rs:15`; `geometry/boundary.rs:32` vs `session/compute.rs:146`.
- Fix: DOC → TYPES where possible. `SelectionInputs` `pub(crate)` (not
  prologue-private: three modules name it); the buildable check inside the
  RunSimulation handler, or a token only the check can mint; plain
  `effective_boundary` private. Correct the other texts; drop call counts.
- Verifier: both confirm B.

#### F139 N2 User-facing text still says 'Kc' after the Kc model was removed
E · S · CONFIRMED · new · not in flux · below cut
- Sites: `tool_load/deflection.rs:77,298`, `tool_load/verdict.rs:39,225`,
  `material/CLAUDE.md:24`.
- Fix: rewrite the four strings in force-line terms (DOC).
- Verifier: both re-grade to E: text only; the code obeys B6.

#### F140 N3 Folder file maps and symbol names point to the wrong place
E · M · CONFIRMED · new · in flux · R21 member
- Sites: `ops/CLAUDE.md:4,19`, `tool/CLAUDE.md:10`, `export/CLAUDE.md:3`,
  `metrology/CLAUDE.md:4`, `compute/CLAUDE.md:5`, `session/CLAUDE.md:7`, viz
  `ui/CLAUDE.md:3`, `app/CLAUDE.md:10`, `FEATURE_CATALOG.md:23,32`.
- Fix: NONE → SENTRY. Every backticked `.rs` resolves; every `.rs` in the
  folder is listed or excluded.
- Verifier: confirm. `dressup/CLAUDE.md:23`, `pins/CLAUDE.md:9` and
  `session/CLAUDE.md:32` are correct: drop them.

#### F141 N4 The 40-line CLAUDE.md limit is met by line width, not size
E · S · CONFIRMED · new · in flux · below cut
- Sites: `compute/CLAUDE.md:22` (one line is 1592 chars), `feeds/CLAUDE.md`,
  `tool_load/CLAUDE.md`, `session/CLAUDE.md`, `ops/CLAUDE.md`.
- Fix: CONVENTION → SENTRY. A byte cap (about 3 KB) per file.
- Verifier: confirm.

#### F142 N5 Broken intra-doc links and no rustdoc lint in the gate
E · S · PLAUSIBLE · TECH_DEBT_PLAN W4 residue 3 (160 private + 95 unresolved; still unguarded) · in flux · R21 member
- Sites: `ops/waterline.rs:78`, `finish/finish_setup.rs:15`,
  `diagnostics/mod.rs:33,247`, `Cargo.toml`.
- Fix: NONE → LINT. `broken_intra_doc_links = "deny"` plus a `cargo doc` gate
  row (the lint does nothing without that row).
- Verifier: `Self::rename_setup` and siblings resolve (pub(crate)): drop them.

#### F143 N6 Doc comments and module docs describe a past state
E · M · CONFIRMED · new · in flux · below cut
- Sites: `tool_load/verdict.rs:232`, `compute/execute.rs:580`,
  `compute/toolpath_stats.rs:4`, `material/mod.rs:556`, `session/command.rs:1109`,
  `finish/finish_setup.rs:1`, viz `lib.rs:28`, `app.rs:772`, `ui/theme.rs:8`,
  `session/mod.rs:1`, `diagnostics/mod.rs:30`, `compute/mod.rs:3`, `lib.rs:3`.
- Fix: correct each line; delete counts, line numbers and versions (DOC; LINT
  for links only).
- Verifier: confirm (13 finding types, not 12).

#### F144 N7 feeds/INTEGRATION.md is a second, stale description of the feeds flow
D · S · CONFIRMED · new · not in flux · R21 member
- Sites: `feeds/INTEGRATION.md:9,29,31`, `feeds/suggest.rs:1006`.
- Fix: delete it; one pointer line in `feeds/CLAUDE.md`.
- Verifier: confirm.

#### F145 N8 No module-doc discipline and no missing_docs lint
F · M · CONFIRMED · new · in flux · below cut
- Sites: viz `ui/properties/mod.rs`, `render/mod.rs`, `controller/events/mod.rs`,
  `compute/mod.rs`, `state/mod.rs`, `ui/mod.rs`, `lib.rs`; `rs_cam_mcp/src/lib.rs`;
  cli `main.rs`.
- Fix: NONE → SENTRY then LINT. First: every `lib.rs`/`mod.rs` starts with `//!`.
- Verifier: F, not E. 86 of 620 files lack a module doc.

#### F146 N9 Live product code sits in a file named research
E · S · CONFIRMED · new · in flux · below cut
- Sites: `finish/scallop/research.rs:84`, `compute/execute/finish_raster.rs:119`,
  `finish/scallop.rs:18`, `finish/CLAUDE.md:17`.
- Fix: NONE → TYPES. Live entries to `scallop/iso_field.rs`; gate the
  research-only builders after an `rg` of their callers.
- Verifier: confirm.

#### F147 N10 path:line citations in comments rot
F · M · CONFIRMED · new · in flux · below cut
- Sites: `session/command.rs:1109` and 33 test-file citations past EOF.
- Fix: CONVENTION → LINT. Cite a symbol; intra-doc links in src.
- Verifier: confirm, 389 citations.

#### F148 N11 Code cites purged planning/ paths, some not retrievable from the purge tag
F · M · CONFIRMED · new · in flux · below cut
- Sites: `session/mod.rs:1077,1133`, `adaptive3d/path.rs:110`,
  `adaptive3d/clearing.rs:1194`, `stock/simulation_cut.rs:205`,
  `stock/simulation_cut/analysis.rs:132`, `stock/simulation_cut/accumulate.rs:117`,
  `trace/toolpath_spans.rs:575`.
- Fix: DOC → SENTRY. Copy the reason inline; point the RCA at
  `planning/archive/P3_TRANSIT_PEAK_DOC_RCA.md` in the tag and the
  acceptance_loop files at the parent of 0d559359; inline the text of
  `adaptive_review_2026-04.md` (never committed).
- Verifier: 165 references; 68 distinct paths by the verifier regex, not 106.
  F-039 is also irretrievable.

#### F150 N13 Two names for one place
F · M · PLAUSIBLE · new · in flux (verifier correction) · below cut
- Sites: `session/mutation.rs`, `stock/dexel.rs`, viz
  `ui/properties/toolpath_panel.rs`, `ui/toolpath_panel.rs`,
  `feeds/extrapolation/plunge.rs`, viz `ui/theme.rs`.
- Fix: rename by role; delete `theme.rs` (with F089).
- Verifier: `mutation.rs` beside `mutation/` is idiomatic: leave it.

#### F194 Hard-coded counts and stale 'how to add' prose
F · S · CONFIRMED · new · in flux · R8 member
- Sites: `tests/every_cell_declares_its_feeds_support_fm0.rs:191,199`,
  `compute/execute.rs:306`, viz `state/job.rs:620,626`,
  `architecture/high_level_design.md:141`.
- Fix: SENTRY → TYPES. `OperationType::ALL` is already generated: delete the
  `== 24` now and write the matrix size from `ALL.len()`. Core exhaustive
  matches for `ToolType` and `PostFormat` replace the viz count tests.
- Verifier: confirm.

#### F201 Unsourced numbers drive production feeds
B · M · CONFIRMED · new (T-16 is a sibling row, not this one) · not in flux · R22b
- Claim: the formula path of the feeds calculator reads cutting speeds,
  plunge baselines and DOC/WOC factors that cite no printed source in the
  repo.
- Sites: `material/mod.rs:962-981` (`base_cutting_speed_m_min`; the
  aluminium row says "conservative SFM placeholder"), `material/mod.rs:995-1010`
  (`plunge_rate_base` baselines), read in production at
  `feeds/mod.rs:1547,2545`; `feeds/mod.rs:2822-2927`
  (`operation_default_profile` factor table) and `:2939` ("From reference
  calcs.rs").
- Rule broken: root `CLAUDE.md:66` ("never type a number from memory") and
  `material/CLAUDE.md:24-26` ("Never type an envelope value or a density as a
  literal").
- Evidence: the cited `calcs.rs` is under `reference/shapeoko_feeds_and_speeds/`
  (`research/feeds_and_speeds_integration_plan.md:236`), and `.gitignore:24`
  excludes `/reference/`, so the source is not in the repo. The fiberglass
  speed cites a vendor row midpoint (`material/mod.rs:974-979`), the only
  partial source.
- Fix: DOC → SENTRY. Each constant names its printed source in the repo,
  or the formula path refuses (as the force line refuses with
  `ForceLineRefusal`); a sentry like `wood_species_library_provenance`
  checks the tables.
- Fold note: the critic also cited `compute/tool_config.rs:928`
  `NAME_MATCH_REL_TOL`. It is a design tolerance with its reason in the doc
  (`:924-927`), not a physical constant: dropped. Map: `core-feeds.md` leads
  at `material/mod.rs:962-1010`, `feeds/mod.rs:2822-2929`.

### 4.12 test-debt

#### F151 CI never runs: ci.yml triggers on branch main, the trunk is master
B · S · CONFIRMED · new · not in flux · R20
- Sites: `.github/workflows/ci.yml:6,24,83`, `crates/rs_cam_core/tests/CLAUDE.md:25`,
  core `Cargo.toml:16`, root `CLAUDE.md:102`.
- Evidence: no `origin/main`. `ci.yml:24` runs workspace-wide `cargo test`,
  which root CLAUDE.md forbids. Two files still say CI runs the heavy tests.
- Fix: NONE → SENTRY. Trigger on master and use the per-crate gate commands,
  or delete `ci.yml` and correct the two docs. Operator decides.
- Verifier: both confirm. `pull_request:` still triggers a run. The emulator
  step is at `:91`. Check the Actions account state too.

#### F152 #[ignore] parks known-red and retired tests; print-only tests run unignored
B · M · CONFIRMED · new · not in flux · R20 member
- Sites: `tests/a_descent_over_two_overlapping_swaths_g_staletop.rs:846`,
  `tests/adaptive3d_boundary_clear_parity.rs:744`,
  `tests/drop_cutter_off_mesh.rs:212`, `tests/wanaka_e2e_chipload_gate.rs:55`,
  `tests/adaptive_property_harness.rs:618,641`, six print-only tests,
  `tests/CLAUDE.md:27`.
- Fix: DOC → SENTRY. Inverted assertions for parked reds; delete the retired
  claim; `#[ignore = "instrument: .."]` on print-only tests; a sentry on
  ignore-reason prefixes.
- Verifier: both confirm. `adaptive3d/tests.rs:2229` is a valid instrument:
  drop it. Rewrite `drop_cutter_off_mesh.rs:212` as a bound, do not invert it.

#### F153 Named sentries pass vacuously through skip-and-return paths
B · S · CONFIRMED · new · in flux · R20 member
- Sites: `tests/dexel_stock_z_frame_f026.rs:136`,
  `test_data/ux_2d_pocket_softwood.toml:15`,
  `tests/capability_link_moves_safety.rs:829`,
  `tests/machine_kinematics_cycle_time_f034.rs:284`,
  `tests/feed_modulation_cycle_time_f036c.rs:161`.
- Evidence: the f026 fixture had `auto_from_model = false` from its first
  commit (c1121c65), so the test never checked F-026; its docstring says true.
- Fix: NONE → SENTRY. Set the flag true in the fixture (it is a 2D test; do
  not swap to a 3D fixture) and `.expect` the load; a panicking
  `require_fixture`; give the pencil arm a mesh that yields chains.
- Verifier: both confirm. The f034/f036c skips are latent only (fixture present).

#### F155 Gate tests load operator work files from planning/ and absolute home paths
B · S · CONFIRMED · new · in flux · R20 member
- Sites: viz `controller/tests/undo.rs:1382`,
  `tests/test_data_smoke_csv_alignment.rs:50`, 23-25 core tests with
  `/home/ricky` paths, `tests/CLAUDE.md:13`.
- Evidence: `testpiece_mini.toml` was edited three times on 2026-10-16 and
  holds seven absolute model paths under `/home/ricky/cnc_jobs`.
- Fix: DOC → SENTRY. Copy the project and its models into `tests/fixtures`
  with relative paths; a sentry refuses `planning/` and `/home/` literals in
  non-ignored tests; one rule for `planning/fixtures` vs `tests/fixtures`.
- Verifier: both confirm.

#### F156 Source-scan sentries re-check rules that visibility now enforces
E · M · PLAUSIBLE · T-2, T-3 · in flux · R20 member
- Sites: viz `tests/production_writes_go_through_apply_wp15a.rs:37,444`,
  `tests/setters_are_crate_private_wp15b.rs:22`, viz
  `tests/egui_draw_sites_write_through_commands_wp6.rs:15`,
  `tests/hatches_are_crate_private_wp7.rs`.
- Changed since T-2/T-3: WP7 and WP15b made the hatches and setters
  `pub(crate)`, so three scan arms now re-check what the compiler enforces.
- Fix: delete wp15b arm 2, wp15a property 1 and wp6 scan 1; keep wp15b arm 1.
- Verifier: `compile_fail` doctests do not run under `--lib` and pass on any
  error; pair each with a compile-pass twin, or put setters behind a sealed
  token. Counts depend on method (about 103-106 files).

#### F158 Three fixture builders encode the same stock-below-Z0 frame knowledge
D · M · CONFIRMED · new · in flux · below cut
- Sites: viz `compute/worker/test_fixture.rs:10`, `compute/worker/tests.rs:31`,
  `tests/common/op_fixtures.rs:12`, `tests/common/session.rs:213`.
- Fix: CONVENTION → TYPES. `StockConfig::top_at_z0(x, y, thickness)` in core.
- Verifier: confirm; 47 private helpers, 9 `ball_tool` copies. Do not merge
  them in bulk.

#### F159 41 instrument-only test binaries build in every core test run
F · M · CONFIRMED · new · not in flux · below cut
- Sites: `tests/param_sweep.rs:443`, `tests/wanaka_axial_doc.rs:31`,
  `tests/tier_trial_350.rs`, core `Cargo.toml:169`, `tests/CLAUDE.md:3`,
  `compute/execute/unified_finish_ring_collapse_g_unifiedcrash.rs`, viz
  `controller/results_parity_tests.rs`.
- Fix: DOC → SENTRY. An `instruments` feature (then clippy needs it) or one
  merged instruments binary; fix the "about 330" count (495).
- Verifier: confirm.

#### F204 A viz test module does not compile in the build without mcp
C · S · CONFIRMED · new · not in flux · R20 member
- Claim: the test that guards the no-`mcp` failure path calls `run` with
  one argument; `run` takes two. Nobody builds that configuration.
- Sites: viz `src/lib.rs:219-224` (`#[cfg(all(test, not(feature = "mcp")))]`,
  `super::run(true)`), `src/lib.rs:48` (`run(mcp_mode, settings)`),
  `Cargo.toml:40` (`default = ["mcp"]`).
- Evidence: `cargo test -p rs_cam_viz --no-default-features` must fail to
  build (read from code; cargo did not run). The test reads as a guard and
  never runs.
- Fix: NONE → SENTRY. Pass a default `LoadedSettings`, and add the
  no-default-features build to the viz gate, or delete the module. Map:
  `viz-rest-and-small-crates.md` lead 4.

#### F206 The derive crate that TH-DOMAIN extends has no tests
E · S · CONFIRMED · new · not in flux · R6 member (prerequisite)
- Claim: `rs_cam_derive` (433 lines) has no `#[test]`, no `trybuild` and no
  `compile_fail` doctest. F185 and F163 add a `compile_error!` to it.
- Sites: `rs_cam_derive/src/lib.rs`; the `compile_fail` pattern exists at
  `measurement.rs:433` and `trace/transform_provenance.rs:309`.
- Evidence: core tests check the derive output indirectly
  (`tests/schema_enum_values_g_schemaenum.rs`), but nothing proves that a
  wrong attribute fails to compile.
- Fix: NONE → SENTRY. `compile_fail` doctests (or `trybuild`) for each
  refusal before W6 adds one. Map: `viz-rest-and-small-crates.md` lead
  "rs_cam_derive has no tests".

### 4.13 enforcement-gap

#### F161 Enum dispatch fails open: _ =>, matches! and == swallow a new variant
B · L · PLAUSIBLE · new · in flux · R8 member
- Sites (real per-variant decisions only): `tool/mod.rs:330,396`,
  `feeds/suggest/apply.rs:999`, `feeds/suggest/axial_envelope.rs:347`, viz
  `ui/properties/operations/shape_diagrams.rs:54`.
- Evidence: G-BULLCUSP (`tool/mod.rs:333-346`) shipped a 2.5× scallop spacing
  through this shape. `compute/CLAUDE.md:20` (W2.4) says a consumer never
  matches on the op type.
- Fix: NONE → TYPES plus LINT. Exhaustive matches in a capability or a hint
  method; `wildcard_enum_match_arm` per folder, not workspace-wide.
- Verifier: one plausible, one confirm. Drop `registry.rs:421`,
  `tool_config.rs:40`, `gcode/mod.rs:1488` (string parse),
  `project_file.rs:1418`, `mutation/config.rs:258`, `commands.rs:2955` (one
  variant filter), `export_wizard.rs:590` and `machine_panel.rs:542`. The 490
  and 815 counts are raw grep totals. Tool-shape sites fold into F182.

#### F162 Hand-kept ALL arrays beside the enum; diagnostics ids::ALL misses 8 of 102
B · M · CONFIRMED · new · in flux · R8 · also F098, F154, F177
- Sites: `diagnostics/ids.rs:248,309,416`, `compute/tool_config.rs:22`,
  `gcode/mod.rs:1435`, `feeds/mod.rs:256`, `stock/sim_measurability.rs:108`,
  `diagnostics/tests.rs:41`, viz `state/job.rs:617,620`, and others.
- Evidence: missing ids GEOM_CLIPPED_BAND, GEOM_RAMP_REACH_CLAMP,
  GEOM_OFFSET_LIBRARY_FAILURE, GEOM_BOUNDARY_CLIP_DROPPED,
  GEOM_WATERLINE_LEVELS_BELOW_STOCK, GEOM_WATERLINE_LADDER_EMPTY,
  CONFIG_DERIVED_STEPOVER, CONFIG_CLAIMS_REFERENCE. The doc promises a
  registry; only tests read it. `ToolType::ALL` drives `parse_lenient`.
- Re-count (2026-10-19, fold). Rule: an id is a `pub` or `pub(crate) const
  NAME: &str` in `diagnostics/ids.rs`; `ALL` is the list at `ids.rs:419-514`
  (comments removed). Three points in time:
  - Baseline 99b6fb41: 101 ids (100 `pub`, plus `pub(crate)
    PROJECT_KINEMATIC_UTILIZATION` at `:170`); `ALL` lists 93; 8 missing.
  - 2d8f5dbf (after e76f82c8, selection P5): 102 ids; `ALL` lists 93; 9
    missing. e76f82c8 added `GEOM_TAB_PLACEMENT` (`ids.rs:279`) and no `ALL`
    entry: the defect class repeated during the review.
  - HEAD a682e333 and the working tree: 102 ids; `ALL` lists 94; 8 missing.
    The selection session added `GEOM_TAB_PLACEMENT` to `ALL` in 16c1b78b
    (`ids.rs:492`). The 8 older ids above stay for the F162 wave (W7).
  The critic wrote "101 ids, 93 in `ALL`" for 2d8f5dbf; it counted the `pub`
  consts only.
- Fix: NONE → TYPES. A local `diag_ids! { NAME = "str", .. }` macro emits each
  const and `ALL`; an `enum_with_all!` macro or an exhaustive `ordinal()` proof
  for enums (no strum dependency today).
- Verifier: both confirm. 47 `const ALL` sites by `rg`.

#### F163 Op-param macros accept a missing decision
B · M · PLAUSIBLE · new · in flux · R6 member
- Sites: `rs_cam_derive/src/lib.rs:209`, `session/compute/params.rs:64`,
  `compute/operation_configs.rs:221,2779`.
- Evidence: the range half duplicates F185. The macro half: the
  `stepover`/`depth_per_pass`/`total_depth`/`ramp_feed_rate` clauses are
  optional and default to `None`.
- Fix: CONVENTION → TYPES. `#[param(role = stepover)]` generates the
  accessors; interim: make each clause required with an explicit `none`.
- Verifier: one plausible (counts differ: 64 vs 107), one confirm. The
  role-clause half alone is tier E.

#### F166 rs_cam_mcp wire tokens are String, against its own CLAUDE.md
B · M · CONFIRMED · new · in flux · R7 member · also F021, F099
- Sites: `rs_cam_mcp/src/server.rs:33,199,360-375,466,603,646,668,832,1530,1554,1971,1973,1989`,
  viz `app/mcp/commands.rs:432,741,1514`, `app/mcp/generation.rs:865`,
  `app/mcp.rs:95`, `ui_command.rs:179`, `mcp_bridge.rs:1211`.
- Evidence: `rs_cam_mcp/CLAUDE.md:7-11` requires an enum; 25
  `schemars(inline)` marks show the template. Viz accepts alias spellings
  against the no-legacy ruling.
- Fix: DOC → TYPES. Wire enums with `rename_all = "snake_case"` and
  `schemars(inline)`; generate the `OperationType` wire enum in `for_each_op!`.
- Verifier: both confirm. `material` is not a fixed set: keep `String`. Core
  has no `JsonSchema`: mirrors need an exhaustive `From` match.

#### F170 The // SAFETY: rule for allow(...) has no enforcer; no #[expect] in use
B · M · CONFIRMED · Q7-Q10 (below the 09-16 cut; now measured, still no enforcer) · not in flux · R22
- Sites: root `CLAUDE.md:110`, `Cargo.toml`, `scripts/debt_scan.py:93`, viz
  `mcp_server.rs:498`, `dexel_stock/simulation.rs`, `surface/slope.rs`.
- Evidence: 438 of 693 allows lack SAFETY; 98 have no comment; 0 `#[expect`.
- Fix: CONVENTION → LINT. Deny `allow_attributes_without_reason` and
  `allow_attributes`; script-convert to `#[expect(x, reason = "..")]`.
- Verifier: both confirm. Keep `#[allow(.., reason)]` where a lint fires only
  under some feature (heavy-tests, research), or `#[expect]` is unfulfilled.

#### F171 Cross-crate source-scan sentries run in no per-crate gate
B · M · CONFIRMED · T-3 (open; several scans now hold visibility rules) · in flux · R20 member
- Sites: `tests/hatches_are_crate_private_wp7.rs:118`,
  `tests/setters_are_crate_private_wp15b.rs:383`,
  `tests/stale_set_has_one_answer_wp28.rs:81`,
  `tests/air_cut_denominators_lh1.rs:269`,
  `tests/gen_inputs_one_assembly_n12.rs:248`,
  `tests/loose_executor_is_crate_private_wp12.rs:329`.
- Fix: SENTRY → TYPES for visibility rules; move the other viz scans into
  `crates/rs_cam_viz/tests`.
- Verifier: both confirm. A virtual manifest cannot hold a workspace test
  target. The `cfg(feature = "mcp")` point is refuted (default feature).

#### F172 Lint policy has no design lint for the dominant cold-start silent-miss class
E · L · CONFIRMED · Q12 (assessed six cast/float lints; new evidence disagrees) · not in flux · R22 member
- Sites: `Cargo.toml`, core `Cargo.toml:71`.
- Fix: NONE → LINT. `unsafe_code = "forbid"` now (zero overrides);
  `match_wildcard_for_single_variants` first, then per-folder
  `wildcard_enum_match_arm`.
- Verifier: confirm.

#### F173 "No literal colour in a pipeline" has a blind sentry and is broken
B (bottom of B) · M · CONFIRMED · new · not in flux · R21 member · also F100
- Sites: viz `render/toolpath_render.rs:228,471,760,839,1008`,
  `render/sim_render.rs:135,175`, `render/grid_render.rs:127`,
  `render/mesh_render.rs:269`, `ui/sim_timeline.rs:591`,
  `ui/toolpath_panel.rs:591`, `tests/panels_read_the_token_module_up1.rs:122`.
- Fix: DOC → TYPES. `Rgb` with a constructor visible only in `colors.rs`;
  derived colours as methods there.
- Verifier: both confirm; impact is cosmetic. Cost M (about 29 sites).

#### F176 A library Save overwrites a different tool: dedupe_key names 7 of 20 ToolConfig fields
A · S · CONFIRMED · new · not in flux · R1 · also F160
- Claim: `dedupe_key` omits `shaft_diameter` and `corner_radius_mm` (defining
  geometry) and `helix_deg`, `shank_diameter`, `stickout`, `tool_material`,
  `cut_direction`, `name`. The GUI Save button calls `add_or_replace_tool`,
  which overwrites the first equal-key entry. A second tapered ball with a
  different shaft diameter deletes the first one from the library. The status
  line names only the new tool.
- Sites: `io/tool_library.rs:145,156,298,316`, viz `ui/properties/tool.rs:104`,
  `controller/events/model.rs:170`, `compute/tool_config.rs:725`.
- Fix: NONE → TYPES. `let ToolConfig { id: _, name: _, tool_type, diameter,
  .., vendor: _, product_id: _, size_units: _, tool_number: _ } = t;` with no
  `..`. Decide `name` and `stickout` in the same commit (safe default: they
  make a tool distinct). Report the replaced name.
- Verifier: both confirm A (silent data loss). Cut the merged F160 sites
  (`session/mod.rs:911`, `diagnostics.rs:812`, `simulate.rs:987`,
  `wizard_overlay.rs:123`) from this finding: they belong to F065/F187/F101.
  Correct the stale doc at `tool_config.rs:435`.

#### F181 MCP replies and the CLI report copy core structs field by field in json!
B · L · CONFIRMED · new · in flux · R7 · also F119, F167
- Sites: viz `controller/events/compute.rs:2202,2345,2509`,
  `app/mcp/project.rs:205,480,482,866`, `app/mcp/commands.rs:2580`,
  `app/mcp/simulation.rs:726,734`, `session/mod.rs:1012`, cli `project.rs:512`,
  `rs_cam_mcp/src/server.rs:758`, `compute/stock_config.rs:77`.
- Evidence: `ProjectDiagnostics.verdicts` never reaches MCP. The top-level
  totals are re-derived from the trace. The G-RESULTS per-row fix landed
  (`compute.rs:2352-2362`). The parity test is at
  `crates/rs_cam_viz/src/controller/results_parity_tests.rs`, not `tests/`.
- Fix: SENTRY → TYPES. Base object = `serde_json::to_value(&core_diagnostics)`
  plus viz-only keys; projections as exhaustive destructures (no rest
  pattern); CLI `From<&ProjectDiagnostics>` with the same destructure.
- Verifier: both confirm. Check that the viz and core rapid counts agree
  before a merge hides a parity defect.

#### F182 Tool-shape facts are wildcard arms and a trait default (geometry_hint returns Flat)
B · M · CONFIRMED · new · in flux · R8 member
- Sites: `tool/mod.rs:330,396,585`, `compute/config.rs:726`,
  `compute/tool_config.rs:253,268,385`, `feeds/mod.rs:1936,2061`, viz
  `ui/properties/tool.rs:315`, `ui/tool_library_modal.rs:324,488`.
- Evidence: all six production impls override the default today; cold-start
  task 6 recorded 25 silent misses; G-BULLCUSP is a past break.
- Fix: NONE → TYPES. Remove the default body; methods on `ToolGeometryHint`
  with one exhaustive match each; exhaustive GUI `ToolType` matches.
- Verifier: both confirm. `feeds/mod.rs:1852` matches an `Option`: drop it.
  `valley_radius_mm` returns `radius()` for a bull nose on purpose.

#### F183 Per-op facts sit in _ => decision arms on OperationConfig
B (contested: E) · M · PLAUSIBLE · new · in flux · R8 member
- Sites: `feeds/suggest/apply.rs:1019`, `feeds/suggest/axial_envelope.rs:326,352`,
  viz `ui/properties/operations/shape_diagrams.rs:54`.
- Evidence: `pick_axial_envelope` re-matches the eight Finish3d ops by hand
  and ends `_ => {}` ("unreachable in practice" is false for a new row);
  `apply_stock_defaults` gives a new op no stock-aware depth.
- Fix: DOC → TYPES. Match on `capabilities().axial_envelope` and use
  `kind_str()`; a required `stock_default` field on the registry row.
- Verifier: `AxialEnvelope` already is a capability. Drop `apply.rs:966`,
  `adaptive_entry.rs:580`, `from_preconditions.rs:125` and
  `registry.rs:429` (filters or safe defaults). The diagram row choice is
  already forced.

#### F185 OpParams derive publishes a number with no domain: 107 of 269 numeric params
B · M · CONFIRMED · new (DR-LIVE residue: DR-LIVE fixed peck_depth only) · in flux · R6
- Claim: `check_param_range` returns `Ok` when the range is `None`, so MCP and
  the CLI accept `feed_rate = 0` or a negative feed on every op.
- Sites: `rs_cam_derive/src/lib.rs:106`, `compute/operation_configs.rs:499`,
  `session/compute/params.rs:64,185`, `compute/catalog/schema.rs:200`.
- Evidence: about 90 of the 107 are `feed_rate`, `plunge_rate`,
  `ramp_feed_rate`, `spindle_rpm`. No feed ≤ 0 refusal exists. Whether F0
  reaches G-code was not measured, so B, not A.
- Fix: NONE → TYPES. `compile_error!` for a numeric published field with no
  domain key unless `unbounded = "reason"`; shared `FEED_DOMAIN` consts.
  Keep "require help" as a separate, later rule.
- Verifier: both confirm (106-107 unbounded).
- Re-check after the tabs work (2026-10-19): `rs_cam_derive/src/lib.rs` and
  `session/compute/params.rs` have no change since the baseline. P5 kept the
  ranges on `tab_count`, `tab_width` and `tab_height` and added
  `tab_points: Vec<[f64; 2]>` with no domain (`compute/operation_configs.rs`,
  ProfileConfig). The scalar count holds; the W6 rule must say how a list
  param is treated (section 3.1, conflict 3).

#### F187 Cache and staleness keys list their inputs by hand
B · M-L · CONFIRMED · new · in flux · R7 member
- Sites: `session/compute/diagnostics.rs:193`, viz
  `state/simulation/issue_triage.rs:148`, `session/load_report.rs:95`,
  `compute/simulate.rs:987,1001`.
- Evidence: the only guard is a doc at `diagnostics.rs:812-814`;
  `evidence_fingerprint` records a past miss (late holder report).
  `tool_hashes`/`stock_hash` have no reader.
- Fix: DOC → TYPES. A `TriageView<'a>` of borrowed fields built in one place;
  `triage_from_parts` takes only that; the key derives from it.
- Verifier: both confirm. `StockConfig` has no `Hash`, so "derive Hash + Eq on
  an owned struct" does not compile as written.

#### F188 'None means not measured' is prose; a plain f64 figure compiles and reads 0 as clean
B · L · CONFIRMED · new · not in flux · R16 · also F175
- Sites: `stock/CLAUDE.md:18`, `diagnostics/CLAUDE.md:19,20`, viz
  `controller/events/compute.rs:2447`, `session/diagnostics_types.rs:365`,
  `tool_load/distribution.rs:310,339`, viz `ui/sim_diagnostics.rs:1209`.
- Evidence: U2 (2026-10-01) and X-VAC are past breaks.
  `DistributionOutcome::NotMeasured` is the typed precedent.
- Fix: DOC → TYPES. `Measured<T> { Value(T), NotMeasured(NotMeasuredReason) }`
  with no `unwrap_or` accessor; first replace the four Option figures plus the
  side-channel string in `compute.rs`.
- Verifier: both confirm. `SimulationCutSummary` plain `f64` fields need a
  per-field decision.

#### F189 Triage class is a choice of Vec; the Warnings tab reads 2 of 3 buckets
E · M · CONFIRMED · new · not in flux · R4 member
- Sites: `stock/sim_triage.rs:222`, viz `ui/sim_diagnostics.rs:647,870`,
  `controller/events/compute.rs:2493`.
- Fix: NONE → TYPES. `class: TriageClass` set by the rule constructor; one
  routing push; `SimulationIssueKind::cluster()` exhaustive (S).
- Verifier: confirm; advisories reach the verdict line, not the tab rows.

#### F191 GUI rows name a param by string; only a text-scan sentry checks the name
E · M · CONFIRMED · new · not in flux · R6 member · also F024
- Sites: viz `ui/properties/linking_dressup.rs:35,53`,
  `ui/properties/operations/boundary_2d.rs:69`,
  `tests/the_help_key_is_the_param_name_ui04.rs`, `session/compute/params.rs:166`.
- Fix: SENTRY → TYPES. The derive emits a `ParamKey<C, T>` per field with an
  accessor; `ParamType::KIND` replaces the type-string check.
- Verifier: confirm.

#### F192 Agent and CLI help text lists op, tool and post tokens by hand
E · S · CONFIRMED · new · in flux · R2 member · also F193
- Sites: `rs_cam_mcp/src/server.rs:1973,1989`, cli `main.rs:116`, `run.rs:79`,
  `session/compute/params.rs:487`, `export/gcode_validator.rs:154`,
  `gcode/post.rs:443`, `machine/kinematics.rs:541`, `machine/mod.rs:248`,
  `gcode/CLAUDE.md:23`.
- Fix: NONE → TYPES. Build lists from `ALL` (pattern at `params.rs:487`);
  clap possible values print the list.
- Verifier: confirm. `ControllerFirmware::Grbl` is never constructed.

#### F195 398 numeric as casts saturate in silence; no cast lint
E · L · CONFIRMED · Q12 (2026-09-16: six cast/float lints assessed, below the cut) · not in flux · R22 member
- Claim: `as usize` on a float saturates: `inf` becomes `usize::MAX` and
  `NaN` becomes 0. A step of 0 makes a count of `usize::MAX`.
- Sites: 398 `) as u32/usize/u8/u16/i32` in core and viz `src/`
  (`rg -c '\) as (u32|usize|u8|u16|i32)\b'`); 175 float → `usize` through
  `floor`/`ceil`/`round` in core. Example with a user step:
  `ops/zigzag.rs:167` `((perp_max - origin) / stepover).ceil().max(0.0) as usize + 1`.
  Examples with a fixed divisor: `dressup/entry_descent.rs:702`,
  `compute/execute/standing_material.rs:181`.
- Evidence: `Cargo.toml:35-63` (`[workspace.lints.clippy]`) has no cast
  lint. With stepover 0 the zigzag count is `usize::MAX` and `+ 1`
  overflows: the F030 mechanism at another site.
- Fix: NONE → LINT. `cast_possible_truncation` and `cast_sign_loss` per
  folder where a user value divides; a checked `steps(len, step) ->
  Option<usize>` helper. F030 closes the input side.

#### F205 ComputeBackend gives its test doubles silent no-op defaults
E · S · CONFIRMED · new · not in flux · below cut
- Claim: three trait methods have empty default bodies, so a new lane
  method that a backend forgets compiles and does nothing.
- Sites: viz `compute/mod.rs:492` (`submit_job`), `:500`
  (`submit_reach_map`), `:518` (`clear_sim_prefix_cache`); `:590`
  (`lane_snapshots -> [LaneSnapshot; 4]` names the lanes by hand). One
  production impl, `compute/worker.rs:1187`.
- Evidence: `rg 'ComputeBackend for' crates/rs_cam_viz` finds 31 impls (30
  test doubles). The critic counted 29 and the map 31. Each default has a
  doc reason; `generation_control` is non-default on purpose
  (`compute/mod.rs:505-509`), which shows the safer form.
- Fix: CONVENTION → TYPES. No default bodies; one shared scripted backend
  for the tests. Map: `viz-rest-and-small-crates.md` leads 6 and 7.

#### F209 project_entry_onto copies ToolpathEntry fields by hand
E · S · CONFIRMED · new · in flux · R7 member
- Claim: the panel write-back starts from the stored config and assigns 16
  entry fields one by one. A new `ToolpathEntry` field that the panel edits
  never reaches the session.
- Sites: viz `ui/properties/mod.rs:746-794` (`project_entry_onto`), called
  each frame from `write_entry_config_to_session` (`:823`).
- Evidence: the `stored.clone()` start keeps a new `ToolpathConfig` field,
  so the gap is on the entry side only (the critic corrected the map).
- Fix: CONVENTION → TYPES. Destructure `ToolpathEntry` with no `..` and
  name each field: written, or ignored with a reason.

#### F215 MCP refusal field: a wildcard over SessionError names "index"
E · S · CONFIRMED · new · in flux · R8 member
- Claim: two reply builders match a few `SessionError` variants and send
  every other variant with the refused field `"index"`.
- Sites: viz `app/mcp/commands.rs:1915` and `:1924` (`_ =>`; the critic
  cited `:1914,1923`).
- Evidence: a new `SessionError` variant (for example a selection refusal)
  reaches the agent as a bad `index`.
- Fix: NONE → TYPES. An exhaustive match, or a `SessionError::field()`
  method in core.

### 4.14 Outside the 13 categories (runtime cost)

The checklist has no runtime-cost category. These three findings come from
the critic fold. No agent measured a frame time, so the cost is read from
code.

#### F216 The toolpath panel runs a feeds calculation and an Apply dry run on every frame
E · S · PLAUSIBLE (the work per frame is certain; its cost is not measured) · new · in flux · below cut
- Sites: viz `ui/properties/toolpath_panel.rs:507-523`
  (`feeds_result_for_operation` on each draw; the comment says "Compute +
  cache" but no input check guards it), `:529-538` (`PillSuggestions::new`,
  the "one dry run of the apply funnel per frame").
- Evidence: `feeds_result_for_operation` runs the feeds calculator
  (`feeds::calculate` is 1321 lines, F131) and a vendor LUT lookup.
- Fix: NONE → CONVENTION. Cache the result on a key of the operation, tool,
  material and machine; recompute only when the key moves.

#### F217 The panel writes ReplaceToolpathConfig on every frame
E · M · PLAUSIBLE · new · in flux · below cut
- Sites: viz `ui/properties/mod.rs:797-838` (the doc at `:801` says it "runs
  every frame"), core `session/command.rs:2618-2640` (clone, two
  `generation_inputs_signature` calls, an unconditional write).
- Evidence: a frame where the operator only looks at the panel still
  clones and boxes one `ToolpathConfig` and runs `apply`. The empty stale
  set guard (`mod.rs:835-838`, G-HEIGHTSTAB) stops the dirty mark only.
- Fix: DOC → CONVENTION. Send the command only when the entry differs from
  the stored config, or give the panel a commit event.

#### F218 Blocking file dialogs run inside the frame and stop the MCP drain
E · M · CONFIRMED · new · not in flux · below cut
- Sites: 18 `rfd::FileDialog::new()` calls in viz: `ui/menu_bar.rs:40,50,60,70`,
  `ui/setup_panel.rs:185`, `ui/properties/machine_panel.rs:450`,
  `ui/properties/model_sim_panels.rs:72`, `app/input.rs:170,219,253,275,732`,
  `app/export.rs:46,67,106,339,372,452`.
- Evidence: `pick_file` blocks the calling thread. The MCP requests drain on
  the same thread once per frame (`app.rs:792`; the doc at `compute/mod.rs:122` says the drain runs on the
  egui main thread once per frame), so an open dialog stops every MCP call.
  `machine_panel.rs:454` also reads the file with `std::fs::read_to_string`
  inside the draw.
- Fix: NONE → CONVENTION. `rfd::AsyncFileDialog` (or a dialog thread) that
  posts an `AppEvent` with the path.

### 4.15 Alias index

The verifiers merged these ids into a primary finding. Search here first.

| Alias | Primary | Alias | Primary |
|---|---|---|---|
| F003 | F053 | F098 | F162 |
| F021 | F166 | F099 | F166 |
| F024 | F191 | F100 | F173 |
| F026 | F015 | F107 | F130 |
| F035 | F016 | F108 | F004 |
| F039 | F050 | F119 | F181 |
| F041 | F096 | F135 | F131 |
| F043 | F126 | F137 | F136 |
| F044 | F085 | F149 | F131 |
| F045 | F086 | F154 | F162 |
| F052 | F051 | F157 | F085 |
| F060 | F087 | F160 | F176 |
| F066 | F113 | F164 | F096 |
| F068 | F117 | F165 | F015 |
| F069 | F116 | F167 | F181 |
| F073 | F102 | F168 | F016 |
| F079 | F101 | F169 | F126 |
| F081 | F186 | F174 | F102 |
| F083 | F104 | F175 | F188 |
| F092 | F086 | F177 | F162 |
| F097 | F015 | F178 | F015 |
| | | F179 | F016 |
| | | F180 | F118 |
| | | F184 | F113 |
| | | F190 | F128 |
| | | F193 | F192 |

Cold-start ids map to primary findings as follows: CS-1 F162, CS-2 F015/F028,
CS-3 F016, CS-4 F118, CS-5 F181, CS-6 F176, CS-7 F182, CS-8 F183, CS-9
F113/F117/F116, CS-10 F185, CS-11 F186, CS-12 F187, CS-13 F189, CS-14 F128,
CS-15 F191, CS-16 F192, CS-17 F214, CS-18 F194, CS-19 F188. CS-17 mapped to
F192 until the critic fold; it is a different root cause (post dialect facts
live twice), so it has its own finding.

## 5. Notes for the geometry-selection programme

These four findings are properties of the unfinished B-rep selection feature.
They are notes for that programme (rs-cam-f6), not debt. All four are in flux.

#### F013 One ResolvedSelection type for the model frame and the setup frame
E · S · CONFIRMED
- Claim: `SelectionFrames { model, setup }` holds two `ResolvedSelection`
  values. `face_top_z` is correct only on the setup one; a doc line says so.
  `to_setup_frame` returns `Self`, so a second call or a missing call
  compiles.
- Sites: `session/selection_status.rs:154,192`, `selection/resolve.rs:180,205`,
  `selection/item.rs:104`, `selection/rule.rs:76`,
  `session/compute/generation.rs:576`.
- Note: `ResolvedSelection<F>` with sealed frame markers;
  `to_setup_frame(Option<&SetupTransformInfo>)` so the identity case is an
  explicit conversion; `face_top_z` only on `<SetupFrame>`. The two
  resolve-then-transform sites take different inputs (op selection and
  boundary selection): share one helper, do not merge the doors. Fits
  TH-FRAME (R18).

#### F050 ZRange and DiameterRange publish raw fields; MCP builds them unchecked
E · S · CONFIRMED · also F039
- Claim: an inverted range, a negative tolerance or NaN is representable and
  matches nothing with no reason.
- Sites: `selection/rule.rs:43,44,78,79`, `rs_cam_mcp/src/server.rs:986,1017`,
  `selection/item.rs:245`.
- Note: private fields; `ZRange::new` / `DiameterRange::new -> Result`;
  `serde(try_from)` on the core types so project files go through the check;
  `TryFrom` on the wire params; a `SelectionRefusal` cause. About six test
  literals change. `Item::new` in the same module already shows the pattern.

#### F094 outlines_pre_selection outlived its planned P2.3 deletion
C · S · CONFIRMED
- Claim: the doc says "P2.3 deletes this", but the method stays.
  `ItemSet::scaled` and `with_circles` compute and carry the memo in
  production on each scaled import. The pub `LoadedModel` door is read only
  by tests.
- Sites: `selection/item.rs:564,603,642`, `session/mod.rs:557`.
- Note: P2.5 kept it on purpose as a reference. If the oracle stays, move it
  into `tests/common` (nest before the scale, then scale). Then delete the
  `OnceLock` field, the two production carries and the pub door.

#### F127 selection imports session and compute, and both import it back
E · S · CONFIRMED
- Claim: `selection/` imports `session::LoadedModel`,
  `compute::transform::SetupTransformInfo` and `compute::catalog::ItemUse`;
  session and compute import `selection` (63 lines in 19 files).
- Sites: `selection/resolve.rs:14,17`, `selection/refusal.rs:11`,
  `compute/config.rs:449`, `compute/catalog.rs:987`.
- Note: give `resolve` a narrow input (mesh, items, transform); move `ItemUse`
  into `selection/` with a re-export. `SetupTransformInfo` stays a compute
  edge unless it moves to a lower layer. A small `rg` sentry enforces the
  direction.

## 6. Below the cut: recorded, not scheduled

These findings are real but not scheduled. A theme wave may fix one as a side
effect; the wave then states it. Grouped by reason.

E, weak pattern with no break yet:
F008 angles, F009 air-cut tuple, F010 grid slices, F011 DiagnosticId,
F012 units, F018 arc bool, F019 capability bools, F022 field-name dial,
F037 adapter re-checks, F046 pub invariant fields, F057 smoke resolution,
F058 inverse deflection sentinel, F062 MCP reply shapes, F089 theme alias
layer, F106 cancel vocabularies, F109 severity vocabularies,
F122-F125 layering (42 cycles, no facade, IR file, viz ui imports; F121
rides with W5), F129 LUT plus radius (D), F130 entry-point ladders,
F133 anonymous tuples, F134 debt instrument, F139 Kc text, F141 CLAUDE.md
size, F143 stale docs, F146 research file name. From the critic fold: F203
auto_regen fallback, F205 backend no-op defaults, F208 overlay string ids,
F212 global library paths, F216 per-frame feeds work, F217 per-frame config
write, F218 blocking file dialogs.

D, duplication with no divergence yet or larger than its value now:
F007 bbox arrays, F077 third band dispatcher (D14), F105 mark lifts,
F158 fixture builders. From the critic fold: F198 G-code IR modal text,
F211 lock-poison policies, F214 post dialect facts (CS-17).

F, size, style, navigation:
F049 unsealed traits, F064 `Result<_, String>`, F084 cache scaffolding (D8),
F112 module layout, F131 `feeds::calculate` (T-5), F136 `commands.rs` size,
F145 module docs, F147 line citations, F148 purged planning paths,
F150 two names for one place, F159 instrument binaries. From the critic
fold: F196 `impl ProjectSession` in 25 files, F213 `capitalised` copies.

Below-cut findings that grew since their register row: F131 (T-5: 1302 →
1321 lines, 16 → 23 mutable locals), F130 (Q10: 110 → 147 allows), F134 (Q11
list inherited the scanner errors), F077 (D14: two of three copies merged).

## 7. Refuted claims (do not re-file)

The verifiers refuted no whole finding. They refuted these sub-claims, and
the finders dropped these candidates after a check.

Refuted sub-claims:
- F057: "the run measures at the stored cell". The measurement uses the raw
  argument (`smoke.rs:760-761`).
- F072: "a refused command leaves the mirror and the session different".
  `SetPostConfig` cannot be refused (`session/command.rs:2504`).
- F116: "the export datum shift is not applied". The job `SetupDef` has no
  datum.
- F096 and F102: "CLI `project.rs:309` and `smoke.rs:751` bypass the CLI
  door". `apply_command` is a pass-through; the CLI has no Effects consumer.
- F042: "none has a production caller". `apply_feeds_result_to_op` is called
  at `feeds/suggest.rs:915`.
- F019: the Inlay gouge example. `TRANSFORM_V_WIDTH` is all false; a swap
  changes nothing.
- F022: the `relink_dial` write risk. It is a text label only.
- F138/F140: `dressup/CLAUDE.md:23` (relink_fragments), `pins/CLAUDE.md:9`
  (PinDrillState) and `session/CLAUDE.md:32` (RETIRED_KEYS) are correct.
- F142: `Self::rename_setup`, `add_obstacle`, `edit_toolpath_selection` links
  resolve (pub(crate) methods).
- F171: the `#![cfg(feature = "mcp")]` gate gap. `mcp` is a default feature.
- F082: the proposed home in `feeds/force.rs` breaks layering; use
  `tool_load::deflection`.
- F076: `conformal_spiral` takes the upper median. It averages the middle pair.
- F161: `registry.rs:421`, `tool_config.rs:40`, `gcode/mod.rs:1488`, the
  Pencil RestDepth filters and the two `== GrblHal` UI gates are not
  fail-open dispatch.
- F183: `apply.rs:966`, `adaptive_entry.rs:580`, `from_preconditions.rs:125`
  are filters or safe defaults; the diagram row choice is already forced.
- F182: `feeds/mod.rs:1852` matches an `Option`, not a tool enum.
- F085: `reach_map.rs:1854` is already gated.
- F086: `HELIX_ENTRY`, `with_flag`, `EMBEDDED_LUT` are referenced.
- F103: the four EntryStyle enums are the normal config/engine split.
- F095: the DropCutter, Pencil and Trace variant matches are field access,
  not op-type policy.
- F152: `adaptive3d/tests.rs:2229` is a valid instrument ignore.

Critic sub-claims that the fold refuted or corrected (`CRITIC.md`):
- CRITIC section 1: "viz `controller/events/preferences.rs:117` is a
  production `#![allow(unwrap_used, expect_used)]` with no `SAFETY:` line".
  The allow is in a `#[cfg(test)] mod tests` (`:114`), and `:116` carries a
  `// SAFETY:` line. F170 does not change.
- CRITIC 2.11: "REVIEW drops the non-JSON fallback; a merge loss". F062
  already cites `mcp_bridge.rs:1124` and names the fix. F207 lifts the
  mechanism to its own row; nothing was lost.
- CRITIC 2.9: "28 test doubles". `rg 'ComputeBackend for'` finds 31 impls,
  30 of them test doubles (F205).
- CRITIC 2.2: "20 `Statement::Raw` pushes". 14 are production; 6 are in the
  test module from `program_builder.rs:850` (F198).
- CRITIC 2.5: `compute/tool_config.rs:928` `NAME_MATCH_REL_TOL` is a design
  tolerance with its reason in the doc, not an unsourced constant (F201).

Leads the fold checked and found no debt:
- `dressup/arcfit.rs:38-41` `RUN_SCAN_READS` is a `#[cfg(test)]` test seam
  (`core-strategies.md` lead 27).
- `ui/components/mod.rs:23` cites `planning/ui_audit/ARCHITECTURE.md`. The
  purge tag holds it (`git show planning-pre-purge-2026-09-17:...`), so it
  is an F148-class citation, not a new finding.
- Checklist section 7 commented-out code: the checklist regex gives 102
  hits, all prose. A scan for such lines that end in `;` or `{` gives 1
  hit, also prose (`tool_load/mod.rs:346`). Checklist section 5
  `Box<dyn Error>` in core, viz and mcp `src/`: 0 hits. Checklist section
  12 exact-float asserts: 3 hits, all on printed constants (critic count;
  the fold did not repeat it).

Candidates the finders checked and dropped:
- dead-code: `CleanupStrategy::Legacy` (live GUI mode),
  `ClassificationSampler` arms (schema escape hatch), `ReferenceEngagement`,
  `enforce_unmodeled_policy`, `MoveKinematics` (instrument false positives),
  `viz_project_evidence` (5 callers), `scallop/research.rs` (production), W4
  residue (already deleted), the `MoveIntent::Unknown` doc claim,
  `pre_gcode`/`post_gcode` (wired), dexel `simulate_*` variants (all called),
  the stale-defaults validator (CMP-25 closed). `session/compute/params.rs:122`
  keeps a fn alive for a text sentry: noted, not filed.
- duplication: cycle_time/readiness wrapper, generation_plan Step→PlanStep
  adapter, `build_sim_request`, `XyTerritory`/`TerritoryXy`, CLI
  `ToolpathDiagnostic` (exhaustive destructure), MCP `From<XParam>` wire
  mirrors, `memory_budget`, territory/capabilities.
- inconsistency: the ui components Widget-vs-show split.
- cold-start: the transit-filter copy in `stock/simulation_cut/accumulate.rs:44,118`
  (feeds reported peaks, not gates).
- navigability: the map claim that `unified_finish.rs:371` has no
  `tracing::warn` (`:1543` has it).
- illegal-states: the `feeds::calculate` preview path, and `vendor_lut`
  `validate_observation` (known L16).
- encapsulation: the three session `*_mut` hatches (closed by WP7/S30), the
  other COMPUTE_DOORS (ruling §25.4), `from_key` as its own defect (L12).

## 8. Method

Instruments:
- `CHECKLIST.md` (14 categories, signals, detection commands).
- Ten area maps in `maps/` with recurring-pattern tables and enforcement
  grades. The grade counts in 1.3 come from the first grade cell of each
  section 5 row; a row with no single grade is excluded (153 graded rows).
  The earlier figure (166 rows, 61 TYPES) had no stated rule and no
  instrument reproduced it.
- `instruments/`: `dead_pub_surface.md`, `test_only_pub_api.md`,
  `allows_without_safety.md`, `legacy_residue.md`, `inventory.md`,
  `cargo_machete.txt`, `churn_since_2026-09-18.txt`, two duplication sweeps
  (`dup_sweep_0.88_src.md`, `dup_sweep_0.93_all.md`), `in_flux_paths.txt`
  (313 paths).
- Eight cold-start tasks, two traces each (with and without CLAUDE.md):
  `coldstart/task1.json` to `task8.json`, summarised in
  `findings/cold-start.md`.
- Known debt: `planning/TECH_DEBT_REGISTER.md` and
  `planning/tech_debt_2026-09-16/TECH_DEBT_PLAN.md`.

Agents: 13 category finders plus one cold-start reviewer filed 194 ids. Two
independent verifiers checked most A and B findings; one verifier checked the
rest. 47 ids merged into 147 primary findings. A completeness critic
(`CRITIC.md`) then proposed candidates; the fold added 24 of them as F195 to
F218 (171 primary findings). The advisor was rate-limited
for three finders (bool-string-params, architecture-layering, and one
second review).

Rules applied in this file:
- In flux: true when any cited site is in `in_flux_paths.txt`. F150 is set to
  true on the verifier's correction. F129 and F130 are partly in flux
  (`compute/simulate.rs`).
- Contested tier: the finder tier stays in the ranked table and the row says
  "contested"; the "as verified" count uses the lower tier.
- Selection programme: only F013, F050, F094, F127.
- Line numbers: every `path:line` is at the baseline 99b6fb41 unless the
  entry says otherwise. The critic-fold findings (F195 to F218) and the
  "Re-check" and "Changed by" lines cite the working tree of 2026-10-19
  (HEAD a682e333 plus the selection P4.1 edits).

Known gaps:
- No agent ran cargo, clippy, rustdoc or cargo-udeps. No proposed fix is
  compile-checked. Consequences in F067, F113, F115 are read from code, not
  observed on screen or in G-code.
- Counts come from `rg` and Python scans and are approximate. Verifiers often
  got a different number (noted per finding).
- `rs_cam_mcp_proxy` was skimmed (signatures, manifest) only.
  `rs_cam_derive` was read only for the domain rules.
- Not swept: test-only duplication (1 449 pairs), benches, compile and link
  times, the `FEATURE_CATALOG.md` claims against code (checklist section 11),
  and the candidate lints `float_cmp`, `match_same_arms`,
  `missing_errors_doc`, `must_use_candidate`, `return_self_not_must_use` and
  `unreachable_pub` (checklist section 13; `Cargo.toml:35-63` has none of
  them, and no agent ran clippy to count their hits). The GUI literal
  ranges are swept now (F210).
- Not applied: the churn raise of the severity rubric ("raise one level" on
  a hot change path). No tier in this file uses
  `instruments/churn_since_2026-09-18.txt`. Hot files that carry findings:
  `session/compute.rs` 46 commits, `session/mod.rs` 41, viz `mcp_server.rs`
  33, `compute/catalog/registry.rs` 33, viz `ui/properties/mod.rs` 31,
  `diagnostics/ids.rs` 30, viz `app/mcp/commands.rs` 27 (critic counts).
  `ids.rs` broke F162 again during the review, which supports the raise.
- Not checked one by one: 101 own-file-only and 38 test-only pub rows, the 381
  bool-literal call sites, the 84 `.max(1e-N)` floors, every must/never line
  in the 44 CLAUDE.md files, the 26 viz `_ =>` arms in `ui/overlays/live.rs`
  and `present_mode.rs`.
- Not measured: whether a feed of 0 reaches emitted G-code (F185); whether the
  advisor travel cap changes a ranking (F002); the pencil vacuous skip (F153)
  was not run.
- Live edits during the review: `compute/catalog/schema.rs`,
  `compute/execute.rs`, `compute/operation_configs.rs`, `diagnostics/ids.rs`,
  `dressup/mod.rs` and `dressup/tabs.rs`. They landed in e76f82c8 (P5) and
  16c1b78b (P5b). The fold re-checked F016, F128, F162 and F185 (each entry
  has a "Re-check" line) and found that e76f82c8 changed F023 (its
  dead-branch claim no longer holds).
- Critic leads not checked by anyone (do not file without a check):
  `OperationFamily::Face` dead, `simplify_path` against `rdp`, the three.js
  CDN URL, the `FaceGroupId` cap, the `ComposedRest` `Send` bound,
  `MetricEvaluator` partial use, `vendor_lut::embedded()` dropping a bad
  file.
- The critic fold (2026-10-19): one reader checked each `CRITIC.md`
  candidate with `rg` and file reads, with no cargo run. Its findings carry
  one verdict, not two.

## 9. Operator rulings (2026-10-19)

1. F015: an export with no valid Z datum is refused. An unknown `z_datum`
   token never falls back to the stock top.
2. F033: an inverted height stack is refused at export, and the UI prevents
   an operator from setting it. One core height-order function decides; the
   badge, the diagnostics and the export door read it. The fix waits for the
   geometry-selection height work (section 3.1, conflict 1).
3. F151: delete `ci.yml`. Docs say "no CI".
4. F176: geometry and `tool_material` make a library tool distinct. `name`,
   `stickout`, `tool_number`, `vendor` and `product_id` do not. The key is an
   exhaustive destructure of `ToolConfig` with no `..`, so a new field forces
   a decision.
5. F034: the project and config loaders refuse every unknown key
   (`deny_unknown_fields`), under the no-legacy ruling.

## 10. Progress

| Wave | Commit | Findings closed | Notes |
|---|---|---|---|
| W1 Tool model | e67652e6 | F176, F029 step 1, F075, F210 (tool part) | Lead ruling: the tool domain holds only the bounds that the geometry gives; the old GUI literals are the editor drag range (`ui_range`) and refuse nothing. Left: ProjectSessionBuilder does not validate; `BullNoseEndmill` has pub fields (ToolShape end state, F182); `corner_radius_mm` and `corner_radius` are one concept stored twice. |
| W3 Verdict surfaces | 5561dc37 | F113, F189, F117, F181 (reply part), F048 | F117 rule is `stepover > diameter`. Left: CLI `summary.json` collision_count is a holder-only sweep, the verdict count includes obstacles; F117 has no CLI generation-refusal door. Not seen on screen. |
| W6a Param domains (derive) | this commit | F206, F185, F023, F163 (four role clauses), F030 (HorizontalFinish refuse) | Every published number states `range`/`min`/`gt`/`max`, `frame_dependent` or `unbounded = "reason"`, or the derive refuses it (const-eval error E0080, decided by `ParamType::KIND`, so a type alias is caught). Feeds and spindle `gt = 0`; lengths `min = 0`; slope angles `0..=90`. Lead ruling: `bisector_strength min = 0` from its definition (0 = off). Left for W6b: the validate doors (AddToolpath, ReplaceToolpathConfig, load), F036 slope-band order, F210 stock/head/shoe/machine; F163 scallop and `extra` clauses still optional. The GUI feed rows now clamp just above 0 (not seen on screen). |
| W2 CLI boundary | 9fd4f3cb | F014, F028 (CLI part), F057, F116 step 1, F151 | Job tools with unset stickout and shank take the ToolConfig defaults (45 mm, 6.35 mm). Left: README "Verification gate" lists a workspace-wide `cargo test`; `sweep::write_gcode` calls raw `emit_gcode` (W5). |

Pre-existing reds found during W1 (red at 82d8e926, not caused by a wave),
for W14: viz `component_contracts_up2::the_raw_drag_value_allowance_is_not_vacuous_ui10`
(`ui/properties/setup.rs` has 1 raw `DragValue`, the test expects 2) and viz
`wizard_e2e::default_wizard_state_does_not_mutate_export` (the hand-built
baseline has no feed-stage line since 66a81d77).

Next, by REVIEW section 3: W4 (project file and tokens: the Z-datum refusal
of ruling 1, `deny_unknown_fields` after selection P4 and P6), W5 (export
door), W6 (param domains, after F206), then the waves that wait for the
selection programme. F033 (ruling 2) waits for selection P3.
