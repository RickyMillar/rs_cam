//! Shared text formatting for the UI surfaces.
//!
//! One concept, one implementation. `slugify` lived in `app/export.rs`
//! and in `ui/export_wizard.rs`. A copy per surface can drift, and a drift
//! here shows the operator one filename in the preview and writes
//! another one to disk.

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

#[cfg(test)]
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
}
