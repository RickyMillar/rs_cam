//! The unit a person reads a tool size in: millimetres or inches.
//!
//! A DISPLAY and ENTRY convention only. Every stored and computed length
//! stays in millimetres. A tool sold in inches ("1/4\" Downcut") is shown
//! in inches, with the millimetre figure beside it, so its label reads the
//! way the vendor sells it (operator ruling, 2026-09-24: "if it's in
//! imperial, we just need to keep that convention").

use serde::{Deserialize, Serialize};

/// Millimetres per inch.
pub const MM_PER_INCH: f64 = 25.4;

/// The finest inch fraction a label prints: 1/64".
const FRACTION_DENOMINATOR: u64 = 64;

/// A length reads as a fraction only within this share of it.
const FRACTION_REL_TOL: f64 = 0.005;

/// A length reads as a metric size when it sits within this share of a
/// whole half millimetre. Tight on purpose: 25.4 mm (1") is 0.39 % from
/// 25.5 mm and must not read as metric.
const HALF_MM_REL_TOL: f64 = 0.001;

/// The unit a tool size is shown and entered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeUnits {
    Metric,
    Imperial,
}

impl SizeUnits {
    pub const ALL: &[SizeUnits] = &[SizeUnits::Metric, SizeUnits::Imperial];

    /// The short label a selector shows.
    pub const fn label(self) -> &'static str {
        match self {
            SizeUnits::Metric => "mm",
            SizeUnits::Imperial => "inch",
        }
    }

    /// The snake_case token of the project file and the MCP wire.
    pub const fn serde_token(self) -> &'static str {
        match self {
            SizeUnits::Metric => "metric",
            SizeUnits::Imperial => "imperial",
        }
    }

    /// Case-insensitive parse of the serde token. `None` for any other
    /// text.
    pub fn parse_lenient(s: &str) -> Option<SizeUnits> {
        let lower = s.trim().to_ascii_lowercase();
        SizeUnits::ALL
            .iter()
            .copied()
            .find(|u| u.serde_token() == lower)
    }
}

/// The nearest 1/64" fraction of `mm`, when `mm` is within 0.5 % of it,
/// as `(whole inches, numerator, denominator)` in lowest terms. `None`
/// for a length that is not near a 1/64" step.
fn nearest_fraction(mm: f64) -> Option<(u64, u64, u64)> {
    let inches = mm / MM_PER_INCH;
    if !inches.is_finite() || inches <= 0.0 {
        return None;
    }
    let steps = (inches * FRACTION_DENOMINATOR as f64).round();
    if steps < 1.0 {
        return None;
    }
    let snapped = steps / FRACTION_DENOMINATOR as f64;
    if (snapped - inches).abs() > FRACTION_REL_TOL * inches {
        return None;
    }
    let steps = steps as u64;
    let divisor = gcd(steps, FRACTION_DENOMINATOR);
    let (num, den) = (steps / divisor, FRACTION_DENOMINATOR / divisor);
    Some((num / den, num % den, den))
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `mm` in inches: the nearest 1/64" fraction when it is within 0.5 %
/// (`1/4"`, `1-1/4"`, `1"`), else decimal inches (`0.236"`).
pub fn inch_text(mm: f64) -> String {
    match nearest_fraction(mm) {
        Some((whole, 0, _)) => format!("{whole}\""),
        Some((0, num, den)) => format!("{num}/{den}\""),
        Some((whole, num, den)) => format!("{whole}-{num}/{den}\""),
        None => format!("{:.3}\"", mm / MM_PER_INCH),
    }
}

/// Does `mm` look like an inch size rather than a metric size? True when
/// it is near a 1/64" step and NOT near a whole half millimetre: 6.35 and
/// 3.175 are inch sizes, 6.0 and 2.0 are metric. The caution line uses
/// this to print the tool's figure in the unit it was most likely sold in.
pub fn looks_imperial(mm: f64) -> bool {
    let half_mm = (mm * 2.0).round() / 2.0;
    let near_half_mm = (mm - half_mm).abs() <= HALF_MM_REL_TOL * mm.abs();
    nearest_fraction(mm).is_some() && !near_half_mm
}

/// The text an entry field shows for a length of `mm` in `units`: the
/// inch text in [`SizeUnits::Imperial`], else millimetres to three
/// decimals with no unit (the field carries the ` mm` suffix).
pub fn entry_text(mm: f64, units: SizeUnits) -> String {
    match units {
        SizeUnits::Imperial => inch_text(mm),
        SizeUnits::Metric => format!("{mm:.3}"),
    }
}

/// Parse a length typed into an entry field, in millimetres.
///
/// Accepts a decimal (`6.35`, `0.25`), a fraction (`1/4`), a mixed
/// fraction (`1-1/4`, `1 1/4`), each with an optional unit suffix: `mm`,
/// `"`, `”`, `in`, `inch` or `inches`. A figure with no suffix is read in
/// `units`. `None` for anything else, and for a length that is not
/// positive and finite.
pub fn parse_length_entry(text: &str, units: SizeUnits) -> Option<f64> {
    let lower = text.trim().to_ascii_lowercase();
    let (body, unit) = if let Some(rest) = lower.strip_suffix("mm") {
        (rest, SizeUnits::Metric)
    } else if let Some(rest) = ["inches", "inch", "in", "\"", "\u{201D}"]
        .iter()
        .find_map(|suffix| lower.strip_suffix(suffix))
    {
        (rest, SizeUnits::Imperial)
    } else {
        (lower.as_str(), units)
    };
    let value = parse_number_or_fraction(body.trim())?;
    let mm = match unit {
        SizeUnits::Metric => value,
        SizeUnits::Imperial => value * MM_PER_INCH,
    };
    (mm.is_finite() && mm > 0.0).then_some(mm)
}

/// `6.35`, `1/4`, `1-1/4` or `1 1/4` as a number.
fn parse_number_or_fraction(body: &str) -> Option<f64> {
    if let Ok(v) = body.parse::<f64>() {
        return Some(v);
    }
    let (whole, fraction) = match body.rsplit_once(['-', ' ']) {
        Some((whole, fraction)) if !whole.trim().is_empty() => {
            (whole.trim().parse::<f64>().ok()?, fraction)
        }
        _ => (0.0, body),
    };
    let (num, den) = fraction.split_once('/')?;
    let num = num.trim().parse::<f64>().ok()?;
    let den = den.trim().parse::<f64>().ok().filter(|d| *d > 0.0)?;
    Some(whole + num / den)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// The common router sizes print as the fraction they are sold as.
    #[test]
    fn common_fractions_print_as_fractions() {
        let cases: &[(f64, &str)] = &[
            (6.35, "1/4\""),
            (3.175, "1/8\""),
            (1.5875, "1/16\""),
            (9.525, "3/8\""),
            (12.7, "1/2\""),
            (25.4, "1\""),
            (31.75, "1-1/4\""),
            (0.79375, "1/32\""),
        ];
        for &(mm, text) in cases {
            assert_eq!(inch_text(mm), text, "{mm} mm");
        }
    }

    /// A size that is not near a 1/64" step prints as decimal inches.
    #[test]
    fn other_sizes_print_as_decimal_inches() {
        assert_eq!(inch_text(6.0), "0.236\"");
        assert_eq!(inch_text(2.0), "0.079\"");
    }

    #[test]
    fn inch_sizes_are_told_from_metric_sizes() {
        for mm in [6.35, 3.175, 1.5875, 9.525, 12.7, 25.4] {
            assert!(looks_imperial(mm), "{mm} mm should read as inches");
        }
        for mm in [6.0, 2.0, 1.0, 3.0, 4.0, 8.0, 10.0, 12.0] {
            assert!(!looks_imperial(mm), "{mm} mm should read as metric");
        }
    }

    #[test]
    fn entry_text_parses_back() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let imperial = SizeUnits::Imperial;
        let metric = SizeUnits::Metric;
        assert!(close(parse_length_entry("1/4", imperial).unwrap(), 6.35));
        assert!(close(parse_length_entry("1/4\"", metric).unwrap(), 6.35));
        assert!(close(parse_length_entry("0.25in", metric).unwrap(), 6.35));
        assert!(close(parse_length_entry("1-1/4\"", metric).unwrap(), 31.75));
        assert!(close(
            parse_length_entry("1 1/4 inch", metric).unwrap(),
            31.75
        ));
        assert!(close(parse_length_entry("6", metric).unwrap(), 6.0));
        assert!(close(parse_length_entry("6mm", imperial).unwrap(), 6.0));
        assert!(close(parse_length_entry("0.25", imperial).unwrap(), 6.35));
        assert!(parse_length_entry("", metric).is_none());
        assert!(parse_length_entry("abc", metric).is_none());
        assert!(parse_length_entry("1/0", imperial).is_none());
        assert!(parse_length_entry("-2", metric).is_none());
        for mm in [6.35, 3.175, 12.7, 6.0] {
            let shown = entry_text(mm, imperial);
            let back = parse_length_entry(&shown, imperial).unwrap();
            assert!((back - mm).abs() <= 0.005 * mm, "{mm} -> {shown} -> {back}");
        }
    }

    #[test]
    fn tokens_round_trip() {
        for &u in SizeUnits::ALL {
            assert_eq!(SizeUnits::parse_lenient(u.serde_token()), Some(u));
            assert_eq!(
                serde_json::to_value(u).unwrap(),
                serde_json::Value::String(u.serde_token().to_owned())
            );
        }
        assert_eq!(
            SizeUnits::parse_lenient("IMPERIAL"),
            Some(SizeUnits::Imperial)
        );
        assert_eq!(SizeUnits::parse_lenient("inch"), None);
    }
}
