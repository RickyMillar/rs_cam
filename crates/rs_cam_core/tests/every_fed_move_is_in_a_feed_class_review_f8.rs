//! Review 2026-11-15 F8 (lead ruling): `session::EmittedFeeds` puts every
//! fed move in a class (`FeedClass`: the drill feed, the air lead and the
//! approach included). For each op kind on the standard fixture, the sum
//! of the class counts equals the count of fed moves of the result, so a
//! fed move in no class fails here. The header of a drill phase prints its
//! feeds.
//!
//! `cargo test -p rs_cam_core -q --test every_fed_move_is_in_a_feed_class_review_f8`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;

mod common;
use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};

#[test]
fn every_fed_move_of_every_op_kind_is_in_a_class_review_f8() {
    let mut checked = 0;
    let mut drills = 0;
    let mut classes_seen = std::collections::BTreeSet::new();
    for &op_type in OperationType::ALL {
        let (mut session, index) = case_session(CaseSpec {
            name: "op",
            op: OperationConfig::new_default(op_type),
            prior: Prior::None,
            stock_source: StockSource::Fresh,
            frame: Frame::TopUp,
            fixture: Fixture::Standard,
            tool: None,
        });
        let Ok(result) = session.generate_toolpath(index, &AtomicBool::new(false)) else {
            continue;
        };
        let fed = result
            .toolpath()
            .moves
            .iter()
            .filter(|m| m.move_type.feed_rate().is_some())
            .count();
        let feeds = session.emitted_feeds(index).expect("a result");
        assert_eq!(
            feeds.fed_moves(),
            fed,
            "{op_type:?}: every fed move is in one class: {feeds:?}"
        );
        for (name, present) in [
            ("cutting", feeds.cutting.is_some()),
            ("plunge", feeds.plunge.is_some()),
            ("ramp", feeds.ramp.is_some()),
            ("drill", feeds.drill.is_some()),
            ("approach", feeds.approach.is_some()),
            ("lead", feeds.lead.is_some()),
            ("link", feeds.link.is_some()),
        ] {
            if present {
                let _ = classes_seen.insert(name);
            }
        }
        if feeds.drill.is_some() {
            drills += 1;
            let gcode = session
                .export_gcode_checked(None)
                .expect("a warning never refuses the export")
                .gcode;
            assert!(
                gcode.lines().any(|l| l.contains("drill feeds run")),
                "{op_type:?}: the header of a drill phase prints its feeds"
            );
        }
        checked += 1;
    }
    assert!(checked >= 15, "non-vacuity: {checked} op kinds generated");
    assert!(drills >= 1, "non-vacuity: a drill op");
    for name in ["cutting", "drill", "approach"] {
        assert!(
            classes_seen.contains(name),
            "non-vacuity: some op has {name} moves: {classes_seen:?}"
        );
    }
}
