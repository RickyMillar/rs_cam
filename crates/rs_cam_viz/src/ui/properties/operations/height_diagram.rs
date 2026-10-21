//! The 2D height diagram: the height rows drawn against the stock profile.
//!
//! The diagram shows Safe Z, Clearance, Top, Retract and Bottom as lines over
//! a stock rectangle, with the model profile behind them. The parent
//! re-exports `draw_height_diagram`.

use super::bottom_z_pin_note;
use crate::state::panels::HeightsRefusal;
use crate::state::toolpath::{HeightContext, HeightMode, HeightsConfig, OperationType};
use rs_cam_core::compute::catalog::FloorOwner;
use rs_cam_core::compute::floor::{Floor, FloorReferences};

/// What the floor row and the height diagram read beside the floor of a
/// Pocket, Profile, Zigzag or Adaptive 2D (geometry selection P3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct FloorInputs {
    /// The Z of each floor reference (`ToolpathHeights::floor_references`):
    /// THE rule of a change of reference and of a drag (ruling L-10).
    pub refs: FloorReferences,
    /// Under "each picked face": the distinct floor Z of the cut faces, top
    /// first, each with the count of faces at that floor
    /// (`SelectionReport::item_floor_z`, `foreign_floor_z`). Empty for
    /// every other floor.
    pub face_floors: Vec<FaceFloor>,
}

/// One distinct face floor under "each picked face": its Z and the count of
/// cut faces at that Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceFloor {
    /// The floor Z in the setup frame.
    pub z: f64,
    /// The count of cut faces whose floor is at `z`. Never 0.
    pub faces: usize,
}

impl FloorInputs {
    /// The inputs from the heights answer under `heights` and the op
    /// selection report.
    #[must_use]
    pub fn new(
        answer: &rs_cam_core::session::ToolpathHeights,
        heights: &HeightsConfig,
        report: Option<&rs_cam_core::session::SelectionReport>,
    ) -> Self {
        let mut floor_z: Vec<f64> = report
            .into_iter()
            .flat_map(|r| r.item_floor_z.iter().chain(&r.foreign_floor_z))
            .filter_map(|z| *z)
            .collect();
        floor_z.sort_by(|a, b| b.total_cmp(a));
        let mut face_floors: Vec<FaceFloor> = Vec::new();
        for z in floor_z {
            match face_floors.last_mut() {
                Some(last) if last.z.to_bits() == z.to_bits() => last.faces += 1,
                _ => face_floors.push(FaceFloor { z, faces: 1 }),
            }
        }
        Self {
            refs: answer.floor_references(heights),
            face_floors,
        }
    }
}

/// The floor of an `OpFloor` op, for the diagram: the draft floor (a drag
/// writes it) and its inputs.
pub struct DiagramFloor<'a> {
    pub floor: &'a mut Floor,
    pub inputs: &'a FloorInputs,
}

/// Height line definition for diagram rendering and interaction.
struct DiagramLine {
    z: f64,
    color: egui::Color32,
    /// The text drawn beside the line. It carries the value, except on a
    /// Bottom the operation does not read: there it names the state, and the
    /// value stays in the Bottom field above (ruling R33).
    text: String,
    /// Which field index (0..5) for drag targeting.
    index: usize,
    /// P3.7: a run of one face floor under "each picked face": drawn
    /// dashed.
    dashed: bool,
    /// The hover of a floor run of an `OpFloor` op ([`FloorRun::hover`]).
    hover: Option<String>,
}

/// One floor run of the height diagram of an `OpFloor` op: the one floor
/// ("FL"), or one distinct face floor under "each picked face" ("F1",
/// "F2", ...).
#[derive(Debug, Clone, PartialEq)]
pub struct FloorRun {
    /// The floor Z in the setup frame.
    pub z: f64,
    /// The label beside the run: "FL -6.0" or "F2 15.0".
    pub text: String,
    /// The hover of the run: the Z, the reference and, under "each picked
    /// face", the face count ("F2: Z 15.0, 1 face, floor from Each picked
    /// face").
    pub hover: String,
    /// A face floor draws dashed.
    pub dashed: bool,
}

/// The floor runs of an `OpFloor` op (geometry selection P3.7; the hover
/// P3.8). Under "each picked face" with resolved face floors: one dashed
/// run per distinct face floor, top first. Else one run at the floor Z
/// that the floor references give, or at `bottom_z` when they give none.
#[must_use]
pub fn floor_runs(floor: Option<(&Floor, &FloorInputs)>, bottom_z: f64) -> Vec<FloorRun> {
    let each_face = floor
        .filter(|(f, _)| matches!(f, Floor::EachFace { .. }))
        .map(|(_, inputs)| inputs.face_floors.as_slice())
        .unwrap_or_default();
    if each_face.is_empty() {
        let z = floor
            .and_then(|(f, inputs)| inputs.refs.floor_z(f).ok())
            .unwrap_or(bottom_z);
        let reference = floor.map_or("Floor", |(f, _)| f.reference_label());
        return vec![FloorRun {
            z,
            text: format!("FL {z:.1}"),
            hover: format!("FL: Z {z:.1}, floor from {reference}"),
            dashed: false,
        }];
    }
    each_face
        .iter()
        .enumerate()
        .map(|(i, run)| {
            let n = i + 1;
            let z = run.z;
            let noun = if run.faces == 1 { "face" } else { "faces" };
            FloorRun {
                z,
                text: format!("F{n} {z:.1}"),
                hover: format!(
                    "F{n}: Z {z:.1}, {} {noun}, floor from {}",
                    run.faces,
                    Floor::EachFace { depth: 0.0 }.reference_label()
                ),
                dashed: true,
            }
        })
        .collect()
}

/// The `DiagramLine::index` of the Bottom row. The drag handler and the
/// R33 hover both name it.
const BOTTOM_LINE_INDEX: usize = 4;

/// How far the pointer may sit from a height line and still take it, in
/// points.
const HIT_THRESHOLD_PX: f32 = 12.0;

/// Two lines closer than this, in points, count as one line under the
/// pointer. The tie-break in [`pick_line`] then decides.
const TIE_EPS_PX: f32 = 0.5;

/// The grid a dragged height lands on: ten steps per millimetre, so the
/// step is 0.1 mm. A multiply then a divide by ten lands on the nearest
/// double to the printed value; a multiply by 0.1 does not.
const PIN_STEPS_PER_MM: f64 = 10.0;

/// Pick the line under the pointer from `candidates`, each `(index,
/// distance)` with the distance in points.
///
/// The nearest line wins. When two lines are within [`TIE_EPS_PX`] of the
/// nearest, the line with the HIGHER index wins: Bottom (4) over Top (3)
/// over Feed (2) and so on.
///
/// # R5 (2026-09-18): coincident lines picked Top
///
/// On a waterline the Auto Top and the Auto Bottom coincide. The old hit
/// test took the FIRST line within the threshold with a strict `<`, and
/// the lines are ordered Clearance, Retract, Feed, Top, Bottom, so a drag
/// on the shared line moved Top. The Auto Bottom then followed it. The
/// operator meant to drag the Bottom down and pinned the Top at −1.37
/// instead; the waterline cut the bed
/// (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.2). A line
/// lower in the ladder is the one an operator reaches for when two
/// coincide, so the higher index wins the tie.
///
/// The result does not depend on the order of `candidates`.
#[must_use]
pub fn pick_line(candidates: &[(usize, f32)]) -> Option<usize> {
    let nearest = candidates.iter().map(|&(_, d)| d).reduce(f32::min)?;
    candidates
        .iter()
        .filter(|&&(_, d)| d <= nearest + TIE_EPS_PX)
        .map(|&(idx, _)| idx)
        .max()
}

/// Round a dragged height onto the 0.1 mm grid ([`PIN_STEPS_PER_MM`]).
///
/// R5. The drag handler writes `current_z + dz` with `dz` derived from
/// screen pixels, so a value such as −1.3704614721537909 reached the
/// project file. The drag stop rounds it once. A negative zero becomes a
/// positive zero, so the file never carries `-0.0`.
#[must_use]
pub fn round_pin(v: f64) -> f64 {
    let rounded = (v * PIN_STEPS_PER_MM).round() / PIN_STEPS_PER_MM;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// F033 (operator ruling 2): write `draft` into `heights` unless THE core
/// order rule refuses the edit (`ToolpathHeights::edit_refusal`: the draft
/// adds a refusing break). The Heights rows, the Auto buttons and the
/// diagram drag all write through this one gate, so a value that would
/// invert the stack never reaches the config. A refused drag stops at the
/// last accepted value. The refusal text is kept in `refusal` (GUI state,
/// `state.panels.heights_refusal`, UI-09) for the Heights tab. Returns
/// `true` when the draft was written.
pub fn admit_heights_edit(
    ui: &egui::Ui,
    heights: &mut HeightsConfig,
    draft: HeightsConfig,
    answer: &rs_cam_core::session::ToolpathHeights,
    op_type: OperationType,
    refusal: &mut HeightsRefusal,
) -> bool {
    if draft == *heights {
        return false;
    }
    match answer.edit_refusal(op_type, heights, &draft) {
        None => {
            *heights = draft;
            refusal.clear();
            true
        }
        Some(violations) => {
            let text = violations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            let now = ui.input(|i| i.time);
            refusal.refuse(format!("Refused: {text}."), now);
            false
        }
    }
}

/// The stored mode behind a `DiagramLine::index`.
fn height_field(heights: &mut HeightsConfig, idx: usize) -> &mut HeightMode {
    match idx {
        0 => &mut heights.clearance_z,
        1 => &mut heights.retract_z,
        2 => &mut heights.feed_z,
        3 => &mut heights.top_z,
        // `idx` is always 0..5 from the `DiagramLine` definitions, so the
        // last arm is `BOTTOM_LINE_INDEX`.
        _ => &mut heights.bottom_z,
    }
}

/// What the height diagram can honestly say about the model's Z profile.
///
/// # The defect this names (F-1, 2026-09-14)
///
/// The diagram drew the model as a rectangle between `ctx.model_top_z` and
/// `ctx.model_bottom_z`, and drew nothing at all when either was `None`.
/// Both branches were wrong on the wanaka project, and in different ways.
///
/// **A 2D model carries no Z.** `rs_cam_core::session::LoadedModel::bbox`
/// reports an SVG or DXF bbox at
/// `z = 0.0 ..= 0.0`, so an operation bound to a drawing got a model
/// rectangle of ZERO height. epaint collapses such a rectangle to a
/// one-point line in the STROKE colour (`Tessellator::tessellate_rect`
/// turns a rect thinner than the feathering into a line segment), and that
/// stroke was `DIAGRAM_MATERIAL` — a dark grey on the dark canvas. The
/// operator therefore read "no model" on operations 2 to 5 of setup 1,
/// which are bound to the drawings, while operation 1, which is bound to
/// `terrain.stl` and has a real Z extent, drew correctly.
///
/// **A missing profile is a different thing.** It is NOT MEASURED, and this
/// repo never renders an absent value as a clean result. The two cases must
/// not share one blank frame, so they are two variants here and the drawing
/// states which one it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModelProfile {
    /// The model carries a Z extent. The diagram draws a box.
    Solid,
    /// The model is measured and its Z extent is zero. A 2D drawing sits on
    /// one plane, so the honest profile is a rule, not a box.
    Flat,
    /// No model profile reached the diagram. NOT MEASURED.
    Absent,
}

/// A model Z extent at or below this reads as flat. A 2D drawing reports
/// exactly zero, so the value only has to exclude arithmetic noise from the
/// setup transform.
const MODEL_FLAT_EPS_MM: f64 = 1.0e-6;

/// The canvas height in points. The width follows the panel.
const DIAGRAM_HEIGHT: f32 = 180.0;
/// The widest canvas. A wider panel gives the diagram no more to say.
const DIAGRAM_MAX_WIDTH: f32 = 260.0;
/// The narrowest canvas that still holds its own labels. The inspector is
/// 280 points wide (`app.rs`, `default_size`), so this floor never binds
/// there. It keeps the layout total rather than letting a label escape.
const DIAGRAM_MIN_WIDTH: f32 = 160.0;
/// The label column never takes more than this share of the canvas, so a
/// long value cannot push the height lines out of sight.
const LABEL_COLUMN_MAX_FRACTION: f32 = 0.45;
/// The legend swatch length in points.
const LEGEND_SWATCH_W: f32 = 10.0;
/// Stroke widths for a height line, a hovered height line, a legend swatch
/// and a flat model profile.
const LINE_STROKE: f32 = 1.5;
const LINE_STROKE_HOVER: f32 = 2.5;
const SWATCH_STROKE: f32 = 2.0;
const FLAT_PROFILE_STROKE: f32 = 2.0;
/// Half-widths of the stock and model boxes, as a share of the body.
const STOCK_HALF_WIDTH_FRACTION: f32 = 0.275;
const MODEL_HALF_WIDTH_FRACTION: f32 = 0.2;

/// Classify the model profile a [`HeightContext`] carries.
///
/// This is a pure read of the context, kept apart from the drawing, so the
/// sentry asserts the DECISION rather than a pixel.
#[must_use]
pub(crate) fn model_profile(ctx: &HeightContext) -> ModelProfile {
    match (ctx.model_top_z, ctx.model_bottom_z) {
        (Some(top), Some(bottom)) => {
            if (top - bottom).abs() <= MODEL_FLAT_EPS_MM {
                ModelProfile::Flat
            } else {
                ModelProfile::Solid
            }
        }
        _ => ModelProfile::Absent,
    }
}

/// Draw an interactive 2D side-view diagram showing stock, model, and height planes.
///
/// # F-2 (2026-09-14): the canvas holds its content
///
/// Every label used to sit at a hardcoded offset — the value column at
/// `rect.right() - 42.0` and the legend at `rect.bottom() - 8.0` — and
/// `ui.painter_at(rect)` then cut off whatever did not fit. The operator saw
/// `Stock` and `Model` sliced at the container edge. Rule F of
/// `planning/ui_declutter_2026-09-14/PLAN.md` says a container sizes to hold
/// its content, so this function now LAYS OUT every text run first, takes
/// the width and the height those runs need, and clamps each run inside the
/// allocated rect. `painter_at` stays as the backstop, and nothing should
/// reach it. The sentry is
/// `crates/rs_cam_viz/tests/the_heights_diagram_fits_f2.rs`.
///
/// # F-6 / ruling R33 (2026-09-14): the diagram agrees with the panel
///
/// The diagram drew a `DANGER` red floor at `resolved.bottom_z` on every
/// operation. `Capabilities::floor_owner` (`HeightsBottom`) says three of the
/// twenty-four read that dial, so on the other twenty-one the panel printed
/// "Bottom: Not used by this operation. …" and the diagram under it drew a
/// red floor. Two surfaces gave opposite answers about where the cut stops,
/// and red is the strongest mark on this canvas.
///
/// The line STAYS. The operator set that value, and a diagram that drops it
/// invites the reading "the pin did not take". What changes is the mark: an
/// inert Bottom reads `UNKNOWN`, its text names the state, and the reason
/// arrives on hover in `bottom_z_pin_note`'s own words, so the two surfaces
/// cannot fork their vocabulary.
///
/// ## Why the inert run carries no value
///
/// Do not "fix" `BZ (not used)` back into `BZ 26.0 (not used)`.
///
/// This diagram is a picture of CUT LIMITS. Every other run on it names a Z
/// that bounds the cut. On an operation that does not read
/// `heights.bottom_z`, the pin is not a cut limit: the operation floors at
/// `top_z` minus its own depth dial. A number printed beside four real
/// limits reads as operative, whatever follows it in brackets, so it would
/// re-assert the one thing this ruling exists to deny. The line's POSITION
/// shows that the pin exists and where it sits. The value belongs to the
/// Bottom field above, which is the surface about the SETTING rather than
/// about the cut. Those are two questions, and they now have two homes.
///
/// A second mechanism agrees. `LABEL_COLUMN_MAX_FRACTION` feeds
/// `painter.layout` as a WRAP width, not a truncation cap, so
/// `BZ 26.0 (not used)` would not clip. It would wrap to two rows in a
/// five-row diagram, and then meet the label-collision push below.
///
/// # P3.7: the floor of Pocket, Profile, Zigzag and Adaptive 2D
///
/// An `OpFloor` op takes its floor from its floor field, so its bottom run
/// is the floor, in `DANGER`: "FL" and its Z, or under "each picked face"
/// one dashed run per distinct face floor ("F1", "F2", ...). A drag of a
/// floor run writes `floor` in its current reference
/// (`FloorReferences::at_z`, ruling L-10), on the 0.1 mm grid; a value the
/// domain refuses is not written. "BZ (not used)" stays for `OpDial` ops.
pub fn draw_height_diagram(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    answer: &rs_cam_core::session::ToolpathHeights,
    op_type: OperationType,
    mut floor: Option<DiagramFloor<'_>>,
    refusal: &mut HeightsRefusal,
) {
    use crate::ui::tokens;

    // P3.1: THE session rule; an auto top is the face top generation cuts
    // from.
    let ctx = &answer.context;
    let resolved = answer.resolve(heights);
    let profile = model_profile(ctx);

    // R33. One fact — `floor_owner` — drives the colour and the text, and
    // the same fact drives `bottom_z_pin_note` in the panel above. P3.7: an
    // `OpFloor` op takes its floor from its floor field, so its bottom runs
    // are the floor.
    // The inert run names the state and carries NO value: this diagram
    // pictures cut limits, and an inert pin is not one. See the ruling in
    // this function's doc comment before changing the text.
    let owner = op_type.capabilities().floor_owner;
    let bottom_drives_the_cut = match owner {
        FloorOwner::HeightsBottom | FloorOwner::OpFloor => true,
        FloorOwner::OpDial => false,
    };
    // The floor runs of an `OpFloor` op: the face floors under "each picked
    // face", else the one resolved floor. Each floor run has a hover.
    let floor_runs: Vec<(f64, String, bool, Option<String>)> = match owner {
        FloorOwner::OpFloor => floor_runs(
            floor.as_ref().map(|f| (&*f.floor, f.inputs)),
            resolved.bottom_z,
        )
        .into_iter()
        .map(|run| (run.z, run.text, run.dashed, Some(run.hover)))
        .collect(),
        FloorOwner::HeightsBottom => vec![(
            resolved.bottom_z,
            format!("BZ {:.1}", resolved.bottom_z),
            false,
            None,
        )],
        FloorOwner::OpDial => vec![(resolved.bottom_z, "BZ (not used)".to_owned(), false, None)],
    };
    let bottom_color = if bottom_drives_the_cut {
        tokens::DANGER
    } else {
        tokens::UNKNOWN
    };

    // Build line definitions (ordered top to bottom for rendering)
    let mut lines = vec![
        DiagramLine {
            z: resolved.clearance_z,
            color: tokens::DIAGRAM_INK,
            text: format!("CZ {:.1}", resolved.clearance_z),
            index: 0,
            dashed: false,
            hover: None,
        },
        DiagramLine {
            z: resolved.retract_z,
            color: tokens::DIAGRAM_INK,
            text: format!("RZ {:.1}", resolved.retract_z),
            index: 1,
            dashed: false,
            hover: None,
        },
        DiagramLine {
            z: resolved.feed_z,
            color: tokens::OK,
            text: format!("FZ {:.1}", resolved.feed_z),
            index: 2,
            dashed: false,
            hover: None,
        },
        DiagramLine {
            z: resolved.top_z,
            color: tokens::CAUTION,
            text: format!("TZ {:.1}", resolved.top_z),
            index: 3,
            dashed: false,
            hover: None,
        },
    ];
    lines.extend(
        floor_runs
            .iter()
            .map(|(z, text, dashed, hover)| DiagramLine {
                z: *z,
                color: bottom_color,
                text: text.clone(),
                index: BOTTOM_LINE_INDEX,
                dashed: *dashed,
                hover: hover.clone(),
            }),
    );

    // Compute Z range with margin.
    //
    // The model extents belong in the range. The diagram DRAWS the model,
    // so a model above or below every height would map outside the plot and
    // paint over the canvas edge. With every drawn Z inside the range, and a
    // margin on each side, `z_to_y` is total: it cannot leave the plot.
    let mut all_z_values = vec![
        resolved.clearance_z,
        resolved.retract_z,
        resolved.feed_z,
        resolved.top_z,
        resolved.bottom_z,
        ctx.stock_top_z,
        ctx.stock_bottom_z,
    ];
    all_z_values.extend(floor_runs.iter().map(|(z, _, _, _)| *z));
    all_z_values.extend(ctx.model_top_z);
    all_z_values.extend(ctx.model_bottom_z);
    let z_min_raw = all_z_values.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let z_max_raw = all_z_values
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(10.0);
    let z_span = (z_max_raw - z_min_raw).max(1.0);
    let margin = z_span * 0.12;
    let z_min = z_min_raw - margin;
    let z_max = z_max_raw + margin;

    // Canvas
    let avail = ui.available_width();
    let width = avail.clamp(DIAGRAM_MIN_WIDTH, DIAGRAM_MAX_WIDTH);
    let desired_size = egui::vec2(width, DIAGRAM_HEIGHT);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, tokens::RADIUS_SM, tokens::DIAGRAM_CANVAS);

    let inner = rect.shrink(tokens::SPACE_2);

    // ── The legend, measured before the plot takes its height ───────────
    //
    // The legend names the two boxes, and on a flat profile it says so. It
    // is laid out first because the band it needs comes off the plot. The
    // canvas therefore holds the legend instead of drawing it over the
    // stock or off the bottom edge.
    let legend_font = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Proportional);
    let legend_wrap = (inner.width() - LEGEND_SWATCH_W - tokens::SPACE_1).max(1.0);
    let legend_text = |text: &str| {
        painter.layout(
            text.to_owned(),
            legend_font.clone(),
            tokens::TEXT_MUTED,
            legend_wrap,
        )
    };
    let mut legend: Vec<(egui::Color32, std::sync::Arc<egui::Galley>)> = Vec::new();
    legend.push((tokens::DIAGRAM_MATERIAL, legend_text("Stock")));
    match profile {
        ModelProfile::Solid => legend.push((tokens::DIAGRAM_DIM, legend_text("Model"))),
        ModelProfile::Flat => legend.push((tokens::DIAGRAM_DIM, legend_text("Model (flat)"))),
        // An absent profile gets no swatch, because there is no shape to
        // key. It is stated under the canvas instead, as NOT MEASURED.
        ModelProfile::Absent => {}
    }

    let mut legend_row_h = 0.0_f32;
    let mut legend_items_w = 0.0_f32;
    for (_, galley) in &legend {
        legend_row_h = legend_row_h.max(galley.size().y);
        legend_items_w += LEGEND_SWATCH_W + tokens::SPACE_1 + galley.size().x;
    }
    let legend_gaps = legend.len().saturating_sub(1) as f32;
    let legend_one_row_w = legend_items_w + tokens::SPACE_3 * legend_gaps;
    let legend_stacked = legend_one_row_w > inner.width();
    let legend_row_count = if legend_stacked { legend.len() } else { 1 };
    let legend_rows = legend_row_count as f32;
    let legend_h = legend_row_h * legend_rows + tokens::SPACE_1 * (legend_rows - 1.0);

    // ── The plot area ───────────────────────────────────────────────────
    let plot_bottom = inner.bottom() - legend_h - tokens::SPACE_2;
    let plot_bottom = plot_bottom.max(inner.top() + 1.0);
    let plot = egui::Rect::from_min_max(inner.min, egui::pos2(inner.right(), plot_bottom));

    // Coordinate mapping: Z → screen Y (higher Z = higher on screen = lower Y)
    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        plot.bottom() - (frac as f32) * plot.height()
    };

    // ── The value labels, measured before the lines are drawn ───────────
    //
    // The column takes the width the widest label needs, capped so the
    // lines keep the majority of the canvas. Every label is laid out with a
    // wrap width, so no run can be wider than the column it sits in.
    let value_font = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Monospace);
    let label_wrap = (plot.width() * LABEL_COLUMN_MAX_FRACTION).max(1.0);
    let mut labels: Vec<std::sync::Arc<egui::Galley>> = Vec::new();
    let mut label_w = 0.0_f32;
    for line in &lines {
        let galley = painter.layout(
            line.text.clone(),
            value_font.clone(),
            line.color,
            label_wrap,
        );
        label_w = label_w.max(galley.size().x);
        labels.push(galley);
    }
    label_w = label_w.min(label_wrap);
    let label_x = (plot.right() - label_w).max(plot.left());
    let line_right = (label_x - tokens::SPACE_1).max(plot.left());

    // The body is the part of the plot the boxes may use: everything left
    // of the label column.
    let body = egui::Rect::from_min_max(plot.min, egui::pos2(line_right, plot.bottom()));

    // Stock rectangle (centered in the body)
    let stock_hw = body.width() * STOCK_HALF_WIDTH_FRACTION;
    let stock_left = body.center().x - stock_hw;
    let stock_right = body.center().x + stock_hw;
    let stock_rect = egui::Rect::from_min_max(
        egui::pos2(stock_left, z_to_y(ctx.stock_top_z)),
        egui::pos2(stock_right, z_to_y(ctx.stock_bottom_z)),
    );
    painter.rect_filled(stock_rect, tokens::RADIUS_SM, tokens::SURFACE_OVERLAY);
    painter.rect_stroke(
        stock_rect,
        tokens::RADIUS_SM,
        egui::Stroke::new(1.0_f32, tokens::DIAGRAM_MATERIAL),
        egui::StrokeKind::Middle,
    );

    // The model profile. A solid model is a box. A flat model is a rule at
    // its one plane, drawn in the model's own fill colour so it reads as
    // clearly as a box. An absent model draws nothing here and states
    // itself under the canvas.
    let model_hw = body.width() * MODEL_HALF_WIDTH_FRACTION;
    let model_left = body.center().x - model_hw;
    let model_right = body.center().x + model_hw;
    match (profile, ctx.model_top_z, ctx.model_bottom_z) {
        (ModelProfile::Solid, Some(model_top), Some(model_bottom)) => {
            let model_rect = egui::Rect::from_min_max(
                egui::pos2(model_left, z_to_y(model_top)),
                egui::pos2(model_right, z_to_y(model_bottom)),
            );
            painter.rect_filled(model_rect, tokens::RADIUS_SM, tokens::DIAGRAM_DIM);
            painter.rect_stroke(
                model_rect,
                tokens::RADIUS_SM,
                egui::Stroke::new(1.0_f32, tokens::DIAGRAM_MATERIAL),
                egui::StrokeKind::Middle,
            );
        }
        (ModelProfile::Flat, Some(model_top), _) => {
            let y = z_to_y(model_top);
            painter.line_segment(
                [egui::pos2(model_left, y), egui::pos2(model_right, y)],
                egui::Stroke::new(FLAT_PROFILE_STROKE, tokens::DIAGRAM_DIM),
            );
        }
        _ => {}
    }

    // Height lines + labels

    // The line at a screen Y, if one is within the hit threshold. R5: the
    // tie between coincident lines goes to `pick_line`.
    let line_at = |y: f32| -> Option<usize> {
        let candidates: Vec<(usize, f32)> = lines
            .iter()
            .map(|line| (line.index, (y - z_to_y(line.z)).abs()))
            .filter(|&(_, dist)| dist < HIT_THRESHOLD_PX)
            .collect();
        pick_line(&candidates)
    };

    // Check pointer proximity for hover cursor
    let nearest_line = response.hover_pos().and_then(|p| line_at(p.y));
    if nearest_line.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    // R33. The reason for an inert Bottom is the panel's own sentence, not a
    // second wording. `nearest_line` is `Some` only while the pointer is
    // within the hit threshold of that line, so the tooltip belongs to the
    // Bottom line rather than to the whole canvas.
    if !bottom_drives_the_cut
        && nearest_line == Some(BOTTOM_LINE_INDEX)
        && let Some(note) = bottom_z_pin_note(op_type)
    {
        response = response.on_hover_text(note);
    }
    // P3.8: a floor run of an `OpFloor` op names its reference, its Z and,
    // under "each picked face", its face count. Every floor run shares
    // `BOTTOM_LINE_INDEX` (the drag moves the floor), so the hover takes
    // the floor run nearest the pointer.
    if nearest_line == Some(BOTTOM_LINE_INDEX)
        && let Some(pointer) = response.hover_pos()
        && let Some(hover) = lines
            .iter()
            .filter(|line| line.index == BOTTOM_LINE_INDEX)
            .filter_map(|line| {
                let hover = line.hover.as_ref()?;
                Some(((pointer.y - z_to_y(line.z)).abs(), hover))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, hover)| hover.clone())
    {
        response = response.on_hover_text(hover);
    }

    // Labels are pushed apart in the order they are drawn, so two heights a
    // millimetre apart do not print on top of each other. The capture
    // `planning/ui_premium_2026-09-13/review_heights.png` shows `TZ 27.0`
    // and `BZ 26.0` superimposed into one unreadable run. The push is
    // clamped inside the plot, so it can never move a label out of the
    // canvas; at worst two labels share the last row, which is what they
    // did before.
    let mut next_free_y = plot.top();

    for (line, galley) in lines.iter().zip(labels) {
        let y = z_to_y(line.z);
        let is_hovered = nearest_line == Some(line.index);

        // Line
        let stroke_width = if is_hovered {
            LINE_STROKE_HOVER
        } else {
            LINE_STROKE
        };
        let stroke = egui::Stroke::new(stroke_width, line.color);
        let ends = [egui::pos2(plot.left(), y), egui::pos2(line_right, y)];
        if line.dashed {
            painter.extend(egui::Shape::dashed_line(
                &ends,
                stroke,
                tokens::SPACE_2,
                tokens::SPACE_1,
            ));
        } else {
            painter.line_segment(ends, stroke);
        }

        // Label, clamped into the plot on both axes.
        let size = galley.size();
        let text_x = label_x.min(plot.right() - size.x).max(plot.left());
        let text_y_max = (plot.bottom() - size.y).max(plot.top());
        let wanted_y = (y - size.y * 0.5).max(next_free_y);
        let text_y = wanted_y.clamp(plot.top(), text_y_max);
        next_free_y = text_y + size.y;
        painter.galley(egui::pos2(text_x, text_y), galley, line.color);
    }

    // Drag interaction
    let drag_id = ui.id().with("height_drag_idx");

    // The canvas senses click AND drag, so egui reports `drag_started` only
    // after the pointer has moved past the click distance. By then the
    // pointer can sit on a neighbour line. The press origin is where the
    // operator pressed, so the drag takes the line under THAT point. The
    // inspector layer carries no transform, so the global origin is the
    // canvas origin. The hovered line is the fallback.
    if response.drag_started()
        && let Some(idx) = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|p| line_at(p.y))
            .or(nearest_line)
    {
        ui.memory_mut(|mem| mem.data.insert_temp(drag_id, idx));
    }

    // One read serves the drag and the drag stop. `drag_started` and
    // `dragged` are both true on the first frame, so the read follows the
    // insert.
    let drag_idx = ui.memory(|mem| mem.data.get_temp::<usize>(drag_id));

    if response.dragged()
        && let Some(idx) = drag_idx
    {
        let dy = response.drag_delta().y;
        // Convert screen delta to Z delta (screen Y is inverted relative to Z)
        let z_per_pixel = (z_max - z_min) / plot.height() as f64;
        let dz = -(dy as f64) * z_per_pixel;

        // Get current resolved value and apply delta. The per-frame write
        // is the live feedback; `drag_stopped` below rounds it once.
        let current_z = match idx {
            0 => resolved.clearance_z,
            1 => resolved.retract_z,
            2 => resolved.feed_z,
            3 => resolved.top_z,
            _ => resolved.bottom_z,
        };
        // P3.7: the bottom of an `OpFloor` op is its floor. The drag moves
        // the floor in its own reference (L-10); it never pins Bottom Z,
        // which these ops refuse. Under each face the depth moves every
        // face floor together.
        match (idx, owner, floor.as_mut()) {
            (BOTTOM_LINE_INDEX, FloorOwner::OpFloor, Some(f)) => {
                if let Ok(z) = f.inputs.refs.floor_z(f.floor)
                    && let Ok(moved) = f.inputs.refs.at_z(*f.floor, z + dz)
                {
                    *f.floor = moved;
                }
            }
            (BOTTOM_LINE_INDEX, FloorOwner::OpFloor, None) => {}
            // F033: the drag writes through the one order gate, so it stops
            // at the last value that keeps the stack in order.
            _ => {
                let mut draft = heights.clone();
                *height_field(&mut draft, idx) = HeightMode::Manual(current_z + dz);
                let _ = admit_heights_edit(ui, heights, draft, answer, op_type, refusal);
            }
        }
    }

    if response.drag_stopped() {
        // R5. The dragged field lands on the 0.1 mm grid. `dragged` is
        // false on this frame, so nothing writes the raw value back. A
        // press and release under the click distance never starts a drag,
        // so a field the drag did not write is not `Manual` here and stays
        // as it was.
        match (drag_idx, owner, floor.as_mut()) {
            (Some(BOTTOM_LINE_INDEX), FloorOwner::OpFloor, Some(f)) => {
                let rounded = f.floor.with_value(round_pin(f.floor.value()));
                if rounded.door_refusal().is_none() {
                    *f.floor = rounded;
                }
            }
            (Some(BOTTOM_LINE_INDEX), FloorOwner::OpFloor, None) | (None, _, _) => {}
            (Some(idx), _, _) => {
                let mut draft = heights.clone();
                let field = height_field(&mut draft, idx);
                if let HeightMode::Manual(v) = *field {
                    *field = HeightMode::Manual(round_pin(v));
                    // F033: a rounding that crosses a plane is refused too.
                    let _ = admit_heights_edit(ui, heights, draft, answer, op_type, refusal);
                }
            }
        }
        ui.memory_mut(|mem| mem.data.remove::<usize>(drag_id));
    }

    // ── The legend, drawn into the band reserved for it ──────────────────
    let mut legend_x = inner.left();
    let mut legend_y = inner.bottom() - legend_h;
    for (swatch, galley) in legend {
        let size = galley.size();
        let centre_y = legend_y + legend_row_h * 0.5;
        painter.line_segment(
            [
                egui::pos2(legend_x, centre_y),
                egui::pos2(legend_x + LEGEND_SWATCH_W, centre_y),
            ],
            egui::Stroke::new(SWATCH_STROKE, swatch),
        );
        let text_x = legend_x + LEGEND_SWATCH_W + tokens::SPACE_1;
        let text_y = legend_y + (legend_row_h - size.y) * 0.5;
        painter.galley(egui::pos2(text_x, text_y), galley, tokens::TEXT_MUTED);
        if legend_stacked {
            legend_y += legend_row_h + tokens::SPACE_1;
        } else {
            legend_x = text_x + size.x + tokens::SPACE_3;
        }
    }

    // ── What the canvas cannot show, said in words ──────────────────────
    //
    // An empty frame must never stand for a measurement. A flat profile IS
    // a measurement, and it says what it measured. An absent profile
    // abstains with the `NotMeasured` treatment and names the reason.
    match profile {
        ModelProfile::Solid => {}
        // IC1: the legend already says "Model (flat)"; a caption under the
        // canvas said it a second time.
        ModelProfile::Flat => {}
        ModelProfile::Absent => {
            ui.add_space(tokens::SPACE_1);
            ui.horizontal(|ui| {
                ui.add(crate::ui::components::NotMeasured::new().reason(
                    "No model geometry reached this operation, so the diagram \
                     has no model profile to draw.",
                ));
                ui.add(crate::ui::components::OneLine::new(
                    "Model profile: not measured",
                ));
            });
        }
    };
}
