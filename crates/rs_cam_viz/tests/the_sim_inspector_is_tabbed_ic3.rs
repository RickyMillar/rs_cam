//! IC3: the Simulation inspector is one tab strip, it opens on the cut-metric
//! charts, and it opens wide enough to show them.
//!
//! # The request this holds
//!
//! Operator, 2026-10-13, after inspector clean-up round 2: "can we de-clutter
//! by moving the sections into tabs. default to showing cut metric graphs.
//! Tabs for warning (with the count as an icon) tab for navigation and
//! similar? Also default the side bar to be opened fully (it default clips
//! the right side, you dont see things like '-- Not measurable'). Also
//! instead of that not measurable can you just replace the -- with 'N/A' and
//! remove the extra tag on the right."
//!
//! # The arms
//!
//! - (a) The default tab is Metrics, and it draws the cut-metric cards. The
//!   Warnings, Navigate and Project content is not on the page.
//! - (b) The Warnings tab label carries the triage count; no badge at 0.
//! - (c) A not-measured `DataTable` row paints "N/A" and no status chip. The
//!   status word is on the hover.
//! - (d) At `SIM_DIAGNOSTICS_DEFAULT_WIDTH` no text run of the Metrics tab is
//!   elided or paints past the content edge. The arm also measures the
//!   narrowest such width and holds the constant to it.
//!
//! The fixture is `limits_fixture`: the demo pocket, generated and simulated
//! through the shipped path, so the cards read a fresh cut trace.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod limits_fixture;

use limits_fixture::{Cut, faces, hovers, inspector_text, simulated_state};
use rs_cam_viz::app::{
    SIDE_PANEL_MAX_WIDTH, SIM_DIAGNOSTICS_DEFAULT_WIDTH, SidePanelEdge, side_panel,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::simulation::SimInspectorTab;
use rs_cam_viz::ui::components::tab_strip::count_text;
use rs_cam_viz::ui::components::{DataRow, DataTable, NOT_MEASURED_TEXT, Role, TabBadge, TabStrip};
use rs_cam_viz::ui::sim_diagnostics::warnings_badge;
use rs_cam_viz::ui::tokens;

/// The tolerance for layout rounding, in points.
const SLACK: f32 = 0.5;

/// The step of the width sweep, in points. The constant may be at most one
/// step wider than the measured width.
const STEP: f32 = 8.0;

/// The card titles of the fixture's banded rows
/// (`sim_diagnostics::CutMetricSpec::of(..).title`).
const CARD_TITLES: [&str; 4] = [
    "Chipload",
    "Spindle power",
    "Tool deflection",
    "Depth of cut",
];

/// True when `run` is the title of the card `title`: a measured card paints
/// the title, an abstention row paints the title and the detail mark.
fn is_card_title(run: &str, title: &str) -> bool {
    run == title || run == format!("{title} {}", tokens::GLYPH_DETAIL)
}

// ── arm (a) — the default tab is Metrics, and it draws the charts ─────────

#[test]
fn the_default_tab_is_metrics_and_it_draws_the_charts_ic3() {
    assert_eq!(
        AppState::new().simulation.inspector_tab,
        SimInspectorTab::Metrics,
        "the Simulation inspector must open on the Metrics tab"
    );
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let texts = inspector_text(&mut state);
    let painted = faces(&texts);
    for title in CARD_TITLES {
        assert!(
            painted.iter().any(|t| is_card_title(t, title)),
            "the default tab did not paint the {title:?} card. Runs were {painted:#?}"
        );
    }
    // Every tab label is on the strip.
    for tab in SimInspectorTab::ALL {
        assert!(
            painted.iter().any(|t| t.starts_with(tab.label())),
            "the tab strip did not paint {:?}. Runs were {painted:#?}",
            tab.label()
        );
    }
    // The content of the other tabs is not on the page.
    for other in [
        "Findings",
        "Top hotspots",
        "Now playing",
        "Span",
        "Cut distance",
    ] {
        assert!(
            !painted.iter().any(|t| t == other),
            "the Metrics tab painted {other:?}, which belongs to another tab"
        );
    }
    // The charts come first: the first card title is before the first
    // table row of other limits.
    let first_card = painted
        .iter()
        .position(|t| CARD_TITLES.iter().any(|title| is_card_title(t, title)))
        .expect("a card title");
    if let Some(other) = painted.iter().position(|t| t == "Other limits") {
        assert!(
            first_card < other,
            "the charts must come before the other limits"
        );
    }
}

#[test]
fn each_tab_draws_its_own_content_ic3() {
    let cases = [
        (SimInspectorTab::Warnings, "Findings"),
        (SimInspectorTab::Navigate, "Now playing"),
        (SimInspectorTab::Project, "Cut distance"),
    ];
    for (tab, anchor) in cases {
        let mut state = simulated_state(Cut::InsideTheRigidityCap);
        state.simulation.inspector_tab = tab;
        let painted = faces(&inspector_text(&mut state));
        assert!(
            painted.iter().any(|t| t == anchor),
            "the {tab:?} tab did not paint {anchor:?}. Runs were {painted:#?}"
        );
        for title in CARD_TITLES {
            assert!(
                !painted.iter().any(|t| t == title),
                "the {tab:?} tab painted the {title:?} card; the cards are on Metrics"
            );
        }
    }
}

// ── arm (b) — the Warnings tab label carries the count ────────────────────

#[test]
fn the_warnings_badge_counts_the_triage_ic3() {
    assert_eq!(warnings_badge(0, 0), None, "no badge at 0");
    assert_eq!(
        warnings_badge(0, 2),
        Some(TabBadge::Count(2, Role::Caution)),
        "actions alone take the caution role"
    );
    assert_eq!(
        warnings_badge(1, 2),
        Some(TabBadge::Count(3, Role::Danger)),
        "one safety item takes the danger role, and the count is the sum"
    );
}

/// The painted text runs of one strip.
fn strip_text(badge: Option<TabBadge>) -> Vec<String> {
    limits_fixture::painted(|ui| {
        let mut tab = SimInspectorTab::Metrics;
        let _ = TabStrip::new(&mut tab)
            .tab(SimInspectorTab::Metrics, "Metrics")
            .tab_badge(SimInspectorTab::Warnings, "Warnings", badge)
            .show(ui);
    })
}

#[test]
fn the_warnings_label_carries_the_count_ic3() {
    let with = strip_text(warnings_badge(1, 2));
    let label = format!("Warnings {}", count_text(3));
    assert!(
        with.contains(&label),
        "the Warnings tab must read {label:?}. Runs were {with:#?}"
    );
    let without = strip_text(warnings_badge(0, 0));
    assert!(
        without.iter().any(|t| t == "Warnings"),
        "with no item the tab reads \"Warnings\" alone. Runs were {without:#?}"
    );
    assert!(
        !without.iter().any(|t| t.contains(&count_text(0))),
        "a count of 0 shows no badge"
    );

    // The real inspector paints the label with the triage count.
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    let (safety, actions) = {
        let session = &state.session;
        let triage = state.simulation.cached_simulation_triage(session);
        (triage.safety.len(), triage.actions.len())
    };
    let painted = faces(&inspector_text(&mut state));
    let expected = match safety + actions {
        0 => "Warnings".to_owned(),
        n => format!("Warnings {}", count_text(n)),
    };
    assert!(
        painted.contains(&expected),
        "the inspector must paint the tab {expected:?}. Runs were {painted:#?}"
    );
}

// ── arm (c) — a not-measured row reads N/A with no chip ───────────────────

#[test]
fn a_not_measured_row_reads_na_with_no_chip_ic3() {
    let word = "not measurable at 0.20 mm cell";
    let texts = limits_fixture::painted(|ui| {
        ui.set_max_width(240.0);
        let _ = DataTable::new("ic3_na")
            .row(
                DataRow::not_measured("Radial engagement")
                    .status(Role::Unknown, word)
                    .hover("The dexel cell is too coarse for the tip."),
            )
            .show(ui);
    });
    let painted = faces(&texts);
    assert!(
        painted.iter().any(|t| t == NOT_MEASURED_TEXT),
        "the value cell must read {NOT_MEASURED_TEXT:?}. Runs were {painted:#?}"
    );
    assert!(
        !painted.iter().any(|t| t.contains(word)),
        "the status word painted as a chip on the row. Runs were {painted:#?}"
    );
    assert!(
        !painted.iter().any(|t| t.contains(tokens::GLYPH_UNKNOWN)),
        "the value cell still paints the dash. Runs were {painted:#?}"
    );
    // The reason is on the hover, as its first line.
    let hover = hovers(&texts);
    assert!(
        hover
            .iter()
            .any(|t| t.starts_with("Not measurable at 0.20 mm cell\n")),
        "the status word must open the row hover. Hovers were {hover:#?}"
    );
}

// ── arm (d) — the default width shows the Metrics tab whole ───────────────

/// What one frame of the Metrics tab painted, at one panel width.
struct Fit {
    /// Text runs that `truncate` cut short.
    elided: Vec<String>,
    /// Text runs that paint past the content edge.
    past_edge: Vec<String>,
    /// True when the content drew at least one card title.
    drew_cards: bool,
}

/// Draw the Metrics tab of the fixture through the real `side_panel` at
/// `width`, two frames, and read the second.
fn fit_at(width: f32) -> Fit {
    let mut state = simulated_state(Cut::InsideTheRigidityCap);
    // No `set_everything_is_visible`: a hover is not part of the tab face.
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut fit = Fit {
        elided: Vec::new(),
        past_edge: Vec::new(),
        drew_cards: false,
    };
    let mut content_right = 0.0_f32;
    for pass in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                limits_fixture::SCREEN,
            )),
            ..Default::default()
        };
        let AppState {
            simulation,
            session,
            gui,
            ..
        } = &mut state;
        let mut out = ctx.run_ui(input, |ui| {
            side_panel(ui, SidePanelEdge::Right, "sim_diagnostics", width, |ui| {
                content_right = ui.max_rect().right();
                let mut events = Vec::new();
                rs_cam_viz::ui::sim_diagnostics::draw(ui, simulation, session, gui, &mut events);
            });
        });
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let run = text.galley.job.text.clone();
                    if CARD_TITLES.iter().any(|title| is_card_title(&run, title)) {
                        fit.drew_cards = true;
                    }
                    if text.galley.elided {
                        fit.elided.push(run.clone());
                    }
                    let right = text.pos.x + text.galley.rect.right();
                    if right > content_right + SLACK {
                        fit.past_edge
                            .push(format!("{run} (+{:.2})", right - content_right));
                    }
                }
            }
        }
        out.textures_delta.clear();
    }
    fit
}

#[test]
fn the_default_width_shows_the_metrics_tab_whole_ic3() {
    // The measurement: the narrowest width, in steps of STEP from the old
    // 240-point rail, at which the tab shows whole.
    let mut needed = None;
    let mut width = 240.0_f32;
    while width <= SIDE_PANEL_MAX_WIDTH {
        let fit = fit_at(width);
        if fit.elided.is_empty() && fit.past_edge.is_empty() {
            needed = Some(width);
            break;
        }
        eprintln!(
            "ic3: at {width} points the Metrics tab cut short {:?}, past the edge {:?}",
            fit.elided, fit.past_edge
        );
        width += STEP;
    }
    let needed = needed.expect("no width up to SIDE_PANEL_MAX_WIDTH shows the Metrics tab whole");
    eprintln!("ic3: the Metrics tab shows whole from {needed} points");
    let fit = fit_at(SIM_DIAGNOSTICS_DEFAULT_WIDTH);
    assert!(
        fit.drew_cards,
        "the Metrics tab drew no card at the default width; the arm is vacuous"
    );
    assert!(
        fit.elided.is_empty() && fit.past_edge.is_empty(),
        "at the default width {SIM_DIAGNOSTICS_DEFAULT_WIDTH} the Metrics tab cut \
         {:?} short and painted {:?} past the edge",
        fit.elided,
        fit.past_edge
    );

    assert!(
        SIM_DIAGNOSTICS_DEFAULT_WIDTH <= needed + STEP,
        "SIM_DIAGNOSTICS_DEFAULT_WIDTH is {SIM_DIAGNOSTICS_DEFAULT_WIDTH}, more than one \
         {STEP}-point step over the measured {needed}. Set it from the measurement."
    );
    const {
        assert!(
            SIM_DIAGNOSTICS_DEFAULT_WIDTH <= SIDE_PANEL_MAX_WIDTH,
            "the default may not pass the side-panel ceiling"
        );
    }
}
