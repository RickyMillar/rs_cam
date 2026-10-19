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
//! - the setup orientation and the stock box in both frames (the face
//!   filter and the frame door read them).
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

use crate::compute::catalog::{GeometryRequirement, ItemUse, OperationType};
use crate::compute::config::BoundarySource;
use crate::compute::transform::{FaceUp, ZRotation};
use crate::geo::{BoundingBox3, P2};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::{ModelId, ToolpathId};
use crate::selection::{
    Hole, ItemError, ItemKind, ItemRef, ItemSet, MatchKey, RefusalCause, ResolveScope,
    ResolvedSelection, Selection, SelectionEdit, SelectionNote, SelectionRefusal, Shape, SourceRef,
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
    /// with no selection slot: Face and every 3D op).
    pub fn for_face_pick(tc: &ToolpathConfig) -> Self {
        match tc.operation.selection() {
            Some(_) => Self::Op,
            None => Self::Boundary,
        }
    }
}

/// The `ItemUse` that reads a boundary selection: closed regions
/// (`resolve_boundary_selection_polys` and the boundary report).
pub(crate) const BOUNDARY_ITEM_USE: ItemUse = ItemUse::Regions;

/// The name that a refusal of a boundary selection gives as its op.
pub(crate) const BOUNDARY_LABEL: &str = "Machining boundary";

/// The refusal of a boundary edit or read when the boundary source of
/// `name` is not a selection. No command changes the source in silence.
pub(crate) fn boundary_not_a_selection(name: &str) -> SessionError {
    SessionError::InvalidParam(format!(
        "the boundary of '{name}' is not a selection: set the boundary source to Selection"
    ))
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
    /// The holes that the op drills, in the MODEL frame. Empty for an op
    /// that drills no hole, and when the selection refuses.
    pub holes: Vec<Hole>,
    /// The key of every item the selection holds (its `identity`, in the
    /// frame and with the source of the drawing: `SourceRef::Model(id)` or
    /// `SourceRef::Stock`), in resolution order. A viewport click reads it
    /// to choose between `AddPick` and `RemovePick`
    /// ([`ProjectSession::pick_edit`]); the face highlight reads its face
    /// keys. Empty when the selection refuses before it resolves.
    pub selected: Vec<MatchKey>,
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
}

/// The two frames of one resolved selection.
pub(crate) struct SelectionFrames {
    /// As the drawing holds the items (the drill picker reads it).
    pub(crate) model: ResolvedSelection,
    /// After THE frame door: what the prologue receives.
    pub(crate) setup: ResolvedSelection,
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
    ) -> Result<SelectionFrames, SessionError> {
        let empty = Selection::empty();
        let spec = tc.selection_of(target)?.unwrap_or(&empty);
        self.resolve_spec_frames(tc, spec, ctx)
            .map_err(|e| SessionError::OperationFailed(e.to_string()))
    }

    /// [`Self::resolve_selection_frames`] for one stored selection `spec`
    /// of `tc`. The boundary generation (`resolve_boundary_selection_polys`)
    /// and the status report share it, so the two cannot differ. `Err` is
    /// an item that the frame door cannot move.
    pub(crate) fn resolve_spec_frames(
        &self,
        tc: &ToolpathConfig,
        spec: &Selection,
        ctx: &SetupEvalContext,
    ) -> Result<SelectionFrames, ItemError> {
        let resolved = crate::selection::resolve(
            spec,
            &ResolveScope {
                toolpath_model: self.toolpath_model(tc),
                models: &self.models,
                stock_bbox: &ctx.heights_stock_bbox,
                setup: ctx.local_to_global.as_ref(),
            },
        );
        let setup = match ctx.local_to_global.as_ref() {
            Some(info) => resolved.clone().to_setup_frame(info)?,
            None => resolved.clone(),
        };
        Ok(SelectionFrames {
            model: resolved,
            setup,
        })
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
        if tc.operation.selection().is_some() || !tc.boundary.enabled {
            return Ok(None);
        }
        match &tc.boundary.source {
            BoundarySource::Selection { .. } => Ok(self
                .resolve_selection_frames(tc, SelectionTarget::Boundary, ctx)?
                .setup
                .face_top_z()),
            BoundarySource::Stock
            | BoundarySource::ModelSilhouette
            | BoundarySource::DerivedRestRegions { .. }
            | BoundarySource::PlannedTierRegions { .. } => Ok(None),
        }
    }

    /// The selection that a boundary `Reset` restores, and that the
    /// "Selection" boundary source writes (P4.3): the closed shapes of the
    /// first model that has a closed ring, else no item. With no item the
    /// boundary refuses `Empty` until the first pick.
    pub fn boundary_selection_default(&self) -> Selection {
        self.models
            .iter()
            .find(|m| m.items.has_closed_ring())
            .map_or_else(Selection::empty, |m| {
                Selection::closed_shapes_of(ModelId(m.id))
            })
    }

    /// The model of `tc` by its EXACT id, or the id when no model has it.
    /// No fallback to another model (geometry selection P2.4).
    pub(crate) fn toolpath_model(&self, tc: &ToolpathConfig) -> Result<&LoadedModel, ModelId> {
        self.find_model_by_raw_id(tc.model_id)
            .ok_or(ModelId(tc.model_id))
    }

    /// [`Self::resolve_selection_frames`] for `tc` in the setup that owns
    /// it, in the MODEL frame.
    pub(crate) fn resolve_toolpath_selection(
        &self,
        tc: &ToolpathConfig,
    ) -> Result<ResolvedSelection, SessionError> {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        Ok(self
            .resolve_selection_frames(tc, SelectionTarget::Op, &ctx)?
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
    fn build_selection_report(
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
        let frames = self.resolve_selection_frames(tc, target, ctx)?;
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
        let mut report = SelectionReport {
            hole_items,
            selected,
            ..SelectionReport::default()
        };
        report.lost = frames
            .model
            .lost()
            .iter()
            .map(|lost| ItemRef::new(lost.key.clone()))
            .collect();
        for r in frames.setup.items() {
            let kind = r.item().shape().kind();
            if kind.serves(item_use) {
                *report.counts.entry(kind).or_insert(0) += 1;
            }
        }
        if !runs_view {
            return Ok(report);
        }
        match crate::selection::view(&frames.setup, item_use) {
            Ok((items, notes)) => {
                report.skipped_by_kind = items.skipped_by_kind().clone();
                report.notes = notes;
                report.islands = items
                    .regions()
                    .map_or(0, |r| r.as_slice().iter().map(|p| p.holes.len()).sum());
                if items.holes().is_some()
                    && let Ok((model_items, _)) = crate::selection::view(&frames.model, item_use)
                {
                    report.holes = model_items
                        .holes()
                        .map(|h| h.as_slice().to_vec())
                        .unwrap_or_default();
                }
            }
            Err(refusal) => report.refusal = Some(refusal.for_op(label)),
        }
        Ok(report)
    }

    /// Every point and circle of the toolpath model of a drill op, in the
    /// MODEL frame, each marked when `selected` holds it. Empty for every
    /// other op.
    fn hole_items(&self, tc: &ToolpathConfig, selected: &[MatchKey]) -> Vec<HoleItem> {
        match tc.operation.capabilities().item_use {
            ItemUse::Holes { .. } => {}
            ItemUse::None | ItemUse::Regions | ItemUse::Curves => return Vec::new(),
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
            ItemUse::None | ItemUse::Regions | ItemUse::Curves => return None,
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
        model_id: usize,
        face: FaceGroupId,
    ) -> Result<MatchKey, SelectionRefusal> {
        let model = self.find_model_by_raw_id(model_id).ok_or_else(|| {
            SelectionRefusal::new(
                None,
                RefusalCause::MissingSource(SourceRef::Model(ModelId(model_id))),
            )
        })?;
        let source = if model_id == tc.model_id {
            SourceRef::ToolpathModel
        } else {
            SourceRef::Model(ModelId(model_id))
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

    /// The edit that a viewport click on `key` sends to the selection
    /// `target` of `tc` (lead ruling P2.2: no toggle edit in the selection;
    /// P4.1: the target). It reads
    /// THE resolution through the memo ([`Self::selection_status`]): an
    /// item that the selection holds gives `RemovePick` (a rule match then
    /// gets an exclude), any other item gives `AddPick`.
    ///
    /// `key` names its source as a stored pick does. The source
    /// [`SourceRef::ToolpathModel`] reads as the model of `tc`.
    pub fn pick_edit(
        &self,
        tc: &ToolpathConfig,
        target: SelectionTarget,
        key: MatchKey,
    ) -> Result<SelectionEdit, SessionError> {
        let source = match key.source {
            SourceRef::ToolpathModel => SourceRef::Model(ModelId(tc.model_id)),
            other @ (SourceRef::Model(_) | SourceRef::Stock) => other,
        };
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

    /// The items of the model with id `model_id`, in import order, with
    /// an optional layer and kind filter: the `list_model_items` read.
    pub fn list_model_items(
        &self,
        model_id: usize,
        layer: Option<&str>,
        kind: Option<ItemKind>,
    ) -> Result<Vec<ModelItem>, SessionError> {
        let model = self.find_model_by_raw_id(model_id).ok_or_else(|| {
            let known: Vec<String> = self.models.iter().map(|m| m.id.to_string()).collect();
            SessionError::MissingGeometry(format!(
                "No model has id {model_id}; the model ids are [{}]",
                known.join(", ")
            ))
        })?;
        Ok(model
            .items
            .iter()
            .filter(|item| layer.is_none_or(|want| item.layer() == Some(want)))
            .filter(|item| kind.is_none_or(|want| item.kind() == want))
            .map(|item| ModelItem {
                key: MatchKey {
                    source: SourceRef::Model(ModelId(model_id)),
                    ..item.key().clone()
                },
                kind: item.kind(),
                layer: item.layer().map(str::to_owned),
                measure: item.key().measure,
            })
            .collect())
    }
}

/// True when the prologue of `op_type` runs `view` (its row reads items).
fn reads_items(op_type: OperationType) -> bool {
    match op_type.geometry_requirement() {
        GeometryRequirement::Items | GeometryRequirement::ItemsAndMesh => true,
        GeometryRequirement::Stock | GeometryRequirement::Mesh => false,
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
    model_id: usize,
    models: Vec<(usize, Weak<ItemSet>)>,
    face_up: FaceUp,
    z_rotation: ZRotation,
    world_stock: [u64; 6],
    emission_stock: [u64; 6],
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
        let item_use = match target {
            SelectionTarget::Op => tc.operation.capabilities().item_use,
            SelectionTarget::Boundary => BOUNDARY_ITEM_USE,
        };
        Some(Self {
            target,
            selection: serde_json::to_vec(&tc.selection_of(target).ok()?).ok()?,
            op_type: tc.operation.op_type(),
            item_use,
            model_id: tc.model_id,
            models: session
                .models
                .iter()
                .map(|m| (m.id, Arc::downgrade(&m.items)))
                .collect(),
            face_up: ctx.face_up,
            z_rotation: ctx.z_rotation,
            world_stock: box_bits(&ctx.world_stock_bbox),
            emission_stock: box_bits(&ctx.heights_stock_bbox),
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
            && self.model_id == now.model_id
            && self.face_up == now.face_up
            && self.z_rotation == now.z_rotation
            && self.world_stock == now.world_stock
            && self.emission_stock == now.emission_stock
            && self.models.len() == now.models.len()
            && self
                .models
                .iter()
                .zip(&now.models)
                .all(|((id, set), (now_id, now_set))| id == now_id && set.ptr_eq(now_set))
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
