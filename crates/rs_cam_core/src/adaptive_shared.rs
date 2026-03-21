use std::f64::consts::{PI, TAU};

use crate::geo::P2;

/// Compute the target engagement fraction from stepover and tool radius.
pub(crate) fn target_engagement_fraction(stepover: f64, tool_radius: f64) -> f64 {
    let woc = stepover.min(2.0 * tool_radius);
    let alpha = (1.0 - woc / tool_radius).clamp(-1.0, 1.0).acos();
    alpha / TAU
}

/// Average a buffer of angles, handling wraparound correctly.
pub(crate) fn average_angles(angles: &[f64]) -> f64 {
    let mut sx = 0.0;
    let mut sy = 0.0;
    for &angle in angles {
        sx += angle.cos();
        sy += angle.sin();
    }
    sy.atan2(sx)
}

/// Normalize an angle difference to [-π, π].
pub(crate) fn angle_diff(a: f64, b: f64) -> f64 {
    let mut delta = a - b;
    while delta > PI {
        delta -= TAU;
    }
    while delta < -PI {
        delta += TAU;
    }
    delta
}

/// Refine an angle bracket around a target engagement using interpolation.
///
/// The tuple shape is `(angle, engagement, payload)`, where `payload` can carry
/// caller-specific data such as score, z-height, or both.
pub(crate) fn refine_angle_bracket<T: Clone, F>(
    mut lo: (f64, f64, T),
    mut hi: (f64, f64, T),
    target_frac: f64,
    iterations: usize,
    mut eval: F,
) -> Option<(f64, f64, T)>
where
    F: FnMut(f64) -> Option<(f64, f64, T)>,
{
    let mut best = None;
    for _ in 0..iterations {
        let delta = hi.1 - lo.1;
        if delta.abs() <= 0.001 {
            break;
        }
        let t = ((target_frac - lo.1) / delta).clamp(0.0, 1.0);
        let angle = lo.0 + t * angle_diff(hi.0, lo.0);
        let candidate = eval(angle)?;
        if candidate.1 < target_frac {
            lo = candidate.clone();
        } else {
            hi = candidate.clone();
        }
        best = Some(candidate);
    }
    best
}

/// Blend sharp corners in a path with arcs of at least `min_radius`.
pub(crate) fn blend_corners(path: &[P2], min_radius: f64) -> Vec<P2> {
    if min_radius <= 0.0 || path.len() < 3 {
        return path.to_vec();
    }

    let mut result = vec![path[0]];

    for i in 1..path.len() - 1 {
        let a = path[i - 1];
        let b = path[i];
        let c = path[i + 1];

        let ba_x = a.x - b.x;
        let ba_y = a.y - b.y;
        let bc_x = c.x - b.x;
        let bc_y = c.y - b.y;
        let ba_len = (ba_x * ba_x + ba_y * ba_y).sqrt();
        let bc_len = (bc_x * bc_x + bc_y * bc_y).sqrt();

        if ba_len < 1e-10 || bc_len < 1e-10 {
            result.push(b);
            continue;
        }

        let cos_full = (ba_x * bc_x + ba_y * bc_y) / (ba_len * bc_len);
        let cos_full = cos_full.clamp(-1.0, 1.0);
        let full_angle = cos_full.acos();
        let half = full_angle / 2.0;

        if full_angle > 170.0_f64.to_radians() || half < 0.02 {
            result.push(b);
            continue;
        }

        let setback = min_radius / half.tan();
        if setback > ba_len * 0.4 || setback > bc_len * 0.4 {
            result.push(b);
            continue;
        }

        let t1 = P2::new(b.x + ba_x / ba_len * setback, b.y + ba_y / ba_len * setback);
        let t2 = P2::new(b.x + bc_x / bc_len * setback, b.y + bc_y / bc_len * setback);

        let bis_x = ba_x / ba_len + bc_x / bc_len;
        let bis_y = ba_y / ba_len + bc_y / bc_len;
        let bis_len = (bis_x * bis_x + bis_y * bis_y).sqrt();
        if bis_len < 1e-10 {
            result.push(b);
            continue;
        }
        let center_dist = min_radius / half.sin();
        let arc_cx = b.x + bis_x / bis_len * center_dist;
        let arc_cy = b.y + bis_y / bis_len * center_dist;

        let a1 = (t1.y - arc_cy).atan2(t1.x - arc_cx);
        let a2 = (t2.y - arc_cy).atan2(t2.x - arc_cx);

        let mut sweep = a2 - a1;
        if sweep > PI {
            sweep -= TAU;
        }
        if sweep < -PI {
            sweep += TAU;
        }

        let n_pts = ((sweep.abs() / (PI / 18.0)).ceil() as usize).clamp(2, 20);
        result.push(t1);
        for j in 1..n_pts {
            let t = j as f64 / n_pts as f64;
            let angle = a1 + sweep * t;
            result.push(P2::new(
                arc_cx + min_radius * angle.cos(),
                arc_cy + min_radius * angle.sin(),
            ));
        }
        result.push(t2);
    }

    result.push(*path.last().expect("path has at least 3 elements"));
    result
}
