//! **P1 — the rubbing floor consults the band the gate will judge against.**
//!
//! Ruling R4 WP2a (2026-09-23): the floor now only warns
//! (`FeedsWarning::ChiploadBelowRubbingFloor`); it does not lift a feed. The
//! band rule below still decides when the warning fires. The probe record
//! below uses the retired name `ChiploadClampedToFloor`.
//!
//! `RUBBING_FLOOR_MM_TOOTH` (0.025 mm/tooth) is subordinated to the matched
//! vendor band by `effective_rubbing_floor`. Until ruling R4 Q9 (2026-09-24)
//! it took the band ceiling; it now takes the band minimum, or the ceiling
//! of a band with no minimum. That rule was already right; what it was
//! handed was not.
//!
//! `chipload_bounds` comes from the **recipe** resolver
//! (`find_best_row_for_geometry`), which lets RPM-only anchors compete and
//! win. When one wins, the recipe legitimately keeps that row's RPM — but
//! `chipload_bounds` is `None`, so the floor saw no band and applied the bare
//! constant, while the post-simulation gate went on to resolve a
//! *chipload-bearing* row through the **envelope** resolver and judge the
//! same cut against it. Two resolvers, one cut, and the floor was consulting
//! the one that had nothing to say.
//!
//! P1 fixes the input, not the rule: no constant moves, no exponent is
//! introduced, and `chipload_bounds` stays `None` because the recipe really
//! does rest on an RPM anchor. Only the floor's band changes, and
//! `FeedsWarning::VendorRowPublishesNoChipload::floor_band_from` names the
//! row it came from.
//!
//! # What P1 turned out NOT to be — measured 2026-08-22, read this first
//!
//! The proposal in `rubbing_floor_diameter_scaling_measurement.rs` said the
//! live Ø1.0 tapered-ball case was "**plausibly**" this asymmetry, naming
//! `whiteside-sc64-conical-ball-nose-…` as the suspected RPM-only winner. It
//! hedged, and the hedge was right to be there. Probed directly:
//!
//! ```text
//! Ø1.0 tapered ball (tip r0.5, 7°), 2F, white oak, ap 0.3
//!   parallel/finish  recipe row = amana-tapered-hardwood-parallel-3175-2f
//!                    envelope row = amana-tapered-hardwood-parallel-3175-2f   <- SAME ROW
//!                    bounds = 0.00484 .. 0.00968   floor applied = 0.00968
//!                    ChiploadClampedToFloor { requested 0.00581, floor 0.00968,
//!                                             band_capped_from: Some(0.025) }
//!   scallop/finish   same shape, band 0.00581 .. 0.01161, floor 0.01161
//!   contour/finish   recipe row = None, envelope row = None, bounds = None
//!                    ChiploadClampedToFloor { requested 0.01205, floor 0.025,
//!                                             band_capped_from: None }
//! ```
//!
//! So on the motivating cut the two resolvers **agree**, the band was already
//! in hand, and the subordination rule was already applying it correctly —
//! 0.00968, not 0.025. P1's premise does not hold there.
//!
//! The reported live symptom (a ~0.012 request raised to a flat 0.025) matches
//! the **third** row above: a cut with *no vendor row at all*, where there is
//! no envelope band to subordinate to either. P1 cannot reach that case, and
//! neither can any resolver fix — only a floor that carries a diameter would
//! (P2 in the measurement file, deliberately **not** adopted).
//!
//! `band_capped_from` is what tells the two shapes apart on a live surface:
//! `Some(0.025)` means a band was found and beat the constant; `None` means
//! the bare constant applied because nothing was found.
//!
//! **What P1 is worth, then, stated without inflation.** It aligns two
//! resolvers that had no business disagreeing, it introduces no number, and on
//! the LUT as shipped it changes **no recipe** — `the_fallback_lowers_the_floor_only_to_a_published_bound`
//! measures exactly that and will say so if it ever stops being true. It is
//! kept because a floor consulting the resolver that cannot see bands is wrong
//! whether or not it currently costs anything.
//!
//! # Non-vacuity
//!
//! This repo has four recorded cases of a bar reading healthy over an empty
//! population, so nothing here asserts a property over a set it has not first
//! proven non-empty. The sweep below *discovers* the affected inputs from
//! shipped data rather than hard-coding a row id that a LUT edit could
//! silently retire, and fails naming the axis to widen if it finds none.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::vendor_lookup;
use rs_cam_core::feeds::vendor_lut::VendorLut;
use rs_cam_core::feeds::{
    ChiploadBounds, FeedsInput, FeedsWarning, OperationFamily, PassRole, RUBBING_FLOOR_MM_TOOTH,
    SetupContext, SpindleStrategy, ToolGeometryHint, calculate, effective_rubbing_floor,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The sweep. Wide on purpose.
///
/// A-6's census measured the recipe-vs-envelope disagreement at **141 of
/// 18 144** queries — under 0.8 %, concentrated in a handful of cells. A
/// hand-picked fixture list is therefore the wrong instrument twice over: it
/// is likely to miss the cells entirely (the first draft of this file swept 20
/// combinations and hit none, which is how this comment exists), and a row id
/// written into a test is a row id a LUT edit can silently retire. So the
/// sweep is a cross-product and the tests *discover* the population.
const DIAMETERS: &[f64] = &[0.794, 1.0, 1.5, 2.0, 3.0, 3.175, 6.0, 12.0];

const FLUTES: &[u32] = &[2, 3];

const OPERATIONS: &[OperationFamily] = &[
    OperationFamily::Adaptive,
    OperationFamily::Pocket,
    OperationFamily::Contour,
    OperationFamily::Parallel,
    OperationFamily::Scallop,
    OperationFamily::Trace,
    OperationFamily::Face,
];

const PASS_ROLES: &[PassRole] = &[PassRole::Roughing, PassRole::Finish];

const SPECIES: &[WoodSpecies] = &[
    WoodSpecies::GenericSoftwood,
    WoodSpecies::RadiataPine,
    WoodSpecies::HardMaple,
    WoodSpecies::WhiteOak,
    WoodSpecies::Walnut,
    WoodSpecies::Ipe,
];

/// Cutter geometries, sized against the swept diameter so a Ø0.794 tapered
/// ball is not asked to carry a 0.5 mm tip radius it does not have.
fn geometries(d: f64) -> Vec<(&'static str, ToolGeometryHint)> {
    vec![
        ("flat", ToolGeometryHint::Flat),
        ("ball", ToolGeometryHint::Ball),
        (
            "bull",
            ToolGeometryHint::Bull {
                corner_radius: (d * 0.15).min(1.0),
            },
        ),
        (
            "tapered_ball",
            ToolGeometryHint::TaperedBall {
                tip_radius: d * 0.5,
                taper_angle_deg: 7.0,
            },
        ),
        (
            "vbit60",
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.2,
            },
        ),
    ]
}

/// The warning we are looking for, plus what the floor did with it.
struct Observed {
    label: String,
    recipe_row: String,
    floor_band_from: Option<String>,
    bounds: Option<ChiploadBounds>,
}

fn observe(lut: &VendorLut) -> Vec<Observed> {
    let machine = MachineProfile::shapeoko_vfd();
    let mut out = Vec::new();
    for &d in DIAMETERS {
        for (geom_label, geometry) in geometries(d) {
            for &flutes in FLUTES {
                for &operation in OPERATIONS {
                    for &pass_role in PASS_ROLES {
                        for species in SPECIES {
                            let material = Material::SolidWood { species: *species };
                            let axial = if pass_role == PassRole::Roughing {
                                d
                            } else {
                                (d * 0.3).min(1.0)
                            };
                            let input = FeedsInput {
                                tool_diameter: d,
                                flute_count: flutes,
                                flute_length: d * 4.0,
                                tool_geometry: geometry,
                                shank_diameter: Some(6.0),
                                material: &material,
                                machine: &machine,
                                operation,
                                operation_kind: None,
                                pass_role,
                                axial_depth_mm: Some(axial),
                                radial_width_mm: None,
                                target_scallop_mm: None,
                                vendor_lut: Some(lut),
                                setup: SetupContext::default(),
                                spindle_strategy: SpindleStrategy::default(),
                            };
                            let result = calculate(&input);
                            for w in &result.warnings {
                                if let FeedsWarning::VendorRowPublishesNoChipload {
                                    observation_id,
                                    floor_band_from,
                                    ..
                                } = w
                                {
                                    out.push(Observed {
                                        label: format!(
                                            "D{d} {geom_label} {flutes}F {operation:?}/{pass_role:?} in {species:?}"
                                        ),
                                        recipe_row: observation_id.clone(),
                                        floor_band_from: floor_band_from.clone(),
                                        bounds: result.chipload_bounds,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// The population this file's claims are made over must exist. If the LUT is
/// edited so that no recipe row is ever an RPM-only anchor for these cuts,
/// this fails rather than quietly passing every test below.
#[test]
fn the_rpm_only_recipe_row_population_is_not_empty() {
    let lut = VendorLut::embedded();
    let hits = observe(&lut);
    assert!(
        !hits.is_empty(),
        "no swept cut resolved an RPM-only recipe row, so every claim in this \
         file would be vacuous. Widen the sweep (DIAMETERS, geometries, \
         OPERATIONS, PASS_ROLES, SPECIES) until at least one fires — do not \
         delete the assertion."
    );
}

/// The core of P1: where the recipe row publishes no chipload but a
/// chipload-bearing row exists, the floor is the envelope band's ceiling, not
/// the bare constant.
#[test]
fn floor_falls_back_to_the_envelope_band_not_the_bare_constant() {
    let lut = VendorLut::embedded();
    let hits = observe(&lut);

    let with_band: Vec<&Observed> = hits
        .iter()
        .filter(|h| h.floor_band_from.is_some())
        .collect();

    assert!(
        !with_band.is_empty(),
        "{} RPM-only recipe rows fired but not one found an envelope band, so \
         P1 changed nothing on this LUT. Either the envelope resolver regressed \
         or the sweep no longer covers a cut where both resolvers disagree. \
         First few that fired without a band: {:?}",
        hits.len(),
        hits.iter().take(5).map(|h| &h.label).collect::<Vec<_>>()
    );

    for h in &with_band {
        // The recipe genuinely carries no band — that is the precondition,
        // and asserting it here is what stops this test from passing for the
        // wrong reason if `chipload_bounds` ever starts being populated.
        assert!(
            h.bounds.is_none(),
            "{}: precondition broken — the recipe row published a band after \
             all, so this is not the P1 case",
            h.label
        );
        assert_ne!(
            h.floor_band_from.as_deref(),
            Some(h.recipe_row.as_str()),
            "{}: the floor band must come from a DIFFERENT row than the \
             RPM-only recipe row",
            h.label
        );
    }
}

/// The shipped Amana ZrN tapered-ball row, alone in a LUT.
///
/// RE-DERIVED 2026-09-23. The R5 printed Onsrud 77-100 rows
/// (3885c8bf..bab9a236) now win the envelope query for a Ø1 tapered ball in
/// hard maple, and their scaled band maximum is 0.0546 mm/tooth, above the
/// floor. The premise of this test (a tapered-ball band wholly under 0.025)
/// is still a real shipped row, so the fixture isolates it: row
/// `amana-tapered-hardwood-parallel-3175-2f`, 0.010-0.020 mm/tooth at
/// Ø3.175 and Janka 1450. Scaled to Ø1 by `D^0.61` it is about
/// 0.020 x (1 / 3.175)^0.61 = 0.020 x 0.494 = 0.0099 mm/tooth at the top,
/// under the 0.025 floor.
///
/// Extrapolation P1 (2026-09-24): on the embedded LUT the printed Amana ZrN
/// v8 1.0 mm tip row now wins this query. The fixture keeps its one-row LUT,
/// so the premise stands. Ruling A1 keys the lookup at the 1.0 mm tip (it was
/// the 0.92 mm engaged ball chord at the 0.3 mm depth), so the "Ø1" in the
/// scale above is now the exact key.
fn tapered_ball_sub_floor_lut() -> VendorLut {
    let row = VendorLut::embedded()
        .observations
        .iter()
        .find(|o| o.observation_id == "amana-tapered-hardwood-parallel-3175-2f")
        .expect("the shipped Amana ZrN tapered-ball parallel row exists")
        .clone();
    VendorLut {
        observations: vec![row],
    }
}

/// The tapered-ball case the operator hit, asserted on the numbers rather
/// than on a row id: a tapered-ball band entirely below the global floor
/// must bring the floor below the constant, and the recipe must warn (ruling
/// R4 WP2a: warn, never lift).
///
/// Ruling R4 Q9 (2026-09-24): the floor is the band MINIMUM. The row prints
/// 0.010–0.020, so the midpoint target is 1.5 x the minimum and the recipe
/// ships inside the band at the Shapeoko's own ceiling. The machine here has
/// a 20 mm/min cutting ceiling, which takes the advance under the minimum:
/// 20 / (2 x 6000 rpm) = 0.0017 mm/tooth at the lowest Shapeoko RPM, and
/// the scaled minimum is about 0.010 x 0.494 = 0.0049.
#[test]
fn tapered_ball_floor_lands_below_the_global_constant() {
    let lut = tapered_ball_sub_floor_lut();
    let mut machine = MachineProfile::shapeoko_vfd();
    machine.max_cutting_feed_mm_min = Some(20.0);
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let input = FeedsInput {
        tool_diameter: 1.0,
        flute_count: 2,
        flute_length: 4.0,
        tool_geometry: ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 7.0,
        },
        shank_diameter: Some(6.0),
        material: &material,
        machine: &machine,
        operation: OperationFamily::Parallel,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(0.3),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::default(),
    };

    let query = rs_cam_core::feeds::vendor_normalize::to_lookup_query(&input)
        .expect("a tapered-ball parallel finish in hard maple must route to a LUT query");
    let envelope = vendor_lookup::find_best_chip_envelope_row(&lut, &query, &input.tool_geometry)
        .expect(
            "the envelope resolver must find a chipload-bearing tapered-ball row — \
             the LUT ships 8 of them",
        );
    let band_max = envelope
        .chip_load_max_mm
        .expect("the envelope resolver only returns chipload-bearing rows");

    assert!(
        band_max < RUBBING_FLOOR_MM_TOOTH,
        "this test's premise is that the tapered-ball band sits UNDER the global \
         floor; got band max {band_max} against floor {RUBBING_FLOOR_MM_TOOTH}. \
         If the LUT changed, re-derive the premise rather than relaxing the bar."
    );

    // And the subordination rule, applied to that band, is what the recipe
    // now uses — the same function, so this cannot drift from the caller.
    let floor = effective_rubbing_floor(Some(ChiploadBounds {
        min_mm_per_tooth: envelope.chip_load_min_mm.unwrap_or(band_max),
        max_mm_per_tooth: band_max,
    }));
    assert!(
        floor < RUBBING_FLOOR_MM_TOOTH,
        "subordinating to a sub-floor band must lower the floor, got {floor}"
    );

    let result = calculate(&input);
    let named = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::VendorRowPublishesNoChipload {
            floor_band_from, ..
        } => floor_band_from.clone(),
        _ => None,
    });
    // The recipe resolver may legitimately match a chipload-bearing row here,
    // in which case there is no warning and no fallback — that is the healthy
    // path, and the floor already sees a band. Only the RPM-only case is
    // P1's, so assert conditionally rather than forcing a shape onto the LUT.
    if let Some(row) = named {
        assert!(
            !row.is_empty(),
            "the fallback row id must be non-empty when present"
        );
    }

    // The warning fires on the subordinated floor: the band minimum, not the
    // constant. The 20 mm/min ceiling puts the advance under it.
    let warned = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            source,
            ..
        } => Some((*commanded, *floor, *source)),
        _ => None,
    });
    let (commanded, warned_floor, source) = warned.unwrap_or_else(|| {
        panic!(
            "a recipe on a sub-floor band must warn; warnings: {:?}",
            result.warnings
        )
    });
    assert!(
        warned_floor < RUBBING_FLOOR_MM_TOOTH && commanded < warned_floor,
        "the warning must read the band minimum as its floor: commanded \
         {commanded:.6}, floor {warned_floor:.6}"
    );
    assert_eq!(
        source,
        rs_cam_core::feeds::RubbingFloorSource::BandMinimum,
        "the row publishes a minimum, so the minimum sets the floor (ruling R4 Q9)"
    );
}

/// **What P1 does NOT do, measured and pinned as a tripwire.**
///
/// On the LUT as shipped 2026-08-22, the envelope fallback is reached (the
/// test above proves it) but **never lowers the floor**. The cells where the
/// two resolvers disagree are the Ø6-and-up flat/bull ones A-6 measured, whose
/// envelope bands sit comfortably *above* 0.025, so `min(0.025, band_max)`
/// returned the constant unchanged. Ruling R4 Q9 (2026-09-24) moved the rule
/// to `min(0.025, band_min)`: a band whose minimum is under 0.025 now lowers
/// the floor. A cell whose envelope band spans 0.025 now reports a floor
/// under the constant when its advance is under the band minimum, so this
/// tripwire can fill for that reason alone. If it fills, re-measure the
/// named cells before a re-bless. The tapered-ball rows that publish sub-floor
/// bands are cells where both resolvers agree, so the floor already had the
/// band before P1.
///
/// So P1 removes a latent asymmetry and moves no number today. Saying that
/// plainly is worth more than a green test implying otherwise — and this is
/// written as a tripwire rather than a comment so that the day a LUT edit makes
/// the fallback bite, something says so instead of the change landing silently.
#[test]
fn the_fallback_lowers_the_floor_only_to_a_published_bound() {
    let lut = VendorLut::embedded();
    let machine = MachineProfile::shapeoko_vfd();
    let mut lowered = Vec::new();
    let mut unexplained: Vec<String> = Vec::new();

    for &d in DIAMETERS {
        for (geom_label, geometry) in geometries(d) {
            for &flutes in FLUTES {
                for &operation in OPERATIONS {
                    for &pass_role in PASS_ROLES {
                        for species in SPECIES {
                            let material = Material::SolidWood { species: *species };
                            let axial = if pass_role == PassRole::Roughing {
                                d
                            } else {
                                (d * 0.3).min(1.0)
                            };
                            let input = FeedsInput {
                                tool_diameter: d,
                                flute_count: flutes,
                                flute_length: d * 4.0,
                                tool_geometry: geometry,
                                shank_diameter: Some(6.0),
                                material: &material,
                                machine: &machine,
                                operation,
                                operation_kind: None,
                                pass_role,
                                axial_depth_mm: Some(axial),
                                radial_width_mm: None,
                                target_scallop_mm: None,
                                vendor_lut: Some(&lut),
                                setup: SetupContext::default(),
                                spindle_strategy: SpindleStrategy::default(),
                            };
                            let result = calculate(&input);
                            // Only the P1 shape: a recipe carrying no band of
                            // its own, whose floor nevertheless came out under
                            // the global constant.
                            if result.chipload_bounds.is_some() {
                                continue;
                            }
                            for w in &result.warnings {
                                // R4 WP2a (2026-09-23): the warning no longer
                                // lifts; a floor under the constant still means
                                // a band set it.
                                if let FeedsWarning::ChiploadBelowRubbingFloor {
                                    floor,
                                    source,
                                    band_min,
                                    band_max,
                                    ..
                                } = w
                                    && *floor < RUBBING_FLOOR_MM_TOOTH
                                    && band_max.is_some()
                                {
                                    lowered.push(format!(
                                        "D{d} {geom_label} {flutes}F \
                                         {operation:?}/{pass_role:?} in {species:?}: \
                                         floor {floor:.5}"
                                    ));
                                    // Ruling R4 Q9: a lowered floor is always a
                                    // PUBLISHED bound, named by its source. Any
                                    // other value would be an invisible rule.
                                    use rs_cam_core::feeds::RubbingFloorSource as Src;
                                    let bound = match source {
                                        Src::BandMinimum => *band_min,
                                        Src::BandMaximum => *band_max,
                                        Src::RepoConstant => None,
                                    };
                                    if bound.is_none_or(|b| (b - floor).abs() > 1e-9) {
                                        unexplained.push(format!(
                                            "D{d} {geom_label} {flutes}F \
                                             {operation:?}/{pass_role:?} in {species:?}: \
                                             floor {floor:.5} from {source:?}, band \
                                             {band_min:?}..{band_max:?}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Measured 2026-09-24 under ruling R4 Q9 on the LUT as shipped: the
    // fallback lowers the floor on 36 bandless recipes (the small V-bit
    // Trace cells whose envelope band minimum sits under 0.025). Before Q9
    // the count was zero, because `min(0.025, band max)` never went under
    // the constant for those bands. The tripwire now pins two things: the
    // lowering still happens (Q9 is live on this path), and every lowered
    // floor is the published bound its source names.
    assert!(
        !lowered.is_empty(),
        "ruling R4 Q9 must lower the floor on the bandless V-bit Trace cells whose \
         envelope band minimum is under 0.025; it lowered none, so the fallback no \
         longer reaches the band minimum"
    );
    assert!(
        unexplained.is_empty(),
        "{} lowered floors are not the published bound their source names (an \
         invisible rule): {:?}",
        unexplained.len(),
        unexplained.iter().take(5).collect::<Vec<_>>()
    );
}

/// The constant this whole file argues about has not moved. P1 is a change of
/// input, not of threshold, and this is the pin that says so.
#[test]
fn p1_moved_no_constant() {
    assert!(
        (RUBBING_FLOOR_MM_TOOTH - 0.025).abs() < f64::EPSILON,
        "RUBBING_FLOOR_MM_TOOTH is {RUBBING_FLOOR_MM_TOOTH}, not 0.025 — P1 was \
         explicitly a resolver fix that introduces no new constant and moves no \
         old one. If the constant is being changed, that is P2 and needs its own \
         evidence."
    );
    // And the subordination rule itself is untouched: no band → bare constant.
    assert!((effective_rubbing_floor(None) - RUBBING_FLOOR_MM_TOOTH).abs() < f64::EPSILON);
}
