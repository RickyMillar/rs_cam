//! Unit tests for the inspector's tab enum and its diagnostic-row merge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::{ToolpathTab, merge_stateful_gate_rows};

/// The MCP `set_ui_view` tool documents these tab keys — every
/// documented key must parse, every tab must be reachable, and
/// unknown keys must stay `None` (set_ui_view validates upstream,
/// this is the backstop).
#[test]
fn toolpath_tab_parse_covers_all_documented_keys() {
    assert!(matches!(
        ToolpathTab::parse("geometry"),
        Some(ToolpathTab::Geometry)
    ));
    assert!(matches!(
        ToolpathTab::parse("feeds"),
        Some(ToolpathTab::FeedsSpeeds)
    ));
    assert!(matches!(
        ToolpathTab::parse("feeds_speeds"),
        Some(ToolpathTab::FeedsSpeeds)
    ));
    assert!(matches!(
        ToolpathTab::parse("linking"),
        Some(ToolpathTab::Linking)
    ));
    assert!(matches!(
        ToolpathTab::parse("heights"),
        Some(ToolpathTab::Heights)
    ));
    assert!(matches!(
        ToolpathTab::parse("dressup"),
        Some(ToolpathTab::Dressup)
    ));
    assert!(ToolpathTab::parse("not_a_tab").is_none());
    // Every variant in ALL is reachable through some key.
    for &tab in ToolpathTab::ALL {
        let key = match tab {
            ToolpathTab::Geometry => "geometry",
            ToolpathTab::FeedsSpeeds => "feeds",
            ToolpathTab::Linking => "linking",
            ToolpathTab::Heights => "heights",
            ToolpathTab::Dressup => "dressup",
        };
        assert!(ToolpathTab::parse(key).is_some());
    }
}

fn stateful_diag(
    source: rs_cam_core::diagnostics::Source,
    message: &str,
) -> rs_cam_core::diagnostics::Diagnostic {
    use rs_cam_core::diagnostics as dx;
    dx::Diagnostic {
        id: dx::DiagnosticId::new("test.gate"),
        scope: dx::Scope::Toolpath {
            id: rs_cam_core::ToolpathId(0),
        },
        category: dx::Category::State,
        severity: dx::Severity::Info,
        confidence: dx::Confidence::Static,
        state: dx::DiagnosticState::StaleEvidence,
        source,
        message: message.to_owned(),
        evidence: None,
        fix: None,
        supersedes: Vec::new(),
        suppressed_diagnostics: Vec::new(),
    }
}

/// Density pass V3 — three per-gate copies of the same stale-sim
/// sentence must collapse into one "Gates: …" row; gate rows with a
/// unique status and non-gate rows pass through untouched.
#[test]
fn stateful_gate_rows_with_shared_status_merge_into_one_gates_row() {
    use rs_cam_core::diagnostics::Source;
    let stale = "simulation stale — re-run to verify";
    let chipload = stateful_diag(Source::ToolLoad, &format!("Chipload: {stale}"));
    let power = stateful_diag(Source::ToolLoad, &format!("Power: {stale}"));
    let deflection = stateful_diag(Source::ToolLoad, &format!("Deflection: {stale}"));
    let unique_gate = stateful_diag(
        Source::ToolLoad,
        "Drill: no vendor LUT row matches this tool/material — supply vendor data \
         to enable this gate",
    );
    let non_gate = stateful_diag(Source::StaticValidation, "Heights: needs simulation");

    let stateful = vec![&chipload, &power, &deflection, &unique_gate, &non_gate];
    let (merged, rest) = merge_stateful_gate_rows(&stateful);

    assert_eq!(merged.len(), 1, "three shared-status gates → one row");
    assert_eq!(merged[0].message, format!("Gates: {stale}"));
    assert_eq!(rest.len(), 2, "unique gate + non-gate pass through");
    assert!(rest.iter().any(|d| d.message.starts_with("Drill:")));
    assert!(rest.iter().any(|d| d.message.starts_with("Heights:")));
}

/// Both wordings ship today — "run simulation to evaluate" (never
/// simulated) must merge independently of the stale wording.
#[test]
fn stateful_gate_rows_merge_groups_by_exact_status_text() {
    use rs_cam_core::diagnostics::Source;
    let chipload = stateful_diag(Source::ToolLoad, "Chipload: run simulation to evaluate");
    let power = stateful_diag(Source::ToolLoad, "Power: run simulation to evaluate");
    let deflection = stateful_diag(
        Source::ToolLoad,
        "Deflection: simulation stale — re-run to verify",
    );

    let stateful = vec![&chipload, &power, &deflection];
    let (merged, rest) = merge_stateful_gate_rows(&stateful);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].message, "Gates: run simulation to evaluate");
    assert_eq!(rest.len(), 1, "differently-worded gate stays separate");
}
