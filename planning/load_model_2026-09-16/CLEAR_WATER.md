# The load model, as it actually is

Synthesis of three independent research passes run on 2026-09-17:
`CENSUS_PROVENANCE.md`, `SWEEP_DEFECTS.md`, `MAP_LOAD_MODEL.md`. Every claim
below that carries a number was checked against the code by the commissioning
session before it was written down.

---

## 1. The one-line answer

**The load model is a well-built machine running on numbers nobody has
measured.** The physics is sound and internally consistent. The inputs are
almost entirely folklore. And the one number that decides nearly every cut —
the rigidity cap — is the weakest of them.

---

## 2. Provenance: 81 numbers, none measured

| Class | Count |
|---|---|
| MEASURED | **0** |
| LITERATURE | 12 |
| FITTED | 4 |
| DERIVED | 9 |
| FOLKLORE | **53** |
| FABRICATED | 3 |

Not one number on the path from a parameter to a newton or a kilowatt has been
measured on a machine or a workpiece. Six places say "pending bench
validation". The bench never happened.

### The headline overclaim, and how far it actually reaches

`feeds/force.rs` heads its calibration section **"literature-absolute"**. The
census found that the anchor `LIT_ANCHOR_KC_N_PER_MM2 = 35.1` is
`MILLING_KC_FACTOR 2.7 × FPL shear 13.0`, and that 2.7 exists because it
"lands GenericHardwood at ~35 N/mm² (particleboard parity)". Its only citation
is a deleted document. `CREDITS.md` still says the lift needs bench validation.

So the census concluded the absolute magnitude of every force and power number
rests on an unstated premise: *a mid hardwood cuts like particleboard*.

**Checked, and it does not reach that far.** The factor appears in both the
numerator and the anchor:

```
Ks_species = LIT_KS × (shear_species × 2.7) / (13.0 × 2.7)
           = LIT_KS ×  shear_species / 13.0
```

**It cancels.** Verified numerically across softwood, the hardwood anchor and
ipe, at factors of 1.0, 2.7 and 5.0 — `Ks` does not move. And every consumer
of `Material::kc_n_per_mm2()` on a load path either gates on `is_none()` or
routes through `affine_coefficients_for_kc`, where the cancellation happens.
**`MILLING_KC_FACTOR` is inert on every load path in the tree.**

What the force magnitudes actually rest on:

- `LIT_KS = 49.95` and `LIT_FEDGE = 5.30`, the woodresearch.sk quasi-orthogonal
  fit at R² ≈ 0.99 — a real fit to real data.
- The RATIO of FPL Table 5-3a shear values between species — published.

Both literature. Independently corroborated earlier in this programme against
two further wood datasets, within about 10 % once the species scaling is
applied.

**The census's finding survives in a narrower and still useful form: the
comment overclaims.** "Literature-absolute" is doing work the citation cannot
support, the anchor's species is never named, and the factor's own citation is
dead. A reader would reasonably conclude the numbers are shakier than they
are. The prose needs correcting; the physics does not.

### The overclaim that does bite

`GRAIN_ANISOTROPY_FACTOR = 2.0` calls itself "a documented physical factor,
not a knob" four lines after recording that it moved from 2.5 to 2.0 to hold
the product `Kc × factor` steady. **It doubles every power number**, and it
does not cancel anywhere.

### Highest value per hour, if measured

`ENDMILL_CORE_FRACTION = 0.7`. Deflection goes as the inverse fourth power, so
a 10 % error in this one constant is a 46 % error in every deflection number,
and the back-off loop now depends on its known +36 % bias. One afternoon with
a dial indicator and a known weight.

---

## 3. The defects, ranked

Nine confirmed, three in each of the classes this repository keeps
rediscovering.

**Worst, and it is a safety verdict:** `rs_cam_cli::project` writes
`collision_count: 0, status: "ok"` when a collision check **failed**. The error
arm logs a warning and inserts nothing; both readers do `.unwrap_or(0)`. A
clean bill of health asserted with no evidence, in the CLI's only
machine-readable artifact. The identical shape was found and fixed twice on
the CLI's own CSV reader — on the consumer, never on the producer.

The rest, by class:

**A number computed at one state, consumed at another**
- `cut_efficiency` builds one struct from two operating points and the UI
  prints them on one line. Gap up to 3.5× on the shipped fixture.
- `realised_step_down` runs BEFORE `enforce_invariants`, so the T-12 guard
  this programme shipped is bypassed whenever a later clamp fires.
- `mrr_mm3_min` is the unnamed sibling of the known-open stale `power_kw`.

**A quantity multiplied by a fraction of a different quantity**
- The GUI nomogram's `preview_power_kw` scales the affine model linearly by
  feed, ignores the explored RPM, and divides by rated power rather than the
  gate ceiling. Three errors on one live slider.
- The optimizer's power advice caps stepover by a power ratio, but the edge
  term goes as `√ae`, so the advice under-corrects and the operator re-runs
  and fails again.

**An absence rendering as a reading**
- `predict_peak_deflection_um` still returns `0.0` on refusal, so the
  deflection back-off silently does nothing and says nothing.
- `feeds::calculate` reports `power_kw = 0.0` for a material with no `Kc` —
  indistinguishable from a cut that needs no power.

---

## 4. The ordering hazard

`geometry_feed_factor` ignores its `ae` argument and returns the depth-tier
multiplier alone. That multiplier RISES as depth falls, 0.45 up to 1.00. So a
clamp that lowers depth across a tier boundary makes pass 9 **raise** the feed
by up to 2.22× — and pass 9 does not re-check the power ceiling.

Its doc says why, and cites the same 23.6 % peak utilisation that justified
deleting the power bar from the Feeds card. That figure predates R1 and is
stale: re-measured, the spread is median 17.8 %, p90 89.4 %, peak 100 %. The
doc's own stated failure condition is now met.

Logged as T-15, with the correction that it is reachable by hand today —
`depth_per_pass` is user-editable, and adaptive operations already run
`adaptive_doc_factor` at 1.5 to 2.0.

---

## 5. What this changes

**Nothing about the physics.** The force model is corroborated, the power
model is correct, the ladder works, and the helix question is closed.

**Everything about confidence in the inputs.** A model whose every constant is
folklore can still be internally perfect and externally wrong by an unknown
factor. That is the honest position, and it is not currently what the software
tells an operator.

**It also reorders the work.** Before this pass the plan was a UI surface.
After it:

1. The CLI collision defect is a safety verdict asserted without evidence. It
   outranks everything else here.
2. `ENDMILL_CORE_FRACTION`, the gantry thrust and the gantry stiffness are all
   measurable in one afternoon, and between them they move the deflection
   model, the depth cap and the unmodelled fifth limit.
3. The surface work stands as planned, and is worth more once at least one
   number on it is measured.

**One caution this package now carries throughout.** Several findings rest on
sweeps over the shipped presets. A sweep that does not fire is evidence about
the sweep. The 23.6 % figure justified two decisions for months, was honestly
measured, and stopped being true when one model changed underneath it. Every
"we measured and it did not fire" in these documents should be read with the
sample stated.
