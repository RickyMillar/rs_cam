# Gaps and Issues

Catalog of every discrepancy found: GUI gaps, placeholders, overlapping parameters, inconsistencies.

---

## 1. Config Fields NOT Exposed in GUI

These fields exist in GUI state config structs but have no corresponding GUI control.

| Operation | Config field | Type | Default | Impact |
|-----------|-------------|------|---------|--------|
| DropCutter | `skip_air_cuts` | bool | false | Dead field — user cannot toggle air-cut skipping for 3D finish |
| DropCutter | `slope_from` | f64 | 0.0 | Dead field — user cannot restrict to steep-only regions |
| DropCutter | `slope_to` | f64 | 90.0 | Dead field — user cannot restrict to shallow-only regions |
| Pencil | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| Scallop | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| SteepShallow | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| RampFinish | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| SpiralFinish | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| RadialFinish | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |
| HorizontalFinish | `stock_to_leave_radial` | f64 | 0.0 | Only axial shown; radial exists but is hidden |

**Decision needed**: For the 3D finishing operations, are `stock_to_leave_radial` fields:
- (a) Intentionally hidden because they're not meaningful for these operations? (Finishing ops typically only leave axial stock.)
- (b) Unfinished — should be exposed with separate "Wall Stock" and "Floor Stock" controls like Adaptive3D?

**Recommendation**: For 3D finishing ops, axial-only stock-to-leave makes sense (these are surface-following). The radial field could be removed from their configs to eliminate confusion. Adaptive3D is the exception because it cuts walls (needs radial).

---

## 2. GUI Controls That Are Placeholders / Not Fully Wired

| Operation | GUI control | Issue | Status |
|-----------|------------|-------|--------|
| Profile | `CompensationType::InControl` | GUI shows "In Control" option but G41/G42 is NOT emitted in G-code export | **Known partial** (documented in FEATURE_CATALOG.md) |
| All | Workholding Rigidity | Feeds calculator supports it, but GUI hardcodes `Medium` | **Known partial** (documented) |
| All | `pre_gcode` / `post_gcode` | Editable in GUI but NOT emitted during export | **Known partial** (documented) |

---

## 3. Core Params Not Matching GUI Configs

Fields that exist in core `*Params` structs but have no corresponding GUI config field (aside from auto-derived fields like `tool_radius`).

| Core struct | Core field | GUI config | Notes |
|-------------|-----------|------------|-------|
| `AdaptiveParams` | `initial_stock` | — | Auto-populated from StockSource, not user-editable. OK. |
| `Adaptive3dParams` | `initial_stock` | — | Same as above. OK. |
| `Adaptive3dParams` | `max_stay_down_dist` | — | Computed internally (tool_radius × 6), not exposed. OK for now. |
| `PencilParams` | (all fields) | `PencilConfig` | Core has single `stock_to_leave`; config splits into radial/axial. Only axial passed through. |

---

## 4. Default Inconsistencies Between Core and GUI Config

Where the core `impl Default` and GUI `impl Default` disagree:

| Parameter | Core default | GUI config default | Issue |
|-----------|-------------|-------------------|-------|
| `SteepShallowParams::threshold_angle` | 40.0 | `SteepShallowConfig`: 45.0 | Mismatch — GUI users get 45°, CLI/tests get 40° |
| `SteepShallowParams::overlap_distance` | 4.0 | `SteepShallowConfig`: 1.0 | Mismatch — GUI is much smaller |
| `SteepShallowParams::wall_clearance` | 2.0 | `SteepShallowConfig`: 0.5 | Mismatch — GUI is much smaller |
| `ScallopParams::scallop_height` | 0.01 | `ScallopConfig`: 0.1 | Mismatch — core is 10× finer than GUI |
| `RampFinishParams::slope_from` | 0.0 | `RampFinishConfig`: 30.0 | Mismatch — GUI excludes shallow areas by default |

**Impact**: These are mostly cosmetic since the GUI config defaults are what users actually see. The core defaults only matter for CLI/test usage. But they should be aligned to avoid confusion.

---

## 5. Parameter Overlap / Redundancy Analysis

### Potentially confusing parameter pairs

| Pair | Operations | Overlap? | Verdict |
|------|-----------|----------|---------|
| `depth` vs `bottom_z` (Heights) | 2.5D ops | Both control how deep to cut. `depth` is on the Params tab, `bottom_z` is on Heights tab. | **Potential confusion**: changing depth should update bottom_z and vice versa. Currently `bottom_z` Auto mode uses `op_depth`, which comes from the depth field. This is correct but the interaction is implicit. |
| `safe_z` (core) vs `retract_z` / `clearance_z` (Heights) | All ops | Core param `safe_z` is derived from Heights, not a separate user control. | **OK** — no overlap; Heights system produces safe_z for core. |
| `depth_per_pass` vs `fine_stepdown` (Adaptive3D) | Adaptive3D | `depth_per_pass` = main Z stepping; `fine_stepdown` = insert intermediate Z levels. Different functions. | **OK** — independent; fine_stepdown subdivides within depth_per_pass steps. |
| `stock_to_leave_axial` vs `stock_to_leave_radial` (Adaptive3D) | Adaptive3D | Axial = floor; Radial = walls. Different dimensions. | **OK** — independent. |
| `tolerance` vs `sampling` | Various 3D ops | Tolerance = path simplification; Sampling = fiber/grid spacing. Different things. | **OK** — independent. |
| `stepover` vs `scallop_height` (Scallop) | Scallop | Scallop uses scallop_height as primary control, computes stepover internally. No explicit stepover. | **OK** — scallop_height IS the stepover control, just expressed differently. |
| `z_step` (Waterline) vs `depth_per_pass` (other ops) | Waterline vs others | Same concept (Z level spacing) but different names. Waterline also has `start_z`/`final_z` instead of using Heights. | **MILD INCONSISTENCY**: Waterline has its own Z system outside Heights. |
| `Pocket.depth` + `Pocket.depth_per_pass` vs Heights `top_z` + `bottom_z` | Pocket | Pocket has `depth` on Params tab AND Heights has `bottom_z`. If both are manual, which wins? | **POTENTIAL BUG**: Need to verify that Pocket.depth is used as `op_depth` for Heights Auto, and that manual Heights bottom_z overrides it. |

### No true duplicates found

No two parameters on the same operation do the same thing. The closest is `depth` (Params tab) vs `bottom_z` (Heights tab), but these are intentionally layered: depth is the "simple" control, Heights is the "advanced" override.

---

## 6. Naming Inconsistencies

| Concept | Different names used | Operations |
|---------|---------------------|------------|
| Cut depth | `depth`, `cut_depth`, `pocket_depth`, `max_depth` | Face vs Pocket vs Inlay vs VCarve |
| Step down | `depth_per_pass`, `max_stepdown`, `z_step`, `fine_stepdown` | Most ops vs RampFinish vs Waterline vs Adaptive3D |
| Stock to leave | `stock_to_leave`, `stock_to_leave_axial`, `stock_to_leave_radial` | Core vs GUI config |
| Sampling | `sampling`, `point_spacing`, `tolerance` | Waterline/Pencil vs Radial/ProjectCurve vs VCarve |
| Cut direction | `climb: bool`, `CutDirection` enum, `FaceDirection` enum | Pocket/Profile vs RampFinish vs Face |
| Line angle | `angle`, `angular_step` | Zigzag/Rest vs RadialFinish |

**Assessment**: These are reasonable per-operation names that reflect domain-specific meanings. Not bugs, but worth documenting for test clarity.

---

## 7. Waterline Height System Divergence

Waterline uses its own `start_z` / `final_z` / `z_step` instead of the standard Heights system for depth control:

| Standard Height | Waterline equivalent | Notes |
|----------------|---------------------|-------|
| `top_z` | `start_z` | Waterline has its own start Z |
| `bottom_z` | `final_z` | Waterline has its own final Z |
| depth_per_pass | `z_step` | Waterline has its own step |

The standard Heights system still applies for clearance/retract/feed Z, but the depth range is independently controlled. This means:
- Changing Heights `bottom_z` does NOT change Waterline `final_z`
- User must manage two sets of Z controls

**Risk**: User changes stock height, Heights auto-updates, but Waterline `start_z`/`final_z` remain stale at 0.0/-25.0.

---

## 8. Summary of Action Items

### Must fix (bugs/confusion)
1. **Waterline Z divergence**: Either wire Waterline to Heights system or clearly document the separation
2. **SteepShallow default mismatch**: Align core and GUI defaults

### Should fix (completeness)
3. **DropCutter missing GUI fields**: Expose `slope_from`/`slope_to` (already in config); decide on `skip_air_cuts`
4. **stock_to_leave_radial cleanup**: Either expose in GUI for all 3D finishing ops or remove from their configs
5. **Adaptive3D stock_top_z**: Should auto-initialize from stock bounds, not hardcoded 30mm

### Nice to have (polish)
6. **Tool-relative dressup defaults**: Scale lead_radius, helix_radius, link_max_distance with tool
7. **Feeds auto-trigger**: Consider recalculating on tool/material change, not just on tab render
8. **Workholding rigidity**: Expose the existing feeds calculator input in the GUI
