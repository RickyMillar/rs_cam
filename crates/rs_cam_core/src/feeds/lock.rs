//! W3k: the feeds lock — which of the feed, the spindle speed and the
//! chipload holds when the operator edits one of the other two.
//!
//! The three are one relation, `feed = chipload × RPM × flutes`
//! ([`AdvancePerToothMm`]). An edit of one of them must move one other, and
//! the lock names the one that does not move. Operator, 2026-10-31: "you
//! can lock any one of them at a time. by default it's chipload."
//!
//! The lock is the operator's choice, per toolpath, in the project file
//! (`ToolpathConfig::feeds_lock`). It moves no motion: it decides only what
//! the GUI writes for the next edit. A raw parameter write (MCP
//! `set_toolpath_param`, the CLI) does no lock arithmetic.

use serde::{Deserialize, Serialize};

use super::{AdvancePerToothMm, CommandedFeedMmMin};

/// The field that holds when the operator edits another of the three.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedsLock {
    /// The feed holds: a spindle edit moves the chipload, a chipload edit
    /// moves the spindle speed.
    Feed,
    /// The spindle speed holds: a feed edit moves the chipload, a chipload
    /// edit moves the feed.
    Spindle,
    /// The chipload holds: a feed edit moves the spindle speed, a spindle
    /// edit moves the feed.
    #[default]
    Chipload,
}

impl FeedsLock {
    /// Every lock, in row order (Feed, Spindle, Chipload).
    pub const ALL: [Self; 3] = [Self::Feed, Self::Spindle, Self::Chipload];

    /// True for the default (Chipload). The project file writes no key
    /// for it.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The key of the lock in the project file and in MCP.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Feed => "feed",
            Self::Spindle => "spindle",
            Self::Chipload => "chipload",
        }
    }

    /// The write of one edit under this lock: the edited field and the one
    /// field that moves with it.
    ///
    /// `feed` and `rpm` are the values before the edit. An edit of the
    /// LOCKED field itself moves the field and holds the third as an edit
    /// with no lock does: a feed or spindle edit moves the chipload, a
    /// chipload edit moves the feed (the W3i rule).
    ///
    /// The dependent field is not written when its inputs are absent: with
    /// no feed or no spindle speed there is no chipload to hold, so the
    /// edit writes its own field only.
    #[must_use]
    pub fn resolve(
        self,
        edit: SpeedEdit,
        feed: Option<f64>,
        rpm: Option<f64>,
        flutes: u32,
    ) -> SpeedWrite {
        let chipload = || {
            let (feed, rpm) = (feed?, rpm?);
            AdvancePerToothMm::from_commanded_at(CommandedFeedMmMin::new(feed), rpm, flutes)
        };
        match (edit, self) {
            // The chipload holds: the other speed follows the edit.
            (SpeedEdit::Feed(f), Self::Chipload) => SpeedWrite {
                feed: Some(f),
                rpm: chipload().and_then(|c| c.rpm_for(CommandedFeedMmMin::new(f), flutes)),
            },
            (SpeedEdit::Spindle(r), Self::Chipload) => SpeedWrite {
                feed: chipload().map(|c| c.feed_at(r, flutes).mm_min()),
                rpm: Some(r),
            },
            // A feed or spindle edit with the other speed (or itself) locked
            // writes itself; the chipload readout moves.
            (SpeedEdit::Feed(f), Self::Spindle | Self::Feed) => SpeedWrite {
                feed: Some(f),
                rpm: None,
            },
            (SpeedEdit::Spindle(r), Self::Feed | Self::Spindle) => SpeedWrite {
                feed: None,
                rpm: Some(r),
            },
            // The feed holds: the spindle speed follows the chipload. With
            // no feed to hold, the feed follows the chipload instead.
            (SpeedEdit::Chipload(c), Self::Feed) if feed.is_some() => SpeedWrite {
                feed: None,
                rpm: feed.and_then(|f| {
                    AdvancePerToothMm::new(c).rpm_for(CommandedFeedMmMin::new(f), flutes)
                }),
            },
            // The spindle speed holds (or the chipload itself is locked):
            // the feed follows the chipload.
            (SpeedEdit::Chipload(c), _) => SpeedWrite {
                feed: rpm.map(|r| AdvancePerToothMm::new(c).feed_at(r, flutes).mm_min()),
                rpm: None,
            },
        }
    }
}

impl FeedsLock {
    /// The range an edit of `field` may take under this lock, so that the
    /// field that moves with it stays inside its own range: the feed range
    /// `ranges.feed` (mm/min) and the spindle range `ranges.rpm`.
    ///
    /// `None` when the edit moves no other field (then the field's own
    /// range applies), or when the relation has no value (no feed, no
    /// spindle speed, no flutes). A narrowed range that is empty is `None`
    /// too.
    #[must_use]
    pub fn edit_range(
        self,
        field: SpeedField,
        ranges: SpeedRanges,
        feed: Option<f64>,
        rpm: Option<f64>,
        flutes: u32,
    ) -> Option<(f64, f64)> {
        let fl = f64::from(flutes);
        let chipload = feed
            .zip(rpm)
            .filter(|(_, r)| *r > 0.0 && fl > 0.0)
            .map(|(f, r)| f / (r * fl));
        let (flo, fhi) = ranges.feed;
        let (rlo, rhi) = ranges.rpm;
        let range = match (field, self) {
            (SpeedField::Feed, Self::Chipload) => {
                let cl = chipload?;
                (flo.max(rlo * cl * fl), fhi.min(rhi * cl * fl))
            }
            (SpeedField::Spindle, Self::Chipload) => {
                let cl = chipload.filter(|c| *c > 0.0)?;
                (rlo.max(flo / (cl * fl)), rhi.min(fhi / (cl * fl)))
            }
            (SpeedField::Feed | SpeedField::Spindle, _) => return None,
            (SpeedField::Chipload, Self::Feed) if feed.is_some() => {
                let f = feed?;
                (f / (rhi * fl), f / (rlo * fl))
            }
            (SpeedField::Chipload, _) => {
                let r = rpm.filter(|r| *r > 0.0)?;
                (flo / (r * fl), fhi / (r * fl))
            }
        };
        (range.0.is_finite() && range.1.is_finite() && range.0 <= range.1 && fl > 0.0)
            .then_some(range)
    }
}

/// One of the three speed rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedField {
    /// The feed row.
    Feed,
    /// The spindle row.
    Spindle,
    /// The chipload row.
    Chipload,
}

/// The ranges of the feed and the spindle rows, each `(min, max)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedRanges {
    /// The feed range, mm/min.
    pub feed: (f64, f64),
    /// The spindle range, RPM.
    pub rpm: (f64, f64),
}

/// One operator edit of a speed row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedEdit {
    /// A feed, mm/min.
    Feed(f64),
    /// A spindle speed, RPM.
    Spindle(f64),
    /// A chipload (advance per tooth), mm.
    Chipload(f64),
}

/// The values one edit writes. `None` leaves the field as it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedWrite {
    /// The feed to write, mm/min.
    pub feed: Option<f64>,
    /// The spindle speed to write, RPM (not rounded).
    pub rpm: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::{FeedsLock, SpeedEdit, SpeedWrite};

    const FLUTES: u32 = 2;
    /// Feed 1000 mm/min at 10 000 RPM: chipload 0.05 mm.
    const FEED: Option<f64> = Some(1000.0);
    const RPM: Option<f64> = Some(10_000.0);

    fn write(lock: FeedsLock, edit: SpeedEdit) -> SpeedWrite {
        lock.resolve(edit, FEED, RPM, FLUTES)
    }

    #[test]
    fn the_default_lock_is_the_chipload_w3k() {
        assert_eq!(FeedsLock::default(), FeedsLock::Chipload);
        assert!(FeedsLock::Chipload.is_default());
    }

    #[test]
    fn a_chipload_lock_moves_the_other_speed_w3k() {
        let w = write(FeedsLock::Chipload, SpeedEdit::Feed(2000.0));
        assert_eq!(w.feed, Some(2000.0));
        assert_eq!(w.rpm, Some(20_000.0), "rpm = feed / (0.05 x 2)");
        let w = write(FeedsLock::Chipload, SpeedEdit::Spindle(5000.0));
        assert_eq!(w.rpm, Some(5000.0));
        assert_eq!(w.feed, Some(500.0), "feed = 0.05 x 5000 x 2");
    }

    #[test]
    fn a_feed_lock_holds_the_feed_w3k() {
        let w = write(FeedsLock::Feed, SpeedEdit::Spindle(20_000.0));
        assert_eq!(
            w,
            SpeedWrite {
                feed: None,
                rpm: Some(20_000.0)
            }
        );
        let w = write(FeedsLock::Feed, SpeedEdit::Chipload(0.1));
        assert_eq!(w.feed, None);
        assert_eq!(w.rpm, Some(5000.0), "rpm = 1000 / (0.1 x 2)");
    }

    #[test]
    fn a_spindle_lock_holds_the_spindle_speed_w3k() {
        let w = write(FeedsLock::Spindle, SpeedEdit::Feed(1500.0));
        assert_eq!(
            w,
            SpeedWrite {
                feed: Some(1500.0),
                rpm: None
            }
        );
        let w = write(FeedsLock::Spindle, SpeedEdit::Chipload(0.1));
        assert_eq!(
            w,
            SpeedWrite {
                feed: Some(2000.0),
                rpm: None
            }
        );
    }

    #[test]
    fn an_edit_range_keeps_the_dependent_field_in_its_range_w3k() {
        use super::{SpeedField, SpeedRanges};
        let ranges = SpeedRanges {
            feed: (1.0, 4000.0),
            rpm: (1000.0, 24_000.0),
        };
        // Chipload 0.05, 2 flutes: rpm = feed / 0.1, so the feed stops at
        // 2400 (24 000 RPM) and starts at 100 (1000 RPM).
        let r = FeedsLock::Chipload.edit_range(SpeedField::Feed, ranges, FEED, RPM, FLUTES);
        assert_eq!(r, Some((100.0, 2400.0)));
        // feed = rpm x 0.1: the spindle stops at 40 000 > 24 000, so 24 000.
        let r = FeedsLock::Chipload.edit_range(SpeedField::Spindle, ranges, FEED, RPM, FLUTES);
        assert_eq!(r, Some((1000.0, 24_000.0)));
        // Feed 1000 holds: chipload = 1000 / (rpm x 2).
        let r = FeedsLock::Feed.edit_range(SpeedField::Chipload, ranges, FEED, RPM, FLUTES);
        assert_eq!(r, Some((1000.0 / 48_000.0, 0.5)));
        assert_eq!(
            FeedsLock::Spindle.edit_range(SpeedField::Feed, ranges, FEED, RPM, FLUTES),
            None
        );
    }

    #[test]
    fn with_no_chipload_an_edit_writes_its_own_field_w3k() {
        let w = FeedsLock::Chipload.resolve(SpeedEdit::Feed(2000.0), FEED, None, FLUTES);
        assert_eq!(
            w,
            SpeedWrite {
                feed: Some(2000.0),
                rpm: None
            }
        );
    }
}
