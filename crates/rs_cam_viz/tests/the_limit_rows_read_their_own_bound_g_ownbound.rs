//! **G-OWNBOUND — every limit row reads the bound its own gate judged, and
//! the face carries a reading rather than a caveat.**
//!
//! # The two defects this file pins
//!
//! **Class 2 — a quantity divided by a fraction of a DIFFERENT quantity.**
//! `verdict_badge` drew the deflection row's percent against
//! `DEFLECTION_SAFE_LD_RATIO = 4.0`, an L over D RATIO, beside a gate that
//! judges MILLIMETRES against `tool_load::deflection::EXCEEDS_BOUND_MM =
//! 0.200`. The two share neither unit nor meaning, and the percent on screen
//! was therefore about 20 times smaller than the one the gate would state.
//! S4 put `bound` on every `CriterionStatus`; V2 makes the row read it.
//!
//! **A caveat on the face.** The `Approximate` arms appended `\u{2248}` and
//! repainted the row in `WARNING_MILD`, so a row that was WITHIN its bound
//! read as a warning. The operator's ruling of 2026-09-18: every limit reads
//! as a plain 0-to-limit figure, and the confidence tier moves into the
//! hover. The behaviour still carries the meaning — a weak bound cannot
//! refuse an export (`BoundSource::gates_export`) — so the face does not have
//! to.
//!
//! # Rendered, not source-scanned
//!
//! Every arm runs the Simulation Inspector through a real `egui::Context` and
//! reads the painted text, the way
//! `the_corridor_bounds_the_band_g_corridor.rs` reads painted shapes. A
//! source scan passes for a row that is built and never reaches the screen,
//! and for a percent computed correctly and painted from the wrong binding.
//!
//! # Non-vacuity
//!
//! `the_fixture_measures_something_g_ownbound` fails the file if the fixture
//! stops producing a modelled row with a positive peak. Every arm below
//! would pass on an all-unmodelled report, where every row paints `—`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod limits_fixture;

use limits_fixture::{Cut, faces, hovers, inspector_text, load_report, simulated_state};
use rs_cam_core::tool_load::ToolpathLoadVerdict;
use rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM;
use rs_cam_core::tool_load::verdict::{Confidence, CriterionKind, CriterionStatus, LoadState};

/// The label a row paints on its face. It is `CriterionKind::label`, the
/// crate's own word, because `SURVEY_UI.md` §4 counted the SAME three
/// verdicts rendering in six vocabularies — `advance/tooth` / `advance/t` /
/// `Chip`, `L/D` / `defl` / `deflection` — and a shared scale has to pick
/// one. The core label is the one every other surface can also read.
fn face_label(kind: CriterionKind) -> String {
    kind.label().to_owned()
}

/// The face of the row for `kind`: the single-line run that begins with the
/// row's label.
fn row_face(texts: &[String], kind: CriterionKind) -> String {
    let prefix = format!("{} ", face_label(kind));
    faces(texts)
        .into_iter()
        .find(|t| t.starts_with(&prefix))
        .unwrap_or_else(|| {
            panic!(
                "no face painted for the {:?} row. Runs were {:#?}",
                kind,
                faces(texts)
            )
        })
}

/// The integer percent a face states, or `None` when the face states no
/// percent (the `—`, `∅` and `BURN` cases).
fn face_percent(face: &str) -> Option<i32> {
    let at = face.find('%')?;
    face[..at]
        .rsplit(' ')
        .next()
        .and_then(|n| n.parse::<i32>().ok())
}

/// The one milling verdict the fixture simulates.
///
/// `CriterionStatus` borrows the verdict it came from, so an arm holds the
/// report in a local and takes its rows from that.
fn only_verdict(report: &rs_cam_core::tool_load::ToolLoadReport) -> &ToolpathLoadVerdict {
    report
        .per_toolpath
        .first()
        .expect("the fixture simulates one toolpath")
}

/// The milling kinds a milling toolpath must paint, in `criteria()` order.
const MILLING_KINDS: [CriterionKind; 5] = [
    CriterionKind::Chipload,
    CriterionKind::Power,
    CriterionKind::Deflection,
    CriterionKind::DepthOfCut,
    CriterionKind::GantryPush,
];

// ── non-vacuity ───────────────────────────────────────────────────────────

/// The instrument check. Without a modelled row carrying a positive peak,
/// every arm below is satisfied by a page of dashes.
#[test]
fn the_fixture_measures_something_g_ownbound() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let modelled: Vec<&CriterionStatus<'_>> = criteria
        .iter()
        .filter(|s| {
            s.state == LoadState::Within
                && !s.is_vacuous()
                && s.display_peak.is_some_and(|p| p > 0.0)
                && s.bound.is_some_and(|b| b > 0.0)
        })
        .collect();
    assert!(
        !modelled.is_empty(),
        "no criterion is modelled `Within` with a positive peak and a \
         positive bound, so every arm in this file would pass on an \
         all-unmodelled report. Rows were {:#?}",
        criteria
            .iter()
            .map(|s| (s.kind, s.state, s.display_peak, s.bound))
            .collect::<Vec<_>>()
    );
}

// ── arm 1 — the deflection row reads millimetres against its own budget ───

/// The class-2 repair, and the arm that is RED before V2: today the badge
/// divides a tip deflection in millimetres by an L over D ratio of 4.0.
#[test]
fn the_deflection_row_reads_its_own_budget_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let deflection = criteria
        .iter()
        .find(|s| s.kind == CriterionKind::Deflection)
        .expect("a deflection row");
    let Some(peak) = deflection.display_peak else {
        panic!(
            "the deflection gate did not model this fixture ({:?}); the arm \
             cannot measure the percent it exists to measure",
            deflection.unmodeled_reason
        );
    };
    assert_eq!(
        deflection.bound,
        Some(EXCEEDS_BOUND_MM),
        "the deflection row's bound must be the budget the gate judged \
         against, in millimetres"
    );

    let expected = (peak / EXCEEDS_BOUND_MM * 100.0).round() as i32;
    let texts = inspector_text(&mut state);
    let face = row_face(&texts, CriterionKind::Deflection);
    let painted = face_percent(&face)
        .unwrap_or_else(|| panic!("the deflection row painted no percent; its face was {face:?}"));
    assert_eq!(
        painted, expected,
        "the deflection row painted {painted}% for a peak of {peak:.4} mm \
         against a budget of {EXCEEDS_BOUND_MM:.3} mm, which is \
         {expected}%. A percent taken against any other denominator is a \
         reading of a quantity the gate never judged."
    );
}

// ── arm 2 — no ratio and no approximation mark on the face ────────────────

#[test]
fn no_face_carries_a_ratio_or_an_approximation_mark_g_ownbound() {
    for cut in [Cut::InsideTheRigidityCap, Cut::PastTheRigidityCap] {
        let mut state = simulated_state(cut);
        let texts = inspector_text(&mut state);
        for face in faces(&texts) {
            assert!(
                !face.contains("L/D"),
                "a face painted {face:?} on the {cut:?} fixture. `L/D` is a \
                 ratio; the deflection gate judges millimetres, and the row \
                 must name the quantity it reads."
            );
            assert!(
                !face.contains('\u{2248}'),
                "a face painted {face:?} on the {cut:?} fixture. The ruling \
                 of 2026-09-18 moves the confidence tier into the hover: \
                 every limit reads as a plain 0-to-limit figure."
            );
        }
    }
}

// ── arm 3 — one row per criterion kind, in `criteria()` order ─────────────

#[test]
fn a_milling_toolpath_paints_one_row_per_kind_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let kinds: Vec<CriterionKind> = criteria.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        MILLING_KINDS.to_vec(),
        "the fixture's verdict no longer carries the five milling criteria; \
         re-derive this arm against `criteria()` rather than dropping it"
    );

    let texts = inspector_text(&mut state);
    let painted = faces(&texts);
    let mut last = None;
    for kind in MILLING_KINDS {
        let prefix = format!("{} ", face_label(kind));
        let at = painted
            .iter()
            .position(|t| t.starts_with(&prefix))
            .unwrap_or_else(|| {
                panic!(
                    "the {kind:?} row painted no face. `draw_tool_load_badges` \
                     must iterate `criteria()`, not a hard-coded three. Runs \
                     were {painted:#?}"
                )
            });
        if let Some(previous) = last {
            assert!(
                at > previous,
                "the {kind:?} row painted out of `criteria()` order (at {at}, \
                 after {previous}). One order, so the GUI, the CLI and the \
                 MCP list the same rows the same way."
            );
        }
        last = Some(at);
    }
}

/// The gantry row is a KNOWN ABSENCE: no machine-side thrust rating exists
/// (register T-10). It paints a dash and no number.
#[test]
fn the_gantry_row_paints_a_dash_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let texts = inspector_text(&mut state);
    let face = row_face(&texts, CriterionKind::GantryPush);
    assert!(
        face.contains('\u{2014}'),
        "the gantry push row painted {face:?}. No thrust rating exists, so \
         the row states the absence — a percent there would be an absence \
         rendered as a reading."
    );
    assert_eq!(
        face_percent(&face),
        None,
        "the gantry push row painted a percent in {face:?}, against a bound \
         that does not exist"
    );
}

// ── arm 4 — the caption names the setting, the hover carries the bound ────

#[test]
fn every_modelled_row_names_its_setting_and_its_bound_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let texts = inspector_text(&mut state);
    let painted = faces(&texts);
    let all = texts.join("\n\u{2500}\u{2500}\n");

    let mut checked = 0usize;
    for status in &criteria {
        let Some(source) = status.bound_source.as_ref() else {
            continue;
        };
        let clause = status.bound_clause();
        assert!(
            !clause.is_empty(),
            "the {:?} row carries a source with no bound clause",
            status.kind
        );
        checked += 1;

        // The caption, which is the face that follows the row's own face.
        let prefix = format!("{} ", face_label(status.kind));
        let at = painted
            .iter()
            .position(|t| t.starts_with(&prefix))
            .unwrap_or_else(|| panic!("no face for the {:?} row", status.kind));
        let caption = painted.get(at + 1).unwrap_or_else(|| {
            panic!(
                "the {:?} row painted no caption under its face. W1: the \
                 provenance rides in the caption the row already has, so a \
                 measured bound and a rule of thumb keep one geometry.",
                status.kind
            )
        });
        assert!(
            caption.starts_with(source.setting()),
            "the {:?} row's caption is {caption:?} and does not open with \
             its setting {:?}. Rule 3: every limit traces back to the \
             setting that set it.",
            status.kind,
            source.setting()
        );

        assert!(
            all.contains(&clause),
            "the {:?} row's hover never painted its bound clause {clause:?}. \
             The bound reaches the operator through \
             `CriterionStatus::bound_clause`, formatted at render time. \
             Hovers were {:#?}",
            status.kind,
            hovers(&texts)
        );
    }
    assert!(
        checked > 0,
        "no row carried a bound source, so this arm checked nothing"
    );
}

/// Every figure on the face and in the caption is FORMATTED from the value it
/// describes. This arm reads the population back off the row and finds it on
/// the caption, so a caption that stopped tracking the gate's own numbers
/// fails here rather than at the machine.
#[test]
fn every_row_states_the_population_it_measured_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let texts = inspector_text(&mut state);
    let painted = faces(&texts);

    let mut checked = 0usize;
    for status in &criteria {
        let Some(population) = status.population.filter(|p| !p.is_vacuous()) else {
            continue;
        };
        checked += 1;
        let stated = format!(
            "{} of {} {}",
            population.contributing,
            population.offered,
            population.unit.plural()
        );
        assert!(
            painted.iter().any(|t| t.contains(&stated)),
            "the {:?} row never stated its population {stated:?}. W4: a \
             verdict drawn from three samples must not read like a measured \
             run. Faces were {painted:#?}",
            status.kind
        );
    }
    assert!(
        checked > 0,
        "no row stated a population, so this arm checked nothing"
    );
}

// ── arm 5 — an approximate row keeps its reason, in the hover only ────────

#[test]
fn an_approximate_row_keeps_its_reason_in_the_hover_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let criteria = only_verdict(&report).criteria();
    let approximate: Vec<(CriterionKind, &String)> = criteria
        .iter()
        .filter_map(|s| match s.confidence {
            Some(Confidence::Approximate(why)) => Some((s.kind, why)),
            _ => None,
        })
        .collect();
    if approximate.is_empty() {
        // Not a failure: a fixture whose every input is validated is a legal
        // state. Arm 2 already pins that no face carries the mark.
        return;
    }

    let texts = inspector_text(&mut state);
    let all = texts.join("\n\u{2500}\u{2500}\n");
    for (kind, why) in approximate {
        assert!(
            all.contains(why.as_str()),
            "the {kind:?} row is `Approximate({why:?})` and no painted run \
             carries the reason. Dropping the `\u{2248}` from the face moves \
             the tier into the hover; it does not delete it."
        );
        let face = row_face(&texts, kind);
        assert!(
            !face.contains(why.as_str()),
            "the {kind:?} row painted its approximation reason on the face \
             ({face:?}). The face is a 0-to-limit reading."
        );
    }
}
