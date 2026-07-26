//! v3 process-proof cascade harness on wanaka ×2
//! (`planning/unified_v3_design.md` §0.a; `planning/v3_process_proof_prompt.md`).
//!
//! End goal (reframed 2026-07-13): on a SCALED (×2) copy of the wanaka
//! terrain, prove that ONE unified rest-clear finishing cascade — a ball
//! all-over pass ("Op A") followed by a region-level rest-clear ("Op B",
//! `unified_finish`) — beats a single all-over ball-tip scallop ("Op D") on
//! wall-clock at equal COLUMNS quality, with Region spans showing the mix.
//!
//! This slice ships the fixture builder + a cheap smoke test only. The
//! full cascade-vs-all-over A/B scoring (quantitative COLUMNS comparison
//! between the `Cascade` and `AllOverTip` branches) lands in a later
//! slice; the branch-selection and chain-running helpers here are
//! structured for that reuse today.
//!
//! Fixture notes:
//! - the scaled STL and generated project TOML are written at test
//!   runtime under `target/v3_scaled/` — this directory is never
//!   committed, and nothing here writes to it ahead of time;
//! - the source STL (`terrain.stl`, the un-scaled wanaka export) is
//!   user-local under `~/Downloads/wanaka100/`, matching every other
//!   wanaka-derived harness in this crate;
//! - `planning/airrun_2026-06-01/wanaka.toml` is READ ONLY here (copied
//!   from, never written to) — it is a live, user-modified project file.
//!
//! `#[ignore]` — generates a rough pass over a ×2-scale terrain and runs
//! two dexel simulations. Run with:
//! `cargo test -p rs_cam_core --test v3_cascade_ab --release -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{CreaseReference, UnifiedFinishConfig};
use rs_cam_core::session::{ProjectSession, SessionError, SimulationOptions};

/// Linear scale factor applied to the wanaka terrain STL and its stock
/// footprint for the v3 process-proof fixture.
const SCALE: f64 = 2.0;

/// Un-scaled wanaka terrain export. User-local, like every other
/// wanaka-derived harness in this crate (see `p2c_headless_ab_wanaka.rs`,
/// `finish_planner_wanaka_decompose.rs`).
fn source_stl_path() -> PathBuf {
    let path = PathBuf::from("/home/ricky/Downloads/wanaka100/rivmap_export/terrain.stl");
    assert!(
        path.exists(),
        "v3 cascade harness needs the user-local wanaka100 export at {} \
         (not part of the repo — see planning/airrun_2026-06-01/wanaka.toml for the scale-1 sibling)",
        path.display()
    );
    path
}

/// Scratch directory the ×2 fixture (STL + generated project TOML) lives
/// under. Never committed — created fresh (or reused) at test runtime.
fn v3_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("v3_scaled");
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
    dir
}

/// Binary-STL triangle record: 12 bytes normal + 36 bytes vertices (3x
/// f32 xyz each) + 2 bytes attribute byte count = 50 bytes, following an
/// 80-byte header + u32 LE triangle count.
const STL_HEADER_LEN: usize = 84;
const STL_TRIANGLE_LEN: usize = 50;
/// Offset of the first vertex float within a triangle record (past the
/// 12-byte normal).
const STL_VERTS_OFFSET: usize = 12;

/// Write a uniformly ×`SCALE`'d copy of `source_stl_path()` to
/// `target/v3_scaled/terrain_x2.stl`, skipping the rewrite when a
/// same-length output already exists (binary STL length is fully
/// determined by triangle count, so a length match is a safe staleness
/// check here). Normals are untouched — uniform scale doesn't rotate
/// them.
fn ensure_scaled_stl() -> PathBuf {
    let out_path = v3_dir().join("terrain_x2.stl");
    let src_path = source_stl_path();

    let mut bytes = std::fs::read(&src_path)
        .unwrap_or_else(|e| panic!("failed to read source STL {}: {e}", src_path.display()));
    assert!(
        bytes.len() > STL_HEADER_LEN,
        "source STL {} is too small to be a valid binary STL",
        src_path.display()
    );
    let tri_count = u32::from_le_bytes(
        bytes[80..84]
            .try_into()
            .expect("4-byte slice for triangle count"),
    ) as usize;
    let expected_len = STL_HEADER_LEN + tri_count * STL_TRIANGLE_LEN;
    assert_eq!(
        bytes.len(),
        expected_len,
        "source STL {} length doesn't match a plain binary-STL layout \
         (got {} bytes, expected {} for {} triangles) — is it ASCII STL?",
        src_path.display(),
        bytes.len(),
        expected_len,
        tri_count
    );

    if let Ok(meta) = std::fs::metadata(&out_path)
        && meta.len() as usize == expected_len
    {
        eprintln!(
            "v3 fixture: {} already up to date ({tri_count} triangles), skipping rewrite",
            out_path.display()
        );
        return out_path;
    }

    let scale = SCALE as f32;
    for t in 0..tri_count {
        let verts_start = STL_HEADER_LEN + t * STL_TRIANGLE_LEN + STL_VERTS_OFFSET;
        for v in 0..9 {
            let off = verts_start + v * 4;
            let f = f32::from_le_bytes(
                bytes[off..off + 4]
                    .try_into()
                    .expect("4-byte slice for vertex float"),
            );
            bytes[off..off + 4].copy_from_slice(&(f * scale).to_le_bytes());
        }
    }

    std::fs::write(&out_path, &bytes)
        .unwrap_or_else(|e| panic!("failed to write scaled STL {}: {e}", out_path.display()));
    eprintln!(
        "v3 fixture: wrote {} ({tri_count} triangles, x{SCALE})",
        out_path.display()
    );
    out_path
}

/// Write (overwriting any prior copy) a minimal 4-op project TOML over
/// the ×2-scaled wanaka terrain: one rough (`adaptive3d`, Ø6 end mill)
/// feeding a 3-way finish choice — the cascade pair of Op A (scallop,
/// ball `ball_diameter_mm`) and Op B (`unified_finish`, rest-clear with
/// the 1mm tapered-ball tip), vs Op D (scallop, same tip tool) as the
/// all-over baseline. `apply_branch` / `set_enabled_by_name` pick which
/// subset is enabled for a given run. Machine/post/tool blocks are
/// copied verbatim from `planning/airrun_2026-06-01/wanaka.toml`
/// (read-only reference, never written here).
fn write_fixture_project(ball_diameter_mm: f64) -> PathBuf {
    let stl_path = ensure_scaled_stl();
    let stl_path_str = stl_path.to_string_lossy();
    let out_path = v3_dir().join(format!("wanaka_x2_ball{ball_diameter_mm:.0}.toml"));

    let toml = format!(
        r#"format_version = 3
toolpaths = []

[job]
name = "wanaka_x2_cascade"

[job.stock]
x = 210.0
y = 210.0
z = 14.0
origin_x = -5.0
origin_y = -5.0
origin_z = -5.0
padding = 0.0
workholding_rigidity = "Medium"
auto_from_model = false

[job.stock.material.SolidWood]
species = "GenericHardwood"

[job.post]
format = "grbl"
spindle_speed = 18000
safe_z = 10.0
high_feedrate_mode = false
high_feedrate = 5000.0
spindle_strategy = "match_chart"

[job.machine]
name = "Shapeoko Pro XXL"
max_feed_mm_min = 10000.0
max_shank_mm = 6.35
safety_factor = 0.75

[job.machine.spindle.Variable]
min_rpm = 8000.0
max_rpm = 24000.0

[job.machine.power.ConstantPower]
power_kw = 0.8

[job.machine.chip_load]
k0 = 0.024
p = 0.61
q = 1.26

[job.machine.rigidity]
doc_roughing_factor = 0.2
doc_finishing_factor = 0.08
woc_roughing_factor = 0.7
woc_roughing_max_mm = 5.0
woc_finishing_mm = 0.5
adaptive_doc_factor = 1.5
adaptive_woc_factor = 0.2

[job.machine.kinematics]
acceleration_mm_s2 = 350.0
junction_deviation_mm = 0.01

[[tools]]
id = 3
name = "End Mill"
type = "end_mill"
diameter = 6.0
cutting_length = 25.0
helix_deg = 30.0
corner_radius_mm = 0.0
corner_radius = 2.0
included_angle = 90.0
taper_half_angle = 15.0
shaft_diameter = 6.35
holder_diameter = 25.0
shank_diameter = 6.35
shank_length = 20.0
stickout = 45.0
flute_count = 2
tool_number = 3
tool_material = "carbide"
cut_direction = "up_cut"
vendor = ""
product_id = ""

[[tools]]
id = 2
name = "Tapered Ball 2mm tip / 7° / 6mm shank"
type = "tapered_ball_nose"
diameter = 1.0
cutting_length = 25.0
helix_deg = 30.0
corner_radius_mm = 0.0
corner_radius = 2.0
included_angle = 90.0
taper_half_angle = 7.0
shaft_diameter = 6.0
holder_diameter = 25.0
shank_diameter = 6.0
shank_length = 20.0
stickout = 35.0
flute_count = 2
tool_number = 2
tool_material = "carbide"
cut_direction = "up_cut"
vendor = ""
product_id = ""

[[tools]]
id = 20
name = "Ball Ø{ball_diameter_mm:.1} 3F"
type = "ball_nose"
diameter = {ball_diameter_mm:.1}
cutting_length = 12.0
helix_deg = 30.0
corner_radius_mm = 0.0
corner_radius = 0.0
included_angle = 90.0
taper_half_angle = 15.0
shaft_diameter = 6.0
holder_diameter = 25.0
shank_diameter = 6.0
shank_length = 20.0
stickout = 27.0
flute_count = 3
tool_number = 20
tool_material = "carbide"
cut_direction = "up_cut"
vendor = ""
product_id = ""

[[models]]
id = 1
path = "{stl_path_str}"
name = "terrain_x2.stl"
kind = "stl"

[models.units]
kind = "millimeters"

[[setups]]
id = 0
name = "Setup 1"
face_up = "top"
z_rotation = "0"
fixtures = []
keep_out_zones = []

[[setups.toolpaths]]
id = 1
name = "Rough"
type = "adaptive3d"
enabled = true
tool_id = 3
model_id = 1
boundary_inherit = false
stock_source = "fresh"
coolant = "off"

[setups.toolpaths.operation]
kind = "adaptive3d"

[setups.toolpaths.operation.params]
stepover = 2.2
depth_per_pass = 2.6
stock_to_leave_radial = 0.5
stock_to_leave_axial = 0.5
feed_rate = 4000.0
plunge_rate = 500.0
tolerance = 0.1
min_cutting_radius = 0.0
entry_style = "plunge"
ramp_angle_deg = 10.0
helix_radius_factor = 0.3
helix_pitch = 2.0
fine_stepdown = 0.0
detect_flat_areas = false
region_ordering = "global"
clearing_strategy = "agent_search"
trochoid_cap_mult = 1.6
engagement_measure = "DiskArea"
z_blend = false
spindle_rpm = 12194
mill_shallow_areas = false
min_region_cut_length_mm = 15.0
stay_down_clearance_mm = 0.5

[setups.toolpaths.dressups]
entry_style = "none"
ramp_angle = 3.0
helix_radius = 2.0
helix_pitch = 1.0
dogbone = false
dogbone_angle = 90.0
lead_in_out = false
lead_radius = 2.0
link_moves = true
link_max_distance = 10.0
link_feed_rate = 500.0
arc_fitting = true
arc_tolerance = 0.05
segment_merge = false
segment_merge_tolerance = 0.3
feed_optimization = false
feed_max_rate = 3000.0
feed_ramp_rate = 200.0
optimize_rapid_order = true
retract_strategy = "minimum"

[setups.toolpaths.heights.clearance_z]
mode = "auto"

[setups.toolpaths.heights.retract_z]
mode = "auto"

[setups.toolpaths.heights.feed_z]
mode = "auto"

[setups.toolpaths.heights.top_z]
mode = "auto"

[setups.toolpaths.heights.bottom_z]
mode = "auto"

[setups.toolpaths.boundary]
enabled = true
source = "model_silhouette"
containment = "inside"
offset = 0.0

[setups.toolpaths.debug_options]
enabled = false

[[setups.toolpaths]]
id = 2
name = "Op A Ball Finish"
type = "scallop"
enabled = true
tool_id = 20
model_id = 1
boundary_inherit = false
stock_source = "from_remaining_stock"
coolant = "off"

[setups.toolpaths.operation]
kind = "scallop"

[setups.toolpaths.operation.params]
scallop_height = 0.011
tolerance = 0.05
direction = "outside_in"
continuous = false
slope_from = 0.0
slope_to = 90.0
feed_rate = 3000.0
plunge_rate = 150.0
stock_to_leave = 0.0
spindle_rpm = 21000

[setups.toolpaths.dressups]
entry_style = "none"
ramp_angle = 3.0
helix_radius = 2.0
helix_pitch = 1.0
dogbone = false
dogbone_angle = 90.0
lead_in_out = false
lead_radius = 2.0
link_moves = false
link_max_distance = 10.0
link_feed_rate = 500.0
arc_fitting = true
arc_tolerance = 0.05
segment_merge = false
segment_merge_tolerance = 0.3
feed_optimization = true
feed_max_rate = 3000.0
feed_ramp_rate = 200.0
optimize_rapid_order = true
retract_strategy = "full"

[setups.toolpaths.heights.clearance_z]
mode = "auto"

[setups.toolpaths.heights.retract_z]
mode = "auto"

[setups.toolpaths.heights.feed_z]
mode = "auto"

[setups.toolpaths.heights.top_z]
mode = "auto"

[setups.toolpaths.heights.bottom_z]
mode = "auto"

[setups.toolpaths.boundary]
enabled = true
source = "model_silhouette"
containment = "inside"
offset = 0.0

[setups.toolpaths.debug_options]
enabled = false

[[setups.toolpaths]]
id = 3
name = "Op B Unified Rest"
type = "unified_finish"
enabled = true
tool_id = 2
model_id = 1
boundary_inherit = false
stock_source = "from_remaining_stock"
coolant = "off"

[setups.toolpaths.operation]
kind = "unified_finish"

[setups.toolpaths.operation.params]
steep_threshold_deg = 45.0
waterline_threshold_deg = 75.0
overlap_mm = 2.0
scallop_height = 0.011
tolerance = 0.05
raster_stepover = 0.21
z_step = 0.3
sampling = 0.5
stock_to_leave = 0.0
feed_rate = 3000.0
plunge_rate = 150.0
spindle_rpm = 21000
pencil_claims = false
min_rest_depth_mm = 0.02

[setups.toolpaths.dressups]
entry_style = "none"
ramp_angle = 3.0
helix_radius = 2.0
helix_pitch = 1.0
dogbone = false
dogbone_angle = 90.0
lead_in_out = false
lead_radius = 2.0
link_moves = false
link_max_distance = 10.0
link_feed_rate = 500.0
arc_fitting = true
arc_tolerance = 0.05
segment_merge = false
segment_merge_tolerance = 0.3
feed_optimization = true
feed_max_rate = 3000.0
feed_ramp_rate = 200.0
optimize_rapid_order = true
retract_strategy = "full"

[setups.toolpaths.heights.clearance_z]
mode = "auto"

[setups.toolpaths.heights.retract_z]
mode = "auto"

[setups.toolpaths.heights.feed_z]
mode = "auto"

[setups.toolpaths.heights.top_z]
mode = "auto"

[setups.toolpaths.heights.bottom_z]
mode = "auto"

[setups.toolpaths.boundary]
enabled = true
source = "model_silhouette"
containment = "inside"
offset = 0.0

[setups.toolpaths.debug_options]
enabled = false

[[setups.toolpaths]]
id = 4
name = "D All-Over Tip"
type = "scallop"
enabled = false
tool_id = 2
model_id = 1
boundary_inherit = false
stock_source = "from_remaining_stock"
coolant = "off"

[setups.toolpaths.operation]
kind = "scallop"

[setups.toolpaths.operation.params]
scallop_height = 0.011
tolerance = 0.05
direction = "outside_in"
continuous = false
slope_from = 0.0
slope_to = 90.0
feed_rate = 3000.0
plunge_rate = 150.0
stock_to_leave = 0.0
spindle_rpm = 21000

[setups.toolpaths.dressups]
entry_style = "none"
ramp_angle = 3.0
helix_radius = 2.0
helix_pitch = 1.0
dogbone = false
dogbone_angle = 90.0
lead_in_out = false
lead_radius = 2.0
link_moves = false
link_max_distance = 10.0
link_feed_rate = 500.0
arc_fitting = true
arc_tolerance = 0.05
segment_merge = false
segment_merge_tolerance = 0.3
feed_optimization = true
feed_max_rate = 3000.0
feed_ramp_rate = 200.0
optimize_rapid_order = true
retract_strategy = "full"

[setups.toolpaths.heights.clearance_z]
mode = "auto"

[setups.toolpaths.heights.retract_z]
mode = "auto"

[setups.toolpaths.heights.feed_z]
mode = "auto"

[setups.toolpaths.heights.top_z]
mode = "auto"

[setups.toolpaths.heights.bottom_z]
mode = "auto"

[setups.toolpaths.boundary]
enabled = true
source = "model_silhouette"
containment = "inside"
offset = 0.0

[setups.toolpaths.debug_options]
enabled = false
"#
    );

    std::fs::write(&out_path, toml).unwrap_or_else(|e| {
        panic!(
            "failed to write fixture project {}: {e}",
            out_path.display()
        )
    });
    out_path
}

// ── branch selection (structured now, exercised by the A/B slice) ──────

/// Which finishing path a chain run takes (`planning/unified_v3_design.md`
/// §0.a). Exercised by `score_branch` below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Branch {
    /// Op A (ball all-over scallop) + Op B (unified_finish rest-clear).
    Cascade,
    /// Op D only: a single all-over scallop pass with the tip tool.
    AllOverTip,
}

/// Set toolpath `enabled` by config NAME: every name in `enable` is
/// turned on, every name in `disable` is turned off; toolpaths named in
/// neither list are left as-is. Matching by name (not index) because the
/// fixture's op order is stable but this helper is meant to be reusable
/// against hand-edited copies too.
fn set_enabled_by_name(s: &mut ProjectSession, enable: &[&str], disable: &[&str]) {
    let n = s.toolpath_count();
    for i in 0..n {
        let name = s
            .get_toolpath_config(i)
            .expect("toolpath config")
            .name
            .clone();
        if enable.contains(&name.as_str()) {
            s.set_toolpath_enabled(i, true)
                .unwrap_or_else(|e| panic!("enable '{name}': {e}"));
        } else if disable.contains(&name.as_str()) {
            s.set_toolpath_enabled(i, false)
                .unwrap_or_else(|e| panic!("disable '{name}': {e}"));
        }
    }
}

/// Apply a `Branch` to the fixture's 4-op chain by toolpath name.
fn apply_branch(s: &mut ProjectSession, branch: Branch) {
    let (enable, disable): (&[&str], &[&str]) = match branch {
        Branch::Cascade => (
            &["Rough", "Op A Ball Finish", "Op B Unified Rest"],
            &["D All-Over Tip"],
        ),
        Branch::AllOverTip => (
            &["Rough", "D All-Over Tip"],
            &["Op A Ball Finish", "Op B Unified Rest"],
        ),
    };
    set_enabled_by_name(s, enable, disable);
}

// ── chain runner ────────────────────────────────────────────────────────

/// Totals from one full `run_chain` pass. `per_op_s` is what `score_branch`
/// sums over to get a branch's "finish stack" seconds (rather than pinning
/// a single "finish op" name the way `p2c_headless_ab_wanaka.rs` does — the
/// cascade branch has two finish ops, not one).
struct ChainOutcome {
    project_total_s: f64,
    per_op_s: Vec<(String, f64)>,
    collisions: usize,
    project_removed_mm3: f64,
}

/// Generate every ENABLED op (F.4 ladder: rest ops fail hard from fresh
/// state until an upstream simulation makes their stock available), run
/// a final simulation with `adaptive_feed_modulation` at
/// `ConstrainedMax`/1.0 aggressiveness (matching every other wanaka
/// harness in this crate), print the per-op runtime-by-intent table, and
/// return the totals. Adapted from `p2c_headless_ab_wanaka.rs::run_chain`,
/// generalized to accumulate per-op seconds by name instead of pinning
/// one "finish" op.
fn run_chain(label: &str, s: &mut ProjectSession) -> ChainOutcome {
    let cancel = AtomicBool::new(false);
    let n = s.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();

    // Pass 1: rest ops fail hard from fresh state by design (F.4).
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }

    // F.4 ladder: each simulation unlocks the first pending rest op.
    let mut ladder_rounds = 0usize;
    while !pending.is_empty() {
        ladder_rounds += 1;
        assert!(
            ladder_rounds <= enabled.len() + 2,
            "[{label}] ladder failed to converge; still pending: {pending:?}"
        );
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder simulation");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(
            pending.len() < before,
            "[{label}] ladder made no progress at round {ladder_rounds}; still pending: {pending:?}"
        );
    }

    let final_opts = SimulationOptions {
        adaptive_feed_modulation: true,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
        ..Default::default()
    };
    s.run_simulation(&final_opts, &cancel)
        .expect("final simulation");
    let sim = s.simulation_result().expect("sim result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");

    eprintln!("== v3 cascade A/B [{label}] ==");
    eprintln!("rapid_collisions={}", sim.rapid_collisions.len());

    let mut per_op_s: Vec<(String, f64)> = Vec::new();
    for tp in &trace.toolpath_summaries {
        let name = (0..n)
            .filter_map(|i| s.get_toolpath_config(i))
            .find(|tc| tc.id == tp.toolpath_id)
            .map(|tc| tc.name.clone())
            .unwrap_or_else(|| format!("{:?}", tp.toolpath_id));
        per_op_s.push((name.clone(), tp.total_runtime_s));
        match tp.runtime_by_intent {
            Some(b) => eprintln!(
                "op={name:<22} total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1} removed={:9.0}mm3",
                tp.total_runtime_s,
                b.cutting_s,
                b.entry_s,
                b.linking_s,
                b.rapid_s,
                b.retract_s,
                b.unknown_s,
                tp.total_removed_volume_est_mm3
            ),
            None => eprintln!(
                "op={name:<22} total={:8.1}s removed={:9.0}mm3 (no runtime_by_intent — kinematics off?)",
                tp.total_runtime_s, tp.total_removed_volume_est_mm3
            ),
        }
    }

    let project_total_s = trace.summary.total_runtime_s;
    let project_removed_mm3 = trace.summary.total_removed_volume_est_mm3;
    if let Some(p) = trace.summary.runtime_by_intent {
        eprintln!(
            "PROJECT total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1} removed={:9.0}mm3",
            project_total_s,
            p.cutting_s,
            p.entry_s,
            p.linking_s,
            p.rapid_s,
            p.retract_s,
            p.unknown_s,
            project_removed_mm3
        );
    }

    ChainOutcome {
        project_total_s,
        per_op_s,
        collisions: sim.rapid_collisions.len(),
        project_removed_mm3,
    }
}

// ── smoke test ───────────────────────────────────────────────────────────

#[test]
#[ignore = "scaled-wanaka fixture smoke (one rough generation + sims); run with --ignored --nocapture"]
fn v3_fixture_smoke() {
    ensure_scaled_stl();
    let project_path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));

    assert_eq!(
        s.toolpath_count(),
        4,
        "expected exactly 4 toolpaths in the v3 fixture chain"
    );

    // Model bbox sanity: source terrain is (0,0,-2.033)..(100,100,3.947),
    // so ×SCALE should read ~200x200 XY, z ~-4.066..7.894.
    let bboxes = s.collect_model_bboxes();
    assert_eq!(bboxes.len(), 1, "expected exactly 1 model in the fixture");
    let (_, bbox) = &bboxes[0];
    let span_x = bbox.max.x - bbox.min.x;
    let span_y = bbox.max.y - bbox.min.y;
    assert!(
        (span_x - 100.0 * SCALE).abs() < 0.1,
        "scaled model X span {span_x} != {}",
        100.0 * SCALE
    );
    assert!(
        (span_y - 100.0 * SCALE).abs() < 0.1,
        "scaled model Y span {span_y} != {}",
        100.0 * SCALE
    );
    assert!(
        (bbox.min.z - (-4.066)).abs() < 0.1,
        "scaled model min Z {} != -4.066",
        bbox.min.z
    );
    assert!(
        (bbox.max.z - 7.894).abs() < 0.1,
        "scaled model max Z {} != 7.894",
        bbox.max.z
    );

    // Only the rough op for this smoke — no finish generation needed to
    // prove the fixture loads and cuts.
    set_enabled_by_name(
        &mut s,
        &["Rough"],
        &["Op A Ball Finish", "Op B Unified Rest", "D All-Over Tip"],
    );

    let outcome = run_chain("x2 smoke rough-only", &mut s);
    assert_eq!(
        outcome.collisions, 0,
        "rough-only chain should have zero rapid collisions"
    );
    assert!(
        outcome.project_total_s > 0.0,
        "expected nonzero project runtime"
    );
    assert!(
        outcome.project_removed_mm3 > 0.0,
        "expected nonzero removed volume"
    );

    eprintln!(
        "FIXTURE OK: v3 scaled-wanaka loads, rough generates, 0 collisions, {:.1}s / {:.0}mm3 removed",
        outcome.project_total_s, outcome.project_removed_mm3
    );
}

// ── Op B cascade dials (design doc §0.a item 4) ─────────────────────────

/// The cascade Op B config: `unified_finish` running in rest-clearer mode
/// against Op A's own machined (ball all-over) stock. Every dial choice
/// below is pinned to something measured or specified, not a guess dressed
/// up as one:
///
/// - `steep_threshold_deg`/`waterline_threshold_deg` (45/75): the P2.e-locked
///   band thresholds every other harness in this crate uses
///   (`ab_unified_config` in `p2c_headless_ab_wanaka.rs`) — no reason to
///   deviate for the cascade's rest-clear pass.
/// - `overlap_mm` 2.0: same band-overlap convention as P2.e/S1.
/// - `scallop_height` 0.011 / `tolerance` 0.05: Op A's own cusp target
///   (see `write_fixture_project`'s Op A params) — Op B must not visibly
///   under- or over-cut relative to the pass it's cleaning up after.
/// - `raster_stepover` 0.21: cusp-consistent for a Ø1 TIP ball
///   (`sqrt(8 * r * h)` with r=0.5mm, h=0.011mm ≈ 0.2098mm) — the shallow
///   raster strategy's horizontal stepover that reproduces the SAME cusp
///   the scallop height targets, so Op B's raster and scallop bands don't
///   silently disagree on quality the way an arbitrary stepover would.
/// - `z_step` 0.3: wall-spacing parity with every other harness's waterline
///   dial in this crate.
/// - `sampling` 0.5 / `stock_to_leave` 0.0: standard finish-pass values.
/// - `feed_rate`/`plunge_rate`/`spindle_rpm`: copied verbatim from the
///   fixture's Op A and Op B TOML blocks (`write_fixture_project`) so the
///   cascade A/B doesn't introduce a feeds/speeds confound alongside the
///   claims-pipeline one.
/// - `pencil_claims` TRUE: this is the whole point of the cascade — Op B
///   must run its rest detector and claim creases, unlike every prior A/B
///   in this crate which pins it OFF as the pre-v3 baseline.
/// - `min_rest_depth_mm` 0.022 (≈ 2× Op A's 0.011mm cusp, design doc §2.1
///   step 4's sizing note): territory below Op A's own cusp is Op A's
///   noise floor, not real rest material for Op B to chase.
/// - `min_region_rest_share` 0.10: S2's region-level territory filter —
///   drop whole conditioned islands whose measured rest is ≤10% of the
///   island area. First dial guess (design doc §0.a build list); the tail
///   gate in `v3_cascade_ab_ball3` is the safety net if this is too
///   aggressive.
/// - `claims_reference` `MachinedStock`: sanctioned here specifically
///   because Op B follows Op A's OWN ball all-over pass — a genuinely
///   finish-quality reference, not a rough-chain terrace field (the S1
///   lesson that ruled this reference out for wanaka.toml's rough→finish
///   chain).
/// - `territory_clip` TRUE (S4): intersect surviving band regions against
///   the detector's rest-region polygons so Op B generates only over real
///   rest material. S2's whole-island keep-or-drop couldn't shrink a giant
///   conditioned island that merely CONTAINS >10% rest somewhere — the
///   first wanaka ×2 cascade A/B measured Op B at +47% over the all-over
///   baseline for exactly that reason (see the `UnifiedFinishConfig::
///   territory_clip` field doc). Sanctioned here because
///   `claims_reference` is `MachinedStock` (the clip is skipped with a
///   warning under `SelfProbe`).
fn op_b_claims_config() -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 2.0,
        scallop_height: 0.011,
        tolerance: 0.05,
        raster_stepover: 0.21,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        spindle_rpm: Some(21000),
        pencil_claims: true,
        min_rest_depth_mm: 0.022,
        min_region_rest_share: 0.10,
        claims_reference: CreaseReference::MachinedStock,
        territory_clip: true,
    }
}

/// Resolve a toolpath index by config NAME (panics if absent) — used by
/// `score_branch` to target "Op B Unified Rest" / "D All-Over Tip"
/// regardless of their fixed indices in `write_fixture_project`.
fn toolpath_index_by_name(s: &ProjectSession, name: &str) -> usize {
    (0..s.toolpath_count())
        .find(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.name == name))
        .unwrap_or_else(|| panic!("no toolpath named '{name}' in the fixture"))
}

// ── fidelity instrument (adapted from `p2c_headless_ab_wanaka.rs`) ─────
//
// Duplicated locally rather than shared — `p2c_headless_ab_wanaka.rs`'s
// pieces are file-local (per the design prompt's explicit instruction not
// to refactor that harness). Two differences from the p2c original:
// output lands under `target/v3_scaled/` (this fixture's scratch dir,
// `v3_dir()`) instead of `target/p2f_fidelity/`, and the classification
// cutter/planner mirror Op B's OWN Ø1-tip dials (`for_tool(0.5)`) instead
// of p2c's Ø6-ball convention — the territory being measured here is
// whatever Op B actually routes, not a bulk-ball's.

/// Per-cell band ownership rasterized from the planner's conditioned
/// regions onto the classification grid. Code 0 = no region (off-model or
/// unclassified), 1 = Shallow, 2 = MidSteep, 3 = VerySteep.
struct BandMap {
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    rows: usize,
    cols: usize,
    codes: Vec<u8>,
}

const BAND_NAMES: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];

impl BandMap {
    fn code_at(&self, x: f64, y: f64) -> u8 {
        let col = ((x - self.origin_x) / self.cell).round();
        let row = ((y - self.origin_y) / self.cell).round();
        if col < 0.0 || row < 0.0 || col >= self.cols as f64 || row >= self.rows as f64 {
            return 0;
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let idx = row as usize * self.cols + col as usize;
        self.codes[idx]
    }
}

/// Build the band map with the SAME classification + decomposition dials
/// Op B's cascade config uses (Ø1 tip -> `tool_radius` 0.5, `overlap_mm`
/// 2.0, locked 45/75 thresholds via `for_tool`), so deviations are
/// attributed to the regions Op B actually routes. The D branch is scored
/// against the same map — the comparison question is "what did each
/// strategy's territory look like", so the territory definition must be
/// identical across branches.
fn build_band_map(s: &ProjectSession) -> BandMap {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::geo::P2;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::tool::BallEndmill;

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("v3 scaled terrain mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(1.0, 25.0);
    let never = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &never)
        .expect("classification surface");
    let mut planner = FinishPlannerParams::for_tool(0.5);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        &surface.heightmap.covered,
        &[],
        0.5,
        &planner,
    );

    let hm = &surface.heightmap;
    let (rows, cols, cell) = (hm.rows, hm.cols, hm.cell_size);
    let mut codes = vec![0u8; rows * cols];
    for (band, code) in [
        (FinishBand::Shallow, 1u8),
        (FinishBand::MidSteep, 2u8),
        (FinishBand::VerySteep, 3u8),
    ] {
        let polys: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .map(|r| r.polygon.clone())
            .collect();
        if polys.is_empty() {
            continue;
        }
        let rs = RegionSet::new(polys);
        for r in 0..rows {
            for c in 0..cols {
                let x = hm.origin_x + c as f64 * cell;
                let y = hm.origin_y + r as f64 * cell;
                if rs.contains(&P2::new(x, y)) {
                    codes[r * cols + c] = code;
                }
            }
        }
    }

    let counts = codes.iter().fold([0usize; 4], |mut acc, &c| {
        acc[c as usize] += 1;
        acc
    });
    eprintln!(
        "BAND MAP {rows}x{cols} @ {cell:.3}mm: off-region={} shallow={} mid-steep={} very-steep={}",
        counts[0], counts[1], counts[2], counts[3]
    );

    // Coverage accounting (the 25%-coverage anomaly, 2026-07-13): how much
    // of the classification grid is covered at all, vs how much the
    // conditioned polygons reclaim of it.
    let covered_n = hm.covered.iter().filter(|&&c| c).count();
    let mut band_stats = String::new();
    for band in [
        FinishBand::Shallow,
        FinishBand::MidSteep,
        FinishBand::VerySteep,
    ] {
        let (n, area): (usize, f64) = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .fold((0, 0.0), |(n, a), r| (n + 1, a + r.polygon.area()));
        band_stats.push_str(&format!(" {band:?}: {n} regions {area:.0}mm2;"));
    }
    eprintln!(
        "BAND COVERAGE: covered={covered_n}/{} ({:.1}%) | planned:{band_stats} decompose stats: {:?}",
        rows * cols,
        100.0 * covered_n as f64 / (rows * cols) as f64,
        planned.stats
    );

    BandMap {
        origin_x: hm.origin_x,
        origin_y: hm.origin_y,
        cell,
        rows,
        cols,
        codes,
    }
}

/// Histogram bin edges (mm), identical to `p2c_headless_ab_wanaka.rs`.
/// Negative = OVERCUT, positive = leftover material.
const DEV_EDGES: [f32; 12] = [
    -0.5, -0.3, -0.2, -0.1, -0.05, -0.01, 0.01, 0.05, 0.1, 0.2, 0.3, 0.5,
];
const DEV_BIN_COUNT: usize = DEV_EDGES.len() + 1;
const DEV_BIN_LABELS: [&str; DEV_BIN_COUNT] = [
    "<-.5", "-.5", "-.3", "-.2", "-.1", "-.05", "on-size", "+.05", "+.1", "+.2", "+.3", "+.5",
    ">+.5",
];

#[derive(Default, Clone, Copy)]
struct BandAcc {
    bins: [usize; DEV_BIN_COUNT],
    leftover_n: usize,
    leftover_sum: f64,
    leftover_max: f32,
    overcut_n: usize,
    overcut_sum: f64,
    overcut_min: f32,
}

/// Re-simulate the chain at measurement resolution (0.25 mm — the default
/// 0.5 mm dexel grid aliases away exactly the terrain texture the
/// band-fidelity defect beheads; see `p2c_headless_ab_wanaka.rs`). Run
/// AFTER `run_chain` so the reported times/collisions come from the
/// standard chain options; this sim exists only to populate `deviations`
/// and `column_deviations`.
fn run_measurement_sim(s: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.25,
        ..Default::default()
    };
    s.run_simulation(&opts, &cancel)
        .expect("hi-res measurement simulation");
}

/// Per-band deviation report for the CURRENT simulation result on `s`. Call
/// after `run_chain` + `run_measurement_sim`. Trims `p2c_headless_ab_wanaka.rs`'s
/// original down to the two pieces the design doc (§4) calls load-bearing:
/// a top-down deviation PNG (vertex-based, saturated at ±0.3mm) for visual
/// sanity, and the group-filtered FIDELITY-COLUMNS table — pointwise dexel
/// tops, the ONLY instrument the A/B's quality gates read (P2.g Task 1:
/// corner-averaged vertex heights filter phase-coherent machined texture
/// away, producing a fake branch-dependent shift).
fn fidelity_report(tag: &str, s: &ProjectSession, bm: &BandMap) {
    let sim = s.simulation_result().expect("sim result");
    const EPS: f32 = 1e-4;

    // ── deviation PNG (vertex-based, visual only) ──────────────────────
    if let Some(devs) = sim.deviations.as_ref() {
        let verts = &sim.mesh.vertices;
        assert_eq!(verts.len(), devs.len() * 3, "vertex/deviation mismatch");

        let icell = bm.cell * 0.5;
        let icols = bm.cols * 2;
        let irows = bm.rows * 2;
        let mut img: Vec<f32> = vec![0.0; irows * icols];
        let mut img_hit: Vec<bool> = vec![false; irows * icols];
        for (i, &d) in devs.iter().enumerate() {
            if d == 0.0 {
                continue; // sentinel: vertex not relevant (stock bottom etc.)
            }
            let x = f64::from(verts[i * 3]);
            let y = f64::from(verts[i * 3 + 1]);
            let col = ((x - bm.origin_x) / icell).round();
            let row = ((y - bm.origin_y) / icell).round();
            if col >= 0.0 && row >= 0.0 && col < icols as f64 && row < irows as f64 {
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let idx = row as usize * icols + col as usize;
                if !img_hit[idx] || d.abs() > img[idx].abs() {
                    img[idx] = d;
                    img_hit[idx] = true;
                }
            }
        }
        let mut px = vec![0u8; irows * icols * 4];
        for r in 0..irows {
            for c in 0..icols {
                let idx = r * icols + c;
                let (rr, gg, bb) = if !img_hit[idx] {
                    (0u8, 0u8, 0u8)
                } else {
                    let d = img[idx];
                    if d < -EPS {
                        let t = (f64::from(-d) / 0.3).min(1.0);
                        let g = (110.0 * (1.0 - t)) as u8;
                        ((110.0 + 145.0 * t) as u8, g, g)
                    } else if d > EPS {
                        let t = (f64::from(d) / 0.3).min(1.0);
                        let g = (110.0 * (1.0 - t)) as u8;
                        (g, g, (110.0 + 145.0 * t) as u8)
                    } else {
                        (110u8, 110u8, 110u8)
                    }
                };
                let ir = irows - 1 - r;
                let i = (ir * icols + c) * 4;
                px[i] = rr;
                px[i + 1] = gg;
                px[i + 2] = bb;
                px[i + 3] = 255;
            }
        }
        let dev_path = v3_dir().join(format!("{tag}_deviation.png"));
        image::save_buffer(
            &dev_path,
            &px,
            icols as u32,
            irows as u32,
            image::ColorType::Rgba8,
        )
        .expect("save deviation png");
        eprintln!(
            "deviation png: {} (red=overcut, blue=leftover, sat +-0.3mm)",
            dev_path.display()
        );
    } else {
        eprintln!("[{tag}] vertex deviations unavailable (sim ran without a reference mesh?)");
    }

    // ── FIDELITY-COLUMNS, group-filtered (the gate-relevant table) ─────
    let Some(cols) = sim.column_deviations.as_ref() else {
        eprintln!("[{tag}] column deviations unavailable");
        return;
    };
    let bin_of = |d: f32| -> usize {
        DEV_EDGES
            .iter()
            .position(|&e| d < e)
            .unwrap_or(DEV_BIN_COUNT - 1)
    };
    let max_group = cols.iter().map(|cd| cd.group).max().unwrap_or(0);
    for group in 0..=max_group {
        let mut caccs = [BandAcc::default(); 4];
        for cd in cols.iter().filter(|cd| cd.group == group) {
            let code = bm.code_at(cd.x, cd.y) as usize;
            let a = &mut caccs[code];
            a.bins[bin_of(cd.dev)] += 1;
            if cd.dev > EPS {
                a.leftover_n += 1;
                a.leftover_sum += f64::from(cd.dev);
                a.leftover_max = a.leftover_max.max(cd.dev);
            } else if cd.dev < -EPS {
                a.overcut_n += 1;
                a.overcut_sum += f64::from(cd.dev);
                a.overcut_min = a.overcut_min.min(cd.dev);
            }
        }
        eprintln!(
            "== FIDELITY-COLUMNS [{tag}] group {group} (pointwise dexel tops; negative = overcut) =="
        );
        eprintln!(
            "{:<11} | {:>9} {:>9} {:>8} | {:>9} {:>9} {:>8} | histogram",
            "band", "over_n", "over_mean", "worst", "left_n", "left_mean", "max"
        );
        for (code, acc) in caccs.iter().enumerate() {
            let over_mean = acc.overcut_sum / (acc.overcut_n as f64).max(1.0);
            let left_mean = acc.leftover_sum / (acc.leftover_n as f64).max(1.0);
            let hist: Vec<String> = DEV_BIN_LABELS
                .iter()
                .zip(acc.bins.iter())
                .map(|(l, n)| format!("{l}:{n}"))
                .collect();
            eprintln!(
                "{:<11} | {:>9} {:>9.4} {:>8.4} | {:>9} {:>9.4} {:>8.4} | {}",
                BAND_NAMES[code],
                acc.overcut_n,
                over_mean,
                acc.overcut_min,
                acc.leftover_n,
                left_mean,
                acc.leftover_max,
                hist.join(" ")
            );
        }
    }
}

/// Group-filtered on-size / `+.05` / `>+.5`-tail shares from
/// `sim.column_deviations`, restricted to `group` and to `band_code`
/// (from `BandMap::code_at` — 1=shallow, 2=mid-steep, 3=very-steep).
/// Returns `(sample_count, on_size_pct, plus05_pct, tail_count)`. Mirrors
/// `p2c_headless_ab_wanaka.rs::band_shares`.
fn band_shares(
    s: &ProjectSession,
    bm: &BandMap,
    group: usize,
    band_code: u8,
) -> (usize, f64, f64, usize) {
    let sim = s.simulation_result().expect("sim result");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations (sim ran without a reference model mesh?)");
    let bin_of = |d: f32| -> usize {
        DEV_EDGES
            .iter()
            .position(|&e| d < e)
            .unwrap_or(DEV_BIN_COUNT - 1)
    };
    const ON_SIZE_BIN: usize = 6;
    const PLUS_05_BIN: usize = 7;
    const TAIL_BIN: usize = DEV_BIN_COUNT - 1; // ">+.5" — big standing leftover
    let mut total = 0usize;
    let mut on_size = 0usize;
    let mut plus05 = 0usize;
    let mut tail = 0usize;
    for cd in cols.iter().filter(|cd| cd.group == group) {
        if bm.code_at(cd.x, cd.y) != band_code {
            continue;
        }
        total += 1;
        match bin_of(cd.dev) {
            ON_SIZE_BIN => on_size += 1,
            PLUS_05_BIN => plus05 += 1,
            TAIL_BIN => tail += 1,
            _ => {}
        }
    }
    let pct = |n: usize| 100.0 * n as f64 / (total.max(1) as f64);
    (total, pct(on_size), pct(plus05), tail)
}

/// Region spans on `spans` that are NOT strictly nested inside another
/// Region span — the node-level table (one per band/crease strategy),
/// coarser than any finer scallop-event Region spans a strategy nests
/// inside its own move range. Identical to
/// `p2c_headless_ab_wanaka.rs::outer_region_spans`.
fn outer_region_spans(
    spans: &[rs_cam_core::toolpath_spans::Span],
) -> Vec<&rs_cam_core::toolpath_spans::Span> {
    use rs_cam_core::toolpath_spans::SpanKind;
    let regions: Vec<&rs_cam_core::toolpath_spans::Span> = spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region)
        .collect();
    regions
        .iter()
        .copied()
        .filter(|s| {
            !regions.iter().any(|other| {
                !std::ptr::eq(*other, *s)
                    && other.start_move <= s.start_move
                    && other.end_move >= s.end_move
                    && (other.start_move, other.end_move) != (s.start_move, s.end_move)
            })
        })
        .collect()
}

/// Sum of `FinishingCut` segment lengths (mm) among moves
/// `[start_move, end_move)` — each move's target is the END of a segment
/// starting at the previous move's target, so segment `i`'s length is
/// `dist(moves[i-1], moves[i])` and its intent is `moves[i].intent`.
fn segment_cutting_length_mm(
    moves: &[rs_cam_core::toolpath::Move],
    start_move: usize,
    end_move: usize,
) -> f64 {
    use rs_cam_core::toolpath::MoveIntent;
    let end = end_move.min(moves.len());
    let start = start_move.max(1);
    let mut len = 0.0;
    for i in start..end {
        if moves[i].intent == MoveIntent::FinishingCut {
            let a = moves[i - 1].target;
            let b = moves[i].target;
            len += ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        }
    }
    len
}

// ── branch scoring ──────────────────────────────────────────────────────

/// One branch's full scoring: chain totals + per-band COLUMNS quality
/// shares for bands 1..=3 (index 0 unused — `BandMap::code_at`'s
/// off-region code has no quality meaning).
#[allow(dead_code)]
struct BranchScore {
    outcome: ChainOutcome,
    n_by_band: [usize; 4],
    on_size_by_band: [f64; 4],
    plus05_by_band: [f64; 4],
    tail_by_band: [usize; 4],
}

/// Load the fixture fresh, apply `branch`, (for `Cascade`) swap Op B to the
/// claims config, run the chain, measure COLUMNS quality at 0.25mm, and
/// (for `Cascade`) print the Region-span mix table + claims-detector
/// evidence. `project_path` is expected to come from `write_fixture_project`
/// (a fresh `ProjectSession::load` per call — branches never share a
/// mutated session, matching every other A/B in this crate).
fn score_branch(label: &str, project_path: &std::path::Path, branch: Branch) -> BranchScore {
    let mut s = ProjectSession::load(project_path)
        .unwrap_or_else(|e| panic!("[{label}] failed to load {}: {e}", project_path.display()));
    apply_branch(&mut s, branch);

    if branch == Branch::Cascade {
        let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
        s.set_toolpath_operation(idx, OperationConfig::UnifiedFinish(op_b_claims_config()))
            .unwrap_or_else(|e| panic!("[{label}] swap Op B to claims config: {e}"));
    }

    let outcome = run_chain(label, &mut s);

    run_measurement_sim(&mut s);
    let bm = build_band_map(&s);
    fidelity_report(label, &s, &bm);

    // Group filter: resolve off whichever op is this branch's quality
    // reference (Op B for Cascade, D for AllOverTip). Single-setup
    // fixture, so this always resolves to 0 — but resolved properly
    // rather than hardcoded, per the TP15 lesson (never assume resolution
    // or grouping without reading it off the session).
    let ref_name = match branch {
        Branch::Cascade => "Op B Unified Rest",
        Branch::AllOverTip => "D All-Over Tip",
    };
    let ref_idx = toolpath_index_by_name(&s, ref_name);
    let ref_id = s.get_toolpath_config(ref_idx).expect("ref op config").id;
    let group = s
        .setup_of_toolpath_id(ref_id)
        .unwrap_or_else(|| panic!("[{label}] '{ref_name}' has no setup group"));

    let mut n_by_band = [0usize; 4];
    let mut on_size_by_band = [0.0f64; 4];
    let mut plus05_by_band = [0.0f64; 4];
    let mut tail_by_band = [0usize; 4];
    for code in 1u8..=3 {
        let (n, on, plus05, tail) = band_shares(&s, &bm, group, code);
        n_by_band[code as usize] = n;
        on_size_by_band[code as usize] = on;
        plus05_by_band[code as usize] = plus05;
        tail_by_band[code as usize] = tail;
    }
    eprintln!(
        "[{label}] band shares (group {group}): shallow n={} on={:.1}% +.05={:.1}% tail={} | \
         mid-steep n={} on={:.1}% +.05={:.1}% tail={} | very-steep n={} on={:.1}% +.05={:.1}% tail={}",
        n_by_band[1],
        on_size_by_band[1],
        plus05_by_band[1],
        tail_by_band[1],
        n_by_band[2],
        on_size_by_band[2],
        plus05_by_band[2],
        tail_by_band[2],
        n_by_band[3],
        on_size_by_band[3],
        plus05_by_band[3],
        tail_by_band[3],
    );

    if branch == Branch::Cascade {
        let op_b_idx = toolpath_index_by_name(&s, "Op B Unified Rest");
        let result = s
            .get_result(op_b_idx)
            .unwrap_or_else(|| panic!("[{label}] Op B result missing after generation"));
        let ann = result.annotated();
        let outer = outer_region_spans(&ann.spans);
        assert!(
            !outer.is_empty(),
            "[{label}] Op B emitted zero outer Region spans — cascade routing regressed \
             (design doc §2.4: Region spans are a MUST)"
        );
        eprintln!("== [{label}] Op B REGION-SPAN MIX ==");
        eprintln!("{:<28} {:>10} {:>14}", "span label", "moves", "cutting_mm");
        for span in &outer {
            let move_len = span.end_move.saturating_sub(span.start_move);
            let cut_mm =
                segment_cutting_length_mm(&ann.toolpath.moves, span.start_move, span.end_move);
            eprintln!("{:<28} {:>10} {:>14.1}", span.label, move_len, cut_mm);
        }
        eprintln!(
            "[{label}] claims-detector evidence: rest_grid={} rest_regions={}",
            ann.rest_grid.is_some(),
            ann.rest_regions.is_some()
        );
    }

    BranchScore {
        outcome,
        n_by_band,
        on_size_by_band,
        plus05_by_band,
        tail_by_band,
    }
}

// ── the cascade A/B (process proof, design doc §0.a) ────────────────────

#[test]
#[ignore = "two full scaled-wanaka chains + 0.25mm measurement sims (long); run with --ignored --nocapture"]
fn v3_cascade_ab_ball3() {
    let path = write_fixture_project(3.0);

    let d = score_branch("v3_D_allover_tip", &path, Branch::AllOverTip);
    let c = score_branch("v3_cascade_b3", &path, Branch::Cascade);

    let cascade_finish_s: f64 = c
        .outcome
        .per_op_s
        .iter()
        .filter(|(name, _)| {
            name.as_str() == "Op A Ball Finish" || name.as_str() == "Op B Unified Rest"
        })
        .map(|(_, secs)| *secs)
        .sum();
    let d_finish_s: f64 = d
        .outcome
        .per_op_s
        .iter()
        .filter(|(name, _)| name.as_str() == "D All-Over Tip")
        .map(|(_, secs)| *secs)
        .sum();

    eprintln!("== v3 CASCADE vs ALL-OVER-TIP VERDICT (ball Ø3, wanaka x2) ==");
    eprintln!(
        "project_total_s : D={:8.1}s  cascade={:8.1}s  Δ={:+8.1}s ({:+.1}%)",
        d.outcome.project_total_s,
        c.outcome.project_total_s,
        c.outcome.project_total_s - d.outcome.project_total_s,
        100.0 * (c.outcome.project_total_s - d.outcome.project_total_s)
            / d.outcome.project_total_s.max(1e-9)
    );
    eprintln!(
        "finish_stack_s  : D={d_finish_s:8.1}s  cascade={cascade_finish_s:8.1}s  Δ={:+8.1}s ({:+.1}%)",
        cascade_finish_s - d_finish_s,
        100.0 * (cascade_finish_s - d_finish_s) / d_finish_s.max(1e-9)
    );
    eprintln!(
        "collisions      : D={}  cascade={}",
        d.outcome.collisions, c.outcome.collisions
    );
    const BAND_LABEL: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];
    for (code, band_label) in BAND_LABEL.iter().enumerate().skip(1) {
        eprintln!(
            "{:<11}: on-size D={:5.1}% (n={:>7}) cascade={:5.1}% (n={:>7}) | +.05 D={:5.1}% cascade={:5.1}% | '>+.5' tail D={} cascade={}",
            band_label,
            d.on_size_by_band[code],
            d.n_by_band[code],
            c.on_size_by_band[code],
            c.n_by_band[code],
            d.plus05_by_band[code],
            c.plus05_by_band[code],
            d.tail_by_band[code],
            c.tail_by_band[code],
        );
    }

    // ── GATES (design doc §4 order — quality must report before time) ──

    // (a) collisions == 0 for BOTH branches. This fixture is single-setup
    // and freshly generated every run — it does not inherit wanaka.toml's
    // `BASELINE_RAPID_COLLISIONS = 4` allowance (that baseline is specific
    // to the live multi-op project's pre-existing state).
    assert_eq!(
        d.outcome.collisions, 0,
        "D branch: expected 0 rapid collisions on the fresh scaled fixture, got {}",
        d.outcome.collisions
    );
    assert_eq!(
        c.outcome.collisions, 0,
        "cascade branch: expected 0 rapid collisions on the fresh scaled fixture, got {}",
        c.outcome.collisions
    );

    // (b) quality: cascade on-size share must not regress beyond 2.0pp on
    // mid-steep or shallow (very-steep is not gated here — Op A's ball
    // all-over pass may not reach very-steep territory at all on this
    // fixture, and Op B's rest-clear coverage there is not yet a claim
    // this A/B makes). Skip a band's gate (with an explanation) when
    // either branch's sample count is too small to be meaningful.
    const N_GUARD: usize = 1000;
    const QUALITY_TOL_PP: f64 = 2.0;
    for code in [1usize, 2usize] {
        let n = d.n_by_band[code].min(c.n_by_band[code]);
        if n < N_GUARD {
            eprintln!(
                "[{}] n={n} < {N_GUARD} guard — skipping quality gate (d_n={} c_n={})",
                BAND_LABEL[code], d.n_by_band[code], c.n_by_band[code]
            );
            continue;
        }
        assert!(
            c.on_size_by_band[code] >= d.on_size_by_band[code] - QUALITY_TOL_PP,
            "QUALITY GATE FAILED [{}]: cascade on-size {:.1}% regressed beyond {QUALITY_TOL_PP}pp \
             vs D's {:.1}%",
            BAND_LABEL[code],
            c.on_size_by_band[code],
            d.on_size_by_band[code]
        );
    }

    // (c) tails: cascade's '>+.5' standing-leftover tail must not grow
    // beyond D's by more than 5% + 100 columns (the `s1_claims_ab` slack
    // pattern — measurement-sim texture jitters raw tail counts run to
    // run). Gated on mid-steep and shallow, matching the quality gate
    // above.
    for code in [1usize, 2usize] {
        let off_n = d.tail_by_band[code];
        let on_n = c.tail_by_band[code];
        assert!(
            on_n <= off_n + off_n / 20 + 100,
            "TAIL GATE FAILED [{}]: cascade left {on_n} '>+.5' columns vs D's {off_n} \
             — the rest-island territory filter is skipping cuttable material",
            BAND_LABEL[code]
        );
    }

    // (d) TIME — the process proof itself.
    assert!(
        cascade_finish_s < d_finish_s,
        "TIME GATE FAILED: cascade finish-stack {cascade_finish_s:.1}s is not faster than \
         D's {d_finish_s:.1}s — the cascade does not beat the all-over baseline"
    );
    assert!(
        c.outcome.project_total_s < d.outcome.project_total_s,
        "TIME GATE FAILED: cascade project total {:.1}s is not faster than D's {:.1}s",
        c.outcome.project_total_s,
        d.outcome.project_total_s
    );
}

/// Ball-size sweep skeleton (slice 5 prep, design doc §0.a item 5): three
/// cascade-only chains at Ø2/3/4, no D comparison (that lives in
/// `v3_cascade_ab_ball3` — this test's job is the "optimal ball" curve
/// across cascade runs, not another A/B). Only per-run collisions are
/// gated; the CSV-ish table at the end is read by hand (or piped into a
/// future analysis script) once slice 5 defines the sweep's acceptance
/// rule.
#[test]
#[ignore = "three scaled-wanaka cascade chains (very long); run explicitly with --ignored --nocapture"]
fn v3_ball_sweep() {
    struct Row {
        ball_mm: f64,
        op_a_s: f64,
        op_b_s: f64,
        finish_stack_s: f64,
        project_s: f64,
        mid_steep_on_size_pct: f64,
        mid_steep_tail: usize,
    }

    let mut rows: Vec<Row> = Vec::new();
    for &ball_mm in &[2.0, 3.0, 4.0] {
        let path = write_fixture_project(ball_mm);
        let label = format!("v3_cascade_b{ball_mm:.0}");
        let score = score_branch(&label, &path, Branch::Cascade);
        assert_eq!(
            score.outcome.collisions, 0,
            "[{label}] ball Ø{ball_mm}: expected 0 rapid collisions"
        );
        let op_a_s = score
            .outcome
            .per_op_s
            .iter()
            .find(|(n, _)| n.as_str() == "Op A Ball Finish")
            .map_or(0.0, |(_, secs)| *secs);
        let op_b_s = score
            .outcome
            .per_op_s
            .iter()
            .find(|(n, _)| n.as_str() == "Op B Unified Rest")
            .map_or(0.0, |(_, secs)| *secs);
        rows.push(Row {
            ball_mm,
            op_a_s,
            op_b_s,
            finish_stack_s: op_a_s + op_b_s,
            project_s: score.outcome.project_total_s,
            mid_steep_on_size_pct: score.on_size_by_band[2],
            mid_steep_tail: score.tail_by_band[2],
        });
    }

    eprintln!("== v3 BALL SWEEP (cascade only; D comparison lives in v3_cascade_ab_ball3) ==");
    eprintln!(
        "ball_mm,op_a_s,op_b_s,finish_stack_s,project_s,mid_steep_on_size_pct,mid_steep_tail_gt05"
    );
    for r in &rows {
        eprintln!(
            "{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{}",
            r.ball_mm,
            r.op_a_s,
            r.op_b_s,
            r.finish_stack_s,
            r.project_s,
            r.mid_steep_on_size_pct,
            r.mid_steep_tail
        );
    }
}

// ── diagnostics ─────────────────────────────────────────────────────────

/// Frame diagnostic (kept as the reproduction for the identity-setup
/// deviation-frame RCA, 2026-07-13): the first cascade A/B measured a
/// uniform ~−4 mm "overcut" in every band on this fixture — physically
/// impossible next to 0 collisions and sane removed volumes, so a FRAME
/// question. Prints the three frames side by side after a rough-only
/// chain: model bbox (world truth), composite sim stock mesh z range, and
/// a spread of `column_deviations` rows with the implied model reference
/// (`top_z − dev`). Root cause was `SimulationRequest::model_mesh`
/// arriving stock-relative while identity groups' dexel grids are
/// world-framed (F-024); fixed in `compute/simulate.rs`, sentried by
/// `column_deviations_pointwise_against_flat_model`.
#[test]
#[ignore = "frame diagnostic (one rough generation + sims); run with --ignored --nocapture"]
fn v3_frame_probe() {
    let project_path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    set_enabled_by_name(
        &mut s,
        &["Rough"],
        &["Op A Ball Finish", "Op B Unified Rest", "D All-Over Tip"],
    );
    run_chain("frame probe rough-only", &mut s);

    let bboxes = s.collect_model_bboxes();
    let (_, mb) = &bboxes[0];
    eprintln!(
        "MODEL bbox: x {:.3}..{:.3}  y {:.3}..{:.3}  z {:.3}..{:.3}",
        mb.min.x, mb.max.x, mb.min.y, mb.max.y, mb.min.z, mb.max.z
    );

    let sim = s.simulation_result().expect("sim result");
    let verts = &sim.mesh.vertices;
    let mut zmin = f32::INFINITY;
    let mut zmax = f32::NEG_INFINITY;
    for i in 0..verts.len() / 3 {
        let z = verts[i * 3 + 2];
        zmin = zmin.min(z);
        zmax = zmax.max(z);
    }
    eprintln!("SIM STOCK MESH z range: {zmin:.3}..{zmax:.3} (expected world −5..9)");

    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations present");
    eprintln!("column_deviations: n={}", cols.len());
    for (k, cd) in cols.iter().enumerate().step_by(cols.len() / 8 + 1) {
        eprintln!(
            "col[{k}]: x={:8.2} y={:8.2} top_z={:8.3} dev={:8.3} implied_model_ref={:8.3} (row={} col={} group={})",
            cd.x,
            cd.y,
            cd.top_z,
            cd.dev,
            f64::from(cd.top_z) - f64::from(cd.dev),
            cd.row,
            cd.col,
            cd.group
        );
    }
    let hi = cols
        .iter()
        .max_by(|a, b| a.top_z.total_cmp(&b.top_z))
        .expect("nonempty");
    let lo = cols
        .iter()
        .min_by(|a, b| a.top_z.total_cmp(&b.top_z))
        .expect("nonempty");
    for (tag, cd) in [("max-top", hi), ("min-top", lo)] {
        eprintln!(
            "{tag}: x={:8.2} y={:8.2} top_z={:8.3} dev={:8.3} implied_model_ref={:8.3}",
            cd.x,
            cd.y,
            cd.top_z,
            cd.dev,
            f64::from(cd.top_z) - f64::from(cd.dev)
        );
    }
}

/// Tail diagnostic: cross-references every '>+.5' leftover column (Op B's
/// group) against (a) the claims detector's own rest grid at that XY —
/// bucketed NaN / below-dial / at-or-above-dial — and (b) the band map.
/// This is what proved the detector SEES the material the first clipped
/// cascade skipped (tail columns sat on above-dial rest, mean 1.2-1.7 mm),
/// i.e. the loss was in territory PLUMBING (capped polygonization), not
/// detection — which motivated the pre-decompose mask-AND.
#[test]
#[ignore = "one cascade chain + measurement sim (~9 min); run with --ignored --nocapture"]
fn v3_tail_probe() {
    let project_path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    apply_branch(&mut s, Branch::Cascade);
    let op_b_idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        op_b_idx,
        OperationConfig::UnifiedFinish(op_b_claims_config()),
    )
    .expect("swap Op B config");
    run_chain("tail probe cascade", &mut s);
    run_measurement_sim(&mut s);
    let bm = build_band_map(&s);

    let op_id = s.get_toolpath_config(op_b_idx).expect("cfg").id;
    let group = s
        .setup_of_toolpath_id(op_id)
        .expect("op B belongs to a setup");
    let ann = s.get_result(op_b_idx).expect("op B generated").annotated();
    let grid = ann
        .rest_grid
        .as_ref()
        .expect("claims ran -> rest_grid carried");

    let total = (grid.nx * grid.ny) as f64;
    let mut n_nan = 0usize;
    let mut n_below = 0usize;
    let mut n_above = 0usize;
    for &r in &grid.rest {
        if r.is_nan() {
            n_nan += 1;
        } else if f64::from(r) >= 0.022 {
            n_above += 1;
        } else {
            n_below += 1;
        }
    }
    eprintln!(
        "REST GRID {}x{} cell={:.3} origin=({:.2},{:.2}): nan={n_nan} ({:.1}%) below-dial={n_below} ({:.1}%) above-dial={n_above} ({:.1}%)",
        grid.nx,
        grid.ny,
        grid.cell_mm,
        grid.origin_x,
        grid.origin_y,
        100.0 * n_nan as f64 / total,
        100.0 * n_below as f64 / total,
        100.0 * n_above as f64 / total,
    );

    let rest_at = |x: f64, y: f64| -> Option<f32> {
        let col = ((x - grid.origin_x) / grid.cell_mm).round();
        let row = ((y - grid.origin_y) / grid.cell_mm).round();
        if col < 0.0 || row < 0.0 || col >= grid.nx as f64 || row >= grid.ny as f64 {
            return None;
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        Some(grid.rest[row as usize * grid.nx + col as usize])
    };
    let sim = s.simulation_result().expect("sim result");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations present");
    // counts[band][bucket]: bucket 0=off-grid, 1=NaN, 2=below, 3=above
    let mut counts = [[0usize; 4]; 4];
    let mut dev_sum = [[0.0f64; 4]; 4];
    for cd in cols.iter().filter(|cd| cd.group == group && cd.dev > 0.5) {
        let band = bm.code_at(cd.x, cd.y) as usize;
        let bucket = match rest_at(cd.x, cd.y) {
            None => 0usize,
            Some(r) if r.is_nan() => 1,
            Some(r) if f64::from(r) < 0.022 => 2,
            Some(_) => 3,
        };
        counts[band][bucket] += 1;
        dev_sum[band][bucket] += f64::from(cd.dev);
    }
    eprintln!("TAIL COLUMNS (dev>0.5, group {group}) by band x rest-bucket:");
    eprintln!(
        "{:<11} | {:>9} {:>9} {:>10} {:>10} | mean dev per bucket",
        "band", "off-grid", "nan", "below-dial", "above-dial"
    );
    const BAND_LABEL2: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];
    for (band, label) in BAND_LABEL2.iter().enumerate() {
        let c = counts[band];
        let m = |i: usize| dev_sum[band][i] / (c[i].max(1) as f64);
        eprintln!(
            "{label:<11} | {:>9} {:>9} {:>10} {:>10} | {:.2} {:.2} {:.2} {:.2}",
            c[0],
            c[1],
            c[2],
            c[3],
            m(0),
            m(1),
            m(2),
            m(3)
        );
    }
}

/// Classification-only probe (no generation, ~3 min): the band-map
/// coverage accounting for the ×2 fixture. Answers "how much of the
/// classification grid is covered, and how much do the conditioned band
/// polygons actually reclaim of it" — the measurement that showed
/// decompose's extraction reclaiming only ~17% of covered area at Ø1-tip
/// dials (min-area absorption at tool scale), independent of any
/// territory logic.
#[test]
#[ignore = "classification only (~3 min); run with --ignored --nocapture"]
fn v3_band_coverage_probe() {
    let project_path = write_fixture_project(3.0);
    let s = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    build_band_map(&s);
}

// ── follow-up: tool load + air anatomy (2026-08-03) ─────────────────────
//
// Two questions the ball sweep left open, both raised as hypotheses worth
// testing rather than assumed:
//
// 1. TOOL LOAD. The campaign scored time and COLUMNS quality only. The
//    cascade's whole premise is that a Ø3-4 ball does the bulk of the
//    surface work and a Ø1 tapered tip only visits what the ball couldn't
//    reach — which is exactly the shape of a tool-load win, since the
//    all-over baseline runs that same fragile tip over the ENTIRE part.
//    A time loss bought with a load win is a different (and possibly
//    better) trade than a time loss for nothing.
//
// 2. WHERE THE AIR IS. Op B spends 44-58% of its time in rapids and the
//    campaign named S3's cross-region fused router as the owner. That
//    attribution is an assumption: a cross-region router only helps if
//    the air is BETWEEN regions. If it is INSIDE one region — the scallop
//    emitter retracting between disconnected ring fragments on a
//    dendritic polygon — then S3 as designed cannot fix it, and the real
//    work is intra-region linking. The run-7 region-span mix (one span
//    with 206k moves, two with a few hundred) makes this the live
//    question. Rapids are attributed to the innermost outer-Region span
//    containing their move index; rapids in no span are inter-region.

/// Per-branch tool-load rollup: every enabled toolpath's milling criteria
/// (chipload / power / deflection) with state and peak, plus a project
/// roll-up. Read AFTER `run_chain` (the verdicts consume the sim trace).
fn tool_load_table(label: &str, s: &ProjectSession) {
    use rs_cam_core::tool_load::verdict::LoadState;

    let report = s.tool_load_report();
    let n = s.toolpath_count();
    eprintln!("== TOOL LOAD [{label}] ==");
    eprintln!(
        "{:<22} | {:<18} {:<10} {:>10} unit",
        "op", "criterion", "state", "peak"
    );
    let mut within = 0usize;
    let mut exceeds = 0usize;
    let mut unmodeled = 0usize;
    for v in &report.per_toolpath {
        let name = (0..n)
            .filter_map(|i| s.get_toolpath_config(i))
            .find(|tc| tc.id == v.toolpath_id)
            .map(|tc| tc.name.clone())
            .unwrap_or_else(|| format!("{:?}", v.toolpath_id));
        for c in v.criteria() {
            match c.state {
                LoadState::Within => within += 1,
                LoadState::Exceeds => exceeds += 1,
                LoadState::Unmodeled => unmodeled += 1,
            }
            let peak = c
                .display_peak
                .map_or_else(|| "—".to_owned(), |p| format!("{p:.4}"));
            eprintln!(
                "{name:<22} | {:<18} {:<10} {peak:>10} {}",
                format!("{:?}", c.kind),
                format!("{:?}", c.state),
                c.unit
            );
        }
    }
    eprintln!("[{label}] criteria rollup: within={within} exceeds={exceeds} unmodeled={unmodeled}");
}

/// Rapid-move anatomy for one op: how much of its air is INSIDE a routed
/// region (the strategy emitter's own retract/replunge between path
/// fragments) vs BETWEEN regions (the cross-region router's territory).
/// This is the measurement that decides whether S3's fused router can
/// actually recover Op B's air, or whether the cost lives one level down
/// in the per-region emitters.
fn air_anatomy(label: &str, s: &ProjectSession, op_index: usize) {
    use rs_cam_core::toolpath::MoveType;

    let ann = s
        .get_result(op_index)
        .unwrap_or_else(|| panic!("[{label}] op {op_index} not generated"))
        .annotated();
    let moves = &ann.toolpath.moves;
    let outer = outer_region_spans(&ann.spans);

    // Innermost outer-Region span containing a move index, if any.
    let span_of = |mi: usize| -> Option<usize> {
        outer
            .iter()
            .position(|sp| mi >= sp.start_move && mi < sp.end_move)
    };

    let dist = |a: &rs_cam_core::geo::P3, b: &rs_cam_core::geo::P3| -> f64 {
        ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
    };

    let mut intra_n = 0usize;
    let mut intra_mm = 0.0f64;
    let mut inter_n = 0usize;
    let mut inter_mm = 0.0f64;
    let mut cut_mm = 0.0f64;
    // Per-span rapid tally so a single dominant region is visible.
    let mut per_span: Vec<(usize, f64)> = vec![(0, 0.0); outer.len()];
    let mut prev: Option<rs_cam_core::geo::P3> = None;
    for (mi, mv) in moves.iter().enumerate() {
        if let Some(p) = prev {
            let d = dist(&p, &mv.target);
            if matches!(mv.move_type, MoveType::Rapid) {
                match span_of(mi) {
                    Some(si) => {
                        intra_n += 1;
                        intra_mm += d;
                        if let Some(slot) = per_span.get_mut(si) {
                            slot.0 += 1;
                            slot.1 += d;
                        }
                    }
                    None => {
                        inter_n += 1;
                        inter_mm += d;
                    }
                }
            } else {
                cut_mm += d;
            }
        }
        prev = Some(mv.target);
    }

    eprintln!("== AIR ANATOMY [{label}] (op index {op_index}) ==");
    eprintln!(
        "moves={} outer_region_spans={} | cutting/feed {cut_mm:.0}mm",
        moves.len(),
        outer.len()
    );
    let tot_mm = intra_mm + inter_mm;
    let pct = |v: f64| 100.0 * v / tot_mm.max(1e-9);
    eprintln!(
        "rapids INSIDE regions : n={intra_n:>7} {intra_mm:>12.0}mm ({:.1}% of rapid length)  <- emitter/intra-region linking",
        pct(intra_mm)
    );
    eprintln!(
        "rapids BETWEEN regions: n={inter_n:>7} {inter_mm:>12.0}mm ({:.1}% of rapid length)  <- cross-region router (S3)",
        pct(inter_mm)
    );
    for (i, sp) in outer.iter().enumerate() {
        let (n, mm) = per_span.get(i).copied().unwrap_or((0, 0.0));
        eprintln!(
            "  span[{i}] {:<16} moves {:>7} | rapids n={n:>7} {mm:>12.0}mm",
            sp.label.as_ref(),
            sp.end_move.saturating_sub(sp.start_move)
        );
    }
}

/// Follow-up measurement: run BOTH branches once more and score the two
/// open hypotheses — tool load (is the cascade's load story better than
/// all-over-tip's?) and air anatomy (is Op B's air intra- or
/// inter-region, i.e. can S3's router actually recover it?). No gates:
/// this is a measurement, and its output feeds the next design decision.
#[test]
#[ignore = "two full chains + measurement sims (~20 min); run with --ignored --nocapture"]
fn v3_load_and_air_probe() {
    let project_path = write_fixture_project(4.0);

    // ── D: all-over tip ────────────────────────────────────────────────
    let mut d = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    apply_branch(&mut d, Branch::AllOverTip);
    run_chain("load probe D", &mut d);
    tool_load_table("D all-over tip", &d);
    let d_idx = toolpath_index_by_name(&d, "D All-Over Tip");
    air_anatomy("D all-over tip", &d, d_idx);

    // ── Cascade at Ø4 (the sweep's best finish stack) ──────────────────
    let mut c = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    apply_branch(&mut c, Branch::Cascade);
    let op_b_idx = toolpath_index_by_name(&c, "Op B Unified Rest");
    c.set_toolpath_operation(
        op_b_idx,
        OperationConfig::UnifiedFinish(op_b_claims_config()),
    )
    .expect("swap Op B config");
    run_chain("load probe cascade b4", &mut c);
    tool_load_table("cascade Ø4", &c);
    let op_a_idx = toolpath_index_by_name(&c, "Op A Ball Finish");
    air_anatomy("cascade Ø4 Op A", &c, op_a_idx);
    air_anatomy("cascade Ø4 Op B", &c, op_b_idx);
}

/// Upper-bound estimate of how much of an op's intra-region air is
/// RECOVERABLE by reordering, computed offline without touching core.
///
/// Why this exists: `air_anatomy` proved Op B's air is 100% intra-region,
/// and `crate::tsp::optimize_rapid_order` — the repo's existing 2-opt
/// reorderer — never runs on it, because `execute.rs`'s capability gate
/// gives up when the toolpath carries no `RapidOrderBarrier` spans, and
/// those come only from depth sections / adaptive3d events. Surface
/// finishers (scallop, waterline, raster, unified) emit none, so the
/// optimizer is structurally unreachable for the entire op family.
///
/// Before proposing that change, size the prize: split the toolpath into
/// rapid-separated cut FRAGMENTS, then compare the emitted traversal
/// order against a greedy nearest-neighbour tour over the same fragments
/// (either endpoint, since a fragment may be cut in reverse). NN is a
/// weak heuristic — a real 2-opt does better — so this is a conservative
/// LOWER bound on the win and an upper bound on remaining air.
fn recoverable_air(label: &str, s: &ProjectSession, op_index: usize) {
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::MoveType;

    let ann = s
        .get_result(op_index)
        .unwrap_or_else(|| panic!("[{label}] op {op_index} not generated"))
        .annotated();
    let moves = &ann.toolpath.moves;

    // A fragment = a maximal run of non-rapid moves. Record its entry and
    // exit points; the rapid hops between consecutive fragments are the
    // cost we can reorder away.
    let mut frags: Vec<(P3, P3)> = Vec::new();
    let mut cur_start: Option<P3> = None;
    let mut cur_end: P3 = P3::new(0.0, 0.0, 0.0);
    let mut prev: Option<P3> = None;
    for mv in moves {
        if matches!(mv.move_type, MoveType::Rapid) {
            if let Some(st) = cur_start.take() {
                frags.push((st, cur_end));
            }
        } else {
            if cur_start.is_none() {
                cur_start = Some(prev.unwrap_or(mv.target));
            }
            cur_end = mv.target;
        }
        prev = Some(mv.target);
    }
    if let Some(st) = cur_start {
        frags.push((st, cur_end));
    }

    let d = |a: P3, b: P3| -> f64 {
        ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
    };

    // Emitted order: hop from each fragment's exit to the next's entry.
    let emitted: f64 = frags.windows(2).map(|w| d(w[0].1, w[1].0)).sum();

    // Greedy nearest-neighbour over unvisited fragments, either end.
    let n = frags.len();
    let mut visited = vec![false; n];
    let mut nn_total = 0.0f64;
    let mut cur = 0usize;
    visited[0] = true;
    let mut cur_exit = frags[0].1;
    for _ in 1..n {
        let mut best = usize::MAX;
        let mut best_d = f64::INFINITY;
        let mut best_rev = false;
        for (j, f) in frags.iter().enumerate() {
            if visited[j] {
                continue;
            }
            let d_fwd = d(cur_exit, f.0);
            if d_fwd < best_d {
                best_d = d_fwd;
                best = j;
                best_rev = false;
            }
            let d_rev = d(cur_exit, f.1);
            if d_rev < best_d {
                best_d = d_rev;
                best = j;
                best_rev = true;
            }
        }
        if best == usize::MAX {
            break;
        }
        visited[best] = true;
        nn_total += best_d;
        cur_exit = if best_rev {
            frags[best].0
        } else {
            frags[best].1
        };
        cur = best;
    }
    let _ = cur;

    let saved = emitted - nn_total;
    eprintln!("== RECOVERABLE AIR [{label}] (op index {op_index}) ==");
    eprintln!(
        "cut fragments={n} | emitted-order hops {emitted:.0}mm | nearest-neighbour {nn_total:.0}mm | recoverable {saved:.0}mm ({:.1}%)",
        100.0 * saved / emitted.max(1e-9)
    );
}

/// Sizes the reordering prize on the branch that needs it (cascade Ø4).
/// Measurement only — feeds the decision on whether to make the existing
/// rapid-order optimizer reachable for surface-finishing ops.
#[test]
#[ignore = "one cascade chain (~3 min); run with --ignored --nocapture"]
fn v3_recoverable_air_probe() {
    let project_path = write_fixture_project(4.0);
    let mut c = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    apply_branch(&mut c, Branch::Cascade);
    let op_b_idx = toolpath_index_by_name(&c, "Op B Unified Rest");
    c.set_toolpath_operation(
        op_b_idx,
        OperationConfig::UnifiedFinish(op_b_claims_config()),
    )
    .expect("swap Op B config");
    run_chain("recoverable air cascade b4", &mut c);
    let op_a_idx = toolpath_index_by_name(&c, "Op A Ball Finish");
    recoverable_air("cascade Ø4 Op A", &c, op_a_idx);
    recoverable_air("cascade Ø4 Op B", &c, op_b_idx);

    // Baseline for scale: the all-over tip pass on a contiguous surface.
    let mut d = ProjectSession::load(&project_path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", project_path.display()));
    apply_branch(&mut d, Branch::AllOverTip);
    run_chain("recoverable air D", &mut d);
    let d_idx = toolpath_index_by_name(&d, "D All-Over Tip");
    recoverable_air("D all-over tip", &d, d_idx);
}
