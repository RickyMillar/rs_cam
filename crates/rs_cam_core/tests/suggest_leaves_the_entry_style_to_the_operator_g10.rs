//! G10 Q11 — Suggest never writes the entry style; the operator owns it.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/G10_PLAN.md` (§1 F4
//! and F6, §3 A6, §3 A10 item 3).
//!
//! Before G10, `pick_adaptive3d_entry_style` (Suggest's strategy pass)
//! rewrote an Adaptive3d `Plunge` entry to `Ramp` or `Helix` when the depth
//! per pass was over half the tool diameter and the field was at its
//! default, and filed `StrategyRewrote`. Ruling Q11 (2026-09-25) deleted the
//! rewrite: Suggest keeps the style and warns (`PlungeEntryUnstableAtDpp`).
//! The Plunge arms below fail on the pre-G10 code: the style came back
//! `Ramp` (or `Helix` over a model bbox with helix headroom).
//!
//! The fixture is the Wanaka Back Rough scaffold: a 6 mm 2-flute end mill
//! at DPP 3.69 mm (DPP / D = 0.615) in hard maple, stickout 30 mm (short, so
//! the deflection back-off does not lower the depth under 0.5 D).
//!
//! - The shipped op: the apply funnel with the Speeds scope keeps the
//!   operator's depth, so the depth stays at 0.615 D.
//! - `s.operation`: the full Suggest (feeds and cut geometry).
//!
//! Each runs with and without a model bbox.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, SuggestContext, SuggestForOperationInput, SuggestWarning, apply,
    feeds_preview_for_operation, suggest_for_operation,
};
use rs_cam_core::feeds::{FeedsProvenance, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const DPP_MM: f64 = 3.69;

fn tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.stickout = 30.0;
    tool.flute_count = 2;
    tool
}

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// The Wanaka stock box, 140 x 150 x 50 mm: room for a helix.
fn bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, -50.0),
        max: P3::new(140.0, 150.0, 0.0),
    }
}

fn op(style: Adaptive3dEntryStyle) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        depth_per_pass: DPP_MM,
        stepover: 1.2,
        feed_rate: 911.0,
        spindle_rpm: Some(16_000),
        entry_style: style,
        ..Adaptive3dConfig::default()
    })
}

fn style_of(op: &OperationConfig) -> Adaptive3dEntryStyle {
    let OperationConfig::Adaptive3d(cfg) = op else {
        panic!("the op must stay Adaptive3d")
    };
    cfg.entry_style
}

fn unstable(warnings: &[SuggestWarning]) -> bool {
    warnings
        .iter()
        .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. }))
}

/// The apply funnel (Speeds scope) on the operator's op: the style that
/// ships, and the warnings.
fn shipped(
    style: Adaptive3dEntryStyle,
    model_bbox: Option<&BoundingBox3>,
) -> (OperationConfig, Vec<SuggestWarning>) {
    let machine = MachineProfile::default();
    let (tool, material) = (tool(), material());
    let base = op(style);
    let preview = feeds_preview_for_operation(
        &base,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    let rec = preview.applicable().expect("the Adaptive3d cell validates");
    let mut applied = base.clone();
    let mut prov = FeedsProvenance::default();
    let warnings = apply(
        &rec,
        ApplyScope::Speeds,
        &mut applied,
        &mut prov,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: base.feeds_style().1,
            suggest: SuggestContext {
                model_bbox,
                ..SuggestContext::default()
            },
        },
    );
    (applied, warnings)
}

/// A deep Plunge entry stays Plunge, on the shipped op and in
/// `s.operation`, and `PlungeEntryUnstableAtDpp` fires.
#[test]
fn a_deep_plunge_entry_stays_a_plunge_and_warns_g10() {
    let bbox = bbox();
    for model_bbox in [Some(&bbox), None] {
        let case = format!("bbox {}", model_bbox.is_some());
        let (applied, warnings) = shipped(Adaptive3dEntryStyle::Plunge, model_bbox);
        assert_eq!(applied.depth_per_pass(), Some(DPP_MM), "{case}");
        assert_eq!(
            style_of(&applied),
            Adaptive3dEntryStyle::Plunge,
            "{case}: the funnel moved the style"
        );
        assert!(unstable(&warnings), "{case}: no warning: {warnings:?}");

        let operation = op(Adaptive3dEntryStyle::Plunge);
        let (tool, material) = (tool(), material());
        let s = suggest_for_operation(SuggestForOperationInput {
            operation: &operation,
            tool: &tool,
            machine: &MachineProfile::default(),
            material: &material,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::MatchChart,
            context: SuggestContext {
                model_bbox,
                ..SuggestContext::default()
            },
        })
        .expect("Suggest serves the Adaptive3d cell");
        assert_eq!(
            style_of(&s.operation),
            Adaptive3dEntryStyle::Plunge,
            "{case}: Suggest moved the style"
        );
        // Non-vacuity: the depth Suggest ships is still over 0.5 D, the
        // depth at which the pre-G10 pass rewrote the style.
        let deep = s.operation.depth_per_pass().is_some_and(|d| d / 6.0 > 0.5);
        assert!(deep, "{case}: depth {:?}", s.operation.depth_per_pass());
        assert!(
            unstable(&s.warnings),
            "{case}: no warning: {:?}",
            s.warnings
        );
    }
}

/// An operator Helix or Ramp is kept, and does not warn.
#[test]
fn an_operator_helix_or_ramp_is_kept_g10() {
    let bbox = bbox();
    for style in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
        for model_bbox in [Some(&bbox), None] {
            let case = format!("{style:?} bbox {}", model_bbox.is_some());
            let (applied, warnings) = shipped(style, model_bbox);
            assert_eq!(style_of(&applied), style, "{case}");
            assert!(!unstable(&warnings), "{case}: {warnings:?}");
        }
    }
}
