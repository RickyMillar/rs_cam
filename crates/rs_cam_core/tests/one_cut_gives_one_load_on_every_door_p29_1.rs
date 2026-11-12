//! Feeds contract P29-1
//! (`planning/feeds_contract_2026-11-03/phases/P29_FINDINGS.md` section 4.3):
//! one cut gives one load on every door.
//!
//! `feeds::cut_load::CutLoad` is the one forward read of the power, the
//! engaged-edge force, the tip deflection and the peak chip of an
//! engagement at an operating point. Every door of these values calls it.
//!
//! - Claim A: each door equals `CutLoad` on that door's own engagement and
//!   operating point, bit for bit.
//! - Claim B: on one cut (a sample whose two depths are equal and whose
//!   flute count is the tool's), the planned doors and the sample doors
//!   agree within 1e-12 relative. The arc round trip `ae -> psi -> ae` is
//!   not exact, so this claim is not bit for bit.
//! - Claim C (printed, not asserted): the values of the sample doors when
//!   the flute count of the sample is not the flute count of the tool. This
//!   is the open item of P29 that `cut_load.rs` names.
//! - Claim D: outside `feeds/cut_load.rs` no product code calls a model
//!   function for a forward read. The exceptions are counted and named.
//! - Claim E (P29-3): the two straight-plunge doors (the operating-point
//!   figure and the calculator) equal `CutLoad::of_cut` on the planned
//!   drill cut, bit for bit. A shaped tip has no plunge model on either
//!   door, and the force, the deflection and the chip of a plunge are
//!   `NotModelled`.
//!
//! Inject check (2026-11-11): with `sample_power_kw` building
//! `OperatingPoint::planned(feed, rpm, tool.flute_count + 1)` in its own
//! file, claims A and B go red on the power door (5.39e-2 against 4.13e-2
//! kW).

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints claim C for the reader"
)]

mod common;

use common::wide_samples::core_source;

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::feeds::cut_load::{CutLoad, NoNumber, OperatingPoint};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::geometry::RadialThinning;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::ToolDefinition;

const D: f64 = 6.0;
const AP: f64 = 3.0;
const ARC: f64 = 1.1;
const RPM: u32 = 18_000;
const FEED: f64 = 3_600.0;

fn tool() -> ToolDefinition {
    build_cutter(&common::tools::endmill_tool_config(D)).expect("the cutter builds")
}

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// One cutting sample: both depths `AP`, the arc `ARC`, `flutes` flutes.
fn sample(tool: &ToolDefinition, flutes: u32) -> SimulationCutSample {
    let mut s = SimulationCutSample::test_fixture();
    s.is_cutting = true;
    s.axial_doc_mm = AP;
    s.axial_engagement_mm = AP;
    s.arc_engagement_radians = Some(ARC);
    s.engagement.radial_woc_fraction = (1.0 - ARC.cos()) / 2.0;
    s.feed_rate_mm_min = FEED;
    s.spindle_rpm = RPM;
    s.flute_count = flutes;
    s.chipload_mm_per_tooth = FEED / (f64::from(RPM) * f64::from(tool.flute_count));
    s
}

fn bits(what: &str, door: f64, load: f64) {
    assert_eq!(
        door.to_bits(),
        load.to_bits(),
        "{what}: the door gives {door:e}, CutLoad gives {load:e}"
    );
}

fn close(what: &str, a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()),
        "{what}: {a:e} against {b:e}"
    );
}

/// The four values of the sample doors, in the order power, peak force,
/// deflection, peak chip.
fn sample_doors(tool: &ToolDefinition, material: &Material, s: &SimulationCutSample) -> [f64; 4] {
    let line = material.force_line().expect("a force line");
    let no_predicted = rs_cam_core::machine::kinematics::PredictedFeedMap::default();
    let fz = rs_cam_core::tool_load::deflection::deflection_feed_per_tooth_mm(s, &no_predicted);
    [
        rs_cam_core::tool_load::power::sample_power_kw(tool, &line, s, FEED).expect("power"),
        rs_cam_core::tool_load::cut_force::sample_peak_force_n(tool, material, s, fz)
            .expect("force"),
        rs_cam_core::tool_load::deflection::sample_tip_deflection_mm(tool, material, s, fz)
            .expect("deflection"),
        rs_cam_core::tool_load::display::peak_chip_per_tooth(
            s,
            &no_predicted,
            tool,
            RadialThinning::Applied,
        )
        .expect("chip")
        .peak_chip_mm,
    ]
}

#[test]
fn each_sample_door_equals_the_one_load_bit_for_bit() {
    let (tool, material) = (tool(), material());
    let s = sample(&tool, tool.flute_count);
    let [power, force, deflection, chip] = sample_doors(&tool, &material, &s);

    let for_power = Engagement::from_sample(&s, &tool).expect("the power gate's engagement");
    let for_force = rs_cam_core::tool_load::deflection::sample_engagement(&tool, &s)
        .expect("the deflection gate's engagement");
    let power_load = CutLoad::of(
        &for_power,
        OperatingPoint::of_sample_for_power(&s, &tool, FEED),
        &tool,
        &material,
    );
    let force_load = CutLoad::of(
        &for_force,
        OperatingPoint::of_sample_for_deflection(&s, &tool, FEED),
        &tool,
        &material,
    );
    bits("power", power, power_load.power_kw().expect("power"));
    bits(
        "peak force",
        force,
        force_load.force().expect("force").peak_resultant_n,
    );
    bits(
        "deflection",
        deflection,
        force_load.deflection_mm().expect("deflection"),
    );
    let advance = FEED / (f64::from(RPM) * f64::from(s.flute_count));
    bits(
        "peak chip",
        chip,
        CutLoad::of(
            &for_power,
            OperatingPoint::of_sample_for_chip(&s, advance),
            &tool,
            &material,
        )
        .chip(RadialThinning::Applied)
        .expect("a side cut has a chip")
        .peak_mm,
    );
}

#[test]
fn each_planned_door_equals_the_one_load_bit_for_bit() {
    let (tool, material) = (tool(), material());
    let z = tool.flute_count;
    let r = D / 2.0;
    let ae = r * (1.0 - ARC.cos());
    let fz = FEED / (f64::from(RPM) * f64::from(z));
    let engagement = Engagement::from_tool_cut(
        CutPoint {
            ap_mm: AP,
            ae_mm: ae,
        },
        &tool,
    );
    let load = CutLoad::of(
        &engagement,
        OperatingPoint::of_chip(fz, z),
        &tool,
        &material,
    );
    bits(
        "force_budget::peak_force_n",
        rs_cam_core::feeds::force_budget::peak_force_n(&tool, &material, AP, ae, fz)
            .expect("force"),
        load.force().expect("force").peak_resultant_n,
    );
    bits(
        "tip_deflection_from_engagement",
        rs_cam_core::feeds::predict::tip_deflection_from_engagement(
            &tool,
            &material,
            &engagement,
            fz,
        )
        .expect("deflection"),
        load.deflection_mm().expect("deflection"),
    );
    // The planned point with a feed gives the same chip as the chip point.
    bits(
        "OperatingPoint::planned chip",
        OperatingPoint::planned(FEED, f64::from(RPM), z).fz_mm(),
        fz,
    );
}

#[test]
fn the_planned_doors_and_the_sample_doors_agree_on_one_cut() {
    let (tool, material) = (tool(), material());
    let z = tool.flute_count;
    let s = sample(&tool, z);
    let [power, force, deflection, chip] = sample_doors(&tool, &material, &s);
    let ae = (D / 2.0) * (1.0 - ARC.cos());
    let planned = CutLoad::of(
        &Engagement::from_tool_cut(
            CutPoint {
                ap_mm: AP,
                ae_mm: ae,
            },
            &tool,
        ),
        OperatingPoint::planned(FEED, f64::from(RPM), z),
        &tool,
        &material,
    );
    close("power", power, planned.power_kw().expect("power"));
    close(
        "peak force",
        force,
        planned.force().expect("force").peak_resultant_n,
    );
    close(
        "deflection",
        deflection,
        planned.deflection_mm().expect("deflection"),
    );
    close(
        "peak chip",
        chip,
        planned
            .chip(RadialThinning::Applied)
            .expect("a side cut has a chip")
            .peak_mm,
    );
    // The mean of the edge force is the power model's force: P = F v.
    let mean = planned.force().expect("force").mean_tangential_n;
    let speed_m_s = std::f64::consts::PI * D * f64::from(RPM) / 60_000.0;
    assert!(
        (mean * speed_m_s / 1000.0 - power).abs() <= 1e-9 * power,
        "mean force x cutting speed {} kW against the power {power} kW",
        mean * speed_m_s / 1000.0
    );
}

#[test]
fn the_doors_of_a_sample_whose_flute_count_is_not_the_tools_are_printed() {
    let (tool, material) = (tool(), material());
    let same = sample_doors(&tool, &material, &sample(&tool, tool.flute_count));
    let other_z = tool.flute_count + 1;
    let other = sample_doors(&tool, &material, &sample(&tool, other_z));
    eprintln!(
        "P29-1 claim C (open item): tool Z {}, sample Z {} against sample Z {}",
        tool.flute_count, tool.flute_count, other_z
    );
    let names = [
        ("power (the sample's Z in the edge term)", "kW"),
        (
            "peak force (chip from the sample's Z, edge sum with the tool's Z)",
            "N",
        ),
        ("deflection (as the force)", "mm"),
        ("peak chip (the sample's Z)", "mm"),
    ];
    for ((name, unit), (same, other)) in names.into_iter().zip(same.into_iter().zip(other)) {
        eprintln!(
            "  {name}: {same:.6} {unit} against {other:.6} {unit} (ratio {:.4})",
            other / same
        );
    }
}

/// A file that may hold a needle: the path, the count and the reason.
type Allowed = (&'static str, usize, &'static str);

/// The code of a core source file with no comment and no test module.
fn product_code(rel: &str) -> String {
    let src = core_source(rel);
    match src.find("#[cfg(test)]") {
        Some(at) => src[..at].to_owned(),
        None => src,
    }
}

fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the source folder reads") {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_product_code_calls_a_model_function_outside_the_one_load() {
    // The needle, and each file that may hold it with its count and reason.
    let rules: [(&str, &[Allowed]); 5] = [
        (
            ".edge_force(",
            &[
                ("src/feeds/cut_load.rs", 1, "the one forward read"),
                ("src/feeds/predict.rs", 1, "the deflection model function"),
            ],
        ),
        (
            "PowerTerms::of(",
            &[
                (
                    "src/feeds/cut_load.rs",
                    1,
                    "the one forward read of a side cut",
                ),
                (
                    "src/dressup/feed_modulation.rs",
                    1,
                    "an inverse: the feed is the unknown",
                ),
            ],
        ),
        (
            "tip_deflection_at(",
            &[
                ("src/feeds/cut_load.rs", 1, "the one forward read"),
                ("src/feeds/predict.rs", 1, "the definition"),
            ],
        ),
        (
            "PowerModelInputs::axial_plunge(",
            &[(
                "src/feeds/cut_load.rs",
                1,
                "the one forward read of a straight plunge (P29-3)",
            )],
        ),
        (
            "tip_deflection_from_engagement(",
            &[(
                "src/feeds/predict.rs",
                1,
                "the definition (a door of the one load)",
            )],
        ),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    assert!(files.len() > 100, "the scan reads the source tree");
    for (needle, allowed) in rules {
        let mut seen = 0usize;
        for file in &files {
            let rel = file
                .strip_prefix(root)
                .expect("a file under the crate")
                .to_string_lossy()
                .replace('\\', "/");
            if rel.ends_with("/tests.rs") || rel.contains("/tests/") {
                continue;
            }
            let count = product_code(&rel).matches(needle).count();
            seen += count;
            let want = allowed
                .iter()
                .find(|(path, _, _)| *path == rel)
                .map_or(0, |(_, n, _)| *n);
            assert_eq!(
                count, want,
                "{rel} holds `{needle}` {count} times; the rule allows {want}. A forward read \
                 of a load goes through feeds::cut_load::CutLoad"
            );
        }
        // Non-vacuity: the needle is in the tree.
        assert!(seen > 0, "the needle `{needle}` is in no product file");
    }
}

/// Claim E: an end mill of diameter `d` with `z` flutes and core `core`
/// (0: it cuts at its centre), or a ball nose.
fn drill_tool(
    d: f64,
    z: u32,
    core: f64,
    ball: bool,
) -> rs_cam_core::compute::tool_config::ToolConfig {
    use rs_cam_core::compute::tool_config::{
        CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
    };
    let kind = if ball {
        ToolType::BallNose
    } else {
        ToolType::EndMill
    };
    let mut tool = ToolConfig::test_default(ToolId::from_raw(6), kind);
    tool.diameter = d;
    tool.shaft_diameter = d;
    tool.set_cutting_length(ToolFact::Stated(20.0));
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_flute_count(ToolFact::Stated(z));
    let centre = if core > 0.0 {
        CentreCutting::No(NonCentreCuttingGeometry {
            centre_core_diameter_mm: core,
            blade_protrusion_mm: 1.0,
        })
    } else {
        CentreCutting::Yes
    };
    tool.set_centre_cutting(ToolFact::Stated(centre));
    tool
}

fn drill_op() -> rs_cam_core::compute::catalog::OperationConfig {
    rs_cam_core::compute::catalog::OperationConfig::Drill(
        rs_cam_core::compute::operation_configs::DrillConfig {
            floor: rs_cam_core::compute::floor::Floor::Top { depth: 8.0 },
            feed_rate: Some(1000.0),
            spindle_rpm: Some(RPM),
            ..rs_cam_core::compute::operation_configs::DrillConfig::default()
        },
    )
}

#[test]
fn each_plunge_door_equals_the_one_load_bit_for_bit() {
    use rs_cam_core::feeds::{SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut};
    use rs_cam_core::limits::{Cut, CutEngagement, NoEngagementModel};

    let machine = rs_cam_core::machine::MachineProfile::generic_wood_router();
    let oak = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let line = oak.force_line().expect("a force line");
    let mut doors = 0;
    for (d, z, core) in [(18.0, 2, 8.0), (18.0, 2, 0.0), (6.0, 3, 0.0)] {
        let tool = drill_tool(d, z, core, false);
        let body = rs_cam_core::feeds::CutBody::of_tool(&tool);
        // The operating-point figure.
        let figure =
            rs_cam_core::feeds::power_at_operating_point(&drill_op(), &tool, &oak, &machine, None)
                .expect("a flat end mill plunge has the axial section");
        let cut = Cut::planned_drill(
            ToolGeometryHint::Flat,
            d,
            body.core_diameter_mm,
            OperatingPoint::planned(
                figure.feed_mm_min,
                figure.rpm,
                tool.flute_count_planning().max(1),
            ),
        );
        let load = CutLoad::of_cut(&cut, None, Some(line)).expect("a planned point");
        bits(
            "operating-point plunge power",
            figure.required_kw,
            load.power_kw().expect("power"),
        );
        // The calculator.
        let input = rs_cam_core::feeds::FeedsInput::of_operation(
            &drill_op(),
            &tool,
            &oak,
            &machine,
            embedded_vendor_lut(),
            SpindleStrategy::MatchChart,
        );
        let result = calculate(&input);
        let cut = Cut::planned_drill(
            input.tool_geometry(),
            input.tool_diameter(),
            input.cut_body().core_diameter_mm,
            OperatingPoint::planned(result.feed_rate_mm_min, result.rpm, input.flute_count()),
        );
        let load = CutLoad::of_cut(&cut, None, Some(line)).expect("a planned point");
        bits(
            "calculator plunge power",
            result.power_kw.expect("the drill cycle has a power figure"),
            load.power_kw().expect("power"),
        );
        // Power only: a plunge has no side engagement.
        assert_eq!(load.force().err(), Some(NoNumber::NotModelled));
        assert_eq!(
            load.deflection_mm().err(),
            Some(NoNumber::NoToolBody),
            "no built tool given"
        );
        assert_eq!(
            load.chip(RadialThinning::DrillCycle).err(),
            Some(NoNumber::NotModelled)
        );
        doors += 2;
    }
    assert_eq!(doors, 6, "three tools, two doors each");

    // A ball plunges on a curved tip: no model on either door.
    let ball = drill_tool(6.0, 2, 0.0, true);
    let cut = Cut::planned_drill(
        ball.geometry_hint(),
        6.0,
        0.0,
        OperatingPoint::planned(1000.0, f64::from(RPM), 2),
    );
    assert_eq!(
        cut.engagement,
        CutEngagement::NoModel(NoEngagementModel::PlungeOfShapedTip)
    );
    let load = CutLoad::of_cut(&cut, None, Some(line)).expect("a planned point");
    assert_eq!(load.power_kw().err(), Some(NoNumber::NotModelled));
    assert_eq!(
        rs_cam_core::feeds::power_at_operating_point(&drill_op(), &ball, &oak, &machine, None)
            .err(),
        Some(rs_cam_core::feeds::PowerUnmodeled::AxialTipNotModelled)
    );
}
