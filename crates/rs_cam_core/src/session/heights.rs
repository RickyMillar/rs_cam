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
use crate::compute::config::{
    HeightContext, HeightMode, HeightReference, HeightsConfig, ResolvedHeights,
};
use crate::mesh::TriangleMesh;
use crate::session::{ProjectSession, SessionError, ToolpathConfig};

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

    /// The face top of the selection, in the setup frame.
    #[must_use]
    pub const fn face_top_z(&self) -> Option<f64> {
        self.face_top_z
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
        }
    }
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
    // N12 item 1: an AUTO top follows the picked faces. A PINNED top is the
    // operator's own answer and stays. P2.3b: the top is in the setup frame.
    match face_top_z {
        Some(top) => {
            heights.top_z = top;
            if cfg.bottom_z.is_auto() {
                heights.bottom_z = top - ctx.op_depth.abs();
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
        let mesh = self.toolpath_mesh(tc)?;
        let setup_mesh = match mesh {
            Some(raw) if ctx.needs_transform() => Some(crate::maps::geom_cache::cached_transform(
                &raw,
                &self.setup_transform_info(ctx.face_up, ctx.z_rotation),
            )),
            other => other,
        };
        let face_top_z = match self.selection_status(tc, SelectionTarget::Op)?.face_top_z {
            Some(top) => Some(top),
            None if self.reads_boundary_faces(tc) => {
                self.selection_status(tc, SelectionTarget::Boundary)?
                    .face_top_z
            }
            None => None,
        };
        Ok(self.heights_in_setup(tc, &ctx, setup_mesh.as_deref(), face_top_z))
    }

    /// [`Self::heights_in_setup`] for generation: the L-4 refusal is an
    /// error here.
    pub(crate) fn generation_heights(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup_mesh: Option<&TriangleMesh>,
        face_top_z: Option<f64>,
    ) -> Result<ResolvedHeights, SessionError> {
        let answer = self.heights_in_setup(tc, ctx, setup_mesh, face_top_z);
        if let Some(refusal) = answer.model_reference_refusal(&tc.heights) {
            return Err(SessionError::MissingGeometry(format!(
                "Toolpath '{}': {refusal}",
                tc.name
            )));
        }
        Ok(answer.heights)
    }

    /// THE rule (see the module documentation). `setup_mesh` is the mesh
    /// of `tc` in the setup frame; `face_top_z` is the top of the up faces
    /// of its selection, in the setup frame.
    pub(crate) fn heights_in_setup(
        &self,
        tc: &ToolpathConfig,
        ctx: &SetupEvalContext,
        setup_mesh: Option<&TriangleMesh>,
        face_top_z: Option<f64>,
    ) -> ToolpathHeights {
        // K3: the heights do not read the obstacles. The obstacle step
        // after the dressups lifts only the moves that cross one.
        let context = ctx.height_context(
            tc.operation.default_depth_for_heights(),
            setup_mesh.map(|m| &m.bbox),
        );
        let (heights, top_from) = apply_rule(&tc.heights, &context, face_top_z);
        ToolpathHeights {
            context,
            heights,
            top_from,
            face_top_z,
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
            let surface = self.find_model_by_raw_id(surface_id.0).ok_or_else(|| {
                SessionError::MissingGeometry(format!(
                    "Toolpath '{}' projects onto surface model id {}, but no model has that \
                     id. Pick a surface model, or set the surface to the input model.",
                    tc.name, surface_id.0
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
