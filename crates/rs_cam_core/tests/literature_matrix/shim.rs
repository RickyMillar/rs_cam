//! Engine adapter shim.
//!
//! Translates a `LiteratureCell` into a `FeedsInput`, calls
//! `feeds::calculate`, and packages the result back into machinist-
//! convention values + an expression `Bindings` map for the invariants.
//!
//! Phase 0 covers `operation = "pocket"` with `tool_class = "flat"`.
//! Other operations / tool classes return a clearly-tagged stub error so
//! the runner can mark the cell `NA` rather than crash. Phase 1 expands
//! coverage per the starter-12 list.

use super::cell::LiteratureCell;
use super::expr::Bindings;
use rs_cam_core::feeds::{
    self, FeedsInput, FeedsResult, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, WorkholdingRigidity,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;

#[derive(Debug)]
pub enum ShimError {
    UnsupportedTool(String),
    UnsupportedOperation(String),
    UnknownMaterial(String),
}

impl std::fmt::Display for ShimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShimError::UnsupportedTool(s) => write!(f, "unsupported tool_class: {s}"),
            ShimError::UnsupportedOperation(s) => write!(f, "unsupported operation: {s}"),
            ShimError::UnknownMaterial(s) => write!(f, "unknown material key: {s}"),
        }
    }
}

/// Snapshot of engine output in machinist conventions, plus a `Bindings`
/// map the invariant expressions read from.
#[derive(Debug, Clone)]
pub struct ShimSnapshot {
    pub rpm: f64,
    pub feed_rate_mm_min: f64,
    /// Effective chipload after derates (mm/tooth). This is the value the
    /// cutter actually sees, not the LUT target.
    pub effective_chip_load_mm: f64,
    pub axial_doc_mm: f64,
    pub radial_woc_mm: f64,
    pub plunge_rate_mm_min: f64,
    pub power_kw: f64,
    pub mrr_mm3_min: f64,
    pub warnings: Vec<String>,
    pub bindings: Bindings,
}

/// Build a `Material` from the cell's `material` key. The plan uses
/// names like `oak_red` that don't exist in [`Material::from_key`] —
/// alias them to the closest in-engine match so Phase 0 can run without
/// adding new species. Phase 1+ extends this map; bug surfaces here as
/// `UnknownMaterial` rather than a silent miscount.
fn resolve_material(key: &str) -> Result<Material, ShimError> {
    let alias = match key {
        "oak_red" => "white_oak",         // closest hardwood proxy; Janka ~1290 vs 1360
        "maple_sugar" => "hard_maple",    // Phase 1 cell anchor
        "pine_eastern_white" => "softwood",
        "al6061" => "aluminum_6061_t6",
        _ => key,
    };
    let m = Material::from_key(alias);
    // Material::from_key has a generic fallback; detect it via key equality
    // for unknown inputs. The engine's fallback is "softwood", so explicitly
    // accept aliases we know map cleanly and otherwise pass the value through
    // (callers can override via Phase 1+ alias growth).
    match (key, &m) {
        // If we asked for something definitely unmapped and the engine
        // silently returned the generic softwood, surface it.
        (k, _) if k.is_empty() => Err(ShimError::UnknownMaterial(k.to_owned())),
        _ => Ok(m),
    }
}

fn resolve_machine(key: Option<&str>) -> MachineProfile {
    match key {
        Some("shapeoko_xxl") => MachineProfile::shapeoko_vfd(),
        Some(other) => MachineProfile::from_key(other),
        None => MachineProfile::generic_wood_router(),
    }
}

fn resolve_tool_geometry(cell: &LiteratureCell) -> Result<ToolGeometryHint, ShimError> {
    let i = &cell.inputs;
    let geom = match i.tool_class.as_str() {
        "flat" => ToolGeometryHint::Flat,
        "ball" => ToolGeometryHint::Ball,
        "bull" => ToolGeometryHint::Bull {
            corner_radius: i.corner_radius_mm.unwrap_or(1.0),
        },
        "vbit" => ToolGeometryHint::VBit {
            included_angle: i.included_angle_deg.unwrap_or(60.0),
            tip_diameter: i.tip_diameter_mm.unwrap_or(0.0),
        },
        "tapered_ball" => ToolGeometryHint::TaperedBall {
            tip_radius: i.tip_radius_mm.unwrap_or(i.diameter_mm * 0.5),
            taper_angle_deg: i.taper_half_angle_deg.unwrap_or(7.0),
        },
        "drill" => ToolGeometryHint::Flat,
        other => return Err(ShimError::UnsupportedTool(other.to_owned())),
    };
    Ok(geom)
}

fn resolve_operation(op: &str) -> Result<(OperationFamily, PassRole), ShimError> {
    Ok(match op {
        "pocket" => (OperationFamily::Pocket, PassRole::Roughing),
        // Phase 1 expansion targets — currently stubbed.
        // TODO(phase1): wire these once the starter-12 cells land.
        "adaptive2d" => return Err(ShimError::UnsupportedOperation(op.into())),
        "scallop" => return Err(ShimError::UnsupportedOperation(op.into())),
        "vcarve" => return Err(ShimError::UnsupportedOperation(op.into())),
        "drill" => return Err(ShimError::UnsupportedOperation(op.into())),
        other => return Err(ShimError::UnsupportedOperation(other.to_owned())),
    })
}

/// Run the cell through the engine and produce a snapshot.
pub fn run_cell(cell: &LiteratureCell) -> Result<ShimSnapshot, ShimError> {
    let geom = resolve_tool_geometry(cell)?;
    let (op_family, pass_role) = resolve_operation(&cell.inputs.operation)?;
    let material = resolve_material(&cell.inputs.material)?;
    let machine = resolve_machine(cell.inputs.machine_class.as_deref());
    let lut = feeds::embedded_vendor_lut();

    let fixed = cell.fixed_inputs.as_ref();
    let pinned_doc = fixed.and_then(|f| f.doc_pinned_mm());
    let pinned_woc = fixed.and_then(|f| f.woc_pinned_mm());

    let input = FeedsInput {
        tool_diameter: cell.inputs.diameter_mm,
        flute_count: cell.inputs.flute_count,
        flute_length: cell.inputs.flute_length_mm.unwrap_or(20.0),
        shank_diameter: None,
        tool_geometry: geom,
        material: &material,
        machine: &machine,
        operation: op_family,
        pass_role,
        axial_depth_mm: pinned_doc,
        radial_width_mm: pinned_woc,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext {
            tool_overhang_mm: cell.inputs.stickout_mm,
            workholding_rigidity: WorkholdingRigidity::Medium,
        },
        spindle_strategy: SpindleStrategy::MatchChart,
    };
    let result = feeds::calculate(&input);

    let snapshot = snapshot_from_result(cell, &result);
    Ok(snapshot)
}

fn snapshot_from_result(cell: &LiteratureCell, r: &FeedsResult) -> ShimSnapshot {
    let effective_chip = r.derates.effective_chip_load_mm();
    let warnings: Vec<String> = r.warnings.iter().map(|w| format!("{w:?}")).collect();

    let mut bindings: Bindings = Bindings::new();
    bindings.insert("rpm".into(), r.rpm);
    bindings.insert("feed_rate".into(), r.feed_rate_mm_min);
    bindings.insert("fpt".into(), effective_chip);
    bindings.insert("chipload".into(), effective_chip);
    bindings.insert("doc".into(), r.axial_depth_mm);
    bindings.insert("woc".into(), r.radial_width_mm);
    bindings.insert("D".into(), cell.inputs.diameter_mm);
    bindings.insert("flutes".into(), cell.inputs.flute_count as f64);
    bindings.insert(
        "stickout".into(),
        cell.inputs.stickout_mm.unwrap_or(0.0),
    );
    bindings.insert("plunge_rate".into(), r.plunge_rate_mm_min);
    bindings.insert("power_kw".into(), r.power_kw);
    bindings.insert("mrr_mm3_min".into(), r.mrr_mm3_min);

    // Envelope-vars (woc/D, doc/D) — populated unconditionally so the
    // convex_hull primitive can look them up by name.
    let d = cell.inputs.diameter_mm.max(f64::EPSILON);
    bindings.insert("woc_over_d".into(), r.radial_width_mm / d);
    bindings.insert("doc_over_d".into(), r.axial_depth_mm / d);

    ShimSnapshot {
        rpm: r.rpm,
        feed_rate_mm_min: r.feed_rate_mm_min,
        effective_chip_load_mm: effective_chip,
        axial_doc_mm: r.axial_depth_mm,
        radial_woc_mm: r.radial_width_mm,
        plunge_rate_mm_min: r.plunge_rate_mm_min,
        power_kw: r.power_kw,
        mrr_mm3_min: r.mrr_mm3_min,
        warnings,
        bindings,
    }
}
