# W5 implementation brief: one vocabulary across MCP and CLI

Package W5 of `PLAN.md` (§4.6, §5 D2/D4, §6 W5). Scope: the wire words for
generate, simulate and rest dependency state. This brief plans the work.

Read first: `crates/rs_cam_mcp/CLAUDE.md`, `crates/rs_cam_cli/CLAUDE.md`,
`crates/rs_cam_viz/src/app/CLAUDE.md`. There is no
`crates/rs_cam_viz/src/app/mcp/CLAUDE.md`.

Operator ruling 2026-09-16 applies: break a wire shape, state the break in
the commit, add no alias.

## 0. What W5 needs from other packages

| Needs | From | For |
|---|---|---|
| `session::dependencies::edges`, `EdgeKind`, `EdgeState` | W0 | (c), (f) |
| serde tokens on those enums | W0 | (c) wire words |
| `GenerationPlanProgress { step, of, label }` | W1 | (d) |
| A GUI-thread stamp of that progress | W1 | (d), see §4 |
| `session::generation_plan::plan` | W0 or W1 | (f), see §8 |

Ask W0 for `#[serde(rename_all = "snake_case")] Serialize` on both enums:
`stock` / `regions` / `prev_tool` and `ready` / `pending` / `broken`.
`state` needs the session, because a Stock edge asks whether the snapshot
exists: `pub fn state(session: &ProjectSession, edge: &Edge) -> EdgeState`.

Items (a), (b), (e), (g) need neither W0 nor W1. They can land first.

---

## 1. Item (a): one `awaiting_prior_stock` shape

### 1.1 Six sites, two subjects

`AwaitingPriorStock` lives at
`crates/rs_cam_core/src/compute/config.rs:32-47` and derives
`Debug, Clone, PartialEq, Eq`. That file already imports
`serde::{Deserialize, Serialize}` at line 1, and `StockSource` beside it
derives both. Core already depends on serde here.

**Object sites.** The value describes the BLOCKER. All three keys, identical
text:

| File | Line | Reader |
|---|---|---|
| `crates/rs_cam_viz/src/app/mcp/project.rs` | 71-75 | `list_toolpaths` row |
| `crates/rs_cam_viz/src/app/mcp/simulation.rs` | 37-41 | `mcp_runtime_status_for_toolpath_id` |
| `crates/rs_cam_viz/src/controller/events/compute.rs` | 2118-2123 | `build_mcp_diagnostics` row |

**Object site with a defect.** `compute.rs:1956-1959`, the
`generate_toolpath` failure reply. It omits `message`.

**Array sites.** The row describes the WAITING op and carries its block:

| File | Line | Shape today |
|---|---|---|
| `compute.rs` | 2089-2096 | `{toolpath_index, toolpath_id, name, blocking_toolpath_id, blocking_toolpath_index, message}` |
| `crates/rs_cam_viz/src/mcp_bridge.rs` | 1491-1502 | `{toolpath_id, message}` |

### 1.2 The proposal

Core, `config.rs:31`: add `Serialize` only. Nothing reads the shape back.

```rust
-#[derive(Debug, Clone, PartialEq, Eq)]
+#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
 pub struct AwaitingPriorStock {
```

`blocking_toolpath_id: Option<ToolpathId>` serialises as a bare integer or
`null`, because `ToolpathId` is `#[serde(transparent)]`
(`crates/rs_cam_core/src/ids.rs:22-24`). The three object sites stay
byte-identical.

Viz, one row type for both array sites, beside the summary renderer in
`crates/rs_cam_viz/src/controller/generate_all.rs`:

```rust
#[derive(serde::Serialize)]
pub struct BlockedRow {
    pub toolpath_id: rs_cam_core::ToolpathId,
    pub toolpath_index: Option<usize>,
    pub name: String,
    #[serde(flatten)]
    pub block: rs_cam_core::compute::config::AwaitingPriorStock,
}
```

Then:

- `project.rs:71-75`, `simulation.rs:37-41`, `compute.rs:2118-2123` become
  `"awaiting_prior_stock": status.blocked_on(),`. No break.
- `compute.rs:2089-2096` builds a `BlockedRow`. Six keys, same names. No
  break.
- `compute.rs:1956-1959` becomes the same expression. **Break**: the reply
  gains `message`.
- `mcp_bridge.rs:1491-1498` renders `BlockedRow` for
  `awaiting_prior_stock`. **Break**: rows gain `toolpath_index`, `name`,
  `blocking_toolpath_id`, `blocking_toolpath_index`. `errors` keeps its flat
  `{toolpath_id, message}`, because a failure names no blocker.

`toolpath_index` is `Option<usize>` because the `generate_all` push site
(`compute.rs:1855`) holds only `tp_id`. It serialises as the same integer
when it is `Some`, so `compute.rs:2089-2096` stays byte-identical.

### 1.3 The carrier behind that break

`GenerateAllSummary.blocked` is `Vec<(usize, String)>`
(`generate_all.rs:262`); `PendingGenerateAll.blocked` is
`Vec<(ToolpathId, String)>` (`:162`, mapped at `:176-180`). The push site is
`compute.rs:1854-1856`:

```rust
Some((false, ComputeStatus::AwaitingPriorStock(b))) => {
    ga.blocked.push((tp_id, b.message));
}
```

`b` is the whole struct in hand, so the fix is local. Change both fields to
`Vec<BlockedRow>`, push `b.clone()` with the op's name and index, and delete
the tuple mapping at `generate_all.rs:176-180`.
`generate_all_headline` (`:314-336`) reads only `blocked.len()`.

### 1.4 Sentries

- `crates/rs_cam_viz/src/controller/tests/generate_all.rs:255`, `:308`,
  `:580` read `reply["awaiting_prior_stock"]`. Assert the new keys.
- `crates/rs_cam_viz/src/controller/results_parity_tests.rs:324` says the
  key has no CLI equivalent. Item (f) gives it one. Update that doc row.
- Wire snapshot: no impact. It pins input schemas only
  (`mcp_wire_surface_pin.rs:12-16`).

---

## 2. Item (b): `set_stock_source.source` becomes an enum

**Trap.** `rs_cam_core` cannot derive `schemars::JsonSchema`. schemars
reaches `rs_cam_mcp` only through `use rmcp::schemars;`
(`crates/rs_cam_mcp/src/server.rs:11`), and core does not depend on rmcp. So
the param takes a **mirror enum in `rs_cam_mcp`**, exactly as
`ZRotationParam` mirrors `ZRotation` (`server.rs:36-87`). Copy that
precedent, its doc reason included.

`crates/rs_cam_mcp/src/server.rs:1001-1007`:

```rust
-pub struct SetStockSourceParam {
-    pub index: usize,
-    /// Either "fresh" or "from_remaining_stock"
-    pub source: String,
-}
+/// The two legal stock sources, as a wire enum (W5 item b). A thin mirror
+/// of `StockSource`, which cannot derive `schemars::JsonSchema` because
+/// `rs_cam_core` does not depend on schemars. Each token is the token
+/// core's serde already stores in a project file. `inline` keeps both
+/// values IN the property; the published schema carries no `$defs`.
+#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, schemars::JsonSchema)]
+#[serde(rename_all = "snake_case")]
+#[schemars(inline)]
+pub enum StockSourceParam { #[default] Fresh, FromRemainingStock }
+
+impl From<StockSourceParam> for StockSource { /* two arms */ }
+
+pub struct SetStockSourceParam {
+    pub index: usize,
+    /// Where this operation's material comes from.
+    pub source: StockSourceParam,
+}
```

Add `use rs_cam_core::compute::config::StockSource;` beside the core imports
at `server.rs:14-18`.

**The parser goes.** `crates/rs_cam_viz/src/app/mcp/commands.rs:607-630`:
delete the match and the refusal arm; pass `p.source.into()` to
`SetStockSourceArgs`. `before.extra` at `:623` must serialise the CORE
`StockSource` (already `Serialize`), not the mirror. One serde token owner.

**Break**: an unknown or mixed-case token is now an rmcp deserialisation
error, not the `mutation_error_json` refusal at `commands.rs:611-618`.

**Wire snapshot moves**: `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json:2028-2031`.
The `source` property changes from `"type": "string"` to an inline `enum`.
Re-bless with
`RS_CAM_UPDATE_WIRE_SNAPSHOT=1 cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`,
read the diff, commit the file.

---

## 3. Item (c): `list_toolpaths` gains `depends_on` and `stock_source`

Site: `crates/rs_cam_viz/src/app/mcp/project.rs:61-76`.

`ToolpathSummary` (`crates/rs_cam_core/src/session/mod.rs:1014-1021`) carries
no `stock_source`. Do not widen it. Read the config in viz through
`session.find_toolpath_config_by_id(s.id)`, beside the existing
`toolpath_rt` read. Call `dependencies::edges(session)` ONCE above the
`map`, not per row.

New keys per row, after `awaiting_prior_stock`:

```json
"stock_source": "fresh",
"depends_on": [{"id": 3, "kind": "stock", "state": "pending"}]
```

`depends_on` is `[]` when nothing is depended on. An empty list means "asked
and none". Never emit `null` there.

**Caps.** `list_toolpaths` is uncapped today, and `depends_on` adds a few
rows per op. The `MAX_RESPONSE_BYTES` budget
(`crates/rs_cam_mcp/src/response.rs:50`) is not at risk. Add no cap; per
`rs_cam_mcp/CLAUDE.md` a cap is named in `response.rs` if one is needed.

**Break**: none, both keys are additive.

Add the capability token `"toolpath_depends_on"` to `build_info().features`
(`crates/rs_cam_mcp/src/server.rs:1435-1450`) and to the in-crate assertion
list at `:1563-1580`, so an agent can probe for the keys.

---

## 4. Item (d): `generation_status` gains `plan`

Site: `crates/rs_cam_viz/src/mcp_bridge.rs:927-944`
(`build_generation_status_response`).

**The thread problem.** `generation_status` is answered on the MCP server
thread (`crates/rs_cam_viz/src/mcp_server.rs:1330-1334`) from
`self.generation.snapshot()` and `self.reads.frame_loop()`. Plan progress
lives in `PendingGenerateAll` on the GUI thread. The compute lane cannot see
it, so a field on `LaneSnapshot` would always read null.

There are two GUI to MCP channels already. Pick the second:

- `McpReadSnapshot` (`mcp_bridge.rs:33-41`), published by
  `publish_mcp_read_snapshot` (`crates/rs_cam_viz/src/app/mcp.rs:205-224`)
  on a 500 ms rate limit. **Reject it.** A plan step changes faster than
  500 ms, and the snapshot stops when the frame loop parks. That is the
  exact failure `generation_status` exists to see through (G-LV.1).
- `FrameLoopBeat`, a shared cell on `McpReadCache` (`mcp_bridge.rs:387-400`)
  that the GUI thread stamps and the MCP thread reads, with no rate limit.
  **Take this one.** A second cell on the same struct extends the existing
  path; it is not a parallel flow.

```rust
pub struct PlanBeat { cell: RwLock<Option<GenerationPlanProgress>> }
impl PlanBeat { pub fn stamp(&self, p: Option<GenerationPlanProgress>); pub fn read(&self) -> Option<GenerationPlanProgress>; }
// McpReadCache gains: plan: Arc<PlanBeat>, pub fn plan(&self) -> &PlanBeat
```

**W1 must own the stamp.** W1 stamps at every step and clears the cell when
the plan ends or is cancelled. Without that stamp `plan` is always `null`.

The hunk, after `"queue_depth"` at `mcp_bridge.rs:936`:

```rust
+ "plan": plan.map(|p| serde_json::json!({
+     "step": p.step, "of": p.of,
+     "simulating": p.label == PlanStepLabel::Simulating,
+ })),
```

`build_generation_status_response` takes a third argument
`plan: Option<GenerationPlanProgress>`; both call sites pass
`self.reads.plan().read()`. Derive `simulating` from W1's `label`; do not
store the bool twice. Ask W1 for an enum label, not a `String`.

**Break**: none. The key is additive and `null` when no plan runs.
`crates/rs_cam_viz/src/app/mcp/generation.rs` needs no edit: it holds the
planner and feeds handlers, not `generation_status`. A client reads `plan`
beside `frame_loop`: a parked frame loop means the stamp has stopped, not
that the plan finished.

Add the token `"generation_plan_progress"` to both lists in §3.

---

## 5. Item (e): `rounds` becomes `steps`

`crates/rs_cam_viz/src/mcp_bridge.rs:1499`:

```rust
-        "rounds": summary.rounds,
+        "steps": summary.steps,
         "simulations": summary.simulations,
```

Rename `GenerateAllSummary.rounds` to `steps` (`generate_all.rs:263-264`)
and its producer at `:181`. After W1 the value counts plan steps, not ladder
rounds, so the old word reports a measure that no longer exists. Rewrite
`generate_all_headline` (`:325-331`) from `"in {} generate round{}"` to
`"in {} step{}"`.

**Break**: `rounds` is gone. No alias.

Sentries that move: `controller/tests/generate_all.rs:262`, `:301-304`,
`:572`; `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs:38`, `:189`
(prose and a bound assertion). Wire snapshot: no impact.

---

## 6. Item (f): the CLI

### 6.1 `--resolution` stops defaulting

`crates/rs_cam_cli/src/main.rs:155-157` (`project`) and `:267-269` (the
acceptance sweep) both carry `#[arg(long, default_value = "0.5")]
resolution: f64`. clap cannot see the project, so the refusal cannot live in
the flag. Change the `project` flag only to `resolution: Option<f64>`.

`run_project_command` (`crates/rs_cam_cli/src/project.rs:229-243`) takes
`Option<f64>` and refuses after the load at `:249`, before generation:

```rust
let has_stock_edge = dependencies::edges(&session).iter().any(|e| e.kind == EdgeKind::Stock);
let resolution = match (resolution, has_stock_edge) {
    (Some(r), _) => r,
    (None, false) => 0.5,
    (None, true) => anyhow::bail!(REST_NEEDS_RESOLUTION),
};
```

The MCP refusal text is at
`crates/rs_cam_viz/src/controller/events/compute.rs:1523-1537`. Its middle
is surface neutral and must be reused word for word:

> The resolution is NOT guessed: collision counts and engagement both move
> with cell size, so a silently chosen one would hand you verdicts you did
> not ask for. Pass the same resolution you will use for verification, well
> below the finishing tool's TIP radius (e.g. 0.1 for a 1 mm ball).

Lift that middle into one `pub const` in
`crates/rs_cam_core/src/compute/config.rs`, beside `AwaitingPriorStock`.
`compute.rs:1524` interpolates it. Each surface adds its own opening clause
(`generate_all needs simulation_resolution_mm` / `--resolution is
required`) and its own escape clause (`fixpoint: false` / none).

Leave `main.rs:267-269` alone. That command reads a CSV of synthetic cases,
so it has no edge to test.

### 6.2 The ladder cap goes, it does not move

`crates/rs_cam_cli/src/project.rs:372` is
`let ladder_cap = session.toolpath_count() + 2;`. After W1's plan walk there
is no cap: each enabled op is visited once. Delete the
`while !pending.is_empty()` loop at `:374-412` with it.

Fallback, if the owner refuses §8 and the CLI keeps its own ladder: set the
cap to `rest_dependent_ops + 1`, which is what `FixpointPlan::looping`
(`crates/rs_cam_viz/src/controller/generate_all.rs:240-250`) uses. Do not
leave `toolpath_count() + 2`; the two surfaces must bound alike.

**Correction to the package note**: `project.rs:376-381` is a warn and a
`break`, not an export refusal. It logs `"fixpoint ladder did not converge;
exporting without these toolpaths"` and carries on.

### 6.3 A blocked op is listed, not absent

`project.rs:564-593` builds `per_toolpath` from `diag.per_toolpath`, and the
core builds a diagnostic only for a toolpath that HAS a result. The
per-toolpath JSON loop at `:469-475` skips the same set. So an op still
blocked at the end appears nowhere in `summary.json`.

Add a second list to `ProjectSummary` (`project.rs:184-224`) rather than
faking a diagnostic row:

```rust
/// W5 item (f): operations that never generated because an upstream
/// simulated stock was missing. An EMPTY list means none were blocked.
awaiting_prior_stock: Vec<BlockedEntry>,
```

`BlockedEntry` mirrors `BlockedRow` from §1.2. This closes the
`results_parity_tests.rs:324` note.

The CLI holds no `ComputeStatus`. Today it classifies by a stall heuristic
(`project.rs:384-394`). **Core carries no typed blocked answer**:
`rg -n "PriorStock|prior_stock" crates/rs_cam_core/src/session` returns only
`prior_stocks` snapshot plumbing, and `prior_stock_blocker` is a viz
function (`crates/rs_cam_viz/src/controller/events/compute.rs:77`). So the
CLI reads the edge state: `EdgeState::Pending` on a Stock edge is the
blocked case, and the message comes from the same builder the viz side
uses. Moving `prior_stock_blocker` into core beside `AwaitingPriorStock` is
the clean version; propose it to W0.

### 6.4 Export: a finding, not W5 work

`export_gcode_checked` (`crates/rs_cam_core/src/gcode/mod.rs:323-335`) uses
`let result = project.get_result(idx)?;` inside a `filter_map`, so an
enabled op with no result is SILENTLY DROPPED. The G-EXPORTSKIP refusal is
viz-only (`crates/rs_cam_viz/src/io/export.rs:201-244`). The CLI's
`--emit-gcode` (`project.rs:700-712`) therefore ships a program minus its
blocked ops with no word. Record this for the ledger. Do not widen W5 into
core export.

---

## 7. Item (g): three concepts, one stem

| Name | Meaning | Verdict |
|---|---|---|
| `stale` | one row: its inputs changed after generation | keep |
| `stale_toolpaths` | `MutationResult`: the set this mutation invalidated | keep |
| `stale_defaults` | findings: a parameter still at a default that does not suit the tool or material | rename |

The first two mean "out of date against the current inputs". The third means
"never chosen". Rename it **`default_findings`**; `findings` already names
this population (`StaleDefault`, `from_stale_default`).

Readers: `crates/rs_cam_viz/src/app/mcp/project.rs:26,37`;
`crates/rs_cam_cli/src/project.rs:214,658-686`;
`crates/rs_cam_core/src/compute/validate.rs:116` (the function name);
`crates/rs_cam_core/src/session/compute/export.rs:132,177`;
`crates/rs_cam_viz/src/ui/properties/operations/validate.rs:455,472`;
`crates/rs_cam_core/src/diagnostics/tests.rs:584`;
`crates/rs_cam_core/src/session/compute/diagnostics.rs:1090` (prose);
`crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs:213,230`.

Cost, stated so the operator can rule:

- `crates/rs_cam_cli/tests/stale_defaults_reach_the_cli_cmp25.rs:61,68,73`
  source-scans for the literals `validate::validate_stale_defaults(`,
  `stale_defaults: Vec<rs_cam_core::compute::validate::StaleDefault>` and
  `stale_defaults,`. A rename breaks the scan. Move it and rename the file.
- `build_info().features` carries `"stale_defaults"` as a capability token
  (`server.rs:1436`, asserted at `:1564`). KEEP that token: it records a
  shipped capability, not a key name. Add `"default_findings"` beside it.

This is the widest item in W5 and the one with the least behaviour behind
it. **Recommend: land it last, in its own commit. A reverse of one commit
then removes it without touching the other eight.**

---

## 8. The plan walk belongs in core

**Recommend yes.** The walk needs only the edge function, the session and a
simulate callback. Nothing in it is a GUI concern, and three surfaces need
it. `rs_cam_cli/CLAUDE.md` already rules: route through the same core doors
the GUI uses, and add no CLI-only semantics.

```rust
// crates/rs_cam_core/src/session/generation_plan.rs
pub enum Scope { Project, Setup(SetupId), Ancestors(ToolpathId) }
pub enum Step {
    Simulate { setup: SetupId, upto: ToolpathId },
    Generate { toolpath: ToolpathId, index: usize },
}
/// The ordered work that makes `scope` current. Pure: reads the session
/// and the edges, runs nothing.
pub fn plan(session: &ProjectSession, scope: Scope) -> Vec<Step>;
```

The CLI driver replaces `project.rs:334-412`:

```rust
for step in generation_plan::plan(&session, Scope::Project) {
    match step {
        Step::Simulate { .. } => session.run_simulation_memoized(&sim_opts, &cancel, Some(memo))?,
        Step::Generate { toolpath, index } => {
            if session.generate_toolpath(index, &cancel).is_err() {
                // The typed blocked answer: see §6.3. Core has none today.
                blocked.push(BlockedEntry::for_op(&session, toolpath));
            }
        }
    }
}
```

The S5 prefix memo still applies: the walk's simulations are prefixes of one
another.

**W1 then owns only the async state machine.** It drives the same
`Vec<Step>` through the compute lane, stamps `PlanBeat`, renders the
Generate All button and cancels. It no longer owns the ordering rule, so
GUI, MCP and CLI cannot disagree about what "make this current" means.

**Cross-package flag.** If the walk lands in core it belongs to W0, which
already edits `session/`. Propose that to the programme owner. W5's CLI work
then depends on W0 alone.

---

## 9. MCP tool descriptions

`crates/rs_cam_viz/src/mcp_server.rs`. Short, active, one instruction per
sentence.

**`generate_all` (`:1273`)**. The text today is one sentence of nine lines,
and it names rounds:

> Generate every enabled toolpath. The call plans the work over the
> dependency edges: it simulates a setup prefix when an operation starts
> from remaining stock, then generates that operation. A project reaches a
> fully generated state in ONE call. `simulation_resolution_mm` is REQUIRED
> when any enabled operation starts from remaining stock. The call refuses
> rather than guess a cell size, because collision counts and engagement
> both move with it. The reply separates `errors` (genuine failures) from
> `awaiting_prior_stock` (operations still waiting, each naming what it
> waits for), and reports `steps` and `simulations`. The call waits
> indefinitely. Pass `timeout_s` to bound the wait; on timeout the reply
> says `status: "running"` and generation continues. Read
> `generation_status` for live progress. Call `cancel_generation` to stop
> it.

Keep one sentence for `fixpoint` only if W1 keeps the parameter.

**`generate_toolpath` (`:1237`)**, add for R6:

> The call first makes this operation's dependencies current, then generates
> it.

**`generation_status` (`:1330`)**, add:

> `plan` reports the step a Generate All is on, and whether that step is a
> simulation. A null `plan` means no plan runs.

**`run_simulation` (`:1338`)**, the default is unnamed today:

> `resolution` is the cell size in mm. It defaults to 0.5. Collision counts
> and engagement both move with it, so pass the value you intend.

**`set_stock_source` (`:1203`)**, the token list moves into the schema:

> Set where a toolpath's material comes from. `fresh` starts from raw stock.
> `from_remaining_stock` starts from the simulated stock of the prior
> enabled operations, and adds a Stock dependency edge. The call invalidates
> the toolpath result.

**`set_rest_analysis_config` (`:1141`)**, "rest" appears four times:

> Run the rest-depth detector on this toolpath after it generates. It
> attaches a heatmap grid and the machining regions another toolpath can use
> as a `derived_rest_regions` boundary. `reference_tool_id` names a library
> tool as the reference; unset prefers the machined stock. `cell_mm`,
> `min_valley_depth` and `region_margin_mm` default to 0.5, 0.05 and
> 0.5 mm. `offset_stepover_mm` and `num_offset_passes` tune the fan the
> ROUTING criterion assumes; leave both unset for the canonical reach
> policy. The call invalidates the cached result.

**`get_project_diagnostics` (`:501`)** lists `awaiting_prior_stock` as a
lane column. No rewrite; confirm the key list after item (a).

Descriptions are NOT in the wire snapshot
(`mcp_wire_surface_pin.rs:24-28`). Check
`crates/rs_cam_viz/tests/mcp_authoring_surface.rs` and
`mcp_rebind_surface_g_mcprebind.rs` before you edit one: they pin the
descriptions that carry a contract.

---

## 10. New sentries

### 10.1 `crates/rs_cam_viz/tests/awaiting_prior_stock_has_one_shape_d4.rs`

**Recommend both halves.** Serialisation alone cannot see a seventh
hand-built `json!`; a grep alone cannot see the key count.

Half one, the arity, after
`crates/rs_cam_core/tests/air_cut_denominators_lh1.rs:149-154`: build the
struct, `serde_json::to_value`, assert the three keys, then

```rust
assert_eq!(obj.len(), 3,
  "serialize_struct arity must match the field count: {obj:?}");
```

Half two, the net, after `air_cut_denominators_lh1.rs:243-308`:
`include_str!` the six producer files, strip line comments, assert each
source passes a size floor (a scan that reads nothing passes and looks
healthy), then

```rust
assert!(!src.contains("\"blocking_toolpath_id\""),
  "{rel} hand-builds the awaiting_prior_stock shape. Serialise \
   AwaitingPriorStock instead, so the six surfaces cannot drift (D4).");
```

A Rust field name is not a quoted string, so this fires only on a `json!`
key literal.

The non-vacuity anchor is per file, as a `(file, &[needles])` table like
LH-1's at `air_cut_denominators_lh1.rs:261-288`. One anchor per file does
not fit every file: `generate_all.rs` and the CLI's `project.rs` hold the
row TYPES and never emit the key.

| File | Anchor it must still contain |
|---|---|
| `crates/rs_cam_viz/src/app/mcp/project.rs` | `awaiting_prior_stock` |
| `crates/rs_cam_viz/src/app/mcp/simulation.rs` | `awaiting_prior_stock` |
| `crates/rs_cam_viz/src/controller/events/compute.rs` | `awaiting_prior_stock` |
| `crates/rs_cam_viz/src/mcp_bridge.rs` | `awaiting_prior_stock` |
| `crates/rs_cam_viz/src/controller/generate_all.rs` | `struct BlockedRow` |
| `crates/rs_cam_cli/src/project.rs` | `awaiting_prior_stock: Vec<` |

### 10.2 The CLI sentry for the required resolution

`rs_cam_cli` declares no `[lib]` (`crates/rs_cam_cli/src/command.rs:8-9`),
so an integration test cannot call `run_project_command`. Both existing CLI
sentries are source scans:
`crates/rs_cam_cli/tests/stale_defaults_reach_the_cli_cmp25.rs:39-58` and
`a_failed_collision_check_is_not_a_clean_one_g_colfail.rs:41-53`. Copy that
shape into `crates/rs_cam_cli/tests/resolution_is_not_defaulted_d2.rs`:

- read `../src/main.rs` and `../src/project.rs` with `include_str!`, strip
  line comments, assert a size floor;
- assert `main.rs` contains `resolution: Option<f64>`;
- assert `project.rs` contains `dependencies::edges(`;
- assert the const IDENTIFIER `REST_NEEDS_RESOLUTION` appears in BOTH
  `crates/rs_cam_cli/src/project.rs` and
  `crates/rs_cam_viz/src/controller/events/compute.rs`. The sentence text is
  not in either source once the const owns it, so the identifier is the
  drift proof: two surfaces, one sentence, no paraphrase;
- assert `project.rs` contains `awaiting_prior_stock: Vec<`, so the summary
  type can express a blocked op.

### 10.3 The local loop

```
cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin
cargo test -p rs_cam_viz -q --test mcp_authoring_surface
cargo test -p rs_cam_viz -q --test mcp_core_arm_describes_every_row
cargo test -p rs_cam_viz -q --test mcp_escape_hatches
cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity
cargo test -p rs_cam_viz -q --test awaiting_prior_stock_has_one_shape_d4
cargo test -p rs_cam_cli -q
cargo test -p rs_cam_mcp -q
```

`mcp_core_arm_describes_every_row` and `mcp_escape_hatches` are in the list
because item (b) edits a `CoreRequest` arm in `commands.rs`; both are named
in `crates/rs_cam_viz/src/app/CLAUDE.md`.

Then clippy. Do NOT run a full core gate (operator ruling 2026-09-11). Run
every cargo command through `scripts/cargo_lane.sh`.

---

## 11. Blast radius and risks

| Item | Files | Break | Risk |
|---|---|---|---|
| (a) | core config.rs, 3 viz, 2 tests | generate_all rows; generate_toolpath gains `message` | LOW: the three object sites stay byte-identical |
| (b) | mcp server.rs, viz commands.rs, snapshot | a bad token fails at the schema | LOW: `ZRotationParam` is the precedent |
| (c) | viz project.rs | none, additive | MEDIUM: needs W0's edge API and its tokens |
| (d) | viz mcp_bridge.rs, mcp_server.rs | none, additive | HIGH: needs W1's GUI-thread stamp |
| (e) | viz generate_all.rs, mcp_bridge.rs, 2 tests | `rounds` is gone | LOW |
| (f) | cli main.rs, cli project.rs, core const | `--resolution` required on a rest project | MEDIUM: every existing invocation must gain the flag |
| (g) | core 4, viz 2, cli 1, 2 tests | `stale_defaults` key is gone | MEDIUM: widest reach, least behaviour |

1. **(d) can ship an empty key.** Ship `plan` before W1 stamps it and an
   agent reads "no plan runs" while one runs. Gate (d) on W1.
2. **(f) can break a script.** Check `toolpath_stress_test/agents/` and any
   job runner for a hardcoded `rs_cam_cli project` invocation.
3. **The CLI still ships a program minus its blocked ops.** §6.4.
4. **(g) moves many lines and changes no behaviour.** A reviewer cannot see
   a real change inside it. Put no other edit in that commit.
5. **A description edit can trip an authoring sentry.** Run
   `mcp_authoring_surface` after §9.

**MCP clients.** `.mcp.json` runs `target/release/rs_cam_gui --mcp`, one GUI
per session. Rebuild the release binary BEFORE a live test
(`app/CLAUDE.md`); never trust the binary mtime.
`rg -n "set_stock_source|awaiting_prior_stock|rounds\b" .claude/ CLAUDE.md
crates/*/CLAUDE.md research/` returns nothing: no skill under
`.claude/skills/` (`dev`, `lint-fix`, `refresh-lit-matrix`, `sim-analysis`,
`verify`) passes a `source:` string or reads `rounds`. The breaks in (b) and
(e) reach no checked-in client. A live agent session that learned the old
schema is the only client at risk; the capability tokens in §3 and §4 let it
detect the version.

## 12. Commit order

1. (a) core `Serialize`, the six sites, the D4 sentry.
2. (b) the mirror enum, the parser deletion, the re-blessed snapshot.
3. (e) `rounds` to `steps`.
4. (c) `depends_on` and `stock_source`, after W0.
5. §8 `session::generation_plan`, if the owner moves it to W0.
6. (f) the CLI refusal, the cap deletion, the blocked list, the CLI sentry.
7. (d) `plan`, with or after W1.
8. §9 the description rewrites.
9. (g) the `stale_defaults` rename, alone.
