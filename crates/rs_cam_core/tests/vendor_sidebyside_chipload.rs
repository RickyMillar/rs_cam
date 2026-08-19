//! **VSBS** — vendor side-by-side chipload spot-check probe.
//!
//! A reproduction of current state, not a bar. It exists so that
//! `planning/review_2026-08-08/VENDOR_SIDEBYSIDE_CHIPLOAD.md` prints
//! numbers produced by **production code** — `feeds::calculate` on the
//! **shipped** vendor LUT — rather than numbers re-derived by hand in
//! prose.
//!
//! For each probe it reports, in one line per query:
//!
//! - which vendor observation the matcher actually chose;
//! - that row's RAW stored chipload band (mm/tooth), read back out of
//!   `VendorLut::embedded()` by `observation_id`;
//! - the RAW transfer ratios and the APPLIED scales, i.e. the
//!   `D^CHIPLOAD_DIAMETER_EXPONENT` and `Janka^-CHIPLOAD_HARDNESS_EXPONENT`
//!   laws (both repo-derived — `CREDITS.md` says so);
//! - the DOC-derated band `FeedsResult::chipload_bounds`, which is what
//!   the post-sim gate compares against;
//! - the commanded advance per tooth `feed_rate / (rpm * flutes)` —
//!   the same quantity the chipload gate observes since 2026-08-06 —
//!   and where it sits inside both the derated and the raw vendor band.
//!
//! Assertions here are deliberately structural + directional. The
//! *magnitudes* live in the document, because several downstream clamps
//! (RCTF, LD-overhang, rubbing floor, machine ceiling, safety factor)
//! compose into the commanded feed and pinning their composition in a
//! test would make this file a tripwire for unrelated work.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::vendor_lut::VendorLut;
use rs_cam_core::feeds::{
    ChiploadSource, FeedsInput, FeedsResult, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{AluminumAlloy, Material, WoodSpecies};

struct Probe {
    label: &'static str,
    diameter_mm: f64,
    flutes: u32,
    geometry: ToolGeometryHint,
    material: Material,
    operation: OperationFamily,
    pass_role: PassRole,
    doc_mm: f64,
}

fn probes() -> Vec<Probe> {
    vec![
        Probe {
            label: "A  Ø3.0 2F flat / Ipe (Janka 3510) / pocket rough / DOC 0.6",
            diameter_mm: 3.0,
            flutes: 2,
            geometry: ToolGeometryHint::Flat,
            material: Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            doc_mm: 0.6,
        },
        Probe {
            label: "B  Ø6.0 2F flat / white oak (1360) / contour finish / DOC 12.0 (2xD)",
            diameter_mm: 6.0,
            flutes: 2,
            geometry: ToolGeometryHint::Flat,
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            operation: OperationFamily::Contour,
            pass_role: PassRole::Finish,
            doc_mm: 12.0,
        },
        Probe {
            label: "C  Ø6.0 2F flat / hard maple (1450) / pocket rough / DOC 4.0",
            diameter_mm: 6.0,
            flutes: 2,
            geometry: ToolGeometryHint::Flat,
            material: Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            doc_mm: 4.0,
        },
        Probe {
            label: "D  Ø1.5 2F ball / radiata pine (710) / parallel finish / DOC 0.3",
            diameter_mm: 1.5,
            flutes: 2,
            geometry: ToolGeometryHint::Ball,
            material: Material::SolidWood {
                species: WoodSpecies::RadiataPine,
            },
            operation: OperationFamily::Parallel,
            pass_role: PassRole::Finish,
            doc_mm: 0.3,
        },
        Probe {
            label: "E  Ø12.7 2F flat / white oak (1360) / pocket rough / DOC 6.0",
            diameter_mm: 12.7,
            flutes: 2,
            geometry: ToolGeometryHint::Flat,
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            doc_mm: 6.0,
        },
        Probe {
            label: "F  Ø3.0 2F flat / 6061-T6 aluminium / pocket rough / DOC 1.5",
            diameter_mm: 3.0,
            flutes: 2,
            geometry: ToolGeometryHint::Flat,
            material: Material::Aluminum {
                alloy: AluminumAlloy::Alloy6061T6,
            },
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            doc_mm: 1.5,
        },
    ]
}

fn run(probe: &Probe) -> FeedsResult {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    calculate(&FeedsInput {
        tool_diameter: probe.diameter_mm,
        flute_count: probe.flutes,
        flute_length: probe.diameter_mm * 4.0,
        shank_diameter: None,
        tool_geometry: probe.geometry,
        material: &probe.material,
        machine: &machine,
        operation: probe.operation,
        operation_kind: None,
        pass_role: probe.pass_role,
        axial_depth_mm: Some(probe.doc_mm),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// Raw (unscaled, underated) stored band for an observation id, read
/// back out of the shipped LUT so the document can quote the number the
/// repo ships rather than a number recomputed from the scales.
fn raw_band(lut: &VendorLut, observation_id: &str) -> Option<(f64, f64, String, String)> {
    lut.observations
        .iter()
        .find(|o| o.observation_id == observation_id)
        .and_then(|o| {
            Some((
                o.chipload_min_mm_tooth?,
                o.chipload_max_mm_tooth?,
                format!("{:?}", o.source_vendor),
                o.source_page.clone().unwrap_or_default(),
            ))
        })
}

#[test]
fn vendor_sidebyside_chipload_spotcheck() {
    let lut = embedded_vendor_lut();
    let mut vendor_backed = 0usize;

    for probe in probes() {
        let result = run(&probe);
        let divisor = result.rpm * f64::from(probe.flutes);
        let commanded_fpt = if divisor > 0.0 {
            result.feed_rate_mm_min / divisor
        } else {
            0.0
        };

        println!("\n=== {} ===", probe.label);
        println!(
            "  rpm={:.1}  feed={:.3} mm/min  commanded fpt={:.6} mm/tooth",
            result.rpm, result.feed_rate_mm_min, commanded_fpt
        );
        println!("  chipload_source = {:?}", result.chipload_source);
        let dr = &result.derates;
        println!(
            "  ap={:.4} ae={:.4} effective_d={:.4}",
            result.axial_depth_mm, result.radial_width_mm, result.effective_diameter_mm
        );
        println!(
            "  target_chip_load={:.6}   chip-thinning OBSERVED but NOT applied: \
             rctf={:.6} axial={:.6} combined={:.6}",
            dr.target_chip_load_mm,
            dr.observed_radial_chip_thinning,
            dr.observed_axial_chip_thinning,
            dr.observed_combined_chip_thinning
        );
        println!(
            "           depth_tier={:.6}  ld={:.6}  workholding={:.6}  power={:.6}  \
             feed_clamp={:.6}  safety={:.6}  spindle_speedup={:.6}",
            dr.depth_tier,
            dr.ld_overhang,
            dr.workholding,
            dr.power_limit,
            dr.feed_clamp,
            dr.safety_factor,
            dr.spindle_speedup
        );

        match (&result.matched_lut_row, result.chipload_bounds) {
            (Some(row), Some(bounds)) => {
                vendor_backed += 1;
                let id = row.observation_id.clone();
                let (raw_min, raw_max, vendor, page) = raw_band(lut, &id).unwrap_or((
                    f64::NAN,
                    f64::NAN,
                    "?".to_owned(),
                    String::new(),
                ));
                let total_scale = row.chipload_diameter_scale * row.chipload_hardness_scale;
                let derate = if raw_min > 0.0 && total_scale > 0.0 {
                    bounds.min_mm_per_tooth / (raw_min * total_scale)
                } else {
                    f64::NAN
                };
                println!("  row = {id}   vendor={vendor}   page={page}");
                println!("  raw stored band      = {raw_min:.5} .. {raw_max:.5} mm/tooth");
                println!(
                    "  raw ratios           = D {:.6}  Janka {:.6}   (extrapolated={})",
                    row.chipload_diameter_ratio_raw,
                    row.chipload_hardness_ratio_raw,
                    row.is_extrapolated
                );
                println!(
                    "  applied scales       = D {:.6}  Janka {:.6}   total {:.6}",
                    row.chipload_diameter_scale, row.chipload_hardness_scale, total_scale
                );
                println!(
                    "  transferred band     = {:.6} .. {:.6}",
                    raw_min * total_scale,
                    raw_max * total_scale
                );
                println!("  DOC derate factor    = {derate:.6}");
                println!(
                    "  derated band (gate)  = {:.6} .. {:.6}",
                    bounds.min_mm_per_tooth, bounds.max_mm_per_tooth
                );
                let mid = 0.5 * (bounds.min_mm_per_tooth + bounds.max_mm_per_tooth);
                println!(
                    "  commanded / derated  = min {:.4}x   mid {:.4}x   max {:.4}x",
                    commanded_fpt / bounds.min_mm_per_tooth,
                    commanded_fpt / mid,
                    commanded_fpt / bounds.max_mm_per_tooth
                );
                let raw_mid = 0.5 * (raw_min + raw_max);
                println!(
                    "  commanded / RAW      = min {:.4}x   mid {:.4}x   max {:.4}x",
                    commanded_fpt / raw_min,
                    commanded_fpt / raw_mid,
                    commanded_fpt / raw_max
                );
            }
            _ => {
                println!("  NO vendor band (matched_lut_row or chipload_bounds absent)");
            }
        }
        for w in &result.warnings {
            println!("  warning: {w:?}");
        }

        assert!(
            result.rpm > 0.0 && result.feed_rate_mm_min > 0.0,
            "{}: every probe must produce a usable recipe",
            probe.label
        );
        assert!(
            commanded_fpt > 0.0,
            "{}: commanded advance per tooth must be positive",
            probe.label
        );
    }

    assert!(
        vendor_backed >= 5,
        "expected at least 5 of 6 probes to resolve to a banded vendor LUT row, got {vendor_backed}"
    );
}

/// **The anchor is the band MIDPOINT, not the band minimum.**
///
/// Measured 2026-08-16, and it corrects a claim in circulation. The
/// Checkpoint J-1 retirement note records the post-retirement resting
/// point as "the derated band **minimum**, which the calculator already
/// reaches unaided (measured 0.999× on both Adaptive3d fixtures)"
/// (`feeds/suggest.rs`, pass-8 retirement comment). That reading is an
/// *outcome* on two Adaptive3d fixtures, not the mechanism: the
/// calculator seeds `chip_load` from the matched row's **midpoint**
/// (`vendor_lookup::build_result` — `chipload_midpoint(obs) *
/// total_scale`) and then composes multipliers onto it. Landing near
/// the band floor happens when those multipliers happen to compose near
/// `band_min / band_mid`; it is not a targeted floor.
///
/// What is pinned here is the mechanism, which is stable: on every
/// probe whose feed is neither machine-clamped nor power-limited, the
/// commanded advance per tooth equals
///
/// ```text
/// target_chip_load_mm x depth_tier
///                     x ld x workholding x safety_factor x spindle_speedup
/// ```
///
/// **`combined_chip_thinning` left this identity on 2026-08-19**
/// (G-CHIPTHIN-HALFFIX, operator-ruled): the calculator no longer multiplies
/// the feed by it, so composing it here would predict a feed the engine does
/// not emit. It is still reported above, as an observation.
///
/// to float precision, where `target_chip_load_mm` is the matched row's
/// **transferred** midpoint. So the same identity that puts one op at
/// 0.79x the derated midpoint puts another at 1.7x it — the
/// composition, not a target, decides where in the vendor window the
/// recommendation lands.
#[test]
fn the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack() {
    let mut checked = 0usize;
    for probe in probes() {
        let result = run(&probe);
        if result.chipload_bounds.is_none()
            || !matches!(result.chipload_source, ChiploadSource::VendorLut { .. })
        {
            continue;
        }
        // Only the unclamped, unlimited arms: the machine feed ceiling
        // and the power back-off both truncate the identity by
        // construction, and both are reported separately in the doc.
        let dr = &result.derates;
        if dr.feed_clamp < 1.0 || dr.power_limit < 1.0 {
            continue;
        }
        let divisor = result.rpm * f64::from(probe.flutes);
        assert!(divisor > 0.0, "{}: no fpt divisor", probe.label);
        let commanded_fpt = result.feed_rate_mm_min / divisor;
        let predicted = dr.target_chip_load_mm
            * dr.depth_tier
            * dr.ld_overhang
            * dr.workholding
            * dr.safety_factor
            * dr.spindle_speedup;
        assert!(
            (commanded_fpt - predicted).abs() < 1e-9,
            "{}: commanded {commanded_fpt:.9} != seed-midpoint identity {predicted:.9} \
             (derates {dr:?})",
            probe.label,
        );
        checked += 1;
    }
    assert!(
        checked >= 4,
        "expected at least 4 unclamped vendor-banded probes to exercise the identity, \
         got {checked}"
    );
}

/// **The two sides of the comparison carry DIFFERENT DOC derates, and
/// they are not the same function.**
///
/// `FeedsResult::chipload_bounds` — the band the post-sim gate judges
/// against — is scaled by `geometry::doc_derating_scale`, a PIECEWISE
/// LINEAR ramp (1.000 at 1xD, 0.750 at 2xD, 0.500 at and past 3xD).
/// `FeedsResult::derates.target_chip_load_mm` — the seed the feed is
/// actually built from — is NOT scaled by it at all; the feed instead
/// takes `geometry::depth_tier_multiplier`, a STEP function
/// (1.00 / 0.75 / 0.50 / 0.45 for ratio <=1 / >1 / >2 / >3).
///
/// The two agree exactly at the vendor break points the rule is quoted
/// from (1xD, 2xD, 3xD) and diverge everywhere between them — at
/// DOC/Ø = 1.5 the band is derated 0.875 while the feed is derated
/// 0.750, so the commanded advance sits 14 % lower against the band
/// than the same recipe would at 2xD. Reproduction of current state,
/// pinned on probe B (DOC/Ø = 2.0), the only probe with a non-unit DOC
/// derate.
#[test]
fn the_band_derate_and_the_feed_derate_are_different_functions() {
    let probe = probes()
        .into_iter()
        .find(|p| p.label.starts_with("B "))
        .expect("probe B must exist");
    let result = run(&probe);
    let bounds = result
        .chipload_bounds
        .expect("probe B must resolve a banded vendor row");
    let dr = &result.derates;
    let derated_mid = 0.5 * (bounds.min_mm_per_tooth + bounds.max_mm_per_tooth);

    // The seed is the transferred midpoint BEFORE the DOC derate, so at
    // DOC/Ø = 2.0 it sits at exactly 1 / 0.75 of the published band's
    // midpoint.
    assert!(
        (dr.target_chip_load_mm - derated_mid / 0.75).abs() < 1e-9,
        "probe B: seed {:.9} should be the UN-DOC-derated midpoint, i.e. \
         derated mid {derated_mid:.9} / 0.75",
        dr.target_chip_load_mm,
    );
    assert!(
        (dr.depth_tier - 0.75).abs() < 1e-12,
        "probe B: depth_tier should be 0.75 at DOC/Ø = 2.0, got {}",
        dr.depth_tier,
    );
}

/// **INVERTED 2026-08-19 (G-CHIPTHIN-HALFFIX), not deleted.**
///
/// This test used to assert the defect: a sub-Ø2 ball finishing recommendation
/// landed **above** the vendor window it was derived from, at **1.478× the
/// derated band ceiling**, because the chip-thinning stack multiplied the band
/// midpoint by more than the band's own max/mid ratio. The gate has observed a
/// plain advance per tooth since 2026-08-06 and is engagement-blind, so it read
/// that as an exceedance. It was the headline of the VSBS side-by-side and the
/// reason `VSBS-SEED` was raised.
///
/// The multiplication is gone. Probe D now commands **0.034067 mm/tooth**
/// against a derated band of **0.034455..0.056390** — it falls just under the
/// vendor minimum rather than over the maximum, a 1.5 % undershoot in place of
/// a 47.8 % overshoot.
///
/// The assertion is inverted rather than dropped, per this repo's convention
/// for a defect reproduction that has been fixed (`arc_fit_disposition_a5`
/// did the same when pass 8 retired): the old magnitude stays in the docs so
/// the fix remains checkable, and the new assertion fails if the multiplication
/// ever comes back.
#[test]
fn the_sub_two_millimetre_ball_probe_no_longer_commands_above_its_vendor_band() {
    let probe = probes()
        .into_iter()
        .find(|p| p.label.starts_with("D "))
        .expect("probe D must exist");
    let result = run(&probe);
    let bounds = result
        .chipload_bounds
        .expect("probe D must resolve a banded vendor row");
    let commanded_fpt = result.feed_rate_mm_min / (result.rpm * f64::from(probe.flutes));
    assert!(
        commanded_fpt <= bounds.max_mm_per_tooth,
        "probe D commanded {commanded_fpt:.6}, ABOVE the derated band max {:.6} \
         (band {:.6}..{:.6}) — that is {:.3}× the ceiling. Chip thinning was deleted from \
         the feed on 2026-08-19 by operator ruling precisely to stop this; if it is back, \
         see the Step 5 note in feeds/mod.rs before re-pinning.",
        bounds.max_mm_per_tooth,
        bounds.min_mm_per_tooth,
        bounds.max_mm_per_tooth,
        commanded_fpt / bounds.max_mm_per_tooth,
    );
    // And the direction it moved to, so a future regression that overshoots
    // the OTHER way is caught too. The undershoot is expected and small: a
    // sub-Ø2 ball in this material simply cannot be fed hard.
    assert!(
        commanded_fpt > bounds.min_mm_per_tooth * 0.9,
        "probe D commanded {commanded_fpt:.6}, more than 10 % under the derated band \
         minimum {:.6} — the deletion was expected to land it just below the minimum \
         (measured 0.034067 vs 0.034455, a 1.5 % undershoot), not to collapse it",
        bounds.min_mm_per_tooth,
    );
}
