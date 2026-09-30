use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared cancellation predicate used by cancellable core algorithms.
pub trait CancelCheck {
    fn cancelled(&self) -> bool;
}

impl<F> CancelCheck for F
where
    F: Fn() -> bool + Send + Sync,
{
    fn cancelled(&self) -> bool {
        self()
    }
}

/// Error returned when a long-running computation is cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// A [`CancelCheck`] that cannot fire.
///
/// Every generator publishes a cancellable form and an uncancellable
/// convenience wrapper over it. Twenty-six of those wrappers each built their
/// own `|| false` closure and each carried their own
/// `#[allow(clippy::expect_used)]` over the `Result` it made unreachable.
/// This type and [`run_uncancellable`] are the one copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NeverCancel;

impl CancelCheck for NeverCancel {
    fn cancelled(&self) -> bool {
        false
    }
}

/// A [`CancelCheck`] over a borrowed `AtomicBool`.
///
/// Many algorithms and lanes hold a `&AtomicBool` and wrote their own
/// `|| flag.load(Ordering::SeqCst)` closure to reach a `&dyn CancelCheck`.
/// This type is that adapter. A [`crate::budget::BudgetGuard`] sets the same
/// flag when the memory budget trips, so a caller that passes
/// `FlagCancel(guard.flag())` stops for the budget too, with no signature
/// change.
#[derive(Debug, Clone, Copy)]
pub struct FlagCancel<'a>(pub &'a AtomicBool);

impl CancelCheck for FlagCancel<'_> {
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Run a cancellable computation under [`NeverCancel`] and return its answer.
///
/// This holds the workspace's ONE `expect` for the idiom. A `Cancelled` here
/// cannot come from the caller, because [`NeverCancel::cancelled`] returns
/// `false` unconditionally — it could only mean the computation invented a
/// cancellation it was never told about, which is a defect in that
/// computation and must not be mapped to a silent empty answer.
// SAFETY: unreachable by construction — see the paragraph above.
#[allow(clippy::expect_used)]
pub fn run_uncancellable<T>(run: impl FnOnce(&NeverCancel) -> Result<T, Cancelled>) -> T {
    run(&NeverCancel).expect("a computation run under NeverCancel reported cancellation")
}

#[inline]
pub fn check_cancel(cancel: &dyn CancelCheck) -> Result<(), Cancelled> {
    if cancel.cancelled() {
        Err(Cancelled)
    } else {
        Ok(())
    }
}
