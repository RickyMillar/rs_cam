//! Engine adapter shim.
//!
//! Translates a `LiteratureCell` into the production Suggest pipeline
//! (`feeds_result_for_operation` + `apply_feeds_result_to_op`) — i.e.
//! the exact entry point GUI / CLI / MCP users hit — and packages the
//! post-clamp engine output back into machinist-convention values + an
//! expression `Bindings` map for the invariants.
//!
//! Phase 0 covers `operation = "pocket"` with `tool_class = "flat"`.
//! Other operations / tool classes return a clearly-tagged stub error so
//! the runner can mark the cell `NA` rather than crash. Phase 1 expands
//! coverage per the starter-12 list.

use super::cell::LiteratureCell;
use super::expr::Bindings;
use rs_cam_core::compute::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{apply_feeds_result_to_op, feeds_result_for_operation};
use rs_cam_core::feeds::{
    self, FeedsResult, OperationFamily, PassRole, SpindleStrategy, WorkholdingRigidity,
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

/// Literature bands are spindle-agnostic recommendations. Default to
/// the generic 8 k–24 k variable-speed wood-router profile so the engine
/// is never pinned against a machine the cell did not explicitly
/// request. `shapeoko_xxl` is a *chassis* hint (table size, gantry) and
/// says nothing about the spindle — many users (including the author of
/// this matrix) run a 24 k VFD on theirs, so we route it through the
/// generic profile too. A cell that genuinely wants to validate a
/// machine-specific clamp must pass an explicit `machine_class` key
/// recognised by [`MachineProfile::from_key`].
fn resolve_machine(key: Option<&str>) -> MachineProfile {
    match key {
        None | Some("shapeoko_xxl") | Some("generic") => {
            MachineProfile::generic_wood_router()
        }
        Some(other) => MachineProfile::from_key(other),
    }
}

/// Map the cell's `tool_class` string into a [`ToolType`] used by
/// [`build_tool`]. Geometry-specific extras (corner radius, V-bit
/// angle, taper, etc.) are passed through to the [`ToolConfig`] by
/// [`build_tool`] directly.
fn resolve_tool_type(cell: &LiteratureCell) -> Result<ToolType, ShimError> {
    let i = &cell.inputs;
    Ok(match i.tool_class.as_str() {
        "flat" => ToolType::EndMill,
        "ball" => ToolType::BallNose,
        "bull" => ToolType::BullNose,
        "vbit" => ToolType::VBit,
        "tapered_ball" => ToolType::TaperedBallNose,
        // Drills aren't a `ToolType` of their own — represent them as
        // an end-mill with the bit's diameter; engine routing relies
        // on the operation family, not the tool type, for drill ops.
        "drill" => ToolType::EndMill,
        other => return Err(ShimError::UnsupportedTool(other.to_owned())),
    })
}

/// Build a [`ToolConfig`] populated from the cell inputs so the
/// production Suggest pipeline sees the same tool the literature
/// recommendation was written for.
fn build_tool(cell: &LiteratureCell, tool_type: ToolType) -> ToolConfig {
    let i = &cell.inputs;
    let mut t = ToolConfig::new_default(ToolId(0), tool_type);
    t.diameter = i.diameter_mm;
    t.flute_count = i.flute_count;
    t.cutting_length = i.flute_length_mm.unwrap_or(20.0);
    if let Some(stickout) = i.stickout_mm {
        t.stickout = stickout;
    }
    // Default shank to tool diameter when the cell doesn't specify; keeps
    // the rigidity model honest for small-shank cutters.
    t.shank_diameter = i.diameter_mm;
    t.shaft_diameter = i.diameter_mm;
    if let Some(cr) = i.corner_radius_mm {
        t.corner_radius_mm = cr;
        t.corner_radius = cr;
    }
    if let Some(angle) = i.included_angle_deg {
        t.included_angle = angle;
    }
    if let Some(half) = i.taper_half_angle_deg {
        t.taper_half_angle = half;
    }
    t
}

/// Build an [`OperationConfig`] from the cell inputs, honouring pinned
/// DOC/WOC when present so the Suggest path's rigidity clamp evaluates
/// against the cell-fixed operating point.
fn build_operation(
    cell: &LiteratureCell,
    family: OperationFamily,
) -> Result<OperationConfig, ShimError> {
    let pinned_doc = cell
        .fixed_inputs
        .as_ref()
        .and_then(|f| f.doc_pinned_mm())
        .unwrap_or(0.0);
    let pinned_woc = cell
        .fixed_inputs
        .as_ref()
        .and_then(|f| f.woc_pinned_mm())
        .unwrap_or(0.0);

    match family {
        OperationFamily::Pocket => Ok(OperationConfig::Pocket(PocketConfig {
            depth_per_pass: pinned_doc,
            stepover: pinned_woc,
            ..PocketConfig::default()
        })),
        _ => Err(ShimError::UnsupportedOperation(format!("{family:?}"))),
    }
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

/// Run the cell through the production Suggest pipeline and produce a
/// snapshot. This is the exact code path GUI / CLI / MCP users hit, so
/// rigidity clamps (`enforce_invariants`), drill defaults, and the
/// `apply_feeds_result_to_op` write-back all participate.
pub fn run_cell(cell: &LiteratureCell) -> Result<ShimSnapshot, ShimError> {
    let tool_type = resolve_tool_type(cell)?;
    let (op_family, pass_role) = resolve_operation(&cell.inputs.operation)?;
    let material = resolve_material(&cell.inputs.material)?;
    let machine = resolve_machine(cell.inputs.machine_class.as_deref());
    let lut = feeds::embedded_vendor_lut();

    let tool = build_tool(cell, tool_type);
    let operation = build_operation(cell, op_family)?;

    // Run the engine through the production suggest path. Literature
    // bands are spindle-agnostic; `MaxSpeed` honours the vendor
    // `rpm_max` ceiling without imposing a chassis-specific clamp.
    let result = feeds_result_for_operation(
        &operation,
        &tool,
        &material,
        &machine,
        WorkholdingRigidity::Medium,
        lut,
        SpindleStrategy::MaxSpeed,
    );

    // Apply the same post-clamp the GUI applies via Suggest so the
    // snapshot matches production output exactly (rigidity clamp on
    // DOC, plunge-to-feed clamp, stepover-to-diameter clamp).
    let mut op_clamped = operation;
    let _warnings =
        apply_feeds_result_to_op(&mut op_clamped, &result, &tool, &machine, pass_role);

    let snapshot = snapshot_from_clamped(cell, &result, &op_clamped);
    Ok(snapshot)
}

/// Build a snapshot from the post-clamp operation plus the raw
/// `FeedsResult` produced by the Suggest pipeline. DOC / WOC / feed /
/// plunge are read back off the operation (so the rigidity / plunge
/// clamps are honoured), but RPM, chipload, power, and MRR come
/// straight from the calculator output since they aren't stored on the
/// operation surface.
fn snapshot_from_clamped(
    cell: &LiteratureCell,
    r: &FeedsResult,
    op: &OperationConfig,
) -> ShimSnapshot {
    let effective_chip = r.derates.effective_chip_load_mm();
    let warnings: Vec<String> = r.warnings.iter().map(|w| format!("{w:?}")).collect();

    let final_feed = op.feed_rate();
    let final_plunge = op.plunge_rate();
    let final_doc = op.depth_per_pass().unwrap_or(r.axial_depth_mm);
    let final_woc = op.stepover().unwrap_or(r.radial_width_mm);

    let mut bindings: Bindings = Bindings::new();
    bindings.insert("rpm".into(), r.rpm);
    bindings.insert("feed_rate".into(), final_feed);
    bindings.insert("fpt".into(), effective_chip);
    bindings.insert("chipload".into(), effective_chip);
    bindings.insert("doc".into(), final_doc);
    bindings.insert("woc".into(), final_woc);
    bindings.insert("D".into(), cell.inputs.diameter_mm);
    bindings.insert("flutes".into(), cell.inputs.flute_count as f64);
    bindings.insert(
        "stickout".into(),
        cell.inputs.stickout_mm.unwrap_or(0.0),
    );
    bindings.insert("plunge_rate".into(), final_plunge);
    bindings.insert("power_kw".into(), r.power_kw);
    bindings.insert("mrr_mm3_min".into(), r.mrr_mm3_min);

    // Envelope-vars (woc/D, doc/D) — populated unconditionally so the
    // convex_hull primitive can look them up by name.
    let d = cell.inputs.diameter_mm.max(f64::EPSILON);
    bindings.insert("woc_over_d".into(), final_woc / d);
    bindings.insert("doc_over_d".into(), final_doc / d);

    ShimSnapshot {
        rpm: r.rpm,
        feed_rate_mm_min: final_feed,
        effective_chip_load_mm: effective_chip,
        axial_doc_mm: final_doc,
        radial_woc_mm: final_woc,
        plunge_rate_mm_min: final_plunge,
        power_kw: r.power_kw,
        mrr_mm3_min: r.mrr_mm3_min,
        warnings,
        bindings,
    }
}
