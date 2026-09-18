//! R11 (Corne case analysis 2026-09-18, §4.6) — the machine-safety pass
//! compares in ONE frame: the frame of the program text.
//!
//! ## The defect
//!
//! `export_gcode` on a project with stock Z 0..23 and `post.safe_z = 10`
//! reported 18 680 errors: "Rapid (G0) repositions in X/Y at Z5.000, below
//! the clearance plane 10.000". The program is correct. The export writes
//! Z0 at the stock top (`ZDatum::StockTop`, since b252e2ce), and the
//! toolpath retracts to `effective_safe_z = max(10, 23 + 5) = 28` world,
//! which the datum shift turns into program Z5. The validator compared
//! the program's Z5 against the raw world-frame `post.safe_z = 10`. Every
//! datum-at-stock-top export read as unsafe.
//!
//! ## The fix
//!
//! `gcode_validator::emission_frame_clearance_z` resolves the retract
//! plane the way the export does (`SetupEvalContext::safe_z` plus the
//! setup's `export_datum_shift().z`) and the export doors feed that value
//! to `MachineSafety::clearance_z`. The check is not weaker: a rapid XY
//! move below the emitted retract plane is still an error.
//!
//! ## Fixture
//!
//! Stock 60×60×23 at origin (0,0,0), so the stock top is world Z 23 and
//! the identity setup keeps the `StockTop` default. Two traces of two
//! squares with a Ø6 end mill, exported through the production session
//! entry point (`ProjectSession::export_gcode_with_policy`). The program
//! has XY rapids at the retract plane between the squares and between
//! the two phases.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, toolpath_config};

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::SAFE_Z_CLEARANCE_MM;
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::export::gcode_validator::{
    Finding, FindingKind, MachineSafety, emission_frame_clearance_z, validate_machine_safety,
};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};
use std::sync::atomic::AtomicBool;

const STOCK_XY: f64 = 60.0;
const STOCK_Z: f64 = 23.0;
const EPS: f64 = 1e-9;

fn square(x0: f64, y0: f64, side: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x0 + side, y0),
        P2::new(x0 + side, y0 + side),
        P2::new(x0, y0 + side),
    ])
}

fn trace_op() -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: 4.0,
        depth_per_pass: 2.0,
        ..TraceConfig::default()
    })
}

/// One identity setup over a stock whose top sits at world Z 23. Two
/// traces of a model that holds two separate squares.
fn build_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: STOCK_XY,
        y: STOCK_XY,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(polygon_model(
        vec![square(5.0, 5.0, 15.0), square(35.0, 35.0, 15.0)],
        "two_squares",
    ));

    let _ = builder
        .add_toolpath(0, toolpath_config("TraceA", trace_op(), tool_id, model_id))
        .expect("add trace A");
    let _ = builder
        .add_toolpath(0, toolpath_config("TraceB", trace_op(), tool_id, model_id))
        .expect("add trace B");
    builder.build()
}

/// Generate both toolpaths and export one program through the production
/// session entry point. Returns the session and the program text.
fn export_program() -> (ProjectSession, String) {
    let mut session = build_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate trace A");
    session
        .generate_toolpath(1, &cancel)
        .expect("generate trace B");

    // Unique per call: the tests below run concurrently in one binary.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("rs_cam_r11_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("stock_top_23_{seq}.nc"));
    session
        .export_gcode_with_policy(
            &path,
            rs_cam_core::gcode::ToolLoadExportPolicy {
                // No simulation runs here, so every load gate reads
                // `Unmodeled`. This file is about frames, not load.
                accept_unmodeled: true,
                accept_exceeded: true,
            },
        )
        .expect("export the program");
    let gcode = std::fs::read_to_string(&path).expect("read the exported program");
    let _ = std::fs::remove_file(&path);
    (session, gcode)
}

fn count_kind(findings: &[Finding], kind: FindingKind) -> usize {
    findings.iter().filter(|f| f.kind == kind).count()
}

fn safety(clearance_z: f64) -> MachineSafety {
    MachineSafety {
        clearance_z,
        min_z: None,
        max_feed_mm_min: None,
    }
}

/// The value of word `letter` on a comment-free G-code line.
fn word(line: &str, letter: char) -> Option<f64> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(letter).and_then(|v| v.parse::<f64>().ok()))
}

/// How many `G0` lines move in X or Y while the modal Z equals `plane`.
fn xy_rapids_at(gcode: &str, plane: f64) -> usize {
    let mut z: Option<f64> = None;
    let mut count = 0usize;
    for line in gcode.lines() {
        let is_motion = line.starts_with("G0 ") || line.starts_with("G1 ");
        if !is_motion {
            continue;
        }
        if let Some(v) = word(line, 'Z') {
            z = Some(v);
        }
        let moves_xy = word(line, 'X').is_some() || word(line, 'Y').is_some();
        let at_plane = z.is_some_and(|cur| (cur - plane).abs() < 1e-6);
        if line.starts_with("G0 ") && moves_xy && at_plane {
            count += 1;
        }
    }
    count
}

/// The emitted retract plane on this fixture: `max(10, 23 + 5) - 23`.
fn expected_plane(session: &ProjectSession) -> f64 {
    let raw = session.post_config().safe_z;
    raw.max(STOCK_Z + SAFE_Z_CLEARANCE_MM) - STOCK_Z
}

#[test]
fn the_helper_resolves_the_retract_plane_in_the_program_frame() {
    let session = build_session();
    let plane = emission_frame_clearance_z(&session, 0..2);
    let expected = expected_plane(&session);
    assert!(
        (plane - expected).abs() < EPS,
        "R11: emission_frame_clearance_z read {plane}, expected {expected} \
         (effective safe Z 28 world, stock top 23 at program Z0)"
    );
    assert!(
        (plane - SAFE_Z_CLEARANCE_MM).abs() < EPS,
        "the fixture does not exercise the datum shift: plane {plane}"
    );
}

#[test]
fn an_export_with_the_datum_at_the_stock_top_reads_clean() {
    let (session, gcode) = export_program();
    let plane = emission_frame_clearance_z(&session, 0..2);

    // Non-vacuity: the program does rapid in XY at the emitted plane.
    let at_plane = xy_rapids_at(&gcode, plane);
    assert!(
        at_plane > 0,
        "fixture is vacuous: no G0 XY move at Z{plane:.3} in:\n{gcode}"
    );

    // The old comparison (raw world-frame post.safe_z) reproduces R11.
    let raw = session.post_config().safe_z;
    let old = validate_machine_safety(&gcode, safety(raw));
    assert!(
        count_kind(&old, FindingKind::RapidBelowClearance) > 0,
        "fixture does not reproduce R11: a raw clearance of {raw} raised no \
         RapidBelowClearance finding"
    );

    // The one-frame comparison is silent on the same bytes.
    let fixed = validate_machine_safety(&gcode, safety(plane));
    let below = count_kind(&fixed, FindingKind::RapidBelowClearance);
    assert_eq!(
        below,
        0,
        "R11: {below} RapidBelowClearance findings against the emitted plane \
         {plane:.3}; first: {:?}",
        fixed
            .iter()
            .find(|f| f.kind == FindingKind::RapidBelowClearance)
            .map(|f| &f.message)
    );
}

#[test]
fn a_rapid_below_the_emitted_plane_is_still_an_error() {
    let session = build_session();
    let plane = emission_frame_clearance_z(&session, 0..2);

    let program_with_rapid_at = |z: f64| {
        format!(
            "G54\nM3 S1000\nG0 X0 Y0 Z{plane:.3}\nG1 Z-2 F200\nG1 X10 F600\n\
             G0 Z{z:.3}\nG0 X20 Y20\nG0 Z{plane:.3}\nM5\nM30\n"
        )
    };

    // Below the stock top: inside the part.
    let inside = validate_machine_safety(&program_with_rapid_at(-5.0), safety(plane));
    assert_eq!(
        count_kind(&inside, FindingKind::RapidBelowClearance),
        1,
        "a G0 XY move at Z-5 (inside the stock) must be an error"
    );
    let message = &inside
        .iter()
        .find(|f| f.kind == FindingKind::RapidBelowClearance)
        .expect("the finding")
        .message;
    assert!(
        message.contains("program frame"),
        "the message does not name its frame: {message}"
    );

    // Above the stock top, below the retract plane: still an error.
    let low = validate_machine_safety(&program_with_rapid_at(plane - 0.1), safety(plane));
    assert_eq!(
        count_kind(&low, FindingKind::RapidBelowClearance),
        1,
        "a G0 XY move 0.1 mm under the retract plane must be an error"
    );

    // At the plane: clean.
    let at = validate_machine_safety(&program_with_rapid_at(plane), safety(plane));
    assert_eq!(
        count_kind(&at, FindingKind::RapidBelowClearance),
        0,
        "a G0 XY move at the retract plane is not an error"
    );
}

/// The 2D convention (stock top at world Z0) is unchanged: the plane is
/// the raw `post.safe_z`, so every existing 2D export reads as before.
#[test]
fn a_stock_top_at_world_zero_keeps_the_raw_post_safe_z() {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: STOCK_XY,
        y: STOCK_XY,
        z: 12.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let session = builder.build();
    let raw = session.post_config().safe_z;
    let plane = emission_frame_clearance_z(&session, std::iter::empty::<usize>());
    assert!(
        (plane - raw).abs() < EPS,
        "2D convention: plane {plane} should equal the raw post safe Z {raw}"
    );
}
