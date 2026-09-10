//! **G-RELOADTARGETS** (F4.4) — a model refresh replaces the geometry and
//! KEEPS the previous file's drill targets.
//!
//! # The mechanism, stated exactly
//!
//! The programme's follow-on row said `reload_model` "drops `drill_targets`
//! and `layers`, so reloading a drawing whose holes MOVED keeps the previous
//! version's targets". That sentence names two different outcomes. Only one
//! of them happens.
//!
//! `AppController::reload_model` (`controller/io.rs:133-145`) re-imports the
//! file through the interactive door and then assigns FIVE fields onto the
//! stored record: `mesh`, `polygons`, `enriched_mesh`, `winding_report`,
//! `load_error`. It never assigns `drill_targets` and never assigns
//! `layers`. Nothing writes an empty list. The `Arc<Vec<DrillTarget>>` the
//! PREVIOUS import put there simply survives, beside the new polygons.
//!
//! So the outcome is (b): the targets are STALE, at the previous file's
//! coordinates. They are not emptied.
//!
//! # Why that is a wrong cut and not a display defect
//!
//! A drill operation reads the model record at generation time —
//! `session/compute.rs:1393` clones the record's `drill_targets` into the
//! request, and `compute/execute.rs:1033` turns them into holes. So the
//! machine drills the OLD positions from a drawing that no longer contains
//! them. The operator moved a hole in CAD, pressed Reload, watched the
//! outline move on screen, and got the previous version's holes.
//!
//! # This is NOT a fourth divergence in the loader PAIR
//!
//! The standing lesson (`CLAUDE.md`) is that `io::load_model_file` and
//! `session::project_file::load_model_geometry` must agree. I read both.
//! They agree about drill targets: the DXF arms both scale the targets
//! (`io.rs:85-88`, `project_file.rs:639-643`) and the SVG arms both
//! classify circle-like rings AFTER the unit scale (`io.rs:64-68`,
//! `project_file.rs:658-662`). `project_file.rs:661` carries a comment
//! saying so and names G-UNITSRELOAD.
//!
//! The divergence is in a DIFFERENT family: the three viz doors that
//! refresh a model record in place, each hand-copying its own subset of a
//! core type's fields.
//!
//! | door | `controller/io.rs` | carries targets and layers |
//! |---|---|---|
//! | `rescale_model` | `:100-103` | **no** |
//! | `reload_model` | `:140-144` | **no** |
//! | `relink_model` | `:236-245` | yes |
//!
//! `relink_model` is the youngest of the three (F4.3, G-MODELRELINK) and its
//! own comment names the gap: "Unlike `reload_model`, these two move as
//! well". It is the control in this file: it must stay green.
//!
//! # What is pinned
//!
//! Agreement, not a coordinate. After a refresh, the stored record must
//! carry what a FRESH import of the file it is now bound to carries. A test
//! asserting "the target is at x = 70" would pass just as well if both sides
//! were wrong together, and it would have to be rewritten every time the
//! fixture moved. This is the shape
//! `model_units_survive_reload_g_unitsreload.rs` uses for the loader pair.
//!
//! Source: `planning/ui_fix_2026-09-09/reports/F4.3.md` §9 row F4.4, and
//! `research/R0.7.md` §3.3 (the "Apply" and "Reload / Rescale" rows).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::stock_config::{ModelId, ModelKind, ModelUnits};
use rs_cam_core::io::load_model_file;
use rs_cam_core::session::LoadedModel;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;

// ── harness ─────────────────────────────────────────────────────────────

struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

/// A unique scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_reloadtargets_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// One circle-like ring at `(cx, cy)`, radius 10.
///
/// Radius 10 is chosen so the classifier cannot be the thing that fails:
/// `circle_like_ring` needs at least 8 vertices and a radial spread inside
/// `max(0.02 r, 0.05 mm)`, and a bezier circle of this size flattened at the
/// import tolerance clears both by a wide margin.
fn drawing_with_hole(cx: f64, cy: f64) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"120\">\
         <circle cx=\"{cx}\" cy=\"{cy}\" r=\"10\"/></svg>"
    )
}

/// The same drawing with the hole DELETED — a rectangle and nothing else.
fn drawing_with_no_hole() -> String {
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"120\">\
     <rect x=\"10\" y=\"10\" width=\"160\" height=\"90\"/></svg>"
        .to_owned()
}

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write drawing");
}

/// What a fresh import of `path` at `units` reports.
fn fresh(path: &Path, units: ModelUnits) -> LoadedModel {
    load_model_file(path, 0, ModelKind::Svg, units).expect("the interactive door must import")
}

/// A comparable description of one target. `DrillTarget` derives neither
/// `PartialEq` nor `Eq`, so the fields are named here rather than derived.
fn describe(model: &LoadedModel) -> Vec<(f64, f64, String, String)> {
    model
        .drill_targets
        .iter()
        .map(|t| {
            (
                (t.x * 1e6).round() / 1e6,
                (t.y * 1e6).round() / 1e6,
                t.layer.clone(),
                format!("{:?}", t.kind),
            )
        })
        .collect()
}

/// Import drawing A, then return the controller and the id the session
/// assigned. Every test starts here.
///
/// The model is found by its PATH, not by position. `ProjectSession::
/// new_empty` seeds no models today, so slot 0 would be right — but a
/// seeded demo project would then silently break the control below, and
/// the control is the reason the other assertions can be believed.
fn seeded(dir: &Path) -> (AppController<SilentBackend>, PathBuf, ModelId) {
    let path = dir.join("part.svg");
    write(&path, &drawing_with_hole(30.0, 40.0));
    let mut controller = AppController::with_backend(SilentBackend);
    controller
        .import_svg_path(&path)
        .expect("the fixture must import");
    let id = controller
        .state
        .session
        .models()
        .iter()
        .find(|m| m.path == path)
        .expect("the imported fixture must be in the session")
        .id;
    (controller, path, ModelId(id))
}

/// The record the session holds for `id`.
fn record(controller: &AppController<SilentBackend>, id: ModelId) -> &LoadedModel {
    controller
        .state
        .session
        .models()
        .iter()
        .find(|m| m.id == id.0)
        .expect("the model must still be in the session")
}

// ── non-vacuity ─────────────────────────────────────────────────────────

/// Without this, every assertion below could be a comparison of two empty
/// lists. It pins that the fixture drawings really do carry a target, that
/// the target really does move between A and B, and that deleting the
/// circle really does remove it.
#[test]
fn the_three_fixture_drawings_report_the_targets_this_file_assumes() {
    let dir = scratch("vacuity");
    let a = dir.join("a.svg");
    let b = dir.join("b.svg");
    let c = dir.join("c.svg");
    write(&a, &drawing_with_hole(30.0, 40.0));
    write(&b, &drawing_with_hole(70.0, 40.0));
    write(&c, &drawing_with_no_hole());

    let fa = fresh(&a, ModelUnits::Millimeters);
    let fb = fresh(&b, ModelUnits::Millimeters);
    let fc = fresh(&c, ModelUnits::Millimeters);

    assert_eq!(
        fa.drill_targets.len(),
        1,
        "drawing A must carry exactly one drill target, got {:?}",
        describe(&fa)
    );
    assert_eq!(
        fb.drill_targets.len(),
        1,
        "drawing B must carry exactly one drill target, got {:?}",
        describe(&fb)
    );
    assert!(
        fc.drill_targets.is_empty(),
        "drawing C has no circle, so it must carry no drill target, got {:?}",
        describe(&fc)
    );
    assert!(
        (fa.drill_targets[0].x - fb.drill_targets[0].x).abs() > 1.0,
        "the hole must MOVE between A and B, else the reload test is vacuous"
    );
    assert!(
        !fa.layers.is_empty(),
        "a drawing with a target must name the layer it is on"
    );
    assert!(
        fc.layers.is_empty(),
        "a drawing with no target must name no layer"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── the defect ──────────────────────────────────────────────────────────

/// **The operator's case.** The hole moved in CAD. Reload from disk.
///
/// Pre-fix the record keeps drawing A's target while its polygons are
/// drawing B's, and a drill operation reads the record — so the program
/// drills a hole that is not in the file.
#[test]
fn a_reloaded_drawing_carries_the_new_files_drill_targets() {
    let dir = scratch("moved");
    let (mut controller, path, id) = seeded(&dir);

    // The same path, different contents — this is what "Reload from disk"
    // is for.
    write(&path, &drawing_with_hole(70.0, 40.0));
    controller.reload_model(id).expect("reload must succeed");

    let stored = record(&controller, id);
    let expected = fresh(&path, ModelUnits::Millimeters);

    assert_eq!(
        describe(stored),
        describe(&expected),
        "after Reload the record must carry the drill targets of the file \
         on disk. `reload_model` assigns mesh, polygons, enriched_mesh, \
         winding_report and load_error, and never assigns drill_targets, so \
         the PREVIOUS import's targets survive beside the new polygons. A \
         drill operation reads the record at generation time, so these are \
         the holes the machine cuts."
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The worse shape of the same defect.** The operator DELETED the holes
/// from the drawing. The record still carries them, so the program drills
/// holes the file does not contain at all — and the layer list still names
/// a layer that no longer has anything on it.
#[test]
fn a_reloaded_drawing_that_lost_its_holes_carries_no_targets() {
    let dir = scratch("deleted");
    let (mut controller, path, id) = seeded(&dir);

    write(&path, &drawing_with_no_hole());
    controller.reload_model(id).expect("reload must succeed");

    let stored = record(&controller, id);

    assert!(
        stored.drill_targets.is_empty(),
        "the drawing no longer has a circle, so the record must carry no \
         drill target — it carries {:?}",
        describe(stored)
    );
    assert!(
        stored.layers.is_empty(),
        "and no layer, because the layer list is derived from the targets — \
         it carries {:?}",
        stored.layers
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The same hole in the units door.** A rescale re-imports the file at new
/// declared units, so the polygons move by the scale factor. The targets do
/// not, because `rescale_model` assigns four fields and `drill_targets` is
/// not one of them. The drawn circle and the hole the machine cuts then sit
/// in different places.
#[test]
fn a_rescaled_model_carries_rescaled_drill_targets() {
    let dir = scratch("rescale");
    let (mut controller, path, id) = seeded(&dir);

    controller
        .rescale_model(id, ModelUnits::Inches)
        .expect("rescale must succeed");

    let stored = record(&controller, id);
    let expected = fresh(&path, ModelUnits::Inches);

    assert_eq!(
        describe(stored),
        describe(&expected),
        "a rescale moves the polygons by the unit scale, so it must move \
         the drill targets by the same scale. `rescale_model` assigns mesh, \
         polygons, units and winding_report only."
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── the control ─────────────────────────────────────────────────────────

/// **This one passes before the fix**, and it is in the file to prove the
/// assertion above is capable of passing. `relink_model` (F4.3) moves
/// `drill_targets` and `layers` across; the other two doors do not. If this
/// test ever goes red the shared path has been broken, not extended.
#[test]
fn a_relinked_model_carries_the_new_files_drill_targets_control() {
    let dir = scratch("relink");
    let (mut controller, _path, id) = seeded(&dir);

    let other = dir.join("other.svg");
    write(&other, &drawing_with_hole(120.0, 60.0));
    controller
        .relink_model(id, &other)
        .expect("relink must succeed");

    let stored = record(&controller, id);
    let expected = fresh(&other, ModelUnits::Millimeters);

    assert_eq!(
        describe(stored),
        describe(&expected),
        "the relink door already carries targets — this is the control"
    );
    assert_eq!(
        stored.path, other,
        "and the record is bound to the new file"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
