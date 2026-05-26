//! F-036a regression net — G-code emitter per-move F-word emission.
//!
//! F-036 (`crates/rs_cam_core/src/feed_modulation.rs`) introduced an
//! adaptive feed modulator that writes per-move `feed_rate` values into
//! the IR. F-036a's job is to make sure the G-code emitter actually
//! propagates that per-move variation as per-move `F<rate>` words — and,
//! critically, that the modal F-elision still collapses runs of equal
//! feeds into one `F<rate>` (otherwise every previously-quiet toolpath
//! would spam an F-word on every line and the F-037 smoke baseline
//! would diverge byte-for-byte).
//!
//! These three tests pin both halves of that contract:
//!
//! 1. `modulated_toolpath_emits_per_move_f_words` — three Linear moves
//!    at distinct feeds (1000 / 1500 / 2000) → three distinct F-words
//!    in the emitted G-code.
//! 2. `unmodulated_toolpath_emits_single_f_word` — three Linear moves
//!    all at the same feed (1500) → exactly ONE F-word in the emitted
//!    G-code. This is the byte-identical invariant that protects the
//!    F-037 smoke baseline.
//! 3. `modulated_gcode_round_trips_through_parser` — modulated path →
//!    emit → re-extract feeds from emitted text → reconstructed feeds
//!    match the IR's per-move feeds within 1 mm/min. (`rs_cam_core`
//!    has no public G-code parser; the round-trip uses a regex on
//!    `G1 ... F<rate>` lines, per the finding's documented workaround.)
//!
//! See `planning/acceptance_loop/findings/F-036a-feed-modulation-gcode-emission.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::{emit_gcode, post};
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::Toolpath;

/// Count occurrences of standalone F-words (e.g. `F1500`) anywhere on
/// any line of the emitted G-code. Uses a hand-rolled scanner rather
/// than pulling in a regex crate: tokens are split on whitespace, and a
/// token qualifies if it starts with `F` followed by a parseable
/// floating-point number (covers `F1500`, `F1500.0`, `F600.5`).
fn collect_f_words(gcode: &str) -> Vec<f64> {
    let mut feeds = Vec::new();
    for line in gcode.lines() {
        // Skip lines that the post may emit as comments. Both Grbl
        // (`(...)`) and LinuxCNC (`;...`) comment styles are tolerated.
        let trimmed = line.trim_start();
        if trimmed.starts_with('(') || trimmed.starts_with(';') {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(rest) = token.strip_prefix('F')
                && let Ok(n) = rest.parse::<f64>()
            {
                feeds.push(n);
            }
        }
    }
    feeds
}

/// Three distinct cutting feeds → three distinct F-words. The
/// emitter's modal F-elision must NOT collapse them, because the
/// per-move feeds differ.
#[test]
fn modulated_toolpath_emits_per_move_f_words() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -1.0), 1000.0);
    tp.feed_to(P3::new(20.0, 0.0, -1.0), 1500.0);
    tp.feed_to(P3::new(30.0, 0.0, -1.0), 2000.0);

    let gcode = emit_gcode(&tp, post::grbl(), 18_000);
    let feeds = collect_f_words(&gcode);

    assert_eq!(
        feeds.len(),
        3,
        "expected 3 F-words for 3 distinct modulated feeds, got {feeds:?}\n--- emitted ---\n{gcode}",
    );
    assert!(
        (feeds[0] - 1000.0).abs() < 1.0,
        "first F-word should be ~1000, got {}",
        feeds[0]
    );
    assert!(
        (feeds[1] - 1500.0).abs() < 1.0,
        "second F-word should be ~1500, got {}",
        feeds[1]
    );
    assert!(
        (feeds[2] - 2000.0).abs() < 1.0,
        "third F-word should be ~2000, got {}",
        feeds[2]
    );
}

/// Three cutting moves at the same feed → exactly ONE F-word (modal
/// suppression unchanged). This is the byte-identical invariant that
/// protects the F-037 smoke baseline: if this regresses, every
/// non-modulated toolpath in the smoke corpus would start emitting an
/// F-word on every line, the smoke captures would drift, and the
/// "no regressions" gate would fail.
#[test]
fn unmodulated_toolpath_emits_single_f_word() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -1.0), 1500.0);
    tp.feed_to(P3::new(20.0, 0.0, -1.0), 1500.0);
    tp.feed_to(P3::new(30.0, 0.0, -1.0), 1500.0);

    let gcode = emit_gcode(&tp, post::grbl(), 18_000);
    let feeds = collect_f_words(&gcode);

    assert_eq!(
        feeds.len(),
        1,
        "expected exactly 1 F-word for 3 identical feeds (modal suppression), got {feeds:?}\n--- emitted ---\n{gcode}",
    );
    assert!(
        (feeds[0] - 1500.0).abs() < 1.0,
        "single F-word should be ~1500, got {}",
        feeds[0]
    );
}

/// Round-trip: modulated IR → G-code text → extracted F-words → match
/// the IR's per-move feeds within 1 mm/min. Documents end-to-end that
/// `feed_rate` modulation reaches the controller as a sequence of
/// F-words a downstream parser (or operator scrolling the file) can
/// read back.
///
/// Uses the same `collect_f_words` regex-style scanner as the other
/// two tests because `rs_cam_core` does not expose a G-code parser
/// (per the finding's "workaround" note).
#[test]
fn modulated_gcode_round_trips_through_parser() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    // Per-move feeds the modulator would plausibly produce on a
    // varying-engagement path: gentle ramp up + a corner deceleration.
    let commanded_feeds = [1200.0, 1800.0, 2400.0, 900.0, 1500.0];
    let mut x = 0.0;
    for &f in &commanded_feeds {
        x += 10.0;
        tp.feed_to(P3::new(x, 0.0, -1.0), f);
    }

    let gcode = emit_gcode(&tp, post::grbl(), 18_000);
    let observed = collect_f_words(&gcode);

    assert_eq!(
        observed.len(),
        commanded_feeds.len(),
        "expected {} F-words round-tripped, got {} ({observed:?})\n--- emitted ---\n{gcode}",
        commanded_feeds.len(),
        observed.len(),
    );
    for (i, (&commanded, &emitted)) in commanded_feeds.iter().zip(observed.iter()).enumerate() {
        assert!(
            (commanded - emitted).abs() <= 1.0,
            "move {i}: commanded {commanded}, emitted {emitted} (delta > 1 mm/min)",
        );
    }
}
