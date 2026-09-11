//! G-MODELRELINK (F4.3) — what a model path means, in memory and on disk.
//!
//! Two rules, and before F4.3 neither existed:
//!
//! - **In memory a model path is RESOLVED.** The project loader used to store
//!   the raw string from the file, so a project saying `path = "m.dxf"` kept a
//!   path that resolves against the process working directory. Reload from
//!   disk looked for `./m.dxf` wherever `rs_cam_gui` was launched, and the
//!   missing-model warning printed a string naming no directory anyone had
//!   searched. The resolve existed — it was applied for the READ and thrown
//!   away.
//! - **On disk a model path is RELATIVE when the model is under the project
//!   directory, absolute otherwise.** The save was a pass-through of whatever
//!   the in-memory path happened to be, so whether a project survived being
//!   moved depended on how its models were first added. A project folder that
//!   carries its own models is the case this exists for.
//!
//! Companion to `model_units_survive_reload_g_unitsreload.rs`, which pins that
//! the two loader doors agree about GEOMETRY for one path. This one is about
//! the path itself.
//!
//! Source: `planning/ui_fix_2026-09-09/research/R0.7.md` §2.1 and §3.2 (rule
//! R1), and PLAN.md §6 row F4.3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::ProjectSession;

/// A unique scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_modelrelink_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// A project whose model path is written verbatim into the `[[models]]`
/// section.
fn write_job(dir: &Path, model_path: &str) -> PathBuf {
    let job = dir.join("job.toml");
    std::fs::write(
        &job,
        format!(
            r#"format_version = 3

[job]
name = "Path Round Trip"

[[tools]]
id = 1
name = "Fixture End Mill"
type = "end_mill"

[[models]]
id = 1
path = "{model_path}"
name = "Plate"
kind = "dxf"

[[setups]]
name = "Setup 1"
"#
        ),
    )
    .expect("write job");
    job
}

/// Rule one. A relative path in the file becomes an absolute path in memory,
/// resolved against the PROJECT's directory — not the working directory,
/// which is what made the pre-fix value unusable for anything but the one
/// read that computed it.
#[test]
fn a_relative_path_resolves_against_the_project_directory() {
    let dir = scratch("resolve");
    std::fs::copy(fixture("rivers_aligned.dxf"), dir.join("m.dxf")).expect("stage the model");
    let job = write_job(&dir, "m.dxf");

    let session = ProjectSession::load(&job).expect("the project loads");

    assert_eq!(
        session.models()[0].path,
        dir.join("m.dxf"),
        "the session holds the resolved path, not the string from the file"
    );
    assert!(
        session.models()[0].polygons.is_some(),
        "and the geometry loaded through it"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Rule two, and the round trip that matters: load a project with a relative
/// path, save it back, and the file still says the same relative thing.
///
/// This is what lets a project folder be copied to another machine. Pre-fix
/// the save wrote whatever the in-memory path held; with rule one in place
/// that is now always absolute, so without rule two every save would have
/// BROKEN a portable project — the two rules have to land together.
#[test]
fn a_model_under_the_project_directory_round_trips_relative() {
    let dir = scratch("relative");
    std::fs::copy(fixture("rivers_aligned.dxf"), dir.join("m.dxf")).expect("stage the model");
    let job = write_job(&dir, "m.dxf");

    let session = ProjectSession::load(&job).expect("load");
    session.save(&job).expect("save");

    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains("path = \"m.dxf\""),
        "a model under the project directory stays relative to it:\n{written}"
    );

    // And it still opens.
    let reloaded = ProjectSession::load(&job).expect("reload");
    assert!(reloaded.models()[0].polygons.is_some());
    assert_eq!(reloaded.models()[0].path, dir.join("m.dxf"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of rule two. Save the same project somewhere else and the
/// model — which did not move — is written absolute, because that is the only
/// honest description of where it is from the new location.
#[test]
fn saving_elsewhere_writes_the_absolute_path_the_model_still_has() {
    let dir = scratch("elsewhere");
    let other = scratch("elsewhere_dest");
    std::fs::copy(fixture("rivers_aligned.dxf"), dir.join("m.dxf")).expect("stage the model");
    let job = write_job(&dir, "m.dxf");

    let session = ProjectSession::load(&job).expect("load");
    let moved_job = other.join("job.toml");
    session.save(&moved_job).expect("save as");

    let written = std::fs::read_to_string(&moved_job).expect("read back");
    assert!(
        written.contains(&format!("path = \"{}\"", dir.join("m.dxf").display())),
        "the model did not move, so the new project file names where it is:\n{written}"
    );

    // The point of writing it absolute: the copy opens and finds the model.
    let reloaded = ProjectSession::load(&moved_job).expect("reload from the new location");
    assert!(
        reloaded.models()[0].polygons.is_some(),
        "a Save As must not break the project it just wrote"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

/// An absolute path that happens to sit under the project directory is
/// SHORTENED on save. This is the case a relink produces: the interactive
/// door hands back an absolute path from the file dialog, and if the operator
/// located the file inside the project folder the project should become
/// portable, not stay pinned to this machine.
#[test]
fn an_absolute_path_under_the_project_directory_is_written_relative() {
    let dir = scratch("shorten");
    std::fs::create_dir_all(dir.join("models")).expect("create models dir");
    std::fs::copy(fixture("rivers_aligned.dxf"), dir.join("models/m.dxf")).expect("stage");
    let absolute = dir.join("models/m.dxf");
    let job = write_job(&dir, &absolute.display().to_string().replace('\\', "\\\\"));

    let session = ProjectSession::load(&job).expect("load");
    assert_eq!(session.models()[0].path, absolute);
    session.save(&job).expect("save");

    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains("path = \"models/m.dxf\""),
        "an absolute path under the project directory is shortened:\n{written}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A model that failed to load keeps the RESOLVED path too, which is the
/// whole reason the repair route can work: "Locate file…" needs to tell the
/// operator where it looked, and Reload needs a path that means something.
#[test]
fn a_missing_model_still_carries_the_path_that_was_searched() {
    let dir = scratch("missing");
    let job = write_job(&dir, "models/absent.dxf");

    let session = ProjectSession::load(&job).expect("a project with a missing model still loads");

    assert_eq!(
        session.models()[0].path,
        dir.join("models/absent.dxf"),
        "the error placeholder carries the resolved path, not the raw string"
    );
    assert!(session.models()[0].polygons.is_none());
    assert!(
        session.models()[0].load_error.is_some(),
        "and the loader's own reason, which the GUI now renders"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// `invalidate_model` is what a relink calls, and it must drop every
/// dependent — including the downstream `FromRemainingStock` op, whose stock
/// comes from the op above it.
///
/// The id does NOT change across a relink, so the signature comparison that
/// catches an operator re-pointing an operation's Input combo sees nothing.
/// Keying on the id is what makes this the right instrument.
#[test]
fn invalidate_model_drops_every_dependent_and_leaves_others_alone() {
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::session::ToolpathConfig;

    fn toolpath(
        id: usize,
        model_id: usize,
        stock: rs_cam_core::compute::config::StockSource,
    ) -> ToolpathConfig {
        ToolpathConfig {
            id: rs_cam_core::ToolpathId(id),
            name: format!("Op {id}"),
            enabled: true,
            operation: OperationConfig::new_default(OperationType::Face),
            dressups: Default::default(),
            heights: Default::default(),
            tool_id: 0,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            rest_analysis: Default::default(),
            stock_source: stock,
            coolant: Default::default(),
            face_selection: None,
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            planner_origin: None,
        }
    }

    fn result() -> rs_cam_core::session::ToolpathComputeResult {
        rs_cam_core::session::ToolpathComputeResult {
            op_data: rs_cam_core::drill_op::OpData::Toolpath(std::sync::Arc::new(
                rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
                    rs_cam_core::toolpath::Toolpath::new(),
                ),
            )),
            stats: Default::default(),
            debug_trace: None,
            semantic_trace: None,
        }
    }

    let mut session = ProjectSession::new_empty();
    session.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    session
        .add_toolpath(
            0,
            toolpath(0, 0, rs_cam_core::compute::config::StockSource::Fresh),
        )
        .unwrap();
    session
        .add_toolpath(
            0,
            toolpath(
                1,
                0,
                rs_cam_core::compute::config::StockSource::FromRemainingStock,
            ),
        )
        .unwrap();
    session
        .add_toolpath(
            0,
            toolpath(2, 1, rs_cam_core::compute::config::StockSource::Fresh),
        )
        .unwrap();
    for index in 0..3 {
        let revision = session.toolpath_revision(index);
        let _ = session
            .apply(rs_cam_core::session::Command::AdoptResult(
                rs_cam_core::session::AdoptResultArgs {
                    index,
                    revision,
                    result: Box::new(result()),
                },
            ))
            .expect("the fixture adopts at the current revision");
    }

    let effects = session.invalidate_model(0);

    assert_eq!(
        effects.stale,
        [0, 1]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        "both operations on model 0"
    );
    assert!(session.get_result(0).is_none());
    assert!(
        session.get_result(1).is_none(),
        "including the one downstream of it — its stock comes from geometry \
         that has just been replaced"
    );
    assert!(
        session.get_result(2).is_some(),
        "an operation on another model is untouched"
    );
    assert!(session.simulation_result().is_none());
}
