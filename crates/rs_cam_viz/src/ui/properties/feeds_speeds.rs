//! The feeds half of the Passes tab (P5b merged the Feeds tab into Passes):
//! the speed rows and the Chipload row, the inline chart and the
//! recommendation (`ui/feeds/explore.rs`, `ui/feeds/compare.rs`, W3i),
//! the measured operating point and the entry-style preview diagram. The
//! vendor cutting data is a window (`ui/feeds/vendor.rs`).

use super::linking_dressup::{Fields, help_for};
use crate::state::toolpath::{
    DressupConfig, DressupEntryStyle, HeightsConfig, OperationConfig, ToolpathEntry,
};
use crate::ui::components::{
    DataRow, DataTable, Details, PropRow, ProvKind, Role, SectionHeader, Source,
};
use rs_cam_core::feeds::FeedsField;

/// R1: the mark of a value that Suggest wrote (design rule 8).
const SUGGESTED_GLYPH: &str = "\u{26A1}";

/// R1: the hover of a value that Suggest wrote.
const SUGGESTED_HOVER: &str = "Suggested by the calculator. Type a value to replace it.";

/// The hover of an empty speed field (S11): the op refuses until it is set.
const NOT_SET_HOVER: &str = "Not set. The calculator has no data for this tool and \
     material, so type a value. The operation does not generate until it is set.";

/// F216, W3i: fill the toolpath's feeds memo (the calculator, the funnel
/// dry run and the explanation payload) for the bound tool. The panel host
/// calls it once per frame BEFORE it places the findings and draws a tab,
/// so every tab (the Geometry notes counter too) reads the same answer on
/// the first frame of a selection. The memo runs the work only when an
/// input moved. With no tool there is nothing to calculate.
pub(super) fn refresh_feeds_memo(
    entry: &mut ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
) {
    let Some(tool) = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t)
    else {
        return;
    };
    entry.feeds_memo.refresh(
        &mut entry.feeds_result,
        crate::state::runtime::FeedsInputs {
            operation: &entry.operation,
            tool,
            material: &inputs.material,
            machine: &inputs.machine,
            spindle_strategy: inputs.spindle_strategy,
            model_bbox,
        },
        super::pills::pill_previews,
    );
}

/// W6: the face of a feeds refusal; the refusal text is the hover.
pub(super) const FEEDS_UNAVAILABLE: &str = "Feeds unavailable";

/// What the Passes tab feeds block reads, besides the entry (P5b: the Feeds
/// tab merged into Passes). The panel host builds it once per frame.
pub(super) struct FeedsBlock<'a> {
    pub tool: &'a crate::state::job::ToolConfig,
    pub material: &'a rs_cam_core::material::Material,
    pub machine: &'a rs_cam_core::machine::MachineProfile,
    pub spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    /// Feeds contract P21b: the stored operator RPM ceiling.
    pub spindle_rpm_ceiling: Option<u32>,
    pub load_verdict: Option<&'a rs_cam_core::tool_load::ToolpathLoadVerdict>,
    /// Q1: the bbox of the model this toolpath machines, from
    /// `ToolpathPanelSnapshot`. The memo key reads it.
    pub model_bbox: Option<&'a rs_cam_core::geo::BoundingBox3>,
}

/// The card's funnel answers, once per input change (W3i), and the
/// explanation payload. The key is the `Debug` text of every input of the
/// build (the `FeedsKey` idiom), the dressups included. `None` before the
/// memo has a preview.
fn feeds_card(
    entry: &mut ToolpathEntry,
    block: &FeedsBlock<'_>,
) -> Option<(
    std::sync::Arc<rs_cam_core::feeds::suggest::FeedsPreview>,
    std::sync::Arc<crate::state::runtime::FeedsCard>,
)> {
    let preview = entry.feeds_memo.preview_shared()?;
    let FeedsBlock {
        tool,
        material,
        machine,
        spindle_strategy,
        model_bbox,
        ..
    } = *block;
    let key = format!(
        "{:?}|{tool:?}|{material:?}|{machine:?}|{spindle_strategy:?}|{model_bbox:?}|{:?}",
        entry.operation, entry.dressups
    );
    let card = entry.feeds_memo.card(key, || {
        crate::ui::feeds::compare::build_card(
            &entry.operation,
            &entry.dressups,
            tool,
            material,
            machine,
            spindle_strategy,
            model_bbox,
            &preview,
        )
    });
    Some((preview, card))
}

/// The nomogram of the Passes tab (W3i; P5b: under the envelope plot,
/// above the op rows). `candidate`: the speeds of the envelope candidate
/// ring (live link); the nomogram draws that cut while it exists. A
/// refused pairing draws no chart: the recommendation block names the
/// refusal.
pub(super) fn draw_feeds_chart(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    block: &FeedsBlock<'_>,
    candidate: Option<rs_cam_core::feeds::envelope::SpeedsSample>,
) {
    if entry.feeds_memo.refusal().is_some() {
        return;
    }
    let Some((preview, card)) = feeds_card(entry, block) else {
        return;
    };
    let op = crate::ui::feeds::shared::OpFeeds {
        toolpath_id: entry.id,
        operation: &entry.operation,
        tool: block.tool,
        material: block.material,
        machine: block.machine,
        feed_max_rate: entry.dressups.feed_max_rate,
        preview: &preview,
        card: &card,
    };
    crate::ui::feeds::explore::draw_inline(ui, &op, candidate);
}

/// The derived block of the Passes tab under the speed rows: the
/// recommendation (W3i), then the measured operating point. With no basis,
/// one line gives the refusal. The memo was filled by the panel host
/// (`refresh_feeds_memo`, F216); this block only reads it.
///
/// R1 (operator ruling 2026-10-30) replaces F.5 ("the calculator never
/// writes"): core seeds each EMPTY feed field with the Suggest value on op
/// add, and each empty or suggested one on a tool change
/// (`ProjectSession::seed_feeds`). This draw writes nothing itself; the
/// two apply buttons and the spindle policy push events. W3g: the cut half
/// of the recommendation is the "⚡" action of the Depth/pass and Stepover
/// rows, and the ⚡ of the envelope plot (P5b).
pub(super) fn draw_feeds_recommendation(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    block: &FeedsBlock<'_>,
    events: &mut Vec<crate::ui::AppEvent>,
) {
    if let Some(e) = entry.feeds_memo.refusal().map(ToString::to_string) {
        ui.add_space(8.0);
        // G-E (4), W6: one short face; the refusal is the hover. A tool
        // refusal is also the state of the Tool row.
        ui.add(
            crate::ui::components::OneLine::new(FEEDS_UNAVAILABLE)
                .color(crate::ui::tokens::DANGER)
                .detail(e),
        );
        return;
    }
    if let Some((preview, card)) = feeds_card(entry, block) {
        ui.add_space(crate::ui::tokens::SPACE_2);
        let op = crate::ui::feeds::shared::OpFeeds {
            toolpath_id: entry.id,
            operation: &entry.operation,
            tool: block.tool,
            material: block.material,
            machine: block.machine,
            feed_max_rate: entry.dressups.feed_max_rate,
            preview: &preview,
            card: &card,
        };
        let (strategy, rpm_ceiling, machine) = (
            block.spindle_strategy.kind(),
            block.spindle_rpm_ceiling,
            block.machine,
        );
        crate::ui::feeds::explore::draw_recommendation_inline(
            ui,
            &op,
            |ui, events| draw_spindle_policy_row(ui, strategy, rpm_ceiling, machine, events),
            events,
        );
    }
    draw_feeds_card(ui, block.load_verdict);
}

/// W1 — the operator sets the feed, the plunge and the RPM here.
///
/// # Why this exists again
///
/// `d323cabb` (UR4) deleted the `SPEED — how fast` section from this tab on
/// its way past. Feed and plunge had ALREADY been moved off the Geometry
/// panel into this section by W3.2, and the spindle override by W3.1, so
/// the deletion left the product with no way to set a feed rate at all —
/// only `⚡ Apply all`, which overwrites the whole recipe at once. The
/// operator found it the same day.
///
/// # The rows (W3c)
///
/// Each field is one `PropRow` (Feed, Plunge, Spindle, and Ramp feed when
/// the op has one), the unit inside the well, the field state from the
/// placement. R1: core fills an EMPTY field from Suggest on op add, and an
/// empty or suggested field on a tool change, and stamps it with a Suggest
/// source. Such a value shows ⚡
/// (`ValueProvenance::is_suggested`). A typed value replaces it. This block
/// does not consult the calculator: it reads the stamp.
///
/// # The lock (W3k)
///
/// The Feed, Spindle and Chipload rows end in a padlock. Exactly one of the
/// three is locked (`ToolpathConfig::feeds_lock`, default Chipload); an edit
/// of one of the other two moves the third (core `FeedsLock::resolve`). The
/// edited value and the value that moves with it land on the entry in the
/// same frame, so the write-back sends ONE command and stamps both Manual
/// (`FeedsProvenance::detect_manual_edits`). Plunge and Ramp feed take no
/// part in the lock.
///
/// The write idiom is the panel's own: edit the scratch `ToolpathEntry` and
/// stamp `stale_since`. The panel's write-back turns that into a `Command`,
/// so no draw site touches the session (WP6).
pub(super) fn draw_speed_controls(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    flutes: Option<u32>,
    machine: &rs_cam_core::machine::MachineProfile,
    fields: Fields<'_>,
) {
    use rs_cam_core::feeds::{FeedsLock, SpeedEdit, SpeedField};
    // A drill is Z-only: its plunge rate IS its feed rate. A second field
    // there is a duplicate that writes nothing (the W3.2 rule). F095: the
    // core drill-family predicate decides it.
    let z_only = entry.operation.op_type().is_drill_kinematics();
    let op_type = entry.operation.op_type();
    // Design rule 7: the small-caps label of the section; space groups it.
    SectionHeader::new("Speed")
        .small_caps()
        .hover(
            "The cut feed, the plunge feed and the spindle speed. ⚡ marks a value that \
             Suggest wrote. The padlock marks the one of feed, spindle speed and \
             chipload that holds when you edit another.",
        )
        .show(ui);
    let stamped = |field: FeedsField| {
        entry
            .feeds_provenance
            .get(field)
            .is_some_and(rs_cam_core::feeds::ValueProvenance::is_suggested)
    };
    let feed_suggested = stamped(FeedsField::FeedRate);
    let plunge_suggested = stamped(FeedsField::PlungeRate);
    let spindle_suggested = stamped(FeedsField::SpindleRpm);
    let ramp_suggested = stamped(FeedsField::RampFeedRate);

    let lock = entry.feeds_lock;
    let flutes = flutes.filter(|f| *f > 0);
    let feed_before = entry.operation.feed_rate();
    let rpm_before = entry.operation.spindle_rpm().map(f64::from);
    let feed_range = feed_range_for(op_type);
    let ranges = rs_cam_core::feeds::SpeedRanges {
        feed: (*feed_range.start(), *feed_range.end()),
        rpm: (*SPINDLE_RANGE.start(), *SPINDLE_RANGE.end()),
    };
    // The range of an edit under the lock: the row's own range, narrowed so
    // the value that moves with it stays inside ITS row's range.
    let edit_range = |field: SpeedField, own: std::ops::RangeInclusive<f64>| {
        flutes
            .and_then(|f| lock.edit_range(field, ranges, feed_before, rpm_before, f))
            .map_or(own, |(lo, hi)| lo..=hi)
    };
    // At most one of the three rows is edited in a frame.
    let mut edit: Option<SpeedEdit> = None;
    let mut new_lock = lock;

    // S11: a feed that Suggest did not write (no feeds data for the tool)
    // is not set. The field is empty, and the op refuses to generate until
    // it is set.
    let mut feed = feed_before;
    let (row, detail) = machine_state(
        speed_prop_row("Feed", op_type, "feed_rate", fields),
        fields.state("feed_rate").role.is_some(),
        feed_cap_reason(feed, machine),
    );
    let out = speed_row(
        ui,
        (lock_row(row, lock, FeedsLock::Feed), detail),
        &mut feed,
        "mm/min",
        50.0,
        edit_range(SpeedField::Feed, feed_range),
        feed_suggested,
        NOT_SET_HOVER,
    );
    if out.lock_clicked {
        new_lock = FeedsLock::Feed;
    }
    feed_cap_chip(ui, &out.rects, feed, entry.dressups.feed_max_rate, machine);
    if out.edited
        && let Some(feed) = feed
    {
        edit = Some(SpeedEdit::Feed(feed));
    }

    if z_only {
        let text = feed.map_or_else(|| "not set".to_owned(), |feed| format!("{feed:.0}"));
        let _ = PropRow::new("Plunge")
            .unit("mm/min")
            .hover(Some(
                "A drilling operation moves in Z only, so its plunge rate IS its feed \
                 rate. Set the feed above.",
            ))
            .show_read_only(ui, text);
    } else {
        let mut plunge = entry.operation.plunge_rate();
        let (row, detail) = speed_prop_row("Plunge", op_type, "plunge_rate", fields);
        if speed_row(
            ui,
            (row, detail),
            &mut plunge,
            "mm/min",
            10.0,
            1.0..=10_000.0,
            plunge_suggested,
            NOT_SET_HOVER,
        )
        .edited
            && let Some(plunge) = plunge
        {
            entry.operation.set_plunge_rate(plunge);
            entry.stale_since = Some(std::time::Instant::now());
        }
    }

    // S11: the spindle speed of the op. With no feeds data it is not set,
    // and the op refuses to generate until it is set; the post spindle
    // speed is no fallback.
    let mut spindle = rpm_before;
    let (row, detail) = machine_state(
        speed_prop_row("Spindle", op_type, "spindle_rpm", fields),
        fields.state("spindle_rpm").role.is_some(),
        spindle_range_reason(spindle, machine),
    );
    let out = speed_row(
        ui,
        (lock_row(row, lock, FeedsLock::Spindle), detail),
        &mut spindle,
        "RPM",
        RPM_DRAG_SPEED,
        edit_range(SpeedField::Spindle, SPINDLE_RANGE),
        spindle_suggested,
        NOT_SET_HOVER,
    );
    if out.lock_clicked {
        new_lock = FeedsLock::Spindle;
    }
    rpm_limit_chip(
        ui,
        &out.rects,
        entry
            .feeds_result
            .as_ref()
            .and_then(|r| r.rpm_choice.as_ref()),
    );
    if out.edited
        && let Some(rpm) = spindle
    {
        edit = Some(SpeedEdit::Spindle(rpm));
    }

    // G6 ramp: the entry feed, on an op that has the field. Empty means the
    // entry runs at the plunge rate.
    if entry.operation.param_names().contains(&"ramp_feed_rate") {
        let mut ramp = entry.operation.ramp_feed_rate();
        let range = OperationConfig::param_range_for_type(op_type, "ramp_feed_rate")
            .map_or(1.0..=50_000.0, |range| range.inclusive_bounds());
        let (row, detail) = speed_prop_row("Ramp feed", op_type, "ramp_feed_rate", fields);
        if speed_row(
            ui,
            (row, detail),
            &mut ramp,
            "mm/min",
            10.0,
            range,
            ramp_suggested,
            "Empty: the entry runs at the plunge rate.",
        )
        .edited
            && let Some(ramp) = ramp
            && entry
                .operation
                .as_params_mut()
                .set_ramp_feed_rate(Some(ramp))
        {
            entry.stale_since = Some(std::time::Instant::now());
        }
    }

    // W3i: the chipload of the feed and the spindle speed.
    let out = draw_chipload_row(
        ui,
        entry,
        flutes,
        lock,
        edit_range(SpeedField::Chipload, 0.0..=f64::MAX),
        feed_suggested && spindle_suggested,
    );
    if out.lock_clicked {
        new_lock = FeedsLock::Chipload;
    }
    if let Some(chipload) = out.edited {
        edit = Some(SpeedEdit::Chipload(chipload));
    }

    // W3k: the one write of the edit: the edited value and the value that
    // moves with it, in this frame.
    if let Some(edit) = edit {
        write_speed_edit(entry, edit, flutes, &ranges);
    }
    entry.feeds_lock = new_lock;
}

/// The range of the Spindle row: the clamp of a typed spindle speed.
const SPINDLE_RANGE: std::ops::RangeInclusive<f64> = 1_000.0..=60_000.0;

/// W3k: the write of one speed edit under the entry's lock: the edited
/// value and the one that moves with it (core `FeedsLock::resolve`). A
/// written value is clamped to its row's range, as a typed value is. With
/// no flutes the edit writes its own field only (a chipload edit writes
/// nothing). The write-back stamps each written value Manual.
fn write_speed_edit(
    entry: &mut ToolpathEntry,
    edit: rs_cam_core::feeds::SpeedEdit,
    flutes: Option<u32>,
    ranges: &rs_cam_core::feeds::SpeedRanges,
) {
    use rs_cam_core::feeds::{SpeedEdit, SpeedWrite};
    let feed = entry.operation.feed_rate();
    let rpm = entry.operation.spindle_rpm().map(f64::from);
    let write = match (flutes, edit) {
        (Some(flutes), _) => entry.feeds_lock.resolve(edit, feed, rpm, flutes),
        (None, SpeedEdit::Feed(f)) => SpeedWrite {
            feed: Some(f),
            rpm: None,
        },
        (None, SpeedEdit::Spindle(r)) => SpeedWrite {
            feed: None,
            rpm: Some(r),
        },
        (None, SpeedEdit::Chipload(_)) => return,
    };
    if let Some(feed) = write.feed {
        entry
            .operation
            .set_feed_rate(feed.clamp(ranges.feed.0, ranges.feed.1));
    }
    if let Some(rpm) = write.rpm {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the clamp holds the RPM to 1 000..=60 000, inside u32"
        )]
        entry
            .operation
            .set_spindle_rpm(Some(rpm.clamp(ranges.rpm.0, ranges.rpm.1).round() as u32));
    }
    entry.stale_since = Some(std::time::Instant::now());
}

/// The hover of a lock that is closed, per row.
fn lock_hover(
    lock: rs_cam_core::feeds::FeedsLock,
    row: rs_cam_core::feeds::FeedsLock,
) -> &'static str {
    use rs_cam_core::feeds::FeedsLock;
    if lock != row {
        return match row {
            FeedsLock::Feed => {
                "Lock the feed: an edit of the spindle speed or the chipload keeps it."
            }
            FeedsLock::Spindle => {
                "Lock the spindle speed: an edit of the feed or the chipload keeps it."
            }
            FeedsLock::Chipload => {
                "Lock the chipload: an edit of the feed or the spindle speed keeps it."
            }
        };
    }
    match row {
        FeedsLock::Feed => {
            "Locked: an edit of the spindle speed moves the chipload; an edit of the \
             chipload moves the spindle speed."
        }
        FeedsLock::Spindle => {
            "Locked: an edit of the feed moves the chipload; an edit of the chipload \
             moves the feed."
        }
        FeedsLock::Chipload => {
            "Locked: an edit of the feed moves the spindle speed; an edit of the \
             spindle speed moves the feed."
        }
    }
}

/// The row with its padlock: closed when `row` is the lock.
fn lock_row<'a>(
    row: PropRow<'a>,
    lock: rs_cam_core::feeds::FeedsLock,
    this: rs_cam_core::feeds::FeedsLock,
) -> PropRow<'a> {
    row.lock(lock == this).action_hover(lock_hover(lock, this))
}

/// W3k: a Caution on a speed row whose value is outside the machine's
/// range, when the placement gives the row no state (`placed` is `false`).
/// A value typed or moved by the lock is written as it is; the row says
/// that the machine cannot run it.
fn machine_state<'a>(
    (row, detail): (PropRow<'a>, Option<String>),
    placed: bool,
    reason: Option<String>,
) -> (PropRow<'a>, Option<String>) {
    match reason {
        Some(reason) if !placed => (row.state(Some(Role::Caution)), Some(reason)),
        _ => (row, detail),
    }
}

/// P20e: one chip beside the Feed value when a cap is below the op feed:
/// the G-code commands `min(op feed, feed_max_rate, cutting ceiling)`
/// (`MachineProfile::op_steady_feed`, the ceiling the modulator caps at).
/// The chip sits in the label column, right-aligned to the padlock, so it
/// takes no width from the well and adds no row (the operator rule: no
/// text clutter).
fn feed_cap_chip(
    ui: &mut egui::Ui,
    rects: &crate::ui::components::RowRects,
    feed: Option<f64>,
    feed_max_rate: Option<f64>,
    machine: &rs_cam_core::machine::MachineProfile,
) {
    let Some(steady) = feed.map(|f| machine.op_steady_feed(f, feed_max_rate)) else {
        return;
    };
    if steady.limit() == rs_cam_core::machine::SteadyFeedLimit::OpFeed {
        return;
    }
    let chip = crate::ui::components::StatusChip::new(
        format!("cap {:.0}", steady.mm_min()),
        crate::ui::components::Role::Info,
    )
    .hover(format!(
        "{} runs every cut at {:.0} mm/min, below the op feed {:.0} mm/min. \
         The chipload gate and the power figure grade {:.0} mm/min.",
        capitalise(steady.limit().label()),
        steady.mm_min(),
        steady.op_feed_mm_min(),
        steady.mm_min(),
    ));
    label_slot_chip(ui, rects, "Feed", chip);
}

/// One chip in the label column of a speed row, right-aligned to the
/// padlock, after the painted label text `label`. It takes no width from
/// the well and adds no row.
fn label_slot_chip(
    ui: &mut egui::Ui,
    rects: &crate::ui::components::RowRects,
    label: &str,
    chip: crate::ui::components::StatusChip,
) {
    let right = rects.action.map_or(rects.label.right(), |a| {
        a.left() - crate::ui::tokens::SPACE_2
    });
    // The slot starts after the painted label text.
    let label_w = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::PLACEHOLDER,
        )
        .size()
        .x;
    let left = (rects.label.left() + label_w + crate::ui::tokens::SPACE_2).min(right);
    let slot = egui::Rect::from_min_max(
        egui::pos2(left, rects.label.top()),
        egui::pos2(right, rects.label.bottom()),
    );
    let _ = ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(slot)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| ui.add(chip),
    );
}

/// P21c: one chip beside the Spindle value under the constant-chip policy:
/// the limit that sets the Suggest RPM. The hover lists every limit with
/// its RPM. No chip under Match chart (`FeedsResult::rpm_choice` is
/// `None`).
fn rpm_limit_chip(
    ui: &mut egui::Ui,
    rects: &crate::ui::components::RowRects,
    choice: Option<&rs_cam_core::feeds::RpmChoice>,
) {
    let Some(choice) = choice else {
        return;
    };
    let chip = crate::ui::components::StatusChip::new(
        choice.binding.label().to_owned(),
        crate::ui::components::Role::Info,
    )
    .hover(rpm_limit_hover(choice));
    label_slot_chip(ui, rects, "Spindle", chip);
}

/// The hover of [`rpm_limit_chip`]: the Suggest RPM, the limit that binds
/// and each limit with its RPM.
fn rpm_limit_hover(choice: &rs_cam_core::feeds::RpmChoice) -> String {
    format!(
        "Constant chip: Suggest gives {:.0} RPM, set by {}.\n{}\nAcceleration: no cap; \
         the share of the cut at the full feed is known after generation.",
        choice.rpm,
        choice.binding.label(),
        choice.limit_lines().join("\n"),
    )
}

/// The text with its first letter in upper case.
fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The reason of a feed above the machine's cutting feed limit.
fn feed_cap_reason(
    feed: Option<f64>,
    machine: &rs_cam_core::machine::MachineProfile,
) -> Option<String> {
    let cap = machine.cutting_ceiling().mm_min();
    feed.filter(|f| *f > cap)
        .map(|_| format!("Above the machine's cutting feed limit of {cap:.0} mm/min."))
}

/// The reason of a spindle speed outside the machine's spindle range.
fn spindle_range_reason(
    rpm: Option<f64>,
    machine: &rs_cam_core::machine::MachineProfile,
) -> Option<String> {
    let (lo, hi) = machine.rpm_range();
    let rpm = rpm?;
    if rpm > hi {
        Some(format!(
            "Above the machine's spindle maximum of {hi:.0} RPM."
        ))
    } else if rpm < lo {
        Some(format!(
            "Below the machine's spindle minimum of {lo:.0} RPM."
        ))
    } else {
        None
    }
}

/// The clamp range of the Feed row of `op_type`. W5: an op whose schema
/// states a feed range (the drill ops) clamps to it; the others take this
/// row's own range.
fn feed_range_for(
    op_type: rs_cam_core::compute::catalog::OperationType,
) -> std::ops::RangeInclusive<f64> {
    OperationConfig::param_range_for_type(op_type, "feed_rate")
        .map_or(1.0..=50_000.0, |range| range.inclusive_bounds())
}

/// The unit of the Chipload row: millimetres per tooth. The short form
/// fits the four-decimal value and the unit in the 240-point rail; the
/// hover spells it out.
const CHIPLOAD_UNIT: &str = "mm/t";

/// The hover of the Chipload row.
const CHIPLOAD_HOVER: &str = "The commanded advance per tooth, in mm per tooth: feed \
     \u{00F7} (spindle speed \u{00D7} flutes). The padlock of the feed, spindle and \
     chipload rows names the value that holds when you edit another.";

/// What the Chipload row returns: the typed chipload, and a click on its
/// padlock.
struct ChiploadRowOut {
    edited: Option<f64>,
    lock_clicked: bool,
}

/// W3i — the Chipload row: the commanded advance per tooth of the op's feed,
/// spindle speed and tool flutes, through core `AdvancePerToothMm`. W3k: an
/// edit returns the typed chipload; the caller writes it under the lock
/// (`write_speed_edit`). `range` is the edit range under the lock.
///
/// With no feed, no spindle speed or no flutes the row is read-only "—"
/// and the hover gives the reason. ⚡ shows when the feed and the spindle
/// speed both carry a Suggest stamp: then the chipload is Suggest's too.
fn draw_chipload_row(
    ui: &mut egui::Ui,
    entry: &ToolpathEntry,
    flutes: Option<u32>,
    lock: rs_cam_core::feeds::FeedsLock,
    range: std::ops::RangeInclusive<f64>,
    suggested: bool,
) -> ChiploadRowOut {
    use rs_cam_core::feeds::FeedsLock;
    let rpm = entry.operation.spindle_rpm().filter(|rpm| *rpm > 0);
    let (Some(_), Some(flutes)) = (rpm, flutes) else {
        let reason = if rpm.is_none() {
            "The spindle speed is not set, so the chipload has no value. Set the spindle \
             speed above."
        } else {
            "The tool has no flutes, so the chipload has no value."
        };
        let out = lock_row(PropRow::new("Chipload"), lock, FeedsLock::Chipload)
            .unit(CHIPLOAD_UNIT)
            .hover(Some(CHIPLOAD_HOVER))
            .detail(Some(reason))
            .show_read_only(ui, "\u{2014}");
        return ChiploadRowOut {
            edited: None,
            lock_clicked: out.action_clicked,
        };
    };
    let mut chipload = chipload_of(&entry.operation, flutes);
    let row = (
        lock_row(
            PropRow::new("Chipload").hover(Some(CHIPLOAD_HOVER)),
            lock,
            FeedsLock::Chipload,
        ),
        None,
    );
    let out = speed_row_spec(
        ui,
        row,
        &mut chipload,
        CHIPLOAD_UNIT,
        &crate::ui::components::NumberSpec::new(0.0005)
            .range(range)
            .decimals(4),
        suggested,
        "The feed is not set. Type a chipload to set the feed at this spindle speed.",
    );
    ChiploadRowOut {
        edited: chipload.filter(|_| out.edited),
        lock_clicked: out.lock_clicked,
    }
}

/// The chipload of `operation` for a tool of `flutes`: core
/// `AdvancePerToothMm::from_commanded_at` of the feed at the spindle speed.
/// `None` with no feed, no spindle speed or no flutes.
fn chipload_of(operation: &OperationConfig, flutes: u32) -> Option<f64> {
    use rs_cam_core::feeds::{AdvancePerToothMm, CommandedFeedMmMin};
    let rpm = f64::from(operation.spindle_rpm()?);
    let feed = operation.feed_rate()?;
    AdvancePerToothMm::from_commanded_at(CommandedFeedMmMin::new(feed), rpm, flutes)
        .map(AdvancePerToothMm::mm)
}

/// The `PropRow` of one speed field: the label, the registry help of the
/// parameter `name` (ui04) and the field state from the placement.
fn speed_prop_row<'a>(
    label: &'a str,
    op_type: rs_cam_core::compute::catalog::OperationType,
    name: &'static str,
    fields: Fields<'a>,
) -> (PropRow<'a>, Option<String>) {
    let state = fields.state(name);
    (
        PropRow::new(label)
            .hover(help_for(op_type, name))
            .state(state.role),
        state.detail,
    )
}

/// One speed row: a `DragValue` in the numeric well (W3g), the unit
/// inside the well, and ⚡ in amber in the left padding of the well when
/// Suggest wrote the value (R1). The mark takes no width from the value, so
/// a five-digit RPM fits the 240-point rail.
///
/// An empty value draws "—" (no long hint fits the rail); its hover adds
/// `empty_hover`. A typed number sets it, clamped to the range.
#[expect(
    clippy::too_many_arguments,
    reason = "the row, the value, its unit, speed and range, the mark and the empty hover"
)]
fn speed_row(
    ui: &mut egui::Ui,
    row: (PropRow<'_>, Option<String>),
    val: &mut Option<f64>,
    unit: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suggested: bool,
    empty_hover: &str,
) -> SpeedRowOut {
    let spec = crate::ui::components::NumberSpec::new(speed).range(range);
    speed_row_spec(ui, row, val, unit, &spec, suggested, empty_hover)
}

/// What a speed row returns: the value changed, and its padlock (W3k) was
/// clicked.
struct SpeedRowOut {
    edited: bool,
    lock_clicked: bool,
    /// The rectangles of the drawn row (P20e: the cap chip sits beside the
    /// padlock).
    rects: crate::ui::components::RowRects,
}

/// [`speed_row`] with the caller's `NumberSpec` (the Chipload row takes four
/// decimals).
fn speed_row_spec(
    ui: &mut egui::Ui,
    (row, detail): (PropRow<'_>, Option<String>),
    val: &mut Option<f64>,
    unit: &str,
    spec: &crate::ui::components::NumberSpec<'_>,
    suggested: bool,
    empty_hover: &str,
) -> SpeedRowOut {
    let mut changed = false;
    let was_set = val.is_some();
    let out = row
        .detail(detail.as_deref())
        .numeric()
        .unit(unit)
        .show(ui, |ui| {
            // An empty value draws "—" (`NumberSpec::new`).
            let (resp, edited) =
                crate::ui::components::optional_number_field(ui, val, spec, ui.available_size());
            changed |= edited;
            resp
        });
    let mark = suggested && was_set;
    if mark {
        // The mark sits in the left padding of the well, before the value.
        let at = egui::pos2(out.rects.well.left() + 2.0, out.rects.well.center().y);
        let _ = ui.painter().text(
            at,
            egui::Align2::LEFT_CENTER,
            SUGGESTED_GLYPH,
            egui::FontId::proportional(crate::ui::tokens::SIZE_CAPTION),
            crate::ui::tokens::CAUTION,
        );
    }
    let hover = if mark {
        format!("{}\n\n{SUGGESTED_HOVER}", out.hover)
    } else if was_set {
        out.hover
    } else {
        format!("{}\n\n{empty_hover}", out.hover)
    };
    let _ = out.inner.on_hover_text(hover);
    SpeedRowOut {
        edited: changed,
        lock_clicked: out.action_clicked,
        rects: out.rects,
    }
}

/// The derived rows of the tab: the measured operating point. The
/// recommendation (`⚡ Apply all`, `⚡ Speeds only`, its `Details`) is
/// above it, under the chart (W3i).
fn draw_feeds_card(
    ui: &mut egui::Ui,
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
) {
    // This is accepted simulation evidence, not a recommendation.
    if let Some(v) = load_verdict
        && (v.feed_explanation.is_some() || v.modulation_summary.is_some())
    {
        ui.add_space(8.0);
        draw_operating_point(ui, v);
    }
}

/// Map a chipload provenance source into the shared UI vocabulary.
///
/// The per-field suggestion builder lives in `pills.rs` and still uses this
/// mapping even though UR4 removes the duplicate pills from the Feeds tab.
pub(super) fn prov_from_chipload(
    source: &rs_cam_core::feeds::ChiploadSource,
) -> (ProvKind, Option<&str>) {
    use rs_cam_core::feeds::ChiploadSource;

    match source {
        ChiploadSource::VendorLut { observation_id } => {
            (ProvKind::VendorLut, Some(observation_id.as_str()))
        }
        ChiploadSource::FormulaFallback => (ProvKind::Formula, None),
        ChiploadSource::EdgeRadiusFloor => (ProvKind::EdgeRadiusFloor, None),
    }
}

/// F-039 — read-only "solved operating point" for this toolpath: the single
/// constraint that bound feed across the most cuts, how far modulation moved
/// the feed off the commanded value, and how much of the path it touched. The
/// *measured* counterpart to the Suggest-predicted "Derived" rollup; the data
/// is the per-toolpath modulation summary captured during simulation
/// (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §10.6). Display-only — it is
/// the optimizer's result, not a field to edit.
///
/// Round 2 (2026-10-13): one table. The default view is one status row: the
/// achieved advance per tooth with the chipload gate verdict. The commanded
/// advance, the vendor band, the binding constraint and the modulation rows
/// are derivation, in `Details`.
fn draw_operating_point(ui: &mut egui::Ui, verdict: &rs_cam_core::tool_load::ToolpathLoadVerdict) {
    SectionHeader::new("Operating point (measured)").show(ui);
    let mut face = DataTable::new("feeds_operating_point");
    let mut details = DataTable::new("feeds_operating_point_details");
    if let Some(explain) = verdict.feed_explanation.as_deref() {
        let (role, word) = advance_gate_verdict(&verdict.chipload);
        let (achieved, rest) = advance_per_tooth_rows(explain);
        // The status truncates in the rail, so the hover ends with it.
        face = face.row(achieved.status(role, word.clone()).hover_suffix(&word));
        details = details.rows(rest);
    }
    if let Some(summary) = verdict.modulation_summary.as_ref() {
        details = details.rows(modulation_rows(summary));
    }
    if face.is_empty() {
        face = face.row(DataRow::not_measured("Chipload gate").hover(
            "The simulation recorded no feed explanation for this toolpath, so the \
             chipload gate has no advance per tooth to judge.",
        ));
    }
    face.show(ui);
    let count = details.len();
    if count > 0 {
        Details::new("feeds_operating_point_details")
            .count(count)
            .show(ui, |ui| {
                details.show(ui);
            });
    }
}

/// Checkpoint H4.4 — the chip rows: **Commanded advance / Achieved peak
/// chip / Vendor band**, read-only. P11b2: the achieved row is the gate's
/// peak chip (the advance over the chip-thinning lift), not the advance.
///
/// The point of putting them adjacent is that the interesting number is
/// the *gap*: commanded is what the operator typed, achieved is what the
/// machine reaches after the F-035 kinematics substitution, and the band
/// is the only one of the three published by a vendor. A recommendation
/// accepted against the commanded figure alone can be well outside the
/// band by the time the cutter is in the wood. Every number below is read
/// off the gate's own `FeedExplanation` stages — nothing is recomputed
/// here, so the rows cannot drift from the verdict they sit beside.
///
/// Display-only. This is the Feeds tab, and the standing rule is that it
/// never auto-locks a numeric field — there is no input affordance here.
///
/// Returns the achieved row, which carries the gate verdict on the face, and
/// the commanded and band rows for `Details`.
fn advance_per_tooth_rows(
    explain: &rs_cam_core::feeds::FeedExplanation,
) -> (DataRow, Vec<DataRow>) {
    use rs_cam_core::feeds::{ACHIEVED_PEAK_CHIP, COMMANDED_ADVANCE_PER_TOOTH};
    // P11b2: the gate judges the PEAK chip, the advance over the radial
    // chip-thinning lift; the hover gives the commanded peak chip too.
    // P20e: the commanded feed is the G-code F word, the steady feed under
    // the op cap and the machine ceiling; name the cap when it binds.
    let cap_clause = match explain.commanded.feed_limit {
        rs_cam_core::machine::SteadyFeedLimit::OpFeed => String::new(),
        limit @ (rs_cam_core::machine::SteadyFeedLimit::OpFeedCap
        | rs_cam_core::machine::SteadyFeedLimit::MachineCeiling(_)) => format!(
            " ({} below the op feed {:.0} mm/min)",
            limit.label(),
            explain.commanded.op_feed_mm_min,
        ),
    };
    let commanded_hover = format!(
        "feed {:.0} mm/min{cap_clause} \u{00f7} ({} RPM \u{00d7} {} flutes). Peak chip {:.4} mm \
         (\u{00f7} the median chip-thinning lift {:.3} of the measured cut).",
        explain.commanded.feed_rate_mm_min,
        explain.commanded.spindle_rpm,
        explain.commanded.flute_count,
        explain.commanded.peak_chip_mm,
        explain.commanded.thinning_lift,
    );
    let ratio_note = match explain.achieved_feed.median_ratio {
        Some(r) => format!(
            "effective feed \u{00f7} (RPM \u{00d7} flutes) \u{00f7} the chip-thinning \
             lift at each sample's arc, over the {} \
             {}. The machine reaches {:.0}% of the commanded feed \
             (median) on this path.",
            explain.gate.sample_count,
            explain.gate.statistic.label(),
            r * 100.0,
        ),
        // No predicted-feed map: `effective_feed_for_sample` returns the
        // commanded feed, so this row IS the commanded value and must say so
        // rather than implying a measurement.
        None => format!(
            "No kinematics prediction on this trace, so the effective \
             feed falls back to the commanded feed \u{2014} this row is \
             not independent evidence. Over the {} {}.",
            explain.gate.sample_count,
            explain.gate.statistic.label(),
        ),
    };
    let band = match explain.band.min_mm_per_tooth {
        Some(lo) => format!("{lo:.4}\u{2013}{:.4}", explain.band.max_mm_per_tooth),
        None => format!("\u{2264} {:.4}", explain.band.max_mm_per_tooth),
    };
    let band_hover = format!(
        "Vendor chipload column from row {} (calibrated d={:.3} mm), \
         as printed at every depth. It holds the peak chip (Garr chip \
         thinning), the quantity of the achieved row.",
        explain.band.observation_id, explain.band.row_diameter_mm,
    );
    let achieved = DataRow::measured(
        ACHIEVED_PEAK_CHIP,
        format!("{:.4}", explain.gate.value_mm),
        "mm/tooth",
    )
    .hover(ratio_note);
    let rest = vec![
        DataRow::measured(
            COMMANDED_ADVANCE_PER_TOOTH,
            format!("{:.4}", explain.commanded.feed_per_tooth_mm),
            "mm/tooth",
        )
        .hover(commanded_hover),
        DataRow::measured("Vendor band", band, "mm/tooth")
            .source(Source::Prov(
                ProvKind::VendorLut,
                Some(explain.band.observation_id.clone()),
            ))
            .hover(band_hover),
    ];
    (achieved, rest)
}

/// The modulation rows: the constraint that bound the feed on the most
/// cuts, the feed against the commanded feed, the share of cuts the
/// modulator touched, the strategy, and one row for each constraint when
/// more than one bound somewhere on the path.
fn modulation_rows(summary: &rs_cam_core::tool_load::ModulationSummary) -> Vec<DataRow> {
    use rs_cam_core::tool_load::ModulationStrategyTag;
    let mut rows = Vec::new();
    if let Some((binding, frac)) = summary
        .binding_constraint_distribution
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        rows.push(DataRow::value(
            "Limited by",
            format!("{} ({:.0} % of cuts)", binding.label(), frac * 100.0),
        ));
    }
    let d = summary.median_feed_delta_pct;
    let sign = if d >= 0.0 { "+" } else { "" };
    rows.push(DataRow::value(
        "Feed vs commanded",
        format!("{sign}{d:.0} % median"),
    ));
    rows.push(DataRow::value(
        "Modulated",
        format!("{} / {} cuts", summary.moves_touched, summary.moves_total),
    ));
    let strategy = match summary.strategy {
        ModulationStrategyTag::ConstrainedMax => "constrained-max",
        ModulationStrategyTag::BandMid => "band-mid",
    };
    rows.push(DataRow::value(
        "Strategy",
        format!("{strategy} \u{00b7} feed scale {:.1}", summary.feed_scale),
    ));
    // The per-constraint breakdown, only when more than one constraint
    // bound somewhere on the path.
    if summary.binding_constraint_distribution.len() > 1 {
        for (binding, frac) in &summary.binding_constraint_distribution {
            rows.push(DataRow::value(
                format!("  {}", binding.label()),
                format!("{:.0} % of cuts", frac * 100.0),
            ));
        }
    }
    // P16 item 7: the notes of the run (for example "force budget: no
    // force line").
    for note in &summary.notes {
        rows.push(DataRow::value("Note", note.clone()));
    }
    rows
}

/// The chipload gate's verdict as a role and a lower-case phrase.
fn advance_gate_verdict(chipload: &rs_cam_core::tool_load::ChiploadVerdict) -> (Role, String) {
    use rs_cam_core::tool_load::ChiploadVerdict;
    use rs_cam_core::tool_load::verdict::ChipSide;
    match chipload {
        ChiploadVerdict::Within { burn_advisory, .. } => match burn_advisory {
            Some(_) => (Role::Caution, "within band (burn advisory)".to_owned()),
            None => (Role::Ok, "within band".to_owned()),
        },
        ChiploadVerdict::Exceeds { side, .. } => (
            Role::Danger,
            match side {
                ChipSide::Low => "below band (burn/rubbing)".to_owned(),
                ChipSide::High => "above band (breakage)".to_owned(),
            },
        ),
        // A gate that could not evaluate must not render as a pass.
        ChiploadVerdict::Unmodeled { reason } => {
            (Role::Unknown, format!("not modelled ({reason:?})"))
        }
    }
}

// ── Spindle policy ──────────────────────────────────────────────────

/// The drag speed of an RPM field (RPM per pixel): the Spindle row and the
/// RPM ceiling row.
const RPM_DRAG_SPEED: f64 = 100.0;

/// The spindle policy of the project (chart RPM or constant chip), the
/// first row of the recommendation `Details` (W3i). The Explore window held
/// it as two radio buttons; it is the one GUI control of
/// `PostConfig::spindle_strategy` (MCP: `set_spindle_strategy`).
///
/// Feeds contract P21b: one "RPM ceiling" row under it, drawn under
/// Constant chip or while a ceiling is set. Empty is no ceiling (the RPM
/// stays at the chart RPM, P21c); a ceiling
/// above the spindle maximum takes the Caution state, the sentence on
/// hover. The row never writes zero (core refuses it): its range starts
/// at one RPM.
fn draw_spindle_policy_row(
    ui: &mut egui::Ui,
    current: rs_cam_core::feeds::SpindleStrategyKind,
    rpm_ceiling: Option<u32>,
    machine: &rs_cam_core::machine::MachineProfile,
    events: &mut Vec<crate::ui::AppEvent>,
) {
    use rs_cam_core::feeds::SpindleStrategyKind;
    let (_, max_rpm) = machine.rpm_range();
    let label = |s: SpindleStrategyKind| match s {
        SpindleStrategyKind::MatchChart => "Match chart",
        SpindleStrategyKind::ConstantChip => "Constant chip",
    };
    let hover = format!(
        "Match chart: use the RPM that the vendor row prints. Constant chip: hold the \
         chip per tooth and raise the RPM to the first limit: the RPM ceiling, the \
         spindle maximum ({max_rpm:.0} RPM), the cutting feed limit, the Z axis rate \
         (a drill), the spindle power, or the vendor RPM maximum. The feed rises with \
         the RPM. The machine must state its axis rates and accelerations. This is a \
         project setting: it moves the recommendation of every toolpath."
    );
    let mut next = current;
    let _ = super::linking_dressup::combo_in_row(
        ui,
        PropRow::new("Spindle policy").hover(Some(&hover)),
        "feeds_spindle_policy",
        label(current),
        |ui| {
            for s in [
                SpindleStrategyKind::MatchChart,
                SpindleStrategyKind::ConstantChip,
            ] {
                let _ = ui.selectable_value(&mut next, s, label(s));
            }
        },
    );
    let mut next_ceiling = rpm_ceiling;
    if next == SpindleStrategyKind::ConstantChip || rpm_ceiling.is_some() {
        super::placement::note_drawn("spindle_rpm_ceiling");
        let warning =
            rpm_ceiling.and_then(|c| rs_cam_core::gcode::rpm_ceiling_above_spindle(c, machine));
        let ceiling_hover = warning.clone().unwrap_or_else(|| {
            "The operator RPM ceiling of the constant-chip strategy. Suggest never \
             gives an RPM above it. Empty: the RPM stays at the chart RPM."
                .to_owned()
        });
        let row = PropRow::new("RPM ceiling")
            .hover(Some(&ceiling_hover))
            .state(
                warning
                    .as_ref()
                    .map(|_| crate::ui::components::Role::Caution),
            );
        let mut value = rpm_ceiling.map(f64::from);
        if super::linking_dressup::opt_row(
            ui,
            row,
            &mut value,
            "RPM",
            RPM_DRAG_SPEED,
            1.0..=f64::INFINITY,
            "none",
            true,
        ) {
            next_ceiling = value.map(|v| v.round() as u32);
        }
    }
    if next != current || next_ceiling != rpm_ceiling {
        events.push(crate::ui::AppEvent::SetSpindleStrategy {
            strategy: next,
            rpm_ceiling: next_ceiling,
        });
    }
}

// ── Engagement diagram ──────────────────────────────────────────────────

// ── Entry style preview diagram ─────────────────────────────────────────

/// Draw a 2D side-view of the entry style geometry (ramp or helix).
///
/// `helix_radius_mm` is the radius the engine emits (G10 Q6 and D2: the
/// operator value or 0.3 x D, capped at the flat bottom); `None` when no
/// tool is bound and the rule has no D, and then no helix is drawn.
pub(super) fn draw_entry_preview_diagram(
    ui: &mut egui::Ui,
    dressups: &DressupConfig,
    helix_radius_mm: Option<f64>,
    answer: &rs_cam_core::session::ToolpathHeights,
    heights: &HeightsConfig,
) {
    let resolved = answer.resolve(heights);
    let feed_z = resolved.feed_z;
    let top_z = resolved.top_z;
    let z_drop = feed_z - top_z;
    if z_drop <= 0.0 {
        return;
    }

    let desired_size = egui::vec2(ui.available_width().min(260.0), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    // Z range with margin
    let z_min = top_z - z_drop * 0.15;
    let z_max = feed_z + z_drop * 0.25;

    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        rect.bottom() - (frac as f32) * rect.height()
    };

    let feed_y = z_to_y(feed_z);
    let top_y = z_to_y(top_z);
    let dim_color = crate::ui::tokens::TEXT_FAINT;
    let scale_color = crate::ui::tokens::BORDER;

    // Z-axis scale bar on the left edge
    let scale_x = rect.left() + 3.0;
    painter.line_segment(
        [egui::pos2(scale_x, feed_y), egui::pos2(scale_x, top_y)],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    // Ticks + values at feed_z and top_z
    painter.line_segment(
        [
            egui::pos2(scale_x, feed_y),
            egui::pos2(scale_x + 4.0, feed_y),
        ],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    painter.line_segment(
        [egui::pos2(scale_x, top_y), egui::pos2(scale_x + 4.0, top_y)],
        egui::Stroke::new(1.0_f32, scale_color),
    );
    // Z drop distance label
    painter.text(
        egui::pos2(scale_x + 2.0, (feed_y + top_y) / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{z_drop:.1}"),
        egui::FontId::proportional(8.0),
        scale_color,
    );

    // Dashed horizontal reference lines
    // W3e: the words and the precision of the Heights rows, so the diagram
    // and the rows print one number one way.
    for &(z, label) in &[
        (feed_z, "Feed"),
        (top_z, super::operations::TOP_HEIGHT_LABEL),
    ] {
        let y = z_to_y(z);
        // Draw dashed line
        let dash_len = 6.0;
        let gap_len = 4.0;
        let mut x = rect.left() + 12.0;
        while x < rect.right() - 50.0 {
            let end_x = (x + dash_len).min(rect.right() - 50.0);
            painter.line_segment(
                [egui::pos2(x, y), egui::pos2(end_x, y)],
                egui::Stroke::new(0.5_f32, dim_color),
            );
            x += dash_len + gap_len;
        }
        painter.text(
            egui::pos2(rect.right() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{label} {z:.2}"),
            egui::FontId::proportional(8.0),
            dim_color,
        );
    }

    let entry_color = crate::ui::tokens::DIAGRAM_INK;
    let stroke = egui::Stroke::new(2.0_f32, entry_color);
    let cx = rect.center().x;

    match dressups.entry_style {
        DressupEntryStyle::Ramp => {
            let angle_rad = (dressups.ramp_angle as f32).to_radians();
            let ramp_horiz = z_drop as f32 / angle_rad.tan().max(0.01);

            // Scale horizontal distance to fit canvas
            let available_w = rect.width() * 0.6;
            let h_scale = available_w / ramp_horiz.max(1.0);
            let v_height = (top_y - feed_y).abs();
            let h_pixels = ramp_horiz * h_scale.min(1.0);

            let start_x = cx - h_pixels / 2.0;
            let end_x = cx + h_pixels / 2.0;

            // Ramp line
            painter.add(egui::Shape::line(
                vec![egui::pos2(start_x, feed_y), egui::pos2(end_x, top_y)],
                stroke,
            ));

            // Angle arc annotation
            let arc_r = 20.0_f32;
            let mut arc_pts = Vec::with_capacity(12);
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let a = -angle_rad * t;
                arc_pts.push(egui::pos2(
                    start_x + arc_r * a.cos(),
                    feed_y - arc_r * a.sin(),
                ));
            }
            painter.add(egui::Shape::line(
                arc_pts,
                egui::Stroke::new(1.0_f32, entry_color),
            ));
            painter.text(
                egui::pos2(start_x + arc_r + 4.0, feed_y - 8.0),
                egui::Align2::LEFT_CENTER,
                format!("{:.1}\u{00B0}", dressups.ramp_angle),
                egui::FontId::proportional(9.0),
                entry_color,
            );

            // Entry point marker
            painter.circle_filled(egui::pos2(end_x, top_y), 3.0, entry_color);

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Ramp Entry",
                egui::FontId::proportional(10.0),
                entry_color,
            );

            let _ = v_height;
        }
        DressupEntryStyle::Helix => {
            let Some(radius) = helix_radius_mm else {
                return;
            };
            let pitch = dressups.helix_pitch;
            let turns = z_drop / pitch.max(0.01);

            // Side view of helix: sinusoidal wave descending
            let total_angle = turns * std::f64::consts::TAU;
            let steps = (turns * 32.0).clamp(32.0, 200.0) as usize;

            // Scale radius to fit canvas
            let available_w = rect.width() * 0.5;
            let r_pixels = (radius as f32 * available_w / (radius as f32 * 2.0).max(1.0))
                .min(available_w / 2.0);

            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let angle = total_angle * t;
                let z = feed_z - z_drop * t;
                let x_off = (radius * angle.cos()) as f32 * (r_pixels / radius.max(0.01) as f32);
                pts.push(egui::pos2(cx + x_off, z_to_y(z)));
            }
            painter.add(egui::Shape::line(pts, stroke));

            // Entry point marker
            painter.circle_filled(egui::pos2(cx, top_y), 3.0, entry_color);

            // Radius annotation
            painter.line_segment(
                [
                    egui::pos2(cx, z_to_y(feed_z)),
                    egui::pos2(cx + r_pixels, z_to_y(feed_z)),
                ],
                egui::Stroke::new(1.0_f32, dim_color),
            );
            painter.text(
                egui::pos2(cx + r_pixels / 2.0, z_to_y(feed_z) - 8.0),
                egui::Align2::CENTER_BOTTOM,
                format!("r={radius:.1}"),
                egui::FontId::proportional(8.0),
                dim_color,
            );

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                format!("Helix Entry ({turns:.1} turns)"),
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
        DressupEntryStyle::None => {
            // Vertical plunge arrow
            painter.line_segment([egui::pos2(cx, feed_y), egui::pos2(cx, top_y)], stroke);
            // Arrowhead
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(cx - 4.0, top_y - 8.0),
                    egui::pos2(cx, top_y),
                    egui::pos2(cx + 4.0, top_y - 8.0),
                ],
                stroke,
            ));
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Direct Plunge",
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::{chipload_of, write_speed_edit};
    use crate::state::toolpath::{OperationConfig, OperationType, ToolpathEntry};
    use rs_cam_core::feeds::{FeedsField, FeedsLock, ProvenanceSource, SpeedEdit};

    const FLUTES: u32 = 2;

    /// A Pocket at feed 1000 mm/min and 18 000 RPM, with every feed field
    /// stamped by Suggest, and its stored config.
    fn pocket() -> (rs_cam_core::session::ToolpathConfig, ToolpathEntry) {
        let mut operation = OperationConfig::new_default(OperationType::Pocket);
        operation.set_feed_rate(1000.0);
        operation.set_spindle_rpm(Some(18_000));
        let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
        provenance.set(
            FeedsField::FeedRate,
            rs_cam_core::feeds::ValueProvenance::formula(),
        );
        provenance.set(
            FeedsField::SpindleRpm,
            rs_cam_core::feeds::ValueProvenance::formula(),
        );
        let stored = rs_cam_core::session::ToolpathConfig {
            shoe: Default::default(),
            feeds_lock: Default::default(),
            id: rs_cam_core::compute::ToolpathId::from_raw(0),
            name: "Pocket".to_owned(),
            enabled: true,
            operation: operation.clone(),
            dressups: Default::default(),
            heights: Default::default(),
            tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
            model_id: rs_cam_core::ids::ModelId::from_raw(0),
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            stock_source: Default::default(),
            coolant: Default::default(),
            debug_options: Default::default(),
            feeds_provenance: provenance.clone(),
            rest_analysis: Default::default(),
            planner_origin: None,
        };
        let mut entry = ToolpathEntry::new(
            stored.id,
            stored.name.clone(),
            rs_cam_core::ids::ToolId::from_raw(1),
            stored.model_id,
            operation,
        );
        entry.feeds_provenance = provenance;
        (stored, entry)
    }

    /// The ranges of the two rows, wide enough for every case below.
    const RANGES: rs_cam_core::feeds::SpeedRanges = rs_cam_core::feeds::SpeedRanges {
        feed: (1.0, 50_000.0),
        rpm: (1_000.0, 60_000.0),
    };

    /// One edit under `lock` through the tab's write, then the write-back:
    /// the projected config (ONE `ReplaceToolpathConfig`, so one undo step).
    fn edit_under(
        lock: FeedsLock,
        edit: SpeedEdit,
    ) -> (
        rs_cam_core::session::ToolpathConfig,
        rs_cam_core::session::ToolpathConfig,
    ) {
        let (stored, mut entry) = pocket();
        entry.feeds_lock = lock;
        write_speed_edit(&mut entry, edit, Some(FLUTES), &RANGES);
        let config = crate::ui::properties::project_entry_onto(&stored, &entry);
        (stored, *config)
    }

    fn source(
        config: &rs_cam_core::session::ToolpathConfig,
        field: FeedsField,
    ) -> Option<ProvenanceSource> {
        config.feeds_provenance.get(field).map(|p| p.source)
    }

    /// W3k: every lock against every edit. The edited field takes the typed
    /// value, the dependent field moves by `feed = chipload x RPM x flutes`,
    /// the locked one holds, and both written fields are Manual. The start
    /// is feed 1000, 18 000 RPM, 2 flutes: chipload 1/36.
    #[test]
    fn each_lock_moves_the_dependent_field_and_holds_the_locked_one_w3k() {
        let cl = 1000.0 / (18_000.0 * f64::from(FLUTES));
        // (lock, edit, feed after, rpm after, the fields written)
        let cases: [(FeedsLock, SpeedEdit, f64, u32, &[FeedsField]); 6] = [
            // Chipload locked: a feed edit moves the RPM; an RPM edit the feed.
            (
                FeedsLock::Chipload,
                SpeedEdit::Feed(2000.0),
                2000.0,
                36_000,
                &[FeedsField::FeedRate, FeedsField::SpindleRpm],
            ),
            (
                FeedsLock::Chipload,
                SpeedEdit::Spindle(9000.0),
                500.0,
                9000,
                &[FeedsField::FeedRate, FeedsField::SpindleRpm],
            ),
            // Feed locked: an RPM edit moves the chipload; a chipload edit
            // moves the RPM.
            (
                FeedsLock::Feed,
                SpeedEdit::Spindle(12_000.0),
                1000.0,
                12_000,
                &[FeedsField::SpindleRpm],
            ),
            (
                FeedsLock::Feed,
                SpeedEdit::Chipload(0.05),
                1000.0,
                10_000,
                &[FeedsField::SpindleRpm],
            ),
            // Spindle locked: a feed edit moves the chipload; a chipload edit
            // moves the feed (the W3i behaviour).
            (
                FeedsLock::Spindle,
                SpeedEdit::Feed(1500.0),
                1500.0,
                18_000,
                &[FeedsField::FeedRate],
            ),
            (
                FeedsLock::Spindle,
                SpeedEdit::Chipload(0.05),
                1800.0,
                18_000,
                &[FeedsField::FeedRate],
            ),
        ];
        for (lock, edit, feed, rpm, written) in cases {
            let (_, config) = edit_under(lock, edit);
            let got_feed = config.operation.feed_rate().expect("a feed");
            assert!(
                (got_feed - feed).abs() < 1e-9,
                "{lock:?} {edit:?}: feed {got_feed}, not {feed}"
            );
            assert_eq!(
                config.operation.spindle_rpm(),
                Some(rpm),
                "{lock:?} {edit:?}: rpm"
            );
            for field in [FeedsField::FeedRate, FeedsField::SpindleRpm] {
                let manual = source(&config, field) == Some(ProvenanceSource::Manual);
                assert_eq!(
                    manual,
                    written.contains(&field),
                    "{lock:?} {edit:?}: {field:?} Manual = {manual}; a written field is \
                     the operator's value, a held one keeps its Suggest stamp"
                );
            }
            assert_eq!(config.feeds_lock, lock, "the write-back carries the lock");
            // The locked value holds.
            match lock {
                FeedsLock::Feed => assert!((got_feed - 1000.0).abs() < 1e-9),
                FeedsLock::Spindle => assert_eq!(config.operation.spindle_rpm(), Some(18_000)),
                FeedsLock::Chipload => {
                    let after = chipload_of(&config.operation, FLUTES).expect("a chipload");
                    assert!(
                        (after - cl).abs() < 1e-9,
                        "{edit:?}: chipload {after}, not {cl}"
                    );
                }
            }
        }
    }

    /// Feeds contract P21b: the RPM ceiling row draws under Constant chip or
    /// while a ceiling is set, and a frame with no input sends no event (a
    /// passive frame must not write the post block).
    #[test]
    fn the_rpm_ceiling_row_draws_under_constant_chip_or_a_set_value_p21b() {
        use rs_cam_core::feeds::SpindleStrategyKind;
        let machine = rs_cam_core::machine::MachineProfile::default();
        let ceiling = Some(rs_cam_core::gcode::PostConfig::default().spindle_speed);
        for (kind, rpm_ceiling, drawn) in [
            (SpindleStrategyKind::MatchChart, None, false),
            (SpindleStrategyKind::MatchChart, ceiling, true),
            (SpindleStrategyKind::ConstantChip, None, true),
            (SpindleStrategyKind::ConstantChip, ceiling, true),
        ] {
            let ctx = egui::Context::default();
            crate::ui::tokens::apply(&ctx);
            crate::ui::tokens::apply_fonts(&ctx);
            let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
            warmup.textures_delta.clear();
            let mut events = Vec::new();
            let keys = super::super::placement::record_drawn(|| {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    super::draw_spindle_policy_row(ui, kind, rpm_ceiling, &machine, &mut events);
                });
                out.textures_delta.clear();
            });
            assert_eq!(
                keys.contains("spindle_rpm_ceiling"),
                drawn,
                "{kind:?} with {rpm_ceiling:?}"
            );
            assert!(
                events.is_empty(),
                "{kind:?} with {rpm_ceiling:?}: a passive frame sent {} event(s)",
                events.len()
            );
        }
    }

    /// W3k: the default lock is the chipload, on a new entry and in the
    /// core config.
    #[test]
    fn the_default_lock_is_the_chipload_w3k() {
        let (stored, entry) = pocket();
        assert_eq!(entry.feeds_lock, FeedsLock::Chipload);
        assert_eq!(stored.feeds_lock, FeedsLock::Chipload);
    }

    /// W3k: a lock change alone is one config write that moves no value.
    #[test]
    fn a_lock_change_writes_the_lock_only_w3k() {
        let (stored, mut entry) = pocket();
        let before = crate::ui::properties::project_entry_onto(&stored, &entry);
        entry.feeds_lock = FeedsLock::Feed;
        let config = crate::ui::properties::project_entry_onto(&stored, &entry);
        assert_eq!(config.feeds_lock, FeedsLock::Feed);
        assert_eq!(config.operation.feed_rate(), Some(1000.0));
        assert_eq!(config.operation.spindle_rpm(), Some(18_000));
        assert_eq!(
            config.generation_inputs_signature(),
            before.generation_inputs_signature(),
            "the lock moves no motion"
        );
    }

    /// W3i: an edit of the spindle speed changes the Chipload readout and
    /// not the feed.
    #[test]
    fn an_rpm_edit_moves_the_chipload_not_the_feed_w3i() {
        let (_, mut entry) = pocket();
        let before = chipload_of(&entry.operation, FLUTES).expect("a chipload");
        assert!((before - 1000.0 / (18_000.0 * 2.0)).abs() < 1e-12);
        entry.operation.set_spindle_rpm(Some(12_000));
        let after = chipload_of(&entry.operation, FLUTES).expect("a chipload");
        assert!((after - 1000.0 / (12_000.0 * 2.0)).abs() < 1e-12);
        assert_eq!(entry.operation.feed_rate(), Some(1000.0), "the feed moved");
    }

    /// With no spindle speed the chipload has no value, and a chipload edit
    /// under the default lock writes nothing.
    #[test]
    fn no_rpm_no_chipload_w3i() {
        let (_, mut entry) = pocket();
        entry.operation.set_spindle_rpm(None);
        assert_eq!(chipload_of(&entry.operation, FLUTES), None);
        write_speed_edit(&mut entry, SpeedEdit::Chipload(0.05), Some(FLUTES), &RANGES);
        assert_eq!(entry.operation.feed_rate(), Some(1000.0));
    }
}
