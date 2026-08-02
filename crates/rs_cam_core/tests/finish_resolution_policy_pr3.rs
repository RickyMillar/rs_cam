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
//!    another scale fails here.
//! 2. **Byte-identical output** — three toolpath fingerprints captured at
//!    HEAD `606b8d5` (i.e. *before* the refactor) on a shared ridge fixture.
//!
//! **PR-8a moved exactly one of them, on purpose** (H3 wave, approved
//! Checkpoint B): `ramp_finish` now selects
//! [`FinishResolutionMode::GeoMeanEnvelopeCusp`], so its expectation in (1)
//! and its fingerprint in (2) both changed in that commit, with the
//! pre-PR-8a value recorded in place. Scallop's and steep/shallow's are
//! untouched — the per-consumer property this file exists to hold is that
//! one op can move without dragging the others, and this is the first time
//! that has been exercised rather than merely asserted.
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
    ramp_finish_toolpath_structured_annotated_with_resolution,
};
use rs_cam_core::scallop::{ScallopParams, scallop_generation_resolution, scallop_toolpath};
use rs_cam_core::steep_shallow::{
    SteepShallowParams, steep_shallow_generation_resolution, steep_shallow_toolpath,
};
use rs_cam_core::tool::MillingCutter;

mod common;

use common::fingerprint::move_fingerprint as fingerprint;
use common::tools::{ball_cutter, wanaka_taper as taper};
use rs_cam_core::unified_finish::{
    unified_finish_classification_resolution, unified_finish_mid_steep_generation_resolution,
};

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

// C6: the fingerprint (byte-level — move count plus an FNV-1a hash of the
// `Debug` rendering, which round-trips every f64 exactly) and the finishing
// taper now come from `tests/common/`. Both are imported UNDER THEIR OLD
// NAMES, so every call site and every pinned constant below is untouched:
// this file is the migration's proof, not a place to also change values.

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
    // MOVED BY PR-8a, deliberately. RampFinish now selects
    // `GeoMeanEnvelopeCusp` (0.306 mm on this taper, against 0.750), so its
    // ramp sampling step `cell_size * 2` halves and the emitted chords get
    // shorter. The pre-PR-8a value on this fixture was
    // `(164, 596513364120972287)`, captured at HEAD 606b8d5; it is recorded
    // here rather than deleted because it is the number the Checkpoint B
    // evidence was measured against.
    //
    // MOVED AGAIN BY PR-8b: the reach clamp raises every ramp point that was
    // commanded below the depth this cutter can hold at its XY. The
    // intermediate PR-8a value on this fixture was
    // `(236, 13853886592394416024)`.
    //
    // The QUALITY justification is not this fingerprint — it is
    // `checkpoint_b_resolution_ab::ramp_finish_geo_mean_policy_halves_the_
    // descent_chords` (PR-8a's mechanism) and
    // `..._reach_clamp_removes_the_cone_fixture_gouge` +
    // `tests/ramp_reach_clamp_pr8b.rs` (PR-8b's), which assert chord length
    // and residuals directly against the pinned 0.05 mm reference field.
    assert_eq!(
        fingerprint(&tp),
        RAMP_FINISH_GEO_MEAN_FINGERPRINT,
        "ramp_finish output moved AGAIN; PR-8b's value is the clamped one"
    );
}

/// PR-8a + PR-8b's ramp-finish fingerprint on `ridge_mesh()` with the taper.
/// Named so the value has one home and the two assertions that read it
/// cannot drift apart.
const RAMP_FINISH_GEO_MEAN_FINGERPRINT: (usize, u64) = (277, 18_231_352_062_362_901_444);

/// PR-8a control: the geo-mean of two EQUAL numbers is that number, so a
/// cutter whose cusp radius is its envelope radius must not move at all.
///
/// Stated as a byte-identical toolpath equality, not as a cell-size
/// equality: the cell is the mechanism, the emitted path is the claim.
#[test]
fn ball_ramp_finish_is_unmoved_by_the_geo_mean_policy() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let ball = ball_cutter(6.0);
    let tol = 0.01;
    let legacy = FinishResolutionPolicy::legacy_envelope_quarter(&ball, tol);
    let geo_mean = FinishResolutionPolicy::geo_mean_envelope_cusp(&ball, tol);
    assert!(
        (legacy.cell_mm() - geo_mean.cell_mm()).abs() < 1e-12,
        "ball: envelope/4 = {} but geo-mean = {}",
        legacy.cell_mm(),
        geo_mean.cell_mm()
    );
    // The MODE and the provenance still differ — a scale-degenerate cutter
    // must not make two policies interchangeable (same rule as
    // `ball_resolves_both_modes_to_the_same_cell`).
    assert_ne!(legacy.mode(), geo_mean.mode());
    assert_ne!(legacy.cell_source(), geo_mean.cell_source());

    let params = RampFinishParams {
        max_stepdown: 0.5,
        tolerance: tol,
        ..Default::default()
    };
    let cancel = || false;
    let shipped = ramp_finish_toolpath(&mesh, &index, &ball, &params);
    let (legacy_arm, _, _) = ramp_finish_toolpath_structured_annotated_with_resolution(
        &mesh, &index, &ball, &params, None, None, legacy, &cancel,
    )
    .expect("legacy arm");
    assert!(!shipped.moves.is_empty(), "non-vacuity");
    assert_eq!(
        fingerprint(&shipped),
        fingerprint(&legacy_arm),
        "a plain ball's geo-mean cell IS its legacy cell, so PR-8a must not \
         have moved one emitted move"
    );
    // NOTE: this is a policy-vs-policy equality, so it stays true through
    // PR-8b — the reach clamp is resolution-independent and applies equally
    // to both arms. It pins PR-8a's claim, not PR-8b's.
}

/// The taper is the discriminating case, and PR-8a's whole premise is that
/// the three cells are three different grids. If a future tool-model change
/// collapses them, every §3.2 number stops meaning anything.
#[test]
fn the_geo_mean_sits_strictly_between_its_two_factors_on_a_taper() {
    let t = taper();
    let tol = 0.01;
    let envelope = FinishResolutionPolicy::legacy_envelope_quarter(&t, tol);
    let cusp = FinishResolutionPolicy::cusp_quarter(&t, tol);
    let geo = FinishResolutionPolicy::geo_mean_envelope_cusp(&t, tol);
    assert!((envelope.cell_mm() - 0.75).abs() < 1e-12);
    assert!((cusp.cell_mm() - 0.125).abs() < 1e-12);
    assert!(
        (geo.cell_mm() - (0.75_f64 * 0.125).sqrt()).abs() < 1e-12,
        "geo-mean cell {} is not sqrt(0.75 · 0.125)",
        geo.cell_mm()
    );
    // The defining property of the GEOMETRIC mean: equal RATIO to each end,
    // which is what makes it the honest midpoint of a shaft/tip ratio.
    let up = envelope.cell_mm() / geo.cell_mm();
    let down = geo.cell_mm() / cusp.cell_mm();
    assert!(
        (up - down).abs() < 1e-12,
        "geo-mean must be the same FACTOR from each end: {up} vs {down}"
    );
    assert!(cusp.cell_mm() < geo.cell_mm() && geo.cell_mm() < envelope.cell_mm());
    assert_eq!(geo.mode(), FinishResolutionMode::GeoMeanEnvelopeCusp);
    assert_eq!(geo.cell_source(), CellSource::GeoMeanEnvelopeCuspRadius);
    // A geo-mean cell is its OWN grid: it must not compare with either
    // factor's (PR-0 `comparable_to`), or a report could put a 0.306 mm
    // measurement beside a 0.750 mm one under one heading.
    let provenance = |p: FinishResolutionPolicy| {
        MeasurementProvenance::new(
            MeasurementDomain::ProjectedXyArea,
            MeasurementStage::RawThreshold,
        )
        .with_cell(p.cell_mm(), p.cell_source())
    };
    assert!(!provenance(geo).comparable_to(&provenance(envelope)));
    assert!(!provenance(geo).comparable_to(&provenance(cusp)));
    // And the tolerance floor still overrides the family tag when it binds.
    let floored = FinishResolutionPolicy::geo_mean_envelope_cusp(&t, 2.0);
    assert!(floored.tolerance_floor_applied());
    assert_eq!(floored.cell_source(), CellSource::ToleranceFloor);
    assert!((floored.cell_mm() - 2.0).abs() < 1e-12);
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
    // PR-8a, under the approved Checkpoint B: RampFinish is the ONE
    // generation consumer that moved off the legacy cell. Its expectation
    // changed here and the other three did not — which is exactly the
    // per-consumer property this file was built to hold.
    assert_policy(
        "ramp_finish",
        ramp_finish_generation_resolution(&t, tol),
        FinishResolutionMode::GeoMeanEnvelopeCusp,
        (envelope_quarter * cusp_quarter).sqrt(),
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
    let ball = ball_cutter(6.0);
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
    assert_eq!(
        adapter.heightmap.z_or_bbox_floor_values(),
        policy.heightmap.z_or_bbox_floor_values()
    );

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
        classification.heightmap.z_or_bbox_floor_values(),
        classification_policy.heightmap.z_or_bbox_floor_values()
    );
}
