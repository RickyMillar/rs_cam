# Feeds Data Ingest — Completion Results (Phase F)

**Date:** 2026-05-31
**Predecessor commits:** Phase A `d654936`, B `585e2d8`, C `1ebf55e`,
D `16eb650`, E `20cc4ed`.
**Plan reference:** `planning/feeds_data_ingest_completion_2026-05-31.md`.

Closes the completion plan with the final verification gate (Phase F)
and the diff vs the Phase A baseline.

## Phases shipped

| Phase | Commit | What landed |
|-------|--------|-------------|
| A | `d654936` | Empirical baseline (param_sweep `5a67c1d`-pinned snapshot + verdicts) + AS001–AS015 smoke section laid out for operator |
| B | `585e2d8` | Garr aluminum per-series flute split (228 → 239 embedded LUT rows; 11 promoted, Mid-Range + GP skipped honestly) |
| C | `1ebf55e` | AluminumAlloy enum 2 → 7 (added 2024-T3, 5052-H32, 3003-H14, 1100-O, 7050-T7651) |
| D | `16eb650` | PlasticFamily enum 5 → 11 (added UHMW-PE, PP, Nylon 6/6, ABS, PETG, Rigid PVC) + PlasticHardness::RockwellR |
| E | `20cc4ed` | `Material::SolidWoodByJanka` parametric variant + `WOOD_SPECIES_LIBRARY` (132 species) + S3-13 test_fixture helper |
| F | this doc | Final verification (param_sweep diff vs Phase A) |

## Sweep diff (autonomous half — green)

Both the Phase A baseline (post-`5a67c1d`) and the Phase E
re-run produce **identical verdicts and identical per-variant metric
summaries**.

```
Phase A: {'total_variants': 105, 'pass': 96, 'fail': 0, 'no_effect': 9, 'unexpected': 0}
Phase E: {'total_variants': 105, 'pass': 96, 'fail': 0, 'no_effect': 9, 'unexpected': 0}
Per-variant verdict diffs: 0
Per-variant summary string diffs (metric numbers changed): 0
```

The bit-for-bit match is the expected outcome: Phases B-E added
*data* (LUT rows, enum variants, library entries) but did not touch
any toolpath-generation behaviour, and the `param_sweep` fixtures
don't exercise the new material/LUT additions at any of their
parameter combinations. The Phase F gate confirms no incidental
regression slipped in via the shared code paths
(`vendor_normalize::material_to_lut`, plastic / aluminum match arms,
`Material::SolidWoodByJanka` constructor).

The same 9 NO_EFFECTs are present in both runs and are all the
pre-existing known-explained cases (face direction, inlay glue_gap,
pencil bitangency/num_offset_passes, pocket/profile climb, scallop
direction) — see Phase A baseline section of
`planning/data_ingest_2026-05-30/kc_retune_log.md` for the
per-rule decomposition.

### Snapshots and verdicts on disk

| Artifact | Phase A baseline | Phase E completion |
|----------|------------------|--------------------|
| Sweep dir snapshot | `planning/data_ingest_2026-05-30/sweep_snapshot_5a67c1d.tar.gz` (22 MB) | `planning/data_ingest_2026-05-30/sweep_snapshot_phaseE_completion.tar.gz` (22 MB) |
| Verdict JSON | `planning/data_ingest_2026-05-30/sweep_verdicts_5a67c1d.json` (38 KB) | `planning/data_ingest_2026-05-30/sweep_verdicts_phaseE_completion.json` (38 KB) |
| Raw cargo log | `target/sweep_baseline_5a67c1d.log` (transient) | `target/sweep_phaseE_completion.log` (transient) |

Note: the Phase A `sweep_verdicts_5a67c1d.json` is regenerated in
this commit because the original Phase A capture used
`>file 2>&1` which corrupted the JSON with stderr-interleaved
summary text. The replacement is the clean JSON-only output from
re-analyzing the Phase A snapshot tarball — the underlying data is
unchanged.

## MCP smoke (operator-only — pending)

The MCP smoke AS001-AS015 section of `kc_retune_log.md` is laid out
under "Phase A baseline (post-`5a67c1d`)" with a per-case table
ready to fill in. **Operator action item:** run the smoke suite via
`cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp` and populate
the table. A separate "Phase F final (post-completion)" section
should also be added after the Phase E commit lands at the
operator's bench, with the same per-case peak µm / chipload verdict
/ power verdict / rapid-collision-count columns.

The deflection-model investigation policy still applies: if any
case flips past 200 µm AND is operator-known-good in practice,
file a deflection-model finding — do NOT widen
`EXCEEDS_BOUND_MM`.

## Test counts across phases (sanity check)

| Phase | lib | integration | literature_parity sentries |
|-------|-----|-------------|-----------------------------|
| Pre-A (post-`5a67c1d`) | 1668 | 1843 | 11 |
| A | 1668 | 1843 | 11 (no code) |
| B | 1668 | 1843 | 11 (data only) |
| C | 1668 | 1848 | 16 (+5 aluminum) |
| D | 1668 | 1854 | 22 (+6 plastic) |
| E | 1668 | 1859 | 24 (+2 SolidWoodByJanka helper anchors) |
| F | 1668 | 1859 | 24 (verification only) |

Net additions across A–F: **+16 integration tests, +13 sentries, +132
wood species, +11 Garr LUT rows, +5 aluminum alloys, +6 plastic
families.**

## 10/10 ledger

Per the completion plan's "Where we are (6.5/10)" → "What 10/10
means" mapping:

| Plan stream | Status | Where it lives |
|-------------|--------|----------------|
| 1. Empirical validation | **Autonomous half DONE; operator MCP smoke PENDING** | sweep snapshots + verdicts in `planning/data_ingest_2026-05-30/`; smoke table in `kc_retune_log.md` |
| 2. Garr aluminum | **DONE** | `garr_aluminum.json` 11 rows; planning `phaseB_2026-05-31.md` |
| 3. Plastic family extension | **DONE** | 6 new variants + RockwellR; planning `phaseD_2026-05-31.md` |
| 4. Aluminum alloy extension | **DONE** | 5 new variants; planning `phaseC_2026-05-31.md` |
| 5. Wood species expansion | **DONE (core + data); GUI dropdown DEFERRED** | `SolidWoodByJanka` + `WOOD_SPECIES_LIBRARY` 132 entries; planning `phaseE_2026-05-31.md` |

Effective rating: **9/10** — every code/data deliverable shipped,
two follow-ups remain (one operator, one GUI):

1. **Operator MCP smoke run** — bench time, ~30 min, not blocking
   on any code change.
2. **GUI wood-species library dropdown** — `crates/rs_cam_viz/`
   change to surface `WOOD_SPECIES_LIBRARY` as a searchable
   material picker. Out of scope for the core-crate work; cleanest
   landed as its own commit when the UI pattern is clear.

The audit deferrals folded into Phases D / E:

- **S2-9 (leaky LimitInputs):** NOT folded into Phase D — the code
  paths don't actually overlap with plastic Kc, per `phaseD_2026-05-31.md`
  "explicit non-fold" section. **Closed 2026-05-31 in commit `933b2a1`**
  as a standalone surgical refactor: `PowerLimitInputs.kc_eff_n_per_mm2`
  → `kc_n_per_mm2` (raw), with the canonical
  `GRAIN_ANISOTROPY_FACTOR` applied at the consumer in
  `feed_modulation.rs::adaptive_feed_modulate_inner`. Audit doc
  proposed a `<'a>` lifetime + `&Material` + `&ToolDefinition`
  shape; on inspection the leak is purely the pre-multiplication
  so the field rename is the cleaner fix. F-039 pinned numerics
  held bit-exactly (`2.0 × 30 = 60`).
- **S3-13 (test fixture clutter):** `Material::test_fixture_custom()`
  helper landed in Phase E. **Closed 2026-05-31 in commit `78f53ac`**:
  all 4 audit-cited ad-hoc `Material::Custom { name, hardness_index,
  kc }` constructions substituted. Initial assessment said "3 of 4
  fit" but on inspection all 4 sites are variant-gated (`is_wood_class()`
  / `matches!(.., Custom { .. })` only check the tag, not the
  scalars) and substitute cleanly with clarifying comments where
  the original scalars were decorative.

**Post-completion follow-ups also shipped this session:**

- **GUI wood-species library picker** — commit `111b4dd` in
  `crates/rs_cam_viz/src/ui/properties/stock.rs`. Searchable
  ComboBox below the existing Material dropdown surfaces
  `WOOD_SPECIES_LIBRARY` (132 entries) with a 🔍 text filter
  matching against display_name + scientific_name. Selection
  constructs `Material::SolidWoodByJanka { ... }` and routes
  through the existing `StockMaterialChanged` event. Closes the
  only real gap from the post-completion GUI audit.

## Explicit out-of-scope items (per completion plan, not regressed)

- Per-spindle gate for Freud 1/2" industrial rows — parked in
  `data/vendor_lut/industrial_only/` since Phase 4.
- PMMA Grade B Kc promotion — staging "do NOT promote" caveat stands.
- POM/Delrin and PC milling Kc — still genuine gaps; refusal-first.
- `MachineKinematics::shapeoko_xxl_ricky_tuned()` — untouched.
- `crates/rs_cam_core/src/feeds/explain.rs` and
  `crates/rs_cam_viz/src/ui/feeds_modal.rs` — still untracked per
  operator rule.

## Resume note

The completion-plan-scoped work is closed AND the post-completion
audit follow-ups (S2-9, S3-13, wood-species GUI picker) are closed
too. Single remaining item:

- **Operator** — run AS001-AS015 MCP smoke and fill the
  `kc_retune_log.md` Phase A + Phase F sections. Requires the
  live GUI via `cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`.
  Not blocking on any further code change.

All other completion-plan deliverables AND all audit follow-ups
are landed and locked behind literature_parity sentries +
provenance gates. Final ledger: **10 commits this session, zero
regressions, 1668 lib + 1859 integration + 24 literature_parity
tests green, clippy clean across all phases.**
