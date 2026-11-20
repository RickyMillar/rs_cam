"""Red proofs of P44b: revert ONE hunk by hand in the tree. usage: revert.py <H1|H2|H3|RULE>
Restore: cp the saved copy back (red.sh does it)."""
import sys
B = "/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/"
def edit(p, a, b):
    s = open(B + p).read(); assert s.count(a) == 1, (p, a[:50], s.count(a)); open(B + p, "w").write(s.replace(a, b))
k = sys.argv[1]
if k == "H1":   # the small-region bound is back (no record)
    edit("clearing.rs", "    let mut regions = regions;\n", """    let mut regions = regions;
    let tool_diameter = ctx.tool_radius * 2.0;
    let baseline_dpp = ctx.tool_radius.max(0.5);
    let nominal_min_area = (tool_diameter * 2.0).powi(2);
    let dpp_factor = (ctx.depth_per_pass / baseline_dpp).clamp(0.0, 1.0);
    let area_floor = tool_diameter.powi(2) * 0.25;
    let min_region_area_mm2 = (nominal_min_area * dpp_factor).max(area_floor);
    regions.retain(|r| r.area().abs() >= min_region_area_mm2);
    if regions.is_empty() {
        return None;
    }
""")
elif k == "H2":  # the forecast insets the region by the tool radius plus 0.25 mm again
    edit("clearing.rs", """            super::level_contour::contour_line_inset(
                sink.material_stock.z_grid.cell_size,
                ctx.cut_tolerance,
            ),
""", "            ctx.tool_radius + 0.25,\n")
elif k == "H3":  # no region of a level gets the region contour
    edit("clearing.rs", "        &uncut_regions,\n        segments,\n", "        &[],\n        segments,\n")
elif k == "RULE":  # a place with no drop record raises the finding
    edit("left_stock.rs", "self.standing_pieces > 0 || self.held_off_area_mm2 > 0.0 || self.places_total > 0",
         "self.standing_pieces > 0\n            || self.held_off_area_mm2 > 0.0\n            || self.places_total > 0\n            || self.unrecorded_places > 0")
