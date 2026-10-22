//! P10 (2026-10-12): the per-setup G-code header and the setup sheet list
//! the dowels.
//!
//! Plan: `planning/alignment_pins_review_2026-10-12/REVIEW.md` row P10.
//! When the stock has alignment pins and a setup flips the part:
//!
//! - every file lists the dowels (number, X, Y, diameter), stock-local;
//! - the file of the setup that drills the dowel holes says so;
//! - the flipped file says "Seat the part on dowels 1-N" in its turn
//!   message and gives one orientation cue from the geometry;
//! - a pattern that does not key gets one warning line.
//!
//! A project with no pins, or with no flipped setup, gets no dowel line.
//! The test calls `setup_file_header` directly: the header reads only the
//! session, so the toolpaths need no generated result.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

use rs_cam_core::compute::alignment_pins::{AlignmentPin, NOT_KEYED_WARNING};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::AlignmentPinDrillConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::gcode::PostFormat;
use rs_cam_core::session::{
    AddSetupArgs, AddToolpathArgs, Command, ProjectSession, SetStockConfigArgs, ToolpathConfig,
};
use rs_cam_viz::io::export::setup_file_header;
use rs_cam_viz::io::setup_sheet::generate_setup_sheet_from_session;
use rs_cam_viz::state::runtime::GuiState;

/// The 3-pin pattern of the mini test piece: one pin on the flip axis
/// (y = 32.5) and a mirror pair off the axis.
fn three_pins() -> Vec<AlignmentPin> {
    vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(7.5, 9.0, 6.0),
        AlignmentPin::new(7.5, 56.0, 6.0),
    ]
}

fn pin_drill_toolpath() -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(7),
        name: "Pin Drill".to_owned(),
        enabled: true,
        operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(0),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

/// A 65 x 65 stock with `pins`, setup 1 `Top` with a pin-drill operation,
/// and, when `flip` is set, setup 2 `Bottom`.
fn session_with(pins: Vec<AlignmentPin>, flip: bool) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let _ = session
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(StockConfig {
                x: 65.0,
                y: 65.0,
                z: 12.0,
                origin_x: 0.0,
                origin_y: 0.0,
                origin_z: -12.0,
                auto_from_model: false,
                alignment_pins: pins,
                ..StockConfig::default()
            }),
        }))
        .expect("the stock applies");
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(pin_drill_toolpath()),
        }))
        .expect("the pin drill joins setup 1");
    let face_up = if flip { FaceUp::Bottom } else { FaceUp::Top };
    let _ = session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Bottom".to_owned()),
            face_up,
        }))
        .expect("a second setup");
    session
}

/// The two headers (setup 1, setup 2) of `session` on `format`.
fn headers(session: &ProjectSession, format: PostFormat) -> (String, String) {
    let post = format.definition();
    let ids: Vec<usize> = session.list_setups().iter().map(|s| s.id.raw()).collect();
    (
        setup_file_header(
            session,
            post,
            rs_cam_core::ids::SetupId::from_raw(ids[0]),
            None,
            0,
            2,
        ),
        setup_file_header(
            session,
            post,
            rs_cam_core::ids::SetupId::from_raw(ids[1]),
            Some(rs_cam_core::ids::SetupId::from_raw(ids[0])),
            1,
            2,
        ),
    )
}

#[test]
fn a_keyed_three_pin_flip_lists_the_dowels_the_drill_file_and_the_cue() {
    let session = session_with(three_pins(), true);
    let (first, second) = headers(&session, PostFormat::GrblHal);
    for header in [&first, &second] {
        assert!(
            header.contains("(DOWELS: 3 in the spoilboard, X Y from X0 Y0, diameter)\n"),
            "{header}"
        );
        assert!(
            header.contains("(Dowel 1: X 59.00 Y 32.50, 6.00 mm)\n"),
            "{header}"
        );
        assert!(
            header.contains("(Dowel 2: X 7.50 Y 9.00, 6.00 mm)\n"),
            "{header}"
        );
        assert!(
            header.contains("(Dowel 3: X 7.50 Y 56.00, 6.00 mm)\n"),
            "{header}"
        );
        assert!(!header.contains("WARNING"), "{header}");
    }
    assert!(
        first.contains("(This file drills the dowel holes 1-3.)\n"),
        "{first}"
    );
    assert!(!first.contains("Seat the part"), "{first}");
    assert!(!first.contains("axis pin"), "{first}");

    assert!(!second.contains("drills the dowel holes"), "{second}");
    assert!(
        second.contains(concat!(
            "(MSG,TURN THE PART BEFORE RUNNING [Top 0 deg -> Bottom 0 deg] -- Seat the part on ",
            "dowels 1-3; KEEP the same X/Y zero;"
        )),
        "{second}"
    );
    // The comment renderer turns `()` into `[]`.
    assert!(
        second.contains(concat!(
            "(The part turns over about y = 32.5 mm. Dowel 1 [the axis pin] is on the +X side ",
            "and keeps its hole; dowels 2 and 3 change holes.)\n"
        )),
        "{second}"
    );

    // The line budget: the two setup lines, then 3 + N for N = 3 dowels.
    // Setup 1 adds the lead, 3 dowels and the drill line; setup 2 the
    // lead, 3 dowels, the cue and the turn message.
    assert_eq!(first.lines().count(), 2 + 5, "{first}");
    assert_eq!(second.lines().count(), 2 + 6, "{second}");
}

#[test]
fn an_equal_pair_gets_the_not_keyed_warning() {
    let pins = vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(6.0, 32.5, 6.0),
    ];
    let session = session_with(pins, true);
    let (first, second) = headers(&session, PostFormat::Grbl);
    assert!(!first.contains("WARNING"), "{first}");
    assert!(
        second.contains(&format!("({NOT_KEYED_WARNING})\n")),
        "{second}"
    );
    assert!(second.contains("Seat the part on dowels 1-2"), "{second}");
}

#[test]
fn no_pins_or_no_flip_gives_no_dowel_line() {
    for session in [
        session_with(Vec::new(), true),
        session_with(three_pins(), false),
    ] {
        let (first, second) = headers(&session, PostFormat::GrblHal);
        for header in [&first, &second] {
            for text in ["DOWELS", "Dowel", "dowel", "Seat the part", "axis pin"] {
                assert!(!header.contains(text), "{text}: {header}");
            }
        }
    }
}

#[test]
fn the_setup_sheet_prints_the_same_dowel_text() {
    let session = session_with(three_pins(), true);
    let gui = GuiState::new();
    let html = generate_setup_sheet_from_session(&session, &gui, None);
    assert!(
        html.contains("DOWELS: 3 in the spoilboard, X Y from X0 Y0, diameter"),
        "{html}"
    );
    assert!(
        html.contains("<li>Dowel 1: X 59.00 Y 32.50, 6.00 mm</li>"),
        "{html}"
    );
    assert!(
        html.contains("This setup drills the dowel holes 1-3."),
        "{html}"
    );
    assert!(html.contains("Seat the part on dowels 1-3."), "{html}");
    // The sheet is HTML: the parentheses stay.
    assert!(
        html.contains("Dowel 1 (the axis pin) is on the +X side and keeps its hole"),
        "{html}"
    );

    let none = generate_setup_sheet_from_session(&session_with(Vec::new(), true), &gui, None);
    assert!(!none.contains("DOWELS"), "{none}");
}
