# `cavalier_contours` `Shape` failure — reproduction, mapping, and proposed contract

Date: 2026-08-04
Wave: W4 (lane C — 2D geometry), plan item **R2-H2**
Revision surveyed: `0e7d38b` (branch `experiment/adaptive-spiral`)
Status: **RESEARCH ONLY.** No failure contract is implemented. §6 is the
decision list for **Checkpoint C**.
Instrument: `crates/rs_cam_core/tests/cavalier_shape_failure_r2.rs`

---

## 1. The inherited item, restated

> `cavalier` `Shape` panic mapping — panic can become a silent collapsed
> offset. **R2-H2**, adversarial reproduction and explicit failure policy.
> — plan §1.1

And the plan's own bar:

> Replace "collapsed offset" masking with a typed, surfaced generation
> finding/error only after the failure contract is approved. **Do not turn a
> panic into a clean-looking empty path.**
> — plan §H3 fix-shape 4

---

## 2. Library and version context

| Item | Value |
|---|---|
| Declared | `Cargo.toml:30` — `cavalier_contours = "0.7"` (workspace dep) |
| Resolved | `Cargo.lock:742-748` — **0.7.0**, sha256 `31cab9e73a5a3533d3d6fb36818e8735dd033e080fa70a63d90846c8183708c9` |
| Consumed by | `crates/rs_cam_core/Cargo.toml:23` only — one dependent in the whole workspace |
| Source | `~/.cargo/registry/src/index.crates.io-.../cavalier_contours-0.7.0` (not vendored) |
| Unwinding | `Cargo.toml:85-92` keeps `panic = "unwind"` in `[profile.release]`, and the comment names `polygon::offset_polygon`'s containment as a reason. **Without that, every `catch_unwind` below is dead code.** |

**Four** call sites reach the library's offset:

| `polygon.rs` | Call | Path |
|---|---|---|
| :388 | `pline.parallel_offset(distance)` | single-shot, no holes |
| :404 | `shape.parallel_offset(distance, Default::default())` | single-shot, **holes — the panicking one** |
| :797 | `boundary.parallel_offset(distance)` | cascade (`RingGroup::offset`), no holes |
| :803 | `shape.parallel_offset(distance, Default::default())` | cascade, holes |

Supporting calls: `remove_repeat_pos` (:317), `remove_redundant` (:369, :837),
`arcs_to_approx_lines` (:889).

---

## 3. The failure sites inside cavalier 0.7.0

### 3.1 The one the captured asset trips — and it is a `debug_assert!`

`src/polyline/pline_view.rs:507-510`, in `PlineViewData::from_slice_points`:

```rust
debug_assert!(
    start_index <= end_index || source.is_closed(),
    "start index should be less than or equal to end index if polyline is open"
);
```

Reached from `Shape::parallel_offset`'s slice dissection. This is the exact
message `polygon.rs:279-280` quotes.

**It is a `debug_assert!`.** In a release build the invariant is not checked,
`from_slice_points` proceeds with `start_index > end_index`, and the
`catch_unwind` at `polygon.rs:289` never fires for that path. So for the
primary captured class the containment is a **debug/test-only net**, and what
release produces instead is *unvalidated* — a malformed slice stitched into a
ring, rather than a collapsed-empty result. That is arguably a worse outcome
than the one the containment is documented as giving, and it is not what the
"always under-cut, never a gouge" safety argument describes.

This was already known and written down at the time:

> Both cavalier asserts are `debug_assert!` (dev/test crash, release silently
> proceeds…) — `planning/TECH_DEBT_REVIEW_2026-06-10.md:43-44`

### 3.2 The sites that DO panic in a release build

| File:line | Construct | Trigger |
|---|---|---|
| `shape_algorithms/mod.rs:786-788` | `unreachable!("loop_count exceeded max_loop_count while stitching slices together")` | the stitching loop cannot close a ring — **the classic hostile-input hard panic** |
| `polyline/internal/pline_offset.rs:1401` | same `unreachable!`, single-polyline path | same, no-holes branch |
| `pline_view.rs:316-319` | `assert!(traverse_count != 0, …)` | `PlineViewData::create` |
| `pline_view.rs:374-377` | `assert!(vc >= 2, "source must have at least 2 vertexes…")` | `from_entire_pline` |
| `pline_view.rs:438-441` | `assert!(vc >= 2, …)` | sibling constructor |

### 3.3 The rest of `Shape::parallel_offset`'s panic surface (debug + `unwrap`/`expect`/indexing, which fire in release)

`shape_algorithms/mod.rs`: `:173`, `:317`, `:372`, `:855` `.expect("expect
non-empty polyline")`; `:182`, `:765`, `:864` `build().unwrap()`; `:198`
empty-shape build; `:327` `.expect("failed to build spatial index of offset
loop bounds")`; `:569`, `:773`, `:776`, `:792`, `:842` raw indexing;
`:675-676` `sorted_intrs.last().unwrap()` / `&sorted_intrs[0]` on a possibly
empty intersect list; `:840` `query_results[0]`; `:611` `debug_assert!`.

`pline_offset.rs` `debug_assert!`s at `:85`, `:166`, `:225`, `:318`, `:406`,
`:702`, `:1488`, `:1533` — one of which is the *"input assumed to not have
repeat position vertexes"* class that R1 root-fixed with `dedupe_pline`.

**The indexing and `unwrap` sites are the release-relevant majority.** A
census of the debug assertions alone understates the release exposure.

---

## 4. Reproduction — measured

Run: `cargo test -p rs_cam_core --test cavalier_shape_failure_r2 -- --nocapture --test-threads=1`
at `0e7d38b`, debug. **4 passed, 0 failed, 1 ignored, 0.67 s.**

### R-1 — the asset is still what it was captured as: **PINNED**

86-vertex exterior, 13 holes, distance `5.529999999999999`, holes present so
the `Shape` path is taken. And one fact that was *not* previously recorded
anywhere: **the asset is self-intersecting** (`has_self_intersection() == true`).

### R-2 — the panic, reproduced verbatim: **REPRODUCED**

```text
direct cavalier call on the R1 asset:
PANIC @ .../cavalier_contours-0.7.0/src/polyline/pline_view.rs:507:
start index should be less than or equal to end index if polyline is open
```

Exactly the assertion `polygon.rs:279-280` names, exactly where it says, with
the source location recovered — which the production `Err(_payload)` arm
throws away (§5.2 property 1, decision D-4).

### R-2b — **but production no longer takes that path**, and this is the correction the wave produced

`offset_polygon` on the same asset at the same distance returns **1 polygon**,
not an empty `Vec`. The reason is R1.5 (2026-07-06): the asset is
self-intersecting, so `Polygon2::repaired()` splits it *before* cavalier is
called, and the repaired pieces do not trip the assertion.

So the R1 `catch_unwind` is **not currently firing for the asset it was
written for.** R1's own note — *"the captured slice still asserts inside
cavalier 0.7.0 even with clean input, so containment is the only fix"* — was
true when written and has been superseded by the repair that landed a month
later. The containment is still load-bearing for other classes (R-4b below);
it is no longer load-bearing for this one.

This also means `offset_polygon_degenerate_inputs_r1`'s first test —
whose bar is "no panic, result may legitimately be empty" — has been passing
for a while without exercising the containment at all.

### R-3 — synthetic many-hole stitching stress: **NOT REPRODUCED**

`synthetic_many_hole_shape_offset_is_reported_not_asserted`, a 100 mm square
with a 4×4 grid of Ø14 holes, offset inward at 1 / 3 / 6 / 6.5 / 7 / 12 mm —
distances chosen so neighbouring holes merge, then merge again, then swallow
the part:

| distance | direct cavalier | `offset_polygon` |
|---|---|---|
| +1.00 | ok (17 plines) | 1 poly |
| +3.00 | ok (16 plines) | 1 poly |
| +6.00 | ok (11 plines) | 10 polys |
| +6.50 | ok (10 plines) | 10 polys |
| +7.00 | ok (25 plines) | 25 polys |
| +12.00 | ok (0 plines) | 0 polys |

No panic at any distance. **Honest negative:** the stitching stress this
fixture was designed to apply does not reproduce the class. Whatever the R1
asset's exterior was doing, it was not simply "many holes merging".

### R-4 — nothing escapes the chokepoint: **GATED, PASSES**

22 fixtures × 4 distances × every ring through the production
`offset_polygon`: zero escapes. 26 `(polygon, distance)` pairs collapsed to an
empty `Vec` — 14 of them `tiny-islands` islands smaller than the tool (a
legitimate collapse), and 12 from the three invalid fixtures.

### R-4b — **a panic class R1 never saw, in a different crate**: `static_aabb2d_index`

The run tripped, four times, a site nobody had listed:

```text
.../static_aabb2d_index-2.0.0/src/static_aabb2d_index.rs:266:9:
assertion failed: min_x <= max_x
```

Four occurrences = the `invalid-nan` fixture at its four distances. **A single
`NaN` vertex propagates into cavalier's spatial-index build**, where the
bounding box's own sanity check rejects it. `offset_polygon` contained all
four and returned empty, so the gate passed — but the class is new, it is in a
*transitive* dependency (`cavalier_contours` → `static_aabb2d_index 2.0.0`),
and R1's census never mentioned it.

**And it has the same debug/release split, documented by the library itself**
(`static_aabb2d_index.rs:255-258`):

> For performance reasons the sanity checks of `min_x <= max_x` and
> `min_y <= max_y` are **only debug asserted**. If an invalid box is added it
> may lead to a panic **or unexpected behavior** from the constructed
> `StaticAABB2DIndex`.

In release, a `NaN` vertex therefore does not panic. It builds a corrupt
spatial index and the offset proceeds on it. That is the same shape as §3.1
and arguably worse: the library is telling us, in its own docs, that the
release path is undefined.

Nothing upstream of `offset_polygon` filters non-finite coordinates. There is
no validating constructor on `Polygon2` and no `Result` on the type
(`ADVERSARIAL_2D_FIXTURE_SPEC.md` §2.2).

### R-5 — release behaviour: **NOT EXERCISED**

Programme rule 11 forbids release builds in a research wave. The debug/release
divergence in §3.1 and R-4b is established by *reading* `debug_assert!` versus
`assert!`, not by running. This must not be reported as a pass.

---

## 5. The current mapping — what actually happens today

### 5.1 The two chokepoints

`polygon.rs:276-305` (`offset_one`, the single-shot path):

```rust
match std::panic::catch_unwind(AssertUnwindSafe(|| offset_polygon_inner(polygon, distance))) {
    Ok(v) => v,
    Err(_payload) => {
        tracing::warn!(distance, exterior_verts = …, holes = …,
            "offset_polygon: cavalier_contours panicked on degenerate input …
             treating as collapsed offset (empty result)");
        Vec::new()
    }
}
```

`polygon.rs:793-828` (`RingGroup::offset`, the cascade) is the same shape with
its own `warn!` and `return Vec::new()`.

### 5.2 Four properties of that mapping

1. **The payload is discarded.** `Err(_payload)` at both sites, so the `warn!`
   cannot name which assertion tripped — even though a working extractor,
   `panic_payload_message`, sits in the same crate at
   `tool_load/optimize/candidate.rs:342`.
2. **The `warn!` has no consumer.** It is not a `Diagnostic`, not a
   `ToolpathStats` finding, not narration, not an MCP field. Nothing on any
   operator or agent surface changes when it fires. **A warning nobody sees is
   not a warning** (programme rule 4).
3. **Three different events return the identical value.** A ring below the
   `< 3` guard (`polygon.rs:377`), a genuine geometric collapse, and a
   contained panic are all `Vec::new()` from `offset_polygon`. The return type
   `Vec<Polygon2>` has no channel that could carry the difference. Pinned by
   `a_contained_panic_is_indistinguishable_from_a_collapse`.
4. **Every 2D consumer maps empty onto a successful empty toolpath:**

   | Consumer | Code | Result |
   |---|---|---|
   | `trace.rs:54-66` | `if result.is_empty() { return Toolpath::new(); }` | empty toolpath, `Ok` |
   | `zigzag.rs:65-68` | `if inset.is_empty() { return Vec::new(); }` | no passes, `Ok` |
   | `profile.rs:78-86` | `.next().filter(len ≥ 3).map(…)` | `None` → "offset collapsed" |
   | `pocket.rs:90-94` | `for comp in &compensated { … }` | loop body never runs → zero contours |
   | `boundary.rs:36-42` | `Inside => offset_polygon(boundary, tool_radius)` | empty effective boundary → everything clipped away |
   | `adaptive3d/clearing.rs:1724,1818,1954` | direct consumption | region reads as not machinable |

### 5.3 The exception that breaks the safety argument — **the top R2 finding**

`polygon.rs:295-300` justifies the mapping as *"all under-cut directions,
never a gouge"*. At the **boundary-clip** layer it is the opposite.

`boundary::effective_boundary` (`boundary.rs:31-43`) is how a containment
setting becomes geometry:

```rust
ToolContainment::Inside  => offset_polygon(boundary,  tool_radius),  // shrink
ToolContainment::Outside => offset_polygon(boundary, -tool_radius),  // grow
```

An empty result there does not shrink the boundary to nothing. It removes the
**boundary clip entirely**, and both consumers say so in their own comments:

`crates/rs_cam_core/src/session/compute.rs:1793-1804`

```rust
let boundaries = effective_boundary(&stock_poly, containment, tool_radius);
// An empty `boundaries` means the boundary collapsed (e.g. the tool
// is larger than the stock): the set clipper passes the toolpath
// through with an identity mapping …
let clipped = clip_annotated_to_boundary_set(
    annotated,
    boundaries.first().map(std::slice::from_ref).unwrap_or(&[]),
    safe_z,
) …
```

`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:696-712` — the live GUI
worker — is the same shape as an `if let Some(boundary) = boundaries.first()`
whose `else` is *do not clip*.

And `boundary::clip_annotated_to_boundary_set` (`boundary.rs:79-106`) states
the contract:

> An EMPTY `boundaries` slice means the boundary collapsed (tool larger than
> the stock, every rest region eaten by the inset). That is not an error and
> not a clip: **the toolpath passes through** with an identity mapping.

**So: an operator sets `BoundaryContainment::Inside` — "keep the whole cutter
inside this boundary", a safety containment. `offset_polygon` is asked to
shrink the boundary by the tool radius. If cavalier panics on that boundary
polygon, the containment maps the panic to empty, the clip is skipped, and
the toolpath is emitted with no boundary containment at all.**

Two things make this the highest-severity row in R2:

1. **Reachability.** `BoundaryConfig` is a GUI dial with `containment` and
   `offset` fields (`compute/config.rs:1249-1256`), on the live worker's path.
2. **The captured panic asset is boundary-shaped.** `cavalier_panic_polygon_r1.json`
   is a WANAKA Back Rough terrain slice: 86-vertex exterior, **13 holes**,
   offset inward 5.53 mm. That is exactly the shape and exactly the operation
   a boundary containment performs.

The rationale is honest for the cause it names — "the tool is larger than the
stock", where nothing is machinable anyway and passing through at least does
not silently delete the path. What it does not do is distinguish that cause
from a library failure, because **nothing downstream can**: §5.2 property 3.

Related, smaller, and in the opposite direction: both single-region sites take
`boundaries.first()` only. A boundary that *splits* into several polygons
under the offset keeps piece 1 and clips everything outside it away — an
under-cut. The multi-region variant at `session/compute.rs:1881` does
`.flat_map(…)` and keeps them all, so the two paths disagree.

### 5.4 The other exception — a silently skipped offset

`polygon.rs:295-300` justifies the mapping as *"all under-cut directions,
never a gouge"*. Three sites do **not** collapse on empty — they silently skip
the offset and keep the un-offset polygon:

```rust
let offset_polys = offset_polygon(p, -req.boundary.offset);
if let Some(largest) = offset_polys.into_iter().max_by(area) {
    *p = largest;                 // on empty: `p` keeps its UN-OFFSET value
}
```

- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:169-176`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:680-687`
- `crates/rs_cam_core/src/session/compute.rs:1727-1732`

For a **negative** `BoundaryConfig::offset` — the operator shrinking the
machining boundary inwards, the protective direction — skipping it leaves the
toolpath clipped to a **larger** region than requested. That is an over-cut,
in the one direction the containment's own safety argument says cannot
happen. `BoundaryConfig::offset` is a GUI dial and both viz sites are on the
live worker's boundary path, so reachability is high.

Cross-referenced as finding **O-2** in `OFFSET_CONSUMER_ROLLOUT.md` §4; §5.3
is finding **O-4** and is ranked first in `ADVERSARIAL_2D_FINDINGS.md`.

---

## 6. Proposed failure contracts — the Checkpoint C decision list

Each option is stated with what it costs and what it forecloses. **The plan's
rule is a floor, not a preference: never a silently-empty completion.**

### D-1 — separate the three empty results

The minimum that makes anything else possible. `offset_polygon` and
`OffsetRingSet::offset` return a value that distinguishes:

- `Collapsed` — geometry legitimately ran out (the expected, common case);
- `RejectedInput { reason }` — the `< 3`-vertex guard, a non-finite
  coordinate, a ring that dedupes below a triangle;
- `LibraryFailure { assertion, location }` — the containment fired.

**Options for the shape.**

| Option | Signature | Cost |
|---|---|---|
| **A. Typed result** | `fn offset_polygon(&Polygon2, f64) -> OffsetOutcome` with `polys()` accessor | touches all ~20 call sites; every one must state what it does with a failure. That is the point, and it is also the whole cost. |
| **B. Side channel** | keep `Vec<Polygon2>`, add `fn offset_polygon_reported(&Polygon2, f64) -> (Vec<Polygon2>, Option<OffsetFailure>)`; the old name delegates | zero churn at the 15 geometry-only sites; the 5 that matter opt in. **Least disruptive; recommended starting point.** |
| **C. Thread-local / collector** | a findings sink the session drains after generation | invisible coupling, hard to test, and it is the field-by-field-copy debt pattern R7-H1 is removing elsewhere. **Not recommended.** |

### D-2 — surface a `LibraryFailure` where an operator can see it

The existing report-only channel is `ToolpathStats`
(`compute/config.rs`), which already carries twelve generation findings under
a stated `None` = *not measured* / `Some(0.0)` = *measured clean* contract and
is already read by `narrate_toolpath`, the diagnostics list and the MCP
per-toolpath summary. A thirteenth slot — `offset_library_failures: Option<usize>`
— costs five sites and no new plumbing.

**Explicitly NOT proposed:** turning a contained panic into a hard generation
error. That would convert today's silent under-cut into a refusal on inputs
that may still be producing usable geometry elsewhere in the same operation,
and it is a behaviour change with an operator-visible failure mode. It needs
its own ruling if wanted.

### D-3 — repair the boundary-clip escape (§5.3) and the skipped-offset sites (§5.4)

Independent of D-1/D-2, and the only items on this list with a plausible
**over-cut**. Neither requires the primitive to change.

**D-3a (§5.3, the top finding).** An empty `effective_boundary` result must
not silently mean "no containment". Options, in increasing strength:

| | Behaviour on an empty boundary | Cost |
|---|---|---|
| a | keep today's pass-through, but emit a typed finding naming the containment that was dropped | report-only; the operator can still ship an unclipped path |
| b | pass through only when the *cause* is a genuine collapse (which D-1 makes knowable); refuse on a library failure | needs D-1 |
| c | clip everything away — the conservative reading of "the boundary collapsed" | changes existing behaviour on the legitimate tool-larger-than-stock case, which currently produces a usable path |

**Not proposed:** (c) alone. The comment at `boundary.rs:79-83` describes a
real case where pass-through is right, and turning it into "emit nothing"
would break it. This is precisely why (b) needs the cause, and why D-1 is the
enabler rather than the deliverable.

**D-3b (§5.4).** Make `session/compute.rs:1727` and the two viz
`max_by(area)` sites distinguish "the offset produced nothing" from "the
offset produced something", instead of falling through to the un-offset
polygon.

**D-3c.** Decide whether the single-region boundary path should keep
`boundaries.first()` or `.flat_map` like the multi-region path at
`session/compute.rs:1881`. They currently disagree.

### D-4 — record the payload

One-line change at both chokepoints: `Err(payload)` instead of `Err(_payload)`
and reuse `candidate.rs`'s `panic_payload_message`, so the `warn!` names the
assertion. Worth doing whatever else is ruled; it costs nothing and it is the
difference between "cavalier panicked" and "cavalier tripped
`pline_view.rs:507`".

### D-5 — the release question

Decide explicitly whether the debug/release divergence in §3.1 is accepted.
Options: (a) accept and document — the release path produces a malformed slice
rather than a collapse, and no evidence exists either way; (b) upgrade or fork
`cavalier_contours` so the invariant is a hard `assert!`; (c) add a
pre-condition check on our side (`start_index <= end_index` cannot be checked
from outside, but the *input* class can — many-hole shapes at offsets that
merge holes) and refuse before the call. **(a) is the honest default for this
programme; (b) and (c) are new work.**

---

## 7. How to run the instrument

```bash
# gates (cheap, CI tier)
cargo test -p rs_cam_core --test cavalier_shape_failure_r2

# the census — installs a process-global panic hook per call
cargo test -p rs_cam_core --test cavalier_shape_failure_r2 -- \
    --ignored --nocapture --test-threads=1 census_direct_cavalier_calls
```

The census writes `target/adversarial_2d/cavalier_panic_census.md` (override
the directory with `R2_ARTIFACT_DIR`).

---

## 8. Honest limits of this document

- **Every result is a debug result.** §3.1's divergence means the debug census
  and the release behaviour are different questions, and only one of them has
  been asked. Recorded as `NOT EXERCISED`, per programme rule.
- **The panic sites in §3.2/§3.3 were read, not triggered.** They are a
  static census of `cavalier_contours` 0.7.0's source, so a site listed here
  may be unreachable from the two entry points we use.
- **No fix is proposed for cavalier itself.** R1 already ruled that the
  captured slice *"still asserts inside cavalier 0.7.0 (latest) even with clean
  input, so containment is the only fix for that class"*
  (`planning/TECH_DEBT_REVIEW_2026-06-10.md:29-31`). This document does not
  reopen that; it addresses what happens on our side of the containment.
- **`a_contained_panic_is_indistinguishable_from_a_collapse` passes today by
  describing the defect.** It is deliberately written to break when a typed
  channel lands, so the contract has to be re-stated on purpose rather than
  drifting.
