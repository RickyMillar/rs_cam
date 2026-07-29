//! H3 / PR-3 sentries for the explicit finish-surface resolution policy.
//!
//! Oracle: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §H3 fix-sequence steps 1-2. The cell-size formula that used to live
//! *inside* `finish_setup::build_finish_surface_with_cancel` is now a named
//! [`FinishResolutionMode`] selected by each consumer at its own call site.
//! PR-3 changes NO cell size and NO emitted move — this file is the evidence
//! for both halves of that claim:
//!
//! 1. **Policy selection** — each of the four consumers (Scallop,
//!    RampFinish, SteepShallow, UnifiedFinish) reports the mode, the resolved
//!    `cell_mm` and the `CellSource` it is supposed to select, on a TAPERED
//!    tool where envelope and cusp differ 6×. A consumer silently moving to
//!    another scale fails here. (Today no policy *forbids* the envelope
//!    scale — every generation consumer selects it. When H3 step 4 moves one,
//!    that op's expectation below changes and the other three must not.)
//! 2. **Byte-identical output** — three toolpath fingerprints captured at
//!    HEAD `606b8d5` (i.e. *before* the refactor) on a shared ridge fixture.
//!    They are unchanged by the refactor and would change if any grid moved.
//!
//! The classification/generation split itself is pinned by the PR-2 tripwire
//! `tool_scale_semantics_pr2::finish_surface_cell_source_names_the_radius_that_sized_the_grid`,
//! which this PR deliberately leaves untouched.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::finish_setup::{
    FinishResolutionMode, FinishResolutionPolicy, build_classification_surface_with_cancel,
    build_classification_surface_with_policy_and_cancel, build_finish_surface_with_cancel,
    build_finish_surface_with_cell_size_and_cancel, build_finish_surface_with_policy_and_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::measurement::{
    CellSource, MeasurementDomain, MeasurementProvenance, MeasurementStage,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ramp_finish::{
    RampFinishParams, ramp_finish_generation_resolution, ramp_finish_toolpath,
};
use rs_cam_core::scallop::{ScallopParams, scallop_generation_resolution, scallop_toolpath};
use rs_cam_core::steep_shallow::{
    SteepShallowParams, steep_shallow_generation_resolution, steep_shallow_toolpath,
};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::unified_finish::{
    unified_finish_classification_resolution, unified_finish_mid_steep_generation_resolution,
};

/// The project's finishing tool: Ø1 tip, 7° half-angle, Ø6 shaft.
/// Envelope 3.0 mm, cusp 0.5 mm — a 6× split, so any drift between the two
/// scales is visible in the fingerprint.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// A 20×20 ridge with a gentle along-Y ripple: shallow flanks, a crest, and
/// enough curvature that stepover, ring decimation and slope classification
/// all do real work.
fn ridge_mesh() -> TriangleMesh {
    let n: usize = 21;
    let mut verts = Vec::with_capacity(n * n);
    for iy in 0..n {
        for ix in 0..n {
            let x = ix as f64;
            let y = iy as f64;
            let z = 3.0 * (1.0 - (x - 10.0).abs() / 10.0) + 0.5 * (y * 0.3).sin();
            verts.push(P3::new(x, y, z));
        }
    }
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for iy in 0..(n - 1) {
        for ix in 0..(n - 1) {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Byte-level fingerprint of an emitted toolpath: move count plus a hash of
/// the `Debug` rendering (which round-trips every f64 exactly).
fn fingerprint(tp: &Toolpath) -> (usize, u64) {
    // FNV-1a, not `DefaultHasher`: the pinned constants below must survive a
    // toolchain bump, and `DefaultHasher`'s algorithm is explicitly unstable.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!("{:?}", tp.moves).bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (tp.moves.len(), h)
}

#[test]
fn scallop_fingerprint() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let params = ScallopParams {
        scallop_height: 0.05,
        tolerance: 0.01,
        continuous: false,
        ..Default::default()
    };
    let tp = scallop_toolpath(&mesh, &index, &t, &params);
    assert_eq!(
        fingerprint(&tp),
        (1318, 4897619324930985607),
        "scallop output moved; captured at HEAD 606b8d5 before the H3 policy refactor"
    );
}

#[test]
fn ramp_finish_fingerprint() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let params = RampFinishParams {
        max_stepdown: 0.5,
        tolerance: 0.01,
        ..Default::default()
    };
    let tp = ramp_finish_toolpath(&mesh, &index, &t, &params);
    assert_eq!(
        fingerprint(&tp),
        (164, 596513364120972287),
        "ramp_finish output moved; captured at HEAD 606b8d5 before the H3 policy refactor"
    );
}

#[test]
fn steep_shallow_fingerprint() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let params = SteepShallowParams {
        tolerance: 0.01,
        ..Default::default()
    };
    let tp = steep_shallow_toolpath(&mesh, &index, &t, &params);
    assert_eq!(
        fingerprint(&tp),
        (913, 14129959905444107510),
        "steep_shallow output moved; captured at HEAD 606b8d5 before the H3 policy refactor"
    );
}

// ── 2. Per-consumer policy selection ─────────────────────────────────────

/// Assert a policy's full shape in one place: mode, resolved cell, provenance
/// tag, and whether the tolerance floor bound.
#[track_caller]
fn assert_policy(
    label: &str,
    policy: FinishResolutionPolicy,
    mode: FinishResolutionMode,
    cell_mm: f64,
    floor: bool,
) {
    assert_eq!(policy.mode(), mode, "{label}: selected the wrong mode");
    assert!(
        (policy.cell_mm() - cell_mm).abs() < 1e-12,
        "{label}: cell {} mm, expected {cell_mm} mm",
        policy.cell_mm()
    );
    // Wave D3: the provenance is the mode's own UNLESS the tolerance floor
    // set the cell, in which case the tool scale had no say and the tag says
    // so instead of naming a radius that did not decide anything.
    let expected_source = if floor {
        CellSource::ToleranceFloor
    } else {
        mode.cell_source()
    };
    assert_eq!(
        policy.cell_source(),
        expected_source,
        "{label}: provenance must be the mode's own unless the floor bound"
    );
    assert_eq!(
        policy.tolerance_floor_applied(),
        floor,
        "{label}: tolerance-floor flag"
    );
}

/// The four consumers, on the tapered tool where the two scales differ 6×.
///
/// This is the "no hidden global formula change" gate: every generation
/// consumer must still resolve to `envelope/4` and UnifiedFinish's
/// classification to `cusp/4`, but each is now asserted through the function
/// it actually calls, so one moving does not move the others.
#[test]
fn each_consumer_selects_its_own_policy() {
    let t = taper();
    let tol = 0.01;
    let envelope_quarter = t.envelope_radius_mm() / 4.0; // 0.75 mm
    let cusp_quarter = t.cusp_radius_mm() / 4.0; // 0.125 mm
    assert!(
        envelope_quarter > cusp_quarter * 3.0,
        "fixture must keep a large shaft/tip split or these assertions go inert"
    );

    assert_policy(
        "scallop",
        scallop_generation_resolution(&t, tol),
        FinishResolutionMode::LegacyEnvelopeQuarter,
        envelope_quarter,
        false,
    );
    assert_policy(
        "ramp_finish",
        ramp_finish_generation_resolution(&t, tol),
        FinishResolutionMode::LegacyEnvelopeQuarter,
        envelope_quarter,
        false,
    );
    assert_policy(
        "steep_shallow",
        steep_shallow_generation_resolution(&t, tol),
        FinishResolutionMode::LegacyEnvelopeQuarter,
        envelope_quarter,
        false,
    );
    assert_policy(
        "unified_finish classification",
        unified_finish_classification_resolution(&t, tol),
        FinishResolutionMode::CuspQuarter,
        cusp_quarter,
        false,
    );
    // UnifiedFinish's MidSteep band builds no grid of its own — it inherits
    // scallop's. Asserting IDENTITY (not just equal numbers) is what makes
    // H3 step 4 move both together instead of silently splitting them.
    assert_eq!(
        unified_finish_mid_steep_generation_resolution(&t, tol),
        scallop_generation_resolution(&t, tol),
        "UnifiedFinish's MidSteep generation must BE scallop's policy"
    );
}

/// Ball control: on a ball the cusp IS the envelope, so both modes resolve to
/// the same cell — the H3 acceptance gate "Ball fixtures remain unchanged
/// where the policy resolves to the legacy cell size".
#[test]
fn ball_resolves_both_modes_to_the_same_cell() {
    let ball = BallEndmill::new(6.0, 25.0);
    let tol = 0.01;
    let legacy = FinishResolutionPolicy::legacy_envelope_quarter(&ball, tol);
    let cusp = FinishResolutionPolicy::cusp_quarter(&ball, tol);
    assert!(
        (legacy.cell_mm() - cusp.cell_mm()).abs() < 1e-12,
        "ball: envelope/4 = {} but cusp/4 = {}",
        legacy.cell_mm(),
        cusp.cell_mm()
    );
    assert!((legacy.cell_mm() - 0.75).abs() < 1e-12);
    // The MODES still differ, and so does the provenance tag — a ball being
    // scale-degenerate must not make the two policies interchangeable.
    assert_ne!(legacy.mode(), cusp.mode());
    assert_ne!(legacy.cell_source(), cusp.cell_source());
}

/// The tolerance floor is a distinct fact from the tool scale, and the policy
/// says so.
#[test]
fn tolerance_floor_is_recorded_not_hidden() {
    let t = taper();
    // 2 mm tolerance dwarfs both envelope/4 (0.75) and cusp/4 (0.125).
    let floored = [
        FinishResolutionPolicy::legacy_envelope_quarter(&t, 2.0),
        FinishResolutionPolicy::cusp_quarter(&t, 2.0),
    ];
    for policy in floored {
        assert!((policy.cell_mm() - 2.0).abs() < 1e-12);
        assert!(
            policy.tolerance_floor_applied(),
            "{:?}: the floor set the cell, the tool scale did not",
            policy.mode()
        );
        // Wave D3: and the provenance tag says so, instead of naming a
        // radius that did not decide the number.
        assert_eq!(
            policy.cell_source(),
            CellSource::ToleranceFloor,
            "{:?}: a floor-bound cell must not claim a tool scale",
            policy.mode()
        );
    }
    // Wave D3 — the point of the variant: two DIFFERENT modes that both
    // bottom out on the same floor describe the same grid, so measurements
    // taken on them are comparable. Pre-D3 the family tags (EnvelopeRadius
    // vs CuspRadius) made `comparable_to` refuse this.
    let provenance = |policy: FinishResolutionPolicy| {
        MeasurementProvenance::new(
            MeasurementDomain::ProjectedXyArea,
            MeasurementStage::RawThreshold,
        )
        .with_cell(policy.cell_mm(), policy.cell_source())
    };
    let [env_floor, cusp_floor] = floored;
    assert_ne!(env_floor.mode(), cusp_floor.mode());
    assert!(
        provenance(env_floor).comparable_to(&provenance(cusp_floor)),
        "two floor-bound policies resolve to the SAME grid — they must compare"
    );
    // And a floor-bound cell is still NOT comparable with a tool-scaled cell
    // of the same size: same number, different reason.
    let tool_scaled = FinishResolutionPolicy::legacy_envelope_quarter(&t, 0.75);
    assert!(
        (tool_scaled.cell_mm() - env_floor.cell_mm()).abs() > 1e-12
            || !provenance(tool_scaled).comparable_to(&provenance(env_floor)),
        "a tool-scaled cell must not silently compare with a floor-bound one"
    );
    // Exactly at the boundary the tool scale still wins (`.max`, not `>`).
    let boundary = FinishResolutionPolicy::legacy_envelope_quarter(&t, 0.75);
    assert!(!boundary.tolerance_floor_applied());
    assert!((boundary.cell_mm() - 0.75).abs() < 1e-12);

    // An explicit cell has no floor to apply.
    let explicit = FinishResolutionPolicy::explicit(0.31);
    assert_eq!(explicit.mode(), FinishResolutionMode::Explicit);
    assert_eq!(explicit.cell_source(), CellSource::Explicit);
    assert!((explicit.cell_mm() - 0.31).abs() < 1e-12);
    assert!(!explicit.tolerance_floor_applied());
}

// ── 3. Builder honours the policy, and carries it ────────────────────────

/// Every surface must report the policy that sized it, and `cell_source` must
/// stay in lockstep with `resolution.cell_source()` — the invariant that lets
/// the PR-0 provenance tag be read without consulting the policy.
#[test]
fn surfaces_carry_the_policy_that_sized_them() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let cancel = || false;
    let tol = 0.01;

    for policy in [
        scallop_generation_resolution(&t, tol),
        FinishResolutionPolicy::cusp_quarter(&t, tol),
        FinishResolutionPolicy::explicit(0.4),
    ] {
        let s = build_finish_surface_with_policy_and_cancel(&mesh, &index, &t, policy, &cancel)
            .expect("generation surface");
        assert_eq!(s.resolution, policy);
        assert_eq!(s.cell_source, policy.cell_source());
        assert!((s.cell_size() - policy.cell_mm()).abs() < 1e-12);
        // Finer cell ⇒ more cells: the resolution really reaches the grid,
        // it is not merely recorded on it.
        let span_x = mesh.bbox.max.x - mesh.bbox.min.x + 2.0 * t.envelope_radius_mm();
        let expected_cols = (span_x / policy.cell_mm()).ceil() as usize + 1;
        assert_eq!(s.cols(), expected_cols, "{:?}", policy.mode());
    }

    let classification = build_classification_surface_with_policy_and_cancel(
        &mesh,
        &index,
        &t,
        unified_finish_classification_resolution(&t, tol),
        &cancel,
    )
    .expect("classification surface");
    assert_eq!(classification.cell_source, CellSource::CuspRadius);
    assert!((classification.cell_size() - t.cusp_radius_mm() / 4.0).abs() < 1e-12);
}

/// The three legacy entry points are now adapters. Each must produce exactly
/// what the policy path produces — otherwise "no behavior change" is only
/// true on the paths the sentries happen to walk.
#[test]
fn legacy_entry_points_are_policy_adapters() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let cancel = || false;
    let tol = 0.01;

    let adapter =
        build_finish_surface_with_cancel(&mesh, &index, &t, tol, &cancel).expect("legacy adapter");
    let policy = build_finish_surface_with_policy_and_cancel(
        &mesh,
        &index,
        &t,
        FinishResolutionPolicy::legacy_envelope_quarter(&t, tol),
        &cancel,
    )
    .expect("policy path");
    assert_eq!(adapter.resolution, policy.resolution);
    assert_eq!(adapter.heightmap.z_values, policy.heightmap.z_values);

    let pinned = build_finish_surface_with_cell_size_and_cancel(&mesh, &index, &t, 0.4, &cancel)
        .expect("pinned adapter");
    assert_eq!(pinned.resolution, FinishResolutionPolicy::explicit(0.4));

    let classification = build_classification_surface_with_cancel(&mesh, &index, &t, tol, &cancel)
        .expect("classification adapter");
    let classification_policy = build_classification_surface_with_policy_and_cancel(
        &mesh,
        &index,
        &t,
        FinishResolutionPolicy::cusp_quarter(&t, tol),
        &cancel,
    )
    .expect("classification policy path");
    assert_eq!(
        classification.resolution, classification_policy.resolution,
        "the classification adapter must select CuspQuarter"
    );
    assert_eq!(
        classification.heightmap.z_values,
        classification_policy.heightmap.z_values
    );
}
