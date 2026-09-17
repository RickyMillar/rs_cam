//! G-MODELRELINK — a missing model, an absolute path, and a refused
//! relink to another kind.

use super::*;

/// THE repair route, end to end: a project opens with its model missing, the
/// operator locates the file, and the operations built on it survive — but
/// are no longer current, because the geometry changed under them.
#[test]
fn missing_model_relink_g_modelrelink() {
    let dir = relink_fixture_dir("relink");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();

    controller
        .open_job_from_path(&job)
        .expect("a project with a missing model still opens");

    // The path is RESOLVED, not the raw string. Pre-fix the session stored
    // "models/missing.stl", which resolves against the process working
    // directory — so Reload looked somewhere nobody had searched and the
    // warning named a directory that did not exist.
    assert_eq!(
        controller.state.session.models()[0].path,
        dir.join("models/missing.stl"),
        "an in-memory model path is absolute, whatever the file said"
    );
    assert!(controller.state.session.models()[0].mesh.is_none());
    assert!(
        controller.state.session.models()[0].load_error.is_some(),
        "the loader's reason is kept, and is now rendered"
    );
    assert_eq!(controller.load_warnings().len(), 1);

    // Make the toolpath look generated, so the invalidation assertion below
    // is not comparing two empty caches.
    let revision = controller.state.session.toolpath_revision(0);
    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result()),
        }))
        .expect("index 0 exists");
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let rt = controller.state.gui.toolpath_rt_or_default(tp_id);
    rt.status = crate::state::toolpath::ComputeStatus::Done;
    rt.stale_since = None;
    // The GUI's drawable copy too, or the assertion below cannot tell
    // `EditedSince` (had geometry, now out of date) from `NoResult` (never
    // generated) — which is the distinction the whole card vocabulary rests
    // on.
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            Toolpath::new(),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);

    // The operator locates the file.
    let located = dir.join("models/plate.stl");
    std::fs::copy(fixture_path("flat_plate.stl"), &located).expect("stage the located file");
    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        located.clone(),
    ));

    let model = &controller.state.session.models()[0];
    assert!(model.mesh.is_some(), "the geometry loaded");
    assert!(model.load_error.is_none(), "and the complaint is gone");
    assert_eq!(model.path, located, "the model now points at the new file");
    assert_eq!(model.id, 1, "IDENTITY IS KEPT — this is the whole point");
    assert_eq!(
        model.name, "Plate",
        "and so is the name the operator gave it"
    );

    // The dependents. The model id did NOT change, so the signature
    // comparison that catches a re-pointed Input combo sees nothing here —
    // `invalidate_model` keys on the id, which is why it is the right
    // instrument for a relink.
    assert!(
        controller.state.session.get_result(0).is_none(),
        "every result built on the old geometry is dropped"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and the operation says so on every surface"
    );
    assert!(
        controller.state.gui.toolpath_rt[&tp_id]
            .stale_since
            .is_some(),
        "with a regeneration requested"
    );
    assert!(controller.state.gui.dirty);
    assert!(
        controller.load_warnings().is_empty(),
        "the complaint goes when the thing it complained about is repaired"
    );

    // The saved path is RELATIVE, because the model is under the project
    // directory — which is what lets the folder be copied or moved.
    controller.save_job_to_path(&job).expect("save");
    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains("path = \"models/plate.stl\""),
        "a model under the project directory is stored relative to it:\n{written}"
    );

    // And it reopens.
    controller.open_job_from_path(&job).expect("reopen");
    assert!(controller.state.session.models()[0].mesh.is_some());
    assert!(controller.load_warnings().is_empty());
    assert_eq!(controller.state.session.models()[0].path, located);

    let _ = std::fs::remove_dir_all(&dir);
}

/// A model OUTSIDE the project directory has no shorter honest description
/// than its absolute path, and gets one.
#[test]
fn a_model_outside_the_project_directory_saves_absolute_g_modelrelink() {
    let dir = relink_fixture_dir("relink_abs");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();
    controller.open_job_from_path(&job).expect("open");

    let outside = fixture_path("flat_plate.stl");
    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        outside.clone(),
    ));
    assert!(controller.state.session.models()[0].mesh.is_some());

    controller.save_job_to_path(&job).expect("save");
    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains(&format!("path = \"{}\"", outside.display())),
        "a model the project folder does not contain is stored absolute:\n{written}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A relink to a different KIND is refused. "This file moved" and "use a
/// different model" are different operations; the second is the Input combo,
/// and a mesh operation cannot run on polygons.
#[test]
fn a_relink_to_another_kind_is_refused_g_modelrelink() {
    let dir = relink_fixture_dir("relink_kind");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();
    controller.open_job_from_path(&job).expect("open");
    let before = controller.state.session.models()[0].path.clone();

    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        fixture_path("square.svg"),
    ));

    let model = &controller.state.session.models()[0];
    assert_eq!(model.kind, Some(crate::state::job::ModelKind::Stl));
    assert_eq!(model.path, before, "the model is untouched");
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("Cannot relink")),
        "and the operator is told why, rather than the refusal being silent"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
