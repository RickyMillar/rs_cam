# Antipatterns and tech debt observed during the 2026-07-29/30 programme

Cross-cutting observations from fifteen agent reports plus orchestration.
These are NOT scheduled programme items (M3/M4/M5/A-M6/A-M7/A-M10/H4/L1 are
tracked elsewhere) — this is the "noticed along the way" ledger, grouped by
pattern. Ordered roughly by leverage.

## Disposition, closed by L1 (wave 16, 2026-08-04)

L1 absorbs this document: every theme now carries a verdict, so nothing here
is left in the ambiguous state where "logged" reads as "handled". Details are
in each section; this is the index.

| # | Theme | Disposition |
|---|---|---|
| P1 | No transform provenance contract | **FIXED — wave 2 (C1)**, `77267ce`: `*_with_provenance` is the compile-enforced convention, `SemanticLinkCarrier` retired, per-dressup items bind true ranges. Residue: the viz worker still only builds a semantic recorder under `debug_options.enabled`, so those remap paths stay untested in the default configuration. |
| P2 | Silent-sentinel `0.0` (X-19 class) | **FIXED — wave 3 (C2)**, `ce426d6` + `96bc300`: `GridZ` newtype, private grid storage, 17-row consumer audit, 4 sentinels converted / 2 documented+tested / 8 ruled out. `steep_shallow`'s `min_z()` use was VERDICT: not defective — the bbox floor is intended for the waterline wall ladder. |
| P3 | Per-surface reimplementation | **STRUCTURALLY CLOSED — wave 8 (C3)**, `c44a2be`: the CLI diagnostic exhaustively destructures the core struct, so divergence is a compile error. Two sub-bullets closed in place below. **But the GUI worker's hand-copied findings path is NOT closed** (ledger B7): wave 15 needed two more copy lines, wave 16 a fifth. Still open, and still the highest-frequency silent-drop shape in the repo. |
| P4 | Stringly-typed vocabularies | **FIXED — wave 8 (C4)**, `ad445f3` / `b514046` / `263ba5d`. |
| P5 | `ComputeMessage` at its size ceiling | **FIXED — wave 1 (C5)**, `88ea12f`: `ComputeMessage::Toolpath` is boxed, so the per-field tax is gone. Wave 16 added two `ToolpathStats` fields and paid nothing. |
| P6 | No shared test-fixture library | **FIXED — wave 5 (C6)**, `c8e40f7` / `1cd6eee`: `tests/common/` with a bit-identity smoke harness pinning the shared generators to their donors. Wave 16's two new sentries were both written against it. |
| P7 | Ignored mega-harness rot | **STILL OPEN.** No wave touched it. `v3_cascade_ab.rs` and `p2c_headless_ab_wanaka.rs` remain enormous, `#[ignore]`d, compile-checked only, and load-bearing. The decision the entry asks for (split / archive / schedule) is still unmade, and wave 15's `strategy_comparison_h4.rs` is a third harness of the same shape. |
| P8 | Findings/report single-slot representation | **FIXED — wave 8 (C8)**. |
| P9 | Model debt knowingly accepted | **THREE OF FOUR CLOSED — wave 10 (C9)**; bullet 4 is now a pinned OPEN ANOMALY (refining the rest cell makes the shipped detector find LESS), owner: whoever next touches `rest_field::box_smooth_rest` / `nms_candidates` [owner re-pointed 2026-08-05, Checkpoint E ruling Q4/E10 — see `planning/review_2026-08-04/REST_GRID_ANOMALY_STUDY.md` §10; was `rest_field::measure_cross_section` — that function is downstream of the defect: it walks correctly from the cell it is given, and its own rim-quantisation contribution is bounded to one cell (≤0.5mm, ~0.25mm mean) and cannot by itself move reach by the logged 0.583mm, whereas `box_smooth_rest`'s fixed 3×3 window followed by `nms_candidates`'s single-cell argmax displace the ridge O(h) inboard of the true rest maximum on an asymmetric peak; `rest_grid_resolution_c9` stays green and untouched until a replacement explains and supersedes it]. |
| P10 | Repo hygiene one-offs | **FIXED — wave 1**, `288b19b` (whole-repo fmt) + `9e43b78` (`.gitignore`, which found a SECOND live instance). The foreign looping `cargo test` was environmental. |
| P11 | Pins and claims captured mid-wave | **STILL OPEN as a practice.** The fix shape is recorded, not enforced by anything. Wave 16 hit its own instance from the other side — see the note appended to that section. |
| P12 | Convex fixtures cannot adjudicate concave defects | **PARTLY ADDRESSED.** The offset bench is extended (rosette, holed, cascades); the M4 oracle fixtures are not, and that is recorded as a limit rather than fixed. |
| P13 | A deferral that names no decider is not a decision | **NEW, wave 16.** See the section below. |
| P14 | Absolute-mm NMS prominence floor vanishes under refinement | **NEW, found incidentally by W7 during the rest-grid anomaly study (Checkpoint E ruling Q4/E11), NOT FIXED.** Latent on the shipped groove fixtures; bites hardest on broad smooth rest maxima, i.e. real parts. See the section below. |
| P15 | A refused rest branch outside the threshold mask disappears from both lists | **NEW, found incidentally by W7 during the rest-grid anomaly study (Checkpoint E ruling Q4/E11), NOT FIXED.** A reporting blind spot independent of the rest-grid anomaly. See the section below. |

## P1. Transforms have no provenance contract (structural)

No post-generation transform (dressups, clip, descent optimizer) can *report*
its own move remap — each builds and consumes a `MoveRemap` privately. That is
why the semantic-trace fix (ae10cb2) needed the `SemanticLinkCarrier`
piggyback hack, and why the next index-carrying channel someone adds will
silently break again. Fix shape: a `*_with_provenance` return convention so a
transform that moves indices *must* hand back the map — compile-enforced.
Related: per-dressup semantic items bind `0..len` (every item claims the whole
path), so per-step attribution is meaningless; viz worker only builds a
semantic recorder when `debug_options.enabled`, so the worker's remap paths
are untested in the default configuration.

## P2. Silent-sentinel values (the X-19 class)

`0.0` meaning "not measured" keeps recurring: fixed for standing material
(Option-typed), but `axial_doc_fraction == 0.0` still means "unknown",
`part_footprint_fraction` emits 0.0 ⇒ silence when no grid exists, and
`min_z()` returns the padded grid's mesh-bbox floor on uncovered cells — the
direct cause of PR-8b's plane-wide clamp firing, and **every other `min_z()`
consumer is unaudited** (steep_shallow uses it for its ladder bottom). Worth
one sweep: every f64 field where zero/sentinel is semantically "missing"
becomes Option or gets an audited doc contract.

## P3. Per-surface reimplementation (the GUI-IA root cause, alive in core)

- CLI still carries a parallel `ToolpathDiagnostic` (D3 copies 5 shared fields
  across; the struct itself remains a second implementation).
- ~~`waterline.rs` / `compute/execute.rs` hold private copies of finish_setup
  pieces (Z-ladder, slope-window sentinel). waterline.rs also contains PR-8d's
  sub-quantum-segment defect from the same source — left for blast radius.~~
  **CLOSED C3 2026-08-02.** Half of it was never true: `execute.rs` has called
  the shared `slope_filter_active` since `4b105da`, the commit that created
  `finish_setup.rs`, and both that module's header and this line said
  otherwise for a year. The real items — waterline's Z-ladder copy and the
  missing PR-8d floor — are fixed. The floor's red-first evidence also
  reverses §8.1's attribution: the 0.000891 mm segments are MADE in
  `waterline_contours` and merely observed in the steep half.
- ~~Three divergent tapered-ball width models still coexist;
  `feeds/geometry.rs::tapered_ball_effective_diameter` is a straight cone
  (no tangency), ~5% off at 0.5 mm DOC, with no parity sentry against
  `MillingCutter::width_at_height`.~~ **CLOSED C3 2026-08-02**, and "~5%" was
  generous: measured +290.6% / −44.1% across the shipped taper geometries,
  with the growth term clamped DEAD at every production call site (`tip_r ==
  nominal_d / 2` makes it the constant `nominal_d`). Model retired; the one
  caller delegates to `ToolGeometryHint::engaged_diameter_at_doc`. Suggest's
  shallow tapered-ball feed rises +8–26% as a result — stated in
  `tests/tapered_width_model_parity_c3.rs` and the wave log.

## P4. Stringly-typed vocabularies with silent fallbacks

~~D3 fixed `expand_span_kind_synonyms` (was silently swallowing
`waterline_cleanup`), but the class persists: `p2c_headless_ab_wanaka.rs:~3754`
and `v3_cascade_ab.rs:~4327` still key off the `"Pencil claims"` *label*;
drill hole-vs-peck nesting is distinguished by label text ("Hole N" vs
"Hole N plunge M") — wants a third `RegionSpanRole`; and
`ToolpathSemanticParams` is a `BTreeMap<String, serde_json::Value>` bag where
typing and provenance die at the boundary (PR-0 documented the loss; the bag
itself is the debt).~~ **CLOSED C4 2026-08-02.** Drill nesting got TWO roles,
not the one asked for — naming only the child would have left "a hole is a
`GeneratorPass` in a drill operation" as an unwritten rule. The harness keys
go through `RegionKind::from_span_label`; a payload field was declined so
`toolpath_spans` keeps no finishing dependency, and a round-trip sentry makes
a label change break loudly in ONE place instead of silently at each
consumer. `SemanticKey` closes the bag's key half: 74 keys, 110 call sites,
zero literals left at any producer or reader, JSON wire pinned as
hand-transcribed strings. The VALUE half stays a `serde_json::Value` on
purpose — the values are genuinely heterogeneous. **H4's mix tables must be
built on roles or keys, never labels**; recorded in three doc comments.

## P5. `ComputeMessage` is at its size ceiling (recurring friction)

Three separate waves each boxed one new `Option` to stay under clippy's
280-byte `large_enum_variant` ceiling. The real fix is boxing
`ComputeMessage::Toolpath(Box<ComputeResult>)` (~16 sites, mostly viz tests).
Every future `ToolpathStats` field pays this tax until someone does it once.

## P6. No shared test-fixture library

Every wave re-derived or copied fixtures: the 17-field `ToolpathConfig`
literal, mixed-slope plateau meshes, groove generators, ProjectSession
builders. `standing_material_channel_am9.rs` became the de-facto template by
accident. Extracting a `tests/common/` fixture module (meshes, tools, session
builders, FNV fingerprint helper) would have saved measurable time in
literally every wave and will keep paying off through M3/M4/M5.

## P7. Ignored mega-harness rot

`v3_cascade_ab.rs` and `p2c_headless_ab_wanaka.rs` are enormous, `#[ignore]`d,
compile-checked only, and accumulate stale *claims in prose* (two retractions
edited in place this programme). They are simultaneously load-bearing
(headless wanaka access) and unrunnable in CI. Decide: split the reusable
loaders out, archive the campaign-specific analysis sections as docs, or give
them a scheduled characterisation cadence. Related gate-design hazard from
Checkpoint B: region-count topology gates are scale-sensitive for band-shaped
classes (a 0.05 mm reference fragments a 45° annulus into 39 components) —
any future §A.0-style gate needs a minimum-component or annulus-aware reading.

## P8. Findings/report single-slot representation

~~`GenerationFindings` has one derived-stepover slot for what can be two
derivations (first-writer-wins was a shrug, not a decision). Partial height
clipping is unreported (only total band collapse produces the D1 finding).
`narrate` still reports `regions 0` for Trace, Scallop, and SpiralFinish — the
same structural gap A/M8 closed for UnifiedFinish, unclosed for three ops.
RampFinish has no standing-material area channel (lift magnitude only).~~
**CLOSED C8 2026-08-02.**

* Derived stepover → `Vec`; `GenerationFindings` drops `Copy` for `RefCell`.
  The PR-6a rationale turned out to be ORPHANED onto the wrong function (a
  missing blank line between two `///` blocks), which is most of how
  first-writer-wins survived unexamined.
* Partial clips reported: measured 185.98 mm² laddering 5 of 9 levels and
  leaving **4.311 mm of an 8.31 mm groove wall unfinished**, previously
  silent on every surface. `Caution` vs `Info` in disjoint collections, so
  the loud finding still cannot be buried. *Only the `VerySteep` arm measures
  a clip at all* — MidSteep and Shallow remain invisible, recorded on the
  finding type.
* `regions 0` was **three different causes wearing one symptom**: scallop had
  a real partition dropped in transit, SpiralFinish has no partition, Trace
  had a naming divergence and a missing chain counter. Inventing a
  per-boundary count for the latter two would have reported a structure the
  generator does not have.
* RampFinish reports 370.5 mm² of ramp swath on its own fixture, with its own
  `MeasurementStage` and a sentry asserting it is NOT the ring-cascade
  provenance — and gained the narration line it never had.

## P9. Model debt knowingly accepted (documented, revisit-worthy)

- The reach policy's V-model is a model: a trapezoid is not a V, one wall
  angle either understates the wall or ignores the floor. The recorded
  generalisation — erode directly against the sampled `surface_z`
  cross-section, needing no angle at all — is strictly better and would
  retire the model; module doc in `reach.rs` records it.
- Claims stepover is one scalar sized at `min_valley_depth` while reach is
  per-point — `centerline_cut_paths` still takes a scalar fan.
- `tsp::remap_spans` is O(spans × moves); carrier spans added a constant
  factor to the dominant cost on wanaka-class jobs. Wants an interval index.
- Reach cross-sections are measured on the 0.5 mm rest cell against a Ø1 tip —
  H3's resolution question applies to the rest grid too.

**CLOSED 2026-08-03 by C-sequence wave 10** (`9963128` / `0dff17e` /
`ef4011c` / `d8d09da`; see the wave entry in `ORCHESTRATION_LOG.md`):

* Bullet 1 — `reach::solve_reach_sampled` exists, is matrix-gated, and BEATS
  the V model on the two shapes the V actually misreads. Production does NOT
  switch: its coverage/routing bars need a 0.002–0.010 mm cross-section pitch,
  50–250× finer than the shipped rest cell. `PRODUCTION_REACH_MODEL` carries
  the ruling. **Correction to this bullet's own premise:** a PLAIN trapezoid
  is not a shape the V misreads — the tip can never go below the floor, so
  only the straight wall constrains it, and the V encodes a straight wall
  exactly. The discriminating shape is a CHAMFERED groove.
* Bullet 2 — retired. The defect was an OVERLAP violation (63.66 % where the
  policy specifies 50 %), not the under-coverage it looked like.
* Bullet 3 — `RemapIndex`, 13.7× at 14.87 MB, outputs identical.
* Bullet 4 — one third answered, and it opened a NEW question rather than
  closing this one: refining the rest cell makes the shipped detector find
  LESS feature and makes measured reach collapse toward zero. Pinned as an
  open anomaly in `tests/rest_grid_resolution_c9.rs`; owner is whoever next
  touches `rest_field::box_smooth_rest` / `nms_candidates` [owner re-pointed
  2026-08-05, Checkpoint E ruling Q4/E10 — see
  `planning/review_2026-08-04/REST_GRID_ANOMALY_STUDY.md` §10; was
  `rest_field::measure_cross_section` — downstream of the defect: its
  rim-quantisation contribution is bounded to one cell (≤0.5mm, ~0.25mm
  mean) and cannot by itself move reach by the logged 0.583mm, while
  `box_smooth_rest`'s fixed 3×3 window plus `nms_candidates`'s single-cell
  argmax displace the ridge O(h) inboard of the true rest maximum on an
  asymmetric peak; `rest_grid_resolution_c9` stays green and untouched until
  a replacement explains and supersedes it].

## P10. Repo hygiene one-offs

- Repo is not `cargo fmt` clean (~50 pre-existing sites); every agent had to
  tiptoe around rustfmt cascades. One deliberate whole-repo `cargo fmt` commit
  (nothing else in it) ends the hazard permanently.
- `.gitignore` audit: the unanchored `diagnostics/` entry ate a source file
  (fixed, 52fa78a) — one pass over the remaining unanchored directory entries
  is cheap insurance. Standing lesson: `rg` respects .gitignore; use
  `grep -r` when a file may be ignored.
- A foreign repo's looping `cargo test` (sysml-spec-tests) collided with the
  one-cargo-job rule all session — an environment note for the operator, not
  a repo fix.

## P11. Pins and claims captured mid-wave (three instances, escalating)

**Observed:** M4, then wave 14 twice. A fingerprint or an expectation is
re-captured while a wave is still landing behaviour, so it records an
intermediate build. It then reads as the wave's conclusion forever.

The three instances, in order of how hard they are to catch:

1. **A stale number.** `finish_resolution_policy_pr3` pinned at 899 moves with
   a "−36.8%" narrative; the shipped value is 2674 (+87.9%). Caught by the
   test going red the next time it ran.
2. **Stale numbers in a doc.** Wave 9b's A0-vs-A9 margin table, measured with
   four-interval chord probing and drop-only decimation, both of which wave 14
   changed. Nothing goes red; the numbers just quietly stop being true, and
   they had already been cited twice.
3. **A stale RATIONALE.** `scallop_candidates_m4` asserted A2 ≡ A0 *because*
   "Fixed20's stride is 1 for any ring under 40 vertices, and the arc
   cascade's rings are that sparse". The premise died when the sampling bound
   landed. **This is the dangerous form: a stale number goes red, a stale
   reason does not** — unless the assertion names its own mechanism, which is
   the only reason this one was caught.

**A fourth instance, wave 16, from the other side.** A4's sentry pinned a
10 µm engagement floor chosen by reasoning rather than measurement. The
sentry then failed, and the failure was the fixture telling the truth: a pass
riding on ground it already cut measures ~18 µm of apparent material against
a sampled reference, so the pin would have been wrong for the rest of time in
the one direction the report must never err. The floor is now DERIVED from
the reference's resolution and travels on the finding. The lesson generalises
the section: **a threshold captured before its instrument is measured is the
same defect as a pin captured mid-wave** — both record an intermediate state
as a conclusion.

**Fix shape:** (a) re-capture every pin after a wave's LAST behavioural
commit, never during — cheap, and it would have prevented all of instance 1;
(b) write assertions that carry their reasoning in the message, so a red says
which premise moved rather than only that something did; (c) when a wave
changes a mechanism, grep the log for entries whose numbers were measured on
it and attach an erratum rather than leaving them to be re-cited.

**Related:** the `feedback_instrument_integrity` memory ("a changed instrument
makes its own docstring a lie you then cite") is the same failure one layer
up. This is that pattern applied to expectations rather than to instruments.

## P12. Convex fixtures cannot adjudicate concave defects (three places)

**Observed:** wave 14, in three independent places at once. The offset
vertex-inflation defect's entire mechanism is arc joins, which exist only at
reflex corners. Yet:

* the M4 scallop oracle fixtures are bounded by the convex mesh-bbox
  rectangle, so they could never show it;
* `benches/perf_suite.rs`'s offset benchmark was square-only — the one shape
  with no reflex corners at all;
* the PR-3 ridge, on which the +88%-moves question was posed, is smooth and
  reads 0.000 mm² gouge on every arm, so a quality question was nearly
  adjudicated on the one fixture with nothing to gouge.

**Fix shape:** a fixture set for a geometric primitive must include the shape
class the primitive struggles with, and the ONUS is on the fixture to prove it
can exhibit the defect. "The gate was green" means nothing if the population
could not have gone red. The bench is now extended (rosette, holed, cascades);
the oracle fixtures are not, and that is recorded as a limit rather than
fixed.

**Related:** the standing programme rule *"never gate on an aggregate without
rendering the surface"* — same family, different axis. That one is about the
statistic hiding the defect; this one is about the fixture never containing it.

## P13. A deferral that names no decider is not a decision (wave 14, adopted)

**Observed:** wave 14, then applied deliberately by every wave after it.

Wave 14 shipped `polygon::OffsetRingSet` and left two things undone. The
difference between them is the whole point:

* *"`FlattenPolicy` stays explicit at the boundary; migrating the remaining
  consumers is deferred"* — a decision, because it names who decides (the
  next consumer to need it) and on what evidence.
* *"cavalier can panic on the `Shape` path and be mapped to a collapsed
  offset"* — was, at first, a shrug. A known way for a hang to become a
  silent wrong answer, with nobody named. It is now carried in the open-items
  ledger with an owner condition attached.

**The rule:** every "not fixed, stated" line must name **who decides and on
what evidence**, or it is not a deferral — it is an unrecorded defect wearing
a deferral's clothes. Waves 8 through 16 each close with an explicit
"NOT fixed, stated" block for exactly this reason, and the Checkpoint E menu
(ledger §6.3) is the same rule applied to a whole programme: nine open items,
each with a named condition for re-opening.

**Related:** P11 is this failure applied to numbers, P12 to fixtures. All
three are the same shape — something intermediate recorded as if final.

## P14. Absolute-mm NMS prominence floor against a one-cell difference

**Observed:** found incidentally by W7 during the rest-grid anomaly study,
not part of the anomaly itself. Ledgered under Checkpoint E ruling Q4/E11.
Evidence: `planning/review_2026-08-04/REST_GRID_ANOMALY_STUDY.md` §3.4.

`rest_field.rs:1382,1418,1444` — `prominence = max(0.1·min_valley_depth,
NMS_PROMINENCE_FLOOR_MM)` with the floor at **0.005 mm absolute**, compared
against a difference measured over **ONE cell** of a box-smoothed field. For
a smooth maximum that one-cell difference scales as `½|f″|h²`, so against a
constant absolute bar it **vanishes under refinement** — a fine grid loses
ridges a coarse one keeps.

**Reproduction:** build a rest field with a broad SMOOTH maximum (not a
groove kink) and sweep `cell_mm`; candidate count falls as h falls. Note
explicitly that this is NOT what happens on the two c9 groove fixtures — a
groove's rest peak is a kink, so its one-cell prominence GROWS with
refinement, which is why this is latent rather than observed. It bites
hardest on broad smooth rest maxima, i.e. real parts.

**Status:** NOT FIXED.

**Owner condition:** whoever next changes NMS prominence or adds a
smooth-maximum rest fixture; re-open when a fixture with a broad smooth rest
maximum exists.

## P15. A refused rest branch outside the threshold mask disappears from both lists

**Observed:** found incidentally by W7 during the rest-grid anomaly study,
not part of the anomaly itself. Ledgered under Checkpoint E ruling Q4/E11.
Evidence: `planning/review_2026-08-04/REST_GRID_ANOMALY_STUDY.md` §4.

`rest_field.rs:986` — `clearing_comps.insert(comp)` is guarded by `comp !=
usize::MAX`. A branch that is detected, traced, measured, and then REFUSED,
whose ridge cells all sit outside the threshold mask, lands in neither
`centerlines` nor `clearing_regions` and vanishes without trace. This is a
**reporting blind spot** independent of the rest-grid anomaly.

**Reproduction:** any branch reaching `branch_verdict` with a non-Pencil
verdict and `comp == usize::MAX`.

**Status:** NOT FIXED.

**Owner condition:** whoever next touches `detect_rest_valleys`'s routing
exit; re-open with a fixture that produces `comp == usize::MAX` on a refused
branch.

## Process note (not repo debt)

IDE/rust-analyzer diagnostics were stale mid-edit snapshots after every agent
wave — scary-looking missing-field errors that contradicted green gates. The
working protocol: trust the agent's clippy/test ladder, confirm with one
`cargo check --workspace` before handing the cargo slot on. Never act on the
panel alone.
