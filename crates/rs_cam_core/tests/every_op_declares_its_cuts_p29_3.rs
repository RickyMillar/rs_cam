//! Feeds contract P29-3, commit B
//! (`planning/feeds_contract_2026-11-03/phases/P29-3_DESIGN.md` sections 2
//! and 6): every op declares its cuts (`OperationConfig::cuts`), and the
//! readers that moved read the same values as before.
//!
//! The configs: each op of `OperationType::ALL` at its default, each
//! `ClearingStrategy` of 3D Rough, 2D Adaptive with `slot_clearing` on and
//! off and with each `cleanup_strategy`, Pocket contour and zigzag with and
//! without a spring pass, Profile with spring passes, Face on the stock
//! and on a drawing region, and Bore with a
//! leave above 0 and at 0.
//! The tool diameter of each op is the fixture tool of
//! `common::op_fixtures::tool_for`.
//!
//! - Claim 1: the power cases of the cut list give the widths of the old
//!   rule: the nominal width, the planner cap
//!   (`planner_width().power_width_mm`), and the slot (`first_pass ==
//!   Slot || radial == Slot`). The rule is written out here, so the oracle
//!   does not read `cuts()`. P29-7 7c: the power fold is the driver over
//!   the declared cuts (`feeds::planned::worst_power`); with a power that
//!   grows with the width it picks the widest old case (a tie goes to the
//!   later case), and `power_case_width` gives the width of each old case.
//!   The lift width (`CutList::full_feed_width`) is the old one too. P29-7 7b: the side
//!   cuts of each config, with the feed rule and the presence, equal a
//!   written-out list (`want_side_cuts`), the list drops no cut and holds
//!   at most `MAX_DECLARED_CUTS`, and each feed rule is
//!   `DeclaredFeed::of_kind`. The cuts stand in the tie order: the body,
//!   the planner cap, the first slot (or the slot ring).
//! - Claim 2: the facts of every declared cut equal the four label
//!   functions, written out here: `RadialThinning::of(family, role)`, the
//!   Pocket roughing stepover test, the leave of the role and the
//!   stock-to-leave field, and the depth rule (the family, the role and,
//!   P29-5, the material sides of the radial rule).
//! - Claim 3: the entry kinds of the list equal
//!   `EntryContract::record_kinds()` of the op's row, in order.
//! - Claim 4: a source scan of `src/` (comments stripped). The catalogue
//!   `FirstPass` and `planner_width(` have no reader outside
//!   `compute/catalog/cuts.rs` and their definitions, except the named
//!   residue below. The residue is exact both ways: a site that moves
//!   leaves the list in the same commit. P29-5 deleted
//!   `RigidityProfile::depth_cap_rule(family, role)` and its needle: every
//!   reader reads `OperationType::body_depth_limit` through the registry
//!   row.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scan indexes its own char buffer"
)]

mod common;

#[path = "common/source_scan.rs"]
mod source_scan;

use std::collections::BTreeSet;
use std::path::Path;

use rs_cam_core::adaptive::CleanupStrategy;
use rs_cam_core::adaptive3d::FeedCapSource;
use rs_cam_core::compute::catalog::{
    CutList, DeclaredFeed, DeclaredWidth, FirstPass, MAX_DECLARED_CUTS, OperationConfig,
    OperationType, PlannerWidth, Presence, Radial, RadialRule,
};
use rs_cam_core::compute::operation_configs::{ClearingStrategy, PocketPattern};
use rs_cam_core::feeds::engagement::PowerCase;
use rs_cam_core::feeds::geometry::RadialThinning;
use rs_cam_core::feeds::planned::{DeclaredCuts, power_case_width, worst_power};
use rs_cam_core::feeds::{OperationFamily, PassRole};
use rs_cam_core::limits::{
    CleanupKind, CutFacts, CutKind, DepthLimit, EntryKind, FirstPassKind, Leave, NoDepthLimit,
};

/// One config with its name.
struct Case {
    name: String,
    op: OperationConfig,
    d: f64,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    let mut add = |name: String, op: OperationConfig| {
        let d = common::op_fixtures::tool_for(op.op_type()).diameter;
        out.push(Case { name, op, d });
    };
    for &op in OperationType::ALL {
        add(op.kind_str().to_owned(), OperationConfig::new_default(op));
    }
    for strategy in [
        ClearingStrategy::GeodesicAdaptive,
        ClearingStrategy::ContourParallel,
    ] {
        let mut op = OperationConfig::new_default(OperationType::Adaptive3d);
        if let OperationConfig::Adaptive3d(c) = &mut op {
            c.clearing_strategy = strategy;
        }
        add(format!("adaptive3d/{strategy:?}"), op);
    }
    for slot in [true, false] {
        let mut op = OperationConfig::new_default(OperationType::Adaptive);
        if let OperationConfig::Adaptive(c) = &mut op {
            c.slot_clearing = slot;
        }
        add(format!("adaptive/slot_clearing={slot}"), op);
    }
    for cleanup in [
        CleanupStrategy::Legacy,
        CleanupStrategy::ResidueMop,
        CleanupStrategy::ContourParallelNarrow,
    ] {
        let mut op = OperationConfig::new_default(OperationType::Adaptive);
        if let OperationConfig::Adaptive(c) = &mut op {
            c.cleanup_strategy = cleanup;
        }
        add(format!("adaptive/cleanup={cleanup:?}"), op);
    }
    for pattern in [PocketPattern::Contour, PocketPattern::Zigzag] {
        for spring in [0, 1] {
            let mut op = OperationConfig::new_default(OperationType::Pocket);
            if let OperationConfig::Pocket(c) = &mut op {
                c.pattern = pattern;
                c.finishing_passes = spring;
            }
            add(format!("pocket/{pattern:?}/spring={spring}"), op);
        }
    }
    let mut op = OperationConfig::new_default(OperationType::Profile);
    if let OperationConfig::Profile(c) = &mut op {
        c.finishing_passes = 2;
    }
    add("profile/spring=2".to_owned(), op);
    // P29-5b: a Face on a drawing region keeps the tool inside the outline,
    // so its first row is a slot.
    let mut op = OperationConfig::new_default(OperationType::Face);
    if let OperationConfig::Face(c) = &mut op {
        c.selection = rs_cam_core::selection::Selection::drawing();
    }
    add("face/drawing".to_owned(), op);
    for leave in [0.3, 0.0] {
        let mut op = OperationConfig::new_default(OperationType::Bore);
        if let OperationConfig::Bore(c) = &mut op {
            c.finish_allowance = leave;
        }
        add(format!("bore/leave={leave}"), op);
    }
    out
}

/// The old rule of the power cases (K7, P23a), written out.
fn old_widths(op: &OperationConfig, ae: f64, d: f64) -> Vec<(PowerCase, f64)> {
    let spec = op.cut_spec();
    let mut out = vec![(PowerCase::Nominal, ae)];
    if let Some(w) = op.planner_width().power_width_mm(ae, d) {
        out.push((PowerCase::PlannerCap, w));
    }
    if matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot) {
        out.push((PowerCase::Slot, d));
    }
    out
}

/// The side cuts of a list, in order (no entry): the kind, the feed rule
/// and `true` for a cut with `Presence::MayOccur`.
fn side_cuts(list: &CutList) -> Vec<(CutKind, DeclaredFeed, bool)> {
    list.iter()
        .filter(|c| !matches!(c.kind, CutKind::Entry(_)))
        .map(|c| (c.kind, c.feed, matches!(c.presence, Presence::MayOccur(_))))
        .collect()
}

/// P29-7 7b: the side cuts of each config, written out (design section
/// 2.2, with the conditions of the generators: a spring pass only with
/// `finishing_passes` above 0, a Bore wall finish only with an allowance
/// above 0, the 2D Adaptive cleanup by `cleanup_strategy`). The tie order
/// of the power cases stands first: the body, the planner cap, the first
/// slot (or the slot ring of a contour Pocket, which is its first slot).
fn want_side_cuts(name: &str) -> Vec<(CutKind, DeclaredFeed, bool)> {
    use DeclaredFeed::{DrillCycle, OpFeed, PlannerCapped, Unlifted};
    let body = (CutKind::Body, OpFeed, false);
    let cap = (CutKind::FirstPass(FirstPassKind::PlannerCap), OpFeed, false);
    let slot = (CutKind::FirstPass(FirstPassKind::Slot), OpFeed, false);
    let maybe_slot = (CutKind::FirstPass(FirstPassKind::Slot), OpFeed, true);
    let ring = (CutKind::SlotRing, Unlifted, false);
    let spring = (CutKind::SpringPass, OpFeed, false);
    let link = (CutKind::Link, OpFeed, false);
    let drill = (CutKind::Drill, DrillCycle, false);
    let wall = (CutKind::WallFinish, OpFeed, false);
    let sweep = (CutKind::Cleanup(CleanupKind::WaterlineSweep), OpFeed, false);
    let mop = (CutKind::Cleanup(CleanupKind::ResidueMop), OpFeed, false);
    let contour_residue = (CutKind::Cleanup(CleanupKind::ContourResidue), OpFeed, false);
    let departure = (
        CutKind::CappedStep(FeedCapSource::DepartureWindow),
        PlannerCapped,
        false,
    );
    let stock_read = (
        CutKind::CappedStep(FeedCapSource::StockRead3d),
        PlannerCapped,
        false,
    );
    match name {
        "face" => vec![body, link],
        "face/drawing" => vec![body, slot, link],
        "pocket" | "pocket/Contour/spring=0" => vec![body, ring],
        "pocket/Contour/spring=1" => vec![body, ring, spring],
        "pocket/Zigzag/spring=0" => vec![body, slot, link],
        "pocket/Zigzag/spring=1" => vec![body, slot, link, spring],
        "profile" => vec![body, slot],
        "profile/spring=2" => vec![body, slot, spring],
        "zigzag" => vec![body, slot, link],
        "adaptive" | "adaptive/slot_clearing=false" => {
            vec![body, cap, contour_residue, departure]
        }
        "adaptive/slot_clearing=true" => vec![body, cap, slot, contour_residue, departure],
        "adaptive/cleanup=Legacy" => vec![body, cap, departure],
        "adaptive/cleanup=ResidueMop" | "adaptive/cleanup=ContourParallelNarrow" => {
            vec![body, cap, mop, departure]
        }
        "adaptive3d" | "adaptive3d/ContourParallel" => vec![body, cap, slot, sweep, stock_read],
        "adaptive3d/GeodesicAdaptive" => vec![body, cap, maybe_slot, sweep, mop, stock_read],
        "v_carve" | "inlay" | "trace" | "project_curve" | "bore" | "bore/leave=0" => {
            vec![body, slot]
        }
        "bore/leave=0.3" => vec![body, slot, wall],
        "drill" | "alignment_pin_drill" => vec![body, drill],
        "inlay_plug" | "chamfer" | "drop_cutter" | "waterline" | "pencil" | "scallop"
        | "unified_finish" | "steep_shallow" | "ramp_finish" | "spiral_finish"
        | "radial_finish" | "horizontal_finish" => vec![body],
        other => panic!("{other}: no written-out list"),
    }
}

#[test]
fn claim_1_the_power_cases_keep_the_widths_in_order() {
    let mut slot_seen = false;
    let mut cap_seen = false;
    for case in cases() {
        let list = case.op.cuts();
        let tool = common::op_fixtures::tool_for(case.op.op_type());
        let declared = DeclaredCuts::of_operation(&case.op, &tool);
        for ae in [0.3, 1.0, 2.5, 0.4 * case.d, case.d] {
            let old = old_widths(&case.op, ae, case.d);
            // The worst case of a power that grows with the width: the
            // widest old case; a tie goes to the later case.
            let mut want = (PowerCase::Nominal, ae);
            for &(c, w) in &old {
                if w >= want.1 {
                    want = (c, w);
                }
            }
            let (nc, _, nw) = worst_power(&declared, ae, |w| w);
            assert_eq!(nc, want.0, "{} ae {ae}: the worst case", case.name);
            assert_eq!(
                nw.to_bits(),
                want.1.to_bits(),
                "{} ae {ae}: {nc:?}",
                case.name
            );
            for &(oc, ow) in &old {
                assert_eq!(
                    power_case_width(&declared, oc, ae).to_bits(),
                    ow.to_bits(),
                    "{} ae {ae}: the width of {oc:?}",
                    case.name
                );
            }
            assert_eq!(
                list.full_feed_width(ae, case.d).to_bits(),
                case.op
                    .planner_width()
                    .full_feed_width_mm(ae, case.d)
                    .to_bits(),
                "{} ae {ae}: the lift width",
                case.name
            );
            slot_seen |= old.iter().any(|(c, _)| *c == PowerCase::Slot);
            cap_seen |= old.iter().any(|(c, _)| *c == PowerCase::PlannerCap);
        }
        // P29-7 7b: the declared cuts, written out; no cut is dropped.
        assert_eq!(
            side_cuts(&list),
            want_side_cuts(&case.name),
            "{}: the declared cuts",
            case.name
        );
        assert_eq!(list.dropped(), None, "{}: a cut was dropped", case.name);
        assert!(list.len() <= MAX_DECLARED_CUTS, "{}: the bound", case.name);
        for cut in list.iter() {
            assert_eq!(
                cut.feed,
                DeclaredFeed::of_kind(cut.kind),
                "{}: the feed rule of {:?}",
                case.name,
                cut.kind
            );
        }
        // A planner cap states the planner width that the power case reads.
        if let Some(cap) = list.of_kind(CutKind::FirstPass(FirstPassKind::PlannerCap)) {
            let ok = match (cap.width, case.op.planner_width()) {
                (DeclaredWidth::PassCeiling { .. }, PlannerWidth::PassCeiling) => true,
                (DeclaredWidth::CapMult { mult, .. }, PlannerWidth::CapMult(m)) => {
                    mult.to_bits() == m.to_bits()
                }
                (
                    DeclaredWidth::Stated(_)
                    | DeclaredWidth::Slot
                    | DeclaredWidth::PassCeiling { .. }
                    | DeclaredWidth::CapMult { .. }
                    | DeclaredWidth::Scallop(_)
                    | DeclaredWidth::None(_),
                    _,
                ) => false,
            };
            assert!(ok, "{}: the planner cap width", case.name);
        }
    }
    assert!(slot_seen && cap_seen, "the configs reach both extra cases");
}

/// The old label functions, written out.
fn old_facts(op: &OperationConfig) -> CutFacts {
    let (family, role) = op.feeds_style();
    // The table of the depth limit (P13c, R2, R3), written out: the
    // product filler `depth_limit` is not the oracle. P29-4 (ruling R1):
    // the Adaptive family has no arm of its own. P29-5 (ruling R3): a slot
    // or a groove (material on both sides) reads the force budget under
    // every role.
    let both_sides = match op.capabilities().cut_rule.radial {
        RadialRule::Slot | RadialRule::Groove => true,
        RadialRule::Stepover
        | RadialRule::Scallop
        | RadialRule::ScallopElseStepover
        | RadialRule::OffsetStepoverElseSurface
        | RadialRule::AngularStep
        | RadialRule::SurfaceFollow
        | RadialRule::NoRadial => false,
    };
    let depth_rule = match (family, role, both_sides) {
        (OperationFamily::Drill, _, _) => DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
        (_, PassRole::Roughing, _) | (_, PassRole::SemiFinish | PassRole::Finish, true) => {
            DepthLimit::ForceBudget
        }
        (_, PassRole::SemiFinish | PassRole::Finish, false) => {
            DepthLimit::NoLimit(NoDepthLimit::FinishPass)
        }
    };
    let leave = match (role, op.radial_stock_to_leave_mm()) {
        (PassRole::Finish | PassRole::SemiFinish, _) => Leave::FinishPass,
        (PassRole::Roughing, None) => Leave::NoField,
        (PassRole::Roughing, Some(mm)) if mm.is_finite() && mm > 0.0 => Leave::Mm(mm),
        (PassRole::Roughing, Some(_)) => Leave::Zero,
    };
    CutFacts {
        thins_chip: RadialThinning::of(family, role),
        offset_ring: family == OperationFamily::Pocket
            && role == PassRole::Roughing
            && matches!(op.cut_spec().radial, Radial::Width(_)),
        leave_after_mm: leave,
        depth_rule,
    }
}

#[test]
fn claim_2_the_facts_equal_the_label_functions() {
    let mut seen = BTreeSet::new();
    for case in cases() {
        let want = old_facts(&case.op);
        let list = case.op.cuts();
        assert_eq!(list.body_facts(), want, "{}: the body facts", case.name);
        for cut in list.iter() {
            assert_eq!(
                cut.facts, want,
                "{}: the facts of {:?}",
                case.name, cut.kind
            );
        }
        seen.insert(format!("{:?}", want.thins_chip));
        seen.insert(format!("ring={}", want.offset_ring));
        seen.insert(format!("{:?}", want.depth_rule));
        let leave = match want.leave_after_mm {
            Leave::FinishPass => "leave:finish",
            Leave::NoField => "leave:nofield",
            Leave::Zero => "leave:zero",
            Leave::Mm(_) => "leave:mm",
        };
        seen.insert(leave.to_owned());
    }
    for v in [
        "Applied",
        "FinishPass",
        "DrillCycle",
        "ring=true",
        "ring=false",
        "ForceBudget",
        "NoLimit(FinishPass)",
        "NoLimit(DrillCycle)",
        "leave:finish",
        "leave:nofield",
        "leave:mm",
        "leave:zero",
    ] {
        assert!(
            seen.contains(v),
            "no config reaches the fact value {v}: {seen:?}"
        );
    }
}

#[test]
fn claim_3_the_entry_kinds_equal_the_contract() {
    let mut entries = 0;
    for case in cases() {
        let got: Vec<EntryKind> = case
            .op
            .cuts()
            .iter()
            .filter_map(|c| match c.kind {
                CutKind::Entry(k) => Some(k),
                CutKind::Body
                | CutKind::FirstPass(_)
                | CutKind::SpringPass
                | CutKind::WallFinish
                | CutKind::EntryApproach
                | CutKind::Cleanup(_)
                | CutKind::CappedStep(_)
                | CutKind::SlotRing
                | CutKind::LeadIn
                | CutKind::LeadOut
                | CutKind::Link
                | CutKind::DressupRelief
                | CutKind::Drill => None,
            })
            .collect();
        let want: Vec<EntryKind> = case
            .op
            .op_type()
            .registry_entry()
            .entry
            .record_kinds()
            .kinds()
            .collect();
        assert_eq!(got, want, "{}: the entry kinds", case.name);
        entries += got.len();
    }
    assert!(entries > OperationType::ALL.len(), "the lists hold entries");
}

/// A needle, the files that may read it (the definitions and `cuts.rs`),
/// and the residue: the files that still read it, each with its reason.
struct Needle {
    text: &'static [&'static str],
    /// A file that holds the needle (non-vacuity).
    anchor: &'static str,
    allowed: &'static [&'static str],
    residue: &'static [(&'static str, &'static str)],
}

const CUTS_RS: &str = "src/compute/catalog/cuts.rs";

const NEEDLES: [Needle; 2] = [
    Needle {
        // The catalogue `FirstPass` (`CutKind::FirstPass` of `limits` does
        // not hold `FirstPass::`) and the field read.
        text: &["FirstPass::", ".first_pass"],
        anchor: CUTS_RS,
        allowed: &[
            CUTS_RS,
            "src/compute/catalog.rs",
            "src/compute/catalog/cut_spec.rs",
        ],
        residue: &[(
            "src/trace/lineage.rs",
            "STAYS (design 5.2): the lineage prints the operator's stated cut",
        )],
    },
    Needle {
        text: &["planner_width("],
        anchor: CUTS_RS,
        allowed: &[CUTS_RS, "src/compute/catalog.rs"],
        residue: &[],
    },
];

#[test]
fn claim_4_no_label_reader_outside_the_cut_list() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = source_scan::rust_files(&root.join("src"));
    assert!(files.len() > 100, "the scan reads the source tree");
    let mut readers: Vec<BTreeSet<String>> = vec![BTreeSet::new(); NEEDLES.len()];
    for file in &files {
        let rel = file
            .strip_prefix(root)
            .expect("a file under the crate")
            .to_string_lossy()
            .replace('\\', "/");
        let text = source_scan::strip_comments(
            &std::fs::read_to_string(file).expect("a source file reads"),
        );
        for (i, needle) in NEEDLES.iter().enumerate() {
            if needle.text.iter().any(|t| text.contains(t)) {
                readers
                    .get_mut(i)
                    .expect("one set per needle")
                    .insert(rel.clone());
            }
        }
    }
    for (needle, found) in NEEDLES.iter().zip(&readers) {
        // Non-vacuity: the scan finds each needle in its anchor.
        assert!(
            found.contains(needle.anchor),
            "{:?}: {} holds it (the scan finds the needle)",
            needle.text,
            needle.anchor
        );
        let residue: BTreeSet<String> = needle
            .residue
            .iter()
            .map(|(f, _)| (*f).to_owned())
            .collect();
        for file in found {
            assert!(
                needle.allowed.contains(&file.as_str()) || residue.contains(file),
                "{file} reads {:?} outside the cut list; read `OperationConfig::cuts`",
                needle.text
            );
        }
        for (file, reason) in needle.residue {
            assert!(
                found.contains(*file),
                "{file} no longer reads {:?} ({reason}); remove it from the residue",
                needle.text
            );
        }
    }
}
