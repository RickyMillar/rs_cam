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
//! No new classifier: the move intents of the cut kinds
//! (`dressup::feed_modulation::intents_of`). A cutting move is a fed move
//! with an intent of `CutKind::Body`; a plunge entry one of
//! `CutKind::Entry(Plunge)`; a ramp entry one of `Entry(Ramp)` or
//! `Entry(Helix)`. A link, a lead, a retract, an approach and a drill move
//! are in no class.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use serde::Serialize;

use super::{FeedStage, ProjectSession};
use crate::limits::{CutKind, EntryKind};
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

/// The entry moves of one class, as the F words state them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EntryFeeds {
    /// The largest emitted feed of the class (mm/min).
    pub max_mm_min: f64,
    /// The stated feed of the class, as the F word states it.
    pub stated_mm_min: Option<f64>,
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
}

impl EmittedFeeds {
    const fn from_figures(figures: Figures, stage: FeedStage, feed_optimised: bool) -> Self {
        let Figures {
            stated_mm_min,
            cutting,
            plunge,
            ramp,
        } = figures;
        Self {
            stage,
            feed_optimised,
            stated_mm_min,
            cutting,
            plunge,
            ramp,
        }
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

    /// The entry clauses: one per entry class whose largest emitted feed is
    /// not the stated one. The header and the hover print them.
    #[must_use]
    pub fn entry_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (class, stated_name, entry) in [
            ("plunge entries", "plunge feed", self.plunge),
            ("ramp entries", "ramp feed", self.ramp),
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

    /// The G-code header lines of the phase, after its label: the feed
    /// stage ([`FeedStage::statement`]), the cutting line, then the entry
    /// lines. No line holds a parenthesis: a G-code comment cannot nest one.
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
        for entry in row.feeds.entry_lines() {
            report.push_str(&format!("\n    {entry}"));
        }
    }
    Some(report)
}

/// The intents of one class (the cut-kind table of the modulator).
fn in_class(intent: MoveIntent, kinds: &[CutKind]) -> bool {
    kinds
        .iter()
        .any(|kind| crate::dressup::feed_modulation::intents_of(*kind).contains(&intent))
}

fn figures_of(
    toolpath: &Toolpath,
    stated: StatedFeeds,
    post: &crate::gcode::PostDefinition,
) -> Figures {
    let stated_cut = stated.cut.map(|f| post.emitted_feed(f));
    let mut cutting: Option<CuttingFeeds> = None;
    let mut plunge: Option<f64> = None;
    let mut ramp: Option<f64> = None;
    let fold = |acc: Option<f64>, f: f64| Some(acc.map_or(f, |a| a.max(f)));
    for m in &toolpath.moves {
        let Some(feed) = m.move_type.feed_rate() else {
            continue;
        };
        if in_class(m.intent, &[CutKind::Body]) {
            let f = post.emitted_feed(feed);
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
        } else if in_class(m.intent, &[CutKind::Entry(EntryKind::Plunge)]) {
            plunge = fold(plunge, post.emitted_feed(feed));
        } else if in_class(
            m.intent,
            &[
                CutKind::Entry(EntryKind::Ramp),
                CutKind::Entry(EntryKind::Helix),
            ],
        ) {
            ramp = fold(ramp, post.emitted_feed(feed));
        }
    }
    Figures {
        stated_mm_min: stated_cut,
        cutting,
        plunge: plunge.map(|max_mm_min| EntryFeeds {
            max_mm_min,
            stated_mm_min: stated.plunge.map(|f| post.emitted_feed(f)),
        }),
        ramp: ramp.map(|max_mm_min| EntryFeeds {
            max_mm_min,
            stated_mm_min: stated.ramp.map(|f| post.emitted_feed(f)),
        }),
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
