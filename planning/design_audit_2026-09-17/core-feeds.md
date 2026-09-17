# Design and feature-debt audit — core-feeds

Group: `crates/rs_cam_core/src/feeds/` (calculator, Suggest model, vendor LUT,
geometry/force/profile, provenance, rationale). All findings in this file are
`owner: power session` — no proposal here changes physics, constants, bands
or the calculator's numbers. Audit is of structure and shape only.

### FDS-01 SuggestContext.stock is None at every mutating call site
- kind: feature-debt
- pattern: field read but never set from any surface
- where: `crates/rs_cam_core/src/feeds/suggest.rs:144` (`SuggestContext` def, `stock` field), `crates/rs_cam_core/src/feeds/suggest/apply.rs:741-785` (`apply_drill_defaults`, the sole reader), `crates/rs_cam_core/src/session/mod.rs:1770` (the one populator)
- evidence: `rg -n "SuggestContext\s*\{" --type rust crates/` finds 11 non-test fresh-construction sites (`rs_cam_cli/src/smoke.rs:541`; `rs_cam_viz/src/app/mcp/commands.rs:867`; `rs_cam_viz/src/controller/events/{toolpath.rs:125,model.rs:768,mod.rs:1029,mod.rs:1222}`; `rs_cam_viz/src/ui/properties/{pills.rs:149,feeds_speeds.rs:210}`; `rs_cam_core/src/session/{compute.rs:1189,multitool.rs:795}`) plus one doc example (`rs_cam_cli/examples/apply_suggest_save.rs:73`). Every one of the 10 GUI/MCP/CLI-smoke/multitool sites uses `..SuggestContext::default()` and never sets `stock`, leaving it `None`. Only `session/mod.rs:1770` (the read-only `cutter_op_profile` rationale path used by `get_suggest_rationale` and the feeds modal) and the CLI example populate it: `stock: Some(&stock_ctx)`. `pills.rs:148` even says so in a comment: "the wired Suggest sites leave `SuggestContext::stock` empty too." `apply.rs:741-749` documents the consequence explicitly: `apply_drill_defaults`'s `AlignmentPinDrill` peck-depth clamp against `stock_z + spoilboard_penetration` "cannot be applied" when `stock` is `None` — "the value is left at the unclamped Suggest default, exactly as before."
- proposal: Give the ~10 write-path call sites the same one-line `StockContext::from_stock_bbox(session.stock_bbox(), stock.padding)` that `session/mod.rs:1770` already computes, or add a `SuggestContext::for_session(&ProjectSession, ...)` constructor next to `cutter_op_profile` that every mutating caller uses instead of hand-rolling `..SuggestContext::default()`.
- breaks: none (widens a `None` to `Some`; `apply_drill_defaults` already treats `None` as a no-op fallback, so no signature changes)
- effort: S
- risk: low — the sentry `rubbing_floor_never_exceeds_band` and `pill_writes_clamped_value_g_pillclamp.rs` exercise the funnel, but no shipped test currently pins a pin-drill peck depth added via the GUI/MCP path with `stock` populated
- sentry: none today for the mutating paths; write one that adds an `AlignmentPinDrill` toolpath via the same call path `commands.rs:867` uses and asserts the peck clamp fired
- owner: power session

### FDS-02 CutterOpProfile.predictions / .constraints computed on every call, read by nobody outside profile.rs
- kind: feature-debt
- pattern: computed value nothing consumes (repeat of an already-retired defect class)
- where: `crates/rs_cam_core/src/feeds/profile.rs:42-79` (`Predictions`, `ConstraintEnvelopes` defs), `:145,:142` (fields on `CutterOpProfile`), `:206-214` (populated in `for_combo`)
- evidence: `rg -n "\.predictions\b|\.constraints\b" --type rust crates/` outside `feeds/profile.rs` returns zero hits — no file in `rs_cam_viz`, `rs_cam_cli`, `rs_cam_mcp` or any `tests/` integration file reads either field. The two live production callers of `cutter_op_profile` (`rs_cam_viz/src/app/mcp/generation.rs:38-58`, `rs_cam_cli/src/project.rs:707-724`) read only `profile.feasibility`, `.warnings`, `.suggested_operation`, `.feeds`. `profile.rs:55-60` shows this is a repeat: a sibling field `observed_chipload` on the same `Predictions` struct was deleted 2026-08-13 for the identical reason ("computed here on every profile and never rendered... its only behavioural consumer was Suggest pass 8, itself retired").
- proposal: Either wire `.predictions`/`.constraints` into the MCP `get_suggest_rationale` JSON and/or the GUI feeds modal (the stated purpose — "one preflight view"), or retire the two fields from `CutterOpProfile::for_combo`'s hot path the same way `observed_chipload` was retired, keeping the underlying `predict_peak_deflection_um` / `predict_move_count` calls that `suggest/invariants.rs` already uses directly.
- breaks: removing the fields breaks `CutterOpProfile`'s public shape (used only inside `feeds/profile.rs`'s own tests per the same grep)
- effort: S
- risk: low — the closed-form predictors themselves stay wired through `suggest/invariants.rs`; only the profile-level aggregate copies are unread
- sentry: `feeds/profile.rs`'s own module tests (`for_combo` tests at :293,:338,:365,:373) construct but do not assert on `.predictions`/`.constraints`; none would break
- owner: power session

### FDS-03 Three "chipload band" types in feeds/, three different construction names
- kind: design
- pattern: one concept, several representations, ad hoc conversions
- where: `crates/rs_cam_core/src/feeds/mod.rs:421-432` (`ChiploadBounds`), `crates/rs_cam_core/src/feeds/geometry.rs:184-204` (`DeratedChiploadBand`, `.into_pair()`), `crates/rs_cam_core/src/feeds/quantities.rs:200-222` (`VendorChiploadBand`, `::new()` / `::from_advance_range()`)
- evidence: all three are `{min, max}` pairs describing the same vendor chipload band in mm/tooth, each with a different, non-shared construction/conversion API: `DeratedChiploadBand::into_pair() -> Option<(f64,f64)>` (feeds/geometry.rs:200), `VendorChiploadBand::new(min, max)` and `::from_advance_range(&Range<f64>)` (quantities.rs:208,217), and `ChiploadBounds` built by field literal at call sites (e.g. `suggest.rs`). Each is individually well-justified in its own doc comment (required-both vs optional-min semantics, typed-unit display boundary vs raw internal value) but there is no shared trait or single naming convention across the three, so a reader has to learn three different vocabularies for the same shape.
- proposal: Give the three types one common, named conversion method (e.g. all expose `fn bounds(&self) -> Option<(f64, f64)>` or a shared tiny trait) rather than three ad hoc names, without merging the types themselves (their differing semantics are legitimate).
- breaks: none if added as a new shared method; a rename of the existing methods would break their current call sites (`geometry.rs`'s one call to `.into_pair()`, `quantities.rs`'s `::new()`/`::from_advance_range()` callers in `efficiency.rs` and `tool_load/mod.rs`)
- effort: S
- risk: low — pure naming/API-surface addition, no behavior change
- sentry: `feeds/quantities.rs` unit tests (`classes_match_the_shipped_viewport_thresholds`, etc.) and `feeds/geometry.rs` module tests already pin each type's own behavior
- owner: power session

### FDS-04 LookupQuery: 9-field parameter struct with a duplicate type alias
- kind: design
- pattern: parameter struct with more than eight fields
- where: `crates/rs_cam_core/src/feeds/vendor_lookup.rs:13-21` (`LookupQuery`), `:24` (`pub type LookupCriteria = LookupQuery;`)
- evidence: `LookupQuery` has 9 fields (`tool_family`, `tool_subfamily`, `diameter_mm`, `flute_count`, `material_family`, `hardness_kind`, `hardness_value`, `operation_family`, `pass_role`), one over the brief's 8-field design-smell threshold. Line 24 additionally aliases it as `LookupCriteria`, and both names are used interchangeably at call sites (`find_best_row(lut, criteria: &LookupCriteria)` at :131 vs `lookup_best(lut, query: &LookupQuery)` at :575) — one type under two names in the same file.
- proposal: Drop the `LookupCriteria` alias and standardize on `LookupQuery` everywhere in this file (a rename, not a behavior change). Leave the 9-field struct as-is unless a future change adds a tenth field — flagging it now so the next addition is the trigger to group the hardness/material fields into a sub-struct.
- breaks: renaming `LookupCriteria` → `LookupQuery` at its 2-3 call sites is a signature-name change, not a behavior change
- effort: S
- risk: low
- sentry: `cargo test -p rs_cam_core -q --test lookup_parity`
- owner: power session

### FDS-05 GeometryClass::ShallowTerrain / SteepTerrain are unreachable today
- kind: feature-debt
- pattern: half-built capability, dead enum variant matched against
- where: `crates/rs_cam_core/src/feeds/geometry_class.rs:20-45` (`GeometryClass` def), `:89-113` (`classify_3d_terrain`, the only producer), `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs:110` (a match arm naming `ShallowTerrain`)
- evidence: `classify_3d_terrain` (geometry_class.rs:89-113) has exactly one non-`Unknown` return statement, `GeometryClass::MixedTerrain` (line 113); `ShallowTerrain` and `SteepTerrain` are declared variants (`:29-45`) that no function in the crate ever constructs (`rg -n "GeometryClass::(ShallowTerrain|SteepTerrain)" --type rust crates/` matches only the enum's own doc comments and the one dead alternative at `adaptive_entry.rs:110`, `GeometryClass::MixedTerrain | GeometryClass::ShallowTerrain => ...`). The module's own doc (`:9-15`) says why: "today the 3D op types collapse to `MixedTerrain`... slope-histogram analysis... tracked as follow-up work."
- proposal: No action needed beyond documentation — this is a self-declared, honestly-labeled placeholder, not a silent gap. If picked up, note in `FEATURE_CATALOG.md` (if it claims terrain-aware entry-style selection) that the terrain classifier is op-type-only today.
- breaks: none (no-op finding)
- effort: S
- risk: low
- sentry: `geometry_class.rs`'s own module tests (`:187,:206,:223,:233,:242`) already pin that only `FeatureDriven`/`PocketLike`/`MixedTerrain`/`Unknown` are reachable
- owner: power session

## Top three

1. **FDS-01** SuggestContext.stock is None at every mutating call site — cheapest fix (S), and the doc comment already names the exact consequence (pin-drill peck clamp silently skipped) on every GUI/MCP/multitool path.
2. **FDS-02** CutterOpProfile.predictions / .constraints computed on every call, read by nobody — same defect class the codebase already fixed once (`observed_chipload`, 2026-08-13); either wire it in or retire it the same way.
3. **FDS-05** GeometryClass::ShallowTerrain / SteepTerrain unreachable — zero-risk, already self-documented; listed for completeness of the census, not because it needs work.

## Checked and clear

- `feeds::geometry::doc_derating_scale` is genuinely canonical: `tool_load/chipload.rs:192` re-exports it (`pub(super) use crate::feeds::geometry::doc_derating_scale;`) with a drift-guard comment rather than redefining it.
- `dressup/feedopt.rs::rctf` and `feeds/efficiency.rs::deflection_chipload_ceiling_mm` are documented thin wrappers that delegate to `feeds::geometry`/`feeds::force`, not duplicate implementations — the dup-sweep's 0.89-0.92 hits on these pairs are false positives (shared doc-comment shape, not shared logic).
- `feeds::vendor_lut::LutOperationFamily` and `vendor_normalize::op_family_to_lut` look like a duplicate-enum pair in the dup sweep but are the already-consolidated fix for a named prior defect (C5, "re-deriving the mapping inline") — one canonical mapping function, not scattered duplicates.
- `feeds::profile::CutterOpProfileInput` and `feeds::suggest::SuggestForOperationInput` are field-for-field identical by design, per `profile.rs:81-84`'s own doc comment (kept separate only so their borrowed lifetimes can differ) — not an accidental duplicate.
- The four `pub` items the tech-debt census flagged as test-only (`feed_explanation.rs::unit_family`, `::predicted_gate_observation_mm`, `suggest/apply.rs::write_to`, `::preview_field_apply`) are each explicitly labelled "Test door: `<test file>`" in their own doc comments — a deliberate, consistently-applied convention for integration-test seams that must be `pub` to cross the crate boundary, not oversight.
- `feeds::quantities` newtypes are intentionally narrow (5 quantities, per its own module doc: "deliberately not a units framework") — the 39 remaining raw-`f64` `pub fn` signatures elsewhere in the folder are out of that module's stated scope, not an inconsistency.
- `embedded_vendor_lut()` loads the ~20 embedded JSON files exactly once via `std::sync::LazyLock` (`feeds/mod.rs:50-51`) — no per-call re-parse, no ad hoc cache to consolidate.
- `FeedsError` is a structured enum (`WrongToolForOperation { operation, actual_geometry, required }`), not a `String` error; no `map_err` that drops a cause was found in the folder's non-test files.
- The three explanation types (`rationale.rs::SuggestRationale`, `feed_explanation.rs::FeedExplanation`, `explain_payload.rs::FeedsExplain`) are not three copies of one thing: each is cross-documented against the other two and serves a distinct, all-live purpose (warning→human string, stage-labelled quantity record, UI data contract) — `FeedExplanation` is constructed live at `tool_load/chipload.rs:423` and rendered via `diagnostics/adapters/from_tool_load.rs` and `rs_cam_viz/src/ui/properties/feeds_speeds.rs`.
- `FeedsResult.formula: Option<FormulaBreakdown>` (flagged dead-pub by the prior census) is in fact read live at `rs_cam_viz/src/ui/feeds/why.rs:237` — the census predates this consumer or missed it.

## Add-a-thing count

Adding one new feeds dimension (a `FeedsField` variant, e.g. a hypothetical ramp-angle dial) touches at minimum:

- `crates/rs_cam_core/src/feeds/provenance.rs` — the `FeedsField` enum itself, plus `FeedsProvenance`'s per-field storage and its `.set()`/`.get()` match (12 `FeedsField::` references).
- `crates/rs_cam_core/src/feeds/suggest/apply.rs` — `FieldApplyPreview::write_to`'s match, `preview_field_applies`'s slot list, and `preview_field_apply` (17 references).
- `crates/rs_cam_core/src/compute/validate.rs` — the field's validation dispatch (2 references).
- `crates/rs_cam_core/src/session/compute/params.rs` — the session-level param-write dispatch (5 references).
- `crates/rs_cam_viz/src/ui/properties/pills.rs` — the GUI pill for the new field (5 references; outside this group's folder but on the same extension point).
