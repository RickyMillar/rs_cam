# Kc milling calibration — make the load gates physics-anchored, not shear-folklore

**Status:** CORE IMPLEMENTED 2026-06-17 (pending rebuild + re-read). The
approach was **refined during implementation** — see "Implementation note"
below; the plan's `GRAIN_SPREAD = 1.4` was dropped in favour of following the
existing Phase 2B sheet-good precedent.
**Date:** 2026-06-17
**Owner question that triggered this:** "verify the 8% spindle-load claim… that's a big lever."
Verifying it surfaced a real modelling gap that flips the tool-limited vs
machine-limited regime call for the wanaka job.

## One-line summary

The **deflection gate runs on shear-parallel `Kc` (~13 N/mm² for hardwood),
which is ~2.7× below measured peripheral-milling `Kc` (~32–38 N/mm²)**. Because
deflection is the gate that decides *tool-limited vs machine-limited*, this
under-read is why the wanaka rough *looked* machine-limited. Corrected, peak
deflection moves from ~96–153 µm to ~260–410 µm — past the 200 µm wall — i.e.
**tool-limited**, which is the regime where the constant-load (ContourSpiral)
strategy earns its keep. Fixing this is a precondition for any keep/cull
decision and for the "tool-limited / machine-limited operation" UX framing.

## Implementation note (refined from the plan below)

While implementing, found that **Phase 2B (2026-05-30) already did this exact
calibration for sheet goods**: `material.rs` SheetGood values are measured
*milling* Kc (MDF 31.4, particleboard 35.0 — the Sydor/Pałubicki figure), and
the comment there records dropping `ANISOTROPY_MULTIPLIER 2.5 → 2.0` to "keep
the product `Kc × factor` honest." **Solid wood is the only path still on raw
FPL shear.** So the correct, lower-risk move is to *follow that precedent*, not
invent a new split:

- **Milling-calibrate the solid-wood base** via `MILLING_KC_FACTOR = 2.7`
  (applied to the `SolidWood` and `SolidWoodByJanka` arms only). Now solid-wood
  `kc_n_per_mm2()` returns milling Kc, exactly like sheet goods.
- **Leave `GRAIN_ANISOTROPY_FACTOR = 2.0` untouched** — Phase 2B already tuned
  it to pair with milling bases. No `power.rs`/`deflection.rs` logic change
  needed: both gates already read `kc_n_per_mm2()`, so the lift propagates to
  deflection (×2.7) and power (now `2.0 × milling`) automatically and
  consistently with sheet goods.

Net effect unchanged in spirit (deflection ×2.7 → tool-limited regime shows up),
but the diff is far smaller and follows precedent instead of diverging from it.

**Shipped:** `material.rs` (const + two arms + comments + sentry test),
`tool_load/power.rs` (one test's expected value), `wanaka_full_tuned.toml`
(spindle → `VfdConstantTorque 1.5 kW @ 24 000`). NOT touched: `power.rs` /
`deflection.rs` / `feeds` / `feed_modulation` logic (they inherit the base).

## Evidence

### Code trace (current behaviour)

| Gate | Effective `Kc` | Source | GenericHardwood |
|------|----------------|--------|-----------------|
| Power (`tool_load/power.rs`) | `GRAIN_ANISOTROPY_FACTOR × kc` | `predicted_power_kw`, factor = **2.0** | 2.0 × 13 = **26 N/mm²** |
| Deflection (`tool_load/deflection.rs`) | **raw `kc`** (no factor) | doc §"Modeling assumptions": *"Raw Kc, no grain-anisotropy factor"* | **13 N/mm²** |

- Base `Kc` comes from `Material::kc_n_per_mm2()` (`material.rs`), pinned to
  **USDA FPL Wood Handbook Table 5-3a shear-parallel-to-grain (12% MC)**:
  GenericSoftwood 6.5, RadiataPine 6.0, LongleafPine 10.4, **GenericHardwood 13.0**,
  HardMaple 16.0, Walnut 9.5, Birch 13.0, Ipe 28.0 (N/mm²).
- `material.rs` already flags the gap in its own SCOPE NOTE:
  *"The literature delta between shear-block testing and peripheral milling Kc
  (a 3–5× size-effect multiplier) is NOT applied here … TODO Phase 6+:
  absolute-Kc calibration … needs operator field validation."*

### Literature (peripheral-milling Kc — measured)

| Source | Material | Milling `Kc` (N/mm²) |
|--------|----------|----------------------|
| Sydor et al. (particleboard up-milling, PMC8123317) | 542 kg/m³ | **32.0 (40 m/s) – 37.6 (60 m/s)** |
| Cristóvão / Axelsson | oak, with-grain | ~14 (low-end) |
| FPL Table 5-3a (current base) | hardwoods | 13–16 (shear ∥) |

- Density→energy regression (Andrade 2022, 9 species CNC router):
  `specific_energy = 552 + 408·density [J/cm³]`, R²=0.38 — density explains only
  ~38%, so **species/Janka-driven Kc is reasonable but inherently a band
  (±~15–40% from grain/anatomy), not a point**.
- **TRAP (do not use):** Andrade's 627–930 J/cm³ are *electrical* specific
  energy (motor power ÷ volume) — ~15–25× inflated by motor inefficiency + idle.
  These are NOT mechanical `Kc`. Recorded here so nobody mistakes them for it.

### Implied effect on the wanaka rough

`F ∝ Kc`, `δ ∝ F` ⇒ deflection scales with `Kc`. Lifting deflection's `Kc`
from 13 → ~35 (×2.7) takes the measured 96–153 µm → **~260–410 µm**, i.e. over
the 200 µm `Exceeds` bound. Regime flips machine-limited → **tool-limited**.

## Root cause

Two distinct physical effects are conflated into one factor and mis-scoped:

1. **Shear → milling mean-force (size-effect):** real milling mean `Kc`
   (~32–38) is ~2.7× the shear-parallel value (~13). This applies to **mean
   force** → both gates should see it. Currently applied to **neither** as a
   named factor (power's 2.0 partially absorbs it; deflection gets nothing).
2. **Grain directional spread (peak/mean across orientations):** Pałubicki 2021
   ~1.3–1.5. This is a **worst-case** effect → power-safety only. Currently
   over-stated as 2.0 and doing double duty for (1).

## Proposed model

Split into two honestly-scoped constants, both material-agnostic globals:

```text
Kc_mill   = MILLING_KC_FACTOR × kc_shear           # mean milling force
            MILLING_KC_FACTOR ≈ 2.7  (cited: 32–38 / 13)

Deflection (mean force):   F = Kc_mill × A
Power     (worst case):    P = Kc_mill × GRAIN_SPREAD × A × feed / 60e6
            GRAIN_SPREAD ≈ 1.4  (Pałubicki directional spread, was 2.0)
```

Resulting effective `Kc` (GenericHardwood):

| Gate | now | proposed | Δ |
|------|-----|----------|---|
| Deflection | 13 | **~35** | ×2.7 (the fix) |
| Power | 26 | ~49 | ×1.9 (still far under a 1.5 kW spindle) |

Self-consistency: particleboard milling 32–38 is the (≈isotropic) **mean** →
deflection target. Power worst-case = mean × directional spread ≈ 35×1.4 ≈ 49.

### Why factor-on-shear, not replace the per-species values

Keeps the FPL citations and the material-selector drive intact; one cited
multiplier instead of sourcing scarce milling-`Kc` per species. Density/Janka
ordering across species is preserved. (If per-species milling data later
appears, swap the values and set `MILLING_KC_FACTOR = 1.0` — same architecture.)

## Implementation steps

1. **`material.rs`** — introduce `MILLING_KC_FACTOR` (≈2.7, doc-cited) and a
   `kc_milling_n_per_mm2()` (or fold the factor into the single consumed path).
   Keep `kc_n_per_mm2()` as the raw FPL shear accessor for provenance/tests.
2. **`tool_load/deflection.rs` + `feeds/predict.rs::tip_deflection_from_engagement`**
   — switch the force calc from raw shear `Kc` to `Kc_mill`. (This is the
   regime-deciding change.)
3. **`tool_load/power.rs`** — rename/re-scope `GRAIN_ANISOTROPY_FACTOR` 2.0 →
   `GRAIN_SPREAD` ~1.4, and have `predicted_power_kw` take `Kc_mill` as base
   (net power Kc_eff = Kc_mill × GRAIN_SPREAD).
4. **Sentry test** — assert effective milling `Kc` for a medium hardwood lands
   in the 32–38 N/mm² literature band; assert deflection-base == power-mean-base
   (they can't silently diverge again).
5. **Machine profile (separate, parallel)** — wanaka spindle is a **1.5 kW VFD**,
   currently mis-modelled as `ConstantPower 0.8`. Switch to
   `VfdConstantTorque { rated_power_kw: 1.5, rated_rpm: <confirm, 24000?> }`.
   Independent of the Kc work but needed for the same re-read.

## Blast radius — every `Kc` consumer

Changing the base force ripples beyond the two gates (this is desirable —
consistency — but needs baseline updates + review):

- `tool_load/power.rs`, `tool_load/deflection.rs` — the gates (intended).
- `feeds/predict.rs`, `feeds/mod.rs` — Suggest/predicted feeds derive from `Kc`;
  predicted feeds will drop (higher force ⇒ lower safe feed). **Expected.**
- `feed_modulation.rs` — force-based modulation uses `Kc`; modulated feeds shift.
- `session/compute.rs` — builds `PowerLimitInputs` from `Kc`.
- **Smoke / sentry baselines WILL shift** (material.rs already warned). Plan a
  baseline-update pass and call it out in the commit.
- **Chipload gate** uses the vendor-LUT `kc1.1/mc` band (separate model) — verify
  it's independent and unaffected.

## Validation (no bench required)

1. Unit/sentry: effective milling `Kc` ∈ [32, 38] for a medium hardwood; the
   `heavy_cut_exceeds` / `light_cut_within` power fixtures still pass (re-baselined).
2. Live re-read: wanaka Back Rough, 1.5 kW VFD set, dense-hardwood species →
   read deflection. Expect peak to cross 200 µm (Exceeds → tool-limited).
3. Cross-check power stays Within (49 N/mm² × light cut ≪ ~1.0 kW available@16k).
4. Confirm the regime label now reads **tool-limited** for the wanaka rough.

## Risks / open questions

- **2.7 and 1.4 are literature-anchored, not your-machine-validated.** Gate stays
  "approximate" — a band, not a point. This is honest, but the regime call near
  the 200 µm boundary should carry a margin, not be treated as exact.
- Particleboard ≠ solid dense hardwood; 2.7 is a defensible central estimate
  (range 2.5–5 in the literature). Treat as tunable; the sentry pins the band.
- Re-baselining smoke is real work and will look like a big diff — sequence it
  as its own commit with the rationale.
- Confirm VFD rated rpm (assumed 24 000) and cooling (water = holds rated;
  air = derates) before trusting the absolute power ceiling.

## Out of scope (explicitly not in this change)

- Per-species *milling* Kc sourcing (future; architecture already supports it).
- Chip-thickness size-effect model (Kienzle `h^-mc`) — Sydor found `Kc` ≈ constant
  over 0–0.31 mm, so the rough's 0.19 mm chips don't need it. "Good enough."
- Machine acceleration model (the "machine-limited" half of the regime calc) —
  tracked separately; needed later for the smoothness/violence knob.
- The strategy cull decision — gated on this landing + the re-read.

## Decision needed before coding

- Approve `MILLING_KC_FACTOR ≈ 2.7` and `GRAIN_SPREAD ≈ 1.4` as the starting
  constants (tunable, sentry-pinned)?
- Confirm VFD rated rpm + cooling for the machine-profile change.
- OK to absorb the smoke/sentry baseline shift as part of this?
