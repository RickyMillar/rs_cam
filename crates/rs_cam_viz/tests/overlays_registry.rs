//! P6 — sentries for the Overlays registry.
//!
//! The registry's whole claim is that ONE list feeds the panel, the MCP
//! `set_ui_view` `overlays` map and the per-workspace defaults, so those
//! three cannot disagree about what exists, what it is called, or why it
//! cannot draw. These tests pin the parts of that claim a reader cannot
//! check by eye:
//!
//! - **completeness** — every render-consumed `show_*` flag declared on
//!   `ViewportState` has exactly one row. A flag with no row is a control
//!   nothing lists; the 2026-09-08 audit found nineteen of them.
//! - **upload triggers** — every row that names `OverlayMechanism::UploadTime`
//!   is carried in the composite `overlay_upload_key`. This is the
//!   tool-profile ghost's defect class: an upload-time flag whose upload
//!   nothing fires is a checkbox that does nothing until an unrelated event
//!   happens to fire one (audit §3.6).
//! - **honest refusals** — a disabled row's reason is never empty, and the
//!   MCP reply quotes the panel's own string.
//! - **exclusivity** — at most one scalar-field row is on per surface.
//! - **workspace defaults** — a switch applies them and a switch back
//!   restores what they displaced.
//!
//! Two of these read SOURCE rather than behaviour, on purpose. A field
//! census cannot find a missing upload trigger — only the trigger trace can
//! (audit §4.5) — and the trigger lives in a `pub(crate)` struct an
//! integration test cannot name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use rs_cam_viz::state::{AppState, Workspace};
use rs_cam_viz::ui::overlays::registry::{
    self, OverlayMechanism, OverlaySurface, Precondition, ROWS,
};

fn source(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The inspector's own source: every `.rs` file directly in
/// `src/ui/properties/`, concatenated.
///
/// P4 (2026-09-17) split `properties/mod.rs` into `mod.rs` plus seven panel
/// children beside it. The scans below read the inspector as one surface —
/// a positive anchor may live in any child, and a negative scan is only
/// honest over all of them — so the reader is the folder. `operations/` is
/// a sub-folder and is not read; it carries its own sentries.
fn inspector_source() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no source under {}", dir.display());
    let mut out = String::new();
    for path in paths {
        out.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
        );
        out.push('\n');
    }
    out
}

/// Every `pub <name>: bool` declared inside `struct <struct_name> {` in
/// `state/viewport.rs`.
fn declared_bool_fields(struct_name: &str) -> Vec<String> {
    let text = source("src/state/viewport.rs");
    let head = format!("pub struct {struct_name} {{");
    let start = text
        .find(&head)
        .unwrap_or_else(|| panic!("`{head}` no longer exists in state/viewport.rs"))
        + head.len();
    let body = &text[start..];
    let end = body
        .find("\n}")
        .unwrap_or_else(|| panic!("`{struct_name}`'s body is unterminated"));
    body[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("pub ")?;
            let (name, ty) = rest.split_once(':')?;
            (ty.trim().trim_end_matches(',') == "bool").then(|| name.trim().to_owned())
        })
        .collect()
}

// ── completeness ───────────────────────────────────────────────────────────

/// The load-bearing sentry. A `show_*` flag with no registry row is an
/// overlay the panel does not list and `set_ui_view` cannot reach.
#[test]
fn every_viewport_flag_has_exactly_one_registry_row() {
    let mut expected: Vec<String> = declared_bool_fields("ViewportState")
        .into_iter()
        .map(|name| format!("viewport.{name}"))
        .collect();
    expected.extend(
        declared_bool_fields("SpanKindFilter")
            .into_iter()
            .map(|name| format!("viewport.span_kind_filter.{name}")),
    );
    assert!(
        expected.len() > 20,
        "the field scan found only {} flags — the parser has drifted from \
         state/viewport.rs, so this whole test is vacuous",
        expected.len()
    );

    for flag in &expected {
        let rows: Vec<&str> = ROWS
            .iter()
            .filter(|row| row.flag == Some(flag.as_str()))
            .map(|row| row.id)
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "`{flag}` is driven by {} registry rows ({rows:?}); it must be \
             exactly one — zero means no control lists it, two means two \
             homes for one state",
            rows.len()
        );
    }

    // And nothing in the other direction: a row naming a `show_*` field that
    // no longer exists would keep printing a checkbox over a field the panel
    // cannot write. Only the boolean rows are checked here — a colour choice
    // names an enum VALUE (`viewport.toolpath_color_mode.normal`), not a
    // field, and `a_radio_surface_always_holds_exactly_one_choice` is what
    // pins those.
    for row in ROWS {
        let Some(flag) = row.flag else { continue };
        let last = flag.rsplit('.').next().unwrap_or(flag);
        if !flag.starts_with("viewport.") || !last.starts_with("show_") {
            continue;
        }
        assert!(
            expected.iter().any(|f| f == flag),
            "row `{}` names `{flag}`, which state/viewport.rs no longer declares",
            row.id
        );
    }
}

#[test]
fn every_registry_id_is_unique_and_wire_safe() {
    let mut seen = BTreeMap::new();
    for row in ROWS {
        assert!(
            seen.insert(row.id, row.label).is_none(),
            "duplicate registry id `{}` — the id is the MCP wire name",
            row.id
        );
        assert!(
            !row.id.is_empty()
                && row
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "id `{}` is not a stable snake_case wire name",
            row.id
        );
        assert!(!row.label.is_empty(), "row `{}` has no label", row.id);
        assert!(!row.hover.is_empty(), "row `{}` has no hover text", row.id);
    }
}

/// A row with no flag is a row with NO RENDERER, and it must be disabled in
/// every state — otherwise the panel offers a checkbox that draws nothing.
#[test]
fn a_flagless_row_can_never_be_switched_on() {
    let mut state = AppState::new();
    let flagless: Vec<&str> = ROWS
        .iter()
        .filter(|row| row.flag.is_none())
        .map(|row| row.id)
        .collect();
    assert_eq!(
        flagless,
        vec![
            "planner_islands",
            "derived_rest_regions",
            "boundary_outline"
        ],
        "the flagless set changed — a new one needs a renderer or a reason"
    );
    for id in flagless {
        let row = registry::row(id).unwrap();
        for workspace in [
            Workspace::Setup,
            Workspace::Toolpaths,
            Workspace::Simulation,
        ] {
            state.workspace = workspace;
            let precondition = (row.precondition)(&state);
            assert!(
                !precondition.is_ready(),
                "`{id}` reported Ready in {workspace:?} while it has no renderer"
            );
            assert!(!(row.get)(&state), "`{id}` reads ON while it cannot draw");
        }
    }
}

// ── upload triggers ────────────────────────────────────────────────────────

/// Every upload-time row names a trigger, and the trigger really exists.
///
/// `overlay_upload_key` is `pub(crate)`, so this reads `app.rs` as source.
/// That is the point: a field census cannot find a missing trigger, because
/// the flag has a writer AND a reader and still does nothing (audit §4.5).
#[test]
fn every_upload_time_row_is_carried_in_the_upload_key() {
    let app_rs = source("src/app.rs");
    let key_start = app_rs
        .find("pub(crate) fn overlay_upload_key")
        .expect("app.rs no longer builds an `overlay_upload_key`");
    let key_body = &app_rs[key_start..];
    let key_end = key_body.find("\n}").expect("unterminated function");
    let key_body = &key_body[..key_end];

    let mut upload_rows = 0;
    for row in ROWS {
        let OverlayMechanism::UploadTime(trigger) = row.mechanism else {
            continue;
        };
        upload_rows += 1;
        assert!(
            !trigger.trim().is_empty(),
            "upload-time row `{}` names no trigger",
            row.id
        );
        let Some(flag) = row.flag else {
            panic!("upload-time row `{}` has no flag", row.id);
        };
        // The key reads a field, and a row can name something INSIDE that
        // field: `viewport.span_kind_filter.show_entry` is carried by
        // `state.viewport.span_kind_filter`, and the three
        // `simulation.stock_viz_mode.*` choices by
        // `state.simulation.stock_viz_mode`. So accept the whole path, or —
        // for a path of three or more segments — its parent. Never a
        // shallower prefix than that: `state.viewport` alone would match
        // anything.
        let mut candidates = vec![format!("state.{flag}")];
        if flag.split('.').count() >= 3
            && let Some((parent, _)) = flag.rsplit_once('.')
        {
            candidates.push(format!("state.{parent}"));
        }
        assert!(
            candidates.iter().any(|needle| key_body.contains(needle)),
            "row `{}` is upload-time but none of {candidates:?} appears in \
             `overlay_upload_key` — that is the tool-profile ghost defect: \
             the flag has a writer and a reader and still never reaches the \
             screen until some unrelated event fires an upload",
            row.id
        );
    }
    assert!(
        upload_rows >= 12,
        "only {upload_rows} upload-time rows found — the mechanism labels \
         have drifted and this test is vacuous"
    );
}

// ── honest refusals ────────────────────────────────────────────────────────

#[test]
fn every_disabled_reason_is_a_non_empty_sentence() {
    let mut state = AppState::new();
    let mut disabled_seen = 0;
    for workspace in [
        Workspace::Setup,
        Workspace::Toolpaths,
        Workspace::Simulation,
        Workspace::Readiness,
    ] {
        state.workspace = workspace;
        for row in ROWS {
            if let Precondition::Disabled { reason, .. } = (row.precondition)(&state) {
                disabled_seen += 1;
                assert!(
                    reason.trim().len() > 4,
                    "row `{}` in {workspace:?} is disabled with the unhelpful \
                     reason {reason:?}",
                    row.id
                );
            }
        }
    }
    assert!(
        disabled_seen > 20,
        "only {disabled_seen} disabled rows across four workspaces on an \
         empty project — the preconditions are not being exercised"
    );
}

/// The two gates the design keeps HARD render as disabled rows with a
/// reason, rather than as a checkbox that does nothing.
#[test]
fn the_two_hard_gates_are_disabled_rows_with_reasons() {
    let mut state = AppState::new();
    state.workspace = Workspace::Simulation;
    let model = registry::row("model").unwrap();
    let reason = (model.precondition)(&state)
        .reason()
        .expect("the model row must be disabled in Simulation")
        .to_owned();
    assert!(
        reason.contains("simulated stock"),
        "the model row's Simulation reason should say what replaced it, got {reason:?}"
    );

    // The tier map's setup clause is the other one. With no planner preview
    // the row is disabled for the earlier reason, which is what an empty
    // project can observe; the setup-mismatch arm is exercised by the
    // planner fixtures.
    state.workspace = Workspace::Toolpaths;
    let tier = registry::row("tier_map").unwrap();
    let reason = (tier.precondition)(&state)
        .reason()
        .expect("no preview means no tier map")
        .to_owned();
    assert!(
        reason.contains("preview"),
        "the tier map's reason should name the preview, got {reason:?}"
    );
}

// ── the MCP surface ────────────────────────────────────────────────────────

#[test]
fn set_ui_view_refuses_an_unknown_overlay_id() {
    let mut state = AppState::new();
    let requested = BTreeMap::from([("no_such_overlay".to_owned(), true)]);
    let report = registry::apply_overlays(&mut state, &requested);
    assert!(report.applied.is_empty());
    let reason = report
        .refused
        .get("no_such_overlay")
        .expect("an unknown id is refused, not dropped");
    assert!(
        reason.contains("no_such_overlay"),
        "the refusal should name the id, got {reason:?}"
    );
}

#[test]
fn set_ui_view_refuses_a_not_ready_id_with_the_panels_own_reason() {
    let mut state = AppState::new();
    state.workspace = Workspace::Toolpaths;
    let panel_reason = (registry::row("tier_map").unwrap().precondition)(&state)
        .reason()
        .expect("no planner preview on an empty project")
        .to_owned();

    let requested = BTreeMap::from([("tier_map".to_owned(), true)]);
    let report = registry::apply_overlays(&mut state, &requested);
    assert_eq!(
        report.refused.get("tier_map"),
        Some(&panel_reason),
        "the MCP refusal must quote the string the panel prints, or the two \
         surfaces disagree about why"
    );
    assert!(
        !state.viewport.show_tier_preview,
        "a refused overlay must not be switched on anyway"
    );
}

/// Switching a row OFF needs no precondition: hiding something that cannot
/// draw is harmless, and refusing it would make an agent's "clean slate"
/// call fail on an empty project.
#[test]
fn switching_an_overlay_off_is_applied_even_when_it_cannot_draw() {
    let mut state = AppState::new();
    state.workspace = Workspace::Toolpaths;
    // Reach is ON by default and cannot DRAW on an empty project (no
    // selection, so no map), which is exactly the case that must still
    // accept an "off".
    assert!(state.viewport.show_reach_map, "fixture precondition");
    assert!(!(registry::row("reach_map").unwrap().precondition)(&state).is_ready());
    let requested = BTreeMap::from([("reach_map".to_owned(), false)]);
    let report = registry::apply_overlays(&mut state, &requested);
    assert_eq!(report.applied.get("reach_map"), Some(&false));
    assert!(report.refused.is_empty());
    assert!(!state.viewport.show_reach_map);
}

/// "Off" has no meaning for a member of a radio group, so it is refused
/// rather than silently ignored — the agent asked for something the surface
/// cannot express.
#[test]
fn switching_a_colour_choice_off_is_refused() {
    let mut state = AppState::new();
    let requested = BTreeMap::from([("move_colour_palette".to_owned(), false)]);
    let report = registry::apply_overlays(&mut state, &requested);
    assert!(report.applied.is_empty());
    let reason = report.refused.get("move_colour_palette").unwrap();
    assert!(
        reason.contains("moves"),
        "the refusal should name the surface, got {reason:?}"
    );
}

#[test]
fn every_overlay_is_refused_in_the_readiness_workspace() {
    let mut state = AppState::new();
    state.workspace = Workspace::Readiness;
    let requested = BTreeMap::from([("grid".to_owned(), true), ("rapids".to_owned(), false)]);
    let report = registry::apply_overlays(&mut state, &requested);
    assert!(report.applied.is_empty());
    assert_eq!(report.refused.len(), 2);
    for reason in report.refused.values() {
        assert!(
            reason.contains("no viewport"),
            "Readiness refusals should say the workspace renders no viewport, \
             got {reason:?}"
        );
    }
}

// ── exclusivity ────────────────────────────────────────────────────────────

fn surface_on_count(state: &AppState, surface: OverlaySurface) -> usize {
    ROWS.iter()
        .filter(|row| row.surface == surface && (row.get)(state))
        .count()
}

#[test]
fn the_constructed_state_already_satisfies_per_surface_exclusivity() {
    let state = AppState::new();
    for surface in OverlaySurface::EXCLUSIVE {
        assert!(
            surface_on_count(&state, surface) <= 1,
            "{surface:?} starts with {} colour sources on",
            surface_on_count(&state, surface)
        );
    }
}

/// The operator's ruling on the model surface (2026-09-08): the reach map is
/// the always-on answer — "show the reach map when ANY finishing op is
/// selected" — and the rest heatmap is the diagnostic switched on to inspect
/// rest regions. So Reach owns the Toolpaths default and the rest heatmap is
/// off in every workspace. Reversing this pair silently would take a shipped
/// answer off the screen.
#[test]
fn reach_owns_the_model_surface_default_and_the_rest_heatmap_is_off() {
    let reach = registry::row("reach_map").unwrap();
    let rest = registry::row("rest_heatmap").unwrap();
    assert_eq!(reach.surface, OverlaySurface::Model);
    assert_eq!(rest.surface, OverlaySurface::Model);

    assert_eq!((reach.default_for)(Workspace::Toolpaths), Some(true));
    assert_eq!((rest.default_for)(Workspace::Toolpaths), Some(false));

    let state = AppState::new();
    assert!(
        state.viewport.show_reach_map,
        "the constructed state agrees"
    );
    assert!(!state.viewport.show_rest_heatmap);

    // And the pair still cannot both be on, in any workspace.
    let mut state = state;
    for workspace in [
        Workspace::Setup,
        Workspace::Toolpaths,
        Workspace::Simulation,
    ] {
        registry::switch_workspace(&mut state, workspace);
        assert!(surface_on_count(&state, OverlaySurface::Model) <= 1);
    }
}

/// The one cross-group exclusion: Reach and the rest heatmap both colour the
/// model surface, so switching one on clears the other.
#[test]
fn enabling_reach_clears_the_rest_heatmap_and_the_reverse() {
    let mut state = AppState::new();
    let reach = registry::row("reach_map").unwrap();
    let rest = registry::row("rest_heatmap").unwrap();

    registry::set_overlay(&mut state, rest, true);
    registry::set_overlay(&mut state, reach, true);
    assert!(state.viewport.show_reach_map);
    assert!(
        !state.viewport.show_rest_heatmap,
        "Reach must clear the rest heatmap — both shade the model surface"
    );

    registry::set_overlay(&mut state, rest, true);
    assert!(state.viewport.show_rest_heatmap);
    assert!(!state.viewport.show_reach_map, "and the reverse");
    assert_eq!(surface_on_count(&state, OverlaySurface::Model), 1);
}

#[test]
fn a_radio_surface_always_holds_exactly_one_choice() {
    let mut state = AppState::new();
    for surface in [OverlaySurface::Stock, OverlaySurface::Moves] {
        let members: Vec<_> = ROWS.iter().filter(|row| row.surface == surface).collect();
        assert!(members.len() >= 3, "{surface:?} lost its members");
        for row in &members {
            registry::set_overlay(&mut state, row, true);
            assert_eq!(
                surface_on_count(&state, surface),
                1,
                "{surface:?} holds {} choices after switching `{}` on",
                surface_on_count(&state, surface),
                row.id
            );
        }
    }
}

/// A territory row stacks: the tier map is not on the model surface, so it
/// survives a Reach or rest-heatmap change. The shipped rest-plus-tier pair
/// must keep working.
#[test]
fn a_territory_overlay_stacks_with_a_scalar_field() {
    let mut state = AppState::new();
    let tier = registry::row("tier_map").unwrap();
    let rest = registry::row("rest_heatmap").unwrap();
    registry::set_overlay(&mut state, tier, true);
    registry::set_overlay(&mut state, rest, true);
    assert!(state.viewport.show_tier_preview);
    assert!(state.viewport.show_rest_heatmap);
}

// ── per-workspace defaults ─────────────────────────────────────────────────

/// The app opens in Toolpaths, so the constructed state must equal that
/// column of the default table — otherwise the first frame contradicts the
/// table and every default test below starts from a state the design does
/// not describe.
#[test]
fn the_constructed_state_equals_the_toolpaths_default_column() {
    let state = AppState::new();
    assert_eq!(state.workspace, Workspace::Toolpaths);
    for row in ROWS {
        let Some(default) = (row.default_for)(Workspace::Toolpaths) else {
            continue;
        };
        assert_eq!(
            (row.get)(&state),
            default,
            "`{}` starts at {} but the Toolpaths column says {default}",
            row.id,
            (row.get)(&state)
        );
    }
    assert_eq!(registry::non_default_count(&state), 0);
}

#[test]
fn a_workspace_switch_applies_that_workspaces_defaults() {
    let mut state = AppState::new();
    registry::switch_workspace(&mut state, Workspace::Simulation);
    assert_eq!(state.workspace, Workspace::Simulation);
    for row in ROWS {
        let Some(default) = (row.default_for)(Workspace::Simulation) else {
            continue;
        };
        assert_eq!(
            (row.get)(&state),
            default,
            "`{}` did not take its Simulation default",
            row.id
        );
    }
    // The concrete promises of the table, spelled out so a silent table edit
    // has to justify itself here.
    assert!(state.viewport.show_sim_stock, "the simulated stock shows");
    assert!(state.viewport.show_collisions, "collisions show");
    assert!(!state.viewport.show_grid, "the grid hides");
    assert!(!state.viewport.show_stock_solid, "the solid block hides");
    assert!(
        state.viewport.show_alignment_pins,
        "pins stay on, so pin-drill review keeps working"
    );
    assert!(
        !state.viewport.show_reach_map,
        "the reach map draws in Toolpaths only"
    );
}

#[test]
fn switching_back_restores_what_the_defaults_displaced() {
    let mut state = AppState::new();
    // An operator override the Simulation column does NOT name: it must
    // survive the round trip untouched.
    state.viewport.show_tool_profile_preview = true;
    // And one it does name: displaced on the way in, restored on the way
    // out.
    assert!(state.viewport.show_grid);

    registry::switch_workspace(&mut state, Workspace::Simulation);
    assert!(!state.viewport.show_grid);
    assert!(
        state.viewport.show_tool_profile_preview,
        "the Simulation column names no default for the ghost, so it must be \
         left alone"
    );

    registry::switch_workspace(&mut state, Workspace::Toolpaths);
    assert!(
        state.viewport.show_grid,
        "the grid must come back when the workspace that hid it is left"
    );
    assert!(state.viewport.show_tool_profile_preview);
    assert!(state.overlays.displaced.is_empty() || state.workspace == Workspace::Toolpaths);
}

/// The Readiness workspace renders no viewport, so it names no defaults and
/// must not disturb the overlays on the way through.
#[test]
fn the_readiness_workspace_disturbs_no_overlay() {
    let mut state = AppState::new();
    let before: Vec<bool> = ROWS.iter().map(|row| (row.get)(&state)).collect();
    registry::switch_workspace(&mut state, Workspace::Readiness);
    let after: Vec<bool> = ROWS.iter().map(|row| (row.get)(&state)).collect();
    assert_eq!(before, after);
}

/// A default write goes through `set_overlay`, so it can never leave a
/// surface with two colour sources on.
#[test]
fn workspace_defaults_cannot_break_exclusivity() {
    let mut state = AppState::new();
    for workspace in [
        Workspace::Setup,
        Workspace::Simulation,
        Workspace::Toolpaths,
        Workspace::Readiness,
    ] {
        registry::switch_workspace(&mut state, workspace);
        for surface in OverlaySurface::EXCLUSIVE {
            assert!(
                surface_on_count(&state, surface) <= 1,
                "{surface:?} holds two colour sources in {workspace:?}"
            );
        }
    }
}

// ── the retired homes ──────────────────────────────────────────────────────

/// `Show ▼` and the `Shaded ▼` render-mode menu are gone, and their function
/// is in the registry. A source sentry, because their absence is the
/// migration's whole point: a second home would reintroduce the two-writers
/// defect the audit found for the per-toolpath cut / rapid state.
#[test]
fn the_retired_controls_have_no_second_home() {
    let toolbar = source("src/ui/viewport_overlay.rs");
    assert!(
        !toolbar.contains("\"Show ▼\""),
        "the `Show ▼` popover is back beside the Overlays panel"
    );
    assert!(
        !toolbar.contains("RenderMode"),
        "the render-mode menu is back — its Wireframe arm drew nothing"
    );
    assert!(
        toolbar.contains("panel::toolbar_button"),
        "the toolbar no longer opens the Overlays panel"
    );
    assert!(
        toolbar.contains("overlay_collision_check"),
        "the automation label that located the collision toggle is gone"
    );

    let viewport_state = source("src/state/viewport.rs");
    assert!(
        !viewport_state.contains("enum RenderMode"),
        "`RenderMode` is back; the model is a plain visibility row now"
    );

    let properties = inspector_source();
    assert!(
        !properties.contains("\"Cut\")"),
        "the duplicate per-toolpath Cut checkbox is back in the properties panel"
    );
    assert!(
        properties.contains("toolpath_row_controls::draw"),
        "the properties panel no longer delegates to the one row-control home"
    );
}

// ── F5 / F3 (P5.1, 2026-09-08) ─────────────────────────────────────────────

/// The moves family is ON in Toolpaths — UX §6.4's "Cutting, Rapids, Entry
/// markers | off | on | on" row, pinned by NAME.
///
/// # Why by name, when `the_constructed_state_equals_the_toolpaths_default_column`
/// already covers every row
///
/// Because a live look reported these two OFF in Toolpaths and asked for the
/// default to be fixed, and the investigation found the default was already
/// `Some(true)`: the `Overlays (2)` badge in the same screenshot is
/// `non_default_count`, which counts rows that DIFFER from the workspace
/// default — and exactly the two unchecked rows were counted. So the live
/// state was an override (very likely a previous session hiding the moves to
/// see the reach shading under them, which is F4), not a wrong default.
///
/// That is worth one named test rather than a note in a report. The generic
/// sentry would still pass if someone "fixed" this by flipping
/// `moves_default`'s Toolpaths arm to `false` and moving `AppState::new`'s
/// initialiser to match — both halves would agree, and the operator would
/// lose the moves. This test names the answer the design gives.
#[test]
fn the_moves_family_is_on_in_toolpaths_and_off_in_setup() {
    for id in ["cutting_moves", "rapids", "entry_markers"] {
        let row = registry::row(id).unwrap_or_else(|| panic!("`{id}` has no row"));
        assert_eq!(
            (row.default_for)(Workspace::Toolpaths),
            Some(true),
            "UX 6.4 puts `{id}` ON in Toolpaths"
        );
        assert_eq!(
            (row.default_for)(Workspace::Simulation),
            Some(true),
            "UX 6.4 puts `{id}` ON in Simulation"
        );
        assert_eq!(
            (row.default_for)(Workspace::Setup),
            Some(false),
            "UX 6.4 puts `{id}` OFF in Setup"
        );
    }

    // And a workspace round trip through Setup restores them, which is the
    // path the live look most plausibly took.
    let mut state = AppState::new();
    assert!(state.viewport.show_cutting && state.viewport.show_rapids);
    registry::switch_workspace(&mut state, Workspace::Setup);
    assert!(
        !state.viewport.show_cutting && !state.viewport.show_rapids,
        "Setup's column hides the moves"
    );
    registry::switch_workspace(&mut state, Workspace::Toolpaths);
    assert!(
        state.viewport.show_cutting && state.viewport.show_rapids,
        "returning to Toolpaths must bring the moves back"
    );
    assert_eq!(registry::non_default_count(&state), 0);
}

/// F3 — a toolpath selection in a `set_ui_view` call is in force, DERIVED
/// STATE INCLUDED, before that same call's `overlays` map is judged.
///
/// The defect: `set_ui_view(toolpath_index: 13, overlays: {reach_map: true})`
/// in one call was refused with the reach row's own "select a finishing
/// operation", while the identical overlays map in a SECOND call applied. The
/// selection write itself was already synchronous; what was not was the
/// per-selection artefact the reach row's precondition actually reads
/// (`gui.reach_overlay`, which `AppController::process_reach_overlay`
/// refreshes on the frame pump — after the request returns).
///
/// A SOURCE sentry, for the same reason the two above it are: the ordering is
/// the claim, `mcp_set_ui_view` is 200 lines of `&mut self` on the App, and
/// an integration test cannot construct one. What can be checked, and is
/// exactly what broke, is that the reach pump sits between the selection
/// write and `apply_overlays`.
#[test]
fn a_selection_is_pumped_before_the_same_calls_overlays_map() {
    // P4 (2026-09-17) moved the handler into `app/mcp/view.rs`, where it is
    // the last item, so the body runs to the end of that file.
    let mcp = source("src/app/mcp/view.rs");
    let marker = "fn mcp_set_ui_view";
    let start = mcp
        .find(marker)
        .unwrap_or_else(|| panic!("`{marker}` no longer exists in app/mcp/view.rs"));
    let body = &mcp[start..];

    let select = body
        .find("selection = Selection::Toolpath(tp_id)")
        .expect("mcp_set_ui_view no longer writes the toolpath selection");
    let pump = body.find("process_reach_overlay()").expect(
        "mcp_set_ui_view no longer pumps the reach overlay — a `toolpath_index` \
             plus `overlays: {reach_map: true}` in ONE call will be refused again \
             (F3, 2026-09-08)",
    );
    let apply = body
        .find("registry::apply_overlays")
        .expect("mcp_set_ui_view no longer applies the overlays map");

    assert!(
        select < pump,
        "the reach pump must run AFTER the selection write, or it refreshes \
         against the previous selection"
    );
    assert!(
        pump < apply,
        "the reach pump must run BEFORE apply_overlays, or the overlay \
         precondition is judged against stale derived state"
    );
}

/// P6 (2026-09-08) — the 3D viewport cannot be squeezed to nothing.
///
/// `screenshot_gui` at the window's own 1400 x 900 returned a frame with **no
/// 3D viewport**: the operation list and the inspector filled it. Three
/// things combined. The workspace side panels are `resizable` with no
/// ceiling, and egui remembers a resizable panel's width across frames and
/// does not shrink it when the window does. The docked Overlays column then
/// took `exact_size(PANEL_WIDTH)` out of whatever was left. And the viewport
/// handed `ui.available_size()` straight to `allocate_exact_size`, so the
/// remainder could reach zero with nothing to stop it.
///
/// A source sentry, like the two above it: the layout is `&mut self` on the
/// App behind an eframe run loop, and what broke is the ARITHMETIC, which is
/// readable.
#[test]
fn the_viewport_keeps_a_minimum_width() {
    let app = source("src/app.rs");
    assert!(
        app.contains("const SIDE_PANEL_MAX_WIDTH"),
        "the workspace side panels have no ceiling again; a pair dragged wide \
         on a big monitor survives into a 1400x900 capture"
    );
    let caps = app.matches(".max_size(SIDE_PANEL_MAX_WIDTH)").count();
    assert_eq!(
        caps, 6,
        "every left/right workspace panel must carry the ceiling — setup, \
         toolpaths and simulation, two each; found {caps}"
    );

    let panel = source("src/ui/overlays/panel.rs");
    assert!(
        panel.contains("pub const MIN_VIEWPORT_WIDTH"),
        "the viewport floor is gone"
    );
    assert!(
        panel.contains("if ui.available_width() - PANEL_WIDTH < MIN_VIEWPORT_WIDTH"),
        "the docked Overlays column no longer refuses to dock when it would \
         take the 3D view under its floor"
    );

    let viewport = source("src/app/viewport.rs");
    assert!(
        !viewport.contains("ui.allocate_exact_size(ui.available_size()"),
        "the viewport is handing `available_size` straight to \
         `allocate_exact_size` again — that is the call that returned zero"
    );
    assert!(
        viewport.contains("MIN_VIEWPORT_WIDTH"),
        "the viewport no longer floors its own allocation"
    );
    // A refusal to dock must become the floating form, or a pinned panel on a
    // narrow window would simply vanish.
    assert!(
        viewport.contains("draw_floating(ui, state, events, rect, docked)"),
        "the floating fallback no longer knows whether the dock refused"
    );
}

/// P5.3 — the FOURTH surface quotes the shared notes too, and no surface
/// writes its own description of the area base.
///
/// The panel legend took `area_basis_note()`; the inspector's "Show reach
/// map" line did not, and went on printing "unreachable 51.7 % of MEASURED
/// area" beside a legend saying "of 3D surface area, rim-eroded 3.0 mm". Two
/// surfaces, one quantity, two descriptions — the exact drift the shared
/// notes exist to stop, surviving in the one place nobody re-read.
#[test]
fn every_reach_surface_quotes_the_shared_area_and_bias_notes() {
    let inspector = inspector_source();
    let legend = source("src/ui/overlays/panel.rs");

    for (name, text) in [("inspector", &inspector), ("panel legend", &legend)] {
        assert!(
            text.contains("area_basis_note"),
            "the {name} no longer quotes `ReachMap::area_basis_note` — it is \
             describing the area base in its own words again"
        );
        assert!(
            text.contains("over_statement_note"),
            "the {name} no longer quotes `ReachMap::over_statement_note` — the \
             direction of the bias is being paraphrased again, and it was \
             paraphrased BACKWARDS once already"
        );
    }

    // And no surface in the crate rolls its own denominator sentence.
    for (file, text) in [
        ("ui/properties/", &inspector),
        ("overlays/panel.rs", &legend),
        ("app/mcp.rs", &source("src/app/mcp.rs")),
        ("app/mcp/view.rs", &source("src/app/mcp/view.rs")),
    ] {
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            assert!(
                !trimmed.contains("of MEASURED area"),
                "{file} writes its own area base again: {trimmed}"
            );
        }
    }
}

/// P5.3 — the live viewport dims the toolpath moves while the reach overlay
/// is drawing, and does it at DRAW TIME.
///
/// With cutting moves on — the Toolpaths default — the palette-coloured lines
/// dominated the shading in the live view, exactly as they did in the
/// offscreen composite before F4. The rule is now the same in both. What is
/// NOT the same is the ribbon half: the offscreen renderer draws tubes and
/// can thin them, the viewport draws `PrimitiveTopology::LineList` at one
/// pixel with no width control in wgpu, so only the colour factor transfers.
///
/// It must stay a draw-time treatment. Dimming by moving a registry flag
/// would leave the Overlays panel showing Cutting moves OFF and take the
/// switch out of the operator's hands.
#[test]
fn the_live_viewport_dims_moves_under_the_reach_overlay() {
    let render = source("src/render/mod.rs");
    assert!(
        render.contains("pub const MOVE_DIM_UNDER_REACH"),
        "the live dim factor is gone"
    );
    assert!(
        render.contains("line_dim_bind_group"),
        "the second, dimmed line bind group is gone — a uniform cannot be \
         rewritten inside a render pass, so the dim needs its own"
    );
    assert!(
        render.contains("in.color * uniforms.dim"),
        "the line shader no longer applies the dim factor"
    );
    // The dim's condition must be the reach draw's own, so the two cannot
    // disagree about whether shading is on screen.
    assert!(
        render.contains(
            "self.show_reach_overlay\n                    && resources.reach_overlay_data.is_some()"
        ) || render.contains("self.show_reach_overlay && resources.reach_overlay_data.is_some()"),
        "the move dim is no longer gated on the same condition as the reach \
         draw itself"
    );
    // Draw-time, not a visibility toggle.
    let registry_src = source("src/ui/overlays/registry.rs");
    assert!(
        !registry_src.contains("show_reach_map")
            || registry_src.contains("s.viewport.show_cutting = on"),
        "sanity: the registry still owns the move flags"
    );
    assert!(
        !render.contains("show_cutting = false"),
        "the renderer is writing a visibility flag to dim the moves; that \
         would flip the Overlays panel row and steal the operator's switch"
    );
    let legend = source("src/ui/overlays/panel.rs");
    assert!(
        legend.contains("moves dimmed while reach map is on"),
        "the legend no longer says the moves are being dimmed, so a ticked \
         Cutting moves row beside faint lines reads as a contradiction"
    );
}
