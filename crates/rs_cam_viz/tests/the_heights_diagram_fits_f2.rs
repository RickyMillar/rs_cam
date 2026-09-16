//! F-1 / F-2: the Heights diagram draws its model, and it fits its canvas.
//!
//! # The two defects this exists to catch
//!
//! Both are from `planning/ui_declutter_2026-09-14/PLAN.md` §3, and both are
//! wrong rather than ugly.
//!
//! **F-1 — the diagram drew no model.** Capture
//! `planning/ui_premium_2026-09-13/review_heights.png`, 2026-09-14: the
//! operation `Rivers (back) (copy)` is bound to `rivers_aligned.dxf`, and its
//! Heights diagram shows the stock and nothing else. The legend still prints
//! the word `Model`, which is the tell: `ctx.model_top_z` was `Some`, so the
//! diagram DID have a profile and drew it. A 2D model carries no Z —
//! `rs_cam_core::session::polygons_bbox` reports such a bbox at
//! `z = 0.0 ..= 0.0` —
//! so the model rectangle had ZERO height, and epaint collapses such a
//! rectangle to a one-point line in the stroke colour
//! (`Tessellator::tessellate_rect`). That stroke was `DIAGRAM_MATERIAL`, a
//! dark grey on the dark canvas. Operation 1 of the same setup is bound to
//! `terrain.stl`, has a real Z extent, and drew correctly.
//!
//! **F-2 — the canvas clipped its own labels.** Every text run sat at a
//! hardcoded offset: the value column at `rect.right() - 42.0` and the legend
//! at `rect.bottom() - 8.0`. `ui.painter_at(rect)` then cut off whatever did
//! not fit. Rule F: a container sizes to hold its content, and clipping is a
//! defect, never a layout choice.
//!
//! # Why these invariants, and not a screenshot diff
//!
//! A screenshot diff catches the defect once and then fails on every
//! legitimate change. Two narrower invariants actually hold:
//!
//! 1. **Nothing the diagram paints lies outside the rect it allocated.** The
//!    clip rect is a backstop, not a layout. This is asserted on the SHAPES,
//!    before tessellation, so a shape drawn outside is visible to the test
//!    even though the operator only sees the slice that survived.
//! 2. **A profile the diagram cannot draw is stated, never left blank.** An
//!    absent value is NOT MEASURED in this repo and must never render as a
//!    clean result.
//!
//! Arm 4 is the non-vacuity arm: it proves the layout still MEASURES its
//! text, so a toolkit change cannot make arm 1 pass for the wrong reason.
//!
//! Arm 5 carries ruling R33 (F-6): the Bottom line must agree with the
//! sentence the panel prints beside it. It runs over every `OperationType`,
//! both sides of `honors_pinned_bottom_z`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::state::toolpath::{HeightContext, HeightsConfig, OperationType};
use rs_cam_viz::ui::properties::{bottom_z_pin_note, draw_height_diagram};
use rs_cam_viz::ui::tokens;

/// The panel width the app gives the inspector (`app.rs`, `default_size`).
const PANEL_WIDTH: f32 = 280.0;

/// How far outside the allocated rect a shape may reach. A stroke is centred
/// on its path, so half a stroke width is legitimate; everything the diagram
/// draws sits at least `SPACE_2` inside the rect, which is wider than any
/// stroke it uses.
const EDGE_TOLERANCE: f32 = 0.5;

/// The label column the pre-fix code reserved, in points. It is here only so
/// arm 4 can show that a constant cannot track the text.
const PRE_FIX_LABEL_COLUMN: f32 = 42.0;

/// One headless pass over the diagram.
struct Drawn {
    /// The rect `allocate_exact_size` handed the diagram.
    canvas: egui::Rect,
    /// Every shape recorded in the pass, with its clip rect.
    shapes: Vec<egui::epaint::ClippedShape>,
    /// How wide the block asked to be. The panel clips anything wider.
    requested_width: f32,
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // epaint 0.36's `Drop for TexturesDelta` panics if the delta is dropped
    // unapplied, so every pass clears it before the output goes out of scope.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// Solid model: a mesh with a real Z extent. This is operation 1 on setup 1.
fn solid_ctx() -> HeightContext {
    HeightContext {
        safe_z: 15.0,
        op_depth: 5.0,
        stock_top_z: 9.8,
        stock_bottom_z: -17.2,
        model_top_z: Some(9.8),
        model_bottom_z: Some(0.0),
    }
}

/// Flat model: a 2D drawing. Its bbox reports `z = 0.0 ..= 0.0`. This is
/// operations 2 to 5 on setup 1, the F-1 case.
fn flat_ctx() -> HeightContext {
    HeightContext {
        model_top_z: Some(0.0),
        model_bottom_z: Some(0.0),
        ..solid_ctx()
    }
}

/// No model profile reached the diagram at all.
fn absent_ctx() -> HeightContext {
    HeightContext {
        model_top_z: None,
        model_bottom_z: None,
        ..solid_ctx()
    }
}

/// The operation the arms that are not about R33 use. It honours a pinned
/// Bottom Z, so its Bottom line carries a value and the labels are at their
/// widest.
const BOTTOM_HONOURED: OperationType = OperationType::Adaptive3d;

/// An operation that does NOT read `heights.bottom_z`. Its panel prints
/// "Bottom: Not used by this operation. Its Depth field sets the floor."
const BOTTOM_INERT: OperationType = OperationType::Pocket;

fn draw(height_ctx: &HeightContext, op_type: OperationType) -> Drawn {
    draw_in(&context(), height_ctx, op_type)
}

/// The same pass, in a context the caller supplies. Building a context loads
/// the font set, so an arm that sweeps every operation reuses one.
fn draw_in(ctx: &egui::Context, height_ctx: &HeightContext, op_type: OperationType) -> Drawn {
    let mut heights = HeightsConfig::default();
    let mut requested_width = 0.0_f32;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_max_width(PANEL_WIDTH);
        let inner = ui.scope(|ui| {
            ui.set_max_width(PANEL_WIDTH);
            draw_height_diagram(ui, &mut heights, height_ctx, op_type);
        });
        requested_width = inner.response.rect.width();
    });
    let shapes = std::mem::take(&mut out.shapes);
    out.textures_delta.clear();
    let canvas = canvas_rect(&shapes);
    Drawn {
        canvas,
        shapes,
        requested_width,
    }
}

/// The canvas rect is the rect the diagram fills with `DIAGRAM_CANVAS`. It
/// is the first thing the function paints, and it is exactly the rect
/// `allocate_exact_size` returned.
fn canvas_rect(shapes: &[egui::epaint::ClippedShape]) -> egui::Rect {
    for clipped in shapes {
        if let egui::Shape::Rect(rect_shape) = &clipped.shape
            && rect_shape.fill == tokens::DIAGRAM_CANVAS
        {
            return rect_shape.rect;
        }
    }
    panic!(
        "the diagram painted no `DIAGRAM_CANVAS` background, so this test \
         cannot find the rect it allocated. Re-derive the probe against the \
         current `draw_height_diagram`."
    );
}

impl Drawn {
    /// Every text run the pass painted, canvas and captions alike.
    fn text_runs(&self) -> Vec<String> {
        let mut runs = Vec::new();
        for clipped in &self.shapes {
            if let egui::Shape::Text(text) = &clipped.shape {
                runs.push(text.galley.text().to_owned());
            }
        }
        runs
    }

    fn says(&self, needle: &str) -> bool {
        self.text_runs().iter().any(|run| run.contains(needle))
    }

    /// The right-hand end of the height lines, as a distance from the
    /// canvas's right edge. The label column sits to the right of it, so
    /// this number reports how much width the labels took.
    fn label_column_width(&self) -> f32 {
        let mut rightmost = self.canvas.left();
        for clipped in &self.shapes {
            if let egui::Shape::LineSegment { points, stroke } = &clipped.shape
                && stroke.color == tokens::DIAGRAM_INK
            {
                rightmost = rightmost.max(points[0].x).max(points[1].x);
            }
        }
        self.canvas.right() - rightmost
    }

    /// How many line segments the pass painted in this colour.
    ///
    /// The count, rather than "is there one", because the model legend
    /// swatch shares the model's own colour. A flat profile must add a
    /// SECOND segment — the rule at its one plane — and counting is what
    /// tells the two apart.
    fn segment_count(&self, color: egui::Color32) -> usize {
        self.shapes
            .iter()
            .filter(|clipped| {
                matches!(
                    &clipped.shape,
                    egui::Shape::LineSegment { stroke, .. } if stroke.color == color
                )
            })
            .count()
    }
}

/// Arm 1 — nothing the diagram paints reaches outside the rect it allocated.
#[test]
fn the_diagram_draws_inside_its_own_rect_f2() {
    let cases: [(&str, HeightContext, OperationType); 5] = [
        ("solid model", solid_ctx(), BOTTOM_HONOURED),
        ("flat 2D model", flat_ctx(), BOTTOM_HONOURED),
        ("no model profile", absent_ctx(), BOTTOM_HONOURED),
        // R33 makes the Bottom label a sentence rather than a value, so it
        // is a wider run than any number and belongs in this arm.
        ("inert bottom", solid_ctx(), BOTTOM_INERT),
        (
            // The Heights rows range over -500..=500, so these are the
            // widest values an operator can produce.
            "extreme heights",
            HeightContext {
                safe_z: 500.0,
                op_depth: 500.0,
                stock_top_z: -100.0,
                stock_bottom_z: -500.0,
                model_top_z: Some(-100.0),
                model_bottom_z: Some(-480.0),
            },
            BOTTOM_HONOURED,
        ),
    ];

    for (name, height_ctx, op_type) in cases {
        let drawn = draw(&height_ctx, op_type);
        let allowed = drawn.canvas.expand(EDGE_TOLERANCE);
        let mut escaped: Vec<(String, egui::Rect)> = Vec::new();
        for clipped in &drawn.shapes {
            // Only the diagram's own painter clips to the canvas. The
            // captions below it are ordinary widgets on the panel's clip
            // rect, and arm 3 covers those.
            if clipped.clip_rect != drawn.canvas {
                continue;
            }
            let bounds = clipped.shape.visual_bounding_rect();
            if !bounds.is_finite() || !bounds.is_positive() {
                continue;
            }
            if !allowed.contains_rect(bounds) {
                escaped.push((describe(&clipped.shape), bounds));
            }
        }
        assert!(
            escaped.is_empty(),
            "on the {name} case the diagram painted {} shape(s) outside its \
             own canvas {:?}: {:?}. `painter_at` then slices them, which is \
             exactly what the operator saw. A container sizes to hold its \
             content (Rule F); it does not clip.",
            escaped.len(),
            drawn.canvas,
            escaped
        );
        assert!(
            drawn.requested_width <= PANEL_WIDTH + EDGE_TOLERANCE,
            "on the {name} case the Heights diagram asked for {} points \
             inside a {PANEL_WIDTH} point panel, so the panel clips it.",
            drawn.requested_width
        );
    }
}

/// A short name for a shape, for the failure message.
fn describe(shape: &egui::Shape) -> String {
    match shape {
        egui::Shape::Text(text) => format!("text {:?}", text.galley.text()),
        egui::Shape::Rect(_) => "rect".to_owned(),
        egui::Shape::LineSegment { .. } => "line".to_owned(),
        other => format!("{other:?}"),
    }
}

/// Arm 2 — a profile the diagram cannot draw is STATED, never left blank.
#[test]
fn the_diagram_states_what_it_cannot_draw_f1() {
    // A missing profile is NOT MEASURED. It draws the `NotMeasured`
    // treatment and names itself, rather than leaving an empty frame that
    // reads as a measured "no model".
    let absent = draw(&absent_ctx(), BOTTOM_HONOURED);
    assert!(
        absent.says(tokens::GLYPH_UNKNOWN),
        "with no model profile the diagram drew no `NotMeasured` glyph. An \
         absent value must never render as a clean result. Text runs: {:?}",
        absent.text_runs()
    );
    assert!(
        absent.says("not measured"),
        "with no model profile the diagram did not say so in words. Text \
         runs: {:?}",
        absent.text_runs()
    );

    // A flat profile IS measured, and its measurement is zero Z extent. It
    // draws a visible rule and says the drawing is flat. Before F-1 it drew
    // a zero-height rectangle that epaint collapsed into a near-invisible
    // one-point line, and said nothing.
    let flat = draw(&flat_ctx(), BOTTOM_HONOURED);
    let solid = draw(&solid_ctx(), BOTTOM_HONOURED);
    let flat_rules = flat.segment_count(tokens::DIAGRAM_DIM);
    let solid_rules = solid.segment_count(tokens::DIAGRAM_DIM);
    assert!(
        flat_rules > solid_rules,
        "a flat 2D model drew {flat_rules} model-coloured rules against a \
         solid model's {solid_rules}, so it drew no profile of its own. \
         This is F-1: a drawing has zero Z extent, so its profile is a rule, \
         and a zero-height rectangle is invisible."
    );
    assert!(
        flat.says("flat"),
        "a flat 2D model did not say that its profile is one plane. Text \
         runs: {:?}",
        flat.text_runs()
    );
    assert!(
        !flat.says(tokens::GLYPH_UNKNOWN),
        "a flat profile is MEASURED, not absent, so it must not abstain."
    );

    // A solid model neither abstains nor claims to be flat.
    assert!(
        solid.says("Model"),
        "a solid model lost its legend. Text runs: {:?}",
        solid.text_runs()
    );
    assert!(
        !solid.says(tokens::GLYPH_UNKNOWN),
        "a solid model must not draw the `NotMeasured` treatment."
    );
}

/// Arm 3 — the captions under the canvas stay inside the panel too.
///
/// A caption is where UP4's D-16 defect lived: `TextWrapMode::Extend` sets an
/// INFINITE max width, so one long sentence grows the `Ui` past the panel and
/// the panel then draws the over-wide content off its own left edge.
#[test]
fn the_diagram_captions_stay_inside_the_panel_f2() {
    for height_ctx in [solid_ctx(), flat_ctx(), absent_ctx()] {
        let drawn = draw(&height_ctx, BOTTOM_HONOURED);
        assert!(
            drawn.requested_width <= PANEL_WIDTH + EDGE_TOLERANCE,
            "the Heights diagram and its captions asked for {} points \
             inside a {PANEL_WIDTH} point panel.",
            drawn.requested_width
        );
    }
}

/// Arm 4 — non-vacuity. The layout still MEASURES its text.
///
/// Without this, a toolkit change that made every galley zero-width would
/// satisfy arm 1 while the operator kept reading a sliced label.
#[test]
fn the_layout_still_measures_its_text_f2() {
    // (a) A longer value takes a wider column. If these two agreed, the
    //     column would be a constant again and arm 1 would prove nothing.
    let short = draw(
        &HeightContext {
            safe_z: 5.0,
            op_depth: 1.0,
            stock_top_z: 0.0,
            stock_bottom_z: -1.0,
            model_top_z: Some(0.0),
            model_bottom_z: Some(-1.0),
        },
        BOTTOM_HONOURED,
    );
    let long = draw(
        &HeightContext {
            safe_z: 400.0,
            op_depth: 100.0,
            stock_top_z: 200.0,
            stock_bottom_z: -200.0,
            model_top_z: Some(200.0),
            model_bottom_z: Some(-100.0),
        },
        BOTTOM_HONOURED,
    );
    let short_column = short.label_column_width();
    let long_column = long.label_column_width();
    assert!(
        long_column > short_column + 1.0,
        "the label column measured {short_column} for short values and \
         {long_column} for long ones. A column that does not widen with its \
         text is a constant, and a constant cannot hold its content — which \
         is the mechanism F-2 is about."
    );

    // (b) The constant the pre-fix code used is too narrow for a label this
    //     diagram can emit, so the old rule really did clip.
    let ctx = context();
    let mut widest = 0.0_f32;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let font = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Monospace);
        let galley = ui
            .painter()
            .layout_no_wrap("BZ -500.0".to_owned(), font, tokens::DANGER);
        widest = galley.size().x;
    });
    out.textures_delta.clear();
    assert!(
        widest > PRE_FIX_LABEL_COLUMN,
        "the widest label the diagram can emit measures {widest} points, \
         which fits the {PRE_FIX_LABEL_COLUMN} point column the pre-fix code \
         reserved. Re-derive this arm against the current font: it no longer \
         reproduces F-2's mechanism."
    );
}

/// Arm 5 — F-6 / ruling R33: the Bottom line agrees with the panel beside it.
///
/// # The defect
///
/// The diagram drew a `DANGER` red floor at `resolved.bottom_z` on every
/// operation. `OperationType::honors_pinned_bottom_z`
/// (`rs_cam_core::compute::catalog`) says three of the twenty-four read that
/// dial. On the other twenty-one the panel printed "Bottom: Not used by this
/// operation. …" and the diagram under it drew a red floor: two surfaces
/// giving opposite answers about where the cut stops, with the strongest
/// mark on the canvas sitting on the one line that does not apply.
///
/// # Why the colour, and not the pixels
///
/// `DANGER` appears nowhere else in this diagram, so "the pass painted no
/// `DANGER` shape" is an exact reading of "the Bottom line is not marked as
/// the floor". The arm runs over EVERY `OperationType`, both sides of the
/// predicate, so a new operation cannot reopen the defect by being added.
///
/// # The inert run carries NO value, and that is deliberate
///
/// The assertion below asks for `BZ (not used)`, not `BZ 26.0 (not used)`.
/// The diagram pictures CUT LIMITS: every other run on it names a Z that
/// bounds the cut, and an inert pin does not, because the operation floors
/// at `top_z` minus its own depth dial. A number beside four real limits
/// reads as operative whatever follows it in brackets. The value lives in
/// the Bottom field above, which is about the setting rather than the cut.
#[test]
fn the_bottom_line_agrees_with_the_panel_r33() {
    let mut honoured = 0_usize;
    let mut inert = 0_usize;

    let ctx = context();
    for &op_type in OperationType::ALL {
        let drawn = draw_in(&ctx, &solid_ctx(), op_type);
        let reds = drawn.segment_count(tokens::DANGER);
        if op_type.honors_pinned_bottom_z() {
            honoured += 1;
            assert!(
                reds >= 1,
                "{op_type:?} DOES read `heights.bottom_z`, so its Bottom line \
                 must still be marked as the floor. It painted {reds} \
                 DANGER-coloured lines."
            );
            assert!(
                !drawn.says("not used"),
                "{op_type:?} reads `heights.bottom_z`, so the diagram must \
                 not say the row is unused. Text runs: {:?}",
                drawn.text_runs()
            );
        } else {
            inert += 1;
            assert!(
                reds == 0,
                "{op_type:?} does NOT read `heights.bottom_z` — the panel \
                 beside this diagram prints {:?} — yet the diagram painted \
                 {reds} DANGER-coloured lines. Red is the strongest mark on \
                 this canvas and it is on the one line that does not apply.",
                bottom_z_pin_note(op_type)
            );
            // The line is NOT deleted. The operator set that value, and a
            // diagram that drops it invites the reading "the pin did not
            // take". It is drawn in `UNKNOWN` and names its state.
            assert!(
                drawn.segment_count(tokens::UNKNOWN) >= 1,
                "{op_type:?} lost its Bottom line altogether. R33 keeps the \
                 line and changes the mark."
            );
            assert!(
                drawn.says("BZ (not used)"),
                "{op_type:?} did not name the Bottom line's state. Text \
                 runs: {:?}",
                drawn.text_runs()
            );
        }
    }

    // Non-vacuity: the predicate really has two sides, and this arm drove
    // both. If a future edit made every operation answer the same way, the
    // assertions above would pass without testing anything.
    assert!(
        honoured >= 1 && inert >= 1,
        "the arm drove {honoured} honouring and {inert} inert operations, so \
         it exercised only one side of `honors_pinned_bottom_z`."
    );
}
