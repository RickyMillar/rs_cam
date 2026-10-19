//! The selection of a toolpath, resolved for the readers that do not
//! generate (geometry selection P2.3c). P2.4 grows this into the
//! `SelectionStatus` query.
//!
//! Generation, the precondition check, the dowel-hole check (P8) and the
//! GUI drill picker read ONE resolution: [`ProjectSession::resolve_op_selection`]
//! (the op's own selection, plus its picked faces as picks) and the pure
//! [`crate::selection::view`]. So the Generate button, the diagnostics
//! ribbon, the viewport markers and the generator cannot say different
//! things about which holes an op drills (the contract that
//! `drill_targets_refusal` held before P2.3c).
//!
//! The readers here stay in the MODEL frame: a stored pick is a key in the
//! model frame, and the dowel-hole check measures a drawing hole against
//! the stock pins in the stock-local frame of the part. Generation moves
//! the same resolution into the setup frame (`to_setup_frame`).

use crate::compute::catalog::ItemUse;
use crate::compute::transform::SetupTransformInfo;
use crate::geo::{BoundingBox3, P2};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::selection::{
    Hole, MatchKey, ResolveScope, ResolvedSelection, Selection, SelectionEdit, Shape, SourceRef,
};
use crate::session::{LoadedModel, ProjectSession, SessionError, ToolpathConfig};

/// The pick keys of the picked BREP faces of `model` (geometry selection
/// P2.3b). Each key names the Face item whose BREP id is the picked id, on
/// the toolpath model. A picked face with no Face item refuses and names
/// the face: the STEP door makes a Face item for each flat horizontal face
/// only (`io::brep_face_items`), and a pick that the op cannot cut must
/// not drop out in silence.
pub(crate) fn face_pick_keys(
    toolpath: &str,
    model: Option<&LoadedModel>,
    ids: &[FaceGroupId],
) -> Result<Vec<MatchKey>, SessionError> {
    let Some(model) = model else {
        return Err(SessionError::MissingGeometry(format!(
            "Toolpath '{toolpath}' picks model faces, but its model does not exist. Pick a \
             model for the toolpath, or clear the face pick."
        )));
    };
    ids.iter()
        .map(|id| {
            model
                .items
                .iter()
                .find(|item| match item.shape() {
                    Shape::Face(face) => face.brep == Some(*id),
                    Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => {
                        false
                    }
                })
                .map(|item| MatchKey {
                    source: SourceRef::ToolpathModel,
                    ..item.key().clone()
                })
                .ok_or_else(|| {
                    SessionError::MissingGeometry(format!(
                        "Toolpath '{toolpath}' picks face {} of model '{}', but that face is not \
                         a flat horizontal face, so it has no outline to cut. Remove the face \
                         from the pick.",
                        id.0, model.name
                    ))
                })
        })
        .collect()
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
/// frame.
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

impl ProjectSession {
    /// The selection of `tc` as generation resolves it, in the MODEL frame:
    /// the op's own selection (`OperationConfig::selection`) with each
    /// picked face added as a pick, against the items of the models and
    /// the stock. An op that holds no selection resolves its picked faces
    /// only.
    ///
    /// `model` is the toolpath model, `stock_bbox` the stock in the
    /// emission frame (the stock source) and `setup` the setup transform
    /// (the face filter reads the setup frame). Generation calls this,
    /// then moves the result into the setup frame (`to_setup_frame`).
    pub(crate) fn resolve_op_selection(
        &self,
        tc: &ToolpathConfig,
        model: Option<&LoadedModel>,
        face_picks: Vec<MatchKey>,
        stock_bbox: &BoundingBox3,
        setup: Option<&SetupTransformInfo>,
    ) -> Result<ResolvedSelection, SessionError> {
        let mut spec = tc
            .operation
            .selection()
            .cloned()
            .unwrap_or_else(Selection::empty);
        let op_default = tc
            .operation
            .default_selection()
            .unwrap_or_else(Selection::empty);
        for key in face_picks {
            spec.apply(SelectionEdit::AddPick(key), &op_default)
                .map_err(|e| SessionError::OperationFailed(e.to_string()))?;
        }
        Ok(crate::selection::resolve(
            &spec,
            &ResolveScope {
                toolpath_model: model,
                models: &self.models,
                stock_bbox,
                setup,
            },
        ))
    }

    /// [`Self::resolve_op_selection`] for `tc` with the setup, the stock and
    /// the picked faces that generation reads, in the MODEL frame.
    pub(crate) fn resolve_toolpath_selection(
        &self,
        tc: &ToolpathConfig,
    ) -> Result<ResolvedSelection, SessionError> {
        let model = self.find_model_by_raw_id(tc.model_id);
        let face_picks = match tc.face_selection.as_deref() {
            Some(ids) if !ids.is_empty() => face_pick_keys(&tc.name, model, ids)?,
            Some(_) | None => Vec::new(),
        };
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        self.resolve_op_selection(
            tc,
            model,
            face_picks,
            &ctx.heights_stock_bbox,
            ctx.local_to_global.as_ref(),
        )
    }

    /// The holes of a drill op (`ItemUse::Holes`), as generation resolves
    /// them, in the MODEL frame. `None` for an op that drills no hole.
    ///
    /// The GUI drill picker, its viewport markers, the precondition check
    /// and the dowel-hole check read this, so each says what the generator
    /// does.
    pub fn hole_selection(&self, tc: &ToolpathConfig) -> Option<HoleSelection> {
        let item_use = tc.operation.capabilities().item_use;
        match item_use {
            ItemUse::Holes { .. } => {}
            ItemUse::None | ItemUse::Regions | ItemUse::Curves => return None,
        }
        let op = tc.operation.op_type().label();
        let resolved = match self.resolve_toolpath_selection(tc) {
            Ok(resolved) => resolved,
            Err(error) => {
                return Some(HoleSelection {
                    refusal: Some(error.to_string()),
                    ..HoleSelection::default()
                });
            }
        };
        let (holes, refusal) = match crate::selection::view(&resolved, item_use) {
            Ok((items, _)) => (
                items
                    .holes()
                    .map(|h| h.as_slice().to_vec())
                    .unwrap_or_default(),
                None,
            ),
            Err(refusal) => (Vec::new(), Some(refusal.for_op(op).to_string())),
        };
        let items = self
            .find_model_by_raw_id(tc.model_id)
            .map(|model| {
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
                        let selected = resolved
                            .items()
                            .iter()
                            .any(|r| r.identity().matches(item.key()));
                        Some(HoleItem {
                            key: MatchKey {
                                source: SourceRef::ToolpathModel,
                                ..item.key().clone()
                            },
                            xy,
                            diameter,
                            layer: item.layer().map(str::to_owned),
                            selected,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(HoleSelection {
            items,
            holes,
            lost_picks: resolved.lost().len(),
            refusal,
        })
    }
}
