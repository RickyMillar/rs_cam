//! PR-6b sentry (H2.4) — the crease-own-region threshold, made LIVE and
//! given a tool scale.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Guards**: the H2.4 slice. `finish_planner::decompose` used to take a
//!   bare positional `tool_radius: f64` used for exactly one decision — the
//!   canyon rule `half_width_mm >= corridor_k * tool_radius` — and the only
//!   production caller (`unified_finish`) passed `cutter.radius()`, the
//!   ENVELOPE, while building the very same `FinishPlannerParams` from
//!   `cusp_radius()`. One struct, two tool scales.
//! * **Dormancy**: that call passes an EMPTY crease slice, so the rule is
//!   unreachable from production today (verified twice in the 2026-07-29
//!   review). Gate 1 is the make-it-live sentry the plan demands: it drives
//!   `decompose` with a NON-EMPTY crease slice where the threshold, and
//!   nothing else, decides ownership.
//! * **Variables held fixed**: one flat 60 × 60 grid at 1 mm cells, one
//!   straight centreline, one set of planner dials. The ONLY variable
//!   between the two arms of gate 1 is the crease's `half_width_mm`; the
//!   only variable in gate 2 is which tool scale sized the threshold.
//! * **Metric domains**: `half_width_mm` and
//!   `crease_own_region_half_width_mm` are both LATERAL half-widths in mm on
//!   the classification grid's XY plane. Gate 3's fingerprint is a move
//!   count plus an FNV-1a hash of the emitted move list.
//!
//! ## Why this cannot pass vacuously
//!
//! Gate 1 asserts the crease produced a corridor at all (i.e. the slice was
//! live and rasterizable) before asserting anything about `own_region`, and
//! it asserts BOTH sides of the threshold on the same fixture, so a rule
//! stuck at always-`Some` or always-`None` fails.
//!
//! ## Why gate 3 is a real proof, not a tautology
//!
//! Production still passes `&[]`, so H2.4 cannot move the emitted toolpath —
//! and the pinned constants were CAPTURED on the pre-H2.4 tree (commit
//! `b8e3a0d`) with a throwaway probe, then re-asserted after. They are
//! evidence, not a snapshot of whatever the code does now.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

// ── Tool geometry (the wanaka-class finisher, as in M2.1) ───────────────

const TIP_DIAMETER_MM: f64 = 1.0;
const SHANK_DIAMETER_MM: f64 = 6.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;
const BALL_DIAMETER_MM: f64 = 3.0;

fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(
        TIP_DIAMETER_MM,
        TAPER_HALF_ANGLE_DEG,
        SHANK_DIAMETER_MM,
        25.0,
    )
}

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: BALL_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

// ── Mesh fixture ────────────────────────────────────────────────────────

fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A 30 × 30 plateau with two grooves the Ø1 tip can work and the Ø6 shank
/// cannot: a 4 mm-wide 45° V (a valley WIDE enough that a fan actually
/// fits — the envelope rule's `half_width − 3.0` is negative here, which is
/// exactly the dead-code symptom Checkpoint A recorded) and a narrow steep
/// one.
fn two_groove_plateau() -> TriangleMesh {
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-8.0, 0.0),
        (-6.0, -2.0),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    extrude_profile(&profile, -15.0, 15.0)
}

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "two_groove_plateau".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://two_groove_plateau.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        // The whole point: the claims pipeline is the site that derives the
        // stepover. Default-off, so the fixture must turn it on.
        pencil_claims: true,
        ..UnifiedFinishConfig::default()
    })
}

fn toolpath(op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Unified Finish".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-9.0),
            ..HeightsConfig::default()
        },
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

fn stock() -> StockConfig {
    StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The REAL production entry point, so the derivation is exercised through
/// the wiring the GUI and the CLI share.
fn generate_through_session(tool: ToolConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    session
        .add_toolpath(0, toolpath(unified_finish_op(), tool_id, model_id))
        .expect("add unified-finish toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("UnifiedFinish must generate on the two-groove plateau");
    session
}

// ── Gate 1: the make-it-live sentry ─────────────────────────────────────

/// A straight centreline along `y = 30.0`, `x` in `[10, 50)`, on a flat
/// 60 × 60 mm grid — the same shape `finish_planner`'s own crease unit tests
/// use, so the fixture is not novel geometry, only a live call.
fn crease_fixture(
    half_width_mm: f64,
) -> (
    rs_cam_core::slope::SlopeMap,
    Vec<bool>,
    Vec<rs_cam_core::rest_field::RestCenterline>,
) {
    let (rows, cols, cell) = (60usize, 60usize, 1.0f64);
    let z = vec![0.0; rows * cols];
    let slope_map = rs_cam_core::slope::SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
    let covered = vec![true; rows * cols];
    let points: Vec<P3> = (10..50).map(|x| P3::new(x as f64, 30.0, 0.0)).collect();
    let creases = vec![rs_cam_core::rest_field::RestCenterline::without_samples(
        points,
        half_width_mm,
    )];
    (slope_map, covered, creases)
}

/// The crease slice IS live, and the threshold — nothing else — decides
/// whether a crease becomes its own planned region.
#[test]
fn the_crease_threshold_decides_own_region_ownership() {
    use rs_cam_core::finish_planner::{FinishPlannerParams, decompose};

    // Shipped dials for the Ø1-tip taper: `for_tool(cusp_radius)`.
    let t = taper();
    let params = FinishPlannerParams::for_tool(t.cusp_radius());
    let threshold = params.crease_own_region_half_width_mm;
    println!(
        "PR-6b taper threshold: {threshold:.4} mm (cusp {:.4})",
        t.cusp_radius()
    );
    assert!(
        threshold > 0.0,
        "a zero threshold makes every crease a canyon and decides nothing"
    );

    let run = |half_width: f64| {
        let (slope_map, covered, creases) = crease_fixture(half_width);
        decompose(&slope_map, &covered, &creases, &params)
    };

    // Straddle the bar by a hair, so ONLY the threshold can explain the
    // difference.
    let below = run(threshold * 0.99);
    let above = run(threshold * 1.01);

    // Non-vacuity: both arms must have produced a live, rasterized crease.
    for (label, planned) in [("below", &below), ("above", &above)] {
        assert_eq!(
            planned.creases.len(),
            1,
            "{label}: the crease slice must be live"
        );
        assert!(
            planned.creases[0].corridor.is_some(),
            "{label}: every crease claims a corridor — if this is None the \
             fixture degenerated and the ownership assertion means nothing"
        );
        assert_eq!(planned.stats.claimed_creases, 1, "{label}");
    }

    assert!(
        below.creases[0].own_region.is_none(),
        "a crease below the threshold stays folded into the pencil pass"
    );
    assert!(
        above.creases[0].own_region.is_some(),
        "a crease at/above the threshold is planned as its own region"
    );
    // The promoted crease is counted as a region; the folded one is not.
    assert_eq!(above.stats.region_count, above.regions.len() + 1);
    assert_eq!(below.stats.region_count, below.regions.len());
}

// ── Gate 2: which tool scale ────────────────────────────────────────────

/// The threshold is CUSP-scaled, and on the shipped taper that is a 6×
/// difference from the envelope-scaled number the retired positional
/// argument carried.
///
/// This is a planner-TERRITORY decision, not a fit decision: the fit
/// question was already answered upstream by `reach`'s coverage criterion
/// (a crease only reaches `decompose` when the detector routed it to
/// Pencil). See `FinishPlannerParams::crease_own_region_half_width_mm`.
#[test]
fn the_threshold_is_cusp_scale_not_envelope_scale() {
    use rs_cam_core::finish_planner::{CREASE_OWN_REGION_K, FinishPlannerParams, decompose};

    let t = taper();
    let params = FinishPlannerParams::for_tool(t.cusp_radius());
    let cusp_scaled = params.crease_own_region_half_width_mm;
    // What the retired pairing produced: `corridor_k` (2.0, now
    // `CREASE_OWN_REGION_K`) times the ENVELOPE radius `unified_finish`
    // passed positionally.
    let envelope_scaled = CREASE_OWN_REGION_K * t.envelope_radius_mm();

    println!(
        "PR-6b scales: cusp-scaled {cusp_scaled:.3} mm vs envelope-scaled {envelope_scaled:.3} mm"
    );
    assert!((cusp_scaled - 1.0).abs() < 1e-12, "2 × the Ø1 tip radius");
    assert!(
        (envelope_scaled - 6.0).abs() < 1e-12,
        "2 × the Ø6 shank radius"
    );

    // A 1.5 mm-half-width valley: real work for a Ø1 tip, six times below
    // the envelope bar. It must be a canyon now, and provably was not.
    let (slope_map, covered, creases) = crease_fixture(1.5);
    let planned = decompose(&slope_map, &covered, &creases, &params);
    assert!(
        planned.creases[0].corridor.is_some(),
        "non-vacuity: the crease must rasterize"
    );
    assert!(
        planned.creases[0].own_region.is_some(),
        "a 1.5 mm half-width valley clears the cusp-scaled bar"
    );
    assert!(
        1.5 < envelope_scaled,
        "...and would NOT have cleared the envelope-scaled bar — the \
         differential this gate exists to record"
    );

    // A ball is unaffected: its cusp radius IS its envelope radius.
    let ball = BallEndmill::new(BALL_DIAMETER_MM, 25.0);
    let ball_params = FinishPlannerParams::for_tool(ball.cusp_radius());
    assert!(
        (ball_params.crease_own_region_half_width_mm
            - CREASE_OWN_REGION_K * ball.envelope_radius_mm())
        .abs()
            < 1e-12,
        "the ball control must not move"
    );
}

// ── Gate 3: production output is byte-identical ─────────────────────────

/// H2.4 changes no emitted move: `unified_finish` still hands `decompose`
/// an EMPTY crease slice, so `apply_crease_corridor` never runs in
/// production.
///
/// The constants were captured on the pre-H2.4 tree and are asserted here.
/// If a future change makes the crease slice non-empty in production, this
/// test is expected to go red — and that is the signal, not a nuisance:
/// re-pin it deliberately, with the A/B that justifies the move.
#[test]
fn production_unified_finish_output_is_byte_identical() {
    use rs_cam_core::toolpath::Toolpath;

    // FNV-1a over the `Debug` rendering (round-trips every f64 exactly).
    // Not `DefaultHasher`: its algorithm is explicitly unstable.
    fn fingerprint(tp: &Toolpath) -> (usize, u64) {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in format!("{:?}", tp.moves).bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        (tp.moves.len(), h)
    }

    // Captured at `b8e3a0d` (PR-6a, pre-H2.4) with a throwaway probe.
    //
    // TAPER RE-PINNED 2026-08-03 (C-sequence wave 11), 1855 → 1874 moves.
    // The mover is `dde7a54` — M4's iso-field gouge fix — NOT anything in
    // this file's subject. That wave knew it moved scallop geometry and
    // re-pinned the scallop-side fingerprint in `a376b1e`; what it missed is
    // that `UnifiedFinish` embeds scallop for its MidSteep band, so a SECOND
    // pin lives here. Proven by surgical revert: restoring `scallop.rs` +
    // `scallop_isofield.rs` to `dde7a54^` reproduces (1855, 0xe031…)
    // exactly, and every other commit in the window — including the claims
    // fan (`0dff17e`), which was the first suspect — leaves the value
    // untouched.
    //
    // The A/B that justifies the move (per this test's own instruction),
    // measured by decomposing both toolpaths into cutting fragments:
    //
    //   fragments        88 → 88      (no pass added, none dropped)
    //   DEEP groove      336 moves / 644.2994 mm → IDENTICAL
    //   SHALLOW groove   708 → 727 moves, 357.3665 → 358.7768 mm (+0.39%)
    //
    // All 19 added moves land in 4 of the 5 fragments inside the shallow
    // 4 mm 45° V; the 80 deep-groove fragments are byte-identical. The four
    // changed passes reach marginally deeper and wider — and, decisively,
    // none of them enters new depth territory: the groove's longest
    // fragment already reached z = −1.7891 before the change and still
    // does, while the three short fragments converge toward it
    // (−1.6513/−1.7358/−1.6931 → −1.7802/−1.7641/−1.7789) without passing
    // it. Denser, more consistent sampling of the SAME surface — not a new
    // pass, not a wander outside the groove rims, not a duplicate.
    //
    // BOTH RE-PINNED 2026-08-03 (C-sequence wave 14): taper 1874 → 1387
    // (−26.0%), ball 1301 → 1072 (−17.6%). The mover is the arc-carrying
    // offset cascade adopted at Checkpoint D — `UnifiedFinish` embeds scallop
    // for its MidSteep band, which is exactly why this second pin exists, and
    // wave 14 came looking for it rather than being told about it afterwards.
    //
    // Fewer moves, and the reason is that there are fewer VERTICES to emit,
    // not fewer places visited: the cascade no longer flattens each ring's
    // arc joins to chords before feeding it back in, so the accumulated
    // arc-join debris — which drop-only decimation then had to remove again —
    // never exists. Ring PLACEMENT is measured EXACT by the 2D erosion oracle
    // (0.0 µm, against 143 µm for the flattened cascade), and the same-build
    // M4 envelope A/B on the grooved block has the arc cascade at 46.5 µm
    // achieved cusp / 3.17 mm² unfinished against decimation's 58.5 µm /
    // 4.38 mm². The chord-sag gate that bounds what scallop feeds
    // (`scallop_isofield_gouge_m4`) is green on both ring sources and all
    // three fixtures at a 100 µm tolerance, after two chord-refinement
    // defects this change exposed were fixed.
    //
    // **Both values below were re-captured a second time, at the END of wave
    // 14** (taper 1387 -> 1595, ball 1072 -> 1109). The first capture was
    // taken before `polygon::FlattenPolicy` gained its SAMPLING bound
    // (`scallop::RingSampleBound`), so it described a build that does not
    // ship. Same erratum as `finish_resolution_policy_pr3`'s, same lesson:
    // pins are captured after a wave's last behavioural commit, not during.
    // The added moves are ring samples on straight runs, and what they buy is
    // measured on the envelope oracle in `ring_sample_bound_w14` — gouge
    // containment on surfaces with slope discontinuities (1.570 mm² -> 0.000
    // mm² on the grooved block), not achieved cusp.
    // **RE-PINNED 2026-08-06, and the reason matters: this pin was ALREADY
    // STALE on the parent revision.** The F2 census (`planning/
    // review_2026-08-04/ORCHESTRATION_LOG.md`, F23-impl) ran this test on an
    // untouched checkout and read taper `(1464, 0x655861151cebaee4)` against
    // the pinned `(1595, 0xfbd7fd118b8f2e25)` — a −131-move gap that no
    // commit had recorded. It is NOT F2's: F2's own scallop coverage-guard
    // change re-measured this fixture at the SAME (1464, 0x6558…), i.e. the
    // guard does not move this configuration at all. Some earlier
    // finish-geometry commit moved it and left the pin behind.
    //
    // BOTH arms were stale, by similar fractions: taper 1595 -> 1464
    // (−8.2%), ball 1109 -> 972 (−12.4%). Because the assertion sits inside
    // this loop the taper arm aborted first, so the ball pin had not been
    // evaluated at all since the gap opened. Both values below were measured
    // twice: once on an untouched checkout, once with F2's geometry changes
    // applied. They are IDENTICAL in both, which is what establishes that F2
    // does not move this fixture and the gap is somebody else's.
    //
    // The gap is therefore re-pinned here, unattributed and said so, rather
    // than folded into a commit that would then look like it caused it. If
    // this fixture's numbers matter to a future reader, the −131 moves are
    // an open question with a known bracket: between `e3427f8` (the last
    // commit that touched this pin) and `88ce23a`.
    // Re-pinned 2026-08-21 for G-SAFEZ-LOCAL, and this one IS attributed.
    // This fixture's stock is 9 mm thick at `origin_z = -9.0`, so its world
    // top is Z0 and the project's own `post.safe_z = 10` already clears it.
    // The old local-rooted floor overrode that with `9 + 5 = 14`. Both arms
    // were measured before and after with a distinct-Z probe: the two Z
    // multisets are IDENTICAL except the single retract plane, 14.000 ->
    // 10.000 mm, and both move counts are unchanged (1464, 972). No cutting
    // Z moved, which is what makes this a pin refresh rather than a
    // geometry change.
    /// `(move count, geometry hash)` — what `fingerprint` returns.
    type Fp = (usize, u64);
    let mut drift: Vec<(&str, Fp, Fp)> = Vec::new();
    for (label, tool, expect) in [
        (
            "taper",
            tapered_ball_tool(),
            (1464usize, 0xadd9_39bf_5beb_02b0u64),
        ),
        ("ball", ball_tool(), (972usize, 0xaf85_0700_ffcd_0179u64)),
    ] {
        let session = generate_through_session(tool);
        let result = session.get_result(0).expect("a generated result");
        let got = fingerprint(result.toolpath());
        println!("PR-6b FP {label}: moves {} hash 0x{:016x}", got.0, got.1);
        assert!(
            got.0 > 0,
            "{label}: an empty toolpath fingerprints vacuously"
        );
        drift.push((label, got, expect));
    }

    // Accumulate-then-assert. The comment above records that an in-loop
    // `assert_eq!` aborted on the taper arm and left the ball pin
    // unevaluated for however many commits the gap was open. Reporting
    // both arms is the whole point of having two.
    let stale: Vec<String> = drift
        .iter()
        .filter(|(_, got, expect)| got != expect)
        .map(|(label, got, expect)| {
            format!(
                "{label}: got ({}, 0x{:016x}), pinned ({}, 0x{:016x})",
                got.0, got.1, expect.0, expect.1
            )
        })
        .collect();
    assert!(
        stale.is_empty(),
        "H2.4 must not move a single emitted move -- {}",
        stale.join("; ")
    );
}
