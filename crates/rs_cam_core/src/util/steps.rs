//! Checked step counts: how many steps of one length cover another (F195).
//!
//! A bare `(len / step).ceil() as usize` saturates in silence. A step of 0
//! gives `inf`, which becomes `usize::MAX`, and a `+ 1` on that count then
//! overflows. A NaN becomes 0. These helpers give the same count as the
//! bare cast for every input that the cast represents, and `None` for an
//! input that it does not.
//!
//! The folders that turn a user value into a count (`ops/`, `dressup/`,
//! `compute/execute/`) deny `clippy::cast_possible_truncation` and
//! `clippy::cast_sign_loss`, so a new count there goes through this module
//! or states its bound in an `#[expect]` reason.

/// The number of steps of length `step` that cover `len`: `ceil(len / step)`.
///
/// A negative `len` gives `Some(0)`, as the bare cast does. The result is
/// `None` when `step` is not a finite positive number, when `len` is NaN,
/// or when the count does not fit a `usize`.
#[must_use]
pub fn steps(len: f64, step: f64) -> Option<usize> {
    count(len, step, f64::ceil)
}

/// The number of whole steps of length `step` that fit in `len`:
/// `floor(len / step)`.
///
/// The same refusals as [`steps`].
#[must_use]
pub fn whole_steps(len: f64, step: f64) -> Option<usize> {
    count(len, step, f64::floor)
}

fn count(len: f64, step: f64, round: fn(f64) -> f64) -> Option<usize> {
    if !(step.is_finite() && step > 0.0) || len.is_nan() {
        return None;
    }
    let n = round(len / step);
    // `usize::MAX as f64` rounds up to 2^64, so `n < limit` fits a usize.
    if !n.is_finite() || n >= usize::MAX as f64 {
        return None;
    }
    if n <= 0.0 {
        return Some(0);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "n is finite, positive and below 2^64 (checked above), so it fits a usize"
    )]
    Some(n as usize)
}

#[cfg(test)]
mod tests {
    use super::{steps, whole_steps};

    #[test]
    fn a_count_matches_the_bare_cast_where_the_cast_is_defined() {
        assert_eq!(steps(10.0, 3.0), Some(4));
        assert_eq!(steps(9.0, 3.0), Some(3));
        assert_eq!(steps(0.0, 3.0), Some(0));
        assert_eq!(steps(-5.0, 3.0), Some(0));
        assert_eq!(whole_steps(10.0, 3.0), Some(3));
        assert_eq!(whole_steps(2.0, 3.0), Some(0));
    }

    #[test]
    fn a_step_that_is_not_a_finite_positive_number_gives_no_count() {
        assert_eq!(steps(10.0, 0.0), None);
        assert_eq!(steps(10.0, -1.0), None);
        assert_eq!(steps(10.0, f64::NAN), None);
        assert_eq!(steps(10.0, f64::INFINITY), None);
        assert_eq!(steps(f64::NAN, 1.0), None);
        assert_eq!(steps(f64::INFINITY, 1.0), None);
        assert_eq!(steps(1e300, 1e-300), None);
        assert_eq!(whole_steps(10.0, 0.0), None);
    }
}
