# Combined Suggest — Design Doc

**Date**: 2026-06-03
**Trigger**: Wanaka air-run validation surfaced three failure modes when
the Feeds-Suggest button was applied verbatim: a deflection-critical
DPP on Back Rough, a zero-cut path on 3D Rough 6, and an unworkable
0.03 mm stepover on 3D Finish. Diagnostics caught all three *after*
generation — but Suggest had already written values the diagnostics
would immediately reject. The two systems clearly need to be one.

## Problem statement

The GUI exposes two parameter surfaces for any toolpath:

1. **Toolpath params panel** — strategy / entry / engagement geometry
   (`clearing_strategy`, `entry_style`, `stock_to_leave_*`, `tolerance`,
   `helix_pitch`, etc.). User-curated.
2. **Feeds & Speeds modal** with a "Suggest" button — feed_rate,
   plunge_rate, stepover, depth_per_pass, spindle_rpm. LUT-driven.

These two surfaces are intrinsically coupled but architecturally
independent. Suggest is a single-pass calculator (LUT → rigidity-factor
clamp → write). Every other constraint — deflection, chipload-vs-LUT,
runtime sanity, strategy/engagement compatibility — lives in
post-simulation verifiers. So Suggest can (and does) ship values the
diagnostics will immediately flag as critical.

### Concrete evidence (wanaka_suggested.toml run)

| Toolpath | Suggest wrote | Verifier said |
|---|---|---|
| Back Rough (3D Rough, 6 mm endmill) | DPP=9 mm (1.5× dia, machine `adaptive_doc_factor`) | Deflection 358 µm > 200 µm critical |
| 3D Rough 6 (same) | DPP=9 mm, `stock_to_leave_axial=0.5`, `entry_style=plunge`, `clearing_strategy=agent_search` | 25 moves, **0 mm cutting** — strategy can't find entry at this DPP/leave combo |
| 3D Finish 6 (tapered ball drop_cutter) | stepover=0.03 mm (scallop-height math literal) | Would emit ~4.6 M passes; generation effectively blocked |

In each case the *verifier* is correct. The bug is that Suggest
doesn't *consult* the verifier before writing.

## The hidden dependency graph

Parameters today's Suggest writes vs gates today's diagnostics check:

```
                        ┌─────────────────────────────┐
  Suggest writes ───►   │  feed_rate                  │
  (LUT + rigidity       │  plunge_rate                │
   clamp)               │  stepover         ◄────────┐│
                        │  depth_per_pass   ◄───────┐││
                        │  spindle_rpm              │││
                        └─────────────────────────────┘
                                                    │││
  Verifier reads:                                   │││
   • deflection_gate(DPP, stepover, material) ──────┼┼┘
   • chipload_gate(feed, RPM, arc-fit)        ─────┘│
   • runtime_estimate(stepover, model_bbox)  ──────┘
   • strategy_compatibility(DPP, stock_to_leave, entry_style) ──► NEVER consulted by Suggest

  Suggest does NOT write:
   • clearing_strategy
   • entry_style
   • stock_to_leave_*
   • tolerance
   • min_region_cut_length_mm
   • helix_pitch, ramp_angle_deg
   • DropCutter slope_from / slope_to / scallop_height
   • Drill cycle, peck_depth (partly — apply_drill_defaults handles peck)
```

Every dashed arrow is a coupling the user has to maintain manually
today.

## Design implications

### 1. Suggest becomes iterative, not closed-form

Today: propose → clamp → write. One pass.

Combined Suggest: propose → forward-check → back off → re-propose →
converge. Implies:

- **Convergence criterion**: all forward-predicted gates within some
  margin (e.g. 80 % of threshold) of passing, or N=5 iterations,
  whichever first.
- **Back-off priority**: deterministic so Suggest is idempotent.
  Proposed order:
  1. DPP (most physically impactful on deflection)
  2. Stepover (impacts both deflection and runtime)
  3. Feed (impacts chipload directly; back off last because reducing
     feed only helps if chipload exceeds LUT *high* — the more common
     failure is chipload *low*, which back-off worsens)
  4. RPM (rarely the right knob; spindle strategy is already a
     deliberate choice)
- **Idempotency**: same input → same iterations → same output.
  Required for reliable tests + agent-driven workflows.

### 2. Gate verifiers need forward-callable variants

The deflection model in `tool_load/deflection.rs` consumes post-sim
sample arrays. It already encodes the cantilever-beam math. To
consult it at Suggest-time we need a closed-form sibling:

```rust
fn predict_peak_deflection(
    op: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> DeflectionPrediction {
    // F_lateral = Kc × ap × ae × chipload_per_tooth × grain_factor
    // δ_tip    = F × L_stickout³ / (3 × E × I_eff)
    // I_eff    = π × d_core⁴ / 64 (account for flute relief)
    ...
}
```

Same skeleton for `predict_chipload_at_arc` (returns nominal +
arc-corrected) and `predict_move_count` (returns
`moves_estimate(stepover, bbox)`).

These are *new public functions* that share the underlying math with
the post-sim gates. The post-sim gate stays canonical; the forward
predictor is its first-order approximation, tested against
post-sim observation across the literature matrix.

### 3. Output becomes a rationale tree

Today `SuggestedParams.warnings: Vec<SuggestWarning>` is flat. A
combined Suggest needs structured rationale:

```rust
pub struct SuggestionReason {
    pub param: ParamId,
    pub from: f64,
    pub to: f64,
    pub trigger: Trigger,  // LutClamp | DeflectionPredict | RuntimeFloor | RigidityFactor | UserPin
    pub evidence: Option<Evidence>, // LUT row, predicted µm, predicted move count
}

pub struct SuggestionResult {
    pub operation: OperationConfig,
    pub reasons: Vec<SuggestionReason>,
    pub unmet_gates: Vec<UnmetGate>,  // gates that still fail after iteration
    pub iterations: u8,
}
```

The MCP `diagnostic_delta` and the GUI Feeds modal can both render
this — "DPP capped to 4 mm because Suggest wanted 9, deflection-predict
said 358 µm at 9, backed off ×0.8 until 158 µm at 4."

### 4. Session context dependency

Combined Suggest can't be a pure
`(op, tool, machine, material) → params` function any more. It needs:

- **Model bbox + feature density** (for strategy + runtime estimation)
- **Prior toolpath's leftover stock** (for finish DPP — the rough
  toolpath ahead of this one constrains how deep a finish pass can go)
- **Stock height vs DPP** (room for at least one pass)

Two paths:

- **A. Pass `&ProjectSession`** to Suggest. Easy, but couples Suggest
  to session structure and worsens testability.
- **B. Pass `SuggestContext` struct** explicitly:

  ```rust
  pub struct SuggestContext<'a> {
      pub model_bbox: Option<Aabb>,
      pub stock: &'a StockContext,
      pub upstream_leftover_stock_mm: Option<f64>, // None for first op
      pub neighboring_strategy_hint: Option<ClearingStrategy>, // for downstream finish
  }
  ```

  Caller (CLI, MCP, GUI) builds the context from session state.
  Suggest itself stays a pure function. **Preferred.**

### 5. Scope-controlled UX

Machinists treat "feeds & speeds" and "strategy & engagement" as
different *intentions*. Collapsing the GUI buttons risks
"I bumped RPM and it changed my entry style." Resolution:

```rust
pub enum SuggestScope {
    /// Today's behavior. Preserved for back-compat and power users.
    FeedsOnly,
    /// Feeds + gate forward-prediction. DPP/stepover/feed back off if
    /// forward predicts a deflection/chipload/runtime gate failure.
    FeedsWithGates,
    /// Above + strategy/entry/stock_to_leave selection based on model
    /// geometry classification. V2.
    StrategyAndFeeds,
}
```

Default to `FeedsWithGates`. The GUI button still says "Suggest" but
under the hood does the verification loop.

**User-pinned params**: if the user manually set `entry_style = plunge`,
combined Suggest should not stomp it. Piggyback on the existing
`stale_defaults` machinery — a value the user touched is pinned;
untouched values are fair game. Pinned params appear in the rationale
tree as `Trigger::UserPin`.

### 6. The "infeasible" outcome

What if no parameter combo passes all forward-predicted gates? Two
choices:

- **Refuse silently** — don't write, return error. Risk: user has no
  guidance on what to relax.
- **Best-effort with explicit `unmet_gates`** — write the
  most-conservative values we could find; surface which gates still
  fail and what input would unblock them ("relax workholding rigidity",
  "use shorter tool", "use stiffer material"). **Preferred.**

The `unmet_gates` field is distinct from `warnings` — warnings are
informational ("we capped X for Y"); unmet_gates are red flags
("this combo can't be made safe by tweaking feeds alone").

## Migration

- **Keep `feeds::suggest_for_operation`** as the primitive. It stays
  the LUT-only path.
- **Add `feeds::suggest::suggest_full(scope, context)`** that composes
  the primitive with gate forward-prediction.
- **CLI** `--apply-suggest` becomes `--apply-suggest=feeds|gates|full`,
  default `gates`.
- **GUI** Feeds modal's Suggest button calls `suggest_full(Gates)`.
  V2 adds a separate Strategy-Suggest path (not part of v1).
- **MCP** new tool `suggest_toolpath(index, scope)` returns the
  `SuggestionResult` envelope. Pairs with `set_toolpath_param` for
  selective accept/reject of individual `SuggestionReason` entries.

## Test surface

The literature-matrix sentry suite (recently landed) is the natural
test scaffold. Each cell currently checks "feeds match LUT." Combined
Suggest extends this to:

1. Combined Suggest produces a parameter set whose post-sim verifier
   verdict is "Within" for chipload, deflection, drill gates.
2. Forward-prediction error vs post-sim observation stays within
   ±20 % across the matrix (regression: prevents the forward model
   from drifting away from the canonical post-sim gate).
3. Suggest is idempotent: running twice in succession produces no
   value changes.
4. Suggest is monotone in workholding: tightening rigidity never
   loosens DPP/stepover.

Per-function unit tests:

- `predict_peak_deflection` — cantilever beam math vs known cases.
- `predict_chipload_at_arc` — nominal + arc-fit ratio per
  `op_kinematics_class`.
- `predict_move_count` — bbox × stepover envelope, validated against
  small generated cases (DropCutter on synthetic terrain).

## Pragmatic v1 cut

Don't try to land all of this. Three additions to `feeds/suggest.rs`,
each independently testable, that fix the three wanaka findings:

### v1.1 — Deflection-aware DPP back-off

```rust
fn predict_peak_deflection_um(...) -> f64 { ... }

// inside enforce_invariants for Roughing pass_role:
let mut dpp = current;
for _ in 0..5 {
    let predicted = predict_peak_deflection_um(op, tool, material, machine);
    if predicted <= 0.8 * DEFLECTION_EXCEEDS_THRESHOLD_UM { break; }
    dpp *= 0.8;
    operation.set_depth_per_pass(dpp);
}
warnings.push(SuggestWarning::DppCappedByDeflection { ... });
```

Fixes Back Rough.

### v1.2 — Runtime-sanity stepover floor

```rust
fn predict_move_count(stepover_mm, model_bbox, op_kind) -> u64 { ... }

// after stepover is written:
if predict_move_count(stepover, bbox, op_kind) > 500_000 {
    // raise stepover until predict ≤ threshold (clamp to scallop-height
    // floor if scallop targeting is in play)
    ...
    warnings.push(SuggestWarning::StepoverRaisedForRuntime { ... });
}
```

Fixes 3D Finish.

### v1.3 — Strategy/DPP compatibility warning (not fix)

```rust
// inside enforce_invariants for Adaptive feeds_family with plunge entry:
if matches!(op.entry_style(), Some(EntryStyle::Plunge))
    && dpp > 1.0 * tool.diameter {
    warnings.push(SuggestWarning::PlungeEntryUnstableAtDpp { dpp, diameter });
}
```

Fixes 3D Rough 6 *as a surfaced warning*. Full auto-strategy
selection is v2.

### Out of scope for v1

- Strategy/entry_style/stock_to_leave auto-selection
- Model-geometry classifier
- Cross-toolpath context (finish-pass DPP from rough leftover)
- New MCP tool surface (use the existing Suggest path with the
  v1 changes; structured rationale tree is v2)

## Risks & open questions

- **Forward-prediction drift**: the closed-form deflection predictor
  needs ±20 % agreement with the post-sim gate, validated across the
  matrix. If drift exceeds that, gate firings will surprise users.
- **Iteration count budget**: 5 iterations × N toolpaths × matrix
  cells = test runtime. Need to verify the convergence proof holds
  (back-off is strictly monotone in DPP-bound case).
- **Plunge-feed vs Suggest**: drill cycles already have an
  `apply_drill_defaults` path that scales peck_depth with diameter.
  Combined Suggest should incorporate this rather than duplicate it.
- **Chipload-low back-off direction**: today's Suggest aims at the LUT
  *minimum*, not the median. When the arc-fit ratio pushes observed
  below LUT min, the obvious back-off (raise feed) breaks the
  rigidity-clamped DPP × stepover budget. May need to relax DPP
  instead — non-obvious, needs experiment.
- **Scallop-height vs runtime floor**: 3D Finish drop_cutter has a
  scallop-height target. Raising stepover to satisfy the runtime
  floor degrades scallop. Need an explicit
  `runtime_vs_scallop_tradeoff` knob with a sane default.

## Next actions

1. Land `predict_peak_deflection` as a public function with unit
   tests against the existing post-sim gate (sample 10 cells from
   the literature matrix; predicted vs observed within ±20 %).
2. Wire it into `enforce_invariants` per v1.1.
3. Re-run the wanaka_suggested.toml validation. Expected outcome:
   Back Rough DPP comes down to ~4 mm; deflection within bounds;
   `SuggestWarning::DppCappedByDeflection` surfaced.
4. v1.2 stepover/runtime — same pattern, smaller blast radius.
5. v1.3 plunge-entry warning — one-line check + warning variant.
6. Update `wanaka_suggested.toml` and re-run; air-run gates should
   pass on suggested values without manual intervention.
