# Completeness critic for REVIEW.md (2026-10-19)

Scope: `REVIEW.md`, `CHECKLIST.md`, the ten `maps/*.md`, the 14
`findings/*.md`. The critic read code at HEAD `2d8f5dbf` (six commits after
the baseline `99b6fb41`) with `rg` and file reads. No cargo ran. No file
under `crates/` or `scripts/` changed.

Each new candidate has this tag: tier · cost · grade now → grade after ·
in_flux · known_id. "Checked" means the critic opened the cited lines.

## 0. The most important result

F162 broke again during the review. At HEAD, `diagnostics/ids.rs` holds 101
id constants and `ALL` lists 93. The baseline missed 8. Commit `e76f82c8`
(selection P5, tabs) added `GEOM_TAB_PLACEMENT` at `diagnostics/ids.rs:279`
and did not add it to `ALL`. The finding proves its own claim. Update the
F162 count to 9 and cite `e76f82c8` as a present break. `ids.rs` is also a
top-churn file (30 commits since 2026-09-18).

## 1. Checklist signals that no finder swept

| Signal (CHECKLIST section) | Count at HEAD | Status |
|---|---|---|
| Numeric casts; `cast_possible_truncation`, `as_conversions` (13) | 398 `) as u32/usize/u8/u16/i32` in core and viz production; 175 float→`usize` through `floor`/`ceil`/`round` in core | Not swept. No cast lint in any `Cargo.toml`. Example: `dressup/entry_descent.rs:702` `(len / ENTRY_CLIP_SPACING_MM).ceil().max(1.0) as usize`; `compute/execute/standing_material.rs:181` `(dilate_mm / cell).ceil() as usize`. A step of 0 gives `inf`, and `as usize` saturates to `usize::MAX`. This is the mechanism under F030 (stepover 0). E · L · NONE → LINT. Add to TH-LINT. |
| `#![allow` inner attributes (13) | 703 in all crates. Most are CLI print allows with a reason comment or test modules | Partly in F170. One production non-CLI site: viz `controller/events/preferences.rs:117` `#![allow(clippy::unwrap_used, clippy::expect_used)]` with no `SAFETY:` line. Add to F170. |
| `float_cmp`, `match_same_arms`, `missing_errors_doc`, `must_use_candidate` (5, 13) | 0 mentions in REVIEW or findings | Not swept. The checklist names each one as a candidate lint. |
| `impl` blocks of one type in many files (10) | `impl ProjectSession` in 25 files under `crates/rs_cam_core/src` | Not filed. F · M · CONVENTION. |
| Commented-out code (7) | 1 hit, a false positive (`tool_load/mod.rs:346`) | Swept now: no debt. |
| Exact-float asserts (12) | 3 hits, all on printed constants | Swept now: no debt. |
| `Box<dyn Error>` in library code (5) | 0 | Swept now: no debt. |
| `FEATURE_CATALOG.md` claims against code (11) | 1 mention in REVIEW | Not swept. |
| Churn raise (severity rubric) | — | Not applied. The rubric says "raise one level" on a hot change path. REVIEW uses `churn_since_2026-09-18.txt` nowhere in a tier. Hot files that carry findings: `session/compute.rs` 46, `session/mod.rs` 41, viz `mcp_server.rs` 33, `compute/catalog/registry.rs` 33, viz `ui/properties/mod.rs` 31, `diagnostics/ids.rs` 30, viz `app/mcp/commands.rs` 27. |

Outside the 13 categories (no category owns them; say so, do not file):
the toolpath panel runs `feeds_result_for_operation` and a `PillSuggestions`
dry run on every frame (`ui/properties/toolpath_panel.rs:514-538`); the panel
sends `ReplaceToolpathConfig` on every frame (`ui/properties/mod.rs:807-829`);
blocking `rfd::FileDialog` calls run inside draw passes
(`ui/menu_bar.rs:40,50,60,70`, `ui/setup_panel.rs:185`,
`ui/properties/machine_panel.rs:450-454`). A blocked frame also blocks the
MCP pump. The checklist has no runtime-cost category.

## 2. Map leads with no finding (checked in code)

Each item below is a map lead that REVIEW and `findings/` do not carry. The
critic checked each one.

### 2.1 Four hand copies of the stale-trace rewrite; one copy was missed once
B · S · CONVENTION → TYPES · in_flux yes (`gcode/mod.rs`) · known_id T-21
(closed 2026-09-18, `TECH_DEBT_REGISTER.md:37`; the instance is fixed, the
pattern stays)
- Sites: `gcode/mod.rs:641,652,663,680` (one fn per verdict type), the call
  block `gcode/mod.rs:813-822`, the struct `tool_load/verdict.rs:237-250`.
- Evidence: the doc at `gcode/mod.rs:672-679` records that S3 added the depth
  gate with no rewrite arm. A sixth gate needs a fifth fn and a fifth call.
- Fix: one `ToolpathLoadVerdict` method that destructures the struct with no
  `..` and rewrites each `Unmodeled` arm. Map: `core-io-output.md` lead 11.

### 2.2 The G-code IR carries modal state as text
D · M · NONE → TYPES · in_flux no (`gcode/ir.rs`, `program_builder.rs`,
`emitter.rs`) · new
- Sites: `gcode/ir.rs:44-47` (`Raw(String)` holds M3/M5/M6/M7/M8/M9, G40,
  G41/G42 and G4); 20 `Statement::Raw` pushes in `gcode/program_builder.rs`
  (for example `:178,222,655,845`); three text scanners:
  `gcode/emitter.rs:125` `unsupported_mcode_in_line`,
  `gcode/program_builder.rs:481` `dangerous_modal_words`, and the validator
  word scan in `export/gcode_validator.rs`.
- Evidence: tests find coolant stops by `Statement::Raw(t) if t == "M9\n"`
  (`program_builder.rs:1346,1546,1565`). The IR cannot say "coolant off".
- Fix: typed IR variants (`Coolant(Coolant)`, `CutterComp(..)`, `Dwell(s)`);
  `Raw` only for operator snippets. Map: `core-io-output.md` lead 7. Fits
  F005 (bare IR numbers) as a sibling.

### 2.3 The default region-extraction names hide the 64-region cap
B · S · DOC → TYPES · in_flux partly (`session/multitool.rs`) · new
- Sites: `geometry/region_mask.rs:111-119` (`region_polygons_from_mask`),
  `:149-159` (`_clamped`); production callers of the hiding forms:
  `finish/finish_planner.rs:537,957`, `session/multitool.rs:1291`.
- Evidence: `geometry/region_mask.rs:107-110` says the return value "cannot
  tell you whether the cap fired". `maps/tier_islands.rs:16-18` measured 502
  of 566 islands dropped on wanaka by the plain form. Only `tracing::warn!`
  reports the drop.
- Fix: keep only `_reported`; `#[must_use]` on the report type. Map:
  `core-io-output.md` lead 16.

### 2.4 The air / low-engagement partition is typed at six sites
D · S · CONVENTION → TYPES · in_flux no · new
- Sites: `stock/sim_triage.rs:199-201`, `stock/simulation_cut/analysis.rs:161-163`,
  `stock/simulation_cut/reporting.rs:110-112`,
  `stock/simulation_cut/accumulate.rs:135-138`, `tool_load/power.rs:425`,
  `trace/narrate.rs:730`.
- Evidence: `AIR_CUT_RADIAL_WOC_FRACTION` exists (`stock/simulation_cut.rs:253`)
  and none of these sites reads it. The 0.10 bound has no constant. The comment
  above `reporting.rs:110` says the copies "must stay in step".
- Fix: one `SimulationCutSample` method that returns an engagement class
  enum; every site matches it. Add to TH-DUPSMALL (R24). Map:
  `core-stock.md` lead 1.

### 2.5 Unsourced numbers drive production feeds
B · M · DOC → SENTRY · in_flux no · new (T-16 is a sibling row, not this one)
- Sites: `material/mod.rs:962-981` (`base_cutting_speed_m_min`, "conservative
  SFM placeholder"), `material/mod.rs:995-1010` (`plunge_rate_base`
  baselines), read in production at `feeds/mod.rs:1547,2545`;
  `feeds/mod.rs:2822-2929` (`operation_default_profile` factor table,
  `default_engagement` "From reference calcs.rs" at `:2939`);
  `compute/tool_config.rs:928` `NAME_MATCH_REL_TOL = 0.10`.
- Rule broken: root `CLAUDE.md` ("never type a number from memory") and
  `material/CLAUDE.md` ("Never type an envelope value ... as a literal").
- Fix: each constant names its printed source in the repo, or the formula
  path refuses; a sentry like `wood_species_library_provenance` checks the
  table. Map: `core-feeds.md` leads at `material/mod.rs:962-1010`,
  `feeds/mod.rs:2822-2929`, `feeds/suggest/apply.rs:1000-1030`.

### 2.6 `$341` tool-change mode is a raw `u32` in core and an enum on the wire
E · S · CONVENTION → TYPES · in_flux no · new
- Sites: `machine/mod.rs:292-296` (`tool_change_mode: Option<u32>`, five
  meanings in the doc), `machine/mod.rs:333` (`TOOL_CHANGE_MODE_IGNORE`),
  `machine/mod.rs:411`; the wire already has `ToolChangeModeParam`
  (`rs_cam_mcp/src/server.rs:245`). `$32` `mode: Option<u32>` at
  `machine/mod.rs:286` has the same shape.
- Why it matters: the 2026-10-08 M6 crash came from tool-change settings.
  Map: `core-io-output.md` lead 22.

### 2.7 Two runtime-row creators hard-code `auto_regen = true`
E (rule already broken at two sites; no live defect found) · S · DOC → TYPES ·
in_flux no · new
- Sites: viz `state/runtime.rs:353-356` (`toolpath_rt_or_default` →
  `ToolpathRuntime::new(true)`), viz `ui/properties/mod.rs:275`. Called from
  `controller/events/compute.rs:36,56,295,410,497`.
- Rule: `state/stale.rs:35-41` says every row creator reads the catalog, and
  "a row hardcoded to `true` would queue a 3D finishing pass nobody asked
  for". Five creators obey it: `controller/io.rs:470`,
  `controller/events/toolpath.rs:219,279`, `controller/events/planner.rs:91`,
  `app/mcp/commands.rs:2414`.
- Check: load and add create a row for every toolpath, so the fallback did
  not fire in the paths the critic read. A new door that skips row creation
  makes `compute.rs:410` queue an unrequested 3D finish.
- Fix: `ToolpathRuntime::new` takes the `OperationConfig` (private field
  plus constructor); delete the `true` fallback. Map:
  `viz-rest-and-small-crates.md` lead 2.

### 2.8 A test that does not compile, in a configuration nobody builds
C · S · NONE → TYPES · in_flux no · new
- Site: viz `lib.rs:219-228` calls `super::run(true)`; `run` takes two
  arguments (`lib.rs:48`). The module is `cfg(all(test, not(feature =
  "mcp")))` and `mcp` is a default feature (`rs_cam_viz/Cargo.toml:40`).
  `cargo test --no-default-features` fails to build. Map:
  `viz-rest-and-small-crates.md` lead 4.

### 2.9 `ComputeBackend` has silent no-op defaults for its test doubles
E · S · NONE → TYPES · in_flux no · new
- Sites: viz `compute/mod.rs:492,500,518` (`submit_job`, `submit_reach_map`,
  `clear_sim_prefix_cache` default to `{}`), `compute/mod.rs:590`
  (`lane_snapshots -> [LaneSnapshot; 4]` hard-codes the lane count).
  29 `impl ComputeBackend for` in viz; one is production
  (`compute/worker.rs:1187`).
- Fix: no default bodies; one shared scripted backend for the tests.
  Map: `viz-rest-and-small-crates.md` leads 6 and 7 (the map says 31
  doubles; the count is 28).

### 2.10 The derive crate that TH-DOMAIN extends has no tests
E · S · NONE → SENTRY · in_flux no · new
- Site: `rs_cam_derive/src/lib.rs` (433 lines, 0 `#[test]`, no `trybuild`,
  no `compile_fail` doctest). The workspace has the `compile_fail` pattern
  (`measurement.rs:433`, `trace/transform_provenance.rs:309`).
- Why it matters: F185 and F163 put a `compile_error!` into this crate. The
  proof that wrong code fails to compile must exist before W6. Map:
  `viz-rest-and-small-crates.md` lead "rs_cam_derive has no tests".

### 2.11 MCP outcome: a non-JSON reply reads as success
B · M · DOC → TYPES · in_flux yes (`mcp_bridge.rs`, `app/mcp.rs`) · new
- Sites: viz `mcp_bridge.rs:1124-1127` (`from_json_response` returns
  `Succeeded` when the text is not JSON), `app/mcp.rs:379` (the fallback when
  a handler states no outcome).
- Evidence: the doc at `mcp_bridge.rs:1100-1115` names G-MCPTOAST, a past
  break of the same toast class. `findings/error-handling.md:240` records the
  site. REVIEW drops it: F062 cites `mcp_bridge.rs` for the reply shapes but
  not for this fallback. This is a merge loss.

### 2.12 Smaller sites that belong to an existing theme

| Site | Theme | Note |
|---|---|---|
| viz `app/mcp/commands.rs:1914,1923` `_ =>` over `SessionError` picks the refused field `"index"` | TH-WILDCARD (R8) | in flux |
| viz `ui/overlays/registry.rs:204-209` `flag: Option<&'static str>` is a field path; 16 `registry::row("...")` lookups behind `if let Some` (`ui/viewport_overlay.rs:707,763`, `ui/overlays/panel.rs:553`, `controller/events/mod.rs:487`) | F022 class; an `OverlayId` enum makes `row` total | A renamed id hides the control with no message. Map `viz-ui.md` lead 8 |
| viz `ui/properties/mod.rs:746-800` `project_entry_onto` copies 16 `ToolpathEntry` fields by hand | TH-FIELDLIST (R7) | Map correction: the clone keeps a new `ToolpathConfig` field. A new `ToolpathEntry` field is the one that never writes back. in flux |
| viz `ui/properties/{tool,stock,head,shoe_row,machine_panel}.rs`: 31 literal `..=` ranges, for example `tool.rs:297` included angle `1.0..=179.0` | TH-DOMAIN (R6), F029 | REVIEW section 8 says "not swept". One `rg` sweeps it. The GUI range is today the only domain check for the V-bit angle that panics through MCP (F029) |
| `trace/debug_trace.rs:221,286,320,354,448,460` and `trace/semantic_trace.rs:668,695,728,748,1000` `lock().expect("poisoned")`; viz `compute/worker.rs:472,485,637` `unwrap_or_else(into_inner)` | inconsistency (two poison policies) | D · S. The critic did not prove a GUI panic path; file as inconsistency only |
| `settings/paths.rs:174-200` process-global `PATHS`, read lazily by every library resolver | encapsulation (hidden input) | E · M. Map `core-session.md` lead 27 |

Leads the critic did not check (label as unchecked, do not file):
`OperationFamily::Face` dead, `simplify_path` against `rdp`, the three.js CDN
URL, the `FaceGroupId` cap, `ComposedRest` `Send` bound, `MetricEvaluator`
partial use, `vendor_lut::embedded()` dropping a bad file.

## 3. REVIEW claims that a finding does not back

1. Section 1.3 "166 rows, 61 TYPES (37 %)" and "core-compute 16 of 26
   rows". The critic counted the section 5 tables: 153 rows, 50 TYPES (33 %),
   and core-compute 14 rows with 5 TYPES. All map tables together give 192
   rows and 66 TYPES (34 %). Neither count gives 166/61. The "about one in
   three" result holds. State the counting rule, or use the section 5
   figures.
2. Section 1.2 and F162 "misses 8". At HEAD it misses 9 (section 0).
3. Alias index: CS-17 maps to F192 (help text lists tokens). CS-17 is a
   different root cause: the post dialect facts live twice. The code table
   `export/gcode_validator.rs:153-203` (`supports_m6`, `no_canned_cycles`)
   restates facts that the post data holds (`gcode/post.rs:299`
   `canned_drill_cycles`, `gcode/post.rs:432` `tool_change_commands_m6`). An
   edited post TOML can disagree with the validator. D · S · CONVENTION →
   TYPES (derive the invariants from the `Post`) · in_flux no. The merge
   lost a D finding into an E finding.
4. Wave W3 in-flux column says "partly (`tab_badges.rs`)". viz
   `app/mcp/project.rs` is also in `in_flux_paths.txt`, and commit
   `89df3283` changed it after the baseline.
5. Section 3 "start W1, W2 and W3 in parallel". W2 is marked in flux, and
   `cli/src/job.rs` changed by 23 lines in `89df3283`. Re-read the F028 and
   F116 `job.rs` line numbers before W2 starts.
6. Row 7 (TH-FIELDLIST) has members with no wave: F118 (wire patch
   structs), F166 (wire tokens are `String`) and F048. W3 takes F181 and W11
   takes F187, F065, F101. Assign the other three, or say why they wait
   (selection P4 adds MCP rows to the same file).
7. Section 8 "Re-check F016, F128, F162 and F185 after that session lands".
   The tabs session landed (`e76f82c8`). `dressup/mod.rs` lost 264 lines and
   `compute/operation_configs.rs` changed by 24. F162 is re-checked above;
   F016, F128 and F185 are not.

Spot checks that held at HEAD: F176 (`io/tool_library.rs:298-309` keys 7
fields), F028 (`cli/src/run.rs:175-176` `unwrap_or(Grbl)`), F151
(`.github/workflows/ci.yml:6-7` branch `main`), F094 (`selection/item.rs:564`
still reads `outlines_pre_selection`).

## 4. Fixes that conflict with each other or with geometry selection

1. F033 (TH-VERDICT, W3, "first") against selection P3. F033 changes the
   height-order logic at `compute/config.rs:379,398` and
   `session/compute/export.rs:59`. P3 step 1 (`geometry_selection_2026-10-16/PLAN.md:179-180`)
   changes heights resolution to the setup frame. W3 does not name P3. F033
   must wait for P3, or P3 must own F033.
2. TH-APPLY (W9) makes `AppState.session` private. The viz has 435
   non-test `.session` references in 55 files. W9 lists the apply sites only
   and rates M-L; the blast radius is L. Selection P4 (viewport picks, MCP
   parity for every UI action) edits many of the same files. Order: after
   P4, not "after W3, W6".
3. TH-DOMAIN (W6) adds a `compile_error!` for an unbounded numeric field.
   P5b is running and changes the tab params (`PLAN.md:345`). A P5 field with
   no range fails the W6 build. Order: audit the P5 params, add derive tests
   (2.10), then W6.
4. F029 end state (`ToolShape` enum, W1) against F182 (exhaustive `ToolType`
   matches, W7). W7 rewrites matches that `ToolShape` deletes. Land
   `ToolShape` before W7, or move F182 into W1. F176's destructure in W1
   also changes when `ToolShape` lands.
5. F015 (W4) changes `Material::from_key` and `MachineProfile::from_key` to
   return `Option`. F091 (W15) deletes `Material::from_key`; F015's own
   evidence says both fns have test callers only. W4 deletes them; it does
   not retype them.
6. TH-LOAD `deny_unknown_fields` (W4) against P4 and P6, which add
   project-file keys, and against the open operator decision 5. Run the
   flatten and tag audit after P6.
7. TH-LINT (W16) replaces `allow` with `#[expect]`. TH-TESTDOOR gates items
   behind `test-support` and `research`. An `#[expect(dead_code)]` on an item
   that one feature set uses gives `unfulfilled_lint_expectations` in that
   set. The full-lint command uses three features and the per-crate test
   commands use the defaults. Check both feature sets in W16.

## 5. Areas a map reader expects to see and does not

| Area | Findings in REVIEW | Map lead with no finding |
|---|---|---|
| viz `ui/overlays/` | 0 | `viz-ui.md` lead 8 (string ids); see 2.12 |
| viz `ui/components/` | 1 (F086, dead pub item) | `viz-ui.md` leads 19, 26, 29 (unchecked) |
| `rs_cam_derive` | 0 of its own (F163, F185 name it as the fix site) | no tests; see 2.10 |
| `machine/` `$341`, `effective_kinematics` | F002 covers the feed name only | `core-io-output.md` leads 22, 36; see 2.6 |
| `settings/` global state | F034, F064 only | `core-session.md` lead 27; see 2.12 |
| `gcode/ir.rs` modal text | F005 (numbers) only | `core-io-output.md` lead 7; see 2.2 |
| `stock/` engagement partition | F087, F189 elsewhere | `core-stock.md` lead 1; see 2.4 |
| viz `compute/` backend trait | F054, F112, F158 | `viz-rest-and-small-crates.md` leads 6, 7; see 2.9 |
| `geometry/region_mask.rs` | F007, F010 (shapes) only | `core-io-output.md` lead 16; see 2.3 |
| `dressup/arcfit.rs` | 0 | `core-strategies.md` lead 27 (unchecked) |

Summary: the review is sound where the critic checked it. The gaps are
eleven checked candidates (2.1 to 2.11; four at tier B: 2.1 and 2.11 by a
past break, 2.3 and 2.5 by a broken documented rule), one lost D finding
(CS-17), one
count that grew during the review (F162), seven unswept checklist signals,
and seven wave conflicts, four of them with selection P3, P4, P5b and P6.
Folded into REVIEW.md on 2026-10-19: 24 new findings F195-F218, 7 claims fixed, 7 conflicts resolved.
