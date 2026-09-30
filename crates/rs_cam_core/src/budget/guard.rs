//! The per-job guard: one cancel flag, one stop reason, one usage probe.
//!
//! A job shares ONE [`BudgetGuard`] through an `Arc`. The guard stops the job
//! for one of two reasons ([`StopReason`]) and records the FIRST one, so a
//! lane can report "over budget" and not a bare "cancelled" (B3).
//!
//! Two doors reach the guard:
//!
//! - an algorithm that takes `&dyn CancelCheck` gets the guard itself: its
//!   [`CancelCheck::cancelled`] reads the flag and polls the probe;
//! - an algorithm that takes `&AtomicBool` today gets [`BudgetGuard::flag`].
//!   The guard sets that flag when the budget trips, so the algorithm stops
//!   at its next flag read with no signature change. Something must poll the
//!   probe for such a job: [`BudgetGuard::watch`] starts a thread that does.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{MemoryBudget, StopReason};
use crate::interrupt::CancelCheck;

/// The shortest time between two probe reads of one guard.
///
/// This is a RATE LIMIT on the probe, not a memory number: `cancelled()` runs
/// in hot loops on many threads, and a probe read costs a file read on
/// Linux (`/proc/self/…`). A job that allocates faster than this interval
/// can pass the limit by what it allocates in one interval; the cgroup stays
/// the outer net for that case (plan, "Architecture" 7). It is a tuning
/// value; change it here only.
pub const PROBE_INTERVAL: Duration = Duration::from_millis(100);

/// A reading of the memory the process uses.
pub trait UsageProbe: Send + Sync {
    /// The bytes in use now, or `None` when the platform does not say.
    fn used_bytes(&self) -> Option<u64>;
}

/// The resident set size of this process, through `memory-stats` 1.2:
/// `memory_stats::memory_stats()` and its `physical_mem` field (RSS on Linux
/// and macOS, the working set on Windows).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessRss;

impl UsageProbe for ProcessRss {
    fn used_bytes(&self) -> Option<u64> {
        memory_stats::memory_stats().and_then(|stats| u64::try_from(stats.physical_mem).ok())
    }
}

/// The cancel flag, the stop reason and the usage probe of one job.
pub struct BudgetGuard {
    flag: Arc<AtomicBool>,
    reason: OnceLock<StopReason>,
    budget: MemoryBudget,
    probe: Box<dyn UsageProbe>,
    interval: Duration,
    last_probe: Mutex<Option<Instant>>,
}

impl fmt::Debug for BudgetGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetGuard")
            .field("flag", &self.flag.load(Ordering::SeqCst))
            .field("reason", &self.reason.get())
            .field("budget", &self.budget)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl BudgetGuard {
    /// A guard with its own flag and the process RSS probe.
    #[must_use]
    pub fn new(budget: MemoryBudget) -> Self {
        Self::with_flag(Arc::new(AtomicBool::new(false)), budget)
    }

    /// A guard over a flag that the caller already holds, for example a
    /// worker lane's `Arc<AtomicBool>`. The existing cancel button keeps
    /// working through that flag.
    #[must_use]
    pub fn with_flag(flag: Arc<AtomicBool>, budget: MemoryBudget) -> Self {
        Self::with_probe(flag, budget, Box::new(ProcessRss), PROBE_INTERVAL)
    }

    /// A guard with a given probe and rate limit. Tests use this with a
    /// fake probe and a zero interval.
    #[must_use]
    pub fn with_probe(
        flag: Arc<AtomicBool>,
        budget: MemoryBudget,
        probe: Box<dyn UsageProbe>,
        interval: Duration,
    ) -> Self {
        Self {
            flag,
            reason: OnceLock::new(),
            budget,
            probe,
            interval,
            last_probe: Mutex::new(None),
        }
    }

    /// The flag an `&AtomicBool` algorithm reads. The guard sets it on every
    /// stop.
    #[must_use]
    pub fn flag(&self) -> &AtomicBool {
        &self.flag
    }

    /// A second owner of the flag.
    #[must_use]
    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }

    /// The budget this guard enforces.
    #[must_use]
    pub fn budget(&self) -> MemoryBudget {
        self.budget
    }

    /// Stop the job for `reason`. The first reason stays; a later one only
    /// sets the flag again.
    pub fn stop(&self, reason: StopReason) {
        // The first reason wins, so a failed `set` is the expected case.
        let _ = self.reason.set(reason);
        self.flag.store(true, Ordering::SeqCst);
    }

    /// The operator cancelled the job.
    pub fn request_cancel(&self) {
        self.stop(StopReason::User);
    }

    /// Why the job stopped, or `None` while it runs.
    ///
    /// A flag that some other code set, with no recorded reason, reads as
    /// [`StopReason::User`]: before this guard, only the operator set it.
    #[must_use]
    pub fn stop_reason(&self) -> Option<StopReason> {
        if let Some(reason) = self.reason.get() {
            return Some(*reason);
        }
        self.flag.load(Ordering::SeqCst).then_some(StopReason::User)
    }

    /// True when the job must stop. Reads the flag, then reads the probe
    /// when [`PROBE_INTERVAL`] has passed since the last read.
    pub fn poll(&self) -> bool {
        if self.flag.load(Ordering::SeqCst) {
            return true;
        }
        if !self.budget.is_limited() || !self.probe_due() {
            return false;
        }
        self.probe_now()
    }

    /// Read the probe now, with no rate limit. Stops the job with
    /// [`StopReason::OverBudget`] when the process uses more than the
    /// limit. Returns true when the job must stop.
    pub fn probe_now(&self) -> bool {
        if let Some(limit_bytes) = self.budget.limit_bytes
            && let Some(used) = self.probe.used_bytes()
            && used > limit_bytes
        {
            self.stop(StopReason::OverBudget {
                need_bytes: used,
                limit_bytes,
            });
        }
        self.flag.load(Ordering::SeqCst)
    }

    /// The preflight door: stop the job before it allocates when the
    /// estimate `need_bytes` is over the limit.
    ///
    /// # Errors
    /// The [`StopReason`] of the job: the recorded one when the job has
    /// already stopped, else [`StopReason::OverBudget`] for this estimate.
    pub fn check_estimate(&self, need_bytes: u64) -> Result<(), StopReason> {
        if let Some(reason) = self.stop_reason() {
            return Err(reason);
        }
        self.budget.check(need_bytes).inspect_err(|reason| {
            self.stop(*reason);
        })
    }

    /// Start a thread that polls the probe every [`PROBE_INTERVAL`] until
    /// the job stops or the returned handle drops. Use it for a job whose
    /// algorithms read [`Self::flag`] and never call [`Self::poll`].
    ///
    /// A guard with no limit starts no thread.
    #[must_use]
    pub fn watch(self: &Arc<Self>) -> BudgetWatch {
        if !self.budget.is_limited() {
            return BudgetWatch {
                done: Arc::new(AtomicBool::new(true)),
                thread: None,
            };
        }
        let done = Arc::new(AtomicBool::new(false));
        let guard = Arc::clone(self);
        let thread_done = Arc::clone(&done);
        let spawned = std::thread::Builder::new()
            .name("rs_cam-budget-watch".to_owned())
            .spawn(move || {
                while !thread_done.load(Ordering::SeqCst) {
                    if guard.probe_now() {
                        return;
                    }
                    std::thread::park_timeout(guard.interval);
                }
            });
        let thread = match spawned {
            Ok(handle) => Some(handle),
            Err(error) => {
                tracing::warn!(%error, "memory budget watcher did not start");
                None
            }
        };
        BudgetWatch { done, thread }
    }

    /// True when the rate limit allows a probe read now. Records the time.
    /// A thread that finds the lock taken skips this read.
    fn probe_due(&self) -> bool {
        let Ok(mut last) = self.last_probe.try_lock() else {
            return false;
        };
        let now = Instant::now();
        match *last {
            Some(then) if now.duration_since(then) < self.interval => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

impl CancelCheck for BudgetGuard {
    fn cancelled(&self) -> bool {
        self.poll()
    }
}

/// The handle of a [`BudgetGuard::watch`] thread. The thread stops when the
/// handle drops.
#[derive(Debug)]
pub struct BudgetWatch {
    done: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for BudgetWatch {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            if thread.join().is_err() {
                tracing::warn!("memory budget watcher panicked");
            }
        }
    }
}
