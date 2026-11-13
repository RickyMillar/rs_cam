//! **The force field of the width x depth plane** (feeds contract P17b
//! item 3; moved to core by P29-10 10g, `P29-10_DESIGN.md` 3.8, lead
//! rulings O4, R1 and R9).
//!
//! # What it is
//!
//! A [`ForceField`] is the force law of the BODY cut sampled on a grid of
//! the plane: at each node `(ae, ap)` the peak engaged-edge resultant
//! [`crate::feeds::force_budget::peak_force_n`] of the op's tool on its
//! material at the PLANNED advance per tooth (the op's steady feed over its
//! spindle speed and the tool's flutes, through
//! [`crate::feeds::AdvancePerToothMm::from_commanded_at`]). A node the law
//! cannot answer (no force line, a width or a depth of 0) is NaN. The grid
//! is [`FIELD_COLUMNS`] x [`FIELD_ROWS`] nodes over `[0, D]` x
//! `[0, ap_top]`; the caller states `ap_top` (the top of the chart that
//! draws the field), so the grid covers every depth the chart shows.
//!
//! A [`ForcePlane`] is the field, the machine force budget and the level
//! lines of the field at [`CONTOUR_SHARES`] of that budget. The budget is
//! the machine fact `MachineProfile::rigidity.force_budget_n`
//! (`machine/mod.rs`, derived by `feeds::force_budget::hobby_budget` for
//! the hobby class), the budget that the force-budget rows and the depth
//! gate read. [`BudgetRole`] says whether that budget is a limit of this
//! cut (`cut_force::budget_line`: the body cut has the force-budget depth
//! limit) or only a reference line (lead ruling O4: the field draws for
//! every op with a force line, and the 100 % line says "no limit here").
//!
//! # What it is NOT
//!
//! The field judges nothing. It is the field the samples live in (they
//! read the same law, `cut_force::sample_peak_force_n`), so its 100 % line
//! predicts the over-budget samples ([`super::cut_force::CutPoint3::over_budget`]).
//! The force-budget edge of the envelope is the lowest ceiling over EVERY
//! declared cut (a slot ring, a planner cap at its wider width, an entry at
//! its own chip), so it can sit under the body line; the planned mark of
//! the cut that sets it shows why (lead ruling R1). The plane is built once
//! per load report (the cut-metric memo of the GUI), never per frame.

use crate::material::Material;
use crate::tool::ToolDefinition;

/// The nodes of the field across the width `[0, D]`.
pub const FIELD_COLUMNS: usize = 48;

/// The nodes of the field up the depth `[0, ap_top]`.
pub const FIELD_ROWS: usize = 32;

/// The level lines of a [`ForcePlane`], as shares of the machine force
/// budget: the budget itself and the three quarters under it, so the
/// lines step in equal force under the one line that a row reads (a
/// display choice of four equal steps, not a limit).
pub const CONTOUR_SHARES: [f64; 4] = [0.25, 0.5, 0.75, 1.0];

/// The force law on a grid of the width x depth plane. Row-major: the node
/// `(column c, row r)` is `force_n[r * aes.len() + c]`, at the width
/// `aes[c]` and the depth `aps[r]`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ForceField {
    /// The width (mm) of each column, ascending.
    pub aes: Vec<f64>,
    /// The depth (mm) of each row, ascending.
    pub aps: Vec<f64>,
    /// The peak force (N) at each node, row-major; NaN where the law does
    /// not answer (`null` on the wire).
    pub force_n: Vec<f64>,
}

impl ForceField {
    /// The field of `tool` on `material` at the advance per tooth `fz_mm`,
    /// [`FIELD_COLUMNS`] x [`FIELD_ROWS`] nodes over `[0, diameter_mm]` x
    /// `[0, ap_top_mm]`. `None` when a span or the advance is not positive.
    #[must_use]
    pub fn build(
        tool: &ToolDefinition,
        material: &Material,
        diameter_mm: f64,
        ap_top_mm: f64,
        fz_mm: f64,
    ) -> Option<Self> {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !(positive(diameter_mm) && positive(ap_top_mm) && positive(fz_mm)) {
            return None;
        }
        let aes = Self::nodes(diameter_mm, FIELD_COLUMNS);
        let aps = Self::nodes(ap_top_mm, FIELD_ROWS);
        Some(Self::from_fn(aes, aps, |ae, ap| {
            if ae > 0.0 && ap > 0.0 {
                crate::feeds::force_budget::peak_force_n(tool, material, ap, ae, fz_mm)
                    .unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        }))
    }

    /// The field of `f(ae, ap)` on the nodes `aes` x `aps` (a test builds a
    /// field of a known shape here).
    #[must_use]
    pub fn from_fn(aes: Vec<f64>, aps: Vec<f64>, f: impl Fn(f64, f64) -> f64) -> Self {
        let mut force_n = Vec::with_capacity(aes.len() * aps.len());
        for ap in &aps {
            for ae in &aes {
                force_n.push(f(*ae, *ap));
            }
        }
        Self { aes, aps, force_n }
    }

    /// `n` nodes from 0 to `top`, the ends included.
    fn nodes(top: f64, n: usize) -> Vec<f64> {
        (0..n)
            .map(|k| top * k as f64 / (n - 1).max(1) as f64)
            .collect()
    }

    /// The force at the node `(column, row)`; NaN off the grid.
    #[must_use]
    pub fn at(&self, column: usize, row: usize) -> f64 {
        if column >= self.aes.len() {
            return f64::NAN;
        }
        self.force_n
            .get(row * self.aes.len() + column)
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// The depth step of the grid (mm): one cell.
    #[must_use]
    pub fn cell_mm(&self) -> f64 {
        match (self.aps.first(), self.aps.last()) {
            (Some(lo), Some(hi)) if self.aps.len() > 1 => (hi - lo) / (self.aps.len() - 1) as f64,
            (Some(_) | None, Some(_) | None) => 0.0,
        }
    }

    /// The largest finite force of the field, `None` when every node is
    /// NaN.
    #[must_use]
    pub fn peak(&self) -> Option<f64> {
        self.force_n
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(None, |acc: Option<f64>, v| {
                Some(acc.map_or(v, |a| a.max(v)))
            })
    }
}

/// Whether the machine force budget is a limit of the op's body cut
/// (lead ruling O4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetRole {
    /// The body cut has the force-budget depth limit
    /// (`cut_force::budget_line` is `Some`): the 100 % line is the limit,
    /// and a sample over it is over the budget.
    Limit,
    /// The body cut has no force limit (a one-sided finish pass): the 100 %
    /// line is a reference only, and no sample is over a limit.
    NoLimitHere,
}

impl BudgetRole {
    /// The name of the 100 % line on the chart.
    #[must_use]
    pub const fn line_name(self) -> &'static str {
        match self {
            Self::Limit => "100 % budget",
            Self::NoLimitHere => "budget, no limit here",
        }
    }
}

/// One level line of a [`ForcePlane`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ForceLevel {
    /// The share of the budget.
    pub share: f64,
    /// The force of the level (N).
    pub force_n: f64,
    /// The polylines, each point `[ae mm, ap mm]`.
    pub lines: Vec<Vec<[f64; 2]>>,
}

/// The force field of an op with the machine budget and its level lines.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ForcePlane {
    /// The field of the body cut at the planned chip.
    pub field: ForceField,
    /// The advance per tooth the field reads (mm).
    pub fz_mm: f64,
    /// The machine force budget (N).
    pub budget_n: f64,
    /// Whether the budget is a limit of the body cut.
    pub role: BudgetRole,
    /// The level lines at [`CONTOUR_SHARES`] of the budget that the field
    /// reaches, the budget last.
    pub levels: Vec<ForceLevel>,
}

impl ForcePlane {
    /// The plane of `tool` on `material` at the steady feed
    /// `steady_feed_mm_min` and the spindle speed `rpm`, over
    /// `[0, diameter_mm]` x `[0, ap_top_mm]`, against `budget_n`. `None`
    /// when the advance per tooth is not positive, the budget is not
    /// positive or the field does not build.
    #[expect(
        clippy::too_many_arguments,
        reason = "the inputs of one law on one grid; the session door gathers them"
    )]
    #[must_use]
    pub fn build(
        tool: &ToolDefinition,
        material: &Material,
        steady_feed_mm_min: f64,
        rpm: f64,
        diameter_mm: f64,
        ap_top_mm: f64,
        budget_n: f64,
        role: BudgetRole,
    ) -> Option<Self> {
        if !(budget_n.is_finite() && budget_n > 0.0) {
            return None;
        }
        let fz_mm = crate::feeds::AdvancePerToothMm::from_commanded_at(
            crate::feeds::CommandedFeedMmMin::new(steady_feed_mm_min),
            rpm,
            tool.flute_count,
        )?
        .mm();
        let field = ForceField::build(tool, material, diameter_mm, ap_top_mm, fz_mm)?;
        let levels = CONTOUR_SHARES
            .into_iter()
            .map(|share| ForceLevel {
                share,
                force_n: share * budget_n,
                lines: contours(&field, share * budget_n),
            })
            .filter(|level| !level.lines.is_empty())
            .collect();
        Some(Self {
            field,
            fz_mm,
            budget_n,
            role,
            levels,
        })
    }
}

/// The level set of `field` at `level`: polylines in `[ae, ap]` (mm) by
/// marching squares. Each cell of four finite nodes gives the segments of
/// the crossing (a saddle splits by the cell's mean); a cell with a NaN
/// node gives none. The crossing on a shared edge is computed once per
/// edge, from the lower node to the higher, so the segments of two
/// neighbour cells meet exactly and [`join`] chains them into polylines.
/// A closed loop ends where it starts (the first point repeated).
#[must_use]
pub fn contours(field: &ForceField, level: f64) -> Vec<Vec<[f64; 2]>> {
    let cols = field.aes.len();
    let rows = field.aps.len();
    if cols < 2 || rows < 2 || !level.is_finite() {
        return Vec::new();
    }
    // The crossing on the edge between two nodes, `None` when the level
    // does not cross it. The edge runs from the lower-index node to the
    // higher one in both cells that share it.
    let cross = |p0: [f64; 2], v0: f64, p1: [f64; 2], v1: f64| -> Option<[f64; 2]> {
        let (lo, hi) = (v0.min(v1), v0.max(v1));
        if level < lo || level > hi || (hi - lo) <= 0.0 {
            return None;
        }
        let t = (level - v0) / (v1 - v0);
        Some([p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t])
    };
    let mut segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (
                field.aes.get(c),
                field.aes.get(c + 1),
                field.aps.get(r),
                field.aps.get(r + 1),
            ) else {
                continue;
            };
            let v00 = field.at(c, r);
            let v10 = field.at(c + 1, r);
            let v01 = field.at(c, r + 1);
            let v11 = field.at(c + 1, r + 1);
            if !(v00.is_finite() && v10.is_finite() && v01.is_finite() && v11.is_finite()) {
                continue;
            }
            // The four edges, each from its lower-index node.
            let bottom = cross([x0, y0], v00, [x1, y0], v10);
            let top = cross([x0, y1], v01, [x1, y1], v11);
            let left = cross([x0, y0], v00, [x0, y1], v01);
            let right = cross([x1, y0], v10, [x1, y1], v11);
            let above = |v: f64| v >= level;
            let case = usize::from(above(v00))
                | usize::from(above(v10)) << 1
                | usize::from(above(v11)) << 2
                | usize::from(above(v01)) << 3;
            let mut push = |a: Option<[f64; 2]>, b: Option<[f64; 2]>| {
                if let (Some(a), Some(b)) = (a, b)
                    && a != b
                {
                    segments.push((a, b));
                }
            };
            match case {
                1 | 14 => push(left, bottom),
                2 | 13 => push(bottom, right),
                3 | 12 => push(left, right),
                4 | 11 => push(right, top),
                6 | 9 => push(bottom, top),
                7 | 8 => push(left, top),
                5 | 10 => {
                    // The saddle: the cell's mean decides which corners
                    // join.
                    let mean = (v00 + v10 + v01 + v11) / 4.0;
                    if (case == 5) == above(mean) {
                        push(left, top);
                        push(bottom, right);
                    } else {
                        push(left, bottom);
                        push(right, top);
                    }
                }
                // 0 and 15: the cell is all under or all over the level.
                _ => {}
            }
        }
    }
    join(segments)
}

/// Chain `segments` into polylines: a segment joins the chain whose end
/// is its start or its end (within [`JOIN_EPS`] of the grid span), from
/// both ends of the chain, until no segment is left.
fn join(mut segments: Vec<([f64; 2], [f64; 2])>) -> Vec<Vec<[f64; 2]>> {
    let span = segments
        .iter()
        .flat_map(|(a, b)| [a[0].abs(), a[1].abs(), b[0].abs(), b[1].abs()])
        .fold(0.0_f64, f64::max);
    let eps = span * JOIN_EPS;
    let same = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() <= eps && (p[1] - q[1]).abs() <= eps;
    let mut out = Vec::new();
    while let Some((a, b)) = segments.pop() {
        let mut chain = vec![a, b];
        let mut grew = true;
        while grew {
            grew = false;
            if let Some(&end) = chain.last()
                && let Some(i) = segments
                    .iter()
                    .position(|(p, q)| same(*p, end) || same(*q, end))
            {
                let (p, q) = segments.swap_remove(i);
                chain.push(if same(p, end) { q } else { p });
                grew = true;
            }
            if let Some(&start) = chain.first()
                && let Some(i) = segments
                    .iter()
                    .position(|(p, q)| same(*p, start) || same(*q, start))
            {
                let (p, q) = segments.swap_remove(i);
                chain.insert(0, if same(p, start) { q } else { p });
                grew = true;
            }
        }
        out.push(chain);
    }
    out
}

/// The join tolerance of [`join`], as a share of the largest coordinate:
/// two crossings of one shared edge are the same computation, so any
/// positive tolerance under a cell joins them.
const JOIN_EPS: f64 = 1e-9;

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "test code: a panic is how a test reports a failure"
    )]

    use super::*;

    fn nodes(top: f64, n: usize) -> Vec<f64> {
        ForceField::nodes(top, n)
    }

    #[test]
    fn a_plane_field_gives_one_frame_to_frame_line_at_its_level() {
        // f = ap: the level 1.5 is the horizontal line ap = 1.5 across
        // the width.
        let field = ForceField::from_fn(nodes(6.0, 7), nodes(4.0, 9), |_, ap| ap);
        let lines = contours(&field, 1.5);
        assert_eq!(lines.len(), 1, "one polyline: {lines:?}");
        let line = &lines[0];
        assert!(line.iter().all(|p| (p[1] - 1.5).abs() < 1e-12), "{line:?}");
        let xs: Vec<f64> = line.iter().map(|p| p[0]).collect();
        let (lo, hi) = (
            xs.iter().copied().fold(f64::INFINITY, f64::min),
            xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        assert!(
            (lo - 0.0).abs() < 1e-12 && (hi - 6.0).abs() < 1e-12,
            "frame to frame: {xs:?}"
        );
        assert!((field.cell_mm() - 0.5).abs() < 1e-12, "one cell: 4 mm / 8");
    }

    #[test]
    fn a_radial_field_gives_one_closed_loop() {
        // f = the distance from the centre: the level 1 is a circle of
        // radius 1 about (3, 2), inside the grid.
        let field = ForceField::from_fn(nodes(6.0, 25), nodes(4.0, 17), |ae, ap| {
            ((ae - 3.0).powi(2) + (ap - 2.0).powi(2)).sqrt()
        });
        let lines = contours(&field, 1.0);
        assert_eq!(lines.len(), 1, "one loop: {}", lines.len());
        let loop_ = &lines[0];
        let first = loop_[0];
        let last = loop_[loop_.len() - 1];
        assert!(
            (first[0] - last[0]).abs() < 1e-9 && (first[1] - last[1]).abs() < 1e-9,
            "closed: {first:?} vs {last:?}"
        );
        for p in loop_ {
            let r = ((p[0] - 3.0).powi(2) + (p[1] - 2.0).powi(2)).sqrt();
            assert!(
                (r - 1.0).abs() < 0.05,
                "on the circle (linear interpolation): r {r}"
            );
        }
    }

    #[test]
    fn a_nan_node_gives_no_segment_in_its_cells() {
        let field = ForceField::from_fn(nodes(6.0, 7), nodes(4.0, 9), |ae, ap| {
            if ae <= 0.0 { f64::NAN } else { ap }
        });
        let lines = contours(&field, 1.5);
        assert!(
            lines.iter().flatten().all(|p| p[0] >= 1.0 - 1e-12),
            "the first column is NaN, so no point is left of its right edge: {lines:?}"
        );
    }

    #[test]
    fn a_level_off_the_field_gives_nothing() {
        let field = ForceField::from_fn(nodes(6.0, 7), nodes(4.0, 9), |_, ap| ap);
        assert!(contours(&field, 9.0).is_empty());
        assert!(contours(&field, f64::NAN).is_empty());
        assert_eq!(field.peak().expect("a finite node"), 4.0);
    }

    #[test]
    fn the_budget_role_names_its_line() {
        assert_eq!(BudgetRole::Limit.line_name(), "100 % budget");
        assert_eq!(BudgetRole::NoLimitHere.line_name(), "budget, no limit here");
    }
}
