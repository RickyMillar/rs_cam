//! TIER-TRIAL-350 — the tiered finishing trial on the x3.5 terrain board.
//!
//! Pre-registration: `planning/tier_trial_2026-10-01/PLAN.md`. This file is
//! the instrument that plan names. It runs ONE arm per process. The arm
//! comes from the environment variable `TIER_TRIAL_ARM` (A1..A6, T1..T9, C1,
//! C2, F1, and the phase-2 arms P1..P3). The run writes `planning/tier_trial_2026-10-01/runs/<ARM>.json`
//! and the residual maps next to it.
//!
//! Run (release build):
//! `TIER_TRIAL_ARM=A1 cargo test --release -p rs_cam_core --features test-support
//!  --test tier_trial_350 -- --ignored --nocapture`
//!
//! Other environment dials:
//! - `TIER_TRIAL_CELL` — the window cell in mm (default 0.05). A value other
//!   than 0.05 adds `_cell<value>` to every output name.
//! - `TIER_TRIAL_CELL_CHECK` — a window index 0..15. The run then also
//!   simulates that window at 0.025 mm and records the p99 difference.
//! - `TIER_TRIAL_TAG` — a suffix for every output name (for example `rep`
//!   for the determinism repeat).
//! - `TIER_TRIAL_PENCIL_PASSES` — P1 only: the CAP on offset passes per
//!   side of each rest-depth pencil centreline (default 3). It is a cap, not
//!   a count: the rest-depth detector narrows it per chain to the passes
//!   that fit the local valley half-width. The half-width of the band where
//!   an R2 ball leaves more than 0.03 mm over an R1 ball depends on the
//!   valley's flank angle, so no single value follows from the geometry.
//!   The default is a judgement call; the value used is in the JSON notes.
//!
//! # Phase-2 arms
//!
//! - P1: op 2 re-dialled to R2 iso h 0.03 (whole board), then a Pencil op on
//!   the R1 (tool 6), detector `rest_depth`, `reference_tool_id` = the R2.
//! - P2: T3 with `MultitoolPlanSpec::coarse_skips_fine_islands = true`.
//! - P3: T3's planner chain with the planner's tier-0 op DISABLED and op 2
//!   re-dialled to R2 iso h 0.03 in its place. The R1 cleanup is the
//!   planner's tier-1 op: iso Scallop h 0.03, boundary
//!   `PlannedTierRegions { [R2, R1], tier 1, tolerance 0.15 }`.
//!
//! # What the run does
//!
//! 1. It loads `planning/fixtures/rivmap100/rivmap100_memory_repro.toml`.
//!    It copies the R2.0 taper (tool 12) from the tiered donor fixture, as
//!    `rivmap350_tiered_finish_step0_preview` does. Arms with H1 add a
//!    hypothetical 6.35 mm ball nose (see [`h1_tool`]).
//! 2. It sets up the arm's finish chain through `ProjectSession::apply`.
//!    The tier arms call `plan_multitool_finishing`.
//! 3. Every enabled operation takes the Suggest feeds by the route of the
//!    CLI's `--apply-suggest` (`apply_suggested_feeds_to_session`,
//!    `crates/rs_cam_cli/src/project.rs`). The arm's own geometry dials
//!    (cusp height, stepover) are put back after Suggest, as the planner's
//!    `restore_planned_geometry` does. An operation with no feasible feed
//!    makes the arm REFUSED.
//! 4. It walks core's generation plan at a 0.25 mm cell, as the CLI's
//!    `run_generation_plan` does, then runs one closing whole-board
//!    simulation with the machine kinematics of the fixture and the default
//!    adaptive feed modulation. The per-toolpath time is
//!    `toolpath_runtimes[..].breakdown` of that run.
//! 5. It drops the session. Then it simulates 16 windows of 25 x 25 mm at the
//!    window cell with every toolpath of the arm, in order.
//!
//! Population: the columns inside the model silhouette and at least 3 mm in
//! from its edge. The silhouette is `model_silhouette` at its default 0.5 mm
//! cell, reduced to the outer loop by `silhouette_machining_outline`. The
//! 3 mm inset is `offset_polygon(outline, 3.0)` (positive is inward).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{ArcFitParams, BoundarySource, DressupConfig, StockSource};
use rs_cam_core::compute::operation_configs::PencilConfig;
use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::compute::simulate::{
    ColumnDeviation, SimGroupEntry, SimToolpathEntry, SimulationRequest,
    contributes_simulated_motion, entry_metrics_not_applicable, entry_tool_fields,
    hash_operation_config, locate_global_move, run_simulation,
};
use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
};
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::finish::pencil::PencilDetector;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::geometry::boundary::{model_silhouette, silhouette_machining_outline};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::session::generation_plan::{self, Scope, Step};
use rs_cam_core::session::multitool::tier_arc_tolerance_mm;
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, Command, MultitoolPlanSpec, ProjectSession,
    ReplaceToolpathConfigArgs, SetDressupConfigArgs, SetDressupFieldArgs,
    SetSimulationResolutionArgs, SetToolpathEnabledArgs, SetToolpathParamArgs, SetToolpathToolArgs,
    SimulationOptions, SimulationResolution, TierStrategy, equal_cusp_stepover_mm,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType};
use serde_json::{Value, json};

// ── Fixed dials of the plan ─────────────────────────────────────────────

/// The whole-board simulation cell (PLAN "Whole board at 0.25 mm").
const BOARD_CELL_MM: f64 = 0.25;
/// The default window cell (PLAN "Fine windows ... at a 0.05 mm cell").
const DEFAULT_WINDOW_CELL_MM: f64 = 0.05;
/// The cell of the cell check (PLAN "Instrument").
const CHECK_CELL_MM: f64 = 0.025;
/// The window side (PLAN "16 windows of 25 x 25 mm").
const WINDOW_MM: f64 = 25.0;
/// The lattice span: centres at 350·(k+½)/4 (PLAN "Fine windows").
const LATTICE_SPAN_MM: f64 = 350.0;
/// The population inset from the silhouette edge (PLAN "Population").
const INSET_MM: f64 = 3.0;
/// The reference cusp of A1 and the planner cusp of every tier arm.
const CUSP_MM: f64 = 0.03;
/// The fixture's operation ids.
const FACE_ID: usize = 7;
const ROUGH_ID: usize = 1;
const SCALLOP_ID: usize = 2;
/// The fixture's R1.0 taper and the donor's R2.0 taper.
const R1_TOOL: usize = 6;
const DONOR_R2_TOOL: usize = 12;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ── The arms ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    R1,
    R2,
    H1,
}

#[derive(Clone, Debug)]
enum Arm {
    /// One whole-board Scallop op (the fixture's op 2), re-dialled.
    Whole {
        tool: Tool,
        cusp: f64,
        iso_field: bool,
    },
    /// One whole-board Parallel raster (`DropCutter`) at the equal-cusp
    /// stepover for R1 / h.
    Raster {
        cusp: f64,
        /// The raster stepover holds `cusp` up to this slope (degrees): XY
        /// stepover = equal-cusp stepover x cos(slope). 0 = the flat value.
        design_slope_deg: f64,
        /// Some(θ): the raster runs on slopes 0..θ and an R1 iso scallop
        /// (h `cusp`) on θ..90.
        split_deg: Option<f64>,
    },
    /// R2 iso h `semi` then R1 iso h `cusp`, both whole board.
    Semi { semi: f64, cusp: f64 },
    /// The tier planner.
    Tier {
        tools: Vec<Tool>,
        tolerance: f64,
        strategy: Option<TierStrategy>,
        /// Cusp for tier 0 (the coarsest) set after planning, if any.
        tier0_cusp: Option<f64>,
        /// `MultitoolPlanSpec::coarse_skips_fine_islands` (P2).
        coarse_skips_fine_islands: bool,
    },
    /// P1: op 2 = R2 iso h `coarse_cusp` whole board, then an R1 Pencil op
    /// (rest-depth detector, reference tool = the R2).
    CoarsePencil { coarse_cusp: f64 },
    /// P3: the [R2, R1] planner chain at `tolerance`, IsoScallop, cusp
    /// `CUSP_MM`; the planner's tier-0 op is disabled and op 2 (R2 iso h
    /// `coarse_cusp`, whole board) runs in its place.
    CoarseTierCleanup { coarse_cusp: f64, tolerance: f64 },
}

fn arm_of(name: &str) -> Arm {
    use Tool::{H1, R1, R2};
    let whole = |tool, cusp| Arm::Whole {
        tool,
        cusp,
        iso_field: true,
    };
    let tier = |tools: Vec<Tool>, tolerance, strategy, tier0_cusp| Arm::Tier {
        tools,
        tolerance,
        strategy,
        tier0_cusp,
        coarse_skips_fine_islands: false,
    };
    let iso = Some(TierStrategy::IsoScallop);
    match name {
        "A1" => whole(R1, 0.03),
        "A2" => whole(R1, 0.02),
        "A3" => whole(R1, 0.05),
        "A4" => whole(R1, 0.08),
        "A5" => whole(R2, 0.03),
        "A6" => whole(R2, 0.02),
        "T1" => tier(vec![R2, R1], 0.05, iso, None),
        "T2" => tier(vec![R2, R1], 0.10, iso, None),
        "T3" => tier(vec![R2, R1], 0.15, iso, None),
        "T4" => tier(vec![R2, R1], 0.25, iso, None),
        "T5" => tier(vec![R2, R1], 0.15, None, None),
        "T6" => tier(vec![H1, R2, R1], 0.10, iso, None),
        "T7" => tier(vec![H1, R2, R1], 0.15, iso, None),
        "T8" => tier(vec![H1, R1], 0.15, iso, None),
        "T9" => tier(vec![R2, R1], 0.15, iso, Some(0.05)),
        "C1" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 0.0,
            split_deg: None,
        },
        // Raster family (added 2026-10-02 after C1 ran 3.1 h).
        "R45" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 45.0,
            split_deg: None,
        },
        "R60" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 60.0,
            split_deg: None,
        },
        "R70" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 70.0,
            split_deg: None,
        },
        "RS30" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 30.0,
            split_deg: Some(30.0),
        },
        "RS45" => Arm::Raster {
            cusp: 0.03,
            design_slope_deg: 45.0,
            split_deg: Some(45.0),
        },
        "C2" => Arm::Whole {
            tool: R1,
            cusp: 0.03,
            iso_field: false,
        },
        "F1" => Arm::Semi {
            semi: 0.08,
            cusp: 0.03,
        },
        // Blob arms: the big ball takes the flats, the R1 one big region
        // (run with TIER_TRIAL_CLOSE_MM / TIER_TRIAL_H1_DIAM).
        "B1" => Arm::Tier {
            tools: vec![H1, R1],
            tolerance: 0.15,
            strategy: iso,
            tier0_cusp: None,
            coarse_skips_fine_islands: true,
        },
        "P1" => Arm::CoarsePencil { coarse_cusp: 0.03 },
        "P2" => Arm::Tier {
            tools: vec![R2, R1],
            tolerance: 0.15,
            strategy: iso,
            tier0_cusp: None,
            coarse_skips_fine_islands: true,
        },
        "P3" => Arm::CoarseTierCleanup {
            coarse_cusp: 0.03,
            tolerance: 0.15,
        },
        other => panic!("TIER_TRIAL_ARM={other} is not an arm of PLAN.md"),
    }
}

fn arm_uses_h1(arm: &Arm) -> bool {
    matches!(arm, Arm::Tier { tools, .. } if tools.contains(&Tool::H1))
        || matches!(arm, Arm::Whole { tool: Tool::H1, .. })
}

/// H1: a HYPOTHETICAL 6.35 mm ball nose. The geometry is copied from the
/// repo's own ball-nose definition, tool 12 "6mm Ball Nose 2F" in
/// `crates/rs_cam_core/tests/fixtures/wanaka100/wanaka_full_tuned.toml`,
/// with the diameter and both shank fields set to 6.35 mm. Nothing else
/// changes. The operator's library is not known to hold this tool.
/// The "blob" dials (operator idea, 2026-10-02: a big ball takes the flats
/// and the sea, the R1 takes the whole range as one big region). Env
/// `TIER_TRIAL_CLOSE_MM` sets the tier islands' close radius and
/// `TIER_TRIAL_MIN_AREA_MM2` their minimum area; both unset = planner
/// defaults.
fn blob_islands(mut spec: MultitoolPlanSpec, notes: &mut Vec<String>) -> MultitoolPlanSpec {
    let num = |k: &str| {
        std::env::var(k).ok().map(|v| {
            v.parse::<f64>()
                .unwrap_or_else(|_| panic!("{k} is a number"))
        })
    };
    if let Some(r) = num("TIER_TRIAL_CLOSE_MM") {
        spec.islands.close_radius_mm = Some(r);
        notes.push(format!("tier islands close radius {r} mm"));
    }
    if let Some(a) = num("TIER_TRIAL_MIN_AREA_MM2") {
        spec.islands.min_region_area_mm2 = Some(a);
        notes.push(format!("tier islands min area {a} mm2"));
    }
    spec
}

/// The H1 diameter: env `TIER_TRIAL_H1_DIAM` (6.0 = the R3 ball), default
/// 6.35.
fn h1_diameter() -> f64 {
    std::env::var("TIER_TRIAL_H1_DIAM")
        .ok()
        .map_or(6.35, |v| v.parse().expect("TIER_TRIAL_H1_DIAM is a number"))
}

fn h1_tool() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
    let d = h1_diameter();
    t.name = format!("H1 {d}mm Ball Nose 2F (HYPOTHETICAL)");
    t.diameter = d;
    t.cutting_length = 25.0;
    t.helix_deg = 30.0;
    t.corner_radius_mm = 0.0;
    t.corner_radius = 0.0;
    t.included_angle = 90.0;
    t.taper_half_angle = 15.0;
    t.shaft_diameter = 6.35;
    t.holder_diameter = 25.0;
    t.shank_diameter = 6.35;
    t.shank_length = 20.0;
    t.stickout = 40.0;
    t.flute_count = 2;
    t.tool_number = 2;
    t.tool_material = ToolMaterial::Carbide;
    t.cut_direction = BitCutDirection::UpCut;
    t.vendor = String::new();
    t.product_id = String::new();
    t
}

// ── Session helpers ─────────────────────────────────────────────────────

fn index_of(session: &ProjectSession, id: usize) -> usize {
    session
        .find_toolpath_config_by_id(rs_cam_core::ToolpathId(id))
        .unwrap_or_else(|| panic!("toolpath id {id} is in the project"))
        .0
}

fn add_tool(session: &mut ProjectSession, tool: ToolConfig) -> usize {
    let created = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        }))
        .expect("add a tool")
        .created
        .expect("add_tool reports the new index");
    session.tools()[created].id.0
}

fn set_param(session: &mut ProjectSession, index: usize, param: &str, value: Value) {
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value,
        }))
        .unwrap_or_else(|e| panic!("set {param} on toolpath {index}: {e}"));
}

/// Give every enabled finish op that the multitool planner did not emit the
/// planner's own dressup policy (`plan_tier_dressups`, multitool.rs): the
/// op type's `DressupConfig::for_op`, arc tolerance = cusp / 2. The arms of
/// both families then share one dressup policy (PLAN.md amendment 1).
fn planner_dressups(session: &mut ProjectSession) -> String {
    let mut done = Vec::new();
    for index in 0..session.toolpath_configs().len() {
        let tc = &session.toolpath_configs()[index];
        if !tc.enabled || tc.id.0 == FACE_ID || tc.id.0 == ROUGH_ID || tc.planner_origin.is_some() {
            continue;
        }
        let cusp = match &tc.operation {
            OperationConfig::Scallop(s) => s.scallop_height,
            OperationConfig::DropCutter(d) => d.scallop_height.unwrap_or(CUSP_MM),
            // The pencil has no cusp dial; it cleans to the arm's cusp.
            OperationConfig::Pencil(_) => CUSP_MM,
            _ => CUSP_MM,
        };
        let mut dressups = DressupConfig::for_op(tc.operation.op_type());
        dressups.arc_fitting = dressups
            .arc_fitting
            .and_then(|_| tier_arc_tolerance_mm(cusp).map(|tolerance| ArcFitParams { tolerance }));
        let id = tc.id.0;
        let _ = session
            .apply(Command::SetDressupConfig(SetDressupConfigArgs {
                index,
                dressups: Box::new(dressups),
            }))
            .expect("set dressup config");
        done.push((id, cusp));
    }
    format!("planner dressup policy (for_op, arc = cusp/2) on finish ops (id, cusp) {done:?}")
}

/// Set the arc-fit dressup of every enabled finish op: `off`, or on with the
/// given tolerance (mm). Returns the note for the JSON.
fn override_arc_fitting(session: &mut ProjectSession, arc: &str) -> String {
    let finish: Vec<usize> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(_, tc)| tc.enabled && tc.id.0 != FACE_ID && tc.id.0 != ROUGH_ID)
        .map(|(i, _)| i)
        .collect();
    let field = |session: &mut ProjectSession, index: usize, key: &str, value: Value| {
        let _ = session
            .apply(Command::SetDressupField(SetDressupFieldArgs {
                index,
                key: key.to_owned(),
                value,
            }))
            .expect("set dressup field");
    };
    for &index in &finish {
        if arc == "off" {
            field(session, index, "arc_fitting", json!(false));
        } else {
            let tol: f64 = arc.parse().expect("TIER_TRIAL_ARC is `off` or a number");
            field(session, index, "arc_fitting", json!(true));
            field(session, index, "arc_tolerance", json!(tol));
        }
    }
    format!("arc fitting override `{arc}` on finish op indices {finish:?}")
}

fn set_enabled(session: &mut ProjectSession, index: usize, enabled: bool) {
    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index,
            enabled,
        }))
        .expect("set enabled");
}

fn set_tool(session: &mut ProjectSession, index: usize, tool_id: usize) {
    let _ = session
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id,
        }))
        .expect("set tool");
}

fn scallop_of(session: &ProjectSession, index: usize) -> (f64, bool) {
    match &session.toolpath_configs()[index].operation {
        OperationConfig::Scallop(s) => (s.scallop_height, s.iso_field),
        other => panic!("toolpath {index} is not a scallop: {:?}", other.op_type()),
    }
}

/// Re-dial a Scallop op through `SetToolpathParam`, then read it back.
fn dial_scallop(session: &mut ProjectSession, index: usize, cusp: f64, iso_field: bool) {
    set_param(session, index, "scallop_height", json!(cusp));
    set_param(session, index, "iso_field", json!(iso_field));
    let (h, iso) = scallop_of(session, index);
    assert!(
        (h - cusp).abs() < 1e-12 && iso == iso_field,
        "toolpath {index}: the dial did not take (h {h}, iso {iso})"
    );
}

/// The geometry dials an arm owns, snapshot before Suggest and put back
/// after it. A copy of the planner's private `restore_planned_geometry`
/// (`src/session/multitool.rs`) plus the raster stepover of C1.
fn restore_geometry(operation: &mut OperationConfig, planned: &OperationConfig) -> Vec<String> {
    let mut changed = Vec::new();
    let mut note = |field: &str, a: f64, b: f64| {
        if (a - b).abs() > 1e-12 {
            changed.push(format!("{field}: suggest {a} -> arm {b}"));
        }
    };
    match (&mut *operation, planned) {
        (OperationConfig::UnifiedFinish(out), OperationConfig::UnifiedFinish(want)) => {
            note("scallop_height", out.scallop_height, want.scallop_height);
            note("raster_stepover", out.raster_stepover, want.raster_stepover);
            note("stock_to_leave", out.stock_to_leave, want.stock_to_leave);
            out.scallop_height = want.scallop_height;
            out.raster_stepover = want.raster_stepover;
            out.stock_to_leave = want.stock_to_leave;
            out.monotone_cell_decomposition = want.monotone_cell_decomposition;
        }
        (OperationConfig::Scallop(out), OperationConfig::Scallop(want)) => {
            note("scallop_height", out.scallop_height, want.scallop_height);
            note("stock_to_leave", out.stock_to_leave, want.stock_to_leave);
            out.scallop_height = want.scallop_height;
            out.stock_to_leave = want.stock_to_leave;
            out.continuous = want.continuous;
            out.intra_pass_hookup_mm = want.intra_pass_hookup_mm;
            out.iso_field = want.iso_field;
            out.slope_from = want.slope_from;
            out.slope_to = want.slope_to;
        }
        (OperationConfig::DropCutter(out), OperationConfig::DropCutter(want)) => {
            note("stepover", out.stepover, want.stepover);
            out.stepover = want.stepover;
            out.scallop_height = want.scallop_height;
        }
        (OperationConfig::Pencil(out), OperationConfig::Pencil(want)) => {
            // Suggest maps its stepover onto `offset_stepover`. Every pencil
            // dial but the four feed fields is the arm's: put them all back.
            note("offset_stepover", out.offset_stepover, want.offset_stepover);
            note(
                "num_offset_passes",
                out.num_offset_passes as f64,
                want.num_offset_passes as f64,
            );
            note(
                "min_valley_depth",
                out.min_valley_depth,
                want.min_valley_depth,
            );
            note("stock_to_leave", out.stock_to_leave, want.stock_to_leave);
            if out.detector != want.detector || out.reference_tool_id != want.reference_tool_id {
                changed.push("detector / reference_tool_id: put back".to_owned());
            }
            let mut keep = want.clone();
            keep.feed_rate = out.feed_rate;
            keep.plunge_rate = out.plunge_rate;
            keep.ramp_feed_rate = out.ramp_feed_rate;
            keep.spindle_rpm = out.spindle_rpm;
            *out = keep;
        }
        _ => {}
    }
    changed
}

/// Which operations own their geometry dials (the finish ops of the arm).
fn arm_owns_geometry(op: &OperationConfig) -> bool {
    matches!(
        op,
        OperationConfig::Scallop(_)
            | OperationConfig::UnifiedFinish(_)
            | OperationConfig::DropCutter(_)
            | OperationConfig::Pencil(_)
    )
}

/// The CLI's `--apply-suggest` pass, replicated from
/// `apply_suggested_feeds_to_session` (`crates/rs_cam_cli/src/project.rs`):
/// `cutter_op_profile` per enabled toolpath, the suggested operation, the
/// rounded RPM, the per-field provenance and the ramp stamp, written back
/// with `ReplaceToolpathConfig`. One addition: a finish op's geometry dials
/// are put back (see [`restore_geometry`]).
///
/// Returns the feed record, or `Err` with the refusal when an enabled
/// operation gets no feasible feed.
fn apply_suggest(session: &mut ProjectSession) -> Result<Vec<Value>, String> {
    let mut suggestions = Vec::new();
    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let Some(profile) = session.cutter_op_profile(tc) else {
            return Err(format!(
                "toolpath {} ({}): tool {} not found",
                tc.id.0, tc.name, tc.tool_id
            ));
        };
        if let Err(e) = &profile.feasibility {
            return Err(format!(
                "toolpath {} ({}): Suggest refused: {e}",
                tc.id.0, tc.name
            ));
        }
        let (Some(operation), Some(feeds)) = (profile.suggested_operation, profile.feeds) else {
            return Err(format!(
                "toolpath {} ({}): Suggest gave no feeds",
                tc.id.0, tc.name
            ));
        };
        let rpm_written = feeds.rpm.is_finite() && feeds.rpm > 0.0;
        let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
        provenance.apply_suggested(&feeds, &operation, rpm_written);
        let ramp = profile.warnings.iter().find_map(|w| match w {
            rs_cam_core::feeds::suggest::SuggestWarning::RampFeed { record, .. } => Some(record),
            _ => None,
        });
        if let Some(record) = ramp {
            provenance.stamp_ramp(record);
        }
        let lut = feeds
            .matched_lut_row
            .as_ref()
            .map(|r| format!("{r:?}").chars().take(240).collect::<String>());
        suggestions.push((idx, operation, feeds.rpm, provenance, lut));
    }

    let mut record = Vec::new();
    for (idx, suggested_op, suggested_rpm, provenance, lut) in suggestions {
        let mut draft = session.toolpath_configs()[idx].clone();
        let planned = draft.operation.clone();
        let before = json!({
            "feed": planned.feed_rate(),
            "plunge": planned.plunge_rate(),
            "stepover": planned.stepover(),
            "dpp": planned.depth_per_pass(),
            "rpm": planned.spindle_rpm(),
        });
        draft.operation = suggested_op;
        if suggested_rpm.is_finite() && suggested_rpm > 0.0 {
            draft
                .operation
                .set_spindle_rpm(Some(suggested_rpm.round() as u32));
        }
        draft.feeds_provenance = provenance;
        let restored = if arm_owns_geometry(&planned) {
            restore_geometry(&mut draft.operation, &planned)
        } else {
            Vec::new()
        };
        let after = json!({
            "feed": draft.operation.feed_rate(),
            "plunge": draft.operation.plunge_rate(),
            "stepover": draft.operation.stepover(),
            "dpp": draft.operation.depth_per_pass(),
            "rpm": draft.operation.spindle_rpm(),
        });
        record.push(json!({
            "id": draft.id.0,
            "name": draft.name,
            "before": before,
            "after": after,
            "geometry_put_back_after_suggest": restored,
            "provenance": serde_json::to_value(&draft.feeds_provenance).unwrap(),
            "matched_lut_row": lut,
        }));
        let _ = session
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index: idx,
                config: Box::new(draft),
            }))
            .expect("replace toolpath config");
    }
    Ok(record)
}

/// Set the arm up. Returns notes for the JSON.
fn configure_arm(
    session: &mut ProjectSession,
    arm: &Arm,
    r2: usize,
    h1: Option<usize>,
) -> Vec<String> {
    let tool_id = |t: Tool| match t {
        Tool::R1 => R1_TOOL,
        Tool::R2 => r2,
        Tool::H1 => h1.expect("H1 was added"),
    };
    let scallop = index_of(session, SCALLOP_ID);
    let mut notes = Vec::new();
    match arm {
        Arm::Whole {
            tool,
            cusp,
            iso_field,
        } => {
            set_tool(session, scallop, tool_id(*tool));
            dial_scallop(session, scallop, *cusp, *iso_field);
            notes.push(format!(
                "op {SCALLOP_ID} re-dialled: tool {}, h {cusp}, iso_field {iso_field}",
                tool_id(*tool)
            ));
        }
        Arm::Raster {
            cusp,
            design_slope_deg,
            split_deg,
        } => {
            let stepover = equal_cusp_stepover_mm(1.0, *cusp) * design_slope_deg.to_radians().cos();
            let slope_to = split_deg.unwrap_or(90.0);
            let original = session.toolpath_configs()[scallop].clone();
            let mut cfg = original.clone();
            let mut op = OperationConfig::new_default(OperationType::DropCutter);
            match &mut op {
                OperationConfig::DropCutter(d) => {
                    d.stepover = stepover;
                    d.scallop_height = Some(*cusp);
                    // The scallop op ran full slope range 0..90 too.
                    d.slope_from = 0.0;
                    d.slope_to = slope_to;
                }
                _ => unreachable!(),
            }
            cfg.operation = op;
            cfg.name = format!("Parallel raster R1 s {stepover:.4}");
            cfg.tool_id = R1_TOOL;
            let _ = session
                .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                    index: scallop,
                    config: Box::new(cfg),
                }))
                .expect("replace op 2 with a raster");
            notes.push(format!(
                "op {SCALLOP_ID} replaced by DropCutter (parallel raster), stepover {stepover} = \
                 equal_cusp_stepover_mm(1.0, {cusp}) x cos({design_slope_deg} deg); slope 0..{slope_to}; \
                 op 2's boundary, heights and dressups kept; raster min_z default"
            ));
            if let Some(split) = split_deg {
                let mut steep = original;
                steep.name = format!("R1 iso h {cusp} on slopes {split}..90");
                let created = session
                    .apply(Command::AddToolpath(AddToolpathArgs {
                        setup_index: 0,
                        config: Box::new(steep),
                    }))
                    .expect("add the steep op")
                    .created
                    .expect("add_toolpath reports the index");
                set_tool(session, created, R1_TOOL);
                dial_scallop(session, created, *cusp, true);
                set_param(session, created, "slope_from", json!(split));
                set_param(session, created, "slope_to", json!(90.0));
                notes.push(format!(
                    "new op (index {created}) = R1 iso h {cusp}, slopes {split}..90, after the raster"
                ));
            }
        }
        Arm::Semi { semi, cusp } => {
            // The R1 finish: a copy of op 2 appended at the end of the setup.
            let mut r1 = session.toolpath_configs()[scallop].clone();
            r1.name = format!("R1 iso h {cusp} after semi");
            let created = session
                .apply(Command::AddToolpath(AddToolpathArgs {
                    setup_index: 0,
                    config: Box::new(r1),
                }))
                .expect("add the R1 op")
                .created
                .expect("add_toolpath reports the index");
            set_tool(session, created, R1_TOOL);
            dial_scallop(session, created, *cusp, true);
            // Op 2 becomes the R2 semi-finish and runs first.
            set_tool(session, scallop, r2);
            dial_scallop(session, scallop, *semi, true);
            notes.push(format!(
                "op {SCALLOP_ID} = R2 iso h {semi}; new op (index {created}) = R1 iso h {cusp}, \
                 appended after it; both stock_source Fresh (the fixture's value): the iso-field \
                 scallop path follows the surface, so it needs no remaining stock to plan"
            ));
        }
        Arm::Tier {
            tools,
            tolerance,
            strategy,
            tier0_cusp,
            coarse_skips_fine_islands,
        } => {
            set_enabled(session, scallop, false);
            let ids: Vec<usize> = tools.iter().map(|t| tool_id(*t)).collect();
            let strategies = match strategy {
                Some(s) => vec![*s; ids.len()],
                None => Vec::new(),
            };
            let spec = MultitoolPlanSpec {
                setup_index: 0,
                model_id: 1,
                tool_ids: ids.clone(),
                tolerance_mm: *tolerance,
                cusp_height_mm: CUSP_MM,
                tier_strategies: strategies,
                coarse_skips_fine_islands: *coarse_skips_fine_islands,
                ..MultitoolPlanSpec::default()
            };
            let spec = blob_islands(spec, &mut notes);
            let outcome = session
                .plan_multitool_finishing(&spec)
                .expect("the planner emits the chain");
            assert!(
                outcome.replaced.is_empty(),
                "the fixture holds no planner op; replaced {:?}",
                outcome.replaced
            );
            notes.push(format!(
                "op {SCALLOP_ID} disabled; planner emitted {:?} (coarse -> fine), tools {ids:?}, \
                 tolerance {tolerance}, cusp {CUSP_MM}, strategy {:?}, \
                 coarse_skips_fine_islands {coarse_skips_fine_islands}",
                outcome.toolpath_ids.iter().map(|t| t.0).collect::<Vec<_>>(),
                strategy.unwrap_or_default()
            ));
            if let Some(h) = tier0_cusp {
                let first = index_of(session, outcome.toolpath_ids[0].0);
                set_param(session, first, "scallop_height", json!(*h));
                let (got, _) = scallop_of(session, first);
                assert!((got - h).abs() < 1e-12, "tier 0 cusp did not take");
                notes.push(format!("tier 0 scallop_height set to {h} after planning"));
            }
        }
        Arm::CoarsePencil { coarse_cusp } => {
            set_tool(session, scallop, r2);
            dial_scallop(session, scallop, *coarse_cusp, true);
            let passes = pencil_passes();
            // R1 tip radius 1.0 mm (tool 6), the arm's cusp.
            let offset_stepover = equal_cusp_stepover_mm(1.0, CUSP_MM);
            let r2_diameter = session
                .get_tool(ToolId(r2))
                .expect("the R2 was added")
                .diameter;
            let pencil = PencilConfig {
                detector: PencilDetector::RestDepth,
                reference_tool_id: Some(ToolId(r2)),
                // Read only when the id does not resolve; kept equal to it.
                reference_tool_diameter: r2_diameter,
                // Keep a valley where the R2 leaves more than the arm's cusp
                // above what the R1 reaches.
                min_valley_depth: CUSP_MM,
                num_offset_passes: passes,
                offset_stepover,
                ..PencilConfig::default()
            };
            // The GUI / MCP add route (`core_add_toolpath`,
            // `handle_add_toolpath`): a ToolpathConfig with the op, the
            // op type's `DressupConfig::for_op` and `StockSource::Fresh`,
            // appended with `Command::AddToolpath`. Boundary and heights are
            // op 2's (model silhouette, offset 1.0) so every finish op of
            // the trial shares one boundary; the GUI would give
            // `BoundaryConfig::for_3d_op(diameter)`. Fresh stock keeps the
            // rest reference on the R2 tool (reference mode 1), not on the
            // simulated stock.
            let mut cfg = session.toolpath_configs()[scallop].clone();
            cfg.name = format!("Pencil rest_depth R1 after R2, passes {passes}");
            cfg.tool_id = R1_TOOL;
            cfg.operation = OperationConfig::Pencil(pencil.clone());
            cfg.dressups = DressupConfig::for_op(OperationType::Pencil);
            cfg.stock_source = StockSource::Fresh;
            cfg.feeds_provenance = rs_cam_core::feeds::FeedsProvenance::default();
            let created = session
                .apply(Command::AddToolpath(AddToolpathArgs {
                    setup_index: 0,
                    config: Box::new(cfg),
                }))
                .expect("add the pencil op")
                .created
                .expect("add_toolpath reports the index");
            notes.push(format!(
                "op {SCALLOP_ID} = R2 (tool {r2}) iso h {coarse_cusp} whole board; new Pencil op \
                 (index {created}) on tool {R1_TOOL}: detector rest_depth, reference_tool_id {r2} \
                 (diameter {r2_diameter}), min_valley_depth {CUSP_MM}, num_offset_passes cap \
                 {passes} (TIER_TRIAL_PENCIL_PASSES, default 3), offset_stepover \
                 {offset_stepover} = equal_cusp_stepover_mm(1.0, {CUSP_MM}), rest_cell_mm {}, \
                 sampling {}, stock_source Fresh, op 2's boundary",
                pencil.rest_cell_mm, pencil.sampling
            ));
        }
        Arm::CoarseTierCleanup {
            coarse_cusp,
            tolerance,
        } => {
            let spec = MultitoolPlanSpec {
                setup_index: 0,
                model_id: 1,
                tool_ids: vec![r2, R1_TOOL],
                tolerance_mm: *tolerance,
                cusp_height_mm: CUSP_MM,
                tier_strategies: vec![TierStrategy::IsoScallop; 2],
                ..MultitoolPlanSpec::default()
            };
            let outcome = session
                .plan_multitool_finishing(&spec)
                .expect("the planner emits the chain");
            assert!(
                outcome.replaced.is_empty(),
                "the fixture holds no planner op"
            );
            assert_eq!(outcome.toolpath_ids.len(), 2, "two tiers");
            let tier0 = index_of(session, outcome.toolpath_ids[0].0);
            let tier1 = index_of(session, outcome.toolpath_ids[1].0);
            {
                let t1 = &session.toolpath_configs()[tier1];
                assert_eq!(t1.tool_id, R1_TOOL, "tier 1 is the R1");
                assert!(
                    t1.boundary.enabled
                        && matches!(
                            t1.boundary.source,
                            BoundarySource::PlannedTierRegions { tier: 1, .. }
                        ),
                    "tier 1 carries its planned tier regions"
                );
                assert_eq!(
                    session.toolpath_configs()[tier0].tool_id,
                    r2,
                    "tier 0 is the R2"
                );
            }
            set_enabled(session, tier0, false);
            set_tool(session, scallop, r2);
            dial_scallop(session, scallop, *coarse_cusp, true);
            notes.push(format!(
                "planner [R2 {r2}, R1 {R1_TOOL}] IsoScallop, tolerance {tolerance}, cusp \
                 {CUSP_MM}: emitted {:?}; tier-0 op {} DISABLED; op {SCALLOP_ID} = R2 iso h \
                 {coarse_cusp} whole board (model silhouette, Fresh) in its place; R1 cleanup = \
                 tier-1 op {} (iso scallop, PlannedTierRegions tier 1, FromRemainingStock)",
                outcome.toolpath_ids.iter().map(|t| t.0).collect::<Vec<_>>(),
                outcome.toolpath_ids[0].0,
                outcome.toolpath_ids[1].0
            ));
        }
    }
    notes
}

/// P1's cap on pencil offset passes per side (`TIER_TRIAL_PENCIL_PASSES`,
/// default 3). See the module doc: the default is a judgement call.
fn pencil_passes() -> usize {
    std::env::var("TIER_TRIAL_PENCIL_PASSES")
        .ok()
        .map_or(3, |v| {
            v.parse().expect("TIER_TRIAL_PENCIL_PASSES is a count")
        })
}

/// The boundary of one op: `None` when disabled, else its source label
/// and, for a planned tier boundary, its tier and tolerance.
fn boundary_summary(tc: &rs_cam_core::session::ToolpathConfig) -> Value {
    if !tc.boundary.enabled {
        return Value::Null;
    }
    let mut v = json!({
        "source": tc.boundary.source.label(),
        "containment": format!("{:?}", tc.boundary.containment),
        "offset_mm": tc.boundary.offset,
    });
    if let BoundarySource::PlannedTierRegions {
        tool_ids,
        tier,
        tolerance_mm,
        cell_mm,
        ..
    } = &tc.boundary.source
    {
        v["planned_tier"] = json!({
            "tool_ids": tool_ids, "tier": tier, "tolerance_mm": tolerance_mm, "cell_mm": cell_mm,
        });
    }
    v
}

/// The pencil dials of one op, or `None` for any other op.
fn pencil_summary(op: &OperationConfig) -> Value {
    match op {
        OperationConfig::Pencil(p) => json!({
            "detector": format!("{:?}", p.detector),
            "reference_tool_id": p.reference_tool_id.map(|t| t.0),
            "reference_tool_diameter": p.reference_tool_diameter,
            "min_valley_depth": p.min_valley_depth,
            "num_offset_passes": p.num_offset_passes,
            "offset_stepover": p.offset_stepover,
            "rest_cell_mm": p.rest_cell_mm,
            "sampling": p.sampling,
            "stock_to_leave": p.stock_to_leave,
        }),
        _ => Value::Null,
    }
}

// ── Toolpath measures ───────────────────────────────────────────────────

fn toolpath_counts(tp: &rs_cam_core::toolpath::Toolpath) -> Value {
    let mut rapid_runs = 0usize;
    let mut entry_runs = 0usize;
    let mut retract_moves = 0usize;
    let mut prev_rapid = false;
    let mut prev_entry = false;
    for m in &tp.moves {
        let rapid = matches!(m.move_type, MoveType::Rapid);
        if rapid && !prev_rapid {
            rapid_runs += 1;
        }
        let entry = matches!(
            m.intent,
            MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp
        );
        if entry && !prev_entry {
            entry_runs += 1;
        }
        if matches!(m.intent, MoveIntent::Retract) {
            retract_moves += 1;
        }
        prev_rapid = rapid;
        prev_entry = entry;
    }
    json!({
        "rapid_runs": rapid_runs,
        "entry_runs": entry_runs,
        "retract_moves": retract_moves,
    })
}

// ── Population and statistics ───────────────────────────────────────────

struct Population {
    pieces: Vec<Polygon2>,
    area_mm2: f64,
    silhouette_area_mm2: f64,
    vertex_count: usize,
    /// Scanline cache: for one `y` (by bit pattern), the sorted `x` where
    /// a ring edge crosses the line. Every column of a grid row shares its
    /// `y`, so one row costs one pass over the edges.
    rows: std::collections::HashMap<u64, Vec<f64>>,
}

impl Population {
    /// Even-odd point-in-polygon over every ring of every piece, by a ray
    /// to +x. An edge counts when `(a.y > y) != (b.y > y)` (half-open), so
    /// a point on a horizontal edge or a vertex has one fixed answer.
    fn contains(&mut self, x: f64, y: f64) -> bool {
        let pieces = &self.pieces;
        let xs = self.rows.entry(y.to_bits()).or_insert_with(|| {
            let mut xs = Vec::new();
            for poly in pieces {
                for ring in std::iter::once(&poly.exterior).chain(poly.holes.iter()) {
                    let n = ring.len();
                    for i in 0..n {
                        let (a, b) = (ring[i], ring[(i + 1) % n]);
                        if (a.y > y) != (b.y > y) {
                            xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                        }
                    }
                }
            }
            xs.sort_by(f64::total_cmp);
            xs
        });
        let beyond = xs.len() - xs.partition_point(|&cx| cx <= x);
        beyond % 2 == 1
    }
}

fn population(mesh: &TriangleMesh) -> Population {
    let sil = model_silhouette(mesh, None);
    let outline = silhouette_machining_outline(&sil).expect("the model has a silhouette");
    let pieces = offset_polygon(&outline, INSET_MM);
    assert!(!pieces.is_empty(), "the 3 mm inset is not empty");
    let area: f64 = pieces.iter().map(Polygon2::area).sum();
    assert!(
        area < outline.area(),
        "a positive offset must shrink the outline ({area} vs {})",
        outline.area()
    );
    Population {
        vertex_count: pieces.iter().map(|p| p.exterior.len()).sum(),
        area_mm2: area,
        silhouette_area_mm2: outline.area(),
        pieces,
        rows: std::collections::HashMap::new(),
    }
}

/// Nearest-rank quantile of a sorted slice: `sorted[ceil(p·n) − 1]`.
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let rank = ((p * n as f64).ceil() as usize).clamp(1, n);
    sorted[rank - 1]
}

fn stats(devs: &[f64]) -> Value {
    if devs.is_empty() {
        return json!({ "n": 0 });
    }
    let mut s = devs.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    let over = s.iter().filter(|&&d| d > 0.06).count() as f64;
    let gouge = s.iter().filter(|&&d| d < -0.02).count() as f64;
    json!({
        "n": s.len(),
        "p50": quantile(&s, 0.50),
        "p90": quantile(&s, 0.90),
        "p99": quantile(&s, 0.99),
        "frac_dev_gt_0_06": over / n,
        "frac_dev_lt_m0_02": gouge / n,
        "min": s[0],
        "max": s[s.len() - 1],
    })
}

// ── Residual maps ───────────────────────────────────────────────────────

/// The discrete colour bands of every residual map (mm, lower edge
/// inclusive). Written into the JSON as `map_legend`.
const BANDS: [(f64, [u8; 3]); 11] = [
    (f64::NEG_INFINITY, [80, 0, 120]),
    (-0.05, [200, 0, 60]),
    (-0.02, [120, 170, 255]),
    (0.0, [255, 255, 255]),
    (0.015, [255, 245, 170]),
    (0.03, [255, 210, 90]),
    (0.045, [250, 160, 40]),
    (0.06, [230, 90, 20]),
    (0.10, [180, 30, 20]),
    (0.30, [90, 0, 0]),
    (1.0, [0, 0, 0]),
];
const OUTSIDE: [u8; 3] = [150, 150, 150];

fn band_colour(dev: f64) -> [u8; 3] {
    let mut c = BANDS[0].1;
    for (edge, col) in BANDS {
        if dev >= edge {
            c = col;
        }
    }
    c
}

fn legend() -> Value {
    let rows: Vec<Value> = BANDS
        .iter()
        .map(|(e, c)| json!({ "from_mm": if e.is_finite() { json!(e) } else { json!("-inf") }, "rgb": c }))
        .collect();
    json!({ "bands": rows, "outside_population_rgb": OUTSIDE, "orientation": "north (+y) up, +x right" })
}

/// Paint columns into a PNG over `[x0, x0 + w·cell) × [y0, y0 + h·cell)`.
fn write_map(path: &std::path::Path, cols: &[(f64, f64, f64, bool)], cell: f64) {
    // Column centres sit on a lattice of pitch `cell`. Index each column by
    // ROUNDING its offset from the lowest centre, so float noise cannot put
    // two columns on one pixel.
    let x0 = cols.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let y0 = cols.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let x1 = cols.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
    let y1 = cols.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
    let w = ((x1 - x0) / cell).round() as u32 + 1;
    let h = ((y1 - y0) / cell).round() as u32 + 1;
    let mut img = image::RgbImage::from_pixel(w, h, image::Rgb([40, 40, 40]));
    for &(x, y, dev, inside) in cols {
        let px = ((x - x0) / cell).round() as u32;
        let py = ((y - y0) / cell).round() as u32;
        let c = if inside { band_colour(dev) } else { OUTSIDE };
        img.put_pixel(px, h - 1 - py, image::Rgb(c));
    }
    img.save(path).expect("write the residual map");
}

// ── Window simulation ───────────────────────────────────────────────────

/// What a window simulation needs of one toolpath, kept after the session
/// is dropped. Built the way `ProjectSession::run_simulation_memoized`
/// builds its entries (`src/session/compute/simulation.rs`).
struct EntryParts {
    id: rs_cam_core::ToolpathId,
    name: String,
    annotated: Arc<rs_cam_core::trace::toolpath_spans::AnnotatedToolpath>,
    tool: Arc<rs_cam_core::tool::ToolDefinition>,
    flute_count: u32,
    tool_summary: String,
    semantic_trace: Option<Arc<rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>>,
    spindle_rpm: Option<u32>,
    metrics_not_applicable: bool,
    drill_op: Option<Arc<rs_cam_core::ops::drill_op::DrillOp>>,
    operation_config_hash: u64,
}

impl EntryParts {
    fn entry(&self) -> SimToolpathEntry {
        SimToolpathEntry {
            id: self.id,
            name: self.name.clone(),
            annotated: Arc::clone(&self.annotated),
            tool: Arc::clone(&self.tool),
            flute_count: self.flute_count,
            tool_summary: self.tool_summary.clone(),
            semantic_trace: self.semantic_trace.clone(),
            spindle_rpm: self.spindle_rpm,
            metrics_not_applicable: self.metrics_not_applicable,
            drill_op: self.drill_op.clone(),
            operation_config_hash: self.operation_config_hash,
        }
    }
}

fn collect_entries(session: &ProjectSession) -> Vec<EntryParts> {
    let mut out = Vec::new();
    for setup in session.list_setups() {
        for &idx in &setup.toolpath_indices {
            let tc = &session.toolpath_configs()[idx];
            let Some(result) = session.get_result(idx) else {
                continue;
            };
            if !tc.enabled || !contributes_simulated_motion(result.annotated().toolpath.moves.len())
            {
                continue;
            }
            let tool = session
                .get_tool(ToolId(tc.tool_id))
                .expect("every arm tool resolves");
            let (tool_def, flute_count, tool_summary) = entry_tool_fields(tool);
            out.push(EntryParts {
                id: tc.id,
                name: tc.name.clone(),
                annotated: Arc::clone(result.annotated()),
                tool: tool_def,
                flute_count,
                tool_summary,
                semantic_trace: result.semantic_trace.clone(),
                spindle_rpm: tc.operation.spindle_rpm(),
                metrics_not_applicable: entry_metrics_not_applicable(
                    result.is_drill_op(),
                    &result.annotated().toolpath,
                    tc.operation.op_type(),
                ),
                drill_op: result.drill_op().cloned(),
                operation_config_hash: hash_operation_config(&tc.operation),
            });
        }
    }
    out
}

fn translate(mesh: &TriangleMesh, d: P3) -> TriangleMesh {
    let verts = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x - d.x, v.y - d.y, v.z - d.z))
        .collect();
    TriangleMesh::from_raw(verts, mesh.triangles.clone())
}

struct WindowCtx<'a> {
    parts: &'a [EntryParts],
    mesh: &'a TriangleMesh,
    stock_z: (f64, f64),
    spindle_rpm: u32,
    rapid_feed: f64,
}

struct WindowRun {
    cols: Vec<ColumnDeviation>,
    /// Per column with dev < -0.02: the position (in toolpath order) of the
    /// first toolpath after which the column top was already more than
    /// 0.02 mm under the model. Read off the run's per-toolpath checkpoints.
    first_gouge: Vec<Option<usize>>,
    cell_used: f64,
    clamped: bool,
    secs: f64,
}

/// Simulate one window. The request's stock is the window in XY and the
/// whole stock in Z. The setup is identity (face up top, rotation 0), so
/// the group grid is world-framed (F-024) and the model mesh goes in the
/// window's stock-relative frame: the world mesh shifted by `-window.min`.
fn simulate_window(ctx: &WindowCtx<'_>, cx: f64, cy: f64, cell: f64) -> WindowRun {
    let t0 = Instant::now();
    let half = WINDOW_MM / 2.0;
    let bbox = BoundingBox3 {
        min: P3::new(cx - half, cy - half, ctx.stock_z.0),
        max: P3::new(cx + half, cy + half, ctx.stock_z.1),
    };
    let request = SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: ctx.parts.iter().map(EntryParts::entry).collect(),
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: bbox,
        stock_top_z: bbox.max.z,
        resolution: cell,
        spindle_rpm: ctx.spindle_rpm,
        rapid_feed_mm_min: ctx.rapid_feed,
        model_mesh: Some(Arc::new(translate(ctx.mesh, bbox.min))),
        kinematics: None,
        display_stride: 1,
    };
    let cancel = AtomicBool::new(false);
    let result = run_simulation(&request, &cancel).expect("the window simulates");
    let cols = result
        .column_deviations
        .clone()
        .expect("a model gives deviations");
    assert_eq!(
        result.checkpoints.len(),
        ctx.parts.len(),
        "one checkpoint per toolpath"
    );
    let first_gouge = cols
        .iter()
        .map(|c| {
            if c.dev >= -0.02 {
                return None;
            }
            // dev = top - model (identity frame), so model = top - dev.
            let model = f64::from(c.top_z) - f64::from(c.dev);
            result.checkpoints.iter().position(|cp| {
                cp.mesh_stock
                    .z_grid
                    .top_z_at(c.row, c.col)
                    .is_none_or(|t| f64::from(t) - model < -0.02)
            })
        })
        .collect();
    WindowRun {
        first_gouge,
        cols,
        cell_used: result.column_grid_cell_mm,
        clamped: result.resolution_clamped,
        secs: t0.elapsed().as_secs_f64(),
    }
}

fn vm_hwm_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

// ── The instrument ──────────────────────────────────────────────────────

#[test]
#[ignore = "tier trial instrument: one arm per run (TIER_TRIAL_ARM), release build, minutes"]
fn tier_trial_350() {
    let wall = Instant::now();
    let arm_name = std::env::var("TIER_TRIAL_ARM").expect("set TIER_TRIAL_ARM (A1..F1)");
    let arm = arm_of(&arm_name);
    let window_cell: f64 = std::env::var("TIER_TRIAL_CELL")
        .ok()
        .map_or(DEFAULT_WINDOW_CELL_MM, |v| {
            v.parse().expect("TIER_TRIAL_CELL is a number")
        });
    let cell_check: Option<usize> = std::env::var("TIER_TRIAL_CELL_CHECK")
        .ok()
        .map(|v| v.parse().expect("TIER_TRIAL_CELL_CHECK is a window index"));
    let tag = std::env::var("TIER_TRIAL_TAG").ok();
    let mut stem = arm_name.clone();
    if (window_cell - DEFAULT_WINDOW_CELL_MM).abs() > 1e-12 {
        stem.push_str(&format!("_cell{window_cell}"));
    }
    if let Some(t) = &tag {
        stem.push('_');
        stem.push_str(t);
    }
    let out_dir = repo_root().join("planning/tier_trial_2026-10-01/runs");
    std::fs::create_dir_all(&out_dir).expect("create the runs folder");
    eprintln!("== tier trial arm {arm_name} ({arm:?}), window cell {window_cell}, out {stem}");

    // 1. Load, add tools.
    let donor = ProjectSession::load(
        &repo_root().join("planning/fixtures/rivmap100/rivmap100_tiered_finish.toml"),
    )
    .expect("load the rivmap100 tiered donor");
    let r2_cfg = donor
        .tools()
        .iter()
        .find(|t| t.id.0 == DONOR_R2_TOOL)
        .expect("the donor's R2.0 taper")
        .clone();
    drop(donor);
    let fixture = repo_root().join("planning/fixtures/rivmap100/rivmap100_memory_repro.toml");
    let mut session = ProjectSession::load(&fixture).expect("load the x3.5 project");
    let r2 = add_tool(&mut session, r2_cfg);
    let h1 = arm_uses_h1(&arm).then(|| add_tool(&mut session, h1_tool()));

    let model_mesh = session
        .models()
        .iter()
        .find(|m| m.id == 1)
        .and_then(|m| m.mesh.clone())
        .expect("model 1 carries the terrain mesh");
    let mbb = model_mesh.bbox;
    let (mx, my) = (mbb.max.x - mbb.min.x, mbb.max.y - mbb.min.y);
    assert!(
        (mx - LATTICE_SPAN_MM).abs() < 1.0 && (my - LATTICE_SPAN_MM).abs() < 1.0,
        "the plan's lattice assumes a 350 x 350 model; the bbox is {mx} x {my}"
    );
    let setup = &session.list_setups()[0];
    assert_eq!(
        (setup.face_up, setup.z_rotation),
        (
            rs_cam_core::compute::FaceUp::Top,
            rs_cam_core::compute::ZRotation::Deg0
        ),
        "the window frame assumes an identity setup"
    );
    let stock = session.stock_bbox();

    // 2. The arm.
    let notes = configure_arm(&mut session, &arm, r2, h1);

    // 3. Suggest feeds.
    let feeds = match apply_suggest(&mut session) {
        Ok(f) => f,
        Err(reason) => {
            let doc = json!({ "arm": arm_name, "refused": true, "reason": reason, "notes": notes });
            let path = out_dir.join(format!("{stem}.json"));
            std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
            eprintln!("REFUSED: {reason}");
            return;
        }
    };

    // 3b. Optional arc-fit override on every finish op (env `TIER_TRIAL_ARC`:
    // `off`, or a tolerance in mm). The diagnosis arm of the A1 gouge read.
    let mut notes = notes;
    if std::env::var("TIER_TRIAL_DRESSUPS").as_deref() == Ok("planner") {
        notes.push(planner_dressups(&mut session));
    }
    if let Ok(arc) = std::env::var("TIER_TRIAL_ARC") {
        notes.push(override_arc_fitting(&mut session, &arc));
    }

    // 4. The generation plan walk at 0.25 mm (CLI `run_generation_plan`).
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(BOARD_CELL_MM),
            },
        ))
        .expect("set the board cell");
    let sim_opts = SimulationOptions {
        resolution: BOARD_CELL_MM,
        ..SimulationOptions::default()
    };
    let cancel = AtomicBool::new(false);
    let mut cache = SimPrefixCache::new();
    let mut gen_secs: Vec<(usize, f64)> = Vec::new();
    let mut walk_sims = 0usize;
    let steps = generation_plan::plan(&session, Scope::Project);
    let t_walk = Instant::now();
    for step in &steps {
        match *step {
            Step::Simulate { .. } => {
                walk_sims += 1;
                session
                    .run_simulation_memoized(
                        &sim_opts,
                        &cancel,
                        Some(SimMemo {
                            cache: &mut cache,
                            store: true,
                        }),
                    )
                    .expect("a plan simulation");
            }
            Step::Generate { toolpath, index } => {
                let t = Instant::now();
                session
                    .generate_toolpath(index, &cancel)
                    .unwrap_or_else(|e| panic!("generate toolpath {}: {e}", toolpath.0));
                gen_secs.push((toolpath.0, t.elapsed().as_secs_f64()));
                eprintln!(
                    "   generated {} in {:.1} s",
                    toolpath.0,
                    t.elapsed().as_secs_f64()
                );
            }
        }
    }
    let t_close = Instant::now();
    session
        .run_simulation_memoized(
            &sim_opts,
            &cancel,
            Some(SimMemo {
                cache: &mut cache,
                store: false,
            }),
        )
        .expect("the closing simulation");
    drop(cache);
    let close_secs = t_close.elapsed().as_secs_f64();
    let walk_secs = t_walk.elapsed().as_secs_f64();
    eprintln!("   walk {walk_secs:.1} s ({walk_sims} plan sims), closing sim {close_secs:.1} s");

    // 5. Board-level reads.
    let mut pop = population(&model_mesh);
    let sim = session
        .simulation_result()
        .expect("the closing simulation is stored");
    let trace = sim
        .cut_trace
        .as_ref()
        .expect("the closing run has a cut trace");
    let ranges: Vec<_> = sim.boundaries.iter().map(|b| b.range()).collect();
    let mut collisions_by_id = std::collections::BTreeMap::<usize, usize>::new();
    for &g in &sim.rapid_collision_move_indices {
        if let Some(loc) = locate_global_move(ranges.iter().copied(), g) {
            *collisions_by_id.entry(loc.toolpath_id.0).or_default() += 1;
        }
    }
    let mut ops = Vec::new();
    let finish_ids: Vec<usize> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled && tc.id.0 != FACE_ID && tc.id.0 != ROUGH_ID)
        .map(|tc| tc.id.0)
        .collect();
    let (mut rough_s, mut finish_s) = (0.0, 0.0);
    for setup in session.list_setups() {
        for &idx in &setup.toolpath_indices {
            let tc = &session.toolpath_configs()[idx];
            if !tc.enabled {
                continue;
            }
            let result = session.get_result(idx).expect("every enabled op generated");
            let rt = trace
                .toolpath_runtimes
                .iter()
                .find(|r| r.toolpath_id == tc.id)
                .map(|r| r.breakdown)
                .expect("the kinematics integrator walked every toolpath");
            if finish_ids.contains(&tc.id.0) {
                finish_s += rt.total_s;
            } else {
                rough_s += rt.total_s;
            }
            let tool = session.get_tool(ToolId(tc.tool_id)).expect("tool");
            let (cusp, iso) = match &tc.operation {
                OperationConfig::Scallop(s) => (Some(s.scallop_height), Some(s.iso_field)),
                OperationConfig::UnifiedFinish(u) => (Some(u.scallop_height), None),
                OperationConfig::DropCutter(d) => (d.scallop_height, None),
                _ => (None, None),
            };
            ops.push(json!({
                "id": tc.id.0,
                "name": tc.name,
                "kind": tc.operation.op_type().kind_str(),
                "role": if finish_ids.contains(&tc.id.0) { "finish" } else { "face_rough" },
                "tool_id": tc.tool_id,
                "tool_name": tool.name,
                "hypothetical_tool": Some(tc.tool_id) == h1,
                "stock_source": format!("{:?}", tc.stock_source),
                "arc_fitting": serde_json::to_value(tc.dressups.arc_fitting).unwrap(),
                "planner_tier": tc.planner_origin.as_ref().map(|p| p.tier),
                "scallop_height": cusp,
                "iso_field": iso,
                "stepover": tc.operation.stepover(),
                "boundary": boundary_summary(tc),
                "pencil": pencil_summary(&tc.operation),
                "feed_mm_min": tc.operation.feed_rate(),
                "plunge_mm_min": tc.operation.plunge_rate(),
                "rpm": tc.operation.spindle_rpm(),
                "feeds_provenance": serde_json::to_value(&tc.feeds_provenance).unwrap(),
                "runtime_s": rt.total_s,
                "modulation": trace
                    .modulation_summaries
                    .get(&tc.id)
                    .map(|m| serde_json::to_value(m).unwrap()),
                "runtime_breakdown_s": {
                    "rapid": rt.rapid_s, "cutting": rt.cutting_s, "entry": rt.entry_s,
                    "linking": rt.linking_s, "retract": rt.retract_s, "unknown": rt.unknown_s,
                },
                "cutting_mm": result.stats.cutting_distance,
                "rapid_mm": result.stats.rapid_distance,
                "move_count": result.stats.move_count,
                "retract_trips": result.stats.retract_trips.as_ref().map(|r| r.total),
                "counts": toolpath_counts(result.toolpath()),
                "rapid_collision_count": collisions_by_id.get(&tc.id.0).copied().unwrap_or(0),
                "generation_s": gen_secs.iter().find(|(id, _)| *id == tc.id.0).map(|g| g.1),
            }));
        }
    }

    // Whole board at 0.25 mm: gross defects on the population.
    let board_cell = sim.column_grid_cell_mm;
    let board_cols = sim.column_deviations.as_ref().expect("board deviations");
    let mut board_devs = Vec::new();
    let mut board_paint = Vec::new();
    let (mut over_030, mut gouge) = (0usize, 0usize);
    for c in board_cols {
        let inside = pop.contains(c.x, c.y);
        let d = f64::from(c.dev);
        if inside {
            board_devs.push(d);
            if d > 0.30 {
                over_030 += 1;
            }
            if d < -0.02 {
                gouge += 1;
            }
        }
        if c.x >= mbb.min.x - 5.0
            && c.x < mbb.max.x + 5.0
            && c.y >= mbb.min.y - 5.0
            && c.y < mbb.max.y + 5.0
        {
            board_paint.push((c.x, c.y, d, inside));
        }
    }
    // Self-check of the scanline test against `Polygon2::contains_point`
    // on every 997th board column.
    let mut checked = 0usize;
    let mut mismatched = 0usize;
    for c in board_cols.iter().step_by(997) {
        checked += 1;
        let p = P2::new(c.x, c.y);
        let reference = pop.pieces.iter().any(|poly| poly.contains_point(&p));
        if reference != pop.contains(c.x, c.y) {
            mismatched += 1;
        }
    }
    let cell2 = board_cell * board_cell;
    let board = json!({
        "cell_mm": board_cell,
        "resolution_clamped": sim.resolution_clamped,
        "stats": stats(&board_devs),
        "area_dev_gt_0_30_mm2": over_030 as f64 * cell2,
        "area_dev_lt_m0_02_mm2": gouge as f64 * cell2,
        "population_area_mm2_by_columns": board_devs.len() as f64 * cell2,
        "map": format!("{stem}_board.png"),
    });
    write_map(
        &out_dir.join(format!("{stem}_board.png")),
        &board_paint,
        board_cell,
    );
    drop(board_paint);
    drop(board_devs);
    let board_order: Vec<usize> = sim.boundaries.iter().map(|b| b.id.0).collect();

    // Keep what the windows need; drop the session (prior stocks, the
    // checkpoints, the cut trace).
    let parts = collect_entries(&session);
    assert_eq!(
        parts.iter().map(|p| p.id.0).collect::<Vec<_>>(),
        board_order,
        "the window entries are the closing run's toolpaths, in order"
    );
    let spindle_rpm = session.post_config().spindle_speed;
    let rapid_feed = session.machine().max_feed_mm_min.max(1.0);
    let rss_after_board = vm_hwm_kb();
    drop(session);

    // 6. Windows.
    let wctx = WindowCtx {
        parts: &parts,
        mesh: &model_mesh,
        stock_z: (stock.min.z, stock.max.z),
        spindle_rpm,
        rapid_feed,
    };
    let mut windows = Vec::new();
    let mut pooled = Vec::new();
    let mut window_secs = 0.0;
    let mut window_p99 = Vec::new();
    let mut pooled_gouge_by_op = vec![0usize; parts.len()];
    for ky in 0..4 {
        for kx in 0..4 {
            let k = ky * 4 + kx;
            let cx = mbb.min.x + LATTICE_SPAN_MM * (kx as f64 + 0.5) / 4.0;
            let cy = mbb.min.y + LATTICE_SPAN_MM * (ky as f64 + 0.5) / 4.0;
            let run = simulate_window(&wctx, cx, cy, window_cell);
            window_secs += run.secs;
            assert!(
                !run.clamped && (run.cell_used - window_cell).abs() < 1e-9,
                "window {k} ran at {} mm, not {window_cell}",
                run.cell_used
            );
            let mut devs = Vec::new();
            let mut paint = Vec::with_capacity(run.cols.len());
            let mut top_max = f64::NEG_INFINITY;
            let mut gouge_by_op = vec![0usize; parts.len()];
            for (c, first) in run.cols.iter().zip(&run.first_gouge) {
                let inside = pop.contains(c.x, c.y);
                let d = f64::from(c.dev);
                top_max = top_max.max(f64::from(c.top_z));
                if inside {
                    devs.push(d);
                    if let Some(k) = first {
                        gouge_by_op[*k] += 1;
                    }
                }
                paint.push((c.x, c.y, d, inside));
            }
            let map = format!("{stem}_w{k:02}.png");
            write_map(&out_dir.join(&map), &paint, window_cell);
            let st = stats(&devs);
            window_p99.push(st["p99"].as_f64());
            eprintln!(
                "   window {k:2} ({cx:.2}, {cy:.2}) {:.1} s  n {} p99 {:?}",
                run.secs,
                devs.len(),
                st["p99"]
            );
            windows.push(json!({
                "k": k,
                "centre_world_mm": [cx, cy],
                "centre_model_frame_mm": [cx - mbb.min.x, cy - mbb.min.y],
                "columns": run.cols.len(),
                "max_column_top_z": top_max,
                "secs": run.secs,
                "stats": st,
                "gouge_columns_by_first_op": parts
                    .iter()
                    .zip(&gouge_by_op)
                    .map(|(p, n)| json!({ "id": p.id.0, "name": p.name, "columns": n }))
                    .collect::<Vec<_>>(),
                "map": map,
            }));
            for (acc, n) in pooled_gouge_by_op.iter_mut().zip(&gouge_by_op) {
                *acc += n;
            }
            pooled.extend(devs);
        }
    }
    let pooled_stats = stats(&pooled);
    drop(pooled);

    // Optional cell check: one window again at 0.025 mm.
    let cell_check_doc = cell_check.map(|k| {
        let (kx, ky) = (k % 4, k / 4);
        let cx = mbb.min.x + LATTICE_SPAN_MM * (kx as f64 + 0.5) / 4.0;
        let cy = mbb.min.y + LATTICE_SPAN_MM * (ky as f64 + 0.5) / 4.0;
        let run = simulate_window(&wctx, cx, cy, CHECK_CELL_MM);
        assert!(!run.clamped, "the check window was coarsened");
        let mut devs = Vec::new();
        for c in &run.cols {
            if pop.contains(c.x, c.y) {
                devs.push(f64::from(c.dev));
            }
        }
        let st = stats(&devs);
        let p99_fine = st["p99"].as_f64().unwrap();
        let p99_main = window_p99[k].unwrap();
        eprintln!(
            "   cell check window {k}: p99 {p99_main:.4} at {window_cell} vs {p99_fine:.4} at \
             {CHECK_CELL_MM} (diff {:.4})",
            p99_fine - p99_main
        );
        json!({
            "window": k,
            "cell_mm": CHECK_CELL_MM,
            "cell_used_mm": run.cell_used,
            "secs": run.secs,
            "stats": st,
            "p99_main_cell": p99_main,
            "p99_diff_fine_minus_main": p99_fine - p99_main,
        })
    });

    let doc = json!({
        "arm": arm_name,
        "arm_spec": format!("{arm:?}"),
        "refused": false,
        "plan": "planning/tier_trial_2026-10-01/PLAN.md",
        "fixture": "planning/fixtures/rivmap100/rivmap100_memory_repro.toml",
        "tools": {
            "R1": R1_TOOL,
            "R2": { "id": r2, "source": "planning/fixtures/rivmap100/rivmap100_tiered_finish.toml tool 12" },
            "H1": h1.map(|id| json!({
                "id": id,
                "hypothetical": true,
                "source": "crates/rs_cam_core/tests/fixtures/wanaka100/wanaka_full_tuned.toml tool 12, diameter and shank 6.35",
            })),
        },
        "notes": notes,
        "suggest": feeds,
        "frames": {
            "model_bbox_world": [[mbb.min.x, mbb.min.y, mbb.min.z], [mbb.max.x, mbb.max.y, mbb.max.z]],
            "stock_bbox_world": [[stock.min.x, stock.min.y, stock.min.z], [stock.max.x, stock.max.y, stock.max.z]],
            "window_rule": "centre = model_bbox.min + 350*(k+0.5)/4 in x and y; world frame; identity setup",
            "window_z": [stock.min.z, stock.max.z],
        },
        "population": {
            "rule": "inside silhouette_machining_outline(model_silhouette(mesh, 0.5 mm)) inset 3 mm by offset_polygon",
            "inset_area_mm2": pop.area_mm2,
            "silhouette_area_mm2": pop.silhouette_area_mm2,
            "inset_vertex_count": pop.vertex_count,
        "scanline_vs_contains_point": { "checked": checked, "mismatched": mismatched },
        },
        "time": {
            "source": "closing whole-board simulation at 0.25 mm, fixture machine kinematics, default adaptive feed modulation (ConstrainedMax, scale 1.0); trace.toolpath_runtimes[].breakdown.total_s",
            "finish_s": finish_s,
            "face_rough_s": rough_s,
        },
        "ops": ops,
        "board": board,
        "windows": {
            "cell_mm": window_cell,
            "side_mm": WINDOW_MM,
            "pooled": pooled_stats,
            "pooled_gouge_columns_by_first_op": parts
                .iter()
                .zip(&pooled_gouge_by_op)
                .map(|(p, n)| json!({ "id": p.id.0, "name": p.name, "columns": n }))
                .collect::<Vec<_>>(),
            "gouge_attribution_rule": "a column with dev < -0.02 is charged to the first toolpath after which its top was already > 0.02 mm under the model (per-toolpath checkpoints of the window run)",
            "per_window": windows,
        },
        "cell_check": cell_check_doc,
        "map_legend": legend(),
        "quantile_rule": "nearest rank: sorted[ceil(p*n)-1]",
        "wall": {
            "walk_s": walk_secs,
            "plan_simulations": walk_sims,
            "closing_sim_s": close_secs,
            "windows_s": window_secs,
            "total_s": wall.elapsed().as_secs_f64(),
        },
        "peak_rss_kb": { "after_board": rss_after_board, "end": vm_hwm_kb() },
    });
    let path = out_dir.join(format!("{stem}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).expect("write the JSON");
    eprintln!(
        "== {arm_name}: finish {finish_s:.0} s, pooled {}, board >0.30 {:.0} mm2, wall {:.0} s, \
         VmHWM {:?} kB -> {}",
        doc["windows"]["pooled"],
        doc["board"]["area_dev_gt_0_30_mm2"],
        wall.elapsed().as_secs_f64(),
        vm_hwm_kb(),
        path.display()
    );
}

/// Cheap check of the phase-2 arms: load, configure, Suggest, dressup
/// policy, then print every op. Nothing is generated or simulated.
/// `TIER_TRIAL_ARM` picks one arm; unset, it runs P1, P2 and P3.
#[test]
#[ignore = "tier trial config check: configures arms without generating"]
fn tier_trial_350_config_only() {
    let arms: Vec<String> = std::env::var("TIER_TRIAL_ARM").map_or_else(
        |_| vec!["P1".to_owned(), "P2".to_owned(), "P3".to_owned()],
        |a| vec![a],
    );
    let donor = ProjectSession::load(
        &repo_root().join("planning/fixtures/rivmap100/rivmap100_tiered_finish.toml"),
    )
    .expect("load the rivmap100 tiered donor");
    let r2_cfg = donor
        .tools()
        .iter()
        .find(|t| t.id.0 == DONOR_R2_TOOL)
        .expect("the donor's R2.0 taper")
        .clone();
    drop(donor);
    let fixture = repo_root().join("planning/fixtures/rivmap100/rivmap100_memory_repro.toml");
    for arm_name in arms {
        let arm = arm_of(&arm_name);
        let mut session = ProjectSession::load(&fixture).expect("load the x3.5 project");
        let r2 = add_tool(&mut session, r2_cfg.clone());
        let h1 = arm_uses_h1(&arm).then(|| add_tool(&mut session, h1_tool()));
        let mut notes = configure_arm(&mut session, &arm, r2, h1);
        let feeds = apply_suggest(&mut session);
        if let Err(reason) = &feeds {
            eprintln!("== {arm_name}: REFUSED by Suggest: {reason}");
            continue;
        }
        notes.push(planner_dressups(&mut session));
        eprintln!("== {arm_name} ({arm:?})");
        for n in &notes {
            eprintln!("   note: {n}");
        }
        for r in feeds.as_ref().unwrap() {
            let put_back = &r["geometry_put_back_after_suggest"];
            if put_back.as_array().is_some_and(|a| !a.is_empty()) {
                eprintln!("   suggest put back on op {}: {put_back}", r["id"]);
            }
        }
        for setup in session.list_setups() {
            for &idx in &setup.toolpath_indices {
                let tc = &session.toolpath_configs()[idx];
                let (cusp, iso) = match &tc.operation {
                    OperationConfig::Scallop(s) => (Some(s.scallop_height), Some(s.iso_field)),
                    _ => (None, None),
                };
                eprintln!(
                    "   op {:>3} {:<5} {:<14} tool {:>2} h {:?} iso {:?} feed {:?} rpm {:?} \
                     stock {:?} tier {:?} arc {:?}\n        boundary {}\n        pencil {}",
                    tc.id.0,
                    if tc.enabled { "ON" } else { "off" },
                    tc.operation.op_type().kind_str(),
                    tc.tool_id,
                    cusp,
                    iso,
                    tc.operation.feed_rate(),
                    tc.operation.spindle_rpm(),
                    tc.stock_source,
                    tc.planner_origin.as_ref().map(|p| p.tier),
                    tc.dressups.arc_fitting.as_ref().map(|a| a.tolerance),
                    boundary_summary(tc),
                    pencil_summary(&tc.operation),
                );
            }
        }
    }
}
