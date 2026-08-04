# Adversarial 2D campaign — findings

Date: 2026-08-04
Wave: W4 (lane C — 2D geometry), plan item **H3 / R2**
Revision measured: `0e7d38b` (branch `experiment/adaptive-spiral`), debug build
Status: **RESEARCH ONLY.** No production algorithm changed. Every disposition
is a proposal for **Checkpoint C**.

Companion documents: `ADVERSARIAL_2D_FIXTURE_SPEC.md` (what the fixtures are
and why), `OFFSET_CONSUMER_ROLLOUT.md` (the primitive's consumers),
`CAVALIER_SHAPE_FAILURE.md` (the library failure and its contract options).

---

## 1. Ranking

Severity × GUI reachability. "Reachability" means: can an operator, using the
shipped GUI, reach this without doing anything unusual?

| # | Finding | Severity | Reachability | Evidence | Disposition |
|---|---|---|---|---|---|
| **F-1** | An empty offset at the boundary layer removes the boundary clip **entirely** — a `ToolContainment::Inside` request whose offset comes back empty produces *no containment*, not a collapsed one | **HIGH — over-cut** | **HIGH** — `BoundaryConfig` is a GUI dial, on the live worker path | code, §3.1 | Checkpoint C **D-3a** |
| **F-2** | A library panic, a `< 3`-vertex guard and a genuine collapse are the same observable value; no channel can carry the difference, and no operator surface shows any of them | HIGH — diagnosability | HIGH — every 2D op | pinned test, §3.2 | Checkpoint C **D-1**, **D-2**, **D-4** |
| **F-3** | The primary captured panic class is a `debug_assert!` — the shipped containment is a **debug-only** net for it, and release behaviour is unvalidated | HIGH | HIGH — release is what ships | source read, §3.3 | Checkpoint C **D-5** |
| **F-10** | **Pocket's ring cascade has no ring cap and no divergence check — its only exit is collapse.** A contract-violating CW exterior makes every offset *grow*: it never collapses, never caps, and allocated **22.9 GB RSS without terminating** | HIGH — unbounded resource | **LOW** — both importers normalise winding; measured and asserted, §3.7 | measured + bounded probe, §3.7 | Checkpoint C |
| **F-11** | **A `NaN` vertex trips a `debug_assert!` in `static_aabb2d_index 2.0.0` — a *transitive* dependency R1's census never covered.** In release that check is skipped and the offset proceeds on a corrupt spatial index, which the library's own docs call "unexpected behavior" | HIGH | MEDIUM — nothing filters non-finite coordinates anywhere | measured, §3.8 | Checkpoint C **D-5** |
| **F-12** | **A perfectly VALID fixture reaches a third panic site** — `pline_seg.rs:33` *"v1 must not be on top of v2"*, on a zero-length arc segment. Also a `debug_assert!`: in release it divides by a zero chord length and returns a **NaN arc centre** instead of panicking. Contained → the pocket ring cascade ends early → an inlay female pocket silently leaves material | HIGH | **HIGH** — no contract violation is needed to reach it | measured, §3.9 | Checkpoint C **D-1**, **D-5** |
| **F-4** | **Rest and Drill ignore a cancel flag entirely** | MEDIUM | HIGH — both in the 2D menu | measured, §3.4 | Checkpoint C |
| **F-5** | Profile, Trace and Zigzag poll cancellation **only between Z levels** — on a single-level operation they are uncancellable in practice | MEDIUM | HIGH | code + measured, §3.4 | Checkpoint C |
| **F-6** | A boundary offset that *splits* keeps only `boundaries.first()` on the single-region path; the multi-region path keeps them all | LOW–MEDIUM — under-cut | MEDIUM | code, §3.5 | Checkpoint C **D-3c** |
| **F-7** | `adaptive3d/clearing.rs:1818` lifts every offset vertex onto a heightmap with no declared sampling density — structurally the defect `FlattenPolicy::with_max_segment` exists for | MEDIUM | MEDIUM — 3D lane | code, `OFFSET_CONSUMER_ROLLOUT.md` O-1 | hand off to W8 |
| **F-8** | Two sites silently skip a requested boundary offset and keep the un-offset polygon | LOW–MEDIUM | MEDIUM | code, §3.6 | Checkpoint C **D-3b** |
| **F-9** | Before this wave, **pocket was the only 2D family with any wall-clock or ring-count sentry**; the other eight had none | LOW (process) | — | census | closed by this wave |

### 1.1 What was actually run

| Instrument | Result |
|---|---|
| `adversarial_2d_fixtures_contain_their_mechanism` | **PASS** — 22 fixtures, 11 classes, all render, all prove their mechanism |
| `cancellable_2d_families_return_after_the_flag_is_set` | **PASS** — latencies in §3.4 |
| `cavalier_shape_failure_r2` (4 tests) | **PASS** — 4 passed, 0 failed, 1 ignored, 0.67 s |
| `every_2d_operation_survives_its_worst_fixtures` | **NOT COMPLETED** — see F-10; the run it was in reached 22.9 GB RSS |
| `adversarial_2d_full_campaign` | **PARTIAL — 33 of 198 cells**, §1.2. Stopped on F-12, then blocked by a full disk |
| `the_reflex_cross_generator_is_bit_identical_to_its_donor` | **NOT RUN** — written after the last successful build; `cargo check` and `cargo fmt --check` clean, focused test unverified |
| `uncancellable_2d_families_are_exactly_the_declared_two` | **NOT RUN** — same |
| `the_pocket_ring_cascade_is_bounded_only_by_collapse` | **NOT RUN** — same. Its *conclusion* is measured (F-10 was observed twice, at 22.9 GB and 6.6 GB); the bounded probe that replaces the observation with an assertion has not itself been executed |
| `cargo clippy --workspace --all-targets -- -D warnings` | **NOT RUN** — blocked by the full disk |

**Blocker, stated.** The machine's root filesystem reached **100% (889 GB of
935 GB, 72 MB free)** during this wave. Cargo cannot build. Three focused
tests and the clippy gate are therefore unverified, and are marked as such
rather than assumed. `cargo check` on both new test targets and
`cargo fmt --check` were clean at the last build that succeeded, which covers
compilation and formatting but not behaviour or lints.

### 1.2 The measured matrix — 33 of 198 cells

Wall clock, outcome, emitted topology and RSS growth per cell. Full file:
`artifacts/w4/campaign_matrix_partial.md`. Columns: op · fixture · wall ·
outcome · moves · cutting moves · cut mm · cut runs · RSS growth.

| op | fixture | wall | outcome | moves | cutting | cut mm | runs | RSS growth |
|---|---|---|---|---|---|---|---|---|
| pocket | reflex-cross | 0.005 s | ok | 269 | 236 | 2187.8 | 11 | 2368 kB |
| adaptive | reflex-cross | 0.087 s | ok | 368 | 359 | 1465.8 | 3 | 84 kB |
| profile | reflex-cross | 0.000 s | ok | 26 | 23 | 368.1 | 1 | 0 kB |
| trace | reflex-cross | 0.000 s | ok | 20 | 16 | 329.4 | 1 | 0 kB |
| zigzag | reflex-cross | 0.001 s | ok | 217 | 124 | 2805.0 | 31 | 64 kB |
| vcarve | reflex-cross | 0.487 s | ok | 39 316 | 39 217 | 3216.1 | 33 | 11 524 kB |
| inlay | reflex-cross | 0.610 s | ok | 53 824 | 53 415 | 9100.3 | 111 | 3896 kB |
| drill | reflex-cross | 0.000 s | ok | 6 | 2 | 0.0 | 2 | 0 kB |
| rest | reflex-cross | 0.007 s | ok | 413 | 236 | 2570.5 | 59 | 0 kB |
| pocket | comb-16 | 0.007 s | ok | 471 | 447 | 3623.0 | 8 | 0 kB |
| adaptive | comb-16 | 0.751 s | ok | 447 | 432 | 2721.6 | 5 | 0 kB |
| profile | comb-16 | 0.002 s | ok | 94 | 91 | 1838.9 | 1 | 0 kB |
| trace | comb-16 | 0.000 s | ok | 76 | 72 | 1849.4 | 1 | 0 kB |
| zigzag | comb-16 | 0.026 s | ok | 434 | 248 | 5373.2 | 62 | 0 kB |
| vcarve | comb-16 | 2.607 s | ok | 100 479 | 99 564 | 16 577.0 | 305 | 12 756 kB |
| inlay | comb-16 | 3.230 s | ok | 138 175 | 135 878 | 37 577.7 | 651 | 21 100 kB |
| drill | comb-16 | 0.000 s | ok | 6 | 2 | 0.0 | 2 | 0 kB |
| rest | comb-16 | 0.078 s | ok | 630 | 360 | 4292.8 | 90 | 0 kB |
| pocket | dendrite | 0.014 s | ok | 695 | 593 | 5197.6 | 34 | 0 kB |
| adaptive | dendrite | 1.761 s | ok | 556 | 541 | 3067.9 | 5 | 0 kB |
| profile | dendrite | 0.006 s | ok | 230 | 227 | 1537.0 | 1 | 0 kB |
| trace | dendrite | 0.002 s | ok | 252 | 248 | 1990.2 | 1 | 0 kB |
| zigzag | dendrite | 0.018 s | ok | 1099 | 628 | 9182.3 | 157 | 0 kB |
| vcarve | dendrite | 4.306 s | ok | 96 164 | 95 453 | 13 789.4 | 237 | 0 kB |
| inlay | dendrite | 4.826 s | ok | 136 489 | 134 564 | 32 810.6 | 541 | 1596 kB |
| drill | dendrite | 0.000 s | ok | 6 | 2 | 0.0 | 2 | 0 kB |
| rest | dendrite | 0.050 s | ok | 1281 | 732 | 8012.0 | 183 | 0 kB |
| pocket | rosette-24 | 0.079 s | ok | 1073 | 1010 | 4837.9 | 21 | 0 kB |
| adaptive | rosette-24 | 6.034 s | ok | 1025 | 1013 | 3166.7 | 4 | 0 kB |
| profile | rosette-24 | 0.031 s | ok | 175 | 172 | 827.5 | 1 | 0 kB |
| trace | rosette-24 | 0.004 s | ok | 297 | 293 | 886.7 | 1 | 0 kB |
| zigzag | rosette-24 | 0.100 s | ok | 721 | 412 | 7632.6 | 103 | 0 kB |
| vcarve | rosette-24 | 6.295 s | ok | 95 397 | 95 043 | 9240.6 | 118 | 0 kB |
| **inlay** | **rosette-24** | **> 129 s** | **CEILING EXCEEDED, run stopped** | — | — | — | — | — |

**What the collected rows say, and what they do not.**

- **No panic escaped, no silent empty, no ceiling breach in 33 of 34 cells.**
  Every cell returned `ok` with cutting moves, on four fixtures — the reflex
  cross, a 16-slot comb, a two-scale dendrite and a 24-lobe rosette. The M5
  arc cascade is holding: `pocket × reflex-cross`, the shape that once ran 13
  minutes, is **5 ms**.
- **RSS growth is a lower bound and reads 0 kB on most rows** — that is the
  `VmHWM` method (§4), not a claim of zero allocation. Only the rows that
  raised the process high-water mark show a figure, and the largest,
  `inlay × comb-16` at 21 MB, is the honest one.
- **Emitted size is wildly uneven across families.** On the same fixture,
  trace emits 76 moves and inlay 138 175 — a factor of 1800. `vcarve` and
  `inlay` dominate every column they appear in. Not a defect on this evidence,
  but it is where the runtime is, and it is what makes them the two families a
  hostile fixture can most easily push over a ceiling (F-12).
- **165 cells were never run.** They are `NOT RUN`, not passing.

---

## 2. Fixture inventory and non-vacuity

Twenty-two fixtures across eleven hostile classes. Every one is measured and
must prove it contains its mechanism before it is allowed to gate anything
(`adversarial_2d_fixtures_contain_their_mechanism`). The generated table lands
at `target/adversarial_2d/fixture_mechanisms.md`; the committed copy is §2.1.

Renders: one SVG per fixture (`fixture_<name>.svg`) and one per emitted
toolpath (`<op>_<fixture>.svg`), written to `target/adversarial_2d/` by
default and copied for the record to
`planning/review_2026-08-04/artifacts/w4/`.

| fixture | class | verts | rings | reflex | min seg (mm) | sub-eps segs | min gap (mm) | min curv r (mm) | comps | nest | min area (mm²) | self-int |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| reflex-cross | reflex | 12 | 1 | 4 | 2.00e1 | 0 | 2.00e1 | 14.142 | 1 | 1 | 4800.0000 | false |
| comb-16 | reflex | 68 | 1 | 32 | 5.76e0 | 0 | 5.76e0 | 20.207 | 1 | 1 | 12160.0000 | false |
| dendrite | dendrite | 244 | 1 | 120 | 2.80e0 | 0 | 0.00e0 | 2.778 | 1 | 1 | 11514.8800 | true |
| rosette-24 | reflex | 720 | 1 | 288 | 4.54e-1 | 0 | 4.54e-1 | 0.813 | 1 | 1 | 11407.9229 | false |
| holes-in-holes | holes-in-holes | 20 | 5 | 0 | 2.40e1 | 0 | 1.20e1 | 16.971 | 1 | 9 | 576.0000 | false |
| islands-4x4 | islands | 64 | 16 | 0 | 2.00e1 | 0 | 1.00e1 | 14.142 | 16 | 1 | 400.0000 | false |
| holed-9 | holes-in-holes | 580 | 10 | 0 | 1.47e0 | 0 | 1.47e0 | 15.000 | 1 | 2 | 705.7234 | false |
| walls-1e-2 | near-coincident | 8 | 1 | 2 | 1.00e-2 | 0 | 1.00e-2 | 30.000 | 1 | 1 | 9599.4000 | false |
| walls-1e-6 | near-coincident | 8 | 1 | 2 | 1.00e-6 | 1 | 1.00e-6 | 30.000 | 1 | 1 | 9599.9999 | false |
| short-edges | short-edges | 1600 | 1 | 796 | 4.47e-6 | 800 | 4.47e-6 | 0.280 | 1 | 1 | 10000.0000 | false |
| near-collinear-1nm | near-collinear | 400 | 1 | 196 | 1.00e0 | 0 | 1.00e0 | 0.707 | 1 | 1 | 10000.0000 | false |
| tiny-islands | tiny-islands | 32 | 8 | 0 | 4.50e-1 | 0 | 4.50e-1 | 0.318 | 1 | 3 | 0.2025 | false |
| slot-under | thin-slot | 12 | 1 | 4 | 1.85e1 | 0 | 3.00e0 | 17.623 | 1 | 1 | 3290.0000 | false |
| slot-exact | thin-slot | 12 | 1 | 4 | 1.70e1 | 0 | 6.00e0 | 17.241 | 1 | 1 | 3380.0000 | false |
| high-curvature | high-curvature | 360 | 1 | 204 | 1.25e-2 | 0 | 1.23e-3 | 0.790 | 1 | 1 | 5006.7394 | false |
| invalid-bowtie | invalid | 4 | 1 | 2 | 6.00e1 | 0 | 4.24e1 | 42.426 | 1 | 1 | 0.0000 | true |
| invalid-zero-area | invalid | 12 | 1 | 0 | 5.45e0 | 0 | 0.00e0 | inf | 1 | 1 | 0.0000 | true |
| invalid-two-vertex | invalid | 2 | 1 | 0 | inf | 0 | inf | inf | 0 | 0 | 0.0000 | false |
| invalid-nan | invalid | 4 | 1 | 1 | 6.00e1 | 0 | 6.00e1 | 42.426 | 1 | 1 | 0.0000 | false |
| invalid-cw | invalid | 4 | 1 | 0 | 6.00e1 | 0 | 6.00e1 | 42.426 | 1 | 1 | 3600.0000 | false |
| invalid-open | invalid | 3 | 1 | 0 | 6.00e1 | 0 | inf | 42.426 | 1 | 1 | 1800.0000 | false |
| invalid-far | invalid | 4 | 1 | 0 | 6.00e1 | 0 | 6.00e1 | 42.426 | 1 | 1 | 3584.0000 | false |

---

## 3. Findings in detail

### F-1 — an empty offset at the boundary layer removes the boundary clip entirely

**What.** `boundary::effective_boundary` turns a containment setting into
geometry with a single offset:

```rust
ToolContainment::Inside  => offset_polygon(boundary,  tool_radius),
ToolContainment::Outside => offset_polygon(boundary, -tool_radius),
```

Its empty result is consumed by `clip_annotated_to_boundary_set`, whose
documented contract (`boundary.rs:79-83`) is that an empty boundary slice *"is
not an error and not a clip: the toolpath passes through with an identity
mapping."* Both call sites implement exactly that:

- `crates/rs_cam_core/src/session/compute.rs:1793-1804` —
  `boundaries.first().map(std::slice::from_ref).unwrap_or(&[])`, with the
  behaviour spelled out in its own comment;
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:696-712` — the live GUI
  worker — an `if let Some(boundary) = boundaries.first()` whose implicit
  `else` is *do not clip*.

**So:** an operator sets `BoundaryContainment::Inside` — "keep the whole cutter
inside this boundary", a safety containment. `offset_polygon` is asked to
shrink the boundary by the tool radius. If cavalier panics on that boundary
polygon, the containment maps the panic to an empty `Vec`, the clip is
skipped, and the toolpath is emitted **with no boundary containment at all**.

**Why it is the top row.**

1. This is an **over-cut**, and it is the exact direction `polygon.rs:295-300`
   argues cannot happen: *"all under-cut directions, never a gouge."*
2. `BoundaryConfig` (`compute/config.rs:1249-1256`) is a GUI dial with both
   `containment` and `offset` fields, and both consumers are on shipped paths.
3. **The captured panic asset is boundary-shaped.**
   `test_data/cavalier_panic_polygon_r1.json` is a WANAKA Back Rough terrain
   slice: 86-vertex exterior, **13 holes**, offset inward 5.53 mm. That is the
   shape and the operation a boundary containment performs.

**Honest limit.** The pass-through is *correct* for the cause its comment
names — "the tool is larger than the stock", where nothing is machinable and
emitting the unclipped path is at least not a silent deletion. The defect is
that it cannot tell that cause from a library failure, because nothing
downstream can (F-2). That is why **D-1 is the enabler and D-3a is the fix**,
and why simply clipping everything away is *not* proposed: it would break the
legitimate case.

**Not reproduced end-to-end.** This wave did not drive a panicking boundary
polygon through `apply_boundary_clip` and observe an unclipped toolpath. It is
a code-path finding with all three links cited. Building that reproduction is
the first thing a Checkpoint-C-approved fix should do, red-first.

### F-2 — three different failures, one indistinguishable value

`offset_polygon` returns `Vec<Polygon2>`. It returns `Vec::new()` for:

1. a ring below the `< 3`-vertex guard (`polygon.rs:377`, `:386`, `:394`);
2. a genuine geometric collapse;
3. a contained `cavalier_contours` panic (`polygon.rs:293-303`), and the same
   again in the cascade (`polygon.rs:819-828`).

Pinned by `cavalier_shape_failure_r2::a_contained_panic_is_indistinguishable_from_a_collapse`,
which is written so it **breaks** when a typed channel lands — the contract
then has to be restated deliberately instead of drifting.

Three aggravating properties:

- the panic **payload is discarded** (`Err(_payload)` at both chokepoints),
  so even the `warn!` cannot name which assertion tripped, although a working
  extractor sits in the same crate at
  `tool_load/optimize/candidate.rs:342`;
- the `tracing::warn!` has **no consumer** — not a `Diagnostic`, not a
  `ToolpathStats` finding, not narration, not MCP. Programme rule 4: *a
  warning nobody sees is not a warning*;
- every 2D consumer maps empty onto a successful empty toolpath —
  `trace.rs:54-66`, `zigzag.rs:65-68`, `profile.rs:78-86`, `pocket.rs:90-94`,
  `boundary.rs:36-42`, `adaptive3d/clearing.rs:1724,1818,1954`.

Full table: `CAVALIER_SHAPE_FAILURE.md` §5.2.

### F-3 — the containment is debug-only for the class it was built for

The assertion the R1 asset trips is `pline_view.rs:507`, and it is a
**`debug_assert!`**. In release it is not checked; `from_slice_points`
proceeds with `start_index > end_index` and the `catch_unwind` never fires.
What release produces instead — a malformed slice stitched into a ring — is
unvalidated.

The sites that *do* panic in release are different ones:
`shape_algorithms/mod.rs:786` and `pline_offset.rs:1401`
(`unreachable!("loop_count exceeded max_loop_count while stitching slices
together")`), the hard `assert!`s at `pline_view.rs:316/374/438`, and the
`unwrap`/`expect`/raw-indexing sites listed in `CAVALIER_SHAPE_FAILURE.md`
§3.3.

This was known when R1 landed
(`planning/TECH_DEBT_REVIEW_2026-06-10.md:43-44`) and has not been decided
since. **This wave did not build in release** (programme rule 11), so release
behaviour is recorded as `NOT EXERCISED`, never as `PASS`.

### F-4 / F-5 — the cancellation asymmetry

Pinned by `uncancellable_2d_families_are_exactly_the_declared_two`, which
pre-sets the cancel flag and classifies every family by whether it comes back
with a cancellation error.

| Family | Honours the flag? | Granularity |
|---|---|---|
| pocket | yes | **per offset ring** (`pocket.rs:88`) + per Z level |
| adaptive | yes | per pass |
| inlay | yes | per ring (female pocket) / per scan line (female v-carve) |
| v-carve | yes | **per scan line** (`vcarve.rs:118`) |
| profile | yes | **per Z level only** — `profile_toolpath` has no cancel parameter at all |
| trace | yes | **per Z level only** — `trace_polygon_at_z` has no check |
| zigzag | yes | **per Z level only** — `zigzag_toolpath` has no check |
| **rest** | **NO** | `generate_rest` builds no `cancel_fn` and calls the non-cancellable `depth::toolpath_at_levels` (`execute.rs:709`) |
| **drill** | **NO** | `generate_drill` never touches `ctx.cancel` (`execute.rs:639`) |

`execute.rs:499-508` states the registry-wide version: 19 of 23 families
cancellable; Drill, AlignmentPinDrill, Rest and Chamfer are not. All four are
in `OperationType::ALL_2D`, i.e. all four are in the GUI's 2D menu.

**F-5's practical edge:** a per-Z-level check is not cancellation on a
single-level operation, and it is not cancellation *inside* a level however
many there are. The campaign measures post-flag latency rather than total
runtime for exactly this reason, and reports `NOT EXERCISED (finished first)`
rather than a pass when the generate beat the flag.

Making either group cancellable changes behaviour and needs Checkpoint C. The
existing sentry `execute.rs:4145`
(`cancellable_families_honour_a_preset_cancel_flag`) must be extended in the
same PR or its coverage claim goes stale.

### F-7bis note

### F-10 — pocket's cascade is bounded only by collapse

**Found the hard way.** The first CI-tier run of this campaign reached
**22.9 GB RSS and did not terminate.** There was nothing in the log naming the
cell, because the campaign printed its table only at the end — which is why
`announce()` now prints *before* each cell, and why the wall-clock ceiling
being a post-hoc check is called out in §4.

The cell was `pocket × holes-in-holes`. The mechanism is not the nesting.

`pocket_contours_with_cancel` (`pocket.rs:144-170`):

```rust
loop {
    check_cancel(cancel)?;
    rings = rings.offset(stepover);
    if rings.is_empty() { break; }
    …
}
```

**No ring cap. No divergence check. The only exit is collapse.** That is fine
while every offset shrinks — and whether it shrinks depends on the input's
*winding*, because cavalier's offset sign is relative to the polyline's own
direction. On a CW exterior a positive distance grows the ring, so the loop
never collapses and allocates until the machine gives up.

The first version of the `holes_in_holes` generator alternated ring winding,
which made the odd rings CW. That was my error, not a defect in the fixture
class: a CW exterior violates `Polygon2`'s stated contract
(`polygon.rs:9-12`), so the fixture was a *second* invalid case wearing a
valid label, and it hid the nesting case behind a crash. The generator now
emits every ring CCW; the winding case lives in `invalid-cw`, labelled
honestly.

**Measured, bounded** — `the_pocket_ring_cascade_is_bounded_only_by_collapse`
caps the iteration at 40 rings and measures the area trend, which is safer
than a timeout and a better instrument, because it shows the *direction*:

- CCW control (60 mm square, 2.4 mm step): area non-increasing, collapses
  well inside the cap;
- CW twin: never collapses in 40 rings, area grows by more than 4×.

**Reachability is LOW, and that is asserted rather than assumed.** Both
importers normalise: `svg_input.rs:161` and five sites in `dxf_input.rs` call
`Polygon2::ensure_winding`, and `detect_containment` normalises too
(`polygon.rs:1079`). The probe asserts `ensure_winding` still does its job, so
if that guard ever moves, this finding's severity changes with it.

**Why it is still ranked high.** The bound is missing from the *loop*, and the
winding guard is a property of two importers, not of the cascade. Any future
producer of polygons that does not normalise — a region extractor, a boolean
result, an MCP-supplied polygon, a new importer — re-arms it. `pocket.rs`'s
own comment already says the quiet part:

> Only the cancel hook made that a hang instead of a lock-up, **which is not
> the same thing as being bounded.**

M5 removed the vertex-growth mechanism. It did not add a bound. The same
cascade is reachable through `inlay`'s female pocket (`inlay.rs:108`).

**Proposed for Checkpoint C:** a ring cap plus a monotonic-shrink assertion in
`OffsetRingSet`-driven cascades, and/or a winding precondition at the
`offset_polygon` boundary. Both are behaviour changes; neither is made here.

### F-11 — a `NaN` vertex reaches a spatial index whose validity check is debug-only

`no_hostile_input_escapes_the_offset_chokepoint_as_a_panic` tripped, four
times (the `invalid-nan` fixture at its four distances):

```text
.../static_aabb2d_index-2.0.0/src/static_aabb2d_index.rs:266:9:
assertion failed: min_x <= max_x
```

`offset_polygon` contained all four and returned empty, so the gate passed.
But the class is new: it is in a **transitive** dependency
(`cavalier_contours` → `static_aabb2d_index 2.0.0`) that R1's 2026-06-10
census never mentioned, and it is reached from the bounding-box build inside
the offset, not from the stitching code R1 studied.

It has the same debug/release split as F-3, and this time the library
documents it (`static_aabb2d_index.rs:255-258`):

> For performance reasons the sanity checks of `min_x <= max_x` and
> `min_y <= max_y` are **only debug asserted**. If an invalid box is added it
> may lead to a panic **or unexpected behavior** from the constructed
> `StaticAABB2DIndex`.

So in release a `NaN` vertex does not panic. It builds a corrupt spatial index
and the offset proceeds on it. Nothing upstream filters non-finite
coordinates: `Polygon2` has no validating constructor and no `Result`
anywhere on the type.

### F-12 — a valid fixture reaches a third panic site, and release turns it into NaN

The full campaign tripped, four times on `inlay × rosette-24`:

```text
.../cavalier_contours-0.7.0/src/polyline/pline_seg.rs:33:5:
v1 must not be on top of v2
```

**`rosette-24` is a `Validity::Valid` fixture** — a 24-lobe polar rosette,
CCW, 720 finite vertices, no self-intersection, no degenerate ring. Nothing
about it violates any documented contract. It is simply concave enough that
some offset produces a zero-length arc segment.

The site (`pline_seg.rs:28-40`):

```rust
pub fn seg_arc_radius_and_center<T>(v1: PlineVertex<T>, v2: PlineVertex<T>) -> (T, Vector2<T>) {
    debug_assert!(!v1.bulge_is_zero(), "v1 to v2 must be an arc");
    debug_assert!(!v1.pos().fuzzy_eq(v2.pos()), "v1 must not be on top of v2");
    let chord_v = v2.pos() - v1.pos();
    let chord_len = chord_v.length();
    let radius = chord_len * (abs_bulge * abs_bulge + T::one()) / (T::four() * abs_bulge);
    …
```

`debug_assert!` again. **In release the guard is gone, `chord_len` is zero, and
the function divides by it** — returning a NaN/infinite radius and a NaN arc
centre, which then propagate into the offset geometry. That is the third
instance of the same pattern (F-3, F-11, F-12), and this one needs no
malformed input at all.

**The machining consequence in debug** is not a crash — the containment
catches it. Inlay's female half calls `pocket_toolpath_with_cancel`
(`inlay.rs:108`), whose cascade treats a contained panic as a collapsed ring
and **breaks the loop**. The pocket therefore ends early and the female pocket
silently leaves material, reported as a successful generate. That is F-2's
"indistinguishable" property with a concrete cost attached.

**This is the strongest single argument for D-1.** R1's containment was
justified on the grounds that every empty-result branch is an under-cut and
never a gouge. Under-cut it is — and the operator is never told that a pocket
they asked for stopped early because a dependency's debug assertion fired.

### F-6 — `boundaries.first()` versus `.flat_map`

`session/compute.rs:1800` and `viz .../execute/mod.rs:701` take only the first
polygon of the effective boundary. A boundary that splits into several
polygons under the containment offset keeps piece 1 and clips everything
outside it away. `session/compute.rs:1881` — the multi-region variant —
`.flat_map(…)`s and keeps them all. Two paths, same question, different
answers. Under-cut direction, so lower severity than F-1, but silent.

### F-7 — an inherited sampling density in adaptive3d

`adaptive3d/clearing.rs:1818` offsets a region boundary once, then lifts
**every returned vertex** onto the surface heightmap (`:1675`) and emits the
lifted chain as a cutting segment. The XY spacing of those probes is whatever
cavalier's arc-join debris left behind. Structurally this is the defect
`FlattenPolicy::with_max_segment` was created for; scallop's version of it
emitted *nothing at all* on a four-island mesh
(`capability_link_moves_safety`, `polygon.rs:713-720`).

Not reproduced — no disjoint-island adaptive3d fixture was built. Hand off to
the adaptive3d/W8 lane. Full write-up: `OFFSET_CONSUMER_ROLLOUT.md` O-1.

### F-8 — a silently skipped boundary offset

`session/compute.rs:1727-1732` and the two viz sites share:

```rust
let offset_polys = offset_polygon(p, -boundary_config.offset);
if let Some(largest) = offset_polys.into_iter().max_by(area) { *p = largest; }
```

On empty, `p` keeps its **un-offset** value: the requested offset silently
does not happen. For a *negative* `BoundaryConfig::offset` — the operator
shrinking the machining boundary, the protective direction — that leaves the
toolpath clipped to a larger region than asked for.

### F-9 — the coverage gap this wave closes

Before this wave the entire 2D stack had exactly one termination sentry:
`pocket::tests::pocket_cascade_terminates_on_the_reflex_cross`
(`crates/rs_cam_core/src/pocket.rs:248`), added with the arc cascade on
2026-08-03. It is the right shape — wall clock **and** ring count, because
*"a cascade that collapses early passes a timing check for the wrong reason"*
— and it was the only one. Adaptive, profile, trace, zigzag, inlay, v-carve,
rest and drill had no wall-clock, ring-count, memory or cancellation coverage
at all, and no hostile geometry anywhere in the suite.

---

## 4. Method

**Driver.** Every cell goes through `ProjectSession::generate_toolpath` — the
production funnel the GUI worker, the CLI and MCP all share — with fixtures
injected in memory via `common::session::polygon_model`. Sessions are built
with the real entry points (`add_tool`/`add_model`/`add_toolpath`), never by
poking fields.

**Wall clock.** 60 s ceiling per generate in debug (30 s for the CI tier),
pre-registered before the run. Sized from the defect (13 minutes), not from a
performance target.

**The ceiling is a post-hoc check, not a preemptive watchdog — stated,
because it matters.** `run_op` measures the generate and *then* compares
against the ceiling, so a generate that genuinely never returns hangs the test
rather than failing it. There is no way to make it preemptive that works for
all nine families: the only preemption mechanism is the cancel flag, and
**Rest and Drill ignore it** (F-4). The outer bound is therefore the shell's
`timeout`, and a hang shows up as a killed run rather than a named assertion.
A campaign that hangs *is a finding* and is recorded as one.

**Memory — method stated.** `/proc/self/status` `VmHWM`, read before and
after each call. That is a **process-wide monotonic high-water mark**, so the
campaign runs one operation per measurement, sequentially, single-threaded, in
one process, and reports the *increase*. The increase is a **lower bound** on
the call's own peak: exact when the call is the largest allocator so far, and
zero when an earlier call already peaked higher — reported as `≤ prior peak`,
never as "used no memory". Non-Linux returns `unmeasured`.

**Cancellation.** The flag is set from a timer thread after 150 ms and the
record reports latency **from that moment**, not total runtime.

**Failure conditions.** (1) any panic; (2) `Ok` with zero cutting moves on an
`Expectation::MustCut` fixture; (3) exceeding the ceiling. A typed `Err` is
**not** a failure — on adversarial input it is the best available outcome,
because it is the only one an operator can see.

**One exemption, stated.** `rest` is exempt from (2). Its job is to cut what a
*larger* previous tool could not reach, so on a fixture whose features all
exceed that previous tool there is legitimately nothing to do, and `rest.rs:70`
returns empty by contract when `tool_radius >= prev_tool_radius`. Its empties
are recorded in the matrix instead of failing the gate.

---

## 5. Honest limits

- **Debug only.** No release build (programme rule 11). F-3 makes that a real
  gap rather than a formality.
- **F-1, F-6, F-7 and F-8 are code-path findings, not reproductions.** Every
  link is cited; none was driven end-to-end to observe the bad output. Any
  Checkpoint-C-approved fix should start by building that reproduction,
  red-first.
- **No `param_sweep` integration and no property/fuzz generator.** See
  `ADVERSARIAL_2D_FIXTURE_SPEC.md` §7.
- **Fixtures are injected in memory, not through the SVG/DXF importers.** The
  importer path (and `detect_containment`'s role in it) is untested here.
- **The `MustCut` expectation is a claim about geometry, not about every
  operation's obligation.** It says the fixture contains material a tool of
  the stated diameter can remove. Where an operation legitimately declines
  (rest, above), that is stated as an exemption rather than silently absorbed.
