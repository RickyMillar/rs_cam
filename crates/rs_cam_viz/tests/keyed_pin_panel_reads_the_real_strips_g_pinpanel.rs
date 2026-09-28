//! G-PINPANEL: the stock panel's "Auto-place keyed pair" reads the real
//! clear strips, not the stock padding.
//!
//! # What the finding was
//!
//! The panel planned the pins with `model_x_range = (padding, x - padding)`.
//! The core placer's own doc (`place_keyed_pins`, step 2) says padding
//! describes how the stock would be auto-sized, not how big it is. The live
//! stock was 380 mm wide at origin x -15 around a model at x 0..350, with a
//! padding of 5. The real strips are 15 mm each. The panel said "the clear
//! strip on -X is 5.0 mm" and greyed the button, while the "Two-sided
//! setup" route placed the pair.
//!
//! # What this test measures
//!
//! `ui::properties::stock::keyed_pin_plan` is the one planner both doors
//! call, with `KeyedPinInputs::from_session`. The arms:
//!
//! 1. the live 380 / -15 / 0..350 stock places pins at x 9.0 and 374.0 on
//!    the mirror line y = 255 with a 6 mm pin-drill tool, although the
//!    padding is 5;
//! 2. a 359 mm stock at origin x -5 around the same model has 5 mm and
//!    4 mm strips, and the planner refuses on the 5.0 mm -X strip;
//! 3. `KeyedPinInputs::from_session` reads the union bbox of all models
//!    and the pin-drill tool diameter.
//!
//! # NOT MEASURED
//!
//! The greyed button and the refusal label, which need an egui frame.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::alignment_pins::{PinPlacementError, PinSide};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_viz::ui::properties::stock::{KeyedPinInputs, KeyedPinRefusal, keyed_pin_plan};

const PIN_DIAMETER_MM: f64 = 6.0;
const STOCK_D_MM: f64 = 510.0;

/// The live model footprint: x 0..350.
fn model_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(350.0, 480.0, 20.0),
    }
}

/// A hand-sized stock with padding 5, which does not describe it.
fn stock(width: f64, origin_x: f64) -> StockConfig {
    StockConfig {
        x: width,
        y: STOCK_D_MM,
        z: 26.0,
        origin_x,
        origin_y: -15.0,
        padding: 5.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn inputs() -> KeyedPinInputs {
    KeyedPinInputs {
        model_bbox: Some(model_bbox()),
        pin_diameter: Some(PIN_DIAMETER_MM),
    }
}

#[test]
fn the_live_stock_places_the_pair_in_its_15mm_strips() {
    let pins = keyed_pin_plan(&stock(380.0, -15.0), FaceUp::Bottom, &inputs())
        .unwrap_or_else(|e| panic!("the real strips are 15 mm; got the refusal {e}"));
    assert!((pins[0].x - 9.0).abs() < 1e-9, "pin 0 at x {}", pins[0].x);
    assert!((pins[1].x - 374.0).abs() < 1e-9, "pin 1 at x {}", pins[1].x);
    for pin in &pins {
        assert!(
            (pin.y - STOCK_D_MM * 0.5).abs() < 1e-9,
            "pin at y {}",
            pin.y
        );
        assert!((pin.diameter - PIN_DIAMETER_MM).abs() < 1e-9);
    }
}

#[test]
fn a_5mm_and_4mm_strip_stock_is_refused_on_the_5mm_side() {
    match keyed_pin_plan(&stock(359.0, -5.0), FaceUp::Bottom, &inputs()) {
        Err(KeyedPinRefusal::Placement(PinPlacementError::StripTooNarrow {
            side,
            strip_mm,
            required_mm,
            ..
        })) => {
            assert_eq!(side, PinSide::MinusX);
            assert!((strip_mm - 5.0).abs() < 1e-9, "strip {strip_mm}");
            assert!((required_mm - 10.0).abs() < 1e-9, "required {required_mm}");
        }
        other => panic!("expected the 5.0 mm -X strip refusal, got {other:?}"),
    }
}

#[test]
fn the_inputs_come_from_all_models_and_the_pin_drill_tool() {
    use rs_cam_core::geo::P2;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
    use std::sync::Arc;

    let square = |x0: f64, x1: f64| {
        Polygon2::new(vec![
            P2::new(x0, 0.0),
            P2::new(x1, 0.0),
            P2::new(x1, 100.0),
            P2::new(x0, 100.0),
        ])
    };
    let model = |id: usize, poly: Polygon2| LoadedModel {
        id,
        name: format!("m{id}"),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(format!("m{id}.dxf")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let session = ProjectSessionBuilder::new()
        .model(model(1, square(0.0, 100.0)))
        .model(model(2, square(250.0, 350.0)))
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill))
        .build();

    let got = KeyedPinInputs::from_session(&session);
    let bbox = got.model_bbox.expect("two models carry geometry");
    assert!((bbox.min.x - 0.0).abs() < 1e-9, "min x {}", bbox.min.x);
    assert!(
        (bbox.max.x - 350.0).abs() < 1e-9,
        "the union reaches the second model; max x {}",
        bbox.max.x
    );
    let tool_d = session.tools()[0].diameter;
    assert_eq!(got.pin_diameter, Some(tool_d));
}
