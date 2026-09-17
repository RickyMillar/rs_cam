# Generate ↔ simulate ↔ rest: one path, one indicator

Status: PLAN, not started. Written 2026-09-18 from a read-only map of core,
viz, MCP and CLI. Operator brief (verbatim points):

- "if I generate all, it errors that some need rest. I'd love to instead just
  have the incremental sims happen on generate all if needed."
- "a simple indicator if one path is dependent on another."
- "you can 'machine rest' but also have 'calculate rest' (looking forward).
  This UX is clunky and I'd rather there was one clear path."
- "the indicator on a toolpath should note if it's generating toolpath, or
  sim. The dependency indicator should show if it's blocked or not (stale)."
- "when you make a change you should see anything dependent go stale."
- "I don't want you to add lots of words. Minimal and graphical."

Ownership: this folder is new. `planning/ui_premium_2026-09-13/` and the card
layout in `crates/rs_cam_viz/src/ui/toolpath_panel.rs` (R24, R32, the
declutter phase) belong to the other account. `feeds/`, `tool_load/`,
`tool/` belong to the power session. Nothing here edits those.

---

## 1. What exists today

Every claim carries a file:line. All paths are under `crates/`.

### 1.1 The dependency model is three edge kinds, declared per toolpath

| Edge kind | Declared by | Needs | Where |
|---|---|---|---|
| **Stock** | `stock_source = FromRemainingStock` | a simulated snapshot of the same-setup prefix | `rs_cam_core/src/compute/config.rs:6-12`, seed at `session/compute.rs:222-227` |
| **Regions** | `boundary.source = DerivedRestRegions { source_toolpath_id }` | the source's generated result with `rest_regions` attached | `compute/config.rs:361-363`, read at `session/compute/generation.rs:572-579` |
| **PrevTool** | `OperationConfig::Rest { prev_tool_id }` | an earlier enabled op in the setup with that tool | `rs_cam_viz/src/ui/properties/operations/validate.rs:254-263` |

The Regions edge has a hidden producer half: the source op must have
`rest_analysis.enabled`. A consumer's boundary pick auto-enables it
(`Command::AutoEnableRestAnalysis`, `session/mutation/config.rs:249-272`),
and the inspector force-flips it during draw
(`ui/properties/toolpath_panel.rs:911-915`).

There is **no pure "what does X depend on" function**. The graph lives inside
the invalidation walker `invalidate_output_dependents_of_set`
(`session/mutation/toolpath.rs:297-360`): rule (a) same-setup downstream
Stock ops, rule (b) Regions consumers, to fixpoint. PrevTool is not a walker
rule at all; it is a GUI validation.

### 1.2 Freshness is derived, in viz, from the core result cache

`FreshnessState` has seven arms (`rs_cam_viz/src/state/freshness.rs:25-44`):
`Current, EditedSince, Regenerating, WaitingOnUpstream(AwaitingPriorStock),
Error, NoResult, Disabled`. It reads `session.get_result(i).is_some()` plus
the compute lane's `ComputeStatus`. A setter returns `Effects.stale` and the
core drops the whole downstream chain. This part works: an edit does stale
its dependents, through the core, today.

Simulation freshness is a **second truth**: `sim.is_stale(gui.edit_counter)`
(`state/simulation/playback_state.rs:168`), a GUI counter, while the core
sets `session.simulation = None` on every chain edit
(`session/mutation/toolpath.rs:307`). Ledger G-FRESHNESSDISAGREE is open on
this.

### 1.3 Generate All already has the ladder, and it refuses to start

`handle_generate_all` (`rs_cam_viz/src/controller/events/toolpath.rs:433-477`):

- no rest ops → a bare submit loop, no `PendingGenerateAll`, no summary;
- rest ops present → `plan_fixpoint(true, …, pinned_simulation_resolution())`.
  `pinned_simulation_resolution` returns `None` whenever "Auto from tool
  size" is ticked (`toolpath.rs:488-492`), the panel's default. The plan
  refuses, a six-second warning toast says to untick the checkbox, and
  **nothing is submitted**. That is the error the operator sees.

When the ladder does run (`controller/events/compute.rs:1633-1768`) it is a
fixpoint: generate every enabled op, simulate the whole project, regenerate
the blocked ones, bounded by rest ops + 1 rounds. It writes
`simulation.resolution` and clears `auto_resolution` permanently, and every
blocked submit pushes its own warning toast (`compute.rs:281-287`), so one
click yields several toasts that the next round resolves anyway. Progress is
a five-second status line (`controller.rs:405-415`).

The MCP tool and the CLI have the same ladder with different refusal rules:
MCP refuses without `simulation_resolution_mm`; the CLI silently defaults
`--resolution 0.5` (`rs_cam_cli/src/main.rs:155-157`).

### 1.4 The card and the inspector

The card is one row, `swatch · dot · name · tool · eye · …`
(`ui/toolpath_panel.rs:172-180`). The dot carries the seven-state colour, the
word on hover, a spinner while generating (`:367-411`). There is no
"simulating" form. A `dep` badge exists only on `OperationConfig::Rest`
cards (`:723-831`), in three colours with words.

The inspector's Geometry tab says "rest" four ways:

1. checkbox **Use remaining stock** (`ui/properties/tab_badges.rs:561-584`),
   hidden for pencil;
2. pencil's pair **Machined stock (requires simulation) / Reference tool**
   (`ui/properties/operations/surface_3d.rs:552-574`), the same field with
   other words, plus a caption about a silent fallback;
3. section **Rest Analysis** with the checkbox **Compute rest heatmap
   (material left after this op)** and three parameters
   (`ui/properties/toolpath_panel.rs:868-935`);
4. boundary source **Rest Regions** with a **Rest source** combo
   (`:702-782`).

1 and 2 are "machine rest". 3 is "calculate rest". 4 is the only reason 3
exists.

---

## 2. Diagnosis

Five causes, in the order the operator meets them.

1. **Generate All refuses on the default resolution setting.** The A/M10 rule
   ("refused, never defaulted") was written for the MCP tool, where a silent
   cell size hands an agent verdicts it did not ask for. In the GUI the
   Simulation panel's setting IS the operator's standing choice: the Run
   Simulation button uses it as it stands. Reading it is not a default.
2. **Blocked is reported as an error, in a toast, several times.** The row
   already knows (WAIT). The toast adds nothing and expires.
3. **The producer side of rest is a user control.** Rest analysis is already
   demand-driven. A checkbox that the system flips during draw is not a
   decision the operator makes; it is a decision they are asked to repeat.
4. **One field, two vocabularies.** `stock_source` is "Use remaining stock"
   on most ops and "Machined stock / Reference tool" on pencil.
5. **Dependency is invisible.** Only Rest 2D cards show an edge. Stock and
   Regions edges show nothing until they block, and then only as a hover.

Plus two defects found on the way (§5).

---

## 3. Design rules for this sweep

- **Declare on the consumer, derive the rest.** An operator says what an op
  starts from or is bounded by. Producer analysis, simulation and ordering
  are the system's job.
- **One truth per state.** Freshness stays derived from the core cache. The
  dependency edges come from one pure function that the walker, the card and
  the wire all read. No stored flag.
- **No new words at rest.** A state is a shape and a colour, with a glyph
  beside every colour (ui_premium §2.6 rule 3). Words live on hover.
- **Blocked is a sequencing state, not an error** (A/M11). It never toasts.
- **Generate means "make this current".** Whatever that needs, the system
  does.

---

## 4. The design

### 4.1 One dependency function (core)

Add `session::dependencies::edges(&ProjectSession) -> Vec<Edge>` with

```
Edge { from: ToolpathId, on: ToolpathId, kind: Stock | Regions | PrevTool }
```

read forward ("X depends on Y"), plus `state(&Edge) -> EdgeState`:

| `EdgeState` | Meaning | Role |
|---|---|---|
| `Ready` | source current; for Stock, the snapshot exists | hairline |
| `Pending` | source not current, or snapshot missing | `CAUTION` |
| `Broken` | source missing, disabled, or first in setup | `DANGER` |

The walker's rules (a) and (b) become reads of this function. PrevTool
joins the walker as rule (c) so a Rest 2D consumer drops when its
predecessor's result drops; today it does not. `list_toolpaths` gains
`depends_on: [{id, kind, state}]` from the same function.

### 4.2 Generate is a plan over the edges

Replace the round-based fixpoint with a plan walk that produces the same
result in the same bound and reports per step:

```
for each setup, in plan order:
  for each enabled op:
    if op has a Stock edge and no snapshot → simulate this setup's prefix
    if op has a Regions/PrevTool edge whose source is not current → the
      source is earlier in the walk, so it is already current here
    generate op
```

- `Generate All` runs the walk over every setup. A single `Generate` on op X
  runs the walk over X's ancestors, then X (R6).
- The walk's simulations use the Simulation panel's effective resolution,
  auto included (R1). The ladder no longer writes `simulation.resolution`
  or `auto_resolution`, and `resolution_override_notice` goes.
- Prefix simulation uses `RunSimulationWith(ids)`
  (`controller/events/simulation.rs:344-370`), setup-scoped, with the S5
  prefix memo. One full-project simulation at the end when every op is
  current, so the Simulation workspace lands fresh.
- The no-rest-ops path goes through the same plan, so one summary exists.
- A blocked submit inside a running plan does not toast. A blocked submit
  outside a plan cannot happen after R6.
- MCP `generate_all` keeps `simulation_resolution_mm` as an explicit
  argument; the CLI stops defaulting `--resolution` (D2).

### 4.3 One rest path in the inspector

Geometry tab, one row for every operation type, pencil included:

```
Start from      ( ) Stock   (•) After previous ops
```

That is `stock_source`. "After previous ops" is the Stock edge. The pencil
pair, its caption, and the generic checkbox go. Pencil's reference-tool
picker stays as its analytic reference, shown only under "Stock"; under
"After previous ops" it is not a choice, and the silent fallback ("falls
back to the reference tool if the simulated stock doesn't overlap") becomes
`EdgeState::Broken` with a hover, not a caption.

Boundary stays as it is: `Rest regions of <op>` is the Regions edge, the one
consumer declaration.

The **Rest Analysis** section loses its checkbox. The three parameters
(cell, min valley depth, region margin) sit behind a disclosure titled by
the consumers, "Rest regions → Lakes", drawn only when the op has a Regions
consumer or the rest-heatmap overlay is on for it. The overlay row
`OpenRestAnalysis` (`ui/overlays/registry.rs:127-130`) becomes a demand
like a consumer: turning the heatmap on for an op enables its analysis, the
same door `AutoEnableRestAnalysis` already is. The draw-time flip at
`toolpath_panel.rs:911-915` goes with the checkbox (D5).

### 4.4 The card

No new element. Two changes to the dot, one gutter line.

```
   swatch dot  name              tool
 ┃   ■   ●    Pin Drill         6.00 mm End Mill
 ┃   ■   ●    Back Rough        6.00 mm End Mill
 ┗━  ■   ◌    Holes             6.00 mm End Mill        ← Stock edge, Pending
 ┃   ■   ●    Rivers (back)     20° V-Bit
 ┗━  ■   ◔    Lakes (back)      20° V-Bit               ← Regions edge, simulating
```

**Dot, in-flight forms.** Generating keeps the `egui::Spinner`. Simulating
draws a hollow ring with a rotating gap (◔): a different shape, the same
size. A row shows the ring while the analysis lane is running a request
that covers it. Derived: lane `Running` plus the request's covered ids,
stamped on `SimulationState` at submit beside `submitted_edit_counter`.
Hover: "Simulating · stock for Holes".

**Gutter connector.** A 6-point gutter left of the swatch. A dependent row
draws a vertical line up to its source row and an elbow into its own row.
Colour is the `EdgeState` role. Hover names the source and the kind
("After Back Rough · waiting for simulation"). Click selects the source.
A source outside the visible list draws a short stub with an up-glyph.
The Rest 2D `dep` badge and its three words fold into this (R3).

**Blocked.** The dot already reads WAIT at `CAUTION`. With the connector
beside it, "blocked or stale" is the connector's colour, and the dot is
the op's own state. No word.

**Generate All button.** While a plan runs, the button is the progress
surface: label "Generating 3/8" or "Simulating · Setup 1", a thin bar along
its bottom edge, click cancels. The five-second status line stops carrying
plan progress. No new panel.

### 4.5 Simulation freshness: one truth

`sim.is_stale()` reads `session.simulation.is_none()` for every project
input, and keeps the GUI counter only for runtime-only capture options
(`metric_options`), which the core cannot see. Every reader listed in the
map (`sim_op_list.rs:208`, `sim_timeline.rs:53`, `status_bar.rs:132`,
`workspace_bar.rs:227`, readiness, preflight, optimize) reads the one
function. This closes G-FRESHNESSDISAGREE.

### 4.6 Wire alignment (MCP, CLI)

Per `rs_cam_cli/CLAUDE.md`, one vocabulary per concept:

- one `awaiting_prior_stock` shape `{blocking_toolpath_id,
  blocking_toolpath_index, message}` on all three surfaces that emit it
  (`app/mcp/project.rs:71`, `controller/events/compute.rs:1955`,
  `mcp_bridge.rs:1494`);
- `set_stock_source.source` becomes the `StockSource` enum,
  `#[schemars(inline)]` (moves the wire snapshot; state the break);
- `list_toolpaths.depends_on` from §4.1;
- `generation_status` carries `plan: {step, of, simulating: bool}`;
- the CLI ladder cap matches (`rest ops + 1`), and an op still blocked at
  the end is listed as blocked, not absent, in `summary.json`.

---

## 5. Defects found on the way

| Id | Defect | Evidence |
|---|---|---|
| D1 | A source that **regenerates** without an input edit leaves its Regions consumers `Current`. `insert_result` touches no consumer (`session/mutation/toolpath.rs:720-730`); `AdoptResult` bumps no revision; viz's `mark_derived_rest_dependents_stale` stamps only the auto-regen clock (`controller/events/compute.rs:381-405`). Sequence: re-simulate, regenerate the source, its regions move, the consumer reads OK. | walker rule (b) fires on edits only |
| D2 | The CLI defaults `--resolution 0.5` while GUI and MCP refuse. | `rs_cam_cli/src/main.rs:155-157, 267-269` |
| D3 | Error text says a Regions source "must be a pencil operation with the rest-depth detector enabled"; since P2.5 any op with rest analysis produces regions. | `session/compute/generation.rs:574-578` |
| D4 | Three JSON shapes for `awaiting_prior_stock`. | §4.6 |
| D5 | The inspector writes `rest_analysis.enabled = true` during draw. | `ui/properties/toolpath_panel.rs:911-915` |
| D6 | `prior_stock_blocker` names the nearest generated upstream op; the phantom scan (`compute/simulate.rs:260-303`) gates on the first ungenerated op. The message can name the wrong op. | audit SYNTHESIS `54a25009` |

D1 fix: `AdoptResult` on an index with Regions or PrevTool consumers drops
those consumers through the walker. One sentry: regenerate the source,
assert the consumer's result is gone.

---

## 6. Work packages

One owner per file. Sentries are red-first. Each package ends green on the
local loop (folder sentries, focused crate tests, clippy).

| WP | Scope | Files (owner set) | Sentry |
|---|---|---|---|
| **W0** | `dependencies::edges` + `EdgeState`; walker reads it; PrevTool joins the walker; D1 fix; D3 text | `rs_cam_core/src/session/dependencies.rs` (new), `session/mutation/toolpath.rs`, `session/command.rs` (AdoptResult arm), `session/compute/generation.rs` | `tests/dependency_edges_are_the_walker_dep1.rs`: every drop the walker makes is an edge, and every edge's drop is made; source regenerate drops consumers |
| **W1** | Plan walk replaces rounds; panel resolution; no toast inside a plan; one summary; single Generate resolves ancestors (R6); Generate All button as progress | `controller/generate_all.rs`, `controller/events/toolpath.rs`, `controller/events/compute.rs` (ladder functions only), `ui/toolpath_panel.rs:42-48` (button only, by handoff) | `controller/tests/generate_all_plan_g_genplan.rs`: wanaka fixture, auto resolution, one call → every enabled op current, N prefix sims, zero toasts |
| **W2** | "Start from" row; pencil pair and captions deleted; Rest Analysis checkbox deleted, parameters behind the consumer-titled disclosure; overlay demand | `ui/properties/tab_badges.rs`, `ui/properties/operations/surface_3d.rs`, `ui/properties/toolpath_panel.rs`, `ui/overlays/panel.rs` | `ui/properties/tests/one_start_from_row_g_startfrom.rs`: the string "rest" appears in at most two controls; no draw-time write to `rest_analysis` |
| **W3** | Simulating ring; gutter connector; `dep` badge folded; covered-ids stamp | `ui/toolpath_panel.rs` (card), `state/simulation.rs` (stamp), `state/freshness.rs` (no new arm; a separate `InFlight` read) | `tests/freshness_surfaces_g_freshrender.rs` extended by handoff; `connector_reads_edge_state_g_connector.rs` |
| **W4** | One simulation freshness function; every reader | `state/simulation/playback_state.rs`, the readers in §4.5 | `sim_stale_is_the_core_answer_g_freshnessdisagree.rs` |
| **W5** | Wire rows §4.6; CLI D2 | `app/mcp/project.rs`, `mcp_bridge.rs`, `rs_cam_mcp/src/server.rs`, `rs_cam_cli/src/main.rs`, `rs_cam_cli/src/project.rs` | `mcp_wire_surface_pin` re-blessed with the break stated; `awaiting_prior_stock_has_one_shape_d4.rs` |

Order: W0 → W1 → W2 ∥ W4 ∥ W5 → W3. W3 last because it lands on the other
account's card and needs the handoff (R5). W1 depends on W0 for the edge
walk; W2 does not.

---

## 7. Measures on wanaka, before and after

Take the fixture at load, every op pending, "Auto from tool size" ticked.

| Measure | Before | Target |
|---|---|---|
| Ops current after one Generate All click | 0 (refused) | all enabled |
| Toasts during one Generate All | 1 refusal, or 1 per blocked op per round | 0 |
| Clicks from load to every op current and simulated | ≥ 1 + 2 per rest link | 1 |
| Words visible on a card at rest | 0 (name, tool) | 0 |
| Inspector controls whose label contains "rest" | 4 | 2 |
| Controls the system flips during draw | 1 | 0 |
| Truths for "simulation stale" | 2 | 1 |

Screenshots before and after, in this folder, at the same view.

---

## 8. Rulings needed

- **R1** The GUI plan simulates at the Simulation panel's effective
  resolution, auto included. Recommended yes: it is the operator's standing
  setting, not a default. MCP keeps its explicit argument.
- **R2** Cross-setup stock. `prior_stock_blocker` says an op first in its
  setup has nothing upstream, but the simulation runs setups sequentially on
  one stock (`compute/simulate.rs:475`), so a flipped Setup 2 rest op could
  start from Setup 1's result. Not decided here; the edge function takes a
  rule when there is one.
- **R3** The Rest 2D `dep` badge folds into the connector. Recommended yes.
- **R4** The rest-analysis checkbox goes; parameters show only under demand.
  Recommended yes.
- **R5** W3 edits the other account's card. Handoff or a joint slot.
- **R6** A single Generate on op X resolves X's ancestors first. Recommended
  yes; it is the same walk.
- **R7** Simulating glyph: hollow ring with a rotating gap. Any distinct
  shape works; a second spinner colour does not (colour is never the only
  channel).
