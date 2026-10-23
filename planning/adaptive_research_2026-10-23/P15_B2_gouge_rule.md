# P1.5 editor B2 — the Allowance rule after the evidence run

Worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`
(base 405cbd23, with B's commit ce94b369). Not built and not run (the
verifier runs cargo). Not committed.

## The input

`P15_E_evidence.md` (main checkout), gouge section. The B rule fired on
all eight Phase 0 roughs, and the export of each one refused.

- **Corne op 1** (all three strategies): "3.200 mm" at (30.380, −38.583),
  Z 0.5. This is a radial graze of 0.02 to 0.05 mm on a hex-hole wall. The
  Z-only measure gave the wall height above the tip.
- **Corne op 0:** 0.064 mm past the limit at (109.279, −4.017), Z 17, on a
  45° rim chamfer. The tip is about 0.155 mm above the model. The slack in
  effect is about 0.3 mm (the segment merge tolerance), not 0.1 mm.
- **wanaka "2 Back Rough":** 0.098 mm past the limit with a 4 mm leave.
- **No false positives:** none at AS move 1401 and none at wanaka link
  5147.

Operator ruling: a default rough must export, and a graze within the op's
own tolerance is not a defect.

## The rule now (`compute/model_gouge.rs`, `ModelGougeRule::Allowance`)

With the leave `L` and the chord tolerance `t` of the config
(`OperationConfig::model_gouge_allowance` → `ModelGougeAllowance`):

```
t       = Adaptive3dGeometry::chord_tolerance(tolerance, segment_merge_tolerance)
probe   = the session cutter of the tool, radius R − t     (probe_radial_tolerance_mm)
limit   = CL_probe(x, y) + L − t                           (a finding when the tip is below it)
surface = CL_probe(x, y) − t                               (Blocking when the tip is below it)
```

### (a) The radial tolerance is `t`, not `MODEL_GOUGE_TOL_MM`

A 2.5D op cuts its wall at the exact offset, so the CAD rounding is its
only radial error. The 3D Rough plans its path at its chord tolerance. Its
RDP step may move a draped chord by `t` in 3D, so in XY too, and the tool
side may come that close to a wall by design. `probe_radial_tolerance_mm`
holds the rule in one place: the session calls it, and the sentry calls
it.

It keeps `t` between `MODEL_GOUGE_TOL_MM` and `R − MODEL_GOUGE_TOL_MM`.
Both bounds are existing values. They only keep the probe a real tool, so
that `session_cutter` cannot refuse it. The rough has no radial leave
(L2), so nothing else widens the probe.

### (b) The class: one field on the finding, the existing `Severity`

- `ModelGougeFinding::below_surface_mm` is new. It is the signed depth of
  the deepest sample below CL Z − t. Under `Allowance` the limit and that
  surface differ by `L` at every sample, so the deepest sample below the
  limit is also the deepest below the surface:
  `below_surface_mm = deepest_mm − L`. For every other rule it equals
  `deepest_mm`.
- `ModelGougeFinding::severity()` returns
  `Option<diagnostics::Severity>`:
  - `None` when the toolpath is clean;
  - `Blocking` when `below_surface_mm > MODEL_GOUGE_Z_TOL_MM`
    (`blocks_export()`);
  - `Caution` otherwise.
- The 2.5D rules do not change: each of their findings is still
  Blocking.
- `diagnostics/adapters/from_generation.rs::cuts_into_model` takes its
  severity from the finding. The id stays `safety.cuts_into_model`; a
  Caution under that id is a cut into the leave.
- `session/compute/export.rs::model_gouge_refusal` refuses only on
  `blocks_export()`.
- The viz reads only `Severity::Blocking` (`toolpath_panel::blocking_finding`,
  the BLOCKED chip), so a Caution does not read "BLOCKED". No viz source
  changed.

### (c) The message gives the two depths separately, with a wall flag

The Allowance arm of `message()`:

- opens with "The toolpath cuts into the model" (Blocking) or "The
  toolpath cuts into the stock to leave, not into the model" (Caution);
- gives the depth below the limit, the tip Z and the limit Z;
- then gives "The tip is X mm below / within 0.02 mm of / X mm above the
  model surface less the op tolerance";
- ends with "export refuses while this stands" or "export does not refuse
  on it".

`ModelGougeFinding::at_wall` is new (Allowance only). The guard tests the
deepest sample after the walk: one more `point_drop_cutter` with the
existing point probe at the tip XY. The flag is set when the CL Z is more
than the probe radius above the surface under the tip centre, or when no
surface lies under the centre.

Derivation of the test:

- The CL Z of any cutter of radius `r` is at most the CL Z of a flat end
  mill of radius `r`. A ball, a bull nose or a taper lies on or above the
  flat bottom.
- That is at most the surface under the centre plus `r · tan(a)`, where
  `a` is the steepest slope in the disk.
- So a gap of more than `r` proves a slope steeper than 45° within the
  disk: a wall.
- `r` is the PROBE radius R − t, not the tool radius that the request
  named. The CL in the test is the probe's CL, so R − t is the exact
  bound; R would be looser.

The message then says that the depths are the height of that wall above
the tip, and that the tool may touch only its side. The Z-only measure
still cannot give the radial overlap. The text says so in effect, and the
module doc states the limit.

## Expected readings on the Phase 0 roughs (not measured)

- **Corne op 1 (hex hole):** a graze of 0.02 to 0.05 mm, below t ≈ 0.3 mm.
  The probe (R − t) does not reach the wall, so no finding.
- **Corne op 0 (rim chamfer):** the probe radius drops from 2.98 to about
  2.7. The evidence's disk read gives 16.664 at r 2.8, so the CL at 2.7 is
  at or below that. The limit is ≤ 16.664 + 0.5 − 0.3 = 16.864, which is
  below the tip at 17.0, so no finding. If it fires anyway, it can only be
  Caution: the tip is above the surface.
- **wanaka "2 Back Rough":** 0.098 mm into a 4 mm leave. It reads clean,
  or Caution at worst. It is not Blocking.
- The radial slack is absolute, not relative to the tool. Corne op 1 is
  a 3 mm tool (R 1.5) with t ≈ 0.3, so its probe radius is 1.2 mm. Under
  the ruling, a real wall cut of up to about 0.3 mm on that op reads
  clean. That is the price of "a graze within the op's own tolerance is
  not a defect".
- So all eight Phase 0 roughs should export. A Blocking finding now means
  a tip below CL Z − t: a real cut into the model.

## Files changed (B2)

- `crates/rs_cam_core/src/compute/model_gouge.rs`: the module doc (radial
  tolerance, class, wall flag); `ModelGougeAllowance`;
  `probe_radial_tolerance_mm`; the finding fields `below_surface_mm` and
  `at_wall`; `blocks_export`, `severity`; `GougeProbe::allowance` (replaces
  `limit_offset_mm`); the measure arm; the Allowance message.
- `crates/rs_cam_core/src/compute/catalog.rs`:
  `model_gouge_limit_offset_mm` → `model_gouge_allowance` (exhaustive
  match).
- `crates/rs_cam_core/src/session/compute.rs`: `guard_model_gouge`. It
  computes the allowance first, then shrinks the tool by
  `probe_radial_tolerance_mm`, and passes `allowance`.
- `crates/rs_cam_core/src/session/compute/export.rs`: refuses on
  `blocks_export()`.
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs`: the
  severity comes from the finding. `diagnostics/ids.rs`: the doc of the id.
- `crates/rs_cam_core/src/compute/catalog/registry.rs`: `FIX_ROUGH_3D`
  text (no longer "do not cut this toolpath").
- `crates/rs_cam_core/src/compute/CLAUDE.md`: the `model_gouge.rs` entry
  (40 lines).
- Tests: `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge.rs`
  (rewritten). The ux7b sentry has its two probe literals changed to
  `allowance: ModelGougeAllowance::NONE`. These literals get the two new
  fields: `findings_transport_join_h21.rs`, viz
  `ui/properties/tests.rs`, `a_blocked_result_does_not_read_ok_f20.rs`,
  `the_geometry_rows_fit_the_inspector_f22.rs`.
- `planning/.../P15_B_gouge.md`: a "superseded in part" note.
- No change to `every_op_states_its_capabilities_w2_4` (the row is still
  `Allowance`).

## The sentry

`a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` has six tests. They
use a synthetic flat floor at Z 10, a wall mesh (floor at Z 10, a vertical
wall at X 20, a block top at Z 20), a 6 mm flat end mill, and the default
config (L 0.5, t 0.1):

1. `a_rough_at_its_leave_or_grazing_within_its_tolerance_reads_clean_g_roughgouge`:
   at L, at the limit, and with a graze of t/2 into the wall: no finding.
2. `a_cut_into_the_leave_is_a_caution_g_roughgouge`: a tip at
   limit − 2 × Z tolerance gives Caution, depth 0.04,
   `below_surface_mm` = 0.04 − L, and the text "not into the model",
   "mm above", "export does not refuse".
3. `a_cut_below_the_surface_is_blocking_at_the_right_place_g_roughgouge`:
   a tip at CL − t − 2 × Z tolerance gives Blocking, depth L + 0.04, and
   `below_surface_mm` 0.04. A 1 mm dip gives the finding at the bottom of
   the dip, on its move, with depth 1 + L − t and below-surface 1 − t.
4. `a_tool_past_its_tolerance_into_a_wall_is_blocking_and_named_a_wall_g_roughgouge`:
   the tool side t + 0.1 into the wall gives Blocking with `at_wall`, and
   the text "wall steeper than 45 degrees".
5. `the_pocket_rule_reads_no_allowance_g_roughgouge`: the Pocket probe
   keeps `MODEL_GOUGE_TOL_MM`. A tip in the leave band is clean under
   Pocket. A tip below the floor is Blocking, with
   `below_surface_mm == deepest_mm`.
6. `an_allowance_row_has_an_allowance_g_roughgouge`: the row matches the
   accessor, and the allowance is L and max(t, merge).

Commands:

```
cargo test -p rs_cam_core -q --test a_rough_cuts_no_deeper_than_its_leave_g_roughgouge
cargo test -p rs_cam_core -q --test every_op_states_its_capabilities_w2_4 --test findings_transport_join_h21
cargo test -p rs_cam_core -q --features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b
cargo test -p rs_cam_viz -q --lib
cargo test -p rs_cam_viz -q --test a_blocked_result_does_not_read_ok_f20 --test the_geometry_rows_fit_the_inspector_f22
```

I have not injected a defect to see the sentry go red. Two checks to make:

- Make `probe_radial_tolerance_mm` return `MODEL_GOUGE_TOL_MM` for
  Allowance. The graze in test 1 must then fail.
- Make `blocks_export` return `gouges()`. Test 2 must then fail.

## A deviation from the request

The request asked for "a graze case → caution". A radial graze WITHIN `t`
now reads clean (test 1), as the operator ruling asks. A graze PAST `t`
puts the probe on the wall, and the tip is then below that wall's
CL Z − t, so it is Blocking with the wall flag (test 4). It is a real cut
into the model, past the op's own tolerance. The Caution case is the
leave band in Z (test 2).

## Not finished, and requests

- The session wiring has no session-level test. The sentry calls
  `measure` and `probe_radial_tolerance_mm` directly. Only an evidence
  run covers `guard_model_gouge`. Re-run the eight Phase 0 roughs with a
  GUI built from HEAD, and check that all of them export.
- The wall flag needs one more drop-cutter query per finding (only the
  deepest sample). That cost is negligible.
- The finding still reports only the deepest sample. A Caution and a
  Blocking cannot both show on one toolpath: the deepest one sets the
  class, and that is the correct class for export.
- From B, still open: `adaptive3d_effective_stock_to_leave` is not
  reachable from catalog. The accessor reads `stock_to_leave_axial`
  directly, which is the same field.
