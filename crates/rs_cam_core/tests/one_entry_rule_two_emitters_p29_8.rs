//! P29-8 (commit 8a): one entry rule, two emitters.
//!
//! Pre-registration: `planning/feeds_contract_2026-11-03/phases/P29-8_DESIGN.md`
//! section 4.1. Ruling R6 (the operator, 2026-11-11): "it should be one bit
//! of code shared for the different tool paths".
//!
//! # The claim
//!
//! 1. Source: `feeds/entry_rule.rs` holds every entry number as a private
//!    row. No other file names an entry constant. Only the config defaults
//!    call `stored_default`. No emitter file reads an entry field of a
//!    config (`.ramp_angle`, `.ramp_angle_deg`, `.helix_pitch`,
//!    `.helix_radius`, `.helix_radius_factor`, `helix_radius_for(`): it
//!    reads `ResolvedEntry`.
//! 2. Read-back: on each door the emitted entry moves carry the numbers of
//!    `ResolvedEntry`:
//!    - the dressup door (`apply_dressups`): the ramp slope, the helix
//!      radius and pitch, and the air-lead feed (the plunge rule);
//!    - the 3D Rough door (a session generation on the op fixture): the
//!      ramp slope, and the helix radius.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::{Path, PathBuf};

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, rectangle, tool_for};
use rs_cam_core::compute::catalog::{EntryContract, OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle, StockSource};
use rs_cam_core::compute::execute::{DressupContext, apply_dressups};
use rs_cam_core::compute::operation_configs::Adaptive3dEntryStyle;
use rs_cam_core::feeds::entry_rule::{EntryAngle, EntryDoor, ResolvedEntry};
use rs_cam_core::geo::P3;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The source of `path` with comments and `#[cfg(test)]` items removed.
/// A multi-line attribute after `#[cfg(test)]` splits the attributes from
/// the item in `source_scan::items`, so an unnamed `#[cfg(test)]` item also
/// removes the item after it.
fn product_text(path: &Path) -> String {
    let src = std::fs::read_to_string(path).expect("read a source file");
    let text = source_scan::strip_comments(&src);
    let mut out = String::new();
    let mut skip_next = false;
    for item in source_scan::items(&text) {
        let test_attr = item.text.trim_start().starts_with("#[cfg(test)]");
        if test_attr || skip_next {
            skip_next = test_attr && item.name.is_none();
            continue;
        }
        out.push_str(&item.text);
    }
    out
}

/// True when `text` holds `needle` with no identifier character before it
/// and none after it (`(` after it too when `field` is set: a field read,
/// not a method call).
fn names(text: &str, needle: &str, field: bool) -> bool {
    let bytes = text.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.match_indices(needle).any(|(at, _)| {
        let first = needle.as_bytes()[0];
        let before = !ident(first) || at == 0 || !ident(bytes[at - 1]);
        let end = at + needle.len();
        let after = end >= bytes.len() || !(ident(bytes[end]) || (field && bytes[end] == b'('));
        before && after
    })
}

/// The entry constants that `entry_rule.rs` holds.
const ENTRY_CONSTANTS: [&str; 7] = [
    "RAMP_ANGLE_HARDWOOD_DEG",
    "RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG",
    "RAMP_ANGLE_NON_FERROUS_DEG",
    "RAMP_ANGLE_PLASTIC_DEG",
    "DRESSUP_HELIX_PITCH_MM",
    "ADAPTIVE3D_HELIX_PITCH_MM",
    "HELIX_RADIUS_OVER_D",
];

/// The files that emit an entry or resolve one for an emitter. The bore
/// rule of the 3D Rough planner has a field `helix_radius` too, so
/// `.helix_radius` is checked on these files only.
const EMITTER_FILES: [&str; 7] = [
    "compute/execute/dressup_apply.rs",
    "compute/execute/rough_3d.rs",
    "adaptive3d/path.rs",
    "dressup/mod.rs",
    "dressup/entry_descent.rs",
    "feeds/ramp.rs",
    "session/compute.rs",
];

/// The entry fields of a config, read as a field (not a method call).
const ENTRY_FIELDS: [&str; 4] = [
    ".ramp_angle",
    ".ramp_angle_deg",
    ".helix_pitch",
    ".helix_radius_factor",
];

/// The files that may read an entry field of a config: the rule, and the
/// two config files (the serde wire shapes and `helix_radius_for`, which
/// forwards to the rule).
const ENTRY_FIELD_READERS: [&str; 3] = [
    "feeds/entry_rule.rs",
    "compute/config.rs",
    "compute/operation_configs.rs",
];

/// The path of `path` under `src/`, with `/` separators.
fn rel_of(path: &Path) -> String {
    path.strip_prefix(src_dir())
        .expect("under src")
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn no_file_but_the_entry_rule_names_an_entry_number_p29_8() {
    let rule = src_dir().join("feeds/entry_rule.rs");
    let rule_text = product_text(&rule);
    // Non-vacuity: the rule file holds each constant and the stored door.
    for name in ENTRY_CONSTANTS {
        assert!(
            names(&rule_text, &format!("const {name}"), false),
            "non-vacuity: feeds/entry_rule.rs defines `{name}`"
        );
    }
    assert!(names(&rule_text, "fn stored_default", false));

    let allowed_stored = [
        "feeds/entry_rule.rs",
        "compute/config.rs",
        "compute/operation_configs.rs",
    ];
    let mut constant_hits = Vec::new();
    let mut stored_hits = Vec::new();
    for path in source_scan::rust_files(&src_dir()) {
        let rel = rel_of(&path);
        if rel.ends_with("tests.rs") || rel == "feeds/entry_rule.rs" {
            continue;
        }
        let text = product_text(&path);
        for name in ENTRY_CONSTANTS {
            if names(&text, name, false) {
                constant_hits.push(format!("{rel}: {name}"));
            }
        }
        if !allowed_stored.contains(&rel.as_str()) && names(&text, "stored_default", false) {
            stored_hits.push(rel.clone());
        }
    }
    assert!(
        constant_hits.is_empty(),
        "only feeds/entry_rule.rs names an entry constant: {constant_hits:?}"
    );
    assert!(
        stored_hits.is_empty(),
        "only the config defaults read `entry_rule::stored_default`: {stored_hits:?}"
    );
}

#[test]
fn no_emitter_reads_an_entry_field_of_a_config_p29_8() {
    // Non-vacuity: the needle finds a field read where one stands, and the
    // emitters name the door.
    let rule = product_text(&src_dir().join("feeds/entry_rule.rs"));
    assert!(names(&rule, ".ramp_angle", true));
    assert!(names(&rule, ".helix_radius_factor", true));

    // Every product file under src/ but the named readers.
    let mut hits = Vec::new();
    let mut scanned = 0;
    for path in source_scan::rust_files(&src_dir()) {
        let rel = rel_of(&path);
        if rel.ends_with("tests.rs") || ENTRY_FIELD_READERS.contains(&rel.as_str()) {
            continue;
        }
        scanned += 1;
        let text = product_text(&path);
        for field in ENTRY_FIELDS {
            if names(&text, field, true) {
                hits.push(format!("{rel}: {field}"));
            }
        }
        if names(&text, "helix_radius_for", false) {
            hits.push(format!("{rel}: helix_radius_for"));
        }
    }
    assert!(
        scanned > 100,
        "non-vacuity: the scan reads src/ ({scanned})"
    );

    // The emitter files: no `.helix_radius` field read either.
    let mut doors = 0;
    for rel in EMITTER_FILES {
        let text = product_text(&src_dir().join(rel));
        if names(&text, "ResolvedEntry", false) {
            doors += 1;
        }
        if names(&text, ".helix_radius", true) {
            hits.push(format!("{rel}: .helix_radius"));
        }
    }
    assert!(
        doors >= 4,
        "non-vacuity: dressup_apply, rough_3d, ramp and session read ResolvedEntry ({doors})"
    );
    assert!(
        hits.is_empty(),
        "a file reads an entry number of a config only through ResolvedEntry: {hits:?}"
    );
}

// ── The dressup door ────────────────────────────────────────────────────

const SAFE_Z: f64 = 20.0;
const DEPTH: f64 = -3.0;
const FEED: f64 = 1200.0;
/// The op plunge rate of the dressup cases: under the feed of the plunge
/// move, so the air lead shows which one it reads.
const PLUNGE: f64 = 400.0;
const TOOL_D: f64 = 6.0;

/// A rapid over the entry, a plunge to the depth, then a cut.
fn plunging_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, DEPTH), FEED, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(30.0, 0.0, DEPTH), FEED, MoveIntent::ClearingCut);
    tp
}

/// The stock material of the dressup door cases: generic softwood (the
/// ramp angle rule reads its class, P29-8 8b).
fn stock_material() -> rs_cam_core::material::Material {
    rs_cam_core::material::Material::SolidWood {
        species: rs_cam_core::material::WoodSpecies::GenericSoftwood,
    }
}

fn dress(cfg: &DressupConfig, containment: Option<&[Polygon2]>) -> Toolpath {
    apply_dressups(
        AnnotatedToolpath::new(plunging_toolpath()),
        DressupContext {
            material: &stock_material(),
            cfg,
            nominal_feed_rate: FEED,
            plunge_rate_mm_min: PLUNGE,
            ramp_feed_rate_mm_min: None,
            tool_diameter: TOOL_D,
            safe_z: SAFE_Z,
            stock_top: 0.0,
            prior_stock: None,
            link_kinematics: None,
            op_cutting_ceiling: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::Pocket.capabilities().transform,
            entry: EntryContract::ANY_DRESSUP_FROM_ROLE,
            debug_ctx: None,
            semantic_ctx: None,
            entry_containment: containment,
        },
        &mut ReconcileSet::empty(),
    )
    .toolpath
}

/// Each fed move of `intent` with the target before it.
fn fed_moves(tp: &Toolpath, intent: MoveIntent) -> Vec<(P3, &Move)> {
    tp.moves
        .windows(2)
        .filter(|w| w[1].intent == intent && w[1].move_type != MoveType::Rapid)
        .map(|w| (w[0].target, &w[1]))
        .collect()
}

/// The slope (degrees) of each descending linear move of `intent`.
fn slopes_deg(tp: &Toolpath, intent: MoveIntent) -> Vec<f64> {
    fed_moves(tp, intent)
        .iter()
        .filter_map(|(from, m)| {
            let dxy = (m.target.x - from.x).hypot(m.target.y - from.y);
            let dz = from.z - m.target.z;
            (dxy > 1e-9 && dz > 0.0).then(|| dz.atan2(dxy).to_degrees())
        })
        .collect()
}

/// The steepest slope (degrees) of the linear moves of `intent`.
fn max_slope_deg(tp: &Toolpath, intent: MoveIntent) -> f64 {
    slopes_deg(tp, intent).into_iter().fold(f64::NAN, f64::max)
}

/// The largest XY distance of a move of `intent` from `centre`.
fn max_radius(tp: &Toolpath, intent: MoveIntent, centre: (f64, f64)) -> f64 {
    fed_moves(tp, intent)
        .iter()
        .map(|(_, m)| (m.target.x - centre.0).hypot(m.target.y - centre.1))
        .fold(f64::NAN, f64::max)
}

/// The feed of each fed move of `intent`.
fn feeds(tp: &Toolpath, intent: MoveIntent) -> Vec<f64> {
    fed_moves(tp, intent)
        .iter()
        .filter_map(|(_, m)| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            MoveType::Rapid | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => None,
        })
        .collect()
}

#[test]
fn the_dressup_door_emits_the_resolved_entry_p29_8() {
    // A stated angle, pitch and radius that no rule holds, and the rule.
    for (style, ramp_angle, helix_pitch, helix_radius) in [
        (DressupEntryStyle::Ramp, EntryAngle::Stated(7.0), 1.0, None),
        (
            DressupEntryStyle::Ramp,
            DressupConfig::default().ramp_angle,
            1.0,
            None,
        ),
        (
            DressupEntryStyle::Helix,
            EntryAngle::Stated(3.0),
            1.5,
            Some(1.2),
        ),
        (
            DressupEntryStyle::Helix,
            EntryAngle::Stated(3.0),
            DressupConfig::default().helix_pitch,
            None,
        ),
    ] {
        let cfg = DressupConfig {
            entry_style: style,
            ramp_angle,
            helix_pitch,
            helix_radius,
            link_moves: None,
            feed_optimization: false,
            ..DressupConfig::default()
        };
        // No cutter, as the K4 sentry: the air lead is fed (no replayed
        // stock), and the helix radius is the request.
        let entry = ResolvedEntry::of_dressup(&cfg, None, TOOL_D, &stock_material(), Some(PLUNGE));
        assert_eq!(entry.door(), EntryDoor::Dressup);
        let tp = dress(&cfg, None);

        // P29-8 8c (operator ruling O1): the air lead runs at the op plunge
        // rate, the one plunge rule of both doors (`min` with the plunge
        // move's own feed).
        let lead = feeds(&tp, MoveIntent::EntryApproach);
        assert!(!lead.is_empty(), "non-vacuity: {style:?} has an air lead");
        // P29-8e: the pipeline reads the plunge feed of `ResolvedEntry`.
        let plunge = entry.plunge_feed().expect("the op states a plunge rate");
        assert_eq!(plunge.mm_min().to_bits(), PLUNGE.to_bits());
        let rule = plunge.mm_min().min(FEED);
        for feed in lead {
            assert_eq!(feed.to_bits(), rule.to_bits(), "{style:?}: air-lead feed");
        }

        match style {
            DressupEntryStyle::Ramp => {
                let slope = max_slope_deg(&tp, MoveIntent::EntryRamp);
                assert!(
                    (slope - entry.ramp_angle_deg()).abs() < 1e-6,
                    "ramp slope {slope} vs ResolvedEntry {}",
                    entry.ramp_angle_deg()
                );
            }
            DressupEntryStyle::Helix => {
                let r = max_radius(&tp, MoveIntent::EntryHelix, (0.0, 0.0));
                let want = entry.helix_radius().emitted_mm;
                assert!((r - want).abs() < 1e-6, "helix radius {r} vs {want}");
                // One turn is 36 steps (`HELIX_STEPS_PER_REV`): on a flat
                // stock top the first 36 steps drop one pitch.
                let helix = fed_moves(&tp, MoveIntent::EntryHelix);
                let drop = helix[1].1.target.z - helix[37].1.target.z;
                assert!(
                    (drop - entry.helix_pitch_mm()).abs() < 1e-6,
                    "helix pitch {drop} vs {}",
                    entry.helix_pitch_mm()
                );
            }
            DressupEntryStyle::None => unreachable!("the cases state an entry"),
        }
    }
}

/// Lead ruling 2026-11-12: a helix with no room ramps at the op's own
/// angle, not at a fixed 3 degrees. A containment region that does not hold
/// the entry point leaves no room for any helix.
#[test]
fn a_dressup_helix_with_no_room_ramps_at_the_op_angle_p29_8() {
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        ramp_angle: EntryAngle::Stated(7.0),
        link_moves: None,
        feed_optimization: false,
        ..DressupConfig::default()
    };
    let entry = ResolvedEntry::of_dressup(&cfg, None, TOOL_D, &stock_material(), Some(PLUNGE));
    assert_eq!(entry.fallback_ramp_angle_deg(), 7.0);
    let away = [rectangle(100.0, 100.0, 110.0, 110.0)];
    let tp = dress(&cfg, Some(&away));
    assert!(
        fed_moves(&tp, MoveIntent::EntryHelix).is_empty(),
        "no helix fits outside the containment"
    );
    let slope = max_slope_deg(&tp, MoveIntent::EntryRamp);
    assert!(
        (slope - entry.fallback_ramp_angle_deg()).abs() < 1e-6,
        "fallback ramp slope {slope} vs the op angle {}",
        entry.fallback_ramp_angle_deg()
    );
}

// ── The 3D Rough door ───────────────────────────────────────────────────

/// The 3D Rough op of the fixture with `style`, generated through the
/// session door, and its entry rule.
fn rough_3d(style: Adaptive3dEntryStyle) -> (Toolpath, ResolvedEntry) {
    let mut op = OperationConfig::new_default(OperationType::Adaptive3d);
    let OperationConfig::Adaptive3d(cfg) = &mut op else {
        unreachable!("new_default(Adaptive3d)")
    };
    cfg.entry_style = style;
    let cutter = rs_cam_core::compute::build_cutter(&tool_for(OperationType::Adaptive3d))
        .expect("the fixture tool builds");
    let config = cfg.clone();
    let (mut session, index) = case_session(CaseSpec {
        name: "rough_3d_entry",
        op,
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    // The session's own stock material: the ramp angle rule reads it.
    let entry = ResolvedEntry::of_adaptive3d(
        &config,
        &cutter,
        &session.stock_config().material,
        session.toolpath_configs()[index].operation.plunge_rate(),
    );
    common::session::generate(&mut session, index);
    let tp = session
        .result_at(index)
        .expect("the op has a result")
        .toolpath()
        .clone();
    (tp, entry)
}

#[test]
fn the_3d_rough_door_emits_the_resolved_entry_p29_8() {
    let (ramp_tp, ramp) = rough_3d(Adaptive3dEntryStyle::Ramp);
    assert_eq!(ramp.door(), EntryDoor::Adaptive3d);
    // A ramp leg over open stock runs at the angle; the G-RAMPTERRAIN clip
    // re-plans a leg that meets the surface (steeper or shallower). Most
    // legs on the fixture are open.
    let legs = slopes_deg(&ramp_tp, MoveIntent::EntryRamp);
    let at_angle = legs
        .iter()
        .filter(|s| (**s - ramp.ramp_angle_deg()).abs() < 1e-6)
        .count();
    assert!(
        2 * at_angle > legs.len(),
        "3D ramp: {at_angle} of {} legs at the ResolvedEntry angle {}",
        legs.len(),
        ramp.ramp_angle_deg()
    );

    let (helix_tp, helix) = rough_3d(Adaptive3dEntryStyle::Helix);
    let want = helix.helix_radius().emitted_mm;
    // Each helix turns about the entry XY: the target of the move before
    // the first helix move of each entry.
    let moves = &helix_tp.moves;
    let mut entries = 0;
    for i in 1..moves.len() {
        if moves[i].intent != MoveIntent::EntryHelix
            || moves[i - 1].intent == MoveIntent::EntryHelix
        {
            continue;
        }
        let centre = (moves[i - 1].target.x, moves[i - 1].target.y);
        let r = moves[i..]
            .iter()
            .take_while(|m| m.intent == MoveIntent::EntryHelix)
            .map(|m| (m.target.x - centre.0).hypot(m.target.y - centre.1))
            .fold(0.0, f64::max);
        assert!(
            (r - want).abs() < 1e-6,
            "3D helix radius {r} vs ResolvedEntry {want}"
        );
        entries += 1;
    }
    assert!(
        entries > 0,
        "non-vacuity: the 3D Rough op has helix entries"
    );
}

// ── Stated or the rule (P29-8 8b) ───────────────────────────────────────

/// A setter writes a stated angle; `null` and the token `"rule"` write the
/// rule; the writer omits the rule and saves a stated angle as the number.
/// The 3D Rough param door and the dressup field door read one shape.
#[test]
fn a_setter_writes_a_stated_angle_and_the_token_writes_the_rule_p29_8() {
    use rs_cam_core::session::{Command, SetDressupFieldArgs, SetToolpathParamArgs};
    let (mut session, index) = case_session(CaseSpec {
        name: "entry_angle_setter",
        op: OperationConfig::new_default(OperationType::Adaptive3d),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    let a3d =
        |s: &rs_cam_core::session::ProjectSession| match &s.toolpath_configs()[index].operation {
            OperationConfig::Adaptive3d(c) => c.ramp_angle_deg,
            other => panic!("the case op is a 3D Rough: {other:?}"),
        };
    assert_eq!(a3d(&session), EntryAngle::Rule, "a new op holds the rule");
    for (value, want) in [
        (serde_json::json!(7.0), EntryAngle::Stated(7.0)),
        (serde_json::json!("rule"), EntryAngle::Rule),
        (serde_json::json!(12), EntryAngle::Stated(12.0)),
        (serde_json::Value::Null, EntryAngle::Rule),
    ] {
        let _ = session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: "ramp_angle_deg".to_owned(),
                value: value.clone(),
            }))
            .unwrap_or_else(|e| panic!("ramp_angle_deg = {value}: {e}"));
        assert_eq!(a3d(&session), want, "ramp_angle_deg = {value}");
        let text =
            toml::to_string(&session.toolpath_configs()[index].operation).expect("the op writes");
        assert_eq!(
            text.contains("ramp_angle_deg"),
            !want.is_rule(),
            "the writer omits the rule only: {text}"
        );
    }
    // The schema range still holds a stated angle.
    assert!(
        session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: "ramp_angle_deg".to_owned(),
                value: serde_json::json!(80.0),
            }))
            .is_err(),
        "80 degrees is outside the schema range"
    );

    // The dressup field door.
    for (value, want) in [
        (serde_json::json!(4.5), EntryAngle::Stated(4.5)),
        (serde_json::json!("rule"), EntryAngle::Rule),
        (serde_json::json!(4.5), EntryAngle::Stated(4.5)),
        (serde_json::Value::Null, EntryAngle::Rule),
    ] {
        let _ = session
            .apply(Command::SetDressupField(SetDressupFieldArgs {
                index,
                key: "ramp_angle".to_owned(),
                value: value.clone(),
            }))
            .unwrap_or_else(|e| panic!("ramp_angle = {value}: {e}"));
        let dressups = &session.toolpath_configs()[index].dressups;
        assert_eq!(dressups.ramp_angle, want, "ramp_angle = {value}");
        let text = toml::to_string(dressups).expect("the dressups write");
        assert_eq!(
            text.contains("ramp_angle"),
            !want.is_rule(),
            "the writer omits the rule only: {text}"
        );
    }
}
