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
//! Geometry selection (P1, P2.3a): the op's selection reaches the prologue
//! as [`SelectionInputs`], resolved by the session. The prologue runs
//! [`view`] with the op's [`ItemUse`] and gives the adapter the items it
//! cuts: [`Prepared::regions`], [`Prepared::curves`] or
//! [`Prepared::holes`]. A `Regions` op gets closed areas only; the
//! prologue records a [`SelectionNoteFinding`] for the open paths that the
//! default rule left out. A selection that refuses stops the op with
//! [`OperationError::Selection`]. No op closes an open path with a chord
//! in silence.

use std::borrow::Cow;

use crate::compute::catalog::{GeometryRequirement, ItemUse, OperationConfig, OperationType};
use crate::compute::toolpath_stats::SelectionNoteFinding;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::selection::{Curves, Holes, OpItems, Regions, ResolvedSelection, view};

use super::territory::Territory;
use super::{ExecutionContext, OperationError};

/// A mesh and its spatial index, both present.
#[derive(Clone, Copy)]
pub struct MeshInput<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
}

/// The resolved selection of the op (geometry selection DESIGN 4.5).
///
/// C3 pattern (as `TerritoryInputs`): the field is private to this file,
/// so only the prologue reads it. An adapter reads the items through
/// [`Prepared`], after the check; the compiler refuses an adapter that
/// reads the resolved items around it.
///
/// The session resolves the selection in the setup frame
/// (`resolve_generation_inputs`) and lends it here. The default is an
/// empty selection: a caller that resolves no model gives the op nothing
/// to cut, and an op that needs items then refuses with `Empty`.
pub struct SelectionInputs<'a> {
    resolved: Cow<'a, ResolvedSelection>,
}

impl Default for SelectionInputs<'_> {
    fn default() -> Self {
        Self {
            resolved: Cow::Owned(ResolvedSelection::from_polygons(Vec::new())),
        }
    }
}

impl<'a> SelectionInputs<'a> {
    /// The selection the session resolved, in the emission frame.
    pub(crate) fn resolved(resolved: &'a ResolvedSelection) -> Self {
        Self {
            resolved: Cow::Borrowed(resolved),
        }
    }

    /// Test only: the default rule over a hand-built polygon list
    /// ([`ResolvedSelection::from_polygons`]). Every polygon gives its
    /// items, and [`view`] nests the rings as it nests a drawing, so a
    /// test drawing takes the same door as a session drawing.
    #[cfg(test)]
    pub(crate) fn from_polygons(polygons: &[crate::polygon::Polygon2]) -> SelectionInputs<'static> {
        SelectionInputs {
            resolved: Cow::Owned(ResolvedSelection::from_polygons(polygons.to_vec())),
        }
    }
}

/// The geometry the prologue checked, by the row's [`GeometryRequirement`].
enum CheckedGeometry<'a> {
    /// `Stock`: no model geometry to check.
    None,
    Items(OpItems),
    Mesh(MeshInput<'a>),
    ItemsAndMesh {
        items: OpItems,
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
    /// The checked items of an `Items` or `ItemsAndMesh` row.
    fn items(&self) -> Option<&OpItems> {
        match &self.geometry {
            CheckedGeometry::Items(items) | CheckedGeometry::ItemsAndMesh { items, .. } => {
                Some(items)
            }
            CheckedGeometry::None | CheckedGeometry::Mesh(_) => None,
        }
    }

    /// The closed areas of a `Regions` op, nested. Never empty.
    pub fn regions(&self) -> Result<&Regions, OperationError> {
        self.items()
            .and_then(OpItems::regions)
            .ok_or_else(|| self.mismatch("regions"))
    }

    /// The lines of a `Curves` op (closed and open), or the closed areas
    /// of a `Regions` op as lines. Never empty.
    pub fn curves(&self) -> Result<&Curves, OperationError> {
        self.items()
            .and_then(OpItems::curves)
            .ok_or_else(|| self.mismatch("curves"))
    }

    /// The hole positions of a `Holes` op. P2.3c moves the drill adapters
    /// onto it; until then they read `ExecutionContext::drill_targets`.
    pub fn holes(&self) -> Result<&Holes, OperationError> {
        self.items()
            .and_then(OpItems::holes)
            .ok_or_else(|| self.mismatch("holes"))
    }

    /// The mesh and its spatial index.
    pub fn mesh(&self) -> Result<MeshInput<'a>, OperationError> {
        match &self.geometry {
            CheckedGeometry::Mesh(mesh) | CheckedGeometry::ItemsAndMesh { mesh, .. } => Ok(*mesh),
            CheckedGeometry::None | CheckedGeometry::Items(_) => Err(self.mismatch("a mesh")),
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
        GeometryRequirement::Stock => CheckedGeometry::None,
        GeometryRequirement::Items => {
            CheckedGeometry::Items(require_items(ctx, op_type, item_use)?)
        }
        GeometryRequirement::Mesh => CheckedGeometry::Mesh(require_mesh(ctx, name)?),
        GeometryRequirement::ItemsAndMesh => CheckedGeometry::ItemsAndMesh {
            items: require_items(ctx, op_type, item_use)?,
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

/// The items the op cuts: [`view`] over the resolved selection with the
/// op's [`ItemUse`]. A refusal names the op (CMP-13) and stops the op with
/// [`OperationError::Selection`]. The open paths that the default rule
/// left out go into ONE [`SelectionNoteFinding`] through `ctx.record`.
///
/// P2.3a, the drill families: the view of a `Holes` op is taken as one
/// that may be empty. The drill adapters still read
/// `ExecutionContext::drill_targets` and refuse on their own count
/// (`drill_targets_refusal`) until P2.3c moves them onto
/// [`Prepared::holes`]; an `Empty` refusal here would come first and
/// break that refusal and the stale-pick rule.
fn require_items(
    ctx: &ExecutionContext<'_>,
    op_type: OperationType,
    item_use: ItemUse,
) -> Result<OpItems, OperationError> {
    let view_use = match item_use {
        ItemUse::Holes { .. } => ItemUse::Holes { may_be_empty: true },
        ItemUse::None | ItemUse::Regions | ItemUse::Curves => item_use,
    };
    let (items, notes) = view(&ctx.selection.resolved, view_use)
        .map_err(|refusal| OperationError::Selection(refusal.for_op(op_type.label())))?;
    if !notes.is_empty() {
        ctx.record(&SelectionNoteFinding {
            op: op_type.label(),
            notes,
        });
    }
    Ok(items)
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
