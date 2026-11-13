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
//! A new panel replaces something. P29-10 10e: the rows are the core rows
//! (`limits::CheckRow`, `ProjectSession::limit_rows`) of the inspector's
//! default view, and worst is the one core rank (`CheckRow::worse_than`).
//!
//! # Which toolpath the rows describe
//!
//! The WORST row per limit across the setup, not a selected toolpath.
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

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod limits_fixture;

use limits_fixture::{Cut, faces, readiness_text, rigidity_cap_mm, simulated_state};
use rs_cam_core::limits::{CheckRow, LimitId, Tone};
use rs_viz_rows::rows_of_only_toolpath;

/// The three pill labels the page must no longer paint. Each one counts
/// toolpaths; none of them is a limit.
const RETIRED_PILLS: [&str; 3] = ["within", "exceeds", "unmodeled"];

mod rs_viz_rows {
    use rs_cam_core::limits::CheckRow;
    use rs_cam_viz::state::AppState;

    /// The core rows of the one toolpath the fixture simulates, through
    /// the memo the panels read (`ProjectSession::limit_rows`).
    pub fn rows_of_only_toolpath(state: &AppState) -> Vec<CheckRow> {
        let set = state.simulation.cached_limit_rows(state.session());
        set.first()
            .expect("the fixture simulates one toolpath")
            .rows
            .clone()
    }
}

/// The single-line runs, each without the hover mark that a table row
/// with a hover paints after its name.
fn page_faces(texts: &[String]) -> Vec<String> {
    faces(texts)
        .iter()
        .map(|t| t.strip_suffix(" \u{24D8}").unwrap_or(t).to_owned())
        .collect()
}

/// The headline row of `limit` (the op's own cut group, the stock
/// population).
fn headline(rows: &[CheckRow], limit: LimitId) -> Option<&CheckRow> {
    rows.iter()
        .find(|r| r.is_headline() && r.check().limit == limit)
}

// ── non-vacuity ───────────────────────────────────────────────────────────

#[test]
fn the_readiness_fixture_measures_something_g_readylimits() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let modelled = rows.iter().any(|r| {
        r.is_headline()
            && r.tone() == Tone::Within
            && r.check().value.is_some_and(|v| v > 0.0)
            && r.check().margin.is_some()
    });
    assert!(
        modelled,
        "no headline row is `Within` with a positive value and a margin, so \
         the arms below would pass on a page of dashes. Rows were {:#?}",
        rows.iter().map(CheckRow::face).collect::<Vec<_>>()
    );
}

// ── arm 1 — the page paints the per-limit rows ────────────────────────────

/// Every row of the default view (the headline rows and every row that
/// exceeds, except a `NotApplicable` one) paints its core face. One
/// toolpath, so each row is its own worst.
#[test]
fn readiness_paints_one_row_per_limit_g_readylimits() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let texts = readiness_text(&state);
    let painted = page_faces(&texts);
    let mut checked = 0;
    for row in rows.iter().filter(|r| r.in_default_view()) {
        checked += 1;
        assert!(
            painted.iter().any(|t| t == row.face()),
            "the Readiness page painted no row with the core face {:?}. It \
             owns the question \"is this safe to cut?\" and must answer it per \
             limit. Runs were {painted:#?}",
            row.face()
        );
    }
    assert!(checked >= 5, "non-vacuity: the milling rows ({checked})");
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
/// `MachineForceBudget` — a budget that stands in for an unmeasured frame.
/// So it reports and it does not refuse (`BoundSource::gates_export`,
/// S4): its tone is the advisory one. This arm pins both halves on the one
/// fixture they exist for.
#[test]
fn a_deep_cut_reddens_the_depth_row_and_refuses_no_export_g_readylimits() {
    let state = simulated_state(Cut::PastTheRigidityCap);
    let cap = rigidity_cap_mm(state.session());
    let rows = rows_of_only_toolpath(&state);
    let depth = headline(&rows, LimitId::ForceBudget).expect("a depth-of-cut row");
    assert!(
        depth.tone().exceeds(),
        "a depth per pass of twice the {cap:.3} mm rigidity cap must exceed \
         it; the row read {:?} ({:?})",
        depth.tone(),
        depth.face()
    );
    assert_eq!(
        depth.tone(),
        Tone::ExceedsAdvisory,
        "the force budget does not gate an export, so the row is a caution"
    );

    let texts = readiness_text(&state);
    assert!(
        page_faces(&texts).iter().any(|t| t == depth.face()),
        "no depth row painted with the core face {:?}; runs were {:#?}",
        depth.face(),
        faces(&texts)
    );
    assert!(
        depth.face().contains("over"),
        "the depth row reads its share over the cap: {:?}",
        depth.face()
    );

    let report = rs_cam_viz::ui::readiness::load_report(&state);
    let criteria = report
        .per_toolpath
        .first()
        .expect("the fixture simulates one toolpath")
        .criteria();
    let warning = rs_cam_core::gcode::warning_exceedances(&criteria);
    assert!(
        !warning
            .iter()
            .any(|s| s.kind == rs_cam_core::tool_load::verdict::CriterionKind::DepthOfCut),
        "the exceeded depth row warned on the export. Its bound is a rule of \
         thumb with no published source, so it reports and does not gate \
         (`BoundSource::gates_export`). Warning rows were {:?}",
        warning.iter().map(|s| s.kind).collect::<Vec<_>>()
    );
}
