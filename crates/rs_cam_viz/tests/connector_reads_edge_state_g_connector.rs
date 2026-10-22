//! G-CONNECTOR sentry: the gutter connector is the card's one dependency
//! surface, and it reads the core's edge model.
//!
//! # What this replaces
//!
//! The Rest card carried a `dep` / `no dep` badge with its own predicate
//! (`rest_badge`). R3 of `planning/gen_sim_rest_ux_2026-09-18/` folded that
//! badge into the connector. The Rest operation and its `PrevTool` edge
//! were retired on 2026-10-14 (ruling R3 of `op_architecture_2026-10-14`),
//! and arm 2 (G-RESTBADGE) went with them.
//!
//! # The arms
//!
//! 1. Role, stroke and hover for the six `(EdgeKind, EdgeState)` pairs.
//! 3. Geometry: the elbow sits on the gutter centre line, and an off-list
//!    source draws a stub rather than a line to nothing.
//! 4. Non-vacuity: the panel calls `draw_connectors(` and names neither the
//!    deleted badge nor its words.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;

use rs_cam_core::session::dependencies::{EdgeKind, EdgeState};
use rs_cam_viz::ui::components::Role;
use rs_cam_viz::ui::tokens;
use rs_cam_viz::ui::toolpath_panel::{
    RowGeometry, arrow_tip, connector_path, edge_hover, edge_role, edge_stroke, header_bands,
    rail_gaps,
};

const PANEL_SRC: &str = include_str!("../src/ui/toolpath_panel.rs");

// ── arm 1 — role, stroke and hover for the six pairs ────────────────────

const KINDS: [EdgeKind; 2] = [EdgeKind::Stock, EdgeKind::Regions];
const STATES: [EdgeState; 3] = [EdgeState::Ready, EdgeState::Pending, EdgeState::Broken];

/// The role ladder of §4.4: a pass, a wait, a fault.
///
/// The first drawing gave a ready edge a 1 point `HAIRLINE` grey, on the
/// reading that structure is not a verdict. On screen the operator could see
/// nothing at all, and ruled on 2026-09-18 that a satisfied dependency is a
/// thing to confirm: it draws GREEN, at the `Ok` role, like any other pass.
#[test]
fn every_edge_state_takes_its_own_role_g_connector() {
    assert_eq!(edge_role(EdgeState::Ready), Role::Ok);
    assert_eq!(edge_role(EdgeState::Pending), Role::Caution);
    assert_eq!(edge_role(EdgeState::Broken), Role::Danger);

    let ready = edge_stroke(EdgeState::Ready);
    let pending = edge_stroke(EdgeState::Pending);
    let broken = edge_stroke(EdgeState::Broken);

    // Each state DRAWS in its own role's colour. A line the operator cannot
    // see reports nothing, so this is the arm that keeps the ruling.
    assert_eq!(ready.colour, Role::Ok.text());
    assert_eq!(pending.colour, Role::Caution.text());
    assert_eq!(broken.colour, Role::Danger.text());
    assert_ne!(
        ready.colour,
        tokens::HAIRLINE,
        "a connector is never drawn in the border grey: that was the \
         invisible first drawing"
    );

    // Never a hairline, whatever the state.
    for (name, spec) in [("ready", ready), ("pending", pending), ("broken", broken)] {
        assert!(
            spec.width >= 2.0,
            "the {name} connector is {} points and the floor is 2",
            spec.width
        );
    }

    // §2.6 rule 3: colour is never the only channel. A ready edge is the
    // only SOLID one, and the width is the third channel, because a twelve
    // point gutter cannot hold a glyph.
    assert!(
        ready.dash.is_none(),
        "a ready edge is the one solid line, which is its second channel"
    );
    assert!(
        pending.dash.is_some() && broken.dash.is_some(),
        "an unready edge must carry a dash, or the colour is the only channel"
    );
    assert!(
        broken.width > pending.width,
        "a broken edge is heavier than a pending one: {} vs {}",
        broken.width,
        pending.width
    );
}

/// Six pairs, six distinct hovers, and each names its source.
#[test]
fn every_edge_pair_says_something_different_g_connector() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for kind in KINDS {
        for state in STATES {
            let hover = edge_hover(Some("Back Rough"), kind, state, false);
            assert!(
                !hover.is_empty(),
                "{kind:?}/{state:?} says nothing on hover"
            );
            assert!(
                seen.insert(hover.clone()),
                "{kind:?}/{state:?} repeats a hover another pair already uses: {hover}"
            );
        }
    }
    assert_eq!(
        seen.len(),
        6,
        "non-vacuity: six pairs must yield six strings"
    );

    // The two unready states say WHY, and a ready one does not have to.
    for kind in KINDS {
        let ready = edge_hover(Some("Back Rough"), kind, EdgeState::Ready, false);
        for state in [EdgeState::Pending, EdgeState::Broken] {
            let unready = edge_hover(Some("Back Rough"), kind, state, false);
            assert!(
                unready.len() > ready.len(),
                "{kind:?}/{state:?} must add a reason to the ready wording: \
                 {unready:?} against {ready:?}"
            );
        }
    }

    // All six name the source.
    let named = KINDS
        .iter()
        .flat_map(|k| STATES.iter().map(move |s| (*k, *s)))
        .filter(|(k, s)| edge_hover(Some("Back Rough"), *k, *s, false).contains("Back Rough"))
        .count();
    assert_eq!(
        named, 6,
        "the hover exists to name the source, and only {named} of six do"
    );
}

/// A source that is drawn can be clicked, and the hover says so.
#[test]
fn a_drawn_source_offers_the_click_g_connector() {
    let quiet = edge_hover(Some("Back Rough"), EdgeKind::Stock, EdgeState::Ready, false);
    let clickable = edge_hover(Some("Back Rough"), EdgeKind::Stock, EdgeState::Ready, true);
    assert!(
        !quiet.contains("Click"),
        "a source that is not in the list must not offer a click: {quiet}"
    );
    assert!(
        clickable.starts_with(&quiet) && clickable.contains("Click to select it."),
        "a drawn source adds the click line to the same wording: {clickable}"
    );
}

// ── arm 3 — geometry ────────────────────────────────────────────────────

/// One row at `top`: a card 200 points wide with its swatch inside the
/// card's own 3 point left padding, which is where the real one sits.
fn row(top: f32) -> RowGeometry {
    let card = egui::Rect::from_min_size(egui::pos2(40.0, top), egui::vec2(200.0, 26.0));
    RowGeometry {
        card,
        swatch: egui::Rect::from_min_size(
            egui::pos2(card.left() + 3.0, top + 5.0),
            egui::vec2(8.0, 16.0),
        ),
        colour: egui::Color32::from_rgb(0x30, 0xA0, 0x60),
    }
}

/// The rail is STRAIGHT, and it runs down the swatch column.
///
/// The first drawing hung it in a gutter left of the cards, with a jog out
/// of the source row and a jog back into the dependent one. On screen it
/// read as a bracket beside the list rather than a line between two rows
/// (operator, 2026-09-18). The swatch column is the one x every row already
/// shares.
#[test]
fn the_rail_runs_straight_down_the_swatch_column_g_connector() {
    let source = row(0.0);
    let target = row(60.0);
    let path = connector_path(Some(source), target);

    assert_eq!(path.points.len(), 2, "the rail is straight: {path:?}");
    let (head, foot) = (path.points[0], path.points[1]);
    assert!(
        (head.x - foot.x).abs() < f32::EPSILON,
        "a rail with a jog in it is the bracket this replaced: {path:?}"
    );
    assert!(
        (head.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the rail runs at the SWATCH centre, not beside the card: rail {} \
         swatch {}",
        head.x,
        target.swatch.center().x
    );
    assert!(
        (head.x - source.swatch.center().x).abs() < f32::EPSILON,
        "every row shares that column, so the rail leaves the source swatch \
         dead centre too"
    );
    assert!(
        (head.y - source.swatch.bottom()).abs() < f32::EPSILON,
        "the rail starts at the bottom of the SOURCE swatch: {path:?}"
    );
    assert!(
        head.x > target.card.left(),
        "the rail is INSIDE the card now, so the list keeps its full width"
    );
}

/// The arrowhead points DOWN into the dependent swatch, and stops above it.
#[test]
fn the_arrowhead_lands_on_top_of_the_dependent_swatch_g_connector() {
    let target = row(60.0);
    let path = connector_path(Some(row(0.0)), target);
    let tip = arrow_tip(target);

    assert_eq!(tip, path.tip, "the path reports the tip the caller paints");
    assert!(
        (tip.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the arrow is centred on the swatch it points into: {tip:?}"
    );
    let gap = target.swatch.top() - tip.y;
    assert!(
        gap > 0.0 && gap <= 2.0,
        "the tip stops just ABOVE the swatch, and this one is {gap} points \
         off it"
    );
    let foot = path.points[1];
    assert!(
        foot.y < tip.y,
        "the rail stops short of the tip, so the line and the arrowhead do \
         not overlap: foot {} tip {}",
        foot.y,
        tip.y
    );
    assert!(
        tip.y - foot.y <= 6.0,
        "the arrowhead is small: it is {} points long",
        tip.y - foot.y
    );
}

/// A chain A to B to C is two COLLINEAR segments on one rail.
#[test]
fn a_chain_is_one_rail_with_the_swatches_threaded_on_it_g_connector() {
    let a = row(0.0);
    let b = row(40.0);
    let c = row(80.0);
    let first = connector_path(Some(a), b);
    let second = connector_path(Some(b), c);

    for point in first.points.iter().chain(second.points.iter()) {
        assert!(
            (point.x - a.swatch.center().x).abs() < f32::EPSILON,
            "every point of a chain sits on one column: {point:?}"
        );
    }
    assert!(
        (second.points[0].y - b.swatch.bottom()).abs() < f32::EPSILON,
        "the second segment leaves B's OWN swatch, which is where the first \
         one arrived: {second:?}"
    );
    assert!(
        first.tip.y < b.swatch.top() && second.points[0].y > b.swatch.top(),
        "B's swatch sits BETWEEN the two segments, threaded on the rail"
    );
}

/// An off-list source draws a short rail above the row, not a line to
/// nothing.
#[test]
fn an_off_list_source_draws_a_stub_g_connector() {
    let target = row(200.0);
    let path = connector_path(None, target);
    let stub = path.stub.expect("an undrawn source draws the stub");

    assert!(
        (stub.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the stub keeps the rail's column: {path:?}"
    );
    assert!(
        stub.y < target.swatch.top(),
        "the stub sits ABOVE the swatch, where the source would have been"
    );
    let span = target.swatch.top() - stub.y;
    assert!(
        span > 0.0 && span < target.card.height(),
        "the stub is short: it says `not in this list`, it does not pretend \
         to reach a row. It spans {span} points."
    );
}

/// A rail yields the operator no hit strip over a swatch it crosses.
///
/// The rail runs down the swatch column, so a rail to a row two below passes
/// through the swatch of the row between. That band is the crossed row's own
/// drag grip, and a connector that took its clicks would make the row
/// undraggable.
#[test]
fn a_crossed_swatch_keeps_its_own_clicks_g_connector() {
    // A rail from y=20 to y=100 across one swatch at 40..=56.
    let gaps = rail_gaps(20.0, 100.0, &[(40.0, 56.0)]);
    assert_eq!(gaps, vec![(20.0, 40.0), (56.0, 100.0)]);

    // Two crossings, given out of order, and one that misses the rail.
    let gaps = rail_gaps(0.0, 100.0, &[(60.0, 70.0), (200.0, 216.0), (20.0, 30.0)]);
    assert_eq!(gaps, vec![(0.0, 20.0), (30.0, 60.0), (70.0, 100.0)]);

    // A clear rail is one strip, and a fully covered one is none.
    assert_eq!(rail_gaps(0.0, 40.0, &[]), vec![(0.0, 40.0)]);
    assert!(rail_gaps(10.0, 20.0, &[(0.0, 30.0)]).is_empty());
}

/// A rail never draws across a setup header.
///
/// With R2 landed a dependency crosses a setup boundary, and the rail then
/// ran straight through the "Setup 2" label. The operator's verdict was "it
/// clips over the 'Setup 2'.. that's the only issue" (2026-09-19). The
/// header band is blocked the same way a crossed swatch is, so a cross-setup
/// rail draws as two pieces on one column with the header clear between.
#[test]
fn a_cross_setup_rail_breaks_at_the_header_g_connector() {
    // Two list frames with a header band between them.
    let first = egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(220.0, 60.0));
    let second = egui::Rect::from_min_size(egui::pos2(10.0, 100.0), egui::vec2(220.0, 60.0));
    let bands = header_bands(&[first, second]);
    assert_eq!(
        bands,
        vec![(60.0, 100.0)],
        "the band is the gap between the two list frames: the label, its \
         controls and the air around them"
    );

    // A rail from a row in the first frame to a row in the second one.
    let path = connector_path(Some(row(30.0)), row(110.0));
    let (top, foot) = (path.points[0].y, path.points[1].y);
    let pieces = rail_gaps(top, foot, &bands);
    assert_eq!(
        pieces.len(),
        2,
        "a cross-setup rail draws in two pieces: {pieces:?}"
    );
    assert!(
        (pieces[0].1 - bands[0].0).abs() < f32::EPSILON
            && (pieces[1].0 - bands[0].1).abs() < f32::EPSILON,
        "the gap between the pieces IS the header band: {pieces:?} against \
         {bands:?}"
    );
    assert!(
        pieces[0].0 >= top && pieces[1].1 <= foot,
        "the two pieces stay inside the rail they belong to: {pieces:?}"
    );

    // A same-setup rail sees no band and stays one piece.
    let near = connector_path(Some(row(0.0)), row(30.0));
    assert_eq!(
        rail_gaps(near.points[0].y, near.points[1].y, &bands).len(),
        1,
        "non-vacuity: a rail inside one setup is not broken by a band it \
         never reaches"
    );
}

/// One setup draws no header band, and a header needs two frames.
#[test]
fn a_single_setup_has_no_header_band_g_connector() {
    let frame = egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(220.0, 60.0));
    assert!(header_bands(&[frame]).is_empty());
    assert!(header_bands(&[]).is_empty());

    // Three setups give two bands, in list order.
    let second = egui::Rect::from_min_size(egui::pos2(10.0, 100.0), egui::vec2(220.0, 40.0));
    let third = egui::Rect::from_min_size(egui::pos2(10.0, 180.0), egui::vec2(220.0, 40.0));
    assert_eq!(
        header_bands(&[frame, second, third]),
        vec![(60.0, 100.0), (140.0, 180.0)]
    );
}

// ── arm 4 — non-vacuity ─────────────────────────────────────────────────

#[test]
fn the_panel_draws_connectors_and_no_badge_g_connector() {
    assert!(
        PANEL_SRC.len() > 10_000,
        "the panel source is {} bytes, which is too small to be the \
         operations panel",
        PANEL_SRC.len()
    );
    let code: String = PANEL_SRC
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        code.contains("draw_connectors("),
        "the card's dependency surface is the connector; the panel must call it"
    );
    assert!(
        code.contains("primary_edges("),
        "the connector reads the core's one edge model, the same door the \
         MCP `depends_on` row reads (W5)"
    );
    for gone in ["draw_rest_badge(", "RestBadge", "\"no dep\"", "\"dep\""] {
        assert!(
            !code.contains(gone),
            "R3 folded the Rest `dep` badge into the connector, and {gone} \
             came back. One idea in two places is what R3 deleted."
        );
    }
    assert!(
        code.contains("egui::DragAndDrop::has_payload_of_type"),
        "the connector pass must stand down during a drag: the rects move, \
         so a line drawn then is wrong as well as ugly"
    );
}
