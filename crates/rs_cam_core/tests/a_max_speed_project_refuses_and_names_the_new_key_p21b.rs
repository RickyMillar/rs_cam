//! Feeds contract P21b (ruling Q16): the spindle strategy token `max_speed`
//! is renamed `constant_chip`. A project file that holds the old token
//! refuses at load with a message that names the new key. No alias reads
//! it and nothing converts it.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::feeds::{MAX_SPEED_TOKEN_RENAMED, SpindleStrategy, SpindleStrategyKind};
use rs_cam_core::gcode::PostConfig;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};

/// Save a session with the post block `post`; return the file text and
/// the path of a scratch directory that holds it.
fn saved_text(name: &str, post: PostConfig) -> (std::path::PathBuf, String) {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.toml");
    ProjectSessionBuilder::new()
        .post(post)
        .build()
        .save(&path)
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    (path, text)
}

#[test]
fn a_max_speed_project_refuses_and_names_the_new_key_p21b() {
    let (path, text) = saved_text("p21b_max_speed", PostConfig::default());
    let old = format!(
        "spindle_strategy = \"{}\"",
        SpindleStrategyKind::MatchChart.token()
    );
    // Non-vacuity: the saved file holds the key the edit replaces.
    assert!(
        text.contains(&old),
        "the saved post block holds `{old}`:\n{text}"
    );
    std::fs::write(
        &path,
        text.replace(&old, "spindle_strategy = \"max_speed\""),
    )
    .unwrap();

    let err = ProjectSession::load(&path)
        .err()
        .expect("a max_speed project refuses at load");
    let message = err.to_string();
    assert!(
        message.contains(MAX_SPEED_TOKEN_RENAMED),
        "the refusal names the new key: {message}"
    );
    assert!(message.contains("constant_chip"), "{message}");
}

#[test]
fn a_constant_chip_project_round_trips_with_its_ceiling_p21b() {
    let post = PostConfig {
        spindle_strategy: SpindleStrategyKind::ConstantChip,
        spindle_rpm_ceiling: Some(PostConfig::default().spindle_speed),
        ..PostConfig::default()
    };
    let (path, text) = saved_text("p21b_constant_chip", post.clone());
    assert!(
        text.contains("spindle_strategy = \"constant_chip\""),
        "{text}"
    );
    assert!(text.contains("spindle_rpm_ceiling = "), "{text}");
    let loaded = ProjectSession::load(&path).unwrap();
    assert_eq!(*loaded.post_config(), post);
    assert_eq!(
        loaded.post_config().spindle_policy(),
        SpindleStrategy::ConstantChip {
            rpm_ceiling: post.spindle_rpm_ceiling
        }
    );

    // An old file with no ceiling key loads as no ceiling, and saves
    // without the key.
    let (path, text) = saved_text("p21b_no_ceiling", PostConfig::default());
    assert!(!text.contains("spindle_rpm_ceiling"), "{text}");
    assert_eq!(
        ProjectSession::load(&path)
            .unwrap()
            .post_config()
            .spindle_rpm_ceiling,
        None
    );
}
