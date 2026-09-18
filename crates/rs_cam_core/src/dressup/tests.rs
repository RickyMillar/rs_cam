//! Unit tests for the dressup transforms. Moved out of `dressup/mod.rs`
//! by P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::redundant_clone
)]

use super::*;
use crate::toolpath::MoveIntent;

fn simple_plunge_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    // Rapid to start
    tp.rapid_to(P3::new(10.0, 10.0, 10.0));
    // Plunge straight down
    tp.feed_to(P3::new(10.0, 10.0, -3.0), 500.0);
    // Cut along X
    tp.feed_to(P3::new(50.0, 10.0, -3.0), 1000.0);
    tp.feed_to(P3::new(50.0, 50.0, -3.0), 1000.0);
    // Retract
    tp.rapid_to(P3::new(50.0, 50.0, 10.0));
    tp
}

// --- Ramp entry tests ---

#[test]
fn test_ramp_entry_replaces_plunge() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    // The ramp body itself (MoveIntent::EntryRamp) must not be a
    // straight plunge — that's the dressup's whole point. The
    // lift-bridge safety guard (UX-dial-in B1, 2026-05-21) may emit
    // a separate plunge-feed segment tagged `EntryPlunge` so the
    // descent through stock above the ramp doesn't punch through
    // uncut material; that segment is intentionally vertical and
    // is excluded from the "ramp must have XY" assertion below.
    for i in 1..result.moves.len() {
        let intent = result.moves[i].intent;
        if intent != crate::toolpath::MoveIntent::EntryRamp {
            continue;
        }
        if let MoveType::Linear { .. } = result.moves[i].move_type {
            let prev = &result.moves[i - 1].target;
            let curr = &result.moves[i].target;
            let dz = (curr.z - prev.z).abs();
            let tdx = curr.x - prev.x;
            let tdy = curr.y - prev.y;
            let dxy = (tdx * tdx + tdy * tdy).sqrt();
            if dz > 1.0 {
                assert!(
                    dxy > 0.1,
                    "Ramp body should have XY movement during Z descent: dz={dz}, dxy={dxy}",
                );
            }
        }
    }
}

#[test]
fn test_ramp_entry_reaches_target_z() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Ramp { max_angle_deg: 5.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    // Should still reach the cutting depth
    let has_cut_depth = result
        .moves
        .iter()
        .any(|m| (m.target.z - -3.0).abs() < 0.01);
    assert!(has_cut_depth, "Ramp should reach cut_depth=-3.0");
}

#[test]
fn test_ramp_preserves_cutting_moves() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    // The cutting moves at -3.0 should still be present
    let cut_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            })
            .collect();
    assert!(cut_moves.len() >= 2, "Cutting moves should be preserved");
}

// --- Helix entry tests ---

#[test]
fn test_helix_entry_replaces_plunge() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Helix {
            radius: 2.0,
            pitch: 1.0,
        },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    // Should have many intermediate moves (helix steps)
    assert!(
        result.moves.len() > tp.moves.len(),
        "Helix should add intermediate moves: {} vs {}",
        result.moves.len(),
        tp.moves.len()
    );
}

#[test]
fn test_helix_entry_reaches_target_z() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Helix {
            radius: 2.0,
            pitch: 1.0,
        },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    let has_cut_depth = result.moves.iter().any(|m| (m.target.z - -3.0).abs() < 0.1);
    assert!(has_cut_depth, "Helix should reach cut_depth=-3.0");
}

#[test]
fn test_helix_moves_are_circular() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        EntryStyle::Helix {
            radius: 3.0,
            pitch: 1.0,
        },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;

    // Helix moves should be within radius of center (10, 10)
    let helix_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-10)
                    && m.target.z < 9.0
                    && m.target.z > -3.5
            })
            .collect();

    for m in &helix_moves {
        let hdx = m.target.x - 10.0;
        let hdy = m.target.y - 10.0;
        let dist = (hdx * hdx + hdy * hdy).sqrt();
        assert!(
            dist < 3.5,
            "Helix point ({}, {}) is {} from center, expected ~3.0",
            m.target.x,
            m.target.y,
            dist
        );
    }
}

// --- Entry-descent optimization tests ---

/// Builds a minimal `[Rapid(safe_z)] -> [Linear(EntryPlunge)]` entry at
/// `(x, y)`, matching the pattern every generator's shared emitter
/// (`Toolpath::emit_path_segment_with_intent` /
/// `emit_closed_contour_with_intent`) produces.
fn entry_toolpath(x: f64, y: f64, safe_z: f64, plunge_target_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(x, y, safe_z), crate::toolpath::MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(x, y, plunge_target_z),
        500.0,
        crate::toolpath::MoveIntent::EntryPlunge,
    );
    tp
}

#[test]
fn optimize_entry_descents_splits_on_fresh_stock_top() {
    let mut tp = entry_toolpath(5.0, 5.0, 10.0, -1.0);
    let split_count = optimize_entry_descents(&mut tp, None, 0.0, 3.0, &probe_cutter(), None);

    assert_eq!(split_count, 1, "expected exactly one split");
    assert_eq!(tp.moves.len(), 3, "moves: {:?}", tp.moves);
    assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
    assert!((tp.moves[0].target.z - 10.0).abs() < 1e-10);

    let inserted = &tp.moves[1];
    assert_eq!(inserted.move_type, MoveType::Rapid);
    assert_eq!(inserted.intent, crate::toolpath::MoveIntent::Linking);
    assert!(
        (inserted.target.z - (0.0 + crate::toolpath::PLUNGE_CLEARANCE_MM)).abs() < 1e-10,
        "expected inserted rapid at fresh_stock_top_z + PLUNGE_CLEARANCE_MM, got {}",
        inserted.target.z
    );
    assert!((inserted.target.x - 5.0).abs() < 1e-10);
    assert!((inserted.target.y - 5.0).abs() < 1e-10);

    assert!(matches!(tp.moves[2].move_type, MoveType::Linear { .. }));
    assert_eq!(tp.moves[2].intent, crate::toolpath::MoveIntent::EntryPlunge);
    assert!((tp.moves[2].target.z - (-1.0)).abs() < 1e-10);
}

#[test]
fn optimize_entry_descents_uses_dexel_ceiling_above_mesh() {
    // Flat stock whose Z-grid top is 5.0 everywhere — well above the
    // fresh_stock_top_z passed in, so a correct implementation must
    // read the ceiling from the dexel, not the fresh-stock fallback.
    //
    // A/M10 re-pin: the ceiling now comes from the sliver-safe bound
    // (`max_conservative_top_z_in_disc`), which on untouched stock is
    // exactly the stock top — and the `2 * cell_size` resolution pad
    // this test used to assert is gone, because the bound is
    // conservative by construction rather than by heuristic. On this
    // fixture the two agree on the ceiling and differ by the pad, so
    // the expected Z drops from 9.0 to 7.0.
    let stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 5.0, 1.0);

    let mut tp = entry_toolpath(5.0, 5.0, 10.0, -1.0);
    let split_count =
        optimize_entry_descents(&mut tp, Some(&stock), 0.0, 3.0, &probe_cutter(), None);

    assert_eq!(split_count, 1, "expected exactly one split");
    let inserted = &tp.moves[1];
    assert_eq!(inserted.move_type, MoveType::Rapid);
    assert!(
        (inserted.target.z - (5.0 + crate::toolpath::PLUNGE_CLEARANCE_MM)).abs() < 1e-10,
        "expected inserted rapid at the sliver-safe ceiling (5.0) + \
             PLUNGE_CLEARANCE_MM, got {}",
        inserted.target.z
    );
}

#[test]
fn optimize_entry_descents_no_split_when_no_headroom() {
    // Ceiling + clearance sits above (or too close to) the rapid's z:
    // stock top at 9.0 + 2.0 clearance = 11.0 > safe_z=10.0 — no room.
    let stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 9.0, 1.0);
    let mut tp = entry_toolpath(5.0, 5.0, 10.0, -1.0);
    let split_count =
        optimize_entry_descents(&mut tp, Some(&stock), 0.0, 3.0, &probe_cutter(), None);

    assert_eq!(split_count, 0, "no split expected when there's no headroom");
    assert_eq!(tp.moves.len(), 2, "moves unchanged: {:?}", tp.moves);
}

#[test]
fn optimize_entry_descents_no_split_on_xy_mismatch() {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(
        P3::new(5.0, 5.0, 10.0),
        crate::toolpath::MoveIntent::Linking,
    );
    // Different XY from the rapid — not a vertical entry.
    tp.feed_to_with_intent(
        P3::new(6.0, 5.0, -1.0),
        500.0,
        crate::toolpath::MoveIntent::EntryPlunge,
    );
    let split_count = optimize_entry_descents(&mut tp, None, 0.0, 3.0, &probe_cutter(), None);

    assert_eq!(split_count, 0, "no split expected on XY mismatch");
    assert_eq!(tp.moves.len(), 2, "moves unchanged: {:?}", tp.moves);
}

#[test]
fn optimize_entry_descents_no_split_on_non_entry_plunge_intent() {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(
        P3::new(5.0, 5.0, 10.0),
        crate::toolpath::MoveIntent::Linking,
    );
    // Same XY and descending, but not tagged EntryPlunge.
    tp.feed_to_with_intent(
        P3::new(5.0, 5.0, -1.0),
        500.0,
        crate::toolpath::MoveIntent::FinishingCut,
    );
    let split_count = optimize_entry_descents(&mut tp, None, 0.0, 3.0, &probe_cutter(), None);

    assert_eq!(
        split_count, 0,
        "no split expected when the plunge isn't tagged EntryPlunge"
    );
    assert_eq!(tp.moves.len(), 2, "moves unchanged: {:?}", tp.moves);
}

// --- Tab/bridge tests ---

fn profile_toolpath_for_tabs() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -5.0), 500.0); // plunge
    // Square profile at Z=-5
    tp.feed_to(P3::new(100.0, 0.0, -5.0), 1000.0);
    tp.feed_to(P3::new(100.0, 100.0, -5.0), 1000.0);
    tp.feed_to(P3::new(0.0, 100.0, -5.0), 1000.0);
    tp.feed_to(P3::new(0.0, 0.0, -5.0), 1000.0); // close
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp
}

#[test]
fn test_tabs_lift_at_positions() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(4, 5.0, 3.0);
    let result = apply_tabs(tp.clone(), &tabs, -5.0);

    // Some moves should be at tab height (-5 + 3 = -2)
    let tab_moves: Vec<_> = result
        .moves
        .iter()
        .filter(|m| (m.target.z - -2.0).abs() < 0.01)
        .collect();
    assert!(
        !tab_moves.is_empty(),
        "Should have moves at tab height (-2.0)"
    );
}

#[test]
fn test_tabs_preserve_non_tab_moves() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(2, 3.0, 2.0);
    let result = apply_tabs(tp.clone(), &tabs, -5.0);

    // Should still have moves at cut_depth
    let cut_moves: Vec<_> = result
        .moves
        .iter()
        .filter(|m| (m.target.z - -5.0).abs() < 0.01)
        .collect();
    assert!(
        !cut_moves.is_empty(),
        "Non-tab cutting moves should be preserved"
    );
}

#[test]
fn test_no_tabs_returns_unchanged() {
    let tp = profile_toolpath_for_tabs();
    let result = apply_tabs(tp.clone(), &[], -5.0);
    assert_eq!(result.moves.len(), tp.moves.len());
}

#[test]
fn test_even_tabs_spacing() {
    let tabs = even_tabs(4, 5.0, 3.0);
    assert_eq!(tabs.len(), 4);
    assert!((tabs[0].position - 0.0).abs() < 1e-10);
    assert!((tabs[1].position - 0.25).abs() < 1e-10);
    assert!((tabs[2].position - 0.5).abs() < 1e-10);
    assert!((tabs[3].position - 0.75).abs() < 1e-10);

    for tab in &tabs {
        assert!((tab.width - 5.0).abs() < 1e-10);
        assert!((tab.height - 3.0).abs() < 1e-10);
    }
}

#[test]
fn test_tabs_have_sharp_transitions() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(2, 10.0, 3.0);
    let result = apply_tabs(tp.clone(), &tabs, -5.0);

    // Find vertical step-up feed moves (same XY, Z increases sharply)
    let mut found_step_up = false;
    for i in 1..result.moves.len() {
        if !matches!(result.moves[i].move_type, MoveType::Linear { .. }) {
            continue;
        }
        let prev = &result.moves[i - 1].target;
        let curr = &result.moves[i].target;
        let sdx = curr.x - prev.x;
        let sdy = curr.y - prev.y;
        let dxy = (sdx * sdx + sdy * sdy).sqrt();
        let dz = curr.z - prev.z;
        if dxy < 0.01 && dz > 1.0 && curr.z < 0.0 {
            found_step_up = true;
            // Step-up should go to tab height (-5 + 3 = -2)
            assert!(
                (curr.z - -2.0).abs() < 0.1,
                "Step-up should reach tab height -2.0, got {}",
                curr.z
            );
        }
    }
    assert!(found_step_up, "Should have at least one sharp step-up move");
}

// --- Lead-in/out tests ---

#[test]
fn test_lead_in_adds_arc_moves() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_lead_in_out(
        AnnotatedToolpath::new(tp.clone()),
        2.0,
        None,
        None,
        None,
        None,
    ))
    .toolpath;

    // Should have more moves than original (arc segments added)
    assert!(
        result.moves.len() > tp.moves.len(),
        "Lead-in should add arc moves: {} vs {}",
        result.moves.len(),
        tp.moves.len()
    );
}

#[test]
fn test_lead_in_reaches_cut_point() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_lead_in_out(
        AnnotatedToolpath::new(tp.clone()),
        2.0,
        None,
        None,
        None,
        None,
    ))
    .toolpath;

    // The cut moves at x=50, y=10, z=-3 should still be reachable
    let has_first_cut = result.moves.iter().any(|m| {
        (m.target.z - (-3.0)).abs() < 0.01
            && (m.target.x - 10.0).abs() < 3.0
            && (m.target.y - 10.0).abs() < 3.0
    });
    assert!(
        has_first_cut,
        "Lead-in should arrive near the first cut point"
    );
}

#[test]
fn test_lead_in_preserves_rapids() {
    let tp = simple_plunge_toolpath();
    let result = without_provenance(apply_lead_in_out(
        AnnotatedToolpath::new(tp.clone()),
        2.0,
        None,
        None,
        None,
        None,
    ))
    .toolpath;

    // Should still have a rapid move
    let has_rapid = result.moves.iter().any(|m| m.move_type == MoveType::Rapid);
    assert!(has_rapid, "Lead-in should preserve rapid moves");
}

// --- Dogbone tests ---

fn square_profile_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
    // Square at Z=-3
    tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
    tp.feed_to(P3::new(50.0, 50.0, -3.0), 1000.0);
    tp.feed_to(P3::new(0.0, 50.0, -3.0), 1000.0);
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 1000.0);
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp
}

#[test]
fn test_dogbone_adds_overcuts() {
    let tp = square_profile_toolpath();
    let result = without_provenance(apply_dogbones(
        AnnotatedToolpath::new(tp.clone()),
        3.0,
        170.0,
    ))
    .toolpath;

    // Should have more moves than original (overcut + return at each corner)
    assert!(
        result.moves.len() > tp.moves.len(),
        "Dogbones should add moves: {} vs {}",
        result.moves.len(),
        tp.moves.len()
    );
}

#[test]
fn test_dogbone_overcut_distance() {
    let tp = square_profile_toolpath();
    let tool_radius = 3.0;
    let result = without_provenance(apply_dogbones(
        AnnotatedToolpath::new(tp.clone()),
        tool_radius,
        170.0,
    ))
    .toolpath;

    // Find overcut moves (moves that go away from the path)
    // At corner (50, 0): the overcut should be ~tool_radius from the corner
    for i in 1..result.moves.len() {
        let prev = result.moves[i - 1].target;
        let curr = result.moves[i].target;
        // Look for moves where next move returns to the same point (overcut + return)
        if i + 1 < result.moves.len() {
            let next = result.moves[i + 1].target;
            if (prev.x - next.x).abs() < 0.01
                && (prev.y - next.y).abs() < 0.01
                && (prev.z - curr.z).abs() < 0.01
            {
                // This is an overcut: prev → curr → next where prev ≈ next
                let odx = curr.x - prev.x;
                let ody = curr.y - prev.y;
                let dist = (odx * odx + ody * ody).sqrt();
                assert!(
                    (dist - tool_radius).abs() < 0.5,
                    "Overcut distance should be ~{}, got {}",
                    tool_radius,
                    dist
                );
            }
        }
    }
}

#[test]
fn test_dogbone_preserves_straight_segments() {
    // Straight line — no corners, no dogbones
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
    tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
    tp.feed_to(P3::new(100.0, 0.0, -3.0), 1000.0);
    tp.rapid_to(P3::new(100.0, 0.0, 10.0));

    let result = without_provenance(apply_dogbones(
        AnnotatedToolpath::new(tp.clone()),
        3.0,
        170.0,
    ))
    .toolpath;
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "Straight path should have no dogbones added"
    );
}

#[test]
fn test_dogbone_respects_angle_threshold() {
    // Shallow angle (170°) — should not trigger with default 170° threshold
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
    tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
    // Very slight turn (~5°)
    tp.feed_to(P3::new(100.0, 5.0, -3.0), 1000.0);
    tp.rapid_to(P3::new(100.0, 5.0, 10.0));

    let result = without_provenance(apply_dogbones(
        AnnotatedToolpath::new(tp.clone()),
        3.0,
        100.0,
    ))
    .toolpath; // threshold 100°
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "Shallow angle should not trigger dogbone"
    );
}

#[test]
fn test_tabs_add_transition_moves() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(4, 5.0, 3.0);
    let result = apply_tabs(tp.clone(), &tabs, -5.0);

    // Tab dressup adds step-up/step-down moves at tab edges
    assert!(
        result.moves.len() >= tp.moves.len(),
        "Tab dressup should add transition moves: {} vs {}",
        result.moves.len(),
        tp.moves.len()
    );
}

/// A tagged profile, the shape `ops/profile.rs` emits: every move carries a
/// real [`MoveIntent`], and none is `Unknown`.
fn tagged_profile_toolpath_for_tabs() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 10.0), MoveIntent::Retract);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, -5.0), 500.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(100.0, 0.0, -5.0), 1000.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(
        P3::new(100.0, 100.0, -5.0),
        1000.0,
        MoveIntent::FinishingCut,
    );
    tp.feed_to_with_intent(P3::new(0.0, 100.0, -5.0), 1000.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, -5.0), 1000.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 10.0), MoveIntent::Retract);
    tp
}

/// CUT-08 sentry: `apply_tabs` carries the move intent it replaces, and the
/// fed descents back to depth are plunges.
///
/// Before the fix every rewritten segment came out `MoveIntent::Unknown`,
/// because `apply_tabs` emitted through `Toolpath::feed_to`. A tabbed
/// finishing profile then left `metrology/spacing.rs`, which reads only
/// `FinishingCut`, and two fed vertical descents shipped untagged.
///
/// The step-up onto a tab keeps the CUT intent, not `Retract`: it removes
/// material, and `tests/retract_intent_move_type_census_w6.rs` forbids a
/// `Retract`-tagged `Linear` feed.
///
/// Two tab sets, because `apply_tabs` has two arms. `even_tabs` crosses a
/// zone boundary inside a segment (the split arm). The single wide tab
/// starts and ends exactly on a corner of the 100 mm square, so its segment
/// holds no boundary event (the whole-segment arm).
#[test]
fn apply_tabs_carries_the_move_intent_cut08() {
    // The whole-segment arm: one tab over the third side of the square.
    let wide_tab = vec![Tab {
        position: 250.0 / 400.0,
        width: 100.0,
        height: 3.0,
    }];
    for tabs in [even_tabs(4, 5.0, 3.0), wide_tab] {
        let tp = tagged_profile_toolpath_for_tabs();
        assert!(
            tp.moves.iter().all(|m| m.intent != MoveIntent::Unknown),
            "fixture precondition: the input carries no Unknown intent"
        );

        let result = apply_tabs(tp, &tabs, -5.0);

        let unknown = result
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::Unknown)
            .count();
        assert_eq!(
            unknown, 0,
            "apply_tabs emitted {unknown} moves with MoveIntent::Unknown"
        );

        let mut descents = 0_usize;
        let mut ascents = 0_usize;
        for i in 1..result.moves.len() {
            let mv = &result.moves[i];
            if !matches!(mv.move_type, MoveType::Linear { .. }) {
                continue;
            }
            let prev = &result.moves[i - 1].target;
            let curr = &mv.target;
            let sdx = curr.x - prev.x;
            let sdy = curr.y - prev.y;
            let dxy = (sdx * sdx + sdy * sdy).sqrt();
            let dz = curr.z - prev.z;
            if dxy >= 0.01 || curr.z >= 0.0 {
                continue;
            }
            if dz < -0.5 {
                descents += 1;
                assert_eq!(
                    mv.intent,
                    MoveIntent::EntryPlunge,
                    "a fed descent back to depth at move {i} must be an EntryPlunge"
                );
            } else if dz > 0.5 {
                ascents += 1;
                assert_eq!(
                    mv.intent,
                    MoveIntent::FinishingCut,
                    "the fed lift onto a tab at move {i} must keep the cut intent"
                );
            }
        }
        assert!(
            descents >= 1,
            "fixture precondition: the arm must emit a fed descent"
        );
        assert!(
            ascents >= 1,
            "fixture precondition: the arm must emit a fed lift"
        );

        // The horizontal cuts keep the finishing tag the spacing instrument
        // reads.
        assert!(
            result
                .moves
                .iter()
                .any(|m| m.intent == MoveIntent::FinishingCut),
            "the finishing tag must survive the rewrite"
        );
    }
}

// --- Link-vs-retract tests ---

/// Build a toolpath with two nearby passes (retract between them).
fn two_pass_toolpath(pass_gap: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    // First pass
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
    tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
    // Retract
    tp.rapid_to(P3::new(20.0, 0.0, 10.0));
    // Rapid to second pass start
    tp.rapid_to(P3::new(20.0 + pass_gap, 0.0, 10.0));
    // Plunge
    tp.feed_to(P3::new(20.0 + pass_gap, 0.0, -3.0), 500.0);
    // Second pass
    tp.feed_to(P3::new(40.0 + pass_gap, 0.0, -3.0), 1000.0);
    // Retract
    tp.rapid_to(P3::new(40.0 + pass_gap, 0.0, 10.0));
    tp
}

fn default_link_params() -> LinkMoveParams {
    LinkMoveParams {
        max_link_distance: 18.0, // 3× 6mm tool diameter
        link_feed_rate: 1000.0,
        safe_z_threshold: 10.0,
        tool_radius: 3.0, // half of the assumed 6mm tool diameter above
    }
}

#[test]
fn test_link_basic() {
    // 2mm gap between passes — should be linked
    let tp = two_pass_toolpath(2.0);
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(
        AnnotatedToolpath::new(tp.clone()),
        &params,
    ))
    .toolpath;

    // Should have fewer moves (retract+rapid+plunge replaced with feed)
    assert!(
        result.moves.len() < tp.moves.len(),
        "Link should reduce moves: {} vs {}",
        result.moves.len(),
        tp.moves.len()
    );

    // Should have less rapid distance
    assert!(
        result.total_rapid_distance() < tp.total_rapid_distance(),
        "Link should reduce rapids: {:.1} vs {:.1}",
        result.total_rapid_distance(),
        tp.total_rapid_distance()
    );
}

#[test]
fn test_link_too_far() {
    // 25mm gap — exceeds max_link_distance of 18mm
    let tp = two_pass_toolpath(25.0);
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(
        AnnotatedToolpath::new(tp.clone()),
        &params,
    ))
    .toolpath;

    // Should be unchanged (gap too large)
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "Far passes should not be linked"
    );
}

#[test]
fn test_link_first_entry_preserved() {
    // The very first plunge should never be linked (no prior cutting)
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.rapid_to(P3::new(5.0, 0.0, 10.0));
    tp.feed_to(P3::new(5.0, 0.0, -3.0), 500.0);
    tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
    tp.rapid_to(P3::new(20.0, 0.0, 10.0));

    let params = default_link_params();
    let result = without_provenance(apply_link_moves(
        AnnotatedToolpath::new(tp.clone()),
        &params,
    ))
    .toolpath;

    // First entry should not be linked — all moves preserved
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "First entry should be preserved"
    );
}

#[test]
fn test_link_different_z_preserved() {
    // Two passes at different Z levels — should NOT be linked
    let mut tp = Toolpath::new();
    // First pass at Z=-3
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
    tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
    // Retract
    tp.rapid_to(P3::new(20.0, 0.0, 10.0));
    // Second pass at Z=-6 (different depth)
    tp.rapid_to(P3::new(22.0, 0.0, 10.0));
    tp.feed_to(P3::new(22.0, 0.0, -6.0), 500.0);
    tp.feed_to(P3::new(40.0, 0.0, -6.0), 1000.0);
    tp.rapid_to(P3::new(40.0, 0.0, 10.0));

    let params = default_link_params();
    let result = without_provenance(apply_link_moves(
        AnnotatedToolpath::new(tp.clone()),
        &params,
    ))
    .toolpath;

    // Different Z levels — should not be linked
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "Different Z levels should not be linked"
    );
}

#[test]
fn test_link_reduces_rapid_distance() {
    // 4mm gap: within `bridge_corridor_is_swept`'s reach for
    // `default_link_params`'s 3mm tool_radius (a 5mm gap used to be
    // used here, but the far end of that corridor sits outside one
    // tool radius of the first pass's kerf, so the corridor-safety fix
    // correctly refuses it — this fixture is about the link mechanism
    // reducing rapids, not corridor safety, so keep the gap inside the
    // radius the mechanism is allowed to bridge).
    let tp = two_pass_toolpath(4.0);
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(
        AnnotatedToolpath::new(tp.clone()),
        &params,
    ))
    .toolpath;

    let orig_rapid = tp.total_rapid_distance();
    let linked_rapid = result.total_rapid_distance();
    assert!(
        linked_rapid < orig_rapid * 0.8,
        "Linking should significantly reduce rapids: {:.1} -> {:.1}",
        orig_rapid,
        linked_rapid
    );
}

// ── Span-aware behavior (#53) ─────────────────────────────────────────

#[test]
fn test_link_honors_depth_pass_barrier() {
    // Two adjacent passes that the link logic *would* merge by XY distance,
    // but a DepthPass boundary sits between them. Link must be skipped.
    let tp = two_pass_toolpath(2.0);
    // two_pass_toolpath layout (8 moves):
    //  0: Rapid (0,0,10)
    //  1: Linear plunge to (0,0,-3)
    //  2: Linear feed to (20,0,-3)
    //  3: Rapid (20,0,10)            ← retract
    //  4: Rapid (22,0,10)            ← reposition  ─ link window starts at 3
    //  5: Linear plunge to (22,0,-3) ← plunge
    //  6: Linear feed to (40,0,-3)
    //  7: Rapid (40+gap,0,10)
    // A barrier at index 4 means moves[4..] is a separate pass — collapsing
    // (3,4,5) into one bridge would erase the barrier. Must be skipped.
    let n = tp.moves.len();
    let spans = vec![
        Span::new(0, n, SpanKind::Operation),
        Span::new(0, 4, SpanKind::DepthPass),
        Span::new(4, n, SpanKind::DepthPass),
        Span::boundary(4, SpanKind::RapidOrderBarrier),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp.clone(), spans);
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(annotated, &params));

    assert_eq!(
        result.toolpath.moves.len(),
        tp.moves.len(),
        "Link must not collapse across DepthPass / barrier"
    );
    assert!(
        result.spans.iter().all(|s| s.kind != SpanKind::LinkBridge),
        "no LinkBridge should be inserted when blocked"
    );
    // Spans round-trip unchanged when no remap occurs.
    assert!(result.spans_valid);
    result
        .check_invariants()
        .expect("blocked link preserves invariants");
}

#[test]
fn test_link_remaps_spans_and_tags_bridge() {
    // No barriers between the two passes — the link should fire and the
    // outer Operation span / DepthPass span must shrink to match the new
    // move count, plus a LinkBridge span tags the inserted feed.
    let tp = two_pass_toolpath(2.0);
    let n_in = tp.moves.len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = AnnotatedToolpath::with_spans(tp.clone(), spans);
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(annotated, &params));

    let n_out = result.toolpath.moves.len();
    assert!(n_out < n_in, "link should have fired");

    let op = result
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .expect("Operation span survives");
    assert_eq!(op.start_move, 0);
    assert_eq!(op.end_move, n_out, "Operation span tracks new move count");

    let bridges: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LinkBridge)
        .collect();
    assert_eq!(bridges.len(), 1, "exactly one LinkBridge for the one link");
    assert!(bridges[0].end_move <= n_out);
    result
        .check_invariants()
        .expect("post-link spans pass invariants");
    assert!(result.spans_valid);
}

#[test]
fn test_link_preserves_invalid_spans_flag() {
    // If input spans are flagged invalid, we don't try to remap and we
    // don't compute barriers from them either — behavior matches the
    // legacy unconditional link.
    let tp = two_pass_toolpath(2.0);
    let mut annotated = AnnotatedToolpath::new(tp.clone());
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)]; // garbage
    let params = default_link_params();
    let result = without_provenance(apply_link_moves(annotated, &params));

    assert!(result.toolpath.moves.len() < tp.moves.len(), "link fires");
    assert!(!result.spans_valid, "invalid stays invalid");
    // The garbage span vector should be returned untouched, not remapped.
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

// --- Air-cut filter tests ---

use crate::dexel_stock::TriDexelStock;

/// The cutter these fixtures probe with: Ø6 flat, the shape the old
/// `tool_radius: f64` argument of 3.0 described. Flat is deliberate — its
/// `height_at_radius` is 0 across the envelope, so the disc query reduces
/// to "is any material under the tool above the tip" and each fixture's
/// verdict can be read off the cell layout.
fn probe_cutter() -> crate::tool::FlatEndmill {
    crate::tool::FlatEndmill::new(6.0, 25.0)
}

/// Entry safety with no surface probe — the legacy blind-leg
/// behaviour these tests pin (G-RAMPTERRAIN keeps it for callers
/// with no mesh surface).
fn no_probe(stock_top: f64) -> EntrySafety<'static> {
    EntrySafety {
        stock_top,
        surface: None,
    }
}

/// Build a stock where x < 50 has material (top_z = 5.0) and x >= 50 is
/// cleared (top_z lowered to -10.0 by simulating a cut).  The stock spans
/// x: 0..100, y: 0..100, z: -10..5 with 5mm cells.
///
/// Cleared through [`TriDexelStock::clear_above_at`], not a raw
/// `ray_subtract_above`: the latter empties the ray but leaves
/// `conservative_top` at the original stock top, a state no production
/// path can produce (every stamping route lowers the bound with the ray),
/// and one the S3 disc query would read as standing material.
fn half_cleared_stock() -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, -10.0, 5.0, 5.0);
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    for row in 0..rows {
        for col in 0..cols {
            let world_x = stock.z_grid.origin_u + col as f64 * stock.z_grid.cell_size;
            if world_x >= 50.0 {
                stock.clear_above_at(row, col, -10.0);
            }
        }
    }
    stock
}

/// Build a stock cleared at BOTH ends with a band of material left
/// standing in the middle (45 <= x < 55). Stock spans x/y 0..100,
/// z -10..5, 5 mm cells. Same clearing rule as
/// [`half_cleared_stock`], for the same reason.
fn island_stock() -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, -10.0, 5.0, 5.0);
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    for row in 0..rows {
        for col in 0..cols {
            let world_x = stock.z_grid.origin_u + col as f64 * stock.z_grid.cell_size;
            if !(45.0..55.0).contains(&world_x) {
                stock.clear_above_at(row, col, -10.0);
            }
        }
    }
    stock
}

/// Regression sentry: a cut whose ENDPOINTS are both in air but whose
/// MIDDLE crosses standing material must NOT be classified as air.
///
/// `filter_air_cuts` samples only `source_air` and `target_air`, and its
/// own doc comment claims "moves that partially contact material are
/// preserved" — which is exactly the invariant the endpoint-only test
/// breaks. Deleting such a move (or bridging over it) silently leaves an
/// island standing, and the bias scales with fragment count, so it hits
/// a fragmented rest pass hardest of all
/// (`planning/v3_workplan.md` defect P2).
#[test]
fn air_cut_spanning_an_island_is_not_air() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0));
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
    // x=10 is cleared, x=90 is cleared, but the tool ploughs straight
    // through the material standing at x≈50 on the way.
    tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0);
    tp.rapid_to(P3::new(90.0, 50.0, 10.0));

    let stock = island_stock();
    let result = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ))
    .toolpath;

    let crossing_survives = result.moves.iter().any(|m| {
        matches!(m.move_type, MoveType::Linear { .. })
            && (m.target.x - 90.0).abs() < 1e-9
            && (m.target.z - 2.0).abs() < 1e-9
    });
    assert!(
        crossing_survives,
        "the x=10→90 cut passes through material at x≈50 and must be kept; \
             endpoint-only air classification dropped it, leaving the island \
             standing. moves: {:?}",
        result
            .moves
            .iter()
            .map(|m| (m.move_type, m.target.x, m.target.z))
            .collect::<Vec<_>>()
    );
}

#[test]
fn filter_air_cuts_removes_air_moves() {
    // Toolpath cuts across the stock: left half has material, right half is air.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0)); // rapid to start above stock
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0); // plunge into material (stock top = 5)
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0); // cut in material
    tp.feed_to(P3::new(60.0, 50.0, 2.0), 1000.0); // cut into air (x>=50 is cleared)
    tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0); // still in air
    tp.rapid_to(P3::new(90.0, 50.0, 10.0)); // retract

    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ))
    .toolpath;

    // The moves at x=60 and x=90 should have been removed (both endpoints in air).
    // Specifically, the move from x=60 to x=90 is fully in air (source and target).
    // The move from x=30 to x=60 has source in material, target in air — conservative: preserved.
    // So the result should have fewer cutting moves than the original.
    let original_cutting = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .count();
    let result_cutting = result
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .count();
    assert!(
        result_cutting < original_cutting,
        "Air moves should be removed: orig_cutting={}, result_cutting={}",
        original_cutting,
        result_cutting
    );

    // The result should still contain the initial plunge and the material cuts.
    let has_material_cut = result.moves.iter().any(|m| {
        matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-6)
            && m.target.x <= 50.0
    });
    assert!(
        has_material_cut,
        "Material-region cutting moves should be preserved"
    );
}

#[test]
fn filter_air_cuts_preserves_cutting_moves() {
    // All moves are in material — nothing should be removed.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0));
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
    tp.feed_to(P3::new(20.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0);
    tp.rapid_to(P3::new(30.0, 50.0, 10.0));

    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ))
    .toolpath;

    // All cutting moves are in the left half (x < 50) where material exists
    // at top_z=5.0 and tool is at z=2.0 (below stock top). No air cuts.
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "All-material toolpath should be unchanged: result={}, orig={}",
        result.moves.len(),
        tp.moves.len()
    );
}

#[test]
fn ramp_entry_zero_angle_falls_back() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 100.0);
    let style = EntryStyle::Ramp { max_angle_deg: 0.0 };
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        style,
        50.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;
    // Should not contain NaN or infinity
    for m in &result.moves {
        assert!(m.target.x.is_finite(), "NaN in ramp with 0° angle");
        assert!(m.target.y.is_finite(), "NaN in ramp with 0° angle");
        assert!(m.target.z.is_finite(), "NaN in ramp with 0° angle");
    }
}

#[test]
fn ramp_entry_90deg_angle_falls_back() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 100.0);
    let style = EntryStyle::Ramp {
        max_angle_deg: 90.0,
    };
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        style,
        50.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;
    for m in &result.moves {
        assert!(m.target.x.is_finite(), "NaN in ramp with 90° angle");
        assert!(m.target.z.is_finite(), "NaN in ramp with 90° angle");
    }
}

/// A plunge followed by one straight cut run of `run_mm` along +X.
///
/// The rapid stops AT the ramp start height (`end.z + ENTRY_CLEARANCE`),
/// so the B1 lift-bridge pre-descent in `emit_ramp` emits nothing and the
/// entry is the fold's own output: the folded ramp, or its plunge degrade.
fn plunge_then_run(run_mm: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 10.0, -3.0 + ENTRY_CLEARANCE));
    tp.feed_to(P3::new(10.0, 10.0, -3.0), 500.0);
    tp.feed_to(P3::new(10.0 + run_mm, 10.0, -3.0), 1000.0);
    tp.rapid_to(P3::new(10.0 + run_mm, 10.0, 10.0));
    tp
}

/// R10 (2026-09-18): a ramp fold refuses a run it would lap more than
/// [`entry_descent::RAMP_FOLD_MAX_LAPS`] times and degrades to a plunge.
///
/// The Corne waterline folded a 3 degree ramp along a 2 mm run: a 2 mm
/// drop at 3 degrees is 38 mm of XY, so the fold laid 19 laps over the
/// run at every level and sawed the wall to pins. The tool radius here is
/// 0.5 mm, so `min_run_mm` is 1.0 and the 2 mm run passes the OLD gate;
/// only the lap cap can produce the plunge.
#[test]
fn ramp_fold_caps_laps_and_falls_back_to_plunge_r10() {
    use super::entry_descent::RAMP_FOLD_MAX_LAPS;

    let style = EntryStyle::Ramp { max_angle_deg: 3.0 };
    let ramp_xy_mm = ENTRY_CLEARANCE / 3.0_f64.to_radians().tan();
    let entry = |run_mm: f64| {
        without_provenance(apply_entry(
            AnnotatedToolpath::new(plunge_then_run(run_mm)),
            style,
            500.0,
            no_probe(0.0),
            // G-RAMPCONTAIN: the tool radius. 0.5 mm keeps `min_run_mm`
            // at its 1.0 floor so the 2 mm run clears the old gate.
            0.5,
        ))
        .toolpath
    };

    // The short run: 19 laps at the shipped dials, above the cap.
    let short_run = 2.0;
    assert!(
        ramp_xy_mm / short_run > RAMP_FOLD_MAX_LAPS,
        "fixture: the 2 mm run must need more than {RAMP_FOLD_MAX_LAPS} laps"
    );
    let result = entry(short_run);
    let ramps: Vec<&Move> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .collect();
    assert!(
        ramps.is_empty(),
        "R10: a fold over a 2 mm run must degrade to a plunge, got {} EntryRamp moves",
        ramps.len()
    );
    let plunges: Vec<&Move> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .collect();
    assert_eq!(plunges.len(), 1, "R10: the degrade is ONE plunge move");
    let p = plunges[0].target;
    assert!(
        (p.x - 10.0).abs() < 1e-9 && (p.y - 10.0).abs() < 1e-9 && (p.z + 3.0).abs() < 1e-9,
        "R10: the plunge lands on the entry target, got {p:?}"
    );
    assert_eq!(
        result.moves.len(),
        4,
        "R10: the plunge replaces the plunge one for one"
    );

    // The control: a 40 mm run takes the fold, and the fold laps it at
    // most `RAMP_FOLD_MAX_LAPS` times.
    let long_run = 40.0;
    let result = entry(long_run);
    let first_ramp = result
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::EntryRamp)
        .expect("control: a 40 mm run must still fold (EntryRamp moves present)");
    assert!(first_ramp > 0, "control: the ramp follows the rapid");
    let mut walked = 0.0;
    let mut prev = result.moves[first_ramp - 1].target;
    let mut last_ramp = prev;
    for m in result.moves.iter().skip(first_ramp) {
        if m.intent != MoveIntent::EntryRamp {
            break;
        }
        let dx = m.target.x - prev.x;
        let dy = m.target.y - prev.y;
        walked += (dx * dx + dy * dy).sqrt();
        assert!(
            (10.0 - 1e-9..=10.0 + long_run + 1e-9).contains(&m.target.x),
            "control: the fold stays on the run, got x={}",
            m.target.x
        );
        prev = m.target;
        last_ramp = m.target;
    }
    assert!(
        (walked - ramp_xy_mm).abs() < 1e-6,
        "control: the fold walks the whole ramp, {walked:.3} mm of {ramp_xy_mm:.3}"
    );
    assert!(
        walked / long_run <= RAMP_FOLD_MAX_LAPS,
        "control: {:.2} laps over the 40 mm run exceeds the cap",
        walked / long_run
    );
    assert!(
        (last_ramp.x - 10.0).abs() < 1e-9 && (last_ramp.z + 3.0).abs() < 1e-9,
        "control: the fold returns to the entry target, got {last_ramp:?}"
    );
}

#[test]
fn helix_entry_zero_radius_falls_back() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 100.0);
    let style = EntryStyle::Helix {
        radius: 0.0,
        pitch: 2.0,
    };
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        style,
        50.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;
    for m in &result.moves {
        assert!(m.target.x.is_finite(), "NaN in helix with 0 radius");
        assert!(m.target.z.is_finite(), "NaN in helix with 0 radius");
    }
}

#[test]
fn helix_entry_negative_radius_falls_back() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 100.0);
    let style = EntryStyle::Helix {
        radius: -1.0,
        pitch: 2.0,
    };
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp.clone()),
        style,
        50.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ))
    .toolpath;
    for m in &result.moves {
        assert!(m.target.x.is_finite(), "NaN in helix with negative radius");
    }
}

#[test]
fn filter_air_cuts_conservative_partial() {
    // Move starts in air, ends in material — should be preserved (conservative).
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(70.0, 50.0, 10.0)); // rapid to air region
    tp.feed_to(P3::new(70.0, 50.0, 2.0), 500.0); // plunge in air
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0); // move from air into material
    tp.rapid_to(P3::new(30.0, 50.0, 10.0));

    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ))
    .toolpath;

    // The move from x=70 to x=30 has source in air but target in material.
    // Conservative rule: it should be preserved because the target has material.
    let has_crossing_cut = result.moves.iter().any(|m| {
        matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-6)
            && (m.target.x - 30.0).abs() < 0.01
    });
    assert!(
        has_crossing_cut,
        "Partial air-to-material move should be preserved (conservative)"
    );
}

// ── Span-aware behavior: apply_entry (#50) ──────────────────────────

#[test]
fn apply_entry_remaps_spans_through_transform() {
    let tp = simple_plunge_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![
        Span::new(0, n_in, SpanKind::Operation),
        Span::new(0, 2, SpanKind::DepthPass),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_entry(
        annotated,
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ));
    result
        .check_invariants()
        .expect("post-entry spans pass invariants");
    let n_out = result.toolpath.moves.len();
    let op = result
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .expect("Operation span survives");
    assert_eq!(op.start_move, 0);
    assert_eq!(op.end_move, n_out);
    assert!(result.spans_valid);
}

#[test]
fn apply_entry_tags_new_moves_with_correct_kind() {
    let tp = simple_plunge_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_entry(
        annotated,
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ));
    let entries: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::Entry)
        .collect();
    assert!(
        !entries.is_empty(),
        "ramp entry should tag at least one Entry span"
    );
    for e in &entries {
        assert!(e.end_move <= result.toolpath.moves.len());
        assert!(e.move_count() > 0, "Entry spans should cover ≥ 1 move");
    }
}

#[test]
fn apply_entry_preserves_invalid_flag() {
    let tp = simple_plunge_toolpath();
    let mut annotated = AnnotatedToolpath::new(tp.clone());
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)]; // garbage
    let result = without_provenance(apply_entry(
        annotated,
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        500.0,
        no_probe(0.0),
        // G-RAMPCONTAIN: the tool radius. It only sets the floor under
        // which a ramp fold degrades to a plunge.
        3.0,
    ));
    assert!(!result.spans_valid);
    // Garbage span returned untouched.
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

// ── Span-aware behavior: apply_dogbones (#51) ───────────────────────

#[test]
fn apply_dogbones_remaps_spans_through_transform() {
    let tp = square_profile_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![
        Span::new(0, n_in, SpanKind::Operation),
        Span::new(0, 3, SpanKind::DepthPass),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_dogbones(annotated, 3.0, 170.0));
    result
        .check_invariants()
        .expect("post-dogbone spans pass invariants");
    let n_out = result.toolpath.moves.len();
    let op = result
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .expect("Operation span survives");
    assert_eq!(op.start_move, 0);
    assert_eq!(op.end_move, n_out);
    assert!(result.spans_valid);
}

#[test]
fn apply_dogbones_tags_new_moves_with_correct_kind() {
    let tp = square_profile_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_dogbones(annotated, 3.0, 170.0));
    let dogbones: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::DressupArtifact)
        .collect();
    assert!(
        !dogbones.is_empty(),
        "square profile should tag at least one DressupArtifact span"
    );
    for d in &dogbones {
        assert_eq!(d.label, "dogbone");
        assert!(d.end_move <= result.toolpath.moves.len());
    }
}

#[test]
fn apply_dogbones_preserves_invalid_flag() {
    let tp = square_profile_toolpath();
    let mut annotated = AnnotatedToolpath::new(tp);
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)];
    let result = without_provenance(apply_dogbones(annotated, 3.0, 170.0));
    assert!(!result.spans_valid);
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

// ── Span-aware behavior: apply_lead_in_out (#52) ────────────────────

#[test]
fn apply_lead_in_out_remaps_spans_through_transform() {
    let tp = simple_plunge_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![
        Span::new(0, n_in, SpanKind::Operation),
        Span::new(0, 2, SpanKind::DepthPass),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_lead_in_out(annotated, 2.0, None, None, None, None));
    result
        .check_invariants()
        .expect("post-lead spans pass invariants");
    let n_out = result.toolpath.moves.len();
    let op = result
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .expect("Operation span survives");
    assert_eq!(op.start_move, 0);
    assert_eq!(op.end_move, n_out);
    assert!(result.spans_valid);
}

#[test]
fn apply_lead_in_out_tags_new_moves_with_correct_kind() {
    let tp = simple_plunge_toolpath();
    let n_in = tp.moves.len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let result = without_provenance(apply_lead_in_out(annotated, 2.0, None, None, None, None));
    let entries: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::Entry)
        .collect();
    let leadouts: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LeadOut)
        .collect();
    assert!(!entries.is_empty(), "lead-in should tag Entry span(s)");
    assert!(!leadouts.is_empty(), "lead-out should tag LeadOut span(s)");
    for e in &entries {
        assert!(e.end_move <= result.toolpath.moves.len());
    }
    for l in &leadouts {
        assert!(l.end_move <= result.toolpath.moves.len());
    }
}

#[test]
fn apply_lead_in_out_preserves_invalid_flag() {
    let tp = simple_plunge_toolpath();
    let mut annotated = AnnotatedToolpath::new(tp);
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)];
    let result = without_provenance(apply_lead_in_out(annotated, 2.0, None, None, None, None));
    assert!(!result.spans_valid);
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

#[test]
fn air_bridge_policy_vetoes_bridges_longer_than_the_air_they_skip() {
    use crate::dexel_stock::StockCutDirection;
    use crate::geo::BoundingBox3;
    use crate::tool::FlatEndmill;

    // One long cut with a SHORT air gap in the middle, and safe_z far
    // above: the historical `Always` policy climbs to safe_z and back
    // — ~20mm of travel — to skip ~2mm of air.
    let stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, -10.0),
            max: P3::new(60.0, 20.0, 0.0),
        },
        0.5,
    );
    // Carve a trench so a short stretch mid-pass reads as air. The carving
    // tool (Ø10) is wider than the probing tool (Ø3) by more than the
    // probe's envelope: S3 judges a sample against the whole cutter, so a
    // trench of exactly the probe's own width would put its uncut walls
    // inside the query's disc and no sample would read air at all. That
    // over-read is real and deliberate (S2's kerf-rim note); this test is
    // about the bridge-cost policy, so it stays clear of it.
    let mut carved = stock.clone();
    let mut cut = Toolpath::new();
    cut.rapid_to(P3::new(29.0, 10.0, 10.0));
    cut.feed_to(P3::new(29.0, 10.0, -5.0), 500.0);
    cut.feed_to(P3::new(31.0, 10.0, -5.0), 500.0);
    carved.simulate_toolpath(
        &cut,
        &FlatEndmill::new(10.0, 25.0),
        StockCutDirection::FromTop,
    );
    let probe = FlatEndmill::new(3.0, 25.0);

    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 10.0, 10.0));
    tp.feed_to(P3::new(5.0, 10.0, -3.0), 500.0);
    for x in [10.0_f64, 20.0, 28.0, 30.0, 32.0, 40.0, 50.0] {
        tp.feed_to(P3::new(x, 10.0, -3.0), 1000.0);
    }
    tp.rapid_to(P3::new(50.0, 10.0, 10.0));

    let always = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &carved,
        &probe,
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ))
    .toolpath;
    let costed = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &carved,
        &probe,
        10.0,
        0.1,
        AirBridgePolicy::ShorterThanAirPath,
    ))
    .toolpath;

    let rapid = |t: &Toolpath| t.total_rapid_distance();
    assert!(
        rapid(&costed) < rapid(&always),
        "vetoing a bridge that is longer than the air it skips must cut \
             rapid travel: Always={:.1}mm over {} moves vs Costed={:.1}mm \
             over {} moves",
        rapid(&always),
        always.moves.len(),
        rapid(&costed),
        costed.moves.len(),
    );
    // And the vetoed air moves survive as cutting moves.
    assert!(
        costed.moves.len() >= tp.moves.len(),
        "a vetoed run is emitted verbatim, so no cutting move is lost"
    );
}

#[test]
fn air_bridge_policy_always_is_the_untouched_default() {
    assert_eq!(
        AirBridgePolicy::default(),
        AirBridgePolicy::Always,
        "the cost-aware policy is a shipped-behaviour change and must be \
             opt-in until the wanaka A/B justifies flipping it"
    );
    assert_eq!(
        crate::compute::config::DressupConfig::default().air_bridge_policy,
        AirBridgePolicy::Always
    );
}

// ── Span-aware behavior: filter_air_cuts (#56) ──────────────────────

#[test]
fn filter_air_cuts_remaps_spans_through_transform() {
    // Toolpath that mixes material and air moves, so filter actually drops
    // and inserts (matches filter_air_cuts_removes_air_moves fixture).
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0));
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(60.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0);
    tp.rapid_to(P3::new(90.0, 50.0, 10.0));
    let n_in = tp.moves.len();
    let spans = vec![
        Span::new(0, n_in, SpanKind::Operation),
        Span::new(0, 3, SpanKind::DepthPass),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        annotated,
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ));
    result
        .check_invariants()
        .expect("post-filter spans pass invariants");
    let n_out = result.toolpath.moves.len();
    let op = result
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .expect("Operation span survives");
    assert_eq!(op.start_move, 0);
    assert_eq!(op.end_move, n_out);
    assert!(result.spans_valid);
}

#[test]
fn filter_air_cuts_tags_new_moves_with_correct_kind() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0));
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(60.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0);
    tp.rapid_to(P3::new(90.0, 50.0, 10.0));
    let n_in = tp.moves.len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);
    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        annotated,
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ));
    let bridges: Vec<&Span> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LinkBridge)
        .collect();
    assert!(
        !bridges.is_empty(),
        "filter should insert at least one LinkBridge for the dropped air run"
    );
    for b in &bridges {
        assert!(b.end_move <= result.toolpath.moves.len());
        assert!(b.move_count() > 0);
    }
}

#[test]
fn filter_air_cuts_preserves_invalid_flag() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 50.0, 10.0));
    tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
    tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(60.0, 50.0, 2.0), 1000.0);
    tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0);
    tp.rapid_to(P3::new(90.0, 50.0, 10.0));
    let mut annotated = AnnotatedToolpath::new(tp);
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)];
    let stock = half_cleared_stock();
    let result = without_provenance(filter_air_cuts(
        annotated,
        &stock,
        &probe_cutter(),
        10.0,
        0.1,
        AirBridgePolicy::Always,
    ));
    assert!(!result.spans_valid);
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}
