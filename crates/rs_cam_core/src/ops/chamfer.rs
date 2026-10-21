//! Chamfer operation: edge chamfers using a V-bit along profile edges.
//!
//! A chamfer is a profile cut at one computed Z depth. The V-bit tip runs a
//! small distance on the air side of the edge, and the cone cuts a flat bevel
//! of the asked width into the top edge of the material.
//!
//! # Which side is material (W1.3b)
//!
//! [`ChamferParams::side`] says where the material is:
//!
//! - `Outside`: the outline is a part. The material is inside the outline,
//!   and the air is outside it.
//! - `Inside`: the outline is a cavity (a pocket or a hole). The air is
//!   inside the outline, and the material is outside it.
//!
//! The rule for every ring of the outline: the bevel goes INTO the material.
//! A hole ring of an `Outside` outline is a hole in the part, so its air is
//! inside the hole. A hole ring of an `Inside` outline is an island in the
//! cavity, so its air is outside the island. One signed offset of the whole
//! outline (exterior and holes together) moves every ring `t` into its air:
//! outward by `t` for `Outside`, inward by `t` for `Inside`. The operation
//! cuts every ring of every offset result. Before W1.3b, the operation cut
//! only the exterior of the first offset result, so the edge of a hole in a
//! part had no chamfer.

use crate::ops::profile::{ProfileParams, ProfileSide, profile_path_to_toolpath};
use crate::polygon::Polygon2;
use crate::toolpath::RingMaterial;
use crate::toolpath::{MaterialRing, SidedToolpath, Toolpath};

/// Parameters for chamfer cutting with a V-bit.
pub struct ChamferParams {
    /// Width of the chamfer on the workpiece face (mm).
    pub chamfer_width: f64,
    /// Horizontal distance (mm) from the edge to the V-bit tip. The tip runs
    /// this far into the air side of the wall, so the bevel stops above the
    /// tip and the fragile point does not cut. Default: 0.1 mm.
    pub tip_offset: f64,
    /// Half angle of the V-bit in radians (e.g., 45-degree V-bit = pi/4).
    pub tool_half_angle: f64,
    /// Where the material is: `Outside` chamfers a part, `Inside` chamfers a
    /// cavity. The module doc states the rule for the hole rings.
    pub side: ProfileSide,
    /// Cutting feed rate (mm/min).
    pub feed_rate: f64,
    /// Plunge feed rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves (mm).
    pub safe_z: f64,
    /// Stock-top Z in the emission frame (F-028 / S.2). Chamfer cuts at
    /// `top_z - depth`. Pre-S.2 this was implicitly `0.0` (chamfer
    /// hardcoded `cut_depth = -depth`), which only produced cuts inside
    /// the stock when the world stock top happened to sit at world Z=0.
    /// Callers should pass `heights.top_z` from the resolved height stack.
    pub top_z: f64,
}

/// Compute the cut depth (tip below the stock top) for a chamfer.
///
/// The cone radius at the stock top must reach `chamfer_width` into the
/// material from a tip that sits `tip_offset` into the air.
///
/// `depth = (chamfer_width + tip_offset) / tan(half_angle)`
///
/// The Layered Chamfer (`compute/execute/curve_engrave.rs`) reads it as its
/// one ladder level, `top_z - depth`.
pub(crate) fn chamfer_depth(params: &ChamferParams) -> f64 {
    (params.chamfer_width + params.tip_offset) / params.tool_half_angle.tan()
}

/// The tool-centre rings: every ring of the outline, moved `tip_offset` into
/// its air side (see the module doc), and the count of offsets that failed
/// rather than collapsed (Checkpoint C: `1` when the one offset this makes
/// failed, else `0`). The rings do not depend on Z: the Layered Chamfer
/// makes them once in `prepare` and stamps them at its one level with
/// [`chamfer_rings_at_z`].
pub(crate) fn chamfer_rings_reported(
    polygon: &Polygon2,
    params: &ChamferParams,
) -> (Vec<MaterialRing>, usize) {
    // The offset sign and the hole classification both read the winding
    // (exterior CCW, holes CW). Importers normalise it; a hand-built outline
    // may not.
    let normalised;
    let polygon = if polygon.has_correct_winding() {
        polygon
    } else {
        let mut p = polygon.clone();
        p.ensure_winding();
        normalised = p;
        &normalised
    };
    let (pieces, failures) = if params.tip_offset.abs() < 1e-9 {
        // A zero tip offset puts the tip on the edge itself: no offset to make.
        (vec![polygon.clone()], 0)
    } else {
        // Negative distance = outward, positive = inward (profile's sign).
        let distance = match params.side {
            ProfileSide::Outside => -params.tip_offset,
            ProfileSide::Inside => params.tip_offset,
        };
        let (pieces, failure) = crate::polygon::offset_polygon_reported(polygon, distance);
        (pieces, usize::from(failure.is_some()))
    };
    // P6.0c: an exterior ring has the material on the side the module doc
    // names (inside a part's ring, outside a cavity's ring); a hole ring
    // has it on the other side.
    let exterior = params.side.ring_material();
    let hole = match exterior {
        RingMaterial::Inside => RingMaterial::Outside,
        _ => RingMaterial::Inside,
    };
    let rings = pieces
        .into_iter()
        .flat_map(|p| {
            std::iter::once(MaterialRing {
                points: p.exterior,
                material: exterior,
            })
            .chain(p.holes.into_iter().map(move |points| MaterialRing {
                points,
                material: hole,
            }))
        })
        .filter(|ring| ring.points.len() >= 3)
        .collect();
    (rings, failures)
}

/// The cut of the tool-centre `rings` at the tip Z `z`: one closed profile
/// pass per ring, each with its own approach and retract.
pub(crate) fn chamfer_rings_at_z(
    rings: &[MaterialRing],
    z: f64,
    params: &ChamferParams,
) -> SidedToolpath {
    let profile_params = ProfileParams {
        tool_radius: params.tip_offset,
        side: params.side,
        cut_depth: z,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        climb: false,
        compensate_in_controller: false,
    };
    let mut tp = SidedToolpath::new();
    for ring in rings {
        tp.append(profile_path_to_toolpath(
            &ring.points,
            &profile_params,
            ring.material,
        ));
    }
    tp
}

/// Generate a chamfer toolpath on every edge of a polygon using a V-bit.
///
/// # Geometry
///
/// For a V-bit with half angle `a`, chamfer width `w`, and tip offset `t`:
/// - The tool centre (the tip) follows each ring offset `t` into its air
///   side.
/// - Cut depth: `d = (w + t) / tan(a)` below the stock top.
/// - At the stock top the cone radius is `d * tan(a) = w + t`, so the bevel
///   meets the top face `w` into the material from the edge.
/// - At the edge the cone is `t / tan(a)` above the tip, so the bevel meets
///   the wall `w / tan(a)` below the stock top.
///
/// The bevel makes the angle `90° - a` with the top face: 45° for a 90° bit.
/// G-CHAMFERSIDE (`tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs`)
/// measures this on the simulated stock for an outside edge, and
/// G-CHAMFERINSIDE
/// (`tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs`)
/// for a pocket wall and a hole in a part. Before G-CHAMFERSIDE, the tool
/// centre followed the outline offset `w + t` outward, and the cone touched
/// the part only at the edge line (D4).
///
/// This is the composition of the two halves the Layered Chamfer calls:
/// [`chamfer_rings_reported`] and [`chamfer_rings_at_z`] at
/// `top_z - depth`.
pub fn chamfer_toolpath(polygon: &Polygon2, params: &ChamferParams) -> Toolpath {
    let (rings, _failures) = chamfer_rings_reported(polygon, params);
    chamfer_rings_at_z(&rings, params.top_z - chamfer_depth(params), params).toolpath
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;
    use std::f64::consts::FRAC_PI_4;

    fn default_chamfer_params() -> ChamferParams {
        ChamferParams {
            chamfer_width: 2.0,
            tip_offset: 0.1,
            tool_half_angle: FRAC_PI_4, // 45 degrees
            side: ProfileSide::Outside,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 10.0,
            top_z: 0.0,
        }
    }

    #[test]
    fn test_chamfer_depth_45_degree() {
        // 45-degree V-bit: tan(pi/4) = 1.0
        // depth = (2.0 + 0.1) / 1.0 = 2.1
        let params = default_chamfer_params();
        let depth = chamfer_depth(&params);
        assert!(
            (depth - 2.1).abs() < 1e-10,
            "Expected depth 2.1, got {}",
            depth
        );
    }

    #[test]
    fn test_chamfer_depth_30_degree() {
        // 30-degree V-bit (60-degree included): tan(pi/6) = 1/sqrt(3)
        let params = ChamferParams {
            chamfer_width: 2.0,
            tip_offset: 0.0,
            tool_half_angle: std::f64::consts::FRAC_PI_6,
            side: ProfileSide::Outside,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 10.0,
            top_z: 0.0,
        };
        let depth = chamfer_depth(&params);
        let expected = 2.0 / (std::f64::consts::FRAC_PI_6).tan();
        assert!(
            (depth - expected).abs() < 1e-10,
            "Expected depth {}, got {}",
            expected,
            depth
        );
        // tan(30 deg) ~ 0.5774, so depth ~ 3.464
        assert!(
            (depth - 3.464).abs() < 0.01,
            "30-degree depth should be ~3.464, got {}",
            depth
        );
    }

    #[test]
    fn test_tip_offset_increases_depth() {
        let mut params = default_chamfer_params();
        params.tip_offset = 0.0;
        let depth_no_offset = chamfer_depth(&params);

        params.tip_offset = 0.5;
        let depth_with_offset = chamfer_depth(&params);

        assert!(
            depth_with_offset > depth_no_offset,
            "Tip offset should increase depth: {} > {}",
            depth_with_offset,
            depth_no_offset
        );

        // For 45-degree bit, difference should equal the offset itself
        // (tan(45) = 1, so delta_depth = delta_offset / 1.0 = 0.5)
        let delta = depth_with_offset - depth_no_offset;
        assert!(
            (delta - 0.5).abs() < 1e-10,
            "Depth increase should be 0.5 for 45-deg bit, got {}",
            delta
        );
    }

    #[test]
    fn test_chamfer_on_square_produces_toolpath() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let tp = chamfer_toolpath(&square, &params);

        assert!(
            !tp.moves.is_empty(),
            "Chamfer on 40x40 square should produce moves"
        );

        // Should have rapids (approach + retract)
        let n_rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert_eq!(n_rapids, 2, "Expected 2 rapids, got {}", n_rapids);
    }

    #[test]
    fn test_chamfer_cut_depth_in_toolpath() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let expected_depth = chamfer_depth(&params); // 2.1 for default params
        let tp = chamfer_toolpath(&square, &params);

        // All cutting moves should be at z = -depth
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                assert!(
                    (m.target.z - (-expected_depth)).abs() < 1e-10,
                    "Cutting move at z={}, expected z={}",
                    m.target.z,
                    -expected_depth
                );
            }
        }
    }

    #[test]
    fn test_chamfer_offset_outside_boundary() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let tp = chamfer_toolpath(&square, &params);

        // The tip runs tip_offset (0.1 mm) outside the polygon boundary.
        // Check that cutting moves extend beyond the original square, and
        // by no more than the tip offset.
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .collect();

        assert!(!cutting_moves.is_empty(), "Should have cutting moves");

        let x_min = cutting_moves
            .iter()
            .map(|m| m.target.x)
            .fold(f64::INFINITY, f64::min);
        let x_max = cutting_moves
            .iter()
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        // Tool center should be outside the 0..40 boundary
        assert!(
            x_min < 0.0,
            "Tool center x_min={} should be outside boundary (< 0)",
            x_min
        );
        assert!(
            x_max > 40.0,
            "Tool center x_max={} should be outside boundary (> 40)",
            x_max
        );
        assert!(
            (x_min - -params.tip_offset).abs() < 1e-6
                && (x_max - (40.0 + params.tip_offset)).abs() < 1e-6,
            "Tool center must run tip_offset outside the boundary: x in [{x_min}, {x_max}]"
        );
    }

    #[test]
    fn test_zero_tip_offset_follows_the_edge() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = ChamferParams {
            tip_offset: 0.0,
            ..default_chamfer_params()
        };
        let tp = chamfer_toolpath(&square, &params);
        let cuts: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10))
            .collect();
        assert!(!cuts.is_empty(), "Should have cutting moves");
        for m in cuts {
            let on_edge = m.target.x.abs() < 1e-9
                || (m.target.x - 40.0).abs() < 1e-9
                || m.target.y.abs() < 1e-9
                || (m.target.y - 40.0).abs() < 1e-9;
            assert!(on_edge, "Tip must follow the edge: {:?}", m.target);
        }
    }

    /// The distances of the cut moves from the origin, and their count.
    fn cut_radii(tp: &Toolpath, feed: f64) -> Vec<f64> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - feed).abs() < 1e-10))
            .map(|m| m.target.x.abs().max(m.target.y.abs()))
            .collect()
    }

    /// W1.3b: an Outside chamfer of a part with a square hole cuts the
    /// outer edge `t` outside it and the hole edge `t` inside the hole.
    #[test]
    fn an_outside_chamfer_cuts_the_hole_edge_from_inside_the_hole() {
        let mut part = Polygon2::rectangle(-20.0, -20.0, 20.0, 20.0);
        let mut hole = Polygon2::rectangle(-5.0, -5.0, 5.0, 5.0).exterior;
        hole.reverse();
        part.holes.push(hole);
        let params = default_chamfer_params();
        let tp = chamfer_toolpath(&part, &params);
        let radii = cut_radii(&tp, params.feed_rate);
        let t = params.tip_offset;
        assert!(radii.iter().any(|r| (r - (20.0 + t)).abs() < 1e-6));
        assert!(radii.iter().any(|r| (r - (5.0 - t)).abs() < 1e-6));
        // P6.0: the outer ring goes round each corner of the part on an
        // arc of radius t (points ON the arc), so the outer test is the
        // distance t to the part square, not a Chebyshev radius.
        for m in tp.moves.iter().filter(|m| {
            matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
        }) {
            let (x, y) = (m.target.x.abs(), m.target.y.abs());
            let outer = (x - 20.0).max(0.0).hypot((y - 20.0).max(0.0));
            assert!(
                (outer - t).abs() < 1e-6 || (x.max(y) - (5.0 - t)).abs() < 1e-6,
                "a cut at ({x}, {y}) is on no air side"
            );
        }
    }

    /// W1.3b: an Inside chamfer of a pocket with an island cuts the pocket
    /// wall `t` inside the pocket and the island edge `t` outside the island.
    #[test]
    fn an_inside_chamfer_cuts_the_island_edge_from_the_cavity() {
        let mut pocket = Polygon2::rectangle(-20.0, -20.0, 20.0, 20.0);
        let mut island = Polygon2::rectangle(-5.0, -5.0, 5.0, 5.0).exterior;
        island.reverse();
        pocket.holes.push(island);
        let params = ChamferParams {
            side: ProfileSide::Inside,
            ..default_chamfer_params()
        };
        let radii = cut_radii(&chamfer_toolpath(&pocket, &params), params.feed_rate);
        let t = params.tip_offset;
        assert!(radii.iter().any(|r| (r - (20.0 - t)).abs() < 1e-6));
        assert!(radii.iter().any(|r| (r - (5.0 + t)).abs() < 1e-6));
        for r in radii {
            // The outward offset of the island rounds its corners: a corner
            // point is at most `t` from the island, so its max norm is in
            // `[5 + t / sqrt(2), 5 + t]`.
            let on_island =
                r >= 5.0 + t * std::f64::consts::FRAC_1_SQRT_2 - 1e-6 && r <= 5.0 + t + 1e-6;
            assert!(
                (r - (20.0 - t)).abs() < 1e-6 || on_island,
                "a cut at {r} mm from the centre is on no air side"
            );
        }
    }

    /// A hand-built outline with the wrong winding gets the same rings.
    #[test]
    fn a_reversed_outline_gives_the_same_rings() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let mut reversed = square.clone();
        reversed.exterior.reverse();
        let params = default_chamfer_params();
        let a = cut_radii(&chamfer_toolpath(&square, &params), params.feed_rate);
        let b = cut_radii(&chamfer_toolpath(&reversed, &params), params.feed_rate);
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn test_chamfer_cuts_relative_to_stock_top_s2() {
        // S.2 / F-028: chamfer must cut relative to `top_z`, not world Z=0.
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = ChamferParams {
            top_z: 5.0,
            ..default_chamfer_params()
        };
        let expected_depth = chamfer_depth(&params);
        let tp = chamfer_toolpath(&square, &params);

        assert!(!tp.moves.is_empty(), "Expected moves");

        let mut saw_cut = false;
        for m in &tp.moves {
            match m.move_type {
                MoveType::Rapid => {
                    // Rapids (approach/retract) should stay at safe_z, unaffected by top_z.
                    assert!(
                        (m.target.z - params.safe_z).abs() < 1e-9,
                        "Rapid should be at safe_z={}, got {}",
                        params.safe_z,
                        m.target.z
                    );
                }
                MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10 => {
                    saw_cut = true;
                    assert!(
                        (m.target.z - (params.top_z - expected_depth)).abs() < 1e-10,
                        "Cutting move at z={}, expected z={}",
                        m.target.z,
                        params.top_z - expected_depth
                    );
                }
                _ => {}
            }
        }
        assert!(saw_cut, "Expected at least one cutting move");
    }
}
