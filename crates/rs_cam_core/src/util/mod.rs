//! Small crate-wide helpers that no layer owns.
//!
//! [`panic_message`] classifies a caught panic payload. [`build_info`]
//! reports the crate identity that the product surfaces print.

pub mod build_info;
pub mod panic_message;
