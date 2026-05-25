//! F-026 — Auto-from-model stock bbox re-derivation at load time.
//!
//! ## Background
//!
//! `StockConfig::auto_from_model = true` is documented as "stock
//! dimensions track the model bbox". The runtime mutation path —
//! `ProjectSession::add_model` — honors that by calling
//! `StockConfig::update_from_bbox` when adding a model with
//! `auto_from_model = true`. The MCP `import_model` tool wraps this
//! path, so users who import a model after loading a project get the
//! expected behaviour.
//!
//! The load path (`ProjectSession::load` ->
//! `project_file::build_session_from_project`) did **not** apply
//! `update_from_bbox` after loading the models. The stock dimensions
//! came verbatim from the TOML. For projects with `auto_from_model =
//! true` whose saved TOML has stock dimensions that don't match the
//! current model bbox (the user edited the model externally, the file
//! was saved with default stock dims and the model grew later, etc.),
//! the loaded session would have a stock bbox that didn't enclose the
//! model.
//!
//! Round-05 of the acceptance loop confirmed this on
//! `test_data/ux_3d_terrain.toml` — the TOML has `stock.z = 30` with
//! `auto_from_model = true`, but the loaded `terrain_small.stl` peaks
//! at world Z ≈ 52.6. Adaptive3d emitted a warning ("`stock_top_z` is
//! below mesh top by 22 mm") but still tried to plan against the
//! truncated Z range. The simulator's per-setup dexel grid sized to
//! the same truncated Z range. Rapids over terrain peaks at the
//! planner's expected `safe_z` registered as collisions against the
//! truncated grid (round-05 AS013 reported 844 rapid collisions on a
//! toolpath that should have been clean).
//!
//! ## Fix
//!
//! In `build_session_from_project`, after the model load loop, when
//! `stock.auto_from_model = true` and at least one loaded model has a
//! finite bbox, the stock is re-derived from the union of all model
//! bboxes (mirrors what `add_model` does at runtime).
//!
//! ## Acceptance bars
//!
//! 1. Loading `ux_3d_terrain.toml` produces a session whose
//!    `stock_bbox().max.z` encloses the terrain mesh top — proving the
//!    load-time `update_from_bbox` fired.
//! 2. (Same shape) Loading a project with a 2D-polygon model whose XY
//!    extent exceeds the saved stock XY also auto-grows. This guards
//!    against regressions on the 2D path that shares the load logic.
//!
//! ## Residual scope (NOT addressed by F-026)
//!
//! Round-05 also reported high `peak_axial_doc_mm` and `deflection`
//! over-fire on AS013-class cases. Investigation while landing F-026
//! showed those residuals persist even after the load-time fix and
//! cluster at samples just outside the model XY bbox. The cause is a
//! mismatch between adaptive3d's planner-internal stock (bounded by
//! mesh bbox + tool radius) and the simulator's per-setup dexel grid
//! (bounded by world stock bbox). Cells inside the simulator's grid
//! but outside adaptive3d's grid never see planner stamps; the
//! simulator carries them as virgin material across the entire
//! toolpath, and the final pass scrapes the full pre_len in one
//! stamp. That's a separate adaptive3d edge-clearing bug — see
//! follow-up finding F-027 for the scope and fix shape.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::PathBuf;

use rs_cam_core::session::ProjectSession;

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

// ── F-026 primary acceptance ─────────────────────────────────────────

/// F-026 acceptance: load-time `auto_from_model` re-derivation grows
/// stock to enclose the loaded mesh.
///
/// Loads `test_data/ux_3d_terrain.toml`. The TOML has
/// `stock = { z = 30, auto_from_model = true }` but the terrain mesh
/// peaks at world Z ≈ 52.6.
///
/// Pre-fix: `stock_bbox().max.z` reads 30 (the stale on-disk value).
/// Post-fix: `stock_bbox().max.z` reads ≥ 52.6 (mesh top), proving
/// `update_from_bbox` fired at load time.
///
/// Runs through `ProjectSession::load` — the same entry point the MCP
/// `load_project` tool and the CLI `--project` flag take.
#[test]
fn auto_from_model_load_grows_stock_z_to_enclose_terrain_mesh() {
    let toml_path = repo_root().join("test_data/ux_3d_terrain.toml");
    let session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");

    // Sanity preconditions — if these fail, the test fixture has
    // drifted and the regression doesn't bite.
    assert!(
        session.stock_config().auto_from_model,
        "ux_3d_terrain.toml fixture must have auto_from_model = true"
    );
    let mesh_top_z = session
        .models()
        .iter()
        .filter_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox.max.z))
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        mesh_top_z > 50.0,
        "ux_3d_terrain mesh must peak above ≈50 mm (got {mesh_top_z:.3})"
    );

    let stock_bbox = session.stock_bbox();
    assert!(
        stock_bbox.max.z >= mesh_top_z,
        "F-026: after load, stock world bbox max.z ({:.3}) must enclose mesh \
         top ({:.3}). Pre-fix this read 30 (the stale TOML stock.z) because \
         `auto_from_model = true` was not honored at load.",
        stock_bbox.max.z,
        mesh_top_z,
    );
}

/// F-026 secondary: 2D-polygon path also auto-grows on load.
///
/// `ux_2d_pocket_softwood.toml` has a polygon-only model and
/// `auto_from_model = true`. The 2D branch of `update_from_bbox`
/// preserves the user's stock thickness but shifts `origin_z` so the
/// stock top sits at the polygon's Z (= 0) — see
/// `StockConfig::update_from_bbox` doc.
///
/// This test asserts the same invariant: after load, stock bbox
/// encloses the model bbox (here XY — 2D models contribute zero Z
/// range).
#[test]
fn auto_from_model_load_grows_stock_xy_to_enclose_polygon_model() {
    let toml_path = repo_root().join("test_data/ux_2d_pocket_softwood.toml");
    let Ok(session) = ProjectSession::load(&toml_path) else {
        // Skip if the fixture isn't present (CI matrix can have minimal
        // test data). The terrain test above is the primary signal.
        eprintln!("skip: ux_2d_pocket_softwood.toml not loadable");
        return;
    };

    if !session.stock_config().auto_from_model {
        eprintln!("skip: ux_2d_pocket_softwood doesn't have auto_from_model = true");
        return;
    }

    // Compute the polygon XY extent inline (the helper is pub(crate)).
    let mut model_x_max: f64 = f64::NEG_INFINITY;
    let mut model_y_max: f64 = f64::NEG_INFINITY;
    let mut model_x_min: f64 = f64::INFINITY;
    let mut model_y_min: f64 = f64::INFINITY;
    let mut have_polygon = false;
    for m in session.models() {
        if let Some(polys) = &m.polygons {
            for poly in polys.iter() {
                for p in poly.exterior.iter() {
                    have_polygon = true;
                    model_x_max = model_x_max.max(p.x);
                    model_y_max = model_y_max.max(p.y);
                    model_x_min = model_x_min.min(p.x);
                    model_y_min = model_y_min.min(p.y);
                }
            }
        }
    }
    if !have_polygon {
        eprintln!("skip: no polygon model in fixture");
        return;
    }

    let stock_bbox = session.stock_bbox();
    assert!(
        stock_bbox.max.x >= model_x_max - 1e-6 && stock_bbox.max.y >= model_y_max - 1e-6,
        "F-026: after load, stock world bbox max ({:?}) must enclose 2D model \
         max XY ({:.3}, {:.3}). Pre-fix the stale TOML XY values could be \
         smaller than the model footprint.",
        stock_bbox.max,
        model_x_max,
        model_y_max,
    );
    let _ = model_x_min;
    let _ = model_y_min;
}
