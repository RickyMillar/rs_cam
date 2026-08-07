//! W9 / P-1 — a grblHAL project must still be a grblHAL project after a
//! save/load round trip, and must still export grblHAL G-code.
//!
//! # The defect this pins
//!
//! Two writers emit the token `"grblhal"`
//! (`GuiState::post_to_session`, `AppEvent::WizardSetPost`). Before this
//! test, three readers parsed it back and only one — `gcode::
//! get_post_definition`, reachable solely from the CLI *job-file* path —
//! knew the token. Both readers on the *project* path open-coded a
//! `match` with no `"grblhal"` arm and fell through to
//! `PostFormat::Grbl`:
//!
//! - `gcode::export_gcode_checked` (this file's subject), and
//! - `GuiState::post_from_session` (pinned in
//!   `crates/rs_cam_viz/src/state/runtime.rs`'s own unit tests).
//!
//! So selecting grblHAL in the Post panel, saving and reloading gave you
//! a dropdown reading GRBL and every subsequent export emitting the GRBL
//! dialect — silently, with the correct token still sitting in the file.
//!
//! # What makes this observable
//!
//! The grbl and grblHAL post definitions share their preamble,
//! postamble, tool-change block and decimals. The one behavioural
//! difference is `unsupported_mcodes`: GRBL declares `[6, 7]` and
//! grblHAL declares none (`crates/rs_cam_core/posts/*.toml`;
//! `gcode::post`'s `shipped_post_unsupported_mcodes` pins both sides).
//! `gcode::emitter::filter_raw` replaces any `Statement::Raw` line
//! issuing a denied M-code with a warning comment. A toolpath carrying
//! `post_gcode = "M7"` therefore emits a real `M7` under grblHAL and a
//! `(WARNING: M7 unsupported on GRBL; dropped: M7)` under GRBL — a
//! text-level fact about which definition the export actually used, not
//! a re-assertion of the resolver.
//!
//! # Red-first evidence (parent `777a78b`)
//!
//! `the_exported_gcode_uses_the_grblhal_definition` fails on the parent
//! with the GRBL warning comment present and no `M7` line. The token
//! assertion (`the_saved_token_is_unchanged`) passes on the parent —
//! the file was always right; only the readers were wrong — and is kept
//! so the fix cannot be "change the serialized spelling".

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::{CoolantMode, PostFormat, ToolLoadExportPolicy, export_gcode_checked};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

/// A closed 50 x 40 rectangle. Written to disk because the project file
/// stores a model *path*: an in-memory synthetic model cannot survive
/// the save/load this test is about.
const RECT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
  <path d="M 10 10 L 60 10 L 60 50 L 10 50 Z" fill="black"/>
</svg>
"#;

/// The M-code the two dialects disagree about. GRBL denies it, grblHAL
/// does not.
const DIALECT_PROBE_MCODE: &str = "M7";

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_cam_p1_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn write_model(dir: &Path) -> PathBuf {
    let path = dir.join("rect.svg");
    std::fs::write(&path, RECT_SVG).expect("write svg fixture");
    path
}

/// A one-pocket project whose post format is the caller's token and
/// whose single toolpath carries `M7` as its post-gcode snippet.
fn build_project(dir: &Path, post_token: &str) -> PathBuf {
    let model_path = write_model(dir);

    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });

    let mut post = session.post_config().clone();
    post.format = post_token.to_owned();
    session.set_post_config(post);

    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.name = "End Mill 6mm (P-1)".to_owned();
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "rect".to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: model_path,
        kind: Some(ModelKind::Svg),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: ToolpathId(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation: OperationConfig::Pocket(PocketConfig {
                    stepover: 3.0,
                    depth: 2.0,
                    depth_per_pass: 2.0,
                    feed_rate: 1000.0,
                    plunge_rate: 300.0,
                    climb: true,
                    pattern: PocketPattern::Contour,
                    angle: 0.0,
                    finishing_passes: 0,
                    spindle_rpm: Some(18_000),
                }),
                dressups: DressupConfig::for_op(OperationType::Pocket),
                heights: HeightsConfig::default(),
                tool_id,
                model_id,
                pre_gcode: None,
                post_gcode: Some(DIALECT_PROBE_MCODE.to_owned()),
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                stock_source: StockSource::default(),
                coolant: CoolantMode::Off,
                face_selection: None,
                debug_options: ToolpathDebugOptions::default(),
                feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
                rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
            },
        )
        .expect("add pocket toolpath");

    let project_path = dir.join("project.toml");
    session.save(&project_path).expect("save project");
    project_path
}

/// Load, generate, export. Returns the emitted G-code.
fn reload_and_export(project_path: &Path) -> String {
    let mut session = ProjectSession::load(project_path).expect("reload project");
    assert_eq!(
        session.models().len(),
        1,
        "the SVG model must reload from disk, or the export has no phases to emit"
    );
    assert!(
        session.models()[0].load_error.is_none(),
        "model failed to reload: {:?}",
        session.models()[0].load_error
    );

    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate the reloaded pocket");

    export_gcode_checked(
        &session,
        None,
        ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        },
    )
    .expect("export the reloaded project")
}

/// The file keeps the spelling the writer chose. This passed before the
/// fix and must keep passing: P-1 is a reader defect, and the fix is
/// explicitly not allowed to change the serialized token.
#[test]
fn the_saved_token_is_unchanged() {
    let dir = scratch_dir("token");
    let project_path = build_project(&dir, "grblhal");
    let toml = std::fs::read_to_string(&project_path).expect("read project toml");
    assert!(
        toml.contains("format = \"grblhal\""),
        "the project file must still spell the token `grblhal`; got:\n{toml}"
    );

    let reloaded = ProjectSession::load(&project_path).expect("reload");
    assert_eq!(reloaded.post_config().format, "grblhal");
    assert_eq!(
        PostFormat::from_token(&reloaded.post_config().format),
        Some(PostFormat::GrblHal),
        "the resolver must recognise the token the writer emitted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The P-1 reproducer.** Save grblHAL, reload, export — and read the
/// emitted text for the one thing the two dialects disagree about.
#[test]
fn the_exported_gcode_uses_the_grblhal_definition() {
    let dir = scratch_dir("export_hal");
    let project_path = build_project(&dir, "grblhal");
    let gcode = reload_and_export(&project_path);

    assert!(
        gcode.lines().any(|line| line.trim() == DIALECT_PROBE_MCODE),
        "grblHAL supports M7, so the toolpath's post-gcode snippet must survive verbatim. \
         The export used the GRBL definition instead. Emitted:\n{gcode}"
    );
    assert!(
        !gcode.contains("unsupported on GRBL"),
        "the export fell through to the GRBL definition; emitted:\n{gcode}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The control: the same project saved as GRBL must still be filtered.
/// Without this, "the assertion above passes" could just mean the
/// M-code filter stopped working.
#[test]
fn the_same_project_saved_as_grbl_still_filters_the_mcode() {
    let dir = scratch_dir("export_grbl");
    let project_path = build_project(&dir, "grbl");
    let gcode = reload_and_export(&project_path);

    assert!(
        gcode.contains("unsupported on GRBL"),
        "GRBL denies M7 — the filter must fire on this arm; emitted:\n{gcode}"
    );
    assert!(
        !gcode.lines().any(|line| line.trim() == DIALECT_PROBE_MCODE),
        "the raw M7 must not survive a GRBL export; emitted:\n{gcode}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An unknown token still degrades to GRBL rather than refusing the
/// export. Stated as a test because it is a deliberate choice, not an
/// oversight: `from_token` returns `None` and the export site — not the
/// resolver — picks the fallback.
#[test]
fn an_unknown_post_token_still_falls_back_to_grbl() {
    let dir = scratch_dir("export_unknown");
    let project_path = build_project(&dir, "cobalt-cnc");
    let gcode = reload_and_export(&project_path);

    assert_eq!(PostFormat::from_token("cobalt-cnc"), None);
    assert!(
        gcode.contains("unsupported on GRBL"),
        "an unresolvable token must export as GRBL; emitted:\n{gcode}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
