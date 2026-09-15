//! **G-CHIPVERDICT — the Feeds tab says where the chip sits, and abstains
//! when it cannot.**
//!
//! # What was there, and why it went
//!
//! The inspector's comparison card ended with `Power 0.01 of 0.60 kW (1 %)`.
//! `feeds/mod.rs` already carried the measurement that condemned it: across
//! all three shipped machine presets × ten species × Ø3/Ø6/Ø12 slots, the
//! power branch never fires, and **peak** utilisation across the whole
//! shipped matrix is 23.6 %. Typical is 1 %. A readout whose maximum
//! observed value sits in its left quarter cannot separate a good cut from a
//! bad one, and an operator who learns to read it learns nothing.
//!
//! The chipload can separate them. Below the vendor band there is no
//! trade-off, only loss: the cut spends more energy per mm³ AND more time
//! than the band midpoint, at once. `feeds::efficiency::cut_efficiency`
//! computes that verdict; this row renders it.
//!
//! # The arm that matters
//!
//! `cut_efficiency` returns `Option<CutEfficiency>`, and three of its fields
//! are independently optional. Every one of those `None`s must reach the
//! screen as a **stated abstention** — never as a zero, a blank, a bare dash
//! or a 100 %. This repository has drawn the opposite four times: an empty
//! triage as an all-clear, an empty chart frame as a chart, a 2D model's
//! zero Z as a blank frame, and a hazard verdict from a pointer that was not
//! on the chart. `no_fabricated_number_survives_a_refusal_g_chipverdict` is
//! the fifth's tripwire, and
//! `the_verdict_row_states_real_numbers_g_chipverdict` is its non-vacuity
//! partner: a suite in which everything abstains would pass the refusal arm
//! and prove nothing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlasticFamily, PlywoodGrade, WoodSpecies};
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
    material: Material,
    operation: OperationConfig,
}

/// Baltic birch plywood under a 3D adaptive pass: the derate chain lands the
/// recommendation below the matched vendor band, so the verdict is `Thin`
/// and both ratios exist.
fn thin() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        operation: OperationConfig::Adaptive3d(Default::default()),
    }
}

/// Softwood under a pocket: the shallower recommended DOC drops the depth
/// tier derate, and the recommendation lands inside the vendor band.
fn in_band() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        operation: OperationConfig::Pocket(Default::default()),
    }
}

/// A V-bit pocket. No vendor row routes to this pairing, so there is no
/// band; and the deflection predictor declines a V-bit, so there is no
/// headroom either. Two independent refusals in one row.
fn no_band() -> Fixture {
    Fixture {
        tool_type: ToolType::VBit,
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        operation: OperationConfig::Pocket(Default::default()),
    }
}

/// Acrylic. `material::kc_n_per_mm2` refuses — no primary-source Kc has been
/// fetched for PMMA — so `force::affine_coefficients` refuses and
/// `cut_efficiency` returns `None` outright.
fn no_kc() -> Fixture {
    Fixture {
        tool_type: ToolType::EndMill,
        material: Material::Plastic {
            family: PlasticFamily::Acrylic,
        },
        operation: OperationConfig::Pocket(Default::default()),
    }
}

fn state_for(fixture: Fixture) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), fixture.tool_type);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    tool.stickout = 18.0;

    let stock = StockConfig {
        material: fixture.material,
        ..Default::default()
    };

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
        other => panic!("fixture operation {other:?} has no feed/RPM setter here"),
    }

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
    builder
        .add_toolpath(0, config)
        .expect("add the chip verdict fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, "feeds".to_owned()));
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
    let ctx = ctx();
    let mut state = state_for(fixture);
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

fn compare_source() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds/compare.rs"))
        .expect("read src/ui/feeds/compare.rs")
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
/// The thin and in-band fixtures differ only in material and operation, and
/// the painted verdict must differ with them.
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

// ── arm 3 — the power readout does not come back ───────────────────────────

#[test]
fn no_power_gauge_returns_to_the_card_g_chipverdict() {
    let source = compare_source();
    // Strip line comments: this file and its neighbours discuss the deleted
    // gauge by name, and a scan that reads a comment reports code that is
    // absent.
    let code: String = source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for banned in ["ProgressBar", "power_bar", "power_color", "rail_power_row"] {
        assert!(
            !code.contains(banned),
            "src/ui/feeds/compare.rs builds `{banned}` again. The power \
             readout peaked at 23.6 % across the entire shipped matrix; it \
             cannot separate a good cut from a bad one."
        );
    }

    let texts = painted_text(in_band());
    assert!(
        !texts.iter().any(|t| t.contains(" kW")),
        "the comparison card painted a kW figure again: {texts:#?}"
    );
}

// ── arm 4 — a refusal is stated, never drawn as a number ───────────────────

/// Acrylic has no primary-source Kc, so `cut_efficiency` refuses outright.
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
        hover.contains("primary-source Kc") && hover.contains("MaterialUnvalidated"),
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
