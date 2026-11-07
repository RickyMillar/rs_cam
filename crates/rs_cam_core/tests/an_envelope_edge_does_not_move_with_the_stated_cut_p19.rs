//! Feeds contract P19 (operator report 2026-11-07): "min width should not
//! move when doc moves. it should just be a boundary on the graph!"
//!
//! The claim: an envelope edge does not move when the operator's stated
//! depth (ap) or width (ae) moves. For each drawn edge (every `Limit`
//! whose edge is not `Unmodelled` or `PerPoint`), the drawn shape is equal
//! at two stated depths and two stated widths. The tool, the material, the
//! machine, the stated feed and RPM, and the model box stay the same.
//!
//! Three ops, each with the model box, so the `Runtime` limit is built:
//!
//! 1. A Pocket (one Z level: its move model does not read the depth).
//! 2. An Adaptive3d (its move model reads the depth per pass).
//! 3. A force-budget op: a Pocket roughing op on a material with a force
//!    line and the default (hobby) machine, with a stated chip under the
//!    suggested chip, so the `Rigidity` arm draws an `ApCurve` and the pick
//!    chip differs from the curve chip.
//!
//! Red before P19 phase 2a (HEAD 0795306a2): the Adaptive3d `Runtime`
//! `AeFloor` follows the stated depth per pass, and the `Rigidity` curve
//! carries a sample at the proposal width at the pick chip (a notch that
//! follows the stated cut).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test an_envelope_edge_does_not_move_with_the_stated_cut_p19
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, Limit, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::suggest::{SuggestContext, feeds_input_for_operation};
use rs_cam_core::feeds::{SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The tolerance (mm) of two drawn values. The force-budget curve keeps an
/// exact sample at the stated width (it places the stated point against
/// the solved depth), so two curves differ there by the error of a straight
/// line between solved samples. `refine_budget_columns` splits a segment
/// while that error is over `BINSEARCH_CONVERGENCE_MM` (0.005 mm); the
/// tolerance is four times that value. A notch at another chip is larger
/// by orders of magnitude.
const TOL_MM: f64 = 4.0 * rs_cam_core::feeds::cutter_constraints::BINSEARCH_CONVERGENCE_MM;

fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(25.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(45.0));
    t
}

struct Case {
    name: &'static str,
    op: OperationType,
    material: Material,
    /// The stated chip as a share of the suggested chip at the first cut.
    chip_share: f64,
    depths: [f64; 2],
    widths: [f64; 2],
}

/// The model box (mm) of every case: 40 x 40 x 12.
fn model_box() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, -12.0),
        max: P3::new(40.0, 40.0, 0.0),
    }
}

/// The envelope of `case` at the stated cut `(ap, ae)` and the stated
/// speeds `(feed, rpm)`.
fn envelope(case: &Case, ap: f64, ae: f64, speeds: (f64, u32)) -> SafeEnvelope {
    let tool = tool();
    let machine = MachineProfile::default();
    let bbox = model_box();
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(case.op);
    assert!(op.set_depth_per_pass(ap), "{} holds a depth", case.name);
    assert!(op.set_stepover(ae), "{} holds a stepover", case.name);
    op.set_feed_rate(speeds.0);
    op.set_spindle_rpm(Some(speeds.1));
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &case.material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext {
            model_bbox: Some(&bbox),
            ..SuggestContext::default()
        },
    })
    .expect("the case is backed")
}

/// The stated speeds of `case`: the suggested RPM and the suggested chip x
/// `chip_share` at the first stated cut. One pair for every cut.
fn speeds(case: &Case) -> (f64, u32) {
    let tool = tool();
    let machine = MachineProfile::default();
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(case.op);
    assert!(op.set_depth_per_pass(case.depths[0]));
    assert!(op.set_stepover(case.widths[0]));
    let r = calculate(&feeds_input_for_operation(
        &op,
        &tool,
        &case.material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::default(),
    ));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive RPM below 100 000"
    )]
    let rpm = r.rpm.round() as u32;
    (r.feed_rate_mm_min * case.chip_share, rpm)
}

/// The drawn edges of an envelope, by kind, in boundary order.
fn drawn(env: &SafeEnvelope) -> Vec<(LimitKind, Edge)> {
    env.boundary
        .iter()
        .filter(|l: &&Limit| match l.edge {
            Edge::Unmodelled { .. } | Edge::PerPoint => false,
            Edge::ApCeiling { .. }
            | Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. } => true,
        })
        .map(|l| (l.kind, l.edge.clone()))
        .collect()
}

/// The distance (mm) from the point `s` to the polyline `points` in the
/// (ae, ap) plane. A cliff of the force-budget curve is a near-vertical
/// segment; a distance, not an `ap` at a fixed `ae`, judges it.
fn distance_to(points: &[[f64; 2]], s: [f64; 2]) -> f64 {
    let to_segment = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((s[0] - a[0]) * dx + (s[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (px, py) = (a[0] + t * dx, a[1] + t * dy);
        ((s[0] - px).powi(2) + (s[1] - py).powi(2)).sqrt()
    };
    match points {
        [] => f64::INFINITY,
        [only] => to_segment(*only, *only),
        _ => points
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => Some(to_segment(*a, *b)),
                _ => None,
            })
            .fold(f64::INFINITY, f64::min),
    }
}

/// The differences between two drawn edges of one kind, as text.
fn edge_moves(kind: LimitKind, a: &Edge, b: &Edge) -> Vec<String> {
    let scalar = |x: f64, y: f64| {
        ((x - y).abs() > TOL_MM).then(|| format!("{kind:?}: {a:?} -> {b:?} (value {x} -> {y})"))
    };
    match (a, b) {
        (Edge::ApCeiling { mm: x }, Edge::ApCeiling { mm: y })
        | (Edge::ApFloor { mm: x }, Edge::ApFloor { mm: y })
        | (Edge::AeCeiling { mm: x }, Edge::AeCeiling { mm: y })
        | (Edge::AeFloor { mm: x }, Edge::AeFloor { mm: y }) => {
            scalar(*x, *y).into_iter().collect()
        }
        (Edge::ApCurve { points: p }, Edge::ApCurve { points: q }) => {
            // The two curves as point sets: each sample of one against the
            // other polyline, and the same width span.
            let mut out = Vec::new();
            let span = |c: &[[f64; 2]]| (c.first().map(|s| s[0]), c.last().map(|s| s[0]));
            let ((p0, p1), (q0, q1)) = (span(p), span(q));
            if p0 != q0 || p1 != q1 {
                out.push(format!(
                    "{kind:?}: the width span moves {p0:?}..{p1:?} -> {q0:?}..{q1:?}"
                ));
            }
            for (s, other, side) in p
                .iter()
                .map(|s| (s, q.as_slice(), "first"))
                .chain(q.iter().map(|s| (s, p.as_slice(), "second")))
            {
                let gap = distance_to(other, *s);
                if gap > TOL_MM {
                    out.push(format!(
                        "{kind:?}: the {side} curve's sample (ae {:.4}, ap {:.4}) is {gap:.4} mm off the other curve",
                        s[0], s[1]
                    ));
                }
            }
            out
        }
        _ => vec![format!("{kind:?}: the edge shape moves {a:?} -> {b:?}")],
    }
}

/// Every move between the drawn edges of two envelopes.
fn moves(a: &SafeEnvelope, b: &SafeEnvelope) -> Vec<String> {
    let (ea, eb) = (drawn(a), drawn(b));
    let kinds = |e: &[(LimitKind, Edge)]| e.iter().map(|x| x.0).collect::<Vec<_>>();
    if kinds(&ea) != kinds(&eb) {
        return vec![format!(
            "the drawn kinds move: {:?} -> {:?}",
            kinds(&ea),
            kinds(&eb)
        )];
    }
    ea.iter()
        .zip(eb.iter())
        .flat_map(|((k, x), (_, y))| edge_moves(*k, x, y))
        .collect()
}

fn check(case: &Case) -> Vec<String> {
    let speeds = speeds(case);
    let reference = envelope(case, case.depths[0], case.widths[0], speeds);
    assert!(
        !drawn(&reference).is_empty(),
        "{}: the envelope draws an edge",
        case.name
    );
    let mut out = Vec::new();
    for &ap in &case.depths {
        for &ae in &case.widths {
            let other = envelope(case, ap, ae, speeds);
            for m in moves(&reference, &other) {
                out.push(format!(
                    "{} (ap {} ae {} -> ap {ap} ae {ae}): {m}",
                    case.name, case.depths[0], case.widths[0]
                ));
            }
        }
    }
    out
}

/// The four cases of the main claim. The fourth is the force-budget case
/// at the full chip: the pick chip and the curve chip then agree, so it
/// separates the notch of the pick sample from the other moves.
fn cases() -> [Case; 4] {
    [
        Case {
            name: "pocket",
            op: OperationType::Pocket,
            material: Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
            chip_share: 1.0,
            depths: [1.0, 2.0],
            widths: [1.0, 2.0],
        },
        Case {
            name: "adaptive3d",
            op: OperationType::Adaptive3d,
            material: Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
            chip_share: 1.0,
            depths: [0.5, 1.0],
            widths: [1.0, 2.0],
        },
        Case {
            name: "force-budget pocket",
            op: OperationType::Pocket,
            material: Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
            chip_share: 0.6,
            depths: [1.0, 2.0],
            widths: [1.0, 2.0],
        },
        Case {
            name: "force-budget pocket, full chip",
            op: OperationType::Pocket,
            material: Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
            chip_share: 1.0,
            depths: [1.0, 2.0],
            widths: [1.0, 2.0],
        },
    ]
}

#[test]
fn an_envelope_edge_does_not_move_when_the_stated_cut_moves_p19() {
    let cases = cases();
    // Non-vacuity: the cases draw the edges the claim is about.
    let force = &cases[2];
    let env = envelope(force, force.depths[0], force.widths[0], speeds(force));
    assert!(
        matches!(
            env.limit(LimitKind::Rigidity).map(|l| &l.edge),
            Some(Edge::ApCurve { .. })
        ),
        "the force-budget case draws a Rigidity curve: {:?}",
        env.limit(LimitKind::Rigidity)
    );
    let pocket = &cases[0];
    let env = envelope(pocket, pocket.depths[0], pocket.widths[0], speeds(pocket));
    assert!(
        env.limit(LimitKind::Runtime).is_some(),
        "the Pocket case builds the Runtime limit"
    );
    let a3 = &cases[1];
    let env = envelope(a3, a3.depths[0], a3.widths[0], speeds(a3));
    assert!(
        env.limit(LimitKind::Runtime).is_some(),
        "the Adaptive3d case builds the Runtime limit"
    );
    let moved: Vec<String> = cases.iter().flat_map(check).collect();
    assert!(
        moved.is_empty(),
        "{} drawn edge(s) move with the stated cut:\n{}",
        moved.len(),
        moved.join("\n")
    );
}

/// The force-budget case of the main claim.
fn force_case() -> Case {
    let [_, _, force, _] = cases();
    force
}

/// The number of widths at which the drawn vendor depth table edge is
/// compared with its closed form. The lead ruling asks for 50; the edge has
/// corners (chip share 0.5, 0.75, 1) that 50 midpoints can miss, so the
/// check takes 1000 (each is one closed-form call).
const TABLE_CHECK_WIDTHS: usize = 1000;

#[test]
fn the_drawn_vendor_depth_table_edge_stands_on_its_closed_form_p19() {
    // The lead ruling of P19: a point drawn inside the line never gets an
    // "outside" verdict, and the reverse. The stated point is judged by the
    // closed form (`table_ceiling_at`), so the drawn edge must be within
    // `TOL_MM` of the closed form at every width, not only at the grid
    // columns.
    let mut curves = 0_usize;
    for case in cases() {
        curves += usize::from(table_edge_stands_on_its_closed_form(&case));
    }
    assert!(
        curves > 0,
        "non-vacuity: a case draws a vendor depth table curve"
    );
}

/// The check of [`the_drawn_vendor_depth_table_edge_stands_on_its_closed_form_p19`]
/// for one case at its first stated cut. `false` when the case draws no
/// vendor depth table curve.
fn table_edge_stands_on_its_closed_form(case: &Case) -> bool {
    let speeds = speeds(case);
    let env = envelope(case, case.depths[0], case.widths[0], speeds);
    let Some(Edge::ApCurve { points }) = env.limit(LimitKind::VendorDepthTable).map(|l| &l.edge)
    else {
        return false;
    };
    let tool = tool();
    let machine = MachineProfile::default();
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(case.op);
    assert!(op.set_depth_per_pass(case.depths[0]));
    assert!(op.set_stepover(case.widths[0]));
    let base = calculate(&feeds_input_for_operation(
        &op,
        &tool,
        &case.material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::default(),
    ));
    let band_max = base
        .chipload_bounds
        .map(|b| b.max_mm_per_tooth)
        .or(base.chipload_point_mm)
        .expect("the matched row prints a band");
    let d = tool.diameter;
    let fz = speeds.0 / (f64::from(speeds.1) * f64::from(tool.flute_count_planning()));
    let top = env.speeds.aps.last().copied().expect("a depth span");
    // `table_ceiling_at` (envelope.rs) in closed form.
    let closed = |ae: f64| {
        let psi = rs_cam_core::feeds::force::immersion_angle(ae, 0.5 * d);
        let share = fz * psi.min(std::f64::consts::FRAC_PI_2).sin() / band_max;
        if share > 0.5 {
            rs_cam_core::feeds::geometry::vendor_depth_table_ceiling_ratio(share) * d
        } else {
            top
        }
    };
    let (lo, hi) = (
        points.first().expect("a sample")[0],
        points.last().expect("a sample")[0],
    );
    let mut worst: (f64, f64) = (0.0, lo);
    let mut binding_widths = 0_usize;
    // The widths: an even spread, and the stated widths of the case (the
    // points the envelope judges by the closed form).
    #[expect(
        clippy::cast_precision_loss,
        reason = "an index under 10 000, exact in an f64"
    )]
    let spread = (0..TABLE_CHECK_WIDTHS)
        .map(|i| lo + (hi - lo) * (i as f64 + 0.5) / TABLE_CHECK_WIDTHS as f64);
    for ae in spread.chain(case.widths) {
        let exact = closed(ae);
        if exact < top {
            binding_widths += 1;
        }
        // The distance to the drawn polyline: a cliff (the step from the
        // span top to 3 x D) is a near-vertical drawn segment.
        let gap = distance_to(points, [ae, exact]);
        if gap > worst.0 {
            worst = (gap, ae);
        }
    }
    assert!(
        binding_widths > 0,
        "{}: non-vacuity: the table binds at some width",
        case.name
    );
    assert!(
        worst.0 <= TOL_MM,
        "{}: the drawn vendor depth table edge is {:.4} mm off its closed form at ae {:.4} \
         ({} drawn samples)",
        case.name,
        worst.0,
        worst.1,
        points.len()
    );
    true
}

#[test]
fn the_stated_point_still_warns_on_the_move_budget_p19() {
    use rs_cam_core::feeds::envelope::{MOVE_BUDGET_NOT_MET, MOVE_BUDGET_READS_DEPTH, Placement};
    // 1. An Adaptive3d: the edge is unmodelled, and a stated stepover far
    //    under the old floor (0.0512 mm at dpp 0.5, P19 audit) is outside.
    let a3 = Case {
        name: "adaptive3d",
        op: OperationType::Adaptive3d,
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        chip_share: 1.0,
        depths: [0.5, 1.0],
        widths: [0.03, 1.0],
    };
    let sp = speeds(&a3);
    let fine = envelope(&a3, 0.5, 0.03, sp);
    let runtime = fine.limit(LimitKind::Runtime).expect("a Runtime entry");
    assert_eq!(
        runtime.edge,
        Edge::Unmodelled {
            reason: MOVE_BUDGET_READS_DEPTH
        }
    );
    assert!(
        matches!(
            fine.operator,
            Some(Placement::Outside {
                binding: LimitKind::Runtime,
                ..
            })
        ),
        "the stated point over the move budget warns: {:?}",
        fine.operator
    );
    // The same op at a wide stepover meets the budget: no Runtime verdict.
    let wide = envelope(&a3, 0.5, 1.0, sp);
    assert!(
        !matches!(
            wide.operator,
            Some(Placement::Outside {
                binding: LimitKind::Runtime,
                ..
            })
        ),
        "{:?}",
        wide.operator
    );
    // 2. Saturation: a Pocket on a box where no width up to the ceiling
    //    meets the budget. Pocket moves = dx dy / s^2; at s = 0.5 D = 3 mm
    //    a 3000 x 2000 mm box gives 6.67e5 moves, over the 5e5 budget.
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -12.0),
        max: P3::new(3000.0, 2000.0, 0.0),
    };
    let tool = tool();
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    assert!(op.set_depth_per_pass(1.0));
    assert!(op.set_stepover(2.0));
    let env = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext {
            model_bbox: Some(&bbox),
            ..SuggestContext::default()
        },
    })
    .expect("the case is backed");
    let runtime = env.limit(LimitKind::Runtime).expect("a Runtime entry");
    assert_eq!(
        runtime.edge,
        Edge::Unmodelled {
            reason: MOVE_BUDGET_NOT_MET
        },
        "no floor is drawn at the ceiling"
    );
    assert!(
        env.notes
            .iter()
            .any(|n| n.starts_with("the move budget is not met at any width up to")),
        "{:?}",
        env.notes
    );
    assert!(
        matches!(
            env.operator,
            Some(Placement::Outside {
                binding: LimitKind::Runtime,
                ..
            })
        ),
        "the stated point warns: {:?}",
        env.operator
    );
}

#[test]
fn the_proposal_still_sits_on_its_force_budget_depth_p19() {
    use rs_cam_core::feeds::envelope::{PickRequest, Placement};
    // The pick depth at the proposal width (the pick chip) is out of the
    // drawn curve, but `place` still judges the proposal against it.
    let case = force_case();
    // A stated depth (10 mm) over the pick depth at the stated width (3.94
    // mm at ae 2 in the red output), inside the flute (25 mm).
    let env = envelope(&case, 10.0, case.widths[1], speeds(&case));
    assert_eq!(
        env.proposal_limits.first(),
        Some(&LimitKind::Rigidity),
        "non-vacuity: the force budget sets the proposal depth"
    );
    let pick = env.pick(PickRequest::Proposal);
    match pick.placement() {
        Placement::Inside { margin } => {
            assert!(
                margin.abs() < 1e-9,
                "the proposal sits on its edge: {margin}"
            );
        }
        Placement::Outside { binding, margin } => {
            panic!("the proposal is outside {binding:?} by {margin}")
        }
    }
}
