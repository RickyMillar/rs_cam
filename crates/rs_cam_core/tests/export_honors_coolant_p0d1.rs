//! P0-D1 — the core export door emits the operation's coolant setting.
//!
//! Operator ruling 2026-09-11,
//! `planning/arch_consolidation_2026-09-09/STATUS.md`, "Operator
//! rulings": ruling (a). `ToolpathConfig::coolant`
//! (`src/session/mod.rs:769`) is a per-toolpath core field, and both
//! project-file loaders persist it. The core door
//! `gcode::export_gcode_checked` (`src/gcode/mod.rs:300`) read it as
//! `CoolantMode::Off` at `:367` until P0-D1, so `rs_cam_cli project
//! --emit-gcode` and `rs_cam_cli run` dropped the setting without a
//! word. The GUI door (`rs_cam_viz/src/io/export.rs:340`) and the CLI
//! job-file door (`rs_cam_cli/src/main.rs:547,613`) always honoured it.
//!
//! # This file pins the core door from the core crate
//!
//! `rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` measures the same
//! field ACROSS the two doors, and it lives in the viz crate because it
//! calls both. This file is the core-side half. It calls one door, so a
//! core-only change that drops the field again fails here without a viz
//! build.
//!
//! # The door this file calls
//!
//! `ProjectSession::export_gcode_with_policy`
//! (`src/session/compute.rs:4290`) takes a path and writes a file. It
//! wraps `gcode::export_gcode_checked`, which returns the string, and
//! it forwards the session's own cut trace. This fixture runs no
//! simulation, so that trace is absent and the wrapped door's argument
//! is `None`. This file calls the string-returning door directly and
//! reads its bytes.
//!
//! # The emitter formats this file reads
//!
//! `CoolantMode::start_gcode` (`src/gcode/mod.rs:84-92`) gives `M8` for
//! `Flood`. The program builder pushes that word as a `Statement::Raw`
//! and closes an active mode with a bare `M9`
//! (`src/gcode/program_builder.rs:155-158`, `:199-201`). Both words sit
//! on lines of their own, so an exact line match cannot collide with
//! the same text inside a comment or a coordinate.
//!
//! `Flood` is the probe, not `Mist`. The shipped GRBL post declares
//! `unsupported_mcodes = [6, 7]`, so `filter_raw`
//! (`src/gcode/emitter.rs:154-194`) replaces an `M7` line with a
//! warning comment. A `Mist` setting therefore reaches the emitter and
//! still writes no coolant word on this post, which would make a `Mist`
//! probe pass for the wrong reason.
//!
//! # The fixture
//!
//! One in-memory session: one stock, one 6 mm end mill, one square
//! polygon model, one pocket operation. No generator runs and no
//! simulation runs, so the test inserts a short hand-built result. The
//! insert comes LAST, because a mutating method on `ProjectSession`
//! drops cached results (G-FRESHSTATE) and the door then emits no
//! phase at all.
//!
//! The export policy accepts `Unmodeled` and `Exceeds`. No simulation
//! runs here, so every tool-load criterion reads `Unmodeled`; another
//! file covers the gate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::gcode::{CoolantMode, ToolLoadExportPolicy, export_gcode_checked};
use rs_cam_core::geo::P3;
use rs_cam_core::session::{AdoptResultArgs, Command, ProjectSession, ToolpathComputeResult};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};

/// Half-extent of the square model, in mm.
const MODEL_HALF: f64 = 10.0;

/// Stock height, in mm. `stock_under` roots the stock below world Z0,
/// which is the frame a 2D operation cuts in.
const STOCK_Z: f64 = 12.0;

/// Cutting feed of every sample move (mm/min).
const FEED: f64 = 600.0;

const OP_LABEL: &str = "Coolant Pocket";

/// Three cutting moves at `FEED`, bracketed by rapids. The emitter
/// needs motion between the coolant words for the program to be a
/// program; the coolant contract itself does not depend on the shape.
fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), FEED);
    path.feed_to(P3::new(10.0, 10.0, -1.0), FEED);
    path.feed_to(P3::new(0.0, 10.0, -1.0), FEED);
    path.rapid_to(P3::new(0.0, 10.0, 5.0));
    path
}

fn hand_built_result() -> ToolpathComputeResult {
    let annotated = Arc::new(AnnotatedToolpath::new(sample_toolpath()));
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(annotated),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// One session whose single operation carries `coolant`.
fn session_with_coolant(coolant: CoolantMode) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_under(MODEL_HALF, STOCK_Z),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(MODEL_HALF)], "square"),
        OP_LABEL,
        OperationConfig::Pocket(PocketConfig::default()),
        |tc| tc.coolant = coolant,
    );
    let revision = session.toolpath_revision(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(hand_built_result()),
        }))
        .expect("adopt the hand-built result at the current revision");
    session
}

fn export(session: &ProjectSession) -> String {
    let policy = ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    };
    export_gcode_checked(session, None, policy).expect("the core door exports")
}

/// True when any line of `gcode` is exactly `word`.
fn has_line(gcode: &str, word: &str) -> bool {
    gcode.lines().any(|line| line.trim() == word)
}

/// The zero-based index of the LAST line that is exactly `word`, or
/// `None` when no line is. The program opens a coolant mode with `M8`
/// and closes it with a later `M9`, so an `M9` that precedes the last
/// `M8` leaves the mode open.
fn last_line_index(gcode: &str, word: &str) -> Option<usize> {
    gcode
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim() == word)
        .map(|(index, _)| index)
        .last()
}

/// The core door emits `M8` and a later `M9` for an operation whose
/// coolant is `Flood`.
///
/// The control below runs the same fixture with `CoolantMode::Off` and
/// asserts neither word. Without it this test would also pass on a door
/// that emitted coolant unconditionally.
#[test]
fn core_export_emits_the_operations_flood_coolant() {
    let session = session_with_coolant(CoolantMode::Flood);
    let gcode = export(&session);

    assert!(
        has_line(&gcode, "M8"),
        "P0-D1 ruling (a): the core door must emit the flood-coolant \
         start word M8 for an operation whose coolant is Flood. A door \
         that reads CoolantMode::Off instead of tc.coolant \
         (gcode/mod.rs:367) emits none. Program:\n{gcode}"
    );
    assert!(
        last_line_index(&gcode, "M9") > last_line_index(&gcode, "M8"),
        "P0-D1 ruling (a): the core door must close the coolant mode \
         with an M9 that follows the last M8 \
         (program_builder.rs:199-201). Last M8 line {:?}, last M9 line \
         {:?}.",
        last_line_index(&gcode, "M8"),
        last_line_index(&gcode, "M9")
    );
}

/// The control: the same fixture with coolant `Off` emits no coolant
/// word. This is what makes the `Flood` assertions above measure the
/// field rather than the emitter's habits.
#[test]
fn core_export_emits_no_coolant_word_when_the_operation_asks_for_none() {
    let session = session_with_coolant(CoolantMode::Off);
    let gcode = export(&session);

    assert!(
        !has_line(&gcode, "M8"),
        "the core door must emit no coolant start word for an operation \
         whose coolant is Off. Program:\n{gcode}"
    );
    assert!(
        !has_line(&gcode, "M9"),
        "the core door opens no coolant mode for an operation whose \
         coolant is Off, so it must emit no closing M9. Program:\n{gcode}"
    );
}
