//! **G-VISIBLE — every stage that moves a number is a line on the Feeds card.**
//!
//! # The rule
//!
//! The operator's standing rule (ruling R4, 2026-09-24): no invisible
//! calculations or de-rates. Every stage that moves a number shows on the
//! card, with its source status. The 0.75 safety factor broke this rule for
//! years: it cut every feed, and no line on the card named it.
//!
//! # What this pins
//!
//! 1. Every [`FeedsWarning`] and [`SuggestWarning`] variant is classified by
//!    an exhaustive `match` ([`classify_feeds`], [`classify_suggest`]). A new
//!    variant does not compile until it is classified here.
//! 2. A variant classified [`Class::OnTheFace`] that a fixture raises must
//!    paint a text run on the card FACE that carries its numbers, at the
//!    precision the card prints them. A face run is a run with no newline:
//!    every row hover starts with its value and a newline, so a hover cannot
//!    satisfy this arm.
//! 3. Each fixture must raise the variants it names in `must_raise`, so the
//!    arm is not vacuous.
//!
//! The fixtures render the real inspector through a headless
//! `egui::Context`, as `inspector_width_is_tab_independent_up4` does. The
//! warnings come from the same doors the card calls: the calculator result
//! that the Feeds tab caches on the runtime row, and
//! `suggest_for_operation` with the card's own context.
//!
//! # The fixtures, and the variants each one raises
//!
//! - `adaptive_long_tool`: the `the_recommendation_explains_each_row_g_whyrow`
//!   cell, a Ø6 flat `Adaptive3d` in Baltic birch on the default stickout
//!   45 mm (7.5 x D). Raises `LongToolDerate` and
//!   `EngagementReducedForAggressiveness` (0.85 x 0.75 = 64 %).
//! - `slow_gantry`: the `inspector_width_is_tab_independent_up4` cell on a
//!   cutting-feed ceiling of 200 mm/min. Raises `FeedRateClamped`.
//! - `weak_spindle`: the adaptive cell on a 0.05 kW constant-power spindle.
//!   Raises `PowerLadderReducedCut`.
//! - `short_flute`: the adaptive cell on a tool with 2 mm of flute. Raises
//!   `DocExceedsFlute`.
//! - `finish`: the first Finish-role operation and tool type that ships a
//!   recipe, in generic hardwood, else softwood plywood. Raises
//!   `AggressivenessNotApplied` (`FinishRole`).
//! - `drill`: the first drill operation and tool type that ships a recipe in
//!   softwood plywood, else HDF (materials the R1 judgement does not cover,
//!   so the drill is not refused). Raises `AggressivenessNotApplied`
//!   (`Drill`).
//!
//! # `OnTheFace` variants that no fixture here is known to raise
//!
//! The arm checks each of these whenever a fixture raises it. No fixture is
//! required to raise them, for the reason given:
//!
//! - `FeedsWarning::SlottingDetected`: the calculator raises it on a full
//!   width cut at a depth over its slotting rule; the panel's default
//!   operations do not command a full-width stepover.
//! - `FeedsWarning::DrillFeedClampedToEnvelope`: the material envelope
//!   brackets the drill recipe on every shipped drill cell (spec R4 §1.2,
//!   S14 fires on 0 cells).
//! - `SuggestWarning::FeedRescaledToFinalGeometry`: pass 9 files it only when
//!   the final depth crosses a step of the published depth ladder. Whether
//!   the dial's smaller depth crosses one depends on the cell (FM1: 24 cells).
//!
//! # `RowHover` variants
//!
//! These move a number, and the rationale row of the number they move
//! carries them on its hover (`why::append_rationale`), as the declutter
//! ruling of 2026-09-15 placed them. This file does not require a face line
//! for them. Whether the standing rule of 2026-09-24 moves them to the face
//! is an open question for the operator; the classification names each one
//! so that the answer is one edit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    AggressivenessSkip, SuggestContext, SuggestForOperationInput, SuggestWarning,
    suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsWarning, OperationFamily, PassRole, SpindleStrategy, WorkholdingRigidity,
    embedded_vendor_lut,
};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector rail the Toolpaths workspace draws at.
const PANEL_WIDTH: f32 = 280.0;

// ── the classification ───────────────────────────────────────────────────

/// Where a record must show on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// The stage moves a number (or states that the dial did not act). Its
    /// line is on the card face and carries its numbers.
    OnTheFace,
    /// The stage moves a number. The hover of the row it moves carries it.
    RowHover,
    /// The record moves no number. It reports a condition.
    NoNumberMoves,
}

/// Every calculator record, classified. Exhaustive by design.
fn classify_feeds(w: &FeedsWarning) -> (&'static str, Class) {
    match w {
        FeedsWarning::FeedRateClamped { .. } => ("FeedRateClamped", Class::OnTheFace),
        FeedsWarning::PowerLimited { .. } => ("PowerLimited", Class::NoNumberMoves),
        FeedsWarning::PowerLadderReducedCut { .. } => ("PowerLadderReducedCut", Class::OnTheFace),
        FeedsWarning::ShankTooLarge { .. } => ("ShankTooLarge", Class::NoNumberMoves),
        FeedsWarning::DocExceedsFlute { .. } => ("DocExceedsFlute", Class::OnTheFace),
        FeedsWarning::SlottingDetected { .. } => ("SlottingDetected", Class::OnTheFace),
        FeedsWarning::ScallopInvalid { .. } => ("ScallopInvalid", Class::NoNumberMoves),
        // Ruling R4 WP2a: the engine warns and does not lift the feed.
        FeedsWarning::ChiploadBelowRubbingFloor { .. } => {
            ("ChiploadBelowRubbingFloor", Class::NoNumberMoves)
        }
        // Ruling R4 Q7: a share of the load target, which the dial record
        // then spends on the depth and the stepover.
        FeedsWarning::LongToolDerate { .. } => ("LongToolDerate", Class::OnTheFace),
        FeedsWarning::VendorRowPublishesNoChipload { .. } => {
            ("VendorRowPublishesNoChipload", Class::NoNumberMoves)
        }
        FeedsWarning::NoVendorRowsForRoutedOperation { .. } => {
            ("NoVendorRowsForRoutedOperation", Class::NoNumberMoves)
        }
        FeedsWarning::DrillFeedClampedToEnvelope { .. } => {
            ("DrillFeedClampedToEnvelope", Class::OnTheFace)
        }
    }
}

/// Every Suggest record, classified. Exhaustive by design.
fn classify_suggest(w: &SuggestWarning) -> (&'static str, Class) {
    match w {
        SuggestWarning::EngagementReducedForAggressiveness { .. } => {
            ("EngagementReducedForAggressiveness", Class::OnTheFace)
        }
        SuggestWarning::AggressivenessNotApplied { .. } => {
            ("AggressivenessNotApplied", Class::OnTheFace)
        }
        SuggestWarning::FeedRescaledToFinalGeometry { .. } => {
            ("FeedRescaledToFinalGeometry", Class::OnTheFace)
        }
        SuggestWarning::PlungeClampedToFeed { .. } => ("PlungeClampedToFeed", Class::RowHover),
        SuggestWarning::StepoverClampedToToolDiameter { .. } => {
            ("StepoverClampedToToolDiameter", Class::RowHover)
        }
        SuggestWarning::RoughingDepthClampedToRigidity { .. } => {
            ("RoughingDepthClampedToRigidity", Class::RowHover)
        }
        SuggestWarning::DepthClampedToCuttingLength { .. } => {
            ("DepthClampedToCuttingLength", Class::RowHover)
        }
        SuggestWarning::DppCappedByDeflection { .. } => ("DppCappedByDeflection", Class::RowHover),
        SuggestWarning::StepoverRaisedForRuntime { .. } => {
            ("StepoverRaisedForRuntime", Class::RowHover)
        }
        SuggestWarning::FeedRaisedForChipload { .. } => ("FeedRaisedForChipload", Class::RowHover),
        SuggestWarning::StrategyRewrote { .. } => ("StrategyRewrote", Class::RowHover),
        SuggestWarning::AxialDocClampedByEnvelope { .. } => {
            ("AxialDocClampedByEnvelope", Class::RowHover)
        }
        SuggestWarning::PowerRecheckedAfterRescale { .. } => {
            ("PowerRecheckedAfterRescale", Class::RowHover)
        }
        SuggestWarning::PlungeEntryUnstableAtDpp { .. } => {
            ("PlungeEntryUnstableAtDpp", Class::NoNumberMoves)
        }
        SuggestWarning::DeflectionBackoffUnmodeled { .. } => {
            ("DeflectionBackoffUnmodeled", Class::NoNumberMoves)
        }
        SuggestWarning::DeflectionBackoffFigureIsAFloor { .. } => {
            ("DeflectionBackoffFigureIsAFloor", Class::NoNumberMoves)
        }
        SuggestWarning::ChiploadStillLowAfterRecalibration { .. } => {
            ("ChiploadStillLowAfterRecalibration", Class::NoNumberMoves)
        }
        SuggestWarning::StrategyRecommendedNotApplied { .. } => {
            ("StrategyRecommendedNotApplied", Class::NoNumberMoves)
        }
        SuggestWarning::AxialEnvelopeSafeBandEmpty { .. } => {
            ("AxialEnvelopeSafeBandEmpty", Class::NoNumberMoves)
        }
        SuggestWarning::AxialDocBelowBurnFloor { .. } => {
            ("AxialDocBelowBurnFloor", Class::NoNumberMoves)
        }
        SuggestWarning::ProjectCurveDepthInfeasible { .. } => {
            ("ProjectCurveDepthInfeasible", Class::NoNumberMoves)
        }
        SuggestWarning::FinishEnvelopeAdvisory { .. } => {
            ("FinishEnvelopeAdvisory", Class::NoNumberMoves)
        }
        // Ruling R4 WP2a: pass 9 warns and does not lift the feed.
        SuggestWarning::FeedClampedToChiploadFloor { .. } => {
            ("FeedClampedToChiploadFloor", Class::NoNumberMoves)
        }
        SuggestWarning::CutGeometryFieldNotHeld { .. } => {
            ("CutGeometryFieldNotHeld", Class::NoNumberMoves)
        }
    }
}

/// A number at the precision the card prints it.
fn at(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

/// Both halves of a pair, when both are present.
fn pair(out: &mut Vec<String>, a: Option<f64>, b: Option<f64>, decimals: usize) {
    if let (Some(a), Some(b)) = (a, b) {
        out.push(at(a, decimals));
        out.push(at(b, decimals));
    }
}

/// The numbers the face line of a calculator record must carry. Empty for a
/// record that is not [`Class::OnTheFace`].
fn feeds_numbers(w: &FeedsWarning) -> Vec<String> {
    let mut out = Vec::new();
    match w {
        FeedsWarning::FeedRateClamped { requested, actual } => {
            out.push(at(*requested, 0));
            out.push(at(*actual, 0));
        }
        FeedsWarning::PowerLadderReducedCut {
            rpm_from,
            rpm_to,
            axial_from,
            axial_to,
            radial_from,
            radial_to,
            feed_factor,
            ..
        } => {
            pair(&mut out, *rpm_from, *rpm_to, 0);
            pair(&mut out, *axial_from, *axial_to, 2);
            pair(&mut out, *radial_from, *radial_to, 2);
            if let Some(f) = feed_factor {
                out.push(at(*f, 3));
            }
        }
        FeedsWarning::DocExceedsFlute { requested, capped } => {
            out.push(at(*requested, 1));
            out.push(at(*capped, 1));
        }
        FeedsWarning::SlottingDetected { doc_reduced_to } => out.push(at(*doc_reduced_to, 1)),
        FeedsWarning::LongToolDerate {
            stickout_mm,
            ratio,
            factor,
            ..
        } => {
            out.push(at(*factor, 2));
            out.push(at(*ratio, 1));
            out.push(at(*stickout_mm, 0));
        }
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested, actual, ..
        } => {
            out.push(at(*requested, 0));
            out.push(at(*actual, 0));
        }
        _ => {}
    }
    out
}

/// The numbers the face line of a Suggest record must carry. Empty for a
/// record that is not [`Class::OnTheFace`].
fn suggest_numbers(w: &SuggestWarning) -> Vec<String> {
    let mut out = Vec::new();
    match w {
        SuggestWarning::EngagementReducedForAggressiveness {
            aggressiveness,
            target_share,
            dpp_from,
            dpp_to,
            stepover_from,
            stepover_to,
            force_n_before,
            force_n_after,
            power_kw_before,
            power_kw_after,
            section_mm2_before,
            section_mm2_after,
            ..
        } => {
            out.push(format!("Aggressiveness {}", at(*aggressiveness, 2)));
            out.push(format!("{} %", at(target_share * 100.0, 0)));
            pair(&mut out, *dpp_from, *dpp_to, 2);
            pair(&mut out, *stepover_from, *stepover_to, 2);
            pair(&mut out, *force_n_before, *force_n_after, 0);
            pair(&mut out, *power_kw_before, *power_kw_after, 2);
            pair(&mut out, *section_mm2_before, *section_mm2_after, 2);
        }
        SuggestWarning::AggressivenessNotApplied {
            aggressiveness,
            reason,
        } => {
            out.push(format!("Aggressiveness {}", at(*aggressiveness, 2)));
            out.push(reason.card_text().to_owned());
        }
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            rescaled_mm_per_min,
            factor_at_calculator,
            factor_at_final,
            ..
        } => {
            out.push(at(*requested_mm_per_min, 0));
            out.push(at(*rescaled_mm_per_min, 0));
            out.push(at(*factor_at_calculator, 2));
            out.push(at(*factor_at_final, 2));
        }
        _ => {}
    }
    out
}

// ── the harness ──────────────────────────────────────────────────────────

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Opens every disclosure. It also paints tooltips; the face test below
    // excludes them (a hover run always holds a newline).
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// One fixture: a session on one tool, material and operation, on an
/// optional machine, and the variants it must raise.
struct Case {
    name: &'static str,
    tool: ToolConfig,
    material: Material,
    operation: OperationConfig,
    machine: Option<MachineProfile>,
    must_raise: &'static [&'static str],
}

fn state_for(case: &Case) -> AppState {
    let stock = StockConfig {
        material: case.material.clone(),
        ..Default::default()
    };
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Visible-stage fixture".to_owned(),
        enabled: true,
        operation: case.operation.clone(),
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
        path: PathBuf::from("visible_stage_fixture.svg"),
        name: "Visible-stage fixture".to_owned(),
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
        .tool(case.tool.clone())
        .model(model);
    if let Some(machine) = &case.machine {
        builder = builder.machine(machine.clone());
    }
    builder
        .add_toolpath(0, config)
        .expect("add the visible-stage fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::FeedsSpeeds));
    state
}

/// Every text run the Feeds tab paints on its steady-state frame.
fn painted_text(state: &mut AppState) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("visible_stage_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
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

/// The records behind the card: the calculator's (cached on the runtime row
/// by the Feeds tab) and the Suggest run's, with the card's own context.
fn records(state: &AppState) -> (Vec<FeedsWarning>, Vec<SuggestWarning>) {
    let id = state.session.toolpath_configs()[0].id;
    let feeds = state
        .gui
        .toolpath_rt
        .get(&id)
        .and_then(|runtime| runtime.feeds_result.as_ref())
        .map(|result| result.warnings.clone())
        .expect("the Feeds tab must calculate and cache its recipe");
    let snapshot = properties::toolpath_panel_snapshot(id, &state.session, &state.gui)
        .expect("the toolpath resolves for the properties panel");
    let session = &state.session;
    let tool = session.tools()[0].clone();
    let suggest = suggest_for_operation(SuggestForOperationInput {
        operation: &snapshot.entry.operation,
        tool: &tool,
        machine: session.machine(),
        material: &session.stock_config().material,
        workholding: session.stock_config().workholding_rigidity,
        lut: embedded_vendor_lut(),
        spindle_strategy: session.post_config().spindle_strategy,
        context: SuggestContext {
            model_bbox: snapshot.model_bbox.as_ref(),
            ..SuggestContext::default()
        },
    })
    .map(|suggested| suggested.warnings)
    .unwrap_or_default();
    (feeds, suggest)
}

/// Render `case`, then check every face record and the required set.
/// Returns the names of the face variants the case raised.
fn check(case: &Case) -> Vec<&'static str> {
    let mut state = state_for(case);
    let texts = painted_text(&mut state);
    let (feeds, suggest) = records(&state);
    let face_runs: Vec<&String> = texts.iter().filter(|t| !t.contains('\n')).collect();

    let mut raised = Vec::new();
    let mut checked = Vec::new();
    for (name, class, numbers) in feeds
        .iter()
        .map(|w| {
            let (name, class) = classify_feeds(w);
            (name, class, feeds_numbers(w))
        })
        .chain(suggest.iter().map(|w| {
            let (name, class) = classify_suggest(w);
            (name, class, suggest_numbers(w))
        }))
    {
        raised.push(name);
        if class != Class::OnTheFace {
            continue;
        }
        assert!(
            !numbers.is_empty(),
            "{}: {name} is on the face but names no number to look for",
            case.name
        );
        let found = face_runs
            .iter()
            .any(|run| numbers.iter().all(|n| run.contains(n.as_str())));
        assert!(
            found,
            "{}: {name} moved a number and no line on the card face carries it. \
             Every stage that moves a number is one line on the card, with its \
             source status (ruling R4, 2026-09-24). Looked for {numbers:?} in the \
             face runs {face_runs:?}",
            case.name
        );
        checked.push(name);
    }
    for required in case.must_raise {
        assert!(
            raised.contains(required),
            "{}: the fixture no longer raises {required}, so its arm checks \
             nothing. Raised: {raised:?}",
            case.name
        );
    }
    checked
}

// ── the fixtures ─────────────────────────────────────────────────────────

/// The whyrow cell: a Ø6 flat 2F `Adaptive3d` in Baltic birch, on the
/// default stickout 45 mm (7.5 x D).
fn adaptive_tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    tool
}

fn adaptive_operation() -> OperationConfig {
    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    operation
}

fn baltic_birch() -> Material {
    Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    }
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// The first material, operation and tool type that ship a Suggest recipe
/// and file the "no dial action" record `skip`. The search keeps the fixture
/// valid when a row moves or R1 refuses a cell.
fn first_skip_cell(
    materials: &[Material],
    skip: AggressivenessSkip,
) -> (Material, OperationConfig, ToolConfig) {
    let machine = MachineProfile::generic_wood_router();
    for material in materials {
        for &op in OperationType::ALL {
            let operation = OperationConfig::new_default(op);
            let (family, role) = operation.feeds_style();
            let wanted = match skip {
                AggressivenessSkip::Drill => family == OperationFamily::Drill,
                AggressivenessSkip::FinishRole => {
                    role == PassRole::Finish && family != OperationFamily::Drill
                }
            };
            if !wanted {
                continue;
            }
            for &tool_type in ToolType::ALL {
                let tool = ToolConfig::new_default(ToolId(1), tool_type);
                let Ok(suggested) = suggest_for_operation(SuggestForOperationInput {
                    operation: &operation,
                    tool: &tool,
                    machine: &machine,
                    material,
                    workholding: WorkholdingRigidity::Medium,
                    lut: embedded_vendor_lut(),
                    spindle_strategy: SpindleStrategy::MatchChart,
                    context: SuggestContext::default(),
                }) else {
                    continue;
                };
                let files_skip = suggested.warnings.iter().any(|w| {
                    matches!(
                        w,
                        SuggestWarning::AggressivenessNotApplied { reason, .. } if *reason == skip
                    )
                });
                if files_skip {
                    return (material.clone(), operation, tool);
                }
            }
        }
    }
    panic!("no operation and tool type in {materials:?} files the {skip:?} record");
}

fn cases() -> Vec<Case> {
    let mut slow_gantry = MachineProfile::generic_wood_router();
    slow_gantry.max_cutting_feed_mm_min = Some(200.0);

    let mut weak_spindle = MachineProfile::generic_wood_router();
    weak_spindle.power = PowerModel::ConstantPower { power_kw: 0.05 };

    let mut short_flute = adaptive_tool();
    short_flute.cutting_length = 2.0;

    let mut profile_tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    profile_tool.diameter = 3.175;
    profile_tool.flute_count = 2;
    profile_tool.cutting_length = 12.0;
    profile_tool.shank_diameter = 3.175;
    profile_tool.shaft_diameter = 3.175;
    profile_tool.stickout = 20.0;
    let mut profile = OperationConfig::Profile(Default::default());
    if let OperationConfig::Profile(config) = &mut profile {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }

    let softwood_plywood = Material::Plywood {
        grade: PlywoodGrade::Softwood,
    };
    let hdf = Material::SheetGood {
        kind: SheetGoodKind::Hdf,
    };
    let (finish_material, finish_op, finish_tool) = first_skip_cell(
        &[hardwood(), softwood_plywood.clone()],
        AggressivenessSkip::FinishRole,
    );
    let (drill_material, drill_op, drill_tool) =
        first_skip_cell(&[softwood_plywood, hdf], AggressivenessSkip::Drill);

    vec![
        Case {
            name: "adaptive_long_tool",
            tool: adaptive_tool(),
            material: baltic_birch(),
            operation: adaptive_operation(),
            machine: None,
            must_raise: &["LongToolDerate", "EngagementReducedForAggressiveness"],
        },
        Case {
            name: "slow_gantry",
            tool: profile_tool,
            material: hardwood(),
            operation: profile,
            machine: Some(slow_gantry),
            must_raise: &["FeedRateClamped"],
        },
        Case {
            name: "weak_spindle",
            tool: adaptive_tool(),
            material: baltic_birch(),
            operation: adaptive_operation(),
            machine: Some(weak_spindle),
            must_raise: &["PowerLadderReducedCut"],
        },
        Case {
            name: "short_flute",
            tool: short_flute,
            material: baltic_birch(),
            operation: adaptive_operation(),
            machine: None,
            must_raise: &["DocExceedsFlute"],
        },
        Case {
            name: "finish",
            tool: finish_tool,
            material: finish_material,
            operation: finish_op,
            machine: None,
            must_raise: &["AggressivenessNotApplied"],
        },
        Case {
            name: "drill",
            tool: drill_tool,
            material: drill_material,
            operation: drill_op,
            machine: None,
            must_raise: &["AggressivenessNotApplied"],
        },
    ]
}

// ── the arms ─────────────────────────────────────────────────────────────

/// Every face record a fixture raises paints its numbers on the face, and
/// each fixture raises the records it names.
#[test]
fn every_stage_that_moves_a_number_is_on_the_card_g_visible() {
    let mut checked = Vec::new();
    for case in cases() {
        checked.extend(check(&case));
    }
    // Non-vacuity across the file: the two dial records and the long-tool
    // share were each found on a face.
    for name in [
        "EngagementReducedForAggressiveness",
        "AggressivenessNotApplied",
        "LongToolDerate",
    ] {
        assert!(
            checked.contains(&name),
            "no fixture checked {name} on the face; checked {checked:?}"
        );
    }
}

/// The dial line states the load target as a percent of the base, and the
/// long-tool share inside it. On the adaptive cell that is
/// 0.85 x 0.75 = 64 %.
#[test]
fn the_dial_line_names_the_long_tool_share_g_visible() {
    let case = cases().remove(0);
    let mut state = state_for(&case);
    let texts = painted_text(&mut state);
    let line = texts
        .iter()
        .find(|t| !t.contains('\n') && t.contains("Aggressiveness 0.85"))
        .unwrap_or_else(|| panic!("no aggressiveness line on the face: {texts:?}"));
    assert!(
        line.contains("× 0.75 long tool = 64 %"),
        "the dial line must name the long-tool share and the target: {line:?}"
    );
}
