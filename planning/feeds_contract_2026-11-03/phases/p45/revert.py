"""Red proofs of P45: revert ONE hunk by hand in the tree. usage: revert.py <PASS|READ|SCAN|EDGE>
Restore: red.sh copies the saved files back."""
import sys
B = "/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/"
def edit(p, a, b):
    s = open(B + p).read(); assert s.count(a) == 1, (p, a[:50], s.count(a)); open(B + p, "w").write(s.replace(a, b))
k = sys.argv[1]
if k == "PASS":    # no strip pass after the last level
    edit("path.rs", "        && params.clearing_strategy == ClearingStrategy3d::GeodesicAdaptive\n    {\n        check_cancel(cancel)?;\n        super::level_contour::strip_pass(",
         "        && params.clearing_strategy == ClearingStrategy3d::GeodesicAdaptive\n        && false\n    {\n        check_cancel(cancel)?;\n        super::level_contour::strip_pass(")
elif k == "READ":  # the strip pass reads the ray top (the read of P44 build 2)
    edit("level_contour.rs", "            let top = strip.map_or_else(|| stock_top_z_at(stock, row, col), |s| s.at(row, col));",
         "            let top = stock_top_z_at(stock, row, col);")
elif k == "SCAN":  # the strip state does not follow the cuts of a round
    edit("level_contour.rs", "        if let Some(state) = strip.as_deref_mut() {\n            state.scan(segments, ctx);\n        }",
         "        if let Some(state) = strip.as_deref_mut() {\n            let _ = &state;\n        }")
elif k == "EDGE":  # the state counts a centre on the disc edge as cut
    edit("level_contour.rs", "                if !TriDexelStock::disc_holds_cell_core(du, dv, r_sq, self.cell) {\n                    continue;\n                }\n                let Some(profile) = lut.height_at_dist_sq(d_sq) else {",
         "                if d_sq > r_sq {\n                    continue;\n                }\n                let Some(profile) = lut.height_at_dist_sq(d_sq) else {")
