//! Cell-level verdict aggregation.
//!
//! Sub-verdict tiers come from `invariant::SubVerdict`. The cell verdict
//! rolls them up to one of:
//!
//!   cosmetic | minor | moderate | major | critical
//!
//! per the plan's "CI policy" table.

use super::invariant::{SubVerdict, SubVerdictDetail};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Cosmetic,
    Minor,
    Moderate,
    Major,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Cosmetic => "cosmetic",
            Severity::Minor => "minor",
            Severity::Moderate => "moderate",
            Severity::Major => "major",
            Severity::Critical => "critical",
        }
    }
    pub fn parse(s: &str) -> Severity {
        match s.to_lowercase().as_str() {
            "cosmetic" => Severity::Cosmetic,
            "minor" => Severity::Minor,
            "moderate" => Severity::Moderate,
            "major" => Severity::Major,
            "critical" => Severity::Critical,
            _ => Severity::Moderate,
        }
    }
    /// Severity blocks CI (per plan: major and critical block).
    pub fn blocks_ci(self) -> bool {
        matches!(self, Severity::Major | Severity::Critical)
    }
}

#[derive(Debug, Clone)]
pub struct SubVerdictRow {
    pub label: String,
    pub detail: SubVerdictDetail,
    pub severity_on_fail: Severity,
}

#[derive(Debug, Clone)]
pub struct CellVerdict {
    pub cell_id: String,
    pub rows: Vec<SubVerdictRow>,
    pub overall: Severity,
    pub summary: String,
    /// Wrong-tool category for this cell (`"values"` for normal-use,
    /// `"unadvised"`, `"unusable"`, or `"refuse"`). Surfaced in the
    /// report so a reader knows the verdict cap that was applied. See
    /// the plan §"Wrong-tool discrimination".
    pub mode: String,
}

impl CellVerdict {
    /// Cap the overall severity at the given ceiling. Used for
    /// `unadvised` cells where any sub-verdict failure must not block
    /// CI — the row-level reasons are preserved (the operator/agent
    /// still sees them), but the rolled-up severity caps out at Minor.
    pub fn cap_overall(&mut self, ceiling: Severity) {
        if self.overall > ceiling {
            self.overall = ceiling;
            self.summary = format!(
                "{} (capped — {} rows present)",
                ceiling.as_str(),
                self.rows.len()
            );
        }
    }
}

/// Roll up a list of sub-verdict rows into a cell verdict.
///
/// Rules (per plan §"Verdict shape" + §"CI policy"):
///   - Each row carries its own `severity_on_fail` (declared in the cell
///     TOML, defaulting to `moderate`).
///   - `Within` and `NA` rows contribute no severity.
///   - `Edge` rows contribute one tier below the row's declared severity
///     (clamped to Cosmetic).
///   - `Outside` rows contribute the full declared severity.
///   - Overall = max severity across all contributing rows.
///   - Special case: if *every* row is `Within` or `NA`, overall = Cosmetic
///     (log-only).
pub fn rollup(cell_id: &str, rows: Vec<SubVerdictRow>) -> CellVerdict {
    let mut overall = Severity::Cosmetic;
    let mut worst_row: Option<&SubVerdictRow> = None;
    for row in &rows {
        let contribution = match row.detail.verdict {
            SubVerdict::Within | SubVerdict::NA => continue,
            SubVerdict::Edge => one_tier_below(row.severity_on_fail),
            SubVerdict::Outside => row.severity_on_fail,
        };
        if contribution > overall {
            overall = contribution;
            worst_row = Some(row);
        }
    }
    let summary = match worst_row {
        Some(row) => format!(
            "{} ({}: {})",
            overall.as_str(),
            row.label,
            row.detail.reason
        ),
        None => format!("{} (all checks within bands)", overall.as_str()),
    };
    CellVerdict {
        cell_id: cell_id.to_owned(),
        rows,
        overall,
        summary,
        mode: "values".to_owned(),
    }
}

fn one_tier_below(s: Severity) -> Severity {
    match s {
        Severity::Critical => Severity::Major,
        Severity::Major => Severity::Moderate,
        Severity::Moderate => Severity::Minor,
        Severity::Minor => Severity::Cosmetic,
        Severity::Cosmetic => Severity::Cosmetic,
    }
}
