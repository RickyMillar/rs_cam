//! P4.4: the selection colours live in `render/colors.rs`, and a report
//! change uploads once.
//!
//! # The claims
//!
//! 1. `app/gpu_upload.rs` holds no colour literal (a float triple such as
//!    `[0.2, 0.95, 0.4]`); the drill markers, the datum and the plain
//!    simulated stock read `render/colors.rs`. The scan strips comments and
//!    anchors on the reads of the selection colours, so it is not vacuous.
//! 2. Each selection colour is its UI token divided by 255 (lead ruling
//!    L-10), so a panel chip and a viewport line of one state match.
//! 3. The upload trigger of the highlight (`ReportDetector`, which the
//!    frame loop calls) fires once for a report change and never for a
//!    frame with no change: a second read of the memo gives the same
//!    `Arc`, a selection edit gives a new one.
//!
//! Red proofs (each run, red, reverted):
//!
//! - Claim 1: the drill marker colour put back as the literal
//!   `[0.2_f32, 0.95, 0.4]`: the scan named the line.
//! - Claim 2: `SELECTION_CUT` pointed at `tokens::CAUTION`: the pin named
//!   `SELECTION_CUT`.
//! - Claim 3: `ReportDetector::changed` that does not store the new report
//!   (it always answers "changed"): the no-change frame fired an upload.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::selection::{Filter, Rule, Selection, SelectionEdit, SourceRef};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSessionBuilder, SelectionTarget, ToolpathConfig,
};
use rs_cam_viz::render::colors;
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::state::selection_highlight::ReportDetector;
use rs_cam_viz::ui::tokens;

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(text.len() > 500, "{} is too short", path.display());
    text
}

/// The GPU upload pass as one text: `app/gpu_upload.rs` and every slot file
/// in `app/gpu_upload/` (F132 split the pass by slot).
fn upload_source() -> String {
    let mut text = source("src/app/gpu_upload.rs");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/gpu_upload");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "{} holds no slot file", dir.display());
    for file in files {
        text.push('\n');
        text.push_str(
            &std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("read {}: {e}", file.display())),
        );
    }
    text
}

/// Every line without its `//` comment.
fn code_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .map(|(n, line)| {
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            (n + 1, code.to_owned())
        })
        .collect()
}

/// The float triples `[a, b, c]` of one line whose numbers are not all
/// zero (a zero triple is a position, not a colour).
fn colour_literals(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        let inner = &after[..close];
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let numbers: Vec<f64> = parts
            .iter()
            .filter_map(|p| p.trim_end_matches("_f32").parse::<f64>().ok())
            .collect();
        if parts.len() == 3
            && numbers.len() == 3
            && parts.iter().all(|p| p.contains('.'))
            && numbers.iter().any(|v| *v != 0.0)
        {
            found.push(format!("[{inner}]"));
        }
        rest = &after[close + 1..];
    }
    found
}

#[test]
fn gpu_upload_holds_no_colour_literal_p4_4() {
    let text = upload_source();
    let lines = code_lines(&text);
    // Non-vacuity: the scan must see the reads of the colour module.
    let reads = lines.iter().filter(|(_, l)| l.contains("colors::")).count();
    assert!(
        reads >= 10,
        "only {reads} reads of the colour module: the scan reads the wrong file"
    );
    assert!(
        lines
            .iter()
            .any(|(_, l)| l.contains("colors::selection_state_color(")),
        "the upload no longer colours an item by its selection state"
    );
    assert!(
        colour_literals("let c = [0.2_f32, 0.95, 0.4];").len() == 1
            && colour_literals("let p = [0.0_f32, 0.0, 0.0];").is_empty(),
        "the literal finder does not find a colour triple"
    );
    let offenders: Vec<String> = lines
        .iter()
        .flat_map(|(n, l)| {
            colour_literals(l)
                .into_iter()
                .map(move |lit| format!("line {n}: {lit}"))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "colour literals in app/gpu_upload.rs; move them to render/colors.rs: {offenders:?}"
    );
}

#[test]
fn each_selection_colour_is_its_token_p4_4() {
    let rgb = |c: egui::Color32| {
        [
            f32::from(c.r()) / 255.0,
            f32::from(c.g()) / 255.0,
            f32::from(c.b()) / 255.0,
        ]
    };
    for (name, colour, token) in [
        ("SELECTION_CUT", colors::SELECTION_CUT, tokens::OK),
        ("SELECTION_ISLAND", colors::SELECTION_ISLAND, tokens::ACCENT),
        (
            "SELECTION_SKIPPED",
            colors::SELECTION_SKIPPED,
            tokens::CAUTION,
        ),
        (
            "SELECTION_EXCLUDED",
            colors::SELECTION_EXCLUDED,
            tokens::INK_35,
        ),
        ("SELECTION_IDLE", colors::SELECTION_IDLE, tokens::INK_50),
        ("SELECTION_LOST", colors::SELECTION_LOST, tokens::DANGER),
        (
            "SELECTION_HOVER",
            colors::SELECTION_HOVER,
            tokens::TEXT_STRONG,
        ),
        ("OPEN_PATH", colors::OPEN_PATH, tokens::PATH_OPEN),
        ("PATH_START", colors::PATH_START, tokens::OK),
        ("PATH_END", colors::PATH_END, tokens::DANGER),
    ] {
        assert_eq!(colour, rgb(token), "{name} is not its token / 255");
    }
    // The states have five different colours.
    use rs_cam_core::session::ItemState;
    let states = [
        ItemState::Cut,
        ItemState::Island,
        ItemState::Skipped,
        ItemState::Excluded,
        ItemState::NotSelected,
    ];
    for (i, a) in states.iter().enumerate() {
        for b in &states[i + 1..] {
            assert_ne!(
                colors::selection_state_color(*a),
                colors::selection_state_color(*b),
                "{a:?} and {b:?} draw in one colour"
            );
        }
    }
}

fn toolpath(op: OperationType) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

#[test]
fn a_report_change_uploads_once_p4_4() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, toolpath(OperationType::Pocket))
        .unwrap();
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let id = state.session().toolpath_configs()[0].id;
    state.selection = UiSelection::Toolpath(id);

    // The frame loop's call, frame by frame.
    let mut detector = ReportDetector::default();
    let mut uploads = 0;
    let mut frame = |state: &AppState, detector: &mut ReportDetector| {
        if detector.changed_in(state) {
            uploads += 1;
        }
        uploads
    };
    assert_eq!(frame(&state, &mut detector), 1, "the first frame uploads");
    for _ in 0..5 {
        assert_eq!(
            frame(&state, &mut detector),
            1,
            "a frame with no change uploaded again"
        );
    }
    let _ = state
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::from_rules(vec![Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter {
                    layers: vec!["CUT".into()],
                    ..Filter::default()
                },
            }]))),
        }))
        .unwrap();
    assert_eq!(frame(&state, &mut detector), 2, "a selection edit uploads");
    for _ in 0..5 {
        assert_eq!(
            frame(&state, &mut detector),
            2,
            "the edit uploaded more than once"
        );
    }
    // No toolpath drawn: one upload to clear the highlight, then none.
    state.selection = UiSelection::None;
    assert_eq!(frame(&state, &mut detector), 3);
    assert_eq!(frame(&state, &mut detector), 3);

    // The frame loop calls the detector (a source scan with an anchor).
    let app = source("src/app.rs");
    assert!(
        app.contains(".selection_report_detector") && app.contains(".changed_in("),
        "the frame loop no longer calls the report detector"
    );
}
