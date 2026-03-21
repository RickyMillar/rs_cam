use std::fmt;

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

#[inline]
pub fn check_cancel(cancel: &dyn CancelCheck) -> Result<(), Cancelled> {
    if cancel.cancelled() {
        Err(Cancelled)
    } else {
        Ok(())
    }
}
