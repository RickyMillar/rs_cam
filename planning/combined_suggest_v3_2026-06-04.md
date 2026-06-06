# Combined Suggest v3 — Design Doc

**Date**: 2026-06-04
**Predecessor**: `planning/combined_suggest_design_2026-06-03.md` (v1+v2)
**Trigger**: Wanaka post-v2.1 validation (2026-06-03) returned `verdict=OK`
with zero rapid collisions, but the chipload-low gate still fires by 1–15 %
margins on the three roughing toolpaths, and the operator has no way to:
(a) ask Suggest to aim further inside the LUT band rather than at its lower
edge, (b) opt into the more aggressive "push for speed" recipe we shipped
before v2, or (c) see *why* Suggest backed off a value it could otherwise
have written. v2 also explicitly punted "strategy-aware Suggest" to v3.

## Three goals, one architecture

The v1 design doc treated strategy auto-selection, target-aggressiveness,
and rationale surfacing as separate work items. v3 unifies them under one
type — `SuggestPolicy` — that the caller threads through `SuggestContext`.
The orchestrator in `suggest::enforce_invariants` reads the policy at the
pass-function granularity it already has.

```rust
pub struct SuggestPolicy {
    /// Where inside the LUT band the feed-up recalibration aims.
    pub aggressiveness: SuggestAggressiveness,
    /// Whether the strategy/entry/stock-to-leave passes run, and how
    /// hard they push (warning-only vs auto-rewrite).
    pub scope: SuggestScope,
    /// Whether warning emission goes into the "verbose" mode the
    /// rationale surface consumes. Off by default — the orchestrator
    /// still emits the structured `SuggestWarning` enum; this just
    /// gates the optional `notes: Vec<SuggestNote>` field that carries
    /// human-readable strings for the rationale tree.
    pub verbose_rationale: bool,
}
```

Default `SuggestPolicy::default()` = `{ aggressiveness: Median, scope:
FeedsWithGates, verbose_rationale: false }`. This is the directive's
"calibrate to LUT median should be default" line. Existing call sites
that pre-date the policy (the integration test, the CLI example, the
literature matrix) become `SuggestContext { policy: SuggestPolicy::default(),
.. }` — no behaviour change relative to v2.1 *unless* `lut_min` is
replaced by `lut_median` (it is — see goal 2).

## Goal 1 — Strategy-aware Suggest

### What gets selected, and when

Three operation-config fields are auto-rewriteable today:

| Field | Owner | What v3 selects | Skip when |
|---|---|---|---|
| `Adaptive3dConfig::entry_style` | `Adaptive3dEntryStyle::{Plunge,Helix,Ramp}` | Helix when `dpp > 0.5×D` and the model has ≥ `helix_radius_factor × D` of headroom; Plunge otherwise on `dpp ≤ 0.3×D`; Ramp in the middle | `scope == FeedsOnly`; user has touched the field |
| `Adaptive3dConfig::clearing_strategy` | `ClearingStrategy::{ContourParallel,Adaptive,AgentSearch}` | `ContourParallel` by default; `Adaptive` when geometry classifier reports "mixed terrain"; `AgentSearch` only when a v2.x finding tagged the model as "leaves uncut bands" | `scope == FeedsOnly`; user has touched the field; v3 ships `ContourParallel`-only and surfaces a warning for the other two — auto-selection across these is v4 |
| `Adaptive3dConfig::{stock_to_leave_radial,stock_to_leave_axial}` | `f64` | Both `0.0` on the *last* rough pass before a finish op; both `0.3 × stepover` upstream of an as-yet-unwritten finish op; both `0.0` on standalone rough; user-overridable | `scope == FeedsOnly`; user-pinned; cross-toolpath context unavailable (`SuggestContext::upstream_leftover_stock_mm` is `None`) |

Drill / V-bit / 2D adaptive don't carry these knobs — strategy-aware
Suggest is a no-op for them. Today's `check_plunge_entry_stability` pass
(v1.3) becomes the *warning side* of the same logic in the v3
`pick_adaptive3d_entry_style` pass — strategy auto-rewrite emits no
warning, strategy-mismatch warning fires when `scope == FeedsWithGates`
and the user pinned a bad combo.

### Geometry classifier

A new module `feeds::geometry_class` exposes:

```rust
pub enum GeometryClass {
    /// Mostly flat surfaces (max slope < 30°) — DropCutter/Scallop preferred,
    /// helix entry trivially available.
    ShallowTerrain,
    /// Steep walls or vertical features dominate — waterline-friendly,
    /// helix entry needs interior pocket of size ≥ helix_radius × 2.
    SteepTerrain,
    /// Mix of both — adaptive clearing recommended, strategy choice depends
    /// on roughing vs finishing role.
    MixedTerrain,
    /// 2D / pocket-only geometry — strategy selection is moot.
    PocketLike,
    /// Feature-driven input (V-carve curves, drill holes, trace polylines).
    FeatureDriven,
    /// Can't classify — no model bbox, degenerate inputs, etc.
    Unknown,
}

pub fn classify(
    op_type: OperationType,
    model_bbox: Option<&BoundingBox3>,
    model_summary: Option<&ModelGeometrySummary>,
) -> GeometryClass;
```

`ModelGeometrySummary` already exists in slim form on `inspect_model`
(triangle count, slope histogram for STL/STEP, polygon area for SVG/DXF).
v3 surfaces what's already cheap; it does *not* introduce a new full-model
analysis pass. When `model_summary` is `None`, the classifier degrades to
the op-type-only heuristic ("Adaptive3d → MixedTerrain, Pocket → PocketLike,
V-carve → FeatureDriven, etc.").

`SuggestContext` gains:

```rust
pub model_summary: Option<&'a ModelGeometrySummary>,
pub policy: SuggestPolicy,
```

`upstream_leftover_stock_mm` and `neighboring_strategy_hint` keep their
v1.1 placeholder semantics — v3 wires the first one (finish DPP from
rough leftover) and *deletes* the second (no consumer ever materialized;
strategy hint is now derived from `policy.scope` + classifier).

### Strategy-pass ordering inside `enforce_invariants`

v3 adds three passes between the v1.1 DPP back-off and the v1.3
plunge-entry warning, all gated on `scope ∈ {StrategyAndFeeds, FeedsWithGates}`:

```text
clamp_plunge_to_feed
clamp_stepover_to_diameter
backoff_stepover_for_runtime
clamp_dpp_to_rigidity
clamp_dpp_to_cutting_length
backoff_dpp_for_deflection
+ pick_adaptive3d_entry_style       ← v3 (StrategyAndFeeds only)
+ pick_adaptive3d_clearing_strategy ← v3 (StrategyAndFeeds only; warns when off)
+ pick_stock_to_leave               ← v3 (StrategyAndFeeds only)
check_plunge_entry_stability        ← v1.3 (FeedsWithGates and up)
recalibrate_feed_for_chipload       ← v2.1
```

The strategy passes read the *post-back-off* DPP — they need to know the
real operating depth before deciding entry style. That mirrors the v1.3
ordering and is intentional.

### Pinning

Users routinely set `entry_style` manually because they know the model
has (or hasn't) a helix-friendly pocket. Strategy passes must not stomp
those values. Implementation paths:

- **A. `OperationConfig` carries a per-field "user touched" bit.** Heavy:
  every field on every config gets a sibling `bool`. Rejected.
- **B. Compare against the type's `Default::default()` value at Suggest
  time.** Lightweight, but produces false positives when the user
  *explicitly* picks the default (Helix-as-typed = Helix-as-default).
  Acceptable trade-off because v3's strategy chooser also picks Helix
  in the common case — the "stomp" is a no-op.
- **C. Maintain a per-operation `pinned_fields: BTreeSet<FieldId>` set
  on the project file.** Honest, but requires schema migration and a GUI
  surface to manage it.

v3 ships **B** with a one-line allow-list of "fields treated as default
even if non-default": currently empty. v4 may upgrade to **C** if the
pinning model needs more nuance. The `SuggestWarning::StrategyRewrote`
variant carries `from_value` and `to_value` so any unintended stomp is
visible in the rationale tree.

### When the classifier doesn't fire

`scope == FeedsOnly` → strategy passes short-circuit, behave like v2.1
exactly. CLI's `--apply-suggest=feeds` lands on this path. This is the
escape hatch for power users who don't want geometry inference touching
their carefully-pinned ops.

## Goal 2 — LUT median as default, with speed opt-in

### `SuggestAggressiveness`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuggestAggressiveness {
    /// Aim observed median chipload at LUT band minimum. v2.1 behaviour.
    /// Use when material variability is high, tool is new, or workholding
    /// is suspect.
    Conservative,
    /// Aim observed median chipload at LUT band midpoint
    /// (`(min + max) / 2`). Default — directive 2026-06-03.
    #[default]
    Default,
    /// Aim observed median chipload at LUT band maximum, gated by
    /// deflection (200 µm − 10 µm headroom) and `machine.max_feed_mm_min`.
    /// Equivalent to the pre-v2 "push for speed" recipe — uses every µm
    /// the deflection budget can spare.
    Speed,
}

impl SuggestAggressiveness {
    pub fn target_chipload(self, bounds: ChiploadBounds) -> f64 {
        match self {
            Self::Conservative => bounds.min_mm_per_tooth,
            Self::Default => 0.5 * (bounds.min_mm_per_tooth
                                  + bounds.max_mm_per_tooth),
            Self::Speed => bounds.max_mm_per_tooth,
        }
    }
}
```

### Schema change: `ChiploadBounds` regains `max_mm_per_tooth`

The polish-pass dropped `max_mm_per_tooth` ("only `min` is read"). v3
needs both, because the target line moves. `feeds/mod.rs::ChiploadBounds`
gets the field back:

```rust
pub struct ChiploadBounds {
    pub min_mm_per_tooth: f64,
    pub max_mm_per_tooth: f64,
}
```

`vendor_lookup` already has the LUT row's `chipload_max_mm_tooth` (it's
read in `chipload_midpoint`); `feeds/mod.rs:689` just needs to thread it
through. One-line change.

### Recalibration math change

`recalibrate_feed_for_chipload` substitutes `bounds.min_mm_per_tooth`
with `policy.aggressiveness.target_chipload(bounds)`. Closed-form solve
is unchanged:

```text
target_nominal = target_chipload / arc_fit_ratio
target_feed    = target_nominal × rpm × flutes
new_feed       = min(target_feed, machine.max_feed_mm_min)
```

The `FeedRaisedForChipload` warning grows a `target_chipload_mm_per_tooth`
field (was implicitly `lut_min_mm_per_tooth`). Rename `lut_min_*` →
`lut_target_*` across the warning + integration test + downstream
serialization; behaviour for `Conservative` is identical to v2.1.

### Speed cap interaction with deflection

`Speed` aims at LUT max. If the predicted observed at max-feed exceeds
the deflection headroom, the closed-form solve reverts to the largest
feed that still clears 190 µm — and the `DeflectionThreshold` cap fires
(today's defensive no-op becomes load-bearing for `Speed`). This is the
"deflection-gated push for speed" the directive asked to preserve.

When `policy.aggressiveness = Speed` and the deflection cap binds, an
additional warning `SuggestWarning::SpeedGatedByDeflection` fires —
distinct from `ChiploadStillLowAfterRecalibration` (which means "we
hit max_feed below LUT min"). The two share structure but the *meaning*
is different: SpeedGated is "you asked for max chipload, deflection said
no, we gave you the most we could"; StillLow is "even at max feed we
couldn't reach LUT min, your operating envelope is too tight".

### Test surface

- `wanaka_suggest_integration.rs` baseline currently asserts `obs_after
  ≥ lut_min * 0.95`. v3 splits into three sub-tests by aggressiveness,
  each with its own baseline window. `Default` lands in
  `[lut_min, (min+max)/2]`; `Speed` either reaches `lut_max` or trips
  `SpeedGatedByDeflection`.
- New unit test in `suggest::tests`: same op × tool, three policies, all
  three feeds monotonically ordered (Conservative ≤ Default ≤ Speed).
- Literature matrix: rerun under `Default` and assert no anti-pattern
  trips. The matrix today validates `min`-targeting; switching the
  default means the matrix baselines may shift. Walk it cell-by-cell
  before flipping the default in production.

## Goal 3 — Warning display surface

### What's there today

- `SuggestWarning` (9 variants, structured fields) accumulates in
  `SuggestedParams::warnings: Vec<SuggestWarning>`.
- `wanaka_suggest_integration.rs` reads the vec to assert which variants
  fire — it's the *only* consumer.
- `feeds_modal.rs::draw_warnings` (line 1074) consumes the *other*
  warning type (`FeedsWarning` from the calculator), not `SuggestWarning`.
- MCP exposes nothing about Suggest warnings.

### What v3 adds

**At core**: a `SuggestRationale` struct that materializes from `&[SuggestWarning]`:

```rust
pub struct SuggestRationale {
    pub entries: Vec<RationaleEntry>,
}

pub struct RationaleEntry {
    pub param: RationaleParam,
    pub from_value: Option<f64>,
    pub to_value: Option<f64>,
    pub reason: RationaleReason,
    pub headline: String,           // human-readable, one-line
    pub detail: Option<String>,     // optional supporting numbers
}

pub enum RationaleParam { Dpp, Stepover, Feed, Plunge, EntryStyle, ClearingStrategy, StockToLeave }
pub enum RationaleReason { LutClamp, DeflectionPredict, RuntimeFloor, RigidityFactor,
                           CuttingLengthCap, ChiploadFloor, ChiploadCeiling, GeometryClassifier,
                           PlungeEntryUnstable, UserPin, SpeedTargetGated }

impl SuggestRationale {
    pub fn from_warnings(ws: &[SuggestWarning]) -> Self { ... }
}
```

The conversion is a deterministic match in one place; the type lives in
`feeds::suggest::rationale` and is `Serialize` so MCP/JSON can ship it
verbatim. This is the design-doc §3 rationale tree, scoped to what the
nine warning variants actually carry.

**At GUI**: a new collapsible "Why these values?" section in
`feeds_modal.rs`, rendering `SuggestRationale::entries` as a vertical
list. Existing `draw_warnings` stays for `FeedsWarning`. The Suggest
button populates the rationale from the call result; clicking individual
entries selects the affected parameter for highlight in the modal's
existing param panel. Keep wiring shallow — no new modal state machine,
just a new section under the params.

**At MCP**: a new tool `get_suggest_rationale(toolpath_index: usize) ->
SuggestRationale`. Pairs with the existing `set_toolpath_param` — agents
can read the rationale, then accept/reject specific values. Tool returns
the latest rationale captured when Suggest last ran on that index
(stored on `ProjectSession::toolpath_rationales: HashMap<TpId, SuggestRationale>`).
Empty for toolpaths Suggest has not yet visited.

### What it does *not* add

- A separate rationale modal — keep it inline.
- A persistent "history of suggestions" log — overwrite on each Suggest
  call.
- A diff between previous and current rationale — useful, but v4.

## Migration

Phased, each phase independently shippable:

1. **`ChiploadBounds::max_mm_per_tooth` restore** (v3.0a) — 1 PR, ~30 LOC,
   green tests required. Unblocks goal 2.
2. **`SuggestAggressiveness` + policy plumbing** (v3.0b) — 1 PR, ~100 LOC.
   `SuggestPolicy` lands on `SuggestContext`. Default `Conservative` for
   this PR so the literature matrix doesn't shift.
3. **Default flip to `SuggestAggressiveness::Default`** (v3.0c) — 1 PR.
   Re-baseline `wanaka_suggest_integration.rs` and any matrix cells that
   move. Hold separately so the bisect on "matrix anti-pattern fires"
   isolates this commit cleanly.
4. **`SuggestRationale` + GUI section** (v3.1) — 1 PR, ~250 LOC core +
   ~150 LOC GUI. No semantic change; pure surfacing.
5. **MCP `get_suggest_rationale`** (v3.2) — 1 PR, ~80 LOC. Pairs with the
   rationale type.
6. **Strategy-aware passes** (v3.3) — 1 PR per pass:
   - `pick_adaptive3d_entry_style` (~120 LOC)
   - `pick_stock_to_leave` (~150 LOC, requires `upstream_leftover_stock_mm`
     wiring across the session callsites)
   - `pick_adaptive3d_clearing_strategy` (~80 LOC, ships warn-only; auto-
     rewrite is v4 once classifier is calibrated)
7. **GeometryClass classifier** (v3.4) — module `feeds::geometry_class`
   landed at v3.3 stub stage if we ship strategy first; full module
   when the classifier earns its keep.

Drop any of 5–7 if the budget runs out — 1–4 alone discharge two of the
three directive goals (median default + warning surface). Strategy is
the iceberg; cut it down the middle if needed.

## Test surface

### Existing sentries that must keep passing

- `wanaka_suggest_integration.rs` — re-baseline at v3.0c (median default).
- Literature matrix (`_litmatrix_*`) — rerun at v3.0c; expect cell
  movement; document which cells shift in the v3.0c PR description.
- `crates/rs_cam_core/tests/drill_metrics_pr2.rs` — drill ops are
  policy-invariant; this must stay flat.

### New sentries

- `feeds::suggest::tests::aggressiveness_monotone` — same op/tool, all
  three policies, feeds strictly increasing.
- `feeds::suggest::tests::speed_gated_by_deflection_fires` — synthetic
  long-stickout setup where `Speed` trips the deflection cap.
- `feeds::suggest::rationale::tests::from_warnings_round_trip` — for
  each `SuggestWarning` variant, assert the rationale conversion emits
  the right `(param, reason, from, to)` tuple.
- `feeds::suggest::tests::strategy_pinning_respected` — set
  `entry_style = Ramp` manually on an op where the geometry chooser
  would pick Helix; assert Ramp survives, `StrategyRewrote` doesn't
  fire.

### MCP smoke

- After v3.2: `get_suggest_rationale(4)` on wanaka returns at least
  `DppCappedByDeflection` and `FeedRaisedForChipload` entries; agent
  can read them.

## Open questions

1. **Median calibration on Default-rated arc-fit families.** Today's
   feed-up loop only fires on `ArcFitRatioSource::Calibrated` ops
   (Adaptive3d + DropCutter). With median targeting, the safe-default
   ratios on other families would still push observed above the matrix
   anti-patterns. Resolution: hold the Calibrated-only gate through v3;
   broaden as more cells get measured (per the v1 design doc's
   "broader op families gain Wanaka cells" note).

2. **Speed-mode default for first-time users.** Hard to defend — they
   don't have the deflection / chipload intuition to know what "Speed"
   trades off. v3 keeps `Default = Default` (Median). Reaching for
   `Speed` is an explicit op-by-op choice in the GUI, or a CLI flag
   `--aggressiveness=speed`.

3. **Geometry classifier on STEP B-rep models.** Slope histogram on
   B-rep is a different code path than triangle mesh. v3 ships the
   classifier on STL/SVG/DXF; STEP routes through a degraded heuristic
   (face-type histogram → MixedTerrain) until a B-rep slope analyzer
   lands. Tag in `RationaleEntry::detail` so the operator sees the
   classifier degraded.

4. **`upstream_leftover_stock_mm` source-of-truth.** Today it's `None`
   everywhere (CLI helper, integration test). v3.3 needs a session-side
   helper `ProjectSession::leftover_stock_for(tp_id)` that walks the
   ops in the setup and computes the residual. Out-of-scope for this
   doc; lives in the v3.3 PR.

5. **Should `SuggestScope::FeedsOnly` be exposed in the GUI?** Power-user
   surface that hides functionality from new users. Leaning yes (small
   "Suggest mode" dropdown next to the button), but kept off the v3.1
   GUI PR — defer to a v3.1.1 polish.

## Risks

- **Default flip blast radius**: changing the default from `Conservative`
  to `Default` will move every test that asserts on Suggest output.
  Mitigated by phasing — v3.0b lands the enum with Conservative as
  default, v3.0c flips, the bisect is one commit.
- **Strategy stomp on real projects**: even with the pin-via-default
  heuristic, projects loaded from older sessions where users *did* tune
  `clearing_strategy = AgentSearch` deliberately could see it rewritten
  to `ContourParallel` on Suggest. Mitigated by v3.3 only rewriting
  fields where the *current* value equals `Default::default()`.
  Aggressive auto-rewrite stays opt-in via `SuggestScope::StrategyAndFeeds`.
- **Rationale tree drift**: adding new `SuggestWarning` variants
  without updating `RationaleEntry::from_warnings` produces an empty
  rationale section instead of an error. Mitigated by an exhaustive
  match (no `_` arm) in the conversion.

## Next actions

1. Restore `ChiploadBounds::max_mm_per_tooth` and thread it through
   (v3.0a). Tests required: `wanaka_suggest_integration` green,
   literature matrix green, no behaviour change because targeting still
   uses `min`.
2. Add `SuggestPolicy` + `SuggestAggressiveness`, default Conservative,
   plumb through `SuggestContext` (v3.0b). Tests required: existing
   suite green, new `aggressiveness_monotone` unit test green.
3. Flip default to `SuggestAggressiveness::Default` (v3.0c). Re-run
   wanaka integration + literature matrix; re-baseline whatever moves;
   describe the moves in the PR.
4. Land `SuggestRationale` + GUI section (v3.1). Wanaka MCP screenshot
   should show the rationale entries in the modal.
5. Land MCP `get_suggest_rationale` (v3.2).
6. Strategy-aware passes (v3.3+) — break out of the v3 design once 1–5
   are merged and re-evaluate scope.
