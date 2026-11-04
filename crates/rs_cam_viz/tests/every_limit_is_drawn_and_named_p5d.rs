//! P5d (feeds contract, the operator's second review of the two charts on
//! the Passes tab, 2026-11-04): every limit is drawn and named, the
//! recommendation is quiet, and one mark means one thing on both charts.
//!
//! # The claims (card `planning/feeds_contract_2026-11-03/phases/P5d.md`)
//!
//! 1. D1, D2, D4, D5: on the rivmap100 3D Rough at rest, every non-frame
//!    limit whose edge enters the view draws a line and ONE label: Vendor
//!    DOC, Rigidity DOC and Slot DOC. The depth range is
//!    `min(hard top, VIEW_HEADROOM x max(the flat depth ceilings under the
//!    hard top, the marks))`, derived here from the envelope. The binding
//!    line is `TEXT_BODY`, the others `TEXT_MUTED` and thinner; no line is
//!    DANGER. The vendor label names its depth as a share of D. The width
//!    frame edge D paints its tick.
//! 2. D3: the white dot is the setpoint on both charts. A drag on the
//!    envelope plot moves the envelope white dot to the candidate and the
//!    nomogram white dot to `candidate_speeds`; a hollow `TEXT_MUTED` ghost
//!    marks the stored point on each chart only while the draft exists; Esc
//!    returns both. The recommendation (the proposal diamond, the
//!    Recommended ring and the suggested chip line) is quiet ACCENT, 1 pt,
//!    and does not move during the draft. No mark is full ACCENT.
//! 3. The counts at 340 and 240 points: every drawn envelope line has its
//!    label at both widths; the nomogram RPM axis names at least two ticks
//!    at 240 and "Now" and "Max RPM" do not touch. The nomogram paints no
//!    "Recommended" label: the hover names the ring.
//!
//! Red before P5d: `EnvelopeScene` has no `drawn_edges` or `top`,
//! `SceneEdge` has no `label`, and the envelope module exports no
//! `recommendation_color`, `limit_name` or `VIEW_HEADROOM`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "the derived depth range and the counts print for the P5d card (`--nocapture`)"
)]

mod envelope_k2b;

use envelope_k2b::{ID, ctx, frame, solid, texts};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::feeds::envelope::{CutPoint, Edge, LimitKind, SafeEnvelope};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::feeds::envelope::{
    EnvelopeScene, EnvelopeShape, GHOST_RADIUS, RECOMMENDATION_WIDTH, VIEW_HEADROOM,
    candidate_speeds, limit_name, recommendation_color,
};
use rs_cam_viz::ui::{properties, tokens};

// ── fixtures ────────────────────────────────────────────────────────────

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
);

/// The rivmap100 3D Rough envelope, at the fixture point or at `(ap, ae)`.
fn rivmap_env(point: Option<(f64, f64)>) -> (OperationConfig, SafeEnvelope) {
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let mut tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .clone();
    if let (OperationConfig::Adaptive3d(c), Some((ap, ae))) = (&mut tc.operation, point) {
        c.depth_per_pass = Some(ap);
        c.stepover = Some(ae);
    }
    let env = session
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed envelope");
    (tc.operation, env)
}

/// The rivmap100 project on the Passes tab of the 3D Rough, and its id.
fn rivmap_state() -> (AppState, rs_cam_core::ToolpathId) {
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let mut state = AppState::new();
    state.replace_session(session);
    let id = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .id;
    state
        .gui
        .toolpath_rt
        .insert(id, rs_cam_viz::state::runtime::ToolpathRuntime::new(true));
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Passes));
    (state, id)
}

/// The `mm` of the first `kind` ceiling (level or upright).
fn ceiling(env: &SafeEnvelope, kind: LimitKind) -> Option<f64> {
    env.boundary
        .iter()
        .filter(|l| l.kind == kind)
        .find_map(|l| match l.edge {
            Edge::ApCeiling { mm } | Edge::AeCeiling { mm } => Some(mm),
            Edge::ApCeilingAbove { .. }
            | Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeFloor { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        })
}

const fn is_frame(kind: LimitKind) -> bool {
    matches!(
        kind,
        LimitKind::Diameter
            | LimitKind::CuttingLength
            | LimitKind::FluteGuard
            | LimitKind::EngagementFloor
    )
}

// ── what a frame painted ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct Circle {
    center: egui::Pos2,
    radius: f32,
    fill: egui::Color32,
    stroke: egui::Color32,
    stroke_width: f32,
}

#[derive(Debug, Clone)]
struct PathShape {
    points: Vec<egui::Pos2>,
    closed: bool,
    color: egui::Color32,
    width: f32,
}

#[derive(Debug, Default)]
struct Painted {
    texts: Vec<(String, egui::Rect)>,
    circles: Vec<Circle>,
    paths: Vec<PathShape>,
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn painted(shapes: &[egui::epaint::ClippedShape]) -> Painted {
    let mut out = Painted::default();
    let mut walk = |s: &egui::Shape| match s {
        egui::Shape::Text(t) => out
            .texts
            .push((t.galley.job.text.clone(), t.visual_bounding_rect())),
        egui::Shape::Circle(c) => out.circles.push(Circle {
            center: c.center,
            radius: c.radius,
            fill: c.fill,
            stroke: c.stroke.color,
            stroke_width: c.stroke.width,
        }),
        egui::Shape::Path(p) if p.points.len() >= 2 => out.paths.push(PathShape {
            points: p.points.clone(),
            closed: p.closed,
            color: solid(&p.stroke.color),
            width: p.stroke.width,
        }),
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

/// One frame of the inspector `width` points wide, with `events`.
fn inspector(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
    events: Vec<egui::Event>,
) -> Painted {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 2400.0),
        )),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("p5d_properties")
            .default_size(width)
            .min_size(width)
            .max_size(width)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    });
    out.textures_delta.clear();
    painted(&out.shapes)
}

/// Frames until the layout settles; the last one.
fn settled(ctx: &egui::Context, state: &mut AppState, width: f32) -> Painted {
    for _ in 0..5 {
        let _ = inspector(ctx, state, width, Vec::new());
    }
    inspector(ctx, state, width, Vec::new())
}

/// The bands of the two charts in the inspector, in screen y: the envelope
/// plot (under the `SAFE CUT` header, over its width axis title) and the
/// nomogram (under that title, over the topmost `RPM` title).
struct Bands {
    envelope: (f32, f32),
    nomogram: (f32, f32),
    rpm_title: egui::Rect,
}

fn bands(p: &Painted) -> Bands {
    let top_of = |t: &str| {
        p.texts
            .iter()
            .filter(|(s, _)| s == t)
            .map(|(_, r)| *r)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("`{t}` paints: {:?}", p.texts))
    };
    let head = top_of("SAFE CUT");
    let width_title = top_of("Width (mm)");
    let rpm_title = top_of("RPM");
    Bands {
        envelope: (head.bottom(), width_title.top()),
        nomogram: (width_title.bottom(), rpm_title.top()),
        rpm_title,
    }
}

const fn within(y: f32, band: (f32, f32)) -> bool {
    y > band.0 && y < band.1
}

/// The nomogram marks: the white dot, the ghost and the quiet ring.
#[derive(Debug)]
struct NomoMarks {
    white: Vec<Circle>,
    ghost: Vec<Circle>,
    quiet: Vec<Circle>,
    full_accent: Vec<Circle>,
    suggested: Vec<PathShape>,
}

fn nomo_marks(p: &Painted, b: &Bands) -> NomoMarks {
    let inside: Vec<Circle> = p
        .circles
        .iter()
        .copied()
        .filter(|c| within(c.center.y, b.nomogram))
        .collect();
    NomoMarks {
        white: inside
            .iter()
            .copied()
            .filter(|c| c.fill == tokens::TEXT_STRONG)
            .collect(),
        ghost: inside
            .iter()
            .copied()
            .filter(|c| {
                c.fill == egui::Color32::TRANSPARENT
                    && c.stroke == tokens::TEXT_MUTED
                    && (c.radius - GHOST_RADIUS).abs() < 0.01
            })
            .collect(),
        quiet: inside
            .iter()
            .copied()
            .filter(|c| c.fill == egui::Color32::TRANSPARENT && c.stroke == recommendation_color())
            .collect(),
        full_accent: inside
            .iter()
            .copied()
            .filter(|c| c.fill == tokens::ACCENT || c.stroke == tokens::ACCENT)
            .collect(),
        suggested: p
            .paths
            .iter()
            .filter(|s| {
                !s.closed
                    && s.color == recommendation_color()
                    && s.points.iter().all(|q| within(q.y, b.nomogram))
            })
            .cloned()
            .collect(),
    }
}

/// The nomogram hover at `at`: `(rpm, feed)` parsed from the readout line
/// `<rpm> RPM · <feed> mm/min · ...`, and every painted text.
fn hover_at(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
    at: egui::Pos2,
) -> (Option<(f64, f64)>, Vec<String>) {
    let mut p = Painted::default();
    for _ in 0..6 {
        p = inspector(ctx, state, width, vec![egui::Event::PointerMoved(at)]);
    }
    let reading = p.texts.iter().find_map(|(t, _)| {
        let (rpm, rest) = t.split_once(" RPM \u{00B7} ")?;
        let (feed, _) = rest.split_once(" mm/min")?;
        Some((rpm.parse().ok()?, feed.parse().ok()?))
    });
    (reading, p.texts.into_iter().map(|(t, _)| t).collect())
}

fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

// ── claim 1 ─────────────────────────────────────────────────────────────

#[test]
fn every_limit_in_the_range_draws_a_line_and_a_label_p5d() {
    let (config, env) = rivmap_env(None);
    let scene = EnvelopeScene::of(&env, None);

    // D2: the depth range, derived from the envelope.
    let hard_top = [LimitKind::CuttingLength, LimitKind::FluteGuard]
        .into_iter()
        .filter_map(|k| ceiling(&env, k))
        .fold(f64::INFINITY, f64::min);
    assert!(hard_top.is_finite(), "fixture: a hard top");
    let mut tallest: f64 = 0.0;
    for l in env.boundary.iter().filter(|l| !is_frame(l.kind)) {
        if let Edge::ApCeiling { mm } | Edge::ApCeilingAbove { mm, .. } = l.edge
            && mm < hard_top
        {
            tallest = tallest.max(mm);
        }
    }
    let stated = env.stated.expect("fixture: a stated point");
    tallest = tallest.max(stated.ap_mm).max(env.proposal.get().ap_mm);
    let ap_max = (VIEW_HEADROOM * tallest).min(hard_top);
    eprintln!("P5d: hard top {hard_top}, tallest {tallest}, ap_max {ap_max}");
    assert!(
        (scene.ap_range.1 - ap_max).abs() < 1e-9,
        "D2: the depth range {:?}, derived {ap_max}",
        scene.ap_range
    );
    assert!(ap_max < hard_top, "fixture: the view top is not a stop");
    assert!(scene.top.is_none(), "no frame top label under the hard top");
    let d = ceiling(&env, LimitKind::Diameter).expect("fixture: a diameter");
    assert_eq!(scene.ae_range, (0.0, d), "D2: the width range is 0..D");

    // D1: the drawn lines on the settled plot.
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let mut f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    for _ in 0..3 {
        f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    }
    let plot = f.response.frame.expect("a plot");
    let drawn: Vec<_> = scene.drawn_edges(plot.size());
    let mut kinds: Vec<LimitKind> = drawn.iter().map(|e| e.kind).collect();
    kinds.sort_by_key(|k| format!("{k:?}"));
    assert_eq!(
        kinds,
        vec![LimitKind::Rigidity, LimitKind::Slot, LimitKind::VendorAxial],
        "D1: the vendor, rigidity and slot limits draw at rest"
    );
    // Every non-frame limit whose edge enters the view is drawn, or hugs
    // the frame (D1 exception).
    for e in &scene.edges {
        assert!(!is_frame(e.kind), "{:?} is the frame", e.kind);
    }

    // D4: the names.
    let vendor = ceiling(&env, LimitKind::VendorAxial).expect("a vendor DOC");
    let label_of = |k: LimitKind| {
        drawn
            .iter()
            .find(|e| e.kind == k)
            .map(|e| e.label.clone())
            .expect("drawn")
    };
    let v = label_of(LimitKind::VendorAxial);
    let k = v
        .strip_prefix("Vendor DOC ")
        .and_then(|s| s.strip_suffix("\u{00D7}D"))
        .unwrap_or_else(|| panic!("D4: `Vendor DOC <k>×D`, got `{v}`"));
    assert!(
        k.parse::<f64>()
            .is_ok_and(|k| (k - vendor / d).abs() < 0.005),
        "D4: k = {vendor} / {d}, got `{k}`"
    );
    assert!(
        !k.contains('.') || !k.ends_with('0'),
        "D4: k is trimmed: `{k}`"
    );
    assert_eq!(v, limit_name(&env, LimitKind::VendorAxial));
    assert_eq!(label_of(LimitKind::Rigidity), "Rigidity DOC");
    assert_eq!(label_of(LimitKind::Slot), "Slot DOC");

    // D4: the colours. The binding line is TEXT_BODY and wider.
    assert_eq!(scene.binding, Some(LimitKind::VendorAxial), "fixture");
    for e in &drawn {
        let want = if Some(e.kind) == scene.binding {
            tokens::TEXT_BODY
        } else {
            tokens::TEXT_MUTED
        };
        assert_eq!(scene.edge_color(e), want, "{:?}", e.kind);
        assert_ne!(scene.edge_color(e), tokens::DANGER);
    }
    let body = drawn.iter().find(|e| e.kind == LimitKind::VendorAxial);
    let muted = drawn.iter().find(|e| e.kind == LimitKind::Rigidity);
    if let (Some(b), Some(m)) = (body, muted) {
        assert!(
            scene.edge_width(b) > scene.edge_width(m),
            "D4: muted is thinner"
        );
    }

    // One label per drawn line, its name, once.
    let p = texts(&f.shapes);
    for e in &drawn {
        let n = p.iter().filter(|t| **t == e.label).count();
        assert_eq!(n, 1, "`{}` paints once: {p:?}", e.label);
    }

    // D5: the D tick paints under the frame at the x of D.
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
    let d_text = format!("{d:.0}");
    let shapes = painted(&f.shapes);
    assert!(
        shapes.texts.iter().any(|(t, r)| *t == d_text
            && r.top() >= plot.bottom() - 1.0
            && (r.center().x - at_d.x).abs() < 4.0),
        "D5: the `{d_text}` tick paints at D: {:?}",
        shapes.texts
    );
}

// ── claim 2 ─────────────────────────────────────────────────────────────

#[test]
fn one_mark_rule_on_both_charts_p5d() {
    // The envelope plot: a stated depth over the vendor DOC (ap 8 at the
    // review width ae 2.303), where the proposal is not at the stored
    // point, so the diamond paints.
    let (config, env) = rivmap_env(Some((8.0, 2.303)));
    let scene = EnvelopeScene::of(&env, None);
    assert!(scene.proposal_shown, "fixture: the proposal is not at Now");
    assert_eq!(scene.proposal_color(), recommendation_color());
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    let mut rest = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    for _ in 0..3 {
        rest = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    }
    let diamonds = |p: &Painted| -> Vec<PathShape> {
        p.paths
            .iter()
            .filter(|s| s.closed && s.points.len() == 4 && s.color != egui::Color32::TRANSPARENT)
            .filter(|s| s.color != tokens::SURFACE_OVERLAY)
            .cloned()
            .collect()
    };
    let r = painted(&rest.shapes);
    let at_rest: Vec<PathShape> = diamonds(&r)
        .into_iter()
        .filter(|s| s.color == recommendation_color())
        .collect();
    assert_eq!(at_rest.len(), 1, "one quiet diamond: {:?}", diamonds(&r));
    assert!(
        (at_rest[0].width - RECOMMENDATION_WIDTH).abs() < 1e-6,
        "D3: a 1 pt outline"
    );
    assert!(
        r.circles
            .iter()
            .all(|c| c.fill != tokens::ACCENT && c.stroke != tokens::ACCENT),
        "D3: no full ACCENT mark"
    );
    // A drag on the plot: the diamond stays, the white dot moves.
    let resp = rest.response;
    let screen = |p| resp.screen_of(p, EnvelopeShape::Plane).expect("a frame");
    let to = CutPoint {
        ap_mm: resp.ap_range.1 * 0.25,
        ae_mm: resp.ae_range.1 * 0.6,
    };
    let from = screen(CutPoint {
        ap_mm: resp.ap_range.1 * 0.2,
        ae_mm: resp.ae_range.1 * 0.2,
    });
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![egui::Event::PointerMoved(from)],
    );
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![press(from, true)],
    );
    for k in 1..=10 {
        let at = from + (screen(to) - from) * (k as f32 / 10.0);
        let _ = frame(
            &ctx,
            &config,
            Some(&env),
            &mut candidates,
            vec![egui::Event::PointerMoved(at)],
        );
    }
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![press(screen(to), false)],
    );
    let dragged = painted(&frame(&ctx, &config, Some(&env), &mut candidates, Vec::new()).shapes);
    let got = candidates.get(ID).expect("the drag made a candidate");
    let quiet: Vec<PathShape> = diamonds(&dragged)
        .into_iter()
        .filter(|s| s.color == recommendation_color())
        .collect();
    assert_eq!(quiet.len(), 1, "still one quiet diamond");
    let centre = |s: &PathShape| {
        s.points
            .iter()
            .fold(egui::Vec2::ZERO, |a, q| a + q.to_vec2())
            / s.points.len() as f32
    };
    assert!(
        (centre(&quiet[0]) - centre(&at_rest[0])).length() < 0.5,
        "D3: no blue mark moves on drag"
    );
    let white: Vec<&Circle> = dragged
        .circles
        .iter()
        .filter(|c| c.fill == tokens::TEXT_STRONG || c.fill == tokens::CAUTION)
        .collect();
    assert_eq!(white.len(), 1, "one white dot: {white:?}");
    assert!(
        white[0].center.distance(screen(got)) < 2.0,
        "the envelope white dot is at the candidate"
    );
    let ghost: Vec<&Circle> = dragged
        .circles
        .iter()
        .filter(|c| c.fill == egui::Color32::TRANSPARENT && c.stroke == tokens::TEXT_MUTED)
        .collect();
    assert_eq!(ghost.len(), 1, "one envelope ghost while the draft exists");

    // The nomogram, in the inspector, on the fixture point.
    let width = 340.0;
    let (mut state, id) = rivmap_state();
    let ctx = envelope_k2b::ctx();
    let p0 = settled(&ctx, &mut state, width);
    let b = bands(&p0);
    let m0 = nomo_marks(&p0, &b);
    assert_eq!(m0.white.len(), 1, "one white dot at rest: {m0:?}");
    assert!(m0.ghost.is_empty(), "no nomogram ghost at rest: {m0:?}");
    assert_eq!(m0.quiet.len(), 1, "one quiet Recommended ring: {m0:?}");
    assert!(
        (m0.quiet[0].stroke_width - RECOMMENDATION_WIDTH).abs() < 1e-6,
        "D3: the ring is a 1 pt outline: {m0:?}"
    );
    assert!(m0.full_accent.is_empty(), "D3: no full ACCENT: {m0:?}");
    assert_eq!(
        m0.suggested.len(),
        1,
        "D3: one quiet suggested line: {m0:?}"
    );
    assert!(
        (m0.suggested[0].width - RECOMMENDATION_WIDTH).abs() < 1e-6,
        "D3: the suggested line is 1 pt"
    );
    assert!(
        !p0.texts
            .iter()
            .any(|(t, r)| t == "Recommended" && within(r.center().y, b.nomogram)),
        "D3: no painted Recommended label"
    );
    let stored_at = m0.white[0].center;
    let (stored, _) = hover_at(&ctx, &mut state, width, stored_at);
    let stored = stored.expect("the hover reads at the white dot");
    // The hover names the ring.
    let (_, ring_texts) = hover_at(&ctx, &mut state, width, m0.quiet[0].center);
    assert!(
        ring_texts.iter().any(|t| t == "Recommended"),
        "D3: the hover names the ring: {ring_texts:?}"
    );

    // A draft: the envelope candidate of a drag (the nomogram reads the
    // same candidate state).
    let (_, env0) = rivmap_env(None);
    let c = CutPoint {
        ap_mm: env0.stated.expect("stated").ap_mm * 0.5,
        ae_mm: env0.stated.expect("stated").ae_mm * 1.5,
    };
    let want = candidate_speeds(&env0, Some(c)).expect("the grid answers");
    state.panels.envelope_candidates.set(id, c);
    let p1 = settled(&ctx, &mut state, width);
    let m1 = nomo_marks(&p1, &bands(&p1));
    assert_eq!(m1.white.len(), 1, "one white dot in the draft: {m1:?}");
    assert_eq!(m1.ghost.len(), 1, "one nomogram ghost in the draft: {m1:?}");
    assert!(
        m1.ghost[0].center.distance(stored_at) < 1.0,
        "the ghost marks the stored speeds"
    );
    assert!(
        m1.quiet.len() == 1 && m1.quiet[0].center.distance(m0.quiet[0].center) < 0.5,
        "D3: the Recommended ring does not move: {m0:?} -> {m1:?}"
    );
    assert!(
        m1.suggested.len() == 1 && m1.suggested[0].points == m0.suggested[0].points,
        "D3: the suggested line does not move"
    );
    assert!(m1.full_accent.is_empty(), "D3: no full ACCENT: {m1:?}");
    let moved_to = m1.white[0].center;
    assert!(
        moved_to.distance(stored_at) > 3.0,
        "fixture: the draft speeds differ from the stored ones ({want:?} vs {stored:?})"
    );
    // The white dot sits at `candidate_speeds`: the hover under it reads
    // them, within two points of the axis scale.
    let (at_dot, _) = hover_at(&ctx, &mut state, width, moved_to);
    let (right, _) = hover_at(&ctx, &mut state, width, moved_to + egui::vec2(10.0, 0.0));
    let (up, _) = hover_at(&ctx, &mut state, width, moved_to + egui::vec2(0.0, -10.0));
    let (at_dot, right, up) = (
        at_dot.expect("a reading"),
        right.expect("a reading"),
        up.expect("a reading"),
    );
    let rpm_per_pt = ((right.0 - at_dot.0) / 10.0).abs().max(1.0);
    let feed_per_pt = ((up.1 - at_dot.1) / 10.0).abs().max(1.0);
    assert!(
        (at_dot.0 - want.rpm).abs() <= 2.0 * rpm_per_pt
            && (at_dot.1 - want.feed_mm_min).abs() <= 2.0 * feed_per_pt,
        "D3: the nomogram white dot reads {at_dot:?}, the pick writes {} rpm {} mm/min",
        want.rpm,
        want.feed_mm_min
    );

    // Esc returns both white dots.
    let _ = inspector(&ctx, &mut state, width, vec![escape()]);
    assert!(
        state.panels.envelope_candidates.get(id).is_none(),
        "Esc drops the draft"
    );
    let p2 = settled(&ctx, &mut state, width);
    let m2 = nomo_marks(&p2, &bands(&p2));
    assert!(m2.ghost.is_empty(), "no ghost after Esc: {m2:?}");
    assert!(
        m2.white.len() == 1 && m2.white[0].center.distance(stored_at) < 1.0,
        "the nomogram white dot is back: {m2:?}"
    );
}

// ── claim 3 ─────────────────────────────────────────────────────────────

/// The always-visible counts at rest; printed for the card (P5d "Mock").
#[test]
fn the_counts_at_340_and_240_p5d() {
    let (_, env) = rivmap_env(None);
    let want: Vec<String> = [LimitKind::VendorAxial, LimitKind::Rigidity, LimitKind::Slot]
        .into_iter()
        .map(|k| limit_name(&env, k))
        .collect();
    for width in [340.0_f32, 240.0] {
        let (mut state, _) = rivmap_state();
        let ctx = envelope_k2b::ctx();
        let p = settled(&ctx, &mut state, width);
        let b = bands(&p);
        let env_lines = p
            .paths
            .iter()
            .filter(|s| !s.closed && s.points.iter().all(|q| within(q.y, b.envelope)))
            .count();
        let env_labels: Vec<&str> = p
            .texts
            .iter()
            .filter(|(t, r)| within(r.center().y, b.envelope) && want.contains(t))
            .map(|(t, _)| t.as_str())
            .collect();
        let nomo_lines = p
            .paths
            .iter()
            .filter(|s| !s.closed && s.points.iter().all(|q| within(q.y, b.nomogram)))
            .count();
        let nomo_names = [
            "Recommended",
            "Max feed",
            "Max RPM",
            "Min RPM",
            "Rubbing",
            "Chip limit",
            "Vendor range",
            "Now",
            "Before derate",
        ];
        let nomo_labels: Vec<(&str, egui::Rect)> = p
            .texts
            .iter()
            .filter(|(t, r)| within(r.center().y, b.nomogram) && nomo_names.contains(&t.as_str()))
            .map(|(t, r)| (t.as_str(), *r))
            .collect();
        let rpm_ticks: Vec<&str> = p
            .texts
            .iter()
            .filter(|(t, r)| {
                t.parse::<f64>().is_ok()
                    && r.bottom() <= b.rpm_title.top() + 1.0
                    && r.top() > b.rpm_title.top() - 20.0
            })
            .map(|(t, _)| t.as_str())
            .collect();
        eprintln!(
            "P5d counts at {width}: envelope {env_lines} lines, {} labels {env_labels:?}; \
             nomogram {nomo_lines} lines, {} labels {:?}; RPM ticks {rpm_ticks:?}",
            env_labels.len(),
            nomo_labels.len(),
            nomo_labels.iter().map(|(t, _)| *t).collect::<Vec<_>>()
        );
        let mut got = env_labels.clone();
        got.sort_unstable();
        let mut all: Vec<&str> = want.iter().map(String::as_str).collect();
        all.sort_unstable();
        assert_eq!(
            got, all,
            "at {width}: every drawn envelope line has its label"
        );
        assert!(
            !nomo_labels.iter().any(|(t, _)| *t == "Recommended"),
            "at {width}: D3, no painted Recommended label"
        );
        assert!(
            rpm_ticks.len() >= 2,
            "at {width}: D5, the RPM axis names two ticks: {rpm_ticks:?}"
        );
        let rect = |name: &str| {
            nomo_labels
                .iter()
                .find(|(t, _)| *t == name)
                .map(|(_, r)| *r)
        };
        if let (Some(now), Some(max)) = (rect("Now"), rect("Max RPM")) {
            assert!(
                !now.expand(1.0).intersects(max),
                "at {width}: D5, `Now` {now:?} touches `Max RPM` {max:?}"
            );
        }
    }
}

// ── claim 4 (lead review): an edge off the large step ──────────────────

/// The testpiece_mini fixture's 1/4 in end mill on its back cavity Pocket:
/// a plane envelope whose width frame D is off every large tick step.
fn quarter_inch_pocket() -> (OperationConfig, SafeEnvelope, f64) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../rs_cam_core/tests/fixtures/testpiece_mini_2026-10-20_c3808eb0/testpiece_mini.toml"
    );
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(path))
        .expect("the testpiece_mini fixture loads");
    let tool = session
        .tools()
        .iter()
        .find(|t| t.name.starts_with("1/4 in flat end mill"))
        .expect("testpiece_mini has the 1/4 in end mill");
    let mut tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "Back cavity Z9.00")
        .expect("testpiece_mini has the back cavity")
        .clone();
    tc.tool_id = Some(tool.id);
    let env = session
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed envelope");
    (tc.operation, env, tool.diameter)
}

/// `v` with at most two decimals and no trailing zero.
fn trimmed(v: f64) -> String {
    let text = format!("{v:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Lead review of D5: when D is off the large step (6.35 mm on a 2.5 mm
/// step), the edge tick names D in full and no other width tick label
/// touches it.
#[test]
fn an_edge_off_the_step_names_itself_alone_p5d() {
    let (config, env, d) = quarter_inch_pocket();
    let scene = EnvelopeScene::of(&env, None);
    assert_eq!(scene.shape, EnvelopeShape::Plane, "fixture: a plane op");
    assert_eq!(scene.ae_range.1, d, "fixture: the width frame is D");
    assert!(
        (d * 2.0 - (d * 2.0).round()).abs() > 1e-6,
        "fixture: D {d} is off every half-mm step"
    );
    let want = trimmed(d);
    let ctx = ctx();
    for width in [360.0_f32, 240.0] {
        let mut candidates = EnvelopeCandidates::default();
        let mut f = None;
        for _ in 0..4 {
            let mut sent = Vec::new();
            let mut response = rs_cam_viz::ui::feeds::envelope::EnvelopeResponse::default();
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    response = rs_cam_viz::ui::feeds::envelope::draw_envelope(
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
            f = Some((painted(&out.shapes), response));
        }
        let (p, response) = f.expect("frames");
        let plot = response.frame.expect("a plot");
        let title = p
            .texts
            .iter()
            .find(|(t, _)| t == "Width (mm)")
            .map(|(_, r)| *r)
            .expect("the width title paints");
        let ticks: Vec<&(String, egui::Rect)> = p
            .texts
            .iter()
            .filter(|(t, r)| {
                t.parse::<f64>().is_ok()
                    && r.top() >= plot.bottom() - 1.0
                    && r.bottom() <= title.top() + 1.0
            })
            .collect();
        assert!(
            ticks.iter().any(|(t, _)| *t == want),
            "at {width}: the `{want}` tick paints at D: {ticks:?}"
        );
        for (i, a) in ticks.iter().enumerate() {
            for b in ticks.iter().skip(i + 1) {
                assert!(
                    !a.1.intersects(b.1),
                    "at {width}: the width ticks `{}` {:?} and `{}` {:?} overlap",
                    a.0,
                    a.1,
                    b.0,
                    b.1
                );
            }
        }
    }
}
