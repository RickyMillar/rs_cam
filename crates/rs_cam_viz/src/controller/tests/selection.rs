//! Selection cascade (E-sel): what a delete does to the selection.

use super::*;

#[test]
fn delete_selected_toolpath_clears_selection() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);
    controller.state.selection = Selection::Toolpath(tp_id);

    // Remove the selected toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared after deleting the selected toolpath"
    );
}

#[test]
fn delete_unselected_toolpath_preserves_selection() {
    let mut controller = sample_controller();

    // Add another toolpath
    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));
    let Selection::Toolpath(new_tp_id) = controller.state.selection else {
        panic!("Selection should be new toolpath");
    };

    // Select back to the original toolpath
    controller.state.selection = Selection::Toolpath(ToolpathId(0));

    // Delete the other toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(new_tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::Toolpath(ToolpathId(0)),
        "Selection should be preserved when a different toolpath is deleted"
    );
}

/// F-024 (third-site fix, 2026-05-25): regression test that the controller's
/// world `stock_bbox` helper respects `StockConfig::origin_{x,y,z}`.
///
/// Pre-fix (rounds 03/04 evidence): `build_simulation_groups` constructed the
/// world `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
/// dropping the origin. For AS001 (`origin_z=-12`) this sent a bbox of
/// `(0,0,0)..(100,100,12)` to the worker even though the actual world stock
/// spans `(-10,-10,-12)..(90,90,0)`. The F-024 viz-worker follow-up
/// (commit `1dd1aa7`) made the viz request forward
/// `local_stock_bbox = None` for identity setups so the core would fall back
/// to `request.stock_bbox` — but the fallback bbox itself was the same
/// broken bbox. Result: toolpath cuts at world Z=-2 still sat below every
/// dexel ray (which spanned the wrong Z=[0,12] instead of Z=[-12,0]),
/// `axial_engagement_mm` read the full ray length, and the deflection gate
/// stayed at ~374 µm — byte-identical to round-02.
///
/// Fix: route the world bbox construction through `ProjectSession::
/// stock_bbox()` (which delegates to `StockConfig::bbox()` and applies the
/// origin correctly). Exposed via the pure free function
/// `controller::events::simulation::build_world_stock_bbox` so this test can
/// exercise it without spinning up a full `AppController<B>`.
#[test]
fn build_world_stock_bbox_respects_stock_origin_f024() {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::material::{Material, WoodSpecies};
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    let session = ProjectSessionBuilder::new().stock(stock).build();

    let bbox: BoundingBox3 =
        crate::controller::events::simulation::build_world_stock_bbox(&session);

    assert!(
        (bbox.min.x - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.x must equal stock.origin_x; got {}",
        bbox.min.x
    );
    assert!(
        (bbox.min.y - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.y must equal stock.origin_y; got {}",
        bbox.min.y
    );
    assert!(
        (bbox.min.z - -12.0).abs() < 1e-9,
        "F-024: world stock bbox min.z must equal stock.origin_z (= -12 for \
         AS001); got {}. Pre-fix the controller built bbox.min.z = 0.0 and \
         the per-setup dexel grid spanned Z=[0, 12] instead of Z=[-12, 0], \
         so cuts at world Z=-2 sat below every ray and axial_engagement_mm \
         read the full stock height (12 mm) instead of the commanded DOC.",
        bbox.min.z
    );
    assert!(
        (bbox.max.x - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.x must equal origin_x + stock.x; got {}",
        bbox.max.x
    );
    assert!(
        (bbox.max.y - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.y must equal origin_y + stock.y; got {}",
        bbox.max.y
    );
    assert!(
        (bbox.max.z - 0.0).abs() < 1e-9,
        "F-024: world stock bbox max.z must equal origin_z + stock.z (= 0 for \
         AS001, stock top at world Z=0); got {}",
        bbox.max.z
    );
}
