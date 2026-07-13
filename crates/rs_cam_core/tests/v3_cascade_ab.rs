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

/// Which finishing path a chain run takes. Unused until the cascade A/B
/// scoring slice (`planning/unified_v3_design.md` §0.a) — kept here so
/// the fixture and the branch switch land together.
#[allow(dead_code)]
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

/// Apply a `Branch` to the fixture's 4-op chain by toolpath name. Unused
/// until the cascade A/B scoring slice — see `Branch`.
#[allow(dead_code)]
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

/// Totals from one full `run_chain` pass. `per_op_s` is reusable for
/// per-branch comparisons in the A/B slice (rather than pinning a single
/// "finish op" name the way `p2c_headless_ab_wanaka.rs` does — the
/// cascade branch has two finish ops, not one).
#[allow(dead_code)]
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
