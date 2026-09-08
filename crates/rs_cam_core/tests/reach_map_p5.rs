//! P5 — the per-tool reach map (`rs_cam_core::reach_map`).
//!
//! The map answers one question — *does this ball radius fit into the
//! mountain valleys?* — as a per-cell gap in mm between the surface a cutter
//! would leave and the true mesh. These sentries pin the five properties the
//! answer rests on:
//!
//! 1. a plane is fully reachable by any cutter, at any slope (the property
//!    that makes the min-filter the right law and the analytic slope-bias
//!    correction the wrong one);
//! 2. a V-groove is unreachable at its floor, reachable on its flanks, and
//!    monotone in tip radius — with the apex gap pinned to its closed form
//!    `R·(csc β − 1)`, not to an observed number;
//! 3. `unreachable_fraction` is AREA-weighted, on a fixture where vertex
//!    count and area disagree by an order of magnitude;
//! 4. the memo returns the same `Arc` for the same key and a different map
//!    after a tool-radius edit;
//! 5. the tolerance decides the verdict: one gap, two tolerances, two
//!    answers.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::reach_map::{
    ReachMapParams, ReachMapRequest, ReachToleranceSource, compute_reach_map, reach_color,
    reach_colors, reach_map_for_mesh,
};
use rs_cam_core::reach_map_cache;
use rs_cam_core::tier_map::cl_offset_bias_mm;
use rs_cam_core::tool::{
    BallEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition,
};

fn never_cancel() -> impl Fn() -> bool + Send + Sync {
    || false
}

/// A single sloped plane spanning `x ∈ [0, size]`, `y ∈ [0, size]`, rising at
/// `slope_deg` from horizontal along +X. Two triangles, wound CCW seen from
/// +Z so the normals point up.
fn sloped_plane(size: f64, slope_deg: f64) -> TriangleMesh {
    let rise = slope_deg.to_radians().tan();
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(size, 0.0, size * rise),
        P3::new(size, size, size * rise),
        P3::new(0.0, size, 0.0),
    ];
    TriangleMesh::from_raw(vertices, vec![[0, 1, 2], [0, 2, 3]])
}

/// A V trench running along Y with its apex on `x = 0`, each wall standing
/// `half_angle_deg` from the vertical centreline.
///
/// `half_width` is the XY half-width of the trench, so each wall rises to
/// `half_width / tan(half_angle)`. `cols` and `rows` set the tessellation —
/// the knob the area-weighting sentry turns.
fn v_trench(half_width: f64, half_angle_deg: f64, length: f64, cols: usize, rows: usize) -> Mesh {
    let cot = 1.0 / half_angle_deg.to_radians().tan();
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let nx = cols.max(2);
    let ny = rows.max(1);
    for r in 0..=ny {
        let y = length * r as f64 / ny as f64;
        for c in 0..=nx {
            let x = -half_width + 2.0 * half_width * c as f64 / nx as f64;
            vertices.push(P3::new(x, y, x.abs() * cot));
        }
    }
    let stride = nx + 1;
    for r in 0..ny {
        for c in 0..nx {
            let a = (r * stride + c) as u32;
            let b = a + 1;
            let d = ((r + 1) * stride + c) as u32;
            let e = d + 1;
            triangles.push([a, b, e]);
            triangles.push([a, e, d]);
        }
    }
    Mesh {
        vertices,
        triangles,
    }
}

/// Raw vertex + triangle arrays, so two fixtures can be welded into one mesh
/// before `from_raw` recomputes the faces and the bbox.
struct Mesh {
    vertices: Vec<P3>,
    triangles: Vec<[u32; 3]>,
}

impl Mesh {
    fn merged_with(mut self, other: Mesh) -> TriangleMesh {
        let offset = self.vertices.len() as u32;
        self.vertices.extend(other.vertices);
        self.triangles.extend(
            other
                .triangles
                .iter()
                .map(|t| [t[0] + offset, t[1] + offset, t[2] + offset]),
        );
        TriangleMesh::from_raw(self.vertices, self.triangles)
    }

    fn into_mesh(self) -> TriangleMesh {
        TriangleMesh::from_raw(self.vertices, self.triangles)
    }
}

/// A coarse flat pad at `z = 0`, two triangles however big it is — the "large
/// area, few vertices" half of the area-weighting fixture.
fn flat_pad(x0: f64, x1: f64, y0: f64, y1: f64) -> Mesh {
    Mesh {
        vertices: vec![
            P3::new(x0, y0, 0.0),
            P3::new(x1, y0, 0.0),
            P3::new(x1, y1, 0.0),
            P3::new(x0, y1, 0.0),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

// ── 1. A plane is reachable, at any slope, by any profile ──────────────

#[test]
fn a_flat_plane_is_fully_reachable_by_any_ball() {
    let mesh = sloped_plane(40.0, 0.0);
    let ball = BallEndmill::new(6.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.5);

    assert!(map.is_measured(), "the fixture must produce a population");
    assert!(
        map.max_gap_mm < 1e-6,
        "a horizontal plane leaves no gap; got {} mm",
        map.max_gap_mm
    );
    assert!(
        map.unreachable_fraction.abs() < 1e-12,
        "a horizontal plane is 100 % reachable; got {:.4} %",
        map.unreachable_pct()
    );
}

#[test]
fn a_sloped_plane_is_reachable_and_the_bias_it_cancels_is_large() {
    // The point of the min-filter, stated as a number: on this plane the raw
    // drop-cutter residual against the true mesh IS the tool-centre-offset
    // bias, and the tier map's own closed form says how big that is. If the
    // reach map ever reports something of that order here, it has stopped
    // measuring reach and started measuring slope.
    let ball = BallEndmill::new(6.0, 25.0);
    for slope_deg in [15.0_f64, 30.0, 45.0, 60.0] {
        let mesh = sloped_plane(40.0, slope_deg);
        let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.4);
        let bias = cl_offset_bias_mm(ball.cusp_radius_mm(), slope_deg).unwrap();
        assert!(
            bias > 0.1,
            "the fixture must actually carry a bias at {slope_deg} deg; got {bias} mm"
        );
        assert!(
            map.max_gap_mm < 0.05,
            "a {slope_deg} deg plane must read reachable; got {:.4} mm against a \
             {bias:.4} mm uncorrected bias",
            map.max_gap_mm
        );
        assert!(
            map.unreachable_fraction < 0.02,
            "a {slope_deg} deg plane must be reachable; got {:.2} %",
            map.unreachable_pct()
        );
    }
}

#[test]
fn a_flat_endmill_on_a_slope_is_reachable_too() {
    // The spherical-tip law would subtract `R·(sec θ − 1)` here and leave
    // `R·(tan θ − sec θ + 1)` = 1.76 mm on a Ø6 flat at 45 deg. The profile
    // min-filter has no such arm.
    let mesh = sloped_plane(40.0, 45.0);
    let flat = FlatEndmill::new(6.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &flat, 0.05, 0.4);
    assert!(map.is_measured());
    assert!(
        map.max_gap_mm < 0.05,
        "a flat endmill on a 45 deg plane must read reachable; got {:.4} mm at {}, \
         floor {:.4} mm, cell {:.3} mm",
        map.max_gap_mm,
        worst_cell(&map),
        map.discretisation_floor_mm,
        map.cell_mm
    );
}

/// Where the worst gap sits, in world XY — so a failure names a place, not
/// just a number.
fn worst_cell(map: &rs_cam_core::reach_map::ReachMap) -> String {
    let mut best = (f32::NEG_INFINITY, 0usize);
    for (i, cell) in map.cells.iter().enumerate() {
        if let Some(gap) = cell
            && *gap > best.0
        {
            best = (*gap, i);
        }
    }
    let (row, col) = (best.1 / map.nx.max(1), best.1 % map.nx.max(1));
    format!(
        "({:.2}, {:.2})",
        map.origin_x + col as f64 * map.cell_mm,
        map.origin_y + row as f64 * map.cell_mm
    )
}

// ── 2. A V-groove: floor unreachable, flanks reachable, monotone in R ──

/// The closed form for a ball of radius `R` bottomed out in a V whose walls
/// stand `beta` from the vertical centreline: the tip rests
/// `R·(csc β − 1)` above the apex.
fn apex_gap_mm(radius_mm: f64, half_angle_deg: f64) -> f64 {
    radius_mm * (1.0 / half_angle_deg.to_radians().sin() - 1.0)
}

#[test]
fn a_v_groove_is_unreachable_at_the_floor_and_reachable_on_the_flanks() {
    let half_angle = 30.0_f64;
    let mesh = v_trench(8.0, half_angle, 30.0, 80, 30).into_mesh();
    let ball = BallEndmill::new(3.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.3);

    let expected = apex_gap_mm(ball.cusp_radius_mm(), half_angle);
    let apex = map.gap_at(0.0, 15.0).expect("the apex must be measured");
    assert!(
        (f64::from(apex) - expected).abs() < 0.05,
        "apex gap should be the closed form {expected:.4} mm; got {apex:.4} mm"
    );

    let flank = map.gap_at(5.0, 15.0).expect("the flank must be measured");
    assert!(
        f64::from(flank) < 0.05,
        "a plain wall is a plane and must read reachable; got {flank:.4} mm"
    );

    assert!(
        map.unreachable_fraction > 0.0 && map.unreachable_fraction < 1.0,
        "the groove must split into reached and unreached; got {:.2} %",
        map.unreachable_pct()
    );
}

#[test]
fn a_smaller_ball_reaches_more_of_the_same_groove() {
    let half_angle = 30.0_f64;
    let mesh = v_trench(8.0, half_angle, 30.0, 80, 30).into_mesh();
    let mut previous = f64::INFINITY;
    for diameter in [6.0_f64, 3.0, 1.0] {
        let ball = BallEndmill::new(diameter, 25.0);
        let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.3);
        let expected = apex_gap_mm(ball.cusp_radius_mm(), half_angle);
        assert!(
            (map.max_gap_mm - expected).abs() < 0.1,
            "Ø{diameter} apex gap should be {expected:.4} mm; got {:.4} mm",
            map.max_gap_mm
        );
        assert!(
            map.unreachable_fraction < previous,
            "Ø{diameter} must reach MORE than the tool before it; \
             {:.4} is not below {previous:.4}",
            map.unreachable_fraction
        );
        previous = map.unreachable_fraction;
    }
}

// ── 3. The fraction is area-weighted, not vertex-counted ───────────────

#[test]
fn unreachable_fraction_is_area_weighted_not_vertex_counted() {
    // A finely tessellated 16 x 30 mm groove (thousands of vertices, ~1100
    // mm2 of wall) beside a coarsely tessellated 100 x 30 mm pad (four
    // vertices, 3000 mm2). Vertex counting would put almost all the weight
    // on the groove; area weighting must not.
    let groove = v_trench(8.0, 30.0, 30.0, 80, 30);
    let mesh = groove.merged_with(flat_pad(20.0, 120.0, 0.0, 30.0));
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(3.0, 25.0);
    let params = ReachMapParams {
        cell_mm: 0.4,
        tolerance_mm: 0.05,
        margin_mm: 0.5,
    };
    let map = compute_reach_map(&mesh, &index, &ball, &params, &never_cancel()).unwrap();

    assert!(map.is_measured());
    assert!(
        (map.unreachable_fraction - map.unreachable_area_mm2 / map.measured_area_mm2).abs() < 1e-12,
        "the published fraction must be the published areas' ratio"
    );

    let gaps = map.vertex_gaps(&mesh, &index);
    assert_eq!(
        gaps.len(),
        mesh.vertices.len(),
        "one gap per model vertex, always"
    );
    let measured_vertices = gaps.iter().filter(|g| g.is_finite()).count();
    let unreachable_vertices = gaps
        .iter()
        .filter(|g| g.is_finite() && f64::from(**g) > map.tolerance_mm)
        .count();
    assert!(measured_vertices > 0);
    let vertex_fraction = unreachable_vertices as f64 / measured_vertices as f64;

    assert!(
        vertex_fraction > 2.0 * map.unreachable_fraction,
        "the fixture must make the two measures disagree — vertex {vertex_fraction:.4} \
         against area {:.4}",
        map.unreachable_fraction
    );
    assert!(
        map.unreachable_fraction < 0.15,
        "area weighting must let the big coarse pad dominate; got {:.2} %",
        map.unreachable_pct()
    );
}

#[test]
fn a_colour_vector_has_one_entry_per_model_vertex() {
    let mesh = v_trench(8.0, 30.0, 30.0, 20, 10).into_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(3.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.5);
    let gaps = map.vertex_gaps(&mesh, &index);
    let floors = map.vertex_floors(&mesh);
    let ramp = map.ramp();
    let colors = reach_colors(&gaps, &floors, ramp);
    assert_eq!(colors.len(), mesh.vertices.len());
    assert_eq!(
        colors,
        map.vertex_colors(&mesh, &index),
        "`vertex_colors` must be the same one pass the vector form gives, or \
         the worker and a caller that assembles it by hand would drift"
    );
    // FOUR readings the overlay must not mix (P5.2): reached is
    // green-dominant, unresolved is neutral grey, a miss is red-dominant and
    // DEEPENS with the gap, and not measured is the model mesh's own colour.
    let probe = rs_cam_core::reach_map::ReachRamp {
        tolerance_mm: 0.05,
        floor_mm: 0.05,
        max_gap_mm: 4.0,
    };
    let reached = reach_color(0.0, f32::NAN, probe);
    let missed = reach_color(1.0, f32::NAN, probe);
    let absent = reach_color(f32::NAN, f32::NAN, probe);
    let unresolved = reach_color(0.09, 0.10, probe);
    let shallow = reach_color(0.06, f32::NAN, probe);
    let deep = reach_color(4.0, f32::NAN, probe);
    assert!(reached[1] > reached[0], "reached must read green");
    assert!(missed[0] > missed[1], "unreachable must read red");
    assert!(
        (unresolved[0] - unresolved[1]).abs() < 0.05
            && (unresolved[1] - unresolved[2]).abs() < 0.05,
        "an unresolved vertex must read as neutral grey, not as either \
         verdict; got {unresolved:?}"
    );
    // The whole point of the depth ramp: a near-miss and a gorge floor are
    // not the same colour, and the deep end is DARKER.
    let lum = |c: [f32; 3]| c[0] + c[1] + c[2];
    assert!(
        lum(shallow) > lum(deep) + 0.5,
        "the depth ramp must darken with the gap: a 0.06 mm miss {shallow:?} \
         against a 4.0 mm gorge {deep:?}"
    );
    assert!(
        (shallow[1] - deep[1]).abs() > 0.3,
        "a near-miss and a gorge floor must not be the same red"
    );
    assert_eq!(
        absent,
        [0.6, 0.55, 0.5],
        "not measured keeps the mesh colour"
    );
}

// ── 4. The memo ────────────────────────────────────────────────────────

fn request_for(mesh: &Arc<TriangleMesh>, diameter: f64, tolerance_mm: f64) -> ReachMapRequest {
    let cutter = Arc::new(ToolDefinition::new(
        Box::new(BallEndmill::new(diameter, 25.0)),
        6.0,
        20.0,
        20.0,
        40.0,
        2,
        rs_cam_core::compute::tool_config::ToolMaterial::Carbide,
    ));
    ReachMapRequest {
        mesh: Arc::clone(mesh),
        index: Arc::new(SpatialIndex::build_auto(mesh.as_ref())),
        // The production rule: the cell follows the TOOL and the model, and
        // the tolerance classifies on it (F5, 2026-09-08).
        params: ReachMapParams::for_cutter(cutter.as_ref(), tolerance_mm),
        cutter,
        tool_id: 7,
        model_id: 3,
        tolerance_source: ReachToleranceSource::CallerOverride,
    }
}

#[test]
fn the_memo_hits_on_the_same_key_and_rebuilds_after_a_radius_edit() {
    reach_map_cache::clear();
    reach_map_cache::reset_stats();

    let mesh = Arc::new(v_trench(8.0, 30.0, 20.0, 40, 16).into_mesh());
    let request = request_for(&mesh, 3.0, 0.05);

    let first = reach_map_cache::cached_reach_map(&request, &never_cancel()).unwrap();
    let second = reach_map_cache::cached_reach_map(&request, &never_cancel()).unwrap();
    assert!(
        Arc::ptr_eq(&first, &second),
        "the same key must return the SAME Arc, not an equal rebuild"
    );
    let stats = reach_map_cache::stats();
    assert_eq!(stats.builds, 1, "one build for two identical calls");
    assert_eq!(stats.hits, 1);

    // A tool-radius edit is a different shape key, so it misses. Nothing
    // invalidates explicitly — the key IS the invalidation.
    let edited = request_for(&mesh, 1.0, 0.05);
    let third = reach_map_cache::cached_reach_map(&edited, &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&first, &third), "a smaller tool is a new map");
    assert!(
        third.unreachable_fraction < first.unreachable_fraction,
        "and the smaller tool must reach more"
    );
    assert_eq!(reach_map_cache::stats().builds, 2);

    // The ids ride on the answer, and they are in the key, so a map is never
    // served under another toolpath's name.
    assert_eq!(first.tool_id, Some(7));
    assert_eq!(first.model_id, Some(3));

    reach_map_cache::clear();
    reach_map_cache::reset_stats();
}

// ── 5. The tolerance decides the verdict ───────────────────────────────

#[test]
fn one_gap_two_tolerances_two_verdicts() {
    let half_angle = 30.0_f64;
    let mesh = v_trench(8.0, half_angle, 20.0, 40, 16).into_mesh();
    let ball = BallEndmill::new(3.0, 25.0);
    let gap = apex_gap_mm(ball.cusp_radius_mm(), half_angle);

    let tight = reach_map_for_mesh(&mesh, &ball, 0.05, 0.4);
    let loose = reach_map_for_mesh(&mesh, &ball, gap * 2.0, 0.4);

    assert!(
        (tight.max_gap_mm - loose.max_gap_mm).abs() < 0.15,
        "the GEOMETRY does not move with the tolerance: {:.4} against {:.4}",
        tight.max_gap_mm,
        loose.max_gap_mm
    );
    assert!(
        tight.unreachable_fraction > 0.0,
        "the apex must fail a tolerance well below its {gap:.4} mm gap"
    );
    assert!(
        loose.unreachable_fraction.abs() < 1e-12,
        "and pass one well above it; got {:.4} %",
        loose.unreachable_pct()
    );
}

// ── The population, and the honesty of an absent one ───────────────────

#[test]
fn a_map_over_a_downward_facing_mesh_reports_no_population() {
    // Every triangle faces down, so nothing is in the top-down population.
    // The fraction reads 0.0 — which is exactly why `is_measured` exists and
    // why a reader must consult it before believing a clean percentage.
    let flat = sloped_plane(20.0, 0.0);
    let flipped = flat.z_flipped();
    let ball = BallEndmill::new(3.0, 25.0);
    let map = reach_map_for_mesh(&flipped, &ball, 0.05, 0.5);
    assert!(
        !map.is_measured(),
        "an all-underside mesh has no top-down population"
    );
    assert!(map.unreachable_fraction.abs() < 1e-12);
}

/// Instrument, not a bar. Measured 2026-09-08 on a **debug** build:
/// 320 000 triangles over 200 x 200 mm, cell 0.600 mm, 119 025 cells,
/// **2.28 s**, sampling floor 0.010 mm. That is the answer to the operator's
/// "if it is cheap": a cold map is seconds off the UI thread and a warm one
/// is a memo hit.
///
/// The second figure matters as much. `ReachMapParams::for_cutter` runs on
/// the UI thread's own selection path, and on the shipped tapered ball it
/// took **100 us** in the same debug build while it still bisected a profile
/// sweep. F5 (2026-09-08) retired the bisection — the cell follows the tool's
/// tip sphere and the model, never the tolerance — so the figure this
/// instrument now prints should be a small fraction of that. It is kept
/// because the call is still on a click path and a future rule could put
/// work back on it.
///
/// The assertion below is a generous ceiling that catches an
/// order-of-magnitude regression, never a timing bar.
#[test]
#[ignore = "instrument: cold-build wall clock on a board-sized terrain"]
fn cold_build_wall_clock_on_a_board_sized_terrain() {
    // 200 x 200 mm terrain, ~640 k triangles — the reference board's scale.
    let n = 400usize;
    let mut vertices = Vec::with_capacity((n + 1) * (n + 1));
    for r in 0..=n {
        for c in 0..=n {
            let x = 200.0 * c as f64 / n as f64;
            let y = 200.0 * r as f64 / n as f64;
            let z = 6.0 * (x / 17.0).sin() * (y / 23.0).cos();
            vertices.push(P3::new(x, y, z));
        }
    }
    let stride = n + 1;
    let mut triangles = Vec::with_capacity(n * n * 2);
    for r in 0..n {
        for c in 0..n {
            let a = (r * stride + c) as u32;
            triangles.push([a, a + 1, a + stride as u32 + 1]);
            triangles.push([a, a + stride as u32 + 1, a + stride as u32]);
        }
    }
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(3.0, 25.0);
    // `for_cutter` runs on the UI thread's selection path, so its own cost
    // is part of the answer. The tapered ball is the shape whose tip radius
    // and envelope radius disagree by 6x, so it is the one a cell rule can
    // get wrong.
    let taper = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let params_start = std::time::Instant::now();
    let mut taper_cell = 0.0;
    for _ in 0..100 {
        taper_cell = ReachMapParams::for_cutter(&taper, 0.05).cell_mm;
    }
    let params_us = params_start.elapsed().as_secs_f64() * 1e4;
    let params = ReachMapParams::for_cutter(&ball, 0.05);
    let start = std::time::Instant::now();
    let map = compute_reach_map(&mesh, &index, &ball, &params, &never_cancel()).unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 60.0,
        "INSTRUMENT: {} triangles, cell {:.3} mm, {} cells, {:.3} s, floor {:.5} mm, \
         unreachable {:.2} %, max gap {:.4} mm; for_cutter on a tapered ball \
         {params_us:.1} us -> cell {taper_cell:.3} mm",
        mesh.faces.len(),
        map.cell_mm,
        map.cells.len(),
        elapsed.as_secs_f64(),
        map.discretisation_floor_mm,
        map.unreachable_pct(),
        map.max_gap_mm
    );
}

#[test]
fn the_rim_band_abstains_rather_than_reporting_its_own_truncation() {
    // Within one envelope radius of the footprint edge the min-filter has no
    // CL positions to draw on, so the map says NOT MEASURED there. It must
    // not report a number it cannot know.
    let mesh = sloped_plane(40.0, 45.0);
    let flat = FlatEndmill::new(6.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &flat, 0.05, 0.4);
    assert!(
        (map.rim_erosion_mm - flat.envelope_radius_mm()).abs() < 1e-12,
        "the eroded band is one envelope radius"
    );
    assert!(map.gap_at(0.0, 20.0).is_none(), "the edge abstains");
    assert!(
        map.gap_at(20.0, 20.0).is_some(),
        "the middle of the plane is measured"
    );
    // The abstention is visible on the grid, not silently folded away.
    let (_, _, not_measured) = map.gap_histogram(8);
    let measured = map.cells.iter().filter(|c| c.is_some()).count();
    assert!(measured > 0 && not_measured > 0);
    // A two-triangle plane still reports its whole area as measured, because
    // the area verdict samples each triangle at its CENTROID and both
    // centroids are well inside the band. That is the documented limit of
    // centroid sampling on a coarse mesh, stated here so it cannot be read
    // as the rim erosion failing.
    assert!((map.measured_area_mm2 - map.surface_area_mm2).abs() < 1e-9);
}

// ── The op set, and the kernel budget ──────────────────────────────────

#[test]
fn exactly_ten_operations_carry_a_reach_map() {
    use rs_cam_core::compute::catalog::OperationType;
    // Derived from the registry (mesh geometry, not a roughing role), but
    // PINNED here as a list. A role edit elsewhere would otherwise move the
    // overlay's coverage silently, and the doc on `supports_reach_map` says
    // "exactly ten" in those words.
    let expected = [
        "drop_cutter",
        "waterline",
        "pencil",
        "scallop",
        "unified_finish",
        "steep_shallow",
        "ramp_finish",
        "spiral_finish",
        "radial_finish",
        "horizontal_finish",
    ];
    let mut actual: Vec<&str> = OperationType::ALL
        .iter()
        .filter(|op| op.supports_reach_map())
        .map(|op| op.kind_str())
        .collect();
    actual.sort_unstable();
    let mut want = expected;
    want.sort_unstable();
    assert_eq!(actual, want.to_vec());

    // The three the boundary is drawn against, named so a future reader sees
    // the intent rather than a count.
    assert!(
        OperationType::Waterline.supports_reach_map(),
        "waterline rides the same surface, semi-finish role or not"
    );
    assert!(
        !OperationType::Adaptive3d.supports_reach_map(),
        "a rough leaves stock everywhere on purpose"
    );
    assert!(
        !OperationType::ProjectCurve.supports_reach_map(),
        "a projected curve's reach question is about a curve, not a surface"
    );
}

#[test]
fn a_big_tool_on_a_fine_cell_keeps_its_envelope_tap() {
    // The kernel has a tap budget. It must coarsen the sampling pitch to
    // stay inside it, never stop emitting rings — the rings run inner to
    // outer, so truncation would drop `r = envelope`.
    //
    // The tool is a FLAT endmill deliberately: its binding radius IS the
    // envelope, so a truncated ring list shows up immediately as
    // `(envelope − last kept radius)·tan θ` of gap. A ball at 45 deg binds
    // at `R·sin θ`, well inside the envelope, and would pass this fixture
    // even with the outer ring missing. A Ø16 tool at a 0.25 mm cell is
    // roughly six times over the budget on the un-coarsened pitch.
    let mesh = sloped_plane(60.0, 45.0);
    let flat = FlatEndmill::new(16.0, 40.0);
    let map = reach_map_for_mesh(&mesh, &flat, 0.05, 0.25);
    assert!(map.is_measured(), "the fixture must produce a population");
    assert!(
        map.max_gap_mm < 0.05,
        "a 45 deg plane must read reachable for a Ø16 flat too; got {:.4} mm, \
         floor {:.5} mm",
        map.max_gap_mm,
        map.discretisation_floor_mm
    );
}

/// The map says which WAY its own error runs, and never the wrong way.
///
/// `machined_z` is a minimum over a sampled CL set, so it sits at or above
/// the continuum minimum and the reported gap is at or above the true gap.
/// The bias is non-negative, so the unreachable percentage OVER-states and
/// the truth is at or BELOW it. Four operator surfaces said "lower bound"
/// instead — exactly inverted — at the one moment a reader is being told to
/// distrust the number. Measured on the wanaka board against an independent
/// closing on the same area base: 59.05 % reported against 58.6 % true at the
/// 0.05 mm bar, 51.11 against 42.1 at 0.146, 36.36 against 26.8 at 0.30.
///
/// The strings are built in `ReachMap` and quoted by the panel, the MCP reply
/// and the inspector, so this one assertion covers all three.
#[test]
fn the_grid_note_says_the_bias_over_states_and_names_its_area_base() {
    // A V-groove with a bar far under the floor, so the below-floor arm runs.
    let mesh = v_trench(8.0, 30.0, 30.0, 80, 30).into_mesh();
    let ball = BallEndmill::new(3.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &ball, 0.001, 0.4);
    assert!(map.is_measured(), "the fixture must produce a population");
    assert!(
        map.tolerance_below_floor(),
        "the fixture must put the bar under the floor, or the arm under test \
         never runs: tol {:.4} against floor {:.4}",
        map.tolerance_mm,
        map.discretisation_floor_mm
    );

    let note = map.grid_note();
    let lower = note.to_lowercase();
    assert!(
        !lower.contains("lower bound"),
        "the grid note claims a LOWER bound; the sampled minimum can only \
         over-state, so the truth is at or BELOW the figure. Got: {note}"
    );
    assert!(
        note.contains("AT OR BELOW"),
        "the grid note must say which way the error runs. Got: {note}"
    );
    assert!(
        note.contains("over-states"),
        "the grid note must name the direction of the bias. Got: {note}"
    );
    // P5.2: the base, in words, beside every percentage — an unstated base is
    // what turned a 3.5-point weighting difference into a hunt for a phantom
    // instrument defect.
    assert!(
        note.contains("3D surface area") && note.contains("rim-eroded"),
        "the grid note must state the area base. Got: {note}"
    );
    assert!(
        map.area_basis_note().contains("rim-eroded"),
        "the shared area-basis string is the one the panel prints"
    );
    // And the over-statement sentence is one construction site, so the three
    // surfaces cannot describe the bias three ways.
    assert!(
        note.contains(&map.over_statement_note()),
        "the grid note must quote the shared over-statement sentence"
    );
}
