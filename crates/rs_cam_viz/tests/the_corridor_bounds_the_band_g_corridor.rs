//! **G-CORRIDOR — the nomogram draws the bounds it has, and abstains from
//! the one it does not.**
//!
//! The vendor band says where the vendor tested. The corridor says where the
//! CUT is physical: below the **rubbing floor** the tool burnishes instead
//! of cutting, above the **deflection ceiling** tooth force deflects or
//! chips it. Both are constant-chipload rays, so the chart draws each one as
//! a thin line from the origin in the machine wall's hue — no floating
//! label, no legend row.
//!
//! # Lines, not wedges (G-CHARTLINES, 2026-09-23)
//!
//! The corridor was two shaded wedges, positioned against a shaded vendor
//! band. The operator ruled "no shading", and the vendor band now draws only
//! when the row publishes both limits. So this test reads LINES, and it
//! positions the corridor against the ONE line the chart always draws: the
//! suggested advance per tooth. Every constant-chipload line leaves the
//! origin, and the plot transform is linear, so the ratio of two lines'
//! screen slopes IS the ratio of their advances per tooth. The arms check
//! those ratios against the core numbers, which is stronger than the old
//! "wedge centre above band centre" check it replaced.
//!
//! # The arm that matters
//!
//! **The corridor is one-sided for ordinary tools.** Measured on this
//! file's ordinary fixture — a 6 mm two-flute flat at 45 mm stickout in
//! generic softwood, which cuts 4.20 DOC by 2.10 WOC — the deflection
//! ceiling is **2.4315 mm/tooth** while the top of the chart is **0.12353
//! mm/tooth**: about twenty times off scale. A stubby carbide cutter in
//! wood genuinely is not deflection-limited, which is what `feeds::force`'s
//! module docs and `ADVICE.md` §4 both already said. The binding
//! constraints on this class of machine are the feed cap, the rubbing floor
//! and rigidity.
//!
//! `explore.rs`'s `Ceiling::OffScale` doc printed 9.36 mm/tooth against
//! 0.1176 for that same stated cut. Neither number reproduced here, and the
//! comment was corrected on 2026-09-18 (T-9): it now carries this file's
//! measured pair, 2.4315 against 0.12353. The two are in agreement, so a
//! change to either must move both.
//!
//! Arm 4 carries those numbers, so the threshold can be checked without
//! reading pixels.
//!
//! So a line clamped to the top edge would read as *"you are near the force
//! limit"*, false by two orders of magnitude. That is **absence rendered as
//! a reading**, the failure shape this repository has met four times, most
//! recently a nomogram readout that printed `37891 RPM · BURN risk` for a
//! pointer that was not over the chart at all
//! (`the_nomogram_readout_abstains_g_hoverbound.rs`). Arm 2 pins that
//! nothing is painted up there.
//!
//! The ceiling DOES come on chart for a thin tool, so arm 1 pins that the
//! ceiling line is a conditional, not a deletion. A 2 mm cutter at 120 mm
//! stickout has a ceiling of 0.04796 mm/tooth against a 0.11667 chart top.
//!
//! **The diameter is the lever here, not the stickout.**
//! `ToolDefinition::tip_deflection_mm` models a STEPPED cantilever: a fixed
//! 25 mm flute section of the cutter diameter, below a 6.35 mm shank that
//! fills the rest of the stickout. The shank carries almost none of the
//! compliance at these diameters, so stickout is a weak knob. Measured at
//! Ø2 mm, a stickout of 40 mm gives 2.618e-2 mm/N and a stickout of 120 mm
//! gives 3.767e-2 mm/N — 3x the length for 1.44x the compliance, not 27x.
//! Any prose that says compliance goes with the cube of stickout describes
//! a plain cantilever, not this model.
//!
//! # Rendered, not source-scanned
//!
//! The window is run through a real `egui::Context` and the corridor is read
//! off the painted shapes. A source scan would pass for a line that is
//! built and never reaches the plot, and for a line drawn in the wrong
//! place.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::Material;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::{AppState, FeedsModalState};
use rs_cam_viz::ui::{feeds, tokens};

/// Room for the whole window, so the plot is drawn unclipped.
const SCREEN: egui::Vec2 = egui::Vec2::new(1400.0, 1000.0);

/// The alpha of the corridor's two lines in `explore.rs`
/// (`CORRIDOR_LINE_ALPHA`).
///
/// The corridor takes the machine wall's DANGER hue at a lighter alpha than
/// the wall's own 200, so the two are told apart by this number alone. If
/// the chart re-tunes it, re-tune this constant. It is the handle the test
/// has on the corridor, because the plot draws no legend and the line names
/// never reach the paint list.
const CORRIDOR_LINE_ALPHA: u8 = 150;

/// The suggested line's colour: `chart::BAND`, opaque.
// SAFETY: `CHART_SERIES` has four entries and 1 is a literal inside it.
#[allow(clippy::indexing_slicing)]
fn suggested_colour() -> egui::Color32 {
    tokens::CHART_SERIES[1]
}

fn corridor_colour() -> egui::Color32 {
    let base = tokens::DANGER;
    egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), CORRIDOR_LINE_ALPHA)
}

/// Two advances per tooth agree when their ratio is this close to 1. The
/// slopes are read in f32 screen space, so allow a small error.
const RATIO_TOLERANCE: f64 = 0.02;

/// How a tool is shaped for one arm of this test.
struct Cutter {
    diameter_mm: f64,
    stickout_mm: f64,
}

/// The reference fixture: a 6 mm two-flute flat at the stock stickout. Not
/// deflection-limited, which is the ordinary case.
const STUBBY: Cutter = Cutter {
    diameter_mm: 6.0,
    stickout_mm: 45.0,
};

/// A long, thin cutter. Its section is compliant enough that it IS
/// deflection-limited, and its ceiling lands on the chart at 0.04796
/// mm/tooth against a chart top of 0.11667.
///
/// # The window, and why the fixture sits in the middle of it
///
/// Arm 1 and arm 4 both need a ceiling that EXISTS and is ON chart, so the
/// fixture is bounded on two sides. Write the tip compliance at unit load
/// as `C` (mm/N). Both bounds are bounds on `C`:
///
/// - **Too compliant and the model refuses.** The budget force is
///   `EXCEEDS_BOUND_MM / C`, and the feed-independent edge force is
///   `ap · F_edge`. When the second meets the first, no feed satisfies the
///   bound and `chipload_cap_for_deflection` returns
///   `DeflectionCapRefusal::EdgeForceOverBudget`. At this fixture's cut
///   that is `C = 5.3908e-2`.
/// - **Too stiff and the ceiling leaves the chart**, which is arm 2's case,
///   not arm 1's. The ceiling reaches the chart top at `C = 2.6311e-2`.
///
/// Measured for a Ø2 mm two-flute flat in generic softwood, the window in
/// stickout is **43.685 mm to 159.705 mm**. The fixture takes 120 mm, the
/// geometric centre: the compliance may RISE by **1.4312x** before the
/// model refuses, and FALL by **1.4316x** before the ceiling goes off
/// chart.
///
/// Check a load-model change against those two factors. The previous
/// fixture (Ø1.5 mm at 90 mm) sat outside the refusal bound the moment
/// `93dd145c` made every fluted section bend on 0.80 of its cutting
/// diameter — 2.441x more compliant — because it had no margin on that
/// side at all.
const LONG_AND_THIN: Cutter = Cutter {
    diameter_mm: 2.0,
    stickout_mm: 120.0,
};

fn fixture(cutter: &Cutter) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = cutter.diameter_mm;
    tool.stickout = cutter.stickout_mm;
    tool.flute_count = 2;

    // Generic softwood — the reference fixture's material, and the one the
    // measured 2.4315 mm/tooth ceiling was taken on.
    let stock = StockConfig {
        material: Material::default(),
        ..Default::default()
    };

    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = 1_291.0;
        config.spindle_rpm = Some(17_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Corridor fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };

    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("corridor_fixture.svg"),
        name: "Corridor fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the corridor fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let toolpath_id = state.session.toolpath_configs()[0].id;
    state.feeds_modal = Some(FeedsModalState {
        toolpath_id,
        explore: None,
    });
    state
}

/// One open line the window painted in one stroke colour.
#[derive(Debug, Clone, Copy)]
struct Ray {
    /// The screen-space slope of the line's FIRST segment, as rise over run
    /// with the rise measured UP the screen. Every iso-advance line starts
    /// at the plot origin and its first segment lies on the ray, so this
    /// slope is `advance · flutes · (y scale / x scale)`. The ratio of two
    /// rays' slopes is the ratio of their advances per tooth.
    ///
    /// The guarantee that point 0 is the plot origin comes from
    /// `clip_iso_line` in `explore.rs`, which starts every iso-advance line
    /// at `[0, 0]`. If that function starts a line anywhere else, re-derive
    /// this measure.
    rise: f64,
}

/// Draw the Explore window and return every open line painted in `colour`.
///
/// Twelve passes, as in `the_nomogram_readout_abstains_g_hoverbound.rs`:
/// the plot needs a settled layout before its transform maps plot
/// coordinates onto meaningful screen positions.
fn painted_rays(state: &AppState, colour: egui::Color32) -> Vec<Ray> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut rays = Vec::new();
    for pass in 0..12 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            ..Default::default()
        };
        let mut events = Vec::new();
        let mut out = ctx.run_ui(input, |ui| {
            feeds::draw(ui.ctx(), state, &mut events);
        });
        if pass == 11 {
            for clipped in &out.shapes {
                let egui::epaint::Shape::Path(path) = &clipped.shape else {
                    continue;
                };
                if path.closed || path.points.len() < 2 {
                    continue;
                }
                let egui::epaint::ColorMode::Solid(stroke) = &path.stroke.color else {
                    continue;
                };
                if *stroke != colour {
                    continue;
                }
                let (p0, p1) = (path.points[0], path.points[1]);
                let run = f64::from(p1.x - p0.x);
                assert!(
                    run > 0.0,
                    "a line in this colour does not run left to right from the \
                     origin: {p0:?} to {p1:?}. It is not an iso-advance line \
                     from `clip_iso_line`, or another shape in the window \
                     shares its colour."
                );
                rays.push(Ray {
                    rise: f64::from(p0.y - p1.y) / run,
                });
            }
        }
        out.textures_delta.clear();
    }
    rays
}

/// The ONE solid line at the suggested advance per tooth, which every
/// corridor line is positioned against.
fn suggested_ray(state: &AppState) -> Ray {
    let rays = painted_rays(state, suggested_colour());
    assert_eq!(
        rays.len(),
        1,
        "the nomogram painted {} suggested lines, not one. The ruling is ONE \
         solid line at the suggested value (G-CHARTLINES), and every arm \
         below positions the corridor against it.",
        rays.len()
    );
    rays[0]
}

/// The suggested advance per tooth the chart draws, from the same
/// `FeedsExplain` the chart reads.
fn suggested_mm(cutter: &Cutter) -> f64 {
    let state = fixture(cutter);
    let tc = &state.session.toolpath_configs()[0];
    let tool = &state.session.tools()[0];
    let stock = state.session.stock_config();
    let preview = rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    );
    let r = &preview.explain().recommended;
    r.feed_rate_mm_min / (r.rpm * f64::from(preview.explain().flute_count.max(1)))
}

fn assert_ratio(what: &str, painted: f64, expected: f64) {
    assert!(
        expected.is_finite() && expected > 0.0,
        "{what}: the expected ratio {expected} is not a positive number"
    );
    assert!(
        (painted / expected - 1.0).abs() <= RATIO_TOLERANCE,
        "{what}: the painted slope ratio is {painted}, the core numbers give \
         {expected}. The line is not at the advance per tooth it claims."
    );
}

// ── arm 1 — the ceiling is on chart ──────────────────────────────────────

/// A long, thin cutter IS deflection-limited, so the corridor has both
/// lines: the rubbing floor and the deflection ceiling, each at its own
/// advance per tooth.
#[test]
fn the_corridor_draws_both_bounds_when_the_ceiling_is_on_chart_g_corridor() {
    let state = fixture(&LONG_AND_THIN);
    let suggested = suggested_ray(&state);
    let mut lines = painted_rays(&state, corridor_colour());
    assert_eq!(
        lines.len(),
        2,
        "a deflection-limited tool must get BOTH corridor lines, and the \
         nomogram painted {}. A thin section is exactly the case where the \
         ceiling comes on chart; if it no longer does for this fixture, \
         re-derive `LONG_AND_THIN` against the window its doc records \
         rather than dropping the arm.",
        lines.len()
    );
    lines.sort_by(|a, b| a.rise.total_cmp(&b.rise));
    let (floor_line, ceiling_line) = (lines[0], lines[1]);
    let (floor, ceiling, _) = measured(&LONG_AND_THIN);
    let ceiling = ceiling.expect("arm 4 pins that the thin cutter's ceiling is modelled");
    let suggested_mm = suggested_mm(&LONG_AND_THIN);
    assert_ratio(
        "rubbing floor against the suggested line",
        floor_line.rise / suggested.rise,
        floor / suggested_mm,
    );
    assert_ratio(
        "deflection ceiling against the suggested line",
        ceiling_line.rise / suggested.rise,
        ceiling / suggested_mm,
    );
}

// ── arm 2 — the ceiling is off scale (the ordinary case) ─────────────────

/// The arm that matters. On the reference fixture the ceiling is about
/// twenty times the highest chipload the chart can draw, so exactly ONE
/// corridor line is painted and nothing sits at the top edge pretending to
/// be a limit.
#[test]
fn the_ceiling_line_is_absent_when_the_ceiling_is_off_scale_g_corridor() {
    let state = fixture(&STUBBY);
    let suggested = suggested_ray(&state);
    let lines = painted_rays(&state, corridor_colour());
    assert_eq!(
        lines.len(),
        1,
        "the nomogram painted {} corridor lines for a stubby carbide \
         cutter. Its deflection ceiling is around 2.4 mm/tooth against a \
         chart that draws around 0.12 — a second line here is a force \
         limit manufactured from an absence, wrong by twenty times.",
        lines.len()
    );
    let (floor, _, _) = measured(&STUBBY);
    assert_ratio(
        "the one corridor line is the rubbing floor",
        lines[0].rise / suggested.rise,
        floor / suggested_mm(&STUBBY),
    );
}

// ── arm 3 — non-vacuity ──────────────────────────────────────────────────

/// Arm 2 counts lines, so it would pass just as well if the chart drew no
/// corridor at all. This arm is the floor of that count: the rubbing floor
/// IS painted, on the ordinary fixture, and the suggested line is painted
/// beside it.
#[test]
fn the_rubbing_floor_line_is_drawn_g_corridor() {
    let state = fixture(&STUBBY);
    let lines = painted_rays(&state, corridor_colour());
    assert!(
        !lines.is_empty(),
        "the nomogram painted NO corridor line. Arm 2 asserts that the \
         ceiling line is absent when the ceiling is off scale, and a chart \
         that draws nothing satisfies it for the wrong reason: the floor is \
         always on chart, because its ray leaves the origin."
    );
    let suggested = suggested_ray(&state);
    assert!(
        suggested.rise > 0.0 && lines[0].rise > 0.0,
        "the painted lines do not rise from the origin: suggested {}, floor {}",
        suggested.rise,
        lines[0].rise
    );
}

// ── arm 4 — the in-range decision, in numbers ────────────────────────────

/// What the chart decided, read off the core answer rather than off the
/// pixels.
///
/// Returns the rubbing floor, the deflection ceiling and the highest advance
/// per tooth this chart can draw — `feed_axis_max / (rpm · flutes)`, the
/// quantity `Corridor::resolve` compares the ceiling against.
fn measured(cutter: &Cutter) -> (f64, Option<f64>, f64) {
    let state = fixture(cutter);
    let tc = &state.session.toolpath_configs()[0];
    let tool = &state.session.tools()[0];
    let stock = state.session.stock_config();
    let preview = rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    );
    let efficiency = rs_cam_core::feeds::efficiency::cut_efficiency(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        preview.recommended(),
    )
    .expect("generic softwood carries a primary-source Kc, so the efficiency answer exists");
    let feed_axis_max = preview.explain().machine.max_feed_mm_min * 1.05;
    let chart_top_mm = feed_axis_max / (preview.recommended().rpm * f64::from(tool.flute_count));
    (
        efficiency.rubbing_floor_mm,
        efficiency.deflection_ceiling_mm,
        chart_top_mm,
    )
}

/// The rendered arms above say what is drawn. This one says WHY, in the
/// numbers the decision is made on, so a future reader can check the
/// threshold without reading pixels.
#[test]
fn the_ceiling_decision_is_made_on_the_charts_own_range_g_corridor() {
    let (floor, ceiling, chart_top) = measured(&STUBBY);
    assert!(
        floor > 0.0,
        "the rubbing floor is {floor} mm/tooth. A non-positive floor draws no \
         line at all, which would let the rendered arms pass vacuously."
    );
    let ceiling = ceiling.expect(
        "the deflection model refused for an ordinary 6 mm carbide flat. That \
         is a different abstention from the off-scale one this arm measures, \
         and it would make the off-scale case untested.",
    );
    assert!(
        ceiling > chart_top,
        "the ordinary fixture's deflection ceiling is {ceiling} mm/tooth \
         against a chart top of {chart_top} mm/tooth, so it is ON chart and \
         arm 2 is no longer testing the off-scale case"
    );

    let (_, thin_ceiling, thin_chart_top) = measured(&LONG_AND_THIN);
    let thin_ceiling = thin_ceiling.expect(
        "the deflection model refused for the long thin cutter. \
         `LONG_AND_THIN` left the window its doc block records: the edge \
         force alone now meets the deflection budget, so no feed satisfies \
         the bound. Re-derive the fixture against that window; do not drop \
         the arm.",
    );
    assert!(
        thin_ceiling <= thin_chart_top,
        "the long thin cutter's ceiling is {thin_ceiling} mm/tooth against a \
         chart top of {thin_chart_top} mm/tooth, so arm 1 has no on-chart \
         case to draw"
    );
}
