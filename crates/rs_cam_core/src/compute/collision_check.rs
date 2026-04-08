//! Core collision checking wrapper -- runs holder/shank collision detection
//! against a mesh without any GUI dependencies.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::collision::{CollisionReport, check_collisions_interpolated_with_cancel};
use crate::interrupt::Cancelled;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::ToolDefinition;
use crate::toolpath::Toolpath;

/// Request for a holder/shank collision check.
pub struct CollisionCheckRequest<'a> {
    pub toolpath: &'a Toolpath,
    pub tool: ToolDefinition,
    pub mesh: &'a TriangleMesh,
}

/// Result of a collision check.
pub struct CollisionCheckResult {
    pub collision_report: CollisionReport,
    pub collision_positions: Vec<[f32; 3]>,
}

/// Error type for collision check failures.
#[derive(Debug, Clone)]
pub enum CollisionCheckError {
    /// The check was cancelled via the cancel flag.
    Cancelled,
}

impl std::fmt::Display for CollisionCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Collision check cancelled"),
        }
    }
}

impl std::error::Error for CollisionCheckError {}

impl From<Cancelled> for CollisionCheckError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// Run a holder/shank collision check for a toolpath against a mesh.
///
/// Builds a spatial index for the mesh, constructs the tool assembly from
/// the tool definition, and checks each cutting move for collisions with
/// 1mm interpolation along moves.
///
/// Returns collision positions as `[f32; 3]` arrays for rendering markers.
pub fn run_collision_check(
    request: &CollisionCheckRequest<'_>,
    cancel: &AtomicBool,
) -> Result<CollisionCheckResult, CollisionCheckError> {
    let index = SpatialIndex::build_auto(request.mesh);
    let assembly = request.tool.to_assembly();

    let report = check_collisions_interpolated_with_cancel(
        request.toolpath,
        &assembly,
        request.mesh,
        &index,
        1.0,
        &|| cancel.load(Ordering::SeqCst),
    )
    .map_err(|_cancelled| CollisionCheckError::Cancelled)?;

    let positions: Vec<[f32; 3]> = report
        .collisions
        .iter()
        .map(|collision| {
            [
                collision.position.x as f32,
                collision.position.y as f32,
                collision.position.z as f32,
            ]
        })
        .collect();

    Ok(CollisionCheckResult {
        collision_report: report,
        collision_positions: positions,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::mesh::make_test_hemisphere;

    #[test]
    fn collision_check_no_collision() {
        let mesh = make_test_hemisphere(20.0, 16);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 50.0), 1000.0);

        let tool = ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.0, 25.0)),
            6.0,
            20.0,
            25.0,
            45.0,
            2,
        );

        let req = CollisionCheckRequest {
            toolpath: &tp,
            tool,
            mesh: &mesh,
        };
        let cancel = AtomicBool::new(false);
        let result = run_collision_check(&req, &cancel).unwrap();
        assert!(result.collision_report.is_clear());
        assert!(result.collision_positions.is_empty());
    }

    #[test]
    fn collision_check_cancel() {
        let mesh = make_test_hemisphere(20.0, 16);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let tool = ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.0, 25.0)),
            6.0,
            20.0,
            25.0,
            45.0,
            2,
        );

        let req = CollisionCheckRequest {
            toolpath: &tp,
            tool,
            mesh: &mesh,
        };
        let cancel = AtomicBool::new(true);
        let result = run_collision_check(&req, &cancel);
        assert!(result.is_err());
    }
}
