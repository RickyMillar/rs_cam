//! Feeds contract P18 (review D14): the plunge and the ramp feed read the
//! feed BEFORE the radial chip-thinning lift.
//!
//! Source: `planning/feeds_review_2026-11-06/RECHECK_bd72997c9.md` row D14.
//! The Garr lift (`CREDITS.md`, "Garr Tool, Chip Thinning") corrects a
//! narrow radial width. A plunge is axial, and a ramp or helix entry cuts
//! the full width of the tool: neither has radial thinning. Until P18 the
//! plunge was the G10 fraction of the LIFTED feed: a 6 mm 2F flat in
//! hardwood at a 0.3 mm width plunged at the lift x the axial chip of the
//! same tool at full width.
//!
//! One door: `feeds::geometry::UnliftedFeed` (a private field). The plunge
//! door `plunge::resolve_plunge_at` and the ramp door
//! `ramp::resolve_ramp_feed` take it, so the compiler holds that no caller
//! passes the lifted feed. The raw `plunge::resolve_plunge` stays public
//! only for the FM1 instrument column; the source scan below holds that no
//! product source calls it.
//!
//! The arms:
//! - (a) a thin width: the shipped plunge is `floor(F / Z / lift)`, with the
//!   lift of the kept width (computed here by the one law,
//!   `geometry::peak_chip_thinning_lift`);
//! - (b) a full width: the lift is 1 and the plunge is `floor(F / Z)`, as
//!   before P18;
//! - (c) a ramp entry at the thin width: the cut term of the G6 ramp is the
//!   feed before the lift, and the ramp feed is at or below it;
//! - (d) the source scan (core source): every product plunge and ramp site
//!   calls the door.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::Path;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::geometry::peak_chip_thinning_lift;
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, SuggestWarning, SuggestedParams,
    suggest_for_operation,
};
use rs_cam_core::feeds::{RampFeed, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The tool of cell `P_thin_005`: a 6 mm 2F flat end mill.
const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
/// The stated width of cell `P_thin_005` (0.05 x D).
const THIN_AE_MM: f64 = 0.3;
/// The stated depth of cell `P_thin_005` (1 x D).
const AP_MM: f64 = 6.0;

fn flat() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = D_MM;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    t.set_cutting_length(ToolFact::Stated(D_MM * 4.0));
    t.set_shank_diameter(ToolFact::Stated(D_MM));
    t.shaft_diameter = D_MM;
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn pocket(ae_mm: f64) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Pocket);
    op.set_stepover(ae_mm);
    op.set_depth_per_pass(AP_MM);
    op
}

fn suggest(operation: &OperationConfig, dressups: Option<&DressupConfig>) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation,
        tool: &flat(),
        machine: &MachineProfile::generic_wood_router(),
        material: &hardwood(),
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject {
            dressups,
            ..NoProject::NONE
        }),
    })
    .unwrap_or_else(|e| panic!("Suggest must serve the 6 mm flat Pocket: {e}"))
}

/// The G10 fraction of the claim (1 / Z for a flat end mill).
fn fraction(s: &SuggestedParams) -> f64 {
    s.feeds_result
        .plunge
        .claim()
        .expect("the G10 flat claim covers the 6 mm 2F flat")
        .fraction
}

/// The lift at the kept width `ae_mm`. A flat end mill engages its full
/// diameter at every depth, so `d_eng` is D.
fn lift_at(ae_mm: f64) -> f64 {
    peak_chip_thinning_lift(ae_mm, D_MM)
}

/// (a) A thin width: the plunge is the fraction of the feed before the lift.
#[test]
fn a_thin_width_plunges_at_the_feed_before_the_lift_p18() {
    let s = suggest(&pocket(THIN_AE_MM), None);
    let lift = lift_at(THIN_AE_MM);
    // Non-vacuity: the width thins, so the two feeds differ.
    assert!(lift > 2.0, "the 0.3 mm width lifts the feed: {lift}");
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let plunge = s
        .operation
        .plunge_rate()
        .expect("Suggest writes the plunge");
    let unlifted = feed / lift;
    let want = (fraction(&s) * unlifted).min(unlifted).floor();
    assert_eq!(
        plunge, want,
        "feed {feed}, lift {lift}: the plunge reads {unlifted} (before the lift)"
    );
    // The lifted rule gives a higher plunge: the old defect.
    assert!(
        plunge < (fraction(&s) * feed).floor(),
        "the plunge is below the fraction of the lifted feed"
    );
    // The calculator's own plunge reads the same door.
    let calc_lift = s.feeds_result.derates.radial_chip_thinning;
    assert!(
        calc_lift > 1.0,
        "the calculator lifted the feed: {calc_lift}"
    );
    let calc_unlifted = s.feeds_result.unlifted_feed().mm_min();
    assert!(
        (calc_unlifted - s.feeds_result.feed_rate_mm_min / calc_lift).abs() < 1e-9,
        "{calc_unlifted}"
    );
    assert!(
        s.feeds_result.plunge_rate_mm_min <= (fraction(&s) * calc_unlifted).floor(),
        "calculator plunge {} over the rule at the feed before the lift {calc_unlifted}",
        s.feeds_result.plunge_rate_mm_min
    );
}

/// (b) A full width (a slot): the lift is 1 and the plunge is unchanged.
#[test]
fn a_full_width_keeps_its_plunge_p18() {
    let s = suggest(&pocket(D_MM), None);
    assert_eq!(lift_at(D_MM), 1.0, "a slot does not thin");
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let plunge = s
        .operation
        .plunge_rate()
        .expect("Suggest writes the plunge");
    assert_eq!(
        plunge,
        (fraction(&s) * feed).min(feed).floor(),
        "feed {feed}"
    );
}

/// The one `RampFeed` record of an apply.
fn ramp_record(s: &SuggestedParams) -> &RampFeed {
    s.warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RampFeed { record, .. } => Some(record),
            SuggestWarning::UnsourcedRowsAccepted { .. }
            | SuggestWarning::ComputedWidthAssumptions
            | SuggestWarning::OutsideEnvelope { .. }
            | SuggestWarning::PlungeClampedToFeed { .. }
            | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
            | SuggestWarning::StrategyRecommendedNotApplied { .. }
            | SuggestWarning::FeedRescaledToFinalGeometry { .. }
            | SuggestWarning::FeedClampedToChiploadFloor { .. }
            | SuggestWarning::PowerRecheckedAfterRescale { .. }
            | SuggestWarning::CutGeometryFieldNotHeld { .. }
            | SuggestWarning::RpmLoweredForFeedCeiling { .. }
            | SuggestWarning::PlungeReDerived { .. } => None,
        })
        .expect("a ramp entry files one RampFeed record")
}

/// (c) A ramp entry at the thin width: the cut term is the feed before the
/// lift, and the ramp feed does not exceed it.
#[test]
fn a_ramp_entry_reads_the_feed_before_the_lift_p18() {
    // A 3 degree ramp: a shallow slope, so the cut term of the G6 rule binds
    // or the chip term does; the test reads the record for both arms.
    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 3.0,
        ..DressupConfig::default()
    };
    let s = suggest(&pocket(THIN_AE_MM), Some(&dressups));
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let unlifted = feed / lift_at(THIN_AE_MM);
    let ramp = s
        .operation
        .ramp_feed_rate()
        .expect("Suggest writes the ramp");
    assert!(ramp <= unlifted.floor(), "ramp {ramp} over {unlifted}");
    match ramp_record(&s) {
        RampFeed::Sourced(r) => {
            assert_eq!(r.cut_feed_mm_min, unlifted, "the cut term");
            assert_eq!(ramp, r.cut_feed_mm_min.min(r.chip_term_mm_min).floor());
        }
        RampFeed::PlungeSlope(r) => {
            assert_eq!(r.cut_feed_mm_min, unlifted, "the cut term");
        }
        RampFeed::NoEntryFeed { reason, .. } => {
            panic!("a ramp entry on a G6 tool ships a ramp feed: {reason:?}")
        }
    }
}

/// The comment-free text of every `.rs` file under `dir`, with its path.
fn sources(dir: &Path) -> Vec<(String, String)> {
    source_scan::rust_files(dir)
        .into_iter()
        .map(|p| {
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            (
                p.display().to_string().replace('\\', "/"),
                source_scan::strip_comments(&text),
            )
        })
        .collect()
}

/// (d) Every product site reads the door. The raw resolver is called only
/// inside `feeds/plunge.rs`; the typed door has its three product callers
/// (the calculator, the provenance, the apply funnel); the funnel builds the
/// feed at the kept geometry for the plunge and the ramp.
#[test]
fn every_plunge_and_ramp_site_reads_the_door_p18() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // The core source only: the resolvers are core items, and a scan of
    // viz source belongs in the viz tests (`tests/CLAUDE.md`).
    let files = sources(&root.join("src"));
    assert!(files.len() > 100, "non-vacuity: the scan reads the sources");
    let raw: Vec<&String> = files
        .iter()
        .filter(|(path, text)| {
            text.contains("resolve_plunge(") && !path.ends_with("feeds/plunge.rs")
        })
        .map(|(path, _)| path)
        .collect();
    assert!(
        raw.is_empty(),
        "the raw plunge resolver outside its module: {raw:?}"
    );
    let count = |needle: &str, file: &str| -> usize {
        files
            .iter()
            .filter(|(path, _)| path.ends_with(file))
            .map(|(_, text)| text.matches(needle).count())
            .sum()
    };
    for file in [
        "feeds/calculator/close.rs",
        "feeds/provenance.rs",
        "feeds/suggest/apply.rs",
    ] {
        assert!(
            count("resolve_plunge_at(", file) >= 1,
            "{file} calls the typed plunge door"
        );
    }
    assert_eq!(
        count("unlifted_feed_at_kept_geometry(", "feeds/suggest/apply.rs"),
        2,
        "the funnel's plunge and ramp read the door at the kept geometry"
    );
    assert_eq!(
        count("resolve_ramp_feed(", "feeds/suggest/apply.rs"),
        1,
        "the funnel is the one product ramp site"
    );
}
