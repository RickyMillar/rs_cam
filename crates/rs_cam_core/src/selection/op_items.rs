//! H1 (architecture review 2026-11-05): THE item pipeline of an op.
//!
//! [`op_items`] gives the items that an op cuts from its resolved
//! selection. It is the one body of these steps, in this order:
//!
//! 1. P6.4 (ruling O-2 (B)): an op that extends its open paths to the stock
//!    (`ItemUse::RegionsExtendOpen`) grows the stock rectangle by the tool
//!    radius ([`stock_growth`]).
//! 2. [`view`] with the `ItemUse` of the op.
//! 3. R-A: the "frees the part" note stays only when the op frees the part
//!    ([`drop_part_free_note`]).
//! 4. P6.5 (ruling O-3): THE hole fit step ([`fit_holes`]) at [`fit_size`].
//! 5. Chamfer Clearance: THE wall trim of the model edges
//!    ([`clear_edge_walls`]) when [`needs_edge_clear`] holds.
//! 6. "Cut past open sides": THE area rule ([`grow_past_open_sides`]) when
//!    [`open_sides_radius`] is set. It reads the stock as resolved, before
//!    step 1, so the stock grows by the tool radius once.
//!
//! F29: every `Empty` refusal of steps 2 to 6 of a Drill names the hole
//! rims that the default rule did not take
//! ([`SelectionRefusal::with_default_rims`]), so the fix text is the same
//! when `view` finds no hole and when no hole fits the tool.
//!
//! The prologue of generation, the selection status, the drill-op view and
//! the Pin Drill holes call this function. No reader builds a step itself.
//! The status memo key reads the step predicates below, so the key and the
//! pipeline read one rule.

use std::borrow::Cow;

use crate::compute::catalog::{ItemUse, OperationConfig};
use crate::compute::edge_clear::{EdgeClearProbe, clear_edge_walls};
use crate::compute::floor::Floor;
use crate::compute::open_sides::{OpenSidesProbe, OpenSidesReach, grow_past_open_sides};
use crate::compute::tool_config::ToolConfig;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};

use super::extend::StockXy;
use super::hole_fit::{DrillSize, HoleFitRule, fit_holes};
use super::refusal::{SelectionNote, SelectionRefusal};
use super::resolve::{ResolvedSelection, SetupFrame};
use super::view::{OpItems, drop_part_free_note, view};

/// The tool of an op: the built cutter and the tool config that it was
/// built from. The edge clear step reads the config (the tool kind and the
/// V angle); every other step reads the cutter diameter.
#[derive(Clone, Copy)]
pub struct OpTool<'a> {
    pub cutter: &'a ToolDefinition,
    pub config: &'a ToolConfig,
}

/// The model mesh of an op in the setup frame, and its spatial index. The
/// edge clear step reads both; the open sides step reads the mesh only.
#[derive(Clone, Copy)]
pub struct OpMesh<'a> {
    pub mesh: &'a TriangleMesh,
    /// `None` when the caller has no index. Then the edge clear step does
    /// nothing. A caller that builds the index on demand builds it only
    /// when [`needs_edge_clear`] holds.
    pub index: Option<&'a SpatialIndex>,
}

/// The items of an op, with their notes and the selection that they index.
pub struct OpItemsOutcome<'a> {
    pub items: OpItems,
    pub notes: Vec<SelectionNote>,
    /// The selection that the pipeline viewed: the resolved selection,
    /// with the stock grown by [`stock_growth`] when the op extends its
    /// open paths. The growth changes only the stock rectangle, so the
    /// items are the items of the resolved selection.
    pub resolved: Cow<'a, ResolvedSelection<SetupFrame>>,
}

/// THE item pipeline of `op` over `resolved` (the selection as resolved,
/// in the setup frame, with no growth). See the module doc for the steps.
///
/// With `tool` `None` (the toolpath has no valid tool) only steps 2 and 3
/// run: generation refuses the tool itself. With `mesh` `None` the edge
/// clear and the open sides steps do nothing.
pub fn op_items<'a>(
    resolved: &'a ResolvedSelection<SetupFrame>,
    op: &OperationConfig,
    tool: Option<OpTool<'_>>,
    mesh: Option<OpMesh<'_>>,
) -> Result<OpItemsOutcome<'a>, SelectionRefusal> {
    let item_use = op.capabilities().item_use;
    let diameter = tool.map(|t| t.cutter.diameter());
    // Step 6 reads the stock before the growth of step 1.
    let resolved_stock = resolved.stock_xy();
    let viewed: Cow<'a, ResolvedSelection<SetupFrame>> =
        match diameter.and_then(|d| stock_growth(op, d)) {
            Some(radius) => Cow::Owned(resolved.clone().with_stock_grown(radius)),
            None => Cow::Borrowed(resolved),
        };
    let size = diameter.and_then(|d| fit_size(op, d));
    let steps = Steps {
        op,
        item_use,
        tool,
        mesh,
        size,
        open_sides_radius: diameter.and_then(|d| open_sides_radius(op, d)),
        resolved_stock,
    };
    // F29: one rims rewrite over the refusal of any step.
    let (items, notes) = steps.run(&viewed).map_err(|refusal| match &size {
        Some(size) => refusal.with_default_rims(&viewed, item_use, size),
        None => refusal,
    })?;
    Ok(OpItemsOutcome {
        items,
        notes,
        resolved: viewed,
    })
}

/// The inputs of steps 2 to 6 of [`op_items`].
struct Steps<'o, 't> {
    op: &'o OperationConfig,
    item_use: ItemUse,
    tool: Option<OpTool<'t>>,
    mesh: Option<OpMesh<'t>>,
    size: Option<DrillSize>,
    open_sides_radius: Option<f64>,
    /// The stock as resolved, before the growth of step 1.
    resolved_stock: Option<StockXy>,
}

impl Steps<'_, '_> {
    /// Steps 2 to 6 over `viewed` (the selection after step 1).
    fn run(
        &self,
        viewed: &ResolvedSelection<SetupFrame>,
    ) -> Result<(OpItems, Vec<SelectionNote>), SelectionRefusal> {
        let (mut items, mut notes) = view(viewed, self.item_use)?;
        drop_part_free_note(&mut notes, self.op.frees_the_part());
        if let Some(size) = self.size {
            notes.extend(fit_holes(viewed, &mut items, self.item_use, size)?);
        }
        if let (
            Some(tool),
            Some(OpMesh {
                mesh,
                index: Some(index),
            }),
        ) = (self.tool, self.mesh)
            && let Some((chamfer_width, tip_offset)) = edge_clear_bevel(self.op, tool.config)
        {
            let probe = EdgeClearProbe {
                mesh,
                index,
                cutter: tool.cutter,
                half_angle_rad: (tool.config.included_angle / 2.0).to_radians(),
                chamfer_width,
                tip_offset,
            };
            notes.extend(clear_edge_walls(viewed, &mut items, &probe)?);
        }
        if let (Some(radius), Some(depth), Some(mesh), Some(stock)) = (
            self.open_sides_radius,
            open_sides_depth(self.op),
            self.mesh,
            self.resolved_stock,
        ) {
            let probe = OpenSidesProbe {
                mesh: mesh.mesh,
                reach: OpenSidesReach::Stock(stock),
                tool_radius: radius,
                face_depth: depth,
            };
            notes.extend(grow_past_open_sides(viewed, &mut items, &probe));
        }
        Ok((items, notes))
    }
}

/// Step 1: the radius by which the pipeline grows the stock of `op` with a
/// tool of `diameter`. Only an op that extends its open paths to the stock
/// (`ItemUse::RegionsExtendOpen`) grows it.
pub fn stock_growth(op: &OperationConfig, diameter: f64) -> Option<f64> {
    match op.capabilities().item_use {
        ItemUse::RegionsExtendOpen { .. } => Some(diameter / 2.0),
        ItemUse::None
        | ItemUse::Regions
        | ItemUse::Curves
        | ItemUse::Edges
        | ItemUse::Holes { .. } => None,
    }
}

/// Step 4: the size that the hole fit step of `op` reads with a tool of
/// `diameter` (`OperationConfig::drill_size`). `None` for an op with no
/// fit rule: the step then does nothing.
pub fn fit_size(op: &OperationConfig, diameter: f64) -> Option<DrillSize> {
    match op.capabilities().item_use {
        ItemUse::Holes {
            fit: HoleFitRule::ToolSize | HoleFitRule::LargerThanTool,
            ..
        } => Some(op.drill_size(diameter)),
        ItemUse::Holes {
            fit: HoleFitRule::None,
            ..
        }
        | ItemUse::None
        | ItemUse::Regions
        | ItemUse::RegionsExtendOpen { .. }
        | ItemUse::Curves
        | ItemUse::Edges => None,
    }
}

/// Step 5: the bevel (`w`, `t`) that the wall trim of `op` reads with the
/// tool `config`. `None` for an op with no bevel, and for a tool kind
/// that the op refuses (the tool check of generation refuses it after the
/// pipeline).
pub fn edge_clear_bevel(op: &OperationConfig, config: &ToolConfig) -> Option<(f64, f64)> {
    let bevel = op.bevel_width_and_tip_mm()?;
    op.op_type()
        .tool_refusal(config.tool_type.cutter_kind())
        .is_none()
        .then_some(bevel)
}

/// Step 5: true when the wall trim runs for `op` with `tool`. The trim
/// then needs the mesh and its spatial index.
pub fn needs_edge_clear(op: &OperationConfig, tool: Option<OpTool<'_>>) -> bool {
    tool.is_some_and(|t| edge_clear_bevel(op, t.config).is_some())
}

/// Step 6: the face depth of the floor "Each picked face" when `op` cuts
/// past open sides. `None` when the setting is off or under another
/// floor: the step then does nothing.
fn open_sides_depth(op: &OperationConfig) -> Option<f64> {
    if !op.cuts_past_open_sides() {
        return None;
    }
    if let Some(Floor::EachFace { depth }) = op.floor() {
        Some(depth)
    } else {
        None
    }
}

/// Step 6: the tool radius of the area rule of `op` with a tool of
/// `diameter`, or `None` when the step does nothing.
pub fn open_sides_radius(op: &OperationConfig, diameter: f64) -> Option<f64> {
    open_sides_depth(op).map(|_| diameter / 2.0)
}

/// True when [`op_items`] reads the mesh for `op` with `tool` (step 5 or
/// step 6). A caller that loads the mesh on demand loads it only then.
pub fn needs_mesh(op: &OperationConfig, tool: Option<OpTool<'_>>) -> bool {
    needs_edge_clear(op, tool)
        || tool.is_some_and(|t| open_sides_radius(op, t.cutter.diameter()).is_some())
}
