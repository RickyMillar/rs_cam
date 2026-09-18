//! **G-READYLIMITS — the Readiness page answers its own question.**
//!
//! `Workspace::Readiness` carries the subtitle *"Is this safe to cut?"*. What
//! it showed for tool load was three `CountPill::verdict` counters — `within
//! 1/1`, `exceeds 0/1`, `unmodeled 0/1` — which count TOOLPATHS. They answer
//! *"how many operations are in trouble"*, and never *"how close is this cut
//! to each limit"* (`SURVEY_UI.md` §4, point 3).
//!
//! V3 replaces the `"Tool load"` `check_row` and its three pills with the
//! per-limit rows, drawn by the one renderer the Simulation Inspector uses.
//! A new panel replaces something.
//!
//! # Which toolpath the rows describe
//!
//! The WORST row per kind across the setup, not a selected toolpath.
//! `draw_readiness_layout` (`app.rs`) builds ONE centred column with no rail
//! and no viewport, so the page has no visible selection to scope a reading
//! to, and the question it asks is about the job. A row therefore names the
//! toolpath its reading came from whenever the project has more than one.
//!
//! # Rendered, not source-scanned
//!
//! Every arm runs `readiness_panel::draw` through a real `egui::Context`, in
//! the 560-point column `app.rs` gives it, and reads the painted text.
//!
//! # Non-vacuity
//!
//! `the_readiness_fixture_measures_something_g_readylimits` fails the file if
//! the fixture stops producing a modelled row with a positive peak.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod limits_fixture;

use limits_fixture::{Cut, faces, load_report, readiness_text, rigidity_cap_mm, simulated_state};
use rs_cam_core::tool_load::verdict::{CriterionKind, LoadState};

/// The three pill labels the page must no longer paint. Each one counts
/// toolpaths; none of them is a limit.
const RETIRED_PILLS: [&str; 3] = ["within", "exceeds", "unmodeled"];

/// The face of the row for `kind` on the Readiness page.
fn row_face(texts: &[String], kind: CriterionKind) -> Option<String> {
    let prefix = format!("{} ", kind.label());
    faces(texts).into_iter().find(|t| t.starts_with(&prefix))
}

/// The integer percent a face states.
fn face_percent(face: &str) -> Option<i32> {
    let at = face.find('%')?;
    face[..at]
        .rsplit(' ')
        .next()
        .and_then(|n| n.parse::<i32>().ok())
}

// ── non-vacuity ───────────────────────────────────────────────────────────

#[test]
fn the_readiness_fixture_measures_something_g_readylimits() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let report = load_report(&state);
    let verdict = report
        .per_toolpath
        .first()
        .expect("the fixture simulates one toolpath");
    let modelled = verdict.criteria().into_iter().any(|s| {
        s.state == LoadState::Within
            && !s.is_vacuous()
            && s.display_peak.is_some_and(|p| p > 0.0)
            && s.bound.is_some_and(|b| b > 0.0)
    });
    assert!(
        modelled,
        "no criterion is modelled `Within` with a positive peak, so the arms \
         below would pass on a page of dashes. Rows were {:#?}",
        verdict
            .criteria()
            .iter()
            .map(|s| (s.kind, s.state, s.display_peak, s.bound))
            .collect::<Vec<_>>()
    );
}

// ── arm 1 — the page paints the per-limit rows ────────────────────────────

#[test]
fn readiness_paints_one_row_per_limit_g_readylimits() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let texts = readiness_text(&state);
    for kind in [
        CriterionKind::Chipload,
        CriterionKind::Power,
        CriterionKind::Deflection,
        CriterionKind::DepthOfCut,
        CriterionKind::GantryPush,
    ] {
        assert!(
            row_face(&texts, kind).is_some(),
            "the Readiness page painted no {kind:?} row. It owns the \
             question \"is this safe to cut?\" and must answer it per limit. \
             Runs were {:#?}",
            faces(&texts)
        );
    }
}

// ── arm 2 — the toolpath counters are gone ────────────────────────────────

#[test]
fn readiness_paints_no_toolpath_verdict_pill_g_readylimits() {
    for cut in [Cut::InsideTheRigidityCap, Cut::PastTheRigidityCap] {
        let state = simulated_state(cut);
        let texts = readiness_text(&state);
        for face in faces(&texts) {
            for pill in RETIRED_PILLS {
                assert!(
                    !face.starts_with(&format!("{pill} ")),
                    "the Readiness page painted the retired {pill:?} pill \
                     ({face:?}) on the {cut:?} fixture. It counts toolpaths, \
                     which is not the question this page asks."
                );
            }
        }
    }
}

// ── arm 3 — a deep cut reddens the depth row and refuses no export ────────

/// The depth row is a REAL gate with a real reading, and its bound is a
/// `RigidityRuleOfThumb` — a factor times a diameter, with no published
/// source. So it reports and it does not refuse (`BoundSource::gates_export`,
/// S4). This arm pins both halves on the one fixture they exist for.
#[test]
fn a_deep_cut_reddens_the_depth_row_and_refuses_no_export_g_readylimits() {
    let state = simulated_state(Cut::PastTheRigidityCap);
    let cap = rigidity_cap_mm(&state.session);
    let report = load_report(&state);
    let verdict = report
        .per_toolpath
        .first()
        .expect("the fixture simulates one toolpath");
    let criteria = verdict.criteria();
    let depth = criteria
        .iter()
        .find(|s| s.kind == CriterionKind::DepthOfCut)
        .expect("a depth-of-cut row");
    assert_eq!(
        depth.state,
        LoadState::Exceeds,
        "a depth per pass of twice the {cap:.3} mm rigidity cap must exceed \
         it; the row read {:?} with a peak of {:?}",
        depth.state,
        depth.display_peak
    );

    let texts = readiness_text(&state);
    let face = row_face(&texts, CriterionKind::DepthOfCut)
        .unwrap_or_else(|| panic!("no depth row painted; runs were {:#?}", faces(&texts)));
    let painted =
        face_percent(&face).unwrap_or_else(|| panic!("the depth row painted no percent: {face:?}"));
    assert!(
        painted > 100,
        "the depth row painted {painted}% for a cut past its own cap \
         ({face:?}). The percent is the reading against the bound the gate \
         judged."
    );

    let refusing = rs_cam_core::gcode::refusing_exceedances(&criteria);
    assert!(
        !refusing.iter().any(|s| s.kind == CriterionKind::DepthOfCut),
        "the exceeded depth row refused the export. Its bound is a rule of \
         thumb with no published source, so it reports and does not gate \
         (`BoundSource::gates_export`). Refusing rows were {:?}",
        refusing.iter().map(|s| s.kind).collect::<Vec<_>>()
    );
}
