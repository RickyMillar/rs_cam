//! **G-CHIPVERDICT — the Feeds tab says where the cut sits, and abstains
//! when it cannot.**
//!
//! # Two rows, one question each
//!
//! The card carries one chipload verdict row and one power row. The chipload
//! row says where the chip sits in the vendor corridor and what sitting there
//! costs; the power row says how much of the spindle's limit this cut draws
//! at the depth that will be cut. Neither repeats the other, and each has one
//! row.
//!
//! # The chipload row
//!
//! Below the vendor band there is no trade-off, only loss: the cut spends
//! more energy per mm³ AND more time than the band midpoint, at once.
//! `feeds::efficiency::cut_efficiency` computes that verdict; this row
//! renders it.
//!
//! # The power row, and the ban it replaced
//!
//! The card once ended with `Power 0.01 of 0.60 kW (1 %)`, and this file
//! banned its return: peak utilisation across the whole shipped matrix was
//! 23.6 %, so the readout could not separate a good cut from a bad one. R1
//! then rebuilt power on the affine force model, and the re-measurement made
//! the ban's reason false. The operator reinstated the bar on 2026-09-18
//! (`planning/load_model_2026-09-16/RESUME_PLAN.md` §7, ruling 2). Arm 3
//! below is the ban's successor: it watches the REASON, not the ban. A ban
//! whose reason expired becomes a test of the reason.
//!
//! # The arm that matters
//!
//! `cut_efficiency` returns `Option<CutEfficiency>`, and three of its fields
//! are independently optional; `power_at_operating_point` returns a typed
//! refusal with seven variants. Every one of those absences must reach the
//! screen as a **stated abstention** — never as a zero, a blank, a bare dash,
//! an empty bar or a 100 %. This repository has drawn the opposite four
//! times: an empty triage as an all-clear, an empty chart frame as a chart, a
//! 2D model's zero Z as a blank frame, and a hazard verdict from a pointer
//! that was not on the chart. `no_fabricated_number_survives_a_refusal_g_chipverdict`
//! and `the_power_row_states_its_refusal_g_chipverdict` are the fifth's
//! tripwires, and `the_verdict_row_states_real_numbers_g_chipverdict` and
//! `the_power_row_states_a_real_reading_g_chipverdict` are their non-vacuity
//! partners: a suite in which everything abstains would pass the refusal arms
//! and prove nothing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // The spread arm RECORDS its measurement as well as asserting on it;
    // `--nocapture` is how the figure in V4_IMPLEMENTATION.md was taken.
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::{self, PowerFigure, PowerUnmodeled};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The Simulation workspace's right rail — the narrowest the Feeds tab must
/// hold, and the width `inspector_width_is_tab_independent_up4` pins.
const PANEL_WIDTH: f32 = 240.0;

/// The face of the verdict row always opens with this label.
const ROW_LABEL: &str = "Chip";

// ── fixtures ───────────────────────────────────────────────────────────────

/// One operating point the row has to describe.
struct Fixture {
    tool_type: ToolType,
    /// The cutter diameter. Ø6 unless the fixture needs a cell that only a
    /// different diameter reaches.
    diameter_mm: f64,
    material: Material,
    operation: OperationConfig,
    /// The tool stickout (mm). [`DEFAULT_FIXTURE_STICKOUT_MM`] unless the
    /// fixture needs a short tool.
    stickout_mm: f64,
    /// The machine, when the fixture needs one that is not the builder's
    /// own preset.
    machine: Option<MachineProfile>,
}

/// The stickout of every fixture except [`in_band`].
const DEFAULT_FIXTURE_STICKOUT_MM: f64 = 18.0;

/// The diameter of every fixture except [`in_band`].
const FIXTURE_DIAMETER_MM: f64 = 6.0;

/// Softwood under a Ø6 flat 2F pocket on a slow gantry: the machine's
/// cutting-feed ceiling holds the recommendation below the matched vendor
/// band, so the verdict is `Thin` and both ratios exist.
///
/// Feeds matrix R5 re-bless (2026-09-23). Until R5 this fixture was Baltic
/// birch under a 3D adaptive pass. R5 moved that cell onto
/// `amana-flat-plywood-hardwood-pocket-6000-2f-spektra`, which prints ONE
/// value (max only). The chart-display ruling (5199e06e) gives a one-value
/// row no band, so the row painted "no vendor chipload range matched".
///
/// This cell was the in-band fixture until R5. It resolves to the printed
/// row `amana-zrn-flat-softwood-pocket-6000-2f`, which publishes BOTH
/// limits: 0.1778-0.2286 mm/tooth at Ø6.0 and Janka 600, so the diameter
/// and hardness scales are 1.0 for generic softwood (Janka 600). The
/// arithmetic, from `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`:
///
/// * The recommended axial depth is 4.2 mm = 0.7 x D, so the depth scale
///   is 1.0 and the band stays 0.1778-0.2286.
/// * The seed chipload is the band midpoint, 0.2032 mm/tooth. At 18 000 rpm
///   and 2 flutes that asks for 0.2032 x 18 000 x 2 = 7315 mm/min.
/// * **RE-BLESSED 2026-09-24, ruling R4 Q10.** The RPM now follows a
///   binding cutting-feed ceiling down to hold the chip. On the generic
///   router's 4000 mm/min ceiling the RPM went to 4000 / 0.4064 = 9842 and
///   the chip stayed at the band value 0.2032, so the fixture read "in
///   vendor range". The fixture now needs a floor that STOPS the descent.
/// * The machine is the generic router with a cutting-feed ceiling of
///   1000 mm/min ([`thin_machine`]). Its spindle range is 8000-24 000 rpm.
/// * The chip per rev is 7315 / 18 000 = 0.4064 mm. The RPM that holds the
///   chip at the ceiling is 1000 / 0.4064 = 2460, under the 8000 rpm machine
///   minimum, so the descent stops at 8000 (`RpmLoweredForFeedCeiling`,
///   `held: false`). The feed there, 0.4064 x 8000 = 3251 mm/min, is still
///   over the ceiling, so Step 7 clamps it to 1000 (`FeedRateClamped`).
/// * chipload = 1000 / (8000 x 2) = 0.0625 mm/tooth, below the 0.1778
///   minimum and above the 0.025 rubbing floor, so the verdict is `Thin`.
///   The band midpoint gives the time ratio 0.2032 / 0.0625 = 3.25x. If the
///   vendor row's `rpm_min` is above 8000, the descent stops there instead
///   and the chip is thinner still; the verdict stays `Thin`.
fn thin() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        diameter_mm: FIXTURE_DIAMETER_MM,
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        operation: OperationConfig::Pocket(Default::default()),
        stickout_mm: DEFAULT_FIXTURE_STICKOUT_MM,
        machine: Some(thin_machine()),
    }
}

/// The generic router (spindle 8000-24 000 rpm) with a cutting-feed ceiling
/// of 1000 mm/min, so the Q10 descent stops at the machine minimum and the
/// ceiling thins the chip. See [`thin`].
fn thin_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_cutting_feed_mm_min = Some(1000.0);
    machine
}

/// Generic hardwood under a Ø3.175 flat 2F profile, on a short tool: the
/// recommendation lands inside the vendor band.
///
/// **RE-BLESSED 2026-09-23, ruling R4 WP2a.** Until then the rubbing floor
/// LIFTED this cell's seed to 0.025 mm/tooth, which is inside the band, and
/// that lift was the whole reason the fixture read "in band". With no lift
/// the cell ships its computed chipload.
///
/// **RE-BLESSED 2026-09-24, ruling R4 WP3.** The machine safety factor is
/// gone, and the long-tool share is a load target, not a feed factor (Q7).
/// The fixture ran on a machine with `safety_factor` 0.80 until then; it now
/// runs on the builder's generic router, and no factor touches the feed.
///
/// The stickout is 12 mm, 3.78 x D, under the 4 x D long-tool threshold, so
/// the long-tool share is 1.0 and no long-tool line paints either.
///
/// The arithmetic, from the matrix CSV for the row and the band:
///
/// * The row is `amana-flat-hardwood-contour-3175-2f` (VendorBacked, so R1
///   does not refuse it): 0.018-0.030 mm/tooth at Ø3.175 and Janka 1300.
/// * The hardness scale is (1300 / 1450)^0.5 = 0.9469, so the band is
///   0.0170-0.0284 mm/tooth and the seed chipload is the midpoint, 0.0227.
/// * The depth scale applies to the feed and to the band alike (R3), so it
///   does not move the ratio.
/// * chipload = 0.0227 mm/tooth, inside 0.0170-0.0284, so the verdict is
///   `InBand`. It is above the Q9 floor min(0.025, 0.0170) = 0.0170, so no
///   floor line paints.
///
/// The in-band fixture's stickout (mm): 12 / 3.175 = 3.78 x D, under the
/// 4 x D long-tool threshold. Not 12.7, whose ratio sits on the threshold.
const IN_BAND_STICKOUT_MM: f64 = 12.0;

fn in_band() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        diameter_mm: 3.175,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        operation: OperationConfig::Profile(Default::default()),
        stickout_mm: IN_BAND_STICKOUT_MM,
        machine: None,
    }
}

/// A V-bit pocket. No vendor row routes to this pairing, so there is no
/// band; and the deflection predictor declines a V-bit, so there is no
/// headroom either. Two independent refusals in one row.
fn no_band() -> Fixture {
    Fixture {
        tool_type: ToolType::VBit,
        diameter_mm: FIXTURE_DIAMETER_MM,
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        operation: OperationConfig::Pocket(Default::default()),
        stickout_mm: DEFAULT_FIXTURE_STICKOUT_MM,
        machine: None,
    }
}

/// Acrylic. `Material::force_line` refuses — no source prints a
/// cutting-force line for a plastic (ruling B6) — so `cut_efficiency`
/// returns `None` outright.
fn no_kc() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        diameter_mm: FIXTURE_DIAMETER_MM,
        material: Material::Plastic {
            family: PlasticFamily::Acrylic,
        },
        operation: OperationConfig::Pocket(Default::default()),
        stickout_mm: DEFAULT_FIXTURE_STICKOUT_MM,
        machine: None,
    }
}

fn state_for(fixture: Fixture) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), fixture.tool_type);
    tool.diameter = fixture.diameter_mm;
    tool.flute_count = 2;
    tool.stickout = fixture.stickout_mm;

    let mut operation = fixture.operation;
    match &mut operation {
        OperationConfig::Adaptive3d(config) => {
            config.feed_rate = 3_000.0;
            config.spindle_rpm = Some(18_000);
        }
        OperationConfig::Pocket(config) => {
            config.feed_rate = 3_000.0;
            config.spindle_rpm = Some(18_000);
        }
        OperationConfig::Profile(config) => {
            config.feed_rate = 3_000.0;
            config.spindle_rpm = Some(18_000);
        }
        other => panic!("fixture operation {other:?} has no feed/RPM setter here"),
    }

    state_for_recipe(fixture.machine, tool, fixture.material, operation)
}

/// A session on one tool × material × operation, on `machine` when the
/// recipe names one and on the builder's own preset when it does not.
///
/// The spread arm needs a session on an arbitrary shipped preset, diameter
/// and operation, so the fixture builders above and that arm share one
/// session shape rather than two.
fn state_for_recipe(
    machine: Option<MachineProfile>,
    tool: ToolConfig,
    material: Material,
    operation: OperationConfig,
) -> AppState {
    let stock = StockConfig {
        material,
        ..Default::default()
    };

    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Chip verdict fixture".to_owned(),
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
        path: PathBuf::from("chip_verdict_fixture.svg"),
        name: "Chip verdict fixture".to_owned(),
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
    if let Some(machine) = machine {
        builder = builder.machine(machine);
    }
    builder
        .add_toolpath(0, config)
        .expect("add the chip verdict fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab =
        Some((id, rs_cam_viz::ui::properties::ToolpathTab::FeedsSpeeds));
    state
}

// ── harness ────────────────────────────────────────────────────────────────

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips too, which is where the workings live.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Every text run the Feeds tab paints, in paint order. The production tab
/// override is one-shot, so the second frame is the steady state.
fn painted_text(fixture: Fixture) -> Vec<String> {
    painted_text_of(state_for(fixture))
}

/// The same, on a session the caller built.
fn painted_text_of(mut state: AppState) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("chipverdict_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, &mut state, &mut events);
                    });
                });
        });
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// The single-line runs the verdict row paints: its label, the chipload it
/// is describing, and the verdict phrase. Multi-line runs are hovers and are
/// read separately.
///
/// The three runs follow the label in the paint order `rail_efficiency_row`
/// builds them in.
fn verdict_row(texts: &[String]) -> Vec<String> {
    let label = format!("{ROW_LABEL} {}", tokens::GLYPH_DETAIL);
    let at = texts
        .iter()
        .position(|t| *t == label)
        .unwrap_or_else(|| panic!("no verdict row painted; runs were {texts:#?}"));
    texts[at..]
        .iter()
        .filter(|t| !t.contains('\n'))
        .take(3)
        .cloned()
        .collect()
}

/// The verdict row's hover — the longest multi-line run that opens the way
/// the row's own hover opens.
fn verdict_hover(texts: &[String]) -> String {
    texts
        .iter()
        .filter(|t| {
            t.contains('\n') && (t.starts_with("Chipload ") || t.starts_with("Efficiency is not"))
        })
        .max_by_key(|t| t.len())
        .cloned()
        .unwrap_or_else(|| panic!("the verdict row painted no hover; runs were {texts:#?}"))
}

/// A named fixture builder.
type NamedFixture = (&'static str, fn() -> Fixture);

/// Every fixture, for the arms that hold on all four.
fn all_fixtures() -> [NamedFixture; 4] {
    [
        ("thin", thin as fn() -> Fixture),
        ("in band", in_band),
        ("no band", no_band),
        ("no Kc", no_kc),
    ]
}

// ── arm 1 — exactly one verdict row ────────────────────────────────────────

#[test]
fn the_feeds_tab_paints_exactly_one_verdict_row_g_chipverdict() {
    let label = format!("{ROW_LABEL} {}", tokens::GLYPH_DETAIL);
    for (name, fixture) in all_fixtures() {
        let texts = painted_text(fixture());
        let count = texts.iter().filter(|t| **t == label).count();
        assert_eq!(
            count, 1,
            "the {name} fixture painted {count} verdict rows. One operating \
             point has one verdict; a second row is a second answer to the \
             same question."
        );
    }
}

/// The row joins the per-row explanation pattern
/// `the_recommendation_explains_each_row_g_whyrow` pins: the detail mark on
/// the label, and a multi-line hover of its own behind it.
#[test]
fn the_verdict_row_carries_its_own_explanation_g_chipverdict() {
    for (name, fixture) in all_fixtures() {
        let texts = painted_text(fixture());
        assert!(
            texts
                .iter()
                .any(|t| *t == format!("{ROW_LABEL} {}", tokens::GLYPH_DETAIL)),
            "the {name} fixture's verdict row carries no `{}` mark, so its \
             workings are unreachable",
            tokens::GLYPH_DETAIL
        );
        let hover = verdict_hover(&texts);
        assert!(
            hover.lines().count() >= 3,
            "the {name} fixture's verdict hover is one line: {hover:?}. The \
             row's job is to put the ratios on the face and the workings \
             behind them."
        );
    }
}

// ── arm 2 — the row changes state ──────────────────────────────────────────

/// A readout that says the same thing on every job is the power bar again.
/// The thin and in-band fixtures differ in material, operation and diameter,
/// and the painted verdict must differ with them.
#[test]
fn the_verdict_changes_state_across_fixtures_g_chipverdict() {
    let thin_row = verdict_row(&painted_text(thin()));
    let in_band_row = verdict_row(&painted_text(in_band()));
    assert_ne!(
        thin_row, in_band_row,
        "the thin and in-band fixtures painted the same verdict row. A row \
         that cannot change state is the 1 %-forever power bar it replaced."
    );

    let thin_face = thin_row.join(" ");
    let in_band_face = in_band_row.join(" ");
    assert!(
        thin_face.contains("thin") && thin_face.contains('\u{00D7}'),
        "the thin fixture painted {thin_face:?}; it must name the state and \
         quote the wear/time cost as a ratio."
    );
    assert!(
        in_band_face.contains("in vendor range"),
        "the in-band fixture painted {in_band_face:?}; it must say the cut \
         is inside the vendor window."
    );
}

/// Ratios are the actionable half; the unit figure and the ploughing share
/// belong on the hover. `151 J/mm³` on the face is a number a hobby user
/// cannot act on, and the ploughing share is the physical content of "burn
/// risk", which no surface stated before this row.
#[test]
fn the_units_stay_on_the_hover_g_chipverdict() {
    let texts = painted_text(thin());
    let face = verdict_row(&texts).join(" ");
    assert!(
        !face.contains("J/mm"),
        "the verdict face quotes J/mm³: {face:?}. The unit belongs on the \
         hover, where the reader who wants it will look."
    );
    let hover = verdict_hover(&texts);
    assert!(
        hover.contains("J/mm\u{00B3}"),
        "the verdict hover drops the specific energy: {hover:?}"
    );
    assert!(
        hover.contains("rubbing rather than shearing"),
        "the verdict hover drops the ploughing share in plain words: \
         {hover:?}. That share IS the burn risk, and it is the sentence this \
         row exists to make sayable."
    );
}

// ── arm 3 — the power bar is informative ───────────────────────────────────
//
// This arm replaces `no_power_gauge_returns_to_the_card_g_chipverdict`, which
// banned `ProgressBar`, `power_bar`, `power_color` and `rail_power_row` from
// `src/ui/feeds/compare.rs`. The ban's stated reason was a measurement:
//
// > The power readout peaked at 23.6 % across the entire shipped matrix; it
// > cannot separate a good cut from a bad one.
//
// That measurement was taken against a power model that no longer exists.
// The gauge was deleted at `e475085f`; `2708ee52` (R1) then rebuilt power on
// the affine force model, which added a feed-free edge term carrying most of
// the load at wood chiploads. Re-measured after R1 the spread is median
// 17.8 %, p90 89.4 %, peak 100.0 %, with a quarter of recipes over half
// scale (`planning/load_model_2026-09-16/SURVEY_UI.md`, "The ban on a
// predicted power bar rests on stale evidence"). The operator reversed the
// ruling on 2026-09-18 (`RESUME_PLAN.md` §7, ruling 2): the bar returns as a
// 0-to-limit bar.
//
// A ban whose reason expired becomes a test of the reason. The arms below
// watch the reason, not the ban: the row reads 0 to the machine's limit at
// the point the operation will cut, it states a refusal rather than a zero,
// and its utilisation still spreads across the range.

/// The face of the power row always opens with this label.
const POWER_LABEL: &str = "Power";

/// The single-line runs the power row paints: its label, the percent of the
/// limit, and the setting the limit traces back to.
fn power_row(texts: &[String]) -> Vec<String> {
    let label = format!("{POWER_LABEL} {}", tokens::GLYPH_DETAIL);
    let at = texts
        .iter()
        .position(|t| *t == label)
        .unwrap_or_else(|| panic!("no power row painted; runs were {texts:#?}"));
    texts[at..]
        .iter()
        .filter(|t| !t.contains('\n'))
        .take(3)
        .cloned()
        .collect()
}

/// The power row's hover — the longest multi-line run that opens with the
/// row's own label.
fn power_hover(texts: &[String]) -> String {
    texts
        .iter()
        .filter(|t| t.contains('\n') && t.starts_with(POWER_LABEL))
        .max_by_key(|t| t.len())
        .cloned()
        .unwrap_or_else(|| panic!("the power row painted no hover; runs were {texts:#?}"))
}

/// The figure the row must be showing, read from a session built the same way
/// the painted frame's session is, and through the same public doors the row
/// calls. The test states no power expression of its own.
///
/// **RE-BLESSED 2026-09-24, G-RECOMAPPLIED.** The row reads the cut that
/// `⚡ Apply all` writes, not the operation's current values. The chain:
///
/// * `feeds_preview_for_operation` gives the calculator result;
/// * `preview_field_applies`, with the session's model box (the context the
///   card and the controller's apply pass), gives the value the funnel
///   writes for each field;
/// * those values are written into a copy of the operation, and
///   `power_at_operating_point` reads that copy. Its fallback point is the
///   applied value of each field, or the calculator value where the funnel
///   writes nothing.
///
/// Until then the door read the fixture's own feed 3000 mm/min and 18 000
/// rpm and the operation's default depth and stepover. The in-band arm pins
/// the ratio of the door, not a number, so it moves with the door.
fn door_figure(fixture: Fixture) -> Result<PowerFigure, PowerUnmodeled> {
    use rs_cam_core::feeds::FeedsField;
    let state = state_for(fixture);
    let tc = &state.session.toolpath_configs()[0];
    let tool = &state.session.tools()[0];
    let stock = state.session.stock_config();
    let preview = feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    );
    let recommended = preview.recommended();
    let model_bbox = state.session.model_bbox(tc.model_id);
    let previews = feeds::suggest::preview_field_applies(
        &tc.operation,
        recommended,
        tool,
        state.session.machine(),
        &stock.material,
        tc.operation.feeds_style().1,
        feeds::suggest::SuggestContext {
            model_bbox: model_bbox.as_ref(),
            ..feeds::suggest::SuggestContext::default()
        },
    );
    let mut applied = tc.operation.clone();
    let mut scratch = feeds::FeedsProvenance::default();
    for field in [
        FeedsField::FeedRate,
        FeedsField::PlungeRate,
        FeedsField::SpindleRpm,
        FeedsField::Stepover,
        FeedsField::DepthPerPass,
    ] {
        if let Some(p) = previews.get(field) {
            p.write_to(&mut applied, &mut scratch);
        }
    }
    let value = |field: FeedsField, raw: f64| previews.get(field).map_or(raw, |p| p.value);
    feeds::power_at_operating_point(
        &applied,
        tool,
        &stock.material,
        state.session.machine(),
        Some(feeds::suggest::CalculatorOperatingPoint {
            radial_width_mm: value(FeedsField::Stepover, recommended.radial_width_mm),
            axial_depth_mm: value(FeedsField::DepthPerPass, recommended.axial_depth_mm),
            feed_rate_mm_min: value(FeedsField::FeedRate, recommended.feed_rate_mm_min),
            rpm: value(FeedsField::SpindleRpm, recommended.rpm),
        }),
    )
}

/// The percent the row paints, parsed back off the face.
fn painted_percent(face: &str) -> f64 {
    let at = face
        .find('%')
        .unwrap_or_else(|| panic!("the power face paints no percent: {face:?}"));
    let digits: String = face[..at]
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("the power face paints no parsable percent: {face:?}"))
}

/// The row is one 0-to-limit reading: a percent on the face, the kW pair and
/// the provenance on the hover, and the setting the limit traces back to
/// beside it. The operator's rules 1 to 3 (`RESUME_PLAN.md` §7).
#[test]
fn the_power_row_reads_zero_to_the_limit_g_chipverdict() {
    let label = format!("{POWER_LABEL} {}", tokens::GLYPH_DETAIL);
    for (name, fixture) in [
        ("thin", thin as fn() -> Fixture),
        ("in band", in_band),
        ("no band", no_band),
    ] {
        let texts = painted_text(fixture());
        let count = texts.iter().filter(|t| **t == label).count();
        assert_eq!(
            count, 1,
            "the {name} fixture painted {count} power rows. One operating \
             point has one power reading."
        );

        let face = power_row(&texts).join(" ");
        assert!(
            face.contains('%'),
            "the {name} fixture's power face quotes no percent of the limit: \
             {face:?}. Every limit reads 0 to limit."
        );
        assert!(
            !face.contains(" kW"),
            "the {name} fixture's power face quotes kW: {face:?}. The ruling \
             puts the kW pair on the hover and a 0-to-limit percent on the \
             face."
        );
        assert!(
            face.contains("machine"),
            "the {name} fixture's power row drops its setting: {face:?}. The \
             limit must trace back to the setting that set it."
        );

        let hover = power_hover(&texts);
        for clause in [
            // BoundSource::MachinePowerCurve::clause(), formatted from the
            // rpm the gate stores. Ruling R4 Q2 (2026-09-24): the rated
            // curve, with no fraction.
            "the rated machine power curve at",
            // BoundSource::MachinePowerCurve::setting().
            "machine",
            // The kW pair the face no longer carries.
            " kW",
            // The four figures of the operating point the bar describes.
            "depth of cut",
            "width of cut",
            " rpm",
            "mm/min",
        ] {
            assert!(
                hover.contains(clause),
                "the {name} fixture's power hover drops {clause:?}: {hover:?}. \
                 The hover carries the provenance and the point the figure was \
                 evaluated at."
            );
        }
    }
}

/// Non-vacuity: the in-band fixture paints a real reading, and it is the
/// door's own ratio. A suite in which the row always refused would pass the
/// refusal arm and prove nothing.
#[test]
fn the_power_row_states_a_real_reading_g_chipverdict() {
    let face = power_row(&painted_text(in_band())).join(" ");
    let painted = painted_percent(&face);
    assert!(
        painted > 0.0 && painted < 100.0,
        "the in-band fixture painted {painted} % of the limit: {face:?}. A \
         zero and a flat hundred are both the shape of a constraint that was \
         never evaluated."
    );

    let figure = door_figure(in_band()).unwrap_or_else(|reason| {
        panic!(
            "the in-band fixture refuses the power door: {}",
            reason.clause()
        )
    });
    let expected = figure.required_kw / figure.available_kw * 100.0;
    assert!(
        (painted - expected).abs() <= 0.05,
        "the row painted {painted} % but feeds::power_at_operating_point \
         reads {expected} % ({:.4} kW of {:.4} kW). The face must be the \
         door's own ratio, not a second computation.",
        figure.required_kw,
        figure.available_kw,
    );
}

/// A refusal is stated, never drawn as a zero or an empty bar. Acrylic has no
/// force line (ruling B6), so `power_at_operating_point` returns
/// `PowerUnmodeled::MaterialUnvalidated`.
#[test]
fn the_power_row_states_its_refusal_g_chipverdict() {
    let reason = door_figure(no_kc())
        .err()
        .unwrap_or_else(|| panic!("the no-Kc fixture no longer refuses the power door"));
    assert_eq!(reason, PowerUnmodeled::MaterialUnvalidated);

    let texts = painted_text(no_kc());
    let face = power_row(&texts).join(" ");
    assert!(
        face.contains(reason.clause()),
        "the no-Kc fixture painted {face:?}; it must paint the door's own \
         clause {:?}.",
        reason.clause()
    );
    assert!(
        !face.contains('%'),
        "the no-Kc fixture painted a percent of a limit it cannot read: \
         {face:?}. A refusal is never a zero and never an empty bar."
    );
    // Ruling B6: the refusal clause and the force-line headline name the
    // ruling ("B6"), which is not a figure. Strip those two stated texts
    // before the digit check, so a fabricated number still fails it.
    let refusal_headline = no_kc()
        .material
        .force_line()
        .err()
        .map(|refusal| refusal.headline())
        .unwrap_or_default();
    let figures = face
        .replace(reason.clause(), "")
        .replace(&refusal_headline, "");
    assert!(
        !figures.chars().any(|c| c.is_ascii_digit()),
        "the no-Kc power face carries a number: {face:?}."
    );

    let hover = power_hover(&texts);
    assert!(
        hover.contains(reason.clause()) && hover.contains("MaterialUnvalidated"),
        "the no-Kc power hover does not name the missing input or the \
         engine's own refusal: {hover:?}."
    );
}

// ── the shipped population, and the reason the ban expired ─────────────────

/// A named operation family: what to call it, and how to build one.
type NamedFamily = (&'static str, fn() -> OperationConfig);

/// The three operation families the sweep walks, each a milling family the
/// Feeds card draws a recipe for.
fn sweep_families() -> [NamedFamily; 3] {
    [
        (
            "pocket",
            (|| OperationConfig::Pocket(Default::default())) as fn() -> OperationConfig,
        ),
        ("adaptive", || OperationConfig::Adaptive(Default::default())),
        ("adaptive 3d", || {
            OperationConfig::Adaptive3d(Default::default())
        }),
    ]
}

/// One shipped recipe, and what the power row reads on it.
struct Recipe {
    /// Utilisation of this recipe's own ceiling, as a percent.
    utilisation_pct: f64,
    machine: MachineProfile,
    material: Material,
    tool: ToolConfig,
    /// The operation AFTER the Suggest funnel wrote it — the state the
    /// machine receives, and the state the power row reads.
    operation: OperationConfig,
    name: String,
}

/// Take one shipped recipe through the public doors the Feeds card uses: the
/// Suggest funnel writes the operation, and `power_at_operating_point` reads
/// the point that operation ships.
fn shipped_recipe(
    machine: &MachineProfile,
    species: WoodSpecies,
    diameter: f64,
    family: NamedFamily,
) -> Option<Recipe> {
    let stock = StockConfig {
        material: Material::SolidWood { species },
        ..Default::default()
    };
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = diameter;
    tool.flute_count = 2;
    tool.stickout = 3.0 * diameter;

    // The heaviest cut the calculator will entertain for this tool — a
    // full-width slot at twice the diameter — is what the review's sweep
    // asked for. The clamps answer it, and their answer is the recipe that
    // ships.
    let (family_name, build) = family;
    let mut operation = build();
    operation.set_spindle_rpm(Some(18_000));
    operation.set_feed_rate(3_000.0);
    operation.set_stepover(diameter);
    operation.set_depth_per_pass(2.0 * diameter);

    let preview = feeds::suggest::feeds_preview_for_operation(
        &operation,
        &tool,
        &stock.material,
        machine,
        feeds::embedded_vendor_lut(),
        feeds::SpindleStrategy::MatchChart,
    );
    let recommended = preview.applicable()?.result().clone();
    let (_, pass_role) = operation.feeds_style();
    let mut provenance = feeds::FeedsProvenance::default();
    feeds::suggest::apply_feeds_result_to_op(
        &mut operation,
        &mut provenance,
        &recommended,
        &tool,
        machine,
        &stock.material,
        pass_role,
        feeds::suggest::SuggestContext::default(),
    );

    let figure = feeds::power_at_operating_point(
        &operation,
        &tool,
        &stock.material,
        machine,
        Some(feeds::suggest::CalculatorOperatingPoint {
            radial_width_mm: recommended.radial_width_mm,
            axial_depth_mm: recommended.axial_depth_mm,
            feed_rate_mm_min: recommended.feed_rate_mm_min,
            rpm: recommended.rpm,
        }),
    )
    .ok()?;
    Some(Recipe {
        utilisation_pct: figure.required_kw / figure.available_kw * 100.0,
        machine: machine.clone(),
        material: stock.material.clone(),
        tool,
        operation,
        name: format!(
            "{} / {species:?} / Ø{diameter:.0} / {family_name}",
            machine.name
        ),
    })
}

/// **The reason the ban expired, under test.**
///
/// The ban's evidence was a spread that could not separate a good cut from a
/// bad one: peak 23.6 % across the whole shipped matrix. This arm holds the
/// bar to a spread: the median sits below the peak and the minimum below the
/// median, so the readout is not pinned to one value, and the card paints
/// the value the door reads.
///
/// **Operator ruling 2026-09-25 (B6): no half-scale threshold.** The arm
/// also required one shipped recipe over 50 % of its ceiling. That figure
/// came from the retired force anchor with its 2.0 grain factor. On the B6
/// force line the population reads min 0.2 %, median 3.2 %, p90 14.6 %,
/// peak 31.5 % (measured). The operator ruled that a peak under half scale
/// is correct: a wood router is seldom power-bound.
///
/// The population is the review's, widened to every shipped species: three
/// machine presets × ten species × Ø3/Ø6/Ø12 × three milling families.
#[test]
fn the_power_bar_is_informative_g_chipverdict() {
    let mut recipes = Vec::new();
    for (_preset, machine) in MachineProfile::presets() {
        for species in WoodSpecies::ALL {
            for diameter in [3.0, 6.0, 12.0] {
                for family in sweep_families() {
                    if let Some(recipe) = shipped_recipe(&machine, species, diameter, family) {
                        recipes.push(recipe);
                    }
                }
            }
        }
    }
    assert!(
        recipes.len() >= 100,
        "the sweep read only {} recipes; the population is too small to say \
         anything about the spread",
        recipes.len()
    );
    recipes.sort_by(|a, b| a.utilisation_pct.partial_cmp(&b.utilisation_pct).unwrap());
    let at = |q: f64| recipes[((recipes.len() - 1) as f64 * q).round() as usize].utilisation_pct;
    let (min, median, p90, max) = (at(0.0), at(0.5), at(0.9), at(1.0));
    let peak = recipes.last().unwrap();
    eprintln!(
        "G-CHIPVERDICT power spread | {} recipes: min {min:.1} %, median \
         {median:.1} %, p90 {p90:.1} %, peak {max:.1} %. Peak recipe: {}",
        recipes.len(),
        peak.name,
    );

    assert!(
        median < max,
        "every shipped recipe reads {median:.1} % of its ceiling. A readout \
         that says the same thing on every job is the 1 %-forever power bar \
         again, whatever value it is pinned to."
    );
    assert!(
        min < median,
        "the lower half of the population is flat at {min:.1} %. The bar must \
         move across the range, not step between two values."
    );

    // The two assertions above measure the door. This one measures the
    // BAR: the heaviest shipped recipe, rendered on the card it ships on,
    // must paint the percent the door reads. Without it the arm would stay
    // green with no bar on the card at all, which is the state the ban left
    // behind.
    let texts = painted_text_of(state_for_recipe(
        Some(peak.machine.clone()),
        peak.tool.clone(),
        peak.material.clone(),
        peak.operation.clone(),
    ));
    let face = power_row(&texts).join(" ");
    let painted = painted_percent(&face);
    assert!(
        painted > 0.0 && (painted - max).abs() <= 1.0,
        "the heaviest shipped recipe ({}) reads {max:.1} % of its ceiling \
         through feeds::power_at_operating_point, but the card paints \
         {painted} %: {face:?}. The row must show what the door reads.",
        peak.name,
    );
}

// ── arm 4 — a refusal is stated, never drawn as a number ───────────────────

/// Acrylic has no measured force line (ruling B6), so `cut_efficiency` refuses outright.
/// The row must say so. Its partner below proves the suite is not passing by
/// abstaining everywhere.
#[test]
fn no_fabricated_number_survives_a_refusal_g_chipverdict() {
    let texts = painted_text(no_kc());
    let row = verdict_row(&texts);
    let face = row.join(" ");

    assert!(
        face.contains("not modelled"),
        "the no-Kc fixture painted {face:?}. An unmodelled material must say \
         it is unmodelled."
    );

    // The chipload beside the label is feed ÷ (RPM × flutes) — a commanded
    // value the force model plays no part in — so it is legitimate and is
    // excluded here. The verdict phrase carries only what the refused model
    // would have supplied, so any figure in it is fabricated.
    let verdict = row
        .last()
        .unwrap_or_else(|| panic!("the no-Kc row painted no verdict phrase: {row:#?}"));
    assert!(
        !verdict.chars().any(|c| c.is_ascii_digit()),
        "the no-Kc verdict phrase carries a number: {verdict:?}."
    );
    for fabricated in ['\u{00D7}', '%'] {
        assert!(
            !verdict.contains(fabricated),
            "the no-Kc verdict phrase claims `{fabricated}`: {verdict:?}. No \
             ratio and no headroom exist for a material with no Kc."
        );
    }

    let hover = verdict_hover(&texts);
    assert!(
        hover.contains("no measured force line") && hover.contains("MaterialUnvalidated"),
        "the no-Kc hover does not name the missing input or the engine's own \
         refusal: {hover:?}. A blank row reads as a clean bill of health."
    );
}

/// The non-vacuity partner. If the fixtures all abstained, the arm above
/// would pass while the row said nothing on any job.
#[test]
fn the_verdict_row_states_real_numbers_g_chipverdict() {
    let texts = painted_text(in_band());
    let face = verdict_row(&texts).join(" ");
    assert!(
        face.contains('%') && face.contains("mm/tooth"),
        "the in-band fixture painted {face:?}; it must quote a real chipload \
         and a real force headroom, or the refusal arm proves nothing."
    );
    assert!(
        !face.contains("not modelled"),
        "the in-band fixture abstained: {face:?}"
    );
}

/// The two optional fields refuse **independently**, and each refusal is its
/// own sentence. A V-bit pocket has no matched vendor row (so no band and no
/// ratios) and is declined by the deflection predictor (so no headroom) —
/// yet `u` and the ploughing share still stand, and are still shown.
#[test]
fn each_absent_field_states_its_own_abstention_g_chipverdict() {
    let texts = painted_text(no_band());
    let row = verdict_row(&texts);
    let verdict = row
        .last()
        .unwrap_or_else(|| panic!("the no-band row painted no verdict phrase: {row:#?}"));
    assert!(
        verdict.contains("no vendor chipload range"),
        "the no-band fixture painted {verdict:?}; the missing band must be \
         named, not left as a bare dash."
    );
    assert!(
        !verdict.contains('%') && !verdict.contains('\u{00D7}'),
        "the no-band fixture claimed a ratio or a headroom it does not have: \
         {verdict:?}"
    );

    let hover = verdict_hover(&texts);
    assert!(
        hover.contains("no row matched"),
        "the no-band hover does not say why there is no band: {hover:?}"
    );
    assert!(
        hover.contains("Force headroom: not modelled"),
        "the no-band hover does not state the deflection predictor's own \
         refusal: {hover:?}"
    );
    // The refusals are narrow. What the model still stands behind survives.
    assert!(
        hover.contains("J/mm\u{00B3}"),
        "a missing band deleted the specific energy too: {hover:?}. Each \
         field refuses on its own input, not on its neighbour's."
    );
}

/// Wood routing leaves the deflection bound almost untouched, so an
/// unrounded headroom reads `100 %` on most jobs — indistinguishable from
/// the absent constraint painted as an all-clear that this repository has
/// shipped four times. The face says `>99 %` instead.
#[test]
fn a_near_total_headroom_never_prints_as_a_flat_hundred_g_chipverdict() {
    let face = verdict_row(&painted_text(in_band())).join(" ");
    assert!(
        !face.contains("100 %"),
        "the verdict face printed a flat 100 %: {face:?}. Rounded up from \
         99.x %, it reads exactly like a constraint that was never evaluated."
    );
}
