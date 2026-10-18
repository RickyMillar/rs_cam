//! P7 — the one pin report, the wrong-orientation ghost and the stored
//! part-model choice.
//!
//! The stock panel chips, the pin table, the viewport colours, the ghost
//! and the MCP `pin_report` read `ProjectSession::pin_set_report`. These
//! arms pin what that report says on the mini test piece's shape
//! (`planning/rivmap_block_job/testpiece_mini.toml`: a 65 x 65 stock, two
//! 6 mm pins on the flip axis y = 32.5):
//!
//! 1. the two pins seat and do not key; the ghost (the holes after the
//!    closest wrong seating, a half turn) lands on the real pins, which is
//!    what the operator sees in the viewport;
//! 2. every pin of a set that does not key has the `Fails` state;
//! 3. `SetPinPartModels` changes the walls and stales nothing;
//! 4. the choice survives a save and a load.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::sync::Arc;

use rs_cam_core::compute::alignment_pins::{AlignmentPin, closest_wrong_seating_images};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, PinStatus, ProjectSession, ProjectSessionBuilder, SetPinPartModelsArgs,
};

fn square_model(id: usize, name: &str, lo: f64, hi: f64) -> LoadedModel {
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::new(vec![
                P2::new(lo, lo),
                P2::new(hi, lo),
                P2::new(hi, hi),
                P2::new(lo, hi),
            ]),
        ])),
        path: std::path::PathBuf::from(name),
        kind: None,
        enriched_mesh: None,
        units: None,
        winding_report: None,
        load_error: None,
    }
}

fn mini() -> ProjectSession {
    let stock = StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        alignment_pins: vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ],
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .model(square_model(0, "stock_outline.dxf", 0.0, 65.0))
        .model(square_model(1, "back_cavity.dxf", 12.0, 53.0));
    let _ = builder.add_setup("Front".to_owned(), FaceUp::Top);
    let _ = builder.add_setup("Back".to_owned(), FaceUp::Bottom);
    builder.build()
}

#[test]
fn the_mini_pins_seat_do_not_key_and_the_ghost_lands_on_them() {
    let session = mini();
    let report = session.pin_set_report();
    assert!(report.judged_setup.is_some(), "the Bottom setup is judged");
    assert!(report.flip.seats(), "{:?}", report.flip);
    assert!(!report.flip.keyed(), "{:?}", report.flip);
    // The ghost: a half turn about the midpoint swaps the two holes, so
    // each ghost hole is on the OTHER real dowel.
    let pins = &session.stock_config().alignment_pins;
    assert_eq!(report.wrong_seating_images.len(), 2);
    for (ghost, other) in report.wrong_seating_images.iter().zip(pins.iter().rev()) {
        assert!(
            (ghost.0 - other.x).hypot(ghost.1 - other.y) < 1e-6,
            "ghost {ghost:?} is not on pin {other:?}"
        );
    }
    // The report reads the same function the viewport would call.
    let (_, direct) = closest_wrong_seating_images(pins).unwrap();
    assert_eq!(direct, report.wrong_seating_images);
    for i in 0..2 {
        assert_eq!(report.pin_status(i), PinStatus::Fails);
        assert!(report.seat_miss_mm[i] < 1e-9, "{:?}", report.seat_miss_mm);
    }
}

#[test]
fn a_keyed_set_has_no_failing_pin_and_reads_its_preset() {
    let mut session = mini();
    let pins = vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(6.0, 6.0, 6.0),
        AlignmentPin::new(6.0, 59.0, 6.0),
    ];
    let _ = session
        .apply(Command::SetAlignmentPins(
            rs_cam_core::session::SetAlignmentPinsArgs { pins },
        ))
        .unwrap();
    let report = session.pin_set_report();
    assert!(report.flip.keyed(), "{:?}", report.flip);
    assert!(matches!(
        report.preset,
        Some(rs_cam_core::session::PinPreset::AxisPlusMirrorPair(_))
    ));
    for i in 0..3 {
        assert_ne!(report.pin_status(i), PinStatus::Fails, "pin {i}");
    }
}

#[test]
fn the_part_model_choice_moves_the_walls_and_stales_nothing() {
    let mut session = mini();
    // The default: the outline covers the stock and is not part.
    assert_eq!(session.pin_part_models(), vec![ModelId(1)]);
    let default_part = session.pin_set_report().wall_to_part_mm();
    assert!(default_part.is_some());

    let effects = session
        .apply(Command::SetPinPartModels(SetPinPartModelsArgs {
            models: Some(Vec::new()),
        }))
        .unwrap();
    assert!(effects.stale.is_empty(), "{:?}", effects.stale);
    assert!(session.pin_part_models().is_empty());
    let report = session.pin_set_report();
    assert_eq!(report.wall_to_part_mm(), None, "no model is part now");
    assert!(report.part_models_chosen);

    let _ = session
        .apply(Command::SetPinPartModels(SetPinPartModelsArgs {
            models: None,
        }))
        .unwrap();
    assert_eq!(session.pin_part_models(), vec![ModelId(1)]);
    assert_eq!(session.pin_set_report().wall_to_part_mm(), default_part);
}

#[test]
fn the_part_model_choice_survives_a_save_and_a_load() {
    let mut session = mini();
    let _ = session
        .apply(Command::SetPinPartModels(SetPinPartModelsArgs {
            models: Some(vec![0, 1]),
        }))
        .unwrap();
    let dir = std::env::temp_dir().join(format!("rs_cam_pin_parts_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pins.toml");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("pin_part_models"), "{text}");
    let loaded = ProjectSession::load(&path).unwrap();
    assert_eq!(
        loaded.stock_config().pin_part_models,
        Some(vec![0, 1]),
        "the choice did not round-trip"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
