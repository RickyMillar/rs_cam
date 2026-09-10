# One command surface — ADOPTED 2026-09-11 (operator ruling)

> Written 2026-09-11 from a read-only comparison of `sysml-rs/crates/tooling/sysml-service-macros` against rs_cam at `ce5ccabb`+. Nothing here is implemented. `PLAN.md` stays verbatim; this file is the proposed Phase 0 addition. Read, not run.

## Phase 0 ruling — one command surface (draft, 2026-09-11)

**Status: ADOPTED by the operator on 2026-09-11. Nothing below is implemented yet.**

### What is wrong

Three doors reach the project. The CLI calls `ProjectSession` directly. The MCP
server runs inside the GUI process and mostly calls `state_mut().session`. The
GUI raises an `AppEvent`. Each door declares its own list.

| Surface | Declaration site | Count |
|---|---|---|
| GUI | `pub enum AppEvent`, `viz/src/ui/mod.rs:57-441` | 133 variants |
| MCP wire | `#[tool(...)]` under `#[tool_router]`, `viz/src/mcp_server.rs:400+` | 78 tools |
| MCP bridge | `pub enum McpRequestKind`, `viz/src/mcp_bridge.rs:442-873` | 76 variants |
| MCP schema | `pub struct *Param`, `rs_cam_mcp/src/server.rs:23+` | 53 of 57 pub structs |
| CLI | `enum Commands`, `cli/src/main.rs:38-304` | 7 subcommands |

One MCP capability is declared four times: the param struct, the `#[tool]`
attribute, the `McpRequestKind` variant, the `app/mcp.rs` match arm. Each door
is internally exhaustive — `handle_internal_event` names all 133
(`viz/controller/events/mod.rs:16-454`), `handle_mcp_request` all 76
(`viz/app/mcp.rs:227-975`). The compiler guards reach WITHIN a door. Nothing
guards reach ACROSS doors.

### Six measured divergences

| Capability | GUI | MCP | CLI |
|---|---|---|---|
| Remove setup | handled `events/mod.rs:94`, **emitted by nothing** (0 emit, 4 test sites) | absent | absent |
| Coolant | **no widget** | absent — not a `ParamDef`, so `set_toolpath_param` cannot reach it | job TOML `job.rs:192,350` |
| Setup datum | `properties/setup.rs:106-142` | absent | absent |
| Alignment pins | `properties/stock.rs:174` | `mcp_server.rs:691` | absent |
| `sweep` | absent | absent | `main.rs:125` |
| Machine kinematics | `properties/mod.rs:1513` | `mcp_server.rs:1721` | one fixed preset `project.rs:224` |

Core removes a setup (`core/session/mutation.rs:709`) and no surface reaches it.
The export wizard prints "Coolant is per-toolpath; edit in the toolpath
inspector." (`viz/ui/export_wizard.rs:639`); that control does not exist (P0-D1).
MCP also ships two doors for one GUI action. `add_toolpath_via_gui`
(`viz/app/mcp.rs:3547`) raises `AppEvent::AddToolpath` and inherits the GUI
refusal. `add_toolpath` (`:3935`) bypasses the event path and carries a
`TODO(v1.2)` at `:3988` recording that its model binding diverges.

### Ruling

Adopt a **core-owned command registry** from one `macro_rules!` X-macro list, in
the idiom of `for_each_op!` (`core/compute/catalog.rs:149-181`, 24 rows, two
callback invocations at `:222` and `:256`) and the overlays registry
(`viz/ui/overlays/registry.rs:383`). Declare **four kinds**:

- `Command` — a synchronous validated mutation. It returns `Effects`.
- `Query` — a synchronous read. Core computes; a surface renders.
- `Job` — long work: a handle plus three synchronous steps, never an `async fn`.
  The steps are (i) `apply(Job::Generate { id }) -> Effects::Submit(JobSpec)`, a
  cheap capture that is safe on the frame loop; (ii) `resolve(JobSpec) ->
  ResolvedGenInputs` then `execute(&inputs, &cancel)`, both off the loop and
  neither holding `&mut` session; (iii) `apply(Command::AdoptResult { id,
  revision, result })`. The CLI runs all three inline.
- `UiCommand` — viz only. It never enters the core enum.

The split is not new. `McpRequestKind` already partitions itself by comment:
Reads (instant) 24 (`:443`), Mutations (instant) 34 (`:555`), Compute (async) 4
(`:762`), scrubbing plus screenshots plus UI navigation 14 (`:788,:805,:846`).
This ruling turns those comments into types. 62 of 76 become core kinds; 14 stay
in viz. `AppEvent` splits the same way: about 89 project mutations against about
44 GUI-only (`Select` `:85`, `ResetView` `:90`, `SwitchWorkspace` `:198`, the
six `Sim*` scrub events `:199-205`, `Quit` `:440`). Classify each row before
you move it; the two counts are a reading, not a ruling.

Home: `rs_cam_core::session::command`. **The payload is a plain core type**, not
an MCP struct. `rs_cam_mcp` already depends on core (`server.rs:14`) and derives
`rmcp::schemars::JsonSchema` (`server.rs:11`), so a core enum naming those
structs would invert the dependency and drag rmcp into core. The `*Param`
structs stay where they are and become **wire adapters**: the MCP door converts
one into a `Command`, exactly as the CLI door converts clap arguments. Wire
names are row literals, so programme rule 4 holds.

```rust
for_each_command! {
    // kind    Variant            wire name              payload             surfaces
    (Command,  SetToolpathParam, "set_toolpath_param",  SetToolpathParamArgs, [Gui, Mcp, Cli]),
    (Command,  AddSetup,         "add_setup",           AddSetupArgs,         [Gui, Mcp, CliSkip("job TOML declares setups in `[[setup]]`, job.rs:100")]),
    (Command,  ExportGcode,      "export_gcode",        ExportGcodeArgs,      [Gui, Mcp, Cli]),
}
```

`SetToolpathParamArgs` is `{ index, param, value }` — the shape
`core/session/compute.rs:336` already takes. The wire adapters are
`SetToolpathParamInput` (`rs_cam_mcp/src/server.rs:351`), `AddSetupParam`
(`:23`) and `ExportParam` (`:165`). `ExportGcode` is a `Command` here because
`McpRequestKind::ExportGcode` sits in the Mutations (instant) section today
(`mcp_bridge.rs:586`). Phase 2 may promote it to `Job`.

Callback macros generate per row: the enum variant, `ALL`, `wire_name()`,
`kind()`, the `Surfaces` set, and the `McpRequestKind` variant. **The `#[tool]`
method stays hand-written**, because `#[tool_router]` scans an impl block and
generating that block from a core X-macro is unproven. The sentry guarantees MCP
coverage, not the generator. Also hand-written per surface: the handler body,
the egui widget, the tool description prose, CLI argument parsing, and every
composition (`generate_all`'s fixpoint loop, the toast read).

### The stronger guarantee — make the wrong thing unconstructible

The registry fixes **reach**. It does not fix what reached the cutter.

**Resolution (N12).** `execute_operation_annotated_with_regions` takes **21
loose arguments** under `#[allow(clippy::too_many_arguments)]`
(`core/compute/execute.rs:3469-3505`). Its correct producer
`resolve_generation_inputs` is **private** (`core/session/compute.rs:1090`) and
`ResolvedGenInputs` is a private struct (`:192`). Privacy did not prevent the
parallel copy. It caused it. Viz could not call the resolver, so viz built
`ComputeRequest` (`viz/compute/worker.rs:40-110`), whose own comments say it
mirrors the core type. Publish `ResolvedGenInputs` with **private fields**.
Publish one resolver as its only producer. Narrow the executor to
`&ResolvedGenInputs` alone. Viz then cannot assemble a different answer, because
it cannot assemble one. Apply the same rule to `ResolvedHeights`, which exists
**twice** today (`core/compute/config.rs:1656`,
`core/diagnostics/adapters/from_static_checks.rs:58`).

**Invalidation (N13 class, N14, N15).** `apply(Command) -> Effects` must be the
only write path, and `Effects` must carry the stale set that same call computed.
Today `MutationKind` (`core/session/compute.rs:45-51`) is a *description* the
caller supplies **after** the write, repeated at ten MCP sites
(`viz/app/mcp.rs:3464,3737,3796,3874,4635,4755,5185,5222,5291,5417`), so the
reply's `stale_toolpaths` can disagree with what core dropped. One producer
cannot disagree with itself. `MutationKind` is the embryo of `Command`. Note
that N15's pinning test
(`compute_stale_set_for_toolpath_param_returns_single_toolpath`,
`core/session/compute.rs:5355`) asserts the narrow answer. Phase 1A must change
that test, not only the code.

Restrict the escape hatches to `pub(crate)`: `toolpath_configs_mut`
(`core/session/mod.rs:1710`), `stock_mut` (`:1590`), `machine_mut` (`:1598`),
`tools_mut` (`:1607`), `setups_mut` (`:1720`), `insert_result`
(`core/session/mutation.rs:1284`). **25 non-test viz call sites use them.**
`insert_result` has exactly one non-test caller — the worker adopting a result
(`viz/controller/events/compute.rs:973`) — and it becomes
`apply(Command::AdoptResult { .. })`, the `Job` kind's third step. That is where
Phase 1A meets Phase 1B: the revision stamp `PLAN.md:167-173` asks for rides on
that command. Nine of the 25 sites sit inside egui draw code
(`viz/ui/properties/mod.rs:137-1732`, including `:282`, which hands
`stock_mut()` to a whole draw function). Those nine need scratch-copy and
emit-on-change. That is a pattern change, not a rename, and it is the real
migration cost. It is still small.

### Migration order and first step

1. **Phase 1A** — `apply` and `Effects` are born with the mutation contract.
   The enum is its argument. Restrict the six hatches.
2. **Phase 3** — publish `ResolvedGenInputs`, privatise its fields, narrow the
   executor. N12 closes here.
3. **Phase 4B** — the row's `Params` type carries the validity rules.

Start with `SetToolpathParam` alone. It already reaches core
(`core/session/compute.rs:336`), MCP (`viz/app/mcp.rs:3444`) and the CLI
(`cli/src/job.rs:629`, `cli/src/smoke.rs:586`). N5, N13, N14 and N15 all orbit
it. Ship one row, one `apply`, one sentry. Do not convert 62 rows first.

### Completeness sentry

Model it on `viz/tests/overlays_registry.rs:76`. Iterate `Command::ALL`. Assert
per row: the wire name is unique; every surface in `surfaces` resolves a
handler; every surface NOT in `surfaces` carries a `Skip(reason)` with a
non-empty reason. A missing surface then becomes a declared decision.

### Risks

- **Compile time.** `macro_rules!` keeps `syn` out of the graph, and rustc
  reports errors against the row.
- **Frame loop.** `apply` must stay cheap. Only `Job` submits to the lane
  (`viz/compute/mod.rs:38-62`). Measure the seam before moving work.
- **MCP compatibility.** A wire name is a row literal, so a rename is a visible
  edit, never a side effect of a variant rename.
- **The MCP server stays inside the GUI process.** The 14 `UiCommand` tools need
  the viewport. Keep the two off-loop hatches (`cancel_generation`
  `mcp_server.rs:1434`, `generation_status` `:1442`) outside the registry.
- **Do not generate CLI subcommands.** The CLI is a batch surface: 7 subcommands
  driven by a job TOML. 62 generated subcommands would invent a product nobody
  asked for.

### Where this departs from sysml-rs

The pattern comes from `sysml-service-macros`. Five deliberate departures:

1. **An enum plus an exhaustive `match`, not `inventory::submit!`.** sysml's
   registry is link-time: an unknown name gets a runtime `NotFound` after a
   linear scan (`sysml-service/src/command_trait.rs:59-73`).
2. **`macro_rules!`, not a proc-macro.** sysml's proc-macro mainly derives
   request structs from method signatures — `type_mapping.rs` is 707 of that
   crate's 1,416 lines. rs_cam already owns those structs.
3. **Four declared kinds, not one kind plus a `stateful` flag.** sysml has no
   `async fn` in the annotated block; it models long work as a session family.
   rs_cam has a worker lane and a frame loop, so the kind must be a type.
4. **The unconstructible input types and the `pub(crate)` hatches are primary.
   The registry is secondary.** sysml needs no equivalent: its transports share
   one `SysmlService` and never resolve their own inputs.
5. **A declared surface set per row.** sysml records exclusions as doc comments
   — nine methods say "not a `#[service_command]`"
   (`sysml-service/src/lib.rs:973,2620,2648,2677,2712,2789,2814,3453,6733`). A
   comment cannot fail a test. `CliSkip(reason)` can.
