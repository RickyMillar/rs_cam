# Tech-debt review — arch consolidation programme (ab2e1466..d7192d35)

Independent review of the NEW and REWRITTEN code. No cargo ran. No repo file
was edited. S = under an hour, M = a half day.

Census, measured: core registry **45 rows** (42 `Command`, 2 `Query`, 1 `Job`;
`command.rs:95-529`). View registry **70 rows** (62 `UiCommand`, 8 `UiQuery`;
`ui_command.rs:345-880`). 20 new sentry files; 4 test files moved in-crate.

---

## HIGH — misleads the next engineer, or hides a defect

**H1. Saving a project clears the simulation WP11b just made load-bearing.**
`viz/controller/io.rs:321` and `viz/app/mcp/commands.rs:469` both call
`session.set_post_config(session_post)` **unconditionally** before a save, and
`core/session/mutation.rs:1651-1656` writes `session.simulation = None`. That
field was inert in the GUI before this programme; WP11b made it load-bearing —
`start_generate_toolpath` reads the rest snapshot from it
(`core/session/compute.rs:2519-2540, 2576-2579`) and REFUSES a
`FromRemainingStock` op when it is absent. Nothing re-adopts after a save: the
only `AdoptSimulation` producer is the simulation drain
(`viz/controller/events/compute.rs:780-783`). So a save between a simulation
and a rest generation sends every rest op back to "no simulated
remaining-stock snapshot is available" — the defect N12 item 10 closed. Two
further faults on the same lines: the MCP site MUTATES inside the *conversion*
step, which `app/mcp/commands.rs:10-14` says only reads; and both discard the
`Effects`, so no surface reports the cleared simulation.
**Fix:** guard both on `*session.post_config() != session_post`, as
`ui/properties/mod.rs:641-643` already does. That is the smallest correct
change; whether `set_post_config` should clear the simulation at all is a
separate question, because a post-config edit moves no geometry. **S**

**H2. The programme taught one route to stamp staleness and left the other.**
WP4 rewrote the MCP arm for `SetToolpathEnabled` to stamp the dropped set
(`viz/app/mcp/commands.rs:2471`), while the GUI site for the SAME row stayed
on `let _ =` (`viz/controller/events/mod.rs:139`). `Effects::stale` there is
the DOWNSTREAM set, so over the wire the downstream cards go yellow and in the
GUI they do not. `undo.rs:14,68` `set_stock_config` is the same shape against
a set G-FRESHSTATE says is EVERY toolpath: every card stays green after an
undo of a stock change.

Behind those two sit seventeen more. `Effects` is `#[must_use]`
(`command.rs:745`) and its doc accepts a bare `let _ =` with no reason, so
nineteen production sites take it — `events/mod.rs:139,176`;
`undo.rs:14,21,43,68,75,97`;
`model.rs:349,516,535,572,591,626,776,781`; `controller/io.rs:321,699`;
`ui/properties/mod.rs:643` — while thirteen others were migrated to
`state::stale::stamp_stale`. Not every one predates the programme; all
nineteen are sites the programme's own helper was built for and did not reach.
**Fix:** route each through `stamp_stale(&mut state, &effects.stale)`, GUI
`set_toolpath_enabled` and the two undo arms first. **M**

**H3. Two stale-stamp helpers that do not behave alike.**
`viz/state/stale.rs:31` uses `toolpath_rt_or_default`, which CREATES the
runtime row (`state/runtime.rs:390-394`). `viz/app/mcp.rs:2236-2254`
(`mcp_stamp_stale`) uses `toolpath_rt.get_mut` and SKIPS when the row is
absent, so a never-drawn toolpath is stamped on one route and not the other.
The stated reason for keeping them apart is FALSE: `stale.rs:18-19` says the
MCP helper "reads the window, not the state"; it reads
`self.controller.state()`.
**Fix:** delete `mcp_stamp_stale`, call `stale::stamp_stale`, delete the note. **S**

**H4. A WP12 sentry arm cannot fire, and passed before its own fix.**
`core/tests/loose_executor_is_crate_private_wp12.rs:251-288` greps
**non-comment** lines for the literal `"session/compute.rs"`. A file path in
Rust appears only in comments, and `is_comment` skips them at `:273`. The
motivating occurrence was a `//!` line (`gen_parity_p0_tests.rs:11` at
`89f81cb4^`), and commit `77627c33`'s own RED-OUTPUT block records this arm as
`... ok` on the **pre-fix** commit. Its `!files.is_empty()` guard (`:262-266`)
proves the walk read files, never that the needle can match.
**Fix:** drop the `is_comment` skip for this arm, or delete it and say in the
module doc that arms (a) and (b) carry the guarantee. **S**

**H5. Coverage was lost: the feed-optimisation refusals are now untested.**
WP11b (`4b53576b`) deleted `feed_optimization_uses_real_stock_bounds`,
`feed_optimization_rejects_remaining_stock` and
`feed_optimization_rejects_mesh_derived_operations` from
`viz/compute/worker/tests.rs` with the helper they drove. The behaviour moved
to `core/session/compute.rs:689-704`, gating on
`core/compute/catalog.rs:2767` (`feed_optimization_unavailable_reason`). **No
test in the workspace names that function** — core, viz, cli src, both test
trees and `catalog.rs`'s own test module. The nearest survivor,
`gen_inputs_one_assembly_n12.rs:162`, asserts `modulated > 0` on a `Fresh`
Pocket and touches neither refusal arm nor any grid bound. Verified: the eight
`rg` hits are the declaration, two re-exports, the gate, one GUI read
(`viz/ui/properties/mod.rs:6157`) and one doc line — no test.
**Fix:** one core test over the reason function plus a bounds assertion. **S**

**H6. The module doc states a row count wrong by seven.**
`core/session/command.rs:47-48` — "32 `Command` rows … 38 rows in total".
Measured: 42 and 45. The line was corrected once already for the same reason.
**Fix:** delete the counts; `CommandId::ALL.len()` is the count. **S**

**H7. Twenty `Reach::Skip` reasons name landed work packages as future work.**
`command.rs:139,159,185,229,239,259,269,279,300,310,320,330,340,356,396,407,417,427,437,447`.
12× "…WP6 adopts this row" — WP6 landed (`8eee4c3f`) and did NOT adopt them;
the GUI still calls `session.add_toolpath`
(`viz/controller/events/toolpath.rs:167,222`), `remove_toolpath`, `add_tool`,
`remove_tool`, `add_setup`. 7× "the GUI inspector writes this field directly;
WP5 gives it a door" — now FALSE; the inspector goes through
`Command::ReplaceToolpathConfig`. 1× `:447` "WP4 revisits" — it did not.
**Fix:** strip the WP clause from all twenty; restate the seven the way
`SetToolpathParam` at `:101` already does. **S**

**H8. "Every surface mutates through `ProjectSession::apply`" is not true.**
The claim is in commit `7dff635b`, in the WP7 sentry's failure message
(`core/tests/hatches_are_crate_private_wp7.rs`, arm 1) and in two doc comments
(`core/session/mod.rs:1789,1802`). WP7 closed the nine `*_mut` FIELD
accessors. About 21 public typed setters remain (`session/mutation.rs`:
`add_toolpath:101`, `set_toolpath_enabled:427`, `set_dressup_config:449`,
`set_boundary_config:639`, `set_toolpath_tool:1000`, `set_post_config:1651`,
plus `add_tool`/`remove_tool`/`add_setup`/`rename_setup`/`remove_model`/
`set_stock_config`/`set_machine`/…), and viz calls at least 19 directly (H2).
**No sentry covers them:** WP6 (`egui_..._wp6.rs:47-52`, four names), WP6b
(`non_egui_..._wp6b.rs:67-79`, ten) and WP7 (nine) scan `*_mut` only.
`IMPLEMENTATION_PLAN.md` §7 scopes the greps that way on purpose, so the plan
is intact; three surfaces state the wide claim anyway.
**Fix:** restate all three as "no surface outside core takes a `&mut` on a
session field", and open a §5 residual naming the typed setters. **S**

**H9. The §5 residual notes the audit trail points at were never written.**
`IMPLEMENTATION_PLAN.md` §23 ruling 3 (`:1463`) says the strategy advisor's
`execute_operation_annotated` "is listed in §5"; the §23 addendum (`:1503`)
says the 14-argument `execute_operation` "is a §5 residual"; `STATUS.md:255`
repeats both. §5 is `IMPLEMENTATION_PLAN.md:592-622` and it names **no
residual at all**.
**Fix:** add the two rows to §5 with `path:line` and the retiring condition. **S**

**H10. A doubled operator sentence kept for a landed work package.**
`viz/app/mcp/commands.rs:1671-1679` — `text(format!("Save failed: Save failed:
{error}"))`, commented "…WP6b retires the wrapper with the door". WP6b landed
(`06a7332d`); the wrapper stands (`controller/io.rs:326`).
**Fix:** drop the outer prefix and the comment. **S**

---

## MEDIUM — duplication or inconsistency worth a small follow-up

| # | Where | Smell | Smallest fix | Size |
|---|---|---|---|---|
| M1 | `viz/app/mcp.rs:2848,3375`, `:2216-2217`; `core/session/compute.rs:46,41,54` | The N15 two-producer defect survives on two wire paths. `IMPLEMENTATION_PLAN.md:674` requires `rg MutationKind crates/rs_cam_viz/src` → 0 after WP4 and `:316` requires `compute_stale_set` → `pub(crate)`; neither holds. `STATUS.md:244` states the amendment, §7's table does not, and §7's greps are not tests. | Correct the §7 rows to name the two holdouts (`add_toolpath_via_gui`, `apply_feeds`) and the condition that retires them | S |
| M2 | `core/compute/execute.rs:3357-3358`; `tests/loose_executor_..._wp12.rs:56-58,196-235` | A sentry pins a production-dead function: `execute_operation` (14 args) has zero production callers, and deleting it turns `found.len() == DECLARATIONS.len()` RED | Move the seven test callers to `execute_operation_annotated(..).map(\|at\| at.toolpath)`, delete both the fn and its `DECLARATIONS` entry | M |
| M3 | `hatches_..._wp7.rs:259-268`; `egui_..._wp6.rs:342-347`; `query_cycle_time_one_answer.rs:306-310` | Three vacuous guards. The first two increment unconditionally inside the loop, so the `assert_eq!` compares a constant to itself and the case its message names is undetectable. The third compares `CycleTime::NONE` to `CycleTime::NONE`; the oracle is never asserted non-NONE | Delete the two tautologies; assert `expected != CycleTime::NONE` in `assert_query_matches_oracle:245-275` | S |
| M4 | `command_registry_surfaces.rs:49`; `egui_..._wp6.rs:332`; `mcp_core_arm_describes_every_row.rs:139` | Three source scans `contains()` over UNSTRIPPED text, so a commented-out construction satisfies the claim — while `command_surface_completeness.rs:83-91` strips and documents the hole, and `egui_..._wp6.rs` even strips for its own scan 1 (`:134-161`) | Reuse `strip_comments` in all three | S |
| M5 | `mcp_core_arm_describes_every_row.rs:150` | The wildcard detector keys on exactly twelve spaces of indentation, so a rustfmt change makes it find nothing and pass GREEN. (`command_surface_completeness.rs:361,465` and `resolved_gen_inputs_...:172` are equally exact but fail loudly.) | Match `_ =>` after `trim_start()` and assert one arm was inspected | S |
| M6 | `command_registry_completeness.rs:117` ≡ `command_registry_surfaces.rs:93` | One exact duplicate test (same loop, population and claim), plus four subsumed: `egui_..._wp6.rs:167` and `non_egui_..._wp6b.rs:206` under `hatches_..._wp7.rs:288`; `gen_inputs_one_assembly_n12.rs:265,304` under `loose_executor_...:162` / `resolved_gen_inputs_...:221` | Delete the weaker of each pair | S |
| M7 | `mcp_mutation_rows_reach_core.rs:193-203` and five siblings | `tc()` / `fake_result()` / `fixture()` / `adopt()` copy-pasted verbatim across six new test files while `tests/common/session.rs` exists — and the drift has started: this copy dropped the `enabled` guard the other five carry (`command_registry_completeness.rs:347-350`) | Move the six onto `tests/common/session.rs` | M |
| M8 | `viz/app/mcp/commands.rs:75-146` | `CoreBefore` is an eight-slot bag whose accessors default silently: `index()` → `0`, `extra_f64` → `0.0`, `display_name()` → `""`, against the repo rule that absent means NOT MEASURED. All 18 reads match their 18 writes TODAY, so it is a trap with no victim | Return `Option<usize>` from `index()` and let each arm say what absent means | M |
| M9 | `viz/ui_command.rs:845-1035` vs `core/session/command.rs:1336-1561` | ~200 lines of near-verbatim muncher. The doc (`ui_command.rs:11-16`) justifies two REGISTRIES and says nothing about two MACROS; core already `#[macro_export]`s `for_each_command!` (`:92`) | Export the two generator macros from core, or state why a shared generator was rejected | M |
| M10 | `viz/ui_command.rs:929-952` | `UiQueryAnswer` has eight variants, every one `String`; `into_json` collapses them, so the answer column carries nothing the variant does not | Give the reads typed answers, or say the column mirrors core's shape and no more | M |
| M11 | `viz/ui/properties/mod.rs:590-757` | Four draft patterns in one `match`: scratch draft + `PanelEdit` (`:603,712,746`), direct write + per-frame diff-push through the typed setter (post, `:637-644`), draft + Apply/Revert + flush (tool, `:657-684`), and `draw_machine_panel` (`:652`). Only the tool panel flushes on navigate-away (`:96-107`), so an in-flight stock or setup drag abandoned by a selection change drops the draft (`:628`, `:728`) | Move post and machine onto `PanelEdit` + `Command`; give stock and setup the tool panel's flush | M |
| M12 | `core/session/command.rs:1076,1222,1231,930,1089` vs `:722,851,879,1013,1124,1145,1170,1240` | Payload boxing has no stated threshold: five config-carrying payloads are unboxed (`BoundaryConfig` is large, `compute/config.rs:1785`) while eight are boxed, and each boxing doc cites `large_enum_variant` without naming the size | State the threshold once in the WP4 payload banner (`:813-815`) and box to it | S |
| M13 | `command_surface_completeness.rs:172-181` | `P1_EXEMPT = [GenerateToolpath]`, doc "WP11b removes this exemption". WP11b landed and `viz/controller/events/compute.rs:305` constructs the job, so the exemption now hides a live row from the scan | Delete `P1_EXEMPT` and its guard at `:189` | S |
| M14 | `core/session/command.rs:787-792` | `Effects::created` reports an INDEX for three rows and an ID for `AddModel` — two quantities under one `usize` | Replace with a `Created` enum | M |
| M15 | `viz/controller/events/compute.rs:623-634` and `:2009-2020` | Three structurally identical boundary structs, two hand-copies a dozen lines apart in one function | Keep core's `SimBoundary` on the lane result and let the view read it | M |
| M16 | `core/session/mod.rs:1802-1807`; `core/session/save.rs:584` | `setups_mut` survives behind a `dead_code` guard for one test line that sets a pause message — `Command::SetSetupPauseMessage` (`command.rs:214`) already does that | Rewrite the test line through the row, delete the hatch and the `allow` | S |
| M17 | `viz/app/mcp/commands.rs:355-362,376-383,436-443` (+ ~12 more) | Each conversion reads the session, refuses a bad index in its own words, then hands the index to `apply`, which refuses again in core's — two refusal texts and two lookups per call | One `require_setup` / `require_toolpath` helper; keep the rows whose refusal genuinely says more | M |
| M18 | `core/session/command.rs:1684-1685` | `AddTool` and `AddToolFromLibrary` run one byte-identical arm over one payload type; documented at `:1111-1116`, still two rows differing by a string | Note that the split exists for the wire snapshot alone, or merge and carry the name on the request | S |

---

## LOW — cosmetic

- `command.rs:61` "the exception that proves the rule" — idiom, which STE
  forbids. `viz/app/mcp/commands.rs:2278` "import_model **will** re-derive…" —
  future tense for always-true behaviour. Both **S**.
- `command.rs:1907-1910` — the `SAFETY:` comment on `unwrap_or_else` argues
  against `unwrap_used`, a lint that call does not trip; it invites a reader to
  think a denied lint was worked around. **S**
- `command.rs:1914-1934` — `revision_snapshot` allocates a `Vec` and
  `moved_revisions` a `BTreeSet` on EVERY command, and `ReplaceToolpathConfig`
  runs on every frame the inspector is open (`:693-697`). **S**
- `viz/compute/worker/execute/mod.rs:300-321` — the WP11b doc sits on
  `run_compute`, which has zero production callers, not on
  `run_compute_with_phase_tracker` (`:332`), which production takes. **S**
- `ui_command.rs` — 52 of 70 view rows carry an invented wire name under
  `mcp: Reach::Skip`; nothing in `src/` reads `UiCommandId::wire_name` except
  `SurfaceId::wire_name` (`:1081`). One sentence saying the name serves the
  cross-registry uniqueness scan alone would stop the next author hunting for
  a tool. **S**
- Latent scan holes: `hatches_are_crate_private_wp7.rs:171-176` walks
  `src/session/` non-recursively and `:311` matches method-call syntax only;
  `non_egui_..._wp6b.rs:122-128` excludes the whole viz `src/compute/` tree and
  is covered only because the WP7 arm walks all of `rs_cam_viz/src`. **S**

---

## Dead code the programme left — census

Production callers counted outside every `#[cfg(test)]` boundary.

| Symbol | Declaration | Prod | Verdict |
|---|---|---|---|
| `execute_operation` (14 args) | `core/compute/execute.rs:3358` | **0** | dead; sentry pins it (M2) |
| `ProjectSession::setups_mut` | `core/session/mod.rs:1806` | **0** | dead; one test line (M16) |
| `MutationKind::{SetupChanged,ToolParamChanged,StockChanged}` | `core/session/compute.rs:47-49` | **0** | dead variants |
| `run_compute` | `viz/compute/worker/execute/mod.rs:320` | **0** | test wrapper; guard pre-dates |
| `MutationKind` / `StaleSet` / `compute_stale_set` | `core/session/compute.rs:46,41,54` | 2/1/1 | alive — M1 |
| `mcp_apply_stale` | `viz/app/mcp.rs:2214` | 2 | alive |
| `insert_result` | `core/session/mutation.rs:1889` | 1 | alive (`command.rs:1600`) |
| `execute_operation_annotated` (16) | `core/compute/execute.rs:3406` | 1 | alive (advisor, `session/compute.rs:1716`) |
| `core_simulation_from_lane` | `viz/controller/events/compute.rs:2001` | 1 | alive |
| `cached_auto_index` / `GenObserver` | `core/geom_cache.rs:279`, `session/compute.rs:495` | 3/4 | alive |
| `LaneInner` (8 fields) / `ComputeResult` (6 fields) | `viz/compute/worker.rs:419, :93` | all read | **no dead field** |

The programme added exactly three `dead_code` allows
(`core/compute/execute.rs:3357`, `core/session/mod.rs:1805`,
`viz/compute/worker/gen_parity_p0_tests.rs:275`); the other 27 pre-date it.
Six of the nine WP7 hatches are gone outright; three survive `pub(crate)` with
0, 1 and 1 production callers. Lint policy is respected: no `#[allow]` in new
production code except `viz/app/mcp/commands.rs:1827-1828` with a SAFETY line,
and no `unwrap`/`expect`/`panic`/`print_*`.

## What is healthy, and should not be touched

The kind-split muncher (`command.rs:1336-1561`); `try_with_effects` /
`with_effects` (`:1876-1911`), one construction site measured from the
revision map rather than a drop list — the programme's best idea;
`ResolvedGenInputs` / `GenContext` / `GenerateToolpathHandle`
(`core/session/compute.rs:202-420`), private fields, no `Default`, no `Clone`,
one producer, and `GenContext` duplicates none of the bundle;
`geom_cache::LazyIndex` (`:318-350`) and `ProjectSessionBuilder`
(`core/session/builder.rs:22-46`); `core_simulation_from_lane`
(`viz/controller/events/compute.rs:1982-2029`), whose `cut_trace: None` is
deliberate and explained at the adopt site (`:765-772`); `PanelSideEffects`
(`viz/state/mod.rs:222-237`), one struct and one discharge point consistent
across five raisers; and `mcp_wire_surface_pin.rs`, the strongest sentry here
— it reads the shipped router rather than source text, and its update hatch
(`:196-211`) rewrites then panics rather than passing green. The four WP12
test-file moves lost nothing: byte-parity, all four `mod` declarations present
(`core/compute/execute.rs:4513,4523,4532,4543`).

---

## Top five follow-ups, in order

1. **H1** — guard the two `set_post_config` calls before a save. A save
   currently discards the rest-stock snapshot WP11b installed. Add a sentry:
   simulate, save, `start` a `FromRemainingStock` op, require it to resolve. **S**
2. **H5 + H4** — restore the feed-optimisation refusal coverage over
   `feed_optimization_unavailable_reason`, and fix the WP12 arm that cannot
   fire. Both report green over nothing today. **S**
3. **H2** — stamp staleness at the nineteen `let _ =` sites, starting with
   `events/mod.rs:139` and `undo.rs:14,68`, where the GUI and MCP routes
   already disagree about one row. **M**
4. **H6 + H7 + H9 + H10 + M13** — one prose pass over the landed-WP
   references: twenty `Skip` reasons, the 38-row count, the missing §5
   residuals, the doubled "Save failed", the `P1_EXEMPT` note. **S**
5. **H8 + H3** — restate the "every surface mutates through apply" claim to
   what WP7 closed, open a §5 residual listing the 21 public setters, and merge
   `mcp_stamp_stale` into `stamp_stale`. **S**

---

## Overall assessment

The new code is in good health and unusually well documented for a refactor
this size. The registry, the three job steps, the two resolved-input bundles
and the builder each state their invariants, name the defect that motivated
them, and point at their own sentry. The core shapes are right: one `Effects`
construction site, one generation-input resolver, private fields with a single
producer, exhaustive matches with no wildcard, and a test suite that mostly
carries real non-vacuity floors. Almost nothing here is structurally wrong.
What the programme left is an **honesty gap between the prose and the reach**:
several surfaces say the door is closed ("every surface mutates through
apply", "the draw sites write through commands") when what closed is the
narrower `*_mut` hatch set; about twenty `Skip` reasons and three test
comments still name work packages that landed the same day as if they were
future; and two residual ledger entries were cited but never written. That gap
is cheap to fix and expensive to leave, because the next engineer will trust
it. Beside it sit four measurable holes: a save that clears the simulation the
programme just made load-bearing (H1), thirteen call sites migrated to
`stamp_stale` while nineteen stayed on `let _ =` (H2), a sentry arm that
passed before its own fix (H4), and one genuinely lost test area (H5). Fix
those four, do one prose pass, and this is a clean base.
