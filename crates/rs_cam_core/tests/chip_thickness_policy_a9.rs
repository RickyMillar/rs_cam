//! TD3 wave A-9 — chip-thickness policy PROBES (research-only).
//!
//! These tests change no behaviour. They exist to pin, through public
//! API and against the **shipped** vendor LUT, the measured facts that
//! `planning/review_2026-08-08/CHIP_THICKNESS_POLICY.md` reports for
//! ledger rows **F-BIPOLAR**, **F-VALID** and the axial-DOC floor half
//! of the operator review's step 6.
//!
//! Every assertion below is a *reproduction of the current state*, not
//! a bar. If a future programme fixes any of it, these tests go red and
//! that is the intended signal — read the doc, then re-pin deliberately.
//!
//! What is pinned here:
//!
//! 1. `min_doc_chipload_floor_mm` — the only production consumer that
//!    compares a *predicted* arc-mean chip thickness against a vendor
//!    **advance** band — is `None` for every ball / tapered-ball row in
//!    the shipped LUT, at every stepover any shipped Suggest call site
//!    DEFAULTS to, even when the feed is set to the row's own chipload
//!    MAXIMUM. Its X-VAC shape is `None` ("not measured"), not
//!    `Some(0.0)`, so `safe_band_is_empty()` reports `None` rather than
//!    `Some(false)` and the envelope reads healthy.
//! 2. The bound is dead at the defaults, **not dead code**: it becomes
//!    reachable at a radial WOC of ~0.6275 D, above every shipped
//!    default, on the two shipped softwood ball rows with an unusually
//!    wide band. That boundary is pinned too, so a future fix moves a
//!    measured number rather than an assumption.
//! 3. The floor's private chip model omits the `arc >= PI` branch that
//!    `flat_chip_geometry_for_radius` carries, so at a full slot it
//!    reports ~0 where the canonical model reports `0.6366 * fz`.
//! 4. F-VALID population hole: `BallEndmill::chip_geometry` refuses
//!    past the hemisphere pole, so every sample of a ball cutting
//!    deeper than its own radius carries
//!    `effective_chip_thickness_mm = None` — and the chipload gate's
//!    *vestigial* sample-validity predicate still drops those samples
//!    even though its observation is now purely kinematic.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::cutter_constraints::{AxialBindingConstraint, cutter_axial_constraints};
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, LookupResult, lookup_best};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool::{
    BallEndmill, EngagementError, EngagementMode, MillingCutter, TaperedBallEndmill, ToolDefinition,
};

/// The three radial-WOC fractions every shipped `axial_envelope_for_operation`
/// call site DEFAULTS to on a ball-tipped cutter: `feeds/suggest.rs:1545`
/// the eight-op 3D-finish family 0.15 D, `:1525` ProjectCurve 0.20 D,
/// `:1482` Adaptive3d 0.40 D. Two of the three take the operation's own
/// stepover when it is set (`radial.unwrap_or(...)`), so an operator CAN
/// exceed these — which is exactly what the reachability test measures.
const SHIPPED_WOC_FRACTIONS: &[(f64, &str)] = &[
    (0.15, "3D-finish family default"),
    (0.20, "ProjectCurve"),
    (0.40, "Adaptive3d default"),
];

fn carbide_ball(diameter_mm: f64) -> ToolDefinition {
    ToolDefinition::new(
        Box::new(BallEndmill::new(diameter_mm, 30.0)),
        diameter_mm,
        10.0,
        25.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn carbide_tapered_ball() -> ToolDefinition {
    // Wanaka tool 2 shape: 2 mm tip / 7 deg taper / 6 mm shank.
    ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(2.0, 7.0, 6.0, 30.0)),
        6.0,
        10.0,
        25.0,
        35.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::RadiataPine,
    }
}

/// (label, tool family, diameter, flutes, material family, hardness).
/// Chosen to span the ball / tapered-ball rows the shipped LUT actually
/// carries: 24 distinct (diameter, chipload band) triples across
/// `amana_ball_nose`, `amana_3d_profiling` and `idcwoodcraft_millmage`.
const BALL_QUERIES: &[(&str, ToolFamily, f64, u32, MaterialFamily, f64)] = &[
    (
        "ball 1.0 softwood",
        ToolFamily::BallNose,
        1.0,
        2,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "ball 1.5 softwood",
        ToolFamily::BallNose,
        1.5,
        4,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "ball 3.175 softwood",
        ToolFamily::BallNose,
        3.175,
        2,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "ball 3.175 hardwood",
        ToolFamily::BallNose,
        3.175,
        2,
        MaterialFamily::Hardwood,
        1450.0,
    ),
    (
        "ball 6.0 softwood",
        ToolFamily::BallNose,
        6.0,
        2,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "ball 6.0 hardwood",
        ToolFamily::BallNose,
        6.0,
        2,
        MaterialFamily::Hardwood,
        1450.0,
    ),
    (
        "ball 9.525 softwood",
        ToolFamily::BallNose,
        9.525,
        3,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "ball 12.7 softwood",
        ToolFamily::BallNose,
        12.7,
        3,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "tapered 3.175 softwood",
        ToolFamily::TaperedBallNose,
        3.175,
        2,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "tapered 3.175 hardwood",
        ToolFamily::TaperedBallNose,
        3.175,
        2,
        MaterialFamily::Hardwood,
        1450.0,
    ),
    (
        "tapered 6.0 softwood",
        ToolFamily::TaperedBallNose,
        6.0,
        2,
        MaterialFamily::Softwood,
        600.0,
    ),
    (
        "tapered 6.0 hardwood",
        ToolFamily::TaperedBallNose,
        6.0,
        2,
        MaterialFamily::Hardwood,
        1450.0,
    ),
];

fn matched_rows() -> Vec<(&'static str, f64, LookupResult)> {
    let lut = VendorLut::embedded();
    let mut out = Vec::new();
    for &(label, family, diameter, flutes, material_family, hardness) in BALL_QUERIES {
        for role in [LutPassRole::Finish, LutPassRole::Roughing] {
            let query = LookupQuery {
                tool_family: family,
                tool_subfamily: None,
                diameter_mm: diameter,
                flute_count: flutes,
                material_family,
                hardness_kind: Some(HardnessKind::Janka),
                hardness_value: Some(hardness),
                operation_family: LutOperationFamily::Parallel,
                pass_role: role,
            };
            if let Some(r) = lookup_best(&lut, &query)
                && r.chip_load_min_mm.is_some_and(|v| v > 0.0)
            {
                out.push((label, diameter, r));
                break;
            }
        }
    }
    assert!(
        out.len() >= 10,
        "expected the shipped LUT to answer most ball/tapered queries with a chipload floor; \
         got {} of {}",
        out.len(),
        BALL_QUERIES.len()
    );
    out
}

/// **F-BIPOLAR's sibling, measured.** The axial chipload floor is the
/// only production site that compares a *predicted* arc-mean chip
/// thickness against a vendor advance band (`cutter_constraints.rs:386`,
/// the operator review's `[medium]` finding, location 1 of 3).
///
/// It never produces a value **at any radial WOC a shipped Suggest call
/// site defaults to**. For every ball / tapered-ball row the shipped LUT
/// can match, at 0.15 D / 0.20 D / 0.40 D, with the feed set to the
/// row's OWN chipload MAXIMUM — the most aggressive advance still inside
/// the vendor band — the floor is `None`.
///
/// (The bound is *not* unreachable in principle; see
/// [`the_axial_floor_only_becomes_reachable_above_every_shipped_default`]
/// for where it starts to fire and by how far that is outside the
/// defaults. Measured 2026-08-13 — the first version of this test
/// asserted `None` at that fraction too and was falsified by the run.)
///
/// The mechanism (see the doc §1.3): the bracket's upper end is
/// `stickout * 0.95`, far past the point where a ball's engagement
/// radius saturates at R, so `chip_at(upper)` is evaluated on the
/// plateau where the arc is smallest and the arc-mean chip is at its
/// MINIMUM over the bracket. The guard `chip_hi < cl_min -> None` then
/// fires for every band whose floor sits above that plateau value.
#[test]
fn axial_chipload_floor_is_none_at_every_shipped_default_stepover() {
    let mut checked = 0usize;
    let mut non_none = Vec::new();
    for (label, diameter, row) in matched_rows() {
        let tool = if label.starts_with("tapered") {
            carbide_tapered_ball()
        } else {
            carbide_ball(diameter)
        };
        let material = if label.contains("hardwood") {
            hardwood()
        } else {
            softwood()
        };
        // The most aggressive advance still inside the vendor band.
        let fz = row.chip_load_max_mm.unwrap_or(0.0);
        assert!(fz > 0.0, "{label}: matched row carries no chipload max");
        for &(frac, site) in SHIPPED_WOC_FRACTIONS {
            let woc = (tool.diameter() * frac).max(1.0e-3);
            let env = cutter_axial_constraints(
                &tool,
                &material,
                woc,
                fz,
                Some(&row),
                Some(25.0),
                Some(50.0),
            );
            checked += 1;
            if let Some(v) = env.min_doc_chipload_floor_mm {
                non_none.push(format!(
                    "{label} @ {frac} D ({site}), fz={fz:.5} (= band max), floor={v:.5}"
                ));
            }
            // The X-VAC shape: the envelope reports "no floor was
            // derivable" (`None`), NOT "the band is fine" (`Some(false)`)
            // — and every consumer treats the two the same.
            assert_eq!(
                env.safe_band_is_empty(),
                None,
                "{label} @ {frac} D: expected NO FLOOR (None), not a judged band"
            );
            assert_ne!(
                env.binding_constraint,
                AxialBindingConstraint::SafeBandEmpty,
                "{label} @ {frac} D: SafeBandEmpty is not expected at a shipped default"
            );
        }
    }
    println!("A-9 probe: {checked} (row x shipped-default stepover) combinations exercised");
    assert!(
        checked >= 30,
        "probe population too small to be evidence: {checked}"
    );
    assert!(
        non_none.is_empty(),
        "PRE-EXISTING STATE PINNED: the axial chipload floor is expected to be None on every \
         shipped ball/tapered row at every SHIPPED DEFAULT stepover, even at the band maximum. \
         These produced a value instead (if a fix landed, re-pin deliberately):\n  {}",
        non_none.join("\n  ")
    );
}

/// Where the bound *does* start to fire — and how far outside the
/// shipped defaults that is.
///
/// `chip_at` is unimodal in `ap`, peaking at an engagement arc of
/// ≈ 1.9713 rad with a factor of ≈ 0.4183 × fz. The plateau the guard
/// samples (`ap >= R`, where a ball's engagement radius saturates) is
/// only on the *rising* side of that peak once
/// `radial_woc >= 1.9713 * D / PI ≈ 0.6275 D`. Below that fraction the
/// plateau is the function's minimum and the guard refuses.
///
/// Measured 2026-08-13: at 0.6275 D the floor becomes reachable on the
/// two shipped softwood ball rows whose band is unusually wide
/// (`chipload_min 0.0191` / `chipload_max 0.0508`, a 2.66x band) — and
/// it lands as a **`SafeBandEmpty`**, i.e. the case Suggest surfaces as
/// `AxialEnvelopeSafeBandEmpty`. So the bound is dead at the defaults,
/// not dead code; an operator-set Adaptive3d stepover above ~0.63 D can
/// reach it.
#[test]
fn the_axial_floor_only_becomes_reachable_above_every_shipped_default() {
    // 1.9713 rad / PI, the arc at which the arc-mean chip factor peaks.
    const REACHABILITY_FRACTION: f64 = 0.6275;
    let mut reached = Vec::new();
    for (label, diameter, row) in matched_rows() {
        if label.starts_with("tapered") {
            continue;
        }
        let tool = carbide_ball(diameter);
        let material = if label.contains("hardwood") {
            hardwood()
        } else {
            softwood()
        };
        let fz = row.chip_load_max_mm.unwrap_or(0.0);
        let woc = (tool.diameter() * REACHABILITY_FRACTION).max(1.0e-3);
        let env = cutter_axial_constraints(
            &tool,
            &material,
            woc,
            fz,
            Some(&row),
            Some(25.0),
            Some(50.0),
        );
        if let Some(v) = env.min_doc_chipload_floor_mm {
            reached.push((
                label,
                v,
                env.binding_constraint == AxialBindingConstraint::SafeBandEmpty,
            ));
        }
    }
    for (label, floor, empty) in &reached {
        println!(
            "A-9 probe: reachable @ {REACHABILITY_FRACTION} D — {label} floor={floor:.5} safe_band_empty={empty}"
        );
    }
    assert!(
        !reached.is_empty(),
        "the reachability boundary moved: no shipped ball row produces a floor at \
         {REACHABILITY_FRACTION} D any more. Re-derive the boundary before re-pinning."
    );
    assert!(
        reached.iter().any(|(_, _, empty)| *empty),
        "at least one reachable case is expected to land as SafeBandEmpty (the state Suggest \
         surfaces as AxialEnvelopeSafeBandEmpty); none did: {reached:?}"
    );
}

/// The floor's private `chip_at` closure (`cutter_constraints.rs:409`)
/// says it uses the "same closed-form mean as
/// `flat_chip_geometry_for_radius`". It does not: it omits that
/// function's `arc >= PI` branch (`tool/mod.rs:125-129`), which pins
/// `h_max = fz` at a full slot.
///
/// The canonical model is reachable through the public
/// `MillingCutter::chip_geometry`. At a full slot it reports
/// `0.6366 * fz`; the floor's transcription would report `fz * sin(PI)`
/// — about `1.2e-16 * fz`, i.e. a hard zero. The 0.6366 figure is the
/// same one the A-1 fixture pinned as "0.637 at a full slot".
#[test]
fn the_canonical_chip_model_pins_a_full_slot_where_the_floors_copy_reads_zero() {
    let tool = BallEndmill::new(6.0, 30.0);
    let fz = 0.10;
    let geom = tool
        .chip_geometry(1.0, std::f64::consts::PI, fz, 2, EngagementMode::Slot)
        .expect("ball chip geometry below the pole");
    // Canonical: h_max == fz exactly at arc >= PI.
    assert!(
        (geom.max_chip_thickness_mm - fz).abs() < 1e-12,
        "canonical h_max at a full slot should be fz exactly, got {}",
        geom.max_chip_thickness_mm
    );
    // Canonical arc-mean at a full slot: (2 fz / PI) * (1 - cos(PI/2)).
    let expected_mean =
        (2.0 * fz / std::f64::consts::PI) * (1.0 - (std::f64::consts::FRAC_PI_2).cos());
    assert!(
        (geom.mean_chip_thickness_mm - expected_mean).abs() < 1e-12,
        "canonical arc-mean at a full slot: expected {expected_mean}, got {}",
        geom.mean_chip_thickness_mm
    );
    // The A-1 fixture's pinned "0.637 at a full slot" is exactly 2/pi.
    assert!(
        (geom.mean_chip_thickness_mm / fz - std::f64::consts::FRAC_2_PI).abs() < 1e-12,
        "the pinned 0.637-of-fz full-slot factor moved: {}",
        geom.mean_chip_thickness_mm / fz
    );
    // What the floor's transcription would produce at the same arc.
    let floor_h_max = fz * std::f64::consts::PI.sin().abs();
    assert!(
        floor_h_max < 1e-15,
        "the floor's h_max at a full slot is expected to be a hard zero, got {floor_h_max}"
    );
}

/// **F-VALID, the population hole with the largest reach.**
/// `BallEndmill::chip_geometry` refuses with `Unsupported` at
/// `axial_doc >= radius` (`tool/ball.rs:49-53`). The sim sets
/// `effective_chip_thickness_mm = None` for every such sample, and the
/// chipload gate's sample-validity predicate — which its own comment
/// calls "deliberately unchanged, and now vestigial"
/// (`tool_load/chipload.rs:744-763`) — still `continue`s past them,
/// even though the gate's observation is `effective_feed / (rpm *
/// flutes)` and needs no chip model at all.
///
/// So a ball roughing pass at a DOC deeper than its own radius has an
/// EMPTY gate population for a reason that no longer bears on the
/// quantity being gated.
#[test]
fn ball_chip_geometry_refuses_past_the_pole_which_is_what_empties_the_gate() {
    let tool = BallEndmill::new(6.0, 30.0);
    let r = 3.0;
    // Below the pole: supported.
    assert!(
        tool.chip_geometry(r - 0.01, 1.0, 0.05, 2, EngagementMode::Climb)
            .is_ok(),
        "ball chip geometry should resolve below the hemisphere pole"
    );
    // At and past the pole: Unsupported, at every arc and every feed.
    for ap in [r, r + 0.001, r * 2.0, r * 4.0] {
        for arc in [0.3_f64, 1.0, std::f64::consts::FRAC_PI_2, 3.0] {
            assert!(
                matches!(
                    tool.chip_geometry(ap, arc, 0.05, 2, EngagementMode::Climb),
                    Err(EngagementError::Unsupported { .. })
                ),
                "expected Unsupported at ap={ap} arc={arc} (past the hemisphere pole)"
            );
        }
    }
}
