use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, RetractStrategy};
use rs_cam_core::compute::execute::apply_dressups;
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{MoveType, Toolpath};

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn append_segment(tp: &mut Toolpath, start: P3, end: P3, safe_z: f64) {
        tp.rapid_to(P3::new(start.x, start.y, safe_z));
        tp.feed_to(start, 1000.0);
        tp.feed_to(end, 1000.0);
        tp.rapid_to(P3::new(end.x, end.y, safe_z));
    }

    fn cutting_z_values(tp: &Toolpath) -> Vec<f64> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect()
    }

    #[test]
    fn rapid_order_dressup_respects_adaptive3d_depth_barriers() {
        let safe_z = 10.0;
        let mut raw = Toolpath::new();

        append_segment(
            &mut raw,
            P3::new(0.0, 0.0, 22.0),
            P3::new(1.0, 0.0, 22.0),
            safe_z,
        );
        append_segment(
            &mut raw,
            P3::new(100.0, 0.0, 22.0),
            P3::new(101.0, 0.0, 22.0),
            safe_z,
        );
        let z7_barrier = raw.moves.len();
        append_segment(
            &mut raw,
            P3::new(2.0, 0.0, 7.0),
            P3::new(3.0, 0.0, 7.0),
            safe_z,
        );
        append_segment(
            &mut raw,
            P3::new(4.0, 0.0, 7.0),
            P3::new(5.0, 0.0, 7.0),
            safe_z,
        );

        let cfg = DressupConfig {
            optimize_rapid_order: true,
            retract_strategy: RetractStrategy::Full,
            ..DressupConfig::default()
        };
        let optimized = apply_dressups(
            raw,
            &cfg,
            6.0,
            safe_z,
            None,
            None,
            None,
            &[0, z7_barrier],
            OperationType::Adaptive3d.transform_capabilities(),
        );

        let cutting_z = cutting_z_values(&optimized);

        assert_eq!(&cutting_z[0..4], &[22.0, 22.0, 22.0, 22.0]);
        assert_eq!(&cutting_z[4..], &[7.0, 7.0, 7.0, 7.0]);
    }

    #[test]
    fn adaptive3d_capability_disables_unbarriered_global_rapid_order() {
        let safe_z = 10.0;
        let mut raw = Toolpath::new();
        append_segment(
            &mut raw,
            P3::new(100.0, 0.0, 22.0),
            P3::new(101.0, 0.0, 22.0),
            safe_z,
        );
        append_segment(
            &mut raw,
            P3::new(0.0, 0.0, 7.0),
            P3::new(1.0, 0.0, 7.0),
            safe_z,
        );
        let raw_cutting_z = cutting_z_values(&raw);

        let cfg = DressupConfig {
            optimize_rapid_order: true,
            retract_strategy: RetractStrategy::Full,
            ..DressupConfig::default()
        };
        let optimized = apply_dressups(
            raw,
            &cfg,
            6.0,
            safe_z,
            None,
            None,
            None,
            &[],
            OperationType::Adaptive3d.transform_capabilities(),
        );

        assert_eq!(cutting_z_values(&optimized), raw_cutting_z);
    }
}
