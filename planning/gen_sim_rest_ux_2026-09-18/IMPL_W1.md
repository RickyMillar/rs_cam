# W1 implementation brief: the plan walk replaces the fixpoint rounds

Owner: W1 of `planning/gen_sim_rest_ux_2026-09-18/PLAN.md` §4.2 and §6.
Status: BRIEF. Not implemented. Written 2026-09-18 from a read-only map.

Scope: the fixpoint rounds become a plan walk over dependency edges; the
GUI plan simulates at the Simulation panel's effective resolution, auto
included (R1 assumed YES); MCP keeps its explicit `simulation_resolution_mm`
refusal; prefix simulation per setup with the S5 memo, then one full-project
simulation; the no-rest-ops path uses the same plan; a blocked submit inside
a plan does not toast; one `GenerateToolpath(X)` resolves X's ancestors
first (R6 assumed YES). For the Generate All button W1 specifies only the
controller state (W3/R5 handoff). Out of scope: W2, W3, W5, and `feeds/`,
`tool_load/`, `tool/`.

---

## 1. The plan, the cursor and the state machine

Delete `FixpointPlan` and `PendingGenerateAll`
(`controller/generate_all.rs:134-251`). `GenerateAllSink` (`:109-131`)
stays exactly as it is.

```rust
pub enum PlanStep {
    Generate(ToolpathId),
    /// Simulate setups 0..=setup so `upto` can read its prior stock.
    /// `upto` is a LABEL; the F.4 scan picks the snapshot position (§2.3).
    SimulatePrefix { setup: usize, upto: ToolpathId },
    /// The closing full-project simulation.
    SimulateAll,
}

pub enum StepOutcome {
    Pending,
    Done,
    /// Still no stock AFTER its own prefix simulation. Terminal.
    Blocked(String),
    Failed(String),
    Cancelled,
    /// Nothing to submit. Not an error.
    Skipped(String),
}

pub struct GenerationPlan {
    pub steps: Vec<PlanStep>,
    pub outcomes: Vec<StepOutcome>,
    /// The step in flight. `== steps.len()` means finished.
    pub cursor: usize,
    /// A submit is out and the drain owes this plan an outcome.
    pub in_flight: bool,
    /// Re-entrancy guard for a submit that completes inside itself (§1.2).
    pumping: bool,
    pub cancelled: bool,
    /// MCP only. `None` means "read the Simulation panel" (R1, §3).
    pub resolution_mm: Option<f64>,
    pub loop_error: Option<String>,
    pub sink: GenerateAllSink,
}
```

The controller field `generate_all: Option<PendingGenerateAll>`
(`controller.rs:133`) becomes `plan: Option<GenerationPlan>`.
`awaiting_deferred_completions` (`:234-244`) and `awaiting_generate_all`
(`:248-250`) keep their shape.

### 1.1 The loop

Three methods replace `start_generate_all`, `settle_generate_all_round` and
`resume_generate_all_after_simulation`.

```
start_plan(plan):  store, then pump_plan()

pump_plan():
  if plan.pumping { return }        // nested completion; the outer loop continues
  plan.pumping = true
  loop:
    if plan.cancelled or cursor >= steps.len() { finish(); break }
    if plan.in_flight { break }
    plan.in_flight = true           // set BEFORE the submit (§1.2)
    match steps[cursor]:
      Generate(id)              => self.submit_toolpath_compute(id)
      SimulatePrefix{setup, ..} => if !self.run_simulation_prefix(setup, true)
                                       { record Skipped }
      SimulateAll               => if !self.run_simulation_all(false)
                                       { record Skipped }
  if let Some(p) = self.plan.as_mut() { p.pumping = false }   // finish() may have taken it

record_step_outcome(o): write o at cursor; in_flight = false; cursor += 1; pump_plan()

finish(): self.compute.clear_sim_prefix_cache(); take the plan; report on its sink
```

The last step can complete synchronously, so the nested `record_step_outcome`
advances the cursor and the outer loop calls `finish()`, which TAKES the
plan. The tail must therefore re-borrow, not hold one across the loop.

`start_plan` refuses while `self.plan.is_some()` (§4).

### 1.2 Why `in_flight` is set before the submit

`submit_toolpath_compute` can reach a terminal state inside itself. Every
submit-time refusal funnels through `fail_toolpath_submit`
(`compute.rs:35-44`) or `block_toolpath_submit` (`:52-67`), and both call
`toolpath_completion_landed` (`:1805-1810`), which re-enters the plan while
`pump_plan` is on the stack. `in_flight = true` gives the nested
`record_step_outcome` a step to close. `pumping` makes the nested
`pump_plan` return at once, so the outer loop takes the next step. Without
it the recursion depth equals the step count.

### 1.3 Which `ComputeMessage` arms advance the plan

`drain_compute_results` (`compute.rs:411-430`) has six arms. Two advance
the plan, and no other arm may.

| Arm | Path |
|---|---|
| `ComputeMessage::Toolpath` | `adopt_toolpath_result` (`:436`) → `toolpath_completion_landed` (`:1805`) → `record_step_outcome` |
| `ComputeMessage::Simulation` | `adopt_simulation_result` (`:651`): `Ok` tail (`:925`), `Err(Cancelled)` (`:928-940`), `Err(Message)` (`:942-957`) |

`toolpath_completion_landed` reads the runtime row as
`record_generate_all_completion` does today (`:1826-1895`):
`result.is_some()` → `Done`; `AwaitingPriorStock(b)` → `Blocked(b.message)`;
`Error(e)` → `Failed(e)`; `Pending` → `Cancelled`; the rest → `Failed`. A
`Generate` step closes ONLY when the completion names the cursor's id. A
completion for any other id is foreign (the auto-regen sweep, an MCP single
generate) and leaves the plan alone.

The three simulation arms map to `Done`, `Cancelled` and `Failed(error)`.
A cancelled or failed simulation sets `plan.cancelled` and `loop_error`,
because every later step in that setup needs the snapshot that never came.

### 1.4 Skipping what cannot help

After a `Blocked` or `Failed` `Generate` in setup S, record every later
`SimulatePrefix { setup: S, .. }` as `Skipped`. The F.4 scan latches at the
first enabled op with no generated result (`simulate.rs:271-289`), so those
simulations cannot unlock anything and are pure cost. The `Generate` steps
behind them still run and still report `Blocked`, so the summary is
unchanged.

### 1.5 Termination

The plan is a finite list, no step is retried, and the cursor only moves
forward. `max_rounds` (`generate_all.rs:215`) and the three-part advance
condition (`compute.rs:1646-1660`) both go. The old bound holds
structurally: the walk emits one `SimulatePrefix` per Stock-edge op with no
snapshot.

---

## 2. Plan construction from the edge function

W0 supplies `rs_cam_core::session::dependencies` with
`edges(&ProjectSession) -> Vec<Edge>`,
`Edge { from: ToolpathId, on: Option<ToolpathId>, kind: EdgeKind }`,
`state(&Edge, &ProjectSession) -> EdgeState` and `primary_edges(..)`
(`IMPL_W0.md` §1). W1 consumes `edges` and never re-derives one. Confirmed
against W0: `edges` emits one Stock edge per (consumer, upstream) PAIR, so
a consumer carries a Stock edge for every enabled same-setup predecessor.

### 2.1 The walk

```rust
pub fn build_plan(
    session: &ProjectSession,
    has_snapshot: impl Fn(ToolpathId) -> bool,
    scope: &[ToolpathId],            // the ops to make current
) -> Vec<PlanStep> {
    let mut steps = Vec::new();
    for (setup_idx, setup) in session.list_setups().iter().enumerate() {
        for tp_idx in &setup.toolpath_indices {
            let Some(tc) = session.toolpath_configs().get(*tp_idx) else { continue };
            if !tc.enabled || !scope.contains(&tc.id) { continue }
            if tc.stock_source == StockSource::FromRemainingStock
                && !has_snapshot(tc.id)
            {
                steps.push(PlanStep::SimulatePrefix { setup: setup_idx, upto: tc.id });
            }
            steps.push(PlanStep::Generate(tc.id));
        }
    }
    if !steps.is_empty() { steps.push(PlanStep::SimulateAll); }
    steps
}
```

Setup order is plan order; inside a setup the order is
`setup.toolpath_indices`. That is the only order in which a Regions or a
PrevTool source is guaranteed earlier than its consumer, so those two edge
kinds need no step of their own. `has_snapshot(id)` is
`simulation.prior_stock_for(id).is_some()`, the predicate the submit door
already reads (`compute.rs:280-281`), so a project holding a covering
simulation plans zero prefix simulations.

### 2.2 One prefix simulation per rest op, and why that is exactly F.4

The F.4 rule is on `SimGroupEntry::phantom_prior_stock`
(`rs_cam_core/src/compute/simulate.rs:218-260`) and enforced by
`PhantomPriorStockScan::visit` (`:271-289`): the scan locks in at the FIRST
enabled config with no generated result, so one simulation unlocks at most
one pending op per setup.

Setup 1 = `[rough (Fresh), rest1, rest2]`, all cold. The walk emits
`Generate(rough)`, `SimulatePrefix{0, rest1}`, `Generate(rest1)`,
`SimulatePrefix{0, rest2}`, `Generate(rest2)`, `SimulateAll`. Step 2 runs
when only `rough` has a result, so the scan resolves at `rest1`. Step 4 runs
when `rough` and `rest1` both have results, so it resolves at `rest2`. Each
prefix simulation unlocks exactly the op the next step generates, so k rest
ops in one setup give k prefix simulations.

### 2.3 The prefix request must not be narrowed to `upto`

`run_simulation_with_ids` (`controller/events/simulation.rs:344-384`) is
not usable as a step's submit. It returns `()` (a step that submits nothing
waits forever; the ladder needed the same guard at `compute.rs:1682-1688`),
it hardcodes `memoize_prefix: false` (`:382`), and it toasts when there is
nothing to simulate (`:356-359`).

The fourth reason is correctness. `build_simulation_groups` feeds the scan
`phantom_scan.visit(toolpaths.len(), ...)` (`simulation.rs:143-149`), and
`toolpaths.len()` counts only the entries the include filter ADMITTED. Drop
a generated op that sits before the pending one and `k` shifts down, so the
snapshot is taken before that op's cuts. For a rest op that is over-cut,
not a stale preview. The step's filter is therefore "every ENABLED op in
setups `0..=setup`", never a narrowed id list:

```rust
/// Simulate setups `0..=setup_idx`, every enabled op, for the plan.
/// Answers `false` when the builder produced no group, so the plan records
/// `Skipped` instead of waiting for a result that never drains.
pub(crate) fn run_simulation_prefix(
    &mut self,
    setup_idx: usize,
    memoize_prefix: bool,
) -> bool {
    let Some((groups, tools, bbox)) = self.build_simulation_groups(
        |i, tc| i <= setup_idx && tc.enabled,
        |i| i == setup_idx,
    ) else {
        return false;
    };
    self.submit_simulation_for_groups(groups, &tools, bbox, Some(setup_idx), memoize_prefix);
    true
}
```

`run_simulation_with_ids` keeps its one remaining caller,
`handle_inspect_toolpath_in_simulation` (`events/toolpath.rs:519`).

### 2.4 Read `edges`, not `primary_edges`, for the ancestor closure

A Stock edge's real requirement is every enabled same-setup predecessor,
because that is what the snapshot holds and what the F.4 scan gates on.
`primary_edges` keeps only the NEAREST enabled source, which is the card's
one connector row, and `prior_stock_blocker` (`compute.rs:77-164`) names
that same nearest op while the scan gates on the FIRST ungenerated one.
That mismatch is D6 in PLAN §5 and the 2026-09-18 live observation in
`planning/design_audit_2026-09-17/SYNTHESIS.md`. W1 does not fix D6 and must
not inherit it, so the R6 closure (§5) reads `edges`, never `primary_edges`.
**Interface check for W0:** the Stock arm of `edges` must keep every enabled
predecessor, not only the nearest one.

### 2.5 The S5 prefix memo

- `SimulatePrefix` passes `memoize_prefix = true`. Prefix simulations of one
  setup run back to back over a growing project, the case the memo was
  built for (`events/simulation.rs:316-322`).
- `SimulateAll` passes `false`. It is the last simulation and leaves nothing.
- `clear_sim_prefix_cache` runs once, in `finish()`, on every exit path.
  Today it runs only on the `Next::Finish` arm (`compute.rs:1701`), so a
  ladder stopped by a simulation error leaks the held snapshot. One exit
  closes the leak.
- `run_simulation_with_all_memoized(bool)` becomes `run_simulation_all(bool)`
  beside `run_simulation_prefix(setup, bool)`, so one rule covers both.

---

## 3. Resolution (R1)

Delete `pinned_simulation_resolution` (`events/toolpath.rs:488-492`) and do
NOT replace it with an "effective resolution" read. There is nothing to
pre-compute: the auto value is derived inside `submit_simulation_for_groups`
(`events/simulation.rs:249-252`) by `auto_resolution_for_tools` (`:516-540`)
from the tools of the request being built. The GUI plan therefore carries no
resolution at all. It submits, and the panel's standing setting applies,
auto included. That is R1.

The MCP plan keeps its refusal. `plan_fixpoint` (`generate_all.rs:81-103`)
reduces to one MCP-only check:

```rust
/// MCP only. The GUI reads the Simulation panel (R1).
pub fn require_resolution(
    needs_simulation: bool,
    rest_op_indices: &[usize],
    supplied: Option<f64>,
) -> Result<Option<f64>, MissingResolution>
```

`MissingResolution` (`:54-70`) and the MCP refusal text
(`compute.rs:1515-1540`) stay word for word. The GUI refusal
(`toolpath.rs:459-472`) goes.

`GenerationPlan::resolution_mm` reaches the request through a new
`Option<f64>` argument on `submit_simulation_for_groups`, written into
`SimulationRequest::resolution` (`simulation.rs:293`) instead of the panel
value. `None` keeps today's behaviour.

**The MCP sink appends `SimulateAll` only when `resolution_mm` is `Some`.**
Every plan ends with that step (§2.1), and an MCP `generate_all` on a
project with no rest ops passes `require_resolution` without supplying one.
Appending the step there would simulate at the panel's auto value the agent
never asked for, which is precisely the silently chosen cell size A/M10
refuses. Without a resolution the MCP plan stops after the last `Generate`
and the agent calls `run_simulation` itself. The GUI sink always appends it:
the panel setting IS the operator's standing choice (R1). Three writes go with it: the
`simulation.resolution` write (`compute.rs:1680`), the
`auto_resolution = false` write (`:1681`), and `resolution_override_notice`
(`generate_all.rs:288-310`) with its call (`:1673-1678`).

**Recommendation: delete `resolution_override_notice` outright; do not keep
it for MCP.** Its whole subject is the ladder overwriting the operator's two
dials (`generate_all.rs:274-286`). Once the MCP resolution rides on the
request and never touches `SimulationState`, there is nothing to warn about.
The MCP reply reports the cell used from
`SimulationResults::column_grid_cell_mm` (`state/simulation.rs:583-587`).

Sentries that pin the current refusal text:

| Sentry | What it pins |
|---|---|
| `tests/generate_all_fixpoint_parity.rs:192-214` | the GUI refusal names "Auto from tool size" and the op count |
| `tests/generate_all_fixpoint_parity.rs:39-116` | `plan_fixpoint`'s four arms |
| `src/controller/fixpoint_resolution_notice_g_resnotice.rs` (243 lines) | G-RESNOTICE, the whole notice |
| `src/controller/tests/generate_all.rs:535-560` | the MCP refusal, which STAYS |

The string "Auto from tool size" stays live at `ui/sim_op_list.rs:132`. It
is the checkbox, not the refusal.

---

## 4. No toast inside a plan

`compute.rs:280-288` blocks and toasts unconditionally. The change:

```rust
    let block = self.prior_stock_blocker(tp_id, &toolpath_name);
    let notice = block.message.clone();
    // The SAME rule §1.3 uses to close a step: is this the cursor's target?
    let in_plan = self.plan_step_target() == Some(tp_id);   // read BEFORE the call
    self.block_toolpath_submit(tp_id, block);
    if !in_plan {
        self.push_notification(notice, super::super::Severity::Warning);
    }
    return;
```

The gate is "this submit IS the step in flight", not "a plan is running".
The two differ for a foreign submit landing beside a plan, which keeps its
toast because nobody is watching a progress surface for it. `in_plan` is
read first because `block_toolpath_submit` re-enters the plan and can finish
it (§1.2). The row still reads WAIT: the call sets
`ComputeStatus::AwaitingPriorStock` (`:58`) and the card derives from that.
Nothing is hidden, only untoasted.

### Can a blocked submit happen outside a plan after R6?

Yes, on two paths, and both should go through a plan.

1. **The 500 ms auto-regen sweep**, `controller.rs:419-450`, calls
   `submit_toolpath_compute(id)` directly per stale id. An edit to a rough
   op drops the whole chain through the core walker, so the sweep submits
   rough, rest1 and rest2 together and two of them toast.
   **Recommendation: the sweep builds ONE plan over the stale set and calls
   `start_plan`.** The set is already collected at `:424-435`.

   **The sweep must do nothing while a plan runs.** This is not a corner
   case; it happens inside every plan. `adopt_toolpath_result` calls
   `mark_derived_rest_dependents_stale` (`compute.rs:381-405`) when a
   Regions source lands, and after W0's D1 fix `AdoptResult` drops the
   consumers through the walker as well. So generating `Rivers` stamps
   `Lakes` stale while `Lakes` is still a step in the plan, and 500 ms later
   the sweep would submit it directly, racing the cursor. Guard
   `process_auto_regen` with `if self.plan.is_some() { return }`. The stamps
   survive `finish()`, so a genuine leftover is swept on the next tick.
2. **MCP `generate_toolpath`**, `app/mcp/generation.rs:817-872`, pushes
   `AppEvent::GenerateToolpath` (`:859-861`) onto the same handler as the
   GUI click. **Recommendation: it takes the R6 plan too.** The MCP waiter
   still resolves off `notify_mcp_toolpath_complete`
   (`compute.rs:1911-1960`), keyed by id, so it reads the target op's own
   outcome and not the plan's.

With both routed, a blocked submit outside a plan is reachable only when a
foreign submit lands beside a running plan. That case keeps its toast,
because nobody is watching a progress surface for it.

**Edit mid-plan.** An operator edit drops results the plan has produced.
**Recommendation: an edit cancels the plan and the sweep re-plans 500 ms
later.** Splicing steps into a live cursor is not worth the failure mode.

The discriminator is `GuiState::edit_counter`, NOT `stale_since`. Stamp the
counter on the plan at `start_plan`; `pump_plan` sets `cancelled` when the
live counter differs. Verified: `mark_edited` (`state/runtime.rs:341-344`)
is the only writer of `edit_counter`, and it is called from the edit
handlers, never from `adopt_toolpath_result`; `stamp_stale`
(`state/stale.rs:42-61`) writes `stale_since` alone. So the plan's own
completions cannot cancel it, and an operator edit always does.

---

## 5. A single Generate resolves ancestors (R6)

`AppEvent::GenerateToolpath(tp_id)` dispatches straight to
`submit_toolpath_compute` today (`controller/events/mod.rs:245`). It becomes
`handle_generate_toolpath`, beside `handle_generate_all` in
`controller/events/toolpath.rs`:

```rust
pub(crate) fn handle_generate_toolpath(&mut self, tp_id: ToolpathId) {
    let mut scope = self.ancestors_of(tp_id);        // transitive, plan order
    scope.push(tp_id);
    let steps = build_plan(&self.state.session, |id| self.has_snapshot(id), &scope);
    self.start_plan(GenerationPlan::new(steps, None, GenerateAllSink::Gui));
}

fn ancestors_of(&self, target: ToolpathId) -> Vec<ToolpathId> {
    // `edges`, never `primary_edges`: a Stock edge exists per upstream (§2.4).
    let edges = dependencies::edges(&self.state.session);
    let (mut open, mut seen) = (vec![target], HashSet::new());
    while let Some(id) = open.pop() {
        for src in edges.iter().filter(|e| e.from == id).filter_map(|e| e.on) {
            if seen.insert(src) { open.push(src); }
        }
    }
    plan_order(&self.state.session, seen)   // drop ops that are already current
}
```

An ancestor with a current result leaves the scope, so Generate on a leaf
whose chain is current submits one step. The `SimulateAll` tail still runs,
because the newly generated op changes the stock.

A Regions consumer whose source is stale needs no special case. The source
is an ancestor, so it enters the scope, and the walk puts it before the
consumer in plan order. It regenerates first and the consumer reads fresh
regions. That closes the sequencing half of D1; the staleness half is W0's
`AdoptResult` fix.

---

## 6. The progress state the button reads (W3 / R5 handoff)

W1 exposes one read-only accessor on `AppController`. The button is on the
other account's card at `ui/toolpath_panel.rs:42-48`.

```rust
pub enum PlanActivity { Generating { name: String }, Simulating { setup: String } }

pub struct GenerationPlanProgress {
    pub step: usize,          // 1-based position of the step in flight
    pub of: usize,
    pub activity: PlanActivity,
    pub cancellable: bool,
}

#[must_use]
pub fn generation_plan_progress(&self) -> Option<GenerationPlanProgress>;
```

Derived each frame from the cursor, never stored. `None` means no plan runs
and the button draws its normal label. Names resolve at read time, so a
rename mid-plan reads correctly. The button's contract for W3: label
`Generating 3/8` or `Simulating · Setup 1`, a thin bar at `step / of`, and a
click that raises `AppEvent::CancelGeneration`. The five-second status line
stops carrying plan progress, so the plan's GUI sink no longer calls
`set_status` (`controller.rs:405-407`).

### Cancel

`AppEvent::CancelGeneration` does not exist today. W1 adds it. The handler
sets `plan.cancelled = true`, calls
`self.compute.cancel_lane(ComputeLane::Toolpath)`, and adds
`cancel_lane(ComputeLane::Analysis)` when the step in flight is a
simulation.

- **Do not call `invalidate_simulation`** (`events/simulation.rs:26-37`). It
  clears `results`, `prior_stocks` and playback, which throws away the
  snapshots the plan already earned.
- The cancelled step drains as `ComputeError::Cancelled`. The toolpath arm
  resets the status to `Pending` → `StepOutcome::Cancelled`; the simulation
  arm reaches `Err(Cancelled)` (`compute.rs:928-940`). Either way `finish()`
  runs, the memo is released, and the plan reports what it completed.
- MCP `cancel_generation` cancels the toolpath lane alone, on the MCP
  thread, off the frame loop (`ui_command.rs:772`). It knows nothing about
  the plan, and it does not need to: the resulting `Cancelled` closes the
  step on the next drain.

---

## 7. MCP surface (W5 owns the wire)

`build_generate_all_response` (`mcp_bridge.rs:1481-1503`) keeps `ok`,
`summary`, `generated`, `failed`, `errors`, `awaiting_prior_stock` and
`loop_error`. Two keys change. **State the break in the commit**, per
`rs_cam_mcp/CLAUDE.md` ("breaking a wire shape is allowed; do not add an
alias"):

- `rounds` becomes `steps`, the plan's step count. A round no longer exists.
- `simulations` stays, now `SimulatePrefix` count plus the one `SimulateAll`.

`generate_all_headline` (`generate_all.rs:314-336`) loses "in N generate
rounds" and gains "in N steps". Both surfaces read it, so the GUI toast and
the MCP `summary` stay one text.

`generation_status` gains `plan: { step, of, simulating }`. It is answered
from a `LaneSnapshot` on the MCP server thread, never through the frame loop
(`mcp_bridge.rs:888-944`), so a controller field is unreachable there. W1
publishes the plan beat the way `FrameLoopBeat` is published: an `Arc` of
three atomics (`step`, `of`, `simulating`), stamped by `pump_plan` at every
advance, cleared by `finish()`, handed to the server at construction. W5
renders it. `ProgressUpdate` (`mcp_bridge.rs:429-436`) is unchanged in
shape; the plan sends one per step with `progress = cursor` and
`total = Some(steps.len())`.

---

## 8. Sentry design

New file
`crates/rs_cam_viz/src/controller/tests/generate_all_plan_g_genplan.rs`,
registered in `controller/tests/mod.rs:15-32`.

Harness: `AppController::with_backend(RestChainBackend::new())` plus
`sample_project_into`, the shape `rest_chain_controller` already builds
(`controller/tests/mod.rs:703-736`). Four fixture changes are prerequisites
and W1 owns them.

- **Add `pump_until_idle`.** `pump_until_resolved` (`mod.rs:741-758`) is
  bound to an MCP `rx`. The GUI sink has no oneshot, so the sentry needs a
  twin that loops `drain_compute_results` until `awaiting_generate_all()` is
  false, with the same iteration cap and the same panic message.

- **Un-gate `RestChainBackend`.** It is `#[cfg(feature = "mcp")]`
  (`mod.rs:580, 591, 604`). The GUI sink must be testable without the
  feature.
- **Give its simulation result a real mesh.** It emits `indices: Vec::new()`
  (`mod.rs:664-668`), and `adopt_simulation_result` toasts a Warning for an
  empty mesh (`compute.rs:674-681`), so a "zero warnings" assertion would
  fail before the new code runs. Copy the one-triangle mesh from
  `inject_sim_results` (`mod.rs:368-372`).
- **Make the fake honour F.4.** Today it inserts a prior stock for every
  chain window whose predecessor is generated (`mod.rs:647-658`), which is a
  heuristic. Read `request.groups[].phantom_prior_stock` and insert exactly
  that one id per group, so the sentry proves "one unlock per setup per
  simulation" against the rule the production builder obeys.

Fixture: two setups. Setup 1 = `Rough (Fresh)`, `Rest A
(FromRemainingStock)`, `Rest B (FromRemainingStock)`. Setup 2 = one plain
op. `simulation.auto_resolution = true`, no pinned resolution, every op
cold. One `handle_internal_event(AppEvent::GenerateAll)`, then pump.

1. The step list, in order: `Generate(Rough)`, `SimulatePrefix{0, RestA}`,
   `Generate(RestA)`, `SimulatePrefix{0, RestB}`, `Generate(RestB)`,
   `Generate(plain)`, `SimulateAll`.
2. Exactly 2 `SimulatePrefix` steps, one per rest op, and exactly 1
   `SimulateAll`.
3. `controller.notifications()` holds zero `Severity::Warning` entries. Read
   the whole stack, not `active_notifications` (`controller.rs:383-397`).
4. Every enabled op ends `ComputeStatus::Done` and
   `freshness_at(..).is_current()`.
5. `awaiting_generate_all()` is false and `generation_plan_progress()` is
   `None`.
6. `controller.drain_events()` is empty between pumps (§10.1).

Two more sentries in the same file:

- `a_blocked_submit_inside_a_plan_pushes_no_toast`: arm a plan, submit a
  rest op with no snapshot, assert the row reads `AwaitingPriorStock` and
  the stack gained nothing. Repeat with no plan armed and assert the toast
  IS pushed, so the test is not vacuous.
- `a_single_generate_on_the_last_rest_op_runs_the_ancestors`: cold project,
  `AppEvent::GenerateToolpath(rest_b)`. Assert the plan holds
  `Generate(Rough)` and both prefix simulations, and that `Rough` reaches
  `Done`.

---

## 9. Blast radius

`rg -c` over `crates/`, whole-word, all targets.

| Symbol | Hits | Fate |
|---|---|---|
| `plan_fixpoint` | 14 | → `require_resolution`, MCP only |
| `FixpointPlan` | 13 | DELETE |
| `PendingGenerateAll` | 7 | DELETE → `GenerationPlan` |
| `settle_generate_all_round` | 6 | DELETE → `pump_plan` |
| `resume_generate_all_after_simulation` | 6 | DELETE → `record_step_outcome` |
| `resolution_override_notice` | 6 | DELETE |
| `start_generate_all` | 5 | → `start_plan` |
| `pinned_simulation_resolution` | 4 | DELETE |
| `generate_all_progress` | 3 | → the plan beat |
| `run_simulation_with_all_memoized` | 3 | → `run_simulation_all(bool)` |
| `record_generate_all_completion` | 2 | → `record_step_outcome` |
| `GenerateAllSummary` | 9 | KEEP, `rounds` → `steps` |
| `build_generate_all_response` | 2 | KEEP, two keys change |
| `generate_all_headline` | 4 | KEEP, text edit |
| `clear_sim_prefix_cache` | 5 | KEEP, moves into `finish()` |
| `GenerateAllSink` 24, `awaiting_generate_all` 23, `mcp_start_generate_all` 9, `generate_all_scope` 7, `MissingResolution` 5, `GenerateAllScope` 3, `run_simulation_with_ids` 2 | | KEEP as they are |

Tests that move or flip:

| Test | Why |
|---|---|
| `tests/generate_all_fixpoint_parity.rs:192` `gui_generate_all_refuses_rather_than_guessing_a_resolution` | INVERTS. Auto on now arms a plan with zero toasts. Rename to `gui_generate_all_reads_the_panel_resolution_r1`. |
| `tests/generate_all_fixpoint_parity.rs:39-116` (four `plan_fixpoint` cases) | Rewrite against `require_resolution`; only the MCP arms survive. |
| `tests/generate_all_fixpoint_parity.rs:164` `..._arms_the_ladder_when_a_resolution_is_pinned` | Becomes "arms a plan", pinned or auto. |
| `tests/generate_all_fixpoint_parity.rs:220` `..._without_a_chain_stays_a_single_pass` | INVERTS. The no-rest path goes through the plan, so a plan IS armed. Rename. |
| `src/controller/fixpoint_resolution_notice_g_resnotice.rs` (243 lines) | DELETE with `resolution_override_notice`. |
| `src/controller/tests/generate_all.rs:241` `..._three_deep_rest_chain_to_fixpoint_in_one_call` | Keep the behaviour; assert `steps`, not `rounds`. |
| `src/controller/tests/generate_all.rs:282` `the_fixpoint_loop_terminates_on_a_genuinely_failing_op` | Keep; the bound argument becomes "no step is retried". |
| `src/controller/tests/generate_all.rs:566` `fixpoint_false_keeps_the_old_single_pass_behaviour` | KEEP, including `simulations == 0`: `fixpoint: false` supplies no resolution, so the MCP plan appends no `SimulateAll` (§3) and holds no `SimulatePrefix`. |
| `src/controller/tests/generate_all.rs:535` `generate_all_refuses_to_guess_a_simulation_resolution` | KEEP unchanged. R1 does not touch the MCP refusal. |
| `tests/generate_all_skips_disabled_g_genalldisabled.rs` (165 lines) | Keep; the walk filters on `tc.enabled` at the same place. |
| `tests/mcp_escape_hatches.rs`, `tests/snapshots/mcp_wire_surface.json` | Re-bless for `rounds` → `steps` and the new `plan` block. W5 owns the re-bless. |

Instruction files W1 edits: `crates/rs_cam_viz/src/controller/CLAUDE.md`
(the `generate_all.rs` file-map line and the invariant "do not replace it
with a single pass over the toolpath list"), and the sentry lists in
`controller/CLAUDE.md` and `compute/CLAUDE.md`, which both name
`generate_all_fixpoint_parity` and gain `generate_all_plan_g_genplan`.

---

## 10. Risks

### 10.1 The 137 s dispatch stall (SYNTHESIS `54a25009`)

The cause is in this package's code. The ladder advances by pushing
`AppEvent::GenerateToolpath` onto `self.events` (`compute.rs:1625` and
`:1775-1777`). `self.events` is drained by `RsCamApp::handle_events`
(`app/input.rs:11-14`), and `handle_events` is called from ONE place:
`draw_frame` (`app.rs:918`). The off-frame pump (`app.rs:630-643, 657-666`)
drains compute results and MCP requests and does NOT drain `self.events`.
So every round handoff waits for a painted frame, and on a hidden or
occluded Wayland surface no frame ever runs. That is G-LV.1 again.

**The plan walk must not depend on a painted frame.** `pump_plan` submits by
calling `submit_toolpath_compute(id)`, `run_simulation_prefix(..)` and
`run_simulation_all(..)` DIRECTLY, and pushes no `AppEvent`. The advance is
pumped from `drain_compute_results` (`compute.rs:411`), which
`pump_dispatch` calls first (`app.rs:631`), and `pump_dispatch` runs from
`off_frame_pump` on `about_to_wait` with no frame. The wake is already
armed: `awaiting_deferred_completions` counts the plan
(`controller.rs:235`), and `needs_pump_tick` (`app.rs:1085-1093`) keeps the
loop waking while it is non-zero.

Only the entry points raise an `AppEvent`: `GenerateAll` and
`GenerateToolpath(id)` arrive through `handle_events` because they come
from a click. Everything after the first submit is direct. Assertion 6 of
the sentry pins this, the way
`generate_all_refuses_to_guess_a_simulation_resolution` already asserts an
empty event queue (`controller/tests/generate_all.rs:556-559`).

### 10.2 Other risks

| Risk | Mitigation |
|---|---|
| Re-entrancy: a submit-time refusal closes its own step inside `pump_plan`. | The `in_flight`-before-submit and `pumping` guards (§1.2), with a sentry over a project whose every op refuses at submit. |
| A foreign completion closes the wrong step. | `record_step_outcome` closes a `Generate` step only when the completion names the cursor's id. |
| A narrowed prefix filter shifts the F.4 `k`. | §2.3. The filter is "every enabled op in setups `0..=setup`", asserted in the sentry by reading `phantom_prior_stock` off the fake's request. |
| A plan that never drains (empty simulation request). | Both simulation helpers answer `bool`; `false` records `Skipped` and the cursor advances. |
| An edit mid-plan invalidates completed steps. | The `edit_counter` stamp cancels the plan; the sweep re-plans (§4). |
| The sweep races the cursor on a consumer the plan itself staled. | `process_auto_regen` returns early while a plan runs (§4). |
| An MCP plan simulates at a cell size the agent never chose. | The MCP sink appends `SimulateAll` only with an explicit resolution (§3). |
| The S5 memo leaks on an error exit. | `clear_sim_prefix_cache` moves into `finish()`, the single exit (§2.5). |
| D6: the blocker message names the wrong op. | Not W1. Recorded against W0/W5; W1 only avoids reading `primary_edges` for the closure (§2.4). |
| R2 cross-setup stock is undecided. | The walk scopes a prefix to setups `0..=setup`, which is what the simulator already does. No new rule is assumed. |

---

## 11. Order of work

`generate_all.rs` (plan types, three deletions) → `events/simulation.rs`
(the two simulation helpers, the `Option<f64>` argument) →
`events/compute.rs` (`start_plan`, `pump_plan`, `record_step_outcome`, the
toast gate) → `events/toolpath.rs` (the two handlers) → `controller.rs`
(field rename, sweep, progress accessor, plan beat) → the sentry file and
the fixture changes → the flipped tests → the two `CLAUDE.md` files.

Local loop, per root `CLAUDE.md`: the touched folder sentries, then
`cargo test -p rs_cam_viz -q`, then clippy. Run every cargo command through
`scripts/cargo_lane.sh`. No large test gate.
