//! **G10 Part B** — a helix leaves no core, and a ramp never outruns the
//! feed.
//!
//! Plan: `planning/extrapolation_2026-09-24/G10_PLAN.md` §6 (Part B) and §8
//! (operator decisions D2-D4, 2026-09-25).
//!
//! # What is pinned
//!
//! - **Q6 (the cap).** The engine emits a helix radius capped at the flat
//!   bottom of the tool, `width_at_height(0.0)`, so a flat or bull helix
//!   leaves no core. The numbers are the tool geometry:
//!   - a 3.175 mm flat has a flat bottom of 3.175 / 2 = 1.5875 mm, so an
//!     operator r 2.0 is emitted at r 1.5875;
//!   - a 3.175 mm bull with corner radius 0.476 mm has a flat bottom of
//!     1.5875 - 0.476 = 1.1115 mm (the plan prints 1.111);
//!   - a 6 mm ball has no flat bottom. The rule 0.3 x 6 = 1.8 mm is emitted
//!     uncapped, and the card shows the centre pip the ball profile leaves:
//!     3 - sqrt(3² - 1.8²) = 3 - 2.4 = 0.60 mm;
//!   - a 3.175 mm flat on the rule (`None`) is emitted at 0.3 x 3.175 =
//!     0.9525 mm, inside the flat bottom.
//!
//!   The radius is MEASURED on the generated toolpath: the helix steps are
//!   10° apart about the entry XY (`dressup::emit_helix`), so the step k
//!   and the step k + 18 are opposite points, and half their XY distance is
//!   the radius.
//! - **Q6 on Adaptive3d.** A 6 mm flat with `helix_radius_factor` 0.8
//!   (r 4.8) is emitted at its flat bottom, 3.0 mm.
//! - **G-RAMPCLAMP.** With `ramp_feed_rate = 4000` and the feed lowered to
//!   1500, every `EntryHelix` and `EntryRamp` move runs at or below 1500, on
//!   the dressup door (Pocket) and on Adaptive3d. At least one runs AT 1500,
//!   so the entry really rides the clamped ramp feed and not the plunge.
//! - **D2.** The v3 fixture `test_job.toml` carries `helix_radius = 2.0` on
//!   every toolpath; it loads as the operator value `Some(2.0)`, with no
//!   format bump.
//! - **D3.** A new 2D Adaptive toolpath is Helix; an operator Ramp survives a
//!   dressup write and a project save and load.
//! - **D4.** The named repo rules are the defaults the configs read. The
//!   CLI half (`entry_3d = "ramp"` writes 10°) is a `rs_cam_cli` unit test,
//!   `job::tests::the_adaptive3d_entry_is_the_gui_rule_g10`: core cannot
//!   depend on the CLI.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::{Path, PathBuf};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    DRESSUP_HELIX_PITCH_MM, DRESSUP_RAMP_ANGLE_DEG, DressupConfig, DressupEntryStyle,
    HELIX_RADIUS_OVER_D,
};
use rs_cam_core::compute::operation_configs::{
    ADAPTIVE3D_HELIX_PITCH_MM, ADAPTIVE3D_RAMP_ANGLE_DEG, Adaptive3dConfig, Adaptive3dEntryStyle,
    ClearingStrategy,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::PassRole;
use rs_cam_core::feeds::ramp::entry_notes;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{Command, ProjectSession, SetDressupFieldArgs};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The lowered cut feed (mm/min).
const FEED: f64 = 1500.0;
/// The stored ramp feed, above the cut feed (mm/min).
const RAMP_FEED: f64 = 4000.0;
/// The plunge feed (mm/min), below the cut feed.
const PLUNGE: f64 = 385.0;
/// Pocket half-width (mm): wide enough that no helix is contained away.
const POCKET_HALF: f64 = 20.0;
/// Pocket depth and depth per pass (mm): three helix turns at pitch 1.
const POCKET_DEPTH: f64 = 3.0;

fn tool(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), kind);
    t.diameter = diameter;
    t.cutting_length = 25.0;
    t.flute_count = 2;
    t
}

fn bull_3175() -> ToolConfig {
    let mut t = tool(ToolType::BullNose, 3.175);
    t.corner_radius = 0.476;
    t
}

fn pocket_op(ramp_feed: Option<f64>) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Pocket);
    if let OperationConfig::Pocket(cfg) = &mut op {
        cfg.depth = POCKET_DEPTH;
        cfg.depth_per_pass = POCKET_DEPTH;
        cfg.stepover = 1.0;
        cfg.feed_rate = FEED;
        cfg.plunge_rate = PLUNGE;
        cfg.ramp_feed_rate = ramp_feed;
    }
    op
}

/// A Pocket on a square, through `ProjectSession::generate_toolpath` (the
/// dressup door the GUI and the CLI share). Feed optimisation and arc
/// fitting are off, so the entry feeds are the entry emitter's own.
fn pocket_toolpath(
    tool: ToolConfig,
    style: DressupEntryStyle,
    helix_radius: Option<f64>,
    ramp_feed: Option<f64>,
) -> Toolpath {
    let mut session = common::session::single_op_session_with(
        common::session::stock_under(POCKET_HALF + 5.0, 10.0),
        tool,
        common::session::polygon_model(
            vec![common::session::square_polygon(POCKET_HALF)],
            "square",
        ),
        "Pocket",
        pocket_op(ramp_feed),
        |tc| {
            tc.dressups.entry_style = style;
            tc.dressups.helix_radius = helix_radius;
            tc.dressups.helix_pitch = 1.0;
            tc.dressups.feed_optimization = false;
            tc.dressups.arc_fitting = None;
        },
    );
    common::session::generate(&mut session, 0);
    session.get_result(0).unwrap().toolpath().clone()
}

/// The first contiguous run of `EntryHelix` moves.
fn first_helix_run(tp: &Toolpath) -> Vec<(f64, f64)> {
    let start = tp
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::EntryHelix)
        .unwrap_or_else(|| panic!("no helix entry: the fixture fell back to a plunge"));
    tp.moves[start..]
        .iter()
        .take_while(|m| m.intent == MoveIntent::EntryHelix)
        .map(|m| (m.target.x, m.target.y))
        .collect()
}

/// The emitted helix radius: half the XY distance between the step k and
/// the step k + 18 (10° steps, so 180° apart), over the first turn.
fn emitted_helix_radius(tp: &Toolpath) -> f64 {
    let run = first_helix_run(tp);
    assert!(
        run.len() > 36,
        "the fixture must give a full helix turn: {} steps",
        run.len()
    );
    let radii: Vec<f64> = (0..18)
        .map(|k| {
            let (a, b) = (run[k], run[k + 18]);
            0.5 * (a.0 - b.0).hypot(a.1 - b.1)
        })
        .collect();
    let first = radii[0];
    for r in &radii {
        assert!((r - first).abs() < 1e-6, "not one circle: {radii:?}");
    }
    first
}

fn assert_radius(label: &str, got: f64, want: f64) {
    assert!(
        (got - want).abs() < 1e-6,
        "{label}: the helix is emitted at r {got:.6} mm, want {want:.6} mm"
    );
}

/// Q6: an operator r 2.0 on a 3.175 mm flat is emitted at the flat bottom,
/// 1.5875 mm; on the 0.476 mm corner bull at 1.1115 mm.
#[test]
fn a_flat_or_bull_helix_is_emitted_at_the_flat_bottom_g10() {
    let flat = pocket_toolpath(
        tool(ToolType::EndMill, 3.175),
        DressupEntryStyle::Helix,
        Some(2.0),
        None,
    );
    assert_radius(
        "3.175 flat, r 2.0",
        emitted_helix_radius(&flat),
        3.175 / 2.0,
    );

    let bull = pocket_toolpath(bull_3175(), DressupEntryStyle::Helix, Some(2.0), None);
    assert_radius(
        "3.175 bull rc 0.476, r 2.0",
        emitted_helix_radius(&bull),
        3.175 / 2.0 - 0.476,
    );

    // The rule 0.3 x D sits inside the flat bottom and is not moved.
    let rule = pocket_toolpath(
        tool(ToolType::EndMill, 3.175),
        DressupEntryStyle::Helix,
        None,
        None,
    );
    assert_radius("3.175 flat, rule", emitted_helix_radius(&rule), 0.3 * 3.175);

    // The card states the cap.
    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        helix_radius: Some(2.0),
        ..DressupConfig::default()
    };
    let notes = entry_notes(
        &pocket_op(None),
        Some(&dressups),
        &tool(ToolType::EndMill, 3.175),
        PassRole::Roughing,
    );
    assert!(
        notes
            .as_slice()
            .iter()
            .any(|n| n.headline.contains("capped r 2.00 → 1.59 mm")),
        "{notes:?}"
    );
    assert!(
        !notes.as_slice().iter().any(|n| n.caution),
        "no core is left, so no caution: {notes:?}"
    );
}

/// Q6, Q7: a 6 mm ball has no flat bottom. The rule r 0.3 x 6 = 1.8 mm is
/// emitted as it stands, and the card shows the pip 3 - sqrt(9 - 1.8²) =
/// 0.60 mm.
#[test]
fn a_ball_helix_is_not_capped_and_the_card_shows_the_pip_g10() {
    let ball = tool(ToolType::BallNose, 6.0);
    let tp = pocket_toolpath(ball.clone(), DressupEntryStyle::Helix, None, None);
    assert_radius("6 ball, rule", emitted_helix_radius(&tp), 0.3 * 6.0);

    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        ..DressupConfig::default()
    };
    let notes = entry_notes(&pocket_op(None), Some(&dressups), &ball, PassRole::Roughing);
    let pip = 3.0 - (9.0_f64 - 1.8 * 1.8).sqrt();
    assert!((pip - 0.6).abs() < 1e-12);
    assert!(
        notes
            .as_slice()
            .iter()
            .any(|n| n.headline.contains("centre pip 0.60 mm")),
        "{notes:?}"
    );
    assert!(
        notes.as_slice()[0]
            .headline
            .starts_with("Helix r 1.80 mm (0.30 x D)"),
        "{notes:?}"
    );
}

/// Lead decision 2026-09-25: the rule reads the NOMINAL cutting diameter
/// (`helix_entry_diameter_mm`), not a tapered ball's shaft. A 3.175 mm
/// tapered ball (default shaft 6.35, half-angle 15°) under the rule emits
/// r = 0.3 x 3.175 = 0.9525 mm. That is inside the tip sphere's reach,
/// R cos 15° = 1.5875 x 0.9659 = 1.533 mm, so the pip is the tip sphere's:
/// R - sqrt(R² - r²) = 1.5875 - sqrt(1.5875² - 0.9525²) = 1.5875 - 1.27 =
/// 0.3175 mm. Read off the shaft (0.3 x 6.35 = 1.905 mm) the pip was on the
/// taper.
#[test]
fn a_tapered_helix_reads_the_nominal_diameter_g10() {
    let tapered = tool(ToolType::TaperedBallNose, 3.175);
    assert!(
        tapered.shaft_diameter > tapered.diameter,
        "the fixture is tapered"
    );
    let tp = pocket_toolpath(tapered.clone(), DressupEntryStyle::Helix, None, None);
    assert_radius(
        "3.175 tapered, rule",
        emitted_helix_radius(&tp),
        0.3 * 3.175,
    );

    let r = 0.3 * 3.175_f64;
    let big_r = 3.175 / 2.0_f64;
    assert!(
        r <= big_r * 15.0_f64.to_radians().cos(),
        "r is on the tip sphere"
    );
    let pip = big_r - (big_r * big_r - r * r).sqrt();
    assert!((pip - 0.3175).abs() < 1e-9, "{pip}");

    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        ..DressupConfig::default()
    };
    let notes = entry_notes(
        &pocket_op(None),
        Some(&dressups),
        &tapered,
        PassRole::Roughing,
    );
    assert!(
        notes.as_slice()[0]
            .headline
            .starts_with("Helix r 0.95 mm (0.30 x D)"),
        "{notes:?}"
    );
    assert!(
        notes
            .as_slice()
            .iter()
            .any(|n| n.headline.contains("centre pip 0.32 mm")),
        "{notes:?}"
    );
}

/// Every fed `EntryHelix` / `EntryRamp` move's feed.
fn entry_feeds(tp: &Toolpath) -> Vec<f64> {
    tp.moves
        .iter()
        .filter(|m| matches!(m.intent, MoveIntent::EntryHelix | MoveIntent::EntryRamp))
        .filter_map(|m| m.move_type.feed_rate())
        .collect()
}

fn assert_clamped(label: &str, tp: &Toolpath) {
    let feeds = entry_feeds(tp);
    assert!(!feeds.is_empty(), "{label}: no helix or ramp entry move");
    let max = feeds.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max <= FEED + 1e-9,
        "{label}: an entry move runs at {max} mm/min, above the cut feed {FEED}"
    );
    assert!(
        feeds.iter().any(|f| (f - FEED).abs() < 1e-9),
        "{label}: no entry move rides the clamped ramp feed {FEED}: {feeds:?}"
    );
}

/// G-RAMPCLAMP on the dressup door: the stored ramp 4000 above the feed 1500
/// runs at 1500.
#[test]
fn a_dressup_entry_never_outruns_the_feed_g10() {
    for style in [DressupEntryStyle::Helix, DressupEntryStyle::Ramp] {
        let tp = pocket_toolpath(common::make_endmill_6mm(), style, None, Some(RAMP_FEED));
        assert_clamped(&format!("Pocket {style:?}"), &tp);
    }
}

fn adaptive3d_toolpath(style: Adaptive3dEntryStyle, helix_radius_factor: f64) -> Toolpath {
    let cfg = Adaptive3dConfig {
        stepover: 2.0,
        depth_per_pass: 8.0,
        stock_to_leave_axial: 0.5,
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        ramp_feed_rate: Some(RAMP_FEED),
        entry_style: style,
        helix_radius_factor,
        clearing_strategy: ClearingStrategy::ContourParallel,
        // Keep-down off: every region starts with a styled entry.
        max_stay_down_distance_mm: Some(0.0),
        ..Adaptive3dConfig::default()
    };
    let mut session = common::session::single_op_session_with(
        common::session::stock_over(20.0, 16.0),
        common::make_endmill_6mm(),
        common::session::mesh_model(make_test_flat(40.0), "plate"),
        "3D Rough",
        OperationConfig::Adaptive3d(cfg),
        |tc| {
            tc.dressups.feed_optimization = false;
            tc.dressups.arc_fitting = None;
        },
    );
    common::session::generate(&mut session, 0);
    session.get_result(0).unwrap().toolpath().clone()
}

/// G-RAMPCLAMP on Adaptive3d, and Q6 on its helix: a 6 mm flat at factor
/// 0.8 (r 4.8) is emitted at its flat bottom, 3.0 mm.
#[test]
fn an_adaptive3d_entry_never_outruns_the_feed_and_leaves_no_core_g10() {
    let helix = adaptive3d_toolpath(Adaptive3dEntryStyle::Helix, 0.8);
    assert_clamped("Adaptive3d Helix", &helix);
    assert_radius(
        "Adaptive3d 6 flat, factor 0.8",
        emitted_helix_radius(&helix),
        3.0,
    );

    let ramp = adaptive3d_toolpath(Adaptive3dEntryStyle::Ramp, HELIX_RADIUS_OVER_D);
    assert_clamped("Adaptive3d Ramp", &ramp);
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// D2: a v3 project that stores `helix_radius = 2.0` loads it as the
/// operator value `Some(2.0)`; no format bump.
#[test]
fn a_v3_project_loads_its_helix_radius_as_an_operator_value_g10() {
    let path = fixture_path("test_job.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("format_version = 3"), "the fixture is v3");
    let stored = text.matches("helix_radius = 2.0").count();
    assert!(stored > 0, "the fixture stores helix_radius = 2.0");
    let session = ProjectSession::load(&path).expect("the v3 fixture loads");
    let configs = session.toolpath_configs();
    assert_eq!(configs.len(), stored, "one stored radius per toolpath");
    for tc in configs {
        assert_eq!(tc.dressups.helix_radius, Some(2.0), "{}", tc.name);
    }
}

/// D3: a new 2D Adaptive is Helix; an operator Ramp survives a dressup write
/// and a project save and load (the old `PreferHelix` rewrote it on load).
#[test]
fn an_operator_ramp_on_an_adaptive_survives_write_and_load_g10() {
    assert_eq!(
        DressupConfig::for_op(OperationType::Adaptive).entry_style,
        DressupEntryStyle::Helix
    );
    let mut session = common::session::single_op_session(
        common::session::stock_under(POCKET_HALF + 5.0, 10.0),
        common::make_endmill_6mm(),
        common::session::polygon_model(
            vec![common::session::square_polygon(POCKET_HALF)],
            "square",
        ),
        "Adaptive",
        OperationConfig::new_default(OperationType::Adaptive),
    );
    assert_eq!(
        session.toolpath_configs()[0].dressups.entry_style,
        DressupEntryStyle::Helix,
        "a new Adaptive toolpath is Helix"
    );
    let _ = session
        .apply(Command::SetDressupField(SetDressupFieldArgs {
            index: 0,
            key: "entry_style".to_owned(),
            value: serde_json::json!("ramp"),
        }))
        .expect("the dressup write");
    assert_eq!(
        session.toolpath_configs()[0].dressups.entry_style,
        DressupEntryStyle::Ramp,
        "the write keeps the operator Ramp"
    );
    // D2 on the wire: a number is an operator value, null is the rule.
    for (value, want) in [
        (serde_json::json!(1.5), Some(1.5)),
        (serde_json::Value::Null, None),
    ] {
        let _ = session
            .apply(Command::SetDressupField(SetDressupFieldArgs {
                index: 0,
                key: "helix_radius".to_owned(),
                value,
            }))
            .expect("the helix radius write");
        assert_eq!(session.toolpath_configs()[0].dressups.helix_radius, want);
    }

    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_g10b_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.toml");
    session.save(&path).unwrap();
    let loaded = ProjectSession::load(&path).expect("the saved project loads");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
    assert_eq!(
        loaded.toolpath_configs()[0].dressups.entry_style,
        DressupEntryStyle::Ramp,
        "the load keeps the operator Ramp"
    );
}

/// D4 and the named repo rules: the defaults read the constants.
#[test]
fn the_entry_defaults_are_the_named_repo_rules_g10() {
    let d = DressupConfig::default();
    assert_eq!(d.ramp_angle, DRESSUP_RAMP_ANGLE_DEG);
    assert_eq!(d.helix_pitch, DRESSUP_HELIX_PITCH_MM);
    assert_eq!(d.helix_radius, None, "None is the rule 0.3 x D");
    let a = Adaptive3dConfig::default();
    assert_eq!(a.ramp_angle_deg, ADAPTIVE3D_RAMP_ANGLE_DEG);
    assert_eq!(a.helix_pitch, ADAPTIVE3D_HELIX_PITCH_MM);
    assert_eq!(a.helix_radius_factor, HELIX_RADIUS_OVER_D);
}
