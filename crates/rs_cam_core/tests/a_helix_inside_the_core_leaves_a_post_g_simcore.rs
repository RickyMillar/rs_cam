//! G-SIMCORE (feeds contract P22-0a) — the simulation cuts only with the
//! blade ring of a tool that does not cut at its centre.
//!
//! The probe geometry of `planning/feeds_contract_2026-11-03/phases/
//! P22_sim_core.md`: wanaka tool 6, an 18 mm 2-flute bit with an 8 mm centre
//! core and a 1 mm blade protrusion, over a 12 mm board, cell 0.25 mm. The
//! floor is Z 7 (5 mm into the stock).
//!
//! - A helix whose radius is smaller than the core radius (4 mm) keeps the
//!   helix centre inside the core for the whole entry. No blade passes over
//!   the centre, so real wood keeps a post there at its original height.
//! - A helix of radius 5.4 mm (0.3 D) puts the centre in the blade ring at
//!   every phase: the centre is cut to the floor.
//! - A straight cut at full depth: the ring ahead of the core sweeps every
//!   cell that the core later passes over, so no post stays on the path.
//!
//! Red before the fix: the stamp read the bare flat profile and cut the core
//! as blade, so the r 2 and r 3 arms read the centre at Z 7.
//!
//! Both simulation routes are read: the metric route
//! (`simulate_toolpath_with_metrics_with_cancel`) and the playback route
//! (`simulate_toolpath`). They must agree (`dexel_stock/CLAUDE.md`).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::toolpath::Toolpath;

const DIAMETER_MM: f64 = 18.0;
const CORE_MM: f64 = 8.0;
const PROTRUSION_MM: f64 = 1.0;
const HALF: f64 = 30.0;
const STOCK_TOP_Z: f64 = 12.0;
const FLOOR_Z: f64 = 7.0;
const CELL_MM: f64 = 0.25;
/// The disc the stock top is read in at the helix centre (the probe's).
const READ_RADIUS_MM: f64 = 0.3;
/// One cell of slack on a height read.
const TOL_MM: f64 = CELL_MM;

fn bulk_bit(non_centre_cutting: bool) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "18 mm 2F".to_owned();
    tool.diameter = DIAMETER_MM;
    tool.set_cutting_length(ToolFact::Stated(30.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_shank_diameter(ToolFact::Stated(12.0));
    tool.set_flute_count(ToolFact::Stated(2));
    if non_centre_cutting {
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: CORE_MM,
                blade_protrusion_mm: PROTRUSION_MM,
            },
        )));
    }
    tool
}

fn board() -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-HALF, -HALF, 0.0),
            max: P3::new(HALF, HALF, STOCK_TOP_Z),
        },
        CELL_MM,
    )
}

/// A helix of `radius` around the origin from 0.5 mm over the board down to
/// the floor, at the G-CENTRECUT limit slope (`atan(protrusion / core)`, the
/// clamp `emit_helix` applies), in 10 degree chords, then one level turn at
/// the floor.
fn helix(radius: f64) -> Toolpath {
    let slope = PROTRUSION_MM / CORE_MM;
    let chord = 10f64.to_radians();
    let drop_per_chord = radius * chord * slope;
    let mut tp = Toolpath::new();
    let start_z = STOCK_TOP_Z + 0.5;
    tp.rapid_to(P3::new(radius, 0.0, STOCK_TOP_Z + 5.0));
    tp.rapid_to(P3::new(radius, 0.0, start_z));
    let mut z = start_z;
    let mut a = 0.0f64;
    while z > FLOOR_Z {
        a += chord;
        z = (z - drop_per_chord).max(FLOOR_Z);
        tp.feed_to(P3::new(radius * a.cos(), radius * a.sin(), z), 1000.0);
    }
    for _ in 0..36 {
        a += chord;
        tp.feed_to(P3::new(radius * a.cos(), radius * a.sin(), FLOOR_Z), 1000.0);
    }
    tp
}

/// A straight cut at the floor from outside the board to X 10.
fn straight_cut() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-45.0, 0.0, STOCK_TOP_Z + 5.0));
    tp.rapid_to(P3::new(-45.0, 0.0, FLOOR_Z));
    tp.feed_to(P3::new(10.0, 0.0, FLOOR_Z), 1000.0);
    tp
}

/// The board after `tp`, by the metric route and by the playback route.
fn simulate(tool: &ToolConfig, tp: &Toolpath) -> (TriDexelStock, TriDexelStock) {
    let def = rs_cam_core::compute::cutter::build_cutter(tool).expect("a valid tool");
    let mut metric = board();
    metric
        .simulate_toolpath_with_metrics_with_cancel(
            tp,
            &def,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            18_000,
            2,
            5000.0,
            CELL_MM,
            None,
            &[],
            &[],
            true,
            &rs_cam_core::interrupt::NeverCancel,
        )
        .expect("never cancelled");
    let mut playback = board();
    playback.simulate_toolpath(tp, &def, StockCutDirection::FromTop);
    (metric, playback)
}

fn centre_top(stock: &TriDexelStock) -> f64 {
    stock
        .max_top_z_in_disc(0.0, 0.0, READ_RADIUS_MM)
        .expect("material stands at the centre")
}

#[test]
fn a_helix_inside_the_core_leaves_a_post() {
    let tool = bulk_bit(true);
    for radius in [2.0, 3.0] {
        let (metric, playback) = simulate(&tool, &helix(radius));
        for (route, stock) in [("metric", &metric), ("playback", &playback)] {
            let top = centre_top(stock);
            assert!(
                top >= STOCK_TOP_Z - TOL_MM,
                "{route}, helix r {radius}: the centre stays inside the core, so the \
                 post stands at Z {STOCK_TOP_Z}; the sim reads Z {top:.3}"
            );
        }
    }
}

#[test]
fn a_helix_wider_than_the_core_cuts_the_centre() {
    let tool = bulk_bit(true);
    let (metric, playback) = simulate(&tool, &helix(0.3 * DIAMETER_MM));
    for (route, stock) in [("metric", &metric), ("playback", &playback)] {
        let top = centre_top(stock);
        assert!(
            top <= FLOOR_Z + TOL_MM,
            "{route}, helix r 5.4: the centre is in the blade ring, so it is cut to \
             Z {FLOOR_Z}; the sim reads Z {top:.3}"
        );
    }
}

#[test]
fn a_straight_cut_at_full_depth_leaves_no_post_on_its_path() {
    let tool = bulk_bit(true);
    let (metric, playback) = simulate(&tool, &straight_cut());
    for (route, stock) in [("metric", &metric), ("playback", &playback)] {
        // Every point on the path line inside the board, the end position
        // included: the ring ahead of the core cut it.
        let mut x = -HALF + 1.0;
        while x <= 10.0 {
            let top = stock
                .max_top_z_in_disc(x, 0.0, READ_RADIUS_MM)
                .expect("the floor holds material");
            assert!(
                top <= FLOOR_Z + TOL_MM,
                "{route}: the path at X {x:.1} reads Z {top:.3}, over the floor Z {FLOOR_Z}"
            );
            x += 0.5;
        }
    }
}

/// The control: a centre-cutting bit on the same r 2 helix cuts the centre.
#[test]
fn a_centre_cutting_bit_cuts_the_centre_of_a_small_helix() {
    let tool = bulk_bit(false);
    let (metric, playback) = simulate(&tool, &helix(2.0));
    for (route, stock) in [("metric", &metric), ("playback", &playback)] {
        let top = centre_top(stock);
        assert!(
            top <= FLOOR_Z + TOL_MM,
            "{route}: a centre-cutting bit cuts the helix centre; the sim reads Z {top:.3}"
        );
    }
}

/// A straight 5 mm plunge (case A of the probe) cuts only the ring: the
/// centre keeps its height, a point 6 mm out (inside the ring) is cut to the
/// floor. This arm reads the point kernels (a move with no planar length).
#[test]
fn a_plunge_cuts_only_the_blade_ring() {
    let tool = bulk_bit(true);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, STOCK_TOP_Z + 5.0));
    tp.rapid_to(P3::new(0.0, 0.0, STOCK_TOP_Z + 0.5));
    tp.feed_to(P3::new(0.0, 0.0, FLOOR_Z), 1000.0);
    let (metric, playback) = simulate(&tool, &tp);
    for (route, stock) in [("metric", &metric), ("playback", &playback)] {
        let top = centre_top(stock);
        assert!(
            top >= STOCK_TOP_Z - TOL_MM,
            "{route}: the core removes no stock; the centre reads Z {top:.3}"
        );
        let ring = stock
            .max_top_z_in_disc(6.0, 0.0, READ_RADIUS_MM)
            .expect("the floor holds material");
        assert!(
            ring <= FLOOR_Z + TOL_MM,
            "{route}: the ring at r 6 is cut to the floor; it reads Z {ring:.3}"
        );
    }
}
