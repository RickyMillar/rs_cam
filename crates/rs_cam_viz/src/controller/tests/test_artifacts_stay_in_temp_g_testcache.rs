//! G-TESTCACHE — a test-built controller never writes into the operator's
//! cache folder.
//!
//! A controller test with the default `AppSettings` and debug traces on
//! wrote `toolpath_debug/` files into `~/.cache/rs_cam/artifacts`. One leak
//! was fixed by hand in `stale_cards_g_stalecards.rs`. The seam is now
//! `ArtifactPolicy::from_settings`: every job reads its folder there, and a
//! unit-test build resolves the default folder under `std::env::temp_dir()`.
//! The production resolver (`rs_cam_core::settings::paths::artifact_dir`)
//! does not change.

use crate::compute::ArtifactPolicy;
use crate::controller::AppController;

/// The folder a controller job of `controller` writes into.
fn artifact_dir_of(controller: &AppController) -> std::path::PathBuf {
    ArtifactPolicy::from_settings(&controller.state.app_settings.diagnostics)
        .dir
        .expect("a unit-test build always resolves a folder")
}

#[test]
fn a_test_built_controller_resolves_its_artifact_dir_under_temp_g_testcache() {
    let temp = std::env::temp_dir();
    let mut controller = AppController::new();
    let dir = artifact_dir_of(&controller);
    assert!(
        dir.starts_with(&temp),
        "the default artifact folder of a test must be under {}: {}",
        temp.display(),
        dir.display()
    );
    assert_eq!(dir, crate::compute::worker::test_artifact_dir());
    // Non-vacuity: the production resolver gives the user cache here, so
    // the assertion above tests the seam, not an empty environment.
    if let Some(user_cache) = rs_cam_core::settings::paths::artifact_dir(None) {
        assert_ne!(dir, user_cache, "the test folder is not the user cache");
    }

    // The start settings replace `app_settings` wholesale; the default
    // folder stays under the temp folder.
    controller.apply_startup_settings(rs_cam_core::settings::AppSettings::default());
    assert!(artifact_dir_of(&controller).starts_with(&temp));

    // A value in the settings file still wins.
    let chosen = temp.join(format!("rs_cam_g_testcache_chosen_{}", std::process::id()));
    controller.state.app_settings.diagnostics.artifact_dir = Some(chosen.clone());
    assert_eq!(artifact_dir_of(&controller), chosen);
}
