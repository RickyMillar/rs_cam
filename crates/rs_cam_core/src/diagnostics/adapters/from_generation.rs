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

use crate::compute::toolpath_stats::{
    TRUNCATED_CORE_DOMAIN, TRUNCATED_CORE_RESOLUTION, TRUNCATED_CORE_STAGE, ToolpathStats,
};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use crate::geometry::region_mask::RegionCapReport;
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
    out.extend(derived_stepover(toolpath_id, stats));
    out.extend(clipped_band(toolpath_id, stats));
    out.extend(ramp_reach_clamp(toolpath_id, stats));
    out.extend(claims_reference(toolpath_id, stats));
    out.extend(zero_removal(toolpath_id, stats));
    out.extend(buried_curve(toolpath_id, stats));
    out.extend(selection_note(toolpath_id, stats));
    out.extend(tab_placement(toolpath_id, stats));
    out.extend(offset_library_failures(toolpath_id, stats));
    out.extend(boundary_clip_dropped(toolpath_id, stats));
    out.extend(waterline_ladder(toolpath_id, stats));
    out.extend(inert_claims_dial(toolpath_id, stats));
    out.extend(region_cap_truncated(toolpath_id, stats));
    out.extend(rest_levels_fallback(toolpath_id, stats));
    out.extend(cuts_into_model(toolpath_id, stats));
    out.extend(non_centre_cutting_plunge(toolpath_id, stats));
    out.extend(buried_face(toolpath_id, stats));
    out.extend(feasibility(toolpath_id, stats));
    out
}

/// K5 (feeds contract P7): one diagnostic per physical rule row of the
/// feasibility stage, with the id of the rule's `physical_rules!` row. The
/// verdict is the stage's; this adapter converts. Every row is `Caution`:
/// the pre-simulation reach rows read the stock box and can over-read
/// (lead decision). Only CentreCut stays `Critical`. Silent when the stage
/// did not run or no rule warned.
fn feasibility(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    use crate::compute::feasibility::PhysicalRule;
    let Some(report) = stats.feasibility.as_ref() else {
        return Vec::new();
    };
    report
        .rows()
        .iter()
        .map(|row| {
            let id = row.id();
            let severity = match row.rule() {
                PhysicalRule::CentreCut => Severity::Critical,
                PhysicalRule::ReachFlute | PhysicalRule::ReachHolder | PhysicalRule::VTipFlat => {
                    Severity::Caution
                }
            };
            Diagnostic {
                id: DiagnosticId::from(id),
                scope: Scope::Toolpath { id: toolpath_id },
                category: Category::Safety,
                severity,
                // Read from the emitted moves, the tool record and the
                // stock box, with no simulation.
                confidence: Confidence::Static,
                state: DiagnosticState::Current,
                source: Source::StaticValidation,
                face: crate::diagnostics::presentation::face(id),
                anchor: crate::diagnostics::presentation::anchor(id),
                message: row.text().to_owned(),
                evidence: None,
                fix: None,
                supersedes: vec![],
                suppressed_diagnostics: vec![],
            }
        })
        .collect()
}

/// K6 (G-CENTRECUT, SAFETY): an entry had to plunge a tool that does not
/// cut at its centre straight into stock deeper than its blade protrusion.
/// Before ruling Q6 generation refused; now the toolpath keeps the plunge
/// and this `Critical` finding names it. Silent when no entry plunged.
fn non_centre_cutting_plunge(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(plunge) = stats.non_centre_cutting_plunge else {
        return Vec::new();
    };
    let id = ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE;
    vec![Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Safety,
        severity: Severity::Critical,
        // Read from the emitted entry and the stock at generation.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(id),
        anchor: crate::diagnostics::presentation::anchor(id),
        message: format!("Entry plunge: {plunge}."),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// K6 (UX-1c, SAFETY): on "Rest machining: from previous ops", material
/// stands over the face of a one-depth op. Before ruling Q6 generation
/// refused; now the op generates and this `Critical` finding names it.
/// Silent when the face is clear. The fresh-stock case is the precondition
/// adapter's (`ToolpathHeights::buried_face_refusal`).
fn buried_face(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(buried) = stats.buried_face else {
        return Vec::new();
    };
    let id = ids::PRECOND_BURIED_FACE;
    vec![Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Safety,
        severity: Severity::Critical,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(id),
        anchor: crate::diagnostics::presentation::anchor(id),
        message: buried.to_string(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// UX-7b (SAFETY): the emitted moves cut into the mesh of the toolpath
/// model. The verdict and the class are the guard's (`compute::model_gouge`,
/// `ModelGougeFinding::severity`): `Blocking` when the tip cuts into the
/// model, and the export warns on the same finding (K6); `Caution` when a 3D Rough
/// cuts into its stock to leave only (B2). The evidence is the move and the
/// tip of the deepest gouge, in the setup frame. Silent when the guard did
/// not run or measured clean.
fn cuts_into_model(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some((f, severity)) = stats
        .model_gouge
        .and_then(|f| f.severity().map(|severity| (f, severity)))
    else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::SAFETY_CUTS_INTO_MODEL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Safety,
        severity,
        // Measured at generation on the emitted moves against the mesh.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::SAFETY_CUTS_INTO_MODEL),
        anchor: crate::diagnostics::presentation::anchor(ids::SAFETY_CUTS_INTO_MODEL),
        message: f.message(),
        evidence: Some(crate::diagnostics::DiagnosticEvidence::Move {
            toolpath_id,
            move_index: f.move_index,
            position: Some(f.at),
        }),
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// G-RESTLEVELS: a 2.5D rest operation fell back to the whole outline at
/// every level. `Caution`: the toolpath is safe, because a fallback cuts
/// more, but it cuts air the operator asked to skip. Silent when no
/// fallback occurred, and silent when the operation read no stock.
fn rest_levels_fallback(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    use crate::compute::toolpath_stats::RestLevelsFallback;
    let Some(fallback) = stats.rest_levels.and_then(|f| f.fallback) else {
        return Vec::new();
    };
    let cause = match fallback {
        RestLevelsFallback::StockOffOutline => {
            "the machined stock does not overlap the outline in X and Y, so the stock \
             and the outline are probably in different positions"
                .to_owned()
        }
        RestLevelsFallback::RegionCap { total_before_cap } => format!(
            "the remaining material split into {total_before_cap} islands, more than \
             the limit"
        ),
        RestLevelsFallback::WindowTooLarge { cells, limit } => {
            format!("the outline covers {cells} stock cells, more than the limit of {limit}")
        }
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_REST_LEVELS_FALLBACK),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_REST_LEVELS_FALLBACK),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_REST_LEVELS_FALLBACK),
        message: format!(
            "This rest operation cut its whole outline at every level, because \
             {cause}. It cuts air where the earlier operations already removed the \
             stock. [Recorded at generation. Report-only: no gate consumes this.]"
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// F3: the `MAX_REST_REGIONS` cap truncated this operation's rest-region
/// list, so islands the mask found are absent from the artifacts a
/// `DerivedRestRegions` consumer will be confined to.
///
/// `Caution`. Not `Blocking`: the cap is a deliberate guard and the kept
/// regions are real geometry. Not `Info` either: the dropped islands are
/// territory a downstream operation is now forbidden to cut, and nothing else
/// on any operator surface says so — the pre-F3 trace was a `tracing::warn!`
/// in a process that usually installs no subscriber.
///
/// Silent on a measured-clean extraction. `Some` with nothing truncated is a
/// real measurement, but a per-toolpath notice that nothing happened is a
/// notice nobody reads — the same rule [`zero_removal`] and
/// [`offset_library_failures`] follow. `None` (no extraction ran) is silent
/// for the A/M9 reason: an absence is not a claim.
fn region_cap_truncated(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(r) = stats.region_cap.filter(RegionCapReport::truncated) else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_REGION_CAP_TRUNCATED),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Counted at extraction, from the grouped polygons themselves.
        // A simulation cannot supersede it: the dropped islands are simply
        // absent from every artifact downstream, so every later reading of
        // them is clean and silent.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_REGION_CAP_TRUNCATED),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_REGION_CAP_TRUNCATED),
        message: format!(
            "The rest analysis found {total} islands of remaining material and \
             kept only the largest {kept} (limit {cap}). It dropped {dropped}. An \
             operation that uses these rest areas as its boundary cuts only the \
             {kept} it kept, so it does not cut the dropped islands, and no other \
             message tells you. The usual cause is a threshold below the ridge \
             height the earlier pass left, so the ridges themselves become islands. \
             Raise Min Valley Depth or Min Rest Depth until the count is a few to a \
             few dozen. [Keys: min_valley_depth, min_rest_depth_mm. Count of \
             grouped region polygons before truncation; extraction stage. \
             Report-only — no gate consumes this.]",
            total = r.total_before_cap,
            kept = r.kept,
            cap = r.cap,
            dropped = r.dropped(),
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// F4: a rest-claims dial the operator set that this operation never applies.
///
/// `Caution`, not `Info` and not `Blocking` — the same call
/// a config notice makes, and for the same reason. Nothing is wrong with
/// the geometry; what is wrong is that the operator's setting has no effect
/// and only this says so. The measured cost of the silence is the T4 arm:
/// `min_rest_depth_mm` 0.03 → 0.05 produced a byte-identical toolpath, and
/// the "rest tier" it was configuring ran 23 916 s as a full-board finish.
///
/// It is deliberately NOT a refusal and the dial is deliberately still inert:
/// making it apply would re-cut every shipped project that carries one.
fn inert_claims_dial(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = nothing inert is set — the overwhelming default, and not a
    // claim of any kind.
    let Some(f) = stats.inert_claims_dial else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::CONFIG_INERT_CLAIMS_DIAL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Read straight off the loaded config at generation time — nothing
        // inferred, and no simulation can see it: the emitted path is exactly
        // what it would be at the default, so a dexel run replays a perfectly
        // healthy pass.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::CONFIG_INERT_CLAIMS_DIAL),
        anchor: crate::diagnostics::presentation::anchor(ids::CONFIG_INERT_CLAIMS_DIAL),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// Checkpoint C (Q2): a boundary containment that collapsed, so the path is
/// not contained by anything.
///
/// `Caution`. It is not `Blocking` because the pass-through is the ruled
/// behaviour for this cause and the resulting path is usually harmless — but
/// it is not `Info` either: a safety setting the operator switched on is not
/// in force, and that is a thing to know before pressing cycle start. The
/// sibling case, where the containment collapsed because an offset FAILED,
/// never reaches here at all — it refuses the generate.
fn boundary_clip_dropped(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(f) = stats.boundary_clip_dropped else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_BOUNDARY_CLIP_DROPPED),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Observed at generation, where the containment was resolved. A
        // simulation cannot supersede it: an unclipped path simulates
        // perfectly well, which is precisely the problem.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_BOUNDARY_CLIP_DROPPED),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_BOUNDARY_CLIP_DROPPED),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// R3 / R4 (Corne case): the waterline's Z ladder was floored at the stock
/// bottom, or fewer than two levels reached the cutter.
///
/// `Caution` on both readings. Not `Blocking`: the floored ladder is the
/// SAFE one — the levels that would have cut the bed are gone — and an empty
/// ladder cuts nothing. Not `Info`: an operator who pinned a height at or
/// below the bed, or whose ladder missed the model, must see it before
/// the next operation's rest chain inherits the result. A clean ladder
/// (nothing dropped, two or more levels) is silent here; narration prints
/// it.
fn waterline_ladder(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(f) = stats.waterline_ladder else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let make = |id: &str| Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Observed at generation, where the ladder was built. A simulation
        // cannot supersede it: the dropped levels are not in the toolpath.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(id),
        anchor: crate::diagnostics::presentation::anchor(id),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    };
    if f.floored() {
        out.push(make(ids::GEOM_WATERLINE_LEVELS_BELOW_STOCK));
    }
    if f.degenerate() {
        out.push(make(ids::GEOM_WATERLINE_LADDER_EMPTY));
    }
    // W4 (D69 §4): one line; the GUI shows the ladder finding on hover.
    if f.part_unheld {
        out.push(Diagnostic {
            message: f.part_unheld_message(),
            ..make(ids::GEOM_WATERLINE_PART_UNHELD)
        });
    }
    out
}

/// Checkpoint C (Q1 / D-2): 2D offsets in this generation that FAILED rather
/// than collapsed.
///
/// `Caution`, not `Blocking`: the toolpath is real and runnable, and the
/// operator's ruling on the failure contract was explicitly *not* to turn a
/// contained panic into a refusal — an operation that lost one ring may still
/// be producing usable geometry everywhere else. What is not acceptable is
/// nothing saying so, which is the state F-12 measured: an inlay's female
/// pocket stopped early on a dependency's `debug_assert!`, left material, and
/// reported success.
///
/// `Some(0)` is silent. It is a real measurement — every offset was clean —
/// but a per-toolpath notice that nothing happened is a notice nobody reads,
/// and the same rule already governs `zero_removal`.
fn offset_library_failures(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    // `None` = no offset ran through the reporting name, so nothing was
    // measured. Not a claim of any kind (A/M9's rule).
    let Some(n) = stats.offset_library_failures.filter(|n| *n > 0) else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_OFFSET_LIBRARY_FAILURE),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Observed at generation, at the one chokepoint where the dependency
        // is called. A simulation cannot supersede it: the geometry the
        // failed offset would have produced is simply absent from the path,
        // so a sim of this toolpath is clean and says nothing.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_OFFSET_LIBRARY_FAILURE),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_OFFSET_LIBRARY_FAILURE),
        message: format!(
            "{n} 2D offset(s) in this operation failed. The offset library could \
             not process the shape, or a check refused it. The cut that the offset \
             makes (a pocket ring, a contour, an inset) is missing from this \
             toolpath, so material can stay where the part looks finished. Look \
             for loops in the drawing that cross themselves or pinch, repeated \
             points and bad coordinates. A release build finds fewer of these \
             failures and cuts the bad shape anyway, so a lower count there is not \
             a sign of improvement. [Count of offset CALLS, not distinct rings: a \
             depth-stepped operation re-offsets the same geometry once per Z \
             level; most failures are cavalier_contours debug assertions; \
             generation stage. Report-only — no gate.]"
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
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
        face: crate::diagnostics::presentation::face(ids::GEOM_ZERO_REMOVAL),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_ZERO_REMOVAL),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// W6.5: a projected curve that cuts deeper than its commanded depth.
/// `Info` by the lead ruling: it states a measured fact, and it fires also
/// after a normal rough. Silent when nothing was measured.
fn buried_curve(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(f) = stats.buried_curve else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_BURIED_CURVE),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Info,
        // Measured at generation against the stock this operation reads.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_BURIED_CURVE),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_BURIED_CURVE),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// P1: an op left open paths of the drawing out (P3.7: or a face at the
/// top under "each picked face"). `Caution`: the toolpath is safe (a skip
/// only removes cuts), but an item the selection holds is not in the plan.
/// Silent when nothing was left out.
fn selection_note(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(f) = &stats.selection_note else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_SELECTION_NOTE),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Counted at generation over the drawing the op reads.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_SELECTION_NOTE),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_SELECTION_NOTE),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// P5: a Profile loop did not take its tabs on its straight sides. `Info`:
/// the loop still has tabs, but not where the side rule puts them, or
/// fewer than asked. Silent when every loop followed the rule.
fn tab_placement(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
    let Some(f) = &stats.tab_placement else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_TAB_PLACEMENT),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Info,
        // Decided at generation over the loops the op cuts.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_TAB_PLACEMENT),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_TAB_PLACEMENT),
        message: f.message(),
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
        " You asked to cut only the rest areas (Territory Clip, key \
         territory_clip), but that needs the machined stock as the reference. So \
         this operation cut its FULL area, not only the rest islands."
    } else {
        ""
    };
    // The face says what is wrong, or which stock the rest reads.
    let face = if f.territory_clip_skipped() {
        "Rest pass cut the full area".to_owned()
    } else if r.needs_attention() {
        crate::diagnostics::presentation::face(ids::CONFIG_CLAIMS_REFERENCE)
    } else if r.is_derived() {
        "Rest areas found automatically".to_owned()
    } else {
        "Rest areas use the stock source you set".to_owned()
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
        face,
        anchor: crate::diagnostics::presentation::anchor(ids::CONFIG_CLAIMS_REFERENCE),
        message: format!(
            "The rest areas are measured against {used} ({verb}). {why}{clip_note} \
             [Resolution {label}; key claims_reference. Report-only — no gate.]",
            used = match r.reference() {
                crate::finish::unified_finish::CreaseReference::MachinedStock =>
                    "the stock the earlier pass left",
                crate::finish::unified_finish::CreaseReference::SelfProbe => {
                    "the model shape only"
                }
            },
            verb = if r.is_derived() {
                "chosen automatically"
            } else {
                "set by Rest Reference"
            },
            why = r.why(),
            label = r.label(),
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
        face: crate::diagnostics::presentation::face(ids::GEOM_RAMP_REACH_CLAMP),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_RAMP_REACH_CLAMP),
        message: format!(
            "The tool cannot reach the full depth of the ramp. The toolpath \
             raises {clamped} of {total} ramp points ({pct:.0} %) by up to \
             {lift:.3} mm, across {area}. The lowest level moves up \
             {ladder_lift:.3} mm (from {requested:.3} to {holdable:.3} mm), \
             because the tool cannot reach below that. The toolpath is safe, but \
             that material is LEFT, and a simulation does not show it. Use a \
             smaller tool or a tool with a longer reach, or add an operation that \
             can reach. [Report-only — no gate.]",
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
                None => "an area that was not measured".to_owned(),
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
/// [`zero_removal`] follows.
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
            face: if f.slope_derate.is_some() {
                "Stepover lowered for a steep slope".to_owned()
            } else {
                crate::diagnostics::presentation::face(ids::CONFIG_DERIVED_STEPOVER)
            },
            anchor: crate::diagnostics::presentation::anchor(ids::CONFIG_DERIVED_STEPOVER),
            message: match f.slope_derate {
                // Honest raster (Track B, 2026-09-01): a slope-keyed
                // derivation, not a reach-policy one — the reach-policy
                // sentence below would misdescribe it.
                Some(sd) => format!(
                    "{site}: in region {region} the stepover is {stepover:.3} mm, \
                 not the {configured:.3} mm you set, because the steepest slope \
                 there is {slope:.1}°. On a slope the passes are further apart \
                 on the surface than in plan view. The smaller stepover gives the \
                 spacing you set on the steepest slope. [Stepover × cos(max \
                 slope). Report-only — no gate.]",
                    site = f.site,
                    region = sd.region_index,
                    stepover = f.stepover_mm,
                    configured = f.envelope_rule_mm,
                    slope = sd.slope_max_deg,
                ),
                None => format!(
                    "{site}: the stepover is {stepover:.3} mm, sized for what the \
             tool tip can reach at a rest depth of {depth:.3} mm ({basis}). \
             Half the widest radius of the tool would give {envelope:.3} mm \
             ({ratio:.1} \u{00D7} wider), but on a tapered tool that is the \
             shank, not the tip that cuts. [Reach policy against the retired \
             envelope rule. Report-only — no gate.]",
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
            },
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        })
        .collect()
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
        face: crate::diagnostics::presentation::face(ids::GEOM_STANDING_MATERIAL),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_STANDING_MATERIAL),
        message: format!(
            "{area:.0} mm² of material is left UNCUT inside the area: the \
             scallop rings reached their limit before they met in the middle, \
             so the tool never cut the centre. The part has a raised island. \
             Make the area smaller, use a larger scallop height, or split the \
             operation.{split} [{domain}; {stage}; {resolution}. Report-only — \
             no gate.]",
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
                " Of this, the tool never reached {untouched} (an exact area), and \
                 it reached but left high {standing} (an estimate, not a measured \
                 area). These are different measures: do not add them."
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
        face: crate::diagnostics::presentation::face(ids::GEOM_CLIPPED_BAND),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_CLIPPED_BAND),
        message: f.message(),
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
        face: crate::diagnostics::presentation::face(ids::GEOM_UNMACHINED_BAND),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_UNMACHINED_BAND),
        message: format!(
            "{area:.1} mm² of the {band} surface in {count} planned area(s) \
             is NOT cut: the {clip} of {clip_z:.3} mm removes its full Z range, \
             so the feature stays at full stock. Set the {clip} to the real depth \
             of the feature. [{key} = {clip_z:.3} mm; {provenance}; band clip \
             coverage: {coverage}. Report-only — no gate.]",
            area = f.area_mm2,
            band = crate::compute::toolpath_stats::band_phrase(f.band),
            count = f.region_count,
            clip = f.clip.clip.phrase(),
            key = f.clip.clip.label(),
            clip_z = f.clip.clip_z_mm,
            coverage = f.bands_measured.describe(),
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
    // `None` = this operation emits no centrelines. `Some` with zero
    // floating points is a measured-clean pass — also not a diagnostic.
    let Some(f) = stats.tip_float else {
        return Vec::new();
    };
    if f.floating_points == 0 {
        return Vec::new();
    }
    vec![Diagnostic {
        id: DiagnosticId::from(ids::GEOM_TIP_FLOAT),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::GEOM_TIP_FLOAT),
        anchor: crate::diagnostics::presentation::anchor(ids::GEOM_TIP_FLOAT),
        message: f.message(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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
        assert!(diagnostics_from_generation(ToolpathId::from_raw(1), &stats(0.0)).is_empty());
    }

    /// A/M9: "no cascade ran" is a different state from "a cascade measured
    /// zero", and neither is a defect — but only the former may be reported
    /// as unmeasured downstream, so the adapter must accept it.
    #[test]
    fn silent_when_nothing_was_measured() {
        assert!(
            diagnostics_from_generation(ToolpathId::from_raw(1), &ToolpathStats::default())
                .is_empty(),
            "default stats carry `None` — not measured"
        );
    }

    /// A sliver of un-collapsed polygon is not a raised island.
    #[test]
    fn silent_below_the_floor() {
        assert!(diagnostics_from_generation(ToolpathId::from_raw(1), &stats(0.5)).is_empty());
    }

    /// NaN is "not measured", not "defect" — an unmeasured cascade must not
    /// manufacture a scrap warning.
    #[test]
    fn silent_on_nan() {
        assert!(diagnostics_from_generation(ToolpathId::from_raw(1), &stats(f64::NAN)).is_empty());
    }

    /// The wanaka case that motivated the whole channel: 837 mm² of
    /// standing material that only a `tracing::warn!` ever mentioned.
    #[test]
    fn reports_a_real_uncut_island() {
        let out = diagnostics_from_generation(ToolpathId::from_raw(7), &stats(837.0));
        assert_eq!(out.len(), 1);
        let d = &out[0];
        assert_eq!(d.id, DiagnosticId::from(ids::GEOM_STANDING_MATERIAL));
        assert_eq!(
            d.scope,
            Scope::Toolpath {
                id: ToolpathId::from_raw(7)
            }
        );
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
