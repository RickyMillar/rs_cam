//! Sentry: **the modulator's depth is a measurement in millimetres, never a
//! fraction of the flute length multiplied by something** (G-AXIALUNITS,
//! T-11, 2026-09-16).
//!
//! ## The defect this pins
//!
//! `feed_modulation::effective_axial_mm` used to compute:
//!
//! ```text
//! ctx.nominal_axial_doc_mm * engagement.axial_doc_fraction
//! ```
//!
//! `nominal_axial_doc_mm` is an absolute depth in mm. `axial_doc_fraction`
//! is NOT a fraction of it — `dexel_stock::simulation` writes it as
//! `axial_engagement_mm / flute_length`. The product was
//! `mm x (mm / flute_length)`, which is not a depth.
//!
//! The error factor was `flute_length / nominal_axial_doc_mm`, always too
//! SHALLOW because a pass is always shallower than the flute. A 2 mm pass on
//! a 25 mm flute read 0.16 mm — 12.5x low. That value scales the deflection
//! cap and the power cap in the constrained-max solver, so both ran far too
//! permissive and the modulator handed out feeds the machine should not have
//! been given.
//!
//! ## Why no gate could fail on it
//!
//! Every fixture in `constrained_max_modulation_f039.rs` and in the
//! `feed_modulation` unit tests pinned `axial_doc_fraction: 1.0` — 25 sites,
//! all of them 1.0. At exactly 1.0 the wrong expression returns
//! `nominal x 1.0`, which is the right answer. The single value that hides
//! the defect was the only value under test.
//!
//! **So this file deliberately never uses 1.0.** Every fixture below sets a
//! fraction that disagrees with the depth, which is the realistic case: a
//! 2 mm pass on a 25 mm flute is a fraction of 0.08.
//!
//! ## What is asserted
//!
//! 1. The fraction does not influence the depth. Two moves with the same
//!    measured mm and wildly different fractions get the same feed.
//! 2. The depth does influence the feed. A deeper measured cut yields a
//!    lower power-bound feed, so arm 1 cannot pass by the depth being
//!    ignored altogether.
//! 3. The realistic fraction (0.08) does not re-create the old 12.5x error:
//!    the feed matches the one computed at the true 2 mm depth, not the one
//!    at 0.16 mm.
//!
//! See `planning/TECH_DEBT_REGISTER.md` T-11.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dressup::feed_modulation::{
    ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement, PowerLimitInputs,
    adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const RPM: f64 = 18_000.0;
const FLUTES: u32 = 2;
/// A 25 mm flute against a 2 mm pass — the ratio that made the old defect
/// worth 12.5x. Never 1.0 anywhere in this file.
const FLUTE_LENGTH_MM: f64 = 25.0;

fn kinematics() -> MachineKinematics {
    MachineKinematics::default()
}

fn ctx<'a>(k: &'a MachineKinematics, available_kw: f64) -> ModulationContext<'a> {
    ModulationContext {
        spindle_rpm: RPM,
        flute_count: FLUTES,
        max_feed_mm_min: 10_000.0,
        rapid_feed_mm_min: 10_000.0,
        chipload_band: Some(ChiploadBand::new(0.01, 0.30).unwrap()),
        kinematics: k,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        // Ruling B6: the power cap reads the material's force line. The
        // hardwood line (Ks 51.9 N/mm², F_edge 4.08 N/mm) puts the edge
        // term at about 11.5 W per mm of depth on this cut, so at 50 W a
        // 1, 2 and 4 mm cut all stay power-bound or feed-capped, and the
        // feed falls as the depth rises.
        power_inputs: Some(PowerLimitInputs {
            line: Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            }
            .force_line()
            .expect("generic hardwood has a force line"),
            engagement_diameter_mm: 6.0,
            available_kw,
        }),
        // Deliberately DIFFERENT from every per-move depth this file uses.
        // Two things hang on that:
        //   * the pre-T-11 expression was `nominal x fraction`, so a
        //     non-zero nominal makes that expression fraction-dependent and
        //     the first test below fails on it. A zero nominal would make
        //     the old expression return 0.0 for every fraction and the test
        //     would pass on the defect it exists to catch.
        //   * if the solver ever falls back to this instead of reading the
        //     per-move millimetres, every depth collapses to the same 7 mm
        //     and the second test fails.
        nominal_axial_doc_mm: 7.0,
        plunge_rate_mm_min: 300.0,
    }
}

fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 0..n_cuts {
        let x = (i + 1) as f64 * 50.0;
        tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), feed_mm_min, MoveIntent::ClearingCut);
    }
    tp
}

/// Run the modulator over `n` identical moves and return the first feed.
fn feed_at(depth_mm: f64, fraction: f64, available_kw: f64) -> f64 {
    let k = kinematics();
    let c = ctx(&k, available_kw);
    let mut tp = straight_toolpath(3, 2_000.0);
    let engagements: Vec<PerMoveEngagement> = (0..tp.moves.len())
        .map(|_| PerMoveEngagement {
            radial_woc_fraction: 0.5,
            axial_doc_fraction: fraction,
            axial_doc_mm: depth_mm,
        })
        .collect();
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &c).unwrap();
    let (feed, _) = *outcome
        .per_move
        .values()
        .next()
        .expect("the modulator touched no move");
    feed
}

#[test]
fn the_flute_fraction_does_not_move_the_depth_g_axialunits() {
    // A power budget tight enough that the depth actually binds, so the
    // feed is a readable function of it.
    let kw = 0.05;

    // Same measured depth, three very different flute fractions. On a long
    // flute the fraction is small; on a stubby one it is large. The depth
    // the tool cuts is identical in all three.
    let a = feed_at(2.0, 2.0 / FLUTE_LENGTH_MM, kw); // 0.08 — realistic
    let b = feed_at(2.0, 0.40, kw);
    let c = feed_at(2.0, 0.95, kw);

    assert_eq!(
        a.to_bits(),
        b.to_bits(),
        "the flute fraction moved the feed: {a} at fraction 0.08 vs {b} at 0.40. \
         The depth must come from the millimetre measurement — T-11."
    );
    assert_eq!(
        a.to_bits(),
        c.to_bits(),
        "the flute fraction moved the feed: {a} at fraction 0.08 vs {c} at 0.95. \
         The depth must come from the millimetre measurement — T-11."
    );
}

#[test]
fn a_deeper_cut_lowers_the_power_bound_feed_g_axialunits() {
    // Non-vacuity partner for the test above. If the solver ignored the
    // depth entirely, that test would pass and this one would not.
    let kw = 0.05;
    let fraction = 0.40;

    let shallow = feed_at(1.0, fraction, kw);
    let deep = feed_at(4.0, fraction, kw);

    assert!(
        deep < shallow,
        "a 4 mm cut got {deep} mm/min and a 1 mm cut got {shallow} mm/min — \
         the depth is not reaching the power cap at all"
    );
}

#[test]
fn the_realistic_fraction_does_not_rebuild_the_old_error_g_axialunits() {
    // The regression, stated directly. Pre-T-11, a 2 mm pass on a 25 mm
    // flute was evaluated at `2.0 x 0.08 = 0.16` mm. This asserts the
    // solver now agrees with the 2 mm case and NOT with the 0.16 mm one.
    let kw = 0.05;
    let realistic_fraction = 2.0 / FLUTE_LENGTH_MM;

    let shipped = feed_at(2.0, realistic_fraction, kw);
    let what_the_old_code_saw = feed_at(2.0 * realistic_fraction, realistic_fraction, kw);

    assert!(
        shipped < what_the_old_code_saw,
        "a 2 mm cut got {shipped} mm/min and the old 0.16 mm reading got \
         {what_the_old_code_saw} mm/min. The pre-T-11 expression let the \
         modulator feed as though the cut were 12.5x shallower than it is."
    );
}
