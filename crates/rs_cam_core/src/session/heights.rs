//! THE heights of a toolpath, in its setup frame (geometry selection P3.1).
//!
//! One rule gives the resolved heights to generation, the GUI (the Heights
//! tab, the height diagram, the toolpath rows) and MCP. The rule is
//! [`ProjectSession::heights_in_setup`]:
//!
//! 1. The height context of the setup (`SetupEvalContext::height_context`):
//!    the stock in the emission frame and the MODEL box in the setup frame.
//!    The model box is the box of the mesh after the setup transform
//!    (`geom_cache::cached_transform`), as generation reads it. A drawing
//!    has no mesh, so its context holds no model Z (defect D-B: the GUI
//!    read the drawing box at Z 0 and generation read the stock).
//! 2. `HeightsConfig::resolve` on that context.
//! 3. An AUTO top follows the up faces of the selection (N12 item 1,
//!    DESIGN 7.2), and an auto bottom follows that top by the op depth
//!    (defect D-A: the GUI and MCP did not apply this step).
//!
//! P3.2: the floor of a Pocket, Profile, Zigzag or Adaptive 2D
//! (`compute::floor::Floor`) resolves against the context of step 1 into
//! `HeightContext::floor`, so the auto bottom of step 2 and of step 3 IS the
//! floor. A floor that does not resolve, or that sits at or above the top,
//! is a value of the answer ([`ToolpathHeights::floor_refusal`]);
//! generation refuses with it. A pinned Bottom Z on those four ops refuses
//! at every door ([`ToolpathConfig::floor_conflict`]).
//!
//! P3.3: [`ProjectSession::set_toolpath_floor`] is the door of the
//! `SetToolpathFloor` command (MCP `set_toolpath_floor`, the CLI job key
//! `floor`). It refuses an op with no floor and a floor that
//! `Floor::door_refusal` refuses. The GUI writes the floor through the
//! whole config (L-17). [`ToolpathHeights::floor_z`] is the read of the
//! resolved floor for every surface.
//!
//! P3.5: under the floor "each picked face" the floor resolves against the
//! lowest up face of the selection (`FloorRef::EachFace`): the Bottom Z is
//! the deepest face floor, and an auto top stays at the stock top (ruling
//! L-15). The selection rule (`compute::floor::region_floors`) refuses an
//! area with no face, skips a face at the top with a note (P3.7) and
//! refuses when every face is at the top, by item, in `selection_status`
//! and in the prologue; generation does not refuse them here first.
//!
//! L-4: a Top Z or a Bottom Z that reads the Model Top or the Model Bottom
//! refuses when the toolpath has no mesh. Before P3.1 generation used the
//! stock top there and the GUI used Z 0. The answer carries the refusal
//! ([`ToolpathHeights::model_reference_refusal`]) so that the Heights tab can
//! draw its rows and name it; generation refuses with it.
//!
//! Generation calls the rule with the inputs it holds (the setup mesh and
//! the face top of its own `Build` resolution). [`ProjectSession::toolpath_heights`]
//! is the read door for every other surface: it reads the face top from
//! the `selection_status` memo (a `Peek`, P4.7). An STL face whose flat
//! regions are not computed yet gives no top there until the GUI job
//! computes them; the report says `faces_not_ready` for that time.

use std::sync::Arc;

use crate::compute::BoundarySource;
use crate::compute::catalog::{FloorOwner, OperationType};
use crate::compute::config::{
    HeightContext, HeightMode, HeightOrderViolation, HeightReference, HeightsConfig,
    ResolvedHeights, added_refusals, height_order,
};
use crate::compute::floor::{Floor, FloorRef, FloorReferences, FloorRefusal};
use crate::mesh::TriangleMesh;
use crate::session::{Effects, ProjectSession, SessionError, ToolpathConfig};

use super::SetupEvalContext;
use super::selection_status::SelectionTarget;

/// Where the resolved Top Z of a toolpath comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopSource {
    /// Auto: the stock top in the setup frame.
    AutoStockTop,
    /// Auto: the highest up face of the selection (setup frame).
    AutoFaces,
    /// The stored Top Z (Manual or from a reference).
    Pinned,
}

impl TopSource {
    /// The short text the Heights tab shows beside the Top row.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AutoStockTop => "auto: stock top",
            Self::AutoFaces => "auto: faces",
            Self::Pinned => "pinned",
        }
    }
}

/// F033 (operator ruling 2): a toolpath whose height planes break the
/// order. The edit doors (`SetToolpathHeights`, `ReplaceToolpathConfig`),
/// generation and the export door refuse with it; its text is the same on
/// every surface.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightOrderRefusal {
    /// The name of the toolpath.
    pub toolpath: String,
    /// The refusing breaks, in the order of
    /// [`crate::compute::config::height_order`].
    pub violations: Vec<HeightOrderViolation>,
}

impl std::fmt::Display for HeightOrderRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Toolpath '{}': the height planes are out of order: ",
            self.toolpath
        )?;
        for (i, violation) in self.violations.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{violation}")?;
        }
        f.write_str(".")
    }
}

/// The heights of one toolpath in its setup frame: the answer of
/// [`ProjectSession::toolpath_heights`].
#[derive(Debug, Clone, Copy)]
pub struct ToolpathHeights {
    /// The height context: the stock and the model box in the setup frame.
    pub context: HeightContext,
    /// The heights of the config, with the face top applied.
    pub heights: ResolvedHeights,
    /// Where `heights.top_z` comes from.
    pub top_from: TopSource,
    /// The top of the up faces of the selection, in the setup frame.
    face_top_z: Option<f64>,
    /// The plane Z of the lowest up face of the selection, in the setup
    /// frame (the floor "each picked face" reads it).
    face_bottom_z: Option<f64>,
    /// The floor of an `OpFloor` op resolved against [`Self::context`]
    /// (P3.2), or why it does not resolve. `None` for every other op.
    floor: Option<Result<FloorRef, FloorRefusal>>,
}

impl ToolpathHeights {
    /// The same rule for another `HeightsConfig` of the same toolpath: the
    /// GUI draft, which can differ from the stored config for one frame.
    #[must_use]
    pub fn resolve(&self, heights: &HeightsConfig) -> ResolvedHeights {
        apply_rule(heights, &self.context, self.face_top_z).0
    }

    /// Where the top of another `HeightsConfig` of this toolpath comes from.
    #[must_use]
    pub fn top_source(&self, heights: &HeightsConfig) -> TopSource {
        apply_rule(heights, &self.context, self.face_top_z).1
    }

    /// P3.7 (ruling L-10): the Z of each floor reference of this toolpath
    /// under `heights`, in the setup frame. The GUI floor row and the
    /// height diagram re-express a floor through it, so the resolved floor
    /// Z stays when the reference changes. The Top Z is the top under a
    /// floor that is not each face: [`Floor::Top`] reads it.
    #[must_use]
    pub fn floor_references(&self, heights: &HeightsConfig) -> FloorReferences {
        let mut context = self.context;
        context.floor = None;
        FloorReferences {
            top_z: apply_rule(heights, &context, self.face_top_z).0.top_z,
            stock_top_z: self.context.stock_top_z,
            stock_bottom_z: self.context.stock_bottom_z,
            model_top_z: self.context.model_top_z,
            face_bottom_z: self.face_bottom_z,
        }
    }

    /// The face top of the selection, in the setup frame.
    #[must_use]
    pub const fn face_top_z(&self) -> Option<f64> {
        self.face_top_z
    }

    /// P3.2: the refusal of the floor of this toolpath under `heights`, or
    /// `None`. A floor that does not resolve (L-4, no up face under each
    /// face, a value that is not finite) refuses, and so does a floor at or
    /// above the resolved top (L-16): its ladder would be empty. Under each
    /// face the deepest face floor is the one to check: a face at the top
    /// is skipped with a note, and the op refuses only when every face is
    /// at the top (P3.7).
    #[must_use]
    pub fn floor_refusal(&self, heights: &HeightsConfig) -> Option<FloorRefusal> {
        match self.floor? {
            Err(refusal) => Some(refusal),
            Ok(floor) => {
                let top_z = self.resolve(heights).top_z;
                let floor_z = floor.z(top_z);
                (floor_z >= top_z).then_some(FloorRefusal::AboveTop { floor_z, top_z })
            }
        }
    }

    /// P3.3: the resolved floor Z of an `OpFloor` op under `heights`, in
    /// the setup frame, or why it does not resolve. `None` for an op with
    /// no floor field. The Z is `ResolvedHeights::bottom_z`: the floor IS
    /// the bottom of these ops (P3.2).
    #[must_use]
    pub fn floor_z(&self, heights: &HeightsConfig) -> Option<Result<f64, FloorRefusal>> {
        self.floor.as_ref()?;
        Some(match self.floor_refusal(heights) {
            Some(refusal) => Err(refusal),
            None => Ok(self.resolve(heights).bottom_z),
        })
    }

    /// L-4: the refusal of `heights` on this toolpath, or `None`. A Top Z or
    /// a Bottom Z that reads the Model Top or the Model Bottom refuses when
    /// the toolpath has no mesh: the context then holds no model Z.
    #[must_use]
    pub fn model_reference_refusal(&self, heights: &HeightsConfig) -> Option<String> {
        if self.context.model_top_z.is_some() {
            return None;
        }
        let (slot, reference) = model_reference(heights)?;
        Some(format!(
            "The {slot} reads the {}, but the model of the toolpath has no mesh, so it has \
             no Z extent. Set the {slot} from the stock or to a Z value.",
            reference.label()
        ))
    }

    /// F033: THE height-plane order of `heights` on this toolpath
    /// ([`crate::compute::config::height_order`]), anchored on the stock top
    /// of the context. The GUI badge, the Heights tab gate, the edit doors,
    /// generation and the export door read it.
    #[must_use]
    pub fn height_order(
        &self,
        heights: &HeightsConfig,
        op: OperationType,
    ) -> Vec<HeightOrderViolation> {
        height_order(op, &self.resolve(heights), Some(self.context.stock_top_z))
    }

    /// F033: the refusal of an edit of the heights of this toolpath from
    /// `before` to `after`, or `None`. The edit refuses when `after` adds a
    /// refusing break of the order ([`door_refusals`]: [`added_refusals`]
    /// less the floor break of an `OpFloor` op). The Heights tab and
    /// the height diagram call it before they write; the session doors call
    /// the same comparison.
    #[must_use]
    pub fn edit_refusal(
        &self,
        op: OperationType,
        before: &HeightsConfig,
        after: &HeightsConfig,
    ) -> Option<Vec<HeightOrderViolation>> {
        let added = door_refusals(
            op,
            &self.height_order(before, op),
            &self.height_order(after, op),
        );
        (!added.is_empty()).then_some(added)
    }

    /// Heights from a bare context, with no session and no face: for a test
    /// of the rule or of a panel. Production code reads
    /// [`ProjectSession::toolpath_heights`]; the P3.1 sentry
    /// (`one_heights_resolver_for_every_surface_p3_1`) scans the viz
    /// production source for a call of this function.
    #[must_use]
    pub fn from_context(heights: &HeightsConfig, context: HeightContext) -> Self {
        let (resolved, top_from) = apply_rule(heights, &context, None);
        Self {
            context,
            heights: resolved,
            top_from,
            face_top_z: None,
            face_bottom_z: None,
            floor: context.floor.map(Ok),
        }
    }
}

/// F033: the refusing breaks that an edit adds, for the edit doors (the
/// session door and the Heights tab gate). On an `OpFloor` op (Pocket,
/// Profile, Zigzag, Adaptive 2D) the bottom IS the floor, and the floor rule
/// owns a floor at or above the top (L-16): the Floor line names it and
/// generation refuses it (`floor_refusal`). So the edit doors leave that
/// break to the floor rule, and the GUI Floor row and MCP
/// `set_toolpath_floor` take one floor value alike. Generation and export
/// still refuse it.
fn door_refusals(
    op: OperationType,
    before: &[HeightOrderViolation],
    after: &[HeightOrderViolation],
) -> Vec<HeightOrderViolation> {
    let floor_owned = matches!(op.capabilities().floor_owner, FloorOwner::OpFloor);
    added_refusals(before, after)
        .into_iter()
        .filter(|v| !(floor_owned && matches!(v, HeightOrderViolation::BottomNotBelowTop { .. })))
        .collect()
}

/// THE rule: resolve `cfg` on `ctx`, then an auto top follows the face top.
fn apply_rule(
    cfg: &HeightsConfig,
    ctx: &HeightContext,
    face_top_z: Option<f64>,
) -> (ResolvedHeights, TopSource) {
    let mut heights = cfg.resolve(ctx);
    if !cfg.top_z.is_auto() {
        return (heights, TopSource::Pinned);
    }
    // L-15 (P3.5): under the floor "each picked face" the auto top stays at
    // the stock top. The faces are the floors; a top at the highest face
    // would leave that face no depth to cut.
    if ctx.floor.is_some_and(FloorRef::is_each_face) {
        return (heights, TopSource::AutoStockTop);
    }
    // N12 item 1: an AUTO top follows the picked faces. A PINNED top is the
    // operator's own answer and stays. P2.3b: the top is in the setup frame.
    match face_top_z {
        Some(top) => {
            heights.top_z = top;
            if cfg.bottom_z.is_auto() {
                // P3.2: the floor of an `OpFloor` op follows the face top by
                // its own rule; every other op keeps `top - depth`.
                heights.bottom_z = match ctx.floor {
                    Some(floor) => floor.z(top),
                    None => top - ctx.op_depth.abs(),
                };
            }
            (heights, TopSource::AutoFaces)
        }
        None => (heights, TopSource::AutoStockTop),
    }
}

/// L-4: the name of the first of Top Z and Bottom Z that reads a model
/// reference, with the reference.
fn model_reference(cfg: &HeightsConfig) -> Option<(&'static str, HeightReference)> {
    [("Top Z", cfg.top_z), ("Bottom Z", cfg.bottom_z)]
        .into_iter()
        .find_map(|(slot, mode)| match mode {
            HeightMode::FromReference(r)
                if matches!(
                    r.reference,
                    HeightReference::ModelTop | HeightReference::ModelBottom
                ) =>
            {
                Some((slot, r.reference))
            }
            HeightMode::Auto | HeightMode::Manual(_) | HeightMode::FromReference(_) => None,
        })
}

impl ProjectSession {
    /// THE heights of `tc` (a stored config or a GUI draft) in its setup
    /// frame. Generation, the GUI and MCP read this rule; no surface keeps
    /// a copy (geometry selection P3.1).
    ///
    /// `Err` when the selection of `tc` does not resolve (as generation)
    /// and when a surface model id names no model. The L-4 refusal is a
    /// value of the answer ([`ToolpathHeights::model_reference_refusal`]).
    pub fn toolpath_heights(&self, tc: &ToolpathConfig) -> Result<ToolpathHeights, SessionError> {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        let setup_mesh = self.setup_mesh(tc, &ctx)?;
        let op_status = self.selection_status(tc, SelectionTarget::Op)?;
        let face_top_z = match op_status.face_top_z {
            Some(top) => Some(top),
            None if self.reads_boundary_faces(tc) => {
                self.selection_status(tc, SelectionTarget::Boundary)?
                    .face_top_z
            }
            None => None,
        };
        Ok(self.heights_in_setup(
            tc,
            &ctx,
            setup_mesh.as_deref(),
            face_top_z,
            op_status.face_bottom_z,
        ))
    }

    /// The resolved Top Z of `tc` under the floor "each picked face"
    /// (geometry selection P3.5), with no read of the selection: under that
    /// floor the auto top does not follow the faces (L-15). The
    /// `selection_status` report reads it for the per-area floor check
    /// (`compute::floor::region_floors`), so the status and the prologue
    /// check against one top. `None` for every other floor.
    pub(crate) fn each_face_top_z(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
    ) -> Result<Option<f64>, SessionError> {
        if !matches!(tc.operation.floor(), Some(Floor::EachFace { .. })) {
            return Ok(None);
        }
        let setup_mesh = self.setup_mesh(tc, ctx)?;
        Ok(Some(
            self.heights_in_setup(tc, ctx, setup_mesh.as_deref(), None, None)
                .heights
                .top_z,
        ))
    }

    /// The mesh of `tc` ([`Self::toolpath_mesh`]) in the frame of the setup
    /// of `ctx`.
    fn setup_mesh(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
    ) -> Result<Option<Arc<TriangleMesh>>, SessionError> {
        Ok(match self.toolpath_mesh(tc)? {
            Some(raw) if ctx.needs_transform() => Some(crate::maps::geom_cache::cached_transform(
                &raw,
                &self.setup_transform_info(ctx.face_up, ctx.z_rotation),
            )),
            other => other,
        })
    }

    /// Write the floor of toolpath `index` (geometry selection P3.3): the
    /// door of the `SetToolpathFloor` command row.
    ///
    /// The op must hold a floor (Pocket, Profile, Zigzag, Adaptive 2D), and
    /// the op config with the new floor must pass the one W6b door check
    /// (`compute::domain::validate_operation`, which reads
    /// `Floor::door_refusal`: a finite number in its domain). A floor at or above a
    /// resolved top that is not the Top Z needs the model and the
    /// selection, so generation refuses it (L-16). A change drops the
    /// result chain of the toolpath, as a heights edit does.
    ///
    /// # Errors
    /// [`SessionError::ToolpathNotFound`], [`SessionError::InvalidParam`]
    /// for an op with no floor, or [`SessionError::OutOfDomain`] that names
    /// the toolpath, the op and the fix. Nothing is written then.
    pub(crate) fn set_toolpath_floor(
        &mut self,
        index: usize,
        floor: Floor,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            let Some(before) = tc.operation.floor() else {
                return Err(SessionError::InvalidParam(format!(
                    "{} '{}' has no floor field. Only Pocket, Profile, Zigzag and Adaptive 2D \
                     take a floor; set the depth of this op with set_toolpath_param or its \
                     heights",
                    tc.operation.label(),
                    tc.name
                )));
            };
            // W6b: THE door check of an op config (`validate_operation`,
            // which reads `Floor::door_refusal`), on the candidate config,
            // before any write.
            let mut candidate = tc.operation.clone();
            let _ = candidate.set_floor(floor);
            crate::compute::domain::validate_toolpath_operation(&tc.name, &candidate)?;
            if before != floor {
                tc.operation = candidate;
                let enabled = tc.enabled;
                session.invalidate_result_chain(index, enabled);
            }
            Ok(())
        })
    }

    /// F033: the edit door of the height-plane order. `candidate` is the
    /// config that an edit of toolpath `index` writes. The door refuses when
    /// the candidate ADDS a refusing break of the order
    /// ([`added_refusals`], less the floor break of an `OpFloor` op, which
    /// the floor rule owns); a break that the stored config already holds
    /// does not block an edit. `SetToolpathHeights` (MCP
    /// `set_toolpath_heights`) and `ReplaceToolpathConfig` (the GUI panel,
    /// the CLI feeds apply) call it before they write.
    ///
    /// A candidate whose heights do not resolve (a selection that refuses,
    /// a model id that names no model) passes: generation refuses it with
    /// that reason.
    ///
    /// # Errors
    /// [`SessionError::HeightOrder`]. Nothing is written then.
    pub(crate) fn height_order_door(
        &self,
        index: usize,
        candidate: &ToolpathConfig,
    ) -> Result<(), SessionError> {
        let Ok(after) = self.toolpath_heights(candidate) else {
            return Ok(());
        };
        let op = candidate.operation.op_type();
        let after = after.height_order(&candidate.heights, op);
        let before = self
            .toolpath_configs
            .get(index)
            .and_then(|tc| {
                self.toolpath_heights(tc)
                    .ok()
                    .map(|answer| answer.height_order(&tc.heights, tc.operation.op_type()))
            })
            .unwrap_or_default();
        let added = door_refusals(op, &before, &after);
        if added.is_empty() {
            Ok(())
        } else {
            Err(SessionError::HeightOrder(HeightOrderRefusal {
                toolpath: candidate.name.clone(),
                violations: added,
            }))
        }
    }

    /// F033: the refusal of the height-plane order of `tc`, or `None`: every
    /// refusing break of [`ToolpathHeights::height_order`]. The export door
    /// reads it for each toolpath it emits.
    #[must_use]
    pub fn height_order_refusal(&self, tc: &ToolpathConfig) -> Option<HeightOrderRefusal> {
        let answer = self.toolpath_heights(tc).ok()?;
        let violations: Vec<HeightOrderViolation> = answer
            .height_order(&tc.heights, tc.operation.op_type())
            .into_iter()
            .filter(|v| v.refuses())
            .collect();
        (!violations.is_empty()).then(|| HeightOrderRefusal {
            toolpath: tc.name.clone(),
            violations,
        })
    }

    /// [`Self::heights_in_setup`] for generation: the L-4 refusal is an
    /// error here.
    ///
    /// P3.5: under the floor "each picked face" an area with no face and a
    /// face floor at or above the top refuse in the prologue, by item
    /// (`compute::floor::region_floors`), as `selection_status` refuses
    /// them. So this door does not refuse [`FloorRefusal::NoFace`] and
    /// [`FloorRefusal::AboveTop`] of that floor first.
    pub(crate) fn generation_heights(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup_mesh: Option<&TriangleMesh>,
        face_top_z: Option<f64>,
        face_bottom_z: Option<f64>,
    ) -> Result<ResolvedHeights, SessionError> {
        tc.floor_conflict()?;
        let answer = self.heights_in_setup(tc, ctx, setup_mesh, face_top_z, face_bottom_z);
        let each_face = matches!(tc.operation.floor(), Some(Floor::EachFace { .. }));
        if let Some(refusal) = answer.floor_refusal(&tc.heights)
            && !(each_face
                && matches!(
                    refusal,
                    FloorRefusal::NoFace | FloorRefusal::AboveTop { .. }
                ))
        {
            return Err(SessionError::InvalidParam(format!(
                "Toolpath '{}': {refusal}",
                tc.name
            )));
        }
        if let Some(refusal) = answer.model_reference_refusal(&tc.heights) {
            return Err(SessionError::MissingGeometry(format!(
                "Toolpath '{}': {refusal}",
                tc.name
            )));
        }
        // F033: a refusing break of the height-plane order refuses
        // generation. Under each face the prologue owns a floor at the top,
        // by item (P3.5, P3.7), so the bottom break is left to it.
        let violations: Vec<HeightOrderViolation> = answer
            .height_order(&tc.heights, tc.operation.op_type())
            .into_iter()
            .filter(|v| {
                v.refuses()
                    && !(each_face && matches!(v, HeightOrderViolation::BottomNotBelowTop { .. }))
            })
            .collect();
        if !violations.is_empty() {
            return Err(SessionError::HeightOrder(HeightOrderRefusal {
                toolpath: tc.name.clone(),
                violations,
            }));
        }
        Ok(answer.heights)
    }

    /// THE rule (see the module documentation). `setup_mesh` is the mesh
    /// of `tc` in the setup frame; `face_top_z` and `face_bottom_z` are the
    /// highest and the lowest plane Z of the up faces of its selection, in
    /// the setup frame.
    pub(crate) fn heights_in_setup(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup_mesh: Option<&TriangleMesh>,
        face_top_z: Option<f64>,
        face_bottom_z: Option<f64>,
    ) -> ToolpathHeights {
        // K3: the heights do not read the obstacles. The obstacle step
        // after the dressups lifts only the moves that cross one.
        let mut context = ctx.height_context(
            tc.operation.default_depth_for_heights(),
            setup_mesh.map(|m| &m.bbox),
        );
        // P3.2: the floor resolves against the stock and the model of this
        // context. The top is not known yet, so `Floor::Top` stays relative.
        // P3.5: each face reads the planes of the up faces.
        let floor = tc
            .operation
            .floor()
            .map(|floor| floor.resolve(&context, face_bottom_z));
        context.floor = floor.and_then(Result::ok);
        let (heights, top_from) = apply_rule(&tc.heights, &context, face_top_z);
        ToolpathHeights {
            context,
            heights,
            top_from,
            face_top_z,
            face_bottom_z,
            floor,
        }
    }

    /// The mesh that the heights and generation of `tc` read, in the model
    /// frame: the mesh of the toolpath model, or the surface model of a
    /// Project Curve. A surface id that names no model refuses: it does not
    /// fall back to the mesh of the toolpath model (P2.4).
    pub(crate) fn toolpath_mesh(
        &self,
        tc: &ToolpathConfig,
    ) -> Result<Option<Arc<TriangleMesh>>, SessionError> {
        let mut mesh = self.toolpath_model(tc).ok().and_then(|m| m.mesh.clone());
        // ProjectCurve optionally references a separate surface model for
        // the mesh (so polygons can come from a DXF while the projection
        // target is a terrain STL).
        if let crate::compute::OperationConfig::ProjectCurve(ref cfg) = tc.operation
            && let Some(surface_id) = cfg.surface_model_id
        {
            let surface = self.model_by_id(surface_id).ok_or_else(|| {
                SessionError::MissingGeometry(format!(
                    "Toolpath '{}' projects onto surface model id {}, but no model has that \
                     id. Pick a surface model, or set the surface to the input model.",
                    tc.name,
                    surface_id.raw()
                ))
            })?;
            if surface.mesh.is_some() {
                mesh = surface.mesh.clone();
            }
        }
        Ok(mesh)
    }

    /// `true` when `tc` takes its face picks from its boundary selection: an
    /// op with no selection slot and an enabled `Selection` boundary (P4.1).
    pub(crate) fn reads_boundary_faces(&self, tc: &ToolpathConfig) -> bool {
        tc.operation.selection().is_none()
            && tc.boundary.enabled
            && matches!(tc.boundary.source, BoundarySource::Selection { .. })
    }
}

impl ToolpathConfig {
    /// P3.2 (ruling D9, design 2.6): the one floor field. On an op whose
    /// floor is its `floor` field (`FloorOwner::OpFloor`: Pocket, Profile,
    /// Zigzag, Adaptive 2D) the Heights Bottom Z must be Auto. A pinned
    /// bottom never reached the cut of these ops (G-BOTTOMPIN), so a pin
    /// refuses at every door: `set_heights_config`,
    /// `replace_toolpath_config`, the op change, `add_toolpath` and
    /// generation. The loader refuses it before (`RETIRED_OP_ENTRIES`).
    ///
    /// # Errors
    ///
    /// [`SessionError::InvalidParam`] that names the toolpath and the fix.
    pub fn floor_conflict(&self) -> Result<(), SessionError> {
        match self.operation.capabilities().floor_owner {
            FloorOwner::OpFloor if !self.heights.bottom_z.is_auto() => {
                Err(SessionError::InvalidParam(format!(
                    "{} '{}' takes its floor from its floor field (operation.params.floor), \
                     not from the Heights Bottom Z. Set Bottom Z to Auto, and set the floor",
                    self.operation.label(),
                    self.name
                )))
            }
            FloorOwner::OpFloor | FloorOwner::HeightsBottom | FloorOwner::OpDial => Ok(()),
        }
    }
}
