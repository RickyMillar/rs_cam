//! Core collision checking wrapper -- runs holder/shank collision detection
//! against a mesh without any GUI dependencies.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::interrupt::Cancelled;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::stock::collision::{
    CollisionObstacle, CollisionReport, check_collisions_interpolated_with_cancel,
    check_obstacle_collisions_with_cancel,
};
use crate::tool::ToolDefinition;
use crate::toolpath::Toolpath;

/// Request for a holder/shank collision check.
pub struct CollisionCheckRequest<'a> {
    pub toolpath: &'a Toolpath,
    pub tool: ToolDefinition,
    pub mesh: &'a TriangleMesh,
    /// Workholding fixtures (clearance-expanded boxes, in the toolpath's
    /// frame) the assembly must also clear. Empty when the setup has no
    /// enabled fixtures. W0.1 / P6-003: without these, a holder crashing
    /// a clamp is never flagged.
    pub obstacles: Vec<CollisionObstacle>,
    /// A spatial index the caller already built over `mesh`. `None` makes
    /// this call build its own.
    ///
    /// CMP-24: a sweep over N toolpaths that bind ONE model used to build
    /// the same index N times, because the only entry point built one per
    /// call. A caller that holds the index passes it here and the build
    /// happens once per distinct model. The index MUST describe `mesh`;
    /// an index over another mesh reports collisions against that other
    /// geometry.
    pub index: Option<&'a SpatialIndex>,
}

/// Result of a collision check.
pub struct CollisionCheckResult {
    pub collision_report: CollisionReport,
}

/// What a holder/shank collision check knows about one toolpath.
///
/// Three states, because a check that FAILED is not a check that found
/// nothing. `holder_collision_counts` used to answer `0` for all three,
/// so every consumer read a failure as a clean bill of health on the one
/// question that wrecks a machine (CMP-14). Commit `70a3db27` fixed the
/// same shape in the CLI on the audit day; this is the core twin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderCollisionCheck {
    /// The toolpath binds no mesh — a 2D operation. The check does not
    /// apply. [`Self::count`] reports `Some(0)`, which is the CLI's rule
    /// (`SessionError::MissingGeometry` is the expected 2D case there):
    /// with no model there is no model geometry to collide with, so the
    /// count is a true zero rather than a withheld one.
    NotApplicable,
    /// The check ran and could not finish — a cancellation, or a toolpath
    /// whose tool the project no longer defines. The count is UNKNOWN.
    /// It must never read as zero.
    Failed,
    /// The check ran to the end. `Measured(0)` is measured and clear.
    Measured(usize),
}

impl HolderCollisionCheck {
    /// The count a consumer may publish. `None` means the check failed,
    /// so no count exists. Read [`Self::NotApplicable`] for why a
    /// not-applicable check reports `Some(0)`.
    pub fn count(self) -> Option<usize> {
        match self {
            Self::NotApplicable => Some(0),
            Self::Failed => None,
            Self::Measured(n) => Some(n),
        }
    }

    /// Collisions this check actually found. A failed check found none,
    /// because it found nothing at all — use [`Self::failed`] to report
    /// the absence.
    pub fn collisions(self) -> usize {
        match self {
            Self::Measured(n) => n,
            Self::NotApplicable | Self::Failed => 0,
        }
    }

    /// Did the check run and fail?
    pub fn failed(self) -> bool {
        matches!(self, Self::Failed)
    }
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
/// CMP-24: the result carries the report and nothing else. It used to
/// flatten every `CollisionEvent::position` into `Vec<[f32; 3]>` — a
/// render type in the core model, read by the viewport, the GPU upload and
/// the picker alone. The viz upload path owns that conversion for every
/// other marker it draws; it owns this one too.
pub fn run_collision_check(
    request: &CollisionCheckRequest<'_>,
    cancel: &AtomicBool,
) -> Result<CollisionCheckResult, CollisionCheckError> {
    let owned_index;
    let index = match request.index {
        Some(prebuilt) => prebuilt,
        None => {
            owned_index = SpatialIndex::build_auto(request.mesh);
            &owned_index
        }
    };
    let assembly = request.tool.to_assembly();
    let cancel_check = || cancel.load(Ordering::SeqCst);

    let mut report = check_collisions_interpolated_with_cancel(
        request.toolpath,
        &assembly,
        request.mesh,
        index,
        1.0,
        &cancel_check,
    )
    .map_err(|_cancelled| CollisionCheckError::Cancelled)?;

    // W0.1 — also test the assembly against workholding fixtures. The mesh
    // check above can only see the workpiece, so a holder/shank crashing a
    // clamp would otherwise go unflagged. Same 1mm interpolation; fixture
    // hits are merged into the report's collision list.
    let fixture_hits = check_obstacle_collisions_with_cancel(
        request.toolpath,
        &assembly,
        &request.obstacles,
        1.0,
        &cancel_check,
    )
    .map_err(|_cancelled| CollisionCheckError::Cancelled)?;
    report.collisions.extend(fixture_hits);

    Ok(CollisionCheckResult {
        collision_report: report,
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
            crate::compute::tool_config::ToolMaterial::Carbide,
        );

        let req = CollisionCheckRequest {
            toolpath: &tp,
            tool,
            mesh: &mesh,
            obstacles: Vec::new(),
            index: None,
        };
        let cancel = AtomicBool::new(false);
        let result = run_collision_check(&req, &cancel).unwrap();
        assert!(result.collision_report.is_clear());
    }

    /// CMP-24: a caller that already holds the index gets the same answer
    /// as one that makes the wrapper build its own.
    #[test]
    fn a_prebuilt_index_gives_the_same_answer() {
        let mesh = make_test_hemisphere(20.0, 16);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 50.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let tool = || {
            ToolDefinition::new(
                Box::new(crate::tool::FlatEndmill::new(6.0, 25.0)),
                6.0,
                20.0,
                25.0,
                45.0,
                2,
                crate::compute::tool_config::ToolMaterial::Carbide,
            )
        };
        let cancel = AtomicBool::new(false);

        let built_here = run_collision_check(
            &CollisionCheckRequest {
                toolpath: &tp,
                tool: tool(),
                mesh: &mesh,
                obstacles: Vec::new(),
                index: None,
            },
            &cancel,
        )
        .unwrap();

        let index = SpatialIndex::build_auto(&mesh);
        let handed_in = run_collision_check(
            &CollisionCheckRequest {
                toolpath: &tp,
                tool: tool(),
                mesh: &mesh,
                obstacles: Vec::new(),
                index: Some(&index),
            },
            &cancel,
        )
        .unwrap();

        assert_eq!(
            built_here.collision_report.collisions.len(),
            handed_in.collision_report.collisions.len()
        );
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
            crate::compute::tool_config::ToolMaterial::Carbide,
        );

        let req = CollisionCheckRequest {
            toolpath: &tp,
            tool,
            mesh: &mesh,
            obstacles: Vec::new(),
            index: None,
        };
        let cancel = AtomicBool::new(true);
        let result = run_collision_check(&req, &cancel);
        assert!(result.is_err());
    }
}
