# Implementation plan — one command surface across the nine phases

> **Revision 2, 2026-09-11.** Round-1 adversarial review returned REJECT with
> ten blocking findings. All ten are addressed; §10 is the changelog and
> records the two places I disagree with the reviewer's arithmetic.

## §0 Purpose and reading order

This file sequences the adopted ruling (`RULING_ONE_COMMAND_SURFACE_DRAFT.md`, ADOPTED
`STATUS.md:202-206`) across the nine phases of `PLAN.md`. `PLAN.md` stays verbatim; it says
WHAT and this file says IN WHAT ORDER, WITH WHICH DELETION, AND UNDER WHICH SENTRY. Read
`STATUS.md` first for finding state, then the ruling, then this file. I read every
`path:line` cited below in the file itself, at `master` HEAD `80219e69`. Nothing here is
implemented. A verifier commits on this branch concurrently, so a line number can drift; the
symbol name is the anchor.

---

## §1 Target state, restated as types

### The registry

One `macro_rules!` X-macro in `rs_cam_core::session::command`, in the idiom of
`for_each_op!` (`crates/rs_cam_core/src/compute/catalog.rs:149-181`, two callback
invocations at `:222` and `:256`).

```rust
macro_rules! for_each_command { ($m:ident) => { $m! {
    // kind     Id                wire name             payload               surfaces
    (Command,  SetToolpathParam, "set_toolpath_param", SetToolpathParamArgs,
     Surfaces { gui: Skip("..."), mcp: Reached, cli: Reached }),
} }; }
```

- `Command` — a payload enum, one variant per `Command` row. The payload TYPE is a row
  column (`RULING:86-88`), so the row names it and the callback macro writes
  `SetToolpathParam(SetToolpathParamArgs)`.
- `CommandId` — a FIELDLESS mirror enum. `ALL`, `wire_name()`, `kind()` and `surfaces()`
  hang here. `for_each_op!` works because `OperationType` carries no fields
  (`catalog.rs:216-219`); a payload enum cannot host a `const ALL`.
- `CommandKind { Command, Query, Job, UiCommand }`. `Command` is a synchronous validated
  mutation returning `Effects`; `Query` a synchronous read; `Job` three synchronous steps,
  never an `async fn` — (i) `apply(Job::..)` captures cheaply on the frame loop, (ii)
  `resolve` then `execute` run off the loop holding no `&mut` session, (iii)
  `apply(Command::AdoptResult { .. })`; `UiCommand` is viz only and never enters core.
- `Surfaces` — a STRUCT with one field per surface, not a list, so an omitted surface is a
  compile error. `Reach { Reached, Skip(&'static str) }`. **DEPARTURE from `RULING:86-88`**,
  which writes the surfaces as a list; a list cannot fail to compile when a surface is
  forgotten.

**The compiler guarantees** exactly two things here: a `Surfaces` literal that omits a field
does not compile, and a `match` over `CommandId` that omits a variant does not compile.
Everything else — the wire name is unique and non-empty, every `Skip` carries a reason,
every `Reached` surface resolves a handler — is test-time.

### `Effects`

`Effects { stale: BTreeSet<usize>, simulation_cleared: bool, revision: u64 }`.

I read the chain (`crates/rs_cam_core/src/session/mutation.rs:236-343`).

- **`stale` is the set `drop_result` was called on.** That equals the revision-moved set,
  because `drop_result` (`:1328-1332`) bumps a revision on every call. A second bump
  site exists: `bump_all_revisions` (`:1338-1342`) moves EVERY index on removal and
  bulk replace and never calls `drop_result`. `Effects.stale` must therefore be built
  from the revision-moved set (compare `toolpath_revision` before and after), not from
  the `drop_result` call list, or a removal reports nothing stale. It equals the `dirty` set
  built at `:252-253` and grown at `:308-310` ONLY on the `invalidate_result_chain` paths.
  It does NOT equal `dirty` on the enable-toggle path: `set_toolpath_enabled` (`:327-343`)
  calls `invalidate_output_dependents(index, true)` and never `invalidate_result_chain`, so
  `dirty` contains `index` while `index`'s own result and revision deliberately survive
  (N1's design). Reporting `dirty` there would mark an operation stale whose result core
  kept on purpose.
- **`simulation_cleared`** must become `self.simulation.take().is_some()`. Today `:249`
  writes `None` unconditionally, so "the code wrote `None`" is not evidence a simulation
  existed.
- **`revision` is `toolpath_revision(index)`** (`session/mod.rs:1451`, per-index map at
  `:1297`), NEVER `next_revision` (`:1299`): that is a session-global counter that "Never
  reset"s and that `drop_result` bumps for ANY index, so comparing against it rejects every
  completion whenever any other toolpath is edited. `toolpath_revision`'s doc names R0.1
  §4.2 for exactly this use. Compile-time: nothing. Test-time: one producer, so the reply
  and the drop cannot disagree.

### `ResolvedGenInputs`

Today it is a PRIVATE struct — `struct ResolvedGenInputs` at
`crates/rs_cam_core/src/session/compute.rs:192`, no `pub` — with 17 private fields
(`:192-235`), built by a PRIVATE resolver (`:1090`). Viz could not call it, so viz wrote
`ComputeRequest`, 27 public fields (`crates/rs_cam_viz/src/compute/worker.rs:49-134`), 13 of
which mirror it. Target: `pub struct ResolvedGenInputs` whose fields stay private, one
public resolver as its only producer, and the executor narrowed from 21 loose arguments
(`crates/rs_cam_core/src/compute/execute.rs:3470-3505`, under
`#[allow(clippy::too_many_arguments)]` at `:3469`) to `&ResolvedGenInputs` plus the cancel
flag.

**The compiler guarantees viz cannot assemble a second answer only if three constraints
hold**, and WP11a must assert all three: the published type derives no `Default`; it exposes
no `pub` setter and no `pub` field; and exactly one `pub fn` returns it. The sentry grep is
`rg -n "\-> ResolvedGenInputs" crates/rs_cam_core/src` → 1. Without those, the private
fields buy nothing. The same rule applies to `ResolvedHeights`, which exists twice
(`crates/rs_cam_core/src/compute/config.rs:1656`,
`crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:58`).

### The ELEVEN hatches

**CORRECTION to `RULING:137-140`, which lists six.** `ProjectSession` declares ten `pub fn
*_mut` doors (`rg -n "^\s*pub fn [a-z_0-9]*_mut" crates/rs_cam_core/src/session/*.rs`) plus
`insert_result`.

| Hatch | Declared at | viz prod | CLI prod |
|---|---|---|---|
| `stock_mut` | `session/mod.rs:1590` | 6 | 3 |
| `machine_mut` | `session/mod.rs:1598` | 10 | 1 |
| `tools_mut` | `session/mod.rs:1607` | 2 | 0 |
| `models_mut` | `session/mod.rs:1615` | 4 | 0 |
| `post_mut` | `session/mod.rs:1620` | 2 | 3 |
| `wizard_mut` | `session/mod.rs:1630` | 12 | 0 |
| `find_toolpath_config_by_id_mut` | `session/mod.rs:1650` | 1 | 0 |
| `find_setup_by_id_mut` | `session/mod.rs:1696` | 4 | 0 |
| `toolpath_configs_mut` | `session/mod.rs:1710` | 5 | 2 |
| `setups_mut` | `session/mod.rs:1720` | 1 | 0 |
| `insert_result` | `session/mutation.rs:1284` | 1 | 0 |
| **Total** | | **48** | **9** |

`models_mut` is the door the model-refresh family writes through — the exact class
`G-RESCALESTALE` and `G-RELOADTARGETS` were opened for. `post_mut` writes
`spindle_strategy`, which reaches emitted G-code. Leaving either `pub` makes WP7's goal
false at its own definition of done.

Twelve of the 48 sit inside egui draw code: `properties/mod.rs:137, 282, 374, 408, 418,
1277, 1360, 1421, 1437, 1465, 1601, 1732`.

### `AppEvent` and `McpRequestKind`

**DEPARTURE from `RULING:99-100`**, which lists the `McpRequestKind` variant among the
generated items. Both enums take a WRAPPER variant instead: `AppEvent::Core(Command)` and
`McpRequestKind::Core(Command)`. That is what the row-1 plan's hand-written `From<Command>
for McpRequestKind` converges on. The deletion it buys is the point: the classified
Mutations arms of `handle_mcp_request` (`crates/rs_cam_viz/src/app/mcp.rs:227-975`) collapse
to one delegating arm.

---

## §2 Mapping the ruling onto the nine phases

| Phase | What the ruling makes it produce | Closes | Deletes |
|---|---|---|---|
| 0 | The registry, `Effects`, one row, the completeness sentry, the MCP wire pin | N15 at one site | the vacuous pin `compute.rs:5385` |
| 1A | Every mutation returns `Effects`; the eleven hatches `pub(crate)`; `AppEvent::Core` / `McpRequestKind::Core` wrappers | Finding 2, N6, N14, N15 | `MutationKind`, `compute_stale_set`, `mcp_apply_stale`'s compute half, the classified MCP Mutations arms, the `AppEvent` mutation arms |
| 1B | `Job` step (iii): `Command::AdoptResult { id, revision, result }` carrying the `PLAN.md:167-173` stamp | Finding 3 residue | `insert_result` as a public door |
| 1C | Consumers read artifacts through accessors; `Query` rows | Finding 3 | duplicate result reads |
| 2 | `ExportGcode` promoted from `Command` to `Job` if it exceeds the frame budget | Finding 4 | two duplicate phase builders |
| 3 | `ResolvedGenInputs` public with private fields; one resolver; executor narrowed | Finding 1, N12 | `ComputeRequest`'s 13 mirrored fields, the 21-argument signature, 14 "mirrors session/compute.rs" comments |
| 4A | Canonical import bundle behind one `Command` row | Finding 6 | three of four extension dispatches |
| 4B | The row's payload type carries the validity rules | Finding 7, N11 | per-surface validation copies |
| 5A-C | Drill/emitter/offset contracts consumed by the one executor | Findings 8, 9, 10 | five twice-written drill expressions, five hand-rolled emitters |
| 6A-B | `Query` rows for timing and vendor lookup | Findings 5, 11 | three of four provenance vocabularies |
| 7 | — (no registry content) | Finding 12 | two of three weak-identity memos |
| 8 | The architectural gate reads as greps that return zero | — | — |

**Scope of §4.** WP1..WP6b, WP7 and WP13 land Phase 0 and Phase 1A-1B; WP9 opens 1C;
WP10..WP12 land Phase 3. **Phases 2, 4A, 4B, 5, 6, 7 and 8 get their own packages after
WP13**, written against the registry this plan builds. §7 states the done condition for the
phases §4 reaches, not for all nine.

**Two things `PLAN.md` lacks and where they go.** (a) The `UiCommand` split: `PLAN.md` never
separates viewport-only capability from project capability, so **10** `McpRequestKind`
variants and part of `AppEvent` have no phase. They go in Phase 1A as WP13, classified
before any of them moves. (b) The cross-surface completeness sentry: `PLAN.md`'s
architectural gate (`:495-504`) states seven properties in prose and names no test. WP13
turns them into the sentry pair plus the §7 greps.

---

## §3 Dependency graph and order

Edges are forced by TYPES, not by finding numbers.

| WP | Subject | Requires | State |
|---|---|---|---|
| WP1 | row 1: registry + `Effects` born | — | ready |
| WP2a | MCP wire pin (snapshot; `CommandId` arm added at WP1) | — | ready |
| WP3 | `Effects` everywhere + `AdoptResult` | WP1 | ready |
| WP4 | the MCP Mutations section + 4 reclassified rows | WP1, WP3, WP13's table | ready |
| WP5 | `ReplaceToolpathConfig` (GUI toolpath door) | WP3 | BLOCKED (Q1) |
| WP6 | egui scratch-copy pattern, 12 draw sites | WP1, WP4 (the stock / machine / tool / setup rows) | ready |
| WP6b | the 27 non-egui viz sites and 9 CLI sites | WP4 | ready |
| WP7 | eleven hatches `pub(crate)` | WP3, WP5, WP6, WP6b, and the Q5 door | BLOCKED (Q5) |
| WP8 | N14 / N6 undo, optimizer, drill picks | WP3 | BLOCKED (Q2) |
| WP9 | `Query` first row: cycle time | WP1 | ready |
| WP11a | publish `ResolvedGenInputs` with private fields | WP1 | ready |
| WP10 | `Job` first row: generate one toolpath | WP3, WP11a | ready |
| WP11b | narrow the executor to `&ResolvedGenInputs` | WP10, WP11a | ready |
| WP12 | delete `ComputeRequest`'s mirrored fields | WP11b | ready |
| WP13 | `UiCommand` split + cross-surface sentry | WP1 | ready |

Four edges the ruling's own ordering hides.

1. `AdoptResult` needs `Effects` and `toolpath_revision`, NOT the `Job` kind — so
  `insert_result` closes at WP3, earlier than `RULING:143-145` implies.
2. WP10's sentry is a viz integration test that must name `ResolvedGenInputs`. An external
  crate cannot name a private type, so **WP10 requires WP11a**, not the reverse. That is why
  WP11 splits.
3. WP6's twelve sites are `tools_mut`, `stock_mut`, `machine_mut` and `find_setup_by_id_mut`
  — none is the toolpath door. WP6 needs `Command` rows for the stock, machine, tool and
  setup mutations, and only WP4 creates them. **WP6 requires WP4, and WP6 is NOT parallel
  with WP4.**
4. WP13's classification table decides which Mutations rows WP4 may move, so the table
  precedes WP4 even though the variant moves follow it.

**Parallelisable** (different crates or files, no shared edit): WP1 ∥ WP2a; WP8 ∥ WP9 ∥
WP11a; WP6 ∥ WP6b after WP4. **Strictly serial:** WP1 → WP3 → WP4 → {WP6, WP6b} → WP7; WP1 →
WP3 → WP5 → WP7; WP11a → WP10 → WP11b → WP12.

---

## §4 Work packages

### WP1 — Row 1: `SetToolpathParam` through `apply`

- **Goal.** Prove the registry, `Effects` and one declared surface set on the one row that
  already reaches core, MCP and the CLI.
- **Moves.** `set_toolpath_param` (`crates/rs_cam_core/src/session/compute.rs:336-626`)
  becomes `set_toolpath_param_impl` returning `Effects`; the chain call at `:623` returns
  the set. MCP: `app/mcp.rs:3444-3480` calls `apply` and passes `effects.stale` to a new
  `mcp_stamp_stale` half split out of `mcp_apply_stale` (`:2708-2730`). CLI: three sites
  (`crates/rs_cam_cli/src/run.rs:137`, `job.rs:629`, `smoke.rs:586`) call one new bin-crate
  `apply_command`. `rs_cam_cli` declares no `[lib]`, so its test is a `#[cfg(test)]` module
  inside the bin.
- **Deletes.** `compute_stale_set_for_toolpath_param_returns_single_toolpath`
  (`compute.rs:5385` at `80219e69`). It is VACUOUS-GREEN, not red: `make_session()` plus one
  `Fresh` Pocket, so a chain-aware answer is also `[0]`. `PLAN.md:111` forbids freezing a
  defect as desired behaviour.
- **Sentry.** `crates/rs_cam_core/tests/command_registry_completeness.rs` (red first: it
  does not compile) and `crates/rs_cam_viz/tests/command_registry_surfaces.rs` (the `From`
  bridge compiles, plus a source scan for `name = "<wire_name>"` in `mcp_server.rs`, in the
  idiom of `crates/rs_cam_viz/tests/overlays_registry.rs:60-90`).
- **P0 flip.** `mutation_paths_invalidate_alike_p0.rs:449-475`
  (`n15_compute_stale_set_reports_one_index_while_the_setter_drops_two`). The flip is NOT
  `assert_ne` → `assert_eq` on the same operands: rewrite the arm to compare
  `apply(..)?.stale` against `observe(&s, &before).dropped`. KEEP the `compute_stale_set ==
  vec![0]` assertion at `:453-458` as a separately named pinned divergence.
- **Rollback.** One core module, one MCP arm, one CLI function. Revert restores the setter,
  which stays a thin wrapper through this WP. **Size.** M — the registry is new, the
  behaviour is not.

### WP2a — MCP wire compatibility pin (shape, not only name)

- **Goal.** Nothing disappears from the wire, and nothing changes shape, during the
  migration.
- **Moves.** Nothing. New test only.
- **Sentry.** `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs`. Build the router and
  serialise its own `list_tools` answer — name plus `inputSchema` per tool — and compare
  against a checked-in JSON snapshot. A source scan for `name = "..."` (78 literals today)
  cannot see a field added, removed, renamed or moved between required and `Option`, nor a
  tool whose literal survives while its method leaves the `#[tool_router]` impl. The
  snapshot covers `rs_cam_mcp`'s 57 `pub struct`s (56 suffixed `Param`/`Input`/`Args`)
  through their derived schemas, so the struct-count assertion is not needed.
- **Two stages.** The 78-name-plus-schema snapshot lands alone (no dependency). A second arm
  — every `CommandId` whose `surfaces().mcp` is `Reached` appears in the snapshot's name set
  — is added at WP1, because `CommandId` is born there.
- **P0 flip / Rollback.** None; delete one file. **Size.** S.

### WP3 — `Effects` everywhere; `AdoptResult`

- **Goal.** One producer of the stale answer for every mutation, and a result adoption that
  can reject a stale completion.
- **Moves.** Every `pub fn` on `ProjectSession` that today ends in `invalidate_result_chain`
  / `drop_result` returns `Effects`. `invalidate_output_dependents` returns `(dirty,
  dropped)`; `Effects.stale` carries the revision-moved set, which equals `dropped` on the
  `invalidate_result_chain` paths (§1), so `set_toolpath_enabled`
  (`mutation.rs:327-343`) goes on reporting the downstream set only and keeps its own
  result. `insert_result` (`mutation.rs:1284`) becomes `apply(Command::AdoptResult { id,
  revision, result })`; its ONE non-test caller is
  `crates/rs_cam_viz/src/controller/events/compute.rs:973`. The stamp compares
  `toolpath_revision(index)`, not `next_revision` (§1).
- **THE CALL-SITE COST.** Changing `Result<(), SessionError>` to `Result<Effects,
  SessionError>` breaks every `Ok(()) =>` match arm on those functions: **33** today (`rg -c
  "Ok\(\(\)\) =>" crates/rs_cam_viz/src crates/rs_cam_cli/src` — `app/mcp.rs` 23,
  `controller/events/model.rs` 3, `app.rs` 2, `app/input.rs` 2, `app/export.rs` 1,
  `ui/feeds_modal.rs` 1, `controller/events/mod.rs` 1, CLI 0). Not all 33 match a converted
  function, so triage them first; every one that does must be rewritten in the SAME commit,
  because the workspace does not compile between.
- **Deletes.** `insert_result` as a public door.
- **Sentry.** `crates/rs_cam_viz/tests/adopt_result_rejects_stale_completion.rs`, red first,
  TWO arms: (a) submit, edit THIS toolpath, deliver the old result, assert refusal and an
  empty cache — today it overwrites; (b) submit, edit a DIFFERENT toolpath, deliver the
  result, assert it is ACCEPTED. Arm (b) is what fails if the stamp reads `next_revision`.
- **P0 flip.** None. `mutation_paths_invalidate_alike_p0.rs:336-360` and `:362-379` must
  stay green UNCHANGED — they are the contract `Effects.stale` is defined against.
- **Rollback.** Not one call site: the 33-arm rewrite reverts with it. **Size.** L.

### WP4 — The MCP Mutations section onto `McpRequestKind::Core`

- **Goal.** One MCP mutation arm, not thirty-four.
- **Moves.** `McpRequestKind`'s Mutations section
  (`crates/rs_cam_viz/src/mcp_bridge.rs:555-761`, **34** variants) becomes
  `McpRequestKind::Core(Command)`. **CLASSIFY BEFORE MOVING (WP13's table) — the section is
  not homogeneous.** Its own doc comments contradict its header: `GetNotifications` (`:614`)
  says "read the toast stack. Read-only: it removes nothing", so it is a `Query`;
  `AddToolpathViaGui` (`:609`) dispatches `AppEvent::AddToolpath` and is the GUI-door
  composition `RULING:104-105` keeps hand-written. "34 arms become one" is the ceiling, not
  the answer.
- **FOUR ROWS JOIN FROM OTHER SECTIONS.** `ImportMachineSettings` and
  `LoadMachineFromLibrary` sit in the UI-navigation section (`:846-873`) but write
  `session.machine_mut()` at `app/mcp.rs:2372` and `:2457`. They are `Command` rows and land
  here; without that WP7's `machine_mut` grep cannot reach zero. `ListMachineLibrary` (same
  section) and `ReachMap` (`:805-845`) are `Query` rows — `machine_library.rs` is a core
  module.
- **SIXTEEN PRODUCERS, THREE HOLDOUTS.** `mcp_apply_stale` has **15** call sites
  (`app/mcp.rs:2876,2933,3464,3648,3737,3796,3874,3912,4313,4635,4755,
  5185,5222,5291,5417`). The sixteenth is outside it: `mcp_set_toolpath_enabled` passes a
  hardcoded `vec![index]` at `app/mcp.rs:5262` and stamps no `stale_since`. Three of the
  sixteen keep a HAND-WRITTEN stale set until their core mutation returns `Effects`: `:3648`
  (inside `add_toolpath_via_gui`, the composition this WP keeps hand-written) and `:2876` /
  `:2933` (`SetupChanged`). **`compute_stale_set` therefore goes `pub(crate)` here and is
  DELETED only when all sixteen read an `Effects`** — deleting it earlier leaves those
  replies' `stale_toolpaths` with no producer, a capability loss on the wire (programme rule
  4).
- **Owns these hatch sites.** `app/mcp.rs:2372, 2457, 2815, 3170, 4509, 5341`.
- **Deletes.** `MutationKind` (`compute.rs:44-51`), `StaleSet` (`:40`), their re-export
  (`session/mod.rs:23`), their four sibling unit tests (`compute.rs:5374-5411`),
  `mcp_apply_stale`'s compute half, and the classified Mutations arms in
  `handle_mcp_request`. N15 closes here.
- **Sentry.** WP2a's `CommandId` arm, extended to one arm per `Reached` MCP row; WP2a's
  snapshot proves no wire name or schema moved.
- **P0 flip.** WP1's kept `compute_stale_set` divergence assertion is deleted with the
  function, at the end of this WP.
- **Rollback.** Mechanical per row. **Size.** L.

### WP5 — The GUI inspector toolpath door

- **Goal.** Give viz a validated toolpath write door and close
  `find_toolpath_config_by_id_mut`.
- **Moves.** `generation_inputs_signature`
  (`crates/rs_cam_viz/src/ui/properties/mod.rs:3713-3726`) moves into core.
  `write_entry_config_to_session` (`:3737-3796`) writes **16** fields at `:3759-3782`
  through `find_toolpath_config_by_id_mut` (`:3742`) and calls `invalidate_toolpath_inputs`
  (`:3792`). It becomes one `Command::ReplaceToolpathConfig { index, config }` on the
  existing wide, zero-caller `replace_toolpath_config` (`mutation.rs:1204-1224`).
  **Constraint I read:** that function invalidates UNCONDITIONALLY, so the signature gate
  MUST move into core with it, or the panel drops every result on every frame it is open.
- **Deletes.** The viz-side `generation_inputs_signature`.
- **Sentry.** `crates/rs_cam_viz/tests/inspector_door_is_one_command.rs`: a no- op frame
  produces an empty `Effects.stale`; a real edit produces the setter's set.
- **P0 flip — and a non-vacuity trap.** `mutation_paths_invalidate_alike_p0.rs` runs four
  arms. Arm 3 `arm_inspector_door` (`:299-310`) calls `invalidate_toolpath_inputs`; arm 4
  `arm_replace_config` (`:311-327`) calls `replace_toolpath_config`. Retargeting arm 3 at
  `Command::ReplaceToolpathConfig` would make
  `the_setter_the_inspector_door_and_the_replacement_agree` (`:336`) compare arm 4 against
  arm 4. **Keep arm 3 on the raw core door** and add a FIFTH arm for the command, so the
  file keeps the independent comparison its own non- vacuity guard (`:196`) exists to
  protect.
- **Rollback.** The panel keeps its own door until the sentry is green. **Size.** M.
  **BLOCKED (Q1).**

### WP6 — The egui scratch-copy pattern, twelve draw sites

- **Goal.** Design the scratch-copy plus emit-on-change pattern ONCE, for every hatch site
  inside draw code.
- **Moves.** `crates/rs_cam_viz/src/ui/properties/mod.rs`: `:137` (`tools_mut` inside a
  `find`), `:282` (hands `stock_mut()` to the whole draw function `stock::draw`), `:374,
  408, 418` (`find_setup_by_id_mut`, the setup datum panel — `RULING:37`), `:1277, 1360,
  1421, 1437, 1601, 1732` (`machine_mut`), `:1465` (`stock_mut`). Twelve, not nine. The
  pattern: the panel owns a scratch copy for the frame, the widget writes the scratch, and a
  change detected at frame end emits one `Command`. `:282` is the hard one — `stock::draw`'s
  signature must take a scratch `&mut StockConfig`.
- **Not all twelve are pattern changes.** `:1421` already has the shape: the widget writes
  local `max_feed` / `max_shank` and `let m = machine_mut()` sits inside `if specs_changed`
  (`:1420-1425`). It is a hatch call needing a `Command`, nothing more.
- **TWO SAME-FRAME READERS THE SCRATCH BREAKS.** A dragging slider still renders correctly
  (`Slider::new(&mut scratch, ..)` shows the scratch that frame). What breaks: (a) the
  Conservative / Balanced / Aggressive label at `:1446-1455` reads
  `state.session.machine().safety_factor`, so it lags the handle for the whole drag — it
  must read the scratch; (b) the stock undo push at `:284-295` compares `old !=
  *state.session.stock_config()` right after `stock::draw`, so with a scratch at `:282` it
  is always false and no undo entry is pushed — it must compare the scratch. The drag-frame
  cost EXISTS TODAY: `:1437-1443` pushes `AppEvent::MachineChanged` on every `.changed()`
  frame, so emit-on-release REDUCES work. **State in the package that invalidation is one
  frame late on a drag** — the session holds the old value until release — and say per
  widget what the mid-drag label reads.
- **Deletes.** Twelve direct hatch uses.
- **Sentry.** `crates/rs_cam_viz/tests/inspector_emits_one_command_per_change.rs` — one
  `Command` per completed edit, the label reads the scratch, and a stock drag still pushes
  exactly one undo entry. Plus a frame-time measurement on a project with cached results:
  `PLAN.md` rule 5 forbids buying sharing with GUI responsiveness.
- **P0 flip.** None. **Rollback.** Per widget. Land it widget by widget, not in one patch.
  **Size.** L.

### WP6b — The 27 non-egui viz sites and the 9 CLI sites

- **Goal.** Give every remaining production hatch call an owner, so WP7 can land at all. WP7
  is visibility-only and all-or-nothing: the workspace stops compiling the moment it lands
  until every external caller has moved.
- **The arithmetic.** 48 viz production sites: WP3 owns 1
  (`controller/events/compute.rs:973`), WP4 owns 6 (`app/mcp.rs:2372, 2457, 2815, 3170,
  4509, 5341`), WP10 owns 1 (`app/mcp.rs:5480`), WP5 owns 1 (`properties/mod.rs:3742`), WP6
  owns 12. **27 remain, and this WP owns them, plus all 9 CLI sites.**
- **Moves — viz (27).** `app/input.rs:161, 166, 171, 176, 181, 186, 191, 198, 208, 216, 221`
  (10 `wizard_mut` plus `setups_mut:198`); `app/export.rs:254` (`wizard_mut`);
  `controller/io.rs:25, 61, 114` (`stock_mut`) and `:96, 158, 254, 655` (`models_mut` — the
  model-refresh family, `G-RESCALESTALE` class); `controller/events/mod.rs:346` (`post_mut`,
  spindle strategy), `:415, 938, 1027` (`toolpath_configs_mut`; `:938` is the N13 feeds
  funnel); `controller/events/model.rs:183` (`machine_mut`), `:370` (`stock_mut`);
  `controller/events/undo.rs:109` (`tools_mut`); `controller/events/compute.rs:1708`
  (`toolpath_configs_mut`).
- **Moves — CLI (9).** `job.rs:535` (`stock_mut`), `:642` (`post_mut`); `run.rs:141`
  (`post_mut`); `project.rs:241` (`machine_mut`), `:248` (`post_mut`), `:738`
  (`toolpath_configs_mut`); `smoke.rs:660`, `:1084` (`stock_mut`), `:673`
  (`toolpath_configs_mut`).
- **Deletes.** 36 direct hatch uses.
- **Sentry.** The §7 WP7 grep, run at this WP's end over `crates/rs_cam_viz/src` minus the
  egui file and over `crates/rs_cam_cli/src` — the residue must be exactly WP6's twelve.
- **P0 flip.** None. `gen_parity_p0_tests.rs` and the CLI's 31 integration tests must stay
  green.
- **Rollback.** Per file. **Size.** L, and it is the package most likely to be
  under-estimated: the wizard alone is 12 sites across two files.

### WP7 — The eleven hatches go `pub(crate)`

- **Goal.** Make reaching around `apply` a compile error where the product ships.
- **Moves.** Visibility only, on the eleven declarations in §1.
- **Deletes.** The public hatches.
- **THE COMPILE BLAST.** `pub(crate)` blocks every caller outside `rs_cam_core`, and that
  includes TEST code in three places. Production: **48** viz, **9** CLI. Test, raw `rg -c`:
  **63** in `crates/rs_cam_viz/tests/`, **32** in viz `src` test modules, **22** in
  `crates/rs_cam_core/tests/` (eleven-hatch count; the seven-hatch figures were 33/26/21)
  — an integration test is an external crate, so it breaks too.
  One of those 22 is `mutation_paths_invalidate_alike_p0.rs` itself, and
  `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs:149` calls `tools_mut()`.
- **THE FIXTURE DOOR IS TWO DOORS, and the feature trick is not enough.** (a) *Construction*
  goes through a `pub ProjectSessionBuilder` in core that exposes no method taking `&mut
  ProjectSession`. (b) *Live mutation* cannot use a builder —
  `crates/rs_cam_viz/src/controller/tests.rs` alone holds **12** such lines (`:256, 1508,
  2224, 2384, 2467, 2528, 2643, 2816, 3006, 3503, 3940`, and `:4046` `stock_mut().x =
  321.0`). Those move to `apply`: the larger option, and unavoidable. (c) *A bare
  `#[cfg(feature = "test-fixtures")]` door fails the gate.* The workspace is `resolver =
  "2"` (`Cargo.toml:2`) and `rs_cam_viz` depends on core normally
  (`crates/rs_cam_viz/Cargo.toml:14`). A release build does not enable the feature, so the
  guarantee holds where the product ships; but under resolver 2 the feature unifies into
  viz's dev targets, so `cargo test -p rs_cam_viz` and `cargo clippy --workspace
  --all-targets` — the commanded gate — compile viz's PRODUCTION modules with the door open
  and report nothing. The §7 grep, not the compiler, is the gate-visible check.
- **Sentry.** The §7 WP7 grep, run as a test, plus a second grep-only sentry that counts
  `pub fn *_mut` on `ProjectSession` and fails when a twelfth appears.
- **P0 flip.** None, if construction goes through the builder. The live-mutation rewrites
  touch test bodies, not assertions.
- **Rollback.** One-line visibility revert per hatch — but only if WP6b and the fixture
  rewrites have already landed, which is why WP7 must be last. **Size.** L. **BLOCKED
  (Q5).**

### WP8 — N14 and N6: undo, optimizer, drill picks

- **Goal.** One invalidation answer for equivalent edits, WITHOUT breaking the optimizer's
  documented dependence on narrowness.
- **Moves.** `apply_toolpath_param_snapshot` (`mutation.rs:1234-1251`, narrow: `drop_result`
  plus `simulation = None`) goes `pub(crate)`. The four viz callers
  (`controller/events/undo.rs:145`, `events/mod.rs:619, 766, 1263`) route through a WIDE
  command. The two core optimizer callers (`tool_load/optimize/candidate.rs:416`,
  `optimize/context.rs:275`) KEEP the narrow path: `candidate.rs:416-432` regenerates only
  the candidate index and sims against cached neighbours. `set_drill_selected_holes`
  (`mutation.rs:922`) joins its wide sibling `set_alignment_pin_drill_holes` (`:891`).
- **DEPARTURE from `RULING:125-126`**, which requires `apply(Command) -> Effects` to be "the
  only write path". The two core optimizer callers stay on a `pub(crate)` non-`apply` write
  path by design. The exception is bounded: both live inside core, both are named here, and
  `pub(crate)` keeps any surface off it.
- **Deletes.** `apply_toolpath_param_snapshot` as a public door.
- **Sentry / P0 flip.** `mutation_paths_invalidate_alike_p0.rs:382-405`
  (`n14_undo_and_optimizer_apply_invalidate_one_index_today`): the `assert_eq!` on
  `set(&[0])` becomes `set(&[0, 1])` and the `assert_ne!` at `:396` becomes `assert_eq!`.
  `:410-425` (`n6_set_drill_selected_holes_invalidates_one_index_today`) flips `set(&[0])` →
  `set(&[0, 1])`. `:431-445`, the pin-hole sibling, stays green unchanged; it is the target
  answer.
- **Rollback.** Two visibility changes plus four call sites. **Size.** M. **BLOCKED (Q2).**

### WP9 — `Query` first row: cycle time

- **Goal.** Prove the `Query` kind on a read that five surfaces already share.
- **Moves.** `readiness::toolpath_cycle_time` (`crates/rs_cam_viz/src/ui/readiness.rs:485`)
  has five callers (`io/setup_sheet.rs:324`, `ui/sim_timeline.rs:2030`,
  `ui/sim_diagnostics.rs:1208`, `ui/toolpath_panel.rs:295`, `ui/readiness.rs:552`). Its core
  embryo is `simulation_cut::publish_cycle_times`
  (`crates/rs_cam_core/src/simulation_cut.rs:863`), the N2 single publisher. The decision
  moves to core as `Query::ToolpathCycleTime { id }`; viz renders.
- **Deletes.** The viz-side decision in `readiness.rs:485-536`.
- **Sentry.** `crates/rs_cam_viz/tests/cycle_time_query_one_answer.rs`: all five surfaces
  read the same `Query` answer on one session.
- **P0 flip.** `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` —
  `the_published_runtimes_agree_after_the_retime` (`:469`) and
  `the_drills_runtime_is_a_real_quantity` (`:487`) stay green byte-for-byte.
- **Rollback.** The five callers keep the viz function until green. **Size.** S.

### WP10 — `Job` first row: generate one toolpath

- **Goal.** Prove the three-step `Job` shape on the existing worker lane.
- **Moves.** `submit_toolpath_compute`
  (`crates/rs_cam_viz/src/controller/events/compute.rs:181`) becomes step (i),
  `apply(Job::Generate { id }) -> Effects::Submit(JobSpec)` — a cheap capture, safe on the
  frame loop. `ComputeLane::Toolpath` (`crates/rs_cam_viz/src/compute/mod.rs:14-16`) runs
  step (ii). Step (iii) is WP3's `AdoptResult` at `compute.rs:973`. The CLI runs all three
  inline. Owns the hatch site `app/mcp.rs:5480` (`mcp_generate_toolpath`). The two off-loop
  escape hatches (`mcp_server.rs` `cancel_generation`, `generation_status`) stay OUTSIDE the
  registry, per the ruling.
- **Deletes.** Nothing yet — `ComputeRequest` survives until WP12.
- **Sentry.** `crates/rs_cam_viz/tests/job_steps_hold_no_session_borrow.rs`. **This test
  adds nothing the signature does not already carry** — a step (ii) declared `fn
  execute(&ResolvedGenInputs, &AtomicBool)` cannot hold a session borrow, and that is the
  real proof. The test exists to keep the signature from widening back, and it can only be
  written once WP11a has published the type.
- **P0 flip.** None. `gen_parity_p0_tests.rs` stays at its current answers.
- **Rollback.** The lane is unchanged; only the submit call is wrapped. **Size.** M.

### WP11a — Publish `ResolvedGenInputs` with private fields

- **Goal.** Make a second generation-input assembly unnameable.
- **Moves.** `struct ResolvedGenInputs` (`compute.rs:192`) becomes `pub`, its 17 fields stay
  private, and `resolve_generation_inputs` (`:1090`) becomes its only public producer.
- **The three constraints that make it real** (§1): no `Default` derive, no `pub` field and
  no `pub` setter, exactly one `pub fn` returning it.
- **Deletes.** Nothing. This WP is additive by design, so WP10 can land on it.
- **Sentry.** `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs` — a source
  scan asserting `rg "\-> ResolvedGenInputs" crates/rs_cam_core/src` → 1 and no `pub` field
  in the struct body, in the `overlays_registry.rs` scan idiom.
- **P0 flip / Rollback.** None; revert the `pub`. **Size.** S.

### WP11b — Narrow the executor; close N12

- **Goal.** One resolver answers for both doors.
- **Moves.** `execute_operation_annotated_with_regions` (`execute.rs:3470-3505`) narrows
  from 21 arguments to `(&ResolvedGenInputs, &AtomicBool, ..)`. The seven N12 divergences
  (`STATUS.md:231`) resolve INSIDE that one resolver: `face_selection`
  (`controller/events/compute.rs:354-372, 536-542`), `BoundarySource::FaceSelection`
  (`worker/execute/mod.rs:770-775` vs `compute.rs:2084-2115`), feed-optimisation stock
  (`worker/helpers.rs:41-52` vs `compute.rs:1712`), the dressup stock-top frame, the
  entry-probe index, the four `HeightContext` builders, and the open-coded single-polygon
  clip. `ResolvedHeights` collapses to one type.
- **Deletes.** `#[allow(clippy::too_many_arguments)]` at `execute.rs:3469`; the second
  `ResolvedHeights` (`from_static_checks.rs:58`).
- **Sentry / P0 flip.** `gen_parity_p0_tests.rs:422`
  (`with_the_shipped_default_the_gui_door_modulates_feeds_and_the_session_door_does_not`)
  becomes an IDENTITY assertion — one resolver, one feed-opt decision. `:402`
  (`the_two_doors_generate_one_geometry_with_feed_optimization_off`) stays green and becomes
  redundant, which is the evidence the flip is real.
- **Rollback.** Keep the 21-argument function as a delegating wrapper for one WP, then
  delete it. **Size.** L. Split by N12 item if one owner cannot hold it; item 1
  (`face_selection`) and item 2 (the face boundary) are the pair that must land together.

### WP12 — Delete `ComputeRequest`'s mirrored fields

- **Goal.** Programme rule 3: the migration is not done while the copy stands.
- **Moves.** `ComputeRequest` (`worker.rs:49-134`) has **27** public fields. **13 mirror
  `ResolvedGenInputs` by meaning, 11 by exact name** — `boundary` answers `boundary_config`,
  `stock_bbox` answers `emission_stock_bbox`: `tool`, `mesh`, `polygons`, `drill_targets`,
  `keep_out_footprints`, `boundary`, `heights`, `cutting_levels`, `prev_tool_radius`,
  `reference_tool_cfg`, `operation`, `setup_transform`, `stock_bbox`. **`face_selection` is
  a FOURTEENTH** — WP11b resolves it inside the resolver (N12 item 1). The 13 that genuinely
  remain viz's are `toolpath_id`, `toolpath_index`, `toolpath_name`, `debug_options`,
  `dressups`, `stock_source`, `safe_z`, `enriched_mesh`, `material`, `prior_stock`,
  `derived_rest_regions`, `rest_analysis`, `link_kinematics`; audit each against the
  resolver before keeping it.
- **Deletes.** The 14 fields, the viz-side resolution that fills them, and the **14**
  self-describing "mirrors `session/compute.rs`" comments in
  `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` and `worker.rs`.
- **Sentry.** §7's grep `rg "session/compute.rs" crates/rs_cam_viz/src` → 0.
- **P0 flip.** None beyond WP11b's.
- **Rollback.** Not cheap — this IS the deletion. Land it only after WP11b's sentry has been
  green through one full heavy gate. **Size.** L.

### WP13 — `UiCommand` split and the cross-surface sentry

- **Goal.** Classify what is viewport-only, and turn `PLAN.md:495-504`'s seven prose
  properties into tests. **Its classification table is WP4's input, so the table precedes
  WP4.**
- **Moves.** `McpRequestKind`'s non-core variants become `UiCommand`. Measured: scrubbing
  **6** (`mcp_bridge.rs:788-804`), screenshots **3** of 4 (`:805-845`; `ReachMap` is a core
  measurement, a `Query`), UI navigation **1** of 4 (`:846-873`; only `SetUiView`). **10
  `UiCommand` rows, not 14** — a CORRECTION to `RULING:66-69`, whose 14 reads the section
  comments. `AppEvent` (`ui/mod.rs:57-441`, **133** variants) splits the same way; **the
  ruling's ~89 / ~44 is a reading, not a ruling**, its own words. The FIRST deliverable is
  the per-variant classification table, reviewed before any variant moves.
- **Deletes.** The unemitted `AppEvent::RemoveSetup` handler
  (`viz/controller/events/mod.rs:94`, zero emitters, 4 test sites) — give it an emitter or
  delete it; a handled-but-unemitted variant is the defect the ruling opens with.
- **Sentry.** `crates/rs_cam_viz/tests/command_surface_completeness.rs`: every `CommandId`
  with `gui: Reached` resolves an `AppEvent::Core` path; every `Skip` carries a reason; no
  row classified `UiCommand` writes `ProjectSession`.
- **P0 flip.** None. **Rollback.** Classification is a document; the moves are per variant.
  **Size.** L.

---

## §5 What each package does NOT guarantee

- **WP1.** No containment. All eleven hatches stay `pub`. N15 closes at ONE site.
- **WP2a.** It pins names and schemas, not behaviour. A tool that keeps both and changes its
  meaning passes.
- **WP3.** `Effects` is produced; nothing yet forces a caller to read it.
- **WP4.** The MCP door agrees with core; the GUI door still does not, and three holdout
  sites keep a hand-written stale set.
- **WP5.** One command replaces the write-back; the panel still rebuilds a whole
  `ToolpathConfig` each frame, so a field added to the struct and not to the entry is still
  silently dropped.
- **WP6.** Responsiveness is measured on one project. Invalidation is one frame late on a
  drag, by design.
- **WP6b.** It moves callers; it does not restrict the doors. Containment arrives only with
  WP7.
- **WP7.** **No partial commit is possible** — the workspace does not compile between the
  first `pub(crate)` and the last migrated caller, so WP7 is one commit and it requires WP5,
  WP6 and WP6b already landed. The guarantee is a compile error in the RELEASE build only;
  the commanded gate sees the §7 grep, not the compiler (WP7(c)).
- **WP8.** The optimizer keeps the narrow path by design, so "one answer for equivalent
  edits" holds for operator edits, not for candidate application.
- **WP9.** One `Query` row proves the kind; the other read rows are untouched.
- **WP10.** The `Job` shape is proved on one lane; simulate, optimize and reach keep their
  own submits.
- **WP11a.** Private fields stop a SECOND assembly. They do not prove the ONE assembly is
  correct.
- **WP11b.** Each N12 item needs its own assertion; the parity test covers item 3 alone
  today.
- **WP12.** Deleting the mirror removes the drift risk; it does not re-verify the 13
  remaining viz-owned fields.
- **WP13.** The sentry proves declaration and reach, never that a handler is right.

---

## §6 Risk register

| Risk | Trigger | Detection | Mitigation | Rollback |
|---|---|---|---|---|
| **egui responsiveness or same-frame reader regression** | WP6's scratch copy | the three readers named in WP6; frame-time measurement | label and undo comparison read the scratch; emit on release | per widget |
| **MCP wire rename or schema drift** | WP4 | WP2a's `list_tools` snapshot fails | wire names are row literals | revert the row literal |
| **A fourth resolver appears** | WP11a/WP11b | `rg "\-> ResolvedGenInputs" crates/rs_cam_core/src` → 1; no `pub` field | publish BEFORE deleting `ComputeRequest`'s fields | the delegating 21-argument wrapper |
| **`AdoptResult` over-rejects** | the stamp reads `next_revision` | WP3's sentry arm (b): an edit to a DIFFERENT toolpath must not reject | stamp `toolpath_revision(index)` | one field |
| **The gate never exercises WP7's guarantee** | resolver-2 feature unification into viz's dev targets | `cargo build -p rs_cam_viz` (release path) is the only compile-time check | §7's grep is the gate-visible sentry | — |
| **WP7 lands with callers unmoved** | WP7 before WP6b | the workspace does not compile | WP7 is one commit and requires WP5+WP6+WP6b | revert the single commit |
| **The other account resumes `/techdebt-orchestrate` mid-package** | two lanes edit `session/compute.rs` | STATUS.md rows show `IN PROGRESS (who)`; foreign hunks in `git diff` | claim the row before the first edit; one owner for `session/compute.rs` (`PLAN.md:537`) | the row states the last DONE hash |
| **f036b red-by-design read as a regression** | any full gate run | always `modulation_raises_cutting_chipload_toward_band` at **0.0214** vs **[0.0320, 0.0550]** | quote the number, not "1 failed" | none needed |
| **N12 items 1-2 need a STEP fixture that does not exist in-memory** | WP11b | no mesh-backed `face_selection` fixture in `crates/rs_cam_core/tests`; the heavy gate does not enable `step` | build an `EnrichedMesh` fixture with synthetic `FaceGroupId`s | items 1-2 stay N12-open |
| **`rs_cam_mcp` dependency direction** | a `Command` payload names an MCP `*Param` struct | it would not compile without adding `rmcp` to core | core owns the payload; `*Param` stays a wire adapter | one `From` impl per row |

---

## §7 Definition of done

`PLAN.md`'s architectural gate (`:495-504`) made concrete, for the phases §4 reaches (0, 1A,
1B, 1C's first row, 3).

**Sentries that must exist and be green.** In `crates/rs_cam_core/tests/`:
`command_registry_completeness.rs`, `resolved_gen_inputs_has_one_producer.rs`. In
`crates/rs_cam_viz/tests/`: `command_registry_surfaces.rs`, `mcp_wire_surface_pin.rs`,
`adopt_result_rejects_stale_completion.rs`, `inspector_door_is_one_command.rs`,
`inspector_emits_one_command_per_change.rs`, `cycle_time_query_one_answer.rs`,
`job_steps_hold_no_session_borrow.rs`, `command_surface_completeness.rs`.

**The eight Phase 0 tests, and their state per WP:**

| Phase 0 test | State |
|---|---|
| `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` | flips at WP1 (N15 arm), WP5 (fifth arm added), WP8 (N14 + N6 arms) |
| `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs` | green UNCHANGED through WP1 — the refusals must not reach `Effects` |
| `crates/rs_cam_core/tests/disconnected_finish_retract_structure_p0.rs` | green UNCHANGED throughout |
| `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` | green UNCHANGED; WP9's target contract |
| `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` | green UNCHANGED; coolant arm is an equality since P0-D1 |
| `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` | green UNCHANGED; WP6b touches its funnel site (`events/mod.rs:938`) |
| `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs` | green UNCHANGED throughout |
| `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs` | green through WP1-WP10; Test 2 flips to identity at WP11b |

**Greps that must return zero:**

| After | Command | Expect |
|---|---|---|
| WP7 | `rg "\.(stock_mut\|machine_mut\|tools_mut\|models_mut\|post_mut\|wizard_mut\|setups_mut\|find_setup_by_id_mut\|find_toolpath_config_by_id_mut\|toolpath_configs_mut\|insert_result)\(" crates/rs_cam_viz/src crates/rs_cam_cli/src` | 0 |
| WP7 | `rg -c "^\s*pub fn [a-z_0-9]*_mut" crates/rs_cam_core/src/session/mod.rs` | 0 |
| WP4 | `rg "MutationKind" crates/rs_cam_viz/src` | 0 |
| WP4 | `rg "compute_stale_set" crates/` | 0 (only after all sixteen producers read `Effects`) |
| WP11a | `rg "\-> ResolvedGenInputs" crates/rs_cam_core/src` | 1 |
| WP11b | `rg "too_many_arguments" crates/rs_cam_core/src/compute/execute.rs` | 0 |
| WP11b | `rg "pub struct ResolvedHeights" crates/rs_cam_core/src` | 1 |
| WP12 | `rg "session/compute.rs" crates/rs_cam_viz/src` | 0 |
| WP13 | `rg "AppEvent::RemoveSetup" crates/rs_cam_viz/src` | 0, or ≥1 emitter |

**Full heavy gate.** `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`
read **3801 passed, 1 failed, 288 ignored** when this plan was commissioned; STATUS's last
recorded figure is 3793/1/288 (`STATUS.md:149`) and the verifier's concurrent sentries
account for the delta. The passed count moves UP as sentries land and must never move down.
The 1 failure is always
`adaptive_feed_modulation_pipeline_f036b::modulation_raises_cutting_chipload_toward_band` at
**0.0214** against **[0.0320, 0.0550]**, red by design. Beside it: `cargo test -p rs_cam_viz
-q`, `-p rs_cam_cli -q`, `-p rs_cam_mcp -q`, `cargo clippy --workspace --all-targets
--features rs_cam_core/heavy-tests -- -D warnings`, `cargo fmt --all -- --check`. Add `cargo
build -p rs_cam_viz` at WP7: it is the only build that proves the hatch containment
(WP7(c)).

---

## §8 Tracker protocol during implementation

**STATUS.md rows.** Add one table under a new heading `Command surface — work packages`, one
row per WP: `WP | subject | state | evidence`. States are `TODO` / `IN PROGRESS (who)` /
`BLOCKED (on what)` / `DONE (hash)`. WP5 starts `BLOCKED (Q1)`, WP8 `BLOCKED (Q2)`, **WP7
`BLOCKED (Q5)`**. Never delete a row; mark it `DROPPED (why)`. The orchestrator updates
STATUS.md only; `PLAN.md`, `AUDIT.md` and `RULING_...DRAFT.md` stay verbatim. Slice a row
edit to the NEXT row start, never to a far anchor, and assert the row count after.

**The lane model.** Read-only scouts measure and report; they run no cargo. Writers work in
worktrees and run no cargo; one writer per shared file, and `session/compute.rs` has ONE
designated owner (`PLAN.md:537`). One verifier holds cargo and works red-first in two
commits: commit the sentry alone, then commit the fix with the sentry's RED output verbatim
in its body.

**Standing rules.** This laptop has no swap, so run ONE cargo job at a time, set
`CARGO_BUILD_JOBS=2` and `--test-threads=2` for the full gate, and run that gate at a phase
end, never per package. Never `git reset --hard` in this tree — it drops the foreign
`.mcp.json` edit (`STATUS.md:159-161`); leave that file modified. Write all prose in
Simplified Technical English. A commit that lands a behaviour change states the intentional
output change separately from the refactor parity result (`PLAN.md:516`).

---

## §9 Open questions for the operator

Resolved here, with the default stated — no answer needed:

- **`set_toolpath_param` survives as a thin wrapper through WP1 and is deleted in WP4.**
  Deleting it in WP1 mixes a rename across three CLI sites, two P0 test files and five core
  unit tests into the patch that must prove the pattern.
- **`Effects.stale` is the set `drop_result` was called on.** §1 states why.
- **`Effects.revision` is `toolpath_revision(index)`.** §1 states why.

**Q5 blocks WP7 and nothing else blocks WP1.** Q1 gates only WP5, Q2 only WP8, Q3 and Q4
gate nothing. Work can start on WP1 today.

**Q5 — which test-fixture door?** WP7 makes eleven hatches `pub(crate)` and that breaks
about 117 test sites across three crates (63 in `crates/rs_cam_viz/tests/`, 32 in viz
`src` test modules, 22 in `crates/rs_cam_core/tests/`, eleven-hatch count). Two doors
are needed, not one: a `pub
ProjectSessionBuilder` for setup-time construction, and `apply` for the sites that mutate a
LIVE session mid-test — `crates/rs_cam_viz/src/controller/tests.rs` alone holds 12 of the
second kind (for example `:4046`, `stock_mut().x = 321.0`). A bare `#[cfg(feature =
"test-fixtures")]` door is NOT sufficient on its own: under `resolver = "2"`
(`Cargo.toml:2`) the feature unifies into viz's dev targets, so the commanded gate would
compile viz's production modules with the door open and report nothing. **Recommend: build
both doors, and accept that the second class is a real rewrite.** **Default if unanswered:
WP7 stays BLOCKED.**

**Q1 — Does the GUI inspector become ONE `ReplaceToolpathConfig` command or sixteen
named-param commands?** I read the sixteen fields at
`crates/rs_cam_viz/src/ui/properties/mod.rs:3759-3782`. **Recommend ONE.** `enabled` is
written but not edited by the panel (its own comment at `:3786-3790` says the card's row
control owns it); `feeds_provenance` is derived, not typed (`:3745-3758`); `name`,
`coolant`, `pre_gcode` and `post_gcode` sit deliberately outside
`generation_inputs_signature`. Sixteen commands would ask the signature question sixteen
times for one answer. **Default if unanswered: one command.**

**Q2 — Does the GUI undo route through a WIDE command while the optimizer keeps the narrow
path?** `apply_toolpath_param_snapshot` (`mutation.rs:1234`) has **SIX** production callers,
which I classified by hand: four in viz (`undo.rs:145`, `events/mod.rs:619, 766, 1263`) and
two in core's optimizer (`tool_load/optimize/candidate.rs:416`, `context.rs:275`). Three
further hits are test code — `context.rs:415, 465` sit after that file's `#[cfg(test)]` at
`:335`, and `retarget_reconciliation_a8.rs:238` lives in a whole `#[cfg(test)] mod`
(`optimize/mod.rs:58-59`). `candidate.rs:416-432` regenerates ONLY the candidate index and
sims against cached neighbours, so widening it would re-generate the whole chain per
candidate. **Recommend: the four viz callers go wide, the two core optimizer callers stay
narrow behind `pub(crate)`.** That is the constraint `STATUS.md:245` tells the plan to
carry. **Default if unanswered: the split above.**

**Q3 — Does the MCP server stay inside the GUI process?** Resolved by the plan's own
evidence: the 10 `UiCommand` tools need the viewport, and `cancel_generation` /
`generation_status` read LIVE lane state off the frame loop
(`crates/rs_cam_viz/src/compute/mod.rs` `LaneSnapshot`), which no out-of-process server can
reach. **Default: in-process.** Answer only to re-examine the split.

**Q4 — Does a `debug_enabled` write keep dropping the result chain?** `set_toolpath_param`'s
`debug_enabled` arm (`compute.rs:474-485`) writes `tc.debug_options.enabled` and still
reaches `invalidate_result_chain` at `:623`, while the debug options are deliberately ABSENT
from `generation_inputs_signature` (`properties/mod.rs:3698-3700`). So MCP and the CLI drop
the chain on a debug toggle and the GUI does not. It is one arm inside WP1's own function.
**Recommend: the GUI answer is right — a debug toggle changes no motion — so the named arm
should skip the invalidation.** That is a behaviour change and needs your word. **Default if
unanswered: preserve today's divergence and record it.**

---

## §10 Changelog — review round 1

Review at `/tmp/.../scratchpad/plan_review.md`, verdict REJECT. I re-read every `path:line`
the review cites. Line numbers below are the revised file's.

**Departures and corrections, recounted.** The review measured one declared departure. The
revised file declares **THREE departures** from the ruling — the `Surfaces` struct (§1,
replaces `RULING:86-88`), the `AppEvent::Core` / `McpRequestKind::Core` wrappers (§1,
replaces `RULING:99-100`), and WP8's `pub(crate)` optimizer write path (replaces
`RULING:125-126`'s "only write path") — and **FOUR corrections** to the ruling's readings:
eleven hatches not six (§1, `RULING:137-140`), ten `UiCommand` rows not fourteen (WP13,
`RULING:66-69`), `insert_result` closing at WP3 not with `Job` (§3, `RULING:143-145`), and
the `pub(crate)` blast reaching CLI production plus ~117 test sites (WP7, `RULING:140`).

### Blocking — all ten applied; one arithmetic disagreement

| id | Verified at | Change |
|---|---|---|
| B1 | `rg -n "^\s*pub fn [a-z_0-9]*_mut" crates/rs_cam_core/src/session/*.rs` → ten | §1's table is eleven rows, 48 viz / 9 CLI; WP7 moves eleven; §7's grep names all eleven and adds a `pub fn *_mut` count sentry |
| B2 | 48 viz sites, only 16 owned | new **WP6b** owns the 27 non-egui viz and 9 CLI sites, each by `path:line`, with the arithmetic shown; WP7 requires it; §5 says no partial WP7 commit |
| B3 | `mcp_bridge.rs:846-873` holds the four; `app/mcp.rs:2372,2457` write `machine_mut`; `machine_library.rs` is core | the two writers become `Command` rows in WP4; `ListMachineLibrary` and `ReachMap` become `Query`. **DISAGREEMENT: the residue is 10, not 11** — 14 − 4 = 10 (scrub 6 + screenshots 3 + UI nav 1). §2 and WP13 carry the derivation |
| B4 | `compute.rs:192` reads `struct ResolvedGenInputs`, no `pub` | WP11 splits into **WP11a** (publish) and **WP11b** (narrow); §3 orders WP11a → WP10 → WP11b → WP12 |
| B5 | WP6's twelve sites are `tools_mut` / `stock_mut` / `machine_mut` / `find_setup_by_id_mut`; the toolpath door is WP5's | §3 sets WP6's requires to `WP1, WP4`, deletes the WP6 ∥ WP4 claim, and the chain is `WP1 → WP3 → WP4 → {WP6, WP6b} → WP7` |
| B6 | `set_toolpath_enabled` (`mutation.rs:327-343`) calls `invalidate_output_dependents`; `drop_result` (`:1328`) is the only bump site; `app/mcp.rs:5262` is a hardcoded `vec![index]` | §1 defines `stale` as the `drop_result` set and says where it differs from `dirty`; WP3 returns `(dirty, dropped)`; `:5262` is WP4's sixteenth producer |
| B7 | `next_revision` (`mod.rs:1299`) is session-global, "Never reset"; `toolpath_revision(index)` (`:1451`) is per-index | §1 and WP3 use `toolpath_revision(index)`; WP3's sentry gains arm (b) — an edit to a DIFFERENT toolpath must not reject |
| B8 | `app/mcp.rs:3648` is inside `add_toolpath_via_gui` | WP4 names the three holdouts, sends `compute_stale_set` to `pub(crate)`, and defers its deletion until all sixteen producers read an `Effects`; §7's grep row carries the condition |
| B9 | `Cargo.toml:2` is `resolver = "2"`; `crates/rs_cam_viz/Cargo.toml:14` is a normal dep; `controller/tests.rs` holds 12 live-mutation sites | WP7's fixture door splits into a `ProjectSessionBuilder` and `apply`; WP7(c) records that the commanded gate cannot see the guarantee, so `cargo build -p rs_cam_viz` joins §7 |
| B10 | — | §9 opens with **Q5**; §3 and §8 mark WP7 `BLOCKED (Q5)` |

### Non-blocking

1. Payload column restored to §1's sketch. 2. WP2a's sentry is two-stage: the snapshot
alone, then the `CommandId` arm at WP1. 3. APPLIED — WP2a now snapshots the router's own
`list_tools` name plus `inputSchema` instead of source literals. 4. APPLIED —
`face_selection` is a fourteenth mirrored field in WP12, not "genuinely viz's". 5. APPLIED —
WP5 keeps arm 3 on the raw core door and adds a FIFTH arm, so `:336` keeps an independent
comparison. 6. APPLIED — WP6 states `:1421` already carries the pattern. 7. APPLIED — WP6
names the label at `:1446-1455` and the stock undo at `:284-295` as the two same-frame
readers, records that the drag cost exists today at `:1437-1443`, and states that
invalidation is one frame late. 8. APPLIED — `properties/mod.rs:374, 408, 418` join WP6;
nine becomes twelve. 9. APPLIED — §7 lists all eight Phase 0 tests by path with their state
per WP. 10. APPLIED — WP8's exception is now a labelled DEPARTURE. 11. APPLIED — §2 says
phases 2, 4A-4B, 5, 6, 7 and 8 get packages after WP13, and §7 is scoped to the phases §4
reaches. 12. APPLIED — `session/mod.rs:23`, `mutation.rs:1204-1224`. 13. APPLIED — §9 states
that only Q5 blocks a WP that is otherwise ready, and that WP1 can start today. 14. APPLIED
— WP3 carries the 33 `Ok(()) =>` sites by file, requires them in the same commit, and is
size L with a rollback that is not one call site.

### Compile-time claims, reclassified

- §1's registry: "the compiler guarantees" is now limited to the missing `Surfaces` field
  and the exhaustive `CommandId` match. The uniqueness and reason rules are named test-time.
- WP7: the guarantee is now stated as a compile error in the RELEASE build, with the
  gate-visible check being the §7 grep.
- WP10: the sentry no longer claims to be the proof. The signature is the proof; the test
  keeps the signature from widening.
- §1's `ResolvedGenInputs`: the claim is now conditional on three stated constraints, with
  WP11a asserting them and a grep.

### Opinions

Adopted: WP13's table precedes WP4 (§3 edge 4, WP13 goal); WP2a as a router snapshot; the
`pub fn *_mut` count sentry (§7); WP11b's "split by N12 item" note. Not adopted:
`SetToolpathEnabled` as row 2. B6 is now settled in §1 by reading, so row 2 need not surface
it, and the ruling names `SetToolpathParam` as the first step.

## §11 Changelog — review round 2 (confirmation, 2026-09-11)

Verdict: ACCEPT WITH CHANGES. All ten round-1 blockers confirmed CLOSED against
the live code at `80219e69`. The B3 dispute resolved in the plan's favour: ten
residual `UiCommand` rows. Two accuracy defects found and applied here:

- §1 `Effects.stale`: `drop_result` is not the only revision bump site.
  `bump_all_revisions` (`mutation.rs:1338-1342`) moves every index on removal and
  bulk replace. `stale` is now defined as the revision-moved set, measured by
  comparing `toolpath_revision` before and after the mutation.
- §4 WP7, §9 Q5 and §10 B9: the test-side blast counts were the seven-hatch figures
  (33/26/21, about 80). The eleven-hatch figures are 63/32/22, about 117.

Internal consistency checks (a)-(e) passed: no dangling `Requires:` name, the §7
grep names exactly the eleven hatches, §2/§4/§10 counts agree, the four spot-checked
dependency edges match, no duplicated heading, no joined bullet list.


---

## §12 WP3 pre-implementation corrections and rulings (2026-09-11)

A read-only scout measured §4 WP3 against the WP1 tree (`6179cc2c`). The measured
numbers replace the §4 prose where they differ. The rulings below bind the WP3 writers.
Full inventory: session scratchpad `wp3_brief.md`.

### Corrections

- **Producers: 37 `pub fn`, not "every fn that ends in `invalidate_result_chain` /
  `drop_result`".** 23 take a toolpath index, 6 take a SETUP index, 8 are bulk (stock,
  tool, model, pins, bulk replace). Four more clear the simulation and move no revision
  (`add_toolpath`, `invalidate_machine`, `set_post_config`, `replace_tools`). The §4 grep
  misses `drop_all_results` (`mutation.rs:1354`), `drop_setup_results` (`:1364`) and
  `bump_all_revisions` (`:1339`); `invalidate_all` does not exist.
- **`Ok(()) =>` census is 32, not 33** (`app/mcp.rs` 22). Only **10** sit on a converted
  function, all in `app/mcp.rs` (`:3745, 3804, 3882, 4093, 4643, 4763, 5193, 5230, 5263,
  5299`). Five return-value binders break independently: `invalidate_model` at
  `controller/io.rs:124, 175, 272`, `invalidate_tool` at `events/undo.rs:115` and
  `ui/properties/mod.rs:148` (all bind `Vec<usize>`), and
  `auto_enable_rest_analysis_for_source` at `ui/properties/mod.rs:826` (a `bool` inside a
  let-chain).
- **The contract tests are at `mutation_paths_invalidate_alike_p0.rs:343` and `:369`**,
  not `:336-360` / `:362-379`.
- **`crates/rs_cam_cli/src` holds zero producer call sites.** WP3's caller side is viz only.
- **`insert_result` has one production caller and 21 test call sites in 13 files.**
- **The stale-completion refusal already exists viz-side** at
  `controller/events/compute.rs:922-935`; the submitted revision lives only on
  `gui.toolpath_rt[id].submitted_revision: Option<u64>`, and its unstamped arm accepts.
- **`drain_compute_results` is `pub(crate)`**, so the §4 sentry file under
  `crates/rs_cam_viz/tests/` cannot call it.
- **WP1 shipped `simulation_cleared: simulation_before && self.simulation.is_none()`**, not
  `take().is_some()`. Same answer today; WP3 keeps the WP1 form inside the combinator.

### Rulings

1. **`Effects.revision` becomes `Option<u64>`.** `0` is a live initial revision, so it
   cannot mean "no index". `Some` only when the command names ONE toolpath that still exists
   at the SAME index after the command. `reorder_toolpath`, `move_toolpath_to_setup`,
   `remove_toolpath`, every setup-index producer and every bulk producer report `None`.
   `None` means NOT MEASURED. This edits WP1's struct, `command_registry_completeness.rs`
   and the N15 arm; it is the first hunk of WP3's fix commit.
2. **"One producer" means one CONSTRUCTION site.** `command.rs` gains one `pub(crate)`
   combinator that snapshots every revision and the simulation, runs a closure, and builds
   `Effects` from the diff. Every one of the 37 producers wraps its body in it and returns
   `Result<Effects, SessionError>` (or `Effects` where it returned `()`; `Option<Effects>`
   where it returned a "did anything change" `bool`, `None` = nothing changed). No registry
   rows are added in WP3; the stock, machine, tool and setup rows stay WP4's.
3. **`Effects` carries `#[must_use]`.** About nine bare-statement viz sites then need
   `let _ =`; the writer lists them.
4. **`AdoptResult { index, revision: u64, result }` requires the revision.** `apply` refuses
   with a typed error when `toolpath_revision(index) != revision` and inserts nothing. The
   drain at `compute.rs:973` reads the `submitted_revision` stamp; when it is `None` it
   passes the CURRENT revision with a comment that this reproduces today's accept-when-
   unstamped arm. The viz gate at `:922-935` is deleted in the same commit. Every test
   caller of `insert_result` passes `session.toolpath_revision(i)` read before delivery.
   Carrying the revision on the request itself is WP10's job.
5. **Sentries.** The refusal sentry lives in CORE:
   `crates/rs_cam_core/tests/adopt_result_rejects_stale_completion.rs`, two arms as §4
   states, no GUI. One in-crate viz test in `controller/tests.rs` (precedent `:1119`) proves
   the drain hands the stamp through. `mutation_paths_invalidate_alike_p0.rs:343` and `:369`
   stay green unchanged.
6. **The four sim-only mutations join WP3** when their return type is `()`; one that returns
   a value keeps its return and is recorded in §5.
7. **Two writers, one worktree, one fix commit.** The core writer lands the sentry commit
   and the fix commit (core side; the workspace does not compile between). The viz writer
   AMENDS that fix commit with the caller side. The verifier sees two commits.

---

## §13 WP9 and WP11a pre-implementation corrections and rulings (2026-09-11)

Scouts measured §4 WP9 and §4 WP11a against master `4a480fc8`. Full briefs: session
scratchpad `wp9_brief.md`, `wp11a_brief.md`.

### WP11a corrections

- The sentry grep `-> ResolvedGenInputs` → 1 is vacuous: the producer returns
  `Result<ResolvedGenInputs, SessionError>` (0 hits today). The sentry counts every `fn` in
  core whose return type mentions the struct, including `-> Self` inside an
  `impl ResolvedGenInputs`, and asserts exactly one, named `resolve_generation_inputs`.
- `session/mod.rs` declares `mod compute;` privately, so the package adds the struct to the
  `pub use` line. Without that viz cannot name it and WP10 stays blocked.
- No private field type blocks publication; all eleven field types are already `pub`.
- Line drift only: resolver at `:1118`, executor at `:3492-3527`.

### WP9 corrections

- `readiness::toolpath_cycle_time` is viz, called from five viz sites and one viz test
  (`cycle_time_basis_g_timeest.rs`). MCP and the CLI never call it. MCP `get_cut_trace`
  reads `toolpath_summaries` only (blind to drills) and `narrate_toolpath` adds
  `feed_time_s + dwell_time_s`, a third quantity. Neither is in WP9's scope.
- The aggregate tier (`project_cycle_time`, `estimate_total_time`) gates on
  `gui.toolpath_rt` and stays in viz.
- The `Deletes` range overshoots: the function ends at `readiness.rs:527`.

### WP9 rulings

1. **One list, one `CommandId` union.** `for_each_command!` keeps a single row list and
   `CommandId::ALL` keeps covering every row, so the completeness sentry's
   `ALL.len() == DECLARED_ROWS` stays true. The callback macro splits rows by the kind
   column (a tt-muncher) into the `Command` payload enum and a new `Query` payload enum.
2. **A sixth row column names the answer type.** `Command` rows write `Effects` there.
   `Query` rows name their answer struct. The callback generates `enum QueryAnswer` with one
   variant per `Query` row, and `ProjectSession::query(&self, Query) -> Result<QueryAnswer,
   SessionError>` (a `&self` door: every read site holds `&AppState`).
3. **Row 1 is `ToolpathCycleTime(ToolpathCycleTimeArgs)`** with `Surfaces { gui: Reached,
   mcp: Skip("get_cut_trace and narrate_toolpath report other quantities; WP4 revisits"),
   cli: Skip("the CLI project report prints the simulation total, not per-toolpath") }`.
   The payload carries what the `CuttingOnly` arm needs (`cutting_distance_mm`,
   `nominal_feed_mm_min`) because neither is on the trace.
4. **The five viz sites call the Query; `readiness::toolpath_cycle_time` is deleted.**
   `CycleTime` stays `Copy`; `CycleTimeBasis::remedy()`'s GUI text stays in viz as a
   viz-side extension of the core answer.
5. **Sentry:** `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs` — the core Query
   and the pre-fix viz function agree on the same fixture for every basis arm (the viz
   function is copied into the test as the pre-fix oracle, then the test keeps the oracle
   as a frozen table). Plus the `command_registry_completeness` count moves to 3 rows.
6. **Order:** WP9 starts after WP3 lands; both edit `command.rs`.
