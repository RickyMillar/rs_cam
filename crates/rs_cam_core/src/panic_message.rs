//! Best-effort message extraction from a `std::panic::catch_unwind` payload.
//!
//! Three places in this crate contain a panic rather than let it kill a
//! worker thread — the tool-load candidate evaluator and the two
//! `cavalier_contours` offset chokepoints — and all three want to say WHICH
//! panic they contained. `catch_unwind` hands back a `Box<dyn Any + Send>`,
//! so the message has to be downcast out of it.
//!
//! # What this cannot recover, and why the caller must not pretend otherwise
//!
//! **The source location is not in the payload.** `panic!`'s file/line lives
//! on `PanicHookInfo`, which only the process-global panic hook sees. A
//! library primitive called from parallel worker threads must not install
//! one, so a contained panic here is identified by its assertion TEXT and
//! nothing else. The R2 census (`tests/cavalier_shape_failure_r2.rs`) does
//! install a hook and does report locations — it is `#[ignore]`d and
//! single-threaded for exactly that reason.

/// Best-effort human-readable message from a `catch_unwind` payload.
///
/// `panic!("...")` yields `&str`; `panic!("{x}")`-style formatting yields
/// `String`; anything else gets a placeholder rather than a lie.
pub(crate) fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}
