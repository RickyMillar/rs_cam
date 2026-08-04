# Reference-fixture repeatability — pre-registered study

Wave: W7 (R6/M3) · Date: 2026-08-05 · Companion to `REFERENCE_FIXTURE_SPEC.md`
Parent revision: `ebe77de` (branch `experiment/adaptive-spiral`)

**Execution status: the simulation arms are NOT RUN.** The machine holds one
Cargo slot and W7 is third in the queue; the exact commands are in §7 and a
scheduled execution pass should run them unchanged. Everything that could be
established **without** the slot has been established and is reported as
measured. Everything else carries a pre-registered bar so the run cannot be
retro-fitted to a desired answer.

The rule this document exists to enforce, from the plan's acceptance gates:
**bins come from repeated runs, never from a desired dial.**

---

## 1. The prior failure, restated arithmetically

`SUPERSEDED_CONCLUSIONS.md` §4 records that the v3 gate's ±10 µm bin was
*"below repeatability and aliased by the 0.25 mm grid."* That was the right
call, but it was asserted. Here is the arithmetic.

A dexel column samples the surface at a **fixed lateral position**. Two arms
whose surfaces sit at different phase relative to that lattice — which is
exactly what two different ring/raster strategies produce — read a Z difference
of up to `cell · tan θ` from geometry alone, with no machining difference at
all. Measured table (`artifacts/w7/arp1_measurements.md` §6, render
`artifacts/w7/arp1_alias_bound.png`):

| sim cell | θ = 45° | θ = 75° | θ = 85° |
|---|---|---|---|
| 0.50 mm | 500 µm | 1866 µm | 5715 µm |
| **0.25 mm** | **250 µm** | **933 µm** | 2858 µm |
| 0.10 mm | 100 µm | 373 µm | 1143 µm |
| 0.05 mm | 50 µm | 187 µm | 572 µm |
| 0.02 mm | 20 µm | 75 µm | 229 µm |
| 0.01 mm | 10 µm | 37 µm | 114 µm |

**The prior ±10 µm bin at a 0.25 mm cell was below its own alias bound by 25×
on flat ground and 93× at 75°.** No amount of re-running could have rescued it.

Two caveats, stated so the bound is not over-applied:

- The alias is **common-mode and cancels** for a paired comparison where both
  arms present the *same* surface to the *same* columns. It does not cancel
  when the arms differ in phase — which is the only case anyone runs an A/B for.
- The bound is a worst case over a one-cell phase shift. The realised value is
  smaller and is what §4's runs measure. The bound is what a bin must clear
  *before* the run, so the run is not the first place anyone finds out.

---

## 2. Four things that get called "repeatability", and must not be pooled

The prior campaign's single biggest instrument error was mixing these. They
have different causes, different fixes, and only two of them set a bin.

| # | source | operation | expected | if non-zero |
|---|---|---|---|---|
| **V1** | **Re-run variance** | same binary, same inputs, run twice | **exactly 0** | a **determinism defect**, not a bin. Fix the code; do not widen the bin. |
| **V2** | **Regeneration variance** | regenerate the toolpath, re-simulate | small, > 0 | **this sets the bin.** Cause is generator non-determinism (thread scheduling, hash order, parallel reduction order). |
| **V3** | **Discretisation sensitivity** | change sim cell or mesh step | large, systematic | **not variance at all.** Deterministic and reproducible. Never pool with V1/V2; never compare across cells (standing rule). |
| **V4** | **Alias / phase sensitivity** | same surface, shifted relative to the lattice | §1's bound | sets a **floor** the bin must clear, independent of V1/V2. |

**The minimum reportable bin = max(V2 bar, V4 bound).** V3 is excluded by
construction. V1 must be zero or the study stops and reports a defect.

The prior campaign's *"never compare across regenerations"* rule is V2 being
discovered the hard way. This study measures it instead.

---

## 3. What is already established, without the Cargo slot

| finding | status | evidence |
|---|---|---|
| The V4 alias bound is `cell·tan θ` and is closed form | **MEASURED** | §1, `arp1_measurements.md` §6 |
| The prior ±10 µm bin fails V4 by 25–93× | **MEASURED** | §1 |
| Tessellation error follows `s²`; last-halving ratios 3.93–4.02 | **MEASURED** | spec §4.2 |
| Tessellation error can be driven to ≤ 0.7 µm p99 on every zone at a stated step | **MEASURED** | spec §4.2–4.3 |
| Therefore **the mesh is not the binding limit** on bin width; the instrument is | **DERIVED, sound** | tess p99 0.66 µm ≪ V4 floor 50 µm at 0.05 mm cell / 45° |
| V1 (re-run) is zero | **NOT RUN** | §7 arm A |
| V2 (regeneration) magnitude | **NOT RUN** | §7 arm B |
| V3 (cell sensitivity) shape | **NOT RUN** | §7 arm C |

The fourth row is the study's most useful pre-run result: **refining the
fixture further buys nothing.** At the qualified cell the tessellation error is
two orders of magnitude below the alias floor, so effort belongs on the
instrument's cell size, not on the mesh.

---

## 4. Pre-registered bars

Written before any arm runs. A result outside a bar is published as-is.

**B-1 (V1, determinism).** Two runs of the identical binary on identical
inputs must produce **byte-identical** `column_deviations` — all values, in
order. Bar: `0` differing columns out of the full population. Any non-zero
count is reported as a determinism defect with the differing column indices,
and the study **stops setting a bin** until it is explained. *Vacuity guard:*
the comparison must be over ≥ 10⁵ columns and the harness must prove it read
two genuinely separate runs (distinct process PIDs recorded).

**B-2 (V2, regeneration).** Regenerate the toolpath 5× from the same project
and simulate each. Report the per-column p50/p95/p99/max of
`|z_run_i − z_run_1|`. **The minimum reportable bin is the p99 of this
distribution, rounded UP to the next value in {5, 10, 20, 50, 100, 200} µm.**
This number is the deliverable; it is not permitted to be chosen to fit a
desired dial. *Vacuity guard:* if V2's p99 is `0.000`, the generator is fully
deterministic and the bin is set by V4 alone — that is a legitimate outcome and
must be stated as such, not treated as a failed measurement.

**B-3 (V4, alias, realised).** Translate the fixture by half a cell in X and
re-simulate without regenerating. The realised per-column difference must be
`≤ cell·tan θ` in every slope band. Bar: ≤ 1% of columns exceed the bound
(allowing for the finite-difference slope estimate at band edges). A larger
exceedance means the bound is wrong and §1 must be rewritten before any bin is
adopted.

**B-4 (V3, cell sensitivity, characterization only).** Sweep cell ∈
{0.25, 0.10, 0.05} mm. **No bar** — this arm exists to document the shape and
to make the "never compare across cells" rule concrete with numbers. It must
**not** be used to choose a bin.

**B-5 (non-repeatable region).** Report the slope band and cell combinations
where the required bin exceeds the smallest *interesting* quality difference
(taken as 20 µm, the coarsest scallop dial in the spec's range). Those
combinations are declared **non-reportable** and no gate may be written on
them. Prediction, from §1 and to be checked: **VerySteep at every cell ≥
0.05 mm is non-reportable**, and MidSteep is non-reportable at ≥ 0.10 mm.

---

## 5. Fixture and population

- **Fixture:** `ReferencePlate::arp1()` per the spec, at the §4.1 tessellation
  rule with ε = 1 µm. Zones scored independently via `zone_at` — never a
  dilated band map.
- **Reference surface:** analytic (`z_at`), not the mesh. This is what makes
  the study possible: the residual has an exact zero point.
- **Population:** all columns with a defined zone and a defined analytic
  surface. Per-zone and per-band quantiles reported separately; **no
  whole-part aggregate is reported as a headline**, per the standing rule that
  an aggregate hid a 28 mm uncut block last time.
- **Instrument:** `SimulationResult::column_deviations`, with
  `column_grid_cell_mm` recorded on every row (M1 provenance). Cross-checked
  against the `scallop_oracle` envelope on at least one zone; the two
  instruments disagreeing is itself a finding.

---

## 6. Explicitly out of scope

- Any strategy comparison. This study measures the **instrument**, on a
  fixture with no strategy in it beyond a synthetic analytic raster.
- Any change to production rest-grid resolution or to sim defaults.
- `terrain.stl` repeatability. It has no analytic zero point, so a
  regeneration study on it can only measure V2, not V4 — worth doing later,
  not here.

---

## 7. The exact commands, NOT RUN

To be executed unchanged by a scheduled pass holding the Cargo slot. Slot
discipline before **every** command: `free -g` (≥ 10 GB available) and
`pgrep -af "carg[o]" | grep -v bwrap | grep -v claude` (empty), plus
`df -h / | tail -1`.

```bash
# Arm A — V1 determinism (bar B-1). Two separate processes, PIDs recorded.
cargo test -p rs_cam_core --test reference_repeatability \
    v1_rerun_is_byte_identical -- --ignored --exact --nocapture

# Arm B — V2 regeneration variance (bar B-2). 5 regenerations.
cargo test -p rs_cam_core --test reference_repeatability \
    v2_regeneration_variance -- --ignored --exact --nocapture

# Arm C — V3 cell sensitivity (B-4, characterization only).
cargo test -p rs_cam_core --test reference_repeatability \
    v3_cell_sensitivity -- --ignored --exact --nocapture

# Arm D — V4 realised alias (bar B-3). Half-cell translation, no regenerate.
cargo test -p rs_cam_core --test reference_repeatability \
    v4_half_cell_phase_shift -- --ignored --exact --nocapture

# T1 tier, cheap, runs in the normal suite once reference_plate.rs lands:
cargo test -p rs_cam_core --test reference_plate_contract -- --nocapture
```

The harness `crates/rs_cam_core/tests/reference_repeatability.rs` does not yet
exist; it is part of the Checkpoint E package, not of this research wave.

---

## 8. Verdict

**No bin is adopted by this document.** Two of the four bin inputs are
measured and two are NOT RUN.

What is safe to say today, and is enough to answer the plan's question about
whether B1/B2 may re-open:

1. **The fixture is no longer the limit.** Tessellation error is ≤ 0.7 µm p99
   at a stated, measured step on every zone — two orders below the alias floor
   at any cell anyone would run.
2. **The instrument's cell is the limit, and its floor is closed form.** A bin
   below `cell·tan θ` cannot separate two arms on that slope, ever.
3. **Therefore a defensible bin requires choosing a cell first**, and at
   0.05 mm the flat-ground floor is already 50 µm — five times the bin the
   prior campaign tried to gate on. Any future fine-quality gate must either
   run a much finer cell than has ever been run here, or restrict itself to
   Shallow ground, or use the analytic envelope oracle instead of COLUMNS.
   **That third option is the cheap one and this study recommends costing it
   before anyone buys a finer dexel grid.**
4. **B1/B2 should not re-open on this evidence.** A qualified fixture was
   necessary; it is not sufficient. The bin is still unmeasured.
