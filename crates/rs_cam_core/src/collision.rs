//! Tool holder collision detection — warn when the holder or shank would
//! collide with the workpiece during a toolpath.
//!
//! MVP scope: holder modeled as a single cylinder above the cutter.
//! Detection uses drop-cutter at the holder radius — if the mesh surface
//! at a CL point is higher than the holder bottom, collision occurs.
//!
//! "Holder collision in 3-axis is just drop-cutter at larger radii"

use crate::dropcutter::point_drop_cutter;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::FlatEndmill;
use crate::toolpath::{MoveType, Toolpath};

/// A segment of the holder/shank geometry above the cutter.
///
/// Supports cylindrical and tapered (conical) sections. For tapered segments,
/// collision checking uses the maximum radius (conservative).
#[derive(Debug, Clone)]
pub struct HolderSegment {
    /// Distance from tool tip to bottom of this segment (mm).
    pub z_offset: f64,
    /// Radius at bottom of segment (mm).
    pub radius_bottom: f64,
    /// Radius at top of segment (mm). Equal to radius_bottom for cylinders.
    pub radius_top: f64,
    /// Length of this segment (mm).
    pub length: f64,
}

impl HolderSegment {
    /// Maximum radius of this segment (conservative for collision).
    pub fn max_radius(&self) -> f64 {
        self.radius_bottom.max(self.radius_top)
    }
}

/// Describes the physical tool assembly: cutter + shank + holder.
pub struct ToolAssembly {
    /// Radius of the cutting tool (from MillingCutter::radius())
    pub cutter_radius: f64,
    /// Cutting flute length (from MillingCutter::length())
    pub cutter_length: f64,
    /// Shank diameter above the cutting flutes (mm).
    pub shank_diameter: f64,
    /// Shank length above the cutting flutes (mm).
    pub shank_length: f64,
    /// Holder diameter (mm) — the collet/holder body.
    pub holder_diameter: f64,
    /// Holder length (mm).
    pub holder_length: f64,
}

impl ToolAssembly {
    /// Total tool stickout from holder face to cutter tip.
    pub fn stickout(&self) -> f64 {
        self.cutter_length + self.shank_length
    }

    /// Segments of the assembly from tip upward, as (z_offset, max_radius, length).
    fn segments(&self) -> Vec<(f64, f64, f64)> {
        let mut segs = Vec::new();

        // Shank: above cutter
        if self.shank_length > 0.0 {
            segs.push((
                self.cutter_length,
                self.shank_diameter / 2.0,
                self.shank_length,
            ));
        }

        // Holder: above shank
        if self.holder_length > 0.0 {
            segs.push((
                self.cutter_length + self.shank_length,
                self.holder_diameter / 2.0,
                self.holder_length,
            ));
        }

        segs
    }

    /// Build segments from a multi-segment holder profile.
    ///
    /// Each HolderSegment can have different bottom/top radii (tapered).
    /// Collision checking uses max_radius per segment (conservative).
    pub fn segments_from_profile(
        cutter_radius: f64,
        profile: &[HolderSegment],
    ) -> Vec<(f64, f64, f64)> {
        profile
            .iter()
            .filter(|s| s.max_radius() > cutter_radius + 1e-6)
            .map(|s| (s.z_offset, s.max_radius(), s.length))
            .collect()
    }
}

/// A single collision event.
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    /// Index of the move in the toolpath that caused the collision.
    pub move_idx: usize,
    /// Position of the tool tip when collision occurs.
    pub position: P3,
    /// How deep the holder penetrates the workpiece (mm, positive = penetration).
    pub penetration_depth: f64,
    /// Which segment collided: "shank" or "holder".
    pub segment: String,
}

/// Result of a collision check.
#[derive(Debug)]
pub struct CollisionReport {
    /// All detected collision events.
    pub collisions: Vec<CollisionEvent>,
    /// Minimum stickout that would avoid all collisions (mm).
    /// If no collisions, this equals the current stickout.
    pub min_safe_stickout: f64,
}

impl CollisionReport {
    /// True if no collisions were detected.
    pub fn is_clear(&self) -> bool {
        self.collisions.is_empty()
    }
}

/// Check a toolpath for holder/shank collisions against the mesh.
///
/// For each cutting move, checks whether the shank or holder cylinder
/// (modeled as a flat endmill at that radius) would contact the mesh
/// surface above the allowed Z.
///
/// `step_mm` controls interpolation along moves: positions are checked
/// every `step_mm` between move endpoints to catch collisions mid-travel.
/// Use 0.0 to check endpoints only (legacy behavior).
pub fn check_collisions(
    toolpath: &Toolpath,
    assembly: &ToolAssembly,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> CollisionReport {
    check_collisions_interpolated(toolpath, assembly, mesh, index, 0.0)
}

/// Check collisions with interpolated path sampling.
///
/// Samples every `step_mm` along each move (in addition to endpoints)
/// to catch collisions mid-travel on long linear moves.
pub fn check_collisions_interpolated(
    toolpath: &Toolpath,
    assembly: &ToolAssembly,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    step_mm: f64,
) -> CollisionReport {
    let never_cancel = || false;
    match check_collisions_interpolated_with_cancel(
        toolpath,
        assembly,
        mesh,
        index,
        step_mm,
        &never_cancel,
    ) {
        Ok(report) => report,
        Err(_) => CollisionReport {
            collisions: Vec::new(),
            min_safe_stickout: assembly.stickout(),
        },
    }
}

pub fn check_collisions_interpolated_with_cancel(
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
        // Only check cutting moves (linear), not rapids
        let is_cutting = matches!(
            mv.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if !is_cutting {
            continue;
        }

        // Get previous position for interpolation
        // SAFETY: move_idx > 0 checked in condition
        #[allow(clippy::indexing_slicing)]
        let prev = if move_idx > 0 {
            toolpath.moves[move_idx - 1].target
        } else {
            mv.target
        };

        // Generate sample points along this move
        let sample_points = if step_mm > 0.01 {
            let dx = mv.target.x - prev.x;
            let dy = mv.target.y - prev.y;
            let dz = mv.target.z - prev.z;
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
                vec![mv.target]
            }
        } else {
            vec![mv.target]
        };

        for tip in &sample_points {
            check_cancel(cancel)?;
            for &(z_offset, seg_radius, _seg_length) in &segments {
                if seg_radius <= assembly.cutter_radius + 1e-6 {
                    continue;
                }

                let seg_bottom_z = tip.z + z_offset;

                let virtual_cutter = FlatEndmill::new(seg_radius * 2.0, 1.0);
                let cl = point_drop_cutter(tip.x, tip.y, mesh, index, &virtual_cutter);

                if !cl.contacted {
                    continue;
                }

                let penetration = cl.z - seg_bottom_z;
                if penetration > 0.01 {
                    let seg_name = if seg_radius > assembly.shank_diameter / 2.0 - 0.01 {
                        "holder"
                    } else {
                        "shank"
                    };

                    collisions.push(CollisionEvent {
                        move_idx,
                        position: *tip,
                        penetration_depth: penetration,
                        segment: seg_name.to_owned(),
                    });

                    max_extra_stickout_needed = max_extra_stickout_needed.max(penetration);
                    break; // One collision per move per segment is enough
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

/// A rapid move that passes through stock material.
#[derive(Debug, Clone)]
pub struct RapidCollision {
    /// Index of the move in the toolpath.
    pub move_index: usize,
    /// Start position of the rapid.
    pub start: P3,
    /// End position of the rapid.
    pub end: P3,
}

/// Check for rapid (G0) moves that pass through stock material.
/// Samples points along each rapid move and checks if any point is below
/// the stock top Z and within the stock XY bounds.
pub fn check_rapid_collisions(
    toolpath: &Toolpath,
    stock_bbox: &crate::geo::BoundingBox3,
) -> Vec<RapidCollision> {
    let mut collisions = Vec::new();

    // SAFETY: i ranges 1..len, so i and i-1 are always valid
    #[allow(clippy::indexing_slicing)]
    for i in 1..toolpath.moves.len() {
        if !matches!(toolpath.moves[i].move_type, MoveType::Rapid) {
            continue;
        }

        let start = toolpath.moves[i - 1].target;
        let end = toolpath.moves[i].target;

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;

        // Skip purely vertical retracts (same XY, Z going up).
        // After a cutting move the tool retracts from already-machined
        // material — no stock can be present at that dexel column.
        let xy_dist_sq = dx * dx + dy * dy;
        if dz > 0.0 && xy_dist_sq < 0.01 {
            continue;
        }

        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        let n_steps = (dist / 1.0).ceil().max(1.0) as usize;

        let mut hit = false;
        for step in 0..=n_steps {
            let t = step as f64 / n_steps as f64;
            let px = start.x + t * dx;
            let py = start.y + t * dy;
            let pz = start.z + t * dz;

            if pz < stock_bbox.max.z
                && px >= stock_bbox.min.x
                && px <= stock_bbox.max.x
                && py >= stock_bbox.min.y
                && py <= stock_bbox.max.y
            {
                hit = true;
                break;
            }
        }

        if hit {
            collisions.push(RapidCollision {
                move_index: i,
                start,
                end,
            });
        }
    }

    collisions
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};

    fn test_assembly() -> ToolAssembly {
        ToolAssembly {
            cutter_radius: 3.0, // 6mm ball endmill
            cutter_length: 25.0,
            shank_diameter: 6.0,
            shank_length: 10.0,
            holder_diameter: 35.0,
            holder_length: 40.0,
        }
    }

    #[test]
    fn test_stickout() {
        let asm = test_assembly();
        assert!((asm.stickout() - 35.0).abs() < 0.01);
    }

    #[test]
    fn test_segments() {
        let asm = test_assembly();
        let segs = asm.segments();
        assert_eq!(segs.len(), 2);
        // Shank at z_offset=25, radius=3
        assert!((segs[0].0 - 25.0).abs() < 0.01);
        assert!((segs[0].1 - 3.0).abs() < 0.01);
        // Holder at z_offset=35, radius=17.5
        assert!((segs[1].0 - 35.0).abs() < 0.01);
        assert!((segs[1].1 - 17.5).abs() < 0.01);
    }

    #[test]
    fn test_no_collision_high_safe_z() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = test_assembly();

        // Toolpath well above the mesh
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 50.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 50.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        assert!(
            report.is_clear(),
            "High Z toolpath should have no collisions"
        );
    }

    #[test]
    fn test_collision_detected_low_z() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 10.0, // short cutter
            shank_diameter: 6.0,
            shank_length: 5.0,
            holder_diameter: 35.0,
            holder_length: 40.0,
        };

        // Toolpath at z=0 over the hemisphere center
        // Hemisphere is 20mm tall. With stickout=15, holder bottom at z=15.
        // Holder radius=17.5mm, drop-cutter contacts hemisphere peak at z≈20.
        // Penetration = 20 - 15 = 5mm.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 25.0));
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        // With such a short stickout and large holder over a hemisphere,
        // there should be collisions
        assert!(
            !report.is_clear(),
            "Short stickout over hemisphere should detect collisions"
        );
        assert!(
            report.min_safe_stickout > asm.stickout(),
            "Safe stickout should exceed current stickout"
        );
    }

    #[test]
    fn test_interpolated_catches_mid_move() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 10.0,
            shank_diameter: 6.0,
            shank_length: 5.0,
            holder_diameter: 35.0,
            holder_length: 40.0,
        };

        // Long move that sweeps over the hemisphere center
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-30.0, 0.0, 25.0));
        tp.feed_to(P3::new(-30.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(30.0, 0.0, 0.0), 1000.0); // sweeps over hemisphere

        // Without interpolation (endpoint only at x=30)
        let report_no_interp = check_collisions(&tp, &asm, &mesh, &index);
        // With interpolation (samples along the 60mm move)
        let report_interp = check_collisions_interpolated(&tp, &asm, &mesh, &index, 2.0);

        // Interpolated should catch more collisions (mid-move over hemisphere peak)
        assert!(
            report_interp.collisions.len() >= report_no_interp.collisions.len(),
            "Interpolation should catch at least as many collisions: {} vs {}",
            report_interp.collisions.len(),
            report_no_interp.collisions.len()
        );
    }

    #[test]
    fn test_tapered_holder_segment() {
        // Test the HolderSegment API with a tapered collet nut
        let profile = vec![
            HolderSegment {
                z_offset: 25.0,
                radius_bottom: 5.0, // narrow bottom
                radius_top: 10.0,   // wider top
                length: 10.0,
            },
            HolderSegment {
                z_offset: 35.0,
                radius_bottom: 17.5,
                radius_top: 17.5,
                length: 40.0,
            },
        ];

        let segs = ToolAssembly::segments_from_profile(3.0, &profile);
        assert_eq!(segs.len(), 2);
        // First segment: max_radius = 10.0 (tapered)
        assert!((segs[0].1 - 10.0).abs() < 0.01);
        // Second segment: max_radius = 17.5 (cylinder)
        assert!((segs[1].1 - 17.5).abs() < 0.01);
    }

    #[test]
    fn test_no_collision_adequate_stickout() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 5.0);

        // Generous stickout — holder is well above mesh
        let asm = ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 40.0, // 40mm cutter length
            shank_diameter: 6.0,
            shank_length: 20.0,
            holder_diameter: 35.0,
            holder_length: 40.0,
        };

        // Toolpath at mesh surface level (z ~ 0)
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 25.0));
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(15.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        assert!(
            report.is_clear(),
            "Adequate stickout should have no collisions, got {}",
            report.collisions.len()
        );
    }

    #[test]
    fn test_collision_report_safe_stickout() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 10.0,
            shank_diameter: 6.0,
            shank_length: 5.0,
            holder_diameter: 35.0,
            holder_length: 40.0,
        };

        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);

        if !report.is_clear() {
            // min_safe_stickout should be >= stickout + max_penetration
            for c in &report.collisions {
                assert!(c.penetration_depth > 0.0, "Penetration should be positive");
            }
        }
    }

    #[test]
    fn test_rapid_above_stock_no_collision() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.rapid_to(P3::new(100.0, 100.0, 50.0));

        let collisions = check_rapid_collisions(&tp, &stock);
        assert!(
            collisions.is_empty(),
            "Rapids above stock should not collide"
        );
    }

    #[test]
    fn test_rapid_through_stock_detected() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let mut tp = Toolpath::new();
        // Start high, feed down, then rapid across through stock
        tp.rapid_to(P3::new(0.0, 50.0, 50.0));
        tp.feed_to(P3::new(0.0, 50.0, 10.0), 500.0);
        tp.rapid_to(P3::new(80.0, 50.0, 10.0)); // Z=10 is inside stock

        let collisions = check_rapid_collisions(&tp, &stock);
        assert_eq!(collisions.len(), 1, "Should detect one rapid collision");
        assert_eq!(collisions[0].move_index, 2);
    }

    #[test]
    fn test_no_rapids_empty_result() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, 10.0), 1000.0);
        tp.feed_to(P3::new(80.0, 50.0, 10.0), 1000.0);

        let collisions = check_rapid_collisions(&tp, &stock);
        assert!(collisions.is_empty(), "No rapids means no rapid collisions");
    }

    #[test]
    fn test_vertical_retract_not_flagged() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let mut tp = Toolpath::new();
        // Feed down into stock, then retract straight up (vertical rapid).
        // This represents a tool retracting from a just-cut position — safe.
        tp.feed_to(P3::new(50.0, 50.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(50.0, 50.0, 30.0)); // vertical retract: same XY, Z going up

        let collisions = check_rapid_collisions(&tp, &stock);
        assert!(
            collisions.is_empty(),
            "Vertical retract from cut position should not be flagged, got {} collisions",
            collisions.len()
        );
    }

    #[test]
    fn test_diagonal_retract_still_flagged() {
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 20.0),
        };
        let mut tp = Toolpath::new();
        // Feed down into stock, then diagonal rapid (XY changes while below stock top)
        tp.feed_to(P3::new(50.0, 50.0, 5.0), 1000.0);
        tp.rapid_to(P3::new(70.0, 50.0, 30.0)); // diagonal: XY changes + Z going up

        let collisions = check_rapid_collisions(&tp, &stock);
        assert_eq!(
            collisions.len(), 1,
            "Diagonal retract through stock should still be flagged"
        );
    }

    #[test]
    fn test_finish_raster_retract_pattern() {
        // Simulate a 3D finish raster pattern: cut → retract → traverse → plunge → cut
        let stock = crate::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 12.0),
        };
        let safe_z = 14.0;
        let mut tp = Toolpath::new();

        // Row 0: rapid to start at safe_z, plunge, cut, retract
        tp.rapid_to(P3::new(0.0, 0.0, safe_z));
        tp.feed_to(P3::new(0.0, 0.0, 3.0), 500.0);   // plunge
        tp.feed_to(P3::new(100.0, 0.0, 5.0), 1000.0);  // cut
        tp.rapid_to(P3::new(100.0, 0.0, safe_z));       // vertical retract

        // Row 1: traverse at safe_z, plunge, cut, retract
        tp.rapid_to(P3::new(100.0, 1.0, safe_z));       // horizontal at safe_z
        tp.feed_to(P3::new(100.0, 1.0, 4.0), 500.0);   // plunge
        tp.feed_to(P3::new(0.0, 1.0, 6.0), 1000.0);    // cut
        tp.rapid_to(P3::new(0.0, 1.0, safe_z));         // vertical retract

        let collisions = check_rapid_collisions(&tp, &stock);
        assert!(
            collisions.is_empty(),
            "Well-formed raster finish should have zero rapid collisions, got {}",
            collisions.len()
        );
    }
}
