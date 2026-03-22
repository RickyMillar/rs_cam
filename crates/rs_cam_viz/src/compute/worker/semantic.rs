use rs_cam_core::geo::{P2, P3};
use rs_cam_core::semantic_trace::{ToolpathSemanticScope, ToolpathSemanticWriter};
use rs_cam_core::toolpath::{MoveType, Toolpath};

pub(super) struct CutRun {
    pub move_start: usize,
    pub move_end_exclusive: usize,
    pub closed_loop: bool,
    pub constant_z: bool,
    pub z_min: f64,
    pub z_max: f64,
}

pub(super) fn cutting_runs(toolpath: &Toolpath) -> Vec<CutRun> {
    let mut runs = Vec::new();
    let mut active_start = None;

    for move_idx in 0..toolpath.moves.len() {
        let is_cut = matches!(
            toolpath.moves[move_idx].move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if is_cut && active_start.is_none() {
            active_start = Some(move_idx.saturating_sub(1));
        }

        let next_is_cut = toolpath.moves.get(move_idx + 1).is_some_and(|mv| {
            matches!(
                mv.move_type,
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            )
        });
        if active_start.is_some() && (!is_cut || !next_is_cut) {
            let start = active_start.take().unwrap_or(0);
            let end_exclusive = (move_idx + 1).min(toolpath.moves.len());
            if let Some(run) = describe_run(toolpath, start, end_exclusive) {
                runs.push(run);
            }
        }
    }

    runs
}

fn describe_run(
    toolpath: &Toolpath,
    move_start: usize,
    move_end_exclusive: usize,
) -> Option<CutRun> {
    if move_end_exclusive <= move_start || move_end_exclusive > toolpath.moves.len() {
        return None;
    }

    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;

    if move_start > 0 {
        let prev = &toolpath.moves[move_start - 1].target;
        points.push((prev.x, prev.y));
        z_min = z_min.min(prev.z);
        z_max = z_max.max(prev.z);
    }
    for mv in &toolpath.moves[move_start..move_end_exclusive] {
        points.push((mv.target.x, mv.target.y));
        z_min = z_min.min(mv.target.z);
        z_max = z_max.max(mv.target.z);
    }

    let first = toolpath.moves.get(move_start)?.target;
    let last = toolpath.moves.get(move_end_exclusive - 1)?.target;
    let closed_loop = (first.x - last.x).abs() < 1e-6 && (first.y - last.y).abs() < 1e-6;

    Some(CutRun {
        move_start,
        move_end_exclusive,
        closed_loop,
        constant_z: (z_max - z_min).abs() < 1e-6,
        z_min,
        z_max,
    })
}

pub(super) fn bind_scope_to_run(scope: &ToolpathSemanticScope, toolpath: &Toolpath, run: &CutRun) {
    scope.bind_to_toolpath(toolpath, run.move_start, run.move_end_exclusive);
}

#[allow(dead_code)]
pub(super) fn bind_scope_to_full_toolpath(scope: &ToolpathSemanticScope, toolpath: &Toolpath) {
    scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
}

pub(super) fn append_toolpath(
    writer: &mut ToolpathSemanticWriter<'_>,
    scope: Option<&ToolpathSemanticScope>,
    toolpath: Toolpath,
) {
    writer.append_toolpath(scope, toolpath);
}

pub(super) fn line_toolpath(
    start: P2,
    end: P2,
    cut_depth: f64,
    safe_z: f64,
    plunge_rate: f64,
    feed_rate: f64,
) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(start.x, start.y, safe_z));
    tp.feed_to(P3::new(start.x, start.y, cut_depth), plunge_rate);
    tp.feed_to(P3::new(end.x, end.y, cut_depth), feed_rate);
    tp.rapid_to(P3::new(end.x, end.y, safe_z));
    tp
}

pub(super) fn contour_toolpath(
    contour: &[P2],
    cut_depth: f64,
    safe_z: f64,
    plunge_rate: f64,
    feed_rate: f64,
    reverse: bool,
) -> Toolpath {
    let mut tp = Toolpath::new();
    if contour.is_empty() {
        return tp;
    }
    let points: Vec<P2> = if reverse {
        contour.iter().copied().rev().collect()
    } else {
        contour.to_vec()
    };
    let start = points[0];
    tp.rapid_to(P3::new(start.x, start.y, safe_z));
    tp.feed_to(P3::new(start.x, start.y, cut_depth), plunge_rate);
    for pt in points.iter().skip(1) {
        tp.feed_to(P3::new(pt.x, pt.y, cut_depth), feed_rate);
    }
    tp.feed_to(P3::new(start.x, start.y, cut_depth), feed_rate);
    tp.rapid_to(P3::new(start.x, start.y, safe_z));
    tp
}
