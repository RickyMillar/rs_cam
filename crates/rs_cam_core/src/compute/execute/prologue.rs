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
//!
//! UX-1c (operator ruling 2026-10-22, SAFETY): last, a one-depth op
//! (`FaceTopRule::RefuseBuried`) on the remaining stock refuses when
//! material stands above its face top over its area
//! ([`OperationError::BuriedFace`]).
//!
//! Chamfer Clearance (operator ruling 2026-10-23): after the view and the
//! hole fit, the prologue runs THE wall trim of the model edges of an
//! `Edges` op ([`clear_edge_walls`]). An edge that ends at a higher wall
//! stops short of it; an edge with no clear stretch is left out with a
//! note. The buried check and the adapter read the trimmed runs.
//!
//! Geometry selection P6.4 (ruling O-2 (B)): for an op that extends its open
//! paths to the stock (`ItemUse::RegionsExtendOpen`, Pocket with Extend to
//! stock) the prologue grows the stock rectangle of the selection by the
//! tool radius ([`ResolvedSelection::with_stock_grown`]) before [`view`],
//! so the area ends one tool radius past the stock edge and the tool centre
//! reaches the edge. The `selection_status` report grows it by the same
//! radius.

use std::borrow::Cow;

use crate::compute::catalog::{
    FaceTopRule, GeometryRequirement, ItemUse, OperationConfig, OperationType,
};
use crate::compute::config::ResolvedHeights;
use crate::compute::edge_clear::{EdgeClearProbe, clear_edge_walls};
use crate::compute::floor::{HoleFloorInputs, HoleSpan, hole_floors, region_floors};
use crate::compute::toolpath_stats::SelectionNoteFinding;
use crate::dexel_stock::FRESH_MATERIAL_THRESHOLD_MM;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::selection::{
    Curves, Holes, OpItems, Regions, ResolvedSelection, SelectionNote, SetupFrame, fit_holes, view,
};
use crate::tool::MillingCutter;

use super::standing_material::{ColumnTop, standing_top_in_band, standing_top_in_footprint};
use super::territory::Territory;
use super::{BuriedFaceRefusal, BuriedUnder, ExecutionContext, OperationError};

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
    /// R-B, F19: the depth base and the floor of each hole of a Drill or a
    /// Bore, in the order of [`Prepared::holes`]
    /// ([`crate::compute::floor::hole_floors`]). `None` for an op with no
    /// floor or no holes.
    hole_spans: Option<Vec<HoleSpan>>,
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

    /// The items of an `Edges` op (R-C): the drawing areas and the convex
    /// top model edges. Never both empty.
    pub fn edges(&self) -> Result<&crate::selection::Edges, OperationError> {
        self.items()
            .and_then(OpItems::edges)
            .ok_or_else(|| self.mismatch("edges"))
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

    /// R-B, F19: the depth base and the floor of each hole, in the order of
    /// [`Self::holes`]. `None` for an op with no floor (the Pin Drill).
    pub(crate) fn hole_spans(&self) -> Option<&[HoleSpan]> {
        self.hole_spans.as_deref()
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
    // S11: a value with no derivation that is not set refuses before any
    // geometry work: the op cannot cut with it.
    if let Some(not_set) = op.not_set() {
        return Err(OperationError::NotSet(not_set));
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
    let hole_spans = hole_spans(ctx, op, &geometry)?;
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
    let prepared = Prepared {
        op_type,
        geometry,
        territory: Territory::resolve(ctx, op),
        region_ladders,
        hole_spans,
    };
    refuse_buried_face(ctx, op, &prepared)?;
    Ok(prepared)
}

/// UX-1c (operator ruling 2026-10-22, SAFETY): a one-depth op
/// (`FaceTopRule::RefuseBuried`: V-Carve, Inlay, Inlay Plug; `RefuseBuriedEach`: Chamfer, per item) on
/// the remaining stock refuses when material stands above its top over its
/// area ([`standing_top_in_footprint`] with the tool radius). Its auto top
/// under a face pick is the face top, and it cuts one depth from there, so
/// standing material above the face buries the tool.
///
/// - Fresh stock (no remaining stock here): the session refuses a face
///   below the stock top before the job starts
///   (`ToolpathHeights::buried_face_refusal`); this check does nothing.
/// - A pinned top is the operator's statement of where the op starts, so
///   it does not refuse here.
/// - With no auto face top the top is the stock top, and no material
///   stands above it.
/// - Material counts above `top_z + FRESH_MATERIAL_THRESHOLD_MM`, the floor
///   below which the simulator itself calls material absent. No evidence
///   (no footprint cell on the grid) does not refuse.
fn refuse_buried_face(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
    prepared: &Prepared<'_>,
) -> Result<(), OperationError> {
    let each = match op.capabilities().face_top {
        FaceTopRule::RefuseBuried => false,
        FaceTopRule::RefuseBuriedEach => true,
        FaceTopRule::HigherOfFaceAndStock | FaceTopRule::Face => return Ok(()),
    };
    if ctx.heights.top_pinned {
        return Ok(());
    }
    let Some(stock) = prepared.territory().unguarded_stock() else {
        return Ok(());
    };
    let Some(items) = prepared.items() else {
        return Ok(());
    };
    let Some(regions) = items.regions() else {
        return Ok(());
    };
    let top_z = ctx.heights.top_z;
    let tool_radius = ctx.tool_def.diameter() / 2.0;
    let refusal = |material: f64, top: f64| {
        OperationError::BuriedFace(BuriedFaceRefusal {
            op: op.op_type().label(),
            buried_mm: material - top,
            under: BuriedUnder::StandingMaterial,
        })
    };
    let buried = |column: ColumnTop, top: f64| match column {
        ColumnTop::Top(material) if material > top + FRESH_MATERIAL_THRESHOLD_MM => {
            Err(refusal(material, top))
        }
        ColumnTop::Top(_) | ColumnTop::Empty | ColumnTop::NoEvidence => Ok(()),
    };
    if !each {
        return buried(
            standing_top_in_footprint(stock, regions.as_slice(), tool_radius),
            top_z,
        );
    }
    // R-C / F21: each area against its own top, and each model edge along
    // a band of the cone radius of the bevel on both sides of the edge.
    // The edges are the runs after the wall trim (`clear_edges`), so the
    // band of a run that ends at a higher wall does not reach that wall.
    for (i, outline) in regions.as_slice().iter().enumerate() {
        let own_top = regions.plane_z(i).unwrap_or(top_z);
        buried(
            standing_top_in_footprint(stock, std::slice::from_ref(outline), tool_radius),
            own_top,
        )?;
    }
    let reach = op.bevel_reach_mm().unwrap_or(tool_radius);
    for line in items
        .edges()
        .map_or(&[][..], crate::selection::Edges::lines)
    {
        let (rings, paths): (&[Vec<crate::geo::P2>], &[Vec<crate::geo::P2>]) = if line.line.closed {
            (std::slice::from_ref(&line.line.exterior), &[])
        } else {
            (&[], std::slice::from_ref(&line.line.exterior))
        };
        buried(standing_top_in_band(stock, rings, paths, reach), line.z)?;
    }
    Ok(())
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
    let floors = region_floors(&ctx.selection.resolved, items, &floor, ctx.heights.top_z).map_err(
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

/// R-B, F19: THE per-hole floor rule ([`hole_floors`]) over the fitted
/// holes of a Drill or a Bore, on the resolved heights of the op.
/// `selection_status` runs the same rule on the same heights, so the
/// status and generation give one answer. `None` for an op with no floor
/// or no holes.
fn hole_spans(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
    geometry: &CheckedGeometry<'_>,
) -> Result<Option<Vec<HoleSpan>>, OperationError> {
    let items = match geometry {
        CheckedGeometry::Items(items) | CheckedGeometry::ItemsAndMesh { items, .. } => items,
        CheckedGeometry::Mesh(_) => return Ok(None),
    };
    let (Some(floor), Some(holes)) = (op.floor(), items.holes()) else {
        return Ok(None);
    };
    let inputs = HoleFloorInputs::new(ctx.heights, ctx.stock_bbox.min.z);
    hole_floors(&ctx.selection.resolved, holes, &floor, &inputs)
        .map(Some)
        .map_err(|refusal| {
            OperationError::Selection(Box::new(refusal.for_op(op.op_type().label())))
        })
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
    // P6.4 (O-2 (B)): the extend reader views the stock grown by the tool
    // radius. Every other reader views the selection as resolved.
    let grown;
    let resolved: &ResolvedSelection<SetupFrame> = match item_use {
        ItemUse::RegionsExtendOpen { .. } => {
            grown = ctx
                .selection
                .resolved
                .as_ref()
                .clone()
                .with_stock_grown(ctx.tool_def.diameter() / 2.0);
            &grown
        }
        ItemUse::None
        | ItemUse::Regions
        | ItemUse::Curves
        | ItemUse::Edges
        | ItemUse::Holes { .. } => &ctx.selection.resolved,
    };
    let size = op.drill_size(ctx.tool_def.diameter());
    // F29: an `Empty` Drill names the hole rims that the default rule did
    // not take.
    let (mut items, view_notes) = view(resolved, item_use)
        .map_err(|refusal| selection_error(refusal.with_default_rims(resolved, item_use, &size)))?;
    notes.extend(view_notes);
    let fit_notes = fit_holes(resolved, &mut items, item_use, size).map_err(selection_error)?;
    notes.extend(fit_notes);
    let clear_notes = clear_edges(ctx, op, resolved, &mut items).map_err(selection_error)?;
    notes.extend(clear_notes);
    Ok(items)
}

/// Chamfer Clearance (operator ruling 2026-10-23): THE wall trim
/// ([`clear_edge_walls`]) of the model edges of an `Edges` op, with the
/// mesh of the op, its V-bit and its bevel. Each edge run stops short of a
/// higher wall; [`refuse_buried_face`] and the adapter then read the
/// trimmed runs. With no mesh, no bevel or a tool that the op refuses,
/// the trim does nothing (the tool check refuses after it).
fn clear_edges(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
    resolved: &ResolvedSelection<SetupFrame>,
    items: &mut OpItems,
) -> Result<Vec<SelectionNote>, crate::selection::SelectionRefusal> {
    let (Some(mesh), Some(index), Some((chamfer_width, tip_offset))) =
        (ctx.mesh, ctx.index, op.bevel_width_and_tip_mm())
    else {
        return Ok(Vec::new());
    };
    if op
        .op_type()
        .tool_refusal(ctx.tool_cfg.tool_type.cutter_kind())
        .is_some()
    {
        return Ok(Vec::new());
    }
    let probe = EdgeClearProbe {
        mesh,
        index,
        cutter: ctx.tool_def,
        half_angle_rad: (ctx.tool_cfg.included_angle / 2.0).to_radians(),
        chamfer_width,
        tip_offset,
    };
    clear_edge_walls(resolved, items, &probe)
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
