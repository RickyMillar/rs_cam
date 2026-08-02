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

D3 fixed `expand_span_kind_synonyms` (was silently swallowing
`waterline_cleanup`), but the class persists: `p2c_headless_ab_wanaka.rs:~3754`
and `v3_cascade_ab.rs:~4327` still key off the `"Pencil claims"` *label*;
drill hole-vs-peck nesting is distinguished by label text ("Hole N" vs
"Hole N plunge M") — wants a third `RegionSpanRole`; and
`ToolpathSemanticParams` is a `BTreeMap<String, serde_json::Value>` bag where
typing and provenance die at the boundary (PR-0 documented the loss; the bag
itself is the debt).

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

`GenerationFindings` has one derived-stepover slot for what can be two
derivations (first-writer-wins was a shrug, not a decision). Partial height
clipping is unreported (only total band collapse produces the D1 finding).
`narrate` still reports `regions 0` for Trace, Scallop, and SpiralFinish — the
same structural gap A/M8 closed for UnifiedFinish, unclosed for three ops.
RampFinish has no standing-material area channel (lift magnitude only).

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

## Process note (not repo debt)

IDE/rust-analyzer diagnostics were stale mid-edit snapshots after every agent
wave — scary-looking missing-field errors that contradicted green gates. The
working protocol: trust the agent's clippy/test ladder, confirm with one
`cargo check --workspace` before handing the cargo slot on. Never act on the
panel alone.
