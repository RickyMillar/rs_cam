# R05 source track — sequencing, remaining stock, multi-tool planning

Read-only source investigation for the R05 review package. Written 2026-09-09.
Every path is repo-relative. Every claim carries a `file:line` that was read.
Items marked **HYPOTHESIS** are inferred from the code and not observed live.

Working-tree note: `crates/rs_cam_viz/src/ui/multitool_planner.rs` (+128/−?) and
`crates/rs_cam_core/src/session/multitool.rs` (+207) carry uncommitted edits by
another developer (`git diff --stat`). Line numbers for those two files are the
working-tree numbers, not HEAD.

---

## 1. Vocabulary map

| Term | GUI location and label text | Problem it solves | Input it consumes |
|---|---|---|---|
| **Rest machining** (`OperationConfig::Rest`) | Operations panel → "+ Add" menu, label `"Rest Machining"`, description `"Clean up areas a larger tool couldn't reach"` (`crates/rs_cam_core/src/compute/catalog.rs:2016-2017`). Form: Geometry tab, `"Previous Tool:"` combo (`crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:352-367`). Card badge `dep` / `no dep` (`crates/rs_cam_viz/src/ui/toolpath_panel.rs:711-770`). | 2.5D clean-up of corners a larger tool left. | An **analytic** previous-tool radius only: `prev_tool_id → t.diameter / 2.0` (`crates/rs_cam_core/src/session/compute.rs:1201-1213`). It does **not** read simulated stock. GUI validation requires an earlier enabled op in the same setup with that tool on the same model (`crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1815-1830, 1878-1896`). |
| **Use remaining stock** (`StockSource::FromRemainingStock`) | Inspector header, above the tab bar, checkbox `"Use remaining stock"`, hover `"When enabled, prior operations in this setup are simulated to determine remaining material. The toolpath will skip air cuts and adapt to the actual stock state."` (`crates/rs_cam_viz/src/ui/properties/mod.rs:3949-3966`). Hidden for Pencil ops, which own it under `"Rest reference:"` (`properties/operations/surface_3d.rs:522-550`). | Seed generation with the dexel stock as the preceding ops left it. | `SimulationResult::prior_stocks[tp_id]`, populated only by a **simulation** (`crates/rs_cam_core/src/session/compute.rs:1440-1458`; GUI: `crates/rs_cam_viz/src/controller/events/compute.rs:560-576`). Refuses rather than falling back to fresh stock. |
| **Rest analysis** (`RestAnalysisConfig`) | Geometry tab, section heading `"Rest Analysis"`, checkbox `"Compute rest heatmap (material left after this op)"` (`properties/mod.rs:4506-4520`); `"Reference:"` combo defaulting to `"Self / machined stock"` (`:4542-4560`). When another op consumes it the checkbox is replaced by `"Producing rest regions for: …"` and force-enabled (`:4522-4541`). | Report-only picture of what this op leaves behind; makes the op a source of rest regions. | The op's own tool (or a chosen reference tool) run through the rest-depth detector after generation (`crates/rs_cam_core/src/session/mod.rs:705-710`). |
| **Derived rest regions** (`BoundarySource::DerivedRestRegions`) | Geometry tab → Machining Boundary → Source → `"Rest Regions"` button, then `"Rest source:"` combo with `"(regions ready)"` suffix (`properties/mod.rs:4303-4394`). Overlays panel row `"Derived rest regions"` refuses: `"not drawn yet (no renderer)"` (`crates/rs_cam_viz/src/ui/overlays/registry.rs:873-883`). | Confine THIS op's cut to the islands another op's rest analysis found. | The source op's cached `rest_regions` (`session/compute.rs:1461-1478`). Consumer is invalidated when the source regenerates or is removed (`controller/events/compute.rs:798-834`). |
| **Tier map** | Menu Toolpath → `"Plan multi-tool finishing…"` → dialog `"Plan multi-tool finishing"` (`crates/rs_cam_viz/src/ui/menu_bar.rs:186-203`; `multitool_planner.rs:58`). Overlays row `"Tier map"`, hover `"One colour per tool tier over the territory that tier owns…"` (`registry.rs:808-846`). | Decide which of several tools should own which part of a 3D surface. | Drop-cutter residual between ladder tools over the mesh at `tolerance_mm` (`registry.rs:194-197` hover). `BoundarySource::PlannedTierRegions` on each emitted tier (`session/multitool.rs:395`). |
| **Reach map** | Inspector checkbox `"Show reach map"` on ops where `supports_reach_map()` (`properties/mod.rs:3690-3692`). Overlays row `"Model colour: Reach"`, hover `"Green where this cutter forms the surface inside the operation's tolerance, red where it cannot…"` (`registry.rs:902-943`). Default ON in Toolpaths workspace. | Can this ONE tool's tip fit the valleys of the surface? | Mesh + selected op's tool + its cusp tolerance (`registry.rs:356`). Not a stock or sequencing concept. |

Distinguishability findings:

- "Rest Machining" (op) and "Use remaining stock" (stock source) are both called
  "rest machining" in operator-facing text: the stock-source refusal says
  `"is set to use remaining stock (rest machining)"` (`session/compute.rs:1449`),
  and the Generate All refusal says `"rest-machining operation(s)"` in the MCP
  arm (`app/mcp.rs`, via `controller/events/compute.rs:1560-1575`). The Rest
  op itself never reads stock. Two concepts share one word.
- "Rest heatmap" and "Rest regions" are both under "Rest Analysis". The
  heatmap is a picture; the regions are a boundary source. The overlay row
  for regions has no renderer (`registry.rs:880`), so the only visible
  evidence of a rest-regions boundary is the clipped toolpath itself.
- The Overlays panel groups Rest heatmap, Tier map, Planner islands and
  Derived rest regions under one `Regions` group, and Reach under `Analysis`
  (`registry.rs:770-945`). Two of the four Regions rows are placeholders that
  refuse when ticked.

---

## 2. Boundary precedence — `boundary_inherit` is inert in generation

Facts read:

- A 3D op added on a mesh gets `boundary.enabled = true`,
  `source = ModelSilhouette` **and** `boundary_inherit = true`
  (`crates/rs_cam_viz/src/controller/events/toolpath.rs:139-155`). The same
  `boundary_inherit: true` is stamped on every other constructor: pin drills
  (`controller/events/model.rs:760`), MCP `add_toolpath` (`app/mcp.rs:3541,
  7070`), core session defaults (`session/mod.rs:2056, 2254, 2385`).
- The Geometry tab shows `"Enable boundary"` then `"Inherit from stock"` with
  hover `"Use the stock-level default boundary. Uncheck to configure a custom
  boundary for this toolpath."` The Source selector is drawn **only when
  `boundary_inherit` is false** (`properties/mod.rs:4246-4258`).
- **No generation code reads `boundary_inherit`.** `rg boundary_inherit` over
  `crates/rs_cam_core/src` and `crates/rs_cam_viz/src` finds only: the struct
  field and its doc (`session/mod.rs:703-704`), project-file serialisation
  (`session/project_file.rs:433, 700`; `session/save.rs:170`;
  `viz/io/project.rs:246, 590, 1203-1206`), constructors, the entry ↔ config
  copy (`properties/mod.rs:3421, 3461`) and the one UI branch at
  `properties/mod.rs:4252-4257`. There is no `if tc.boundary_inherit` anywhere
  in `session/compute.rs`, `compute/worker/execute/mod.rs` or
  `controller/events/compute.rs`.
- There is no "stock-level default boundary" object to inherit from. `rg
  default_boundary|stock_boundary` over core and viz returns nothing but the
  two doc comments (`session/mod.rs:703`, `project_file.rs:431`).
- What generation uses is `tc.boundary` verbatim: `let boundary_config =
  tc.boundary.clone();` (`session/compute.rs:1107`), then
  `resolve_containment_polygon(&boundary_config, …)` for Stock /
  ModelSilhouette / FaceSelection (`:1366-1373`) and
  `apply_boundary_clip[_multi]` after dressups (`:1671-1750`). The GUI worker
  path does the same off `req.boundary` (`compute/worker/execute/mod.rs:112-175,
  675-796`), where `req.boundary = tc.boundary.clone()`
  (`controller/events/compute.rs:194`).

Answer: **the stored source wins, always.** With
`boundary.enabled = true, source = ModelSilhouette, boundary_inherit = true`
the op is clipped to the model silhouette. The checkbox labelled "Inherit from
stock" changes nothing except hiding the Source selector. An operator who reads
"Inherit from stock ✓" as "clip to the stock rectangle" is wrong; the real clip
is the silhouette. An operator who unticks it sees "Model Silhouette" already
selected, which is the first moment the truth is visible.

Corollary the planner relies on: the planner sets `boundary_inherit: false` with
the comment "letting the stock default overwrite it would silently un-confine
the fine tier" (`session/multitool.rs:392-395`). That comment describes a
mechanism that does not exist; the flag is set defensively against a rule that
no code enforces.

Correctness classification: this is a **UX-honesty defect, not a
generated-path defect**. The emitted toolpath matches the stored source. The
label misdescribes it.

---

## 3. Operator-visible dependency graph (Operations panel)

Structure, `crates/rs_cam_viz/src/ui/toolpath_panel.rs`:

- Heading `"Operations"`, then a single `"Generate All"` button (`:38-46`).
- Setup headers only when the project has more than one setup: setup name in
  bold, a `"{ready}/{total}"` count where ready means `ComputeStatus::Done`,
  and a per-setup `+ Add` menu (`:61-93`). With one setup there is no header
  and `+ Add` sits below the list (`:166-170`).
- Each setup is an `egui` drop zone. Dropping a card in the same setup emits
  `ReorderToolpath(id, drop_idx)`; in another setup emits
  `MoveToolpathToSetup(id, setup_id, drop_idx)` (`:95-163`). Drop index is a
  Y-position heuristic (`:630-645`). The drag handle is a 10×14 px grip on row
  1 (`:364-388`). Hover controls also offer `"Move Up"` / `"Move Down"`
  (`:598-605`).
- Empty setup renders `"No toolpaths"` (`:101`).

Row status chip, one taxonomy shared with MCP (`ComputeStatus::effective`,
`crates/rs_cam_core/src/compute/config.rs:82-86`), rendered at
`toolpath_panel.rs:398-418`:

| Status | Chip | Colour | Hover |
|---|---|---|---|
| Pending | `PEND` | dim | — |
| Computing | `GEN` | warning | — |
| Done | `OK` | success | — |
| AwaitingPriorStock | `WAIT` | warning (amber, "not a failure") | `block.message` |
| Disabled (derived, never stored) | `OFF` | faint | — |
| Error | `ERR` | error | error text |

Plus `MAN` when auto-regen is off (`:420-429`), and a `TRACE` badge only when
trace availability differs across cards (`:431-446`).

The `WAIT` message (`crates/rs_cam_viz/src/controller/events/compute.rs:76-167`)
names the nearest **enabled** upstream op in the same setup and says whether one
simulation is enough:

- blocker generated: `"'X' is waiting on simulated stock after 'Y' (index N).
  That operation is generated, so ONE simulation is enough: run a simulation,
  then regenerate. (Not falling back to fresh stock.)"`
- blocker not generated: `"… which has not generated yet. The cycle may need
  repeating: generate 'Y', run a simulation, then regenerate this operation —
  and if IT feeds a further rest op, again. \`generate_all\` with a simulation
  resolution does the whole ladder in one call. …"`
- no enabled predecessor: `"… is the first enabled operation in its setup —
  there is no prior operation to leave any stock behind. Set its stock source
  to fresh stock, or move it after the operation it is meant to follow."`

Note the middle message tells a GUI operator to call `generate_all`, an MCP
tool name, and the block is also pushed as a 6 s toast (`compute.rs:572-575`;
ttl `controller.rs:70-76`).

Inspector: the same status appears above the tabs as `"Waiting on upstream
stock"` in amber with the message on hover (`properties/mod.rs:3641-3652`).
Nowhere is the blocker name printed without hovering.

The `dep` badge (`toolpath_panel.rs:711-770`) is drawn **only for
`OperationConfig::Rest`** cards. It checks whether any other toolpath in the
same setup uses `prev_tool_id` (by **tool id**, not by toolpath, and without
checking plan order), and colours: green `dep` resolved, yellow `dep` when that
op needs generation or is stale, red `no dep` when missing or not configured.
It has no hover text and names no operation.

There is **no badge, arrow or line for `FromRemainingStock` consumers**. The
only sequencing signals for the stock chain are the `WAIT` chip after a failed
submit and the "Use remaining stock" checkbox inside the inspector. Row order
is the dependency: `prior_stock_blocker` picks the nearest enabled toolpath
above (`compute.rs:99-119`), and the core invalidation closure walks
`setup.toolpath_indices` in plan order (`session/mutation.rs:229-255`). Nothing
on the card says "this op reads what the op above leaves".

---

## 4. Generate All prerequisite (GUI vs MCP)

GUI, `crates/rs_cam_viz/src/controller/events/toolpath.rs:345-412`:

1. `generate_all_scope` collects every enabled id and the indices of enabled
   `FromRemainingStock` ops (`controller/generate_all.rs:27-43`).
2. No rest ops → every config is submitted once, no notification (`:352-362`).
3. Rest ops present → `plan_fixpoint(true, rest_ops, pinned_resolution())`.
   `pinned_simulation_resolution` returns `None` whenever
   `state.simulation.auto_resolution` is true (`:401-412`).
4. On `None` the GUI pushes one **Warning toast** (6 s ttl) and returns:

   > "Generate All needs a pinned simulation resolution: N enabled
   > operation(s) take their stock from a simulation, so the ladder has to
   > simulate between generate rounds. Untick "Auto from tool size" in the
   > Simulation panel and set a resolution well below the finishing tool's
   > TIP radius (e.g. 0.1 mm for a 1 mm ball). It is not guessed — collision
   > counts and engagement both move with cell size."

   (`toolpath.rs:377-392`). It is a toast in the bottom-right
   (`app.rs:892-906`), not a dialog, not inline on the button, and it does
   **not** switch workspace or focus the control.

Where the control lives: Simulation workspace (heading `"Verification"`), left
panel, inside a collapsible `"Setup & run"` header that is **open only until
the first simulation has run** (`ui/sim_op_list.rs:26-49`). Inside:
`"Resolution:"` showing `"{:.3} mm (auto)"` or a 0.02–1.0 mm log slider, then
the checkbox `"Auto from tool size"` and a `"(re-run to apply)"` warning when
unticked (`:73-96`). So after any prior sim the operator must (a) read the
toast within 6 s, (b) switch workspace via menu or tab, (c) expand a collapsed
header, (d) untick, (e) go back and press Generate All again.

Successful path: status line `"Generate All: round 1, N operation(s)..."`
(`controller/events/compute.rs:1627-1630`), final toast from
`generate_all_headline` (`generate_all.rs:270-291`).

MCP, `controller/events/compute.rs:1537-1612` (called from `app/mcp.rs:5022`):
refusal is a JSON `{"ok": false, "error": …}` naming the rest op **indices**,
explaining that the resolution is not guessed, and offering `fixpoint: false`
for the single-pass behaviour. Structurally identical decision
(`plan_fixpoint` is shared), different wording, and the MCP arm additionally
force-enables `debug_options.enabled` on every enabled toolpath
(`:1591-1598`) — the GUI arm does not.

Parity test `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs:197-222`
asserts the GUI refusal reaches `active_notifications()`, contains
`"resolution"`, contains `"Auto from tool size"` and contains the count. It does
not assert where the operator is sent.

---

## 5. Fixpoint states

Driver: `controller/events/compute.rs:1616-1800`.

- Round 1: status line `"Generate All: round 1, N operation(s)..."`; every
  enabled id is submitted (`:1627-1641`).
- Each blocked submit lands as `WAIT` on its card and a 6 s toast per op
  (`:560-576`).
- When `remaining` drains: advance only if `fixpoint.enabled`, `blocked` is
  non-empty, `completed_this_round > 0` and `round < max_rounds`
  (`max_rounds = rest_ops + 1`, `generate_all.rs:237-247`). Then it **writes
  the operator's simulation settings**: `state.simulation.resolution =
  resolution; auto_resolution = false` (`:1690-1691`) and runs a full sim with
  status `"Simulating so the blocked rest operations can see their stock..."`
  (`:1692-1695`).
- After the sim: `"Round {k}: regenerating N operation(s) that were waiting on
  upstream stock..."` (`:1765-1769`), blocked ids re-submitted.
- Finish: toast `"Generated N toolpaths[, M failed][, K still waiting on
  upstream simulated stock] (in R generate round(s), S simulation(s))[. The
  fixpoint loop stopped early: …]"` (`generate_all.rs:270-291`), Info unless
  failures or loop error → Warning (`compute.rs:1714-1722`).
- Sim failure disables the loop and records `loop_error` (`:1745-1750`).

Only the 5 s status line (`controller.rs:288-297`) and per-card chips carry the
intermediate state. There is no round counter widget, no progress bar and no
persistent log.

`generate_all_fixpoint_parity.rs` asserts (lines 39-93, 169-247):

- `plan_fixpoint` with k rest ops and a resolution → enabled, `max_rounds =
  k+1`, `round = 1`, `simulations = 0`.
- no rest ops → single pass, no resolution needed.
- `fixpoint = false` → single pass even with a chain.
- chain without resolution → refusal names the blocking indices.
- GUI arms the ladder when pinned (`awaiting_generate_all()` true, a deferred
  completion is owed, no "resolution" toast).
- GUI refuses when `auto_resolution` is true (see §4).
- GUI without a chain stays single pass with no notification.
- (mcp feature) `plan_multitool_finishing` and `preview_tier_map` are
  registered with their dials (`:250-373`).

It asserts the **decision**, not the loop: it never drives a round to
completion or checks any status-line text.

---

## 6. Break-and-repair

Core side (`crates/rs_cam_core/src/session/mutation.rs`):

- `invalidate_output_dependents(index, stock_chain_changed)` drops the cached
  core result of every later same-setup enabled `FromRemainingStock` op and
  every enabled `DerivedRestRegions` consumer, transitively, and sets
  `self.simulation = None` (`:176-291`).
- `set_toolpath_enabled` calls it with `true` on any flip (`:302-318`).
- `reorder_toolpath` swaps the two `toolpath_indices` entries and calls it for
  both indices (`:137-174`).
- `move_toolpath_to_setup` calls it in the source setup, then **appends** the
  op at the end of the target setup and drops its own result (`:703-734`).
  The `drop_idx` computed by the panel is passed as `_idx` and ignored
  (`controller/events/toolpath.rs:294-320`).
- `remove_toolpath` calls it, then re-keys results (`:92-129`).
- `generate_toolpath` on a `FromRemainingStock` op with no
  `prior_stocks[tc.id]` returns `OperationFailed` with the "refusing to fall
  back to fresh stock" text (`session/compute.rs:1440-1458`). The GUI submit
  path records `AwaitingPriorStock` instead (`controller/events/compute.rs:
  560-576`).

GUI side — what the operator actually sees after each break:

| Break | Core | GUI runtime (`toolpath_rt`) and GUI sim results | Visible signal |
|---|---|---|---|
| Disable predecessor (`ToggleToolpathEnabled`) | downstream core results dropped, core sim dropped | **nothing touched**: handler is `set_toolpath_enabled` only (`controller/events/mod.rs:114-118`); no `stale_since`, no `invalidate_simulation` | predecessor chip `OFF`; consumer still `OK` |
| Reorder (drag, Move Up/Down) | as above | **nothing touched** (`toolpath.rs:236-292`: `reorder_toolpath` + `mark_edited`) | cards swap; consumer still `OK`; readiness "simulation stale" via `edit_counter` (`ui/readiness.rs:81-90`) |
| Move to another setup | as above + append at end | `pending_upload` only (`toolpath.rs:294-320`) | op appears at the bottom of the other setup; still `OK` |
| Delete predecessor | as above | `toolpath_rt.remove(id)`; `mark_derived_rest_dependents_stale(id)` marks **only DerivedRestRegions consumers** stale (`toolpath.rs:322-343`; `compute.rs:810-834`) | rest-regions consumer goes stale; a FromRemainingStock consumer stays `OK` |
| Duplicate | new config, `stock_source` copied, `planner_origin: None` | new runtime entry, `PEND` | `"X (copy)"` appended at end of setup (`toolpath.rs:174-234`) |

Consequences (all **HYPOTHESIS** — read from code, not observed live):

1. **Stale-snapshot regeneration.** `state.simulation.results.prior_stocks`
   is keyed by toolpath id and survives every mutation above (nothing calls
   `invalidate_simulation`; `controller/events/simulation.rs:22-31` lists its
   callers as reset + undo only). A consumer regenerated after a reorder or a
   predecessor disable reads `prior_stock_for(tp_id)` (`compute.rs:560-565`)
   and gets the **pre-edit** snapshot. The core comment "or the predecessor
   changed since the last sim" (`session/compute.rs:1436`) describes intent;
   the lookup is by id with no freshness check (`rg is_stale` in
   `controller/events/compute.rs` returns nothing).
2. **Export falls back to the pre-edit toolpath.** `io/export.rs:118-140`
   filters on `tc.enabled`, then reads `session.get_result(idx)` and **falls
   back to `gui.toolpath_rt[id].result`** when the core slot is empty. After
   a reorder the core slot for the downstream rest op is gone but the GUI
   slot is intact, so the export emits a toolpath generated against a stock
   sequence that no longer exists. The card reads `OK`. The doc at
   `export.rs:102-113` justifies the fallback for tool edits and param
   snapshots only.
3. The `dep` badge is order-blind (§3), so a Rest op dragged **above** its
   predecessor keeps a green `dep` while GUI validation (`operations/mod.rs:
   1815-1830`) blocks Generate with `"Rest machining requires an earlier
   enabled operation …"` in the inspector.

Undo does not cover any of it: `UndoAction` has StockChange, PostChange,
ToolChange, ToolpathParamChange, MachineChange (`state/history.rs:16-38`;
`controller/events/undo.rs`). Reorder, enable, move, delete, duplicate and
planner Apply are not undoable.

---

## 7. Planner path

Entry: menu Toolpath → `"Plan multi-tool finishing…"`, enabled when ≥2 tools,
a mesh model, and not optimizing; disabled hover `"Needs a 3D model and at
least two tools in the drawer — the tier map is a drop-cutter residual between
two cutters over a surface."` (`ui/menu_bar.rs:181-203`). Also the MCP
`plan_multitool_finishing` and `preview_tier_map` tools (`mcp_server.rs:1080`).

Dialog `"Plan multi-tool finishing"` (`ui/multitool_planner.rs`, working tree):

- Header `"Model: {name}   ·   setup #{index}"` where index is the **0-based**
  `planner.setup_index` (`:95-99`). The Tier map overlay refusal prints the
  same setup as `setup_index + 1` (`registry.rs:836-839`). Two numberings for
  one setup.
- Setup choice is the setup of the **selected toolpath**, else setup 0
  (`controller/events/planner.rs:318-331`), not the active-setup rule
  `AppState::active_setup_index` (`state/mod.rs:261-271`), which also honours
  a selected Setup/Fixture/KeepOut. Nothing in the dialog lets the operator
  pick the setup.
- `LADDER`: `"Tick the tools that take part. The chain runs coarse to fine,
  and the tip radius below is what decides that order — never the shank."`;
  columns use / tool / tip radius / strategy (`Unified (bands)`, `Scallop
  (rings)`, `Iso Scallop`) (`:134-210`). Nothing is pre-ticked
  (`planner.rs:262-264`).
- `REGIONS`: coarseness slider `many small … few large` with `"{:.2}x"`;
  `Overlap`, `Cusp height`, `Tolerance`; checkboxes `"Coarse tool skips fine
  islands"`, `"Split shallow regions into monotone cells"` (`:216-335`).
- `Advanced` (collapsed): `Planning cell`, `Margin`, optional `Merge radius
  (mm)`, `Min island (mm2)`, `Max islands / tier`, `Rim erosion` (`:337-470`).
- Status: idle `"Press Preview to see which tool claims which part of the
  surface. Nothing is generated and nothing is changed until you press
  Apply."`; loading `"Walking the surface…"` with a Close-cancels note;
  `"Preview failed"`; ready → `TERRITORY` table (`:511-557`).
- `TERRITORY`: `"map: cached"` / `"map: rebuilding on next Preview"`; table
  tier / tool / tip R / islands / raw / owned area / machines; cap warnings;
  band advisories; empty-plan note `"No fine tier kept an island …"`
  (`:559-636`). Island dials auto re-preview after a debounce (`:76`).
- Actions: `Preview`, `Apply plan` (disabled hover `"Preview first — applying
  a ladder nobody has looked at is what the preview exists to prevent."`),
  `Close` (hover `"Leaves the project untouched. Your ladder and dials are kept
  for next time."`) (`:801-838`).

**Nothing in the dialog says what Apply will add or replace.** The only
replacement notice is after the fact: toast `"Planned {names}"` with suffix
`" (replaced N prior planner op(s))"` (`controller/events/planner.rs:213-228`).
The dialog closes; the tier overlay stays up (`:192-194`).

Core apply (`crates/rs_cam_core/src/session/multitool.rs`, working tree):

- **Re-plan is REPLACE, not accumulate**: `remove_planned_toolpaths(setup)`
  removes **every** op in the target setup whose `planner_origin.is_some()`,
  whatever its `plan_id` (`:62-69`, `:704-740`). Hand-authored ops
  (`planner_origin: None`) are never touched.
- Emitted per tier: name `"Finish tier {tier}{" scallop"|" iso"|""} (R{cusp:.1})"`,
  `enabled: true`, `boundary_inherit: false`,
  `boundary = PlannedTierRegions{…}`, **`stock_source: FromRemainingStock` for
  every tier including tier 0**, `heights.bottom_z = Manual(mesh.min.z −
  0.2)`, feeds via `suggest_feeds_for`, `planner_origin = Some{plan_id, tier,
  tier_count}` (`:333-414`). Ops are appended to the setup (`add_toolpath`).
- `plan_id` is monotonic (`:695-702`).

Ownership fields: `PlannerOrigin { plan_id, tier, tier_count }`
(`session/mod.rs:665-685`). **No GUI surface reads it**: `rg planner_origin`
in `crates/rs_cam_viz/src` outside tests finds only project IO, the entry copy
and MCP `list_toolpaths` `"tier"` (`app/mcp.rs:4331`). No card badge, no
inspector lock, no "planner-owned" label.

Planner-owned vs editable:

| Field | On a planner tier | Editable? | Survives re-apply? |
|---|---|---|---|
| name, tool, heights, operation params, feeds, dressups, boundary, stock_source, enabled | set by planner | fully editable in the inspector (`write_entry_config_to_session`, `properties/mod.rs:3444-3470`) | **No.** Re-apply deletes the op by `planner_origin` and emits a fresh one. |
| `planner_origin` | set | not shown, not editable; `write_entry_config_to_session` does not touch it | Preserved through hand edits, which is exactly why a hand-edited tier is still deleted on re-apply. |
| Duplicate of a tier | `planner_origin: None` (`toolpath.rs:207`) | editable | **Yes** — a duplicate escapes ownership. |
| MCP `set_toolpath_param` on a tier | keeps `planner_origin` | — | No. |

Cancel: `Close` cancels the Optimize lane walk, hides the overlay, touches no
toolpath (`planner.rs:240-262`). Undo: Apply is **not** an `UndoAction`; the
only reversal is re-plan or manual delete.

Reconciliation on apply (`planner.rs:48-93`): removes runtime rows for
replaced ids, marks their `DerivedRestRegions` consumers stale, clears
selection/isolation, inserts fresh runtime rows. Emitted ops land `PEND` and
are **not generated**; the doc says the operator or the fixpoint ladder
generates them. Generate All on a fresh plan therefore always hits the §4
prerequisite because every tier is `FromRemainingStock`.

---

## 8. Enabled / hidden / export

- `enabled` toggles: card hover-row glyph with hover `"Disable this toolpath
  (excluded from generation, simulation and output)."`
  (`ui/toolpath_row_controls.rs:150-153`); card button `"Disable"`/`"Enable"`
  (`toolpath_panel.rs:588-591`); MCP `set_toolpath_enabled`. Handler is
  `set_toolpath_enabled` only (`controller/events/mod.rs:114-118`).
- Dependents: core drops downstream **core** results (`mutation.rs:302-318`);
  GUI runtime is **not** marked stale and the GUI simulation is **not**
  invalidated (§6). Cards below still read `OK`.
- Hidden (`visible`): eye glyph hover `"Hide this entire toolpath in the 3D
  viewport. Simulation still includes it."` (`toolpath_row_controls.rs:34-37`).
  Visibility is GUI runtime only (`controller/events/mod.rs:121-125`).
- Export: `emitted_toolpaths` filters on `tc.enabled` **only**
  (`io/export.rs:126-128`); `visible` is not consulted. A hidden op exports.
  A disabled op does not export, and cannot be generated by Generate All
  (`generate_all_scope` filters enabled; core `generate_all` skips disabled,
  `session/compute.rs:2392-2416`).
- Simulation: GUI group builder includes `tc.enabled` results only
  (`controller/events/simulation.rs:309, 352`). Core `run_simulation` does
  **not** filter on `tc.enabled` when building entries (`session/compute.rs:
  2456-2530`) — it relies on the enable toggle having dropped downstream
  results, but the toggled op's own result is deliberately kept
  (`mutation.rs:219-223`). **HYPOTHESIS**: CLI/core simulation of a project
  with a disabled op that still has a cached result stamps it anyway. Not the
  GUI path.
- The setup header `"{ready}/{total}"` counts `Done` over **all** cards,
  disabled included (`toolpath_panel.rs:71-84`), so a setup with one disabled
  op can never show n/n.

---

## 9. Expert shortcuts to preserve, and live-only questions

Shortcuts worth keeping:

- One shared status taxonomy across card, inspector, MCP and CLI
  (`config.rs:52-73`), with `WAIT` amber and never counted as an error.
- The `WAIT` message names the blocker and says whether one simulation
  suffices (`compute.rs:76-167`).
- Refusal instead of silent fresh-stock fallback, on both core and GUI paths.
- Refusal instead of a guessed simulation cell, with the control named
  (`toolpath.rs:377-392`).
- Fixpoint ladder shared by GUI and MCP (`start_generate_all`), bounded by
  `rest_ops + 1` rounds.
- Rest-analysis auto-enable on the source when a consumer picks it
  (`properties/mod.rs:4522-4541`), and the `"(regions ready)"` suffix.
- Planner: nothing pre-ticked, Apply gated on a Preview, Close cancels the
  walk, island dials re-cut the cached map with a debounce, cap warnings are
  loud rows, `planner_origin` keeps hand-authored ops out of the replace set.
- Drag-and-drop reorder plus Move Up/Down plus context menu all feed one
  `reorder_toolpath`.
- `Sim` button on a card jumps to the Simulation workspace for that op.

Questions only a live test can answer:

1. After reorder or predecessor disable, does the downstream `OK` op
   regenerate from the stale `prior_stocks` snapshot, and does export emit
   the pre-edit toolpath (§6 items 1–2)? Compare G-code before and after.
2. Is the 6 s refusal toast readable and is "Auto from tool size" findable
   once `"Setup & run"` has collapsed after a first sim run?
3. During a ladder, is the 5 s status line the only visible progress, and do
   the per-op `WAIT` toasts stack on a 3-tier plan?
4. Does the operator understand "Inherit from stock ✓" as "silhouette"
   without unticking it (§2)?
5. Does anyone read the `dep` badge as a stock dependency, and does its green
   survive a wrong order?
6. Does the planner dialog's `setup #0` versus the overlay's `setup 1` cause
   a wrong-setup apply when a toolpath in another setup is selected?
7. After hand-editing a tier and re-applying, does the operator expect the
   edit to survive? The code deletes it silently apart from the
   `"(replaced N prior planner op(s))"` toast suffix.
8. Do "Rest Machining", "Use remaining stock" and "Rest Analysis" read as
   three problems or one?
9. Does drag-to-another-setup landing at the **end** (ignored drop index)
   surprise the operator?
10. Does the `MAN` badge on planner tiers (3D ops default manual regen) leave
    a freshly applied plan looking idle?
