//! **G-CORRIDOR — the nomogram draws the bounds it has, and abstains from
//! the one it does not.**
//!
//! The vendor band says where the vendor tested. The corridor says where the
//! CUT is physical: below the **rubbing floor** the tool burnishes instead
//! of cutting, above the **deflection ceiling** tooth force deflects or
//! chips it. Both are constant-chipload rays, so both are wedges of the
//! band's own shape, drawn in the machine wall's idiom — a fill, no stroke,
//! no floating label, no legend row.
//!
//! # The arm that matters
//!
//! **The corridor is one-sided for ordinary tools.** Measured on this
//! file's ordinary fixture — a 6 mm two-flute flat at 45 mm stickout in
//! generic softwood — the deflection ceiling is **3.143 mm/tooth** while the
//! top of the chart is **0.1235 mm/tooth**: twenty-five times off scale. The
//! package's own reference cut (4.20 DOC, 2.10 WOC) measured 9.36 against
//! 0.1176, eighty times off. A stubby carbide cutter in wood genuinely is
//! not deflection-limited, which is what `feeds::force`'s module docs and
//! `ADVICE.md` §4 both already said. The binding constraints on this class
//! of machine are the feed cap, the rubbing floor and rigidity.
//!
//! Arm 4 carries those numbers, so the threshold can be checked without
//! reading pixels.
//!
//! So a wedge clamped to the top edge would read as *"you are near the force
//! limit"*, false by two orders of magnitude. That is **absence rendered as
//! a reading**, the failure shape this repository has met four times, most
//! recently a nomogram readout that printed `37891 RPM · BURN risk` for a
//! pointer that was not over the chart at all
//! (`the_nomogram_readout_abstains_g_hoverbound.rs`). Arm 2 pins that
//! nothing is painted up there.
//!
//! The ceiling DOES come on chart for long or thin tools — compliance rises
//! with the cube of stickout — so arm 1 pins that the upper wedge is a
//! conditional, not a deletion. A 1.5 mm cutter at 90 mm stickout has a
//! ceiling of 0.0951 mm/tooth against the same 0.1167 chart top.
//!
//! # Rendered, not source-scanned
//!
//! The window is run through a real `egui::Context` and the corridor is read
//! off the painted shapes. A source scan would pass for a wedge that is
//! built and never reaches the plot, and for a wedge drawn in the wrong
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

/// The alpha `explore.rs` washes the corridor's two wedges at.
///
/// The corridor takes the machine wall's DANGER hue at a lighter alpha than
/// the wall's own 35, so the two are told apart by this number alone. If the
/// chart re-tunes it, re-tune this constant — it is the handle the test has
/// on the wedges, because the plot draws no legend and the polygon names
/// never reach the paint list.
const CORRIDOR_ALPHA: u8 = 22;

/// The alpha the vendor band is washed at, in `chart::BAND`.
const BAND_ALPHA: u8 = 50;

// SAFETY: `CHART_SERIES` has four entries and 1 is a literal inside it.
#[allow(clippy::indexing_slicing)]
fn band_fill() -> egui::Color32 {
    let base = tokens::CHART_SERIES[1];
    egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), BAND_ALPHA)
}

fn corridor_fill() -> egui::Color32 {
    let base = tokens::DANGER;
    egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), CORRIDOR_ALPHA)
}

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

/// A long, thin cutter. Compliance goes with the cube of stickout and the
/// fourth power of diameter, so this one IS deflection-limited and its
/// ceiling lands on the chart, at 0.0951 mm/tooth.
///
/// Both numbers matter. Below 1.5 mm the affine inversion runs out of force
/// budget and refuses outright, which is a DIFFERENT abstention from the
/// off-scale one arm 2 measures; above it the ceiling climbs back off the
/// chart. Arm 4 holds the fixture inside that window.
const LONG_AND_THIN: Cutter = Cutter {
    diameter_mm: 1.5,
    stickout_mm: 90.0,
};

fn fixture(cutter: &Cutter) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = cutter.diameter_mm;
    tool.stickout = cutter.stickout_mm;
    tool.flute_count = 2;

    // Generic softwood — the reference fixture's material, and the one the
    // measured 9.36 mm/tooth ceiling was taken on.
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

/// One filled region the window painted: its fill and where it sits.
#[derive(Debug, Clone, Copy)]
struct Region {
    /// Mean of the polygon's own vertices, in SCREEN space — y grows
    /// downward, so a smaller `centre_y` is HIGHER on the chart, which is a
    /// larger feed and therefore a larger advance per tooth.
    centre_y: f32,
    /// The topmost vertex, in screen space.
    top_y: f32,
}

/// Draw the Explore window and return every filled region painted in
/// `fill`.
///
/// Four passes, as in `the_nomogram_readout_abstains_g_hoverbound.rs`: the
/// plot needs a settled layout before its transform maps plot coordinates
/// onto meaningful screen positions.
fn painted_regions(state: &AppState, fill: egui::Color32) -> Vec<Region> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut regions = Vec::new();
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
                if path.fill != fill || path.points.is_empty() {
                    continue;
                }
                let count = path.points.len() as f32;
                let sum: f32 = path.points.iter().map(|p| p.y).sum();
                let top_y = path.points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                regions.push(Region {
                    centre_y: sum / count,
                    top_y,
                });
            }
        }
        out.textures_delta.clear();
    }
    regions
}

/// The vendor band, which both corridor bounds are positioned against.
fn band_region(state: &AppState) -> Region {
    let bands = painted_regions(state, band_fill());
    assert_eq!(
        bands.len(),
        1,
        "the nomogram painted {} vendor-band regions, not one. Every arm \
         below positions the corridor against the band, so the test cannot \
         read the chart until this is one region again.",
        bands.len()
    );
    bands[0]
}

// ── arm 1 — the ceiling is on chart ──────────────────────────────────────

/// A long, thin cutter IS deflection-limited, so the corridor has both
/// sides: the floor wedge below the band, the ceiling wedge above it.
#[test]
fn the_corridor_draws_both_bounds_when_the_ceiling_is_on_chart_g_corridor() {
    let state = fixture(&LONG_AND_THIN);
    let band = band_region(&state);
    let mut wedges = painted_regions(&state, corridor_fill());
    assert_eq!(
        wedges.len(),
        2,
        "a deflection-limited tool must get BOTH corridor bounds, and the \
         nomogram painted {}. Compliance rises with the cube of stickout, so \
         a long thin cutter is exactly the case where the ceiling comes on \
         chart; if it no longer does for this fixture, re-derive \
         `LONG_AND_THIN` rather than dropping the arm.",
        wedges.len()
    );
    wedges.sort_by(|a, b| a.centre_y.total_cmp(&b.centre_y));
    let (above, below) = (wedges[0], wedges[1]);
    assert!(
        above.centre_y < band.centre_y,
        "the ceiling wedge is not above the vendor band: wedge centre \
         {} vs band centre {} (screen y grows downward). Above the ceiling \
         tooth force deflects the cutter, so that region is the HIGH-feed \
         side of the band.",
        above.centre_y,
        band.centre_y
    );
    assert!(
        below.centre_y > band.centre_y,
        "the rubbing-floor wedge is not below the vendor band: wedge centre \
         {} vs band centre {}. Below the floor the tool burnishes, so that \
         region is the LOW-feed side.",
        below.centre_y,
        band.centre_y
    );
}

// ── arm 2 — the ceiling is off scale (the ordinary case) ─────────────────

/// The arm that matters. On the reference fixture the ceiling is about
/// eighty times the highest chipload the chart can draw, so exactly ONE
/// wedge is painted and nothing sits at the top edge pretending to be a
/// limit.
#[test]
fn the_upper_wedge_is_absent_when_the_ceiling_is_off_scale_g_corridor() {
    let state = fixture(&STUBBY);
    let band = band_region(&state);
    let wedges = painted_regions(&state, corridor_fill());
    assert_eq!(
        wedges.len(),
        1,
        "the nomogram painted {} corridor wedges for a stubby carbide \
         cutter. Its deflection ceiling is around 9 mm/tooth against a chart \
         that draws around 0.12 — a second wedge here is a force limit \
         manufactured from an absence, wrong by two orders of magnitude.",
        wedges.len()
    );
    let floor = wedges[0];
    assert!(
        floor.centre_y > band.centre_y,
        "the one wedge drawn is not the rubbing floor: centre {} against a \
         band centre of {}. If the ceiling wedge survived and the floor did \
         not, the chart is drawing the bound it cannot justify and hiding \
         the one it can.",
        floor.centre_y,
        band.centre_y
    );
    assert!(
        floor.top_y > band.top_y,
        "the floor wedge reaches above the top of the vendor band (wedge top \
         {} vs band top {}), so it is not the region below the floor",
        floor.top_y,
        band.top_y
    );
}

// ── arm 3 — non-vacuity ──────────────────────────────────────────────────

/// Arm 2 counts wedges, so it would pass just as well if the chart drew no
/// corridor at all. This arm is the floor of that count: the rubbing floor
/// IS painted, on the ordinary fixture, below the band.
#[test]
fn the_rubbing_floor_wedge_is_drawn_g_corridor() {
    let state = fixture(&STUBBY);
    let wedges = painted_regions(&state, corridor_fill());
    assert!(
        !wedges.is_empty(),
        "the nomogram painted NO corridor wedge. Arm 2 asserts that the \
         upper wedge is absent when the ceiling is off scale, and a chart \
         that draws nothing satisfies it for the wrong reason: the floor is \
         always on chart, because its ray leaves the origin."
    );
    let band = band_region(&state);
    assert!(
        wedges[0].centre_y > band.centre_y,
        "the one wedge on the ordinary fixture sits at {} against a band \
         centre of {}, so it is not the region below the floor",
        wedges[0].centre_y,
        band.centre_y
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
         wedge at all, which would let the rendered arms pass vacuously."
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
    let thin_ceiling = thin_ceiling.expect("the deflection model refused for the long thin cutter");
    assert!(
        thin_ceiling <= thin_chart_top,
        "the long thin cutter's ceiling is {thin_ceiling} mm/tooth against a \
         chart top of {thin_chart_top} mm/tooth, so arm 1 has no on-chart \
         case to draw"
    );
}
