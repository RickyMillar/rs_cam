"""D68: writes the option 2 prototype (the edge pass per level) into the fixture worktree, on top of a0_anchor_port.patch.
The pass runs only when RS_CAM_D68_OPTION2 is set. Usage: python3 apply_option2.py <worktree>"""
import sys
from pathlib import Path
A = Path(sys.argv[1]) / "crates/rs_cam_core/src/adaptive3d"
code = r'''
// ── D68 prototype, G-RESTDOC option 2: the edge pass per level ────────

/// D68 (prototype, not product): the reach band of a rest op. The band is
/// the planner cells a disc centred inside the centre containment reaches,
/// whose centre is outside it, with the column top read before the
/// boundary clear. The pass state lowers a band top where an emitted cut
/// reaches it.
pub(super) struct BandEdge {
    tops: Vec<Option<f64>>,
    rows: usize,
    cols: usize,
    origin_u: f64,
    origin_v: f64,
    cell: f64,
    reach: f64,
    rings: Vec<Vec<P2>>,
    scanned: usize,
}

impl BandEdge {
    /// `None` when there is no containment or no band cell holds stock.
    pub(super) fn new(
        stock: &TriDexelStock,
        prior: &super::path::PriorStockTops,
        centre_clip: Option<&CentreClip>,
        tool_radius: f64,
    ) -> Option<Self> {
        let clip = centre_clip?;
        let grid = &stock.z_grid;
        let polys = clip.polygons();
        let mut tops = vec![None; grid.rows * grid.cols];
        let mut n = 0usize;
        for (i, top) in prior.tops.iter().enumerate() {
            let Some(t) = top else { continue };
            let (row, col) = (i / prior.cols.max(1), i % prior.cols.max(1));
            let (x, y) = grid.cell_to_world(row, col);
            let p = P2::new(x, y);
            if !polys.iter().any(|q| q.contains_point(&p))
                && let Some(slot) = tops.get_mut(i)
            {
                *slot = Some(*t);
                n += 1;
            }
        }
        if n == 0 {
            return None;
        }
        // The pass line: the containment, 0.02 mm inside the clip.
        let step = grid.cell_size;
        let mut rings = Vec::new();
        for poly in polys {
            for shrunk in crate::polygon::offset_polygon(poly, 0.02) {
                for ring in polygon_rings(&shrunk) {
                    let mut pts = Vec::new();
                    for k in 0..ring.len() {
                        let (Some(a), Some(b)) = (ring.get(k), ring.get((k + 1) % ring.len()))
                        else {
                            continue;
                        };
                        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                        let m = (len / step).ceil().max(1.0) as usize;
                        for j in 0..m {
                            let t = j as f64 / m as f64;
                            pts.push(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
                        }
                    }
                    if pts.len() >= 3 {
                        rings.push(pts);
                    }
                }
            }
        }
        tracing::info!(
            band_cells = n,
            polygons = polys.len(),
            rings = rings.len(),
            ring_mm = rings.iter().map(|r| r.len() as f64 * step).sum::<f64>(),
            "D68 band edge: band"
        );
        Some(Self {
            tops,
            rows: grid.rows,
            cols: grid.cols,
            origin_u: grid.origin_u,
            origin_v: grid.origin_v,
            cell: grid.cell_size,
            reach: tool_radius + grid.cell_size * 0.5,
            rings,
            scanned: 0,
        })
    }

    /// Visit the band cells within reach of `(x, y)`.
    fn for_reach(&mut self, x: f64, y: f64, mut f: impl FnMut(&mut f64)) {
        let r = self.reach;
        let c0 = ((x - r - self.origin_u) / self.cell).floor().max(0.0) as usize;
        let c1 = (((x + r - self.origin_u) / self.cell).ceil().max(0.0) as usize)
            .min(self.cols.saturating_sub(1));
        let r0 = ((y - r - self.origin_v) / self.cell).floor().max(0.0) as usize;
        let r1 = (((y + r - self.origin_v) / self.cell).ceil().max(0.0) as usize)
            .min(self.rows.saturating_sub(1));
        for row in r0..=r1 {
            for col in c0..=c1 {
                let cx = self.origin_u + col as f64 * self.cell;
                let cy = self.origin_v + row as f64 * self.cell;
                if (cx - x).powi(2) + (cy - y).powi(2) > r * r {
                    continue;
                }
                if let Some(Some(t)) = self.tops.get_mut(row * self.cols + col) {
                    f(t);
                }
            }
        }
    }

    fn max_top_in_reach(&mut self, x: f64, y: f64) -> f64 {
        let mut m = f64::NEG_INFINITY;
        self.for_reach(x, y, |t| m = m.max(*t));
        m
    }

    fn lower_along(&mut self, path: &[P3]) {
        let step = self.cell;
        for w in path.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else {
                continue;
            };
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let m = (len / step).ceil().max(1.0) as usize;
            for j in 0..=m {
                let t = j as f64 / m as f64;
                let (x, y, z) = (
                    a.x + t * (b.x - a.x),
                    a.y + t * (b.y - a.y),
                    a.z + t * (b.z - a.z),
                );
                self.for_reach(x, y, |top| *top = top.min(z));
            }
        }
        if path.len() == 1
            && let Some(a) = path.first()
        {
            self.for_reach(a.x, a.y, |top| *top = top.min(a.z));
        }
    }

    /// Lower the band under every cut and link emitted since the last scan.
    fn scan(&mut self, segments: &[Adaptive3dSegment]) {
        let from = self.scanned.min(segments.len());
        for seg in segments.get(from..).unwrap_or(&[]) {
            match seg {
                Adaptive3dSegment::Cut(path) => self.lower_along(path),
                Adaptive3dSegment::Link(p) => self.lower_along(&[*p]),
                _ => {}
            }
        }
        self.scanned = segments.len();
    }
}

/// D68 (prototype, option 2): at `z_level`, one contour along the
/// containment line wherever the reach band stands above the cut. Each run
/// of the line where the band stands is one cut; its Z is the level,
/// draped onto the surface, and never more than one Depth/Pass under the
/// highest band top in reach.
pub(super) fn band_edge_pass(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
) {
    let mut guard = ctx.band_edge.borrow_mut();
    let Some(band) = guard.as_mut() else { return };
    band.scan(segments);
    let rings = std::mem::take(&mut band.rings);
    let mut runs: Vec<Vec<P3>> = Vec::new();
    for ring in &rings {
        let n = ring.len();
        let mut flag = Vec::with_capacity(n);
        let mut zs = Vec::with_capacity(n);
        for p in ring {
            let surf = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
            let floor = if surf == f64::NEG_INFINITY {
                z_level
            } else {
                (surf + ctx.stock_to_leave).max(z_level)
            };
            let top = band.max_top_in_reach(p.x, p.y);
            let z = floor.max(top - ctx.depth_per_pass - ctx.tolerance);
            flag.push(top > z + 0.05);
            zs.push(z);
        }
        let Some(start) = (0..n).find(|&i| !flag.get(i).copied().unwrap_or(true)) else {
            // The band stands along the whole ring: one closed cut.
            let mut path: Vec<P3> = ring
                .iter()
                .zip(&zs)
                .map(|(p, z)| P3::new(p.x, p.y, *z))
                .collect();
            if let Some(f) = path.first().copied() {
                path.push(f);
            }
            runs.push(path);
            continue;
        };
        let mut cur: Vec<P3> = Vec::new();
        for k in 1..=n {
            let i = (start + k) % n;
            if flag.get(i).copied().unwrap_or(false) {
                if let (Some(p), Some(z)) = (ring.get(i), zs.get(i)) {
                    cur.push(P3::new(p.x, p.y, *z));
                }
            } else if !cur.is_empty() {
                runs.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            runs.push(cur);
        }
    }
    band.rings = rings;
    drop(guard);
    let seg_len = |r: &Vec<P3>| -> f64 {
        r.windows(2)
            .map(|w| match (w.first(), w.get(1)) {
                (Some(a), Some(b)) => ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt(),
                _ => 0.0,
            })
            .sum()
    };
    let total_mm: f64 = runs.iter().map(seg_len).sum();
    let short = runs.iter().filter(|r| seg_len(r) < 5.0).count();
    tracing::info!(
        z = z_level,
        segments = runs.len(),
        under_5mm = short,
        length_mm = total_mm,
        "D68 band edge: level"
    );
    // Nearest first from the tool.
    let mut left: Vec<Vec<P3>> = runs;
    while !left.is_empty() {
        let here = cursor.last_pos.map_or((0.0, 0.0), |p| (p.x, p.y));
        let mut best = 0usize;
        let mut best_d = f64::INFINITY;
        let mut rev = false;
        for (k, r) in left.iter().enumerate() {
            for (end, is_rev) in [(r.first(), false), (r.last(), true)] {
                if let Some(e) = end {
                    let d = (e.x - here.0).powi(2) + (e.y - here.1).powi(2);
                    if d < best_d {
                        best_d = d;
                        best = k;
                        rev = is_rev;
                    }
                }
            }
        }
        let mut path = left.swap_remove(best);
        if rev {
            path.reverse();
        }
        if path.len() == 1
            && let Some(f) = path.first().copied()
        {
            path.push(P3::new(f.x + ctx.step_len, f.y, f.z));
        }
        let Some(first) = path.first().copied() else {
            continue;
        };
        let should_link = cursor
            .last_pos
            .is_some_and(|lp| side_link_ok(ctx, material_stock, surface_hm, lp, first));
        let entry_seg = if should_link {
            Adaptive3dSegment::Link(first)
        } else {
            Adaptive3dSegment::Rapid(first)
        };
        push_segment_with_stamp(
            segments,
            material_stock,
            ctx.lut,
            ctx.tool_radius,
            cursor,
            entry_seg,
            ctx.safe_z,
            ctx.cut_tolerance,
            ctx.min_cutting_radius,
            &ctx.stamp_drape(),
        );
        push_segment_with_stamp(
            segments,
            material_stock,
            ctx.lut,
            ctx.tool_radius,
            cursor,
            Adaptive3dSegment::Cut(path),
            ctx.safe_z,
            ctx.cut_tolerance,
            ctx.min_cutting_radius,
            &ctx.stamp_drape(),
        );
    }
    if let Some(band) = ctx.band_edge.borrow_mut().as_mut() {
        band.scan(segments);
    }
}
'''
p = A / "clearing.rs"; s = p.read_text()
marker = "\n#[cfg(test)]\nmod region_order_tests {"
assert marker in s
s = s.replace(marker, code + marker, 1)
old = """    pub(super) left_pieces: std::cell::RefCell<Vec<(f64, crate::polygon::Polygon2)>>,
}"""
assert old in s
s = s.replace(old, """    pub(super) left_pieces: std::cell::RefCell<Vec<(f64, crate::polygon::Polygon2)>>,
    /// D68 prototype (G-RESTDOC option 2): the reach band and its pass.
    pub(super) band_edge: std::cell::RefCell<Option<BandEdge>>,
}""", 1)
p.write_text(s)
p = A / "path.rs"; s = p.read_text()
old = """        left_pieces: std::cell::RefCell::new(Vec::new()),
    };"""
assert old in s
s = s.replace(old, """        left_pieces: std::cell::RefCell::new(Vec::new()),
        band_edge: std::cell::RefCell::new(
            std::env::var_os("RS_CAM_D68_OPTION2")
                .is_some()
                .then(|| {
                    prior_tops.as_ref().and_then(|prior| {
                        super::clearing::BandEdge::new(
                            &material_stock,
                            prior,
                            centre_clip.as_ref(),
                            tool_radius,
                        )
                    })
                })
                .flatten(),
        ),
    };""", 1)
old = """    if let Some(scope) = level_scope {
        let tally = tally_segments_for_z_level"""
assert old in s
s = s.replace(old, """    // D68 prototype (G-RESTDOC option 2): the edge pass of this level.
    super::clearing::band_edge_pass(ctx, material_stock, surface_hm, z_level, segments, cursor);
""" + old, 1)
p.write_text(s)
print("ok")
