//! SES-01 — the diagnostics JSON surface is pinned, so a `derive` can
//! replace the hand-written `Serialize` impls without moving a byte.
//!
//! `session/mod.rs` carried four hand-written `impl serde::Serialize`
//! blocks — `ToolpathDiagnostic`, `VerdictEvidence`, `Verdict` and
//! `ProjectDiagnostics`. Every one called `serialize_field` once per
//! declared field, in declaration order, with the exact Rust identifier
//! as the JSON key. No rename, no `skip_serializing_if`, no flattening,
//! no computed field. That is what `#[derive(Serialize)]` writes.
//!
//! The hazard the duplication carried: a field added to one of those
//! structs compiled without a serializer error and was silently dropped
//! from the MCP and CLI wire. The structs are the `null`-means-not-
//! measured contract; a dropped key reads as "the whole struct never
//! carried it".
//!
//! This file is the before-and-after instrument. It ran GREEN against the
//! hand-written impls first, then the impls were deleted and the derives
//! added, and it ran green again UNCHANGED. That is the evidence the swap
//! moved no byte.
//!
//! It also has teeth against the hazard itself: a new field on any of the
//! four structs changes the literal below, so the field must be named
//! here before the test passes.
//!
//! The two enum impls — `VerdictSeverity` and `VerdictKind` — are NOT
//! derives. They write their own snake_case string tags, and `VerdictKind`
//! reads `as_str`, which other surfaces share. They stay hand-written and
//! this file pins their tags too.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_core::finish::unified_finish::MonotoneCellTotals;
use rs_cam_core::session::{
    ProjectDiagnostics, ToolpathDiagnostic, Verdict, VerdictEvidence, VerdictKind, VerdictSeverity,
};

/// Every field carries a DISTINCT value, and every `Option` is `Some`.
///
/// A fixture that leaves an `Option` at `None` cannot tell "the key is
/// absent" from "the key is null", and a fixture that repeats a number
/// cannot tell two keys apart if their order swaps.
fn populated() -> ProjectDiagnostics {
    ProjectDiagnostics {
        total_runtime_s: 11.0,
        air_cut_pct_of_total_runtime: 12.0,
        air_cut_pct_of_cutting_time: 13.0,
        average_engagement: 14.0,
        collision_count: 15,
        collision_checks_failed: 16,
        rapid_collision_count: 17,
        per_toolpath: vec![ToolpathDiagnostic {
            toolpath_id: ToolpathId(21),
            name: "Rough".to_owned(),
            operation_type: "Adaptive".to_owned(),
            op_kind: "adaptive".to_owned(),
            tool_name: "6 mm flat".to_owned(),
            move_count: 22,
            cutting_distance_mm: 23.0,
            rapid_distance_mm: 24.0,
            collision_count: Some(25),
            rapid_collision_count: 26,
            truncated_core_mm2: Some(27.0),
            untouched_material_mm2: Some(28.0),
            reached_uncut_estimate_mm2: Some(29.0),
            unmachined_band_area_mm2: Some(30.0),
            tip_float_points: Some(31),
            max_tip_float_mm: Some(32.0),
            monotone_cells: Some(MonotoneCellTotals {
                regions: 33,
                regions_rotated: 34,
                cells_emitted: 35,
                membership_fallbacks: 36,
                empty_fallbacks: 37,
            }),
        }],
        verdicts: vec![Verdict {
            severity: VerdictSeverity::Important,
            kind: VerdictKind::AirCut,
            headline: "Air cut is high".to_owned(),
            offender_toolpath_ids: vec![ToolpathId(41)],
            fix_hint: "Set a boundary".to_owned(),
            evidence: VerdictEvidence {
                move_index: Some(42),
                z_value: Some(43.0),
                count: Some(44),
            },
        }],
    }
}

/// The JSON the four impls produced before the swap, byte for byte.
const PINNED: &str = concat!(
    r#"{"total_runtime_s":11.0,"#,
    r#""air_cut_pct_of_total_runtime":12.0,"#,
    r#""air_cut_pct_of_cutting_time":13.0,"#,
    r#""average_engagement":14.0,"#,
    r#""collision_count":15,"#,
    r#""collision_checks_failed":16,"#,
    r#""rapid_collision_count":17,"#,
    r#""per_toolpath":[{"toolpath_id":21,"#,
    r#""name":"Rough","#,
    r#""operation_type":"Adaptive","#,
    r#""op_kind":"adaptive","#,
    r#""tool_name":"6 mm flat","#,
    r#""move_count":22,"#,
    r#""cutting_distance_mm":23.0,"#,
    r#""rapid_distance_mm":24.0,"#,
    r#""collision_count":25,"#,
    r#""rapid_collision_count":26,"#,
    r#""truncated_core_mm2":27.0,"#,
    r#""untouched_material_mm2":28.0,"#,
    r#""reached_uncut_estimate_mm2":29.0,"#,
    r#""unmachined_band_area_mm2":30.0,"#,
    r#""tip_float_points":31,"#,
    r#""max_tip_float_mm":32.0,"#,
    r#""monotone_cells":{"regions":33,"regions_rotated":34,"cells_emitted":35,"#,
    r#""membership_fallbacks":36,"empty_fallbacks":37}}],"#,
    r#""verdicts":[{"severity":"important","#,
    r#""kind":"air_cut","#,
    r#""headline":"Air cut is high","#,
    r#""offender_toolpath_ids":[41],"#,
    r#""fix_hint":"Set a boundary","#,
    r#""evidence":{"move_index":42,"z_value":43.0,"count":44}}]}"#,
);

/// CONTRACT. The swap from four hand-written impls to four derives moved
/// no key, no value and no order.
#[test]
fn the_diagnostics_json_surface_does_not_move() {
    let json = serde_json::to_string(&populated()).expect("the diagnostics serialise");
    assert_eq!(
        json, PINNED,
        "SES-01: the diagnostics JSON is the MCP and CLI wire. A key that \
         moved, was renamed or went missing is a break, not a refactor."
    );
}

/// CONTRACT. A `None` writes `null`, and the key stays.
///
/// The structs document `null` as "not measured", which is a different
/// statement from 0. A `skip_serializing_if` — which a careless derive
/// could add — would drop the key and destroy that distinction.
#[test]
fn an_unmeasured_field_writes_null_and_keeps_its_key() {
    let mut diagnostics = populated();
    let tp = &mut diagnostics.per_toolpath[0];
    tp.collision_count = None;
    tp.truncated_core_mm2 = None;
    tp.untouched_material_mm2 = None;
    tp.reached_uncut_estimate_mm2 = None;
    tp.unmachined_band_area_mm2 = None;
    tp.tip_float_points = None;
    tp.max_tip_float_mm = None;
    tp.monotone_cells = None;
    diagnostics.verdicts[0].evidence = VerdictEvidence::default();

    let json = serde_json::to_string(&diagnostics).expect("the diagnostics serialise");
    for key in [
        "\"collision_count\":null",
        "\"truncated_core_mm2\":null",
        "\"untouched_material_mm2\":null",
        "\"reached_uncut_estimate_mm2\":null",
        "\"unmachined_band_area_mm2\":null",
        "\"tip_float_points\":null",
        "\"max_tip_float_mm\":null",
        "\"monotone_cells\":null",
        "\"move_index\":null",
        "\"z_value\":null",
        "\"count\":null",
    ] {
        assert!(
            json.contains(key),
            "SES-01: {key} must survive as an explicit null. I read {json}"
        );
    }
}

/// CONTRACT. The two enums keep their hand-written string tags.
#[test]
fn the_verdict_enums_write_their_own_string_tags() {
    let severities = [
        (VerdictSeverity::Critical, "\"critical\""),
        (VerdictSeverity::Important, "\"important\""),
        (VerdictSeverity::Polish, "\"polish\""),
    ];
    for (value, tag) in severities {
        let json = serde_json::to_string(&value).expect("a severity serialises");
        assert_eq!(json, tag, "SES-01: the severity tag is the wire value");
    }

    let kinds = [
        VerdictKind::HolderCollision,
        VerdictKind::HolderCheckFailed,
        VerdictKind::RapidCollision,
        VerdictKind::PlungeStress,
        VerdictKind::AirCut,
        VerdictKind::GeneratedEmpty,
        VerdictKind::AlignmentPinsUnkeyed,
        VerdictKind::MeasurabilityAbstained,
    ];
    for kind in kinds {
        let json = serde_json::to_string(&kind).expect("a kind serialises");
        assert_eq!(
            json,
            format!("\"{}\"", kind.as_str()),
            "SES-01: the kind tag reads `as_str`, which other surfaces share"
        );
    }
}
