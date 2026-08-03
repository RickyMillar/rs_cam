# Antipatterns and tech debt observed during the 2026-07-29/30 programme

Cross-cutting observations from fifteen agent reports plus orchestration.
These are NOT scheduled programme items (M3/M4/M5/A-M6/A-M7/A-M10/H4/L1 are
tracked elsewhere) — this is the "noticed along the way" ledger, grouped by
pattern. Ordered roughly by leverage.

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
  touches `rest_field::measure_cross_section`.

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

## Process note (not repo debt)

IDE/rust-analyzer diagnostics were stale mid-edit snapshots after every agent
wave — scary-looking missing-field errors that contradicted green gates. The
working protocol: trust the agent's clippy/test ladder, confirm with one
`cargo check --workspace` before handing the cargo slot on. Never act on the
panel alone.
