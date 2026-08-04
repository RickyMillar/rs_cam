# Adversarial 2D fixture specification — R2 / H3

Date: 2026-08-04
Wave: W4 (lane C — 2D geometry)
Implementation: `crates/rs_cam_core/tests/common/adversarial2d.rs`
Campaign: `crates/rs_cam_core/tests/adversarial_2d_campaign_r2.rs`
Status: **RESEARCH ONLY.** Test-only code. No production algorithm changed.

---

## 0. Why this exists, in one fixture

A **twelve-vertex reflex cross** — four inner corners, no holes, no curves —
put `polygon::pocket_offsets` at **13 minutes and 386 MB**, and put
`pocket_contours_with_cancel` past **20 seconds with no result**, until the
arc cascade landed on 2026-08-03 (`e3427f8`,
`planning/review_2026-07-29/CHECKPOINT_D_EVIDENCE.md` §8). Nothing in the
suite would have found it, because until that commit every 2D fixture in the
tree was a square, a circle, a star, or a captured real part. Convex, smooth,
or real — never hostile.

The 3D stack got its hostile fixtures in M4/M5. This is the 2D stack's.

---

## 1. Design rules

### 1.1 A fixture must prove it contains its mechanism

Programme rule 15 and plan §H3 acceptance gate:

> Concave/reflex fixtures prove they contain the mechanism before they gate
> offset behaviour.

Every fixture carries a `Class`, and `Fixture::assert_contains_mechanism`
measures the geometry and fails — naming the measured value — if the class's
condition is not met. `adversarial_2d_fixtures_contain_their_mechanism` runs
that gate on all of them before any operation is invoked. **The fixtures are
tested before the operations are.**

`assert_contains_mechanism` has one arm per `Class` and no catch-all, so a
new class cannot be added without also stating how to prove it is present.

### 1.2 A hostile feature is hostile *relative to a tool*

A "thin slot" wider than the cutter is a pocket. Every fixture declares
`tool_d`, the diameter it is authored against, and the campaign builds the
tool from it. Non-vacuity conditions are expressed against that diameter
(`min_area_mm2 < π·r²`, `min_curvature_radius < r`, `min_gap < d`), not
against absolute numbers.

### 1.3 Existing fixtures are borrowed, not re-authored

`comb`, `dendrite`, `rosette`, `holed` and `short_edges` were authored for M5
and their measurements are quoted in `CHECKPOINT_D_EVIDENCE.md`. They are
re-exported from `common::offset_lab`, not copied. A second copy of a fixture
is a second thing to keep in sync.

The one generator lifted from shipped code — `reflex_cross`, from
`pocket::tests::pocket_cascade_terminates_on_the_reflex_cross` — carries a
**C6 bit-identity donor proof**
(`the_reflex_cross_generator_is_bit_identical_to_its_donor`), comparing
`f64::to_bits` per coordinate against the donor's literal. The donor sentry
keeps spelling its own vertices out, per `common/mod.rs`'s migration policy:
*a sentry's value is that it has not changed.*

### 1.4 Render before verdict

Programme rule 4. `write_fixture_svg` renders every fixture (exterior white,
holes cyan, **vertices as dots** so sub-epsilon clusters read as thickening
rather than as nothing); `write_toolpath_svg` renders every emitted toolpath
over its fixture (cutting amber, rapids dashed red — because on these shapes
the interesting failure is often *where the tool travelled*). The
non-vacuity test asserts that every VALID fixture rendered.

Renders go to `target/adversarial_2d/` by default, overridable with
`R2_ARTIFACT_DIR`. The default is under `target/` on purpose: a test that
writes into a tracked directory on every run makes `git status` lie about
what a wave changed. The committed gallery is copied to
`planning/review_2026-08-04/artifacts/w4/` once, deliberately, and its paths
are recorded in `ADVERSARIAL_2D_FINDINGS.md`.

---

## 2. The classes and their generators

| Class | Generator | Signature | Non-vacuity condition |
|---|---|---|---|
| `Reflex` | `reflex_cross` | `(a, b) -> Polygon2` | `reflex_corners ≥ 4` |
| `Reflex` | `comb` (M5) | `(w, h, teeth, slot_w, slot_d)` | `reflex_corners ≥ 4` |
| `Reflex` | `rosette` (M5) | `(base, amp, lobes, samples)` | `reflex_corners ≥ 4` |
| `Dendrite` | `dendrite` (M5) | `(w, h, teeth, slot_w, slot_d)` | `reflex_corners ≥ 16` |
| `HolesInHoles` | `holes_in_holes` | `(outer, gap, levels) -> Vec<Polygon2>` | `nesting_depth ≥ 3` |
| `HolesInHoles` | `holed` (M5) | `(size, per_side, radius, verts)` | `nesting_depth ≥ 3` |
| `DisconnectedIslands` | `disconnected_islands` | `(n, size, gap) -> Vec<Polygon2>` | `components ≥ 3` |
| `NearCoincidentWalls` | `near_coincident_walls` | `(w, h, gap, depth)` | `min_nonadjacent_gap < tool_d` |
| `ShortEdges` | `short_edges` (M5) | `(size, per_edge)` | `segments_below_pos_eps ≥ 100` |
| `NearCollinear` | `near_collinear` | `(size, per_edge, dev)` | `near_collinear_1um ≥ 20` |
| `TinyIslands` | `tiny_islands` | `(outer, n, island) -> Vec<Polygon2>` | `min_area < π·r²` |
| `ThinSlot` | `thin_slot` | `(pad, width, bridge)` | `min_gap < tool_d` |
| `HighCurvature` | `high_curvature` | `(base, lobes, lobe_r, per_lobe)` | `min_curvature_radius < r` |
| `InvalidContour` | 7 generators, §2.2 | — | `Validity::Invalid` declared |

### 2.1 Valid-geometry generators, in detail

**`reflex_cross(a, b)`** — the 13-minute shape. A plus sign whose four inner
corners are reflex and whose arms are `a` wide. Erosion at stepover `s`
should admit ≈ `a / (2s)` rings before the arms close; the shipped pocket
sentry uses `(20, 60)` at `s = 0.5` and asserts `≥ 15` contours precisely so
that *a cascade which collapses early cannot pass a timing check for the
wrong reason*.

**`holes_in_holes(outer, gap, levels)`** — `levels` nested square rings, each
`gap` mm inside the last, returned as a **flat `Vec<Polygon2>`**. That is the
form an SVG/DXF import hands over; `polygon::detect_containment` is what turns
it into nested holes, and whether it does so correctly is part of what the
fixture tests.

Parameter range: `levels ∈ [3, 9]`, `gap ≥ 2·tool_radius` for the nesting to
be machinable at all. Shipped: `(120.0, 12.0, 5)`.

> **Correction, recorded because it is instructive.** The first version
> alternated ring winding, on the theory that a consumer trusting winding
> rather than containment should get an honest answer. It got one:
> `pocket × holes-in-holes` allocated past **22.9 GB without terminating**.
> But the mechanism was the winding alone, not the nesting
> (`ADVERSARIAL_2D_FINDINGS.md` F-10) — and a CW exterior violates
> `Polygon2`'s contract, so alternating winding made this a *second* invalid
> fixture wearing a `Valid` label, and hid the nesting case behind a crash.
> **A fixture that contains two mechanisms can only report the first one it
> hits.** The winding case now lives in `reversed_winding` / `invalid-cw`,
> where it is labelled honestly.

**`disconnected_islands(n, size, gap)`** — `n × n` disjoint squares. Every
operation must produce `n²` independent regions or explain which it dropped.
This is the class that made scallop emit **nothing at all** when a "lossless"
cleanup left four corners all sitting off the part
(`capability_link_moves_safety`, `polygon.rs:713-720`). Shipped: `(4, 20, 10)`
= 16 islands.

**`near_coincident_walls(w, h, gap, depth)`** — a block with a full-depth slit
of width `gap`, i.e. two walls that are close together and **not adjacent in
the ring**. Drive it at values that straddle cavalier's `pos_equal_eps`
(1e-5): shipped at `1e-2` (real geometry, three orders under the tool) and
`1e-6` (below the epsilon that decides whether two vertices are the same
point at all). Range: `gap ∈ [1e-8, tool_d]`.

**`near_collinear(size, per_edge, dev)`** — vertices pushed `dev` off a
straight chord, alternating side so the run stays a run rather than becoming a
shallow bow an offset could legitimately follow. At `dev = 1e-9` these
vertices are geometrically nothing and every lossless cleanup removes them;
they are still the only thing telling a *sampling* consumer where to probe.
**This is the fixture that makes the `remove_redundant` ruling in
`polygon.rs:322-356` measurable rather than argued.** Range:
`dev ∈ [1e-12, 1e-3]`; shipped `1e-6` × 100 per edge.

**`tiny_islands(outer, n, island)`** — islands with less area than the tool's
own footprint. Shipped: 7 islands of 0.45 mm side = 0.2025 mm², against a
6 mm tool's 28.27 mm² footprint — two orders down.

**`thin_slot(pad, width, bridge)`** — a dumbbell: two pads joined by a slot.
Three cases, and it is the *set* that separates "refuses the impossible" from
"collapses on the marginal":

| Case | `width` vs `tool_d` | What it should do |
|---|---|---|
| under | `0.5 ×` | bridge unmachinable; the two pads are independent regions |
| exact | `1.0 ×` | a single full-width plunge cut with zero offset room |
| over | `1.05 ×` | a normal, if narrow, pocket |

Shipped: `slot-under` (3 mm / 6 mm tool) and `slot-exact` (6.000 mm / 6 mm).

**`high_curvature(base, lobes, lobe_r, per_lobe)`** — a scalloped disc whose
lobes sweep just past a half circle so consecutive lobes meet in a **cusp**
rather than a smooth blend. With `lobe_r < tool_radius`, every lobe root is a
corner the tool physically cannot enter — the case where an offset either
arc-joins correctly or invents geometry. Shipped: `(30, 18, 1.5, 12)` against
a 3 mm tool.

### 2.2 Invalid-contour generators

The `Polygon2` contract (`polygon.rs:9-22`): exterior CCW with positive area,
holes CW, ≥ 3 vertices, no duplicated closing vertex, `closed` defaulting
true. Nothing enforces it — there is no constructor that validates and no
`Result` anywhere on the type.

| Generator | Violation | Why it is interesting |
|---|---|---|
| `bowtie(size)` | self-intersecting exterior | `Polygon2::repaired` (R1.5) is supposed to split it into two triangles before cavalier sees it. If it does not, the class most likely to hit the panic path reaches the panic path. |
| `zero_area_collinear(size, n)` | ring encloses nothing | a well-formed vertex list with zero area; passes every `len() >= 3` guard in the codebase |
| `two_vertex(size)` | fewer than 3 vertices | exercises the `< 3` guard, **whose empty `Vec` is indistinguishable from a contained panic** — see `CAVALIER_SHAPE_FAILURE.md` §5.2 |
| `non_finite(size)` | one `NaN` vertex | nothing upstream filters non-finite coordinates, and every `f64` comparison against `NaN` is false, so every bbox/min/max derived from it is silently wrong rather than rejected |
| `reversed_winding(size)` | CW exterior | the offset sign convention inverts: `distance > 0` grows instead of shrinking |
| `open_contour(size)` | `closed: false` in a closed-region slot | `exterior_to_pline` hardcodes `true` (`polygon.rs:115`), so the flag is silently dropped |
| `far_from_origin(size, 1e9)` | coordinates outside any machine envelope | at 1e9 mm the `f64` spacing is ~1e-7 mm, so a 1e-5 mm epsilon is only two orders above the representable resolution of a difference |

**Bar for an invalid fixture: never a panic, and never a silent success.** Not
"produces a good toolpath". They are declared `Expectation::MayBeEmpty`.

### 2.3 `skip_ops` — the one legitimate reason not to run a cell

`Fixture::skip_ops` names `(operation, reason)` pairs the campaign must not
run. **It exists for exactly one situation: the cell provably does not
terminate.**

A non-terminating cell cannot be *tested* by a campaign whose wall-clock
ceiling is checked after the call returns. It hangs the suite and takes the
machine's memory with it — 22.9 GB on the first R2 run, with no assertion and
nothing in the log naming the cell. The divergence is proved instead by a
**bounded probe** that caps the iteration count and measures the trend, which
is both safe and a better instrument: it reports the direction, not merely the
absence of an answer.

One fixture uses it today: `invalid-cw` skips `pocket` and `inlay`, with
`the_pocket_ring_cascade_is_bounded_only_by_collapse` carrying the proof
(`ADVERSARIAL_2D_FINDINGS.md` F-10).

It must never be used to skip a cell that is merely slow, or one that fails.

---

## 3. Measures

`Mechanism` (`adversarial2d.rs`) is both the non-vacuity oracle and the source
of the spec's parameter tables, so the numbers in this document and the
numbers in the gate cannot drift.

| Field | Definition |
|---|---|
| `reflex_corners` | corners where the boundary turns *into* the material. Orientation-aware: the sign test is taken against the ring's own signed area, so a hole's concavity is not miscounted as convexity |
| `min_segment_mm` / `segments_below_pos_eps` | edge lengths, and how many are under `PLINE_POS_EQUAL_EPS = 1e-5` |
| `near_collinear_1um` | vertices whose perpendicular distance from the chord through their neighbours is under 1 µm |
| `min_nonadjacent_gap_mm` | the narrowest passage: minimum distance between two edges that are not neighbours in the same ring. O(edges²); the largest fixture is ~1600 edges |
| `min_curvature_radius_mm` | minimum circumradius over consecutive triples. `INFINITY` for collinear triples, so a `min` ignores straight runs |
| `components` / `nesting_depth` | top-level polygons, and levels of alternating material (1 = flat set, 2 = polygon with a hole, 3 = island inside a hole) |
| `min_area_mm2` | smallest absolute ring area, exterior or hole |
| `self_intersecting`, `non_finite_vertices`, `open_paths` | contract violations, counted |

All measures are taken on the fixture **as authored**, before tool-radius
compensation: that describes what the operation is handed, which is the input
whose handling is under test.

The measured table for every shipped fixture is written to
`target/adversarial_2d/fixture_mechanisms.md` on every run of the non-vacuity
test, and reproduced in `ADVERSARIAL_2D_FINDINGS.md` §2.

---

## 4. Operation applicability and oracles

All nine 2D families are GUI-reachable
(`toolpath_panel.rs:643` iterates `OperationType::ALL_2D`) and MCP-reachable
(`rs_cam_mcp/src/server.rs:598`). Five are **not** job-file reachable from the
CLI — `job.rs:454` accepts only `pocket`, `profile`, `adaptive`, `rest` plus
the two 3D ops — but all nine are reachable through `rs_cam_cli run`.

| Op | Consumes | Oracle for this campaign | Notes |
|---|---|---|---|
| pocket | `Vec<Polygon2>` | ring count + analytic erosion distance (`offset_lab::erosion_error`); containment; bounded termination | `pattern: Zigzag` silently redirects to `zigzag::zigzag_toolpath` (`execute.rs:1047`), losing per-ring cancellation |
| adaptive | `Vec<Polygon2>` | non-empty; containment; bounded termination | repeated offsets, never compounding (`adaptive/path.rs:1229-1234`) |
| profile | `Vec<Polygon2>` | offset distance = tool radius; topology preservation (one loop per input loop) | no `_with_cancel` variant at all; cancels per Z level only |
| trace | `Vec<Polygon2>` | path follows the source curve within tolerance; no gouge | cancels per Z level only |
| zigzag | `Vec<Polygon2>` | coverage of the inset region; containment | cancels per Z level only |
| inlay | `Vec<Polygon2>` + **V-bit** | female/male pair both non-empty; male fits female | `half_angle` comes from the TOOL, never the config |
| v-carve | `Vec<Polygon2>` + **V-bit** | depth-from-width along the medial axis | same |
| rest | `Vec<Polygon2>` + **two tools** | region left by the larger tool is covered | needs `prev_tool_id`; returns empty by contract when `tool_radius ≥ prev_tool_radius` (`rest.rs:70`) |
| drill | polygon **centroids** | target count == input polygon count | `execute.rs:618-629` takes the arithmetic mean of each exterior; one polygon = one hole. `selected_holes: Some(vec![])` is an error, `None` is "all centroids" |

Universal oracles applied to every cell by the campaign:

1. **no panic** — any input, valid or not;
2. **no silent empty success** — `Ok` with zero cutting moves on an
   `Expectation::MustCut` fixture;
3. **bounded wall clock** — 60 s per generate in debug (30 s for the CI tier).

A typed `Err` is not a failure. On adversarial input it is the best available
outcome, because it is the only one an operator can see.

---

## 5. Watchdog, memory and cancellation

**Wall clock.** `CEILING = 60 s` per generate, debug. Sized from the defect,
not from a performance target: the reflex cross ran 13 minutes. Sixty seconds
is two orders below that and two orders above every healthy generate measured
here, so it separates "slow debug build" from "does not terminate" without
becoming a flaky perf pin.

**Memory — method stated, because the number is meaningless without it.**
`peak_rss_kb()` reads `VmHWM` from `/proc/self/status`. That is a **process-wide
monotonic high-water mark**, not a per-call figure. The campaign therefore runs
one operation per measurement, sequentially, in a single process, and reports
the *increase* in the high-water mark across the call. That increase is a
**lower bound** on the call's own peak: exact when the call is the largest
allocator so far, and zero when an earlier call already peaked higher. A zero
is reported as `≤ prior peak`, never as "used no memory". `current_rss_kb()`
(`VmRSS`) is available to show memory coming back, which a high-water mark
cannot. Non-Linux returns `None` and the campaign records `unmeasured`.

**Cancellation.** `run_op_with_cancel` sets a shared `AtomicBool` after
`CANCEL_AFTER = 150 ms` and reports the latency **from that moment**, not the
total. A generator that only polls its flag between Z levels can pass a
wall-clock ceiling and still be uncancellable inside one level, so the
post-flag latency is the figure that matters. When the generator finished
before the flag was ever set, the record says `NOT EXERCISED (finished first)`
— never a pass.

The cancellation facts this campaign inherits, from `execute.rs:499-508` and a
per-adapter read:

| Family | Cancellable? | Granularity |
|---|---|---|
| pocket | yes | **per offset ring** (`pocket.rs:88`) + per Z level |
| adaptive | yes | per pass |
| inlay | yes | per ring (female pocket) / per scan line (female v-carve) |
| v-carve | yes | **per scan line** (`vcarve.rs:118`) |
| profile | yes | **per Z level only** |
| trace | yes | **per Z level only** |
| zigzag | yes | **per Z level only** |
| **rest** | **NO** | `generate_rest` builds no `cancel_fn` and uses the non-cancellable `depth::toolpath_at_levels` (`execute.rs:709`) |
| **drill** | **NO** | `generate_drill` never touches `ctx.cancel` (`execute.rs:639`) |

Pinned by `uncancellable_2d_families_are_exactly_the_declared_two` so the
findings document cannot go stale silently. Making the two cancellable is a
behaviour change and needs Checkpoint C.

---

## 6. Running it

```bash
# non-vacuity + donor proof + CI acceptance gate + cancellation (CI tier)
cargo test -p rs_cam_core --test adversarial_2d_campaign_r2

# the full 9 × 22 matrix, the findings table and the SVG gallery
cargo test -p rs_cam_core --test adversarial_2d_campaign_r2 -- \
    --ignored --nocapture adversarial_2d_full_campaign

# write the gallery straight into the evidence folder
R2_ARTIFACT_DIR=planning/review_2026-08-04/artifacts/w4 \
  cargo test -p rs_cam_core --test adversarial_2d_campaign_r2 -- --ignored --nocapture
```

---

## 7. Deliberate gaps

- **No property/fuzz generator.** The plan's §4.2 mentions "property/fuzz
  targets" for H3. `tests/property_tests.rs` and
  `tests/generator_extremes_fuzz_r1.rs` already exist and cover the
  parameter-space corners; this library covers the *geometry* space, which is
  the axis they do not. A randomised geometry fuzzer is proposed as follow-on
  work, not delivered.
- **No `param_sweep` family integration.** Sweeps drive the generators
  directly with the shipped fixtures; wiring hostile geometry into them is a
  separate decision about what the sweep fingerprints mean.
- **No DXF/SVG round-trip.** Fixtures are injected in memory via
  `common::session::polygon_model`, which is exact. Routing them through the
  importers would also test `detect_containment`'s import path, and would lose
  the sub-epsilon vertices to text formatting. Both are worth having; only the
  exact path is delivered.
- **Nothing here is a 3D fixture.** `adaptive3d` shares `offset_polygon` and
  has its own exposure (`OFFSET_CONSUMER_ROLLOUT.md` finding O-1); it is out
  of this lane's scope.
