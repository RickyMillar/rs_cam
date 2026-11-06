//! Feeds contract P0 item 5 (decision D3, lead 2026-11-03): a vendor row
//! whose chart prints a feed band and a spindle speed holds a chip band that
//! meets the chip of that printed feed.
//!
//! A row's `source_page` that prints `a" - b" IPM at N RPM` gives the chip
//! band `[a, b] / (N x flute_count) x 25.4` mm per tooth. A single printed
//! feed (`a" IPM at N RPM`) gives the point `a = b` (feeds contract P16,
//! review D11: the Amana V-groove 2 flute columns print one feed). The row's band
//! `[chipload_min_mm_tooth, chipload_max_mm_tooth]` must overlap it. Report
//! 06 found three Amana ZrN 2F 1/16" rows whose printed chip is twice the
//! chip of the printed feed; decision D3 holds them at the feed band.
//!
//! The test is not vacuous: it asserts the number of rows that print an IPM
//! band (27 on 2026-11-03) before it checks each one.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test every_printed_chip_band_meets_its_printed_feed_fc0
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

/// Millimetres per inch: the definition of the inch.
const MM_PER_INCH: f64 = 25.4;

/// The rows that print `a" - b" IPM at N RPM` or `a" IPM at N RPM`: 27 on
/// 2026-11-03, plus the 9 Amana V-groove rows of P16 (review D11) on
/// 2026-11-06. A new row changes it; a parser that stops matching sets it
/// to 0.
const IPM_BAND_ROWS: usize = 36;

/// The printed feed band (in/min) and spindle speed of a `source_page`
/// that prints `a" - b" IPM at N RPM` or `a" IPM at N RPM`. `None` for
/// any other text.
fn printed_ipm_band(source_page: &str) -> Option<(f64, f64, f64)> {
    const MARK: &str = "\" IPM at ";
    let at = source_page.find(MARK)?;
    let before = &source_page[..at];
    // The last token before the mark is the high (or the only) feed.
    let (head, hi) = before.rsplit_once(' ')?;
    // A band prints `a" - b"`; a single feed prints `a"` alone.
    let lo = match head.strip_suffix(" -") {
        Some(band) => band.rsplit(' ').next()?.trim_end_matches('"'),
        None => hi,
    };
    let rest = &source_page[at + MARK.len()..];
    let rpm = rest.split(" RPM").next()?.replace(',', "");
    Some((lo.parse().ok()?, hi.parse().ok()?, rpm.parse().ok()?))
}

#[test]
fn every_printed_chip_band_meets_its_printed_feed_fc0() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vendor_lut/observations");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the observations folder")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no observation file in {}",
        dir.display()
    );

    let mut checked = 0;
    let mut misses = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        for row in doc["observations"].as_array().expect("observations") {
            let page = row["source_page"].as_str().unwrap_or("");
            let Some((ipm_lo, ipm_hi, rpm)) = printed_ipm_band(page) else {
                continue;
            };
            checked += 1;
            let id = row["observation_id"].as_str().unwrap();
            let flutes = row["flute_count"].as_f64().expect("flute_count");
            let feed_lo = ipm_lo / (rpm * flutes) * MM_PER_INCH;
            let feed_hi = ipm_hi / (rpm * flutes) * MM_PER_INCH;
            let min = row["chipload_min_mm_tooth"].as_f64().expect("min");
            let max = row["chipload_max_mm_tooth"].as_f64().expect("max");
            if min > feed_hi || max < feed_lo {
                misses.push(format!(
                    "{id}: chip band [{min}, {max}] mm misses the printed feed band \
                     [{feed_lo}, {feed_hi}] mm ({ipm_lo}-{ipm_hi} IPM at {rpm} RPM, {flutes} flutes)"
                ));
            }
        }
    }
    assert_eq!(
        checked, IPM_BAND_ROWS,
        "rows that print an IPM band and an RPM (non-vacuity anchor)"
    );
    assert!(
        misses.is_empty(),
        "{} rows ship a chip that their printed feed contradicts:\n{}",
        misses.len(),
        misses.join("\n")
    );
}
