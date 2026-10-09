//! S3 sentries — the geometry kernels of a stock change
//! (`planning/stock_additions_2026-10-09/PLAN.md` S3,
//! `planning/stock_fill_2026-10-09/DESIGN.md` §2.1 and §2.2).
//!
//! The simulation applies a group's stock changes, in order, after the S0
//! carry and before the group's first toolpath, to the group stock and to
//! the playback stock. Each test fails on a stub that applies nothing.
//!
//! The stock is 40 x 30 x 20 mm at 0.5 mm cells: nodes at 0, 0.5, ...
//! Each outline rectangle has its edges at half-cell offsets, so the node
//! test is exact and a volume is a whole number of columns: within one cell
//! of the analytic value by construction.

#![allow(
    // SAFETY: test code; a failed fixture is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationError, SimulationRequest, SimulationResult,
    run_simulation,
};
use rs_cam_core::compute::stock_carry::StockCarry;
use rs_cam_core::compute::stock_change::{
    ResolvedStockChange, StockChange, StockChangeOp, StockChangeSource, StockGeometry,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::ids::{ModelId, StockChangeId, ToolpathId};
use rs_cam_core::material::Material;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::stock::material_slot::MaterialSlot;
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

const W: f64 = 40.0;
const D: f64 = 30.0;
const H: f64 = 20.0;
const CELL: f64 = 0.5;

/// The outline: x 10.25..20.25 (nodes 10.5..20.0, 20 columns), y 5.25..15.25
/// (20 rows). 400 nodes, 100 mm² of column area.
const RX0: f64 = 10.25;
const RX1: f64 = 20.25;
const RY0: f64 = 5.25;
const RY1: f64 = 15.25;
const R_AREA: f64 = 100.0;
/// A node inside the outline, off its centre in Y (a mirror shows).
const IN_X: f64 = 15.0;
const IN_Y: f64 = 8.0;

fn tool() -> Arc<ToolDefinition> {
    Arc::new(ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
        20.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    ))
}

fn entry(id: usize, toolpath: Toolpath) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(id),
        name: format!("tp{id}"),
        annotated: Arc::new(AnnotatedToolpath::new(toolpath)),
        tool: tool(),
        flute_count: 2,
        tool_summary: "6mm Flat".into(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 0,
    }
}

/// A short cut far from the outline (x 33..34, y 25), so that a group has
/// an entry and so a `prior_stocks` snapshot of its start stock.
fn spot(id: usize) -> SimToolpathEntry {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(33.0, 25.0, H + 5.0));
    tp.feed_to(P3::new(33.0, 25.0, H - 1.0), 300.0);
    tp.feed_to(P3::new(34.0, 25.0, H - 1.0), 300.0);
    tp.rapid_to(P3::new(34.0, 25.0, H + 5.0));
    entry(id, tp)
}

fn bottom() -> SetupTransformInfo {
    SetupTransformInfo {
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        stock_x: W,
        stock_y: D,
        stock_z: H,
        stock_origin_x: 0.0,
        stock_origin_y: 0.0,
        stock_origin_z: 0.0,
    }
}

fn top_group(entries: Vec<SimToolpathEntry>, changes: Vec<ResolvedStockChange>) -> SimGroupEntry {
    SimGroupEntry {
        toolpaths: entries,
        direction: StockCutDirection::FromTop,
        local_stock_bbox: None,
        local_to_global: None,
        phantom_prior_stock: None,
        stock_changes: changes,
    }
}

fn bottom_group(
    entries: Vec<SimToolpathEntry>,
    changes: Vec<ResolvedStockChange>,
) -> SimGroupEntry {
    let info = bottom();
    SimGroupEntry {
        toolpaths: entries,
        direction: rs_cam_core::compute::simulate::group_stock_cut_direction(info.face_up),
        local_stock_bbox: Some(info.effective_stock_bbox()),
        local_to_global: Some(info),
        phantom_prior_stock: None,
        stock_changes: changes,
    }
}

fn request(groups: Vec<SimGroupEntry>) -> SimulationRequest {
    SimulationRequest {
        groups,
        stock_bbox: BoundingBox3 {
            min: P3::origin(),
            max: P3::new(W, D, H),
        },
        stock_top_z: H,
        resolution: CELL,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
        model_mesh: None,
        kinematics: None,
        display_stride: 1,
    }
}

fn run(req: &SimulationRequest) -> SimulationResult {
    run_simulation(req, &AtomicBool::new(false)).unwrap()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])
}

fn filler() -> Material {
    Material::Custom {
        name: "Filler".to_owned(),
        feed_scale_factor: 1.0,
    }
}

fn change(id: usize, op: StockChangeOp, geometry: StockGeometry) -> StockChange {
    StockChange {
        id: StockChangeId(id),
        name: format!("change {id}"),
        enabled: true,
        op,
        geometry,
        material: filler(),
        display_colour: None,
    }
}

fn outline_change(
    id: usize,
    op: StockChangeOp,
    geometry: StockGeometry,
    polygon: Polygon2,
) -> ResolvedStockChange {
    ResolvedStockChange {
        setup_id: 0,
        change: change(id, op, geometry),
        sources: vec![StockChangeSource::Outlines(Arc::new(vec![polygon]))],
    }
}

fn fill(id: usize, level_z: f64) -> ResolvedStockChange {
    outline_change(
        id,
        StockChangeOp::Add,
        StockGeometry::OutlineFill {
            model_ids: vec![ModelId(1)],
            level_z,
        },
        rect(RX0, RY0, RX1, RY1),
    )
}

fn extrude(id: usize, op: StockChangeOp, z_bottom: f64, z_top: f64) -> ResolvedStockChange {
    outline_change(
        id,
        op,
        StockGeometry::OutlineExtrude {
            model_ids: vec![ModelId(1)],
            z_bottom,
            z_top,
        },
        rect(RX0, RY0, RX1, RY1),
    )
}

fn mesh_change(id: usize, op: StockChangeOp, mesh: TriangleMesh) -> ResolvedStockChange {
    ResolvedStockChange {
        setup_id: 0,
        change: change(
            id,
            op,
            StockGeometry::Model {
                model_id: ModelId(2),
            },
        ),
        sources: vec![StockChangeSource::Mesh(Arc::new(mesh))],
    }
}

/// The ray at `(x, y)` as `(enter, exit, slot)`.
fn ray_at(stock: &TriDexelStock, x: f64, y: f64) -> Vec<(f32, f32, u8)> {
    let (r, c) = stock.z_grid.world_to_cell(x, y).expect("cell on the grid");
    stock
        .z_grid
        .ray(r, c)
        .iter()
        .map(|s| (s.enter, s.exit, s.material.0))
        .collect()
}

fn assert_volume(actual: f64, expected: f64, what: &str) {
    // One cell column of the full stock height: the "one cell" tolerance.
    let tolerance = CELL * CELL * H;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: got {actual} mm³, expected {expected} mm³ (± {tolerance})"
    );
}

/// A closed box: 8 vertices, 12 triangles.
fn box_triangles(lo: P3, hi: P3, base: u32, flip: bool) -> (Vec<P3>, Vec<[u32; 3]>) {
    let v = |x: bool, y: bool, z: bool| {
        P3::new(
            if x { hi.x } else { lo.x },
            if y { hi.y } else { lo.y },
            if z { hi.z } else { lo.z },
        )
    };
    let vertices = vec![
        v(false, false, false),
        v(true, false, false),
        v(true, true, false),
        v(false, true, false),
        v(false, false, true),
        v(true, false, true),
        v(true, true, true),
        v(false, true, true),
    ];
    let mut triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    for t in &mut triangles {
        if flip {
            t.swap(1, 2);
        }
        for i in t.iter_mut() {
            *i += base;
        }
    }
    (vertices, triangles)
}

fn box_mesh(lo: P3, hi: P3) -> TriangleMesh {
    let (v, t) = box_triangles(lo, hi, 0, false);
    TriangleMesh::from_raw(v, t)
}

// ── Outline fill ────────────────────────────────────────────────────────

#[test]
fn an_outline_fill_into_a_pocket_gives_the_pocket_volume() {
    // Setup 1 removes a 5 mm pocket; setup 2 fills it to the top face.
    let req = request(vec![
        top_group(vec![], vec![extrude(1, StockChangeOp::Remove, 15.0, 25.0)]),
        top_group(vec![spot(2)], vec![fill(2, H)]),
    ]);
    let result = run(&req);
    assert_eq!(result.stock_change_volumes.len(), 2);
    assert_volume(
        result.stock_change_volumes[0].removed_mm3,
        R_AREA * 5.0,
        "the pocket",
    );
    let filled = &result.stock_change_volumes[1];
    assert_volume(filled.added_mm3, R_AREA * 5.0, "the fill");
    assert!((filled.added_ml() - filled.added_mm3 / 1000.0).abs() < 1e-12);
    assert_eq!(filled.removed_mm3, 0.0);
    // The column is whole again: stock to the floor, the fill above it.
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(0.0, 15.0, 0), (15.0, 20.0, 1)]
    );
    assert!(start.z_grid.has_added_material);
}

#[test]
fn the_fill_follows_an_overcut() {
    // A real 6 mm cutter cuts the pocket floor at z 15, and plunges 2 mm
    // deeper at (15, 8): the overcut. The fill reaches the cut floor, not
    // the design floor.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(13.0, 9.0, H + 5.0));
    tp.feed_to(P3::new(13.0, 9.0, 15.0), 300.0);
    for (i, y) in [9.0, 10.0, 11.0].into_iter().enumerate() {
        let (a, b) = if i % 2 == 0 {
            (13.0, 17.0)
        } else {
            (17.0, 13.0)
        };
        tp.feed_to(P3::new(a, y, 15.0), 600.0);
        tp.feed_to(P3::new(b, y, 15.0), 600.0);
    }
    tp.rapid_to(P3::new(IN_X, IN_Y, H + 5.0));
    tp.feed_to(P3::new(IN_X, IN_Y, 13.0), 300.0);
    tp.rapid_to(P3::new(IN_X, IN_Y, H + 5.0));
    let groups = |with_fill: bool| {
        vec![
            top_group(vec![entry(1, tp.clone())], vec![]),
            top_group(
                vec![spot(2)],
                if with_fill { vec![fill(2, H)] } else { vec![] },
            ),
        ]
    };
    let before = run(&request(groups(false)));
    let after = run(&request(groups(true)));

    // The expected volume: the empty length under H of every outline
    // column of the stock before the fill.
    let start_before = &before.prior_stocks[&ToolpathId(2)];
    let grid = &start_before.z_grid;
    let mut expected = 0.0;
    let mut cut_columns = 0;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let (u, v) = grid.cell_to_world(row, col);
            if u > RX0 && u < RX1 && v > RY0 && v < RY1 {
                let top = grid.top_z_at(row, col).unwrap();
                let missing = H - f64::from(top);
                if missing > 1e-6 {
                    cut_columns += 1;
                }
                expected += missing * CELL * CELL;
            }
        }
    }
    assert!(
        cut_columns > 50,
        "the cutter cut the outline: {cut_columns}"
    );
    assert_volume(
        after.stock_change_volumes[0].added_mm3,
        expected,
        "the fill of the real cut",
    );

    let start = &after.prior_stocks[&ToolpathId(2)];
    let overcut = ray_at(start, IN_X, IN_Y);
    assert_eq!(
        overcut.last().unwrap().2,
        1,
        "the fill is on top: {overcut:?}"
    );
    let fill_floor = overcut.last().unwrap().0;
    assert!(
        (f64::from(fill_floor) - 13.0).abs() < 0.05,
        "the fill starts at the overcut floor 13, got {fill_floor}: {overcut:?}"
    );
    assert!((f64::from(overcut.last().unwrap().1) - H).abs() < 1e-5);
}

#[test]
fn there_is_no_fill_under_a_bridge() {
    // A closed cavity 10..14 under 6 mm of material: gravity from the
    // top cannot reach it.
    let req = request(vec![top_group(
        vec![spot(2)],
        vec![extrude(1, StockChangeOp::Remove, 10.0, 14.0), fill(2, H)],
    )]);
    let result = run(&req);
    assert_eq!(result.stock_change_volumes[1].added_mm3, 0.0);
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(0.0, 10.0, 0), (14.0, 20.0, 0)],
        "the cavity stays empty"
    );
}

#[test]
fn a_fill_above_the_top_raises_the_top() {
    let req = request(vec![top_group(vec![spot(2)], vec![fill(1, H + 2.0)])]);
    let result = run(&req);
    assert_volume(
        result.stock_change_volumes[0].added_mm3,
        R_AREA * 2.0,
        "the overfill",
    );
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(0.0, 20.0, 0), (20.0, 22.0, 1)]
    );
    let (r, c) = start.z_grid.world_to_cell(IN_X, IN_Y).unwrap();
    assert_eq!(
        start.z_grid.conservative_top_at(r, c),
        22.0,
        "a rapid over the overfill must see it"
    );
    // Outside the outline nothing changes.
    assert_eq!(ray_at(start, 30.0, 8.0), vec![(0.0, 20.0, 0)]);
}

#[test]
fn a_fill_into_a_through_hole_starts_at_the_stock_bottom() {
    let req = request(vec![top_group(
        vec![spot(2)],
        vec![extrude(1, StockChangeOp::Remove, -1.0, 21.0), fill(2, H)],
    )]);
    let result = run(&req);
    assert_volume(
        result.stock_change_volumes[1].added_mm3,
        R_AREA * H,
        "the through-hole fill",
    );
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(ray_at(start, IN_X, IN_Y), vec![(0.0, 20.0, 1)]);
}

// ── Outline extrude ─────────────────────────────────────────────────────

#[test]
fn an_extrude_adds_its_prism_and_only_the_empty_part_of_it() {
    let req = request(vec![top_group(
        vec![spot(3)],
        vec![
            extrude(1, StockChangeOp::Add, 22.0, 25.0),
            // 18..22 overlaps the stock (18..20); only 20..22 is empty.
            extrude(2, StockChangeOp::Add, 18.0, 22.0),
        ],
    )]);
    let result = run(&req);
    assert_volume(
        result.stock_change_volumes[0].added_mm3,
        R_AREA * 3.0,
        "the prism",
    );
    assert_volume(
        result.stock_change_volumes[1].added_mm3,
        R_AREA * 2.0,
        "the empty part",
    );
    let start = &result.prior_stocks[&ToolpathId(3)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(0.0, 20.0, 0), (20.0, 25.0, 1)]
    );
}

// ── Model (mesh) ────────────────────────────────────────────────────────

#[test]
fn a_closed_box_mesh_adds_its_volume() {
    let mesh = box_mesh(P3::new(RX0, RY0, 21.0), P3::new(RX1, RY1, 24.0));
    let req = request(vec![top_group(
        vec![spot(2)],
        vec![mesh_change(1, StockChangeOp::Add, mesh)],
    )]);
    let result = run(&req);
    assert_volume(
        result.stock_change_volumes[0].added_mm3,
        R_AREA * 3.0,
        "the box",
    );
    let start = &result.prior_stocks[&ToolpathId(2)];
    let ray = ray_at(start, IN_X, IN_Y);
    assert_eq!(ray.len(), 2, "{ray:?}");
    assert_eq!(ray[1].2, 1);
    assert!((ray[1].0 - 21.0).abs() < 1e-5 && (ray[1].1 - 24.0).abs() < 1e-5);
}

#[test]
fn a_box_mesh_with_a_hole_adds_two_intervals_per_column_through_the_hole() {
    // An outer box with an inner void (the inner box wound inward): rays
    // through the void cross four faces.
    let (mut vertices, mut triangles) =
        box_triangles(P3::new(RX0, RY0, 21.0), P3::new(RX1, RY1, 27.0), 0, false);
    let (inner_v, inner_t) = box_triangles(
        P3::new(12.25, 7.25, 23.0),
        P3::new(18.25, 13.25, 25.0),
        8,
        true,
    );
    vertices.extend(inner_v);
    triangles.extend(inner_t);
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let req = request(vec![top_group(
        vec![spot(2)],
        vec![mesh_change(1, StockChangeOp::Add, mesh)],
    )]);
    let result = run(&req);
    // Outer 100 mm² x 6 mm minus the void 36 mm² x 2 mm.
    assert_volume(
        result.stock_change_volumes[0].added_mm3,
        R_AREA * 6.0 - 36.0 * 2.0,
        "the box minus its void",
    );
    let start = &result.prior_stocks[&ToolpathId(2)];
    let ray = ray_at(start, IN_X, IN_Y);
    assert_eq!(
        ray.len(),
        3,
        "stock, the lower shell, the upper shell: {ray:?}"
    );
    assert!((ray[1].0 - 21.0).abs() < 1e-5 && (ray[1].1 - 23.0).abs() < 1e-5);
    assert!((ray[2].0 - 25.0).abs() < 1e-5 && (ray[2].1 - 27.0).abs() < 1e-5);
}

#[test]
fn a_mesh_that_is_not_closed_is_refused_by_name() {
    let (vertices, mut triangles) =
        box_triangles(P3::new(RX0, RY0, 21.0), P3::new(RX1, RY1, 24.0), 0, false);
    // Take the top face away: every column now crosses the mesh once.
    triangles.drain(2..4);
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let req = request(vec![top_group(
        vec![spot(2)],
        vec![mesh_change(7, StockChangeOp::Add, mesh)],
    )]);
    let error = run_simulation(&req, &AtomicBool::new(false))
        .err()
        .expect("an open mesh must refuse");
    match &error {
        SimulationError::StockChangeRefused {
            change_id, reason, ..
        } => {
            assert_eq!(*change_id, StockChangeId(7));
            assert!(reason.contains("not a closed mesh"), "{reason}");
        }
        other => panic!("expected a stock change refusal, got {other}"),
    }
    assert!(error.to_string().contains("change 7"), "{error}");
}

#[test]
fn a_missing_model_is_refused_by_name() {
    let mut missing = fill(4, H);
    missing.sources = vec![StockChangeSource::Missing(ModelId(9))];
    let req = request(vec![top_group(vec![spot(2)], vec![missing])]);
    let error = run_simulation(&req, &AtomicBool::new(false))
        .err()
        .expect("a missing model must refuse");
    assert!(
        matches!(&error, SimulationError::StockChangeRefused { reason, .. }
            if reason.contains("model id 9")),
        "{error}"
    );
}

// ── Remove ──────────────────────────────────────────────────────────────

#[test]
fn a_remove_takes_material_of_every_kind() {
    // An added layer 20..22, then a removal box 19..23 through the stock
    // top and the added layer.
    let mesh = box_mesh(P3::new(RX0, RY0, 19.0), P3::new(RX1, RY1, 23.0));
    let req = request(vec![top_group(
        vec![spot(3)],
        vec![
            extrude(1, StockChangeOp::Add, 20.0, 22.0),
            mesh_change(2, StockChangeOp::Remove, mesh),
        ],
    )]);
    let result = run(&req);
    let removed = &result.stock_change_volumes[1];
    assert_eq!(removed.added_mm3, 0.0);
    assert_eq!(removed.material_slot, None);
    assert_volume(
        removed.removed_mm3,
        R_AREA * 3.0,
        "1 mm of stock + 2 mm added",
    );
    let start = &result.prior_stocks[&ToolpathId(3)];
    assert_eq!(ray_at(start, IN_X, IN_Y), vec![(0.0, 19.0, 0)]);
}

// ── The playback stock ──────────────────────────────────────────────────

#[test]
fn the_playback_stock_receives_the_fill_through_the_group_map() {
    // A Bottom setup adds a 2 mm prism above its own top face. The outline
    // is a drawing in the model frame, so the setup frame holds it at the
    // mirrored row (`apply_to_drawing_polygons`), and the playback (global)
    // frame holds it at the drawing's own row, BELOW the global bottom.
    let result = run(&request(vec![
        top_group(vec![spot(1)], vec![]),
        bottom_group(
            vec![spot(2)],
            vec![extrude(1, StockChangeOp::Add, H, H + 2.0)],
        ),
    ]));
    let local = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        ray_at(local, IN_X, D - IN_Y),
        vec![(0.0, 20.0, 0), (20.0, 22.0, 1)]
    );
    assert_eq!(ray_at(local, IN_X, IN_Y), vec![(0.0, 20.0, 0)]);

    let global = &result.checkpoints.last().unwrap().stock;
    assert_eq!(
        ray_at(global, IN_X, IN_Y),
        vec![(-2.0, 0.0, 1), (0.0, 20.0, 0)],
        "the prism under the global bottom, under the drawing"
    );
    assert_eq!(
        ray_at(global, IN_X, D - IN_Y),
        vec![(0.0, 20.0, 0)],
        "nothing at the mirrored row"
    );
}

// ── The carry ───────────────────────────────────────────────────────────

#[test]
fn a_group_with_only_a_change_carries_it_to_the_next_group() {
    // Top, then Bottom with a change and no toolpath, then Top. The Bottom
    // removal of local 15..20 is global 0..5, under the drawing.
    let result = run(&request(vec![
        top_group(vec![spot(1)], vec![]),
        bottom_group(vec![], vec![extrude(1, StockChangeOp::Remove, 15.0, 20.0)]),
        top_group(vec![spot(3)], vec![]),
    ]));
    assert_eq!(result.group_starts.len(), 3);
    assert!(matches!(
        result.group_starts[2].carry,
        StockCarry::Carried { from_group: 1, .. }
    ));
    assert_volume(
        result.stock_change_volumes[0].removed_mm3,
        R_AREA * 5.0,
        "the change-only removal",
    );
    let start = &result.prior_stocks[&ToolpathId(3)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(5.0, 20.0, 0)],
        "the removal from below, carried to the third setup"
    );
    assert_eq!(ray_at(start, IN_X, D - IN_Y), vec![(0.0, 20.0, 0)]);
}

// ── Materials ───────────────────────────────────────────────────────────

#[test]
fn added_segments_carry_the_slot_of_their_material() {
    let mut other = extrude(3, StockChangeOp::Add, 24.0, 26.0);
    other.change.material = Material::Custom {
        name: "Other".to_owned(),
        feed_scale_factor: 1.0,
    };
    let req = request(vec![top_group(
        vec![spot(4)],
        vec![
            extrude(1, StockChangeOp::Add, 20.0, 22.0),
            extrude(2, StockChangeOp::Add, 22.0, 23.0),
            other,
        ],
    )]);
    let result = run(&req);
    let slots: Vec<Option<MaterialSlot>> = result
        .stock_change_volumes
        .iter()
        .map(|v| v.material_slot)
        .collect();
    assert_eq!(
        slots,
        vec![
            Some(MaterialSlot(1)),
            Some(MaterialSlot(1)),
            Some(MaterialSlot(2))
        ],
        "one slot per material, reused for an equal material"
    );
    let start = &result.prior_stocks[&ToolpathId(4)];
    assert_eq!(
        ray_at(start, IN_X, IN_Y),
        vec![(0.0, 20.0, 0), (20.0, 23.0, 1), (24.0, 26.0, 2)]
    );
    let stock_material = Material::default();
    assert_eq!(
        start.materials.material(MaterialSlot(1), &stock_material),
        Some(&filler())
    );
}

#[test]
fn a_full_slot_table_is_refused_by_name() {
    let changes: Vec<ResolvedStockChange> = (0..8)
        .map(|k| {
            let mut c = extrude(k, StockChangeOp::Add, 20.0 + k as f64, 21.0 + k as f64);
            c.change.material = Material::Custom {
                name: format!("M{k}"),
                feed_scale_factor: 1.0,
            };
            c
        })
        .collect();
    let error = run_simulation(
        &request(vec![top_group(vec![spot(9)], changes)]),
        &AtomicBool::new(false),
    )
    .err()
    .expect("the eighth added material has no slot");
    assert!(
        matches!(&error, SimulationError::StockChangeRefused { change_id, reason, .. }
            if *change_id == StockChangeId(7) && reason.contains("M7")),
        "{error}"
    );
}

#[test]
fn a_run_with_no_change_touches_no_stock() {
    let result = run(&request(vec![
        top_group(vec![spot(1)], vec![]),
        top_group(vec![spot(2)], vec![]),
    ]));
    assert!(result.stock_change_volumes.is_empty());
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert!(start.materials.is_empty());
    assert!(!start.z_grid.has_added_material);
    assert_eq!(ray_at(start, IN_X, IN_Y), vec![(0.0, 20.0, 0)]);
}

#[test]
fn a_remove_with_an_outline_fill_is_refused_by_name() {
    let mut removal = fill(5, H);
    removal.change.op = StockChangeOp::Remove;
    let error = run_simulation(
        &request(vec![top_group(vec![spot(2)], vec![removal])]),
        &AtomicBool::new(false),
    )
    .err()
    .expect("a fill holds no material to remove");
    assert!(
        matches!(&error, SimulationError::StockChangeRefused { reason, .. }
            if reason.contains("cannot use an outline fill")),
        "{error}"
    );
}
