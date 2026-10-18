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
//!
//! W6.2: after the checks, the prologue resolves the op's [`Territory`]
//! once. [`Prepared::territory`] gives it to the adapter. K1: the territory
//! carries the setup's obstacles for every op; K3: it pre-clips them.
//!
//! P1 (geometry selection): the op's [`ItemUse`] decides which polygons it
//! gets. A `Regions` op gets the closed polygons only; the prologue records
//! a [`SelectionNoteFinding`] for the open paths it left out, and refuses
//! when no closed polygon is left. No op closes an open path with a chord
//! in silence.

use std::borrow::Cow;

use crate::compute::catalog::{GeometryRequirement, ItemUse, OperationConfig, OperationType};
use crate::compute::toolpath_stats::SelectionNoteFinding;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;

use super::territory::Territory;
use super::{ExecutionContext, OperationError};

/// A mesh and its spatial index, both present.
#[derive(Clone, Copy)]
pub struct MeshInput<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
}

/// The geometry the prologue checked, by the row's [`GeometryRequirement`].
/// The polygons are the ones the op's [`ItemUse`] serves: borrowed when the
/// op gets every polygon, owned when the prologue left open paths out.
enum CheckedGeometry<'a> {
    /// `Stock` and `DrillTargets`: no model geometry to check.
    None,
    Polygons(Cow<'a, [Polygon2]>),
    Mesh(MeshInput<'a>),
    Both {
        polygons: Cow<'a, [Polygon2]>,
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
    territory: Territory<'a>,
}

impl<'a> Prepared<'a> {
    /// The outlines the op's [`ItemUse`] serves. Never empty. A `Regions`
    /// op gets closed polygons only (P1).
    pub fn polygons(&self) -> Result<&[Polygon2], OperationError> {
        match &self.geometry {
            CheckedGeometry::Polygons(polygons) | CheckedGeometry::Both { polygons, .. } => {
                Ok(polygons)
            }
            CheckedGeometry::None | CheckedGeometry::Mesh(_) => Err(self.mismatch("polygons")),
        }
    }

    /// The mesh and its spatial index.
    pub fn mesh(&self) -> Result<MeshInput<'a>, OperationError> {
        match &self.geometry {
            CheckedGeometry::Mesh(mesh) | CheckedGeometry::Both { mesh, .. } => Ok(*mesh),
            CheckedGeometry::None | CheckedGeometry::Polygons(_) => Err(self.mismatch("a mesh")),
        }
    }

    /// The op type of the row the prologue checked.
    pub(crate) fn op_type(&self) -> OperationType {
        self.op_type
    }

    /// W6.2: the territory, resolved once for this op
    /// ([`Territory::resolve`]).
    pub fn territory(&self) -> &Territory<'a> {
        &self.territory
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
    let item_use = op.capabilities().item_use;
    let geometry = match op_type.geometry_requirement() {
        GeometryRequirement::Stock | GeometryRequirement::DrillTargets => CheckedGeometry::None,
        GeometryRequirement::Polygons => {
            CheckedGeometry::Polygons(require_polygons(ctx, op_type, item_use)?)
        }
        GeometryRequirement::Mesh => CheckedGeometry::Mesh(require_mesh(ctx, name)?),
        GeometryRequirement::Both => CheckedGeometry::Both {
            polygons: require_polygons(ctx, op_type, item_use)?,
            mesh: require_mesh(ctx, name)?,
        },
    };
    if let Some(text) = op_type.tool_refusal(ctx.tool_cfg.tool_type.cutter_kind()) {
        return Err(OperationError::InvalidTool(text));
    }
    Ok(Prepared {
        op_type,
        geometry,
        territory: Territory::resolve(ctx, op),
    })
}

/// CMP-13: the refusal names the operation. An empty list refuses as an
/// absent one.
///
/// P1: the op's [`ItemUse`] picks the polygons it gets (the match has no
/// wildcard, so a new use states its answer):
///
/// - `Curves`: every polygon, closed and open.
/// - `Regions`: the closed polygons only. The open paths left out go into
///   ONE [`SelectionNoteFinding`] through `ctx.record`. With no closed
///   polygon left, the op refuses with the same facts and the fix.
/// - `None` and `Holes`: no op reads polygons with these uses
///   (`every_op_states_its_capabilities_w2_4` holds the pairing), so the
///   text is the registry mismatch.
fn require_polygons<'a>(
    ctx: &ExecutionContext<'a>,
    op_type: OperationType,
    item_use: ItemUse,
) -> Result<Cow<'a, [Polygon2]>, OperationError> {
    let op_name = op_type.name();
    let polygons = ctx.polygons.filter(|p| !p.is_empty()).ok_or_else(|| {
        OperationError::MissingGeometry(format!("{op_name} requires 2D geometry"))
    })?;
    match item_use {
        ItemUse::Curves => Ok(Cow::Borrowed(polygons)),
        ItemUse::Regions => {
            let Some(note) = SelectionNoteFinding::open_paths_left_out(op_type.label(), polygons)
            else {
                return Ok(Cow::Borrowed(polygons));
            };
            let closed: Vec<Polygon2> = polygons.iter().filter(|p| p.closed).cloned().collect();
            if closed.is_empty() {
                return Err(OperationError::MissingGeometry(note.refusal()));
            }
            ctx.record(&note);
            Ok(Cow::Owned(closed))
        }
        ItemUse::None | ItemUse::Holes { .. } => Err(OperationError::Other(format!(
            "registry mismatch: {op_name} reads polygons, but its item use is {item_use:?}"
        ))),
    }
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
