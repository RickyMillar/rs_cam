# Implementation index and reconciliation

Written 2026-09-18 after six parallel read-only scans, one per work
package. Each `IMPL_W*.md` is a brief with exact hunks, blast radius and
sentry designs. This file reconciles them: what one brief found that
changes another, the file ownership, the order, and the rulings the scans
added. Read `PLAN.md` first.

No cargo ran during the scans. No source file changed.

| Brief | Lines | Owner file set (one owner per file) |
|---|---|---|
| `IMPL_W0.md` | 509 | `rs_cam_core/src/session/dependencies.rs` (new), `session/generation_plan.rs` (new, moved here from W1/W5), `session/mutation/toolpath.rs`, `session/command.rs`, `session/compute/generation.rs`, `session/project_file.rs` (load post-pass, moved here from W2) |
| `IMPL_W1.md` | 695 | `rs_cam_viz/src/controller/generate_all.rs`, `controller/events/toolpath.rs`, `controller/events/compute.rs` (ladder functions, `block_toolpath_submit`), `controller/events/simulation.rs` (`run_simulation_prefix`), `controller.rs` (auto-regen sweep guard), `mcp_read_cache` plan beat stamp |
| `IMPL_W2.md` | 390 | `rs_cam_viz/src/ui/properties/tab_badges.rs`, `ui/properties/operations/surface_3d.rs`, `ui/properties/operations/registry.rs`, `ui/properties/toolpath_panel.rs`, `ui/properties/mod.rs`, `ui/overlays/registry.rs`, `ui/overlays/panel.rs`, `ui/components/choice_row.rs` (new) |
| `IMPL_W3.md` | 736 | `rs_cam_viz/src/ui/toolpath_panel.rs` (other account's card, handoff), `state/simulation.rs` (`submitted_scope` stamp), `ui/components/button.rs` (progress bar) |
| `IMPL_W4.md` | 397 | `rs_cam_viz/src/state/freshness.rs` (`SimFreshness`), `state/simulation/playback_state.rs`, the thirteen readers, `app/mcp/simulation.rs` (freshness label); core half (D7 epoch) is in W0's file set |
| `IMPL_W5.md` | 684 | `rs_cam_mcp/src/server.rs`, `rs_cam_viz/src/app/mcp/project.rs`, `app/mcp/commands.rs`, `app/mcp/simulation.rs`, `mcp_bridge.rs`, `rs_cam_cli/src/main.rs`, `rs_cam_cli/src/project.rs`, `tests/mcp_wire_surface.json` |

---

## 1. What the scans changed in the plan

### 1.1 The plan walk lives in core (W5 §8, accepted)

W1 designed `build_plan` in the viz controller. W5 shows the walk reads
only the session and the edges, and the CLI needs the same ordering, so it
becomes `rs_cam_core::session::generation_plan::plan(&session, Scope) ->
Vec<Step>` with `Scope::{Project, Setup, Ancestors(id)}`. It moves into
W0's file set. W1 keeps only the async state machine, the plan beat stamp,
the progress struct and cancel. W5's CLI driver loop replaces
`rs_cam_cli/src/project.rs:334-412` and the CLI ladder cap goes.

W1 §2.3 stands as a constraint on the core walk: a `Simulate` step covers
every enabled op in setups `0..=setup`, never a narrowed id list, or the
phantom scan takes the snapshot before a predecessor's cuts.

### 1.2 The 137 s stall is the ladder's own (W1 §10.1)

The ladder advances by pushing `AppEvent` onto `self.events`, which only
`handle_events` inside `draw_frame` drains (`app.rs:918`). The off-frame
pump never drains it, so with no frame painted the next step waits for
one. This closes the open question from the 2026-09-18 live smoke. The
plan submits by direct call from `drain_compute_results`, and the W1
sentry asserts `drain_events()` stays empty across a whole plan.

### 1.3 D1 must not clear the simulation (W0 §4.1, §5)

The walker clears `session.simulation` at `mutation/toolpath.rs:307`.
Routing `AdoptResult` through it would destroy the `prior_stocks` snapshot
the next rest op in the same plan reads. W0 splits a pure
`walk_output_dependents` from the edit-side door, and D1 calls the pure
walk with `chain_seeds` empty, or every in-flight completion is refused as
`StaleCompletion` (`adopt_result_rejects_stale_completion.rs:183` goes
red).

### 1.4 New defect D7: a late simulation re-fills the core (W4 §2)

`Command::AdoptSimulation` stores unconditionally (`session/command.rs:
2219-2229`) while `AdoptResult` refuses a stale revision. A simulation
that lands after an edit cleared the field re-fills it, so a pure core
read of simulation freshness would reopen G-LATESIM. It is also a safety
matter: rest prior stock is read from that field. Fix R-W4.1: a
`simulation_epoch` on the session, one `drop_simulation()` over the
nineteen `simulation = None` sites, the epoch on `AdoptSimulationArgs`,
refusal on mismatch. Core half in W0's file set, after D1.

### 1.5 New finding D8: the CLI exports a program minus its blocked ops (W5 §6.4)

`export_gcode_checked` (`gcode/mod.rs:331`) silently drops an enabled op
with no result. The G-EXPORTSKIP refusal is viz-only, so `--emit-gcode`
after a non-converged ladder ships a partial program. Ledger row, not W5
work; it wants a core-side refusal.

### 1.6 W2 corrections to §4.3 (W2 §0)

- Rest Analysis has four controls: the `Reference` combo writes
  `rest_analysis.reference_tool_id`, which generation reads. It moves into
  the disclosure, visible only under "Stock".
- The pencil picker gate is `detector != RestDepth || stock_source ==
  Fresh`: only the rest-depth arm reads the simulated stock; the dihedral
  and curvature arms always resolve a reference cutter.
- The overlap fallback is generation-time, not an `EdgeState`. It becomes
  a hover from a generation-result read, added when W0 lands.
- The heatmap overlay cannot be switched on without a grid, so the demand
  is the disabled row's action button: `OpenRestAnalysis` becomes
  `EnableRestAnalysis` (apply `AutoEnableRestAnalysis`, then generate).
- Nothing runs the producer hook on load. A core load post-pass sets
  `rest_analysis.enabled = has_consumer` (no legacy, ruling 2026-09-16).
  Owned by W0 in `project_file.rs`.
- The kit has no two-way choice renderer. `ChoiceRow` is new, pinned by
  the W2 sentry; it cannot join `component_contracts_up2`, whose list is
  the other account's spec.
- The write path stays `ReplaceToolpathConfig`; a `SetStockSource` call
  from draw would be a second write of one field per frame.

### 1.7 W3 handoff items for the other account (W3 §9)

- `the_toolpath_card_is_five_elements_dc1.rs:159` asserts the card calls
  `draw_rest_badge(`; folding the badge needs the needle to become
  `draw_connectors(`. The same file anchors on the literal
  `"Generate All"`, so the progress label keeps that prefix.
- `component_contracts_up2` asserts the button paints text once, so the
  progress words are the button's own text and the bar paints after the
  hover block in `INK_05` (the primary fill is already `ACCENT`).
- A 6-point gutter is off their §2.1 grid; W3 proposes `SPACE_3` (8) and a
  dash pattern as the second channel, since no glyph fits the gutter.
- The rect map is a local `Vec` painted in a second pass in the same
  frame; egui temp memory would break `panel_drafts_leave_egui_memory_ui09`.
- `sim_op_list.rs` gets neither ring nor connector: its rows exist only
  after a run lands.

### 1.8 W1 findings that touch other packages

- The auto-regen sweep (`controller.rs:419-449`) races the plan on
  consumers the plan itself stales. `process_auto_regen` returns early
  while a plan runs.
- An MCP plan appends `SimulateAll` only with an explicit resolution, or a
  no-rest `generate_all` simulates at a cell size the agent never chose.
- MCP `generate_toolpath` routes through the R6 ancestor plan too.
- A mid-plan edit cancels the plan; no splicing.
- `resolution_override_notice` and its sentry are deleted, not kept: the
  MCP resolution rides the request, not `SimulationState`.
- Fixture work W1 owns: un-gate `RestChainBackend` from `mcp`, give its
  fake result a real mesh, make it honour `phantom_prior_stock`, add
  `pump_until_idle`.

### 1.9 W5 wire decisions

- `awaiting_prior_stock` has six emit sites, two subjects (the blocker
  object, the waiting-op row). `AwaitingPriorStock` gains `Serialize`; a
  viz `BlockedRow { toolpath_id, toolpath_index, name, #[serde(flatten)]
  block }` serves the two array sites. Object sites stay byte-identical.
- Core cannot derive `JsonSchema` (schemars reaches `rs_cam_mcp` only via
  `rmcp`), so `set_stock_source.source` takes a mirror enum
  `StockSourceParam`, the `ZRotationParam` pattern.
- `generation_status` answers on the MCP thread; plan progress is a
  `PlanBeat` cell on `McpReadCache` beside `FrameLoopBeat`, stamped by W1.
- `stale_defaults` becomes `default_findings`; the `build_info().features`
  token `"stale_defaults"` stays (a capability, not a key).
- No checked-in client reads `rounds` or passes a `source:` string.

### 1.10 W4 shape

`SimFreshness { NoRun, Running, Current, EditedSince,
CaptureOptionsChanged }` in `state/freshness.rs`, `is_stale()` a thin
predicate, one `AppState::simulation_is_stale()` facade so the thirteen
readers change one line each. Three GUI doors mirror `simulation_cleared`
into `invalidate_simulation` and wipe `last_run`, so a GUI edit reads
`NoRun`; the sentry asserts state, not `is_stale == is_none()`. The
collision stamps move to the same epoch. `SimEvidenceMeta` (the emitted
motion hash) stays. `edit_counter` stays for `dirty` and the reach overlay
scheduling key; `last_sim_edit_counter` and `sim_generation` die. MCP
gains a `freshness` label on `project_summary`, `get_diagnostics` and
`run_simulation`.

---

## 2. Order

```
W0a  edges + state + primary_edges, walker reads edges, D1, D3
W0b  generation_plan::plan (from W1 §2.1 + W5 §8), load post-pass (W2 §0.5)
W0c  D7 simulation_epoch + drop_simulation (W4 §2), one owner, after W0a
W1   async plan machine, off-frame submit, panel resolution, no toast,
     R6, progress struct, PlanBeat stamp, auto-regen guard
W2 ∥ W4 ∥ W5   (W2 needs no W0 read to ship; W4 needs W0c; W5 needs W0a+b)
W3   after W1 (progress) and W0a (edges); handoff slot with the other account
```

W0 is one owner, three sequential commits. Nothing in W0 waits on a
ruling except R2, which is one deletable `if` block in `state()`.

## 2a. W0 landed (2026-09-18)

Ten commits by one owner, `fbd1b526` to `92c25308`, plus the sentry fix
`99ff5a52`:

- W0a: `session::dependencies` (edges, state, primary_edges), the walker
  reads the edges with a pure `walk_output_dependents` under the edit-side
  door, D1 (`AdoptResult` drops Regions and PrevTool consumers), D3.
  Sentry `dependency_edges_are_the_walker_dep1` (6 claims).
- W0b: `session::generation_plan::plan(&session, Scope) -> Vec<Step>`,
  no trailing full-simulation step (the GUI appends its own). The loader
  derives `rest_analysis.enabled` from demand
  (`ProjectLoadWarning::RestAnalysisNormalized`), a stated break. Sentry
  `generation_plan_is_the_edge_walk_w0b` (6 claims).
- W0c: `simulation_epoch` + `drop_simulation()` over all 19 clear sites,
  `AdoptSimulationArgs.epoch`, `SessionError::StaleSimulation`; the viz
  lane stamps `submitted_simulation_epoch` at submit. Sentry
  `a_late_simulation_does_not_refill_the_core_d7` (4 claims).

Green: core lib 2521/0, every affected core sentry, cli, mcp, fmt. Not
run: viz lib tests and workspace clippy, both blocked by the peer's
in-flight S4 (`sim_diagnostics.rs:1878` initializer, `tool_load/power.rs`
lint). Re-run both when S4 lands.

Decisions taken in W0 that later packages inherit: the plan emits a
Generate step for a current op and the driver skips it; `Step::Simulate`
carries the semantic `SetupId`, resolve to a position then cover
`0..=position`; `Scope::Setup` is narrow (stall hazard on the variant
doc); the RestDepth-pencil predicate has two copies, a third becomes one
function; rule (c) over-drops on purpose.

## 2b. All five packages landed (2026-09-19)

| WP | Commits | Result |
|---|---|---|
| W1 | `2da975bd` `aee13f76` `d94c3f85` `6e4e9a32` + tail `3194f2c8` `6ca4d099` | plan machine off-frame, R1 rule + confirm state, R6, PlanBeat, one blocked-row shape, serde on the edge enums |
| W2 | `b57c0e06` `4d09dc9a` `899b84f5` | one Start-from row (`ChoiceRow`), rest dials behind a demand disclosure, D5, overlay demand |
| W3 | `2ba48cef` `e3e3e501` `d40c3f87` `81f7758e` + tail `14b99111` | gutter connector from `primary_edges`, ring in flight, Generate All as progress with cancel, `dep` badge folded |
| W4 | `de6f1ee3` `0dd488cb` `6bdc0899` `1e1857f2` `841fce6d` `a8f0ae6b` | `SimFreshness`, one facade, all readers migrated, collision stamps on the epoch, the GUI counter no longer answers |
| W5 | `2d1cfa65` `e057b96b` `a74ab06a` `cb77f0b6` `2386632e` `cd927147` `486392a8` | one `awaiting_prior_stock` shape, `StockSourceParam`, `depends_on`, `plan` on `generation_status`, CLI plan driver + required resolution, `default_findings` |

Checkpoint gate on the merged tree: `cargo fmt --all -- --check` clean,
workspace clippy with the three features clean, viz lib 409/0, nineteen
viz integration binaries green one invocation each (the seventeen the
agents could not run under each other's edits plus the two wire pins),
core lib 2530/0, cli 19/0, mcp 31/0.

Rulings taken during the run: R1 (auto → finer of auto and required,
silently; manual coarser → confirm), R6 yes, R4 yes with the cost rule
(analysis only on demand), the auto-regen sweep stays manual (no
simulation after an edit; a staled rest op waits at WAIT), R3, R7, R10
yes, R8 taken by W0b, R9 taken by W0c.

Still open: R2 cross-setup stock (one `if` in `dependencies::state`),
R11/R12 MCP freshness wording, R13/D8 a core export refusal for a blocked
op (the CLI can still ship a partial program; W5 made the blocked ops
visible in `summary.json`), the view-only isolation simulation, D6 (the
blocker message names the nearest op, the scan gates on the first
ungenerated; documented in `generation_plan`, not fixed).

Not yet done: the live GUI look (confirm modal, ring, connector, Start-from
row), the core integration suite as a whole, the release rebuild at the
time of writing, any push.

## 3. Rulings the scans added

Beside PLAN.md §8 R1 to R7:

- **R8** The plan walk lives in core (§1.1). Recommended yes.
- **R9** D7 fix shape: core epoch (R-W4.1 option A) over a viz-only stamp.
  Recommended core.
- **R10** Gutter width 8 (their grid) with a dash pattern as the second
  channel (W3 §9.2). The other account's call.
- **R11** MCP simulation freshness label now, or wait for WP28 (R-W4.3).
- **R12** One word for stale simulation evidence across GUI, MCP and the
  tool-load report's `SimEvidenceMeta` (R-W4.4).
- **R13** D8: a core-side export refusal for a blocked op, so the CLI
  cannot ship a partial program.
