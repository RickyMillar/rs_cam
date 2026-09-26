//! G-ADAPTORDER — the 2D adaptive clear keeps its planner order.
//!
//! Pre-registration: `planning/rapid_safety_2026-08-28/RAPIDPLUNGETOL_PLAN.md`
//! §5. The 2D adaptive planner clears one material grid per depth level, in
//! the order it emits its runs. A keep-down `Linking` feed is admitted when
//! its corridor reads clear on that grid, i.e. cut by an EARLIER run
//! (`adaptive/path.rs`, `is_clear_path`). The rapid-order pass splits the
//! path only at rapids, so it moves whole keep-down chains; when it puts a
//! chain before the run that cleared the chain's link corridor, the link
//! feeds at full depth through standing stock, and the planner's engagement
//! for the chain's runs no longer holds.
//!
//! The fixture: a 120 x 80 mm pocket with six round islands, 6 mm flat end
//! mill, Depth/Pass 3 over 6 mm, rapid order on. The oracle is the session
//! simulation of the emitted path (dexel stock, metrics on), not the
//! planner.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use common::make_endmill_6mm;
use common::session::{polygon_model, single_op_session_with};

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// A sample removes material when its volume estimate exceeds this, in mm^3.
const CUT_VOLUME_EPS: f64 = 1e-3;

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
fn islands_pocket() -> Polygon2 {
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

fn stock() -> StockConfig {
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

fn adaptive_session(reorder: bool) -> ProjectSession {
    let cfg = AdaptiveConfig {
        stepover: 2.0,
        depth: 6.0,
        depth_per_pass: 3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        ..AdaptiveConfig::default()
    };
    let mut session = single_op_session_with(
        stock(),
        make_endmill_6mm(),
        polygon_model(vec![islands_pocket()], "islands_pocket"),
        "Adaptive",
        OperationConfig::Adaptive(cfg),
        |tc| tc.dressups.optimize_rapid_order = reorder,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("adaptive generation");
    let opts = SimulationOptions {
        resolution: 0.5,
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

fn toolpath_of(session: &ProjectSession) -> Toolpath {
    session
        .get_result(0)
        .expect("adaptive result")
        .toolpath()
        .clone()
}

fn trace_of(session: &ProjectSession) -> &SimulationCutTrace {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metrics-on simulation carries a cut trace")
}

/// The cutting targets in emitted order: the run order made visible.
fn cut_targets(tp: &Toolpath) -> Vec<[i64; 2]> {
    tp.moves
        .iter()
        .filter(|m| m.intent == MoveIntent::ClearingCut)
        .map(|m| {
            [
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            ]
        })
        .collect()
}

/// The planner links at any distance through a corridor that reads clear on
/// its grid, and through material only below this length (six tool radii,
/// `adaptive/path.rs` `max_link_dist`). A longer keep-down link is one the
/// planner admitted ONLY because earlier runs had cleared its corridor.
const CLEAR_ONLY_LINK_MM: f64 = 6.0 * 3.0;

/// One arm of the measurement.
#[derive(Debug, Default)]
struct Arm {
    /// Fed `Linking` moves, and those that remove material.
    links: usize,
    links_in_material: usize,
    link_samples_in_material: usize,
    link_volume_mm3: f64,
    link_peak_radial: f64,
    /// The same for the links longer than [`CLEAR_ONLY_LINK_MM`].
    clear_only_links: usize,
    clear_only_links_in_material: usize,
    clear_only_link_samples_in_material: usize,
    clear_only_link_volume_mm3: f64,
    clear_only_link_peak_radial: f64,
    /// Peak radial engagement over every sample, and over `ClearingCut`.
    peak_radial: f64,
    clearing_peak_radial: f64,
    /// Straight `EntryPlunge` moves that remove material.
    entry_plunges_in_material: usize,
    cycle_s: f64,
}

fn measure(session: &ProjectSession) -> Arm {
    let tp = toolpath_of(session);
    let trace = trace_of(session);
    let mut arm = Arm {
        cycle_s: trace.summary.total_runtime_s,
        ..Arm::default()
    };
    let mut plunges = std::collections::BTreeSet::new();
    for s in &trace.samples {
        let radial = s.engagement.radial_woc_fraction;
        arm.peak_radial = arm.peak_radial.max(radial);
        if s.source_intent == Some(MoveIntent::ClearingCut) {
            arm.clearing_peak_radial = arm.clearing_peak_radial.max(radial);
        }
        if s.source_intent == Some(MoveIntent::EntryPlunge)
            && s.removed_volume_est_mm3 > CUT_VOLUME_EPS
        {
            plunges.insert(s.move_index);
        }
    }
    arm.entry_plunges_in_material = plunges.len();
    for (i, mv) in tp.moves.iter().enumerate() {
        if mv.intent != MoveIntent::Linking || matches!(mv.move_type, MoveType::Rapid) || i == 0 {
            continue;
        }
        let from = tp.moves[i - 1].target;
        let clear_only = (mv.target.x - from.x).hypot(mv.target.y - from.y) > CLEAR_ONLY_LINK_MM;
        let (mut n, mut v, mut r) = (0_usize, 0.0_f64, 0.0_f64);
        for s in trace.samples.iter().filter(|s| s.move_index == i) {
            if s.removed_volume_est_mm3 > CUT_VOLUME_EPS {
                n += 1;
                v += s.removed_volume_est_mm3;
                r = r.max(s.engagement.radial_woc_fraction);
            }
        }
        arm.links += 1;
        arm.links_in_material += usize::from(n > 0);
        arm.link_samples_in_material += n;
        arm.link_volume_mm3 += v;
        arm.link_peak_radial = arm.link_peak_radial.max(r);
        if clear_only {
            arm.clear_only_links += 1;
            arm.clear_only_links_in_material += usize::from(n > 0);
            arm.clear_only_link_samples_in_material += n;
            arm.clear_only_link_volume_mm3 += v;
            arm.clear_only_link_peak_radial = arm.clear_only_link_peak_radial.max(r);
        }
    }
    arm
}

/// The evidence run: reorder on against reorder off. Prints one line per
/// arm; asserts nothing.
#[test]
#[ignore = "instrument: prints the reorder on/off measurement"]
fn measure_reorder_on_against_off() {
    let off = adaptive_session(false);
    let on = adaptive_session(true);
    let same_order = cut_targets(&toolpath_of(&off)) == cut_targets(&toolpath_of(&on));
    eprintln!("same cut order: {same_order}");
    eprintln!("reorder OFF: {:#?}", measure(&off));
    eprintln!("reorder ON : {:#?}", measure(&on));
    eprintln!(
        "cycle OFF {:.1} s, ON {:.1} s",
        measure(&off).cycle_s,
        measure(&on).cycle_s
    );
}

/// The move list as comparable bits: target, type and intent.
fn move_bits(tp: &Toolpath) -> Vec<String> {
    tp.moves
        .iter()
        .map(|m| {
            format!(
                "{:.4} {:.4} {:.4} {:?} {:?}",
                m.target.x, m.target.y, m.target.z, m.move_type, m.intent
            )
        })
        .collect()
}

/// With rapid order on, the session emits the planner order, so no keep-down
/// link runs before the run that cleared its corridor.
///
/// The planner order is the reorder-off arm. Before the veto this fixture
/// emitted a different cut order, and the six links over 6 x R removed
/// 349.3 mm^3 in 122 samples (peak radial 0.64) against 61.7 mm^3 in 18
/// samples (peak 0.30) in the planner order. The planner order is not
/// link-clean: it links through material below 6 x R by design, and the
/// grid-to-dexel difference leaves the 61.7 mm^3; this test holds the
/// reordered path to the planner's own, not to zero.
#[test]
fn adaptive_keeps_the_planner_order_with_rapid_order_on() {
    let off = adaptive_session(false);
    let on = adaptive_session(true);
    let (a_off, a_on) = (measure(&off), measure(&on));
    eprintln!("G-ADAPTORDER planner order: {a_off:?}");
    eprintln!("G-ADAPTORDER rapid order on: {a_on:?}");
    // The fixture must give the planner long links to protect.
    assert!(
        a_off.clear_only_links >= 3,
        "only {} links over 6 x R: the fixture tests no corridor",
        a_off.clear_only_links
    );
    assert_eq!(
        cut_targets(&toolpath_of(&on)),
        cut_targets(&toolpath_of(&off)),
        "rapid order on changed the order of the planner's cutting moves"
    );
    assert_eq!(
        move_bits(&toolpath_of(&on)),
        move_bits(&toolpath_of(&off)),
        "rapid order on changed the emitted path"
    );
    assert!(
        a_on.clear_only_link_samples_in_material <= a_off.clear_only_link_samples_in_material,
        "links over 6 x R cut material in {} samples with rapid order on, {} in the planner order",
        a_on.clear_only_link_samples_in_material,
        a_off.clear_only_link_samples_in_material
    );
    // The GUI greys the rapid-order box out from this flag.
    assert!(
        !rs_cam_core::compute::catalog::OperationType::Adaptive
            .transform_capabilities()
            .allows_rapid_reorder,
        "2D Adaptive must refuse the rapid-order pass"
    );
}
