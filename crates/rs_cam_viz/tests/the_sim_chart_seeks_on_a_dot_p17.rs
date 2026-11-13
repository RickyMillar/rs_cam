//! P17 (feeds contract, operator 2026-11-06, layout B) and P17b (operator
//! 2026-11-07): the sim width x depth chart draws the cut samples on the
//! envelope as a heat map with iso-force contours, and a click seeks.
//!
//! # The claims
//!
//! 1. The pure helpers: a sample lands in its cell of the 48 x 32 grid, a
//!    sample past a range lands in the edge cell, the heat of the densest
//!    cell is 1, and at most `DOT_CAP` over-budget dots paint.
//! 2. One frame in a headless `egui::Context` at 340 points draws the
//!    chart of a fixture envelope with a hand-built sample set; a click at
//!    the screen position of an over-budget dot returns `seek` with that
//!    sample's move, and a click on a non-empty cell returns the cell's
//!    first move.
//! 3. At 240 and 340 points, every text the section paints stays inside
//!    the rail, the turned depth title included (the up4 trap: a turned
//!    text's `visual_bounding_rect` ignores its angle).
//! 4. P17b item 1: the heat ramp is monotone and the densest cell is the
//!    brightest.
//! 5. P17b item 2: a flat limit above the data range extends the axis and
//!    paints as a named line; a limit more than `OFF_SCALE_FACTOR` times
//!    the range keeps one quiet off-scale row above the frame.
//! 6. P17b item 3: `contours` on a hand-built field gives one
//!    frame-to-frame polyline per level of a plane field and one closed
//!    loop of a radial field, each at its level; the chart names the
//!    contours on their lines.
//! 7. P17b item 5: the planned dot is the last mark painted and paints
//!    whole over an over-budget dot at the same point.
//!
//! Red before P17: `ui/sim_cut_chart.rs` does not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{click_at, ctx, envelope_for};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::envelope::SafeEnvelope;
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::tool_load::cut_force::CutPoint3;
use rs_cam_core::tool_load::force_field::{
    BudgetRole, CONTOUR_SHARES, ForceField, ForceLevel, ForcePlane, contours,
};
use rs_cam_viz::state::simulation::force_field::field_top;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeScene, VIEW_HEADROOM};
use rs_cam_viz::ui::sim_cut_chart::{
    CellGrid, CutChartInput, CutChartResponse, DENSEST_ALPHA, DOT_CAP, FAINTEST_ALPHA,
    GRID_COLUMNS, GRID_ROWS, HEAT_STEPS, OFF_SCALE_FACTOR, contour_label, depth_view,
    draw_cut_chart, over_budget_dots, square_rows, step_alpha, step_color,
};
use rs_cam_viz::ui::tokens;

/// The move of the one over-budget sample.
const OVER_MOVE: usize = 777;

/// The fixture: the Pocket envelope (a 6 mm end mill, the default machine
/// and material) and the force budget its rule gives (lead ruling 3: the
/// budget exists only under a depth-cap rule).
fn fixture() -> (SafeEnvelope, f64, f64) {
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let DepthCapRule { budget_n: budget } = MachineProfile::default()
        .rigidity
        .depth_cap_of(
            config
                .cuts(&rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&config))
                .body_facts()
                .depth_rule(),
        )
        .expect("a Pocket has a depth-cap rule");
    let diameter = env
        .limit(rs_cam_core::feeds::envelope::LimitKind::Diameter)
        .map_or(6.0, |l| match l.edge {
            rs_cam_core::feeds::envelope::Edge::AeCeiling { mm } => mm,
            rs_cam_core::feeds::envelope::Edge::ApCeiling { .. }
            | rs_cam_core::feeds::envelope::Edge::ApFloor { .. }
            | rs_cam_core::feeds::envelope::Edge::AeFloor { .. }
            | rs_cam_core::feeds::envelope::Edge::ApCurve { .. }
            | rs_cam_core::feeds::envelope::Edge::PerPoint
            | rs_cam_core::feeds::envelope::Edge::AeCurve { .. }
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

/// Every shape of `shapes`, a `Shape::Vec` opened.
fn flat_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Shape> {
    let mut out = Vec::new();
    for c in shapes {
        if let egui::Shape::Vec(v) = &c.shape {
            out.extend(v.iter().cloned());
        } else {
            out.push(c.shape.clone());
        }
    }
    out
}

/// The painted texts of `shapes` with the x extent of each one, the angle
/// read.
fn text_extents(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, f32, f32, egui::Rect)> {
    flat_shapes(shapes)
        .iter()
        .filter_map(|s| {
            let egui::Shape::Text(text) = s else {
                return None;
            };
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
            Some((
                text.galley.job.text.clone(),
                left,
                right,
                text.visual_bounding_rect(),
            ))
        })
        .collect()
}

// ── claim 1 ─────────────────────────────────────────────────────────────

#[test]
fn a_sample_lands_in_its_cell_p17() {
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
    assert_eq!(grid.y_range(), y, "the grid keeps its data range");

    assert_eq!(grid.densest, Some(i));
    assert!(
        (grid.alpha(i) - 1.0).abs() < 1e-12,
        "the densest cell has heat 1"
    );
    // P17b: the heat is the rank of the cell's time among the four cells
    // that hold time (3, 1, 1, 1 s): three of the four hold at most 1 s.
    assert!(
        (grid.alpha(i + 1) - 0.75).abs() < 1e-12,
        "a neighbour's heat is its rank among the cells that hold time"
    );
    assert_eq!(grid.alpha(0), 0.0, "an empty cell has heat 0");
    assert_eq!(
        grid.step(i),
        Some(HEAT_STEPS - 1),
        "the densest cell is in the last step"
    );
    assert_eq!(grid.step(0), None);
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

// ── claim 4 (P17b item 1): the ramp ─────────────────────────────────────

/// The brightness of a colour on the dark ground: its alpha-weighted
/// luma (the ground is darker than every ink rung, so a higher alpha of
/// one hue is brighter).
fn brightness(c: egui::Color32) -> f32 {
    let [r, g, b, a] = c.to_array();
    (f32::from(r) * 0.299 + f32::from(g) * 0.587 + f32::from(b) * 0.114) * f32::from(a)
}

#[test]
fn the_heat_ramp_is_monotone_and_the_densest_cell_is_the_brightest_p17b() {
    // At least 32 steps: held at compile time.
    const _: () = assert!(HEAT_STEPS >= 32);
    assert!(
        (step_alpha(0) - FAINTEST_ALPHA).abs() < 1e-6
            && (step_alpha(HEAT_STEPS - 1) - DENSEST_ALPHA).abs() < 1e-6,
        "the ramp runs from the faintest to the densest alpha"
    );
    assert!(
        (DENSEST_ALPHA - 1.0).abs() < 1e-6,
        "the densest cell is at full strength"
    );
    for s in 1..HEAT_STEPS {
        assert!(
            step_alpha(s) > step_alpha(s - 1),
            "the ramp rises at step {s}"
        );
        assert!(
            brightness(step_color(s)) > brightness(step_color(s - 1)),
            "the colour brightens at step {s}"
        );
    }
    // A grid with one dense cell and one scattered sample: the dense cell
    // takes the last step, the scattered one a faint step, and no cell is
    // brighter than the densest.
    let grid = CellGrid::bin(
        [(50.0, 1.0, 9.0, 1), (10.0, 0.2, 1.0, 2)],
        (0.0, 100.0),
        (0.0, 2.0),
    );
    let dense = grid.densest.expect("a densest cell");
    let faint = grid.index_of(10.0, 0.2);
    assert_eq!(grid.step(dense), Some(HEAT_STEPS - 1));
    let faint_step = grid.step(faint).expect("the scattered sample paints");
    assert!(
        faint_step < HEAT_STEPS / 2,
        "the cell with less time ranks first of two, the lower half of the ramp: step {faint_step}"
    );
    assert!(faint_step > 0 || FAINTEST_ALPHA > 0.0, "and still reads");
    for i in 0..grid.cells().len() {
        if let Some(s) = grid.step(i) {
            assert!(s < HEAT_STEPS && step_alpha(s) <= step_alpha(HEAT_STEPS - 1));
        }
    }
}

#[test]
fn the_force_field_reaches_the_deepest_sample_p17b() {
    // P17b round 1 (live, rivmap Face 5): the field stopped at the
    // envelope's own view top, under samples twice as deep, so no contour
    // drew. The field top is the top of the chart's own scene, at or over
    // the deepest sample.
    let (env, budget, diameter) = fixture();
    let own_top = EnvelopeScene::of(&env, None).ap_range.1;
    let deep = 2.0 * own_top;
    let mut set = samples(&env, budget, diameter);
    set.push(CutPoint3 {
        ae_mm: diameter / 2.0,
        ap_mm: deep,
        force_n: None,
        weight_s: 1.0,
        move_index: 1,
        sample_index: set.len(),
    });
    let top = field_top(&env, &set);
    assert!(
        top >= deep,
        "the field top {top} mm covers the deepest sample {deep} mm (own view top {own_top} mm)"
    );
    assert!(
        (field_top(&env, &[]) - own_top).abs() < 1e-9,
        "with no sample the field top is the envelope's own view top"
    );
}

#[test]
fn one_heavy_cell_does_not_fade_the_rest_p17b() {
    // The live rivmap rough (P17b round 1): most of the time sat in one
    // cell, so a ramp on the share of the densest cell drew every other
    // cell at the faint end. On the rank ramp the cells keep their order
    // and spread over the ramp: 1 s, 2 s, 3 s and a 1000 s cell.
    let grid = CellGrid::bin(
        [
            (10.0, 0.2, 1.0, 1),
            (30.0, 0.2, 2.0, 2),
            (50.0, 0.2, 3.0, 3),
            (70.0, 0.2, 1000.0, 4),
        ],
        (0.0, 100.0),
        (0.0, 2.0),
    );
    let steps: Vec<usize> = [10.0, 30.0, 50.0, 70.0]
        .iter()
        .map(|x| grid.step(grid.index_of(*x, 0.2)).expect("a cell with time"))
        .collect();
    assert!(
        steps.windows(2).all(|w| matches!(w, [a, b] if a < b)),
        "more time, a brighter step: {steps:?}"
    );
    assert!(
        steps.get(2).is_some_and(|s| *s >= HEAT_STEPS / 2),
        "the 3 s cell is in the upper half next to a 1000 s cell: {steps:?}"
    );
}

#[test]
fn the_heat_cells_are_square_on_the_screen_p17b() {
    // 48 columns over 240 pt: 5 pt a cell; a data range 40 pt tall takes 8
    // rows of 5 pt.
    assert_eq!(square_rows(40.0, 240.0), 8);
    assert_eq!(square_rows(0.5, 240.0), 1, "at least one row");
    assert_eq!(square_rows(4000.0, 240.0), GRID_ROWS, "at most GRID_ROWS");
    let grid = CellGrid::bin_rows([(50.0, 1.9, 1.0, 1)], (0.0, 100.0), (0.0, 2.0), 8);
    assert_eq!(grid.rows(), 8);
    assert_eq!(grid.cells().len(), GRID_COLUMNS * 8);
    let q = grid.quad(grid.index_of(50.0, 1.9));
    assert!(
        (q[2][1] - 2.0).abs() < 1e-12 && (q[0][1] - 1.75).abs() < 1e-12,
        "the top row spans the last eighth of the depth range: {q:?}"
    );
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
        force_plane: None,
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
    // trace order (the planned point may sit on a cell edge). The grid
    // covers the DATA range: `VIEW_HEADROOM` over the deepest sample and
    // the planned depth, never past the scene top.
    let planned = samples.first().expect("a steady sample");
    let deepest = samples.iter().map(|s| s.ap_mm).fold(0.0_f64, f64::max);
    let data_top = (VIEW_HEADROOM * deepest.max(planned.ap_mm)).min(scene.ap_range.1);
    let grid = CellGrid::bin(
        samples
            .iter()
            .map(|s| (scene.ae_share(s.ae_mm), s.ap_mm, s.weight_s, s.move_index)),
        (0.0, 100.0),
        (0.0, data_top),
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
        map.to_screen(0.0, map.y.1 * 0.95).y,
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
        force_plane: None,
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

// ── claim 5 (P17b item 2): a limit above the range ──────────────────────

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
        | rs_cam_core::feeds::envelope::Edge::AeCurve { .. }
        | rs_cam_core::feeds::envelope::Edge::Unmodelled { .. } => None,
    }
}

/// The rivmap100 3D Rough envelope and its one flat depth limit, the
/// vendor DOC. P29-4 (ruling R1): before P29-4 the rigidity DOC
/// (`adaptive_doc_factor x D`, 9 mm) was a second flat limit above it. The
/// Adaptive family now reads the force budget, a curve over the width, and
/// the chart extends to and notes only a flat limit (`depth_view`), as for
/// the Pocket curve since P13c.
fn rough_env() -> (SafeEnvelope, f64) {
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
        .safe_envelope(&tc, rs_cam_viz::state::simulation::run_modulation())
        .expect("a tool")
        .expect("a backed envelope");
    let low = flat_ceiling(&env, LimitKind::VendorAxial).expect("the vendor DOC is flat");
    assert!(
        matches!(
            env.limit(LimitKind::Rigidity).map(|l| &l.edge),
            Some(rs_cam_core::feeds::envelope::Edge::ApCurve { .. })
        ),
        "fixture: the rigidity limit is the force-budget curve: {:?}",
        env.limit(LimitKind::Rigidity)
    );
    (env, low)
}

/// Twenty samples, the first at `deep`, the rest at a third of it.
fn rough_samples(deep: f64) -> Vec<CutPoint3> {
    (0..20)
        .map(|i| CutPoint3 {
            ae_mm: 1.0,
            ap_mm: if i == 0 { deep } else { deep * 0.3 },
            force_n: None,
            weight_s: 1.0,
            move_index: i,
            sample_index: i,
        })
        .collect()
}

/// The chart input of the rough envelope `env` over `samples`: no budget
/// line and no field (the axis claims read the depth limits only).
fn rough_input<'a>(env: &'a SafeEnvelope, samples: &'a [CutPoint3]) -> CutChartInput<'a> {
    CutChartInput {
        env,
        samples,
        diameter_mm: 6.0,
        budget_n: None,
        force_plane: None,
        playhead: None,
        move_count: 20,
    }
}

/// P17b item 2 (operator: "I cant see what the axis lines are"). On the
/// rivmap100 3D Rough:
///
/// - A cut whose data range is under the vendor DOC and within
///   `OFF_SCALE_FACTOR` of it: the axis extends to the vendor DOC, which
///   paints as a line named on it inside the frame, with no note.
/// - A cut whose data range is more than `OFF_SCALE_FACTOR` under the
///   vendor DOC: no extension; the vendor DOC is an off-scale row.
///
/// P29-4 (ruling R1): the arms of a second, higher flat limit (its quiet
/// note above the frame, and the axis extended to it) are deleted: the
/// rigidity DOC of this fixture is a curve now. P29-7 7c: the vendor depth
/// table reads the first slot of this ContourParallel rough (the largest
/// chip share), so it is a flat limit above the vendor DOC (2.9 x D); the
/// pure view of case B gives it an off-scale row too.
#[test]
fn a_limit_above_the_range_extends_the_axis_or_keeps_a_quiet_note_p17b() {
    use rs_cam_core::feeds::envelope::LimitKind;
    let (env, low) = rough_env();
    let low_name = rs_cam_viz::ui::feeds::envelope::limit_name(&env, LimitKind::VendorAxial);
    let table_name = rs_cam_viz::ui::feeds::envelope::limit_name(&env, LimitKind::VendorDepthTable);
    let table_mm = match env.limit(LimitKind::VendorDepthTable).map(|l| &l.edge) {
        Some(rs_cam_core::feeds::envelope::Edge::ApCeiling { mm }) => *mm,
        other => panic!("fixture: a flat depth table, got {other:?}"),
    };
    let marker = |name: &str, mm: f64| {
        format!(
            "{name} {} mm",
            rs_cam_viz::ui::components::histogram::format_value(mm)
        )
    };
    let planned = env.stated.map_or(0.0, |p| p.ap_mm);
    let ctx = ctx();

    // Case A: the vendor DOC within the factor.
    let deep = low / OFF_SCALE_FACTOR / VIEW_HEADROOM * 1.2;
    let deep = deep.max(planned);
    let data_top = VIEW_HEADROOM * deep;
    assert!(
        data_top < low && low <= OFF_SCALE_FACTOR * data_top,
        "fixture: the vendor DOC {low} is above the data range {data_top} and within the factor"
    );
    let extended = rough_samples(deep);
    let extended = rough_input(&env, &extended);
    for width in [340.0_f32, 290.0] {
        let (shapes, response) = frame(&ctx, width, &extended, Vec::new());
        let map = response.map.expect("the chart drew");
        assert!(
            map.y.1 > low,
            "at {width}: the axis {:?} extends past the vendor DOC {low}",
            map.y
        );
        let extents = text_extents(&shapes);
        let texts: Vec<String> = extents.iter().map(|t| t.0.clone()).collect();
        let (_, _, _, rect) = extents.iter().find(|t| t.0 == low_name).unwrap_or_else(|| {
            panic!("at {width}: the vendor DOC is named on its line: {texts:?}")
        });
        assert!(
            map.frame.contains_rect(rect.shrink(1.0)),
            "at {width}: the name sits inside the frame: {rect:?} in {:?}",
            map.frame
        );
        let line_y = map.to_screen(0.0, low).y;
        assert!(
            line_y > map.frame.top() + 2.0,
            "at {width}: the line draws inside the frame, not on its top edge: y {line_y} vs {}",
            map.frame.top()
        );
        assert!(
            !texts.contains(&marker(&low_name, low)),
            "at {width}: the extended-to limit has no note: {texts:?}"
        );
    }

    // Case B: the vendor DOC more than the factor above the range. The
    // drawn chart always holds the planned 2 mm of this fixture in its
    // data range, which puts the vendor DOC within the factor, so this
    // arm reads the pure view: a shallow cut planned shallow.
    let shallow = low / OFF_SCALE_FACTOR / VIEW_HEADROOM * 0.5;
    let scene = EnvelopeScene::of(&env, None);
    let view = depth_view(&scene, shallow, shallow, 0.1);
    assert!(
        view.extended_to.is_none() && view.ap_top < low / OFF_SCALE_FACTOR,
        "the axis does not extend to a limit more than {OFF_SCALE_FACTOR}x the range: {view:?}"
    );
    assert_eq!(
        view.off_scale
            .iter()
            .map(|(name, mm)| (name.as_str(), *mm))
            .collect::<Vec<_>>(),
        vec![(low_name.as_str(), low), (table_name.as_str(), table_mm)],
        "the vendor DOC and the depth table keep a note; the rigidity curve has none"
    );
    assert!(
        (view.data_top - VIEW_HEADROOM * shallow).abs() < 1e-9 && view.ap_top == view.data_top,
        "the axis is the data range: {view:?}"
    );
}

// ── claim 6 (P17b item 3): the contours ─────────────────────────────────

fn nodes(top: f64, n: usize) -> Vec<f64> {
    (0..n).map(|k| top * k as f64 / (n - 1) as f64).collect()
}

#[test]
fn contours_trace_a_plane_and_a_radial_field_at_their_levels_p17b() {
    // The plane f = ap on 6 x 4 mm: each level is one frame-to-frame line
    // at ap = level.
    let plane = ForceField::from_fn(nodes(6.0, 13), nodes(4.0, 9), |_, ap| ap);
    for level in [1.0, 2.0, 3.0] {
        let lines = contours(&plane, level);
        assert_eq!(lines.len(), 1, "one line at {level}: {lines:?}");
        let xs: Vec<f64> = lines[0].iter().map(|p| p[0]).collect();
        assert!(
            lines[0].iter().all(|p| (p[1] - level).abs() < 1e-12),
            "at ap {level}: {:?}",
            lines[0]
        );
        assert!(
            xs.iter().copied().fold(f64::INFINITY, f64::min).abs() < 1e-12
                && (xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 6.0).abs() < 1e-12,
            "frame to frame: {xs:?}"
        );
    }
    // The radial field about (3, 2): the level 1 is one closed loop of
    // radius 1.
    let radial = ForceField::from_fn(nodes(6.0, 49), nodes(4.0, 33), |ae, ap| {
        ((ae - 3.0).powi(2) + (ap - 2.0).powi(2)).sqrt()
    });
    let loops = contours(&radial, 1.0);
    assert_eq!(loops.len(), 1, "one loop: {}", loops.len());
    let first = loops[0][0];
    let last = loops[0][loops[0].len() - 1];
    assert!(
        (first[0] - last[0]).abs() < 1e-9 && (first[1] - last[1]).abs() < 1e-9,
        "closed: {first:?} vs {last:?}"
    );
    for p in &loops[0] {
        let r = ((p[0] - 3.0).powi(2) + (p[1] - 2.0).powi(2)).sqrt();
        assert!((r - 1.0).abs() < 0.02, "on the circle: r {r}");
    }
    assert_eq!(contour_label(0.25, BudgetRole::Limit), "25 %");
    assert_eq!(contour_label(1.0, BudgetRole::Limit), "100 % budget");
    // P29-10 10g (lead ruling O4): where the budget is no limit of the
    // cut, the budget line says so.
    assert_eq!(
        contour_label(1.0, BudgetRole::NoLimitHere),
        "budget, no limit here"
    );
}

#[test]
fn the_chart_names_the_contours_on_their_lines_p17b() {
    let (env, budget, diameter) = fixture();
    let scene = EnvelopeScene::of(&env, None);
    // A plane field f = budget x ap / top: the four levels are four
    // horizontal lines at a quarter, a half, three quarters and the top
    // of the scene range.
    let top = scene.ap_range.1;
    let field = ForceField::from_fn(nodes(diameter, 48), nodes(top, 32), |_, ap| {
        budget * ap / top
    });
    // P29-10 10g: the chart draws the level lines core built with the
    // plane (no contour per frame).
    let plane = ForcePlane {
        levels: CONTOUR_SHARES
            .into_iter()
            .map(|share| ForceLevel {
                share,
                force_n: share * budget,
                lines: contours(&field, share * budget),
            })
            .filter(|level| !level.lines.is_empty())
            .collect(),
        field,
        fz_mm: 0.05,
        budget_n: budget,
        role: BudgetRole::Limit,
    };
    let samples = samples(&env, budget, diameter);
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        force_plane: Some(&plane),
        playhead: None,
        move_count: 31,
    };
    let ctx = ctx();
    let (shapes, response) = frame(&ctx, 340.0, &input, Vec::new());
    let map = response.map.expect("the chart drew");
    let extents = text_extents(&shapes);
    let texts: Vec<String> = extents.iter().map(|t| t.0.clone()).collect();
    for share in [0.25, 0.5, 0.75, 1.0] {
        let level_ap = share * top;
        if level_ap > map.y.1 + 1e-9 {
            // A level above the axis has no line to name.
            continue;
        }
        let name = contour_label(share, BudgetRole::Limit);
        let (_, _, _, rect) = extents
            .iter()
            .find(|t| t.0 == name)
            .unwrap_or_else(|| panic!("the contour {name:?} is named: {texts:?}"));
        let line_y = map.to_screen(50.0, level_ap).y;
        assert!(
            (rect.center().y - line_y).abs() <= rect.height() * 2.0,
            "{name:?} sits on its line: label {rect:?}, line y {line_y}"
        );
    }
    // The contour lines paint under the fills: in the shapes, the first
    // CAUTION line segment (the budget contour) precedes the first mesh.
    let flat = flat_shapes(&shapes);
    let first_mesh = flat
        .iter()
        .position(|s| matches!(s, egui::Shape::Mesh(_)))
        .expect("the wash paints a mesh");
    let budget_line = flat
        .iter()
        .position(|s| {
            matches!(s, egui::Shape::Path(p)
                if p.stroke.color == egui::epaint::ColorMode::Solid(tokens::CAUTION))
        })
        .expect("the budget contour paints as a CAUTION path");
    assert!(
        budget_line < first_mesh,
        "the budget contour ({budget_line}) paints under the fills ({first_mesh})"
    );
}

// ── claim 7 (P17b item 5): the planned dot paints last ──────────────────

#[test]
fn the_planned_dot_is_the_last_mark_and_paints_whole_over_a_dot_p17b() {
    let (env, budget, diameter) = fixture();
    let mut samples = samples(&env, budget, diameter);
    // Every steady sample over the budget: an orange dot at the planned
    // point itself.
    for s in &mut samples {
        s.force_n = Some(budget * 1.5);
    }
    let input = CutChartInput {
        env: &env,
        samples: &samples,
        diameter_mm: diameter,
        budget_n: Some(budget),
        force_plane: None,
        playhead: Some(0),
        move_count: 31,
    };
    let ctx = ctx();
    let (shapes, response) = frame(&ctx, 340.0, &input, Vec::new());
    let map = response.map.expect("the chart drew");
    let scene = EnvelopeScene::of(&env, None);
    let planned = env.stated.expect("the fixture states a point");
    let at = map.to_screen(scene.ae_share(planned.ae_mm), planned.ap_mm);
    let circles: Vec<(usize, egui::epaint::CircleShape)> = flat_shapes(&shapes)
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| {
            // The chart's circles only: the legend swatches under the
            // frame paint after it.
            if let egui::Shape::Circle(c) = s
                && c.center.y <= map.frame.bottom()
            {
                Some((i, c))
            } else {
                None
            }
        })
        .collect();
    let caution: Vec<usize> = circles
        .iter()
        .filter(|(_, c)| c.fill == tokens::CAUTION)
        .map(|(i, _)| *i)
        .collect();
    assert!(
        caution.iter().any(|i| circles
            .iter()
            .any(|(j, c)| j == i && c.center.distance(at) < 0.5)),
        "non-vacuity: an over-budget dot paints at the planned point {at:?}"
    );
    let (last_i, last) = circles.last().expect("a circle painted");
    assert_eq!(
        last.fill,
        tokens::TEXT_STRONG,
        "the last circle is the planned dot"
    );
    assert!(
        last.center.distance(at) < 0.5,
        "at the planned point: {:?}",
        last.center
    );
    let (halo_i, halo) = circles
        .iter()
        .rev()
        .find(|(_, c)| c.fill == tokens::SURFACE_BASE)
        .copied()
        .expect("the halo of the panel ground sits under it");
    assert!(
        halo.radius > last.radius && halo.center.distance(at) < 0.5,
        "the halo is wider than the dot: {} vs {}",
        halo.radius,
        last.radius
    );
    assert!(
        caution.iter().all(|i| *i < halo_i && *i < *last_i),
        "every over-budget dot paints before the halo and the planned dot"
    );
    let ring = circles
        .iter()
        .find(|(_, c)| c.stroke.width > 0.0 && c.fill == egui::Color32::TRANSPARENT);
    let (ring_i, _) = ring.expect("the playhead at move 0 paints a ring");
    assert!(
        halo_i < *ring_i && *ring_i < *last_i,
        "the ring paints between the halo and the planned dot: halo {halo_i}, ring {ring_i}, dot {last_i}"
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
        force_plane: None,
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
    assert!(
        !texts.iter().any(|t| t.contains("steady")),
        "P17b: no steady-pass text: {texts:?}"
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
        force_plane: None,
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
        for anchor in ["DEPTH (mm)", "moves", "cut time", "over budget", "playhead"] {
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
