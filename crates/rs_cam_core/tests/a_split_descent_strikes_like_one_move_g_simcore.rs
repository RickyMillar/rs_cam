//! G-SIMCORE (feeds contract P22-0b) — the core check reads the true stock
//! on every fed move, so a descent strikes however the toolpath splits it.
//!
//! The probe geometry of `planning/feeds_contract_2026-11-03/phases/
//! P22_sim_core.md`: wanaka tool 6, an 18 mm 2-flute bit with an 8 mm centre
//! core and a 1 mm blade protrusion, over a 12 mm board, cell 0.25 mm. The
//! stamp is the simulation's (`RadialProfileLUT::for_stock_stamp`, P22-0a):
//! the core cuts no wood, so the wood under it stays in the stock.
//!
//! - A 5 mm plunge as one move and as 11 moves of 0.5 mm: each pushes the
//!   core into wood (case A and case B of the probe).
//! - A 30 deg ramp as one move and as 22 moves (case E1 and case E2).
//! - A helix of radius 2 mm (inside the core) at the limit slope (case C1).
//! - A forward cut at full depth from outside the board: the ring ahead of
//!   the core cuts every cell before the core reaches it, so no finding.
//!
//! Red before P22-0b: the check ran only on a move steeper than the limit
//! angle, with a lens model of that one move, so cases B, E2 and C1 gave no
//! finding.
//!
//! The sample-engagement arm: a fed move whose only contact is wood under
//! the core reads no cutting engagement (`Engagement::from_sample`). Its
//! control stamps the same move with the envelope (`from_cutter`), which
//! counts the core as blade and reads an engagement.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{MillingCutter, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;

const DIAMETER_MM: f64 = 18.0;
const CORE_MM: f64 = 8.0;
const PROTRUSION_MM: f64 = 1.0;
const HALF: f64 = 30.0;
const STOCK_TOP_Z: f64 = 12.0;
const FLOOR_Z: f64 = 7.0;
const CELL_MM: f64 = 0.25;

fn bulk_bit() -> ToolDefinition {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "18 mm 2F".to_owned();
    tool.diameter = DIAMETER_MM;
    tool.set_cutting_length(ToolFact::Stated(30.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_shank_diameter(ToolFact::Stated(12.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
        NonCentreCuttingGeometry {
            centre_core_diameter_mm: CORE_MM,
            blade_protrusion_mm: PROTRUSION_MM,
        },
    )));
    rs_cam_core::compute::cutter::build_cutter(&tool).expect("a valid tool")
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

/// A toolpath that rapids over the first point, rapids down to it, and
/// feeds through the rest.
fn fed(points: &[P3]) -> Toolpath {
    let mut tp = Toolpath::new();
    let first = points.first().copied().expect("a point");
    tp.rapid_to(P3::new(first.x, first.y, STOCK_TOP_Z + 5.0));
    tp.rapid_to(first);
    for p in points.iter().skip(1) {
        tp.feed_to(*p, 1000.0);
    }
    tp
}

/// Run `tp` on `stock` through the simulation walk with the core check and
/// the table `lut`. Returns the samples and the move indices of the core
/// findings.
fn run(
    stock: &mut TriDexelStock,
    tool: &ToolDefinition,
    lut: &RadialProfileLUT,
    tp: &Toolpath,
) -> (Vec<SimulationCutSample>, Vec<usize>) {
    let mut check = RapidClearanceCheck::new(tool);
    let samples = stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            lut,
            tool,
            tool.radius(),
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
            Some(&mut check),
        )
        .expect("never cancelled");
    let (_rapids, core) = check.into_parts();
    (
        samples,
        core.into_iter().map(|hit| hit.move_index).collect(),
    )
}

/// The core findings of `points` fed over a fresh board with the
/// simulation's own table.
fn strikes(points: &[P3]) -> Vec<usize> {
    let tool = bulk_bit();
    let lut = RadialProfileLUT::for_stock_stamp(&tool, LUT_SAMPLES);
    run(&mut board(), &tool, &lut, &fed(points)).1
}

/// A descent from 0.5 mm over the board to the floor over `run` mm in X, in
/// `pieces` equal moves.
fn descent(run: f64, pieces: usize) -> Vec<P3> {
    let start = P3::new(-10.0, 0.0, STOCK_TOP_Z + 0.5);
    let mut points = vec![start];
    for k in 1..=pieces {
        let t = k as f64 / pieces as f64;
        points.push(P3::new(
            start.x + run * t,
            0.0,
            start.z + (FLOOR_Z - start.z) * t,
        ));
    }
    points
}

/// A helix of `radius` around the origin from 0.5 mm over the board down to
/// the floor at the G-CENTRECUT limit slope per 10 degree chord, then one
/// level turn at the floor.
fn helix(radius: f64) -> Vec<P3> {
    let slope = PROTRUSION_MM / CORE_MM;
    let chord = 10f64.to_radians();
    let drop_per_chord = radius * chord * slope;
    let mut points = vec![P3::new(radius, 0.0, STOCK_TOP_Z + 0.5)];
    let mut z = STOCK_TOP_Z + 0.5;
    let mut a = 0.0f64;
    while z > FLOOR_Z {
        a += chord;
        z = (z - drop_per_chord).max(FLOOR_Z);
        points.push(P3::new(radius * a.cos(), radius * a.sin(), z));
    }
    for _ in 0..36 {
        a += chord;
        points.push(P3::new(radius * a.cos(), radius * a.sin(), FLOOR_Z));
    }
    points
}

#[test]
fn a_split_descent_strikes_like_one_move() {
    let ramp_run = (STOCK_TOP_Z + 0.5 - FLOOR_Z) / 30f64.to_radians().tan();
    for (case, points) in [
        ("one 5 mm plunge", descent(0.0, 1)),
        ("the plunge as 11 moves", descent(0.0, 11)),
        ("one 30 deg ramp", descent(ramp_run, 1)),
        ("the ramp as 22 moves", descent(ramp_run, 22)),
    ] {
        let hits = strikes(&points);
        assert!(
            !hits.is_empty(),
            "{case}: the core goes into wood more than the protrusion, so the check \
             must give a finding"
        );
    }
}

#[test]
fn a_helix_inside_the_core_strikes() {
    let hits = strikes(&helix(2.0));
    assert!(
        !hits.is_empty(),
        "helix r 2: the centre stays inside the core and the post meets the core face"
    );
}

#[test]
fn a_forward_cut_at_full_depth_does_not_strike() {
    let points = [P3::new(-45.0, 0.0, FLOOR_Z), P3::new(10.0, 0.0, FLOOR_Z)];
    let hits = strikes(&points);
    assert!(
        hits.is_empty(),
        "a forward cut: the ring ahead of the core cuts each cell first; findings at moves \
         {hits:?}"
    );
}

/// The board after a helix of r 2 (the post stands at the centre), then a
/// 1 mm floor move from the centre: the only wood the move meets stands
/// under the core.
fn post_and_probe(lut_of_probe: &RadialProfileLUT) -> (Vec<SimulationCutSample>, Vec<usize>) {
    let tool = bulk_bit();
    let ring = RadialProfileLUT::for_stock_stamp(&tool, LUT_SAMPLES);
    let mut stock = board();
    run(&mut stock, &tool, &ring, &fed(&helix(2.0)));
    let mut probe = Toolpath::new();
    probe.rapid_to(P3::new(0.0, 0.0, STOCK_TOP_Z + 5.0));
    probe.rapid_to(P3::new(0.0, 0.0, FLOOR_Z));
    probe.feed_to(P3::new(1.0, 0.0, FLOOR_Z), 1000.0);
    run(&mut stock, &tool, lut_of_probe, &probe)
}

#[test]
fn a_core_only_contact_reads_no_cutting_engagement() {
    let tool = bulk_bit();
    let ring = RadialProfileLUT::for_stock_stamp(&tool, LUT_SAMPLES);
    let (samples, hits) = post_and_probe(&ring);
    let fed: Vec<_> = samples.iter().filter(|s| s.move_index == 2).collect();
    assert!(!fed.is_empty(), "the probe move has samples");
    for sample in &fed {
        assert!(
            Engagement::from_sample(sample, &tool).is_none(),
            "a sample over wood under the core only reads an engagement: arc {:?}, ap {}",
            sample.arc_engagement_radians,
            sample.axial_doc_mm
        );
    }
    assert_eq!(
        hits,
        vec![2],
        "the same move pushes the core into the post: the core check names it"
    );

    // The control: the envelope counts the core as blade and reads a cut.
    let envelope = RadialProfileLUT::from_cutter(&tool, LUT_SAMPLES);
    let (samples, _) = post_and_probe(&envelope);
    assert!(
        samples
            .iter()
            .filter(|s| s.move_index == 2)
            .any(|s| Engagement::from_sample(s, &tool).is_some()),
        "control: the envelope stamp reads the core contact as cutting"
    );
}

fn probe_limits() -> rs_cam_core::compute::tool_config::NonCentreCuttingLimits {
    NonCentreCuttingGeometry {
        centre_core_diameter_mm: CORE_MM,
        blade_protrusion_mm: PROTRUSION_MM,
    }
    .limits()
}

/// A helix of `radius` at `pitch` mm per turn in 10 degree chords from 0.5
/// mm over the board to the floor, then one level turn.
fn helix_at_pitch(radius: f64, pitch: f64) -> Vec<P3> {
    let chord = 10f64.to_radians();
    let mut points = vec![P3::new(radius, 0.0, STOCK_TOP_Z + 0.5)];
    let (mut z, mut a) = (STOCK_TOP_Z + 0.5, 0.0f64);
    while z > FLOOR_Z {
        a += chord;
        z = (z - pitch / 36.0).max(FLOOR_Z);
        points.push(P3::new(radius * a.cos(), radius * a.sin(), z));
    }
    for _ in 0..36 {
        a += chord;
        points.push(P3::new(radius * a.cos(), radius * a.sin(), FLOOR_Z));
    }
    points
}

/// A zigzag of `leg` along X from 0.5 mm over the board to the floor at
/// `slope`, back to the start column at the end.
fn zigzag(leg: f64, slope: f64) -> Vec<P3> {
    let mut points = vec![P3::new(0.0, 0.0, STOCK_TOP_Z + 0.5)];
    let (mut z, mut out) = (STOCK_TOP_Z + 0.5, true);
    while z > FLOOR_Z {
        z = (z - slope * leg).max(FLOOR_Z);
        points.push(P3::new(if out { leg } else { 0.0 }, 0.0, z));
        out = !out;
    }
    if !out {
        points.push(P3::new(0.0, 0.0, FLOOR_Z));
    }
    points
}

/// G-SIMCORE (P22-0b, the emitter rules): a helix at
/// `NonCentreCuttingLimits::max_helix_pitch_mm` and a zigzag at
/// `max_ramp_slope` with legs of `min_ramp_leg_mm` give no core finding.
/// The controls: the old chord clamp (`slope x chord x 36`) at r 4, and a
/// zigzag at the straight-line limit slope or with legs under the core
/// diameter, each strike. The geometry: `p22_0b_core_geometry.py`.
#[test]
fn the_entry_rules_keep_the_core_out_of_wood() {
    let limits = probe_limits();
    for radius in [4.0, 5.4, 9.0] {
        let hits = strikes(&helix_at_pitch(
            radius,
            limits.max_helix_pitch_mm(radius, 36),
        ));
        assert!(
            hits.is_empty(),
            "helix r {radius} at the exact pitch: findings {hits:?}"
        );
    }
    let chord = 2.0 * 4.0 * (std::f64::consts::PI / 36.0).sin();
    let old_pitch = limits.max_descent_slope() * chord * 36.0;
    assert!((limits.max_helix_pitch_mm(4.0, 36) - 2.0).abs() < 1e-9);
    assert!((limits.max_helix_pitch_mm(5.4, 36) - 3.766).abs() < 1e-3);
    assert!(
        !strikes(&helix_at_pitch(4.0, old_pitch)).is_empty(),
        "control: the chord clamp at r 4 ({old_pitch:.3} mm/turn) strikes"
    );
    let leg = limits.min_ramp_leg_mm();
    let hits = strikes(&zigzag(leg, limits.max_ramp_slope()));
    assert!(
        hits.is_empty(),
        "zigzag at p / 4c, legs d_core: findings {hits:?}"
    );
    let hits = strikes(&zigzag(2.0 * leg, limits.max_ramp_slope()));
    assert!(
        hits.is_empty(),
        "zigzag at p / 4c, legs 2 d_core: findings {hits:?}"
    );
    assert!(
        !strikes(&zigzag(2.0 * leg, limits.max_descent_slope())).is_empty(),
        "control: a zigzag at the straight-line limit strikes at its reversals"
    );
    assert!(
        !strikes(&zigzag(0.75 * leg, limits.max_ramp_slope())).is_empty(),
        "control: legs under the core diameter leave a post"
    );
}
