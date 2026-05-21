//! Load-time validator for stale defaults.
//!
//! Projects saved before a default-improvement landed carry the pre-change
//! values forever. This module flags known-stale patterns at project-load
//! time with per-rule auto-fix closures. See
//! `planning/F5_FRESH_DEFAULTS_POLICY.md` (decision doc) and
//! `planning/P5_STALE_DEFAULTS_VALIDATOR_RCA.md`.
//!
//! The validator is **conservative**: each rule has a specific detection
//! pattern (e.g. `min_z <= -49.999` for B.1) and a targeted auto-fix that
//! sets one field to its post-improvement default. No full LUT re-derivation
//! — that would clobber user customisations.
//!
//! Initial rule library covers the three Wanaka findings:
//! - [`StaleDefaultRule::DropCutterMinZPreB1`] — B.1
//! - [`StaleDefaultRule::TaperedBallPlungePreFix2`] — Fix 2
//! - [`StaleDefaultRule::WoodAdaptiveStepoverPreFix1`] — Fix 1
//! - [`StaleDefaultRule::ProjectCurveNegativeDepth`] — A1 (UX dial-in 2026-05-20)
//!
//! Adding a rule per future B-roadmap entry is the ongoing convention
//! (see F5 doc).

use crate::compute::catalog::OperationConfig;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::feeds::ToolGeometryHint;
use crate::material::Material;
use crate::session::{ProjectSession, SessionError, ToolpathConfig};
use crate::tool::MillingCutter;
use crate::tool_load::plunge_stress::safe_plunge_cap_mm_min;
use serde::{Deserialize, Serialize};

/// Specific stale-default patterns the validator can detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleDefaultRule {
    /// `DropCutter::min_z <= -49.999` — Roadmap B.1 changed the default
    /// from `-50.0` to stock bottom. Pre-B.1 projects keep `-50.0`.
    DropCutterMinZPreB1,
    /// Ball or tapered-ball tool with plunge_rate above the
    /// `150 × tip_diameter_mm` cap. Fix 2 (commit c5b9f74) added the
    /// cap to fresh LUT recommendations; pre-Fix-2 projects bypass it.
    TaperedBallPlungePreFix2,
    /// Wood-class material with flat tool on an Adaptive/Adaptive3d op,
    /// stepover below `0.15 × tool.diameter`. Fix 1 raised the wood
    /// adaptive `ae_factor` from 0.14 to 0.20 — pre-Fix-1 projects sit
    /// below.
    WoodAdaptiveStepoverPreFix1,
    /// ProjectCurve with `depth < 0`. The geometric convention is
    /// "positive depth = into material", so a negative value lifts the
    /// cutter into air and produces a 100 % air-cut toolpath. Almost
    /// always a user mistake — surface review 2026-05-20 caught one in
    /// the Wanaka project (TP3 "Rivers (back) (copy)"). Auto-fix flips
    /// the sign.
    ProjectCurveNegativeDepth,
}

impl StaleDefaultRule {
    pub fn id(self) -> &'static str {
        match self {
            Self::DropCutterMinZPreB1 => "drop_cutter_min_z_pre_b1",
            Self::TaperedBallPlungePreFix2 => "tapered_ball_plunge_pre_fix2",
            Self::WoodAdaptiveStepoverPreFix1 => "wood_adaptive_stepover_pre_fix1",
            Self::ProjectCurveNegativeDepth => "project_curve_negative_depth",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::DropCutterMinZPreB1 => "Drop-cutter min_z is at the pre-2026-04 default",
            Self::TaperedBallPlungePreFix2 => "Plunge rate exceeds the small-ball safety cap",
            Self::WoodAdaptiveStepoverPreFix1 => {
                "Adaptive stepover is narrower than the wood/rigidity target"
            }
            Self::ProjectCurveNegativeDepth => {
                "Project-curve depth is negative — toolpath will cut air"
            }
        }
    }
}

/// One detected stale default with its proposed fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDefault {
    pub rule_id: StaleDefaultRule,
    pub toolpath_id: usize,
    pub toolpath_name: String,
    /// Operator-facing one-line title.
    pub title: String,
    /// Operator-facing detail with current value + proposed new value.
    pub detail: String,
    /// Proposed new value (numeric — for UI preview and tests).
    pub new_value: f64,
    /// Current value before fix.
    pub current_value: f64,
}

/// Validate the session's toolpaths against the rule library. Returns one
/// `StaleDefault` per (toolpath, rule) match. Order is toolpath-then-rule.
pub fn validate_stale_defaults(session: &ProjectSession) -> Vec<StaleDefault> {
    let mut out = Vec::new();
    let stock = session.stock_config();
    let stock_bottom_z = compute_stock_bottom_z(session);
    for tc in session.toolpath_configs() {
        let tool = find_tool_for(session, tc.tool_id);
        out.extend(validate_one_toolpath(
            tc,
            tool,
            &stock.material,
            stock_bottom_z,
        ));
    }
    out
}

/// Validate a single toolpath against the rule library. Same rules as
/// [`validate_stale_defaults`] but takes the toolpath, tool, material,
/// and stock-bottom Z directly so callers without a `ProjectSession`
/// (e.g. the GUI toolpath-properties panel) can run the same checks
/// against in-flight edits.
pub fn validate_one_toolpath(
    tc: &ToolpathConfig,
    tool: Option<&ToolConfig>,
    material: &Material,
    stock_bottom_z: f64,
) -> Vec<StaleDefault> {
    let mut out = Vec::new();
    if let Some(d) = check_drop_cutter_min_z(tc, stock_bottom_z) {
        out.push(d);
    }
    if let Some(tool) = tool {
        if let Some(d) = check_tapered_ball_plunge(tc, tool) {
            out.push(d);
        }
        if let Some(d) = check_wood_adaptive_stepover(tc, tool, material) {
            out.push(d);
        }
    }
    if let Some(d) = check_project_curve_negative_depth(tc) {
        out.push(d);
    }
    out
}

/// Apply the auto-fix for one detected stale default. Returns
/// `Err(SessionError::ToolpathNotFound)` if the toolpath has been
/// removed since validation.
pub fn apply_stale_default_fix(
    session: &mut ProjectSession,
    defect: &StaleDefault,
) -> Result<(), SessionError> {
    let configs = session.toolpath_configs_mut();
    let tc = configs
        .iter_mut()
        .find(|tc| tc.id == defect.toolpath_id)
        .ok_or(SessionError::ToolpathNotFound(defect.toolpath_id))?;
    apply_stale_default_to_op(&mut tc.operation, defect);
    Ok(())
}

/// Apply the auto-fix's field change directly to an [`OperationConfig`].
/// Used by callers that hold the operation mutably without a session
/// (e.g. the GUI toolpath-properties panel, which builds a transient
/// `ToolpathEntry` and writes back to the session at the end of the
/// frame). Session-level callers should prefer [`apply_stale_default_fix`].
pub fn apply_stale_default_to_op(op: &mut OperationConfig, defect: &StaleDefault) {
    match defect.rule_id {
        StaleDefaultRule::DropCutterMinZPreB1 => {
            if let OperationConfig::DropCutter(cfg) = op {
                cfg.min_z = defect.new_value;
            }
        }
        StaleDefaultRule::TaperedBallPlungePreFix2 => {
            op.as_params_mut().set_plunge_rate(defect.new_value);
        }
        StaleDefaultRule::WoodAdaptiveStepoverPreFix1 => {
            op.as_params_mut().set_stepover(defect.new_value);
        }
        StaleDefaultRule::ProjectCurveNegativeDepth => {
            if let OperationConfig::ProjectCurve(cfg) = op {
                cfg.depth = defect.new_value;
            }
        }
    }
}

// ── Rule implementations ──────────────────────────────────────────────

fn check_drop_cutter_min_z(tc: &ToolpathConfig, stock_bottom_z: f64) -> Option<StaleDefault> {
    let OperationConfig::DropCutter(cfg) = &tc.operation else {
        return None;
    };
    // The pre-B.1 default was exactly -50.0; allow a tiny epsilon to
    // tolerate float round-trip through TOML/JSON.
    if cfg.min_z > -49.999 {
        return None;
    }
    Some(StaleDefault {
        rule_id: StaleDefaultRule::DropCutterMinZPreB1,
        toolpath_id: tc.id,
        toolpath_name: tc.name.clone(),
        title: StaleDefaultRule::DropCutterMinZPreB1.title().to_owned(),
        detail: format!(
            "Current `min_z` is {:.3} mm (pre-Roadmap-B.1 default). \
             Tightening to the stock bottom ({:.3} mm) makes a future \
             deeper model load visible.",
            cfg.min_z, stock_bottom_z
        ),
        new_value: stock_bottom_z,
        current_value: cfg.min_z,
    })
}

fn check_tapered_ball_plunge(tc: &ToolpathConfig, tool: &ToolConfig) -> Option<StaleDefault> {
    let tool_def = build_cutter(tool);
    let hint = tool_def.to_geometry_hint();
    if !matches!(
        hint,
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. }
    ) {
        return None;
    }
    let cap = safe_plunge_cap_mm_min(hint, tool_def.diameter())?;
    let plunge = tc.operation.plunge_rate();
    if plunge <= cap {
        return None;
    }
    Some(StaleDefault {
        rule_id: StaleDefaultRule::TaperedBallPlungePreFix2,
        toolpath_id: tc.id,
        toolpath_name: tc.name.clone(),
        title: StaleDefaultRule::TaperedBallPlungePreFix2
            .title()
            .to_owned(),
        detail: format!(
            "Tool is a small ball/tapered-ball; plunge rate {plunge:.0} mm/min exceeds \
             the {cap:.0} mm/min flute-tip safety cap. Pre-Fix-2 projects bypass the \
             tool-geometry-aware cap."
        ),
        new_value: cap,
        current_value: plunge,
    })
}

fn check_wood_adaptive_stepover(
    tc: &ToolpathConfig,
    tool: &ToolConfig,
    material: &Material,
) -> Option<StaleDefault> {
    if !is_wood_class(material) {
        return None;
    }
    let tool_def = build_cutter(tool);
    if !matches!(tool_def.to_geometry_hint(), ToolGeometryHint::Flat) {
        return None;
    }
    if !matches!(
        tc.operation,
        OperationConfig::Adaptive(_) | OperationConfig::Adaptive3d(_)
    ) {
        return None;
    }
    let stepover = tc.operation.as_params().stepover()?;
    let d = tool_def.diameter();
    let floor = 0.15 * d;
    if stepover >= floor {
        return None;
    }
    // Suggested new value matches the post-Fix-1 target: 0.20 × D
    // (machine.rigidity.adaptive_woc_factor is the runtime authority but
    // 0.20 is the typical fresh value; the auto-fix uses the machine factor).
    let new_value = 0.20 * d;
    Some(StaleDefault {
        rule_id: StaleDefaultRule::WoodAdaptiveStepoverPreFix1,
        toolpath_id: tc.id,
        toolpath_name: tc.name.clone(),
        title: StaleDefaultRule::WoodAdaptiveStepoverPreFix1
            .title()
            .to_owned(),
        detail: format!(
            "Adaptive stepover {stepover:.3} mm is below the 0.15×D wood/rigidity \
             floor ({floor:.3} mm). Fix 1 raised the wood adaptive ae_factor from \
             0.14 to 0.20 — bumping to {new_value:.3} mm matches the post-fix target."
        ),
        new_value,
        current_value: stepover,
    })
}

fn check_project_curve_negative_depth(tc: &ToolpathConfig) -> Option<StaleDefault> {
    let OperationConfig::ProjectCurve(cfg) = &tc.operation else {
        return None;
    };
    if cfg.depth >= 0.0 {
        return None;
    }
    let new_value = -cfg.depth;
    let direction_label = cfg.direction.label();
    Some(StaleDefault {
        rule_id: StaleDefaultRule::ProjectCurveNegativeDepth,
        toolpath_id: tc.id,
        toolpath_name: tc.name.clone(),
        title: StaleDefaultRule::ProjectCurveNegativeDepth
            .title()
            .to_owned(),
        detail: format!(
            "ProjectCurve `depth` is {:.3} mm (negative). The geometric convention is \
             \"positive depth = into material\" — with `direction = {direction_label}`, a \
             negative depth lifts the cutter into air for the whole toolpath (0 mm of \
             in-material cut). Flipping to {new_value:+.3} mm restores the intended \
             {new_value:.1} mm-deep cut.",
            cfg.depth,
        ),
        new_value,
        current_value: cfg.depth,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────

fn is_wood_class(m: &Material) -> bool {
    matches!(
        m,
        Material::SolidWood { .. } | Material::Plywood { .. } | Material::SheetGood { .. }
    )
}

fn compute_stock_bottom_z(session: &ProjectSession) -> f64 {
    // StockConfig::origin_z is the bottom in world frame (see project memory:
    // 2D ops cut at negative Z; StockConfig origin_z is negative so stock top
    // is at Z=0). For our defaulting purpose we want the world-frame stock
    // bottom = origin_z (which is min Z of the stock).
    session.stock_config().origin_z
}

fn find_tool_for(session: &ProjectSession, raw_id: usize) -> Option<&ToolConfig> {
    session.tools().iter().find(|t| t.id == ToolId(raw_id))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::field_reassign_with_default
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::StockSource;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::{
        Adaptive3dConfig, DropCutterConfig, PocketConfig, ProjectCurveConfig, ProjectCurveDirection,
    };
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::material::{Material, WoodSpecies};
    use crate::session::{ProjectSession, ToolpathConfig};

    fn make_tp(id: usize, name: &str, op: OperationConfig, tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id,
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
        }
    }

    fn flat_em(diameter: f64) -> ToolConfig {
        let mut t = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        t.diameter = diameter;
        t
    }

    fn tapered_ball(tip_d: f64) -> ToolConfig {
        let mut t = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        t.diameter = tip_d;
        t.shaft_diameter = (tip_d + 2.0).max(3.0);
        t.taper_half_angle = 7.0;
        t
    }

    fn wood_session() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        let mut stock = s.stock_config().clone();
        stock.material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        s.set_stock_config(stock);
        s
    }

    // ── DropCutter min_z rule ────────────────────────────────────────

    #[test]
    fn rule_fires_on_pre_b1_min_z_minus_50() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = DropCutterConfig::default();
        cfg.min_z = -50.0;
        s.add_toolpath(
            0,
            make_tp(0, "Old Finish", OperationConfig::DropCutter(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].rule_id, StaleDefaultRule::DropCutterMinZPreB1);
        assert_eq!(defects[0].toolpath_name, "Old Finish");
    }

    #[test]
    fn rule_silent_on_post_b1_min_z() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = DropCutterConfig::default();
        cfg.min_z = -20.0;
        s.add_toolpath(
            0,
            make_tp(0, "New Finish", OperationConfig::DropCutter(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(defects.is_empty(), "min_z = -20 must not fire B.1 rule");
    }

    // ── Tapered ball plunge rule ─────────────────────────────────────

    #[test]
    fn rule_fires_on_wanaka_tp7_pattern_750_on_1mm_tb() {
        let mut s = wood_session();
        s.add_tool(tapered_ball(1.0));
        let mut cfg = DropCutterConfig::default();
        cfg.plunge_rate = 750.0;
        cfg.min_z = -20.0; // disable B.1 rule
        s.add_toolpath(
            0,
            make_tp(0, "3D Finish 6", OperationConfig::DropCutter(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        assert_eq!(
            defects[0].rule_id,
            StaleDefaultRule::TaperedBallPlungePreFix2
        );
        // 1 mm TB cap = 150 mm/min
        assert!((defects[0].new_value - 150.0).abs() < 1e-6);
    }

    #[test]
    fn rule_silent_on_flat_em_at_750() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = DropCutterConfig::default();
        cfg.plunge_rate = 750.0;
        cfg.min_z = -20.0;
        s.add_toolpath(0, make_tp(0, "Flat", OperationConfig::DropCutter(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(defects.is_empty(), "flat EM should not fire plunge rule");
    }

    #[test]
    fn rule_silent_on_tapered_ball_at_cap() {
        let mut s = wood_session();
        s.add_tool(tapered_ball(1.0));
        let mut cfg = DropCutterConfig::default();
        cfg.plunge_rate = 150.0; // at cap
        cfg.min_z = -20.0;
        s.add_toolpath(0, make_tp(0, "TB", OperationConfig::DropCutter(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(defects.is_empty(), "150 mm/min on 1 mm TB is at cap");
    }

    // ── Wood adaptive stepover rule ──────────────────────────────────

    #[test]
    fn rule_fires_on_wanaka_tp1_pattern_narrow_stepover() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = Adaptive3dConfig::default();
        cfg.stepover = 0.7; // 0.117 D, well below 0.15 D floor (0.9)
        s.add_toolpath(
            0,
            make_tp(0, "Back Rough", OperationConfig::Adaptive3d(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        let stepover_defects: Vec<_> = defects
            .iter()
            .filter(|d| d.rule_id == StaleDefaultRule::WoodAdaptiveStepoverPreFix1)
            .collect();
        assert_eq!(stepover_defects.len(), 1, "got {:?}", defects);
        assert!((stepover_defects[0].new_value - 1.20).abs() < 1e-6);
    }

    #[test]
    fn rule_silent_on_wide_stepover() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = Adaptive3dConfig::default();
        cfg.stepover = 1.2; // 0.20 D, at target
        s.add_toolpath(
            0,
            make_tp(0, "Back Rough", OperationConfig::Adaptive3d(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(
            !defects
                .iter()
                .any(|d| d.rule_id == StaleDefaultRule::WoodAdaptiveStepoverPreFix1)
        );
    }

    #[test]
    fn rule_silent_on_metal_with_narrow_stepover() {
        let mut s = ProjectSession::new_empty();
        let mut stock = s.stock_config().clone();
        stock.material = Material::Custom {
            name: "Aluminium 6061".to_owned(),
            hardness_index: 3.0,
            kc: 2500.0,
        };
        s.set_stock_config(stock);
        s.add_tool(flat_em(6.0));
        let mut cfg = Adaptive3dConfig::default();
        cfg.stepover = 0.7;
        s.add_toolpath(0, make_tp(0, "Rough", OperationConfig::Adaptive3d(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(
            !defects
                .iter()
                .any(|d| d.rule_id == StaleDefaultRule::WoodAdaptiveStepoverPreFix1),
            "wood-adaptive rule must not fire on metal"
        );
    }

    #[test]
    fn rule_silent_on_pocket_op() {
        // Wood + flat tool + narrow stepover, but Pocket op (not Adaptive).
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = PocketConfig::default();
        cfg.stepover = 0.5;
        s.add_toolpath(0, make_tp(0, "Pocket", OperationConfig::Pocket(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(
            !defects
                .iter()
                .any(|d| d.rule_id == StaleDefaultRule::WoodAdaptiveStepoverPreFix1),
            "wood-adaptive rule must only fire on Adaptive/Adaptive3d"
        );
    }

    // ── Auto-fix application ─────────────────────────────────────────

    #[test]
    fn apply_fix_clears_drop_cutter_defect() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = DropCutterConfig::default();
        cfg.min_z = -50.0;
        let mut stock = s.stock_config().clone();
        stock.origin_z = -25.0;
        s.set_stock_config(stock);
        s.add_toolpath(
            0,
            make_tp(0, "Old Finish", OperationConfig::DropCutter(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        apply_stale_default_fix(&mut s, &defects[0]).unwrap();
        let after = validate_stale_defaults(&s);
        assert!(after.is_empty(), "auto-fix should clear the defect");
    }

    // ── Mixed-defect session ─────────────────────────────────────────

    #[test]
    fn mixed_wanaka_session_detects_all_three_rules() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0)); // tool 0: 6 mm EM
        s.add_tool(tapered_ball(1.0)); // tool 1: 1 mm TB

        // TP0: DropCutter with stale min_z + tapered-ball plunge defect.
        let mut tp0 = DropCutterConfig::default();
        tp0.min_z = -50.0;
        tp0.plunge_rate = 750.0;
        s.add_toolpath(
            0,
            make_tp(0, "3D Finish 6", OperationConfig::DropCutter(tp0), 1),
        )
        .unwrap();

        // TP1: Adaptive3d with narrow stepover (wood).
        let mut tp1 = Adaptive3dConfig::default();
        tp1.stepover = 0.7;
        s.add_toolpath(
            0,
            make_tp(1, "Back Rough", OperationConfig::Adaptive3d(tp1), 0),
        )
        .unwrap();

        let defects = validate_stale_defaults(&s);
        // Expect 3 defects: B.1 + Fix 2 on TP0, Fix 1 on TP1.
        assert_eq!(defects.len(), 3, "got {:?}", defects);
        let rule_ids: Vec<_> = defects.iter().map(|d| d.rule_id).collect();
        assert!(rule_ids.contains(&StaleDefaultRule::DropCutterMinZPreB1));
        assert!(rule_ids.contains(&StaleDefaultRule::TaperedBallPlungePreFix2));
        assert!(rule_ids.contains(&StaleDefaultRule::WoodAdaptiveStepoverPreFix1));
    }

    // ── ProjectCurve negative-depth rule ─────────────────────────────

    #[test]
    fn rule_fires_on_wanaka_tp3_pattern_negative_depth_from_below() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = ProjectCurveConfig::default();
        cfg.depth = -2.0;
        cfg.direction = ProjectCurveDirection::FromBelow;
        s.add_toolpath(
            0,
            make_tp(
                0,
                "Rivers (back) (copy)",
                OperationConfig::ProjectCurve(cfg),
                0,
            ),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        assert_eq!(
            defects[0].rule_id,
            StaleDefaultRule::ProjectCurveNegativeDepth
        );
        assert!((defects[0].current_value - (-2.0)).abs() < 1e-9);
        assert!((defects[0].new_value - 2.0).abs() < 1e-9);
        // The detail must surface the direction so the user knows which
        // sign convention applies.
        assert!(defects[0].detail.contains("From Below"));
    }

    #[test]
    fn rule_fires_on_negative_depth_from_above_too() {
        // The convention is "positive = into material" for both
        // directions; negative is always wrong.
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = ProjectCurveConfig::default();
        cfg.depth = -0.5;
        cfg.direction = ProjectCurveDirection::FromAbove;
        s.add_toolpath(0, make_tp(0, "PC", OperationConfig::ProjectCurve(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        assert!((defects[0].new_value - 0.5).abs() < 1e-9);
    }

    #[test]
    fn rule_silent_on_positive_depth() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = ProjectCurveConfig::default();
        cfg.depth = 2.0;
        s.add_toolpath(
            0,
            make_tp(0, "Good PC", OperationConfig::ProjectCurve(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(
            defects.is_empty(),
            "positive depth is the documented happy-path"
        );
    }

    #[test]
    fn rule_silent_on_zero_depth() {
        // Zero-depth is a legitimate "trace at surface" use case (e.g.
        // visual-only line, drag-knife style). Don't flag it.
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = ProjectCurveConfig::default();
        cfg.depth = 0.0;
        s.add_toolpath(
            0,
            make_tp(0, "Trace PC", OperationConfig::ProjectCurve(cfg), 0),
        )
        .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(defects.is_empty(), "depth=0 surface trace is allowed");
    }

    #[test]
    fn auto_fix_flips_negative_depth_sign() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = ProjectCurveConfig::default();
        cfg.depth = -3.5;
        s.add_toolpath(0, make_tp(0, "PC", OperationConfig::ProjectCurve(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert_eq!(defects.len(), 1);
        apply_stale_default_fix(&mut s, &defects[0]).unwrap();
        let OperationConfig::ProjectCurve(after) = &s.toolpath_configs()[0].operation else {
            panic!("operation type changed during auto-fix");
        };
        assert!((after.depth - 3.5).abs() < 1e-9);
        // Re-running the validator should now be clean.
        let after_defects = validate_stale_defaults(&s);
        assert!(after_defects.is_empty(), "auto-fix should clear the rule");
    }

    #[test]
    fn clean_session_produces_no_defects() {
        let mut s = wood_session();
        s.add_tool(flat_em(6.0));
        let mut cfg = Adaptive3dConfig::default();
        cfg.stepover = 1.2;
        cfg.plunge_rate = 500.0;
        s.add_toolpath(0, make_tp(0, "Clean", OperationConfig::Adaptive3d(cfg), 0))
            .unwrap();
        let defects = validate_stale_defaults(&s);
        assert!(defects.is_empty(), "clean session should be silent");
    }
}
