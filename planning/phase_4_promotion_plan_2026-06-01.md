# Phase 4 — Verification + LUT promotion grand plan

**Authored:** 2026-06-01
**Source plan:** `planning/feeds_data_ingest_2026-05-30_phased_plan.md`
(Phase 4 spec at line 1029)
**Prereqs done:** Phase 1 (Unblock), Phase 2 (Kc retune), Phase 3
(parallel collection fleet — 8 beats landed staging files)
**Post-Phase-3 audit followup also done:** Phase A–F (Garr per-series
split, Aluminum / Plastic / Plywood variant expansion + LUT promotion,
SolidWoodByJanka + WOOD_SPECIES_LIBRARY, hierarchical material picker).
S2-8 / S2-9 / S3-13 audit cleanups landed in the same window.

## Bottom line

**30 observations** sit in `planning/data_ingest_2026-05-{29,30}/*.json`
that are **NOT yet** in `crates/rs_cam_core/data/vendor_lut/
observations/`. That's the Phase 4 scope. Everything else from
Phase 3 already went through informal promotion (210 observations
across 19 source files are live).

Of those 30 staged-not-live observations:
- **15 Amana** — `ams159_vgroove_v2` + `spektra_engraving_v4`
  (v-groove + engraving rows). Schema-clean. Should promote
  straight through.
- **10 Garr** — general purpose milling + aluminum sub-range
  ladders. Schema-clean. Should promote.
- **4 Freud** — flagged by `validate_lut.py` as chipload suspicious
  (0.53–0.69 mm/tooth on "half" series). Re-fetch source for column-
  confusion check, then promote or move to `_gaps.md`.
- **1 Onsrud** polycarbonate — schema-clean single row. Promote.

## Validation infrastructure status

`planning/data_ingest_2026-05-30/validate_lut.py` was the script
landed at Phase 1's exit and re-used by Phase 3. Current output:

```
TOTAL rows: 121
=== PROBLEMS (4) ===
  [vendor_breadth.json] freud-solid-carbide-half-hardwood:         chipload max suspicious (0.5334)
  [vendor_breadth.json] freud-solid-carbide-half-softwood:         chipload max suspicious (0.5842)
  [vendor_breadth.json] freud-solid-carbide-half-mdf-particle:     chipload max suspicious (0.6858)
  [vendor_breadth.json] freud-solid-carbide-half-plywood-hardwood: chipload max suspicious (0.5334)
=== USABLE NOW (wood + schema-complete + chipload): 112 ===
=== NOT-LIVE (staged) reason counts ===
  Counter({'non-wood': 9})
```

The 9 NOT-LIVE non-wood rows are aluminum + plastic from
`vendor_breadth.json` — schema-clean but skipped by the "wood +
chipload" usable filter. They're still safe to promote; the filter
is conservative, not gating.

## Sequenced step plan

### Step 4.1 — Schema validation gate

Re-run the validator and resolve every PROBLEM line. The four Freud
"half" entries are the only outstanding problems:

```bash
python3 planning/data_ingest_2026-05-30/validate_lut.py
```

**Disposition for each Freud row:**
1. Independent agent re-fetches `source_url` for each suspicious row.
2. Confirm or refute the chipload column (Freud's PDF often presents
   `feed per minute` and `chipload per tooth` adjacent — column
   swaps are a known failure mode).
3. If confirmed → keep with a `notes` field documenting the high
   value. If refuted → correct the row in `vendor_breadth.json` and
   re-run validate_lut.
4. If irrecoverable → move the row out of `vendor_breadth.json` into
   `vendor_breadth_gaps.md` with a reason.

Estimated time: 30 min (4 URL fetches + reconciliation).

### Step 4.2 — Adversarial verifier agent

Dispatch one `general-purpose` agent (it does web work). The prompt:

```
You are an independent VERIFIER. Read every JSON file in
planning/data_ingest_2026-05-29/ and planning/data_ingest_2026-05-30/.
For each: pick 3-5 random observations + every observation that looks
like an outlier (chipload > 0.3 mm/tooth, RPM > 24000, very small
diameter < 2 mm, very large diameter > 12 mm, unfamiliar vendor).
Re-fetch each row's source_url and CONFIRM or REFUTE the diameter_mm,
chipload range, RPM, flute_count, and included_angle_deg (for v-bits)
against the actual page. Report a markdown table CONFIRM/MISMATCH with
the source's number alongside ours. ALSO answer: any signs of
column-confusion, unit errors (in vs mm, IPT vs mm/tooth), or
material-family misclassification? Modify NOTHING.
Output to planning/data_ingest_2026-05-30/verification_report.md.
```

Any MISMATCH must be reconciled BEFORE Step 4.3. Plan for ~10-15
reconciliations on a 30-row promotion; some will be tooltip-precision
nits (3.175 vs 3.18), some real errors.

Estimated time: ~45-60 min agent work + 30 min reconciliation.

### Step 4.3 — Test-gated promotion (the actual code change)

For each verified row, the promotion script:

1. **Group staged rows by source_id** into one or more files under
   `crates/rs_cam_core/data/vendor_lut/observations/<vendor>_<scope>.json`.
   - Amana `ams159_vgroove_v2` → `amana_vgroove_v2.json`
     (merge with `amana_vgroove_engraving.json`? probably a new file —
     don't co-mingle source_ids in a file)
   - Amana `spektra_engraving_v4` → `amana_spektra_engraving_v4.json`
   - Garr aluminum sub-ranges → `garr_aluminum_milling.json` (or
     merge into the existing `garr_aluminum.json` if the source_id
     scheme allows)
   - Garr `general_purpose_milling` → `garr_general_purpose.json`
   - Onsrud polycarbonate → append to `onsrud_plastic.json`
   - Freud (post-Step-4.1 corrections) → append to
     `freud_solid_carbide.json` OR create `freud_router_bit_csv.json`
     (depends on source-id-per-file rule)

2. **Update `VendorLut::embedded()`** in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs` — add new
   `include_str!` entries.

3. **Update both count assertions:**
   - `tests::test_embedded_loads_all_observations` in `vendor_lut.rs`
   - `embedded_count_matches_after_expansion` in
     `tests/vendor_lut_sub_1mm.rs`
   - Both bump by exactly the count of net-new observations.

4. **Run the gate** (per file promotion or batched — batched is
   faster but harder to bisect on regression):
   ```bash
   cargo test -p rs_cam_core --lib   2>&1 | grep -E "test result|FAILED"
   cargo test -p rs_cam_core --tests 2>&1 | grep -E "test result|FAILED"
   cargo clippy -p rs_cam_core --all-targets -- -D warnings
   cargo run -p rs_cam_cli -- smoke --output /tmp/smoke_phase4.csv
   cargo run -p rs_cam_cli -- smoke --diff \
     --baseline planning/toolpath_acceptance/baselines/2026-06-02.csv \
     --output /tmp/smoke_phase4.csv
   ```

5. **On red:**
   - If F-024 / F-026 / F-027 / F-028 / F-031 / F-036b sentry fires,
     ROLLBACK that file's promotion. Find the row(s) that match the
     failing test scenario; either correct the row's material_family /
     diameter / chipload band, or pull just those rows back to
     `_gaps.md`.
   - If `smoke --diff` reports `Within → Exceeds` on any case,
     investigate per-case: the new LUT row may be promoting a
     more-conservative chipload that drops a previously-LUT-matched
     case onto a different row. If the new chipload reflects reality,
     this is correct and the baseline should advance; otherwise pull
     the offending row.

6. **MCP smoke (only if wood rows promoted)** — operator-bound.
   Walk AS001–AS015 manually, confirm 7/7 Within deflection bar
   still holds. Skip if all promoted rows are aluminum / plastic
   (the deflection bar is wood-only).

Estimated time: ~60-90 min code + ~30-60 min reconciliation if
any sentry fires.

### Step 4.4 — CREDITS + manifest update

For every NEW bundled source_id (de-duplicated across the 30 rows):
1. Append to `crates/rs_cam_core/data/vendor_lut/source_manifest.json`.
2. Add a CREDITS.md line under "Vendor LUT source manifest" dated
   2026-06-01 (or whenever the actual promotion lands).

NEW sources (4):
- `amana_ams159_vgroove_v2` — already in manifest? check
- `amana_spektra_engraving_v4` — likely new
- `garr_milling_aluminum_low_range` / `_mid_range` / `_high_range` —
  probably 3 separate manifest entries or one umbrella source_id;
  whichever Phase 3 collected used
- `garr_general_purpose_milling` — likely new
- `freud_router_bit_feed_and_speed_for_cnc_20170822` — probably new
  (post-Step-4.1 surviving rows)
- `onsrud_routing_polycarbonate_article` — likely new

Estimated time: 15 min.

### Step 4.5 — Phase 4 consolidation report

Write `planning/feeds_data_ingest_consolidation_2026-06-01.md`
listing:
- Rows collected this round (broken out by Phase 3 beat).
- Rows promoted live (30 if all survive).
- Rows kept staged with reason (any Freud rows that failed Step 4.1,
  any aluminum/plastic rows that the LUT-fallback filter rejected).
- Kc / hardness findings folded into the audit baselines (already
  done in Phase 2B retune doc; reference, don't re-list).
- Open gaps after Phase 4 (vendors not yet collected: Kennametal,
  Vortex, Sandvik — these were Phase 3 stretch goals that didn't
  land).

Estimated time: 30 min writing.

## Exit gate (Phase 4 closure)

All four must hold:

- [ ] `cargo test -p rs_cam_core --lib` and `--tests` both `0 failed`.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo run -p rs_cam_cli -- smoke --diff` exits 0 vs the
      2026-06-02 baseline (no Within → Exceeds regressions).
- [ ] CREDITS.md + `source_manifest.json` reflect every newly bundled
      source_id.
- [ ] Phase 4 consolidation report written.

If MCP smoke was required (wood rows promoted): also
- [ ] AS001-AS015 deflection bar holds 7/7 Within in operator MCP walk.

## Total time estimate

**Autonomous-doable:** 2.5-4 hours
- Step 4.1: 30 min
- Step 4.3 (promotion): 60-90 min code
- Step 4.4: 15 min
- Step 4.5: 30 min
- Buffer for ~10-15 reconciliations: 45-60 min

**Verifier-agent dependent:** +45-60 min Step 4.2 (agent runtime,
mostly idle on web fetches) + 30 min human reconciliation pass.

**Operator-dependent (only if wood rows promoted):** +20-30 min MCP
smoke walk.

Realistic total: **half a day of focused work** (4-5 hours) with the
agent running in parallel.

## Abort / rollback criteria

Per the master plan: any verdict regression in a `_f0*` test that
can't be traced to a single new row is grounds to roll back the whole
promotion batch. The 30-row scope makes bisection cheap — promote in
3-4 batches (Amana vgroove+engraving, Garr aluminum, Garr general,
Onsrud+Freud salvage) and `git reset --hard` whichever batch reddens.

## What this DOESN'T cover (intentional)

- **Vendor breadth round 3** — Kennametal / Sandvik / Vortex were
  Phase 3 stretch beats that nobody collected. Out of scope here;
  open `_gaps.md` flagged.
- **Per-species Kc derivation** — Phase 3 beat C (FPL Ch.5 shear-∥
  extract) landed `fpl_ch5_extract.md` as a citation-backed reference
  but did NOT wire it into `Material::SolidWood::kc_n_per_mm2`.
  Doing so would shift wood-class Kc values and break the deflection
  bar — needs a coordinated Phase 5 (analog of Phase 2B for solid
  wood). Tracked under TODO Phase 3 markers in `material.rs:709`.
- **AS013 stock_top_z methodology** — separate CLI smoke methodology
  gap, tracked in `planning/toolpath_acceptance/baselines/2026-05-31_
  postphasef_notes.md`.
- **Operator MCP smoke** — operator-bound; do when bench time
  available.

## Resume checkpoints

If interrupted mid-Phase-4, the safe resume points are between
Steps 4.1 / 4.2 / 4.3 batches / 4.4 — each is a single git commit
and no two should be entangled. The promotion commit itself should
be one-batch-per-commit so rollback is `git revert <hash>`, not
`git reset --hard`.
