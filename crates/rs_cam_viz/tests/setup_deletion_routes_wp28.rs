//! Source sentry for the two GUI setup-deletion routes.

#![allow(clippy::panic, clippy::indexing_slicing)]

const INSPECTOR: &str = include_str!("../src/ui/properties/setup.rs");
const SETUP_PANEL: &str = include_str!("../src/ui/setup_panel.rs");
const CONFIRMATION: &str = include_str!("../src/ui/setup_deletion_modal.rs");

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn function_region<'a>(text: &'a str, start: &str, end: &str, file: &str) -> &'a str {
    let start_at = text
        .find(start)
        .unwrap_or_else(|| panic!("non-vacuity anchor `{start}` is missing from {file}"));
    let tail = &text[start_at..];
    let end_at = tail
        .find(end)
        .unwrap_or_else(|| panic!("function boundary `{end}` is missing from {file}"));
    let region = &tail[..end_at];
    assert!(
        region.len() > 500,
        "the bounded function region in {file} is unexpectedly empty"
    );
    region
}

fn between<'a>(text: &'a str, start: &str, end: &str, file: &str) -> &'a str {
    let start_at = text
        .find(start)
        .unwrap_or_else(|| panic!("branch anchor `{start}` is missing from {file}"));
    let tail = &text[start_at..];
    let end_at = tail
        .find(end)
        .unwrap_or_else(|| panic!("branch boundary `{end}` is missing from {file}"));
    &tail[..end_at]
}

fn from_anchor<'a>(text: &'a str, start: &str, file: &str) -> &'a str {
    let start_at = text
        .find(start)
        .unwrap_or_else(|| panic!("branch anchor `{start}` is missing from {file}"));
    &text[start_at..]
}

#[test]
fn inspector_requests_shared_confirmation_and_disables_the_last_setup_route() {
    let source = strip_comments(INSPECTOR);
    let draw = function_region(
        &source,
        "pub fn draw(",
        "pub fn draw_fixture_properties(",
        "src/ui/properties/setup.rs",
    );
    assert!(
        draw.contains("ui.heading(\"Setup Properties\")"),
        "the bounded region must be the setup inspector"
    );

    let disabled = between(
        draw,
        "if setup_count == 1 {",
        "} else if ui.button(\"Remove Setup\").clicked() {",
        "src/ui/properties/setup.rs",
    );
    assert!(disabled.contains("ui.add_enabled(false"));
    assert!(disabled.contains("The last remaining setup cannot be removed."));
    assert!(!disabled.contains("AppEvent::RemoveSetup"));

    assert!(draw.contains("AppEvent::RequestRemoveSetup(setup_id)"));
    assert!(!draw.contains("insert_temp"));
    assert!(!draw.contains("AppEvent::RemoveSetup(setup_id)"));
}

#[test]
fn setup_card_context_menu_requests_the_shared_confirmation() {
    let source = strip_comments(SETUP_PANEL);
    let card = function_region(
        &source,
        "fn draw_setup_card(",
        "fn setup_detail(",
        "src/ui/setup_panel.rs",
    );
    assert!(
        card.contains("Card::new()"),
        "the bounded region must be the setup card"
    );

    let menu = from_anchor(
        card,
        "response.context_menu(|ui| {",
        "src/ui/setup_panel.rs",
    );
    assert!(menu.contains("state.session.list_setups().len() == 1"));
    assert!(menu.contains("The last remaining setup cannot be removed."));
    assert!(menu.contains("AppEvent::RequestRemoveSetup(setup_id)"));
    assert!(!menu.contains("AppEvent::RemoveSetup(setup_id)"));
}

#[test]
fn shared_confirmation_names_the_setup_and_its_toolpaths() {
    let source = strip_comments(CONFIRMATION);
    assert!(source.contains("pending_setup_removal"));
    assert!(source.contains("confirm.setup_name"));
    assert!(source.contains("all toolpaths it contains?"));
    assert!(source.contains("AppEvent::CancelRemoveSetup"));
    assert!(source.contains("AppEvent::RemoveSetup(setup_id)"));
}
