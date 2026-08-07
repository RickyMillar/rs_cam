//! ARP-1 — the analytic reference plate.
//!
//! Adopted at **Checkpoint E, 2026-08-05** (ruling Q1 / asks E1+E2). The
//! specification this implements is
//! `planning/review_2026-08-04/REFERENCE_FIXTURE_SPEC.md`; its executable
//! reference is `planning/review_2026-08-04/artifacts/w7/arp1_reference.py` and
//! its measurements are `.../arp1_measurements.md`. Every closed form below is
//! checked against those numbers by `tests/reference_plate_contract.rs`.
//!
//! # What this is for
//!
//! A **procedurally generated part whose surface is closed-form everywhere**:
//! exact height, exact unit normal, exact principal curvatures, exact
//! per-slope-band area, and — for the groove zones — an **exact tool-reach
//! floor**, i.e. the residual no algorithm can remove because the tool
//! physically cannot get there.
//!
//! It exists because `tests/fixtures/terrain.stl` cannot adjudicate a quality
//! question. terrain.stl is a TIN: the facets *are* the model, so there is no
//! surface behind them to measure discretisation error against; and its p99
//! facet edge (0.626 mm) matches the shipped `scallop_height` default's
//! stepover on a Ø1 ball (0.600 mm) to three digits, so facet creases and
//! machining cusps are the same size in the same places and no aggregate can
//! separate them. See the dated erratum in `REFERENCE_FIXTURE_SPEC.md` §1.
//!
//! **terrain.stl is not replaced.** It stays as *characterization* — it is
//! representative relief and it shows what strategies do. ARP-1 is **not**
//! representative of a customer part and must never be quoted as if it were.
//! The two-fixture rule stands: a strategy statement needs an analytic fixture
//! **and** a representative one.
//!
//! # Design principle: composition by disjoint support
//!
//! **Zones do not blend.** The part is a 4×4 lattice of 24 mm tiles; each zone
//! occupies one tile (two, for the combs) and is separated from its neighbours
//! by a flat datum gutter wider than any tool envelope in the dial range.
//! `z` is the zone's own equation inside its support and exactly `0` outside
//! it. Consequences, all load-bearing:
//!
//! * **Closed form survives composition.** A blend would destroy the exact
//!   normal and the exact curvature at exactly the interesting places.
//! * **Attribution is exact.** Every scored cell belongs to exactly one zone
//!   by an analytic predicate ([`ReferencePlate::zone_at`]) — no cluster-and-
//!   hope, no dilated band map. This is the direct answer to the prior
//!   campaign's failure where *"97% of the tail was raster-owned flats
//!   mislabelled mid-steep by an overlap-dilated band map"*.
//! * **Zones are independently instantiable.** [`ReferencePlate::single`]
//!   builds one zone on its own base tile; a fast unit test does not pay for
//!   the plate.
//!
//! **Every zone carries a plan rotation φ.** Grid-aligned features produce
//! sampling beats against grid-aligned instruments — the prior campaign's
//! *"stripes = 0.3-vs-0.25 sampling beat"*. The [`ReferencePlate::arp1`]
//! rotations are deliberately non-zero and non-45° so no zone is accidentally
//! commensurate with a raster or a dexel lattice. Instantiate a zone at φ = 0
//! **only** from a test that is specifically about grid alignment.
//!
//! # The tessellation rule (measured, not assumed)
//!
//! ```text
//! s  ≤  min( 2·√(ε / κ_hf) ,  λ_min / 8 )
//! ```
//!
//! with **β = 1/10**: tessellation error p99 must be ≤ one tenth of the
//! smallest adopted quality bin. The rule has two terms and the second was
//! found by measurement: the `s²` law only holds once the lattice actually
//! resolves the feature, and on the λ = 0.3 ripple the coarse end
//! under-resolves the wavelength and the log-log fit bends. `λ_min/8` is where
//! the fit is clean. Spec §4.2 reports last-halving ratios of 3.93–4.02
//! against a predicted 4.00 across seven zones.
//!
//! **`κ_hf` is the HEIGHT-FIELD second derivative, not the surface principal
//! curvature.** They differ, and using the surface value under-estimates the
//! required density on slope. On a sphere `d²z/dr² = R²/(R²−r²)^{3/2}`
//! **diverges at the rim** while the surface curvature stays `1/R`: a uniform
//! XY lattice needs a 39× finer step at 85° than at 0° to hold the same sag,
//! i.e. ~1500× the triangles per unit area. A uniform **polar-angle** mesh
//! holds it at constant cost.
//!
//! **Therefore ARP-1 tessellates each zone in its own natural parameter** —
//! polar angle for caps, wrap angle for grooves, and the across-feature
//! direction only for the Y-invariant combs and ripples. This is the single
//! structural difference from every existing fixture in [`super::meshes`], all
//! of which are `height_field_grid` derivatives, and it is precisely the
//! property terrain.stl lacks. See [`ReferencePlate::tess_step_for`], which
//! reports both the arc bound and the XY bound so the difference is legible.
//!
//! # Determinism contract
//!
//! Mirrors [`super::meshes`]'s, and for the same reason — consumers pin
//! fingerprints over these vertices:
//!
//! * zones emitted in a fixed declaration order ([`ReferencePlate::zones`]);
//! * within a zone, vertices in natural-parameter outer/inner order;
//! * triangle winding +Z;
//! * **no `HashMap` iteration anywhere in the generator**;
//! * no floating-point operand reordering without a fingerprint recapture.
//!
//! # C6 relationship to the existing fixture library — stated, not assumed
//!
//! The spec asked for bit-identity against [`super::meshes::plateau`] and
//! [`super::meshes::GroovedBlock`], *"or the new module must state exactly why
//! it deliberately differs."* It differs, on both counts, and here is exactly
//! why:
//!
//! * **vs `plateau`** — `plateau` is a **closed solid**: 8 vertices, 12
//!   triangles, top face + vertical walls + bottom face. ARP-1's mesh is an
//!   **open top surface** (see [`ReferencePlate::mesh`]). They cannot be
//!   bit-identical because they are not the same object. What *is* asserted
//!   instead, and is the stronger property: `plateau`'s output is **unchanged
//!   by this wave**, pinned bit-exactly through `f64::to_bits` by
//!   `reference_plate_contract::c6_donor_bodies_are_bit_identical`.
//! * **vs `GroovedBlock`** — `GroovedBlock` is a **trapezoidal** groove (flat
//!   floor, straight inclined walls) sampled on a dense/coarse **XY lattice**.
//!   ARP-1's [`Zone::UGrooveComb`] is a **circular** groove sampled uniformly
//!   in **wrap angle**. Two deliberate differences, not one: different
//!   surface, and different parametrisation — and the parametrisation
//!   difference is the entire point of §4.4. `GroovedBlock` is likewise pinned
//!   unchanged by the same C6 test.
//!
//! **Nothing in [`super`] was modified to add this module** beyond the
//! `pub mod` declaration and its row in the module table.
//!
//! # What ARP-1 still cannot do
//!
//! Stated so it is not over-claimed later:
//!
//! * **No stock history.** The envelope oracle scores cutter-vs-model. Rest
//!   material, cascades and prior-op stock need the dexel COLUMNS instrument.
//! * **No machine dynamics, no cutting forces, no material.** Geometry only.
//! * **It does not exercise the import path** unless deliberately written to
//!   STL, and writing it to STL reintroduces float32 quantisation (~1e-7
//!   relative, sub-nanometre at these extents — negligible, but it is a step
//!   away from exactness).
//! * **It is not representative.**
//! * **Vertical risers ([`Zone::StepTerrace`]) are invisible to any
//!   normal-based band map.** That is asserted as a property, not worked
//!   around.
//! * **The mesh is an open surface, not a solid.** Consumers that need a
//!   closed volume must add their own walls and floor. This follows
//!   [`super::meshes::grooved_block`]'s precedent, not `plateau`'s.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::f64::consts::PI;

use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::tool::MillingCutter;

use super::scallop_oracle::SlopeBand;

// ===========================================================================
// Zone identity
// ===========================================================================

/// The zone taxonomy — the discriminant [`ReferencePlate::zone_at`] returns.
///
/// Parameters live on [`ZoneParams`]; this is the attribution label, so a gate
/// can say "these cells are Dome" without caring which dome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Zone {
    Datum,
    Dome,
    Bowl,
    Saddle,
    ConeLadder,
    UGrooveComb,
    VGrooveComb,
    StepTerrace,
    MicroRipple,
}

impl Zone {
    pub const ALL: [Self; 9] = [
        Self::Datum,
        Self::Dome,
        Self::Bowl,
        Self::Saddle,
        Self::ConeLadder,
        Self::UGrooveComb,
        Self::VGrooveComb,
        Self::StepTerrace,
        Self::MicroRipple,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Datum => "Z0 datum",
            Self::Dome => "Z1 dome",
            Self::Bowl => "Z2 bowl",
            Self::Saddle => "Z3 saddle",
            Self::ConeLadder => "Z4 cone ladder",
            Self::UGrooveComb => "Z5 U-groove comb",
            Self::VGrooveComb => "Z6 V-groove comb",
            Self::StepTerrace => "Z9 step terrace",
            Self::MicroRipple => "Z10 micro-ripple",
        }
    }
}

/// A zone's closed-form parameters.
///
/// Every variant's equation is written in the zone's **local** frame: origin at
/// the tile centre, datum plane `z = 0`, material below, `u` across the feature
/// and `v` along it for the Y-invariant zones.
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneParams {
    /// `z = 0`. The null control: a flat-ground cusp here must equal the
    /// closed form `R − √(R² − (d/2)²)`.
    Datum,
    /// Spherical cap. `sign = +1` is a boss (Z1 dome), `−1` a dimple (Z2
    /// bowl). Capped at `theta_max_deg` so the rim slope is bounded.
    ///
    /// `z = sign · (√(R²−r²) − R cos θ_max)` for `r ≤ R sin θ_max`.
    ///
    /// Both principal curvatures are exactly `1/R`; `sin θ = r/R`, so the 45°
    /// iso-slope circle is at `r = R/√2` and the 75° circle at `R sin 75°`,
    /// **exactly**.
    SphereCap {
        radius_mm: f64,
        sign: f64,
        theta_max_deg: f64,
    },
    /// `z = (u² − v²) / (2ρ)`. Principal curvatures `±1/ρ` — **opposite
    /// signs**, so a scalar mean-curvature or single-direction classifier
    /// reads zero here. That is the mechanism this zone claims.
    Saddle { rho_mm: f64 },
    /// Right circular cone frustum, `z = −tan θ · (r − r₀)` on the annulus
    /// `r₀ ≤ r ≤ r₁`. Slope is **exactly** θ everywhere and the slant area is
    /// `π(r₁²−r₀²)/cos θ`.
    ///
    /// Instantiated in pairs straddling a shipped band boundary (44/46° and
    /// 74/76°) so band run-off is exhibited by construction.
    ConeAnnulus {
        theta_deg: f64,
        r0_mm: f64,
        r1_mm: f64,
    },
    /// A comb of straight circular grooves of radii `radii_mm`, axis along
    /// `+v`, at `pitch_mm` spacing. Owns the **exact reach floor**
    /// `R + √(ρ²−R²) − ρ` for `ρ > R`, and `0` for `ρ ≤ R` — so the floor
    /// changes sign within the dial sweep.
    UGrooveComb {
        radii_mm: Vec<f64>,
        pitch_mm: f64,
        phi_max_deg: f64,
    },
    /// A comb of symmetric V grooves, half-angle `α` from the **vertical**
    /// axis, so the flank slope is `90° − α`.
    ///
    /// `z = −(w − |u|) · cot α`. Owns the exact floor `ρ(1 − sin α)/sin α`,
    /// which is **always positive**: a V groove is never fully enterable,
    /// which is what makes it the honest "this residual is geometry, not
    /// algorithm" control.
    VGrooveComb {
        alphas_deg: Vec<f64>,
        half_width_mm: f64,
        pitch_mm: f64,
    },
    /// Flat terraces with **vertical** risers — the exact Z-level / waterline
    /// fixture, and the "terrace phantom" trap. Per-level projected area is
    /// exact; the risers are invisible to a normal-based band map.
    StepTerrace { levels: usize, drop_mm: f64 },
    /// `z = A sin(2πu/λ)` with `A = amp_ratio · λ`.
    ///
    /// Because `A/λ` is fixed, **max slope is the same for every λ** (32.14°
    /// at the default ratio 1/10) while the trough radius `λ²/(4π²A)` sweeps
    /// with λ. It therefore isolates **curvature** from **slope** — no other
    /// fixture in this repo does. The exact bridging threshold is
    /// `ρ > λ²/(4π²A)`.
    MicroRipple { lambda_mm: f64, amp_ratio: f64 },
}

impl ZoneParams {
    #[must_use]
    pub fn zone(&self) -> Zone {
        match self {
            Self::Datum => Zone::Datum,
            Self::SphereCap { sign, .. } => {
                if *sign >= 0.0 {
                    Zone::Dome
                } else {
                    Zone::Bowl
                }
            }
            Self::Saddle { .. } => Zone::Saddle,
            Self::ConeAnnulus { .. } => Zone::ConeLadder,
            Self::UGrooveComb { .. } => Zone::UGrooveComb,
            Self::VGrooveComb { .. } => Zone::VGrooveComb,
            Self::StepTerrace { .. } => Zone::StepTerrace,
            Self::MicroRipple { .. } => Zone::MicroRipple,
        }
    }
}

/// One zone placed on the plate.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneSpec {
    pub params: ZoneParams,
    /// Tile centre in world XY.
    pub centre: P2,
    /// Half-extent of the tile this zone owns, in mm. The zone's support is
    /// strictly inside it, so tiles butt together on flat datum.
    pub half_extent: f64,
    /// Plan rotation, degrees CCW. See the module doc: deliberately non-zero
    /// and non-45° on the canonical plate.
    pub plan_rotation_deg: f64,
}

impl ZoneSpec {
    #[must_use]
    pub fn new(params: ZoneParams, centre: P2, half_extent: f64, plan_rotation_deg: f64) -> Self {
        Self {
            params,
            centre,
            half_extent,
            plan_rotation_deg,
        }
    }

    #[must_use]
    pub fn zone(&self) -> Zone {
        self.params.zone()
    }

    /// **Tile membership: the tile is AXIS-ALIGNED; only the feature rotates.**
    ///
    /// This is the load-bearing half of the disjointness property, and getting
    /// it the other way round is a real defect that ARP-1's own contract test
    /// caught before this module was ever committed: a *rotated* 24 mm tile has
    /// a 30.8 mm axis-aligned bounding box at φ = 20°, so rotated tiles on a
    /// 24 mm pitch **overlap their neighbours** and two zones claim the same
    /// ground.
    ///
    /// Rotating the feature inside a fixed tile delivers exactly what §2 asks
    /// for — no zone commensurate with a raster or dexel lattice — while
    /// keeping supports provably disjoint. The comb lanes simply run diagonally
    /// across their tile and are clipped by it; [`ReferencePlate::band_area_mm2`]
    /// accounts for that with an exact chord clip rather than assuming a lane
    /// spans the full tile.
    #[must_use]
    pub fn owns(&self, x: f64, y: f64) -> bool {
        if matches!(self.params, ZoneParams::Datum) {
            return false;
        }
        let dx = x - self.centre.x;
        let dy = y - self.centre.y;
        if dx.abs() > self.half_extent || dy.abs() > self.half_extent {
            return false;
        }
        let (u, v) = self.to_local(x, y);
        self.params.supports(u, v)
    }

    /// Half-span the LOCAL sampling box needs to cover this tile's
    /// axis-aligned square once the feature frame is rotated by φ.
    #[must_use]
    fn local_cover_half(&self) -> f64 {
        let (s, c) = self.plan_rotation_deg.to_radians().sin_cos();
        self.half_extent * (c.abs() + s.abs())
    }

    /// World → local. Translate to the tile centre, then rotate by `−φ`.
    #[must_use]
    fn to_local(&self, x: f64, y: f64) -> (f64, f64) {
        let dx = x - self.centre.x;
        let dy = y - self.centre.y;
        let (s, c) = self.plan_rotation_deg.to_radians().sin_cos();
        (dx * c + dy * s, -dx * s + dy * c)
    }

    /// Local → world. Rotate by `+φ`, then translate.
    #[must_use]
    fn to_world(&self, u: f64, v: f64) -> (f64, f64) {
        let (s, c) = self.plan_rotation_deg.to_radians().sin_cos();
        (u * c - v * s + self.centre.x, u * s + v * c + self.centre.y)
    }

    /// Rotate a local planar vector into world. Used for the normal's XY part.
    #[must_use]
    fn vec_to_world(&self, nu: f64, nv: f64) -> (f64, f64) {
        let (s, c) = self.plan_rotation_deg.to_radians().sin_cos();
        (nu * c - nv * s, nu * s + nv * c)
    }
}

// ===========================================================================
// Closed-form surface evaluation, in the LOCAL frame
// ===========================================================================

/// The exact local readings at a point: height, first derivatives, second
/// derivatives. Everything else is derived from these by closed formulae, so
/// there is exactly one place per zone where the calculus lives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalJet {
    pub z: f64,
    pub zu: f64,
    pub zv: f64,
    pub zuu: f64,
    pub zuv: f64,
    pub zvv: f64,
}

impl LocalJet {
    const FLAT: Self = Self {
        z: 0.0,
        zu: 0.0,
        zv: 0.0,
        zuu: 0.0,
        zuv: 0.0,
        zvv: 0.0,
    };
}

impl ZoneParams {
    /// Is `(u, v)` inside this zone's own support? Outside it the zone
    /// contributes exactly `z = 0` (datum) — no blend, ever.
    #[must_use]
    pub fn supports(&self, u: f64, v: f64) -> bool {
        match self {
            Self::Datum => true,
            Self::SphereCap {
                radius_mm,
                theta_max_deg,
                ..
            } => u.hypot(v) <= radius_mm * theta_max_deg.to_radians().sin(),
            Self::Saddle { .. } | Self::StepTerrace { .. } | Self::MicroRipple { .. } => true,
            Self::ConeAnnulus { r0_mm, r1_mm, .. } => {
                let r = u.hypot(v);
                r >= *r0_mm && r <= *r1_mm
            }
            Self::UGrooveComb {
                radii_mm,
                pitch_mm,
                phi_max_deg,
            } => {
                let sin_phi = phi_max_deg.to_radians().sin();
                comb_lane(u, radii_mm.len(), *pitch_mm)
                    .is_some_and(|(k, du)| du.abs() <= radii_mm[k] * sin_phi)
            }
            Self::VGrooveComb {
                alphas_deg,
                half_width_mm,
                pitch_mm,
            } => comb_lane(u, alphas_deg.len(), *pitch_mm)
                .is_some_and(|(_, du)| du.abs() <= *half_width_mm),
        }
    }

    /// The exact 2-jet at a point in the local frame.
    ///
    /// Returns [`LocalJet::FLAT`] outside the support. `None` where the
    /// surface has no tangent plane at all — a crease or a vertical riser —
    /// so callers cannot silently read a normal that does not exist.
    #[must_use]
    pub fn jet(&self, u: f64, v: f64) -> Option<LocalJet> {
        if !self.supports(u, v) {
            return Some(LocalJet::FLAT);
        }
        match self {
            Self::Datum => Some(LocalJet::FLAT),

            Self::SphereCap {
                radius_mm,
                sign,
                theta_max_deg,
            } => {
                let r2 = u * u + v * v;
                let q2 = radius_mm * radius_mm - r2;
                if q2 <= 0.0 {
                    return None; // the 90° rim; the cap is capped short of it
                }
                let q = q2.sqrt();
                let z_rim = radius_mm * theta_max_deg.to_radians().cos();
                let q3 = q2 * q;
                Some(LocalJet {
                    z: sign * (q - z_rim),
                    zu: -sign * u / q,
                    zv: -sign * v / q,
                    zuu: -sign * (1.0 / q + u * u / q3),
                    zuv: -sign * (u * v / q3),
                    zvv: -sign * (1.0 / q + v * v / q3),
                })
            }

            Self::Saddle { rho_mm } => Some(LocalJet {
                z: (u * u - v * v) / (2.0 * rho_mm),
                zu: u / rho_mm,
                zv: -v / rho_mm,
                zuu: 1.0 / rho_mm,
                zuv: 0.0,
                zvv: -1.0 / rho_mm,
            }),

            Self::ConeAnnulus {
                theta_deg,
                r0_mm,
                r1_mm: _,
            } => {
                let r = u.hypot(v);
                if r <= 1e-12 {
                    return None; // the apex, if an annulus ever reached it
                }
                let t = theta_deg.to_radians().tan();
                let r3 = r * r * r;
                Some(LocalJet {
                    z: -t * (r - r0_mm),
                    zu: -t * u / r,
                    zv: -t * v / r,
                    // d/du (u/r) = 1/r − u²/r³ = v²/r³; symmetric in v.
                    zuu: -t * (v * v / r3),
                    zuv: t * (u * v / r3),
                    zvv: -t * (u * u / r3),
                })
            }

            Self::UGrooveComb {
                radii_mm,
                pitch_mm,
                phi_max_deg,
            } => {
                let (k, du) = comb_lane(u, radii_mm.len(), *pitch_mm)?;
                let radius = radii_mm[k];
                let p2 = radius * radius - du * du;
                if p2 <= 0.0 {
                    return None;
                }
                let p = p2.sqrt();
                let z_rim = radius * phi_max_deg.to_radians().cos();
                Some(LocalJet {
                    z: -(p - z_rim),
                    zu: du / p,
                    zv: 0.0,
                    zuu: 1.0 / p + du * du / (p2 * p),
                    zuv: 0.0,
                    zvv: 0.0,
                })
            }

            Self::VGrooveComb {
                alphas_deg,
                half_width_mm,
                pitch_mm,
            } => {
                let (k, du) = comb_lane(u, alphas_deg.len(), *pitch_mm)?;
                if du.abs() < 1e-12 {
                    return None; // the apex crease: no tangent plane
                }
                let cot = 1.0 / alphas_deg[k].to_radians().tan();
                Some(LocalJet {
                    z: -(half_width_mm - du.abs()) * cot,
                    zu: du.signum() * cot,
                    zv: 0.0,
                    zuu: 0.0,
                    zuv: 0.0,
                    zvv: 0.0,
                })
            }

            Self::StepTerrace { levels, drop_mm } => {
                // Level boundaries are vertical risers: no tangent plane.
                let span = 2.0 * TERRACE_HALF_EXTENT;
                let width = span / *levels as f64;
                let t = (u + TERRACE_HALF_EXTENT) / width;
                if (t - t.round()).abs() < 1e-9 && t > 0.5 && t < *levels as f64 - 0.5 {
                    return None;
                }
                let k = t.floor().clamp(0.0, *levels as f64 - 1.0);
                Some(LocalJet {
                    z: -drop_mm * k,
                    ..LocalJet::FLAT
                })
            }

            Self::MicroRipple {
                lambda_mm,
                amp_ratio,
            } => {
                let amp = lambda_mm * amp_ratio;
                let w = 2.0 * PI / lambda_mm;
                let phase = w * u;
                Some(LocalJet {
                    z: amp * phase.sin(),
                    zu: amp * w * phase.cos(),
                    zv: 0.0,
                    zuu: -amp * w * w * phase.sin(),
                    zuv: 0.0,
                    zvv: 0.0,
                })
            }
        }
    }
}

/// `StepTerrace`'s local half-extent, fixed so the level arithmetic is a pure
/// function of `(levels, drop)` and does not need the tile passed in.
const TERRACE_HALF_EXTENT: f64 = 9.0;

/// Which comb lane does `u` fall in, and where within it?
///
/// Lanes are centred at `(k − (n−1)/2) · pitch` for `k` in `0..n`, so the comb
/// is symmetric about `u = 0` for any lane count. Returns `None` past the last
/// lane's half-pitch.
#[must_use]
fn comb_lane(u: f64, lanes: usize, pitch: f64) -> Option<(usize, f64)> {
    if lanes == 0 {
        return None;
    }
    let first = -0.5 * (lanes as f64 - 1.0) * pitch;
    let t = (u - first) / pitch;
    if t < -0.5 || t > lanes as f64 - 0.5 {
        return None;
    }
    let k = (t.round() as usize).min(lanes - 1);
    Some((k, u - (first + k as f64 * pitch)))
}

/// The local `u` a comb lane is centred on.
#[must_use]
fn lane_centre(k: usize, lanes: usize, pitch: f64) -> f64 {
    -0.5 * (lanes as f64 - 1.0) * pitch + k as f64 * pitch
}

/// **Exact** length of the comb lane at local `u = c` after clipping to the
/// tile's axis-aligned square of half-side `half`.
///
/// The lane is the world line `centre + c·(cos φ, sin φ) + t·(−sin φ, cos φ)`.
/// Liang–Barsky slab clip against `|x| ≤ half`, `|y| ≤ half`; the answer is the
/// surviving `t` interval, which is the chord length because the direction
/// vector is a unit vector. `0.0` if the lane misses the tile entirely.
#[must_use]
fn lane_chord_mm(c: f64, plan_rotation_deg: f64, half: f64) -> f64 {
    let (sn, cs) = plan_rotation_deg.to_radians().sin_cos();
    // P(t) = (c·cs − t·sn, c·sn + t·cs)
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;
    for (origin, dir) in [(c * cs, -sn), (c * sn, cs)] {
        if dir.abs() < 1e-15 {
            if origin.abs() > half {
                return 0.0; // parallel to this slab and outside it
            }
            continue;
        }
        let a = (-half - origin) / dir;
        let b = (half - origin) / dir;
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        t0 = t0.max(lo);
        t1 = t1.min(hi);
    }
    (t1 - t0).max(0.0)
}

/// Principal curvatures of the graph `z = f(u,v)` from its 2-jet, by the
/// standard Monge formulae — **exact**, no finite differences anywhere.
///
/// # Sign convention
///
/// Returned **negated** relative to the textbook upward-normal convention, so
/// that **positive means convex material** (a dome reads `+1/R`, a bowl
/// `−1/R`). Stated because the sign is load-bearing for the saddle's claim and
/// a reader who assumes the other convention will read the saddle correctly
/// and the dome backwards.
///
/// Returns `(κ_max, κ_min)` ordered by value, so `κ_max ≥ κ_min` always.
#[must_use]
pub fn principal_curvatures(j: &LocalJet) -> (f64, f64) {
    let e = 1.0 + j.zu * j.zu;
    let f = j.zu * j.zv;
    let g = 1.0 + j.zv * j.zv;
    let w = (1.0 + j.zu * j.zu + j.zv * j.zv).sqrt();
    let l = j.zuu / w;
    let m = j.zuv / w;
    let n = j.zvv / w;
    let denom = e * g - f * f;
    if denom.abs() < 1e-300 {
        return (0.0, 0.0);
    }
    let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * denom);
    let gauss = (l * n - m * m) / denom;
    let disc = (mean * mean - gauss).max(0.0).sqrt();
    // Negate for the "positive = convex material" convention documented above.
    let a = -(mean + disc);
    let b = -(mean - disc);
    if a >= b { (a, b) } else { (b, a) }
}

// ===========================================================================
// The plate
// ===========================================================================

/// Tessellation dials. `epsilon_mm` is the sag target the §4.1 rule solves for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tessellation {
    /// Target piecewise-linear sag, mm. The repeatability study's qualified
    /// value is **1 µm**; spec §4.2 measures p99 ≤ 0.7 µm on every zone at the
    /// step this yields, two orders below the alias floor at any cell anyone
    /// would run — so the mesh is not the binding limit on bin width.
    pub epsilon_mm: f64,
    /// Along-feature step for the Y-invariant zones (grooves, ripples,
    /// terraces). Nothing varies along `v` there, so this is deliberately
    /// coarse — the [`super::meshes::GroovedBlock`] precedent.
    pub along_step_mm: f64,
}

impl Default for Tessellation {
    fn default() -> Self {
        Self {
            epsilon_mm: 0.001,
            along_step_mm: 1.0,
        }
    }
}

/// The step the §4.1 rule yields for one zone, with both terms exposed so a
/// reader can see which one bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessStep {
    /// `√(8ε/κ)` — the edge bound, in **surface chord** length. What a
    /// natural-parameter mesh uses.
    pub arc_mm: f64,
    /// `2√(ε/κ_hf)` — the diagonal bound against the **height-field** second
    /// derivative. What a uniform XY lattice would need.
    pub xy_mm: f64,
    /// `λ_min / 8`, or `f64::INFINITY` where no wavelength applies.
    pub sampling_mm: f64,
    /// `min(arc_mm, sampling_mm)` — what [`ReferencePlate::mesh`] actually
    /// uses for this zone.
    pub adopted_mm: f64,
    /// Which term bound.
    pub bound_by: TessBound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TessBound {
    Curvature,
    Sampling,
    /// The surface is exactly representable at its breakpoints — planar
    /// flanks, flat terraces, the datum. No refinement buys anything.
    Exact,
}

/// Measured piecewise-linear interpolation error for one zone, µm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessError {
    pub p50_um: f64,
    pub p99_um: f64,
    pub max_um: f64,
    pub probes: usize,
}

/// The plate.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePlate {
    zones: Vec<ZoneSpec>,
    half_extent_mm: f64,
    tess: Tessellation,
}

/// The canonical tile pitch: a 4×4 lattice of 24 mm tiles is 96×96 mm.
pub const TILE_MM: f64 = 24.0;
/// Half-extent of a single tile.
pub const TILE_HALF: f64 = TILE_MM / 2.0;

impl ReferencePlate {
    /// **The canonical 96×96 mm part: sixteen tiles, nine zone kinds.**
    ///
    /// A 4×4 lattice of 24 mm **axis-aligned** tiles. Layout in declaration
    /// order, which is also emission order (the determinism contract):
    ///
    /// | tile centre | zone | why it is here |
    /// |---|---|---|
    /// | (−36,−36) | Datum | the null control; flat-ground cusp must equal `R − √(R²−(d/2)²)` |
    /// | (−12,−36) | Dome R8 | convex chord-sag gouge; exact band areas; exact iso-slope circles |
    /// | (+12,−36) | Bowl R8 | **concave** reach limit — a convex fixture cannot adjudicate a concave defect |
    /// | (+36,−36) | Saddle ρ8 | κ of opposite signs; a scalar mean-curvature classifier reads zero |
    /// | (−36,−12) | Cone 44° | band run-off, Shallow side of the 45° dial |
    /// | (−12,−12) | Cone 46° | band run-off, MidSteep side |
    /// | (+12,−12) | Cone 74° | band run-off, MidSteep side of the 75° dial |
    /// | (+36,−12) | Cone 76° | band run-off, VerySteep side |
    /// | (−36,+12) | U comb R 0.20 / 0.35 | **blocked** at both ρ = 0.5 and ρ = 1.5 |
    /// | (−12,+12) | U comb R 0.50 / 0.80 | **enterable at ρ = 0.5, blocked at ρ = 1.5** — the sign change |
    /// | (+12,+12) | U comb R 1.50 | enterable at both |
    /// | (+36,+12) | V comb α 15° / 30° | floor always > 0; the never-enterable control |
    /// | (−36,+36) | V comb α 45° | the shallowest V, still never enterable |
    /// | (−12,+36) | Step terrace | Z-level/waterline; the terrace-phantom trap |
    /// | (+12,+36) | Ripple λ0.3 | trough radius 0.076 mm — a ρ=0.5 ball **bridges** it |
    /// | (+36,+36) | Ripple λ2.4 | trough radius 0.608 mm — **reaches bottom** |
    ///
    /// **The combs are split across tiles rather than given a double tile.**
    /// §3.3 defect 3 requires `pitch ≥ 2·u_max + envelope_diameter + 2` —
    /// 11.35 mm for a Ø6.35 tool — which fits **two** lanes in a 24 mm tile,
    /// not five. See [`COMB_PITCH_MM`]. Splitting is what makes the reach
    /// floor's **sign change** land between two adjacent tiles, where it is
    /// legible, instead of inside one comb.
    ///
    /// The two ripples share **one** amplitude ratio, so their max slope is
    /// identical (32.14°) while the trough radius sweeps 8× and crosses
    /// ρ = 0.5 between them. That is the only place in this repo where
    /// curvature is varied with slope held fixed.
    ///
    #[must_use]
    pub fn arp1() -> Self {
        let t = TILE_MM;
        let c = |ix: f64, iy: f64| P2::new(ix * t, iy * t);
        // Rotations alternate 20/30 deg — non-zero and non-45 so no zone is
        // commensurate with a raster or a dexel lattice (§2). The tiles
        // themselves stay axis-aligned; see `ZoneSpec::owns`.
        let zones = vec![
            // ── row 1 (y = -36) ────────────────────────────────────────────
            ZoneSpec::new(ZoneParams::Datum, c(-1.5, -1.5), TILE_HALF, 0.0),
            ZoneSpec::new(cap(1.0), c(-0.5, -1.5), TILE_HALF, 20.0),
            ZoneSpec::new(cap(-1.0), c(0.5, -1.5), TILE_HALF, 30.0),
            ZoneSpec::new(
                ZoneParams::Saddle { rho_mm: 8.0 },
                c(1.5, -1.5),
                TILE_HALF,
                20.0,
            ),
            // ── row 2 (y = -12): the band-run-off ladder ───────────────────
            ZoneSpec::new(cone(44.0), c(-1.5, -0.5), TILE_HALF, 30.0),
            ZoneSpec::new(cone(46.0), c(-0.5, -0.5), TILE_HALF, 20.0),
            ZoneSpec::new(cone(74.0), c(0.5, -0.5), TILE_HALF, 30.0),
            ZoneSpec::new(cone(76.0), c(1.5, -0.5), TILE_HALF, 20.0),
            // ── row 3 (y = +12): the U ladder + the first V pair ───────────
            ZoneSpec::new(u_comb(&[0.20, 0.35]), c(-1.5, 0.5), TILE_HALF, 20.0),
            ZoneSpec::new(u_comb(&[0.50, 0.80]), c(-0.5, 0.5), TILE_HALF, 30.0),
            ZoneSpec::new(u_comb(&[1.50]), c(0.5, 0.5), TILE_HALF, 20.0),
            ZoneSpec::new(v_comb(&[15.0, 30.0]), c(1.5, 0.5), TILE_HALF, 30.0),
            // ── row 4 (y = +36) ───────────────────────────────────────────
            ZoneSpec::new(v_comb(&[45.0]), c(-1.5, 1.5), TILE_HALF, 20.0),
            ZoneSpec::new(
                ZoneParams::StepTerrace {
                    levels: 4,
                    drop_mm: 1.0,
                },
                c(-0.5, 1.5),
                TILE_HALF,
                30.0,
            ),
            ZoneSpec::new(ripple(0.3), c(0.5, 1.5), TILE_HALF, 20.0),
            ZoneSpec::new(ripple(2.4), c(1.5, 1.5), TILE_HALF, 30.0),
        ];
        Self {
            zones,
            half_extent_mm: 2.0 * TILE_MM,
            tess: Tessellation::default(),
        }
    }

    /// One zone on its own tile, centred at the origin — the T1-tier
    /// constructor. A fast unit test builds this; it does not pay for the
    /// plate.
    #[must_use]
    pub fn single(spec: ZoneSpec) -> Self {
        let half = spec.half_extent;
        Self {
            zones: vec![spec],
            half_extent_mm: half,
            tess: Tessellation::default(),
        }
    }

    #[must_use]
    pub fn with_tess_epsilon(mut self, mm: f64) -> Self {
        self.tess.epsilon_mm = mm;
        self
    }

    #[must_use]
    pub fn with_along_step(mut self, mm: f64) -> Self {
        self.tess.along_step_mm = mm;
        self
    }

    #[must_use]
    pub fn zones(&self) -> &[ZoneSpec] {
        &self.zones
    }

    #[must_use]
    pub fn half_extent_mm(&self) -> f64 {
        self.half_extent_mm
    }

    /// The zone whose **support** contains `(x, y)`, or `None` for datum
    /// gutter and off-part.
    ///
    /// This is the exact analytic attribution that replaces dilated band maps.
    /// Supports are disjoint by construction, so the first match is the only
    /// match; the scan order is the declaration order, and a debug assertion
    /// in the contract test proves disjointness rather than assuming it.
    #[must_use]
    pub fn zone_at(&self, x: f64, y: f64) -> Option<Zone> {
        self.spec_at(x, y).map(ZoneSpec::zone)
    }

    #[must_use]
    fn spec_at(&self, x: f64, y: f64) -> Option<&ZoneSpec> {
        if x.abs() > self.half_extent_mm || y.abs() > self.half_extent_mm {
            return None;
        }
        self.zones.iter().find(|s| s.owns(x, y))
    }

    /// The full local jet at a world point, with the owning spec.
    #[must_use]
    fn jet_at(&self, x: f64, y: f64) -> Option<(&ZoneSpec, LocalJet)> {
        let spec = self.spec_at(x, y)?;
        let (u, v) = spec.to_local(x, y);
        spec.params.jet(u, v).map(|j| (spec, j))
    }

    /// Exact height. `0.0` on the datum gutter, `NaN` off the part.
    #[must_use]
    pub fn z_at(&self, x: f64, y: f64) -> f64 {
        if x.abs() > self.half_extent_mm || y.abs() > self.half_extent_mm {
            return f64::NAN;
        }
        match self.jet_at(x, y) {
            Some((_, j)) => j.z,
            // Either datum gutter (flat) or a crease/riser, where the height
            // is still well defined even though the normal is not. Re-read the
            // owning zone directly for the latter.
            None => self.crease_height(x, y),
        }
    }

    /// Height at a point whose jet is `None` — a crease, an apex or a riser.
    /// The height is single-valued there even where the normal is not; only
    /// the riser is genuinely multi-valued, and it resolves to the LOWER
    /// terrace so the surface stays a function.
    #[must_use]
    fn crease_height(&self, x: f64, y: f64) -> f64 {
        let Some(spec) = self.spec_at(x, y) else {
            return 0.0;
        };
        let (u, v) = spec.to_local(x, y);
        match &spec.params {
            ZoneParams::VGrooveComb {
                alphas_deg,
                half_width_mm,
                pitch_mm,
            } => comb_lane(u, alphas_deg.len(), *pitch_mm).map_or(0.0, |(k, du)| {
                -(half_width_mm - du.abs()) / alphas_deg[k].to_radians().tan()
            }),
            ZoneParams::StepTerrace { levels, drop_mm } => {
                let width = 2.0 * TERRACE_HALF_EXTENT / *levels as f64;
                let k = ((u + TERRACE_HALF_EXTENT) / width)
                    .floor()
                    .clamp(0.0, *levels as f64 - 1.0);
                -drop_mm * k
            }
            _ => spec.params.jet(u, v).map_or(0.0, |j| j.z),
        }
    }

    /// Exact **unit surface normal**, in world.
    ///
    /// `None` at a crease, a vertical riser, or off the part. This is the
    /// evaluator that closes the gap named in spec §5.1:
    /// `EnvelopeOracle::slope_deg` is a central difference on the sampled
    /// height grid, so (a) its error grows with slope, exactly where the bands
    /// are decided, and (b) **the VerySteep band is systematically
    /// under-populated by the cells that matter**, because a stencil at the
    /// rim of a steep feature crosses uncovered ground and is `NaN`'d out. An
    /// analytic normal removes both.
    #[must_use]
    pub fn normal_at(&self, x: f64, y: f64) -> Option<[f64; 3]> {
        let (spec, j) = self.jet_at(x, y)?;
        let (nx, ny) = spec.vec_to_world(-j.zu, -j.zv);
        let n = (nx * nx + ny * ny + 1.0).sqrt();
        Some([nx / n, ny / n, 1.0 / n])
    }

    /// Exact slope from horizontal, degrees. Rotation-invariant, so it needs
    /// no frame change.
    #[must_use]
    pub fn slope_deg_at(&self, x: f64, y: f64) -> Option<f64> {
        let (_, j) = self.jet_at(x, y)?;
        Some(j.zu.hypot(j.zv).atan().to_degrees())
    }

    /// Exact slope band. The three-band map every shipped classifier uses.
    ///
    /// **A gate on the cone ladder must not read this** — a 44° cone and the
    /// flat datum are the same colour in a three-band map, which makes the
    /// band-run-off pair invisible in the very figure meant to display it
    /// (spec §3.3 defect 1). Read [`Self::band_area_mm2`] per zone instead.
    #[must_use]
    pub fn band_at(&self, x: f64, y: f64) -> Option<SlopeBand> {
        self.slope_deg_at(x, y).map(SlopeBand::of_angle_deg)
    }

    /// Exact principal curvatures `(κ_max, κ_min)`, positive = convex
    /// material. See [`principal_curvatures`] for the sign convention.
    #[must_use]
    pub fn curvature_at(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (_, j) = self.jet_at(x, y)?;
        Some(principal_curvatures(&j))
    }

    /// **The exact tool-reach floor** at a point: the residual that no
    /// algorithm can remove, because the tool physically cannot get there.
    ///
    /// This is the fixture's single most valuable output. Every prior quality
    /// campaign in this repo has had to *argue* about how much of a residual
    /// was tool reach; here it is arithmetic, and it changes with the tool in
    /// a way the harness can predict before it runs.
    ///
    /// | feature | ρ = 0.5 | ρ = 1.5 |
    /// |---|---|---|
    /// | U groove R=0.20 | 158.3 µm | 186.6 µm |
    /// | U groove R=0.50 | **0 (enterable)** | 414.2 µm |
    /// | U groove R=1.50 | **0** | **0** |
    /// | V groove α=15° | 1431.9 µm | 4295.6 µm |
    /// | V groove α=45° | 207.1 µm | 621.3 µm |
    ///
    /// **`None` means NOT DEFINED IN CLOSED FORM, never "zero"** — the
    /// [`Zone::MicroRipple`] and [`Zone::StepTerrace`] floors have no
    /// elementary expression and this deliberately refuses to invent one.
    /// Coercing an absent value to zero would turn "we cannot say" into "the
    /// tool reaches", which is the exact direction that flatters an algorithm.
    /// For the ripple use [`ZoneParams::trough_radius_mm`] and the bridging
    /// predicate instead.
    ///
    /// Uses the cutter's **tip** radius via `height_at_radius`, so tapered and
    /// bull-nose cutters are in scope, not just spheres.
    #[must_use]
    pub fn reach_floor_at(&self, x: f64, y: f64, cutter: &dyn MillingCutter) -> Option<f64> {
        let spec = self.spec_at(x, y)?;
        let (u, _v) = spec.to_local(x, y);
        let rho = cutter.cusp_radius_mm();
        match &spec.params {
            ZoneParams::UGrooveComb {
                radii_mm, pitch_mm, ..
            } => {
                let (k, _) = comb_lane(u, radii_mm.len(), *pitch_mm)?;
                Some(u_groove_reach_floor(radii_mm[k], rho))
            }
            ZoneParams::VGrooveComb {
                alphas_deg,
                pitch_mm,
                ..
            } => {
                let (k, _) = comb_lane(u, alphas_deg.len(), *pitch_mm)?;
                Some(v_groove_reach_floor(alphas_deg[k], rho))
            }
            // Convex and flat ground: a ball reaches it everywhere.
            ZoneParams::Datum | ZoneParams::Saddle { .. } | ZoneParams::ConeAnnulus { .. } => {
                Some(0.0)
            }
            // A CONCAVE cap of radius R obstructs exactly like a U groove: the
            // ball rides the rim once ρ > R. Same arithmetic, spherical rather
            // than cylindrical — the binding contact is the great circle
            // through the centreline. A convex cap (the dome) obstructs
            // nothing.
            //
            // Written as a guard, not a `sign: 1.0` literal pattern: float
            // literal patterns are a hard error, and matching a sign bit by
            // equality is the wrong shape anyway.
            ZoneParams::SphereCap {
                radius_mm, sign, ..
            } => {
                if *sign >= 0.0 {
                    Some(0.0)
                } else {
                    Some(u_groove_reach_floor(*radius_mm, rho))
                }
            }
            ZoneParams::MicroRipple { .. } | ZoneParams::StepTerrace { .. } => None,
        }
    }

    // ── tessellation ───────────────────────────────────────────────────────

    /// The §4.1 rule's output for a zone, with both terms exposed.
    #[must_use]
    pub fn tess_step_for(&self, zone: Zone) -> Option<TessStep> {
        let spec = self.zones.iter().find(|s| s.zone() == zone)?;
        Some(tess_step(&spec.params, self.tess.epsilon_mm))
    }

    /// The deterministic mesh.
    ///
    /// **An OPEN top surface** — no walls, no floor. Consumers that need a
    /// closed volume add their own; this follows
    /// [`super::meshes::grooved_block`]'s precedent, not `plateau`'s, and the
    /// module doc records why.
    ///
    /// Emission order: zones in declaration order, then the datum ground
    /// plane. Within a zone, natural-parameter outer/inner order. All windings
    /// +Z. No `HashMap` anywhere.
    #[must_use]
    pub fn mesh(&self) -> TriangleMesh {
        let mut verts: Vec<P3> = Vec::new();
        let mut tris: Vec<[u32; 3]> = Vec::new();
        for spec in &self.zones {
            self.emit_zone(spec, &mut verts, &mut tris);
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// Measured piecewise-linear interpolation error for one zone, in µm, by
    /// differencing the zone's own tessellation against the closed form.
    ///
    /// This is the measurement the whole tessellation rule rests on: **the
    /// difference between the tessellated reading and the analytic one IS the
    /// tessellation error**, and only a fixture with an analytic surface can
    /// compute it. terrain.stl cannot, at any refinement.
    #[must_use]
    pub fn measured_tess_error(&self, zone: Zone) -> Option<TessError> {
        let spec = self.zones.iter().find(|s| s.zone() == zone)?;
        Some(measure_tess_error(&spec.params, self.tess.epsilon_mm, 4001))
    }

    // ── exact areas ────────────────────────────────────────────────────────

    /// **Exact** 3D surface area of one slope band within one zone, mm².
    ///
    /// A coverage or band-run-off gate can be written as a fraction of this
    /// exact denominator, instead of a count of cells on a TIN.
    ///
    /// | band on the R8 cap | closed form `2πR²(cos t₀ − cos t₁)` |
    /// |---|---|
    /// | Shallow 0–45° | 117.7794 mm² |
    /// | MidSteep 45–75° | 180.2672 mm² |
    /// | VerySteep 75–85° | 69.0299 mm² |
    ///
    /// Returns `None` for zones whose band decomposition has no elementary
    /// closed form ([`Zone::Saddle`], [`Zone::MicroRipple`]) — again, `None`
    /// is **not measured**, never zero. Use [`Self::band_area_quadrature`] for
    /// those and label the result as numeric.
    #[must_use]
    pub fn band_area_mm2(&self, zone: Zone, band: SlopeBand) -> Option<f64> {
        let spec = self.zones.iter().find(|s| s.zone() == zone)?;
        let (lo, hi) = band_limits(band);
        match &spec.params {
            ZoneParams::SphereCap {
                radius_mm,
                theta_max_deg,
                ..
            } => {
                // A = 2πR²(cos t₀ − cos t₁), clipped to the cap's own rim.
                let t0 = lo.to_radians();
                let t1 = hi.min(*theta_max_deg).to_radians();
                if t1 <= t0 {
                    return Some(0.0);
                }
                Some(2.0 * PI * radius_mm * radius_mm * (t0.cos() - t1.cos()))
            }
            ZoneParams::ConeAnnulus {
                theta_deg,
                r0_mm,
                r1_mm,
            } => {
                // Slope is EXACTLY theta everywhere, so the frustum's whole
                // slant area lands in exactly one band. This is why the cone
                // ladder can exhibit band run-off with no ambiguity at all.
                if *theta_deg < lo || *theta_deg >= hi {
                    return Some(0.0);
                }
                Some(PI * (r1_mm * r1_mm - r0_mm * r0_mm) / theta_deg.to_radians().cos())
            }
            ZoneParams::UGrooveComb {
                radii_mm,
                pitch_mm,
                phi_max_deg,
            } => {
                // Wrap angle IS the slope angle on a circular groove, so the
                // band is an exact arc: A = 2·R·Δφ·length per groove. The
                // length is the lane's EXACT chord inside the axis-aligned
                // tile, not `2·half_extent` — the feature frame is rotated, so
                // a lane crosses the tile diagonally and its own centre line
                // is longer than the tile is wide. Assuming otherwise would
                // under-report every comb area by `1/cos φ` or more.
                let cap = phi_max_deg.min(90.0);
                let a = lo.min(cap).to_radians();
                let b = hi.min(cap).to_radians();
                if b <= a {
                    return Some(0.0);
                }
                Some(
                    radii_mm
                        .iter()
                        .enumerate()
                        .map(|(k, r)| {
                            let c = lane_centre(k, radii_mm.len(), *pitch_mm);
                            let len = lane_chord_mm(c, spec.plan_rotation_deg, spec.half_extent);
                            2.0 * r * (b - a) * len
                        })
                        .sum(),
                )
            }
            ZoneParams::VGrooveComb {
                alphas_deg,
                half_width_mm,
                pitch_mm,
            } => {
                // Planar flanks: slope = 90 − α exactly, area = projected/sin α,
                // over the lane's exact clipped chord.
                Some(
                    alphas_deg
                        .iter()
                        .enumerate()
                        .map(|(k, a)| {
                            let slope = 90.0 - a;
                            if slope < lo || slope >= hi {
                                return 0.0;
                            }
                            let c = lane_centre(k, alphas_deg.len(), *pitch_mm);
                            let len = lane_chord_mm(c, spec.plan_rotation_deg, spec.half_extent);
                            2.0 * half_width_mm * len / a.to_radians().sin()
                        })
                        .sum(),
                )
            }
            ZoneParams::Datum | ZoneParams::StepTerrace { .. } => {
                // All flat: everything is Shallow, and the vertical risers
                // have zero projected area AND are invisible to a normal-based
                // band map. Asserted as a property, not worked around.
                let side = 2.0 * spec.half_extent;
                Some(if band == SlopeBand::Shallow {
                    side * side
                } else {
                    0.0
                })
            }
            ZoneParams::Saddle { .. } | ZoneParams::MicroRipple { .. } => None,
        }
    }

    /// Numerically integrated band area for the zones with no closed form.
    ///
    /// Separated from [`Self::band_area_mm2`] on purpose: a number computed by
    /// quadrature must not be able to masquerade as one computed exactly. The
    /// grid is `n × n` over the zone's tile, midpoint rule on `√(1+z_u²+z_v²)`.
    #[must_use]
    pub fn band_area_quadrature(&self, zone: Zone, band: SlopeBand, n: usize) -> Option<f64> {
        let spec = self.zones.iter().find(|s| s.zone() == zone)?;
        let (lo, hi) = band_limits(band);
        // Integrate over the tile's AXIS-ALIGNED square in world XY, since
        // that is the ground the zone actually owns (`ZoneSpec::owns`).
        let h = 2.0 * spec.half_extent / n as f64;
        let mut acc = 0.0;
        for i in 0..n {
            let x = spec.centre.x - spec.half_extent + (i as f64 + 0.5) * h;
            for k in 0..n {
                let y = spec.centre.y - spec.half_extent + (k as f64 + 0.5) * h;
                if !spec.owns(x, y) {
                    continue;
                }
                let (u, v) = spec.to_local(x, y);
                let Some(j) = spec.params.jet(u, v) else {
                    continue;
                };
                let slope = j.zu.hypot(j.zv).atan().to_degrees();
                if slope >= lo && slope < hi {
                    acc += (1.0 + j.zu * j.zu + j.zv * j.zv).sqrt() * h * h;
                }
            }
        }
        Some(acc)
    }
}

fn band_limits(band: SlopeBand) -> (f64, f64) {
    match band {
        SlopeBand::Shallow => (0.0, 45.0),
        SlopeBand::MidSteep => (45.0, 75.0),
        SlopeBand::VerySteep => (75.0, 90.0),
    }
}

// ===========================================================================
// Closed-form reach floors
// ===========================================================================

/// A ball of radius `rho` inside a concave circular cylinder of radius `R`.
///
/// * `ρ ≤ R` — the ball is tangent at the bottom, tip reaches `z = −R`,
///   residual **0**.
/// * `ρ > R` — the ball rides the two rim corners at `(±R, 0)`; its centre
///   sits at `z_c = √(ρ²−R²)`, tip at `z_c − ρ`, so the residual at the
///   centreline is exactly `R + √(ρ²−R²) − ρ`.
#[must_use]
pub fn u_groove_reach_floor(radius_mm: f64, rho_mm: f64) -> f64 {
    if rho_mm <= radius_mm {
        0.0
    } else {
        radius_mm + (rho_mm * rho_mm - radius_mm * radius_mm).sqrt() - rho_mm
    }
}

/// A ball of radius `rho` seated in a symmetric V of half-angle `alpha` from
/// the **vertical**.
///
/// The centre sits `ρ / sin α` above the apex, so the tip is
/// `ρ(1 − sin α)/sin α` above it — **always positive**. A V groove is never
/// fully enterable, which is what makes it the honest "this residual is
/// geometry, not algorithm" control.
#[must_use]
pub fn v_groove_reach_floor(alpha_deg: f64, rho_mm: f64) -> f64 {
    let s = alpha_deg.to_radians().sin();
    rho_mm * (1.0 - s) / s
}

impl ZoneParams {
    /// Concave curvature radius at a ripple trough: `λ²/(4π²A)`. A ball of
    /// radius `ρ` **bridges** relief whose trough radius is smaller than `ρ`,
    /// so the exact bridging threshold is `ρ > λ²/(4π²A)`.
    #[must_use]
    pub fn trough_radius_mm(&self) -> Option<f64> {
        match self {
            Self::MicroRipple {
                lambda_mm,
                amp_ratio,
            } => Some(lambda_mm * lambda_mm / (4.0 * PI * PI * (lambda_mm * amp_ratio))),
            _ => None,
        }
    }

    /// Max slope, degrees — closed form where one exists.
    #[must_use]
    pub fn max_slope_deg(&self) -> Option<f64> {
        match self {
            Self::Datum | Self::StepTerrace { .. } => Some(0.0),
            Self::SphereCap { theta_max_deg, .. } => Some(*theta_max_deg),
            Self::ConeAnnulus { theta_deg, .. } => Some(*theta_deg),
            Self::UGrooveComb { phi_max_deg, .. } => Some(*phi_max_deg),
            Self::VGrooveComb { alphas_deg, .. } => alphas_deg
                .iter()
                .map(|a| 90.0 - a)
                .fold(None, |m: Option<f64>, s| Some(m.map_or(s, |x| x.max(s)))),
            Self::MicroRipple {
                lambda_mm,
                amp_ratio,
            } => Some(
                ((lambda_mm * amp_ratio) * 2.0 * PI / lambda_mm)
                    .atan()
                    .to_degrees(),
            ),
            Self::Saddle { .. } => None,
        }
    }

    /// Radius of the iso-slope circle on a spherical cap: `r = R sin θ`,
    /// **exact**. The 45° circle on the R8 cap is at 5.656854 mm and the 75°
    /// circle at 7.727407 mm — the non-vacuity check spec §3 names.
    #[must_use]
    pub fn iso_slope_radius_mm(&self, deg: f64) -> Option<f64> {
        match self {
            Self::SphereCap { radius_mm, .. } => Some(radius_mm * deg.to_radians().sin()),
            _ => None,
        }
    }

    /// The **height-field** second derivative used by the XY term of the
    /// §4.1 rule — the maximum of `|∂²z/∂u²|` and `|∂²z/∂v²|` over the scored
    /// region. NOT the surface principal curvature; see the module doc.
    #[must_use]
    pub fn kappa_height_field(&self) -> f64 {
        match self {
            Self::Datum | Self::StepTerrace { .. } | Self::VGrooveComb { .. } => 0.0,
            // At the apex, |z_uu| = 1/R; it diverges toward the rim, which is
            // §4.4's whole point, so the XY figure quotes the apex value and
            // the arc figure is the honest one.
            Self::SphereCap { radius_mm, .. } => 1.0 / radius_mm,
            Self::Saddle { rho_mm } => 1.0 / rho_mm,
            Self::ConeAnnulus {
                theta_deg, r0_mm, ..
            } => {
                let r = r0_mm.max(1e-9);
                theta_deg.to_radians().tan() / r
            }
            Self::UGrooveComb { radii_mm, .. } => radii_mm
                .iter()
                .fold(0.0_f64, |m, r| m.max(1.0 / r.max(1e-12))),
            Self::MicroRipple {
                lambda_mm,
                amp_ratio,
            } => (lambda_mm * amp_ratio) * (2.0 * PI / lambda_mm).powi(2),
        }
    }

    /// The **surface** curvature the arc-chord bound uses.
    #[must_use]
    pub fn kappa_surface(&self) -> f64 {
        match self {
            Self::SphereCap { radius_mm, .. } => 1.0 / radius_mm,
            Self::UGrooveComb { radii_mm, .. } => radii_mm
                .iter()
                .fold(0.0_f64, |m, r| m.max(1.0 / r.max(1e-12))),
            // A cone's non-zero principal curvature is sin θ / r — see the
            // divergence note in `reference_plate_contract`.
            Self::ConeAnnulus {
                theta_deg, r0_mm, ..
            } => theta_deg.to_radians().sin() / r0_mm.max(1e-9),
            _ => self.kappa_height_field(),
        }
    }

    /// The `λ_min / 8` sampling term. `INFINITY` where no wavelength applies.
    #[must_use]
    pub fn sampling_limit_mm(&self) -> f64 {
        match self {
            Self::MicroRipple { lambda_mm, .. } => lambda_mm / 8.0,
            Self::UGrooveComb { radii_mm, .. } => {
                // The smallest groove's full width is its own "wavelength".
                radii_mm
                    .iter()
                    .fold(f64::INFINITY, |m: f64, r| m.min(2.0 * r))
                    / 8.0
            }
            _ => f64::INFINITY,
        }
    }
}

/// Comb pitch, mm. The §3.3 defect-3 constraint is
/// `pitch >= 2*u_max + envelope_diameter + 2`: with the widest lane at
/// `u_max = 1.5*sin85 = 1.494` mm and the largest tool in the §7 dial range at
/// Ø6.35, that is 11.35 mm. **12.0 mm leaves 8.36 mm of flat between lanes —
/// more than a Ø6.35 tool needs to establish a rim.** Two lanes per 24 mm tile.
pub const COMB_PITCH_MM: f64 = 12.0;

fn cap(sign: f64) -> ZoneParams {
    ZoneParams::SphereCap {
        radius_mm: 8.0,
        sign,
        theta_max_deg: 85.0,
    }
}

fn u_comb(radii: &[f64]) -> ZoneParams {
    ZoneParams::UGrooveComb {
        radii_mm: radii.to_vec(),
        pitch_mm: COMB_PITCH_MM,
        phi_max_deg: 85.0,
    }
}

fn v_comb(alphas: &[f64]) -> ZoneParams {
    ZoneParams::VGrooveComb {
        alphas_deg: alphas.to_vec(),
        half_width_mm: 1.5,
        pitch_mm: COMB_PITCH_MM,
    }
}

fn cone(theta_deg: f64) -> ZoneParams {
    ZoneParams::ConeAnnulus {
        theta_deg,
        r0_mm: 3.0,
        r1_mm: 9.0,
    }
}

fn ripple(lambda_mm: f64) -> ZoneParams {
    ZoneParams::MicroRipple {
        lambda_mm,
        amp_ratio: 0.10,
    }
}

/// Solve the §4.1 rule for one zone.
#[must_use]
pub fn tess_step(params: &ZoneParams, epsilon_mm: f64) -> TessStep {
    let k_surf = params.kappa_surface();
    let k_hf = params.kappa_height_field();
    let sampling = params.sampling_limit_mm();
    // Edge bound `ε ≤ κ s²/8` → `s = √(8ε/κ)`, in surface chord.
    let arc = if k_surf > 0.0 {
        (8.0 * epsilon_mm / k_surf).sqrt()
    } else {
        f64::INFINITY
    };
    // Diagonal bound `ε ≤ κ_hf s²/4` → `s = 2√(ε/κ_hf)`, on an XY lattice.
    let xy = if k_hf > 0.0 {
        2.0 * (epsilon_mm / k_hf).sqrt()
    } else {
        f64::INFINITY
    };
    let adopted = arc.min(sampling);
    let bound_by = if !adopted.is_finite() {
        TessBound::Exact
    } else if sampling < arc {
        TessBound::Sampling
    } else {
        TessBound::Curvature
    };
    TessStep {
        arc_mm: arc,
        xy_mm: xy,
        sampling_mm: sampling,
        adopted_mm: adopted,
        bound_by,
    }
}

// ===========================================================================
// Tessellation error measurement
// ===========================================================================

/// Difference a zone's own 1D natural-parameter tessellation against the
/// closed form, on a dense probe set. Reports p50/p99/max in µm.
///
/// The zones this is defined for are the ones whose worst direction is 1D —
/// caps along the meridian, grooves and ripples across the feature. That is
/// every zone with curvature; the saddle is measured on its XY grid, which is
/// its natural parameter.
#[must_use]
fn measure_tess_error(params: &ZoneParams, epsilon_mm: f64, probes: usize) -> TessError {
    let step = tess_step(params, epsilon_mm);
    let s = step.adopted_mm;
    // Sample knots in the natural parameter, then evaluate the piecewise
    // linear interpolant at dense probes and difference against the exact z.
    let (lo, hi, knots): (f64, f64, Vec<f64>) = match params {
        ZoneParams::SphereCap {
            radius_mm,
            theta_max_deg,
            ..
        } => {
            // Uniform in POLAR ANGLE — constant chord on the surface.
            let th_max = theta_max_deg.to_radians();
            let dth = (s / radius_mm).max(1e-9);
            let n = (th_max / dth).ceil() as usize + 1;
            let ks = (0..n)
                .map(|i| radius_mm * (th_max * i as f64 / (n - 1) as f64).sin())
                .collect();
            (0.0, radius_mm * th_max.sin(), ks)
        }
        ZoneParams::UGrooveComb {
            radii_mm,
            phi_max_deg,
            ..
        } => {
            // Uniform in WRAP ANGLE, on the tightest groove (worst case).
            let radius = radii_mm.iter().fold(f64::INFINITY, |m: f64, r| m.min(*r));
            let phi_max = phi_max_deg.to_radians();
            let dphi = (s / radius).max(1e-9);
            let n = (2.0 * phi_max / dphi).ceil() as usize + 1;
            let ks = (0..n)
                .map(|i| radius * (-phi_max + 2.0 * phi_max * i as f64 / (n - 1) as f64).sin())
                .collect();
            (-radius * phi_max.sin(), radius * phi_max.sin(), ks)
        }
        ZoneParams::MicroRipple { lambda_mm, .. } => {
            // Uniform in u across one full wavelength.
            let n = (lambda_mm / s).ceil() as usize + 1;
            let ks = (0..n)
                .map(|i| -lambda_mm / 2.0 + lambda_mm * i as f64 / (n - 1) as f64)
                .collect();
            (-lambda_mm / 2.0, lambda_mm / 2.0, ks)
        }
        ZoneParams::Saddle { .. } => {
            let half = 4.5;
            let n = (2.0 * half / s).ceil() as usize + 1;
            let ks = (0..n)
                .map(|i| -half + 2.0 * half * i as f64 / (n - 1) as f64)
                .collect();
            (-half, half, ks)
        }
        // Exactly representable at their breakpoints.
        _ => {
            return TessError {
                p50_um: 0.0,
                p99_um: 0.0,
                max_um: 0.0,
                probes: 0,
            };
        }
    };

    let zk: Vec<f64> = knots
        .iter()
        .map(|&u| params.jet(u, 0.0).map_or(0.0, |j| j.z))
        .collect();
    let mut errs: Vec<f64> = Vec::with_capacity(probes);
    for i in 0..probes {
        let u = lo + (hi - lo) * i as f64 / (probes - 1) as f64;
        // A probe that lands a float epsilon outside the support reads the
        // DATUM (z = 0), not the surface, and would report the feature's full
        // depth as tessellation error. Skip rather than let a rounding artefact
        // dominate a p99.
        if !params.supports(u, 0.0) {
            continue;
        }
        let exact = match params.jet(u, 0.0) {
            Some(j) => j.z,
            None => continue,
        };
        // Locate u in the knot vector (knots are monotone by construction).
        let mut k = 0usize;
        while k + 2 < knots.len() && knots[k + 1] < u {
            k += 1;
        }
        let (u0, u1) = (knots[k], knots[k + 1]);
        let lin = if (u1 - u0).abs() < 1e-15 {
            zk[k]
        } else {
            zk[k] + (zk[k + 1] - zk[k]) * (u - u0) / (u1 - u0)
        };
        errs.push((lin - exact).abs() * 1000.0);
    }
    errs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = errs.len();
    TessError {
        p50_um: if n == 0 { 0.0 } else { errs[n / 2] },
        p99_um: if n == 0 {
            0.0
        } else {
            errs[((0.99 * n as f64) as usize).min(n - 1)]
        },
        max_um: errs.last().copied().unwrap_or(0.0),
        probes: n,
    }
}

// ===========================================================================
// Mesh emission
// ===========================================================================

impl ReferencePlate {
    /// The mesh's height source: [`Self::z_at`] clamped to `0` off the plate.
    ///
    /// **The mesh is a sampling of `z_at`, full stop.** Emitting from the
    /// zone's own `jet` instead would let a tile's sampling box paint its own
    /// feature over ground a *neighbouring* tile owns, so the mesh and the
    /// evaluators would disagree exactly where attribution is contested.
    /// Sourcing both from one function makes that class of bug unreachable —
    /// and `mesh_matches_the_analytic_surface` in the contract test asserts it.
    #[must_use]
    fn sampled_z(&self, x: f64, y: f64) -> f64 {
        let z = self.z_at(x, y);
        if z.is_finite() { z } else { 0.0 }
    }

    /// Emit one zone's tile: its support in the zone's natural parameter, plus
    /// the flat gutter out to the tile square.
    fn emit_zone(&self, spec: &ZoneSpec, verts: &mut Vec<P3>, tris: &mut Vec<[u32; 3]>) {
        match &spec.params {
            ZoneParams::SphereCap { .. } | ZoneParams::ConeAnnulus { .. } => {
                self.emit_radial(spec, verts, tris);
            }
            _ => self.emit_anisotropic_grid(spec, verts, tris),
        }
    }

    /// Circular-support zones: uniform in **polar angle** (caps) or in
    /// **azimuth** (cones), stitched to the tile square by a radial fan.
    ///
    /// The fan is exact regardless of how it is triangulated because every
    /// vertex in the gutter is at `z = 0`.
    fn emit_radial(&self, spec: &ZoneSpec, verts: &mut Vec<P3>, tris: &mut Vec<[u32; 3]>) {
        let step = tess_step(&spec.params, self.tess.epsilon_mm);
        let s = step.adopted_mm;
        // Radial knots in the zone's natural parameter.
        let radii: Vec<f64> = match &spec.params {
            ZoneParams::SphereCap {
                radius_mm,
                theta_max_deg,
                ..
            } => {
                let th_max = theta_max_deg.to_radians();
                let n = ((th_max / (s / radius_mm).max(1e-9)).ceil() as usize + 1).max(2);
                (0..n)
                    .map(|i| radius_mm * (th_max * i as f64 / (n - 1) as f64).sin())
                    .collect()
            }
            ZoneParams::ConeAnnulus { r0_mm, r1_mm, .. } => {
                // z is exactly linear in r, so radial refinement buys nothing;
                // three rings keep the triangles well shaped.
                (0..4)
                    .map(|i| r0_mm + (r1_mm - r0_mm) * i as f64 / 3.0)
                    .collect()
            }
            _ => return,
        };
        let r_outer = radii.last().copied().unwrap_or(0.0);
        // Azimuthal count: chord ≤ s at the outer radius, and never absurd.
        let n_phi = ((2.0 * PI * r_outer / s.max(1e-9)).ceil() as usize).clamp(12, 4096);

        let base = verts.len() as u32;
        // Ring 0 may be a single point (a cap apex) — emit it as a degenerate
        // ring so the index arithmetic stays uniform and deterministic.
        for &r in &radii {
            for i in 0..n_phi {
                let a = 2.0 * PI * i as f64 / n_phi as f64;
                let (u, v) = (r * a.cos(), r * a.sin());
                let (x, y) = spec.to_world(u, v);
                verts.push(P3::new(x, y, self.sampled_z(x, y)));
            }
        }
        // The gutter ring: project each azimuth out to the tile square
        // (Chebyshev projection), all at z = 0.
        for i in 0..n_phi {
            let a = 2.0 * PI * i as f64 / n_phi as f64;
            let (ca, sa) = (a.cos(), a.sin());
            let t = spec.half_extent / ca.abs().max(sa.abs()).max(1e-12);
            let (x, y) = spec.to_world(t * ca, t * sa);
            verts.push(P3::new(x, y, self.sampled_z(x, y)));
        }
        // Also stitch the cone's inner hole shut at its own flat floor level.
        let ring_count = radii.len() + 1;
        // A cap's innermost ring is the apex: `r = 0` collapses all `n_phi`
        // vertices onto one point, so half of that ring's quads are
        // degenerate. Emit only non-degenerate triangles — a zero-area facet
        // has no normal and would poison any consumer that averages them.
        let distinct = |verts: &[P3], p: u32, q: u32, r: u32| -> bool {
            let (a, b, c) = (verts[p as usize], verts[q as usize], verts[r as usize]);
            let same = |u: P3, v: P3| {
                (u.x - v.x).abs() < 1e-12 && (u.y - v.y).abs() < 1e-12 && (u.z - v.z).abs() < 1e-12
            };
            !same(a, b) && !same(b, c) && !same(a, c)
        };
        for ring in 0..ring_count - 1 {
            for i in 0..n_phi {
                let j = (i + 1) % n_phi;
                let a = base + (ring * n_phi + i) as u32;
                let b = base + (ring * n_phi + j) as u32;
                let c = base + ((ring + 1) * n_phi + i) as u32;
                let d = base + ((ring + 1) * n_phi + j) as u32;
                if distinct(verts, a, c, b) {
                    tris.push([a, c, b]);
                }
                if distinct(verts, b, c, d) {
                    tris.push([b, c, d]);
                }
            }
        }
        // A cone annulus has a hole at r0; cap it flat at the rim height so
        // the surface is a function everywhere on the tile.
        if let ZoneParams::ConeAnnulus { .. } = spec.params {
            let centre = verts.len() as u32;
            let (cx, cy) = spec.to_world(0.0, 0.0);
            verts.push(P3::new(cx, cy, self.sampled_z(cx, cy)));
            for i in 0..n_phi {
                let j = (i + 1) % n_phi;
                tris.push([centre, base + i as u32, base + j as u32]);
            }
        }
    }

    /// Everything else: an **anisotropic** lattice — fine across the feature
    /// (the zone's natural parameter), coarse along it. `meshes.rs`'s
    /// `GroovedBlock` already does exactly this and is the precedent.
    fn emit_anisotropic_grid(
        &self,
        spec: &ZoneSpec,
        verts: &mut Vec<P3>,
        tris: &mut Vec<[u32; 3]>,
    ) {
        let step = tess_step(&spec.params, self.tess.epsilon_mm);
        let half = spec.half_extent;
        // Across-feature samples. Y-invariant zones get the fine step in u
        // only; the saddle is isotropic and gets it in both.
        let isotropic = matches!(spec.params, ZoneParams::Saddle { .. });
        let du = if step.adopted_mm.is_finite() {
            step.adopted_mm
        } else {
            half / 4.0
        };
        // The local box must cover the tile's AXIS-ALIGNED square once the
        // feature frame is rotated — see `ZoneSpec::owns`. The corners of that
        // box fall outside the tile and read flat datum; neighbouring tiles
        // then emit coplanar triangles at z = 0 over the same gutter, which is
        // harmless (every reading there is identical) and is the price of
        // keeping each zone's fine sampling in its own natural parameter.
        let cover = spec.local_cover_half();
        let us = sample_axis_over(half, cover, du, &spec.params);
        let dv = if isotropic {
            du
        } else {
            self.tess.along_step_mm
        };
        let vs = sample_axis_over(half, cover, dv, &ZoneParams::Datum);

        let base = verts.len() as u32;
        for &v in &vs {
            for &u in &us {
                let (x, y) = spec.to_world(u, v);
                verts.push(P3::new(x, y, self.sampled_z(x, y)));
            }
        }
        let nx = us.len();
        for jy in 0..vs.len() - 1 {
            for ix in 0..nx - 1 {
                let a = base + (jy * nx + ix) as u32;
                let b = a + 1;
                let c = base + ((jy + 1) * nx + ix) as u32;
                let d = c + 1;
                tris.push([a, c, b]);
                tris.push([b, c, d]);
            }
        }
    }
}

/// Sample `[-half, half]` at `step`, with the zone's breakpoints forced in
/// exactly so creases and rims are reproduced rather than approximated.
///
/// Sorted and deduped, mirroring `GroovedBlock::build`'s construction.
#[must_use]
fn sample_axis_over(breakpoint_half: f64, cover: f64, step: f64, params: &ZoneParams) -> Vec<f64> {
    let half = cover;
    let n = ((2.0 * half) / step.max(1e-9)).ceil() as usize;
    let n = n.clamp(2, 400_000);
    let mut xs: Vec<f64> = (0..=n)
        .map(|i| -half + 2.0 * half * i as f64 / n as f64)
        .collect();
    for b in breakpoints(params) {
        if b.abs() <= breakpoint_half.max(cover) {
            xs.push(b);
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    xs
}

/// The exact `u` values a zone's surface is non-smooth at.
#[must_use]
fn breakpoints(params: &ZoneParams) -> Vec<f64> {
    match params {
        ZoneParams::UGrooveComb {
            radii_mm,
            pitch_mm,
            phi_max_deg,
        } => {
            let sin_phi = phi_max_deg.to_radians().sin();
            let first = -0.5 * (radii_mm.len() as f64 - 1.0) * pitch_mm;
            let mut out = Vec::with_capacity(radii_mm.len() * 3);
            for (k, r) in radii_mm.iter().enumerate() {
                let c = first + k as f64 * pitch_mm;
                out.push(c - r * sin_phi);
                out.push(c);
                out.push(c + r * sin_phi);
            }
            out
        }
        ZoneParams::VGrooveComb {
            alphas_deg,
            half_width_mm,
            pitch_mm,
        } => {
            let first = -0.5 * (alphas_deg.len() as f64 - 1.0) * pitch_mm;
            let mut out = Vec::with_capacity(alphas_deg.len() * 3);
            for k in 0..alphas_deg.len() {
                let c = first + k as f64 * pitch_mm;
                out.push(c - half_width_mm);
                out.push(c);
                out.push(c + half_width_mm);
            }
            out
        }
        ZoneParams::StepTerrace { levels, .. } => {
            let width = 2.0 * TERRACE_HALF_EXTENT / *levels as f64;
            // Duplicate each riser abscissa so the vertical face is emitted
            // rather than smeared into a ramp. The +/- epsilon is the
            // narrowest a riser can be and still be a riser on an f64 lattice.
            let mut out = Vec::new();
            for k in 1..*levels {
                let x = -TERRACE_HALF_EXTENT + k as f64 * width;
                out.push(x - 1e-9);
                out.push(x);
            }
            out
        }
        _ => Vec::new(),
    }
}
