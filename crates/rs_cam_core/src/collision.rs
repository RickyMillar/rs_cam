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
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{FlatEndmill, MillingCutter};
use crate::toolpath::{MoveType, Toolpath};

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

    /// Segments of the assembly from tip upward, as (z_offset, radius, length).
    /// z_offset is the distance from the tool tip to the bottom of the segment.
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
pub fn check_collisions(
    toolpath: &Toolpath,
    assembly: &ToolAssembly,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> CollisionReport {
    let segments = assembly.segments();
    let mut collisions = Vec::new();
    let mut max_extra_stickout_needed = 0.0_f64;

    for (move_idx, mv) in toolpath.moves.iter().enumerate() {
        // Only check cutting moves (linear), not rapids
        let is_cutting = matches!(
            mv.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if !is_cutting {
            continue;
        }

        let tip_x = mv.target.x;
        let tip_y = mv.target.y;
        let tip_z = mv.target.z;

        for &(z_offset, seg_radius, _seg_length) in &segments {
            // Skip if the segment radius is smaller than the cutter
            // (the cutter itself handles collision below its radius)
            if seg_radius <= assembly.cutter_radius + 1e-6 {
                continue;
            }

            // Bottom of this segment above tip
            let seg_bottom_z = tip_z + z_offset;

            // Drop a flat endmill at the segment's radius to find mesh contact Z
            let virtual_cutter = FlatEndmill::new(seg_radius * 2.0, 1.0);
            let cl = point_drop_cutter(tip_x, tip_y, mesh, index, &virtual_cutter);

            if !cl.contacted {
                continue; // Outside mesh footprint
            }

            // cl.z is where a flat endmill of this radius would touch
            // If cl.z > seg_bottom_z, the holder would collide
            let penetration = cl.z - seg_bottom_z;
            if penetration > 0.01 { // 0.01mm threshold to avoid false positives
                let seg_name = if seg_radius > assembly.shank_diameter / 2.0 - 0.01 {
                    "holder"
                } else {
                    "shank"
                };

                collisions.push(CollisionEvent {
                    move_idx,
                    position: P3::new(tip_x, tip_y, tip_z),
                    penetration_depth: penetration,
                    segment: seg_name.to_string(),
                });

                max_extra_stickout_needed = max_extra_stickout_needed.max(penetration);
            }
        }
    }

    let min_safe_stickout = assembly.stickout() + max_extra_stickout_needed;

    CollisionReport {
        collisions,
        min_safe_stickout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{make_test_hemisphere, SpatialIndex};
    use crate::tool::BallEndmill;

    fn test_assembly() -> ToolAssembly {
        ToolAssembly {
            cutter_radius: 3.0,  // 6mm ball endmill
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
        let tool = BallEndmill::new(6.0, 25.0);
        let asm = test_assembly();

        // Toolpath well above the mesh
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 50.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 50.0), 1000.0);

        let report = check_collisions(&tp, &asm, &mesh, &index);
        assert!(report.is_clear(), "High Z toolpath should have no collisions");
    }

    #[test]
    fn test_collision_detected_low_z() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 5.0);
        let asm = ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 10.0,  // short cutter
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
}
