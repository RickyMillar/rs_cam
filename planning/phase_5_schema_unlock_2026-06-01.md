# Phase 5 — Schema unlock + per-species Kc wiring

**Authored:** 2026-06-01
**Predecessor:** `planning/phase_4_promotion_plan_2026-06-01.md` (closed
2026-06-01 in commit `18a056b`).
**Why now:** Phase 4 left 17 vendor LUT rows staged-but-unpromotable
due to schema shape, plus a separate per-species wood Kc wiring TODO
that's been carrying since Phase 2B. The user explicitly authorized
breaking schema changes: *"schema changes are cheap, we have no
consumers yet."*

## Goal in one sentence

Unlock the 17 deferred rows by making the schema fit the data (rather
than forcing the data into a wood-mill schema), then close the
per-species wood Kc loop so `Material::SolidWood::kc_n_per_mm2` is
backed by FPL Ch.5 data instead of folklore.

## Scope

Four sequenced workstreams, each independently committable + gateable:

1. **AS013 `stock_top_z` smoke methodology fix** (smallest — runner-only)
2. **V-bit schema relaxation** (`diameter_mm: Option<f64>` + matcher) —
   unblocks 16 staged rows (15 Amana v-bit/engrave + 1 Onsrud article)
3. **Fiberglass MaterialFamily + Material variant** — unblocks 1 staged
   row (Garr GP-plastics) and adds a real composites class
4. **Per-species wood Kc wiring from FPL Ch.5** — biggest; coordinated
   Kc retune analog to Phase 2B

## Out of scope (intentional)

- **F-033 workflow advisory** (pre-sim warning for finishing op without
  prior rough). Tracked separately; not in this phase.
- **Bench validation against real cuts.** That's the only thing that
  closes the sim-vs-reality gap, but it's operator-bound and parallel
  to this phase. Phase 5 work is sim-internal schema + data wiring;
  bench validation happens whenever the user runs real toolpaths and
  reports back.
- **Vendor breadth round 3** (Kennametal/Sandvik/Vortex). Open since
  Phase 3 stretch beats; defer to a separate ingest round.

## Operational guardrails (READ FIRST)

These come from prior-session feedback memory. Follow exactly:

- `cargo test --workspace` **loops indefinitely** — use
  `cargo test -p rs_cam_core --lib` and `-p rs_cam_core --tests`
  instead. Per-crate is the only safe form.
- **NEVER run `cargo test` while a release build of `rs_cam_viz` is
  in flight** — the combo thrashes swap and crashes the machine.
  Always `pgrep -af "cargo --release|rs_cam_gui"` first; if anything
  prints, wait or coordinate with the user.
- `cargo fmt` cascades to sibling modules — don't run workspace-wide;
  rely on `cargo clippy` for style enforcement.
- **Zero-warning clippy** (16 deny lints in `Cargo.toml`). Use
  `#[allow(clippy::the_lint)]` + `// SAFETY:` comment for provably
  safe sites only.
- Tests modules carry `#[allow(clippy::unwrap_used, expect_used,
  panic, indexing_slicing)]` already — use them freely in tests.
- **Never** stage `crates/rs_cam_core/src/feeds/explain.rs` or
  `crates/rs_cam_viz/src/ui/feeds_modal.rs` — both are untracked WIP
  that must stay unstaged. You CAN edit them if a refactor breaks the
  build, but never `git add` them.
- **Never** touch `.mcp.json` or
  `MachineKinematics::shapeoko_xxl_ricky_tuned()`.
- **Never** use `--no-verify`, `--no-gpg-sign`, `--amend` for published
  commits, or `git reset --hard`.
- **Sentries that MUST stay green** through the whole phase:
  F-024, F-026, F-027, F-028, F-031, F-036b, F-037 smoke baseline
  diff. Each step's exit gate re-runs these.

## Sequenced step plan

### Step 5.1 — AS013 `stock_top_z` smoke methodology fix (~45 min)

**Problem:** `cases_agent_smoke.csv` row AS013 has
`baseline_params=...;stock_top_z=30;...`, but the smoke runner's
`set_toolpath_param` call rejects `stock_top_z` as
*"unknown parameter for 3D Rough operation"*. Round-09's MCP run
applied it via the project's stock config, not the toolpath
operation schema. Result: AS013 reads 262.9 µm Exceeds in CLI smoke
vs round-09's 105 µm Within.

**Fix in `crates/rs_cam_cli/src/smoke.rs`:**
- In `materialize_case_toolpath`, before the
  `set_toolpath_param` loop, intercept any param key with a
  `stock_` prefix and route it to `session.stock_mut()` field
  mutation instead. Likely keys: `stock_top_z`, `stock_bottom_z`,
  `stock_thickness` — but `stock_top_z` is the only one used today.
- For `stock_top_z=N`, set `stock.size.z = N - stock.origin_z`
  (or directly mutate the appropriate field — check `StockConfig`
  shape in `crates/rs_cam_core/src/compute/config.rs` for the actual
  field name; the 2D-stock-Z-frame memory note mentions
  `origin_z must be negative so stock top is at Z=0`).
- Log a `tracing::info!` when a stock_ param is intercepted.

**Verification:**
1. Rebuild + rerun smoke:
   `cargo run -p rs_cam_cli -- smoke --output /tmp/smoke_step1.csv`
2. Verify AS013 deflection drops from 262.9 µm → ~105 µm (matches
   round-09 STATE.md target).
3. F-037 baseline must still pass — update `2026-06-02.csv` to a new
   `2026-06-03.csv` if AS013 verdict changes; bump
   `smoke_baseline_regression_f037.rs::baseline_path()` accordingly.
4. Document in `planning/toolpath_acceptance/baselines/<new>_notes.md`.

**Commit message style:** `fix(smoke): route stock_* params to stock config — closes AS013 setup gap`

---

### Step 5.2 — V-bit schema relaxation (~2-3 hours)

**Problem:** 16 staged rows can't promote because
`VendorObservation.diameter_mm: f64` requires a value but v-bit speed
charts publish per-angle rows without a fixed body diameter (the
cutter's cutting diameter depends on engagement depth for a tapered
V-bit). The Onsrud polycarbonate article is the same shape — a
chipload window that applies across the polymer-routing diameter
range.

**Affected staged rows:**
- 11 Amana AMS-159 v-bit rows: `amana-vbit-*-trace-{18,30,45,60,90}deg-{1,2}f`
- 4 Amana Spektra engraving rows: `amana-engrave-*-trace-{30,45}deg-1f`
- 1 Onsrud row: `onsrud-article-polycarbonate-optimum-chipload-window`

Source files:
- `planning/data_ingest_2026-05-29/amana.json`
- `planning/data_ingest_2026-05-29/onsrud_whiteside.json`

All 16 already verifier-CONFIRMED (see
`planning/data_ingest_2026-05-30/verification_report_2026-06-01.md`)
— pure schema unlock, zero data review needed.

**Fix:**

A. `crates/rs_cam_core/src/feeds/vendor_lut.rs`:
   ```rust
   pub struct VendorObservation {
       ...
       pub diameter_mm: Option<f64>,  // was f64
       ...
   }
   ```
   Update `EMBEDDED_OBSERVATION_COUNT` doc comment to mention
   diameter-optional rows now allowed.

B. `crates/rs_cam_core/src/feeds/vendor_lookup.rs`:
   Update `find_best_row_for_geometry` (and any sibling matcher).
   When `query.tool_family` is `ChamferVbit` / `VBit`:
   - First try matching against rows with `diameter_mm.is_some()`
     using existing diameter-scored logic.
   - Fall back to rows with `diameter_mm.is_none()` if no scored hit;
     score those by `(tool_family, included_angle_deg)` proximity.
   For non-v-bit families, behavior unchanged — `diameter_mm.is_none()`
   rows are skipped (they shouldn't exist for those families anyway).

C. Update existing live observations: every JSON in
   `crates/rs_cam_core/data/vendor_lut/observations/*.json` has rows
   with concrete `diameter_mm`. JSON keeps numeric values; serde
   handles `Some(f)` ↔ JSON number transparently. **No data file
   migration needed.**

D. Promote the 16 staged rows. For Amana → append to existing
   `crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json`
   (it already has `ams159_vgroove_v2` + `spektra_engraving_v4`
   source_ids per the Phase 4 audit). For Onsrud → append to
   `onsrud_plastic.json`.

E. Bump `test_embedded_loads_all_observations` and
   `embedded_count_matches_after_expansion` from 247 → 263 (+16).

**Verification:**
1. `cargo test -p rs_cam_core --lib` — all 1669 + new tests pass.
2. `cargo test -p rs_cam_core --tests` — F-037 etc still green.
3. `cargo clippy -p rs_cam_core -p rs_cam_cli -p rs_cam_viz -p rs_cam_mcp --all-targets -- -D warnings`.
4. CLI smoke diff vs current baseline — no regressions expected
   (v-bit rows don't match wood-class AS001-AS017 cases except
   possibly AS008/AS009/AS010, which use 90° v-bits already covered
   by existing rows).
5. Add a focused matcher test: feed a `ToolGeometryHint` with
   `included_angle_deg = 30` and verify the new Amana 30° v-bit row
   is selected.

**Commit:** `feat(vendor_lut): relax diameter_mm to Option for v-bit + diameter-window rows + promote 16 deferred rows`

---

### Step 5.3 — Fiberglass / composite material class (~3-4 hours)

**Problem:** The 1 staged Garr "Fiberglass/Plastics/G10" row
(`garr-gp-plastics-6000-flat` in
`planning/data_ingest_2026-05-29/harvey_helical_garr.json`) can't
promote because its `material_family: plastic` isn't a `MaterialFamily`
enum variant. Fiberglass/G10 is genuinely a different cutting class
(abrasive, fiber-reinforced) — collapsing it onto Acrylic or HDPE
would be wrong.

**Fix:**

A. Add `Fiberglass` variant to `MaterialFamily` in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs:94`. Use that name
   over `CompositeFiberReinforced` for brevity; "fiberglass" is the
   common cutting-floor term for G10/FR4/glass-epoxy/glass-polyester
   abrasive composites.

B. Add corresponding `Material::Fiberglass { grade: FiberglassGrade }`
   variant to `crates/rs_cam_core/src/material.rs`. `FiberglassGrade`
   enum with at least `G10Fr4`, `Generic`. (G10/FR4 is the canonical
   electrical-grade glass-epoxy; "Generic" covers other glass-resin
   composites at lower confidence.) Source for properties:
   - G10/FR4 Kc: ~120 N/mm² typical (highly abrasive, blunts carbide
     fast). Multiple sources put it at 100-150 N/mm²; pick 120 as
     mid-band conservative. **Cite the source in CREDITS.md** — find
     one before committing.
   - Janka: N/A. `wood_hardness_lbf` returns None.
   - feed_scale_factor: ~0.8 (heuristic — fiberglass cuts at lower
     feeds than aluminum but higher than wood; sub-1.0 reflects
     "softer than wood by feed but more abrasive"; refine when bench
     data exists).
   - drill thresholds, plunge rate: conservative for abrasive
     composites — drill chip-welding 3.0 D/d (low, chips evacuate
     well), per-peck max 1.0 D, plunge 200 mm/min default.

C. Update every match arm in `material.rs` that pattern-matches
   `Material::*` to include `Material::Fiberglass`. There are ~15
   such accessors (`kc_n_per_mm2`, `feed_scale_factor`,
   `wood_hardness_lbf`, `plunge_rate_base`,
   `drill_chip_welding_threshold_dtd`, `drill_per_peck_max_dtd`,
   `drill_speed_rule_envelope`, `category`, `label`, `catalog`,
   `to_key`, `from_key`, `base_cutting_speed_m_min`, etc.). Grep
   `match self {` and `match material {` in `material.rs`.

D. Update `vendor_normalize::material_to_lookup` in
   `crates/rs_cam_core/src/feeds/vendor_normalize.rs` to map
   `Material::Fiberglass { .. }` → `MaterialFamily::Fiberglass`.

E. Add `MaterialCategory::Composite` (or extend an existing) and
   wire into the hierarchical picker in
   `crates/rs_cam_viz/src/ui/properties/stock.rs`. Optional this round
   — material.rs is the load-bearing change; GUI surface is polish.

F. Add literature_parity sentry pinning G10/FR4 Kc to its citation.

G. Promote the Garr row: append corrected version (set
   `material_family: fiberglass`) to a new
   `crates/rs_cam_core/data/vendor_lut/observations/garr_general_purpose.json`
   (one file for Garr GP rows, including the 2 aluminum GP rows that
   currently sit in `garr_aluminum.json` — consider moving them; or
   create a new file just for the fiberglass row). Bump count to 264.

**Verification:** same gate as Step 5.2 plus the literature_parity
sentry.

**Commit:** `feat(material): add Fiberglass MaterialFamily + Material variant + promote 1 deferred Garr row`

---

### Step 5.4 — Per-species wood Kc wiring from FPL Ch.5 (~4-6 hours)

**Problem:** `Material::SolidWood::kc_n_per_mm2()` returns hand-tuned
folklore values per species (see TODO at `material.rs` near line 709).
Phase 3 beat C landed
`planning/data_ingest_2026-05-30/fpl_ch5_extract.md` with FPL Ch.5
Table 5-3a shear-parallel-to-grain values for ~30 species, but they
were never wired into `kc_n_per_mm2`. Wiring will shift wood-class Kc
values — coordinated retune analogous to Phase 2B (which did the same
thing for sheet goods).

**Approach (mirror Phase 2B pattern):**

A. **Compute the derivation.** Read `fpl_ch5_extract.md`. For each
   species in `WoodSpecies` enum + each species in
   `WOOD_SPECIES_LIBRARY` that has a matching FPL entry, derive
   `Kc_milling ≈ shear_parallel_grain × size_effect_factor`, where
   the size-effect factor accounts for the difference between
   3-point shear-block testing (FPL test) and peripheral milling
   (chip thickness ~0.05-0.3 mm). Reference: the existing
   `janka_to_kc_n_per_mm2` formula in `material.rs:483` uses
   `janka/100` as a first-pass; FPL shear data should refine
   per-species.

   Document the chosen formula + size-effect factor in a new
   `planning/data_ingest_2026-05-30/wood_kc_derivation.md` so the
   next reader can re-derive.

B. **Write per-species Kc values** into
   `Material::SolidWood::kc_n_per_mm2()` per-arm. Replace the
   hand-tuned constants with the derived FPL-backed values.

C. **Capture a smoke baseline pre-change:**
   `cargo run -p rs_cam_cli -- smoke --output /tmp/smoke_pre_kc.csv`

D. **Apply the Kc change.** Re-run smoke + diff vs pre-change. Expect
   shifts on AS001 (hardwood pocket), AS002 (softwood adaptive),
   AS003 (hardwood profile), AS013 (softwood adaptive3d). The
   deflection gate consumes raw Kc (no anisotropy factor) so per-
   species changes will be felt directly.

E. **If smoke shifts trigger Within→Exceeds regressions** on any
   case, this is the Phase 2B-style decision moment: is the new Kc
   correct (and the regression real)? Or is the size-effect factor
   wrong? Two outcomes:
   - **Accept regression as more honest** (Kc was previously
     wrong-low and the deflection prediction was overoptimistic).
     Update baseline to capture the new state.
   - **Refine the formula** (the size-effect factor may need to be
     smaller for some species class). Iterate.

F. **Add literature_parity sentries** pinning each per-species Kc to
   its FPL citation, mirroring the Phase C aluminum + Phase D
   plastic patterns.

G. **Update wood-Kc TODO markers** in `material.rs` to "closed
   Phase 5 step 5.4 commit `<sha>`".

H. **Update CREDITS.md** with a 2026-06-XX block citing FPL Ch.5
   Table 5-3a as the wood Kc data source.

**Verification:** same gate + the new literature_parity sentries.

**Commit:** `feat(material): per-species wood Kc from FPL Ch.5 — coordinated retune`

---

### Step 5.5 — Phase 5 consolidation report (~30 min)

Write `planning/feeds_data_ingest_consolidation_2026-06-0X.md`
listing:
- Bundled LUT count delta (247 → 264 expected: +16 v-bit/window
  + 1 fiberglass = +17 net; minus any rows pulled during gate
  iterations).
- Rows promoted in each step.
- Schema changes landed (with file paths + commit shas).
- Per-species wood Kc shifts (table: species | pre | post | smoke
  case affected | verdict shift if any).
- Open gaps after Phase 5 (vendor breadth round 3, F-033 advisory,
  bench validation).
- The list of `_gaps.md` files that can now be marked CLOSED for
  their Phase 5 entries.

## Exit gate (Phase 5 closure)

All four must hold after Step 5.4:

- [ ] `cargo test -p rs_cam_core --lib` 0 failed.
- [ ] `cargo test -p rs_cam_core --tests` 0 failed.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
  (workspace clippy IS safe — only `cargo test --workspace` loops).
- [ ] `cargo run -p rs_cam_cli -- smoke --diff --baseline <new> --output <fresh>`
  exits 0 against the post-5.4 baseline.

If Step 5.4 shifts the smoke baseline:
- [ ] New baseline file at
  `planning/toolpath_acceptance/baselines/2026-06-XX.csv` captures
  post-retune state.
- [ ] `smoke_baseline_regression_f037.rs::baseline_path()` points
  at new file.
- [ ] Notes file documents each per-case shift with rationale.

## What "complete" means here vs. bench reality

This phase closes the **sim-internal schema and data wiring** gaps.
It does NOT validate that the simulator's predictions match real
cuts. That validation is operator-bound and orthogonal — the user
runs a real toolpath, reports back, and we file a finding if sim
disagrees.

After Phase 5: the sim has 247+17 vendor rows, FPL-backed per-species
wood Kc, fiberglass/composites as a first-class material, and v-bit
rows that match by angle instead of fake diameter. The sim is then
"internally honest." Whether it's correct against reality is a
separate question that bench cuts answer.

## Resume checkpoints

Each of Steps 5.1-5.5 is one commit and independently revertable.
Safe pause points are between steps. Step 5.4 (the Kc retune) is the
only one that may require iteration — if the smoke diff exposes
regressions, expect 1-2 inner cycles of "tune size-effect factor,
re-run smoke."

Realistic total: **a full day of focused work**, roughly 10-14 hours
including the Kc retune iteration if it goes 1-2 rounds.

## Pickup instruction for next session

Read this file. Then start at **Step 5.1** (AS013 stock_top_z). Run
the gate after each step. Pause and consult the user **only** if:
- Step 5.4 Kc shifts produce regressions that can't be resolved by
  the formula-tuning loop within 2 iterations.
- Any unexpected sentry (F-024 etc.) reddens.
- The fiberglass Kc citation hunt comes up empty.

Otherwise proceed through all four steps + 5.5 consolidation, commit
each, and report when Phase 5 closes.
