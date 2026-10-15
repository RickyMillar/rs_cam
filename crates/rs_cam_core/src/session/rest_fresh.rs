//! Operator ruling 2026-10-15: compare a rest operation with the same
//! operation on fresh stock, ON REQUEST.
//!
//! Rest can legitimately take longer than fresh (small islands need their
//! own entries and rapids), so the operator is told the two times, not
//! held to a bar. A rest generator never plans the fresh motion it leaves
//! out, so the only honest fresh time is one more generation (the fresh
//! twin). That costs a whole generation, so it runs only when a surface
//! asks for it (MCP `compare_rest_with_fresh`, the GUI Stock tab button).
//!
//! The request is two steps, as a generation is:
//!
//! 1. [`ProjectSession::compare_rest_with_fresh`] (`&self`) generates the
//!    twin and answers [`AdoptRestFreshComparisonArgs`];
//! 2. `apply(Command::AdoptRestFreshComparison(..))` records it.
//!
//! A recorded comparison is current while the toolpath keeps its revision
//! and its result keeps the planned cycle time the comparison read
//! ([`ProjectSession::current_rest_fresh_comparisons`]). It goes stale with
//! the result: an edit, a regenerate with other motion, or a dropped
//! result. Only a current comparison reaches the triage
//! (`project.rest_slower_than_fresh`).

use std::sync::atomic::AtomicBool;

use super::{AdoptRestFreshComparisonArgs, ProjectSession, SessionError};
use crate::compute::toolpath_stats::FreshTwinTime;
use crate::ids::ToolpathId;

/// One recorded comparison: the revision and the times it read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RestFreshRecord {
    /// The toolpath revision the twin was generated at.
    pub revision: u64,
    /// The two times.
    pub times: FreshTwinTime,
}

impl ProjectSession {
    /// Generate the fresh twin of rest toolpath `index` and answer both
    /// cycle times, one measure ([`FreshTwinTime`]). The rest time is the
    /// stored result's planned time (`ToolpathStats::planned_cycle_time_s`);
    /// the fresh time is the twin's
    /// ([`super::compute::fresh_twin_cycle_time`]). Nothing is simulated.
    ///
    /// # Errors
    ///
    /// [`SessionError::ToolpathNotFound`] for an unknown index, and
    /// [`SessionError::OperationFailed`] when the toolpath is not a rest
    /// operation, holds no generated result with a planned time, or the
    /// twin fails, is cancelled or comes out empty.
    pub fn compare_rest_with_fresh(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<AdoptRestFreshComparisonArgs, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        if tc.stock_source != crate::session::StockSource::FromRemainingStock {
            return Err(SessionError::OperationFailed(format!(
                "'{}' cuts fresh stock; only a rest operation (remaining stock) has a fresh \
                 twin to compare with",
                tc.name
            )));
        }
        let rest_s = self
            .results
            .get(&index)
            .and_then(|r| r.stats.planned_cycle_time_s)
            .ok_or_else(|| {
                SessionError::OperationFailed(format!(
                    "'{}' holds no generated result to compare; generate it first",
                    tc.name
                ))
            })?;
        let handle = self.capture_generate_handle(index, cancel)?;
        let fresh_s = super::compute::fresh_twin_cycle_time(
            &handle,
            &super::compute::GenObserver::none(),
            cancel,
        )
        .ok_or_else(|| {
            SessionError::OperationFailed(format!(
                "the fresh twin of '{}' did not generate (failed, cancelled or empty)",
                tc.name
            ))
        })?;
        Ok(AdoptRestFreshComparisonArgs {
            index,
            revision: handle.revision,
            times: FreshTwinTime { rest_s, fresh_s },
        })
    }

    /// The recorded comparisons that are still current, by toolpath id: the
    /// toolpath keeps the recorded revision, and its result keeps the
    /// planned time the comparison read.
    pub fn current_rest_fresh_comparisons(
        &self,
    ) -> std::collections::BTreeMap<ToolpathId, FreshTwinTime> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .filter_map(|(index, tc)| {
                let record = self.rest_fresh_comparisons.get(&tc.id)?;
                let planned = self.results.get(&index)?.stats.planned_cycle_time_s?;
                (record.revision == self.toolpath_revision(index)
                    && planned.to_bits() == record.times.rest_s.to_bits())
                .then_some((tc.id, record.times))
            })
            .collect()
    }
}
