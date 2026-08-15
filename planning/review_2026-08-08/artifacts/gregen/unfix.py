"""Restore G-REGEN-RACE's PRE-FIX semantics in place, keeping the plumbing
(the `ToolpathSubmitOutcome` return, the ledger field) so the tree still
compiles and the sentries still run. This is how the red is taken: same
tree, same tests, one behaviour hunk reverted."""
import pathlib

p = pathlib.Path("crates/rs_cam_viz/src/controller/events/compute.rs")
s = p.read_text()
a_new = """                    let expected_supersede = self.superseded_toolpaths.remove(&tp_id);
                    if expected_supersede && matches!(result.result, Err(ComputeError::Cancelled)) {
                        continue;
                    }
"""
a_old = """                    let _expected_supersede = self.superseded_toolpaths.remove(&tp_id);
"""
assert s.count(a_new) == 1, "hunk A not found"
p.write_text(s.replace(a_new, a_old))
print("UNFIXED: the supersede-is-not-an-outcome guard is gone")
