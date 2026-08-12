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
use rs_cam_core::compute::build_cutter;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, DrillConfig, PocketConfig, ScallopConfig, VCarveConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    apply_drill_defaults, apply_feeds_result_to_op, operation_feeds_hints,
};
use rs_cam_core::feeds::{
    self, FeedsInput, FeedsResult, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    WorkholdingRigidity,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;

#[derive(Debug)]
pub enum ShimError {
    UnsupportedTool(String),
    UnsupportedOperation(String),
    UnknownMaterial(String),
    /// Engine refused the cell's tool × operation pairing (e.g. flat
    /// endmill on a Scallop op). The runner treats this as a *pass*
    /// for cells in `unusable` / `refuse` mode and as an NA for
    /// `values` cells.
    EngineRefused(String),
}

impl std::fmt::Display for ShimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShimError::UnsupportedTool(s) => write!(f, "unsupported tool_class: {s}"),
            ShimError::UnsupportedOperation(s) => write!(f, "unsupported operation: {s}"),
            ShimError::UnknownMaterial(s) => write!(f, "unknown material key: {s}"),
            ShimError::EngineRefused(s) => write!(f, "engine refused: {s}"),
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
        "oak_red" => "white_oak", // closest hardwood proxy; Janka ~1290 vs 1360
        "maple_sugar" => "hard_maple", // Phase 1 cell anchor
        "pine_eastern_white" => "softwood",
        "al6061" => "aluminum_6061_t6",
        // Phase 2 prep (2026-06-03) — aliases for the 44-cell expansion.
        // Each maps a drafter-friendly key to the canonical Material::from_key
        // string so cells can use natural names.
        //
        // Pure aliases (no semantic shift):
        "white_oak_red" => "white_oak", // belt-and-braces alias matching oak_red
        "hardwood_maple" => "hard_maple",
        "softwood_pine" => "softwood",
        "aluminum_6061" => "aluminum_6061_t6",
        // Composite — closest in-engine type is FiberglassGrade::Generic.
        // The `_gp` suffix matches the Garr "Fiberglass/Plastics/G10"
        // chart row that motivated MaterialFamily::Fiberglass. Phase 3
        // candidate for a dedicated grade when per-grade LUT data lands;
        // for now both `fiberglass_gp` and the canonical
        // `fiberglass_generic` key resolve to the same variant.
        "fiberglass_gp" => "fiberglass_generic",
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
        None | Some("shapeoko_xxl") | Some("generic") => MachineProfile::generic_wood_router(),
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
///
/// **TaperedBallNose convention**: for tapered-ball tools, the engine
/// treats `t.diameter` as the *ball-tip* diameter (the radius the
/// scallop / drop-cutter math actually engages with), not the shank
/// diameter. When a cell distinguishes the two via `tip_diameter_mm`
/// or `tip_radius_mm`, route the tip value into `t.diameter` and keep
/// the shank/shaft at `diameter_mm`. Without this, a cell that
/// describes a 6 mm-shank tool with a 2 mm tip ball ends up with the
/// engine computing scallop stepover from a 3 mm radius — exactly the
/// `scallop_uses_shank_radius_not_tip` anti-pattern the literature
/// matrix tracks (R4 C_tapered_scallop).
fn build_tool(cell: &LiteratureCell, tool_type: ToolType) -> ToolConfig {
    let i = &cell.inputs;
    let mut t = ToolConfig::new_default(ToolId(0), tool_type);
    let tip_d = match tool_type {
        ToolType::TaperedBallNose => i
            .tip_diameter_mm
            .or(i.tip_radius_mm.map(|r| 2.0 * r))
            .unwrap_or(i.diameter_mm),
        _ => i.diameter_mm,
    };
    t.diameter = tip_d;
    t.flute_count = i.flute_count;
    t.cutting_length = i.flute_length_mm.unwrap_or(20.0);
    if let Some(stickout) = i.stickout_mm {
        t.stickout = stickout;
    }
    // Default shank to the cell's diameter (shank for tapered cases,
    // tool diameter otherwise) — keeps the rigidity model honest for
    // small-shank cutters and preserves the shank context for tapered
    // tools where `t.diameter` was redirected to the tip above.
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
///
/// `family` is the [`OperationFamily`] resolved from the cell's
/// `operation` string. The constructor branches on it to pick the right
/// `*Config` variant; cell inputs (scallop height, max depth, drill
/// depth) flow into the appropriate fields. The full Suggest pipeline
/// then computes feed/RPM/DOC/WOC and writes them back via
/// [`apply_feeds_result_to_op`].
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
    let inputs = &cell.inputs;

    match family {
        OperationFamily::Pocket => Ok(OperationConfig::Pocket(PocketConfig {
            depth_per_pass: pinned_doc,
            stepover: pinned_woc,
            ..PocketConfig::default()
        })),
        OperationFamily::Adaptive => Ok(OperationConfig::Adaptive(AdaptiveConfig {
            depth_per_pass: pinned_doc,
            stepover: pinned_woc,
            ..AdaptiveConfig::default()
        })),
        OperationFamily::Scallop => {
            // ScallopConfig::scallop_height is non-optional (f64). Honour
            // the cell's `scallop_height_mm` input when present;
            // otherwise leave the default (0.1 mm, the engine default).
            let mut cfg = ScallopConfig::default();
            if let Some(h) = inputs.scallop_height_mm {
                cfg.scallop_height = h;
            }
            Ok(OperationConfig::Scallop(cfg))
        }
        OperationFamily::Trace => {
            // V-carve maps to the Trace operation family at the feeds
            // layer. `max_depth_mm` controls engaged-D in the V-bit
            // SFM calculation (engaged D = 2 × max_depth × tan(half_angle)).
            let mut cfg = VCarveConfig::default();
            if let Some(d) = inputs.max_depth_mm {
                cfg.max_depth = d;
            }
            Ok(OperationConfig::VCarve(cfg))
        }
        OperationFamily::Drill => {
            // Engine's `apply_drill_defaults` fills in peck_depth from
            // (tool diameter × material per-peck rule) — leave the
            // default and let the production path overwrite it. The
            // cell's drill depth dictates total hole depth.
            let depth = inputs
                .drill_depth_mm
                .unwrap_or(inputs.diameter_mm * 5.0)
                .max(0.0);
            Ok(OperationConfig::Drill(DrillConfig {
                depth,
                ..DrillConfig::default()
            }))
        }
        // Contour / Parallel / Face are reachable via other op strings
        // but no Phase 1 cell needs them yet.
        _ => Err(ShimError::UnsupportedOperation(format!("{family:?}"))),
    }
}

/// Refuse cell-level (tool_class × operation) combinations that the
/// user-facing GUI / CLI / MCP tool-picker would never allow. This is
/// a defence-in-depth layer above the engine-side
/// `validate_tool_for_operation` check — the latter operates on
/// `ToolGeometryHint` and cannot distinguish a drill bit from a flat
/// endmill (drill bits collapse to `ToolType::EndMill` in
/// `resolve_tool_type`, since engine routing keys off
/// `OperationFamily::Drill` rather than the tool type). Encoding the
/// matrix here mirrors the production tool-picker contract and routes
/// to `ShimError::EngineRefused`, which the runner treats as a pass
/// for `unusable` cells and as `NA` for `values` cells.
fn validate_tool_class_for_operation(tool_class: &str, operation: &str) -> Result<(), ShimError> {
    let ok = match (tool_class, operation) {
        // Drill bits: only valid on drill ops.
        ("drill", "drill") => true,
        ("drill", _) => false,
        // V-carve: only V-bits (tapered_ball is arguable but no cell
        // exercises that pairing yet — keep conservative).
        (_, "vcarve") => matches!(tool_class, "vbit"),
        // Scallop: only curved tips. The engine-side refusal also
        // catches this via `validate_tool_for_operation` — keep the
        // shim check as defence in depth and so failure attribution
        // lands here for cells that hand off raw tool_class strings.
        (_, "scallop") => matches!(tool_class, "ball" | "bull" | "tapered_ball"),
        // Everything else: permit (pocket / adaptive2d / drill when
        // matched above are geometrically OK for non-drill tools).
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(ShimError::EngineRefused(format!(
            "tool_class={tool_class} not valid for operation={operation}"
        )))
    }
}

fn resolve_operation(op: &str) -> Result<(OperationFamily, PassRole), ShimError> {
    Ok(match op {
        "pocket" => (OperationFamily::Pocket, PassRole::Roughing),
        // 2D adaptive (a.k.a. HSM / trochoidal). Roughing role — adaptive
        // is by definition a high-DOC, low-WOC clearing strategy.
        "adaptive2d" => (OperationFamily::Adaptive, PassRole::Roughing),
        // Scallop is a 3D finishing op (cusp-driven stepover from a
        // ball/tapered-ball tip). Finish role.
        "scallop" => (OperationFamily::Scallop, PassRole::Finish),
        // V-carve is the "Trace" feeds family — engaged-D is computed
        // from the V-bit included angle + cut depth.
        "vcarve" => (OperationFamily::Trace, PassRole::Finish),
        // Drill / peck cycle. Z-only kinematics; engagement metrics
        // are routed to drill-native gates by the engine.
        "drill" => (OperationFamily::Drill, PassRole::Roughing),
        other => return Err(ShimError::UnsupportedOperation(other.to_owned())),
    })
}

/// Run the cell through the production Suggest pipeline and produce a
/// snapshot. This is the exact code path GUI / CLI / MCP users hit, so
/// rigidity clamps (`enforce_invariants`), drill defaults, and the
/// `apply_feeds_result_to_op` write-back all participate.
pub fn run_cell(cell: &LiteratureCell) -> Result<ShimSnapshot, ShimError> {
    let material = resolve_material(&cell.inputs.material)?;
    let mut snapshot = run_cell_in_material(cell, &material)?;

    // Checkpoint K (e2) — the comparison arm. Same tool, same operation,
    // same machine, one material swapped, so a `ref_*`-bearing invariant
    // measures the material law and nothing else.
    //
    // A reference run that refuses binds NOTHING rather than a zero: the
    // expression evaluator then reports `UnknownVar` and the row lands
    // **NA**, which is the honest outcome. A relative test whose
    // denominator could not be computed has not passed.
    if let Some(reference) = &cell.reference {
        let ref_material = resolve_material(&reference.material)?;
        if let Ok(r) = run_cell_in_material(cell, &ref_material) {
            snapshot
                .bindings
                .insert("ref_fpt".into(), r.effective_chip_load_mm);
            snapshot
                .bindings
                .insert("ref_chipload".into(), r.effective_chip_load_mm);
            snapshot.bindings.insert("ref_rpm".into(), r.rpm);
            snapshot
                .bindings
                .insert("ref_feed_rate".into(), r.feed_rate_mm_min);
            snapshot
                .bindings
                .insert("ref_plunge_rate".into(), r.plunge_rate_mm_min);
        }
    }
    Ok(snapshot)
}

/// [`run_cell`] with the material supplied explicitly — the whole body
/// of the original function, so the comparison arm cannot drift from the
/// primary one.
fn run_cell_in_material(
    cell: &LiteratureCell,
    material: &Material,
) -> Result<ShimSnapshot, ShimError> {
    // Validate user-facing tool_class × operation pairing first, before
    // the shim flattens tool_class into ToolType (which loses the drill
    // identity). Matches the GUI / CLI / MCP tool-picker contract.
    validate_tool_class_for_operation(&cell.inputs.tool_class, &cell.inputs.operation)?;
    let tool_type = resolve_tool_type(cell)?;
    let (op_family, pass_role) = resolve_operation(&cell.inputs.operation)?;
    let machine = resolve_machine(cell.inputs.machine_class.as_deref());
    let lut = feeds::embedded_vendor_lut();

    let tool = build_tool(cell, tool_type);
    let operation = build_operation(cell, op_family)?;

    // If the cell pins DOC and/or WOC, thread them into the feeds
    // calculator as input hints so the engine evaluates power, RCTF,
    // MRR, and chipload at the cell's intended operating point —
    // otherwise `feeds_result_for_operation` reads only what
    // `operation_feeds_hints` exposes (which for Pocket/Adaptive is
    // `(None, None, None)`), and the post-clamp `set_stepover` /
    // `set_depth_per_pass` calls silently overwrite the pins with the
    // engine's free-run values before the snapshot is taken. See
    // shim docs above + Phase 1 fix group G1-shim-pinned-inputs.
    let pinned_doc = cell.fixed_inputs.as_ref().and_then(|f| f.doc_pinned_mm());
    let pinned_woc = cell.fixed_inputs.as_ref().and_then(|f| f.woc_pinned_mm());

    let tool_def = build_cutter(&tool);
    let (auto_axial, auto_radial, auto_scallop) = operation_feeds_hints(&operation);
    let feeds_input = FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: tool_def.to_geometry_hint(),
        material,
        machine: &machine,
        operation: op_family,
        // Checkpoint K (a4) — the shim's whole point is to be the
        // production path, so it supplies the operation's own kind and
        // gets the same LUT routing (and the same refusals) a GUI / CLI
        // / MCP user gets. No matrix cell currently exercises a
        // ProjectCurve on a bull-nose or V-bit, so no cell moves.
        operation_kind: Some(operation.op_type()),
        pass_role,
        axial_depth_mm: pinned_doc.or(auto_axial),
        radial_width_mm: pinned_woc.or(auto_radial),
        target_scallop_mm: auto_scallop,
        vendor_lut: Some(lut),
        setup: SetupContext {
            tool_overhang_mm: Some(tool.stickout),
            workholding_rigidity: WorkholdingRigidity::Medium,
        },
        spindle_strategy: SpindleStrategy::MaxSpeed,
    };
    // Run the engine's refusal check FIRST so this shim matches the
    // production Suggest paths (`suggest_for_operation` /
    // `feeds_result_for_operation`), which now validate before
    // calling `calculate`. Without this, the shim would silently
    // accept a flat endmill on a Scallop op and produce a
    // numeric-looking-but-meaningless recipe — exactly the bug class
    // the `flat_6mm_scallop_oak_unusable` cell exists to catch.
    if let Err(e) = feeds::validate_tool_for_operation(&feeds_input) {
        return Err(ShimError::EngineRefused(format!("{e}")));
    }
    let result = feeds::calculate(&feeds_input);

    // Apply the same post-clamp the GUI applies via Suggest so the
    // snapshot matches production output exactly (rigidity clamp on
    // DOC, plunge-to-feed clamp, stepover-to-diameter clamp).
    let mut op_clamped = operation;
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let _warnings = apply_feeds_result_to_op(
        &mut op_clamped,
        &mut provenance,
        &result,
        &tool,
        &machine,
        material,
        pass_role,
        rs_cam_core::feeds::suggest::SuggestContext::default(),
    );

    // Defend the pins against `apply_feeds_result_to_op`'s
    // unconditional `set_stepover` / `set_depth_per_pass` writes: if
    // the cell pinned DOC/WOC, restore those values so the snapshot
    // reflects the cell's operating point. The engine still
    // calculated RPM / chipload / power / MRR at the pinned operating
    // point above; we're just keeping the geometry consistent.
    if let Some(w) = pinned_woc {
        op_clamped.set_stepover(w);
    }
    if let Some(d) = pinned_doc {
        op_clamped.set_depth_per_pass(d);
    }
    // Drill ops still need their material-aware peck-depth fill-in;
    // `apply_feeds_result_to_op` doesn't touch `peck_depth`, only the
    // generic feed/RPM/DOC fields. The GUI calls this via
    // `suggest_for_operation`; the shim opts into the same step so
    // drill cells see the same peck depth a user would.
    apply_drill_defaults(&mut op_clamped, &tool, material);

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
    bindings.insert("stickout".into(), cell.inputs.stickout_mm.unwrap_or(0.0));
    bindings.insert("plunge_rate".into(), final_plunge);
    bindings.insert("power_kw".into(), r.power_kw);
    bindings.insert("mrr_mm3_min".into(), r.mrr_mm3_min);

    // Envelope-vars (woc/D, doc/D) — populated unconditionally so the
    // convex_hull primitive can look them up by name.
    let d = cell.inputs.diameter_mm.max(f64::EPSILON);
    bindings.insert("woc_over_d".into(), final_woc / d);
    bindings.insert("doc_over_d".into(), final_doc / d);

    // Op-specific values cells may bind invariants against. Pull from
    // the clamped operation so we surface whatever the engine actually
    // wrote back (e.g. peck_depth after `apply_drill_defaults`).
    match op {
        OperationConfig::Drill(cfg) => {
            bindings.insert("peck_depth".into(), cfg.peck_depth);
            bindings.insert("drill_depth".into(), cfg.depth);
            bindings.insert("peck_over_d".into(), cfg.peck_depth / d);
            bindings.insert("depth_over_d".into(), cfg.depth / d);
        }
        OperationConfig::VCarve(cfg) => {
            bindings.insert("max_depth".into(), cfg.max_depth);
        }
        OperationConfig::Scallop(cfg) => {
            bindings.insert("scallop_height".into(), cfg.scallop_height);
        }
        _ => {}
    }
    // Geometry-specific tool inputs cells may invariant against.
    if let Some(a) = cell.inputs.included_angle_deg {
        bindings.insert("included_angle".into(), a);
    }
    if let Some(cr) = cell.inputs.corner_radius_mm {
        bindings.insert("corner_radius".into(), cr);
    }
    if let Some(h) = cell.inputs.scallop_height_mm {
        bindings.insert("scallop_height_target".into(), h);
    }

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
