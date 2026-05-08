//! Sample-driven retargeters — translate per-gate sim verdicts into
//! axis patches. One implementation per gate.
//!
//! Step 5 scaffold (G16). Concrete implementations live in
//! `chipload.rs`, `power.rs`, `deflection.rs` — each picked up by a
//! parallel agent. The shared trait + `RetargetSolution` are defined
//! here so all three can land independently.

use super::axes::{AxisContext, AxisView, SearchAxis};
use super::patches::AxisPatch;
use super::space::SearchSpace;

pub mod chipload;
pub mod deflection;
pub mod power;

/// One retargeter per load-driving gate. The verdict type is gate-
/// specific at the trait level; Step 7 swaps the existing flat `Verdict`
/// for typed verdicts (`ChiploadVerdict`, `PowerVerdict`,
/// `DeflectionVerdict`) — that change is local to each retargeter file.
pub trait Retargeter {
    type Verdict;

    /// Axes this retargeter is allowed to drive. Declaring this in the
    /// trait is part of the contract — a retargeter that names only
    /// `[FeedRate]` commits to NOT touching RPM, which keeps the linear
    /// chipload-feed math correct.
    fn driving_axes(&self) -> &'static [SearchAxis];

    /// Compute a target patch list for the given verdict. Returns
    /// `None` when the verdict isn't an `Exceeds` arm of the gate this
    /// retargeter handles (each retargeter is typed to exactly one
    /// gate's verdict via the associated `Verdict` type).
    fn target(
        &self,
        verdict: &Self::Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution>;
}

/// Result of a successful retarget. `patches` carry the change(s) to
/// apply; `rationale` is a human-readable explanation surfaced in MCP /
/// GUI output. Multi-patch when the retargeter has coupled levers
/// (chipload retarget produces a feed patch and a coupled plunge-
/// tracking patch).
#[derive(Debug, Clone)]
pub struct RetargetSolution {
    pub patches: Vec<AxisPatch>,
    pub rationale: String,
}
