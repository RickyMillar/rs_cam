//! Tool-geometry hygiene — a tool's NAME must not contradict its
//! geometry, and a saved project must not keep publishing another
//! type's geometry.
//!
//! # Why this exists
//!
//! Found 2026-08-21 on a live project about to be cut
//! (`planning/airrun_2026-08-19/RUN_LOG.md`, end of "## G-PINAUTO").
//!
//! **Defect 1 — the name lied about the geometry.** Tool id 2 was named
//! "Tapered Ball 2mm tip / 7° / 6mm shank" and carried `diameter: 1.0`.
//! For a tapered ball `diameter` IS the ball/tip diameter — see
//! `TaperedBallEndmill::new(ball_diameter, ..)` — so the tool was R0.5 /
//! Ø1.0, **half its named size**. Every feed decision followed the
//! number; every human decision followed the name. The operator sized a
//! whole finishing strategy off the name and was wrong by 2×, and
//! nothing anywhere compared the two.
//!
//! **Defect 2 — cross-type defaults persisted in saved projects.** That
//! same tapered ball carried `corner_radius_mm: 2.0` and
//! `included_angle: 90.0`, neither meaningful for its type, both from
//! the old type-AGNOSTIC `add_tool` defaults. b0362626 fixed the MCP
//! creation path going forward; every already-saved project still
//! carried the garbage. Not cosmetic: `Session::list_tools` publishes
//! `corner_radius_mm.max(corner_radius)`, so a flat end mill reported a
//! 2 mm corner radius on the wire.
//!
//! The name check is ADVISORY and heuristic, so the load-bearing half of
//! this file is the **false-alarm floor**: the real vendor names from
//! the live project, which are correct and must stay silent.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{
    NamedQuantity, ToolConfig, ToolGeometryField, ToolId, ToolType,
};
use rs_cam_core::tool::MillingCutter;

/// The tapered balls as they appear in `wanaka200.toml` today, after
/// the operator corrected the names: `R<tip radius>mm x <shank>mm x
/// <flute length>mm <flutes>F Tapered Ball`.
fn live_tapered_ball(name: &str, diameter: f64, taper_half_angle: f64, loc: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    tool.name = name.to_owned();
    tool.diameter = diameter;
    tool.cutting_length = loc;
    tool.taper_half_angle = taper_half_angle;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.shank_length = 20.0;
    tool.stickout = 35.0;
    tool
}

/// The false-alarm floor. These are the real, CORRECT vendor strings
/// from the live project — three dimensions and a flute count packed
/// into one phrase, which is exactly the shape a naive parse invents
/// mismatches out of. A check that cries wolf here trains the reader to
/// ignore the one that matters, which is worse than the defect.
#[test]
fn correct_vendor_names_raise_nothing() {
    let live = [
        live_tapered_ball("R0.5mm x 6mm x 20mm 2F Tapered Ball", 1.0, 7.1, 20.0),
        live_tapered_ball("R1.0mm x 6mm x 20mm 2F Tapered Ball", 2.0, 5.7, 20.0),
        live_tapered_ball("R1.5mm x 6mm x 30.5mm 2F Tapered Ball", 3.0, 2.8, 30.5),
    ];
    for tool in &live {
        let found = tool.name_geometry_mismatches();
        assert!(found.is_empty(), "{}: unexpected {found:?}", tool.name);
    }

    // The end mill from the same file, named the ordinary way.
    let mut end_mill = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    end_mill.name = "6mm 2F Carbide End Mill".to_owned();
    end_mill.diameter = 6.0;
    end_mill.shank_diameter = 6.35;
    let found = end_mill.name_geometry_mismatches();
    assert!(found.is_empty(), "unexpected {found:?}");
}

/// THE motivating fixture: the name as it stood before the operator
/// corrected it, against the geometry that was actually cut. One
/// mismatch, naming both numbers, and it is the tip diameter.
#[test]
fn the_two_mm_name_on_the_one_mm_tapered_ball_is_flagged() {
    let name = "Tapered Ball 2mm tip / 7° / 6mm shank";
    let mut tool = live_tapered_ball(name, 1.0, 7.1, 20.0);

    let found = tool.name_geometry_mismatches();
    assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
    let m = &found[0];
    assert_eq!(m.quantity, NamedQuantity::TipDiameter);
    assert_eq!(m.name_value, 2.0);
    assert_eq!(m.config_value, 1.0);
    assert_eq!(m.field, "diameter");

    // Both numbers have to reach the reader: neither alone is the bug.
    let msg = m.message();
    assert!(msg.contains("2.000"), "{msg}");
    assert!(msg.contains("1.000"), "{msg}");

    // Correcting the geometry — the tool the operator thought they had
    // — silences it. The advisory tracks the disagreement, not the name.
    tool.diameter = 2.0;
    let after = tool.name_geometry_mismatches();
    assert!(after.is_empty(), "unexpected {after:?}");
}

/// A saved project written before b0362626 carries every type's
/// geometry on every tool. Loading it must stop republishing the parts
/// that belong to other types — including through the `.max()` in
/// `Session::list_tools`, which is how a flat end mill came to report a
/// 2 mm corner radius on the MCP wire.
#[test]
fn a_pre_b0362626_saved_tool_normalizes_on_load() {
    let saved = r#"
id = 2
name = "R0.5mm x 6mm x 20mm 2F Tapered Ball"
tool_number = 6
tool_type = "tapered_ball_nose"
diameter = 1.0
cutting_length = 20.0
helix_deg = 30.0
corner_radius_mm = 2.0
corner_radius = 2.0
included_angle = 90.0
taper_half_angle = 7.1
shaft_diameter = 6.0
holder_diameter = 25.0
shank_diameter = 6.0
shank_length = 20.0
stickout = 35.0
flute_count = 2
tool_material = "carbide"
cut_direction = "up_cut"
vendor = "endmill.com.au"
product_id = ""
"#;
    let mut tool: ToolConfig = toml::from_str(saved).expect("saved tool deserializes");

    // As loaded: the value a wire consumer would see.
    assert_eq!(tool.corner_radius_mm.max(tool.corner_radius), 2.0);

    let cleared = tool.normalize_geometry();
    let names: Vec<&str> = cleared.iter().map(|c| c.field.field_name()).collect();
    let expected = vec!["corner_radius_mm", "corner_radius", "included_angle"];
    assert_eq!(names, expected);
    assert_eq!(tool.corner_radius_mm.max(tool.corner_radius), 0.0);

    // The geometry that DEFINES a tapered ball is untouched.
    assert_eq!(tool.taper_half_angle, 7.1);
    assert_eq!(tool.shaft_diameter, 6.0);
    assert_eq!(tool.diameter, 1.0);
    assert!(tool.missing_defining_geometry().is_empty());
}

/// The claim normalisation rests on: zeroing another type's geometry
/// changes NO cutter geometry, because every consumer dispatches on
/// `tool_type` before reading these fields. Checked here against the
/// real cutter `build_cutter` produces — the whole silhouette, not just
/// the headline diameter — for every tool type.
#[test]
fn normalizing_changes_no_cutter_geometry() {
    for &tool_type in ToolType::ALL {
        let mut tool = ToolConfig::new_default(ToolId(0), tool_type);
        // Re-plant the pre-b0362626 defaults on every type.
        tool.corner_radius_mm = 0.5;
        tool.corner_radius = 2.0;
        tool.included_angle = 90.0;
        tool.taper_half_angle = 15.0;
        tool.shaft_diameter = 6.35;
        // …except where the value defines the type, which a loader has
        // to leave alone for the tool to be that type at all.
        if tool_type == ToolType::VBit {
            tool.included_angle = 60.0;
        }

        let before = build_cutter(&tool);
        let before_profile = before.profile_points(64);
        let before_envelope = tool.envelope_diameter();

        let cleared = tool.normalize_geometry();
        assert!(!cleared.is_empty(), "{tool_type:?}: nothing to clear");

        let after = build_cutter(&tool);
        assert_eq!(after.diameter(), before.diameter(), "{tool_type:?}");
        assert_eq!(after.radius(), before.radius(), "{tool_type:?}");
        assert_eq!(after.length(), before.length(), "{tool_type:?}");
        let after_cr = after.corner_radius_mm();
        let before_cr = before.corner_radius_mm();
        assert_eq!(after_cr, before_cr, "{tool_type:?}");
        let after_ch = after.center_height();
        let before_ch = before.center_height();
        assert_eq!(after_ch, before_ch, "{tool_type:?}");
        assert_eq!(tool.envelope_diameter(), before_envelope, "{tool_type:?}");

        let after_profile = after.profile_points(64);
        assert_eq!(after_profile.len(), before_profile.len(), "{tool_type:?}");
        for (a, b) in after_profile.iter().zip(before_profile.iter()) {
            let same = (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12;
            assert!(same, "{tool_type:?}: silhouette moved, {a:?} vs {b:?}");
        }
    }
}

/// Type-defining geometry is required, not guessed — the other half of
/// b0362626's rule. A tool whose type was switched in place (the GUI's
/// tool-type combo is the only surface that does that) reports what it
/// now needs, so the caller refills it instead of cutting with a zero.
/// `VBitEndmill::new` asserts `0 < included_angle < 180` and
/// `TaperedBallEndmill::new` asserts both `0 < taper_half_angle < 90`
/// and `shaft_diameter >= diameter`, so this is a panic guard, not a
/// nicety.
#[test]
fn switching_type_in_place_reports_the_geometry_it_now_needs() {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.tool_type = ToolType::VBit;
    let missing = tool.missing_defining_geometry();
    assert_eq!(missing, vec![ToolGeometryField::IncludedAngle]);

    tool.tool_type = ToolType::TaperedBallNose;
    let missing = tool.missing_defining_geometry();
    let expected = vec![
        ToolGeometryField::TaperHalfAngle,
        ToolGeometryField::ShaftDiameter,
    ];
    assert_eq!(missing, expected);

    // Refilled from the type's own defaults, it is a complete tool.
    let defaults = ToolConfig::new_default(tool.id, ToolType::TaperedBallNose);
    tool.taper_half_angle = defaults.taper_half_angle;
    tool.shaft_diameter = defaults.shaft_diameter;
    assert!(tool.missing_defining_geometry().is_empty());
}
