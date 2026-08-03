//! Adapter: generation-time geometry findings → [`Diagnostic`] list.
//!
//! These are facts an operation learned about the geometry while
//! generating, which are not properties of the emitted toolpath and not
//! derivable from a simulation trace — see
//! [`crate::compute::execute::GenerationFindings`].
//!
//! Why this adapter exists at all: `scallop` has always measured the
//! interior area its ring cascade failed to reach, and has always
//! reported it exclusively through `tracing::warn!`. A warning nobody
//! sees is not a warning — the campaign shipped a 28 mm block of
//! unmachined material for weeks because no run installed a subscriber,
//! and the GUI's diagnostics list had no channel for it at all
//! (`planning/unified_v3_design.md` §13/§14c/§14h).

use crate::compute::config::{
    TRUNCATED_CORE_DOMAIN, TRUNCATED_CORE_RESOLUTION, TRUNCATED_CORE_STAGE, ToolpathStats,
};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use crate::ids::ToolpathId;

/// Area (mm²) below which standing material is not worth a diagnostic.
///
/// A ring cascade can finish with a sliver of un-collapsed polygon that
/// the next pass covers anyway. One square millimetre is far below the
/// scale of the defect this exists to catch (837 mm² and 4 461 mm²
/// measured on wanaka) while keeping the list quiet on rounding.
const STANDING_MATERIAL_FLOOR_MM2: f64 = 1.0;

/// Emit diagnostics for a toolpath's generation-time findings.
pub fn diagnostics_from_generation(
    toolpath_id: ToolpathId,
    stats: &ToolpathStats,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(truncated_core(toolpath_id, stats));
    out.extend(unmachined_band(toolpath_id, stats));
    out.extend(tip_float(toolpath_id, stats));
    out.extend(deprecated_dial(toolpath_id, stats));
    out.extend(derived_stepover(toolpath_id, stats));
    out.extend(clipped_band(toolpath_id, stats));
    out.extend(ramp_reach_clamp(toolpath_id, stats));
    out.extend(claims_reference(toolpath_id, stats));
    out.extend(zero_removal(toolpath_id, stats));
    out
}

/// A4 (Checkpoint E): a rest pass whose emitted cutting geometry never gets
/// under the reference stock it was planned against.
///
/// `Caution`, not `Blocking` and not `Info`. The operator's ruling is that
/// this is a **report and not a refusal** — the pass may legitimately be
/// wanted — but it is not advisory trivia either: on H4's §3.2 fixture the
/// pass cost 45% of the arm's runtime and 48 retract round trips to remove
/// nothing, and the most likely cause is a mis-aimed reference, which is a
/// thing an operator would want to know before running it.
///
/// No area floor, unlike its neighbours: there is no small-value case here
/// to be quiet about. Either the pass reaches material or it does not.
fn zero_removal(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = the measurement did not run, or it ran and found real
    // engagement. Neither is a claim (A/M9's rule).
    let Some(f) = stats.zero_removal else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_ZERO_REMOVAL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Measured at generation against the prior stock snapshot this pass
        // was planned on — the same nearest-cell lookup the air-cut filter
        // uses. A simulation cannot supersede it: a sim of this pass removes
        // nothing either, and says so only as an absence.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "This rest pass removes no material: over {samples} sampled \
             cutting positions the tool never gets under the stock the prior \
             operation left (deepest reach {deepest:+.4} mm against a \
             {floor:.4} mm floor, which is what the reference's own \
             sampling can manufacture; positive would be INTO material). It \
             still costs {cutting:.0} mm of cutting \
             travel plus its retracts. Most often the reference is not what \
             was intended — check that the prior operation actually left \
             something here, and that this pass's stock-to-leave is BELOW \
             what the prior pass left. Keeping the pass is a legitimate \
             choice; running it unknowingly is not. [Material standing \
             above the CUTTER's own surface, mm; measured at generation \
             against the prior stock snapshot; sampled along the swept path \
             at the stock grid cell. Report-only — no gate.]",
            samples = f.sampled_positions,
            deepest = f.deepest_engagement_mm,
            floor = f.floor_mm,
            cutting = f.cutting_distance_mm,
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// A/M6: which rest reference the claims pipeline ran against.
///
/// Always emitted when a claims pipeline ran, and never otherwise. That is
/// not the "a notice on every toolpath is a notice nobody reads" case the
/// deprecated-dial rule guards against: claims run only on a `UnifiedFinish`
/// whose `pencil_claims` dial is deliberately on, and when they do, WHICH
/// field the detector read is the single largest lever on what the operation
/// cuts — measured at −88.7% cutting on wanaka from that one choice. It is
/// also visible nowhere else, because `auto` resolves against context that
/// is not a config field.
///
/// `Caution` for the two outcomes that need action; `Info` otherwise.
fn claims_reference(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = no claims pipeline ran, so no reference was resolved. Not a
    // claim of any kind (A/M9's rule).
    let Some(f) = stats.claims_reference else {
        return Vec::new();
    };
    let r = f.resolution;
    let severity = if r.needs_attention() || f.territory_clip_skipped() {
        Severity::Caution
    } else {
        Severity::Info
    };
    // The consequence, spelled out where it exists: `territory_clip` is the
    // dial that turns this operation into a rest pass, and it only runs
    // under a machined-stock reference. Skipping it silently is how a rest
    // pass becomes an all-over pass that still calls itself a rest pass.
    let clip_note = if f.territory_clip_skipped() {
        " Rest-territory confinement (`territory_clip`) was requested and \
         SKIPPED — it needs the machined-stock reference — so this operation \
         generated over its FULL territory, not over rest islands."
    } else {
        ""
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::CONFIG_CLAIMS_REFERENCE),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        // Read off the resolved dial and what was in scope at generation
        // time — nothing inferred, and no simulation can see it.
        confidence: Confidence::Verified,
        severity,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "Rest reference: {used} ({verb} — `{label}`). {why}{clip_note} \
             [Report-only — no gate.]",
            used = match r.reference() {
                crate::unified_finish::CreaseReference::MachinedStock => "machined prior stock",
                crate::unified_finish::CreaseReference::SelfProbe => "analytic self-probe",
            },
            verb = if r.is_derived() {
                "derived"
            } else {
                "pinned by claims_reference"
            },
            label = r.label(),
            why = r.why(),
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// PR-8b: a ramp descent the reach clamp had to raise.
///
/// `Caution` and worded as UNCUT MATERIAL, not as a averted collision: the
/// emitted path is now safe, which is exactly why nothing else will ever
/// mention it. A dexel simulation replays the clamped path and finds a clean
/// pass; the operator's only clue that the feature is not finished would be
/// the part.
///
/// Silent on an inert clamp — the ladder bottom was already holdable and no
/// point moved. Not silent on a measured zero-lift descent with a moved
/// ladder bottom: that IS the finding (two terraces of commanded depth
/// removed before any point was generated).
fn ramp_reach_clamp(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = no ramp descent ran. Not a claim of any kind (A/M9's rule).
    let Some(f) = stats.ramp_reach_clamp.as_deref() else {
        return Vec::new();
    };
    if f.is_inert() {
        return Vec::new();
    }
    let ladder_lift = f.holdable_bottom_z_mm - f.requested_bottom_z_mm;
    let pct = if f.ramp_points > 0 {
        100.0 * f.clamped_points as f64 / f.ramp_points as f64
    } else {
        0.0
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_RAMP_REACH_CLAMP),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Measured during generation against the operation's own drop-cutter
        // tool-centre surface — the deepest this cutter's reference point can
        // descend at each XY. Not a heuristic and not superseded by a sim.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "Ramp descent TRUNCATED by cutter reach: {clamped} of {total} \
             ramp points ({pct:.0}%) were raised, by up to {lift:.3} mm, \
             across {area}, because this cutter cannot hold the commanded \
             depth there; and the descent's bottom was lifted \
             {ladder_lift:.3} mm (from {requested:.3} to {holdable:.3} mm) \
             because nothing below that is reachable at all. The pass is safe \
             as emitted — that material is simply LEFT, and no simulation can \
             tell you so. Use a smaller or longer-reach tool for these \
             features, or follow with an operation that can. [Report-only — \
             no gate.]",
            clamped = f.clamped_points,
            total = f.ramp_points,
            lift = f.max_lift_mm,
            requested = f.requested_bottom_z_mm,
            holdable = f.holdable_bottom_z_mm,
            // C8: the EXTENT. A worst-case depth with no area could mean one
            // stray point or half the part. `None` here is not a zero, so it
            // says so rather than printing one.
            area = match f.lifted_area() {
                Some((area, provenance)) => format!(
                    "{:.1} mm² of ramp swath ({})",
                    area.mm2(),
                    provenance.describe()
                ),
                None => "an unmeasured extent".to_owned(),
            },
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// PR-6a (H2.3): an offset stepover the reach policy sized, where the
/// retired envelope-scaled rule would have given something else.
///
/// `Info`, not `Caution`: nothing is wrong. The operation is using the
/// number the policy says is right, and this is the only surface on which
/// that number appears at all — it is neither a config field nor derivable
/// from the emitted moves.
///
/// Silent when the two agree (every plain ball, at every depth). A notice on
/// every toolpath is a notice nobody reads — the same rule
/// `record_deprecated_dial` follows.
fn derived_stepover(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // C8: one diagnostic per derivation. Empty = this operation derived no
    // stepover, which is not a claim of any kind. Filtering out the
    // agree-with-the-envelope-rule cases is what keeps this quiet on plain
    // balls, and it now happens per derivation rather than deciding the
    // whole operation on whichever one got recorded first.
    stats
        .derived_stepovers
        .iter()
        .filter(|f| !f.matches_the_envelope_rule())
        .map(|f| Diagnostic {
            id: DiagnosticId::from(ids::CONFIG_DERIVED_STEPOVER),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::Geometry,
            severity: Severity::Info,
            // Read straight off the policy call that steered the fan.
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "{site}: offset stepover {stepover:.3} mm, sized by the reach \
             policy at {depth:.3} mm rest depth ({basis}). The retired \
             envelope rule (half the cutter's widest radius) would have used \
             {envelope:.3} mm — {ratio:.1}× wider — which on a tapered tool \
             is the SHANK, not anything the tip cuts. [Report-only — no gate.]",
                site = f.site,
                stepover = f.stepover_mm,
                depth = f.reference_depth_mm,
                basis = f.reference_depth_basis,
                envelope = f.envelope_rule_mm,
                ratio = if f.stepover_mm > 0.0 {
                    f.envelope_rule_mm / f.stepover_mm
                } else {
                    f64::NAN
                },
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        })
        .collect()
}

/// PR-5: a retired dial the loaded project still sets.
///
/// `Caution`, not `Warning`: nothing is wrong with the geometry, and the
/// operation is doing the right thing — but the operator's setting has
/// stopped having an effect and only this says so. Silence here is the
/// failure mode the programme keeps re-learning ("a warning nobody sees is
/// not a warning"); refusing to load the project would be worse.
fn deprecated_dial(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = the project sets nothing retired. Not a claim of any kind.
    let Some(f) = stats.deprecated_dial.as_deref() else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::CONFIG_DEPRECATED_DIAL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Read straight off the loaded config — nothing inferred.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "`{dial}` is set to {value} (default {default}) but is NO LONGER \
             READ: it was replaced by {replaced_by}. The value still loads and \
             saves so this project is unchanged, but it is not steering the \
             operation. [Report-only — no gate.]",
            dial = f.dial,
            value = f.value,
            default = f.default_value,
            replaced_by = f.replaced_by,
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// The A/M9 finding: a ring cascade that hit its cap and left the interior
/// of a region uncut.
///
/// Split out of [`diagnostics_from_generation`] by Wave D1 — it used to
/// early-`return` the whole adapter when nothing measured a cascade, which
/// would have made every later finding conditional on this one.
///
/// **B8 (Checkpoint E):** the message now carries the M4 §5b untouched /
/// standing split that narration has had since wave 15. The two halves are
/// different measurements — an exact hole-aware polygon area, and an
/// estimator — so they are stated as two clauses with their own words, never
/// summed. The diagnostic **id** keeps the pre-wave-16 spelling
/// (`geom.standing_material`) on purpose: it is a stable identity that GUI
/// filters and suppression rules key on, and `MEASUREMENT_DOMAINS.md`'s
/// renaming table rules it out of the A6 rename.
fn truncated_core(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = not measured (no ring cascade ran). An absent measurement is
    // not a defect claim, and it is NOT the same as a measured zero — see
    // `ToolpathStats::truncated_core_mm2` (A/M9).
    let Some(area) = stats.truncated_core_mm2 else {
        return Vec::new();
    };
    // NaN never reports either, for the same reason.
    if area.partial_cmp(&STANDING_MATERIAL_FLOOR_MM2) != Some(std::cmp::Ordering::Greater) {
        return Vec::new();
    }

    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_STANDING_MATERIAL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Measured during generation from the cascade's own residual
        // polygons — not a heuristic, and not superseded by a sim, which
        // cannot see material the toolpath never attempted to cut.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "{area:.0} mm² of material left UNCUT inside the machining \
             region: the scallop ring cascade hit its ring cap before the \
             offsets collapsed, so the INTERIOR was never reached. The part \
             will have a raised island. Reduce the region, coarsen the \
             scallop height, or split the operation.{split} \
             [{domain}; {stage}; {resolution}. Report-only — no gate.]",
            split = untouched_standing_clause(stats),
            domain = TRUNCATED_CORE_DOMAIN,
            stage = TRUNCATED_CORE_STAGE,
            resolution = TRUNCATED_CORE_RESOLUTION,
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// B8: the M4 §5b split, rendered as a sentence to append to the
/// `geom.standing_material` message — or the empty string when nothing
/// measured it.
///
/// Deliberately additive: an operator reading the existing message loses
/// nothing, and the split only appears where it was actually measured. The
/// two halves keep their own vocabulary — *never reached* for the exact
/// hole-aware area, *reached, left high* for the estimator, which is
/// prefixed `~` because it is `(dropped arc length) × stepover` and not an
/// area anyone measured. They are not summed, and the message says why.
fn untouched_standing_clause(stats: &ToolpathStats) -> String {
    match (
        stats.untouched_material_mm2,
        stats.reached_uncut_estimate_mm2,
    ) {
        // Not measured (nothing that runs a cascade reported the split) —
        // stay silent rather than render an absence as a zero.
        (None, None) => String::new(),
        (untouched, standing) => {
            let untouched = untouched
                .filter(|v| v.is_finite())
                .map_or_else(|| "not measured".to_owned(), |v| format!("{v:.0} mm²"));
            let standing = standing
                .filter(|v| v.is_finite())
                .map_or_else(|| "not measured".to_owned(), |v| format!("~{v:.0} mm²"));
            format!(
                " Split: {untouched} never reached (hole-aware, exact) and \
                 {standing} reached but left high (an estimator, not an area). \
                 Different measurements — do not add them."
            )
        }
    }
}

/// C8: a planned finish band whose Z ladder height resolution SHORTENED
/// while it still cut — a PARTLY machined feature.
///
/// `Info`, not `Caution`: unlike [`unmachined_band`] the operation did cut
/// here, and a partial clip is often exactly what an operator asked for by
/// pinning a height. What was missing was any way to tell. The message
/// carries requested-vs-delivered Z and the worst lost height, because
/// "how much of the wall is unfinished" is the question and a level count
/// does not answer it.
///
/// Same area floor as its sibling: a clipped band with no area is a
/// decomposition artefact, not a feature, and NaN never reports.
fn clipped_band(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = no band was partially clipped, or nothing that plans bands
    // ran. Neither is a defect claim (A/M9's rule, X-19).
    let Some(f) = stats.clipped_band.as_deref() else {
        return Vec::new();
    };
    if f.area_mm2.partial_cmp(&STANDING_MATERIAL_FLOOR_MM2) != Some(std::cmp::Ordering::Greater) {
        return Vec::new();
    }
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_CLIPPED_BAND),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Info,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "{area:.1} mm² of the {band} band across {count} planned \
             region(s) cut only PART of its depth: the resolved {clip} = \
             {clip_z:.3} mm shortened the ladder from {req_lo:.3}..{req_hi:.3} mm \
             to {del_lo:.3}..{del_hi:.3} mm ({planned} levels planned, \
             {resolved} laddered), leaving up to {lost:.3} mm of the feature \
             unfinished. If that was not deliberate, pin {clip} to the real \
             depth of the feature. [{provenance}. Report-only — no gate.]",
            area = f.area_mm2,
            band = f.band_label,
            count = f.region_count,
            clip = f.clip_label,
            clip_z = f.clip_z_mm,
            req_lo = f.requested_bottom_z_mm,
            req_hi = f.requested_top_z_mm,
            del_lo = f.delivered_bottom_z_mm,
            del_hi = f.delivered_top_z_mm,
            planned = f.planned_levels,
            resolved = f.resolved_levels,
            lost = f.max_lost_height_mm,
            provenance = f.provenance.describe(),
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// Wave D1: a planned finish band that emitted no cutting because height
/// resolution clipped its Z range away.
///
/// Reported at `Caution` like the standing-material finding: report-only is
/// the plan's gate, and no verdict may move on it. The message names the
/// band, the area, and the height that did it, because those three are what
/// an operator needs to go and pin the height.
fn unmachined_band(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = nothing dropped, or nothing that plans bands ran. Neither is
    // a defect claim (A/M9's rule, `MEASUREMENT_DOMAINS.md` X-19).
    let Some(f) = stats.dropped_band.as_deref() else {
        return Vec::new();
    };
    // A dropped band with no area is a decomposition artefact, not a
    // feature; NaN never reports either.
    if f.area_mm2.partial_cmp(&STANDING_MATERIAL_FLOOR_MM2) != Some(std::cmp::Ordering::Greater) {
        return Vec::new();
    }
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_UNMACHINED_BAND),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Measured at generation from the planner's own band polygons and
        // the resolved heights. A simulation cannot find it: the toolpath
        // never attempted the cut, so there is nothing in a cut record.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "{area:.1} mm² of the {band} band across {count} planned \
             region(s) emitted NO cutting: the resolved {clip} = \
             {clip_z:.3} mm clipped its Z range away, so the feature will be \
             left at full stock. Pin {clip} to the real depth of the \
             feature. [{provenance}. Report-only — no gate.]",
            area = f.area_mm2,
            band = f.band_label,
            count = f.region_count,
            clip = f.clip_label,
            clip_z = f.clip_z_mm,
            provenance = f.provenance.describe(),
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// Wave D1: a valley centreline driven over material the cutter cannot
/// physically reach (Checkpoint A evidence §5 / §9.4).
fn tip_float(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    use crate::compute::config::{TIP_FLOAT_PROVENANCE, TIP_FLOAT_THRESHOLD_MM};
    // `None` = this operation emits no centrelines. `Some` with zero
    // floating points is a measured-clean pass — also not a diagnostic.
    let Some(f) = stats.tip_float else {
        return Vec::new();
    };
    if f.floating_points == 0 {
        return Vec::new();
    }
    let pct = 100.0 * f.floating_fraction().unwrap_or(0.0);
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_TIP_FLOAT),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "{floating} of {total} centreline points ({pct:.0}%) run over \
             material this tool cannot reach — it wedges between the valley \
             walls and floats above the floor. Worst residual {max:.3} mm is \
             left uncut BENEATH the emitted line (float > \
             {TIP_FLOAT_THRESHOLD_MM} mm counts). The pass as emitted cannot \
             remove it: use a smaller tip, or route these valleys to a finer \
             tool. [{provenance}. Report-only — no gate.]",
            floating = f.floating_points,
            total = f.centreline_points,
            max = f.max_float_mm,
            provenance = TIP_FLOAT_PROVENANCE.describe(),
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn stats(truncated_core_mm2: f64) -> ToolpathStats {
        ToolpathStats {
            truncated_core_mm2: Some(truncated_core_mm2),
            ..ToolpathStats::default()
        }
    }

    #[test]
    fn silent_when_nothing_is_left_standing() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(0.0)).is_empty());
    }

    /// A/M9: "no cascade ran" is a different state from "a cascade measured
    /// zero", and neither is a defect — but only the former may be reported
    /// as unmeasured downstream, so the adapter must accept it.
    #[test]
    fn silent_when_nothing_was_measured() {
        assert!(
            diagnostics_from_generation(ToolpathId(1), &ToolpathStats::default()).is_empty(),
            "default stats carry `None` — not measured"
        );
    }

    /// A sliver of un-collapsed polygon is not a raised island.
    #[test]
    fn silent_below_the_floor() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(0.5)).is_empty());
    }

    /// NaN is "not measured", not "defect" — an unmeasured cascade must not
    /// manufacture a scrap warning.
    #[test]
    fn silent_on_nan() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(f64::NAN)).is_empty());
    }

    /// The wanaka case that motivated the whole channel: 837 mm² of
    /// standing material that only a `tracing::warn!` ever mentioned.
    #[test]
    fn reports_a_real_uncut_island() {
        let out = diagnostics_from_generation(ToolpathId(7), &stats(837.0));
        assert_eq!(out.len(), 1);
        let d = &out[0];
        assert_eq!(d.id, DiagnosticId::from(ids::GEOM_STANDING_MATERIAL));
        assert_eq!(d.scope, Scope::Toolpath { id: ToolpathId(7) });
        assert_eq!(d.category, Category::Geometry);
        assert_eq!(d.severity, Severity::Caution);
        assert!(
            d.message.contains("837"),
            "the area is the actionable number: {}",
            d.message
        );
        // M1: an area with no declared domain is exactly the unlabelled
        // `f64` the audit found being compared across domains.
        for needle in [
            TRUNCATED_CORE_DOMAIN,
            TRUNCATED_CORE_STAGE,
            TRUNCATED_CORE_RESOLUTION,
        ] {
            assert!(
                d.message.contains(needle),
                "message must declare {needle:?}: {}",
                d.message
            );
        }
    }
}
