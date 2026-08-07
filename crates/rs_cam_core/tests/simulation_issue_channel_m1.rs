//! **M1 / R4 research instrument** — characterise the simulation issue
//! channel, its measurability floors, and the Rivers B4 axial-DOC spike.
//!
//! Research-only. Nothing here gates production behaviour; the assertions
//! are non-vacuity bars on the *fixture* (so a future refactor that makes
//! the fixture stop exercising the mechanism fails loudly) plus the two
//! measurability floors, which are contract statements about what the
//! dexel can and cannot judge.
//!
//! Findings are written to `planning/review_2026-08-04/SIMULATION_ISSUE_CHANNEL_CENSUS.md`.
//!
//! # What each test establishes
//!
//! 1. `issue_channel_census_synthetic_2d` — the population decomposition.
//!    A synthetic 2D project with one real cut, one re-cut over already
//!    cleared ground (air by construction) and one contour pass. Prints
//!    per-toolpath: samples, cutting samples, coalesced issue segments by
//!    kind, the *per-sample* `air_cut_issue_count` counter, and the
//!    segment-length distribution. The two counters are different
//!    populations under near-identical names; this prints both side by
//!    side on one trace.
//!
//! 2. `engagement_is_unmeasurable_below_the_fresh_material_floor` — the
//!    hard floor. `dexel_stock::stamping` only counts a cell toward the
//!    radial-engagement measurement when it held more than
//!    `FRESH_MATERIAL_THRESHOLD_MM = 0.05` mm of material above the cutter
//!    (`stamping.rs:557`) AND the cell is essentially fully covered
//!    (`PERP_COVERAGE_GATE = 0.95`, `stamping.rs:572`). A pass whose
//!    commanded axial engagement is below that floor therefore reads
//!    radial engagement 0.0 at every sample, is classified `AirCut`
//!    (`simulation_cut.rs:833`), and reports 100% air cut — while removing
//!    material perfectly well. The number is not wrong-ish; it is
//!    unmeasurable, and today it prints as a precise-looking percent.
//!
//! 3. `rivers_b4_probe_project_curve_on_remaining_stock` (`#[ignore]`,
//!    expensive) — the B4 probe. Runs the committed
//!    `tests/fixtures/test_job.toml` structural analogue of Wanaka's
//!    `Rivers (back)`: a `project_curve` op (`Project Curve 6`, tool 2
//!    tapered ball, commanded `depth = 0.2`, `stock_source =
//!    from_remaining_stock`, `surface_model_id = terrain.stl`, curve model
//!    `rivers_aligned.dxf`) downstream of an `adaptive3d` rough that
//!    leaves `stock_to_leave_axial = 5.0`. Groups every cutting sample by
//!    upstream stock coverage (removed height vs commanded depth) and by
//!    transit/source semantic role (`in_transit_span`, span kinds,
//!    `MoveIntent` re-joined through `move_index`) and reports where the
//!    peak lands.
//!
//!    Wanaka itself is a read-only play file and is never touched
//!    (programme rule 9); the specific 6.07 mm value belongs to that file
//!    and is deliberately not the bar here. What this probe can settle is
//!    the *mechanism class* on a committed fixture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

// Deliberately does NOT `mod common;` — R2's wave is editing
// `tests/common/` concurrently and this research instrument must not
// couple its compile to that lane. The one helper it needed is inlined.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, ProfileConfig};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::{AirCutRatios, SimulationCutIssueKind};

/// `dexel_stock::stamping::FRESH_MATERIAL_THRESHOLD_MM`, restated here so
/// the probe fails if the production constant moves without this file
/// being revisited. It is `pub(super)`-scoped, so an integration test
/// cannot import it.
const FRESH_MATERIAL_FLOOR_MM: f64 = 0.05;

/// Workspace root (`tests/common::repo_root` inlined — see the note above
/// the imports).
fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

fn sim_options(resolution: f64) -> SimulationOptions {
    SimulationOptions {
        resolution,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn toolpath_config(
    id: usize,
    name: &str,
    op: OperationConfig,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

/// Load `test_data/ux_2d_pocket.toml` and return the session plus the
/// 6 mm end mill and the demo SVG model it defines.
fn synthetic_2d_session() -> (ProjectSession, usize, usize) {
    let toml_path = repo_root().join("test_data/ux_2d_pocket.toml");
    let session = ProjectSession::load(&toml_path).expect("load ux_2d_pocket");
    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_2d_pocket.toml must define a 6 mm end mill");
    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("ux_2d_pocket.toml must load demo_pocket.svg");
    (session, tool_id, model_id)
}

// ───────────────────────────── 1. census ──────────────────────────────

#[test]
fn issue_channel_census_synthetic_2d() {
    let (mut session, tool_id, model_id) = synthetic_2d_session();

    // TP0 real cut; TP1 the identical pass again (its ground is already
    // cleared, so every sample is air by construction — the
    // noise-by-construction population, in miniature); TP2 a contour.
    let ops: Vec<(&str, OperationConfig)> = vec![
        (
            "pocket-real",
            OperationConfig::Pocket(PocketConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..PocketConfig::default()
            }),
        ),
        (
            "pocket-recut-air",
            OperationConfig::Pocket(PocketConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..PocketConfig::default()
            }),
        ),
        (
            "profile",
            OperationConfig::Profile(ProfileConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..ProfileConfig::default()
            }),
        ),
    ];
    for (i, (name, op)) in ops.iter().enumerate() {
        session
            .add_toolpath(0, toolpath_config(i, name, op.clone(), tool_id, model_id))
            .expect("add census toolpath");
    }

    let cancel = AtomicBool::new(false);
    for (i, (name, _)) in ops.iter().enumerate() {
        session
            .generate_toolpath(i, &cancel)
            .unwrap_or_else(|e| panic!("generate {name}: {e:?}"));
    }
    session
        .run_simulation(&sim_options(0.5), &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let ct = sim.cut_trace.as_ref().expect("metric cut trace");

    println!("\n=== M1 issue-channel census (ux_2d_pocket, cell 0.5 mm) ===");
    println!(
        "trace: {} samples, {} toolpaths, summary.issue_count {} (COALESCED SEGMENTS), \
         hotspot_count {}",
        ct.summary.sample_count,
        ct.summary.toolpath_count,
        ct.summary.issue_count,
        ct.summary.hotspot_count
    );
    println!(
        "project air cut: {:.1}% of total runtime / {:.1}% of cutting time",
        ct.summary.air_cut_pct_of_total_runtime(),
        ct.summary.air_cut_pct_of_cutting_time()
    );

    println!(
        "\n{:<18} {:>8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>8}",
        "toolpath", "samples", "cutting", "airSEG", "lowSEG", "airSAMPLE", "lowSAMPLE", "maxSeg"
    );
    for (i, (name, _)) in ops.iter().enumerate() {
        let id = ToolpathId(i);
        let samples = ct.samples.iter().filter(|s| s.toolpath_id == id).count();
        let cutting = ct
            .samples
            .iter()
            .filter(|s| s.toolpath_id == id && s.is_cutting)
            .count();
        let air_seg = ct
            .issues
            .iter()
            .filter(|x| x.toolpath_id == id && x.kind == SimulationCutIssueKind::AirCut)
            .count();
        let low_seg = ct
            .issues
            .iter()
            .filter(|x| x.toolpath_id == id && x.kind == SimulationCutIssueKind::LowEngagement)
            .count();
        let max_seg = ct
            .issues
            .iter()
            .filter(|x| x.toolpath_id == id)
            .map(|x| x.sample_count)
            .max()
            .unwrap_or(0);
        // The per-SAMPLE counters live on the semantic summaries; the
        // per-toolpath summary carries only the time totals, so recompute
        // the per-sample tally the same way `SummaryAccumulator` does.
        let air_sample = ct
            .samples
            .iter()
            .filter(|s| {
                s.toolpath_id == id && s.is_cutting && s.engagement.radial_woc_fraction < 0.02
            })
            .count();
        let low_sample = ct
            .samples
            .iter()
            .filter(|s| {
                s.toolpath_id == id
                    && s.is_cutting
                    && s.engagement.radial_woc_fraction >= 0.02
                    && s.engagement.radial_woc_fraction < 0.10
            })
            .count();
        println!(
            "{name:<18} {samples:>8} {cutting:>8} {air_seg:>8} {low_seg:>8} \
             {air_sample:>10} {low_sample:>10} {max_seg:>8}"
        );
    }

    // Non-vacuity: the fixture must actually contain the mechanism.
    let recut_air = ct
        .samples
        .iter()
        .filter(|s| {
            s.toolpath_id == ToolpathId(1)
                && s.is_cutting
                && s.engagement.radial_woc_fraction < 0.02
        })
        .count();
    assert!(
        recut_air > 0,
        "fixture is vacuous: the re-cut pass produced no air-cut samples"
    );
    assert!(
        !ct.issues.is_empty(),
        "fixture is vacuous: no issue segments emitted at all"
    );
}

// ────────────────── 2. measurability floor (fresh material) ──────────────────

/// A pass whose axial engagement is below `FRESH_MATERIAL_FLOOR_MM` reads
/// radial engagement 0.0 everywhere and is reported as 100% air cut, while
/// a pass above the floor over the same geometry reads real engagement.
///
/// This is the *engagement/air-cut* measurability floor. It is a fixed
/// millimetre constant in the stamping kernel, not a function of cell
/// size, and it is invisible in every surface that publishes an air-cut
/// percentage today.
#[test]
fn engagement_is_unmeasurable_below_the_fresh_material_floor() {
    let (mut session, tool_id, model_id) = synthetic_2d_session();

    // Shallow arm: one pass, 0.02 mm — well under the 0.05 mm floor.
    let shallow = OperationConfig::Pocket(PocketConfig {
        depth: 0.02,
        depth_per_pass: 0.02,
        ..PocketConfig::default()
    });
    // Deep arm: identical geometry, 2.0 mm — well over it.
    let deep = OperationConfig::Pocket(PocketConfig {
        depth: 2.0,
        depth_per_pass: 2.0,
        ..PocketConfig::default()
    });

    session
        .add_toolpath(
            0,
            toolpath_config(0, "shallow-0.02mm", shallow, tool_id, model_id),
        )
        .expect("add shallow");
    session
        .add_toolpath(0, toolpath_config(1, "deep-2mm", deep, tool_id, model_id))
        .expect("add deep");

    let cancel = AtomicBool::new(false);
    session.generate_toolpath(0, &cancel).expect("gen shallow");
    session.generate_toolpath(1, &cancel).expect("gen deep");
    session
        .run_simulation(&sim_options(0.5), &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let ct = sim.cut_trace.as_ref().expect("metric cut trace");

    let arm = |id: usize| {
        let id = ToolpathId(id);
        let s = ct
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == id)
            .expect("toolpath summary");
        let removed: f64 = ct
            .samples
            .iter()
            .filter(|x| x.toolpath_id == id && x.is_cutting)
            .map(|x| x.axial_engagement_mm)
            .fold(0.0, f64::max);
        let peak_radial = ct
            .samples
            .iter()
            .filter(|x| x.toolpath_id == id && x.is_cutting)
            .map(|x| x.engagement.radial_woc_fraction)
            .fold(0.0, f64::max);
        (
            s.air_cut_pct_of_total_runtime(),
            s.average_engagement,
            peak_radial,
            removed,
            s.total_removed_volume_est_mm3,
        )
    };

    let (sh_air, sh_avg, sh_peak, sh_removed, sh_vol) = arm(0);
    let (dp_air, dp_avg, dp_peak, dp_removed, dp_vol) = arm(1);

    println!(
        "\n=== M1 measurability floor (FRESH_MATERIAL_THRESHOLD_MM = {FRESH_MATERIAL_FLOOR_MM}) ==="
    );
    println!(
        "shallow 0.02 mm: air {sh_air:.1}% of total runtime, avg engagement {sh_avg:.4}, \
         peak radial {sh_peak:.4}, peak removed height {sh_removed:.4} mm, \
         removed volume {sh_vol:.1} mm3"
    );
    println!(
        "deep    2.00 mm: air {dp_air:.1}% of total runtime, avg engagement {dp_avg:.4}, \
         peak radial {dp_peak:.4}, peak removed height {dp_removed:.4} mm, \
         removed volume {dp_vol:.1} mm3"
    );

    // The shallow arm removes real material...
    assert!(
        sh_vol > 0.0,
        "shallow arm removed no material — the fixture does not exercise the floor"
    );
    // ...and yet reads zero radial engagement everywhere.
    assert!(
        sh_peak < 1e-9,
        "shallow arm read peak radial engagement {sh_peak}; expected an identical zero \
         because every cell held < {FRESH_MATERIAL_FLOOR_MM} mm above the cutter"
    );
    // The deep arm over the same geometry is measured normally, so the
    // zero above is the floor and not the fixture.
    assert!(
        dp_peak > 0.1,
        "deep arm read peak radial engagement {dp_peak}; the control arm must be measurable \
         or the shallow result proves nothing"
    );
}

// ─────────────────────── 3. Rivers B4 probe ────────────────────────

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

/// B4 / A/L2 — where does a `project_curve` op's peak axial DOC come from?
///
/// Commanded `depth` is 0.2 mm. `peak_axial_doc_mm` is
/// `max(pre_ray_len − post_ray_len)` over the midpoint disc — the height of
/// material the stamp removed — so if the curve crosses ground the upstream
/// rough never took down, the reading is the standing height there, not the
/// commanded offset. That is the "upstream stock coverage" hypothesis.
///
/// The competing hypothesis is transit/source role: the reading belongs to
/// a lift bridge, lead-out or entry the transit filter failed to mask.
///
/// This test decides between them by attributing the peak.
#[test]
#[ignore = "expensive: generates + simulates the terrain fixture; run explicitly with --ignored"]
fn rivers_b4_probe_project_curve_on_remaining_stock() {
    let path = fixture_path("test_job.toml");
    let mut session = ProjectSession::load(&path).expect("test_job.toml loads");

    // Keep only the upstream rough (id 10) and the project_curve on
    // remaining stock (id 12). Everything else is cost without evidence.
    const ROUGH_ID: usize = 10;
    const PC_ID: usize = 12;
    for tc in session.toolpath_configs_mut() {
        tc.enabled = tc.id == ToolpathId(ROUGH_ID) || tc.id == ToolpathId(PC_ID);
    }

    let cancel = AtomicBool::new(false);
    let _ = session.generate_all(&[], &cancel);
    session
        .run_simulation(&sim_options(0.5), &cancel)
        .expect("simulation completes");
    // FromRemainingStock ops need the simulated upstream stock, so
    // regenerate after the first sim and re-simulate.
    let _ = session.generate_all(&[], &cancel);
    session
        .run_simulation(&sim_options(0.5), &cancel)
        .expect("second simulation completes");

    let commanded_depth_mm = session
        .toolpath_configs()
        .iter()
        .find(|t| t.id == ToolpathId(PC_ID))
        .and_then(|t| match &t.operation {
            OperationConfig::ProjectCurve(p) => Some(p.depth.abs()),
            _ => None,
        })
        .expect("Project Curve 6 is a project_curve op");

    let pc_index = session
        .toolpath_configs()
        .iter()
        .position(|t| t.id == ToolpathId(PC_ID))
        .expect("PC index");
    let intents: Vec<String> = session
        .get_result(pc_index)
        .map(|r| {
            r.annotated()
                .toolpath
                .moves
                .iter()
                .map(|m| format!("{:?}", m.intent))
                .collect()
        })
        .unwrap_or_default();
    let span_kinds: Vec<String> = session
        .get_result(pc_index)
        .map(|r| {
            r.annotated()
                .spans
                .iter()
                .map(|s| format!("{:?}", s.kind))
                .collect()
        })
        .unwrap_or_default();

    let sim = session.simulation_result().expect("simulation result");
    let ct = sim.cut_trace.as_ref().expect("metric cut trace");
    let id = ToolpathId(PC_ID);

    let summary = ct
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == id)
        .expect("project_curve summary present");

    println!("\n=== M1 / B4 Rivers probe — committed test_job.toml analogue ===");
    println!(
        "Project Curve 6: commanded depth {commanded_depth_mm:.3} mm, \
         summary.peak_axial_doc_mm {:.4} mm ({:.1}x commanded), \
         peak_plunge_descent_mm {:.4} mm",
        summary.peak_axial_doc_mm,
        summary.peak_axial_doc_mm / commanded_depth_mm.max(1e-9),
        summary.peak_plunge_descent_mm
    );
    println!(
        "air cut {:.1}% of total runtime / {:.1}% of cutting time; \
         {} spans, kinds: {:?}",
        summary.air_cut_pct_of_total_runtime(),
        summary.air_cut_pct_of_cutting_time(),
        span_kinds.len(),
        {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for k in &span_kinds {
                *counts.entry(k.as_str()).or_default() += 1;
            }
            counts
        }
    );

    // ── Group A: upstream stock coverage, expressed as removed height. ──
    // Bucket every cutting sample by removed height / commanded depth.
    let mut buckets: BTreeMap<&str, usize> = BTreeMap::new();
    let bucket_of = |ratio: f64| -> &'static str {
        if ratio <= 1.5 {
            "<=1.5x commanded (surface-following)"
        } else if ratio <= 3.0 {
            "1.5-3x"
        } else if ratio <= 10.0 {
            "3-10x"
        } else {
            ">10x (standing material)"
        }
    };

    let mut peak = 0.0_f64;
    let mut peak_sample = None;
    for s in ct.samples.iter().filter(|s| s.toolpath_id == id) {
        if !s.is_cutting {
            continue;
        }
        let ratio = s.axial_engagement_mm / commanded_depth_mm.max(1e-9);
        *buckets.entry(bucket_of(ratio)).or_default() += 1;
        if s.axial_engagement_mm > peak {
            peak = s.axial_engagement_mm;
            peak_sample = Some(s.clone());
        }
    }
    println!("\nremoved-height buckets (cutting samples): {buckets:?}");

    // ── Group B: transit / source semantic role of the peak. ──
    match peak_sample {
        None => println!("NO CUTTING SAMPLES — probe is vacuous on this fixture"),
        Some(s) => {
            let intent = intents.get(s.move_index).cloned().unwrap_or_default();
            let kinds: Vec<&String> = s
                .span_path
                .iter()
                .filter_map(|sid| span_kinds.get(sid.0 as usize))
                .collect();
            println!(
                "\npeak cutting sample: axial_engagement {:.4} mm ({:.1}x commanded) at \
                 (x={:.2}, y={:.2}, z={:.3}); move {} intent {intent}; \
                 in_transit_span {}; kinematics {:?}; span kinds {kinds:?}; \
                 radial_woc {:.4}",
                s.axial_engagement_mm,
                s.axial_engagement_mm / commanded_depth_mm.max(1e-9),
                s.position[0],
                s.position[1],
                s.position[2],
                s.move_index,
                s.in_transit_span,
                s.cut_kinematics,
                s.engagement.radial_woc_fraction,
            );

            // Corroborate with the upstream stock snapshot the op consumed.
            // Frame self-check: report the reading and the delta; a wrong
            // frame shows up as a nonsensical or absent value, which the
            // census records rather than silently trusting.
            if let Some(prior) = sim.prior_stocks.get(&id) {
                let r = 1.0;
                let top = prior.max_top_z_in_disc(s.position[0], s.position[1], r);
                println!(
                    "prior-stock max top within {r} mm of the peak XY: {top:?}; \
                     cutter z {:.3}; standing above cutter {:?}",
                    s.position[2],
                    top.map(|t| t - s.position[2])
                );
            } else {
                println!("no prior_stocks entry for the project_curve op");
            }
        }
    }

    // Non-vacuity: the fixture must produce cutting samples for the op.
    let cutting = ct
        .samples
        .iter()
        .filter(|s| s.toolpath_id == id && s.is_cutting)
        .count();
    assert!(
        cutting > 0,
        "B4 probe is vacuous: Project Curve 6 produced no cutting samples"
    );
}
