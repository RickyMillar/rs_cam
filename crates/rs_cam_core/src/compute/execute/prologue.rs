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
//!
//! Geometry selection P3.5: under the floor "each picked face" the prologue
//! runs THE per-area floor rule ([`region_floors`]) after the view, and
//! gives each closed area its own ladder: the catalogue ladder
//! (`OperationConfig::cutting_levels`) down to the floor of that area
//! ([`Prepared::region_ladders`]). An area with no face refuses with
//! [`OperationError::Selection`], as `selection_status` refuses it. P3.7: a
//! face whose floor is at or above the top is left out with a note; the op
//! refuses `FloorAboveTop` only when every face is at the top.
//!
//! Geometry selection P6.5 (ruling O-3): after the view the prologue runs
//! THE hole fit step ([`fit_holes`]) with the tool diameter and the size
//! tolerance of the op (the Drill `size_tolerance`). A Drill leaves
//! out a circle that is not of the tool size, with a note, and refuses a
//! picked one (`HoleDoesNotFit`). [`Prepared::holes`] gives the fitted
//! holes only, so no adapter reads a hole the tool does not make.

use std::borrow::Cow;

use crate::compute::catalog::{GeometryRequirement, ItemUse, OperationConfig, OperationType};
use crate::compute::config::ResolvedHeights;
use crate::compute::floor::region_floors;
use crate::compute::toolpath_stats::SelectionNoteFinding;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::selection::{
    Curves, Holes, OpItems, Regions, ResolvedSelection, SelectionNote, SetupFrame, fit_holes, view,
};
use crate::tool::MillingCutter;

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
pub(crate) struct SelectionInputs<'a> {
    resolved: Cow<'a, ResolvedSelection<SetupFrame>>,
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
    pub(crate) fn resolved(resolved: &'a ResolvedSelection<SetupFrame>) -> Self {
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

    /// Test only: the stock rule over `stock_bbox` (the Face default,
    /// geometry selection P4.2): one stock loop, as the session resolves
    /// `Selection::stock()`.
    #[cfg(test)]
    pub(crate) fn stock(stock_bbox: &crate::geo::BoundingBox3) -> SelectionInputs<'static> {
        let items: Vec<crate::selection::Item> = crate::selection::stock_items(stock_bbox)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        SelectionInputs {
            resolved: Cow::Owned(ResolvedSelection::from_items(items)),
        }
    }

    /// Test only: the default rule over hand-built items
    /// ([`ResolvedSelection::from_items`]), for an op that reads points or
    /// circles (the drill families).
    #[cfg(test)]
    pub(crate) fn from_items(
        items: impl IntoIterator<Item = crate::selection::Item>,
    ) -> SelectionInputs<'static> {
        SelectionInputs {
            resolved: Cow::Owned(ResolvedSelection::from_items(items)),
        }
    }
}

/// The geometry the prologue checked, by the row's [`GeometryRequirement`].
enum CheckedGeometry<'a> {
    Items(OpItems),
    Mesh(MeshInput<'a>),
    ItemsAndMesh { items: OpItems, mesh: MeshInput<'a> },
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
    /// P3.5: the ladder of each closed area under the floor "each picked
    /// face", in the order of [`Prepared::regions`]. `None` for every
    /// other floor.
    region_ladders: Option<Vec<Vec<f64>>>,
}

impl<'a> Prepared<'a> {
    /// The checked items of an `Items` or `ItemsAndMesh` row.
    fn items(&self) -> Option<&OpItems> {
        match &self.geometry {
            CheckedGeometry::Items(items) | CheckedGeometry::ItemsAndMesh { items, .. } => {
                Some(items)
            }
            CheckedGeometry::Mesh(_) => None,
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

    /// The hole positions of a `Holes` op, in the emission frame, after the
    /// hole fit step (P6.5: a Drill gets the points and the circles of the
    /// tool size only). Empty only for an op that may be empty (Pin Drill).
    pub fn holes(&self) -> Result<&Holes, OperationError> {
        self.items()
            .and_then(OpItems::holes)
            .ok_or_else(|| self.mismatch("holes"))
    }

    /// The mesh and its spatial index.
    pub fn mesh(&self) -> Result<MeshInput<'a>, OperationError> {
        match &self.geometry {
            CheckedGeometry::Mesh(mesh) | CheckedGeometry::ItemsAndMesh { mesh, .. } => Ok(*mesh),
            CheckedGeometry::Items(_) => Err(self.mismatch("a mesh")),
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

    /// Geometry selection P3.5: under the floor "each picked face", the Z
    /// ladder of each closed area, top first, in the order of
    /// [`Self::regions`]. Each is the catalogue ladder of the op
    /// (`OperationConfig::cutting_levels`, with its spring passes) from
    /// the one Top Z to the floor of that area. `None` for every other
    /// floor: the op steps the one ladder of `ExecutionContext::cutting_levels`.
    pub(crate) fn region_ladders(&self) -> Option<&[Vec<f64>]> {
        self.region_ladders.as_deref()
    }

    fn mismatch(&self, what: &str) -> OperationError {
        OperationError::Internal(format!(
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
    let mut notes = Vec::new();
    let mut geometry = match op_type.geometry_requirement() {
        GeometryRequirement::Items => {
            CheckedGeometry::Items(require_items(ctx, op, item_use, &mut notes)?)
        }
        GeometryRequirement::Mesh => CheckedGeometry::Mesh(require_mesh(ctx, name)?),
        GeometryRequirement::ItemsAndMesh => CheckedGeometry::ItemsAndMesh {
            items: require_items(ctx, op, item_use, &mut notes)?,
            mesh: require_mesh(ctx, name)?,
        },
    };
    let region_ladders = region_ladders(ctx, op, &mut geometry, &mut notes)?;
    // ONE finding holds every note of the op: the open paths that `view`
    // left out and the faces at the top (P3.7). The finding overwrites.
    if !notes.is_empty() {
        ctx.record(&SelectionNoteFinding {
            op: op_type.label(),
            notes,
        });
    }
    if let Some(text) = op_type.tool_refusal(ctx.tool_cfg.tool_type.cutter_kind()) {
        return Err(OperationError::InvalidTool(text));
    }
    Ok(Prepared {
        op_type,
        geometry,
        territory: Territory::resolve(ctx, op),
        region_ladders,
    })
}

/// P3.5: the ladder of each closed area under the floor "each picked
/// face" (see [`Prepared::region_ladders`]), or `None`. THE per-area floor
/// rule ([`region_floors`]) refuses an area with no face, and refuses when
/// every face floor is at or above the top, by item. P3.7: it removes a
/// face at the top from the items and adds its note to `notes`.
fn region_ladders(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
    geometry: &mut CheckedGeometry<'_>,
    notes: &mut Vec<SelectionNote>,
) -> Result<Option<Vec<Vec<f64>>>, OperationError> {
    let items = match geometry {
        CheckedGeometry::Items(items) | CheckedGeometry::ItemsAndMesh { items, .. } => items,
        CheckedGeometry::Mesh(_) => return Ok(None),
    };
    let Some(floor) = op.floor() else {
        return Ok(None);
    };
    let floors = region_floors(&ctx.selection.resolved, items, floor, ctx.heights.top_z).map_err(
        |refusal| OperationError::Selection(Box::new(refusal.for_op(op.op_type().label()))),
    )?;
    Ok(floors.map(|floors| {
        let ladders = floors
            .as_slice()
            .iter()
            .map(|&bottom_z| {
                op.cutting_levels(&ResolvedHeights {
                    bottom_z,
                    ..*ctx.heights
                })
            })
            .collect();
        notes.extend(floors.into_notes());
        ladders
    }))
}

/// The items the op cuts: [`view`] over the resolved selection with the
/// op's [`ItemUse`]. A refusal names the op (CMP-13) and stops the op with
/// [`OperationError::Selection`]. The open paths that the default rule
/// left out go into `notes`; [`check_inputs`] records them in ONE
/// [`SelectionNoteFinding`] through `ctx.record`.
///
/// A `Holes` op takes its own `may_be_empty` (P2.3c): a Drill with no hole
/// refuses `Empty`; a Pin Drill with no extra hole runs on its stock pins.
/// P6.5: then THE hole fit step ([`fit_holes`]) with the tool diameter and
/// the size tolerance of the op ([`OperationConfig::drill_size`]); its
/// notes join `notes`, and its refusal stops the op as a view refusal does.
fn require_items(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
    item_use: ItemUse,
    notes: &mut Vec<SelectionNote>,
) -> Result<OpItems, OperationError> {
    let op_type = op.op_type();
    let selection_error = |refusal: crate::selection::SelectionRefusal| -> OperationError {
        OperationError::Selection(Box::new(refusal.for_op(op_type.label())))
    };
    let (mut items, view_notes) =
        view(&ctx.selection.resolved, item_use).map_err(selection_error)?;
    notes.extend(view_notes);
    let fit_notes = fit_holes(
        &ctx.selection.resolved,
        &mut items,
        item_use,
        op.drill_size(ctx.tool_def.diameter()),
    )
    .map_err(selection_error)?;
    notes.extend(fit_notes);
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
        .ok_or_else(|| OperationError::Internal(format!("{op_name} requires a spatial index")))?;
    Ok(MeshInput { mesh, index })
}
