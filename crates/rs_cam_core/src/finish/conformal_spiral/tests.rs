//! Unit tests for the conformal-spiral research module. Moved out of
//! `finish/conformal_spiral.rs` by P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::{
    Flattening, SpiralParams, SpiralRefusal, SpiralReport, StallDistanceReference, Topology,
    blend_sigma, build_region_mesh, flatten_to_disk, measure_flattening, plan_spiral,
    region_topology,
};
use crate::finish::direction_field::all_triangles;
use crate::finish::scallop_math::{stepover_from_scallop_curved, stepover_from_scallop_flat};
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use std::f64::consts::TAU;

/// A triangulated disk: a centre vertex, `n_rings` concentric rings and a
/// fan/strip triangulation. Wound counter-clockwise seen from +Z, so the
/// single boundary loop runs counter-clockwise with the interior on its
/// left — the convention `region_topology` and the flip count assume.
fn disk_mesh(radius: f64, n_rings: usize, n_around: usize) -> TriangleMesh {
    let mut vertices = vec![P3::new(0.0, 0.0, 0.0)];
    for i in 1..=n_rings {
        let r = radius * (i as f64) / (n_rings as f64);
        for j in 0..n_around {
            let t = TAU * (j as f64) / (n_around as f64);
            vertices.push(P3::new(r * t.cos(), r * t.sin(), 0.0));
        }
    }
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for j in 0..n_around {
        let jn = (j + 1) % n_around;
        triangles.push([0, (1 + j) as u32, (1 + jn) as u32]);
    }
    for i in 0..(n_rings - 1) {
        let s = 1 + i * n_around;
        let ns = 1 + (i + 1) * n_around;
        for j in 0..n_around {
            let jn = (j + 1) % n_around;
            let (a, b) = ((s + j) as u32, (s + jn) as u32);
            let (c, d) = ((ns + j) as u32, (ns + jn) as u32);
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Run only the front half of the pipeline: submesh, topology, flattening,
/// flatten metrics. Used where the map itself is under test and the ring
/// search would only add runtime.
fn flatten_only(mesh: &TriangleMesh) -> (Topology, Flattening, SpiralReport) {
    let region = build_region_mesh(mesh, &all_triangles(mesh)).unwrap();
    let topo = region_topology(&region).unwrap();
    let params = SpiralParams::default();
    let mut report = SpiralReport {
        region_triangles: region.tris.len(),
        region_vertices: region.verts.len(),
        // Mirror plan_spiral's topology rows — the helper bypasses the
        // pipeline, so it must fill what the pipeline fills or the report
        // under test silently reads Default zeros.
        region_edges: topo.edge_count,
        euler_characteristic: topo.euler,
        boundary_loop_vertices: topo.loop_vertices.len(),
        boundary_loop_length_mm: topo.loop_length_mm,
        ..SpiralReport::default()
    };
    let flat = flatten_to_disk(&region, &topo, &params, &mut report).unwrap();
    measure_flattening(&region, &flat, &mut report);
    (topo, flat, report)
}

// -----------------------------------------------------------------
// SOURCE-2024 Eq. A-11
// -----------------------------------------------------------------

#[test]
fn sigma_has_the_a11_endpoint_and_monotonicity_properties() {
    let p = 2.0;
    assert!(blend_sigma(0.0, p).abs() < 1e-12, "σ(0) must be 0");
    assert!(
        (blend_sigma(TAU, p) - TAU).abs() < 1e-12,
        "σ(2π) must be 2π"
    );

    // σ'(0) = σ'(2π) = 0 — the property Eq. 9's tangency depends on.
    let dt = 1e-5;
    let d0 = (blend_sigma(dt, p) - blend_sigma(0.0, p)) / dt;
    let d1 = (blend_sigma(TAU, p) - blend_sigma(TAU - dt, p)) / dt;
    assert!(d0.abs() < 1e-3, "σ'(0) should vanish, got {d0}");
    assert!(d1.abs() < 1e-3, "σ'(2π) should vanish, got {d1}");

    // Strictly increasing across the interval.
    let mut prev = -1.0_f64;
    for k in 0..=200 {
        let t = TAU * (k as f64) / 200.0;
        let s = blend_sigma(t, p);
        assert!(s > prev - 1e-15, "σ must be monotone at t={t}");
        assert!((0.0..=TAU + 1e-12).contains(&s), "σ out of range at t={t}");
        prev = s;
    }

    // Higher p is admissible and keeps the endpoint conditions.
    for p in [2.0_f64, 3.0, 5.0] {
        assert!(blend_sigma(0.0, p).abs() < 1e-12);
        assert!((blend_sigma(TAU, p) - TAU).abs() < 1e-12);
        assert!((blend_sigma(PI_HALF_TURN, p) - TAU / 2.0).abs() < 1e-9);
    }
}

/// `σ(π) = π` for every `p`: A-12 gives `v(π) = 1/2`, so numerator and
/// denominator halve exactly.
const PI_HALF_TURN: f64 = std::f64::consts::PI;

// -----------------------------------------------------------------
// Disk map — closed forms
// -----------------------------------------------------------------

#[test]
fn flat_disk_flattens_to_the_exact_affine_map() {
    // Mean value coordinates **reproduce the point**: on a planar mesh
    // `v_i = Σ_j λ_ij v_j` is Floater's defining property, so the identity
    // (and any linear image of it) is the exact discrete solution. Equally
    // spaced boundary vertices on a circle get equal arc-length shares, so
    // the boundary data is exactly `v/ρ` and `z ↦ z/ρ` is the exact
    // solution. This closed form therefore survived the switch away from
    // cotangent weights unchanged — which is why it is still the control
    // reading. The tolerance is for Gauss–Seidel convergence, not for the
    // map: the solve stops at a `1e-9` sweep delta, and the error of an
    // iterate is that delta divided by `1 − ρ_GS`, a few times `1e-8` on a
    // fixture this size.
    let radius = 10.0_f64;
    let mesh = disk_mesh(radius, 8, 40);
    let region = build_region_mesh(&mesh, &all_triangles(&mesh)).unwrap();
    let (_topo, flat, report) = flatten_only(&mesh);

    assert_eq!(report.euler_characteristic, 1);
    assert_eq!(report.boundary_loop_vertices, 40);
    assert_eq!(report.flipped_triangles, 0);
    assert!(report.orientation_sign > 0.0);

    for v in 0..region.verts.len() {
        let p = region.point(v);
        let (u, w) = flat.uv[v];
        assert!(
            (u - p.x / radius).abs() < 5e-6 && (w - p.y / radius).abs() < 5e-6,
            "vertex {v}: disk map should be z/ρ, got ({u}, {w}) vs ({}, {})",
            p.x / radius,
            p.y / radius
        );
    }

    // Affine ⇒ constant area distortion (1/ρ²) and zero angle distortion.
    let expect = 1.0 / (radius * radius);
    assert!((report.area_distortion_median - expect).abs() < 1e-7);
    assert!(
        report.area_distortion_max - report.area_distortion_min < 1e-7,
        "area distortion should be constant, spread was {}",
        report.area_distortion_max - report.area_distortion_min
    );
    assert!(report.angle_distortion_max_deg < 1e-3);

    // A similarity has dilatation exactly 1: `J = (1/ρ)·R` for an
    // orthogonal `R`, so both singular values are `1/ρ`. This is the
    // control reading for the anisotropy instrument — if K ever drifts
    // off 1 here, the measurement is wrong, not the map.
    assert_eq!(report.dilatation_unmeasurable, 0);
    assert!(
        report.dilatation_min >= 1.0 - 1e-12,
        "K < 1 is impossible by construction, saw {}",
        report.dilatation_min
    );
    assert!(
        report.dilatation_max < 1.0 + 1e-5,
        "exact affine map must be conformal, saw K_max {}",
        report.dilatation_max
    );
}

/// The switch to mean-value weights exists to make folds structurally
/// impossible, so it gets tested on a triangulation built to be hostile.
///
/// `disk_mesh(10, 2, 64)` has only two mesh rings and 64 segments around,
/// so the strip triangles are extremely skewed: the angle at the inner
/// vertex is ~92.8°, i.e. **obtuse**, which is precisely the condition
/// that gives the cotangent Laplacian a *negative* edge weight and voids
/// Tutte's hypothesis for it. That the obtuse angles exist is asserted
/// here directly; what is **not** asserted is that the old weights would
/// have folded on this exact fixture, because establishing that would mean
/// keeping the retired solver alive purely to watch it fail.
#[test]
fn an_obtuse_triangulation_still_flattens_without_folds() {
    let radius = 10.0_f64;
    let mesh = disk_mesh(radius, 2, 64);

    // Precondition of the test: this fixture really is obtuse.
    let mut worst_deg = 0.0_f64;
    for tri in &mesh.triangles {
        let v: Vec<P3> = tri.iter().map(|&i| mesh.vertices[i as usize]).collect();
        for k in 0..3 {
            let (a, b, c) = (v[k], v[(k + 1) % 3], v[(k + 2) % 3]);
            let (e1, e2) = (b - a, c - a);
            let cos = e1.dot(&e2) / (e1.norm() * e2.norm());
            worst_deg = worst_deg.max(cos.clamp(-1.0, 1.0).acos().to_degrees());
        }
    }
    assert!(
        worst_deg > 91.0,
        "fixture is not obtuse enough to be the test it claims: worst angle {worst_deg}°"
    );

    let (_topo, flat, report) = flatten_only(&mesh);

    // The Tutte precondition, measured rather than assumed.
    assert_eq!(report.mean_value_weight_nonpositive, 0);
    assert!(
        report.mean_value_weight_min > 0.0,
        "min mean-value weight {} must be strictly positive",
        report.mean_value_weight_min
    );
    // ...and its consequence.
    assert_eq!(report.flipped_triangles, 0);
    assert!(report.flipped_triangle_disk_radii.is_empty());
    // Still an exact similarity, even on skewed triangles: dilatation is
    // a property of the map, not of the triangulation that carries it.
    assert!(report.dilatation_max < 1.0 + 1e-5);

    // The planar closed form still holds on a hostile triangulation:
    // mean value coordinates reproduce the identity regardless of shape.
    let region = build_region_mesh(&mesh, &all_triangles(&mesh)).unwrap();
    for v in 0..region.verts.len() {
        let p = region.point(v);
        let (u, w) = flat.uv[v];
        assert!(
            (u - p.x / radius).abs() < 5e-6 && (w - p.y / radius).abs() < 5e-6,
            "vertex {v}: expected z/ρ, got ({u}, {w})"
        );
    }
}

#[test]
fn all_boundary_region_solves_nothing_and_prescribes_everything() {
    // `make_test_flat` is two triangles and four vertices, every one of
    // them on the boundary. The interior system is empty; the solve must
    // report that honestly rather than iterating on nothing.
    let mesh = make_test_flat(100.0);
    let (topo, flat, report) = flatten_only(&mesh);

    assert_eq!(topo.loop_vertices.len(), 4);
    assert_eq!(report.euler_characteristic, 1);
    assert_eq!(report.flatten_interior_vertices, 0);
    // One sweep runs, touches nothing, and measures a zero delta.
    assert_eq!(report.flatten_solver_sweeps, 1);
    assert!(report.flatten_solver_delta.abs() < 1e-15);
    assert_eq!(report.mean_value_weight_nonpositive, 0);
    for (u, v) in &flat.uv {
        assert!(
            (u.hypot(*v) - 1.0).abs() < 1e-12,
            "every prescribed vertex lands on the unit circle"
        );
    }
    assert_eq!(report.flipped_triangles, 0);
}

// -----------------------------------------------------------------
// Refusals
// -----------------------------------------------------------------

#[test]
fn a_punched_interior_triangle_is_refused_as_not_simply_connected() {
    let mesh = disk_mesh(10.0, 4, 24);
    let index = SpatialIndex::build_auto(&mesh);
    let all = all_triangles(&mesh);
    // Triangle 0 is a centre-fan triangle: centre vertex plus two ring-1
    // vertices, none of them on the outer boundary. Removing a
    // boundary-adjacent triangle would only notch the outer loop and still
    // leave one loop, which is why the punch must be interior.
    let holed: Vec<u32> = all.iter().copied().filter(|&t| t != 0).collect();
    let params = SpiralParams::new(2.0, 0.15);
    let (report, outcome) = plan_spiral(&mesh, &index, &holed, &params);
    assert_eq!(
        outcome.unwrap_err(),
        SpiralRefusal::NotSimplyConnected { boundary_loops: 2 },
        "an annulus must be refused, not approximated: the slit map is F2 step 3"
    );
    // The report survives the refusal, filled as far as the pipeline got:
    // the submesh was built, so its rows are there; the flattening never
    // ran, so its rows are not.
    assert_eq!(report.region_triangles, all.len() - 1);
    assert!(report.area_distortion_by_disk_radius.is_empty());
    assert!(report.stall.is_none());
}

// -----------------------------------------------------------------
// Full pipeline — flat disk (exact map, closed-form spacing)
// -----------------------------------------------------------------

#[test]
fn flat_disk_spirals_at_the_flat_scallop_stepover() {
    let radius = 10.0_f64;
    let ball = 2.0_f64;
    let h = 0.15_f64;
    let mesh = disk_mesh(radius, 10, 48);
    let index = SpatialIndex::build_auto(&mesh);
    let params = SpiralParams {
        n_surface_samples: 4000,
        n_angular_samples: 120,
        ring_eps: 0.002,
        // DERIVED, not tuned. Two things have to be absorbed:
        //
        //   1. the paper's criterion has zero margin at every ring midline
        //      (see `SpiralParams::ring_spacing_safety`), so the centroid
        //      audit lands on a knife edge — at factor 1.0 the three
        //      centroid families within 0.003–0.049 mm of a band edge
        //      (k = 2, 5, 8 here) read as unmachined, 14.2 % of the region;
        //   2. the ring step overshoots `2·reach'` by the local radial
        //      SAMPLE gap. This fixture's facets are 1 mm and the inner
        //      strips carry one sample each, so the barycentric lattice
        //      leaves a sample-free annulus of 2/3 mm at every mesh vertex
        //      ring — the worst radial gap is 0.667 mm.
        //
        // Coverage survives while `2·reach·(1 − f) ≥ 0.667`, i.e.
        // `f ≤ 0.56`; and the reported adequacy condition below wants the
        // margin above `2 × max_local_sample_spacing_mm` = 0.722 mm, i.e.
        // `f ≤ 0.525`. 0.45 gives a 0.836 mm margin — 16 % headroom over
        // the tighter of the two. That is a big derate, and it is the
        // honest price of a fixture whose facets are 0.66× the stepover:
        // the same lesson the F2 phase-1 withdrawal recorded at instrument
        // scale, arriving here at unit scale.
        ring_spacing_safety: 0.45,
        // The paper sweeps in π/50 steps (100 candidates). On a disk every
        // start angle is equivalent by symmetry, so two candidates
        // exercise the sweep without paying for 100 rebuilds.
        start_angle_step: TAU / 2.0,
        ..SpiralParams::new(ball, h)
    };
    let expect_stepover = stepover_from_scallop_flat(ball, h);
    let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);
    let result = outcome.expect("simply-connected fixture must plan");

    // Map quality first: if the map were bad, every spacing number below
    // would be measuring the wrong thing.
    assert_eq!(report.flipped_triangles, 0);
    assert!(report.angle_distortion_max_deg < 1e-3);
    assert_eq!(report.ring_points_unlocated, 0);
    // Affine map ⇒ the radial profile is flat: every bucket carries the
    // same 1/ρ² distortion. This is the control reading for the profile
    // that a high-relief region is expected to make explode.
    assert_eq!(report.area_distortion_by_disk_radius.len(), 5);
    for b in &report.area_distortion_by_disk_radius {
        assert!(
            b.triangles > 0,
            "empty radial bucket {}..{}",
            b.r_lo,
            b.r_hi
        );
        assert!((b.area_distortion_median - 1.0 / (radius * radius)).abs() < 1e-7);
        // Isotropic everywhere, so the radial K profile is flat at 1.
        assert!(
            (b.dilatation_median - 1.0).abs() < 1e-5,
            "bucket {}..{} K_median {}",
            b.r_lo,
            b.r_hi,
            b.dilatation_median
        );
    }
    assert!(report.stall.is_none(), "coverage closed, so no stall block");

    // The adequacy condition, with the factor of 2 that makes it sound:
    // `max_local_sample_spacing_mm` is an isotropic in-triangle estimate,
    // but the gap that actually bites is RADIAL and spans a mesh vertex
    // ring, where no sample lands on either side — roughly twice the
    // in-triangle spacing. Below this the ring step quantises to the mesh's
    // feature pitch instead of following the scallop rule.
    assert!(
        report.ring_spacing_margin_mm > 2.0 * report.max_local_sample_spacing_mm,
        "safety margin {} mm must exceed 2× the coarsest local sample \
             spacing {} mm",
        report.ring_spacing_margin_mm,
        report.max_local_sample_spacing_mm
    );

    // The decisive spacing row. On a similarity the local radial scale is
    // ρ at every point of every circle, so every ring's max/min is 1 —
    // meaning the worst-sector rule costs this map nothing. The bar is
    // 1.01 because the measurement's own floor is the finite difference
    // amplifying the Gauss–Seidel error: ~2.5e-8 over a 1e-4 probe step is
    // ~2.5e-4 relative, so 1.01 leaves ~20x headroom over the noise while
    // staying far below the multiples this row exists to detect.
    let anis = report
        .ring_anisotropy
        .as_ref()
        .expect("rings were placed, so anisotropy must be measured");
    assert_eq!(anis.rings_measured, report.ring_count);
    assert_eq!(anis.rings_unmeasurable, 0);
    assert_eq!(anis.per_ring_radial_scale_ratio.len(), report.ring_count);
    assert!(
        anis.worst_ratio < 1.01,
        "a similarity forces no over-cover; worst ring ratio {} at ring {}",
        anis.worst_ratio,
        anis.worst_ring
    );
    assert!(anis.median_ratio <= anis.worst_ratio);

    // Sampling covers the DOMAIN, not a budget: no triangle is invisible
    // to the coverage predicate. Zero here is by construction, and it is
    // measured because this exact class of defect reads as success.
    assert_eq!(report.triangles_without_samples, 0);
    assert!((report.largest_unsampled_triangle_area_mm2).abs() < 1e-12);
    assert!(report.samples_placed >= report.samples_requested);
    assert!(report.samples_placed >= report.region_triangles);
    // The adequacy rule that is aware of apportionment: the coarsest
    // local sample spacing anywhere must sit under half the stepover.
    assert!(
        report.max_local_sample_spacing_mm < 0.5 * expect_stepover,
        "coarsest local sampling {} mm vs half-stepover {}",
        report.max_local_sample_spacing_mm,
        0.5 * expect_stepover
    );

    // The independent witness: mesh centroids, never the sample set.
    let audit = report
        .coverage_audit
        .as_ref()
        .expect("a spiral was emitted, so the audit must have run");
    assert_eq!(audit.centroids_tested, report.region_triangles);
    assert!(
        (audit.region_area_mm2 - std::f64::consts::PI * radius * radius).abs()
            < 0.02 * std::f64::consts::PI * radius * radius
    );
    // With the bands overlapping by 0.076 mm, coverage is contiguous from
    // the centre out to `r₁ + reach`, and the outermost centroid (9.648)
    // sits inside that — so this is asserted at exactly zero, not at a
    // tolerance. A nonzero reading here is real information.
    assert_eq!(
        audit.uncovered_centroid_triangles,
        0,
        "unmachined {} mm² = {:.3}% of region over {} triangles; \
             largest patch {} mm²; distance to centre curve \
             min {:.6} / med {:.6} / max {:.6} mm vs K_c {:.6}; \
             uncovered disk radius min {:.4} / med {:.4} / max {:.4} \
             (near 1 = rim, near 0 = centre, spread = bands at ring midlines)",
        audit.unmachined_area_mm2,
        100.0 * audit.unmachined_area_fraction,
        audit.uncovered_centroid_triangles,
        audit.largest_unmachined_triangle_area_mm2,
        audit.uncovered_distance.min_mm,
        audit.uncovered_distance.median_mm,
        audit.uncovered_distance.max_mm,
        audit.coverage_radius_mm,
        audit.uncovered_disk_radius.min_mm,
        audit.uncovered_disk_radius.median_mm,
        audit.uncovered_disk_radius.max_mm
    );
    assert!((audit.unmachined_area_mm2).abs() < 1e-12);
    assert!((audit.coverage_radius_mm - ball).abs() < 1e-12);

    // Band sizes: every ring must earn its pass.
    assert_eq!(
        report.degenerate_rings, 0,
        "a ring covering <=2 samples is a wasted pass"
    );
    assert!(
        report.band_size_min >= 10,
        "smallest band {}",
        report.band_size_min
    );
    assert!(report.band_size_min <= report.band_size_median);
    assert!(report.band_size_median <= report.band_size_max);

    // Coverage closed on both sides of the bridging step.
    assert_eq!(
        report.uncovered_after_rings, 0,
        "rings must sweep all of S^h"
    );
    assert_eq!(report.uncovered_after_bridging, 0);
    // A full-2π run reproduces its ring's own centre polyline exactly —
    // same radius, same lattice — so the repair has nothing to do. That
    // holds because TAU/2 is an exact multiple of the TAU/120 lattice, so
    // the swept start angle keeps run points in phase with ring points.
    assert_eq!(
        report.bridge_repair_steps, 0,
        "a full-2π run needs no repair"
    );

    // One continuous path.
    assert!(!result.spiral_contact.is_empty());
    assert_eq!(result.spiral_contact.len(), result.spiral_disk.len());
    assert_eq!(report.retract_count, 0);
    assert_eq!(report.disk_self_intersections, 0);
    // The longest ring chord is 2π·10·0.92/120 ≈ 0.48 mm; a hidden jump
    // would show up here as a step an order of magnitude larger.
    assert!(
        report.max_consecutive_step_mm < 1.0,
        "max step {} mm",
        report.max_consecutive_step_mm
    );
    assert_eq!(report.bridge_count, report.ring_count - 1);
    assert_eq!(result.rings_contact.len(), report.ring_count);
    assert_eq!(report.start_angle_candidates, 2);

    // Bridge overhead. Analytic estimate for this fixture: five outer
    // bridges at π/10 (5 % of their own ring each) over rings summing to
    // ΣR ≈ 3.10, plus one near-centre bridge at 2π (≈ 100 % of a ring at
    // R ≈ 0.09), against a total ring length ∝ ΣR ≈ 3.28 — about 7.4 %.
    // The bar is 20 % so sampling jitter in the ring set cannot flip it.
    assert!(
        report.bridge_overhead_pct > 0.0 && report.bridge_overhead_pct < 20.0,
        "bridge overhead {} %",
        report.bridge_overhead_pct
    );

    // Rings pull back to exact circles, because the map is exactly z/ρ.
    assert!(report.ring_count >= 5);
    for (i, ring) in result.rings_contact.iter().enumerate() {
        let want = radius * report.ring_radii[i];
        for p in ring {
            assert!(
                (p.x.hypot(p.y) - want).abs() < 1e-4,
                "ring {i} should be a circle of radius {want}, saw {}",
                p.x.hypot(p.y)
            );
            assert!(p.z.abs() < 1e-6);
        }
    }

    // Spacing, asserted as the two EXACT invariants rather than against
    // the undereated closed form.
    //
    // A ±25 %-of-`expect_stepover` band is not available on this fixture
    // and pretending otherwise is how the previous version of this
    // assertion came to read 0.997 mm against a 1.520 mm expectation: the
    // step is `2·reach'` plus a radial sample gap of up to 0.667 mm, which
    // is 44 % of the stepover all by itself. What IS exact:
    //
    //   floor:   gap >= 2·reach'      — the criterion can never place two
    //                                   rings closer than the derated
    //                                   stepover, whatever the sampling;
    //   ceiling: gap <= 2·reach       — the coverage invariant. This is the
    //                                   whole point of the safety factor,
    //                                   and it is what makes the audit
    //                                   above read exactly zero.
    //
    // Together they bracket the spacing in [0.760, 1.520] mm, and the
    // ceiling is the machining-relevant half.
    let reach_true = 0.5 * expect_stepover;
    let reach_derated = params.ring_spacing_safety * reach_true;
    assert!((report.lateral_reach_mm - reach_true).abs() < 1e-9);
    assert!((report.ring_search_lateral_reach_mm - reach_derated).abs() < 1e-9);
    let mut checked = 0usize;
    for i in 0..report.ring_count.saturating_sub(1) {
        let inner = radius * report.ring_radii[i + 1];
        if inner <= expect_stepover {
            continue;
        }
        let gap = radius * (report.ring_radii[i] - report.ring_radii[i + 1]);
        // `ring_eps` is a disk-domain bisection tolerance, so a ring can
        // land up to `ring_eps · ρ` outward of its exact infimum, and that
        // slack SUBTRACTS from the gap. Both terms of the bound are
        // derived; the ceiling is unaffected because the slack can only
        // ever shrink a gap.
        assert!(
            gap >= 2.0 * reach_derated - params.ring_eps * radius - 1e-9,
            "ring {i}->{} spacing {gap} mm is below the derated stepover {} mm",
            i + 1,
            2.0 * reach_derated
        );
        assert!(
            gap <= 2.0 * reach_true,
            "ring {i}->{} spacing {gap} mm exceeds 2·reach {} mm — coverage \
                 is lost between these rings",
            i + 1,
            2.0 * reach_true
        );
        checked += 1;
    }
    assert!(checked >= 3, "only {checked} spacings were in range");
}

// -----------------------------------------------------------------
// Full pipeline — hemisphere (curved scallop rule)
// -----------------------------------------------------------------

#[test]
fn hemisphere_spirals_at_the_curved_scallop_stepover() {
    let sphere_r = 20.0_f64;
    let ball = 3.0_f64;
    let h = 0.8_f64;
    let mesh = make_test_hemisphere(sphere_r, 20);
    let index = SpatialIndex::build_auto(&mesh);
    let params = SpiralParams {
        n_surface_samples: 4000,
        n_angular_samples: 120,
        ring_eps: 0.003,
        // Same reason as the flat arm. 0.85 rather than 0.50 because this
        // arm's facets are 1.57 mm against a 3.76 mm curved stepover — a
        // better ratio than the flat fixture's — and because the previous
        // radius-form factor of 0.95 was, on this geometry, an effective
        // reach derating of 0.888 that this arm already passed under.
        // 0.85 therefore buys strictly MORE margin than what passed
        // (0.612 mm vs 0.456 mm) while costing ~15 % of stepover, which
        // the ±30 % bar below absorbs.
        ring_spacing_safety: 0.85,
        start_angle_step: TAU / 2.0,
        ..SpiralParams::new(ball, h)
    };
    let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);
    let result = outcome.expect("simply-connected fixture must plan");

    // One boundary loop — the equator — and disk topology.
    assert_eq!(report.boundary_loop_vertices, 80);
    assert_eq!(report.euler_characteristic, 1);
    assert!(
        (report.boundary_loop_length_mm - TAU * sphere_r).abs() < 0.2 * sphere_r,
        "equator length {}",
        report.boundary_loop_length_mm
    );
    // Mean-value weights are strictly positive on *any* triangulation, so
    // Tutte's condition holds unconditionally here — no appeal to this
    // fixture's triangle quality is needed any more. Counted rather than
    // assumed, because that is what makes it a tripwire.
    assert_eq!(report.flipped_triangles, 0);
    assert!(report.flipped_triangle_disk_radii.is_empty());
    assert_eq!(report.mean_value_weight_nonpositive, 0);
    assert!(report.mean_value_weight_min > 0.0);
    // Mean-value is not a conformal map, so area distortion varies from
    // pole to boundary — that is expected and is why it is reported.
    assert!(report.area_distortion_max >= report.area_distortion_min);
    // K >= 1 is structural; the actual value on a curved surface is the
    // open question this row exists to answer, so it is measured and
    // ordered, not bounded.
    assert_eq!(report.dilatation_unmeasurable, 0);
    assert!(report.dilatation_min >= 1.0 - 1e-12);
    assert!(report.dilatation_median >= report.dilatation_min);
    assert!(report.dilatation_p90 >= report.dilatation_median);
    assert!(report.dilatation_max >= report.dilatation_p90);
    let anis = report
        .ring_anisotropy
        .as_ref()
        .expect("rings were placed, so anisotropy must be measured");
    assert_eq!(
        anis.rings_measured + anis.rings_unmeasurable,
        report.ring_count
    );
    assert!(anis.worst_ratio >= anis.median_ratio);
    assert!(
        anis.median_ratio >= 1.0 - 1e-9,
        "a max/min ratio cannot be below 1"
    );

    assert_eq!(report.uncovered_after_rings, 0);
    assert!(report.stall.is_none());
    assert_eq!(report.area_distortion_by_disk_radius.len(), 5);
    assert_eq!(report.uncovered_after_bridging, 0);
    assert!(!result.spiral_contact.is_empty());
    assert_eq!(result.spiral_contact.len(), result.spiral_disk.len());
    assert_eq!(report.retract_count, 0);
    assert_eq!(report.disk_self_intersections, 0);
    assert!(report.ring_count >= 4, "only {} rings", report.ring_count);
    assert_eq!(report.bridge_count, report.ring_count - 1);
    assert_eq!(report.triangles_without_samples, 0);
    assert!(report.samples_placed >= report.region_triangles);
    let audit = report
        .coverage_audit
        .as_ref()
        .expect("a spiral was emitted, so the audit must have run");
    assert_eq!(audit.centroids_tested, report.region_triangles);
    // A hemisphere's 3D area is 2πr²; the faceted one is slightly under.
    assert!(
        (audit.region_area_mm2 - TAU * sphere_r * sphere_r).abs()
            < 0.05 * TAU * sphere_r * sphere_r
    );
    // The bar stays at 5 %: this arm has no closed form, so tightening it
    // would be guessing, and a number picked to look strict is not
    // evidence. What answers "does it pass for the right reason" is the
    // MECHANISM assertion below, which is strictly stronger than any area
    // threshold — it requires that whatever is uncovered be grazing at the
    // rim, not a hole anywhere.
    assert!(
        audit.unmachined_area_fraction < 0.05,
        "unmachined {:.3}% of region over {} triangles; largest patch {} mm²; \
             distance min {:.6} / med {:.6} / max {:.6} mm vs K_c {:.6}; \
             uncovered disk radius min {:.4} / med {:.4} / max {:.4}",
        100.0 * audit.unmachined_area_fraction,
        audit.uncovered_centroid_triangles,
        audit.largest_unmachined_triangle_area_mm2,
        audit.uncovered_distance.min_mm,
        audit.uncovered_distance.median_mm,
        audit.uncovered_distance.max_mm,
        audit.coverage_radius_mm,
        audit.uncovered_disk_radius.min_mm,
        audit.uncovered_disk_radius.median_mm,
        audit.uncovered_disk_radius.max_mm
    );
    if audit.uncovered_centroid_triangles > 0 {
        // Grazing, not a hole: a centroid that misses by microns is the
        // zero-margin knife edge plus facet chords; one that misses by a
        // useful fraction of the tool is unmachined material. The sphere
        // cap's 23.5 % hole sat far outside this bound.
        assert!(
            audit.uncovered_distance.max_mm < audit.coverage_radius_mm + 0.10,
            "uncovered by up to {:.6} mm beyond K_c {:.6} — a hole, not grazing",
            audit.uncovered_distance.max_mm - audit.coverage_radius_mm,
            audit.coverage_radius_mm
        );
        // ...and at the rim, where the region simply ends, not in the
        // interior where a spacing defect would put it.
        assert!(
            audit.uncovered_disk_radius.min_mm > 0.90,
            "uncovered centroids reach disk radius {:.4}; an interior miss is \
                 a spacing defect, not a boundary effect",
            audit.uncovered_disk_radius.min_mm
        );
    }

    // Mean polar angle per ring, measured from the emitted contact points
    // so a dropped point cannot shift the comparison.
    let polar: Vec<f64> = result
        .rings_contact
        .iter()
        .map(|ring| {
            let n = ring.len().max(1) as f64;
            ring.iter().map(|p| p.x.hypot(p.y).atan2(p.z)).sum::<f64>() / n
        })
        .collect();

    // 4000 samples over 2π·20² mm² sit ~0.71 mm apart, 19 % of the
    // 3.76 mm curved stepover, so the bar is 30 %.
    let expect = stepover_from_scallop_curved(ball, h, 1.0 / sphere_r);
    let mut checked = 0usize;
    for i in 0..polar.len().saturating_sub(1) {
        if polar[i + 1] <= expect / sphere_r {
            continue;
        }
        let gap = sphere_r * (polar[i] - polar[i + 1]).abs();
        // Against the undereated curved form, with the ±30 % band covering
        // the deliberate 15 % derating plus the sample-comb overshoot.
        //
        // NOTE, deliberately not asserted here: this arm does NOT satisfy
        // the adequacy condition the flat arm checks. Its equator facets
        // carry ~1 sample each over a 1.57 mm strip, so the radial sample
        // gap is comparable to the 0.61 mm margin. That makes this bar a
        // TOLERANCE, not a proof — the flat arm, which has a closed form
        // and does satisfy the condition, is where the spacing law is
        // actually established. Both numbers are in the message so a
        // failure here is diagnosable rather than mysterious.
        assert!(
            (gap - expect).abs() <= 0.30 * expect,
            "ring {i}->{} meridian spacing {gap} mm vs curved form {expect} mm \
                 (safety factor {}, margin {} mm, coarsest local sampling {} mm)",
            i + 1,
            params.ring_spacing_safety,
            report.ring_spacing_margin_mm,
            report.max_local_sample_spacing_mm
        );
        // No coverage-ceiling invariant here on purpose. The exact bound
        // on a sphere is the CURVED half-stepover (1.88 mm), not the flat
        // `lateral_reach_mm` (2.04 mm) the parameter is defined from, so
        // asserting the flat one would be the wrong bound wearing the
        // right name. On this arm the coverage witness is the independent
        // audit above, which tests the true `K_c` geometry directly; the
        // flat arm is where the exact invariants live.
        checked += 1;
    }
    assert!(checked >= 2, "only {checked} spacings were in range");
}

/// The stall diagnostic is itself an instrument, so it gets measured on a
/// case whose answer is known in closed form rather than only on the
/// terrain run that motivated it.
///
/// `max_rings: 1` forces the ring loop to give up after one ring on the
/// exact-affine flat disk. Every remaining uncovered sample is, by
/// definition, farther than `K_c` from that ring's centre curve — so the
/// measured minimum distance must be at least `K_c`, and the distribution
/// must widen from there toward the disk centre.
#[test]
fn a_forced_ring_limit_publishes_a_measurable_stall_block() {
    let radius = 10.0_f64;
    let ball = 2.0_f64;
    let mesh = disk_mesh(radius, 10, 48);
    let index = SpatialIndex::build_auto(&mesh);
    let params = SpiralParams {
        n_surface_samples: 1500,
        n_angular_samples: 90,
        ring_eps: 0.005,
        max_rings: 1,
        start_angle_step: TAU,
        ..SpiralParams::new(ball, 0.15)
    };
    let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);

    match outcome.unwrap_err() {
        SpiralRefusal::RingLimitReached { rings, uncovered } => {
            assert_eq!(rings, 1);
            assert!(uncovered > 0);
        }
        other => panic!("expected RingLimitReached, got {other:?}"),
    }

    // The report survived, with the ring rows the refusal was about.
    assert_eq!(report.ring_count, 1);
    assert_eq!(report.ring_radii.len(), 1);
    assert_eq!(report.ring_lengths_mm.len(), 1);
    assert!(report.uncovered_after_rings > 0);
    assert_eq!(report.area_distortion_by_disk_radius.len(), 5);

    let stall = report.stall.expect("an unfinished ring search must say so");
    assert_eq!(stall.rings_placed, 1);
    assert_eq!(stall.ring_radii.len(), 1);
    assert_eq!(stall.uncovered, report.uncovered_after_rings);
    assert_eq!(
        stall.distance_reference,
        StallDistanceReference::LastPlacedRing(0)
    );
    assert!((stall.coverage_radius_mm - ball).abs() < 1e-12);
    assert!(stall.all_uncovered.samples > 0);
    assert!(
        stall.all_uncovered.samples <= 2000,
        "sample cap not honoured"
    );
    // Uncovered means farther than K_c, by construction.
    assert!(
        stall.all_uncovered.min_mm >= ball - 1e-6,
        "min distance {} should be at least K_c {ball}",
        stall.all_uncovered.min_mm
    );
    assert!(stall.all_uncovered.median_mm >= stall.all_uncovered.min_mm);
    assert!(stall.all_uncovered.max_mm >= stall.all_uncovered.median_mm);
    // The farthest uncovered point is the disk centre: its distance to a
    // ring at 3D radius ρR₁ ≈ 9.24, whose centre curve sits K_c above the
    // plane, is √(9.24² + (K_c − h)²) ≈ 9.4 mm. Nothing can exceed that.
    assert!(
        stall.all_uncovered.max_mm < radius + ball,
        "max distance {} mm",
        stall.all_uncovered.max_mm
    );

    // The fold census must be clean: the map is an embedding, so every
    // point outside the last placed ring really was swept by it. This is
    // the row that read nonzero on the terrain arms under cotangent
    // weights, and it is the one that separates a fold from a genuinely
    // uncoverable geometry.
    assert_eq!(
        stall.uncovered_outside_last_ring, 0,
        "an embedding cannot leave points outside a ring that certified them"
    );
    // The near-band restriction keeps only points within one estimated
    // band width of the frontier, so it is a strict subset and closer in.
    assert!(stall.band_width_disk > 0.0 && stall.band_width_disk <= 1.0);
    // NOT asserted: that `near_band.samples <= all_uncovered.samples`.
    // `summarise` strides to a 2000 cap, so a subset under the cap can
    // report more samples than a superset over it. Both populations are
    // stride-1 at this fixture's N_S, but the relation is a property of
    // the cap, not of the sets, and must not be turned into an invariant.
    if stall.near_band.samples > 0 {
        assert!(stall.near_band.median_mm <= stall.all_uncovered.median_mm);
    }
    // `max_rings` stopped the search; no radius was ever proven
    // infeasible, so there are no blockers to census.
    assert_eq!(stall.blockers.samples, 0);
}

/// `N_S` far below the triangle count must be **impossible to starve**,
/// and inadequate sampling must be **reported** rather than passed.
///
/// This is the regression for the sphere-cap defect: an area-apportioned
/// hard cap left ~46 % of triangles with no sample, the coverage predicate
/// had no population in the middle, and a 23.5 %-of-region unmachined hole
/// came back reading `uncovered_after_rings = 0`. `N_S = 1` on a
/// 192-triangle fixture is that failure taken to its extreme.
#[test]
fn a_starved_sample_budget_cannot_blind_the_coverage_predicate() {
    let radius = 10.0_f64;
    let ball = 2.0_f64;
    let h = 0.15_f64;
    let mesh = disk_mesh(radius, 2, 64);
    let index = SpatialIndex::build_auto(&mesh);
    let params = SpiralParams {
        n_surface_samples: 1,
        n_angular_samples: 90,
        ring_eps: 0.005,
        start_angle_step: TAU,
        ..SpiralParams::new(ball, h)
    };
    let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);

    // Route taken is the floor, so starvation is structurally impossible:
    // every triangle carries a sample no matter how small N_S is.
    assert_eq!(report.samples_requested, 1);
    assert_eq!(
        report.triangles_without_samples, 0,
        "N_S is a floor, not a cap — no triangle may be invisible"
    );
    assert!((report.largest_unsampled_triangle_area_mm2).abs() < 1e-12);
    assert_eq!(report.samples_placed, report.region_triangles);
    assert!(report.region_triangles >= 192);

    // ...and the sampling that results is genuinely too coarse for the
    // stepover, which the adequacy row must SAY rather than hide. The old
    // rule, √(region area / N_S), is what missed this class entirely.
    let stepover = stepover_from_scallop_flat(ball, h);
    assert!(
        report.max_local_sample_spacing_mm > 0.5 * stepover,
        "this fixture is meant to be under-resolved: {} mm vs half-stepover {}",
        report.max_local_sample_spacing_mm,
        0.5 * stepover
    );

    // Whatever the search then does, the independent audit is the witness.
    if outcome.is_ok() {
        let audit = report
            .coverage_audit
            .as_ref()
            .expect("a spiral was emitted, so the audit must have run");
        assert_eq!(audit.centroids_tested, report.region_triangles);
        assert!(audit.unmachined_area_fraction >= 0.0 && audit.unmachined_area_fraction <= 1.0);
    }
}

#[test]
fn an_empty_region_is_refused() {
    let mesh = disk_mesh(10.0, 3, 12);
    let index = SpatialIndex::build_auto(&mesh);
    let params = SpiralParams::default();
    let (report, outcome) = plan_spiral(&mesh, &index, &[], &params);
    assert_eq!(outcome.unwrap_err(), SpiralRefusal::EmptyRegion);
    assert_eq!(report.region_triangles, 0);
}
