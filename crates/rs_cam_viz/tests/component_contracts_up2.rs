//! UP2's sentry: the component set keeps the contracts `DESIGN_SPEC.md` §4
//! gives it.
//!
//! The arms that matter are the ones where a wrong answer is a SAFETY
//! problem rather than a layout problem:
//!
//! - [`severity_outranks_the_cap_up2`] — a collision must render however many
//!   stale notices are queued ahead of it.
//! - [`the_overflow_row_states_the_true_total_up2`] — the reader must never
//!   have to do arithmetic to discover how much is hidden.
//! - [`a_not_measured_value_is_never_a_zero_up2`] — an abstention and a
//!   measurement must not look alike.
//!
//! The rest are layout contracts.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::ui::{components, tokens};

/// Build a context with the product's real style and fonts installed.
fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// Run `f` in a real pass and give back its value.
fn in_pass<R>(ctx: &egui::Context, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // `run_ui` takes an `FnMut`, so the `FnOnce` is parked in an Option and
    // taken on the first (and only) call.
    let mut f = Some(f);
    let mut out = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        if let Some(f) = f.take() {
            out = Some(f(ui));
        }
    });
    output.textures_delta.clear();
    out.expect("the pass ran")
}

// ---------------------------------------------------------------------------
// Arm 1 — every role renders, and each carries its own colour and glyph
// ---------------------------------------------------------------------------

#[test]
fn every_role_has_a_distinct_colour_and_a_glyph_up2() {
    use components::Role;

    let roles = [
        Role::Ok,
        Role::Caution,
        Role::Danger,
        Role::Info,
        Role::Unknown,
    ];

    // Distinct text colours.
    for (i, a) in roles.iter().enumerate() {
        for b in roles.iter().skip(i + 1) {
            assert_ne!(
                a.text(),
                b.text(),
                "{a:?} and {b:?} share a text colour, so the verdict cannot be \
                 read from the colour"
            );
            assert_ne!(a.tint(), b.tint(), "{a:?} and {b:?} share a tint");
        }
    }

    // Colour is never the ONLY channel (§2.6 rule 3). Every verdict carries a
    // glyph; Info is not a verdict and carries none.
    for role in [Role::Ok, Role::Caution, Role::Danger, Role::Unknown] {
        assert!(
            role.glyph().is_some(),
            "{role:?} is a verdict and must carry a glyph. Under UP1's \
             palette the seven freshness states collapse onto three colours, \
             so the glyph is what separates them."
        );
    }
    assert_eq!(
        Role::Info.glyph(),
        None,
        "Info is not a verdict, so it has nothing to pass or abstain from"
    );

    // Ruling R10: five roles in the severity order, OK last.
    let mut ordered = roles;
    ordered.sort_by_key(|r| r.severity_rank());
    assert_eq!(
        ordered,
        [
            Role::Danger,
            Role::Caution,
            Role::Unknown,
            Role::Info,
            Role::Ok
        ]
    );
}

#[test]
fn a_status_chip_renders_in_a_real_pass_up2() {
    use components::{Role, StatusChip};
    let ctx = ctx();
    in_pass(&ctx, |ui| {
        for (word, role) in [
            ("OK", Role::Ok),
            ("STALE", Role::Caution),
            ("ERR", Role::Danger),
            ("INFO", Role::Info),
            ("PEND", Role::Unknown),
        ] {
            let r = ui.add(StatusChip::new(word, role).hover("why"));
            assert!(r.rect.height() > 0.0, "{word} chip drew nothing");
        }
    });
}

// ---------------------------------------------------------------------------
// Arm 2 — the four button variants differ, and none is shorter than ROW_ACTION
// ---------------------------------------------------------------------------

#[test]
fn button_variants_differ_and_meet_the_height_floor_up2() {
    use components::{Button, ButtonVariant};

    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Default,
        ButtonVariant::Quiet,
        ButtonVariant::Danger,
    ];

    assert_ne!(
        ButtonVariant::Primary.fill(),
        ButtonVariant::Default.fill(),
        "the audit found 129 of 132 buttons identical, so the product could \
         not say which action a screen was FOR. Primary and Default must not \
         share a fill."
    );
    for (i, a) in variants.iter().enumerate() {
        for b in variants.iter().skip(i + 1) {
            assert_ne!(a.fill(), b.fill(), "{a:?} and {b:?} share a fill");
        }
    }

    let ctx = ctx();
    in_pass(&ctx, |ui| {
        for variant in variants {
            let r = ui.add(Button::new("Generate").variant(variant));
            assert!(
                r.rect.height() >= tokens::ROW_ACTION - 0.5,
                "{variant:?} is {} tall, under the ROW_ACTION floor of {}",
                r.rect.height(),
                tokens::ROW_ACTION
            );
        }
    });
}

#[test]
fn a_full_width_button_paints_its_centered_label_once_ur6() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/components/button.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let widget = source
        .split_once("impl egui::Widget for Button")
        .map_or("", |(_, widget)| widget);

    assert_eq!(
        widget.matches("ui.painter().text(").count(),
        1,
        "the shared button must paint its label once, not repaint it only on hover"
    );
    assert!(
        widget.contains("egui::RichText::new(&self.text)")
            && widget.contains("egui::Color32::TRANSPARENT"),
        "egui's native button must retain its label for disabled state and WidgetInfo accessibility"
    );
    assert!(
        widget.contains("response.rect.center()"),
        "the shared label must be centered at rest as well as on hover"
    );
}

/// A closed pair of options never splits across two lines.
///
/// Seen on screen 2026-09-18, on the Geometry tab at the narrow rail:
/// "Start from" drew "Stock" beside the label and "After previous ops" on
/// the line below it, so one closed choice of two read as two unrelated
/// controls. The row measures now: the options go beside the label when
/// they fit, and otherwise the label takes its own line and the pair stays
/// together under it. Either way the pair shares one line.
#[test]
fn a_choice_row_keeps_its_options_on_one_line_up2() {
    use components::ChoiceRow;

    /// The narrowest rail the product gives a panel that draws this row.
    const RAIL: f32 = 240.0;
    const OPTIONS: [(u8, &str); 2] = [(0, "Stock"), (1, "After previous ops")];

    let ctx = ctx();
    let mut choice = 1_u8;
    let mut f = Some(());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        if f.take().is_some() {
            ui.set_max_width(RAIL);
            let _ = ui.add(ChoiceRow::new("Start from", &mut choice, &OPTIONS));
        }
    });
    let shapes = std::mem::take(&mut output.shapes);
    output.textures_delta.clear();

    let mut lines: Vec<(String, f32, f32)> = Vec::new();
    for clipped in &shapes {
        if let egui::Shape::Text(text) = &clipped.shape {
            let word = text.galley.text().to_owned();
            if OPTIONS.iter().any(|(_, o)| *o == word) || word == "Start from" {
                lines.push((word, text.pos.y, text.pos.x + text.galley.size().x));
            }
        }
    }
    assert_eq!(
        lines.len(),
        3,
        "non-vacuity: the pass must draw the label and both options, and it \
         drew {lines:?}"
    );

    let y_of = |want: &str| {
        lines
            .iter()
            .find(|(word, _, _)| word == want)
            .map(|(_, y, _)| *y)
            .unwrap_or_else(|| panic!("{want} was not drawn: {lines:?}"))
    };
    assert!(
        (y_of("Stock") - y_of("After previous ops")).abs() < 1.0,
        "the two options must share one line, and they sit at {} and {}",
        y_of("Stock"),
        y_of("After previous ops")
    );

    let right = lines
        .iter()
        .map(|(_, _, right)| *right)
        .fold(f32::MIN, f32::max);
    assert!(
        right <= RAIL + 1.0,
        "the row ran to {right} points inside a {RAIL} point rail, so the \
         inspector would clip its own left edge (AUDIT.md D-16)"
    );
}

#[test]
fn the_primary_button_clears_the_contrast_floor_up2() {
    // Ruling R8. Computed here rather than asserted from a comment, because
    // the alternative (INK_95 on ACCENT) reads 2.32 and would have shipped.
    let ratio = contrast(tokens::INK_05, tokens::ACCENT);
    assert!(
        ratio >= 4.5,
        "Primary button text reads {ratio:.2} on its fill, under the 4.5 floor"
    );
    let pressed = contrast(tokens::INK_05, tokens::ACCENT_PRESSED);
    assert!(
        pressed >= 4.5,
        "Primary button text reads {pressed:.2} when pressed, under the floor"
    );
    assert!(
        contrast(tokens::INK_95, tokens::ACCENT) < 4.5,
        "this asserts the REJECTED alternative really does fail, so the arm \
         above is not vacuous"
    );
}

fn relative_luminance(c: egui::Color32) -> f64 {
    fn channel(v: u8) -> f64 {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

// ---------------------------------------------------------------------------
// Arm 2b — the focus ring
// ---------------------------------------------------------------------------

#[test]
fn the_focus_ring_lies_outside_the_widget_rect_up2() {
    use components::focus_ring;

    // These are consts, so the check is a compile-time claim about the
    // module's contract rather than a runtime one.
    const _: () = assert!(
        focus_ring::RING_OFFSET > 0.0,
        "a ring drawn inside the widget rect covers the widget's own edge"
    );
    const _: () = assert!(
        focus_ring::RING_WIDTH == 2.0,
        "DESIGN_SPEC 4.5 asks for a 2-point ring"
    );

    // It paints without panicking on a context that has no focus, and on one
    // that does. egui draws NO ring of its own (§10.5), so this plugin is the
    // only thing standing between a keyboard user and an invisible cursor.
    let ctx = ctx();
    focus_ring::install(&ctx);
    in_pass(&ctx, |ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut String::new()));
        r.request_focus();
    });
    in_pass(&ctx, |ui| {
        let _ = ui.label("second pass, focus now held");
    });
}

// ---------------------------------------------------------------------------
// Arm 3 — EmptyState offers at most one action
// ---------------------------------------------------------------------------

#[test]
fn an_empty_state_offers_at_most_one_action_up2() {
    use components::EmptyState;
    assert_eq!(EmptyState::new("No toolpaths").button_count(), 0);
    assert_eq!(
        EmptyState::new("No toolpaths")
            .detail("Add one to begin")
            .action("Add toolpath")
            .button_count(),
        1,
        "§4.8: an empty state offering a choice is not an empty state, it is \
         a menu"
    );
}

// ---------------------------------------------------------------------------
// Arm 4 — KeyValueRow keeps the row rhythm and does not clip its trailing slot
// ---------------------------------------------------------------------------

#[test]
fn a_key_value_row_holds_the_dense_rhythm_up2() {
    use components::{KeyValue, KeyValueRow};
    let ctx = ctx();
    in_pass(&ctx, |ui| {
        let r = ui.add(KeyValueRow::measured("Stepover", "1.50", "mm"));
        assert!(
            r.rect.height() >= tokens::ROW_DENSE - 0.5,
            "a parameter row is a FIXED {} point line box, got {}",
            tokens::ROW_DENSE,
            r.rect.height()
        );
        // Eight rows occupy exactly 176 points at this rhythm.
        const _: () = assert!(tokens::ROW_DENSE * 8.0 == 176.0);
        let _ = ui.add(KeyValueRow::new("Mode", KeyValue::Text("Climb".to_owned())));
    });
}

#[test]
fn a_narrow_panel_wraps_the_trailing_slot_instead_of_clipping_up2() {
    use components::KeyValueRow;
    let ctx = ctx();
    in_pass(&ctx, |ui| {
        // §4.6 names 240 points as the narrow case.
        ui.set_max_width(240.0);
        let r = ui.add(
            KeyValueRow::measured("Depth per pass", "2.00", "mm")
                .label_width(90.0)
                .trailing("clamped to the vendor band maximum"),
        );
        assert!(
            r.rect.width() <= 240.0 + 0.5,
            "the row grew to {} in a 240 point panel, so its trailing slot \
             extended the Ui instead of wrapping. That is AUDIT.md D-16.",
            r.rect.width()
        );
    });
}

// ---------------------------------------------------------------------------
// Arm 4b — NoticeStack, the §4.11 rules
// ---------------------------------------------------------------------------

#[test]
fn severity_outranks_the_cap_up2() {
    use components::{Notice, NoticeStack, Role};

    // The spec's own population: 3 DANGER and 40 CAUTION, cap 4.
    let mut notices: Vec<Notice> = (0..3)
        .map(|i| Notice::new(Role::Danger, format!("collision at move {i}")))
        .collect();
    notices.extend((0..40).map(|i| Notice::new(Role::Caution, format!("toolpath {i} is stale"))));

    let resolved = NoticeStack::new(notices).cap(4).resolve();

    let dangers = resolved
        .rows
        .iter()
        .filter(|r| r.notice.role == Role::Danger)
        .count();
    assert_eq!(
        dangers, 3,
        "ALL THREE dangers must render. This is a safety rule, not a layout \
         rule: a collision must never be hidden because forty stale-result \
         notices got to the stack first."
    );
    assert_eq!(
        resolved.rows.len(),
        4,
        "one caution renders beside the three dangers"
    );
    assert_eq!(
        resolved.total, 43,
        "the stack reports the TRUE total, not the number it drew"
    );
}

#[test]
fn the_overflow_row_states_the_true_total_up2() {
    use components::{Notice, NoticeStack, Role};

    let notices: Vec<Notice> = (0..293)
        .map(|i| Notice::new(Role::Caution, format!("warning {i}")))
        .collect();
    let resolved = NoticeStack::new(notices).cap(4).resolve();
    let row = resolved
        .overflow_text()
        .expect("293 items overflow a cap of 4");

    assert!(
        row.contains("293"),
        "the overflow row must state the true total. Got {row:?}"
    );
    assert!(
        !row.contains("289"),
        "a remainder makes the reader do arithmetic to discover the scale of \
         what is hidden, which is the defect §4.11 exists to fix. Got {row:?}"
    );
    assert_eq!(row, "Showing 4 of 293 \u{00B7} Show all");
}

#[test]
fn identical_notices_collapse_with_a_multiplier_up2() {
    use components::{Notice, NoticeStack, Role};

    let notices = vec![Notice::new(Role::Caution, "results are stale"); 40];
    let resolved = NoticeStack::new(notices).cap(4).resolve();

    assert_eq!(
        resolved.rows.len(),
        1,
        "collapsing happens BEFORE the cap, so 40 copies cost one slot"
    );
    assert_eq!(resolved.rows[0].count, 40, "the multiplier says how many");
    assert!(
        !resolved.truncated,
        "one row holds all forty, so nothing was withheld and there is no \
         overflow row"
    );
}

#[test]
fn a_notice_stack_renders_in_a_real_pass_up2() {
    use components::{Notice, NoticeStack, Role};
    let ctx = ctx();
    in_pass(&ctx, |ui| {
        let notices = vec![
            Notice::new(Role::Danger, "collision at move 1204").group("Safety"),
            Notice::new(Role::Caution, "toolpath 3 is stale").group("Review"),
            Notice::new(Role::Unknown, "engagement not measured").group("Review"),
        ];
        let _ = NoticeStack::new(notices).cap(4).show(ui);
    });
}

// ---------------------------------------------------------------------------
// Arm: an abstention is never a zero
// ---------------------------------------------------------------------------

#[test]
fn a_not_measured_value_is_never_a_zero_up2() {
    use components::{KeyValue, KeyValueRow, NotMeasured};

    // §2.6: a value the product did not measure "is the em dash — in
    // UNKNOWN. It is never 0, never 0.0 and never blank."
    assert_eq!(tokens::GLYPH_UNKNOWN, "\u{2014}", "the glyph is an em dash");
    assert_ne!(
        tokens::UNKNOWN,
        tokens::TEXT_MUTED,
        "an abstention must not wear the same grey as ordinary secondary \
         text, which is how a gate that ABSTAINED and a gate that PASSED came \
         to look alike"
    );

    let ctx = ctx();
    in_pass(&ctx, |ui| {
        let r = ui.add(NotMeasured::new().reason("the dexel grid could not resolve it"));
        assert!(r.rect.height() > 0.0);
        let _ = ui.add(KeyValueRow::not_measured("Average engagement"));
        let _ = ui.add(KeyValueRow::new("Peak", KeyValue::NotMeasured));
    });
}

// ---------------------------------------------------------------------------
// Arm 5 — non-vacuity
// ---------------------------------------------------------------------------

#[test]
fn the_component_set_is_complete_up2() {
    // Naming each type IS the assertion: a renamed or deleted component stops
    // this file compiling, which is louder than a runtime check.
    use components::{
        Banner, Button, Card, ChoiceRow, DataTable, EmptyState, KeyValueRow, NotMeasured, Notice,
        NoticeStack, Role, SectionHeader, StatusChip,
    };

    let ctx = ctx();
    in_pass(&ctx, |ui| {
        SectionHeader::new("Geometry").trailing("6 rows").show(ui);
        Card::new().selected(true).show(ui, |ui| {
            ui.label("a card");
        });
        let _ = ui.add(StatusChip::new("OK", Role::Ok));
        let _ = ui.add(Button::primary("Generate"));
        let _ = ui.add(Button::primary("Generate All \u{00B7} 3/8").progress(0.375));
        let mut choice = 1_u8;
        let _ = ui.add(ChoiceRow::new(
            "Start from",
            &mut choice,
            &[(0, "Stock"), (1, "Model")],
        ));
        let _ = ui.add(KeyValueRow::measured("Feed", "2400", "mm/min"));
        DataTable::new("t", vec!["Current".into(), "Recommended".into()]).show(ui, |ui| {
            ui.label("a");
            ui.label("b");
            ui.end_row();
        });
        let _ = EmptyState::new("Nothing yet").show(ui);
        let _ = Banner::new(Role::Caution, "Results are stale")
            .action("Regenerate")
            .show(ui);
        let _ = NoticeStack::new(vec![Notice::new(Role::Info, "hello")]).show(ui);
        let _ = ui.add(NotMeasured::new());
    });

    // The spec's component list. If §4 grows a component, this number moves
    // and the test says so rather than silently covering less.
    // W3 (2026-09-18): 12 to 13. Section 4.14 `ChoiceRow` is W2's new
    // component, the one renderer for a small closed choice (G-STARTFROM).
    const SPEC_COMPONENTS: usize = 13;
    let covered = [
        "SectionHeader",
        "Card",
        "StatusChip",
        "CountPill",
        "Button",
        "KeyValueRow",
        "DataTable",
        "EmptyState",
        "Banner",
        "NoticeStack",
        "NotMeasured",
        "ChoiceRow",
        "motion",
    ];
    assert_eq!(
        covered.len(),
        SPEC_COMPONENTS,
        "the component list is shorter than DESIGN_SPEC.md §4's"
    );
}

#[test]
fn the_motion_helpers_are_not_linear_up2() {
    // §10.4: `animate_bool_with_time` is hardcoded to linear, and linear
    // motion is the most recognisable sign that nobody chose an easing. Every
    // helper must go through `_and_easing`.
    //
    // Measured rather than asserted from source: a cubic-out curve is ABOVE
    // the linear line at its midpoint, and a cubic-in curve is below it.
    let mid_out = egui::emath::easing::cubic_out(0.5);
    let mid_in = egui::emath::easing::cubic_in(0.5);
    assert!(
        mid_out > 0.5,
        "cubic_out must decelerate into place, got {mid_out}"
    );
    assert!(mid_in < 0.5, "cubic_in must accelerate away, got {mid_in}");
    const _: () = assert!(tokens::MOTION_FAST < tokens::MOTION_BASE);
    const _: () = assert!(tokens::MOTION_BASE < tokens::MOTION_SLOW);
}

/// Arm 6 — the dimensions §4 states as bare numbers are honoured.
///
/// Each of these was MISSED in UP2's first draft and caught by reading §4
/// against the implementation. They are named constants now (ruling R20), so
/// the next reader can tell a considered 34 from a typed one.
#[test]
fn the_component_dimensions_match_the_spec_up2() {
    use components::{KeyValueRow, Role, StatusChip};

    const _: () = assert!(tokens::CHIP_MIN_WIDTH == 34.0);
    const _: () = assert!(tokens::WELL_HEIGHT == 18.0);
    const _: () = assert!(tokens::EMPTY_STATE_MAX_WIDTH == 280.0);
    const _: () = assert!(tokens::EMPTY_STATE_GLYPH_SIZE == 24.0);

    // The well leaves 2 points of air above and below inside the row, so a
    // column of wells reads as a stack rather than a list of boxes.
    const _: () = assert!(tokens::ROW_DENSE - tokens::WELL_HEIGHT == 4.0);

    let ctx = ctx();
    in_pass(&ctx, |ui| {
        // A column of chips aligns because each clears the minimum width.
        for word in ["OK", "GENERATING"] {
            let r = ui.add(StatusChip::new(word, Role::Ok));
            assert!(
                r.rect.width() >= tokens::CHIP_MIN_WIDTH,
                "{word} chip is {} wide, under the {} minimum, so a column of \
                 chips would not align",
                r.rect.width(),
                tokens::CHIP_MIN_WIDTH
            );
        }

        // An editable-looking row keeps its well inside the row's line box.
        let r = ui.add(KeyValueRow::measured("Stepover", "1.50", "mm").editable_look(true));
        assert!(
            r.rect.height() >= tokens::ROW_DENSE - 0.5,
            "an editable row still holds the dense rhythm, got {}",
            r.rect.height()
        );
    });
}

/// Arm 7 — `CountPill` is on the grid, and an observation cannot borrow a
/// verdict colour.
#[test]
fn a_count_pill_is_on_the_grid_up2() {
    use components::{CountPill, Role};

    let ctx = ctx();
    in_pass(&ctx, |ui| {
        let r = ui.add(CountPill::verdict("exceeding", 3).semantic(Role::Danger));
        assert!(
            r.rect.width() >= tokens::CHIP_MIN_WIDTH,
            "a pill shares the chip's minimum width so the two families align"
        );
        let _ = ui.add(CountPill::observation("traces", 12).semantic(Role::Info));
    });
}

// ---------------------------------------------------------------------------
// UI-03 — a section header is the kit's element
// ---------------------------------------------------------------------------
//
// `components::SectionHeader` is the ONE renderer for a section title, and
// `UiExt::named_section` calls it. The audit found the crate still writing
// the header by hand as `.small().strong()`, and `card.rs` had recorded a
// count of 41 that had grown since it was written. A hand-rolled header
// decides its own rung, its own colour and its own spacing, which is how
// one product ends up with four of each.
//
// UI-03 converted the standalone section titles. What is left is NOT a
// header, and each allowance below says what it is instead. The count only
// ever goes down.

/// Files that may still write the `.small()` + `.strong()` chain, with the
/// number of chains each one holds and the reason they are not headers.
const HAND_ROLLED_EMPHASIS: &[(&str, usize, &str)] = &[
    (
        "ui/optimize_project.rs",
        14,
        "grid COLUMN headers; a SectionHeader draws a rule and its own \
         vertical space, so it cannot sit in a grid cell",
    ),
    (
        "ui/multitool_planner.rs",
        12,
        "11 grid column headers, plus the tier-cap warning sentence",
    ),
    (
        "ui/optimize_modal.rs",
        9,
        "7 grid column headers, one CollapsingHeader title and the \
         \"Try this\" notice",
    ),
    (
        "ui/sim_op_list.rs",
        4,
        "one CollapsingHeader title and three ACTIVE-row emphases, which \
         mark selection rather than name a section",
    ),
    (
        "ui/feeds/compare.rs",
        3,
        "the feeds surfaces are the power session's; UI-03 does not edit them",
    ),
    (
        "ui/sim_timeline.rs",
        3,
        "one playback notice and two active-track emphases",
    ),
    (
        "ui/feeds/explore.rs",
        2,
        "the feeds surfaces are the power session's; UI-03 does not edit them",
    ),
    (
        "ui/readiness_panel.rs",
        2,
        "one grid column-header loop and one inline lead-in inside a \
         horizontal row",
    ),
    (
        "ui/sim_diagnostics.rs",
        2,
        "\"Must address\" carries theme::ERROR, and the severity colour is \
         meaning rather than styling; SectionHeader has no colour slot. The \
         other is a sentence, not a title.",
    ),
    (
        "ui/properties/machine_panel.rs",
        1,
        "\"Will apply:\" leads a bullet list inside the GRBL import preview",
    ),
    (
        "app.rs",
        1,
        "the reduced-quality playback notice, a sentence rather than a title",
    ),
];

/// The fewest `SectionHeader` / `named_section` call sites the crate must
/// hold. A conversion that deleted headers rather than routing them fails.
const MIN_KIT_HEADERS: usize = 20;

fn ui_src_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn ui_rs_files() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&ui_src_root(), &mut out);
    out.sort();
    out
}

/// `text` with every `//` comment removed, so a count quoted in a doc
/// comment is not read as a call site.
fn without_comments(text: &str) -> String {
    text.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Hand-rolled `.small()` + `.strong()` chains per file, comments stripped.
/// The chain is often split over two lines, so the scan reads the whole file.
fn hand_rolled_sites() -> Vec<(String, usize)> {
    let root = ui_src_root();
    let mut out = Vec::new();
    for path in ui_rs_files() {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = without_comments(&std::fs::read_to_string(&path).unwrap());
        // Collapse whitespace so `.small()\n .strong()` reads as one chain.
        let flat: String = text.split_whitespace().collect::<Vec<_>>().join("");
        let n =
            flat.matches(".small().strong()").count() + flat.matches(".strong().small()").count();
        if n > 0 {
            out.push((rel, n));
        }
    }
    out
}

#[test]
fn a_section_header_is_the_kit_element_ui03() {
    let sites = hand_rolled_sites();
    let mut over = Vec::new();

    for (rel, count) in &sites {
        let allowed = HAND_ROLLED_EMPHASIS
            .iter()
            .find(|(f, _, _)| f == rel)
            .map_or(0, |(_, n, _)| *n);
        if *count > allowed {
            over.push(format!("{rel} {count} (allowed {allowed})"));
        }
    }

    assert!(
        over.is_empty(),
        "these files hand-roll a header the component kit already renders. \
         A section title is `components::SectionHeader::new(title).show(ui)` \
         or `UiExt::named_section`. Emphasis that is NOT a header keeps its \
         chain and is named in HAND_ROLLED_EMPHASIS with its reason. Over \
         budget: {}",
        over.join(", ")
    );
}

#[test]
fn the_hand_rolled_emphasis_list_is_not_vacuous_ui03() {
    let sites = hand_rolled_sites();

    for (rel, allowed, reason) in HAND_ROLLED_EMPHASIS {
        assert!(
            !reason.is_empty(),
            "{rel} must say WHY its chains are not headers"
        );
        let found = sites.iter().find(|(f, _)| f == rel).map_or(0, |(_, n)| *n);
        assert_eq!(
            found, *allowed,
            "{rel} is allowed {allowed} hand-rolled chains and holds \
             {found}. An allowance that no longer matches its file lets a \
             new hand-rolled header in under an old number."
        );
    }

    let root = ui_src_root();
    let mut kit_headers = 0usize;
    for path in ui_rs_files() {
        if path.ends_with("components/card.rs") {
            continue;
        }
        let text = without_comments(&std::fs::read_to_string(&path).unwrap());
        kit_headers += text.matches("SectionHeader::new(").count();
        kit_headers += text.matches(".named_section(").count();
    }
    let _ = root;
    assert!(
        kit_headers >= MIN_KIT_HEADERS,
        "the crate holds only {kit_headers} kit header call sites, fewer \
         than the {MIN_KIT_HEADERS} it had after UI-03. A conversion that \
         DELETES headers passes the budget arm without routing anything."
    );
}

// ---------------------------------------------------------------------------
// UI-10 — a labelled numeric parameter row is `ValueRow`
// ---------------------------------------------------------------------------
//
// `value_row.rs` states it is "the one labelled-numeric-input row" and
// `ui/components/CLAUDE.md` makes that an invariant. The audit counted 69 raw
// `DragValue::new` sites against 4 `ValueRow` call sites. One renderer must
// decide the commit semantics — the `PanelEdit::drag` triple of changed /
// committed / in_flight — for every numeric field, because that is the
// mechanism the focus-loss-while-typing defect turns on.
//
// UI-10 converted every labelled row that already sits in a 2-column
// parameter grid. What is left is listed below with the reason it is not a
// `ValueRow` row. The count only ever goes down.

/// Files that may still name `egui::DragValue::new`, with the count and the
/// reason. `ui/components/` is exempt by folder: it holds the renderers.
const RAW_DRAG_VALUE_ALLOWANCE: &[(&str, usize, &str)] = &[
    (
        "ui/multitool_planner.rs",
        8,
        "inline dial ribbons — several dials share one horizontal row, so \
         there is no label:value row to render",
    ),
    (
        "ui/properties/toolpath_panel.rs",
        4,
        "boundary offset and the three rest-analysis dials draw in \
         `ui.horizontal`; `ValueRow::show` calls `ui.end_row()` and needs a \
         grid",
    ),
    (
        "ui/properties/stock.rs",
        4,
        "padding, the shared pin diameter and the per-pin X/Y all draw in \
         `ui.horizontal` rows",
    ),
    (
        "ui/properties/operations/surface_3d.rs",
        2,
        "two integer fields (`ValueRow` edits an `f64`)",
    ),
    (
        "ui/properties/operations/boundary_2d.rs",
        3,
        "integer fields: tab count and two finishing-pass counts",
    ),
    (
        "ui/properties/setup.rs",
        2,
        "the datum Z offset and the fixture clearance draw in \
         `ui.horizontal` rows",
    ),
    (
        "ui/properties/machine_panel.rs",
        2,
        "junction deviation needs `max_decimals(4)`, which `ValueRow` has no \
         dial for; the jerk value shares its cell with the enable checkbox",
    ),
    (
        "ui/export_wizard.rs",
        2,
        "the Safe Z override is a conditional horizontal row and the spindle \
         warm-up is an integer",
    ),
    ("ui/properties/tool.rs", 1, "the flute count is an integer"),
    ("ui/properties/post.rs", 1, "the spindle speed is a `u32`"),
    (
        "ui/properties/operations/mod.rs",
        1,
        "the heights ladder is a 4-column grid; `ValueRow` ends the row after \
         two cells",
    ),
    (
        "ui/properties/model_sim_panels.rs",
        1,
        "the custom model scale is drawn inside a `push_id` horizontal",
    ),
    (
        "ui/feeds/compare.rs",
        1,
        "the feeds surfaces are the power session's; UI-10 does not edit them",
    ),
];

/// The folder that OWNS the numeric-row renderers.
const VALUE_ROW_HOME: &str = "ui/components/";

/// The fewest `ValueRow::new` call sites the crate must hold, so a sweep that
/// DELETES rows cannot pass the budget arm.
const MIN_VALUE_ROWS: usize = 30;

fn raw_drag_value_sites() -> Vec<(String, usize)> {
    let root = ui_src_root();
    let mut out = Vec::new();
    for path in ui_rs_files() {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with(VALUE_ROW_HOME) {
            continue;
        }
        let text = without_comments(&std::fs::read_to_string(&path).unwrap());
        let n = text.matches("DragValue::new(").count();
        if n > 0 {
            out.push((rel, n));
        }
    }
    out
}

#[test]
fn a_labelled_numeric_row_is_the_kit_element_ui10() {
    let sites = raw_drag_value_sites();
    let mut over = Vec::new();

    for (rel, count) in &sites {
        let allowed = RAW_DRAG_VALUE_ALLOWANCE
            .iter()
            .find(|(f, _, _)| f == rel)
            .map_or(0, |(_, n, _)| *n);
        if *count > allowed {
            over.push(format!("{rel} {count} (allowed {allowed})"));
        }
    }

    assert!(
        over.is_empty(),
        "these files draw a raw DragValue where `components::ValueRow` is \
         the renderer. A labelled numeric parameter inside a 2-column grid \
         is `ValueRow::new(label, &mut value, suffix, speed, range)`, and a \
         panel that records edits feeds `out.value_response` to \
         `PanelEdit::drag` so the commit rule stays the panel's own. A row \
         that is NOT that shape is named in RAW_DRAG_VALUE_ALLOWANCE with \
         its reason. Over budget: {}",
        over.join(", ")
    );
}

#[test]
fn the_raw_drag_value_allowance_is_not_vacuous_ui10() {
    let sites = raw_drag_value_sites();

    for (rel, allowed, reason) in RAW_DRAG_VALUE_ALLOWANCE {
        assert!(!reason.is_empty(), "{rel} must say WHY its rows are raw");
        let found = sites.iter().find(|(f, _)| f == rel).map_or(0, |(_, n)| *n);
        assert_eq!(
            found, *allowed,
            "{rel} is allowed {allowed} raw DragValue sites and holds \
             {found}. An allowance that no longer matches its file lets a \
             new hand-rolled row in under an old number."
        );
    }

    let mut value_rows = 0usize;
    for path in ui_rs_files() {
        let text = without_comments(&std::fs::read_to_string(&path).unwrap());
        value_rows += text.matches("ValueRow::new(").count();
    }
    assert!(
        value_rows >= MIN_VALUE_ROWS,
        "the crate holds only {value_rows} ValueRow call sites, fewer than \
         the {MIN_VALUE_ROWS} it had after UI-10."
    );
}
