//! The shared fixtures of the K2b envelope plot sentries (feeds contract
//! P5b): an envelope per op type, and a headless egui frame that draws
//! `ui::feeds::envelope::draw_envelope`.

#![allow(
    dead_code,
    reason = "each sentry binary uses a different part of this module"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::ids::{ToolId, ToolpathId};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::feeds::envelope::{EnvelopeResponse, draw_envelope};

/// The toolpath id of every fixture.
pub const ID: ToolpathId = ToolpathId::from_raw(7);

/// The envelope of the fixture op of `op` with the first tool type that
/// the calculator backs (a 6 mm end mill, a 6 mm ball, a 90 degree V-bit),
/// or `None` when none is backed.
pub fn envelope_for(op: OperationType) -> Option<(OperationConfig, SafeEnvelope)> {
    envelope_of(rs_cam_core::compute::fixture_values::fixture_op(op))
}

/// The envelope of `config` with the first backed tool type, as
/// [`envelope_for`] (P5c: a fixture op with an edited stated point).
pub fn envelope_of(config: OperationConfig) -> Option<(OperationConfig, SafeEnvelope)> {
    for kind in [ToolType::EndMill, ToolType::BallNose, ToolType::VBit] {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(1), kind);
        tool.diameter = 6.0;
        let machine = MachineProfile::default();
        let material = Material::default();
        if let Ok(env) = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &config,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
            context: rs_cam_core::feeds::suggest::SuggestContext::default(),
        }) {
            return Some((config, env));
        }
    }
    None
}

/// A context with the app tokens and fonts, warmed once.
pub fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    rs_cam_viz::ui::tokens::apply(&ctx);
    rs_cam_viz::ui::tokens::apply_fonts(&ctx);
    // A tooltip paints on the frame the pointer arrives (the hover arms).
    ctx.all_styles_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// What one frame painted and sent.
pub struct Frame {
    pub shapes: Vec<egui::epaint::ClippedShape>,
    pub events: Vec<AppEvent>,
    pub response: EnvelopeResponse,
}

/// The input of one frame: a screen rect 360 points wide (the inspector)
/// and `events`.
pub fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(360.0, 600.0),
        )),
        events,
        ..Default::default()
    }
}

/// Draw the envelope section once.
pub fn frame(
    ctx: &egui::Context,
    config: &OperationConfig,
    env: Option<&SafeEnvelope>,
    candidates: &mut EnvelopeCandidates,
    events: Vec<egui::Event>,
) -> Frame {
    let mut sent = Vec::new();
    let mut response = EnvelopeResponse::default();
    let mut out = ctx.run_ui(input(events), |ui| {
        response = draw_envelope(
            ui,
            ID,
            config,
            env,
            Some("no data for this tool and material"),
            candidates,
            &mut sent,
        );
    });
    out.textures_delta.clear();
    Frame {
        shapes: out.shapes,
        events: sent,
        response,
    }
}

/// Every text the frame painted.
pub fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|c| {
            if let egui::epaint::Shape::Text(t) = &c.shape {
                Some(t.galley.job.text.clone())
            } else {
                None
            }
        })
        .collect()
}

/// A press and a release at `pos`: a click.
pub fn click_at(pos: egui::Pos2) -> Vec<Vec<egui::Event>> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![
        vec![egui::Event::PointerMoved(pos)],
        vec![button(true)],
        vec![button(false)],
        vec![egui::Event::PointerMoved(pos)],
    ]
}

/// The colour of a solid path stroke; a callback colour reads as
/// transparent.
pub fn solid(color: &egui::epaint::ColorMode) -> egui::Color32 {
    if let egui::epaint::ColorMode::Solid(c) = color {
        *c
    } else {
        egui::Color32::TRANSPARENT
    }
}
