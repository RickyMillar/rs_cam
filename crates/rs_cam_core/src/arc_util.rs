//! Arc linearization utility shared by simulation modules.

use crate::geo::P3;

/// Linearize a circular arc into a sequence of 3D points.
///
/// The arc goes from `start` to `end` with center offset (i, j) relative to
/// `start`. Z is interpolated linearly. `clockwise` selects CW vs CCW sweep.
pub fn linearize_arc(
    start: P3,
    end: P3,
    i: f64,
    j: f64,
    clockwise: bool,
    max_seg_len: f64,
) -> Vec<P3> {
    let cx = start.x + i;
    let cy = start.y + j;
    let r = (i * i + j * j).sqrt();

    if r < 1e-10 {
        return vec![start, end];
    }

    let start_angle = (start.y - cy).atan2(start.x - cx);
    let end_angle = (end.y - cy).atan2(end.x - cx);

    let mut sweep = if clockwise {
        start_angle - end_angle
    } else {
        end_angle - start_angle
    };
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }

    let arc_len = r * sweep;
    const MAX_ARC_SAMPLES: usize = 100_000;
    let samples = (arc_len / max_seg_len).ceil().max(2.0) as usize;
    let samples = samples.min(MAX_ARC_SAMPLES);

    let mut points = Vec::with_capacity(samples + 1);
    for s in 0..=samples {
        let t = s as f64 / samples as f64;
        let angle = if clockwise {
            start_angle - t * sweep
        } else {
            start_angle + t * sweep
        };
        let (sin_a, cos_a) = angle.sin_cos();
        let x = cx + r * cos_a;
        let y = cy + r * sin_a;
        let z = start.z + t * (end.z - start.z);
        points.push(P3::new(x, y, z));
    }
    points
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
mod tests {
    use super::*;

    #[test]
    fn linearize_arc_semicircle() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 0.0);
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);

        for p in &points {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 5.0).abs() < 0.05, "r = {r:.3}");
        }
        let last = points.last().unwrap();
        assert!((last.x - end.x).abs() < 0.1);
    }

    #[test]
    fn linearize_arc_z_interpolation() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 10.0);
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);
        assert!((points.first().unwrap().z).abs() < 1e-10);
        assert!((points.last().unwrap().z - 10.0).abs() < 0.1);
    }
}
