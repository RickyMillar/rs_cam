// SAFETY: test module; a failed unwrap is a failed test.
#![allow(clippy::unwrap_used)]

use super::{ByteSizeError, MemoryBudget, MemoryLimit, StopReason, format_bytes, parse_byte_size};

const GIB: u64 = 1 << 30;

#[test]
fn a_byte_size_parses_binary_units_and_bare_bytes() {
    assert_eq!(parse_byte_size("12GiB").unwrap(), 12 * GIB);
    assert_eq!(parse_byte_size(" 12 gib ").unwrap(), 12 * GIB);
    assert_eq!(parse_byte_size("512MiB").unwrap(), 512 << 20);
    assert_eq!(parse_byte_size("1.5GiB").unwrap(), 3 * GIB / 2);
    assert_eq!(parse_byte_size("4096").unwrap(), 4096);
    assert_eq!(parse_byte_size("7B").unwrap(), 7);
    assert_eq!(parse_byte_size("2TiB").unwrap(), 2 << 40);
}

#[test]
fn a_byte_size_refuses_decimal_units_and_junk() {
    assert_eq!(parse_byte_size(""), Err(ByteSizeError::Empty));
    assert_eq!(
        parse_byte_size("12GB"),
        Err(ByteSizeError::UnknownUnit("GB".to_owned()))
    );
    assert!(matches!(
        parse_byte_size("GiB"),
        Err(ByteSizeError::BadNumber(_))
    ));
    assert!(matches!(
        parse_byte_size("1.2.3GiB"),
        Err(ByteSizeError::BadNumber(_))
    ));
    assert!(matches!(
        parse_byte_size("-1GiB"),
        Err(ByteSizeError::UnknownUnit(_))
    ));
    assert!(matches!(
        parse_byte_size("99999999999TiB"),
        Err(ByteSizeError::TooLarge(_))
    ));
}

#[test]
fn format_bytes_picks_the_largest_unit() {
    assert_eq!(format_bytes(12 * GIB), "12.00 GiB");
    assert_eq!(format_bytes(1536 << 20), "1.50 GiB");
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

/// Operator ruling 2026-10-02: the default is half of the system RAM. This
/// test replaces `no_configured_limit_is_a_valid_unlimited_budget`, which
/// pinned the pending `None`.
#[test]
fn the_default_is_half_of_a_given_total() {
    assert_eq!(super::DEFAULT_SYSTEM_FRACTION, Some(0.5));
    assert_eq!(
        MemoryBudget::from_setting_on(MemoryLimit::Default, Some(64 * GIB)),
        MemoryBudget::with_limit(32 * GIB)
    );
    // An odd byte count floors.
    assert_eq!(
        MemoryBudget::from_setting_on(MemoryLimit::Default, Some(9)),
        MemoryBudget::with_limit(4)
    );
    // No total from the platform: the default has no limit, and that is a
    // valid state, not an error.
    let budget = MemoryBudget::from_setting_on(MemoryLimit::Default, None);
    assert_eq!(budget, MemoryBudget::UNLIMITED);
    assert!(budget.fits(u64::MAX));
    assert_eq!(budget.check(u64::MAX), Ok(()));
    // The total does not change an explicit choice.
    assert_eq!(
        MemoryBudget::from_setting_on(MemoryLimit::Unlimited, Some(64 * GIB)),
        MemoryBudget::UNLIMITED
    );
    assert_eq!(
        MemoryBudget::from_setting_on(MemoryLimit::Bytes(3 * GIB), Some(64 * GIB)),
        MemoryBudget::with_limit(3 * GIB)
    );
}

/// The real path reads the total from `sysinfo`. Two reads in one process
/// agree, so the default equals half of that total.
#[test]
fn the_default_on_this_machine_is_half_of_the_system_memory() {
    assert_eq!(
        MemoryBudget::from_setting(MemoryLimit::Default),
        MemoryBudget::from_setting_on(MemoryLimit::Default, super::system_memory_bytes())
    );
    assert_eq!(
        MemoryBudget::from_setting(MemoryLimit::Unlimited),
        MemoryBudget::UNLIMITED
    );
}

#[test]
fn an_exact_size_reads_back_to_the_same_value() {
    assert_eq!(super::format_exact_size(24 * GIB), "24GiB");
    assert_eq!(super::format_exact_size(3 * GIB / 2), "1536MiB");
    assert_eq!(super::format_exact_size(7), "7B");
    assert_eq!(super::format_exact_size(0), "0B");
    for bytes in [
        0,
        1,
        7,
        1023,
        1024,
        4096,
        3 * GIB / 2,
        24 * GIB,
        2 << 40,
        12_345_678,
    ] {
        let text = super::format_exact_size(bytes);
        assert_eq!(parse_byte_size(&text).unwrap(), bytes, "{text}");
    }
}

#[test]
fn a_configured_limit_refuses_what_does_not_fit() {
    let budget = MemoryBudget::from_setting(MemoryLimit::Bytes(8 * GIB));
    assert_eq!(budget.limit_bytes, Some(8 * GIB));
    assert!(budget.fits(8 * GIB));
    assert!(!budget.fits(8 * GIB + 1));
    assert_eq!(
        budget.check(9 * GIB),
        Err(StopReason::OverBudget {
            need_bytes: 9 * GIB,
            limit_bytes: 8 * GIB,
        })
    );
}

#[test]
fn a_fraction_of_the_total_floors_and_rejects_nonsense() {
    assert_eq!(
        MemoryBudget::from_fraction_of(16 * GIB, 0.5),
        MemoryBudget::with_limit(8 * GIB)
    );
    assert_eq!(
        MemoryBudget::from_fraction_of(16 * GIB, 1.0),
        MemoryBudget::with_limit(16 * GIB)
    );
    for bad in [0.0, -0.5, 1.5, f64::NAN, f64::INFINITY] {
        assert_eq!(
            MemoryBudget::from_fraction_of(16 * GIB, bad),
            MemoryBudget::UNLIMITED,
            "fraction {bad}"
        );
    }
    assert_eq!(
        MemoryBudget::from_fraction_of(0, 0.5),
        MemoryBudget::UNLIMITED
    );
}

#[test]
fn a_stop_reason_names_both_sizes() {
    let text = StopReason::OverBudget {
        need_bytes: 24 * GIB,
        limit_bytes: 12 * GIB,
    }
    .to_string();
    assert!(text.contains("24.00 GiB"), "{text}");
    assert!(text.contains("12.00 GiB"), "{text}");
}
