//! Feeds contract P29-3, commit B
//! (`planning/feeds_contract_2026-11-03/phases/P29-3_DESIGN.md` sections 2
//! and 6): every op declares its cuts (`OperationConfig::cuts`), and the
//! readers that moved read the same values as before.
//!
//! The configs: each op of `OperationType::ALL` at its default, each
//! `ClearingStrategy` of 3D Rough, 2D Adaptive with `slot_clearing` on and
//! off, Pocket contour and zigzag, and Bore with a leave above 0 and at 0.
//! The tool diameter of each op is the fixture tool of
//! `common::op_fixtures::tool_for`.
//!
//! - Claim 1: the power cases of the cut list
//!   (`PowerCases::of_operation`) give the widths of the old rule, in order:
//!   the nominal width, the planner cap (`planner_width().power_width_mm`),
//!   and the slot (`first_pass == Slot || radial == Slot`). The rule is
//!   written out here, so the oracle does not read `cuts()`. The lift
//!   width (`full_feed_width`) is the old one too. The cuts stand in the
//!   tie order: the body, the planner cap, the first slot.
//! - Claim 2: the facts of every declared cut equal the four label
//!   functions, written out here: `RadialThinning::of(family, role)`, the
//!   Pocket roughing stepover test, the leave of the role and the
//!   stock-to-leave field, and the depth rule of `depth_cap_rule`.
//! - Claim 3: the entry kinds of the list equal
//!   `EntryContract::record_kinds()` of the op's row, in order.
//! - Claim 4: a source scan of `src/` (comments stripped). The catalogue
//!   `FirstPass`, `planner_width(` and `depth_cap_rule(` have no reader
//!   outside `compute/catalog/cuts.rs` and their definitions, except the
//!   named residue below. The residue is exact both ways: a site that
//!   moves leaves the list in the same commit.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scan indexes its own char buffer"
)]

mod common;

#[path = "common/source_scan.rs"]
mod source_scan;

use std::collections::BTreeSet;
use std::path::Path;

use rs_cam_core::compute::catalog::{
    CutList, DeclaredWidth, FirstPass, OperationConfig, OperationType, PlannerWidth, Radial,
};
use rs_cam_core::compute::operation_configs::{ClearingStrategy, PocketPattern};
use rs_cam_core::feeds::engagement::{PowerCase, PowerCases};
use rs_cam_core::feeds::geometry::RadialThinning;
use rs_cam_core::feeds::{OperationFamily, PassRole};
use rs_cam_core::limits::{
    CutFacts, CutKind, DepthLimit, DepthRule, EntryKind, FirstPassKind, Leave, NoDepthLimit,
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
    for pattern in [PocketPattern::Contour, PocketPattern::Zigzag] {
        let mut op = OperationConfig::new_default(OperationType::Pocket);
        if let OperationConfig::Pocket(c) = &mut op {
            c.pattern = pattern;
        }
        add(format!("pocket/{pattern:?}"), op);
    }
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

/// The kinds of the side cuts of a list, in order (no entry).
fn side_kinds(list: &CutList) -> Vec<CutKind> {
    list.iter()
        .map(|c| c.kind)
        .filter(|k| !matches!(k, CutKind::Entry(_)))
        .collect()
}

#[test]
fn claim_1_the_power_cases_keep_the_widths_in_order() {
    let mut slot_seen = false;
    let mut cap_seen = false;
    for case in cases() {
        // A list over `MAX_DECLARED_CUTS` drops its last cuts, the entries:
        // claim 3 then goes red.
        let list = case.op.cuts();
        let cases = PowerCases::of_operation(&case.op);
        for ae in [0.3, 1.0, 2.5, 0.4 * case.d, case.d] {
            let new = cases.widths(ae, case.d);
            let old = old_widths(&case.op, ae, case.d);
            assert_eq!(
                new.len(),
                old.len(),
                "{} ae {ae}: the case count",
                case.name
            );
            for ((nc, nw), (oc, ow)) in new.iter().zip(&old) {
                assert_eq!(nc, oc, "{} ae {ae}: the case order", case.name);
                assert_eq!(nw.to_bits(), ow.to_bits(), "{} ae {ae}: {nc:?}", case.name);
            }
            assert_eq!(
                cases.full_feed_width(ae, case.d).to_bits(),
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
        // The tie order of the declared cuts: body, planner cap, slot.
        let spec = case.op.cut_spec();
        let mut want = vec![CutKind::Body];
        match case.op.planner_width() {
            PlannerWidth::None => {}
            PlannerWidth::PassCeiling | PlannerWidth::CapMult(_) => {
                want.push(CutKind::FirstPass(FirstPassKind::PlannerCap));
            }
        }
        if matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot) {
            want.push(CutKind::FirstPass(FirstPassKind::Slot));
        }
        let pocket_contour = matches!(
            &case.op,
            OperationConfig::Pocket(c) if c.pattern == PocketPattern::Contour
        );
        if pocket_contour {
            want.push(CutKind::SlotRing);
        }
        if matches!(
            case.op.op_type(),
            OperationType::Drill | OperationType::AlignmentPinDrill
        ) {
            want.push(CutKind::Drill);
        }
        assert_eq!(side_kinds(&list), want, "{}: the declared cuts", case.name);
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
    // The table of `RigidityProfile::depth_cap_rule` (P13c, R2), written
    // out: the product filler `depth_limit` is not the oracle.
    let depth_rule = match (family, role) {
        (OperationFamily::Drill, _) => DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
        (OperationFamily::Adaptive, _) => DepthLimit::Rule(DepthRule::Factor),
        (_, PassRole::Roughing) => DepthLimit::Rule(DepthRule::ForceBudget),
        (_, PassRole::SemiFinish | PassRole::Finish) => {
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
        "Rule(Factor)",
        "Rule(ForceBudget)",
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

const NEEDLES: [Needle; 3] = [
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
    Needle {
        text: &["depth_cap_rule("],
        // The cut list fills the fact through `RigidityProfile::depth_limit`,
        // the match that `depth_cap_rule` reads.
        anchor: "src/machine/mod.rs",
        allowed: &[CUTS_RS, "src/machine/mod.rs"],
        residue: &[
            (
                "src/tool_load/depth.rs",
                "WAITS: the gate context holds the op kind, not the config",
            ),
            (
                "src/tool_load/cut_force.rs",
                "WAITS: the gate context holds the op kind, not the config",
            ),
            (
                "src/feeds/force_budget.rs",
                "WAITS: rigidity_depth_cap takes (family, role); a viz test fixture calls it",
            ),
        ],
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
