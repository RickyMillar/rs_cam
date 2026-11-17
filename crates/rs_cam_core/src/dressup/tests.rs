//! Unit tests for the dressup transforms. Moved out of `dressup/mod.rs`
//! by P4; the module body is unchanged.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::redundant_clone,
    reason = "test code: a panic is how a test reports a failure; test code accepts the style lint redundant_clone"
)]

use super::*;
use crate::geo::P2;
use crate::toolpath::{ClosedPassMoves, MoveIntent, RingMaterial, SidedToolpath};

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

/// A closed 40 mm square pass at Z -3, CCW, its plunge at (30, 10) in the
/// middle of the bottom edge, with
/// the stated `material` side (P6.0c) in `produced`, as the Layered runner
/// gives it to the lead dressup.
fn sided_square_pass(material: RingMaterial, spans: Vec<Span>) -> AnnotatedToolpath {
    let mut tp = SidedToolpath::new();
    let ring = [
        (30.0, 10.0),
        (50.0, 10.0),
        (50.0, 50.0),
        (10.0, 50.0),
        (10.0, 10.0),
    ]
    .map(|(x, y)| P3::new(x, y, -3.0));
    tp.emit_closed_pass(
        &ring,
        ClosedPassMoves {
            safe_z: 10.0,
            feed: 1000.0,
            plunge: 500.0,
            body_intent: MoveIntent::FinishingCut,
        },
        material,
    );
    let mut annotated = AnnotatedToolpath::with_spans(tp.toolpath, spans);
    annotated.produced.closed_passes = tp.closed_passes;
    annotated
}

/// The square pass of [`sided_square_pass`] with no span.
fn square_with_lead_side(material: RingMaterial) -> AnnotatedToolpath {
    sided_square_pass(material, Vec::new())
}

/// The lead dressup at radius 2, no probe, every pass.
fn leads(annotated: AnnotatedToolpath) -> AnnotatedToolpath {
    without_provenance(apply_lead_in_out(
        annotated,
        2.0,
        None,
        None,
        None,
        None,
        crate::dressup::LeadScope::EveryPass,
    ))
}

/// The XY of every lead move (`LeadIn` or `LeadOut` feed).
fn lead_points(tp: &Toolpath) -> Vec<P2> {
    tp.moves
        .iter()
        .filter(|m| {
            m.move_type != MoveType::Rapid
                && matches!(m.intent, MoveIntent::LeadIn | MoveIntent::LeadOut)
        })
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect()
}

/// True when `p` lies strictly outside the 40 mm square of
/// [`sided_square_pass`].
fn outside_square(p: P2) -> bool {
    p.x < 10.0 - 1e-9 || p.x > 50.0 + 1e-9 || p.y < 10.0 - 1e-9 || p.y > 50.0 + 1e-9
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
            // The fallback angle of every helix before P29-8.
            fallback_ramp_angle_deg: 3.0,
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
            // The fallback angle of every helix before P29-8.
            fallback_ramp_angle_deg: 3.0,
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
            // The fallback angle of every helix before P29-8.
            fallback_ramp_angle_deg: 3.0,
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
    let result = apply_tabs(tp.clone(), &tabs, -5.0, -5.0, None);

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

/// P29-8e (lead ruling Q2): a descent into stock never runs at the cut
/// feed. The descent after each tab runs at the plunge feed of the entry
/// rule; with no rule (`None`) it keeps the cut feed of the interrupted move.
#[test]
fn a_tab_descent_reads_the_plunge_feed_of_the_entry_rule_p29_8e() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(4, 5.0, 3.0);
    let plunge = crate::feeds::entry_rule::EntryPlungeFeed::of_op_plunge_rate(Some(400.0)).ok();
    let descents = |t: &Toolpath| -> Vec<f64> {
        t.moves
            .windows(2)
            .filter(|w| {
                w[1].intent == crate::toolpath::MoveIntent::EntryPlunge
                    && w[1].target.z < w[0].target.z
                    && (w[1].target.x - w[0].target.x).abs() < 1e-9
                    && (w[1].target.y - w[0].target.y).abs() < 1e-9
                    && (w[1].target.z - -5.0).abs() < 1e-9
                    && (w[0].target.z - -2.0).abs() < 1e-9
            })
            .filter_map(|w| match w[1].move_type {
                MoveType::Linear { feed_rate } => Some(feed_rate),
                MoveType::Rapid | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => None,
            })
            .collect()
    };
    let with_rule = descents(&apply_tabs(tp.clone(), &tabs, -5.0, -5.0, plunge));
    assert!(!with_rule.is_empty(), "non-vacuity: the tabs have descents");
    assert!(
        with_rule.iter().all(|f| (*f - 400.0).abs() < 1e-9),
        "{with_rule:?}"
    );
    let no_rule = descents(&apply_tabs(tp, &tabs, -5.0, -5.0, None));
    assert!(
        no_rule.iter().all(|f| (*f - 1000.0).abs() < 1e-9),
        "{no_rule:?}"
    );
}

#[test]
fn test_tabs_preserve_non_tab_moves() {
    let tp = profile_toolpath_for_tabs();
    let tabs = even_tabs(2, 3.0, 2.0);
    let result = apply_tabs(tp.clone(), &tabs, -5.0, -5.0, None);

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
    let result = apply_tabs(tp.clone(), &[], -5.0, -5.0, None);
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
    let result = apply_tabs(tp.clone(), &tabs, -5.0, -5.0, None);

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
    let before = square_with_lead_side(RingMaterial::Inside);
    let n_in = before.toolpath.moves.len();
    let result = leads(before).toolpath;

    // Should have more moves than original (arc segments added)
    assert!(
        result.moves.len() > n_in,
        "Lead-in should add arc moves: {} vs {}",
        result.moves.len(),
        n_in
    );
}

#[test]
fn test_lead_in_reaches_cut_point() {
    let result = leads(square_with_lead_side(RingMaterial::Inside)).toolpath;

    // The first cut point at x=30, y=10, z=-3 should still be reachable
    let has_first_cut = result.moves.iter().any(|m| {
        (m.target.z - (-3.0)).abs() < 0.01
            && (m.target.x - 30.0).abs() < 3.0
            && (m.target.y - 10.0).abs() < 3.0
    });
    assert!(
        has_first_cut,
        "Lead-in should arrive near the first cut point"
    );
}

#[test]
fn test_lead_in_preserves_rapids() {
    let result = leads(square_with_lead_side(RingMaterial::Inside)).toolpath;

    // Should still have a rapid move
    let has_rapid = result.moves.iter().any(|m| m.move_type == MoveType::Rapid);
    assert!(has_rapid, "Lead-in should preserve rapid moves");
}

/// P6.0c: the material inside the ring (a part) puts every lead sample
/// outside it; the material outside (a pocket wall) puts every sample
/// inside it. The square starts in the middle of an edge, so the air
/// check keeps every lead.
#[test]
fn a_lead_turns_away_from_the_stated_material_p6_0c() {
    let outside = lead_points(&leads(square_with_lead_side(RingMaterial::Inside)).toolpath);
    assert!(!outside.is_empty(), "the part pass keeps its lead");
    assert!(
        outside.iter().all(|p| outside_square(*p) || on_square(*p)),
        "material inside: every lead sample is outside the ring: {outside:?}"
    );
    let inside = lead_points(&leads(square_with_lead_side(RingMaterial::Outside)).toolpath);
    assert!(!inside.is_empty(), "the pocket pass keeps its lead");
    assert!(
        inside.iter().all(|p| !outside_square(*p)),
        "material outside: every lead sample is inside the ring: {inside:?}"
    );
}

/// True when `p` lies on the boundary of the square of [`sided_square_pass`].
fn on_square(p: P2) -> bool {
    let on = |v: f64, at: f64| (v - at).abs() < 1e-9;
    on(p.x, 10.0) || on(p.x, 50.0) || on(p.y, 10.0) || on(p.y, 50.0)
}

/// P6.0c: a pass on the line (material on both sides) keeps its plunge
/// and retract.
#[test]
fn a_pass_on_the_line_gets_no_lead_p6_0c() {
    let before = square_with_lead_side(RingMaterial::BothSides);
    let n_in = before.toolpath.moves.len();
    let result = leads(before).toolpath;
    assert_eq!(result.moves.len(), n_in, "no lead move is added");
    assert!(lead_points(&result).is_empty());
}

/// P6.0c: with no stated side and no surface probe, the dressup does not
/// guess a side: the pass keeps its plunge and retract.
#[test]
fn a_pass_with_no_stated_side_gets_no_lead_without_a_probe_p6_0c() {
    let tp = simple_plunge_toolpath();
    let n_in = tp.moves.len();
    let result = leads(AnnotatedToolpath::new(tp)).toolpath;
    assert_eq!(result.moves.len(), n_in, "no lead move is added");
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
    let result = apply_tabs(tp.clone(), &tabs, -5.0, -5.0, None);

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

        let result = apply_tabs(tp, &tabs, -5.0, -5.0, None);

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
        // The finishing cap: these tests pin the folded ramp as a
        // finishing contour sees it.
        fold_lap_cap: Some(super::entry_descent::RAMP_FOLD_MAX_LAPS),
        own_stock: None,
        ramp_feed: None,
        stock_top_measured: false,
        contact_clearance: 0.5,
        contact_top: None,
        core_top: None,
        helix_floor_lap: false,
        helix_containment: None,
        core: None,
        chord_lift: None,
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
        air_timing(&generic_link(), 1000.0),
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
        air_timing(&generic_link(), 1000.0),
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

/// A ramp or helix entry leg entirely above the stock stays fed; a plain
/// cut along the same leg still becomes air (G-RESTRES).
#[test]
fn filter_air_cuts_keeps_ramp_and_helix_entries_fed() {
    let stock = half_cleared_stock(); // top Z 5
    let leg = |intent: MoveIntent| {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(20.0, 50.0, 8.0));
        tp.feed_to_with_intent(P3::new(40.0, 50.0, 6.0), 300.0, intent);
        tp.rapid_to(P3::new(40.0, 50.0, 10.0));
        without_provenance(filter_air_cuts(
            AnnotatedToolpath::new(tp),
            &stock,
            &probe_cutter(),
            10.0,
            0.1,
            air_timing(&generic_link(), 1000.0),
        ))
        .toolpath
    };
    let fed_leg = |tp: &Toolpath| {
        tp.moves.iter().any(|m| {
            matches!(m.move_type, MoveType::Linear { .. })
                && (m.target.x - 40.0).abs() < 1e-9
                && (m.target.z - 6.0).abs() < 1e-9
        })
    };
    for intent in [MoveIntent::EntryRamp, MoveIntent::EntryHelix] {
        assert!(fed_leg(&leg(intent)), "{intent:?} leg must stay fed");
    }
    assert!(
        !fed_leg(&leg(MoveIntent::ClearingCut)),
        "a plain cut on the same all-air leg must still become air"
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
        air_timing(&generic_link(), 1000.0),
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

/// Operator ruling 2026-09-18: with no lap cap (a roughing role) the same
/// 2 mm run still takes the fold. The cap is the finishing roles' rule,
/// not the fold's.
#[test]
fn a_rough_keeps_folding_over_a_short_run_r10() {
    let style = EntryStyle::Ramp { max_angle_deg: 3.0 };
    let safety = EntrySafety {
        stock_top: 0.0,
        surface: None,
        fold_lap_cap: None,
        own_stock: None,
        ramp_feed: None,
        stock_top_measured: false,
        contact_clearance: 0.5,
        contact_top: None,
        core_top: None,
        helix_floor_lap: false,
        helix_containment: None,
        core: None,
        chord_lift: None,
    };
    let result = without_provenance(apply_entry(
        AnnotatedToolpath::new(plunge_then_run(2.0)),
        style,
        500.0,
        safety,
        0.5,
    ))
    .toolpath;
    let ramps = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .count();
    assert!(
        ramps > 0,
        "R10: a rough (no cap) folds its ramp over the 2 mm run, got 0 EntryRamp moves"
    );
    let plunges = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count();
    assert_eq!(plunges, 0, "R10: no plunge degrade without the cap");
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
        ramp_xy_mm / short_run > f64::from(RAMP_FOLD_MAX_LAPS),
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
        walked / long_run <= f64::from(RAMP_FOLD_MAX_LAPS),
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
        // The fallback angle of every helix before P29-8.
        fallback_ramp_angle_deg: 3.0,
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
        // The fallback angle of every helix before P29-8.
        fallback_ramp_angle_deg: 3.0,
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
        air_timing(&generic_link(), 1000.0),
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
    let n_in = square_with_lead_side(RingMaterial::Inside)
        .toolpath
        .moves
        .len();
    let spans = vec![
        Span::new(0, n_in, SpanKind::Operation),
        Span::new(0, 2, SpanKind::DepthPass),
    ];
    let annotated = sided_square_pass(RingMaterial::Inside, spans);
    let result = without_provenance(apply_lead_in_out(
        annotated,
        2.0,
        None,
        None,
        None,
        None,
        crate::dressup::LeadScope::EveryPass,
    ));
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
    let n_in = square_with_lead_side(RingMaterial::Inside)
        .toolpath
        .moves
        .len();
    let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
    let annotated = sided_square_pass(RingMaterial::Inside, spans);
    let result = without_provenance(apply_lead_in_out(
        annotated,
        2.0,
        None,
        None,
        None,
        None,
        crate::dressup::LeadScope::EveryPass,
    ));
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
    let mut annotated = square_with_lead_side(RingMaterial::Inside);
    annotated.spans_valid = false;
    annotated.spans = vec![Span::new(0, 1, SpanKind::Operation)];
    let result = without_provenance(apply_lead_in_out(
        annotated,
        2.0,
        None,
        None,
        None,
        None,
        crate::dressup::LeadScope::EveryPass,
    ));
    assert!(!result.spans_valid);
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

/// The machine envelope the air-cut tests time a bridge with: the generic
/// router, which `MachineProfile::default()` reads.
fn generic_link() -> crate::machine::kinematics::LinkKinematics {
    crate::machine::MachineProfile::default().link_kinematics()
}

/// The air-cut timing of a test: the op's feed `feed` (mm/min) on `link`.
fn air_timing(link: &crate::machine::kinematics::LinkKinematics, feed: f64) -> AirBridgeTiming<'_> {
    AirBridgeTiming {
        feed_rate_mm_min: feed,
        link,
    }
}

/// A stock 120 mm long with a trench across it from `x0` to `x1`, carved
/// by a Ø10 end mill to `z = -5`, and the Ø3 probe. The carving tool is
/// wider than the probe by more than the probe's envelope: S3 judges a
/// sample against the whole cutter, so a trench of the probe's own width
/// would put its uncut walls inside the query's disc.
fn trench_stock(x0: f64, x1: f64) -> TriDexelStock {
    use crate::dexel_stock::StockCutDirection;
    use crate::geo::BoundingBox3;
    use crate::tool::FlatEndmill;
    let mut stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, -10.0),
            max: P3::new(120.0, 20.0, 0.0),
        },
        0.5,
    );
    let mut cut = Toolpath::new();
    cut.rapid_to(P3::new(x0, 10.0, 10.0));
    cut.feed_to(P3::new(x0, 10.0, -5.0), 500.0);
    cut.feed_to(P3::new(x1, 10.0, -5.0), 500.0);
    stock.simulate_toolpath(
        &cut,
        &FlatEndmill::new(10.0, 25.0),
        StockCutDirection::FromTop,
    );
    stock
}

/// One pass at `z = -3` from `x = 5` to `x = 115` along `y = 10`, with a
/// point every 2 mm, over a trench: a stretch of it is air.
fn pass_over_trench() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 10.0, 10.0));
    tp.feed_to(P3::new(5.0, 10.0, -3.0), 500.0);
    let mut x = 7.0_f64;
    while x <= 115.0 + 1e-9 {
        tp.feed_to(P3::new(x, 10.0, -3.0), 1000.0);
        x += 2.0;
    }
    tp.rapid_to(P3::new(115.0, 10.0, 10.0));
    tp
}

/// The fed moves of `tp` that end strictly inside `(x0, x1)` at `z = -3`.
fn fed_inside(tp: &Toolpath, x0: f64, x1: f64) -> usize {
    tp.moves
        .iter()
        .filter(|m| {
            matches!(m.move_type, MoveType::Linear { .. })
                && m.target.x > x0
                && m.target.x < x1
                && (m.target.z + 3.0).abs() < 1e-9
        })
        .count()
}

/// Operator ruling 2026-10-15: an air stretch whose feed is faster than a
/// retract bridge is fed through. A 4 mm trench at 1000 mm/min takes about
/// a quarter of a second to feed; a bridge to `safe_z` 13 mm up and back
/// takes longer. Red with a bridge on every air run (the old `Always`).
#[test]
fn an_air_stretch_shorter_than_the_break_even_is_fed() {
    let stock = trench_stock(58.0, 62.0);
    let probe = crate::tool::FlatEndmill::new(3.0, 25.0);
    let link = generic_link();
    let tp = pass_over_trench();
    let out = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe,
        10.0,
        0.1,
        air_timing(&link, 1000.0),
    ))
    .toolpath;
    // Non-vacuity: the stretch over the trench reads air, so a bridge was a
    // choice, and the time rule must say the feed is faster.
    let air = |x: f64| super::air_cut::sample_is_air_for_tool(&stock, &probe, x, 10.0, -3.0, 0.1);
    assert!(air(60.0), "the trench centre reads air for the probe");
    let path: Vec<P3> = (0..=2)
        .map(|k| P3::new(59.0 + 2.0 * k as f64, 10.0, -3.0))
        .collect();
    assert!(
        !crate::machine::kinematics::retract_saves_time(
            P3::new(57.0, 10.0, -3.0),
            &path,
            1000.0,
            10.0,
            crate::machine::kinematics::RetractDescent::Rapid,
            &link,
        ),
        "fixture check: the feed through the trench is the faster arm"
    );
    assert_eq!(
        out.moves.len(),
        tp.moves.len(),
        "the short air stretch is fed through: no move is dropped or added"
    );
    assert!(
        out.moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .all(|m| (m.target.z - 10.0).abs() < 1e-9),
        "no bridge descends into the pass"
    );
}

/// Operator ruling 2026-10-15: an air stretch whose feed is slower than a
/// retract bridge is bridged. A 60 mm trench at 250 mm/min takes about
/// 14 s to feed; a bridge takes a few seconds.
#[test]
fn an_air_stretch_longer_than_the_break_even_is_bridged() {
    let stock = trench_stock(30.0, 90.0);
    let probe = crate::tool::FlatEndmill::new(3.0, 25.0);
    let link = generic_link();
    let tp = pass_over_trench();
    let out = without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        &stock,
        &probe,
        10.0,
        0.1,
        air_timing(&link, 250.0),
    ))
    .toolpath;
    assert!(
        fed_inside(&tp, 40.0, 80.0) > 0,
        "fixture check: the pass feeds over the trench"
    );
    assert_eq!(
        fed_inside(&out, 40.0, 80.0),
        0,
        "the long air stretch is bridged: no feed over the trench survives"
    );
    let bridge_down = out.moves.iter().any(|m| {
        m.move_type == MoveType::Rapid && m.intent == MoveIntent::Linking && m.target.z < 0.0
    });
    assert!(
        bridge_down,
        "the bridge descends by rapid to the point where the cut resumes"
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
        air_timing(&generic_link(), 1000.0),
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
        air_timing(&generic_link(), 1000.0),
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
        air_timing(&generic_link(), 1000.0),
    ));
    assert!(!result.spans_valid);
    assert_eq!(result.spans, vec![Span::new(0, 1, SpanKind::Operation)]);
}

// ── Ramp feed and entry clearance (operator rulings 2026-09-25) ────────

/// Feed of every `EntryHelix` / `EntryRamp` move, and of every other fed
/// move, in `tp`.
fn entry_feeds(tp: &Toolpath) -> (Vec<f64>, Vec<f64>) {
    let mut ramp = Vec::new();
    let mut other = Vec::new();
    for m in &tp.moves {
        if let MoveType::Linear { feed_rate } = m.move_type {
            if matches!(m.intent, MoveIntent::EntryHelix | MoveIntent::EntryRamp) {
                ramp.push(feed_rate);
            } else {
                other.push(feed_rate);
            }
        }
    }
    (ramp, other)
}

/// A helix and a ramp take their descent through material at the
/// operation's `ramp_feed_rate` when it is set, and at the feed they are
/// handed (the plunge feed) when it is not. The straight feed through air
/// above the material keeps the handed feed.
#[test]
fn helix_and_ramp_descend_at_the_ramp_feed_when_set() {
    const PLUNGE: f64 = 300.0;
    const RAMP: f64 = 2400.0;
    let start = P3::new(10.0, 10.0, 20.0);
    let end = P3::new(10.0, 10.0, 0.0);
    for ramp_feed in [None, Some(RAMP)] {
        let safety = EntrySafety {
            ramp_feed,
            ..no_probe(8.0)
        };
        let mut helix = Toolpath::new();
        helix.rapid_to(start);
        let _ =
            super::entry_descent::emit_helix(&mut helix, &start, &end, 2.0, 1.0, PLUNGE, &safety);
        let mut ramp = Toolpath::new();
        ramp.rapid_to(start);
        let _ = super::entry_descent::emit_ramp(
            &mut ramp,
            &start,
            &end,
            super::entry_descent::RampLegs::along((1.0, 0.0)),
            3.0,
            PLUNGE,
            &safety,
            None,
        );
        let want = ramp_feed.unwrap_or(PLUNGE);
        for (name, tp) in [("helix", &helix), ("ramp", &ramp)] {
            let (entry, other) = entry_feeds(tp);
            assert!(!entry.is_empty(), "{name}: no entry moves");
            assert!(
                entry.iter().all(|f| (f - want).abs() < 1e-9),
                "{name}: entry feeds {entry:?}, want {want}"
            );
            assert!(
                other.iter().all(|f| (f - PLUNGE).abs() < 1e-9),
                "{name}: the straight air feed must keep the plunge feed: {other:?}"
            );
        }
    }
}

/// The helix and the ramp start at the material top plus the entry
/// clearance (`entry_clearance_mm`); every move above that is straight.
#[test]
fn helix_and_ramp_start_at_the_entry_clearance() {
    let start = P3::new(10.0, 10.0, 20.0);
    let end = P3::new(10.0, 10.0, 0.0);
    let top = 8.0;
    for clearance in [0.2, 0.5, 1.5] {
        let safety = EntrySafety {
            contact_clearance: clearance,
            ..no_probe(top)
        };
        let mut helix = Toolpath::new();
        helix.rapid_to(start);
        let _ =
            super::entry_descent::emit_helix(&mut helix, &start, &end, 2.0, 1.0, 300.0, &safety);
        let mut ramp = Toolpath::new();
        ramp.rapid_to(start);
        let _ = super::entry_descent::emit_ramp(
            &mut ramp,
            &start,
            &end,
            super::entry_descent::RampLegs::along((1.0, 0.0)),
            3.0,
            300.0,
            &safety,
            None,
        );
        for (name, tp) in [("helix", &helix), ("ramp", &ramp)] {
            let first = tp
                .moves
                .iter()
                .position(|m| matches!(m.intent, MoveIntent::EntryHelix | MoveIntent::EntryRamp))
                .expect("an entry move");
            let from = tp.moves[first - 1].target;
            assert!(
                (from.z - (top + clearance)).abs() < 1e-9,
                "{name}, clearance {clearance}: the entry starts at z {:.3}, want {:.3}",
                from.z,
                top + clearance
            );
            for w in tp.moves[..first].windows(2) {
                let (a, b) = (w[0].target, w[1].target);
                assert!(
                    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9,
                    "{name}: a move above the entry is not straight down"
                );
            }
        }
    }
}

/// G-CENTRECUT: under the limits of a non-centre-cutting tool (core 8 mm,
/// protrusion 1 mm: 7.125 deg) the ramp and the helix descend no steeper
/// than the limit angle, and a plunge fallback deeper than the protrusion
/// returns a refusal.
#[test]
fn a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut() {
    let core = crate::tool::body::CoreZone::from_geometry(
        crate::compute::tool_config::NonCentreCuttingGeometry {
            centre_core_diameter_mm: 8.0,
            blade_protrusion_mm: 1.0,
        },
    );
    let slope = core.max_descent_slope();
    let start = P3::new(10.0, 10.0, 20.0);
    let end = P3::new(10.0, 10.0, 0.0);
    let safety = EntrySafety {
        core: Some(core),
        ..no_probe(8.0)
    };
    let mut helix = Toolpath::new();
    helix.rapid_to(start);
    let helix_refusal =
        super::entry_descent::emit_helix(&mut helix, &start, &end, 2.0, 3.0, 300.0, &safety);
    let mut ramp = Toolpath::new();
    ramp.rapid_to(start);
    let ramp_refusal = super::entry_descent::emit_ramp(
        &mut ramp,
        &start,
        &end,
        super::entry_descent::RampLegs::along((1.0, 0.0)),
        20.0,
        300.0,
        &safety,
        None,
    );
    assert!(helix_refusal.is_none() && ramp_refusal.is_none());
    for (name, tp) in [("helix", &helix), ("ramp", &ramp)] {
        let mut entry_moves = 0;
        for pair in tp.moves.windows(2) {
            let (a, b) = (pair[0].target, pair[1].target);
            if matches!(
                pair[1].intent,
                MoveIntent::EntryHelix | MoveIntent::EntryRamp
            ) {
                entry_moves += 1;
                let dxy = (b.x - a.x).hypot(b.y - a.y);
                assert!(
                    a.z - b.z <= dxy * slope * (1.0 + 1e-6) + 1e-9,
                    "{name}: a move from {a:?} to {b:?} is steeper than the limit"
                );
            }
        }
        assert!(entry_moves > 0, "{name}: no entry moves");
    }
    // A zero-radius helix can only plunge: 8 mm into stock, a warning.
    // P22-0c-3 (lead ruling on item 2): the depth runs from the material top
    // (8 mm), not from the entry start 0.5 mm above it (8.5 mm before).
    let mut plunge = Toolpath::new();
    plunge.rapid_to(start);
    let refusal =
        super::entry_descent::emit_helix(&mut plunge, &start, &end, 0.0, 1.0, 300.0, &safety)
            .expect("a plunge deeper than the protrusion is refused");
    assert!((refusal.depth_mm - 8.0).abs() < 1e-9, "{refusal:?}");
}

/// G-SIMCORE (P22-0b): a contained helix of a tool that does not cut at its
/// centre needs room for the core floor (`r >= d_core / 2`, 4 mm on the
/// probe tool). Room for r 3 only: the entry ramps and the toolpath carries
/// the fallback (`Produced::helix_core_fallbacks`). Room for r 5.4: a helix
/// of r >= 4, no fallback. Red before P22-0b: the r 3 room gave an r 3
/// helix, which leaves a post under the core.
#[test]
fn a_helix_with_no_room_for_the_core_ramps_and_names_it_g_simcore() {
    let core = crate::tool::body::CoreZone::from_geometry(
        crate::compute::tool_config::NonCentreCuttingGeometry {
            centre_core_diameter_mm: 8.0,
            blade_protrusion_mm: 1.0,
        },
    );
    let run = |half: f64| {
        let region = [crate::polygon::Polygon2::rectangle(
            10.0 - half,
            10.0 - half,
            10.0 + half,
            10.0 + half,
        )];
        let safety = EntrySafety {
            helix_containment: Some(&region),
            core: Some(core),
            ..no_probe(0.0)
        };
        without_provenance(apply_entry(
            AnnotatedToolpath::new(simple_plunge_toolpath()),
            EntryStyle::Helix {
                radius: 5.4,
                pitch: 1.0,
                // The fallback angle of every helix before P29-8.
                fallback_ramp_angle_deg: 3.0,
            },
            500.0,
            safety,
            9.0,
        ))
    };
    let tight = run(3.0);
    assert!(
        !tight
            .toolpath
            .moves
            .iter()
            .any(|m| m.intent == MoveIntent::EntryHelix),
        "room for r 3 under the core radius 4: the entry must not helix"
    );
    assert!(
        tight
            .toolpath
            .moves
            .iter()
            .any(|m| m.intent == MoveIntent::EntryRamp),
        "the entry falls back to the ramp"
    );
    let fallbacks = &tight.produced.helix_core_fallbacks;
    assert_eq!(fallbacks.len(), 1, "one fallback: {fallbacks:?}");
    assert!((fallbacks[0].fit_mm - 3.0).abs() < 1e-9);
    assert!((fallbacks[0].core_floor_mm - 4.0).abs() < 1e-12);
    assert_eq!(
        fallbacks[0].fallback,
        crate::compute::tool_config::EntryFallback::Ramp
    );

    let roomy = run(6.0);
    assert!(roomy.produced.helix_core_fallbacks.is_empty());
    let radius = roomy
        .toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryHelix)
        .map(|m| (m.target.x - 10.0).hypot(m.target.y - 10.0))
        .fold(0.0f64, f64::max);
    assert!(radius >= 4.0 - 1e-9, "the helix radius is {radius}");
}

/// The core findings of `tp` on `stock`, through the simulation walk with
/// the core check and the simulation's own table (P22-0c-2).
fn core_findings_on(
    stock: &mut TriDexelStock,
    tool: &crate::tool::ToolDefinition,
    tp: &Toolpath,
) -> Vec<usize> {
    let lut = crate::tool::body::cut_table(tool, crate::stock::radial_profile::LUT_SAMPLES);
    let mut check = crate::stock::contact::WalkChecks::new(tool);
    let cell = stock.z_grid.cell_size;
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            &lut,
            tool,
            tool.radius(),
            crate::dexel_stock::StockCutDirection::FromTop,
            crate::ids::ToolpathId::from_raw(0),
            18_000,
            2,
            5000.0,
            cell,
            None,
            &[],
            &[],
            true,
            &crate::interrupt::NeverCancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    let core = check.into_findings().core;
    core.into_iter().map(|hit| hit.move_index).collect()
}

/// P22-0c-2 (lead ruling on P22-0b open question 4, a must-fix gap): the
/// rest-stock ladder of `emit_ramp` (`pencil::plan_entry_ramp`) of a tool
/// that does not cut at its centre takes the zigzag rule of the core zone
/// (window >= the core diameter, laps no steeper than `p / 4c`), so the
/// ladder gives zero core findings. The control: the same tool through
/// the centre-cutting ladder (no core) strikes. Probe tool: 18 mm, core
/// 8 mm, protrusion 1 mm; rest stock 5 mm over a flat surface at Z 0.
/// P22-0c-3, design section 9 item 2 (wanaka 21, idx 14 move 21244: a
/// static `safety.non_centre_cutting_plunge` of 1.79 mm, no simulation core
/// finding). Before the fix the static check was wrong on such a move, for
/// two reasons:
///
/// 1. It measured the plunge from the entry START, the contact height
///    `material top + entry_clearance_mm`, so the 0.5 mm of air above the
///    material counted as wood (wanaka: 1.79 mm = 1.29 mm + 0.5 mm).
/// 2. It read the highest material over the whole entry disc, but only the
///    wood within the core radius meets the core; the blade ring cuts the
///    rest.
///
/// Lead ruling: the static check measures from the material top under the
/// core disc (`EntrySafety::core_top`, the kernel's read
/// `stock::contact::core_material_top`, which the entry callers set from
/// their stock), so before a simulation the static answer and the kernel
/// agree. Red before the fix: the static check warned on cases 1 and 2.
#[test]
fn the_static_plunge_and_the_kernel_give_one_answer_p22_0c() {
    let mut config = crate::compute::tool_config::ToolConfig::test_default(
        crate::compute::tool_config::ToolId::from_raw(0),
        crate::compute::tool_config::ToolType::EndMill,
    );
    config.diameter = 18.0;
    config.set_cutting_length(crate::compute::tool_config::ToolFact::Stated(30.0));
    config.set_stickout(crate::compute::tool_config::ToolFact::Stated(45.0));
    config.set_shank_diameter(crate::compute::tool_config::ToolFact::Stated(12.0));
    config.set_centre_cutting(crate::compute::tool_config::ToolFact::Stated(
        crate::compute::tool_config::CentreCutting::No(
            crate::compute::tool_config::NonCentreCuttingGeometry {
                centre_core_diameter_mm: 8.0,
                blade_protrusion_mm: 1.0,
            },
        ),
    ));
    let tool = crate::compute::cutter::build_cutter(&config).expect("a valid tool");
    let core = tool.body().core().expect("a non-centre-cutting tool");
    let block = |top: f64| {
        TriDexelStock::from_bounds(
            &crate::geo::BoundingBox3 {
                min: P3::new(-40.0, -40.0, -5.0),
                max: P3::new(40.0, 40.0, top),
            },
            0.25,
        )
    };
    let start = P3::new(0.0, 0.0, 10.0);
    let end = P3::new(0.0, 0.0, 0.0);
    // The entry as a caller with a stock builds it: the disc top for the
    // rapid floor, the core read for the plunge depth.
    let static_and_kernel = |stock: &TriDexelStock| {
        let disc_top = stock
            .max_conservative_top_z_in_disc(end.x, end.y, tool.radius())
            .expect("stock under the disc");
        let safety = EntrySafety {
            core: Some(core),
            stock_top_measured: true,
            core_top: crate::stock::contact::core_material_top(stock, &core, end.x, end.y),
            ..no_probe(disc_top)
        };
        let mut tp = Toolpath::new();
        tp.rapid_to(start);
        let refusal =
            super::entry_descent::emit_stock_aware_plunge(&mut tp, &start, &end, 300.0, &safety);
        let hits = core_findings_on(&mut stock.clone(), &tool, &tp);
        (refusal, hits)
    };

    // Case 1: 0.7 mm of wood (inside the 1 mm protrusion) under 0.5 mm of
    // entry clearance.
    let (refusal, hits) = static_and_kernel(&block(0.7));
    assert!(
        hits.is_empty(),
        "the kernel: 0.7 mm under the core is inside p"
    );
    assert!(
        refusal.is_none(),
        "case 1: the static check counts the clearance: {refusal:?}"
    );

    // Case 2: the disc stands at 5 mm, the column within 5 mm of the axis
    // is cut to 0.7 mm.
    let mut ring_only = block(5.0);
    let (rows, cols, cs, u0, v0) = (
        ring_only.z_grid.rows,
        ring_only.z_grid.cols,
        ring_only.z_grid.cell_size,
        ring_only.z_grid.origin_u,
        ring_only.z_grid.origin_v,
    );
    for row in 0..rows {
        for col in 0..cols {
            let (x, y) = (u0 + col as f64 * cs, v0 + row as f64 * cs);
            if x.hypot(y) <= 5.0 {
                ring_only.clear_above_at(row, col, 0.7);
            }
        }
    }
    let (refusal, hits) = static_and_kernel(&ring_only);
    assert!(
        hits.is_empty(),
        "the kernel: the core meets only the cut column"
    );
    assert!(
        refusal.is_none(),
        "case 2: the static check reads the ring: {refusal:?}"
    );

    // Case 3 (non-vacuity): 5 mm of fresh wood. Both warn.
    let (refusal, hits) = static_and_kernel(&block(5.0));
    assert!(!hits.is_empty(), "the kernel strikes a 5 mm plunge");
    let refusal = refusal.expect("the static check warns on a 5 mm plunge");
    assert!(
        (refusal.depth_mm - 5.0).abs() < 1e-6,
        "the depth is the wood under the core: {}",
        refusal.depth_mm
    );
}

/// P22-0c-3 (lead ruling 3 of P22-0c-2): a deep rest column needs more
/// laps under the core slope than the old cap of 64. The lap count follows
/// from the depth and the rule: no lap steps more than `window * p / 4c`.
/// Red before: the cap held 64 laps and the step grew to `depth / 64`.
#[test]
fn a_deep_rest_ladder_of_a_core_tool_keeps_the_core_slope_p22_0c() {
    let block = TriDexelStock::from_bounds(
        &crate::geo::BoundingBox3 {
            min: P3::new(-40.0, -40.0, -5.0),
            max: P3::new(40.0, 40.0, 20.0),
        },
        0.25,
    );
    let core = crate::tool::body::CoreZone {
        radius_mm: 4.0,
        protrusion_mm: 0.1,
    };
    let run = [P3::new(0.0, 0.0, 0.0), P3::new(20.0, 0.0, 0.0)];
    let contact = 3.0;
    let plan =
        crate::finish::pencil::plan_entry_ramp(&run, &block, contact, 30.0, true, Some(core))
            .expect("a ladder: the run is longer than the core diameter");
    // The window the plan laps: the run up to its window end.
    let window: f64 = run[..=plan.window_end]
        .windows(2)
        .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
        .sum();
    assert!(window >= core.min_ramp_leg_mm(), "the window is at least d");
    let depth = 20.0;
    assert!(
        depth / plan.step_mm > crate::finish::pencil::ENTRY_RAMP_MAX_LAPS as f64,
        "the fixture needs more laps than the old cap: step {}",
        plan.step_mm
    );
    assert!(
        plan.step_mm <= window * core.max_ramp_slope() * (1.0 + 1e-9),
        "a lap steps {} mm, more than the core rule {} mm over the {window} mm window",
        plan.step_mm,
        window * core.max_ramp_slope()
    );
}

#[test]
fn the_rest_ladder_of_a_non_centre_tool_keeps_the_core_out_of_wood_p22_0c() {
    let mut config = crate::compute::tool_config::ToolConfig::test_default(
        crate::compute::tool_config::ToolId::from_raw(0),
        crate::compute::tool_config::ToolType::EndMill,
    );
    config.diameter = 18.0;
    config.set_cutting_length(crate::compute::tool_config::ToolFact::Stated(30.0));
    config.set_stickout(crate::compute::tool_config::ToolFact::Stated(45.0));
    config.set_shank_diameter(crate::compute::tool_config::ToolFact::Stated(12.0));
    config.set_centre_cutting(crate::compute::tool_config::ToolFact::Stated(
        crate::compute::tool_config::CentreCutting::No(
            crate::compute::tool_config::NonCentreCuttingGeometry {
                centre_core_diameter_mm: 8.0,
                blade_protrusion_mm: 1.0,
            },
        ),
    ));
    let tool = crate::compute::cutter::build_cutter(&config).expect("a valid tool");
    let core = tool.body().core().expect("a non-centre-cutting tool");
    let block = || {
        TriDexelStock::from_bounds(
            &crate::geo::BoundingBox3 {
                min: P3::new(-40.0, -40.0, -5.0),
                max: P3::new(40.0, 40.0, 5.0),
            },
            0.25,
        )
    };
    let rest = block();
    let mesh = crate::mesh::make_test_flat(80.0);
    let index = crate::mesh::SpatialIndex::build(&mesh, 10.0);
    let start = P3::new(0.0, 0.0, 15.0);
    let end = P3::new(0.0, 0.0, 0.0);
    let ladder = |core: Option<crate::tool::body::CoreZone>| {
        let safety = EntrySafety {
            surface: Some(EntrySurfaceProbe {
                mesh: &mesh,
                index: &index,
                cutter: &tool,
                stock_to_leave: 0.0,
                off_mesh: OffMeshEntry::PlungeFallback,
                rest_stock: Some(&rest),
            }),
            fold_lap_cap: None,
            core,
            ..no_probe(5.0)
        };
        let mut tp = Toolpath::new();
        tp.rapid_to(start);
        let refusal = super::entry_descent::emit_ramp(
            &mut tp,
            &start,
            &end,
            super::entry_descent::RampLegs::along((1.0, 0.0)),
            3.0,
            300.0,
            &safety,
            None,
        );
        (tp, refusal)
    };
    let (old, _) = ladder(None);
    assert!(
        old.moves.iter().any(|m| m.intent == MoveIntent::EntryRamp),
        "control: the centre-cutting ladder ramps"
    );
    assert!(
        !core_findings_on(&mut block(), &tool, &old).is_empty(),
        "control: the centre-cutting ladder strikes with this tool"
    );
    let (new, refusal) = ladder(Some(core));
    assert!(refusal.is_none(), "the ladder has room: {refusal:?}");
    let ramp: Vec<P3> = new
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .map(|m| m.target)
        .collect();
    assert!(!ramp.is_empty(), "the core ladder ramps");
    let reach = ramp.iter().map(|p| p.x).fold(0.0_f64, f64::max);
    assert!(
        reach >= core.min_ramp_leg_mm() - 1e-9,
        "the window {reach:.3} is at least the core diameter"
    );
    for pair in new.moves.windows(2) {
        let (a, b) = (pair[0].target, pair[1].target);
        if pair[1].intent == MoveIntent::EntryRamp {
            let dxy = (b.x - a.x).hypot(b.y - a.y);
            assert!(
                a.z - b.z <= dxy * core.max_ramp_slope() * (1.0 + 1e-9) + 1e-9,
                "a lap from {a:?} to {b:?} is steeper than p / 4c"
            );
        }
    }
    let hits = core_findings_on(&mut block(), &tool, &new);
    assert!(
        hits.is_empty(),
        "core findings on the core ladder: {hits:?}"
    );
}

/// P29-8e (lead ruling Q8, F6): one cut gives one entry, whatever its move
/// count. A descent followed by a corner arc and a long line, the arc as one
/// `G2` move or as a fine polyline (0.5 mm chords, as the pipeline sees it
/// before arc fit): at a 20 degree angle on a tool radius of 3.175 mm the
/// walk budget (2.747 mm) is under the minimum run, and the old walk folded
/// the arc but refused the polyline (a straight plunge). Both now fold.
#[test]
fn a_fine_polyline_and_one_arc_give_the_same_entry_p29_8e() {
    use crate::toolpath::MoveIntent;
    let r = 3.175;
    let entry = |polyline: bool| -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -2.0), 500.0, MoveIntent::EntryPlunge);
        if polyline {
            // The arc about (r, 0) from 180 to 90 degrees, clockwise.
            let steps = 10;
            for k in 1..=steps {
                let a = std::f64::consts::PI * (1.0 - 0.5 * f64::from(k) / f64::from(steps));
                tp.feed_to_with_intent(
                    P3::new(r + r * a.cos(), r * a.sin(), -2.0),
                    1000.0,
                    MoveIntent::ClearingCut,
                );
            }
        } else {
            tp.arc_cw_to_with_intent(P3::new(r, r, -2.0), r, 0.0, 1000.0, MoveIntent::ClearingCut);
        }
        tp.feed_to_with_intent(P3::new(40.0, r, -2.0), 1000.0, MoveIntent::ClearingCut);
        tp.rapid_to(P3::new(40.0, r, 10.0));
        without_provenance(apply_entry(
            AnnotatedToolpath::new(tp),
            EntryStyle::Ramp {
                max_angle_deg: 20.0,
            },
            500.0,
            no_probe(0.0),
            r,
        ))
        .toolpath
    };
    for polyline in [false, true] {
        let tp = entry(polyline);
        let ramps = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryRamp)
            .count();
        let plunges = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryPlunge)
            .count();
        assert!(
            ramps > 0,
            "polyline {polyline}: the descent folds into a ramp"
        );
        assert_eq!(plunges, 0, "polyline {polyline}: no straight plunge stays");
    }
}

// ---------------------------------------------------------------------------
// Review 2026-11-15 F1: a fold ramp never goes below the cut it follows
// ---------------------------------------------------------------------------

/// The XY distance from `(x, y)` to the segment `a`-`b`, and the Z of the
/// segment at the nearest point.
fn fold_ref_nearest(a: P3, b: P3, x: f64, y: f64) -> (f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        (((x - a.x) * dx + (y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let (px, py) = (a.x + dx * t, a.y + dy * t);
    (
        ((x - px).powi(2) + (y - py).powi(2)).sqrt(),
        a.z + (b.z - a.z) * t,
    )
}

/// Every sample of every `EntryRamp` move of `out` that is below the cut
/// `reference` at its XY: `(x, y, z, cut z)`. The cut Z at an XY is the
/// LOWEST Z of the reference segments within `tol` of it (at a tab foot
/// the cut stands at both Z values); a sample with no segment within
/// `tol` is off the cut and is reported with a cut Z of infinity.
/// `reference` holds only the segments that have XY length: a vertical
/// lift or drop is at every Z between its ends.
fn fold_samples_below_the_cut(
    out: &Toolpath,
    reference: &[(P3, P3)],
    tol: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let mut low = Vec::new();
    for w in out.moves.windows(2) {
        if w[1].intent != MoveIntent::EntryRamp {
            continue;
        }
        let (a, b) = (w[0].target, w[1].target);
        for s in 0..=40 {
            let t = f64::from(s) / 40.0;
            let (x, y, z) = (
                a.x + t * (b.x - a.x),
                a.y + t * (b.y - a.y),
                a.z + t * (b.z - a.z),
            );
            let cut_z = reference
                .iter()
                .map(|&(p, q)| fold_ref_nearest(p, q, x, y))
                .filter(|&(d, _)| d <= tol)
                .map(|(_, cz)| cz)
                .fold(f64::INFINITY, f64::min);
            if z < cut_z - 1e-9 {
                low.push((x, y, z, cut_z));
            }
        }
    }
    low
}

/// The reviewer's reproduction (REVIEW.md F1, `zz_review_fold_over_a_tab`):
/// a plunge to z -2 at the origin, a cut to x 3, a tab lift to z -0.5, a
/// traverse to x 20, a drop to -2 and a cut to x 40. Before the fix the
/// return chord crossed the tab span at z -1.676 (19 samples under the tab
/// top). The run before the tab is 3 mm, too short for the fold.
#[test]
fn a_fold_ramp_never_runs_over_a_tab_review_f1() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to_with_intent(P3::new(0.0, 0.0, -2.0), 500.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(3.0, 0.0, -2.0), 1000.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(3.0, 0.0, -0.5), 800.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(20.0, 0.0, -0.5), 1000.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(20.0, 0.0, -2.0), 500.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(40.0, 0.0, -2.0), 1000.0, MoveIntent::ClearingCut);
    tp.rapid_to(P3::new(40.0, 0.0, 10.0));
    let out = without_provenance(apply_entry(
        AnnotatedToolpath::new(tp),
        EntryStyle::Ramp { max_angle_deg: 5.0 },
        500.0,
        no_probe(0.0),
        3.0,
    ))
    .toolpath;
    let ramp: Vec<P3> = out
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .map(|m| m.target)
        .collect();
    // The tab descent at x 20 is a plunge too: its ramp folds forward on
    // the cut after the tab (x 20 to 40). No ramp point is over the tab.
    for p in &ramp {
        assert!(
            p.x <= 3.0 + 1e-9 || p.x >= 20.0 - 1e-9,
            "a ramp point is over the tab span x 3 to 20: {p:?}"
        );
    }
    // The run before the tab is 3 mm. The lap cap (3 laps) admits a run of
    // at least 2 x 14.29 / 3 = 9.5 mm (half length 2.5 mm / tan 5 deg / 2),
    // so the first entry takes the stated plunge at the origin.
    assert!(
        out.moves.iter().any(|m| m.intent == MoveIntent::EntryPlunge
            && m.target.x.abs() < 1e-9
            && (m.target.z + 2.0).abs() < 1e-9),
        "the first entry falls back to the plunge at (0, 0, -2)"
    );
    let reference = [
        (P3::new(0.0, 0.0, -2.0), P3::new(3.0, 0.0, -2.0)),
        (P3::new(3.0, 0.0, -0.5), P3::new(20.0, 0.0, -0.5)),
        (P3::new(20.0, 0.0, -2.0), P3::new(40.0, 0.0, -2.0)),
    ];
    let low = fold_samples_below_the_cut(&out, &reference, 1e-6);
    assert!(low.is_empty(), "ramp samples below the cut: {low:?}");
}

/// A small deterministic generator (xorshift64) for the property test.
struct FoldRng(u64);

impl FoldRng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        // 53 bits of the state, in [0, 1).
        (self.0 >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

/// One generated cut: a plunge to the floor at the origin, then lines,
/// arcs and tabs (a lift, a level traverse on a line or an arc, a drop).
/// Returns the toolpath and the reference segments of the cut, with each
/// arc sampled by its own centre and angles (not by the product's arc
/// linearisation).
fn fold_generated_cut(rng: &mut FoldRng) -> (Toolpath, Vec<(P3, P3)>) {
    let floor = -rng.range(1.0, 4.0);
    let mut tp = Toolpath::new();
    let mut reference = Vec::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to_with_intent(P3::new(0.0, 0.0, floor), 500.0, MoveIntent::EntryPlunge);
    let mut at = P3::new(0.0, 0.0, floor);
    let mut heading = rng.range(0.0, std::f64::consts::TAU);
    // 3 to 9 pieces, with no float to integer cast (F195).
    let pieces = 3 + (0..6).filter(|_| rng.next() < 0.5).count();
    for _ in 0..pieces {
        let kind = rng.next();
        let tab = kind > 0.6;
        let z = if tab {
            floor + rng.range(0.2, -floor + 0.5)
        } else {
            floor
        };
        if tab {
            // The lift: vertical, or a short slope.
            let slope = if rng.next() > 0.5 {
                rng.range(0.1, 1.0)
            } else {
                0.0
            };
            let top = P3::new(
                at.x + heading.cos() * slope,
                at.y + heading.sin() * slope,
                z,
            );
            tp.feed_to_with_intent(top, 800.0, MoveIntent::ClearingCut);
            if slope > 0.0 {
                reference.push((at, top));
            }
            at = top;
        }
        if rng.next() > 0.5 {
            // A line.
            heading += rng.range(-1.6, 1.6);
            let len = rng.range(1.0, 8.0);
            let to = P3::new(at.x + heading.cos() * len, at.y + heading.sin() * len, z);
            tp.feed_to_with_intent(to, 1000.0, MoveIntent::ClearingCut);
            reference.push((at, to));
            at = to;
        } else {
            // A tangent arc.
            let r = rng.range(3.0, 8.0);
            let cw = rng.next() > 0.5;
            let side = if cw { -1.0 } else { 1.0 };
            let normal = heading + side * std::f64::consts::FRAC_PI_2;
            let (cx, cy) = (at.x + r * normal.cos(), at.y + r * normal.sin());
            let a0 = (at.y - cy).atan2(at.x - cx);
            let sweep = rng.range(0.5, 3.0);
            let a1 = a0 + side * sweep;
            let to = P3::new(cx + r * a1.cos(), cy + r * a1.sin(), z);
            if cw {
                tp.arc_cw_to_with_intent(to, cx - at.x, cy - at.y, 1000.0, MoveIntent::ClearingCut);
            } else {
                tp.arc_ccw_to_with_intent(
                    to,
                    cx - at.x,
                    cy - at.y,
                    1000.0,
                    MoveIntent::ClearingCut,
                );
            }
            let n = 200;
            let mut prev = at;
            for k in 1..=n {
                let a = a0 + side * sweep * f64::from(k) / f64::from(n);
                let p = P3::new(cx + r * a.cos(), cy + r * a.sin(), z);
                reference.push((prev, p));
                prev = p;
            }
            heading += side * sweep;
            at = to;
        }
        if tab {
            let down = P3::new(at.x, at.y, floor);
            tp.feed_to_with_intent(down, 500.0, MoveIntent::ClearingCut);
            at = down;
        }
    }
    tp.rapid_to(P3::new(at.x, at.y, 10.0));
    (tp, reference)
}

/// Review 2026-11-15 F1, rule (b): over generated cuts with tabs and arcs,
/// every sample of every emitted fold segment (not only its ends) is at or
/// above the cut that the fold follows, at that XY, and on the cut. The
/// tolerance 0.03 mm covers the chord of a linearised arc: the product
/// samples an arc at most 0.5 mm apart, and a 0.5 mm chord on the
/// smallest radius here (3 mm) is 0.0104 mm off the arc (s^2 / 8r).
#[test]
fn no_fold_ramp_point_is_below_the_followed_cut_review_f1() {
    let mut rng = FoldRng(0x9E37_79B9_7F4A_7C15);
    let mut folds = 0;
    for case in 0..400 {
        let (tp, reference) = fold_generated_cut(&mut rng);
        let angle = [3.0, 5.0, 10.0, 20.0][case % 4];
        let radius = [1.0, 2.0, 3.0][case % 3];
        let out = without_provenance(apply_entry(
            AnnotatedToolpath::new(tp),
            EntryStyle::Ramp {
                max_angle_deg: angle,
            },
            500.0,
            EntrySafety {
                fold_lap_cap: if case % 2 == 0 {
                    Some(super::entry_descent::RAMP_FOLD_MAX_LAPS)
                } else {
                    None
                },
                ..no_probe(0.0)
            },
            radius,
        ))
        .toolpath;
        if out.moves.iter().any(|m| m.intent == MoveIntent::EntryRamp) {
            folds += 1;
        }
        let low = fold_samples_below_the_cut(&out, &reference, 0.03);
        assert!(
            low.is_empty(),
            "case {case} (angle {angle}, radius {radius}): ramp samples below or off the cut: {:?}",
            &low[..low.len().min(5)]
        );
    }
    // The generator must exercise the fold, not only its plunge fallback.
    assert!(folds >= 100, "only {folds} of 400 cases folded");
}

/// P39 (R-1d): a ramp leg that the model clip lifts ships its whole clipped
/// profile when the 3D Rough planner chose it (`RampLegs::max_leg_mm`).
///
/// The fixture is the Wanaka back entry at (142.15, 43.81): a floor, a
/// ledge at Z 11 and the top of the part at Z 25.5 along +X, under stock to
/// Z 25.5. The ramp starts above all of it. Without the whole profile each
/// out leg is ONE chord from the start column up to the first lifted
/// sample, so no lap lowers the stock over the ledge, and the lap that
/// reaches Z 11 takes it in one piece (14 mm on the job). With it, no ramp
/// move meets more than one lap of stock. Red with `whole_profile` off.
#[test]
fn a_planned_ramp_leg_over_a_ledge_takes_one_lap_at_most_p39() {
    use crate::dexel_stock::StockCutDirection;
    use crate::geo::BoundingBox3;
    use crate::tool::{FlatEndmill, MillingCutter as _};
    const TOP: f64 = 25.5;
    const CELL: f64 = 0.25;
    let quad = |x0: f64, x1: f64, z: f64, base: u32| {
        (
            vec![
                P3::new(x0, -10.0, z),
                P3::new(x1, -10.0, z),
                P3::new(x1, 10.0, z),
                P3::new(x0, 10.0, z),
            ],
            vec![[base, base + 1, base + 2], [base, base + 2, base + 3]],
        )
    };
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    for (x0, x1, z, base) in [
        (-10.0, 14.6, 0.0, 0),
        (14.6, 16.5, 11.0, 4),
        (16.5, 30.0, TOP, 8),
    ] {
        let (v, t) = quad(x0, x1, z, base);
        vertices.extend(v);
        triangles.extend(t);
    }
    let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
    let index = crate::mesh::SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 30.0);
    let r = tool.radius();
    let safety = EntrySafety {
        surface: Some(EntrySurfaceProbe {
            mesh: &mesh,
            index: &index,
            cutter: &tool,
            stock_to_leave: 0.0,
            off_mesh: OffMeshEntry::Unconstrained,
            rest_stock: None,
        }),
        fold_lap_cap: None,
        stock_top_measured: true,
        contact_top: Some(TOP),
        ..no_probe(TOP)
    };
    let start = P3::new(0.0, 0.0, TOP + 0.5);
    let end = P3::new(0.0, 0.0, 8.0);
    // The most stock above the cutter along one ramp move, on a replay.
    let worst_bite = |legs: super::entry_descent::RampLegs| {
        let mut tp = Toolpath::new();
        tp.rapid_to(start);
        let _ =
            super::entry_descent::emit_ramp(&mut tp, &start, &end, legs, 3.0, 500.0, &safety, None);
        let mut stock = TriDexelStock::from_bounds(
            &BoundingBox3 {
                min: P3::new(-10.0, -10.0, -2.0),
                max: P3::new(30.0, 10.0, TOP),
            },
            CELL,
        );
        let lut = crate::stock::radial_profile::RadialProfileLUT::from_cutter(
            &tool,
            crate::stock::radial_profile::LUT_SAMPLES,
        );
        let mut worst = 0.0_f64;
        for pair in tp.moves.windows(2) {
            let (a, b) = (pair[0].target, pair[1].target);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let n = crate::util::steps::steps(dx.hypot(dy), CELL * 0.5)
                .unwrap_or(0)
                .max(1);
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let z = a.z + (b.z - a.z) * t;
                if let Some(top) =
                    stock.max_top_z_in_disc(a.x + dx * t, a.y + dy * t, r - 2.0 * CELL)
                {
                    worst = worst.max(top - z);
                }
            }
            stock.stamp_linear_segment(&lut, r, a, b, StockCutDirection::FromTop);
        }
        worst
    };
    let planned = worst_bite(super::entry_descent::RampLegs {
        dir: (1.0, 0.0),
        max_leg_mm: Some(19.0),
    });
    assert!(
        planned <= ENTRY_CLEARANCE + CELL,
        "a planned leg meets {planned:.2} mm of stock above the cutter; one lap is {ENTRY_CLEARANCE} mm"
    );
    // The fixture is the defect: the same legs with the clip of the other
    // doors (no whole profile) take far more than one lap.
    let unplanned = worst_bite(super::entry_descent::RampLegs::along((1.0, 0.0)));
    assert!(
        unplanned > 3.0 * ENTRY_CLEARANCE,
        "the fixture no longer shows the defect: {unplanned:.2} mm"
    );
}

/// P40 A: every shipped chord of a ramp or helix entry holds the model plus
/// the stock to leave, also where a leg crosses a wall of the model.
///
/// Record: `planning/feeds_contract_2026-11-03/phases/P40_WALL_CHORD_ONE_PASS_2026-11-17.md`.
///
/// The fixture is a floor at Z 0 and a top at Z 12 with a wall at X 10. A
/// 6 mm tool at X under 7 stands on the floor; from X 7 its edge is over
/// the top. Each entry starts in the floor part and its legs (or turns) go
/// over the line X 7. The clip of an entry reads the floor at samples
/// (`ENTRY_CLIP_SPACING_MM` for a ramp, 10 degrees for a helix). Without
/// the chord lift (`hold_entry_chords`) the chord between the last low
/// sample and the first lifted sample goes through the top edge of the
/// wall: 0.25 mm (the legs of the other doors), 5.1 mm (planned legs) and
/// 0.50 mm (the helix) under the limit at `0a1f60fe1`.
///
/// The judge is the rule of the model gouge guard for a 3D Rough
/// (`compute::model_gouge`, `Allowance`): the tool less the chord tolerance
/// on its radius, the limit `CL Z + leave - tolerance`, the Z bar
/// `MODEL_GOUGE_Z_TOL_MM`. The guard samples a move at a quarter of the
/// diameter; this test samples at `MODEL_GOUGE_TOL_MM`, so it reads the
/// inside of a chord 0.5 mm long.
///
/// A tool with a centre core is not a case: it takes a direction only when
/// each lap comes down again at its limit slope
/// (`replan_descent_at_limit`), so on this fixture it goes along -X.
#[test]
fn an_entry_chord_over_a_model_wall_holds_the_leave_p40() {
    use crate::compute::model_gouge::{MODEL_GOUGE_TOL_MM, MODEL_GOUGE_Z_TOL_MM};
    use crate::tool::FlatEndmill;
    const TOP: f64 = 12.0;
    /// The chord tolerance of the op of the fixture.
    const TOL: f64 = 0.1;
    let vertices = vec![
        P3::new(-20.0, -20.0, 0.0),
        P3::new(10.0, -20.0, 0.0),
        P3::new(10.0, 20.0, 0.0),
        P3::new(-20.0, 20.0, 0.0),
        P3::new(10.0, -20.0, TOP),
        P3::new(30.0, -20.0, TOP),
        P3::new(30.0, 20.0, TOP),
        P3::new(10.0, 20.0, TOP),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 7],
        [1, 7, 2],
        [4, 5, 6],
        [4, 6, 7],
    ];
    let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
    let index = crate::mesh::SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 30.0);
    // The reduced tool of the guard, as the 3D Rough builds it. The lift
    // of the entry reads `tool`; the judge reads this one.
    let reduced = FlatEndmill::new(6.0 - 2.0 * TOL, 30.0);
    let safety = EntrySafety {
        surface: Some(EntrySurfaceProbe {
            mesh: &mesh,
            index: &index,
            cutter: &tool,
            stock_to_leave: 0.0,
            off_mesh: OffMeshEntry::Unconstrained,
            rest_stock: None,
        }),
        chord_lift: Some(ChordLift { tolerance_mm: TOL }),
        fold_lap_cap: None,
        stock_top_measured: true,
        contact_top: Some(TOP),
        ..no_probe(TOP)
    };
    // The deepest point of an entry chord under the limit of the guard
    // (mm), and the count of entry moves that end on the top past the wall.
    let judge = |tp: &Toolpath| {
        let mut worst = 0.0_f64;
        let mut on_top = 0_usize;
        for pair in tp.moves.windows(2) {
            if !matches!(
                pair[1].intent,
                MoveIntent::EntryRamp | MoveIntent::EntryHelix
            ) {
                continue;
            }
            let (a, b) = (pair[0].target, pair[1].target);
            if b.x > 7.0 && (b.z - TOP).abs() < 1e-6 {
                on_top += 1;
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let n = crate::util::steps::steps(dx.hypot(dy), MODEL_GOUGE_TOL_MM)
                .unwrap_or(0)
                .max(1);
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let cl = crate::surface::dropcutter::point_drop_cutter(
                    a.x + dx * t,
                    a.y + dy * t,
                    &mesh,
                    &index,
                    &reduced,
                );
                if cl.contacted {
                    worst = worst.max(cl.z - TOL - (a.z + (b.z - a.z) * t));
                }
            }
        }
        (worst, on_top)
    };
    let ramp = |legs: super::entry_descent::RampLegs| {
        let start = P3::new(0.0, 0.0, TOP + 0.5);
        let end = P3::new(0.0, 0.0, 2.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(start);
        let _ =
            super::entry_descent::emit_ramp(&mut tp, &start, &end, legs, 3.0, 500.0, &safety, None);
        tp
    };
    let helix = {
        let start = P3::new(5.0, 0.0, TOP + 0.5);
        let end = P3::new(5.0, 0.0, 2.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(start);
        let _ = super::entry_descent::emit_helix(&mut tp, &start, &end, 2.5, 3.0, 500.0, &safety);
        tp
    };
    let cases = [
        (
            "ramp, the legs of the other doors",
            ramp(super::entry_descent::RampLegs::along((1.0, 0.0))),
        ),
        (
            "ramp, planned legs (whole profile)",
            ramp(super::entry_descent::RampLegs {
                dir: (1.0, 0.0),
                max_leg_mm: Some(19.0),
            }),
        ),
        ("helix", helix),
    ];
    let mut red = Vec::new();
    for (name, tp) in &cases {
        let (worst, on_top) = judge(tp);
        assert!(on_top > 0, "{name}: no entry move goes over the wall");
        if worst > MODEL_GOUGE_Z_TOL_MM {
            red.push(format!("{name}: {worst:.3} mm under the limit"));
        }
    }
    assert!(
        red.is_empty(),
        "an entry chord cuts the wall top (the bar is {MODEL_GOUGE_Z_TOL_MM} mm): {red:?}"
    );
}
