//! Small crate-wide helpers that no layer owns.
//!
//! [`panic_message`] classifies a caught panic payload. [`build_info`]
//! reports the crate identity that the product surfaces print. [`steps`]
//! turns a length and a step into a checked count (F195).

pub mod build_info;
pub mod panic_message;
pub mod steps;
