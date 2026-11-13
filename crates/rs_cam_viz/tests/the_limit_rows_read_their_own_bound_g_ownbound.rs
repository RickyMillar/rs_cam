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
//! # Where the rows live in the Inspector (2026-09-23)
//!
//! Sim-cut-metrics package C moved the four banded rows (chipload, power,
//! deflection, depth of cut) onto their "Cut metrics" cards. Each card draws
//! its row with the same renderer, `verdict_badge`, so every contract below
//! reads the same face, caption and hover as before. The rows with no card
//! (gantry push) follow the cards, so the Inspector still lists every row
//! once, in `criteria()` order. `every_banded_row_is_on_its_own_card_g_ownbound`
//! holds the move.
//!
//! # The rows move into the status hover (2026-09-24)
//!
//! The operator found the cards too tall. A card no longer paints the row
//! as lines under its title: the row's face, its caption and its tooltip
//! are the first lines of the hover on the card's status glyph. The card
//! builds that hover from `verdict_face`, `row_caption` and
//! `verdict_tooltip`, the parts `verdict_badge` paints, so every contract
//! below reads the same words. The arms find a row's face as a single-line
//! run (a `verdict_badge` row) or as the FIRST LINE of a hover (a card
//! row), and its caption as the next run or the next line. A hover paints
//! in the tooltip layer, so the arms no longer read a card row's position
//! among the faces; the card titles carry the order.
//!
//! # The core row (P29-10 10e, 2026-11-13)
//!
//! Every row is now a core row (`limits::CheckRow`, read through
//! `ProjectSession::limit_rows`): core writes the face, the detail and the
//! tone, and the panel only lays them out. The arms read the painted text
//! against the row: the face on the page or as the first line of a card's
//! status hover, and every detail clause in a hover.
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

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod limits_fixture;

use limits_fixture::{Cut, faces, hovers, inspector_text, op_list_text, simulated_state};
use rs_cam_core::limits::{CheckRow, CheckState, LimitId, NoNumberReason, Tone};
use rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM;
use rs_cam_core::tool_load::verdict::{Confidence, LoadState};
use rs_cam_viz::state::AppState;

/// The core rows of the one toolpath the fixture simulates, through the
/// memo the panels read.
fn rows_of_only_toolpath(state: &AppState) -> Vec<CheckRow> {
    let set = state.simulation.cached_limit_rows(state.session());
    set.first()
        .expect("the fixture simulates one toolpath")
        .rows
        .clone()
}

/// The headline row of `limit`.
fn headline(rows: &[CheckRow], limit: LimitId) -> Option<&CheckRow> {
    rows.iter()
        .find(|r| r.is_headline() && r.check().limit == limit)
}

/// The headline chip row (chip max, or chip min when the chip exceeds low).
fn chip_row(rows: &[CheckRow]) -> &CheckRow {
    headline(rows, LimitId::ChipMax)
        .or_else(|| headline(rows, LimitId::ChipMin))
        .expect("a chip row")
}

/// A painted run with the hover mark that a table row with a hover paints
/// after its name taken off.
fn bare(run: &str) -> String {
    run.strip_suffix(" \u{24D8}").unwrap_or(run).to_owned()
}

/// The single-line runs, each without its hover mark.
fn page_faces(texts: &[String]) -> Vec<String> {
    faces(texts).iter().map(|t| bare(t)).collect()
}

/// Every run that can carry a row's face: each single-line run, and the
/// first line of each hover.
fn face_runs(texts: &[String]) -> Vec<String> {
    let mut runs = page_faces(texts);
    for hover in hovers(texts) {
        if let Some(first) = hover.lines().next() {
            runs.push(first.to_owned());
        }
    }
    runs
}

/// The card titles of the banded gates, in card order
/// (`DistributionMetric::text`), then the force card (P17).
const CARD_TITLES: [&str; 3] = ["Chipload", "Spindle power", "Tool deflection"];
const FORCE_CARD_TITLE: &str = "Cut force";

/// True when `run` is the title of the card `title`. A card core could not
/// measure is a row of the abstention table, whose name paints with the
/// detail mark.
fn is_card_title(run: &str, title: &str) -> bool {
    run == title || run == format!("{title} \u{24D8}")
}

// ── non-vacuity ───────────────────────────────────────────────────────────

/// The instrument check. Without a headline row inside its bound with a
/// positive value, every arm below is satisfied by a page of dashes.
#[test]
fn the_fixture_measures_something_g_ownbound() {
    let state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    assert!(
        rows.iter().any(|r| r.is_headline()
            && r.tone() == Tone::Within
            && r.check().value.is_some_and(|v| v > 0.0)
            && r.check().margin.is_some()),
        "no headline row is `Within` with a value and a margin. Rows were {:#?}",
        rows.iter().map(CheckRow::face).collect::<Vec<_>>()
    );
}

// ── arm 1 — the deflection row reads millimetres against its own budget ───

/// The class-2 repair: the deflection row compares the tip deflection in
/// millimetres with the budget the gate judged, never an L over D ratio.
/// The face the card carries is the core face of that row.
#[test]
fn the_deflection_row_reads_its_own_budget_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let row = headline(&rows, LimitId::ToolDeflection).expect("a deflection row");
    assert_eq!(
        row.check().bound,
        Some(EXCEEDS_BOUND_MM),
        "the deflection row's bound must be the budget the gate judged \
         against, in millimetres"
    );
    assert_eq!(row.check().unit, "mm");
    assert!(row.check().value.is_some(), "the gate modelled the fixture");
    let texts = inspector_text(&mut state);
    assert!(
        face_runs(&texts).iter().any(|t| t == row.face()),
        "the deflection card does not carry the core face {:?}",
        row.face()
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
                 ratio; the deflection gate judges millimetres."
            );
            assert!(
                !face.contains('\u{2248}'),
                "a face painted {face:?} on the {cut:?} fixture. The ruling \
                 of 2026-09-18 moves the confidence tier into the hover."
            );
        }
    }
}

// ── arm 3 — every row of the default view paints once, in order ───────────

#[test]
fn a_milling_toolpath_paints_one_row_per_kind_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let texts = inspector_text(&mut state);
    let runs = face_runs(&texts);
    let mut checked = 0;
    for row in rows.iter().filter(|r| r.in_default_view()) {
        checked += 1;
        assert!(
            runs.iter().any(|t| t == row.face()),
            "the row {:?} painted no face. The Inspector draws every row of \
             the default view: a carded gate in its card's status hover, the \
             rest in \"Other limits\". Runs were {runs:#?}",
            row.face()
        );
    }
    assert!(checked >= 5, "non-vacuity: the milling rows ({checked})");
    // The order: the gantry and depth rows, which have no card, paint after
    // the last card title.
    let painted = page_faces(&texts);
    for limit in [LimitId::GantryPush, LimitId::ForceBudget] {
        let row = headline(&rows, limit).expect("a headline row");
        let at = painted
            .iter()
            .position(|t| t == row.face())
            .unwrap_or_else(|| panic!("{limit:?} paints in \"Other limits\""));
        for title in CARD_TITLES.iter().chain([&FORCE_CARD_TITLE]) {
            let card = painted
                .iter()
                .position(|t| is_card_title(t, title))
                .unwrap_or_else(|| panic!("no card titled {title:?}. Runs were {painted:#?}"));
            assert!(
                card < at,
                "the {limit:?} row painted before the {title:?} card"
            );
        }
    }
}

/// Each banded row is painted ONCE, on its own card (the first line of
/// the card's status hover), and never as a face on the page.
#[test]
fn every_banded_row_is_on_its_own_card_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let texts = inspector_text(&mut state);
    let painted = page_faces(&texts);
    let titles: Vec<usize> = CARD_TITLES
        .iter()
        .map(|title| {
            painted
                .iter()
                .position(|t| is_card_title(t, title))
                .unwrap_or_else(|| panic!("no card titled {title:?}. Runs were {painted:#?}"))
        })
        .collect();
    for pair in titles.windows(2) {
        assert!(pair[0] < pair[1], "the cards are out of order: {titles:?}");
    }
    for row in [
        chip_row(&rows),
        headline(&rows, LimitId::SpindlePower).expect("a power row"),
        headline(&rows, LimitId::ToolDeflection).expect("a deflection row"),
    ] {
        let in_hovers = hovers(&texts)
            .iter()
            .filter(|h| h.lines().next() == Some(row.face()))
            .count();
        assert!(
            in_hovers >= 1,
            "the card hover does not open with the row face {:?}",
            row.face()
        );
        // The "Cut groups" drawer lists every row again by design; the
        // fixture context paints a closed drawer too
        // (`set_everything_is_visible`), so the page ends at its header.
        let page_end = painted
            .iter()
            .position(|t| t.starts_with("Cut groups"))
            .unwrap_or(painted.len());
        assert!(
            !painted[..page_end].iter().any(|t| t == row.face()),
            "the row {:?} painted a face line on the page: a second copy. \
             Runs were {painted:#?}",
            row.face()
        );
    }
    // Feeds ruling R2: a depth with no cap is the core `NoBound` row; it
    // draws with no invented limit and its hover gives the core clause.
    let depth = headline(&rows, LimitId::ForceBudget).expect("a depth row");
    if depth.check().state == CheckState::NoNumber(NoNumberReason::NoBound) {
        let clause = NoNumberReason::NoBound.clause();
        assert!(
            hovers(&texts).iter().any(|h| h.contains(clause)),
            "the depth row reported no bound and no hover gave {clause:?}"
        );
    }
}

/// The gantry row is a KNOWN ABSENCE: no machine-side thrust rating exists
/// (register T-10). Its face states no percent.
#[test]
fn the_gantry_row_paints_a_dash_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let gantry = headline(&rows, LimitId::GantryPush).expect("a gantry row");
    assert!(
        !gantry.face().contains('%'),
        "the gantry push row reads {:?}: a percent against a bound that does \
         not exist",
        gantry.face()
    );
    let texts = inspector_text(&mut state);
    assert!(page_faces(&texts).iter().any(|t| t == gantry.face()));
}

// ── arm 4 — the hover names the setting and carries the bound ─────────────

/// Every clause of each painted row's core detail reaches a hover: the
/// value and the bound, the setting that sets the bound, its source clause
/// and the population.
#[test]
fn every_modelled_row_names_its_setting_and_its_bound_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let texts = inspector_text(&mut state);
    let all = hovers(&texts).join("\n");
    let mut checked = 0usize;
    for row in rows.iter().filter(|r| r.in_default_view()) {
        let Some(basis) = &row.check().basis else {
            continue;
        };
        checked += 1;
        let setting = format!("the {} sets the bound", basis.setting());
        assert!(
            row.detail().contains(&setting),
            "the core detail of {:?} does not name its setting",
            row.face()
        );
        for clause in row.detail().split("; ") {
            assert!(
                all.contains(clause),
                "the row {:?} never painted its clause {clause:?}",
                row.face()
            );
        }
    }
    assert!(
        checked > 0,
        "no row carried a basis, so this arm checked nothing"
    );
}

/// The population of each row reaches the screen (W4: a three-sample
/// verdict does not read like a measured run).
#[test]
fn every_row_states_the_population_it_measured_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let rows = rows_of_only_toolpath(&state);
    let texts = inspector_text(&mut state);
    let mut checked = 0usize;
    for row in rows.iter().filter(|r| r.in_default_view()) {
        let Some(p) = row.check().population.filter(|p| !p.is_vacuous()) else {
            continue;
        };
        checked += 1;
        let stated = format!("{} of {} samples", p.contributing, p.offered);
        assert!(
            texts.iter().any(|t| t.contains(&stated)),
            "the row {:?} never stated its population {stated:?}",
            row.face()
        );
    }
    assert!(
        checked > 0,
        "no row stated a population, so this arm checked nothing"
    );
}

// ── arm 5 — the op list's triage strip paints no approximation mark ──────

/// The ruling is about the FACE of any reading, and a triage chip is a face.
/// A row inside its bound raises no flag, whatever its confidence tier.
#[test]
fn the_op_list_paints_no_approximation_mark_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = rs_cam_viz::ui::readiness::load_report(&state);
    let approximate = report
        .per_toolpath
        .first()
        .expect("one toolpath")
        .criteria()
        .into_iter()
        .any(|s| {
            s.state == LoadState::Within && matches!(s.confidence, Some(Confidence::Approximate(_)))
        });
    assert!(
        approximate,
        "the fixture carries no `Within` + `Approximate` criterion, so this \
         arm would pass on a strip that still paints the mark"
    );
    let texts = op_list_text(&mut state);
    for text in &texts {
        assert!(
            !text.contains('\u{2248}'),
            "the Simulation op list painted {text:?}. A row inside its bound \
             raises no flag; the reason lives in the hover."
        );
    }
    assert!(!texts.is_empty(), "the op list painted nothing at all");
}

/// An approximate gate keeps its reason, in the hover only (the core
/// detail names it).
#[test]
fn an_approximate_row_keeps_its_reason_in_the_hover_g_ownbound() {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let report = rs_cam_viz::ui::readiness::load_report(&state);
    let approximate: Vec<String> = report
        .per_toolpath
        .first()
        .expect("one toolpath")
        .criteria()
        .iter()
        .filter_map(|s| match s.confidence {
            Some(Confidence::Approximate(why)) => Some(why.clone()),
            Some(Confidence::Validated) | None => None,
        })
        .collect();
    if approximate.is_empty() {
        // Not a failure: a fixture whose every input is validated is legal.
        return;
    }
    let texts = inspector_text(&mut state);
    let all = hovers(&texts).join("\n");
    for why in approximate {
        // The hover sets each detail clause on its own line.
        assert!(
            all.contains(why.replace("; ", "\n").as_str()),
            "an `Approximate({why:?})` gate and no hover carries the reason"
        );
        assert!(
            !faces(&texts).iter().any(|f| f.contains(why.as_str())),
            "the approximation reason {why:?} painted on a face"
        );
    }
}
