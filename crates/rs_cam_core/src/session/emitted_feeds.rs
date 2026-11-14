//! Feeds contract P30-1 ("print what runs"): THE record of the feeds that
//! an export of one toolpath emits now.
//!
//! After a simulation with feed modulation, and after the feed optimisation
//! dressup, the F words of a toolpath can differ from the stated feed of its
//! op. [`EmittedFeeds`] states them: the feed stage of the result, the
//! stated cutting feed, the smallest and the largest emitted cutting feed,
//! the count of cutting moves above the stated feed, and the largest entry
//! feed against its stated feed.
//!
//! # One door
//!
//! [`ProjectSession::emitted_feeds`] reads the moves of the result slot:
//! the moves that every export door gives the program builder (the export
//! datum shift moves X and Y only). The program builder writes the header of
//! a phase from this record (`GcodePhase::feeds`), so the header and the F
//! words come from one slot. The GUI, MCP and the CLI read the same door.
//!
//! # The numbers are the F words
//!
//! Each feed goes through the post: the clamp at `limits.max_feed`, then the
//! round at `decimals.feed` ([`crate::gcode::PostDefinition::emitted_feed`]).
//! The stated feed goes through the same function, and a move is above the
//! stated feed when its F word is larger.
//!
//! # The classes
//!
//! Review 2026-11-15 F8: every fed move is in exactly one class,
//! [`FeedClass::of`] its intent (one match that names every intent, no
//! wildcard). The classes follow the move intents of the cut kinds
//! (`dressup::feed_modulation::intents_of`; the lib test
//! `every_feed_class_is_the_class_of_its_cut_kinds_review_f8` holds the two
//! against each other): a cutting move has an intent of `CutKind::Body`; a
//! plunge entry one of `CutKind::Entry(Plunge)`; a ramp entry one of
//! `Entry(Ramp)` or `Entry(Helix)`; a drill move one of `CutKind::Drill`; an
//! air lead one of `CutKind::EntryApproach`; a lead one of `LeadIn` or
//! `LeadOut`; a link one of `CutKind::Link` (a link, a retract, an obstacle
//! clearance).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use serde::Serialize;

use super::{FeedStage, ProjectSession};
use crate::toolpath::{MoveIntent, Toolpath};
use crate::trace::toolpath_spans::AnnotatedToolpath;

/// The stated feeds of an op, in mm/min, before the post.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatedFeeds {
    /// The cutting feed (`OperationConfig::feed_rate`).
    pub cut: Option<f64>,
    /// The plunge feed (`OperationConfig::plunge_rate`).
    pub plunge: Option<f64>,
    /// The ramp and helix feed (`OperationConfig::entry_feed_rate`, else
    /// the plunge feed: the rule of the entry emitters).
    pub ramp: Option<f64>,
}

impl StatedFeeds {
    /// The stated feeds of `operation`.
    #[must_use]
    pub fn of(operation: &crate::compute::catalog::OperationConfig) -> Self {
        let plunge = operation.plunge_rate();
        Self {
            cut: operation.feed_rate(),
            plunge,
            ramp: operation.entry_feed_rate().or(plunge),
        }
    }
}

/// The cutting moves of a toolpath, as the F words state them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CuttingFeeds {
    /// The smallest emitted cutting feed (mm/min).
    pub min_mm_min: f64,
    /// The largest emitted cutting feed (mm/min).
    pub max_mm_min: f64,
    /// The count of fed cutting moves.
    pub moves: usize,
    /// The count of fed cutting moves whose F word is larger than the F
    /// word of the stated feed. Zero with no stated feed.
    pub above_stated: usize,
}

/// The fed moves of one class other than the cutting moves, as the F words
/// state them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EntryFeeds {
    /// The largest emitted feed of the class (mm/min).
    pub max_mm_min: f64,
    /// The stated feed of the class, as the F word states it; `None` for a
    /// class with no stated feed of its own (a lead, a link).
    pub stated_mm_min: Option<f64>,
    /// The count of fed moves of the class (review 2026-11-15 F8).
    pub moves: usize,
}

/// Review 2026-11-15 F8: the class of a fed move. Every fed move is in
/// exactly one class: [`Self::of`] names every move intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedClass {
    /// A cut of the body (`CutKind::Body` intents).
    Cutting,
    /// A plunge entry.
    PlungeEntry,
    /// A ramp or helix entry.
    RampEntry,
    /// A drill move.
    Drill,
    /// The air lead above an entry (`MoveIntent::EntryApproach`, K4).
    Approach,
    /// A lead into or out of the cut.
    Lead,
    /// A link, a retract or an obstacle clearance.
    Link,
}

impl FeedClass {
    /// Every class.
    pub const ALL: [Self; 7] = [
        Self::Cutting,
        Self::PlungeEntry,
        Self::RampEntry,
        Self::Drill,
        Self::Approach,
        Self::Lead,
        Self::Link,
    ];

    /// The class of a fed move with `intent`.
    #[must_use]
    pub const fn of(intent: MoveIntent) -> Self {
        match intent {
            MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Unknown => {
                Self::Cutting
            }
            MoveIntent::EntryPlunge => Self::PlungeEntry,
            MoveIntent::EntryRamp | MoveIntent::EntryHelix => Self::RampEntry,
            MoveIntent::Drilling => Self::Drill,
            MoveIntent::EntryApproach => Self::Approach,
            MoveIntent::LeadIn | MoveIntent::LeadOut => Self::Lead,
            MoveIntent::Linking | MoveIntent::Retract | MoveIntent::ObstacleClearance => Self::Link,
        }
    }
}

impl EntryFeeds {
    /// `true` when the largest emitted feed is not the stated feed.
    #[must_use]
    pub fn differs(&self) -> bool {
        self.stated_mm_min != Some(self.max_mm_min)
    }
}

/// The move figures of a record: the part read from the moves.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Figures {
    stated_mm_min: Option<f64>,
    cutting: Option<CuttingFeeds>,
    plunge: Option<EntryFeeds>,
    ramp: Option<EntryFeeds>,
    drill: Option<EntryFeeds>,
    approach: Option<EntryFeeds>,
    lead: Option<EntryFeeds>,
    link: Option<EntryFeeds>,
}

/// **P30-1: THE record of the feeds that an export of one toolpath emits
/// now.** Built only by [`ProjectSession::emitted_feeds`] (or
/// [`ProjectSession::emitted_feeds_under`] for the CLI job file, which
/// emits with its own post). Every surface prints it through its
/// text methods: the G-code header ([`Self::header_lines`]), the GUI and
/// the CLI ([`Self::line`]); MCP serialises it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmittedFeeds {
    /// The feed stage of the result.
    pub stage: FeedStage,
    /// `true` when the feed optimisation dressup ran on the result: the
    /// option is on and `feed_optimization_unavailable_reason` gives no
    /// reason (the predicate of the step).
    pub feed_optimised: bool,
    /// The stated cutting feed of the op, as the F word states it.
    pub stated_mm_min: Option<f64>,
    /// The cutting moves; `None` when the toolpath has no fed cutting move.
    pub cutting: Option<CuttingFeeds>,
    /// The plunge entries; `None` with no plunge entry.
    pub plunge: Option<EntryFeeds>,
    /// The ramp and helix entries; `None` with none.
    pub ramp: Option<EntryFeeds>,
    /// Review 2026-11-15 F8: the drill moves (against the stated cutting
    /// feed, the feed of a drill cycle); `None` with none.
    pub drill: Option<EntryFeeds>,
    /// The air leads above an entry (against the stated plunge feed, the
    /// rule of the entry emitters, P29-8 8c); `None` with none.
    pub approach: Option<EntryFeeds>,
    /// The leads into and out of the cut; `None` with none.
    pub lead: Option<EntryFeeds>,
    /// The fed links, retracts and obstacle clearances; `None` with none.
    pub link: Option<EntryFeeds>,
}

impl EmittedFeeds {
    const fn from_figures(figures: Figures, stage: FeedStage, feed_optimised: bool) -> Self {
        let Figures {
            stated_mm_min,
            cutting,
            plunge,
            ramp,
            drill,
            approach,
            lead,
            link,
        } = figures;
        Self {
            stage,
            feed_optimised,
            stated_mm_min,
            cutting,
            plunge,
            ramp,
            drill,
            approach,
            lead,
            link,
        }
    }

    /// Review 2026-11-15 F8: the count of fed moves in each class, summed.
    /// It equals the count of fed moves of the toolpath: every fed move is
    /// in one class.
    #[must_use]
    pub fn fed_moves(&self) -> usize {
        self.cutting.map_or(0, |c| c.moves)
            + [
                self.plunge,
                self.ramp,
                self.drill,
                self.approach,
                self.lead,
                self.link,
            ]
            .iter()
            .flatten()
            .map(|e| e.moves)
            .sum::<usize>()
    }

    /// `true` when a cutting move runs above the stated feed. The GUI
    /// colours the line with the caution colour only then.
    #[must_use]
    pub fn runs_above_stated(&self) -> bool {
        self.cutting.is_some_and(|c| c.above_stated > 0)
    }

    /// The stage in words: "as stated", "feed optimisation", "modulated, run
    /// n", "modulated, run n, not current". Short, so the GUI line keeps it
    /// at the rail width; the hover holds the full stage statement.
    #[must_use]
    pub fn stage_words(&self) -> String {
        match self.stage {
            FeedStage::Planned if self.feed_optimised => "feed optimisation".to_owned(),
            FeedStage::Planned => "as stated".to_owned(),
            FeedStage::ModulatedCurrent { simulation_run } => {
                format!("modulated, run {simulation_run}")
            }
            FeedStage::ModulatedSuperseded { simulation_run, .. } => {
                format!("modulated, run {simulation_run}, not current")
            }
        }
    }

    /// The range of the cutting feeds in words: "1000 mm/min" or "943 to
    /// 3000 mm/min".
    fn range_words(cutting: &CuttingFeeds) -> String {
        if cutting.min_mm_min == cutting.max_mm_min {
            format!("{} mm/min", cutting.max_mm_min)
        } else {
            format!("{} to {} mm/min", cutting.min_mm_min, cutting.max_mm_min)
        }
    }

    /// The numbers in words: "runs 943 to 3000 mm/min · 32 of 40 above
    /// 1000", or "no cutting move". The GUI prints it as the first line (the
    /// rail shows about 55 characters, so the stage takes its own line).
    #[must_use]
    pub fn numbers_line(&self) -> String {
        let Some(cutting) = self.cutting.as_ref() else {
            return "no cutting move".to_owned();
        };
        let range = Self::range_words(cutting);
        match self.stated_mm_min {
            Some(stated) => format!(
                "runs {range} · {} of {} above {stated}",
                cutting.above_stated, cutting.moves
            ),
            None => format!("runs {range} · no stated feed"),
        }
    }

    /// `true` when the stage is the stated feed with no feed stage after
    /// it (no feed optimisation, no modulation): the GUI then prints the
    /// stage on the same line, else on its own line.
    #[must_use]
    pub const fn is_as_stated(&self) -> bool {
        matches!(self.stage, FeedStage::Planned) && !self.feed_optimised
    }

    /// One line for the CLI and MCP: [`Self::numbers_line`], then the stage
    /// ("runs 943 to 3000 mm/min · 32 of 40 above 1000 · modulated, run 2").
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} · {}", self.numbers_line(), self.stage_words())
    }

    /// The entry clauses: one per entry class (the plunge and ramp entries
    /// and, review 2026-11-15 F8, the air leads) whose largest emitted feed
    /// is not the stated one. The header and the hover print them.
    #[must_use]
    pub fn entry_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (class, stated_name, entry) in [
            ("plunge entries", "plunge feed", self.plunge),
            ("ramp entries", "ramp feed", self.ramp),
            ("air leads", "plunge feed", self.approach),
        ] {
            let Some(entry) = entry.filter(EntryFeeds::differs) else {
                continue;
            };
            out.push(match entry.stated_mm_min {
                Some(stated) => format!(
                    "{class} run up to {} mm/min, the stated {stated_name} is {stated} mm/min",
                    entry.max_mm_min
                ),
                None => format!(
                    "{class} run up to {} mm/min, no stated {stated_name}",
                    entry.max_mm_min
                ),
            });
        }
        out
    }

    /// Review 2026-11-15 F8: the drill line, for a toolpath with drill
    /// moves: the largest emitted drill feed against the stated feed.
    #[must_use]
    pub fn drill_line(&self) -> Option<String> {
        let drill = self.drill?;
        Some(match drill.stated_mm_min {
            Some(stated) if stated == drill.max_mm_min => {
                format!("drill feeds run {stated} mm/min, the stated feed")
            }
            Some(stated) => format!(
                "drill feeds run up to {} mm/min, the stated feed is {stated} mm/min",
                drill.max_mm_min
            ),
            None => format!(
                "drill feeds run up to {} mm/min, the op states no feed",
                drill.max_mm_min
            ),
        })
    }

    /// The G-code header lines of the phase, after its label: the feed
    /// stage ([`FeedStage::statement`]), the cutting line, the drill line
    /// (review 2026-11-15 F8: a drill phase prints its feeds), then the
    /// entry lines. No line holds a parenthesis: a G-code comment cannot
    /// nest one.
    #[must_use]
    pub fn header_lines(&self) -> Vec<String> {
        let mut out = vec![self.stage.statement()];
        if let Some(cutting) = self.cutting.as_ref() {
            let range = Self::range_words(cutting);
            let mut line = match self.stated_mm_min {
                Some(stated) => format!(
                    "feeds run {range}, {} of {} cutting moves above the stated {stated} mm/min",
                    cutting.above_stated, cutting.moves
                ),
                None => format!("feeds run {range}, the op states no cutting feed"),
            };
            if self.feed_optimised && matches!(self.stage, FeedStage::Planned) {
                line.push_str(", set by feed optimisation");
            }
            out.push(line);
        }
        out.extend(self.drill_line());
        out.extend(self.entry_lines());
        out
    }

    /// The hover of the GUI line: what the counts mean, the stage
    /// statement and the entry lines.
    #[must_use]
    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if let (Some(cutting), Some(stated)) = (self.cutting.as_ref(), self.stated_mm_min) {
            parts.push(format!(
                "{} of {} cutting moves run above the stated feed of {stated} mm/min",
                cutting.above_stated, cutting.moves
            ));
        }
        parts.push(self.stage.statement());
        parts.extend(self.drill_line());
        parts.extend(self.entry_lines());
        parts.join("\n")
    }
}

/// The record of one emitted toolpath with its name: a row of
/// `ExportedGcode::emitted_feeds`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NamedEmittedFeeds {
    /// The toolpath name.
    pub toolpath: String,
    /// Its record.
    pub feeds: EmittedFeeds,
}

/// The text that an export reply (MCP `export_gcode`, the CLI) prints for
/// the records of the emitted toolpaths: one line per toolpath. `None` with
/// no record.
#[must_use]
pub fn emitted_feeds_report(rows: &[NamedEmittedFeeds]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let mut report = "Feeds that run:".to_owned();
    for row in rows {
        report.push_str(&format!("\n  '{}': {}", row.toolpath, row.feeds.line()));
        for entry in row
            .feeds
            .drill_line()
            .into_iter()
            .chain(row.feeds.entry_lines())
        {
            report.push_str(&format!("\n    {entry}"));
        }
    }
    Some(report)
}

fn figures_of(
    toolpath: &Toolpath,
    stated: StatedFeeds,
    post: &crate::gcode::PostDefinition,
) -> Figures {
    let stated_cut = stated.cut.map(|f| post.emitted_feed(f));
    let stated_plunge = stated.plunge.map(|f| post.emitted_feed(f));
    let mut cutting: Option<CuttingFeeds> = None;
    let mut plunge: Option<(f64, usize)> = None;
    let mut ramp: Option<(f64, usize)> = None;
    let mut drill: Option<(f64, usize)> = None;
    let mut approach: Option<(f64, usize)> = None;
    let mut lead: Option<(f64, usize)> = None;
    let mut link: Option<(f64, usize)> = None;
    let fold = |acc: &mut Option<(f64, usize)>, f: f64| {
        *acc = Some(acc.map_or((f, 1), |(a, n)| (a.max(f), n + 1)));
    };
    for m in &toolpath.moves {
        let Some(feed) = m.move_type.feed_rate() else {
            continue;
        };
        let f = post.emitted_feed(feed);
        match FeedClass::of(m.intent) {
            FeedClass::Cutting => {
                let above = usize::from(stated_cut.is_some_and(|s| f > s));
                cutting = Some(match cutting {
                    None => CuttingFeeds {
                        min_mm_min: f,
                        max_mm_min: f,
                        moves: 1,
                        above_stated: above,
                    },
                    Some(c) => CuttingFeeds {
                        min_mm_min: c.min_mm_min.min(f),
                        max_mm_min: c.max_mm_min.max(f),
                        moves: c.moves + 1,
                        above_stated: c.above_stated + above,
                    },
                });
            }
            FeedClass::PlungeEntry => fold(&mut plunge, f),
            FeedClass::RampEntry => fold(&mut ramp, f),
            FeedClass::Drill => fold(&mut drill, f),
            FeedClass::Approach => fold(&mut approach, f),
            FeedClass::Lead => fold(&mut lead, f),
            FeedClass::Link => fold(&mut link, f),
        }
    }
    let class = |acc: Option<(f64, usize)>, stated_mm_min: Option<f64>| {
        acc.map(|(max_mm_min, moves)| EntryFeeds {
            max_mm_min,
            stated_mm_min,
            moves,
        })
    };
    Figures {
        stated_mm_min: stated_cut,
        cutting,
        plunge: class(plunge, stated_plunge),
        ramp: class(ramp, stated.ramp.map(|f| post.emitted_feed(f))),
        drill: class(drill, stated_cut),
        approach: class(approach, stated_plunge),
        lead: class(lead, None),
        link: class(link, None),
    }
}

/// The key of one memo entry: the inputs of [`Figures`] other than the
/// moves (the moves are keyed by the identity of their `Arc`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct FiguresKey {
    stated: StatedFeeds,
    feed_decimals: usize,
    max_feed: Option<f64>,
}

struct MemoEntry {
    moves: Weak<AnnotatedToolpath>,
    key: FiguresKey,
    figures: Figures,
}

/// The session memo of the move figures of each toolpath. A frame of the
/// GUI reads the record of the selected toolpath; the memo keeps the walk
/// over the moves to one per result. A clone of the session starts empty.
#[derive(Default)]
pub(crate) struct EmittedFeedsMemo {
    entries: Mutex<HashMap<usize, MemoEntry>>,
}

impl Clone for EmittedFeedsMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ProjectSession {
    /// The feed optimisation dressup ran on the result of `tc`: the option
    /// is on and the step gives no reason to skip
    /// (`feed_optimization_unavailable_reason`, the predicate of the step).
    fn feed_optimisation_ran(tc: &super::ToolpathConfig) -> bool {
        tc.dressups.feed_optimization
            && crate::compute::catalog::feed_optimization_unavailable_reason(
                &tc.operation,
                tc.stock_source,
            )
            .is_none()
    }

    /// **P30-1: THE door to the record of the feeds that an export of the
    /// toolpath at `index` emits now**, under the post of the session.
    /// `None` when the toolpath has no result.
    #[must_use]
    pub fn emitted_feeds(&self, index: usize) -> Option<EmittedFeeds> {
        let post = self.post.format.definition();
        self.emitted_feeds_under(index, post)
    }

    /// [`Self::emitted_feeds`] under `post`: the CLI job file emits with
    /// its own post.
    #[must_use]
    pub fn emitted_feeds_under(
        &self,
        index: usize,
        post: &crate::gcode::PostDefinition,
    ) -> Option<EmittedFeeds> {
        let tc = self.toolpath_configs.get(index)?;
        let result = self.results.get(&index)?;
        let stage = self.feed_stage_of(index, &result.provenance);
        let key = FiguresKey {
            stated: StatedFeeds::of(&tc.operation),
            feed_decimals: post.decimals.feed,
            max_feed: post.limits.max_feed.map(crate::gcode::Feedrate::get),
        };
        let moves = result.annotated();
        let mut guard = match self.emitted_feeds_memo.entries.lock() {
            Ok(guard) => guard,
            // An entry is written only after its build returns, so a
            // poisoned map still holds whole entries.
            Err(poisoned) => poisoned.into_inner(),
        };
        let hit = guard.get(&index).and_then(|entry| {
            let same = entry.key == key
                && entry
                    .moves
                    .upgrade()
                    .is_some_and(|held| Arc::ptr_eq(&held, moves));
            same.then_some(entry.figures)
        });
        let figures = match hit {
            Some(figures) => figures,
            None => {
                let figures = figures_of(&moves.toolpath, key.stated, post);
                let _ = guard.insert(
                    index,
                    MemoEntry {
                        moves: Arc::downgrade(moves),
                        key,
                        figures,
                    },
                );
                figures
            }
        };
        Some(EmittedFeeds::from_figures(
            figures,
            stage,
            Self::feed_optimisation_ran(tc),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::FeedClass;
    use crate::dressup::feed_modulation::intents_of;
    use crate::limits::{CutKind, EntryKind};
    use crate::toolpath::MoveIntent;

    /// Review 2026-11-15 F8: the class of each intent of a cut kind is the
    /// class of that kind (a literal table), so the record and the
    /// modulator read one cut-kind table.
    #[test]
    fn every_feed_class_is_the_class_of_its_cut_kinds_review_f8() {
        let table: [(CutKind, FeedClass); 12] = [
            (CutKind::Body, FeedClass::Cutting),
            (CutKind::SpringPass, FeedClass::Cutting),
            (CutKind::WallFinish, FeedClass::Cutting),
            (CutKind::SlotRing, FeedClass::Cutting),
            (CutKind::DressupRelief, FeedClass::Cutting),
            (CutKind::Entry(EntryKind::Plunge), FeedClass::PlungeEntry),
            (CutKind::Entry(EntryKind::Ramp), FeedClass::RampEntry),
            (CutKind::Entry(EntryKind::Helix), FeedClass::RampEntry),
            (CutKind::EntryApproach, FeedClass::Approach),
            (CutKind::LeadIn, FeedClass::Lead),
            (CutKind::Link, FeedClass::Link),
            (CutKind::Drill, FeedClass::Drill),
        ];
        for (kind, class) in table {
            let intents = intents_of(kind);
            assert!(!intents.is_empty(), "{kind:?}: non-vacuity");
            for intent in intents {
                assert_eq!(FeedClass::of(*intent), class, "{kind:?} {intent:?}");
            }
        }
        assert_eq!(FeedClass::of(MoveIntent::LeadOut), FeedClass::Lead);
    }
}
