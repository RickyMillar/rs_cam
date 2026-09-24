//! FM1 — the feeds matrix instrument.
//!
//! Programme: feeds matrix 2026-09-23, Phase 1
//! (`planning/feeds_matrix_2026-09-23/PLAN.md` §4 and §6).
//!
//! This file is an instrument, not a sentry. It walks every cell of the
//! matrix and RECORDS what the product doors already answer:
//!
//! - rows: `ToolType::ALL` at two diameters each;
//! - columns: `OperationType::ALL`;
//! - depth: the four wood material families.
//!
//! Per cell it calls `suggest_params`, the diagnostic adapters on the
//! suggested recipe, and `RigidityProfile::depth_cap_mm`. A second table
//! generates and simulates a small subset of cells and reads the
//! post-simulation load verdicts.
//!
//! The instrument adds no arithmetic, no threshold and no verdict. It
//! asserts only that the walk completed and that the files were written.
//!
//! Run it by name:
//!
//! ```text
//! scripts/cargo_lane.sh test -p rs_cam_core -q --test feeds_matrix_instrument_fm1 -- --ignored --nocapture
//! ```
//!
//! Outputs, under `planning/feeds_matrix_2026-09-23/`:
//! `matrix_2026-09-23.csv`, `matrix_2026-09-23_sim.csv`, `MATRIX_SUMMARY.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use common::meshes::height_field;
use common::session::{
    mesh_model, pinned_heights, polygon_model, single_op_session_with, square_polygon, stock_under,
};

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::Diagnostic;
use rs_cam_core::diagnostics::adapters::from_feeds::{
    diagnostic_from_suggest_warning, diagnostics_from_feeds_result,
    heuristic_hints_from_recommendation,
};
use rs_cam_core::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks;
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, suggest_params,
};
use rs_cam_core::feeds::vendor_lut::{MaterialFamily, VendorObservation};
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsSupport, SpindleStrategy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};
use rs_cam_core::session::SimulationOptions;
use rs_cam_core::tool_load::{
    ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
};

/// The date in every output name. Fixed, so a rerun on another day does
/// not fork the files.
const DATE: &str = "2026-09-23";

/// The planning folder of the programme, relative to the repository root.
const OUT_DIR: &str = "planning/feeds_matrix_2026-09-23";

/// The run command, printed into the summary.
const RUN_COMMAND: &str = "scripts/cargo_lane.sh test -p rs_cam_core -q --test \
                           feeds_matrix_instrument_fm1 -- --ignored --nocapture";

/// The wall-clock budget of the simulation subset. When the subset passes
/// it, the remaining cells are recorded as skipped, not dropped.
const SIM_BUDGET: Duration = Duration::from_secs(150);

/// Flute count for every tool. The V-bit LUT rows are mixed: the 13
/// single-flute rows are 15°-45° and 120° engraving and V-groove cutters;
/// the 60° rows (the angle `tool_of` sets) are two-flute.
const FLUTES: u32 = 2;

/// The common diameter of the simulation subset. It is the one diameter at
/// which every milling family except the V-bit has a wood row.
const SIM_DIAMETER: f64 = 6.0;

// ── Cell definition ─────────────────────────────────────────────────────

/// The four wood material families of the matrix, one material each.
fn wood_materials() -> [(&'static str, Material); 4] {
    [
        (
            "softwood",
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "hardwood",
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "mdf",
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
        (
            "plywood_hardwood",
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
        ),
    ]
}

/// The LUT material families that the four materials above resolve to.
/// Used only to filter the printed diameter table.
fn is_wood_family(f: MaterialFamily) -> bool {
    matches!(
        f,
        MaterialFamily::Softwood
            | MaterialFamily::Hardwood
            | MaterialFamily::Mdf
            | MaterialFamily::PlywoodHardwood
    )
}

/// The two diameters walked per tool type: `[small, common]`. The choice
/// and its reason per type are in [`diameter_reason`].
fn diameters(kind: ToolType) -> [f64; 2] {
    match kind {
        ToolType::EndMill | ToolType::BallNose | ToolType::BullNose | ToolType::TaperedBallNose => {
            [3.175, 6.0]
        }
        ToolType::VBit => [6.35, 12.7],
    }
}

fn diameter_reason(kind: ToolType) -> &'static str {
    match kind {
        ToolType::EndMill => {
            "flat_end wood rows carry 3.175 (16 rows) and 6.0 (8 rows); 6.0 is shared with the other families"
        }
        ToolType::BallNose => "ball_nose wood rows carry 3.175 (4 rows) and 6.0 (4 rows)",
        ToolType::BullNose => {
            "bull_nose wood rows carry only 6.0 (3 rows); 3.175 has NO bull_nose row and is the small size of the other families"
        }
        ToolType::VBit => {
            "chamfer_vbit 60° rows (the angle the tool uses) carry 6.35 and 12.7 (Whiteside 1540 / 1550)"
        }
        ToolType::TaperedBallNose => {
            "tapered_ball_nose wood rows carry 3.175 (4 rows) and 6.0 (4 rows) as the tip diameter"
        }
    }
}

/// The tool builder of `chipload_thinning_magnitude_survey.rs`, copied.
fn tool_of(kind: ToolType, diameter: f64, flutes: u32) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = flutes;
    t.cutting_length = (diameter * 3.0).max(12.0);
    t.shank_diameter = diameter.max(3.0);
    t.shaft_diameter = diameter.max(3.0);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = diameter * 0.15;
        t.corner_radius_mm = diameter * 0.15;
    }
    if matches!(kind, ToolType::TaperedBallNose) {
        // The shank must be larger than the tip: `tool/tapered_ball.rs`
        // panics when it is not.
        t.taper_half_angle = 7.0;
        t.shank_diameter = (diameter + 3.0).max(6.0);
        t.shaft_diameter = t.shank_diameter;
    }
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
    }
    t
}

fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    }
}

fn suggest(
    op: OperationType,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    stock: &StockContext,
) -> Result<SuggestedParams, rs_cam_core::feeds::FeedsError> {
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine,
        material,
        lut: &EMBEDDED_LUT,
        stock_ctx: stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
}

// ── CSV helpers (formatting only) ───────────────────────────────────────

/// Quote a field when it holds a comma, a quote or a line break.
fn q(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// A number with 4 decimals; an absent value is an empty field, not zero.
fn num(v: Option<f64>) -> String {
    v.map_or_else(String::new, |x| format!("{x:.4}"))
}

/// The variant name of a `Debug` string: the text before the first space,
/// brace or parenthesis.
fn variant(debug: &str) -> String {
    debug
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn row(fields: &[String]) -> String {
    fields.iter().map(|f| q(f)).collect::<Vec<_>>().join(",")
}

// ── Pre-simulation matrix ───────────────────────────────────────────────

/// One recorded cell of the pre-simulation matrix.
struct Cell {
    kind: ToolType,
    op: OperationType,
    support_arm: String,
    diagnostic_ids: Vec<String>,
    feeds_warnings: Vec<String>,
    suggest_warnings: Vec<String>,
}

const MATRIX_HEADER: &[&str] = &[
    "tool_type",
    "diameter_mm",
    "flutes",
    "operation",
    "material",
    "status",
    "refusal_text",
    "rpm",
    "feed_mm_min",
    "plunge_mm_min",
    "stepover_mm",
    "depth_per_pass_mm",
    "r_axial_depth_mm",
    "r_radial_width_mm",
    "r_chip_load_mm",
    "force_n",
    "power_kw",
    "chipload_bounds_min_mm",
    "chipload_bounds_max_mm",
    "chipload_point_mm",
    "support_arm",
    "support_detail",
    "lut_observation_id",
    "lut_row_pass_role",
    "lut_is_extrapolated",
    "lut_evidence_grade",
    "lut_row_kind",
    "lut_source_id",
    "diameter_ratio_raw",
    "claim_form",
    "claim_scale",
    "claim_range",
    "claim_residual",
    "hardness_ratio_raw",
    "hardness_scale",
    "hardness_basis",
    "chipload_source",
    "vendor_source",
    "diagnostic_ids",
    "diagnostic_severities",
    "feeds_warnings",
    "suggest_warnings",
    "cap_family",
    "cap_role",
    "cap_factor",
    "cap_mm",
];

fn support_columns(s: &FeedsSupport) -> (String, String) {
    match s {
        FeedsSupport::VendorBacked => ("VendorBacked".to_owned(), String::new()),
        FeedsSupport::Extrapolated { claim } => {
            let (headline, detail) = claim.card_text();
            ("Extrapolated".to_owned(), format!("{headline}; {detail}"))
        }
        FeedsSupport::FormulaOnly { source } => ("FormulaOnly".to_owned(), (*source).to_owned()),
        FeedsSupport::Refuse { reason } => ("Refuse".to_owned(), reason.to_string()),
    }
}

/// The five G1 claim columns of a matched row (extrapolation P1 step 3):
/// the raw diameter ratio, the form (A, B, C, or the basis name when there
/// is no claim), the scale, the range in mm, and the residual.
fn claim_columns(m: &rs_cam_core::feeds::vendor_lookup::LookupResult) -> [String; 5] {
    use rs_cam_core::feeds::extrapolation::ClaimResidual;
    let ratio = num(Some(m.chipload_diameter_ratio_raw));
    match m.size_basis.claim() {
        Some(c) => [
            ratio,
            c.form.name().to_owned(),
            num(Some(c.scale)),
            format!("{:.4}-{:.4}", c.range_mm.start(), c.range_mm.end()),
            match &c.residual {
                ClaimResidual::Bracket { lo_value, hi_value } => {
                    format!("bracket {lo_value:.4}-{hi_value:.4}")
                }
                ClaimResidual::FitRms { fraction } => format!("rms {fraction:.4}"),
                ClaimResidual::VendorSpread { lo, hi, family, .. } => {
                    format!("spread x{lo:.2}-{hi:.2} ({family:?})")
                }
            },
        ],
        None => [
            ratio,
            m.size_basis.name().to_owned(),
            String::new(),
            String::new(),
            String::new(),
        ],
    }
}

/// The three G2 hardness columns of a matched row (extrapolation P2 step
/// 4): the raw hardness ratio, the applied hardness scale, and the basis
/// name (`Unscaled`, `CompositeBoard`, `Law` or `Capped`). A capped row
/// keeps its raw ratio, so the two numbers differ from the law there.
fn hardness_columns(m: &rs_cam_core::feeds::vendor_lookup::LookupResult) -> [String; 3] {
    [
        num(Some(m.chipload_hardness_ratio_raw)),
        num(Some(m.chipload_hardness_scale)),
        m.hardness_basis.name().to_owned(),
    ]
}

/// The diagnostics every pre-simulation door raises on the suggested recipe.
fn recipe_diagnostics(s: &SuggestedParams, tool: &ToolConfig) -> Vec<Diagnostic> {
    let tp = ToolpathId(0);
    let r = &s.feeds_result;
    let mut out = diagnostics_from_static_checks(tp, &s.operation, tool, None);
    out.extend(diagnostics_from_feeds_result(tp, r));
    out.extend(heuristic_hints_from_recommendation(
        tp,
        s.operation.feed_rate(),
        s.operation.stepover(),
        s.operation.depth_per_pass(),
        r,
    ));
    // Ruling R4: the Suggest records that carry a rule id (the
    // aggressiveness record) are diagnostics too.
    out.extend(
        s.warnings
            .iter()
            .filter_map(|w| diagnostic_from_suggest_warning(tp, w)),
    );
    out
}

/// The cap the one producer gives this cell. Feeds matrix R2
/// (2026-09-23): the diameter is `depth_cap_diameter_mm` at the cell's
/// depth per pass, the function the Suggest clamp and the depth gate
/// call, and a finishing role prints empty cap columns (no cap).
fn cap_columns(machine: &MachineProfile, op: &OperationConfig, tool: &ToolConfig) -> [String; 4] {
    let (family, role) = op.feeds_style();
    let diameter = rs_cam_core::feeds::geometry::depth_cap_diameter_mm(
        &rs_cam_core::compute::cutter::build_cutter(tool),
        op.depth_per_pass().unwrap_or(tool.diameter),
    );
    let cap = machine.rigidity.depth_cap_mm(family, role, diameter);
    [
        format!("{family:?}"),
        format!("{role:?}"),
        num(cap.as_ref().map(|c| c.factor)),
        num(cap.as_ref().map(|c| c.cap_mm())),
    ]
}

fn walk_matrix(
    machine: &MachineProfile,
    rows_by_id: &HashMap<&str, &VendorObservation>,
) -> (String, Vec<Cell>) {
    let stock = stock_ctx();
    let mut csv = String::new();
    writeln!(csv, "{}", MATRIX_HEADER.join(",")).expect("write");
    let mut cells = Vec::new();

    for &kind in ToolType::ALL {
        for diameter in diameters(kind) {
            let tool = tool_of(kind, diameter, FLUTES);
            for &op in OperationType::ALL {
                for (mat_label, material) in &wood_materials() {
                    let head = vec![
                        format!("{kind:?}"),
                        num(Some(diameter)),
                        FLUTES.to_string(),
                        format!("{op:?}"),
                        (*mat_label).to_owned(),
                    ];
                    let mut fields = head;
                    match suggest(op, &tool, machine, material, &stock) {
                        Err(e) => {
                            fields.push("refused".to_owned());
                            fields.push(e.to_string());
                            // 13 recipe columns (incl. the force and power
                            // at the shipped point, and the A2 point), the
                            // support pair, 16 row
                            // columns (6 row, 5 G1 claim, 3 G2 hardness,
                            // chipload source, vendor source), 4 diagnostic /
                            // warning columns.
                            fields.extend(std::iter::repeat_n(String::new(), 13));
                            fields.push("Refused".to_owned());
                            fields.push(String::new());
                            fields.extend(std::iter::repeat_n(String::new(), 16));
                            fields.extend(std::iter::repeat_n(String::new(), 4));
                            fields.extend(cap_columns(
                                machine,
                                &OperationConfig::new_default(op),
                                &tool,
                            ));
                            cells.push(Cell {
                                kind,
                                op,
                                support_arm: "Refused".to_owned(),
                                diagnostic_ids: Vec::new(),
                                feeds_warnings: Vec::new(),
                                suggest_warnings: Vec::new(),
                            });
                        }
                        Ok(s) => {
                            let r = &s.feeds_result;
                            fields.push("ok".to_owned());
                            fields.push(String::new());
                            fields.push(num(Some(
                                s.operation.spindle_rpm().map_or(r.rpm, f64::from),
                            )));
                            fields.push(num(Some(s.operation.feed_rate())));
                            fields.push(num(Some(s.operation.plunge_rate())));
                            fields.push(num(s.operation.stepover()));
                            fields.push(num(s.operation.depth_per_pass()));
                            fields.push(num(Some(r.axial_depth_mm)));
                            fields.push(num(Some(r.radial_width_mm)));
                            fields.push(num(Some(r.chip_load_mm)));
                            // The predicted load at the SHIPPED point (R4 WP3
                            // baseline, 2026-09-24; corrected the same day: the
                            // first version read the calculator's inputs, which
                            // the dial does not move). The depth and width are
                            // the operation's shipped values (the calculator's
                            // when the operation holds none), the advance per
                            // tooth is the shipped feed over the shipped RPM and
                            // the flutes, so the column moves with the dial and
                            // the RPM follow-down. `None` where a model refuses.
                            let shipped_rpm = s.operation.spindle_rpm().map_or(r.rpm, f64::from);
                            let shipped_fz =
                                s.operation.feed_rate() / (shipped_rpm * f64::from(FLUTES));
                            let shipped_ap =
                                s.operation.depth_per_pass().unwrap_or(r.axial_depth_mm);
                            let shipped_ae = s.operation.stepover().unwrap_or(r.radial_width_mm);
                            let force_n = rs_cam_core::feeds::force::lateral_cutting_force(
                                material,
                                shipped_ap,
                                rs_cam_core::feeds::force::immersion_angle(
                                    shipped_ae,
                                    tool.diameter / 2.0,
                                ),
                                shipped_fz,
                            );
                            fields.push(num(force_n));
                            let power_kw = rs_cam_core::feeds::power_at_operating_point(
                                &s.operation,
                                &tool,
                                material,
                                machine,
                                None,
                            )
                            .ok()
                            .map(|p| p.required_kw);
                            fields.push(num(power_kw));
                            fields.push(num(r.chipload_bounds.map(|b| b.min_mm_per_tooth)));
                            fields.push(num(r.chipload_bounds.map(|b| b.max_mm_per_tooth)));
                            fields.push(num(r.chipload_point_mm));
                            let (arm, detail) = support_columns(&r.support);
                            fields.push(arm.clone());
                            fields.push(detail);
                            match r.matched_lut_row.as_ref() {
                                Some(m) => {
                                    fields.push(m.observation_id.clone());
                                    fields.push(format!("{:?}", m.row_pass_role));
                                    fields.push(m.is_extrapolated.to_string());
                                    match rows_by_id.get(m.observation_id.as_str()) {
                                        Some(o) => {
                                            fields.push(format!("{:?}", o.evidence_grade));
                                            fields.push(format!("{:?}", o.row_kind));
                                            fields.push(o.source_id.clone());
                                        }
                                        None => {
                                            fields.extend(std::iter::repeat_n(
                                                "n/a: id not in EMBEDDED_LUT".to_owned(),
                                                3,
                                            ));
                                        }
                                    }
                                    fields.extend(claim_columns(m));
                                    fields.extend(hardness_columns(m));
                                }
                                None => fields.extend(std::iter::repeat_n(String::new(), 14)),
                            }
                            fields.push(format!("{:?}", r.chipload_source));
                            fields.push(r.vendor_source.clone().unwrap_or_default());

                            let diags = recipe_diagnostics(&s, &tool);
                            let ids: Vec<String> =
                                diags.iter().map(|d| d.id.as_str().to_owned()).collect();
                            let sev: Vec<String> =
                                diags.iter().map(|d| format!("{:?}", d.severity)).collect();
                            let fw: Vec<String> = r
                                .warnings
                                .iter()
                                .map(|w| variant(&format!("{w:?}")))
                                .collect();
                            let sw: Vec<String> = s
                                .warnings
                                .iter()
                                .map(|w| variant(&format!("{w:?}")))
                                .collect();
                            fields.push(ids.join(";"));
                            fields.push(sev.join(";"));
                            fields.push(fw.join(";"));
                            fields.push(sw.join(";"));
                            fields.extend(cap_columns(machine, &s.operation, &tool));
                            cells.push(Cell {
                                kind,
                                op,
                                support_arm: arm,
                                diagnostic_ids: ids,
                                feeds_warnings: fw,
                                suggest_warnings: sw,
                            });
                        }
                    }
                    assert_eq!(fields.len(), MATRIX_HEADER.len(), "column count");
                    writeln!(csv, "{}", row(&fields)).expect("write");
                }
            }
        }
    }
    (csv, cells)
}

// ── Post-simulation subset ──────────────────────────────────────────────

const SIM_HEADER: &[&str] = &[
    "tool_type",
    "diameter_mm",
    "operation",
    "material",
    "fixture",
    "status",
    "error_text",
    "rpm",
    "feed_mm_min",
    "stepover_mm",
    "depth_per_pass_mm",
    "support_arm",
    "chipload_state",
    "chipload_detail",
    "power_state",
    "power_peak_kw",
    "power_available_kw",
    "power_detail",
    "deflection_state",
    "deflection_peak_mm",
    "deflection_detail",
    "depth_state",
    "depth_peak_mm",
    "depth_cap_mm",
    "depth_cap_factor",
    "depth_detail",
    "diagnostic_ids_after_sim",
    "diagnostic_severities_after_sim",
    "elapsed_s",
];

struct SimCell {
    op: OperationType,
    kind: ToolType,
    mat_label: &'static str,
    material: Material,
    three_d: bool,
}

fn sim_cells() -> Vec<SimCell> {
    let mut out = Vec::new();
    for op in [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
    ] {
        for kind in [ToolType::EndMill, ToolType::BullNose] {
            for (mat_label, material) in wood_materials() {
                out.push(SimCell {
                    op,
                    kind,
                    mat_label,
                    material,
                    three_d: false,
                });
            }
        }
    }
    for (op, kind) in [
        (OperationType::Waterline, ToolType::EndMill),
        (OperationType::DropCutter, ToolType::EndMill),
        (OperationType::Adaptive3d, ToolType::EndMill),
        (OperationType::Scallop, ToolType::BallNose),
        (OperationType::Scallop, ToolType::TaperedBallNose),
        (OperationType::DropCutter, ToolType::BallNose),
        (OperationType::DropCutter, ToolType::TaperedBallNose),
    ] {
        for (mat_label, material) in wood_materials() {
            if matches!(mat_label, "softwood" | "hardwood") {
                out.push(SimCell {
                    op,
                    kind,
                    mat_label,
                    material,
                    three_d: true,
                });
            }
        }
    }
    out
}

/// The 3D fixture: a dome, top at z = 0, on a flat base at z = -8. The
/// footprint (46 mm) is larger than the stock (44 mm), so no sample falls
/// off the mesh edge.
fn dome() -> rs_cam_core::mesh::TriangleMesh {
    height_field(23.0, 1.0, |x, y| {
        (8.0 - (x * x + y * y) / 32.0).max(0.0) - 8.0
    })
}

fn chipload_cols(v: &ChiploadVerdict) -> [String; 2] {
    let detail = match v {
        ChiploadVerdict::Within { .. } => String::new(),
        ChiploadVerdict::Exceeds { side, .. } => format!("side={side:?}"),
        ChiploadVerdict::Unmodeled { reason } => variant(&format!("{reason:?}")),
    };
    [format!("{:?}", v.state()), detail]
}

fn power_cols(v: &PowerVerdict) -> [String; 4] {
    match v {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            ..
        }
        | PowerVerdict::Exceeds {
            peak_kw,
            available_kw,
            ..
        } => [
            format!("{:?}", v.state()),
            num(Some(*peak_kw)),
            num(Some(*available_kw)),
            String::new(),
        ],
        PowerVerdict::Unmodeled { reason } => [
            format!("{:?}", v.state()),
            String::new(),
            String::new(),
            variant(&format!("{reason:?}")),
        ],
    }
}

fn deflection_cols(v: &DeflectionVerdict) -> [String; 3] {
    match v {
        DeflectionVerdict::Within { peak_mm, .. } | DeflectionVerdict::Exceeds { peak_mm, .. } => [
            format!("{:?}", v.state()),
            num(Some(*peak_mm)),
            String::new(),
        ],
        DeflectionVerdict::Unmodeled { reason } => [
            format!("{:?}", v.state()),
            String::new(),
            variant(&format!("{reason:?}")),
        ],
    }
}

fn depth_cols(v: &DepthVerdict) -> [String; 5] {
    match v {
        DepthVerdict::Within { peak_mm, bound, .. }
        | DepthVerdict::Exceeds { peak_mm, bound, .. } => [
            format!("{:?}", v.state()),
            num(Some(*peak_mm)),
            num(Some(bound.cap_mm())),
            num(Some(bound.factor)),
            String::new(),
        ],
        // Feeds matrix R2 (2026-09-23): a finishing pass has a peak and
        // no cap.
        DepthVerdict::Reported { peak_mm, .. } => [
            format!("{:?}", v.state()),
            num(Some(*peak_mm)),
            String::new(),
            String::new(),
            "Reported".to_owned(),
        ],
        DepthVerdict::Unmodeled { reason } => [
            format!("{:?}", v.state()),
            String::new(),
            String::new(),
            String::new(),
            variant(&format!("{reason:?}")),
        ],
    }
}

fn verdict_cols(v: Option<&ToolpathLoadVerdict>) -> Vec<String> {
    match v {
        Some(v) => {
            let mut out = Vec::new();
            out.extend(chipload_cols(&v.chipload));
            out.extend(power_cols(&v.power));
            out.extend(deflection_cols(&v.deflection));
            out.extend(depth_cols(&v.depth));
            out
        }
        None => std::iter::repeat_n("n/a: no verdict row".to_owned(), 14).collect(),
    }
}

/// The result of one simulated cell: the fields after `status`, or an error.
fn run_sim_cell(cell: &SimCell, machine: &MachineProfile) -> Result<Vec<String>, String> {
    let tool = tool_of(cell.kind, SIM_DIAMETER, FLUTES);
    let s = suggest(cell.op, &tool, machine, &cell.material, &stock_ctx())
        .map_err(|e| format!("suggest refused: {e}"))?;
    let stock = StockConfig {
        material: cell.material.clone(),
        ..stock_under(20.0, 18.0)
    };
    let model = if cell.three_d {
        mesh_model(dome(), "fm1_dome")
    } else {
        polygon_model(vec![square_polygon(20.0)], "fm1_square")
    };
    let three_d = cell.three_d;
    let provenance = s.provenance.clone();
    let mut session = single_op_session_with(
        stock,
        tool,
        model,
        &format!("{:?}", cell.op),
        s.operation.clone(),
        |cfg| {
            cfg.feeds_provenance = provenance;
            if three_d {
                cfg.heights = pinned_heights(0.0, -8.0);
            }
        },
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| format!("generate: {e}"))?;
    session
        .run_simulation(
            &SimulationOptions {
                resolution: 1.0,
                metrics_enabled: true,
                auto_resolution: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .map_err(|e| format!("simulate: {e}"))?;
    let report = session.tool_load_report();
    let diags = session
        .diagnose_toolpath(0)
        .map_err(|e| format!("diagnose: {e}"))?;

    let r = &s.feeds_result;
    let mut out = vec![
        num(Some(s.operation.spindle_rpm().map_or(r.rpm, f64::from))),
        num(Some(s.operation.feed_rate())),
        num(s.operation.stepover()),
        num(s.operation.depth_per_pass()),
        support_columns(&r.support).0,
    ];
    out.extend(verdict_cols(report.per_toolpath.first()));
    out.push(
        diags
            .iter()
            .map(|d| d.id.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(";"),
    );
    out.push(
        diags
            .iter()
            .map(|d| format!("{:?}", d.severity))
            .collect::<Vec<_>>()
            .join(";"),
    );
    Ok(out)
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "panic with a non-text payload".to_owned())
}

struct SimOutcome {
    csv: String,
    ran: usize,
    errors: Vec<String>,
    skipped: usize,
    elapsed: Duration,
    ids: BTreeMap<String, usize>,
}

fn walk_sim(machine: &MachineProfile) -> SimOutcome {
    let mut csv = String::new();
    writeln!(csv, "{}", SIM_HEADER.join(",")).expect("write");
    let started = Instant::now();
    let mut ran = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();

    for cell in sim_cells() {
        let mut fields = vec![
            format!("{:?}", cell.kind),
            num(Some(SIM_DIAMETER)),
            format!("{:?}", cell.op),
            cell.mat_label.to_owned(),
            if cell.three_d {
                "dome_46mm_on_base_z-8".to_owned()
            } else {
                "square_40mm_polygon".to_owned()
            },
        ];
        let label = format!("{:?} / {:?} / {}", cell.op, cell.kind, cell.mat_label);
        if started.elapsed() > SIM_BUDGET {
            fields.push("skipped".to_owned());
            fields.push("time budget of the subset passed".to_owned());
            fields.extend(std::iter::repeat_n(String::new(), SIM_HEADER.len() - 7));
            skipped += 1;
            writeln!(csv, "{}", row(&fields)).expect("write");
            continue;
        }
        let t0 = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| run_sim_cell(&cell, machine)))
            .unwrap_or_else(|p| Err(format!("panic: {}", panic_text(p.as_ref()))));
        let dt = t0.elapsed();
        match result {
            Ok(rest) => {
                fields.push("ok".to_owned());
                fields.push(String::new());
                if let Some(joined) = rest.get(rest.len().saturating_sub(2)) {
                    for id in joined.split(';').filter(|s| !s.is_empty()) {
                        *ids.entry(id.to_owned()).or_default() += 1;
                    }
                }
                fields.extend(rest);
                ran += 1;
            }
            Err(e) => {
                fields.push("error".to_owned());
                fields.push(e.clone());
                fields.extend(std::iter::repeat_n(String::new(), SIM_HEADER.len() - 8));
                errors.push(format!("{label}: {e}"));
            }
        }
        fields.push(format!("{:.1}", dt.as_secs_f64()));
        assert_eq!(fields.len(), SIM_HEADER.len(), "sim column count");
        writeln!(csv, "{}", row(&fields)).expect("write");
        println!(
            "  sim {label}: {:.1} s (total {:.1} s)",
            dt.as_secs_f64(),
            started.elapsed().as_secs_f64()
        );
    }
    SimOutcome {
        csv,
        ran,
        errors,
        skipped,
        elapsed: started.elapsed(),
        ids,
    }
}

// ── Summary ─────────────────────────────────────────────────────────────

fn lut_diameter_table() -> String {
    let mut by_family: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut flutes: BTreeMap<String, BTreeMap<u32, usize>> = BTreeMap::new();
    for o in EMBEDDED_LUT
        .observations
        .iter()
        .filter(|o| is_wood_family(o.material_family))
    {
        let fam = format!("{:?}", o.tool_family);
        let d = o
            .diameter_mm
            .map_or_else(|| "none".to_owned(), |d| format!("{d:.4}"));
        *by_family
            .entry(fam.clone())
            .or_default()
            .entry(d)
            .or_default() += 1;
        *flutes
            .entry(fam)
            .or_default()
            .entry(o.flute_count)
            .or_default() += 1;
    }
    let mut out = String::new();
    writeln!(
        out,
        "| LUT tool family | wood-row diameters (mm: rows) | flute counts (flutes: rows) |"
    )
    .expect("write");
    writeln!(out, "|---|---|---|").expect("write");
    for (fam, ds) in &by_family {
        let d: Vec<String> = ds.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        let f: Vec<String> = flutes[fam]
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        writeln!(out, "| {fam} | {} | {} |", d.join(", "), f.join(", ")).expect("write");
    }
    out
}

fn summary(machine: &MachineProfile, cells: &[Cell], sim: &SimOutcome) -> String {
    let mut md = String::new();
    let w = &mut md;
    writeln!(w, "# Feeds matrix {DATE}: summary (FM1 instrument)").expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "The test `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs` writes this file. \
         It records what the product doors answer. It adds no arithmetic and no verdict."
    )
    .expect("write");
    writeln!(w).expect("write");
    writeln!(w, "Run command:").expect("write");
    writeln!(w).expect("write");
    writeln!(w, "```text\n{RUN_COMMAND}\n```").expect("write");
    writeln!(w).expect("write");

    writeln!(w, "## Walk").expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "- Machine: `MachineProfile::default()` = `{}`. The repository holds no measured \
         rigidity or power profile. The presets `shapeoko_vfd` and `shapeoko_makita` and the \
         kinematics preset `shapeoko_xxl_ricky_tuned` exist; the instrument does not walk them.",
        machine.name
    )
    .expect("write");
    writeln!(
        w,
        "- `SpindleStrategy::default()`, `SuggestContext::default()`, \
         stock context top 0, bottom -18, height 18, padding 2."
    )
    .expect("write");
    writeln!(
        w,
        "- Cells: {} tool types × 2 diameters × {} operations × 4 materials = {} cells. \
         Flutes: {FLUTES} on every tool.",
        ToolType::ALL.len(),
        OperationType::ALL.len(),
        cells.len()
    )
    .expect("write");
    writeln!(w).expect("write");
    writeln!(w, "Diameters walked (small, common):").expect("write");
    writeln!(w).expect("write");
    for &kind in ToolType::ALL {
        let [a, b] = diameters(kind);
        writeln!(
            w,
            "- `{kind:?}`: {a} and {b} mm. {}.",
            diameter_reason(kind)
        )
        .expect("write");
    }
    writeln!(
        w,
        "- V-bit flutes: 2. The LUT V-bit rows are mixed (13 single-flute, 12 two-flute); \
         the single-flute rows are 15°-45° and 120° cutters, and the 60° rows are two-flute."
    )
    .expect("write");
    writeln!(w).expect("write");
    writeln!(w, "Wood rows in `EMBEDDED_LUT`, read by the instrument:").expect("write");
    writeln!(w).expect("write");
    w.push_str(&lut_diameter_table());
    writeln!(w).expect("write");

    // Cell classes.
    writeln!(w, "## Cell classes per tool type").expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "The support arm is one axis: `Refused` (Suggest returned an error), `VendorBacked`, \
         `Extrapolated` (a G1 size claim), `FormulaOnly`, `Refuse` (the `FeedsSupport` arm). `fires-a-diagnostic` is a separate \
         flag: the cell has one or more diagnostic ids from the three pre-simulation doors. \
         A vendor-backed cell can also fire. The `FeedsWarning` and `SuggestWarning` counts \
         are separate columns."
    )
    .expect("write");
    writeln!(w).expect("write");
    let classes = [
        "Refused",
        "VendorBacked",
        "Extrapolated",
        "FormulaOnly",
        "Refuse",
    ];
    write!(w, "| class |").expect("write");
    for &kind in ToolType::ALL {
        write!(w, " {kind:?} |").expect("write");
    }
    writeln!(w, " all |").expect("write");
    write!(w, "|---|").expect("write");
    for _ in 0..=ToolType::ALL.len() {
        write!(w, "---|").expect("write");
    }
    writeln!(w).expect("write");
    let count_line = |w: &mut String, name: &str, pred: &dyn Fn(&Cell) -> bool| {
        write!(w, "| {name} |").expect("write");
        for &kind in ToolType::ALL {
            let n = cells.iter().filter(|c| c.kind == kind && pred(c)).count();
            write!(w, " {n} |").expect("write");
        }
        let n = cells.iter().filter(|c| pred(c)).count();
        writeln!(w, " {n} |").expect("write");
    };
    for class in classes {
        count_line(w, class, &|c: &Cell| c.support_arm == class);
    }
    count_line(w, "fires-a-diagnostic", &|c: &Cell| {
        !c.diagnostic_ids.is_empty()
    });
    count_line(w, "has a FeedsWarning", &|c: &Cell| {
        !c.feeds_warnings.is_empty()
    });
    count_line(w, "has a SuggestWarning", &|c: &Cell| {
        !c.suggest_warnings.is_empty()
    });
    count_line(w, "total", &|_| true);
    writeln!(w).expect("write");

    // Per operation.
    writeln!(
        w,
        "## Cell classes per operation (all tools, diameters and materials)"
    )
    .expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "| operation | Refused | VendorBacked | Extrapolated | FormulaOnly | Refuse | \
         fires-a-diagnostic |"
    )
    .expect("write");
    writeln!(w, "|---|---|---|---|---|---|---|").expect("write");
    for &op in OperationType::ALL {
        let of = |class: &str| {
            cells
                .iter()
                .filter(|c| c.op == op && c.support_arm == class)
                .count()
        };
        let fires = cells
            .iter()
            .filter(|c| c.op == op && !c.diagnostic_ids.is_empty())
            .count();
        writeln!(
            w,
            "| {op:?} | {} | {} | {} | {} | {} | {fires} |",
            of("Refused"),
            of("VendorBacked"),
            of("Extrapolated"),
            of("FormulaOnly"),
            of("Refuse")
        )
        .expect("write");
    }
    writeln!(w).expect("write");

    // Distinct ids.
    let tally = |f: &dyn Fn(&Cell) -> &Vec<String>| {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for c in cells {
            // Count a cell once per distinct id.
            let distinct: BTreeSet<&String> = f(c).iter().collect();
            for id in distinct {
                *m.entry(id.clone()).or_default() += 1;
            }
        }
        m
    };
    let write_tally = |w: &mut String, title: &str, m: &BTreeMap<String, usize>| {
        writeln!(w, "## {title}").expect("write");
        writeln!(w).expect("write");
        if m.is_empty() {
            writeln!(w, "None.").expect("write");
        } else {
            writeln!(w, "| id | cells |").expect("write");
            writeln!(w, "|---|---|").expect("write");
            for (id, n) in m {
                writeln!(w, "| `{id}` | {n} |").expect("write");
            }
        }
        writeln!(w).expect("write");
    };
    write_tally(
        w,
        "Pre-simulation diagnostic ids (cells that fire each id)",
        &tally(&|c: &Cell| &c.diagnostic_ids),
    );
    write_tally(
        w,
        "FeedsWarning variants (cells that carry each)",
        &tally(&|c: &Cell| &c.feeds_warnings),
    );
    write_tally(
        w,
        "SuggestWarning variants (cells that carry each)",
        &tally(&|c: &Cell| &c.suggest_warnings),
    );

    // Simulation subset.
    writeln!(w, "## Simulation subset (`matrix_{DATE}_sim.csv`)").expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "- 2D: Pocket, Profile, Adaptive on a 40 mm square polygon, stock 44 × 44 × 18 below \
         z = 0; end_mill and bull_nose at {SIM_DIAMETER} mm; the four materials."
    )
    .expect("write");
    writeln!(
        w,
        "- 3D: a dome height field (top z = 0, flat base z = -8, 46 mm footprint) in the same \
         stock; heights pinned to top 0 and bottom -8 on all 3D cells (`bottom_z: Auto` \
         collapses a waterline band). Waterline, DropCutter, Adaptive3d with end_mill; Scallop \
         and DropCutter with ball_nose and tapered_ball_nose; {SIM_DIAMETER} mm; softwood and \
         hardwood."
    )
    .expect("write");
    writeln!(
        w,
        "- Simulation: resolution 1.0, metrics on, auto resolution off, other fields from \
         `SimulationOptions::default()`. That default has `adaptive_feed_modulation: true`, so \
         the post-simulation verdicts read the modulated feed, as the GUI default does."
    )
    .expect("write");
    writeln!(
        w,
        "- Cells run: {}; errors: {}; skipped on the {} s budget: {}; wall-clock of the subset: {:.1} s.",
        sim.ran,
        sim.errors.len(),
        SIM_BUDGET.as_secs(),
        sim.skipped,
        sim.elapsed.as_secs_f64()
    )
    .expect("write");
    writeln!(w).expect("write");
    if !sim.errors.is_empty() {
        writeln!(w, "Cells recorded as error:").expect("write");
        writeln!(w).expect("write");
        for e in &sim.errors {
            writeln!(w, "- {e}").expect("write");
        }
        writeln!(w).expect("write");
    }
    write_tally(
        w,
        "Post-simulation diagnostic ids (cells that fire each id)",
        &sim.ids,
    );

    writeln!(w, "## Doors that could not answer").expect("write");
    writeln!(w).expect("write");
    writeln!(
        w,
        "- `lut_evidence_grade`, `lut_row_kind`, `lut_source_id` are not on `FeedsResult`. \
         The instrument joins `matched_lut_row.observation_id` to `EMBEDDED_LUT.observations`. \
         A cell whose id is absent from the LUT records `n/a`."
    )
    .expect("write");
    writeln!(
        w,
        "- A refused cell has no recipe, so its recipe, row and diagnostic columns are empty; \
         its cap columns come from the registry default of the operation."
    )
    .expect("write");
    md
}

// ── The instrument ──────────────────────────────────────────────────────

#[test]
#[ignore = "FM1 instrument: writes planning/feeds_matrix_2026-09-23/matrix_<date>.csv; run with -- --ignored --nocapture"]
fn feeds_matrix_instrument_fm1() {
    let machine = MachineProfile::default();
    let rows_by_id: HashMap<&str, &VendorObservation> = EMBEDDED_LUT
        .observations
        .iter()
        .map(|o| (o.observation_id.as_str(), o))
        .collect();
    let out_dir = common::repo_root().join(OUT_DIR);
    std::fs::create_dir_all(&out_dir).expect("create the output folder");

    let t0 = Instant::now();
    let (matrix_csv, cells) = walk_matrix(&machine, &rows_by_id);
    let expected = ToolType::ALL.len() * 2 * OperationType::ALL.len() * 4;
    assert_eq!(cells.len(), expected, "the walk did not visit every cell");
    let matrix_path = out_dir.join(format!("matrix_{DATE}.csv"));
    // Write the matrix before the simulation subset starts, so a failure
    // there cannot lose it.
    std::fs::write(&matrix_path, &matrix_csv).expect("write the matrix CSV");
    println!(
        "FM1: {} cells walked in {:.1} s -> {}",
        cells.len(),
        t0.elapsed().as_secs_f64(),
        matrix_path.display()
    );

    let sim = walk_sim(&machine);
    let sim_path = out_dir.join(format!("matrix_{DATE}_sim.csv"));
    std::fs::write(&sim_path, &sim.csv).expect("write the simulation CSV");
    println!(
        "FM1: simulation subset {} run, {} error, {} skipped in {:.1} s -> {}",
        sim.ran,
        sim.errors.len(),
        sim.skipped,
        sim.elapsed.as_secs_f64(),
        sim_path.display()
    );

    let md = summary(&machine, &cells, &sim);
    let md_path = out_dir.join("MATRIX_SUMMARY.md");
    std::fs::write(&md_path, &md).expect("write the summary");
    println!("FM1: summary -> {}", md_path.display());
    println!("{md}");

    for p in [&matrix_path, &sim_path, &md_path] {
        let len = std::fs::metadata(p).expect("output exists").len();
        assert!(len > 0, "{} is empty", p.display());
    }
}
