//! Literature-matrix regression — extreme hardwood (Ipe, Janka 3510)
//! must derate chipload below oak's baseline (Janka 1290) when the
//! winning vendor row carries no per-row `hardness_value`.
//!
//! Cell: `flat_3mm_pocket_ipe_extreme`. Pre-fix, the LUT path's
//! `hardness_ratio_raw` (in `feeds/vendor_lookup.rs`, named
//! `hardness_scale_factor` until 2026-08-06) silently
//! degraded to identity (scale = 1.0) whenever the matched row was
//! missing its `hardness_value` annotation — even when the row WAS
//! wood-family tagged. The Ipe 3 mm pocket query matched
//! `whiteside-ru1600-upcut-spiral-fusion360` (material_family =
//! hardwood, no hardness_value), so Ipe inherited the row's full
//! chipload target unscaled and ended up at the same fpt as oak.
//!
//! The fix synthesises a family-default Janka anchor (1290 for
//! `Hardwood`) when the row has no per-row hardness and the query is
//! Janka-tagged, so Ipe (3510) derates against the Hardwood anchor.
//!
//! This sentry locks in: for the 3 mm flat-end pocket query, Ipe must
//! command a strictly lower chipload than red oak. **The assertion is
//! DIRECTIONAL** — it pins the sign of the derate, not its size, and
//! that is deliberate: several downstream clamps (RCTF, LD-overhang,
//! rubbing floor, safety factor) close part of the gap, so a magnitude
//! assertion here would be pinning their composition rather than the
//! hardness law.
//!
//! ## Docstring corrected 2026-08-06 — the "≈ 0.37×" was two claims,
//! ## one of which was never true of this test
//!
//! It used to read *"so Ipe (3510) derates by ≈ 1290 / 3510 ≈ 0.37×
//! against the Hardwood anchor"*. Two problems:
//!
//! 1. `0.37×` is the **hardness scale applied to the LUT row's band**,
//!    not the derate this test observes. What the test measures is
//!    `feed_rate / (rpm × flutes)` out of the whole of
//!    `feeds::calculate`, after every clamp. Quoting the scale factor as
//!    if it were the outcome invited exactly the magnitude assertion the
//!    paragraph above explains this test must not make.
//! 2. `1290 / 3510` was the ratio under the then-shipped `^1.0`
//!    hardness law. **`vendor_lookup::CHIPLOAD_HARDNESS_EXPONENT` was
//!    adopted at 0.5 later the same day**, so the transfer this cell
//!    exercises is now `0.3675^0.5 = 0.606×`, not `0.3675×`. A number
//!    written into a docstring as if it were a law is the "stale
//!    rationale outliving the code" class the programme's P11 names,
//!    and this one would have gone stale within hours of being written.
//!
//! B-lit §4.2 also settled a census claim about this file, and the
//! adoption confirmed it: the census's T3.4 evidence column listed an
//! `_litmatrix_ipe_janka_scaling` re-pin as required if the hardness law
//! moves. **It was not required.** The law moved and this file's
//! assertion did not, because it is directional and `0.606 < 1.0` holds
//! exactly as `0.3675 < 1.0` did. Nothing here changed for the
//! adoption — the docstring had already been made law-agnostic, which
//! is why.

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
        operation_kind: None,
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
    // material_family = hardwood) derates Ipe against the Hardwood
    // default (1290) — by `(1290/3510)^q` where `q` is
    // `vendor_lookup::CHIPLOAD_HARDNESS_EXPONENT`. The SIGN of that
    // derate is what this test pins; its size is a law parameter and
    // deliberately not asserted here (see the module docstring).
    // Downstream clamps (RCTF, LD-overhang, rubbing floor, safety) close
    // part of the gap, but Ipe's fpt must remain strictly below oak's.
    assert!(
        ipe_fpt < oak_fpt,
        "Ipe (Janka 3510) chipload {ipe_fpt:.4} must derate below \
         oak (Janka 1290) chipload {oak_fpt:.4}; pre-fix both \
         degraded to identity on the no-hardness Whiteside row",
    );
}
