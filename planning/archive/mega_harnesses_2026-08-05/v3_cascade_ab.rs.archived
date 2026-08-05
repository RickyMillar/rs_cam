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
use rs_cam_core::compute::operation_configs::{ClaimsReference, UnifiedFinishConfig};
use rs_cam_core::dressup::AirBridgePolicy;
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

/// Vertical exaggeration applied on top of `SCALE`, from `V3_ZEXAG`.
///
/// Wanaka is gentle terrain: at 1.0 the decomposition finds NO very-steep
/// band at all (measured, §14 — waterline 0%), so the fixture structurally
/// cannot present the mixed-strategy rest that `unified_finish` exists
/// for. Scaling Z alone turns the same terrain into steep mountains while
/// keeping the XY footprint, the triangle count and the facet topology
/// identical — so the strategy mix is the ONLY thing that moves, which is
/// exactly the comparison the question needs.
///
/// A slope `θ` becomes `atan(k·tanθ)`: at k = 4 a 20° hillside reads 55.5°
/// and a 45° face reads 76°, crossing both band thresholds
/// (`steep_threshold_deg` 45, `waterline_threshold_deg` 75).
///
/// 1.0 (the default) reproduces every earlier run: same STL path, same
/// stock, same `safe_z`.
fn z_exag_from_env() -> f64 {
    let raw = std::env::var("V3_ZEXAG").unwrap_or_else(|_| "1".to_owned());
    let k: f64 = raw
        .parse()
        .unwrap_or_else(|e| panic!("V3_ZEXAG must be a number, got {raw:?}: {e}"));
    assert!(k > 0.0, "V3_ZEXAG must be positive, got {k}");
    k
}

/// Write a ×`SCALE`'d copy of `source_stl_path()` — with Z additionally
/// multiplied by `z_exag` — to `target/v3_scaled/`, skipping the rewrite
/// when a same-length output already exists (binary STL length is fully
/// determined by triangle count, so a length match is a safe staleness
/// check here, and the exaggeration is in the FILENAME so variants never
/// alias each other).
///
/// Stored normals are left stale under a non-uniform scale. That is safe
/// and deliberate: `Mesh` recomputes every face normal from the vertices
/// on load, and its consistency check is edge-topological (winding), which
/// a Z scale preserves.
///
/// Returns the path and the scaled mesh's Z range, which
/// `write_fixture_project` needs to keep the stock and `safe_z` around a
/// model whose height changed.
fn ensure_scaled_stl(z_exag: f64) -> (PathBuf, (f64, f64)) {
    let out_path = if (z_exag - 1.0).abs() < 1e-12 {
        v3_dir().join("terrain_x2.stl")
    } else {
        v3_dir().join(format!("terrain_x2_z{z_exag:.2}.stl"))
    };
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

    // Scale in place and measure the Z range in the same pass — the
    // caller needs it whether or not the file had to be rewritten, so
    // this runs unconditionally and the staleness check only skips the
    // WRITE.
    let scale = SCALE as f32;
    let z_scale = (SCALE * z_exag) as f32;
    let (mut min_z, mut max_z) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in 0..tri_count {
        let verts_start = STL_HEADER_LEN + t * STL_TRIANGLE_LEN + STL_VERTS_OFFSET;
        for v in 0..9 {
            let off = verts_start + v * 4;
            let f = f32::from_le_bytes(
                bytes[off..off + 4]
                    .try_into()
                    .expect("4-byte slice for vertex float"),
            );
            // Vertex floats run x,y,z per vertex, so every third is Z.
            let scaled = if v % 3 == 2 { f * z_scale } else { f * scale };
            if v % 3 == 2 {
                min_z = min_z.min(scaled as f64);
                max_z = max_z.max(scaled as f64);
            }
            bytes[off..off + 4].copy_from_slice(&scaled.to_le_bytes());
        }
    }

    if let Ok(meta) = std::fs::metadata(&out_path)
        && meta.len() as usize == expected_len
    {
        eprintln!(
            "v3 fixture: {} already up to date ({tri_count} triangles, \
             z in [{min_z:.3}, {max_z:.3}]), skipping rewrite",
            out_path.display()
        );
        return (out_path, (min_z, max_z));
    }

    std::fs::write(&out_path, &bytes)
        .unwrap_or_else(|e| panic!("failed to write scaled STL {}: {e}", out_path.display()));
    eprintln!(
        "v3 fixture: wrote {} ({tri_count} triangles, x{SCALE} xy / x{z_scale} z, \
         z in [{min_z:.3}, {max_z:.3}])",
        out_path.display()
    );
    (out_path, (min_z, max_z))
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
    let z_exag = z_exag_from_env();
    let (stl_path, (model_min_z, model_max_z)) = ensure_scaled_stl(z_exag);
    let stl_path_str = stl_path.to_string_lossy();
    let stl_name = stl_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "terrain_x2.stl".to_owned());
    let out_path = if (z_exag - 1.0).abs() < 1e-12 {
        v3_dir().join(format!("wanaka_x2_ball{ball_diameter_mm:.0}.toml"))
    } else {
        v3_dir().join(format!(
            "wanaka_x2_z{z_exag:.2}_ball{ball_diameter_mm:.0}.toml"
        ))
    };

    // The ×1 fixture's hardcoded stock/safe_z, and the model Z range they
    // were chosen around. A taller model has to keep the SAME clearances
    // or the comparison changes two things at once — so the margins are
    // what carry over, not the literals. `V3_ZEXAG=1` short-circuits to
    // the literals, so the default fixture is unchanged (the emitted TOML
    // now reads `14.000` where it read `14.0` — same value, same parse).
    const BASE_ORIGIN_Z: f64 = -5.0;
    const BASE_STOCK_Z: f64 = 14.0;
    const BASE_SAFE_Z: f64 = 10.0;
    let (stock_origin_z, stock_z, safe_z) = if (z_exag - 1.0).abs() < 1e-12 {
        (BASE_ORIGIN_Z, BASE_STOCK_Z, BASE_SAFE_Z)
    } else {
        let (base_min_z, base_max_z) = (model_min_z / z_exag, model_max_z / z_exag);
        let origin_z = model_min_z + (BASE_ORIGIN_Z - base_min_z);
        let top_z = model_max_z + (BASE_ORIGIN_Z + BASE_STOCK_Z - base_max_z);
        (
            origin_z,
            top_z - origin_z,
            model_max_z + (BASE_SAFE_Z - base_max_z),
        )
    };
    eprintln!(
        "v3 fixture: z_exag={z_exag} → stock origin_z={stock_origin_z:.3} z={stock_z:.3}, \
         safe_z={safe_z:.3}"
    );

    let toml = format!(
        r#"format_version = 3
toolpaths = []

[job]
name = "wanaka_x2_cascade"

[job.stock]
x = 210.0
y = 210.0
z = {stock_z:.3}
origin_x = -5.0
origin_y = -5.0
origin_z = {stock_origin_z:.3}
padding = 0.0
workholding_rigidity = "Medium"
auto_from_model = false

[job.stock.material.SolidWood]
species = "GenericHardwood"

[job.post]
format = "grbl"
spindle_speed = 18000
safe_z = {safe_z:.3}
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
name = "{stl_name}"
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
    let (enable, disable) = branch_ops(branch);
    set_enabled_by_name(s, enable, disable);
}

/// The enable/disable name lists behind a `Branch`, so a probe can build a
/// partial chain (e.g. Rough + Op A, no Op B) from the same vocabulary
/// rather than hand-rolling a second copy of the op names.
fn branch_ops(branch: Branch) -> (&'static [&'static str], &'static [&'static str]) {
    match branch {
        Branch::Cascade => (
            &["Rough", "Op A Ball Finish", "Op B Unified Rest"],
            &["D All-Over Tip"],
        ),
        Branch::AllOverTip => (
            &["Rough", "D All-Over Tip"],
            &["Op A Ball Finish", "Op B Unified Rest"],
        ),
    }
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
    ensure_scaled_stl(z_exag_from_env());
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
/// - `claims_reference` `MachinedStock`: sanctioned here specifically
///   because Op B follows Op A's OWN ball all-over pass — a genuinely
///   finish-quality reference, not a rough-chain terrace field (the S1
///   lesson that ruled this reference out for wanaka.toml's rough→finish
///   chain).
/// - `territory_clip` TRUE (S4): AND a per-cell rest keep-mask into
///   coverage BEFORE `decompose` runs, so Op B generates only over real
///   rest material. An earlier region-level whole-island keep-or-drop
///   filter (informally "S2", since removed — see the `unified_finish`
///   module doc) couldn't shrink a giant conditioned island that merely
///   CONTAINS above-dial rest somewhere — the first wanaka ×2 cascade A/B
///   measured Op B at +47% over the all-over baseline for exactly that
///   reason (see the `UnifiedFinishConfig::territory_clip` field doc).
///   Sanctioned here because `claims_reference` is `MachinedStock` (the
///   clip is skipped with a warning under `SelfProbe`).
fn op_b_claims_config() -> UnifiedFinishConfig {
    op_b_claims_config_with_hookup(0.0)
}

/// §9 lever: `intra_region_hookup_mm` as an explicit A/B dial. `0.0` is
/// the shipped default and reproduces the pre-§9 op byte-for-byte.
fn op_b_claims_config_with_hookup(intra_region_hookup_mm: f64) -> UnifiedFinishConfig {
    op_b_config(intra_region_hookup_mm, true, 5.0)
}

/// `pencil_claims` as an explicit A/B dial alongside the §9 hookup. Every
/// measurement this campaign has taken ran with it ON; it is the one
/// unbounded linker left in Op B at SHIPPED dials (the crease node emits
/// through `pencil::emit_paths` at `PencilParams::default()`'s 5 mm
/// `hookup_distance` with no territory boundary — `unified_finish.rs`
/// Step 3.5).
fn op_b_config(
    intra_region_hookup_mm: f64,
    pencil_claims: bool,
    crease_hookup_mm: f64,
) -> UnifiedFinishConfig {
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
        pencil_claims,
        min_rest_depth_mm: 0.022,
        // A/M6: the DIAL type is now `ClaimsReference` (three-valued);
        // `MachinedStock` keeps its exact pre-A/M6 meaning — pinned, not
        // derived — so this arm is unchanged.
        claims_reference: ClaimsReference::MachinedStock,
        territory_clip: true,
        intra_region_hookup_mm,
        crease_hookup_mm,
        // M3 wave 7b: pinned to the production sampler by NAME, not by
        // `PRODUCTION`, so this campaign's arms keep measuring the same
        // classifier if the production default ever moves again.
        classification_sampler: rs_cam_core::classify_probe::ClassificationSampler::TileRaster,
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
///
/// # Measurement caveats (M1 / LH-4) — read before quoting anything from here
///
/// 1. **The bands OVERLAP.** `planner.overlap_mm = 2.0` dilates every band
///    polygon at extraction, so the band polygons mutually overlap by a 2 mm
///    ring. Summing `polygon.area()` across bands DOUBLE-COUNTS that ring;
///    the sum is not a total area and is not a share of anything. (The
///    rasterised `codes` grid does not double-count — a cell takes the
///    steeper band on overlap — but the polygon areas do.)
/// 2. **Cells and mm² are different domains.** The covered-cell count and
///    the band polygon areas below are printed separately, each with its
///    domain, and no ratio is formed between them. The retracted
///    "~17% of covered area reclaimed" claim was exactly that division
///    (`MEASUREMENT_DOMAINS.md` LH-4 / X-2).
/// 3. **A dilated band map is not an attribution channel.** Over-dilation
///    mislabels raster-owned flats as mid-steep. Verdicts must attribute by
///    `Region` spans, not by this map — the established rule after the span
///    fix in `77f2b7a`. This map is kept only because both branches are
///    scored against the *same* definition; it does not make either
///    labelling correct.
///
/// Redesigning the extraction is H4 territory and deliberately NOT done here.
fn build_band_map(s: &ProjectSession) -> BandMap {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::geo::P2;
    use rs_cam_core::measurement::{
        CellSource, MeasurementDomain, MeasurementProvenance, MeasurementStage,
    };
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
        surface.heightmap.covered_flags(),
        &[],
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

    // Coverage accounting (the 25%-coverage anomaly, 2026-07-13).
    //
    // LH-4: this block used to print a boolean CELL COUNT and overlap-dilated
    // mm² on one line, and a docstring then quoted a ratio between them
    // ("~17% of covered area reclaimed"). Cells are not mm² and the dilated
    // band areas double-count their mutual overlap, so that ratio was wrong
    // twice over. Both quantities are still printed — each on its own line,
    // each naming its domain, stage and grid — and NO ratio crosses them.
    let covered_cells = hm.covered_flags().iter().filter(|&&c| c).count();
    let grid_cells = rows * cols;
    let covered_projected_xy_area_mm2 = covered_cells as f64 * cell * cell;
    let coverage_prov = MeasurementProvenance::new(
        MeasurementDomain::GridCellCount,
        MeasurementStage::RawThreshold,
    )
    .with_cell(cell, CellSource::CuspRadius);
    let band_prov = MeasurementProvenance::new(
        MeasurementDomain::ProjectedXyArea,
        MeasurementStage::PolygonExtraction,
    )
    .with_cell(cell, CellSource::CuspRadius)
    .with_extraction_dilation_mm(planner.overlap_mm);

    // Same-domain ratio (cells over cells on one grid) — legal.
    eprintln!(
        "BAND COVERAGE / covered mask [{}]: {covered_cells} of {grid_cells} cells ({:.1}% of \
         grid cells) = {covered_projected_xy_area_mm2:.0} mm2 XY-projected",
        coverage_prov.describe(),
        100.0 * covered_cells as f64 / grid_cells as f64,
    );
    // Different stage AND dilated — printed alone, never divided by the line
    // above. Per-band, because the across-band sum is not a meaningful total.
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
        eprintln!(
            "BAND COVERAGE / {band:?}: {n} regions, {area:.0} mm2 XY-projected [{}]",
            band_prov.describe()
        );
    }
    eprintln!("BAND COVERAGE / decompose stats: {:?}", planned.stats);

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

/// Where the deep over-cut lives. The shipped cascade puts 1 218 shallow
/// columns below −0.5 mm (worst −3.03 mm) against D's 8 (worst −0.58) —
/// a gouge population, not cusp texture, and the `<-.5` bin alone cannot
/// say whether it is scattered (a systematic depth error) or clustered (a
/// handful of bad entries). Prints the per-band count, the worst columns'
/// world XY, and a coarse occupancy map so the two read apart at a glance.
fn deep_overcut_locator(tag: &str, s: &ProjectSession, bm: &BandMap, group: usize) -> usize {
    const DEEP: f32 = -0.5;
    const MAP: usize = 24;
    let Some(sim) = s.simulation_result() else {
        return 0;
    };
    let Some(cols) = sim.column_deviations.as_ref() else {
        return 0;
    };
    let deep: Vec<&rs_cam_core::compute::simulate::ColumnDeviation> = cols
        .iter()
        .filter(|cd| cd.group == group && cd.dev < DEEP)
        .collect();
    if deep.is_empty() {
        eprintln!("== [{tag}] DEEP OVER-CUT: none below {DEEP}mm ==");
        return 0;
    }
    let mut by_band = [0usize; 4];
    for cd in &deep {
        by_band[bm.code_at(cd.x, cd.y) as usize] += 1;
    }
    eprintln!(
        "== [{tag}] DEEP OVER-CUT (dev < {DEEP}mm, group {group}): n={} | {} ==",
        deep.len(),
        BAND_NAMES
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{n}={}", by_band[i]))
            .collect::<Vec<_>>()
            .join(" ")
    );

    // Worst columns, and — separately — the worst in each GATED band. The
    // gate reads bands 1..3, but off-region routinely dominates the raw
    // ranking, so a top-N by depth alone never shows a gate-relevant site.
    // `model_z` comes from a vertical ray so the absolute Z a move had to
    // reach is readable directly, instead of being inferred from `top_z`
    // (which is the setup group's LOCAL frame while `dev` is world).
    let mesh = s.models().iter().find_map(|m| m.mesh.clone());
    let model_z = |x: f64, y: f64| -> f64 {
        let Some(mesh) = mesh.as_ref() else {
            return f64::NAN;
        };
        let origin = rs_cam_core::geo::P3::new(x, y, 1.0e6);
        let dir = rs_cam_core::geo::V3::new(0.0, 0.0, -1.0);
        rs_cam_core::mesh::ray_pick_triangle(mesh, &origin, &dir)
            .map_or(f64::NAN, |(_, t)| 1.0e6 - t)
    };
    let mut worst = deep.clone();
    worst.sort_by(|a, b| a.dev.total_cmp(&b.dev));
    let show = |cd: &rs_cam_core::compute::simulate::ColumnDeviation| {
        let mz = model_z(cd.x, cd.y);
        eprintln!(
            "   dev={:+7.3} at ({:8.2},{:8.2}) band={:<11} model_z={:7.3} world_top={:7.3} (local top_z={:.3})",
            cd.dev,
            cd.x,
            cd.y,
            BAND_NAMES[bm.code_at(cd.x, cd.y) as usize],
            mz,
            mz + f64::from(cd.dev),
            cd.top_z
        );
    };
    for cd in worst.iter().take(8) {
        show(cd);
    }
    for code in 1u8..=3 {
        eprintln!("   -- worst in {} --", BAND_NAMES[code as usize]);
        for cd in worst
            .iter()
            .filter(|cd| bm.code_at(cd.x, cd.y) == code)
            .take(4)
        {
            show(cd);
        }
    }

    // Occupancy over the band-map extent: '.' none, digits log10-ish.
    let span_x = bm.cell * bm.cols as f64;
    let span_y = bm.cell * bm.rows as f64;
    let mut occ = vec![0usize; MAP * MAP];
    for cd in &deep {
        let cx = (((cd.x - bm.origin_x) / span_x) * MAP as f64).floor();
        let cy = (((cd.y - bm.origin_y) / span_y) * MAP as f64).floor();
        if cx < 0.0 || cy < 0.0 || cx >= MAP as f64 || cy >= MAP as f64 {
            continue;
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let idx = cy as usize * MAP + cx as usize;
        occ[idx] += 1;
    }
    let occupied = occ.iter().filter(|&&n| n > 0).count();
    eprintln!(
        "   occupancy over {MAP}x{MAP} tiles: {occupied}/{} tiles hold the {} deep columns",
        MAP * MAP,
        deep.len()
    );
    for r in (0..MAP).rev() {
        let row: String = (0..MAP)
            .map(|c| match occ[r * MAP + c] {
                0 => '.',
                1..=3 => '1',
                4..=10 => '2',
                11..=30 => '3',
                31..=100 => '4',
                _ => '#',
            })
            .collect();
        eprintln!("   |{row}|");
    }
    deep.len()
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
    /// Columns below −0.5 mm across all bands — the DEFECT count, which
    /// the verdict established matters more than the ±10 µm bin.
    deep_overcut_total: usize,
}

/// Load the fixture fresh, apply `branch`, (for `Cascade`) swap Op B to the
/// claims config, run the chain, measure COLUMNS quality at 0.25mm, and
/// (for `Cascade`) print the Region-span mix table + claims-detector
/// evidence. `project_path` is expected to come from `write_fixture_project`
/// (a fresh `ProjectSession::load` per call — branches never share a
/// mutated session, matching every other A/B in this crate).
/// The two §9/§10 dials, applied identically to BOTH branches so a
/// comparison never varies more than one thing (the trap this campaign
/// paid for twice).
#[derive(Debug, Clone, Copy)]
struct Dials {
    air_bridge_policy: AirBridgePolicy,
    intra_region_hookup_mm: f64,
    /// Op B's crease-claims node (v3 S1). ON in every measurement this
    /// campaign has taken; `V3_CLAIMS=off` turns it off to test whether
    /// the crease emitter owns the off-region deep over-cut — it links at
    /// `PencilParams::default().hookup_distance` (5 mm) with NO territory
    /// boundary, the one unbounded linker left in Op B at SHIPPED dials.
    ///
    /// CAVEAT: `pencil_claims = false` disables the WHOLE claims pipeline,
    /// territory clip included (`execute.rs`: `cfg.pencil_claims.then(..)`),
    /// so it varies two things. `crease_hookup_mm` is the isolated lever.
    pencil_claims: bool,
    /// Crease-node link cap (mm). 5.0 is what every measurement before
    /// 2026-07-27 ran with; `V3_CREASE_HOOKUP=0` keeps the claims pipeline
    /// and its territory clip while removing only the unbounded links.
    crease_hookup_mm: f64,
    /// Override every op's `optimize_rapid_order` dressup. `None` leaves
    /// the fixture's setting (on). `V3_REORDER=off` — the decisive test
    /// for whether TSP reassembly is corrupting arc centres (review
    /// finding 2: a segment's first move keeps `i/j` relative to its
    /// ORIGINAL source, and `rebuild_group` gives it a new one).
    optimize_rapid_order: Option<bool>,
    /// Override every op's `arc_fitting` dressup. `None` leaves the
    /// fixture's setting (on). Arc fitting replaces a drop-cutter-probed
    /// polyline with an arc in XY while interpolating Z linearly, so the
    /// tool travels at a height computed for points the arc never passes
    /// through — a candidate mechanism for cutting under the model that
    /// no amount of link/territory work would reach. `V3_ARCFIT=off`.
    arc_fitting: Option<bool>,
}

impl Dials {
    /// What ships today.
    const SHIPPED: Self = Self {
        air_bridge_policy: AirBridgePolicy::Always,
        intra_region_hookup_mm: 0.0,
        pencil_claims: true,
        crease_hookup_mm: 5.0,
        arc_fitting: None,
        optimize_rapid_order: None,
    };
    /// §9 intra-region stay-down linking + §10 cost-aware air bridges.
    const V3: Self = Self {
        air_bridge_policy: AirBridgePolicy::ShorterThanAirPath,
        intra_region_hookup_mm: 6.0,
        pencil_claims: true,
        crease_hookup_mm: 5.0,
        arc_fitting: None,
        optimize_rapid_order: None,
    };
    /// §10 cost-aware air bridges alone — the lever §10a isolated as
    /// carrying the whole win (−22.8% finish stack) at a twelfth of the
    /// over-cut the pair produces.
    const BRIDGES: Self = Self {
        air_bridge_policy: AirBridgePolicy::ShorterThanAirPath,
        intra_region_hookup_mm: 0.0,
        pencil_claims: true,
        crease_hookup_mm: 5.0,
        arc_fitting: None,
        optimize_rapid_order: None,
    };
    /// §9 intra-region stay-down links alone.
    const LINKS: Self = Self {
        air_bridge_policy: AirBridgePolicy::Always,
        intra_region_hookup_mm: 6.0,
        pencil_claims: true,
        crease_hookup_mm: 5.0,
        arc_fitting: None,
        optimize_rapid_order: None,
    };
}

/// `V3_DIALS=shipped|bridges|links|both`, so a dial setting can be chosen
/// per run rather than per rebuild. Shared by the two-branch proof and the
/// single-branch isolation probe so they can never disagree about what a
/// name means.
fn dials_from_env(default: &str) -> (String, Dials) {
    let which = std::env::var("V3_DIALS").unwrap_or_else(|_| default.to_owned());
    let mut dials = match which.as_str() {
        "shipped" => Dials::SHIPPED,
        "bridges" => Dials::BRIDGES,
        "links" => Dials::LINKS,
        "both" => Dials::V3,
        other => panic!("V3_DIALS must be shipped|bridges|links|both, got {other:?}"),
    };
    // Composes with any of the above: `V3_DIALS=shipped V3_CLAIMS=off`.
    let mut label = which;
    match std::env::var("V3_CLAIMS").as_deref() {
        Ok("off") => {
            dials.pencil_claims = false;
            label.push_str("+noclaims");
        }
        Ok("on") | Err(_) => {}
        Ok(other) => panic!("V3_CLAIMS must be on|off, got {other:?}"),
    }
    match std::env::var("V3_REORDER").as_deref() {
        Ok("off") => {
            dials.optimize_rapid_order = Some(false);
            label.push_str("+noreorder");
        }
        Ok("on") => dials.optimize_rapid_order = Some(true),
        Err(_) => {}
        Ok(other) => panic!("V3_REORDER must be on|off, got {other:?}"),
    }
    match std::env::var("V3_ARCFIT").as_deref() {
        Ok("off") => {
            dials.arc_fitting = Some(false);
            label.push_str("+noarcfit");
        }
        Ok("on") => dials.arc_fitting = Some(true),
        Err(_) => {}
        Ok(other) => panic!("V3_ARCFIT must be on|off, got {other:?}"),
    }
    if let Ok(raw) = std::env::var("V3_CREASE_HOOKUP") {
        let mm: f64 = raw
            .parse()
            .unwrap_or_else(|e| panic!("V3_CREASE_HOOKUP must be a number, got {raw:?}: {e}"));
        dials.crease_hookup_mm = mm;
        label.push_str(&format!("+crease{mm}"));
    }
    (label, dials)
}

fn score_branch(label: &str, project_path: &std::path::Path, branch: Branch) -> BranchScore {
    score_branch_with(label, project_path, branch, Dials::SHIPPED)
}

fn score_branch_with(
    label: &str,
    project_path: &std::path::Path,
    branch: Branch,
    dials: Dials,
) -> BranchScore {
    let (enable, disable) = branch_ops(branch);
    let ref_name = match branch {
        Branch::Cascade => "Op B Unified Rest",
        Branch::AllOverTip => "D All-Over Tip",
    };
    score_stage(label, project_path, enable, disable, ref_name, dials)
}

/// Score an arbitrary enabled subset of the fixture's op chain. `ref_name`
/// is the op whose setup group the COLUMNS table is filtered to (the
/// branch's quality reference). Op B's claims config and Region-span mix
/// are keyed off whether Op B is in `enable`, so a partial chain that
/// stops before it is scored exactly like the full one minus that op.
fn score_stage(
    label: &str,
    project_path: &std::path::Path,
    enable: &[&str],
    disable: &[&str],
    ref_name: &str,
    dials: Dials,
) -> BranchScore {
    score_stage_tweaked(
        label,
        project_path,
        enable,
        disable,
        ref_name,
        dials,
        &|_| {},
    )
}

/// `score_stage` with a hook applied to the freshly-loaded session, after
/// enable/disable and before the Op B config swap. Exists because
/// `score_stage` LOADS the project itself, so a caller cannot configure a
/// session and pass it in — any per-run parameter change has to happen
/// inside.
fn score_stage_tweaked(
    label: &str,
    project_path: &std::path::Path,
    enable: &[&str],
    disable: &[&str],
    ref_name: &str,
    dials: Dials,
    tweak: &dyn Fn(&mut ProjectSession),
) -> BranchScore {
    let mut s = ProjectSession::load(project_path)
        .unwrap_or_else(|e| panic!("[{label}] failed to load {}: {e}", project_path.display()));
    set_enabled_by_name(&mut s, enable, disable);
    tweak(&mut s);
    let has_op_b = enable.contains(&"Op B Unified Rest");

    if has_op_b {
        let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
        s.set_toolpath_operation(
            idx,
            OperationConfig::UnifiedFinish(op_b_config(
                dials.intra_region_hookup_mm,
                dials.pencil_claims,
                dials.crease_hookup_mm,
            )),
        )
        .unwrap_or_else(|e| panic!("[{label}] swap Op B to claims config: {e}"));
    }

    // The air-cut filter is a shared dressup, so the policy applies to every
    // op in the chain — including the roughing pass both branches share.
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d)
            .unwrap_or_else(|e| panic!("[{label}] set dressups on {i}: {e}"));
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

    let deep_overcut_total = deep_overcut_locator(label, &s, &bm, group);
    write_surface_renders(label, &s, group);

    if has_op_b {
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
        deep_overcut_total,
    }
}

// ── the cascade A/B (process proof, design doc §0.a) ────────────────────

#[test]
#[ignore = "two full scaled-wanaka chains + 0.25mm measurement sims (long); run with --ignored --nocapture"]
fn v3_cascade_ab_ball3() {
    let path = write_fixture_project(3.0);

    let d = score_branch("v3_D_allover_tip", &path, Branch::AllOverTip);
    let c = score_branch("v3_cascade_b3", &path, Branch::Cascade);
    verdict("ball Ø3, wanaka x2, SHIPPED dials", &d, &c);
}

/// The process proof re-run with the §9/§10 dials on, applied to BOTH
/// branches so the comparison stays honest — the air-cut filter is a shared
/// dressup and speeds up the all-over baseline too.
///
/// §10 measured Op B alone at -34.5% with these dials (38 868 -> 25 474 s).
/// This answers the question that actually matters: does the cascade now
/// beat the all-over-tip pass on time at equal COLUMNS quality — the §0.a
/// contract the campaign has been chasing since it opened.
/// Install a stderr tracing subscriber for a probe run.
///
/// Default filter carries `warn` for the whole crate on top of the
/// unified_finish info stream, because the campaign's most expensive
/// defect — scallop silently truncating its ring cascade and leaving a
/// 28 mm block of standing material — was invisible for weeks purely
/// because no harness run had a subscriber installed to show its
/// warning. A warning nobody sees is not a warning.
fn init_probe_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("warn,rs_cam_core::unified_finish=info")
            }),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .try_init();
}

#[test]
#[ignore = "two full scaled-wanaka chains + 0.25mm measurement sims (long); run with --ignored --nocapture"]
fn v3_process_proof_ab() {
    init_probe_tracing();

    let (which, dials) = dials_from_env("both");
    eprintln!("== PROCESS PROOF at dials: {which} => {dials:?} ==");
    let path = write_fixture_project(3.0);
    let d = score_branch_with(
        &format!("v3_D_allover_tip_{which}"),
        &path,
        Branch::AllOverTip,
        dials,
    );
    let c = score_branch_with(
        &format!("v3_cascade_b3_{which}"),
        &path,
        Branch::Cascade,
        dials,
    );
    verdict(&format!("ball Ø3, wanaka x2, dials={which}"), &d, &c);
}

/// Isolate WHICH §9/§10 dial causes the over-cut the process-proof COLUMNS
/// gate caught. Scores the CASCADE branch only (one chain + one measurement
/// sim, so it survives the memory pressure the two-branch proof does not) at
/// one dial setting chosen by `V3_DIALS`:
///
/// * `shipped`  — the pinned reference (measured: shallow `<-.5` = 1 218)
/// * `bridges`  — §10 cost-aware air bridges alone
/// * `links`    — §9 intra-region stay-down links alone
/// * `both`     — measured: shallow `<-.5` = 4 068, worst −3.95mm
///
/// The boundary check (`bddf82a`) was hypothesised as the cause and is
/// REFUTED: it refused only 50 of ~1 584 junctions and moved Op B by +0.2%
/// and removed volume by −0.3%. Removal splits roughly evenly between the
/// two dials (links ≈ +1 860 mm³, bridges ≈ +1 732 mm³), so this reads the
/// COLUMNS consequence of each rather than guessing from volume.
#[test]
#[ignore = "one cascade chain + measurement sim; set V3_DIALS=shipped|bridges|links|both"]
fn v3_cascade_dial_isolation() {
    init_probe_tracing();
    let (which, dials) = dials_from_env("both");
    eprintln!("== DIAL ISOLATION: {which} => {dials:?} ==");
    let path = write_fixture_project(3.0);
    let c = score_branch_with(
        &format!("v3_cascade_b3_{which}"),
        &path,
        Branch::Cascade,
        dials,
    );
    const BAND_LABEL: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];
    for (code, band_label) in BAND_LABEL.iter().enumerate().skip(1) {
        eprintln!(
            "{band_label:<11}: on-size={:5.1}% (n={:>7}) '>+.5' tail={}",
            c.on_size_by_band[code], c.n_by_band[code], c.tail_by_band[code],
        );
    }
    let finish_s: f64 = c
        .outcome
        .per_op_s
        .iter()
        .filter(|(n, _)| n == "Op A Ball Finish" || n == "Op B Unified Rest")
        .map(|(_, s)| *s)
        .sum();
    eprintln!(
        "[{which}] finish_stack={finish_s:.1}s collisions={}",
        c.outcome.collisions
    );
}

/// Localize the cascade's SHALLOW deficit by scoring a PARTIAL chain.
/// The gate blocker is pre-existing — at SHIPPED dials the cascade reads
/// shallow on-size 15.2% vs D's 19.2%, so it fails before any §9/§10 dial
/// is touched. The shipped histograms decompose that 4pp into two
/// unrelated populations:
///
/// * a gouge — 1 218 columns below −0.5mm, worst −3.03mm, against D's 8
///   at worst −0.58mm; and
/// * a wider mid-range leftover spread (+0.1..+0.5 up ~3 350 columns)
///   traded against a much smaller far tail (`>+.5` 1 282 vs D's 2 964).
///
/// Only the chain can say which op owns each. `V3_STAGE` picks how far
/// down the chain to run — `rough`, `opa` (Rough + Op A), `opb` (the full
/// cascade), `d` (Rough + D) — all at SHIPPED dials. Whichever stage the
/// deep population first appears at owns the gouge.
#[test]
#[ignore = "one partial chain + measurement sim; set V3_STAGE=rough|opa|opb|d"]
fn v3_shallow_deficit_localize() {
    init_probe_tracing();
    let stage = std::env::var("V3_STAGE").unwrap_or_else(|_| "opa".to_owned());
    let (enable, disable, ref_name): (&[&str], &[&str], &str) = match stage.as_str() {
        "rough" => (
            &["Rough"],
            &["Op A Ball Finish", "Op B Unified Rest", "D All-Over Tip"],
            "Rough",
        ),
        "opa" => (
            &["Rough", "Op A Ball Finish"],
            &["Op B Unified Rest", "D All-Over Tip"],
            "Op A Ball Finish",
        ),
        "opb" => (
            &["Rough", "Op A Ball Finish", "Op B Unified Rest"],
            &["D All-Over Tip"],
            "Op B Unified Rest",
        ),
        "d" => (
            &["Rough", "D All-Over Tip"],
            &["Op A Ball Finish", "Op B Unified Rest"],
            "D All-Over Tip",
        ),
        other => panic!("V3_STAGE must be rough|opa|opb|d, got {other:?}"),
    };
    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== SHALLOW DEFICIT LOCALIZE: stage={stage} dials={dial_label} ==");
    let path = write_fixture_project(3.0);
    let c = score_stage(
        &format!("v3_stage_{stage}_{dial_label}"),
        &path,
        enable,
        disable,
        ref_name,
        dials,
    );
    const BAND_LABEL: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];
    for (code, band_label) in BAND_LABEL.iter().enumerate().skip(1) {
        eprintln!(
            "[{stage}] {band_label:<11}: on-size={:5.1}% (n={:>7}) '>+.5' tail={}",
            c.on_size_by_band[code], c.n_by_band[code], c.tail_by_band[code],
        );
    }
    eprintln!(
        "[{stage}] project_total={:.1}s collisions={}",
        c.outcome.project_total_s, c.outcome.collisions
    );
}

/// Read the actual MOVES at a gouge site instead of A/B-ing around it.
///
/// §11 localized the cascade's deep over-cut to Op B and then ran out of
/// road: removing the crease node's unbounded links left the worst column
/// byte-identical (−5.565 mm at (198.75, 52.75)), and `pencil::
/// lift_to_surface` proves crease path Z is a drop-cutter result, so
/// neither candidate mechanism can put the tool 5.5 mm under the model.
/// Whatever does is in the emitted move list, so this prints it: every
/// cutting move passing within `V3_SITE_R` of `V3_SITE`, with its Z, type,
/// intent and enclosing spans, for each enabled op.
///
/// Deliberately skips the 0.25 mm measurement sim — this asks what the
/// toolpath CONTAINS, not what the stock ends up as.
#[test]
#[ignore = "one cascade chain, no measurement sim; set V3_SITE=x,y and V3_SITE_R"]
fn v3_gouge_site_probe() {
    init_probe_tracing();
    use rs_cam_core::toolpath::MoveType;

    let site = std::env::var("V3_SITE").unwrap_or_else(|_| "198.75,52.75".to_owned());
    let (sx, sy) = site
        .split_once(',')
        .unwrap_or_else(|| panic!("V3_SITE must be 'x,y', got {site:?}"));
    let (sx, sy): (f64, f64) = (
        sx.trim().parse().expect("V3_SITE x"),
        sy.trim().parse().expect("V3_SITE y"),
    );
    let radius: f64 = std::env::var("V3_SITE_R")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(1.5);
    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== GOUGE SITE PROBE at ({sx}, {sy}) r={radius} dials={dial_label} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("gouge_site", &mut s);

    // Distance from the site to the XY segment (from -> to).
    let seg_dist = |a: rs_cam_core::geo::P3, b: rs_cam_core::geo::P3| -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 < 1e-12 {
            0.0
        } else {
            (((sx - a.x) * dx + (sy - a.y) * dy) / len2).clamp(0.0, 1.0)
        };
        let (px, py) = (a.x + dx * t, a.y + dy * t);
        ((sx - px).powi(2) + (sy - py).powi(2)).sqrt()
    };

    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        if !tc.enabled {
            continue;
        }
        let name = tc.name.clone();
        let Some(result) = s.get_result(i) else {
            continue;
        };
        let ann = result.annotated();
        let moves = &ann.toolpath.moves;
        let mut near: Vec<(usize, rs_cam_core::geo::P3)> = Vec::new();
        let mut min_z = f64::INFINITY;
        eprintln!("== [{name}] moves within {radius}mm of ({sx}, {sy}) ==");
        for (k, m) in moves.iter().enumerate() {
            let from = k
                .checked_sub(1)
                .and_then(|j| moves.get(j))
                .map_or(m.target, |p| p.target);
            if seg_dist(from, m.target) > radius {
                continue;
            }
            min_z = min_z.min(from.z.min(m.target.z));
            near.push((k, from));
        }
        let hits = near.len();
        // Print the DEEPEST moves, not the first N — the first N are
        // whatever the emission order happened to put there, and the
        // question is what reached furthest down.
        near.sort_by(|a, b| {
            let za = moves
                .get(a.0)
                .map_or(f64::INFINITY, |m| m.target.z.min(a.1.z));
            let zb = moves
                .get(b.0)
                .map_or(f64::INFINITY, |m| m.target.z.min(b.1.z));
            za.total_cmp(&zb)
        });
        for &(k, from) in near.iter().take(30) {
            let Some(m) = moves.get(k) else { continue };
            let kind = match m.move_type {
                MoveType::Rapid => "RAPID",
                MoveType::Linear { .. } => "feed",
                MoveType::ArcCW { .. } => "arcCW",
                MoveType::ArcCCW { .. } => "arcCCW",
            };
            let spans: Vec<&str> = ann
                .spans
                .iter()
                .filter(|sp| sp.start_move <= k && k < sp.end_move.max(sp.start_move + 1))
                .map(|sp| &*sp.label)
                .collect();
            eprintln!(
                "  #{k:<7} {kind:<6} ({:8.3},{:8.3},{:8.3}) -> ({:8.3},{:8.3},{:8.3}) intent={:?} spans={:?}",
                from.x, from.y, from.z, m.target.x, m.target.y, m.target.z, m.intent, spans
            );
        }
        eprintln!("[{name}] {hits} moves near the site; lowest Z touched = {min_z:.3}");
    }
}

/// Was each cutting move ever LEGAL? — a chord-vs-drop-cutter gouge check.
///
/// §11 ruled out five candidate mechanisms for the cascade's deep
/// over-cut and left one: a straight feed chord between two valid CL
/// points can pass INSIDE the material, and `scallop::refine_chord`
/// checks nothing when `len < 2 × probe_step` — which on Op A's Ø3 ball
/// is most of its ring chords (0.28 mm spacing against a 0.375 mm probe
/// floor), including the near-vertical cliff chords that drop 2.6 mm of Z
/// per 0.18 mm of XY.
///
/// So: probe each cutting move's interior against the drop-cutter surface
/// it should be riding. `gouge = cl.z − chord_z` — positive means the tool
/// is BELOW where the cutter can legally sit, i.e. into the model. This
/// needs no dexel sim and no reference stock; it asks only whether the
/// emitted geometry was ever valid, which is a question the dexel
/// instrument cannot answer and `bridge_corridor_is_swept` only asks of
/// link corridors.
#[test]
#[ignore = "one cascade chain + a drop-cutter probe per cutting move"]
fn v3_chord_gouge_probe() {
    use rs_cam_core::toolpath::MoveType;

    let (dial_label, dials) = dials_from_env("shipped");
    let probe_step: f64 = std::env::var("V3_CHORD_STEP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.05);
    eprintln!("== CHORD GOUGE PROBE dials={dial_label} probe_step={probe_step} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("chord_gouge", &mut s);

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("terrain mesh");
    let index = rs_cam_core::mesh::SpatialIndex::build(&mesh, 10.0);

    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        if !tc.enabled {
            continue;
        }
        let (name, tool_id) = (tc.name.clone(), tc.tool_id);
        let Some(tool_cfg) = s.tools().iter().find(|t| t.id.0 == tool_id).cloned() else {
            continue;
        };
        let cutter = rs_cam_core::compute::cutter::build_cutter(&tool_cfg);
        let Some(result) = s.get_result(i) else {
            continue;
        };
        let moves = result.annotated().toolpath.moves.clone();

        // (gouge_mm, x, y, chord_z, cl_z, move_index)
        let mut worst: Vec<(f64, f64, f64, f64, f64, usize)> = Vec::new();
        let mut probed = 0usize;
        let mut over_tol = 0usize;
        let mut over_half = 0usize;
        for (k, m) in moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let Some(prev) = k.checked_sub(1).and_then(|j| moves.get(j)) else {
                continue;
            };
            let (a, b) = (prev.target, m.target);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let xy_len = (dx * dx + dy * dy).sqrt();
            if xy_len < 1e-9 {
                continue; // pure plunge/retract: no chord to check
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = ((xy_len / probe_step).ceil() as usize).clamp(2, 64);
            for si in 1..steps {
                let t = si as f64 / steps as f64;
                let (x, y) = (a.x + dx * t, a.y + dy * t);
                let cl = rs_cam_core::dropcutter::point_drop_cutter(x, y, &mesh, &index, &cutter);
                if !cl.contacted || !cl.z.is_finite() {
                    continue;
                }
                probed += 1;
                let chord_z = a.z + (b.z - a.z) * t;
                let gouge = cl.z - chord_z;
                if gouge > 0.05 {
                    over_tol += 1;
                }
                if gouge > 0.5 {
                    over_half += 1;
                    worst.push((gouge, x, y, chord_z, cl.z, k));
                }
            }
        }
        worst.sort_by(|p, q| q.0.total_cmp(&p.0));
        eprintln!(
            "== [{name}] CHORD GOUGE: probed={probed} >0.05mm={over_tol} >0.5mm={over_half} =="
        );
        for (g, x, y, cz, clz, k) in worst.iter().take(10) {
            eprintln!(
                "   gouge={g:6.3} at ({x:8.2},{y:8.2}) chord_z={cz:7.3} cl_z={clz:7.3} move #{k}"
            );
        }
    }
}

/// Does the swept CUTTER penetrate the model? — the flank check.
///
/// `v3_chord_gouge_probe` asks whether a move's CENTRELINE sits below the
/// drop-cutter surface. That is necessary but not sufficient: a tool
/// perfectly on its CL surface still removes everything inside its own
/// solid, and on concave or steep ground the part doing the removing is
/// the cutter's FLANK, several millimetres from the axis. Fixing chord
/// fidelity made the COLUMNS gate WORSE precisely because a more faithful
/// centreline drags the flank through more material (§11), so the flank
/// is what the remaining over-cut most likely is.
///
/// For a cutter whose tip sits at `z_t`, `MillingCutter::height_at_radius`
/// gives the profile height above the tip at radial distance `r`, so the
/// cutter's solid at that radius spans `z_t + h(r)` up to
/// `z_t + cutting_length`. A model vertex is INSIDE that solid when it
/// sits ABOVE the lower profile and below the flute top, and
/// `p.z - (z_t + h(r))` is how far in. (Getting this inequality backwards
/// measures "model is below the tool", which is the UNCUT side and reads
/// as tens of millimetres of nonsense on every roughing move.)
///
/// Approximation, stated because it bounds what a null result means: this
/// samples model VERTICES, not triangle interiors, so a facet that dips
/// inside the cutter between its corners is missed. On this fixture
/// (220 k triangles over 200 mm, so ~0.4 mm facets against a 1.5 mm ball)
/// vertices are dense relative to the cutter; on a coarse mesh they would
/// not be.
#[test]
#[ignore = "one cascade chain + a swept-cutter probe per cutting move"]
fn v3_flank_gouge_probe() {
    use rs_cam_core::toolpath::MoveType;

    let (dial_label, dials) = dials_from_env("shipped");
    let step: f64 = std::env::var("V3_FLANK_STEP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.2);
    eprintln!("== FLANK GOUGE PROBE dials={dial_label} step={step} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("flank_gouge", &mut s);

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("terrain mesh");
    let index = rs_cam_core::mesh::SpatialIndex::build(&mesh, 10.0);

    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        if !tc.enabled {
            continue;
        }
        let (name, tool_id) = (tc.name.clone(), tc.tool_id);
        let Some(tool_cfg) = s.tools().iter().find(|t| t.id.0 == tool_id).cloned() else {
            continue;
        };
        let cutter = rs_cam_core::compute::cutter::build_cutter(&tool_cfg);
        let radius = rs_cam_core::tool::MillingCutter::radius(&cutter);
        let flute_top = rs_cam_core::tool::MillingCutter::length(&cutter);
        let Some(result) = s.get_result(i) else {
            continue;
        };
        let moves = result.annotated().toolpath.moves.clone();

        let mut worst: Vec<(f64, f64, f64, usize)> = Vec::new();
        let mut samples = 0usize;
        let (mut over_tol, mut over_half) = (0usize, 0usize);
        for (k, m) in moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let Some(prev) = k.checked_sub(1).and_then(|j| moves.get(j)) else {
                continue;
            };
            let (a, b) = (prev.target, m.target);
            let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
            let len = (dx * dx + dy * dy + dz * dz).sqrt();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = ((len / step).ceil() as usize).clamp(1, 32);
            for si in 0..=n {
                let t = si as f64 / n as f64;
                let (tx, ty, tz) = (a.x + dx * t, a.y + dy * t, a.z + dz * t);
                samples += 1;
                let mut deepest = 0.0f64;
                for &fi in &index.query(tx, ty, radius) {
                    let Some(tri) = mesh.faces.get(fi) else {
                        continue;
                    };
                    for p in tri.v {
                        let r = ((p.x - tx).powi(2) + (p.y - ty).powi(2)).sqrt();
                        let Some(h) =
                            rs_cam_core::tool::MillingCutter::height_at_radius(&cutter, r)
                        else {
                            continue; // outside the cutter's profile
                        };
                        // Inside the flutes only: above the lower profile,
                        // below the flute top. The shank/holder is a
                        // separate collision question, not a gouge.
                        let lower = tz + h;
                        if p.z <= lower || p.z >= tz + flute_top {
                            continue;
                        }
                        let pen = p.z - lower;
                        if pen > deepest {
                            deepest = pen;
                        }
                    }
                }
                if deepest > 0.05 {
                    over_tol += 1;
                }
                if deepest > 0.5 {
                    over_half += 1;
                    worst.push((deepest, tx, ty, k));
                }
            }
        }
        worst.sort_by(|p, q| q.0.total_cmp(&p.0));
        eprintln!(
            "== [{name}] FLANK GOUGE (r={radius:.2}): samples={samples} \
             >0.05mm={over_tol} >0.5mm={over_half} =="
        );
        for (d, x, y, k) in worst.iter().take(10) {
            eprintln!("   penetration={d:6.3} tool at ({x:8.2},{y:8.2}) move #{k}");
        }
    }
}

/// WHICH OP dropped this column? — the per-op stock ladder.
///
/// Every attribution so far has been indirect: enable a subset and see
/// what the totals do. `prior_stocks` makes it direct — each op's entry is
/// the stock snapshot taken BEFORE it carved, so the column top read out
/// of successive snapshots is the ladder, and the op whose entry differs
/// from its successor's is the op that removed the material.
///
/// This exists because both gouge probes only inspect emitted CUTTING
/// moves, and at the worst deep column (198.75, 52.75) — model top 7.335,
/// stock left at 1.770 — `v3_gouge_site_probe` found no op with a cutting
/// move anywhere near that Z. Something removed 5.5 mm there. This says
/// what.
#[test]
#[ignore = "one cascade chain + measurement sim; set V3_SITES=x,y;x,y;..."]
fn v3_column_ladder_probe() {
    init_probe_tracing();
    let sites_raw = std::env::var("V3_SITES")
        .unwrap_or_else(|_| "198.75,52.75;35.50,149.75;51.25,123.75;99.00,129.75".to_owned());
    let sites: Vec<(f64, f64)> = sites_raw
        .split(';')
        .filter_map(|s| {
            let (a, b) = s.trim().split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect();
    assert!(
        !sites.is_empty(),
        "V3_SITES parsed empty from {sites_raw:?}"
    );
    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== COLUMN LADDER dials={dial_label} sites={sites:?} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("column_ladder", &mut s);
    run_measurement_sim(&mut s);

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("terrain mesh");
    let order: Vec<(String, rs_cam_core::ToolpathId)> = (0..s.toolpath_count())
        .filter_map(|i| s.get_toolpath_config(i))
        .filter(|tc| tc.enabled)
        .map(|tc| (tc.name.clone(), tc.id))
        .collect();
    let sim = s.simulation_result().expect("sim result");

    for (x, y) in sites {
        let origin = rs_cam_core::geo::P3::new(x, y, 1.0e6);
        let dir = rs_cam_core::geo::V3::new(0.0, 0.0, -1.0);
        let model_z = rs_cam_core::mesh::ray_pick_triangle(&mesh, &origin, &dir)
            .map_or(f64::NAN, |(_, t)| 1.0e6 - t);
        eprintln!("-- site ({x:.2}, {y:.2}) model_z={model_z:.3} --");
        for (name, id) in &order {
            let top = sim.prior_stocks.get(id).and_then(|st| {
                let g = &st.z_grid;
                g.world_to_cell(x, y).and_then(|(r, c)| g.top_z_at(r, c))
            });
            match top {
                Some(t) => eprintln!(
                    "   before {name:<20} top={:8.3}  (dev {:+7.3})",
                    t,
                    f64::from(t) - model_z
                ),
                None => eprintln!("   before {name:<20} top=<none>"),
            }
        }
        let final_top = sim.column_deviations.as_ref().and_then(|cols| {
            cols.iter()
                .filter(|cd| (cd.x - x).abs() < 0.2 && (cd.y - y).abs() < 0.2)
                .min_by(|p, q| {
                    let dp = (p.x - x).powi(2) + (p.y - y).powi(2);
                    let dq = (q.x - x).powi(2) + (q.y - y).powi(2);
                    dp.total_cmp(&dq)
                })
                .map(|cd| (cd.top_z, cd.dev))
        });
        match final_top {
            Some((t, dev)) => {
                eprintln!("   FINAL (columns)          top={t:8.3}  (dev {dev:+7.3})");
            }
            None => eprintln!("   FINAL (columns)          <no column within 0.2mm>"),
        }
    }
}

/// Is the simulation's Op B stamp faithful to Op B's toolpath?
///
/// §11a's blocker: the per-op ladder says Op B removed 5.5 mm from a
/// column whose neighbourhood contains no Op B move below Z 2.899 (final
/// top 1.770), and the tapered profile cannot reach it from 1.6 mm away.
/// Either the stamp over-removes or `prior_stocks` does not mean what the
/// ladder assumes — and every gouge figure in §11 rests on which.
///
/// Op B is the last enabled op, so re-stamping ITS toolpath onto ITS OWN
/// pre-carve snapshot must reproduce the final stock exactly. Comparison
/// is by `(row, col)` straight off `ColumnDeviation` — `prior_stocks`
/// snapshots share the grid, and inverse-transforming XY is the mistake
/// P2.g already paid for.
///
/// Equal → the sim is faithful, the reach argument is wrong, and §11's
/// numbers stand. Different → the stamp is a simulation defect, which
/// outranks this campaign.
#[test]
#[ignore = "one cascade chain + measurement sim + a full Op B re-stamp"]
fn v3_restamp_probe() {
    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== OP B RE-STAMP PROBE dials={dial_label} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("restamp", &mut s);
    run_measurement_sim(&mut s);

    // Op B must be the LAST enabled op for the re-stamp to be comparable.
    let enabled: Vec<String> = (0..s.toolpath_count())
        .filter_map(|i| s.get_toolpath_config(i))
        .filter(|tc| tc.enabled)
        .map(|tc| tc.name.clone())
        .collect();
    assert_eq!(
        enabled.last().map(String::as_str),
        Some("Op B Unified Rest"),
        "re-stamp assumes Op B carves last; enabled chain is {enabled:?}"
    );

    let op_b = s.get_toolpath_config(idx).expect("Op B config");
    let (op_b_id, op_b_tool) = (op_b.id, op_b.tool_id);
    let tool_cfg = s
        .tools()
        .iter()
        .find(|t| t.id.0 == op_b_tool)
        .cloned()
        .expect("Op B tool");
    let cutter = rs_cam_core::compute::cutter::build_cutter(&tool_cfg);
    let toolpath = s
        .get_result(idx)
        .expect("Op B result")
        .annotated()
        .toolpath
        .clone();

    let sim = s.simulation_result().expect("sim result");
    let prior = sim
        .prior_stocks
        .get(&op_b_id)
        .expect("Op B prior stock")
        .checkpoint();
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations")
        .clone();

    let mut restamp = prior;
    let never = std::sync::atomic::AtomicBool::new(false);
    restamp
        .simulate_toolpath_with_cancel(
            &toolpath,
            &cutter,
            rs_cam_core::dexel_stock::StockCutDirection::FromTop,
            &(|| never.load(std::sync::atomic::Ordering::SeqCst)),
        )
        .expect("re-stamp Op B");

    // Global agreement, indexed by (row, col) — no frame round-trip.
    let mut compared = 0usize;
    let mut worst = 0.0f64;
    let mut over_quantum = 0usize;
    let mut sim_lower = 0usize;
    let mut restamp_lower = 0usize;
    for cd in &cols {
        let Some(t) = restamp.z_grid.top_z_at(cd.row, cd.col) else {
            continue;
        };
        compared += 1;
        let d = f64::from(t) - f64::from(cd.top_z);
        if d.abs() > worst.abs() {
            worst = d;
        }
        if d.abs() > 0.01 {
            over_quantum += 1;
            if d > 0.0 {
                sim_lower += 1; // sim removed MORE than the re-stamp
            } else {
                restamp_lower += 1;
            }
        }
    }
    eprintln!(
        "== RE-STAMP vs SIM FINAL: compared={compared} |Δ|>0.01mm={over_quantum} \
         (sim removed more: {sim_lower}, re-stamp removed more: {restamp_lower}) worst Δ={worst:+.4} =="
    );

    // The four §11a ladder columns, by name.
    for (x, y) in [
        (198.75, 52.75),
        (35.50, 149.75),
        (99.00, 129.75),
        (51.25, 123.75),
    ] {
        let Some(cd) = cols
            .iter()
            .filter(|cd| (cd.x - x).abs() < 0.2 && (cd.y - y).abs() < 0.2)
            .min_by(|p, q| {
                let dp = (p.x - x).powi(2) + (p.y - y).powi(2);
                let dq = (q.x - x).powi(2) + (q.y - y).powi(2);
                dp.total_cmp(&dq)
            })
        else {
            eprintln!("   site ({x:.2},{y:.2}): no column within 0.2mm");
            continue;
        };
        let rs = restamp.z_grid.top_z_at(cd.row, cd.col);
        eprintln!(
            "   site ({x:.2},{y:.2}) [r{},c{}] sim_top={:.3} restamp_top={:?} dev={:+.3}",
            cd.row, cd.col, cd.top_z, rs, cd.dev
        );
    }
}

/// WHICH MOVE dropped this column? — per-move stamp attribution.
///
/// Item 1 cleared `prior_stocks` and the simulation wrapper: re-stamping
/// Op B reproduces the sim exactly. So the reach contradiction lives in
/// the shared stamping path or in the site probe's move enumeration, and
/// only naming the responsible move can say which. Since the radial
/// profile height is never negative, no tool position at tip Z 2.899 can
/// leave a 1.770 top — so whatever move the stamp credits must either sit
/// lower than the site probe found, or be further away than its profile
/// can reach.
///
/// Stamps Op B one move at a time onto its own pre-carve snapshot with a
/// pre-built LUT (production's `simulate_toolpath_with_lut_cancel`, two
/// moves at a time so arcs still linearize), reading the target column
/// after each. Exact attribution in one pass.
#[test]
#[ignore = "one cascade chain + measurement sim + per-move Op B stamping"]
fn v3_move_attribution_probe() {
    let site = std::env::var("V3_SITE").unwrap_or_else(|_| "198.75,52.75".to_owned());
    let (sx, sy) = site.split_once(',').expect("V3_SITE must be 'x,y'");
    let (sx, sy): (f64, f64) = (
        sx.trim().parse().expect("site x"),
        sy.trim().parse().expect("site y"),
    );
    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== MOVE ATTRIBUTION at ({sx}, {sy}) dials={dial_label} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("move_attr", &mut s);
    run_measurement_sim(&mut s);

    let op_b = s.get_toolpath_config(idx).expect("Op B config");
    let (op_b_id, op_b_tool) = (op_b.id, op_b.tool_id);
    let tool_cfg = s
        .tools()
        .iter()
        .find(|t| t.id.0 == op_b_tool)
        .cloned()
        .expect("Op B tool");
    let cutter = rs_cam_core::compute::cutter::build_cutter(&tool_cfg);
    let moves = s
        .get_result(idx)
        .expect("Op B result")
        .annotated()
        .toolpath
        .moves
        .clone();

    let sim = s.simulation_result().expect("sim result");
    let cd = sim
        .column_deviations
        .as_ref()
        .expect("column deviations")
        .iter()
        .filter(|cd| (cd.x - sx).abs() < 0.2 && (cd.y - sy).abs() < 0.2)
        .min_by(|p, q| {
            let dp = (p.x - sx).powi(2) + (p.y - sy).powi(2);
            let dq = (q.x - sx).powi(2) + (q.y - sy).powi(2);
            dp.total_cmp(&dq)
        })
        .copied()
        .expect("a column at the site");
    let (row, col) = (cd.row, cd.col);
    let mut stock = sim
        .prior_stocks
        .get(&op_b_id)
        .expect("Op B prior stock")
        .checkpoint();

    let lut = rs_cam_core::radial_profile::RadialProfileLUT::from_cutter(
        &cutter,
        rs_cam_core::radial_profile::LUT_SAMPLES,
    );
    let radius = rs_cam_core::tool::MillingCutter::radius(&cutter);
    let never = std::sync::atomic::AtomicBool::new(false);
    let cancel = || never.load(std::sync::atomic::Ordering::SeqCst);

    let start_top = stock.z_grid.top_z_at(row, col);
    eprintln!(
        "   column [r{row},c{col}] at ({:.2},{:.2}) starts at {start_top:?}, sim final {:.3}, radius={radius:.2}",
        cd.x, cd.y, cd.top_z
    );

    let mut prev = start_top;
    let mut drops = 0usize;
    for i in 1..moves.len() {
        let mut two = rs_cam_core::toolpath::Toolpath::new();
        two.moves.push(moves[i - 1].clone());
        two.moves.push(moves[i].clone());
        stock
            .simulate_toolpath_with_lut_cancel(
                &two,
                &lut,
                radius,
                rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                &cancel,
            )
            .expect("stamp move");
        let now = stock.z_grid.top_z_at(row, col);
        if now != prev {
            drops += 1;
            let (a, b) = (moves[i - 1].target, moves[i].target);
            let d_xy = {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                let t = if l2 < 1e-12 {
                    0.0
                } else {
                    (((sx - a.x) * dx + (sy - a.y) * dy) / l2).clamp(0.0, 1.0)
                };
                ((sx - (a.x + dx * t)).powi(2) + (sy - (a.y + dy * t)).powi(2)).sqrt()
            };
            if drops <= 40 {
                eprintln!(
                    "   #{i:<7} {:?} {:?} ({:8.3},{:8.3},{:8.3})->({:8.3},{:8.3},{:8.3}) \
                     xy_dist={d_xy:6.3} top {prev:?} -> {now:?}",
                    moves[i].move_type, moves[i].intent, a.x, a.y, a.z, b.x, b.y, b.z
                );
            }
            prev = now;
        }
    }
    eprintln!(
        "   {drops} moves changed this column; final {prev:?} (sim {:.3})",
        cd.top_z
    );
}

/// Do any emitted arcs sweep the LONG way round? — arc direction sanity.
///
/// `v3_move_attribution_probe` found a single `ArcCW` whose endpoints are
/// 3.55 mm apart on a 57.04 mm-radius circle, sweeping from 71.34° to
/// 74.90°. Increasing angle is COUNTER-clockwise, so tagged CW it
/// commands 356° instead of 3.6° — a 114 mm-diameter circle through the
/// workpiece. The simulator cut along it (5.5 mm off a column 97 mm from
/// the move) and a machine would too. `arc_fitting` is a default-ON
/// dressup, so this is not campaign-specific.
///
/// Sweep is computed from the arc's own `i/j` and direction flag exactly
/// as a controller would, and anything beyond `MAX_PLAUSIBLE_SWEEP_DEG`
/// is reported.
///
/// READ THE RATIO, NOT THE COUNT. A reflex sweep is not automatically a
/// bug: raster turnarounds are deliberate ~270° loops at `stepover/√2`
/// (0.148 mm at this fixture's 0.21 mm stepover), and 353 of Op B's arcs
/// are exactly those. What marks a MIS-DIRECTED arc is arc length wildly
/// exceeding the path it replaced — the field case ran 354 mm of arc for
/// a 3.55 mm chord, a ratio of 100, against ~3.3 for a turnaround.
#[test]
#[ignore = "one cascade chain; pure toolpath analysis, no simulation"]
fn v3_arc_direction_sanity() {
    init_probe_tracing();
    use rs_cam_core::toolpath::MoveType;
    /// Beyond this, an arc between close endpoints is a direction error.
    const MAX_PLAUSIBLE_SWEEP_DEG: f64 = 200.0;

    let (dial_label, dials) = dials_from_env("shipped");
    eprintln!("== ARC DIRECTION SANITY dials={dial_label} ==");
    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path).expect("load fixture");
    let (enable, disable) = branch_ops(Branch::Cascade);
    set_enabled_by_name(&mut s, enable, disable);
    let idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }
    run_chain("arc_sanity", &mut s);

    let mut grand_total = 0usize;
    let mut grand_bad = 0usize;
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        if !tc.enabled {
            continue;
        }
        let name = tc.name.clone();
        let Some(result) = s.get_result(i) else {
            continue;
        };
        let moves = &result.annotated().toolpath.moves;
        let (mut arcs, mut bad, mut worst) = (0usize, 0usize, 0.0f64);
        let mut shown = 0usize;
        for (k, m) in moves.iter().enumerate() {
            let (io, jo, cw) = match m.move_type {
                MoveType::ArcCW { i: io, j: jo, .. } => (io, jo, true),
                MoveType::ArcCCW { i: io, j: jo, .. } => (io, jo, false),
                _ => continue,
            };
            let Some(prev) = k.checked_sub(1).and_then(|j| moves.get(j)) else {
                continue;
            };
            arcs += 1;
            let (a, b) = (prev.target, m.target);
            let (cx, cy) = (a.x + io, a.y + jo);
            let a0 = (a.y - cy).atan2(a.x - cx);
            let a1 = (b.y - cy).atan2(b.x - cx);
            // Controller semantics: CW decreases angle, CCW increases;
            // wrap into (0, 2pi].
            let mut sweep = if cw { a0 - a1 } else { a1 - a0 };
            while sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let deg = sweep.to_degrees();
            let chord = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let r = ((a.x - cx).powi(2) + (a.y - cy).powi(2)).sqrt();
            if deg > worst {
                worst = deg;
            }
            if deg > MAX_PLAUSIBLE_SWEEP_DEG {
                bad += 1;
                shown += 1;
                if shown <= 6 {
                    eprintln!(
                        "   [{name}] #{k} sweep={deg:7.2}° r={r:8.3} chord={chord:6.3} \
                         arc_len={:9.3} len/chord={:7.1} ({:.3},{:.3})->({:.3},{:.3})",
                        r * sweep,
                        r * sweep / chord.max(1e-9),
                        a.x,
                        a.y,
                        b.x,
                        b.y
                    );
                }
            }
        }
        grand_total += arcs;
        grand_bad += bad;
        eprintln!(
            "== [{name}] arcs={arcs} sweeping>{MAX_PLAUSIBLE_SWEEP_DEG}°={bad} worst={worst:.2}° =="
        );
    }
    eprintln!("== TOTAL arcs={grand_total} reflex/mis-directed={grand_bad} ==");
}

/// Render the MACHINED SURFACE, not a statistic.
///
/// Every gate in this campaign reads aggregates over `column_deviations`,
/// and aggregates cannot show tool marks, ridge direction, banding at
/// region seams, or a gouge you would notice instantly by eye. The
/// existing `{tag}_deviation.png` is vertex-based — the instrument P2.g
/// Task 1 showed averages machined texture away — and it answers "how far
/// from the model", which is not the same question as "does this look
/// like an acceptable cut".
///
/// So this hillshades `ColumnDeviation::top_z` (the same data the gate
/// reads) as a relief map, indexed by `(row, col)` with no frame
/// round-trip, and writes a matching deviation map on the SAME data for
/// side-by-side reading. Zoom with `V3_CROP=x,y,size_mm`.
fn write_surface_renders(tag: &str, s: &ProjectSession, group: usize) {
    let Some(sim) = s.simulation_result() else {
        return;
    };
    let Some(cols) = sim.column_deviations.as_ref() else {
        return;
    };
    let mine: Vec<&rs_cam_core::compute::simulate::ColumnDeviation> =
        cols.iter().filter(|cd| cd.group == group).collect();
    if mine.is_empty() {
        return;
    }
    let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
    for cd in &mine {
        r0 = r0.min(cd.row);
        r1 = r1.max(cd.row);
        c0 = c0.min(cd.col);
        c1 = c1.max(cd.col);
    }
    let (rows, cw) = (r1 - r0 + 1, c1 - c0 + 1);
    let mut z = vec![f32::NAN; rows * cw];
    let mut dev = vec![f32::NAN; rows * cw];
    for cd in &mine {
        let i = (cd.row - r0) * cw + (cd.col - c0);
        z[i] = cd.top_z;
        dev[i] = cd.dev;
    }

    // Hillshade: classic Lambertian on the height field, light from the
    // NW at 45°. Tool marks are sub-10µm ripples on a ±14mm terrain, so
    // the gradient is scaled hard — that is the point, an unexaggerated
    // relief of this surface shows nothing.
    const GAIN: f64 = 400.0;
    let at = |r: usize, c: usize| -> Option<f64> {
        if r >= rows || c >= cw {
            return None;
        }
        let v = z[r * cw + c];
        v.is_finite().then(|| f64::from(v))
    };
    let mut px = vec![0u8; rows * cw * 4];
    let mut dpx = vec![0u8; rows * cw * 4];
    for r in 0..rows {
        for c in 0..cw {
            let i = r * cw + c;
            let o = i * 4;
            if let (Some(h), Some(hx), Some(hy)) = (
                at(r, c),
                at(r, c.saturating_sub(1)),
                at(r.saturating_sub(1), c),
            ) {
                let (dzdx, dzdy) = ((h - hx) * GAIN, (h - hy) * GAIN);
                let n = (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
                // Light direction (-1,-1,1)/√3.
                let lam = ((-dzdx - dzdy + 1.0) / (n * 3.0_f64.sqrt())).clamp(0.0, 1.0);
                let v = (40.0 + 215.0 * lam) as u8;
                px[o] = v;
                px[o + 1] = v;
                px[o + 2] = v;
                px[o + 3] = 255;
            }
            let d = dev[i];
            if d.is_finite() {
                // Diverging, saturating at ±0.1mm so ordinary cusp
                // texture is visible rather than a flat grey field.
                let t = (f64::from(d) / 0.1).clamp(-1.0, 1.0);
                let (rr, gg, bb) = if t < 0.0 {
                    (255, (255.0 * (1.0 + t)) as u8, (255.0 * (1.0 + t)) as u8)
                } else {
                    ((255.0 * (1.0 - t)) as u8, (255.0 * (1.0 - t)) as u8, 255)
                };
                dpx[o] = rr;
                dpx[o + 1] = gg;
                dpx[o + 2] = bb;
                dpx[o + 3] = 255;
            }
        }
    }
    // Flip vertically so +Y is up, matching every other render here.
    let flip = |src: &[u8]| -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for r in 0..rows {
            let sr = (rows - 1 - r) * cw * 4;
            let dr = r * cw * 4;
            out[dr..dr + cw * 4].copy_from_slice(&src[sr..sr + cw * 4]);
        }
        out
    };
    for (buf, kind) in [(flip(&px), "surface"), (flip(&dpx), "devcol")] {
        let p = v3_dir().join(format!("{tag}_{kind}.png"));
        image::save_buffer(&p, &buf, cw as u32, rows as u32, image::ColorType::Rgba8)
            .expect("save render");
        eprintln!("render: {}", p.display());
    }
}

/// What does the CUSP DIAL cost? — the lever the campaign never touched.
///
/// Both branches have targeted `scallop_height = 0.011` throughout, a
/// number inherited from `p2c_headless_ab_wanaka.rs` rather than chosen
/// for this part. It sets essentially the whole runtime: stepover goes as
/// √cusp, so pass count — and therefore time — goes as 1/√cusp. Two
/// sessions of strategy work moved ±2%; this dial moves the answer by
/// integer factors.
///
/// On hardwood an 11 µm cusp is far below what survives grain, moisture
/// or one pass of sandpaper, so the interesting question is not "is 11 µm
/// achievable" but "what do we actually need". This measures the D branch
/// (Rough + all-over tip — the simplest complete process) across cusp
/// targets and reports time against the DEFECT metrics, not against the
/// ±10 µm bin the verdict established is unusable here.
///
/// A surface render lands per setting (`write_surface_renders`), which is
/// the part a machinist should judge — the numbers cannot say what finish
/// is acceptable, only what each one costs.
#[test]
#[ignore = "one full D chain + measurement sim PER cusp value; set V3_CUSPS"]
fn v3_cusp_sweep() {
    init_probe_tracing();
    let raw = std::env::var("V3_CUSPS").unwrap_or_else(|_| "0.011,0.025,0.05".to_owned());
    let cusps: Vec<f64> = raw
        .split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect();
    assert!(!cusps.is_empty(), "V3_CUSPS parsed empty from {raw:?}");
    eprintln!("== CUSP SWEEP (D branch: Rough + all-over Ø1 tip) cusps={cusps:?} ==");

    let path = write_fixture_project(3.0);
    let (enable, disable) = branch_ops(Branch::AllOverTip);
    /// One sweep row: what a cusp target cost and what it left behind.
    struct CuspRow {
        cusp: f64,
        stepover: f64,
        finish_s: f64,
        collisions: usize,
        tail: [usize; 4],
        deep: usize,
    }
    let mut rows: Vec<CuspRow> = Vec::new();

    for &cusp in &cusps {
        let tag = format!("v3_cusp_{cusp:.4}");
        // Theoretical flat stepover for the Ø1 tip, for the table.
        let stepover = 2.0 * (2.0 * 0.5 * cusp - cusp * cusp).max(0.0).sqrt();
        let c = score_stage_tweaked(
            &tag,
            &path,
            enable,
            disable,
            "D All-Over Tip",
            Dials::SHIPPED,
            &|s: &mut ProjectSession| {
                let d_idx = toolpath_index_by_name(s, "D All-Over Tip");
                s.set_toolpath_param(d_idx, "scallop_height", serde_json::json!(cusp))
                    .expect("set scallop_height");
            },
        );
        let finish_s: f64 = c
            .outcome
            .per_op_s
            .iter()
            .filter(|(n, _)| n == "D All-Over Tip")
            .map(|(_, v)| *v)
            .sum();
        rows.push(CuspRow {
            cusp,
            stepover,
            finish_s,
            collisions: c.outcome.collisions,
            tail: c.tail_by_band,
            deep: c.deep_overcut_total,
        });
    }

    eprintln!("\n== CUSP SWEEP RESULT (D branch) ==");
    eprintln!(
        "{:>8} {:>10} {:>11} {:>7} {:>11} {:>22}",
        "cusp mm", "stepover", "D time s", "collis", "vs 0.011", "standing (shallow/mid/steep)"
    );
    let base = rows.first().map_or(1.0, |r| r.finish_s);
    for r in &rows {
        eprintln!(
            "{:>8.4} {:>10.3} {:>11.1} {:>7} {:>10.1}% {:>7}/{:>6}/{:>6}  deep={}",
            r.cusp,
            r.stepover,
            r.finish_s,
            r.collisions,
            100.0 * (r.finish_s - base) / base.max(1e-9),
            r.tail[1],
            r.tail[2],
            r.tail[3],
            r.deep,
        );
    }
    eprintln!(
        "\nNOTE: on-size (±10µm) is deliberately NOT the verdict here — a bigger\n\
         cusp is SUPPOSED to miss that bin. Judge defects (collisions, standing\n\
         material) and the per-setting surface renders."
    );
}

/// Shared verdict printer + quality gate for the branch comparison.
fn verdict(what: &str, d: &BranchScore, c: &BranchScore) {
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

    eprintln!("== v3 CASCADE vs ALL-OVER-TIP VERDICT ({what}) ==");
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

/// Classification-only probe (no generation, ~3 min): the band-map coverage
/// accounting for the ×2 fixture.
///
/// **The "~17% of covered area reclaimed" conclusion this docstring used to
/// state is RETRACTED** (M1 / LH-4, 2026-07-29). It divided an overlap-
/// dilated XY-projected mm² sum by a boolean grid-cell count: two different
/// domains, and a numerator that double-counts the 2 mm dilation ring across
/// bands. Both errors push the figure in unknown directions, so the number is
/// void — not falsified. What the probe now reports is what it can honestly
/// measure: covered cells (and their mm² equivalent) on one line, per-band
/// extracted polygon area on another, and no ratio between them. Sizing how
/// much min-area absorption actually costs needs a same-domain, same-stage,
/// undilated comparison and is H4 work.
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

/// §10: the air-cut filter, not the emitter, is where Op B's fragments come
/// from. The relink telemetry measured the generator emitting 1 634
/// fragments while the shipped toolpath carries 15 373 — and `filter_air_cuts`
/// is the only pass that turns cutting moves into rapids. It bridges EVERY
/// air run to `safe_z` with no length test, so a 2 mm sliver of air costs a
/// ~35 mm retract round trip.
///
/// Crossed with the §9 linker, because the two interact: linking joins
/// fragments the filter would otherwise have to bridge back apart.
#[test]
#[ignore = "four cascade chains (~9 min); run with --ignored --nocapture"]
fn v3_air_bridge_probe() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("rs_cam_core::unified_finish=info")
            }),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .try_init();

    let project_path = write_fixture_project(3.0);
    let cases: [(&str, AirBridgePolicy, f64); 4] = [
        (
            "shipped (bridge always, no links)",
            AirBridgePolicy::Always,
            0.0,
        ),
        ("links only", AirBridgePolicy::Always, 6.0),
        (
            "cost-aware bridges only",
            AirBridgePolicy::ShorterThanAirPath,
            0.0,
        ),
        ("both", AirBridgePolicy::ShorterThanAirPath, 6.0),
    ];
    let mut rows: Vec<(&str, f64, f64, usize)> = Vec::new();

    for (label, policy, hookup) in cases {
        let mut c = ProjectSession::load(&project_path).unwrap_or_else(|e: SessionError| {
            panic!("failed to load {}: {e}", project_path.display())
        });
        apply_branch(&mut c, Branch::Cascade);
        let op_b_idx = toolpath_index_by_name(&c, "Op B Unified Rest");
        c.set_toolpath_operation(
            op_b_idx,
            OperationConfig::UnifiedFinish(op_b_claims_config_with_hookup(hookup)),
        )
        .expect("swap Op B config");
        // The policy applies to every op in the chain, not just Op B — the
        // filter is a shared dressup, so a per-op comparison would not be
        // the shipped-behaviour question.
        for i in 0..c.toolpath_count() {
            let Some(tc) = c.get_toolpath_config(i) else {
                continue;
            };
            let mut d = tc.dressups.clone();
            d.air_bridge_policy = policy;
            c.set_dressup_config(i, d).expect("set dressups");
        }
        let out = run_chain(label, &mut c);
        let op_b_s = out
            .per_op_s
            .iter()
            .find(|(name, _)| name == "Op B Unified Rest")
            .map_or(0.0, |(_, s)| *s);
        link_anatomy(label, &c, op_b_idx);
        tool_load_table(label, &c);
        rows.push((label, op_b_s, out.project_total_s, out.collisions));
    }

    eprintln!("== AIR-BRIDGE PROBE (wanaka x2, ball Ø3) ==");
    let base = rows.first().map_or(0.0, |r| r.1);
    eprintln!("case                               |    Op B s |  project s | coll | vs shipped");
    for (label, op_b_s, project_s, collisions) in &rows {
        eprintln!(
            "{label:<34} | {op_b_s:>9.1} | {project_s:>10.1} | {collisions:>4} | {:+.1}%",
            if base > 0.0 {
                100.0 * (op_b_s - base) / base
            } else {
                0.0
            }
        );
    }
}

/// What the relinker actually did to the emitted path: how many junctions
/// stayed on the surface vs still retract, and what the surviving rapids
/// cost. The generator's own `tracing::info!` tally needs a subscriber the
/// harness does not install, and this reads the shipped toolpath rather
/// than trusting the generator's self-report, which is the better
/// instrument anyway.
fn link_anatomy(label: &str, s: &ProjectSession, op_index: usize) {
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{MoveIntent, MoveType};

    let ann = s
        .get_result(op_index)
        .unwrap_or_else(|| panic!("[{label}] op {op_index} not generated"))
        .annotated();
    let moves = &ann.toolpath.moves;
    let d = |a: P3, b: P3| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();

    // A "fragment" here matches tsp/relink: a maximal run of non-rapid moves.
    let mut fragments = 0usize;
    let mut in_frag = false;
    let mut rapid_mm = 0.0f64;
    let mut rapid_n = 0usize;
    let mut link_mm = 0.0f64;
    let mut link_n = 0usize;
    let mut plunges = 0usize;
    let mut prev: Option<P3> = None;
    for mv in moves {
        let step = prev.map_or(0.0, |p| d(p, mv.target));
        if matches!(mv.move_type, MoveType::Rapid) {
            in_frag = false;
            rapid_n += 1;
            rapid_mm += step;
        } else {
            if !in_frag {
                fragments += 1;
                in_frag = true;
            }
            if mv.intent == MoveIntent::Linking {
                link_n += 1;
                link_mm += step;
            }
            if mv.intent == MoveIntent::EntryPlunge {
                plunges += 1;
            }
        }
        prev = Some(mv.target);
    }
    eprintln!(
        "== LINK ANATOMY [{label}] == moves={} fragments={fragments} \
         rapids={rapid_n} ({rapid_mm:.0}mm) linking_feeds={link_n} ({link_mm:.0}mm) \
         entry_plunges={plunges}",
        moves.len(),
    );
    eprintln!(
        "   junctions still retracting ~= {} of {} ({:.0}%)",
        plunges.saturating_sub(1),
        fragments.saturating_sub(1),
        100.0 * (plunges.saturating_sub(1)) as f64 / fragments.saturating_sub(1).max(1) as f64,
    );
}

/// §9 lever: intra-region stay-down linking, measured against its own
/// off state on the same fixture. Prints Op B's time and rapid share at
/// each `intra_region_hookup_mm`, plus the pass's link/retract tallies
/// (emitted by `unified_finish` via `tracing::info!`).
///
/// The theory §9 leaves standing: Op B's air is COUNT-bound — 12 780
/// fragment junctions, each paying two ~30 mm Z legs — so removing the
/// legs (this) should beat shortening the hop (the reorder, which bought
/// 4.6%). If this measures flat, the theory is wrong and the count itself
/// has to come down instead.
#[test]
#[ignore = "one cascade chain per dial (~10 min each); run with --ignored --nocapture"]
fn v3_intra_region_link_probe() {
    // Surface the generator's own relink tallies (why a junction refused to
    // link is not derivable from the emitted toolpath — a refused link and
    // a junction that was never a candidate look identical there).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("rs_cam_core::unified_finish=info")
            }),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .try_init();

    let project_path = write_fixture_project(3.0);
    let dials = [0.0_f64, 1.0, 3.0, 6.0];
    let mut rows: Vec<(f64, f64, f64, usize)> = Vec::new();

    for hookup in dials {
        let mut c = ProjectSession::load(&project_path).unwrap_or_else(|e: SessionError| {
            panic!("failed to load {}: {e}", project_path.display())
        });
        apply_branch(&mut c, Branch::Cascade);
        let op_b_idx = toolpath_index_by_name(&c, "Op B Unified Rest");
        c.set_toolpath_operation(
            op_b_idx,
            OperationConfig::UnifiedFinish(op_b_claims_config_with_hookup(hookup)),
        )
        .expect("swap Op B config");
        let out = run_chain(&format!("intra-region link hookup={hookup}"), &mut c);
        let op_b_s = out
            .per_op_s
            .iter()
            .find(|(name, _)| name == "Op B Unified Rest")
            .map_or(0.0, |(_, s)| *s);
        link_anatomy(&format!("hookup={hookup}"), &c, op_b_idx);
        tool_load_table(&format!("hookup={hookup}"), &c);
        rows.push((hookup, op_b_s, out.project_total_s, out.collisions));
    }

    eprintln!("== INTRA-REGION LINK PROBE (wanaka x2, ball Ø3) ==");
    eprintln!("hookup_mm |    Op B s |  project s | collisions | vs hookup=0");
    let base = rows.first().map_or(0.0, |r| r.1);
    for (hookup, op_b_s, project_s, collisions) in &rows {
        eprintln!(
            "{hookup:>9.1} | {op_b_s:>9.1} | {project_s:>10.1} | {collisions:>10} | {:+.1}%",
            if base > 0.0 {
                100.0 * (op_b_s - base) / base
            } else {
                0.0
            }
        );
    }
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

// ── STEP 1: rest anatomy — does the merge premise even hold? ────────────
//
// The reframed question (2026-07-28) is NOT "does staging beat one
// all-over pass" (closed: not provable on this fixture). It is the one
// `unified_finish` was actually built for and which no run has ever
// isolated:
//
//   the rest may need contour on the steeps, pencil in the valleys and
//   rings in between; if each is SPARSE, is merging them into ONE linked
//   op better than running them as separate sequential ops?
//
// Op B already implements exactly that mix — `VerySteep -> waterline`,
// `MidSteep -> scallop rings`, `Shallow -> raster`, `Crease -> pencil
// claims` (`unified_finish.rs` step 4). What has never been measured is
// whether the MERGE pays.
//
// The premise has a precondition, and this probe tests it before any A/B
// is built: merging can only pay if the strategies are sparse AND
// spatially interleaved, so that running them as separate ops would
// re-traverse the same ground once per strategy. If they are segregated,
// N sweeps cost about the same as one and the merge is worth nothing.
//
// Crucially, the merged and unfused arms would share the decomposition,
// the per-region strategy, the parameters and the tool — so the ONLY
// thing that differs is the order regions are visited in (plus a couple
// of op-level approaches). That makes the prize exactly:
//
//   (hop cost of a strategy-GROUPED tour) - (hop cost of a MIXED tour)
//
// computable offline from the emitted region geometry, with no second
// chain to generate and no measurement sim. Both tours are costed with
// `machine_kinematics::retract_link_time` — the same integrator the
// router itself uses — so the model's absolute accuracy cancels and only
// the delta is load-bearing.

/// The four strategies `unified_finish` dispatches, named by what they
/// actually emit rather than by band, because the band names are an
/// implementation detail and the user's question is about strategies.
///
/// `None` for anything else — critically, for the scallop emitter's own
/// `Ring N` annotations. Those are NOT routing units: `unified_finish_
/// spans` builds the annotation spans with `spans_from_labeled_events`
/// (each runs to the NEXT annotation) and then SPLICES the region-node
/// spans in as SIBLINGS, so the two families interleave instead of
/// nesting and `outer_region_spans` returns both. The router's nodes are
/// the `region_table`-derived ones alone; a `Ring` is sub-structure
/// inside one of them, and counting rings as regions inflates the region
/// count ~100× (measured: 620 outer spans, 615 of them rings).
///
/// C4: the four label literals this used to carry are gone. It asks
/// `RegionKind::from_span_label` — the inverse of the function that WROTE
/// the label — and takes the strategy off the kind, so the mapping cannot
/// drift from the producer and a label change is caught by
/// `unified_finish::tests::region_kind_span_labels_round_trip` rather than
/// by this harness silently classifying every node as `"other"`.
fn strategy_of_span(label: &str) -> Option<&'static str> {
    rs_cam_core::unified_finish::RegionKind::from_span_label(label)
        .map(|kind| kind.strategy().label())
}

/// Strategy visiting order for the UNFUSED stack: steep first, shallow
/// last, detail last of all — the order a human would sequence three
/// separate finishing ops in.
const UNFUSED_ORDER: [&str; 5] = ["waterline", "scallop", "raster", "pencil", "other"];

/// One of Op B's outer Region spans, reduced to what the merge question
/// needs: which strategy, how much cutting, where it starts and ends, and
/// which 1 mm footprint cells it touches.
struct RestRegion {
    strategy: &'static str,
    label: String,
    moves: usize,
    cutting_mm: f64,
    entry: rs_cam_core::geo::P3,
    exit: rs_cam_core::geo::P3,
    cells: std::collections::HashSet<(i32, i32)>,
}

/// Stamp a segment's 1 mm footprint cells, walking at 0.5 mm so no cell
/// is skipped. Footprint, not swept volume — this measures where the op
/// GOES, which is what decides whether two strategies share ground.
fn stamp_cells(
    cells: &mut std::collections::HashSet<(i32, i32)>,
    a: rs_cam_core::geo::P3,
    b: rs_cam_core::geo::P3,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    let steps = ((len / 0.5).ceil() as usize).max(1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = a.x + dx * t;
        let y = a.y + dy * t;
        cells.insert((x.floor() as i32, y.floor() as i32));
    }
}

/// Every 1 mm footprint cell an op's cutting moves touch.
fn op_footprint_cells(
    s: &ProjectSession,
    op_index: usize,
) -> std::collections::HashSet<(i32, i32)> {
    let mut cells = std::collections::HashSet::new();
    let Some(result) = s.get_result(op_index) else {
        return cells;
    };
    let moves = &result.annotated().toolpath.moves;
    for i in 1..moves.len() {
        if moves[i].move_type.is_cutting() {
            stamp_cells(&mut cells, moves[i - 1].target, moves[i].target);
        }
    }
    cells
}

/// Split Op B's toolpath into the ROUTER'S region nodes — the
/// `region_table`-derived spans only (see `strategy_of_span`), in move
/// order. Returns the nodes plus the count of emitter sub-structure spans
/// that were correctly excluded, so the caller can report the difference
/// rather than silently hide it.
fn rest_regions(s: &ProjectSession, op_b_idx: usize) -> (Vec<RestRegion>, usize) {
    use rs_cam_core::toolpath_spans::SpanKind;
    let result = s
        .get_result(op_b_idx)
        .unwrap_or_else(|| panic!("Op B (index {op_b_idx}) not generated"));
    let ann = result.annotated();
    let moves = &ann.toolpath.moves;
    let mut out = Vec::new();
    let mut sub_structure = 0usize;
    let mut nodes: Vec<&rs_cam_core::toolpath_spans::Span> = ann
        .spans
        .iter()
        .filter(|sp| sp.kind == SpanKind::Region)
        .filter(|sp| {
            if strategy_of_span(&sp.label).is_some() {
                true
            } else {
                sub_structure += 1;
                false
            }
        })
        .collect();
    nodes.sort_by_key(|sp| sp.start_move);
    for span in nodes {
        let Some(strategy) = strategy_of_span(&span.label) else {
            continue;
        };
        let start = span.start_move.max(1);
        let end = span.end_move.min(moves.len());
        let mut cells = std::collections::HashSet::new();
        let mut entry = None;
        let mut exit = None;
        for i in start..end {
            if !moves[i].move_type.is_cutting() {
                continue;
            }
            stamp_cells(&mut cells, moves[i - 1].target, moves[i].target);
            if entry.is_none() {
                entry = Some(moves[i - 1].target);
            }
            exit = Some(moves[i].target);
        }
        let (Some(entry), Some(exit)) = (entry, exit) else {
            continue; // a span with no cutting move has no place in a tour
        };
        out.push(RestRegion {
            strategy,
            label: span.label.to_string(),
            moves: span.end_move.saturating_sub(span.start_move),
            cutting_mm: segment_cutting_length_mm(moves, span.start_move, span.end_move),
            entry,
            exit,
            cells,
        });
    }
    (out, sub_structure)
}

/// Count and time an op's RAPID runs (contiguous `MoveType::Rapid`
/// sequences = one retract round trip each), split by whether the run
/// starts inside a routing node or between two of them.
///
/// §8's lesson was that this air is COUNT-bound, not distance-bound — a
/// hop pays two ~`safe_z` Z legs whatever its XY length — so the counts
/// here are the actionable number, not the millimetres.
fn rapid_round_trips(
    s: &ProjectSession,
    op_index: usize,
    nodes: &[(usize, usize)],
) -> (usize, f64, usize, f64) {
    use rs_cam_core::toolpath::MoveType;
    let Some(result) = s.get_result(op_index) else {
        return (0, 0.0, 0, 0.0);
    };
    let ann = result.annotated();
    let moves = &ann.toolpath.moves;
    let inside = |mi: usize| nodes.iter().any(|&(a, b)| mi >= a && mi < b);

    let (mut in_n, mut in_mm, mut out_n, mut out_mm) = (0usize, 0.0f64, 0usize, 0.0f64);
    let mut run_start: Option<usize> = None;
    for i in 0..moves.len() {
        let is_rapid = matches!(moves[i].move_type, MoveType::Rapid);
        if is_rapid && run_start.is_none() {
            run_start = Some(i);
        }
        let ends = is_rapid
            && moves
                .get(i + 1)
                .is_none_or(|n| !matches!(n.move_type, MoveType::Rapid));
        if !ends {
            continue;
        }
        let Some(start) = run_start.take() else {
            continue;
        };
        let mut mm = 0.0;
        for j in start.max(1)..=i {
            let (a, b) = (moves[j - 1].target, moves[j].target);
            mm += ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        }
        if inside(start) {
            in_n += 1;
            in_mm += mm;
        } else {
            out_n += 1;
            out_mm += mm;
        }
    }
    (in_n, in_mm, out_n, out_mm)
}

/// Greedy nearest-neighbour tour over `candidates` (indices into
/// `regions`), starting from `from` and hopping exit -> entry. Returns
/// the visiting order and the final exit point. Weak as a heuristic, but
/// applied IDENTICALLY to both the mixed and grouped tours, so it cannot
/// favour either.
fn greedy_tour(
    regions: &[RestRegion],
    candidates: &[usize],
    from: rs_cam_core::geo::P3,
) -> (Vec<usize>, rs_cam_core::geo::P3) {
    let mut remaining: Vec<usize> = candidates.to_vec();
    let mut order = Vec::with_capacity(remaining.len());
    let mut here = from;
    while !remaining.is_empty() {
        let mut best = 0usize;
        let mut best_d = f64::INFINITY;
        for (slot, &ri) in remaining.iter().enumerate() {
            let e = regions[ri].entry;
            let d = (e.x - here.x).powi(2) + (e.y - here.y).powi(2);
            if d < best_d {
                best_d = d;
                best = slot;
            }
        }
        let ri = remaining.remove(best);
        here = regions[ri].exit;
        order.push(ri);
    }
    (order, here)
}

/// Kinematics-integrated cost of a whole visiting order's hops, using the
/// same retract-rapid-replunge model the router costs its own junctions
/// with.
#[allow(clippy::too_many_arguments)]
fn tour_hop_seconds(
    regions: &[RestRegion],
    order: &[usize],
    safe_z: f64,
    plunge_rate: f64,
    kin: &rs_cam_core::machine_kinematics::MachineKinematics,
    max_feed: f64,
    rapid_feed: f64,
) -> (f64, f64) {
    let mut secs = 0.0;
    let mut mm = 0.0;
    for pair in order.windows(2) {
        let from = regions[pair[0]].exit;
        let to = regions[pair[1]].entry;
        secs += rs_cam_core::machine_kinematics::retract_link_time(
            from,
            to,
            safe_z,
            None,
            plunge_rate,
            kin,
            max_feed,
            rapid_feed,
        );
        mm += (to.x - from.x).hypot(to.y - from.y);
    }
    (secs, mm)
}

/// Does the Region-span table actually ACCOUNT for the operation?
///
/// The design doc (§2.4) makes "Region spans proving the mix" a MUST, and
/// every strategy-mix claim this campaign has made reads that table. But
/// nothing has ever checked that the routing nodes COVER the toolpath. If
/// they do not, the mix table describes a fraction of the op and every
/// conclusion drawn from it is scoped to that fraction.
///
/// Prints, for both span families: how many moves and how much cutting
/// length fall inside them, against the operation's own totals.
fn span_coverage_audit(label: &str, s: &ProjectSession, op_index: usize) {
    use rs_cam_core::toolpath::MoveIntent;
    use rs_cam_core::toolpath_spans::SpanKind;

    let result = s
        .get_result(op_index)
        .unwrap_or_else(|| panic!("[{label}] op {op_index} not generated"));
    let ann = result.annotated();
    let moves = &ann.toolpath.moves;
    let seg = |i: usize| -> f64 {
        let (a, b) = (moves[i - 1].target, moves[i].target);
        ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
    };

    let mut total_cut_mm = 0.0;
    let mut total_finish_mm = 0.0;
    for (i, mv) in moves.iter().enumerate().skip(1) {
        if mv.move_type.is_cutting() {
            total_cut_mm += seg(i);
        }
        if mv.intent == MoveIntent::FinishingCut {
            total_finish_mm += seg(i);
        }
    }

    let mut covered_node = vec![false; moves.len()];
    let mut covered_sub = vec![false; moves.len()];
    for sp in ann.spans.iter().filter(|sp| sp.kind == SpanKind::Region) {
        let target = if strategy_of_span(&sp.label).is_some() {
            &mut covered_node
        } else {
            &mut covered_sub
        };
        for flag in target
            .iter_mut()
            .take(sp.end_move.min(moves.len()))
            .skip(sp.start_move)
        {
            *flag = true;
        }
    }

    let tally = |cov: &[bool]| -> (usize, f64) {
        let mut n = 0usize;
        let mut mm = 0.0;
        for i in 1..moves.len() {
            if cov[i] && moves[i].move_type.is_cutting() {
                n += 1;
                mm += seg(i);
            }
        }
        (n, mm)
    };
    let (node_n, node_mm) = tally(&covered_node);
    let (sub_n, sub_mm) = tally(&covered_sub);
    let cut_moves = (1..moves.len())
        .filter(|&i| moves[i].move_type.is_cutting())
        .count();

    eprintln!("== [{label}] SPAN COVERAGE AUDIT ==");
    eprintln!(
        "moves {} | cutting moves {cut_moves} | cutting {total_cut_mm:.0} mm \
         (of which FinishingCut-tagged {total_finish_mm:.0} mm)",
        moves.len()
    );
    // `spans_valid == false` means a dressup rewrote the move list and
    // passed the OLD spans through unchanged rather than dropping them
    // (`dressup.rs`: `let new_spans = if spans_valid { remap } else
    // { spans }`). Stale ranges then read as nonsense against the new
    // toolpath — which is exactly what a node "covering" 20 135 moves and
    // 54 mm of cutting looks like.
    let node_extent = ann
        .spans
        .iter()
        .filter(|sp| sp.kind == SpanKind::Region && strategy_of_span(&sp.label).is_some())
        .fold((usize::MAX, 0usize), |(lo, hi), sp| {
            (lo.min(sp.start_move), hi.max(sp.end_move))
        });
    eprintln!(
        "spans_valid = {} | routing-node spans span moves [{}, {}) of {}",
        ann.spans_valid,
        node_extent.0,
        node_extent.1,
        moves.len()
    );
    eprintln!(
        "routing NODE spans cover      {node_n:>8} cutting moves ({:.1}%), {node_mm:>10.0} mm ({:.1}%)",
        100.0 * node_n as f64 / cut_moves.max(1) as f64,
        100.0 * node_mm / total_cut_mm.max(1e-9)
    );
    eprintln!(
        "emitter SUB-STRUCTURE spans   {sub_n:>8} cutting moves ({:.1}%), {sub_mm:>10.0} mm ({:.1}%)",
        100.0 * sub_n as f64 / cut_moves.max(1) as f64,
        100.0 * sub_mm / total_cut_mm.max(1e-9)
    );
}

/// STEP 1 of the reframed campaign — characterise the rest before
/// building anything. Prints the region/strategy mix, how sparse the rest
/// is against the pass that produced it, whether the strategies are
/// spatially interleaved, and the ceiling on what merging can be worth.
///
/// No measurement sim: this asks a question about the TOOLPATH, not the
/// surface, so it costs one cascade chain.
#[test]
#[ignore = "one cascade chain, no measurement sim; run with --ignored --nocapture"]
fn v3_rest_anatomy() {
    init_probe_tracing();
    let (which, dials) = dials_from_env("shipped");
    eprintln!("== REST ANATOMY at dials: {which} => {dials:?} ==");

    let path = write_fixture_project(3.0);
    let mut s = ProjectSession::load(&path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", path.display()));
    apply_branch(&mut s, Branch::Cascade);
    let op_b_idx = toolpath_index_by_name(&s, "Op B Unified Rest");
    s.set_toolpath_operation(
        op_b_idx,
        OperationConfig::UnifiedFinish(op_b_config(
            dials.intra_region_hookup_mm,
            dials.pencil_claims,
            dials.crease_hookup_mm,
        )),
    )
    .expect("swap Op B config");
    for i in 0..s.toolpath_count() {
        let Some(tc) = s.get_toolpath_config(i) else {
            continue;
        };
        let mut d = tc.dressups.clone();
        d.air_bridge_policy = dials.air_bridge_policy;
        if let Some(af) = dials.arc_fitting {
            d.arc_fitting = af;
        }
        if let Some(ro) = dials.optimize_rapid_order {
            d.optimize_rapid_order = ro;
        }
        s.set_dressup_config(i, d).expect("set dressups");
    }

    let outcome = run_chain("rest anatomy", &mut s);
    let op_a_idx = toolpath_index_by_name(&s, "Op A Ball Finish");
    let op_b_s = outcome
        .per_op_s
        .iter()
        .find(|(n, _)| n == "Op B Unified Rest")
        .map(|(_, v)| *v)
        .unwrap_or(0.0);
    let op_a_s = outcome
        .per_op_s
        .iter()
        .find(|(n, _)| n == "Op A Ball Finish")
        .map(|(_, v)| *v)
        .unwrap_or(0.0);

    let (regions, sub_structure) = rest_regions(&s, op_b_idx);
    assert!(!regions.is_empty(), "Op B emitted no routing Region nodes");
    eprintln!(
        "\nOp B Region spans: {} routing NODES + {sub_structure} emitter sub-structure spans \
         (scallop `Ring N` etc., siblings not children — see `strategy_of_span`)",
        regions.len()
    );
    span_coverage_audit("Op B", &s, op_b_idx);

    // ── 1. strategy rollup ─────────────────────────────────────────────
    let mut by_strategy: std::collections::BTreeMap<&'static str, (usize, f64, usize, usize)> =
        std::collections::BTreeMap::new();
    let mut all_cells: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for r in &regions {
        let e = by_strategy.entry(r.strategy).or_insert((0, 0.0, 0, 0));
        e.0 += 1;
        e.1 += r.cutting_mm;
        e.2 += r.moves;
        e.3 += r.cells.len();
        all_cells.extend(r.cells.iter().copied());
    }
    let total_cut_mm: f64 = regions.iter().map(|r| r.cutting_mm).sum();

    eprintln!("\n== OP B STRATEGY MIX ==");
    eprintln!(
        "{:<12} {:>8} {:>14} {:>8} {:>12} {:>9}",
        "strategy", "regions", "cutting_mm", "%cut", "footprint_mm2", "%area"
    );
    let area_total = all_cells.len() as f64;
    for (strat, (n, mm, _moves, cells)) in &by_strategy {
        eprintln!(
            "{:<12} {:>8} {:>14.0} {:>7.1}% {:>12} {:>8.1}%",
            strat,
            n,
            mm,
            100.0 * mm / total_cut_mm.max(1e-9),
            cells,
            100.0 * *cells as f64 / area_total.max(1.0),
        );
    }
    eprintln!(
        "{:<12} {:>8} {:>14.0} {:>7.1}% {:>12} {:>8.1}%   (footprint sums > distinct: overlap)",
        "TOTAL",
        regions.len(),
        total_cut_mm,
        100.0,
        all_cells.len(),
        100.0
    );

    // ── 2. sparsity: how much of Op A's ground does Op B revisit? ───────
    let op_a_cells = op_footprint_cells(&s, op_a_idx);
    let shared = all_cells.intersection(&op_a_cells).count();
    eprintln!("\n== SPARSITY (1 mm footprint cells) ==");
    eprintln!(
        "Op A (ball all-over) {:>9} cells | Op B (rest) {:>9} cells = {:.1}% of Op A",
        op_a_cells.len(),
        all_cells.len(),
        100.0 * all_cells.len() as f64 / op_a_cells.len().max(1) as f64
    );
    eprintln!(
        "Op B cells also touched by Op A: {shared} ({:.1}% of Op B)",
        100.0 * shared as f64 / all_cells.len().max(1) as f64
    );

    // ── 2b. EFFICIENCY WITH BOUNDS — area finished per second ──────────
    //
    // The user's metric, and the one raw time cannot be argued out of:
    // material removed is the wrong yardstick for a finishing pass (Op B
    // removes almost nothing by design), and total seconds depends on
    // every free dial. Surface AREA COVERED per second is invariant to
    // the tool and to the part's size, and it is what a finishing pass is
    // actually buying.
    let node_ranges: Vec<(usize, usize)> = {
        let ann = s.get_result(op_b_idx).expect("Op B result");
        let ann = ann.annotated();
        ann.spans
            .iter()
            .filter(|sp| sp.kind == rs_cam_core::toolpath_spans::SpanKind::Region)
            .filter(|sp| strategy_of_span(&sp.label).is_some())
            .map(|sp| (sp.start_move, sp.end_move))
            .collect()
    };
    let (in_n, in_mm, out_n, out_mm) = rapid_round_trips(&s, op_b_idx, &node_ranges);
    let (a_in_n, a_in_mm, a_out_n, a_out_mm) = rapid_round_trips(&s, op_a_idx, &[]);

    let mut d_sess = ProjectSession::load(&path)
        .unwrap_or_else(|e: SessionError| panic!("failed to load {}: {e}", path.display()));
    apply_branch(&mut d_sess, Branch::AllOverTip);
    let d_outcome = run_chain("rest anatomy D", &mut d_sess);
    let d_idx = toolpath_index_by_name(&d_sess, "D All-Over Tip");
    let d_cells = op_footprint_cells(&d_sess, d_idx);
    let d_s = d_outcome
        .per_op_s
        .iter()
        .find(|(n, _)| n == "D All-Over Tip")
        .map(|(_, v)| *v)
        .unwrap_or(0.0);
    let (d_in_n, _, _, _) = rapid_round_trips(&d_sess, d_idx, &[(0, usize::MAX)]);

    eprintln!("\n== EFFICIENCY WITH BOUNDS (area finished per second) ==");
    eprintln!(
        "{:<26} {:>10} {:>12} {:>12} {:>12}",
        "pass", "seconds", "area_mm2", "mm2/s", "rapid_trips"
    );
    let row = |name: &str, secs: f64, area: usize, trips: usize| {
        eprintln!(
            "{name:<26} {secs:>10.0} {area:>12} {:>12.3} {trips:>12}",
            area as f64 / secs.max(1e-9)
        );
    };
    row(
        "Op A ball all-over",
        op_a_s,
        op_a_cells.len(),
        a_in_n + a_out_n,
    );
    row("Op B unified rest", op_b_s, all_cells.len(), in_n + out_n);
    row(
        "cascade (A+B)",
        op_a_s + op_b_s,
        op_a_cells.union(&all_cells).count(),
        a_in_n + a_out_n + in_n + out_n,
    );
    row("D all-over tip", d_s, d_cells.len(), d_in_n);
    eprintln!(
        "\nOp B rapid round trips: {in_n} inside a routing node ({in_mm:.0} mm), \
         {out_n} between nodes ({out_mm:.0} mm)"
    );
    eprintln!(
        "Op A rapid round trips: {} ({:.0} mm)",
        a_in_n + a_out_n,
        a_in_mm + a_out_mm
    );

    // ── 3. interleaving: would N sweeps re-traverse the same ground? ────
    const TILE: i32 = 10;
    let mut tiles: std::collections::HashMap<(i32, i32), std::collections::BTreeSet<&'static str>> =
        std::collections::HashMap::new();
    for r in &regions {
        for c in &r.cells {
            tiles
                .entry((c.0.div_euclid(TILE), c.1.div_euclid(TILE)))
                .or_default()
                .insert(r.strategy);
        }
    }
    let mut by_count = [0usize; 5];
    for set in tiles.values() {
        by_count[set.len().min(4)] += 1;
    }
    let multi: usize = by_count[2..].iter().sum();
    eprintln!("\n== INTERLEAVING ({TILE} mm tiles) ==");
    eprintln!(
        "tiles touched by Op B: {} | 1 strategy {} | 2 {} | 3 {} | 4 {}",
        tiles.len(),
        by_count[1],
        by_count[2],
        by_count[3],
        by_count[4]
    );
    eprintln!(
        "tiles hosting >1 strategy: {multi} ({:.1}%)  <- the ground an unfused stack re-traverses",
        100.0 * multi as f64 / tiles.len().max(1) as f64
    );

    // ── 4. the prize ceiling: mixed tour vs strategy-grouped tour ───────
    let machine = s.machine();
    let kin = machine.effective_kinematics();
    let max_feed = machine.cutting_feed_ceiling_mm_min().max(1.0);
    let rapid_feed = machine.max_feed_mm_min.max(1.0);
    let plunge_rate = 150.0; // `op_b_config`
    let safe_z = s
        .get_result(op_b_idx)
        .expect("Op B result")
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, rs_cam_core::toolpath::MoveType::Rapid))
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max);

    let emitted: Vec<usize> = (0..regions.len()).collect();
    let start = regions[0].entry;
    let (mixed_order, _) = greedy_tour(&regions, &emitted, start);
    let mut grouped_order: Vec<usize> = Vec::with_capacity(regions.len());
    let mut here = start;
    for strat in UNFUSED_ORDER {
        let group: Vec<usize> = (0..regions.len())
            .filter(|&i| regions[i].strategy == strat)
            .collect();
        if group.is_empty() {
            continue;
        }
        let (ord, end) = greedy_tour(&regions, &group, here);
        here = end;
        grouped_order.extend(ord);
    }

    let cost = |order: &[usize]| {
        tour_hop_seconds(
            &regions,
            order,
            safe_z,
            plunge_rate,
            &kin,
            max_feed,
            rapid_feed,
        )
    };
    let (emit_s, emit_mm) = cost(&emitted);
    let (mixed_s, mixed_mm) = cost(&mixed_order);
    let (grouped_s, grouped_mm) = cost(&grouped_order);

    eprintln!("\n== THE PRIZE: what merging can be worth ==");
    eprintln!(
        "safe_z {safe_z:.1} mm, rapid {rapid_feed:.0} mm/min, {} hops",
        regions.len().saturating_sub(1)
    );
    eprintln!(
        "{:<34} {:>12} {:>12}",
        "inter-region tour", "hop_seconds", "hop_mm"
    );
    eprintln!(
        "{:<34} {:>12.1} {:>12.0}",
        "as emitted (router's order)", emit_s, emit_mm
    );
    eprintln!(
        "{:<34} {:>12.1} {:>12.0}",
        "greedy NN, mixed  = MERGED", mixed_s, mixed_mm
    );
    eprintln!(
        "{:<34} {:>12.1} {:>12.0}",
        "greedy NN, grouped = UNFUSED", grouped_s, grouped_mm
    );
    let prize_s = grouped_s - mixed_s;
    eprintln!(
        "\nPRIZE (unfused - merged) = {prize_s:+.1}s = {:+.2}% of Op B ({op_b_s:.0}s), \
         {:+.2}% of the finish stack ({:.0}s)",
        100.0 * prize_s / op_b_s.max(1e-9),
        100.0 * prize_s / (op_a_s + op_b_s).max(1e-9),
        op_a_s + op_b_s
    );
    eprintln!(
        "Both arms share the decomposition, strategies, params and tool, so this hop \
         delta IS the merge's whole mechanism (plus ~{} op-level approaches the \
         unfused stack pays and the merged op does not).",
        by_strategy.len().saturating_sub(1)
    );

    // ── 5. the region table (largest first, tail summarised) ───────────
    let mut idx: Vec<usize> = (0..regions.len()).collect();
    idx.sort_by(|&a, &b| regions[b].cutting_mm.total_cmp(&regions[a].cutting_mm));
    eprintln!("\n== REGION TABLE (top 25 by cutting length) ==");
    eprintln!(
        "{:<6} {:<22} {:<11} {:>8} {:>12} {:>10}",
        "rank", "span label", "strategy", "moves", "cutting_mm", "cells"
    );
    for (rank, &i) in idx.iter().take(25).enumerate() {
        let r = &regions[i];
        eprintln!(
            "{:<6} {:<22} {:<11} {:>8} {:>12.1} {:>10}",
            rank + 1,
            r.label,
            r.strategy,
            r.moves,
            r.cutting_mm,
            r.cells.len()
        );
    }
    if idx.len() > 25 {
        let tail_mm: f64 = idx[25..].iter().map(|&i| regions[i].cutting_mm).sum();
        eprintln!(
            "... {} more regions, {tail_mm:.0} mm ({:.1}% of cutting)",
            idx.len() - 25,
            100.0 * tail_mm / total_cut_mm.max(1e-9)
        );
    }
}
