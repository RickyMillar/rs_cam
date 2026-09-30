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

#[test]
fn no_configured_limit_is_a_valid_unlimited_budget() {
    // RULING PENDING: the default fraction is `None`, so the default has no
    // limit. When the ruling lands, this test changes with the constant.
    assert_eq!(super::DEFAULT_SYSTEM_FRACTION, None);
    let budget = MemoryBudget::from_setting(MemoryLimit::Default);
    assert_eq!(budget, MemoryBudget::UNLIMITED);
    assert!(budget.fits(u64::MAX));
    assert_eq!(budget.check(u64::MAX), Ok(()));
    assert_eq!(
        MemoryBudget::from_setting(MemoryLimit::Unlimited),
        MemoryBudget::UNLIMITED
    );
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
