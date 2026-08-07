# ARP-1 — the analytic reference plate

Wave: W7 (R6/M3) · Date: 2026-08-05 · Status: **SPEC, research-only. No production change. No strategy claim.**
Parent revision measured: `ebe77de` (branch `experiment/adaptive-spiral`)
Executable spec: `artifacts/w7/arp1_reference.py` · measurements: `artifacts/w7/arp1_measurements.md` · renders: `artifacts/w7/*.png`

This document specifies a procedural reference part. It does **not** re-open
B1/B2, and it makes no statement about any strategy. Per the plan, only
Checkpoint E may rule that a fixture is qualified; this is the evidence that
ruling would be made on.

---

## 0. What this delivers, in one paragraph

ARP-1 is a **procedurally generated part whose surface is closed-form
everywhere**: exact height, exact unit normal, exact principal curvatures,
exact per-slope-band 3D area, and — for the groove and ripple zones — an
**exact tool-reach floor**, i.e. the residual that no algorithm can remove
because the tool physically cannot get there. Nothing is a committed blob; the
mesh is generated from equations with a stated deterministic vertex order. The
tessellation rule is **measured, not assumed**: §4 reports a five-point
step-refinement study per zone with a pre-registered `s²` prediction, which the
data confirms (last-halving ratios 3.93–4.02 against a predicted 4.00).

---

## 1. Why: what the incumbent fixture can and cannot adjudicate

### 1.1 A correction to the record, measured

The programme plan, `RESEARCH_COMMISSION.md`, and
`review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` all state that the terrain
fixture is *"a coarse TIN: 1.8% of triangles carry 40.8% of area."* **That
statistic does not reproduce against `crates/rs_cam_core/tests/fixtures/terrain.stl`.**
Measured directly from the binary STL (`artifacts/w7/` method: exact triangle
areas from the cross product, no library):

| reading | terrain.stl, whole mesh | terrain.stl, upward-facing relief only |
|---|---|---|
| triangles | 214,997 | 214,233 (99.6%) |
| total area | 26,226 mm² | 14,841 mm² (56.6% of total) |
| top **1.8%** of triangles carry | **57.0%** of area | **25.5%** of area |
| 40.8% of area is carried by | **99 triangles = 0.05%** | 16,485 triangles = 7.69% |
| facet edge √area p50 / p99 / max | — | 0.200 / 0.626 / 3.326 mm |

Neither reading is 1.8%/40.8%. The whole-mesh reading is dominated by the
**base box**: the single largest facet has a 70.7 mm edge (the 100×100 mm
bottom face's diagonal), and 43.4% of the mesh's area is not relief at all.
The cited statistic was most likely computed on a different asset (the scaled
wanaka part) and then attributed to `terrain.stl`.

**This does not rehabilitate the fixture.** It relocates the objection, and the
relocated objection is stronger:

### 1.2 The real objection

1. **`terrain.stl` has no ground truth other than itself.** It is a TIN; the
   facets *are* the model. There is no surface behind them to measure
   discretisation error against, so "how much of my residual is the fixture"
   is not a question it can answer at any refinement.
2. **Its facet scale collides with the finishing scale.** Median facet edge is
   **0.200 mm**; p99 is **0.626 mm**. The shipped `scallop_height` default is
   **0.1 mm** (`crates/rs_cam_core/src/scallop.rs:128`), which on a Ø1 ball is
   a stepover of `d = 2√(2Rh − h²) = 0.600 mm` — i.e. **the shipped finishing
   stepover equals this fixture's p99 facet edge to three digits, and is 3× its
   median.** Facet creases and machining cusps are therefore the same size, in
   the same places, and no aggregate can separate them.
3. **A single-density TIN cannot be right everywhere** (§4.4): the step needed
   to hold a fixed sag varies by a factor of ~39 between flat and 85° ground on
   a single R8 sphere. One global density is either wasteful on the flat or
   wrong on the steep. `terrain.stl` has one.

ARP-1 answers 1 and 2 by construction and answers 3 with a per-zone rule.

### 1.3 What ARP-1 does *not* replace

`terrain.stl` stays as **characterization**: it is representative relief and it
shows what strategies *do*. The two-fixture rule stands — any strategy
statement needs an analytic fixture **and** a representative one. ARP-1 is not
representative of a customer part and must never be quoted as if it were.

---

## 2. Design principle: composition by disjoint support

**Zones do not blend.** The part is a 4×4 lattice of 24 mm tiles; each zone
occupies one tile and is separated from its neighbours by a flat datum gutter
wider than any tool envelope in the dial range. Consequences, all load-bearing:

- **Closed form survives composition.** A blend would destroy the exact normal
  and the exact curvature at exactly the interesting places. `z` is the zone's
  own equation inside its support and `0` outside it, full stop.
- **Attribution is exact.** Every scored cell belongs to exactly one zone by an
  analytic predicate — no cluster-and-hope, no dilated band map. This is the
  direct answer to the prior campaign's failure where *"97% of the tail was
  raster-owned flats mislabelled mid-steep by an overlap-dilated band map"*.
- **Zones are independently instantiable.** A fast unit test builds one zone;
  it does not pay for the plate.

**Every zone carries a plan rotation φ.** Grid-aligned features produce
sampling beats against grid-aligned instruments — the prior campaign's
*"stripes = 0.3-vs-0.25 sampling beat"*. ARP-1's default φ values are
deliberately non-zero and non-45° (20°, 30°) so no zone is accidentally
commensurate with a raster or a dexel lattice. A zone may be instantiated at
φ = 0 **only** by a test that is specifically about grid alignment.

---

## 3. Feature catalogue

Layout render: `artifacts/w7/arp1_zone_and_slope_map.png` (height + exact-normal
band map). Cross-sections: `artifacts/w7/arp1_cross_sections.png`.

Local frame `(u,v)`, origin at tile centre, datum plane `z = 0`, material below.

| id | zone | equation | exact truth it owns | mechanism it can exhibit | non-vacuity test |
|---|---|---|---|---|---|
| Z0 | Datum | `z = 0` | trivially exact | null control; flat-ground cusp must equal `R − √(R²−(d/2)²)` | `flat_ground_max_residual_is_the_closed_form_cusp` already exists |
| Z1 | Dome (sphere cap, R, capped 85°) | `z = √(R²−r²) − R cos85°`, `r ≤ R sin85°` | κ₁=κ₂=1/R; **exact band areas**; **exact iso-slope circles** | convex chord-sag gouge; band run-off across a *curved* boundary | the 45° and 75° circles must land at r = 5.656854 and 7.727407 mm for R=8 |
| Z2 | Bowl (inverted cap) | `z = −(√(R²−r²) − R cos85°)` | same, concave | concave reach limit; a convex fixture cannot adjudicate a concave defect (P12) | a ball of ρ > R must report a non-zero floor |
| Z3 | Saddle | `z = (u²−v²)/(2ρ)` | κ = ±1/ρ, **opposite signs** | a scalar mean-curvature or single-direction classifier reads zero here | mean curvature must be 0 and max \|κ\| must be 1/ρ at the same cell |
| Z4 | Cone ladder | `z = −tanθ·(r−r₀)`, annulus | slope **exactly** θ; slant area `π(r₁²−r₀²)/cosθ` | **band run-off**: pairs at 44/46° and 74/76° straddle the shipped 45°/75° dials by ±1° | classifier must put 44° in Shallow and 46° in MidSteep |
| Z5 | U-groove comb (R ∈ 0.2…1.5) | `z = −(√(R²−u²) − R cos85°)` | **exact reach floor** `R + √(ρ²−R²) − ρ` for ρ>R, `0` for ρ≤R | tool-cannot-enter vs algorithm-failed | at the nominal tool ≥1 groove enterable AND ≥1 not |
| Z6 | V-groove comb (α ∈ 15/30/45°) | `z = −(w−\|u\|)·cotα` | **exact floor** `ρ(1−sinα)/sinα`, always > 0 | concave crease; pencil/valley targeting; never-enterable control | floor must be > 0 for every ρ > 0 |
| Z9 | Step terrace | flats at 0,−1,−2,−3 with vertical risers | exact per-level projected area | Z-level/waterline; the "terrace phantom" trap | risers are invisible to a normal-based band map — asserted, not assumed |
| Z10 | Micro-ripple comb (λ ∈ 0.3…2.4, A = λ/10) | `z = A sin(2πu/λ)` | trough radius `λ²/(4π²A)`; κ = A(2π/λ)² | **tool bridges relief finer than its radius** (`meshes.rs`'s documented blind spot) | λ swept so the trough radius crosses ρ: 0.076 → 0.608 mm across ρ=0.5 |

Deliberate property of Z10: **A/λ is fixed, so max slope is 32.14° for every
λ** while the trough radius sweeps 8×. It isolates *curvature* from *slope* —
no other fixture in the repo does.

### 3.1 Exact reach floors (measured, `arp1_measurements.md` §2)

| feature | D1.0 ball (ρ=0.5) | D3.0 ball (ρ=1.5) |
|---|---|---|
| U groove R=0.20 | 158.3 µm | 186.6 µm |
| U groove R=0.35 | 207.1 µm | 308.6 µm |
| U groove R=0.50 | **0 (enterable)** | 414.2 µm |
| U groove R=0.80 | **0** | 568.9 µm |
| U groove R=1.50 | **0** | **0** |
| V groove α=15° (75° flanks) | 1431.9 µm | 4295.6 µm |
| V groove α=30° | 500.0 µm | 1500.0 µm |
| V groove α=45° | 207.1 µm | 621.3 µm |

This table is the fixture's single most valuable output. Every prior quality
campaign in this repo has had to *argue* about how much of a residual was tool
reach; here it is arithmetic, and it changes with the tool in a way the harness
can predict before it runs.

### 3.2 Exact band areas (R=8 cap, `arp1_measurements.md` §1)

| band | closed form `2πR²(cos t₀ − cos t₁)` | numeric integral | rel. err |
|---|---|---|---|
| Shallow 0–45° | 117.7794 mm² | 117.7794 | 1.3e−12 |
| MidSteep 45–75° | 180.2672 mm² | 180.2672 | 5.7e−13 |
| VerySteep 75–85° | 69.0299 mm² | 69.0299 | 6.3e−14 |

A coverage or band-run-off gate can now be written as a **fraction of an exact
denominator**, instead of a count of cells on a TIN.

### 3.3 Three defects the render caught, recorded

Per the standing rule (*never gate on an aggregate without rendering the
surface*), the layout was rendered and read before this section was written.
Three defects in the first draft, all now spec'd out:

1. **A 44° cone is coloured identically to the flat datum in a three-band
   map.** The band-run-off pair — the whole point of Z4a — is *invisible* in
   the figure meant to display it. Fix: gates on Z4 read **per-zone areas**,
   never the band map; the render carries a feature-support outline overlay.
2. **Saddle at ρ=3 over-ran its tile** (±13.5 mm of relief, corners at 76.7°,
   and it dominated the height colour scale). Fixed to half-extent 4.5 mm.
3. **U-groove comb pitch of 3.2 mm leaves only 0.2 mm of flat between the
   R=1.5 lanes** — not enough datum for a Ø6.35 tool to establish a rim.
   Constraint added: `pitch ≥ 2·u_max + envelope_diameter + 2 mm`, which puts
   the five-groove comb on a **double tile at 8 mm pitch**.

---

## 4. The tessellation rule — MEASURED

### 4.1 The rule

For a piecewise-linear interpolant of a surface sampled at chord length `s`,
the sag scales as `s²`. The rule has **two** terms, and the second was found by
measurement, not derived up front:

```
s  ≤  min( 2·√(ε / κ_hf) ,  λ_min / 8 )
```

- **Curvature term.** `κ_hf` is the **height-field** second derivative, not the
  surface principal curvature. They differ, and using the surface value
  under-estimates the required density on slope (§4.4). The `/4` in the bound
  `ε ≤ κ s²/4` is the square cell's diagonal, not its edge.
- **Sampling term.** The `s²` law only holds once the lattice actually resolves
  the feature. On the λ=0.3 ripple the coarsest steps under-resolve the
  wavelength and the log-log fit bends (`slope = 1.805` over the full range,
  vs `3.96` for the final halving). `λ_min/8` is where the fit is clean.

**Adopted safety factor: β = 1/10.** Tessellation error p99 must be ≤ one tenth
of the smallest adopted quality bin. Stated as a pre-registered rule so that a
future bin choice cannot be quietly relaxed to fit a mesh already generated.

### 4.2 Measured (`arp1_measurements.md` §4)

Pre-registered prediction, written before the run: **log-log slope 2.00, and
halving `s` divides p99 error by 4.00.**

| zone | κ (/mm) | p99 at coarsest | p99 at finest | log-log slope | last-halving ratio |
|---|---|---|---|---|---|
| Dome R8 (0–40°) | 0.125 | 175.8 µm @ s=1.285 | 0.663 µm @ s=0.080 | **2.009** | **4.02** |
| Bowl R8 (0–40°) | 0.125 | 175.8 µm | 0.663 µm | **2.009** | **4.02** |
| Saddle ρ8 | 0.125 | 74.7 µm @ s=2.25 | 0.292 µm @ s=0.141 | **2.000** | **4.00** |
| U groove R1.5 | 0.667 | 9.81 µm @ s=0.25 | 0.044 µm @ s=0.0156 | **1.955** | **3.95** |
| U groove R0.35 | 2.857 | 2.98 µm @ s=0.0625 | 0.014 µm @ s=0.0039 | **1.937** | **3.93** |
| Ripple λ1.2 | 3.290 | 223.6 µm @ s=1.0 | 1.59 µm @ s=0.0625 | 1.805 † | **3.96** |
| Ripple λ0.3 | 13.160 | 55.9 µm @ s=0.25 | 0.397 µm @ s=0.0156 | 1.805 † | **3.96** |

† full-range fit contaminated by the under-resolved coarse end; the
last-halving ratio is the clean statement, and it is the reason for the
`λ_min/8` term.

**Verdict: the prediction holds.** Every last-halving ratio is in
[3.93, 4.02] against a predicted 4.00. The rule is measured.

### 4.3 Working step sizes for a 1 µm sag

| zone | κ_hf | step for ε = 1 µm | triangles for a 18 mm scored window |
|---|---|---|---|
| Dome/Bowl R8, ≤ 45° | 0.354 | 0.106 mm | ~58k |
| Saddle ρ8 | 0.125 | 0.179 mm | ~20k |
| U groove R0.35 (across only) | 2.86 | 0.037 mm | anisotropic; ~1D |
| Ripple λ0.3 | 13.2 | 0.017 mm (`λ/8` binds at 0.0375) | 0.0175 mm binds |

Anisotropic sampling is mandatory for the groove and ripple combs: they are
Y-invariant, so the fine step is needed across the feature only. `meshes.rs`'s
`GroovedBlock` already does exactly this and is the precedent.

### 4.4 Why a global XY grid is rejected

A height field's apparent second derivative on a sphere is
`d²z/dr² = R²/(R²−r²)^{3/2}`, which **diverges at the rim** even though the
surface curvature stays 1/R. Measured (`arp1_measurements.md` §5):

| slope θ | height-field d²z/dr² | XY step for 1 µm sag | arc-length step for 1 µm sag |
|---|---|---|---|
| 0° | 0.125 | 0.179 mm | 0.253 mm |
| 45° | 0.354 | 0.106 mm | 0.253 mm |
| 75° | 7.210 | 0.0236 mm | 0.253 mm |
| 85° | 188.81 | **0.0046 mm** | **0.253 mm** |

A uniform XY lattice needs a **39× finer step at 85° than at 0°** — i.e. ~1500×
the triangles per unit area — to hold the same sag. A uniform polar-angle mesh
holds it at constant cost, verified along the meridian at log-log slope
**1.982**, slope-independent from 0° to 85°.

**Therefore: ARP-1 tessellates each zone in its own natural parameter**
(polar angle for caps, wrap angle for grooves, arc length along the
cross-section for ripples), not on a global XY lattice. This is the single
structural difference from every existing fixture in `tests/common/meshes.rs`,
all of which are `height_field_grid` derivatives — and it is precisely the
property `terrain.stl` lacks.

Render: `artifacts/w7/arp1_tessellation_rule.png`.

---

## 5. Ground-truth evaluators

| evaluator | status | note |
|---|---|---|
| `true_surface_analytic(grid, z)` | **EXISTS** — `tests/common/scallop_oracle.rs:339` | takes a closed-form `z`; ARP-1 supplies one |
| `true_surface_from_mesh(grid, mesh, index)` | **EXISTS** — `scallop_oracle.rs:307` | the tessellated reading; differencing the two IS the tessellation measurement |
| `EnvelopeOracle::score` / `report` | **EXISTS** — `scallop_oracle.rs:~360, ~560` | analytic cutter-envelope residual with slope bands |
| `tool_reach_floor` | **EXISTS** — `scallop_oracle.rs:~477` | numeric; ARP-1 gives it a **closed-form** answer to be checked against |
| **`normal_at(x,y)` exact** | **GAP — new** | see §5.1 |
| **`band_area_mm2(zone, band)` exact** | **GAP — new** | closed form per zone, §3.2 |
| **`reach_floor_at(x,y,cutter)` closed form** | **GAP — new** | §3.1 |
| **`zone_at(x,y)` exact attribution** | **GAP — new** | replaces dilated band maps |
| **`curvature_at(x,y)` → (κ₁,κ₂)** | **GAP — new** | drives the tessellation rule |

### 5.1 The exact-normal gap is not cosmetic

`EnvelopeOracle::slope_deg` is a **central difference on the sampled height
grid** (`scallop_oracle.rs:~1028`, `slope_from_heights`), and it is `NaN`
wherever the stencil crosses uncovered ground. Two consequences:

1. Its error grows with slope, exactly where the bands are decided. A cell near
   a 75° boundary is attributed by a finite difference whose own error is
   `O(κ_hf·cell²)` — and §4.4 shows `κ_hf` diverges there.
2. **The VerySteep band is systematically under-populated by the cells that
   matter**, because a stencil at the rim of a steep feature crosses uncovered
   ground and gets `NaN`'d out.

An analytic normal removes both. This is the concrete, mechanical reason the
plan's phrase *"known surface-normal truth"* is a requirement and not a
nicety, and it is a real limitation of the current oracle that this spec is
the first to name.

---

## 6. Procedural API

New module `crates/rs_cam_core/tests/common/reference_plate.rs`, alongside the
existing C6 library. **No production code.**

```rust
pub enum Zone { Datum, Dome, Bowl, Saddle, ConeLadder, UGrooveComb,
                VGrooveComb, StepTerrace, MicroRipple }

pub struct ZoneSpec { pub zone: Zone, pub centre: P2, pub half_extent: f64,
                      pub plan_rotation_deg: f64, /* per-zone params */ }

pub struct ReferencePlate { /* zones, extent, datum_z, depth, tess */ }

impl ReferencePlate {
    pub fn arp1() -> Self;                  // the canonical 96x96 part
    pub fn single(zone: ZoneSpec) -> Self;  // one zone on its own base block
    pub fn with_tess_epsilon(self, mm: f64) -> Self;

    // --- exact, closed form ------------------------------------------------
    pub fn z_at(&self, x: f64, y: f64) -> f64;                 // NaN off-part
    pub fn normal_at(&self, x: f64, y: f64) -> Option<[f64; 3]>;
    pub fn slope_deg_at(&self, x: f64, y: f64) -> Option<f64>;
    pub fn curvature_at(&self, x: f64, y: f64) -> Option<(f64, f64)>;
    pub fn zone_at(&self, x: f64, y: f64) -> Option<Zone>;
    pub fn band_area_mm2(&self, zone: Zone, band: SlopeBand) -> f64;
    pub fn reach_floor_at(&self, x: f64, y: f64, c: &dyn MillingCutter) -> Option<f64>;

    // --- tessellation ------------------------------------------------------
    pub fn tess_step_for(&self, zone: Zone) -> f64;   // the §4.1 rule's output
    pub fn mesh(&self) -> TriangleMesh;               // deterministic order
    pub fn measured_tess_error(&self, zone: Zone) -> TessError; // p50/p99/max
}
```

**Determinism contract**, mirroring `meshes.rs`'s: zones emitted in a fixed
declaration order; within a zone, vertices in natural-parameter outer/inner
order; triangle winding +Z; no HashMap iteration anywhere in the generator; no
floating-point operand reordering without a fingerprint recapture. `meshes.rs`
already carries this contract and its wording is reused verbatim.

**C6-style contract proofs required before use** (per the brief's "bit-identity
/ contract proofs for existing helpers"): `ReferencePlate::single(Datum)` must
produce a mesh bit-identical to `meshes::plateau` at matching parameters, and
the ARP-1 groove zone must reproduce `meshes::GroovedBlock` bit-for-bit at
matching parameters, or the new module must state exactly why it deliberately
differs. W4's `adversarial2d.rs` is untouched.

---

## 7. Tool and dial ranges

| dial | range | why |
|---|---|---|
| ball radius ρ | 0.5, 1.0, 1.5, 3.175 mm | brackets the Z5 comb's R ladder so the reach floor changes sign within the sweep |
| stepover | 0.05–0.60 mm | flat-ground cusp 0.6–100 µm on ρ=0.5; the top of the range is the shipped default |
| scallop dial | 10, 20, 40, **100** µm | 100 µm is the shipped `scallop.rs:128` default and must be in range, or the fixture tests dials nobody ships |
| stock_to_leave | 0.0 and 0.2 mm | 0.0 is the oracle's clean case; 0.2 exercises the D-16.2 shape |
| sim cell | 0.05, 0.10, 0.25 mm | §8; 0.25 is the **prior** campaign's cell, kept only to reproduce its alias |

Tapered and bull-nose cutters are in scope through `height_at_radius` — the
oracle is already written against the profile accessor, not a sphere.

---

## 8. Intended gate classes and fixture tiers

| tier | cost | what runs | example gates |
|---|---|---|---|
| **T1 fast unit geometry** | < 1 s, no simulation | analytic identities on one zone | exact normal vs central difference; band area vs numeric integral; reach floor vs `tool_reach_floor`; `s²` convergence on one zone |
| **T2 medium deterministic integration** | < 60 s | oracle scoring of an analytic raster on 1–2 zones | flat cusp = closed form; Z4 band-run-off non-vacuity; Z5 enterable/not split |
| **T3 ignored characterization** | minutes+ | full plate, real generators, COLUMNS | A/B comparisons — **named owner and cadence required**, never CI |
| **T4 read-only live validation** | operator | generated STL written to `target/`, loaded in GUI/MCP | visual confirmation only; the STL is **never committed** |

**Gate-writing rules this fixture imposes:**

1. A gate on Z4 reads per-zone areas, not a band map (§3.3 defect 1).
2. A residual claim on Z5/Z6/Z10 **must** subtract the closed-form reach floor
   first, or it is measuring the tool, not the algorithm.
3. No gate may use a bin below the §8-qualified minimum for its cell (see
   `REFERENCE_FIXTURE_REPEATABILITY.md`).
4. No fine-quality winner gate may depend on `terrain.stl` (plan acceptance
   gate, unchanged).

---

## 9. What ARP-1 still cannot do

Stated so it is not over-claimed later:

- **No stock history.** The envelope oracle scores cutter-vs-model. Rest
  material, cascades, and prior-op stock need the dexel COLUMNS instrument.
- **No machine dynamics, no cutting forces, no material.** Geometry only.
- **It does not exercise the import path** unless deliberately written to STL,
  and writing it to STL reintroduces float32 quantisation (~1e-7 relative,
  i.e. sub-nanometre at these extents — negligible, but it is a step away from
  exactness and should be measured once if a T4 run ever disagrees with T2).
- **It is not representative.** The two-fixture rule stands.
- **Vertical risers (Z9) are invisible to any normal-based band map.** That is
  asserted as a property, not worked around.

---

## 10. Open decisions for Checkpoint E

Listed in `ORCHESTRATION_LOG.md`; repeated here for the reader of this file.
W7 proposes; it does not decide.

- **E1.** Adopt ARP-1 as the qualified analytic fixture, at the §4.1 rule with
  β = 1/10?
- **E2.** Approve `tests/common/reference_plate.rs` with the §6 determinism
  contract and the C6 bit-identity proofs against `plateau` / `GroovedBlock`?
- **E3.** Correct the "1.8% / 40.8%" citation in the plan,
  `RESEARCH_COMMISSION.md`, and `SUPERSEDED_CONCLUSIONS.md` to the measured
  §1.1 values, with the relocated §1.2 objection?
- **E4.** Do B1/B2 re-open? **W7's recommendation is NO, not yet** — a
  qualified fixture is necessary but not sufficient; the repeatability study's
  runs are NOT RUN (see the companion document), so no bin is yet defensible.
