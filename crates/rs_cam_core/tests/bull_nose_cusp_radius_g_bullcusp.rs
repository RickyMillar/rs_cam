//! G-BULLCUSP sentry — a bull-nose cutter reports its CORNER radius as its
//! cusp-forming radius, not its envelope radius.
//!
//! The operator ruled on 2026-09-10 that a bull-nose tool is allowed on a
//! Scallop operation (`planning/ui_fix_2026-09-09/STATUS.md`, R0.3 §7 Q3).
//! The generator could not honour that ruling:
//!
//! * `MillingCutter::cusp_radius` special-cased
//!   `ToolGeometryHint::TaperedBall { tip_radius }` and fell through to
//!   `_ => self.radius()` for every other shape.
//! * `BullNoseEndmill` overrides `corner_radius_mm` and `geometry_hint`
//!   (`Bull { corner_radius }`) but NOT `cusp_radius`.
//! * So a Ø10 bull nose with a 2 mm corner reported its cusp-forming radius
//!   as **5.0**, and `scallop.rs` drives every stepover and cusp equation
//!   from `cutter.cusp_radius_mm()`.
//!
//! The consequence is silent and it is the wrong direction. A cusp radius
//! 2.5× too large makes `equal_cusp_stepover_mm` space the passes far wider
//! than the requested scallop height allows, and the operation reports
//! success while it leaves scallops taller than asked for. A permissive
//! registry over a generator that cuts wrong is worse than a registry that
//! refuses.
//!
//! The feeds side already read the corner radius correctly
//! (`feeds::cutter_constraints::max_doc_scallop`,
//! `Bull { corner_radius } => Some(corner_radius)`), and so did the pencil
//! (`pencil::tip_contact_radius` prefers `corner_radius_mm()` where it is
//! nonzero). Only the shared geometry query did not.
//!
//! ## Scope of the arm
//!
//! `ToolGeometryHint::Bull` is produced by exactly one type,
//! `BullNoseEndmill::geometry_hint`, so no other shape changes. A bull
//! declared with a ZERO corner radius is a flat end mill wearing a bull's
//! name; it keeps `radius()`, because a cusp radius of zero would collapse
//! every equation downstream into a divide-by-zero or a zero stepover.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::session::multitool::equal_cusp_stepover_mm;
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill,
};

/// The reference tool of the ruling: Ø10 bull nose, 2 mm corner.
fn bull_10_r2() -> BullNoseEndmill {
    // `BullNoseEndmill::new(diameter, corner_radius, cutting_length)`.
    BullNoseEndmill::new(10.0, 2.0, 40.0)
}

#[test]
fn a_bull_nose_reports_its_corner_radius_as_its_cusp_radius() {
    let bull = bull_10_r2();
    println!(
        "G-BULLCUSP: diameter {:.1}, radius {:.1}, corner {:.1}, cusp {:.3}",
        bull.diameter(),
        bull.radius(),
        bull.corner_radius_mm(),
        bull.cusp_radius()
    );

    // Population: the fixture really is a bull, not a degenerate one.
    assert!(
        bull.corner_radius_mm() > 0.0 && bull.corner_radius_mm() < bull.radius(),
        "population: the fixture must be a genuine bull nose, corner {} radius {}",
        bull.corner_radius_mm(),
        bull.radius()
    );

    assert!(
        (bull.cusp_radius() - 2.0).abs() < 1e-9,
        "cusp radius is {:.3}, expected the 2.0 mm corner. Pre-fix this read \
         {:.3} — the ENVELOPE radius — and every cusp equation downstream \
         spaced its passes off it.",
        bull.cusp_radius(),
        bull.radius()
    );
    // Both spellings are documented aliases and must never disagree.
    assert!(
        (bull.cusp_radius() - bull.cusp_radius_mm()).abs() < 1e-12,
        "cusp_radius and cusp_radius_mm disagree: {} vs {}",
        bull.cusp_radius(),
        bull.cusp_radius_mm()
    );
}

#[test]
fn the_scallop_stepover_follows_the_corner_radius() {
    // The consequence that matters. `scallop.rs` takes
    // `cutter.cusp_radius_mm()` and drives `equal_cusp_stepover_mm` from it.
    let bull = bull_10_r2();
    let cusp_height_mm = 0.05;

    let stepover = equal_cusp_stepover_mm(bull.cusp_radius_mm(), cusp_height_mm);
    let by_corner = equal_cusp_stepover_mm(2.0, cusp_height_mm);
    let by_envelope = equal_cusp_stepover_mm(5.0, cusp_height_mm);

    println!(
        "G-BULLCUSP stepover at h={cusp_height_mm}: tool {stepover:.4} mm, \
         corner-law {by_corner:.4} mm, envelope-law {by_envelope:.4} mm"
    );

    // Population: the two laws must actually differ, or this proves nothing.
    assert!(
        (by_envelope - by_corner).abs() > 1e-3,
        "population: the corner and envelope stepovers are indistinguishable \
         at this scallop height, so the arm cannot discriminate"
    );
    assert!(
        (stepover - by_corner).abs() < 1e-9,
        "the scallop stepover for a bull nose is {stepover:.4} mm; the \
         corner radius asks for {by_corner:.4} mm. Pre-fix it was \
         {by_envelope:.4} mm, which leaves scallops far taller than the \
         requested {cusp_height_mm} mm."
    );
}

#[test]
fn a_zero_corner_bull_keeps_the_envelope_radius() {
    // A bull declared with no corner is a flat end mill. A cusp radius of
    // zero would divide by zero in `equal_cusp_stepover_mm` and collapse
    // every claim floor and region area downstream, so the arm must not
    // take it.
    let flatish = BullNoseEndmill::new(6.0, 0.0, 25.0);
    assert!(
        (flatish.cusp_radius() - flatish.radius()).abs() < 1e-9,
        "a zero-corner bull reports cusp {:.3}, expected its radius {:.3}",
        flatish.cusp_radius(),
        flatish.radius()
    );
    assert!(
        flatish.cusp_radius() > 0.0,
        "a cusp radius of zero collapses every equation that divides by it"
    );
}

#[test]
fn no_other_shape_moved() {
    // Collateral guard. `ToolGeometryHint::Bull` is produced by
    // `BullNoseEndmill` alone, so these must be bit-for-bit what they were.
    let flat = FlatEndmill::new(6.0, 25.0);
    assert!(
        (flat.cusp_radius() - 3.0).abs() < 1e-12,
        "flat end mill cusp moved to {}",
        flat.cusp_radius()
    );

    let ball = BallEndmill::new(6.0, 25.0);
    assert!(
        (ball.cusp_radius() - 3.0).abs() < 1e-12,
        "ball nose cusp moved to {}",
        ball.cusp_radius()
    );

    // `tool_scale_semantics_pr2.rs`'s own fixture: Ø1 tip, 7 deg half-angle,
    // Ø6 shaft. Envelope 3.0 mm, cusp 0.5 mm — a 6x split, so a fall-through
    // to `radius()` could not hide.
    let tapered = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    assert!(
        (tapered.cusp_radius() - 0.5).abs() < 1e-12,
        "tapered ball cusp moved to {} — it must stay the TIP radius",
        tapered.cusp_radius()
    );
    assert!(
        tapered.radius() > tapered.cusp_radius(),
        "population: the tapered fixture must have a shank wider than its \
         tip, or it cannot detect a fall-through to radius()"
    );
}
