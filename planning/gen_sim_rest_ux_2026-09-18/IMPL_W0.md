# W0 implementation brief: one dependency function in core

Package W0 of `PLAN.md` (§4.1, §5 D1 and D3, §6). Written 2026-09-18 from a
read-only map. No cargo was run. Paths under `crates/rs_cam_core/src` unless
stated.

Owner set, the only files W0 changes: `session/dependencies.rs` (new),
`session/mod.rs` (one `pub mod`, one re-export), `session/mutation/toolpath.rs`,
`session/command.rs` (the `AdoptResult` arm), `session/compute/generation.rs`
(D3 text), `tests/dependency_edges_are_the_walker_dep1.rs` (new),
`session/CLAUDE.md`. Out of scope: `feeds/`, `tool_load/`, `tool/`, all viz,
all MCP, the CLI.

---

## 1. Module placement and public surface

`session/dependencies.rs`, declared beside the other children at
`session/mod.rs:14-24` as `pub mod dependencies;`, with
`pub use dependencies::{Edge, EdgeKind, EdgeState};` beside the
`diagnostics_types` re-export at `session/mod.rs:57-61`. The module is pure:
it takes `&ProjectSession`, returns values, mutates nothing, names no GUI type.

```rust
pub enum EdgeKind { Stock, Regions, PrevTool }
pub enum EdgeState { Ready, Pending, Broken }

pub struct Edge {
    pub from: ToolpathId,        // the consumer; it declares the edge
    pub on: Option<ToolpathId>,  // None = the declaration resolves to no
                                 // toolpath; the state is then Broken
    pub kind: EdgeKind,
}

pub fn edges(session: &ProjectSession) -> Vec<Edge>;
pub fn state(edge: &Edge, session: &ProjectSession) -> EdgeState;
/// One row per (consumer, kind): the edge the card connector draws.
/// `edges` holds many Stock edges per consumer (§2); this keeps the
/// nearest enabled source, or the `on: None` row when none is enabled.
pub fn primary_edges(session: &ProjectSession) -> Vec<Edge>;
```

`edges` is the walker's input. `primary_edges` is what the W3 card and the W5
`list_toolpaths.depends_on` read, so no surface re-derives an edge.

Thin wrappers: in W0 only the walker's rules become reads of `edges`.
`rest_predecessors` (`rs_cam_viz/src/state/rest_dependency.rs:68-82`) stays in
viz for the whole of W0, so no viz file changes; W0 COPIES that rule into
`dependencies.rs` with a doc line naming the twin, and W3 deletes the viz
module and re-points its two callers (`ui/toolpath_panel.rs:770`,
`ui/properties/operations/validate.rs:261`). The viz rule uses viz newtypes;
core reads `tool_id: usize` and `model_id: usize` (`session/mod.rs:881-882`)
and `RestConfig::prev_tool_id: Option<ToolId>`
(`compute/operation_configs.rs:474`), so the core copy compares
`tc.tool_id == prev.0` and `tc.model_id == consumer.model_id`.

---

## 2. Edge derivation

**Stock.** Declared by `stock_source == StockSource::FromRemainingStock`
(`compute/config.rs:6-12`). Recommendation: **MANY edges, one per upstream
index in the same setup, with NO `enabled` filter on the source.** Two proofs,
both against rule (a) at `session/mutation/toolpath.rs:322-343`.

1. Nearest-only changes the drop set. Plan order
   `[0 Fresh, 1 Fresh, 2 FromRemainingStock]`, edit index 0. Today
   `upstream_changed` latches at index 0 and index 2 drops. Nearest-only puts
   index 2's edge on index 1, which is clean, so index 2 survives. Rule (a) is
   "any upstream change dirties"; only a full upstream set states that.
2. An `enabled` filter on the source changes the drop set.
   `set_toolpath_enabled` writes `tc.enabled = false` at
   `mutation/toolpath.rs:401`, THEN calls the walker at `:404` with
   `stock_chain_changed = true`, so `chain_dirty` carries a disabled index.
   `mutation/tests.rs:267` and
   `tests/adopt_result_rejects_stale_completion.rs:402-430` pin that drop.
   `remove_toolpath` (`:90`), `reorder_toolpath` (`:191-192`) and
   `move_toolpath_to_setup` (`:480`) also pass `true` unfiltered.

The doc at `mutation/toolpath.rs:288-290` says `chain_dirty` "must hold only
ENABLED seeds". Four callers contradict it. Record that; do not change the
callers in W0. A consumer with no upstream index in its setup gets one edge
with `on: None`, which is
`AwaitingPriorStock { blocking_toolpath_id: None }` (`compute/config.rs:32-40`)
and the third message of `prior_stock_blocker`
(`rs_cam_viz/src/controller/events/compute.rs:120-129`).

The connector draws one line, so `primary_edges` keeps the nearest ENABLED
source, matching `prior_stock_blocker`'s pick (`compute.rs:104-118`). D6 says
that pick can disagree with `PhantomPriorStockScan`
(`compute/simulate.rs:266-289`), which locks on the FIRST enabled ungenerated
op. W0 does not resolve D6; it makes both readable from one place.

**Regions.** Declared by `boundary.enabled` plus
`BoundarySource::DerivedRestRegions { source_toolpath_id }`
(`compute/config.rs:361-363`). One edge. `on` is `Some(id)` when a toolpath
with that id exists, else `None`. The source is explicit.

**PrevTool.** Declared by `OperationConfig::Rest(cfg)`
(`compute/catalog.rs:746`). One edge per consumer, on the LAST predecessor or
`None`. `cfg.prev_tool_id == None` gives `on: None`; otherwise the predecessor
is an EARLIER toolpath in the SAME setup that is ENABLED, carries
`model_id == consumer.model_id` and `tool_id == prev_tool_id.0`. That is
`rest_predecessors` (`rs_cam_viz/src/state/rest_dependency.rs:68-82`); the core
diagnostic at `diagnostics/adapters/from_preconditions.rs:194-202` is the same
rule minus the model test. Use the viz rule: it is stricter, and the card reads
it. ONE edge, not many as for Stock: the connector draws the nearest line, the
generator consumes no predecessor result at all (§8 risk 1), and rule (c) is
already an over-approximation, so one edge is the minimal honest set.

PrevTool IS `enabled`-filtered and Stock is not, on purpose. A Stock edge asks
"did the material above me move", and a just-disabled op moves it. A PrevTool
edge asks "does a predecessor with that cutter exist", and a disabled op is not
one. State this, or a reviewer aligns them and breaks proof 2.

---

## 3. `EdgeState` predicates

```rust
pub fn state(edge: &Edge, session: &ProjectSession) -> EdgeState {
    let Some(src_id) = edge.on else { return EdgeState::Broken };
    let Some((src_idx, src)) = session.find_toolpath_config_by_id(src_id)
    else { return EdgeState::Broken };
    if !src.enabled { return EdgeState::Broken }
    match edge.kind {
        EdgeKind::Stock => {
            // R2, cross-setup stock: ONE LINE. Replace when ruled.
            if session.setup_of_toolpath_id(src_id)
                != session.setup_of_toolpath_id(edge.from) {
                return EdgeState::Broken;
            }
            let snapshot = session.simulation_result()
                .is_some_and(|s| s.prior_stocks.contains_key(&edge.from));
            if snapshot && session.get_result(src_idx).is_some() {
                EdgeState::Ready
            } else { EdgeState::Pending }
        }
        EdgeKind::Regions => match session.get_result(src_idx) {
            None => EdgeState::Pending,
            Some(r) => match r.annotated().rest_regions.as_deref() {
                Some(rs) if !rs.is_empty() => EdgeState::Ready,
                _ => EdgeState::Broken,
            },
        },
        EdgeKind::PrevTool => if session.get_result(src_idx).is_some() {
            EdgeState::Ready
        } else { EdgeState::Pending },
    }
}
```

1. **The snapshot key is the CONSUMER.** `prior_stocks` holds the stock before
   a toolpath carves (`compute/simulate.rs:514-517`), and the generator looks
   itself up: `sim.prior_stocks.get(&tc.id)` at `session/compute.rs:2094` and
   `:2145`, where `tc` is the op being generated. The predicate reads
   `edge.from`, never `edge.on`.
2. **Regions state comes from the result payload, not from
   `rest_analysis.enabled`.** Two producers attach `rest_regions`: the generic
   analysis (`compute/execute/dressup_apply.rs:129`) and the pencil `RestDepth`
   detector (`compute/execute/finish_3d.rs:270,585`). A config read would call
   a pencil source Broken. The payload read reproduces the generation refusal
   exactly (`session/compute/generation.rs:565-580`).
3. **R2 is one line.** `PLAN.md` §8 leaves it open, and
   `compute/simulate.rs:475` runs setups sequentially on one stock, so a ruling
   may widen it.

PrevTool `Ready` asks only "has the predecessor generated". The generator
derives `prev_tool_radius` from the TOOL table
(`session/compute/generation.rs:294-303`), so nothing finer exists to read.
See §8 risk 1.

---

## 4. The walker rewrite

### 4.1 Split the simulation clear out first

`invalidate_output_dependents_of_set` writes `self.simulation = None` at
`mutation/toolpath.rs:307`, before the loop. D1 routes `AdoptResult` through
the same walk. An adopt that cleared the simulation would destroy the
`prior_stocks` snapshot the NEXT op in the ladder reads
(`session/compute.rs:2094`), and would report `simulation_cleared == true` on
every completion. So split, keeping the old name for the six existing sites:

```rust
/// Pure propagation: drop and bump, touch no simulation.
pub(crate) fn walk_output_dependents(&mut self, seeds: BTreeSet<usize>,
    chain_seeds: BTreeSet<usize>) -> (BTreeSet<usize>, BTreeSet<usize>)
{ /* §4.2 */ }

/// The edit-side door: the walk, plus the clear an edit owes.
pub(crate) fn invalidate_output_dependents_of_set(&mut self,
    seeds: BTreeSet<usize>, chain_seeds: BTreeSet<usize>)
    -> (BTreeSet<usize>, BTreeSet<usize>)
{ self.simulation = None; self.walk_output_dependents(seeds, chain_seeds) }
```

### 4.2 The new loop body

Derive the edges ONCE, before the loop. Edges come from configs; the loop
changes only results and revisions, so no edge appears or vanishes inside it.

```rust
debug_assert!(chain_seeds.is_subset(&seeds), "...");
let edges = crate::session::dependencies::edges(&*self);
let index_of: HashMap<ToolpathId, usize> = self.toolpath_configs.iter()
    .enumerate().map(|(i, tc)| (tc.id, i)).collect();
let mut dirty = seeds;                    // BTreeSet<usize>
let mut dropped = BTreeSet::new();
let mut chain_dirty = chain_seeds;

loop {
    let id_of = |i: &usize| self.toolpath_configs.get(*i).map(|tc| tc.id);
    let dirty_ids: BTreeSet<_> = dirty.iter().filter_map(id_of).collect();
    let chain_ids: BTreeSet<_> = chain_dirty.iter().filter_map(id_of).collect();

    let mut newly: Vec<usize> = Vec::new();
    for edge in &edges {
        let Some(src_id) = edge.on else { continue };
        let hit = match edge.kind {
            // (a) the material above the consumer moved
            EdgeKind::Stock => chain_ids.contains(&src_id),
            // (b) and (c) the source's OUTPUT moved
            EdgeKind::Regions | EdgeKind::PrevTool => dirty_ids.contains(&src_id),
        };
        let Some(&idx) = index_of.get(&edge.from) else { continue };
        if !hit || dirty.contains(&idx) || newly.contains(&idx) { continue }
        newly.push(idx);
    }
    if newly.is_empty() { break }
    for tp_idx in newly {                 // unchanged tail block
        self.drop_result(tp_idx);
        dirty.insert(tp_idx);
        dropped.insert(tp_idx);
        if self.toolpath_configs.get(tp_idx).is_some_and(|tc| tc.enabled) {
            chain_dirty.insert(tp_idx);
        }
    }
}
(dirty, dropped)
```

`edges` and `index_of` are owned, so the `&mut self` drops do not conflict.
`toolpath.rs` imports only `BTreeSet` today (`:6`); add the `HashMap` use line.

### 4.3 The drop set for (a) and (b) is unchanged

| Old, at `mutation/toolpath.rs:319-360` | New, and why it is the same |
|---|---|
| (a) latches at the first `chain_dirty` index in the setup, then drops every later enabled `FromRemainingStock` index | one Stock edge per (consumer, upstream) pair, hit on `chain_dirty`. The predicate is "any upstream in `chain_dirty`", and §2 keeps every upstream in the set |
| (a) never tests the upstream for `enabled` | the Stock source is not `enabled`-filtered (§2 proof 2) |
| (a) the CONSUMER must be `enabled` and `FromRemainingStock` | the derivation emits a Stock edge only for such a consumer. Keep that `enabled` test |
| (a) `continue` on a dirty upstream | the `dirty.contains(&idx)` guard |
| (b) consumer needs `boundary.enabled`; source in `dirty`, not `chain_dirty` | the Regions edge carries the `boundary.enabled` test and hits on `dirty` |
| (b) a source id naming no toolpath matches nothing | `edge.on` is `None` and the loop skips it |
| fixpoint, `dropped` excludes every seed, a disabled seed in `chain_seeds` | unchanged tail block, the `dirty` guard, and no seed is re-tested |

The one behaviour change is rule (c). It adds a drop only when a Rest consumer
has `prev_tool_id: Some(t)`, a resolved predecessor, and is NOT
`FromRemainingStock`. A `FromRemainingStock` Rest op is already reached by rule
(a), because its predecessor is by definition an earlier enabled op in the same
setup. On a predecessor DISABLE rule (c) does not fire at all: the walker
derives `edges` AFTER the mutation wrote `tc.enabled = false`
(`mutation/toolpath.rs:401` then `:404`), the PrevTool filter removes the edge,
and the consumer keeps its result, as today. The card then reads `Broken`.

### 4.4 Sentries that pin the drop set

From `rg -n "invalidate|Effects.stale|stale"` over
`src/session/mutation/tests*`, `tests/` and
`rs_cam_viz/src/controller/tests/`.

| Sentry | Pins | Moves |
|---|---|---|
| `tests/mutation_paths_invalidate_alike_p0.rs` | four write paths drop `{0,1}`; fixture index 1 is `Rest(RestConfig::default())`, so `prev_tool_id` is `None` and no PrevTool edge resolves | no |
| `tests/adopt_result_rejects_stale_completion.rs:183-188, 305-320` | `AdoptResult` reports `stale.is_empty()`; same fixture, no Regions and no PrevTool consumer | no, and this is why D1 must seed no `chain_seeds` (§5) |
| same file `:402-430`, and `src/session/mutation/tests.rs:267` | the enable toggle reports `{1}` through a DISABLED Stock source | no, by §2 proof 2 |
| `src/session/mutation/tests.rs:296-330` | one edit reaches a `FromRemainingStock` op and a `DerivedRestRegions` consumer transitively | no |
| `src/session/mutation/tests.rs:1120-1210` | the setter and `invalidate_toolpath_inputs` drop alike | no |
| `tests/stale_set_has_one_answer_wp28.rs` | no second staleness producer | no: `dependencies` produces edges, not a stale set |
| `tests/setters_have_rows_wp15a.rs:149-150` | the two walker names sit in `CRATE_PRIVATE_HELPERS` | **YES**: add `"walk_output_dependents"` |
| `restore_snapshot_invalidates_like_the_setter_n14.rs`, `replace_toolpath_config_gates_on_the_signature.rs`, `command_registry_completeness.rs` | each asserts its fixture rows are enabled; none reads a PrevTool edge | no |
| `rs_cam_viz/tests/rest_badge_one_predicate_g_restbadge.rs` | the card badge rule, with `prev_tool_id = Some(ROUGH_TOOL)` at `:106` and an adopt at `:152` | no, see the order rule below |

**D1 makes adopt ORDER matter in every existing fixture.** Source-then-consumer
is safe; consumer-then-source drops the consumer's result. Checked, with `rg`
over every `AdoptResult` site:

- `rest_badge_one_predicate_g_restbadge.rs` is the one fixture with a resolved
  PrevTool edge. Its only adopt sits inside `add()` (`:140-160`), which adopts
  at INSERT time, so every adopt precedes the existence of any later consumer.
  Safe by construction. Do not add an adopt of an earlier op after a later one.
- The three in-crate `DerivedRestRegions` fixtures
  (`src/session/mutation/tests.rs:384`, `:424`, `:462`) seed with
  `s.results.insert(..)`, the crate-private hatch, not with `AdoptResult`. D1 is
  inert there.
- Of the fourteen `AdoptResult` sites under `crates/rs_cam_core/tests/`, only
  `toolpath_rebind_g_mcprebind.rs` names `prev_tool_id`, and it sets the field
  at `:447` to assert the tool BINDING did not move (`:454`), not a result.

Rule (c) therefore lands green on the suite; §6 gives it its own coverage.

---

## 5. D1: `AdoptResult` drops its Regions and PrevTool consumers

`session/command.rs:2198-2215` checks the index, compares the revision and
calls `insert_result`, which bumps nothing by design
(`mutation/toolpath.rs:721-739`). So a source that regenerates without an input
edit leaves its consumers `Current`.

```rust
self.try_with_effects(Some(index), move |session| {
    session.insert_result(index, *result)?;
    // D1. The source's OUTPUT moved: its regions may have appeared,
    // changed or vanished. Rules (b) and (c) only. `chain_seeds` is
    // EMPTY: recording an answer removes no material, so rule (a)
    // must not fire.
    let _ = session.walk_output_dependents(
        BTreeSet::from([index]), BTreeSet::new(),
    );
    Ok(())
})
```

1. **Use the walker, not `results.remove`.** `Effects::stale` is derived from
   the revision map (`command.rs:1059-1068`, `:2562-2580`), and `drop_result`
   is the one site that bumps a revision (`mutation/toolpath.rs:772-776`). A
   bare `results.remove` drops the geometry and reports `stale` empty, the WP28
   defect this repository deleted once.
2. **`chain_seeds` MUST be empty.** Seeding `{index}` fires rule (a) and drops
   every downstream `FromRemainingStock` op on every adopt. In round 1 of a
   Generate All that refuses every in-flight completion with `StaleCompletion`,
   and it turns `adopt_result_rejects_stale_completion.rs:183-188` red, whose
   fixture index 1 IS `FromRemainingStock`.
3. **The simulation is NOT cleared**, because the arm calls
   `walk_output_dependents` (§4.1).

**Bumping a consumer's revision is correct, and cheap.** A consumer whose
generation started before the source's new result was adopted resolved its
boundary against the OLD `rest_regions` (`session/compute/generation.rs:565-580`
resolves from `self.results` at generation time), so its answer describes a
region set that no longer exists. `AdoptResult` refuses it
(`command.rs:2206-2212`), the core slot stays empty, the op reads
`EditedSince`, and the viz `Err` branch keeps `rt.result` so the viewport does
not go blank (`rs_cam_viz/src/controller/events/compute.rs:568-579`). The cost
is one wasted generation, and it cannot loop: the adopt happens once per
generation and the resubmission starts from the new revision. W1's plan walk
generates the source before the consumer, so inside a plan the refusal cannot
happen; the hand-regenerate case is W1's to resubmit.

**The viz twin `mark_derived_rest_dependents_stale`**
(`rs_cam_viz/src/controller/events/compute.rs:381-405`), three callers. Do not
edit any of them in W0.

- `compute.rs:589`, after an adopt: REDUNDANT with D1. Core now drops the
  consumer's result and bumps its revision, which derives `EditedSince` from
  the one truth. Its `stale_since` stamp is the GUI auto-regeneration clock, a
  separate mechanism. **W3 deletes this call** and re-points that clock at the
  adopt's `Effects::stale`.
- `toolpath.rs:408`, after a removal: already redundant, because core's
  `remove_toolpath` walks (`mutation/toolpath.rs:90`). **W1 deletes it or
  records why it stays.**
- `planner.rs:73`, after `plan_multitool_finishing` replaced ops: the planner
  removes ops inside core, so the same argument applies. **W1 checks it.**

Two more W1 handoffs at the same site. The comment at `compute.rs:569-571`
("The `Ok` effects are empty") becomes wrong. More important, the adopt call at
`:562-568` DISCARDS `Ok(effects)` and reads only the `Err` arm, so nothing in
viz stamps `stale_since` from the new stale set. Regions consumers still get a
stamp from `mark_derived_rest_dependents_stale` at `:589`; **PrevTool consumers
get no stamp at all** until W1 routes `effects.stale` through
`crate::state::stale::stamp_stale`, as every other command route does. Core
freshness still derives `EditedSince` from the empty result slot, so this is a
W1 correctness item, not a W0 blocker.

---

## 6. D3, and the sentry

**D3.** `session/compute/generation.rs:574-578` says the source "must be a
pencil operation with the rest-depth detector enabled". Since P2.5 any op with
`rest_analysis.enabled` attaches regions
(`compute/execute/dressup_apply.rs:129`), beside the pencil detector
(`compute/execute/finish_3d.rs:270,585`). Replace the message with: "'{source}'
produced no rest regions. Its rest analysis found no material above the
threshold, or it is switched off. Check the rest analysis settings on
'{source}' and regenerate it." `rg -n "must be a pencil operation"` reports one
production site and no test that matches the sentence; re-check before editing.

**The sentry**, red first:
`tests/dependency_edges_are_the_walker_dep1.rs` (claim words plus the programme
code, per `tests/CLAUDE.md`). Fixture, public doors only, the shape
`mutation_paths_invalidate_alike_p0.rs:150-256` uses:
`ProjectSessionBuilder::new()`, `add_tool` twice (rough `ToolId(0)`, fine
`ToolId(1)`), `add_model`, `add_setup("Setup 2", FaceUp::Bottom)`,
`add_toolpath`, `build`.

- Setup 1: `0` rough `Fresh`; `1` profile `Fresh`; `2` finish
  `FromRemainingStock`; `3` lakes `Fresh` with `DerivedRestRegions` on `0`;
  `4` rest2d `Fresh` with `Rest { prev_tool_id: Some(ToolId(0)) }`.
- Setup 2: `5` back rough `Fresh`; `6` back finish `FromRemainingStock`.
- Tools, which decide where the PrevTool edge lands: indices `0` and `5` carry
  `ToolId(0)`; `1`, `2`, `3`, `4` and `6` carry `ToolId(1)`. One model
  throughout. Without this, index 4's edge could resolve to index 1, 2 or 3 and
  claim 4 would read `{3}`.

Index 1 is the non-vacuity of §2 proof 1; Setup 2 is the non-vacuity of the
setup scope. Seed results with `Command::AdoptResult` at the current revision
(the `adopt` helper at `mutation_paths_invalidate_alike_p0.rs:214-227`), and
give index 0's result a non-empty `rest_regions` so index 3 can read `Ready`.
Seed a simulation with `Command::AdoptSimulation` (`session/command.rs:877-905`):
`SimulationResult` is public and every field is constructible
(`session/mod.rs:2492-2500` builds one), with `prior_stocks` holding
`TriDexelStock::from_bounds(&bbox, 2.0)` under the ids of indices 2 and 6. That
is what makes a `Ready` Stock edge reachable from an integration test.

Four claims:

1. `edges_name_every_declaration`. Non-vacuity: one edge of each kind exists,
   and index 1 declares none.
2. `every_walker_drop_is_an_edge`. For each seed set S in a table (edit index 0;
   disable index 0; edit index 2; adopt index 0), read `Effects.stale` through
   `apply`, subtract S, and assert each remaining index is the `from` of an edge
   inside the closure.
3. `every_edge_drop_is_made`. Compute the expected set INDEPENDENTLY in the test
   from `edges()`, with the kind-specific source sets the walker uses (Stock
   hits `chain_dirty`; Regions and PrevTool hit `dirty`), iterated to a
   fixpoint. Assert it equals `Effects.stale \ S`. Read `edges()` AFTER
   `apply`, never before: the walker derives them after the mutation wrote, so
   a pre-apply snapshot of the disable case still holds `4 -> 0` and predicts a
   drop the walker does not make. Phrase the claim this way, not as "every
   edge's drop is made": a Stock edge whose source is clean makes no drop, so
   the flat phrasing is false.
4. `regenerating_a_source_drops_its_consumers` (D1). Adopt index 0 again at its
   current revision. Assert `effects.stale == {3, 4}`, that `get_result(3)` and
   `get_result(4)` are gone, that `get_result(2)` and `get_result(6)` REMAIN
   (rule (a) did not fire), and that `simulation_result()` is still `Some` with
   `prior_stocks` intact.

Inject each guarded defect once and confirm red (`tests/CLAUDE.md`): drop the
`enabled` filter from the PrevTool derivation, make the Stock derivation
nearest-only, seed `chain_seeds` in the `AdoptResult` arm. The fixture builds
no geometry, so the file runs in seconds.

---

## 7. Blast radius

Measured with `rg`, not the graph tools (root `CLAUDE.md`).

| Symbol | Callers | Where |
|---|---|---|
| `invalidate_output_dependents_of_set` | 2 production | `mutation/toolpath.rs:271`, `:832` |
| `invalidate_output_dependents` | 6 sites in 5 methods | `mutation/toolpath.rs:90`, `:191`, `:192`, `:244`, `:404`, `:480` |
| both names as STRINGS | 1 test | `tests/setters_have_rows_wp15a.rs:149-150`; add `walk_output_dependents` |
| both names in prose | 9 doc comments, plus `session/CLAUDE.md:26` | `rg -n "invalidate_output_dependents" crates/` lists them. Comments only; none breaks |
| `Command::AdoptResult` | 1 production | `rs_cam_viz/src/controller/events/compute.rs:562`; see §5's three handoffs |
| `Command::AdoptResult` | 28 test sites | 14 files under `crates/rs_cam_core/tests/`, 11 under `rs_cam_viz/tests/`, 3 under `rs_cam_viz/src/controller/tests/`. §4.4's order rule covers them |
| `insert_result` | 1 caller | the `AdoptResult` arm, unchanged |
| the D3 sentence | 1 site | `session/compute/generation.rs:574-578`; no test matches it |
| `rest_predecessors` | 2 production, 1 test | `ui/toolpath_panel.rs:770`, `ui/properties/operations/validate.rs:261`, `rs_cam_viz/tests/rest_badge_one_predicate_g_restbadge.rs:41`; untouched in W0 |

Gate for W0: the four sentries in `session/CLAUDE.md`, the new `dep1` sentry,
`--test setters_have_rows_wp15a`, `--test mutation_paths_invalidate_alike_p0`,
`-p rs_cam_core -q --lib`, then fmt and workspace clippy. Every cargo command
through `scripts/cargo_lane.sh`. No full heavy gate.

---

## 8. Risks, and the one ruling

**R2, cross-setup stock, is the only ruling W0 waits on, and it does not block
the package.** §3 writes it as one `if` block that names R2. Ship W0 on the
narrow rule, which is what `prior_stock_blocker` already assumes
(`rs_cam_viz/src/controller/events/compute.rs:96-102`).

**Risk 1: rule (c) over-drops.** The generator reads the predecessor's TOOL,
not its result (`session/compute/generation.rs:294-303`), so a Rest consumer
whose predecessor regenerates has unchanged generation inputs. The cost is
bounded: rule (a) already covers every `FromRemainingStock` Rest op, so rule
(c) adds drops only for a `Fresh` Rest op with a resolved predecessor, and
those regenerate in seconds. Implement it as `PLAN.md` §4.1 states, and put
this paragraph in the module doc so a later reader does not read it as an
oversight.

**Risk 2: `edges()` is quadratic on the Stock kind.** A setup of k
`FromRemainingStock` ops emits up to k(k-1)/2 Stock edges. Real setups hold
tens of ops, the walker derives the set once (§4.2), and `primary_edges`
collapses it, so no surface renders the quadratic set.

**Risk 3: two copies of the PrevTool rule for one package.** W3 deletes the viz
copy. If W3 slips, record the duplicate in `planning/TECH_DEBT_REGISTER.md`.

**For W1, not a risk here:** a Regions consumer whose source has no result
fails with `SessionError::OperationFailed`
(`session/compute/generation.rs:565-569`), which reads as an ERROR, not as
`AwaitingPriorStock`. `PLAN.md` §3 says blocked is a sequencing state. W1 owns
the conversion; W0 changes only the D3 wording.

---

## 9. Order of work

`dependencies.rs` → the `dep1` sentry (confirm RED) → split the walker (§4.1)
→ rewrite the loop body (§4.2) → the `wp15a` allowlist row → D1 (§5) → D3
(§6) → `session/CLAUDE.md` (file map, sentry list, the invariant line naming
the edge function as the walker's input) → the local loop (§7).
