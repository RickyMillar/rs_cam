//! P17 (feeds contract, operator 2026-11-06, layout B): the sim width x
//! depth chart draws the cut samples on the envelope and a click seeks.
//!
//! # The claims
//!
//! 1. The pure helpers: a sample lands in its cell of the 48 x 32 grid, a
//!    sample past a range lands in the edge cell, the alpha of the densest
//!    cell is 1, the cluster share sums the densest cell and its eight
//!    neighbours, and at most `DOT_CAP` over-budget dots paint.
//! 2. One frame in a headless `egui::Context` at 340 points draws the
//!    chart of a fixture envelope with a hand-built sample set; a click at
//!    the screen position of an over-budget dot returns `seek` with that
//!    sample's move, and a click on a non-empty cell returns the cell's
//!    first move.
//! 3. At 240 and 340 points, every text the section paints stays inside
//!    the rail, the turned depth title included (the up4 trap: a turned
//!    text's `visual_bounding_rect` ignores its angle).
//!
//! Red before P17: `ui/sim_cut_chart.rs` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{click_at, ctx, envelope_for};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::envelope::SafeEnvelope;
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::tool_load::cut_force::CutPoint3;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeScene, VIEW_HEADROOM};
use rs_cam_viz::ui::sim_cut_chart::{
    ALPHA_BUCKETS, CellGrid, CutChartInput, CutChartResponse, DOT_CAP, GRID_COLUMNS, GRID_ROWS,
    draw_cut_chart, over_budget_dots,
};

/// The move of the one over-budget sample.
const OVER_MOVE: usize = 777;

/// The fixture: the Pocket envelope (a 6 mm end mill, the default machine
/// and material) and the force budget its rule gives (lead ruling 3: the
/// budget exists only under the `ForceBudget` rule).
fn fixture() -> (SafeEnvelope, f64, f64) {
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let (family, role) = config.feeds_style();
    let budget = match MachineProfile::default()
        .rigidity
        .depth_cap_rule(family, role)
        .expect("a Pocket has a depth-cap rule")
    {
        DepthCapRule::ForceBudget(n) => n,
        DepthCapRule::Factor(f) => panic!("a Pocket is under the force budget, not a factor {f}"),
    };
    let diameter = env
        .limit(rs_cam_core::feeds::envelope::LimitKind::Diameter)
        .map_or(6.0, |l| match l.edge {
            rs_cam_core::feeds::envelope::Edge::AeCeiling { mm } => mm,
            rs_cam_core::feeds::envelope::Edge::ApCeiling { .. }
            | rs_cam_core::feeds::envelope::Edge::ApFloor { .. }
            | rs_cam_core::feeds::envelope::Edge::AeFloor { .. }
            | rs_cam_core::feeds::envelope::Edge::ApCurve { .. }
            | rs_cam_core::feeds::envelope::Edge::PerPoint
            | rs_cam_core::feeds::envelope::Edge::Unmodelled { .. } => 6.0,
        });
    (env, budget, diameter)
}

/// A hand-built set: thirty steady samples at the planned point (under
/// the budget), one over-budget sample at half D and a third of the depth
/// range, at `OVER_MOVE`.
fn samples(env: &SafeEnvelope, budget: f64, diameter: f64) -> Vec<CutPoint3> {
    let scene = EnvelopeScene::of(env, None);
    let planned = env
        .stated
        .or(env.proposal.get().point())
        .expect("the fixture states or proposes a point");
    let mut out: Vec<CutPoint3> = (0..30)
        .map(|i| CutPoint3 {
            ae_mm: planned.ae_mm,
            ap_mm: planned.ap_mm,
            force_n: Some(budget * 0.5),
            weight_s: 1.0,
            move_index: i,
            sample_index: i,
        })
        .collect();
    out.push(CutPoint3 {
        ae_mm: diameter / 2.0,
        ap_mm: scene.ap_range.1 / 3.0,
        force_n: Some(budget * 1.5),
        weight_s: 1.0,
        move_index: OVER_MOVE,
        sample_index: 30,
    });
    out
}

/// One frame of the chart at `width`, with `events`.
fn frame(
    ctx: &egui::Context,
    width: f32,
    input: &CutChartInput<'_>,
    events: Vec<egui::Event>,
) -> (Vec<egui::epaint::ClippedShape>, CutChartResponse) {
    let mut response = CutChartResponse::default();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            response = draw_cut_chart(ui, input);
        },
    );
    out.textures_delta.clear();
    (out.shapes, response)
}

/// The painted texts of `shapes` with the x extent of each one, the angle
/// read.
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

// ── claim 1 ─────────────────────────────────────────────────────────────

#[test]
fn a_sample_lands_in_its_cell_and_the_cluster_sums_nine_cells_p17() {
    let x = (0.0, 100.0);
    let y = (0.0, 2.0);
    // Three seconds in the cell at the centre, one in a neighbour, one far
    // away, and one past both ranges (it clamps to the top-right cell).
    let centre = (50.5, 1.03, 3.0, 40);
    let neighbour = (52.5, 1.03, 1.0, 41);
    let far = (10.0, 0.2, 1.0, 42);
    let past = (150.0, 9.0, 1.0, 43);
    let grid = CellGrid::bin([centre, neighbour, far, past], x, y);

    let col = 24; // 50.5 of 100 on 48 columns
    let row = 16; // 1.03 of 2 on 32 rows
    let i = row * GRID_COLUMNS + col;
    assert_eq!(grid.index_of(50.5, 1.03), i, "the sample's cell");
    let cell = grid.cells().get(i).copied().expect("the cell");
    assert_eq!(cell.count, 1);
    assert_eq!(cell.first_move, Some(40));
    assert_eq!(
        grid.index_of(150.0, 9.0),
        (GRID_ROWS - 1) * GRID_COLUMNS + GRID_COLUMNS - 1,
        "a sample past the ranges clamps to the edge cell"
    );
    assert_eq!(
        grid.index_of(52.5, 1.03),
        i + 1,
        "the neighbour is one column over"
    );

    assert_eq!(grid.densest, Some(i));
    assert!(
        (grid.alpha(i) - 1.0).abs() < 1e-12,
        "the densest cell has alpha 1"
    );
    assert!(
        (grid.alpha(i + 1) - (1.0_f64 / 3.0).sqrt()).abs() < 1e-12,
        "a neighbour's alpha is the square root of its share of the densest"
    );
    assert_eq!(grid.alpha(0), 0.0, "an empty cell has alpha 0");
    assert_eq!(
        grid.bucket(i),
        Some(ALPHA_BUCKETS - 1),
        "the densest cell is in the last bucket"
    );
    assert_eq!(grid.bucket(0), None);

    let (densest, share) = grid.cluster().expect("a cluster");
    assert_eq!(densest, i);
    assert!(
        (share - 4.0 / 6.0).abs() < 1e-12,
        "the cluster is the densest cell and its neighbours: 4 of 6 s, not {share}"
    );
    assert_eq!(CellGrid::neighbourhood(i).len(), 9);
    assert_eq!(
        CellGrid::neighbourhood(0).len(),
        4,
        "a corner has three neighbours"
    );
}

#[test]
fn the_over_budget_dots_are_capped_p17() {
    let budget = 10.0;
    let many: Vec<CutPoint3> = (0..1000)
        .map(|i| CutPoint3 {
            ae_mm: 1.0,
            ap_mm: 1.0,
            force_n: Some(if i % 2 == 0 { 20.0 } else { 5.0 }),
            weight_s: f64::from(i),
            move_index: i as usize,
            sample_index: i as usize,
        })
        .collect();
    let (shown, hidden) = over_budget_dots(&many, Some(budget));
    assert!(shown.len() <= DOT_CAP, "{} dots paint", shown.len());
    assert_eq!(shown.len() + hidden, 500, "500 samples are over the budget");
    assert!(
        shown.iter().all(|s| s.force_n.is_some_and(|f| f > budget)),
        "every painted dot is over the budget"
    );
    assert_eq!(
        shown.first().map(|s| s.weight_s),
        Some(998.0),
        "the heaviest sample paints first"
    );
    let (none, hidden) = over_budget_dots(&many, None);
    assert!(none.is_empty() && hidden == 0, "no line, no dot");
}

// ── claim 2 ─────────────────────────────────────────────────────────────

#[test]
fn a_click_on_a_dot_seeks_to_its_move_p17() {
    let (env, budget, diameter) = fixture();
    let samples = samples(&env, budget, diameter);
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        playhead: Some(10),
        move_count: 1000,
    };
    let ctx = ctx();
    let (_, first) = frame(&ctx, 340.0, &input, Vec::new());
    let map = first.map.expect("the chart drew");
    let scene = EnvelopeScene::of(&env, None);
    let over = samples.last().expect("the over-budget sample");
    let dot = map.to_screen(scene.ae_share(over.ae_mm), over.ap_mm);
    assert!(map.frame.contains(dot), "the dot is on the plot: {dot:?}");

    let mut seek = None;
    for events in click_at(dot) {
        let (_, response) = frame(&ctx, 340.0, &input, events);
        seek = seek.or(response.seek);
    }
    assert_eq!(
        seek,
        Some(OVER_MOVE),
        "a click on the dot seeks to its move"
    );

    // A click on the centre of the steady cell seeks to its first move in
    // trace order (the planned point may sit on a cell edge).
    let planned = samples.first().expect("a steady sample");
    let grid = CellGrid::bin(
        samples
            .iter()
            .map(|s| (scene.ae_share(s.ae_mm), s.ap_mm, s.weight_s, s.move_index)),
        (0.0, 100.0),
        scene.ap_range,
    );
    let (cx, cy) = grid.centre(grid.index_of(scene.ae_share(planned.ae_mm), planned.ap_mm));
    let cell = map.to_screen(cx, cy);
    let mut seek = None;
    for events in click_at(cell) {
        let (_, response) = frame(&ctx, 340.0, &input, events);
        seek = seek.or(response.seek);
    }
    assert_eq!(seek, Some(0), "a click on a cell seeks to its first sample");

    // A click on an empty cell seeks nowhere.
    let empty = egui::pos2(
        map.frame.right() - 2.0,
        map.to_screen(0.0, scene.ap_range.1 * 0.95).y,
    );
    let mut seek = None;
    for events in click_at(empty) {
        let (_, response) = frame(&ctx, 340.0, &input, events);
        seek = seek.or(response.seek);
    }
    assert_eq!(seek, None, "an empty cell has no move to seek");
}

// ── round 2: the depth range holds every sample ─────────────────────────

/// Lead review, round 2: a sample deeper than the envelope's own view
/// widens the chart's depth range (`VIEW_HEADROOM` over the deepest
/// sample, never past the hard top), so it does not clamp into the top
/// edge cell.
#[test]
fn a_deep_sample_widens_the_depth_range_p17() {
    let (env, budget, diameter) = fixture();
    let own = EnvelopeScene::of(&env, None);
    let mut samples = samples(&env, budget, diameter);
    let deep = own.ap_range.1 * 1.2;
    samples.push(CutPoint3 {
        ae_mm: diameter / 4.0,
        ap_mm: deep,
        force_n: Some(budget * 0.5),
        weight_s: 1.0,
        move_index: 900,
        sample_index: 31,
    });
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        playhead: None,
        move_count: 1000,
    };
    let ctx = ctx();
    let (_, response) = frame(&ctx, 340.0, &input, Vec::new());
    let map = response.map.expect("the chart drew");
    assert!(
        map.y.1 >= deep,
        "the depth range {:?} must hold the deepest sample {deep}",
        map.y
    );
    assert!(
        map.y.1 <= deep * VIEW_HEADROOM + 1e-6,
        "the range is at most VIEW_HEADROOM over the deepest sample (the hard top under it): {:?}",
        map.y
    );
    let grid = CellGrid::bin(
        samples
            .iter()
            .map(|s| (own.ae_share(s.ae_mm), s.ap_mm, s.weight_s, s.move_index)),
        (0.0, 100.0),
        map.y,
    );
    let (_, row) = CellGrid::col_row(grid.index_of(own.ae_share(diameter / 4.0), deep));
    assert!(
        row < GRID_ROWS - 1,
        "the deep sample sits inside the frame, not in the top edge cell (row {row})"
    );
}

// ── round 3: a limit above the range is an off-scale marker ─────────────

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
);

/// The `mm` of the flat depth ceiling of `kind`, when `env` has it.
fn flat_ceiling(env: &SafeEnvelope, kind: rs_cam_core::feeds::envelope::LimitKind) -> Option<f64> {
    match env.limit(kind)?.edge {
        rs_cam_core::feeds::envelope::Edge::ApCeiling { mm } => Some(mm),
        rs_cam_core::feeds::envelope::Edge::AeCeiling { .. }
        | rs_cam_core::feeds::envelope::Edge::ApFloor { .. }
        | rs_cam_core::feeds::envelope::Edge::AeFloor { .. }
        | rs_cam_core::feeds::envelope::Edge::ApCurve { .. }
        | rs_cam_core::feeds::envelope::Edge::PerPoint
        | rs_cam_core::feeds::envelope::Edge::Unmodelled { .. } => None,
    }
}

/// Lead review, round 3: the rivmap100 3D Rough has two flat depth limits
/// (the vendor DOC and the rigidity DOC). With the samples deep enough
/// that the range holds the lower one and not the higher, the lower draws
/// as a line with its name, and the higher paints as an off-scale marker
/// at the top edge: its name and its value.
#[test]
fn a_limit_above_the_range_is_an_off_scale_marker_p17() {
    use rs_cam_core::feeds::envelope::LimitKind;
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(FIXTURE))
        .expect("the rivmap100 fixture loads");
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .clone();
    let env = session
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed envelope");
    let low = flat_ceiling(&env, LimitKind::VendorAxial).expect("the vendor DOC is flat");
    let high = flat_ceiling(&env, LimitKind::Rigidity).expect("the rigidity DOC is flat");
    assert!(low < high, "fixture: vendor {low} under rigidity {high}");
    // The deepest sample puts the range between the two.
    let deep = (low + high) / 2.0 / VIEW_HEADROOM;
    let samples: Vec<CutPoint3> = (0..20)
        .map(|i| CutPoint3 {
            ae_mm: 1.0,
            ap_mm: if i == 0 { deep } else { deep * 0.3 },
            force_n: None,
            weight_s: 1.0,
            move_index: i,
            sample_index: i,
        })
        .collect();
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: 6.0,
        budget_n: None,
        playhead: None,
        move_count: 20,
    };
    let ctx = ctx();
    // Round 4: at the real rail (about 290 points) both marker labels
    // paint: the samples put the range under BOTH flat limits.
    let shallow: Vec<CutPoint3> = samples
        .iter()
        .map(|s| CutPoint3 {
            ap_mm: s.ap_mm * 0.3,
            ..*s
        })
        .collect();
    let two_markers = CutChartInput {
        samples: &shallow,
        ..input
    };
    for width in [340.0_f32, 290.0] {
        let (shapes, response) = frame(&ctx, width, &two_markers, Vec::new());
        let map = response.map.expect("the chart drew");
        assert!(map.y.1 < low, "both limits above the range {:?}", map.y);
        let extents = text_extents(&shapes);
        let texts: Vec<String> = extents.iter().map(|t| t.0.clone()).collect();
        for (kind, mm) in [(LimitKind::VendorAxial, low), (LimitKind::Rigidity, high)] {
            let marker = format!(
                "{} {} mm",
                rs_cam_viz::ui::feeds::envelope::limit_name(&env, kind),
                rs_cam_viz::ui::components::histogram::format_value(mm)
            );
            let (_, _, right, rect) = extents
                .iter()
                .find(|t| t.0 == marker)
                .unwrap_or_else(|| panic!("at {width}: the marker {marker:?}: {texts:?}"));
            // Round 5: the row sits ABOVE the frame, right-aligned to it.
            assert!(
                rect.bottom() <= map.frame.top() + 0.5,
                "at {width}: {marker:?} paints above the frame top: {rect:?} against {:?}",
                map.frame
            );
            assert!(
                (right - map.frame.right()).abs() <= 1.0,
                "at {width}: {marker:?} is right-aligned to the frame: {right} vs {}",
                map.frame.right()
            );
        }
    }

    let (shapes, response) = frame(&ctx, 340.0, &input, Vec::new());
    let map = response.map.expect("the chart drew");
    assert!(
        map.y.1 > low && map.y.1 < high,
        "the range {:?} holds the vendor DOC {low} and not the rigidity DOC {high}",
        map.y
    );
    let extents = text_extents(&shapes);
    let texts: Vec<String> = extents.iter().map(|t| t.0.clone()).collect();
    let low_name = rs_cam_viz::ui::feeds::envelope::limit_name(&env, LimitKind::VendorAxial);
    let high_name = rs_cam_viz::ui::feeds::envelope::limit_name(&env, LimitKind::Rigidity);
    assert!(
        texts.contains(&low_name),
        "the limit inside the range paints as a line with its name {low_name:?}: {texts:?}"
    );
    let marker = format!(
        "{high_name} {} mm",
        rs_cam_viz::ui::components::histogram::format_value(high)
    );
    let (_, _, _, rect) = extents
        .iter()
        .find(|t| t.0 == marker)
        .unwrap_or_else(|| panic!("the limit above the range paints {marker:?}: {texts:?}"));
    assert!(
        rect.bottom() <= map.frame.top() + 0.5,
        "the off-scale row sits above the frame: {rect:?} against {:?}",
        map.frame
    );
    assert!(
        !texts.contains(&high_name),
        "the limit above the range paints no line label: {texts:?}"
    );
}

// ── round 5: the legend names the dots only when one painted ────────────

#[test]
fn the_legend_lists_over_budget_only_with_a_dot_p17() {
    let (env, budget, diameter) = fixture();
    let mut samples = samples(&env, budget, diameter);
    // Every sample under the budget: a line exists, no dot paints.
    for s in &mut samples {
        s.force_n = Some(budget * 0.5);
    }
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        playhead: None,
        move_count: 31,
    };
    let ctx = ctx();
    let (shapes, _) = frame(&ctx, 340.0, &input, Vec::new());
    let texts: Vec<String> = text_extents(&shapes).into_iter().map(|t| t.0).collect();
    assert!(
        texts.iter().any(|t| t == "cut time") && texts.iter().any(|t| t == "planned"),
        "non-vacuity: the legend paints: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "over budget"),
        "no dot painted, so the legend names none: {texts:?}"
    );
}

// ── claim 3 ─────────────────────────────────────────────────────────────

#[test]
fn every_chart_text_stays_inside_the_rail_p17() {
    let (env, budget, diameter) = fixture();
    let samples = samples(&env, budget, diameter);
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        playhead: None,
        move_count: 30_591,
    };
    let ctx = ctx();
    for width in [240.0_f32, 340.0] {
        let mut last = None;
        for _ in 0..4 {
            let (shapes, _) = frame(&ctx, width, &input, Vec::new());
            last = Some(shapes);
        }
        let extents = text_extents(&last.expect("frames"));
        for anchor in ["DEPTH (mm)", "steady passes", "cut time", "over budget"] {
            assert!(
                extents.iter().any(|(t, _, _, _)| t.contains(anchor)),
                "non-vacuity at {width}: `{anchor}` paints: {extents:?}"
            );
        }
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
