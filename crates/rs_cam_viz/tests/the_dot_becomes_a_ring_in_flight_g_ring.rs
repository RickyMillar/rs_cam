//! G-RING sentry: a row that a worker is chewing on draws a moving glyph,
//! and a row that is idle draws the dot.
//!
//! # The defect this exists to catch
//!
//! `FreshnessState` is derived from the core result cache, and the cache
//! says nothing about the analysis lane. So a project-wide simulation left
//! every card reading its LAST state while the machine worked: the operator
//! saw `PEND` on eight rows for two minutes with nothing to say that
//! anything was happening (`PLAN.md` §4.4).
//!
//! R7 answers with a second SHAPE, not a second colour: generating keeps
//! `egui::Spinner`, simulating draws a hollow ring with a rotating gap, and
//! an idle row keeps the dot. §2.6 rule 3 is the reason — two spinners in
//! two colours make colour the only channel.
//!
//! # The two arms
//!
//! 1. The derivation is pure, so every case is driven without a `Ui`,
//!    including the precedence rule.
//! 2. The glyph is rendered headless: in flight it is a path, idle it is a
//!    circle.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::config::AwaitingPriorStock;
use rs_cam_viz::state::freshness::FreshnessState;
use rs_cam_viz::ui::toolpath_panel::{InFlight, PlanFocus, RowFacts, draw_state_glyph, in_flight};
use rs_cam_viz::ui::tokens;

const ROW: ToolpathId = ToolpathId(7);

fn row() -> RowFacts {
    RowFacts {
        id: ROW,
        setup_position: 1,
        enabled: true,
        has_result: true,
    }
}

/// The plan simulates everything up to and including setup 1.
fn plan_simulating() -> PlanFocus {
    PlanFocus {
        generating: None,
        simulating_upto: Some(1),
    }
}

// ── arm 1 — the derivation ──────────────────────────────────────────────

/// The plan's own simulation step covers every row up to that setup.
#[test]
fn a_row_the_plan_simulates_shows_the_ring_g_ring() {
    assert_eq!(
        in_flight(
            &FreshnessState::Current,
            plan_simulating(),
            false,
            row()
        ),
        Some(InFlight::Simulating),
        "a prefix simulation covers setups 0..=1, and this row sits in setup 1"
    );
    let later = RowFacts {
        setup_position: 2,
        ..row()
    };
    assert_eq!(
        in_flight(&FreshnessState::Current, plan_simulating(), false, later),
        None,
        "a row BELOW the prefix is not covered by it, so it keeps the dot"
    );
}

/// A simulation started outside a plan covers the rows that hold a result.
#[test]
fn a_lane_simulation_covers_the_rows_it_carves_g_ring() {
    assert_eq!(
        in_flight(&FreshnessState::Current, PlanFocus::default(), true, row()),
        Some(InFlight::Simulating),
        "the analysis lane is simulating and this row holds the result it carves"
    );
    let no_result = RowFacts {
        has_result: false,
        ..row()
    };
    assert_eq!(
        in_flight(
            &FreshnessState::NoResult,
            PlanFocus::default(),
            true,
            no_result
        ),
        None,
        "a row with no result is not in the run: the simulator carves nothing for it"
    );
}

/// The lane being quiet is the second belt. A stale claim draws no ring.
#[test]
fn an_idle_lane_draws_the_dot_g_ring() {
    assert_eq!(
        in_flight(&FreshnessState::Current, PlanFocus::default(), false, row()),
        None,
        "no plan and no lane means nothing is in flight"
    );
    assert_eq!(
        in_flight(
            &FreshnessState::WaitingOnUpstream(AwaitingPriorStock {
                blocking_toolpath_id: None,
                blocking_toolpath_index: None,
                message: "the upstream operation has not been simulated".to_owned(),
            }),
            PlanFocus::default(),
            false,
            row()
        ),
        None,
        "waiting is not working: the residual failure must be a MISSING ring, \
         never a stuck one"
    );
}

/// A disabled row is not in any run.
#[test]
fn a_disabled_row_is_never_in_flight_g_ring() {
    let off = RowFacts {
        enabled: false,
        ..row()
    };
    assert_eq!(
        in_flight(&FreshnessState::Disabled, plan_simulating(), true, off),
        None,
        "generation, simulation and output all skip a disabled operation"
    );
}

/// Precedence: generating wins over simulating, from either source.
#[test]
fn generating_outranks_simulating_g_ring() {
    assert_eq!(
        in_flight(
            &FreshnessState::Regenerating,
            plan_simulating(),
            true,
            row()
        ),
        Some(InFlight::Generating),
        "the freshness state already folds the toolpath lane, and it wins"
    );
    let focus = PlanFocus {
        generating: Some(ROW),
        simulating_upto: Some(1),
    };
    assert_eq!(
        in_flight(&FreshnessState::Current, focus, true, row()),
        Some(InFlight::Generating),
        "the plan naming this row as its Generate step wins too"
    );
    let other = PlanFocus {
        generating: Some(ToolpathId(99)),
        simulating_upto: None,
    };
    assert_eq!(
        in_flight(&FreshnessState::Current, other, false, row()),
        None,
        "the plan generating ANOTHER row says nothing about this one"
    );
}

// ── arm 2 — the rendered glyph ──────────────────────────────────────────

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // epaint panics if a `TexturesDelta` drops unapplied.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn glyph_shapes(state: Option<InFlight>) -> Vec<egui::Shape> {
    let ctx = context();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(12.0, 12.0));
        draw_state_glyph(ui, rect, state, tokens::CAUTION);
    });
    let shapes = std::mem::take(&mut out.shapes);
    out.textures_delta.clear();
    shapes.into_iter().map(|clipped| clipped.shape).collect()
}

/// An idle row draws a filled circle, and nothing else.
#[test]
fn an_idle_row_draws_a_circle_g_ring() {
    let shapes = glyph_shapes(None);
    let circles = shapes
        .iter()
        .filter(|s| matches!(s, egui::Shape::Circle(_)))
        .count();
    assert_eq!(
        circles, 1,
        "the idle glyph is ONE dot (R24); the pass drew {shapes:?}"
    );
}

/// A simulating row draws the ring: a stroked path in the INFO accent.
#[test]
fn a_simulating_row_draws_the_ring_g_ring() {
    let shapes = glyph_shapes(Some(InFlight::Simulating));
    assert!(
        !shapes
            .iter()
            .any(|s| matches!(s, egui::Shape::Circle(_))),
        "the ring REPLACES the dot; it does not sit beside it: {shapes:?}"
    );
    let ring = shapes.iter().find_map(|s| match s {
        egui::Shape::Path(path)
            if matches!(
                path.stroke.color,
                egui::epaint::ColorMode::Solid(colour) if colour == tokens::INFO
            ) =>
        {
            Some(path)
        }
        _ => None,
    });
    let ring = ring.unwrap_or_else(|| {
        panic!("no INFO-stroked path in the simulating pass: {shapes:?}");
    });
    assert!(
        ring.points.len() > 8,
        "the ring is sampled as a polyline, and {} points will not read as \
         an arc",
        ring.points.len()
    );
    assert!(
        !ring.fill.is_opaque(),
        "the ring is HOLLOW, which is what makes it a different shape from \
         the dot rather than a second colour of it"
    );

    // A gap, not a closed circle: the first and last samples must not meet.
    let first = ring.points[0];
    let last = ring.points[ring.points.len() - 1];
    assert!(
        first.distance(last) > 1.0,
        "the ring needs its rotating gap, and the ends meet at {first:?} {last:?}"
    );
}
