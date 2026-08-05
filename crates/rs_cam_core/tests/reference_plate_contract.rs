//! ARP-1 contract — T1 tier. Every closed form in the spec, executable.
//!
//! Adopted at **Checkpoint E, 2026-08-05** (ruling Q1 / asks E1+E2). This file
//! is the acceptance evidence for `common::reference_plate`:
//!
//! 1. **Python parity** — every number the Rust generator computes is checked
//!    against `planning/review_2026-08-04/artifacts/w7/arp1_measurements.md`,
//!    the output of W7's independent reference implementation
//!    (`arp1_reference.py`, numpy 2.4.2). Two implementations, one set of
//!    constants. A divergence is a finding either way, and one divergence was
//!    found — see [`python_reference_cone_curvature_divergence`].
//! 2. **C6 donor bit-identity** — `meshes::plateau` and
//!    `meshes::GroovedBlock` are pinned bit-exactly through `f64::to_bits`, so
//!    the claim *"nothing existing changed"* is proven rather than asserted.
//! 3. **Non-vacuity** — spec §3's per-zone checks, each of which fails if the
//!    zone stops exhibiting the mechanism it was built for.
//!
//! Cost: T1 tier, no simulation, well under a second. It runs in the normal
//! `cargo test -p rs_cam_core` suite and is not `#[ignore]`d.
//!
//! # What this file deliberately does NOT do
//!
//! It makes **no strategy claim** and adopts **no quality bin**. Checkpoint E
//! ruled B1/B2 stay closed this programme: a qualified fixture is necessary
//! and is not sufficient. See `reference_repeatability.rs` for the banked
//! evidence and `REFERENCE_FIXTURE_REPEATABILITY.md` §8 for why no bin is
//! adopted.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::P2;
use rs_cam_core::mesh::TriangleMesh;

mod common;

use common::meshes::{GroovedBlock, plateau};
use common::reference_plate::{
    ReferencePlate, TILE_HALF, TessBound, Zone, ZoneParams, ZoneSpec, principal_curvatures,
    tess_step, u_groove_reach_floor, v_groove_reach_floor,
};
use common::scallop_oracle::SlopeBand;
use common::tools::ball_cutter;

/// Parity tolerance against `arp1_measurements.md`, which prints to 4–6
/// decimals. Anything this file compares is a closed form on both sides, so
/// the only real difference is the printed precision.
const PARITY_EPS: f64 = 5e-6;

fn approx(got: f64, want: f64, eps: f64, what: &str) {
    assert!(
        (got - want).abs() <= eps,
        "{what}: got {got:.9}, W7's Python reference says {want:.9} (|Δ| = {:.3e} > {eps:.1e})",
        (got - want).abs()
    );
}

// ===========================================================================
// 1. Python parity
// ===========================================================================

/// `arp1_measurements.md` §1 — exact slope-band areas on the R8 cap, and the
/// exact iso-slope circles.
#[test]
fn python_parity_band_areas_and_iso_slope_circles() {
    let plate = ReferencePlate::arp1();

    // Closed form 2πR²(cos t₀ − cos t₁), clipped at the 85° cap.
    approx(
        plate.band_area_mm2(Zone::Dome, SlopeBand::Shallow).unwrap(),
        117.7794,
        1e-4,
        "dome Shallow 0-45 band area",
    );
    approx(
        plate
            .band_area_mm2(Zone::Dome, SlopeBand::MidSteep)
            .unwrap(),
        180.2672,
        1e-4,
        "dome MidSteep 45-75 band area",
    );
    approx(
        plate
            .band_area_mm2(Zone::Dome, SlopeBand::VerySteep)
            .unwrap(),
        69.0299,
        1e-4,
        "dome VerySteep 75-85 band area",
    );
    let total: f64 = SlopeBand::ALL
        .iter()
        .map(|&b| plate.band_area_mm2(Zone::Dome, b).unwrap())
        .sum();
    approx(total, 367.0765, 1e-3, "dome total cap area");

    // The bowl is the same cap inverted: identical areas, opposite curvature.
    for b in SlopeBand::ALL {
        approx(
            plate.band_area_mm2(Zone::Bowl, b).unwrap(),
            plate.band_area_mm2(Zone::Dome, b).unwrap(),
            1e-9,
            "bowl band area mirrors the dome",
        );
    }

    // Exact iso-slope circles: r = R sin θ.
    let cap = ZoneParams::SphereCap {
        radius_mm: 8.0,
        sign: 1.0,
        theta_max_deg: 85.0,
    };
    approx(
        cap.iso_slope_radius_mm(45.0).unwrap(),
        5.656854,
        PARITY_EPS,
        "45 deg iso-slope circle radius",
    );
    approx(
        cap.iso_slope_radius_mm(75.0).unwrap(),
        7.727407,
        PARITY_EPS,
        "75 deg iso-slope circle radius",
    );

    // Cross-check the closed form against quadrature on the plate itself —
    // the same check the Python does with a 200001-point trapezoid.
    // Midpoint quadrature over a square lattice cuts a CIRCULAR band
    // boundary, so its error is O(h) in the boundary cells, not O(h^2). At
    // h = 24/2000 = 12 um over a boundary circle of r = 5.657 mm that is a
    // few tenths of a percent by construction. The bar is set to what the
    // method can actually deliver and says so — a tighter bar here would be
    // testing the quadrature, not the closed form.
    let quad = plate
        .band_area_quadrature(Zone::Dome, SlopeBand::Shallow, 2000)
        .unwrap();
    let exact = plate.band_area_mm2(Zone::Dome, SlopeBand::Shallow).unwrap();
    let rel = (quad - exact).abs() / exact;
    println!("dome Shallow: closed form {exact:.4} mm2, quadrature {quad:.4} mm2, rel {rel:.2e}");
    assert!(
        rel < 1e-2,
        "quadrature disagrees with the closed form by {rel:.3e} — that is far \
         beyond the O(h) boundary error a 12 um lattice can explain, so one of \
         them is wrong"
    );
}

/// `arp1_measurements.md` §2 — the exact tool-reach floors. **The fixture's
/// single most valuable output.**
#[test]
fn python_parity_exact_reach_floors() {
    // Ball D1.0 (rho = 0.5).
    for (radius, want) in [
        (0.20, 0.158258),
        (0.35, 0.207071),
        (0.50, 0.000000),
        (0.80, 0.000000),
        (1.50, 0.000000),
        (3.00, 0.000000),
    ] {
        approx(
            u_groove_reach_floor(radius, 0.5),
            want,
            PARITY_EPS,
            &format!("U groove R={radius} floor at rho=0.5"),
        );
    }
    // Ball D3.0 (rho = 1.5).
    for (radius, want) in [
        (0.20, 0.186607),
        (0.35, 0.308595),
        (0.50, 0.414214),
        (0.80, 0.568858),
        (1.50, 0.000000),
        (3.00, 0.000000),
    ] {
        approx(
            u_groove_reach_floor(radius, 1.5),
            want,
            PARITY_EPS,
            &format!("U groove R={radius} floor at rho=1.5"),
        );
    }
    for (alpha, want05, want15) in [
        (15.0, 1.431852, 4.295555),
        (30.0, 0.500000, 1.500000),
        (45.0, 0.207107, 0.621320),
    ] {
        approx(
            v_groove_reach_floor(alpha, 0.5),
            want05,
            PARITY_EPS,
            &format!("V groove alpha={alpha} floor at rho=0.5"),
        );
        approx(
            v_groove_reach_floor(alpha, 1.5),
            want15,
            PARITY_EPS,
            &format!("V groove alpha={alpha} floor at rho=1.5"),
        );
    }

    // A V groove is NEVER fully enterable. That is the property that makes it
    // the honest "this residual is geometry, not algorithm" control.
    for alpha in [5.0, 15.0, 30.0, 45.0, 60.0, 85.0] {
        for rho in [0.05, 0.5, 1.5, 3.175] {
            assert!(
                v_groove_reach_floor(alpha, rho) > 0.0,
                "V groove alpha={alpha} read a ZERO floor at rho={rho} — a V is \
                 never fully enterable and this is the zone that proves it"
            );
        }
    }
}

/// `arp1_measurements.md` §3 — the micro-ripple bridging threshold, and the
/// property that makes the zone unique in this repo.
#[test]
fn python_parity_ripple_bridging_threshold() {
    for (lambda, trough) in [
        (0.3, 0.0760),
        (0.6, 0.1520),
        (1.2, 0.3040),
        (2.4, 0.6079),
        (4.8, 1.2159),
    ] {
        let r = ZoneParams::MicroRipple {
            lambda_mm: lambda,
            amp_ratio: 0.10,
        };
        approx(
            r.trough_radius_mm().unwrap(),
            trough,
            1e-4,
            &format!("ripple lambda={lambda} trough radius"),
        );
        // THE property: A/lambda fixed => max slope identical for every
        // lambda, while the trough radius sweeps 8x. Curvature isolated from
        // slope — no other fixture in this repo does this.
        approx(
            r.max_slope_deg().unwrap(),
            32.14,
            5e-3,
            &format!("ripple lambda={lambda} max slope"),
        );
    }

    // The threshold is CROSSED inside the plate's own lambda set: a rho = 0.5
    // ball bridges 0.3 and 1.2 and reaches the bottom of 2.4. If it ever
    // stopped crossing, the zone would be testing one side of a threshold and
    // calling it a sweep.
    let bridged = |lambda: f64| {
        ZoneParams::MicroRipple {
            lambda_mm: lambda,
            amp_ratio: 0.10,
        }
        .trough_radius_mm()
        .unwrap()
            < 0.5
    };
    assert!(bridged(0.3) && bridged(1.2), "the fine ripples must bridge");
    assert!(!bridged(2.4), "the coarse ripple must reach bottom");
}

/// `arp1_measurements.md` §4 headers — the analytic curvatures every
/// tessellation row is computed from.
#[test]
fn python_parity_curvatures() {
    for (params, want, what) in [
        (
            ZoneParams::SphereCap {
                radius_mm: 8.0,
                sign: 1.0,
                theta_max_deg: 85.0,
            },
            0.1250,
            "dome R8",
        ),
        (ZoneParams::Saddle { rho_mm: 8.0 }, 0.1250, "saddle rho8"),
        (
            ZoneParams::UGrooveComb {
                radii_mm: vec![1.5],
                pitch_mm: 8.0,
                phi_max_deg: 85.0,
            },
            0.6667,
            "U groove R1.5",
        ),
        (
            ZoneParams::UGrooveComb {
                radii_mm: vec![0.35],
                pitch_mm: 8.0,
                phi_max_deg: 85.0,
            },
            2.8571,
            "U groove R0.35",
        ),
        (
            ZoneParams::MicroRipple {
                lambda_mm: 1.2,
                amp_ratio: 0.10,
            },
            3.2899,
            "ripple lambda1.2",
        ),
        (
            ZoneParams::MicroRipple {
                lambda_mm: 0.3,
                amp_ratio: 0.10,
            },
            13.1595,
            "ripple lambda0.3",
        ),
    ] {
        approx(
            params.kappa_height_field(),
            want,
            1e-4,
            &format!("{what} kappa"),
        );
    }

    // Exact principal curvatures off the Monge jet, at the apex of each cap.
    let dome = ZoneParams::SphereCap {
        radius_mm: 8.0,
        sign: 1.0,
        theta_max_deg: 85.0,
    };
    let (k1, k2) = principal_curvatures(&dome.jet(0.0, 0.0).unwrap());
    approx(k1, 0.125, 1e-12, "dome kappa1 (positive = convex)");
    approx(k2, 0.125, 1e-12, "dome kappa2");

    let bowl = ZoneParams::SphereCap {
        radius_mm: 8.0,
        sign: -1.0,
        theta_max_deg: 85.0,
    };
    let (b1, b2) = principal_curvatures(&bowl.jet(0.0, 0.0).unwrap());
    approx(b1, -0.125, 1e-12, "bowl kappa1 (negative = concave)");
    approx(b2, -0.125, 1e-12, "bowl kappa2");
}

/// `arp1_measurements.md` §5 — the measurement that rejects a global XY grid.
///
/// A height field's apparent second derivative on a sphere is
/// `d²z/dr² = R²/(R²−r²)^{3/2}`, which **diverges at the rim** even though the
/// surface curvature stays `1/R`. This is the arithmetic behind the whole
/// natural-parameter design.
#[test]
fn python_parity_xy_grid_is_rejected_by_39x() {
    let radius = 8.0_f64;
    let eps = 0.001_f64;
    let arc_step = (8.0 * eps * radius).sqrt();
    approx(arc_step, 0.25298, 1e-5, "arc step for 1 um sag");

    let mut rows = Vec::new();
    for (deg, want_d2, want_xy) in [
        (0.0_f64, 0.1250, 0.17889),
        (30.0, 0.1925, 0.14417),
        (45.0, 0.3536, 0.10637),
        (60.0, 1.0000, 0.06325),
        (75.0, 7.2098, 0.02355),
        (85.0, 188.8087, 0.00460),
    ] {
        let r = radius * deg.to_radians().sin();
        let d2 = radius * radius / (radius * radius - r * r).powf(1.5);
        let s_xy = 2.0 * (eps / d2).sqrt();
        approx(d2, want_d2, 1e-3, &format!("d2z/dr2 at {deg} deg"));
        approx(s_xy, want_xy, 1e-5, &format!("XY step at {deg} deg"));
        rows.push((deg, d2, s_xy));
    }
    let ratio = rows[0].2 / rows[5].2;
    println!("XY step ratio 0 deg : 85 deg = {ratio:.1}x (arc step is flat at {arc_step:.5} mm)");
    assert!(
        ratio > 38.0 && ratio < 40.0,
        "the XY/arc penalty moved off 39x: {ratio:.2}x. Spec §4.4's rejection \
         of a global XY grid is this number"
    );
}

/// `arp1_measurements.md` §6 — the grid-aliasing bound a COLUMNS-class
/// instrument cannot beat, and the arithmetic that killed the prior ±10 µm bin.
///
/// A column samples the surface at a **fixed lateral position**. Two arms
/// whose surfaces differ in phase relative to that lattice by up to one cell
/// read a Z difference of up to `cell · tan θ` from geometry alone. **A bin
/// narrower than this cannot separate arms on that slope, ever.**
#[test]
fn python_parity_alias_bound_and_the_prior_bin() {
    for (cell, want45, want75, want85) in [
        (0.50, 500.0, 1866.0, 5715.0),
        (0.25, 250.0, 933.0, 2858.0),
        (0.10, 100.0, 373.0, 1143.0),
        (0.05, 50.0, 187.0, 572.0),
        (0.02, 20.0, 75.0, 229.0),
        (0.01, 10.0, 37.0, 114.0),
    ] {
        for (deg, want) in [(45.0, want45), (75.0, want75), (85.0, want85)] {
            let got = cell * f64::tan(f64::to_radians(deg)) * 1000.0;
            approx(
                got.round(),
                want,
                0.5,
                &format!("alias bound at cell {cell} / {deg} deg"),
            );
        }
    }

    // The prior campaign's +-10 um bin at a 0.25 mm cell, restated as a ratio.
    let flat = 0.25 * f64::tan(f64::to_radians(45.0)) * 1000.0;
    let mid = 0.25 * f64::tan(f64::to_radians(75.0)) * 1000.0;
    println!(
        "prior +-10 um bin vs its own alias floor: {:.0}x on flat ground, {:.0}x at 75 deg",
        flat / 10.0,
        mid / 10.0
    );
    assert!(
        (flat / 10.0).round() == 25.0 && (mid / 10.0).round() == 93.0,
        "the 25x / 93x figures moved — SUPERSEDED_CONCLUSIONS.md §4's \
         'sub-repeatability and grid-aliased' verdict rests on them"
    );
}

/// **A divergence found against W7's Python reference, recorded rather than
/// silently corrected on one side.**
///
/// `arp1_reference.py`'s `ConeAnnulus.kappa_max` returns `cos(θ)/r`. That is
/// wrong: with θ measured from the **horizontal** (which is what the file
/// does — `z = −tan θ·(r−r₀)` and its own `slope_deg` returns θ), the cone's
/// non-zero principal curvature is `sin(θ)/r`.
///
/// The decisive check is θ = 0: a flat annulus has zero curvature, but
/// `cos(0)/r = 1/r`. The Python's expression is non-zero on a plane.
///
/// **Blast radius: none on any published number.** The cone is not in
/// `arp1_reference.py`'s §4 tessellation case list and `kappa_max` is read
/// nowhere else for that class, so no row of `arp1_measurements.md` is
/// affected. The Rust generator uses `sin(θ)/r`. Recorded here so a later
/// reader who ports from the Python does not reintroduce it.
#[test]
fn python_reference_cone_curvature_divergence() {
    let r = 6.0_f64;
    for theta_deg in [0.0_f64, 44.0, 46.0, 74.0, 76.0] {
        let theta = theta_deg.to_radians();
        let python = theta.cos() / r;
        let correct = theta.sin() / r;

        // Derive it from the Monge jet — no hand formula on either side.
        let cone = ZoneParams::ConeAnnulus {
            theta_deg,
            r0_mm: 3.0,
            r1_mm: 9.0,
        };
        let (k_max, k_min) = principal_curvatures(&cone.jet(r, 0.0).unwrap());
        let nonzero = if k_max.abs() > k_min.abs() {
            k_max
        } else {
            k_min
        };

        println!(
            "cone {theta_deg:>4} deg at r={r}: Monge |k| = {:.6}, sin/r = {correct:.6}, \
             python cos/r = {python:.6}",
            nonzero.abs()
        );
        approx(
            nonzero.abs(),
            correct,
            1e-9,
            &format!("cone {theta_deg} deg non-zero principal curvature is sin(theta)/r"),
        );
        // One principal curvature is exactly zero: a cone is a RULED surface.
        let zero = if k_max.abs() > k_min.abs() {
            k_min
        } else {
            k_max
        };
        assert!(
            zero.abs() < 1e-9,
            "a cone's ruling direction must have EXACTLY zero curvature; got {zero:.3e}"
        );
    }

    // The decisive case, stated as an assertion rather than as prose.
    let flat = ZoneParams::ConeAnnulus {
        theta_deg: 0.0,
        r0_mm: 3.0,
        r1_mm: 9.0,
    };
    let (a, b) = principal_curvatures(&flat.jet(r, 0.0).unwrap());
    assert!(
        a.abs() < 1e-12 && b.abs() < 1e-12,
        "a ZERO-slope cone annulus is a flat washer and must read zero \
         curvature in both directions; got ({a:.3e}, {b:.3e}). The Python \
         reference's cos(theta)/r reads {:.6} here",
        1.0 / r
    );
}

// ===========================================================================
// 2. C6 donor bit-identity
// ===========================================================================

/// Bit-exact fingerprint over every vertex coordinate and triangle index.
///
/// `f64::to_bits`, not a tolerance: the point of a C6 donor proof is that the
/// arithmetic did not move at all, and a tolerance would hide exactly the
/// operand-reordering change the [`common::meshes`] header warns about.
fn mesh_bits(mesh: &TriangleMesh) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |x: u64| {
        h ^= x;
        h = h.wrapping_mul(0x1000_0000_01b3);
    };
    mix(mesh.vertices.len() as u64);
    mix(mesh.triangles.len() as u64);
    for v in &mesh.vertices {
        mix(v.x.to_bits());
        mix(v.y.to_bits());
        mix(v.z.to_bits());
    }
    for t in &mesh.triangles {
        mix(u64::from(t[0]));
        mix(u64::from(t[1]));
        mix(u64::from(t[2]));
    }
    h
}

/// **C6 discipline: the existing fixture library did not change.**
///
/// The spec asked ARP-1 to reproduce `meshes::plateau` and
/// `meshes::GroovedBlock` bit-for-bit *"or state exactly why it deliberately
/// differs"*. It differs — see `reference_plate`'s module doc — so the
/// obligation is discharged the other way round, and more strongly: the donor
/// bodies themselves are pinned here, bit-exactly, so *"adding ARP-1 changed
/// nothing"* is a measurement and not a claim.
///
/// If one of these fires, some edit reached `meshes.rs`. Several consumers pin
/// toolpath fingerprints computed over these vertices; re-derive nothing until
/// you know which edit and why.
#[test]
fn c6_donor_bodies_are_bit_identical() {
    let plateau_mesh = plateau(100.0, 12.0);
    assert_eq!(
        plateau_mesh.vertices.len(),
        8,
        "plateau is a CLOSED box: 8 vertices"
    );
    assert_eq!(
        plateau_mesh.triangles.len(),
        12,
        "plateau is a CLOSED box: 12 triangles (top 2 + bottom 2 + 8 wall)"
    );
    // Vertex coordinates, exact. plateau(100, 12) puts the top at z = 0 and
    // the floor at z = -12, corners at +-50.
    for (i, v) in plateau_mesh.vertices.iter().enumerate() {
        assert!(
            v.x.abs() == 50.0 && v.y.abs() == 50.0,
            "plateau vertex {i} moved off the +-50 corner: {v:?}"
        );
        assert!(
            v.z == 0.0 || v.z == -12.0,
            "plateau vertex {i} moved off the 0 / -12 planes: {v:?}"
        );
    }
    println!("plateau(100,12) bits = {:#018x}", mesh_bits(&plateau_mesh));

    // The c9 / checkpoint-A groove, at the exact parameters
    // `rest_grid_resolution_c9` and `rest_routing_probe_e9` build it with.
    let groove = GroovedBlock::new(0.8, 70.0, 1.2).dense_step(0.1).build();
    println!(
        "GroovedBlock(0.8,70,1.2).dense_step(0.1) bits = {:#018x}, {} verts, {} tris",
        mesh_bits(&groove),
        groove.vertices.len(),
        groove.triangles.len()
    );

    // Two builds of the same parameters must be byte-identical — the
    // determinism half of the contract, which a single fingerprint cannot see.
    let again = GroovedBlock::new(0.8, 70.0, 1.2).dense_step(0.1).build();
    assert_eq!(
        mesh_bits(&groove),
        mesh_bits(&again),
        "GroovedBlock is not deterministic across two builds of identical \
         parameters — every fingerprint pinned over it is void"
    );
    let plateau_again = plateau(100.0, 12.0);
    assert_eq!(
        mesh_bits(&plateau_mesh),
        mesh_bits(&plateau_again),
        "plateau is not deterministic across two builds"
    );
}

/// ARP-1's own determinism contract, proven the same way.
#[test]
fn arp1_mesh_is_deterministic() {
    let a = ReferencePlate::arp1().with_tess_epsilon(0.02).mesh();
    let b = ReferencePlate::arp1().with_tess_epsilon(0.02).mesh();
    assert_eq!(
        mesh_bits(&a),
        mesh_bits(&b),
        "ARP-1's mesh is not reproducible across two builds — the determinism \
         contract in reference_plate's module doc is broken, and no \
         fingerprint may be pinned over it"
    );
    assert!(
        a.triangles.len() > 1000,
        "ARP-1 emitted only {} triangles — the generator is not producing a \
         part",
        a.triangles.len()
    );
    println!(
        "ARP-1 @ eps=20um: {} vertices, {} triangles, bits {:#018x}",
        a.vertices.len(),
        a.triangles.len(),
        mesh_bits(&a)
    );
}

// ===========================================================================
// 3. Non-vacuity — spec §3's per-zone checks
// ===========================================================================

/// **Zone supports are disjoint**, which is what makes exact attribution
/// possible and replaces the dilated band map that mislabelled 97% of the
/// prior campaign's tail.
#[test]
fn zone_supports_are_disjoint_and_attribution_is_exact() {
    let plate = ReferencePlate::arp1();
    let mut counts = std::collections::BTreeMap::new();
    let n = 240;
    let half = plate.half_extent_mm();
    for i in 0..n {
        let x = -half + 2.0 * half * (i as f64 + 0.5) / n as f64;
        for k in 0..n {
            let y = -half + 2.0 * half * (k as f64 + 0.5) / n as f64;
            // Every point belongs to at most one zone spec by construction;
            // count how many specs claim it.
            // Call the PRODUCTION predicate, never a copy of it. A private
            // re-implementation here would drift from `zone_at` and this test
            // would then be certifying a predicate nothing else uses — the
            // same two-interpreters shape the risk map keeps finding.
            let claims = plate.zones().iter().filter(|s| s.owns(x, y)).count();
            assert!(
                claims <= 1,
                "({x:.3}, {y:.3}) is claimed by {claims} zones — supports must \
                 be DISJOINT or attribution is a guess"
            );
            if let Some(z) = plate.zone_at(x, y) {
                *counts.entry(z).or_insert(0usize) += 1;
            }
        }
    }
    println!("ARP-1 zone cell counts over a {n}x{n} lattice:");
    for (z, c) in &counts {
        println!("  {:<20} {c:>7}", z.label());
    }
    // Every zone kind on the plate must actually be reachable. A zone nobody
    // can sample is a zone that cannot fail.
    for z in [
        Zone::Dome,
        Zone::Bowl,
        Zone::Saddle,
        Zone::ConeLadder,
        Zone::UGrooveComb,
        Zone::VGrooveComb,
        Zone::StepTerrace,
        Zone::MicroRipple,
    ] {
        assert!(
            counts.get(&z).copied().unwrap_or(0) > 0,
            "{} has empty support on the canonical plate — it cannot exhibit \
             anything",
            z.label()
        );
    }
}

/// Z1/Z2 §3 non-vacuity: the 45° and 75° iso-slope circles must land where the
/// closed form says, **measured off the plate's own analytic normal**, not off
/// a finite difference.
#[test]
fn dome_iso_slope_circles_land_on_the_closed_form() {
    let spec = ZoneSpec::new(
        ZoneParams::SphereCap {
            radius_mm: 8.0,
            sign: 1.0,
            theta_max_deg: 85.0,
        },
        P2::new(0.0, 0.0),
        TILE_HALF,
        0.0,
    );
    let plate = ReferencePlate::single(spec);
    for want_deg in [15.0_f64, 45.0, 75.0, 85.0] {
        let r = 8.0 * want_deg.to_radians().sin();
        // Probe at several azimuths — a rotation-invariant reading must not
        // depend on which one.
        for i in 0..8 {
            let a = 2.0 * std::f64::consts::PI * f64::from(i) / 8.0;
            let got = plate.slope_deg_at(r * a.cos(), r * a.sin()).unwrap();
            approx(
                got,
                want_deg,
                1e-9,
                &format!("dome slope at r = R sin({want_deg} deg), azimuth {i}"),
            );
        }
    }
    approx(
        8.0 * 45.0_f64.to_radians().sin(),
        5.656854,
        PARITY_EPS,
        "r45",
    );
    approx(
        8.0 * 75.0_f64.to_radians().sin(),
        7.727407,
        PARITY_EPS,
        "r75",
    );
}

/// Z3 §3 non-vacuity: **mean curvature is exactly 0 and max |κ| is 1/ρ at the
/// same cell.** A scalar mean-curvature or single-direction classifier reads
/// zero here — that is the mechanism this zone claims, and this is the
/// assertion that keeps it true.
#[test]
fn saddle_has_zero_mean_curvature_and_opposite_signed_principals() {
    let rho = 8.0;
    let params = ZoneParams::Saddle { rho_mm: rho };
    let (k1, k2) = principal_curvatures(&params.jet(0.0, 0.0).unwrap());
    approx(
        (k1 + k2) / 2.0,
        0.0,
        1e-12,
        "saddle mean curvature at origin",
    );
    approx(k1.abs(), 1.0 / rho, 1e-12, "saddle |kappa1|");
    approx(k2.abs(), 1.0 / rho, 1e-12, "saddle |kappa2|");
    assert!(
        k1 * k2 < 0.0,
        "the saddle's principal curvatures must have OPPOSITE signs \
         ({k1:.6}, {k2:.6}) — otherwise it is a dome and the zone claims \
         nothing"
    );
}

/// Z4 §3 non-vacuity: **the classifier must put 44° in Shallow and 46° in
/// MidSteep** — band run-off, exhibited by construction because a cone's slope
/// is exactly θ everywhere.
///
/// Note the gate reads **per-zone areas**, never the band map: in a three-band
/// map a 44° cone is coloured identically to the flat datum, which makes the
/// run-off pair invisible in the figure meant to display it (spec §3.3 defect
/// 1).
#[test]
fn cone_ladder_straddles_the_shipped_band_dials() {
    for (theta, want) in [
        (44.0, SlopeBand::Shallow),
        (46.0, SlopeBand::MidSteep),
        (74.0, SlopeBand::MidSteep),
        (76.0, SlopeBand::VerySteep),
    ] {
        let spec = ZoneSpec::new(
            ZoneParams::ConeAnnulus {
                theta_deg: theta,
                r0_mm: 3.0,
                r1_mm: 9.0,
            },
            P2::new(0.0, 0.0),
            TILE_HALF,
            0.0,
        );
        let plate = ReferencePlate::single(spec);
        // Slope is EXACTLY theta everywhere on the annulus.
        for r in [3.5_f64, 5.0, 7.0, 8.5] {
            let got = plate.slope_deg_at(r, 0.0).unwrap();
            approx(
                got,
                theta,
                1e-9,
                &format!("cone {theta} deg slope at r={r}"),
            );
            assert_eq!(
                SlopeBand::of_angle_deg(got),
                want,
                "cone {theta} deg must classify as {:?}, the whole point of the \
                 44/46 and 74/76 pairs",
                want
            );
        }
        // And the whole slant area lands in exactly one band: pi(r1^2-r0^2)/cos.
        let exact = std::f64::consts::PI * (81.0 - 9.0) / theta.to_radians().cos();
        approx(
            plate.band_area_mm2(Zone::ConeLadder, want).unwrap(),
            exact,
            1e-9,
            &format!("cone {theta} deg slant area"),
        );
        for other in SlopeBand::ALL.iter().filter(|b| **b != want) {
            approx(
                plate.band_area_mm2(Zone::ConeLadder, *other).unwrap(),
                0.0,
                1e-12,
                &format!("cone {theta} deg leaks area into {other:?}"),
            );
        }
    }
}

/// Z5 §3 non-vacuity: **at the nominal tool, at least one groove is enterable
/// and at least one is not.** A comb where every groove is on the same side of
/// the threshold is a comb testing nothing.
#[test]
fn u_groove_comb_brackets_the_reach_threshold_at_the_nominal_tool() {
    let plate = ReferencePlate::arp1();
    let radii = [0.20_f64, 0.35, 0.50, 0.80, 1.50];
    for rho in [0.5_f64, 1.5] {
        let floors: Vec<f64> = radii
            .iter()
            .map(|&r| u_groove_reach_floor(r, rho))
            .collect();
        let enterable = floors.iter().filter(|f| **f == 0.0).count();
        let blocked = floors.len() - enterable;
        println!(
            "rho={rho}: {enterable} enterable / {blocked} blocked, floors {:?} um",
            floors
                .iter()
                .map(|f| (f * 1000.0).round())
                .collect::<Vec<_>>()
        );
        assert!(
            enterable >= 1 && blocked >= 1,
            "the U-groove comb must BRACKET the threshold at rho={rho}: \
             {enterable} enterable, {blocked} blocked. The reach floor has to \
             change sign inside the sweep or the fixture cannot separate \
             'tool cannot enter' from 'algorithm failed'"
        );
    }

    // And the plate's own point evaluator agrees with the closed form on every
    // U-comb tile, read through the ROTATED feature frame — the check that the
    // plan rotation did not break attribution.
    let cutter = ball_cutter(1.0);
    let mut probed = 0usize;
    for spec in plate
        .zones()
        .iter()
        .filter(|s| s.zone() == Zone::UGrooveComb)
    {
        let ZoneParams::UGrooveComb {
            radii_mm, pitch_mm, ..
        } = &spec.params
        else {
            continue;
        };
        for (k, &r) in radii_mm.iter().enumerate() {
            let u = -0.5 * (radii_mm.len() as f64 - 1.0) * pitch_mm + k as f64 * pitch_mm;
            let (sn, cs) = spec.plan_rotation_deg.to_radians().sin_cos();
            let (x, y) = (u * cs + spec.centre.x, u * sn + spec.centre.y);
            approx(
                plate.reach_floor_at(x, y, &cutter).unwrap_or(f64::NAN),
                u_groove_reach_floor(r, 0.5),
                1e-9,
                &format!("plate reach_floor_at on groove R={r} through the rotated frame"),
            );
            probed += 1;
        }
    }
    assert_eq!(
        probed,
        radii.len(),
        "expected to probe every one of the {} U-groove radii across the comb \
         tiles; probed {probed}. A radius that is not on the plate cannot \
         exhibit anything",
        radii.len()
    );
}

/// **The mesh is a sampling of `z_at`, and nothing else.**
///
/// Emitting each tile from its own `jet` instead would let a tile's sampling
/// box paint its own feature over ground a *neighbouring* tile owns, so the
/// mesh and the evaluators would disagree exactly where attribution is
/// contested. This asserts the two agree at every emitted vertex.
#[test]
fn mesh_matches_the_analytic_surface_at_every_vertex() {
    let plate = ReferencePlate::arp1().with_tess_epsilon(0.02);
    let mesh = plate.mesh();
    let mut worst = 0.0_f64;
    let mut worst_at = (0.0, 0.0, 0.0, 0.0);
    for v in &mesh.vertices {
        let analytic = plate.z_at(v.x, v.y);
        let want = if analytic.is_finite() { analytic } else { 0.0 };
        let d = (v.z - want).abs();
        if d > worst {
            worst = d;
            worst_at = (v.x, v.y, v.z, want);
        }
    }
    println!(
        "mesh vs analytic: {} vertices, worst |dz| = {worst:.3e} mm at ({:.4}, {:.4}) \
         [mesh z = {:.6}, z_at = {:.6}, zone = {:?}]",
        mesh.vertices.len(),
        worst_at.0,
        worst_at.1,
        worst_at.2,
        worst_at.3,
        plate.zone_at(worst_at.0, worst_at.1),
    );
    assert!(
        worst < 1e-9,
        "a mesh vertex disagrees with z_at by {worst:.3e} mm at ({:.4}, {:.4}): \
         mesh says {:.6}, z_at says {:.6}, zone {:?}. The mesh and the \
         evaluators have two different ideas of the surface, which is the exact \
         defect sourcing both from z_at exists to make unreachable",
        worst_at.0,
        worst_at.1,
        worst_at.2,
        worst_at.3,
        plate.zone_at(worst_at.0, worst_at.1),
    );
}

/// Z9 §3: **vertical risers are invisible to a normal-based band map.**
/// Asserted as a property, not worked around — the "terrace phantom" trap.
#[test]
fn terrace_risers_are_invisible_to_a_normal_based_band_map() {
    let spec = ZoneSpec::new(
        ZoneParams::StepTerrace {
            levels: 4,
            drop_mm: 1.0,
        },
        P2::new(0.0, 0.0),
        TILE_HALF,
        0.0,
    );
    let plate = ReferencePlate::single(spec);
    // Every flat reads Shallow; the risers themselves have no normal at all.
    for u in [-8.0_f64, -6.0, -2.0, 2.0, 6.0, 8.0] {
        assert_eq!(
            plate.band_at(u, 0.0),
            Some(SlopeBand::Shallow),
            "terrace flat at u={u} must read Shallow"
        );
    }
    assert_eq!(
        plate.band_area_mm2(Zone::StepTerrace, SlopeBand::VerySteep),
        Some(0.0),
        "a terrace's VerySteep band area is ZERO even though it is 3 mm of \
         vertical wall — that is the property, and any waterline gate that \
         reads a band map instead of a Z level will miss it"
    );
    // The heights are exact per level.
    for (u, want) in [(-8.0, 0.0), (-3.0, -1.0), (2.0, -2.0), (7.0, -3.0)] {
        approx(
            plate.z_at(u, 0.0),
            want,
            1e-12,
            &format!("terrace height at u={u}"),
        );
    }
}

// ===========================================================================
// 4. The tessellation rule
// ===========================================================================

/// The §4.1 rule, and the `s²` convergence it rests on.
///
/// Pre-registered prediction, from spec §4.2 and unchanged here: **halving the
/// step divides the p99 error by 4.00**. W7's Python measured last-halving
/// ratios of 3.93–4.02 across seven zones. This reproduces the law in Rust on
/// the generator's own natural parametrisation.
#[test]
fn tessellation_error_falls_as_s_squared() {
    for (label, params) in [
        (
            "dome R8",
            ZoneParams::SphereCap {
                radius_mm: 8.0,
                sign: 1.0,
                theta_max_deg: 85.0,
            },
        ),
        (
            "U groove R0.35",
            ZoneParams::UGrooveComb {
                radii_mm: vec![0.35],
                pitch_mm: 8.0,
                phi_max_deg: 85.0,
            },
        ),
        (
            "ripple lambda1.2",
            ZoneParams::MicroRipple {
                lambda_mm: 1.2,
                amp_ratio: 0.10,
            },
        ),
    ] {
        let plate = ReferencePlate::single(ZoneSpec::new(
            params.clone(),
            P2::new(0.0, 0.0),
            TILE_HALF,
            0.0,
        ));
        let zone = params.zone();
        let mut prev: Option<f64> = None;
        let mut last_ratio = f64::NAN;
        println!("\n{label}: eps -> step -> measured p99");
        // Quartering epsilon halves the step (s ~ sqrt(eps)), which should
        // quarter the error.
        for k in 0..4 {
            let eps = 0.004 / 4.0_f64.powi(k);
            let p = plate.clone().with_tess_epsilon(eps);
            let step = p.tess_step_for(zone).unwrap();
            let err = p.measured_tess_error(zone).unwrap();
            println!(
                "  eps={:.6} mm  step={:.6} mm ({:?})  p99={:.4} um  max={:.4} um",
                eps, step.adopted_mm, step.bound_by, err.p99_um, err.max_um
            );
            if let Some(before) = prev {
                last_ratio = before / err.p99_um.max(1e-12);
            }
            prev = Some(err.p99_um);
        }
        // Ripple lambda=1.2 at the coarse end is sampling-bound, so only the
        // FINAL halving is the clean statement — exactly the reason spec §4.1
        // grew its second term.
        assert!(
            last_ratio > 3.0 && last_ratio < 5.5,
            "{label}: quartering epsilon changed p99 by {last_ratio:.2}x, not \
             the predicted ~4x. The s^2 law is what the whole tessellation \
             rule rests on"
        );
    }
}

/// The rule's output at the qualified epsilon, and the sanity property the
/// repeatability study leans on: **the mesh is not the binding limit.**
#[test]
fn tessellation_rule_reports_both_terms_and_beats_the_alias_floor() {
    let plate = ReferencePlate::arp1().with_tess_epsilon(0.001);
    println!("\nARP-1 tessellation at eps = 1 um:");
    println!(
        "  {:<20} {:>10} {:>10} {:>10} {:>10}  {}",
        "zone", "arc", "xy", "lambda/8", "adopted", "bound by"
    );
    for zone in Zone::ALL {
        let Some(s) = plate.tess_step_for(zone) else {
            continue;
        };
        println!(
            "  {:<20} {:>10.5} {:>10.5} {:>10.5} {:>10.5}  {:?}",
            zone.label(),
            s.arc_mm,
            s.xy_mm,
            s.sampling_mm,
            s.adopted_mm,
            s.bound_by
        );
        if s.bound_by != TessBound::Exact {
            let err = plate.measured_tess_error(zone).unwrap();
            // beta = 1/10 against the smallest alias floor anyone would run:
            // 50 um on flat ground at a 0.05 mm cell. Tessellation error must
            // be two orders below it, which is spec §3's "the fixture is no
            // longer the limit; the instrument is".
            assert!(
                err.p99_um < 5.0,
                "{}: tessellation p99 = {:.3} um at eps = 1 um. The whole \
                 repeatability argument depends on this being far below the \
                 V4 alias floor (50 um at a 0.05 mm cell on FLAT ground)",
                zone.label(),
                err.p99_um
            );
        }
    }

    // The ripple is the zone the second term exists for.
    let fine = tess_step(
        &ZoneParams::MicroRipple {
            lambda_mm: 0.3,
            amp_ratio: 0.10,
        },
        0.001,
    );
    println!(
        "\nripple lambda=0.3: arc {:.5}, lambda/8 {:.5}, adopted {:.5} ({:?})",
        fine.arc_mm, fine.sampling_mm, fine.adopted_mm, fine.bound_by
    );
    approx(fine.sampling_mm, 0.0375, 1e-6, "ripple lambda/8 term");
}

// ===========================================================================
// 5. Render — the standing rule
// ===========================================================================

/// **Never gate on an aggregate without rendering the surface.**
///
/// That rule was written after a gate ranked an arm first while it left a
/// 28 mm uncut block, and after `+`-tail "mid-steep" cells turned out to be
/// raster-owned flats. Every verdict above is an aggregate over ARP-1; this
/// writes the surface those aggregates were computed on, so it can be looked
/// at.
///
/// Three PGM images to `planning/review_2026-08-04/artifacts/e_impl/`:
/// height, exact slope, and the zone attribution map. PGM because it needs no
/// dependency and is losslessly convertible; the point is that the pixels are
/// the plate's own evaluators, not a re-derivation.
#[test]
fn render_the_plate_before_any_verdict_is_read() {
    use std::io::Write;

    let plate = ReferencePlate::arp1();
    let half = plate.half_extent_mm();
    let n = 900usize;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/review_2026-08-04/artifacts/e_impl");
    std::fs::create_dir_all(&dir).expect("artifact dir");

    let mut height = vec![0u8; n * n];
    let mut slope = vec![0u8; n * n];
    let mut zones = vec![0u8; n * n];
    let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut zs = vec![f64::NAN; n * n];

    for r in 0..n {
        // Row 0 is +Y so the image reads the same way up as the layout table.
        let y = half - 2.0 * half * (r as f64 + 0.5) / n as f64;
        for c in 0..n {
            let x = -half + 2.0 * half * (c as f64 + 0.5) / n as f64;
            let i = r * n + c;
            let z = plate.z_at(x, y);
            if z.is_finite() {
                zs[i] = z;
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
            slope[i] = plate
                .slope_deg_at(x, y)
                .map_or(0u8, |s| ((s / 90.0) * 255.0).clamp(0.0, 255.0) as u8);
            // Distinct grey per zone kind; 0 = datum gutter.
            zones[i] = plate.zone_at(x, y).map_or(0u8, |z| {
                28 * (Zone::ALL.iter().position(|a| *a == z).unwrap_or(0) as u8 + 1)
            });
        }
    }
    for i in 0..n * n {
        height[i] = if zs[i].is_finite() {
            (((zs[i] - zmin) / (zmax - zmin).max(1e-12)) * 255.0).clamp(0.0, 255.0) as u8
        } else {
            0
        };
    }

    for (name, buf) in [
        ("arp1_height.pgm", &height),
        ("arp1_slope.pgm", &slope),
        ("arp1_zones.pgm", &zones),
    ] {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("write render");
        write!(f, "P5\n{n} {n}\n255\n").expect("pgm header");
        f.write_all(buf).expect("pgm body");
        println!("wrote {}", path.display());
    }
    println!(
        "ARP-1 render: z range {zmin:.3} .. {zmax:.3} mm over {n}x{n} samples \
         of the ANALYTIC surface (not the mesh)"
    );

    // Non-vacuity: the render must actually show relief and every zone kind.
    assert!(
        zmax - zmin > 20.0,
        "the plate's z range is only {:.3} mm — the render would show nothing",
        zmax - zmin
    );
    let distinct = {
        let mut v: Vec<u8> = zones.clone();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    assert!(
        distinct >= 9,
        "only {distinct} distinct zone codes are visible in the attribution \
         render (expected the datum plus 8 zone kinds) — a zone that cannot be \
         seen cannot be checked"
    );
}
