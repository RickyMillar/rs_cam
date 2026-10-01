//! The `[memory] limit` vocabulary of the app settings file (B5).
//!
//! The settings file itself (the path, the loader and the ONE writer) is
//! `crate::settings`. This module owns only the text of the memory limit,
//! because the CLI flag `--memory-limit` reads the same text:
//!
//! ```toml
//! [memory]
//! limit = "12GiB"      # a binary size: B, KiB, MiB, GiB or TiB
//! # limit = 12884901888  (a bare integer is a byte count)
//! # limit = "unlimited"  (no limit, whatever the default is)
//! # limit = "default"    (the default: half of the system memory)
//! ```
//!
//! No `limit` key, or `"default"`, means the default: [`MemoryLimit::Default`],
//! which reads [`super::DEFAULT_SYSTEM_FRACTION`] (operator ruling
//! 2026-10-02: half of the system RAM).
//!
//! A surface flag overrides the file: [`resolve_limit`]. The CLI flag
//! `--memory-limit` is the one such flag today.

use super::{ByteSizeError, MemoryLimit, parse_byte_size};

/// The text that asks for no limit, in the file and in the CLI flag.
pub const UNLIMITED: &str = "unlimited";

/// The text that asks for the default limit (half of the system memory), in
/// the file and in the CLI flag.
pub const DEFAULT: &str = "default";

/// Parse one limit as text: a binary size (`"12GiB"`), a byte count
/// (`"4096"`), [`UNLIMITED`] or [`DEFAULT`], in any case. The file's text
/// value and the CLI flag both read through this function.
///
/// # Errors
/// The [`ByteSizeError`] of a text that is not a size.
pub fn parse_limit_text(text: &str) -> Result<MemoryLimit, ByteSizeError> {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case(UNLIMITED) {
        return Ok(MemoryLimit::Unlimited);
    }
    if trimmed.eq_ignore_ascii_case(DEFAULT) {
        return Ok(MemoryLimit::Default);
    }
    parse_byte_size(text).map(MemoryLimit::Bytes)
}

/// The text that `crate::settings::save_to` writes for `limit`, and that
/// [`parse_limit_text`] reads back to the same value: [`DEFAULT`],
/// [`UNLIMITED`], or an exact binary size such as `"24GiB"`.
#[must_use]
pub fn limit_text(limit: MemoryLimit) -> String {
    match limit {
        MemoryLimit::Default => DEFAULT.to_owned(),
        MemoryLimit::Unlimited => UNLIMITED.to_owned(),
        MemoryLimit::Bytes(bytes) => super::format_exact_size(bytes),
    }
}

/// The limit that applies: the surface flag when it is given, else the
/// file's value. A flag of [`MemoryLimit::Default`] is still a flag.
#[must_use]
pub fn resolve_limit(flag: Option<MemoryLimit>, file: MemoryLimit) -> MemoryLimit {
    flag.unwrap_or(file)
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap is a failed test.
    #![allow(clippy::unwrap_used)]

    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn the_limit_text_reads_back_to_the_same_limit() {
        for limit in [
            MemoryLimit::Default,
            MemoryLimit::Unlimited,
            MemoryLimit::Bytes(0),
            MemoryLimit::Bytes(4097),
            MemoryLimit::Bytes(24 * GIB),
        ] {
            assert_eq!(parse_limit_text(&limit_text(limit)).unwrap(), limit);
        }
        assert_eq!(limit_text(MemoryLimit::Bytes(24 * GIB)), "24GiB");
        assert_eq!(limit_text(MemoryLimit::Default), "default");
    }

    #[test]
    fn the_flag_text_and_the_file_text_parse_the_same_way() {
        for text in ["12GiB", "4096", "unlimited", " UNLIMITED ", "default"] {
            let file = crate::settings::parse(&format!("[memory]\nlimit = \"{text}\"\n"))
                .unwrap()
                .memory_limit;
            assert_eq!(parse_limit_text(text).unwrap(), file, "{text}");
        }
        assert!(parse_limit_text("12GB").is_err());
        assert!(parse_limit_text("lots").is_err());
    }
}
