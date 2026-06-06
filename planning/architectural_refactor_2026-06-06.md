# Architectural refactor — making Cutter × Operation × Metric explicit

**Date:** 2026-06-06  
**Status:** Architecture sketch, revised after codebase review. This is **not** an
implementation spec; it is a scaffold for follow-up agents to analyze and turn
into smaller design/implementation plans.  
**Companion:** `planning/cutter_axial_constraints_2026-06-06.md` — useful Phase 0
work, but that plan needs its own review fixes before implementation.

---

## 1. Purpose

`rs_cam` now has 23 operations, 5 cutter families, and several tool-load / feeds
metrics. The interactions form a real dispatch matrix:

```text
OperationConfig × Tool/Cutter geometry × Material/Machine × Metric/Gate
```

A lot of this matrix is currently represented by scattered enum matches and
small per-site helper tables. Some of those are already compiler-enforced
(exhaustive `match` with no wildcard); others silently fall through or duplicate
routing logic. The goal of this refactor is **not** to replace all enums with
traits. The goal is to make the matrix more explicit, reduce duplicate routing,
and preserve/extend compile-time coverage where Rust can actually enforce it.

This revised sketch is intentionally more conservative than the first draft:

- keep public serde/TOML and MCP shapes stable unless a phase explicitly opts in
  to a schema change;
- avoid pretending `macro_rules!` can append enum variants from separate macro
  calls;
- avoid `enum_dispatch` designs with incompatible associated `Config` types;
- separate pre-sim profiles from post-sim gate verdicts;
- use the existing architecture (`OperationType::spec`, `OperationConfig`,
  `OperationParams`, `MillingCutter`, `ToolpathLoadVerdict`) as the starting
  point.

---

## 2. Current workflow audit — what is enforced vs what is scattered

### 2.1 Add a new operation today

Example: adding a hypothetical `Trochoidal` operation.

| # | Site | File | Today | Coverage signal |
|---|---|---|---|---|
| 1 | `OperationType` variant + `ALL` / `ALL_2D` / `ALL_3D` | `crates/rs_cam_core/src/compute/catalog.rs` | manual enum/list | partly manual; tests assert counts/categories |
| 2 | `OperationType::spec()` metadata | `compute/catalog.rs` | exhaustive match | compile-time if no wildcard |
| 3 | `OperationConfig` variant | `compute/catalog.rs` | serde-tagged enum | compile-time for exhaustive matches |
| 4 | Config struct + `Default` | `compute/operation_configs.rs` | manual | compiler for struct; no semantic coverage |
| 5 | `OperationParams` impl | `compute/operation_configs.rs` | manual trait impl | compiler enforces trait methods; optional accessors can silently default |
| 6 | Param schema array | `compute/catalog.rs::param_defs_for_type` | manual match/table | compile-time if no wildcard, but field/schema drift is semantic |
| 7 | Tool constraints | `compute/catalog.rs::tool_constraints_for_type` | manual match | compile-time if no wildcard |
| 8 | Generator function/body | usually op module + `compute/execute.rs` | actual algorithm + dispatch arm | `execute_operation_annotated` exhaustive match catches missing arm |
| 9 | Drill/aux op data, spans, semantic trace | `compute/execute.rs` and op module | manual | mostly semantic/test coverage |
| 10 | Feeds hints | `feeds/suggest.rs::operation_feeds_hints` | manual per-op hints | fallback to `(None, None, None)` is easy to miss |
| 11 | Suggest invariant passes | `feeds/suggest.rs::enforce_invariants` and helpers | mixed generic + per-op checks | many helpers are generic, but per-op routing can be skipped |
| 12 | Predictions | `feeds/predict.rs` | per-op match tables | mixed: exhaustive in places, NotApplicable fallbacks in others |
| 13 | LUT routing exceptions | `tool_load/chipload.rs::routed_lookup_family` | targeted exceptions | easy to miss when new op is not default-family-compatible |
| 14 | Gate not-applicable predicates | e.g. `tool_load/power.rs::is_plunge_only_op` | shared helper for drill today | new kinematic classes can be missed |
| 15 | GUI property editor | `crates/rs_cam_viz/src/ui/properties/...` | manual controls | not compiler-enforced by core |
| 16 | MCP schema/params | mostly via core schema; some viz/MCP glue | mostly derived, some manual | mixed |
| 17 | Project IO | `session/project_file.rs`, viz IO | serde handles operation block; defaults/legacy behavior manual | mostly automatic, compatibility tests needed |
| 18 | Tests / sweeps / lit matrix | `crates/rs_cam_core/tests/` | manual | discipline, not type-system |

**Important correction:** not all 17–18 sites are silent. The current codebase
already uses a lot of exhaustive matches. The real problem is not “Rust gives no
help”; it is that the single conceptual matrix is split across many separately
correct exhaustive matches, plus a handful of permissive defaults.

### 2.2 Add a new cutter geometry today

Example: a future `FacingCutter`.

| # | Site | File | Today | Coverage signal |
|---|---|---|---|---|
| 1 | `ToolType` variant/defaults | `compute/tool_config.rs` | public serde enum + defaults | compile-time for matches; project parsing has fallback |
| 2 | Tool project IO parse/display | `session/project_file.rs::parse_tool_type`, viz IO | manual string mapping | fallback to `EndMill` can hide unknown strings |
| 3 | `MillingCutter` impl | `tool/*.rs` | trait impl | compiler-enforced methods |
| 4 | Build `ToolDefinition` | `compute/cutter.rs::build_cutter` | match on `ToolType` | exhaustive compile-time |
| 5 | `ToolGeometryHint` / geometry classification | `feeds/mod.rs` | public enum | exhaustive where matched |
| 6 | LUT `ToolFamily` | `feeds/vendor_lut.rs` | serde enum/data | data + match coverage |
| 7 | Tool family mapping | `tool_load/chipload.rs::tool_family_for` and `vendor_normalize` | match | exhaustive compile-time |
| 8 | Feeds geometry math | `ToolGeometryHint::engaged_diameter_at_doc`, feeds calc | match + helper | exhaustive compile-time, semantic tests needed |
| 9 | Cutter-specific chip geometry / deflection profile | `tool/*`, `tool_load/*` | trait methods + gate helpers | mixed |
| 10 | GUI tool library/editor | viz UI | manual | not core-enforced |
| 11 | Vendor LUT rows/source metadata | `data/vendor_lut/observations` | data | lit-matrix/tests |

The cutter side is healthier because `MillingCutter` is already a behavior
trait and `build_cutter` is exhaustive. The risky bits are public API stability,
project IO fallbacks, GUI editor coverage, and per-metric semantic support.

### 2.3 Add a new tool-load metric today

Example: `SurfaceFinishRa`.

| # | Site | File | Today | Coverage signal |
|---|---|---|---|---|
| 1 | Gate module | `tool_load/*.rs` | implementation | local tests |
| 2 | Verdict type(s) | `tool_load/verdict.rs` | typed enums/structs | compiler after field added |
| 3 | `ToolpathLoadVerdict` field | `tool_load/verdict.rs` | fixed public serde shape | manual schema decision |
| 4 | Evaluate in report builder | `tool_load/mod.rs::evaluate_toolpath` | manual call | compile-time once field exists |
| 5 | Summary / export / optimizer consumers | `tool_load/*`, `optimizer/*` | manual | mixed |
| 6 | MCP serialization | `rs_cam_mcp`, viz MCP bridge | serde helps if field exists; UI text manual | mixed |
| 7 | GUI display | viz properties/diagnostics | manual | not core-enforced |
| 8 | Rationale / Suggest if pre-sim | `feeds/rationale.rs`, `feeds/suggest.rs` | manual | exhaustive warning match helps |
| 9 | Tests / lit-matrix | tests/data | manual | discipline |

Metric refactoring has a hard fork:

1. **Keep typed public report** (`ToolpathLoadVerdict { chipload, power,
   deflection, drill_gates, ... }`). This preserves MCP/serde compatibility but
   adding a metric necessarily touches the typed report and consumers.
2. **Move to dynamic metric vector** (`Vec<CriterionStatus>` / registry). This
   enables “implement + register” ergonomics but is a public schema migration.

The first implementation phase should not pretend we get both for free.

---

## 3. Refined conceptual model

### 3.1 Keep the existing axes, add explicit registries/profiles around them

The current axes are already real and useful:

- `OperationType` + `OperationConfig` + `OperationParams`
- `ToolType` + `ToolConfig` + `ToolDefinition`/`MillingCutter`
- typed verdicts + `ToolpathLoadVerdict`

The refactor should first make their metadata/routing explicit before trying to
replace them.

```text
                   ┌─────────────────────────┐
                   │ Operation registry       │
                   │ - OperationType          │
                   │ - OperationSpec          │
                   │ - Param schema           │
                   │ - tool compatibility     │
                   │ - feeds hints            │
                   └───────────┬─────────────┘
                               │
┌─────────────────┐     ┌──────▼──────┐      ┌────────────────────┐
│ Cutter profile  │     │ Preflight   │      │ Metric evaluators  │
│ - ToolType      │────▶│ profile     │◀────▶│ - chipload         │
│ - CutterKind    │     │ (pre-sim)   │      │ - power            │
│ - geometry math │     │ feeds +     │      │ - deflection       │
│ - compatibility │     │ constraints │      │ - drill_gates      │
└─────────────────┘     └─────────────┘      └────────────────────┘
                               │
                               ▼
                    existing OperationConfig + generators
```

### 3.2 `CutterOpProfile` is pre-sim, not a replacement for sim gates

A realistic profile object:

```rust
pub struct CutterOpProfile<'a> {
    pub tool_cfg: &'a ToolConfig,
    pub tool_def: &'a ToolDefinition,
    pub operation: &'a OperationConfig,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,

    pub cutter_kind: CutterKind,
    pub op_type: OperationType,
    pub spec: OperationSpec,

    pub feasibility: Result<(), RefusalReason>,
    pub feeds: Option<FeedsResult>,
    pub constraints: ConstraintEnvelopes,
    pub predictions: Predictions,
    pub warnings: Vec<SuggestWarning>,
}
```

Boundaries:

- **Can own:** tool/op compatibility, feeds result, pre-sim axial/radial
  envelopes, move-count estimate, closed-form predictions, Suggest warnings.
- **Must not own:** final `ToolpathLoadVerdict` from simulation trace. Chipload,
  power, deflection, air-cut, drill summaries still need sim samples/spans.
- **Can feed into gates:** it can provide shared context (routed LUT family,
  compatibility result, expected kinematic class) so gates duplicate less logic.

This makes Phase 4 a bundling/refactoring phase, not a behavioral change.

### 3.3 Avoid the associated-type trap for operations

The first sketch used:

```rust
trait ToolpathOp { type Config; ... }
#[enum_dispatch]
enum ToolpathOpKind { Scallop(ScallopOp), Pocket(PocketOp), ... }
```

That does **not** fit a single dispatch enum when every variant has a different
`Config` associated type.

Viable alternatives:

#### Option A — keep `OperationConfig` as the behavior enum

```rust
impl OperationConfig {
    pub fn registry_entry(&self) -> &'static OpRegistryEntry { ... }
    pub fn feeds_hints(&self) -> FeedsHints { ... }
    pub fn compatibility(&self, cutter: CutterKind) -> Result<(), RefusalReason> { ... }
}
```

Pros: minimal churn, serde-compatible, no new dependency.  
Cons: still enum-method oriented; less “traity”.

#### Option B — central static operation registry

```rust
pub struct OpRegistryEntry {
    pub kind: OperationType,
    pub spec: OperationSpec,
    pub params: &'static [ParamDef],
    pub tool_constraints: ToolConstraints,
    pub feeds_hints: fn(&OperationConfig) -> FeedsHints,
    pub compatibility: fn(CutterKind) -> Result<(), RefusalReason>,
    pub generate: GenerateFn,
}
```

Pros: one source of metadata/routing, easy to inspect, no associated-type issue.  
Cons: function pointers take `&OperationConfig` and must validate the expected
variant internally.

#### Option C — per-config trait impls, no enum dispatch

Each config struct implements a common trait, and `OperationConfig` forwards via
one exhaustive match. This keeps type-specific config methods inside the config
impl but accepts one central forwarding match.

Recommended starting point: **Option A/B hybrid**. Keep `OperationConfig`; create
an `OpRegistryEntry` table generated from a single X-macro list; migrate callers
to read the table before considering any trait-object/enum-dispatch design.

### 3.4 Use an X-macro, not append-style `define_op!`

A `macro_rules! define_op! { ... }` invocation in one module cannot append a
variant to an enum declared elsewhere. Rust macros expand locally; there is no
stable “central include gets entries from all files” mechanism without a build
script/proc macro/inventory-style linker trick.

A realistic macro shape is a single authoritative list:

```rust
macro_rules! for_each_op {
    ($m:ident) => {
        $m!(Face, FaceConfig, face_spec, face_params, face_generate);
        $m!(Pocket, PocketConfig, pocket_spec, pocket_params, pocket_generate);
        // ... all 23 ops ...
    };
}
```

Then derive from that one list:

- `OperationType`
- `OperationType::ALL`
- `OperationType::spec()`
- `OperationConfig` variant list (maybe later)
- `param_defs_for_type`
- `tool_constraints_for_type`
- registry entries/tests

This is less magical and works with `macro_rules!`. Later, after the shape is
stable, agents can decide whether a proc macro/build script is worth it.

---

## 4. What the type system can and cannot enforce

### Strong compile-time wins we can realistically get

| Change | Enforcement mechanism |
|---|---|
| Add op to central list | generated `OperationType`, `ALL`, spec, schema stay in sync |
| Add op but forget generator adapter | registry entry / generated tests fail to compile |
| Add cutter kind | exhaustive matches on `CutterKind` fail where no wildcard is used |
| Add new required method to a trait | compiler breaks every impl |
| Add Suggest warning | `feeds/rationale.rs` exhaustive match already forces rationale coverage |
| Add typed metric field | report builders/tests fail until field initialized |

### Things Rust will not prove

- generator algorithm correctness;
- that a config schema description matches every field’s semantic meaning;
- GUI renders every new config field nicely;
- lit-matrix has enough coverage;
- a dynamic metric registry remains backwards-compatible with MCP clients;
- a fallback arm is “safe” rather than hiding a missing case.

### Design rule

Prefer **one central exhaustive match/table** over many small exhaustive matches.
When a fallback is necessary, encode it as a named policy, not `_ => default`.

---

## 5. Tooling recommendations, revised

### 5.1 Use now / likely good

**In-tree X-macros (`macro_rules!`)**  
Best first tool. No dependency. Works for deriving repeated enum/list/table
boilerplate from one authoritative operation list.

**Sealed traits only for new internal traits**  
Do not immediately seal public `MillingCutter`; it is currently public and used
through `ToolDefinition`. Add internal sealed traits such as `OpBehavior` or
`MetricEvaluator` only where we know external extension is not a supported API.

**Existing exhaustive matches + tests**  
Several current matches are already good. Preserve them while moving their data
sources into one registry.

### 5.2 Possible later

**`enum_dispatch`**  
Useful only if the trait has no incompatible associated types and the enum
variants are homogeneous from the trait’s perspective. Do not choose it until a
prototype proves the `OperationConfig`/config-type issue is solved.

**`bon` typed builder**  
Nice-to-have for large profile construction, but not necessary. A plain
constructor or small builder struct may be clearer until the shape stabilizes.

**`strum`**  
Not currently in the manifests. Could replace manual `ALL` lists, but an X-macro
may make it unnecessary. Treat as a dependency decision, not assumed reality.

**Dynamic metric registry**  
Powerful, but only after deciding whether public report schemas can change.

### 5.3 Avoid for now

- proc macros for Phase 1/2;
- build-script codegen unless the X-macro becomes unmaintainable;
- `#[non_exhaustive]` on internal matrix enums;
- macro-wrapping generator math.

---

## 6. Phased refactor plan, revised

Each phase should produce small PRs and parity tests. The LOC deltas below are
rough; do not promise net −1600 LOC until a prototype measures it.

### Phase 0 — Axial constraint envelope (before structural refactor)

Ship the independently valuable axial-constraint work first, but incorporate the
prior review corrections:

- reuse existing `ap_min_mm` / `ap_max_mm` rather than inventing duplicate
  absolute-mm fields;
- build deflection bounds from explicit axial/radial inputs and
  `ToolDefinition`/`tip_deflection_mm`, not only `predict_peak_deflection_um`;
- use binary search for nonlinear cutter profiles;
- start with Adaptive3d DPP + VCarve/ProjectCurve feasibility/clamps; make
  surface-following finish mutation conservative/warning-first.

Outcome: real-world pressure test for profile/constraint API signatures.

### Phase 1 — Operation registry from existing data

Goal: one place to ask “what is this op?” without changing generation.

Tasks:

1. Introduce `OpRegistryEntry` and `FeedsHints` types.
2. Move/bridge existing `OperationType::spec`, `param_defs_for_type`,
   `tool_constraints_for_type`, and `operation_feeds_hints` behind registry
   accessors.
3. Keep existing public functions and serde shape intact.
4. Add tests:
   - every `OperationType::ALL` has one registry entry;
   - every `OperationConfig::new_default(op).op_type() == op`;
   - schema field names match serialized default params;
   - no registry entry uses placeholder compatibility unintentionally.

Expected risk: low-medium. This is mostly metadata consolidation.

### Phase 2 — Central operation X-macro

Goal: remove duplicated operation lists/counts.

Tasks:

1. Add `for_each_op!` central list.
2. Generate `OperationType`, `OperationType::ALL`, maybe `ALL_2D`/`ALL_3D`,
   and registry stubs from it.
3. Migrate one generated surface at a time; keep diffs reviewable.
4. Do **not** generate `OperationConfig` until the list is trusted.

Expected risk: medium. Macro mistakes can create noisy diffs, so this should be
split into mechanical PRs.

### Phase 3 — Cutter classification cleanup

Goal: make cutter kind and compatibility explicit without breaking public cutter
APIs.

Tasks:

1. Introduce internal `CutterKind` as the canonical classifier.
2. Provide conversions from `ToolType`, `ToolGeometryHint`, and `ToolDefinition`.
3. Move `tool_family_for`, V-bit angle routing, and compatibility checks toward
   this classifier.
4. Audit project IO fallbacks (`parse_tool_type`) and decide whether unknown tool
   types should warn/error rather than silently become `EndMill`.
5. Do not seal `MillingCutter` yet; decide separately if external/custom cutter
   impls are supported.

Expected risk: low-medium.

### Phase 4 — Preflight `CutterOpProfile`

Goal: build one pre-sim context for Suggest, feeds explanation, and diagnostics.

Tasks:

1. Add `CutterOpProfile::for_combo` around existing feeds/suggest helpers.
2. Include axial constraints from Phase 0 and any existing predictions.
3. Migrate consumers gradually:
   - feeds modal/explain path;
   - Suggest orchestration;
   - MCP “explain/recommend” surfaces;
   - diagnostics context builders.
4. Keep simulation gate evaluation separate, but allow gates to consume profile
   context where it removes duplicated routing.

Expected risk: medium; useful tests are snapshot/parity tests around Suggest and
MCP outputs.

### Phase 5 — Operation behavior adapters

Goal: reduce `execute_operation_annotated`/routing weight without an all-at-once
rewrite.

Tasks:

1. For one operation family, create a `GenerateFn` adapter that takes existing
   `ExecutionContext` + `&OperationConfig` and calls the current generator.
2. Move adapters into registry entries one family at a time.
3. Keep `execute_operation_annotated` as the public entry point; internally it
   can dispatch through the registry after parity is proven.
4. Retain exhaustive checks/tests so missing adapters fail loudly.

Expected risk: medium-high because this touches generation plumbing, spans, drill
aux data, semantic trace, cancellation, and remaining-stock inputs.

### Phase 6 — Metric refactor decision point

Before coding, choose one:

#### 6A. Typed report preserved

- Add shared `MetricContext` to reduce duplicated argument lists.
- Keep `ToolpathLoadVerdict` fields.
- Metric modules implement a common internal trait for tests/organization, but
  report construction remains typed.

Pros: compatible, low risk.  
Cons: adding a metric still touches report fields/consumers.

#### 6B. Dynamic metric vector

- Introduce `MetricId`, `MetricStatus`, registry iteration.
- Keep old typed fields during a compatibility window, or version the schema.
- Update MCP/GUI/export to render vectors.

Pros: “implement + register” becomes real.  
Cons: public schema migration; larger UI/MCP impact.

Recommendation for now: **Phase 6A first**. Do not promise dynamic registry
benefits until a schema migration is explicitly approved.

### Phase 7 — Optional config/schema generation

Only after Phases 1–5 stabilize, evaluate generating config structs/defaults or
schema declarations from macros. This is where a future `define_op!` may make
sense, but it should be based on the proven registry shape rather than designed
up front.

---

## 7. Out of scope

- Rewriting algorithm bodies (`adaptive3d`, `scallop`, `waterline`, etc.).
- Changing project TOML format unless a phase explicitly says so.
- Changing MCP schemas before a migration decision.
- Changing GUI layout/theme.
- Reinterpreting vendor LUT rows beyond Phase 0 axial fields.
- Reverting v3 / Findings fixes.
- Turning rs_cam into a plugin system. The refactor is internal-first.

---

## 8. Value vs cost

### Value

- Fewer duplicated operation/cutter/metric routing tables.
- Central “what does this operation mean?” registry.
- Easier axial/radial constraint integration.
- Cleaner agent handoff: each area has an obvious home.
- Better compile-time coverage where the current code still relies on permissive
  defaults.

### Cost

- Several weeks of structural work if all phases ship.
- High review burden for macro-generated diffs.
- Risk of churn in a codebase that already has many sentry tests.
- Possible dependency/API decisions if `enum_dispatch`, `strum`, or dynamic
  metric registries are adopted.

### Current recommendation

Do **not** launch a big-bang architectural rewrite. Do:

1. fix/ship Phase 0 axial constraints;
2. run agents on Phase 1 registry design and Phase 3 cutter classification;
3. prototype the X-macro on metadata only;
4. postpone generation/metric dynamic registry until those prototypes prove the
   shape.

---

## 9. Open questions for reviewer/agent wave

1. **Operation registry shape:** `OpRegistryEntry` with function pointers, or
   methods on `OperationConfig` backed by a registry?
2. **Central list:** X-macro now, or keep hand-written exhaustive matches until
   after Phase 1?
3. **Cutter API:** is external `MillingCutter` implementation supported? If yes,
   do not seal it; if no, document the internal-only contract first.
4. **Unknown project tool type:** keep fallback-to-EndMill, warn, or fail load?
5. **Phase 0 finish-op behavior:** warning-first vs automatic `stock_to_leave`
   mutation for surface-following finish ops.
6. **Metrics:** typed report preserved (6A) or schema migration to dynamic vector
   (6B)?
7. **Dependencies:** approve any of `enum_dispatch`, `bon`, `strum`, or require a
   no-new-dependency prototype first?
8. **Cutover strategy:** one op family at a time vs metadata-only all ops first.
9. **Parity gates:** which exact tests/sweeps must pass after each phase?

---

## 10. Suggested agent split

For the upcoming analysis/cleanup wave:

1. **Operation registry agent** — inspect `compute/catalog.rs`,
   `operation_configs.rs`, `compute/execute.rs`; propose Phase 1 registry shape
   with minimal diff.
2. **Cutter classification agent** — inspect `ToolType`, `ToolGeometryHint`,
   `MillingCutter`, vendor LUT mapping, project IO; propose `CutterKind` and
   compatibility matrix.
3. **Metric/reporting agent** — inspect `tool_load/verdict.rs`, `tool_load/mod.rs`,
   MCP/GUI consumers; decide 6A vs 6B implications.
4. **Suggest/preflight agent** — inspect `feeds/suggest.rs`, `feeds/predict.rs`,
   `feeds/mod.rs`; design `CutterOpProfile` inputs/outputs around Phase 0 axial.
5. **Macro/prototype agent** — test a tiny X-macro over 2–3 ops in a throwaway
   branch/worktree; report rust-analyzer/test ergonomics.

---

## 11. Pre-implementation checklist

Before coding beyond Phase 0:

- [ ] Phase 0 axial plan updated with prior review corrections.
- [ ] Decide typed vs dynamic metric reporting for long-term direction.
- [ ] Decide whether `MillingCutter` remains externally implementable.
- [ ] Approve or reject new dependencies for prototypes.
- [ ] Pick Phase 1 registry shape.
- [ ] Define parity test set for operation metadata and Suggest outputs.

---

## tl;dr

The refactor direction is good, but it should start as metadata/profile
consolidation, not a trait/macro big bang. Keep `OperationConfig` and public
schemas stable; introduce an operation registry, internal cutter classifier, and
pre-sim `CutterOpProfile`; use a single X-macro only after the registry shape is
proven; treat metric registry as a schema decision. Phase 0 axial constraints
should still ship first, with the review fixes applied, because it will reveal
the real interfaces this architecture needs.
