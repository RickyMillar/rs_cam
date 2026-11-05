//! Perf sweep round 2, W3b: the interpolated holder/shank walk gives the
//! same report on rayon as the serial walk did.
//!
//! `check_collisions_interpolated_with_cancel` runs its moves in parallel
//! chunks. Each move reads only `moves[i - 1].target` and `moves[i]`, so
//! the moves are independent. This sentry keeps a verbatim copy of the
//! serial body from before the change (`reference_walk`) and compares the
//! full report: every event field, f64 values by `to_bits`, the event
//! order and `min_safe_stickout`.
//!
//! The toolpath has more than 4000 moves, so it crosses several chunk
//! boundaries, and the fixture strikes with the shank and the holder.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::{AtomicUsize, Ordering};

use rs_cam_core::geo::P3;
use rs_cam_core::interrupt::{CancelCheck, Cancelled, NeverCancel, check_cancel};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::stock::collision::{
    AssemblySegment, BODY_STRIKE_THRESHOLD_MM, CollisionEvent, CollisionKind, CollisionReport,
    HeadBand, ToolAssembly, body_segment_penetration_mm, check_collisions_interpolated_with_cancel,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// A copy of the private `interpolate_samples` from before the change.
fn reference_samples(prev: P3, target: P3, step_mm: f64) -> Vec<P3> {
    if step_mm <= 0.01 {
        return vec![target];
    }
    let dx = target.x - prev.x;
    let dy = target.y - prev.y;
    let dz = target.z - prev.z;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let n_steps = (dist / step_mm).ceil() as usize;
    if n_steps > 1 {
        let mut pts = Vec::with_capacity(n_steps + 1);
        for i in 0..=n_steps {
            let t = i as f64 / n_steps as f64;
            pts.push(P3::new(prev.x + t * dx, prev.y + t * dy, prev.z + t * dz));
        }
        pts
    } else {
        vec![target]
    }
}

/// The serial body of `check_collisions_interpolated_with_cancel` at HEAD
/// `bb179e8bd`, before W3b. Do not edit it: it is the reference.
fn reference_walk(
    toolpath: &Toolpath,
    assembly: &ToolAssembly,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    step_mm: f64,
    cancel: &dyn CancelCheck,
) -> Result<CollisionReport, Cancelled> {
    let segments = assembly.segments();
    let mut collisions = Vec::new();
    let mut max_extra_stickout_needed = 0.0_f64;

    for (move_idx, mv) in toolpath.moves.iter().enumerate() {
        check_cancel(cancel)?;
        let is_cutting = matches!(
            mv.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if !is_cutting {
            continue;
        }
        let prev = if move_idx > 0 {
            toolpath.moves[move_idx - 1].target
        } else {
            mv.target
        };
        let sample_points = reference_samples(prev, mv.target, step_mm);
        for tip in &sample_points {
            check_cancel(cancel)?;
            for band in &segments {
                let (z_offset, seg_radius) = (band.z_offset, band.radius);
                if seg_radius <= assembly.cutter_radius + 1e-6 {
                    continue;
                }
                let Some(penetration) =
                    body_segment_penetration_mm(*tip, z_offset, seg_radius, mesh, index)
                else {
                    continue;
                };
                if penetration > BODY_STRIKE_THRESHOLD_MM {
                    collisions.push(CollisionEvent {
                        move_index: move_idx,
                        position: *tip,
                        penetration_depth: penetration,
                        segment: band.segment,
                        kind: CollisionKind::Workpiece,
                    });
                    max_extra_stickout_needed = max_extra_stickout_needed.max(penetration);
                    break;
                }
            }
        }
    }

    let min_safe_stickout = assembly.stickout() + max_extra_stickout_needed;
    Ok(CollisionReport {
        collisions,
        min_safe_stickout,
    })
}

/// A 6 mm cutter with a 10 mm shank (it strikes) and a 35 mm holder at a
/// 15 mm stickout (it strikes the 20 mm hemisphere too).
fn assembly() -> ToolAssembly {
    ToolAssembly {
        cutter_radius: 3.0,
        cutter_length: 8.0,
        shank_diameter: 10.0,
        head: vec![HeadBand::holder_at_face(35.0, 15.0)],
        stickout: 15.0,
    }
}

/// A raster over the hemisphere with short moves at varied Z, an arc every
/// seventh move and a rapid every thirteenth move: 4000+ moves.
fn toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-30.0, -28.0, 30.0));
    let mut k = 0usize;
    let mut row = 0usize;
    let mut y = -28.0;
    while y <= 28.0 {
        let forward = row.is_multiple_of(2);
        let mut step = 0usize;
        while step <= 40 {
            let t = step as f64 / 40.0;
            let x = if forward {
                -30.0 + 60.0 * t
            } else {
                30.0 - 60.0 * t
            };
            // Z moves between 0 and 9 mm so some samples are clear and
            // some strike.
            let z = 4.5 + 4.5 * ((k as f64) * 0.37).sin();
            let target = P3::new(x, y, z);
            if k.is_multiple_of(13) {
                tp.rapid_to(target);
            } else if k.is_multiple_of(7) {
                tp.arc_cw_to(target, 0.5, 0.5, 900.0);
            } else if k.is_multiple_of(11) {
                tp.arc_ccw_to(target, -0.5, 0.25, 900.0);
            } else {
                tp.feed_to(target, 1200.0);
            }
            k += 1;
            step += 1;
        }
        y += 0.5;
        row += 1;
    }
    tp
}

fn assert_same(got: &CollisionReport, want: &CollisionReport) {
    assert_eq!(got.collisions.len(), want.collisions.len(), "event count");
    for (i, (g, w)) in got.collisions.iter().zip(&want.collisions).enumerate() {
        assert_eq!(g.move_index, w.move_index, "event {i}: move_index");
        assert_eq!(
            g.position.x.to_bits(),
            w.position.x.to_bits(),
            "event {i}: x"
        );
        assert_eq!(
            g.position.y.to_bits(),
            w.position.y.to_bits(),
            "event {i}: y"
        );
        assert_eq!(
            g.position.z.to_bits(),
            w.position.z.to_bits(),
            "event {i}: z"
        );
        assert_eq!(
            g.penetration_depth.to_bits(),
            w.penetration_depth.to_bits(),
            "event {i}: penetration"
        );
        assert_eq!(g.segment, w.segment, "event {i}: segment");
        assert_eq!(g.kind, w.kind, "event {i}: kind");
    }
    assert_eq!(
        got.min_safe_stickout.to_bits(),
        want.min_safe_stickout.to_bits(),
        "min_safe_stickout"
    );
}

#[test]
fn the_parallel_walk_gives_the_serial_report() {
    let mesh = make_test_hemisphere(20.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let asm = assembly();
    let tp = toolpath();
    assert!(tp.moves.len() > 4000, "{} moves", tp.moves.len());

    for step_mm in [1.0, 0.0] {
        let want = reference_walk(&tp, &asm, &mesh, &index, step_mm, &NeverCancel).unwrap();
        let got = check_collisions_interpolated_with_cancel(
            &tp,
            &asm,
            &mesh,
            &index,
            step_mm,
            &NeverCancel,
        )
        .unwrap();
        assert_same(&got, &want);

        // The fixture must strike with both bands, or the check is empty.
        assert!(
            want.collisions
                .iter()
                .any(|c| c.segment == AssemblySegment::Shank),
            "step {step_mm}: the shank must strike"
        );
        assert!(
            want.collisions
                .iter()
                .any(|c| c.segment == AssemblySegment::Holder),
            "step {step_mm}: the holder must strike"
        );
        let last = want.collisions.last().unwrap().move_index;
        assert!(
            last > 2048,
            "step {step_mm}: strikes past two chunks ({last})"
        );
        assert!(want.min_safe_stickout > asm.stickout());
    }
}

#[test]
fn a_preset_cancel_returns_cancelled() {
    let mesh = make_test_hemisphere(20.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let asm = assembly();
    let tp = toolpath();
    let cancel = || true;
    assert!(reference_walk(&tp, &asm, &mesh, &index, 1.0, &cancel).is_err());
    assert!(
        check_collisions_interpolated_with_cancel(&tp, &asm, &mesh, &index, 1.0, &cancel).is_err()
    );
}

#[test]
fn a_cancel_on_the_second_poll_returns_cancelled() {
    let mesh = make_test_hemisphere(20.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let asm = assembly();
    let tp = toolpath();
    let polls = AtomicUsize::new(0);
    let cancel = || polls.fetch_add(1, Ordering::SeqCst) >= 1;
    assert!(
        check_collisions_interpolated_with_cancel(&tp, &asm, &mesh, &index, 1.0, &cancel).is_err()
    );
}

#[test]
fn an_empty_toolpath_with_a_preset_cancel_is_clear() {
    let mesh = make_test_hemisphere(20.0, 32);
    let index = SpatialIndex::build_auto(&mesh);
    let asm = assembly();
    let tp = Toolpath::new();
    let cancel = || true;
    let want = reference_walk(&tp, &asm, &mesh, &index, 1.0, &cancel).unwrap();
    let got =
        check_collisions_interpolated_with_cancel(&tp, &asm, &mesh, &index, 1.0, &cancel).unwrap();
    assert_same(&got, &want);
}
