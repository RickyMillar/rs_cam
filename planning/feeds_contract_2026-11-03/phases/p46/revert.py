"""Red proofs of P46: revert ONE hunk by hand in the tree.
usage: revert.py <HOLD|CENTRE|RING|UNLEAVE|REACH|STANDS|RECORD|RIM>
Restore: red.sh copies the saved files back."""
import sys
B = "/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/"
def edit(p, a, b):
    s = open(B + p).read(); assert s.count(a) == 1, (p, a[:50], s.count(a)); open(B + p, "w").write(s.replace(a, b))
k = sys.argv[1]
if k == "HOLD":      # P45: emit_runs does not hold the Z along the shipped chords
    edit("level_contour.rs", "            path = hold_shipped_chords(ctx, material_stock, hold, path, cell);",
         "            let _ = hold;")
elif k == "CENTRE":  # P45: pass 1 of the strip read counts each centre inside the disc edge
    edit("level_contour.rs", "            if strip.is_some() && !TriDexelStock::disc_holds_cell_core(du, dv, r_sq, cs) {\n                continue;\n            }\n            let profile = ctx",
         "            let profile = ctx")
elif k == "RING":    # P45: the ring of pass 3 starts at the disc edge for the strip read too
    edit("level_contour.rs", "                d_sq <= r_sq && TriDexelStock::disc_holds_cell_core(du, dv, r_sq, cs)",
         "                d_sq <= r_sq")
elif k == "UNLEAVE": # P46 A: the strip pass removes no left piece
    edit("level_contour.rs", "    let pieces = unleave_cut_pieces(ctx, material_stock, surface_hm, &state);",
         "    let pieces = CutPieces {\n        reached: 0,\n        removed: 0,\n    };")
elif k == "REACH":   # P46 A, rule 1: a piece that no cut of the pass reached is judged too
    edit("level_contour.rs", "        if !cells.iter().any(|&(row, col)| state.cut_at(row, col)) {\n            return false;\n        }",
         "        if cells.is_empty() {\n            return false;\n        }")
elif k == "STANDS":  # P46 A, rule 2: a reached piece is removed with stock on it
    edit("level_contour.rs", "        !super::left_stock::piece_stands(&cells, z_level, stock, surface_hm, ctx.stock_to_leave)",
         "        true")
elif k == "RECORD":  # P46 A: the drop record of a removed piece stays
    edit("clearing.rs", "            unplanned.remove(record);", "            let _ = record;")
elif k == "RIM":     # P46 D: a rim place keeps the reason of the record on its cells
    edit("left_stock.rs", "                if all_rim {\n                    PlaceWhy::LineMargin\n                } else if whys[0] == most {",
         "                if whys[0] == most {")
else:
    sys.exit("unknown hunk " + k)
