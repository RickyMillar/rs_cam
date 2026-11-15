//! P35 part C, the evidence run: the strategy advisor on the roughing
//! benchmark (`planning/fixtures/rivmap100/rivmap100.toml`, the toolpath
//! "3D Rough") under each setting of the project feed modulation switch.
//!
//! Operator ruling 2026-11-16 ("Read the switch"): the advisor times each
//! candidate in the mode of the switch. This run prints the choice and the
//! times per setting, so the lead can see if the choice moves between cap
//! only and full. It asserts no winner.
//!
//! ```text
//! cargo test --release -p rs_cam_core --test the_advisor_reads_the_switch_on_rivmap100_p35 \
//!     -- --ignored --nocapture
//! ```

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "an evidence run: a panic reports a failure, and the printed table is the result"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{Command, FeedModulationSwitch, ProjectSession, SetFeedModulationArgs};

/// The benchmark project, from the crate folder.
const PROJECT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../planning/fixtures/rivmap100/rivmap100.toml"
);

#[test]
#[ignore = "evidence: reads planning/fixtures/rivmap100 and plans two roughing candidates three times; run with --release --ignored"]
fn the_advisor_choice_per_switch_setting_on_rivmap100_p35() {
    let mut session = ProjectSession::load(std::path::Path::new(PROJECT)).expect("load rivmap100");
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name == "3D Rough")
        .expect("the project holds the toolpath 3D Rough");
    let cancel = AtomicBool::new(false);
    for switch in FeedModulationSwitch::ALL {
        let _ = session
            .apply(Command::SetFeedModulation(SetFeedModulationArgs { switch }))
            .expect("write the switch");
        let started = std::time::Instant::now();
        let rec = session
            .recommend_clearing_strategy(index, &cancel)
            .expect("the advisor resolves the op")
            .expect("the advisor gives a recommendation");
        eprintln!(
            "switch {}: chosen {:?}, ratio to the runner-up {:.3}, regime {:?} ({:.0} s of compute)",
            switch.word(),
            rec.chosen,
            rec.time_ratio_vs_runner_up,
            rec.regime,
            started.elapsed().as_secs_f64()
        );
        for r in &rec.ranked {
            eprintln!(
                "    {:?}: {:.1} s, regime {:?}",
                r.strategy, r.wall_clock_s, r.regime
            );
        }
        assert!(!rec.ranked.is_empty(), "a candidate has a time");
    }
}
