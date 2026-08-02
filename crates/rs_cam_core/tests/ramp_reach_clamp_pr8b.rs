//! PR-8b (H3): the ramp-finish reach clamp and its diagnostic channel.
//!
//! Oracle: `planning/review_2026-07-29/CHECKPOINT_B_EVIDENCE.md` §8.2 —
//! "RampFinish gouges 4.2 mm on `patches + hole` at every resolution (45–67
//! samples past 50 µm) — the ramp descends into a 62° cone the tool profile
//! cannot enter. A genuine reach failure with **no diagnostic channel**; a
//! user would ship it."
//!
//! ## What the measurement said, and where §8.2's hypothesis was wrong
//!
//! Two separate commanded-below-reach errors produce that number, and only
//! one of them is about the cone:
//!
//! 1. **The ladder bottom.** `ramp_finish` took its lowest Z from
//!    `SurfaceHeightmap::min_z()`, the minimum over ALL cells — and a finish
//!    grid is padded by one envelope radius per side, so uncovered cells
//!    carrying the `min_z` clamp are always present. The ladder bottom was
//!    therefore the MESH BBOX FLOOR on essentially every ramp-finish run. On
//!    this fixture: −3.000 requested, −2.407 holdable.
//!    [`the_canonical_reach_policy_refuses_the_requested_depth_in_the_cone`]
//!    corroborates that independently through `rs_cam_core::reach`.
//! 2. **The per-point blend.** `ramp_between_contours` pairs two contours by
//!    arc-length parameter after `match_contours` paired the loops by nearest
//!    centroid; neither correspondence is geometric, so a blended point can
//!    land anywhere between the two loops.
//!
//! **(2) is what produces the 4.2 mm, and it is NOT in the cone.** The
//! deepest gouge sits at (3.981, 4.113) — the flank of a convex 50° DOME —
//! path Z −1.757 against a reachable tool-centre surface at +2.472. Clamping
//! only the ladder bottom leaves it at −4.229 unchanged (probed during
//! implementation, then reverted). A convex dome flank has no valley, no rim
//! and no walls, so no cross-section reach model has anything to say about
//! it; the clamp has to be per point, against the surface itself. That is
//! the fifth time in this programme that a confidently-named mechanism was
//! not the cause.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::config::ToolpathStats;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ramp_finish::{
    RampFinishParams, RampReachClamp, ramp_finish_toolpath_structured_annotated,
};
use rs_cam_core::reach::{LocalValley, ValleySide, solve_reach};
use rs_cam_core::tool::{BallEndmill, TaperedBallEndmill};

/// The project's finishing tool: Ø1 tip, 7° half-angle, Ø6 shank.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

const HALF: f64 = 8.0;
const MESH_STEP: f64 = 0.25;

fn height_field(z: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let n = ((2.0 * HALF) / MESH_STEP).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        let y = -HALF + j as f64 * MESH_STEP;
        for i in 0..n {
            let x = -HALF + i as f64 * MESH_STEP;
            vertices.push(P3::new(x, y, z(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * n + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The Checkpoint B `patches + hole` fixture, verbatim: two disconnected 50°
/// domes plus a 62° conical pit 1.6 mm in radius and 3 mm deep.
fn disconnected_patches() -> TriangleMesh {
    height_field(|x, y| {
        let dome = |cx: f64, cy: f64| {
            let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            if r < 2.5 { 3.0 * (1.0 - r / 2.5) } else { 0.0 }
        };
        let pit = {
            let r = (x * x + y * y).sqrt();
            if r < 1.6 { -3.0 * (1.0 - r / 1.6) } else { 0.0 }
        };
        dome(-4.5, -4.5) + dome(4.5, 4.5) + pit
    })
}

/// A plain 40° cone-free slope: nothing on it is out of reach for this tool,
/// so the clamp must be measurably INERT — the control that stops the clamp
/// from being a thing that always fires.
fn gentle_ramp() -> TriangleMesh {
    height_field(|x, _y| 3.0 + 0.3 * x)
}

fn params() -> RampFinishParams {
    RampFinishParams {
        max_stepdown: 0.5,
        tolerance: 0.10,
        ..Default::default()
    }
}

fn run(mesh: &TriangleMesh, cutter: &dyn rs_cam_core::tool::MillingCutter) -> RampReachClamp {
    let index = SpatialIndex::build(mesh, 2.0);
    let (_tp, _anns, clamp) =
        ramp_finish_toolpath_structured_annotated(mesh, &index, cutter, &params(), None, None);
    clamp
}

// ── 1. The canonical reach policy, as the independent oracle ─────────────

/// The ladder half of the defect, checked against `rs_cam_core::reach`
/// rather than against the surface the production clamp uses — two
/// independent routes to the same verdict.
///
/// The pit is a 1.6 mm-radius, 3 mm-deep cone: walls at atan(3/1.6) = 61.9°
/// from horizontal. `solve_reach` REFUSES the requested 3 mm depth for this
/// taper (the profile wedges between the walls) and does not refuse a
/// shallow one — so "the cutter cannot hold this depth here" is the
/// canonical policy's own verdict on the number `min_z()` was handing the
/// ladder, not a rule PR-8b invented.
///
/// The production clamp does NOT call this: for a per-point clamp the
/// drop-cutter contact query answers the same question exactly, against the
/// real mesh instead of a V model, and `reach`'s own module doc already
/// records solving directly against the sampled cross-section as the strict
/// generalisation of the V. Re-deriving a coarser model beside an exact one
/// is what PR-6b declined to do in `decompose`, for the same reason.
#[test]
fn the_canonical_reach_policy_refuses_the_requested_depth_in_the_cone() {
    let t = taper();
    let wall = (3.0_f64 / 1.6).atan();
    assert!(
        (wall.to_degrees() - 61.9).abs() < 0.2,
        "fixture wall angle drifted: {:.2}°",
        wall.to_degrees()
    );
    let side = || ValleySide::from_wall_angle(1.6, wall);

    let requested = solve_reach(
        &t,
        &LocalValley {
            rest_depth_mm: 3.0,
            left: side(),
            right: side(),
        },
    );
    assert!(
        requested.refused,
        "the policy must refuse the 3 mm the ladder was asking for: {requested:?}"
    );

    let shallow = solve_reach(
        &t,
        &LocalValley {
            rest_depth_mm: 0.3,
            left: side(),
            right: side(),
        },
    );
    assert!(
        !shallow.refused,
        "the same cone at a depth the profile fits must NOT refuse, or this \
         fixture proves nothing about depth"
    );
}

// ── 2. The clamp, measured ───────────────────────────────────────────────

/// The clamp fires on the cone fixture, and both halves of the defect are
/// recorded with their magnitudes.
#[test]
fn the_clamp_records_what_it_prevented_on_the_cone_fixture() {
    let clamp = run(&disconnected_patches(), &taper());
    assert!(clamp.ramp_points > 100, "non-vacuity: {clamp:?}");
    assert!(
        clamp.max_lift_mm > 4.0,
        "the §8.2 4.2 mm defect must still be REPRODUCED by the fixture and \
         PREVENTED by the clamp; max lift {:.4} mm",
        clamp.max_lift_mm
    );
    assert!(
        clamp.clamped_points * 2 > clamp.ramp_points,
        "{} of {} points were commanded below reach — the majority, which is \
         the scale of the defect",
        clamp.clamped_points,
        clamp.ramp_points
    );
    // The ladder half, independently: the requested bottom is the mesh bbox
    // floor (`min_z()`'s uncovered clamp) and the holdable one is not.
    assert!((clamp.requested_bottom_z_mm + 3.0).abs() < 1e-9);
    assert!(
        clamp.holdable_bottom_z_mm > -2.5 && clamp.holdable_bottom_z_mm < -2.3,
        "holdable bottom {:.4} mm — the closed-form ball-in-cone contact for \
         this taper and this cone is −2.439",
        clamp.holdable_bottom_z_mm
    );
    assert!(!clamp.is_inert());
}

/// **The clamp fires on a FLAT PLANE**, and that is the finding, not a bug
/// in the clamp.
///
/// `gentle_ramp` is a single 17° plane. There is no pit, no wedge, no
/// unreachable depth anywhere on it, and the ladder bottom needs no lifting
/// (requested 0.600 mm against 0.638 mm holdable — the tool-centre offset,
/// nothing more). Yet 567 of 1182 ramp points sit below the reachable
/// surface, by up to 4.71 mm, which is very nearly the plane's entire 4.8 mm
/// height.
///
/// So the blend defect described in this file's header is GENERIC, not a
/// property of the Checkpoint B fixture: `match_contours` +
/// `ramp_between_contours` pair two loops of different perimeter by
/// arc-length fraction, and on a plane the two level loops run down opposite
/// sides of the model, so the "interpolated" point crosses the middle of the
/// part at an interpolated Z. Every ramp-finish operation on every model has
/// been doing this.
///
/// This is asserted rather than described because it is the load-bearing
/// consequence: the PR-8b diagnostic will fire on essentially every
/// ramp-finish toolpath, and a reader who assumes otherwise will mis-read
/// the channel as rare. Fixing the correspondence itself is a rewrite of the
/// op's core and is NOT attempted here — the clamp makes the emitted path
/// safe and makes the defect audible, in that order.
#[test]
fn the_clamp_fires_even_on_a_plane_because_the_blend_is_the_defect() {
    let clamp = run(&gentle_ramp(), &taper());
    assert!(clamp.ramp_points > 100, "non-vacuity: {clamp:?}");
    // The LADDER is fine here — the plane has no unreachable depth — which
    // is what isolates the blend as the sole cause.
    assert!(
        (clamp.holdable_bottom_z_mm - clamp.requested_bottom_z_mm).abs() < 0.2,
        "this fixture must NOT exercise the ladder half: {:.4} -> {:.4}",
        clamp.requested_bottom_z_mm,
        clamp.holdable_bottom_z_mm
    );
    assert!(
        clamp.clamped_points > 0 && clamp.max_lift_mm > 1.0,
        "a plane must still expose the blend defect, or this file's central \
         claim is wrong: {clamp:?}"
    );
    println!(
        "17° PLANE: {} of {} ramp points below reach, max lift {:.4} mm on a \
         4.8 mm-tall plane (ladder {:.4} -> {:.4}, unmoved)",
        clamp.clamped_points,
        clamp.ramp_points,
        clamp.max_lift_mm,
        clamp.requested_bottom_z_mm,
        clamp.holdable_bottom_z_mm
    );
}

/// The clamp is about the CUTTER, not the model: a Ø3 ball is a different
/// profile in the same 62° cone and lands a different holdable bottom.
/// Recorded rather than asserted to a magic number — the point is that the
/// clamp is tool-aware, which a fixed depth limit would not be.
#[test]
fn the_holdable_bottom_follows_the_cutter_profile() {
    let mesh = disconnected_patches();
    let tapered = run(&mesh, &taper());
    let ball = run(&mesh, &BallEndmill::new(3.0, 25.0));
    assert!(
        ball.holdable_bottom_z_mm > tapered.holdable_bottom_z_mm,
        "a Ø3 ball is fatter at the tip than a Ø1-tip taper, so it must wedge \
         HIGHER in the same cone: ball {:.4} vs taper {:.4}",
        ball.holdable_bottom_z_mm,
        tapered.holdable_bottom_z_mm
    );
    println!(
        "holdable bottom: Ø1-tip taper {:.4} mm, Ø3 ball {:.4} mm \
         (requested {:.4} mm on both)",
        tapered.holdable_bottom_z_mm, ball.holdable_bottom_z_mm, tapered.requested_bottom_z_mm
    );
}

// ── 3. The diagnostic channel §8.2 said did not exist ────────────────────

fn diagnostics(clamp: Option<RampReachClamp>) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(3),
        &ToolpathStats {
            ramp_reach_clamp: clamp.map(Box::new),
            ..ToolpathStats::default()
        },
    )
}

/// A truncated descent reaches the operator, with the numbers that make it
/// actionable — and it says the material is LEFT, because that is the part
/// no simulation can tell them: the emitted path is safe, so a dexel run
/// replays a clean pass.
#[test]
fn a_truncated_descent_is_reported_with_its_magnitudes() {
    let measured = run(&disconnected_patches(), &taper());
    let out = diagnostics(Some(measured));
    assert_eq!(out.len(), 1, "{out:#?}");
    let d = &out[0];
    assert_eq!(d.id.as_str(), ids::GEOM_RAMP_REACH_CLAMP);
    assert_eq!(d.severity, Severity::Caution);
    for needle in ["4.231", "LEFT", "-3.000", "-2.407"] {
        assert!(
            d.message.contains(needle),
            "message must carry {needle:?}: {}",
            d.message
        );
    }
}

/// A/M9's rule, applied to a second measure (`MEASUREMENT_DOMAINS.md` X-19):
/// "no descent ran" and "a descent ran and everything was holdable" are
/// different states, and NEITHER is a defect. Both must be silent, and the
/// difference must survive to the report.
///
/// The inert case is CONSTRUCTED rather than generated, deliberately: as
/// [`the_clamp_fires_even_on_a_plane_because_the_blend_is_the_defect`]
/// shows, there is currently no mesh that produces one. Testing the
/// adapter's silence rule against a value no generator can currently make is
/// the honest way round — the rule is the adapter's, and it must hold the
/// day the blend is fixed and inert clamps become the norm.
#[test]
fn nothing_is_reported_when_nothing_was_measured_or_nothing_moved() {
    assert!(
        diagnostics(None).is_empty(),
        "`None` = no ramp descent ran; that is not a claim of any kind"
    );
    let inert = RampReachClamp {
        clamped_points: 0,
        ramp_points: 900,
        max_lift_mm: 0.0,
        requested_bottom_z_mm: -1.25,
        holdable_bottom_z_mm: -1.25,
        // C8: a ramp path ran and lifted nothing — a MEASURED zero, which is
        // the case `None` must stay distinct from.
        lifted_area_mm2: Some(0.0),
    };
    assert!(inert.is_inert());
    assert!(
        diagnostics(Some(inert)).is_empty(),
        "an inert clamp is a measured-clean descent, not a defect"
    );
    // A ladder that moved is NOT inert even with zero points lifted: two
    // terraces of commanded depth were removed before any point existed.
    let ladder_only = RampReachClamp {
        holdable_bottom_z_mm: -0.75,
        ..inert
    };
    assert!(!ladder_only.is_inert());
    assert_eq!(diagnostics(Some(ladder_only)).len(), 1);
}

// ── C8: the standing-material AREA channel ───────────────────────────────

/// C8 (`ANTIPATTERNS_BACKLOG.md` P8: "RampFinish has no standing-material
/// area channel (lift magnitude only)").
///
/// PR-8b's finding carried `max_lift_mm` — one worst-case DEPTH with no
/// extent. "Up to 5 mm left standing" could mean one stray point or half the
/// part, and nothing in the report said which. The clamp now measures the XY
/// swath of ramp path it lifted, following A/M9's Option-typed contract with
/// its own domain/stage provenance.
#[test]
fn the_clamp_measures_the_area_it_leaves_standing() {
    let clamp = run(&disconnected_patches(), &taper());

    let (area, provenance) = clamp.lifted_area().expect(
        "a ramp path ran, so the swath is MEASURED — `None` here would \
                 mean no path was built at all",
    );
    println!(
        "C8 ramp swath: {:.3} mm² over {} lifted of {} points, worst lift {:.3} mm [{}]",
        area.mm2(),
        clamp.clamped_points,
        clamp.ramp_points,
        clamp.max_lift_mm,
        provenance.describe()
    );

    assert!(
        clamp.clamped_points > 0,
        "non-vacuity: this fixture must actually clamp, or the area proves nothing"
    );
    assert!(
        area.mm2() > 0.0,
        "points were lifted, so the swath must have real extent: {}",
        area.mm2()
    );

    // The provenance must NAME its domain and stage, and must not be
    // mistakable for the ring-cascade residual it sits beside in
    // `ToolpathStats`. M1's non-negotiable rule 3.
    let described = provenance.describe();
    assert!(
        described.contains("ramp-finish reach-clamp swath"),
        "the stage must be this measure's own, not borrowed: {described}"
    );
    assert!(
        described.to_lowercase().contains("cusp"),
        "the resolution note must say what width the swath assumed — the \
         ENVELOPE would be the shank on a tapered tool, three times too wide \
         (C3): {described}"
    );
    assert_ne!(
        provenance,
        rs_cam_core::scallop::ScallopReport::PROVENANCE,
        "a path swath is not a ring-cascade residual; the two must never be \
         summed or ratio'd"
    );

    // Sanity: the swath cannot exceed the whole fixture footprint by more
    // than the overlap the note admits to. A wildly larger number would mean
    // the width or the segment sum is wrong, not that more was left.
    assert!(
        area.mm2() < 20_000.0,
        "swath area is implausible for a 16x16 mm fixture: {}",
        area.mm2()
    );
}

/// The diagnostic must CARRY the area, not just hold it. A channel nobody
/// prints is the failure mode this whole programme keeps re-learning.
#[test]
fn the_reported_magnitudes_now_include_the_area() {
    let clamp = run(&disconnected_patches(), &taper());
    let diags = diagnostics(Some(clamp));
    assert_eq!(diags.len(), 1);
    let message = &diags[0].message;
    assert!(
        message.contains("mm² of ramp swath"),
        "the diagnostic must state the extent: {message}"
    );
    assert!(
        message.contains("ramp-finish reach-clamp swath"),
        "…with its provenance, so nobody compares it to a ring residual: {message}"
    );
}
