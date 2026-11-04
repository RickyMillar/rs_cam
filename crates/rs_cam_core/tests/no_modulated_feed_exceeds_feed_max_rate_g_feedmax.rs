//! G-FEEDMAX (operator ruling 2026-10-28;
//! `planning/adaptive_research_2026-10-23/P15_D_verify.md` "## D59"): the
//! feed modulator obeys `DressupConfig::feed_max_rate`.
//!
//! Before the ruling only the feed optimiser read the dial. The modulator
//! raised a cutting move up to the machine cutting ceiling, so a job with
//! `feed_max_rate = 3000` cut at F4000 (Corne) and F5245 (wanaka).
//!
//! The fixture is the F115 pocket (the AS001 pocket of F-036b on the
//! Shapeoko XXL kinematics), whose ConstrainedMax modulation lifts its
//! 770 mm/min cut feed above 3000. Three claims:
//!
//! 1. With `feed_max_rate = 3000` no fed move of the modulated result is
//!    above 3000, and the modulation still lifts some feed above the
//!    commanded 770.
//! 2. Non-vacuity: with no `feed_max_rate` (the machine ceiling) the same
//!    run lifts a feed above 3000.
//! 3. The cap only lowers: a `feed_max_rate` above the machine cutting
//!    ceiling gives the same moves as no `feed_max_rate`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use rs_cam_core::compute::tool_config::ToolFact;
use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
use rs_cam_core::geo::P2;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetMachineArgs, SimulationOptions,
};

/// The value `compute::fixture_values` gives every fixture op.
const RATE: f64 = rs_cam_core::compute::fixture_values::FIXTURE_FEED_MAX_RATE;
const COMMANDED_FEED: f64 = 770.0;

fn end_mill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.name = "End Mill 6mm (G-FEEDMAX)".to_owned();
    tool
}

fn rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let n = 64;
    let hole = (0..n)
        .map(|i| {
            let t = f64::from(i) * TAU / f64::from(n);
            P2::new(40.0 + 10.0 * (-t).cos(), 30.0 + 10.0 * (-t).sin())
        })
        .collect();
    Polygon2::with_holes(exterior, vec![hole])
}

/// The F115 pocket with `feed_max_rate`, generated and simulated with
/// ConstrainedMax modulation.
fn modulated_pocket(feed_max_rate: Option<f64>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: 100.0,
        y: 100.0,
        z: Some(12.0),
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });
    let tool_idx = builder.add_tool(end_mill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(common::session::polygon_model(
        vec![rect_with_island()],
        "as001_pocket",
    ));
    let pocket = PocketConfig {
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(6.0),
        depth_per_pass: 2.0,
        feed_rate: Some(COMMANDED_FEED),
        plunge_rate: Some(385.0),
        climb: true,
        pattern: PocketPattern::Contour,
        spindle_rpm: Some(18_000),
        selection: rs_cam_core::selection::Selection::drawing(),
        ..PocketConfig::default()
    };
    let mut tc = common::session::toolpath_config(
        "Pocket",
        OperationConfig::Pocket(pocket),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    tc.dressups.feed_max_rate = feed_max_rate;
    let _ = builder.add_toolpath(0, tc).expect("add the pocket");
    let mut session = builder.build();
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    let _ = session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("generate the pocket");
    let opts = SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(1.0),
        adaptive_feed_modulation: true,
        modulation_strategy: ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
        ..SimulationOptions::default()
    };
    let _ = session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation completes");
    session
}

fn fed_feeds(session: &ProjectSession) -> Vec<f64> {
    session
        .result_at(0)
        .expect("generated")
        .toolpath()
        .moves
        .iter()
        .filter_map(|m| m.move_type.feed_rate())
        .collect()
}

fn max_of(feeds: &[f64]) -> f64 {
    feeds.iter().copied().fold(0.0, f64::max)
}

#[test]
fn no_modulated_feed_exceeds_feed_max_rate() {
    // 2. Non-vacuity first: with the machine ceiling only, the modulation
    // lifts a feed above the rate.
    let uncapped = fed_feeds(&modulated_pocket(None));
    assert!(
        max_of(&uncapped) > RATE,
        "the fixture must lift a feed above {RATE} without the cap, or the \
         claim is vacuous: max {}",
        max_of(&uncapped)
    );

    // 1. With the rate, no fed move is above it, and the modulation still
    // lifts feeds above the commanded feed.
    let capped = fed_feeds(&modulated_pocket(Some(RATE)));
    let over: Vec<f64> = capped
        .iter()
        .copied()
        .filter(|f| *f > RATE + 1e-6)
        .collect();
    assert!(
        over.is_empty(),
        "{} fed moves above feed_max_rate {RATE}: max {}",
        over.len(),
        max_of(&over)
    );
    assert!(
        capped.iter().any(|f| *f > COMMANDED_FEED + 1e-6),
        "the capped run must still modulate a feed above the commanded {COMMANDED_FEED}"
    );
}

#[test]
fn a_rate_above_the_machine_ceiling_changes_nothing() {
    let session = modulated_pocket(None);
    let ceiling = session.machine().cutting_ceiling().mm_min();
    let above = fed_feeds(&modulated_pocket(Some(2.0 * ceiling)));
    assert_eq!(
        fed_feeds(&session),
        above,
        "a feed_max_rate above the machine ceiling ({ceiling}) must only lower, \
         never raise or move a feed"
    );

    // The typed cap only lowers, and ignores a value it cannot read.
    let typed = session.machine().cutting_ceiling();
    assert_eq!(typed.capped_at(Some(2.0 * ceiling)), typed);
    assert_eq!(typed.capped_at(None), typed);
    assert_eq!(typed.capped_at(Some(f64::NAN)), typed);
    assert_eq!(typed.capped_at(Some(0.0)), typed);
    assert_eq!(typed.capped_at(Some(RATE)).mm_min(), RATE.min(ceiling));
}
