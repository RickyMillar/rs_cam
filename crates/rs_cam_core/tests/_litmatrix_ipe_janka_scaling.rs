//! Literature-matrix regression — extreme hardwood (Ipe, Janka 3510)
//! must derate chipload below oak's baseline (Janka 1290) when the
//! winning vendor row carries no per-row `hardness_value`.
//!
//! Cell: `flat_3mm_pocket_ipe_extreme`. Pre-fix, the LUT path's
//! `hardness_scale_factor` (in `feeds/vendor_lookup.rs`) silently
//! degraded to identity (scale = 1.0) whenever the matched row was
//! missing its `hardness_value` annotation — even when the row WAS
//! wood-family tagged. The Ipe 3 mm pocket query matched
//! `whiteside-ru1600-upcut-spiral-fusion360` (material_family =
//! hardwood, no hardness_value), so Ipe inherited the row's full
//! chipload target unscaled and ended up at the same fpt as oak.
//!
//! The fix synthesises a family-default Janka anchor (1290 for
//! `Hardwood`) when the row has no per-row hardness and the query is
//! Janka-tagged, so Ipe (3510) derates by ≈ 1290 / 3510 ≈ 0.37×
//! against the Hardwood anchor.
//!
//! This sentry locks in: for the 3 mm flat-end pocket query, Ipe must
//! command a strictly lower chipload than red oak.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn calc_3mm_pocket(material: &Material) -> rs_cam_core::feeds::FeedsResult {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();

    calculate(&FeedsInput {
        tool_diameter: 3.0,
        flute_count: 2,
        flute_length: 12.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MaxSpeed,
    })
}

fn chipload(result: &rs_cam_core::feeds::FeedsResult, flutes: f64) -> f64 {
    result.feed_rate_mm_min / (result.rpm * flutes)
}

#[test]
fn ipe_chipload_derates_below_oak_when_row_lacks_hardness_annotation() {
    let flutes = 2.0_f64;
    let oak = calc_3mm_pocket(&Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    });
    let ipe = calc_3mm_pocket(&Material::SolidWood {
        species: WoodSpecies::Ipe,
    });

    assert!(oak.rpm > 0.0 && ipe.rpm > 0.0);
    assert!(oak.feed_rate_mm_min > 0.0 && ipe.feed_rate_mm_min > 0.0);

    let oak_fpt = chipload(&oak, flutes);
    let ipe_fpt = chipload(&ipe, flutes);

    // Ipe (Janka 3510) is ~2.7× harder than red oak (Janka 1290).
    // After the family-anchor fix, the LUT row (no hardness_value,
    // material_family = hardwood) should derate Ipe against the
    // Hardwood default (1290) by roughly 1290 / 3510 ≈ 0.37×.
    // Other downstream clamps (RCTF, LD-overhang, rubbing floor,
    // safety) may close some of that gap, but Ipe's fpt must remain
    // strictly below oak's after the fix.
    assert!(
        ipe_fpt < oak_fpt,
        "Ipe (Janka 3510) chipload {ipe_fpt:.4} must derate below \
         oak (Janka 1290) chipload {oak_fpt:.4}; pre-fix both \
         degraded to identity on the no-hardness Whiteside row",
    );
}
