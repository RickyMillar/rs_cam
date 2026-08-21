//! G-EXPORT-DATUM (2026-08-19) — every setup's G-code must express XY in
//! ONE frame, or a two-sided job hands the operator two different datums.
//!
//! ## The defect
//!
//! A setup with `local_to_global = None` (identity: `face_up = Top`,
//! `z_rotation = Deg0`) emits its toolpath in the WORLD frame. Every other
//! setup emits in the zero-rooted setup-local frame, whose first step is
//! `-stock_origin` (see `SetupTransformInfo::world_to_local`) — i.e. it is
//! stock-relative. When `StockConfig::origin_{x,y} != 0` the two frames
//! differ by exactly the origin, and export copied the stored toolpath
//! through verbatim.
//!
//! Measured on `planning/airrun_2026-08-19/wanaka200.toml` (stock
//! 240×250×25 at origin (-20,-25,-18); setup 1 `face_up = bottom`, setup 2
//! `face_up = top`), exported with `split_setups`:
//!
//! - setup 1's alignment-pin drill sits at `X2.500 Y2.500` / `X237.500
//!   Y247.500` — exactly `stock.alignment_pins`, which are dimensioned
//!   stock-relative. Stock-relative frame.
//! - setup 2's rough spans `X 2.82..200.25`, `Y 2.82..199.67` — the model
//!   at WORLD 0..200.
//!
//! Both files emit a plain `G54` with no `G10`/`G92` work offset, and the
//! only header warning was about Z. Keeping one XY zero across the flip —
//! which is what the alignment pins are for — machines the second side
//! 20 mm / 25 mm out of position.
//!
//! ## The fix
//!
//! `gcode::export_datum_shift_for_toolpath` + `toolpath_in_export_datum`
//! translate an identity setup's emitted coordinates by `-stock_bbox.min`
//! in XY at emit time. The stored toolpath is untouched, so the simulator,
//! viewport, screenshots and metrics are unaffected.
//!
//! **Z is deliberately not shifted** — see the module note on
//! `gcode::export_datum_shift_for_toolpath`. Shifting it would move program
//! Z0 to the stock's underside for every identity setup, breaking the
//! repo's 2D convention (`StockConfig::update_from_bbox` puts the stock TOP
//! at Z0 for 2D models so 2D ops cut at negative Z), and Z — unlike XY — is
//! explicitly re-zeroed between setups.
//!
//! ## Fixture
//!
//! Stock 60×70×12 at origin (-20,-25,-12). A 30×30 square model at world
//! XY [0,30]², traced (no offset) with a Ø6 end mill in
//! two setups: setup 0 identity (`Top`), setup 1 flipped (`Bottom`).
//!
//! In the shared stock-relative export frame the two setups' emitted
//! extents must be related by exactly the `Bottom` flip: X identical (the
//! flip preserves X) and Y mirrored about the stock's Y centre (`70 - y`).
//! That relation is what "the same physical point in the same coordinates"
//! means across a flip, and it is what the assertions below check.
//!
//! Measured with the fix disabled, the emitted X extents were
//! identity `-2.000..30.000` (WORLD) vs flipped `18.000..50.000`
//! (stock-relative) — a constant 20.000 mm apart, i.e. exactly
//! `-origin_x`. With the fix both read `18.000..50.000`.

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
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ToolpathConfig;
use rs_cam_core::session::ProjectSession;
use std::sync::atomic::AtomicBool;

const STOCK_X: f64 = 60.0;
const STOCK_Y: f64 = 70.0;
const STOCK_Z: f64 = 12.0;
const ORIGIN_X: f64 = -20.0;
const ORIGIN_Y: f64 = -25.0;
const ORIGIN_Z: f64 = -12.0;

const IDENTITY_LABEL: &str = "SideTop";
const FLIPPED_LABEL: &str = "SideBottom";

/// World-frame square spanning XY [0,30]² — deliberately NOT centred on the
/// stock, so the Y mirror across the flip is detectable.
fn square_model_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(30.0, 0.0),
        P2::new(30.0, 30.0),
        P2::new(0.0, 30.0),
    ])
}

/// Trace, not profile: trace follows the polygon with no offset, so its cut
/// EXTENT does not depend on which way the ring is wound. (An outside profile
/// does: the mirror reverses the winding and the offset flips inward — that
/// was G-PROFILE-FLIP, now fixed in `apply_to_polygons`.) Traversal DIRECTION
/// still changes with the winding, which is why `trace_toolpath` below takes
/// the lead dressup off — see its doc.
fn trace_op() -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: 4.0,
        depth_per_pass: 2.0,
        ..TraceConfig::default()
    })
}

/// `toolpath_config` with the lead-in/out dressup off.
///
/// ISOLATE THE VARIABLE. Trace inherits `lead_in_out: true` from the Finish
/// role, and a lead is a 2 mm tangential extension at the path's START — so
/// where it lands physically depends on which way the ring is traversed.
/// Until the G-PROFILE-FLIP winding fix, a `Bottom` flip left the mirrored
/// ring wound backwards, which put the flipped setup's lead at the mirror of
/// the identity setup's and made this sentry's Y assertion cancel by
/// coincidence. Normalising the winding (correctly — it is what stops an
/// outside profile becoming an inside one, and what stops `Shape::from_plines`
/// reading a flipped pocket exterior as a hole) moves the lead to the other
/// end of the square, and this sentry started failing on a 2 mm delta that has
/// nothing to do with the datum it exists to pin.
///
/// The datum is the subject here, so the lead comes off. With it off the
/// assertion compares cut geometry alone and is sharper, not weaker: identity
/// emits 25..55 and flipped 15..45, an exact mirror about `STOCK_Y`.
fn trace_toolpath(name: &str, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let mut tc = toolpath_config(name, trace_op(), tool_id, model_id);
    tc.dressups.lead_in_out = false;
    tc
}

/// Two setups over one stock with a non-zero XY origin: setup 0 identity,
/// setup 1 `face_up = Bottom`. Both carry the same trace of the
/// same model, so the only thing that can differ between their emitted
/// coordinates is the frame.
fn build_two_setup_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: ORIGIN_X,
        origin_y: ORIGIN_Y,
        origin_z: ORIGIN_Z,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model(vec![square_model_polygon()], "square30"));

    // Setup 0 is the identity setup `new_empty` already created.
    session
        .add_toolpath(
            0,
            trace_toolpath(IDENTITY_LABEL, tool_id, model_id),
        )
        .expect("add identity-setup trace");

    let flipped = session.add_setup("Flip".to_owned(), FaceUp::Bottom);
    session
        .add_toolpath(
            flipped,
            trace_toolpath(FLIPPED_LABEL, tool_id, model_id),
        )
        .expect("add flipped-setup trace");

    session
}

/// Generate both toolpaths and export one G-code program through the
/// production session entry point (`ProjectSession::export_gcode_*`, which
/// the CLI `project` subcommand and every headless export take).
fn export_two_setup_gcode() -> String {
    let mut session = build_two_setup_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate identity-setup trace");
    session
        .generate_toolpath(1, &cancel)
        .expect("generate flipped-setup trace");

    // Unique per call: the tests below run concurrently in one binary.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("rs_cam_export_datum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("two_setup_{seq}.nc"));
    session
        .export_gcode_with_policy(
            &path,
            rs_cam_core::gcode::ToolLoadExportPolicy {
                // No simulation is run here — the load gates would all read
                // `Unmodeled`. This test is about coordinates, not load.
                accept_unmodeled: true,
                accept_exceeded: true,
            },
        )
        .expect("export two-setup G-code");
    let gcode = std::fs::read_to_string(&path).expect("read exported G-code");
    let _ = std::fs::remove_file(&path);
    gcode
}

#[derive(Debug, Clone, Copy)]
struct Extents {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

/// XY extents of every motion line inside the phase whose label comment is
/// `label`, up to the next phase's label comment.
fn phase_extents(gcode: &str, label: &str, other_label: &str) -> Extents {
    let mut in_phase = false;
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    let mut seen = 0usize;

    for line in gcode.lines() {
        if line.contains(&format!("({label})")) {
            in_phase = true;
            continue;
        }
        if line.contains(&format!("({other_label})")) {
            in_phase = false;
            continue;
        }
        if !in_phase {
            continue;
        }
        let is_motion = line.starts_with("G0 ") || line.starts_with("G1 ");
        if !is_motion {
            continue;
        }
        let mut saw_xy = false;
        for word in line.split_whitespace() {
            let Some(rest) = word.strip_prefix('X').or_else(|| word.strip_prefix('Y')) else {
                continue;
            };
            let Ok(v) = rest.parse::<f64>() else { continue };
            if word.starts_with('X') {
                x_min = x_min.min(v);
                x_max = x_max.max(v);
            } else {
                y_min = y_min.min(v);
                y_max = y_max.max(v);
            }
            saw_xy = true;
        }
        if saw_xy {
            seen += 1;
        }
    }

    assert!(
        seen > 0,
        "phase '{label}' emitted no XY motion lines — fixture is vacuous"
    );
    Extents {
        x_min,
        x_max,
        y_min,
        y_max,
    }
}

/// Both setups' emitted XY must lie inside the stock box `[0, stock]`.
/// This is what "program zero is the stock's min corner" means, and it is
/// the assertion the operator's single G54 depends on.
#[test]
fn both_setups_emit_inside_the_stock_relative_box() {
    let gcode = export_two_setup_gcode();
    for (label, other) in [
        (IDENTITY_LABEL, FLIPPED_LABEL),
        (FLIPPED_LABEL, IDENTITY_LABEL),
    ] {
        let e = phase_extents(&gcode, label, other);
        assert!(
            e.x_min >= -1e-6 && e.x_max <= STOCK_X + 1e-6,
            "G-EXPORT-DATUM: setup phase '{label}' emitted X outside the \
             stock-relative box [0, {STOCK_X}]: {:.3}..{:.3}. Pre-fix the \
             identity setup emitted WORLD X (-2..30) while the flipped setup \
             emitted stock-relative X (18..50) — two datums under one G54.",
            e.x_min,
            e.x_max
        );
        assert!(
            e.y_min >= -1e-6 && e.y_max <= STOCK_Y + 1e-6,
            "G-EXPORT-DATUM: setup phase '{label}' emitted Y outside the \
             stock-relative box [0, {STOCK_Y}]: {:.3}..{:.3}",
            e.y_min,
            e.y_max
        );
    }
}

/// The cross-file registration assertion: one physical feature, expressed
/// consistently in both files. A `face_up = Bottom` flip preserves X and
/// mirrors Y about the stock's Y centre, so the identity setup's extents
/// must equal the flipped setup's extents under exactly that map.
#[test]
fn identity_and_flipped_setups_agree_on_one_physical_feature() {
    let gcode = export_two_setup_gcode();
    let top = phase_extents(&gcode, IDENTITY_LABEL, FLIPPED_LABEL);
    let bottom = phase_extents(&gcode, FLIPPED_LABEL, IDENTITY_LABEL);

    // Tolerance covers G-code decimal rounding (3 dp) only.
    let tol = 2e-3;

    assert!(
        (top.x_min - bottom.x_min).abs() < tol && (top.x_max - bottom.x_max).abs() < tol,
        "G-EXPORT-DATUM: a Bottom flip preserves X, so both setups must emit \
         the same X for the same physical feature. identity {:.3}..{:.3} vs \
         flipped {:.3}..{:.3} (delta {:.3} = stock origin_x). Pre-fix the \
         identity setup emitted world X and the flipped setup stock-relative \
         X, so an operator keeping one XY zero across the flip machined the \
         second side {} mm out.",
        top.x_min,
        top.x_max,
        bottom.x_min,
        bottom.x_max,
        bottom.x_min - top.x_min,
        -ORIGIN_X,
    );

    assert!(
        (top.y_min - (STOCK_Y - bottom.y_max)).abs() < tol
            && (top.y_max - (STOCK_Y - bottom.y_min)).abs() < tol,
        "G-EXPORT-DATUM: a Bottom flip mirrors Y about the stock's Y centre, \
         so identity Y must equal {STOCK_Y} - flipped Y. identity \
         {:.3}..{:.3} vs flipped {:.3}..{:.3} (mirror of flipped = \
         {:.3}..{:.3})",
        top.y_min,
        top.y_max,
        bottom.y_min,
        bottom.y_max,
        STOCK_Y - bottom.y_max,
        STOCK_Y - bottom.y_min,
    );

    // Non-vacuity: the fixture must actually sit off-centre in Y, or the
    // mirror assertion above would hold for the buggy output too.
    assert!(
        (top.y_min - bottom.y_min).abs() > 5.0,
        "fixture sanity: the traced square must be off-centre in Y so the \
         mirror is detectable; identity y_min {:.3} vs flipped y_min {:.3}",
        top.y_min,
        bottom.y_min
    );
}

/// The shift itself: XY only, and only for identity setups. This pins the
/// Z decision — a Z component here would move program Z0 to the stock's
/// underside for every identity setup and break the 2D `origin_z = -z`
/// convention (`StockConfig::update_from_bbox`).
#[test]
fn export_datum_shift_is_xy_only_and_identity_only() {
    let session = build_two_setup_session();

    let identity = rs_cam_core::gcode::export_datum_shift_for_toolpath(&session, 0);
    assert!(
        (identity.x - (-ORIGIN_X)).abs() < 1e-9 && (identity.y - (-ORIGIN_Y)).abs() < 1e-9,
        "identity setup must shift by -stock_bbox.min in XY; got {identity:?}"
    );
    assert!(
        identity.z.abs() < 1e-9,
        "Z is deliberately NOT shifted (see gcode::export_datum_shift_for_toolpath): \
         shifting it would move program Z0 to the stock underside for every \
         identity setup and break the 2D stock-top-at-Z0 convention; got \
         z = {}",
        identity.z
    );

    let flipped = rs_cam_core::gcode::export_datum_shift_for_toolpath(&session, 1);
    assert!(
        flipped.x.abs() < 1e-9 && flipped.y.abs() < 1e-9 && flipped.z.abs() < 1e-9,
        "non-identity setups already emit stock-relative — shift must be zero; \
         got {flipped:?}"
    );
}

/// No behavioural change for the overwhelmingly common case: a project whose
/// stock origin is already at the world origin emits byte-identical G-code.
#[test]
fn zero_origin_stock_is_unchanged() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model(vec![square_model_polygon()], "square30"));
    session
        .add_toolpath(
            0,
            trace_toolpath(IDENTITY_LABEL, tool_id, model_id),
        )
        .expect("add identity-setup trace");

    let shift = rs_cam_core::gcode::export_datum_shift_for_toolpath(&session, 0);
    assert!(
        shift.x == 0.0 && shift.y == 0.0 && shift.z == 0.0,
        "zero-origin stock must produce a zero shift (no emitted-G-code change \
         for the common case); got {shift:?}"
    );
}
