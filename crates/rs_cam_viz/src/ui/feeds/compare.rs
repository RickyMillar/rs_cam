//! Compare and apply — the per-operation feeds job.
//!
//! This file answers "what is this operation set to, what does the
//! recommendation say, and do I take it?". W3i (2026-10-30) put the card
//! back on the Feeds tab, under the inline chart ([`draw_recommendation`]):
//! the two apply buttons and the problem rows on the face, every other row
//! in ONE closed `Details`. The Explore window is deleted.
//!
//! **Apply contract (Checkpoint I, 2026-08-12).** Both writes — `⚡ Apply
//! all` and `⚡ Speeds only` — go through `feeds::suggest::apply`,
//! the same funnel the properties panel uses, and both are replaced by the
//! refusal text when `validate_tool_for_operation` declines the tool ×
//! operation pairing (I-3).

use rs_cam_core::feeds::efficiency::{ChipVerdict, CutEfficiency, cut_efficiency};
use rs_cam_core::feeds::suggest::FeedsPreview;
use rs_cam_core::feeds::{FeedsExplain, FeedsField, PowerFigure, PowerUnmodeled};
use rs_cam_core::material::force_line::{ForceAtPoint, ForceLine, ForceLineRefusal};
use rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM;
use rs_cam_core::tool_load::verdict::BoundSource;

use rs_cam_core::feeds::rationale::SuggestRationale;
use rs_cam_core::feeds::suggest::SuggestWarning;

use super::shared::OpFeeds;
use super::shared::{
    AppliedRecipe, CurrentValues, ball_tip_radius, combined_scale, hardness_label,
    material_family_label, tool_family_label,
};
use super::why;
use crate::state::AppState;
use crate::ui::components::compare;
use crate::ui::components::role_of_tone;
use crate::ui::components::{DataRow, DataTable, Details, ProvKind, Role, SectionHeader, Source};
use crate::ui::sim_diagnostics::{limit_data_row_with_face, row_hover};
use crate::ui::{AppEvent, theme, tokens};
use rs_cam_core::limits::CheckRow;

const ADVANCE_ROW_LABEL: &str = "Advance/tooth";

/// The face of the full apply. A-3: the face says what the write changes.
const APPLY_ALL_FACE: &str = "⚡ Apply all — changes the cut";

/// The face of the speeds-only apply. It says what the write changes AND
/// what it holds, so the two buttons read as one pair. IC1 shortened it
/// from "Match vendor chipload — changes the speeds, not the cut", which
/// wrapped to two rows in the rail.
const APPLY_SPEEDS_FACE: &str = "⚡ Speeds only — holds the cut";

pub(crate) fn read_current_values(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Option<CurrentValues> {
    let tc = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)?;
    let tool = state
        .session()
        .tools()
        .iter()
        .find(|t| Some(t.id) == tc.tool_id)?;
    Some(current_values_for_operation(&tc.operation, tool))
}

/// Build the modal's payload.
///
/// Returns a [`FeedsPreview`], not a bare [`FeedsExplain`]: the explain payload
/// is infallible by design (you want the nomogram even for a pairing you would
/// decline to run) and therefore cannot tell this surface that the pairing was
/// refused — which is exactly how the modal came to offer eleven writes on
/// operations the engine had already declared unrunnable (A-3 census §3.1).
/// The preview carries the refusal alongside the numbers, and hands out
/// something writable only when there isn't one.
pub(crate) fn current_values_for_operation(
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
) -> CurrentValues {
    // S11: an op with no spindle speed has none; the post speed is no
    // fallback (W3i dropped the last one here).
    CurrentValues {
        feed_rate_mm_min: operation.feed_rate(),
        plunge_rate_mm_min: operation.plunge_rate(),
        spindle_rpm: operation.spindle_rpm(),
        depth_per_pass: operation.depth_per_pass(),
        stepover: operation.stepover(),
        flute_count: tool.flute_count_planning(),
        scallop_height: operation.scallop_height(),
        // P29-11 L-1 (S8): a registry capability, not a test of the op type.
        supports_scallop_override: operation.capabilities().supports_scallop_override,
    }
}

pub(crate) fn compute_preview(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Option<FeedsPreview> {
    let tc = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)?;
    let tool = state
        .session()
        .tools()
        .iter()
        .find(|t| Some(t.id) == tc.tool_id)?;
    let stock = state.session().stock_config();
    Some(rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session().machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        state.session().post_config().spindle_policy(),
    ))
}

/// The card's funnel answers for `FeedsMemo::card` (W3i): the Suggest
/// warnings of the invariant pass and, on a pairing that is not refused,
/// the funnel dry run of `⚡ Apply all` (G-RECOMAPPLIED: the recommended
/// column prints what the apply writes).
pub(crate) fn build_card(
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    // F114, P29-9: THE Suggest context of the panel: the session door's
    // slots and the toolpath's dressups (the G6 entry θ too).
    context: rs_cam_core::feeds::suggest::SuggestContext<'_>,
    preview: &FeedsPreview,
) -> crate::state::runtime::FeedsCard {
    let context = || context;
    // `FeedsPreview` carries calculator warnings; the rows also quote the
    // invariant pass's own account of what it did (ruling R4, 2026-09-24).
    let suggest_warnings = rs_cam_core::feeds::suggest::suggest_for_operation(
        rs_cam_core::feeds::suggest::SuggestForOperationInput {
            operation,
            tool,
            machine,
            material,
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy,
            context: context(),
        },
    )
    .ok()
    .map(|suggested| suggested.warnings);
    // A refused pairing has no apply, so the column keeps the calculator
    // values there.
    let previews = preview.refusal().is_none().then(|| {
        rs_cam_core::feeds::suggest::preview_field_applies(
            operation,
            tool,
            machine,
            material,
            rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy,
            context(),
        )
    });
    crate::state::runtime::FeedsCard {
        suggest_warnings,
        previews,
    }
}

/// W3i (2026-10-30): the recommendation of one toolpath, under the inline
/// chart of the Feeds tab: `⚡ Apply all` and `⚡ Speeds only` (small,
/// at the chart's left edge), the problem rows, then ONE closed `Details`
/// with the spindle policy (`policy`), the rows that differ, the scallop
/// control and the derivation.
///
/// It reads the panel's own copy of the operation, the tool, the stock
/// material and the machine, and the memoised preview and funnel answers
/// ([`OpFeeds`]); it runs no calculator and no funnel itself.
pub(crate) fn draw_recommendation(
    ui: &mut egui::Ui,
    op: &OpFeeds<'_>,
    current: &CurrentValues,
    limits: Option<&CardLimits<'_>>,
    policy: impl FnOnce(&mut egui::Ui, &mut Vec<AppEvent>),
    events: &mut Vec<AppEvent>,
) {
    let OpFeeds {
        toolpath_id,
        operation,
        tool,
        material,
        machine,
        feed_max_rate,
        preview,
        card,
    } = *op;
    let suggest_warnings = card.suggest_warnings.clone();
    let rationale = suggest_warnings
        .as_deref()
        .map(SuggestRationale::from_warnings);
    let previews = card.previews.clone();
    let applied = AppliedRecipe::new(previews);
    draw_inspector_comparison(
        ui,
        current,
        preview,
        &applied,
        rationale.as_ref(),
        suggest_warnings.as_deref().unwrap_or_default(),
        operation,
        tool,
        material,
        machine,
        feed_max_rate,
        toolpath_id,
        limits,
        policy,
        events,
    );
}

/// **The limit rows of the Suggest card** (P29-10 10f, design 3.6, rulings
/// O1, O2 and O6, lead rulings on 10f): the rows of
/// `ProjectSession::limit_rows` that the panel memo holds. `envelope` is
/// `SafeEnvelope::rows` of the memo envelope (the planned rows at the
/// proposal, then the stated rows); `rpm` is the RPM part
/// (`FeedsMemo::rpm_rows`). Core wrote every face, detail, tone and the
/// rank; this only sorts the rows into the two parts of the drawer, groups
/// them by cut and picks the face row by the core rank.
pub(crate) struct CardLimits<'a> {
    /// `SafeEnvelope::rows`.
    pub(crate) envelope: &'a [CheckRow],
    /// The rows of the Suggest RPM choice.
    pub(crate) rpm: &'a [CheckRow],
    /// The proposal that ⚡ writes: (depth, width) in mm. The depth is
    /// `None` when the envelope proposes no depth.
    pub(crate) proposal: (Option<f64>, f64),
}

/// The planned rows of one declared cut, in the core order.
struct CutRows<'a> {
    cut: rs_cam_core::limits::CutKind,
    rows: Vec<&'a CheckRow>,
}

impl CardLimits<'_> {
    /// Part one, "As the op stands": the rows of the cut that the op holds
    /// now (`CheckRow::is_stated`). P29-10 10i (lead ruling L1): every
    /// declared cut at the stated point, then the stated point against each
    /// limit that is not a limit of each cut. The face row reads only this
    /// part.
    fn as_it_stands(&self) -> Vec<&CheckRow> {
        self.envelope.iter().filter(|row| row.is_stated()).collect()
    }

    /// Part two, "If you apply ⚡": the planned rows at the proposal,
    /// grouped by cut in the declaration order (core writes them by limit,
    /// then by cut; the first row of a cut fixes its place).
    fn cuts(&self) -> Vec<CutRows<'_>> {
        self.cuts_where(CheckRow::is_planned)
    }

    /// P29-10 10i: part one by cut, with the folds of part two.
    fn stated_cuts(&self) -> Vec<CutRows<'_>> {
        self.cuts_where(|row| row.is_stated())
    }

    /// The rows of `keep` that name a declared cut, grouped by cut.
    fn cuts_where(&self, keep: impl Fn(&CheckRow) -> bool) -> Vec<CutRows<'_>> {
        let mut out: Vec<CutRows<'_>> = Vec::new();
        for row in self.envelope.iter().filter(|row| keep(row)) {
            let Some(cut) = row.cut() else { continue };
            match out.iter_mut().find(|group| group.cut == cut) {
                Some(group) => group.rows.push(row),
                None => out.push(CutRows {
                    cut,
                    rows: vec![row],
                }),
            }
        }
        out
    }

    /// Every row of part two: the planned rows, then the RPM rows.
    fn if_applied(&self) -> impl Iterator<Item = &CheckRow> {
        self.envelope
            .iter()
            .filter(|row| row.is_planned())
            .chain(self.rpm.iter())
    }

    /// THE face row (ruling O1, lead ruling 1 on 10f): the worst row of
    /// part one by the one core rank (`CheckRow::rank_cmp`), only when it
    /// exceeds. A proposal row never reaches the face: the face describes
    /// the op as it stands. P29-10z: every stated row ranks.
    pub(crate) fn face_row(&self) -> Option<&CheckRow> {
        self.as_it_stands()
            .into_iter()
            .filter(|row| row.tone().exceeds())
            .min_by(|a, b| a.rank_cmp(b))
    }

    /// The stated row of the limit that places the cut of today outside
    /// the envelope (`Check::binding`, the limit that `OutsideEnvelope`
    /// names), else the worst stated row that exceeds by the core rank.
    /// The `OutsideEnvelope` stage line reads it.
    pub(crate) fn stated_outside(&self) -> Option<&CheckRow> {
        let outside: Vec<&CheckRow> = self
            .as_it_stands()
            .into_iter()
            .filter(|row| row.tone().exceeds())
            .collect();
        outside
            .iter()
            .copied()
            .find(|row| row.check().binding)
            .or_else(|| outside.iter().copied().min_by(|a, b| a.rank_cmp(b)))
    }

    /// The number of rows and the number that exceed, for the drawer label.
    fn counts(&self) -> (usize, usize) {
        self.as_it_stands()
            .into_iter()
            .chain(self.if_applied())
            .fold((0, 0), |(n, over), row| {
                (n + 1, over + usize::from(row.tone().exceeds()))
            })
    }

    /// The number of part-two rows that exceed, for its title.
    fn applied_over(&self) -> usize {
        self.if_applied().filter(|row| row.tone().exceeds()).count()
    }

    /// True when the drawer has no row.
    pub(crate) fn is_empty(&self) -> bool {
        self.counts().0 == 0
    }
}

/// The face row of the card (ruling O1, O2): one full-width status line,
/// the role glyph and the core face in the colour of the core tone
/// (`role_of_tone`, the one map), the core detail on hover. It takes no
/// status chip, so a face of 60 characters keeps its caveat word.
fn face_limit_row(row: &CheckRow) -> DataRow {
    let role = role_of_tone(row.tone());
    let text = match role.glyph() {
        Some(glyph) => format!("{glyph} {}", row.face()),
        None => row.face().to_owned(),
    };
    DataRow::note(role, text).hover(row_hover(row))
}

/// The label of the limits drawer.
pub(crate) const LIMITS_DETAILS: &str = "Limits";

/// The title of the first part of the drawer: the cut that the op holds.
pub(crate) const AS_IT_STANDS_TITLE: &str = "As the op stands";

/// The label of the closed line that folds the rows with no number.
pub(crate) const NO_NUMBER_LABEL: &str = "no number";

/// One row under its title: the core face without its subject
/// (`CheckRow::face_in_group`), drawn as the one limit row of the viz.
fn group_row(row: &CheckRow) -> DataRow {
    limit_data_row_with_face(row, row.face_in_group(), None)
}

/// The rows of one group: the rows that hold a number, then ONE closed
/// line "no number · n" that opens to the rows with no number. Nothing is
/// dropped (lead ruling 2 on 10f).
fn draw_group_rows(ui: &mut egui::Ui, id: &str, rows: &[&CheckRow]) {
    let (numbers, none): (Vec<&CheckRow>, Vec<&CheckRow>) =
        rows.iter().copied().partition(|row| row.holds_a_number());
    DataTable::new((id, "numbers"))
        .rows(numbers.iter().map(|row| group_row(row)))
        .show(ui);
    if !none.is_empty() {
        let _ = Details::new((id, "no_number"))
            .label(NO_NUMBER_LABEL)
            .count(none.len())
            .show(ui, |ui| {
                DataTable::new((id, "no_number_rows"))
                    .rows(none.iter().map(|row| group_row(row)))
                    .show(ui);
            });
    }
}

/// The caveat words of a cut (`CheckRow::cut_words`: "worst case", "may
/// not occur"), once for the cut: they go in its title, not on each row.
fn cut_words(rows: &[&CheckRow]) -> Vec<&'static str> {
    let mut words: Vec<&'static str> = Vec::new();
    for word in rows.iter().flat_map(|row| row.cut_words()) {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

/// One declared cut of part two. P29-10 10g (item 4c): a cut where no row
/// exceeds is ONE closed line ("planner cap · 6 within · 1 no number", the
/// core words of `limits::quiet_summary`) that opens to its rows; a cut
/// with a row that exceeds stays open: a title (the cut and its caveat
/// words), the rows that hold a number and one closed "no number · n" line.
fn draw_cut(ui: &mut egui::Ui, part: &str, group: &CutRows<'_>) {
    let mut title = group.cut.label().to_owned();
    for word in cut_words(&group.rows) {
        title.push_str(&format!(" \u{00B7} {word}"));
    }
    let id = format!("feeds_card_limits_{part}_{title}");
    let quiet = rs_cam_core::limits::quiet_summary(&group.rows);
    if let Some(summary) = quiet {
        let tail = cut_words(&group.rows)
            .into_iter()
            .map(str::to_owned)
            .chain(std::iter::once(summary))
            .collect::<Vec<_>>()
            .join(" \u{00B7} ");
        let _ = Details::new((&id, "quiet"))
            .label(group.cut.label())
            .tail(Some(tail))
            .show(ui, |ui| {
                draw_group_rows(ui, &id, &group.rows);
            });
    } else {
        DataTable::new((&id, "title"))
            .row(DataRow::record(title).tone(tokens::TEXT_BODY))
            .show(ui);
        draw_group_rows(ui, &id, &group.rows);
    }
}

/// **The "Limits · n" drawer of the Suggest card** (P29-10 10f, ruling O6,
/// lead ruling 2 on 10f): closed by default, every row in two parts. Part
/// one, "As the op stands": the stated rows. Part two, "If you apply ⚡ · k
/// over" and the line "proposal ap × ae mm": the planned rows grouped by cut (the cut
/// name and its caveat words as a title), then the RPM rows. Each row is
/// the one limit row of the viz (`limit_data_row_with_face`, the core
/// tone through `role_of_tone`, the core detail on hover) with the core
/// face without its subject. In each group the rows with no number fold
/// into ONE closed line. `beside` draws at the right end of the header row
/// (`Vendor data…`), so the drawer adds no line. `open` is
/// `PanelState::card_limits_open` (`set_ui_view card_limits`).
pub(crate) fn draw_limits_drawer<B>(
    ui: &mut egui::Ui,
    limits: &CardLimits<'_>,
    open: &mut bool,
    beside: impl FnOnce(&mut egui::Ui) -> B,
) -> B {
    let (n, over) = limits.counts();
    let tail = (over > 0).then(|| format!("{over} over"));
    let (beside, _) = Details::new("feeds_card_limits")
        .label(LIMITS_DETAILS)
        .count(n)
        .tail(tail)
        .show_held_beside(ui, open, beside, |ui| {
            let stands = limits.as_it_stands();
            if !stands.is_empty() {
                SectionHeader::new(AS_IT_STANDS_TITLE).small_caps().show(ui);
                // P29-10 10i: the stated point against the limits that are
                // not limits of each cut, then each declared cut at the
                // stated point with the folds of part two.
                let point_rows: Vec<&CheckRow> = stands
                    .into_iter()
                    .filter(|row| row.cut().is_none())
                    .collect();
                // P29-10z (lead ruling T): the stated-point rows take a
                // title in the style of a cut title; the words are core's.
                if !point_rows.is_empty() {
                    DataTable::new("feeds_card_limits_stands_title")
                        .row(
                            DataRow::record(rs_cam_core::limits::STATED_POINT_TITLE)
                                .tone(tokens::TEXT_BODY),
                        )
                        .show(ui);
                }
                draw_group_rows(ui, "feeds_card_limits_stands", &point_rows);
                for group in &limits.stated_cuts() {
                    draw_cut(ui, "stands", group);
                }
            }
            let cuts = limits.cuts();
            if !cuts.is_empty() || !limits.rpm.is_empty() {
                SectionHeader::new(if_applied_title(limits.applied_over()))
                    .small_caps()
                    .show(ui);
                DataTable::new("feeds_card_limits_proposal")
                    .row(DataRow::record(proposal_line(limits.proposal)))
                    .show(ui);
                for group in &cuts {
                    draw_cut(ui, "applied", group);
                }
                if !limits.rpm.is_empty() {
                    let rpm: Vec<&CheckRow> = limits.rpm.iter().collect();
                    DataTable::new("feeds_card_limits_rpm_title")
                        .row(DataRow::record(RPM_TITLE).tone(tokens::TEXT_BODY))
                        .show(ui);
                    draw_group_rows(ui, "feeds_card_limits_rpm", &rpm);
                }
            }
        });
    beside
}

/// The title of the RPM rows in part two.
const RPM_TITLE: &str = "Spindle speed";

/// The title of the second part and the count of its rows that exceed.
/// The small-caps title holds no number of the cut, so it fits the rail;
/// the proposal is the line under it ([`proposal_line`]).
fn if_applied_title(over: usize) -> String {
    if over > 0 {
        format!("If you apply \u{26A1} \u{00B7} {over} over")
    } else {
        "If you apply \u{26A1}".to_owned()
    }
}

/// The line under the part-two title: the proposal that ⚡ writes. It says
/// "proposal", so a reader does not take a planned row for the cut of
/// today (design 3.6).
fn proposal_line((ap, ae): (Option<f64>, f64)) -> String {
    match ap {
        Some(ap) => format!("proposal {ap:.2} \u{00D7} {ae:.2} mm"),
        None => format!("proposal width {ae:.2} mm"),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the card reads the operation, the tool, the material and the machine on top of the recommendation, because the chipload verdict is a statement about the cut, not about the recipe. `draw_feeds_card` carries the same allow for the same reason: bundling what the session already holds into a struct would build a second data model of it."
)]
pub(crate) fn draw_inspector_comparison(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    preview: &FeedsPreview,
    applied: &AppliedRecipe,
    rationale: Option<&SuggestRationale>,
    suggest_warnings: &[SuggestWarning],
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    feed_max_rate: Option<f64>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    limits: Option<&CardLimits<'_>>,
    policy: impl FnOnce(&mut egui::Ui, &mut Vec<AppEvent>),
    events: &mut Vec<AppEvent>,
) {
    let explain = preview.explain();
    // Core computes the verdict; this file renders it. `rs_cam_viz`'s rule:
    // do not recompute a narrower answer in the UI.
    //
    // G-RECOMAPPLIED: the verdict and the power row describe the cut that
    // `⚡ Apply all` writes, the same cut the rows print. Until 2026-09-24
    // the verdict read the calculator's feed, RPM, depth and width (and the
    // operation's current values for the force headroom), and the power row
    // read the operation's current values. The calculator's cut is evaluated
    // too, so each hover can quote it.
    let (applied_op, applied_point) = applied.applied_cut(operation, &explain.recommended);
    let calculator_op = applied.calculator_cut(operation, &explain.recommended);
    let efficiency = cut_efficiency(&applied_op, tool, material, machine, &applied_point);
    let calculator_efficiency = cut_efficiency(
        &calculator_op,
        tool,
        material,
        machine,
        &explain.recommended,
    );
    let efficiency_note = headroom_note(
        efficiency.as_ref(),
        calculator_efficiency.as_ref(),
        &applied.cut_cause(),
    );
    let mut power = PowerReading::at_shipped_point(
        &applied_op,
        tool,
        material,
        machine,
        &applied_point,
        feed_max_rate,
    );
    let calculator_power = PowerReading::at_shipped_point(
        &calculator_op,
        tool,
        material,
        machine,
        &explain.recommended,
        feed_max_rate,
    );
    power.calculator_note = power_note(&power, &calculator_power, &applied.cut_cause());

    // W2: every row explains ITS OWN number. The `Why is the recommendation
    // here?` disclosure that used to sit under this card explained the recipe
    // as a whole, which is not the question an operator asks while reading a
    // row that tripled.
    let why = |row: why::RecipeRow| why::row_explanation(row, current, explain, applied, rationale);
    // G-RECOMAPPLIED: the recommended column is what `⚡ Apply all` writes,
    // read off the funnel's dry run. The raw calculator value is on each
    // row's hover.
    let r = &explain.recommended;
    let rpm = applied.value(FeedsField::SpindleRpm, r.rpm);
    let feed = applied.value(FeedsField::FeedRate, r.feed_rate_mm_min);
    let plunge = applied.value(FeedsField::PlungeRate, r.plunge_rate_mm_min);
    let doc = applied.value(FeedsField::DepthPerPass, r.axial_depth_mm);
    let woc = applied.value(FeedsField::Stepover, r.radial_width_mm);
    // F006: the typed advance; core holds the one divisor.
    let advance = rs_cam_core::feeds::AdvancePerToothMm::from_commanded_at(
        rs_cam_core::feeds::CommandedFeedMmMin::new(feed),
        rpm,
        current.flute_count,
    )
    .map(rs_cam_core::feeds::AdvancePerToothMm::mm);
    // G-FEEDSLABEL (UX-R03-005), G-RECOMAPPLIED (2026-09-24): the
    // recommended DOC / WOC are the values `⚡ Apply all` writes (the
    // funnel's dry run, `preview_field_applies`). Until 2026-09-24 the
    // column printed the raw calculator values. On a Ø6 hardwood pocket it
    // recommended 4.20 mm of DOC where the apply wrote 0.81 mm. Each row's
    // hover keeps the calculator value. The advance row prints
    // feed ÷ (RPM × flutes) in both columns.
    let recipe = [
        change_row(
            "RPM",
            current.spindle_rpm.map(f64::from),
            Some(rpm),
            "",
            1.0,
            why(why::RecipeRow::Rpm),
        ),
        change_row(
            "Feed",
            current.feed_rate_mm_min,
            Some(feed),
            " mm/min",
            1.0,
            why(why::RecipeRow::Feed),
        ),
        change_row(
            "Plunge",
            current.plunge_rate_mm_min,
            Some(plunge),
            " mm/min",
            1.0,
            why(why::RecipeRow::Plunge),
        ),
        change_row(
            "DOC",
            current.depth_per_pass,
            Some(doc),
            " mm",
            0.01,
            why(why::RecipeRow::Doc),
        ),
        woc_row(current, explain, woc, why(why::RecipeRow::Woc)),
        change_row(
            ADVANCE_ROW_LABEL,
            Some(current.chipload_mm()),
            advance,
            " mm/tooth",
            0.0001,
            why(why::RecipeRow::Advance),
        ),
    ];

    // Round 2 (2026-10-13), W3i (2026-10-30). The face shows the two apply
    // buttons and one status line for each problem. The chart above already
    // shows the gap between the operation and the recommendation, so the
    // rows that differ go in the ONE `Details` with the derivation: the rows
    // at the recommendation, the verdicts that are not a problem, the
    // context, the row basis, the force line and the Suggest stages.
    let mut face = DataTable::new("feeds_recommendation_problems");
    let mut changes = DataTable::new("feeds_recommendation").name_share(CHANGE_NAME_SHARE);
    let mut level = DataTable::new("feeds_recommendation_level").name_share(CHANGE_NAME_SHARE);
    let mut details = DataTable::new("feeds_derivation");
    let mut any_change = false;
    for (row, differs) in recipe {
        if differs {
            any_change = true;
            changes = changes.row(row);
        } else {
            level = level.row(row);
        }
    }
    let (chip, chip_problem) = chip_row(efficiency.as_ref(), efficiency_note.as_deref(), advance);
    if chip_problem {
        face = face.row(chip);
    } else {
        details = details.row(chip);
    }
    // P29-10 10f (ruling O1, F1): the power row is derivation. A power
    // that exceeds reaches the face through its limit row; no viz share
    // makes a problem row.
    details = details.row(power_row(&power));
    // MRR = DOC × WOC × feed, the calculator's own product (Step 5), on the
    // values Apply writes.
    details = details.row(DataRow::measured(
        "MRR",
        format!("{:.0}", doc * woc * feed),
        "mm\u{00B3}/min",
    ));
    details = details.rows(context_rows(explain, tool));
    details = details.rows(force_line_rows(&power));
    details = details.rows(why::row_basis_rows(explain));
    for placement in why::engaged_diameter_row(current, explain)
        .into_iter()
        .chain(why::warning_rows(explain))
        .chain(why::suggest_rows(
            suggest_warnings,
            limits.and_then(CardLimits::stated_outside),
        ))
    {
        match placement {
            why::Placement::Problem(row) => face = face.row(row),
            why::Placement::Detail(row) => details = details.row(row),
        }
    }
    // P29-10 10f (ruling O1): at most ONE limit row on the face, the worst
    // row by the core rank, and only when it exceeds. It replaces the
    // finish-only chip floor row (F5: a role read on a display surface);
    // the planned chip rows judge every role.
    if let Some(row) = limits.and_then(CardLimits::face_row) {
        face = face.row(face_limit_row(row));
    }

    // W3k (operator, 2026-10-31): the two apply buttons are inside the
    // `Recommended` disclosure, beside the rows they write. A refusal that
    // blocks the apply stays on the face.
    draw_apply_refusal(ui, preview.refusal());
    if !face.is_empty() {
        face.show(ui);
    }
    let count = 1 + changes.len() + level.len() + details.len();
    let can_apply = preview.refusal().is_none() && any_change;
    Details::new("feeds_details")
        .label(RECOMMENDED_DETAILS)
        .count(count)
        .show(ui, |ui| {
            if can_apply {
                draw_apply_buttons(ui, toolpath_id, events);
            }
            policy(ui, events);
            if any_change {
                changes.show(ui);
            } else {
                DataTable::new("feeds_recommendation_ok")
                    .row(DataRow::note(
                        Role::Ok,
                        format!("{} at the recommendation", crate::ui::tokens::GLYPH_OK),
                    ))
                    .show(ui);
            }
            // S1 — scallop-driven stepover control (DropCutter only). It sets
            // a value, so it comes before the derivation rows.
            if current.supports_scallop_override {
                draw_scallop_control(ui, current, explain, toolpath_id, events);
            }
            level.show(ui);
            details.show(ui);
        });
}

/// The name-column share of a table of change rows: the two values and the
/// ratio take the rest.
const CHANGE_NAME_SHARE: f32 = 0.25;

/// Tool, material and recommendation-source context: three rows of the
/// card's `Details`.
///
/// The size reads `ToolConfig::size_label`, so a tapered ball says
/// "tip Ø2.00 mm (R1.00)" and never a bare "2.00 mm". The source is a
/// badge. A row that is far from the query but has no G1 size claim and no
/// G2 cap (a V-bit, or a hardness transfer under the cap) keeps its combined
/// scale as a caution status; a claim states its own scale in its basis row
/// (`why::row_basis_rows`).
fn context_rows(explain: &FeedsExplain, tool: &crate::state::job::ToolConfig) -> Vec<DataRow> {
    let tool_text = format!(
        "{} \u{00B7} {} flute \u{00B7} {}",
        tool.size_label(),
        explain.flute_count,
        tool_family_label(explain.query.tool_family),
    );
    let material_text = format!(
        "{} ({})",
        material_family_label(explain.query.material_family),
        hardness_label(explain.query_hardness_kind, explain.query_hardness_value),
    );
    let source = match &explain.matched_row {
        Some(row) => {
            let base = DataRow::value("Vendor row", row.observation_id.clone()).source(
                Source::Prov(ProvKind::VendorLut, Some(row.observation_id.clone())),
            );
            if row.is_extrapolated
                && row.size_basis.claim().is_none()
                && row.hardness_basis.cap().is_none()
            {
                base.status(
                    Role::Caution,
                    format!("approx \u{00D7}{:.2}", combined_scale(row)),
                )
            } else {
                base
            }
        }
        None => DataRow::value("Vendor row", "none: the formula sets the values")
            .source(Source::Prov(ProvKind::Formula, None)),
    };
    vec![
        DataRow::value("Tool", tool_text),
        DataRow::value("Material", material_text),
        source,
    ]
}

/// One `label current → recommended Δ` row, and whether the recommendation
/// differs from the current value at the precision the row prints.
///
/// `explanation` is the row's own answer to "why is this number what it is",
/// and it is not optional. W2 deleted the `Why is the recommendation here?`
/// disclosure precisely because a whole-recipe explanation cannot answer a
/// per-row question; a row with nothing to say would put that hole back.
///
/// Round 2: a row with no recommendation, a row with no current value, or a
/// row whose recommendation prints the same as the current value, is not a
/// difference. The card puts it in `Details`.
fn change_row(
    label: &str,
    current: Option<f64>,
    recommended: Option<f64>,
    unit: &str,
    precision: f64,
    explanation: String,
) -> (DataRow, bool) {
    let current_text = compare::format_optional(current, unit, precision);
    let recommended_text = compare::format_optional(recommended, unit, precision);
    // An operation with no value for the field (a 3D finish has no depth per
    // pass) cannot hold the recommendation, so it is not a difference.
    let differs = recommended.is_some() && current.is_some() && current_text != recommended_text;
    (
        DataRow::change(
            label,
            current_text,
            recommended_text,
            compare::delta_tag(current, recommended),
        )
        .hover(explanation),
        differs,
    )
}

/// The rail's chipload verdict — where this cut sits in the corridor, and
/// what sitting there costs. One row, and a problem when the chip is thin
/// or heavy.
///
/// # This replaces a gauge that could not fail (Phase A)
///
/// The line here used to read `Power 0.01 of 0.60 kW (1 %)`. `feeds/mod.rs`
/// already carried the measurement that condemned it: across all three
/// shipped machine presets × ten species × Ø3/Ø6/Ø12 slots, the power branch
/// never fires at all, and **peak** utilisation is 23.6 %. Typical is 1 %: a
/// 6 mm cutter in softwood needs about seven watts. A figure whose maximum
/// observed value across the entire shipped matrix sits in its left quarter
/// cannot distinguish a safe cut from a safer one.
///
/// The chipload can. Below the vendor band there is no trade-off, only loss:
/// at the fixture's 0.038 mm/tooth the cut spends 1.6× the energy per mm³
/// AND 1.8× the time of the band midpoint. That is a state an operator can
/// act on, and this row changes it on every job that matters.
///
/// # Every number here can be absent, and absence is said out loud
///
/// `cut_efficiency` refuses as a whole when the material has no
/// force line (ruling B6), and each ratio refuses on its own when its input is
/// missing. A `None` renders as a stated abstention — never as a zero, a
/// blank, a bare dash or a 100 %. The ratios go on the face because "1.6×
/// tool wear" is actionable; `u` in J/mm³ and the ploughing share go on the
/// hover, where the person who wants the workings will look.
fn chip_row(
    efficiency: Option<&CutEfficiency>,
    calculator_note: Option<&str>,
    fallback_advance_mm: Option<f64>,
) -> (DataRow, bool) {
    // The advance per tooth is feed ÷ (RPM × flutes) — a commanded value,
    // not a force-model output — so it survives the model's refusal and is
    // still printed beside the abstention.
    let advance_mm = efficiency
        .map(|e| e.advance_per_tooth_mm)
        .or(fallback_advance_mm);
    let (role, verdict) = match efficiency {
        Some(e) => verdict_face(e),
        None => (
            Role::Unknown,
            "efficiency not modelled for this material".to_owned(),
        ),
    };
    let mut hover = match efficiency {
        Some(e) => efficiency_hover(e),
        None => unmodelled_hover(advance_mm),
    };
    if let Some(note) = calculator_note {
        hover.push_str("\n\n");
        hover.push_str(note);
    }
    // The status slot truncates in the rail; the hover ends with the whole
    // verdict.
    hover.push_str(&format!("\n\nVerdict: {verdict}."));
    let problem = matches!(role, Role::Caution | Role::Danger);
    (
        DataRow::value(
            "Chip",
            compare::format_optional(advance_mm, " mm/tooth", 0.0001),
        )
        .status(role, verdict)
        .hover(hover),
        problem,
    )
}

/// The verdict phrase and its role. The status slot adds the role glyph.
///
/// Each arm prints only the ratios that exist. A missing ratio becomes a
/// short abstention in the phrase, never a silent omission and never a zero.
fn verdict_face(efficiency: &CutEfficiency) -> (Role, String) {
    match efficiency.verdict {
        ChipVerdict::Thin => {
            let mut costs = Vec::new();
            if let Some(wear) = efficiency.wear_ratio_vs_band_mid {
                costs.push(format!("{wear:.1}\u{00D7} tool wear"));
            }
            if let Some(time) = efficiency.time_ratio_vs_band_mid {
                costs.push(format!("{time:.1}\u{00D7} the time"));
            }
            let tail = if costs.is_empty() {
                "below the vendor range; the cost of that is not modelled".to_owned()
            } else {
                costs.join(", ")
            };
            (Role::Caution, format!("thin \u{2014} {tail}"))
        }
        ChipVerdict::InBand => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => headroom_phrase(headroom),
                None => "force headroom not modelled".to_owned(),
            };
            (Role::Ok, format!("in vendor range \u{00B7} {tail}"))
        }
        ChipVerdict::Heavy => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => headroom_phrase(headroom),
                None => "above the vendor range; force headroom not modelled".to_owned(),
            };
            (Role::Caution, format!("heavy \u{2014} {tail}"))
        }
        ChipVerdict::NoBand => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => format!(" \u{00B7} {}", headroom_phrase(headroom)),
                None => String::new(),
            };
            (
                Role::Unknown,
                format!("no vendor chipload range matched{tail}"),
            )
        }
    }
}

/// The verdict hover's sentence on the calculator's cut (G-RECOMAPPLIED).
///
/// `None` when the two cuts give the same phrase, or when either headroom is
/// not modelled: the hover already states that refusal.
fn headroom_note(
    applied: Option<&CutEfficiency>,
    calculator: Option<&CutEfficiency>,
    cause: &str,
) -> Option<String> {
    let applied = headroom_phrase(applied?.force_headroom?);
    let calculator = headroom_phrase(calculator?.force_headroom?);
    (applied != calculator).then(|| {
        format!(
            "Force headroom: calculator {calculator}; {cause}, so the cut Apply \
             writes has {applied}."
        )
    })
}

/// The power hover's sentence on the calculator's cut (G-RECOMAPPLIED).
///
/// `None` when the two cuts print the same percent, or when either figure is
/// refused: the hover already states that refusal.
fn power_note(applied: &PowerReading, calculator: &PowerReading, cause: &str) -> Option<String> {
    let applied = power_face(applied.figure.as_ref().ok()?);
    let calculator = power_face(calculator.figure.as_ref().ok()?);
    (applied != calculator).then(|| {
        format!(
            "Power: calculator {calculator} of the limit; {cause}, so the cut \
             Apply writes draws {applied}."
        )
    })
}

/// Force headroom as a phrase.
///
/// Two roundings are deliberate. A **negative** headroom is a real finding —
/// the predicted deflection is already past the bound — so it is said, not
/// clamped to zero. And a headroom above 99.5 % prints as `>99 %` rather
/// than rounding up: wood routing leaves the deflection bound almost
/// untouched on most cuts, and a flat `100 %` on the face is the shape this
/// repository has drawn four times — an absent constraint painted as an
/// all-clear. `>99 %` cannot be mistaken for one.
fn headroom_phrase(headroom: f64) -> String {
    if headroom < 0.0 {
        format!("{:.0} % past the deflection bound", -headroom * 100.0)
    } else if headroom > 0.995 {
        ">99 % force headroom".to_owned()
    } else {
        format!("{:.0} % force headroom", headroom * 100.0)
    }
}

/// The row's workings: the unit figure, the ploughing share in plain words,
/// the three bounds, and where each came from.
///
/// Every line is present in every state. A bound the model declines says it
/// declined; it does not drop out of the list, because a reader cannot tell a
/// missing line from a bound that does not bite.
fn efficiency_hover(efficiency: &CutEfficiency) -> String {
    let mut out = format!(
        "Chipload {:.4} mm/tooth — the recommendation's commanded advance per \
         tooth, feed \u{00F7} (RPM \u{00D7} flutes).\n\n\
         Energy per mm\u{00B3}: u = {:.0} J/mm\u{00B3}. {:.0} % of the cutting \
         power is rubbing rather than shearing wood. That share is the burn \
         and wear risk, and it falls as the chip gets thicker.\n\n",
        efficiency.advance_per_tooth_mm,
        efficiency.specific_energy_j_per_mm3,
        efficiency.ploughing_share * 100.0,
    );
    // A range only when the row publishes two different limits
    // (G-CHARTLINES). A one-point row prints its one value.
    match efficiency.band {
        Some(band) if band.min_mm_per_tooth < band.max_mm_per_tooth => out.push_str(&format!(
            "Vendor range: {:.3}–{:.3} mm/tooth, from the matched vendor row.\n",
            band.min_mm_per_tooth, band.max_mm_per_tooth,
        )),
        Some(band) => out.push_str(&format!(
            "Vendor value: {:.3} mm/tooth. The matched row publishes one value, \
             not a range.\n",
            band.max_mm_per_tooth,
        )),
        None => out.push_str(
            "Vendor range: none published \u{2014} no row matched this tool \
             \u{00D7} material, or the row publishes one limit only, so there \
             is no wear or time ratio to quote.\n",
        ),
    }
    out.push_str(&format!(
        "Rubbing floor: {:.3} mm/tooth — below it the edge burnishes instead \
         of cutting.\n",
        efficiency.rubbing_floor_mm,
    ));
    match efficiency.deflection_ceiling_mm {
        Some(ceiling) => out.push_str(&format!(
            "Deflection ceiling: {ceiling:.3} mm/tooth — the heaviest chip \
             that keeps predicted tip deflection inside {EXCEEDS_BOUND_MM:.3} mm.\n",
        )),
        None => out.push_str(
            "Deflection ceiling: not modelled — this tool, depth or engagement \
             leaves the deflection model nothing to solve.\n",
        ),
    }
    match efficiency.force_headroom {
        Some(headroom) if headroom >= 0.0 => out.push_str(&format!(
            "Force headroom: {:.1} % of the {EXCEEDS_BOUND_MM:.3} mm deflection \
             bound is unused.\n",
            headroom * 100.0,
        )),
        Some(headroom) => out.push_str(&format!(
            "Force headroom: none — the predicted deflection is {:.1} % past \
             the {EXCEEDS_BOUND_MM:.3} mm bound.\n",
            -headroom * 100.0,
        )),
        None => out.push_str(
            "Force headroom: not modelled — the pre-simulation deflection \
             predictor declined this pairing.\n",
        ),
    }
    match (
        efficiency.wear_ratio_vs_band_mid,
        efficiency.time_ratio_vs_band_mid,
    ) {
        (Some(wear), Some(time)) => out.push_str(&format!(
            "\nAgainst the middle of the vendor range: {wear:.2}\u{00D7} the \
             energy per mm\u{00B3}, {time:.2}\u{00D7} the cutting time.\n",
        )),
        _ if efficiency.band.is_some() => out.push_str(
            "\nWear and time ratios: not modelled — the band midpoint gives \
             the closed form no usable value.\n",
        ),
        _ => {}
    }
    out.push_str(
        "\nu = Ks + (F_edge \u{00B7} D \u{00B7} \u{03C8}) / (2 \u{00B7} ae \
         \u{00B7} fz), from this material's force line (ruling B6). Neither \
         RPM nor depth of cut moves it; only the \
         chipload and the radial engagement do. Approximate — verify on a \
         test cut.",
    );
    out
}

/// The hover for the refusal state.
///
/// The material has no force line (ruling B6), so there is no force fit to
/// read and no efficiency number to publish. This says which input is
/// missing and names the refusal the engine already makes, so the empty row
/// cannot be read as a clean bill of health.
fn unmodelled_hover(advance_mm: Option<f64>) -> String {
    let mut out = String::from(
        "Efficiency is not modelled for this material.\n\n\
         The energy-per-mm\u{00B3} model needs a force line, and this \
         material has no measured force line (ruling B6), so there is no \
         cutting-force fit to read. Nothing \
         is shown rather than a fabricated number — the same refusal the load \
         gate makes through MaterialUnvalidated. No wear ratio, no time ratio \
         and no force headroom exist for this cut.\n\n",
    );
    match advance_mm {
        Some(fz) => out.push_str(&format!(
            "The {fz:.4} mm/tooth beside this label is the recommendation's \
             commanded advance per tooth, feed \u{00F7} (RPM \u{00D7} flutes). \
             It does not depend on the force model.",
        )),
        None => out.push_str(
            "The advance per tooth is not shown either: the recommendation has \
             no positive feed, RPM and flute count to divide.",
        ),
    }
    out
}

/// The power row's payload: the figure at the point this operation ships, or
/// the typed refusal, plus the provenance of the bound.
///
/// The provenance is built from the figure's own RPM — the number the
/// post-simulation power gate stores on its own verdict — so the predicted row
/// and the measured row name the same source. The limit is the rated power
/// curve with no fraction (ruling R4 Q2, 2026-09-24). The GUI owns no limit: the ceiling is
/// [`PowerFigure::available_kw`], and nothing here computes one.
struct PowerReading {
    figure: Result<PowerFigure, PowerUnmodeled>,
    source: Option<BoundSource>,
    /// The sentence on the calculator's cut, when it draws a different
    /// percent (G-RECOMAPPLIED).
    calculator_note: Option<String>,
    /// The material's force line, or its named refusal (ruling B6).
    force_line: Result<ForceLine, ForceLineRefusal>,
    /// The force line at the mean chip of this cut: the power figure's own
    /// point, else `force_at_operating_point` (the door the MCP reads).
    /// `None` when the cut resolves no point.
    force_point: Option<ForceAtPoint>,
}

impl PowerReading {
    /// Evaluate the power this operation will draw at the point it ships.
    ///
    /// **Not `FeedsResult::power_kw`.** That figure sits at the CALCULATOR's
    /// geometry, and the operation can ship a different depth per pass (the
    /// operator's value or the envelope pick). Before feeds contract P5 the
    /// deleted `enforce_invariants` chain lowered the depth; on a shipped Ø12
    /// roughing preset the figure was 11.96× the power at the depth that
    /// cuts. The recommendation supplies
    /// only the FALLBACK point here, for an operation that exposes no depth,
    /// no stepover or no speed of its own — the surface-following finishes
    /// command no axial step, so the calculator's value is the correct
    /// substitute for them.
    fn at_shipped_point(
        operation: &crate::state::toolpath::OperationConfig,
        tool: &crate::state::job::ToolConfig,
        material: &rs_cam_core::material::Material,
        machine: &rs_cam_core::machine::MachineProfile,
        recommended: &rs_cam_core::feeds::FeedsResult,
        feed_max_rate: Option<f64>,
    ) -> Self {
        // P14 (review M9): the one core door of the shipped point. P20e: it
        // grades the steady feed under the op cap and the machine ceiling.
        let figure = rs_cam_core::feeds::power_at_shipped_point(
            operation,
            tool,
            material,
            machine,
            recommended,
            feed_max_rate,
        );
        let source = figure
            .as_ref()
            .ok()
            .map(|figure| BoundSource::MachinePowerCurve { rpm: figure.rpm });
        let force_point = figure.as_ref().ok().map(|figure| figure.force).or_else(|| {
            rs_cam_core::feeds::operating_point::force_at_shipped_point(
                operation,
                tool,
                material,
                machine,
                recommended,
                feed_max_rate,
            )
            .ok()
        });
        Self {
            figure,
            source,
            calculator_note: None,
            force_line: material.force_line(),
            force_point,
        }
    }
}

/// The rail's power row — 0 to the machine's own limit at this spindle
/// speed. It is a row of `Details`, never a face row (P29-10 10f, ruling O1:
/// the 70 % share that made it a problem row had no source).
///
/// # Why the bar does not read the body power row (P29-10 10f, item 5)
///
/// The design asked the bar to read the body `SpindlePower` planned row
/// when the two are equal on every op. They are not. On the four files of
/// `P29-10_DESIGN.md` 13 the available power is equal on every op, but the
/// required power differs by more than 1e-5 kW on 30 of 38 ops (equal on
/// 2, under 3e-6 kW apart on 6): the bar is the largest power
/// case that the cut admits (K7: the planner cap width, the slot) at the
/// applied cut and the steady feed; the row is the body alone at the
/// proposal. Examples: rivmap100 live idx 1 bar 10.0 %, row 3.3 %; the
/// drill ops bar 12.5 % (advisory, axial section), row 21 %. So the bar
/// stays as it is.
///
/// # Why the bar is back
///
/// It was deleted on a measurement: peak utilisation 23.6 % across the whole
/// shipped matrix, so the readout could not separate a good cut from a bad
/// one. R1 then rebuilt power on the affine force model, whose feed-free edge
/// term carries most of the load at wood chiploads, and the same sweep reads
/// median 17.8 %, p90 89.4 % and peak 100 %. The operator reinstated the bar
/// on 2026-09-18. `the_power_bar_is_informative_g_chipverdict` replaced the
/// ban with a test of its reason, and holds the new measurement.
///
/// # What the face carries, and what it does not
///
/// The face is the percent of the limit and nothing else: every limit on this
/// product reads 0 to limit, and "limit calculated from X" belongs in the
/// hover. The kW pair, the provenance clause and the four figures of the
/// operating point are on the hover. The source slot is the SETTING that
/// moves the bound, so a reader can trace the limit back to it.
///
/// A 100 % reading here is a real reading, not an absent constraint painted
/// as an all-clear: the Step 6 power ladder clamps a power-limited recipe
/// onto the ceiling rather than letting it through, so the top of the scale
/// is where those recipes land.
///
/// # A refusal is stated, never drawn
///
/// `power_at_operating_point` refuses, typed, on seven missing inputs. Each
/// paints its own clause in the dim abstention style the chipload row uses.
/// An empty bar would read as a cut with power to spare, which is the
/// opposite of what an absent model says.
fn power_row(power: &PowerReading) -> DataRow {
    let hover = match (&power.figure, &power.source) {
        (Ok(figure), Some(source)) => {
            let mut hover = power_hover(figure, source);
            if let Some(note) = &power.calculator_note {
                hover.push_str("\n\n");
                hover.push_str(note);
            }
            hover
        }
        (Err(reason), _) => power_unmodelled_hover(*reason),
        // `PowerReading` builds the source from the figure, so an `Ok` with
        // no source cannot occur. It is worded as an abstention rather than
        // a panic because a GUI row is not the place to assert an invariant.
        (Ok(_), None) => power_unmodelled_hover(PowerUnmodeled::NoAvailablePower),
    };
    match (&power.figure, &power.source) {
        (Ok(figure), Some(source)) => {
            // The door promises a finite, positive `available_kw`, so the
            // ratio is always defined. The FILL is clamped because a bar
            // cannot draw past its end; the TEXT is not, because a cut over
            // its ceiling must read over its ceiling.
            let utilisation = figure.required_kw / figure.available_kw;
            DataRow::bar(
                "Power",
                utilisation as f32,
                power_face(figure),
                compare::power_color(utilisation),
            )
            .source(Source::Setting(source.setting().to_owned()))
            .hover(hover)
        }
        _ => {
            let clause = match &power.figure {
                Err(reason) => reason.clause(),
                Ok(_) => PowerUnmodeled::NoAvailablePower.clause(),
            };
            DataRow::value(
                "Power",
                format!("{} {clause}", crate::ui::tokens::GLYPH_UNKNOWN),
            )
            .tone(theme::TEXT_DIM)
            .hover(hover)
        }
    }
}

/// The force-line rows (plan B6 §6), dim.
///
/// - The line's card text, or the refusal headline.
/// - Only when the mean chip of this cut is outside the printed range: the
///   extrapolation line. A measured chip adds no line.
///
/// The hover of each line is the line's detail, or the refusal detail.
fn force_line_rows(power: &PowerReading) -> Vec<DataRow> {
    let (face, hover) = rs_cam_core::material::force_line::card_lines(&power.force_line);
    let mut rows = vec![
        DataRow::record(face)
            .tone(theme::TEXT_DIM)
            .hover(hover.clone()),
    ];
    if power.force_line.is_ok()
        && let Some(extrapolation) = power
            .force_point
            .as_ref()
            .and_then(ForceAtPoint::extrapolation_text)
    {
        rows.push(
            DataRow::record(extrapolation)
                .tone(theme::TEXT_DIM)
                .hover(hover),
        );
    } else if power.force_line.is_ok()
        && power.force_point.is_none()
        && let Err(reason @ rs_cam_core::feeds::PowerUnmodeled::DepthAfterSimulation) = power.figure
    {
        // P16 (item 2b): a Project Curve has no chip regime before the
        // simulation; the row says why, not nothing.
        rows.push(
            DataRow::record(format!("Chip regime: unmodeled ({})", reason.clause()))
                .tone(theme::TEXT_DIM)
                .hover(hover),
        );
    }
    rows
}

/// The face: the percent of the limit, formatted from the two figures it
/// describes. No number on this face is typed; both come off the figure.
fn power_face(figure: &PowerFigure) -> String {
    let share = figure.required_kw / figure.available_kw * 100.0;
    // P20d: an advisory figure (the axial plunge) says so on the face.
    if figure.section.gates() {
        format!("{share:.1} %")
    } else {
        format!("{share:.1} % (advisory)")
    }
}

/// The row's workings: the kW pair the face does not carry, where the limit
/// came from, and the operating point the figure was evaluated at — so the
/// reader can see that the bar describes the depth that will be cut.
fn power_hover(figure: &PowerFigure, source: &BoundSource) -> String {
    // P20d (operator ruling 2026-11-08): an axial figure (a drill or a
    // straight plunge) reads `ap_mm` as the advance per revolution and
    // `ae_mm` as the diameter; a side cut keeps depth and width of cut.
    let (ap_label, ae_label) = power_point_labels(figure.section);
    // P20e: the feed the figure grades, and the cap that sets it.
    let feed_clause = match figure.steady_feed {
        Some(steady) if steady.limit() != rs_cam_core::machine::SteadyFeedLimit::OpFeed => {
            format!(
                "feed {:.0} mm/min ({} below the op feed {:.0} mm/min)",
                figure.feed_mm_min,
                steady.limit().label(),
                steady.op_feed_mm_min(),
            )
        }
        Some(_) | None => format!("feed {:.0} mm/min", figure.feed_mm_min),
    };
    format!(
        "Power {:.3} kW of {:.3} kW available \u{2014} {} of the limit.\n\n\
         Limit from {}. Setting: {}.\n\n\
         Evaluated at the cut `\u{26A1} Apply all` writes: {ap_label} \
         {:.3} mm, {ae_label} {:.3} mm, {:.0} rpm, {feed_clause}. The \
         bar describes the depth that WILL be cut, not the depth the \
         calculator sized its feed at \u{2014} the rigidity clamp can lower \
         one without moving the other.\n\n\
         Power case: {} (width {:.3} mm), the largest of the cases the cut \
         admits (K7).\n\n\
         {} (P20d).",
        figure.required_kw,
        figure.available_kw,
        power_face(figure),
        source.clause(),
        source.setting(),
        figure.ap_mm,
        figure.ae_mm,
        figure.rpm,
        figure.case.label(),
        figure.case_ae_mm,
        figure.section.basis_text(),
    )
}

/// P20d (operator ruling 2026-11-08): the hover labels of `ap_mm` and
/// `ae_mm`. An axial figure (`SectionKind::AxialCircle`) reads the advance
/// per revolution and the diameter; a side cut the depth and width of cut.
fn power_point_labels(
    section: rs_cam_core::feeds::section::SectionKind,
) -> (&'static str, &'static str) {
    use rs_cam_core::feeds::section::SectionKind;
    match section {
        SectionKind::AxialCircle => ("advance per rev", "diameter"),
        SectionKind::Rectangle | SectionKind::VGroove => ("depth of cut", "width of cut"),
    }
}

/// The hover for a refused power figure.
///
/// It names the missing input and the engine's own typed refusal, so the
/// empty row cannot be read as a cut with power to spare.
fn power_unmodelled_hover(reason: PowerUnmodeled) -> String {
    format!(
        "Power is not modelled at this operating point.\n\n\
         {} \u{2014} so feeds::power_at_operating_point refuses with \
         {reason:?}, and there is no figure and no limit to draw. Nothing is \
         drawn rather than a zero or an empty bar: an empty bar reads as a \
         cut with power to spare, which is the opposite of what an absent \
         model says.\n\n\
         The machine's power curve is unaffected. What is missing is the \
         input named above.",
        reason.clause(),
    )
}

/// The refusal of the apply, on the card face.
///
/// Checkpoint I-3 (2026-08-12): on a tool × operation pairing
/// `validate_tool_for_operation` refuses, the explanation remains visible but
/// the **apply buttons are replaced by the refusal**. The write becomes
/// impossible; the explanation survives.
///
/// Round 2: when every row is at the recommendation, the writes would
/// change nothing, so the buttons are not drawn.
fn draw_apply_refusal(ui: &mut egui::Ui, refusal: Option<&rs_cam_core::feeds::FeedsError>) {
    if let Some(err) = refusal {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "Cannot apply — Suggest refuses this recipe {}",
                    crate::ui::tokens::GLYPH_DETAIL
                ))
                .small()
                .strong()
                .color(theme::ERROR),
            )
            .wrap(),
        )
        .on_hover_text(
            "The numbers above show what the calculator would suggest. The \
             line below gives the reason for the refusal. Change the tool, \
             the operation or the material to enable Apply.",
        );
        // The one block on this card that may wrap: a refusal that blocks
        // the write (IC1).
        ui.add(
            egui::Label::new(
                egui::RichText::new(err.to_string())
                    .small()
                    .color(theme::ERROR),
            )
            .wrap(),
        );
    }
}

/// The label of the card's one disclosure (W3k: "Recommended"; it was
/// "Recommendation"). It holds the two apply buttons and every row.
pub(crate) const RECOMMENDED_DETAILS: &str = "Recommended";

/// `⚡ Apply all` and `⚡ Speeds only`, at the top of the `Recommended`
/// disclosure (W3k). The caller draws them only when Suggest does not refuse
/// and a row differs.
fn draw_apply_buttons(
    ui: &mut egui::Ui,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    // W3i: both buttons are small and share one row; a rail too narrow for
    // the pair wraps the second under the first.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = crate::ui::tokens::SPACE_2;
        // A-3's contract: the 'changes the cut' attribution sits on the
        // button's FACE, not its hover — the operator must see it before
        // clicking. The truncation only guards a rail narrower than the
        // face.
        let apply = egui::Button::new(egui::RichText::new(APPLY_ALL_FACE).small())
            .small()
            .wrap_mode(egui::TextWrapMode::Truncate);
        if ui
            .add(apply)
            .on_hover_text(
                "Overwrite RPM, feed, and plunge (how fast) AND DOC/WOC (the cut) with \
                 the recommended values, after the safety clamps. \
                 CHANGES THE CUT: the applied DOC/WOC are the invariant-resolved values. \
                 This is the one canonical route for applying the recommendation.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsAll(toolpath_id));
        }
        // Phase C (SPEC.md, 2026-09-16). The verdict row can say the
        // chipload is thin and costs 1.6× tool wear. Before this button the
        // only write was `⚡ Apply all`, so the only offered answer to a wrong
        // feed also rewrote the operator's cut geometry.
        //
        // A-3's attribution rule applies here too, in the mirror image: the
        // face says what this write changes AND what it holds, so the two
        // buttons read as one pair. This is not a per-field apply — it is the
        // same funnel with `ApplyScope::Speeds`, which the funnel was built to
        // serve.
        let speeds = egui::Button::new(egui::RichText::new(APPLY_SPEEDS_FACE).small())
            .small()
            .wrap_mode(egui::TextWrapMode::Truncate);
        if ui
            .add(speeds)
            .on_hover_text(
                "Overwrite RPM, feed, and plunge (how fast) with the recommended values, \
                 after the safety clamps, so the chipload lands in the vendor band. \
                 HOLDS THE CUT: DOC and WOC keep the values you authored. \
                 Use this when the chipload verdict reads thin or heavy and the cut \
                 geometry is already the one you want.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsSpeeds(toolpath_id));
        }
    });
}

/// WOC (stepover) comparison row. When the operation is in
/// scallop-driven-stepover mode (S1) the row is rendered as *derived*:
/// the label gains a scallop mark and a hover spelling out the chord-height
/// math, so it's visually distinct from a manually-entered stepover.
///
/// `woc` is the stepover `⚡ Apply all` writes (G-RECOMAPPLIED). The derived
/// value of the scallop formula stays on the hover.
fn woc_row(
    current: &CurrentValues,
    explain: &FeedsExplain,
    woc: f64,
    explanation: String,
) -> (DataRow, bool) {
    let scallop_active = current.supports_scallop_override && current.scallop_height.is_some();
    if !scallop_active {
        return change_row("WOC", current.stepover, Some(woc), " mm", 0.01, explanation);
    }

    let h = current.scallop_height.unwrap_or(0.0);
    let derived = explain.recommended.radial_width_mm;
    let tip_r = ball_tip_radius(explain);
    let math = match tip_r {
        Some(r) => format!(
            "Auto-derived from scallop height.\n\
             h = {:.0} μm, tip r = {:.2} mm\n\
             ae = 2·√(2·r·h − h²) = {derived:.3} mm",
            h * 1000.0,
            r,
        ),
        None => "Scallop-driven stepover needs a ball or tapered-ball tool; \
                 the formula default is used instead."
            .to_owned(),
    };
    // G-RECOMAPPLIED: the face prints what `⚡ Apply all` writes. When the
    // funnel moves the derived value, the hover says so.
    let math = if format!("{woc:.2}") == format!("{derived:.2}") {
        math
    } else {
        format!("{math}\nThe invariant funnel changes it, so Apply writes {woc:.2} mm.")
    };

    // The scallop-derived variant of the per-field WOC apply (census row M6)
    // was deleted with the other five at Checkpoint I-1: it wrote
    // `explain.recommended.radial_width_mm` raw, so it skipped
    // `clamp_stepover_to_diameter` and the runtime back-off along with
    // everything else. The derived value is on the hover; the face shows the
    // clamped form of it that `⚡ Apply all` writes (G-RECOMAPPLIED).
    let current_text = compare::format_optional(current.stepover, " mm", 0.01);
    let differs = current_text != format!("{woc:.2} mm");
    (
        DataRow::change(
            "WOC \u{24E2}",
            current_text,
            format!("{woc:.2} mm (auto)"),
            compare::delta_tag(current.stepover, Some(woc)),
        )
        .tone(theme::SUCCESS)
        .hover(format!("{math}\n\n{explanation}")),
        differs,
    )
}

/// S1 — scallop-driven-stepover control for DropCutter. A checkbox
/// toggles the override on/off; when on, a micron DragValue dials the
/// target cusp height and a live readout shows the derived stepover.
fn draw_scallop_control(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    let tip_r = ball_tip_radius(explain);
    // `horizontal_wrapped` so the DragValue and the derived readout wrap to
    // their own line inside the 240-point rail instead of overflowing it.
    ui.horizontal_wrapped(|ui| {
        let mut enabled = current.scallop_height.is_some();
        if ui
            .checkbox(&mut enabled, "Scallop-driven stepover")
            .on_hover_text(
                "Derive WOC from a target cusp (scallop) height and the tool's \
                 ball-tip radius instead of the formula default. Turn off to \
                 restore the formula-based stepover.",
            )
            .changed()
        {
            let value = enabled.then(|| current.scallop_height.unwrap_or(0.010));
            events.push(AppEvent::SetDropCutterScallopHeight { toolpath_id, value });
        }
        if let Some(h) = current.scallop_height {
            let mut microns = h * 1000.0;
            let resp = ui.add(
                egui::DragValue::new(&mut microns)
                    .speed(0.5)
                    .range(1.0..=500.0)
                    .suffix(" μm"),
            );
            if resp.changed() {
                let value = Some((microns / 1000.0).max(0.0001));
                events.push(AppEvent::SetDropCutterScallopHeight { toolpath_id, value });
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "→ ae {:.3} mm",
                        explain.recommended.radial_width_mm
                    ))
                    .small()
                    .color(theme::SUCCESS),
                )
                .truncate(),
            );
        }
    });
    if current.scallop_height.is_some() && tip_r.is_none() {
        DataTable::new("feeds_scallop_caution")
            .row(
                DataRow::note(Role::Caution, "⚠ No spherical tip: scallop height ignored")
                    .hover("This tool has no ball tip, so the formula stepover is used."),
            )
            .show(ui);
    }
}
