//! Shared text formatting for the UI surfaces.
//!
//! One concept, one implementation. `slugify` lived in `app/export.rs`
//! and in `ui/export_wizard.rs`; `format_cycle` and `format_delta` lived
//! in both Optimize surfaces. A copy per surface can drift, and a drift
//! here shows the operator one filename in the preview and writes
//! another one to disk.

use rs_cam_core::tool_load::optimize::{FullCutCheck, FullCutVerdict, ParamDelta};

/// Make one filename component safe. ASCII alphanumerics and `-`, `_`,
/// `.` survive; every other character becomes `_`.
pub(crate) fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Format cycle time in mm:ss for cycles ≥ 60s, or as "X.Xs" for
/// shorter runs. A non-finite value is not a measurement, so it reads
/// "—".
pub(crate) fn format_cycle(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "—".to_owned();
    }
    if seconds >= 60.0 {
        let minutes = (seconds / 60.0).floor();
        let secs = seconds - 60.0 * minutes;
        format!("{minutes:.0}:{secs:04.1}")
    } else {
        format!("{seconds:.1}s")
    }
}

/// Render a `ParamDelta` as a compact one-liner: "feed 2100, DOC 2.5".
/// The link lever reads "links on" or "links off". Empty (no changes)
/// returns "—".
pub(crate) fn format_delta(delta: &ParamDelta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(f) = delta.feed_mm_min {
        parts.push(format!("feed {f:.0}"));
    }
    if let Some(rpm) = delta.spindle_rpm {
        parts.push(format!("rpm {rpm}"));
    }
    if let Some(s) = delta.stepover_mm {
        parts.push(format!("stepover {s:.2}"));
    }
    if let Some(d) = delta.depth_per_pass_mm {
        parts.push(format!("DOC {d:.2}"));
    }
    match delta.link_moves {
        Some(true) => parts.push("links on".to_owned()),
        Some(false) => parts.push("links off".to_owned()),
        None => {}
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(", ")
    }
}

/// The verdict of a full-cut check as one cell text: "holds",
/// "fails: <reason>" or "not measured: <reason>".
pub(crate) fn format_full_cut_verdict(verdict: &FullCutVerdict) -> String {
    match verdict {
        FullCutVerdict::Holds => "holds".to_owned(),
        FullCutVerdict::Fails { reason } => format!("fails: {reason}"),
        FullCutVerdict::NotMeasured { reason } => format!("not measured: {reason}"),
    }
}

/// The three readings of a full-cut check on one line. A reading that
/// is `None` reads "not measured", never zero.
pub(crate) fn format_full_cut_readings(check: &FullCutCheck) -> String {
    let volume = check
        .removed_volume_mm3
        .map_or_else(|| "not measured".to_owned(), |v| format!("{v:.0} mm³"));
    let collisions = check
        .rapid_collisions
        .map_or_else(|| "not measured".to_owned(), |n| n.to_string());
    let overcut = check
        .overcut_peak_mm
        .map_or_else(|| "not measured".to_owned(), |p| format!("{p:.2} mm"));
    format!("removed volume {volume}; rapid collisions {collisions}; over-cut peak {overcut}")
}

/// Cut a text to `max` characters. A cut text ends with "…".
pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn slugify_replaces_the_space_and_the_separator() {
        assert_eq!(slugify("Job 1/Setup A"), "Job_1_Setup_A");
    }

    #[test]
    fn slugify_keeps_alnum_dash_underscore_and_dot() {
        assert_eq!(slugify("wanaka-front_v2.nc"), "wanaka-front_v2.nc");
    }

    #[test]
    fn slugify_replaces_a_non_ascii_character() {
        assert_eq!(slugify("café"), "caf_");
    }

    #[test]
    fn format_cycle_short_seconds() {
        assert_eq!(format_cycle(12.3), "12.3s");
    }

    #[test]
    fn format_cycle_minutes() {
        assert_eq!(format_cycle(125.0), "2:05.0");
    }

    #[test]
    fn format_cycle_handles_inf() {
        assert_eq!(format_cycle(f64::INFINITY), "—");
    }

    #[test]
    fn format_delta_empty() {
        assert_eq!(format_delta(&ParamDelta::default()), "—");
    }

    #[test]
    fn format_delta_with_feed_and_doc() {
        let delta = ParamDelta {
            feed_mm_min: Some(2100.0),
            depth_per_pass_mm: Some(2.5),
            ..Default::default()
        };
        assert_eq!(format_delta(&delta), "feed 2100, DOC 2.50");
    }

    #[test]
    fn format_delta_names_the_link_lever() {
        let on = ParamDelta {
            link_moves: Some(true),
            ..Default::default()
        };
        assert_eq!(format_delta(&on), "links on");
        let off = ParamDelta {
            depth_per_pass_mm: Some(3.0),
            link_moves: Some(false),
            ..Default::default()
        };
        assert_eq!(format_delta(&off), "DOC 3.00, links off");
    }

    #[test]
    fn format_full_cut_verdict_names_each_kind() {
        assert_eq!(format_full_cut_verdict(&FullCutVerdict::Holds), "holds");
        assert_eq!(
            format_full_cut_verdict(&FullCutVerdict::Fails {
                reason: "more rapid collisions".to_owned()
            }),
            "fails: more rapid collisions"
        );
        assert_eq!(
            format_full_cut_verdict(&FullCutVerdict::NotMeasured {
                reason: "no trace".to_owned()
            }),
            "not measured: no trace"
        );
    }

    /// A `None` reading reads "not measured", and a measured zero reads 0.
    #[test]
    fn format_full_cut_readings_keeps_absence_apart_from_zero() {
        let check = FullCutCheck {
            removed_volume_mm3: Some(1234.4),
            rapid_collisions: Some(0),
            overcut_peak_mm: None,
            verdict: FullCutVerdict::Holds,
        };
        assert_eq!(
            format_full_cut_readings(&check),
            "removed volume 1234 mm³; rapid collisions 0; over-cut peak not measured"
        );
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn truncate_long_appends_ellipsis() {
        let out = truncate("the quick brown fox", 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }
}
