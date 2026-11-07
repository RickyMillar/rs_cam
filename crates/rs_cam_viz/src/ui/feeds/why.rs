//! Why — the sentences that explain one recommended number.
//!
//! This file used to draw a panel: a `Why is the recommendation here?`
//! disclosure holding provenance, then rationale, then a derate chain, then
//! a result, then warnings. The operator's verdict on 2026-09-15 was that it
//! is "all just too much", and that what they actually want is: *if a
//! recommended value differs from mine, let me hover it and see why.*
//!
//! They were right, and the reason is structural rather than a matter of
//! length. The disclosure explained **the recommendation as a whole**. The
//! operator reads the card **one row at a time**, and the question that
//! arises is always about one row — *why is my DOC being tripled?* A
//! whole-recipe explanation cannot answer a per-row question however short
//! it is, and the declutter earlier that day (44 painted runs down to 13)
//! proved it: shorter, still not an answer.
//!
//! So this is no longer a panel. It is a library of sentences, one per row
//! of the comparison card, and `compare.rs` hangs each one on the row it
//! explains. See `planning/feeds_rework_2026-09-15/PLAN.md` W2.
//!
//! Four things deliberately stay rows of their own rather than moving to a
//! hover. Round 2 (2026-10-13) gives each one a [`Placement`]: a problem is
//! one status line on the face, and derivation is a row in the card's
//! `Details`, which is closed by default.
//!
//! - **Warnings** ([`warning_rows`]). A warning behind a hover is a warning
//!   that was deleted, so a problem stays on the face.
//! - **The ramp record and the entry notes** (G6 ramp and G10,
//!   2026-09-25): one row from the funnel's `SuggestWarning::RampFeed`.
//!   The card has no ramp row, so the line names the ramp feed, the arm
//!   that set it (the G6 chip, the plunge slope, or no entry feed) and θ.
//!   Under it, one row per entry note: the ramp angle or the helix, the
//!   core or the pip, the clearance and the straight-plunge caution
//!   ([`suggest_rows`]).
//! - **The engaged-diameter row** ([`engaged_diameter_row`]), for tapered
//!   and V tools, where the published tip size understates what cuts.
//! - **The row basis** ([`row_basis_rows`]): the G10 plunge basis
//!   (the claim or the named repo rule), the V-bit lookup key (ruling B4),
//!   the G1 size claim of an off-size row, the G2 soft/hard cap of a capped
//!   hardness transfer, and the printed material label of a derived row.
//!
//! Every quantity here is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)`. This surface runs before a simulation and has no
//! measured value to show. The achieved figure lives on the properties
//! panel's operating-point card, after a sim (Checkpoint H2, 2026-08-08).

use rs_cam_core::feeds::extrapolation::{Claim, HardnessBasis};
use rs_cam_core::feeds::rationale::SuggestRationale;
use rs_cam_core::feeds::suggest::{FeedRecalibrationCap, SuggestWarning};
use rs_cam_core::feeds::vendor_lookup::vbit_key_text;
use rs_cam_core::feeds::vendor_lut::ObservationKind;
use rs_cam_core::feeds::{
    FeedsExplain, FeedsField, PlungeBasis, RampFeed, SpindleScaleReason, ToolGeometryHint,
};

use super::shared::{
    AppliedRecipe, CurrentValues, engaged_diameter_context, material_family_label, vendor_band,
    vendor_single_value,
};
use crate::ui::components::{DataRow, ProvKind, Role, Source};
use crate::ui::{theme, tokens};

/// A factor within this of 1.0 changed nothing.
const UNITY_TOLERANCE: f64 = 1e-3;

/// One row of the comparison card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecipeRow {
    Rpm,
    Feed,
    Plunge,
    Doc,
    Woc,
    Advance,
}

impl RecipeRow {
    /// The keywords that route a rationale entry to this row.
    ///
    /// The Suggest pass emits its own account of what it did — "adaptive3d
    /// depth_per_pass clamped 9.00 → 4.20 mm (vendor_ap)". That sentence is
    /// about ONE row, and it belongs on that row. An entry that matches no
    /// row is recipe-level and lands on [`RecipeRow::Advance`], which is the
    /// row that describes the recipe itself.
    fn rationale_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rpm => &["rpm", "spindle"],
            Self::Feed => &["feed"],
            Self::Plunge => &["plunge"],
            Self::Doc => &["depth_per_pass", "dpp", "axial", "doc", "depth"],
            Self::Woc => &["stepover", "radial", "woc", "scallop"],
            Self::Advance => &[],
        }
    }
}

/// One derivation record: one full-width row, with its explanation on the
/// row hover.
///
/// IC1 and round 2: the row never wraps. A long record truncates on the
/// face; the hover gives the full record and then the explanation.
fn record(text: impl Into<String>, color: egui::Color32, hover: &str) -> DataRow {
    DataRow::record(text).tone(color).hover(hover)
}

/// One caution: a status line for a problem, in the caution colour.
fn caution(text: impl Into<String>) -> DataRow {
    DataRow::note(Role::Caution, text)
}

// ── The per-row explanations ─────────────────────────────────────────

/// The sentences that explain one recommended value.
///
/// Returns the hover text for `row`. Always non-empty: a row that did not
/// move still answers "why is this NOT moving", which is the same question
/// inverted and is asked just as often.
///
/// G-RECOMAPPLIED (2026-09-24): the row prints what `⚡ Apply all` writes
/// (`applied`). The first line of the hover is that value. When the funnel
/// moved it, one sentence gives the raw calculator value and the cause.
pub(crate) fn row_explanation(
    row: RecipeRow,
    current: &CurrentValues,
    explain: &FeedsExplain,
    applied: &AppliedRecipe,
    rationale: Option<&SuggestRationale>,
) -> String {
    let mut out = String::new();
    match row {
        RecipeRow::Rpm => explain_rpm(&mut out, explain, applied),
        RecipeRow::Feed => explain_feed(&mut out, current, explain, applied),
        RecipeRow::Plunge => explain_plunge(&mut out, explain, applied),
        RecipeRow::Doc => explain_doc(&mut out, explain, applied),
        RecipeRow::Woc => explain_woc(&mut out, current, explain, applied),
        RecipeRow::Advance => explain_advance(&mut out, explain, applied),
    }
    append_rationale(&mut out, row, rationale);
    out
}

/// The first lines of a row hover: the value Apply writes and, when the
/// funnel moved it, the raw calculator value and the cause.
///
/// `decimals` is the precision the row prints at. A value that prints the
/// same as the calculator value did not move, and gets no sentence.
fn applied_lines(
    out: &mut String,
    applied: &AppliedRecipe,
    field: FeedsField,
    calculator: f64,
    unit: &str,
    decimals: usize,
) {
    let written = applied.value(field, calculator);
    let shown = format!("{written:.decimals$}{unit}");
    let raw = format!("{calculator:.decimals$}{unit}");
    out.push_str(&format!("{shown}.\n"));
    if shown == raw {
        return;
    }
    out.push_str(&format!(
        "calculator {raw}; the invariant funnel changes it, so Apply writes {shown}.\n"
    ));
}

fn explain_rpm(out: &mut String, explain: &FeedsExplain, applied: &AppliedRecipe) {
    applied_lines(
        out,
        applied,
        FeedsField::SpindleRpm,
        explain.recommended.rpm,
        " RPM",
        0,
    );
    match &explain.matched_row {
        Some(row) => match row.rpm_min.zip(row.rpm_max) {
            Some((lo, hi)) => out.push_str(&format!(
                "Vendor row {} publishes {lo:.0}–{hi:.0} RPM.\n",
                row.observation_id
            )),
            None => match row.rpm_nominal {
                Some(nom) => out.push_str(&format!(
                    "Vendor row {} publishes {nom:.0} RPM nominal.\n",
                    row.observation_id
                )),
                None => out.push_str("The vendor row publishes no RPM; this is the formula's.\n"),
            },
        },
        None => out.push_str("No vendor row matched. This is the empirical formula's RPM.\n"),
    }
    // The sentence comes from the RECORDED reason, never from the factor.
    // Until 2026-09-16 this read the factor alone and said "MaxSpeed lifted
    // it" for anything that was not 1.0 — which was true only while MaxSpeed
    // was the one thing that could move it. The power ladder walks the same
    // line downward, so a factor-only reading would name the wrong cause.
    let scale = explain.recommended.derates.spindle_scale;
    let reason = explain.recommended.derates.spindle_scale_reason;
    match reason {
        SpindleScaleReason::MaxSpeedPolicy => out.push_str(&format!(
            "Spindle policy MaxSpeed lifted it ×{scale:.3} toward the spindle \
             ceiling, and scaled the feed with it to hold the advance/tooth \
             constant.\n"
        )),
        SpindleScaleReason::PowerLimit => out.push_str(&format!(
            "The cut was over the spindle's power budget, so the RPM came DOWN \
             ×{scale:.3} and the feed came down with it. The advance per tooth \
             is unchanged — the cut is the same shape, just slower.\n"
        )),
        SpindleScaleReason::FeedCeiling => out.push_str(&format!(
            "The feed hit the machine's cutting-feed ceiling, so the RPM came \
             DOWN ×{scale:.3} and the feed came down with it. The advance per \
             tooth stays at the band value (ruling R4 Q10).\n"
        )),
        SpindleScaleReason::Unchanged => {
            if (scale - 1.0).abs() > UNITY_TOLERANCE {
                // A factor that moved with no reason recorded is a bug in the
                // calculator, not something to narrate over.
                out.push_str(&format!(
                    "The RPM was scaled ×{scale:.3} and the engine did not \
                     record why. Treat this recommendation as unexplained.\n"
                ));
            } else {
                out.push_str("Spindle policy is Match chart, so the RPM follows the vendor row.\n");
            }
        }
    }
}

fn explain_feed(
    out: &mut String,
    current: &CurrentValues,
    explain: &FeedsExplain,
    applied: &AppliedRecipe,
) {
    let r = &explain.recommended;
    let flutes = current.flute_count.max(1);
    let advance = if r.rpm > 0.0 {
        r.feed_rate_mm_min / (r.rpm * f64::from(flutes))
    } else {
        0.0
    };
    applied_lines(
        out,
        applied,
        FeedsField::FeedRate,
        r.feed_rate_mm_min,
        " mm/min",
        0,
    );
    out.push_str(&format!(
        "calculator feed = advance/tooth × RPM × flutes = {advance:.4} × {:.0} × {flutes}.\n",
        r.rpm
    ));
    let d = &r.derates;
    if d.power_limit < 0.999 {
        out.push_str(&format!(
            "The spindle could not deliver the power, so the feed was cut ×{:.3}.\n",
            d.power_limit
        ));
    }
    if d.feed_clamp < 0.999 {
        out.push_str(&format!(
            "The feed hit the machine cutting-feed ceiling and was clamped ×{:.3}.\n",
            d.feed_clamp
        ));
    }
    out.push_str("Change the advance/tooth to change this; see that row.\n");
}

fn explain_plunge(out: &mut String, explain: &FeedsExplain, applied: &AppliedRecipe) {
    applied_lines(
        out,
        applied,
        FeedsField::PlungeRate,
        explain.recommended.plunge_rate_mm_min,
        " mm/min",
        0,
    );
    // G10 (2026-09-25): the plunge basis states its claim or its named
    // rule. Ruling R4 Q5: the dial has no lever on a plunge.
    let (headline, detail) = explain.recommended.plunge.card_text();
    out.push_str(&headline);
    out.push('\n');
    out.push_str(&detail);
    out.push('\n');
}

fn explain_doc(out: &mut String, explain: &FeedsExplain, applied: &AppliedRecipe) {
    applied_lines(
        out,
        applied,
        FeedsField::DepthPerPass,
        explain.recommended.axial_depth_mm,
        " mm",
        2,
    );
    applied_note(out, applied, FeedsField::DepthPerPass);
}

fn explain_woc(
    out: &mut String,
    current: &CurrentValues,
    explain: &FeedsExplain,
    applied: &AppliedRecipe,
) {
    applied_lines(
        out,
        applied,
        FeedsField::Stepover,
        explain.recommended.radial_width_mm,
        " mm",
        2,
    );
    if current.supports_scallop_override && current.scallop_height.is_some() {
        out.push_str(
            "This stepover is derived from your target scallop height and the \
             tool's ball-tip radius, not from the vendor row.\n",
        );
    }
    applied_note(out, applied, FeedsField::Stepover);
}

fn explain_advance(out: &mut String, explain: &FeedsExplain, applied: &AppliedRecipe) {
    let r = &explain.recommended;
    let d = &r.derates;
    let flutes = explain.flute_count.max(1) as f64;
    // The advance the applied feed and RPM give (G-RECOMAPPLIED), which is
    // the value the row prints.
    let feed = applied.value(FeedsField::FeedRate, r.feed_rate_mm_min);
    let rpm = applied.value(FeedsField::SpindleRpm, r.rpm);
    let effective = if rpm > 0.0 {
        feed / (rpm * flutes)
    } else {
        0.0
    };
    out.push_str(&format!("{effective:.4} mm/tooth.\n"));

    // Where the target came from.
    match (&r.chipload_source, &d.formula) {
        (rs_cam_core::feeds::ChiploadSource::VendorLut { observation_id }, _) => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth — the midpoint of vendor row {observation_id}.\n",
                d.target_chip_load_mm
            ));
        }
        (rs_cam_core::feeds::ChiploadSource::FormulaFallback, Some(f))
        | (rs_cam_core::feeds::ChiploadSource::EdgeRadiusFloor, Some(f)) => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth — no vendor row matched, so the empirical \
                 formula set it: fz = K₀·D^p·(1/H)^q = {:.4}·{:.2}^{:.2}·(1/{:.1})^{:.2}.\n",
                f.result_mm_tooth, f.k0, f.diameter_mm, f.p, f.feed_scale_factor, f.q
            ));
        }
        _ => {
            out.push_str(&format!(
                "Target {:.4} mm/tooth before the factors below.\n",
                d.target_chip_load_mm
            ));
        }
    }

    // What moved it.
    let combined = d.combined_factor();
    let pct = ((1.0 - combined) * 100.0).max(0.0);
    let mut moved: Vec<String> = Vec::new();
    // Ruling R4 (2026-09-24): the machine safety factor is gone, and the
    // long-tool share is a load target, not a feed factor (Q7). Neither is
    // in this list; the long-tool line on the card states it. The
    // workholding factor is gone (Q8), and the dial is gone (D9).
    // P11b (L1): the radial chip-thinning lift raises the feed so that the
    // peak chip is the target; the power limit and the feed cap lower it.
    for (label, value) in [
        ("chip thinning", d.radial_chip_thinning),
        ("power limit", d.power_limit),
        ("feed cap", d.feed_clamp),
    ] {
        if (value - 1.0).abs() > UNITY_TOLERANCE {
            moved.push(format!("{label} ×{value:.3}"));
        }
    }
    if moved.is_empty() {
        out.push_str("Nothing scaled it.\n");
    } else if combined >= 1.0 {
        out.push_str(&format!("Scaled ×{combined:.3} by {}.\n", moved.join(", ")));
    } else {
        out.push_str(&format!(
            "Derated ×{combined:.3} ({pct:.0}% reduction) by {}.\n",
            moved.join(", ")
        ));
    }

    if d.ld_overhang < 1.0 - UNITY_TOLERANCE {
        out.push_str(&format!(
            "The long-tool share ×{:.2} does not change the feed.\n",
            d.ld_overhang
        ));
    }

    // P11b (L1): the radial lift is applied (the line above). The axial
    // term of a ball at a shallow depth stays observed, not applied.
    if d.observed_axial_chip_thinning > 1.001 {
        out.push_str(&format!(
            "Axial chip thinning ×{:.3} is OBSERVED, NOT APPLIED: the vendor \
             charts give no rule for a depth below 1 × D.\n",
            d.observed_axial_chip_thinning
        ));
    }

    // Where it landed. A band only when the row publishes both limits
    // (G-CHARTLINES); a one-value row names its one value as a point (A2)
    // and claims no band.
    //
    // P11b (L1): the band holds the PEAK chip, the advance over the
    // chip-thinning lift.
    let lift = d.radial_chip_thinning.max(1.0);
    let held = effective / lift;
    let which = if lift > 1.0 + UNITY_TOLERANCE {
        format!("the peak chip {held:.4}")
    } else {
        "this".to_owned()
    };
    if effective > 0.0 {
        if let Some((lo, hi)) = vendor_band(explain) {
            out.push_str(&format!(
                "Vendor band {lo:.4}–{hi:.4}; {which} sits at {:.0}% of the maximum.\n",
                (held / hi) * 100.0
            ));
        } else if let Some(value) = vendor_single_value(explain) {
            out.push_str(&format!(
                "Vendor value {value:.4} (one printed value, held as a point; no \
                 band); {which} sits at {:.0}% of it.\n",
                (held / value) * 100.0
            ));
        }
    }
}

/// The Suggest pass's own account of what it did, routed to its row.
fn append_rationale(out: &mut String, row: RecipeRow, rationale: Option<&SuggestRationale>) {
    let Some(rationale) = rationale else {
        return;
    };
    for entry in &rationale.entries {
        let headline = entry.headline.to_lowercase();
        let mine = if row == RecipeRow::Advance {
            // The catch-all: an entry that names no row is recipe-level.
            !RECIPE_ROWS
                .iter()
                .filter(|candidate| **candidate != RecipeRow::Advance)
                .any(|candidate| {
                    candidate
                        .rationale_keywords()
                        .iter()
                        .any(|k| headline.contains(k))
                })
        } else {
            row.rationale_keywords()
                .iter()
                .any(|k| headline.contains(k))
        };
        if !mine {
            continue;
        }
        out.push_str(&format!("\n• {}", entry.headline));
        if let Some(detail) = &entry.detail {
            out.push_str(&format!("\n  {detail}"));
        }
    }
}

const RECIPE_ROWS: [RecipeRow; 6] = [
    RecipeRow::Rpm,
    RecipeRow::Feed,
    RecipeRow::Plunge,
    RecipeRow::Doc,
    RecipeRow::Woc,
    RecipeRow::Advance,
];

/// Say where the row gets its value, when the funnel writes `field`. On a
/// refused pairing Apply writes nothing, and the row shows the calculator
/// value, so the note does not apply.
fn applied_note(out: &mut String, applied: &AppliedRecipe, field: FeedsField) {
    if applied.written(field).is_some() {
        out.push_str(APPLIED_NOTE);
    }
}

/// Where the DOC and WOC rows get their value (G-RECOMAPPLIED).
const APPLIED_NOTE: &str = "The row shows what `⚡ Apply all` writes: the calculator value \
     after the invariant funnel (the rigidity cap).";

// ── What stays on the page ───────────────────────────────────────────

/// Where a row of the Feeds card goes: the default view or `Details`.
pub(crate) enum Placement {
    /// A problem: one status line in the default view.
    Problem(DataRow),
    /// Derivation or provenance: a row inside `Details`.
    Detail(DataRow),
}

/// S2 — the engaged diameter at the DOC, for tapered-ball and V-bit tools,
/// where the published tip size understates what cuts at depth.
///
/// A flagged diameter is a problem; else the row is derivation.
pub(crate) fn engaged_diameter_row(
    current: &CurrentValues,
    explain: &FeedsExplain,
) -> Option<Placement> {
    let (doc, engaged, tip_dia, kind) = engaged_diameter_context(current, explain)?;
    // Flag when the engaged diameter has hit the shank, or differs from
    // the published tip by more than 50 % — i.e. the cone shoulder is
    // doing most of the work and the tip spec is misleading.
    let flag = engaged >= explain.shank_diameter_mm - 1e-6
        || (tip_dia > 1e-6 && (engaged - tip_dia).abs() / tip_dia > 0.5);
    let hover = engaged_diameter_hover(explain.tool_geometry, kind, tip_dia);
    let face = format!("Engaged Ø {engaged:.2} mm at DOC {doc:.2} mm");
    Some(if flag {
        Placement::Problem(caution(format!("⚠ {face}")).hover(hover))
    } else {
        Placement::Detail(record(face, theme::TEXT_STRONG, &hover))
    })
}

/// The hover of the engaged-diameter row. Ruling A1 (2026-09-24) keys a
/// tapered-ball row at the tip; ruling B4 (2026-09-25) keys a V-bit row at
/// the nominal diameter, not the engaged width, so the two cone tools need
/// two texts.
fn engaged_diameter_hover(geometry: ToolGeometryHint, kind: &str, tip_dia: f64) -> String {
    let opening = format!(
        "The published {kind} Ø is {tip_dia:.2} mm, but the cone shoulder does most of the \
         cutting at this depth."
    );
    match geometry {
        ToolGeometryHint::TaperedBall { .. } => format!(
            "{opening} Suggest reads the vendor row at the tip Ø (ruling A1). The RPM, the \
             depth ladder, the band's depth derate and the deflection check use the engaged Ø."
        ),
        ToolGeometryHint::VBit { .. } => format!(
            "{opening} Suggest reads the vendor row, the depth ladder and the band's depth \
             derate at the nominal Ø (ruling B4), not this engaged Ø. The RPM, the chip-load \
             formula and the deflection check still use the engaged Ø."
        ),
        ToolGeometryHint::Flat | ToolGeometryHint::Ball | ToolGeometryHint::Bull { .. } => format!(
            "{opening} Suggest reads the vendor row at the engaged Ø, and every advance/tooth \
             figure on this surface uses the engaged Ø."
        ),
    }
}

/// The basis of the matched row, as rows of the Feeds card's `Details`
/// (extrapolation P1 step 4, P2 step 4; round 2 moved them from the face
/// into `Details`, where "a visible line" below means a row).
///
/// Operator rule: no invisible calculation. The G10 plunge basis
/// (`FeedsResult::plunge`) is the first line: the claim headline in the
/// claim tone, or the named repo rule dimmed; the rule, the witnesses and
/// the Q12 sentence are on its hover. On a V-bit query the row's key
/// (`vendor_lookup::vbit_key_text`, ruling B4) is a visible line: the
/// printed cutting diameter, or the printed included angle when the chart
/// prints no diameter. When the row is off-size, the G1 size claim
/// (`feeds::extrapolation::Claim`) moved the band. Its headline (the scale
/// and the anchor row) is a visible line. Its detail (the rule, the range
/// and the spread) and the hardness scale are on the hover. When the
/// soft/hard cap stopped the hardness transfer (`HardnessBasis::Capped`),
/// the cap headline is a visible line and the cap detail (the printed
/// ratio and the law's value) is on its hover. When
/// a G3 family rule carries the row from its home operation family
/// (`FamilyBasis::Transferred`, A3), the family headline is a visible line
/// and the rule, the row and the served families are on its hover. When
/// the G6 drill rule reads the row (`DrillBasis::Transferred`, ruling B5),
/// the drill headline is a visible line and the rule, the RPM cap and the
/// peck rule are on its hover. When
/// the row is `Derived` (ruling A4: a "Wood, MDF, Sign-Foam" row that
/// serves hardwood), the row's printed material label is a visible line.
/// When the cell is a `ProjectCurve` on a V-bit (operator ruling
/// 2026-09-25), the row is a printed Trace row reached by a route, not the
/// operation's own declared family — that route is a visible line too.
pub(crate) fn row_basis_rows(explain: &FeedsExplain) -> Vec<DataRow> {
    let mut rows = Vec::new();
    // G10 (2026-09-25): the plunge states its claim or its named repo rule.
    // It reads the plunge basis, not the matched row: the named rule serves
    // a tool with no row too. A drill cycle has no plunge rule (the plunge is
    // the drill feed), so it paints no line. Round 2: the basis variant
    // gives the source badge; a repo rule reads as "no source".
    let plunge = &explain.recommended.plunge;
    let plunge_tone = match plunge {
        PlungeBasis::Claimed { .. } => Some((theme::WARNING_MILD, ProvKind::PublishedRule)),
        PlungeBasis::MaterialBase { .. } => Some((theme::TEXT_DIM, ProvKind::RepoRule)),
        PlungeBasis::DrillCycle { .. } => None,
    };
    if let Some((tone, kind)) = plunge_tone {
        let (headline, detail) = plunge.card_text();
        rows.push(record(headline, tone, &detail).source(Source::Prov(kind, None)));
    }
    let Some(row) = explain.matched_row.as_ref() else {
        return rows;
    };
    if explain.project_curve_vbit_routed_to_trace {
        rows.push(record(
            "ProjectCurve on a V-bit reads the V-bit trace rows",
            theme::TEXT_DIM,
            &format!(
                "Operator ruling 2026-09-25: the tool follows the curve at a set depth, the \
                 same engagement a v-carve or trace pass has, so the query is routed to the \
                 printed V-bit Trace rows instead of a made-up ProjectCurve family.\nRow: {}.",
                row.observation_id
            ),
        ));
    }
    // P16 item 8 (operator ruling D2): the vendor band. Every member is
    // named; Suggest ships the lowest. The hover names each row and its
    // size basis. To cut higher inside the band, state the chip on the
    // Chipload row (the Chipload lock).
    if let Some(band) = explain.recommended.vendor_band()
        && let Some(line) = band.card_text()
    {
        let members: Vec<String> = band
            .members
            .iter()
            .map(|m| {
                format!(
                    "{} {:.4} mm/tooth ({}, {})",
                    m.vendor,
                    m.chip_mid_mm,
                    m.row.id(),
                    m.size_basis.name()
                )
            })
            .collect();
        rows.push(record(
            line,
            theme::TEXT_DIM,
            &format!(
                "The printed rows for this tool, size and material form a band. Suggest ships the lowest chip. To cut higher inside the band, state the chip on the Chipload row.\n{}.",
                members.join(";\n")
            ),
        ));
    }
    if let Some(key_text) = vbit_key_text(explain.query.tool_family, row) {
        rows.push(record(
            key_text,
            theme::TEXT_DIM,
            &format!(
                "Suggest reads a V-bit row at its printed key (ruling B4): the printed \
                 cutting diameter, or the printed included angle when the chart prints no \
                 diameter.\nRow: {}.",
                row.observation_id
            ),
        ));
    }
    if let Some(family) = row.family_basis.claim() {
        let (headline, detail) = family.card_text();
        rows.push(record(
            headline,
            theme::WARNING_MILD,
            &format!("{detail}.\nRow: {}.", row.observation_id),
        ));
    }
    // B5 (G6): a flat end mill plunge reads the side row's chip / Z. The
    // claim moves the chip, so its headline is a row; the rule, the RPM cap
    // and the peck rule are on its hover.
    if let Some(drill) = row.drill_basis.claim() {
        let (headline, detail) = drill.card_text();
        rows.push(record(
            headline,
            theme::WARNING_MILD,
            &format!("{detail}.\nRow: {}.", row.observation_id),
        ));
    }
    if let Some(claim) = row.size_basis.claim() {
        let (headline, detail) = claim.card_text();
        rows.push(record(
            headline,
            theme::WARNING_MILD,
            &claim_hover(claim, &detail, &row.hardness_basis),
        ));
    }
    if let Some((headline, detail)) = row.hardness_basis.card_text() {
        rows.push(record(
            headline,
            theme::WARNING_MILD,
            &format!("{detail}.\nRow: {}.", row.observation_id),
        ));
    }
    if row.row_kind == ObservationKind::Derived && !row.material_label.is_empty() {
        rows.push(record(
            format!("Row material: {}", row.material_label),
            theme::TEXT_DIM,
            &format!(
                "The vendor prints one row for the material in the label. The LUT \
                 derives this {} row ({}) from it.",
                material_family_label(explain.query.material_family),
                row.observation_id
            ),
        ));
    }
    rows
}

/// The hover of the claim line: the claim's detail, its source rows, and
/// the hardness scale that applies after the claim. A capped hardness scale
/// says so, with the law's value.
fn claim_hover(claim: &Claim, detail: &str, hardness: &HardnessBasis) -> String {
    let mut out = format!("{detail}.");
    if !claim.source_rows.is_empty() {
        out.push_str(&format!("\nRows: {}.", claim.source_rows.join(", ")));
    }
    let hardness_scale = hardness.scale();
    let capped = match hardness {
        HardnessBasis::Capped { law_scale, .. } => {
            format!(" (capped at the soft/hard cap; the Janka law gives x{law_scale:.2})")
        }
        HardnessBasis::InColumn { law_scale, .. } => format!(
            " (one chart column for the material family; the Janka law gives x{law_scale:.2}, not \
             applied)"
        ),
        HardnessBasis::Unscaled | HardnessBasis::CompositeBoard | HardnessBasis::Law { .. } => {
            String::new()
        }
    };
    out.push_str(&format!(
        "\nThe hardness scale x{hardness_scale:.2}{capped} applies after the claim; the band \
         scale is x{:.2} in total.",
        claim.scale * hardness_scale
    ));
    out
}

/// S3 — chipload-min warning. For finishing operations a chipload below
/// the vendor band minimum is the common failure mode (operator slows
/// feed for surface quality → rubbing / burning). Surface it loudly.
///
/// The warning needs a PUBLISHED minimum. A max-only row or a one-point row
/// has none, so this function draws nothing for it. Until 2026-09-23 it
/// compared against an invented `0.7 × max` floor (G-CHARTLINES).
pub(crate) fn chipload_min_row(current: &CurrentValues, explain: &FeedsExplain) -> Option<DataRow> {
    if current.pass_role != rs_cam_core::feeds::PassRole::Finish {
        return None;
    }
    let cl = current.chipload_mm();
    if cl <= 0.0 {
        return None;
    }
    let (lo, _hi) = vendor_band(explain)?;
    if cl >= lo {
        return None;
    }
    Some(
        caution("⚠ Advance/tooth is below the vendor band minimum").hover(
            "The tool rubs instead of cutting, which burns the work and the \
             cutting edge. The feed is too slow, the RPM is too high, or both. \
             Raise the feed or drop the RPM.",
        ),
    )
}

/// One warning as a headline and, where it has one, the detail behind it.
fn warning_lines(warning: &rs_cam_core::feeds::FeedsWarning) -> (String, Option<String>) {
    use rs_cam_core::feeds::FeedsWarning;
    match warning {
        FeedsWarning::FeedRateClamped { requested, actual } => (
            format!("Feed clamped: {requested:.0} → {actual:.0} mm/min (machine limit)"),
            None,
        ),
        FeedsWarning::PowerLimited {
            required_kw,
            available_kw,
        } => (
            format!("Power limited: {required_kw:.2} kW needed, {available_kw:.2} kW available"),
            None,
        ),
        FeedsWarning::PowerLadderReducedCut {
            rpm_from,
            rpm_to,
            axial_from,
            axial_to,
            radial_from,
            radial_to,
            feed_factor,
            required_kw_before,
            required_kw_after,
            available_kw,
        } => {
            let mut moved: Vec<String> = Vec::new();
            if let (Some(a), Some(b)) = (rpm_from, rpm_to) {
                moved.push(format!("RPM {a:.0} → {b:.0}"));
            }
            if let (Some(a), Some(b)) = (axial_from, axial_to) {
                moved.push(format!("depth {a:.2} → {b:.2} mm"));
            }
            if let (Some(a), Some(b)) = (radial_from, radial_to) {
                moved.push(format!("width {a:.2} → {b:.2} mm"));
            }
            if let Some(f) = feed_factor {
                moved.push(format!("feed ×{f:.3} (last resort)"));
            }
            let cleared = required_kw_after <= available_kw;
            (
                format!(
                    "Cut reduced to fit spindle power: {}{}",
                    moved.join(", "),
                    if cleared { "" } else { " — still over" }
                ),
                Some(format!(
                    "The spindle could not turn the cut you asked for. The engine made the \
                     cut SMALLER rather than slower: {required_kw_before:.2} kW → \
                     {required_kw_after:.2} kW against {available_kw:.2} kW available.\n\
                     {}{}",
                    if feed_factor.is_some() {
                        "The chip was thinned as a LAST resort, after the RPM and the cut \
                         size had already been reduced. The ploughing part of the load \
                         carries no feed term, so a slower feed sheds only part of it — \
                         which is why it is the last dial tried, not the first."
                    } else {
                        "The advance per tooth is unchanged. The cut got smaller, not \
                         thinner."
                    },
                    if cleared {
                        ""
                    } else {
                        "\nIt is STILL over budget. Take a shallower or narrower cut, or \
                         use a machine with more spindle power."
                    }
                )),
            )
        }
        FeedsWarning::DocExceedsFlute {
            requested_mm,
            guard_mm,
            shipped_mm,
        } => (
            if (shipped_mm - requested_mm).abs() < 1e-9 {
                format!("DOC {requested_mm:.1} mm past the flute guard {guard_mm:.1} mm")
            } else {
                format!("DOC capped: {requested_mm:.1} → {shipped_mm:.1} mm (flute guard)")
            },
            None,
        ),
        FeedsWarning::CuttingCapUnstated { travel_rate_mm_min } => (
            format!(
                "No cutting cap stated: travel rate {travel_rate_mm_min:.0} mm/min caps the cut"
            ),
            Some(
                "The machine states no max_cutting_feed_mm_min and no X and Y axis rate. \
                 State the cutting cap of the machine."
                    .to_owned(),
            ),
        ),
        FeedsWarning::ScallopInvalid {
            target,
            max_possible,
        } => (
            format!("Invalid scallop: {target:.3} mm (max {max_possible:.1} mm)"),
            None,
        ),
        FeedsWarning::ShankTooLarge { shank_mm, max_mm } => (
            format!("Shank {shank_mm:.1} mm exceeds max {max_mm:.1} mm"),
            None,
        ),
        // P16 (review D7): Project Curve has no depth before the simulation.
        FeedsWarning::DepthJudgedAfterSimulation => (
            "Depth, power and deflection: judged after the simulation".to_owned(),
            Some(
                "The tool follows the model surface at an offset, so the depth into the \
                 stock is known after the simulation. Suggest gives no power and no \
                 deflection figure. The feed comes from the chip load."
                    .to_owned(),
            ),
        ),
        // The headline below is asserted verbatim by
        // `inspector_width_is_tab_independent_up4.rs`: it is the real
        // warning that sentry renders to prove the tab holds a 240-point
        // rail. Keep the wording on the FACE — a hover is not painted.
        //
        // Ruling R4 WP2a (2026-09-23): the engine does not raise the feed.
        // The face line names the consequence and the two levers, because
        // the operator now owns the fix.
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            source,
            ..
        } => (
            format!(
                "Advance per tooth below the rubbing floor: {commanded:.4} mm/tooth \
                 (floor {floor:.4}, {}). The feed is not raised. The tool can \
                 rub and burn the work: raise the feed or lower the RPM.",
                source.describe()
            ),
            None,
        ),
        // Ruling R4 WP1 (2026-09-23): the long-tool de-rate is a repo rule
        // with no source. It fires on most default tools (stickout 45 mm), so
        // it is one short row with the numbers. Since ruling R4 Q7
        // (2026-09-24) it scales the load target, not the feed. Round 2: the
        // row carries the repo-rule badge, so the text does not say it.
        FeedsWarning::LongToolDerate {
            stickout_mm,
            diameter_mm,
            ratio,
            factor,
        } => (
            format!(
                "Long tool: load target ×{factor:.2} (stickout {stickout_mm:.0} mm is \
                 {ratio:.1} × Ø{diameter_mm} mm)"
            ),
            Some(
                "A repo rule with no source (ruling R4 WP1). The feed does not change.".to_owned(),
            ),
        ),
        // Ruling R4 Q10 (2026-09-24): the RPM follows the feed ceiling down
        // to hold the chip. Core owns the one text.
        FeedsWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
            arms,
        } => (
            rs_cam_core::feeds::rpm_lowered_text(
                *rpm_from,
                *rpm_to,
                *feed_ceiling_mm_min,
                *rpm_floor,
                *floor_source,
                *held,
                arms,
            ),
            None,
        ),
        // Checkpoint K (a4) — the routing refused; there is no vendor
        // row behind any number on this surface.
        FeedsWarning::NoVendorRowsForRoutedOperation {
            operation_kind,
            tool_family,
            missing_rows,
        } => (
            format!("No vendor data for {operation_kind} on a {tool_family} cutter"),
            Some(format!(
                "The recommendation is formula-derived and carries no band. \
                 Missing rows: {missing_rows}."
            )),
        ),
        // Checkpoint K (a3) — RPM-anchor row: the number above it is the
        // empirical formula's, and the nomogram's band has nothing to draw.
        FeedsWarning::VendorRowPublishesNoChipload {
            observation_id,
            formula_chipload_mm,
            floor_band_from,
        } => (
            format!("Vendor row {observation_id} publishes RPM only — no band"),
            Some(match floor_band_from {
                Some(row) => format!(
                    "The {formula_chipload_mm:.4} mm/tooth shown is the \
                     empirical formula's, and this recommendation carries no \
                     vendor band. The rubbing floor came from {row} instead, \
                     which is the chipload-bearing row the post-sim gate also \
                     resolves."
                ),
                None => format!(
                    "The {formula_chipload_mm:.4} mm/tooth shown is the \
                     empirical formula's. This recommendation carries no \
                     vendor band, and no chipload-bearing row matched either."
                ),
            }),
        ),
        // P13 item 4 (law B): the flat end mill has no printed roughing row
        // of its own material; the chip is the machine formula.
        FeedsWarning::FormulaChipNoPrintedRow {
            formula_chipload_mm,
        } => (
            "No printed row; formula chip".to_owned(),
            Some(format!(
                "No vendor roughing row matches this tool and material, so the \
                 {formula_chipload_mm:.4} mm/tooth comes from the machine chip formula \
                 (no source in the repository) and carries no band."
            )),
        ),
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested,
            actual,
            envelope_lo,
            envelope_hi,
        } => (
            format!("Drill feed clamped: {requested:.0} → {actual:.0} mm/min"),
            Some(format!(
                "The drilling envelope for this tool and material is \
                 {envelope_lo:.0}–{envelope_hi:.0} mm/min."
            )),
        ),
        FeedsWarning::DrillFeedAtZAxisRate {
            requested,
            z_rate_mm_min,
            rpm_from,
            rpm_to,
        } => (
            format!("Drill feed at the machine Z rate: {requested:.0} → {z_rate_mm_min:.0} mm/min"),
            Some(format!(
                "{}.",
                rs_cam_core::feeds::vertical::drill_feed_at_z_axis_rate_text(
                    *requested,
                    *z_rate_mm_min,
                    *rpm_from,
                    *rpm_to,
                )
            )),
        ),
    }
}

/// The recommendation's warnings, one row each.
///
/// A problem stays on the page: a warning behind a hover is a warning that
/// was deleted, and this surface exists to stop an operator burning a
/// cutter. Round 2: a warning that records a stage or a source is not a
/// problem, and its row goes in `Details`. These are the long-tool share
/// (a repo rule), the RPM that follows the feed ceiling down, and the two
/// "no vendor data" records (the context row's formula badge states them).
pub(crate) fn warning_rows(explain: &FeedsExplain) -> Vec<Placement> {
    use rs_cam_core::feeds::FeedsWarning;
    explain
        .recommended
        .warnings
        .iter()
        .map(|warning| {
            let (headline, detail) = warning_lines(warning);
            match warning {
                FeedsWarning::LongToolDerate { .. } => Placement::Detail(
                    DataRow::record(headline)
                        .hover_opt(detail)
                        .source(Source::Prov(ProvKind::RepoRule, None)),
                ),
                // P16 (D9): the cap row itself is on the face; the unstated
                // cap is its record in Details.
                FeedsWarning::RpmLoweredForFeedCeiling { .. }
                | FeedsWarning::DrillFeedAtZAxisRate { .. }
                | FeedsWarning::CuttingCapUnstated { .. }
                | FeedsWarning::NoVendorRowsForRoutedOperation { .. }
                | FeedsWarning::VendorRowPublishesNoChipload { .. } => {
                    Placement::Detail(DataRow::record(headline).hover_opt(detail))
                }
                FeedsWarning::FeedRateClamped { .. }
                | FeedsWarning::PowerLimited { .. }
                | FeedsWarning::PowerLadderReducedCut { .. }
                | FeedsWarning::DocExceedsFlute { .. }
                | FeedsWarning::ScallopInvalid { .. }
                | FeedsWarning::ShankTooLarge { .. }
                | FeedsWarning::DepthJudgedAfterSimulation
                | FeedsWarning::ChiploadBelowRubbingFloor { .. }
                | FeedsWarning::FormulaChipNoPrintedRow { .. }
                | FeedsWarning::DrillFeedClampedToEnvelope { .. } => {
                    Placement::Problem(caution(format!("⚠ {headline}")).hover_opt(detail))
                }
            }
        })
        .collect()
}

// ── The Suggest stages that move a number ────────────────────────────

/// The record line of one Suggest record, and whether it is a Caution.
///
/// The operator's standing rule (ruling R4, 2026-09-24): every stage that
/// moves a number is one line on the card, with its source status. Round 2
/// puts the line in the card's `Details` ([`suggest_rows`]). The match is
/// exhaustive, so a new record must choose here. A record that gives no
/// line either moves no number, or its row hover carries it (the declutter
/// ruling of 2026-09-15); the sentry
/// `every_stage_that_moves_a_number_is_on_the_card_g_visible` holds the
/// list.
fn suggest_line(warning: &SuggestWarning) -> Option<(String, bool)> {
    match warning {
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            rescaled_mm_per_min,
            factor_at_calculator,
            factor_at_final,
            cap_hit,
        } => Some((
            format!(
                "Feed re-derived at the final depth: {requested_mm_per_min:.0} → \
                 {rescaled_mm_per_min:.0} mm/min (depth ladder ×{factor_at_calculator:.2} → \
                 ×{factor_at_final:.2}, published charts){}",
                match cap_hit {
                    Some(FeedRecalibrationCap::MaxFeed) => "; held at the machine feed ceiling",
                    Some(FeedRecalibrationCap::DeflectionThreshold) => {
                        "; held by the deflection budget"
                    }
                    None => "",
                }
            ),
            cap_hit.is_some(),
        )),
        // Ruling R4 Q10: this record mirrors the calculator's
        // `FeedsWarning::RpmLoweredForFeedCeiling`, which `draw_warnings`
        // paints on the face with the same core text. A second face line
        // would repeat it; the rationale row carries it on the RPM hover.
        SuggestWarning::RpmLoweredForFeedCeiling { .. } => None,
        // G6 ramp (2026-09-25): the card has no ramp row, so the record is a
        // face line. It names the arm and θ, and the value the operation
        // holds now. The three arms (the G6 chip, the G10 plunge slope, no
        // entry feed) share the line. The entry notes follow it as their own
        // rows (`suggest_rows`).
        SuggestWarning::RampFeed {
            from_mm_min,
            record,
            ..
        } => {
            let (line, _) = record.card_text();
            let now = match from_mm_min {
                Some(from) => format!("now {from:.0} mm/min"),
                None => "now the plunge rate".to_owned(),
            };
            // A sourced line ends in its chip limit; a fallback line ends in
            // a full sentence.
            let line = if record.value().is_some() {
                format!("{line}; {now}")
            } else {
                format!("{line} It is {now}.")
            };
            Some((line, false))
        }
        // These records reach the card through the rationale rows, on the
        // hover of the row whose number they move (`append_rationale`), or
        // they move no number. `PlungeReDerived` (G10) is on the plunge row
        // hover.
        SuggestWarning::PlungeClampedToFeed { .. }
        | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
        | SuggestWarning::StrategyRecommendedNotApplied { .. }
        | SuggestWarning::PlungeReDerived { .. }
        | SuggestWarning::FeedClampedToChiploadFloor { .. }
        | SuggestWarning::PowerRecheckedAfterRescale { .. }
        | SuggestWarning::CutGeometryFieldNotHeld { .. } => None,
        // Feeds contract P5 (K2, D4): the card shows the rules with no
        // source that the apply accepts, before the click.
        SuggestWarning::UnsourcedRowsAccepted { rows } => {
            Some((format!("No source: {}", rows.join(", ")), false))
        }
        // P13c (O3): the computed width states its assumptions; the text
        // is the one core line MCP and the CLI print.
        SuggestWarning::ComputedWidthAssumptions => Some((
            rs_cam_core::feeds::force_budget::computed_width_assumptions_line(),
            true,
        )),
        // K2b, K6: the operator's point is outside the safe envelope.
        SuggestWarning::OutsideEnvelope {
            binding, margin, ..
        } => Some((
            format!(
                "Outside the safe envelope: {:.0} % past the {}",
                -margin * 100.0,
                binding.label()
            ),
            true,
        )),
    }
}

/// The Suggest stages that move a number, one row each.
///
/// Ruling R4 (2026-09-24): the feed re-derive at the final depth changes
/// the recipe, so it has a row. Round 2: the rows are derivation and go in
/// `Details`. A Caution keeps the warning mark and the Caution tone inside
/// `Details`. The ramp record gives one row, then
/// one row per entry note (G10); each note's rule is on its hover.
pub(crate) fn suggest_rows(warnings: &[SuggestWarning]) -> Vec<Placement> {
    let mut out = Vec::new();
    for warning in warnings {
        let Some((line, is_caution)) = suggest_line(warning) else {
            continue;
        };
        // P13c (O3): the visible warning of a computed width sits on the
        // envelope header row (`envelope::header`, the core envelope notes);
        // here it is a caution stage inside `Details`, the same core line.
        // P13c: the computed-width assumptions take the short core face (it
        // fits at 340 points) with the whole core line on hover.
        if matches!(warning, SuggestWarning::ComputedWidthAssumptions) {
            out.push(Placement::Detail(
                stage_record(
                    rs_cam_core::feeds::force_budget::COMPUTED_WIDTH_ASSUMPTIONS_FACE.to_owned(),
                    is_caution,
                )
                .hover(line),
            ));
            continue;
        }
        {
            let mut row = stage_record(line, is_caution);
            if let SuggestWarning::RampFeed { record, .. } = warning {
                let (_, detail) = record.card_text();
                row = row.hover(detail);
                match record {
                    RampFeed::Sourced(_) => {
                        row = row.source(Source::Prov(ProvKind::PublishedRule, None));
                    }
                    RampFeed::PlungeSlope(_) => {
                        row = row.source(Source::Prov(ProvKind::RepoRule, None));
                    }
                    RampFeed::NoEntryFeed { .. } => {}
                }
            }
            out.push(Placement::Detail(row));
        }
        if let SuggestWarning::RampFeed { notes, .. } = warning {
            for note in notes.as_slice() {
                out.push(Placement::Detail(
                    stage_record(note.headline.clone(), note.caution).hover(note.detail.clone()),
                ));
            }
        }
    }
    out
}

/// One stage record: the warning mark and the Caution tone for a Caution,
/// else the muted record tone.
fn stage_record(line: String, is_caution: bool) -> DataRow {
    if is_caution {
        DataRow::record(format!("⚠ {line}")).tone(theme::WARNING_MILD)
    } else {
        DataRow::record(line).tone(tokens::TEXT_MUTED)
    }
}
