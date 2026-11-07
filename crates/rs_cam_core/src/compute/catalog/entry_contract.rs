//! CONTRACT K4: every op states how it enters material (feeds contract P3).
//!
//! [`EntryContract`] is a required field of each registry row
//! (`OpRegistryEntry::entry`). The row has no `Default` and no row uses
//! `..`, so a new op does not compile until it states its entry. The
//! contract states the entry behaviour of today; no number moves here.
//!
//! The readers:
//! - `DressupConfig::for_op` and `DressupConfig::normalize_for_op` set the
//!   dressup entry style from the contract.
//! - `PipelineRun::effective_entry_style` (the dressup pipeline) gives no
//!   dressup entry to an op whose contract is not
//!   [`EntryContract::Dressup`].
//! - `JobPhases::run_stages` builds one [`EntryRecord`] per entry
//!   ([`entry_records`]) into `Produced::entries`. No stage that writes
//!   moves reads a record.
//!
//! No type in this file has `Default`. The refusals are `compile_fail`
//! doctests on [`entry_contract_refusals`].

use crate::geo::P3;
use crate::material::FormulaConstant;
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// CONTRACT K4. How one op enters material. A required registry field; no
/// `Default`. Each variant states what the compiler refuses in its doc
/// (see [`entry_contract_refusals`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryContract {
    /// The dressup builds the entry (`DressupConfig::entry_style`). The
    /// operator picks a kind in `allowed`, which is not empty.
    Dressup {
        /// The kinds the dressup entry style can give.
        allowed: EntrySet,
        /// The style that a new toolpath starts with.
        default: RoleDefault,
        /// The entry that the style None gives for this op.
        none_is: NoneEntry,
        /// The kinds the op builds itself beside the dressup entry, or
        /// `None` (Pencil: its crease ramp on rest stock).
        own: Option<EntrySet>,
    },
    /// The planner builds its own entries (3D Rough, Unified Finish). The
    /// dressup entry style is None.
    Planner {
        /// The kinds the planner can build.
        kinds: EntrySet,
    },
    /// The descent is the cut (Drill, Pin Drill, Bore). The op has no
    /// entry move before its first cutting move. The dressup entry style
    /// is None.
    DescentIsCut,
    /// A straight plunge is the only entry. The dressup entry style is
    /// None. `reason` is the text of the row.
    ///
    /// G-CENTRECUT (K5, P7 C; ruling Q8): the dressup fallback ramp does
    /// not fire on this contract. A `No(..)` tool keeps its plunges and
    /// warns on each one deeper than the blade protrusion; an `Unknown`
    /// tool stops generation with the D12 reason.
    StraightPlunge {
        /// Why the op takes no dressup entry.
        reason: &'static str,
        /// The feed of the plunge.
        feed: PlungeFeedRule,
    },
}

impl EntryContract {
    /// The contract of an op that takes any dressup entry: the role sets
    /// the style of a new toolpath, None is the plunge of the generator,
    /// and the op builds no entry of its own.
    pub const ANY_DRESSUP_FROM_ROLE: Self = Self::Dressup {
        allowed: EntrySet::ALL_DRESSUP,
        default: RoleDefault::FromRole,
        none_is: NoneEntry::GeneratorPlunge,
        own: None,
    };

    /// True when generation reads the dressup entry style of the op.
    #[must_use]
    pub const fn entry_style_applies(self) -> bool {
        match self {
            Self::Dressup { .. } => true,
            Self::Planner { .. } | Self::DescentIsCut | Self::StraightPlunge { .. } => false,
        }
    }

    /// The entry kinds that an [`EntryRecord`] of this op can have.
    ///
    /// - `Dressup`: `allowed`, `own`, and the kind of `none_is` (a plunge).
    /// - `Planner`: `kinds`.
    /// - `DescentIsCut`: none ([`entry_records`] gives no record).
    /// - `StraightPlunge`: `Plunge`.
    ///
    /// G-ISOCLIPENTRY is a session stage, not a kind of the contract: on a
    /// remaining-stock pass of an op with a surface probe, the session can
    /// turn a plunge into the rest ramp ladder (`optimize_entry_descents`),
    /// which records as `Ramp` under any contract.
    #[must_use]
    pub const fn record_kinds(self) -> EntrySet {
        match self {
            Self::Dressup { allowed, own, .. } => {
                let with_own = match own {
                    Some(own) => allowed.union(own),
                    None => allowed,
                };
                with_own.union(EntrySet::of(&[EntryKind::Plunge]))
            }
            Self::Planner { kinds } => kinds,
            Self::DescentIsCut => EntrySet::EMPTY,
            Self::StraightPlunge { .. } => EntrySet::of(&[EntryKind::Plunge]),
        }
    }
}

/// One kind of entry into material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A straight descent at one XY.
    Plunge,
    /// A descent along a line or a zigzag.
    Ramp,
    /// A descent along a helix.
    Helix,
    /// Plunges at one XY with a retract between them (3D Rough).
    PeckLadder,
}

impl EntryKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 4] = [Self::Plunge, Self::Ramp, Self::Helix, Self::PeckLadder];

    const fn bit(self) -> u8 {
        match self {
            Self::Plunge => 1,
            Self::Ramp => 2,
            Self::Helix => 4,
            Self::PeckLadder => 8,
        }
    }
}

/// A set of entry kinds. Not empty by construction: `EntrySet::of(&[])`
/// panics, so in a `const` it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntrySet(u8);

impl EntrySet {
    /// The three styles of the dressup entry: plunge, ramp and helix.
    pub const ALL_DRESSUP: Self = Self::of(&[EntryKind::Plunge, EntryKind::Ramp, EntryKind::Helix]);

    /// The set with no kind. Only [`EntryContract::record_kinds`] of
    /// `DescentIsCut` gives it; no contract holds it.
    const EMPTY: Self = Self(0);

    /// The set of `kinds`.
    ///
    /// # Panics
    ///
    /// When `kinds` is empty. In a `const` the panic is a compile error.
    #[must_use]
    pub const fn of(kinds: &[EntryKind]) -> Self {
        assert!(!kinds.is_empty(), "an entry set holds at least one kind");
        let mut bits = 0_u8;
        let mut rest = kinds;
        while let [first, tail @ ..] = rest {
            bits |= first.bit();
            rest = tail;
        }
        Self(bits)
    }

    /// True when the set holds `kind`.
    #[must_use]
    pub const fn contains(self, kind: EntryKind) -> bool {
        self.0 & kind.bit() != 0
    }

    /// The kinds of this set or of `other`.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The kinds of the set, in the order of [`EntryKind::ALL`].
    pub fn kinds(self) -> impl Iterator<Item = EntryKind> {
        EntryKind::ALL
            .into_iter()
            .filter(move |k| self.contains(*k))
    }
}

/// The dressup entry style that a new toolpath starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleDefault {
    /// The style of `DressupConfig::for_role` (Roughing: Ramp; SemiFinish
    /// and Finish: None).
    FromRole,
    /// Helix (G10 D3, operator decision 2026-09-25). Only
    /// `DressupConfig::for_op` reads it; `normalize_for_op` never changes
    /// the style of the operator.
    Helix,
}

/// The entry that the dressup style None gives for an op today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoneEntry {
    /// The plunge that the generator emits.
    GeneratorPlunge,
    /// The stock-aware plunge of the dressup pipeline: a rapid through air,
    /// then a fed plunge (`Capabilities::transform.stock_aware_plunge`,
    /// G-ADAPTPASSLOAD).
    StockAwarePlunge,
}

/// The plunge feeds, named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlungeFeedRule {
    /// The plunge rate of the op. Suggest writes it through the one
    /// vertical-rate limit (`feeds::vertical::VerticalRateLimit`, P20b: the
    /// G6 axial chip and the machine Z axis rate). P20b deleted the
    /// `AxialChip` variant (Q8): the limit now bounds every op, so the
    /// contract no longer states it.
    OpPlungeRate,
    /// The feed of the dressup entry for the air lead and for the plunge
    /// fallback: the first linear feed of the toolpath times
    /// [`ENTRY_PLUNGE_FIRST_FEED_FRACTION`], else
    /// [`ENTRY_PLUNGE_FALLBACK_MM_MIN`]. The emitters then take the `min`
    /// with the feed of the plunge move. [`Self::first_linear_feed_fraction`]
    /// is the one evaluation.
    FirstLinearFeedFraction,
}

/// F201: the fraction of the first linear feed that the dressup entry
/// plunges at ([`PlungeFeedRule::FirstLinearFeedFraction`]).
pub const ENTRY_PLUNGE_FIRST_FEED_FRACTION: FormulaConstant = FormulaConstant::unsourced(
    "dressup.entry_plunge_rate.first_linear_feed_fraction",
    0.5,
    "repo rule with no primary source (dressup_apply.rs entry_plunge_rate, feeds contract P3)",
);

/// F201: the plunge feed of the dressup entry (mm/min) when the toolpath has
/// no linear move ([`PlungeFeedRule::FirstLinearFeedFraction`]).
pub const ENTRY_PLUNGE_FALLBACK_MM_MIN: FormulaConstant = FormulaConstant::unsourced(
    "dressup.entry_plunge_rate.fallback_mm_min",
    500.0,
    "repo rule with no primary source (dressup_apply.rs entry_plunge_rate, feeds contract P3)",
);

impl PlungeFeedRule {
    /// The one evaluation of [`Self::FirstLinearFeedFraction`] on
    /// `toolpath`: the first linear feed times
    /// [`ENTRY_PLUNGE_FIRST_FEED_FRACTION`], else
    /// [`ENTRY_PLUNGE_FALLBACK_MM_MIN`].
    #[must_use]
    pub fn first_linear_feed_fraction(toolpath: &Toolpath) -> f64 {
        toolpath
            .moves
            .iter()
            .find_map(|m| match m.move_type {
                MoveType::Linear { feed_rate } => {
                    Some(feed_rate * ENTRY_PLUNGE_FIRST_FEED_FRACTION.value)
                }
                MoveType::Rapid | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => None,
            })
            .unwrap_or(ENTRY_PLUNGE_FALLBACK_MM_MIN.value)
    }
}

/// CONTRACT K4 (record). One entry of a generated toolpath. The fields are
/// private; only [`entry_records`] builds one.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryRecord {
    kind: EntryKind,
    approach_from: P3,
    from: P3,
    to: P3,
    feed_mm_min: f64,
}

impl EntryRecord {
    /// The kind of the entry.
    #[must_use]
    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    /// The tool position before the first fed move of the entry (the end
    /// of the move before the run).
    #[must_use]
    pub fn approach_from(&self) -> P3 {
        self.approach_from
    }

    /// The start of the first move of the entry that is not an air lead
    /// (`MoveIntent::EntryApproach`). It is `approach_from` when the run
    /// has no air lead.
    #[must_use]
    pub fn from(&self) -> P3 {
        self.from
    }

    /// The end of the last move of the entry.
    #[must_use]
    pub fn to(&self) -> P3 {
        self.to
    }

    /// The lowest feed of the moves of the entry that are not an air lead
    /// (mm/min). The feed of the air lead when the run is only air.
    #[must_use]
    pub fn feed_mm_min(&self) -> f64 {
        self.feed_mm_min
    }

    /// The same record moved by `shift` (`Produced::translated`).
    #[must_use]
    pub(crate) fn translated(&self, shift: P3) -> Self {
        let move_by = |p: P3| P3::new(p.x + shift.x, p.y + shift.y, p.z + shift.z);
        Self {
            kind: self.kind,
            approach_from: move_by(self.approach_from),
            from: move_by(self.from),
            to: move_by(self.to),
            feed_mm_min: self.feed_mm_min,
        }
    }
}

/// True for the intents of an entry run.
const fn is_entry_intent(intent: MoveIntent) -> bool {
    match intent {
        MoveIntent::EntryApproach
        | MoveIntent::EntryPlunge
        | MoveIntent::EntryRamp
        | MoveIntent::EntryHelix => true,
        MoveIntent::Drilling
        | MoveIntent::ClearingCut
        | MoveIntent::FinishingCut
        | MoveIntent::Linking
        | MoveIntent::Retract
        | MoveIntent::LeadIn
        | MoveIntent::LeadOut
        | MoveIntent::ObstacleClearance
        | MoveIntent::Unknown => false,
    }
}

/// XY tolerance of a peck ladder: the plunges of one ladder are at one XY.
const LADDER_XY_EPS_MM: f64 = 1e-6;

/// One record per entry of `toolpath` under `contract`.
///
/// An entry is a maximal run of fed moves with an entry intent
/// (`EntryApproach`, `EntryPlunge`, `EntryRamp`, `EntryHelix`). A rapid at
/// the same XY between two plunge runs (a peck retract) joins them into one
/// entry. The kind comes from the run: a helix move gives `Helix`, else a
/// ramp move gives `Ramp`, else two or more plunge runs at one XY give
/// `PeckLadder`, else `Plunge`.
///
/// `DescentIsCut` gives no record: the descent is the cut (`Drilling`, or
/// the cut helix of a Bore), not an entry.
pub(crate) fn entry_records(toolpath: &Toolpath, contract: EntryContract) -> Vec<EntryRecord> {
    match contract {
        EntryContract::DescentIsCut => return Vec::new(),
        EntryContract::Dressup { .. }
        | EntryContract::Planner { .. }
        | EntryContract::StraightPlunge { .. } => {}
    }
    let moves = &toolpath.moves;
    let mut records = Vec::new();
    let mut i = 0;
    while i < moves.len() {
        let fed_entry = |k: usize| {
            moves
                .get(k)
                .is_some_and(|m| m.move_type.is_cutting() && is_entry_intent(m.intent))
        };
        if !fed_entry(i) {
            i += 1;
            continue;
        }
        let approach_from = i
            .checked_sub(1)
            .and_then(|k| moves.get(k))
            .map_or(P3::new(0.0, 0.0, 0.0), |m| m.target);
        let mut from: Option<P3> = None;
        let mut has_helix = false;
        let mut has_ramp = false;
        let mut plunge_runs = 0_usize;
        let mut in_plunge_run = false;
        let mut feed: Option<f64> = None;
        let mut air_feed: Option<f64> = None;
        let mut start_of_move = approach_from;
        let mut end = approach_from;
        let mut k = i;
        loop {
            match moves.get(k) {
                Some(m) if m.move_type.is_cutting() && is_entry_intent(m.intent) => {
                    let f = m.move_type.feed_rate().unwrap_or(0.0);
                    match m.intent {
                        MoveIntent::EntryApproach => {
                            air_feed = Some(air_feed.map_or(f, |a| a.min(f)));
                            in_plunge_run = false;
                        }
                        MoveIntent::EntryHelix => {
                            has_helix = true;
                            in_plunge_run = false;
                        }
                        MoveIntent::EntryRamp => {
                            has_ramp = true;
                            in_plunge_run = false;
                        }
                        MoveIntent::EntryPlunge => {
                            if !in_plunge_run {
                                plunge_runs += 1;
                            }
                            in_plunge_run = true;
                        }
                        MoveIntent::Drilling
                        | MoveIntent::ClearingCut
                        | MoveIntent::FinishingCut
                        | MoveIntent::Linking
                        | MoveIntent::Retract
                        | MoveIntent::LeadIn
                        | MoveIntent::LeadOut
                        | MoveIntent::ObstacleClearance
                        | MoveIntent::Unknown => {}
                    }
                    if m.intent != MoveIntent::EntryApproach {
                        from.get_or_insert(start_of_move);
                        feed = Some(feed.map_or(f, |a| a.min(f)));
                    }
                    start_of_move = m.target;
                    end = m.target;
                    k += 1;
                }
                // A peck retract: a rapid at the XY of the run, followed by
                // the next plunge of the ladder at the same XY.
                Some(m)
                    if m.move_type == MoveType::Rapid
                        && in_plunge_run
                        && (m.target.x - end.x).abs() < LADDER_XY_EPS_MM
                        && (m.target.y - end.y).abs() < LADDER_XY_EPS_MM
                        && moves.get(k + 1).is_some_and(|next| {
                            next.intent == MoveIntent::EntryPlunge
                                && next.move_type.is_cutting()
                                && (next.target.x - end.x).abs() < LADDER_XY_EPS_MM
                                && (next.target.y - end.y).abs() < LADDER_XY_EPS_MM
                        }) =>
                {
                    in_plunge_run = false;
                    start_of_move = m.target;
                    k += 1;
                }
                Some(_) | None => break,
            }
        }
        let kind = if has_helix {
            EntryKind::Helix
        } else if has_ramp {
            EntryKind::Ramp
        } else if plunge_runs >= 2 {
            EntryKind::PeckLadder
        } else {
            EntryKind::Plunge
        };
        records.push(EntryRecord {
            kind,
            approach_from,
            from: from.unwrap_or(approach_from),
            to: end,
            feed_mm_min: feed.or(air_feed).unwrap_or(0.0),
        });
        i = k;
    }
    records
}

/// K4: the code that the compiler refuses. Each `compile_fail` snippet has a
/// sibling that compiles, so only the named defect stops the refused one.
///
/// An entry contract has no `Default`:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::EntryContract;
/// let _ = EntryContract::default();
/// ```
///
/// The sibling: a full variant compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::EntryContract;
/// let _ = EntryContract::DescentIsCut;
/// ```
///
/// An entry set is not empty (a panic in const evaluation):
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::EntrySet;
/// const _: EntrySet = EntrySet::of(&[]);
/// ```
///
/// The sibling: a set of one kind compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::{EntryKind, EntrySet};
/// const _: EntrySet = EntrySet::of(&[EntryKind::Plunge]);
/// ```
///
/// A straight plunge states its reason:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::{EntryContract, PlungeFeedRule};
/// let _ = EntryContract::StraightPlunge { feed: PlungeFeedRule::OpPlungeRate };
/// ```
///
/// The sibling: with the reason it compiles.
///
/// ```
/// use rs_cam_core::compute::catalog::{EntryContract, PlungeFeedRule};
/// let _ = EntryContract::StraightPlunge {
///     reason: "the op starts on the line at zero depth",
///     feed: PlungeFeedRule::OpPlungeRate,
/// };
/// ```
///
/// Code outside the crate cannot build an entry record:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::{EntryKind, EntryRecord};
/// use rs_cam_core::geo::P3;
/// let p = P3::new(0.0, 0.0, 0.0);
/// let _ = EntryRecord { kind: EntryKind::Plunge, approach_from: p, from: p, to: p, feed_mm_min: 1.0 };
/// ```
///
/// The sibling: code outside the crate reads the records of a result.
///
/// ```
/// use rs_cam_core::compute::catalog::{EntryKind, EntryRecord};
/// fn kinds(records: &[EntryRecord]) -> Vec<EntryKind> {
///     records.iter().map(EntryRecord::kind).collect()
/// }
/// let _ = kinds(&[]);
/// ```
#[doc(hidden)]
pub mod entry_contract_refusals {}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    fn tp(steps: &[(MoveIntent, Option<f64>, [f64; 3])]) -> Toolpath {
        let mut t = Toolpath::new();
        for (intent, feed, [x, y, z]) in steps {
            match feed {
                Some(f) => t.feed_to_with_intent(P3::new(*x, *y, *z), *f, *intent),
                None => t.rapid_to_with_intent(P3::new(*x, *y, *z), *intent),
            }
        }
        t
    }

    #[test]
    fn an_air_lead_and_a_ramp_are_one_ramp_record() {
        let t = tp(&[
            (MoveIntent::Linking, None, [0.0, 0.0, 10.0]),
            (MoveIntent::EntryApproach, Some(250.0), [0.0, 0.0, 3.0]),
            (MoveIntent::EntryRamp, Some(400.0), [5.0, 0.0, 1.0]),
            (MoveIntent::EntryRamp, Some(400.0), [0.0, 0.0, -1.0]),
            (MoveIntent::ClearingCut, Some(1000.0), [9.0, 0.0, -1.0]),
        ]);
        let r = entry_records(&t, EntryContract::ANY_DRESSUP_FROM_ROLE);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind(), EntryKind::Ramp);
        assert_eq!(r[0].approach_from(), P3::new(0.0, 0.0, 10.0));
        assert_eq!(r[0].from(), P3::new(0.0, 0.0, 3.0));
        assert_eq!(r[0].to(), P3::new(0.0, 0.0, -1.0));
        assert!((r[0].feed_mm_min() - 400.0).abs() < 1e-12);
    }

    #[test]
    fn plunges_with_peck_retracts_are_one_ladder() {
        let t = tp(&[
            (MoveIntent::Linking, None, [2.0, 2.0, 10.0]),
            (MoveIntent::EntryPlunge, Some(300.0), [2.0, 2.0, 5.0]),
            (MoveIntent::Retract, None, [2.0, 2.0, 6.0]),
            (MoveIntent::EntryPlunge, Some(300.0), [2.0, 2.0, 3.0]),
            (MoveIntent::ClearingCut, Some(1000.0), [9.0, 2.0, 3.0]),
            (MoveIntent::Retract, None, [9.0, 2.0, 10.0]),
            (MoveIntent::EntryPlunge, Some(300.0), [9.0, 2.0, 1.0]),
        ]);
        let r = entry_records(&t, EntryContract::ANY_DRESSUP_FROM_ROLE);
        let kinds: Vec<EntryKind> = r.iter().map(EntryRecord::kind).collect();
        assert_eq!(kinds, vec![EntryKind::PeckLadder, EntryKind::Plunge]);
        assert_eq!(r[0].to(), P3::new(2.0, 2.0, 3.0));
    }

    #[test]
    fn a_descent_that_is_the_cut_has_no_record() {
        let t = tp(&[
            (MoveIntent::Linking, None, [0.0, 0.0, 10.0]),
            (MoveIntent::EntryPlunge, Some(300.0), [0.0, 0.0, 1.0]),
        ]);
        assert!(entry_records(&t, EntryContract::DescentIsCut).is_empty());
    }

    #[test]
    fn the_first_linear_feed_fraction_reads_the_first_linear_feed() {
        let t = tp(&[
            (MoveIntent::Linking, None, [0.0, 0.0, 10.0]),
            (MoveIntent::ClearingCut, Some(1000.0), [1.0, 0.0, 10.0]),
        ]);
        let half = 1000.0 * ENTRY_PLUNGE_FIRST_FEED_FRACTION.value;
        assert_eq!(
            PlungeFeedRule::first_linear_feed_fraction(&t).to_bits(),
            half.to_bits()
        );
        assert_eq!(
            PlungeFeedRule::first_linear_feed_fraction(&Toolpath::new()).to_bits(),
            ENTRY_PLUNGE_FALLBACK_MM_MIN.value.to_bits()
        );
    }

    #[test]
    fn every_kind_is_in_its_set_and_union_keeps_both() {
        for kind in EntryKind::ALL {
            assert!(EntrySet::of(&[kind]).contains(kind));
        }
        let s = EntrySet::of(&[EntryKind::Ramp]).union(EntrySet::of(&[EntryKind::Helix]));
        assert_eq!(
            s.kinds().collect::<Vec<_>>(),
            vec![EntryKind::Ramp, EntryKind::Helix]
        );
    }
}
