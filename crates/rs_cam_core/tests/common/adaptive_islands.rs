//! The six-island 2D Adaptive fixture (G-ADAPTORDER, G-ADAPTLINKLOAD,
//! G-ADAPTPASSLOAD) and the load limit its sentries read.
//!
//! A 120 x 80 mm pocket with six round islands of radius 8 on a 3 x 2 grid,
//! a 6 mm flat end mill, stepover 2, Depth/Pass 3 over 6 mm. The session is
//! generated and then simulated (dexel stock, sim cell 0.5 mm, metrics on),
//! so the cut trace is the oracle, not the planner.

use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use super::make_endmill_6mm;
use super::session::{polygon_model, single_op_session_with};

use rs_cam_core::adaptive::pass_engagement_limit;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::toolpath::Toolpath;

/// The simulation cell, mm.
pub const SIM_RESOLUTION_MM: f64 = 0.5;
/// The tool radius of [`make_endmill_6mm`], mm.
pub const TOOL_RADIUS_MM: f64 = 3.0;
/// The Adaptive stepover of the fixture, mm.
pub const STEPOVER_MM: f64 = 2.0;
/// The Adaptive tolerance of the fixture (`AdaptiveConfig::default()`), mm.
pub const TOLERANCE_MM: f64 = 0.1;

/// The planner's material-grid cell: `max(R / 6, tolerance)`
/// (`adaptive/path.rs`, `adaptive_segments_with_debug`).
pub fn planner_cell_mm() -> f64 {
    (TOOL_RADIUS_MM / 6.0).max(TOLERANCE_MM)
}

/// The pass ceiling as the simulator's radial width-of-cut fraction a_e/D:
/// `radial_woc_fraction_from_leading_arc(pass_engagement_limit(s, R))`,
/// 0.3626 at s 2, R 3.
pub fn pass_limit_radial() -> f64 {
    radial_woc_fraction_from_leading_arc(pass_engagement_limit(STEPOVER_MM, TOOL_RADIUS_MM))
}

/// What the simulator may read above the planner on the same cut, as a
/// fraction of D. The sim radial is the perpendicular extent of fresh sim
/// cells over D (`dexel_stock/stamping.rs`). Three widths separate it from
/// the planner's grid: a planner cell is cleared when its lattice point is
/// inside a stamp disc, so up to one planner cell (0.5 mm) of real material
/// can sit where the planner reads cleared; the sim measures extent at sim
/// cell centres (0.5 mm); and the emitted path is the planner path
/// simplified to the operation tolerance (0.1 mm). (0.5 + 0.5 + 0.1) / 6 =
/// 0.183.
pub fn discretisation_tolerance() -> f64 {
    (planner_cell_mm() + SIM_RESOLUTION_MM + TOLERANCE_MM) / (2.0 * TOOL_RADIUS_MM)
}

fn circle(cx: f64, cy: f64, r: f64) -> Vec<P2> {
    let n = 48;
    (0..n)
        .map(|i| {
            let t = -(i as f64) * TAU / f64::from(n);
            P2::new(cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}

/// A 120 x 80 pocket with six round islands of radius 8 on a 3 x 2 grid.
pub fn islands_pocket() -> Polygon2 {
    let exterior = vec![
        P2::new(-60.0, -40.0),
        P2::new(60.0, -40.0),
        P2::new(60.0, 40.0),
        P2::new(-60.0, 40.0),
    ];
    let mut holes = Vec::new();
    for &x in &[-32.0, 0.0, 32.0] {
        for &y in &[-16.0, 16.0] {
            holes.push(circle(x, y, 8.0));
        }
    }
    Polygon2::with_holes(exterior, holes)
}

/// The exact distance, mm, from `(x, y)` to the nearest wall of
/// [`islands_pocket`] (its exterior or an island ring), the polygon the
/// planner and the dressup read.
pub fn wall_distance(x: f64, y: f64) -> f64 {
    let part = islands_pocket();
    let mut d = f64::INFINITY;
    for ring in std::iter::once(&part.exterior).chain(part.holes.iter()) {
        for (i, &a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            d = d.min(point_segment_distance(P2::new(x, y), a, b));
        }
    }
    d
}

/// The exact distance, mm, from the segment `p`..`q` to the standing walls
/// of [`islands_pocket`]: 0 when any point of it lies in the part's
/// material (outside the pocket or inside an island), else the least
/// segment-to-edge distance.
pub fn wall_distance_of_segment(p: P2, q: P2) -> f64 {
    let part = islands_pocket();
    if !part.contains_point(&p) || !part.contains_point(&q) {
        return 0.0;
    }
    let mut d = f64::INFINITY;
    for ring in std::iter::once(&part.exterior).chain(part.holes.iter()) {
        for (i, &a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            d = d.min(segment_segment_distance(p, q, a, b));
        }
    }
    d
}

fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

fn segment_segment_distance(p: P2, q: P2, a: P2, b: P2) -> f64 {
    let cross = |o: P2, u: P2, v: P2| (u.x - o.x) * (v.y - o.y) - (u.y - o.y) * (v.x - o.x);
    let (d1, d2) = (cross(a, b, p), cross(a, b, q));
    let (d3, d4) = (cross(p, q, a), cross(p, q, b));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    point_segment_distance(p, a, b)
        .min(point_segment_distance(q, a, b))
        .min(point_segment_distance(a, p, q))
        .min(point_segment_distance(b, p, q))
}

pub fn stock() -> StockConfig {
    StockConfig {
        x: 130.0,
        y: 90.0,
        z: 12.0,
        origin_x: -65.0,
        origin_y: -45.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// Generate and simulate the fixture, with the rapid-order box as given.
pub fn adaptive_session(reorder: bool) -> ProjectSession {
    adaptive_session_with(reorder, None)
}

/// [`adaptive_session`] with the entry style set (`None` keeps the op's
/// default, the helix).
pub fn adaptive_session_with(
    reorder: bool,
    entry_style: Option<rs_cam_core::compute::config::DressupEntryStyle>,
) -> ProjectSession {
    let cfg = AdaptiveConfig {
        stepover: STEPOVER_MM,
        depth: 6.0,
        depth_per_pass: 3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        ..AdaptiveConfig::default()
    };
    assert_eq!(
        cfg.tolerance, TOLERANCE_MM,
        "the fixture pins the default tolerance"
    );
    let mut session = single_op_session_with(
        stock(),
        make_endmill_6mm(),
        polygon_model(vec![islands_pocket()], "islands_pocket"),
        "Adaptive",
        OperationConfig::Adaptive(cfg),
        |tc| {
            tc.dressups.optimize_rapid_order = reorder;
            if let Some(style) = entry_style {
                tc.dressups.entry_style = style;
            }
        },
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("adaptive generation");
    let opts = SimulationOptions {
        resolution: SIM_RESOLUTION_MM,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");
    session
}

pub fn toolpath_of(session: &ProjectSession) -> Toolpath {
    session
        .get_result(0)
        .expect("adaptive result")
        .toolpath()
        .clone()
}

pub fn trace_of(session: &ProjectSession) -> &SimulationCutTrace {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metrics-on simulation carries a cut trace")
}
