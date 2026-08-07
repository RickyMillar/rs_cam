//! Source freshness + citation audit for the literature matrix.
//!
//! Phase 5 deliverable per
//! `planning/feeds_literature_matrix_2026-06-03.md` §"Phase plan / Risk
//! register": keep the matrix useful past year 1 by surfacing when
//! `last_verified` on a source is drifting toward stale, and by
//! verifying that every source key a cell cites actually exists in
//! `sources.toml`.
//!
//! Two independent clocks per source (P2, W6 audit §8.3):
//!   - `last_verified` — when the row's NUMBERS were last read out of
//!     the document.
//!   - `url_verified`  — when the row's LINK was last confirmed to
//!     resolve to a document.
//!
//! They answer different questions and rot independently: a chart can
//! move to a new URL without its contents changing, and a stable URL
//! can be re-published with new numbers. `url_verified` absent means the
//! link has never been confirmed, and is reported as such rather than
//! being backfilled from `last_verified`.
//!
//! Knobs (env vars):
//!   - `LIT_MATRIX_TODAY="YYYY-MM-DD"` — override "today" for the
//!     freshness clock. Set it to pin a report for a reproducible
//!     comparison; unset, the clock reads the real system date (P4).
//!   - `LIT_MATRIX_DECAY_FAIL=1` — promote stale (>18 months) sources
//!     from a warning to a hard failure. Default is warn-only.
//!
//! Categories:
//!   - fresh: age < 12 months
//!   - warn : 12 ≤ age < 18 months
//!   - stale: age ≥ 18 months
//!
//! Citation audit: any source key referenced by a cell band /
//! invariant / anti-pattern that does NOT appear in `sources.toml` is
//! a schema error and always fails the matrix run (regardless of
//! `LIT_MATRIX_DECAY_FAIL`).

use super::cell::CellsFile;
use std::collections::{BTreeMap, BTreeSet};

/// Fallback used only when the system clock cannot be read at all
/// (`SystemTime::now()` before the Unix epoch). Not a default in the
/// normal sense — see [`today_str`].
const CLOCK_UNAVAILABLE_FALLBACK: &str = "2026-06-03";
const FRESH_MONTHS: i32 = 12;
const STALE_MONTHS: i32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessBucket {
    Fresh,
    Warn,
    Stale,
}

impl FreshnessBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            FreshnessBucket::Fresh => "fresh",
            FreshnessBucket::Warn => "warn",
            FreshnessBucket::Stale => "stale",
        }
    }
}

#[derive(Debug)]
pub struct SourceRow {
    pub key: String,
    pub last_verified: Option<String>,
    pub age_months: Option<i32>,
    pub bucket: FreshnessBucket,
    /// P2 (W6 audit §8.3): the date the row's `citation_url` was last
    /// confirmed to resolve to a document, separate from the date its
    /// *numbers* were last read out of that document.
    ///
    /// `None` means the link has **never** been confirmed — which is
    /// the honest state for the rows the census found dead (hard 404 /
    /// retired host) and for the rows it could not determine (403
    /// bot-block, unreachable from the sandbox). It is deliberately not
    /// backfilled with `last_verified`: that would assert a check
    /// nobody performed.
    pub url_verified: Option<String>,
    pub url_age_months: Option<i32>,
    pub url_bucket: Option<FreshnessBucket>,
    pub note: String,
}

#[derive(Debug)]
pub struct FreshnessReport {
    pub today: String,
    pub rows: Vec<SourceRow>,
}

impl FreshnessReport {
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut f = 0;
        let mut w = 0;
        let mut s = 0;
        for r in &self.rows {
            match r.bucket {
                FreshnessBucket::Fresh => f += 1,
                FreshnessBucket::Warn => w += 1,
                FreshnessBucket::Stale => s += 1,
            }
        }
        (f, w, s)
    }

    /// Rows whose `citation_url` has never been confirmed to resolve.
    /// Report-only: a link that was never checked is a maintenance
    /// signal, not a failure — the hard failure is a *malformed* URL
    /// (`audit_citation_urls`, P1).
    pub fn url_unverified_keys(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.url_verified.is_none())
            .map(|r| r.key.clone())
            .collect()
    }

    pub fn stale_keys(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.bucket == FreshnessBucket::Stale)
            .map(|r| r.key.clone())
            .collect()
    }
}

/// Parse a YYYY-MM-DD string into (year, month, day). Returns None on
/// malformed input. Deliberately *not* using chrono — the matrix has a
/// zero-new-deps policy.
fn parse_ymd(s: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: i32 = parts[1].parse().ok()?;
    let d: i32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

/// Whole-month age between two YYYY-MM-DDs. Day-of-month is taken
/// into account: if `today.day < verified.day` we subtract one
/// month. Returns 0 for future dates (never negative).
fn months_between(verified: (i32, i32, i32), today: (i32, i32, i32)) -> i32 {
    let mut months = (today.0 - verified.0) * 12 + (today.1 - verified.1);
    if today.2 < verified.2 {
        months -= 1;
    }
    months.max(0)
}

fn classify(age_months: i32) -> FreshnessBucket {
    if age_months < FRESH_MONTHS {
        FreshnessBucket::Fresh
    } else if age_months < STALE_MONTHS {
        FreshnessBucket::Warn
    } else {
        FreshnessBucket::Stale
    }
}

/// Convert days since the Unix epoch to a civil (year, month, day), UTC.
///
/// UTC rather than local time is deliberate: the buckets are
/// month-granularity, a day of timezone skew cannot move one, and a
/// clock that depends on the operator's timezone would make two
/// machines disagree about the same file.
///
/// Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for
/// any date this repo will ever see. Integer arithmetic only — the
/// matrix has a zero-new-deps policy, so `chrono` is not available.
fn civil_from_days(z: i64) -> (i32, i32, i32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as i32, d as i32)
}

/// "Today", for the freshness clock.
///
/// P4 (W6 audit §8.3). This used to be the hardcoded constant
/// `"2026-06-03"` — **the same date 30 of the 32 source rows carry as
/// `last_verified`**, with the other two at 2026-05-29, which the
/// day-of-month rule also floors to age 0. Absent `LIT_MATRIX_TODAY`,
/// every row computed `age_months = 0` and classified `Fresh`
/// permanently, by construction: a default run could never produce a
/// `warn` or `stale` row no matter how much time passed. The
/// `/refresh-lit-matrix` skill's whole premise ("re-verify stale rows
/// roughly once a year") depends on this report being able to say
/// `stale`, so the freeze disabled the skill rather than just the test.
///
/// The clock now reads the real system date. That makes the *report*
/// time-dependent, which is the point — the buckets are a maintenance
/// signal, not an assertion. Determinism where it is actually needed
/// (reproducing a report, pinning a comparison) comes from
/// `LIT_MATRIX_TODAY`, and the only hard failure in this file is the
/// offline URL-shape check, which does not read the clock at all.
fn today_str() -> String {
    let pinned = std::env::var("LIT_MATRIX_TODAY").ok();
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs());
    today_from_parts(pinned, epoch_secs)
}

/// The decision `today_str` makes, without the two impure reads — so it
/// is testable without mutating process environment (this workspace
/// denies `unsafe_code`, and `std::env::set_var` is unsafe).
fn today_from_parts(pinned: Option<String>, epoch_secs: Option<u64>) -> String {
    if let Some(pinned) = pinned {
        return pinned;
    }
    match epoch_secs {
        Some(secs) => {
            let (y, m, d) = civil_from_days((secs / 86_400) as i64);
            format!("{y:04}-{m:02}-{d:02}")
        }
        // System clock before the epoch: fall back rather than panic.
        None => CLOCK_UNAVAILABLE_FALLBACK.to_owned(),
    }
}

/// Build the freshness report by walking the parsed `sources.toml`
/// map. Each top-level table is treated as a source entry; the
/// `last_verified` string field controls bucketing. Missing field is
/// reported as a stale row with a note (operator must add it).
pub fn build_freshness_report(sources: &BTreeMap<String, toml::Value>) -> FreshnessReport {
    let today_s = today_str();
    let today = parse_ymd(&today_s).unwrap_or((2026, 6, 3));

    let mut rows: Vec<SourceRow> = Vec::new();
    for (key, value) in sources {
        let last_verified = value
            .as_table()
            .and_then(|t| t.get("last_verified"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let url_verified = value
            .as_table()
            .and_then(|t| t.get("url_verified"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        // The link's own clock. Ages on the same buckets as the
        // content clock, so a link that has not been touched in 18
        // months reads `stale` even when the numbers were re-read
        // yesterday.
        let (url_age_months, url_bucket) = match url_verified.as_deref().and_then(parse_ymd) {
            Some(u) => {
                let age = months_between(u, today);
                (Some(age), Some(classify(age)))
            }
            None => (None, None),
        };

        match last_verified.as_deref().and_then(parse_ymd) {
            Some(v_ymd) => {
                let age = months_between(v_ymd, today);
                let bucket = classify(age);
                // P4's self-detecting half: `months_between` floors at
                // 0, so a date AHEAD of "today" is indistinguishable
                // from one verified this morning. Name it instead of
                // absorbing it — this is what a re-frozen clock, or a
                // mistyped year, looks like.
                let note = if (v_ymd.0, v_ymd.1, v_ymd.2) > (today.0, today.1, today.2) {
                    format!(
                        "last_verified {} is in the FUTURE relative to today {} — \
                         a verification date cannot be ahead of the clock",
                        last_verified.as_deref().unwrap_or("<none>"),
                        today_s,
                    )
                } else {
                    String::new()
                };
                rows.push(SourceRow {
                    key: key.clone(),
                    last_verified: last_verified.clone(),
                    age_months: Some(age),
                    bucket,
                    url_verified: url_verified.clone(),
                    url_age_months,
                    url_bucket,
                    note,
                });
            }
            None => {
                // Missing or malformed last_verified is treated as
                // stale so the operator is forced to address it.
                let note = match &last_verified {
                    Some(s) => format!("malformed last_verified `{s}`"),
                    None => "missing last_verified field".to_owned(),
                };
                rows.push(SourceRow {
                    key: key.clone(),
                    last_verified,
                    age_months: None,
                    bucket: FreshnessBucket::Stale,
                    url_verified,
                    url_age_months,
                    url_bucket,
                    note,
                });
            }
        }
    }

    // Sort by bucket severity (Stale first, then Warn, then Fresh),
    // then alphabetically by key — operator's eye lands on the
    // problems first.
    rows.sort_by(|a, b| {
        let ord = |b: FreshnessBucket| match b {
            FreshnessBucket::Stale => 0,
            FreshnessBucket::Warn => 1,
            FreshnessBucket::Fresh => 2,
        };
        ord(a.bucket)
            .cmp(&ord(b.bucket))
            .then_with(|| a.key.cmp(&b.key))
    });

    FreshnessReport {
        today: today_s,
        rows,
    }
}

pub fn render_freshness_text(report: &FreshnessReport) -> String {
    let (fresh, warn, stale) = report.counts();
    let total = report.rows.len();
    let mut s = String::new();
    s.push_str("=== Source freshness report ===\n");
    s.push_str(&format!(
        "  today={}  total={}  fresh={}  warn={}  stale={}\n",
        report.today, total, fresh, warn, stale
    ));
    s.push_str(&format!(
        "  buckets: fresh <{} months, warn {}-{} months, stale ≥{} months\n",
        FRESH_MONTHS, FRESH_MONTHS, STALE_MONTHS, STALE_MONTHS
    ));
    let unverified = report.url_unverified_keys();
    s.push_str(&format!(
        "  citation_url clock: {} of {} rows have a confirmed link; {} never confirmed\n",
        total - unverified.len(),
        total,
        unverified.len(),
    ));
    if !unverified.is_empty() {
        s.push_str(&format!(
            "  never-confirmed links (report-only): {}\n",
            unverified.join(", ")
        ));
    }
    // Only list warn/stale rows in the human-readable output — a wall
    // of "fresh" entries is noise.
    let mut printed_any = false;
    for r in &report.rows {
        if matches!(r.bucket, FreshnessBucket::Fresh) {
            continue;
        }
        printed_any = true;
        let lv = r.last_verified.as_deref().unwrap_or("<none>");
        let age = r
            .age_months
            .map(|m| format!("{m}mo"))
            .unwrap_or_else(|| "n/a".into());
        let note = if r.note.is_empty() {
            String::new()
        } else {
            format!(" — {}", r.note)
        };
        s.push_str(&format!(
            "  [{}] {:<28} last_verified={} age={}{}\n",
            r.bucket.as_str(),
            r.key,
            lv,
            age,
            note,
        ));
    }
    if !printed_any {
        s.push_str("  all sources fresh\n");
    }
    s
}

pub fn render_freshness_json(report: &FreshnessReport) -> String {
    let rows: Vec<serde_json::Value> = report
        .rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "key": r.key,
                "last_verified": r.last_verified,
                "age_months": r.age_months,
                "bucket": r.bucket.as_str(),
                "url_verified": r.url_verified,
                "url_age_months": r.url_age_months,
                "url_bucket": r.url_bucket.map(FreshnessBucket::as_str),
                "note": r.note,
            })
        })
        .collect();
    let (fresh, warn, stale) = report.counts();
    let obj = serde_json::json!({
        "today": report.today,
        "fresh": fresh,
        "warn": warn,
        "stale": stale,
        "url_unverified": report.url_unverified_keys().len(),
        "rows": rows,
    });
    serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".into())
}

// --- Citation URL shape audit (P1) ----------------------------------------

/// One `sources.toml` row whose `citation_url` is not a usable link.
#[derive(Debug)]
pub struct MalformedUrl {
    pub key: String,
    pub value: Option<String>,
    pub reason: String,
}

/// Offline validation of every source row's `citation_url`.
///
/// W6 audit (2026-08-04) §8.2 "Hole 1": `citation_url` was **never
/// read** — `build_freshness_report` reads only `last_verified`, and
/// `audit_citations` is a referential-integrity check on source *keys*.
/// Six rows shipped the literal string `"(pending)"` and every one of
/// them was reported `fresh`.
///
/// This is deliberately a **shape** check, not a liveness check: it does
/// no network I/O, adds no dependency, and is safe to run in CI. It
/// cannot detect a URL that has rotted to 404 — that is the opt-in
/// liveness proposal (P3), which is not implemented and belongs to the
/// `/refresh-lit-matrix` walk.
///
/// Rules, each with a failure the census actually contained:
/// 1. field present (a row with no `citation_url` at all cites nothing)
/// 2. non-empty after trimming
/// 3. not a `(...)`-shaped sentinel — catches `"(pending)"`, `"(tbd)"`,
///    `"(none)"` and any future placeholder of the same shape
/// 4. starts with `http://` or `https://`
/// 5. has a host, and the host contains a dot (rejects `https://` alone,
///    `https://localhost`, and accidental path-only strings)
pub fn audit_citation_urls(sources: &BTreeMap<String, toml::Value>) -> Vec<MalformedUrl> {
    let mut bad = Vec::new();
    for (key, value) in sources {
        let raw = value
            .as_table()
            .and_then(|t| t.get("citation_url"))
            .and_then(|v| v.as_str());
        let Some(raw) = raw else {
            bad.push(MalformedUrl {
                key: key.clone(),
                value: None,
                reason: "missing `citation_url` field".to_owned(),
            });
            continue;
        };
        let url = raw.trim();
        let reason = if url.is_empty() {
            Some("empty `citation_url`".to_owned())
        } else if url.starts_with('(') && url.ends_with(')') {
            Some(format!(
                "placeholder sentinel `{url}` — a source with no retrievable \
                 document must either carry a real URL or be rewritten as \
                 repo-authored (see CREDITS.md)"
            ))
        } else if !(url.starts_with("http://") || url.starts_with("https://")) {
            Some("not an http(s) URL".to_owned())
        } else {
            let rest = url
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or_default();
            let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
            if host.is_empty() {
                Some("no host".to_owned())
            } else if !host.contains('.') {
                Some(format!("host `{host}` has no dot"))
            } else {
                None
            }
        };
        if let Some(reason) = reason {
            bad.push(MalformedUrl {
                key: key.clone(),
                value: Some(url.to_owned()),
                reason,
            });
        }
    }
    bad
}

pub fn render_url_audit_text(bad: &[MalformedUrl]) -> String {
    let mut s = String::new();
    s.push_str("=== Citation URL shape audit (offline; no network) ===\n");
    if bad.is_empty() {
        s.push_str("  all citation_url values are well-formed links\n");
    } else {
        s.push_str(&format!("  {} malformed citation_url(s):\n", bad.len()));
        for m in bad {
            let shown = m.value.as_deref().unwrap_or("<absent>");
            s.push_str(&format!("  - `{}` = `{}` — {}\n", m.key, shown, m.reason));
        }
    }
    s
}

// --- Citation audit -------------------------------------------------------

#[derive(Debug)]
pub struct CitationAudit {
    /// Map: cell_id -> list of (location, missing_key) pairs.
    pub missing: Vec<MissingCitation>,
}

#[derive(Debug)]
pub struct MissingCitation {
    pub cell_id: String,
    pub location: String,
    pub key: String,
}

pub fn audit_citations(
    cells: &CellsFile,
    sources: &BTreeMap<String, toml::Value>,
) -> CitationAudit {
    let known: BTreeSet<&str> = sources.keys().map(String::as_str).collect();
    let mut missing: Vec<MissingCitation> = Vec::new();

    for cell in &cells.cells {
        // Bands attached to ExpectedBands fields.
        let band_slots: [(&str, Option<&super::cell::Band>); 5] = [
            ("expected.rpm", cell.expected.rpm.as_ref()),
            (
                "expected.feed_per_tooth",
                cell.expected.feed_per_tooth.as_ref(),
            ),
            ("expected.axial_doc", cell.expected.axial_doc.as_ref()),
            ("expected.radial_woc", cell.expected.radial_woc.as_ref()),
            ("expected.plunge_feed", cell.expected.plunge_feed.as_ref()),
        ];
        for (label, band_opt) in band_slots {
            if let Some(band) = band_opt {
                for src in &band.sources {
                    if !known.contains(src.as_str()) {
                        missing.push(MissingCitation {
                            cell_id: cell.id.clone(),
                            location: label.into(),
                            key: src.clone(),
                        });
                    }
                }
            }
        }
        // Extras bands.
        for (k, band) in &cell.expected.extras {
            for src in &band.sources {
                if !known.contains(src.as_str()) {
                    missing.push(MissingCitation {
                        cell_id: cell.id.clone(),
                        location: format!("expected.{k}"),
                        key: src.clone(),
                    });
                }
            }
        }
        // Invariants do not currently carry a typed `sources` field in
        // the schema (see cell.rs::Invariant) — they live in the TOML
        // but are not deserialized into the struct. Skip them here;
        // when invariant sources are formalised in the schema this
        // audit will pick them up automatically.
        //
        // Anti-patterns: same story — `sources` is in the TOML but not
        // on the typed struct. Skip for now.
    }

    CitationAudit { missing }
}

pub fn render_citation_audit_text(audit: &CitationAudit) -> String {
    let mut s = String::new();
    s.push_str("=== Source citation audit ===\n");
    if audit.missing.is_empty() {
        s.push_str("  all citations resolve to known sources\n");
    } else {
        s.push_str(&format!("  {} missing citation(s):\n", audit.missing.len()));
        for m in &audit.missing {
            s.push_str(&format!(
                "  - cell `{}` @ {} cites unknown source `{}`\n",
                m.cell_id, m.location, m.key
            ));
        }
    }
    s
}

pub fn decay_fail_enabled() -> bool {
    std::env::var("LIT_MATRIX_DECAY_FAIL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_between_basic() {
        assert_eq!(months_between((2025, 6, 3), (2026, 6, 3)), 12);
        assert_eq!(months_between((2025, 6, 3), (2026, 6, 2)), 11);
        assert_eq!(months_between((2024, 12, 1), (2026, 6, 3)), 18);
        assert_eq!(months_between((2027, 1, 1), (2026, 6, 3)), 0);
    }

    #[test]
    fn classify_bands() {
        assert_eq!(classify(0), FreshnessBucket::Fresh);
        assert_eq!(classify(11), FreshnessBucket::Fresh);
        assert_eq!(classify(12), FreshnessBucket::Warn);
        assert_eq!(classify(17), FreshnessBucket::Warn);
        assert_eq!(classify(18), FreshnessBucket::Stale);
        assert_eq!(classify(99), FreshnessBucket::Stale);
    }

    /// P4: the clock must actually advance. Anchors are the epoch and
    /// three dates whose day-numbers are independently checkable.
    #[test]
    fn civil_from_days_anchors() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_607), (2026, 6, 3));
        // Leap day, to catch an off-by-one in the era arithmetic.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    /// The pin still works — that is what keeps a report reproducible
    /// now that the default is a real date — and the unpinned path
    /// tracks the clock instead of a constant.
    #[test]
    fn today_is_pinned_when_asked_and_real_otherwise() {
        assert_eq!(
            today_from_parts(Some("2027-01-15".to_owned()), Some(0)),
            "2027-01-15",
            "an explicit pin must win over the clock"
        );
        // 2026-06-03T00:00:00Z = 20607 days after the epoch.
        assert_eq!(today_from_parts(None, Some(20_607 * 86_400)), "2026-06-03");
        // One year on, the same code must say so — this is the
        // property the frozen constant could not have.
        assert_eq!(today_from_parts(None, Some(20_972 * 86_400)), "2027-06-03");
        assert_eq!(
            today_from_parts(None, None),
            CLOCK_UNAVAILABLE_FALLBACK,
            "an unreadable clock falls back rather than panicking"
        );
    }

    #[test]
    fn parse_ymd_ok_and_err() {
        assert_eq!(parse_ymd("2026-06-03"), Some((2026, 6, 3)));
        assert_eq!(parse_ymd("2026/06/03"), None);
        assert_eq!(parse_ymd("2026-13-01"), None);
    }
}
