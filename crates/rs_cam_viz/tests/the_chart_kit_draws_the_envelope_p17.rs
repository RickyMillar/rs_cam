//! P17 (feeds contract, operator 2026-11-06): the chart kit draws the
//! envelope plot.
//!
//! # The claims
//!
//! 1. `ui/feeds/envelope.rs` and `ui/components/chart.rs` name no
//!    `egui_plot` in their code. The anchors: the envelope still has
//!    `fn draw_plane(` and builds a `PlaneChart::new(`; the kit defines
//!    `pub struct PlaneChart`.
//! 2. One frame of the rivmap100 3D Rough plot paints exactly one fill in
//!    the histogram's OK wash (`tokens::OK` at `histogram::WASH_OK`): the
//!    safe region (the operator sentence of 2026-11-06, "the safe region
//!    as a soft fill").
//! 3. The width axis is a share of D: the "100 %" tick paints with its
//!    right end at the x of D (the kit anchors the edge label to the edge,
//!    as the histogram anchors its range-end label).
//! 4. The hover at a point inside names the share: the readout line reads
//!    "ae <mm> mm (<share> % D)".
//! 5. At 240 and 340 points, every text the section paints stays inside
//!    the screen rect, the turned y title included (the up4 trap: a
//!    rotated text's `visual_bounding_rect` ignores its angle, so the arm
//!    reads the turned corners).
//!
//! Red before P17: `envelope.rs` imports `egui_plot`, and
//! `ui/components/chart.rs` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use std::path::{Path, PathBuf};

use envelope_k2b::{ID, ctx, frame, texts};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::feeds::envelope::{CutPoint, Edge, LimitKind, SafeEnvelope};
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::components::histogram;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeShape, WIDTH_EDGE_TEXT, draw_envelope};
use rs_cam_viz::ui::tokens;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
);

/// The rivmap100 3D Rough envelope at the fixture point.
fn rivmap_env() -> (OperationConfig, SafeEnvelope) {
    let session = rs_cam_core::session::ProjectSession::load(Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .clone();
    let env = session
        .safe_envelope(&tc, rs_cam_viz::state::simulation::run_modulation())
        .expect("a tool")
        .expect("a backed envelope");
    (tc.operation, env)
}

/// The tool diameter of `env`, the width frame D.
fn diameter(env: &SafeEnvelope) -> f64 {
    env.boundary
        .iter()
        .filter(|l| l.kind == LimitKind::Diameter)
        .find_map(|l| match l.edge {
            Edge::AeCeiling { mm } => Some(mm),
            Edge::ApCeiling { .. }
            | Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeFloor { .. }
            | Edge::PerPoint
            | Edge::AeCurve { .. }
            | Edge::Unmodelled { .. } => None,
        })
        .expect("fixture: a diameter limit")
}

fn viz_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The file's code with every comment removed (a line loses everything
/// from its first `//`).
fn code_of(relative: &str) -> String {
    let path = viz_file(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The painted texts of `shapes` with the x extent of each one, the angle
/// read: a turned text's `visual_bounding_rect` ignores its angle.
fn text_extents(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, f32, f32, egui::Rect)> {
    let mut out = Vec::new();
    let mut walk = |s: &egui::Shape| {
        if let egui::Shape::Text(text) = s {
            let rot = egui::emath::Rot2::from_angle(text.angle);
            let r = text.galley.rect;
            let xs: Vec<f32> = [
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            ]
            .into_iter()
            .map(|corner| (text.pos + rot * corner.to_vec2()).x)
            .collect();
            let left = xs.iter().copied().fold(f32::INFINITY, f32::min);
            let right = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            out.push((
                text.galley.job.text.clone(),
                left,
                right,
                text.visual_bounding_rect(),
            ));
        }
    };
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            v.iter().for_each(&mut walk);
        } else {
            walk(&c.shape);
        }
    }
    out
}

/// The one-colour meshes and the filled closed paths of a frame.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn fills(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Color32> {
    let mut out = Vec::new();
    let mut walk = |s: &egui::Shape| match s {
        egui::Shape::Mesh(m) => {
            if let Some(first) = m.vertices.first()
                && m.vertices.iter().all(|v| v.color == first.color)
            {
                out.push(first.color);
            }
        }
        egui::Shape::Path(p) if p.closed && p.fill != egui::Color32::TRANSPARENT => {
            out.push(p.fill);
        }
        _ => {}
    };
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            v.iter().for_each(&mut walk);
        } else {
            walk(&c.shape);
        }
    }
    out
}

// ── claim 1 ─────────────────────────────────────────────────────────────

#[test]
fn the_envelope_and_the_kit_name_no_egui_plot_p17() {
    let envelope = code_of("src/ui/feeds/envelope.rs");
    assert!(
        envelope.contains("fn draw_plane(") && envelope.contains("PlaneChart::new("),
        "non-vacuity: the envelope plot draws its plane through the kit"
    );
    let chart = code_of("src/ui/components/chart.rs");
    assert!(
        chart.contains("pub struct PlaneChart"),
        "non-vacuity: the kit defines `PlaneChart`"
    );
    for (name, code) in [("envelope.rs", &envelope), ("chart.rs", &chart)] {
        assert!(
            !code.contains("egui_plot"),
            "`{name}` names `egui_plot`; the kit paints the plane (P17)"
        );
    }
}

// ── claims 2, 3 and 4 ───────────────────────────────────────────────────

#[test]
fn one_frame_paints_one_ok_fill_and_the_share_axis_p17() {
    let (config, env) = rivmap_env();
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let mut f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    for _ in 0..3 {
        f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    }
    let plot = f.response.frame.expect("a plot");

    // Claim 2: one fill in the OK wash, and no other translucent fill.
    let wash = tokens::OK.gamma_multiply(histogram::WASH_OK);
    let painted = fills(&f.shapes);
    assert_eq!(
        painted.iter().filter(|c| **c == wash).count(),
        1,
        "one fill in the OK wash: {painted:?}"
    );
    assert!(
        painted.iter().all(|c| *c == wash || c.a() >= 200),
        "every other fill is an opaque label chip: {painted:?}"
    );

    // Claim 3: "100 %" ends at the x of D, under the frame.
    let d = diameter(&env);
    let at_d = f
        .response
        .screen_of(
            CutPoint {
                ap_mm: 0.0,
                ae_mm: d,
            },
            EnvelopeShape::Plane,
        )
        .expect("a frame");
    let extents = text_extents(&f.shapes);
    let tick = extents
        .iter()
        .find(|(t, _, _, r)| t == WIDTH_EDGE_TEXT && r.top() >= plot.bottom() - 1.0)
        .unwrap_or_else(|| panic!("the `{WIDTH_EDGE_TEXT}` tick paints: {extents:?}"));
    assert!(
        (tick.2 - at_d.x).abs() < 1.5,
        "the `{WIDTH_EDGE_TEXT}` tick ends at D ({:.1} vs {:.1})",
        tick.2,
        at_d.x
    );

    // Claim 4: the hover at the proposal (inside) names the share.
    let inside = env.proposal.get().point().expect("a proposed depth");
    let pos = f
        .response
        .screen_of(inside, EnvelopeShape::Plane)
        .expect("a frame");
    let mut painted = Vec::new();
    for _ in 0..4 {
        let f = frame(
            &ctx,
            &config,
            Some(&env),
            &mut candidates,
            vec![egui::Event::PointerMoved(pos)],
        );
        painted = texts(&f.shapes);
    }
    let line = painted
        .iter()
        .find(|t| t.starts_with("ap "))
        .unwrap_or_else(|| panic!("no readout painted: {painted:?}"));
    let share = format!("({:.0} % D)", inside.ae_mm / d * 100.0);
    assert!(
        line.contains(&share),
        "the readout names the share `{share}`: {line}"
    );
    assert!(line.contains(" mm "), "the readout keeps its mm: {line}");
    assert!(candidates.get(ID).is_none(), "a hover moves no candidate");
}

// ── claim 5 ─────────────────────────────────────────────────────────────

#[test]
fn every_chart_text_stays_inside_the_rail_p17() {
    let (config, env) = rivmap_env();
    let ctx = ctx();
    for width in [240.0_f32, 340.0] {
        let mut candidates = EnvelopeCandidates::default();
        let mut last = None;
        for _ in 0..4 {
            let mut sent = Vec::new();
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let _ = draw_envelope(
                        ui,
                        ID,
                        &config,
                        Some(&env),
                        None,
                        &mut candidates,
                        &mut sent,
                    );
                },
            );
            out.textures_delta.clear();
            last = Some(out.shapes);
        }
        let shapes = last.expect("frames");
        let extents = text_extents(&shapes);
        assert!(
            extents.iter().any(|(t, _, _, _)| t == "DEPTH (mm)"),
            "non-vacuity at {width}: the turned depth title paints: {extents:?}"
        );
        for (text, left, right, rect) in &extents {
            assert!(
                *left >= -0.5 && *right <= width + 0.5,
                "at {width}: `{text}` spans {left:.1}..{right:.1}, past the rail 0..{width}"
            );
            assert!(
                rect.top() >= -0.5,
                "at {width}: `{text}` paints above the screen: {rect:?}"
            );
        }
    }
}
