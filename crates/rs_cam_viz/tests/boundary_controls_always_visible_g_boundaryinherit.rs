//! G-BOUNDARYINHERIT sentries — the Geometry tab has no dead
//! "Inherit from stock" checkbox, and it names the stored boundary source
//! (UX-R03-009, `planning/ui_review_2026-09-09/results/R03/REPORT.md`).
//!
//! Live reproduction, 2026-09-09: a Scallop added on a mesh showed
//! "Enable boundary ✓ / Inherit from stock ✓" while `get_toolpath_params`
//! reported `boundary.source = model_silhouette`. No generation code ever
//! read `boundary_inherit` (`session/compute.rs` clones `tc.boundary`
//! unconditionally), and there is no stock-level default boundary. While
//! ticked, the checkbox HID the Source / Containment / Offset controls that
//! do drive generation.
//!
//! Three layers are pinned here:
//!
//! - The UI SOURCE: no file under `crates/rs_cam_viz/src/ui` names
//!   `boundary_inherit`. The Geometry tab needs a live `egui` context, so
//!   the source check stands in for driving the checkbox.
//! - The summary line the tab prints names the stored `boundary.source`.
//! - A project file that still carries `boundary_inherit = true` loads, and
//!   every generation-relevant boundary field survives unchanged. The
//!   section type is core's, since C11 deleted the viz copy.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::session::ProjectToolpathSection;
use rs_cam_viz::ui::properties::boundary_summary_line;

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read ui dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A struct-literal WRITE of the retained core `ToolpathConfig` field with a
/// constant (`boundary_inherit: true,`). The core field survives for
/// project-file compatibility, so a UI test helper that builds a core
/// config must still name it; that is not a reader.
fn is_constant_core_write(line: &str) -> bool {
    let t = line.trim();
    t == "boundary_inherit: true," || t == "boundary_inherit: false,"
}

/// (a) `rg boundary_inherit crates/rs_cam_viz/src/ui` returns no reader.
#[test]
fn ui_sources_never_read_boundary_inherit() {
    let ui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    collect_rs_files(&ui_dir, &mut files);
    assert!(!files.is_empty(), "no .rs files under {}", ui_dir.display());

    let offenders: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).expect("read ui source");
            text.lines()
                .enumerate()
                .find(|(_, line)| {
                    line.contains("boundary_inherit") && !is_constant_core_write(line)
                })
                .map(|(i, line)| format!("{}:{}: {}", path.display(), i + 1, line.trim()))
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "the UI still reads the dead `boundary_inherit` dial (UX-R03-009):\n{}",
        offenders.join("\n")
    );
}

/// (b) The summary line names the stored `boundary.source`.
#[test]
fn summary_line_names_the_stored_source() {
    let with = |source: BoundarySource| BoundaryConfig {
        enabled: true,
        source,
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    assert_eq!(
        boundary_summary_line(&with(BoundarySource::Stock), false),
        "Boundary: stock rectangle"
    );
    assert_eq!(
        boundary_summary_line(&with(BoundarySource::ModelSilhouette), false),
        "Boundary: model silhouette"
    );
    assert_eq!(
        boundary_summary_line(&with(BoundarySource::ModelSilhouette), true),
        "Boundary: model silhouette (auto)"
    );
    assert_eq!(
        boundary_summary_line(&with(BoundarySource::FaceSelection), false),
        "Boundary: face selection"
    );
    assert_eq!(
        boundary_summary_line(
            &with(BoundarySource::DerivedRestRegions {
                source_toolpath_id: ToolpathId(7),
            }),
            false
        ),
        "Boundary: rest regions of toolpath 7"
    );
    // The panel cannot prove auto provenance today, so it must never say so
    // for the same config it prints without the flag.
    assert!(!boundary_summary_line(&with(BoundarySource::Stock), false).contains("(auto)"));
}

const LEGACY_SECTION: &str = r#"
name = "Finish"
boundary_inherit = true

[boundary]
enabled = true
source = "model_silhouette"
containment = "inside"
offset = 1.5
"#;

fn expected_boundary() -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::ModelSilhouette,
        containment: BoundaryContainment::Inside,
        offset: 1.5,
    }
}

/// (c) A section carrying `boundary_inherit = true` loads, and the boundary
/// fields generation reads survive unchanged.
///
/// **C11 changed what this arm can claim.** The section type used to be
/// viz's own. That copy marked `boundary_inherit` `skip_serializing`, so
/// the arm also asserted the dead key was never written back.
///
/// C11 deleted the viz copy. Core is the one project schema, and core
/// writes the key. It must: the `o1b` multitool round-trip sentry asserts
/// that `!boundary_inherit` survives a save and a load, and the field's
/// serde default is `true`.
///
/// The dial is still dead. No file under `crates/rs_cam_viz/src/ui` reads
/// it, which arm (a) pins. `session/compute.rs` clones `tc.boundary`
/// unconditionally. The written key is a constant, and nothing acts on it.
#[test]
fn legacy_boundary_inherit_key_loads_and_is_not_read() {
    let section: ProjectToolpathSection = toml::from_str(LEGACY_SECTION).expect("legacy loads");
    assert_eq!(section.boundary, expected_boundary());
}
