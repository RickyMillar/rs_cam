//! The selection of a toolpath, resolved without a kernel: the
//! `SelectionStatus` query (geometry selection DESIGN 5.4, P2.4).
//!
//! Generation and every reader that does not generate read ONE
//! resolution: [`ProjectSession::resolve_selection_frames`] (the op's own
//! selection, plus its picked faces as picks, resolved against the models
//! and the stock, then moved into the setup frame through THE frame door).
//! Generation hands the setup-frame half to the prologue, which runs
//! [`crate::selection::view`]. [`ProjectSession::selection_status`] runs
//! the same `view` with the same `ItemUse` on the same half. So the
//! Generate button, the diagnostics ribbon, the drill picker and the
//! generator cannot give different refusals for one selection.
//!
//! The drill picker reads the MODEL frame: a stored pick is a key in the
//! model frame, and the dowel-hole check measures a drawing hole against
//! the stock pins in the stock-local frame of the part. The report keeps
//! those lists in the model frame ([`SelectionReport::hole_items`],
//! [`SelectionReport::holes`]). The viewport moves each item into the
//! setup frame to draw it and to hit-test a click (P2.6).
//!
//! The memo: the GUI asks for the report on every frame (the drill panel,
//! the viewport markers, the validator). The memo keeps one report per
//! toolpath and the inputs it read, compared by value or by allocation
//! identity, as the tool-load memo does (`load_report.rs`). No key part is
//! a counter, so no mutation path can forget to move it:
//!
//! - the target (P4.1: the op selection or the boundary selection), the
//!   selection of that target (serialized), the op type, the `ItemUse` that
//!   reads the target and the model id of the toolpath;
//! - the item set of each model, by `Weak` identity (a re-import gives a
//!   new `Arc`), and its id;
//! - P4.7: when the selection names a face, the cached flat-region set of
//!   each mesh model, by `Weak` identity, or its absence. When the GUI job
//!   fills the cache, the key moves, so "computing faces" builds again
//!   once;
//! - the setup orientation and the stock box in both frames (the face
//!   filter and the frame door read them).
//!
//! - P6.5: for a Drill (`HoleFitRule::ToolSize`) the tool diameter, by
//!   value, because the hole fit step reads it. A tool edit then builds
//!   the report of the Drill again, and of no other op.
//! - P6.4: for a Pocket with Extend to stock (`ItemUse::RegionsExtendOpen`)
//!   the tool radius, by value, because the stock grows by it before
//!   `view`. A tool edit then builds the report again.
//! - "Cut past open sides" (operator ruling 2026-10-23): the tool radius,
//!   by value, when the setting is on, because THE area rule grows the
//!   stock by it.
//!
//! The key does not hold the `holes` flag of a boundary: the report does
//! not read it (the holes of the union are a generation step after `view`).
//! The key does not hold `generation_inputs_signature`: that signature
//! also holds the feeds, the dressups, the heights and the tool, which the
//! selection does not read, so an edit of those would build again.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::compute::catalog::{GeometryRequirement, ItemUse, OperationType, OutlineUse};
use crate::compute::config::{BoundaryConfig, BoundarySource};
use crate::compute::floor::Floor;
use crate::compute::transform::{FaceUp, ZRotation};
use crate::geo::{BoundingBox3, P2, P3};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::{ModelId, ToolpathId};
use crate::selection::hole_fit::DrillSize;
use crate::selection::{
    FLAT_REGION_Z_BAND_MM, FacesByFacing, Hole, HoleDepth, HoleFit, HoleFitRule, ItemError,
    ItemKind, ItemRef, ItemRole, ItemSet, MatchKey, MatchedBy, MeshFaces, ModelFrame, OpItems,
    RefusalCause, ResolveScope, ResolvedSelection, Selection, SelectionEdit, SelectionNote,
    SelectionRefusal, SetupFrame, Shape, SourceRef, fit_holes, hole_fit,
};
use crate::session::{LoadedModel, ProjectSession, SessionError, ToolpathConfig};

use super::SetupEvalContext;

/// Which selection of a toolpath a command or a read names (geometry
/// selection P4.1). A toolpath holds up to two selections: the selection of
/// its op (`OperationConfig::selection`) and the selection of its machining
/// boundary (`BoundarySource::Selection`). DESIGN 6: a 3D op uses a
/// selection only through its boundary.
///
/// The wire spelling is the file spelling: `"op"` or `"boundary"`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SelectionTarget {
    /// The selection of the op: what the op cuts.
    #[default]
    Op,
    /// The selection inside `BoundarySource::Selection`: where the tool may
    /// go.
    Boundary,
}

impl SelectionTarget {
    /// The selection that a viewport face click on `tc` edits: the op
    /// selection when the op holds one, else the boundary selection (an op
    /// with no selection slot: every 3D op).
    pub fn for_face_pick(tc: &ToolpathConfig) -> Self {
        match tc.operation.selection() {
            Some(_) => Self::Op,
            None => Self::Boundary,
        }
    }
}

/// May a resolution build the flat-region faces of a mesh model (geometry
/// selection P4.7)?
///
/// The WP11b rule: a read on the frame loop builds nothing. The
/// `selection_status` memo, the tab layout and the diagnostics `Peek`; a
/// cold cache then gives "computing faces" (`faces_not_ready`). Generation
/// `Build`s before it resolves, so a generated toolpath does not depend on
/// whether the GUI job ran first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaceAccess {
    /// Read the cache; do not build.
    Peek,
    /// Build on a cold cache.
    Build,
}

/// The `ItemUse` that reads a boundary selection: closed regions
/// (`resolve_boundary_selection_polys` and the boundary report). The GUI
/// selection section reads it for the kinds a boundary rule can name.
pub const BOUNDARY_ITEM_USE: ItemUse = ItemUse::Regions;

/// R-A: what a boundary selection does with the part outline of a mesh
/// model: its default does not take it, and a pick may name it (the
/// outline of the part is a valid machining boundary).
pub const BOUNDARY_OUTLINE_USE: OutlineUse = OutlineUse::PickOnly;

/// The item use and the outline use (R-A) of the reader of the selection
/// `target` of `tc`: the op, or the boundary.
pub(crate) fn target_uses(tc: &ToolpathConfig, target: SelectionTarget) -> (ItemUse, OutlineUse) {
    match target {
        SelectionTarget::Op => (
            tc.operation.capabilities().item_use,
            tc.operation.outline_use(),
        ),
        SelectionTarget::Boundary => (BOUNDARY_ITEM_USE, BOUNDARY_OUTLINE_USE),
    }
}

/// The name that a refusal of a boundary selection gives as its op.
pub(crate) const BOUNDARY_LABEL: &str = "Machining boundary";

/// The refusal of a boundary edit or read when the boundary source of
/// `name` is not a selection. No command changes the source in silence.
pub(crate) fn boundary_not_a_selection(name: &str) -> SessionError {
    SessionError::InvalidParam(format!(
        "the boundary of '{name}' is not a selection: set the boundary source to Selection"
    ))
}

/// What the selection does to one item (geometry selection P4.4). The
/// viewport colours each item by its state; MCP `selection_status` gives
/// the same states.
///
/// The wire spelling is snake case: `"cut"`, `"island"`, `"skipped"`,
/// `"excluded"`, `"not_selected"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// A served item that the op cuts: an outer region, a curve, a hole, a
    /// face.
    Cut,
    /// A served closed ring that the nesting makes a hole of a region (an
    /// inner ring). The op cuts round it.
    Island,
    /// The selection holds the item and the op does not cut it: the default
    /// rule matched a kind that the op cannot cut, or the selection refuses
    /// (then no item is `Cut`).
    Skipped,
    /// A rule matched the item and an exclude removed it.
    Excluded,
    /// No rule and no pick matches the item.
    NotSelected,
}

/// What one viewport click does to a toolpath
/// ([`ProjectSession::click_edit`], geometry selection P4.D).
#[derive(Debug, Clone, PartialEq)]
pub enum ClickEdit {
    /// An edit of the selection that the click names
    /// ([`ProjectSession::pick_edit`]). Apply it with
    /// `Command::EditSelection`.
    Selection(SelectionEdit),
    /// Ruling L-8 revised: the boundary source was not `Selection`. The
    /// new boundary has the source `Selection` with the one pick; every
    /// other field is as stored. Apply it with ONE
    /// `Command::SetBoundaryConfig`, so it is one undo step.
    BoundaryToSelection {
        boundary: Box<BoundaryConfig>,
        /// The kind of the picked item, for the notice.
        kind: ItemKind,
    },
}

impl ClickEdit {
    /// The notice that tells the user what the click did, or `None` for a
    /// plain selection edit: "Boundary set to Selection: 1 face."
    pub fn notice(&self) -> Option<String> {
        match self {
            ClickEdit::Selection(_) => None,
            ClickEdit::BoundaryToSelection { kind, .. } => Some(format!(
                "Boundary set to Selection: 1 {}.",
                crate::selection::kind_noun(*kind, 1)
            )),
        }
    }
}

/// One point or circle of a toolpath model, for the drill picker.
#[derive(Debug, Clone, PartialEq)]
pub struct HoleItem {
    /// The key a pick of this item stores (source: the toolpath model).
    pub key: MatchKey,
    /// The point, or the circle centre, in the MODEL frame.
    pub xy: P2,
    /// The circle diameter; `None` for a point.
    pub diameter: Option<f64>,
    pub layer: Option<String>,
    /// True when the selection of the op holds this item.
    pub selected: bool,
}

/// The holes of a drill op, as generation resolves them, in the MODEL
/// frame. A projection of the [`SelectionReport`] of the op
/// ([`ProjectSession::hole_selection`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HoleSelection {
    /// Every point and circle of the toolpath model, in import order.
    pub items: Vec<HoleItem>,
    /// The holes the op drills from its selection (`view`), in selection
    /// order. A Pin Drill also drills its stock pins, which are not here.
    pub holes: Vec<Hole>,
    /// The count of the stored picks that name no item, or more than one.
    pub lost_picks: usize,
    /// The refusal that generation gives for the selection, with the op
    /// name, or `None` when the selection gives the op its holes.
    pub refusal: Option<String>,
}

impl HoleSelection {
    /// The layers of [`Self::items`], sorted, each once.
    pub fn layers(&self) -> Vec<String> {
        let mut layers: Vec<String> = self.items.iter().filter_map(|i| i.layer.clone()).collect();
        layers.sort();
        layers.dedup();
        layers
    }

    /// The count of [`Self::items`] that the selection holds.
    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }
}

/// What the selection of a toolpath resolves to, with no kernel (DESIGN
/// 5.4): the answer of the `selection_status` query.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionReport {
    /// The items of the selection that the op cuts, by kind ("14 loops,
    /// 2 circles").
    pub counts: BTreeMap<ItemKind, usize>,
    /// The faces of the selection that the reader uses, by facing in the
    /// SETUP frame (geometry selection P3.4): up and down. `counts` holds
    /// the total of the faces; an op refuses a down face
    /// (`FaceFacesDown`), and a boundary keeps it.
    pub faces_by_facing: FacesByFacing,
    /// The islands of the closed areas that the op cuts (the inner rings
    /// after containment). 0 for an op that cuts no closed area, and when
    /// the selection refuses.
    pub islands: usize,
    /// The items that the default rule matched and that the op leaves out,
    /// by kind (`OpItems::skipped_by_kind`).
    pub skipped_by_kind: BTreeMap<ItemKind, usize>,
    /// The notes that generation records for the open paths the op leaves
    /// out (`SelectionNoteFinding`).
    pub notes: Vec<SelectionNote>,
    /// The stored picks that name no item, or more than one.
    pub lost: Vec<ItemRef>,
    /// The refusal that generation gives for the selection, with the op
    /// label. `None` when the selection gives the op its items.
    pub refusal: Option<SelectionRefusal>,
    /// Every point and circle of the toolpath model, in the MODEL frame.
    /// Empty for an op that drills no hole.
    pub hole_items: Vec<HoleItem>,
    /// The holes that the op drills, in the MODEL frame, after the hole fit
    /// step (P6.5). Empty for an op that drills no hole, and when the
    /// selection refuses.
    pub holes: Vec<Hole>,
    /// Geometry selection P6.5: the fit of each item of `hole_items`
    /// against the tool ([`hole_fit`](fn@hole_fit)), by the key of the item. Empty for
    /// an op whose rule is not `HoleFitRule::ToolSize` (Pin Drill), and
    /// when the toolpath has no valid tool.
    pub hole_fits: Vec<(MatchKey, HoleFit)>,
    /// The key of every item the selection holds (its `identity`, in the
    /// frame and with the source of the drawing: `SourceRef::Model(id)` or
    /// `SourceRef::Stock`), in resolution order. A viewport click reads it
    /// to choose between `AddPick` and `RemovePick`
    /// ([`ProjectSession::pick_edit`]); the face highlight reads its face
    /// keys. Empty when the selection refuses before it resolves.
    pub selected: Vec<MatchKey>,
    /// One state per item of the toolpath model, index-aligned with
    /// `LoadedModel::items` (P4.4). The memo key holds that item set by
    /// `Weak` identity, so the alignment cannot go stale. Empty when the
    /// toolpath model does not exist.
    pub item_states: Vec<ItemState>,
    /// The items of the selection that are not in `LoadedModel::items` of
    /// the toolpath model: the items of another model, the stock items and
    /// the flat-region faces of a mesh (P4.7). The key (with the source of
    /// the item) and the state. The viewport finds the geometry by key.
    pub foreign: Vec<(MatchKey, ItemState)>,
    /// Geometry selection P3.7: the floor Z of each item, index-aligned
    /// with `item_states`. `Some` only for a `Cut` face under the floor
    /// "each picked face": the floor of its area, in the setup frame (the
    /// per-area floor rule of the prologue).
    pub item_floor_z: Vec<Option<f64>>,
    /// The floor Z of each `foreign` item, index-aligned with `foreign`,
    /// by the same rule as `item_floor_z`.
    pub foreign_floor_z: Vec<Option<f64>>,
    /// The mesh models whose flat faces the selection names and that are
    /// not computed yet (geometry selection P4.7). While it is not empty
    /// the report holds no count and no refusal: the GUI section says
    /// "computing faces…", and generation computes the faces itself.
    pub faces_not_ready: Vec<ModelId>,
    /// The highest plane Z of the up faces of the selection, in the SETUP
    /// frame (`ResolvedSelection::face_top_z`): the auto top that
    /// [`ProjectSession::toolpath_heights`] reads (geometry selection P3.1).
    pub face_top_z: Option<f64>,
    /// The lowest plane Z of the up faces of the selection, in the SETUP
    /// frame (`ResolvedSelection::face_bottom_z`): the deepest floor plane
    /// under "each picked face" (geometry selection P3.5).
    pub face_bottom_z: Option<f64>,
    /// Geometry selection P6.4 (ruling O-2): the outer ring of each closed
    /// area that an open path gives when the op extends it to the stock
    /// (Pocket with Extend to stock), in the MODEL frame (the frame of the
    /// drawing, as `holes`), in the order of the areas. The path itself is
    /// `Cut` in `item_states`. "Cut past open sides" (operator ruling
    /// 2026-10-23): also the outer ring of each face area that the setting
    /// grew to the stock edge. Empty for every other op and when the
    /// selection refuses. The GUI preview (P6.7) draws it.
    pub extended: Vec<Vec<P2>>,
    /// Geometry selection GT-2: one row per item of `selected`, in the same
    /// order: what holds the item (the default rule, a named rule, a pick)
    /// and its plane Z in the SETUP frame. The Geometry tab groups its
    /// list by it. R-E: the floor circle of a hole whose rim is a row is
    /// not a row; the rim row holds the hole ([`SelectedRow::hole`]).
    pub rows: Vec<SelectedRow>,
    /// R-B: the lowest rim Z of the fitted holes of a Drill or a Bore, in
    /// the SETUP frame (`compute::floor::hole_planes`). `Some` makes the
    /// auto top "auto: hole rims"; [`ProjectSession::toolpath_heights`]
    /// and generation read it.
    pub hole_rim_bottom_z: Option<f64>,
    /// R-B: the deepest hole floor of the fitted holes with no extra depth
    /// and no stock clamp, in the SETUP frame. "Each hole floor" reads it.
    pub hole_floor_bottom_z: Option<f64>,
}

/// R-E (operator ruling 2026-10-23): one round hole of a STEP model as
/// one row: its rim circle and its depth. The GUI Geometry tab, the MCP
/// `list_model_items` and `selection_status` replies read this one value
/// and its one text ([`Self::label`]).
#[derive(Debug, Clone, PartialEq)]
pub struct HoleSummary {
    /// The diameter of the hole (mm).
    pub diameter: f64,
    /// The Z of the rim, in the MODEL frame.
    pub rim_z: f64,
    /// The depth of the hole below its rim.
    pub depth: HoleDepth,
    /// The key of the floor circle of a blind hole (source: the model), or
    /// `None` for a through hole.
    pub floor_key: Option<MatchKey>,
}

impl HoleSummary {
    /// The hole of the rim at `index` of `items`, or `None` when that item
    /// is not a hole rim with a depth. `source` is the source the keys
    /// state.
    #[must_use]
    pub fn of(items: &ItemSet, index: usize, source: SourceRef) -> Option<Self> {
        let rim = items.iter().nth(index)?;
        let depth = rim.rim_depth()?;
        let Shape::Circle { diameter, .. } = rim.shape() else {
            return None;
        };
        Some(Self {
            diameter: *diameter,
            rim_z: rim.z()?,
            depth,
            floor_key: items
                .hole_floor_of(index)
                .and_then(|at| items.iter().nth(at))
                .map(|floor| MatchKey {
                    source,
                    ..floor.key().clone()
                }),
        })
    }

    /// THE text of a hole row: "Ø6 hole, 8 deep", "Ø6 hole, through".
    #[must_use]
    pub fn label(&self) -> String {
        hole_label(self.diameter, self.depth)
    }
}

/// R-E: THE text of one hole: "Ø6 hole, 8 deep" or "Ø6 hole, through".
/// Each number is [`short_number`].
#[must_use]
pub fn hole_label(diameter: f64, depth: HoleDepth) -> String {
    let d = short_number(diameter);
    match depth {
        HoleDepth::Blind { depth_mm } => {
            format!("\u{00D8}{d} hole, {} deep", short_number(depth_mm))
        }
        HoleDepth::Through { .. } => format!("\u{00D8}{d} hole, through"),
    }
}

/// `value` at 2 decimals with no trailing zero: 20.00 is "20", 12.50 is
/// "12.5". THE short number of the Geometry tab rows and of the hole text.
#[must_use]
pub fn short_number(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// One item that a selection holds (geometry selection GT-2): a row of
/// the selected list of the Geometry tab.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedRow {
    /// The key of the item, with the source of the drawing (as
    /// [`SelectionReport::selected`]).
    pub key: MatchKey,
    /// What holds the item: the default rule, a rule that names its kinds,
    /// or a pick (the explicit act wins, [`MatchedBy`]).
    pub by: MatchedBy,
    /// The index of the item in `LoadedModel::items` of the toolpath model,
    /// or `None` for an item of another source.
    pub own_index: Option<usize>,
    /// The plane Z of a face, or the Z of a model edge item, in the SETUP
    /// frame. `None` for a drawing item.
    pub plane_z: Option<f64>,
    /// R-E: the hole when this row is the rim of a hole with a depth.
    pub hole: Option<HoleSummary>,
    /// R-A: true when this row is the part outline of a mesh model. The GUI
    /// row and the MCP `role` read it ("Part outline").
    pub outline: bool,
}

impl SelectionReport {
    /// The state of `row`: from `item_states` by its own index, else from
    /// `foreign`. `NotSelected` when the report holds no state for it.
    pub fn row_state(&self, row: &SelectedRow) -> ItemState {
        match row.own_index {
            Some(index) => self.item_states.get(index).copied(),
            None => self
                .foreign
                .iter()
                .find(|(k, _)| k.matches(&row.key))
                .map(|(_, state)| *state),
        }
        .unwrap_or(ItemState::NotSelected)
    }

    /// "Cut past open sides" (operator ruling 2026-10-23): true when the
    /// selection holds a model area that a Pocket cuts as a face: a face,
    /// or a loop or a circle at its own Z (a STEP ring). A drawing item has
    /// no Z. The GUI shows the Pocket setting only then.
    pub fn holds_a_model_area(&self) -> bool {
        self.rows.iter().any(|row| {
            row.plane_z.is_some()
                && matches!(
                    row.key.kind,
                    ItemKind::Face | ItemKind::Loop | ItemKind::Circle
                )
        })
    }
}

/// One item of a model, as the `list_model_items` read gives it (geometry
/// selection DESIGN 10.2): the pick source of an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelItem {
    /// The key of the item, with the source `SourceRef::Model(id)` of the
    /// listed model. The `edit_toolpath_selection` command stores it as
    /// `SourceRef::ToolpathModel` on a toolpath of that model (P2.7).
    pub key: MatchKey,
    pub kind: ItemKind,
    pub layer: Option<String>,
    /// The key measure: loop and face `sqrt(|area|)`, path length, circle
    /// diameter, point 0 (mm).
    pub measure: f64,
    /// R-E: the hole when this item is the rim of a hole with a depth. The
    /// floor circle of that hole is not a row of the answer.
    pub hole: Option<HoleSummary>,
    /// R-A: true when this item is the part outline of a mesh model.
    pub outline: bool,
}

/// The `near` filter of the `list_model_items` read (geometry selection
/// P4.5): a point in the MODEL frame (the frame of the item keys) and a
/// radius in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Near {
    pub xy: [f64; 2],
    pub radius_mm: f64,
}

/// The two frames of one resolved selection. The frame is in the type
/// (F013): only `setup` has `face_top_z`.
pub(crate) struct SelectionFrames {
    /// As the drawing holds the items (the drill picker reads it).
    pub(crate) model: ResolvedSelection<ModelFrame>,
    /// After THE frame door: what the prologue receives.
    pub(crate) setup: ResolvedSelection<SetupFrame>,
}

impl ProjectSession {
    /// THE selection resolution of `tc`, for generation and for every
    /// reader: the selection `target` of `tc` (the op selection,
    /// `OperationConfig::selection`, or the boundary selection), resolved
    /// against the items of the models and the stock, then moved into the
    /// setup frame (`to_setup_frame`). A face pick is a pick of that
    /// selection (geometry selection P2.6, P4.1).
    ///
    /// `ctx` is the setup context of the toolpath: the stock in the
    /// emission frame (the stock source) and the setup transform (the face
    /// filter and the frame door). An op that holds no selection resolves
    /// to no item. A boundary whose source is not a selection refuses.
    pub(crate) fn resolve_selection_frames(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        ctx: &SetupEvalContext,
        faces: FaceAccess,
    ) -> Result<SelectionFrames, SessionError> {
        let empty = Selection::empty();
        let spec = tc.selection_of(target)?.unwrap_or(&empty);
        let uses = target_uses(tc, target);
        self.resolve_spec_frames(tc, spec, ctx, faces, uses)
            .map_err(|e| SessionError::OperationFailed(e.to_string()))
    }

    /// [`Self::resolve_selection_frames`] for one stored selection `spec`
    /// of `tc`. The boundary generation (`resolve_boundary_selection_polys`)
    /// and the status report share it, so the two cannot differ. `Err` is
    /// an item that the frame door cannot move.
    ///
    /// P4.7: when `spec` names a face, each mesh model it reads resolves
    /// against its flat-region faces ([`Self::mesh_faces_of`]); `faces`
    /// says whether a cold cache builds. `(item_use, outline_use)` is the
    /// use of the reader: the default rule reads it on a model edge item
    /// (R-C) and on the part outline (R-A).
    pub(crate) fn resolve_spec_frames(
        &self,
        tc: &ToolpathConfig,
        spec: &Selection,
        ctx: &SetupEvalContext,
        faces: FaceAccess,
        (item_use, outline_use): (ItemUse, OutlineUse),
    ) -> Result<SelectionFrames, ItemError> {
        let mesh_faces = self.mesh_faces_of(tc, spec, faces);
        let resolved = crate::selection::resolve(
            spec,
            &ResolveScope {
                toolpath_model: self.toolpath_model(tc),
                models: &self.models,
                stock_bbox: &ctx.heights_stock_bbox,
                setup: ctx.local_to_global.as_ref(),
                mesh_faces: &mesh_faces,
                item_use,
                outline_use,
            },
        );
        let setup = resolved
            .clone()
            .to_setup_frame(ctx.local_to_global.as_ref())?;
        Ok(SelectionFrames {
            model: resolved,
            setup,
        })
    }

    /// UX-8: the plane Z of the face of the floor "A picked face" of `tc`,
    /// in the setup frame of `ctx`, or `None` when the floor is not a face.
    ///
    /// The key resolves through THE resolution of a selection
    /// ([`Self::resolve_spec_frames`]) as a one-pick selection, so a floor
    /// key and a pick follow a re-import by one rule. Then [`crate::selection::view`]'s
    /// order of refusals applies: a missing model, faces not computed, a key
    /// that matches no item (`RefusalCause::FloorFaceLost`) or two items
    /// (`AmbiguousPick`). A face that points down in the setup has no
    /// plane to stand on (`FaceFacesDown`).
    pub(crate) fn floor_face_z(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        faces: FaceAccess,
    ) -> Option<Result<f64, SelectionRefusal>> {
        let key = tc.operation.floor()?.face_key()?.clone();
        let named = Some(ItemRef::new(key.clone()));
        let spec = Selection::from_picks(vec![key]);
        // The frame door fails only when the item breaks its shape
        // invariants in the setup frame (`ItemError`): a face or a ring that
        // a lateral setup turns on its side collapses. That face is not
        // flat and horizontal in this setup.
        // A one-pick selection: the use does not change what a pick matches.
        let uses = target_uses(tc, SelectionTarget::Op);
        let Ok(frames) = self.resolve_spec_frames(tc, &spec, ctx, faces, uses) else {
            return Some(Err(SelectionRefusal::new(
                named,
                RefusalCause::FaceNotFlat { face: None },
            )));
        };
        let refusal = |cause| Some(Err(SelectionRefusal::new(named.clone(), cause)));
        if let Some(source) = frames.model.missing_sources().first() {
            return refusal(RefusalCause::MissingSource(*source));
        }
        if let Some(model) = frames.model.faces_not_ready().first() {
            return refusal(RefusalCause::FacesNotReady { model: *model });
        }
        if let Some(lost) = frames.model.lost().first() {
            return refusal(if lost.matches == 0 {
                RefusalCause::FloorFaceLost
            } else {
                RefusalCause::AmbiguousPick {
                    matches: lost.matches,
                }
            });
        }
        match frames.setup.face_top_z() {
            Some(z) => Some(Ok(z)),
            None if frames.setup.items().is_empty() => refusal(RefusalCause::FloorFaceLost),
            None => refusal(RefusalCause::FaceFacesDown),
        }
    }

    /// The mesh models that `spec` reads whose faces are their flat regions
    /// (geometry selection P4.7), each with its face items: built when
    /// `faces` is `Build`, else read from the cache (`None` when cold).
    /// Empty when `spec` names no face ([`Selection::names_faces`]): the
    /// default selection never reads, and never builds, the flat regions.
    fn mesh_faces_of(
        &self,
        tc: &ToolpathConfig,
        spec: &Selection,
        faces: FaceAccess,
    ) -> Vec<MeshFaces> {
        if !spec.names_faces() {
            return Vec::new();
        }
        self.face_models_of(tc, spec)
            .into_iter()
            .map(|model| MeshFaces {
                model: model.id,
                items: match faces {
                    FaceAccess::Peek => model.face_items(),
                    FaceAccess::Build => Some(model.face_items_built()),
                },
            })
            .collect()
    }

    /// The mesh models (faces = flat regions) that a rule, a pick or an
    /// exclude of `spec` names, each once, in model order.
    fn face_models_of(&self, tc: &ToolpathConfig, spec: &Selection) -> Vec<&LoadedModel> {
        let named: Vec<ModelId> = spec
            .sources()
            .filter_map(|source| match source {
                SourceRef::ToolpathModel => Some(tc.model_id),
                SourceRef::Model(id) => Some(id),
                SourceRef::Stock => None,
            })
            .collect();
        self.models
            .iter()
            .filter(|m| m.faces_are_flat_regions() && named.contains(&m.id))
            .collect()
    }

    /// The auto top of an op with no selection slot (geometry selection
    /// P4.1, ruling O-1 A): the highest plane Z of the faces that its
    /// boundary selection picks, in the SETUP frame. `None` when the op
    /// holds a selection (its own faces give the top), when the boundary is
    /// off, when its source is not a selection, or when it picks no face.
    pub(crate) fn boundary_face_top_z(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
    ) -> Result<Option<f64>, SessionError> {
        if !self.reads_boundary_faces(tc) {
            return Ok(None);
        }
        Ok(self
            .resolve_selection_frames(tc, SelectionTarget::Boundary, ctx, FaceAccess::Build)?
            .setup
            .face_top_z())
    }

    /// The selection that a boundary `Reset` restores, and that the
    /// "Selection" boundary source writes (P4.3): the closed shapes of the
    /// first model that has a closed ring, else no item. With no item the
    /// boundary refuses `Empty` until the first pick.
    pub fn boundary_selection_default(&self) -> Selection {
        self.models
            .iter()
            .find(|m| m.items.has_closed_ring())
            .map_or_else(Selection::empty, |m| Selection::closed_shapes_of(m.id))
    }

    /// The model of `tc` by its EXACT id, or the id when no model has it.
    /// No fallback to another model (geometry selection P2.4).
    pub(crate) fn toolpath_model(&self, tc: &ToolpathConfig) -> Result<&LoadedModel, ModelId> {
        self.model_by_id(tc.model_id).ok_or(tc.model_id)
    }

    /// [`Self::resolve_selection_frames`] for `tc` in the setup that owns
    /// it, in the MODEL frame.
    pub(crate) fn resolve_toolpath_selection(
        &self,
        tc: &ToolpathConfig,
    ) -> Result<ResolvedSelection<ModelFrame>, SessionError> {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        Ok(self
            .resolve_selection_frames(tc, SelectionTarget::Op, &ctx, FaceAccess::Peek)?
            .model)
    }

    /// The `selection_status` answer for the selection `target` of `tc`:
    /// what generation does with that selection, with no kernel (DESIGN
    /// 5.4, P4.1).
    ///
    /// The memo gives the stored report while no input of its key moves
    /// (see the module documentation). `Err` is an error that is not a
    /// selection refusal (an item that the frame door cannot move, or a
    /// boundary whose source is not a selection); the memo does not keep
    /// it.
    pub fn selection_status(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
    ) -> Result<Arc<SelectionReport>, SessionError> {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        let mut guard = match self.selection_status_memo.entries.lock() {
            Ok(guard) => guard,
            // An entry is written only after its build returns, so a panic
            // in an earlier build leaves no partial entry.
            Err(poisoned) => poisoned.into_inner(),
        };
        let key = StatusKey::capture(self, tc, target, &ctx);
        if let (Some(key), Some(entry)) = (key.as_ref(), guard.get(&(tc.id, target)))
            && entry.key.answers(key)
        {
            return Ok(Arc::clone(&entry.report));
        }
        self.selection_status_memo
            .builds
            .fetch_add(1, Ordering::Relaxed);
        let report = Arc::new(self.build_selection_report(tc, target, &ctx)?);
        // Drop the entries of the toolpaths that no longer exist.
        guard.retain(|(id, _), _| self.toolpath_configs.iter().any(|t| t.id == *id));
        match key {
            Some(key) => {
                guard.insert(
                    (tc.id, target),
                    StatusEntry {
                        key,
                        report: Arc::clone(&report),
                    },
                );
            }
            None => {
                guard.remove(&(tc.id, target));
            }
        }
        Ok(report)
    }

    /// How many times [`Self::selection_status`] built a report on this
    /// session. A sentry reads it to prove that an unrelated edit builds
    /// nothing and that a selection input builds again.
    pub fn selection_status_builds(&self) -> u64 {
        self.selection_status_memo.builds.load(Ordering::Relaxed)
    }

    /// The report itself, with no memo.
    pub(super) fn build_selection_report(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        ctx: &SetupEvalContext,
    ) -> Result<SelectionReport, SessionError> {
        let op_type = tc.operation.op_type();
        let (label, item_use, runs_view) = match target {
            SelectionTarget::Op => (
                op_type.label(),
                tc.operation.capabilities().item_use,
                // The prologue runs `view` for an op whose row reads items,
                // and only for it (`prologue::check_inputs`).
                reads_items(op_type),
            ),
            // The boundary generation always runs `view` with `Regions`
            // (`resolve_boundary_selection_polys`).
            SelectionTarget::Boundary => (BOUNDARY_LABEL, BOUNDARY_ITEM_USE, true),
        };
        let frames = self.resolve_selection_frames(tc, target, ctx, FaceAccess::Peek)?;
        // "Cut past open sides": the stock as resolved, before the P6.4
        // growth, as the prologue reads it.
        let resolved_stock = frames.setup.stock_xy();
        // P6.4 (O-2 (B)): the stock grown by the tool radius, as the
        // prologue grows it, so the report extends as generation does.
        let frames = match (target, self.extend_radius(tc)) {
            (SelectionTarget::Op, Some(radius)) => SelectionFrames {
                setup: frames.setup.with_stock_grown(radius),
                model: frames.model,
            },
            (SelectionTarget::Op | SelectionTarget::Boundary, _) => frames,
        };
        let selected: Vec<MatchKey> = frames
            .model
            .items()
            .iter()
            .map(|r| r.identity().clone())
            .collect();
        let hole_items = match target {
            SelectionTarget::Op => self.hole_items(tc, &selected),
            SelectionTarget::Boundary => Vec::new(),
        };
        // P6.5: the tool diameter and the size tolerance of the hole fit
        // step, as the prologue reads them. `None` for an op with no fit
        // rule and for a toolpath with no valid tool (generation refuses
        // that tool itself).
        let fit_size = match target {
            SelectionTarget::Op => self.hole_fit_size(tc),
            SelectionTarget::Boundary => None,
        };
        let hole_fits = fit_size.map_or_else(Vec::new, |size| {
            hole_items
                .iter()
                .map(|h| (h.key.clone(), hole_fit(h.diameter, &size)))
                .collect()
        });
        let own_source = SourceRef::Model(tc.model_id);
        let own_items = self.toolpath_model(tc).ok().map(|m| Arc::clone(&m.items));
        let rows: Vec<SelectedRow> = frames
            .model
            .items()
            .iter()
            .zip(frames.setup.items())
            .map(|(model, setup)| {
                let own_index = (model.item().source() == own_source).then(|| model.index());
                SelectedRow {
                    key: model.identity().clone(),
                    by: model.by(),
                    own_index,
                    plane_z: match setup.item().shape() {
                        Shape::Face(face) => Some(face.plane_z),
                        Shape::Loop(_)
                        | Shape::Path(_)
                        | Shape::Point(_)
                        | Shape::Circle { .. } => setup.item().z(),
                    },
                    hole: own_index
                        .zip(own_items.as_deref())
                        .and_then(|(at, items)| HoleSummary::of(items, at, own_source)),
                    outline: model.item().is_part_outline(),
                }
            })
            .collect();
        // R-E: one hole is one row. The floor circle of a hole whose rim is
        // a row folds into the rim row.
        let rows = match own_items.as_deref() {
            Some(items) => fold_hole_floors(rows, items, |row| row.own_index),
            None => rows,
        };
        let mut report = SelectionReport {
            rows,
            hole_fits,
            hole_items,
            selected,
            face_top_z: frames.setup.face_top_z(),
            face_bottom_z: frames.setup.face_bottom_z(),
            ..SelectionReport::default()
        };
        // P4.4: the resolution states first. An item that the selection
        // holds is `Skipped` until `view` gives it its role, so a refused
        // selection marks no item `Cut`.
        report.item_states = self.toolpath_model(tc).map_or_else(
            |_| Vec::new(),
            |m| vec![ItemState::NotSelected; m.items.len()],
        );
        report.item_floor_z = vec![None; report.item_states.len()];
        for excluded in frames.model.excluded() {
            set_state(
                &mut report,
                own_source,
                (excluded.source, excluded.index, &excluded.key),
                ItemState::Excluded,
                None,
            );
        }
        report.lost = frames
            .model
            .lost()
            .iter()
            .map(|lost| ItemRef::new(lost.key.clone()))
            .collect();
        for r in frames.setup.items() {
            let shape = r.item().shape();
            let kind = shape.kind();
            if kind.serves(item_use) {
                *report.counts.entry(kind).or_insert(0) += 1;
                // P3.4: the facing in the setup frame (after the frame
                // door), by the one facing rule.
                if let Shape::Face(face) = shape {
                    report.faces_by_facing.count(face.normal.z);
                }
            }
        }
        // P3.7: `floors` gives the floor Z of each resolution item (each
        // face under "each picked face"), index-aligned with the items.
        let resolution_states =
            |report: &mut SelectionReport, roles: Option<&[ItemRole]>, floors: &[Option<f64>]| {
                for (at, r) in frames.model.items().iter().enumerate() {
                    let state = match roles.and_then(|roles| roles.get(at)) {
                        Some(ItemRole::Cut) => ItemState::Cut,
                        Some(ItemRole::Island) => ItemState::Island,
                        Some(ItemRole::Skipped) | None => ItemState::Skipped,
                    };
                    let floor_z = floors.get(at).copied().flatten();
                    set_state(
                        report,
                        own_source,
                        (r.item().source(), r.index(), r.identity()),
                        state,
                        floor_z.filter(|_| state == ItemState::Cut),
                    );
                }
            };
        // P4.7: faces that are not computed yet give no count and no
        // refusal ("computing faces"); generation computes them itself.
        if !frames.model.faces_not_ready().is_empty() {
            resolution_states(&mut report, None, &[]);
            report.faces_not_ready = frames.model.faces_not_ready().to_vec();
            report.counts.clear();
            report.faces_by_facing = FacesByFacing::default();
            return Ok(report);
        }
        if !runs_view {
            resolution_states(&mut report, None, &[]);
            return Ok(report);
        }
        // P3.4: a boundary keeps a down face (its outline); an op refuses
        // it. Each reads the view that its generation reads.
        let viewed = match target {
            // R-A: the "frees the part" note by the prologue rule.
            SelectionTarget::Op => {
                crate::selection::view(&frames.setup, item_use).map(|(items, mut notes)| {
                    crate::selection::drop_part_free_note(
                        &mut notes,
                        tc.operation.frees_the_part(),
                    );
                    (items, notes)
                })
            }
            SelectionTarget::Boundary => crate::selection::view_boundary(&frames.setup),
        };
        // "Cut past open sides" (operator ruling 2026-10-23): THE area rule
        // of the prologue (`compute::open_sides`), on the setup mesh and the
        // tool radius that generation reads, before the per-face floors as
        // in the prologue, so the status shows the area that the op cuts.
        let viewed = match (viewed, target) {
            (Ok((mut items, mut notes)), SelectionTarget::Op)
                if tc.operation.cuts_past_open_sides() =>
            {
                notes.extend(self.grow_open_sides_as_generation(
                    tc,
                    ctx,
                    &frames.setup,
                    resolved_stock,
                    &mut items,
                )?);
                Ok((items, notes))
            }
            (viewed, _) => viewed,
        };
        // P3.5: under "each picked face" every closed area needs a face.
        // P3.7: a face whose floor is at or above the top is left out with a
        // note, and the op refuses only when every face is at the top. THE
        // rule (`compute::floor::region_floors`) that the prologue runs, with
        // the same top, so the status and generation give one answer.
        let mut floors = Vec::new();
        // S11: an unset floor has no plane rule; the heights refuse it.
        let set_floor = tc.operation.floor().filter(|floor| !floor.is_unset());
        let viewed = match (viewed, target, set_floor) {
            (Ok((mut items, mut notes)), SelectionTarget::Op, Some(floor)) => {
                match self.each_face_top_z(tc, ctx)? {
                    Some(top_z) => crate::compute::floor::region_floors(
                        &frames.setup,
                        &mut items,
                        &floor,
                        top_z,
                    )
                    .map(|region_floors| {
                        if let (Some(region_floors), Some(regions)) =
                            (region_floors, items.regions())
                        {
                            floors = vec![None; frames.model.items().len()];
                            for (index, &floor_z) in region_floors.as_slice().iter().enumerate() {
                                if let Some(slot) = regions
                                    .exterior_item(index)
                                    .and_then(|at| floors.get_mut(at))
                                {
                                    *slot = Some(floor_z);
                                }
                            }
                            notes.extend(region_floors.into_notes());
                        }
                        (items, notes)
                    }),
                    None => Ok((items, notes)),
                }
            }
            (viewed, _, _) => viewed,
        };
        // P6.5: THE hole fit step of the prologue, with the same tool
        // diameter and size tolerance, so a skipped circle is `Skipped` here and not cut there.
        let viewed = match (viewed, fit_size) {
            (Ok((mut items, mut notes)), Some(size)) => {
                fit_holes(&frames.setup, &mut items, item_use, size).map(|fit_notes| {
                    notes.extend(fit_notes);
                    (items, notes)
                })
            }
            (viewed, _) => viewed,
        };
        // Chamfer Clearance: THE wall trim of the prologue
        // (`compute::edge_clear`), on the setup mesh and the tool that
        // generation reads, so an edge with no clear stretch is `Skipped`
        // here and not cut there, and the notes are the same.
        let viewed = match (viewed, target) {
            (Ok((mut items, mut notes)), SelectionTarget::Op) if item_use == ItemUse::Edges => self
                .clear_edges_as_generation(tc, ctx, &frames.setup, &mut items)?
                .map(|clear_notes| {
                    notes.extend(clear_notes);
                    (items, notes)
                }),
            (viewed, _) => viewed,
        };
        // R-B, F19: THE per-hole floor rule of the prologue
        // (`compute::floor::hole_floors`), on the heights that the hole
        // planes of this report give, so the status and generation give
        // one answer. Each Cut hole gets its floor Z (`item_floor_z`).
        // S11: the hole planes are facts of the holes, so the report gives
        // them under an unset floor too (`each_hole_floor_due` reads them);
        // only the per-hole floors need a set floor.
        let viewed = match (viewed, target, tc.operation.floor()) {
            (Ok((items, notes)), SelectionTarget::Op, Some(floor)) => {
                match items.holes().cloned() {
                    None => Ok((items, notes)),
                    Some(holes) if floor.is_unset() => {
                        let (rim_bottom, floor_bottom) = crate::compute::floor::hole_planes(&holes);
                        report.hole_rim_bottom_z = rim_bottom;
                        report.hole_floor_bottom_z = floor_bottom;
                        Ok((items, notes))
                    }
                    Some(holes) => {
                        let (rim_bottom, floor_bottom) = crate::compute::floor::hole_planes(&holes);
                        report.hole_rim_bottom_z = rim_bottom;
                        report.hole_floor_bottom_z = floor_bottom;
                        let mesh = self.setup_mesh(tc, ctx)?;
                        let answer = self.heights_in_setup(
                            tc,
                            ctx,
                            mesh.as_deref(),
                            super::heights::FacePlanes {
                                top: report.face_top_z,
                                bottom: report.face_bottom_z,
                                floor_face: self
                                    .floor_face_z(tc, ctx, FaceAccess::Peek)
                                    .and_then(Result::ok),
                                hole_rim_bottom: rim_bottom,
                                hole_floor_bottom: floor_bottom,
                            },
                        );
                        let inputs = crate::compute::floor::HoleFloorInputs::new(
                            &answer.heights,
                            answer.context.stock_bottom_z,
                        );
                        crate::compute::floor::hole_floors(&frames.setup, &holes, &floor, &inputs)
                            .map(|spans| {
                                if floors.is_empty() {
                                    floors = vec![None; frames.model.items().len()];
                                }
                                for (index, span) in spans.iter().enumerate() {
                                    if let Some(slot) =
                                        holes.item(index).and_then(|at| floors.get_mut(at))
                                    {
                                        *slot = Some(span.floor_z);
                                    }
                                }
                                (items, notes)
                            })
                    }
                }
            }
            (viewed, _, _) => viewed,
        };
        // F29: the `Empty` refusal of a Drill names the hole rims that the
        // default rule did not take, as the prologue names them.
        let viewed = match fit_size {
            Some(size) => {
                viewed.map_err(|refusal| refusal.with_default_rims(&frames.setup, item_use, &size))
            }
            None => viewed,
        };
        // UX-8: the face of the floor "A picked face" refuses as generation
        // refuses it (before the prologue runs `view`), so the status and
        // generation give one answer.
        let viewed = match (target, self.floor_face_z(tc, ctx, FaceAccess::Peek)) {
            (SelectionTarget::Op, Some(Err(refusal))) => Err(refusal),
            (SelectionTarget::Op | SelectionTarget::Boundary, _) => viewed,
        };
        match viewed {
            Ok((items, notes)) => {
                // The frame door keeps the order of the items, so the
                // roles of the setup half align with the model half.
                resolution_states(&mut report, Some(items.roles()), &floors);
                report.skipped_by_kind = items.skipped_by_kind().clone();
                report.notes = notes;
                report.islands = items
                    .regions()
                    .map_or(0, |r| r.as_slice().iter().map(|p| p.holes.len()).sum());
                report.extended = extended_areas(&frames.setup, &items, ctx);
                if items.holes().is_some()
                    && let Ok((mut model_items, _)) =
                        crate::selection::view(&frames.model, item_use)
                {
                    // The frame door keeps each diameter, so the fit of the
                    // model half is the fit of the setup half.
                    if let Some(size) = fit_size {
                        let _ = fit_holes(&frames.model, &mut model_items, item_use, size);
                    }
                    report.holes = model_items
                        .holes()
                        .map(|h| h.as_slice().to_vec())
                        .unwrap_or_default();
                }
            }
            Err(refusal) => {
                resolution_states(&mut report, None, &[]);
                report.refusal = Some(refusal.for_op(label));
            }
        }
        Ok(report)
    }

    /// Chamfer Clearance: THE wall trim (`compute::edge_clear`) of the
    /// model edges in `items`, with the inputs that the prologue reads: the
    /// setup mesh of the toolpath model, the built cutter of the tool, and
    /// the bevel of the op. With no mesh, no bevel or a tool that the op
    /// refuses, the trim does nothing, as in the prologue.
    fn clear_edges_as_generation(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup: &crate::selection::ResolvedSelection<SetupFrame>,
        items: &mut OpItems,
    ) -> Result<Result<Vec<SelectionNote>, SelectionRefusal>, SessionError> {
        let Some((chamfer_width, tip_offset)) = tc.operation.bevel_width_and_tip_mm() else {
            return Ok(Ok(Vec::new()));
        };
        let Some(tool) = self.toolpath_tool(tc) else {
            return Ok(Ok(Vec::new()));
        };
        if tc
            .operation
            .op_type()
            .tool_refusal(tool.tool_type.cutter_kind())
            .is_some()
        {
            return Ok(Ok(Vec::new()));
        }
        let Ok(cutter) = crate::compute::cutter::build_cutter(tool) else {
            return Ok(Ok(Vec::new()));
        };
        let Some(mesh) = self.setup_mesh(tc, ctx)? else {
            return Ok(Ok(Vec::new()));
        };
        let index = crate::maps::geom_cache::cached_auto_index(&mesh);
        let probe = crate::compute::edge_clear::EdgeClearProbe {
            mesh: &mesh,
            index: &index,
            cutter: &cutter,
            half_angle_rad: (tool.included_angle / 2.0).to_radians(),
            chamfer_width,
            tip_offset,
        };
        Ok(crate::compute::edge_clear::clear_edge_walls(
            setup, items, &probe,
        ))
    }

    /// "Cut past open sides": THE area rule (`compute::open_sides`) of the
    /// prologue, on the setup mesh and the tool radius of the built cutter,
    /// with the stock as resolved. No tool, no cutter, no stock, no mesh or
    /// a floor that is not "Each picked face": no change (generation
    /// refuses the tool itself).
    fn grow_open_sides_as_generation(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup: &crate::selection::ResolvedSelection<SetupFrame>,
        stock: Option<crate::selection::StockXy>,
        items: &mut OpItems,
    ) -> Result<Vec<SelectionNote>, SessionError> {
        let Some(radius) = self.open_sides_radius(tc) else {
            return Ok(Vec::new());
        };
        let (Some(stock), Some(Floor::EachFace { depth })) = (stock, tc.operation.floor()) else {
            return Ok(Vec::new());
        };
        let Some(mesh) = self.setup_mesh(tc, ctx)? else {
            return Ok(Vec::new());
        };
        let probe = crate::compute::open_sides::OpenSidesProbe {
            mesh: &mesh,
            stock,
            tool_radius: radius,
            face_depth: depth,
        };
        Ok(crate::compute::open_sides::grow_past_open_sides(
            setup, items, &probe,
        ))
    }

    /// "Cut past open sides": the tool radius by which THE area rule grows
    /// the stock (`ToolDefinition::diameter` of the built cutter, as the
    /// prologue reads it), or `None` when the setting is off or the
    /// toolpath has no valid tool.
    fn open_sides_radius(&self, tc: &ToolpathConfig) -> Option<f64> {
        if !tc.operation.cuts_past_open_sides() {
            return None;
        }
        let tool = self.toolpath_tool(tc)?;
        crate::compute::cutter::build_cutter(tool)
            .ok()
            .map(|cutter| crate::tool::MillingCutter::diameter(&cutter) / 2.0)
    }

    /// P6.5: the tool diameter and the size tolerance that the hole fit
    /// step of `tc` reads, as the prologue reads them
    /// (`ToolDefinition::diameter` of the built cutter,
    /// `OperationConfig::drill_size`). `None` when the op has no fit rule (`HoleFitRule::None`, every op
    /// that reads no hole), or when the toolpath has no valid tool.
    fn hole_fit_size(&self, tc: &ToolpathConfig) -> Option<DrillSize> {
        match tc.operation.capabilities().item_use {
            ItemUse::Holes {
                fit: HoleFitRule::ToolSize | HoleFitRule::LargerThanTool,
                ..
            } => {}
            ItemUse::Holes {
                fit: HoleFitRule::None,
                ..
            }
            | ItemUse::None
            | ItemUse::Regions
            | ItemUse::RegionsExtendOpen { .. }
            | ItemUse::Curves
            | ItemUse::Edges => return None,
        }
        let tool = self.toolpath_tool(tc)?;
        crate::compute::cutter::build_cutter(tool)
            .ok()
            .map(|cutter| {
                tc.operation
                    .drill_size(crate::tool::MillingCutter::diameter(&cutter))
            })
    }

    /// P6.4 (ruling O-2 (B)): the tool radius by which the prologue grows
    /// the stock of an op that extends its open paths to the stock
    /// (`ItemUse::RegionsExtendOpen`), as the prologue reads it
    /// (`ToolDefinition::diameter` of the built cutter, halved). `None` for
    /// every other op, and for a toolpath with no valid tool: the report
    /// then extends to the stock edge itself, and generation refuses the
    /// tool.
    fn extend_radius(&self, tc: &ToolpathConfig) -> Option<f64> {
        match tc.operation.capabilities().item_use {
            ItemUse::RegionsExtendOpen { .. } => {}
            ItemUse::None
            | ItemUse::Regions
            | ItemUse::Curves
            | ItemUse::Edges
            | ItemUse::Holes { .. } => {
                return None;
            }
        }
        let tool = self.toolpath_tool(tc)?;
        crate::compute::cutter::build_cutter(tool)
            .ok()
            .map(|cutter| crate::tool::MillingCutter::diameter(&cutter) / 2.0)
    }

    /// Every point and circle of the toolpath model of a drill op, in the
    /// MODEL frame, each marked when `selected` holds it. Empty for every
    /// other op.
    fn hole_items(&self, tc: &ToolpathConfig, selected: &[MatchKey]) -> Vec<HoleItem> {
        match tc.operation.capabilities().item_use {
            ItemUse::Holes { .. } => {}
            ItemUse::None
            | ItemUse::Regions
            | ItemUse::RegionsExtendOpen { .. }
            | ItemUse::Curves
            | ItemUse::Edges => return Vec::new(),
        }
        let Ok(model) = self.toolpath_model(tc) else {
            return Vec::new();
        };
        model
            .items
            .iter()
            .filter_map(|item| {
                let (xy, diameter) = match item.shape() {
                    Shape::Point(p) => (*p, None),
                    Shape::Circle {
                        centre, diameter, ..
                    } => (*centre, Some(*diameter)),
                    Shape::Loop(_) | Shape::Path(_) | Shape::Face(_) => return None,
                };
                Some(HoleItem {
                    key: MatchKey {
                        source: SourceRef::ToolpathModel,
                        ..item.key().clone()
                    },
                    xy,
                    diameter,
                    layer: item.layer().map(str::to_owned),
                    selected: selected.iter().any(|k| k.matches(item.key())),
                })
            })
            .collect()
    }

    /// The holes of a drill op (`ItemUse::Holes`), as generation resolves
    /// them, in the MODEL frame. `None` for an op that drills no hole.
    ///
    /// A projection of [`Self::selection_status`] (the memo), so the GUI
    /// drill picker, its viewport markers, the precondition check and the
    /// dowel-hole check say what the generator does, and none of them
    /// resolves again on each frame.
    pub fn hole_selection(&self, tc: &ToolpathConfig) -> Option<HoleSelection> {
        match tc.operation.capabilities().item_use {
            ItemUse::Holes { .. } => {}
            ItemUse::None
            | ItemUse::Regions
            | ItemUse::RegionsExtendOpen { .. }
            | ItemUse::Curves
            | ItemUse::Edges => return None,
        }
        Some(match self.selection_status(tc, SelectionTarget::Op) {
            Ok(report) => HoleSelection {
                items: report.hole_items.clone(),
                holes: report.holes.clone(),
                lost_picks: report.lost.len(),
                refusal: report.refusal.as_ref().map(ToString::to_string),
            },
            Err(error) => HoleSelection {
                refusal: Some(error.to_string()),
                ..HoleSelection::default()
            },
        })
    }
}

impl ProjectSession {
    /// The pick key of the BREP face `face` of model `model_id`, for a pick
    /// in the selection of `tc` (geometry selection P2.6: a viewport face
    /// click). The key names the Face item whose BREP id is `face`. Its
    /// source is [`SourceRef::ToolpathModel`] when the model is the model of
    /// `tc`, else [`SourceRef::Model`].
    ///
    /// The STEP door makes a Face item for each flat horizontal face only
    /// (`io::brep_face_items`), so a face with no Face item refuses
    /// `FaceNotFlat` and names the face. A model id that names no model
    /// refuses `MissingSource`.
    pub fn face_pick_key(
        &self,
        tc: &ToolpathConfig,
        model_id: ModelId,
        face: FaceGroupId,
    ) -> Result<MatchKey, SelectionRefusal> {
        let model = self.model_by_id(model_id).ok_or_else(|| {
            SelectionRefusal::new(
                None,
                RefusalCause::MissingSource(SourceRef::Model(model_id)),
            )
        })?;
        let source = if model_id == tc.model_id {
            SourceRef::ToolpathModel
        } else {
            SourceRef::Model(model_id)
        };
        model
            .items
            .iter()
            .find(|item| match item.shape() {
                Shape::Face(f) => f.brep == Some(face),
                Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => false,
            })
            .map(|item| MatchKey {
                source,
                ..item.key().clone()
            })
            .ok_or_else(|| {
                SelectionRefusal::new(None, RefusalCause::FaceNotFlat { face: Some(face) })
            })
    }

    /// The pick key of the flat-region face of mesh model `model_id` under
    /// the ray hit `hit`, in the MODEL frame (geometry selection P4.7: a
    /// viewport click on an STL). The face is the `Face` item whose plane Z
    /// is within [`FLAT_REGION_Z_BAND_MM`] of the hit Z and whose rings
    /// hold the hit XY (an edge within the band counts). The source is as
    /// for [`Self::face_pick_key`].
    ///
    /// It reads the cache and builds nothing (the frame loop calls it):
    /// a cold cache refuses `FacesNotReady`. A hit on no flat region (a
    /// wall, a slope) refuses `FaceNotFlat`. A model id that names no model
    /// refuses `MissingSource`.
    pub fn face_pick_key_at(
        &self,
        tc: &ToolpathConfig,
        model_id: ModelId,
        hit: P3,
    ) -> Result<MatchKey, SelectionRefusal> {
        let model = self.model_by_id(model_id).ok_or_else(|| {
            SelectionRefusal::new(
                None,
                RefusalCause::MissingSource(SourceRef::Model(model_id)),
            )
        })?;
        // The key takes its source below, so the unbound cached set serves
        // (no bind per hover frame).
        let faces = match (&model.mesh, model.faces_are_flat_regions()) {
            (Some(mesh), true) => crate::maps::geom_cache::peek_flat_regions(mesh),
            _ => Some(Arc::clone(&model.items)),
        }
        .ok_or_else(|| {
            SelectionRefusal::new(None, RefusalCause::FacesNotReady { model: model_id })
        })?;
        let source = if model_id == tc.model_id {
            SourceRef::ToolpathModel
        } else {
            SourceRef::Model(model_id)
        };
        let xy = P2::new(hit.x, hit.y);
        faces
            .iter()
            .find(|item| match item.shape() {
                Shape::Face(f) => {
                    (f.plane_z - hit.z).abs() <= FLAT_REGION_Z_BAND_MM
                        && f.rings.contains_point_eps(&xy, FLAT_REGION_Z_BAND_MM)
                }
                Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => false,
            })
            .map(|item| MatchKey {
                source,
                ..item.key().clone()
            })
            .ok_or_else(|| SelectionRefusal::new(None, RefusalCause::FaceNotFlat { face: None }))
    }

    /// The edit that a viewport click on `key` sends to the selection
    /// `target` of `tc` (lead ruling P2.2: no toggle edit in the selection;
    /// P4.1: the target). It reads
    /// THE resolution through the memo ([`Self::selection_status`]): an
    /// item that the selection holds gives `RemovePick` (the command turns
    /// a rule that holds it into picks, GT-Q8), any other item gives
    /// `AddPick`.
    ///
    /// `key` names its source as a stored pick does. The source
    /// [`SourceRef::ToolpathModel`] reads as the model of `tc`.
    ///
    /// Operator ruling O-PICK (2026-10-22): while the selection is still
    /// its default ([`Selection::holds_default`]), a click on an item that
    /// is not the stock gives `AddPick`, also when the default rule holds
    /// the item. [`Selection::apply`] then replaces the default with that
    /// one pick, so the op cuts only the picked item. A click on the stock
    /// item of a Face op under its stock default still toggles it.
    pub fn pick_edit(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: MatchKey,
    ) -> Result<SelectionEdit, SessionError> {
        let key = self.hole_rim_key(tc, target, key);
        let source = match key.source {
            SourceRef::ToolpathModel => SourceRef::Model(tc.model_id),
            other @ (SourceRef::Model(_) | SourceRef::Stock) => other,
        };
        if source != SourceRef::Stock && self.holds_its_default(tc, target)? {
            return Ok(SelectionEdit::AddPick(key));
        }
        let report = self.selection_status(tc, target)?;
        let selected = report
            .selected
            .iter()
            .any(|k| k.source == source && k.names_same_geometry(&key));
        Ok(if selected {
            SelectionEdit::RemovePick(key)
        } else {
            SelectionEdit::AddPick(key)
        })
    }

    /// R-B, decision 4 (lead, 2026-10-23): THE pick rule of a hole. On the
    /// op selection of a Drill or a Bore (`ItemUse::Holes`), a pick of the
    /// floor circle of a hole is a pick of its rim ([`ItemSet::rim_of`]);
    /// the key keeps its source. Every other key comes back unchanged. A
    /// viewport click ([`Self::pick_edit`]) reads it; the MCP `near` read
    /// folds the floor circle into its rim row by the same pairing rule.
    #[must_use]
    pub fn hole_rim_key(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: MatchKey,
    ) -> MatchKey {
        let holes_op = target == SelectionTarget::Op
            && matches!(tc.operation.capabilities().item_use, ItemUse::Holes { .. });
        let model_id = match key.source {
            SourceRef::ToolpathModel => Some(tc.model_id),
            SourceRef::Model(id) => Some(id),
            SourceRef::Stock => None,
        };
        let Some(model) = model_id
            .filter(|_| holes_op)
            .and_then(|id| self.model_by_id(id))
        else {
            return key;
        };
        let items = &model.items;
        let rim = items
            .iter()
            .position(|item| item.key().names_same_geometry(&key))
            .and_then(|at| items.rim_of(at))
            .and_then(|at| items.iter().nth(at));
        match rim {
            Some(rim) => MatchKey {
                source: key.source,
                ..rim.key().clone()
            },
            None => key,
        }
    }

    /// The default of the selection `target` of `tc`: the op default
    /// (`OperationConfig::default_selection`) or
    /// [`Self::boundary_selection_default`]. `None` for an op that holds
    /// no selection.
    pub(crate) fn selection_default_of(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
    ) -> Option<Selection> {
        match target {
            SelectionTarget::Op => tc.operation.default_selection(),
            SelectionTarget::Boundary => Some(self.boundary_selection_default()),
        }
    }

    /// True when the selection `target` of `tc` is still its default rule
    /// (operator ruling O-PICK, [`Selection::holds_default`]). The
    /// Geometry tab and the Selection section read it for the default row.
    pub fn holds_its_default(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
    ) -> Result<bool, SessionError> {
        let current = tc.selection_of(target)?;
        Ok(match (current, self.selection_default_of(tc, target)) {
            (Some(current), Some(default)) => current.holds_default(&default, tc.model_id),
            _ => false,
        })
    }

    /// Operator ruling GT-Q8 (2026-10-22): the edit that drops the item
    /// `key` from the rules of the selection `target` of `tc`. Each rule
    /// that holds the item turns into picks: all its members less the
    /// dropped item. The rules that do not hold it, the other picks and the
    /// excludes stay. A pick that names the item goes too. The answer is
    /// ONE `Replace`, so the act is one undo step.
    ///
    /// The members of a rule are the items that it gives the op on its
    /// own (with the excludes of the selection): for a default rule
    /// ([`crate::selection::KindFilter::Served`]) the items that the op
    /// cuts (`Cut` or `Island`), so an item that the default leaves out
    /// (an open path on a Pocket, a Drill circle of another size) does not
    /// become a named item that refuses. A rule that names its kinds gives
    /// every item that it matches.
    ///
    /// `None` when no rule holds the item: the plain `RemovePick` then
    /// applies. `edit_toolpath_selection` calls it for every `RemovePick`,
    /// so the GUI click and the MCP `remove_pick` take one rule.
    pub(crate) fn rule_drop_edit(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: &MatchKey,
    ) -> Result<Option<SelectionEdit>, SessionError> {
        let model = tc.model_id;
        let Some(current) = tc.selection_of(target)? else {
            return Ok(None);
        };
        let current = current.clone().for_toolpath_model(model);
        let key = key.clone().for_toolpath_model(model);
        let names_key = |k: &MatchKey| k.clone().for_toolpath_model(model).matches(&key);
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        let mut kept_rules = Vec::new();
        let mut rule_picks: Vec<MatchKey> = Vec::new();
        let mut dropped_a_rule = false;
        for rule in current.rules() {
            let members = self.rule_members(tc, target, &ctx, rule, current.excludes())?;
            if !members.iter().any(&names_key) {
                kept_rules.push(rule.clone());
                continue;
            }
            dropped_a_rule = true;
            for member in members {
                if !names_key(&member) && !rule_picks.iter().any(|k| k.matches(&member)) {
                    rule_picks.push(member);
                }
            }
        }
        if !dropped_a_rule {
            return Ok(None);
        }
        let mut picks: Vec<MatchKey> = current
            .picks()
            .iter()
            .filter(|k| !names_key(k))
            .cloned()
            .collect();
        for member in rule_picks {
            if !picks.iter().any(|k| k.matches(&member)) {
                picks.push(member);
            }
        }
        Ok(Some(SelectionEdit::Replace(Box::new(
            Selection::from_parts(kept_rules, picks, current.excludes().to_vec()),
        ))))
    }

    /// The members of `rule` in the selection `target` of `tc`: the items
    /// that it gives the op on its own, with `excludes`, each key in the
    /// toolpath form. A default rule ([`crate::selection::KindFilter::Served`])
    /// gives the items that the op cuts (`Cut` or `Island`); a rule that
    /// names its kinds gives every item that it matches. GT-Q8
    /// ([`Self::rule_drop_edit`]) and the GT-3 rule offers
    /// ([`Self::rule_offers`]) read this one rule.
    pub(crate) fn rule_members(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        ctx: &SetupEvalContext,
        rule: &crate::selection::Rule,
        excludes: &[MatchKey],
    ) -> Result<Vec<MatchKey>, SessionError> {
        let probe = with_selection(
            tc,
            target,
            Selection::from_parts(vec![rule.clone()], Vec::new(), excludes.to_vec()),
        );
        let report = self.build_selection_report(&probe, target, ctx)?;
        Ok(report
            .selected
            .iter()
            .filter(|k| {
                rule.names_kinds()
                    || matches!(
                        self.item_state_in(tc, &report, k),
                        Some(ItemState::Cut | ItemState::Island)
                    )
            })
            .map(|k| k.clone().for_toolpath_model(tc.model_id))
            .collect())
    }

    /// The state of the item `key` (a key of `report.selected`) in
    /// `report`: from `foreign`, or from `item_states` by the item index in
    /// the toolpath model.
    fn item_state_in(
        &self,
        tc: &ToolpathConfig,
        report: &SelectionReport,
        key: &MatchKey,
    ) -> Option<ItemState> {
        if let Some((_, state)) = report.foreign.iter().find(|(k, _)| k.matches(key)) {
            return Some(*state);
        }
        if key.source != SourceRef::Model(tc.model_id) {
            return None;
        }
        let model = self.model_by_id(tc.model_id)?;
        let index = model
            .items
            .iter()
            .position(|item| item.key().names_same_geometry(key))?;
        report.item_states.get(index).copied()
    }

    /// What a viewport click on `key` does to the selection `target` of
    /// `tc` (geometry selection P4.D). The GUI click and the GUI pick bar
    /// read it; MCP has no click act (its `edit_toolpath_selection` is an
    /// explicit edit and keeps its refusal).
    ///
    /// Ruling L-8 revised (operator, 2026-10-20): a click on the boundary
    /// target of a toolpath whose boundary source is not `Selection` sets
    /// the source to `Selection` with that one pick
    /// ([`ClickEdit::BoundaryToSelection`]). Every other field of the
    /// boundary stays as stored, `enabled` too. Every other click is
    /// [`Self::pick_edit`].
    pub fn click_edit(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: MatchKey,
    ) -> Result<ClickEdit, SessionError> {
        let boundary_is_selection = matches!(tc.boundary.source, BoundarySource::Selection { .. });
        if target == SelectionTarget::Boundary && !boundary_is_selection {
            let kind = key.kind;
            let mut boundary = tc.boundary.clone();
            boundary.source = BoundarySource::Selection {
                selection: Selection::from_picks(vec![key]).for_toolpath_model(tc.model_id),
                holes: false,
            };
            return Ok(ClickEdit::BoundaryToSelection {
                boundary: Box::new(boundary),
                kind,
            });
        }
        self.pick_edit(tc, target, key).map(ClickEdit::Selection)
    }

    /// The edit that a viewport shift-click on `key` sends to the selection
    /// `target` of `tc` (geometry selection P4.5): the layer of the item as
    /// a rule, for the kinds `kinds` (the chips that are on) and, for the
    /// circles, the diameter range `circle` ([`Rule::layer_rules`](crate::selection::rule::Rule::layer_rules)).
    ///
    /// Under the default selection, the first layer rule replaces the
    /// default ([`SelectionEdit::add_rules`], the rule of the P4.3
    /// `+ Rule` card). The default of the op target is the op default; the
    /// default of the boundary target is
    /// [`Self::boundary_selection_default`] (the `Reset` value).
    ///
    /// Refusals (the GUI gives a notice and no edit): an item with no
    /// layer, no kind, an op that holds no selection, a boundary that is
    /// not a selection.
    pub fn layer_rule_edit(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: &MatchKey,
        kinds: &[ItemKind],
        circle: Option<crate::selection::DiameterRange>,
    ) -> Result<SelectionEdit, SessionError> {
        let Some(layer) = key.layer.as_deref() else {
            return Err(SessionError::InvalidParam(format!(
                "The {} is on no layer, so no layer rule can name it",
                ItemRef::new(key.clone())
            )));
        };
        let no_selection = || {
            SessionError::InvalidParam(format!(
                "Toolpath '{}' holds no selection: its operation cuts no drawing item",
                tc.name
            ))
        };
        let current = tc.selection_of(target)?.ok_or_else(no_selection)?;
        let op_default = self
            .selection_default_of(tc, target)
            .ok_or_else(no_selection)?;
        let rules = crate::selection::Rule::layer_rules(key.source, layer, kinds, circle);
        SelectionEdit::add_rules(current, &op_default, tc.model_id, rules).ok_or_else(|| {
            SessionError::InvalidParam("No kind is on, so the layer rule names no item".to_owned())
        })
    }

    /// The items of the model with id `model_id`, in import order, with
    /// an optional layer and kind filter: the `list_model_items` read.
    pub fn list_model_items(
        &self,
        model_id: ModelId,
        layer: Option<&str>,
        kind: Option<ItemKind>,
    ) -> Result<Vec<ModelItem>, SessionError> {
        self.list_model_items_near(model_id, layer, kind, None)
    }

    /// [`Self::list_model_items`] with an optional [`Near`] filter
    /// (geometry selection P4.5): with `near`, only the items whose drawn
    /// lines pass within `near.radius_mm` of `near.xy`, nearest first, by
    /// the pick rule of a viewport click ([`crate::selection::items_near`]).
    /// So `near` is the MCP form of a click.
    ///
    /// P4.7: with `kind: Face` on a mesh with no BREP faces (an STL), the
    /// items are the flat-region faces, built on a cold cache (an explicit
    /// ask for faces builds, as generation does). With any other kind the
    /// read lists `LoadedModel::items`, so it lists no face of an STL.
    pub fn list_model_items_near(
        &self,
        model_id: ModelId,
        layer: Option<&str>,
        kind: Option<ItemKind>,
        near: Option<Near>,
    ) -> Result<Vec<ModelItem>, SessionError> {
        let model = self
            .model_by_id(model_id)
            .ok_or_else(|| SessionError::model_not_found(model_id, self))?;
        let source: Arc<ItemSet> = if kind == Some(ItemKind::Face) && model.faces_are_flat_regions()
        {
            model.face_items_built()
        } else {
            Arc::clone(&model.items)
        };
        let admitted = source
            .iter()
            .filter(|item| layer.is_none_or(|want| item.layer() == Some(want)))
            .filter(|item| kind.is_none_or(|want| item.kind() == want));
        let ordered: Vec<&crate::selection::Item> = match near {
            None => admitted.collect(),
            Some(near) => {
                let admitted: Vec<&crate::selection::Item> = admitted.collect();
                crate::selection::items_near(
                    admitted.iter().copied(),
                    P2::new(near.xy[0], near.xy[1]),
                    near.radius_mm,
                )
                .into_iter()
                .filter_map(|(index, _)| admitted.get(index).copied())
                .collect()
            }
        };
        // R-E: the index of each item in `source`, for the hole fold.
        let index_of =
            |item: &crate::selection::Item| source.iter().position(|i| std::ptr::eq(i, item));
        let rows: Vec<(Option<usize>, ModelItem)> = ordered
            .into_iter()
            .map(|item| {
                let at = index_of(item);
                let row = ModelItem {
                    key: MatchKey {
                        source: SourceRef::Model(model_id),
                        ..item.key().clone()
                    },
                    kind: item.kind(),
                    layer: item.layer().map(str::to_owned),
                    measure: item.key().measure,
                    hole: at
                        .and_then(|at| HoleSummary::of(&source, at, SourceRef::Model(model_id))),
                    outline: item.is_part_outline(),
                };
                (at, row)
            })
            .collect();
        Ok(fold_hole_floors(rows, &source, |(at, _)| *at)
            .into_iter()
            .map(|(_, row)| row)
            .collect())
    }
}

/// R-E: `rows` less each floor circle of a hole whose rim is also a row
/// ([`ItemSet::rim_of`], THE pairing rule). `index` gives the index of a
/// row in `items`, or `None` for a row of another source.
fn fold_hole_floors<T>(
    rows: Vec<T>,
    items: &ItemSet,
    index: impl Fn(&T) -> Option<usize>,
) -> Vec<T> {
    let present: std::collections::BTreeSet<usize> = rows.iter().filter_map(&index).collect();
    rows.into_iter()
        .filter(|row| {
            index(row)
                .and_then(|at| items.rim_of(at))
                .is_none_or(|rim| !present.contains(&rim))
        })
        .collect()
}

/// What the `build_model_faces` job captured (geometry selection P4.7):
/// the mesh whose flat-region faces it computes. It owns the mesh, so
/// [`execute_build_model_faces`] holds no session and runs off the frame
/// loop.
pub struct BuildModelFacesHandle {
    model_id: ModelId,
    mesh: Arc<crate::mesh::TriangleMesh>,
}

impl BuildModelFacesHandle {
    /// The model id the job builds the faces of.
    #[must_use]
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }
}

/// The answer of the `build_model_faces` job. The faces themselves go into
/// the per-mesh cache (`maps::geom_cache`), which every reader peeks; the
/// answer only names the model and the count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFacesBuilt {
    pub model_id: ModelId,
    /// The count of the flat-region faces of the mesh.
    pub faces: usize,
}

impl ProjectSession {
    /// Step (i) of the `build_model_faces` job: capture the mesh of model
    /// `model_id`. A model that is not a mesh with no BREP faces refuses:
    /// its faces are in `items` and need no build.
    pub(crate) fn capture_build_model_faces(
        &self,
        model_id: ModelId,
    ) -> Result<BuildModelFacesHandle, SessionError> {
        let model = self
            .model_by_id(model_id)
            .ok_or_else(|| SessionError::model_not_found(model_id, self))?;
        match (&model.mesh, model.faces_are_flat_regions()) {
            (Some(mesh), true) => Ok(BuildModelFacesHandle {
                model_id,
                mesh: Arc::clone(mesh),
            }),
            _ => Err(SessionError::InvalidParam(format!(
                "Model {model_id} holds its faces in its items; it has no flat regions to build"
            ))),
        }
    }
}

/// Step (ii) of the `build_model_faces` job (geometry selection P4.7):
/// fill the flat-region cache of the captured mesh
/// (`maps::geom_cache::cached_flat_regions`). It reads no session. A set
/// cancel flag stops it before the build; the build itself is one union
/// per plane and does not poll the flag.
pub fn execute_build_model_faces(
    handle: &BuildModelFacesHandle,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<ModelFacesBuilt, SessionError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(SessionError::Operation(
            crate::compute::execute::OperationError::Cancelled,
        ));
    }
    let faces = crate::maps::geom_cache::cached_flat_regions(&handle.mesh).len();
    Ok(ModelFacesBuilt {
        model_id: handle.model_id,
        faces,
    })
}

/// Write `state` for one item: into `item_states` at `index` when the item
/// is an item of the toolpath model (`own`), else into `foreign` by key.
fn set_state(
    report: &mut SelectionReport,
    own: SourceRef,
    (source, index, key): (SourceRef, usize, &MatchKey),
    state: ItemState,
    floor_z: Option<f64>,
) {
    if source == own
        && let Some(slot) = report.item_states.get_mut(index)
    {
        *slot = state;
        if let Some(floor) = report.item_floor_z.get_mut(index) {
            *floor = floor_z;
        }
        return;
    }
    report.foreign.push((
        MatchKey {
            source,
            ..key.clone()
        },
        state,
    ));
    report.foreign_floor_z.push(floor_z);
}

/// True when the prologue of `op_type` runs `view` (its row reads items).
fn reads_items(op_type: OperationType) -> bool {
    match op_type.geometry_requirement() {
        GeometryRequirement::Items | GeometryRequirement::ItemsAndMesh => true,
        GeometryRequirement::Mesh => false,
    }
}

impl ToolpathConfig {
    /// The stored selection `target` of this toolpath (P4.1). `Ok(None)`:
    /// an op that holds no selection (it resolves to no item). A boundary
    /// whose source is not a selection refuses
    /// ([`boundary_not_a_selection`]).
    pub fn selection_of(
        &self,
        target: SelectionTarget,
    ) -> Result<Option<&Selection>, SessionError> {
        match target {
            SelectionTarget::Op => Ok(self.operation.selection()),
            SelectionTarget::Boundary => match &self.boundary.source {
                BoundarySource::Selection { selection, .. } => Ok(Some(selection)),
                BoundarySource::Stock
                | BoundarySource::ModelSilhouette
                | BoundarySource::DerivedRestRegions { .. }
                | BoundarySource::PlannedTierRegions { .. } => {
                    Err(boundary_not_a_selection(&self.name))
                }
            },
        }
    }
}

/// A copy of `tc` whose selection `target` is `selection` (the GT-Q8
/// probe of one rule). A boundary that is not a selection, or an op with
/// no selection slot, stays as it is.
pub(super) fn with_selection(
    tc: &ToolpathConfig,
    target: SelectionTarget,
    selection: Selection,
) -> ToolpathConfig {
    let mut probe = tc.clone();
    match target {
        SelectionTarget::Op => {
            if let Some(slot) = probe.operation.selection_slot() {
                *slot = selection;
            }
        }
        SelectionTarget::Boundary => {
            if let BoundarySource::Selection {
                selection: slot, ..
            } = &mut probe.boundary.source
            {
                *slot = selection;
            }
        }
    }
    probe
}

/// P6.4: the outer ring of each closed area of `items` whose exterior item
/// is an open path (an area that the extend reader closed by the stock
/// edge), and of each face area that "Cut past open sides" grew to the
/// stock edge (`Regions::is_grown`), mapped from the SETUP frame back to the drawing frame
/// (`SetupTransformInfo::local_to_drawing`, the exact inverse of the frame
/// door). Empty for a view with no such area.
fn extended_areas(
    setup: &ResolvedSelection<SetupFrame>,
    items: &crate::selection::OpItems,
    ctx: &SetupEvalContext,
) -> Vec<Vec<P2>> {
    let Some(regions) = items.regions() else {
        return Vec::new();
    };
    let info = ctx
        .local_to_global
        .as_ref()
        .filter(|info| info.needs_transform());
    regions
        .as_slice()
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            regions.is_grown(*index)
                || regions
                    .exterior_item(*index)
                    .and_then(|at| setup.items().get(at))
                    .is_some_and(|r| r.item().kind() == ItemKind::Path)
        })
        .map(|(_, polygon)| match info {
            Some(info) => polygon
                .exterior
                .iter()
                .map(|p| info.local_to_drawing(*p))
                .collect(),
            None => polygon.exterior.clone(),
        })
        .collect()
}

/// The bit patterns of a box, for an exact compare.
fn box_bits(b: &BoundingBox3) -> [u64; 6] {
    [
        b.min.x.to_bits(),
        b.min.y.to_bits(),
        b.min.z.to_bits(),
        b.max.x.to_bits(),
        b.max.y.to_bits(),
        b.max.z.to_bits(),
    ]
}

/// Every input of one report (see the module documentation).
struct StatusKey {
    target: SelectionTarget,
    selection: Vec<u8>,
    op_type: OperationType,
    item_use: ItemUse,
    /// R-A: the outline use (a Profile side edit changes it) and the
    /// "frees the part" answer (a floor or a tab edit changes it).
    outline_use: OutlineUse,
    frees_the_part: bool,
    model_id: ModelId,
    models: Vec<(ModelId, Weak<ItemSet>)>,
    /// P4.7: the cached flat-region set of each mesh model the selection
    /// names a face on, or `None` while it is cold.
    flat_faces: Vec<(ModelId, Option<Weak<ItemSet>>)>,
    face_up: FaceUp,
    z_rotation: ZRotation,
    world_stock: [u64; 6],
    emission_stock: [u64; 6],
    /// P3.5: under the floor "each picked face", the floor and the Top Z
    /// config: the per-area floor check reads both. UX-8: under the floor
    /// "A picked face", the floor (its key refuses or resolves). R-B: on a
    /// Drill or a Bore, the floor and the Top Z config (the per-hole
    /// floors). Empty for every other floor, so a heights edit does not
    /// rebuild the report.
    each_face: Vec<u8>,
    /// P6.5: the bits of the tool diameter that the hole fit step reads
    /// (`ProjectSession::hole_fit_size`), and the bits of the size
    /// tolerance of the op (operator ruling 2026-10-22), so a tolerance
    /// edit builds the report again. `None` for every op with no
    /// fit rule, so a tool edit does not rebuild their reports.
    drill_size: Option<[u64; 2]>,
    /// P6.4: the bits of the tool radius by which an extend reader grows
    /// the stock (`ProjectSession::extend_radius`), so a tool edit builds
    /// the report again. `None` for every other op.
    extend_radius: Option<u64>,
    /// Chamfer Clearance: the tool and the bevel (`w`, `t`) that the wall
    /// trim reads, so a tool or a bevel edit builds the report again.
    /// Empty for every op with no model edges.
    edge_clear: Vec<u8>,
    /// "Cut past open sides": the bits of the tool radius of THE area rule
    /// (`ProjectSession::open_sides_radius`), so a tool edit or the
    /// setting builds the report again. `None` when the setting is off.
    open_sides: Option<u64>,
}

impl StatusKey {
    /// The key of `tc` in `session` as they are now, or `None` when the
    /// selection does not serialize (the memo then keeps nothing).
    fn capture(
        session: &ProjectSession,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        ctx: &SetupEvalContext,
    ) -> Option<Self> {
        let (item_use, outline_use) = target_uses(tc, target);
        let spec = tc.selection_of(target).ok()?;
        // UX-8: the face key of the floor "A picked face" resolves as a
        // one-pick selection, so its model's flat faces are a key part too.
        let floor_spec = match (target, tc.operation.floor()) {
            (SelectionTarget::Op, Some(Floor::Face { key, .. })) => {
                Some(Selection::from_picks(vec![key]))
            }
            _ => None,
        };
        let flat_faces = spec
            .into_iter()
            .chain(floor_spec.as_ref())
            .filter(|sel| sel.names_faces())
            .flat_map(|sel| session.face_models_of(tc, sel))
            .map(|m| {
                let peeked = m
                    .mesh
                    .as_ref()
                    .and_then(crate::maps::geom_cache::peek_flat_regions);
                (m.id, peeked.as_ref().map(Arc::downgrade))
            })
            .collect();
        Some(Self {
            target,
            selection: serde_json::to_vec(&spec).ok()?,
            op_type: tc.operation.op_type(),
            item_use,
            outline_use,
            frees_the_part: target == SelectionTarget::Op && tc.operation.frees_the_part(),
            model_id: tc.model_id,
            models: session
                .models
                .iter()
                .map(|m| (m.id, Arc::downgrade(&m.items)))
                .collect(),
            flat_faces,
            face_up: ctx.face_up,
            z_rotation: ctx.z_rotation,
            world_stock: box_bits(&ctx.world_stock_bbox),
            emission_stock: box_bits(&ctx.heights_stock_bbox),
            each_face: match (target, tc.operation.floor()) {
                (
                    SelectionTarget::Op,
                    Some(floor @ (Floor::EachFace { .. } | Floor::Face { .. })),
                ) => serde_json::to_vec(&(floor, &tc.heights.top_z)).ok()?,
                // R-B: the per-hole floors read the floor and the Top Z.
                (SelectionTarget::Op, Some(floor)) if matches!(item_use, ItemUse::Holes { .. }) => {
                    serde_json::to_vec(&(floor, &tc.heights.top_z)).ok()?
                }
                _ => Vec::new(),
            },
            drill_size: match target {
                SelectionTarget::Op => session
                    .hole_fit_size(tc)
                    .map(|size| [size.diameter.to_bits(), size.tolerance.to_bits()]),
                SelectionTarget::Boundary => None,
            },
            extend_radius: match target {
                SelectionTarget::Op => session.extend_radius(tc).map(f64::to_bits),
                SelectionTarget::Boundary => None,
            },
            edge_clear: match (target, tc.operation.bevel_width_and_tip_mm()) {
                (SelectionTarget::Op, Some(bevel)) if item_use == ItemUse::Edges => {
                    serde_json::to_vec(&(session.toolpath_tool(tc), bevel)).ok()?
                }
                _ => Vec::new(),
            },
            open_sides: match target {
                SelectionTarget::Op => session.open_sides_radius(tc).map(f64::to_bits),
                SelectionTarget::Boundary => None,
            },
        })
    }

    /// Does the stored key describe the same inputs as `now`? A model item
    /// set compares by allocation: a live `Weak` keeps the address
    /// reserved, so no later set can take it (no ABA).
    fn answers(&self, now: &StatusKey) -> bool {
        self.target == now.target
            && self.selection == now.selection
            && self.op_type == now.op_type
            && self.item_use == now.item_use
            && self.outline_use == now.outline_use
            && self.frees_the_part == now.frees_the_part
            && self.model_id == now.model_id
            && self.face_up == now.face_up
            && self.z_rotation == now.z_rotation
            && self.world_stock == now.world_stock
            && self.emission_stock == now.emission_stock
            && self.each_face == now.each_face
            && self.drill_size == now.drill_size
            && self.extend_radius == now.extend_radius
            && self.edge_clear == now.edge_clear
            && self.open_sides == now.open_sides
            && self.models.len() == now.models.len()
            && self
                .models
                .iter()
                .zip(&now.models)
                .all(|((id, set), (now_id, now_set))| id == now_id && set.ptr_eq(now_set))
            && self.flat_faces.len() == now.flat_faces.len()
            && self
                .flat_faces
                .iter()
                .zip(&now.flat_faces)
                .all(|((id, set), (now_id, now_set))| {
                    id == now_id
                        && match (set, now_set) {
                            (None, None) => true,
                            (Some(a), Some(b)) => a.ptr_eq(b),
                            (None, Some(_)) | (Some(_), None) => false,
                        }
                })
    }
}

/// The stored report of one toolpath and the key it answers.
struct StatusEntry {
    key: StatusKey,
    report: Arc<SelectionReport>,
}

/// The session's selection-status memo, one entry per toolpath and
/// target.
///
/// A clone of the session starts with an empty memo. The memo is derived
/// data, and a what-if copy diverges from its source at once.
#[derive(Default)]
pub(crate) struct SelectionStatusMemo {
    entries: Mutex<BTreeMap<(ToolpathId, SelectionTarget), StatusEntry>>,
    /// How many times the memo built a report. Read by the sentries
    /// through [`ProjectSession::selection_status_builds`].
    builds: AtomicU64,
}

impl Clone for SelectionStatusMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}
