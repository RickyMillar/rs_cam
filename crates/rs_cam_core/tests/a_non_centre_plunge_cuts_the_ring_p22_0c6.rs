//! Feeds contract P22-0c-6b, D6
//! (`planning/feeds_contract_2026-11-03/phases/P22-0c-6.md`, sections 1.4
//! and 2.3): a straight plunge of a tool that does not cut at its centre
//! cuts the ring outside the core, not the full circle.
//!
//! From the zone radii (core radius `c`, outer `R = D / 2`):
//! `A = pi (R^2 - c^2)` and `J = integral from c to R of 2 pi r dr`, the
//! same value. Both terms of `P = [Ks A vf + F_edge Z n J] / 6e7` take
//! `k = 1 - (d_core / D)^2`, so the power and the MRR take `k` and the
//! specific energy `P / MRR` does not change.
//!
//! The claim: an 18 mm 2F flat with an 8 mm core (wanaka tool 6) in a drill
//! cycle gives `k = 1 - (8 / 18)^2 = 260 / 324 = 0.802469136` times the
//! power, the area and the MRR of its centre-cutting twin, to 1e-12
//! relative, through the operating point (`power_at_operating_point`) and
//! Suggest (`FeedsResult::power_kw`, `FeedsResult::mrr_mm3_min`). The section
//! kind stays `AxialCircle` and stays advisory (`gates()` is false). The
//! centre-cutting 6 mm section keeps its bits. Red before: the ratio is 1.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::f64::consts::PI;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::feeds::section::{EngagedSection, SectionKind};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::{
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut, power_at_operating_point,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// Wanaka tool 6: 18 mm, 2 flutes, core 8 mm, protrusion 1 mm.
const D_MM: f64 = 18.0;
const CORE_MM: f64 = 8.0;
const PROTRUSION_MM: f64 = 1.0;
const FEED_MM_MIN: f64 = 1000.0;
const RPM: u32 = 18_000;

/// `k = 1 - (d / D)^2`, from the facts.
fn ring_k() -> f64 {
    1.0 - (CORE_MM / D_MM).powi(2)
}

fn flat(centre: CentreCutting) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(6), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.shaft_diameter = D_MM;
    tool.set_cutting_length(ToolFact::Stated(20.0));
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_centre_cutting(ToolFact::Stated(centre));
    tool
}

fn cored() -> ToolConfig {
    flat(CentreCutting::No(NonCentreCuttingGeometry {
        centre_core_diameter_mm: CORE_MM,
        blade_protrusion_mm: PROTRUSION_MM,
    }))
}

fn twin() -> ToolConfig {
    flat(CentreCutting::Yes)
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn drill() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        floor: rs_cam_core::compute::floor::Floor::Top { depth: 8.0 },
        feed_rate: Some(FEED_MM_MIN),
        spindle_rpm: Some(RPM),
        ..DrillConfig::default()
    })
}

fn assert_ratio(what: &str, ring: f64, circle: f64) {
    let k = ring_k();
    assert!(circle > 0.0, "{what}: the twin has a positive figure");
    assert!(
        (ring / circle - k).abs() <= 1e-12 * k,
        "{what}: ring {ring} / circle {circle} = {}, want k = {k}",
        ring / circle
    );
}

#[test]
fn a_non_centre_plunge_cuts_the_ring_p22_0c6() {
    // The factor of the worked example (section 2.3): 260 / 324.
    assert!((ring_k() - 260.0 / 324.0).abs() <= 1e-15);
    assert!((ring_k() - 0.802469136).abs() < 5e-10);

    // The section: the area of the ring, from the zone radii.
    let ring = EngagedSection::axial_plunge(ToolGeometryHint::Flat, D_MM, CORE_MM)
        .expect("a flat end mill has an axial section");
    let circle = EngagedSection::axial_plunge(ToolGeometryHint::Flat, D_MM, 0.0)
        .expect("a flat end mill has an axial section");
    let want_ring = PI * (D_MM * D_MM - CORE_MM * CORE_MM) / 4.0;
    assert!((ring.area_mm2() - want_ring).abs() <= 1e-12 * want_ring);
    assert_ratio("section area", ring.area_mm2(), circle.area_mm2());
    assert_eq!(ring.kind(), SectionKind::AxialCircle);

    // The operating point (the shipped power figure).
    let machine = MachineProfile::generic_wood_router();
    let figure = |tool: &ToolConfig| {
        power_at_operating_point(&drill(), tool, &oak(), &machine, None)
            .expect("a flat end mill plunge has the axial section")
    };
    let (ring_fig, circle_fig) = (figure(&cored()), figure(&twin()));
    assert_eq!(
        ring_fig.feed_mm_min.to_bits(),
        circle_fig.feed_mm_min.to_bits()
    );
    assert_ratio(
        "operating point power",
        ring_fig.required_kw,
        circle_fig.required_kw,
    );
    assert_eq!(ring_fig.section, SectionKind::AxialCircle);
    assert_eq!(circle_fig.section, SectionKind::AxialCircle);
    assert!(!ring_fig.section.gates(), "the axial figure stays advisory");

    // Suggest: the published power and the removal rate.
    let suggest = |tool: &ToolConfig| {
        calculate(&feeds_input_for_operation(
            &drill(),
            tool,
            &oak(),
            &machine,
            embedded_vendor_lut(),
            SpindleStrategy::MatchChart,
        ))
    };
    let (ring_r, circle_r) = (suggest(&cored()), suggest(&twin()));
    assert_eq!(
        ring_r.feed_rate_mm_min.to_bits(),
        circle_r.feed_rate_mm_min.to_bits()
    );
    assert_eq!(ring_r.rpm.to_bits(), circle_r.rpm.to_bits());
    let ring_kw = ring_r.power_kw.expect("the drill cycle has a power figure");
    let circle_kw = circle_r
        .power_kw
        .expect("the drill cycle has a power figure");
    assert_ratio("Suggest power", ring_kw, circle_kw);
    let ring_mrr = ring_r
        .mrr_mm3_min
        .expect("the drill cycle has a removal rate");
    let circle_mrr = circle_r
        .mrr_mm3_min
        .expect("the drill cycle has a removal rate");
    assert_ratio("Suggest MRR", ring_mrr, circle_mrr);
    // The specific energy does not change.
    let (u_ring, u_circle) = (ring_kw / ring_mrr, circle_kw / circle_mrr);
    assert!(
        (u_ring - u_circle).abs() <= 1e-12 * u_circle,
        "specific energy {u_ring} vs {u_circle}"
    );
}

/// The centre-cutting pin keeps its bits: `k = 1` exactly.
#[test]
fn a_centre_cutting_plunge_keeps_the_full_circle_p22_0c6() {
    let d = 6.0;
    let section = EngagedSection::axial_plunge(ToolGeometryHint::Flat, d, 0.0)
        .expect("a flat end mill has an axial section");
    assert_eq!(section.area_mm2().to_bits(), (PI * d * d / 4.0).to_bits());
}
