//! Feeds contract P11b2 (`planning/feeds_contract_2026-11-03/STATUS.md` item
//! 3a; a consequence of ruling L6): the post-sim chipload gate reads the
//! printed band at every depth, as Suggest does.
//!
//! P11b removed the vendor depth table (1 x D full, 2 x D less 25 %, 3 x D
//! less 50 %) from the Suggest chip and band; the envelope draws the table as
//! a limit (`LimitKind::VendorDepthTable`). Before P11b2 the gate still
//! lowered its band by the table at the peak steady-state depth, so at 2 x D
//! it judged a cut against 0.75 of the band that Suggest sized the feed for.
//!
//! The fixture is a hand-built trace on a D 6, Z 2 flat end mill in white
//! oak, a roughing pocket at a full slot (no chip thinning, so the observed
//! chip is the advance at every depth). One arm cuts at 1 x D, one at
//! 2 x D; everything else is equal.
//!
//! Arm: the gate band at 2 x D equals the gate band at 1 x D (red before
//! P11b2: 0.75 x).

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipBounds, ChiploadVerdict};
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const RPM: u32 = 18_000;
const FEED_MM_MIN: f64 = 1800.0;
const CUTTING_LENGTH_MM: f64 = 25.0;
const SAMPLES: usize = 24;
const TOOLPATH: ToolpathId = ToolpathId::from_raw(0);

fn end_mill() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(D_MM, CUTTING_LENGTH_MM)),
        6.35,
        45.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn gate_bounds_at_depth(ap_mm: f64) -> ChipBounds {
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let fz = FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES));
    let arc = std::f64::consts::PI;
    let chip =
        rs_cam_core::dexel_stock::effective_chip_thickness_mm(&tool, ap_mm, Some(arc), fz, FLUTES)
            .expect("the chip model resolves at this depth and arc");
    let samples = (0..SAMPLES)
        .map(|i| SimulationCutSample {
            toolpath_id: TOOLPATH,
            move_index: i + 1,
            sample_index: i,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: FEED_MM_MIN,
            spindle_rpm: RPM,
            flute_count: FLUTES,
            axial_doc_mm: ap_mm,
            axial_engagement_mm: ap_mm,
            arc_engagement_radians: Some(arc),
            chipload_mm_per_tooth: fz,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(1.0),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        })
        .collect();
    let trace = SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        predicted_feeds: PredictedFeedMap::new(),
        ..SimulationCutTrace::test_fixture()
    };
    let verdict = evaluate_toolpath(
        &ToolpathLoadContext::of_operation(
            TOOLPATH,
            &rs_cam_core::compute::catalog::OperationConfig::new_default(OperationType::Pocket),
            &tool,
            &material,
            rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(FEED_MM_MIN),
            None,
            rs_cam_core::limits::CapTable::empty(),
            None,
        ),
        Some(&trace),
        None,
        &ToleranceBands::default(),
    );
    match verdict.chipload {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.bounds,
        ChiploadVerdict::Exceeds { triggering, .. } => triggering.bounds,
        ChiploadVerdict::Unmodeled { reason } => {
            panic!("fixture: the gate must reach a modelled verdict; got Unmodeled({reason:?})")
        }
    }
}

#[test]
fn the_gate_does_not_lower_the_band_at_two_diameters_p11b2() {
    let at_1d = gate_bounds_at_depth(D_MM);
    let at_2d = gate_bounds_at_depth(2.0 * D_MM);
    assert!(
        at_1d.max_mm_per_tooth > 0.0,
        "fixture: the gate matched a band at 1 x D ({at_1d:?})"
    );
    assert_eq!(
        at_2d.max_mm_per_tooth, at_1d.max_mm_per_tooth,
        "P11b2 (L6): the gate band max at 2 x D is the printed max, as at 1 x D"
    );
    assert_eq!(
        at_2d.min_mm_per_tooth, at_1d.min_mm_per_tooth,
        "P11b2 (L6): the gate band min at 2 x D is the printed min, as at 1 x D"
    );
}
