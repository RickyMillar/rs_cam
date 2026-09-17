//! G-FRESHRENDER (F2.2) — the three surfaces a test cannot drive.
//!
//! Most of F2.2's sentry is in `src/controller/tests.rs`, driving the real
//! functions: the chip vocabulary, both workspace badges, `operations_check`
//! and the shared counter are all pure over `&AppState`. Three surfaces are
//! not: the inspector header and the card body draw through `egui::Ui`, and
//! the dimmed viewport path is a wgpu bind-group choice inside a render pass.
//! This file reads their source, the same stand-in
//! `mcp_toasts_report_outcome_g_mcptoast.rs` and
//! `inspector_header_wraps_g_reachwrap.rs` already use here.
//!
//! **What it proves:** each of the three reads `FreshnessState` and not the
//! thing it used to read, and the `EditedSince` arm exists and says
//! something. **What it does not:** that any of it looks right on screen.
//! PLAN's acceptance for F2.2 is an MCP-VIEW screenshot on the terrain seed
//! plus a `list_toolpaths.stale` cross-check; the MCP server is down this
//! session and no GUI may be started, so neither was run and neither is
//! claimed. Visual confirmation is owed to V6.2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// Every `.rs` file directly in `src/ui/properties/`, concatenated in
/// file-name order.
///
/// P4 (2026-09-17) split `properties/mod.rs` into `mod.rs` plus seven panel
/// children beside it. The header's status match moved into
/// `toolpath_panel.rs`, and the export-wording scan below is only honest
/// over the whole inspector, so the reader is the folder. `operations/` is
/// a sub-folder and is not read; it carries its own sentries.
fn properties_src() -> String {
    let mut out = String::new();
    for (_, text) in properties_files() {
        out.push_str(&text);
        out.push('\n');
    }
    out
}

/// The same files, one row each: the label a message prints and the text.
///
/// The export-wording scan below exempts a surface that calls the shared
/// builder. Over the concatenation one child exempts all fourteen files,
/// so that scan reads the rows, not the join.
fn properties_files() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no source under {}", dir.display());
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .unwrap_or_else(|| panic!("no file name: {}", path.display()))
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            (format!("ui/properties/{name}"), text)
        })
        .collect()
}
const PANEL_SRC: &str = include_str!("../src/ui/toolpath_panel.rs");
const RENDER_SRC: &str = include_str!("../src/render/mod.rs");
const VIEWPORT_SRC: &str = include_str!("../src/app/viewport.rs");
const READINESS_PANEL_SRC: &str = include_str!("../src/ui/readiness_panel.rs");
const WORKSPACE_BAR_SRC: &str = include_str!("../src/ui/workspace_bar.rs");

/// The inspector header's status line reads the freshness state, and its
/// `EditedSince` arm says both halves — the generation finished, and what it
/// produced no longer answers the settings on screen.
///
/// Pre-fix the header matched `ComputeStatus::effective(entry.enabled,
/// &entry.status)`, whose `Done` arm printed a green "Done" directly above
/// the fields the operator had just changed.
#[test]
fn the_inspector_header_reads_the_freshness_state() {
    let src = properties_src();
    let at = src
        .find("// F2.2 — the same one state the card chip")
        .expect("the header's status match moved");
    let block = &src[at..(at + 3000).min(src.len())];

    assert!(
        block.contains("match freshness {"),
        "the header must match on the freshness state"
    );
    assert!(
        !block.contains("ComputeStatus::effective(entry.enabled"),
        "the header must not re-derive its own answer from ComputeStatus"
    );
    assert!(
        block.contains("FreshnessState::EditedSince =>"),
        "the header needs an arm for the state that did not exist before"
    );
    assert!(
        block.contains("edited since"),
        "and that arm has to say so: {block}"
    );
}

/// The card's state indicator and its generate route read the one state.
///
/// The generate route is the sharper of the two: `ComputeStatus::needs_generation`
/// answers `false` for `Done`, and an edited operation keeps `Done`, so the
/// quick-generate control was hidden on precisely the card whose whole message
/// is "regenerate me".
///
/// **This test lost its third arm on 2026-09-14 (DC1, ruling R27).** It
/// asserted that the card's stats row prefixed an edited operation's figures
/// with `old: `. The stats row is gone: three numbers per card times nine
/// cards was 27 figures competing with nine names, and the inspector — which
/// is open whenever a card is selected — carries them instead. The arm below
/// pins the DELETION, so the row cannot return to the card without a ruling.
/// The `old: ` guarantee itself now belongs to whichever surface draws those
/// figures, and it is not this one.
#[test]
fn the_card_reads_the_freshness_state() {
    assert!(
        PANEL_SRC.contains("status_chip(freshness)"),
        "the state indicator must come from the shared mapping"
    );
    assert!(
        !PANEL_SRC.contains("ComputeStatus::effective("),
        "the card must not re-derive its own answer from ComputeStatus"
    );
    assert!(
        PANEL_SRC.contains("let needs_generation = !matches!(")
            && !PANEL_SRC.contains("if status.needs_generation()"),
        "the generate route must follow freshness, not ComputeStatus::needs_generation"
    );
    assert!(
        !PANEL_SRC.contains("if is_stale { \"old: \" } else { \"\" }"),
        "R27 removed the stats row from the card. If it came back, it came \
         back without the ruling that would justify it — and a card whose \
         height varies with its content breaks Rule D."
    );
}

/// The viewport dims a stale path instead of drawing it at full strength.
///
/// Drawn, not hidden: the operator asked to see this operation, and hiding it
/// would replace a wrong picture with no picture.
#[test]
fn the_viewport_dims_a_stale_path() {
    assert!(
        RENDER_SRC.contains("pub stale_toolpaths:"),
        "the renderer needs to be told which toolpaths are stale"
    );
    let at = RENDER_SRC
        .find("let stale = tp_gpu")
        .expect("the per-toolpath dim decision moved");
    let block = &RENDER_SRC[at..(at + 500).min(RENDER_SRC.len())];
    assert!(
        block.contains("self.stale_toolpaths.contains(&id)"),
        "the decision must be per toolpath, not per frame:\n{block}"
    );
    assert!(
        block.contains("line_dim_bind_group"),
        "a stale path takes the dimmed bind group:\n{block}"
    );
    // Populated from the one model, not from a second opinion about it.
    assert!(
        VIEWPORT_SRC.contains("stale_toolpaths: state")
            && VIEWPORT_SRC.contains("FreshnessState::EditedSince"),
        "the set must be derived from freshness_at, not from stale_since"
    );
}

/// Readiness names its count "current", not "computed", and says how many
/// were edited after generation.
///
/// An operation edited after generation WAS computed; what it is not is the
/// answer to the configuration now in the project.
#[test]
fn readiness_says_current_and_names_the_edited_ones() {
    assert!(
        READINESS_PANEL_SRC.contains("{computed}/{enabled} current"),
        "the row must not go on calling an edited operation computed"
    );
    assert!(
        !READINESS_PANEL_SRC.contains("{computed}/{enabled} computed"),
        "the old wording must be replaced, not kept beside the new one"
    );
    assert!(
        READINESS_PANEL_SRC.contains("edited since generation"),
        "and it must name the edited ones, which is the distinction \
         'uncomputed' could not draw"
    );
}

/// No freshness surface writes its own export-blocking sentence.
///
/// **This test changed its subject on 2026-09-10 (F2.9), and the reason is
/// worth keeping.** It was written under F2.2, against a head where F2.3 had
/// not landed: an operation whose card read STALE could still export its old
/// geometry, so the risk was that a freshness string would over-promise and
/// turn a display notice into a safety claim the programme had not earned.
/// The test therefore refused the word "export" anywhere near "stale".
///
/// F2.3 merged and the gate exists. That caution is spent, and keeping it
/// would now be actively wrong twice over: its failure message asserted a
/// falsehood, and it would have failed a future card chip that said "this
/// will not export until you regenerate" — a true and useful sentence.
///
/// **The risk inverted.** It is no longer over-promising; it is a second
/// surface explaining WHY an operation blocks, in its own words, and drifting
/// from the one builder F2.3 gave export, pre-flight and MCP `export_gcode`
/// (`io::export::blocking_toolpath_message`). Two texts for one refusal is
/// the shape of every defect this phase has closed: three freshness stores in
/// R0.1, four workspace lists in G-WSMENU, two chip vocabularies here.
///
/// So the rule is now: a freshness surface may SAY an operation will not
/// export — but the sentence comes from the shared builder, not from a
/// literal it wrote itself. The word "export" on its own is fine, and has to
/// be: `readiness_panel.rs` carries an "Export G-code…" button and
/// `properties/mod.rs` a Getting-started step.
#[test]
fn no_freshness_surface_writes_its_own_export_blocking_sentence() {
    /// Words that turn a mention of export into a claim about whether one
    /// will happen. A label or a menu item contains none of them.
    const BLOCKING_CLAIM: &[&str] = &[
        "cannot",
        "can't",
        "won't",
        "will not",
        "unable",
        "block",
        "refus",
        "prevent",
        "not be export",
    ];

    let inspector = properties_files();
    for (label, src) in [
        ("ui/toolpath_panel.rs", PANEL_SRC),
        ("readiness_panel.rs", READINESS_PANEL_SRC),
        ("workspace_bar.rs", WORKSPACE_BAR_SRC),
    ]
    .into_iter()
    .chain(
        inspector
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str())),
    ) {
        // A surface that calls the shared builder is using the one text by
        // construction; the rule is about surfaces that write their own.
        if src.contains("blocking_toolpath_message") {
            continue;
        }
        for (number, line) in src.lines().enumerate() {
            let lower = line.to_lowercase();
            // Only string literals — a comment may discuss the gate freely,
            // and several of them do.
            if !line.contains('"') || !lower.contains("export") {
                continue;
            }
            let trimmed = lower.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if let Some(claim) = BLOCKING_CLAIM.iter().find(|c| lower.contains(**c)) {
                panic!(
                    "{label}:{}: this surface writes its own export-blocking sentence                      (matched {claim:?}). That text has ONE home since F2.3 —                      `io::export::blocking_toolpath_message` — which export, the                      pre-flight modal and MCP `export_gcode` all share. Call it                      instead of writing a second wording that can drift from it:\n{line}",
                    number + 1
                );
            }
        }
    }
}

/// The other half of the same rule: the shared builder is reachable. A rule
/// that says "call this instead" is only fair if a surface can.
#[test]
fn the_shared_blocking_text_is_available_to_a_freshness_surface() {
    let src = include_str!("../src/io/export.rs");
    assert!(
        src.contains("pub fn blocking_toolpath_message("),
        "the one builder must stay public, or the rule above has no remedy"
    );
    assert!(
        src.contains("FreshnessState::EditedSince => format!("),
        "and it must still key on the freshness state, which is what let it \
         say something an edited operation could not say under ComputeStatus"
    );
}
