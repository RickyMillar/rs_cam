//! The reference geometry: the stock box, the origin axes and the fixture
//! lines (obstacles, alignment pins, flip axis, datum).
//!
//! The stock, the axes, the pins and the datum are cheap and rebuild on
//! every pass. The obstacle lines resolve each obstacle, so they are a keyed
//! slot (K2): they rebuild only when their key moves, and every pass appends
//! the cached lines.

use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::session::SetupData;

use crate::render::RenderResources;
use crate::render::stock_render::StockGpuData;
use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::state::AppState;
use crate::state::job::SetupFrame;
use crate::state::selection::Selection;

use super::{Gpu, PassFrame};

/// Upload the stock wireframe and the solid stock. Returns the stock box of
/// the display frame, which the height planes read too.
pub(super) fn upload_stock(
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) -> BoundingBox3 {
    let stock = frame.stock();
    let stock_bbox = match frame.frame() {
        Some(setup) => {
            let (w, d, h) = setup.effective_stock(stock);
            BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(w, d, h),
            }
        }
        None => stock.bbox(),
    };
    resources.stock_data = Some(StockGpuData::from_bbox(gpu.device, &stock_bbox));
    resources.solid_stock_data = Some(crate::render::stock_render::SolidStockGpuData::from_bbox(
        gpu.device,
        &stock_bbox,
    ));
    stock_bbox
}

/// Upload the origin axes at the stock origin (the local origin in machine
/// view).
pub(super) fn upload_origin_axes(
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let stock = frame.stock();
    let origin = if frame.frame().is_some() {
        [0.0_f32, 0.0, 0.0]
    } else {
        [
            stock.origin_x as f32,
            stock.origin_y as f32,
            stock.origin_z as f32,
        ]
    };
    let min_dim = stock.x.min(stock.y).min(stock.z) as f32;
    let length = (min_dim * 0.3).clamp(5.0, 50.0);
    resources.origin_axes_data = Some(crate::render::grid_render::OriginAxesGpuData::new(
        gpu.device, origin, length,
    ));
}

/// Everything `RenderResources::obstacle_lines` is built from (K2).
pub(super) struct ObstacleInputs<'a> {
    frame: &'a PassFrame,
    setup_frame: SetupFrame,
    setup: &'a SetupData,
    /// The geometry of each model an obstacle reads, in obstacle order.
    models: Vec<upload_cache::ObstacleModelKey>,
    focus: Option<(
        crate::state::job::SetupId,
        Option<rs_cam_core::ids::ObstacleId>,
    )>,
    show: bool,
    /// `obstacle_marks::push_obstacle_lines` reads the parts above through
    /// the whole state (the stock, the models, the selection and the
    /// flag). It is the one door the drawing and the pick share, so the
    /// build hands it the state; each part it reads is a field above.
    state: &'a AppState,
}

impl SlotInputs for ObstacleInputs<'_> {
    type Key = upload_cache::ObstacleLinesKey;

    fn key(&self) -> Self::Key {
        let Self {
            frame,
            // The frame key holds the face and the rotation.
            setup_frame: _,
            setup,
            models,
            focus,
            show,
            // Read only through the fields above (see the field doc).
            state: _,
        } = self;
        upload_cache::ObstacleLinesKey {
            frame: frame.key(),
            obstacles: setup
                .obstacles
                .iter()
                .map(|o| (o.id, o.effect_digest()))
                .collect(),
            models: models.clone(),
            focus: *focus,
            show: *show,
        }
    }
}

/// The obstacle inputs of the active setup, or `None` when no setup is
/// active. The model geometry is held by identity: a re-import is a new
/// `Arc`, and a missing model is `None`.
fn obstacle_inputs<'a>(state: &'a AppState, frame: &'a PassFrame) -> Option<ObstacleInputs<'a>> {
    let (setup_id, setup_frame) = frame.setup?;
    let setup = state
        .session
        .list_setups()
        .iter()
        .find(|sd| sd.id == setup_id.0)?;
    let mut models = Vec::new();
    for o in &setup.obstacles {
        for id in o.geometry.model_ids() {
            let model = state.session.models().iter().find(|m| m.id == id.0);
            models.push(upload_cache::ObstacleModelKey {
                model_id: id.0,
                mesh: model
                    .and_then(|m| m.mesh.as_ref())
                    .map(upload_cache::ArcId::new),
                items: model.map(|m| upload_cache::ArcId::new(&m.items)),
            });
        }
    }
    Some(ObstacleInputs {
        frame,
        setup_frame,
        setup,
        models,
        focus: match state.selection {
            Selection::Setup(id) => Some((id, None)),
            Selection::Obstacle(id, obstacle) => Some((id, Some(obstacle))),
            _ => None,
        },
        show: state.viewport.show_obstacles,
        state,
    })
}

/// Upload the reference lines: obstacles, pins, flip axis, datum.
///
/// P6 — four overlays share this one line buffer, and each is filtered into
/// it by its OWN registry flag: obstacles, alignment pins, the flip axis and
/// the datum crosshair (audit §3.3, §5 Geometry). The old workspace clauses
/// are gone: the flags carry per-workspace DEFAULTS instead, so the
/// Simulation view still hides obstacles while the operator can switch them
/// back on.
///
/// Every one of these is therefore an UPLOAD-time flag, and upload-time
/// flags need a trigger. `overlay_upload_key` in `app.rs` is that trigger,
/// and it carries the workspace too — `UiCommand::SwitchWorkspace` never set
/// `pending_upload` itself (audit §3.3, "a future edit that removes the
/// incidental trigger would expose the gap silently").
pub(super) fn upload_fixture_lines(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    use crate::render::fixture_render::FixtureGpuData;

    let stock = frame.stock();
    // Only show obstacles and pins from the active setup (each setup has
    // its own local frame, so mixing them is wrong).
    let active_session_setup = frame
        .setup_id()
        .and_then(|sid| state.session.list_setups().iter().find(|sd| sd.id == sid.0));

    if let Refresh::Build(inputs) = upload_cache::refresh(
        &mut resources.obstacle_lines_key,
        obstacle_inputs(state, frame),
    ) {
        resources.obstacle_lines.clear();
        if let Some(inputs) = inputs {
            build_obstacle_lines(&inputs, &mut resources.obstacle_lines);
        }
    }
    let mut pin_vertices: Vec<crate::render::LineVertex> = resources.obstacle_lines.clone();

    // The alignment pins, the wrong-orientation ghost and the flip axis
    // (P7), in the frame the pick reads (C8).
    if let Some(setup) = frame.frame() {
        crate::app::pin_marks::push_pin_lines(state, setup, &mut pin_vertices);
    }

    // Add datum crosshair markers. Datum is always in local frame
    // coords. The Setup-only clause this gate used to carry is a
    // per-workspace DEFAULT now (UX §6.4 lists the datum on in Setup
    // AND in Toolpaths), so the flag decides.
    if state.viewport.show_datum
        && let Some(setup) = frame.frame()
        && let Some(sd) = active_session_setup
    {
        use rs_cam_core::session::{Corner, XYDatum};

        let (eff_w, eff_d, eff_h) = setup.effective_stock(stock);
        let color = crate::render::colors::DATUM_MARK;

        // The datum is persisted project state (W9 / P-2), read
        // straight off the setup instead of a GUI-side overlay.
        let datum = &sd.datum;

        // Datum in setup-local frame: XY at corner/center, Z at top surface
        let local_datum: Option<P3> = match &datum.xy_method {
            XYDatum::CornerProbe(corner) => {
                let x = match corner {
                    Corner::FrontLeft | Corner::BackLeft => 0.0,
                    Corner::FrontRight | Corner::BackRight => eff_w,
                };
                let y = match corner {
                    Corner::FrontLeft | Corner::FrontRight => 0.0,
                    Corner::BackLeft | Corner::BackRight => eff_d,
                };
                Some(P3::new(x, y, eff_h))
            }
            XYDatum::CenterOfStock => Some(P3::new(eff_w / 2.0, eff_d / 2.0, eff_h)),
            _ => None,
        };

        if let Some(local) = local_datum {
            // Always in local frame — use local coords directly.
            let dx = local.x as f32;
            let dy = local.y as f32;
            let dz = local.z as f32;

            let arm = 15.0_f32;
            let diamond = 3.0_f32;

            // Crosshair lines in all 3 directions (works for any face)
            for &(ax, ay, az) in &[(arm, 0.0, 0.0), (0.0, arm, 0.0), (0.0, 0.0, arm)] {
                pin_vertices.push(crate::render::LineVertex {
                    position: [dx - ax, dy - ay, dz - az],
                    color,
                });
                pin_vertices.push(crate::render::LineVertex {
                    position: [dx + ax, dy + ay, dz + az],
                    color,
                });
            }

            // Diamond in XY plane at datum Z
            for &[(x1, y1), (x2, y2)] in &[
                [(diamond, 0.0), (0.0, diamond)],
                [(0.0, diamond), (-diamond, 0.0)],
                [(-diamond, 0.0), (0.0, -diamond)],
                [(0.0, -diamond), (diamond, 0.0)],
            ] {
                pin_vertices.push(crate::render::LineVertex {
                    position: [dx + x1, dy + y1, dz],
                    color,
                });
                pin_vertices.push(crate::render::LineVertex {
                    position: [dx + x2, dy + y2, dz],
                    color,
                });
            }
        }
    }

    resources.fixture_data =
        FixtureGpuData::from_lines(gpu.device, &resources.gpu_limits, &pin_vertices);
}

fn build_obstacle_lines(inputs: &ObstacleInputs<'_>, out: &mut Vec<crate::render::LineVertex>) {
    let ObstacleInputs {
        frame: _,
        setup_frame,
        setup,
        models: _,
        focus: _,
        show: _,
        state,
    } = inputs;
    crate::app::obstacle_marks::push_obstacle_lines(state, setup_frame, setup, out);
}
