# Toolpath Spans Design

**Status**: Proposed (2026-05-07)
**Replaces**: ad-hoc `OperationAnnotations` enum in `crates/rs_cam_core/src/compute/execute.rs`
**Motivates**: prevents the bug class that produced the wanaka Back Rough z=7 over-cut (TSP reordered cuts across depth-pass boundaries because the depth-pass structure was sidecar metadata that TSP didn't see).

---

## Problem statement

The pipeline today emits a `Toolpath` (sequence of moves) plus an `OperationAnnotations` sidecar (op-specific enum). Dressup transforms (`apply_link_moves`, `optimize_rapid_order`, `apply_dogbones`, `fit_arcs`, boundary clip, `filter_air_cuts`) mutate the move list **without updating the annotations**. Anything downstream that reads annotations sees stale move indices.

Concrete failure modes this has produced:
- TSP reordered adaptive3d cuts across depth-pass boundaries → wanaka 18mm full-depth pass-1 bite (fixed in commit 01727e1, but only because `rapid_order_barriers` was special-cased for Adaptive3d in `execute.rs:50-73`).
- Per-op sidecar means RampFinish/Scallop/SpiralFinish/Pencil emit annotations but their `rapid_order_barriers()` returns empty (audit FT-2). The infrastructure exists but is wired only for one op.
- GUI tracing builds a separate semantic trace before dressups; after dressups, that trace's move ranges are stale (`crates/rs_cam_viz/src/compute/worker/helpers.rs`).

The invariant we want — and the one task #41 enforces via tests — is:

> Any transform that changes toolpath move order or move count must either preserve semantic spans (by remapping their move indices) or explicitly mark them invalidated.

---

## Data model

Add `crates/rs_cam_core/src/toolpath_spans.rs` and re-export it from `lib.rs`.

Do **not** convert `toolpath.rs` into a module in the first commit; keeping spans in a sibling module avoids broad import churn while the model settles.

```rust
use std::borrow::Cow;
use std::ops::Range;

/// A range of moves [start..end) within a Toolpath, tagged with semantic info.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start_move: usize,  // inclusive
    pub end_move: usize,    // exclusive
    pub kind: SpanKind,
    pub label: Cow<'static, str>,
    pub payload: Option<SpanPayload>,
}

/// Categorical type for what this span represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanKind {
    /// Wraps every move of one operation. Always present at the top level.
    Operation,
    /// Moves at a single Z depth in a multi-pass operation.
    DepthPass,
    /// A region (closed polygon area or chain) within an op.
    Region,
    /// An entry / lead-in transition (rapid + plunge or ramp/helix).
    Entry,
    /// A lead-out transition.
    LeadOut,
    /// A linker bridge inserted by apply_link_moves.
    LinkBridge,
    /// A dressup-introduced segment (dogbone, arc-fit replacement).
    DressupArtifact,
    /// Hard barrier before `start_move`: TSP must not reorder across this
    /// move boundary. Zero-width span: start_move == end_move.
    RapidOrderBarrier,
}

/// Optional structured payload for span-specific data.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanPayload {
    DepthPass { z_level: f64, pass_index: u32 },
    Region { region_id: u32 },
    Entry { kind: EntryKind },
    // ... add as ops need them. Most spans don't need a payload.
}

/// Toolpath bundled with optional semantic spans.
#[derive(Debug, Clone)]
pub struct AnnotatedToolpath {
    pub toolpath: Toolpath,
    pub spans: Vec<Span>,
    /// Set false when a transform invalidated spans without remapping.
    /// Downstream code MUST honor this and not rely on span.start_move/end_move.
    pub spans_valid: bool,
}

impl AnnotatedToolpath {
    pub fn new(toolpath: Toolpath) -> Self;
    pub fn with_spans(toolpath: Toolpath, spans: Vec<Span>) -> Self;

    /// All spans (regardless of kind) covering this move index.
    pub fn spans_at(&self, move_idx: usize) -> impl Iterator<Item = &Span>;

    /// All spans of the given kind, in order.
    pub fn spans_of_kind(&self, kind: SpanKind) -> impl Iterator<Item = &Span>;

    /// Move-boundary indices that act as TSP barriers (RapidOrderBarrier
    /// spans + DepthPass starts). Index 0 means before the first move;
    /// index toolpath.moves.len() means after the last move.
    pub fn rapid_order_barriers(&self) -> Vec<usize>;

    /// Validate that all span ranges are in-bounds and ordered. Used in debug assertions.
    pub fn check_invariants(&self) -> Result<(), SpanInvariantViolation>;
}

/// Explicit mapping from old move indices to post-transform move ranges.
/// A transform that changes move count/order should return one of these
/// alongside the new Toolpath so spans can be remapped mechanically.
pub struct MoveRemap {
    pub old_to_new: Vec<Option<Range<usize>>>,
}
```

Design choices:

1. **Spans live separately from `Toolpath`.** `Toolpath` itself does not change. Most code keeps using `&Toolpath` or `&[Move]`. Span-aware code uses `AnnotatedToolpath`. This keeps churn bounded.

2. **Spans are half-open `[start..end)`.** This matches Rust slices, naturally represents empty toolpaths as `[0..0)`, and avoids off-by-one ambiguity when transforms insert/delete moves. A `RapidOrderBarrier` is a zero-width boundary span `[i..i)`.

3. **Spans can overlap.** An `Operation` span covers all moves of one op. `DepthPass` spans inside it cover sub-ranges. `Entry` spans further inside. `RapidOrderBarrier` spans are zero-width and sit at the boundary before a `DepthPass`/protected group.

4. **Generic `SpanKind`, op data in `SpanPayload`.** We do not embed Adaptive3dRuntimeAnnotation, ScallopRuntimeAnnotation etc. in the span. Op-specific structured data goes in `SpanPayload` only when needed (depth pass z value, region id). Most ops don't need a payload.

5. **`spans_valid` escape hatch.** Boundary clipping is hard to span-remap because it can split, drop, or fragment moves arbitrarily. Rather than force every transform to do precise remapping, we let "I don't know what these spans mean anymore" be a first-class state. Downstream code that needs spans checks the flag. Tests should assert `spans_valid == true` after the standard dressup pipeline **before boundary clipping**; boundary clipping may intentionally mark spans invalid in the first implementation.

6. **Backward compatibility.** The existing `OperationAnnotations` enum and `pub struct AnnotatedToolpath { toolpath, annotations }` shell stay during transition. A new `AnnotatedToolpath` (with spans) replaces them in execute.rs / apply_dressups in one focused commit. The op-specific RuntimeAnnotation types remain for narrate / debug / GUI tracing, populated alongside spans during the transition; deleted when nothing reads them.

---

## Phasing

### Phase 1: Type definitions only (#42-1)

- Add `crates/rs_cam_core/src/toolpath_spans.rs` with `Span`, `SpanKind`, `SpanPayload`, `AnnotatedToolpath`, `MoveRemap`, `check_invariants`.
- No other code changes.
- Test: round-trip construction, `spans_at`, `spans_of_kind`, `rapid_order_barriers` derivation, `check_invariants` catches out-of-bounds and inverted ranges.

### Phase 2: Convert at the execute boundary (#43)

- In `compute/execute.rs`, after each generator runs, build the `Vec<Span>` from its `OperationAnnotations`.
- Adaptive3d's `Adaptive3dRuntimeEvent::{RegionZLevel, GlobalZLevel, WaterlineCleanup}` → zero-width `RapidOrderBarrier` spans at the group start boundary (replacing the existing `rapid_order_barriers()` Vec<usize> path).
- Adaptive3d's `RegionStart`, `PassEntry`, `PassSummary` → `Region`/`DepthPass` spans.
- Best-effort spans for Scallop, RampFinish, SpiralFinish, Pencil (whatever their existing annotations carry).
- Wrap each operation in a top-level `Operation` span.
- For ops returning `OperationAnnotations::None`, return an `AnnotatedToolpath` with one `Operation` span and `spans_valid: true`.
- Plumb `Vec<Span>` through `apply_dressups` (replacing the `&[usize]` `rapid_order_barriers` arg).

### Phase 3: Span-aware dressups (#45)

One sub-task per dressup so each can be reviewed independently:

- **3a** `apply_entry`: prepend moves at op start; shift all spans by entry-move count; tag the new moves with an `Entry` span.
- **3b** `apply_dogbones`: insert moves at corner indices; build move-index remap; rewrite span ranges through the remap.
- **3c** `apply_lead_in_out`: prepend/append moves at cut-segment boundaries; remap spans.
- **3d** `apply_link_moves`: replace runs of (retract, rapid, plunge) with a single linear; build remap; **honor span boundaries** (do not link across `RapidOrderBarrier` or `DepthPass` boundaries; this is the wanaka safety guarantee generalized).
- **3e** `fit_arcs`: replace N moves with 1 arc; remap.
- **3f** `optimize_rapid_order` / `optimize_rapid_order_with_barriers`: derive barriers from `AnnotatedToolpath::rapid_order_barriers()` instead of the explicit `&[usize]` arg; permute cut-segment groups; rewrite span ranges through the permutation.
- **3g** `filter_air_cuts`: drop moves; remap.
- **3h** `optimize_feed_rates`: no move-count change; preserves spans by construction; assert.
- **3i** `boundary_clip`: hard case. First pass: set `spans_valid = false` and warn in tracing. Defer precise remap.

Each sub-task adds a focused test that:
1. Builds a synthetic `AnnotatedToolpath` with known spans.
2. Runs the dressup.
3. Asserts `check_invariants()` passes.
4. Asserts spans cover the expected post-transform move ranges.

### Phase 4: Centralize dressup pipeline (#44)

After all dressups are span-aware, replace the duplicate `apply_dressups` in `crates/rs_cam_viz/src/compute/worker/helpers.rs` with a thin wrapper around the core pipeline that adds GUI tracing hooks. Drift risk gone.

### Phase 5: Cross-cutting invariant test (#46)

For every op type, build a minimal fixture, run the full dressup pipeline, assert `check_invariants()` and that span kinds match expectations. Already-loose-fit ops (Trace, VCarve, Chamfer, Inlay, Face, RadialFinish) get explicit `link_moves: true` coverage here too.

### Phase 6: Wanaka MCP validation (#47)

Reload wanaka, regen Back Rough, sim, confirm pass-1 still ~40 cm³ / 3mm DOC. Catches any regression from the refactor.

---

## Out of scope (for now)

- Removing `OperationAnnotations` entirely — keep it during transition for narrate/debug consumers. Delete in a separate follow-up once nothing reads it.
- Persisting spans to disk (project-IO). Span data is post-generation-only; it's reconstructed each time the toolpath regenerates.
- GUI rendering of spans. The existing `ToolpathSemanticKind` UI tracing hierarchy is a separate concern; spans inform but do not replace it (yet).
- A SpanPayload variant for every op. Add per-payload as concrete consumers need them.

---

## Open questions

- Should `apply_link_moves` ever be allowed to bridge across a `DepthPass` boundary? Current answer: no, by default. If a future use case wants it, add a per-link-bridge override.
- Should `RapidOrderBarrier` and `DepthPass` boundaries be unified? Currently `rapid_order_barriers()` derives from both. Could collapse if no consumer needs them apart.
- How should boundary-clip eventually remap? One option: clip operates per-span, producing a new `Vec<Span>` from the remaining moves in each input span. Defer until we have a consumer that actually needs valid spans post-clip.
