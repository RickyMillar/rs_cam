//! End-to-end integration tests: STL -> drop-cutter -> G-code,
//! pocket + profile on same geometry, SVG -> pocket -> G-code,
//! and tri-dexel stock simulation.

use rs_cam_core::{
    dexel_stock::{StockCutDirection, TriDexelStock},
    dropcutter::batch_drop_cutter,
    gcode::{GrblPost, emit_gcode},
    geo::{BoundingBox3, P3},
    mesh::{SpatialIndex, TriangleMesh},
    pocket::{PocketParams, pocket_toolpath},
    polygon::Polygon2,
    profile::{ProfileParams, ProfileSide, profile_toolpath},
    svg_input::load_svg_data,
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::{MoveType, Toolpath, raster_toolpath_from_grid},
};
use std::path::Path;

#[test]
fn test_terrain_stl_to_gcode() {
    let stl_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join("terrain_small.stl");

    if !stl_path.exists() {
        eprintln!("Skipping: fixture not found at {:?}", stl_path);
        return;
    }

    // Load
    let mesh = TriangleMesh::from_stl(&stl_path).expect("Failed to load STL");
    assert!(
        mesh.faces.len() > 1000,
        "Expected a real mesh, got {} triangles",
        mesh.faces.len()
    );

    // Index
    let tool = BallEndmill::new(6.35, 25.0);
    let index = SpatialIndex::build(&mesh, tool.diameter() * 2.0);

    // Drop-cutter
    let grid = batch_drop_cutter(&mesh, &index, &tool, 2.0, 0.0, -50.0);
    assert!(grid.rows > 5);
    assert!(grid.cols > 5);

    // Verify Z values are within mesh bounds (with some margin for the cutter radius)
    let margin = tool.diameter();
    for cl in &grid.points {
        assert!(cl.z >= -50.0 - 1e-6, "CL.z {} below min_z", cl.z);
        assert!(
            cl.z <= mesh.bbox.max.z + margin,
            "CL.z {} above mesh max {} + margin {}",
            cl.z,
            mesh.bbox.max.z,
            margin
        );
    }

    // Generate toolpath
    let toolpath = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0);
    assert!(toolpath.moves.len() > 100);
    assert!(toolpath.total_cutting_distance() > 0.0);

    // Emit G-code
    let gcode = emit_gcode(&toolpath, &GrblPost, 18000);
    assert!(gcode.contains("G17 G21 G90"));
    assert!(gcode.contains("M3 S18000"));
    assert!(gcode.contains("G0"));
    assert!(gcode.contains("G1"));
    assert!(gcode.contains("M30"));
    assert!(gcode.len() > 1000);
}

#[test]
fn test_programmatic_hemisphere_to_gcode() {
    use rs_cam_core::mesh::make_test_hemisphere;

    let mesh = make_test_hemisphere(20.0, 16);
    let tool = BallEndmill::new(6.35, 25.0);
    let index = SpatialIndex::build(&mesh, tool.diameter() * 2.0);

    let grid = batch_drop_cutter(&mesh, &index, &tool, 1.0, 0.0, -30.0);

    // Center point should be at approximately z = hemisphere_radius
    let center_row = grid.rows / 2;
    let center_col = grid.cols / 2;
    let center_cl = grid.get(center_row, center_col);
    assert!(
        (center_cl.z - 20.0).abs() < 1.0,
        "Center CL.z = {}, expected ~20.0",
        center_cl.z
    );

    let toolpath = raster_toolpath_from_grid(&grid, 1000.0, 500.0, 25.0);
    let gcode = emit_gcode(&toolpath, &GrblPost, 18000);
    assert!(gcode.len() > 500);
}

// ---------------------------------------------------------------------------
// E-e2e1: Pocket + Profile on the same polygon geometry
// ---------------------------------------------------------------------------

fn make_test_rectangle() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 40.0, 30.0)
}

/// Helper: check that no move target contains NaN coordinates.
fn assert_no_nan_coords(tp: &Toolpath, label: &str) {
    for (i, m) in tp.moves.iter().enumerate() {
        assert!(
            !m.target.x.is_nan() && !m.target.y.is_nan() && !m.target.z.is_nan(),
            "{label}: move {i} contains NaN coordinate: {:?}",
            m.target
        );
        match m.move_type {
            MoveType::ArcCW { i, j, feed_rate }
            | MoveType::ArcCCW { i, j, feed_rate } => {
                assert!(
                    !i.is_nan() && !j.is_nan() && !feed_rate.is_nan(),
                    "{label}: arc move {0} has NaN arc params",
                    0
                );
            }
            MoveType::Linear { feed_rate } => {
                assert!(
                    !feed_rate.is_nan(),
                    "{label}: linear move {} has NaN feed_rate",
                    i
                );
            }
            MoveType::Rapid => {}
        }
    }
}

#[test]
fn pocket_and_profile_on_same_polygon() {
    let polygon = make_test_rectangle();

    // ---- Pocket ----
    let pocket_params = PocketParams {
        tool_radius: 3.175,
        stepover: 2.0,
        cut_depth: -3.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: false,
    };
    let pocket_tp = pocket_toolpath(&polygon, &pocket_params);
    assert!(
        !pocket_tp.moves.is_empty(),
        "Pocket toolpath should be non-empty"
    );

    // ---- Profile (outside) ----
    let profile_params = ProfileParams {
        tool_radius: 3.175,
        side: ProfileSide::Outside,
        cut_depth: -3.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: false,
    };
    let profile_tp = profile_toolpath(&polygon, &profile_params);
    assert!(
        !profile_tp.moves.is_empty(),
        "Profile toolpath should be non-empty"
    );

    // ---- Profile stays outside/on polygon boundary ----
    // Profile outside: tool center is offset outward by tool_radius,
    // so all XY coords at cut depth should be outside or on boundary.
    // We check that all cutting-move XY coords are at least not deeply
    // inside the polygon (allowing small numerical tolerance).
    let cutting_profile_pts: Vec<_> = profile_tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .filter(|m| m.target.z < 0.0) // cutting moves (below surface)
        .map(|m| rs_cam_core::geo::P2::new(m.target.x, m.target.y))
        .collect();
    assert!(
        !cutting_profile_pts.is_empty(),
        "Profile should have cutting moves"
    );
    // For an outside profile on a 40x30 rect with 3.175 tool radius,
    // the tool center should be outside the original polygon.
    for pt in &cutting_profile_pts {
        assert!(
            !polygon.contains_point(pt),
            "Profile (outside) cutting point ({:.3}, {:.3}) should be outside polygon",
            pt.x,
            pt.y
        );
    }

    // ---- Pocket stays inside polygon boundary ----
    let cutting_pocket_pts: Vec<_> = pocket_tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .filter(|m| m.target.z < 0.0)
        .map(|m| rs_cam_core::geo::P2::new(m.target.x, m.target.y))
        .collect();
    assert!(
        !cutting_pocket_pts.is_empty(),
        "Pocket should have cutting moves"
    );
    for pt in &cutting_pocket_pts {
        assert!(
            polygon.contains_point(pt),
            "Pocket cutting point ({:.3}, {:.3}) should be inside polygon",
            pt.x,
            pt.y
        );
    }

    // ---- No NaN coordinates ----
    assert_no_nan_coords(&pocket_tp, "pocket");
    assert_no_nan_coords(&profile_tp, "profile");
}

// ---------------------------------------------------------------------------
// E-e2e2: Import SVG -> extract polygons -> pocket -> G-code
// ---------------------------------------------------------------------------

#[test]
fn svg_import_pocket_gcode() {
    // Minimal SVG with a rectangle: 10x10 at (5,5)
    let svg_data = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">
  <rect x="5" y="5" width="30" height="20" fill="none" stroke="black" stroke-width="1" />
</svg>"#;

    let polygons = load_svg_data(svg_data, 0.1).expect("SVG should parse successfully");
    assert!(
        !polygons.is_empty(),
        "SVG should produce at least one polygon"
    );
    let polygon = &polygons[0];
    assert!(
        polygon.exterior.len() >= 3,
        "Polygon should have at least 3 vertices"
    );

    // Run pocket operation
    let pocket_params = PocketParams {
        tool_radius: 2.0,
        stepover: 1.5,
        cut_depth: -2.0,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        safe_z: 5.0,
        climb: false,
    };
    let pocket_tp = pocket_toolpath(polygon, &pocket_params);
    assert!(
        !pocket_tp.moves.is_empty(),
        "Pocket toolpath from SVG polygon should be non-empty"
    );

    // Emit G-code
    let gcode = emit_gcode(&pocket_tp, &GrblPost, 18000);

    // Verify expected G-code patterns
    assert!(gcode.contains("G0"), "G-code should contain rapid moves (G0)");
    assert!(
        gcode.contains("G1"),
        "G-code should contain linear feed moves (G1)"
    );
    assert!(
        gcode.contains("F800"),
        "G-code should contain cutting feedrate"
    );
    assert!(
        gcode.contains("F400"),
        "G-code should contain plunge feedrate"
    );
    assert!(
        gcode.contains("M3 S18000"),
        "G-code should contain spindle start"
    );
    assert!(
        gcode.contains("M30"),
        "G-code should contain program end"
    );
    assert!(
        gcode.len() > 200,
        "G-code output should be substantial, got {} bytes",
        gcode.len()
    );
}

#[test]
fn svg_fixture_file_import_pocket_gcode() {
    let svg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join("demo_pocket.svg");

    if !svg_path.exists() {
        eprintln!("Skipping: fixture not found at {:?}", svg_path);
        return;
    }

    let polygons = rs_cam_core::svg_input::load_svg(&svg_path, 0.1)
        .expect("demo_pocket.svg should parse");
    assert!(
        !polygons.is_empty(),
        "demo_pocket.svg should produce polygons"
    );

    // Run pocket on the first polygon
    let pocket_params = PocketParams {
        tool_radius: 2.0,
        stepover: 1.5,
        cut_depth: -2.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: false,
    };
    let pocket_tp = pocket_toolpath(&polygons[0], &pocket_params);
    assert!(!pocket_tp.moves.is_empty());

    let gcode = emit_gcode(&pocket_tp, &GrblPost, 18000);
    assert!(gcode.contains("G0"));
    assert!(gcode.contains("G1"));
    assert!(gcode.contains("M30"));
}

// ---------------------------------------------------------------------------
// E-e2e3: Tri-dexel stock simulation test
// ---------------------------------------------------------------------------

#[test]
fn tridexel_simulation_modifies_stock() {
    // Create a stock block
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(50.0, 50.0, 25.0),
    };
    let cell_size = 1.0;
    let mut stock = TriDexelStock::from_bounds(&bbox, cell_size);

    // Create a simple toolpath: plunge + linear cut across the stock
    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(25.0, 25.0, 30.0)); // above stock
    toolpath.feed_to(P3::new(25.0, 25.0, 20.0), 500.0); // plunge into stock
    toolpath.feed_to(P3::new(25.0, 40.0, 20.0), 1000.0); // linear cut

    // Use a flat endmill for simulation
    let tool = FlatEndmill::new(6.35, 25.0);

    // Snapshot the stock before simulation
    let stock_before = stock.clone();

    // Simulate
    stock.simulate_toolpath(&toolpath, &tool, StockCutDirection::FromTop);

    // Verify the stock was modified: at least some rays should have
    // a different top-Z after simulation
    let mut modified_count = 0;
    for (i, (before_ray, after_ray)) in stock_before
        .z_grid
        .rays
        .iter()
        .zip(stock.z_grid.rays.iter())
        .enumerate()
    {
        let before_top = rs_cam_core::dexel::ray_top(before_ray);
        let after_top = rs_cam_core::dexel::ray_top(after_ray);
        if before_top != after_top {
            modified_count += 1;
            // After cut, the top should be lower than the original
            if let (Some(bt), Some(at)) = (before_top, after_top) {
                assert!(
                    at <= bt,
                    "Ray {i}: stock top after cut ({at}) should be <= before ({bt})"
                );
            }
        }
    }

    assert!(
        modified_count > 0,
        "Simulation should have modified at least some stock rays, but none changed"
    );
}

#[test]
fn tridexel_simulation_two_toolpaths_carry_forward() {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 40.0, 20.0),
    };
    let cell_size = 1.0;
    let mut stock = TriDexelStock::from_bounds(&bbox, cell_size);

    let tool = FlatEndmill::new(6.35, 25.0);

    // First toolpath: cut a slot at y=10
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(5.0, 10.0, 25.0));
    tp1.feed_to(P3::new(5.0, 10.0, 15.0), 500.0);
    tp1.feed_to(P3::new(35.0, 10.0, 15.0), 1000.0);

    stock.simulate_toolpath(&tp1, &tool, StockCutDirection::FromTop);
    let stock_after_first = stock.clone();

    // Second toolpath: cut another slot at y=20
    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(5.0, 20.0, 25.0));
    tp2.feed_to(P3::new(5.0, 20.0, 15.0), 500.0);
    tp2.feed_to(P3::new(35.0, 20.0, 15.0), 1000.0);

    stock.simulate_toolpath(&tp2, &tool, StockCutDirection::FromTop);

    // Verify: the first cut should still be present (carry-forward)
    // Check a ray along the first slot
    let first_slot_modified = stock_after_first.z_grid.rays.iter().any(|ray| {
        rs_cam_core::dexel::ray_top(ray).is_some_and(|t| (t - 20.0_f32).abs() > 0.01)
    });
    assert!(first_slot_modified, "First slot should have modified stock");

    // After second toolpath, both slots should be cut
    let mut first_slot_still_cut = false;
    let mut second_slot_cut = false;

    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            let idx = row * stock.z_grid.cols + col;
            let top = rs_cam_core::dexel::ray_top(&stock.z_grid.rays[idx]);
            let first_top = rs_cam_core::dexel::ray_top(&stock_after_first.z_grid.rays[idx]);
            if let Some(t) = top
                && (t - 20.0_f32).abs() > 0.01 && t < 20.0
            {
                // This ray was cut
                let y = stock.z_grid.origin_v + row as f64 * stock.z_grid.cell_size;
                if (y - 10.0).abs() < 5.0 {
                    first_slot_still_cut = true;
                }
                if (y - 20.0).abs() < 5.0 {
                    second_slot_cut = true;
                }
            }
            // Also verify first cut wasn't lost
            if let (Some(ft), Some(t)) = (first_top, top)
                && ft < 20.0
            {
                assert!(
                    t <= ft,
                    "Stock should not grow back: first-cut top={ft}, after-second={t}"
                );
            }
        }
    }

    assert!(
        first_slot_still_cut,
        "First slot should still be visible after second toolpath"
    );
    assert!(second_slot_cut, "Second slot should be cut");
}
