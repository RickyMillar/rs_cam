//! **Track B evidence instrument** — does the SHIPPED shallow-band raster
//! meet its own scallop spec on sloped ground?
//! (`planning/honest_raster_2026-09-01/TRACK.md`; pre-registration in
//! `planning/honest_raster_2026-09-01/FINDINGS.md`.)
//!
//! # The question
//!
//! `unified_finish`'s `FinishBand::Shallow` arm builds an XY lattice at
//! `raster_stepover` (`build_shallow_raster_grid`) and emits it through
//! `raster_toolpath_from_grid`. The scallop constraint lives on the 3D
//! surface. On ground whose cross-feed slope is `theta_y`, an XY-uniform
//! raster's passes sit `s_XY / cos(theta_y)` apart along the surface. If the
//! operator sets `raster_stepover = s_max` (the iso-scallop limit), the
//! shipped arm then exceeds `s_max` on every cross-slope — the same defect
//! `planning/finishing_synthesis_2026-08-30.md` §1 measured on the
//! test-local harness raster (`raster_candidate`), which this instrument
//! does NOT call. Only the shipped orchestrator runs here.
//!
//! # Fixtures (analytic only; facet-to-stepover ratio printed as proof)
//!
//! * **SPHERE CAP** — restated from `conformal_spiral_synthetic_f2.rs`
//!   (`sphere_cap_mesh`, R_s = 20, cap radius 6), densified to
//!   64 rings x 384 sectors so the `<= stepover/3` facet budget also holds
//!   at the honest arm's tighter stepover. Umbilic: `s_max` is one exact
//!   number.
//! * **PLANE 20 deg / PLANE 40 deg** — a plane tilted about X, slope rising
//!   along +Y (the 0-degree raster's cross-feed direction), 12 x 12 mm in
//!   XY. Isolates the mechanism exactly: prediction
//!   `achieved = s_XY / cos(theta)`.
//!
//! # Status after the fix (2026-09-01)
//!
//! The Track B run CONFIRMED the defect and the operator ruled: fix it,
//! always on, no dial. The Shallow arm now derates its effective stepover
//! by `cos(theta_max)` per region before any lattice is built
//! (`unified_finish.rs`, `shallow_region_max_slope_deg`). This instrument
//! is now the FIX-ACCEPTANCE gate: it runs the shipped arm only, checks
//! the emitted rows sit at the report's derated stepover (F-B1'), and
//! ASSERTS every fixture reads CLEAN (`<= 1.02 x s_max`). The pre-fix
//! manual HONEST arm is retired — a second external derate would now
//! derate twice. Fast flat/sloped sentries live in
//! `shallow_raster_slope_derate.rs`.
//!
//! # Verdict bands (pre-registered, `TRACK.md` — do not soften)
//!
//! * **DEFECT CONFIRMED**: achieved surface spacing > `s_max x 1.05` over
//!   more than 10% of sloped samples (total slope > 5 deg) on an analytic
//!   fixture.
//! * **CLEAN**: achieved <= `s_max x 1.02` everywhere measurable.
//! * Between: report as measured, no verdict.
//!
//! # Run
//!
//! ```text
//! cargo test --release -p rs_cam_core --test shipped_raster_spacing_b1 \
//!   -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]


use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::spacing::{
    ContactMaps, SpacingMeasurement, SpacingSample, measure_raster_spacing,
};
use rs_cam_core::scallop_math;
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::unified_finish::{
    UnifiedFinishParams, UnifiedFinishReport, unified_finish_toolpath_with_cancel,
};

// ---- Pinned dials, shared with the F2 instrument -------------------------

/// Ball radius `K_c` (mm). Same as `conformal_spiral_synthetic_f2.rs`.
const BALL_RADIUS_MM: f64 = 1.0;
const BALL_CUTTING_LENGTH_MM: f64 = 20.0;
/// Scallop / cusp spec `h` (mm). Same as the F2 instrument.
const CUSP_HEIGHT_MM: f64 = 0.03;

const SPHERE_RADIUS_MM: f64 = 20.0;
const SPHERE_CAP_RADIUS_MM: f64 = 6.0;
/// Densified from the F2 file's 55 x 340: the honest arm's stepover is
/// `cos(17.5 deg)` tighter, so its facet budget is tighter too.
const SPHERE_CAP_RINGS: usize = 64;
const SPHERE_CAP_SECTORS: usize = 384;

/// Half-side (mm) of the tilted-plane patches in XY.
const PLANE_HALF_MM: f64 = 6.0;

/// Interior inset (mm) on the CL sample filter: one tool radius so the ball
/// cannot touch the mesh rim, plus one stepover of margin.
const INTERIOR_INSET_MM: f64 = 2.0;

/// TRACK.md band edges.
const EXCESS_FACTOR: f64 = 1.05;
const CLEAN_FACTOR: f64 = 1.02;
const MATERIAL_FRACTION: f64 = 0.10;
const SLOPED_MIN_DEG: f64 = 5.0;

// ---- Fixtures -------------------------------------------------------------

/// One analytic fixture: the mesh plus the closed forms the measurement
/// needs. The closed forms are functions of the CONTACT point, never of the
/// facets, so facet error cannot enter the oracle side.
struct Fixture {
    name: &'static str,
    mesh: TriangleMesh,
    /// The iso-scallop limit `s_max` — exact and constant on both fixture
    /// families (umbilic sphere, flat plane).
    s_max_mm: f64,
    /// Ball-centre -> analytic contact point on the true surface.
    contact_of_center: Box<dyn Fn(P3) -> P3>,
    /// Ball-centre -> distance to the true surface (self-check F-B3;
    /// must be `K_c`).
    center_surface_distance: Box<dyn Fn(P3) -> f64>,
    /// Contact XY -> total slope (deg).
    total_slope_deg: Box<dyn Fn(f64, f64) -> f64>,
    /// Contact XY -> cross-feed slope `theta_y = atan(|dz/dy|)` (deg).
    cross_slope_deg: Box<dyn Fn(f64, f64) -> f64>,
    /// CL XY -> is this sample in the interior window?
    interior_cl: Box<dyn Fn(f64, f64) -> bool>,
}

/// A spherical cap as a convex-up dome. **Restated from
/// `conformal_spiral_synthetic_f2.rs::sphere_cap_mesh`** (that file's copy
/// is private to its own test binary), with denser dials — see
/// [`SPHERE_CAP_RINGS`].
fn sphere_cap_mesh(
    sphere_radius_mm: f64,
    cap_radius_mm: f64,
    rings: usize,
    sectors: usize,
) -> TriangleMesh {
    let rings = rings.max(1);
    let sectors = sectors.max(3);
    let rim_z = (sphere_radius_mm * sphere_radius_mm - cap_radius_mm * cap_radius_mm)
        .max(0.0)
        .sqrt();
    let height = |r: f64| {
        (sphere_radius_mm * sphere_radius_mm - r * r)
            .max(0.0)
            .sqrt()
            - rim_z
    };
    let mut verts: Vec<P3> = Vec::with_capacity(1 + rings * sectors);
    verts.push(P3::new(0.0, 0.0, height(0.0)));
    for i in 1..=rings {
        let r = cap_radius_mm * (i as f64) / (rings as f64);
        let z = height(r);
        for j in 0..sectors {
            let t = std::f64::consts::TAU * (j as f64) / (sectors as f64);
            verts.push(P3::new(r * t.cos(), r * t.sin(), z));
        }
    }
    let v = |i: usize, j: usize| -> u32 { (1 + (i - 1) * sectors + (j % sectors)) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(sectors * (2 * rings - 1));
    for j in 0..sectors {
        tris.push([0, v(1, j), v(1, j + 1)]);
    }
    for i in 2..=rings {
        for j in 0..sectors {
            let (a, b) = (v(i - 1, j), v(i - 1, j + 1));
            let (c, d) = (v(i, j), v(i, j + 1));
            tris.push([a, c, d]);
            tris.push([a, d, b]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

fn sphere_fixture() -> Fixture {
    let mesh = sphere_cap_mesh(
        SPHERE_RADIUS_MM,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_RINGS,
        SPHERE_CAP_SECTORS,
    );
    let rim_z =
        (SPHERE_RADIUS_MM * SPHERE_RADIUS_MM - SPHERE_CAP_RADIUS_MM * SPHERE_CAP_RADIUS_MM).sqrt();
    // Sphere centre in world coordinates (rim at z = 0, apex up).
    let centre = P3::new(0.0, 0.0, -rim_z);
    let s_max = scallop_math::stepover_from_scallop_curved(
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
        1.0 / SPHERE_RADIUS_MM,
    );
    let slope = move |x: f64, y: f64| -> f64 {
        let r = x.hypot(y).min(SPHERE_RADIUS_MM);
        (r / SPHERE_RADIUS_MM).asin().to_degrees()
    };
    let cross = move |x: f64, y: f64| -> f64 {
        let r2 = x * x + y * y;
        let root = (SPHERE_RADIUS_MM * SPHERE_RADIUS_MM - r2).max(1e-9).sqrt();
        (y.abs() / root).atan().to_degrees()
    };
    Fixture {
        name: "SPHERE CAP (R_s 20, cap radius 6)",
        mesh,
        s_max_mm: s_max,
        contact_of_center: Box::new(move |c: P3| {
            let d = c - centre;
            let n = d.norm().max(1e-12);
            P3::new(
                centre.x + d.x * SPHERE_RADIUS_MM / n,
                centre.y + d.y * SPHERE_RADIUS_MM / n,
                centre.z + d.z * SPHERE_RADIUS_MM / n,
            )
        }),
        center_surface_distance: Box::new(move |c: P3| (c - centre).norm() - SPHERE_RADIUS_MM),
        total_slope_deg: Box::new(slope),
        cross_slope_deg: Box::new(cross),
        interior_cl: Box::new(|x, y| x.hypot(y) <= SPHERE_CAP_RADIUS_MM - 1.0),
    }
}

/// A plane tilted `theta_deg` about the X axis, slope rising along +Y:
/// `z = tan(theta) * (y + PLANE_HALF_MM)`, over `[-h, h]^2` in XY.
///
/// `cells` per side is chosen by the caller from the facet budget of the
/// TIGHTEST stepover the run uses; the binding edge is the split-quad
/// diagonal, `sqrt(2) * cell / cos(theta)` in 3D.
fn tilted_plane_mesh(theta_deg: f64, cells: usize) -> TriangleMesh {
    let m = theta_deg.to_radians().tan();
    let n = cells.max(1);
    let step = 2.0 * PLANE_HALF_MM / (n as f64);
    let mut verts: Vec<P3> = Vec::with_capacity((n + 1) * (n + 1));
    for iy in 0..=n {
        let y = -PLANE_HALF_MM + (iy as f64) * step;
        for ix in 0..=n {
            let x = -PLANE_HALF_MM + (ix as f64) * step;
            verts.push(P3::new(x, y, m * (y + PLANE_HALF_MM)));
        }
    }
    let idx = |ix: usize, iy: usize| -> u32 { (iy * (n + 1) + ix) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(2 * n * n);
    for iy in 0..n {
        for ix in 0..n {
            let (a, b) = (idx(ix, iy), idx(ix + 1, iy));
            let (c, d) = (idx(ix, iy + 1), idx(ix + 1, iy + 1));
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

fn plane_fixture(theta_deg: f64, honest_stepover_mm: f64) -> Fixture {
    // Facet budget from the TIGHTEST stepover this run measures (the honest
    // arm's): 3D diagonal <= stepover/3 => cell <= budget * cos / sqrt(2).
    let theta = theta_deg.to_radians();
    let edge_budget = honest_stepover_mm / 3.0;
    let cell = edge_budget * theta.cos() / std::f64::consts::SQRT_2;
    let cells = (2.0 * PLANE_HALF_MM / cell).ceil() as usize;
    let mesh = tilted_plane_mesh(theta_deg, cells);
    let s_max = scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, 0.0);
    let m = theta.tan();
    // Unit surface normal (0, -sin, cos); plane passes through
    // (0, -PLANE_HALF_MM, 0).
    let (ny, nz) = (-theta.sin(), theta.cos());
    let name: &'static str = match theta_deg as i64 {
        20 => "PLANE 20 deg (slope along +Y)",
        40 => "PLANE 40 deg (slope along +Y)",
        _ => "PLANE (slope along +Y)",
    };
    Fixture {
        name,
        mesh,
        s_max_mm: s_max,
        contact_of_center: Box::new(move |c: P3| {
            // Signed distance from the ball centre to the plane, along n.
            let d = (c.y + PLANE_HALF_MM) * ny + c.z * nz;
            P3::new(c.x, c.y - d * ny, c.z - d * nz)
        }),
        center_surface_distance: Box::new(move |c: P3| (c.y + PLANE_HALF_MM) * ny + c.z * nz),
        total_slope_deg: Box::new(move |_x, _y| theta_deg),
        cross_slope_deg: Box::new(move |_x, _y| m.abs().atan().to_degrees()),
        interior_cl: Box::new(|x, y| {
            x.abs() <= PLANE_HALF_MM - INTERIOR_INSET_MM
                && y.abs() <= PLANE_HALF_MM - INTERIOR_INSET_MM
        }),
    }
}

// ---- The shipped arm -------------------------------------------------------

/// Run the SHIPPED orchestrator once, with
/// `monotone_cell_decomposition` pinned OFF: the shared 0-degree lattice,
/// byte-identical to the pre-C2 shipped band. That was the default shape
/// until the 2026-09-01 flip (C4 ruling); this instrument pins it because
/// it measures the undivided raster's spacing.
///
/// `intra_region_hookup_mm` is pinned to `0.0` (the
/// `shallow_band_stock_to_leave_exhibit_d16_2.rs` precedent) so the emitted
/// motion is the raster band alone: the relink pass changes links, never
/// pass spacing, and its extra surface-riding feed moves would blur the
/// cutting-distance term of the x floor score.
fn run_shipped(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    raster_stepover_mm: f64,
) -> (Toolpath, UnifiedFinishReport) {
    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let params = UnifiedFinishParams {
        raster_stepover: raster_stepover_mm,
        scallop_height: CUSP_HEIGHT_MM,
        intra_region_hookup_mm: 0.0,
        // Pinned OFF since the 2026-09-01 default flip (C4 ruling): this
        // instrument measures the UNDIVIDED shared-lattice raster, per the
        // doc above, so it must not follow the new default.
        monotone_cell_decomposition: false,
        ..UnifiedFinishParams::default()
    };
    // `for_tool` takes the CUSP radius, never `radius()` (its own doc; the
    // 2026-07-29 radius programme). Coincident on a ball tool, stated anyway.
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius_mm());
    let never_cancel = || false;
    let (toolpath, _anns, report) = unified_finish_toolpath_with_cancel(
        mesh,
        index,
        &cutter,
        mesh.bbox.max.z + 1.0,
        mesh.bbox.min.z - 1.0,
        &params,
        &planner,
        /* machining_boundary */ None,
        /* link_kinematics */ None,
        /* claims */ None,
        /* debug */ None,
        &never_cancel,
    )
    .expect("uncancelled generation");
    (toolpath, report)
}

/// Hard gate: the run measures the Shallow band and nothing else.
fn assert_all_shallow(report: &UnifiedFinishReport, label: &str) {
    for entry in &report.region_table {
        assert_eq!(
            entry.kind.band(),
            Some(FinishBand::Shallow),
            "{label}: region_table entry is {} — REFUSE: a mixed-band run \
             cannot answer the Shallow-band question",
            entry.kind.band_label(),
        );
    }
    assert!(
        !report.region_table.is_empty(),
        "{label}: the planner produced no regions at all"
    );
}

// ---- Measurement ------------------------------------------------------------

// PROMOTED (Track M, 2026-09-02): `SpacingSample`, the spacing
// measurement and `dist_point_segment` live in
// `rs_cam_core::metrology::spacing`, extracted verbatim from this file.
// The fixture's ANALYTIC closed forms stay here and ride in as
// `ContactMaps`, so the ruler still carries no estimator.
fn measure(toolpath: &Toolpath, fixture: &Fixture, stepover_mm: f64) -> SpacingMeasurement {
    let maps = ContactMaps {
        contact_of_center: &*fixture.contact_of_center,
        interior_cl: &*fixture.interior_cl,
        center_surface_distance: &*fixture.center_surface_distance,
        total_slope_deg: &*fixture.total_slope_deg,
        cross_slope_deg: &*fixture.cross_slope_deg,
    };
    measure_raster_spacing(toolpath, &maps, BALL_RADIUS_MM, stepover_mm)
}
fn percentile(sorted: &[f64], f: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() as f64 - 1.0) * f).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// Print the slope-band table and return the verdict inputs:
/// `(sloped_count, sloped_exceeding, max_ratio_over_all_samples)`.
fn print_band_table(m: &SpacingMeasurement, s_max: f64) -> (usize, usize, f64) {
    eprintln!(
        "\n     {:<16} {:>7} {:>10} {:>10} {:>10} {:>9} {:>11}",
        "cross-slope band", "n", "median", "p90", "max", "med/s_max", "% > 1.05 s"
    );
    for band in 0..9 {
        let (lo, hi) = (band as f64 * 5.0, band as f64 * 5.0 + 5.0);
        let mut vals: Vec<f64> = m
            .samples
            .iter()
            .filter(|s| s.cross_slope_deg >= lo && s.cross_slope_deg < hi)
            .map(|s| s.achieved_mm)
            .collect();
        if vals.is_empty() {
            continue;
        }
        vals.sort_by(f64::total_cmp);
        let n = vals.len();
        let over = vals.iter().filter(|&&v| v > EXCESS_FACTOR * s_max).count();
        eprintln!(
            "     {:>4.0}-{:<11.0} {:>7} {:>10.5} {:>10.5} {:>10.5} {:>9.4} {:>10.1}%",
            lo,
            hi,
            n,
            percentile(&vals, 0.5),
            percentile(&vals, 0.9),
            vals[n - 1],
            percentile(&vals, 0.5) / s_max,
            100.0 * over as f64 / n as f64,
        );
    }
    let sloped: Vec<&SpacingSample> = m
        .samples
        .iter()
        .filter(|s| s.total_slope_deg > SLOPED_MIN_DEG)
        .collect();
    let exceeding = sloped
        .iter()
        .filter(|s| s.achieved_mm > EXCESS_FACTOR * s_max)
        .count();
    let cross_sloped = m
        .samples
        .iter()
        .filter(|s| s.cross_slope_deg > SLOPED_MIN_DEG)
        .count();
    let cross_exceeding = m
        .samples
        .iter()
        .filter(|s| s.cross_slope_deg > SLOPED_MIN_DEG && s.achieved_mm > EXCESS_FACTOR * s_max)
        .count();
    let max_ratio = m
        .samples
        .iter()
        .map(|s| s.achieved_mm / s_max)
        .fold(0.0f64, f64::max);
    eprintln!(
        "\n     sloped samples (total slope > {SLOPED_MIN_DEG:.0} deg)      {:>7}   of which \
         > 1.05 x s_max: {} ({:.1}%)   <- VERDICT population",
        sloped.len(),
        exceeding,
        100.0 * exceeding as f64 / sloped.len().max(1) as f64,
    );
    eprintln!(
        "     cross-sloped samples (theta_y > {SLOPED_MIN_DEG:.0} deg)     {:>7}   of which \
         > 1.05 x s_max: {} ({:.1}%)   (mechanism population, context)",
        cross_sloped,
        cross_exceeding,
        100.0 * cross_exceeding as f64 / cross_sloped.max(1) as f64,
    );
    eprintln!("     max achieved / s_max over ALL samples       {max_ratio:>10.4}");
    (sloped.len(), exceeding, max_ratio)
}

/// Max 3D triangle edge (mm) over the whole mesh — the facet proof.
fn max_edge_mm(mesh: &TriangleMesh) -> f64 {
    let mut max_edge = 0.0f64;
    for face in &mesh.faces {
        for k in 0..3 {
            max_edge = max_edge.max((face.v[(k + 1) % 3] - face.v[k]).norm());
        }
    }
    max_edge
}

/// Max face slope (deg) — `theta_max` for the honest arm's derate.
fn max_slope_deg(mesh: &TriangleMesh) -> f64 {
    let mut worst = 0.0f64;
    for face in &mesh.faces {
        let nz = face.normal.z.abs().clamp(0.0, 1.0);
        worst = worst.max(nz.acos().to_degrees());
    }
    worst
}

/// 3D surface area (mm^2) — the floor's numerator (`s_max` is constant on
/// these fixtures, so `L_min = area / s_max` IS the synthesis §1 integral).
fn area_3d_mm2(mesh: &TriangleMesh) -> f64 {
    let mut area = 0.0f64;
    for face in &mesh.faces {
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        area += 0.5 * e1.cross(&e2).norm();
    }
    area
}

struct ArmResult {
    label: &'static str,
    stepover_mm: f64,
    cut_mm: f64,
    x_floor: f64,
    sloped: usize,
    exceeding: usize,
    max_ratio: f64,
}

/// One arm end-to-end: run the shipped orchestrator, gate, measure, print.
fn run_arm(
    fixture: &Fixture,
    index: &SpatialIndex,
    label: &'static str,
    stepover_mm: f64,
    l_min_mm: f64,
) -> ArmResult {
    eprintln!("\n   ---- ARM {label}: raster_stepover = {stepover_mm:.5} mm ----");
    let (toolpath, report) = run_shipped(&fixture.mesh, index, stepover_mm);
    assert_all_shallow(&report, fixture.name);
    eprintln!(
        "     regions: {} (all Shallow — asserted), moves: {}",
        report.region_table.len(),
        toolpath.moves.len()
    );
    // Fix acceptance (2026-09-01): the shipped arm now derates its own
    // effective stepover by cos(theta_max) of each region. The emitted rows
    // must be XY-uniform at the DERATED value the report declares — the
    // audit trail and the motion must agree.
    for d in &report.shallow_slope_derates {
        eprintln!(
            "     derate: region {} theta_max {:.3} deg   {:.5} mm -> {:.5} mm",
            d.region_index, d.slope_max_deg, d.configured_stepover_mm, d.derated_stepover_mm
        );
        assert!(
            (d.configured_stepover_mm - stepover_mm).abs() < 1e-9,
            "derate record does not carry the configured stepover"
        );
    }
    let effective_mm = report
        .shallow_slope_derates
        .iter()
        .map(|d| d.derated_stepover_mm)
        .fold(stepover_mm, f64::min);
    for d in &report.shallow_slope_derates {
        assert!(
            (d.derated_stepover_mm - effective_mm).abs() < 1e-9,
            "two Shallow regions carry different derates on a one-region fixture — \
             the Delta-y census below cannot be read"
        );
    }
    let m = measure(&toolpath, fixture, effective_mm);
    eprintln!(
        "     raster rows: {}   adjacent-row Delta-y: min {:.6} / max {:.6} mm \
         (XY mechanism check, F-B1; expected effective stepover {:.6} mm)",
        m.rows, m.row_dy_min, m.row_dy_max, effective_mm
    );
    assert!(
        (m.row_dy_min - effective_mm).abs() < 1e-6 && (m.row_dy_max - effective_mm).abs() < 1e-6,
        "F-B1 FAILED: emitted rows are not XY-uniform at the effective stepover — \
         the cos-theta arithmetic may not be quoted"
    );
    eprintln!(
        "     CL->contact self-check (F-B3): max |dist(center, surface) - K_c| = {:.6} mm",
        m.center_distance_err_max
    );
    assert!(
        m.center_distance_err_max < 0.01 * BALL_RADIUS_MM,
        "F-B3 FAILED: contact mapping is reading rim roll-off or facet error"
    );
    assert!(
        !m.samples.is_empty(),
        "no interior spacing samples — the instrument has no population"
    );
    let (sloped, exceeding, max_ratio) = print_band_table(&m, fixture.s_max_mm);
    let cut_mm = toolpath.total_cutting_distance();
    let x_floor = cut_mm / l_min_mm;
    eprintln!("     cutting distance {cut_mm:>10.1} mm   x floor {x_floor:>7.3}");
    ArmResult {
        label,
        stepover_mm,
        cut_mm,
        x_floor,
        sloped,
        exceeding,
        max_ratio,
    }
}

// ---- The instrument ----------------------------------------------------------

#[test]
#[ignore = "Track B evidence instrument — run explicitly with --ignored --nocapture"]
fn shipped_shallow_raster_spacing_on_analytic_fixtures() {
    eprintln!(
        "\n================ TRACK B — SHIPPED SHALLOW RASTER vs ITS OWN SCALLOP SPEC \
         ================"
    );
    eprintln!(
        "  Code path: unified_finish_toolpath_with_cancel -> FinishBand::Shallow ->\n\
         \x20 build_shallow_raster_grid + raster_toolpath_from_grid. The harness raster\n\
         \x20 (`raster_candidate`) is NOT called anywhere in this instrument.\n\
         \x20 K_c = {BALL_RADIUS_MM} mm, h = {CUSP_HEIGHT_MM} mm. Verdict bands: TRACK.md."
    );

    // The honest stepovers are needed up front: the plane facet budget must
    // hold for the TIGHTEST stepover the run measures.
    let s_flat = scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, 0.0);
    let fixtures: Vec<Fixture> = vec![
        sphere_fixture(),
        plane_fixture(20.0, s_flat * 20.0f64.to_radians().cos()),
        plane_fixture(40.0, s_flat * 40.0f64.to_radians().cos()),
    ];

    let mut verdict_rows: Vec<(String, ArmResult)> = Vec::new();
    for fixture in &fixtures {
        eprintln!("\n========== FIXTURE {} ==========", fixture.name);
        let index = SpatialIndex::build_auto(&fixture.mesh);
        let s_max = fixture.s_max_mm;
        let theta_max = max_slope_deg(&fixture.mesh);
        let honest = s_max * theta_max.to_radians().cos();
        let edge = max_edge_mm(&fixture.mesh);
        let area = area_3d_mm2(&fixture.mesh);
        let l_min = area / s_max;
        eprintln!(
            "     triangles {}   3D area {:.3} mm^2   s_max {:.5} mm (exact: constant \
             curvature)",
            fixture.mesh.faces.len(),
            area,
            s_max
        );
        eprintln!(
            "     theta_max (measured from faces) {theta_max:.3} deg   honest stepover = \
             s_max x cos(theta_max) = {honest:.5} mm"
        );
        eprintln!(
            "     FACET PROOF: max 3D edge {:.5} mm; edge/stepover = {:.4} (stock), {:.4} \
             (honest) — both must be <= 1/3",
            edge,
            edge / s_max,
            edge / honest
        );
        assert!(
            edge / s_max <= 1.0 / 3.0 + 1e-9 && edge / honest <= 1.0 / 3.0 + 1e-9,
            "facet budget violated: the fixture cannot resolve the measurand"
        );
        eprintln!("     L_min = area / s_max = {l_min:.1} mm (the synthesis §1 floor, exact)");

        // The shipped arm at the spec stepover. Since the 2026-09-01 fix
        // the arm derates its own effective stepover per region, so the
        // pre-fix manual HONEST arm (a second run at s_max x cos) would
        // now derate TWICE and measures nothing registered; it is retired.
        let stock = run_arm(fixture, &index, "SHIPPED (s_XY = s_max)", s_max, l_min);
        let frac = stock.exceeding as f64 / stock.sloped.max(1) as f64;
        let verdict = if frac > MATERIAL_FRACTION {
            "DEFECT CONFIRMED"
        } else if stock.max_ratio <= CLEAN_FACTOR {
            "CLEAN"
        } else {
            "BETWEEN (report as measured, no verdict)"
        };
        eprintln!(
            "\n     >>> FIXTURE VERDICT ({}): {verdict} — {:.1}% of sloped samples exceed \
             1.05 x s_max; max ratio {:.4}",
            fixture.name,
            100.0 * frac,
            stock.max_ratio
        );
        // Fix-acceptance falsifier (registered in FINDINGS.md §"fix
        // acceptance" before the run): every fixture must read CLEAN.
        assert!(
            stock.max_ratio <= CLEAN_FACTOR,
            "FIX ACCEPTANCE FAILED on {}: max achieved/s_max = {:.4} > {CLEAN_FACTOR}",
            fixture.name,
            stock.max_ratio
        );
        verdict_rows.push((fixture.name.to_owned(), stock));
    }

    // ---- Summary: the x floor / price table.
    eprintln!("\n================ x FLOOR TABLE (shipped arm, all fixtures) ================");
    eprintln!(
        "  {:<36} {:>10} {:>10} {:>8}",
        "fixture", "s_XY dial", "cut mm", "x floor"
    );
    for (name, stock) in &verdict_rows {
        eprintln!(
            "  {:<36} {:>10.5} {:>10.1} {:>8.3}",
            name, stock.stepover_mm, stock.cut_mm, stock.x_floor
        );
    }
    eprintln!(
        "\n  Read x floor in one direction only: below 1.0 PROVES under-coverage \
         (synthesis §1)."
    );

    // ---- Overall verdict, per the pre-registered bands.
    let any_defect = verdict_rows
        .iter()
        .any(|(_, stock)| stock.exceeding as f64 / stock.sloped.max(1) as f64 > MATERIAL_FRACTION);
    let all_clean = verdict_rows
        .iter()
        .all(|(_, stock)| stock.max_ratio <= CLEAN_FACTOR);
    let overall = if any_defect {
        "DEFECT CONFIRMED"
    } else if all_clean {
        "CLEAN"
    } else {
        "BETWEEN (report as measured, no verdict)"
    };
    eprintln!("\n================ OVERALL VERDICT: {overall} ================");
    for (name, stock) in &verdict_rows {
        eprintln!(
            "  {name}: shipped max achieved/s_max {:.4} ({})",
            stock.max_ratio, stock.label
        );
    }
    assert!(all_clean, "FIX ACCEPTANCE FAILED: a fixture is not CLEAN");
}
