//! V-carving toolpath generation.
//!
//! Produces variable-depth toolpaths for V-bit engraving. The tool depth
//! at each point equals the distance to the nearest polygon boundary
//! divided by tan(half_angle), creating a V-groove that exactly meets
//! the design outline.
//!
//! Uses scan-line sampling: zigzag lines are generated across the polygon,
//! and for each sample point the exact Euclidean distance to the nearest
//! boundary edge determines the cut depth.
//!
//! Reference: research/02_algorithms.md §11

use crate::edge_distance::EdgeDistanceField;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Parameters for V-carve toolpath generation.
pub struct VCarveParams {
    /// V-bit half-angle in radians (e.g. π/4 for a 90° V-bit).
    pub half_angle: f64,
    /// Maximum cut depth in mm (positive). Clamps depth for wide areas.
    /// If 0.0 or negative, depth is unlimited (no clamping).
    pub max_depth: f64,
    /// Distance between scan lines in mm.
    pub stepover: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Sampling interval along each scan line in mm.
    pub tolerance: f64,
    /// Stock-top Z in the emission frame (F-028 / S.2). V-carve cuts at
    /// `top_z - depth`. Pre-S.2 this was implicitly `0.0` (v-carve
    /// hardcoded `z = -depth`), which only produced cuts inside the stock
    /// when the world stock top happened to sit at world Z=0. Callers
    /// should pass `heights.top_z` from the resolved height stack.
    pub top_z: f64,
}

// ── Scan-line sampling ────────────────────────────────────────────────

/// Scan lines sampled between cancellation polls (G9).
///
/// The pre-G9 loop polled once per scan line. With the lines batched across
/// rayon the poll moves to the chunk boundary, so this is the cancellation
/// latency knob: one chunk is 32 lines spread over the pool, a few
/// milliseconds on the shipped-default fixture. Small enough to stay
/// responsive, large enough that the per-chunk join is noise.
pub(crate) const SCAN_CHUNK: usize = 32;

/// Sample one scan line into the 3D points the V-groove cuts along it.
///
/// Returns an empty vec for a degenerate (zero-length) line, which the caller
/// skips — the pre-G9 `continue`.
fn sample_scan_line(
    line: &[P2; 2],
    field: &EdgeDistanceField,
    sample_step: f64,
    tan_half: f64,
    params: &VCarveParams,
) -> Vec<P3> {
    #[allow(clippy::indexing_slicing)]
    // SAFETY: `line` is a fixed-size [P2; 2] array.
    let (dx, dy) = (line[1].x - line[0].x, line[1].y - line[0].y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return Vec::new();
    }

    let n_samples = (len / sample_step).ceil() as usize;
    let mut points: Vec<P3> = Vec::with_capacity(n_samples + 1);

    for i in 0..=n_samples {
        let t = i as f64 / n_samples.max(1) as f64;
        #[allow(clippy::indexing_slicing)]
        // SAFETY: `line` is a fixed-size [P2; 2] array.
        let (x, y) = (line[0].x + t * dx, line[0].y + t * dy);

        let dist = field.distance(&P2::new(x, y));
        let depth = if params.max_depth > 0.0 {
            (dist / tan_half).min(params.max_depth)
        } else {
            dist / tan_half
        };
        points.push(P3::new(x, y, params.top_z - depth));
    }

    points
}

/// Sample a batch of scan lines, in parallel when the `parallel` feature is
/// on. Order is preserved, so emission — and therefore the emitted G-code —
/// is byte-identical either way.
fn sample_chunk(
    chunk: &[[P2; 2]],
    field: &EdgeDistanceField,
    sample_step: f64,
    tan_half: f64,
    params: &VCarveParams,
) -> Vec<Vec<P3>> {
    #[cfg(feature = "parallel")]
    {
        chunk
            .par_iter()
            .map(|line| sample_scan_line(line, field, sample_step, tan_half, params))
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        chunk
            .iter()
            .map(|line| sample_scan_line(line, field, sample_step, tan_half, params))
            .collect()
    }
}

// ── Public API ─────────────────────────────────────────────────────────

/// Generate a V-carve toolpath for a 2D polygon region.
///
/// The V-bit follows scan lines across the polygon. At each point, the
/// cut depth is determined by the distance to the nearest boundary edge:
/// `depth = distance / tan(half_angle)`, clamped to `max_depth`.
///
/// This produces a V-groove that exactly meets the design outline when
/// the V-bit half-angle matches the specified value.
pub fn vcarve_toolpath(polygon: &Polygon2, params: &VCarveParams) -> Toolpath {
    let never_cancel = || false;
    // infallible: cancel closure always returns false, so Cancelled is unreachable
    #[allow(clippy::expect_used)]
    vcarve_toolpath_with_cancel(polygon, params, &never_cancel)
        .expect("non-cancellable vcarve toolpath should never be cancelled")
}

/// Cancellable variant of [`vcarve_toolpath`]. Checks `cancel` as its very
/// first statement, then polls again once per [`SCAN_CHUNK`]-line batch of
/// the distance-field sampling loop
/// (planning/finishing_stack_review_2026-07.md S.5: "vcarve/inlay
/// (scanline distance field)").
///
/// **G9 (2026-08-20) moved the poll from per-line to per-batch** so the
/// lines within a batch can be sampled in parallel. The cadence is still
/// bounded by wall clock, not by polygon size: a batch is 32 lines shared
/// across the rayon pool. A cancel flag set before the call still
/// short-circuits before any sampling, which is what
/// `cancellable_families_honour_a_preset_cancel_flag` pins.
pub fn vcarve_toolpath_with_cancel(
    polygon: &Polygon2,
    params: &VCarveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    let tan_half = params.half_angle.tan();
    if tan_half < 1e-10 {
        return Ok(Toolpath::new()); // degenerate angle
    }

    // Generate scan lines with a tiny inset so clipping works
    let inset = params.tolerance.min(0.05);
    let scan_lines = crate::zigzag::zigzag_lines(polygon, inset, params.stepover, 0.0);

    let sample_step = params.tolerance.max(0.05);

    // G9: index the boundary ONCE instead of walking every edge per sample.
    let field = EdgeDistanceField::from_polygon(polygon);

    let mut tp = Toolpath::new();

    for batch in scan_lines.chunks(SCAN_CHUNK) {
        check_cancel(cancel)?;
        for points in sample_chunk(batch, &field, sample_step, tan_half, params) {
            if points.is_empty() {
                continue;
            }
            tp.emit_path_segment_with_intent(
                &points,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
    }

    Ok(tp)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    /// Shim keeping the pre-G9 unit tests calling the shape they were written
    /// against. The generator now builds one [`EdgeDistanceField`] per
    /// polygon rather than one per sample, so this is the same query, not a
    /// parallel implementation of it.
    fn point_to_polygon_distance(point: &P2, polygon: &Polygon2) -> f64 {
        EdgeDistanceField::from_polygon(polygon).distance(point)
    }

    /// The pre-G9 generator body, verbatim except that the per-sample
    /// distance comes from the field's **linear** scan. Reference for
    /// `vcarve_indexed_field_is_bit_identical_to_linear_scan`.
    fn naive_vcarve(polygon: &Polygon2, params: &VCarveParams) -> Toolpath {
        let tan_half = params.half_angle.tan();
        if tan_half < 1e-10 {
            return Toolpath::new();
        }
        let inset = params.tolerance.min(0.05);
        let scan_lines = crate::zigzag::zigzag_lines(polygon, inset, params.stepover, 0.0);
        let sample_step = params.tolerance.max(0.05);
        let field = EdgeDistanceField::from_polygon(polygon);

        let mut tp = Toolpath::new();
        for line in &scan_lines {
            let dx = line[1].x - line[0].x;
            let dy = line[1].y - line[0].y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-10 {
                continue;
            }
            let n_samples = (len / sample_step).ceil() as usize;
            let mut points: Vec<P3> = Vec::with_capacity(n_samples + 1);
            for i in 0..=n_samples {
                let t = i as f64 / n_samples.max(1) as f64;
                let x = line[0].x + t * dx;
                let y = line[0].y + t * dy;
                let dist = field.distance_linear(&P2::new(x, y));
                let depth = if params.max_depth > 0.0 {
                    (dist / tan_half).min(params.max_depth)
                } else {
                    dist / tan_half
                };
                points.push(P3::new(x, y, params.top_z - depth));
            }
            if points.is_empty() {
                continue;
            }
            tp.emit_path_segment_with_intent(
                &points,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
        tp
    }

    fn move_bits(tp: &Toolpath) -> Vec<(u64, u64, u64, String)> {
        tp.moves
            .iter()
            .map(|m| {
                (
                    m.target.x.to_bits(),
                    m.target.y.to_bits(),
                    m.target.z.to_bits(),
                    format!("{:?}/{:?}", m.move_type, m.intent),
                )
            })
            .collect()
    }

    /// **G9 equivalence sentry.**
    ///
    /// Raw bits, not `==`: `-0.0 == 0.0` would hide a divergence, and wave 1's
    /// G3 shipped a "provably sound" reject that produced an identical move
    /// *count* with a different hash. V-carve maps boundary distance straight
    /// to cut depth, so a one-ULP index error is a cut-quality defect, not a
    /// rounding curiosity.
    #[test]
    fn vcarve_indexed_field_is_bit_identical_to_linear_scan() {
        // A lettering-ish fixture: outer frame plus a grid of many-vertex
        // holes, so pruning has something to prune and ties are plentiful.
        let mut poly = Polygon2::rectangle(-30.0, -30.0, 30.0, 30.0);
        for r in 1..=3 {
            for c in 1..=3 {
                let (cx, cy) = (-30.0 + 15.0 * c as f64, -30.0 + 15.0 * r as f64);
                poly.holes.push(
                    (0..96)
                        .map(|i| {
                            let t = -std::f64::consts::TAU * i as f64 / 96.0;
                            P2::new(cx + 3.3 * t.cos(), cy + 3.3 * t.sin())
                        })
                        .collect(),
                );
            }
        }

        for (max_depth, tolerance, half_angle) in [
            (4.0, 0.05, FRAC_PI_4),
            (0.0, 0.2, FRAC_PI_4),
            (2.0, 0.5, (30.0_f64).to_radians()),
        ] {
            let params = VCarveParams {
                half_angle,
                max_depth,
                stepover: 1.0,
                feed_rate: 1200.0,
                plunge_rate: 400.0,
                safe_z: 10.0,
                tolerance,
                top_z: 0.0,
            };
            let indexed = vcarve_toolpath(&poly, &params);
            let linear = naive_vcarve(&poly, &params);
            assert!(!indexed.moves.is_empty(), "fixture should cut something");
            assert_eq!(
                move_bits(&indexed),
                move_bits(&linear),
                "indexed distance field diverged from the linear scan at \
                 max_depth={max_depth} tolerance={tolerance}"
            );
        }
    }

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params() -> VCarveParams {
        VCarveParams {
            half_angle: FRAC_PI_4, // 90° V-bit
            max_depth: 10.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
            top_z: 0.0,
        }
    }

    #[test]
    fn test_point_to_polygon_distance_center() {
        let sq = square_polygon(20.0);
        // Center of 20×20 square → distance to nearest wall = 10.0
        let d = point_to_polygon_distance(&P2::new(0.0, 0.0), &sq);
        assert!(
            (d - 10.0).abs() < 0.1,
            "Center should be ~10mm from wall, got {:.2}",
            d
        );
    }

    #[test]
    fn test_point_to_polygon_distance_near_wall() {
        let sq = square_polygon(20.0);
        // 1mm from right wall
        let d = point_to_polygon_distance(&P2::new(9.0, 0.0), &sq);
        assert!(
            (d - 1.0).abs() < 0.1,
            "Should be ~1mm from wall, got {:.2}",
            d
        );
    }

    #[test]
    fn test_point_to_polygon_distance_with_hole() {
        let hole = vec![
            P2::new(-2.0, -2.0),
            P2::new(-2.0, 2.0),
            P2::new(2.0, 2.0),
            P2::new(2.0, -2.0),
        ];
        let poly = Polygon2::with_holes(square_polygon(20.0).exterior, vec![hole]);

        // Point between hole and exterior: 3mm from hole edge, 7mm from exterior
        let d = point_to_polygon_distance(&P2::new(5.0, 0.0), &poly);
        assert!(
            (d - 3.0).abs() < 0.1,
            "Should be ~3mm from hole, got {:.2}",
            d
        );
    }

    // ── V-carve depth tests ───────────────────────────────────────────

    #[test]
    fn test_vcarve_depth_at_center() {
        // 90° V-bit (half_angle = 45°, tan = 1.0)
        // Center of 10mm square → distance = 5mm → depth = 5mm
        let sq = square_polygon(10.0);
        let d = point_to_polygon_distance(&P2::new(0.0, 0.0), &sq);
        let depth = d / FRAC_PI_4.tan();
        assert!(
            (depth - 5.0).abs() < 0.1,
            "90° V-bit at center of 10mm square should cut ~5mm deep, got {:.2}",
            depth
        );
    }

    #[test]
    fn test_vcarve_depth_at_boundary() {
        // At the boundary, distance = 0 → depth = 0
        let sq = square_polygon(10.0);
        let d = point_to_polygon_distance(&P2::new(5.0, 0.0), &sq);
        assert!(d < 0.1, "At boundary, distance should be ~0, got {:.2}", d);
    }

    #[test]
    fn test_vcarve_max_depth_clamp() {
        // Center of large square with small max_depth
        let sq = square_polygon(40.0);
        let params = VCarveParams {
            max_depth: 3.0,
            ..default_params()
        };

        let tp = vcarve_toolpath(&sq, &params);

        // No feed move should go deeper than -max_depth
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    m.target.z >= -3.0 - 1e-10,
                    "No cut should exceed max_depth, got z={:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Toolpath structure tests ──────────────────────────────────────

    #[test]
    fn test_vcarve_toolpath_basic() {
        let sq = square_polygon(20.0);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "V-carve should generate moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have significant cutting, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_vcarve_z_varies_along_pass() {
        let sq = square_polygon(20.0);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);

        // Collect Z values of feed moves in a single pass
        // (between first rapid and second rapid)
        let mut z_values: Vec<f64> = Vec::new();
        let mut in_cut = false;
        for m in &tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if in_cut {
                        break; // end of first cut pass
                    }
                }
                crate::toolpath::MoveType::Linear { .. } => {
                    in_cut = true;
                    z_values.push(m.target.z);
                }
                _ => {}
            }
        }

        // Z should vary within the pass (not constant)
        if z_values.len() > 3 {
            let z_min = z_values.iter().copied().fold(f64::INFINITY, f64::min);
            let z_max = z_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (z_max - z_min).abs() > 0.5,
                "Z should vary along pass, range={:.2} ({:.2} to {:.2})",
                z_max - z_min,
                z_min,
                z_max
            );
        }
    }

    #[test]
    fn test_vcarve_empty_polygon() {
        // Too small to have scan lines
        let sq = square_polygon(0.01);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);
        assert!(
            tp.moves.len() <= 2,
            "Tiny polygon should produce minimal toolpath"
        );
    }

    #[test]
    fn test_vcarve_max_depth_zero_means_unlimited() {
        // max_depth=0.0 should mean unlimited — no clamping
        let sq = square_polygon(20.0);
        let params = VCarveParams {
            max_depth: 0.0,
            ..default_params()
        };

        let tp = vcarve_toolpath(&sq, &params);

        // The center of a 20mm square is 10mm from the wall.
        // With 90° V-bit (tan=1), depth at center = 10mm.
        // If max_depth=0.0 erroneously clamped, all depths would be 0.
        let has_nonzero_depth = tp.moves.iter().any(|m| {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                m.target.z < -0.1
            } else {
                false
            }
        });
        assert!(
            has_nonzero_depth,
            "max_depth=0.0 should produce non-zero depths (unlimited)"
        );
    }

    #[test]
    fn test_vcarve_cuts_relative_to_stock_top_s2() {
        // S.2 / F-028: v-carve must cut relative to `top_z`, not world Z=0.
        // Center of a 10mm square is 5mm from the wall -> depth = 5mm for a
        // 90-degree bit. With top_z = 5.0, cutting moves should land at
        // top_z - depth, not at -depth.
        let sq = square_polygon(10.0);
        let params = VCarveParams {
            top_z: 5.0,
            ..default_params()
        };

        let tp = vcarve_toolpath(&sq, &params);
        assert!(!tp.moves.is_empty(), "Expected moves");

        let mut saw_deep_cut = false;
        for m in &tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    // Rapids (approach/retract) should stay at safe_z, unaffected by top_z.
                    assert!(
                        (m.target.z - params.safe_z).abs() < 1e-9,
                        "Rapid should be at safe_z={}, got {}",
                        params.safe_z,
                        m.target.z
                    );
                }
                crate::toolpath::MoveType::Linear { .. } => {
                    // Cutting moves must never dip below top_z - max_depth,
                    // and must never exceed top_z (can't cut above stock top).
                    assert!(
                        m.target.z <= params.top_z + 1e-9,
                        "Cutting move z={} should not exceed top_z={}",
                        m.target.z,
                        params.top_z
                    );
                    assert!(
                        m.target.z >= params.top_z - params.max_depth - 1e-9,
                        "Cutting move z={} should not exceed top_z - max_depth={}",
                        m.target.z,
                        params.top_z - params.max_depth
                    );
                    if (m.target.z - (params.top_z - 5.0)).abs() < 0.5 {
                        saw_deep_cut = true;
                    }
                }
                _ => {}
            }
        }
        assert!(
            saw_deep_cut,
            "Expected a cut near top_z - 5.0 (center of 10mm square with 90-deg bit)"
        );
    }

    #[test]
    fn test_vcarve_60_degree_bit() {
        // 60° V-bit → half_angle = 30° → tan(30°) ≈ 0.577
        // At distance 5mm from wall: depth = 5 / 0.577 ≈ 8.66mm
        let half_angle = (30.0_f64).to_radians();
        let dist = 5.0;
        let depth = dist / half_angle.tan();
        assert!(
            (depth - 8.66).abs() < 0.1,
            "60° V-bit at 5mm from wall should cut ~8.66mm, got {:.2}",
            depth
        );
    }
}
