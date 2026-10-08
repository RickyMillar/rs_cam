//! S0 sentries — the stock carries across setups
//! (`planning/stock_additions_2026-10-09/PLAN.md` S0,
//! `planning/stock_fill_2026-10-09/DESIGN.md` §2.0).
//!
//! Before S0, every setup group started from a full block. Setup N+1's
//! metrics, gates, `prior_stocks` and checkpoint meshes did not see the cuts
//! of setup N, and the composite mesh stacked one closed solid per setup.
//!
//! The tests:
//!
//! 1. A Top/Bottom pair: a pocket from setup 1 is in setup 2's start stock at
//!    the mirrored cells, at the right depth, and nowhere else.
//! 2. An origin shift: an identity setup (world frame, stock origin not
//!    zero) carries into a zero-rooted Bottom setup.
//! 3. A Z rotation of 90°: the carry transposes the grid.
//! 4. A stock extent that is not a whole number of cells: the nearest node,
//!    with the offset on the result.
//! 5. A lateral setup starts fresh, and the Z-axis setup after it carries
//!    from the Z-axis setup before it.
//! 6. The composite mesh is ONE solid: the uncut face of setup 2 does not
//!    cover the pocket of setup 1.
//! 7. Cache keys: a change in setup 1 stales setup 2 (memo miss, new start
//!    stock, new `SourceStock`), and a memo resume carries as a full replay
//!    does.
//! 8. The scrub: the carried start stock of setup 2, mapped back to the
//!    playback frame (`map_stock_to_global`), is setup 1's final stock bit
//!    for bit, and it adds no difference to the playback stock of its own.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult, run_simulation,
    run_simulation_memoized,
};
use rs_cam_core::compute::source_stock::SourceEntry;
use rs_cam_core::compute::stock_carry::{StockCarry, map_stock_to_global};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

const W: f64 = 40.0;
const D: f64 = 30.0;
const H: f64 = 20.0;
const CELL: f64 = 0.5;
const POCKET_DEPTH: f64 = 5.0;
/// The pocket's centre in setup 1's zero-rooted frame. Off-centre in Y, so a
/// carry that forgets the mirror puts the pocket at the wrong cells.
const POCKET_X: f64 = 15.0;
const POCKET_Y: f64 = 8.0;

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

/// A raster pocket around `(cx, cy)` with its floor at `floor_z`, in the
/// frame the toolpath is emitted in. The 6 mm cutter clears about
/// `cx ± 6` by `cy ± 4`.
fn pocket(cx: f64, cy: f64, floor_z: f64, top_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(cx - 3.0, cy - 1.0, top_z + 5.0));
    tp.feed_to(P3::new(cx - 3.0, cy - 1.0, floor_z), 300.0);
    for (i, y) in [cy - 1.0, cy, cy + 1.0].into_iter().enumerate() {
        let (from, to) = if i % 2 == 0 {
            (cx - 3.0, cx + 3.0)
        } else {
            (cx + 3.0, cx - 3.0)
        };
        tp.feed_to(P3::new(from, y, floor_z), 600.0);
        tp.feed_to(P3::new(to, y, floor_z), 600.0);
    }
    tp.rapid_to(P3::new(cx + 3.0, cy + 1.0, top_z + 5.0));
    tp
}

fn entry(
    id: usize,
    annotated: &Arc<AnnotatedToolpath>,
    tool: &Arc<ToolDefinition>,
) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(id),
        name: format!("tp{id}"),
        annotated: Arc::clone(annotated),
        tool: Arc::clone(tool),
        flute_count: 2,
        tool_summary: "6mm Flat".into(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 0,
    }
}

fn info(
    face_up: FaceUp,
    z_rotation: ZRotation,
    w: f64,
    d: f64,
    h: f64,
    min: P3,
) -> SetupTransformInfo {
    SetupTransformInfo {
        face_up,
        z_rotation,
        stock_x: w,
        stock_y: d,
        stock_z: h,
        stock_origin_x: min.x,
        stock_origin_y: min.y,
        stock_origin_z: min.z,
    }
}

fn zero_bbox(w: f64, d: f64, h: f64) -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(w, d, h),
    }
}

/// An identity (Top, 0°) group: no transform, the world-frame grid (F-024).
fn identity_group(entries: Vec<SimToolpathEntry>) -> SimGroupEntry {
    SimGroupEntry {
        toolpaths: entries,
        direction: StockCutDirection::FromTop,
        local_stock_bbox: None,
        local_to_global: None,
        phantom_prior_stock: None,
        stock_changes: Vec::new(),
    }
}

/// A non-identity group over the zero-rooted effective stock of `info`.
fn setup_group(entries: Vec<SimToolpathEntry>, info: SetupTransformInfo) -> SimGroupEntry {
    SimGroupEntry {
        toolpaths: entries,
        direction: rs_cam_core::compute::simulate::group_stock_cut_direction(info.face_up),
        local_stock_bbox: Some(info.effective_stock_bbox()),
        local_to_global: Some(info),
        phantom_prior_stock: None,
        stock_changes: Vec::new(),
    }
}

fn request(groups: Vec<SimGroupEntry>, stock_bbox: BoundingBox3) -> SimulationRequest {
    SimulationRequest {
        groups,
        stock_bbox,
        stock_top_z: stock_bbox.max.z,
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

/// The ray of `stock` at local `(x, y)`, as `(enter, exit)` pairs.
fn ray_at(stock: &TriDexelStock, x: f64, y: f64) -> Vec<(f32, f32)> {
    let (r, c) = stock.z_grid.world_to_cell(x, y).expect("cell on the grid");
    stock
        .z_grid
        .ray(r, c)
        .iter()
        .map(|s| (s.enter, s.exit))
        .collect()
}

fn assert_close(actual: f32, expected: f64, what: &str) {
    assert!(
        (f64::from(actual) - expected).abs() < 0.05,
        "{what}: got {actual}, expected {expected}"
    );
}

/// A far-away cut so that setup 2 has an entry, and so a `prior_stocks`
/// snapshot, without touching the pocket's cells.
fn spot(x: f64, y: f64, floor_z: f64, top_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(x, y, top_z + 5.0));
    tp.feed_to(P3::new(x, y, floor_z), 300.0);
    tp.feed_to(P3::new(x + 1.0, y, floor_z), 300.0);
    tp.rapid_to(P3::new(x + 1.0, y, top_z + 5.0));
    tp
}

// ── 1. Top/Bottom ───────────────────────────────────────────────────────

#[test]
fn a_pocket_from_the_top_is_in_the_bottom_setup_stock_at_the_mirrored_cells() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &bottom_tp, &tool)], bottom),
        ],
        zero_bbox(W, D, H),
    );
    let result = run(&req);

    assert_eq!(result.group_starts.len(), 2);
    assert_eq!(result.group_starts[0].carry, StockCarry::FreshFirst);
    match result.group_starts[1].carry {
        StockCarry::Carried {
            from_group,
            z_flipped,
            max_node_offset_mm,
        } => {
            assert_eq!(from_group, 0);
            assert!(z_flipped, "a Top/Bottom pair reverses Z");
            assert!(
                max_node_offset_mm < 1e-9,
                "D / cell = 60 is a whole number: the map is exact, got {max_node_offset_mm}"
            );
        }
        other => panic!("setup 2 must carry, got {other:?}"),
    }

    let start = &result.prior_stocks[&ToolpathId(2)];
    // Bottom: local (x, D - y, H - z). The pocket floor at global z = 15 is
    // local z = 5; the material below it in local Z is gone.
    let mirrored = ray_at(start, POCKET_X, D - POCKET_Y);
    assert_eq!(
        mirrored.len(),
        1,
        "one segment at the mirrored pocket cell: {mirrored:?}"
    );
    assert_close(
        mirrored[0].0,
        POCKET_DEPTH,
        "the pocket floor seen from the bottom",
    );
    assert_close(mirrored[0].1, H, "the uncut bottom face is the new top");

    // The same local XY without the mirror is outside the pocket: full.
    let unmirrored = ray_at(start, POCKET_X, POCKET_Y);
    assert_eq!(
        unmirrored,
        vec![(0.0, H as f32)],
        "no pocket at the unmirrored cell"
    );

    // conservative_top follows the flip: the new top face is the uncut
    // bottom face, so the bound is the full height over the pocket too.
    let (r, c) = start.z_grid.world_to_cell(POCKET_X, D - POCKET_Y).unwrap();
    assert_close(
        start.z_grid.conservative_top_at(r, c),
        H,
        "conservative top",
    );
}

// ── 2. Origin shift ─────────────────────────────────────────────────────

#[test]
fn an_identity_setup_with_a_stock_origin_carries_into_a_zero_rooted_setup() {
    let tool = tool();
    let min = P3::new(100.0, 50.0, -20.0);
    let stock_bbox = BoundingBox3 {
        min,
        max: P3::new(min.x + W, min.y + D, min.z + H),
    };
    // The identity group emits in WORLD coordinates (F-024).
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        min.x + POCKET_X,
        min.y + POCKET_Y,
        min.z + H - POCKET_DEPTH,
        min.z + H,
    )));
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, min);
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &bottom_tp, &tool)], bottom),
        ],
        stock_bbox,
    );
    let result = run(&req);

    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        (start.stock_bbox.min, start.stock_bbox.max),
        (P3::origin(), P3::new(W, D, H)),
        "setup 2's grid is zero-rooted"
    );
    let mirrored = ray_at(start, POCKET_X, D - POCKET_Y);
    assert_eq!(mirrored.len(), 1, "{mirrored:?}");
    assert_close(
        mirrored[0].0,
        POCKET_DEPTH,
        "the pocket floor after the shift",
    );
    assert_close(mirrored[0].1, H, "the full height after the shift");
    // A missed shift would put the pocket off the grid or at world-sized
    // offsets; the full cell next to it proves the cells did not move.
    assert_eq!(ray_at(start, POCKET_X, POCKET_Y), vec![(0.0, H as f32)]);
}

// ── 3. Z rotation ───────────────────────────────────────────────────────

#[test]
fn a_z_rotation_of_90_degrees_transposes_the_carried_stock() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let rotated = info(FaceUp::Top, ZRotation::Deg90, W, D, H, P3::origin());
    // Deg90: local = (D - y, x, z), over a D x W x H local stock.
    let rotated_tp = Arc::new(AnnotatedToolpath::new(spot(2.0, 37.0, H - 1.0, H)));
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &rotated_tp, &tool)], rotated),
        ],
        zero_bbox(W, D, H),
    );
    let result = run(&req);

    match result.group_starts[1].carry {
        StockCarry::Carried {
            z_flipped,
            max_node_offset_mm,
            ..
        } => {
            assert!(!z_flipped, "a rotation keeps Z");
            assert!(max_node_offset_mm < 1e-9, "{max_node_offset_mm}");
        }
        other => panic!("{other:?}"),
    }
    let start = &result.prior_stocks[&ToolpathId(2)];
    assert_eq!(
        (start.stock_bbox.min, start.stock_bbox.max),
        (P3::origin(), P3::new(D, W, H))
    );
    let moved = ray_at(start, D - POCKET_Y, POCKET_X);
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_close(moved[0].0, 0.0, "the floor of the stock is untouched");
    assert_close(moved[0].1, H - POCKET_DEPTH, "the pocket floor keeps its Z");
    // The un-transposed position is outside the pocket.
    assert_eq!(ray_at(start, POCKET_X, POCKET_Y), vec![(0.0, H as f32)]);
    // Z keeps its sense, so the sliver-safe bound maps with the ray.
    let (r, c) = start.z_grid.world_to_cell(D - POCKET_Y, POCKET_X).unwrap();
    assert_close(
        start.z_grid.conservative_top_at(r, c),
        H - POCKET_DEPTH,
        "conservative top over the pocket",
    );
}

// ── 4. A stock extent that is not a whole number of cells ───────────────

#[test]
fn a_stock_depth_off_the_cell_grid_takes_the_nearest_node_and_records_the_offset() {
    let tool = tool();
    let d = 30.3; // 60.6 cells: the mirrored nodes miss the grid by 0.2 mm
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, d, H, P3::origin());
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &bottom_tp, &tool)], bottom),
        ],
        zero_bbox(W, d, H),
    );
    let result = run(&req);
    match result.group_starts[1].carry {
        StockCarry::Carried {
            max_node_offset_mm, ..
        } => {
            // ceil(30.3 / 0.5) * 0.5 - 30.3 = 0.2.
            assert!(
                (max_node_offset_mm - 0.2).abs() < 1e-6,
                "the offset is the grid miss, got {max_node_offset_mm}"
            );
        }
        other => panic!("{other:?}"),
    }
    let start = &result.prior_stocks[&ToolpathId(2)];
    let mirrored = ray_at(start, POCKET_X, d - POCKET_Y);
    assert_close(
        mirrored[0].0,
        POCKET_DEPTH,
        "the pocket floor, nearest node",
    );
}

// ── 5. A lateral setup stays fresh ──────────────────────────────────────

#[test]
fn a_lateral_setup_starts_fresh_and_the_next_z_setup_carries_past_it() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let front = info(FaceUp::Front, ZRotation::Deg0, W, D, H, P3::origin());
    // Front's local stock is W x H x D; a short cut near local (5, 5).
    let front_tp = Arc::new(AnnotatedToolpath::new(spot(5.0, 5.0, D - 1.0, D)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &front_tp, &tool)], front),
            setup_group(vec![entry(3, &bottom_tp, &tool)], bottom),
        ],
        zero_bbox(W, D, H),
    );
    let result = run(&req);

    assert_eq!(result.group_starts[1].carry, StockCarry::FreshLateral);
    let lateral_start = &result.prior_stocks[&ToolpathId(2)];
    let full = (0.0_f32, D as f32);
    assert!(
        lateral_start
            .z_grid
            .rays
            .iter()
            .all(|ray| ray.len() == 1 && (ray[0].enter, ray[0].exit) == full),
        "the lateral setup's start stock is a full block"
    );

    match result.group_starts[2].carry {
        StockCarry::Carried { from_group, .. } => {
            assert_eq!(
                from_group, 0,
                "the Bottom setup carries from the Top setup, past the lateral"
            );
        }
        other => panic!("{other:?}"),
    }
    let start = &result.prior_stocks[&ToolpathId(3)];
    assert_close(
        ray_at(start, POCKET_X, D - POCKET_Y)[0].0,
        POCKET_DEPTH,
        "the Top pocket reaches the Bottom setup",
    );

    // The rest record follows the same rule: the Bottom setup's snapshot was
    // made after the Top entry, not after the lateral entry.
    let after: Vec<ToolpathId> = result.prior_stock_sources[&ToolpathId(3)]
        .after
        .iter()
        .filter_map(|e: &SourceEntry| e.toolpath_id())
        .collect();
    assert_eq!(after, vec![ToolpathId(1)]);
    assert!(result.prior_stock_sources[&ToolpathId(2)].after.is_empty());
}

// ── 6. The composite is one solid ───────────────────────────────────────

#[test]
fn the_composite_mesh_does_not_stack_a_solid_per_setup() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &bottom_tp, &tool)], bottom),
        ],
        zero_bbox(W, D, H),
    );
    let result = run(&req);

    // Before S0, setup 2's fresh block put a vertex at the global top
    // (z = 20) over the pocket of setup 1. Now the only surface over the
    // pocket centre is its floor.
    let over_pocket_top = result
        .mesh
        .vertices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|v| {
            (f64::from(v[0]) - POCKET_X).abs() < 1.0 && (f64::from(v[1]) - POCKET_Y).abs() < 0.5
        })
        .map(|v| v[2])
        .fold(f32::MIN, f32::max);
    assert!(
        over_pocket_top < (H - POCKET_DEPTH + 0.1) as f32,
        "the highest surface over the pocket must be its floor, got z = {over_pocket_top}"
    );

    // The composite is the mesh of the last checkpoint's local stock: one
    // solid, not two.
    let last = result.checkpoints.last().unwrap();
    assert_eq!(
        result.mesh.vertices.len(),
        last.build_mesh().vertices.len(),
        "the composite is the last Z-axis group's mesh"
    );
}

// ── 7. Cache keys ───────────────────────────────────────────────────────

fn run_memo(req: &SimulationRequest, cache: &mut SimPrefixCache) -> SimulationResult {
    run_simulation_memoized(
        req,
        &AtomicBool::new(false),
        |_| {},
        Some(SimMemo { cache, store: true }),
    )
    .unwrap()
}

fn rays(stock: &TriDexelStock) -> Vec<Vec<(u32, u32)>> {
    stock
        .z_grid
        .rays
        .iter()
        .map(|ray| {
            ray.iter()
                .map(|s| (s.enter.to_bits(), s.exit.to_bits()))
                .collect()
        })
        .collect()
}

#[test]
fn a_change_in_setup_one_stales_the_start_stock_of_setup_two() {
    let tool = tool();
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let first = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let deeper = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - 8.0,
        H,
    )));
    let make = |top: &Arc<AnnotatedToolpath>| {
        request(
            vec![
                identity_group(vec![entry(1, top, &tool)]),
                setup_group(vec![entry(2, &bottom_tp, &tool)], bottom.clone()),
            ],
            zero_bbox(W, D, H),
        )
    };

    let mut cache = SimPrefixCache::new();
    let before = run_memo(&make(&first), &mut cache);
    assert_eq!(
        cache.stats().snapshots_stored,
        1,
        "the first run stores a prefix"
    );

    let after = run_memo(&make(&deeper), &mut cache);
    assert_eq!(
        cache.stats().hits,
        0,
        "an edit in setup 1 must miss the memo"
    );
    let start = &after.prior_stocks[&ToolpathId(2)];
    assert_close(
        ray_at(start, POCKET_X, D - POCKET_Y)[0].0,
        8.0,
        "setup 2 starts from the edited setup 1",
    );
    assert_ne!(
        rays(&before.prior_stocks[&ToolpathId(2)]),
        rays(start),
        "the start stock of setup 2 moved with setup 1"
    );
    // G-RESTRES: the rest record of setup 2 names setup 1's carved output,
    // so a rest result generated on the old stock no longer matches.
    assert_ne!(
        before.prior_stock_sources[&ToolpathId(2)],
        after.prior_stock_sources[&ToolpathId(2)],
    );
    assert_eq!(after.prior_stock_sources[&ToolpathId(2)].after.len(), 1);
}

#[test]
fn a_memo_resume_after_a_lateral_setup_carries_as_a_full_replay_does() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let front = info(FaceUp::Front, ZRotation::Deg0, W, D, H, P3::origin());
    let front_tp = Arc::new(AnnotatedToolpath::new(spot(5.0, 5.0, D - 1.0, D)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let groups = |bottom_entries: Vec<SimToolpathEntry>| {
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &front_tp, &tool)], front.clone()),
            setup_group(bottom_entries, bottom.clone()),
        ]
    };
    // Round 1: the Bottom setup is not generated yet. Its group carries only
    // a phantom snapshot, so the memo's snapshot point is the end of the
    // LATERAL group. Round 2 resumes there, and the Bottom setup must carry
    // from the Top setup, which the resume does not replay.
    let mut pending = groups(Vec::new());
    pending[2].phantom_prior_stock = Some((0, ToolpathId(3)));
    let mut cache = SimPrefixCache::new();
    let first = run_memo(&request(pending, zero_bbox(W, D, H)), &mut cache);
    assert!(first.prior_stocks.contains_key(&ToolpathId(3)));
    let three_req = request(
        groups(vec![entry(3, &bottom_tp, &tool)]),
        zero_bbox(W, D, H),
    );
    let resumed = run_memo(&three_req, &mut cache);
    assert_eq!(cache.stats().hits, 1, "round 2 resumes from the prefix");

    let full = run(&three_req);
    assert_eq!(resumed.group_starts, full.group_starts);
    assert_eq!(
        rays(&resumed.prior_stocks[&ToolpathId(3)]),
        rays(&full.prior_stocks[&ToolpathId(3)]),
        "the resumed carry must be bit-identical to a full replay"
    );
    assert_eq!(resumed.mesh.vertices, full.mesh.vertices);
}

// ── 8. The scrub ────────────────────────────────────────────────────────

/// Per-column material length of a stock, in grid order.
fn lengths(stock: &TriDexelStock) -> Vec<f32> {
    stock
        .z_grid
        .rays
        .iter()
        .map(|ray| ray.iter().map(|s| s.exit - s.enter).sum::<f32>())
        .collect()
}

#[test]
fn the_carried_start_stock_maps_back_onto_the_final_stock_of_setup_one() {
    let tool = tool();
    let top_tp = Arc::new(AnnotatedToolpath::new(pocket(
        POCKET_X,
        POCKET_Y,
        H - POCKET_DEPTH,
        H,
    )));
    let bottom_tp = Arc::new(AnnotatedToolpath::new(spot(35.0, 25.0, H - 1.0, H)));
    let bottom = info(FaceUp::Bottom, ZRotation::Deg0, W, D, H, P3::origin());
    let req = request(
        vec![
            identity_group(vec![entry(1, &top_tp, &tool)]),
            setup_group(vec![entry(2, &bottom_tp, &tool)], bottom.clone()),
        ],
        zero_bbox(W, D, H),
    );
    let result = run(&req);

    // The scrub resumes setup 2 from its carried start stock, mapped into
    // the playback frame (`map_stock_to_global`). The stock origin is zero,
    // so that frame is setup 1's world frame, and the map must give setup
    // 1's final stock back, bit for bit.
    let carried_global = map_stock_to_global(
        &result.prior_stocks[&ToolpathId(2)],
        &Some(bottom),
        &zero_bbox(W, D, H),
    );
    let setup_one_final = &result.checkpoints[0].mesh_stock;
    assert_eq!(
        rays(&carried_global),
        rays(setup_one_final),
        "the carry is a lossless map when the extent is a whole number of cells"
    );

    // Before S0 the scrub resumed setup 2 from the playback stock of setup
    // 1's last checkpoint (`SimCheckpointMesh::stock`). The playback route
    // stamps that stock, and it differs from the metric route in some edge
    // columns (a pre-existing difference, not an S0 one). The carry adds no
    // difference of its own: the carried stock differs from the playback
    // stock exactly where setup 1's own local stock does.
    let playback = lengths(&result.checkpoints[0].stock);
    let local = lengths(setup_one_final);
    let carried = lengths(&carried_global);
    assert_eq!(playback.len(), carried.len());
    for ((p, l), c) in playback.iter().zip(&local).zip(&carried) {
        assert_eq!((p - l).to_bits(), (p - c).to_bits());
    }
}
