//! Feeds contract P16 item 8 (operator ruling D2, 2026-11-06): the vendor
//! band. The printed rows that match one tool, size and material form a
//! band. Suggest ships the lowest chip and names every member
//! (`LookupResult::band`, the rules in the `VendorBand` doc).
//!
//! The four claims:
//! - (a) a band of more than one member ships the lowest and names all;
//! - (b) a row of a hobby feed chart never enters the band, and the IDC
//!   Woodcraft row passes every other rule for the probe query;
//! - (c) a grade c row joins only when the band has no grade a or b member;
//! - (d) no chip rises: the shipped chip is at or below the score winner's
//!   chip on every query of a sample grid, and every member has the score
//!   winner's series rank (band rule 8, option B).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::series::series_rank;
use rs_cam_core::feeds::vendor_lookup::{BandRule, LookupQuery, LookupResult, lookup_best};
use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, SourceClass,
    ToolFamily, VendorLut, VendorObservation,
};
use rs_cam_core::material::WoodSpecies;

/// The IDC Woodcraft 1/2 in ball row (`idcwoodcraft_millmage.json`): ball
/// nose, parallel finish, hardwood, 12.7 mm, 2 flutes.
const IDC_BALL_ROW: &str = "idcwoodcraft-bn-12-ball-nose";

fn row(id: &str) -> VendorObservation {
    embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("the embedded LUT has no row {id}"))
        .clone()
}

/// A query at the row's own tool, size, flute count, operation, role and
/// material, at the generic Janka of the material family.
fn query_at(obs: &VendorObservation) -> LookupQuery {
    let janka = match obs.material_family {
        MaterialFamily::Softwood => Some(WoodSpecies::GenericSoftwood.janka_lbf()),
        MaterialFamily::Hardwood => Some(WoodSpecies::GenericHardwood.janka_lbf()),
        MaterialFamily::PlywoodSoftwood
        | MaterialFamily::PlywoodHardwood
        | MaterialFamily::Mdf
        | MaterialFamily::Hdf
        | MaterialFamily::Particleboard
        | MaterialFamily::Acrylic
        | MaterialFamily::Hdpe
        | MaterialFamily::Polycarbonate
        | MaterialFamily::Delrin
        | MaterialFamily::Aluminum
        | MaterialFamily::Fiberglass => None,
    };
    LookupQuery {
        tool_family: obs.tool_family,
        tool_subfamily: None,
        diameter_mm: obs.diameter_mm.expect("the probe row prints a diameter"),
        flute_count: obs.flute_count,
        material_family: obs.material_family,
        hardness_kind: janka.map(|_| HardnessKind::Janka),
        hardness_value: janka,
        operation_family: obs.operation_family,
        pass_role: obs.pass_role,
    }
}

fn member_ids(result: &LookupResult) -> Vec<&str> {
    result.band.members.iter().map(|m| m.row.id()).collect()
}

/// (a) The generic hardwood 6.35 mm 2-flute flat end mill, pocket roughing:
/// the Spektra, Onsrud and Freud charts all print this cell. The band
/// names every member, lowest chip first, and the lowest ships.
#[test]
fn a_band_of_several_members_ships_the_lowest_and_names_all_p16() {
    let lut = embedded_vendor_lut();
    let query = LookupQuery {
        tool_family: ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm: 6.35,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(WoodSpecies::GenericHardwood.janka_lbf()),
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
    };
    let result = lookup_best(lut, &query).expect("the cell is printed");
    let band = &result.band;
    assert!(
        band.members.len() > 1,
        "a band of one: {:?}",
        member_ids(&result)
    );
    let ids = member_ids(&result);
    assert!(
        ids.contains(&"freud-solid-carbide-quarter-hardwood"),
        "the Freud 1/4 in row is a member: {ids:?}"
    );
    for pair in band.members.windows(2) {
        assert!(
            pair[0].chip_mid_mm <= pair[1].chip_mid_mm,
            "the members are not ascending"
        );
    }
    let lowest = band.lowest().expect("members exist");
    assert_eq!(band.rule, BandRule::Lowest);
    assert_eq!(
        result.observation_id,
        lowest.row.id(),
        "the lowest member ships"
    );
    assert_eq!(result.chip_load_mm, lowest.chip_mid_mm);
    // Freud prints 0.2032-0.2794 mm (mid 0.2413) at 1/4 in hardwood: the
    // highest member, so it does not ship.
    assert_eq!(
        band.highest().map(|m| m.row.id()),
        Some("freud-solid-carbide-quarter-hardwood"),
        "Freud is the highest member"
    );
    let text = band
        .card_text()
        .expect("a band of two or more has a card line");
    assert!(text.starts_with("Vendors: "), "{text}");
    assert!(text.ends_with("(ships the lowest)"), "{text}");
}

/// (b) A hobby row never enters the band. Non-vacuity: the same row with
/// the vendor class is a member, so it passes every other rule.
#[test]
fn a_hobby_row_never_enters_the_band_p16() {
    let idc = row(IDC_BALL_ROW);
    assert_eq!(
        idc.source_class,
        SourceClass::HobbyFeedChart,
        "the loader reads the class"
    );
    let query = query_at(&idc);

    // The embedded LUT: the IDC row is a candidate of this query, but no
    // member.
    let result = lookup_best(embedded_vendor_lut(), &query).expect("the cell is printed");
    assert!(
        !member_ids(&result).contains(&IDC_BALL_ROW),
        "the hobby row is a band member: {:?}",
        member_ids(&result)
    );

    // The IDC row alone, with the hobby class: no member.
    let alone = VendorLut {
        observations: vec![idc.clone()],
    };
    let hobby = lookup_best(&alone, &query).expect("the row matches its own query");
    assert!(hobby.band.members.is_empty(), "{:?}", member_ids(&hobby));
    assert_eq!(hobby.band.rule, BandRule::NoMember);

    // The same row with the vendor class: a member.
    let mut vendor = idc;
    vendor.source_class = SourceClass::Vendor;
    let alone = VendorLut {
        observations: vec![vendor],
    };
    let vendor = lookup_best(&alone, &query).expect("the row matches its own query");
    assert_eq!(
        member_ids(&vendor),
        vec![IDC_BALL_ROW],
        "with the vendor class the row passes every band rule"
    );
}

/// (c) A grade c row joins only when the band has no grade a or b member.
/// The probe copies one grade a row: the copy is grade c with a lower
/// chip.
#[test]
fn a_grade_c_row_joins_only_without_a_strong_member_p16() {
    let strong = row("freud-solid-carbide-quarter-hardwood");
    assert_eq!(strong.evidence_grade, EvidenceGrade::A);
    let mut weak = strong.clone();
    weak.observation_id = "zz-probe-grade-c-copy".to_owned();
    weak.evidence_grade = EvidenceGrade::C;
    // Half the printed chip: lower than the strong row, so a member would
    // ship.
    weak.chipload_min_mm_tooth = strong.chipload_min_mm_tooth.map(|v| v / 2.0);
    weak.chipload_max_mm_tooth = strong.chipload_max_mm_tooth.map(|v| v / 2.0);
    let query = query_at(&strong);

    let both = VendorLut {
        observations: vec![strong.clone(), weak.clone()],
    };
    let result = lookup_best(&both, &query).expect("the rows match");
    assert_eq!(
        member_ids(&result),
        vec![strong.observation_id.as_str()],
        "the grade c row joined next to a grade a member"
    );
    assert_eq!(result.observation_id, strong.observation_id);

    let mut weak_too = strong;
    weak_too.evidence_grade = EvidenceGrade::C;
    let weak_only = VendorLut {
        observations: vec![weak_too.clone(), weak.clone()],
    };
    let result = lookup_best(&weak_only, &query).expect("the rows match");
    let mut ids = member_ids(&result);
    ids.sort_unstable();
    let mut want = vec![
        weak.observation_id.as_str(),
        weak_too.observation_id.as_str(),
    ];
    want.sort_unstable();
    assert_eq!(ids, want, "with no strong member the grade c rows join");
}

/// (d) No chip rises: on every query of a sample grid the shipped chip is
/// at or below the score winner's chip, the members are ascending, and a
/// `Lowest` band ships its first member.
#[test]
fn no_shipped_chip_is_above_the_score_winner_p16() {
    let lut = embedded_vendor_lut();
    let tools = [
        ToolFamily::FlatEnd,
        ToolFamily::BallNose,
        ToolFamily::BullNose,
        ToolFamily::TaperedBallNose,
    ];
    // The FM1 diameters (`tests/fixtures/feeds_matrix_2026-09-23`).
    let diameters = [3.175, 6.0, 6.35, 12.7];
    let materials = [
        (
            MaterialFamily::Softwood,
            Some(WoodSpecies::GenericSoftwood.janka_lbf()),
        ),
        (
            MaterialFamily::Hardwood,
            Some(WoodSpecies::GenericHardwood.janka_lbf()),
        ),
        (MaterialFamily::Mdf, None),
        (MaterialFamily::PlywoodHardwood, None),
    ];
    let ops = [
        (LutOperationFamily::Pocket, LutPassRole::Roughing),
        (LutOperationFamily::Adaptive, LutPassRole::Roughing),
        (LutOperationFamily::Contour, LutPassRole::Finish),
        (LutOperationFamily::Parallel, LutPassRole::Finish),
    ];
    let mut checked = 0_usize;
    let mut several = 0_usize;
    for tool in tools {
        for d in diameters {
            for flutes in [1_u32, 2, 3] {
                for (material, janka) in materials {
                    for (op, role) in ops {
                        let query = LookupQuery {
                            tool_family: tool,
                            tool_subfamily: None,
                            diameter_mm: d,
                            flute_count: flutes,
                            material_family: material,
                            hardness_kind: janka.map(|_| HardnessKind::Janka),
                            hardness_value: janka,
                            operation_family: op,
                            pass_role: role,
                        };
                        let Some(result) = lookup_best(lut, &query) else {
                            continue;
                        };
                        let band = &result.band;
                        for pair in band.members.windows(2) {
                            assert!(pair[0].chip_mid_mm <= pair[1].chip_mid_mm, "{query:?}");
                        }
                        if let Some(winner_chip) = band.score_winner_chip_mm {
                            assert!(
                                result.chip_load_mm <= winner_chip * (1.0 + 1e-9),
                                "{query:?}: shipped {} above the score winner {winner_chip}",
                                result.chip_load_mm
                            );
                            checked += 1;
                        }
                        if band.rule == BandRule::Lowest {
                            assert_eq!(
                                Some(result.observation_id.as_str()),
                                band.lowest().map(|m| m.row.id()),
                                "{query:?}"
                            );
                        }
                        // Band rule 8 (option B): every member has the
                        // series rank of the score winner.
                        let winner_rank = series_rank(None, &row(band.score_winner.id()));
                        for m in &band.members {
                            assert_eq!(
                                series_rank(None, &row(m.row.id())),
                                winner_rank,
                                "{query:?}: {} crosses the series rank",
                                m.row.id()
                            );
                        }
                        if band.members.len() > 1 {
                            several += 1;
                        }
                    }
                }
            }
        }
    }
    // Non-vacuity: the grid reaches matched rows and bands of several
    // members.
    assert!(checked > 100, "only {checked} queries had a winner chip");
    assert!(
        several > 10,
        "only {several} bands had more than one member"
    );
}
