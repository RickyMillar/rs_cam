//! W2.2: the shared kernel prologue.
//!
//! [`super::execute_operation_annotated`] runs [`prepare`] before it calls the
//! registry row's `generate`. The prologue does three checks, in this order:
//!
//! 1. The cancel flag. A pre-set flag reports `Cancelled`, never a refusal
//!    that happened to come first (O-CANC; pinned by
//!    `cancellable_families_honour_a_preset_cancel_flag`).
//! 2. The geometry the row's [`GeometryRequirement`] names.
//! 3. The tool, against the row's `tool_constraints`, with the registry's
//!    text ([`OperationType::tool_refusal`]).
//!
//! An adapter receives the checked geometry as a [`Prepared`]. It does not
//! check the mesh, the index, the polygons or the tool kind again.

use crate::compute::catalog::{GeometryRequirement, OperationConfig, OperationType};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;

use super::{ExecutionContext, OperationError};

/// A mesh and its spatial index, both present.
#[derive(Clone, Copy)]
pub struct MeshInput<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
}

/// The geometry the prologue checked, by the row's [`GeometryRequirement`].
#[derive(Clone, Copy)]
enum CheckedGeometry<'a> {
    /// `Stock` and `DrillTargets`: no model geometry to check.
    None,
    Polygons(&'a [Polygon2]),
    Mesh(MeshInput<'a>),
    Both {
        polygons: &'a [Polygon2],
        mesh: MeshInput<'a>,
    },
}

/// What an adapter receives from the prologue: the inputs its row requires,
/// already checked.
///
/// The accessors return `Err` only when an adapter asks for geometry its own
/// row does not require. That is a registry defect, the same class as a
/// `config_guard!` mismatch, and the text says so.
pub struct Prepared<'a> {
    op_type: OperationType,
    geometry: CheckedGeometry<'a>,
}

impl<'a> Prepared<'a> {
    /// The outlines. Never empty.
    pub fn polygons(&self) -> Result<&'a [Polygon2], OperationError> {
        match self.geometry {
            CheckedGeometry::Polygons(polygons) | CheckedGeometry::Both { polygons, .. } => {
                Ok(polygons)
            }
            CheckedGeometry::None | CheckedGeometry::Mesh(_) => Err(self.mismatch("polygons")),
        }
    }

    /// The mesh and its spatial index.
    pub fn mesh(&self) -> Result<MeshInput<'a>, OperationError> {
        match self.geometry {
            CheckedGeometry::Mesh(mesh) | CheckedGeometry::Both { mesh, .. } => Ok(mesh),
            CheckedGeometry::None | CheckedGeometry::Polygons(_) => Err(self.mismatch("a mesh")),
        }
    }

    fn mismatch(&self, what: &str) -> OperationError {
        OperationError::Other(format!(
            "registry adapter mismatch: {} reads {what}, but its row's geometry is {:?}",
            self.op_type.name(),
            self.op_type.geometry_requirement()
        ))
    }
}

/// THE prologue: the cancel flag first, then [`check_inputs`].
pub(super) fn prepare<'a>(
    ctx: &ExecutionContext<'a>,
    op: &OperationConfig,
) -> Result<Prepared<'a>, OperationError> {
    if ctx.is_cancelled() {
        return Err(OperationError::Cancelled);
    }
    check_inputs(ctx, op)
}

/// The geometry and the tool checks of the prologue, without the cancel
/// check. Only [`prepare`] and the test that drives an adapter past the
/// prologue call it.
pub(super) fn check_inputs<'a>(
    ctx: &ExecutionContext<'a>,
    op: &OperationConfig,
) -> Result<Prepared<'a>, OperationError> {
    let op_type = op.op_type();
    let name = op_type.name();
    let geometry = match op_type.geometry_requirement() {
        GeometryRequirement::Stock | GeometryRequirement::DrillTargets => CheckedGeometry::None,
        GeometryRequirement::Polygons => CheckedGeometry::Polygons(require_polygons(ctx, name)?),
        GeometryRequirement::Mesh => CheckedGeometry::Mesh(require_mesh(ctx, name)?),
        GeometryRequirement::Both => CheckedGeometry::Both {
            polygons: require_polygons(ctx, name)?,
            mesh: require_mesh(ctx, name)?,
        },
    };
    if let Some(text) = op_type.tool_refusal(ctx.tool_cfg.tool_type.cutter_kind()) {
        return Err(OperationError::InvalidTool(text));
    }
    Ok(Prepared { op_type, geometry })
}

/// CMP-13: the refusal names the operation. An empty list refuses as an
/// absent one.
fn require_polygons<'a>(
    ctx: &ExecutionContext<'a>,
    op_name: &str,
) -> Result<&'a [Polygon2], OperationError> {
    ctx.polygons
        .filter(|p| !p.is_empty())
        .ok_or_else(|| OperationError::MissingGeometry(format!("{op_name} requires 2D geometry")))
}

/// The mesh, then its spatial index. The two texts are the ones the
/// adapters carried before W2.2.
fn require_mesh<'a>(
    ctx: &ExecutionContext<'a>,
    op_name: &str,
) -> Result<MeshInput<'a>, OperationError> {
    let mesh = ctx
        .mesh
        .ok_or_else(|| OperationError::MissingGeometry(format!("{op_name} requires a 3D mesh")))?;
    let index = ctx
        .index
        .ok_or_else(|| OperationError::Other(format!("{op_name} requires a spatial index")))?;
    Ok(MeshInput { mesh, index })
}
