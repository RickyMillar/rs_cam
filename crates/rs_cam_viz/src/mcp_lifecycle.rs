//! MCP transport lifetime shared between the server thread and winit host.
//!
//! The completion guard covers returns and unwinds that cross the outer
//! `mcp-server` thread closure. Panics in detached Tokio tasks or request
//! handlers do not cross that boundary and are not claimed as covered here.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use crate::GuiWaker;

/// Time allowed for winit to acknowledge an MCP-driven exit request.
///
/// Two seconds is intentionally much longer than a healthy event-loop turn but
/// still bounds shutdown when Wayland/FIFO has blocked the host in presentation.
const MCP_HOST_EXIT_GRACE: Duration = Duration::from_secs(2);

const RUNNING: u8 = 0;
const CLEAN_EOF: u8 = 1;
const FAILURE: u8 = 2;

#[repr(u8)]
enum WatchdogState {
    Armed,
    Acknowledged,
    Firing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpExitOutcome {
    CleanEof,
    Failure,
}

impl McpExitOutcome {
    const fn state(self) -> u8 {
        match self {
            Self::CleanEof => CLEAN_EOF,
            Self::Failure => FAILURE,
        }
    }

    const fn exit_code(self) -> i32 {
        match self {
            Self::CleanEof => 0,
            Self::Failure => 1,
        }
    }
}

type WatchdogAction = Arc<dyn Fn(McpExitOutcome) + Send + Sync>;

struct McpExitState {
    request_state: AtomicU8,
    watchdog_state: AtomicU8,
    watchdog_grace: Duration,
    watchdog_action: WatchdogAction,
}

impl McpExitState {
    fn outcome(&self) -> Option<McpExitOutcome> {
        match self.request_state.load(Ordering::Acquire) {
            CLEAN_EOF => Some(McpExitOutcome::CleanEof),
            FAILURE => Some(McpExitOutcome::Failure),
            _ => None,
        }
    }

    fn claim_watchdog_exit(&self) -> Option<McpExitOutcome> {
        self.watchdog_state
            .compare_exchange(
                WatchdogState::Armed as u8,
                WatchdogState::Firing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok()
            .and_then(|_| self.outcome())
    }
}

/// One-way notification that the MCP server thread has finished.
///
/// The first requester fixes the process outcome, starts a process-owned
/// watchdog, and wakes winit. If the host cannot acknowledge within
/// [`MCP_HOST_EXIT_GRACE`], the watchdog terminates the process with status 0
/// for clean EOF or 1 for failure. Later requests cannot overwrite the first
/// outcome.
#[derive(Clone)]
pub(crate) struct McpExitSignal {
    state: Arc<McpExitState>,
    waker: GuiWaker,
}

impl McpExitSignal {
    pub(crate) fn new(waker: GuiWaker) -> Self {
        Self::with_watchdog(
            waker,
            MCP_HOST_EXIT_GRACE,
            Arc::new(|outcome| std::process::exit(outcome.exit_code())),
        )
    }

    fn with_watchdog(
        waker: GuiWaker,
        watchdog_grace: Duration,
        watchdog_action: WatchdogAction,
    ) -> Self {
        Self {
            state: Arc::new(McpExitState {
                request_state: AtomicU8::new(RUNNING),
                watchdog_state: AtomicU8::new(WatchdogState::Armed as u8),
                watchdog_grace,
                watchdog_action,
            }),
            waker,
        }
    }

    #[cfg(test)]
    fn new_without_hard_exit(
        waker: GuiWaker,
        watchdog_grace: Duration,
    ) -> (Self, std::sync::mpsc::Receiver<McpExitOutcome>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let action = Arc::new(move |outcome| {
            let _ = tx.send(outcome);
        });
        (Self::with_watchdog(waker, watchdog_grace, action), rx)
    }

    fn request_exit(&self, outcome: McpExitOutcome) {
        if self
            .state
            .request_state
            .compare_exchange(
                RUNNING,
                outcome.state(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }

        // Start the process-owned fallback before asking winit to wake. Even if
        // the compositor has blocked the event-loop thread, this thread remains
        // able to enforce the shutdown deadline.
        let state = Arc::clone(&self.state);
        let spawn_result = std::thread::Builder::new()
            .name("mcp-exit-watchdog".into())
            .spawn(move || {
                std::thread::sleep(state.watchdog_grace);
                if let Some(outcome) = state.claim_watchdog_exit() {
                    (state.watchdog_action)(outcome);
                }
            });

        (self.waker)();

        // Thread creation failure must not turn the watchdog into a best-effort
        // promise. Production exits immediately; tests receive the outcome via
        // their no-hard-exit action.
        if spawn_result.is_err() {
            (self.state.watchdog_action)(outcome);
        }
    }

    #[cfg(any(feature = "mcp", test))]
    pub(crate) fn request_failure(&self) {
        self.request_exit(McpExitOutcome::Failure);
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.outcome().is_some()
    }

    pub(crate) fn outcome(&self) -> Option<McpExitOutcome> {
        self.state.outcome()
    }

    pub(crate) fn acknowledge(&self) {
        let _ = self.state.watchdog_state.compare_exchange(
            WatchdogState::Armed as u8,
            WatchdogState::Acknowledged as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    #[cfg(any(feature = "mcp", test))]
    pub(crate) fn waker(&self) -> GuiWaker {
        Arc::clone(&self.waker)
    }
}

/// Reports failure unless clean service completion is explicitly recorded.
///
/// This guard covers only an unwind crossing the outer server-thread closure;
/// it cannot observe a panic confined to a detached handler task.
#[cfg(any(feature = "mcp", test))]
pub(crate) struct McpCompletionGuard {
    signal: McpExitSignal,
    outcome: McpExitOutcome,
}

#[cfg(any(feature = "mcp", test))]
impl McpCompletionGuard {
    pub(crate) fn new(signal: McpExitSignal) -> Self {
        Self {
            signal,
            outcome: McpExitOutcome::Failure,
        }
    }

    pub(crate) fn mark_clean(&mut self) {
        self.outcome = McpExitOutcome::CleanEof;
    }
}

#[cfg(any(feature = "mcp", test))]
impl Drop for McpCompletionGuard {
    fn drop(&mut self) {
        let outcome = if std::thread::panicking() {
            McpExitOutcome::Failure
        } else {
            self.outcome
        };
        self.signal.request_exit(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::{McpCompletionGuard, McpExitOutcome, McpExitSignal};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    const TEST_GRACE: Duration = Duration::from_millis(100);

    fn counting_signal() -> (
        McpExitSignal,
        Arc<AtomicUsize>,
        std::sync::mpsc::Receiver<McpExitOutcome>,
    ) {
        let wakes = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&wakes);
        let waker = Arc::new(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        let (signal, watchdog) = McpExitSignal::new_without_hard_exit(waker, TEST_GRACE);
        (signal, wakes, watchdog)
    }

    #[test]
    fn first_exit_request_fixes_failure_and_wakes_only_once() {
        let (signal, wakes, _watchdog) = counting_signal();

        signal.request_failure();
        signal.request_failure();

        assert!(signal.is_requested());
        assert_eq!(signal.outcome(), Some(McpExitOutcome::Failure));
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        signal.acknowledge();
    }

    #[test]
    fn completion_guard_defaults_to_failure() {
        let (signal, _wakes, _watchdog) = counting_signal();

        {
            let _completion = McpCompletionGuard::new(signal.clone());
        }

        assert_eq!(signal.outcome(), Some(McpExitOutcome::Failure));
        signal.acknowledge();
    }

    #[test]
    fn completion_guard_records_clean_wait_completion() {
        let (signal, _wakes, _watchdog) = counting_signal();

        {
            let mut completion = McpCompletionGuard::new(signal.clone());
            completion.mark_clean();
        }

        assert_eq!(signal.outcome(), Some(McpExitOutcome::CleanEof));
        signal.acknowledge();
    }

    #[test]
    fn completion_guard_marks_outer_thread_unwind_failed_even_after_clean_mark() {
        let (signal, _wakes, _watchdog) = counting_signal();
        let thread_signal = signal.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut completion = McpCompletionGuard::new(thread_signal);
            completion.mark_clean();
            std::panic::resume_unwind(Box::new("intentional outer-thread unwind"));
        }));

        assert!(result.is_err());
        assert_eq!(signal.outcome(), Some(McpExitOutcome::Failure));
        signal.acknowledge();
    }

    #[test]
    fn watchdog_reports_clean_outcome_without_host_acknowledgement() {
        let (signal, _wakes, watchdog) = counting_signal();

        {
            let mut completion = McpCompletionGuard::new(signal);
            completion.mark_clean();
        }

        assert_eq!(
            watchdog.recv_timeout(Duration::from_secs(1)),
            Ok(McpExitOutcome::CleanEof)
        );
    }

    #[test]
    fn host_acknowledgement_disarms_watchdog() {
        let (signal, _wakes, watchdog) = counting_signal();

        signal.request_failure();
        signal.acknowledge();

        assert_eq!(
            watchdog.recv_timeout(TEST_GRACE.saturating_mul(3)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        );
    }

    #[test]
    fn host_and_watchdog_have_one_atomic_winner() {
        let (host_wins, _wakes, _watchdog) = counting_signal();
        host_wins.request_failure();
        host_wins.acknowledge();
        assert_eq!(host_wins.state.claim_watchdog_exit(), None);
        assert_eq!(host_wins.outcome(), Some(McpExitOutcome::Failure));

        let (watchdog_wins, _wakes, _watchdog) = counting_signal();
        watchdog_wins.request_failure();
        assert_eq!(
            watchdog_wins.state.claim_watchdog_exit(),
            Some(McpExitOutcome::Failure)
        );
        watchdog_wins.acknowledge();
        assert_eq!(watchdog_wins.outcome(), Some(McpExitOutcome::Failure));
    }
}
